//! Steam in the app: the Steam [`StoreBackend`]
//! (`docs/stores-phase4-plan.md`). Steam is the first store with no tool,
//! no login window and no GRID-run job, so most of the trait answers
//! "Steam does that itself":
//!
//! - no tool (`status().tool` is an empty [`ToolStatus`]);
//! - no login window: Steam connects in Settings › Stores (`steam_test`,
//!   `steam_save`), never through `store_connect`;
//! - installs, updates, repairs, DLC and uninstalls are Steam's own; GRID
//!   only hands the client a `steam://` link (`steam_open`);
//! - no cloud saves, no Game Settings, no update check;
//! - launch: [`StoreBackend::launch_watched`] opens `steam://rungameid/<id>`
//!   and registers a watched session, so `launch_game_key` never reaches
//!   `LaunchService::launch_key` (which refuses Steam rows).
//!
//! Step 0 is a stub: the status reads `[stores.steam]` and
//! `SteamService::find_client()`; `steam_test` checks the SteamID64's form;
//! every other Steam command answers `not implemented` (package C).
//!
//! Secrets: the Web API key lives only in the OS keyring
//! ([`SteamKeyStore`]) and in memory as a [`SecretString`]. It is never
//! read here (only grid-core's `stores/steam/api.rs` exposes it), never in
//! a status, an event, an error, a log line or the config.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use grid_core::config::{Config, SteamConfig};
use grid_core::launch::{GameSession, LaunchError, LaunchService};
use grid_core::library::queue::{
    DownloadsSnapshot, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink,
};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::LibraryError;
use grid_core::secrets::SteamKeyStore;
use grid_core::stores::steam::{SteamApiError, SteamEndpoints, SteamId, SteamLink, SteamService};
use grid_core::stores::{GameKey, StoreError, StoreKind, SyncMode, NOT_IMPLEMENTED};
use secrecy::SecretString;

use super::{BoxFut, JobCtx, StoreBackend, StoreEvent, StoreShared};
use crate::commands::stores::{
    OwnedState, SteamAccount, SteamCard, SteamKeyState, SteamOpenAction, SteamTestResult,
    StoreCloudState, StoreConnection, StoreGameSettings, StoreInstallOptions, StoreInstallRequest,
    StoreLibrary, StoreSaveConflict, StoreStatus, ToolState, ToolStatus,
};
use crate::store_auth::LoginSpec;

/// The store's id.
const SOURCE: &str = "steam";

/// Every login path's answer: Steam has no login window.
pub const CONNECTS_IN_SETTINGS: &str = "Steam connects in Settings \u{203a} Stores.";

/// Every install, update, repair, DLC and uninstall path's answer.
pub const STEAM_DOES_IT: &str = "Steam installs, updates and uninstalls its games itself.";

/// When the Steam client isn't found.
pub const STEAM_NOT_INSTALLED: &str =
    "Steam isn\u{2019}t installed. Install Steam, then try again.";

/// A Test with no key given and none saved (Choices taken 1).
pub const PASTE_KEY_FIRST: &str = "Paste your key first.";

/// The Web API's per-request deadline.
const API_TIMEOUT: Duration = Duration::from_secs(20);

/// The e2e build's mock Steam base (`e2e/mock-romm/mock-steam.mjs`).
#[cfg(feature = "e2e")]
pub const E2E_STEAM_BASE_ENV: &str = "GRID_LAUNCHER_E2E_STEAM_BASE";
/// The e2e build's fake Steam root (`e2e/fixtures-steam/`).
#[cfg(feature = "e2e")]
pub const E2E_STEAM_ROOT_ENV: &str = "GRID_LAUNCHER_E2E_STEAM_ROOT";
/// The e2e build's fake `steam` (`e2e/fake-tools/steam.mjs`).
#[cfg(feature = "e2e")]
pub const E2E_STEAM_EXE_ENV: &str = "GRID_LAUNCHER_E2E_STEAM_EXE";

/// The Steam endpoints for this run: Steam's own, or (e2e builds only)
/// `GRID_LAUNCHER_E2E_STEAM_BASE` for every address.
pub fn steam_endpoints() -> SteamEndpoints {
    #[cfg(feature = "e2e")]
    if let Some(base) = std::env::var(E2E_STEAM_BASE_ENV)
        .ok()
        .and_then(|value| tauri::Url::parse(value.trim()).ok())
    {
        return SteamEndpoints::e2e(base);
    }
    SteamEndpoints::real()
}

