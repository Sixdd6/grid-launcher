//! Settings › Library › "Move existing files" (phase 9b): moves the
//! library's `games/`, `emulators/` and `saves/` folders from the current
//! root to a new folder, then rewrites every stored path.
//!
//! The job runs in three phases, recorded in a journal beside the config
//! (`library-move.json`, written atomically on every state change, never
//! per byte), so a crash or a close resumes where it stopped:
//!
//! 1. **Copy** (cancellable). Items on another volume are copied into
//!    `<new>/.grid-library-move/<name>` and verified: every file's size,
//!    plus a full SHA-256 for `saves/` and a sampled one (first and last
//!    1 MiB) for `games/` and `emulators/`. The sources are not touched.
//!    Items on the same volume do nothing here: they are renamed in
//!    Commit, so the old library stays complete until then and a cancel
//!    only deletes the staging folder.
//! 2. **Commit** (not cancellable; every step idempotent, so a resume
//!    re-runs all of it): place the items (staging → `<new>/<name>`, or
//!    a rename old → new); re-create the links the copy recorded and
//!    repoint absolute links in renamed items; repair the catalog
//!    user-data links; rewrite the emulator config files that store
//!    absolute library paths (the only repair for the add-only ones);
//!    rewrite the registry; one config edit that relocates the emulator
//!    entries and switches the root (the atomic switch point); the
//!    managed-key resync the app injects; a scan for values still under
//!    the old root.
//! 3. **Cleanup**. For each copied item the source is compared with the
//!    copy again (every source file must have a copy of the same length
//!    and modified time); only a match deletes the source. The old root
//!    goes only when it is empty. The journal is deleted last.
//!
//! The pure parts — the [`Relocator`] matcher, the plan and its
//! [`MoveRefusal`]s, the [`Journal`] and the [`LibraryGate`] — carry no
//! state of their own. The executor talks to the config only through the
//! injected [`MoveHooks::edit_config`], so the app layer keeps its single
//! config writer. Nothing here imports Tauri; journal, events and errors
//! carry paths only, never a secret.

use std::collections::HashSet;
use std::fs::{self, File, FileTimes};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::layout_migration::{emulator_config_files, rewrite_paths_in_text_with};
use super::path_change::{self, LibraryActivity, PathChangeRefusal};
use super::paths::{saves_dir, EMULATORS_DIR, GAMES_DIR, LAYOUT_VERSION_CURRENT, SAVES_DIR};
use super::registry::{InstalledGame, Registry};
use super::user_data_links::{ensure_user_data_links, is_link, user_data_root};
use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::{profile_for_entry, EmulatorProfile};

/// The journal's file name, beside `config.toml` and `grid-launcher.db`.
pub const JOURNAL_FILE_NAME: &str = "library-move.json";
/// The journal format this build reads and writes. A journal with any
/// other version is refused rather than guessed at.
pub const JOURNAL_VERSION: u64 = 1;
/// The staging folder under the new root that copied items wait in until
/// Commit renames them into place.
pub const STAGING_DIR: &str = ".grid-library-move";
/// The three top-level folders a move takes along. Everything else at the
/// old root (unrelated files, dot-folders) stays where it is.
pub const MOVED_DIRS: [&str; 3] = [GAMES_DIR, EMULATORS_DIR, SAVES_DIR];
/// The reason a [`LibraryGate`] closed by a library move gives.
pub const LIBRARY_MOVE_BUSY: &str =
    "The library is being moved. Wait until the move finishes, then try again.";
/// With the `e2e` feature, `1` here makes [`probe_strategy`] answer Copy,
/// so the end-to-end suite exercises the copy path on one disk.
#[cfg(feature = "e2e")]
pub const FORCE_COPY_ENV: &str = "GRID_LAUNCHER_E2E_FORCE_COPY_MOVE";

const GIB: u64 = 1 << 30;
/// The copy buffer, and the step between cancel checks and progress.
const CHUNK: usize = 1 << 20;
/// How much of each end of a file the sampled hash reads.
const SAMPLE: u64 = 1 << 20;
/// Two modified times closer than this are the same time: FAT and exFAT
/// store them in 2-second steps.
const MTIME_TOLERANCE: Duration = Duration::from_secs(2);

/// The journal for the config at `config_path`: `library-move.json` in the
/// same directory (so `GRID_LAUNCHER_DATA_DIR` moves it too).
pub fn journal_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(JOURNAL_FILE_NAME)
}

// --- journal ----------------------------------------------------------------

/// How one item reaches the new root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Same volume: one rename in Commit.
    Rename,
    /// Another volume: copied and verified in Copy, renamed out of staging
    /// in Commit, the source deleted in Cleanup.
    Copy,
}

/// Where the job is. Only Copy can be cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Copy,
    Commit,
    Cleanup,
}

/// One item's progress through the phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Nothing done yet.
    Pending,
    /// Copied into staging and verified.
    Copied,
    /// At `<new>/<name>`.
    Placed,
    /// The source is gone (deleted after the recheck, or renamed).
    Cleaned,
    /// The source was left in place; the reason is in [`Journal::report`].
    Kept,
}

/// One moved top-level folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalItem {
    /// `games`, `emulators` or `saves`.
    pub name: String,
    pub strategy: Strategy,
    pub state: ItemState,
    /// Bytes on disk at plan time (links count as nothing).
    pub bytes: u64,
    /// Regular files at plan time.
    pub files: u64,
    /// Set by the post-commit scan when a stored value still points into
    /// this item at the old root: Cleanup then keeps the source.
    #[serde(default)]
    pub keep_source: bool,
    /// SHA-256 over the source tree as Copy found it (every entry's path,
    /// length and modified time), taken before the copy. Cleanup deletes the
    /// source only while it still matches. Blank for a renamed item.
    #[serde(default)]
    pub source_digest: String,
}

/// A link the copy did not copy as itself: an absolute link (any link on
/// Windows, where a junction is always absolute) or a relative one that
/// points outside the moved folders. Commit re-creates it at the new root,
/// pointing at the relocated target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedLink {
    /// The link's path relative to the library root, `/`-separated.
    pub path: String,
    /// Where it pointed, as an absolute path.
    pub target: String,
    /// A directory link (a junction on Windows) rather than a file link.
    pub dir: bool,
}

/// The persistent record of one move. No secrets: paths and states only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub version: u64,
    /// The old root as the config typed it.
    pub old_root: String,
    /// The new root as the config will store it.
    pub new_root: String,
    /// Both roots as the filesystem resolves them (links followed), taken
    /// when the move began. Files that stored a resolved path (RPCS3,
    /// PCSX2) are matched through these. Blank when it could not be read.
    pub old_canonical: String,
    pub new_canonical: String,
    /// GRID created the new root, so a cancel removes it again (when it is
    /// empty).
    pub created_new_root: bool,
    pub phase: Phase,
    pub items: Vec<JournalItem>,
    #[serde(default)]
    pub links: Vec<RecordedLink>,
    /// How many Commit steps have finished. Informational: a resume
    /// re-runs the whole Commit, every step of which is idempotent.
    #[serde(default)]
    pub commit_steps: u32,
    /// One line per thing the user should know (a kept source, a link that
    /// could not be made). Shown when the move finishes.
    #[serde(default)]
    pub report: Vec<String>,
}

impl Journal {
    /// The journal at `path`, `None` when there is none. A journal with an
    /// unknown version is [`MoveError::UnknownJournalVersion`]; anything
    /// unreadable is [`MoveError::BadJournal`].
    pub fn load(path: &Path) -> Result<Option<Journal>, MoveError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error("could not read", path, e)),
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| MoveError::BadJournal(e.to_string()))?;
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| MoveError::BadJournal("it has no version".to_string()))?;
        if version != JOURNAL_VERSION {
            return Err(MoveError::UnknownJournalVersion(version));
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| MoveError::BadJournal(e.to_string()))
    }

    /// Atomic and durable: `<path>.tmp`, fsync, rename over the target.
    pub fn save(&self, path: &Path) -> Result<(), MoveError> {
        let parent = path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| io_error("could not create", parent, e))?;
        let tmp = path.with_extension("json.tmp");
        let text =
            serde_json::to_string_pretty(self).map_err(|e| MoveError::BadJournal(e.to_string()))?;
        let write = || -> io::Result<()> {
            let mut file = File::create(&tmp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp, path)?;
            sync_dir(parent);
            Ok(())
        };
        write().map_err(|e| io_error("could not write", path, e))
    }

    /// Removes the journal. A missing one is fine.
    pub fn delete(path: &Path) -> Result<(), MoveError> {
        match fs::remove_file(path) {
            Ok(()) => {
                sync_dir(path.parent().unwrap_or(Path::new(".")));
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error("could not delete", path, e)),
        }
    }

    /// Adds `line` to the report once.
    fn note(&mut self, line: String) {
        if !self.report.contains(&line) {
            self.report.push(line);
        }
    }
}

/// Makes a rename or delete inside `dir` durable. Best effort, and a no-op
/// off unix (NTFS journals its metadata).
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

// --- errors -----------------------------------------------------------------

/// Why a move cannot start. Serialized as a snake_case tag, so the frontend
/// can branch on it; [`Self::message`] is the user-facing text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveRefusal {
    /// The library is not in the current layout (the startup migration has
    /// not finished).
    LayoutNotCurrent,
    /// The current library folder is not there.
    OldRootMissing,
    /// The new folder already has `games`, `emulators`, `saves` or a
    /// staging folder.
    TargetHasLibraryFolders,
    /// The copied items plus the margin do not fit.
    NotEnoughSpace,
    /// A move is already running or waits to resume.
    MoveActive,
    /// One of the phase 9a path checks.
    PathChange(PathChangeRefusal),
}

impl MoveRefusal {
    /// The sentence the Library pane shows.
    pub fn message(self) -> &'static str {
        match self {
            Self::LayoutNotCurrent => {
                "The library is still being updated. Restart GRID, then try again."
            }
            Self::OldRootMissing => "The current library folder cannot be found.",
            Self::TargetHasLibraryFolders => {
                "The new folder already has a games, emulators or saves folder. Choose another folder."
            }
            Self::NotEnoughSpace => "There is not enough free space in the new folder.",
            Self::MoveActive => "A library move is already running.",
            Self::PathChange(refusal) => refusal.message(),
        }
    }
}

/// Why a move step failed. The texts name paths only.
#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("{}", .0.message())]
    Refused(MoveRefusal),
    #[error("{0}")]
    Io(String),
    #[error("The library move record has version {0}, which this version of GRID cannot read.")]
    UnknownJournalVersion(u64),
    #[error("The library move record cannot be read: {0}")]
    BadJournal(String),
    #[error("No library move is in progress.")]
    NoJournal,
    #[error("The library move was cancelled.")]
    Cancelled,
    #[error("The copy of {0} does not match the original.")]
    VerifyFailed(String),
    #[error("The folder {0} cannot be reached.")]
    RootUnreachable(String),
    #[error("The move can no longer be cancelled: the files have already moved.")]
    NotCancellable,
    #[error("{0} is a special file that GRID cannot copy.")]
    Unsupported(String),
    /// A test failpoint stopped the run right after a journal write, as a
    /// crash would.
    #[error("The library move was interrupted.")]
    Interrupted,
}

