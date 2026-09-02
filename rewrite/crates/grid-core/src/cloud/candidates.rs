//! Ignore-set resolution and cloud-save candidate scanners.
//!
//! Ported from `grid_launcher/library/cloud_sync.py` (folder scanners
//! `cloud_sync_directory_candidates_for_game` :439-481,
//! `cemu_save_directories_for_game` :492-566, and the file scanner
//! `cloud_sync_candidates_for_game` / `_fallback_state_candidates`
//! :401-436,574-651), `grid_launcher/ui/mixins/cloud_mixin.py`
//! (`_pcsx2_save_directories_for_game` :1147-1170,
//! `_rpcs3_save_directories_for_game` :1177-1200,
//! `_ppsspp_save_directories_for_game` :1427-1447, and
//! `_sync_directory_ignore_basenames_for_emulator` at `grid-launcher.py:3712`
//! for the PCSX2 superblock addition), and
//! `grid_launcher/emulator/profiles.py` (`resolved_save_strategy_for_emulator`
//! :338-356, `resolved_ignore_basenames_for_emulator` :361-378,
//! `resolved_ignore_extensions_for_emulator` :385-406,
//! `normalize_ignore_extension_value` :158-175, `split_configured_paths`
//! :133-138).
//!
//! **D9 (token secrecy, binding, deliberate deviation from Python):** the
//! default ignore-basename set carries four extra credential/config
//! basenames — `retroarch.cfg`, `pcsx2.ini`, `ppsspp.ini`,
//! `ppsspp_retroachievements.dat` — that Python's
//! `DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES` (`cloud_transfer.py:19-24`) does
//! not have. Token secrecy outranks parity here: a save path pointed at an
//! emulator's own config root must never upload a file that can hold a
//! RetroAchievements/session token.
//!
//! **Two Python quirks ported as-is (NOT fixed):** the RPCS3 scanner sorts
//! by configured-directory index BEFORE recency (`rpcs3_save_directories`),
//! and the PPSSPP scanner sorts by each candidate directory's OWN mtime,
//! applies no ignore set, and requires no contained file at all
//! (`ppsspp_save_directories`). See this module's function docs.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::autoconfig::entry::normalize_save_strategy;
use crate::config::EmulatorEntry;
use crate::launch::profiles::EmulatorProfile;

use super::tokens::{
    cemu_title_id_tokens, compact_alnum, is_state_file_candidate, state_candidate_hash_group_key,
    state_candidate_matches_tokens,
};
use super::{file_mtime_secs, latest_mtime_under, IgnoreSets, SaveType};

/// `DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES` (`cloud_transfer.py:19-24`) plus
/// D9's four credential/config basenames (see module doc comment).
/// Lowercased, matching [`IgnoreSets`]'s expected member casing.
const DEFAULT_IGNORE_BASENAMES: &[&str] = &[
    ".ds_store",
    "desktop.ini",
    "ehthumbs.db",
    "thumbs.db",
    // D9 additions — never present in the Python constant:
    "retroarch.cfg",
    "pcsx2.ini",
    "ppsspp.ini",
    "ppsspp_retroachievements.dat",
];

// -- small local string helpers (duplicated rather than made pub in
// cloud::tokens, matching that module's own precedent of duplicating
// `split_suffix` from `launch/emu_install.rs` rather than adding
// crate-internal visibility for a two-line helper) ------------------------

/// `pathlib.Path(name).stem` / `.suffix` for a bare file name: mirrors
/// `cloud::tokens`'s private `split_suffix`.
fn split_name(name: &str) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    let n = chars.len();
    let split_at = match chars.iter().rposition(|&c| c == '.') {
        Some(i) if i > 0 && i < n - 1 => i,
        _ => n,
    };
    (
        chars[..split_at].iter().collect(),
        chars[split_at..].iter().collect(),
    )
}

fn stem_of(name: &str) -> String {
    split_name(name).0
}

fn extension_of(name: &str) -> String {
    split_name(name).1
}

/// The final path segment of `value`, treating both `/` and `\` as
/// separators — mirrors `cloud::tokens`'s private `final_path_component`
/// (same cross-platform convention for a string-typed path field).
fn final_path_segment(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    normalized.rsplit('/').next().unwrap_or("").to_string()
}

