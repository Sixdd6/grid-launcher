//! Restores a game backed up as a copy of its install (an archive in parts,
//! `PackGame::installed`) from a backup drive into the library
//! (`docs/superpowers/plans/2026-11-09-game-backups-local-copies.md`,
//! Part 4).
//!
//! - [`plan_restore`] picks where the tree goes (always the modern platform
//!   folder) and checks the library has room, before admission.
//! - [`restore_from_drive`] is the row's download step: it reads the parts
//!   off the drive into `<platform folder>/.grid-restore-<rom id>`.
//! - [`InstallService::finalize_restore`] moves that folder into place and
//!   writes the registry row ([`restored_record`]).
//!
//! The Wine prefix is never in the archive: a native game restored on a
//! non-Windows host gets a fresh `prefix/`, and the saves the backup
//! copied out of the old one are put into it
//! ([`InstallService::restore_drive_saves`], which a drive install of the
//! server's files runs too).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use super::drive_install::DriveCopyHook;
use super::path_change::is_strictly_inside;
use super::registry::InstalledGame;
use super::{
    append_warning, is_appimage, is_ps3_platform, make_executable, new_record, path_string,
    platform_dir, removal_key, sanitize_component, specials, DriveSource, GameSource, InstallJob,
    InstallMode, InstallService, LibraryError,
};
use crate::backup::archive::{self, PartSource, RestoreError};
use crate::backup::local::WINE_PREFIX_DIR;
use crate::backup::pack::{self, InstalledLayout, PackGame, PackInstalled};
use crate::backup::saves;
use crate::backup::write::SPACE_MARGIN;
use crate::cloud::native::{row_wine_prefix, OwnedWinePrefix};
use crate::config::Config;
use crate::images::ImageFields;
use crate::romm::RomDetail;

/// The refusal for a copy this build cannot restore.
pub const NEEDS_NEWER_GRID: &str = "This backup needs a newer GRID";

/// The temporary folder a restore reads into, in the platform folder.
const TMP_PREFIX: &str = ".grid-restore-";

/// How an installed copy is restored: where its parts are on the drive and
/// where the tree goes on this computer.
#[derive(Clone, Debug)]
pub(super) struct RestorePlan {
    /// The archive's parts on the drive, in order (resolved, absolute).
    pub parts: Vec<PartSource>,
    /// Where the tree (or, for [`InstalledLayout::File`], the file) ends up.
    pub dest: PathBuf,
    /// `<platform folder>/.grid-restore-<rom id>`: the archive is read into
    /// it, then moved to `dest`.
    pub tmp: PathBuf,
    pub installed: PackInstalled,
}

/// The error for a backup whose copy cannot be used.
fn damaged(game: &str, label: &str) -> LibraryError {
    LibraryError::Extract(format!("The backup of {game} on {label} is damaged"))
}

/// Where a restored copy goes under `platform_root` (the modern platform
/// folder). Pure.
///
/// - `extracted`: `<platform>/<sanitized copy name>`
/// - `native` and `multi`: `<platform>/<sanitized game name>`, where a
///   server install puts them
/// - `file`: `<platform>/<copy name>`, which must be one plain name
pub(super) fn restore_destination(
    platform_root: &Path,
    game_name: &str,
    installed: &PackInstalled,
) -> Option<PathBuf> {
    match installed.layout {
        InstalledLayout::Extracted => {
            Some(platform_root.join(sanitize_component(&installed.name, "game")))
        }
        InstalledLayout::Native | InstalledLayout::Multi => {
            Some(platform_root.join(sanitize_component(game_name, "game")))
        }
        InstalledLayout::File => {
            let rel = pack::validate_rel(&installed.name).ok()?;
            (rel.components().count() == 1).then(|| platform_root.join(rel))
        }
        InstalledLayout::Unknown => None,
    }
}

