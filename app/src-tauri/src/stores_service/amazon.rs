//! Amazon Games in the app: the Amazon [`StoreBackend`].
//!
//! Step 0 of `docs/stores-phase3-plan.md`: the backend exists so the
//! commands route Amazon calls through the same trait as GOG and Epic, but
//! every store action fails with grid-core's `not implemented` until
//! package C fills it in (status, the owned list from nile's
//! `library.json`, notices, install guards, the login through
//! `prepare_login`, the launch hook through `AmazonService::launch_prep`).
//! Its status always reads "not connected", so the frontend (which hides
//! Amazon with `ENABLED_STORES = ['epic', 'gog']` anyway) shows nothing
//! half-done.
//!
//! What is real already: the login spec (nile's sign-in page and Amazon's
//! finish redirect, told apart from Amazon's home page by its
//! `openid.assoc_handle`), the row-level Game Settings (no cloud saves:
//! Amazon has none), the art check, and the [`StoreLaunchHook`] that
//! refuses to start an Amazon row until nile's launch prep exists.
//!
//! Secrets: nothing here logs a token, a code or a store tool's output.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use grid_core::config::{AmazonConfig, Config};
use grid_core::launch::{LaunchError, StoreLaunchHook, StoreLaunchPrep};
use grid_core::library::queue::{DownloadsSnapshot, StoreJob};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::amazon::meta::AmazonMeta;
use grid_core::stores::amazon::{
    AmazonEndpoints, AmazonLoginStart, AmazonService, AMAZON_ASSOC_HANDLE, AMAZON_CODE_PARAM,
    AMAZON_SIGNIN_PATH,
};
use grid_core::stores::tools::{self, StoreTool};
use grid_core::stores::{GameSource, StoreError, StoreKind, SyncMode};
use secrecy::SecretString;

use super::{BoxFut, JobCtx, StoreBackend, StoreShared};
use crate::commands::stores::{
    OwnedState, StoreCloudState, StoreConnection, StoreGameSettings, StoreInstallOptions,
    StoreInstallRequest, StoreLibrary, StoreSaveConflict, StoreStatus, ToolState, ToolStatus,
};
use crate::store_auth::LoginSpec;

/// The Amazon endpoints for this run: Amazon's own, or (e2e builds only)
/// `GRID_LAUNCHER_E2E_AMAZON_BASE` for every address.
pub fn amazon_endpoints() -> AmazonEndpoints {
    #[cfg(feature = "e2e")]
    if let Some(base) = std::env::var(E2E_AMAZON_BASE_ENV)
        .ok()
        .and_then(|value| tauri::Url::parse(value.trim()).ok())
    {
        return AmazonEndpoints::e2e(base);
    }
    AmazonEndpoints::real()
}

/// The e2e build's Amazon base URL variable (`e2e/mock-romm/mock-amazon.mjs`,
/// package C).
#[cfg(feature = "e2e")]
pub const E2E_AMAZON_BASE_ENV: &str = "GRID_LAUNCHER_E2E_AMAZON_BASE";

/// The Amazon backend (Step 0: stubs).
pub struct AmazonStore {
    service: AmazonService,
    data_dir: PathBuf,
    config_path: PathBuf,
    shared: Arc<StoreShared>,
    /// nile's answer for the sign-in in progress
    /// (`auth --login --non-interactive`), kept for `register` once the
    /// code comes back. Set by `prepare_login`, taken by
    /// `run_finish_login` (package C).
    login: Mutex<Option<AmazonLoginStart>>,
}

const SOURCE: &str = "amazon";

/// What every not-yet-built Amazon action answers.
fn not_implemented() -> String {
    StoreError::not_implemented().to_string()
}

impl AmazonStore {
    pub fn new(
        service: AmazonService,
        data_dir: PathBuf,
        config_path: PathBuf,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        Arc::new(AmazonStore {
            service,
            data_dir,
            config_path,
            shared,
            login: Mutex::new(None),
        })
    }