/// Uppercased, `[A-Z0-9]`-only form of `s`. The upper-case counterpart of
/// `cloud::tokens::compact_alnum`, used for the Nintendo-console-style
/// directory-name normalization the PCSX2/RPCS3/PPSSPP/Cemu scanners share
/// (`re.sub(r"[^A-Z0-9]+", "", value.upper())` at each of their call sites).
fn upper_alnum(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// `profiles.py:133`'s `split_configured_paths`: split on runs of `;`,
/// `\r`, `\n`; trim each piece; drop blanks. This is the exact splitting
/// `autoconfig::entry`'s `multiline_profile_value` writes into an
/// `EmulatorEntry`'s `ignore_files`/`ignore_extensions` string fields (join
/// with `";\n"`), so it is this function's inverse.
fn split_entry_list(value: &str) -> Vec<String> {
    value
        .split([';', '\r', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Non-blank, trimmed profile list items. `profiles.py:361-378,385-406`'s
/// `[item.strip() for item in raw_profile_values if isinstance(item, str)
/// and item.strip()]`.
fn nonblank_trimmed(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// `profiles.py:158-175`'s `normalize_ignore_extension_value`: a bare
/// extension or `*.ext` glob becomes `.ext` (lowercased); a path-like value
/// (containing `/` or `\`) is reduced to its own final extension first;
/// anything that doesn't end up matching `^\.[a-z0-9]+$`, or that is
/// `.jpg`/`.jpeg`, normalizes to `""`.
fn normalize_ignore_extension_value(value: &str) -> String {
    let mut normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return String::new();
    }
    if normalized.contains('/') || normalized.contains('\\') {
        normalized = extension_of(&final_path_segment(&normalized));
    }
    if let Some(stripped) = normalized.strip_prefix("*.") {
        normalized = format!(".{stripped}");
    }
    if !normalized.starts_with('.') {
        normalized = format!(".{}", normalized.trim_start_matches('*'));
    }
    let body = normalized.strip_prefix('.').unwrap_or("");
    let valid = !body.is_empty()
        && body
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    if !valid {
        return String::new();
    }
    if normalized == ".jpg" || normalized == ".jpeg" {
        return String::new();
    }
    normalized
}

/// `profiles.py:361-378`'s per-value basename rule: `Path(value).name`,
/// trimmed and lowercased; dropped when blank or when its own extension is
/// `.jpg`/`.jpeg`.
fn ignore_basenames_from_values(values: &[String]) -> BTreeSet<String> {
    let mut basenames = BTreeSet::new();
    for value in values {
        let basename = final_path_segment(value).trim().to_lowercase();
        if basename.is_empty() {
            continue;
        }
        let extension = extension_of(&basename);
        if extension == ".jpg" || extension == ".jpeg" {
            continue;
        }
        basenames.insert(basename);
    }
    basenames
}

/// `profiles.py:385-406`'s per-value extension rule.
fn ignore_extensions_from_values(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|v| normalize_ignore_extension_value(v))
        .filter(|v| !v.is_empty())
        .collect()
}

/// Case-insensitive dedupe by the full path string, first occurrence wins.
/// `cloud_sync.py:346-355`'s `_unique_casefold_paths`, duplicated verbatim
/// at every scanner call site in `cloud_mixin.py` (PCSX2, RPCS3, PPSSPP)
/// and here in `cloud_sync.py` itself.
fn dedupe_casefold(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            unique.push(path);
        }
    }
    unique
}

/// All files at or under `root`, recursively — the `rglob("*")` stand-in
/// the brief calls for. Same symlink handling as
/// [`super::latest_mtime_under`]: a subdirectory is descended into only
/// when `DirEntry::file_type` (which does not follow symlinks) itself
/// reports `is_dir()`, so a symlinked directory is never recursed into and
/// never double-walked; every other entry (a real file, or ANY symlink) is
/// returned as a raw candidate for the caller's own `is_file()` check —
/// mirroring `Path.rglob("*")`, which likewise yields symlinks-to-files as
/// themselves without descending into symlinks-to-directories.
fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                stack.push(path);
                continue;
            }
            files.push(path);
        }
    }
    files
}

// -- resolved_ignore_sets / resolved_save_strategy -------------------------

/// The effective ignore sets for a scan: `DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES`
/// plus D9's four credential basenames, plus `entry`'s configured
/// `ignore_files`/`ignore_extensions` (comma/semicolon/newline-split exactly
/// like the other M5 entry list fields — see [`split_entry_list`]) when
/// non-empty, ELSE `profile`'s `ignore_files`/`ignore_extensions` lists
/// (this is a fallback, not a union, between entry and profile — matching
/// `profiles.py:369,393`'s `all_values = configured_values if
/// configured_values else profile_values`), plus `_pcsx2_superblock` added
/// to the basenames when `is_pcsx2 && save_type == SaveType::Save`
/// (`grid-launcher.py:3712`'s `_sync_directory_ignore_basenames_for_emulator`
/// — an existing Python behavior, not part of D9). `profiles.py:361,385`.
pub fn resolved_ignore_sets(
    entry: Option<&EmulatorEntry>,
    profile: Option<&EmulatorProfile>,
    save_type: SaveType,
    is_pcsx2: bool,
) -> IgnoreSets {
    let mut basenames: BTreeSet<String> = DEFAULT_IGNORE_BASENAMES
        .iter()
        .map(|s| s.to_string())
        .collect();

    let entry_basename_values = entry
        .map(|e| split_entry_list(&e.ignore_files))
        .unwrap_or_default();
    let profile_basename_values = profile
        .map(|p| nonblank_trimmed(&p.ignore_files))
        .unwrap_or_default();
    let chosen_basename_values = if !entry_basename_values.is_empty() {
        entry_basename_values
    } else {
        profile_basename_values
    };
    basenames.extend(ignore_basenames_from_values(&chosen_basename_values));

    let entry_extension_values = entry
        .map(|e| split_entry_list(&e.ignore_extensions))
        .unwrap_or_default();
    let profile_extension_values = profile
        .map(|p| nonblank_trimmed(&p.ignore_extensions))
        .unwrap_or_default();
    let chosen_extension_values = if !entry_extension_values.is_empty() {
        entry_extension_values
    } else {
        profile_extension_values
    };
    let extensions = ignore_extensions_from_values(&chosen_extension_values);

    if is_pcsx2 && save_type == SaveType::Save {
        basenames.insert("_pcsx2_superblock".to_string());
    }

    IgnoreSets {
        basenames,
        extensions,
    }
}

/// The effective save strategy string (`"auto"` / `"single_file"` /
/// `"folder"`): `entry`'s configured `save_strategy` when it normalizes to
/// something other than `"auto"`, else `profile`'s when that normalizes to
/// something other than `"auto"`, else `"single_file"` for
/// [`SaveType::State`] or `"auto"` for [`SaveType::Save`].
/// `profiles.py:338-356`'s `resolved_save_strategy_for_emulator`.
pub fn resolved_save_strategy(
    entry: Option<&EmulatorEntry>,
    profile: Option<&EmulatorProfile>,
    save_type: SaveType,
) -> String {
    if let Some(entry) = entry {
        let normalized = normalize_save_strategy(&entry.save_strategy);
        if normalized != "auto" {
            return normalized;
        }
    }
    if let Some(profile) = profile {
        let normalized = normalize_save_strategy(&profile.save_strategy);
        if normalized != "auto" {
            return normalized;
        }
    }
    match save_type {
        SaveType::State => "single_file".to_string(),
        SaveType::Save => "auto".to_string(),
    }
}

