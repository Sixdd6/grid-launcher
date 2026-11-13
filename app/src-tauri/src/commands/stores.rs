//! External game stores (Phase 1: GOG) commands and events
//! (`docs/stores-phase1-plan.md` "Tauri commands and events"). Thin
//! wrappers: `stores_service.rs` and grid-core's `stores` module own every
//! rule.
//!
//! camelCase arguments, snake_case JSON. A `key` argument is a game key
//! string (`gog:1207664643`); for the backup commands it is the drive key.
//! The IPC types below mirror `app/src/lib/api.ts` field for field.
//!
//! Secrets: no payload here carries a login code or a token, and nothing in
//! this file may log or forward a store tool's output.
//!
//! Step 0: every command is registered and answers "not implemented" until
//! package D fills it in.

// The contract types are built by package D; until then only the command
// signatures name them.
#![allow(dead_code)]

use grid_core::backup::plan::BackupQueued;
use serde::{Deserialize, Serialize};

/// Emitted with the full `StoreStatus[]` whenever a store's status changes.
pub const STORES_CHANGED_EVENT: &str = "stores-changed";
/// Emitted with a [`StoreLoginEvent`] when the login window finishes.
pub const STORE_LOGIN_EVENT: &str = "store-login";
/// Emitted with a [`StoreNotice`] for a toast the backend decides on.
pub const STORE_NOTICE_EVENT: &str = "store-notice";

/// The error every Step 0 stub answers.
const NOT_IMPLEMENTED: &str = "not implemented";

fn not_implemented<T>() -> Result<T, String> {
    Err(NOT_IMPLEMENTED.to_string())
}

// --- IPC types (mirror api.ts) ----------------------------------------------

