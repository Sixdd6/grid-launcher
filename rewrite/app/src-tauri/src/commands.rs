pub mod cloud;
pub mod specials;
pub mod updates;

use crate::config_write::modify_config;
use crate::images::ImageService;
use grid_core::autoconfig::{self, entry as autoconfig_entry, RaCredentials};
use grid_core::config::{Config, EmulatorEntry};
use grid_core::images::urls::{filter_to_server_host, resolve_image_url};
use grid_core::launch::catalog::{catalog_entries, mark_installed, CatalogEntry};
use grid_core::launch::profiles::{
    load_profiles, profile_for_entry, visible_profiles, EmulatorProfile,
};
use grid_core::launch::selection::{
    compatible_emulator_names_for_platform, emulator_entry_by_name, emulator_supports_platform,
    installed_core_resolver,
};
use grid_core::launch::{GameSession, LaunchService, SessionsSnapshot};
use grid_core::library::queue::DownloadsSnapshot;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::romm::{GameSummary, Platform, RomDetail};
use grid_core::secrets::RaTokenStore;
use grid_core::session::{RestoreOutcome, SessionManager, SessionState};
use secrecy::SecretString;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub session: Arc<SessionManager>,
    pub install: Result<Arc<InstallService>, String>,
    pub launch: Result<Arc<LaunchService>, String>,
    /// The RetroAchievements token's keyring slot — a SECOND, independent
    /// item from the RomM credential `state.session` holds (secrets.rs).
    pub ra_store: Arc<dyn RaTokenStore>,
    /// Cloud save/state sync: the emulator-entry/sync-dir caches and the
    /// D5 auto-upload pool. See `cloud_service.rs` and `commands/cloud.rs`.
    pub cloud: Arc<crate::cloud_service::CloudService>,
    /// Cover/screenshot pipeline glue: the startup sweep, the one-at-a-time
    /// replenish job, and the post-install prefetch. See `images.rs`.
    pub images: Arc<ImageService>,
    /// Background firmware triggers and their one-job-per-emulator-directory
    /// guard. See `firmware_service.rs`.
    pub firmware: Arc<crate::firmware_service::FirmwareService>,
    /// The transient set of games with a newer server version, and the
    /// triggers that recompute it. See `update_service.rs`.
    pub updates: Arc<crate::update_service::UpdateService>,
    /// The launcher's own self-update notice, pullable because the startup
    /// check can emit before the webview listens. See `app_update.rs`.
    pub app_update: Arc<crate::app_update::AppUpdateState>,
}

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    // RommError/SessionError Display are credential-free by construction.
    e.to_string()
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    server_url: String,
    username: String,
    secret: String,
    use_token: bool,
) -> Result<SessionState, String> {
    // Wrap immediately; the plain String is dropped at the end of this scope.
    let secret = SecretString::from(secret);
    let result = state
        .session
        .connect(server_url, username, secret, use_token)
        .await
        .map_err(err)?;
    if let Ok(install) = state.install.as_ref() {
        state
            .images
            .spawn_replenish(app.clone(), state.session.clone(), install.clone());
        state
            .updates
            .spawn_refresh(app, state.session.clone(), install.clone());
    }
    Ok(result)
}

