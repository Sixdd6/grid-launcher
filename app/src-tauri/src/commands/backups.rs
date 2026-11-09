//! Backup-drive commands (`docs/superpowers/plans/2026-11-09-game-backups.md`
//! §1.3). Thin wrappers: grid-core's `backup` module and `InstallService`
//! own every rule, `drive_service.rs` owns the scan.
//!
//! Every command names a drive by its key (the mount path, or the drive
//! letter on Windows) and resolves it against the latest scan; none takes a
//! raw path to write to. `setup_drive` takes the mount the Settings pane
//! shows and refuses one the current scan did not find.
//!
//! Errors and logs carry drive labels, paths and game names, never a server
//! URL with userinfo or a header.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use grid_core::backup::pack::{self, Pack, PackEmulator, PackGame, PackOs, PackState, TMP_SUFFIX};
use grid_core::backup::plan::{
    self, BackupPlan, BackupQueued, EmulatorItem, EmulatorKind, EmulatorPick, EmulatorSource,
    LocalState, Outdated, PlanFirmwareSet, PlanGame, PlanInput, Planned, SourceChoice,
};
use grid_core::backup::write;
use grid_core::config::{Config, PendingDefault};
use grid_core::images::urls::resolve_image_url;
use grid_core::launch::compat::{discover, request_compat_default, which_on_path};
use grid_core::launch::profiles::load_profiles;
use grid_core::library::platforms::{is_native_platform, is_windows_platform};
use grid_core::library::registry::InstalledGame;
use grid_core::library::update_detection::{game_has_server_update, ServerVersion};
use grid_core::library::InstallService;
use grid_core::romm::{strip_userinfo, RommClient};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::{ensure_library_idle, err, AppState};
use crate::drive_service::{
    drive_info, DriveInfo, DrivesSnapshot, PendingDriveFirmware, ScannedDrive,
};
use crate::update_service::{local_version_label, server_version_label, UpdateService};

/// Why a backup command needs a live session.
const NEEDS_SERVER: &str = "Backing up needs a server";

// --- DTOs ---------------------------------------------------------------------

/// What the Backup dialog asks to backup. `platform_ids` is optional: the
/// platforms the games are on, when the caller knows them (the Server grid
/// does). With them the plan reads one game list per platform; without them
/// it reads each game's detail.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BackupRequest {
    pub rom_ids: Vec<i64>,
    #[serde(default)]
    pub platform_ids: Vec<i64>,
    /// The dialog's title source; not read by the backend.
    #[serde(default)]
    #[allow(dead_code)]
    pub title: String,
}

/// The user's pick for one out-of-date local game: this computer's copy
/// or the server's newer files.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct SourcePick {
    pub rom_id: i64,
    pub source: SourceChoice,
}

/// What the Backup dialog chose besides the games: the "Also copy" boxes,
/// "Copy installed games from this computer" and the per-game picks.
#[derive(Debug, Clone, Default)]
struct PlanOptions {
    include_firmware: bool,
    include_runtime: bool,
    use_local: bool,
    choices: HashMap<i64, SourceChoice>,
}

/// What changing a pack's computers did.
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct OsChangeOutcome {
    /// `0` when nothing was queued.
    pub batch: u64,
    /// Emulator builds queued for the added OS.
    pub queued: u32,
    /// Emulator builds removed for the removed OS.
    pub removed: u32,
}

/// One backed-up game, as Library › Backup drives shows it. Art fields are
/// pack-relative paths for `drive_asset`, blank when the pack has none.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DriveGame {
    pub rom_id: i64,
    /// The server it was backed up from (never with userinfo).
    pub server: String,
    pub name: String,
    /// The RomM platform slug.
    pub platform: String,
    pub platform_name: String,
    pub size: u64,
    /// The largest single file.
    pub files_max: u64,
    pub cover: String,
    pub fanart: String,
    pub logo: String,
    pub screenshots: Vec<String>,
    pub summary: String,
    pub region: String,
    pub languages: String,
    pub genres: Vec<String>,
    pub companies: Vec<String>,
    pub franchises: Vec<String>,
    pub game_modes: Vec<String>,
    pub release_date: String,
    pub players: String,
    pub rating: String,
    pub tags: String,
    pub revision: String,
    pub version: String,
    pub added: String,
    /// Already installed on this computer.
    pub installed: bool,
    /// A copy of the install on the computer that backed it up, not the
    /// server's files.
    pub installed_copy: bool,
    /// The installed copy's version (`"v1.2"`), else empty.
    pub installed_version: String,
    /// The server had newer files when the installed copy was backed up.
    pub outdated: bool,
    /// A Windows game whose saves were copied out of its Wine prefix.
    pub saves_copied: bool,
}

/// What `install_from_drive` queued besides the game, as toast lines
/// ("DuckStation from Couch kit added to download queue").
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct DriveInstallOutcome {
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BackupSettingsDto {
    pub include_firmware: bool,
}

// --- pure helpers ---------------------------------------------------------------

/// The picks by rom id. Pure. A rom id picked twice keeps the last pick.
pub fn choices_map(picks: Option<Vec<SourcePick>>) -> HashMap<i64, SourceChoice> {
    picks
        .unwrap_or_default()
        .into_iter()
        .map(|pick| (pick.rom_id, pick.source))
        .collect()
}

/// Why `start_backup` refuses a plan that still waits on picks, or `None`.
/// Pure.
pub fn needs_choice_refusal(needs_choice: u32) -> Option<String> {
    match needs_choice {
        0 => None,
        1 => Some("Choose which copy to backup for 1 game".to_string()),
        n => Some(format!("Choose which copy to backup for {n} games")),
    }
}

/// Fills `outdated` on each eligible local game that has newer files on the
/// server. Pure. `newer` maps a rom id to the server's ROM file name, for
/// the games known to have an update; `rows` are the registry rows, whose
/// ROM file name gives this computer's version.
pub fn fill_outdated(
    states: &mut HashMap<i64, LocalState>,
    rows: &[InstalledGame],
    newer: &HashMap<i64, String>,
) {
    for (rom_id, state) in states.iter_mut() {
        let LocalState::Eligible { outdated, .. } = state else {
            continue;
        };
        let Some(server_file) = newer.get(rom_id) else {
            *outdated = None;
            continue;
        };
        let installed_file = rows
            .iter()
            .find(|row| row.rom_id == Some(*rom_id))
            .map(|row| row.rom_file_name.as_str())
            .unwrap_or_default();
        *outdated = Some(Outdated {
            installed_version: local_version_label(installed_file),
            server_version: server_version_label(installed_file, server_file),
        });
    }
}

/// The eligible local games among `states` with a newer server copy, as
/// the update set has them: rom id to the server's ROM file name. `None`
/// while no update pass has finished this session (an absent entry then
/// means "not checked yet").
pub fn newer_from_updates(
    updates: &UpdateService,
    states: &HashMap<i64, LocalState>,
) -> Option<HashMap<i64, String>> {
    if !updates.has_completed_pass() {
        return None;
    }
    Some(
        eligible_ids(states)
            .filter_map(|rom_id| {
                updates
                    .info(rom_id)
                    .map(|info| (rom_id, info.server_rom_file_name))
            })
            .collect(),
    )
}

fn eligible_ids(states: &HashMap<i64, LocalState>) -> impl Iterator<Item = i64> + '_ {
    states
        .iter()
        .filter(|(_, state)| matches!(state, LocalState::Eligible { .. }))
        .map(|(rom_id, _)| *rom_id)
}

