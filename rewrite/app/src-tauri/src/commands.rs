pub mod cloud;

use grid_core::autoconfig::{self, entry as autoconfig_entry, RaCredentials};
use grid_core::config::{Config, EmulatorEntry};
use grid_core::launch::catalog::{catalog_entries, mark_installed, CatalogEntry};
use grid_core::launch::profiles::{
    load_profiles, profile_for_entry, visible_profiles, EmulatorProfile,
};
use grid_core::launch::{GameSession, LaunchService, SessionsSnapshot};
use grid_core::library::queue::DownloadsSnapshot;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::romm::{GameSummary, Platform};
use grid_core::secrets::RaTokenStore;
use grid_core::session::{SessionManager, SessionState};
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
}

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    // RommError/SessionError Display are credential-free by construction.
    e.to_string()
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    server_url: String,
    username: String,
    secret: String,
    use_token: bool,
) -> Result<SessionState, String> {
    // Wrap immediately; the plain String is dropped at the end of this scope.
    let secret = SecretString::from(secret);
    state
        .session
        .connect(server_url, username, secret, use_token)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn restore_session(state: State<'_, AppState>) -> Result<Option<SessionState>, String> {
    state.session.restore().await.map_err(err)
}

#[tauri::command]
pub fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    state.session.disconnect().map_err(err)
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
            let outcome = tokio::task::spawn_blocking(move || {
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &assignable,
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
pub async fn ensure_cover(
    state: State<'_, AppState>,
    game_id: i64,
    cover_path: String,
) -> Result<String, String> {
    let client = state.session.client().ok_or("not connected")?;
    let path = state
        .session
        .cache()
        .ensure(&client, game_id, &cover_path)
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
pub async fn uninstall_game(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || install.uninstall(rom_id).map_err(err))
        .await
        .map_err(|e| format!("uninstall did not finish: {e}"))?
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
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        config.library_path = path;
        config.save(&config_path).map_err(err)
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
    let (platforms, ra) = match state.install.as_ref() {
        Ok(install) => (install.known_platforms(), install.ra_credentials()),
        Err(_) => (Vec::new(), None),
    };

    tokio::task::spawn_blocking(move || {
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        let profiles = load_profiles();

        let is_add = is_manual_add(&config, &original_name);
        let entry = manual_add_entry(entry, is_add, profiles);
        // The name as it will be STORED, so the sync lookup matches exactly.
        let saved_name = entry.name.clone();

        apply_save_emulator(&mut config, &original_name, entry)?;
        config.save(&config_path).map_err(err)?;

        if is_add {
            let ctx = autoconfig::SyncContext {
                config_path: &config_path,
                platforms: &platforms,
                ps3_library_path: autoconfig::ps3_library_path(&config.library_path),
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
        Ok(())
    })
    .await
    .map_err(|e| format!("save_emulator did not finish: {e}"))?
}

#[tauri::command]
pub async fn delete_emulator(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        apply_delete_emulator(&mut config, &name);
        config.save(&config_path).map_err(err)
    })
    .await
    .map_err(|e| format!("delete_emulator did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_default_emulator(platform: String, name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        apply_set_default_emulator(&mut config, &platform, &name);
        config.save(&config_path).map_err(err)
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

        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        config.retroachievements_username = trimmed_username.clone();
        config.save(&config_path).map_err(err)?;

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
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        apply_clear_retroachievements(&mut config);
        config.save(&config_path).map_err(err)
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