#[tauri::command]
pub async fn restore_session(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<RestoreOutcome, String> {
    let outcome = state.session.restore().await.map_err(err)?;
    if matches!(outcome, RestoreOutcome::Connected { .. }) {
        if let Ok(install) = state.install.as_ref() {
            state
                .images
                .spawn_replenish(app.clone(), state.session.clone(), install.clone());
            state
                .updates
                .spawn_refresh(app, state.session.clone(), install.clone());
        }
    }
    Ok(outcome)
}

#[tauri::command]
pub async fn retry_connect(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SessionState, String> {
    let result = state.session.retry().await.map_err(err)?;
    if let Ok(install) = state.install.as_ref() {
        state
            .images
            .spawn_replenish(app.clone(), state.session.clone(), install.clone());
        state
            .updates
            .spawn_refresh(app, state.session.clone(), install.clone());
    }
    Ok(result)
}

#[tauri::command]
pub fn disconnect(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    state.session.disconnect().map_err(err)?;
    // The update set describes a server that is no longer connected.
    state.updates.clear(&app);
    Ok(())
}

/// Whether a `list_platforms` response should re-run the defaults backfill:
/// only when the assignable platform list is non-empty. An empty list means
/// no session, or a server response `assignable_platforms` filtered down to
/// nothing — either way the backfill would do no useful work, matching
/// `sync_new_emulator`'s own no-op on an empty platform list.
fn should_backfill_on_platform_list(assignable_platforms: &[String]) -> bool {
    !assignable_platforms.is_empty()
}

#[tauri::command]
pub async fn list_platforms(state: State<'_, AppState>) -> Result<Vec<Platform>, String> {
    let client = state.session.client().ok_or("not connected")?;
    let platforms = client.platforms().await.map_err(err)?;
    // grid-core holds no session, so this fetch is the only way it learns the
    // platform list the autoconfig defaults assignment writes against.
    if let Ok(install) = state.install.as_ref() {
        let names: Vec<String> = platforms.iter().map(|p| p.name.clone()).collect();
        let assignable = autoconfig_entry::assignable_platforms(&names);
        install.set_known_platforms(assignable.clone());
        // The firmware triggers need the platform *id*, not just the name,
        // and grid-core holds no session to fetch it with. Recorded from the
        // FULL platform list (not the assignable subset): a platform the
        // autoconfig defaults skip can still have server firmware.
        install.set_platform_ids(platforms.iter().map(|p| (p.name.clone(), p.id)).collect());
        // Slug-first RetroArch core resolution (D-RC-2) needs the server's
        // own slug for each platform; like the ids above, this is recorded
        // from the FULL list, not the assignable subset.
        install.set_platform_slugs(
            platforms
                .iter()
                .map(|p| (p.name.clone(), p.slug.clone()))
                .collect(),
        );

        // Self-heal for the gap D3's own trigger policy leaves: an emulator
        // installed or added before the FIRST successful platform fetch got
        // no platform/core defaults at that time, and nothing else re-runs
        // the backfill until the next add/install. Now that a platform list
        // has arrived, re-run it across every entry. Cheap and idempotent on
        // every later call too — `backfill_all_defaults` no-ops once nothing
        // is missing. Read out of `install` before the blocking hop: `State`
        // is not `Send`.
        if should_backfill_on_platform_list(&assignable) {
            let config_path = Config::default_path();
            let ra = install.ra_credentials();
            let profiles = load_profiles();
            let slugs = install.platform_slugs();
            let outcome = tokio::task::spawn_blocking(move || {
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &assignable,
                    platform_slugs: &slugs,
                    ps3_library_path: String::new(),
                    ra,
                    profiles,
                };
                autoconfig::backfill_all_defaults(&ctx)
            })
            .await;
            // Never fails the response — a load/save error or a panicked
            // task is logged only, exactly like `save_emulator`'s own
            // autoconfig warnings.
            match outcome {
                Ok(Ok(_changed)) => {}
                Ok(Err(e)) => tracing::warn!("emulator defaults backfill: {e}"),
                Err(e) => tracing::warn!("emulator defaults backfill did not finish: {e}"),
            }
        }
    }
    Ok(platforms)
}

#[tauri::command]
pub async fn list_games(
    state: State<'_, AppState>,
    platform_id: i64,
) -> Result<Vec<GameSummary>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.games(platform_id).await.map_err(err)
}

#[tauri::command]
pub async fn get_rom_detail(state: State<'_, AppState>, rom_id: i64) -> Result<RomDetail, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.rom_detail(rom_id).await.map_err(err)
}

#[tauri::command]
pub async fn ensure_image(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path = state
        .session
        .cache()
        .ensure(client.as_deref(), &resolved)
        .await
        .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn install_game(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    let client = state.session.client().ok_or("not connected")?;
    install.install(client, rom_id).await.map_err(err)
}

#[tauri::command]
pub fn cancel_install(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.cancel(entry_id);
    Ok(())
}

#[tauri::command]
pub async fn retry_install(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    // `None` when no session is connected: an emulator retry does not need a
    // RomM client, and `retry` reports "not connected" itself for a game row.
    install
        .retry(state.session.client(), entry_id)
        .await
        .map_err(err)
}

#[tauri::command]
pub fn dismiss_download(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.dismiss(entry_id);
    Ok(())
}

#[tauri::command]
pub async fn uninstall_game(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    rom_id: i64,
) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let install_for_updates = install.clone();
    tokio::task::spawn_blocking(move || install.uninstall(rom_id).map_err(err))
        .await
        .map_err(|e| format!("uninstall did not finish: {e}"))??;
    // The uninstalled row can no longer carry an update.
    state
        .updates
        .spawn_refresh(app, state.session.clone(), install_for_updates);
    Ok(())
}

#[tauri::command]
pub fn list_downloads(state: State<'_, AppState>) -> Result<DownloadsSnapshot, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    Ok(install.snapshot())
}

#[tauri::command]
pub async fn list_installed(state: State<'_, AppState>) -> Result<Vec<InstalledGame>, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || install.installed().map_err(err))
        .await
        .map_err(|e| format!("list_installed did not finish: {e}"))?
}

#[tauri::command]
pub async fn get_library_path() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.library_path)
    })
    .await
    .map_err(|e| format!("get_library_path did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_library_path(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            config.library_path = path;
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_library_path did not finish: {e}"))?
}

// --- launch/emulator types ---------------------------------------------------

/// An autoprofile, trimmed to what the frontend needs (task-7-brief.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileSummary {
    pub name: String,
    pub args: String,
}

/// Config fields the launch/emulator UI needs together (task-7-brief.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LaunchDefaults {
    pub default_emulators: BTreeMap<String, String>,
    pub retroarch_cores: BTreeMap<String, String>,
    pub launch_args: String,
}

// --- launch/session commands -------------------------------------------------

/// The installed row for `rom_id`, off the blocking pool — shared by the
/// two cloud auto-triggers `launch_game` runs around the actual launch.
async fn installed_game_by_rom_id(
    install: &Arc<InstallService>,
    rom_id: i64,
) -> Result<Option<InstalledGame>, String> {
    let install = install.clone();
    let games = tokio::task::spawn_blocking(move || install.installed().map_err(err))
        .await
        .map_err(|e| format!("registry lookup did not finish: {e}"))??;
    Ok(games.into_iter().find(|g| g.rom_id == Some(rom_id)))
}

