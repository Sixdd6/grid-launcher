use grid_core::config::{Config, EmulatorEntry};
use grid_core::launch::profiles::{load_profiles, profile_for_entry, visible_profiles};
use grid_core::launch::{GameSession, LaunchService, SessionsSnapshot};
use grid_core::library::queue::DownloadsSnapshot;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::romm::{GameSummary, Platform};
use grid_core::session::{SessionManager, SessionState};
use secrecy::SecretString;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub session: SessionManager,
    pub install: Result<Arc<InstallService>, String>,
    pub launch: Result<Arc<LaunchService>, String>,
}

fn err(e: impl std::fmt::Display) -> String {
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

#[tauri::command]
pub async fn list_platforms(state: State<'_, AppState>) -> Result<Vec<Platform>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.platforms().await.map_err(err)
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
    let client = state.session.client().ok_or("not connected")?;
    install.retry(client, entry_id).await.map_err(err)
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

#[tauri::command]
pub async fn launch_game(state: State<'_, AppState>, rom_id: i64) -> Result<GameSession, String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?;
    launch.launch(rom_id).await.map_err(err)
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

#[tauri::command]
pub async fn save_emulator(original_name: String, entry: EmulatorEntry) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        apply_save_emulator(&mut config, &original_name, entry)?;
        config.save(&config_path).map_err(err)
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
            source_id: String::new(),
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
}