fn io_error(action: &str, path: &Path, error: io::Error) -> MoveError {
    MoveError::Io(format!("{action} {}: {error}", path.display()))
}

fn refused(refusal: MoveRefusal) -> MoveError {
    MoveError::Refused(refusal)
}

// --- relocate matcher -------------------------------------------------------

/// Whether `c` separates path components in either spelling.
fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Rewrites stored paths from the old root to the new one.
///
/// A value matches when it starts with a spelling of the old root, then
/// one or more separators, then `games`, `emulators` or `saves`, then a
/// separator or the end. The spelling is replaced by the paired spelling of
/// the new root; the rest of the value is kept byte for byte. Spellings
/// come in `(old, new)` pairs — as typed, `~`-expanded, canonical — and
/// each pair is also tried with `/` and `\` swapped both ways and with `\`
/// escaped as `\\`. On Windows the comparison ignores case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relocator {
    /// `(old spelling, new spelling)`, longest old spelling first.
    pairs: Vec<(String, String)>,
}

impl Relocator {
    pub fn new(spellings: &[(String, String)]) -> Self {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (old, new) in spellings {
            let (old, new) = (old.trim(), new.trim());
            if old.is_empty() || new.is_empty() {
                continue;
            }
            let backslashed = |text: &str| text.replace('/', "\\");
            let variants = [
                (old.to_string(), new.to_string()),
                (old.replace('\\', "/"), new.replace('\\', "/")),
                (backslashed(old), backslashed(new)),
                (old.replace('\\', "\\\\"), new.replace('\\', "\\\\")),
                (
                    backslashed(old).replace('\\', "\\\\"),
                    backslashed(new).replace('\\', "\\\\"),
                ),
            ];
            for (old_form, new_form) in variants {
                let old_form = old_form.trim_end_matches(is_separator).to_string();
                let new_form = new_form.trim_end_matches(is_separator).to_string();
                if old_form.is_empty() || new_form.is_empty() {
                    continue;
                }
                if pairs
                    .iter()
                    .any(|(existing, _)| same_spelling(existing, &old_form))
                {
                    continue;
                }
                pairs.push((old_form, new_form));
            }
        }
        // Stable: equal lengths keep their order, so the typed spelling
        // wins over a later one that differs only in case.
        pairs.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
        Self { pairs }
    }

    /// The spellings a journal records: typed, `~`-expanded and canonical.
    pub fn for_journal(journal: &Journal) -> Self {
        let old_expanded = expand_root(&journal.old_root)
            .to_string_lossy()
            .into_owned();
        Self::new(&[
            (journal.old_root.clone(), journal.new_root.clone()),
            (old_expanded, journal.new_root.clone()),
            (journal.old_canonical.clone(), journal.new_canonical.clone()),
        ])
    }

    /// `value` under the new root, or `None` when it is not under one of
    /// the moved folders at the old root.
    pub fn relocate(&self, value: &str) -> Option<String> {
        let (index, after, _) = self.match_root(value)?;
        Some(format!("{}{after}", self.pairs[index].1))
    }

    /// Which moved folder `value` lies in at the old root, if any.
    pub fn moved_dir_of(&self, value: &str) -> Option<&'static str> {
        self.match_root(value).map(|(_, _, name)| name)
    }

    /// Every old-root path inside a text file relocated, every other byte
    /// kept; `None` when nothing changed. Same token rules as the layout
    /// migration's rewrite (quotes and line ends end a path).
    pub fn rewrite_text(&self, text: &str) -> Option<String> {
        let starts = |rest: &str| {
            self.pairs
                .iter()
                .any(|(old, _)| strip_spelling(rest, old).is_some())
        };
        rewrite_paths_in_text_with(text, &starts, &|token| self.relocate(token))
    }

    fn match_root<'v>(&self, value: &'v str) -> Option<(usize, &'v str, &'static str)> {
        for (index, (old, _)) in self.pairs.iter().enumerate() {
            let Some(after) = strip_spelling(value, old) else {
                continue;
            };
            if let Some(name) = moved_component(after) {
                return Some((index, after, name));
            }
        }
        None
    }
}

/// The moved folder `after` (the rest of a value after the root spelling)
/// starts with: one or more separators, a moved name, then a separator or
/// the end.
fn moved_component(after: &str) -> Option<&'static str> {
    let rest = after.trim_start_matches(is_separator);
    if rest.len() == after.len() {
        return None;
    }
    let end = rest.find(is_separator).unwrap_or(rest.len());
    let name = &rest[..end];
    MOVED_DIRS.into_iter().find(|dir| names_equal(dir, name))
}

fn names_equal(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

fn chars_equal(a: char, b: char) -> bool {
    a == b || (cfg!(windows) && a.to_lowercase().eq(b.to_lowercase()))
}

fn same_spelling(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count() && strip_spelling(a, b).is_some()
}

/// `value` without its leading `spelling` (case-insensitive on Windows;
/// separators must match exactly, each separator form is its own pair).
fn strip_spelling<'v>(value: &'v str, spelling: &str) -> Option<&'v str> {
    let mut offset = 0;
    let mut chars = value.chars();
    for expected in spelling.chars() {
        let actual = chars.next()?;
        if !chars_equal(expected, actual) {
            return None;
        }
        offset += actual.len_utf8();
    }
    Some(&value[offset..])
}

/// `raw` trimmed, with a leading `~/` (or `~\` on Windows) joined onto the
/// home directory: the same expansion the 9a path checks use.
fn expand_root(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("~/")
        .or_else(|| cfg!(windows).then(|| trimmed.strip_prefix("~\\")).flatten());
    match (rest, crate::platform::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(trimmed),
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

// --- plan -------------------------------------------------------------------

/// The free space a copy needs beyond its own bytes: 1 GiB or 2 % of the
/// copy, whichever is more.
pub fn space_margin(copy_bytes: u64) -> u64 {
    GIB.max(copy_bytes / 50)
}

/// The bytes a copy of `copy_bytes` needs free, or
/// [`MoveRefusal::NotEnoughSpace`]. Nothing to copy needs nothing. An
/// unknown free space (`None`) is let through: the copy then fails on a
/// full disk, before anything is committed.
pub fn check_space(copy_bytes: u64, free: Option<u64>) -> Result<u64, MoveRefusal> {
    if copy_bytes == 0 {
        return Ok(0);
    }
    let required = copy_bytes.saturating_add(space_margin(copy_bytes));
    match free {
        Some(free) if free < required => Err(MoveRefusal::NotEnoughSpace),
        _ => Ok(required),
    }
}

/// Decides an item's [`Strategy`]: `(directory the item lives on, the
/// nearest existing directory of the new root)`.
pub type StrategyProbe = dyn Fn(&Path, &Path) -> io::Result<Strategy> + Send + Sync;
/// Free bytes for the volume holding a path.
pub type FreeSpace = dyn Fn(&Path) -> Option<u64> + Send + Sync;

/// The system probes; the tests inject their own.
pub struct PlanEnv<'a> {
    pub probe: &'a StrategyProbe,
    pub free_space: &'a FreeSpace,
}

static SYSTEM_PROBE: fn(&Path, &Path) -> io::Result<Strategy> = probe_strategy;
static SYSTEM_FREE_SPACE: fn(&Path) -> Option<u64> = free_space;

impl PlanEnv<'static> {
    /// [`probe_strategy`] and [`free_space`].
    pub fn system() -> Self {
        Self {
            probe: &SYSTEM_PROBE,
            free_space: &SYSTEM_FREE_SPACE,
        }
    }
}

/// Renames an empty probe folder from `from_dir` into `to_dir`: success is
/// [`Strategy::Rename`], a cross-device error is [`Strategy::Copy`], any
/// other error is returned. Leaves nothing behind.
pub fn probe_strategy(from_dir: &Path, to_dir: &Path) -> io::Result<Strategy> {
    #[cfg(feature = "e2e")]
    if std::env::var(FORCE_COPY_ENV).is_ok_and(|value| value.trim() == "1") {
        return Ok(Strategy::Copy);
    }
    let name = format!(".grid-library-move-probe-{}", std::process::id());
    let source = from_dir.join(&name);
    let dest = to_dir.join(&name);
    fs::create_dir(&source)?;
    match fs::rename(&source, &dest) {
        Ok(()) => {
            fs::remove_dir(&dest)?;
            Ok(Strategy::Rename)
        }
        Err(error) => {
            let _ = fs::remove_dir(&source);
            if is_cross_device(&error) {
                Ok(Strategy::Copy)
            } else {
                Err(error)
            }
        }
    }
}

fn is_cross_device(error: &io::Error) -> bool {
    #[cfg(windows)]
    const RAW: i32 = 17; // ERROR_NOT_SAME_DEVICE
    #[cfg(unix)]
    const RAW: i32 = libc::EXDEV;
    #[cfg(not(any(unix, windows)))]
    const RAW: i32 = -1;
    error.kind() == io::ErrorKind::CrossesDevices || error.raw_os_error() == Some(RAW)
}

/// Free bytes available to this user on the volume holding `path` (its
/// nearest existing ancestor when it does not exist yet). `None` when it
/// cannot be read.
pub fn free_space(path: &Path) -> Option<u64> {
    let existing = path.ancestors().find(|dir| dir.is_dir())?;
    free_space_at(existing)
}

#[cfg(windows)]
fn free_space_at(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available: u64 = 0;
    // SAFETY: `wide` is NUL-terminated and outlives the call; the two
    // optional out-pointers may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

#[cfg(unix)]
// The statvfs field types differ between targets (u32 on some 32-bit
// ones), so the casts are needed there even where they are no-ops here.
#[allow(clippy::unnecessary_cast)]
fn free_space_at(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    // SAFETY: an all-zero `statvfs` is a valid value to be overwritten.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is NUL-terminated and `stat` is a valid out-pointer.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    (rc == 0).then(|| (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
}

#[cfg(not(any(unix, windows)))]
fn free_space_at(_dir: &Path) -> Option<u64> {
    None
}

/// One folder the move takes along.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewItem {
    pub name: String,
    pub strategy: Strategy,
    pub bytes: u64,
    pub files: u64,
}

/// What a move would do, computed before anything changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MovePreview {
    pub old_root: String,
    pub new_root: String,
    pub items: Vec<PreviewItem>,
    /// Every moved byte.
    pub total_bytes: u64,
    /// The bytes that are copied (items on another volume).
    pub copy_bytes: u64,
    /// `copy_bytes` plus the margin; 0 for a same-volume move.
    pub required_bytes: u64,
    /// Free bytes at the new root, when known.
    pub free_bytes: Option<u64>,
}

