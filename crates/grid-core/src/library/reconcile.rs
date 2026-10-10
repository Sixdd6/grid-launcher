//! Moved-file detection (Q2 phase 9c).
//!
//! On startup and on every Library refresh, each installed row's install is
//! checked on disk:
//!
//! - **present**: the row's anchor (the one recorded path that stands for
//!   the whole install, [`anchor`]) exists — nothing to do;
//! - **moved**: the anchor is gone, the row sat at a layout location of a
//!   library root (`games/<Platform>/`, the legacy `<root>/<Platform>/`, or
//!   the root itself), and an entry of the same kind and name is found at
//!   one of those locations, or at a candidate the `paths` helpers derive
//!   from the server file name — the row is re-pointed there
//!   ([`repoint`], one registry transaction);
//! - **missing**: anything else. Computed on every pass and never stored:
//!   a drive that comes back, or a folder the user puts back, clears it on
//!   the next refresh with no write.
//!
//! Only the library roots handed in are searched, and only at the fixed
//! layout locations: there is no disk-wide scan, and nothing is ever
//! deleted. A row whose anchor lies anywhere else — a native game installed
//! outside the library, a PS3 game routed into RPCS3's `dev_hdd0` — is only
//! checked, never searched or re-pointed.
//!
//! [`plan`] is the pure decision; the caller applies each
//! [`RowState::Moved`] with `Registry::rewrite_row_paths` and [`repoint`].
//! Every function takes the roots as a list, so a future multi-library
//! setup passes all of its roots.
//!
//! [`locate_target`] checks a folder the user picked with Locate…. It may
//! lie anywhere: the user chose it explicitly.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::path_change::{expand, is_strictly_inside};
use super::paths::{
    archive_name, candidate_archives, candidate_extracted_dirs, is_plain_component,
    legacy_platform_dir, platform_dir,
};
use super::registry::InstalledGame;
use super::removal_key;
use super::update_detection::is_emulators_platform;

/// What an entry on disk is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
}

/// The filesystem probe [`plan`] and [`locate_target`] use in production:
/// links followed, anything that is neither a file nor a folder is `None`.
pub fn fs_probe(path: &Path) -> Option<EntryKind> {
    let meta = fs::metadata(path).ok()?;
    if meta.is_dir() {
        Some(EntryKind::Dir)
    } else if meta.is_file() {
        Some(EntryKind::File)
    } else {
        None
    }
}

/// Which column a row's [`Anchor`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorColumn {
    MultiFileGameDir,
    NativeGameDir,
    ExtractedDir,
    ArchivePath,
    Ps3IsoPath,
    ExtractedPath,
}

/// The one recorded path that stands for a row's whole install: every
/// other recorded path of the row lies inside it or is it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub column: AnchorColumn,
}

/// The row's anchor, `~/` expanded against `home`: the first non-blank of
/// the multi-file game folder, the native game's home folder, the
/// extraction folder, the archive, the PS3 ISO and the launch file. `None`
/// for a row that records no location at all (it is resolved against the
/// library root by launch, so there is nothing of its own to check).
pub fn anchor(row: &InstalledGame, home: Option<&Path>) -> Option<Anchor> {
    use AnchorColumn::*;
    let columns = [
        (&row.multi_file_game_dir, EntryKind::Dir, MultiFileGameDir),
        (&row.native_game_dir, EntryKind::Dir, NativeGameDir),
        (&row.extracted_dir, EntryKind::Dir, ExtractedDir),
        (&row.archive_path, EntryKind::File, ArchivePath),
        (&row.ps3_iso_path, EntryKind::File, Ps3IsoPath),
        (&row.extracted_path, EntryKind::File, ExtractedPath),
    ];
    columns
        .into_iter()
        .find(|(raw, _, _)| !raw.trim().is_empty())
        .map(|(raw, kind, column)| Anchor {
            path: expand(raw, home),
            kind,
            column,
        })
}

/// True when `path` sits directly in one of the places a library root
/// keeps installs for `platform`: `games/<Platform>/`, the legacy
/// `<root>/<Platform>/`, or the root itself.
fn at_layout_location(path: &Path, root: &Path, platform: &str, home: Option<&Path>) -> bool {
    let Some(parent) = path.parent().and_then(|p| removal_key(p, home)) else {
        return false;
    };
    [
        platform_dir(root, platform),
        legacy_platform_dir(root, platform),
        root.to_path_buf(),
    ]
    .iter()
    .any(|place| removal_key(place, home).as_ref() == Some(&parent))
}

