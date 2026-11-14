//! Epic Games Store in the app: the Epic [`StoreBackend`].
//!
//! Step 0 of `docs/stores-phase2-plan.md`: the backend exists so the
//! commands route Epic calls through the same trait as GOG, but every
//! store action fails with grid-core's `not implemented` until package C
//! fills it in (status, owned cache and refresh, notices, install guards,
//! cloud hooks, the launch hook through `EpicService::launch_prep`). Its
//! status always reads "not connected", so the frontend (which hides Epic
//! with `ENABLED_STORES = ['gog']` anyway) shows nothing half-done.
//!
//! What is real already: the login spec (Epic's page and its launcher
//! redirect), the row-level Game Settings, the art check, and the
//! [`StoreLaunchHook`] that refuses to start an Epic row without a launch
//! ticket.
//!
//! Secrets: nothing here logs a token, a code or a store tool's output.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use grid_core::config::{Config, EpicConfig};
use grid_core::launch::{LaunchError, StoreLaunchHook, StoreLaunchPrep};
use grid_core::library::queue::{DownloadsSnapshot, StoreJob};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::epic::meta::EpicMeta;
use grid_core::stores::epic::{EpicEndpoints, EpicService, EPIC_CODE_PARAM, EPIC_LOGIN_USER_AGENT};
use grid_core::stores::tools::{self, StoreTool};
use grid_core::stores::{GameSource, StoreError, StoreKind, SyncMode};
use secrecy::SecretString;

use super::{BoxFut, JobCtx, StoreBackend, StoreShared};
use crate::commands::stores::{
    OwnedState, StoreCloudState, StoreConnection, StoreGameSettings, StoreInstallOptions,
    StoreInstallRequest, StoreLibrary, StoreSaveConflict, StoreStatus, ToolState, ToolStatus,
};
use crate::store_auth::LoginSpec;

/// The Epic endpoints for this run: Epic's own, or (e2e builds only)
/// `GRID_LAUNCHER_E2E_EPIC_BASE` for every address.
pub fn epic_endpoints() -> EpicEndpoints {
    #[cfg(feature = "e2e")]
    if let Some(base) = std::env::var(E2E_EPIC_BASE_ENV)
        .ok()
        .and_then(|value| tauri::Url::parse(value.trim()).ok())
    {
        return EpicEndpoints::e2e(base);
    }
    EpicEndpoints::real()
}

/// The e2e build's Epic base URL variable (`e2e/mock-romm/mock-epic.mjs`,
/// package C).
#[cfg(feature = "e2e")]
pub const E2E_EPIC_BASE_ENV: &str = "GRID_LAUNCHER_E2E_EPIC_BASE";

/// The Epic backend (Step 0: stubs).
pub struct EpicStore {
    service: EpicService,
    data_dir: PathBuf,
    config_path: PathBuf,
    shared: Arc<StoreShared>,
}

const SOURCE: &str = "epic";

/// What every not-yet-built Epic action answers.
fn not_implemented() -> String {
    StoreError::not_implemented().to_string()
}

impl EpicStore {
    pub fn new(
        service: EpicService,
        data_dir: PathBuf,
        config_path: PathBuf,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        Arc::new(EpicStore {
            service,
            data_dir,
            config_path,
            shared,
        })
    }

    /// The real Epic backend for this run.
    pub fn real(
        data_dir: PathBuf,
        config_path: PathBuf,
        http: reqwest::Client,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let service = EpicService::new(data_dir.clone(), http, epic_endpoints());
        EpicStore::new(service, data_dir, config_path, shared)
    }

    fn epic_config(&self) -> EpicConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.epic)
            .unwrap_or_default()
    }

    fn installed_rows(&self) -> Vec<InstalledGame> {
        self.shared
            .registry()
            .and_then(|registry| registry.store_rows(StoreKind::Epic).ok())
            .unwrap_or_default()
    }
}

/// Epic's card while nothing but Step 0 exists: not connected, legendary's
/// pin and whether it is on disk, the installed rows' counts.
fn assemble_status(
    config: &EpicConfig,
    rows: &[InstalledGame],
    tool_installed: bool,
    install_dir: &str,
) -> StoreStatus {
    let pin = tools::pin(StoreTool::Legendary);
    let installed_count = rows.len() as u32;
    StoreStatus {
        source: SOURCE.to_string(),
        connection: StoreConnection::NotConnected,
        username: String::new(),
        tool: ToolStatus {
            name: StoreTool::Legendary.name().to_string(),
            version: pin.version.to_string(),
            state: if tool_installed {
                ToolState::Installed
            } else {
                ToolState::NotInstalled
            },
            size_bytes: tools::host_asset(StoreTool::Legendary).map_or(0, |asset| asset.size),
            error: String::new(),
            license: pin.license.to_string(),
            source_url: pin.source_url.to_string(),
            done: 0,
            total: 0,
        },
        owned: OwnedState::Idle,
        owned_error: String::new(),
        owned_count: 0,
        installed_count,
        installed_bytes: rows
            .iter()
            .map(|row| row.filesize_bytes.max(0) as u64)
            .sum(),
        refreshed_at: 0,
        cloud_saves: config.cloud_saves,
        install_dir: install_dir.to_string(),
        in_rail: installed_count > 0,
        last_error: String::new(),
    }
}

