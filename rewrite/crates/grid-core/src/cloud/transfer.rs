//! Sidecar image lookup, upload job builders, short-circuit checks, and
//! upload completion messages for the cloud-save engine.
//!
//! Ported from `grid_launcher/library/cloud_transfer.py` (image sidecars
//! :25-100, `normalize_candidate_url` :133-139, download candidate paths
//! :141-149,214-222, grouped/native job helpers :354-476,
//! `ppsspp_state_upload_jobs`/`retroarch_state_upload_jobs` :590-660,
//! `filter_upload_jobs_by_session_window` :668-687, the short-circuit
//! predicates `should_skip_known_latest`/`is_local_newer_than_server`
//! :705-712) and `grid_launcher/library/cloud_upload.py` (`file_upload_jobs`
//! /`directory_archive_upload_jobs` :9-25, `no_matching_upload_message`
//! :29-34, `upload_completion_message` :37-60). The shared-single job name
//! (`f"{emulator_name or 'Shared Save'} Storage"`) is
//! `grid_launcher/ui/mixins/cloud_mixin.py:2519-2527`.
//!
//! Archive writing itself (zipping, temp paths) lives in
//! [`super::archive`] — this module only decides WHAT to zip and what to
//! name the result, never how bytes get written.

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Regex;

use super::archive;
use super::window::Window;
use super::{file_mtime_secs, IgnoreSets, SaveType};

/// `cloud_transfer.py:25-31`'s `SUPPORTED_IMAGE_EXTENSIONS`, canonical home
/// (`cloud::tokens` previously carried a private duplicate pending this
/// task; it now defers to this constant — see that module's one-line diff).
pub const SUPPORTED_IMAGE_EXTENSIONS: [&str; 6] =
    [".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp"];

// --- job types -----------------------------------------------------------

/// One upload request: a display name shown to the user, and an ordered
/// payload of `(field name, path)` pairs — field names are the literal
/// wire strings `"saveFile"` / `"stateFile"` / `"screenshotFile"`, main
/// file first, an optional screenshot sidecar appended. Mirrors the Python
/// `(display_name, {field: path, ...})` tuple shape, with the dict
/// replaced by an order-preserving `Vec` (Python dicts are
/// insertion-ordered too, so this is an exact behavioral match, not just
/// an approximation).
#[derive(Debug, Clone, PartialEq)]
pub struct UploadJob {
    pub display_name: String,
    pub payload: Vec<(String, PathBuf)>,
}

/// A batch of upload jobs plus every temp archive created while building
/// them — the caller must clean up `temp_archives` (via
/// [`super::archive::cleanup_temp_archives`]) once the jobs have been sent,
/// regardless of which jobs survived any later filtering. Mirrors the
/// `(upload_jobs, temporary_archives)` tuple every Python job builder
/// returns.
#[derive(Debug, Default)]
pub struct BuiltJobs {
    pub jobs: Vec<UploadJob>,
    pub temp_archives: Vec<PathBuf>,
}

// --- sidecar lookup --------------------------------------------------------

/// The first existing supported-image sidecar formed by REPLACING `path`'s
/// own suffix with each of [`SUPPORTED_IMAGE_EXTENSIONS`] in turn (e.g.
/// `game.ppst` -> `game.png`), `None` if none exist. Mirrors
/// `supported_image_sidecar_path` (`cloud_transfer.py:54-68`).
///
/// Python's version also takes a `blocked_basenames` set (defaulting to
/// the four OS-metadata names) and skips a candidate whose name matches
/// one — but every candidate here is `<path's stem>.<image ext>`, which can
/// never equal `.ds_store`/`desktop.ini`/`ehthumbs.db`/`thumbs.db` (none of
/// those end in a supported image extension), so that check can never
/// actually fire. Dropped from this signature as dead weight, not a
/// behavior change.
pub fn replaced_suffix_sidecar_path(path: &Path) -> Option<PathBuf> {
    for extension in SUPPORTED_IMAGE_EXTENSIONS {
        let candidate = path.with_extension(&extension[1..]);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// The first existing supported-image sidecar formed by APPENDING each of
/// [`SUPPORTED_IMAGE_EXTENSIONS`] to `path`'s complete file name (e.g.
/// `game.state1` -> `game.state1.png`), `None` if none exist. Mirrors
/// `appended_image_sidecar_path` (`cloud_transfer.py:70-84`); the
/// `blocked_basenames` parameter is dropped for the same reason as
/// [`replaced_suffix_sidecar_path`] (a candidate here always ends in a
/// supported image extension, never one of the four blocked basenames).
pub fn appended_image_sidecar_path(path: &Path) -> Option<PathBuf> {
    for extension in SUPPORTED_IMAGE_EXTENSIONS {
        let mut name = path.as_os_str().to_os_string();
        name.push(extension);
        let candidate = PathBuf::from(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Recursively lists every entry under `root` — `None` if ANY `read_dir`
/// call anywhere in the walk fails (the root itself, or any nested
/// subdirectory, or a per-entry read error while iterating one), discarding
/// whatever had already been collected. Mirrors Python's
/// `list(directory.rglob("*"))` wrapped in exactly ONE `try/except
/// OSError: continue` per TOP-LEVEL directory (`cloud_transfer.py:109-113`):
/// an OSError anywhere in that directory's recursive walk — root or
/// nested — discards that entire top-level directory's results, not just
/// the failing subtree. There is no "keep the siblings" partial-success
/// path here, unlike [`super::latest_mtime_under`]'s per-entry tolerance.
fn walk_files_best_effort(root: &Path) -> Option<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let read_dir = std::fs::read_dir(&dir).ok()?;
        for entry in read_dir {
            let entry = entry.ok()?;
            let path = entry.path();
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                stack.push(path.clone());
            }
            out.push(path);
        }
    }
    Some(out)
}

/// The most recently captured screenshot from `dirs` whose mtime falls
/// within the inclusive `window`. `None` when `dirs` is empty, `window` is
/// `None`, or nothing qualifies. Recurses into every directory; skips
/// non-files, unsupported extensions, and basenames blocked by
/// `ignore.basenames` — NOT `ignore.extensions`, which Python's own
/// `session_screenshot_path` never consults (it filters strictly by
/// [`SUPPORTED_IMAGE_EXTENSIONS`] instead). Mirrors `session_screenshot_path`
/// (`cloud_transfer.py:89-121`).
pub fn session_screenshot_path(
    dirs: &[PathBuf],
    window: Option<Window>,
    ignore: &IgnoreSets,
) -> Option<PathBuf> {
    let (start, end) = window?;
    if dirs.is_empty() {
        return None;
    }

    let mut best: Option<PathBuf> = None;
    let mut best_mtime = -1.0_f64;

    for dir in dirs {
        let Some(candidates) = walk_files_best_effort(dir) else {
            continue;
        };
        for candidate in candidates {
            if !candidate.is_file() {
                continue;
            }
            let extension = candidate
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e.to_lowercase()))
                .unwrap_or_default();
            if !SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            let name = candidate
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_lowercase();
            if ignore.basenames.contains(&name) {
                continue;
            }
            let Some(mtime) = file_mtime_secs(&candidate) else {
                continue;
            };
            if start <= mtime && mtime <= end && mtime > best_mtime {
                best = Some(candidate);
                best_mtime = mtime;
            }
        }
    }

    best
}

// --- URL normalization -----------------------------------------------------