/// Plans a move of the library in `config` to `new_root` (as typed). Reads
/// only; creates nothing. Refusals, in order: a move already active
/// (`activity.library_move` or a journal on disk), the 9a checks
/// ([`path_change::validate_new_library_path`]), a layout that is not
/// current, a missing old root, a new folder that already has library
/// folders, not enough space for the copied items.
pub fn plan_move(
    config: &Config,
    new_root: &str,
    activity: LibraryActivity,
    journal_path: &Path,
    env: &PlanEnv,
) -> Result<MovePreview, MoveError> {
    if activity.library_move || fs::symlink_metadata(journal_path).is_ok() {
        return Err(refused(MoveRefusal::MoveActive));
    }
    let target = path_change::validate_new_library_path(&config.library_path, new_root, activity)
        .map_err(|r| refused(MoveRefusal::PathChange(r)))?;
    if config.library_layout_version != LAYOUT_VERSION_CURRENT {
        return Err(refused(MoveRefusal::LayoutNotCurrent));
    }
    let old = expand_root(&config.library_path);
    if config.library_path.trim().is_empty() || !old.is_dir() {
        return Err(refused(MoveRefusal::OldRootMissing));
    }
    if MOVED_DIRS
        .iter()
        .chain(std::iter::once(&STAGING_DIR))
        .any(|name| fs::symlink_metadata(target.join(name)).is_ok())
    {
        return Err(refused(MoveRefusal::TargetHasLibraryFolders));
    }
    let probe_to = target
        .ancestors()
        .find(|dir| dir.is_dir())
        .ok_or_else(|| refused(MoveRefusal::PathChange(PathChangeRefusal::NotCreatable)))?;

    let mut items = Vec::new();
    for name in MOVED_DIRS {
        let path = old.join(name);
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let link = is_link(&path);
        let from = if link || !meta.is_dir() {
            old.as_path()
        } else {
            path.as_path()
        };
        let strategy = (env.probe)(from, probe_to).map_err(|e| {
            MoveError::Io(format!(
                "could not test a move from {} to {}: {e}",
                from.display(),
                probe_to.display()
            ))
        })?;
        let (bytes, files) = if link { (0, 0) } else { tree_size(&path) };
        items.push(PreviewItem {
            name: name.to_string(),
            strategy,
            bytes,
            files,
        });
    }
    let total_bytes = items.iter().map(|item| item.bytes).sum();
    let copy_bytes = items
        .iter()
        .filter(|item| item.strategy == Strategy::Copy)
        .map(|item| item.bytes)
        .sum();
    let free_bytes = (env.free_space)(&target);
    let required_bytes = check_space(copy_bytes, free_bytes).map_err(refused)?;
    Ok(MovePreview {
        old_root: config.library_path.trim().to_string(),
        new_root: text(&target),
        items,
        total_bytes,
        copy_bytes,
        required_bytes,
        free_bytes,
    })
}

/// [`plan_move`], then creates the new root and writes the first journal
/// (phase Copy). The caller closes the [`LibraryGate`] and checks the
/// activity BEFORE this, and starts [`run`] after it.
pub fn start_move(
    config: &Config,
    new_root: &str,
    activity: LibraryActivity,
    journal_path: &Path,
    env: &PlanEnv,
) -> Result<Journal, MoveError> {
    let preview = plan_move(config, new_root, activity, journal_path, env)?;
    let new = PathBuf::from(&preview.new_root);
    let existed = fs::symlink_metadata(&new).is_ok();
    path_change::prepare_library_dir(&new).map_err(|r| refused(MoveRefusal::PathChange(r)))?;
    let canonical = |path: &Path| {
        crate::platform::canonicalize(path)
            .map(|resolved| text(&resolved))
            .unwrap_or_default()
    };
    let journal = Journal {
        version: JOURNAL_VERSION,
        old_canonical: canonical(&expand_root(&config.library_path)),
        new_canonical: canonical(&new),
        old_root: preview.old_root,
        new_root: preview.new_root,
        created_new_root: !existed,
        phase: Phase::Copy,
        items: preview
            .items
            .into_iter()
            .map(|item| JournalItem {
                name: item.name,
                strategy: item.strategy,
                state: ItemState::Pending,
                bytes: item.bytes,
                files: item.files,
                keep_source: false,
                source_digest: String::new(),
            })
            .collect(),
        links: Vec::new(),
        commit_steps: 0,
        report: Vec::new(),
    };
    journal.save(journal_path)?;
    Ok(journal)
}

/// `(bytes, regular files)` under `path`, links and reparse points neither
/// counted nor followed.
fn tree_size(path: &Path) -> (u64, u64) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return (0, 0);
    };
    if link_info(path, &meta).is_some() || is_reparse_point(&meta) {
        return (0, 0);
    }
    if meta.is_file() {
        return (meta.len(), 1);
    }
    if !meta.is_dir() {
        return (0, 0);
    }
    let Ok(entries) = fs::read_dir(path) else {
        return (0, 0);
    };
    entries
        .flatten()
        .map(|entry| tree_size(&entry.path()))
        .fold((0, 0), |(bytes, files), (b, f)| (bytes + b, files + f))
}

// --- links ------------------------------------------------------------------

/// A link's raw target and whether it is a directory link.
struct LinkInfo {
    raw: PathBuf,
    dir: bool,
}

#[cfg(unix)]
fn link_info(path: &Path, meta: &fs::Metadata) -> Option<LinkInfo> {
    if !meta.file_type().is_symlink() {
        return None;
    }
    let raw = fs::read_link(path).ok()?;
    let dir = fs::metadata(path).is_ok_and(|target| target.is_dir());
    Some(LinkInfo { raw, dir })
}

#[cfg(windows)]
fn link_info(path: &Path, meta: &fs::Metadata) -> Option<LinkInfo> {
    use std::os::windows::fs::FileTypeExt;
    if junction::exists(path).unwrap_or(false) {
        let raw = junction::get_target(path).ok()?;
        return Some(LinkInfo { raw, dir: true });
    }
    if meta.file_type().is_symlink() {
        let raw = fs::read_link(path).ok()?;
        return Some(LinkInfo {
            raw,
            dir: meta.file_type().is_symlink_dir(),
        });
    }
    None
}

#[cfg(not(any(unix, windows)))]
fn link_info(_path: &Path, _meta: &fs::Metadata) -> Option<LinkInfo> {
    None
}

/// Any NTFS reparse point. Always false off Windows.
#[cfg(windows)]
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &fs::Metadata) -> bool {
    false
}

/// `path` with `.` and `..` folded lexically.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The absolute target of a link at `link` whose raw target is `raw`.
fn absolute_target(link: &Path, raw: &Path) -> PathBuf {
    if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        normalize(&link.parent().unwrap_or(Path::new(".")).join(raw))
    }
}

/// Creates a link at `link` to the absolute `target`: a symlink on unix, a
/// junction (directory) or a file symlink on Windows.
fn create_link(target: &Path, link: &Path, dir: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        let _ = dir;
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if dir {
            junction::create(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link, dir);
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Removes the link at `path` itself, never what it points at.
#[cfg(windows)]
fn remove_any_link(path: &Path, meta: &fs::Metadata) -> io::Result<()> {
    use std::os::windows::fs::FileTypeExt;
    if junction::exists(path).unwrap_or(false) {
        return super::user_data_links::remove_link(path);
    }
    if meta.file_type().is_symlink_dir() {
        return fs::remove_dir(path);
    }
    fs::remove_file(path)
}

#[cfg(not(windows))]
fn remove_any_link(path: &Path, _meta: &fs::Metadata) -> io::Result<()> {
    fs::remove_file(path)
}

// --- the executor -----------------------------------------------------------

/// One change to the config, handed to a [`ConfigEditor`].
pub type ConfigChange<'c> = dyn FnMut(&mut Config) -> Result<(), String> + 'c;

/// Applies one change to the config through the app's single writer (load,
/// change, save as one step). An `Err` from the change aborts the save.
pub type ConfigEditor<'a> = dyn Fn(&mut ConfigChange) -> Result<(), String> + 'a;

/// A [`ConfigEditor`] over the file at `path` with no lock of its own: for
/// tests and single-writer callers. The app passes its `modify_config`.
pub fn config_file_editor(path: &Path) -> impl Fn(&mut ConfigChange) -> Result<(), String> + '_ {
    move |change: &mut ConfigChange| {
        let mut config = Config::load(path).map_err(|e| e.to_string())?;
        change(&mut config)?;
        config.save(path).map_err(|e| e.to_string())
    }
}

/// Copy progress for one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MoveProgress {
    pub phase: Phase,
    pub item: String,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

/// What the executor needs from its caller.
pub struct MoveHooks<'a> {
    /// The config file, read for the emulator entries. Writes go through
    /// [`Self::edit_config`].
    pub config_path: &'a Path,
    pub registry: &'a Registry,
    pub profiles: &'a [EmulatorProfile],
    pub edit_config: &'a ConfigEditor<'a>,
    /// The managed-key resync (`run_emulator_sync`, `fresh_install =
    /// false`), handed the switched config. A failure is reported, not
    /// fatal: the switch has happened.
    pub resync: &'a (dyn Fn(&Config) -> Result<(), String> + 'a),
    /// Rechecked before a Copy phase resumes.
    pub free_space: &'a (dyn Fn(&Path) -> Option<u64> + 'a),
    /// Set to cancel during Copy. Ignored once Commit starts.
    pub cancel: &'a AtomicBool,
    pub progress: &'a (dyn Fn(&MoveProgress) + 'a),
    /// Test failpoint: called after every journal write; `true` stops the
    /// run with [`MoveError::Interrupted`] as a crash would. `None` in the
    /// app.
    pub after_journal_write: Option<&'a (dyn Fn(&Journal) -> bool + 'a)>,
}

/// What a finished move did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MoveReport {
    pub new_root: String,
    /// Lines for the user: kept sources, links that could not be made,
    /// files that could not be updated.
    pub report: Vec<String>,
    /// The old root was empty and has been removed.
    pub old_root_removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveOutcome {
    Finished(MoveReport),
    /// Cancelled in Copy: the old library is exactly as it was.
    Cancelled,
}

/// What [`recover`] leaves for the background.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    /// No move was in progress.
    None,
    /// The Copy phase resumes: the OLD root is still the library.
    Copy(Journal),
    /// The move has committed; Cleanup resumes: the NEW root is the
    /// library.
    Cleanup(Journal),
}

fn write_journal(journal: &Journal, path: &Path, hooks: &MoveHooks) -> Result<(), MoveError> {
    journal.save(path)?;
    match hooks.after_journal_write {
        Some(failpoint) if failpoint(journal) => Err(MoveError::Interrupted),
        _ => Ok(()),
    }
}