/// The store tool's state in Settings › Stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    NotInstalled,
    Installing,
    Installed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolStatus {
    /// `gogdl`.
    pub name: String,
    pub version: String,
    pub state: ToolState,
    pub size_bytes: u64,
    pub error: String,
    pub license: String,
    pub source_url: String,
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreConnection {
    Connected,
    Expired,
    NotConnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnedState {
    Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreStatus {
    /// `gog`.
    pub source: String,
    pub connection: StoreConnection,
    pub username: String,
    pub tool: ToolStatus,
    pub owned: OwnedState,
    pub owned_error: String,
    pub owned_count: u32,
    pub installed_count: u32,
    pub installed_bytes: u64,
    pub refreshed_at: i64,
    pub cloud_saves: bool,
    pub install_dir: String,
    pub in_rail: bool,
    pub last_error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreDlc {
    pub id: String,
    pub title: String,
    pub download_bytes: u64,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreGame {
    pub key: String,
    pub source: String,
    pub store_id: String,
    pub title: String,
    pub installed: bool,
    pub cover: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub developer: String,
    pub genres: Vec<String>,
    pub year: String,
    /// `linux` | `windows`.
    pub platforms: Vec<String>,
    /// `linux` | `windows`, `None` when not installed.
    pub installed_platform: Option<String>,
    pub download_bytes: u64,
    pub disk_bytes: u64,
    pub version: String,
    pub update_version: String,
    /// `gogdl` | `offline` | `drive`, `None` when not installed.
    pub origin: Option<String>,
    pub not_supported: String,
    pub dlc: Vec<StoreDlc>,
    /// `off` | `none` | `synced` | `conflict` | `unknown`.
    pub cloud: String,
    pub last_sync: i64,
    pub install_dir: String,
    pub compat_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreLibrary {
    pub status: StoreStatus,
    pub installed: Vec<StoreGame>,
    pub available: Vec<StoreGame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreVersionOption {
    /// `linux` | `windows`.
    pub platform: String,
    pub recommended: bool,
    pub download_bytes: u64,
    pub disk_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreInstallOptions {
    pub versions: Vec<StoreVersionOption>,
    pub languages: Vec<String>,
    pub default_language: String,
    pub dlc: Vec<StoreDlc>,
    pub install_dir: String,
    pub free_bytes: u64,
    pub volume: String,
    pub compat_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StoreInstallRequest {
    pub key: String,
    /// `linux` | `windows`.
    pub platform: String,
    pub language: String,
    pub dlc_ids: Vec<String>,
    pub install_dir: String,
    pub compat_tool: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreGameSettings {
    pub install_dir: String,
    pub executable: String,
    pub exe_user_pinned: bool,
    pub launch_parameters: String,
    pub compat_tool: String,
    pub wineprefix: String,
    pub cloud_saves: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreSaveConflict {
    pub local_at: i64,
    pub local_files: u32,
    pub local_bytes: u64,
    pub cloud_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreCloudState {
    /// `off` | `none` | `synced` | `syncing` | `conflict`.
    pub state: String,
    pub synced_at: i64,
    pub locations: Vec<String>,
    pub conflict: Option<StoreSaveConflict>,
}

/// `store_cloud_resolve`'s choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudChoice {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallerVerdict {
    Install,
    Update { installed_version: String },
    Dlc { base_title: String },
    Refused { title: String, body: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallerInspection {
    pub path: String,
    /// `linux_sh` | `windows_exe` | `mac_pkg`.
    pub kind: String,
    pub key: Option<String>,
    pub title: String,
    pub cover: String,
    pub version: String,
    pub bytes: u64,
    pub verdict: InstallerVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreNoticeAction {
    /// `play` | `open_downloads` | `try_again` | `sign_in` | `install_tool`
    /// | `show_updates` | `install_base` | `open_settings`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreNotice {
    pub source: Option<String>,
    pub message: String,
    pub detail: String,
    pub error: bool,
    pub action: Option<StoreNoticeAction>,
}

/// The [`STORE_LOGIN_EVENT`] payload. Never the login code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreLoginEvent {
    pub source: String,
    /// `connected` | `cancelled` | `failed`.
    pub outcome: String,
    pub username: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriveStoreGame {
    pub key: String,
    pub name: String,
    /// `linux` | `windows`.
    pub platform: String,
    /// `installer` | `installed`.
    pub layout: String,
    pub version: String,
    pub size: u64,
    pub cover: String,
    pub installed: bool,
    pub added: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreBackupRow {
    pub key: String,
    pub name: String,
    /// `installer` | `installed`.
    pub layout: String,
    pub detail: String,
    pub bytes: u64,
    pub windows_prefix_note: bool,
    pub already: bool,
    pub refused: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreBackupPlan {
    pub drive: String,
    pub rows: Vec<StoreBackupRow>,
    pub bytes_to_write: u64,
    pub free_bytes: u64,
    pub fits: bool,
}

// --- commands -----------------------------------------------------------------

/// Every store's status (GOG only in Phase 1).
#[tauri::command]
pub async fn stores_status() -> Result<Vec<StoreStatus>, String> {
    not_implemented()
}

#[tauri::command]
pub async fn store_library(source: String) -> Result<StoreLibrary, String> {
    let _ = source;
    not_implemented()
}

/// Refreshes the owned list; the result comes as events.
#[tauri::command]
pub async fn store_refresh(source: String) -> Result<(), String> {
    let _ = source;
    not_implemented()
}

/// Checks for updates; the result comes as events.
#[tauri::command]
pub async fn store_check_updates(source: String) -> Result<(), String> {
    let _ = source;
    not_implemented()
}

/// Opens the login window; the result is a [`STORE_LOGIN_EVENT`].
#[tauri::command]
pub async fn store_connect(source: String) -> Result<(), String> {
    let _ = source;
    not_implemented()
}

#[tauri::command]
pub async fn store_cancel_login(source: String) -> Result<(), String> {
    let _ = source;
    not_implemented()
}

#[tauri::command]
pub async fn store_disconnect(source: String) -> Result<(), String> {
    let _ = source;
    not_implemented()
}

#[tauri::command]
pub async fn store_install_tool(source: String) -> Result<(), String> {
    let _ = source;
    not_implemented()
}

/// Types `text` into the login page (TV mode's on-screen keyboard).
#[tauri::command]
pub async fn store_login_type(text: String) -> Result<(), String> {
    let _ = text;
    not_implemented()
}

#[tauri::command]
pub async fn store_set_install_dir(source: String, path: String) -> Result<(), String> {
    let _ = (source, path);
    not_implemented()
}

#[tauri::command]
pub async fn store_set_cloud_saves(source: String, on: bool) -> Result<(), String> {
    let _ = (source, on);
    not_implemented()
}

#[tauri::command]
pub async fn store_install_options(key: String) -> Result<StoreInstallOptions, String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_install(request: StoreInstallRequest) -> Result<(), String> {
    let _ = request;
    not_implemented()
}

#[tauri::command]
pub async fn store_update(key: String) -> Result<(), String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_repair(key: String) -> Result<(), String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_uninstall(key: String) -> Result<(), String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_install_dlc(key: String, dlc_id: String) -> Result<(), String> {
    let _ = (key, dlc_id);
    not_implemented()
}

#[tauri::command]
pub async fn store_remove_dlc(key: String, dlc_id: String) -> Result<(), String> {
    let _ = (key, dlc_id);
    not_implemented()
}

#[tauri::command]
pub async fn store_game_settings(key: String) -> Result<StoreGameSettings, String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_set_game_settings(
    key: String,
    settings: StoreGameSettings,
) -> Result<(), String> {
    let _ = (key, settings);
    not_implemented()
}

#[tauri::command]
pub async fn store_cloud_state(key: String) -> Result<StoreCloudState, String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_cloud_sync(key: String) -> Result<StoreCloudState, String> {
    let _ = key;
    not_implemented()
}

#[tauri::command]
pub async fn store_cloud_resolve(
    key: String,
    choice: CloudChoice,
) -> Result<StoreCloudState, String> {
    let _ = (key, choice);
    not_implemented()
}

#[tauri::command]
pub async fn offline_inspect(path: String) -> Result<InstallerInspection, String> {
    let _ = path;
    not_implemented()
}

#[tauri::command]
pub async fn offline_install(
    path: String,
    compat_tool: String,
    install_dir: Option<String>,
) -> Result<(), String> {
    let _ = (path, compat_tool, install_dir);
    not_implemented()
}

/// `key` is the drive key; `game_keys` the store games' keys
/// (`commands/store_backups.rs`).
#[tauri::command]
pub async fn plan_store_backup(
    state: tauri::State<'_, crate::commands::AppState>,
    app: tauri::AppHandle,
    key: String,
    game_keys: Vec<String>,
) -> Result<StoreBackupPlan, String> {
    crate::commands::store_backups::plan(&state, &app, &key, &game_keys).await
}

#[tauri::command]
pub async fn start_store_backup(
    state: tauri::State<'_, crate::commands::AppState>,
    app: tauri::AppHandle,
    key: String,
    game_keys: Vec<String>,
) -> Result<BackupQueued, String> {
    crate::commands::store_backups::start(&state, &app, &key, &game_keys).await
}

#[tauri::command]
pub async fn drive_store_games(
    state: tauri::State<'_, crate::commands::AppState>,
    app: tauri::AppHandle,
    key: String,
) -> Result<Vec<DriveStoreGame>, String> {
    crate::commands::store_backups::list(&state, &app, &key).await
}

#[tauri::command]
pub async fn install_store_from_drive(
    state: tauri::State<'_, crate::commands::AppState>,
    app: tauri::AppHandle,
    key: String,
    game_key: String,
) -> Result<(), String> {
    crate::commands::store_backups::install(&state, &app, &key, &game_key).await
}

/// Answers how many games were removed.
#[tauri::command]
pub async fn remove_store_from_drive(
    state: tauri::State<'_, crate::commands::AppState>,
    app: tauri::AppHandle,
    key: String,
    game_keys: Vec<String>,
) -> Result<u32, String> {
    crate::commands::store_backups::remove(&state, &app, &key, &game_keys).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts_serialize_tagged() {
        assert_eq!(
            serde_json::to_value(InstallerVerdict::Install).unwrap(),
            serde_json::json!({"kind": "install"})
        );
        assert_eq!(
            serde_json::to_value(InstallerVerdict::Update {
                installed_version: "1.2".into()
            })
            .unwrap(),
            serde_json::json!({"kind": "update", "installed_version": "1.2"})
        );
        assert_eq!(
            serde_json::to_value(ToolState::NotInstalled).unwrap(),
            serde_json::json!("not_installed")
        );
        assert_eq!(
            serde_json::from_value::<CloudChoice>(serde_json::json!("upload")).unwrap(),
            CloudChoice::Upload
        );
    }
}
