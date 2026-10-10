//! Changing the library root from Settings › Library (Q2).
//!
//! Phase 9a covers "Start fresh"; phase 9b adds "Move existing files". Both
//! share the first and last steps here:
//!
//! 1. [`validate_new_library_path`] refuses a path that cannot become the
//!    new root, or a change while the library is in use. Its lexical half,
//!    [`check_new_library_path`], is pure.
//! 2. [`prepare_library_dir`] creates the new root and proves it writable,
//!    before anything is changed.
//! 3. The flow's own work: for Start fresh, [`start_fresh_rows`] picks the
//!    rows that leave the library, and `InstallService::start_fresh_forget` /
//!    `start_fresh_delete` drop them (the delete through the guarded
//!    uninstall path only).
//! 4. [`switch_library_root`] edits the config: the new `library_path`,
//!    the old root appended to `former_library_paths`, the layout version
//!    stamped for the new folder.
//!
//! Emulator entries are never touched here: their files under
//! `<old>/emulators` stay where they are and keep working, and the
//! `RemovalGuard` protects the old root's `emulators/` and `saves/`.

use std::fs;
use std::path::{Path, PathBuf};

use super::paths::layout_version_for_library_path;
use super::registry::InstalledGame;
use super::removal_key;
use super::update_detection::is_emulators_platform;
use crate::config::Config;

/// What is running right now that a path change must wait for. The app
/// layer fills it from the download queue, the live game sessions and the
/// cloud transfer counter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibraryActivity {
    /// A download or install is queued, running or cancelling.
    pub installs: bool,
    /// A game launched through GRID is still running.
    pub running_games: bool,
    /// A cloud save upload or restore is in flight.
    pub cloud_transfers: bool,
}

/// Why a new library path was refused. Serialized as a snake_case tag, so
/// the frontend can branch on it; [`Self::message`] is the user-facing text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathChangeRefusal {
    Blank,
    NotAbsolute,
    SameAsCurrent,
    InsideCurrent,
    ContainsCurrent,
    NotADirectory,
    NotCreatable,
    NotWritable,
    InstallActive,
    GameRunning,
    CloudTransferActive,
}

impl PathChangeRefusal {
    /// The sentence the Library pane shows.
    pub fn message(self) -> &'static str {
        match self {
            Self::Blank => "Enter a folder for the library.",
            Self::NotAbsolute => "Enter a full folder path, such as C:\\Games or /home/you/Games.",
            Self::SameAsCurrent => "This is already the library folder.",
            Self::InsideCurrent => "The new folder cannot be inside the current library folder.",
            Self::ContainsCurrent => "The new folder cannot contain the current library folder.",
            Self::NotADirectory => "This path is a file, not a folder.",
            Self::NotCreatable => "This folder cannot be created.",
            Self::NotWritable => "GRID cannot write to this folder.",
            Self::InstallActive => {
                "Wait until downloads and installs finish, or cancel them, then try again."
            }
            Self::GameRunning => "Close the running game, then try again.",
            Self::CloudTransferActive => {
                "Wait until the cloud save transfer finishes, then try again."
            }
        }
    }
}

/// How `candidate` relates to `current`, both as comparable keys.
fn compare_keys(current: &[String], candidate: &[String]) -> Result<(), PathChangeRefusal> {
    if candidate == current {
        Err(PathChangeRefusal::SameAsCurrent)
    } else if candidate.starts_with(current) {
        Err(PathChangeRefusal::InsideCurrent)
    } else if current.starts_with(candidate) {
        Err(PathChangeRefusal::ContainsCurrent)
    } else {
        Ok(())
    }
}

/// `raw` trimmed, with a leading `~/` (or `~\` on Windows) joined onto
/// `home`. Any other form is taken as typed.
fn expand(raw: &str, home: Option<&Path>) -> PathBuf {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("~/")
        .or_else(|| cfg!(windows).then(|| trimmed.strip_prefix("~\\")).flatten());
    match (rest, home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(trimmed),
    }
}

