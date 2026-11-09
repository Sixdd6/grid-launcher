//! Backing up a game from its install on this computer: which installs
//! qualify and what their trees hold.
//!
//! - [`locate`] is the pure rule set: does the registry row stand for an
//!   install GRID may copy, and if so what is its tree ([`LocalSource`]).
//! - [`scan_tree`] walks that tree (metadata only) and returns the
//!   [`TreePlan`] the archive writer follows, with the **exact** size of the
//!   tar it will write.
//! - [`assess`] is both, folded into the plan's [`LocalState`].
//! - [`snapshot_row`] keeps the row values a restore needs, relative to
//!   the tree.
//!
//! The archive is an uncompressed GNU tar. Every entry is appended with
//! [`tar_header`] and `tar::Builder::append_data(header, &entry.name, …)`,
//! in [`TreePlan::entries`] order, then `finish`: that is what
//! [`TreePlan::archive_bytes`] counts.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::pack::{is_reserved_name, server_key, InstalledLayout, PackInstalledRow};
use super::plan::LocalState;
use crate::library::path_change::{expand, is_strictly_inside};
use crate::library::platforms::is_xbox360_platform;
use crate::library::reconcile::{anchor, at_layout_location, repoint, AnchorColumn};
use crate::library::registry::InstalledGame;
use crate::library::removal_key;
use crate::library::update_detection::is_emulators_platform;

/// Missing in the registry pass: the anchor is gone.
pub const REASON_MISSING: &str = "Its files weren't found on this computer";
/// Not in a platform folder of the library: a native game moved with
/// Locate…, a PS3 game in RPCS3's `dev_hdd0`, a launch file elsewhere.
pub const REASON_OUTSIDE: &str = "Installed outside the library folder";
/// The tree holds a symlink or a junction.
pub const REASON_LINKS: &str = "Its install contains links";
/// A name a Windows drive cannot hold, on a pack for Windows.
pub const REASON_NOT_PORTABLE: &str = "Its install has a file name Windows can't use";
/// A name that is not UTF-8 (tar on Windows cannot write it).
pub const REASON_NOT_TEXT: &str = "Its install has a file name that isn't valid text";
/// A FIFO, socket or device file in the tree.
pub const REASON_SPECIAL: &str = "Its install contains a file GRID can't copy";
/// The tree could not be walked.
pub const REASON_UNREADABLE: &str = "Its files couldn't be read on this computer";
/// Xenia keeps Xbox 360 content outside the library.
pub const REASON_XBOX360: &str = "Xbox 360 games always download from the server";
/// Emulators-platform packages are emulators, not games.
pub const REASON_EMULATORS: &str = "Emulator packages always download from the server";
/// The row was installed from a backup of another server.
pub const REASON_OTHER_SERVER: &str = "It was installed from another server";
/// The row predates rom-id tracking, so it cannot be tied to this game.
pub const REASON_NO_ROM_ID: &str = "It isn't linked to this server's game";

/// The folder inside a native game's home that holds its Wine prefix. Never
/// copied: saves kept in the prefix stay on this computer.
pub const WINE_PREFIX_DIR: &str = "prefix";

/// The tar block size.
const BLOCK: u64 = 512;
/// The longest name a GNU header holds; a longer one gets a long-name record.
const GNU_NAME_BYTES: usize = 100;

/// An install GRID may copy: its tree and the values the pack entry needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSource {
    pub layout: InstalledLayout,
    /// The folder (or, for [`InstalledLayout::File`], the file) the archive
    /// holds. `~/` expanded.
    pub root: PathBuf,
    /// The folder or file name in the platform folder (`PackInstalled.name`).
    pub name: String,
    /// Folders under `root` left out of the archive (a native game's Wine
    /// prefix). Absolute.
    pub excludes: Vec<PathBuf>,
    /// The launch file relative to `root`, `/`-separated (`PackInstalled.launch`);
    /// `""` for [`InstalledLayout::File`] or when the row records none.
    pub launch: String,
}

/// What [`locate`] decided for a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    /// No install of this game: it downloads, with no note.
    NotInstalled,
    /// Installed, but it downloads, for this reason.
    Ineligible(String),
    Eligible(LocalSource),
}

