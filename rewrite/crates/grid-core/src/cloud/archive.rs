//! Cloud archive writers, filtered extraction, and zip-slip guards.
//!
//! Ported from `grid_launcher/library/cloud_transfer.py`:
//! `_temporary_archive_path` (:286), `zip_selected_files_for_upload` /
//! `zip_directory_for_upload` / `zip_native_save_dirs_for_upload`
//! (:311-476), `extract_zip_archive_bytes_to_directory` +
//! `_extract_zip_with_7z` (:34,:150-215,:240-283), and temp cleanup
//! (`cleanup_temporary_paths`, :691). The zip-sniff helper mirrors
//! `grid_launcher/library/cloud_restore.py:180`'s `_payload_is_zip_archive`
//! — see [`payload_is_zip`]'s doc comment for the one deliberate,
//! brief-pinned divergence from it.
//!
//! See `docs/porting/06-cloud-saves.md` for the invariants this module
//! implements (zip-slip guard, ignore filtering on both write and extract
//! sides, partial-archive cleanup on write failure).

use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::library::extract::which_on_path;
use crate::library::paths::sanitize_component;

use super::IgnoreSets;

// --- temp archive path -------------------------------------------------

/// Builds a fresh temporary archive path in `std::env::temp_dir()` named
/// `"<sanitized-title>-<local ISO-8601 seconds, ':' -> '-'>.zip"`,
/// appending the current unix-epoch millisecond count as a further `-<ms>`
/// suffix if that path already exists.
///
/// `title` is sanitized internally via [`sanitize_component`] (the same
/// component sanitizer Python's three call sites apply *before* calling
/// `_temporary_archive_path`, see `cloud_transfer.py:322,405,449` and
/// `cloud_mixin.py:1462,2723`), then trimmed and defaulted to `"game"` when
/// blank — mirroring `_temporary_archive_path`'s own `safe_title.strip() or
/// "game"` on top of the caller's sanitization. Folding both steps in here
/// means every caller in this module can pass a raw, unsanitized title.
pub fn temp_archive_path(title: &str) -> PathBuf {
    let sanitized = sanitize_component(title, "game");
    let trimmed = sanitized.trim();
    let base_title = if trimmed.is_empty() { "game" } else { trimmed };

    let timestamp = chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
        .replace(':', "-");

    let archive_path = std::env::temp_dir().join(format!("{base_title}-{timestamp}.zip"));
    if !archive_path.exists() {
        return archive_path;
    }

    let suffix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!("{base_title}-{timestamp}-{suffix_ms}.zip"))
}

// --- shared writer plumbing ---------------------------------------------

fn deflate_options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn zip_err_to_io(err: zip::result::ZipError) -> io::Error {
    io::Error::other(err)
}

/// Writes `source`'s bytes into `writer` under `member_name`, deflate
/// compressed.
fn write_file_entry<W: Write + io::Seek>(
    writer: &mut ZipWriter<W>,
    source: &Path,
    member_name: &str,
) -> io::Result<()> {
    writer
        .start_file(member_name, deflate_options())
        .map_err(zip_err_to_io)?;
    let mut file = fs::File::open(source)?;
    io::copy(&mut file, writer)?;
    Ok(())
}

/// `path`, rendered with `/` separators regardless of platform — mirrors
/// Python's `PurePath.as_posix()` for zip member names.
fn to_posix_member(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Deletes `path` if it exists, best-effort (mirrors the
/// `if archive_path.exists(): archive_path.unlink()` cleanup Python runs
/// in every writer's `except OSError` branch).
fn unlink_best_effort(path: &Path) {
    let _ = fs::remove_file(path);
}

/// All filesystem entries at or under `root`, both files and directories,
/// `root` itself excluded — mirrors Python's `Path.rglob("*")`. As in
/// [`super::latest_mtime_under`], a directory is only recursed into when
/// `DirEntry::file_type` (which does not follow symlinks) itself reports
/// `is_dir()`; a symlinked directory is listed as an entry but not
/// descended into, matching `rglob`'s own behavior.
fn walk_all_entries(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_real_dir = entry.file_type()?.is_dir();
            if is_real_dir {
                stack.push(path.clone());
            }
            out.push(path);
        }
    }
    Ok(out)
}

/// [`walk_all_entries`], filtered to regular files only (following
/// symlinks, like `Path.is_file()`).
fn walk_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    Ok(walk_all_entries(root)?
        .into_iter()
        .filter(|p| p.is_file())
        .collect())
}

// --- zip_directory_for_upload --------------------------------------------

