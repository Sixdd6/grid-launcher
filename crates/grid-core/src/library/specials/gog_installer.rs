//! RomM games whose archive holds a GOG offline installer (the GameStores
//! card, "RomM games with a GOG installer in their archive"). The user
//! zips a `setup_*.exe` with its `.bin` parts, or a Linux `.sh`, and
//! uploads it as a Windows or Linux game. After the download GRID finds the
//! installer ([`crate::stores::offline::detect_in_dir`]) and runs the
//! offline-installer flow into the game's own folder. **The game stays a
//! RomM game**: this module only puts the files on disk and says what the
//! row launches; the normal RomM finalize writes the row.
//!
//! Layout, like every native RomM install: `<platform>/<title>/game` holds
//! the installed game and `<platform>/<title>/prefix` the Wine prefix. A
//! DLC installer runs in its base game's folder and prefix instead.

use std::path::{Path, PathBuf};

use crate::library::queue::StoreProgressSink;
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use crate::stores::gog::goggame::GogGameInfo;
use crate::stores::gog::meta::{GogDlcMeta, GogMeta, GogPlatform};
use crate::stores::gog::row::gog_row;
use crate::stores::offline::{
    self, base_title_guess, installed_infos, linux_launch, read_installer, remove_install,
    row_platform, InstallerKind, InstallerSpawner, Refusal, Watch,
};
use crate::stores::{GameKey, StoreKind};

/// What one installer archive run needs.
pub struct ArchiveRun<'a> {
    /// The installer [`offline::detect_in_dir`] found.
    pub installer: &'a Path,
    /// The game's own folder, `<platform>/<title>`.
    pub game_dir: &'a Path,
    /// The RomM platform is a Windows one.
    pub windows_platform: bool,
    pub registry: &'a Registry,
    /// The compat tool a Windows installer runs under.
    pub compat: &'a str,
    pub wizard_allowed: bool,
    pub spawner: &'a dyn InstallerSpawner,
    pub watch: Watch,
}

/// What the RomM row launches after the installer ran.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArchiveInstall {
    /// The row's install folder (`extracted_dir`).
    pub extracted_dir: PathBuf,
    /// The executable (absolute), `""` when the game info names none.
    pub executable: String,
    pub arguments: String,
    /// Relative to `extracted_dir`, `/`-separated.
    pub working_dir: String,
    /// `""` for a native run.
    pub wineprefix: String,
    /// The compat tool the row keeps (a DLC: its base's), `""` for the
    /// default.
    pub compat_tool: String,
    /// The installed game's info.
    pub info: GogGameInfo,
    /// For a DLC: the base game's row it was added to.
    pub dlc_of: Option<InstalledGame>,
}

/// Whether a failed installer run removes the download (the card's
/// "The download was removed."): an unrecognised installer, a DLC without
/// its base, an installer whose setup window exited with an error. Silent
/// runs that failed where no setup window may open
/// ([`LibraryError::InstallerSilentFailed`], TV mode), or a cancel, keep it
/// for a retry.
pub fn removes_download(error: &LibraryError) -> bool {
    matches!(
        error,
        LibraryError::InstallerUnrecognised { .. }
            | LibraryError::InstallerDlcBaseMissing { .. }
            | LibraryError::InstallerExit(_)
    )
}