/// Decides whether `row` (the registry row found for `rom_id`, if any) is
/// an install GRID may copy instead of downloading. Pure: the file system
/// is not touched; `missing` is the row's state in the last registry pass
/// (`InstallService::installed_rows`).
///
/// The checks, in order:
/// 1. the row is for `rom_id` (a row for another game is `NotInstalled`;
///    one with no rom id is refused) and from the live server (a blank
///    `source_server` is a server install);
/// 2. its files are present;
/// 3. it is not an Emulators-platform package, an Xbox 360 game or a PS3
///    game routed into RPCS3's `dev_hdd0`;
/// 4. its anchor ([`anchor`]) picks the layout: the multi-file folder, the
///    native game's home (less its `prefix/`), the extraction folder, or
///    the file (an archive as downloaded, a PS3 ISO);
/// 5. the anchor sits directly in the library's platform folder for the
///    row (`games/<Platform>/` or the legacy `<library>/<Platform>/`), and
///    the launch files the row records lie inside it.
pub fn locate(
    rom_id: i64,
    row: Option<&InstalledGame>,
    missing: bool,
    library: &Path,
    home: Option<&Path>,
    live_server: &str,
) -> Located {
    let Some(row) = row else {
        return Located::NotInstalled;
    };
    match row.rom_id {
        Some(id) if id == rom_id => {}
        Some(_) => return Located::NotInstalled,
        None => return Located::Ineligible(REASON_NO_ROM_ID.into()),
    }
    let source = row.source_server.trim();
    if !source.is_empty() && server_key(source) != server_key(live_server) {
        return Located::Ineligible(REASON_OTHER_SERVER.into());
    }
    if missing {
        return Located::Ineligible(REASON_MISSING.into());
    }
    if is_emulators_platform(&row.platform) {
        return Located::Ineligible(REASON_EMULATORS.into());
    }
    if is_xbox360_platform(&row.platform) {
        return Located::Ineligible(REASON_XBOX360.into());
    }
    if !row.ps3_game_id.trim().is_empty() || !row.ps3_trophy_paths.trim().is_empty() {
        return Located::Ineligible(REASON_OUTSIDE.into());
    }
    let outside = || Located::Ineligible(REASON_OUTSIDE.into());
    let Some(anchor) = anchor(row, home) else {
        return outside();
    };
    let layout = match anchor.column {
        AnchorColumn::MultiFileGameDir => InstalledLayout::Multi,
        AnchorColumn::NativeGameDir => InstalledLayout::Native,
        AnchorColumn::ExtractedDir => InstalledLayout::Extracted,
        AnchorColumn::ArchivePath | AnchorColumn::Ps3IsoPath | AnchorColumn::ExtractedPath => {
            InstalledLayout::File
        }
    };
    let root = anchor.path;
    let in_platform_folder = is_strictly_inside(&root, library, home)
        && at_layout_location(&root, library, &row.platform, home)
        && root
            .parent()
            .is_some_and(|parent| removal_key(parent, home) != removal_key(library, home));
    if !in_platform_folder {
        return outside();
    }
    let Some(name) = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
    else {
        return outside();
    };

    let mut launch = String::new();
    if layout == InstalledLayout::File {
        // The file is the install: a launch path, if any, is the file itself.
        let recorded = [&row.extracted_path, &row.archive_path, &row.ps3_iso_path];
        let root_key = removal_key(&root, home);
        if recorded
            .into_iter()
            .filter(|raw| !raw.trim().is_empty())
            .any(|raw| removal_key(&expand(raw, home), home) != root_key)
        {
            return outside();
        }
    } else {
        for raw in [&row.extracted_path, &row.native_executable_path] {
            if raw.trim().is_empty() {
                continue;
            }
            match relative_to(raw, &root, home) {
                Some(rel) if launch.is_empty() => launch = rel,
                Some(_) => {}
                None => return outside(),
            }
        }
    }

    let mut excludes = Vec::new();
    if layout == InstalledLayout::Native {
        excludes.push(root.join(WINE_PREFIX_DIR));
        let prefix = row.native_wineprefix.trim();
        if !prefix.is_empty() {
            let prefix = expand(prefix, home);
            if is_strictly_inside(&prefix, &root, home)
                && !excludes
                    .iter()
                    .any(|have| removal_key(have, home) == removal_key(&prefix, home))
            {
                excludes.push(prefix);
            }
        }
    }

    Located::Eligible(LocalSource {
        layout,
        root,
        name,
        excludes,
        launch,
    })
}

/// [`locate`] and [`scan_tree`] as the plan's [`LocalState`]. `outdated` is
/// always `None`: the caller fills it from the update set.
pub fn assess(
    rom_id: i64,
    row: Option<&InstalledGame>,
    missing: bool,
    library: &Path,
    home: Option<&Path>,
    live_server: &str,
    for_windows: bool,
) -> LocalState {
    match locate(rom_id, row, missing, library, home, live_server) {
        Located::NotInstalled => LocalState::NotInstalled,
        Located::Ineligible(reason) => LocalState::Ineligible { reason },
        Located::Eligible(source) => scanned_state(&scan_tree(&source, for_windows)),
    }
}

/// A scan's result as the plan's [`LocalState`] (`outdated` left `None`).
pub fn scanned_state(scan: &Result<TreePlan, ScanRefusal>) -> LocalState {
    match scan {
        Ok(tree) => LocalState::Eligible {
            layout: tree.layout,
            archive_bytes: tree.archive_bytes,
            outdated: None,
        },
        Err(refusal) => LocalState::Ineligible {
            reason: refusal.reason().to_string(),
        },
    }
}

/// `raw` (`~/` expanded) relative to `root`, `/`-separated, when it lies
/// strictly inside `root`.
fn relative_to(raw: &str, root: &Path, home: Option<&Path>) -> Option<String> {
    let path = expand(raw, home);
    if !is_strictly_inside(&path, root, home) {
        return None;
    }
    let rel = repoint(&path.to_string_lossy(), root, Path::new(""), home)?;
    let parts: Vec<String> = Path::new(&rel)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// What one archive entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeEntryKind {
    File,
    Dir,
}