impl StoreBackend for EpicStore {
    fn kind(&self) -> StoreKind {
        StoreKind::Epic
    }
    fn status(&self) -> StoreStatus {
        let config = self.epic_config();
        let install_dir =
            super::default_install_dir(&self.config_path, &config.install_dir, "Epic Games");
        assemble_status(
            &config,
            &self.installed_rows(),
            tools::installed_path(&self.data_dir, StoreTool::Legendary).is_some(),
            &install_dir,
        )
    }
    fn connection(&self) -> StoreConnection {
        StoreConnection::NotConnected
    }
    fn changed(&self) {
        self.shared.changed();
    }
    fn load(&self) {}
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async {})
    }
    fn observe_downloads(&self, _snapshot: &DownloadsSnapshot) {}
    fn login_spec(&self) -> LoginSpec {
        let ep = self.service.endpoints();
        LoginSpec {
            store_name: StoreKind::Epic.platform_name().to_string(),
            login_url: ep.login.clone(),
            finish: ep.finish.clone(),
            code_param: EPIC_CODE_PARAM,
            user_agent: Some(EPIC_LOGIN_USER_AGENT.to_string()),
        }
    }
    fn begin_login(&self) -> Result<(), String> {
        Err(not_implemented())
    }
    fn login_window_failed(&self, _error: &str) {}
    fn login_cancelled(&self) {}
    fn login_redirect_failed(&self) {}
    fn run_finish_login(self: Arc<Self>, code: SecretString) -> BoxFut<'static, ()> {
        drop(code);
        Box::pin(async {})
    }
    fn disconnect(&self) -> Result<(), String> {
        Err(not_implemented())
    }
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async {})
    }
    fn run_refresh(self: Arc<Self>, _announce: bool) -> BoxFut<'static, ()> {
        Box::pin(async {})
    }
    fn library(&self, _updates: &HashMap<String, String>) -> StoreLibrary {
        StoreLibrary {
            status: self.status(),
            installed: Vec::new(),
            available: Vec::new(),
        }
    }
    fn check_updates(self: Arc<Self>, _app: tauri::AppHandle) {}
    fn set_install_dir(&self, _path: &str) {}
    fn set_cloud_saves(&self, _on: bool) {}
    fn ensure_can_install(&self) -> Result<(), String> {
        Err(not_implemented())
    }
    fn install_options(
        self: Arc<Self>,
        _store_id: String,
    ) -> BoxFut<'static, Result<StoreInstallOptions, String>> {
        Box::pin(async { Err(not_implemented()) })
    }
    fn install_job(
        self: Arc<Self>,
        _store_id: String,
        _request: StoreInstallRequest,
        _ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async { Err(not_implemented()) })
    }
    fn update_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.service.update_job(row, ctx.epic())
    }
    fn repair_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.service.repair_job(row, ctx.epic())
    }
    fn dlc_job(
        self: Arc<Self>,
        _row: InstalledGame,
        _dlc_id: String,
        _ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async { Err(not_implemented()) })
    }
    fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc_id: &str,
    ) -> Result<(), StoreError> {
        self.service.remove_dlc(registry, row, dlc_id)
    }
    fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
        self.service.uninstall(registry, store_id)
    }
    fn game_settings(&self, row: &InstalledGame) -> StoreGameSettings {
        let meta = EpicMeta::from_json(&row.store_meta);
        StoreGameSettings {
            install_dir: row.native_game_dir.clone(),
            executable: row.native_executable_path.clone(),
            exe_user_pinned: meta.exe_user_pinned,
            launch_parameters: row.native_launch_parameters.clone(),
            compat_tool: row.native_compat_tool.clone(),
            wineprefix: row.native_wineprefix.clone(),
            cloud_saves: meta.cloud_saves,
        }
    }
    fn apply_game_settings(
        &self,
        mut row: InstalledGame,
        settings: &StoreGameSettings,
    ) -> InstalledGame {
        // Every Epic game is a Windows version: it always takes a compat
        // tool. The executable is Epic's (legendary's launch ticket names
        // it); a different one becomes the user's pin.
        let mut meta = EpicMeta::from_json(&row.store_meta);
        let executable = settings.executable.trim();
        if !executable.is_empty() && executable != row.native_executable_path {
            row.native_executable_path = executable.to_string();
            meta.exe_user_pinned = true;
        }
        if meta.exe_user_pinned {
            row.native_launch_source = "user".to_string();
        }
        row.native_launch_parameters = settings.launch_parameters.trim().to_string();
        row.native_compat_tool = settings.compat_tool.trim().to_string();
        meta.cloud_saves = settings.cloud_saves;
        row.store_meta = meta.to_json();
        row
    }
    fn windows_version(&self, _row: &InstalledGame) -> bool {
        true
    }
    fn cloud_enabled(&self, row: &InstalledGame) -> bool {
        self.epic_config().cloud_saves && EpicMeta::from_json(&row.store_meta).cloud_saves
    }
    fn cloud_state<'a>(
        &'a self,
        _row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<StoreCloudState, String>> {
        Box::pin(async { Err(not_implemented()) })
    }
    fn cloud_sync<'a>(
        &'a self,
        _registry: &'a Registry,
        _row: &'a InstalledGame,
        _mode: SyncMode,
    ) -> BoxFut<'a, Result<StoreCloudState, String>> {
        Box::pin(async { Err(not_implemented()) })
    }
    fn sync_before_launch<'a>(
        &'a self,
        _registry: &'a Registry,
        _row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<(), StoreSaveConflict>> {
        Box::pin(async { Ok(()) })
    }
    fn sync_after_session<'a>(
        &'a self,
        _registry: &'a Registry,
        _row: &'a InstalledGame,
    ) -> BoxFut<'a, ()> {
        Box::pin(async {})
    }
    fn is_art_url(&self, url: &str) -> bool {
        grid_core::stores::art::is_store_art_url(url, &self.service.endpoints().art_origins)
    }
}