    /// The real Amazon backend for this run.
    pub fn real(
        data_dir: PathBuf,
        config_path: PathBuf,
        http: reqwest::Client,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let service = AmazonService::new(data_dir.clone(), http, amazon_endpoints());
        AmazonStore::new(service, data_dir, config_path, shared)
    }

    fn amazon_config(&self) -> AmazonConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.amazon)
            .unwrap_or_default()
    }

    pub fn registry(&self) -> Option<Arc<Registry>> {
        self.shared.registry()
    }

    fn installed_rows(&self) -> Vec<InstalledGame> {
        self.registry()
            .and_then(|registry| registry.store_rows(StoreKind::Amazon).ok())
            .unwrap_or_default()
    }

    /// The login window's page: the URL nile printed for the sign-in in
    /// progress, else the sign-in page's address with no query.
    fn login_url(&self) -> tauri::Url {
        if let Some(start) = self.login.lock().unwrap().as_ref() {
            return start.url.clone();
        }
        let ep = self.service.endpoints();
        let host = ep
            .login_hosts
            .first()
            .cloned()
            .unwrap_or_else(|| ep.finish.clone());
        host.join(AMAZON_SIGNIN_PATH).unwrap_or(host)
    }
}

/// Amazon's card while nothing but Step 0 exists: not connected, nile's
/// pin and whether it is on disk, the installed rows' counts. Amazon has
/// no cloud saves, so `cloud_saves` is always off.
fn assemble_status(rows: &[InstalledGame], tool_installed: bool, install_dir: &str) -> StoreStatus {
    let pin = tools::pin(StoreTool::Nile);
    let installed_count = rows.len() as u32;
    StoreStatus {
        source: SOURCE.to_string(),
        connection: StoreConnection::NotConnected,
        username: String::new(),
        tool: ToolStatus {
            name: StoreTool::Nile.name().to_string(),
            version: pin.version.to_string(),
            state: if tool_installed {
                ToolState::Installed
            } else {
                ToolState::NotInstalled
            },
            size_bytes: tools::host_asset(StoreTool::Nile).map_or(0, |asset| asset.size),
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
        cloud_saves: false,
        install_dir: install_dir.to_string(),
        in_rail: installed_count > 0,
        last_error: String::new(),
    }
}

/// The row after Game Settings' Save. Every Amazon game is a Windows
/// version, so it always takes a compat tool. The executable is
/// `fuel.json`'s; a different one becomes the user's pin. No cloud saves.
pub(crate) fn apply_game_settings(
    mut row: InstalledGame,
    settings: &StoreGameSettings,
) -> InstalledGame {
    let mut meta = AmazonMeta::from_json(&row.store_meta);
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
    row.store_meta = meta.to_json();
    row
}

impl StoreBackend for AmazonStore {
    fn kind(&self) -> StoreKind {
        StoreKind::Amazon
    }
    fn status(&self) -> StoreStatus {
        let config = self.amazon_config();
        let install_dir =
            super::default_install_dir(&self.config_path, &config.install_dir, "Amazon Games");
        assemble_status(
            &self.installed_rows(),
            tools::installed_path(&self.data_dir, StoreTool::Nile).is_some(),
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
            store_name: StoreKind::Amazon.platform_name().to_string(),
            login_url: self.login_url(),
            finish: ep.finish.clone(),
            finish_query: Some(AMAZON_ASSOC_HANDLE),
            code_param: AMAZON_CODE_PARAM,
            user_agent: None,
        }
    }
    fn begin_login(&self) -> Result<(), String> {
        Err(not_implemented())
    }
    fn login_window_failed(&self, _error: &str) {}
    fn login_cancelled(&self) {
        self.login.lock().unwrap().take();
    }
    fn login_redirect_failed(&self) {
        self.login.lock().unwrap().take();
    }
    fn run_finish_login(self: Arc<Self>, code: SecretString) -> BoxFut<'static, ()> {
        drop(code);
        self.login.lock().unwrap().take();
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
        self.service.update_job(row, ctx.amazon())
    }
    fn repair_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.service.verify_job(row, ctx.amazon())
    }
    fn dlc_job(
        self: Arc<Self>,
        _row: InstalledGame,
        _dlc_id: String,
        _ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        // Amazon has no DLC model: an add-on is just another product.
        Box::pin(async { Err(not_implemented()) })
    }
    fn remove_dlc(
        &self,
        _registry: &Registry,
        _row: &InstalledGame,
        _dlc_id: &str,
    ) -> Result<(), StoreError> {
        Err(StoreError::not_implemented())
    }
    fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
        self.service.uninstall(registry, store_id)
    }
    fn game_settings(&self, row: &InstalledGame) -> StoreGameSettings {
        let meta = AmazonMeta::from_json(&row.store_meta);
        StoreGameSettings {
            install_dir: row.native_game_dir.clone(),
            executable: row.native_executable_path.clone(),
            exe_user_pinned: meta.exe_user_pinned,
            launch_parameters: row.native_launch_parameters.clone(),
            compat_tool: row.native_compat_tool.clone(),
            wineprefix: row.native_wineprefix.clone(),
            cloud_saves: false,
        }
    }
    fn apply_game_settings(
        &self,
        row: InstalledGame,
        settings: &StoreGameSettings,
    ) -> InstalledGame {
        apply_game_settings(row, settings)
    }
    fn windows_version(&self, _row: &InstalledGame) -> bool {
        true
    }
    fn cloud_enabled(&self, _row: &InstalledGame) -> bool {
        false
    }
    fn cloud_state<'a>(
        &'a self,
        _row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<StoreCloudState, String>> {
        Box::pin(async {
            Ok(StoreCloudState {
                state: "off".to_string(),
                synced_at: 0,
                locations: Vec::new(),
                conflict: None,
            })
        })
    }
    fn cloud_sync<'a>(
        &'a self,
        _registry: &'a Registry,
        _row: &'a InstalledGame,
        _mode: SyncMode,
    ) -> BoxFut<'a, Result<StoreCloudState, String>> {
        Box::pin(async { Err("Amazon Games has no cloud saves.".to_string()) })
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
    fn launch_hook(self: Arc<Self>) -> Option<Arc<dyn StoreLaunchHook>> {
        Some(self)
    }
}