/// The Steam service for this run. Its HTTP client follows no redirect
/// (reqwest would otherwise chase a 3xx; Steam's API never sends one, so a
/// 3xx reads as "Couldn't reach api.steampowered.com"). An e2e build always
/// sets the roots override (the fake root, or none), so it never reads a
/// real Steam root.
pub fn steam_service() -> SteamService {
    let http = reqwest::Client::builder()
        .user_agent(concat!("grid-launcher/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(API_TIMEOUT)
        .build()
        .unwrap_or_default();
    let service = SteamService::new(http, steam_endpoints());
    #[cfg(feature = "e2e")]
    let service = {
        let env_path = |name: &str| {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        service.with_roots_override(
            env_path(E2E_STEAM_ROOT_ENV).into_iter().collect(),
            env_path(E2E_STEAM_EXE_ENV),
        )
    };
    service
}

/// The Steam backend.
pub struct SteamStore {
    service: SteamService,
    endpoints: SteamEndpoints,
    config_path: PathBuf,
    /// The keyring's Steam slot.
    keys: Arc<dyn SteamKeyStore>,
    /// The emitter and registry every store shares.
    shared: Arc<StoreShared>,
}

impl SteamStore {
    pub fn new(
        service: SteamService,
        config_path: PathBuf,
        keys: Arc<dyn SteamKeyStore>,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let endpoints = service.endpoints().clone();
        Arc::new(SteamStore {
            service,
            endpoints,
            config_path,
            keys,
            shared,
        })
    }

    /// The real Steam backend for this run. `keys` is the OS keyring's
    /// Steam slot (`KeyringStore` in `lib.rs`; `MemoryStore` in tests).
    pub fn real(
        config_path: PathBuf,
        keys: Arc<dyn SteamKeyStore>,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        SteamStore::new(steam_service(), config_path, keys, shared)
    }

    fn steam_config(&self) -> SteamConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.steam)
            .unwrap_or_default()
    }

    /// Emits `stores-changed` with every store's status.
    pub fn changed(&self) {
        self.shared.changed();
    }

    #[allow(dead_code)]
    fn emit(&self, event: StoreEvent) {
        self.shared.emit(event);
    }

    pub fn connection(&self) -> StoreConnection {
        if self.steam_config().connected {
            StoreConnection::Connected
        } else {
            StoreConnection::NotConnected
        }
    }

    /// Steam's card: the config's connection, the client, no tool.
    pub fn status(&self) -> StoreStatus {
        let config = self.steam_config();
        let client = self.service.find_client();
        let accounts = client
            .as_ref()
            .map(|client| self.service.local_accounts(client))
            .unwrap_or_default()
            .into_iter()
            .map(|account| SteamAccount {
                persona: account.persona,
                steam_id: account.steam_id.to_string(),
                most_recent: account.most_recent,
            })
            .collect();
        assemble_status(&config, client.map(|c| c.display), accounts)
    }

    /// Whether `url` is store art the UI may load (Steam's image hosts, the
    /// e2e mock's origin).
    pub fn is_art_url(&self, url: &str) -> bool {
        grid_core::stores::art::is_store_art_url(url, &self.endpoints.art_origins)
    }

    // --- the Steam commands (package C) ---------------------------------------

    /// `steam_test`: the SteamID64's form (no request), then one
    /// `GetOwnedGames` call with `key`, or with the saved key when `None`.
    pub async fn test(
        &self,
        steam_id: &str,
        key: Option<SecretString>,
    ) -> Result<SteamTestResult, String> {
        let Ok(id) = steam_id.parse::<SteamId>() else {
            return Ok(SteamTestResult::BadSteamId);
        };
        let key = match key {
            Some(key) => key,
            None => {
                let keys = self.keys.clone();
                tokio::task::spawn_blocking(move || keys.load())
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| format!("Couldn\u{2019}t read your key from the OS keyring: {e}"))?
                    .ok_or_else(|| PASTE_KEY_FIRST.to_string())?
            }
        };
        let owned = self.service.owned(&key, id).await;
        Ok(test_result(owned.map(|owned| owned.games.len() as u32)))
    }

    /// `steam_save`: the test, then the keyring, `[stores.steam]`, a scan.
    pub async fn save(
        &self,
        steam_id: &str,
        key: Option<SecretString>,
    ) -> Result<SteamTestResult, String> {
        match self.test(steam_id, key).await? {
            SteamTestResult::BadSteamId => Ok(SteamTestResult::BadSteamId),
            _ => Err(NOT_IMPLEMENTED.to_string()),
        }
    }

    /// `steam_remove_key`.
    pub fn remove_key(&self) -> Result<(), String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    /// `steam_recheck_client`.
    pub fn recheck_client(&self) -> Result<(), String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    /// `steam_open`: hands the client an install, uninstall or validate
    /// link (install also admits the Downloads row and starts the watch).
    pub fn open(&self, _store_id: &str, _action: SteamOpenAction) -> Result<(), String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    /// Starts a Steam game through the Steam client and watches it: the
    /// row must be installed and the client found; then
    /// `steam://rungameid/<id>` (with `-silent`) and a watched session.
    async fn launch(
        self: Arc<Self>,
        key: GameKey,
        launch: Arc<LaunchService>,
    ) -> Result<GameSession, LaunchError> {
        let GameKey::Store {
            source: StoreKind::Steam,
            store_id,
        } = &key
        else {
            return Err(LaunchError::NotInstalled);
        };
        let appid: u32 = store_id.parse().map_err(|_| LaunchError::NotInstalled)?;
        let registry = self
            .shared
            .registry()
            .ok_or_else(|| LaunchError::Registry("not ready".to_string()))?;
        let lookup = key.clone();
        let row = tokio::task::spawn_blocking(move || registry.find_key(&lookup))
            .await
            .map_err(|e| LaunchError::Registry(e.to_string()))?
            .map_err(|e| LaunchError::Registry(e.to_string()))?
            .ok_or(LaunchError::NotInstalled)?;
        let client = self
            .service
            .find_client()
            .ok_or_else(|| LaunchError::Validation(STEAM_NOT_INSTALLED.to_string()))?;
        self.service.open(&client, SteamLink::Run(appid))?;
        let alive = self.service.alive_check(&client, appid);
        launch.register_watched(key, &row.title, "Steam", "Steam", 0, alive)
    }
}