/// The lexical checks, in order: blank, not absolute (after `~/`
/// expansion against `home`), the current root itself, inside the current
/// root, containing the current root, then the activity. Returns the new
/// root as it will be stored. No current root (`current` blank) skips the
/// three comparisons. Pure.
pub fn check_new_library_path(
    current: &str,
    candidate: &str,
    activity: LibraryActivity,
    home: Option<&Path>,
) -> Result<PathBuf, PathChangeRefusal> {
    if candidate.trim().is_empty() {
        return Err(PathChangeRefusal::Blank);
    }
    let target = expand(candidate, home);
    let Some(target_key) = removal_key(&target, home) else {
        return Err(PathChangeRefusal::NotAbsolute);
    };
    if !current.trim().is_empty() {
        if let Some(current_key) = removal_key(&expand(current, home), home) {
            compare_keys(&current_key, &target_key)?;
        }
    }
    if activity.installs {
        return Err(PathChangeRefusal::InstallActive);
    }
    if activity.running_games {
        return Err(PathChangeRefusal::GameRunning);
    }
    if activity.cloud_transfers {
        return Err(PathChangeRefusal::CloudTransferActive);
    }
    Ok(target)
}

/// `path` as the filesystem resolves it: its nearest existing ancestor
/// canonicalized (links and junctions followed), the missing tail joined
/// back on. `None` when nothing along the path exists.
fn resolved(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut tail = Vec::new();
    loop {
        if let Ok(canonical) = fs::canonicalize(existing) {
            let mut out = canonical;
            for name in tail.iter().rev() {
                out.push(name);
            }
            return Some(out);
        }
        tail.push(existing.file_name()?.to_os_string());
        existing = existing.parent()?;
    }
}

/// The nearest ancestor of `path` (itself included) that exists.
fn nearest_existing(path: &Path) -> Option<&Path> {
    path.ancestors()
        .find(|ancestor| fs::symlink_metadata(ancestor).is_ok())
}

