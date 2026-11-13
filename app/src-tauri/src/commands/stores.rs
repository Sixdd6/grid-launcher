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
//! The five backup commands at the end (`plan_store_backup` …
//! `remove_store_from_drive`) are package C's and answer "not implemented"
//! until it lands.

use std::path::PathBuf;
use std::sync::Arc;

use grid_core::backup::plan::BackupQueued;
use grid_core::config::Config;
use grid_core::library::queue::StoreJob;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::stores::gog::meta::{GogMeta, GogPlatform};
use grid_core::stores::gog::{CloudState, GogInstallRequest, GogJobCtx, GogProduct, SyncMode};
use grid_core::stores::offline;
use grid_core::stores::{GameKey, StoreKind};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use super::{ensure_library_idle, err, AppState};
use crate::stores_service::{cloud_state_ipc, StoresService};

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

/// The one store Phase 1 ships.
fn require_gog(source: &str) -> Result<(), String> {
    match StoreKind::parse(source.trim()) {
        Some(StoreKind::Gog) => Ok(()),
        _ => Err("GRID doesn't support that store yet.".to_string()),
    }
}

/// A GOG game key's product id.
fn gog_id(key: &str) -> Result<String, String> {
    match key.parse::<GameKey>().map_err(err)? {
        GameKey::Store {
            source: StoreKind::Gog,
            store_id,
        } => Ok(store_id),
        _ => Err("GRID doesn't support that store yet.".to_string()),
    }
}

fn install_service(state: &AppState) -> Result<Arc<InstallService>, String> {
    state.install.as_ref().map_err(Clone::clone).cloned()
}

/// The installed row for a GOG key.
async fn gog_row(install: &Arc<InstallService>, key: &str) -> Result<InstalledGame, String> {
    let id = gog_id(key)?;
    let registry = install.registry();
    tokio::task::spawn_blocking(move || registry.find_key(&GameKey::store(StoreKind::Gog, id)))
        .await
        .map_err(err)?
        .map_err(err)?
        .ok_or_else(|| grid_core::library::NOT_INSTALLED.to_string())
}

/// What every GOG job needs from the app.
fn job_ctx(install: &Arc<InstallService>) -> Result<GogJobCtx, String> {
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let library = grid_core::library::paths::library_root(&config)
        .ok_or_else(|| "Choose a library folder in Settings › Library first.".to_string())?;
    Ok(GogJobCtx {
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

/// Whether the game is running right now.
fn running(state: &AppState, key: &GameKey) -> bool {
    state
        .launch
        .as_ref()
        .is_ok_and(|launch| launch.snapshot().sessions.iter().any(|s| &s.key == key))
}

/// Every store's status (GOG only in Phase 1).
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
    require_gog(&source)?;
    let stores = state.stores.clone();
    let updates = state.updates.store_updates();
    tokio::task::spawn_blocking(move || stores.library(&updates))
        .await
        .map_err(err)
}

/// Refreshes the owned list; the result comes as events.
#[tauri::command]
pub async fn store_refresh(state: State<'_, AppState>, source: String) -> Result<(), String> {
    require_gog(&source)?;
    let stores = state.stores.clone();
    tauri::async_runtime::spawn(async move { stores.refresh_owned(true).await });
    Ok(())
}

/// Checks for updates; the result comes as events.
#[tauri::command]
pub async fn store_check_updates(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    require_gog(&source)?;
    state.updates.spawn_store_check(app, state.stores.clone());
    Ok(())
}

/// What the login window reports, handed to the stores service.
struct LoginSink {
    stores: Arc<StoresService>,
    app: AppHandle,
}

impl crate::store_auth::LoginSink for LoginSink {
    fn code(&self, code: SecretString) {
        let stores = self.stores.clone();
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            stores.finish_login(code).await;
            if stores.connection() == StoreConnection::Connected {
                app.state::<AppState>()
                    .updates
                    .spawn_store_check(app.clone(), stores.clone());
            }
        });
    }
    fn failed(&self) {
        self.stores.login_redirect_failed();
    }
    fn cancelled(&self) {
        self.stores.login_cancelled();
    }
}

fn login_spec(state: &AppState, stores: &StoresService) -> crate::store_auth::LoginWindowSpec {
    crate::store_auth::LoginWindowSpec {
        store_name: StoreKind::Gog.platform_name().to_string(),
        source: StoreKind::Gog.as_str().to_string(),
        login_url: stores.api().login_url(),
        embed: stores.embed_url().clone(),
        data_dir: stores.data_dir().to_path_buf(),
        tv: state.ui_mode.mode() == super::tv::UiMode::Tv,
    }
}