/// Drives the move in `journal_path` from its current phase to the end. A
/// cancel during Copy rolls back and returns [`MoveOutcome::Cancelled`].
/// Any error leaves the journal in place, so the move resumes later.
pub fn run(journal_path: &Path, hooks: &MoveHooks) -> Result<MoveOutcome, MoveError> {
    let mut journal = Journal::load(journal_path)?.ok_or(MoveError::NoJournal)?;
    loop {
        match journal.phase {
            Phase::Copy => match copy_phase(&mut journal, journal_path, hooks) {
                Ok(()) => {}
                Err(MoveError::Cancelled) => {
                    rollback(&journal, journal_path)?;
                    return Ok(MoveOutcome::Cancelled);
                }
                Err(error) => return Err(error),
            },
            Phase::Commit => commit_phase(&mut journal, journal_path, hooks)?,
            Phase::Cleanup => {
                return cleanup_phase(&mut journal, journal_path, hooks).map(MoveOutcome::Finished)
            }
        }
    }
}

/// Startup: a move interrupted in Commit is finished now, synchronously,
/// before any service reads the config. Copy and Cleanup are returned for
/// the caller to resume in the background with [`run`].
pub fn recover(journal_path: &Path, hooks: &MoveHooks) -> Result<Recovery, MoveError> {
    let Some(mut journal) = Journal::load(journal_path)? else {
        return Ok(Recovery::None);
    };
    Ok(match journal.phase {
        Phase::Copy => Recovery::Copy(journal),
        Phase::Commit => {
            commit_phase(&mut journal, journal_path, hooks)?;
            Recovery::Cleanup(journal)
        }
        Phase::Cleanup => Recovery::Cleanup(journal),
    })
}

/// Cancels a move that is NOT running right now (a failed or not yet
/// resumed Copy): deletes the staging folder, the new root when GRID made
/// it and it is empty, and the journal. A running move is cancelled
/// through [`MoveHooks::cancel`] instead. After Copy:
/// [`MoveError::NotCancellable`].
pub fn cancel(journal_path: &Path) -> Result<(), MoveError> {
    let journal = Journal::load(journal_path)?.ok_or(MoveError::NoJournal)?;
    if journal.phase != Phase::Copy {
        return Err(MoveError::NotCancellable);
    }
    rollback(&journal, journal_path)
}

fn rollback(journal: &Journal, journal_path: &Path) -> Result<(), MoveError> {
    let new = PathBuf::from(&journal.new_root);
    let staging = new.join(STAGING_DIR);
    remove_tree(&staging).map_err(|e| io_error("could not delete", &staging, e))?;
    if journal.created_new_root {
        // Only when empty: anything else in it is not ours.
        let _ = fs::remove_dir(&new);
    }
    Journal::delete(journal_path)
}

// --- Copy phase -------------------------------------------------------------

fn copy_phase(
    journal: &mut Journal,
    journal_path: &Path,
    hooks: &MoveHooks,
) -> Result<(), MoveError> {
    let old = expand_root(&journal.old_root);
    let new = PathBuf::from(&journal.new_root);
    if !old.is_dir() {
        return Err(MoveError::RootUnreachable(text(&old)));
    }
    if !new.is_dir() {
        return Err(MoveError::RootUnreachable(text(&new)));
    }
    let staging = new.join(STAGING_DIR);
    let pending: Vec<usize> = (0..journal.items.len())
        .filter(|&i| {
            journal.items[i].strategy == Strategy::Copy
                && journal.items[i].state == ItemState::Pending
        })
        .collect();
    if !pending.is_empty() {
        fs::create_dir_all(&staging).map_err(|e| io_error("could not create", &staging, e))?;
        let remaining: u64 = pending
            .iter()
            .map(|&i| {
                let item = &journal.items[i];
                item.bytes
                    .saturating_sub(tree_size(&staging.join(&item.name)).0)
            })
            .sum();
        check_space(remaining, (hooks.free_space)(&new)).map_err(refused)?;
    }

    let moved_roots: Vec<PathBuf> = MOVED_DIRS.iter().map(|name| old.join(name)).collect();
    let mut buffer = vec![0u8; CHUNK];
    for i in pending {
        let name = journal.items[i].name.clone();
        let mut copier = Copier {
            hooks,
            item: name.clone(),
            total: journal.items[i].bytes,
            done: 0,
            moved_roots: &moved_roots,
            buffer: &mut buffer,
            links: Vec::new(),
            recorded: HashSet::new(),
        };
        let source = old.join(&name);
        let dest = staging.join(&name);
        // Before the copy: a change made while it runs then shows up in
        // Cleanup as a source that no longer matches.
        let digest = source_digest(&source).map_err(|e| io_error("could not read", &source, e))?;
        copier.copy_entry(&source, &dest, &name)?;
        let recorded = std::mem::take(&mut copier.recorded);
        let links = std::mem::take(&mut copier.links);
        verify_tree(&source, &dest, verify_mode(&name), &|path| {
            recorded.contains(path)
        })?;
        let prefix = format!("{name}/");
        journal
            .links
            .retain(|link| link.path != name && !link.path.starts_with(&prefix));
        journal.links.extend(links);
        journal.items[i].source_digest = digest;
        journal.items[i].state = ItemState::Copied;
        write_journal(journal, journal_path, hooks)?;
    }
    journal.phase = Phase::Commit;
    write_journal(journal, journal_path, hooks)
}

fn verify_mode(name: &str) -> VerifyMode {
    if name == SAVES_DIR {
        VerifyMode::Full
    } else {
        VerifyMode::Sampled
    }
}

struct Copier<'h, 'a, 'b> {
    hooks: &'h MoveHooks<'a>,
    item: String,
    total: u64,
    done: u64,
    moved_roots: &'b [PathBuf],
    buffer: &'b mut Vec<u8>,
    links: Vec<RecordedLink>,
    /// Source paths of the links in `links`, for the verify pass.
    recorded: HashSet<PathBuf>,
}