/// A Test's answer from one `GetOwnedGames` result (the owned count).
pub fn test_result(owned: Result<u32, SteamApiError>) -> SteamTestResult {
    match owned {
        Ok(owned_count) => SteamTestResult::Ok { owned_count },
        Err(SteamApiError::Rejected { status }) => SteamTestResult::BadKey { status },
        Err(SteamApiError::Private) => SteamTestResult::Private,
        Err(SteamApiError::NoSuchAccount) => SteamTestResult::BadSteamId,
        Err(SteamApiError::Network) => SteamTestResult::Network,
    }
}

/// Steam's [`StoreStatus`]: no tool, no owned list yet (Step 0), the card's
/// Steam part. The key's state is `none` until package C reads it.
fn assemble_status(
    config: &SteamConfig,
    client_path: Option<String>,
    accounts: Vec<SteamAccount>,
) -> StoreStatus {
    let connection = if config.connected {
        StoreConnection::Connected
    } else {
        StoreConnection::NotConnected
    };
    StoreStatus {
        source: SOURCE.to_string(),
        connection,
        username: if config.connected {
            config.persona.clone()
        } else {
            String::new()
        },
        tool: ToolStatus {
            name: String::new(),
            version: String::new(),
            state: ToolState::NotInstalled,
            size_bytes: 0,
            error: String::new(),
            license: String::new(),
            source_url: String::new(),
            done: 0,
            total: 0,
        },
        owned: OwnedState::Idle,
        owned_error: String::new(),
        owned_count: 0,
        installed_count: 0,
        installed_bytes: 0,
        refreshed_at: 0,
        cloud_saves: false,
        install_dir: String::new(),
        in_rail: config.connected,
        last_error: String::new(),
        steam: Some(SteamCard {
            client_path: client_path.unwrap_or_default(),
            steam_id: config.steam_id.clone(),
            key: SteamKeyState::None,
            key_tail: String::new(),
            accounts,
        }),
    }
}

