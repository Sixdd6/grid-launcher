//! External game stores (GOG, Epic, Amazon, Steam) commands and events
//! (`docs/stores-phase1-plan.md` "Tauri commands and events",
//! `docs/stores-phase2-plan.md` "App"). Thin wrappers: each store command
//! finds its store's backend (`backend_for`, `backend_for_key`) and calls
//! the `stores_service::StoreBackend` trait; `stores_service/` and
//! grid-core's `stores` module own every rule. The offline installer and
//! store backup commands are GOG-only (`StoresService::gog`).
//!
//! camelCase arguments, snake_case JSON. A `key` argument is a game key
//! string (`gog:1207664643`); for the backup commands it is the drive key.
//! The IPC types below mirror `app/src/lib/api.ts` field for field.
//!
//! Secrets: no payload here carries a login code or a token, and nothing in
//! this file may log or forward a store tool's output. The Steam commands
//! take the Web API key as a plain `String` (IPC has no other form) and wrap
//! it in a `SecretString` on their first line; no answer ever carries it.
//!
//! The five backup commands at the end (`plan_store_backup` …
//! `remove_store_from_drive`) are package C's: they delegate to
//! `commands/store_backups.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use grid_core::backup::plan::BackupQueued;
use grid_core::config::Config;
use grid_core::library::queue::StoreJob;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::stores::gog::GogProduct;
use grid_core::stores::offline;
use grid_core::stores::{GameKey, StoreKind, SyncMode};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use super::{ensure_library_idle, err, AppState};
use crate::stores_service::{JobCtx, StoreBackend};

/// Emitted with the full `StoreStatus[]` whenever a store's status changes.
pub const STORES_CHANGED_EVENT: &str = "stores-changed";
/// Emitted with a [`StoreLoginEvent`] when the login window finishes.
pub const STORE_LOGIN_EVENT: &str = "store-login";
/// Emitted with a [`StoreNotice`] for a toast the backend decides on.
pub const STORE_NOTICE_EVENT: &str = "store-notice";

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
    /// `gogdl` | `legendary` | `nile`.
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
    /// `gog` | `epic` | `amazon` | `steam`.
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
    /// Steam's part of the card; `None` for every other store.
    pub steam: Option<SteamCard>,
}

/// Where Steam's Web API key stands. Step 0's status only says `None`;
/// package C sets the rest from the last Web API answer.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SteamKeyState {
    /// Steam answered with games.
    Ok,
    /// No key in the keyring.
    None,
    /// Steam rejected the key on the last call.
    Bad,
    /// The key works, but the profile's game details are private.
    Private,
}

/// An account that signed in to Steam on this computer. Never the login
/// name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SteamAccount {
    pub persona: String,
    pub steam_id: String,
    pub most_recent: bool,
}

/// Steam's part of [`StoreStatus`]. Never the key, nor any part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SteamCard {
    /// The client line: the executable's path, the Flatpak/Snap name; `""`
    /// when Steam isn't installed.
    pub client_path: String,
    /// The SteamID64 (not a secret).
    pub steam_id: String,
    pub key: SteamKeyState,
    /// Always `""`: a saved key shows as dots only (user decision 2).
    pub key_tail: String,
    pub accounts: Vec<SteamAccount>,
}

/// `steam_test` / `steam_save`'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SteamTestResult {
    Ok { owned_count: u32 },
    BadKey { status: u16 },
    Private,
    BadSteamId,
    Network,
}

/// `steam_open`'s action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SteamOpenAction {
    Install,
    Uninstall,
    Validate,
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
    /// `gogdl` | `offline` | `drive` | `legendary` | `nile` | `steam`
    /// (installed through GRID's Install in Steam) | `steam_outside` (found
    /// in Steam's files), `None` when not installed.
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
    /// `None` when the store gives no file list (Epic's `list-saves`).
    pub cloud_files: Option<u32>,
    /// `None` when the store's cloud listing gives no sizes.
    pub cloud_bytes: Option<u64>,
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
    /// Free bytes on the volume it installs to (the new game's folder, the
    /// installed game's for an update, the base game's for a DLC); `None`
    /// when unknown.
    pub free_bytes: Option<u64>,
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