/// Runs the archive's installer. See the module docs for where it goes.
///
/// Errors use the card's failure codes: an installer with no GOG game info
/// is [`LibraryError::InstallerUnrecognised`], a DLC whose base game isn't
/// installed [`LibraryError::InstallerDlcBaseMissing`], and [`offline::run_into`]'s
/// own errors pass through. A fresh install that fails is removed.
pub fn install(
    run: &ArchiveRun<'_>,
    sink: &mut dyn StoreProgressSink,
) -> Result<ArchiveInstall, LibraryError> {
    let file = run
        .installer
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (insp, refusal) = read_installer(run.installer).map_err(|error| match error {
        crate::stores::StoreError::Library(error) => error,
        other => LibraryError::InstallerRefused(other.to_string()),
    })?;
    match refusal {
        None => {}
        Some(Refusal::NotGog) => return Err(LibraryError::InstallerUnrecognised { file }),
        Some(_) => {
            return Err(LibraryError::InstallerRefused(offline::refusal_message(
                &insp.verdict,
            )))
        }
    }
    let offered = insp.kind.platform().unwrap_or_default();
    let platform = if run.windows_platform {
        GogPlatform::Windows
    } else {
        GogPlatform::Linux
    };
    if offered != platform {
        return Err(LibraryError::InstallerRefused(format!(
            "Its archive holds a GOG {} installer, but the game is on a {} platform.",
            platform_word(offered),
            platform_word(platform)
        )));
    }

    // A DLC known before the run goes straight to its base.
    if let Some(info) = insp.info.as_ref().filter(|info| info.is_dlc()) {
        let base = base_for(run.registry, info, offered)?;
        return install_dlc(run, &insp, base, sink);
    }

    let dest = run.game_dir.join("game");
    let prefix = offline::needs_prefix(&insp.kind).then(|| run.game_dir.join("prefix"));
    remove_install(&dest, None);
    let info = match offline::run_into_with(
        &insp,
        &dest,
        prefix.as_deref(),
        run.compat,
        run.wizard_allowed,
        sink,
        run.spawner,
        run.watch,
    ) {
        Ok(info) => info,
        Err(error) => {
            remove_install(&dest, prefix.as_deref());
            return Err(error);
        }
    };
    if info.is_dlc() {
        // Its identity was only known after the run: move its files into
        // the base game's folder.
        let base = match base_for(run.registry, &info, offered) {
            Ok(base) => base,
            Err(error) => {
                remove_install(&dest, prefix.as_deref());
                return Err(error);
            }
        };
        let base_dir = PathBuf::from(base.extracted_dir.trim());
        super::copy_tree_merge(&dest, &base_dir)?;
        remove_install(&dest, prefix.as_deref());
        return Ok(dlc_install(run, base, info));
    }
    Ok(launch_of(
        &info,
        &dest,
        offered,
        prefix
            .map(|prefix| prefix.to_string_lossy().into_owned())
            .unwrap_or_default(),
    ))
}

fn platform_word(platform: GogPlatform) -> &'static str {
    match platform {
        GogPlatform::Linux => "Linux",
        GogPlatform::Windows => "Windows",
    }
}

fn install_dlc(
    run: &ArchiveRun<'_>,
    insp: &offline::Inspection,
    base: InstalledGame,
    sink: &mut dyn StoreProgressSink,
) -> Result<ArchiveInstall, LibraryError> {
    let dest = PathBuf::from(base.extracted_dir.trim());
    let prefix = base_prefix(&base, &insp.kind);
    let compat = if base.native_compat_tool.trim().is_empty() {
        run.compat.to_string()
    } else {
        base.native_compat_tool.trim().to_string()
    };
    let installed = offline::run_into_with(
        insp,
        &dest,
        prefix.as_deref(),
        &compat,
        run.wizard_allowed,
        sink,
        run.spawner,
        run.watch,
    )?;
    let info = if installed.is_dlc() {
        installed
    } else {
        insp.info.clone().unwrap_or_default()
    };
    Ok(dlc_install(run, base, info))
}

/// The base game's Wine prefix: its row's, else `<its home>/prefix`.
fn base_prefix(base: &InstalledGame, kind: &InstallerKind) -> Option<PathBuf> {
    if !offline::needs_prefix(kind) {
        return None;
    }
    let own = base.native_wineprefix.trim();
    if !own.is_empty() {
        return Some(PathBuf::from(own));
    }
    let home = base.native_game_dir.trim();
    let home = if home.is_empty() {
        base.extracted_dir.trim()
    } else {
        home
    };
    Some(Path::new(home).join("prefix"))
}