/// Everything outside the RFC 3986 unreserved set (`ALPHA / DIGIT / - . _
/// ~`), plus `/` and `%`, is left untouched; every other byte (and every
/// non-ASCII byte) is percent-encoded. Mirrors `quote(parsed.path,
/// safe="/%")`.
static PATH_ENCODE_SET: LazyLock<AsciiSet> = LazyLock::new(|| {
    NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~')
        .remove(b'/')
        .remove(b'%')
});

/// Everything outside the RFC 3986 unreserved set is percent-encoded — no
/// extra safe characters. Mirrors `quote_via=quote` called through
/// `urlencode(..., safe='')` (its default), i.e. `quote(v, '')`.
static QUERY_ENCODE_SET: LazyLock<AsciiSet> = LazyLock::new(|| {
    NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~')
});

/// Schemes Python's `urllib.parse.uses_netloc` treats as always having a
/// `//netloc` component, even when netloc happens to be empty — scoped to
/// the schemes this function's real callers (RomM download URLs) ever pass,
/// not the full stdlib list.
fn scheme_always_has_netloc(scheme: &str) -> bool {
    matches!(scheme, "http" | "https" | "ftp" | "ws" | "wss")
}

/// A minimal `urlsplit` equivalent for the two shapes this function's
/// callers ever pass: an absolute `scheme://netloc/path?query#fragment` URL,
/// or a bare `/path?query` server-relative reference. Scheme detection
/// mirrors `urlsplit`'s rule: a `:` that appears before any `/` in the
/// string, whose preceding text is a valid `ALPHA (ALPHA|DIGIT|+|-|.)*`
/// scheme token.
fn split_url(raw: &str) -> (String, String, String, String, String) {
    let (before_fragment, fragment) = match raw.find('#') {
        Some(i) => (&raw[..i], raw[i + 1..].to_string()),
        None => (raw, String::new()),
    };
    let (before_query, query) = match before_fragment.find('?') {
        Some(i) => (&before_fragment[..i], before_fragment[i + 1..].to_string()),
        None => (before_fragment, String::new()),
    };

    let mut scheme = String::new();
    let mut rest = before_query;
    if let Some(colon_idx) = before_query.find(':') {
        let candidate_scheme = &before_query[..colon_idx];
        let slash_idx = before_query.find('/');
        let looks_like_scheme = !candidate_scheme.is_empty()
            && candidate_scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && candidate_scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            && slash_idx.is_none_or(|si| si > colon_idx);
        if looks_like_scheme {
            scheme = candidate_scheme.to_lowercase();
            rest = &before_query[colon_idx + 1..];
        }
    }

    let (netloc, path) = if let Some(stripped) = rest.strip_prefix("//") {
        match stripped.find('/') {
            Some(i) => (stripped[..i].to_string(), stripped[i..].to_string()),
            None => (stripped.to_string(), String::new()),
        }
    } else {
        (String::new(), rest.to_string())
    };

    (scheme, netloc, path, query, fragment)
}

/// Mirrors `urlunsplit`'s netloc-prefixing rule: add `//netloc` ahead of
/// `path` (forcing a leading `/` on `path` first) when `netloc` is
/// non-empty, or when `scheme` is one of [`scheme_always_has_netloc`] and
/// `path` doesn't already start with `//`.
fn join_url(scheme: &str, netloc: &str, path: &str, query: &str, fragment: &str) -> String {
    let needs_netloc = !netloc.is_empty()
        || (!scheme.is_empty() && scheme_always_has_netloc(scheme) && !path.starts_with("//"));

    let mut url = if needs_netloc {
        let mut p = path.to_string();
        if !p.is_empty() && !p.starts_with('/') {
            p = format!("/{p}");
        }
        format!("//{netloc}{p}")
    } else {
        path.to_string()
    };

    if !scheme.is_empty() {
        url = format!("{scheme}:{url}");
    }
    if !query.is_empty() {
        url = format!("{url}?{query}");
    }
    if !fragment.is_empty() {
        url = format!("{url}#{fragment}");
    }
    url
}

/// `application/x-www-form-urlencoded` decode of one query component: `+`
/// becomes a space, then percent-escapes are decoded, invalid UTF-8
/// replaced with U+FFFD. Mirrors `parse_qsl`'s per-component
/// `unquote(value.replace('+', ' '))`.
fn decode_query_component(raw: &str) -> String {
    let spaced = raw.replace('+', " ");
    percent_encoding::percent_decode_str(&spaced)
        .decode_utf8_lossy()
        .into_owned()
}

/// Re-encodes `value` for a query component using [`QUERY_ENCODE_SET`].
fn encode_query_component(value: &str) -> String {
    utf8_percent_encode(value, &QUERY_ENCODE_SET).to_string()
}

/// Percent-encodes `raw`'s path with `safe="/%"`, and re-encodes its query
/// string (decode each `name=value` pair keeping blank values, then
/// re-encode with no extra safe characters — round-tripping through
/// `application/x-www-form-urlencoded` the way `quote`-style, non-`+`
/// re-encoding does). Mirrors `normalize_candidate_url`
/// (`cloud_transfer.py:133-139`).
pub fn normalize_candidate_url(raw: &str) -> String {
    let (scheme, netloc, path, query, fragment) = split_url(raw);

    let encoded_path = utf8_percent_encode(&path, &PATH_ENCODE_SET).to_string();

    let encoded_query = if query.is_empty() {
        String::new()
    } else {
        query
            .split('&')
            .filter(|segment| !segment.is_empty())
            .map(|segment| match segment.split_once('=') {
                Some((name, value)) => (name, value),
                None => (segment, ""),
            })
            .map(|(name, value)| {
                let decoded_name = decode_query_component(name);
                let decoded_value = decode_query_component(value);
                format!(
                    "{}={}",
                    encode_query_component(&decoded_name),
                    encode_query_component(&decoded_value)
                )
            })
            .collect::<Vec<_>>()
            .join("&")
    };

    join_url(&scheme, &netloc, &encoded_path, &encoded_query, &fragment)
}

// --- download candidate paths ----------------------------------------------

/// `download_path`, `file_path`, `full_path` (in that order), trimmed,
/// blanks and non-string values skipped. Shared body for both candidate
/// functions below.
fn candidate_paths_from_record(record: &serde_json::Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in ["download_path", "file_path", "full_path"] {
        if let Some(value) = record.get(key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                candidates.push(trimmed.to_string());
            }
        }
    }
    candidates
}

/// Ordered download-location candidates from a state record's
/// `download_path`/`file_path`/`full_path` fields. Mirrors
/// `state_download_candidate_paths` (`cloud_transfer.py:141-149`).
pub fn state_content_candidate_paths(record: &serde_json::Value) -> Vec<String> {
    candidate_paths_from_record(record)
}

/// Ordered download-location candidates from a screenshot record's
/// `download_path`/`file_path`/`full_path` fields — same three keys, same
/// shape as [`state_content_candidate_paths`]. Mirrors
/// `screenshot_download_candidate_paths` (`cloud_transfer.py:214-222`).
///
/// `record` here is the screenshot record itself (what Python calls
/// `state_record["screenshot"]`), not the enclosing state record — the
/// caller (`cloud_mixin.py:1798-1806`) is the one that extracts
/// `state_record["screenshot"]` and checks `missing_from_fs` BEFORE calling
/// this function; neither of those two steps lives inside the Python
/// function this ports, so neither lives here either.
pub fn screenshot_download_candidate_paths(record: &serde_json::Value) -> Vec<String> {
    candidate_paths_from_record(record)
}

// --- shared file-selection helper -------------------------------------------