/// Zips every non-ignored file under `dir` into a fresh temp archive, with
/// each member named `"<dir's own basename>/<path relative to dir,
/// posix-separated>"`. Mirrors `zip_directory_for_upload`
/// (`cloud_transfer.py:399-424`).
///
/// The archive's own filename is derived from `dir`'s basename (via
/// [`temp_archive_path`]) rather than a caller-supplied game title — a
/// deliberate interface simplification from Python's `safe_title`
/// parameter, which only ever affected the throwaway temp filename, never
/// the member names (always `dirname`-prefixed regardless). See the task
/// report for this documented discrepancy.
///
/// On any I/O failure — walking `dir`, opening a source file, or writing
/// an entry — the partial archive is unlinked and the error propagated,
/// matching Python's `except OSError: archive_path.unlink(...); raise`.
pub fn zip_directory_for_upload(dir: &Path, ignore: &IgnoreSets) -> io::Result<PathBuf> {
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archive")
        .to_string();
    let archive_path = temp_archive_path(&dir_name);

    match write_directory_archive(&archive_path, dir, &dir_name, ignore) {
        Ok(()) => Ok(archive_path),
        Err(err) => {
            unlink_best_effort(&archive_path);
            Err(err)
        }
    }
}

fn write_directory_archive(
    archive_path: &Path,
    dir: &Path,
    dir_name: &str,
    ignore: &IgnoreSets,
) -> io::Result<()> {
    let file = fs::File::create(archive_path)?;
    let mut writer = ZipWriter::new(file);

    for candidate in walk_all_entries(dir)? {
        if !candidate.is_file() {
            continue;
        }
        if ignore.blocks(&candidate) {
            continue;
        }
        let relative = candidate
            .strip_prefix(dir)
            .expect("walk_all_entries always yields paths under root");
        let member_name = format!("{dir_name}/{}", to_posix_member(relative));
        write_file_entry(&mut writer, &candidate, &member_name)?;
    }

    writer.finish().map_err(zip_err_to_io)?;
    Ok(())
}

// --- zip_grouped_files_for_upload -----------------------------------------

/// Dedupes `files` to those that exist and are regular files
/// (case-insensitive dedupe on the path string, first occurrence kept),
/// mirroring `_unique_existing_files` (`cloud_transfer.py:298-309`).
fn unique_existing_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = std::collections::BTreeSet::new();
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

/// Longest common ancestor of `paths`' *parent* directories, by path
/// component — mirrors `os.path.commonpath([p.parent for p in files])`.
/// Falls back to the first file's own parent when there is no common
/// ancestor (mixed roots), matching `zip_selected_files_for_upload`'s
/// `except ValueError` branch.
fn common_parent(files: &[PathBuf]) -> PathBuf {
    let mut parents = files.iter().map(|f| {
        f.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let Some(first) = parents.next() else {
        return PathBuf::from(".");
    };
    let mut common: Vec<Component> = first.components().collect();
    for parent in parents {
        let comps: Vec<Component> = parent.components().collect();
        let shared = common
            .iter()
            .zip(comps.iter())
            .take_while(|(a, b)| a == b)
            .count();
        common.truncate(shared);
    }
    if common.is_empty() {
        return files[0]
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
    }
    common.into_iter().collect()
}

/// Zips `files` into a fresh temp archive named from `archive_name_stem`
/// (via [`temp_archive_path`]), with members named relative to the common
/// parent of every file's directory, falling back to the bare file name
/// when a file isn't actually under that common parent. Mirrors
/// `zip_selected_files_for_upload` (`cloud_transfer.py:311-345`).
///
/// Unlike the Python original, this does not take ignore-basename/
/// extension parameters — the brief's signature treats grouped/selected
/// file lists as pre-filtered by the caller before reaching this low-level
/// writer. See the task report for this documented discrepancy.
///
/// Files are deduped to existing regular files first ([`unique_existing_files`]);
/// if none remain, returns an `InvalidInput` error (mirrors Python's
/// `ValueError("No files were provided to archive for upload.")`). On any
/// I/O failure while writing, the partial archive is unlinked and the
/// error propagated.
pub fn zip_grouped_files_for_upload(
    files: &[PathBuf],
    archive_name_stem: &str,
) -> io::Result<PathBuf> {
    let mut selected = unique_existing_files(files);
    if selected.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No files were provided to archive for upload.",
        ));
    }

    let archive_path = temp_archive_path(archive_name_stem);
    let root = common_parent(&selected);
    selected.sort_by_key(|p| p.to_string_lossy().to_lowercase());

    match write_grouped_archive(&archive_path, &selected, &root) {
        Ok(()) => Ok(archive_path),
        Err(err) => {
            unlink_best_effort(&archive_path);
            Err(err)
        }
    }
}