/// Every art path `game` names on the drive: cover, small cover, fanart,
/// logo and screenshots. Never a game file or archive part.
pub fn art_paths(game: &PackGame) -> impl Iterator<Item = &str> {
    let assets = &game.assets;
    [
        &assets.cover,
        &assets.cover_small,
        &assets.fanart,
        &assets.logo,
    ]
    .into_iter()
    .flatten()
    .map(String::as_str)
    .chain(assets.screenshots.iter().map(String::as_str))
}

/// The game in `pack` whose art `rel` is. Pure. `None` for anything else,
/// a game file or an archive part included.
pub fn art_owner<'a>(pack: &'a Pack, rel: &str) -> Option<&'a PackGame> {
    pack.games
        .iter()
        .find(|game| art_paths(game).any(|path| path == rel))
}

/// Whether `game` from a pack is installed on this computer. Pure. A row
/// matches by rom id when it was installed from the pack's server: its own
/// `source_server`, or (for a server install) the live session's server
/// when one is connected. A row with no rom id matches by title and
/// platform.
pub fn installed_on_computer(
    rows: &[InstalledGame],
    game: &PackGame,
    live_server: Option<&str>,
) -> bool {
    let pack_server = pack::server_key(&game.source.server);
    rows.iter().any(|row| match row.rom_id {
        Some(id) if id == game.source.rom_id => {
            if !row.source_server.is_empty() {
                pack::server_key(&row.source_server) == pack_server
            } else {
                live_server.map_or(true, |live| pack::server_key(live) == pack_server)
            }
        }
        Some(_) => false,
        None => {
            row.title.trim().eq_ignore_ascii_case(game.name.trim())
                && row
                    .platform
                    .trim()
                    .eq_ignore_ascii_case(game.platform_name.trim())
        }
    })
}

/// The image-cache keys a pack asset is seeded under. Pure. `None` when
/// `rel` is not one of `game`'s assets. The keys are the absolute server
/// URLs the art had (so the same file serves the drive grid and, after an
/// install, the installed row), else `gridpack://<drive key>/<rel>`.
pub fn asset_cache_urls(drive_key: &str, game: &PackGame, rel: &str) -> Option<Vec<String>> {
    let assets = &game.assets;
    let source = &game.source;
    let server = strip_userinfo(source.server.trim())
        .trim_end_matches('/')
        .to_string();
    let is = |slot: &Option<String>| slot.as_deref() == Some(rel);
    let raw: Vec<&str> = if is(&assets.cover) || is(&assets.cover_small) {
        vec![&source.cover_large_path, &source.cover_small_path]
    } else if is(&assets.fanart) {
        source
            .fanart_urls
            .first()
            .map(String::as_str)
            .into_iter()
            .collect()
    } else if is(&assets.logo) {
        vec![&source.logo_url]
    } else if assets.screenshots.iter().any(|shot| shot == rel) {
        // Written as `screenshot-NN.<ext>`, NN counting the server's list
        // from 1, so a screenshot that failed to download shifts nothing.
        let name = rel.rsplit('/').next().unwrap_or(rel);
        name.strip_prefix("screenshot-")
            .and_then(|rest| rest.split('.').next())
            .and_then(|digits| digits.parse::<usize>().ok())
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| source.screenshot_urls.get(index))
            .map(String::as_str)
            .into_iter()
            .collect()
    } else {
        return None;
    };
    let mut urls: Vec<String> = Vec::new();
    for value in raw {
        let url = resolve_image_url(value, &server);
        if !url.is_empty() && !urls.contains(&url) {
            urls.push(url);
        }
    }
    if urls.is_empty() {
        urls.push(format!("gridpack://{drive_key}/{rel}"));
    }
    Some(urls)
}

/// The pick that backs up `entry` again for another OS. Pure. `None` for an
/// archived manual emulator, which exists for one OS only.
pub fn pick_for_entry(entry: &PackEmulator) -> Option<EmulatorPick> {
    let source_id = entry.source_id.trim();
    if source_id.starts_with(grid_core::library::drive_install::LOCAL_SOURCE_PREFIX) {
        return None;
    }
    let kind = if entry.kind == "compat_tool" {
        EmulatorKind::CompatTool
    } else {
        EmulatorKind::Emulator
    };
    let source = if source_id.starts_with("romm:") {
        EmulatorSource::Server {
            source_id: source_id.to_string(),
        }
    } else {
        EmulatorSource::Catalog {
            source_id: source_id.to_string(),
        }
    };
    Some(EmulatorPick {
        name: entry.name.clone(),
        kind,
        source,
        platforms: entry.platforms.clone(),
    })
}

/// The emulator rows adding `os` to a pack queues: one per emulator the pack
/// already holds for another OS, that has none for `os` yet and can be
/// built for it. Pure.
pub fn emulators_for_added_os(pack: &Pack, os: PackOs) -> Vec<EmulatorItem> {
    let mut items: Vec<EmulatorItem> = Vec::new();
    for entry in &pack.emulators {
        if pack
            .emulators
            .iter()
            .any(|have| have.name == entry.name && have.os == os.arch_tag())
            || items.iter().any(|item| item.pick.name == entry.name)
        {
            continue;
        }
        let Some(pick) = pick_for_entry(entry) else {
            continue;
        };
        if pick.target_os(&[os], PackOs::host()).is_empty() {
            continue;
        }
        items.push(EmulatorItem { pick, os });
    }
    items
}

// --- drive resolution ---------------------------------------------------------------

fn install(state: &AppState) -> Result<&Arc<InstallService>, String> {
    state.install.as_ref().map_err(Clone::clone)
}

/// The connected drive `key`, from the latest scan (scanning once more when
/// it is not in it, so a drive plugged in a moment ago is found).
async fn connected(state: &AppState, app: &AppHandle, key: &str) -> Result<ScannedDrive, String> {
    if let Ok(found) = state.drives.connected(key) {
        return Ok(found);
    }
    state.drives.tick(app, state.install.as_ref().ok()).await;
    state.drives.connected(key)
}

/// The drive's pack, for a command that only reads it.
fn readable_pack(scanned: &ScannedDrive) -> Result<Pack, String> {
    let label = &scanned.drive.label;
    match &scanned.pack {
        Ok(PackState::Ready(pack)) => Ok(pack.clone()),
        Ok(PackState::Absent) => Err(format!("{label} is not set up for GRID")),
        Ok(PackState::NewerFormat(_)) => Err(format!(
            "{label} was set up by a newer GRID; update GRID to use it"
        )),
        Ok(PackState::Unreadable(why)) => Err(format!("GRID can't read {label}: {why}")),
        Err(why) => Err(format!("GRID can't read {label}: {why}")),
    }
}