// -- file_candidates / directory_candidates ---------------------------------

/// The grouping fallback when no state candidate matched by name/token: an
/// empty or single-element input returns as-is; otherwise the newest
/// candidate (by mtime desc, lowercased-name tiebreak) picks a hash-group
/// key via [`state_candidate_hash_group_key`] — an empty key (neither hash
/// nor `_<n>`/`_resume` shape) yields no candidates at all — and every
/// candidate sharing that key is returned, newest-first, deduped.
/// `cloud_sync.py:412-436`'s `_fallback_state_candidates`.
fn fallback_state_candidates(candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    if candidates.is_empty() {
        return Vec::new();
    }
    if candidates.len() == 1 {
        return candidates;
    }

    fn sort_key(path: &Path) -> (f64, String) {
        let mtime = file_mtime_secs(path).unwrap_or(0.0);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        (mtime, name)
    }

    let mut by_recency = candidates.clone();
    by_recency.sort_by(|a, b| {
        let (ma, na) = sort_key(a);
        let (mb, nb) = sort_key(b);
        mb.total_cmp(&ma).then_with(|| na.cmp(&nb))
    });

    let latest_name = by_recency[0]
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let group_key = state_candidate_hash_group_key(latest_name);
    if group_key.is_empty() {
        return Vec::new();
    }

    let mut grouped: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| {
            let name = c.file_name().and_then(|n| n.to_str()).unwrap_or("");
            state_candidate_hash_group_key(name) == group_key
        })
        .collect();
    grouped.sort_by(|a, b| {
        let (ma, na) = sort_key(a);
        let (mb, nb) = sort_key(b);
        mb.total_cmp(&ma).then_with(|| na.cmp(&nb))
    });
    dedupe_casefold(grouped)
}

/// Save/state file candidates under `dirs`. For each directory: an
/// "explicit file root" — either the path is itself a file on disk
/// (matching Python's `directory.is_file()` check exactly) OR it appears in
/// `explicit_file_roots` (a rewrite-only addition: callers that already
/// know a configured path names a single file, rather than relying solely
/// on a filesystem probe, can flag it directly) — is checked as that one
/// file; anything else is walked recursively via [`walk_files`]. Every
/// candidate is filtered through `ignore` first.
///
/// [`SaveType::State`]: must pass [`is_state_file_candidate`]; an explicit
/// root or a token match ([`state_candidate_matches_tokens`]) goes to the
/// "matched" bucket, everything else to "unmatched". Matched wins when
/// non-empty; otherwise [`fallback_state_candidates`] runs on the unmatched
/// set.
///
/// [`SaveType::Save`]: kept when `tokens` is empty, OR the candidate is an
/// explicit root, OR any token is a substring of the lowercased file name
/// or of the `[a-z0-9]`-compacted file stem.
///
/// Final order (both kinds): mtime desc, then lowercased name; deduped
/// case-insensitively. `cloud_sync.py:574-651`.
///
/// The Python source also short-circuits to `[]` for any `save_type` other
/// than `"save"`/`"state"` — [`SaveType`] has no other variant, so that
/// check is enforced by the type system here instead of at runtime.
pub fn file_candidates(
    dirs: &[PathBuf],
    tokens: &BTreeSet<String>,
    save_type: SaveType,
    ignore: &IgnoreSets,
    explicit_file_roots: &[PathBuf],
) -> Vec<PathBuf> {
    let explicit_set: BTreeSet<&Path> = explicit_file_roots.iter().map(PathBuf::as_path).collect();

    let mut save_candidates: Vec<PathBuf> = Vec::new();
    let mut matched_state: Vec<PathBuf> = Vec::new();
    let mut unmatched_state: Vec<PathBuf> = Vec::new();

    for directory in dirs {
        if !directory.exists() {
            continue;
        }
        let explicit_file_root = directory.is_file() || explicit_set.contains(directory.as_path());
        let scan_targets: Vec<PathBuf> = if explicit_file_root {
            vec![directory.clone()]
        } else {
            walk_files(directory)
        };

        for candidate in scan_targets {
            if !candidate.is_file() {
                continue;
            }
            if ignore.blocks(&candidate) {
                continue;
            }

            let raw_name = candidate
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            match save_type {
                SaveType::State => {
                    if !is_state_file_candidate(&candidate) {
                        continue;
                    }
                    if explicit_file_root || state_candidate_matches_tokens(&raw_name, tokens) {
                        matched_state.push(candidate);
                    } else {
                        unmatched_state.push(candidate);
                    }
                }
                SaveType::Save => {
                    let name_lower = raw_name.to_lowercase();
                    let stem_compact = compact_alnum(&stem_of(&raw_name));
                    let keep = explicit_file_root
                        || tokens.is_empty()
                        || tokens.iter().any(|t| {
                            !t.is_empty()
                                && (name_lower.contains(t.as_str())
                                    || stem_compact.contains(t.as_str()))
                        });
                    if keep {
                        save_candidates.push(candidate);
                    }
                }
            }
        }
    }

    let mut candidates = match save_type {
        SaveType::State => {
            if !matched_state.is_empty() {
                matched_state
            } else {
                fallback_state_candidates(unmatched_state)
            }
        }
        SaveType::Save => save_candidates,
    };

    candidates.sort_by(|a, b| {
        let ma = file_mtime_secs(a).unwrap_or(0.0);
        let mb = file_mtime_secs(b).unwrap_or(0.0);
        let na = a
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        let nb = b
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        mb.total_cmp(&ma).then_with(|| na.cmp(&nb))
    });
    dedupe_casefold(candidates)
}