/// Where a moved anchor may now be, under `root`, in search order: the
/// anchor's own name at the three layout locations, then the candidates
/// the `paths` helpers derive from the server file name (archives for a
/// file anchor, extraction folders for an extraction-folder anchor).
fn search_candidates(row: &InstalledGame, anchor: &Anchor, root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(name) = anchor.path.file_name() {
        candidates.push(platform_dir(root, &row.platform).join(name));
        candidates.push(legacy_platform_dir(root, &row.platform).join(name));
        candidates.push(root.join(name));
    }
    let name = archive_name(&row.rom_file_name, &row.title, &row.platform);
    if is_plain_component(&name) {
        let archives = candidate_archives(root, &row.platform, "", &name, "");
        match anchor.column {
            AnchorColumn::ArchivePath | AnchorColumn::Ps3IsoPath | AnchorColumn::ExtractedPath => {
                candidates.extend(archives)
            }
            AnchorColumn::ExtractedDir => {
                candidates.extend(candidate_extracted_dirs(&archives, "", ""))
            }
            AnchorColumn::MultiFileGameDir | AnchorColumn::NativeGameDir => {}
        }
    }
    candidates
}

/// One row's state. See the module docs.
fn decide(
    row: &InstalledGame,
    roots: &[PathBuf],
    probe: &dyn Fn(&Path) -> Option<EntryKind>,
    home: Option<&Path>,
) -> RowState {
    if is_emulators_platform(&row.platform) {
        return RowState::NotChecked;
    }
    let Some(anchor) = anchor(row, home) else {
        return RowState::NotChecked;
    };
    if probe(&anchor.path) == Some(anchor.kind) {
        return RowState::Present;
    }
    let Some(root) = roots.iter().find(|root| {
        is_strictly_inside(&anchor.path, root, home)
            && at_layout_location(&anchor.path, root, &row.platform, home)
    }) else {
        return RowState::Missing;
    };
    let old_key = removal_key(&anchor.path, home);
    search_candidates(row, &anchor, root)
        .into_iter()
        .find(|candidate| {
            probe(candidate) == Some(anchor.kind)
                && is_strictly_inside(candidate, root, home)
                && removal_key(candidate, home) != old_key
        })
        .map_or(RowState::Missing, |to| RowState::Moved {
            from: anchor.path.clone(),
            to,
        })
}

/// Where a row stands. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    Present,
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    Missing,
    /// Not judged: an emulator package row, a row that records no
    /// location, or a row an install is writing right now.
    NotChecked,
}

/// One row's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowDecision {
    pub title: String,
    pub platform: String,
    pub state: RowState,
}

/// What one applied pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// Rows re-pointed to where their files are now.
    pub moved: usize,
    /// The `(title_key, platform_key)` of every row whose files are gone.
    pub missing: Vec<(String, String)>,
}

/// Whether a pass may run at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileGate {
    Ready,
    /// A library move runs or waits to resume: its own records are the
    /// truth until it finishes.
    MoveActive,
    /// The library root cannot be reached: every row would look missing.
    /// The unreachable-library dialog owns that case.
    RootUnreachable,
}

/// The pass's decisions. Empty unless `gate` is [`ReconcileGate::Ready`]:
/// a closed gate is a no-op, and no row is called missing.
///
/// `busy` names rows an install or update is writing right now; they are
/// [`RowState::NotChecked`]. `probe` answers what is on disk.
pub fn plan(
    rows: &[InstalledGame],
    roots: &[PathBuf],
    gate: ReconcileGate,
    busy: &dyn Fn(&InstalledGame) -> bool,
    probe: &dyn Fn(&Path) -> Option<EntryKind>,
    home: Option<&Path>,
) -> Vec<RowDecision> {
    if gate != ReconcileGate::Ready {
        return Vec::new();
    }
    rows.iter()
        .map(|row| RowDecision {
            title: row.title.clone(),
            platform: row.platform.clone(),
            state: if busy(row) {
                RowState::NotChecked
            } else {
                decide(row, roots, probe, home)
            },
        })
        .collect()
}