/// The drive's pack, for a command that writes to it.
fn writable_pack(scanned: &ScannedDrive) -> Result<Pack, String> {
    let pack = readable_pack(scanned)?;
    if scanned.drive.read_only {
        return Err(format!("{} is read-only", scanned.drive.label));
    }
    Ok(pack)
}

/// Whether a live backup row writes to the drive `key`.
fn backup_running(install: &InstallService, key: &str) -> bool {
    install
        .snapshot()
        .entries
        .iter()
        .any(|entry| entry.status.is_live() && entry.drive_id == key && entry.job == "backup")
}

fn joined<T>(result: Result<T, tokio::task::JoinError>, what: &str) -> Result<T, String> {
    result.map_err(|e| format!("{what} did not finish: {e}"))
}

/// Removes `rel`'s parent folders under `gridpack/` while they are empty.
fn prune_empty_parents(mount: &Path, rel: &str) {
    let root = pack::pack_dir(mount);
    let Ok(path) = pack::resolve(mount, rel) else {
        return;
    };
    let mut dir = path.parent().map(Path::to_path_buf);
    while let Some(current) = dir {
        if current == root || !current.starts_with(&root) {
            break;
        }
        if std::fs::remove_dir(&current).is_err() {
            break;
        }
        dir = current.parent().map(Path::to_path_buf);
    }
}

/// The bytes of every file under `dir`, links not followed.
fn folder_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => folder_bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |meta| meta.len()),
            _ => 0,
        })
        .sum()
}

// --- commands ---------------------------------------------------------------------

#[tauri::command]
pub async fn list_drives(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<DrivesSnapshot, String> {
    Ok(state
        .drives
        .current(&app, state.install.as_ref().ok())
        .await)
}

/// Sets up the drive mounted at `mount` for GRID: re-checks the mount
/// against a fresh scan, proves GRID can write there, then writes an empty
/// pack named `name` for the computers `os`.
#[tauri::command]
pub async fn setup_drive(
    state: State<'_, AppState>,
    app: AppHandle,
    mount: String,
    name: String,
    os: Vec<PackOs>,
) -> Result<DriveInfo, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Give the drive a name".to_string());
    }
    if os.is_empty() {
        return Err("Choose at least one computer".to_string());
    }
    let key = grid_core::backup::drives::drive_key(Path::new(mount.trim()));
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    let scanned = state
        .drives
        .connected(&key)
        .map_err(|_| "That drive is no longer connected".to_string())?;
    let label = scanned.drive.label.clone();
    match &scanned.pack {
        Ok(PackState::Absent) => {}
        Ok(PackState::Ready(_)) => return Err(format!("{label} is already set up")),
        _ => return Err(readable_pack(&scanned).err().unwrap_or_default()),
    }
    if scanned.drive.read_only {
        return Err(format!("{label} is read-only"));
    }
    let drive_mount = scanned.drive.mount.clone();
    joined(
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let cant_write = || format!("GRID can't write to {label}");
            let dir = pack::pack_dir(&drive_mount);
            let created = !dir.exists();
            let probe = dir.join(format!(".probe{TMP_SUFFIX}"));
            let probed = std::fs::create_dir_all(&dir)
                .and_then(|()| std::fs::write(&probe, b"grid"))
                .and_then(|()| std::fs::remove_file(&probe));
            if probed.is_err() {
                let _ = std::fs::remove_file(&probe);
                if created {
                    let _ = std::fs::remove_dir(&dir);
                }
                return Err(cant_write());
            }
            pack::write_pack_atomic(&drive_mount, &pack::new_pack(&name, &os))
                .map_err(|_| cant_write())
        })
        .await,
        "setup_drive",
    )??;
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    let scanned = state.drives.connected(&key)?;
    Ok(drive_info(&scanned))
}

#[tauri::command]
pub async fn rename_drive(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    name: String,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Give the drive a name".to_string());
    }
    let scanned = connected(&state, &app, &key).await?;
    writable_pack(&scanned)?;
    let mount = scanned.drive.mount.clone();
    joined(
        tokio::task::spawn_blocking(move || pack::edit_pack(&mount, |pack| pack.name = name)).await,
        "rename_drive",
    )?
    .map_err(err)?;
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    Ok(())
}

/// Changes the computers a pack is for. Removing one deletes that OS's
/// emulator builds (games and firmware stay) and works offline. Adding one
/// queues that OS's build of every emulator already on the drive, which
/// needs a server.
#[tauri::command]
pub async fn set_drive_os(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    os: Vec<PackOs>,
) -> Result<OsChangeOutcome, String> {
    if os.is_empty() {
        return Err("Choose at least one computer".to_string());
    }
    let install = install(&state)?.clone();
    let scanned = connected(&state, &app, &key).await?;
    let pack = writable_pack(&scanned)?;
    let added: Vec<PackOs> = os
        .iter()
        .copied()
        .filter(|one| !pack.os.contains(one))
        .collect();
    let client = if added.is_empty() {
        None
    } else {
        Some(
            state
                .session
                .client()
                .ok_or_else(|| "Adding a computer needs a server".to_string())?,
        )
    };
    if backup_running(&install, &key) {
        return Err(format!(
            "Wait for the backup to {} to finish",
            scanned.drive.label
        ));
    }
    let mount = scanned.drive.mount.clone();
    let wanted = os.clone();
    let removed: Vec<PackEmulator> = joined(
        tokio::task::spawn_blocking(move || -> Result<Vec<PackEmulator>, String> {
            let removed = pack::edit_pack(&mount, |pack| {
                let mut removed = Vec::new();
                for old in pack.os.clone() {
                    if !wanted.contains(&old) {
                        removed.extend(pack.remove_emulators_for_os(old));
                    }
                }
                let mut targets: Vec<PackOs> = Vec::new();
                for one in &wanted {
                    if !targets.contains(one) {
                        targets.push(*one);
                    }
                }
                pack.os = targets;
                removed
            })
            .map_err(err)?;
            // The index no longer names them: delete their files.
            for entry in &removed {
                for rel in std::iter::once(&entry.path).chain(entry.supplementals.iter()) {
                    if let Err(e) = write::remove_rel(&mount, rel) {
                        tracing::warn!("emulator file not removed from the drive: {e}");
                    }
                    prune_empty_parents(&mount, rel);
                }
            }
            Ok(removed)
        })
        .await,
        "set_drive_os",
    )??;
    let mut outcome = OsChangeOutcome {
        removed: removed.len() as u32,
        ..Default::default()
    };
    if let Some(client) = client {
        let items = plan::BackupItems {
            emulators: added
                .iter()
                .flat_map(|one| emulators_for_added_os(&pack, *one))
                .collect(),
            ..Default::default()
        };
        if !items.is_empty() {
            ensure_library_idle(&state)?;
            let mut target = scanned.target();
            target.name = pack.name.clone();
            let queued = install.start_backup(client, target, items).map_err(err)?;
            outcome.batch = queued.batch;
            outcome.queued = queued.emulators;
        }
    }
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    Ok(outcome)
}

