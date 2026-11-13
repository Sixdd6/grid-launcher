//! GOG games on backup drives: the bodies of the five store backup
//! commands registered in `commands/stores.rs` (`plan_store_backup`,
//! `start_store_backup`, `drive_store_games`, `install_store_from_drive`,
//! `remove_store_from_drive`). Thin: grid-core's `backup::store`,
//! `backup::plan::plan_store`, `library::backup_job` and
//! `library::drive_restore` own every rule. Each names a drive by its key
//! and resolves it against the latest scan, like `commands/backups.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use grid_core::backup::pack::{self, PackStoreGame, StoreLayout};
use grid_core::backup::plan::{BackupQueued, StoreBackupPlan as CorePlan, StorePlanned};
use grid_core::backup::write;
use grid_core::images::cache::ImageCache;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::stores::gog::meta::GogPlatform;
use grid_core::stores::GameKey;
use tauri::AppHandle;

use crate::commands::backups::{
    self, backup_running, connected, joined, prune_empty_parents, readable_pack, writable_pack,
};
use crate::commands::stores::{DriveStoreGame, StoreBackupPlan, StoreBackupRow};
use crate::commands::{ensure_library_idle, err, AppState};
use crate::drive_service::ScannedDrive;

/// The game keys of a request; one that isn't a key is refused.
fn parse_keys(keys: &[String]) -> Result<Vec<GameKey>, String> {
    keys.iter()
        .map(|key| key.parse::<GameKey>().map_err(|e| e.to_string()))
        .collect()
}

/// Tells the install service which store games are running (the launch
/// service's sessions), so a running GOG game is not copied. Installed
/// again on each call: cheap, and it holds the launch service weakly.
fn watch_running(state: &AppState, install: &InstallService) {
    if let Ok(launch) = &state.launch {
        let launch = Arc::downgrade(launch);
        install.set_store_running_check(Some(Arc::new(move |key: &GameKey| {
            launch.upgrade().is_some_and(|launch| {
                launch
                    .snapshot()
                    .sessions
                    .iter()
                    .any(|session| &session.key == key)
            })
        })));
    }
}

/// A store row's cover on this computer: a local file, or the image
/// cache's copy of its URL.
fn cover_of(cache: &ImageCache, row: &InstalledGame) -> Option<PathBuf> {
    [&row.cover_large_path, &row.cover_small_path]
        .into_iter()
        .map(|raw| raw.trim())
        .filter(|raw| !raw.is_empty())
        .find_map(|raw| {
            let path = Path::new(raw);
            if path.is_absolute() && path.is_file() {
                Some(path.to_path_buf())
            } else {
                cache.cached_path(raw)
            }
        })
}

/// The store backup plan for one drive, judged now.
async fn store_plan(
    state: &AppState,
    install: &Arc<InstallService>,
    scanned: &ScannedDrive,
    keys: Vec<GameKey>,
) -> Result<StorePlanned, String> {
    let pack = writable_pack(scanned)?;
    watch_running(state, install);
    let install = install.clone();
    let session = state.session.clone();
    let target = scanned.target();
    let free = scanned.drive.free_bytes;
    joined(
        tokio::task::spawn_blocking(move || {
            install.plan_store_backup(&target, &pack, &keys, free, &|row| {
                cover_of(session.cache(), row)
            })
        })
        .await,
        "plan_store_backup",
    )
}

fn layout_name(layout: StoreLayout) -> &'static str {
    match layout {
        StoreLayout::Installer => "installer",
        StoreLayout::Installed => "installed",
    }
}

/// The core plan as the dialog reads it. Pure.
pub fn plan_dto(plan: CorePlan) -> StoreBackupPlan {
    StoreBackupPlan {
        drive: plan.drive,
        rows: plan
            .rows
            .into_iter()
            .map(|row| StoreBackupRow {
                key: row.key,
                name: row.name,
                layout: layout_name(row.layout).to_string(),
                detail: row.detail,
                bytes: row.bytes,
                windows_prefix_note: row.windows_prefix_note,
                already: row.already,
                refused: row.refused,
            })
            .collect(),
        bytes_to_write: plan.bytes_to_write,
        free_bytes: plan.free_bytes,
        fits: plan.fits,
    }
}

/// A store game on a drive as the drive grid lists it. Pure.
pub fn drive_store_game(game: &PackStoreGame, installed: bool) -> DriveStoreGame {
    let saves = game.saves.as_ref().map_or(0, |saves| saves.parts_bytes());
    DriveStoreGame {
        key: game.key().to_string(),
        name: game.name.clone(),
        platform: match game.platform {
            GogPlatform::Linux => "linux",
            GogPlatform::Windows => "windows",
        }
        .to_string(),
        layout: layout_name(game.layout).to_string(),
        version: game.version.clone(),
        size: game.size.saturating_add(saves),
        cover: game.assets.cover.clone().unwrap_or_default(),
        installed,
        added: game.added.clone(),
        summary: game.summary.clone(),
    }
}

/// `plan_store_backup`: the dialog's plan for the drive `key`.
pub async fn plan(
    state: &AppState,
    app: &AppHandle,
    key: &str,
    game_keys: &[String],
) -> Result<StoreBackupPlan, String> {
    let install = backups::install(state)?.clone();
    let keys = parse_keys(game_keys)?;
    let scanned = connected(state, app, key).await?;
    let planned = store_plan(state, &install, &scanned, keys).await?;
    Ok(plan_dto(planned.plan))
}