/// Immediate subdirectories of `dirs` that contain at least one non-blocked
/// file anywhere beneath them, matched against `tokens` by the
/// `[a-z0-9]`-compacted child name OR the compacted path relative to its
/// parent directory (either match qualifies; an empty `tokens` accepts
/// everything). Sorted by [`latest_mtime_under`] (with `ignore`) descending,
/// deduped case-insensitively. `cloud_sync.py:439-481`'s
/// `cloud_sync_directory_candidates_for_game`.
pub fn directory_candidates(
    dirs: &[PathBuf],
    tokens: &BTreeSet<String>,
    ignore: &IgnoreSets,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    for directory in dirs {
        if !directory.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            let has_non_blocked_file = walk_files(&child)
                .iter()
                .any(|f| f.is_file() && !ignore.blocks(f));
            if !has_non_blocked_file {
                continue;
            }

            let child_name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let normalized_name = compact_alnum(child_name);
            let relative = child.strip_prefix(directory).unwrap_or(child.as_path());
            let normalized_relative = compact_alnum(&relative.to_string_lossy());

            if !tokens.is_empty()
                && !tokens.iter().any(|t| {
                    normalized_name.contains(t.as_str()) || normalized_relative.contains(t.as_str())
                })
            {
                continue;
            }

            candidates.push(child);
        }
    }

    candidates
        .sort_by(|a, b| latest_mtime_under(b, ignore).total_cmp(&latest_mtime_under(a, ignore)));
    dedupe_casefold(candidates)
}

// -- per-emulator folder scanners -------------------------------------------

/// Cemu save directories: walks `<dir>/<title-high>/<title-low>/user/`,
/// yielding `user`'s child directories, or `user` itself when it has none.
/// `tokens` (the game's RAW match-token set) is first run through
/// [`cemu_title_id_tokens`]'s normalize-and-prefer-16-then-8-else-all ladder
/// to get `match_tokens`, which is then compared against each
/// `<high>`/`<low>` pair's `[A-Z0-9]`-only normalized names (exact match on
/// either, or a substring match against their concatenation) — an empty
/// `match_tokens` accepts every title-id pair.
///
/// A candidate directory whose [`latest_mtime_under`] (with `ignore`) is
/// `<= 0.0` is dropped outright. The title-id-matched list wins when
/// non-empty; otherwise every surviving candidate (regardless of title-id
/// match) is returned. Sorted by `latest_mtime_under` descending, deduped
/// case-insensitively. `cloud_sync.py:492-566`'s
/// `cemu_save_directories_for_game`.
pub fn cemu_save_directories(
    dirs: &[PathBuf],
    tokens: &BTreeSet<String>,
    ignore: &IgnoreSets,
) -> Vec<PathBuf> {
    let match_tokens: BTreeSet<String> = cemu_title_id_tokens(tokens).into_iter().collect();

    let mut matched: Vec<PathBuf> = Vec::new();
    let mut fallback: Vec<PathBuf> = Vec::new();

    for directory in dirs {
        if !directory.is_dir() {
            continue;
        }
        let Ok(high_entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for high_entry in high_entries.flatten() {
            let title_high = high_entry.path();
            if !title_high.is_dir() {
                continue;
            }
            let high_token = upper_alnum(
                title_high
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(""),
            );

            let Ok(low_entries) = std::fs::read_dir(&title_high) else {
                continue;
            };
            for low_entry in low_entries.flatten() {
                let title_low = low_entry.path();
                if !title_low.is_dir() {
                    continue;
                }
                let low_token =
                    upper_alnum(title_low.file_name().and_then(|n| n.to_str()).unwrap_or(""));
                let combined_token = format!("{high_token}{low_token}");
                let matches_title_id = match_tokens.is_empty()
                    || match_tokens.iter().any(|t| {
                        !t.is_empty()
                            && (t == &high_token
                                || t == &low_token
                                || combined_token.contains(t.as_str()))
                    });

                let user_root = title_low.join("user");
                if !user_root.is_dir() {
                    continue;
                }

                let Ok(user_entries) = std::fs::read_dir(&user_root) else {
                    continue;
                };
                let child_dirs: Vec<PathBuf> = user_entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                let candidate_dirs: Vec<PathBuf> = if child_dirs.is_empty() {
                    vec![user_root.clone()]
                } else {
                    child_dirs
                };

                for candidate in candidate_dirs {
                    let latest = latest_mtime_under(&candidate, ignore);
                    if latest <= 0.0 {
                        continue;
                    }
                    fallback.push(candidate.clone());
                    if matches_title_id {
                        matched.push(candidate);
                    }
                }
            }
        }
    }

    let mut candidates = if !matched.is_empty() {
        matched
    } else {
        fallback
    };
    candidates
        .sort_by(|a, b| latest_mtime_under(b, ignore).total_cmp(&latest_mtime_under(a, ignore)));
    dedupe_casefold(candidates)
}

/// PCSX2 save directories: immediate children of `dirs` containing at least
/// one file (ANY file — unlike [`directory_candidates`], this existence
/// check does not consult `ignore`, matching Python's
/// `_pcsx2_save_directories_for_game`, which never filters its `rglob("*")`
/// probe), matched against `serials` (already-normalized `[A-Z0-9]`-only
/// PS2 serial tokens) by substring on the child's `[A-Z0-9]`-normalized
/// name OR its normalized path relative to its parent — an empty `serials`
/// accepts every child. Sorted by [`latest_mtime_under`] (WITH `ignore`,
/// a deliberate deviation from Python's unfiltered
/// `_latest_file_mtime_under_path(item)` sort call — see [`resolved_ignore_sets`]'s
/// `_pcsx2_superblock` addition, whose whole purpose is to keep that file's
/// own churn from winning "most recent") descending, deduped
/// case-insensitively. `cloud_mixin.py:1147-1170`.
pub fn pcsx2_save_directories(
    dirs: &[PathBuf],
    serials: &BTreeSet<String>,
    ignore: &IgnoreSets,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    for directory in dirs {
        if !directory.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            if !walk_files(&child).iter().any(|f| f.is_file()) {
                continue;
            }

            let child_name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let normalized_name = upper_alnum(child_name);
            let relative = child.strip_prefix(directory).unwrap_or(child.as_path());
            let normalized_relative = upper_alnum(&relative.to_string_lossy());

            if !serials.is_empty()
                && !serials.iter().any(|s| {
                    normalized_name.contains(s.as_str()) || normalized_relative.contains(s.as_str())
                })
            {
                continue;
            }

            candidates.push(child);
        }
    }

    candidates
        .sort_by(|a, b| latest_mtime_under(b, ignore).total_cmp(&latest_mtime_under(a, ignore)));
    dedupe_casefold(candidates)
}