impl Copier<'_, '_, '_> {
    fn check_cancel(&self) -> Result<(), MoveError> {
        if self.hooks.cancel.load(Ordering::SeqCst) {
            Err(MoveError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn advance(&mut self, bytes: u64) {
        self.done += bytes;
        (self.hooks.progress)(&MoveProgress {
            phase: Phase::Copy,
            item: self.item.clone(),
            done_bytes: self.done,
            total_bytes: self.total,
        });
    }

    /// Copies `src` to `dst`; `rel` is `src` relative to the old root.
    fn copy_entry(&mut self, src: &Path, dst: &Path, rel: &str) -> Result<(), MoveError> {
        self.check_cancel()?;
        let meta = fs::symlink_metadata(src).map_err(|e| io_error("could not read", src, e))?;
        if let Some(link) = link_info(src, &meta) {
            return self.copy_link(src, dst, rel, link);
        }
        if is_reparse_point(&meta) {
            return Err(MoveError::Unsupported(text(src)));
        }
        if meta.is_dir() {
            fs::create_dir_all(dst).map_err(|e| io_error("could not create", dst, e))?;
            let mut names: Vec<_> = fs::read_dir(src)
                .map_err(|e| io_error("could not read", src, e))?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<Result<_, _>>()
                .map_err(|e| io_error("could not read", src, e))?;
            names.sort();
            for name in names {
                let child_rel = format!("{rel}/{}", name.to_string_lossy());
                self.copy_entry(&src.join(&name), &dst.join(&name), &child_rel)?;
            }
            Ok(())
        } else if meta.is_file() {
            self.copy_file(src, dst, &meta)
        } else {
            Err(MoveError::Unsupported(text(src)))
        }
    }

    /// A relative unix link that stays inside the moved folders is copied
    /// as itself (it resolves the same way at the new root). Every other
    /// link is recorded and re-created at Commit with its target relocated.
    fn copy_link(
        &mut self,
        src: &Path,
        dst: &Path,
        rel: &str,
        link: LinkInfo,
    ) -> Result<(), MoveError> {
        let target = absolute_target(src, &link.raw);
        let inside = self
            .moved_roots
            .iter()
            .any(|root| target == *root || target.starts_with(root));
        #[cfg(unix)]
        if link.raw.is_relative() && inside {
            if let Ok(existing) = fs::read_link(dst) {
                if existing == link.raw {
                    return Ok(());
                }
                fs::remove_file(dst).map_err(|e| io_error("could not replace", dst, e))?;
            }
            return std::os::unix::fs::symlink(&link.raw, dst)
                .map_err(|e| io_error("could not create the link", dst, e));
        }
        #[cfg(not(unix))]
        let _ = (dst, inside);
        self.recorded.insert(src.to_path_buf());
        self.links.push(RecordedLink {
            path: rel.to_string(),
            target: text(&target),
            dir: link.dir,
        });
        Ok(())
    }

    /// Chunked copy with cancel checks and progress; the modified time and
    /// the permissions (read-only, unix mode bits) are kept. A copy left by
    /// an earlier run with the same length and modified time is skipped.
    fn copy_file(&mut self, src: &Path, dst: &Path, meta: &fs::Metadata) -> Result<(), MoveError> {
        let len = meta.len();
        let modified = meta
            .modified()
            .map_err(|e| io_error("could not read the time of", src, e))?;
        if let Ok(existing) = fs::symlink_metadata(dst) {
            if existing.is_file()
                && existing.len() == len
                && existing.modified().is_ok_and(|t| same_time(t, modified))
            {
                self.advance(len);
                return Ok(());
            }
            make_writable(dst, &existing).map_err(|e| io_error("could not replace", dst, e))?;
            fs::remove_file(dst).map_err(|e| io_error("could not replace", dst, e))?;
        }
        let mut input = File::open(src).map_err(|e| io_error("could not open", src, e))?;
        let mut output = File::create(dst).map_err(|e| io_error("could not create", dst, e))?;
        loop {
            self.check_cancel()?;
            let read = input
                .read(self.buffer)
                .map_err(|e| io_error("could not read", src, e))?;
            if read == 0 {
                break;
            }
            output
                .write_all(&self.buffer[..read])
                .map_err(|e| io_error("could not write", dst, e))?;
            self.advance(read as u64);
        }
        output
            .set_times(FileTimes::new().set_modified(modified))
            .map_err(|e| io_error("could not set the time of", dst, e))?;
        output
            .sync_all()
            .map_err(|e| io_error("could not write", dst, e))?;
        drop(output);
        fs::set_permissions(dst, meta.permissions())
            .map_err(|e| io_error("could not set the permissions of", dst, e))
    }
}

fn same_time(a: SystemTime, b: SystemTime) -> bool {
    let gap = match a.duration_since(b) {
        Ok(gap) => gap,
        Err(e) => e.duration(),
    };
    gap < MTIME_TOLERANCE
}

/// Clears the read-only attribute, which on Windows blocks a delete or an
/// overwrite. Unix needs nothing: a delete depends on the folder.
#[cfg(windows)]
// Windows only, where the flag is the read-only attribute and there are no
// world-writable mode bits for this lint to protect.
#[allow(clippy::permissions_set_readonly_false)]
fn make_writable(path: &Path, meta: &fs::Metadata) -> io::Result<()> {
    let mut permissions = meta.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn make_writable(_path: &Path, _meta: &fs::Metadata) -> io::Result<()> {
    Ok(())
}

// --- verify -----------------------------------------------------------------

/// How much of each file [`verify_copy`] hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyMode {
    /// The whole file (`saves/`).
    Full,
    /// The first and last 1 MiB (`games/`, `emulators/`).
    Sampled,
}

/// Checks that `dst` holds a copy of `src`: the same folders, every file
/// with the same size and the same hash ([`VerifyMode`]), every link as a
/// link. [`MoveError::VerifyFailed`] names the first source path that does
/// not match.
pub fn verify_copy(src: &Path, dst: &Path, mode: VerifyMode) -> Result<(), MoveError> {
    verify_tree(src, dst, mode, &|_| false)
}

/// [`verify_copy`], with `skip` naming source links that were recorded
/// instead of copied.
fn verify_tree(
    src: &Path,
    dst: &Path,
    mode: VerifyMode,
    skip: &dyn Fn(&Path) -> bool,
) -> Result<(), MoveError> {
    let failed = || MoveError::VerifyFailed(text(src));
    let meta = fs::symlink_metadata(src).map_err(|e| io_error("could not read", src, e))?;
    if link_info(src, &meta).is_some() {
        if skip(src) || is_link(dst) {
            return Ok(());
        }
        return Err(failed());
    }
    let copy = fs::symlink_metadata(dst).map_err(|_| failed())?;
    if meta.is_dir() {
        if !copy.is_dir() {
            return Err(failed());
        }
        for entry in fs::read_dir(src).map_err(|e| io_error("could not read", src, e))? {
            let name = entry
                .map_err(|e| io_error("could not read", src, e))?
                .file_name();
            verify_tree(&src.join(&name), &dst.join(&name), mode, skip)?;
        }
        return Ok(());
    }
    if !copy.is_file() || copy.len() != meta.len() {
        return Err(failed());
    }
    let source_hash = hash_file(src, mode).map_err(|e| io_error("could not read", src, e))?;
    let copy_hash = hash_file(dst, mode).map_err(|e| io_error("could not read", dst, e))?;
    if source_hash != copy_hash {
        return Err(failed());
    }
    Ok(())
}

fn hash_file(path: &Path, mode: VerifyMode) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = Sha256::new();
    let mut feed = |file: &mut File, limit: u64| -> io::Result<()> {
        let mut buffer = vec![0u8; CHUNK];
        let mut left = limit;
        while left > 0 {
            let want = buffer
                .len()
                .min(usize::try_from(left).unwrap_or(usize::MAX));
            let read = file.read(&mut buffer[..want])?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            left -= read as u64;
        }
        Ok(())
    };
    if mode == VerifyMode::Full || len <= 2 * SAMPLE {
        feed(&mut file, len)?;
    } else {
        feed(&mut file, SAMPLE)?;
        file.seek(SeekFrom::Start(len - SAMPLE))?;
        feed(&mut file, SAMPLE)?;
    }
    Ok(hasher.finalize().to_vec())
}

// --- Commit phase -----------------------------------------------------------

fn commit_phase(
    journal: &mut Journal,
    journal_path: &Path,
    hooks: &MoveHooks,
) -> Result<(), MoveError> {
    let old = expand_root(&journal.old_root);
    let new = PathBuf::from(&journal.new_root);
    if !new.is_dir() {
        return Err(MoveError::RootUnreachable(text(&new)));
    }
    let relocator = Relocator::for_journal(journal);
    let new_roots = new_root_forms(&new, &journal.new_canonical);

    // a) Place every item. A copied item that is itself a recorded link
    // has nothing in staging: its link is re-created below.
    for i in 0..journal.items.len() {
        let name = journal.items[i].name.clone();
        let whole_link = journal.items[i].strategy == Strategy::Copy
            && journal.links.iter().any(|link| link.path == name);
        if !whole_link {
            place_item(&journal.items[i], &old, &new)?;
        }
        if matches!(
            journal.items[i].state,
            ItemState::Pending | ItemState::Copied
        ) {
            journal.items[i].state = ItemState::Placed;
            write_journal(journal, journal_path, hooks)?;
        }
    }
    let _ = fs::remove_dir(new.join(STAGING_DIR));

    // Links before the text files: the files are read through them, and a
    // junction still aimed at the old root (or not re-created yet) would
    // hide them.
    for link in journal.links.clone() {
        if let Some(line) = recreate_link(&new, &link, &relocator)? {
            journal.note(line);
        }
    }
    let mut notes = Vec::new();
    for item in journal
        .items
        .iter()
        .filter(|item| item.strategy == Strategy::Rename)
    {
        repoint_links(&new.join(&item.name), &relocator, &mut notes)?;
    }
    for line in notes.drain(..) {
        journal.note(line);
    }
    commit_step(journal, journal_path, hooks, 1)?;

    repair_emulators(&relocator, &new_roots, hooks, &mut notes)?;
    for line in notes.drain(..) {
        journal.note(line);
    }
    commit_step(journal, journal_path, hooks, 2)?;

    rewrite_registry(&relocator, hooks)?;
    commit_step(journal, journal_path, hooks, 3)?;

    rewrite_config(&relocator, &new, hooks)?;
    commit_step(journal, journal_path, hooks, 4)?;

    resync(hooks, &mut notes)?;
    for (dir, value) in stale_values(&relocator, hooks)? {
        let copied = journal
            .items
            .iter_mut()
            .find(|item| item.name == dir && item.strategy == Strategy::Copy);
        notes.push(match copied {
            Some(item) => {
                item.keep_source = true;
                format!(
                    "{value} still points into the old library folder, so {} was left in place.",
                    old.join(dir).display()
                )
            }
            None => format!("{value} still points into the old library folder."),
        });
    }
    for line in notes {
        journal.note(line);
    }
    commit_step(journal, journal_path, hooks, 5)?;

    journal.phase = Phase::Cleanup;
    write_journal(journal, journal_path, hooks)
}

fn commit_step(
    journal: &mut Journal,
    journal_path: &Path,
    hooks: &MoveHooks,
    step: u32,
) -> Result<(), MoveError> {
    journal.commit_steps = journal.commit_steps.max(step);
    write_journal(journal, journal_path, hooks)
}

/// The new root as stored, and as the filesystem resolves it.
fn new_root_forms(new: &Path, canonical: &str) -> Vec<PathBuf> {
    let mut forms = vec![new.to_path_buf()];
    if !canonical.trim().is_empty() {
        forms.push(PathBuf::from(canonical));
    }
    forms
}

/// Moves one item to `<new>/<name>`: out of staging for a copied item, from
/// the old root for a renamed one. Already there: nothing to do.
fn place_item(item: &JournalItem, old: &Path, new: &Path) -> Result<(), MoveError> {
    let dest = new.join(&item.name);
    let source = match item.strategy {
        Strategy::Copy => new.join(STAGING_DIR).join(&item.name),
        Strategy::Rename => old.join(&item.name),
    };
    let source_exists = fs::symlink_metadata(&source).is_ok();
    let dest_exists = fs::symlink_metadata(&dest).is_ok();
    match (source_exists, dest_exists) {
        (true, false) => fs::rename(&source, &dest).map_err(|e| {
            MoveError::Io(format!(
                "could not move {} to {}: {e}. Close any program that uses this folder, then try again.",
                source.display(),
                dest.display()
            ))
        }),
        (false, true) => Ok(()),
        (true, true) => Err(MoveError::Io(format!(
            "{} and {} both exist; the move stopped so neither is lost",
            source.display(),
            dest.display()
        ))),
        (false, false) => Err(MoveError::Io(format!(
            "{} is missing; the move cannot place it",
            source.display()
        ))),
    }
}

/// Re-creates one recorded link at the new root, its target relocated.
/// Already there: left alone. A directory link that cannot be made fails
/// the commit (the emulator config files are read through such links); a
/// file link (on Windows it can need a privilege) is a report line, so it
/// can never block the commit for good.
fn recreate_link(
    new: &Path,
    link: &RecordedLink,
    relocator: &Relocator,
) -> Result<Option<String>, MoveError> {
    let path = link
        .path
        .split('/')
        .fold(new.to_path_buf(), |path, part| path.join(part));
    if fs::symlink_metadata(&path).is_ok() {
        return Ok(None);
    }
    let target = relocator
        .relocate(&link.target)
        .unwrap_or_else(|| link.target.clone());
    match create_link(Path::new(&target), &path, link.dir) {
        Ok(()) => Ok(None),
        Err(e) if !link.dir => Ok(Some(format!(
            "The link {} to {target} could not be made: {e}",
            path.display()
        ))),
        Err(e) => Err(io_error("could not create the link", &path, e)),
    }
}

/// Walks `dir` (links not followed) and points every absolute link whose
/// target is under the old root at the relocated target. Relative unix
/// links are left as they are: they moved with their targets. As in
/// [`recreate_link`], only a file link that cannot be made is a report line.
fn repoint_links(
    dir: &Path,
    relocator: &Relocator,
    notes: &mut Vec<String>,
) -> Result<(), MoveError> {
    let Ok(meta) = fs::symlink_metadata(dir) else {
        return Ok(());
    };
    if let Some(link) = link_info(dir, &meta) {
        if !link.raw.is_absolute() {
            return Ok(());
        }
        let Some(target) = relocator.relocate(&link.raw.to_string_lossy()) else {
            return Ok(());
        };
        remove_any_link(dir, &meta).map_err(|e| io_error("could not replace the link", dir, e))?;
        return match create_link(Path::new(&target), dir, link.dir) {
            Ok(()) => Ok(()),
            Err(e) if !link.dir => {
                notes.push(format!(
                    "The link {} to {target} could not be made: {e}",
                    dir.display()
                ));
                Ok(())
            }
            Err(e) => Err(io_error("could not create the link", dir, e)),
        };
    }
    if !meta.is_dir() || is_reparse_point(&meta) {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| io_error("could not read", dir, e))? {
        let entry = entry.map_err(|e| io_error("could not read", dir, e))?;
        repoint_links(&entry.path(), relocator, notes)?;
    }
    Ok(())
}

/// Whether the (expanded) `path` is under `<new root>/emulators` in any of
/// its spellings: an emulator GRID manages, whose files moved.
fn is_managed(path: &str, new_roots: &[PathBuf]) -> bool {
    let home = crate::platform::home_dir();
    let expanded = expand_root(path);
    new_roots.iter().any(|root| {
        path_change::is_strictly_inside(&expanded, &root.join(EMULATORS_DIR), home.as_deref())
    })
}

/// For every managed emulator entry, with its path relocated: repair the
/// catalog user-data links (only where the data root holds links, never
/// moving a real folder), then rewrite the config files that store
/// absolute library paths. A file that cannot be read or written is a
/// report line, not a failure.
fn repair_emulators(
    relocator: &Relocator,
    new_roots: &[PathBuf],
    hooks: &MoveHooks,
    notes: &mut Vec<String>,
) -> Result<(), MoveError> {
    let config = Config::load(hooks.config_path)
        .map_err(|e| io_error("could not read", hooks.config_path, io::Error::other(e)))?;
    for entry in &config.emulators {
        let path = relocator
            .relocate(&entry.path)
            .unwrap_or_else(|| entry.path.clone());
        if !is_managed(&path, new_roots) {
            continue;
        }
        let moved = EmulatorEntry {
            path: path.clone(),
            ..entry.clone()
        };
        if let Some(profile) = profile_for_entry(&moved.name, &moved.path, hooks.profiles) {
            if !profile.user_data.is_empty() {
                let root = user_data_root(profile, &expand_root(&moved.path));
                if root.is_dir() && only_links(&root, &profile.user_data) {
                    let target = saves_dir(&new_roots[0], &profile.name);
                    ensure_user_data_links(&root, &target, &profile.user_data).map_err(|e| {
                        MoveError::Io(format!(
                            "could not link the user data of {}: {e}",
                            root.display()
                        ))
                    })?;
                }
            }
        }
        let Some(data_root) = crate::autoconfig::emulator_data_root(&moved, hooks.profiles) else {
            continue;
        };
        for relative in emulator_config_files(&moved, hooks.profiles) {
            if let Err(line) = rewrite_text_file(&data_root.join(relative), relocator) {
                notes.push(line);
            }
        }
    }
    Ok(())
}

/// True when every `user_data` name at `root` is a link or absent, and at
/// least one is a link: the data root GRID linked. A real folder there is
/// the user's and is never merged.
fn only_links(root: &Path, user_data: &[String]) -> bool {
    let mut any_link = false;
    for name in user_data {
        let path = root.join(name.trim());
        if is_link(&path) {
            any_link = true;
        } else if fs::symlink_metadata(&path).is_ok() {
            return false;
        }
    }
    any_link
}

/// Rewrites the old-root paths in one text file (temp file + rename). A
/// missing file is skipped; a failure is the report line.
fn rewrite_text_file(path: &Path, relocator: &Relocator) -> Result<(), String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(format!(
                "{} could not be read, so its paths were not updated: {e}",
                path.display()
            ))
        }
    };
    let Some(rewritten) = relocator.rewrite_text(&content) else {
        return Ok(());
    };
    let tmp = path.with_extension("grid-move.tmp");
    let write = || -> io::Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(rewritten.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!(
            "{} could not be written, so its paths were not updated: {e}",
            path.display()
        )
    })
}