/// One entry of the archive, in write order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Where to read it on this computer.
    pub path: PathBuf,
    /// Its name in the archive: relative to the tree root, `/`-separated,
    /// a folder ending in `/`. For [`InstalledLayout::File`] the single
    /// entry is the file's name.
    pub name: String,
    pub kind: TreeEntryKind,
    /// The length the scan saw (0 for a folder). The writer refuses a file
    /// whose length differs.
    pub size: u64,
    /// The permission bits (`0o755`, `0o644`…); the unix mode, or a default
    /// on Windows.
    pub mode: u32,
    /// Modification time, seconds since the epoch (0 when unknown).
    pub mtime: u64,
}

/// A walked tree, ready to archive. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreePlan {
    pub root: PathBuf,
    pub layout: InstalledLayout,
    pub excludes: Vec<PathBuf>,
    /// Every folder and file, a folder before what it holds, siblings in
    /// byte order of their names.
    pub entries: Vec<TreeEntry>,
    /// The exact number of bytes the tar takes.
    pub archive_bytes: u64,
    /// The files' total length: the space a restore needs.
    pub tree_bytes: u64,
    pub tree_files: u64,
}

/// Why [`scan_tree`] refused a tree. [`Self::reason`] is the user text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanRefusal {
    Link(PathBuf),
    NotPortable(String),
    NotText(PathBuf),
    Special(PathBuf),
    Unreadable(String),
}

impl ScanRefusal {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Link(_) => REASON_LINKS,
            Self::NotPortable(_) => REASON_NOT_PORTABLE,
            Self::NotText(_) => REASON_NOT_TEXT,
            Self::Special(_) => REASON_SPECIAL,
            Self::Unreadable(_) => REASON_UNREADABLE,
        }
    }
}

/// The GNU tar header [`TreeEntry`] is written with (size, mode, type and
/// mtime set; the name is set by `Builder::append_data`).
pub fn tar_header(entry: &TreeEntry) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(match entry.kind {
        TreeEntryKind::File => tar::EntryType::Regular,
        TreeEntryKind::Dir => tar::EntryType::Directory,
    });
    header.set_size(entry.size);
    header.set_mode(entry.mode);
    header.set_mtime(entry.mtime);
    header.set_uid(0);
    header.set_gid(0);
    header
}

/// The bytes one entry takes in the archive: a long-name record when the
/// name is over 100 bytes, the header, the data padded to 512.
pub fn entry_archive_bytes(name: &str, size: u64) -> u64 {
    let long_name = if name.len() > GNU_NAME_BYTES {
        BLOCK + padded(name.len() as u64 + 1)
    } else {
        0
    };
    long_name + BLOCK + padded(size)
}

fn padded(len: u64) -> u64 {
    len.div_ceil(BLOCK) * BLOCK
}

/// Walks `source`'s tree without following links. Refuses a symlink or
/// junction anywhere (the root included), a FIFO, socket or device file, a
/// name that is not UTF-8 and, when the pack is for Windows, a name Windows
/// cannot hold (`<>:"\|?*` or a control character, a reserved device name,
/// a trailing space or dot, over 255 bytes, or two names in one folder
/// that differ only in case). The excluded folders are left out.
pub fn scan_tree(source: &LocalSource, for_windows: bool) -> Result<TreePlan, ScanRefusal> {
    let root = &source.root;
    let meta = fs::symlink_metadata(root).map_err(unreadable)?;
    if meta.file_type().is_symlink() {
        return Err(ScanRefusal::Link(root.clone()));
    }
    let mut entries = Vec::new();
    if meta.is_file() {
        let name = check_name(root, root.file_name().unwrap_or_default(), for_windows)?;
        entries.push(entry(root.clone(), name, TreeEntryKind::File, &meta));
    } else if meta.is_dir() {
        let excluded: HashSet<Vec<String>> = source
            .excludes
            .iter()
            .filter_map(|path| removal_key(path, None))
            .collect();
        walk(root, "", &excluded, for_windows, &mut entries)?;
    } else {
        return Err(ScanRefusal::Special(root.clone()));
    }
    let tree_bytes = entries.iter().map(|e| e.size).sum();
    let tree_files = entries
        .iter()
        .filter(|e| e.kind == TreeEntryKind::File)
        .count() as u64;
    let archive_bytes = entries
        .iter()
        .map(|e| entry_archive_bytes(&e.name, e.size))
        .sum::<u64>()
        + 2 * BLOCK;
    Ok(TreePlan {
        root: root.clone(),
        layout: source.layout,
        excludes: source.excludes.clone(),
        entries,
        archive_bytes,
        tree_bytes,
        tree_files,
    })
}

fn unreadable(error: io::Error) -> ScanRefusal {
    ScanRefusal::Unreadable(error.to_string())
}

