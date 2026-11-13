//! GOG games on a backup drive (SPEC "Game backups", the GameStores card's
//! "GOG games on backup drives", decisions 19 to 23).
//!
//! A GOG game that did not come from RomM goes to a drive one of two ways,
//! decided per game with no choice in the dialog ([`decide_source`]):
//! - **installer files** ([`StoreLayout::Installer`]) when every installer
//!   file GRID recorded for it (`GogMeta::installer`, and each installed
//!   DLC's) is still where it was, with the recorded size; restoring runs
//!   the installer again;
//! - else **the installed copy** ([`StoreLayout::Installed`]): the game's
//!   folder as a tar in parts, never its Wine prefix ([`installed_source`]).
//!
//! Saves go with both: the save folders GOG reports
//! (`GogMeta::save_locations`, turned into GRID's folder strings by
//! [`save_folders`]) plus the ones GRID's cloud saves know for the title,
//! copied out of the Wine prefix (or the install folder) through
//! [`super::saves`]. Epic, Amazon and Steam games are refused with
//! [`super::local::REASON_STORE`].
//!
//! Everything here is pure or reads only metadata; the queue's backup row
//! (`library::backup_job`) and the restore (`library::drive_restore`) do
//! the work.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::local::{LocalSource, REASON_STORE};
use super::pack::{InstalledLayout, StoreLayout, FAT32_MAX_FILE};
use crate::cloud::native::{
    resolve_native_save_dir, translate_windows_path_to_wine_prefix, WinePrefix,
};
use crate::library::path_change::{expand, is_strictly_inside};
use crate::library::registry::InstalledGame;
use crate::library::space::format_size;
use crate::stores::gog::meta::{GogMeta, GogOrigin, GogPlatform};
use crate::stores::StoreKind;

/// A save folder inside the game's install folder, as GRID records it
/// (`<?INSTALL?>\saves`), from GOG's own `<?INSTALL?>` variable.
pub const INSTALL_TOKEN: &str = "<?INSTALL?>";

/// The folder of a GOG game's Wine prefix GRID makes beside its files on
/// a restore (off Windows), like a native RomM game's `prefix/`.
pub const STORE_PREFIX_DIR: &str = "prefix";

/// Why a store game of `kind` can't go on a drive, `None` for GOG.
pub fn store_refusal(kind: StoreKind) -> Option<&'static str> {
    (kind != StoreKind::Gog).then_some(REASON_STORE)
}

/// "Lanternfall is running right now; close it to copy it."
pub fn running_refusal(name: &str) -> String {
    format!("{name} is running right now; close it to copy it.")
}

/// "Copper Saints is being installed or updated. Wait for it to finish."
pub fn installing_refusal(name: &str) -> String {
    format!("{name} is being installed or updated. Wait for it to finish.")
}

/// "Brightwater is no longer installed on this computer; backup again".
pub fn gone_refusal(name: &str) -> String {
    format!("{name} is no longer installed on this computer; backup again")
}

/// One installer file on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallerFile {
    pub path: PathBuf,
    pub size: u64,
}

impl InstallerFile {
    /// The file's name, for the drive and the dialog.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// One installed DLC's installer files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DlcInstaller {
    pub id: String,
    pub title: String,
    pub files: Vec<InstallerFile>,
}

/// Why a game goes on the drive as its installed copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstalledWhy {
    /// Installed with gogdl, which keeps no installer.
    Gogdl,
    /// Copied back from a backup drive's installed copy.
    FromDrive,
    /// Installed from an installer GRID has no record of.
    NoInstaller,
    /// An installer file isn't where it was (its recorded path).
    Moved(String),
    /// The installer files are there but not the size GRID recorded.
    Changed(String),
    /// An installed DLC has no installer file (installed with gogdl).
    DlcNoInstaller(String),
    /// An installer file is over 4 GB and the drive is FAT32 (its name).
    TooBigForDrive(String),
}

/// Where a GOG game's backup comes from ([`decide_source`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreSource {
    Installer {
        files: Vec<InstallerFile>,
        dlc: Vec<DlcInstaller>,
    },
    Installed(InstalledWhy),
}