fn rewrite_registry(relocator: &Relocator, hooks: &MoveHooks) -> Result<(), MoveError> {
    hooks
        .registry
        .rewrite_paths(&|value| relocator.relocate(value))
        .map(|_| ())
        .map_err(|e| MoveError::Io(format!("could not update the installed games: {e}")))
}

/// The one config edit: every emulator path and save path relocated, the
/// root switched (unless it already is), the layout kept current —
/// `switch_library_root` would stamp the new folder 1, because it already
/// has `games/`.
fn rewrite_config(relocator: &Relocator, new: &Path, hooks: &MoveHooks) -> Result<(), MoveError> {
    let home = crate::platform::home_dir();
    (hooks.edit_config)(&mut |config: &mut Config| {
        relocate_config(config, relocator);
        let current = super::removal_key(&expand_root(&config.library_path), home.as_deref());
        if current.is_none() || current != super::removal_key(new, home.as_deref()) {
            path_change::switch_library_root(config, new);
        }
        config.library_layout_version = LAYOUT_VERSION_CURRENT;
        Ok(())
    })
    .map_err(|e| MoveError::Io(format!("could not update the settings: {e}")))
}

fn relocate_config(config: &mut Config, relocator: &Relocator) {
    for entry in &mut config.emulators {
        if let Some(path) = relocator.relocate(&entry.path) {
            entry.path = path;
        }
        entry.save_paths = relocate_list(&entry.save_paths, relocator);
        entry.state_paths = relocate_list(&entry.state_paths, relocator);
    }
    for map in [
        &mut config.native_manual_save_paths,
        &mut config.native_pcgw_save_paths,
        &mut config.native_removed_save_paths,
    ] {
        for values in map.values_mut() {
            for value in values.iter_mut() {
                if let Some(path) = relocator.relocate(value) {
                    *value = path;
                }
            }
        }
    }
}

/// A `;`/newline-separated path list with each item relocated, re-joined
/// with `;`. Unchanged when no item moves.
fn relocate_list(value: &str, relocator: &Relocator) -> String {
    let items = split_list(value);
    if !items.iter().any(|item| relocator.relocate(item).is_some()) {
        return value.to_string();
    }
    items
        .into_iter()
        .map(|item| relocator.relocate(&item).unwrap_or(item))
        .collect::<Vec<_>>()
        .join(";")
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split([';', '\r', '\n'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}

fn resync(hooks: &MoveHooks, notes: &mut Vec<String>) -> Result<(), MoveError> {
    let config = Config::load(hooks.config_path)
        .map_err(|e| io_error("could not read", hooks.config_path, io::Error::other(e)))?;
    if let Err(e) = (hooks.resync)(&config) {
        notes.push(format!("Emulator settings could not be refreshed: {e}"));
    }
    Ok(())
}

/// Every registry and config value still under a moved folder at the old
/// root, with that folder's name. Empty after a clean commit.
fn stale_values(
    relocator: &Relocator,
    hooks: &MoveHooks,
) -> Result<Vec<(&'static str, String)>, MoveError> {
    let mut values: Vec<String> = Vec::new();
    let rows = hooks
        .registry
        .all()
        .map_err(|e| MoveError::Io(format!("could not read the installed games: {e}")))?;
    for row in &rows {
        values.extend(row_paths(row));
    }
    let config = Config::load(hooks.config_path)
        .map_err(|e| io_error("could not read", hooks.config_path, io::Error::other(e)))?;
    for entry in &config.emulators {
        values.push(entry.path.clone());
        values.extend(split_list(&entry.save_paths));
        values.extend(split_list(&entry.state_paths));
    }
    for map in [
        &config.native_manual_save_paths,
        &config.native_pcgw_save_paths,
        &config.native_removed_save_paths,
    ] {
        values.extend(map.values().flatten().cloned());
    }
    Ok(values
        .into_iter()
        .filter_map(|value| relocator.moved_dir_of(&value).map(|dir| (dir, value)))
        .collect())
}

fn row_paths(row: &InstalledGame) -> Vec<String> {
    let mut out: Vec<String> = [
        &row.archive_path,
        &row.extracted_path,
        &row.extracted_dir,
        &row.multi_file_game_dir,
        &row.native_executable_path,
        &row.native_wineprefix,
        &row.native_game_dir,
        &row.ps3_iso_path,
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .cloned()
    .collect();
    out.extend(serde_json::from_str::<Vec<String>>(&row.ps3_trophy_paths).unwrap_or_default());
    out
}

// --- Cleanup phase ----------------------------------------------------------

fn cleanup_phase(
    journal: &mut Journal,
    journal_path: &Path,
    hooks: &MoveHooks,
) -> Result<MoveReport, MoveError> {
    let old = expand_root(&journal.old_root);
    let reachable = old.is_dir();
    for i in 0..journal.items.len() {
        if matches!(journal.items[i].state, ItemState::Cleaned | ItemState::Kept) {
            continue;
        }
        let item = journal.items[i].clone();
        let source = old.join(&item.name);
        let state = if item.strategy == Strategy::Rename
            || fs::symlink_metadata(&source).is_err() && reachable
        {
            ItemState::Cleaned
        } else if !reachable {
            journal.note(format!(
                "{} could not be reached, so nothing there was deleted.",
                old.display()
            ));
            ItemState::Kept
        } else if item.keep_source {
            ItemState::Kept
        } else if !item.source_digest.is_empty()
            && source_digest(&source).is_ok_and(|digest| digest == item.source_digest)
        {
            remove_tree(&source).map_err(|e| io_error("could not delete", &source, e))?;
            ItemState::Cleaned
        } else {
            journal.note(format!(
                "{} changed during the move, so it was left in place.",
                source.display()
            ));
            ItemState::Kept
        };
        journal.items[i].state = state;
        write_journal(journal, journal_path, hooks)?;
    }
    let old_root_removed = reachable
        && fs::read_dir(&old).is_ok_and(|mut entries| entries.next().is_none())
        && fs::remove_dir(&old).is_ok();
    Journal::delete(journal_path)?;
    Ok(MoveReport {
        new_root: journal.new_root.clone(),
        report: journal.report.clone(),
        old_root_removed,
    })
}

/// SHA-256 (hex) over the tree at `root`, links not followed: for every
/// entry in sorted order its relative path and kind, plus a file's length
/// and modified time (nanoseconds) and a link's raw target. Any added,
/// removed, resized, touched or retargeted entry changes it.
///
/// This compares the source with ITSELF, not with the copy: Commit rewrites
/// emulator config files in the copy and the user may play (and save) as
/// soon as the move commits, so the copy is expected to differ.
fn source_digest(root: &Path) -> io::Result<String> {
    fn walk(root: &Path, path: &Path, hasher: &mut Sha256) -> io::Result<()> {
        let meta = fs::symlink_metadata(path)?;
        let rel = path.strip_prefix(root).unwrap_or(path);
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        if let Some(link) = link_info(path, &meta) {
            hasher.update(b"link");
            hasher.update(link.raw.to_string_lossy().as_bytes());
        } else if meta.is_dir() && !is_reparse_point(&meta) {
            hasher.update(b"dir");
            let mut names: Vec<_> = fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<Result<_, _>>()?;
            names.sort();
            for name in names {
                walk(root, &path.join(name), hasher)?;
            }
        } else {
            hasher.update(b"file");
            hasher.update(meta.len().to_le_bytes());
            let nanos = match meta.modified()?.duration_since(SystemTime::UNIX_EPOCH) {
                Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
                Err(before) => -i128::try_from(before.duration().as_nanos()).unwrap_or(i128::MAX),
            };
            hasher.update(nanos.to_le_bytes());
        }
        hasher.update([0]);
        Ok(())
    }
    let mut hasher = Sha256::new();
    walk(root, root, &mut hasher)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Deletes `path` and everything under it, never following a link or a
/// junction (the link itself goes). A missing path is fine. Read-only
/// files are made writable first on Windows.
fn remove_tree(path: &Path) -> io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if link_info(path, &meta).is_some() {
        return remove_any_link(path, &meta);
    }
    if is_reparse_point(&meta) {
        return Err(io::Error::other(format!(
            "{} is a special file and was not deleted",
            path.display()
        )));
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            remove_tree(&entry?.path())?;
        }
        return fs::remove_dir(path);
    }
    make_writable(path, &meta)?;
    fs::remove_file(path)
}

// --- relocate references only -------------------------------------------------

/// The unreachable-library fix: the library now lives at `new_root` (the
/// user moved it outside GRID). Rewrites every reference from `old_root`
/// (as typed) to `new_root` — links, emulator config files, registry,
/// config with the root switch — and moves no file. Returns the report
/// lines. Not journaled: every step is idempotent, so running it again
/// after a crash finishes it.
pub fn relocate_references(
    old_root: &str,
    new_root: &Path,
    hooks: &MoveHooks,
) -> Result<Vec<String>, MoveError> {
    if !new_root.is_dir() {
        return Err(MoveError::RootUnreachable(text(new_root)));
    }
    let new_text = text(new_root);
    let old_expanded = expand_root(old_root);
    let new_canonical = crate::platform::canonicalize(new_root)
        .map(|resolved| text(&resolved))
        .unwrap_or_default();
    let mut spellings = vec![
        (old_root.to_string(), new_text.clone()),
        (text(&old_expanded), new_text.clone()),
    ];
    if let Ok(resolved) = crate::platform::canonicalize(&old_expanded) {
        spellings.push((text(&resolved), new_canonical.clone()));
    }
    let relocator = Relocator::new(&spellings);
    let new_roots = new_root_forms(new_root, &new_canonical);

    let mut notes = Vec::new();
    for name in MOVED_DIRS {
        repoint_links(&new_root.join(name), &relocator, &mut notes)?;
    }
    repair_emulators(&relocator, &new_roots, hooks, &mut notes)?;
    rewrite_registry(&relocator, hooks)?;
    rewrite_config(&relocator, new_root, hooks)?;
    resync(hooks, &mut notes)?;
    for (_, value) in stale_values(&relocator, hooks)? {
        notes.push(format!("{value} still points into the old library folder."));
    }
    Ok(notes)
}

// --- gate -------------------------------------------------------------------

/// The library is busy: why, in the user's words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct LibraryBusy {
    pub reason: String,
}

/// Open or closed, shared by every clone. A library move closes it first
/// and opens it when it ends; installs, launches, uninstalls and path
/// changes check it. Work that must not start under a closed gate and
/// takes a moment to register (a launch) holds a [`GateTicket`] meanwhile,
/// so the closer can wait for those with [`Self::wait_no_tickets`] before
/// it reads what is running.
#[derive(Clone, Default)]
pub struct LibraryGate {
    inner: Arc<GateInner>,
}

#[derive(Default)]
struct GateInner {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Default)]
struct GateState {
    reason: Option<String>,
    tickets: usize,
}