#[tauri::command]
pub async fn launch_game(state: State<'_, AppState>, rom_id: i64) -> Result<GameSession, String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let config_path = Config::default_path();

    // Auto-restore before launch (parity: details_view_mixin.py:1497,
    // `_auto_sync_before_launch`), BEFORE the process spawns. A lookup or
    // restore failure never blocks the launch — errors are swallowed here
    // (and only debug-logged inside `auto_restore_before_launch`).
    if let Ok(Some(installed_game)) = installed_game_by_rom_id(&install, rom_id).await {
        // Firmware top-up before the process spawns (install_mixin.py:528's
        // call site, re-run at launch so a game installed before its
        // emulator existed still gets its BIOS). Fire-and-forget: it spawns
        // its own task, returns immediately, and can never fail the launch.
        state.firmware.spawn_for_game(
            state.session.clone(),
            install.clone(),
            installed_game.clone(),
            crate::firmware_service::FirmwareTrigger::Launch,
        );
        state
            .cloud
            .auto_restore_before_launch(
                &state.session,
                install.clone(),
                launch.clone(),
                &config_path,
                &installed_game,
            )
            .await;
    }

    let session = launch.launch(rom_id).await.map_err(err)?;

    // Session registration parity (cloud_mixin.py:2818-2842): stamp the
    // cloud sync-state session markers at spawn.
    if let Ok(Some(installed_game)) = installed_game_by_rom_id(&install, rom_id).await {
        state
            .cloud
            .stamp_session_started(
                install,
                launch,
                &config_path,
                &installed_game,
                session.started_at as f64,
            )
            .await;
    }

    Ok(session)
}

#[tauri::command]
pub fn stop_game(state: State<'_, AppState>, session_id: u64) -> Result<(), String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?;
    launch.stop(session_id);
    Ok(())
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<SessionsSnapshot, String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?;
    Ok(launch.snapshot())
}

// --- emulator config commands -------------------------------------------------

#[tauri::command]
pub async fn list_emulators() -> Result<Vec<EmulatorEntry>, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.emulators)
    })
    .await
    .map_err(|e| format!("list_emulators did not finish: {e}"))?
}

/// D1 call site B. An ADD (a blank `original_name`, or one naming no current
/// entry) gets the matched profile's defaults applied before the merge and a
/// full autoconfig sync after the save; an EDIT gets neither. The command's
/// `Result` is unchanged either way — a sync warning is logged and the
/// command still returns `Ok`.
#[tauri::command]
pub async fn save_emulator(
    state: State<'_, AppState>,
    original_name: String,
    entry: EmulatorEntry,
) -> Result<(), String> {
    // Read out of the install service before the blocking hop: `State` is not
    // `Send`. An install service that failed to build simply contributes no
    // platforms and no credentials.
    let (platforms, platform_slugs, ra) = match state.install.as_ref() {
        Ok(install) => (
            install.known_platforms(),
            install.platform_slugs(),
            install.ra_credentials(),
        ),
        Err(_) => (Vec::new(), std::collections::BTreeMap::new(), None),
    };

    let session = state.session.clone();
    let install_for_firmware = state.install.as_ref().ok().cloned();
    let firmware = state.firmware.clone();

    // `Some(entry)` only when this save ADDED an RPCS3 entry: the PS3
    // firmware trigger's precondition. Handed back out of the blocking hop
    // so the trigger (which spawns a tokio task) runs on the async side.
    let rpcs3_added =
        tokio::task::spawn_blocking(move || -> Result<Option<EmulatorEntry>, String> {
            let config_path = Config::default_path();
            let profiles = load_profiles();
            // The autoconfig sync below reads no config.json and can be slow
            // (it writes emulator config files), so it runs AFTER the write
            // lock is released, on the three values the closure hands back.
            let (is_add, saved_name, library_path, saved_entry) =
                modify_config(&config_path, |config| {
                    let is_add = is_manual_add(config, &original_name);
                    let entry = manual_add_entry(entry, is_add, profiles);
                    // The name as it will be STORED, so the sync lookup matches exactly.
                    let saved_name = entry.name.clone();
                    let saved_entry = entry.clone();
                    apply_save_emulator(config, &original_name, entry)?;
                    Ok((is_add, saved_name, config.library_path.clone(), saved_entry))
                })?;

            if is_add {
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &platforms,
                    platform_slugs: &platform_slugs,
                    ps3_library_path: autoconfig::ps3_library_path(&library_path),
                    ra,
                    profiles,
                };
                // Warnings name emulators and file paths only — never a secret.
                match autoconfig::sync_new_emulator(&saved_name, &ctx) {
                    Ok(report) => {
                        for warning in report.warnings {
                            tracing::warn!("emulator autoconfig: {warning}");
                        }
                    }
                    Err(e) => tracing::warn!("emulator autoconfig: {e}"),
                }
            }
            // D2/D17: adding an RPCS3 entry by hand kicks off the PS3 firmware
            // fetch, the same as installing RPCS3 from the catalog does. An EDIT
            // never does — the firmware is already there, or the user declined
            // it once.
            let is_rpcs3_add = is_add && autoconfig::is_rpcs3(&saved_entry, profiles);
            Ok(is_rpcs3_add.then_some(saved_entry))
        })
        .await
        .map_err(|e| format!("save_emulator did not finish: {e}"))??;

    if let (Some(entry), Some(install)) = (rpcs3_added, install_for_firmware) {
        firmware.spawn_ps3_firmware(session, install, entry);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_emulator(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            apply_delete_emulator(config, &name);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("delete_emulator did not finish: {e}"))?
}