impl StoreBackend for SteamStore {
    fn kind(&self) -> StoreKind {
        StoreKind::Steam
    }
    fn status(&self) -> StoreStatus {
        SteamStore::status(self)
    }
    fn connection(&self) -> StoreConnection {
        SteamStore::connection(self)
    }
    fn changed(&self) {
        SteamStore::changed(self)
    }
    fn load(&self) {}
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async {})
    }
    fn observe_downloads(&self, _snapshot: &DownloadsSnapshot) {}
    fn login_spec(&self) -> LoginSpec {
        // Never opened: `begin_login` and `prepare_login` refuse first.
        LoginSpec {
            store_name: "Steam".to_string(),
            login_url: self.endpoints.api.clone(),
            finish: self.endpoints.api.clone(),
            finish_query: None,
            code_param: "code",
            user_agent: None,
        }
    }
    fn begin_login(&self) -> Result<(), String> {
        Err(CONNECTS_IN_SETTINGS.to_string())
    }
    fn prepare_login(self: Arc<Self>) -> BoxFut<'static, Result<LoginSpec, String>> {
        Box::pin(async { Err(CONNECTS_IN_SETTINGS.to_string()) })
    }
    fn login_window_failed(&self, _error: &str) {}
    fn login_cancelled(&self) {}
    fn login_redirect_failed(&self) {}
    fn run_finish_login(self: Arc<Self>, _code: SecretString) -> BoxFut<'static, ()> {
        Box::pin(async {})
    }
    fn disconnect(&self) -> Result<(), String> {
        Err(NOT_IMPLEMENTED.to_string())
    }
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()> {
        // Steam has no tool.
        Box::pin(async {})
    }
    fn run_refresh(self: Arc<Self>, _announce: bool) -> BoxFut<'static, ()> {
        Box::pin(async {})
    }
    fn library(&self, _updates: &HashMap<String, String>) -> StoreLibrary {
        StoreLibrary {
            status: SteamStore::status(self),
            installed: Vec::new(),
            available: Vec::new(),
        }
    }
    fn check_updates(self: Arc<Self>, _app: tauri::AppHandle) {
        // Steam updates its own games.
    }
    fn set_install_dir(&self, _path: &str) {
        // Steam picks the library folder in its own install dialog.
    }
    fn set_cloud_saves(&self, _on: bool) {
        // Steam Cloud is Steam's.
    }
    fn ensure_can_install(&self) -> Result<(), String> {
        Err(STEAM_DOES_IT.to_string())
    }
    fn install_options(
        self: Arc<Self>,
        _store_id: String,
    ) -> BoxFut<'static, Result<StoreInstallOptions, String>> {
        Box::pin(async { Err(STEAM_DOES_IT.to_string()) })
    }
    fn install_job(
        self: Arc<Self>,
        _store_id: String,
        _request: StoreInstallRequest,
        _ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async { Err(STEAM_DOES_IT.to_string()) })
    }
    fn update_job(&self, row: &InstalledGame, _ctx: JobCtx) -> StoreJob {
        refused_job(row)
    }
    fn repair_job(&self, row: &InstalledGame, _ctx: JobCtx) -> StoreJob {
        refused_job(row)
    }
    fn dlc_job(
        self: Arc<Self>,
        _row: InstalledGame,
        _dlc_id: String,
        _ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async { Err(STEAM_DOES_IT.to_string()) })
    }
    fn remove_dlc(
        &self,
        _registry: &Registry,
        _row: &InstalledGame,
        _dlc_id: &str,
    ) -> Result<(), StoreError> {
        Err(StoreError::Refused(STEAM_DOES_IT.to_string()))
    }
    fn uninstall(&self, _registry: &Registry, _store_id: &str) -> Result<(), StoreError> {
        Err(StoreError::Refused(STEAM_DOES_IT.to_string()))
    }
    fn game_settings(&self, row: &InstalledGame) -> StoreGameSettings {
        // The design gives Steam games no Game Settings.
        StoreGameSettings {
            install_dir: row.extracted_dir.clone(),
            executable: String::new(),
            exe_user_pinned: false,
            launch_parameters: String::new(),
            compat_tool: String::new(),
            wineprefix: String::new(),
            cloud_saves: false,
        }
    }
    fn apply_game_settings(
        &self,
        row: InstalledGame,
        _settings: &StoreGameSettings,
    ) -> InstalledGame {
        row
    }
    fn windows_version(&self, _row: &InstalledGame) -> bool {
        false
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
                state: "none".to_string(),
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
        Box::pin(async { Err("Steam syncs its own cloud saves.".to_string()) })
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
        SteamStore::is_art_url(self, url)
    }
    fn launch_watched(
        self: Arc<Self>,
        key: GameKey,
        launch: Arc<LaunchService>,
    ) -> Option<BoxFut<'static, Result<GameSession, LaunchError>>> {
        Some(Box::pin(self.launch(key, launch)))
    }
}