/// Proof that the gate was open when the work began. Dropping it lets
/// [`LibraryGate::wait_no_tickets`] return.
pub struct GateTicket {
    gate: LibraryGate,
}

impl Drop for GateTicket {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.tickets = state.tickets.saturating_sub(1);
        self.gate.inner.changed.notify_all();
    }
}

impl std::fmt::Debug for LibraryGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryGate")
            .field("reason", &self.reason())
            .finish()
    }
}

impl LibraryGate {
    /// A gate that starts closed (a journal exists at startup).
    pub fn closed(reason: impl Into<String>) -> Self {
        let gate = Self::default();
        gate.close(reason);
        gate
    }

    /// A poisoned lock is recovered: the state is two plain fields.
    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Closes the gate. `false` when it was already closed (the reason is
    /// kept).
    pub fn close(&self, reason: impl Into<String>) -> bool {
        let mut state = self.lock();
        if state.reason.is_some() {
            return false;
        }
        state.reason = Some(reason.into());
        self.inner.changed.notify_all();
        true
    }

    pub fn open(&self) {
        self.lock().reason = None;
        self.inner.changed.notify_all();
    }

    pub fn is_closed(&self) -> bool {
        self.lock().reason.is_some()
    }

    pub fn reason(&self) -> Option<String> {
        self.lock().reason.clone()
    }

    /// `Err` with the reason while closed.
    pub fn check(&self) -> Result<(), LibraryBusy> {
        match &self.lock().reason {
            Some(reason) => Err(LibraryBusy {
                reason: reason.clone(),
            }),
            None => Ok(()),
        }
    }

    /// [`Self::check`], and a ticket held until the work has registered.
    pub fn enter(&self) -> Result<GateTicket, LibraryBusy> {
        let mut state = self.lock();
        if let Some(reason) = &state.reason {
            return Err(LibraryBusy {
                reason: reason.clone(),
            });
        }
        state.tickets += 1;
        Ok(GateTicket { gate: self.clone() })
    }

    /// Waits up to `timeout` for the gate to open; `true` when it is open.
    pub fn wait_open(&self, timeout: Duration) -> bool {
        self.wait_until(timeout, |state| state.reason.is_none())
    }

    /// Waits up to `timeout` for every ticket to drop; `true` when none is
    /// left.
    pub fn wait_no_tickets(&self, timeout: Duration) -> bool {
        self.wait_until(timeout, |state| state.tickets == 0)
    }

    fn wait_until(&self, timeout: Duration, done: impl Fn(&GateState) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        while !done(&state) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self
                .inner
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Instant;

    fn pairs(list: &[(&str, &str)]) -> Relocator {
        Relocator::new(
            &list
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect::<Vec<_>>(),
        )
    }

    fn s(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    // --- matcher -------------------------------------------------------------

    #[test]
    fn relocate_table() {
        let r = pairs(&[
            ("~/Lib", "/mnt/New"),
            ("/home/u/Lib", "/mnt/New"),
            ("/real/Lib", "/real/New"),
        ]);
        let cases: &[(&str, Option<&str>)] = &[
            (
                "/home/u/Lib/games/SNES/x.sfc",
                Some("/mnt/New/games/SNES/x.sfc"),
            ),
            ("/home/u/Lib/emulators", Some("/mnt/New/emulators")),
            ("/home/u/Lib/saves/", Some("/mnt/New/saves/")),
            (
                "~/Lib/saves/PCSX2/memcards",
                Some("/mnt/New/saves/PCSX2/memcards"),
            ),
            ("/real/Lib/games/a", Some("/real/New/games/a")),
            // Both separator forms.
            ("\\home\\u\\Lib\\games\\x", Some("\\mnt\\New\\games\\x")),
            // Not one of the three moved folders.
            ("/home/u/Lib/notes.txt", None),
            ("/home/u/Lib/gamesX/y", None),
            ("/home/u/Lib/.vfs/x", None),
            ("/home/u/Lib", None),
            // Another root that only starts or ends the same way.
            ("/home/u/Lib2/games/x", None),
            ("/other/home/u/Lib/games/x", None),
            ("", None),
        ];
        for (value, expected) in cases {
            assert_eq!(r.relocate(value).as_deref(), *expected, "value {value:?}");
        }
    }

    #[test]
    fn relocate_windows_spellings() {
        let r = pairs(&[("C:\\Lib", "D:\\New")]);
        assert_eq!(
            r.relocate("C:\\Lib\\games\\x").as_deref(),
            Some("D:\\New\\games\\x")
        );
        assert_eq!(
            r.relocate("C:/Lib/saves/y").as_deref(),
            Some("D:/New/saves/y")
        );
        // A `\\`-escaped value, as a JSON or TOML basic string stores it.
        assert_eq!(
            r.relocate("C:\\\\Lib\\\\games\\\\z").as_deref(),
            Some("D:\\\\New\\\\games\\\\z")
        );
        let other_case = r.relocate("c:\\lib\\Games\\x");
        if cfg!(windows) {
            assert_eq!(other_case.as_deref(), Some("D:\\New\\Games\\x"));
        } else {
            assert_eq!(other_case, None, "case matters off Windows");
        }
    }

    #[test]
    fn moved_dir_of_names_the_folder() {
        let r = pairs(&[("/lib", "/new")]);
        assert_eq!(r.moved_dir_of("/lib/saves/x"), Some(SAVES_DIR));
        assert_eq!(r.moved_dir_of("/lib/games"), Some(GAMES_DIR));
        assert_eq!(r.moved_dir_of("/lib/other/x"), None);
        assert_eq!(r.moved_dir_of("/new/games/x"), None);
    }

    #[test]
    fn rewrite_text_covers_every_emulator_file_format() {
        let r = pairs(&[("/lib", "/new"), ("/real/lib", "/real/new")]);
        let pcsx2 =
            "[Folders]\nBios = /real/lib/saves/PCSX2 (Playstation 2)/bios\nSnapshots = snaps\n";
        assert_eq!(
            r.rewrite_text(pcsx2).as_deref(),
            Some(
                "[Folders]\nBios = /real/new/saves/PCSX2 (Playstation 2)/bios\nSnapshots = snaps\n"
            )
        );
        let vfs = "\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"/real/lib/games/PlayStation 3/.vfs/dev_hdd0/\"\n";
        assert_eq!(
            r.rewrite_text(vfs).as_deref(),
            Some("\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"/real/new/games/PlayStation 3/.vfs/dev_hdd0/\"\n")
        );
        let games = "BLUS00001: /lib/games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n";
        assert_eq!(
            r.rewrite_text(games).as_deref(),
            Some("BLUS00001: /new/games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n")
        );
        let qt = "[version_manager]\nversionSelected=/lib/emulators/ShadPS4 (Playstation 4)/shadps4.exe\n";
        assert_eq!(
            r.rewrite_text(qt).as_deref(),
            Some("[version_manager]\nversionSelected=/new/emulators/ShadPS4 (Playstation 4)/shadps4.exe\n")
        );
        let xemu = "[sys.files]\nbootrom_path = '/lib/emulators/Xemu (Xbox)/mcpx_1.0.bin'\nhdd_path = '/lib/emulators/Xemu (Xbox)/xbox_hdd.qcow2'\n";
        assert_eq!(
            r.rewrite_text(xemu).as_deref(),
            Some("[sys.files]\nbootrom_path = '/new/emulators/Xemu (Xbox)/mcpx_1.0.bin'\nhdd_path = '/new/emulators/Xemu (Xbox)/xbox_hdd.qcow2'\n")
        );
        // An unrelated path that ENDS with the library spelling stays.
        assert_eq!(r.rewrite_text("Path = /other/lib/games/x\n"), None);
        assert_eq!(r.rewrite_text("nothing\n"), None);
    }

    #[test]
    fn rewrite_text_is_idempotent() {
        let r = pairs(&[("/lib", "/new")]);
        let once = r.rewrite_text("a = /lib/games/x\n").unwrap();
        assert_eq!(r.rewrite_text(&once), None);
    }

    fn journal_with(old: &str, new: &str, old_canonical: &str, new_canonical: &str) -> Journal {
        Journal {
            version: JOURNAL_VERSION,
            old_root: old.into(),
            new_root: new.into(),
            old_canonical: old_canonical.into(),
            new_canonical: new_canonical.into(),
            created_new_root: true,
            phase: Phase::Copy,
            items: vec![JournalItem {
                name: "games".into(),
                strategy: Strategy::Copy,
                state: ItemState::Pending,
                bytes: 10,
                files: 2,
                keep_source: false,
                source_digest: String::new(),
            }],
            links: vec![RecordedLink {
                path: "emulators/E/memcards".into(),
                target: "/lib/saves/E/memcards".into(),
                dir: true,
            }],
            commit_steps: 0,
            report: Vec::new(),
        }
    }

    #[test]
    fn for_journal_matches_the_typed_and_the_canonical_root() {
        let r = Relocator::for_journal(&journal_with("/lib", "/new", "/real/lib", "/real/new"));
        assert_eq!(r.relocate("/lib/games/a").as_deref(), Some("/new/games/a"));
        assert_eq!(
            r.relocate("/real/lib/saves/a").as_deref(),
            Some("/real/new/saves/a")
        );
    }

    /// A root reached through a symlink: files that stored the resolved path
    /// are matched through the canonical spelling the journal records.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_root_records_its_canonical_form() {
        let tmp = tempfile::tempdir().unwrap();
        let base = crate::platform::canonicalize(tmp.path()).unwrap();
        let real = base.join("real");
        fs::create_dir_all(real.join("games/SNES")).unwrap();
        let link = base.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let config = Config {
            library_path: s(&link),
            library_layout_version: LAYOUT_VERSION_CURRENT,
            ..Default::default()
        };
        let journal = start_move(
            &config,
            &s(&base.join("new")),
            LibraryActivity::default(),
            &base.join("data/library-move.json"),
            &test_env(&|_: &Path, _: &Path| Ok(Strategy::Rename), &|_: &Path| None),
        )
        .unwrap();
        assert_eq!(journal.old_canonical, s(&real));
        let r = Relocator::for_journal(&journal);
        assert_eq!(
            r.relocate(&format!("{}/games/SNES/x", s(&real))),
            Some(format!("{}/games/SNES/x", journal.new_canonical))
        );
    }

    // --- journal --------------------------------------------------------------

    #[test]
    fn journal_round_trips_and_writes_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("library-move.json");
        assert_eq!(Journal::load(&path).unwrap(), None);
        let journal = journal_with("/lib", "/new", "/lib", "/new");
        journal.save(&path).unwrap();
        assert_eq!(Journal::load(&path).unwrap(), Some(journal));
        let names: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["library-move.json"], "no temp file is left");
    }