/// Writes and removes one probe file in `dir`.
fn probe_write(dir: &Path) -> Result<(), PathChangeRefusal> {
    let probe = dir.join(format!(".grid-launcher-write-test-{}", std::process::id()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| PathChangeRefusal::NotWritable)?;
    fs::remove_file(&probe).map_err(|_| PathChangeRefusal::NotWritable)
}

/// [`check_new_library_path`], then the same three comparisons on the
/// paths as the filesystem resolves them (a symlink or junction can put a
/// lexically separate folder inside the current root), then the target
/// itself: an existing path must be a writable directory; a missing one
/// must have a directory as its nearest existing ancestor. Creates nothing
/// that outlives the call.
pub fn validate_new_library_path(
    current: &str,
    candidate: &str,
    activity: LibraryActivity,
) -> Result<PathBuf, PathChangeRefusal> {
    let home = crate::platform::home_dir();
    let target = check_new_library_path(current, candidate, activity, home.as_deref())?;

    if !current.trim().is_empty() {
        let current_root = expand(current, home.as_deref());
        let keys = (
            resolved(&current_root).and_then(|p| removal_key(&p, None)),
            resolved(&target).and_then(|p| removal_key(&p, None)),
        );
        if let (Some(current_key), Some(target_key)) = keys {
            compare_keys(&current_key, &target_key)?;
        }
    }

    match fs::metadata(&target) {
        Ok(meta) if meta.is_dir() => probe_write(&target)?,
        Ok(_) => return Err(PathChangeRefusal::NotADirectory),
        Err(_) => {
            let ancestor = nearest_existing(&target).ok_or(PathChangeRefusal::NotCreatable)?;
            if !ancestor.is_dir() {
                return Err(PathChangeRefusal::NotCreatable);
            }
        }
    }
    Ok(target)
}

/// Creates `root` (and its parents) and proves a file can be written in
/// it. Runs right before a change commits, so a folder that validated but
/// cannot be created stops the change before any row or file is touched.
pub fn prepare_library_dir(root: &Path) -> Result<(), PathChangeRefusal> {
    if root.exists() && !root.is_dir() {
        return Err(PathChangeRefusal::NotADirectory);
    }
    fs::create_dir_all(root).map_err(|_| PathChangeRefusal::NotCreatable)?;
    probe_write(root)
}

/// The config edit every path change ends with: `library_path` becomes
/// `new_root`; the old root (when set, and not already listed) is appended
/// to `former_library_paths`; `new_root` leaves that list if it was there
/// (it is the current root again); the layout version is stamped for the
/// new folder's contents ([`layout_version_for_library_path`]).
///
/// The seam phase 9b's move job reuses once its files have moved.
pub fn switch_library_root(config: &mut Config, new_root: &Path) {
    let home = crate::platform::home_dir();
    let key = |raw: &str| removal_key(&expand(raw, home.as_deref()), home.as_deref());
    let new_text = new_root.to_string_lossy().into_owned();
    let new_key = key(&new_text);

    let old = config.library_path.trim().to_string();
    let mut former = std::mem::take(&mut config.former_library_paths);
    if !old.is_empty() {
        let old_key = key(&old);
        if !former.iter().any(|f| key(f) == old_key) {
            former.push(old);
        }
    }
    former.retain(|f| !f.trim().is_empty() && key(f) != new_key);

    config.former_library_paths = former;
    config.library_layout_version = layout_version_for_library_path(&new_text);
    config.library_path = new_text;
}

/// The current root (when set) and every former root, with `~/` expanded:
/// what the uninstall `RemovalGuard` protects.
pub fn protected_library_roots(config: &Config) -> Vec<PathBuf> {
    std::iter::once(&config.library_path)
        .chain(&config.former_library_paths)
        .filter(|raw| !raw.trim().is_empty())
        .map(|raw| super::paths::expand_home(raw.trim()))
        .collect()
}

/// True when `path` lies strictly inside `root`, compared lexically the
/// way the `RemovalGuard` compares (case-folded on Windows). Never true for
/// `root` itself, a relative path or a path with `..`.
pub fn is_strictly_inside(path: &Path, root: &Path, home: Option<&Path>) -> bool {
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    match (removal_key(path, home), removal_key(root, home)) {
        (Some(path_key), Some(root_key)) => {
            path_key.len() > root_key.len() && path_key.starts_with(&root_key)
        }
        _ => false,
    }
}

/// True when `row`'s install lives in the library at `root`: one of its
/// recorded install locations (archive, extraction, multi-file dir, native
/// game dir, PS3 ISO) is inside `root`, or it records none at all (such a
/// row is resolved against the library root, so it belongs to it).
pub fn row_lives_under(row: &InstalledGame, root: &Path, home: Option<&Path>) -> bool {
    let locations: Vec<&str> = [
        &row.archive_path,
        &row.extracted_path,
        &row.extracted_dir,
        &row.multi_file_game_dir,
        &row.native_game_dir,
        &row.ps3_iso_path,
    ]
    .into_iter()
    .map(|raw| raw.trim())
    .filter(|raw| !raw.is_empty())
    .collect();
    locations.is_empty()
        || locations
            .iter()
            .any(|raw| is_strictly_inside(Path::new(raw), root, home))
}

/// The rows Start fresh takes out of the library: every row that
/// [`row_lives_under`] the old root, except the hidden Emulators-platform
/// package rows — those back an emulator entry, and emulators are left
/// untouched by a path change.
pub fn start_fresh_rows(
    rows: &[InstalledGame],
    old_root: &Path,
    home: Option<&Path>,
) -> Vec<InstalledGame> {
    rows.iter()
        .filter(|row| !is_emulators_platform(&row.platform))
        .filter(|row| row_lives_under(row, old_root, home))
        .cloned()
        .collect()
}

/// Bytes on disk under `path`: a file's length, or a directory's files
/// summed. Symlinks and junctions are counted as nothing and never
/// followed, so the total is what a removal would actually free. A path
/// that cannot be read counts as 0.
pub fn disk_usage(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() || is_reparse_point(&meta) {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| disk_usage(&entry.path()))
        .sum()
}

/// True for any NTFS reparse point (a junction, a symlink, a cloud
/// placeholder): never descended into. Always false off Windows.
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

/// One game Start fresh › Delete would remove: what the confirmation lists.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StartFreshGame {
    pub title: String,
    pub platform: String,
    /// The files and folders its uninstall removes, all inside the old root.
    pub paths: Vec<String>,
    pub bytes: u64,
}