/// RPCS3 save directories. QUIRK, ported as-is: immediate children of
/// `dirs` matched against `ids` (already-normalized `[A-Z0-9]`-only PS3
/// game-id tokens, substring on the child's normalized name; an empty
/// `ids` accepts every child) — NO contained-file requirement at all, and
/// NO ignore filtering. Sorted by `(configured-directory index ascending,
/// latest_mtime_under with no ignore, descending)` — the directory a
/// candidate was found under always outranks how recently it was touched,
/// so a stale directory listed first in `dirs` beats a fresh one listed
/// second. Deduped case-insensitively. `cloud_mixin.py:1177-1200`'s
/// `_rpcs3_save_directories_for_game`.
pub fn rpcs3_save_directories(dirs: &[PathBuf], ids: &[String]) -> Vec<PathBuf> {
    let no_ignore = IgnoreSets::default();
    let mut scored: Vec<(usize, f64, PathBuf)> = Vec::new();

    for (index, directory) in dirs.iter().enumerate() {
        if !directory.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            let child_name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let normalized_name = upper_alnum(child_name);
            if !ids.is_empty() && !ids.iter().any(|id| normalized_name.contains(id.as_str())) {
                continue;
            }
            let mtime = latest_mtime_under(&child, &no_ignore);
            scored.push((index, mtime, child));
        }
    }

    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.total_cmp(&a.1)));
    dedupe_casefold(scored.into_iter().map(|(_, _, path)| path).collect())
}