fn write_grouped_archive(archive_path: &Path, files: &[PathBuf], root: &Path) -> io::Result<()> {
    let file = fs::File::create(archive_path)?;
    let mut writer = ZipWriter::new(file);

    for source in files {
        let member_name = match source.strip_prefix(root) {
            Ok(relative) => to_posix_member(relative),
            Err(_) => source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        write_file_entry(&mut writer, source, &member_name)?;
    }

    writer.finish().map_err(zip_err_to_io)?;
    Ok(())
}

// --- zip_native_save_dirs_for_upload ---------------------------------------

/// Zips the contents of multiple native-save directories into one combined
/// temp archive, each directory's files under an `"<index>/<relative
/// path>"` prefix, plus a top-level `_grid_launcher_dirs.json` manifest
/// mapping `"<index>"` to that directory's raw (unexpanded) path string.
/// Mirrors `zip_native_save_dirs_for_upload` (`cloud_transfer.py:431-476`).
///
/// Unlike Python, this does not take a title parameter (archive naming
/// falls back to `"game"` via [`temp_archive_path`], since no single
/// directory's name is representative) and returns only `(archive,
/// files_added)`, not the manifest map — the caller has `dirs` already and
/// can reconstruct it if needed. See the task report for this documented
/// discrepancy.
///
/// A directory whose full listing can't be read is skipped entirely and
/// omitted from the manifest; an individual file that can't be read is
/// skipped, without affecting the rest of that directory. The manifest
/// member is always written, even when zero files were added. Only a
/// failure creating the archive file itself or writing the manifest aborts
/// the whole operation (partial archive unlinked, error propagated) —
/// matching Python's two independently-caught inner `try/except OSError`
/// blocks versus its one outer `except OSError` around archive creation.
pub fn zip_native_save_dirs_for_upload(
    dirs: &[(String, PathBuf)],
    ignore: &IgnoreSets,
) -> io::Result<(PathBuf, usize)> {
    let archive_path = temp_archive_path("");

    match write_native_save_dirs_archive(&archive_path, dirs, ignore) {
        Ok(total_files) => Ok((archive_path, total_files)),
        Err(err) => {
            unlink_best_effort(&archive_path);
            Err(err)
        }
    }
}

fn write_native_save_dirs_archive(
    archive_path: &Path,
    dirs: &[(String, PathBuf)],
    ignore: &IgnoreSets,
) -> io::Result<usize> {
    let file = fs::File::create(archive_path)?;
    let mut writer = ZipWriter::new(file);

    let mut manifest = serde_json::Map::new();
    let mut total_files = 0usize;

    for (idx, (raw_path, directory)) in dirs.iter().enumerate() {
        let Ok(mut candidates) = walk_all_entries(directory) else {
            continue;
        };
        candidates.sort_by_key(|p| p.to_string_lossy().to_lowercase());

        manifest.insert(idx.to_string(), serde_json::Value::String(raw_path.clone()));

        for candidate in candidates {
            if !candidate.is_file() {
                continue;
            }
            if ignore.blocks(&candidate) {
                continue;
            }
            let relative = match candidate.strip_prefix(directory) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => candidate.file_name().map(PathBuf::from).unwrap_or_default(),
            };
            let member_name = format!("{idx}/{}", to_posix_member(&relative));
            if write_file_entry(&mut writer, &candidate, &member_name).is_ok() {
                total_files += 1;
            }
        }
    }

    let manifest_bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(manifest))
        .expect("manifest of strings always serializes");
    writer
        .start_file("_grid_launcher_dirs.json", deflate_options())
        .map_err(zip_err_to_io)?;
    writer.write_all(&manifest_bytes)?;

    writer.finish().map_err(zip_err_to_io)?;
    Ok(total_files)
}

// --- payload sniff + extraction --------------------------------------------

/// Whether `bytes` starts with the zip local-file-header magic `"PK"`.
///
/// **Deliberate divergence from Python** (task-brief pinned): the Python
/// original this ports, `_payload_is_zip_archive`
/// (`cloud_restore.py:179-182`), calls `zipfile.is_zipfile`, which scans
/// for a valid end-of-central-directory record — a real structural check.
/// This port intentionally narrows that to a magic-only sniff: fast,
/// dependency-free, and sufficient as a *routing* signal (zip-shaped vs.
/// raw payload) since [`extract_payload_zip`] performs its own real
/// validation via the `zip` crate regardless. A payload that merely starts
/// with `PK` but isn't a well-formed archive returns `true` here yet still
/// fails extraction with a clear error there.
pub fn payload_is_zip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && &bytes[..2] == b"PK"
}

/// Whether `raw_name` (a raw, untrusted zip member name, `\` not yet
/// normalized) is safe to extract: not absolute, and no component is `.`
/// or `..`. Mirrors the traversal guard in
/// `extract_zip_archive_bytes_to_directory`
/// (`cloud_transfer.py:253-262`): `relative_path.is_absolute() or
/// any(part in {"", ".", ".."} for part in relative_path.parts)`.
///
/// Python's check is against `PurePath(...).parts`, which — like this
/// function — collapses runs of repeated separators before splitting into
/// components: `PurePosixPath("a//b").parts == ("a", "b")`, not `("a", "",
/// "b")`. So an empty segment produced only by a repeated `/` (`"a//b"`)
/// is *not* itself treated as an unsafe `""` component here; it is
/// filtered out before the `.`/`..` check, matching Python exactly. A
/// name that is empty, or made of nothing but separators, is still
/// rejected — by the `is_empty` check below (an all-slash name is already
/// excluded earlier: it's either absolute, caught by `is_absolute`, or a
/// trailing-slash directory entry, filtered by the caller before this
/// function is ever reached).
pub(crate) fn is_safe_member_name(normalized: &str) -> bool {
    if normalized.is_empty() {
        return false;
    }
    if Path::new(normalized).is_absolute() {
        return false;
    }
    !normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .any(|part| part == "." || part == "..")
}