    #[test]
    fn journal_with_an_unknown_version_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("library-move.json");
        fs::write(&path, r#"{"version": 99, "anything": true}"#).unwrap();
        assert!(matches!(
            Journal::load(&path),
            Err(MoveError::UnknownJournalVersion(99))
        ));
        fs::write(&path, "not json").unwrap();
        assert!(matches!(
            Journal::load(&path),
            Err(MoveError::BadJournal(_))
        ));
    }

    #[test]
    fn journal_path_is_beside_the_config() {
        assert_eq!(
            journal_path(Path::new("/data/config.toml")),
            Path::new("/data").join(JOURNAL_FILE_NAME)
        );
    }

    // --- gate -------------------------------------------------------------------

    #[test]
    fn gate_opens_closes_and_shares_state_between_clones() {
        let gate = LibraryGate::default();
        assert!(!gate.is_closed());
        assert_eq!(gate.check(), Ok(()));
        let clone = gate.clone();
        assert!(clone.close(LIBRARY_MOVE_BUSY));
        assert!(!gate.close("again"), "already closed");
        assert!(gate.is_closed());
        assert_eq!(gate.reason().as_deref(), Some(LIBRARY_MOVE_BUSY));
        assert_eq!(
            gate.check(),
            Err(LibraryBusy {
                reason: LIBRARY_MOVE_BUSY.into()
            })
        );
        assert!(gate.enter().is_err());
        clone.open();
        assert_eq!(gate.check(), Ok(()));
        assert!(LibraryGate::closed("x").is_closed());
    }

    #[test]
    fn gate_waits_for_tickets_and_for_opening() {
        let gate = LibraryGate::default();
        let ticket = gate.enter().unwrap();
        gate.close(LIBRARY_MOVE_BUSY);
        assert!(!gate.wait_no_tickets(Duration::from_millis(20)));
        let waiter = {
            let gate = gate.clone();
            std::thread::spawn(move || gate.wait_no_tickets(Duration::from_secs(5)))
        };
        drop(ticket);
        assert!(waiter.join().unwrap());

        assert!(!gate.wait_open(Duration::from_millis(20)));
        let opener = {
            let gate = gate.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(30));
                gate.open();
            })
        };
        let started = Instant::now();
        assert!(gate.wait_open(Duration::from_secs(5)));
        assert!(started.elapsed() < Duration::from_secs(5));
        opener.join().unwrap();
    }

    // --- space ------------------------------------------------------------------

    #[test]
    fn the_margin_is_one_gib_or_two_percent() {
        const GIB: u64 = 1 << 30;
        assert_eq!(space_margin(0), GIB);
        assert_eq!(space_margin(10 * GIB), GIB);
        assert_eq!(space_margin(100 * GIB), 2 * GIB);
        assert_eq!(check_space(0, Some(0)), Ok(0));
        assert_eq!(check_space(10 * GIB, Some(11 * GIB)), Ok(11 * GIB));
        assert_eq!(
            check_space(10 * GIB, Some(11 * GIB - 1)),
            Err(MoveRefusal::NotEnoughSpace)
        );
        assert_eq!(check_space(100 * GIB, Some(102 * GIB)), Ok(102 * GIB));
    }

    // --- plan refusals ------------------------------------------------------------

    fn test_env<'a>(probe: &'a StrategyProbe, free: &'a FreeSpace) -> PlanEnv<'a> {
        PlanEnv {
            probe,
            free_space: free,
        }
    }

    struct PlanFx {
        _tmp: tempfile::TempDir,
        base: PathBuf,
        old: PathBuf,
        config: Config,
        journal: PathBuf,
    }

    fn plan_fx() -> PlanFx {
        let tmp = tempfile::tempdir().unwrap();
        let base = crate::platform::canonicalize(tmp.path()).unwrap();
        let old = base.join("old");
        fs::create_dir_all(old.join("games/SNES")).unwrap();
        fs::write(old.join("games/SNES/a.sfc"), vec![1u8; 1000]).unwrap();
        fs::create_dir_all(old.join("saves")).unwrap();
        fs::write(old.join("notes.txt"), "x").unwrap();
        let config = Config {
            library_path: s(&old),
            library_layout_version: LAYOUT_VERSION_CURRENT,
            ..Default::default()
        };
        PlanFx {
            journal: base.join("data/library-move.json"),
            _tmp: tmp,
            base,
            old,
            config,
        }
    }

    fn plan(
        fx: &PlanFx,
        config: &Config,
        target: &Path,
        free: Option<u64>,
    ) -> Result<MovePreview, MoveError> {
        let free_fn = move |_: &Path| free;
        plan_move(
            config,
            &s(target),
            LibraryActivity::default(),
            &fx.journal,
            &test_env(&|_: &Path, _: &Path| Ok(Strategy::Copy), &free_fn),
        )
    }

    fn refusal(result: Result<MovePreview, MoveError>) -> MoveRefusal {
        match result {
            Err(MoveError::Refused(refusal)) => refusal,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn plan_lists_the_existing_moved_folders() {
        let fx = plan_fx();
        let preview = plan(&fx, &fx.config, &fx.base.join("new"), Some(u64::MAX)).unwrap();
        let names: Vec<&str> = preview.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["games", "saves"]);
        assert_eq!(preview.items[0].bytes, 1000);
        assert_eq!(preview.items[0].files, 1);
        assert_eq!(preview.copy_bytes, 1000);
        assert_eq!(preview.required_bytes, 1000 + (1 << 30));
        assert!(!fx.base.join("new").exists(), "planning creates nothing");
    }

    #[test]
    fn plan_refusals() {
        let fx = plan_fx();
        let new = fx.base.join("new");

        let old_layout = Config {
            library_layout_version: 1,
            ..fx.config.clone()
        };
        assert_eq!(
            refusal(plan(&fx, &old_layout, &new, Some(u64::MAX))),
            MoveRefusal::LayoutNotCurrent
        );

        let missing = Config {
            library_path: s(&fx.base.join("gone")),
            ..fx.config.clone()
        };
        assert_eq!(
            refusal(plan(&fx, &missing, &new, Some(u64::MAX))),
            MoveRefusal::OldRootMissing
        );

        let taken = fx.base.join("taken");
        fs::create_dir_all(taken.join("saves")).unwrap();
        assert_eq!(
            refusal(plan(&fx, &fx.config, &taken, Some(u64::MAX))),
            MoveRefusal::TargetHasLibraryFolders
        );

        assert_eq!(
            refusal(plan(&fx, &fx.config, &new, Some(10))),
            MoveRefusal::NotEnoughSpace
        );

        assert_eq!(
            refusal(plan(&fx, &fx.config, &fx.old.join("inner"), Some(u64::MAX))),
            MoveRefusal::PathChange(PathChangeRefusal::InsideCurrent)
        );

        fs::create_dir_all(fx.journal.parent().unwrap()).unwrap();
        fs::write(&fx.journal, "{}").unwrap();
        assert_eq!(
            refusal(plan(&fx, &fx.config, &new, Some(u64::MAX))),
            MoveRefusal::MoveActive
        );
    }

    #[test]
    fn a_running_move_in_the_activity_is_refused() {
        let fx = plan_fx();
        let busy = LibraryActivity {
            library_move: true,
            ..Default::default()
        };
        let result = plan_move(
            &fx.config,
            &s(&fx.base.join("new")),
            busy,
            &fx.journal,
            &test_env(&|_: &Path, _: &Path| Ok(Strategy::Copy), &|_: &Path| None),
        );
        assert_eq!(refusal(result), MoveRefusal::MoveActive);
    }

    #[test]
    fn every_refusal_has_a_message() {
        for refusal in [
            MoveRefusal::LayoutNotCurrent,
            MoveRefusal::OldRootMissing,
            MoveRefusal::TargetHasLibraryFolders,
            MoveRefusal::NotEnoughSpace,
            MoveRefusal::MoveActive,
            MoveRefusal::PathChange(PathChangeRefusal::InsideCurrent),
        ] {
            assert!(refusal.message().ends_with('.'), "{refusal:?}");
        }
    }

    #[test]
    fn the_rename_probe_reports_the_same_volume() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        assert_eq!(probe_strategy(&a, &b).unwrap(), Strategy::Rename);
        assert_eq!(fs::read_dir(&a).unwrap().count(), 0, "the probe cleans up");
        assert_eq!(fs::read_dir(&b).unwrap().count(), 0, "the probe cleans up");
    }

    #[test]
    fn free_space_reads_a_number_for_an_existing_folder() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(free_space(tmp.path()).is_some_and(|free| free > 0));
        // A missing folder is measured at its nearest existing ancestor.
        assert!(free_space(&tmp.path().join("a/b")).is_some());
    }
}