/// What every store command answers for a store GRID doesn't support.
const UNSUPPORTED_STORE: &str = "GRID doesn't support that store yet.";

/// The store a `source` argument names: GOG, Epic, Amazon or Steam.
fn store_kind(source: &str) -> Result<StoreKind, String> {
    StoreKind::parse(source.trim()).ok_or_else(|| UNSUPPORTED_STORE.to_string())
}

/// The backend for a `source` argument.
fn backend_for(state: &AppState, source: &str) -> Result<Arc<dyn StoreBackend>, String> {
    let kind = store_kind(source)?;
    state
        .stores
        .backend(kind)
        .ok_or_else(|| UNSUPPORTED_STORE.to_string())
}

/// A store game key's store and product id (GOG product id, Epic app name,
/// Amazon product id, Steam app id).
fn store_key(key: &str) -> Result<(StoreKind, String), String> {
    match key.parse::<GameKey>().map_err(err)? {
        GameKey::Store { source, store_id } => {
            store_kind(source.as_str())?;
            Ok((source, store_id))
        }
        GameKey::Rom(_) => Err(UNSUPPORTED_STORE.to_string()),
    }
}

/// The backend and product id for a store game key.
fn backend_for_key(
    state: &AppState,
    key: &str,
) -> Result<(Arc<dyn StoreBackend>, StoreKind, String), String> {
    let (kind, id) = store_key(key)?;
    let backend = state
        .stores
        .backend(kind)
        .ok_or_else(|| UNSUPPORTED_STORE.to_string())?;
    Ok((backend, kind, id))
}

fn install_service(state: &AppState) -> Result<Arc<InstallService>, String> {
    state.install.as_ref().map_err(Clone::clone).cloned()
}

/// The installed row for a store game key.
async fn store_row(install: &Arc<InstallService>, key: &str) -> Result<InstalledGame, String> {
    let (kind, id) = store_key(key)?;
    let registry = install.registry();
    tokio::task::spawn_blocking(move || registry.find_key(&GameKey::store(kind, id)))
        .await
        .map_err(err)?
        .map_err(err)?
        .ok_or_else(|| grid_core::library::NOT_INSTALLED.to_string())
}

/// What every store job needs from the app.
fn job_ctx(install: &Arc<InstallService>) -> Result<JobCtx, String> {
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let library = grid_core::library::paths::library_root(&config)
        .ok_or_else(|| "Choose a library folder in Settings › Library first.".to_string())?;
    Ok(JobCtx {
        registry: install.registry(),
        library,
        default_compat_tool: config.default_compat_tool,
    })
}

/// Queues `job` (the frontend raises the queued card).
fn enqueue(install: &Arc<InstallService>, job: StoreJob) -> Result<(), String> {
    match install.enqueue_store_job(job).map_err(err)? {
        Some(_) => Ok(()),
        None => Err("It's already in the download queue.".to_string()),
    }
}

/// Refuses to change a store game while a backup copies it.
fn refuse_while_backing_up(
    install: &InstallService,
    kind: StoreKind,
    row: &InstalledGame,
) -> Result<(), String> {
    let key = GameKey::store(kind, row.store_id.clone());
    if install.store_backup_busy(&key) {
        return Err(format!(
            "{} is being backed up. Try again when the backup is done.",
            row.title
        ));
    }
    Ok(())
}

/// Whether the game is running right now.
fn running(state: &AppState, key: &GameKey) -> bool {
    state
        .launch
        .as_ref()
        .is_ok_and(|launch| launch.snapshot().sessions.iter().any(|s| &s.key == key))
}