fn walk(
    dir: &Path,
    prefix: &str,
    excluded: &HashSet<Vec<String>>,
    for_windows: bool,
    out: &mut Vec<TreeEntry>,
) -> Result<(), ScanRefusal> {
    let mut children: Vec<(String, PathBuf)> = Vec::new();
    for item in fs::read_dir(dir).map_err(unreadable)? {
        let item = item.map_err(unreadable)?;
        let path = item.path();
        let name = check_name(&path, &item.file_name(), for_windows)?;
        children.push((name, path));
    }
    children.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    if for_windows {
        let mut seen = HashSet::new();
        for (name, _) in &children {
            if !seen.insert(name.to_lowercase()) {
                return Err(ScanRefusal::NotPortable(format!("{prefix}{name}")));
            }
        }
    }
    for (name, path) in children {
        let meta = fs::symlink_metadata(&path).map_err(unreadable)?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            return Err(ScanRefusal::Link(path));
        }
        if kind.is_dir() {
            if removal_key(&path, None).is_some_and(|key| excluded.contains(&key)) {
                continue;
            }
            let rel = format!("{prefix}{name}/");
            out.push(entry(path.clone(), rel.clone(), TreeEntryKind::Dir, &meta));
            walk(&path, &rel, excluded, for_windows, out)?;
        } else if kind.is_file() {
            let rel = format!("{prefix}{name}");
            out.push(entry(path, rel, TreeEntryKind::File, &meta));
        } else {
            return Err(ScanRefusal::Special(path));
        }
    }
    Ok(())
}

fn entry(path: PathBuf, name: String, kind: TreeEntryKind, meta: &fs::Metadata) -> TreeEntry {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs());
    TreeEntry {
        path,
        name,
        kind,
        size: if kind == TreeEntryKind::File {
            meta.len()
        } else {
            0
        },
        mode: mode_of(kind, meta),
        mtime,
    }
}