impl StoreSource {
    pub fn layout(&self) -> StoreLayout {
        match self {
            StoreSource::Installer { .. } => StoreLayout::Installer,
            StoreSource::Installed(_) => StoreLayout::Installed,
        }
    }

    /// Every installer file's bytes, DLC included (`0` for an installed
    /// copy).
    pub fn installer_bytes(&self) -> u64 {
        match self {
            StoreSource::Installer { files, dlc } => files
                .iter()
                .chain(dlc.iter().flat_map(|dlc| dlc.files.iter()))
                .fold(0_u64, |sum, file| sum.saturating_add(file.size)),
            StoreSource::Installed(_) => 0,
        }
    }
}

/// The size of the regular file at `path` (links followed), `None` when
/// there is none. The probe [`decide_source`] uses on a real disk.
pub fn file_size(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
}

/// Decides how a GOG game goes on a drive. Pure: `probe` gives a file's
/// size, `None` when it is missing ([`file_size`]).
///
/// The installer files ([`StoreSource::Installer`]) when all of these hold,
/// else the installed copy with the first reason that applies:
/// 1. GRID recorded installer files (`meta.installer`); otherwise the
///    origin says why (gogdl, a drive restore, an offline install with no
///    record);
/// 2. every base file exists ([`InstalledWhy::Moved`]);
/// 3. their total is `meta.installer_bytes`, when that was recorded
///    ([`InstalledWhy::Changed`]);
/// 4. every installed DLC has installer files ([`InstalledWhy::DlcNoInstaller`])
///    and they all exist ([`InstalledWhy::Moved`]);
/// 5. on a FAT32 drive, no file is over 4 GB ([`InstalledWhy::TooBigForDrive`]).
pub fn decide_source(
    meta: &GogMeta,
    fat32: bool,
    probe: &dyn Fn(&Path) -> Option<u64>,
) -> StoreSource {
    let paths: Vec<&str> = meta
        .installer
        .iter()
        .map(|path| path.trim())
        .filter(|path| !path.is_empty())
        .collect();
    if paths.is_empty() {
        return StoreSource::Installed(match meta.origin {
            GogOrigin::Gogdl => InstalledWhy::Gogdl,
            GogOrigin::Drive => InstalledWhy::FromDrive,
            GogOrigin::Offline => InstalledWhy::NoInstaller,
        });
    }
    let mut files = Vec::new();
    for path in &paths {
        match probe(Path::new(path)) {
            Some(size) => files.push(InstallerFile {
                path: PathBuf::from(path),
                size,
            }),
            None => return StoreSource::Installed(InstalledWhy::Moved(path.to_string())),
        }
    }
    let total = files
        .iter()
        .fold(0_u64, |sum, file| sum.saturating_add(file.size));
    if meta.installer_bytes > 0 && total != meta.installer_bytes {
        return StoreSource::Installed(InstalledWhy::Changed(paths[0].to_string()));
    }
    let mut dlc = Vec::new();
    for item in &meta.dlc {
        let dlc_paths: Vec<&str> = item
            .installer
            .iter()
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
            .collect();
        if dlc_paths.is_empty() {
            let title = if item.title.trim().is_empty() {
                item.id.clone()
            } else {
                item.title.clone()
            };
            return StoreSource::Installed(InstalledWhy::DlcNoInstaller(title));
        }
        let mut dlc_files = Vec::new();
        for path in dlc_paths {
            match probe(Path::new(path)) {
                Some(size) => dlc_files.push(InstallerFile {
                    path: PathBuf::from(path),
                    size,
                }),
                None => return StoreSource::Installed(InstalledWhy::Moved(path.to_string())),
            }
        }
        dlc.push(DlcInstaller {
            id: item.id.clone(),
            title: item.title.clone(),
            files: dlc_files,
        });
    }
    if fat32 {
        if let Some(big) = files
            .iter()
            .chain(dlc.iter().flat_map(|dlc| dlc.files.iter()))
            .find(|file| file.size > FAT32_MAX_FILE)
        {
            return StoreSource::Installed(InstalledWhy::TooBigForDrive(big.name()));
        }
    }
    StoreSource::Installer { files, dlc }
}

