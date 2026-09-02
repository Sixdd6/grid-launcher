//! Cloud save sync engine: shared types used across the `cloud` module.
//!
//! Ported from `grid_launcher/library/{identity,cloud_sync}.py` and the
//! other `grid_launcher/library/cloud_*.py` modules (see
//! `docs/porting/06-cloud-saves.md`).

pub mod archive;
pub mod candidates;
pub mod native;
pub mod restore;
pub mod retention;
pub mod scope;
pub mod state;
pub mod tokens;
pub mod transfer;
pub mod window;

use std::collections::BTreeSet;
use std::path::Path;

/// A kind of cloud-synced save data. `cloud_sync.py` and friends represent
/// this as the plain strings `"save"` / `"state"`; the port uses an enum so
/// call sites are exhaustively checked, with `as_str()` for the two spots
/// that still need the wire string (debug segments, RomM endpoints).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SaveType {
    Save,
    State,
}

impl SaveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaveType::Save => "save",
            SaveType::State => "state",
        }
    }
}

/// Plain-data view of a game for cloud logic. Built from an InstalledGame
/// or a server GameSummary; fields the source lacks stay `""`.
///
/// The three id fields below (`title_id`, `base_title_id`, `ps3_game_id`)
/// are a recorded data-availability gap: Python fills them during PS3/Wii-U
/// archive preparation, which the rewrite has not ported as of this
/// milestone. With them blank, the RPCS3/Cemu scanners run with empty token
/// sets, which — per the reference — accept everything: degraded matching,
/// not a crash.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloudGame {
    pub title: String,
    pub platform: String,
    pub rom_id: String, // string form, "" when absent (Python parity)
    pub rom_file_name: String,
    pub extracted_path: String,
    pub archive_path: String,
    pub description: String,
    pub title_id: String,      // data-availability gap: the rewrite's
    pub base_title_id: String, // registry does not carry these three yet;
    pub ps3_game_id: String,   // token logic ports fully, wiring passes ""
}

/// Case-insensitive block lists for save-directory scanning, in
/// [`latest_mtime_under`] and the directory filters in `cloud::window`:
/// file basenames and extensions (leading dot, e.g. `".tmp"`) to skip.
/// Mirrors the `ignore_basenames`/`ignore_extensions` parameters
/// `cloud_mixin.py:1490-1532`'s `_latest_file_mtime_under_path` takes,
/// pre-casefolded there via a set comprehension before use.
///
/// Members are expected to already be lowercased by the caller — `blocks`
/// lowercases only the path it is checking, not `self`'s sets, so an
/// uppercase member here would simply never match (parity: Python builds
/// its casefolded sets once per call from whatever raw casing was
/// configured, then compares against `.casefold()`'d path parts — this
/// type just moves that casefolding to construction time).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IgnoreSets {
    pub basenames: BTreeSet<String>,
    pub extensions: BTreeSet<String>,
}

impl IgnoreSets {
    /// True when `path`'s file name, lowercased, is in `basenames`, OR its
    /// extension (`Path::extension()` with the leading dot restored,
    /// lowercased — `""` when the path has none) is in `extensions`. Either
    /// match blocks; matches `cloud_mixin.py:1509-1513`/`1524-1526`'s pair
    /// of independent `name.casefold() in blocked_basenames` /
    /// `suffix.casefold() in blocked_extensions` checks. Like Python's
    /// `Path.suffix`, this is the LAST dotted component only — for
    /// `"archive.tar.gz"` the extension is `".gz"`, not `".tar.gz"`.
    pub fn blocks(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if self.basenames.contains(&name.to_lowercase()) {
                return true;
            }
        }
        let extension = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => format!(".{}", e.to_lowercase()),
            None => String::new(),
        };
        self.extensions.contains(&extension)
    }
}

/// The mtime (unix seconds) of `path` itself when it can be stat'd,
/// `None` on any failure (missing file, permission error, non-UTF8-clean
/// clock, ...) — the shared "stat failure skips" primitive used by
/// `latest_mtime_under` and every mtime filter in `cloud::window`.
fn file_mtime_secs(path: &Path) -> Option<f64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let elapsed = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Some(elapsed.as_secs_f64())
}

/// The most recent mtime (unix seconds) of any non-blocked file at or
/// under `dir`, `0.0` when `dir` doesn't exist or nothing qualifies.
/// Mirrors `cloud_mixin.py:1490-1532`'s `_latest_file_mtime_under_path`:
/// if `dir` is itself a file, it is checked directly (blocked → `0.0`,
/// else its own mtime); if it's a directory, walked recursively via plain
/// `std::fs::read_dir`, comparing only files, skipping blocked ones and any
/// entry that can't be listed or stat'd — a walk/stat failure on one entry
/// is never fatal to the rest.
///
/// The walk descends into a subdirectory only when `DirEntry::file_type`
/// (which does NOT follow symlinks) itself reports `is_dir()` — a
/// SYMLINKED directory is never recursed into. This matches Python's
/// `Path.rglob("*")`, which likewise does not descend into symlinked
/// subdirectories (avoiding both double-counted files reachable two ways
/// and unbounded recursion on a symlink cycle). A symlink is still
/// evaluated as a candidate FILE exactly like any other directory entry —
/// `Path::is_file()` follows the symlink, so a symlink to a regular file is
/// picked up (its target's mtime), while a symlink to a directory fails
/// `is_file()` and is simply skipped, matching Python's
/// `candidate.is_file()` gate ahead of `.stat()`.
pub fn latest_mtime_under(dir: &Path, ignore: &IgnoreSets) -> f64 {
    if !dir.exists() {
        return 0.0;
    }
    if dir.is_file() {
        if ignore.blocks(dir) {
            return 0.0;
        }
        return file_mtime_secs(dir).unwrap_or(0.0);
    }
    if !dir.is_dir() {
        return 0.0;
    }

    let mut latest = 0.0_f64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // A real (non-symlink) directory: descend. Anything else —
            // a real file OR any symlink, whatever it targets — falls
            // through to the file check below instead of being recursed
            // into.
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                stack.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            if ignore.blocks(&path) {
                continue;
            }
            if let Some(mtime) = file_mtime_secs(&path) {
                latest = latest.max(mtime);
            }
        }
    }
    latest
}