/// Every store's status.
#[tauri::command]
pub async fn stores_status(state: State<'_, AppState>) -> Result<Vec<StoreStatus>, String> {
    let stores = state.stores.clone();
    tokio::task::spawn_blocking(move || stores.statuses())
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn store_library(
    state: State<'_, AppState>,
    source: String,
) -> Result<StoreLibrary, String> {
    let backend = backend_for(&state, &source)?;
    let updates = state.updates.store_versions(backend.kind());
    tokio::task::spawn_blocking(move || backend.library(&updates))
        .await
        .map_err(err)
}

/// Refreshes the owned list; the result comes as events.
#[tauri::command]
pub async fn store_refresh(state: State<'_, AppState>, source: String) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    tauri::async_runtime::spawn(backend.run_refresh(true));
    Ok(())
}

/// Checks for updates; the result comes as events.
#[tauri::command]
pub async fn store_check_updates(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    backend.check_updates(app);
    Ok(())
}

/// What the login window reports, handed to the store's backend.
struct LoginSink {
    backend: Arc<dyn StoreBackend>,
    app: AppHandle,
}

impl crate::store_auth::LoginSink for LoginSink {
    fn code(&self, code: SecretString) {
        let backend = self.backend.clone();
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            backend.clone().run_finish_login(code).await;
            if backend.connection() == StoreConnection::Connected {
                backend.check_updates(app);
            }
        });
    }
    fn failed(&self) {
        self.backend.login_redirect_failed();
    }
    fn cancelled(&self) {
        self.backend.login_cancelled();
    }
}

fn login_spec(
    state: &AppState,
    backend: &dyn StoreBackend,
    login: crate::store_auth::LoginSpec,
) -> crate::store_auth::LoginWindowSpec {
    crate::store_auth::LoginWindowSpec {
        login,
        source: backend.kind().as_str().to_string(),
        data_dir: state.stores.data_dir().to_path_buf(),
        tv: state.ui_mode.mode() == super::tv::UiMode::Tv,
    }
}

/// Opens the login window (or focuses it); the result is a
/// [`STORE_LOGIN_EVENT`]. The backend prepares the login first
/// (`StoreBackend::prepare_login`: Amazon installs nile and asks it for
/// the per-attempt login URL); an open window is only focused.
#[tauri::command]
pub async fn store_connect(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    let already_open = crate::store_auth::login_window_open(&app);
    let login = if already_open {
        backend.login_spec()
    } else {
        backend.clone().prepare_login().await?
    };
    let spec = login_spec(&state, backend.as_ref(), login);
    let sink = LoginSink {
        backend: backend.clone(),
        app: app.clone(),
    };
    // Windows are built on the main thread.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let build_app = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::store_auth::open_login_window(&build_app, spec, sink));
    })
    .map_err(err)?;
    let result = match rx.await {
        Ok(result) => result.map_err(err),
        Err(e) => Err(err(e)),
    };
    if let Err(e) = &result {
        if !already_open {
            backend.login_window_failed(e);
        }
    }
    result
}

/// Closes the login window silently (its close reports `cancelled`).
#[tauri::command]
pub async fn store_cancel_login(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    if crate::store_auth::login_window_open(&app) {
        crate::store_auth::close_login_window(&app);
    } else {
        backend.login_cancelled();
    }
    Ok(())
}

#[tauri::command]
pub async fn store_disconnect(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    crate::store_auth::close_login_window(&app);
    let kind = backend.kind();
    tokio::task::spawn_blocking(move || backend.disconnect())
        .await
        .map_err(err)??;
    state.updates.clear_store_updates(&app, &state.stores, kind);
    Ok(())
}

#[tauri::command]
pub async fn store_install_tool(state: State<'_, AppState>, source: String) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    tauri::async_runtime::spawn(backend.run_install_tool());
    Ok(())
}

/// Types `text` into the login page (TV mode's on-screen keyboard). Key
/// words move focus or press a key (`store_auth::KEY_*`: `"\t"` Tab,
/// `"\u{1b}[Z"` Shift-Tab, `"\n"` Enter, `"\u{8}"` Backspace). The text is
/// never logged.
#[tauri::command]
pub async fn store_login_type(app: AppHandle, text: String) -> Result<(), String> {
    if crate::store_auth::type_into_login_window(&app, &text) {
        Ok(())
    } else {
        Err("The sign-in window isn't open.".to_string())
    }
}