/// The emulator names that support each requested platform, keyed by the
/// platform string that was asked about. One config + profile load answers
/// the whole batch; each platform runs the ported
/// `compatible_emulator_names_for_platform` (doc 04 §2), so names come back
/// in config order with blank-named entries skipped.
///
/// The Emulators panel calls this to build its per-platform default
/// selector, which offers only compatible emulators — matching Python's
/// `_on_default_platform_changed` (emulator_ui_mixin.py:598).
#[tauri::command]
pub async fn compatible_emulators(
    platforms: Vec<String>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let profiles = load_profiles();
        Ok(platforms
            .into_iter()
            .map(|platform| {
                let names = compatible_emulator_names_for_platform(
                    &config.emulators,
                    &platform,
                    profiles,
                    &installed_core_resolver,
                );
                (platform, names)
            })
            .collect())
    })
    .await
    .map_err(|e| format!("compatible_emulators did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_default_emulator(platform: String, name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let profiles = load_profiles();
        modify_config(&Config::default_path(), |config| {
            // Inside the closure so the check and the write see the same
            // config; an Err here aborts the write (config_write.rs).
            check_default_emulator_supported(config, &platform, &name, profiles)?;
            apply_set_default_emulator(config, &platform, &name);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_default_emulator did not finish: {e}"))?
}

#[tauri::command]
pub async fn get_launch_defaults() -> Result<LaunchDefaults, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(LaunchDefaults {
            default_emulators: config.default_emulators,
            retroarch_cores: config.retroarch_cores,
            launch_args: config.launch_args,
        })
    })
    .await
    .map_err(|e| format!("get_launch_defaults did not finish: {e}"))?
}

// --- RetroAchievements credential commands ------------------------------------

/// [`get_retroachievements_status`]'s return shape. `token_present` is a
/// bare boolean — never the token, its length, or a prefix.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaStatus {
    pub username: String,
    pub token_present: bool,
}

/// One [`autoconfig::fan_out_ra_credentials`] row, renamed for the IPC
/// boundary (`emulator` rather than the tuple's positional field).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaFanOutRow {
    pub emulator: String,
    pub changed: bool,
}

fn ra_fan_out_rows(rows: Vec<(String, bool)>) -> Vec<RaFanOutRow> {
    rows.into_iter()
        .map(|(emulator, changed)| RaFanOutRow { emulator, changed })
        .collect()
}

/// Saves the RetroAchievements login, then fans it out to every registered
/// RA-capable emulator's narrow credential writer (D2). The reference
/// re-runs the FULL per-emulator sync for every emulator instead
/// (`_on_ra_login_finished`, grid-launcher.py:2730-2754); this only ever
/// touches the three credential keys, via
/// [`autoconfig::fan_out_ra_credentials`].
///
/// `token` is checked for blankness (post-trim) on the plain argument and
/// then wrapped in `SecretString` immediately — the plain `String` is never
/// read again and is dropped at the end of this scope, matching `connect`.
/// A blank token clears the keyring entry rather than storing an empty
/// secret; either way the username is written to
/// `Config.retroachievements_username` (plain, non-secret) before the
/// fan-out runs, so `fan_out_ra_credentials`'s own `usable()` gate decides
/// whether anything is actually written.
#[tauri::command]
pub async fn set_retroachievements_credentials(
    state: State<'_, AppState>,
    username: String,
    token: String,
) -> Result<Vec<RaFanOutRow>, String> {
    let token_is_blank = token.trim().is_empty();
    let token = SecretString::from(token);
    let trimmed_username = username.trim().to_string();
    let ra_store = state.ra_store.clone();

    tokio::task::spawn_blocking(move || {
        if token_is_blank {
            ra_store.clear().map_err(err)?;
        } else {
            ra_store.save(&token).map_err(err)?;
        }

        // The fan-out writes emulator config files, never config.json, so
        // it runs outside the write lock on the saved snapshot.
        let config = modify_config(&Config::default_path(), |config| {
            config.retroachievements_username = trimmed_username.clone();
            Ok(config.clone())
        })?;

        let ra = RaCredentials::new(trimmed_username, token);
        let rows = autoconfig::fan_out_ra_credentials(&config, load_profiles(), &ra);
        Ok(ra_fan_out_rows(rows))
    })
    .await
    .map_err(|e| format!("set_retroachievements_credentials did not finish: {e}"))?
}

/// The username from config and whether a token is stored — NEVER the
/// token, its length, or a prefix.
#[tauri::command]
pub async fn get_retroachievements_status(state: State<'_, AppState>) -> Result<RaStatus, String> {
    let ra_store = state.ra_store.clone();
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let token_present = ra_store.load().map_err(err)?.is_some();
        Ok(RaStatus {
            username: config.retroachievements_username,
            token_present,
        })
    })
    .await
    .map_err(|e| format!("get_retroachievements_status did not finish: {e}"))?
}