/// The normal components of `path` in their own spelling, exactly the ones
/// `removal_key` keys on (a `..` pops, `.` and — on Windows — a name made
/// only of dots and spaces are dropped). `None` for a path that is not
/// absolute.
fn key_names(path: &Path) -> Option<Vec<std::ffi::OsString>> {
    let mut rooted = false;
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::CurDir => {}
            Component::RootDir => rooted = true,
            Component::ParentDir => {
                names.pop()?;
            }
            Component::Normal(name) => {
                let dots_only = cfg!(windows)
                    && name
                        .to_string_lossy()
                        .trim_end_matches(['.', ' '])
                        .is_empty();
                if !dots_only {
                    names.push(name.to_os_string());
                }
            }
        }
    }
    rooted.then_some(names)
}

/// `value` with the prefix `from` replaced by `to`, or `None` when `value`
/// does not lie at or under `from`. Compared the way the removal guard
/// compares (case-folded on Windows); `~/` expanded against `home`.
pub fn repoint(value: &str, from: &Path, to: &Path, home: Option<&Path>) -> Option<String> {
    let value = expand(value, home);
    let value_key = removal_key(&value, home)?;
    let from_key = removal_key(from, home)?;
    if !value_key.starts_with(&from_key) {
        return None;
    }
    // The key's first slot is the prefix; the rest are the names.
    let names = key_names(&value)?;
    let skip = from_key.len() - 1;
    let mut out = to.to_path_buf();
    for name in names.get(skip..)? {
        out.push(name);
    }
    Some(out.to_string_lossy().into_owned())
}

/// Why a folder picked with Locate… cannot hold the row's install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateRefusal {
    /// The row records no install location to look for.
    NoInstallLocation,
    /// The picked path is not a folder.
    NotAFolder,
    /// The folder holds no entry named like the install (or, for a game
    /// folder, not the launch file the row records).
    NoMatch { name: String },
}

impl LocateRefusal {
    pub fn message(&self) -> String {
        match self {
            Self::NoInstallLocation => {
                "This game records no install location, so there is nothing to look for.".into()
            }
            Self::NotAFolder => "Choose a folder.".into(),
            Self::NoMatch { name } => {
                format!("That folder does not contain “{name}”. Choose the folder that holds it.")
            }
        }
    }
}