/// Resolves `relative` under `dest_root` the way Python's
/// `Path.resolve(strict=False)` does when checked against
/// `destination_root` (`cloud_transfer.py:253-262`, and the 7z path's
/// `:198`): each path component that already exists on disk is
/// symlink-resolved (via [`Path::canonicalize`]) as the walk descends into
/// it; once a component is reached that doesn't exist yet, every
/// remaining component is appended lexically, with no further resolution
/// (nothing to resolve — it isn't there). `dest_root` must already be
/// canonical (both call sites pass an already-canonicalized destination
/// root).
///
/// This is what makes the zip-slip guard symlink-aware: a *lexical*
/// `dest_root.join(relative)` + `starts_with(dest_root)` check (which this
/// function replaces at both call sites) never notices a pre-existing
/// symlink under `dest_root` — a symlinked save directory, common under
/// Flatpak/portable installs — pointing outside it. Walking and resolving
/// component-by-component catches that: the symlinked component
/// canonicalizes to its real, out-of-root target, so the final path's
/// `starts_with(dest_root)` check (still applied by the caller) correctly
/// fails.
pub(crate) fn resolve_under_root(dest_root: &Path, relative: &Path) -> io::Result<PathBuf> {
    let mut resolved = dest_root.to_path_buf();
    let mut still_existing = true;
    for component in relative.components() {
        let candidate = resolved.join(component);
        if still_existing && candidate.exists() {
            resolved = candidate.canonicalize()?;
        } else {
            still_existing = false;
            resolved = candidate;
        }
    }
    Ok(resolved)
}

/// Extracts a zip archive's bytes into `dest`, skipping members blocked by
/// `ignore` and rejecting zip-slip attempts, returning the number of files
/// written. Mirrors `extract_zip_archive_bytes_to_directory`
/// (`cloud_transfer.py:225-278`).
///
/// If the archive fails to parse at all, returns a "not a zip archive"
/// error (no fallback — this corresponds to Python's `zipfile.is_zipfile`
/// gate rejecting the payload up front). If a *member* fails to decode
/// because its compression method isn't supported by the `zip` crate
/// (`ZipError::CompressionMethodNotSupported`/`UnsupportedArchive` — the
/// crate-accurate trigger for what Python observes as `NotImplementedError`
/// from `zipfile`), extraction falls back to a system 7-Zip binary,
/// re-extracting the *whole* archive into a scratch temp directory and
/// re-applying the same ignore + zip-slip checks before copying files into
/// `dest` — matching `_extract_zip_with_7z` (`cloud_transfer.py:150-215`).
/// As in Python, whatever files the zip path already wrote before hitting
/// the unsupported member are left in place; only the fallback's own count
/// is returned (the two counts are never summed).
pub fn extract_payload_zip(
    bytes: &[u8],
    dest: &Path,
    ignore: &IgnoreSets,
) -> Result<usize, String> {
    fs::create_dir_all(dest).map_err(|e| format!("failed to prepare destination: {e}"))?;
    let dest_root = dest
        .canonicalize()
        .map_err(|e| format!("failed to resolve destination: {e}"))?;

    let mut archive = match ZipArchive::new(Cursor::new(bytes)) {
        Ok(archive) => archive,
        Err(_) => return Err("Downloaded save is not a zip archive.".to_string()),
    };

    let mut extracted = 0usize;
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(entry) => entry,
            Err(
                zip::result::ZipError::UnsupportedArchive(_)
                | zip::result::ZipError::CompressionMethodNotSupported(_),
            ) => {
                return extract_with_system_7z(bytes, &dest_root, ignore);
            }
            Err(err) => return Err(format!("failed to read archive entry: {err}")),
        };

        let raw_name = entry.name().to_string();
        let normalized = raw_name.replace('\\', "/");
        if normalized.ends_with('/') {
            continue; // directory entry
        }
        if !is_safe_member_name(&normalized) {
            continue;
        }
        let relative = Path::new(&normalized);
        if ignore.blocks(relative) {
            continue;
        }

        let out_path = resolve_under_root(&dest_root, relative)
            .map_err(|e| format!("failed to resolve extraction path: {e}"))?;
        if !out_path.starts_with(&dest_root) {
            continue; // zip-slip guard: resolved path escapes dest_root (e.g. via a symlink)
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("failed to create directory: {e}"))?;
        }
        let mut out_file =
            fs::File::create(&out_path).map_err(|e| format!("failed to create file: {e}"))?;
        io::copy(&mut entry, &mut out_file).map_err(|e| format!("failed to extract file: {e}"))?;
        extracted += 1;
    }

    Ok(extracted)
}