/// PPSSPP save directories. QUIRK, ported as-is: immediate children of
/// `dirs` matched against `ids` (already-normalized `[A-Z0-9]`-only PSP id
/// tokens, substring on the child's normalized name; an empty `ids`
/// accepts every child) — NO contained-file requirement, and NO ignore
/// filtering. Sorted by each candidate directory's OWN mtime (NOT the
/// latest file anywhere beneath it — a plain `stat()` on the directory
/// itself) descending, deduped case-insensitively.
/// `cloud_mixin.py:1427-1447`'s `_ppsspp_save_directories_for_game`.
pub fn ppsspp_save_directories(dirs: &[PathBuf], ids: &BTreeSet<String>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    for directory in dirs {
        if !directory.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if !child.is_dir() {
                continue;
            }
            let child_name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let normalized_name = upper_alnum(child_name);
            if !ids.is_empty() && !ids.iter().any(|id| normalized_name.contains(id.as_str())) {
                continue;
            }
            candidates.push(child);
        }
    }

    candidates.sort_by(|a, b| {
        let ma = file_mtime_secs(a).unwrap_or(0.0);
        let mb = file_mtime_secs(b).unwrap_or(0.0);
        mb.total_cmp(&ma)
    });
    dedupe_casefold(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, UNIX_EPOCH};
    use tempfile::TempDir;

    fn touch_at(path: &Path, unix_secs: f64) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
        let modified = UNIX_EPOCH + Duration::from_secs_f64(unix_secs);
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(modified).unwrap();
    }

    fn touch_dir_at(path: &Path, unix_secs: f64) {
        fs::create_dir_all(path).unwrap();
        let modified = UNIX_EPOCH + Duration::from_secs_f64(unix_secs);
        let dir = fs::File::open(path).unwrap();
        dir.set_modified(modified).unwrap();
    }

    fn ignore(basenames: &[&str], extensions: &[&str]) -> IgnoreSets {
        IgnoreSets {
            basenames: basenames.iter().map(|s| s.to_lowercase()).collect(),
            extensions: extensions.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    // -- resolved_ignore_sets ------------------------------------------

    #[test]
    fn default_basenames_include_d9_credential_files() {
        let ignore = resolved_ignore_sets(None, None, SaveType::Save, false);
        for name in [
            ".ds_store",
            "desktop.ini",
            "ehthumbs.db",
            "thumbs.db",
            "retroarch.cfg",
            "pcsx2.ini",
            "ppsspp.ini",
            "ppsspp_retroachievements.dat",
        ] {
            assert!(
                ignore.basenames.contains(name),
                "missing default basename: {name}"
            );
        }
    }

    #[test]
    fn pcsx2_ignore_set_gains_the_superblock_basename() {
        let save_ignore = resolved_ignore_sets(None, None, SaveType::Save, true);
        assert!(save_ignore.basenames.contains("_pcsx2_superblock"));

        // Only added for save_type == Save, and only for PCSX2.
        let state_ignore = resolved_ignore_sets(None, None, SaveType::State, true);
        assert!(!state_ignore.basenames.contains("_pcsx2_superblock"));

        let non_pcsx2 = resolved_ignore_sets(None, None, SaveType::Save, false);
        assert!(!non_pcsx2.basenames.contains("_pcsx2_superblock"));
    }

    #[test]
    fn entry_ignore_files_win_over_profile_when_non_blank() {
        let entry = EmulatorEntry {
            ignore_files: "Keep.txt;\nAnother.log".into(),
            ..Default::default()
        };
        let profile = EmulatorProfile {
            ignore_files: strings(&["Profile.dat"]),
            ..Default::default()
        };
        let ignore = resolved_ignore_sets(Some(&entry), Some(&profile), SaveType::Save, false);
        assert!(ignore.basenames.contains("keep.txt"));
        assert!(ignore.basenames.contains("another.log"));
        assert!(!ignore.basenames.contains("profile.dat"));
    }

    #[test]
    fn profile_ignore_files_are_the_fallback_when_entry_is_blank() {
        let entry = EmulatorEntry {
            ignore_files: "   ".into(),
            ..Default::default()
        };
        let profile = EmulatorProfile {
            ignore_files: strings(&["Profile.dat"]),
            ..Default::default()
        };
        let ignore = resolved_ignore_sets(Some(&entry), Some(&profile), SaveType::Save, false);
        assert!(ignore.basenames.contains("profile.dat"));
    }

    #[test]
    fn ignore_extensions_resolve_and_reject_jpg() {
        let entry = EmulatorEntry {
            ignore_extensions: "*.tmp;.LOG".into(),
            ..Default::default()
        };
        let ignore = resolved_ignore_sets(Some(&entry), None, SaveType::Save, false);
        assert!(ignore.extensions.contains(".tmp"));
        assert!(ignore.extensions.contains(".log"));

        let jpg_entry = EmulatorEntry {
            ignore_extensions: "*.jpg".into(),
            ..Default::default()
        };
        let jpg_ignore = resolved_ignore_sets(Some(&jpg_entry), None, SaveType::Save, false);
        assert!(jpg_ignore.extensions.is_empty());
    }

    // -- resolved_save_strategy ------------------------------------------

    #[test]
    fn save_strategy_defaults_by_save_type() {
        assert_eq!(resolved_save_strategy(None, None, SaveType::Save), "auto");
        assert_eq!(
            resolved_save_strategy(None, None, SaveType::State),
            "single_file"
        );
    }

    #[test]
    fn save_strategy_prefers_entry_then_profile() {
        let entry = EmulatorEntry {
            save_strategy: "folder".into(),
            ..Default::default()
        };
        let profile = EmulatorProfile {
            save_strategy: "single_file".into(),
            ..Default::default()
        };
        assert_eq!(
            resolved_save_strategy(Some(&entry), Some(&profile), SaveType::Save),
            "folder"
        );

        let auto_entry = EmulatorEntry::default();
        assert_eq!(
            resolved_save_strategy(Some(&auto_entry), Some(&profile), SaveType::Save),
            "single_file"
        );
    }

    // -- file_candidates: save ---------------------------------------------

    #[test]
    fn save_candidates_accept_explicit_file_roots() {
        // Port of test_cloud_transfer.py:512.
        let dir = TempDir::new().unwrap();
        let memory_card = dir.path().join("Card A.raw");
        touch_at(&memory_card, 1_000.0);

        let candidates = file_candidates(
            std::slice::from_ref(&memory_card),
            &set(&["f-zero gx", "fzerogx"]),
            SaveType::Save,
            &IgnoreSets::default(),
            &[],
        );
        assert_eq!(candidates, vec![memory_card]);
    }

    #[test]
    fn file_candidates_skip_blocked_basenames_extensions_and_d9_credential_files() {
        let dir = TempDir::new().unwrap();
        let save_dir = dir.path().join("saves");
        let keep = save_dir.join("Chrono Trigger.srm");
        let blocked_basename = save_dir.join("thumbs.db");
        let blocked_extension = save_dir.join("Chrono Trigger.tmp");
        let credential_file = save_dir.join("retroarch.cfg");
        touch_at(&keep, 100.0);
        touch_at(&blocked_basename, 200.0);
        touch_at(&blocked_extension, 300.0);
        touch_at(&credential_file, 400.0);

        let ignore = resolved_ignore_sets(None, None, SaveType::Save, false);
        let mut effective_ignore = ignore;
        effective_ignore.extensions.insert(".tmp".to_string());

        let candidates = file_candidates(
            &[save_dir],
            &BTreeSet::new(),
            SaveType::Save,
            &effective_ignore,
            &[],
        );
        assert_eq!(names(&candidates), vec!["Chrono Trigger.srm"]);
    }

    #[test]
    fn save_candidates_match_on_compacted_stem_substring() {
        let dir = TempDir::new().unwrap();
        let save_dir = dir.path().join("saves");
        // "SonicTheHedgehog" only shows up once the stem is lowercased and
        // stripped of the hyphen — the raw lowercased file name alone
        // ("sonic-the-hedgehog.srm") does NOT contain the token
        // "sonicthehedgehog" as a substring.
        let matching = save_dir.join("Sonic-The-Hedgehog.srm");
        let unrelated = save_dir.join("Streets of Rage.srm");
        touch_at(&matching, 100.0);
        touch_at(&unrelated, 200.0);

        let candidates = file_candidates(
            &[save_dir],
            &set(&["sonicthehedgehog"]),
            SaveType::Save,
            &IgnoreSets::default(),
            &[],
        );
        assert_eq!(names(&candidates), vec!["Sonic-The-Hedgehog.srm"]);
    }

    // -- file_candidates: state ----------------------------------------

    #[test]
    fn state_candidates_filtered_to_matching_rom_name() {
        // Port of test_cloud_transfer.py:565.
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("states");
        let matching_state = state_dir.join("Sonic The Hedgehog.state1");
        let matching_auto_state = state_dir.join("Sonic The Hedgehog.state.auto");
        let unrelated_state = state_dir.join("Streets of Rage.state1");
        touch_at(&matching_state, 100.0);
        touch_at(&matching_auto_state, 200.0);
        touch_at(&unrelated_state, 300.0);

        let candidates = file_candidates(
            &[state_dir],
            &set(&["sonic the hedgehog", "sonicthehedgehog"]),
            SaveType::State,
            &IgnoreSets::default(),
            &[],
        );
        assert_eq!(
            names(&candidates),
            vec![
                "Sonic The Hedgehog.state.auto".to_string(),
                "Sonic The Hedgehog.state1".to_string(),
            ]
        );
    }

    #[test]
    fn state_candidates_allow_only_common_name_variants() {
        // Port of test_cloud_transfer.py:595.
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("states");
        let matching_dash_variant = state_dir.join("Sonic-The-Hedgehog.state2");
        let matching_compact_variant = state_dir.join("SonicTheHedgehog.state.auto");
        let unrelated_variant = state_dir.join("Sonic Spinball.state1");
        let sequel_variant = state_dir.join("SonicTheHedgehog2.state3");
        touch_at(&matching_dash_variant, 100.0);
        touch_at(&matching_compact_variant, 200.0);
        touch_at(&unrelated_variant, 300.0);
        touch_at(&sequel_variant, 400.0);

        let mut candidates = file_candidates(
            &[state_dir],
            &set(&["sonic the hedgehog", "sonicthehedgehog"]),
            SaveType::State,
            &IgnoreSets::default(),
            &[],
        );
        candidates.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        assert_eq!(
            names(&candidates),
            vec![
                "Sonic-The-Hedgehog.state2".to_string(),
                "SonicTheHedgehog.state.auto".to_string(),
            ]
        );
    }

    #[test]
    fn state_fallback_returns_the_newest_hash_group() {
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("states");
        // No tokens match any of these (unrelated title), so all three land
        // in the unmatched bucket and the hash-group fallback decides.
        let newest = state_dir.join("d4a53e48.1.sav");
        let older_same_group = state_dir.join("d4a53e48.0.sav");
        let unrelated_group = state_dir.join("aaaaaaaa.0.sav");
        touch_at(&newest, 300.0);
        touch_at(&older_same_group, 100.0);
        touch_at(&unrelated_group, 200.0);

        let candidates = file_candidates(
            &[state_dir],
            &set(&["unrelated title"]),
            SaveType::State,
            &IgnoreSets::default(),
            &[],
        );
        assert_eq!(
            names(&candidates),
            vec!["d4a53e48.1.sav".to_string(), "d4a53e48.0.sav".to_string()]
        );
    }

    #[test]
    fn state_fallback_single_unmatched_candidate_is_taken() {
        let dir = TempDir::new().unwrap();
        let state_dir = dir.path().join("states");
        let only = state_dir.join("random.state1");
        touch_at(&only, 100.0);

        let candidates = file_candidates(
            &[state_dir],
            &set(&["unrelated title"]),
            SaveType::State,
            &IgnoreSets::default(),
            &[],
        );
        assert_eq!(names(&candidates), vec!["random.state1".to_string()]);
    }

    #[test]
    fn ordering_is_mtime_desc_then_name_with_ci_dedupe() {
        let dir = TempDir::new().unwrap();
        let save_dir = dir.path().join("saves");
        let older_b = save_dir.join("b.srm");
        let same_time_a = save_dir.join("a.srm");
        let same_time_c = save_dir.join("C.srm");
        touch_at(&older_b, 100.0);
        touch_at(&same_time_a, 200.0);
        touch_at(&same_time_c, 200.0);

        let candidates = file_candidates(
            std::slice::from_ref(&save_dir),
            &BTreeSet::new(),
            SaveType::Save,
            &IgnoreSets::default(),
            &[],
        );
        // Same mtime (200.0): lowercased-name tiebreak puts "a.srm" before
        // "C.srm" ("a" < "c"); the older file sorts last.
        assert_eq!(
            names(&candidates),
            vec![
                "a.srm".to_string(),
                "C.srm".to_string(),
                "b.srm".to_string()
            ]
        );

        // The same directory listed twice must not double up the results —
        // dedupe keys on the full (casefolded) path string, so the second
        // pass over `save_dir` contributes nothing new.
        let deduped = file_candidates(
            &[save_dir.clone(), save_dir],
            &BTreeSet::new(),
            SaveType::Save,
            &IgnoreSets::default(),
            &[],
        );
        assert_eq!(deduped.len(), 3);
    }

    // -- directory_candidates --------------------------------------------

    #[test]
    fn directory_candidates_require_a_non_blocked_file_beneath() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let empty_child = root.join("empty-game");
        let only_blocked_child = root.join("only-blocked");
        let real_child = root.join("real-game");
        fs::create_dir_all(&empty_child).unwrap();
        touch_at(&only_blocked_child.join("thumbs.db"), 100.0);
        touch_at(&real_child.join("save.dat"), 200.0);

        let candidates =
            directory_candidates(&[root], &BTreeSet::new(), &ignore(&["thumbs.db"], &[]));
        assert_eq!(names(&candidates), vec!["real-game".to_string()]);
    }

    #[test]
    fn directory_candidates_match_compacted_name_or_relative_path() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let matching = root.join("Chrono Trigger");
        let unrelated = root.join("Streets of Rage");
        touch_at(&matching.join("save.dat"), 100.0);
        touch_at(&unrelated.join("save.dat"), 200.0);

        let candidates =
            directory_candidates(&[root], &set(&["chronotrigger"]), &IgnoreSets::default());
        assert_eq!(names(&candidates), vec!["Chrono Trigger".to_string()]);
    }

    // -- cemu_save_directories --------------------------------------------

    #[test]
    fn cemu_selects_nested_user_folders() {
        // Port of test_cloud_transfer.py:532.
        let dir = TempDir::new().unwrap();
        let save_root = dir.path().join("mlc01/usr/save");
        let persistent_dir = save_root.join("00050000/1010ED00/user/80000001");
        let common_dir = save_root.join("00050000/1010ED00/user/common");
        let unrelated_dir = save_root.join("00050000/1010EE00/user/80000001");
        touch_at(&persistent_dir.join("progress.dat"), 100.0);
        touch_at(&common_dir.join("settings.dat"), 200.0);
        touch_at(&unrelated_dir.join("other.dat"), 300.0);

        let candidates = cemu_save_directories(
            &[save_root],
            &set(&["000500001010ED00", "00050000", "1010ED00"]),
            &IgnoreSets::default(),
        );

        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(&persistent_dir));
        assert!(candidates.contains(&common_dir));
        assert!(!candidates.contains(&unrelated_dir));
    }

    #[test]
    fn cemu_uses_user_root_itself_when_childless() {
        let dir = TempDir::new().unwrap();
        let save_root = dir.path().join("mlc01/usr/save");
        let user_root = save_root.join("00050000/1010ED00/user");
        touch_at(&user_root.join("save.dat"), 100.0);

        let candidates =
            cemu_save_directories(&[save_root], &BTreeSet::new(), &IgnoreSets::default());
        assert_eq!(candidates, vec![user_root]);
    }

    #[test]
    fn cemu_drops_candidates_with_zero_latest_mtime() {
        let dir = TempDir::new().unwrap();
        let save_root = dir.path().join("mlc01/usr/save");
        let empty_user_dir = save_root.join("00050000/1010ED00/user");
        fs::create_dir_all(&empty_user_dir).unwrap();

        let candidates =
            cemu_save_directories(&[save_root], &BTreeSet::new(), &IgnoreSets::default());
        assert!(candidates.is_empty());
    }

    // -- pcsx2_save_directories --------------------------------------------

    #[test]
    fn pcsx2_matches_serial_and_sorts_newest_first() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let matching = root.join("SLUS-12345");
        let unrelated = root.join("SLUS-99999");
        touch_at(&matching.join("save.dat"), 100.0);
        touch_at(&unrelated.join("save.dat"), 200.0);

        let candidates =
            pcsx2_save_directories(&[root], &set(&["SLUS12345"]), &IgnoreSets::default());
        assert_eq!(names(&candidates), vec!["SLUS-12345".to_string()]);
    }

    #[test]
    fn pcsx2_ignore_set_affects_sort_but_not_the_file_existence_check() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let only_superblock = root.join("only-superblock");
        let real_save = root.join("real-save");
        // A directory containing ONLY an (ignored) superblock still passes
        // the "at least one file" existence check (unlike
        // `directory_candidates`, which requires a NON-blocked file) —
        // that's the literal Python port. But the superblock's recent
        // touch must not win the sort once it's in the ignore set.
        touch_at(&only_superblock.join("_pcsx2_superblock"), 900.0);
        touch_at(&real_save.join("save.dat"), 100.0);

        let mut with_superblock_ignored = IgnoreSets::default();
        with_superblock_ignored
            .basenames
            .insert("_pcsx2_superblock".to_string());

        let candidates =
            pcsx2_save_directories(&[root], &BTreeSet::new(), &with_superblock_ignored);
        assert_eq!(candidates.len(), 2);
        // real-save (mtime 100) now outranks only-superblock (whose only
        // file is ignored, so its latest_mtime_under is 0.0).
        assert_eq!(
            names(&candidates),
            vec!["real-save".to_string(), "only-superblock".to_string()]
        );
    }

    // -- rpcs3_save_directories ---------------------------------------------

    #[test]
    fn rpcs3_directory_index_outranks_recency() {
        let dir = TempDir::new().unwrap();
        let stale_root = dir.path().join("dir0");
        let fresh_root = dir.path().join("dir1");
        let stale_child = stale_root.join("BLUS30443");
        let fresh_child = fresh_root.join("BLUS30443-copy");
        touch_at(&stale_child.join("save.dat"), 100.0);
        touch_at(&fresh_child.join("save.dat"), 900.0);

        let candidates = rpcs3_save_directories(&[stale_root, fresh_root], &[]);
        // dirs[0]'s child sorts first even though dirs[1]'s child has a
        // much newer mtime.
        assert_eq!(
            names(&candidates),
            vec!["BLUS30443".to_string(), "BLUS30443-copy".to_string()]
        );
    }

    #[test]
    fn rpcs3_matches_game_id_substring() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let matching = root.join("BLUS30443");
        let unrelated = root.join("BLUS99999");
        fs::create_dir_all(&matching).unwrap();
        fs::create_dir_all(&unrelated).unwrap();

        let candidates = rpcs3_save_directories(&[root], &strings(&["BLUS30443"]));
        assert_eq!(names(&candidates), vec!["BLUS30443".to_string()]);
    }

    // -- ppsspp_save_directories ---------------------------------------------

    #[test]
    fn ppsspp_uses_directory_own_mtime_and_ignores_nothing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let stale_by_content_fresh_by_dir = root.join("ULUS10307");
        let fresh_by_content_stale_by_dir = root.join("ULUS10307-copy");
        fs::create_dir_all(&stale_by_content_fresh_by_dir).unwrap();
        fs::create_dir_all(&fresh_by_content_stale_by_dir).unwrap();
        // The directory's own mtime is what's sorted on — NOT the mtime of
        // a file inside it. Put an OLDER file inside the directory that
        // should sort first, and touch the directory itself LAST/newest.
        touch_at(&stale_by_content_fresh_by_dir.join("old.bin"), 100.0);
        touch_dir_at(&stale_by_content_fresh_by_dir, 900.0);
        touch_at(&fresh_by_content_stale_by_dir.join("new.bin"), 800.0);
        touch_dir_at(&fresh_by_content_stale_by_dir, 200.0);

        let candidates = ppsspp_save_directories(&[root], &BTreeSet::new());
        assert_eq!(
            names(&candidates),
            vec!["ULUS10307".to_string(), "ULUS10307-copy".to_string(),]
        );
    }

    #[test]
    fn ppsspp_requires_no_contained_file() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let empty_dir = root.join("ULUS10307");
        fs::create_dir_all(&empty_dir).unwrap();

        let candidates = ppsspp_save_directories(&[root], &BTreeSet::new());
        assert_eq!(candidates, vec![empty_dir]);
    }

    #[test]
    fn ppsspp_matches_id_substring() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root");
        let matching = root.join("ULUS10307");
        let unrelated = root.join("ULUS99999");
        fs::create_dir_all(&matching).unwrap();
        fs::create_dir_all(&unrelated).unwrap();

        let candidates = ppsspp_save_directories(&[root], &set(&["ULUS10307"]));
        assert_eq!(names(&candidates), vec!["ULUS10307".to_string()]);
    }
}