/// The launch hook: an Amazon row needs nile's launch prep (its FUEL
/// environment, package A), so Step 0 refuses to start one; every other
/// row launches as it is.
impl StoreLaunchHook for AmazonStore {
    fn prepare<'a>(
        &'a self,
        game: &'a InstalledGame,
    ) -> BoxFut<'a, Result<Option<StoreLaunchPrep>, LaunchError>> {
        Box::pin(async move {
            if game.source != GameSource::Store(StoreKind::Amazon) {
                return Ok(None);
            }
            Err(LaunchError::Validation(format!(
                "Couldn\u{2019}t start {} through nile: {}",
                game.title,
                not_implemented()
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Arc<AmazonStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let service = AmazonService::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            AmazonEndpoints::e2e(tauri::Url::parse("http://127.0.0.1:4400/").unwrap()),
        );
        let store = AmazonStore::new(
            service,
            dir.path().to_path_buf(),
            config_path,
            StoreShared::new(),
        );
        (store, dir)
    }

    #[test]
    fn step_0_amazon_is_not_connected_and_refuses_actions() {
        let (store, _dir) = store();
        let status = store.status();
        assert_eq!(status.source, "amazon");
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.tool.name, "nile");
        assert_eq!(status.tool.version, "1.2.0");
        assert_eq!(status.tool.license, "GPL-3.0");
        assert_eq!(status.tool.source_url, "https://github.com/imLinguin/nile");
        assert!(!status.cloud_saves);
        assert!(!status.in_rail);
        assert_eq!(store.begin_login().unwrap_err(), "not implemented");
        assert_eq!(store.ensure_can_install().unwrap_err(), "not implemented");
        assert!(store.disconnect().is_err());
    }

    #[tokio::test]
    async fn prepare_login_says_not_implemented() {
        let (store, _dir) = store();
        let error = store.clone().prepare_login().await.unwrap_err();
        assert_eq!(error, "not implemented");
    }

    #[test]
    fn the_login_spec_needs_the_assoc_handle_on_the_finish() {
        let (store, _dir) = store();
        let spec = store.login_spec();
        assert_eq!(spec.store_name, "Amazon Games");
        assert_eq!(spec.finish.as_str(), "http://127.0.0.1:4400/");
        assert_eq!(
            spec.finish_query,
            Some(("openid.assoc_handle", "amzn_sonic_games_launcher"))
        );
        assert_eq!(spec.code_param, "openid.oa2.authorization_code");
        assert_eq!(spec.user_agent, None);
        assert_eq!(spec.login_url.as_str(), "http://127.0.0.1:4400/ap/signin");
        assert!(store.is_art_url("http://127.0.0.1:4400/art/a1.jpg"));
        assert!(store.is_art_url("https://m.media-amazon.com/images/I/a.jpg"));
        assert!(!store.is_art_url("https://www.amazon.com/a.jpg"));

        // The URL nile printed for this attempt wins while it is kept.
        let nile_url =
            tauri::Url::parse("http://127.0.0.1:4400/ap/signin?openid.assoc_handle=x").unwrap();
        *store.login.lock().unwrap() = Some(AmazonLoginStart::new(
            nile_url.clone(),
            SecretString::from("{}"),
        ));
        assert_eq!(store.login_spec().login_url, nile_url);
        store.login_cancelled();
        assert_eq!(
            store.login_spec().login_url.as_str(),
            "http://127.0.0.1:4400/ap/signin"
        );
    }

    #[tokio::test]
    async fn the_launch_hook_refuses_amazon_rows_only() {
        let (store, _dir) = store();
        let amazon = InstalledGame {
            source: GameSource::Store(StoreKind::Amazon),
            store_id: "amzn1.adg.product.1".into(),
            title: "Fogline".into(),
            ..Default::default()
        };
        let error = store.prepare(&amazon).await.unwrap_err();
        assert!(
            error.to_string().contains("Fogline through nile"),
            "{error}"
        );
        let gog = InstalledGame {
            source: GameSource::Store(StoreKind::Gog),
            ..Default::default()
        };
        assert!(store.prepare(&gog).await.unwrap().is_none());
        assert!(store.clone().launch_hook().is_some());
    }

    #[test]
    fn game_settings_take_a_compat_tool_and_have_no_cloud() {
        let (store, _dir) = store();
        let row = InstalledGame {
            native_executable_path: "/g/F/bin/Fogline.exe".into(),
            store_meta: AmazonMeta::default().to_json(),
            ..Default::default()
        };
        assert!(!store.game_settings(&row).cloud_saves);
        assert!(!store.cloud_enabled(&row));
        let settings = StoreGameSettings {
            install_dir: String::new(),
            executable: "/g/F/bin/Other.exe".into(),
            exe_user_pinned: false,
            launch_parameters: " -windowed ".into(),
            compat_tool: "GE-Proton9-20".into(),
            wineprefix: String::new(),
            cloud_saves: true,
        };
        let out = store.apply_game_settings(row, &settings);
        assert_eq!(out.native_compat_tool, "GE-Proton9-20");
        assert_eq!(out.native_launch_parameters, "-windowed");
        assert_eq!(out.native_executable_path, "/g/F/bin/Other.exe");
        assert_eq!(out.native_launch_source, "user");
        assert!(AmazonMeta::from_json(&out.store_meta).exe_user_pinned);
        assert!(store.windows_version(&out));
    }
}