/// A DLC row launches its base game: its own folder is its RomM home
/// (which holds nothing once the archive is gone, so uninstalling the DLC
/// row never touches the base), and it runs the base's executable in the
/// base's prefix. A GOG base row records the DLC in its `meta.dlc`.
fn dlc_install(run: &ArchiveRun<'_>, base: InstalledGame, info: GogGameInfo) -> ArchiveInstall {
    if base.is_store() {
        let mut meta = GogMeta::from_json(&base.store_meta);
        if !meta.dlc.iter().any(|dlc| dlc.id == info.game_id) {
            meta.dlc.push(GogDlcMeta {
                id: info.game_id.clone(),
                title: info.name.clone(),
                installer: Vec::new(),
            });
            let mut updated = base.clone();
            updated.store_meta = meta.to_json();
            if let Err(error) = run.registry.upsert_store(&updated) {
                tracing::warn!(%error, "the DLC was not recorded on its GOG row");
            }
        }
    }
    let _ = std::fs::create_dir_all(run.game_dir);
    ArchiveInstall {
        extracted_dir: run.game_dir.to_path_buf(),
        executable: base.native_executable_path.clone(),
        arguments: base.native_launch_parameters.clone(),
        working_dir: String::new(),
        wineprefix: base.native_wineprefix.clone(),
        compat_tool: base.native_compat_tool.clone(),
        info,
        dlc_of: Some(base),
    }
}

/// The launch fields of a game installed in `dest`, from its play tasks
/// (a Linux game: `start.sh`).
fn launch_of(
    info: &GogGameInfo,
    dest: &Path,
    platform: GogPlatform,
    wineprefix: String,
) -> ArchiveInstall {
    let meta = GogMeta {
        platform,
        ..Default::default()
    };
    // The play-task folder: where the goggame file is (`game/` in a Linux
    // installer).
    let task_root = installed_infos(dest)
        .into_iter()
        .find(|(_, found)| found.game_id == info.game_id)
        .and_then(|(file, _)| file.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| dest.to_path_buf());
    let mut row = gog_row(info, &task_root, &meta, None, None);
    if platform == GogPlatform::Linux {
        linux_launch(&mut row, dest);
    }
    let working_dir = if row.native_working_dir.is_empty() {
        String::new()
    } else {
        match task_root.strip_prefix(dest) {
            Ok(rel) if !rel.as_os_str().is_empty() => format!(
                "{}/{}",
                rel.to_string_lossy().replace('\\', "/"),
                row.native_working_dir
            ),
            _ => row.native_working_dir.clone(),
        }
    };
    ArchiveInstall {
        extracted_dir: dest.to_path_buf(),
        executable: row.native_executable_path,
        arguments: row.native_launch_parameters,
        working_dir,
        wineprefix,
        compat_tool: String::new(),
        info: info.clone(),
        dlc_of: None,
    }
}

/// The installed base game of a DLC: its GOG row, else a native row (any
/// source) whose install folder holds the base's `goggame-<id>.info`. The
/// base must be the same OS version ("A Linux game takes Linux DLC
/// installers only").
fn base_for(
    registry: &Registry,
    dlc: &GogGameInfo,
    offered: GogPlatform,
) -> Result<InstalledGame, LibraryError> {
    let root = dlc.root_game_id.trim();
    let missing = || LibraryError::InstallerDlcBaseMissing {
        base_title: base_title_guess(&dlc.name),
        base_gog_id: root.to_string(),
    };
    let mut base = registry.find_key(&GameKey::store(StoreKind::Gog, root))?;
    if base.is_none() {
        base = registry.all()?.into_iter().find(|row| {
            row.is_native()
                && !row.extracted_dir.trim().is_empty()
                && installed_infos(Path::new(row.extracted_dir.trim()))
                    .iter()
                    .any(|(_, info)| info.game_id == root)
        });
    }
    let base = base.ok_or_else(missing)?;
    let installed = row_platform(&base);
    if installed != offered {
        return Err(LibraryError::InstallerRefused(format!(
            "Its archive holds a DLC installer for the {} version of {}, which is installed here as the {} version. A {} game takes {} DLC installers only.",
            platform_word(offered),
            base.title,
            platform_word(installed),
            platform_word(installed),
            platform_word(installed)
        )));
    }
    Ok(base)
}