/// `files` filtered to existing regular files, deduped by lowercased path
/// string (first occurrence wins), order preserved. Mirrors
/// `_unique_existing_files` (`cloud_transfer.py:298-309`) — duplicated from
/// [`super::archive`]'s private copy of the same helper rather than
/// exposed from there, matching that module's own precedent (its
/// `unique_existing_files` isn't `pub` either).
fn unique_existing_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for path in files {
        if !path.is_file() {
            continue;
        }
        let key = path.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            unique.push(path.clone());
        }
    }
    unique
}

// --- grouped / directory / shared-single job builders -----------------------

/// `cloud_transfer.py:353-359`'s `_grouped_upload_key`: the lowercased FULL
/// file name for `"stateFile"`, else the lowercased STEM (falling back to
/// the full name when the stem is blank).
fn grouped_upload_key(path: &Path, field: &str) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if field == "stateFile" {
        return name.to_lowercase();
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if stem.is_empty() {
        name.to_lowercase()
    } else {
        stem.to_lowercase()
    }
}

/// Groups `files` by [`grouped_upload_key`] (insertion order of first
/// appearance), then per group: a lone file becomes a raw upload named
/// after its own file name; two or more are archived together (via
/// [`super::archive::zip_grouped_files_for_upload`], named from `title`)
/// under a display name taken from the FIRST file's stem (falling back to
/// the archive's own stem, then the first file's full name, if that stem
/// is blank). Mirrors `grouped_file_upload_jobs`
/// (`cloud_transfer.py:361-395`).
pub fn grouped_file_upload_jobs(
    files: &[PathBuf],
    field: &str,
    title: &str,
) -> io::Result<BuiltJobs> {
    let selected = unique_existing_files(files);
    if selected.is_empty() {
        return Ok(BuiltJobs::default());
    }

    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in &selected {
        let key = grouped_upload_key(file, field);
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(file.clone());
    }

    let mut jobs = Vec::new();
    let mut temp_archives = Vec::new();
    for key in order {
        let group = groups.remove(&key).unwrap_or_default();
        if group.is_empty() {
            continue;
        }
        if group.len() == 1 {
            let file = &group[0];
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            jobs.push(UploadJob {
                display_name: name,
                payload: vec![(field.to_string(), file.clone())],
            });
            continue;
        }

        let archive_path = archive::zip_grouped_files_for_upload(&group, title)?;
        temp_archives.push(archive_path.clone());
        let first_stem = group[0]
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let display_name = if !first_stem.is_empty() {
            first_stem
        } else if let Some(archive_stem) = archive_path.file_stem().and_then(|s| s.to_str()) {
            if !archive_stem.is_empty() {
                archive_stem.to_string()
            } else {
                group[0]
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string()
            }
        } else {
            group[0]
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string()
        };
        jobs.push(UploadJob {
            display_name,
            payload: vec![(field.to_string(), archive_path)],
        });
    }

    Ok(BuiltJobs {
        jobs,
        temp_archives,
    })
}

/// One archive per directory (via
/// [`super::archive::zip_directory_for_upload`]), named after the
/// directory's own basename — both the archive's job display name AND (via
/// `zip_directory_for_upload`) its member-name prefix. Field name is
/// hardcoded `"saveFile"`: Python's `directory_archive_upload_jobs`
/// (`cloud_upload.py:13-25`) is parameterized on `file_field`, but its only
/// call site (`cloud_mixin.py:2501`) is inside the `save_type == "save"`
/// branch, where `file_field` is always `"saveFile"`.
pub fn directory_archive_upload_jobs(
    dirs: &[PathBuf],
    ignore: &IgnoreSets,
) -> io::Result<BuiltJobs> {
    let mut jobs = Vec::new();
    let mut temp_archives = Vec::new();
    for dir in dirs {
        let archive_path = archive::zip_directory_for_upload(dir, ignore)?;
        temp_archives.push(archive_path.clone());
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        jobs.push(UploadJob {
            display_name: name,
            payload: vec![("saveFile".to_string(), archive_path)],
        });
    }
    Ok(BuiltJobs {
        jobs,
        temp_archives,
    })
}

/// All of `files` archived together into ONE upload job, regardless of
/// [`grouped_upload_key`] grouping — the "shared-single" save scope.
/// Mirrors `cloud_mixin.py:2519-2527`: `shared_archive =
/// zip_selected_files_for_upload(save_files, ...)`, `grouped_jobs =
/// [(f"{emulator_name or 'Shared Save'} Storage", {"saveFile":
/// shared_archive})]` — `display_name` here is that already-computed
/// string, passed in by the caller rather than recomputed. Field name is
/// hardcoded `"saveFile"`: this scope only ever applies within the
/// `save_type == "save"` branch.
pub fn shared_single_upload_job(
    files: &[PathBuf],
    display_name: &str,
    title: &str,
) -> io::Result<BuiltJobs> {
    let archive_path = archive::zip_grouped_files_for_upload(files, title)?;
    Ok(BuiltJobs {
        jobs: vec![UploadJob {
            display_name: display_name.to_string(),
            payload: vec![("saveFile".to_string(), archive_path.clone())],
        }],
        temp_archives: vec![archive_path],
    })
}

// --- ppsspp / retroarch job builders ----------------------------------------

/// `[^A-Z0-9]+` — strips everything but uppercase letters/digits from a
/// candidate's uppercased file name before token containment matching.
/// Mirrors `cloud_transfer.py:610`'s inline `re.sub(r"[^A-Z0-9]+", "",
/// state_file.name.upper())`; duplicated here rather than reused from
/// `cloud::tokens` (whose equivalent pattern is a private module static)
/// per that module's own precedent of small, local regex duplication over
/// cross-module plumbing for one-liners.
static PPSSPP_NAME_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Z0-9]+").unwrap());

/// Every `*.ppst` file directly under each of `dirs` (non-recursive,
/// case-sensitive extension match — Python's `directory.glob("*.ppst")`),
/// not blocked by `ignore` (basename OR extension — mirrors
/// `cloud_transfer.py:608-609`'s pair of `blocked_basenames`/
/// `blocked_extensions` checks on the PRIMARY state file, applied before
/// the token match), whose uppercased name with every non-`[A-Z0-9]`
/// character stripped CONTAINS at least one of `tokens` as a substring —
/// or every non-blocked `.ppst` file when `tokens` is empty. Each
/// candidate's `"stateFile"` job carries a [`replaced_suffix_sidecar_path`]
/// screenshot when one exists (the screenshot sidecar itself is never
/// ignore-checked — see that function's doc comment for why Python's own
/// `blocked_basenames` there is dead code). Results are sorted NEWEST FIRST
/// by mtime (missing/unstat-able treated as `0.0`), then deduped by
/// lowercased path (first — i.e. newest — occurrence wins). Mirrors
/// `ppsspp_state_upload_jobs` (`cloud_transfer.py:590-632`); never produces
/// a temp archive.
pub fn ppsspp_state_upload_jobs(
    dirs: &[PathBuf],
    tokens: &BTreeSet<String>,
    ignore: &IgnoreSets,
) -> BuiltJobs {
    let mut candidates: Vec<(PathBuf, Option<PathBuf>, f64)> = Vec::new();

    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_ppst = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "ppst");
            if !is_ppst {
                continue;
            }
            if ignore.blocks(&path) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let normalized = PPSSPP_NAME_STRIP_RE
                .replace_all(&name.to_uppercase(), "")
                .into_owned();
            if !tokens.is_empty()
                && !tokens
                    .iter()
                    .any(|token| normalized.contains(token.as_str()))
            {
                continue;
            }
            let screenshot = replaced_suffix_sidecar_path(&path);
            let mtime = file_mtime_secs(&path).unwrap_or(0.0);
            candidates.push((path, screenshot, mtime));
        }
    }

    // Stable sort, descending mtime — ties keep discovery order, matching
    // Python's stable `list.sort(..., reverse=True)`.
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut jobs = Vec::new();
    let mut seen = BTreeSet::new();
    for (path, screenshot, _mtime) in candidates {
        let key = path.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let mut payload = vec![("stateFile".to_string(), path.clone())];
        if let Some(shot) = screenshot {
            payload.push(("screenshotFile".to_string(), shot));
        }
        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        jobs.push(UploadJob {
            display_name,
            payload,
        });
    }

    BuiltJobs {
        jobs,
        temp_archives: Vec::new(),
    }
}