/// The bytes "Remove GRID files…" frees: everything under `gridpack/`.
#[tauri::command]
pub async fn remove_grid_files_size(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
) -> Result<u64, String> {
    let scanned = connected(&state, &app, &key).await?;
    let dir = pack::pack_dir(&scanned.drive.mount);
    joined(
        tokio::task::spawn_blocking(move || folder_bytes(&dir)).await,
        "remove_grid_files_size",
    )
}

/// "Remove GRID files…": deletes `<mount>/gridpack` (guarded: exactly that
/// folder, holding `gridpack.json`, not a link) and forgets the drive.
#[tauri::command]
pub async fn remove_grid_files(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
) -> Result<(), String> {
    let scanned = connected(&state, &app, &key).await?;
    if scanned.drive.read_only {
        return Err(format!("{} is read-only", scanned.drive.label));
    }
    if let Ok(install) = install(&state) {
        let busy = install
            .snapshot()
            .entries
            .iter()
            .any(|entry| entry.status.is_live() && entry.drive_id == key);
        if busy {
            return Err(format!(
                "Wait for the downloads using {} to finish",
                scanned.drive.label
            ));
        }
    }
    let mount = scanned.drive.mount.clone();
    joined(
        tokio::task::spawn_blocking(move || write::remove_all_grid_files(&mount)).await,
        "remove_grid_files",
    )?
    .map_err(err)?;
    state.drives.forget(&key).await;
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    Ok(())
}

#[tauri::command]
pub async fn forget_drive(
    state: State<'_, AppState>,
    app: AppHandle,
    mount: String,
) -> Result<(), String> {
    state.drives.forget(&mount).await;
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    Ok(())
}

#[tauri::command]
pub async fn open_drive_folder(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
) -> Result<(), String> {
    let scanned = connected(&state, &app, &key).await?;
    let dir = pack::pack_dir(&scanned.drive.mount);
    let target = if dir.is_dir() {
        dir
    } else {
        scanned.drive.mount.clone()
    };
    app.opener()
        .open_path(target.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| format!("Could not open the folder: {e}"))
}

#[tauri::command]
pub async fn drive_games(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
) -> Result<Vec<DriveGame>, String> {
    let scanned = connected(&state, &app, &key).await?;
    let pack = readable_pack(&scanned)?;
    let rows = match install(&state) {
        Ok(install) => {
            let registry = install.registry();
            joined(
                tokio::task::spawn_blocking(move || registry.all()).await,
                "drive_games",
            )?
            .map_err(err)?
        }
        Err(_) => Vec::new(),
    };
    let live = state.session.client().map(|_| state.session.server_url());
    Ok(pack
        .games
        .iter()
        .map(|game| DriveGame {
            rom_id: game.source.rom_id,
            server: game.source.server.clone(),
            name: game.name.clone(),
            platform: game.platform.clone(),
            platform_name: game.platform_name.clone(),
            size: game.size,
            files_max: game.files.iter().map(|file| file.size).max().unwrap_or(0),
            cover: game.assets.cover.clone().unwrap_or_default(),
            fanart: game.assets.fanart.clone().unwrap_or_default(),
            logo: game.assets.logo.clone().unwrap_or_default(),
            screenshots: game.assets.screenshots.clone(),
            summary: game.summary.clone(),
            region: game.region.clone(),
            languages: game.languages.clone(),
            genres: game.genres.clone(),
            companies: game.companies.clone(),
            franchises: game.franchises.clone(),
            game_modes: game.game_modes.clone(),
            release_date: game.release_date.clone(),
            players: game.players.clone(),
            rating: game.rating.clone(),
            tags: game.tags.clone(),
            revision: game.revision.clone(),
            version: game.version.clone(),
            added: game.added.clone(),
            installed: installed_on_computer(&rows, game, live.as_deref()),
            installed_copy: game.installed.is_some(),
            installed_version: game
                .installed
                .as_ref()
                .map(|copy| copy.version.clone())
                .unwrap_or_default(),
            outdated: game.installed.as_ref().is_some_and(|copy| copy.outdated),
            saves_copied: game.saves.as_ref().is_some_and(|saves| saves.has_copy()),
        })
        .collect())
}

/// Seeds the image cache with `game`'s pack asset `rel` and returns the
/// cached file's path. Blocking.
fn seed_asset(
    cache: &grid_core::images::cache::ImageCache,
    mount: &Path,
    key: &str,
    game: &PackGame,
    rel: &str,
) -> Result<PathBuf, String> {
    let urls = asset_cache_urls(key, game, rel)
        .ok_or_else(|| "That file is not a game's art on the drive".to_string())?;
    if let Some(hit) = cache.cached_path(&urls[0]) {
        return Ok(hit);
    }
    let src = pack::resolve(mount, rel).map_err(err)?;
    let mut first = None;
    for url in &urls {
        let path = cache.seed(url, &src).map_err(err)?;
        first.get_or_insert(path);
    }
    first.ok_or_else(|| "nothing to seed".to_string())
}

/// The cached image file for the pack asset `rel` (a cover, fanart, logo or
/// screenshot a game on the drive names), seeding the cache from the drive.
#[tauri::command]
pub async fn drive_asset(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    rel: String,
) -> Result<String, String> {
    let scanned = connected(&state, &app, &key).await?;
    let pack = readable_pack(&scanned)?;
    let game = art_owner(&pack, &rel)
        .cloned()
        .ok_or_else(|| "That file is not a game's art on the drive".to_string())?;
    let session = state.session.clone();
    let mount = scanned.drive.mount.clone();
    let path = joined(
        tokio::task::spawn_blocking(move || seed_asset(session.cache(), &mount, &key, &game, &rel))
            .await,
        "drive_asset",
    )??;
    Ok(path.to_string_lossy().into_owned())
}