#[cfg(unix)]
fn mode_of(_kind: TreeEntryKind, meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn mode_of(kind: TreeEntryKind, _meta: &fs::Metadata) -> u32 {
    match kind {
        TreeEntryKind::Dir => 0o755,
        TreeEntryKind::File => 0o644,
    }
}

/// One name of the tree, checked: UTF-8 always; portable to Windows when
/// `for_windows`.
fn check_name(
    path: &Path,
    name: &std::ffi::OsStr,
    for_windows: bool,
) -> Result<String, ScanRefusal> {
    let Some(name) = name.to_str() else {
        return Err(ScanRefusal::NotText(path.to_path_buf()));
    };
    if for_windows && !portable(name) {
        return Err(ScanRefusal::NotPortable(name.to_string()));
    }
    Ok(name.to_string())
}

/// Whether one name can be a file or folder name on Windows.
fn portable(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= super::pack::MAX_COMPONENT_BYTES
        && !name
            .chars()
            .any(|c| matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') || c < ' ')
        && !name.ends_with([' ', '.'])
        && !is_reserved_name(name)
}

/// The row values a restore needs, relative to the tree at `root`: the
/// native executable as a path inside the tree (`""` when it lies outside
/// or is not set), the working folder only when it is relative, and the
/// plain values as they are. Never an absolute path.
pub fn snapshot_row(row: &InstalledGame, root: &Path, home: Option<&Path>) -> PackInstalledRow {
    let working_dir = row.native_working_dir.trim();
    let working_dir = if working_dir.is_empty() {
        String::new()
    } else if Path::new(working_dir).is_absolute() || working_dir.starts_with('~') {
        relative_to(working_dir, root, home).unwrap_or_default()
    } else {
        working_dir.replace('\\', "/")
    };
    PackInstalledRow {
        native_executable: relative_to(&row.native_executable_path, root, home).unwrap_or_default(),
        native_launch_parameters: row.native_launch_parameters.clone(),
        native_launch_source: row.native_launch_source.clone(),
        native_working_dir: working_dir,
        ps4_game_id: row.ps4_game_id.clone(),
        ps4_content: row.ps4_content.clone(),
        included_dlc: row.included_dlc.clone(),
        ra_id: row.ra_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::paths::{legacy_platform_dir, platform_dir};

    const LIVE: &str = "https://romm.home.lan";
    const PS4: &str = "PlayStation 4";

    fn library() -> PathBuf {
        std::env::temp_dir().join("grid-local-test-library")
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn base_row(platform: &str) -> InstalledGame {
        InstalledGame {
            title: "Ember Run".into(),
            platform: platform.into(),
            rom_id: Some(77),
            rom_file_name: "Ember Run (v1.02).zip".into(),
            ..Default::default()
        }
    }

    fn extracted_row(dir: &Path) -> InstalledGame {
        InstalledGame {
            extracted_dir: text(dir),
            extracted_path: text(&dir.join("CUSA00001").join("eboot.bin")),
            ..base_row(PS4)
        }
    }

    fn native_row(dir: &Path) -> InstalledGame {
        InstalledGame {
            native_game_dir: text(dir),
            extracted_dir: text(&dir.join("game")),
            extracted_path: text(&dir.join("game").join("run.sh")),
            native_executable_path: text(&dir.join("game").join("run.sh")),
            native_wineprefix: text(&dir.join("pfx")),
            ..base_row("Linux")
        }
    }

    fn located(row: &InstalledGame) -> Located {
        locate(77, Some(row), false, &library(), None, LIVE)
    }

    fn eligible(row: &InstalledGame) -> LocalSource {
        match located(row) {
            Located::Eligible(source) => source,
            other => panic!("expected eligible, got {other:?}"),
        }
    }

    fn refused(row: &InstalledGame) -> String {
        match located(row) {
            Located::Ineligible(reason) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    // --- locate: layouts ------------------------------------------------------

    #[test]
    fn an_extracted_install_in_the_platform_folder_is_eligible() {
        let dir = platform_dir(&library(), PS4).join("Ember Run");
        let source = eligible(&extracted_row(&dir));
        assert_eq!(source.layout, InstalledLayout::Extracted);
        assert_eq!(source.root, dir);
        assert_eq!(source.name, "Ember Run");
        assert_eq!(source.launch, "CUSA00001/eboot.bin");
        assert!(source.excludes.is_empty());
    }

    #[test]
    fn a_legacy_platform_folder_is_accepted() {
        let dir = legacy_platform_dir(&library(), PS4).join("Ember Run");
        assert_eq!(eligible(&extracted_row(&dir)).root, dir);
    }

    #[test]
    fn a_native_install_leaves_out_its_prefix() {
        let dir = platform_dir(&library(), "Linux").join("Ember Run");
        let source = eligible(&native_row(&dir));
        assert_eq!(source.layout, InstalledLayout::Native);
        assert_eq!(source.root, dir);
        assert_eq!(source.launch, "game/run.sh");
        assert_eq!(source.excludes, [dir.join("prefix"), dir.join("pfx")]);

        let mut row = native_row(&dir);
        row.native_wineprefix = text(&dir.join("prefix"));
        assert_eq!(eligible(&row).excludes, [dir.join("prefix")], "listed once");
        row.native_wineprefix = text(&std::env::temp_dir().join("elsewhere"));
        assert_eq!(eligible(&row).excludes, [dir.join("prefix")]);
    }

    #[test]
    fn a_multi_file_install_is_eligible() {
        let dir = platform_dir(&library(), "Sony PlayStation").join("Ember Run");
        let row = InstalledGame {
            multi_file_game_dir: text(&dir),
            extracted_path: text(&dir.join("Ember Run.m3u")),
            ..base_row("Sony PlayStation")
        };
        let source = eligible(&row);
        assert_eq!(source.layout, InstalledLayout::Multi);
        assert_eq!(source.launch, "Ember Run.m3u");
    }

    #[test]
    fn a_downloaded_file_is_eligible() {
        let file = platform_dir(&library(), "Sega Genesis").join("Ember Run.zip");
        let row = InstalledGame {
            archive_path: text(&file),
            ..base_row("Sega Genesis")
        };
        let source = eligible(&row);
        assert_eq!(source.layout, InstalledLayout::File);
        assert_eq!(source.root, file);
        assert_eq!(source.name, "Ember Run.zip");
        assert_eq!(source.launch, "");
    }

    #[test]
    fn a_ps3_iso_only_install_is_a_file_but_a_routed_one_is_refused() {
        let iso = platform_dir(&library(), "PlayStation 3").join("Ember Run.iso");
        let row = InstalledGame {
            ps3_iso_path: text(&iso),
            extracted_path: text(&iso),
            ..base_row("PlayStation 3")
        };
        let source = eligible(&row);
        assert_eq!(source.layout, InstalledLayout::File);
        assert_eq!(source.root, iso);

        let dev_hdd0 = library().join("emulators").join("rpcs3").join("dev_hdd0");
        let game = dev_hdd0.join("game").join("BLUS30336");
        let routed = InstalledGame {
            ps3_game_id: "BLUS30336".into(),
            extracted_dir: text(&game),
            extracted_path: text(&game.join("EBOOT.BIN")),
            ..base_row("PlayStation 3")
        };
        assert_eq!(refused(&routed), REASON_OUTSIDE);
        let trophies = InstalledGame {
            ps3_trophy_paths: r#"["x"]"#.into(),
            ..row
        };
        assert_eq!(refused(&trophies), REASON_OUTSIDE);
    }

    // --- locate: refusals -----------------------------------------------------

    #[test]
    fn no_row_or_another_games_row_is_not_installed() {
        assert_eq!(
            locate(77, None, false, &library(), None, LIVE),
            Located::NotInstalled
        );
        let mut row = extracted_row(&platform_dir(&library(), PS4).join("Ember Run"));
        row.rom_id = Some(78);
        assert_eq!(located(&row), Located::NotInstalled);
    }

    #[test]
    fn a_row_without_a_rom_id_is_refused() {
        let mut row = extracted_row(&platform_dir(&library(), PS4).join("Ember Run"));
        row.rom_id = None;
        assert_eq!(refused(&row), REASON_NO_ROM_ID);
    }

    #[test]
    fn only_the_live_server_counts() {
        let mut row = extracted_row(&platform_dir(&library(), PS4).join("Ember Run"));
        row.source_server = "https://other.lan".into();
        assert_eq!(refused(&row), REASON_OTHER_SERVER);
        row.source_server = "https://ROMM.home.lan:443/".into();
        assert!(
            matches!(located(&row), Located::Eligible(_)),
            "same server key"
        );
        row.source_server = String::new();
        assert!(
            matches!(located(&row), Located::Eligible(_)),
            "a server install"
        );
    }

    #[test]
    fn a_missing_install_is_refused() {
        let row = extracted_row(&platform_dir(&library(), PS4).join("Ember Run"));
        assert_eq!(
            locate(77, Some(&row), true, &library(), None, LIVE),
            Located::Ineligible(REASON_MISSING.into())
        );
    }

    #[test]
    fn xbox_360_and_emulator_packages_are_refused() {
        let row = InstalledGame {
            extracted_dir: text(&platform_dir(&library(), "Xbox 360").join("Ember Run")),
            ..base_row("Xbox 360")
        };
        assert_eq!(refused(&row), REASON_XBOX360);
        let row = InstalledGame {
            extracted_dir: text(&platform_dir(&library(), "Emulators").join("DuckStation")),
            ..base_row("Emulators")
        };
        assert_eq!(refused(&row), REASON_EMULATORS);
    }

    #[test]
    fn an_install_outside_the_platform_folder_is_refused() {
        let elsewhere = std::env::temp_dir()
            .join("grid-local-test-elsewhere")
            .join("Ember Run");
        assert_eq!(refused(&extracted_row(&elsewhere)), REASON_OUTSIDE);
        // Directly in the library root, or in another platform's folder.
        let at_root = library().join("Ember Run");
        assert_eq!(refused(&extracted_row(&at_root)), REASON_OUTSIDE);
        let other = platform_dir(&library(), "PlayStation 5").join("Ember Run");
        assert_eq!(refused(&extracted_row(&other)), REASON_OUTSIDE);
        // Deeper than the platform folder, or the platform folder itself.
        let deep = platform_dir(&library(), PS4).join("a").join("Ember Run");
        assert_eq!(refused(&extracted_row(&deep)), REASON_OUTSIDE);
        let itself = platform_dir(&library(), PS4);
        assert_eq!(refused(&extracted_row(&itself)), REASON_OUTSIDE);
    }

    #[test]
    fn a_launch_path_outside_the_root_is_refused() {
        let dir = platform_dir(&library(), PS4).join("Ember Run");
        let mut row = extracted_row(&dir);
        row.extracted_path = text(
            &platform_dir(&library(), PS4)
                .join("other")
                .join("eboot.bin"),
        );
        assert_eq!(refused(&row), REASON_OUTSIDE);

        let dir = platform_dir(&library(), "Linux").join("Ember Run");
        let mut row = native_row(&dir);
        row.native_executable_path = text(&std::env::temp_dir().join("moved").join("run.sh"));
        assert_eq!(refused(&row), REASON_OUTSIDE);

        let file = platform_dir(&library(), "Sega Genesis").join("Ember Run.zip");
        let row = InstalledGame {
            archive_path: text(&file),
            extracted_path: text(&file.with_extension("bin")),
            ..base_row("Sega Genesis")
        };
        assert_eq!(refused(&row), REASON_OUTSIDE);
    }

    #[test]
    fn a_row_without_a_location_is_refused() {
        assert_eq!(refused(&base_row(PS4)), REASON_OUTSIDE);
    }

    #[test]
    fn the_home_folder_is_expanded() {
        let home = std::env::temp_dir().join("grid-local-test-home");
        let library = home.join("Games");
        let row = InstalledGame {
            extracted_dir: "~/Games/games/PlayStation 4/Ember Run".into(),
            extracted_path: "~/Games/games/PlayStation 4/Ember Run/eboot.bin".into(),
            ..base_row(PS4)
        };
        let Located::Eligible(source) = locate(77, Some(&row), false, &library, Some(&home), LIVE)
        else {
            panic!("eligible");
        };
        assert_eq!(source.root, platform_dir(&library, PS4).join("Ember Run"));
        assert_eq!(source.launch, "eboot.bin");
    }

    // --- scan_tree --------------------------------------------------------------

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7_u8; len]).unwrap();
    }

    fn source_for(root: &Path, layout: InstalledLayout) -> LocalSource {
        LocalSource {
            layout,
            root: root.to_path_buf(),
            name: root.file_name().unwrap().to_string_lossy().into_owned(),
            excludes: Vec::new(),
            launch: String::new(),
        }
    }

    /// The tar `tree` produces when written the way the module docs say.
    fn tar_of(tree: &TreePlan) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for entry in &tree.entries {
            let mut header = tar_header(entry);
            match entry.kind {
                TreeEntryKind::File => {
                    let file = fs::File::open(&entry.path).unwrap();
                    builder.append_data(&mut header, &entry.name, file).unwrap();
                }
                TreeEntryKind::Dir => builder
                    .append_data(&mut header, &entry.name, io::empty())
                    .unwrap(),
            }
        }
        builder.into_inner().unwrap()
    }

    #[test]
    fn the_size_is_exactly_what_tar_writes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Ember Run");
        let long = "l".repeat(120);
        write(&root.join("eboot.bin"), 1000);
        write(&root.join("zero.bin"), 0);
        write(&root.join("block.bin"), 512);
        write(&root.join("block+1.bin"), 513);
        write(&root.join(&long), 3);
        write(&root.join("e".repeat(100)), 1);
        write(&root.join("o".repeat(101)), 1);
        write(&root.join("sce_sys").join(&long).join("x.dat"), 9);
        write(&root.join("ünïcødé").join("日本語のファイル名.txt"), 77);
        write(&root.join("ünïcødé").join("ü".repeat(55)), 5);
        fs::create_dir_all(root.join("empty").join("nested empty")).unwrap();
        fs::create_dir_all(root.join("d".repeat(99))).unwrap();

        let tree = scan_tree(&source_for(&root, InstalledLayout::Extracted), false).unwrap();
        let bytes = tar_of(&tree);
        assert_eq!(tree.archive_bytes, bytes.len() as u64);
        assert_eq!(tree.tree_files, 10);
        assert_eq!(tree.tree_bytes, 1000 + 512 + 513 + 3 + 1 + 1 + 9 + 77 + 5);

        // Folders come before what they hold, and every name reads back.
        let names: Vec<&str> = tree.entries.iter().map(|e| e.name.as_str()).collect();
        let pos = |name: &str| names.iter().position(|n| *n == name).unwrap();
        assert!(pos("sce_sys/") < pos(&format!("sce_sys/{long}/")));
        assert!(pos(&format!("sce_sys/{long}/")) < pos(&format!("sce_sys/{long}/x.dat")));
        assert!(names.contains(&"empty/nested empty/"));
        let mut archive = tar::Archive::new(bytes.as_slice());
        let read: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let path = e.path().unwrap().to_string_lossy().replace('\\', "/");
                if e.header().entry_type().is_dir() {
                    format!("{}/", path.trim_end_matches('/'))
                } else {
                    path
                }
            })
            .collect();
        assert_eq!(read, names);
    }

    /// The long-name rule is the tar crate's: a name of up to 100 bytes
    /// fits the header; from 101 on it takes a GNU long-name record. A
    /// folder's trailing `/` counts.
    #[test]
    fn the_long_name_rule_matches_the_tar_crate() {
        let written = |name: &str, kind: TreeEntryKind, size: u64| {
            let entry = TreeEntry {
                path: PathBuf::new(),
                name: name.to_string(),
                kind,
                size,
                mode: 0o644,
                mtime: 0,
            };
            let mut builder = tar::Builder::new(Vec::new());
            let data = vec![1_u8; size as usize];
            builder
                .append_data(&mut tar_header(&entry), name, data.as_slice())
                .unwrap();
            builder.get_ref().len() as u64
        };
        for len in [98, 99, 100, 101, 102] {
            let file = "f".repeat(len);
            let folder = format!("{}/", "d".repeat(len - 1));
            let nested = format!("{}/{}", "n".repeat(len / 2), "m".repeat(len - len / 2 - 1));
            let unicode = format!("{}{}", "ü".repeat(len / 2), "u".repeat(len % 2));
            for (name, kind) in [
                (&file, TreeEntryKind::File),
                (&folder, TreeEntryKind::Dir),
                (&nested, TreeEntryKind::File),
                (&unicode, TreeEntryKind::File),
            ] {
                assert_eq!(name.len(), len, "{name}");
                let size = if kind == TreeEntryKind::Dir { 0 } else { 5 };
                let long = written(name, kind, size) > BLOCK + padded(size);
                assert_eq!(long, len > 100, "{len}: {name}");
                assert_eq!(
                    entry_archive_bytes(name, size),
                    written(name, kind, size),
                    "{len}: {name}"
                );
            }
        }
    }

    #[test]
    fn a_single_file_is_one_entry_named_after_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Ember Run.zip");
        write(&file, 2000);
        let tree = scan_tree(&source_for(&file, InstalledLayout::File), true).unwrap();
        assert_eq!(tree.entries.len(), 1);
        assert_eq!(tree.entries[0].name, "Ember Run.zip");
        assert_eq!(tree.tree_files, 1);
        assert_eq!(tree.tree_bytes, 2000);
        assert_eq!(tree.archive_bytes, 512 + 2048 + 1024);
        assert_eq!(tree.archive_bytes, tar_of(&tree).len() as u64);
    }

    #[test]
    fn a_native_scan_leaves_out_the_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let root = platform_dir(library, "Linux").join("Ember Run");
        write(&root.join("game").join("run.sh"), 10);
        write(&root.join("game.json"), 5);
        write(&root.join("prefix").join("drive_c").join("save.dat"), 100);
        write(&root.join("pfx").join("user.reg"), 100);
        let row = native_row(&root);
        let Located::Eligible(source) = locate(77, Some(&row), false, library, None, LIVE) else {
            panic!("eligible");
        };
        let tree = scan_tree(&source, false).unwrap();
        let names: Vec<&str> = tree.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["game/", "game/run.sh", "game.json"]);
        assert_eq!(tree.tree_bytes, 15);

        assert_eq!(
            assess(77, Some(&row), false, library, None, LIVE, false),
            LocalState::Eligible {
                layout: InstalledLayout::Native,
                archive_bytes: tree.archive_bytes,
                outdated: None,
            }
        );
    }

    #[test]
    fn assess_passes_on_a_refusal() {
        let row = extracted_row(&platform_dir(&library(), PS4).join("Ember Run"));
        assert_eq!(
            assess(77, Some(&row), true, &library(), None, LIVE, false),
            LocalState::Ineligible {
                reason: REASON_MISSING.into()
            }
        );
        assert_eq!(
            assess(77, None, false, &library(), None, LIVE, false),
            LocalState::NotInstalled
        );
    }

    #[cfg(unix)]
    #[test]
    fn links_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Ember Run");
        write(&root.join("sub").join("a.bin"), 1);
        std::os::unix::fs::symlink(dir.path(), root.join("sub").join("out")).unwrap();
        let refusal = scan_tree(&source_for(&root, InstalledLayout::Extracted), false).unwrap_err();
        assert!(matches!(refusal, ScanRefusal::Link(_)));
        assert_eq!(
            scanned_state(&Err(refusal)),
            LocalState::Ineligible {
                reason: REASON_LINKS.into()
            }
        );

        let linked_root = dir.path().join("Linked Run");
        std::os::unix::fs::symlink(&root, &linked_root).unwrap();
        let refusal = scan_tree(&source_for(&linked_root, InstalledLayout::Extracted), false);
        assert!(matches!(refusal, Err(ScanRefusal::Link(_))));
    }

    #[cfg(unix)]
    #[test]
    fn names_windows_cannot_hold_are_refused_only_for_windows() {
        for bad in [
            "a:b",
            "what?",
            "CON",
            "aux.txt",
            "trailing.",
            "trailing ",
            "tab\tname",
            "back\\slash",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("Ember Run");
            write(&root.join("sub").join(bad), 1);
            let source = source_for(&root, InstalledLayout::Extracted);
            let refusal = scan_tree(&source, true).unwrap_err();
            assert_eq!(refusal.reason(), REASON_NOT_PORTABLE, "{bad:?}");
            assert!(
                scan_tree(&source, false).is_ok(),
                "{bad:?} is fine on Linux"
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Ember Run");
        write(&root.join("Save.dat"), 1);
        write(&root.join("save.dat"), 1);
        let source = source_for(&root, InstalledLayout::Extracted);
        assert!(matches!(
            scan_tree(&source, true),
            Err(ScanRefusal::NotPortable(_))
        ));
        assert!(scan_tree(&source, false).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_name_that_is_not_utf8_is_refused() {
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Ember Run");
        write(&root.join(std::ffi::OsStr::from_bytes(b"bad\xff.bin")), 1);
        let refusal = scan_tree(&source_for(&root, InstalledLayout::Extracted), false);
        assert_eq!(refusal.unwrap_err().reason(), REASON_NOT_TEXT);
    }

    #[test]
    fn reserved_names() {
        for name in ["CON", "con", "Aux.txt", "COM1.tar.gz", "nul ", "LPT9"] {
            assert!(is_reserved_name(name), "{name}");
        }
        for name in ["CONSOLE", "com10", "xaux.txt", "game.con"] {
            assert!(!is_reserved_name(name), "{name}");
        }
    }

    // --- snapshot_row -----------------------------------------------------------

    #[test]
    fn the_snapshot_holds_only_relative_and_plain_values() {
        let root = platform_dir(&library(), "Windows").join("Ember Run");
        let row = InstalledGame {
            native_executable_path: text(&root.join("game").join("bin").join("Ember.exe")),
            native_launch_parameters: "-windowed".into(),
            native_launch_source: "sidecar".into(),
            native_working_dir: "game/bin".into(),
            ps4_game_id: "CUSA00001".into(),
            ps4_content: r#"[{"kind":"update"}]"#.into(),
            included_dlc: r#"["Pack"]"#.into(),
            ra_id: "12".into(),
            ..base_row("Windows")
        };
        assert_eq!(
            snapshot_row(&row, &root, None),
            PackInstalledRow {
                native_executable: "game/bin/Ember.exe".into(),
                native_launch_parameters: "-windowed".into(),
                native_launch_source: "sidecar".into(),
                native_working_dir: "game/bin".into(),
                ps4_game_id: "CUSA00001".into(),
                ps4_content: r#"[{"kind":"update"}]"#.into(),
                included_dlc: r#"["Pack"]"#.into(),
                ra_id: "12".into(),
            }
        );

        let outside = InstalledGame {
            native_executable_path: text(&std::env::temp_dir().join("Ember.exe")),
            native_working_dir: text(&root.join("game")),
            ..row.clone()
        };
        let snap = snapshot_row(&outside, &root, None);
        assert_eq!(snap.native_executable, "", "outside the tree: dropped");
        assert_eq!(
            snap.native_working_dir, "game",
            "absolute inside: made relative"
        );
        let away = InstalledGame {
            native_working_dir: text(&std::env::temp_dir()),
            ..row
        };
        assert_eq!(snapshot_row(&away, &root, None).native_working_dir, "");
        assert_eq!(
            snapshot_row(&base_row(PS4), &root, None),
            PackInstalledRow::default()
        );
    }
}