/// What Start fresh › Delete would do, computed before anything changes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StartFreshPreview {
    pub old_root: String,
    pub games: Vec<StartFreshGame>,
    pub total_bytes: u64,
    /// Paths an ordinary uninstall of these rows would also remove but that
    /// lie outside the old root (a PS3 game's routed folders in an RPCS3
    /// elsewhere, for example). Start fresh leaves them on disk.
    pub left_outside: Vec<String>,
}

/// What a Start fresh run did.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct StartFreshReport {
    /// Rows taken out of the library.
    pub rows_removed: usize,
    /// One line per path that could not be removed. A row with a failure
    /// stays in the library, so nothing is orphaned.
    pub failures: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute path: `C:\<rest>` on Windows, `/<rest>` elsewhere.
    fn abs(rest: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:/{rest}"))
        } else {
            PathBuf::from(format!("/{rest}"))
        }
    }

    fn s(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    const IDLE: LibraryActivity = LibraryActivity {
        installs: false,
        running_games: false,
        cloud_transfers: false,
    };

    // --- check_new_library_path ---------------------------------------------

    #[test]
    fn check_table() {
        let current = s(&abs("games/lib"));
        let home = abs("home/u");
        let cases: Vec<(String, Result<PathBuf, PathChangeRefusal>)> = vec![
            ("".into(), Err(PathChangeRefusal::Blank)),
            ("   ".into(), Err(PathChangeRefusal::Blank)),
            ("relative/lib".into(), Err(PathChangeRefusal::NotAbsolute)),
            ("lib".into(), Err(PathChangeRefusal::NotAbsolute)),
            (s(&abs("games/lib")), Err(PathChangeRefusal::SameAsCurrent)),
            (s(&abs("games/lib/")), Err(PathChangeRefusal::SameAsCurrent)),
            (
                s(&abs("games/x/../lib")),
                Err(PathChangeRefusal::SameAsCurrent),
            ),
            (
                s(&abs("games/lib/new")),
                Err(PathChangeRefusal::InsideCurrent),
            ),
            (
                s(&abs("games/lib/games")),
                Err(PathChangeRefusal::InsideCurrent),
            ),
            (s(&abs("games")), Err(PathChangeRefusal::ContainsCurrent)),
            (s(&abs("")), Err(PathChangeRefusal::ContainsCurrent)),
            (s(&abs("games/lib2")), Ok(abs("games/lib2"))),
            (s(&abs("other/lib")), Ok(abs("other/lib"))),
            ("~/Games".into(), Ok(home.join("Games"))),
        ];
        for (candidate, expected) in cases {
            assert_eq!(
                check_new_library_path(&current, &candidate, IDLE, Some(&home)),
                expected,
                "candidate {candidate:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn check_compares_without_case_on_windows() {
        assert_eq!(
            check_new_library_path("C:\\Games\\Lib", "c:\\games\\lib", IDLE, None),
            Err(PathChangeRefusal::SameAsCurrent)
        );
        assert_eq!(
            check_new_library_path("C:\\Games\\Lib", "C:\\GAMES\\LIB\\New", IDLE, None),
            Err(PathChangeRefusal::InsideCurrent)
        );
    }

    #[test]
    fn check_without_a_current_root_only_needs_an_absolute_path() {
        assert_eq!(
            check_new_library_path("", &s(&abs("lib")), IDLE, None),
            Ok(abs("lib"))
        );
        assert_eq!(
            check_new_library_path("  ", "lib", IDLE, None),
            Err(PathChangeRefusal::NotAbsolute)
        );
    }

    #[test]
    fn check_refuses_while_the_library_is_busy() {
        let current = s(&abs("lib"));
        let target = s(&abs("lib2"));
        let busy = [
            (
                LibraryActivity {
                    installs: true,
                    ..IDLE
                },
                PathChangeRefusal::InstallActive,
            ),
            (
                LibraryActivity {
                    running_games: true,
                    ..IDLE
                },
                PathChangeRefusal::GameRunning,
            ),
            (
                LibraryActivity {
                    cloud_transfers: true,
                    ..IDLE
                },
                PathChangeRefusal::CloudTransferActive,
            ),
        ];
        for (activity, expected) in busy {
            assert_eq!(
                check_new_library_path(&current, &target, activity, None),
                Err(expected)
            );
        }
    }

    #[test]
    fn every_refusal_has_a_message() {
        for refusal in [
            PathChangeRefusal::Blank,
            PathChangeRefusal::NotAbsolute,
            PathChangeRefusal::SameAsCurrent,
            PathChangeRefusal::InsideCurrent,
            PathChangeRefusal::ContainsCurrent,
            PathChangeRefusal::NotADirectory,
            PathChangeRefusal::NotCreatable,
            PathChangeRefusal::NotWritable,
            PathChangeRefusal::InstallActive,
            PathChangeRefusal::GameRunning,
            PathChangeRefusal::CloudTransferActive,
        ] {
            assert!(refusal.message().ends_with('.'), "{refusal:?}");
        }
    }

    // --- validate_new_library_path / prepare_library_dir --------------------

    #[test]
    fn validate_accepts_an_existing_empty_dir_and_a_missing_one() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("lib");
        fs::create_dir(&current).unwrap();
        let existing = dir.path().join("new");
        fs::create_dir(&existing).unwrap();
        let missing = dir.path().join("a/b/c");

        assert_eq!(
            validate_new_library_path(&s(&current), &s(&existing), IDLE),
            Ok(existing.clone())
        );
        assert_eq!(
            validate_new_library_path(&s(&current), &s(&missing), IDLE),
            Ok(missing.clone())
        );
        assert!(!missing.exists(), "validation must not create the folder");
        assert_eq!(
            fs::read_dir(&existing).unwrap().count(),
            0,
            "the write probe must not be left behind"
        );
    }

    #[test]
    fn validate_refuses_a_file_and_a_path_under_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("lib");
        fs::create_dir(&current).unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, b"x").unwrap();

        assert_eq!(
            validate_new_library_path(&s(&current), &s(&file), IDLE),
            Err(PathChangeRefusal::NotADirectory)
        );
        assert_eq!(
            validate_new_library_path(&s(&current), &s(&file.join("sub")), IDLE),
            Err(PathChangeRefusal::NotCreatable)
        );
    }

    #[test]
    fn validate_runs_the_lexical_checks_first() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("lib");
        fs::create_dir(&current).unwrap();
        assert_eq!(
            validate_new_library_path(&s(&current), &s(&current.join("inner")), IDLE),
            Err(PathChangeRefusal::InsideCurrent)
        );
        assert_eq!(
            validate_new_library_path(&s(&current), &s(&current), IDLE),
            Err(PathChangeRefusal::SameAsCurrent)
        );
    }

    /// A link that lexically sits outside the current root but resolves
    /// inside it is refused.
    #[cfg(unix)]
    #[test]
    fn validate_refuses_a_symlink_into_the_current_root() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("lib");
        fs::create_dir_all(current.join("inner")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(current.join("inner"), &link).unwrap();
        assert_eq!(
            validate_new_library_path(&s(&current), &s(&link), IDLE),
            Err(PathChangeRefusal::InsideCurrent)
        );
    }

    #[test]
    fn prepare_creates_the_folder_and_leaves_it_empty() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("x/y");
        assert_eq!(prepare_library_dir(&target), Ok(()));
        assert!(target.is_dir());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    }

    #[test]
    fn prepare_refuses_a_path_under_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, b"x").unwrap();
        assert_eq!(
            prepare_library_dir(&file.join("sub")),
            Err(PathChangeRefusal::NotCreatable)
        );
    }

    // --- switch_library_root / protected_library_roots ----------------------

    #[test]
    fn switch_appends_the_old_root_once_and_stamps_the_layout() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let new = dir.path().join("new");
        let mut config = Config {
            library_path: s(&old),
            library_layout_version: 2,
            ..Default::default()
        };

        switch_library_root(&mut config, &new);
        assert_eq!(config.library_path, s(&new));
        assert_eq!(config.former_library_paths, vec![s(&old)]);
        // A missing folder starts in the current layout.
        assert_eq!(config.library_layout_version, 2);

        // Back to the old root: it is current again, so it leaves the list,
        // and `new` joins it.
        switch_library_root(&mut config, &old);
        assert_eq!(config.library_path, s(&old));
        assert_eq!(config.former_library_paths, vec![s(&new)]);

        // A third root: the list grows, without duplicates.
        let third = dir.path().join("third");
        switch_library_root(&mut config, &third);
        switch_library_root(&mut config, &new);
        switch_library_root(&mut config, &third);
        assert_eq!(config.former_library_paths, vec![s(&old), s(&new)]);
    }

    #[test]
    fn switch_stamps_a_legacy_folder_as_version_0() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("legacy");
        fs::create_dir_all(legacy.join("SNES")).unwrap();
        let mut config = Config {
            library_path: s(&dir.path().join("old")),
            library_layout_version: 2,
            ..Default::default()
        };
        switch_library_root(&mut config, &legacy);
        assert_eq!(config.library_layout_version, 0);
    }

    #[test]
    fn switch_from_no_root_lists_no_former_root() {
        let mut config = Config::default();
        switch_library_root(&mut config, &abs("lib"));
        assert_eq!(config.library_path, s(&abs("lib")));
        assert!(config.former_library_paths.is_empty());
    }

    #[test]
    fn protected_roots_are_the_current_and_every_former_root() {
        let config = Config {
            library_path: s(&abs("now")),
            former_library_paths: vec![s(&abs("a")), "  ".into(), s(&abs("b"))],
            ..Default::default()
        };
        assert_eq!(
            protected_library_roots(&config),
            vec![abs("now"), abs("a"), abs("b")]
        );
        assert!(protected_library_roots(&Config::default()).is_empty());
    }

    // --- rows ---------------------------------------------------------------

    fn row(title: &str, platform: &str) -> InstalledGame {
        InstalledGame {
            title: title.into(),
            platform: platform.into(),
            ..Default::default()
        }
    }

    #[test]
    fn is_strictly_inside_table() {
        let root = abs("lib");
        assert!(is_strictly_inside(&abs("lib/games/x"), &root, None));
        assert!(!is_strictly_inside(&abs("lib"), &root, None));
        assert!(!is_strictly_inside(&abs("lib2/x"), &root, None));
        assert!(!is_strictly_inside(&abs("lib/../x"), &root, None));
        assert!(!is_strictly_inside(Path::new("lib/x"), &root, None));
    }

    #[test]
    fn start_fresh_takes_rows_under_the_old_root_only() {
        let old = abs("old");
        let under = InstalledGame {
            extracted_dir: s(&abs("old/games/SNES/Mario")),
            archive_path: s(&abs("old/games/SNES/Mario.zip")),
            ..row("Mario", "SNES")
        };
        let native_under = InstalledGame {
            native_game_dir: s(&abs("old/games/Windows/Doom")),
            ..row("Doom", "Windows")
        };
        let ps3_under = InstalledGame {
            ps3_iso_path: s(&abs("old/games/PS3/Game.iso")),
            ..row("PS3 Game", "PlayStation 3")
        };
        let elsewhere = InstalledGame {
            native_game_dir: s(&abs("D/Games/Quake")),
            ..row("Quake", "Windows")
        };
        let no_paths = row("Legacy", "SNES");
        let server_emulator = InstalledGame {
            extracted_dir: s(&abs("old/games/Emulators/pkg")),
            ..row("Some Emu", "Emulators")
        };
        let rows = vec![
            under.clone(),
            native_under.clone(),
            ps3_under.clone(),
            elsewhere,
            no_paths.clone(),
            server_emulator,
        ];
        let picked: Vec<String> = start_fresh_rows(&rows, &old, None)
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(picked, vec!["Mario", "Doom", "PS3 Game", "Legacy"]);
    }

    // --- disk_usage ---------------------------------------------------------

    #[test]
    fn disk_usage_sums_files_and_skips_links() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("sub")).unwrap();
        fs::write(game.join("a.bin"), vec![0u8; 1000]).unwrap();
        fs::write(game.join("sub/b.bin"), vec![0u8; 234]).unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("big.bin"), vec![0u8; 5000]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, game.join("link")).unwrap();
        #[cfg(windows)]
        junction::create(&outside, game.join("link")).unwrap();

        assert_eq!(disk_usage(&game), 1234);
        assert_eq!(disk_usage(&game.join("a.bin")), 1000);
        assert_eq!(disk_usage(&dir.path().join("missing")), 0);
    }
}