/// `start_store_backup`: queues the backup the plan showed, planned again
/// now.
pub async fn start(
    state: &AppState,
    app: &AppHandle,
    key: &str,
    game_keys: &[String],
) -> Result<BackupQueued, String> {
    ensure_library_idle(state)?;
    let install = backups::install(state)?.clone();
    let keys = parse_keys(game_keys)?;
    let scanned = connected(state, app, key).await?;
    let planned = store_plan(state, &install, &scanned, keys).await?;
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
        .start_store_backup(
            scanned.target(),
            planned.items,
            Some(scanned.drive.free_bytes),
        )
        .map_err(err)
}

/// `drive_store_games`: the store games on the drive `key`, in the pack's
/// order.
pub async fn list(
    state: &AppState,
    app: &AppHandle,
    key: &str,
) -> Result<Vec<DriveStoreGame>, String> {
    let scanned = connected(state, app, key).await?;
    let pack = readable_pack(&scanned)?;
    let installed: Vec<GameKey> = match backups::install(state) {
        Ok(install) => {
            let registry = install.registry();
            joined(
                tokio::task::spawn_blocking(move || registry.all()).await,
                "drive_store_games",
            )?
            .map_err(err)?
            .iter()
            .filter(|row| row.is_store())
            .filter_map(InstalledGame::key)
            .collect()
        }
        Err(_) => Vec::new(),
    };
    Ok(pack
        .store_games
        .iter()
        .map(|game| drive_store_game(game, installed.contains(&game.key())))
        .collect())
}

/// `install_store_from_drive`: restores `game_key` from the drive `key` as
/// a `store_restore` row in Downloads.
pub async fn install(
    state: &AppState,
    app: &AppHandle,
    key: &str,
    game_key: &str,
) -> Result<(), String> {
    ensure_library_idle(state)?;
    let install = backups::install(state)?.clone();
    let wanted: GameKey = game_key.parse().map_err(err)?;
    let scanned = connected(state, app, key).await?;
    let pack = readable_pack(&scanned)?;
    let game = pack
        .find_store_game(&wanted)
        .cloned()
        .ok_or_else(|| format!("{} doesn't hold that game", scanned.drive.label))?;
    let target = scanned.target();
    joined(
        tokio::task::spawn_blocking(move || install.install_store_from_drive(&target, &game)).await,
        "install_store_from_drive",
    )?
    .map(|_| ())
    .map_err(err)
}

/// `remove_store_from_drive`: their index entries, then their files.
/// Refused while a backup to the drive runs. Answers how many were
/// removed.
pub async fn remove(
    state: &AppState,
    app: &AppHandle,
    key: &str,
    game_keys: &[String],
) -> Result<u32, String> {
    let keys = parse_keys(game_keys)?;
    let scanned = connected(state, app, key).await?;
    writable_pack(&scanned)?;
    if let Ok(install) = backups::install(state) {
        if backup_running(install, key) {
            return Err(format!(
                "Wait for the backup to {} to finish",
                scanned.drive.label
            ));
        }
    }
    let mount = scanned.drive.mount.clone();
    let removed = joined(
        tokio::task::spawn_blocking(move || -> Result<u32, String> {
            let gone = pack::edit_pack(&mount, |pack| {
                keys.iter()
                    .filter_map(|key| pack.remove_store_game(key))
                    .collect::<Vec<_>>()
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
        "remove_store_from_drive",
    )??;
    state.drives.tick(app, state.install.as_ref().ok()).await;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::backup::pack::{PackAssets, PackSaves};
    use grid_core::stores::StoreKind;

    #[test]
    fn a_drive_game_lists_with_its_saves_and_cover() {
        let game = PackStoreGame {
            source: StoreKind::Gog,
            store_id: "1207664643".into(),
            name: "Ashen Tide".into(),
            platform: GogPlatform::Windows,
            layout: StoreLayout::Installed,
            version: "1.1.0".into(),
            summary: "A tide.".into(),
            companies: Vec::new(),
            genres: Vec::new(),
            release_date: String::new(),
            size: 100,
            installer: Vec::new(),
            dlc_installers: Vec::new(),
            installed: None,
            saves: Some(PackSaves {
                parts: vec![grid_core::backup::pack::PackPart {
                    path: "stores/gog/Ashen Tide-1207664643/saves/saves.tar.001".into(),
                    size: 20,
                    md5: String::new(),
                }],
                ..PackSaves::default()
            }),
            assets: PackAssets {
                cover: Some("stores/gog/Ashen Tide-1207664643/assets/cover.png".into()),
                ..PackAssets::default()
            },
            meta: serde_json::Value::Null,
            added: "2026-11-13T00:00:00Z".into(),
            extra: Default::default(),
        };
        let listed = drive_store_game(&game, true);
        assert_eq!(listed.key, "gog:1207664643");
        assert_eq!(listed.platform, "windows");
        assert_eq!(listed.layout, "installed");
        assert_eq!(listed.size, 120);
        assert_eq!(
            listed.cover,
            "stores/gog/Ashen Tide-1207664643/assets/cover.png"
        );
        assert!(listed.installed);
    }
}