/// One job per file in `files` (deduped/existence-filtered via the same
/// rule as [`grouped_file_upload_jobs`]), skipping any PRIMARY file blocked
/// by `ignore` (basename OR extension — mirrors `cloud_transfer.py:654-656`'s
/// pair of `blocked_basenames`/`blocked_extensions` checks). Each surviving
/// file carries a `"stateFile"` entry plus an [`appended_image_sidecar_path`]
/// `"screenshotFile"` when one exists (never ignore-checked itself, per
/// that function's doc comment). Field name is hardcoded `"stateFile"`:
/// this builder's only call site (`cloud_mixin.py:2578`) is inside the
/// `save_type == "state"` branch. Mirrors `retroarch_state_upload_jobs`
/// (`cloud_transfer.py:635-660`); never produces a temp archive.
pub fn retroarch_state_upload_jobs(files: &[PathBuf], ignore: &IgnoreSets) -> BuiltJobs {
    let selected = unique_existing_files(files);
    let mut jobs = Vec::new();
    for file in selected {
        if ignore.blocks(&file) {
            continue;
        }
        let mut payload = vec![("stateFile".to_string(), file.clone())];
        if let Some(shot) = appended_image_sidecar_path(&file) {
            payload.push(("screenshotFile".to_string(), shot));
        }
        let display_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        jobs.push(UploadJob {
            display_name,
            payload,
        });
    }
    BuiltJobs {
        jobs,
        temp_archives: Vec::new(),
    }
}

// --- session-window job filter ----------------------------------------------

/// `window = None` passes `jobs` through untouched. Otherwise keeps a job
/// when ANY of its payload paths has an mtime inside the inclusive
/// `window` (a path that can't be stat'd never qualifies) — no fallback to
/// the unfiltered set when everything is dropped (unlike the file/directory
/// candidate filters in [`super::window`]; this is quirk 11, pinned as-is).
/// `temp_archives` is always returned IN FULL, unfiltered by which jobs
/// survived — every archive built while assembling `jobs` still needs
/// cleaning up regardless of whether its job was kept or dropped, so a
/// dropped job's archive is never silently left behind. Mirrors
/// `filter_upload_jobs_by_session_window` (`cloud_transfer.py:668-687`).
pub fn filter_upload_jobs_by_session_window(jobs: BuiltJobs, window: Option<Window>) -> BuiltJobs {
    let BuiltJobs {
        jobs: job_list,
        temp_archives,
    } = jobs;

    let Some((start, end)) = window else {
        return BuiltJobs {
            jobs: job_list,
            temp_archives,
        };
    };

    let filtered = job_list
        .into_iter()
        .filter(|job| {
            job.payload
                .iter()
                .any(|(_, path)| match file_mtime_secs(path) {
                    Some(mtime) => start <= mtime && mtime <= end,
                    None => false,
                })
        })
        .collect();

    BuiltJobs {
        jobs: filtered,
        temp_archives,
    }
}

// --- short-circuit predicates ------------------------------------------------

/// `last_downloaded_id` is non-empty, equals `current_id`, AND
/// `local_latest_mtime > 0.0`. Mirrors `should_skip_known_latest`
/// (`cloud_transfer.py:705-707`).
pub fn should_skip_known_latest(
    last_downloaded_id: &str,
    current_id: &str,
    local_latest_mtime: f64,
) -> bool {
    !last_downloaded_id.is_empty() && last_downloaded_id == current_id && local_latest_mtime > 0.0
}

/// `local_mtime > 0.0 AND local_mtime > server_timestamp + 1.0` — a whole
/// second of slack against clock-skew/rounding. Mirrors
/// `is_local_newer_than_server` (`cloud_transfer.py:709-710`).
pub fn is_local_newer_than_server(local_mtime: f64, server_timestamp: f64) -> bool {
    local_mtime > 0.0 && local_mtime > server_timestamp + 1.0
}

// --- completion messages -----------------------------------------------------

/// "save files" for [`SaveType::Save`], "save states" for
/// [`SaveType::State`]. Mirrors `uploaded_kind_label`
/// (`cloud_transfer.py:701-702`).
fn uploaded_kind_label(save_type: SaveType) -> &'static str {
    match save_type {
        SaveType::Save => "save files",
        SaveType::State => "save states",
    }
}

/// The result of one upload run: how many jobs succeeded, how many were
/// attempted in total, and the display names of the ones that failed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UploadOutcome {
    pub uploaded: usize,
    pub total: usize,
    pub failed: Vec<String>,
}

/// Whether an [`upload_completion_message`] is informational or a warning
/// (Python's `is_warning: bool` second tuple element, as a proper enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSeverity {
    Info,
    Warning,
}

/// The user-facing completion message for one upload run, and its
/// severity — `None` only when there is nothing to report at all (no
/// attempts, no successes, no failures). Mirrors `upload_completion_message`
/// (`cloud_upload.py:37-60`)'s four-row table, in priority order:
///
/// 1. every attempted job failed (`failed` non-empty, `uploaded == 0`):
///    `"Cloud upload failed for all matching files."` — warning.
/// 2. some failed (`failed` non-empty, `uploaded > 0`): `"Uploaded N <kind>.
///    Failed: <first 5 failed names, comma-joined>"` — warning.
/// 3. all succeeded but retention pruning couldn't remove everything it
///    tried to (`retention_failed > 0`): `"Uploaded N <kind>. Could not
///    remove K older cloud saves for retention limit L."` — warning.
/// 4. otherwise: `"Uploaded N <kind>."` — info.
///
/// `<kind>` is [`uploaded_kind_label`] (singular/plural is baked into that
/// label itself, matching Python — there is no further pluralization
/// logic on the count `N`).
pub fn upload_completion_message(
    outcome: &UploadOutcome,
    save_type: SaveType,
    retention_failed: usize,
    retention_limit: u32,
) -> Option<(String, MessageSeverity)> {
    if outcome.total == 0 && outcome.uploaded == 0 && outcome.failed.is_empty() {
        return None;
    }

    let kind_label = uploaded_kind_label(save_type);

    if !outcome.failed.is_empty() && outcome.uploaded == 0 {
        return Some((
            "Cloud upload failed for all matching files.".to_string(),
            MessageSeverity::Warning,
        ));
    }

    if !outcome.failed.is_empty() {
        let joined = outcome
            .failed
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Some((
            format!(
                "Uploaded {} {kind_label}. Failed: {joined}",
                outcome.uploaded
            ),
            MessageSeverity::Warning,
        ));
    }

    if retention_failed > 0 {
        return Some((
            format!(
                "Uploaded {} {kind_label}. Could not remove {retention_failed} older cloud saves for retention limit {retention_limit}.",
                outcome.uploaded
            ),
            MessageSeverity::Warning,
        ));
    }

    Some((
        format!("Uploaded {} {kind_label}.", outcome.uploaded),
        MessageSeverity::Info,
    ))
}