/// Opens the login window (or focuses it); the result is a
/// [`STORE_LOGIN_EVENT`].
#[tauri::command]
pub async fn store_connect(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    require_gog(&source)?;
    let stores = state.stores.clone();
    let already_open = crate::store_auth::login_window_open(&app);
    if !already_open {
        stores.begin_login()?;
    }
    let spec = login_spec(&state, &stores);
    let sink = LoginSink {
        stores: stores.clone(),
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
            stores.login_window_failed(e);
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
    require_gog(&source)?;
    if crate::store_auth::login_window_open(&app) {
        crate::store_auth::close_login_window(&app);
    } else {
        state.stores.login_cancelled();
    }
    Ok(())
}

#[tauri::command]
pub async fn store_disconnect(
    state: State<'_, AppState>,
    app: AppHandle,
    source: String,
) -> Result<(), String> {
    require_gog(&source)?;
    crate::store_auth::close_login_window(&app);
    let stores = state.stores.clone();
    tokio::task::spawn_blocking(move || stores.disconnect())
        .await
        .map_err(err)??;
    state.updates.clear_store_updates(&app, &state.stores);
    Ok(())
}

#[tauri::command]
pub async fn store_install_tool(state: State<'_, AppState>, source: String) -> Result<(), String> {
    require_gog(&source)?;
    let stores = state.stores.clone();
    tauri::async_runtime::spawn(async move { stores.install_tool().await });
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
    require_gog(&source)?;
    let stores = state.stores.clone();
    tokio::task::spawn_blocking(move || stores.set_install_dir(&path))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn store_set_cloud_saves(
    state: State<'_, AppState>,
    source: String,
    on: bool,
) -> Result<(), String> {
    require_gog(&source)?;
    let stores = state.stores.clone();
    tokio::task::spawn_blocking(move || stores.set_cloud_saves(on))
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn store_install_options(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreInstallOptions, String> {
    let id = gog_id(&key)?;
    let stores = state.stores.clone();
    stores.ensure_can_install()?;
    stores.ensure_owned_fetched().await;
    let options = stores
        .api()
        .install_options(&id)
        .await
        .map_err(|e| stores.store_error(e))?;
    let title = stores.product(&id).map(|p| p.title).unwrap_or(id);
    let compat = Config::load(&Config::default_path())
        .map(|config| config.default_compat_tool)
        .unwrap_or_default();
    let base = stores.install_base_dir();
    tokio::task::spawn_blocking(move || {
        crate::stores_service::install_options_ipc(&options, &title, &base, &compat)
    })
    .await
    .map_err(err)
}

fn parse_platform(platform: &str) -> Result<GogPlatform, String> {
    match platform {
        "linux" => Ok(GogPlatform::Linux),
        "windows" => Ok(GogPlatform::Windows),
        _ => Err("GRID installs the Linux or Windows version only.".to_string()),
    }
}

#[tauri::command]
pub async fn store_install(
    state: State<'_, AppState>,
    request: StoreInstallRequest,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let id = gog_id(&request.key)?;
    let install = install_service(&state)?;
    let stores = state.stores.clone();
    stores.ensure_can_install()?;
    let platform = parse_platform(&request.platform)?;
    stores.ensure_owned_fetched().await;
    let title = stores
        .product(&id)
        .map(|p| p.title)
        .unwrap_or_else(|| id.clone());
    let install_dir = if request.install_dir.trim().is_empty() {
        let base = stores.install_base_dir();
        if base.is_empty() {
            return Err("Choose a library folder in Settings › Library first.".to_string());
        }
        std::path::Path::new(&base).join(grid_core::library::paths::sanitize_component(
            &title, "game",
        ))
    } else {
        PathBuf::from(request.install_dir.trim())
    };
    let ctx = job_ctx(&install)?;
    let job = stores.api().install_job(
        GogInstallRequest {
            store_id: id,
            title: title.clone(),
            platform,
            language: request.language,
            dlc_ids: request.dlc_ids,
            install_dir,
            compat_tool: request.compat_tool,
        },
        ctx,
    );
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_update(state: State<'_, AppState>, key: String) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = install_service(&state)?;
    state.stores.ensure_can_install()?;
    let row = gog_row(&install, &key).await?;
    let job = state.stores.api().update_job(&row, job_ctx(&install)?);
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_repair(state: State<'_, AppState>, key: String) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = install_service(&state)?;
    state.stores.ensure_can_install()?;
    let row = gog_row(&install, &key).await?;
    let job = state.stores.api().repair_job(&row, job_ctx(&install)?);
    enqueue(&install, job)
}

#[tauri::command]
pub async fn store_uninstall(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = install_service(&state)?;
    let row = gog_row(&install, &key).await?;
    let game_key = GameKey::store(StoreKind::Gog, row.store_id.clone());
    if running(&state, &game_key) {
        return Err(format!("Quit {} before uninstalling it.", row.title));
    }
    let stores = state.stores.clone();
    let registry = install.registry();
    let id = row.store_id.clone();
    let api = stores.api().clone();
    tokio::task::spawn_blocking(move || api.uninstall(&registry, &id))
        .await
        .map_err(err)?
        .map_err(err)?;
    state
        .updates
        .forget_store_update(&app, &stores, &row.store_id);
    let _ = app.emit(crate::relink_service::INSTALLED_CHANGED_EVENT, 1usize);
    stores.changed();
    Ok(())
}

#[tauri::command]
pub async fn store_install_dlc(
    state: State<'_, AppState>,
    key: String,
    dlc_id: String,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = install_service(&state)?;
    state.stores.ensure_can_install()?;
    state.stores.ensure_owned_fetched().await;
    let row = gog_row(&install, &key).await?;
    let job = state
        .stores
        .api()
        .dlc_job(&row, &dlc_id, job_ctx(&install)?);
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
    let install = install_service(&state)?;
    let row = gog_row(&install, &key).await?;
    let api = state.stores.api().clone();
    let registry = install.registry();
    let blocking_row = row.clone();
    tokio::task::spawn_blocking(move || api.remove_dlc(&registry, &blocking_row, &dlc_id))
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
    let install = install_service(&state)?;
    let row = gog_row(&install, &key).await?;
    let meta = GogMeta::from_json(&row.store_meta);
    Ok(StoreGameSettings {
        install_dir: row.native_game_dir.clone(),
        executable: row.native_executable_path.clone(),
        exe_user_pinned: meta.exe_user_pinned,
        launch_parameters: row.native_launch_parameters.clone(),
        compat_tool: row.native_compat_tool.clone(),
        wineprefix: row.native_wineprefix.clone(),
        cloud_saves: meta.cloud_saves,
    })
}

/// The row after Game settings' Save: a new executable becomes the user's
/// choice (GOG updates leave it alone); the install folder and the Wine
/// prefix are read-only, and only a Windows version takes a compat tool.
fn apply_game_settings(mut row: InstalledGame, settings: &StoreGameSettings) -> InstalledGame {
    let mut meta = GogMeta::from_json(&row.store_meta);
    let executable = settings.executable.trim();
    if !executable.is_empty() && executable != row.native_executable_path {
        row.native_executable_path = executable.to_string();
        meta.exe_user_pinned = true;
    }
    if meta.exe_user_pinned {
        row.native_launch_source = "user".to_string();
    }
    row.native_launch_parameters = settings.launch_parameters.trim().to_string();
    if meta.platform == GogPlatform::Windows {
        row.native_compat_tool = settings.compat_tool.trim().to_string();
    }
    meta.cloud_saves = settings.cloud_saves;
    row.store_meta = meta.to_json();
    row
}

#[tauri::command]
pub async fn store_set_game_settings(
    state: State<'_, AppState>,
    app: AppHandle,
    key: String,
    settings: StoreGameSettings,
) -> Result<(), String> {
    let install = install_service(&state)?;
    let row = gog_row(&install, &key).await?;
    let registry = install.registry();
    let updated = apply_game_settings(row, &settings);
    tokio::task::spawn_blocking(move || registry.upsert_store(&updated))
        .await
        .map_err(err)?
        .map_err(err)?;
    let _ = app.emit(crate::relink_service::INSTALLED_CHANGED_EVENT, 1usize);
    state.stores.changed();
    Ok(())
}

#[tauri::command]
pub async fn store_cloud_state(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreCloudState, String> {
    let install = install_service(&state)?;
    let row = gog_row(&install, &key).await?;
    let stores = state.stores.clone();
    let meta = GogMeta::from_json(&row.store_meta);
    if !stores.cloud_enabled(&row) {
        return Ok(cloud_state_ipc(&CloudState::Off, &meta, false));
    }
    if stores.is_syncing(&row.store_id) {
        return Ok(cloud_state_ipc(
            &CloudState::Synced { at: meta.last_sync },
            &meta,
            true,
        ));
    }
    let cloud = stores
        .api()
        .cloud_state(&row)
        .await
        .map_err(|e| stores.store_error(e))?;
    stores.remember_cloud(&row.store_id, &cloud);
    Ok(cloud_state_ipc(&cloud, &meta, false))
}

/// Syncs one game's saves in `mode`; answers the row (re-read, its
/// `last_sync` moved) and the new state.
async fn sync_now(
    state: &AppState,
    key: &str,
    mode: SyncMode,
) -> Result<(InstalledGame, CloudState), String> {
    let install = install_service(state)?;
    let row = gog_row(&install, key).await?;
    let stores = state.stores.clone();
    if !stores.cloud_enabled(&row) {
        return Ok((row, CloudState::Off));
    }
    if running(state, &GameKey::store(StoreKind::Gog, row.store_id.clone())) {
        return Err(format!(
            "Quit {} before syncing its cloud saves.",
            row.title
        ));
    }
    let registry = install.registry();
    stores.set_syncing(&row.store_id, true);
    stores.changed();
    let result = stores.api().cloud_sync(&registry, &row, mode).await;
    stores.set_syncing(&row.store_id, false);
    let cloud = result.map_err(|e| stores.store_error(e));
    if let Ok(cloud) = &cloud {
        stores.remember_cloud(&row.store_id, cloud);
    }
    stores.changed();
    let row = gog_row(&install, key).await.unwrap_or(row);
    Ok((row, cloud?))
}

#[tauri::command]
pub async fn store_cloud_sync(
    state: State<'_, AppState>,
    key: String,
) -> Result<StoreCloudState, String> {
    let (row, cloud) = sync_now(&state, &key, SyncMode::Auto).await?;
    Ok(cloud_state_ipc(
        &cloud,
        &GogMeta::from_json(&row.store_meta),
        false,
    ))
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
    let (row, cloud) = sync_now(&state, &key, mode).await?;
    Ok(cloud_state_ipc(
        &cloud,
        &GogMeta::from_json(&row.store_meta),
        false,
    ))
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
    }
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
        .and_then(|info| state.stores.product(&info.game_id));
    Ok(inspection_ipc(&insp, product.as_ref()))
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
    let job = offline::install_job(insp, opts, job_ctx(&install)?, install.wizard_allowed());
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
    fn keys_and_sources_are_gog_only() {
        assert_eq!(gog_id("gog:1207664643").unwrap(), "1207664643");
        assert!(gog_id("epic:Fortnite").is_err());
        assert!(gog_id("romm:5").is_err());
        assert!(gog_id("nonsense").is_err());
        assert!(require_gog("gog").is_ok());
        assert!(require_gog("steam").is_err());
        assert_eq!(parse_platform("linux").unwrap(), GogPlatform::Linux);
        assert!(parse_platform("mac").is_err());
    }

    #[test]
    fn game_settings_pin_a_new_executable_and_keep_the_folder() {
        let meta = GogMeta {
            platform: GogPlatform::Windows,
            cloud_saves: true,
            ..GogMeta::default()
        };
        let row = InstalledGame {
            native_game_dir: "/g/Ashen".into(),
            native_executable_path: "/g/Ashen/game.exe".into(),
            native_launch_source: "sidecar".into(),
            native_wineprefix: "/p".into(),
            store_meta: meta.to_json(),
            ..Default::default()
        };
        let mut settings = StoreGameSettings {
            install_dir: "/elsewhere".into(),
            executable: "/g/Ashen/game.exe".into(),
            exe_user_pinned: false,
            launch_parameters: " -windowed ".into(),
            compat_tool: "GE-Proton9-20".into(),
            wineprefix: "/other".into(),
            cloud_saves: false,
        };
        let same = apply_game_settings(row.clone(), &settings);
        assert_eq!(same.native_launch_source, "sidecar");
        assert_eq!(same.native_launch_parameters, "-windowed");
        assert_eq!(same.native_compat_tool, "GE-Proton9-20");
        assert_eq!(same.native_game_dir, "/g/Ashen");
        assert_eq!(same.native_wineprefix, "/p");
        assert!(!GogMeta::from_json(&same.store_meta).cloud_saves);

        settings.executable = "/g/Ashen/launcher.exe".into();
        let pinned = apply_game_settings(row, &settings);
        assert_eq!(pinned.native_executable_path, "/g/Ashen/launcher.exe");
        assert_eq!(pinned.native_launch_source, "user");
        assert!(GogMeta::from_json(&pinned.store_meta).exe_user_pinned);
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