/// A job that only fails with [`STEAM_DOES_IT`] (`update_job` and
/// `repair_job` can't refuse; the UI never asks for them for Steam).
fn refused_job(row: &InstalledGame) -> StoreJob {
    StoreJob {
        source: StoreKind::Steam,
        store_id: row.store_id.clone(),
        op: StoreOp::Update,
        title: row.title.clone(),
        drive: None,
        runner: Arc::new(RefusedRunner),
    }
}

struct RefusedRunner;

impl StoreJobRunner for RefusedRunner {
    fn download(
        &self,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Extract(STEAM_DOES_IT.to_string()))
    }
    fn finalize(&self, _sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        Err(LibraryError::Extract(STEAM_DOES_IT.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::secrets::MemoryStore;

    fn store() -> (Arc<SteamStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let service = SteamService::new(reqwest::Client::new(), SteamEndpoints::real())
            .with_roots_override(Vec::new(), None);
        let store = SteamStore::new(
            service,
            config_path,
            Arc::new(MemoryStore::default()),
            StoreShared::new(),
        );
        (store, dir)
    }

    #[test]
    fn the_status_has_no_tool_and_the_steam_card() {
        let (store, _dir) = store();
        let status = store.status();
        assert_eq!(status.source, "steam");
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.tool.name, "");
        assert!(!status.in_rail);
        let card = status.steam.unwrap();
        assert_eq!(card.client_path, "");
        assert_eq!(card.key, SteamKeyState::None);
        assert_eq!(card.key_tail, "");
    }

    #[test]
    fn a_connected_config_shows_the_persona_and_the_rail() {
        let (store, _dir) = store();
        crate::config_write::modify_config(&store.config_path, |config| {
            config.stores.steam = SteamConfig {
                connected: true,
                steam_id: "76561198000000001".into(),
                persona: "SamPlays".into(),
            };
            Ok(())
        })
        .unwrap();
        let status = store.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.username, "SamPlays");
        assert!(status.in_rail);
        assert_eq!(status.steam.unwrap().steam_id, "76561198000000001");
    }

    #[tokio::test]
    async fn test_checks_the_steam_ids_form_first() {
        let (store, _dir) = store();
        assert_eq!(
            store.test("123", None).await,
            Ok(SteamTestResult::BadSteamId)
        );
        assert_eq!(
            store
                .test(
                    "76561198000000001",
                    Some(SecretString::from("FAKE-STEAM-KEY-not-real"))
                )
                .await,
            Ok(SteamTestResult::Network),
            "Step 0's service answers Network for every call"
        );
        assert_eq!(
            store.test("76561198000000001", None).await,
            Err(PASTE_KEY_FIRST.to_string())
        );
        store
            .keys
            .save(&SecretString::from("FAKE-STEAM-KEY-not-real"))
            .unwrap();
        assert_eq!(
            store.test("76561198000000001", None).await,
            Ok(SteamTestResult::Network)
        );
    }

    #[test]
    fn every_api_answer_is_a_test_result() {
        assert_eq!(test_result(Ok(4)), SteamTestResult::Ok { owned_count: 4 });
        assert_eq!(
            test_result(Err(SteamApiError::Rejected { status: 401 })),
            SteamTestResult::BadKey { status: 401 }
        );
        assert_eq!(
            test_result(Err(SteamApiError::Private)),
            SteamTestResult::Private
        );
        assert_eq!(
            test_result(Err(SteamApiError::NoSuchAccount)),
            SteamTestResult::BadSteamId
        );
        assert_eq!(
            test_result(Err(SteamApiError::Network)),
            SteamTestResult::Network
        );
    }

    #[tokio::test]
    async fn steam_refuses_the_tool_and_login_paths() {
        let (store, _dir) = store();
        assert_eq!(store.begin_login(), Err(CONNECTS_IN_SETTINGS.to_string()));
        assert!(store.clone().prepare_login().await.is_err());
        assert_eq!(store.ensure_can_install(), Err(STEAM_DOES_IT.to_string()));
        assert!(store.clone().install_options("620".into()).await.is_err());
        assert!(store.is_art_url("https://shared.akamai.steamstatic.com/a.jpg"));
        assert!(!store.is_art_url("https://evil.akamaihd.net/a.jpg"));
    }
}