/// The general "nothing to upload" message for `save_type` — the PPSSPP
/// `.ppst`-specific variant (`"No matching PPSSPP .ppst state files were
/// found to upload."`) lives at its call site, not here, matching the
/// brief. Mirrors the non-PPSSPP branches of `no_matching_upload_message`
/// (`cloud_upload.py:29-34`).
pub fn no_jobs_message(save_type: SaveType) -> String {
    match save_type {
        SaveType::Save => {
            "No matching save files or save folders were found to upload.".to_string()
        }
        SaveType::State => "No matching save states were found to upload.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, UNIX_EPOCH};

    fn touch_at(path: &Path, unix_secs: f64) {
        fs::write(path, b"x").unwrap();
        let modified = UNIX_EPOCH + Duration::from_secs_f64(unix_secs);
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(modified).unwrap();
    }

    fn no_ignore() -> IgnoreSets {
        IgnoreSets::default()
    }

    // --- replaced_suffix_sidecar_path / appended_image_sidecar_path -------

    #[test]
    fn replaced_suffix_sidecar_finds_first_supported_extension() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("game.ppst");
        fs::write(&state, b"state").unwrap();
        let shot = temp.path().join("game.png");
        fs::write(&shot, b"\x89PNG").unwrap();

        assert_eq!(replaced_suffix_sidecar_path(&state), Some(shot));
    }

    #[test]
    fn replaced_suffix_sidecar_none_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("game.ppst");
        fs::write(&state, b"state").unwrap();
        assert_eq!(replaced_suffix_sidecar_path(&state), None);
    }

    /// Ports `test_appended_image_sidecar_path_finds_png_appended_to_full_filename`
    /// (`test_cloud_transfer.py:333`).
    #[test]
    fn appended_image_sidecar_finds_appended_extension_not_replaced_extension() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("game.state1");
        let appended = temp.path().join("game.state1.png");
        fs::write(&state, b"").unwrap();
        fs::write(&appended, b"").unwrap();
        assert_eq!(appended_image_sidecar_path(&state), Some(appended));

        let temp2 = tempfile::tempdir().unwrap();
        let state2 = temp2.path().join("game.state1");
        let replaced = temp2.path().join("game.png"); // replaced-suffix form, NOT appended
        fs::write(&state2, b"").unwrap();
        fs::write(&replaced, b"").unwrap();
        assert_eq!(appended_image_sidecar_path(&state2), None);
    }

    // --- ppsspp_state_upload_jobs -------------------------------------------

    /// Ports `test_ppsspp_state_upload_jobs_uses_supported_image_sidecars_only`
    /// (`test_cloud_transfer.py:315`).
    #[test]
    fn ppsspp_jobs_use_supported_image_sidecars_only() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("states");
        fs::create_dir(&state_dir).unwrap();
        let state_file = state_dir.join("ULUS12345_1.ppst");
        fs::write(&state_file, b"state").unwrap();
        let screenshot_file = state_dir.join("ULUS12345_1.png");
        fs::write(&screenshot_file, b"\x89PNG").unwrap();
        fs::write(state_dir.join("Thumbs.db"), b"not-an-image").unwrap();

        let tokens: BTreeSet<String> = ["ULUS12345".to_string()].into_iter().collect();
        let built = ppsspp_state_upload_jobs(&[state_dir], &tokens, &no_ignore());

        assert_eq!(built.jobs.len(), 1);
        let job = &built.jobs[0];
        assert_eq!(job.display_name, "ULUS12345_1.ppst");
        let payload: HashMap<&str, &PathBuf> =
            job.payload.iter().map(|(k, v)| (k.as_str(), v)).collect();
        assert_eq!(
            payload["stateFile"].file_name().unwrap(),
            "ULUS12345_1.ppst"
        );
        assert_eq!(
            payload["screenshotFile"].file_name().unwrap(),
            screenshot_file.file_name().unwrap()
        );
    }

    #[test]
    fn ppsspp_jobs_empty_tokens_matches_everything() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("states");
        fs::create_dir(&state_dir).unwrap();
        fs::write(state_dir.join("ANYTHING.ppst"), b"state").unwrap();

        let built = ppsspp_state_upload_jobs(&[state_dir], &BTreeSet::new(), &no_ignore());
        assert_eq!(built.jobs.len(), 1);
    }

    #[test]
    fn ppsspp_jobs_non_matching_token_excluded() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("states");
        fs::create_dir(&state_dir).unwrap();
        fs::write(state_dir.join("ULUS99999.ppst"), b"state").unwrap();

        let tokens: BTreeSet<String> = ["ULUS12345".to_string()].into_iter().collect();
        let built = ppsspp_state_upload_jobs(&[state_dir], &tokens, &no_ignore());
        assert!(built.jobs.is_empty());
    }

    #[test]
    fn ppsspp_jobs_non_recursive_glob() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("states");
        fs::create_dir(&state_dir).unwrap();
        let nested = state_dir.join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("deep.ppst"), b"state").unwrap();
        fs::write(state_dir.join("top.ppst"), b"state").unwrap();

        let built = ppsspp_state_upload_jobs(&[state_dir], &BTreeSet::new(), &no_ignore());
        assert_eq!(built.jobs.len(), 1);
        assert_eq!(built.jobs[0].display_name, "top.ppst");
    }

    #[test]
    fn ppsspp_jobs_newest_first_and_deduped() {
        let temp = tempfile::tempdir().unwrap();
        let dir_a = temp.path().join("a");
        let dir_b = temp.path().join("b");
        fs::create_dir(&dir_a).unwrap();
        fs::create_dir(&dir_b).unwrap();
        let older = dir_a.join("OLD.ppst");
        let newer = dir_b.join("NEW.ppst");
        touch_at(&older, 100.0);
        touch_at(&newer, 200.0);

        let built = ppsspp_state_upload_jobs(&[dir_a, dir_b], &BTreeSet::new(), &no_ignore());
        assert_eq!(built.jobs.len(), 2);
        assert_eq!(built.jobs[0].display_name, "NEW.ppst");
        assert_eq!(built.jobs[1].display_name, "OLD.ppst");
    }

    /// Fix-round addition: `ignore` blocks a PRIMARY `.ppst` state file by
    /// extension, mirroring `cloud_transfer.py:608-609`'s
    /// `blocked_extensions` check (Python applies this to the state file
    /// itself, not just the screenshot sidecar).
    #[test]
    fn ppsspp_jobs_exclude_ignored_extension() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("states");
        fs::create_dir(&state_dir).unwrap();
        fs::write(state_dir.join("BLOCKED.ppst"), b"state").unwrap();

        let ignore = IgnoreSets {
            basenames: BTreeSet::new(),
            extensions: [".ppst".to_string()].into_iter().collect(),
        };
        let built = ppsspp_state_upload_jobs(&[state_dir], &BTreeSet::new(), &ignore);
        assert!(
            built.jobs.is_empty(),
            "a state file with an ignored extension must never become a job"
        );
    }

    // --- retroarch_state_upload_jobs ----------------------------------------

    /// Ports `test_retroarch_state_upload_jobs_attaches_appended_png_sidecar`
    /// (`test_cloud_transfer.py:376`).
    #[test]
    fn retroarch_jobs_attach_appended_sidecar() {
        let temp = tempfile::tempdir().unwrap();
        let state_file = temp.path().join("Chrono Trigger.state1");
        let screenshot_file = temp.path().join("Chrono Trigger.state1.png");
        fs::write(&state_file, b"state").unwrap();
        fs::write(&screenshot_file, b"").unwrap();

        let built = retroarch_state_upload_jobs(std::slice::from_ref(&state_file), &no_ignore());
        assert!(built.temp_archives.is_empty());
        assert_eq!(built.jobs.len(), 1);
        assert_eq!(
            built.jobs[0].payload[0],
            ("stateFile".to_string(), state_file)
        );
        assert_eq!(
            built.jobs[0].payload[1],
            ("screenshotFile".to_string(), screenshot_file)
        );
    }

    /// Ports `test_retroarch_state_upload_jobs_omits_screenshotfile_when_no_sidecar`
    /// (`test_cloud_transfer.py:390`).
    #[test]
    fn retroarch_jobs_omit_screenshot_field_when_absent() {
        let temp = tempfile::tempdir().unwrap();
        let state_file = temp.path().join("game.state");
        fs::write(&state_file, b"state").unwrap();

        let built = retroarch_state_upload_jobs(&[state_file], &no_ignore());
        assert_eq!(built.jobs.len(), 1);
        assert_eq!(built.jobs[0].payload.len(), 1);
        assert_eq!(built.jobs[0].payload[0].0, "stateFile");
    }

    /// Ports `test_retroarch_state_upload_jobs_separate_jobs_per_slot`
    /// (`test_cloud_transfer.py:403`).
    #[test]
    fn retroarch_jobs_one_per_slot() {
        let temp = tempfile::tempdir().unwrap();
        let state1 = temp.path().join("game.state1");
        let shot1 = temp.path().join("game.state1.png");
        let state2 = temp.path().join("game.state2");
        let shot2 = temp.path().join("game.state2.png");
        fs::write(&state1, b"s1").unwrap();
        fs::write(&shot1, b"").unwrap();
        fs::write(&state2, b"s2").unwrap();
        fs::write(&shot2, b"").unwrap();

        let built = retroarch_state_upload_jobs(&[state1.clone(), state2.clone()], &no_ignore());
        assert_eq!(built.jobs.len(), 2);
        assert_eq!(built.jobs[0].payload[0].1, state1);
        assert_eq!(built.jobs[0].payload[1].1, shot1);
        assert_eq!(built.jobs[1].payload[0].1, state2);
        assert_eq!(built.jobs[1].payload[1].1, shot2);
    }

    /// Ports `test_retroarch_state_upload_jobs_ignores_non_image_sidecar`
    /// (`test_cloud_transfer.py:423`).
    #[test]
    fn retroarch_jobs_ignore_non_image_sidecar() {
        let temp = tempfile::tempdir().unwrap();
        let state_file = temp.path().join("game.state");
        let non_image = temp.path().join("game.state.txt");
        fs::write(&state_file, b"state").unwrap();
        fs::write(&non_image, b"metadata").unwrap();

        let built = retroarch_state_upload_jobs(&[state_file], &no_ignore());
        assert_eq!(built.jobs.len(), 1);
        assert_eq!(built.jobs[0].payload.len(), 1);
    }

    /// Ports `test_retroarch_state_upload_jobs_screenshot_in_files_payload`
    /// (`test_cloud_transfer.py:436`).
    #[test]
    fn retroarch_jobs_screenshot_in_payload() {
        let temp = tempfile::tempdir().unwrap();
        let state_file = temp.path().join("game.state1");
        let screenshot_file = temp.path().join("game.state1.png");
        fs::write(&state_file, b"state").unwrap();
        fs::write(&screenshot_file, b"").unwrap();

        let built = retroarch_state_upload_jobs(&[state_file], &no_ignore());
        assert_eq!(built.jobs[0].payload[1].1, screenshot_file);
    }

    /// Fix-round addition: `ignore` blocks a PRIMARY state file by
    /// extension, mirroring `cloud_transfer.py:654-656`'s
    /// `blocked_extensions` check.
    #[test]
    fn retroarch_jobs_exclude_ignored_extension() {
        let temp = tempfile::tempdir().unwrap();
        let state_file = temp.path().join("game.state");
        fs::write(&state_file, b"state").unwrap();

        let ignore = IgnoreSets {
            basenames: BTreeSet::new(),
            extensions: [".state".to_string()].into_iter().collect(),
        };
        let built = retroarch_state_upload_jobs(&[state_file], &ignore);
        assert!(
            built.jobs.is_empty(),
            "a state file with an ignored extension must never become a job"
        );
    }

    // --- grouped_file_upload_jobs --------------------------------------------

    /// Ports `test_grouped_file_upload_jobs_archives_multiple_files_into_one_upload`
    /// (`test_cloud_transfer.py:463`).
    #[test]
    fn grouped_jobs_same_stem_archives_into_one_upload() {
        let temp = tempfile::tempdir().unwrap();
        let save_dir = temp.path().join("saves");
        fs::create_dir(&save_dir).unwrap();
        let save_file = save_dir.join("Chrono Trigger.srm");
        let rtc_file = save_dir.join("Chrono Trigger.rtc");
        fs::write(&save_file, b"save").unwrap();
        fs::write(&rtc_file, b"rtc").unwrap();

        let built =
            grouped_file_upload_jobs(&[save_file, rtc_file], "saveFile", "Chrono Trigger").unwrap();

        assert_eq!(built.jobs.len(), 1);
        let job = &built.jobs[0];
        // Group order follows input order (`[save_file, rtc_file]`), so
        // `group[0]` is `save_file`; its stem is the exact display name.
        assert_eq!(job.display_name, "Chrono Trigger");
        let archive_path = &job.payload[0].1;
        assert_eq!(archive_path.extension().unwrap(), "zip");

        let file = fs::File::open(archive_path).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let members: BTreeSet<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert_eq!(
            members,
            ["Chrono Trigger.rtc", "Chrono Trigger.srm"]
                .into_iter()
                .map(String::from)
                .collect::<BTreeSet<_>>()
        );

        archive::cleanup_temp_archives(&built.temp_archives);
    }

    /// Ports `test_grouped_file_upload_jobs_keeps_distinct_state_slots_separate`
    /// (`test_cloud_transfer.py:490`).
    #[test]
    fn grouped_jobs_distinct_state_slots_stay_separate() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("states");
        fs::create_dir(&state_dir).unwrap();
        let slot_file = state_dir.join("Chrono Trigger.state1");
        let auto_file = state_dir.join("Chrono Trigger.state.auto");
        fs::write(&slot_file, b"slot").unwrap();
        fs::write(&auto_file, b"auto").unwrap();

        let built =
            grouped_file_upload_jobs(&[slot_file, auto_file], "stateFile", "Chrono Trigger")
                .unwrap();

        assert_eq!(built.jobs.len(), 2);
        assert!(built.temp_archives.is_empty());
        let mut names: Vec<String> = built
            .jobs
            .iter()
            .map(|j| {
                j.payload[0]
                    .1
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        let mut expected = vec![
            "Chrono Trigger.state1".to_string(),
            "Chrono Trigger.state.auto".to_string(),
        ];
        expected.sort();
        assert_eq!(names, expected);
    }

    #[test]
    fn grouped_jobs_empty_input_returns_empty() {
        let built = grouped_file_upload_jobs(&[], "saveFile", "Game").unwrap();
        assert!(built.jobs.is_empty());
        assert!(built.temp_archives.is_empty());
    }

    // --- directory_archive_upload_jobs / shared_single_upload_job -----------

    #[test]
    fn directory_archive_jobs_named_after_folder() {
        let temp = tempfile::tempdir().unwrap();
        let save_dir = temp.path().join("MySaveDir");
        fs::create_dir(&save_dir).unwrap();
        fs::write(save_dir.join("a.sav"), b"a").unwrap();

        let built = directory_archive_upload_jobs(&[save_dir], &no_ignore()).unwrap();
        assert_eq!(built.jobs.len(), 1);
        assert_eq!(built.jobs[0].display_name, "MySaveDir");
        assert_eq!(built.jobs[0].payload[0].0, "saveFile");
        assert_eq!(built.temp_archives.len(), 1);
        archive::cleanup_temp_archives(&built.temp_archives);
    }

    #[test]
    fn shared_single_job_archives_all_files_together() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a.sav");
        let b = temp.path().join("b.sav");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();

        let built = shared_single_upload_job(&[a, b], "MyEmu Storage", "Game").unwrap();
        assert_eq!(built.jobs.len(), 1);
        assert_eq!(built.jobs[0].display_name, "MyEmu Storage");
        assert_eq!(built.jobs[0].payload[0].0, "saveFile");
        assert_eq!(built.temp_archives.len(), 1);
        archive::cleanup_temp_archives(&built.temp_archives);
    }

    // --- filter_upload_jobs_by_session_window --------------------------------

    /// `window_job_filter_keeps_any_inwindow_path_and_reaps_dropped_temp_archives`
    /// — brief extra.
    #[test]
    fn window_job_filter_keeps_any_inwindow_path_and_reaps_dropped_temp_archives() {
        let temp = tempfile::tempdir().unwrap();
        let in_window_main = temp.path().join("kept_main.dat");
        let in_window_shot = temp.path().join("kept_shot.png");
        touch_at(&in_window_shot, 500.0); // screenshot in window
        touch_at(&in_window_main, 1.0); // main file out of window

        let out_main = temp.path().join("dropped.dat");
        touch_at(&out_main, 1.0);
        let dropped_archive = temp.path().join("dropped_archive.zip");
        fs::write(&dropped_archive, b"zip").unwrap();

        let kept_job = UploadJob {
            display_name: "kept".to_string(),
            payload: vec![
                ("stateFile".to_string(), in_window_main.clone()),
                ("screenshotFile".to_string(), in_window_shot),
            ],
        };
        let dropped_job = UploadJob {
            display_name: "dropped".to_string(),
            payload: vec![("stateFile".to_string(), out_main)],
        };
        let jobs = BuiltJobs {
            jobs: vec![kept_job, dropped_job],
            temp_archives: vec![dropped_archive.clone()],
        };

        let result = filter_upload_jobs_by_session_window(jobs, Some((400.0, 600.0)));
        assert_eq!(result.jobs.len(), 1);
        assert_eq!(result.jobs[0].display_name, "kept");
        // The dropped job's archive is still in temp_archives to be cleaned
        // up — not silently discarded from the returned struct.
        assert_eq!(result.temp_archives, vec![dropped_archive]);
    }

    #[test]
    fn window_job_filter_none_window_passes_through() {
        let temp = tempfile::tempdir().unwrap();
        let f = temp.path().join("a.dat");
        fs::write(&f, b"a").unwrap();
        let jobs = BuiltJobs {
            jobs: vec![UploadJob {
                display_name: "a".to_string(),
                payload: vec![("stateFile".to_string(), f)],
            }],
            temp_archives: Vec::new(),
        };
        let result = filter_upload_jobs_by_session_window(jobs, None);
        assert_eq!(result.jobs.len(), 1);
    }

    // --- session_screenshot_path (TestSessionScreenshotPath, :631) -----------

    #[test]
    fn screenshot_path_returns_none_when_no_directories() {
        assert_eq!(
            session_screenshot_path(&[], Some((0.0, 9_999_999_999.0)), &no_ignore()),
            None
        );
    }

    #[test]
    fn screenshot_path_returns_none_when_window_is_none() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("shot.png");
        fs::write(&img, b"\x89PNG").unwrap();
        assert_eq!(
            session_screenshot_path(&[temp.path().to_path_buf()], None, &no_ignore()),
            None
        );
    }

    #[test]
    fn screenshot_path_returns_none_when_no_images_in_window() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("shot.png");
        touch_at(&img, 1_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((2_000.0, 3_000.0)),
                &no_ignore()
            ),
            None
        );
    }

    #[test]
    fn screenshot_path_returns_image_within_window() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("shot.png");
        touch_at(&img, 5_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            Some(img)
        );
    }

    #[test]
    fn screenshot_path_returns_most_recent_when_multiple_in_window() {
        let temp = tempfile::tempdir().unwrap();
        let earlier = temp.path().join("earlier.png");
        let later = temp.path().join("later.png");
        touch_at(&earlier, 2_000.0);
        touch_at(&later, 4_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            Some(later)
        );
    }

    #[test]
    fn screenshot_path_ignores_non_image_files() {
        let temp = tempfile::tempdir().unwrap();
        let txt = temp.path().join("notes.txt");
        touch_at(&txt, 5_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            None
        );
    }

    #[test]
    fn screenshot_path_scans_subdirectories_recursively() {
        let temp = tempfile::tempdir().unwrap();
        let subdir = temp.path().join("GameID");
        fs::create_dir(&subdir).unwrap();
        let img = subdir.join("shot.png");
        touch_at(&img, 5_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            Some(img)
        );
    }

    #[test]
    fn screenshot_path_skips_blocked_basenames() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("blocked.png");
        touch_at(&img, 5_000.0);
        let ignore = IgnoreSets {
            basenames: ["blocked.png".to_string()].into_iter().collect(),
            extensions: BTreeSet::new(),
        };
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &ignore
            ),
            None
        );
    }

    #[test]
    fn screenshot_path_skips_missing_directories_gracefully() {
        let missing = PathBuf::from("/nonexistent/screenshot/dir/that/does/not/exist");
        assert_eq!(
            session_screenshot_path(&[missing], Some((1_000.0, 9_000.0)), &no_ignore()),
            None
        );
    }

    /// Fix-round addition: Python (`cloud_transfer.py:109-113`) wraps
    /// `list(directory.rglob("*"))` in ONE `try/except OSError` per
    /// TOP-LEVEL directory — a read failure anywhere in that directory's
    /// recursive walk (root or nested) discards that entire top-level
    /// directory's results, not just the failing subtree. `dir_a` here has
    /// an in-window image directly under it AND an unreadable nested
    /// subdirectory; the whole of `dir_a` must be dropped, so the result
    /// comes only from the untouched `dir_b`.
    #[test]
    #[cfg(unix)]
    fn screenshot_path_top_level_dir_discarded_on_nested_read_failure() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let dir_a = temp.path().join("dir_a");
        fs::create_dir(&dir_a).unwrap();
        let doomed_image = dir_a.join("doomed.png");
        touch_at(&doomed_image, 5_000.0);
        let locked_sub = dir_a.join("locked");
        fs::create_dir(&locked_sub).unwrap();
        fs::set_permissions(&locked_sub, fs::Permissions::from_mode(0o000)).unwrap();

        let dir_b = temp.path().join("dir_b");
        fs::create_dir(&dir_b).unwrap();
        let winner = dir_b.join("winner.png");
        touch_at(&winner, 6_000.0);

        let result = session_screenshot_path(
            &[dir_a.clone(), dir_b.clone()],
            Some((1_000.0, 9_000.0)),
            &no_ignore(),
        );

        // Restore permissions before any panicking assertion so the temp
        // dir can always be cleaned up.
        fs::set_permissions(&locked_sub, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(
            result,
            Some(winner),
            "dir_a's entire result set (including the in-window doomed.png) \
             must be discarded because of the unreadable nested directory; \
             only dir_b's clean image can win"
        );
    }

    #[test]
    fn screenshot_path_supports_jpg_extension() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("shot.jpg");
        touch_at(&img, 5_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            Some(img)
        );
    }

    #[test]
    fn screenshot_path_supports_webp_extension() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("shot.webp");
        touch_at(&img, 5_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            Some(img)
        );
    }

    #[test]
    fn screenshot_path_supports_bmp_extension() {
        let temp = tempfile::tempdir().unwrap();
        let img = temp.path().join("shot.bmp");
        touch_at(&img, 5_000.0);
        assert_eq!(
            session_screenshot_path(
                &[temp.path().to_path_buf()],
                Some((1_000.0, 9_000.0)),
                &no_ignore()
            ),
            Some(img)
        );
    }

    // --- screenshot / state download candidate paths -------------------------

    /// Ports `test_screenshot_download_candidate_paths_returns_ordered_candidates`
    /// (`test_cloud_transfer.py:354`).
    #[test]
    fn candidate_paths_ordered_by_key_precedence() {
        let record = serde_json::json!({
            "download_path": "a/b.png",
            "file_path": "c/d.png",
            "full_path": "e/f.png",
        });
        assert_eq!(
            screenshot_download_candidate_paths(&record),
            vec!["a/b.png", "c/d.png", "e/f.png"]
        );
        assert_eq!(
            state_content_candidate_paths(&record),
            vec!["a/b.png", "c/d.png", "e/f.png"]
        );
    }

    /// Ports `test_screenshot_download_candidate_paths_skips_blank_and_missing_keys`
    /// (`test_cloud_transfer.py:363`).
    #[test]
    fn candidate_paths_skip_blank_and_missing_keys() {
        let record = serde_json::json!({
            "download_path": "",
            "full_path": "x/y.png",
        });
        assert_eq!(
            screenshot_download_candidate_paths(&record),
            vec!["x/y.png"]
        );
    }

    /// Ports `test_screenshot_download_candidate_paths_returns_empty_for_empty_record`
    /// (`test_cloud_transfer.py:371`).
    #[test]
    fn candidate_paths_empty_record_returns_empty() {
        let record = serde_json::json!({});
        assert!(screenshot_download_candidate_paths(&record).is_empty());
    }

    // --- normalize_candidate_url ---------------------------------------------

    /// `normalize_candidate_url_percent_encodes_path_and_query` — brief
    /// extra. Expected outputs verified against the real Python
    /// `normalize_candidate_url` (`cloud_transfer.py:133-139`) run
    /// standalone against these exact inputs.
    #[test]
    fn normalize_candidate_url_percent_encodes_path_and_query() {
        assert_eq!(
            normalize_candidate_url(
                "https://cloud.example.com/api/saves/My Game (2001)?title=Chrono Trigger&path=a/b c&blank="
            ),
            "https://cloud.example.com/api/saves/My%20Game%20%282001%29?title=Chrono%20Trigger&path=a%2Fb%20c&blank="
        );
        assert_eq!(
            normalize_candidate_url("/api/saves/relative path?x=1"),
            "/api/saves/relative%20path?x=1"
        );
        assert_eq!(
            normalize_candidate_url("https://host/already%20encoded/path%2Fslash?raw=%2F&plus=a+b"),
            "https://host/already%20encoded/path%2Fslash?raw=%2F&plus=a%20b"
        );
    }

    // --- short-circuit predicates ----------------------------------------------

    /// `known_latest_requires_matching_id_and_positive_local_mtime` — brief
    /// extra, all three clauses toggled.
    #[test]
    fn known_latest_requires_matching_id_and_positive_local_mtime() {
        // All three true: skip.
        assert!(should_skip_known_latest("id-1", "id-1", 5.0));
        // Empty last_downloaded_id: don't skip.
        assert!(!should_skip_known_latest("", "id-1", 5.0));
        // Mismatched id: don't skip.
        assert!(!should_skip_known_latest("id-1", "id-2", 5.0));
        // Non-positive local mtime: don't skip.
        assert!(!should_skip_known_latest("id-1", "id-1", 0.0));
        assert!(!should_skip_known_latest("id-1", "id-1", -1.0));
    }

    /// `local_newer_needs_more_than_one_second` — brief extra.
    #[test]
    fn local_newer_needs_more_than_one_second() {
        assert!(!is_local_newer_than_server(0.0, 0.0), "local must be > 0");
        assert!(
            !is_local_newer_than_server(101.0, 100.0),
            "exactly 1s ahead is not \"newer\" (needs > server + 1.0)"
        );
        assert!(
            is_local_newer_than_server(101.001, 100.0),
            "just over 1s ahead is newer"
        );
        assert!(
            !is_local_newer_than_server(-5.0, -100.0),
            "local must be > 0"
        );
    }

    // --- completion messages -----------------------------------------------

    /// `completion_message_table` — brief extra, all four rows byte-exact.
    #[test]
    fn completion_message_table() {
        // Row 1: every attempted job failed.
        let outcome = UploadOutcome {
            uploaded: 0,
            total: 2,
            failed: vec!["a.sav".to_string(), "b.sav".to_string()],
        };
        assert_eq!(
            upload_completion_message(&outcome, SaveType::Save, 0, 5),
            Some((
                "Cloud upload failed for all matching files.".to_string(),
                MessageSeverity::Warning
            ))
        );

        // Row 2: some failed, first 5 names joined.
        let outcome = UploadOutcome {
            uploaded: 3,
            total: 9,
            failed: vec![
                "a.sav".to_string(),
                "b.sav".to_string(),
                "c.sav".to_string(),
                "d.sav".to_string(),
                "e.sav".to_string(),
                "f.sav".to_string(),
            ],
        };
        assert_eq!(
            upload_completion_message(&outcome, SaveType::Save, 0, 5),
            Some((
                "Uploaded 3 save files. Failed: a.sav, b.sav, c.sav, d.sav, e.sav".to_string(),
                MessageSeverity::Warning
            ))
        );

        // Row 3: all succeeded, retention pruning partially failed.
        let outcome = UploadOutcome {
            uploaded: 4,
            total: 4,
            failed: Vec::new(),
        };
        assert_eq!(
            upload_completion_message(&outcome, SaveType::State, 2, 5),
            Some((
                "Uploaded 4 save states. Could not remove 2 older cloud saves for retention limit 5."
                    .to_string(),
                MessageSeverity::Warning
            ))
        );

        // Row 4: clean success.
        let outcome = UploadOutcome {
            uploaded: 4,
            total: 4,
            failed: Vec::new(),
        };
        assert_eq!(
            upload_completion_message(&outcome, SaveType::State, 0, 5),
            Some(("Uploaded 4 save states.".to_string(), MessageSeverity::Info))
        );
    }

    #[test]
    fn no_jobs_message_matches_python_general_branch() {
        assert_eq!(
            no_jobs_message(SaveType::Save),
            "No matching save files or save folders were found to upload."
        );
        assert_eq!(
            no_jobs_message(SaveType::State),
            "No matching save states were found to upload."
        );
    }
}