/// The launch hook: an Epic row needs legendary's launch ticket (package
/// B), so Step 0 refuses to start one; every other row launches as it is.
impl StoreLaunchHook for EpicStore {
    fn prepare<'a>(
        &'a self,
        game: &'a InstalledGame,
    ) -> BoxFut<'a, Result<Option<StoreLaunchPrep>, LaunchError>> {
        Box::pin(async move {
            if game.source != GameSource::Store(StoreKind::Epic) {
                return Ok(None);
            }
            Err(LaunchError::Validation(format!(
                "Couldn\u{2019}t get a launch ticket from Epic Games: {}",
                not_implemented()
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Arc<EpicStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let service = EpicService::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            EpicEndpoints::e2e(tauri::Url::parse("http://127.0.0.1:4300/").unwrap()),
        );
        let store = EpicStore::new(
            service,
            dir.path().to_path_buf(),
            config_path,
            StoreShared::new(),
        );
        (store, dir)
    }

    #[test]
    fn step_0_epic_is_not_connected_and_refuses_actions() {
        let (store, _dir) = store();
        let status = store.status();
        assert_eq!(status.source, "epic");
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.tool.name, "legendary");
        assert_eq!(status.tool.version, "0.21.1");
        assert_eq!(status.tool.license, "GPL-3.0");
        assert!(!status.in_rail);
        assert_eq!(store.begin_login().unwrap_err(), "not implemented");
        assert_eq!(store.ensure_can_install().unwrap_err(), "not implemented");
        assert!(store.disconnect().is_err());
    }

    #[test]
    fn the_login_spec_is_epics_launcher_redirect() {
        let (store, _dir) = store();
        let spec = store.login_spec();
        assert_eq!(spec.store_name, "Epic Games");
        assert_eq!(
            spec.finish.as_str(),
            "http://127.0.0.1:4300/launcher/authorized"
        );
        assert_eq!(spec.code_param, "code");
        assert_eq!(spec.user_agent.as_deref(), Some(EPIC_LOGIN_USER_AGENT));
        assert!(store.is_art_url("http://127.0.0.1:4300/art/e1.jpg"));
        assert!(store.is_art_url("https://cdn2.unrealengine.com/a.jpg"));
    }

    #[tokio::test]
    async fn the_launch_hook_refuses_epic_rows_only() {
        let (store, _dir) = store();
        let epic = InstalledGame {
            source: GameSource::Store(StoreKind::Epic),
            store_id: "Starlight".into(),
            ..Default::default()
        };
        let error = store.prepare(&epic).await.unwrap_err();
        assert!(error.to_string().contains("launch ticket"), "{error}");
        let gog = InstalledGame {
            source: GameSource::Store(StoreKind::Gog),
            ..Default::default()
        };
        assert!(store.prepare(&gog).await.unwrap().is_none());
    }

    #[test]
    fn game_settings_always_take_a_compat_tool() {
        let (store, _dir) = store();
        let row = InstalledGame {
            native_executable_path: "/g/S/Game.exe".into(),
            store_meta: EpicMeta {
                cloud_saves: true,
                ..Default::default()
            }
            .to_json(),
            ..Default::default()
        };
        let settings = StoreGameSettings {
            install_dir: String::new(),
            executable: "/g/S/Game.exe".into(),
            exe_user_pinned: false,
            launch_parameters: " -dx11 ".into(),
            compat_tool: "GE-Proton9-20".into(),
            wineprefix: String::new(),
            cloud_saves: false,
        };
        let out = store.apply_game_settings(row, &settings);
        assert_eq!(out.native_compat_tool, "GE-Proton9-20");
        assert_eq!(out.native_launch_parameters, "-dx11");
        assert!(!EpicMeta::from_json(&out.store_meta).cloud_saves);
        assert!(!EpicMeta::from_json(&out.store_meta).exe_user_pinned);
        assert!(store.windows_version(&out));
    }
}