/// The requested games as the planner needs them, and each platform's
/// server id by slug.
///
/// For a FAT32 drive the list is asked for with its file entries, so the
/// 4 GB check reads each game's largest file, not its total size; a game
/// over 4 GB whose largest file the server still did not list (an older
/// server ignoring `with_files`) is looked up by id. Other drives need no
/// file sizes.
async fn plan_games(
    client: &RommClient,
    request: &BackupRequest,
    fat32: bool,
) -> Result<(Vec<PlanGame>, BTreeMap<String, i64>), String> {
    let platforms = client.platforms().await.map_err(err)?;
    let mut found: BTreeMap<i64, PlanGame> = BTreeMap::new();
    let mut ids: BTreeMap<String, i64> = BTreeMap::new();
    for platform_id in &request.platform_ids {
        let Some(platform) = platforms.iter().find(|p| p.id == *platform_id) else {
            continue;
        };
        let games = if fat32 {
            client.games_with_files(*platform_id).await
        } else {
            client.games(*platform_id).await
        }
        .map_err(err)?;
        for game in games {
            if !request.rom_ids.contains(&game.id) {
                continue;
            }
            ids.insert(platform.slug.clone(), platform.id);
            let size = game.fs_size_bytes.max(0) as u64;
            let mut largest_file = game.largest_file_bytes.map(|bytes| bytes.max(0) as u64);
            if fat32 && largest_file.is_none() && size > pack::FAT32_MAX_FILE {
                let detail = client.rom_detail(game.id).await.map_err(err)?;
                largest_file = Some(detail.largest_game_file_bytes() as u64);
            }
            found.insert(
                game.id,
                PlanGame {
                    rom_id: game.id,
                    name: game.name.clone(),
                    platform_label: platform.label().to_string(),
                    platform_name: platform.label().to_string(),
                    platform_slug: platform.slug.clone(),
                    size,
                    largest_file,
                    local: plan::LocalState::NotInstalled,
                },
            );
        }
    }
    let mut games = Vec::new();
    for rom_id in &request.rom_ids {
        if let Some(game) = found.remove(rom_id) {
            if !games.iter().any(|known: &PlanGame| known.rom_id == *rom_id) {
                games.push(game);
            }
            continue;
        }
        if games.iter().any(|known: &PlanGame| known.rom_id == *rom_id) {
            continue;
        }
        let detail = client.rom_detail(*rom_id).await.map_err(err)?;
        let label = platforms
            .iter()
            .find(|p| p.id == detail.platform_id)
            .map(|p| p.label().to_string())
            .unwrap_or_else(|| detail.platform_name.clone());
        ids.insert(detail.platform_slug.clone(), detail.platform_id);
        games.push(PlanGame {
            rom_id: detail.id,
            name: detail.name.clone(),
            platform_label: label.clone(),
            platform_name: label,
            platform_slug: detail.platform_slug.clone(),
            size: detail.filesize_bytes.max(0) as u64,
            largest_file: Some(detail.largest_game_file_bytes() as u64),
            local: plan::LocalState::NotInstalled,
        });
    }
    Ok((games, ids))
}

/// How many detail requests the out-of-date fallback runs at once.
const DETAIL_FETCHES: usize = 4;

/// Whether each requested game can be copied from this computer
/// ([`InstallService::local_states`], once per request), with `outdated`
/// filled for the eligible ones: from the update set when a pass has
/// finished this session, else from each eligible game's detail (4 at a
/// time) and [`game_has_server_update`]. A failure here never fails the
/// plan: the games then simply download.
async fn local_states(
    install: &Arc<InstallService>,
    updates: &UpdateService,
    client: &Arc<RommClient>,
    server: &str,
    rom_ids: &[i64],
    for_windows: bool,
) -> HashMap<i64, LocalState> {
    let assessed = {
        let install = install.clone();
        let rom_ids = rom_ids.to_vec();
        let server = server.to_string();
        tokio::task::spawn_blocking(move || {
            let states = install.local_states(&rom_ids, &server, for_windows)?;
            let rows = install.registry().all()?;
            Ok::<_, grid_core::library::LibraryError>((states, rows))
        })
        .await
    };
    let (mut states, rows) = match assessed {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => {
            tracing::warn!("backup plan: installed games not checked: {e}");
            return HashMap::new();
        }
        Err(e) => {
            tracing::warn!("backup plan: installed games not checked: {e}");
            return HashMap::new();
        }
    };
    let newer = match newer_from_updates(updates, &states) {
        Some(newer) => newer,
        None => newer_from_details(client, &states, &rows).await,
    };
    fill_outdated(&mut states, &rows, &newer);
    states
}

/// The out-of-date fallback: each eligible game's detail, 4 at a time,
/// judged by [`game_has_server_update`]. A detail that fails to load counts
/// as up to date (the backup job checks again and records the truth).
async fn newer_from_details(
    client: &Arc<RommClient>,
    states: &HashMap<i64, LocalState>,
    rows: &[InstalledGame],
) -> HashMap<i64, String> {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(DETAIL_FETCHES));
    let mut tasks = Vec::new();
    for rom_id in eligible_ids(states) {
        let Some(row) = rows.iter().find(|row| row.rom_id == Some(rom_id)).cloned() else {
            continue;
        };
        let client = client.clone();
        let semaphore = semaphore.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.ok()?;
            let detail = match client.rom_detail(rom_id).await {
                Ok(detail) => detail,
                Err(_) => {
                    tracing::debug!("backup plan: rom {rom_id} detail fetch failed");
                    return None;
                }
            };
            let server = ServerVersion {
                platform: &detail.platform_name,
                platform_slug: &detail.platform_slug,
                rom_file_name: &detail.fs_name,
                updated_at: &detail.server_updated_at,
            };
            game_has_server_update(&row, &server).then(|| (rom_id, detail.fs_name.clone()))
        }));
    }
    let mut newer = HashMap::new();
    for task in tasks {
        if let Ok(Some((rom_id, name))) = task.await {
            newer.insert(rom_id, name);
        }
    }
    newer
}

/// Plans a backup of `request` to `scanned`. With `resolve_sizes`, the
/// emulator sizes not known yet are looked up first (dialog only).
async fn make_plan(
    install: &Arc<InstallService>,
    updates: &UpdateService,
    client: Arc<RommClient>,
    scanned: &ScannedDrive,
    request: &BackupRequest,
    options: &PlanOptions,
    resolve_sizes: bool,
) -> Result<Planned, String> {
    let pack = writable_pack(scanned)?;
    let include_firmware = options.include_firmware;
    let include_runtime = options.include_runtime;
    let (mut games, platform_ids) = plan_games(&client, request, scanned.drive.fat32).await?;
    let server = strip_userinfo(client.server_url());
    let rom_ids: Vec<i64> = games.iter().map(|game| game.rom_id).collect();
    let mut states = local_states(
        install,
        updates,
        &client,
        &server,
        &rom_ids,
        pack.os.contains(&PackOs::Windows),
    )
    .await;
    for game in &mut games {
        if let Some(state) = states.remove(&game.rom_id) {
            game.local = state;
        }
    }
    let mut firmware = Vec::new();
    for game in &games {
        if firmware
            .iter()
            .any(|set: &PlanFirmwareSet| set.slug == game.platform_slug)
        {
            continue;
        }
        let Some(&platform_id) = platform_ids.get(&game.platform_slug) else {
            continue;
        };
        match client.firmware(platform_id).await {
            Ok(records) => firmware.push(PlanFirmwareSet {
                platform_id,
                slug: game.platform_slug.clone(),
                platform_name: game.platform_name.clone(),
                files: records.len() as u32,
                bytes: records.iter().map(|record| record.file_size_bytes).sum(),
            }),
            Err(e) => tracing::warn!(platform_id, "backup plan: firmware list failed: {e}"),
        }
    }
    let (emulators, compat) = install.backup_emulator_choices(&games);
    let key = scanned.key();
    let run = |sizes: &HashMap<(String, PackOs), u64>| {
        plan::plan(&PlanInput {
            drive: &key,
            pack: &pack,
            server: &server,
            fat32: scanned.drive.fat32,
            free_bytes: scanned.drive.free_bytes,
            games: &games,
            emulators: &emulators,
            compat_tool: compat.as_ref(),
            firmware: &firmware,
            include_firmware,
            include_runtime,
            host: PackOs::host(),
            sizes,
            choices: &options.choices,
            use_local: options.use_local,
        })
    };
    let planned = run(&install.emulator_sizes());
    let mut to_size: Vec<EmulatorItem> = planned.items.emulators.clone();
    // The runtime line shows its size even while its box is unticked.
    if planned
        .plan
        .runtime
        .as_ref()
        .is_some_and(|line| line.bytes.is_none() && !line.included)
    {
        to_size.push(EmulatorItem {
            pick: grid_core::backup::steam_runtime::runtime_pick(),
            os: PackOs::Linux,
        });
    }
    if !resolve_sizes || to_size.is_empty() {
        return Ok(planned);
    }
    install
        .resolve_emulator_sizes(Some(client.clone()), &to_size)
        .await;
    Ok(run(&install.emulator_sizes()))
}