/// `path` with this computer's home folder written as `~`.
fn tilde(path: &str, home: Option<&Path>) -> String {
    match home.and_then(|home| Path::new(path).strip_prefix(home).ok()) {
        Some(rest) if !rest.as_os_str().is_empty() => {
            format!("~/{}", rest.to_string_lossy().replace('\\', "/"))
        }
        _ => path.to_string(),
    }
}

/// The dialog's line for a game's row (after "GOG · "): what goes on the
/// drive and its size, `bytes` being what the row writes.
///
/// - Installer files: "brightwater_1_3_58810.sh · 2.9 GB, and its saves"
///   (the first file's name, "and N more files" when there are more).
/// - Installed copy: why, then the size: "Installed with gogdl, which
///   keeps no installer, so the installed files are copied · 21.6 GB, and
///   its saves".
pub fn plan_detail(source: &StoreSource, bytes: u64, home: Option<&Path>) -> String {
    let size = format_size(bytes);
    match source {
        StoreSource::Installer { files, dlc } => {
            let first = files.first().map(InstallerFile::name).unwrap_or_default();
            let more = files.len().saturating_sub(1)
                + dlc.iter().map(|dlc| dlc.files.len()).sum::<usize>();
            let name = match more {
                0 => first,
                1 => format!("{first} and 1 more file"),
                n => format!("{first} and {n} more files"),
            };
            format!("{name} · {size}, and its saves")
        }
        StoreSource::Installed(why) => {
            let lead = match why {
                InstalledWhy::Gogdl => {
                    "Installed with gogdl, which keeps no installer, so the installed files are copied"
                        .to_string()
                }
                InstalledWhy::FromDrive => {
                    "Installed from a backup drive, so the installed files are copied".to_string()
                }
                InstalledWhy::NoInstaller => {
                    "GRID has no record of its installer, so the installed files are copied"
                        .to_string()
                }
                InstalledWhy::Moved(path) => format!(
                    "Its installer isn’t at {} any more, so the installed files are copied",
                    tilde(path, home)
                ),
                InstalledWhy::Changed(path) => format!(
                    "Its installer at {} has changed, so the installed files are copied",
                    tilde(path, home)
                ),
                InstalledWhy::DlcNoInstaller(title) => format!(
                    "{title} was installed with gogdl, which keeps no installer, so the installed files are copied"
                ),
                InstalledWhy::TooBigForDrive(name) => format!(
                    "{name} is over 4 GB, which this drive can’t hold, so the installed files are copied"
                ),
            };
            format!("{lead} · {size}, and its saves")
        }
    }
}

/// Whether the dialog adds its Windows note to a row: a Windows game
/// copied as installed files on a computer that runs it under Wine.
pub fn windows_prefix_note(meta: &GogMeta, source: &StoreSource) -> bool {
    cfg!(not(windows))
        && meta.platform == GogPlatform::Windows
        && source.layout() == StoreLayout::Installed
}

/// The install folder of a GOG row (`native_game_dir`, else
/// `extracted_dir`), `~/` expanded. `None` when the row records neither.
pub fn install_dir(row: &InstalledGame, home: Option<&Path>) -> Option<PathBuf> {
    [&row.native_game_dir, &row.extracted_dir]
        .into_iter()
        .map(|raw| raw.trim())
        .find(|raw| !raw.is_empty())
        .map(|raw| expand(raw, home))
}

/// The tree of a GOG game's installed copy: its install folder, less its
/// Wine prefix (the row's, when it lies inside the folder, and a
/// `prefix/` folder holding a `drive_c` or `pfx`). `Err` with the reason
/// the dialog shows when the folder is gone.
pub fn installed_source(row: &InstalledGame, home: Option<&Path>) -> Result<LocalSource, String> {
    let missing = || "Its files weren't found on this computer".to_string();
    let root = install_dir(row, home).ok_or_else(missing)?;
    if !root.is_dir() {
        return Err(missing());
    }
    let mut excludes = Vec::new();
    let prefix = row.native_wineprefix.trim();
    if !prefix.is_empty() {
        let prefix = expand(prefix, home);
        if is_strictly_inside(&prefix, &root, None) {
            excludes.push(prefix);
        }
    }
    let own = root.join(STORE_PREFIX_DIR);
    if (own.join("drive_c").is_dir() || own.join("pfx").is_dir()) && !excludes.contains(&own) {
        excludes.push(own);
    }
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(LocalSource {
        layout: InstalledLayout::Multi,
        root,
        name,
        excludes,
        launch: String::new(),
    })
}