/// The new anchor for `row` when the user picks `folder` with Locate….
///
/// Accepted, in this order: `folder/<anchor name>` of the anchor's kind;
/// or, for a folder anchor, `folder` itself when its name is the anchor's
/// name or it holds the launch file the row records inside its anchor.
/// When the row records a launch file inside a folder anchor, that file
/// must be there too. `folder` may lie anywhere.
pub fn locate_target(
    row: &InstalledGame,
    folder: &Path,
    probe: &dyn Fn(&Path) -> Option<EntryKind>,
    home: Option<&Path>,
) -> Result<PathBuf, LocateRefusal> {
    let anchor = anchor(row, home).ok_or(LocateRefusal::NoInstallLocation)?;
    if probe(folder) != Some(EntryKind::Dir) {
        return Err(LocateRefusal::NotAFolder);
    }
    let name = anchor
        .path
        .file_name()
        .ok_or(LocateRefusal::NoInstallLocation)?
        .to_os_string();

    // The launch file the row records inside a folder anchor: it must be
    // at the same place under the new folder.
    let launch = (anchor.kind == EntryKind::Dir)
        .then(|| {
            [&row.extracted_path, &row.native_executable_path]
                .into_iter()
                .map(|raw| expand(raw, home))
                .find(|path| is_strictly_inside(path, &anchor.path, home))
        })
        .flatten();
    let holds_launch = |candidate: &Path| match &launch {
        Some(file) => repoint(&file.to_string_lossy(), &anchor.path, candidate, home)
            .is_some_and(|moved| probe(Path::new(&moved)) == Some(EntryKind::File)),
        None => true,
    };

    let inside = folder.join(&name);
    if probe(&inside) == Some(anchor.kind) && holds_launch(&inside) {
        return Ok(inside);
    }
    if anchor.kind == EntryKind::Dir {
        let same_name = match (removal_key(folder, home), removal_key(&anchor.path, home)) {
            (Some(a), Some(b)) => a.last() == b.last(),
            _ => false,
        };
        if (same_name || launch.is_some()) && holds_launch(folder) {
            return Ok(folder.to_path_buf());
        }
    }
    Err(LocateRefusal::NoMatch {
        name: name.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const SNES: &str = "Super Nintendo Entertainment System";

    fn mkfile(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// An absolute folder that does not exist, on every OS.
    fn gone() -> PathBuf {
        std::env::temp_dir().join("grid-reconcile-test-gone")
    }

    /// An extracted SNES row whose folder is `dir` (launch file `game.sfc`).
    fn extracted_row(title: &str, dir: &Path) -> InstalledGame {
        InstalledGame {
            title: title.to_string(),
            platform: SNES.to_string(),
            rom_id: Some(1),
            rom_file_name: "game.zip".to_string(),
            extracted_dir: text(dir),
            extracted_path: text(&dir.join("game.sfc")),
            ..Default::default()
        }
    }

    fn run(rows: &[InstalledGame], root: &Path, gate: ReconcileGate) -> Vec<RowState> {
        plan(
            rows,
            &[root.to_path_buf()],
            gate,
            &|_| false,
            &fs_probe,
            None,
        )
        .into_iter()
        .map(|d| d.state)
        .collect()
    }

    // --- anchor -------------------------------------------------------------

    #[test]
    fn anchor_prefers_the_outermost_recorded_location() {
        let mut row = InstalledGame {
            extracted_path: "/l/g/P/T/game/x.exe".into(),
            extracted_dir: "/l/g/P/T/game".into(),
            native_game_dir: "/l/g/P/T".into(),
            ..Default::default()
        };
        let a = anchor(&row, None).unwrap();
        assert_eq!(
            (a.path, a.kind, a.column),
            (
                PathBuf::from("/l/g/P/T"),
                EntryKind::Dir,
                AnchorColumn::NativeGameDir
            )
        );
        row.native_game_dir.clear();
        assert_eq!(
            anchor(&row, None).unwrap().column,
            AnchorColumn::ExtractedDir
        );
        row.extracted_dir.clear();
        let a = anchor(&row, None).unwrap();
        assert_eq!(
            (a.kind, a.column),
            (EntryKind::File, AnchorColumn::ExtractedPath)
        );
        let archive = InstalledGame {
            archive_path: "/l/g/P/Game.zip".into(),
            ..Default::default()
        };
        assert_eq!(
            anchor(&archive, None).unwrap().column,
            AnchorColumn::ArchivePath
        );
        let multi = InstalledGame {
            multi_file_game_dir: "/l/g/P/T".into(),
            extracted_path: "/l/g/P/T/game.m3u".into(),
            ..Default::default()
        };
        assert_eq!(
            anchor(&multi, None).unwrap().column,
            AnchorColumn::MultiFileGameDir
        );
        assert_eq!(anchor(&InstalledGame::default(), None), None);
    }

    // --- the reconcile table -------------------------------------------------

    #[test]
    fn a_row_whose_files_are_there_is_present() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("games").join(SNES).join("Mario");
        mkfile(&dir.join("game.sfc"));
        let rows = [extracted_row("Mario", &dir)];
        assert_eq!(
            run(&rows, root.path(), ReconcileGate::Ready),
            [RowState::Present]
        );
    }

    #[test]
    fn a_folder_moved_to_the_current_layout_location_is_re_pointed() {
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join(SNES).join("Mario");
        let now = root.path().join("games").join(SNES).join("Mario");
        mkfile(&now.join("game.sfc"));
        let rows = [extracted_row("Mario", &recorded)];
        assert_eq!(
            run(&rows, root.path(), ReconcileGate::Ready),
            [RowState::Moved {
                from: recorded,
                to: now
            }]
        );
    }

    #[test]
    fn a_folder_moved_to_a_legacy_location_is_re_pointed() {
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join("games").join(SNES).join("Mario");
        let now = root.path().join(SNES).join("Mario");
        mkfile(&now.join("game.sfc"));
        let rows = [extracted_row("Mario", &recorded)];
        assert_eq!(
            run(&rows, root.path(), ReconcileGate::Ready),
            [RowState::Moved {
                from: recorded,
                to: now
            }]
        );
    }

    #[test]
    fn an_archive_moved_to_the_root_is_found_through_the_candidates() {
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join("games").join(SNES).join("Game.zip");
        let now = root.path().join("Game.zip");
        mkfile(&now);
        let row = InstalledGame {
            title: "Game".into(),
            platform: SNES.into(),
            rom_file_name: "Game.zip".into(),
            archive_path: text(&recorded),
            ..Default::default()
        };
        assert_eq!(
            run(&[row], root.path(), ReconcileGate::Ready),
            [RowState::Moved {
                from: recorded,
                to: now
            }]
        );
    }

    #[test]
    fn an_extraction_found_under_the_derived_archive_stem_is_re_pointed() {
        // The recorded folder had another name; the server file name's stem
        // is where `candidate_extracted_dirs` looks.
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join("games").join(SNES).join("Old Name");
        let now = root.path().join(SNES).join("game");
        mkfile(&now.join("game.sfc"));
        let rows = [extracted_row("Mario", &recorded)];
        assert_eq!(
            run(&rows, root.path(), ReconcileGate::Ready),
            [RowState::Moved {
                from: recorded,
                to: now
            }]
        );
    }

    #[test]
    fn a_row_whose_files_are_gone_is_missing() {
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join("games").join(SNES).join("Mario");
        let rows = [extracted_row("Mario", &recorded)];
        assert_eq!(
            run(&rows, root.path(), ReconcileGate::Ready),
            [RowState::Missing]
        );
    }

    #[test]
    fn a_file_where_a_folder_was_is_not_a_match() {
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join("games").join(SNES).join("Mario");
        mkfile(&root.path().join(SNES).join("Mario"));
        let rows = [extracted_row("Mario", &recorded)];
        assert_eq!(
            run(&rows, root.path(), ReconcileGate::Ready),
            [RowState::Missing]
        );
    }

    #[test]
    fn a_native_row_outside_the_root_is_only_checked() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let home = elsewhere.path().join("My Game");
        // A same-named folder inside the library must NOT capture the row.
        mkfile(
            &root
                .path()
                .join("games")
                .join("Windows")
                .join("My Game")
                .join("x.exe"),
        );
        let row = InstalledGame {
            title: "My Game".into(),
            platform: "Windows".into(),
            native_game_dir: text(&home),
            extracted_dir: text(&home.join("game")),
            native_executable_path: text(&home.join("game").join("x.exe")),
            ..Default::default()
        };
        assert_eq!(
            run(
                std::slice::from_ref(&row),
                root.path(),
                ReconcileGate::Ready
            ),
            [RowState::Missing]
        );
        mkfile(&home.join("game").join("x.exe"));
        assert_eq!(
            run(&[row], root.path(), ReconcileGate::Ready),
            [RowState::Present]
        );
    }

    #[test]
    fn a_ps3_row_in_dev_hdd0_is_only_checked() {
        let root = tempfile::tempdir().unwrap();
        let routed = root
            .path()
            .join("emulators")
            .join("RPCS3")
            .join("dev_hdd0")
            .join("game")
            .join("BLUS30001");
        mkfile(
            &root
                .path()
                .join("games")
                .join("PlayStation 3")
                .join("BLUS30001")
                .join("EBOOT.BIN"),
        );
        let row = InstalledGame {
            title: "Game".into(),
            platform: "PlayStation 3".into(),
            ps3_game_id: "BLUS30001".into(),
            extracted_dir: text(&routed),
            extracted_path: text(&routed.join("USRDIR").join("EBOOT.BIN")),
            ..Default::default()
        };
        assert_eq!(
            run(&[row], root.path(), ReconcileGate::Ready),
            [RowState::Missing]
        );
    }

    #[test]
    fn a_closed_gate_is_a_no_op() {
        let root = tempfile::tempdir().unwrap();
        let recorded = root.path().join("games").join(SNES).join("Mario");
        mkfile(&root.path().join(SNES).join("Mario").join("game.sfc"));
        let rows = [
            extracted_row("Mario", &recorded),
            extracted_row("Gone", &root.path().join("games").join(SNES).join("Gone")),
        ];
        assert!(run(&rows, root.path(), ReconcileGate::RootUnreachable).is_empty());
        assert!(run(&rows, root.path(), ReconcileGate::MoveActive).is_empty());
    }

    #[test]
    fn busy_emulator_package_and_location_less_rows_are_not_checked() {
        let root = tempfile::tempdir().unwrap();
        let gone = root.path().join("games").join(SNES).join("Gone");
        let busy = extracted_row("Busy", &gone);
        let mut package = extracted_row("RetroArch", &gone);
        package.platform = "Emulators".into();
        let bare = InstalledGame {
            title: "Bare".into(),
            platform: SNES.into(),
            ..Default::default()
        };
        let states: Vec<RowState> = plan(
            &[busy, package, bare],
            &[root.path().to_path_buf()],
            ReconcileGate::Ready,
            &|row| row.title == "Busy",
            &fs_probe,
            None,
        )
        .into_iter()
        .map(|d| d.state)
        .collect();
        assert_eq!(states, vec![RowState::NotChecked; 3]);
    }

    // --- repoint ------------------------------------------------------------

    #[test]
    fn repoint_swaps_the_prefix_and_keeps_the_tail() {
        let lib = gone().join("lib");
        let from = lib.join("games").join("SNES").join("Mario");
        let to = lib.join("SNES").join("Mario");
        assert_eq!(
            repoint(&text(&from.join("sub").join("game.sfc")), &from, &to, None),
            Some(text(&to.join("sub").join("game.sfc")))
        );
        assert_eq!(repoint(&text(&from), &from, &to, None), Some(text(&to)));
        let sibling = lib.join("games").join("SNES").join("Mario2").join("x");
        assert_eq!(repoint(&text(&sibling), &from, &to, None), None);
        assert_eq!(
            repoint(&text(&gone().join("elsewhere")), &from, &to, None),
            None
        );
        assert_eq!(repoint("relative/x", &from, &to, None), None);
    }

    #[cfg(windows)]
    #[test]
    fn repoint_ignores_case_and_separators_on_windows() {
        let from = Path::new(r"C:\Lib\games\SNES\Mario");
        let to = Path::new(r"D:\Lib\SNES\Mario");
        assert_eq!(
            repoint("c:/lib/GAMES/snes/mario/game.sfc", from, to, None),
            Some(r"D:\Lib\SNES\Mario\game.sfc".to_string())
        );
    }

    // --- locate -------------------------------------------------------------

    #[test]
    fn locate_accepts_a_folder_that_holds_the_install() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let row = extracted_row("Mario", &root.path().join("games").join(SNES).join("Mario"));
        let copy = elsewhere.path().join("Mario");
        mkfile(&copy.join("game.sfc"));
        // The containing folder, and the game folder itself, both work, and
        // a folder outside the library is fine: the user chose it.
        assert_eq!(
            locate_target(&row, elsewhere.path(), &fs_probe, None),
            Ok(copy.clone())
        );
        assert_eq!(locate_target(&row, &copy, &fs_probe, None), Ok(copy));
    }

    #[test]
    fn locate_accepts_a_renamed_game_folder_that_holds_the_launch_file() {
        let elsewhere = tempfile::tempdir().unwrap();
        let row = extracted_row("Mario", &gone().join("games").join(SNES).join("Mario"));
        let renamed = elsewhere.path().join("Mario (backup)");
        mkfile(&renamed.join("game.sfc"));
        assert_eq!(locate_target(&row, &renamed, &fs_probe, None), Ok(renamed));
    }

    #[test]
    fn locate_refuses_a_folder_without_the_install() {
        let elsewhere = tempfile::tempdir().unwrap();
        let row = extracted_row("Mario", &gone().join("games").join(SNES).join("Mario"));
        // A folder of the right name but without the launch file.
        fs::create_dir_all(elsewhere.path().join("Mario")).unwrap();
        assert_eq!(
            locate_target(&row, elsewhere.path(), &fs_probe, None),
            Err(LocateRefusal::NoMatch {
                name: "Mario".into()
            })
        );
        let file = elsewhere.path().join("note.txt");
        mkfile(&file);
        assert_eq!(
            locate_target(&row, &file, &fs_probe, None),
            Err(LocateRefusal::NotAFolder)
        );
        assert_eq!(
            locate_target(&InstalledGame::default(), elsewhere.path(), &fs_probe, None),
            Err(LocateRefusal::NoInstallLocation)
        );
    }

    #[test]
    fn locate_finds_an_archive_in_the_picked_folder() {
        let elsewhere = tempfile::tempdir().unwrap();
        let row = InstalledGame {
            archive_path: text(&gone().join("games").join(SNES).join("Game.zip")),
            ..Default::default()
        };
        assert_eq!(
            locate_target(&row, elsewhere.path(), &fs_probe, None),
            Err(LocateRefusal::NoMatch {
                name: "Game.zip".into()
            })
        );
        mkfile(&elsewhere.path().join("Game.zip"));
        assert_eq!(
            locate_target(&row, elsewhere.path(), &fs_probe, None),
            Ok(elsewhere.path().join("Game.zip"))
        );
    }
}