/// Plans a backup for the dialog. `use_local` ("Copy installed games from
/// this computer") defaults to on; `choices` are the user's picks for
/// out-of-date local games.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn plan_backup(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    request: BackupRequest,
    include_firmware: bool,
    include_runtime: Option<bool>,
    use_local: Option<bool>,
    choices: Option<Vec<SourcePick>>,
) -> Result<BackupPlan, String> {
    let install = install(&state)?.clone();
    let client = state.session.client().ok_or(NEEDS_SERVER)?;
    let scanned = connected(&state, &app, &key).await?;
    let options = PlanOptions {
        include_firmware,
        include_runtime: include_runtime.unwrap_or(false),
        use_local: use_local.unwrap_or(true),
        choices: choices_map(choices),
    };
    let planned = make_plan(
        &install,
        &state.updates,
        client,
        &scanned,
        &request,
        &options,
        true,
    )
    .await?;
    Ok(planned.plan)
}

/// Queues the backup `plan_backup` showed, planned again with the same
/// options. Refused while an out-of-date local game has no pick.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn start_backup(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    request: BackupRequest,
    include_firmware: bool,
    include_runtime: Option<bool>,
    use_local: Option<bool>,
    choices: Option<Vec<SourcePick>>,
) -> Result<BackupQueued, String> {
    ensure_library_idle(&state)?;
    let install = install(&state)?.clone();
    let client = state.session.client().ok_or(NEEDS_SERVER)?;
    let scanned = connected(&state, &app, &key).await?;
    let options = PlanOptions {
        include_firmware,
        include_runtime: include_runtime.unwrap_or(false),
        use_local: use_local.unwrap_or(true),
        choices: choices_map(choices),
    };
    let planned = make_plan(
        &install,
        &state.updates,
        client.clone(),
        &scanned,
        &request,
        &options,
        false,
    )
    .await?;
    if let Some(refusal) = needs_choice_refusal(planned.plan.needs_choice) {
        return Err(refusal);
    }
    if planned.items.is_empty() {
        return Err("Nothing to backup".to_string());
    }
    if !planned.plan.fits {
        return Err(format!(
            "{} doesn't have enough free space",
            scanned.drive.label
        ));
    }
    install
        .start_backup(client, scanned.target(), planned.items)
        .map_err(err)
}

/// Whether this computer has a compat tool a Windows game can run with.
fn has_compat_tool(config: &Config) -> bool {
    if !config.default_compat_tool.trim().is_empty() {
        return true;
    }
    let home = directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .unwrap_or_default();
    discover(
        &home,
        &config.compat_tool_installs,
        std::env::consts::OS,
        &|name| which_on_path(name),
    )
    .iter()
    .any(|tool| tool.kind != "wine")
}

/// Installs `rom_id` (from `server`, else the first game with that id) from
/// the drive. The game's art goes into the image cache first, so it shows
/// offline. When its system has no emulator and the drive has one for this
/// computer, that emulator installs from the drive too and becomes the
/// system's emulator; the drive's firmware for the system follows once it
/// lands (or at once when the system already has its emulator). A Windows
/// game on Linux brings the drive's Proton the same way.
#[tauri::command]
pub async fn install_from_drive(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    rom_id: i64,
    server: Option<String>,
) -> Result<DriveInstallOutcome, String> {
    ensure_library_idle(&state)?;
    let install = install(&state)?.clone();
    let scanned = connected(&state, &app, &key).await?;
    let pack = readable_pack(&scanned)?;
    let game = pack
        .games
        .iter()
        .find(|game| {
            game.source.rom_id == rom_id
                && server.as_deref().map_or(true, |wanted| {
                    pack::server_key(wanted) == pack::server_key(&game.source.server)
                })
        })
        .cloned()
        .ok_or_else(|| format!("That game is not on {}", pack.name))?;
    let mut target = scanned.target();
    target.name = pack.name.clone();

    // Art first: the installed row's covers then hit the cache offline.
    {
        let session = state.session.clone();
        let mount = scanned.drive.mount.clone();
        let key = key.clone();
        let game = game.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let rels: Vec<String> = [&game.assets.cover, &game.assets.fanart, &game.assets.logo]
                .into_iter()
                .flatten()
                .cloned()
                .chain(game.assets.screenshots.iter().cloned())
                .collect();
            for rel in rels {
                if let Err(e) = seed_asset(session.cache(), &mount, &key, &game, &rel) {
                    tracing::debug!("drive art not cached: {e}");
                }
            }
        })
        .await;
    }

    let live = state.session.client().map(|_| state.session.server_url());
    install
        .install_from_drive(target.clone(), &game, live.as_deref())
        .map_err(err)?;

    let mut outcome = DriveInstallOutcome::default();
    let Some(host) = PackOs::host() else {
        return Ok(outcome);
    };
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let profiles = load_profiles();
    let platform = game.platform_name.clone();

    // A Windows game on Linux: the drive's Proton, when there is none here.
    if is_windows_platform(&game.platform, &game.platform_name) {
        if host == PackOs::Linux && !has_compat_tool(&config) {
            let tool = pack.emulators.iter().find(|entry| {
                entry.kind == "compat_tool"
                    && entry.os == host.arch_tag()
                    && !plan::is_umu_entry(&entry.source_id)
            });
            if let Some(tool) = tool.cloned() {
                let source_id = tool.source_id.clone();
                let applied =
                    crate::config_write::modify_config(&Config::default_path(), |config| {
                        Ok(request_compat_default(config, &source_id))
                    })?;
                if applied.is_none() {
                    install
                        .install_emulator_from_drive(target.clone(), &tool)
                        .map_err(err)?;
                    outcome.notices.push(format!(
                        "{} from {} added to download queue",
                        tool.name, pack.name
                    ));
                }
            }
        }
        return Ok(outcome);
    }
    if is_native_platform(&game.platform, &game.platform_name) {
        return Ok(outcome);
    }

    let firmware = pack
        .firmware
        .iter()
        .find(|set| set.platform == game.platform && !set.files.is_empty())
        .cloned();
    let has_emulator =
        crate::firmware_service::default_entry_for_platform(&config, &platform, profiles).is_some();
    if has_emulator {
        if let Some(entry) = firmware {
            state.firmware.spawn_from_pack(
                app.clone(),
                platform,
                scanned.drive.mount.clone(),
                pack.name.clone(),
                entry,
            );
        }
        return Ok(outcome);
    }
    let emulator = pack.emulators.iter().find(|entry| {
        entry.kind == "emulator"
            && entry.os == host.arch_tag()
            && entry.platforms.contains(&game.platform)
    });
    let Some(emulator) = emulator.cloned() else {
        return Ok(outcome);
    };
    let retroarch = grid_core::launch::selection::is_retroarch_name(&emulator.name);
    let pending = PendingDefault {
        emulator: emulator.name.clone(),
        name: platform.clone(),
        slug: game.platform.clone(),
        retroarch,
        server_source: emulator
            .source_id
            .starts_with("romm:")
            .then(|| emulator.source_id.to_lowercase()),
    };
    let label = platform.clone();
    crate::config_write::modify_config(&Config::default_path(), move |config| {
        config.setup.pending.insert(label, pending);
        Ok(())
    })?;
    install
        .install_emulator_from_drive(target, &emulator)
        .map_err(err)?;
    outcome.notices.push(format!(
        "{} from {} added to download queue",
        emulator.name, pack.name
    ));
    if let Some(entry) = firmware {
        state.drives.add_pending_firmware(PendingDriveFirmware {
            platform,
            mount: scanned.drive.mount.clone(),
            pack_name: pack.name.clone(),
            entry,
        });
    }
    Ok(outcome)
}