/// A path's plain parts, either separator; `None` when one climbs out
/// (`..`) or names a drive.
fn plain_parts(text: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    for part in text.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') {
            return None;
        }
        parts.push(part);
    }
    Some(parts)
}

/// One GOG save location (`<?APPLICATION_DATA_ROAMING?>/Foo`) as GRID's
/// folder string: a Windows-token path (`%APPDATA%\Foo`) or
/// [`INSTALL_TOKEN`]`\…` for a folder inside the install folder. A plain
/// absolute path inside `install_dir` or the prefix's `drive_c` is turned
/// into those forms too. `None` for anything else (a macOS location, an
/// unknown variable, a path elsewhere).
pub fn gog_save_folder(raw: &str, install_dir: &Path, prefix: Option<&Path>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (base, rest): (String, &str) = if let Some(after) = raw.strip_prefix("<?") {
        let end = after.find("?>")?;
        let var = &after[..end];
        let base = match var {
            "INSTALL" => INSTALL_TOKEN,
            "SAVED_GAMES" => "%USERPROFILE%\\Saved Games",
            "APPLICATION_DATA_LOCAL" => "%LOCALAPPDATA%",
            "APPLICATION_DATA_LOCAL_LOW" => "%USERPROFILE%\\AppData\\LocalLow",
            "APPLICATION_DATA_ROAMING" => "%APPDATA%",
            "DOCUMENTS" => "%USERPROFILE%\\Documents",
            _ => return None,
        };
        (base.to_string(), &after[end + 2..])
    } else if let Some(after) = raw.strip_prefix('%') {
        let end = after.find('%')? + 1;
        (raw[..=end].to_string(), &raw[end + 1..])
    } else {
        let path = Path::new(raw);
        if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) {
            return None;
        }
        if let Ok(rel) = path.strip_prefix(install_dir) {
            let text = rel.to_string_lossy();
            let parts = plain_parts(&text)?;
            return Some(join_token(INSTALL_TOKEN, &parts));
        }
        let prefix = prefix?;
        return super::saves::token_for_prefix_path(path, prefix);
    };
    let parts = plain_parts(rest)?;
    Some(join_token(&base, &parts))
}

fn join_token(base: &str, parts: &[&str]) -> String {
    if parts.is_empty() {
        base.to_string()
    } else {
        format!("{base}\\{}", parts.join("\\"))
    }
}

/// The save folders to record for a GOG game: the ones GOG reports
/// (`meta.save_locations`, through [`gog_save_folder`]), then `extra` (the
/// Windows-token folders GRID's cloud saves know for the title). Duplicates
/// (compared without case) are dropped, the first one kept.
pub fn save_folders(
    meta: &GogMeta,
    extra: &[String],
    install_dir: &Path,
    prefix: Option<&Path>,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    meta.save_locations
        .iter()
        .filter_map(|location| gog_save_folder(&location.path, install_dir, prefix))
        .chain(extra.iter().map(|folder| folder.trim().to_string()))
        .filter(|folder| !folder.is_empty())
        .filter(|folder| seen.insert(folder.to_lowercase()))
        .collect()
}

/// Whether a recorded save folder lies inside the install folder.
pub fn is_install_folder(raw: &str) -> bool {
    raw.trim().starts_with(INSTALL_TOKEN)
}