/// Python's exact 7-Zip candidate order (`cloud_transfer.py:150-165`):
/// the bundled `assets/tools/7z/7z.exe` on Windows, checked first, then
/// `7z`, `7za`, `7zz` from `PATH` — in that order, trying each in turn
/// until one succeeds. This intentionally differs from
/// `library::extract::find_system_7z`'s order (no bundled-exe check, plus
/// extra hardcoded Unix install locations as a further fallback) — that
/// function serves the install/extraction engine, not cloud saves; see the
/// task report for why the two aren't unified.
fn run_system_7z_extract(archive: &Path, dest: &Path) -> Result<(), String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(bundled) = bundled_7z_windows_path() {
        if bundled.is_file() {
            candidates.push(bundled);
        }
    }
    for name in ["7z", "7za", "7zz"] {
        candidates.push(which_on_path(name).unwrap_or_else(|| PathBuf::from(name)));
    }

    for command in candidates {
        let mut invocation = Command::new(&command);
        invocation
            .arg("x")
            .arg(archive)
            .arg(format!("-o{}", dest.display()))
            .arg("-y")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        // Mirrors `subprocess.CREATE_NO_WINDOW` in `cloud_transfer.py`'s
        // `_extract_zip_with_7z` (:159-160): suppresses the console window
        // a spawned Windows process would otherwise briefly flash open.
        // Same flag + precedent as `launch/mod.rs`'s `spawn_child`.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            invocation.creation_flags(CREATE_NO_WINDOW);
        }
        let ran = invocation.output();
        if let Ok(output) = ran {
            if output.status.success() {
                return Ok(());
            }
        }
    }

    Err("No 7-Zip found to extract this archive.".to_string())
}

/// The bundled 7-Zip executable's expected location on Windows, next to
/// the running binary (`assets/tools/7z/7z.exe`, mirroring
/// `cloud_transfer.py`'s `_BUNDLED_7Z_PATH`, which is relative to the
/// installed package root). No such bundling convention exists yet
/// elsewhere in this rewrite; this is inert on every platform this crate
/// currently ships for (non-Windows), so it's a documented placeholder
/// pending that convention rather than a verified path.
#[cfg(windows)]
fn bundled_7z_windows_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    Some(
        exe_dir
            .join("assets")
            .join("tools")
            .join("7z")
            .join("7z.exe"),
    )
}

#[cfg(not(windows))]
fn bundled_7z_windows_path() -> Option<PathBuf> {
    None
}

/// The system-7z fallback used from within [`extract_payload_zip`]:
/// writes `bytes` to a temp file, extracts it via `run_system_7z_extract`
/// into a scratch temp directory, then re-walks that directory applying
/// `ignore` + the same zip-slip-style safety check before copying each
/// file into `dest_root`. Mirrors `_extract_zip_with_7z`
/// (`cloud_transfer.py:150-215`); both temp paths are cleaned up via RAII
/// (`tempfile`'s `Drop`), matching Python's `finally: shutil.rmtree(...)`.
fn extract_with_system_7z(
    bytes: &[u8],
    dest_root: &Path,
    ignore: &IgnoreSets,
) -> Result<usize, String> {
    let mut zip_tmp = tempfile::Builder::new()
        .prefix("grid-launcher-save-")
        .suffix(".zip")
        .tempfile()
        .map_err(|e| format!("failed to create temporary archive: {e}"))?;
    zip_tmp
        .write_all(bytes)
        .map_err(|e| format!("failed to write temporary archive: {e}"))?;
    zip_tmp
        .flush()
        .map_err(|e| format!("failed to write temporary archive: {e}"))?;

    let extract_dir = tempfile::Builder::new()
        .prefix("grid-launcher-save-7z-")
        .tempdir()
        .map_err(|e| format!("failed to create temporary directory: {e}"))?;

    run_system_7z_extract(zip_tmp.path(), extract_dir.path())?;

    let mut extracted = 0usize;
    let files = walk_files(extract_dir.path())
        .map_err(|e| format!("failed to read extracted files: {e}"))?;
    for source in files {
        let relative = source
            .strip_prefix(extract_dir.path())
            .expect("walk_files always yields paths under root");
        if ignore.blocks(relative) {
            continue;
        }
        let out_path = resolve_under_root(dest_root, relative)
            .map_err(|e| format!("failed to resolve extraction path: {e}"))?;
        if !out_path.starts_with(dest_root) {
            continue; // zip-slip guard, re-applied after the 7z fallback extraction
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("failed to create directory: {e}"))?;
        }
        fs::copy(&source, &out_path).map_err(|e| format!("failed to copy extracted file: {e}"))?;
        extracted += 1;
    }

    Ok(extracted)
}

// --- cleanup -----------------------------------------------------------