/// The restore job for the installed copy `installed` of `game`.
///
/// Refused before admission: a copy this build cannot restore ("This backup
/// needs a newer GRID"), an unusable pack path, or a library without room
/// for `tree_bytes` plus [`SPACE_MARGIN`] (`free` is the library's free
/// space, `None` when it cannot be read, which skips the check).
///
/// The job runs in [`InstallMode::Base`] with no targets and no file ids,
/// so `already_installed` still short-circuits it.
pub(super) fn plan_restore(
    detail: &RomDetail,
    game: &PackGame,
    installed: &PackInstalled,
    library: &Path,
    drive: DriveSource,
    free: Option<u64>,
) -> Result<InstallJob, LibraryError> {
    if !installed.is_supported() {
        return Err(LibraryError::Extract(NEEDS_NEWER_GRID.to_string()));
    }
    let platform_root = platform_dir(library, &detail.platform_name);
    let dest = restore_destination(&platform_root, &detail.name, installed)
        .ok_or_else(|| damaged(&game.name, &drive.label))?;
    let parts = installed
        .parts
        .iter()
        .map(|part| {
            pack::resolve(&drive.mount, &part.path)
                .map(|path| PartSource {
                    path,
                    size: part.size,
                    md5: part.md5.clone(),
                })
                .map_err(|error| {
                    LibraryError::Extract(format!(
                        "The backup on {} can't be used: {error}",
                        drive.label
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.is_empty() {
        return Err(damaged(&game.name, &drive.label));
    }
    let needed = installed.tree_bytes.saturating_add(SPACE_MARGIN);
    if free.is_some_and(|free| free < needed) {
        return Err(LibraryError::Extract(format!(
            "There isn't enough free space in the library folder for {}",
            game.name
        )));
    }
    let tmp = platform_root.join(format!("{TMP_PREFIX}{}", detail.id));
    let launch_entry = if installed.rom_file_name.is_empty() {
        detail.fs_name.clone()
    } else {
        installed.rom_file_name.clone()
    };
    Ok(InstallJob {
        space: None,
        rom_id: detail.id,
        mode: InstallMode::Base,
        content_kind: None,
        file_ids: Vec::new(),
        detail: detail.clone(),
        targets: Vec::new(),
        primary_archive: dest.clone(),
        multi_file_game_dir: None,
        launch_entry,
        native_game_dir: None,
        game_json_target: None,
        source: GameSource::Drive(drive),
        restore: Some(RestorePlan {
            parts,
            dest,
            tmp,
            installed: installed.clone(),
        }),
    })
}

/// The download step of a restore: reads the archive's parts off the drive
/// into `plan.tmp` on the blocking pool, running `hook` after each chunk.
///
/// - `on_progress` gets `(read, total, avg_speed_bps)` over the parts' size.
/// - An error while the drive's `gridpack` folder is gone is
///   [`LibraryError::DriveRemoved`] ("<label> was removed"); a part whose
///   size or MD5 is wrong, a missing part or an unsafe entry is "The backup
///   of <Game> on <label> is damaged".
/// - `plan.tmp` is removed on any error or cancel.
pub(super) async fn restore_from_drive(
    drive: &DriveSource,
    game_name: &str,
    plan: &RestorePlan,
    cancel: Arc<AtomicBool>,
    hook: Option<DriveCopyHook>,
    mut on_progress: impl FnMut(u64, u64, f64) + Send + 'static,
) -> Result<(), LibraryError> {
    let root = pack::pack_dir(&drive.mount);
    let label = drive.label.clone();
    let game = game_name.to_string();
    let parts = plan.parts.clone();
    let tmp = plan.tmp.clone();
    let result = tokio::task::spawn_blocking(move || {
        // A folder a crashed restore left behind would make the restore
        // refuse a non-empty destination.
        remove_path(&tmp);
        if let Some(parent) = tmp.parent() {
            fs::create_dir_all(parent).map_err(LibraryError::Io)?;
        }
        let start = Instant::now();
        let mut progress = |read: u64, total: u64| {
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                read as f64 / elapsed
            } else {
                0.0
            };
            on_progress(read, total, speed);
        };
        let mut after_chunk = || hook.as_ref().map_or(Ok(()), |hook| hook());
        let result = archive::restore_tree(
            &root,
            &parts,
            &tmp,
            &cancel,
            &mut progress,
            &mut after_chunk,
        );
        result.map_err(|error| restore_error(error, &root, &game, &label))
    })
    .await
    .map_err(|e| LibraryError::Extract(format!("the copy did not finish: {e}")))
    .and_then(|result| result);
    if result.is_err() {
        remove_path(&plan.tmp);
    }
    result
}

/// The library error for a failed restore.
fn restore_error(error: RestoreError, root: &Path, game: &str, label: &str) -> LibraryError {
    match error {
        RestoreError::Cancelled => LibraryError::Cancelled,
        RestoreError::DriveRemoved => LibraryError::DriveRemoved(format!("{label} was removed")),
        RestoreError::Io(_) if !root.is_dir() => {
            LibraryError::DriveRemoved(format!("{label} was removed"))
        }
        RestoreError::Io(error) => LibraryError::Io(error),
        RestoreError::PartMismatch(_) | RestoreError::Incomplete(_) | RestoreError::Unsafe(_) => {
            damaged(game, label)
        }
    }
}

/// Removes a file or a folder with everything in it; silent when it is
/// not there.
pub(super) fn remove_path(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => {
            let _ = fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = fs::remove_file(path);
        }
        Err(_) => {}
    }
}

/// `rel` (a `/`-separated path inside the restored tree) under `dir`, when
/// it is a plain path to a file that resolves inside `dir`.
fn file_inside(dir: &Path, rel: &str) -> Option<PathBuf> {
    let path = dir.join(pack::validate_rel(rel).ok()?);
    if !path.is_file() {
        return None;
    }
    let inside = match (
        crate::platform::canonicalize(&path),
        crate::platform::canonicalize(dir),
    ) {
        (Ok(path), Ok(dir)) => path.starts_with(dir),
        _ => false,
    };
    inside.then_some(path)
}

/// `rel` under `dest` when it is a plain relative path; `""` otherwise.
fn tree_path(dest: &Path, rel: &str) -> String {
    if rel.is_empty() {
        return String::new();
    }
    pack::validate_rel(rel)
        .map(|rel| path_string(&dest.join(rel)))
        .unwrap_or_default()
}

/// The registry row of a copy restored at `dest`. Pure: the paths come
/// from the copy's relative values, which [`InstallService::finalize_restore`]
/// checks against the tree before this row is written.
///
/// - Metadata comes from `detail`, the ROM file name and the server
///   timestamp from the copy, so a copy that was out of date when it was
///   backed up shows Update once connected.
/// - `source_server` is `server`; `rom_id` is kept only with `keep_rom_id`.
/// - The path columns follow the layout (`extracted`, `native`, `multi`,
///   `file`), the same columns a server install of that shape fills.
/// - A native game on a non-Windows host gets `dest/prefix` as its Wine
///   prefix; the compat tool is left blank so the default applies.
pub fn restored_record(
    detail: &RomDetail,
    installed: &PackInstalled,
    dest: &Path,
    keep_rom_id: bool,
    server: &str,
) -> InstalledGame {
    let rom_file_name = if installed.rom_file_name.is_empty() {
        detail.fs_name.as_str()
    } else {
        installed.rom_file_name.as_str()
    };
    let mut record = new_record(detail, rom_file_name);
    record.server_updated_at = installed.server_updated_at.clone();
    record.source_server = server.to_string();
    if !keep_rom_id {
        record.rom_id = None;
    }
    let row = &installed.row;
    record.included_dlc = row.included_dlc.clone();
    record.ra_id = row.ra_id.clone();
    let launch = tree_path(dest, &installed.launch);
    match installed.layout {
        InstalledLayout::Extracted => {
            record.extracted_dir = path_string(dest);
            record.extracted_path = launch;
            record.ps4_game_id = row.ps4_game_id.clone();
            record.ps4_content = row.ps4_content.clone();
        }
        InstalledLayout::Native => {
            record.native_game_dir = path_string(dest);
            let game_folder =
                installed.launch.starts_with("game/") || row.native_executable.starts_with("game/");
            if game_folder {
                record.extracted_dir = path_string(&dest.join("game"));
            }
            record.extracted_path = launch;
            record.native_executable_path = tree_path(dest, &row.native_executable);
            record.native_launch_parameters = row.native_launch_parameters.clone();
            record.native_launch_source = row.native_launch_source.clone();
            record.native_working_dir = row.native_working_dir.clone();
            if cfg!(not(windows)) {
                record.native_wineprefix = path_string(&dest.join(WINE_PREFIX_DIR));
            }
            // A native install that kept its archive inside its folder: it
            // extracted nothing, so, as on a server install, the row names
            // the archive and no extracted launch file (the registry keeps
            // only one of the two).
            record.archive_path = tree_path(dest, &row.archive_path);
            if !record.archive_path.is_empty() {
                record.extracted_path.clear();
            }
        }
        InstalledLayout::Multi => {
            record.multi_file_game_dir = path_string(dest);
            record.extracted_path = launch;
        }
        InstalledLayout::File | InstalledLayout::Unknown => {
            record.archive_path = path_string(dest);
            let iso = dest
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("iso"));
            if is_ps3_platform(&detail.platform_name) && iso {
                record.ps3_iso_path = path_string(dest);
                record.extracted_path = path_string(dest);
            }
        }
    }
    record
}

/// Whether some registry row records `dest` itself, or a path inside it.
fn dest_in_use(rows: &[InstalledGame], dest: &Path) -> bool {
    let key = removal_key(dest, None);
    rows.iter().any(|row| {
        [
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
        .any(|raw| {
            let path = Path::new(raw);
            removal_key(path, None) == key || is_strictly_inside(path, dest, None)
        })
    })
}

impl InstallService {
    /// Finalizes a restore: moves the restored tree from `plan.tmp` to
    /// `plan.dest` (replacing a `dest` no registry row records), checks the
    /// launch file, and writes the row ([`restored_record`]). Nothing is
    /// registered for Xbox 360 or the Emulators platform: neither is ever
    /// backed up as an installed copy. The temporary folder is removed on
    /// any error, and so is the moved tree when the row cannot be written.
    pub(super) fn finalize_restore(
        self: &Arc<Self>,
        job: &InstallJob,
        plan: &RestorePlan,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let result = self.finalize_restore_inner(job, plan, warning);
        remove_path(&plan.tmp);
        result
    }

    fn finalize_restore_inner(
        self: &Arc<Self>,
        job: &InstallJob,
        plan: &RestorePlan,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let GameSource::Drive(drive) = &job.source else {
            return Err(LibraryError::Extract(
                "this restore lost its drive".to_string(),
            ));
        };
        let detail = &job.detail;
        let installed = &plan.installed;
        let dest = plan.dest.as_path();
        let file_layout = installed.layout == InstalledLayout::File;
        let restored = if file_layout {
            plan.tmp.join(&installed.name)
        } else {
            plan.tmp.clone()
        };
        let restored_ok = if file_layout {
            restored.is_file()
        } else {
            restored.is_dir()
        };
        if !restored_ok {
            return Err(damaged(&detail.name, &drive.label));
        }

        if fs::symlink_metadata(dest).is_ok() {
            let rows = self.registry.all()?;
            if dest_in_use(&rows, dest) {
                return Err(LibraryError::Extract(format!(
                    "{} is already used by another game",
                    dest.display()
                )));
            }
            remove_path(dest);
        }
        fs::rename(&restored, dest)?;

        match self.write_restored_row(job, drive, plan, warning) {
            Ok(()) => Ok(()),
            Err(error) => {
                remove_path(dest);
                Err(error)
            }
        }
    }

    fn write_restored_row(
        self: &Arc<Self>,
        job: &InstallJob,
        drive: &DriveSource,
        plan: &RestorePlan,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let detail = &job.detail;
        let installed = &plan.installed;
        let dest = plan.dest.as_path();
        let mut record = restored_record(detail, installed, dest, drive.keep_rom_id, &drive.server);

        match installed.layout {
            InstalledLayout::File => {
                if is_appimage(dest) {
                    make_executable(dest);
                }
            }
            _ => {
                if installed.launch.is_empty() {
                    if installed.layout != InstalledLayout::Native {
                        return Err(LibraryError::NoLaunchFile);
                    }
                } else {
                    let launch = file_inside(dest, &installed.launch).ok_or_else(|| {
                        LibraryError::Extract(format!(
                            "The backup of {} has no launch file at {}",
                            detail.name, installed.launch
                        ))
                    })?;
                    make_executable(&launch);
                    record.extracted_path = path_string(&launch);
                }
            }
        }

        if installed.layout == InstalledLayout::Native {
            let exe = &installed.row.native_executable;
            match file_inside(dest, exe) {
                Some(path) if !exe.is_empty() => {
                    make_executable(&path);
                    record.native_executable_path = path_string(&path);
                }
                _ => record.native_executable_path.clear(),
            }
            if !record.extracted_dir.is_empty() && !dest.join("game").is_dir() {
                record.extracted_dir.clear();
            }
            // The kept archive must be a file inside the restored tree.
            match file_inside(dest, &installed.row.archive_path) {
                Some(path) if !installed.row.archive_path.is_empty() => {
                    record.archive_path = path_string(&path);
                    record.extracted_path.clear();
                }
                _ => record.archive_path.clear(),
            }
            if !record.native_wineprefix.is_empty() {
                let prefix = dest.join(WINE_PREFIX_DIR);
                if fs::create_dir_all(&prefix).is_err() {
                    record.native_wineprefix.clear();
                }
            }
            // The sidecar travels inside the tree: its metadata fills what
            // the pack's detail left blank, as on a server install.
            if let Ok(bytes) = fs::read(dest.join(super::METADATA_FILE_NAME)) {
                if let Some(parsed) = specials::native::parse_game_json(&bytes) {
                    specials::native::apply_game_json(&mut record, &parsed);
                    if !installed.row.included_dlc.is_empty() {
                        record.included_dlc = installed.row.included_dlc.clone();
                    }
                }
            }
        }

        self.registry.upsert(&record)?;
        // Before the finalized hook: it looks the save folders up for
        // cloud saves only when the config has none.
        self.restore_drive_saves(job, &record, warning);

        let hook = self.image_hook.read().unwrap().clone();
        if let Some(hook) = hook {
            hook(ImageFields {
                cover_small_path: record.cover_small_path.clone(),
                cover_large_path: record.cover_large_path.clone(),
                screenshot_urls: record.screenshot_urls.clone(),
                fanart_urls: record.fanart_urls.clone(),
            });
        }
        let finalized = self.game_finalized_hook.read().unwrap().clone();
        if let Some(finalized) = finalized {
            finalized(record);
        }
        Ok(())
    }

    /// Puts a Windows game's saves from the drive back, once its row
    /// (`record`) is written. Both drive installs end here: a restored copy
    /// ([`Self::finalize_restore`]) and the server's files copied off the
    /// drive (`finalize_base`).
    ///
    /// 1. The save folders go to cloud saves ([`saves::seed_folders`]), so
    ///    they are known with no lookup.
    /// 2. The saves archive, if any, is read into a temporary folder in the
    ///    platform folder and each folder copied to its place
    ///    ([`saves::restore_saves`]): inside the row's Wine prefix off
    ///    Windows, the real folder on Windows, and never into a folder that
    ///    already holds files.
    ///
    /// Nothing here fails the install: every problem is a line in `warning`.
    pub(super) fn restore_drive_saves(
        &self,
        job: &InstallJob,
        record: &InstalledGame,
        warning: &mut String,
    ) {
        let GameSource::Drive(drive) = &job.source else {
            return;
        };
        let Some(drive_saves) = &drive.saves else {
            return;
        };
        if drive_saves.folders.is_empty() {
            return;
        }
        // The Windows default compat tool decides a new prefix's user.
        let mut default_compat_tool = String::new();
        let seeded = Config::load(&self.config_path).and_then(|mut config| {
            default_compat_tool = config.default_compat_tool.clone();
            if saves::seed_folders(&mut config, &record.title, &drive_saves.folders) {
                config.save(&self.config_path)?;
            }
            Ok(())
        });
        if let Err(error) = seeded {
            append_warning(
                warning,
                &format!("Its save folders weren't added to cloud saves: {error}"),
            );
        }
        if drive_saves.unusable {
            append_warning(
                warning,
                &format!("Its saves on {} can't be used", drive.label),
            );
            return;
        }
        if drive_saves.parts.is_empty() {
            return;
        }
        let prefix = if cfg!(windows) {
            None
        } else {
            // Under `steamuser` or this computer's user name, as the prefix
            // and the compat tool that will run the game say.
            match row_wine_prefix(record, &default_compat_tool, None) {
                None => {
                    append_warning(warning, "Its saves weren't restored: it has no Wine prefix");
                    return;
                }
                prefix => prefix,
            }
        };
        let tmp = match self.library_root() {
            Ok(library) => platform_dir(&library, &job.detail.platform_name)
                .join(format!("{SAVES_TMP_PREFIX}{}", job.detail.id)),
            Err(error) => {
                append_warning(warning, &format!("Its saves weren't restored: {error}"));
                return;
            }
        };
        let result = saves::restore_saves(
            &pack::pack_dir(&drive.mount),
            &drive_saves.parts,
            &drive_saves.folders,
            &tmp,
            prefix.as_ref().map(OwnedWinePrefix::as_prefix),
            crate::platform::windows_documents_dir().as_deref(),
            &AtomicBool::new(false),
        );
        match result {
            Ok(restored) => {
                for line in restored.warnings(&drive_saves.folders) {
                    append_warning(warning, &line);
                }
            }
            Err(RestoreError::DriveRemoved) => append_warning(
                warning,
                &format!("Its saves weren't restored: {} was removed", drive.label),
            ),
            Err(error) => append_warning(warning, &format!("Its saves weren't restored: {error}")),
        }
    }
}

/// The temporary folder a restore reads a game's saves into, in the
/// platform folder.
const SAVES_TMP_PREFIX: &str = ".grid-saves-";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::pack::{ArchiveKind, PackInstalledRow, PackPart};
    use std::collections::BTreeMap;

    const SERVER: &str = "https://romm.example";

    fn library() -> PathBuf {
        std::env::temp_dir().join("grid-restore-test-library")
    }

    fn installed(layout: InstalledLayout, name: &str, launch: &str) -> PackInstalled {
        PackInstalled {
            archive: ArchiveKind::Tar,
            layout,
            name: name.to_string(),
            parts: vec![PackPart {
                path: format!("games/x/{name}/installed.tar.001"),
                size: 2048,
                md5: "00".repeat(16),
            }],
            tree_bytes: 1000,
            tree_files: 1,
            launch: launch.to_string(),
            rom_file_name: "Ember Run (v1.01).zip".to_string(),
            server_updated_at: "2026-01-01T00:00:00Z".to_string(),
            version: "v1.01".to_string(),
            outdated: true,
            from_os: None,
            row: PackInstalledRow::default(),
        }
    }

    fn game_on(platform: &str, installed: &PackInstalled) -> PackGame {
        serde_json::from_value(serde_json::json!({
            "platform": "x",
            "platform_name": platform,
            "source": {
                "server": SERVER,
                "rom_id": 77,
                "fs_name": "Ember Run",
                "server_updated_at": "2026-06-01T00:00:00Z",
            },
            "name": "Ember: Run",
            "size": 2048,
            "added": "2026-11-09T00:00:00Z",
            "installed": installed,
        }))
        .unwrap()
    }

    fn game(installed: &PackInstalled) -> PackGame {
        game_on("PlayStation 4", installed)
    }

    fn detail(platform: &str) -> RomDetail {
        let copy = installed(InstalledLayout::Extracted, "Game", "");
        pack::pack_game_detail(&game_on(platform, &copy))
    }

    fn drive(mount: &Path) -> DriveSource {
        DriveSource {
            drive_id: "d".to_string(),
            label: "SAMSUNG T7".to_string(),
            mount: mount.to_path_buf(),
            files: BTreeMap::new(),
            server: SERVER.to_string(),
            keep_rom_id: true,
            saves: None,
        }
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn destinations_follow_the_layout_under_the_modern_platform_folder() {
        let root = platform_dir(&library(), "PlayStation 4");
        let cases = [
            (
                InstalledLayout::Extracted,
                "CUSA:0001",
                root.join("CUSA_0001"),
            ),
            (InstalledLayout::Native, "whatever", root.join("Ember_ Run")),
            (InstalledLayout::Multi, "whatever", root.join("Ember_ Run")),
            (
                InstalledLayout::File,
                "Ember Run.iso",
                root.join("Ember Run.iso"),
            ),
        ];
        for (layout, name, want) in cases {
            let copy = installed(layout, name, "");
            assert_eq!(
                restore_destination(&root, "Ember: Run", &copy),
                Some(want),
                "{layout:?}"
            );
        }
        for bad in ["../x.iso", "a/b.iso", ""] {
            let copy = installed(InstalledLayout::File, bad, "");
            assert_eq!(
                restore_destination(&root, "Ember: Run", &copy),
                None,
                "{bad}"
            );
        }
        let copy = installed(InstalledLayout::Unknown, "x", "");
        assert_eq!(restore_destination(&root, "Ember: Run", &copy), None);
    }

    #[test]
    fn plan_restore_builds_a_base_job_with_no_targets() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("gridpack")).unwrap();
        let copy = installed(InstalledLayout::Extracted, "CUSA00001", "eboot.bin");
        let detail = detail("PlayStation 4");
        let job = plan_restore(
            &detail,
            &game(&copy),
            &copy,
            &library(),
            drive(tmp.path()),
            None,
        )
        .unwrap();
        assert_eq!(job.mode, InstallMode::Base);
        assert!(job.targets.is_empty() && job.file_ids.is_empty());
        let plan = job.restore.expect("a restore plan");
        let root = platform_dir(&library(), "PlayStation 4");
        assert_eq!(plan.dest, root.join("CUSA00001"));
        assert_eq!(plan.tmp, root.join(".grid-restore-77"));
        assert_eq!(
            plan.parts[0].path,
            tmp.path()
                .join("gridpack/games/x/CUSA00001/installed.tar.001")
        );
        assert_eq!(job.launch_entry, "Ember Run (v1.01).zip");
    }

    #[test]
    fn plan_restore_refuses_a_full_library_and_an_unknown_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let copy = installed(InstalledLayout::Extracted, "CUSA00001", "eboot.bin");
        let detail = detail("PlayStation 4");
        let plan = |copy: &PackInstalled, free| {
            plan_restore(
                &detail,
                &game(copy),
                copy,
                &library(),
                drive(tmp.path()),
                free,
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        };
        let needed = 1000 + SPACE_MARGIN;
        assert_eq!(
            plan(&copy, Some(needed - 1)).unwrap_err(),
            "There isn't enough free space in the library folder for Ember: Run"
        );
        assert!(plan(&copy, Some(needed)).is_ok());
        assert!(plan(&copy, None).is_ok());

        let mut unknown = copy.clone();
        unknown.layout = InstalledLayout::Unknown;
        assert_eq!(plan(&unknown, None).unwrap_err(), NEEDS_NEWER_GRID);
        let mut unknown = copy.clone();
        unknown.archive = ArchiveKind::Unknown;
        assert_eq!(plan(&unknown, None).unwrap_err(), NEEDS_NEWER_GRID);

        let mut unsafe_part = copy.clone();
        unsafe_part.parts[0].path = "../escape.tar.001".to_string();
        assert!(plan(&unsafe_part, None)
            .unwrap_err()
            .starts_with("The backup on SAMSUNG T7 can't be used"));
    }

    #[test]
    fn restored_record_fills_the_columns_of_each_layout() {
        let dest = library().join("games").join("X").join("Game");
        let at = |rel: &str| text(&dest.join(rel));

        // extracted (PS4)
        let mut copy = installed(InstalledLayout::Extracted, "Game", "CUSA00001/eboot.bin");
        copy.row.ps4_game_id = "CUSA00001".to_string();
        copy.row.ps4_content = "{}".to_string();
        let row = restored_record(&detail("PlayStation 4"), &copy, &dest, true, SERVER);
        assert_eq!(row.extracted_dir, text(&dest));
        assert_eq!(row.extracted_path, at("CUSA00001/eboot.bin"));
        assert_eq!(row.ps4_game_id, "CUSA00001");
        assert_eq!(row.ps4_content, "{}");
        assert_eq!(row.rom_id, Some(77));
        assert_eq!(row.source_server, SERVER);
        assert_eq!(row.server_updated_at, "2026-01-01T00:00:00Z");
        assert_eq!(row.rom_file_name, "Ember Run (v1.01).zip");
        assert!(row.native_game_dir.is_empty() && row.archive_path.is_empty());

        // native
        let mut copy = installed(InstalledLayout::Native, "Game", "game/run.sh");
        copy.row.native_executable = "game/bin/run".to_string();
        copy.row.native_launch_parameters = "-windowed".to_string();
        copy.row.native_launch_source = "sidecar".to_string();
        copy.row.native_working_dir = "game/bin".to_string();
        let row = restored_record(&detail("Linux"), &copy, &dest, false, SERVER);
        assert_eq!(row.rom_id, None);
        assert_eq!(row.native_game_dir, text(&dest));
        assert_eq!(row.extracted_dir, at("game"));
        assert_eq!(row.extracted_path, at("game/run.sh"));
        assert_eq!(row.native_executable_path, at("game/bin/run"));
        assert_eq!(row.native_launch_parameters, "-windowed");
        assert_eq!(row.native_launch_source, "sidecar");
        assert_eq!(row.native_working_dir, "game/bin");
        assert_eq!(row.native_compat_tool, "");
        if cfg!(windows) {
            assert_eq!(row.native_wineprefix, "");
        } else {
            assert_eq!(row.native_wineprefix, at("prefix"));
        }

        assert_eq!(row.archive_path, "", "no kept archive");

        // native, an unsafe executable path is dropped
        copy.row.native_executable = "../escape".to_string();
        let row = restored_record(&detail("Linux"), &copy, &dest, true, SERVER);
        assert_eq!(row.native_executable_path, "");

        // native, a kept archive inside the tree comes back; an unsafe one
        // is dropped
        copy.row.archive_path = "Ember Run.zip".to_string();
        let row = restored_record(&detail("Linux"), &copy, &dest, true, SERVER);
        assert_eq!(row.archive_path, at("Ember Run.zip"));
        assert_eq!(row.extracted_path, "", "the archive is the install");
        copy.row.archive_path = "../Ember Run.zip".to_string();
        let row = restored_record(&detail("Linux"), &copy, &dest, true, SERVER);
        assert_eq!(row.archive_path, "");

        // multi
        let copy = installed(InstalledLayout::Multi, "Game", "Game.m3u");
        let row = restored_record(&detail("PlayStation"), &copy, &dest, true, SERVER);
        assert_eq!(row.multi_file_game_dir, text(&dest));
        assert_eq!(row.extracted_path, at("Game.m3u"));
        assert!(row.extracted_dir.is_empty());

        // file, and a PS3 ISO
        let file = library().join("games").join("X").join("Game.iso");
        let copy = installed(InstalledLayout::File, "Game.iso", "");
        let row = restored_record(&detail("SNES"), &copy, &file, true, SERVER);
        assert_eq!(row.archive_path, text(&file));
        assert!(row.ps3_iso_path.is_empty() && row.extracted_path.is_empty());
        let row = restored_record(&detail("PlayStation 3"), &copy, &file, true, SERVER);
        assert_eq!(row.archive_path, text(&file));
        assert_eq!(row.ps3_iso_path, text(&file));
        assert_eq!(row.extracted_path, text(&file));
    }

    #[test]
    fn a_dest_recorded_by_a_row_is_in_use() {
        let dest = library().join("games").join("X").join("Game");
        let other = InstalledGame {
            extracted_dir: text(&library().join("games").join("X").join("Other")),
            ..Default::default()
        };
        assert!(!dest_in_use(std::slice::from_ref(&other), &dest));
        let same = InstalledGame {
            extracted_dir: text(&dest),
            ..Default::default()
        };
        assert!(dest_in_use(&[other.clone(), same], &dest));
        let inside = InstalledGame {
            extracted_path: text(&dest.join("a.bin")),
            ..Default::default()
        };
        assert!(dest_in_use(&[inside], &dest));
    }
}