/// Where the save folder `raw` ([`save_folders`]) is on this computer:
/// inside `install_dir` for an [`INSTALL_TOKEN`] folder, in the Wine
/// `prefix` when there is one, else (on Windows only) the real folder.
/// `None` when it has no place here.
pub fn resolve_save_folder(
    raw: &str,
    install_dir: &Path,
    prefix: Option<WinePrefix<'_>>,
    windows_documents: Option<&Path>,
) -> Option<PathBuf> {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix(INSTALL_TOKEN) {
        let parts = plain_parts(rest)?;
        return Some(
            parts
                .iter()
                .fold(install_dir.to_path_buf(), |path, part| path.join(part)),
        );
    }
    match prefix {
        Some(prefix) => translate_windows_path_to_wine_prefix(raw, prefix),
        None if cfg!(windows) => Some(resolve_native_save_dir(raw, windows_documents, None))
            .filter(|path| path.is_absolute()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::gog::meta::{GogDlcMeta, SaveLocation};
    use std::collections::HashMap;

    fn meta(installer: &[&str], bytes: u64) -> GogMeta {
        GogMeta {
            origin: GogOrigin::Offline,
            installer: installer.iter().map(|s| s.to_string()).collect(),
            installer_bytes: bytes,
            ..GogMeta::default()
        }
    }

    fn disk(files: &[(&str, u64)]) -> impl Fn(&Path) -> Option<u64> {
        let map: HashMap<PathBuf, u64> = files
            .iter()
            .map(|(path, size)| (PathBuf::from(path), *size))
            .collect();
        move |path: &Path| map.get(path).copied()
    }

    #[test]
    fn the_layout_follows_the_decision_table() {
        let present = disk(&[
            ("/dl/setup.exe", 10),
            ("/dl/setup-1.bin", 20),
            ("/dl/dlc.exe", 5),
            ("/dl/big.bin", FAT32_MAX_FILE + 1),
        ]);
        // gogdl and drive origins keep no installer.
        let gogdl = GogMeta::default();
        assert_eq!(
            decide_source(&gogdl, false, &present),
            StoreSource::Installed(InstalledWhy::Gogdl)
        );
        let drive = GogMeta {
            origin: GogOrigin::Drive,
            ..GogMeta::default()
        };
        assert_eq!(
            decide_source(&drive, false, &present),
            StoreSource::Installed(InstalledWhy::FromDrive)
        );
        assert_eq!(
            decide_source(&meta(&[], 0), false, &present),
            StoreSource::Installed(InstalledWhy::NoInstaller)
        );
        // Every file there with the recorded size: installer files.
        let whole = meta(&["/dl/setup.exe", "/dl/setup-1.bin"], 30);
        let source = decide_source(&whole, false, &present);
        assert_eq!(source.layout(), StoreLayout::Installer);
        assert_eq!(source.installer_bytes(), 30);
        // No recorded size: presence is enough.
        assert_eq!(
            decide_source(&meta(&["/dl/setup.exe"], 0), false, &present).layout(),
            StoreLayout::Installer
        );
        // A part gone.
        assert_eq!(
            decide_source(
                &meta(&["/dl/setup.exe", "/dl/gone.bin"], 30),
                false,
                &present
            ),
            StoreSource::Installed(InstalledWhy::Moved("/dl/gone.bin".into()))
        );
        // A size that changed.
        assert_eq!(
            decide_source(&meta(&["/dl/setup.exe"], 11), false, &present),
            StoreSource::Installed(InstalledWhy::Changed("/dl/setup.exe".into()))
        );
        // DLC: with its installer, missing it, or installed with gogdl.
        let mut with_dlc = whole.clone();
        with_dlc.dlc = vec![GogDlcMeta {
            id: "2".into(),
            title: "Harbor Lights".into(),
            installer: vec!["/dl/dlc.exe".into()],
        }];
        let source = decide_source(&with_dlc, false, &present);
        assert_eq!(source.installer_bytes(), 35);
        let StoreSource::Installer { dlc, .. } = &source else {
            panic!("installer expected");
        };
        assert_eq!(dlc[0].files[0].size, 5);
        with_dlc.dlc[0].installer = vec!["/dl/nope.exe".into()];
        assert_eq!(
            decide_source(&with_dlc, false, &present),
            StoreSource::Installed(InstalledWhy::Moved("/dl/nope.exe".into()))
        );
        with_dlc.dlc[0].installer.clear();
        assert_eq!(
            decide_source(&with_dlc, false, &present),
            StoreSource::Installed(InstalledWhy::DlcNoInstaller("Harbor Lights".into()))
        );
        // FAT32 can't hold a file over 4 GB.
        let big = meta(&["/dl/setup.exe", "/dl/big.bin"], 0);
        assert_eq!(
            decide_source(&big, false, &present).layout(),
            StoreLayout::Installer
        );
        assert_eq!(
            decide_source(&big, true, &present),
            StoreSource::Installed(InstalledWhy::TooBigForDrive("big.bin".into()))
        );
    }

    #[test]
    fn the_dialog_lines_say_what_goes() {
        let home = Path::new("/home/sam");
        let installer = StoreSource::Installer {
            files: vec![InstallerFile {
                path: "/home/sam/Downloads/brightwater_1_3_58810.sh".into(),
                size: 3,
            }],
            dlc: Vec::new(),
        };
        assert_eq!(
            plan_detail(&installer, 3_113_851_290, Some(home)),
            "brightwater_1_3_58810.sh · 2.9 GB, and its saves"
        );
        assert_eq!(
            plan_detail(
                &StoreSource::Installed(InstalledWhy::Gogdl),
                23_192_823_398,
                Some(home)
            ),
            "Installed with gogdl, which keeps no installer, so the installed files are copied · 21.6 GB, and its saves"
        );
        assert_eq!(
            plan_detail(
                &StoreSource::Installed(InstalledWhy::Moved(
                    "/home/sam/Downloads/brightwater_1_3_58810.sh".into()
                )),
                1024,
                Some(home)
            ),
            "Its installer isn’t at ~/Downloads/brightwater_1_3_58810.sh any more, so the installed files are copied · 1.0 KB, and its saves"
        );
    }

    #[test]
    fn gog_save_locations_become_folder_strings() {
        let install = Path::new("/games/GOG/Ashen Tide");
        let prefix = Path::new("/games/GOG/Ashen Tide/prefix");
        let folder = |raw: &str| gog_save_folder(raw, install, Some(prefix));
        assert_eq!(
            folder("<?APPLICATION_DATA_ROAMING?>/Ashen/Saves").as_deref(),
            Some("%APPDATA%\\Ashen\\Saves")
        );
        assert_eq!(
            folder("<?SAVED_GAMES?>\\Ashen").as_deref(),
            Some("%USERPROFILE%\\Saved Games\\Ashen")
        );
        assert_eq!(
            folder("<?INSTALL?>/save").as_deref(),
            Some("<?INSTALL?>\\save")
        );
        assert_eq!(
            folder("%LOCALAPPDATA%\\X").as_deref(),
            Some("%LOCALAPPDATA%\\X")
        );
        assert_eq!(
            folder("/games/GOG/Ashen Tide/profiles").as_deref(),
            Some("<?INSTALL?>\\profiles")
        );
        assert_eq!(folder("<?APPLICATION_SUPPORT?>/Ashen"), None);
        assert_eq!(folder("<?INSTALL?>/../escape"), None);
        assert_eq!(folder("/elsewhere/saves"), None);

        let meta = GogMeta {
            save_locations: vec![
                SaveLocation {
                    name: "saves".into(),
                    path: "<?APPLICATION_DATA_ROAMING?>/Ashen".into(),
                },
                SaveLocation {
                    name: "cfg".into(),
                    path: "<?INSTALL?>/cfg".into(),
                },
            ],
            ..GogMeta::default()
        };
        assert_eq!(
            save_folders(
                &meta,
                &["%appdata%\\ashen".into(), "%LOCALAPPDATA%\\A".into()],
                install,
                None
            ),
            vec!["%APPDATA%\\Ashen", "<?INSTALL?>\\cfg", "%LOCALAPPDATA%\\A"]
        );
        assert!(is_install_folder("<?INSTALL?>\\cfg"));
        assert_eq!(
            resolve_save_folder("<?INSTALL?>\\cfg\\a", install, None, None),
            Some(install.join("cfg").join("a"))
        );
    }

    #[test]
    fn only_gog_can_be_backed_up() {
        assert_eq!(store_refusal(StoreKind::Gog), None);
        for kind in [StoreKind::Epic, StoreKind::Amazon, StoreKind::Steam] {
            assert_eq!(store_refusal(kind), Some(REASON_STORE));
        }
    }
}