/// Best-effort unlink of every path in `paths`, ignoring errors (missing
/// file, permission). Mirrors `cleanup_temporary_paths`
/// (`cloud_transfer.py:690-696`).
pub fn cleanup_temp_archives(paths: &[PathBuf]) {
    for path in paths {
        if !path.exists() {
            continue;
        }
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;
    use std::sync::Mutex;

    /// Serializes tests that call `zip_native_save_dirs_for_upload` with an
    /// empty title. That title makes every such call resolve the *same*
    /// `temp_archive_path("")` base name (`temp_dir()/game-<second>.zip`);
    /// run concurrently (the default for `cargo test`), two calls landing
    /// in the same wall-clock second can both observe the base path as
    /// not-yet-existing and race to create/overwrite it. Python's
    /// `unittest` runner never hits this because it runs tests
    /// sequentially; this guard reproduces that serialization for just the
    /// tests that share the collision-prone empty title.
    static NATIVE_SAVE_DIR_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn ignore_with(basenames: &[&str], extensions: &[&str]) -> IgnoreSets {
        IgnoreSets {
            basenames: basenames.iter().map(|s| s.to_string()).collect(),
            extensions: extensions.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn default_python_ignore() -> IgnoreSets {
        ignore_with(
            &[".ds_store", "desktop.ini", "ehthumbs.db", "thumbs.db"],
            &[],
        )
    }

    fn read_zip_members(path: &Path) -> std::collections::BTreeSet<String> {
        let file = fs::File::open(path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect()
    }

    /// Builds a zip archive in memory containing `entries` (raw member
    /// name, content), using `Stored` compression so fixture bytes are
    /// predictable. Names are written as given, bypassing any of this
    /// crate's own path sanitization — needed to construct zip-slip and
    /// unsafe-name fixtures.
    fn build_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for &(name, content) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    // --- ported Python oracle tests -----------------------------------

    /// Ports `test_zip_directory_for_upload_skips_os_metadata_files`
    /// (`test_cloud_transfer.py:203`).
    #[test]
    fn zip_directory_for_upload_skips_os_metadata_files() {
        let temp = tempfile::tempdir().unwrap();
        let save_dir = temp.path().join("ULUS12345");
        fs::create_dir(&save_dir).unwrap();
        fs::write(save_dir.join("DATA.BIN"), b"save-data").unwrap();
        fs::write(save_dir.join("ICON0.PNG"), b"\x89PNG\r\n\x1a\n").unwrap();
        fs::write(save_dir.join("Thumbs.db"), b"not-an-image").unwrap();
        fs::write(save_dir.join("desktop.ini"), b"cache").unwrap();

        let archive_path = zip_directory_for_upload(&save_dir, &default_python_ignore()).unwrap();
        let members = read_zip_members(&archive_path);
        let _ = fs::remove_file(&archive_path);

        assert!(members.contains("ULUS12345/DATA.BIN"));
        assert!(members.contains("ULUS12345/ICON0.PNG"));
        assert!(!members.contains("ULUS12345/Thumbs.db"));
        assert!(!members.contains("ULUS12345/desktop.ini"));
    }

    /// Ports `test_zip_native_save_dirs_skips_unreadable_directory`
    /// (`test_cloud_transfer.py:224`), using a chmod-0 directory (this
    /// process runs unprivileged) rather than Python's `rglob` monkeypatch
    /// to make the directory's listing genuinely fail.
    #[test]
    #[cfg(unix)]
    fn zip_native_save_dirs_skips_unreadable_directory() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = NATIVE_SAVE_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let dir_a = temp.path().join("DirA");
        let dir_b = temp.path().join("DirB");
        fs::create_dir(&dir_a).unwrap();
        fs::create_dir(&dir_b).unwrap();
        fs::write(dir_a.join("save.dat"), b"save").unwrap();
        fs::set_permissions(&dir_b, fs::Permissions::from_mode(0o000)).unwrap();

        let dirs = vec![
            ("%APPDATA%\\DirB".to_string(), dir_b.clone()),
            ("%APPDATA%\\DirA".to_string(), dir_a.clone()),
        ];

        let result = zip_native_save_dirs_for_upload(&dirs, &IgnoreSets::default());

        // Restore permissions before any panicking assertion so the temp
        // dir can always be cleaned up.
        fs::set_permissions(&dir_b, fs::Permissions::from_mode(0o755)).unwrap();

        let (archive_path, total_files) = result.unwrap();
        assert_eq!(total_files, 1);
        let members = read_zip_members(&archive_path);
        let _ = fs::remove_file(&archive_path);

        assert!(members.contains("1/save.dat"));
        assert!(!members.iter().any(|m| m.starts_with("0/")));
    }

    /// Ports `test_zip_native_save_dirs_skips_locked_file`
    /// (`test_cloud_transfer.py:256`). Rust has no `zipfile.ZipFile.write`
    /// to monkeypatch, so per the task brief this simulates an unreadable
    /// file by putting a directory where a file is expected: the writer's
    /// `candidate.is_file()` gate (mirroring Python's own) then excludes
    /// it exactly as an unreadable file would be excluded by the
    /// (also-present) per-file write-error tolerance.
    #[test]
    fn zip_native_save_dirs_skips_locked_file() {
        let _guard = NATIVE_SAVE_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let save_dir = temp.path().join("DirA");
        fs::create_dir(&save_dir).unwrap();
        fs::create_dir(save_dir.join("locked.sav")).unwrap(); // stands in for "unreadable"
        fs::write(save_dir.join("good.sav"), b"good").unwrap();

        let dirs = vec![("%APPDATA%\\DirA".to_string(), save_dir)];
        let (archive_path, total_files) =
            zip_native_save_dirs_for_upload(&dirs, &IgnoreSets::default()).unwrap();

        assert_eq!(total_files, 1);
        let members = read_zip_members(&archive_path);
        let _ = fs::remove_file(&archive_path);

        assert!(members.contains("0/good.sav"));
        assert!(!members.contains("0/locked.sav"));
    }

    /// Ports
    /// `test_zip_native_save_dirs_all_dirs_fail_returns_zero_files_and_empty_manifest`
    /// (`test_cloud_transfer.py:288`).
    #[test]
    #[cfg(unix)]
    fn zip_native_save_dirs_all_fail_returns_zero_files_and_empty_manifest() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = NATIVE_SAVE_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let dir_a = temp.path().join("DirA");
        fs::create_dir(&dir_a).unwrap();
        fs::set_permissions(&dir_a, fs::Permissions::from_mode(0o000)).unwrap();

        let dirs = vec![("%APPDATA%\\DirA".to_string(), dir_a.clone())];
        let result = zip_native_save_dirs_for_upload(&dirs, &IgnoreSets::default());

        fs::set_permissions(&dir_a, fs::Permissions::from_mode(0o755)).unwrap();

        let (archive_path, total_files) = result.unwrap();
        assert_eq!(total_files, 0);

        let file = fs::File::open(&archive_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut manifest_entry = archive.by_name("_grid_launcher_dirs.json").unwrap();
        let mut manifest_bytes = Vec::new();
        manifest_entry.read_to_end(&mut manifest_bytes).unwrap();
        drop(manifest_entry);
        let _ = fs::remove_file(&archive_path);

        let parsed: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(parsed, serde_json::json!({}));
    }

    // --- brief-listed tests ---------------------------------------------

    #[test]
    fn grouped_archive_members_are_relative_to_the_common_parent() {
        let temp = tempfile::tempdir().unwrap();
        let sub_a = temp.path().join("a");
        let sub_b = temp.path().join("b");
        fs::create_dir(&sub_a).unwrap();
        fs::create_dir(&sub_b).unwrap();
        let file_a = sub_a.join("one.sav");
        let file_b = sub_b.join("two.sav");
        fs::write(&file_a, b"one").unwrap();
        fs::write(&file_b, b"two").unwrap();

        let archive_path = zip_grouped_files_for_upload(&[file_a, file_b], "Grouped Game").unwrap();
        let members = read_zip_members(&archive_path);
        let _ = fs::remove_file(&archive_path);

        assert!(members.contains("a/one.sav"));
        assert!(members.contains("b/two.sav"));
    }

    #[test]
    fn grouped_archive_single_file_member_is_its_own_bare_name() {
        // A single file's common "parent of parents" is its own parent
        // directory, so `strip_prefix` always succeeds and the member name
        // is the bare file name — the same observable result Python's
        // `except ValueError: relative_path = Path(file_path.name)`
        // fallback would produce, but reached without ever needing that
        // branch. That branch (and this port's equivalent `Err` arm in
        // `write_grouped_archive`) is unreachable for absolute POSIX
        // paths, which always share the `/` root — `common_parent` can
        // never return a directory that isn't a real ancestor of every
        // input file on this platform, matching Python's own
        // `os.path.commonpath` on POSIX.
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("solo.sav");
        fs::write(&file, b"solo").unwrap();

        let archive_path = zip_grouped_files_for_upload(&[file], "Solo Game").unwrap();
        let members = read_zip_members(&archive_path);
        let _ = fs::remove_file(&archive_path);

        assert!(members.contains("solo.sav"));
    }

    #[test]
    fn directory_archive_prefixes_the_dirname() {
        let temp = tempfile::tempdir().unwrap();
        let save_dir = temp.path().join("MySaveDir");
        fs::create_dir(&save_dir).unwrap();
        let nested = save_dir.join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(save_dir.join("top.sav"), b"top").unwrap();
        fs::write(nested.join("deep.sav"), b"deep").unwrap();

        let archive_path = zip_directory_for_upload(&save_dir, &IgnoreSets::default()).unwrap();
        let members = read_zip_members(&archive_path);
        let _ = fs::remove_file(&archive_path);

        assert!(members.contains("MySaveDir/top.sav"));
        assert!(members.contains("MySaveDir/nested/deep.sav"));
    }

    #[test]
    fn temp_archive_name_shape_and_collision_suffix() {
        let timestamp = chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string()
            .replace(':', "-");
        let expected_first = std::env::temp_dir().join(format!("My_Game-{timestamp}.zip"));
        fs::write(&expected_first, b"").unwrap();

        let result = temp_archive_path("My:Game");

        let file_name = result.file_name().unwrap().to_str().unwrap().to_string();
        let _ = fs::remove_file(&expected_first);
        let _ = fs::remove_file(&result);

        assert!(
            file_name.starts_with(&format!("My_Game-{timestamp}-")),
            "unexpected shape: {file_name}"
        );
        assert!(file_name.ends_with(".zip"));
        assert_ne!(result, expected_first);
    }

    #[test]
    fn extract_skips_zip_slip_members_and_blocked_names() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("dest");
        fs::create_dir(&dest).unwrap();

        let payload = build_zip_bytes(&[
            ("good.sav", b"good" as &[u8]),
            ("../evil.sav", b"pwned"),
            ("/etc/evil.sav", b"pwned"),
            ("thumbs.db", b"blocked"),
            ("sub/../../escape.sav", b"pwned"),
        ]);

        let ignore = ignore_with(&["thumbs.db"], &[]);
        let extracted = extract_payload_zip(&payload, &dest, &ignore).unwrap();

        assert_eq!(extracted, 1);
        assert!(dest.join("good.sav").is_file());
        assert!(!dest.join("thumbs.db").exists());
        assert!(!temp.path().join("evil.sav").exists());
        assert!(!temp.path().join("escape.sav").exists());
    }

    /// Fix-round regression: a *lexical* `dest_root.join(relative)` +
    /// `starts_with` check does not notice a pre-existing symlink under
    /// `dest` pointing outside it — a symlinked save directory, common
    /// under Flatpak/portable installs. `resolve_under_root` must resolve
    /// each existing path component (following the symlink) before the
    /// containment check runs.
    #[test]
    #[cfg(unix)]
    fn extract_rejects_zip_slip_through_a_pre_existing_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dest.join("link")).unwrap();

        let payload = build_zip_bytes(&[("link/escape.txt", b"pwned" as &[u8])]);
        let extracted = extract_payload_zip(&payload, &dest, &IgnoreSets::default()).unwrap();

        assert_eq!(extracted, 0);
        assert!(!outside.path().join("escape.txt").exists());
    }

    #[test]
    fn extract_writes_nested_members_and_counts_them() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("dest");
        fs::create_dir(&dest).unwrap();

        let payload = build_zip_bytes(&[
            ("a.sav", b"a" as &[u8]),
            ("nested/b.sav", b"b"),
            ("nested/deeper/c.sav", b"c"),
        ]);

        let extracted = extract_payload_zip(&payload, &dest, &IgnoreSets::default()).unwrap();

        assert_eq!(extracted, 3);
        assert_eq!(fs::read(dest.join("a.sav")).unwrap(), b"a");
        assert_eq!(fs::read(dest.join("nested/b.sav")).unwrap(), b"b");
        assert_eq!(fs::read(dest.join("nested/deeper/c.sav")).unwrap(), b"c");
    }

    /// Fix-round regression: Python's `PurePath(...).parts` collapses
    /// runs of repeated separators (`PurePosixPath("a//b").parts == ("a",
    /// "b")`), so `a//b.sav` is a perfectly safe, accepted member name —
    /// it must not be rejected as though it contained an empty `""`
    /// component.
    #[test]
    fn extract_accepts_member_names_with_repeated_separators() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("dest");
        fs::create_dir(&dest).unwrap();

        let payload = build_zip_bytes(&[("a//b.sav", b"ok" as &[u8])]);
        let extracted = extract_payload_zip(&payload, &dest, &IgnoreSets::default()).unwrap();

        assert_eq!(extracted, 1);
        assert_eq!(fs::read(dest.join("a/b.sav")).unwrap(), b"ok");
    }

    #[test]
    fn payload_is_zip_sniffs_magic_only() {
        assert!(!payload_is_zip(b""));
        assert!(!payload_is_zip(b"P"));
        assert!(!payload_is_zip(b"not-a-zip-at-all"));
        assert!(payload_is_zip(b"PK\x03\x04garbage-after-the-magic-is-fine"));
        // Magic-only: this is NOT a well-formed zip (no valid central
        // directory), yet still sniffs true — the deliberate divergence
        // from Python's `zipfile.is_zipfile` documented on the function.
        assert!(payload_is_zip(b"PK not really a zip"));
    }

    // --- cleanup_temp_archives -------------------------------------------

    #[test]
    fn cleanup_temp_archives_removes_existing_and_ignores_missing() {
        let temp = tempfile::tempdir().unwrap();
        let present = temp.path().join("present.zip");
        let missing = temp.path().join("missing.zip");
        fs::write(&present, b"x").unwrap();

        cleanup_temp_archives(&[present.clone(), missing.clone()]);

        assert!(!present.exists());
        assert!(!missing.exists());
    }
}