/// Removes games from the drive: their index entries, then their files.
/// Refused while a backup to the drive runs.
#[tauri::command]
pub async fn remove_from_drive(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    rom_ids: Vec<i64>,
    server: Option<String>,
) -> Result<u32, String> {
    let scanned = connected(&state, &app, &key).await?;
    writable_pack(&scanned)?;
    if let Ok(install) = install(&state) {
        if backup_running(install, &key) {
            return Err(format!(
                "Wait for the backup to {} to finish",
                scanned.drive.label
            ));
        }
    }
    let mount = scanned.drive.mount.clone();
    let removed = joined(
        tokio::task::spawn_blocking(move || -> Result<u32, String> {
            let wanted = server.as_deref().map(pack::server_key);
            let gone = pack::edit_pack(&mount, |pack| {
                let (gone, kept): (Vec<PackGame>, Vec<PackGame>) = std::mem::take(&mut pack.games)
                    .into_iter()
                    .partition(|game| {
                        rom_ids.contains(&game.source.rom_id)
                            && wanted
                                .as_ref()
                                .map_or(true, |key| *key == pack::server_key(&game.source.server))
                    });
                pack.games = kept;
                gone
            })
            .map_err(err)?;
            for game in &gone {
                for rel in game.paths() {
                    if let Err(e) = write::remove_rel(&mount, rel) {
                        tracing::warn!("game file not removed from the drive: {e}");
                    }
                    prune_empty_parents(&mount, rel);
                }
            }
            Ok(gone.len() as u32)
        })
        .await,
        "remove_from_drive",
    )??;
    state.drives.tick(&app, state.install.as_ref().ok()).await;
    Ok(removed)
}

#[tauri::command]
pub async fn get_backup_settings() -> Result<BackupSettingsDto, String> {
    joined(
        tokio::task::spawn_blocking(|| {
            Config::load(&Config::default_path())
                .map(|config| BackupSettingsDto {
                    include_firmware: config.backup.include_firmware,
                })
                .map_err(err)
        })
        .await,
        "get_backup_settings",
    )?
}