/// Clears the keyring entry and blanks `Config.retroachievements_username`.
/// Writes NOTHING to any emulator config and scrubs NOTHING already written
/// (parity with the reference's `_ra_clear_credentials`,
/// grid-launcher.py:2757-2765 — doc 05's "credentials are written but never
/// removed" open question, ruled: follow the code).
#[tauri::command]
pub async fn clear_retroachievements_credentials(state: State<'_, AppState>) -> Result<(), String> {
    let ra_store = state.ra_store.clone();
    tokio::task::spawn_blocking(move || {
        ra_store.clear().map_err(err)?;
        modify_config(&Config::default_path(), |config| {
            apply_clear_retroachievements(config);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("clear_retroachievements_credentials did not finish: {e}"))?
}

/// [`clear_retroachievements_credentials`]'s config-mutation logic, pulled
/// out so it is unit-testable without a keyring: blanks the username and
/// touches nothing else — in particular no emulator config file.
fn apply_clear_retroachievements(config: &mut Config) {
    config.retroachievements_username = String::new();
}

// --- autoprofile commands ----------------------------------------------------

#[tauri::command]
pub fn list_profiles() -> Vec<ProfileSummary> {
    visible_profiles(load_profiles())
        .into_iter()
        .map(|p| ProfileSummary {
            name: p.name.clone(),
            args: p.args.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn match_profile(executable_path: String) -> Option<ProfileSummary> {
    profile_for_entry("", &executable_path, load_profiles()).map(|p| ProfileSummary {
        name: p.name.clone(),
        args: p.args.clone(),
    })
}

// --- emulator catalog commands ------------------------------------------------

/// The "install from catalog" listing, freshly marked against the config on
/// disk. Also surfaces an install-service construction failure early (the
/// same error `install_emulator` would return on click) so the panel's
/// error line can show it before the user ever presses Install.
#[tauri::command]
pub fn list_emulator_catalog(state: State<'_, AppState>) -> Result<Vec<CatalogEntry>, String> {
    state.install.as_ref().map_err(Clone::clone)?;
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let mut entries = catalog_entries(load_profiles());
    mark_installed(&mut entries, &config);
    Ok(entries)
}

#[tauri::command]
pub async fn install_emulator(state: State<'_, AppState>, source_id: String) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.install_emulator(source_id).await.map_err(err)
}

// --- pure config-merge helpers (unit-tested below) ---------------------------

/// [`save_emulator`]'s merge logic. Validates `entry.name`, removes the
/// rename source (`original_name`, case-insensitive) if any, rejects a
/// duplicate against what remains, repoints any `default_emulators` value
/// that named the rename source, then writes `entry` back.
///
/// Selection fallback picks the first config-order match, so an edit must
/// not reorder the list: when `original_name` names an entry that is still
/// present, `entry` replaces it at its original index. Only a genuine add
/// (blank `original_name`, or one that names no current entry) appends at
/// the end.
fn apply_save_emulator(
    config: &mut Config,
    original_name: &str,
    entry: EmulatorEntry,
) -> Result<(), String> {
    if entry.name.trim().is_empty() {
        return Err("Emulator name is required.".to_string());
    }

    let original = original_name.trim();
    let mut original_index = None;
    if !original.is_empty() {
        let folded = original.to_lowercase();
        original_index = config
            .emulators
            .iter()
            .position(|e| e.name.trim().to_lowercase() == folded);
        if let Some(idx) = original_index {
            config.emulators.remove(idx);
        }
    }

    let new_name_folded = entry.name.trim().to_lowercase();
    let duplicate = config
        .emulators
        .iter()
        .any(|e| e.name.trim().to_lowercase() == new_name_folded);
    if duplicate {
        return Err(format!(
            "An emulator named '{}' already exists.",
            entry.name
        ));
    }

    if !original.is_empty() {
        let folded = original.to_lowercase();
        for value in config.default_emulators.values_mut() {
            if value.trim().to_lowercase() == folded {
                *value = entry.name.clone();
            }
        }
    }

    match original_index {
        Some(idx) => config.emulators.insert(idx, entry),
        None => config.emulators.push(entry),
    }
    Ok(())
}

/// Whether this `save_emulator` call is an ADD rather than an edit: a blank
/// `original_name`, or one that names no current entry. Only an add runs the
/// profile defaults and the autoconfig sync (D1).
fn is_manual_add(config: &Config, original_name: &str) -> bool {
    let original = original_name.trim();
    if original.is_empty() {
        return true;
    }
    let folded = original.to_lowercase();
    !config
        .emulators
        .iter()
        .any(|e| e.name.trim().to_lowercase() == folded)
}

/// The hand-typed-entry half of layer 1
/// (`apply_manual_emulator_profile_defaults`, autoconfig.py:228): blank
/// fields take the matched profile's values and `path` is never touched. An
/// edit, or an entry no profile matches, passes through unchanged.
fn manual_add_entry(
    entry: EmulatorEntry,
    is_add: bool,
    profiles: &[EmulatorProfile],
) -> EmulatorEntry {
    if !is_add {
        return entry;
    }
    match profile_for_entry(&entry.name, &entry.path, profiles) {
        Some(profile) => autoconfig_entry::apply_manual_emulator_profile_defaults(&entry, profile),
        None => entry,
    }
}

/// [`delete_emulator`]'s merge logic: drops `name` (case-insensitive) from
/// `emulators`, and any `default_emulators` entry whose value named it.
fn apply_delete_emulator(config: &mut Config, name: &str) {
    let folded = name.trim().to_lowercase();
    config
        .emulators
        .retain(|e| e.name.trim().to_lowercase() != folded);
    config
        .default_emulators
        .retain(|_, v| v.trim().to_lowercase() != folded);
}

/// [`set_default_emulator`]'s guard: the picked emulator must actually
/// support the platform it is being made the default for. A blank `name`
/// (which CLEARS the mapping) always passes; any other name must resolve to
/// a configured entry that `emulator_supports_platform` accepts (doc 04
/// §2). A name no entry matches is refused with the same message — there is
/// nothing to support the platform with.
///
/// Python only ever OFFERS compatible names in the combo box
/// (emulator_ui_mixin.py:598) and drops an incompatible stored default at
/// read time; the port additionally refuses the write.
fn check_default_emulator_supported(
    config: &Config,
    platform: &str,
    name: &str,
    profiles: &[EmulatorProfile],
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let supported = emulator_entry_by_name(&config.emulators, trimmed).is_some_and(|entry| {
        emulator_supports_platform(entry, platform, profiles, &installed_core_resolver)
    });
    if supported {
        Ok(())
    } else {
        Err(format!("{trimmed} does not support {platform}"))
    }
}

/// [`set_default_emulator`]'s merge logic. A blank `name` removes the
/// `platform` key (exact match first, then case-insensitive); otherwise the
/// value is inserted/overwritten under the exact key when one already
/// exists, else under a case-insensitive match's key, else as a new key.
fn apply_set_default_emulator(config: &mut Config, platform: &str, name: &str) {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        remove_platform_key(&mut config.default_emulators, platform);
        return;
    }
    upsert_platform_key(&mut config.default_emulators, platform, trimmed_name);
}

fn remove_platform_key(map: &mut BTreeMap<String, String>, platform: &str) {
    if map.remove(platform).is_some() {
        return;
    }
    let folded = platform.to_lowercase();
    if let Some(key) = map.keys().find(|k| k.to_lowercase() == folded).cloned() {
        map.remove(&key);
    }
}

fn upsert_platform_key(map: &mut BTreeMap<String, String>, platform: &str, value: &str) {
    if map.contains_key(platform) {
        map.insert(platform.to_string(), value.to_string());
        return;
    }
    let folded = platform.to_lowercase();
    if let Some(key) = map.keys().find(|k| k.to_lowercase() == folded).cloned() {
        map.remove(&key);
    }
    map.insert(platform.to_string(), value.to_string());
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn entry(name: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: "/x/emu".to_string(),
            args: String::new(),
            ..Default::default()
        }
    }

    fn config_with(emulators: &[&str], defaults: &[(&str, &str)]) -> Config {
        Config {
            emulators: emulators.iter().map(|n| entry(n)).collect(),
            default_emulators: defaults
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Config::default()
        }
    }

    // --- apply_save_emulator -------------------------------------------------

    #[test]
    fn save_rejects_blank_name() {
        let mut config = config_with(&[], &[]);
        let result = apply_save_emulator(&mut config, "", entry("   "));
        assert_eq!(result, Err("Emulator name is required.".to_string()));
        assert!(config.emulators.is_empty());
    }

    #[test]
    fn save_appends_new_emulator() {
        let mut config = config_with(&[], &[]);
        apply_save_emulator(&mut config, "", entry("Dolphin")).unwrap();
        assert_eq!(config.emulators, vec![entry("Dolphin")]);
    }

    #[test]
    fn save_rejects_duplicate_name_case_insensitively() {
        let mut config = config_with(&["Dolphin"], &[]);
        let result = apply_save_emulator(&mut config, "", entry("dolphin"));
        assert_eq!(
            result,
            Err("An emulator named 'dolphin' already exists.".to_string())
        );
        assert_eq!(config.emulators.len(), 1);
    }

    #[test]
    fn save_rename_removes_original_before_duplicate_check() {
        let mut config = config_with(&["Dolphin"], &[]);
        apply_save_emulator(&mut config, "Dolphin", entry("Dolphin Renamed")).unwrap();
        assert_eq!(config.emulators, vec![entry("Dolphin Renamed")]);
    }

    #[test]
    fn save_rename_to_an_existing_other_name_is_rejected() {
        let mut config = config_with(&["Dolphin", "PCSX2"], &[]);
        let result = apply_save_emulator(&mut config, "Dolphin", entry("PCSX2"));
        assert_eq!(
            result,
            Err("An emulator named 'PCSX2' already exists.".to_string())
        );
        // The original was removed by the (failed) rename attempt's retain
        // step logically, but since the whole call errors before pushing,
        // the caller never persists this — check the in-memory removal is
        // real so the invariant driving the check is verified directly.
        assert_eq!(config.emulators, vec![entry("PCSX2")]);
    }

    #[test]
    fn save_rename_repoints_default_emulators_case_insensitively() {
        let mut config = config_with(&["Dolphin"], &[("GameCube", "dolphin"), ("Wii", "Other")]);
        apply_save_emulator(&mut config, "Dolphin", entry("Dolphin Renamed")).unwrap();
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("Dolphin Renamed")
        );
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some("Other")
        );
    }

    #[test]
    fn save_unrelated_original_name_leaves_existing_untouched() {
        let mut config = config_with(&["Dolphin"], &[]);
        apply_save_emulator(&mut config, "Nonexistent", entry("PCSX2")).unwrap();
        assert_eq!(config.emulators, vec![entry("Dolphin"), entry("PCSX2")]);
    }

    /// Selection fallback is config-order-first, so editing an entry must not
    /// move it — otherwise editing entry #1 silently changes which emulator
    /// auto-launches.
    #[test]
    fn save_edit_first_entry_keeps_its_index_and_order() {
        let mut config = config_with(&["Dolphin", "PCSX2", "Yuzu"], &[]);
        apply_save_emulator(&mut config, "Dolphin", entry("Dolphin Updated")).unwrap();
        assert_eq!(
            config.emulators,
            vec![entry("Dolphin Updated"), entry("PCSX2"), entry("Yuzu")]
        );
    }

    #[test]
    fn save_rename_in_place_keeps_position() {
        let mut config = config_with(&["Dolphin", "PCSX2", "Yuzu"], &[]);
        apply_save_emulator(&mut config, "PCSX2", entry("PCSX2 Renamed")).unwrap();
        assert_eq!(
            config.emulators,
            vec![entry("Dolphin"), entry("PCSX2 Renamed"), entry("Yuzu")]
        );
    }

    // --- apply_delete_emulator ------------------------------------------------

    #[test]
    fn delete_removes_case_insensitively() {
        let mut config = config_with(&["Dolphin", "PCSX2"], &[]);
        apply_delete_emulator(&mut config, "dolphin");
        assert_eq!(config.emulators, vec![entry("PCSX2")]);
    }

    #[test]
    fn delete_drops_default_emulators_pointing_at_it() {
        let mut config = config_with(
            &["Dolphin"],
            &[
                ("GameCube", "Dolphin"),
                ("Wii", "dolphin"),
                ("N64", "Other"),
            ],
        );
        apply_delete_emulator(&mut config, "Dolphin");
        assert!(!config.default_emulators.contains_key("GameCube"));
        assert!(!config.default_emulators.contains_key("Wii"));
        assert_eq!(
            config.default_emulators.get("N64").map(String::as_str),
            Some("Other")
        );
    }

    #[test]
    fn delete_missing_name_is_a_no_op() {
        let mut config = config_with(&["Dolphin"], &[]);
        apply_delete_emulator(&mut config, "Nonexistent");
        assert_eq!(config.emulators, vec![entry("Dolphin")]);
    }

    // --- apply_set_default_emulator -------------------------------------------

    #[test]
    fn set_default_blank_name_removes_exact_key() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "GameCube", "");
        assert!(config.default_emulators.is_empty());
    }

    #[test]
    fn set_default_blank_name_removes_case_insensitive_key() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "gamecube", "  ");
        assert!(config.default_emulators.is_empty());
    }

    #[test]
    fn set_default_blank_name_with_no_matching_key_is_a_no_op() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "Wii", "");
        assert_eq!(config.default_emulators.len(), 1);
    }

    #[test]
    fn set_default_overwrites_exact_key() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "GameCube", "PCSX2");
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("PCSX2")
        );
        assert_eq!(config.default_emulators.len(), 1);
    }

    #[test]
    fn set_default_replaces_case_insensitive_key_with_the_new_casing() {
        let mut config = config_with(&[], &[("gamecube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "GameCube", "PCSX2");
        assert_eq!(config.default_emulators.len(), 1);
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("PCSX2")
        );
        assert!(!config.default_emulators.contains_key("gamecube"));
    }

    #[test]
    fn set_default_inserts_new_key_when_absent() {
        let mut config = config_with(&[], &[]);
        apply_set_default_emulator(&mut config, "Wii", "Dolphin");
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some("Dolphin")
        );
    }

    #[test]
    fn set_default_trims_the_value_but_not_the_platform_key() {
        let mut config = config_with(&[], &[]);
        apply_set_default_emulator(&mut config, "Wii", "  Dolphin  ");
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some("Dolphin")
        );
    }

    // --- check_default_emulator_supported --------------------------------------

    /// A profile that matches the `config_with` entry named "PCSX2" by name
    /// and supports PlayStation 2 only.
    fn ps2_only_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "PCSX2".to_string(),
            platform_keywords: vec!["playstation 2".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn default_check_accepts_a_compatible_pick() {
        let config = config_with(&["PCSX2"], &[]);
        let result = check_default_emulator_supported(
            &config,
            "Sony PlayStation 2",
            "PCSX2",
            &[ps2_only_profile()],
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn default_check_refuses_an_incompatible_pick() {
        let config = config_with(&["PCSX2"], &[]);
        let result =
            check_default_emulator_supported(&config, "GameCube", "PCSX2", &[ps2_only_profile()]);
        assert_eq!(result, Err("PCSX2 does not support GameCube".to_string()));
    }

    #[test]
    fn default_check_refuses_a_name_no_entry_matches() {
        let config = config_with(&["PCSX2"], &[]);
        let result =
            check_default_emulator_supported(&config, "GameCube", "Ghost", &[ps2_only_profile()]);
        assert_eq!(result, Err("Ghost does not support GameCube".to_string()));
    }

    #[test]
    fn default_check_allows_a_blank_name_which_clears_the_mapping() {
        let config = config_with(&["PCSX2"], &[("GameCube", "PCSX2")]);
        let result =
            check_default_emulator_supported(&config, "GameCube", "  ", &[ps2_only_profile()]);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn set_default_refusal_writes_nothing_to_the_config_file() {
        // The guard runs inside the modify_config closure, so its Err has to
        // abort the whole load-modify-save (config_write.rs).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        config_with(&["PCSX2"], &[]).save(&path).unwrap();
        let profiles = vec![ps2_only_profile()];

        let result = modify_config(&path, |config| {
            check_default_emulator_supported(config, "GameCube", "PCSX2", &profiles)?;
            apply_set_default_emulator(config, "GameCube", "PCSX2");
            Ok(())
        });

        assert_eq!(result, Err("PCSX2 does not support GameCube".to_string()));
        assert!(Config::load(&path).unwrap().default_emulators.is_empty());
    }

    // --- manual-add profile defaults (D1) --------------------------------------

    fn pcsx2_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "PCSX2".to_string(),
            match_tokens: vec!["pcsx2*".to_string()],
            args: "-batch %rom%".to_string(),
            save_strategy: "folder".to_string(),
            save_directories: vec!["~/pcsx2/memcards".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn manual_add_applies_profile_defaults_but_an_edit_does_not() {
        let profiles = vec![pcsx2_profile()];
        let typed = EmulatorEntry {
            name: "PCSX2".to_string(),
            path: "/opt/pcsx2/pcsx2.AppImage".to_string(),
            ..Default::default()
        };

        let added = manual_add_entry(typed.clone(), true, &profiles);
        assert_eq!(added.args, "-batch %rom%");
        assert_eq!(added.save_paths, "~/pcsx2/memcards");
        assert_eq!(added.save_strategy, "folder");

        let edited = manual_add_entry(typed.clone(), false, &profiles);
        assert_eq!(edited, typed, "an edit must pass through untouched (D1)");

        // No matching profile: the add passes through unchanged too.
        let unmatched = manual_add_entry(typed.clone(), true, &[]);
        assert_eq!(unmatched, typed);
    }

    #[test]
    fn manual_add_never_overwrites_the_typed_path() {
        let mut profiles = vec![pcsx2_profile()];
        profiles[0].args = "--other %rom%".to_string();
        let typed = EmulatorEntry {
            name: "PCSX2".to_string(),
            path: "/home/me/my own build/pcsx2".to_string(),
            args: "%rom%".to_string(),
            ..Default::default()
        };

        let added = manual_add_entry(typed.clone(), true, &profiles);
        assert_eq!(
            added.path, "/home/me/my own build/pcsx2",
            "autoconfig.py:228 never touches `path`"
        );
        assert_eq!(added.args, "--other %rom%", "a bare %rom% IS replaced");
    }

    #[test]
    fn is_manual_add_is_true_for_a_blank_or_unknown_original_name() {
        let config = config_with(&["Dolphin"], &[]);
        assert!(is_manual_add(&config, ""));
        assert!(is_manual_add(&config, "   "));
        assert!(is_manual_add(&config, "Nonexistent"));
        assert!(!is_manual_add(&config, "Dolphin"));
        assert!(!is_manual_add(&config, "  dolphin  "));
    }

    #[test]
    fn should_backfill_on_platform_list_needs_a_non_empty_assignable_list() {
        assert!(!should_backfill_on_platform_list(&[]));
        assert!(should_backfill_on_platform_list(&["SNES".to_string()]));
    }
}

#[cfg(test)]
mod retroachievements_tests {
    use super::*;

    /// `RaStatus`'s whole point is that the token itself never crosses IPC —
    /// only a presence boolean. Serialize a status built for a token that IS
    /// present and assert the token text never appears in the JSON.
    #[test]
    fn ra_status_never_contains_the_token() {
        let status = RaStatus {
            username: "sixdd6".to_string(),
            token_present: true,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("FAKE-RA-TOKEN-not-real"));
        assert!(!json.to_lowercase().contains("token_value"));
        assert_eq!(json, r#"{"username":"sixdd6","token_present":true}"#);
    }

    /// [`apply_clear_retroachievements`] is the config-mutation half of
    /// `clear_retroachievements_credentials`: it blanks the username and
    /// touches nothing else. Proven here against a real emulator config
    /// file's mtime, standing in for "no emulator file is written" — the
    /// keyring clear itself is covered by
    /// `secrets::tests::ra_token_store_round_trips_independently_of_the_romm_credential`.
    #[test]
    fn clear_blanks_the_username_and_writes_no_emulator_file() {
        let temp = tempfile::tempdir().unwrap();
        let emulator_cfg = temp.path().join("retroarch.cfg");
        std::fs::write(&emulator_cfg, "cheevos_username = \"sixdd6\"\n").unwrap();
        let before = std::fs::metadata(&emulator_cfg)
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut config = Config {
            retroachievements_username: "sixdd6".to_string(),
            ..Config::default()
        };
        apply_clear_retroachievements(&mut config);

        assert_eq!(config.retroachievements_username, "");
        let after = std::fs::metadata(&emulator_cfg)
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            before, after,
            "clear must never touch an emulator config file"
        );
    }
}