#[tauri::command]
pub async fn store_set_install_dir(
    state: State<'_, AppState>,
    source: String,
    path: String,
) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    tokio::task::spawn_blocking(move || backend.set_install_dir(&path))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn store_set_cloud_saves(
    state: State<'_, AppState>,
    source: String,
    on: bool,
) -> Result<(), String> {
    let backend = backend_for(&state, &source)?;
    tokio::task::spawn_blocking(move || backend.set_cloud_saves(on))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn store_install_options(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreInstallOptions, String> {
    let (backend, _, id) = backend_for_key(&state, &key)?;
    backend.install_options(id).await
}

#[tauri::command]
pub async fn store_install(
    state: State<'_, AppState>,
    request: StoreInstallRequest,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, _, id) = backend_for_key(&state, &request.key)?;
    let install = install_service(&state)?;
    backend.ensure_can_install()?;
    let ctx = job_ctx(&install)?;
    let job = backend.install_job(id, request, ctx).await?;
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_update(state: State<'_, AppState>, key: String) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, kind, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    backend.ensure_can_install()?;
    let row = store_row(&install, &key).await?;
    refuse_while_backing_up(&install, kind, &row)?;
    if running(&state, &GameKey::store(kind, row.store_id.clone())) {
        return Err(format!("Quit {} before updating it.", row.title));
    }
    let job = backend.update_job(&row, job_ctx(&install)?);
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_repair(state: State<'_, AppState>, key: String) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, kind, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    backend.ensure_can_install()?;
    let row = store_row(&install, &key).await?;
    refuse_while_backing_up(&install, kind, &row)?;
    if running(&state, &GameKey::store(kind, row.store_id.clone())) {
        return Err(format!("Quit {} before verifying its files.", row.title));
    }
    let job = backend.repair_job(&row, job_ctx(&install)?);
    enqueue(&install, job)
}

/// Why a setup-steps job can't be queued now, `None` when it can: the
/// game is running, another job for it is queued or running, or a backup
/// is copying it.
fn setup_steps_refusal(
    title: &str,
    running: bool,
    job_live: bool,
    backing_up: bool,
) -> Option<String> {
    let title = title.trim();
    if running {
        return Some(format!("Quit {title} before running its setup steps."));
    }
    if job_live {
        return Some(format!(
            "{title} is already in the download queue. Try again when that's done."
        ));
    }
    if backing_up {
        return Some(format!(
            "{title} is being backed up. Try again when the backup is done."
        ));
    }
    None
}

/// Runs only an installed store game's setup steps (Retry on a Downloads
/// row whose warning came from them): a `store_setup` row, no download, no
/// login.
#[tauri::command]
pub async fn store_setup_steps(state: State<'_, AppState>, key: String) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, kind, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    let game_key = GameKey::store(kind, row.store_id.clone());
    if let Some(refusal) = setup_steps_refusal(
        &row.title,
        running(&state, &game_key),
        install.store_job_live(&game_key),
        install.store_backup_busy(&game_key),
    ) {
        return Err(refusal);
    }
    let job = backend.setup_steps_job(&row, job_ctx(&install)?)?;
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_uninstall(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, kind, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    let game_key = GameKey::store(kind, row.store_id.clone());
    if running(&state, &game_key) {
        return Err(format!("Quit {} before uninstalling it.", row.title));
    }
    refuse_while_backing_up(&install, kind, &row)?;
    let registry = install.registry();
    let id = row.store_id.clone();
    let blocking = backend.clone();
    tokio::task::spawn_blocking(move || blocking.uninstall(&registry, &id))
        .await
        .map_err(err)?
        .map_err(err)?;
    state
        .updates
        .forget_store_update(&app, &state.stores, kind, &row.store_id);
    let _ = app.emit(crate::relink_service::INSTALLED_CHANGED_EVENT, 1usize);
    backend.changed();
    Ok(())
}

#[tauri::command]
pub async fn store_install_dlc(
    state: State<'_, AppState>,
    key: String,
    dlc_id: String,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, _, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    backend.ensure_can_install()?;
    let row = store_row(&install, &key).await?;
    let ctx = job_ctx(&install)?;
    let job = backend.dlc_job(row, dlc_id, ctx).await?;
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_remove_dlc(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    dlc_id: String,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let (backend, _, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    let registry = install.registry();
    tokio::task::spawn_blocking(move || backend.remove_dlc(&registry, &row, &dlc_id))
        .await
        .map_err(err)?
        .map_err(err)?;
    let _ = app.emit(crate::relink_service::INSTALLED_CHANGED_EVENT, 1usize);
    Ok(())
}

#[tauri::command]
pub async fn store_game_settings(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreGameSettings, String> {
    let (backend, _, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    Ok(backend.game_settings(&row))
}

/// The executables in an installed store game's folder that Game Settings ›
/// Executable lists, best first, as full paths
/// (`native::store_executables`: a few folders deep, the Wine prefix,
/// uninstallers, installers and redistributables left out).
#[tauri::command]
pub async fn store_game_executables(
    state: State<'_, AppState>,
    key: String,
) -> Result<Vec<String>, String> {
    let (backend, _, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    let windows = backend.windows_version(&row);
    let dir = [row.native_game_dir.trim(), row.extracted_dir.trim()]
        .into_iter()
        .find(|dir| !dir.is_empty())
        .map(PathBuf::from);
    let Some(dir) = dir else {
        return Ok(Vec::new());
    };
    let skip: Vec<PathBuf> = Some(row.native_wineprefix.trim())
        .filter(|prefix| !prefix.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .collect();
    let title = row.title.clone();
    tokio::task::spawn_blocking(move || {
        grid_core::library::specials::native::store_executables(&dir, &title, windows, &skip)
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    })
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn store_set_game_settings(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    settings: StoreGameSettings,
) -> Result<(), String> {
    let (backend, _, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    let registry = install.registry();
    let updated = backend.apply_game_settings(row, &settings);
    tokio::task::spawn_blocking(move || registry.upsert_store(&updated))
        .await
        .map_err(err)?
        .map_err(err)?;
    let _ = app.emit(crate::relink_service::INSTALLED_CHANGED_EVENT, 1usize);
    backend.changed();
    Ok(())
}

#[tauri::command]
pub async fn store_cloud_state(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreCloudState, String> {
    let (backend, _, _) = backend_for_key(&state, &key)?;
    let install = install_service(&state)?;
    let row = store_row(&install, &key).await?;
    backend.cloud_state(&row).await
}

/// Syncs one game's saves in `mode` and answers the new state.
async fn sync_now(state: &AppState, key: &str, mode: SyncMode) -> Result<StoreCloudState, String> {
    let (backend, kind, _) = backend_for_key(state, key)?;
    let install = install_service(state)?;
    let row = store_row(&install, key).await?;
    if backend.cloud_enabled(&row) && running(state, &GameKey::store(kind, row.store_id.clone())) {
        return Err(format!(
            "Quit {} before syncing its cloud saves.",
            row.title
        ));
    }
    let registry = install.registry();
    backend.cloud_sync(&registry, &row, mode).await
}

#[tauri::command]
pub async fn store_cloud_sync(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreCloudState, String> {
    sync_now(&state, &key, SyncMode::Auto).await
}

#[tauri::command]
pub async fn store_cloud_resolve(
    state: State<'_, AppState>,
    key: String,
    choice: CloudChoice,
) -> Result<StoreCloudState, String> {
    let mode = match choice {
        CloudChoice::Upload => SyncMode::PreferUpload,
        CloudChoice::Download => SyncMode::PreferDownload,
    };
    sync_now(&state, &key, mode).await
}

/// An `offline::Inspection` as the UI sees it.
fn inspection_ipc(insp: &offline::Inspection, product: Option<&GogProduct>) -> InstallerInspection {
    let kind = match insp.kind {
        offline::InstallerKind::LinuxSh => "linux_sh",
        offline::InstallerKind::WindowsExe { .. } => "windows_exe",
        offline::InstallerKind::MacPkg => "mac_pkg",
    };
    let key = insp
        .info
        .as_ref()
        .filter(|info| !info.game_id.trim().is_empty())
        .map(|info| GameKey::store(StoreKind::Gog, info.game_id.clone()).to_string());
    let title = product
        .map(|p| p.title.clone())
        .filter(|t| !t.is_empty())
        .or_else(|| insp.info.as_ref().map(|info| info.name.clone()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            insp.path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    let verdict = match &insp.verdict {
        offline::Verdict::Install => InstallerVerdict::Install,
        offline::Verdict::Update { installed_version } => InstallerVerdict::Update {
            installed_version: installed_version.clone(),
        },
        offline::Verdict::Dlc { base_title } => InstallerVerdict::Dlc {
            base_title: base_title.clone(),
        },
        offline::Verdict::Refused { title, body } => InstallerVerdict::Refused {
            title: title.clone(),
            body: body.clone(),
        },
    };
    InstallerInspection {
        path: insp.path.to_string_lossy().into_owned(),
        kind: kind.to_string(),
        key,
        title,
        cover: product.map(|p| p.cover.clone()).unwrap_or_default(),
        version: insp.version.clone(),
        bytes: insp.bytes,
        verdict,
        free_bytes: None,
    }
}

// --- Steam (Settings › Stores › Steam, and Steam's own links) ----------------

/// Tests a SteamID64 and a key (`key: None` tests the saved key) with one
/// `GetOwnedGames` call. The key is wrapped on the first line and never
/// read again here.
#[tauri::command]
pub async fn steam_test(
    state: State<'_, AppState>,
    steam_id: String,
    key: Option<String>,
) -> Result<SteamTestResult, String> {
    let key = key.map(SecretString::from);
    state.stores.steam.test(&steam_id, key).await
}

/// The test, then (on `ok` or `private`) the key into the keyring when
/// given, `[stores.steam]`, a scan and a refresh.
#[tauri::command]
pub async fn steam_save(
    state: State<'_, AppState>,
    steam_id: String,
    key: Option<String>,
) -> Result<SteamTestResult, String> {
    let key = key.map(SecretString::from);
    state.stores.steam.save(&steam_id, key).await
}

/// Clears the key's keyring slot; Steam stays connected with no key.
#[tauri::command]
pub async fn steam_remove_key(state: State<'_, AppState>) -> Result<(), String> {
    let steam = state.stores.steam.clone();
    tokio::task::spawn_blocking(move || steam.remove_key())
        .await
        .map_err(err)?
}

/// Finds the Steam client again and scans.
#[tauri::command]
pub async fn steam_recheck_client(state: State<'_, AppState>) -> Result<(), String> {
    let steam = state.stores.steam.clone();
    steam.recheck_client().await
}

/// Hands the Steam client an install, uninstall or validate link for a
/// Steam game key.
#[tauri::command]
pub async fn steam_open(
    state: State<'_, AppState>,
    key: String,
    action: SteamOpenAction,
) -> Result<(), String> {
    let (kind, id) = store_key(&key)?;
    if kind != StoreKind::Steam {
        return Err(UNSUPPORTED_STORE.to_string());
    }
    let steam = state.stores.steam.clone();
    tokio::task::spawn_blocking(move || steam.open(&id, action))
        .await
        .map_err(err)?
}

/// Reads an installer file.
async fn inspect(state: &AppState, path: &str) -> Result<offline::Inspection, String> {
    let install = install_service(state)?;
    let registry = install.registry();
    let tools_dir = state
        .stores
        .data_dir()
        .join(grid_core::stores::paths::STORES_DIR);
    let path = PathBuf::from(path.trim());
    tokio::task::spawn_blocking(move || offline::inspect(&path, &registry, &tools_dir))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub async fn offline_inspect(
    state: State<'_, AppState>,
    path: String,
) -> Result<InstallerInspection, String> {
    let insp = inspect(&state, &path).await?;
    let product = insp
        .info
        .as_ref()
        .and_then(|info| state.stores.gog.product(&info.game_id));
    let mut ipc = inspection_ipc(&insp, product.as_ref());
    if !matches!(insp.verdict, offline::Verdict::Refused { .. }) {
        let registry = install_service(&state)?.registry();
        let base = state.stores.gog.install_base_dir();
        ipc.free_bytes = tokio::task::spawn_blocking(move || {
            let fresh = if base.is_empty() {
                PathBuf::new()
            } else {
                PathBuf::from(&base).join(grid_core::library::paths::sanitize_component(
                    &insp.title(),
                    "game",
                ))
            };
            let target = offline::target_dir(&insp, &registry, &fresh);
            if target.as_os_str().is_empty() {
                None
            } else {
                grid_core::library::relocate::free_space(&target)
            }
        })
        .await
        .map_err(err)?;
    }
    Ok(ipc)
}

/// Free bytes on the volume holding `path` (its nearest existing folder),
/// for a space line after the install folder changes; `None` when unknown.
#[tauri::command]
pub async fn store_free_space(path: String) -> Result<Option<u64>, String> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    tokio::task::spawn_blocking(move || grid_core::library::relocate::free_space(&path))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn offline_install(
    state: State<'_, AppState>,
    path: String,
    compat_tool: String,
    install_dir: Option<String>,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = install_service(&state)?;
    let insp = inspect(&state, &path).await?;
    if let offline::Verdict::Refused { body, .. } = &insp.verdict {
        return Err(body.clone());
    }
    let opts = offline::OfflineOpts {
        install_dir: install_dir
            .map(|dir| dir.trim().to_string())
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from),
        compat_tool,
    };
    let job = offline::install_job(
        insp,
        opts,
        job_ctx(&install)?.gog(),
        install.wizard_allowed(),
    );
    enqueue(&install, job)
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
    fn setup_steps_wait_for_the_game_its_jobs_and_its_backup() {
        assert_eq!(
            setup_steps_refusal("Kettle Drum", false, false, false),
            None
        );
        assert_eq!(
            setup_steps_refusal("Kettle Drum", true, true, true).unwrap(),
            "Quit Kettle Drum before running its setup steps."
        );
        assert_eq!(
            setup_steps_refusal("Kettle Drum", false, true, true).unwrap(),
            "Kettle Drum is already in the download queue. Try again when that's done."
        );
        assert_eq!(
            setup_steps_refusal(" Kettle Drum ", false, false, true).unwrap(),
            "Kettle Drum is being backed up. Try again when the backup is done."
        );
    }

    #[test]
    fn keys_and_sources_route_to_gog_epic_and_amazon() {
        assert_eq!(
            store_key("gog:1207664643").unwrap(),
            (StoreKind::Gog, "1207664643".to_string())
        );
        assert_eq!(
            store_key("epic:Fortnite").unwrap(),
            (StoreKind::Epic, "Fortnite".to_string())
        );
        assert_eq!(
            store_key("steam:620").unwrap(),
            (StoreKind::Steam, "620".to_string())
        );
        assert_eq!(
            store_key("amazon:amzn1.adg.product.1:x").unwrap(),
            (StoreKind::Amazon, "amzn1.adg.product.1:x".to_string())
        );
        assert_eq!(store_kind("amazon").unwrap(), StoreKind::Amazon);
        assert!(store_key("romm:5").is_err());
        assert!(store_key("nonsense").is_err());
        assert_eq!(store_kind("gog").unwrap(), StoreKind::Gog);
        assert_eq!(store_kind(" epic ").unwrap(), StoreKind::Epic);
        assert_eq!(store_kind("steam").unwrap(), StoreKind::Steam);
        assert_eq!(store_kind("origin").unwrap_err(), UNSUPPORTED_STORE);
        assert!(store_kind("romm").is_err());
    }

    #[test]
    fn steam_ipc_types_serialize_as_api_ts_says() {
        assert_eq!(
            serde_json::to_value(SteamTestResult::Ok { owned_count: 4 }).unwrap(),
            serde_json::json!({"kind": "ok", "owned_count": 4})
        );
        assert_eq!(
            serde_json::to_value(SteamTestResult::BadKey { status: 401 }).unwrap(),
            serde_json::json!({"kind": "bad_key", "status": 401})
        );
        assert_eq!(
            serde_json::to_value(SteamTestResult::BadSteamId).unwrap(),
            serde_json::json!({"kind": "bad_steam_id"})
        );
        assert_eq!(
            serde_json::to_value(SteamKeyState::Private).unwrap(),
            serde_json::json!("private")
        );
        let action: SteamOpenAction =
            serde_json::from_value(serde_json::json!("validate")).unwrap();
        assert_eq!(action, SteamOpenAction::Validate);
    }

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