#[tauri::command]
pub async fn set_backup_include_firmware(value: bool) -> Result<(), String> {
    joined(
        tokio::task::spawn_blocking(move || {
            crate::config_write::modify_config(&Config::default_path(), |config| {
                config.backup.include_firmware = value;
                Ok(())
            })
        })
        .await,
        "set_backup_include_firmware",
    )?
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::backup::pack::{new_pack, PackAssets};

    fn game(rom_id: i64, server: &str) -> PackGame {
        let mut game: PackGame = serde_json::from_value(serde_json::json!({
            "platform": "psx", "platform_name": "Sony PlayStation",
            "source": { "server": server, "rom_id": rom_id,
                "cover_large_path": "/assets/romm/resources/roms/1/cover/big.png",
                "cover_small_path": "/assets/romm/resources/roms/1/cover/small.png",
                "screenshot_urls": ["https://romm.home.lan/s/1.png", "https://romm.home.lan/s/2.png"],
                "logo_url": "" },
            "name": "Velvet Static", "size": 10, "files": [], "added": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        game.assets = PackAssets {
            cover: Some("games/psx/Velvet Static/cover.png".into()),
            cover_small: None,
            fanart: None,
            logo: Some("games/psx/Velvet Static/logo.png".into()),
            screenshots: vec!["games/psx/Velvet Static/screenshot-02.png".into()],
        };
        game
    }

    fn row(rom_id: Option<i64>, source: &str) -> InstalledGame {
        InstalledGame {
            title: "Velvet Static".into(),
            platform: "Sony PlayStation".into(),
            rom_id,
            source_server: source.into(),
            ..Default::default()
        }
    }

    #[test]
    fn installed_matches_by_rom_id_on_the_same_server() {
        let game = game(7, "https://romm.home.lan");
        assert!(installed_on_computer(
            &[row(Some(7), "https://ROMM.home.lan/")],
            &game,
            None
        ));
        assert!(!installed_on_computer(
            &[row(Some(7), "https://other.lan")],
            &game,
            None
        ));
        // A server install: its server is the live session's.
        assert!(installed_on_computer(&[row(Some(7), "")], &game, None));
        assert!(installed_on_computer(
            &[row(Some(7), "")],
            &game,
            Some("https://romm.home.lan")
        ));
        assert!(!installed_on_computer(
            &[row(Some(7), "")],
            &game,
            Some("https://other.lan")
        ));
        assert!(!installed_on_computer(&[row(Some(8), "")], &game, None));
        // An unlinked row matches by title and platform.
        assert!(installed_on_computer(&[row(None, "")], &game, None));
    }

    #[test]
    fn asset_urls_are_the_absolute_server_urls_else_a_gridpack_key() {
        let game = game(7, "https://user:pw@romm.home.lan/");
        let cover = asset_cache_urls("E:", &game, "games/psx/Velvet Static/cover.png").unwrap();
        assert_eq!(
            cover,
            vec![
                "https://romm.home.lan/assets/romm/resources/roms/1/cover/big.png".to_string(),
                "https://romm.home.lan/assets/romm/resources/roms/1/cover/small.png".to_string(),
            ]
        );
        // screenshot-02 is the server's second screenshot.
        let shot =
            asset_cache_urls("E:", &game, "games/psx/Velvet Static/screenshot-02.png").unwrap();
        assert_eq!(shot, vec!["https://romm.home.lan/s/2.png".to_string()]);
        // No server URL for the logo: a key of its own.
        let logo = asset_cache_urls("E:", &game, "games/psx/Velvet Static/logo.png").unwrap();
        assert_eq!(
            logo,
            vec!["gridpack://E:/games/psx/Velvet Static/logo.png".to_string()]
        );
        // Not an asset of this game: refused.
        assert!(asset_cache_urls("E:", &game, "gridpack.json").is_none());
    }

    #[test]
    fn choices_arrive_as_picks_and_the_last_pick_wins() {
        let picks: Vec<SourcePick> = serde_json::from_value(serde_json::json!([
            { "rom_id": 7, "source": "local" },
            { "rom_id": 8, "source": "download" },
            { "rom_id": 7, "source": "download" },
        ]))
        .unwrap();
        let map = choices_map(Some(picks));
        assert_eq!(map.len(), 2);
        assert_eq!(map[&7], SourceChoice::Download);
        assert_eq!(map[&8], SourceChoice::Download);
        assert!(choices_map(None).is_empty());
        assert!(serde_json::from_value::<SourcePick>(
            serde_json::json!({ "rom_id": 1, "source": "elsewhere" })
        )
        .is_err());
    }

    #[test]
    fn a_plan_waiting_on_picks_is_refused() {
        assert_eq!(needs_choice_refusal(0), None);
        assert_eq!(
            needs_choice_refusal(1).unwrap(),
            "Choose which copy to backup for 1 game"
        );
        assert_eq!(
            needs_choice_refusal(3).unwrap(),
            "Choose which copy to backup for 3 games"
        );
    }

    fn eligible() -> LocalState {
        LocalState::Eligible {
            layout: pack::InstalledLayout::Extracted,
            archive_bytes: 4096,
            outdated: None,
        }
    }

    fn installed_row(rom_id: i64, rom_file_name: &str) -> InstalledGame {
        InstalledGame {
            rom_id: Some(rom_id),
            rom_file_name: rom_file_name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn outdated_comes_from_the_update_set_once_a_pass_finished() {
        let mut states: HashMap<i64, LocalState> = HashMap::from([
            (1, eligible()),
            (2, eligible()),
            (3, LocalState::NotInstalled),
            (
                4,
                LocalState::Ineligible {
                    reason: "Its files weren't found on this computer".into(),
                },
            ),
            (5, eligible()),
        ]);
        let updates = UpdateService::new();
        assert!(
            newer_from_updates(&updates, &states).is_none(),
            "no pass yet: the caller falls back to the details"
        );
        updates.commit_for_test(HashMap::from([
            (1, "Velvet Static (v1.3).zip".to_string()),
            (3, "Not Here (v2).zip".to_string()),
            (4, "Missing (v2).zip".to_string()),
            (5, "Untagged.zip".to_string()),
        ]));
        let newer = newer_from_updates(&updates, &states).unwrap();
        assert_eq!(
            newer
                .keys()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            [1, 5].into(),
            "only eligible games"
        );

        let rows = [
            installed_row(1, "Velvet Static (v1.2).zip"),
            installed_row(2, "Moss (v1.0).zip"),
            installed_row(5, "Untagged.zip"),
        ];
        fill_outdated(&mut states, &rows, &newer);
        let outdated = |id: i64| match &states[&id] {
            LocalState::Eligible { outdated, .. } => outdated.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            outdated(1),
            Some(Outdated {
                installed_version: "v1.2".into(),
                server_version: "v1.3".into(),
            })
        );
        assert_eq!(outdated(2), None, "no update known");
        assert_eq!(
            outdated(5),
            Some(Outdated {
                installed_version: String::new(),
                server_version: String::new(),
            }),
            "no version tags: an older copy"
        );
        assert_eq!(states[&3], LocalState::NotInstalled);
    }

    #[test]
    fn only_art_is_served_from_the_drive() {
        let mut copy = game(7, "https://romm.home.lan");
        copy.installed = Some(
            serde_json::from_value(serde_json::json!({
                "archive": "tar", "layout": "extracted", "name": "Velvet Static",
                "parts": [{ "path": "games/psx/Velvet Static/installed.tar.001",
                            "size": 10, "md5": "00000000000000000000000000000000" }],
                "tree_bytes": 10, "tree_files": 1,
            }))
            .unwrap(),
        );
        let mut file = game(8, "https://romm.home.lan");
        file.files = vec![serde_json::from_value(serde_json::json!({
            "path": "games/psx/Other.chd", "size": 10, "server_file_id": 1, "name": "Other.chd"
        }))
        .unwrap()];
        let mut kit = new_pack("Couch kit", &[PackOs::Linux]);
        kit.games = vec![copy, file];

        let owner = art_owner(&kit, "games/psx/Velvet Static/cover.png").unwrap();
        assert_eq!(owner.source.rom_id, 7);
        assert!(art_owner(&kit, "games/psx/Velvet Static/screenshot-02.png").is_some());
        assert!(
            art_owner(&kit, "games/psx/Velvet Static/installed.tar.001").is_none(),
            "an archive part is not art"
        );
        assert!(
            art_owner(&kit, "games/psx/Other.chd").is_none(),
            "a game file is not art"
        );
        assert!(kit.games[0]
            .paths()
            .any(|path| path == "games/psx/Velvet Static/installed.tar.001"));
    }

    fn emulator(name: &str, source_id: &str, kind: &str, os: PackOs) -> PackEmulator {
        PackEmulator {
            name: name.into(),
            kind: kind.into(),
            source_id: source_id.into(),
            version: "v1".into(),
            program: String::new(),
            args: String::new(),
            os: os.arch_tag().into(),
            platforms: vec!["psx".into()],
            path: format!("emulators/{name}/v1/{}/x.zip", os.arch_tag()),
            supplementals: Vec::new(),
            asset_name: "x.zip".into(),
            size: 1,
            extra: Default::default(),
        }
    }

    #[test]
    fn adding_an_os_queues_each_emulator_it_can_build_once() {
        let mut kit = new_pack("Couch kit", &[PackOs::Linux]);
        kit.emulators = vec![
            emulator(
                "DuckStation",
                "stenzek/duckstation",
                "emulator",
                PackOs::Linux,
            ),
            emulator("PCSX2", "romm:pcsx2/pcsx2", "emulator", PackOs::Linux),
            emulator("PCSX2", "romm:pcsx2/pcsx2", "emulator", PackOs::Windows),
            emulator("GE-Proton", "ge/proton", "compat_tool", PackOs::Linux),
            emulator("PICO-8", "local:PICO-8", "emulator", PackOs::Linux),
        ];
        let items = emulators_for_added_os(&kit, PackOs::Windows);
        let names: Vec<&str> = items.iter().map(|item| item.pick.name.as_str()).collect();
        assert_eq!(names, vec!["DuckStation"]);
        assert!(items.iter().all(|item| item.os == PackOs::Windows));
        assert!(matches!(
            items[0].pick.source,
            EmulatorSource::Catalog { ref source_id } if source_id == "stenzek/duckstation"
        ));

        let server = pick_for_entry(&kit.emulators[1]).unwrap();
        assert!(matches!(server.source, EmulatorSource::Server { .. }));
        assert_eq!(
            pick_for_entry(&kit.emulators[3]).unwrap().kind,
            EmulatorKind::CompatTool
        );
        assert!(pick_for_entry(&kit.emulators[4]).is_none());
    }
}
