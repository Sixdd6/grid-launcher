//! GOG in the app: the GOG [`StoreBackend`]. grid-core's
//! `stores::gog::GogService` does the store work; this backend owns what
//! the app adds around it:
//!
//! - the connection state machine (not connected → login window open →
//!   completing → connected; expired when a refresh fails) and the
//!   `store-login` outcome;
//! - the gogdl install right after a connect, with its progress in
//!   `StoreStatus.tool`;
//! - the owned-list cache (`<data_dir>/stores/cache/gog-owned.json`,
//!   product metadata only, never a token) and its refresh;
//! - GOG's `StoreStatus` (the facade emits every store's in
//!   `stores-changed`);
//! - the `store-notice` cards (GameStores card "Downloads and
//!   notifications"), worded in [`notice`];
//! - the IPC shapes of store games, install options, cloud states;
//! - the GOG-only extras the commands reach through
//!   `StoresService::gog`: offline installers and drive backups.
//!
//! The store calls go through [`GogApi`], a seam the unit tests fake.
//!
//! Secrets: the login code arrives as a [`SecretString`] and goes straight
//! to `complete_login`. Nothing here logs a token, a code, a URL with a
//! query, or a store tool's output; every error shown to the user is
//! `StoreError`'s Display (GRID's own words).

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use grid_core::config::{Config, GogConfig};
use grid_core::library::queue::{DownloadStatus, DownloadsSnapshot, StoreJob};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::gog::meta::{GogMeta, GogOrigin, GogPlatform};
use grid_core::stores::gog::{
    CloudState, GogAccount, GogEndpoints, GogInstallOptions, GogInstallRequest, GogJobCtx,
    GogProduct, GogProductDlc, GogService, GogUpdate, SyncMode,
};
use grid_core::stores::tools::{self, StoreTool};
use grid_core::stores::{GameKey, StoreError, StoreKind};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::Url;

use super::{default_install_dir, BoxFut, JobCtx, StoreBackend, StoreEvent, StoreShared};
use crate::commands::stores::{
    OwnedState, StoreCloudState, StoreConnection, StoreDlc, StoreGame, StoreGameSettings,
    StoreInstallOptions, StoreInstallRequest, StoreLibrary, StoreLoginEvent, StoreNotice,
    StoreNoticeAction, StoreSaveConflict, StoreStatus, StoreVersionOption, ToolState, ToolStatus,
};
use crate::store_auth::LoginSpec;

/// What the app asks of GOG. [`RealGog`] forwards to grid-core's
/// `GogService` and the tool installer; the unit tests fake it.
pub trait GogApi: Send + Sync {
    fn login_url(&self) -> Url;
    fn complete_login(&self, code: SecretString) -> BoxFut<'_, Result<GogAccount, StoreError>>;
    fn account(&self) -> BoxFut<'_, Result<Option<GogAccount>, StoreError>>;
    fn disconnect(&self) -> io::Result<()>;
    fn owned(&self) -> BoxFut<'_, Result<Vec<GogProduct>, StoreError>>;
    fn tool_installed(&self) -> bool;
    /// Why the tool can't run here (the Linux gogdl needs Python 3.10),
    /// `None` when it can.
    fn tool_problem(&self) -> Option<String>;
    fn install_tool<'a>(
        &'a self,
        progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
    ) -> BoxFut<'a, Result<(), StoreError>>;
    fn install_options<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFut<'a, Result<GogInstallOptions, StoreError>>;
    fn install_job(&self, req: GogInstallRequest, ctx: GogJobCtx) -> StoreJob;
    fn update_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob;
    fn repair_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob;
    fn dlc_job(&self, row: &InstalledGame, dlc_id: &str, ctx: GogJobCtx) -> StoreJob;
    fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc_id: &str,
    ) -> Result<(), StoreError>;
    fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError>;
    fn check_updates<'a>(
        &'a self,
        rows: &'a [InstalledGame],
    ) -> BoxFut<'a, Result<Vec<GogUpdate>, StoreError>>;
    fn cloud_state<'a>(
        &'a self,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<CloudState, StoreError>>;
    fn cloud_sync<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
        mode: SyncMode,
    ) -> BoxFut<'a, Result<CloudState, StoreError>>;
}

/// [`GogApi`] over grid-core.
pub struct RealGog {
    service: GogService,
    http: reqwest::Client,
    data_dir: PathBuf,
}

impl RealGog {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, endpoints: GogEndpoints) -> Self {
        RealGog {
            service: GogService::new(data_dir.clone(), http.clone(), endpoints),
            http,
            data_dir,
        }
    }
}

impl GogApi for RealGog {
    fn login_url(&self) -> Url {
        self.service.login_url()
    }
    fn complete_login(&self, code: SecretString) -> BoxFut<'_, Result<GogAccount, StoreError>> {
        Box::pin(self.service.complete_login(code))
    }
    fn account(&self) -> BoxFut<'_, Result<Option<GogAccount>, StoreError>> {
        Box::pin(self.service.account())
    }
    fn disconnect(&self) -> io::Result<()> {
        self.service.disconnect()
    }
    fn owned(&self) -> BoxFut<'_, Result<Vec<GogProduct>, StoreError>> {
        Box::pin(self.service.owned())
    }
    fn tool_installed(&self) -> bool {
        tools::installed_path(&self.data_dir, StoreTool::Gogdl).is_some()
    }
    fn tool_problem(&self) -> Option<String> {
        if self.tool_installed() {
            return match tools::tool_command(
                &self.data_dir,
                StoreTool::Gogdl,
                Vec::new(),
                Vec::new(),
            ) {
                Err(StoreError::Refused(problem)) => Some(problem),
                _ => None,
            };
        }
        let asset = tools::host_asset(StoreTool::Gogdl)?;
        if asset.launcher != tools::AssetLauncher::Python3 {
            return None;
        }
        let env = grid_core::launch::spawn::clean_env();
        tools::python_problem(StoreTool::Gogdl, grid_core::launch::umu::probe_python(&env))
    }
    fn install_tool<'a>(
        &'a self,
        mut progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
    ) -> BoxFut<'a, Result<(), StoreError>> {
        Box::pin(async move {
            tools::install_tool(&self.http, &self.data_dir, StoreTool::Gogdl, &mut *progress)
                .await
                .map(|_| ())
        })
    }
    fn install_options<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFut<'a, Result<GogInstallOptions, StoreError>> {
        Box::pin(self.service.install_options(id))
    }
    fn install_job(&self, req: GogInstallRequest, ctx: GogJobCtx) -> StoreJob {
        self.service.install_job(req, ctx)
    }
    fn update_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob {
        self.service.update_job(row, ctx)
    }
    fn repair_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob {
        self.service.repair_job(row, ctx)
    }
    fn dlc_job(&self, row: &InstalledGame, dlc_id: &str, ctx: GogJobCtx) -> StoreJob {
        self.service.dlc_job(row, dlc_id, ctx)
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
    fn check_updates<'a>(
        &'a self,
        rows: &'a [InstalledGame],
    ) -> BoxFut<'a, Result<Vec<GogUpdate>, StoreError>> {
        Box::pin(self.service.check_updates(rows))
    }
    fn cloud_state<'a>(
        &'a self,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<CloudState, StoreError>> {
        Box::pin(self.service.cloud_state(row))
    }
    fn cloud_sync<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
        mode: SyncMode,
    ) -> BoxFut<'a, Result<CloudState, StoreError>> {
        Box::pin(self.service.cloud_sync(registry, row, mode))
    }
}

/// The GOG endpoints for this run: GOG's own, or (e2e builds only)
/// `GRID_LAUNCHER_E2E_GOG_BASE` for every host.
pub fn gog_endpoints() -> GogEndpoints {
    #[cfg(feature = "e2e")]
    if let Some(base) = std::env::var(E2E_GOG_BASE_ENV)
        .ok()
        .and_then(|value| Url::parse(value.trim()).ok())
    {
        return GogEndpoints::e2e(base);
    }
    GogEndpoints::real()
}

/// The e2e build's GOG base URL variable (`e2e/mock-romm/mock-gog.mjs`).
#[cfg(feature = "e2e")]
pub const E2E_GOG_BASE_ENV: &str = "GRID_LAUNCHER_E2E_GOG_BASE";

// --- state --------------------------------------------------------------------

/// Where the login stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginPhase {
    Idle,
    /// The login window is open.
    Open,
    /// The redirect came; `complete_login` runs.
    Completing,
}

/// The tool's state, with its download progress.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ToolPhase {
    NotInstalled,
    Installing { done: u64, total: u64 },
    Installed,
    Failed(String),
}

#[derive(Debug, Clone)]
struct GogState {
    connection: StoreConnection,
    username: String,
    tool: ToolPhase,
    owned: OwnedState,
    owned_error: String,
    products: Vec<GogProduct>,
    refreshed_at: i64,
    /// A failed sign-in's error, left on the store's card.
    last_error: String,
    login: LoginPhase,
    cloud: HashMap<String, CloudState>,
}

impl Default for GogState {
    fn default() -> Self {
        GogState {
            connection: StoreConnection::NotConnected,
            username: String::new(),
            tool: ToolPhase::NotInstalled,
            owned: OwnedState::Idle,
            owned_error: String::new(),
            products: Vec::new(),
            refreshed_at: 0,
            last_error: String::new(),
            login: LoginPhase::Idle,
            cloud: HashMap::new(),
        }
    }
}

/// The GOG backend.
pub struct GogStore {
    api: Arc<dyn GogApi>,
    endpoints: GogEndpoints,
    data_dir: PathBuf,
    config_path: PathBuf,
    state: Mutex<GogState>,
    /// The emitter and registry every store shares.
    shared: Arc<StoreShared>,
    /// Games whose cloud saves are syncing right now.
    syncing: Mutex<HashSet<String>>,
    /// Downloads entries whose finish already had its card.
    announced: Mutex<HashSet<u64>>,
    /// `owned()` answered this run.
    owned_fetched: std::sync::atomic::AtomicBool,
}

/// The tool's name in GRID's words.
const TOOL_NAME: &str = "gogdl";
const STORE_NAME: &str = "GOG";
const SOURCE: &str = "gog";
const OWNED_CACHE_FILE: &str = "gog-owned.json";

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl GogStore {
    pub fn new(
        api: Arc<dyn GogApi>,
        endpoints: GogEndpoints,
        data_dir: PathBuf,
        config_path: PathBuf,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        Arc::new(GogStore {
            api,
            endpoints,
            data_dir,
            config_path,
            state: Mutex::new(GogState::default()),
            shared,
            syncing: Mutex::new(HashSet::new()),
            announced: Mutex::new(HashSet::new()),
            owned_fetched: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// The real GOG backend for this run.
    pub fn real(
        data_dir: PathBuf,
        config_path: PathBuf,
        http: reqwest::Client,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let endpoints = gog_endpoints();
        let api = Arc::new(RealGog::new(data_dir.clone(), http, endpoints.clone()));
        GogStore::new(api, endpoints, data_dir, config_path, shared)
    }

    pub fn api(&self) -> &Arc<dyn GogApi> {
        &self.api
    }

    /// Whether `url` is store art the UI may load
    /// ([`grid_core::stores::art::is_store_art_url`]; the gamesdb origin is
    /// the e2e mock's).
    pub fn is_art_url(&self, url: &str) -> bool {
        grid_core::stores::art::is_store_art_url(url, std::slice::from_ref(&self.endpoints.gamesdb))
    }

    pub fn registry(&self) -> Option<Arc<Registry>> {
        self.shared.registry()
    }

    fn emit(&self, event: StoreEvent) {
        self.shared.emit(event);
    }

    fn notify(&self, notice: StoreNotice) {
        self.emit(StoreEvent::Notice(notice));
    }

    /// Emits `stores-changed` with every store's status.
    pub fn changed(&self) {
        self.shared.changed();
    }

    fn gog_config(&self) -> GogConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.gog)
            .unwrap_or_default()
    }

    fn write_gog_config(&self, change: impl FnOnce(&mut GogConfig)) {
        if let Err(e) = crate::config_write::modify_config(&self.config_path, |config| {
            change(&mut config.stores.gog);
            Ok(())
        }) {
            tracing::warn!("stores: config write failed: {e}");
        }
    }

    // --- startup --------------------------------------------------------------

    /// Startup: the tool, the cached owned list and the config's
    /// connection. The login check runs next ([`Self::check_login`]).
    pub fn load(&self) {
        let config = self.gog_config();
        let cache = read_owned_cache(&self.data_dir);
        let tool = self.tool_phase_now();
        let mut state = self.state.lock().unwrap();
        state.tool = tool;
        if config.connected {
            state.connection = StoreConnection::Connected;
            state.username = config.username.clone();
            if let Some(cache) = cache {
                state.products = cache.products.into_iter().map(Into::into).collect();
                state.refreshed_at = cache.refreshed_at;
                state.owned = OwnedState::Ready;
            }
        }
    }

    /// Asks gogdl whether the saved login still works: expired when its
    /// refresh fails, signed out when its token file is gone. A network
    /// failure keeps the saved state (installed games play offline). Then
    /// refreshes the owned list when signed in.
    pub async fn check_login(self: &Arc<Self>) {
        if !self.gog_config().connected {
            return;
        }
        let result = self.api.account().await;
        let mut expired = false;
        let refresh = {
            let mut state = self.state.lock().unwrap();
            match &result {
                Ok(Some(account)) => {
                    state.connection = StoreConnection::Connected;
                    if !account.username.is_empty() {
                        state.username = account.username.clone();
                    }
                    true
                }
                Ok(None) => {
                    state.connection = StoreConnection::NotConnected;
                    false
                }
                Err(StoreError::LoginExpired) => {
                    state.connection = StoreConnection::Expired;
                    expired = true;
                    false
                }
                Err(e) => {
                    tracing::info!("stores: GOG login not checked: {e}");
                    false
                }
            }
        };
        if let Ok(None) = result {
            self.write_gog_config(|gog| gog.connected = false);
        }
        if expired {
            self.notify(notice::login_expired());
        }
        self.changed();
        if refresh {
            self.refresh_owned(false).await;
            self.refresh_cloud_states().await;
        }
    }

    /// Looks at every installed game's cloud saves (no sync), so the
    /// Library shows SAVE CONFLICT before anyone presses Play. Signed in
    /// only; failures only log.
    pub async fn refresh_cloud_states(self: &Arc<Self>) {
        if self.connection() != StoreConnection::Connected || !self.gog_config().cloud_saves {
            return;
        }
        let rows = self.installed_rows();
        let mut changed = false;
        for row in rows.iter().filter(|row| self.cloud_enabled(row)) {
            match self.api.cloud_state(row).await {
                Ok(state) => {
                    self.remember_cloud(&row.store_id, &state);
                    changed = true;
                }
                Err(e) => tracing::info!("stores: cloud state not read: {e}"),
            }
        }
        if changed {
            self.changed();
        }
    }

    // --- status -----------------------------------------------------------------

    /// GOG's status.
    pub fn status(&self) -> StoreStatus {
        let config = self.gog_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap().clone();
        assemble_status(&state, &config, &rows, &self.default_install_dir(&config))
    }

    fn installed_rows(&self) -> Vec<InstalledGame> {
        self.registry()
            .and_then(|registry| registry.store_rows(StoreKind::Gog).ok())
            .unwrap_or_default()
    }

    /// Where GOG games install: the configured folder, else
    /// `<library>/GOG`. `""` when neither is set.
    fn default_install_dir(&self, gog: &GogConfig) -> String {
        default_install_dir(&self.config_path, &gog.install_dir, "GOG")
    }

    pub fn install_base_dir(&self) -> String {
        self.default_install_dir(&self.gog_config())
    }

    pub fn connection(&self) -> StoreConnection {
        self.state.lock().unwrap().connection
    }

    /// The tool's state on disk: installed, missing, or installed but
    /// unable to run (its Python problem).
    fn tool_phase_now(&self) -> ToolPhase {
        if !self.api.tool_installed() {
            return ToolPhase::NotInstalled;
        }
        match self.api.tool_problem() {
            Some(problem) => ToolPhase::Failed(problem),
            None => ToolPhase::Installed,
        }
    }

    pub fn tool_ready(&self) -> bool {
        matches!(self.state.lock().unwrap().tool, ToolPhase::Installed)
    }

    // --- login ------------------------------------------------------------------

    /// The login window is opening. `Err` while a sign-in is finishing.
    pub fn begin_login(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        match state.login {
            LoginPhase::Completing => Err("GRID is finishing the sign-in to GOG.".to_string()),
            _ => {
                state.login = LoginPhase::Open;
                Ok(())
            }
        }
    }

    /// The window could not open.
    pub fn login_window_failed(&self, error: &str) {
        self.state.lock().unwrap().login = LoginPhase::Idle;
        self.fail_login(&format!("the login window didn't open ({error})"));
    }

    #[cfg(test)]
    pub fn login_phase(&self) -> LoginPhase {
        self.state.lock().unwrap().login
    }

    /// ✕, Cancel or Esc: silent, nothing changes.
    pub fn login_cancelled(&self) {
        {
            let mut state = self.state.lock().unwrap();
            if state.login != LoginPhase::Open {
                return;
            }
            state.login = LoginPhase::Idle;
        }
        self.emit(StoreEvent::Login(StoreLoginEvent {
            source: SOURCE.to_string(),
            outcome: "cancelled".to_string(),
            username: String::new(),
            error: String::new(),
        }));
    }

    /// The finish redirect carried no code.
    pub fn login_redirect_failed(&self) {
        self.state.lock().unwrap().login = LoginPhase::Idle;
        self.fail_login("GOG returned an error. Try again.");
    }

    fn fail_login(&self, why: &str) {
        self.state.lock().unwrap().last_error = format!("Couldn\u{2019}t sign in to GOG: {why}");
        self.emit(StoreEvent::Login(StoreLoginEvent {
            source: SOURCE.to_string(),
            outcome: "failed".to_string(),
            username: String::new(),
            error: why.to_string(),
        }));
        self.changed();
    }

    /// The redirect's code: signs in through gogdl, then installs gogdl
    /// when it is missing, then loads the owned list.
    pub async fn finish_login(self: &Arc<Self>, code: SecretString) {
        self.state.lock().unwrap().login = LoginPhase::Completing;
        let result = self.api.complete_login(code).await;
        match result {
            Ok(account) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.login = LoginPhase::Idle;
                    state.connection = StoreConnection::Connected;
                    state.username = account.username.clone();
                    state.last_error.clear();
                }
                let username = account.username.clone();
                self.write_gog_config(move |gog| {
                    gog.connected = true;
                    gog.username = username;
                });
                self.emit(StoreEvent::Login(StoreLoginEvent {
                    source: SOURCE.to_string(),
                    outcome: "connected".to_string(),
                    username: account.username.clone(),
                    error: String::new(),
                }));
                self.changed();
                self.after_connect().await;
            }
            Err(error) => {
                self.state.lock().unwrap().login = LoginPhase::Idle;
                self.fail_login(&error.to_string());
            }
        }
    }

    /// Right after a connect: gogdl when it is missing, then the owned list.
    async fn after_connect(self: &Arc<Self>) {
        if !self.api.tool_installed() {
            self.install_tool_then(false, true).await;
            if !self.tool_ready() {
                return;
            }
        } else {
            let phase = self.tool_phase_now();
            let ready = phase == ToolPhase::Installed;
            self.state.lock().unwrap().tool = phase;
            if !ready {
                self.changed();
                return;
            }
        }
        self.refresh_owned(true).await;
        self.refresh_cloud_states().await;
    }

    /// Disconnect: gogdl's folder (its token file with it), the login
    /// window's cookies and the owned cache go; installed games stay.
    pub fn disconnect(&self) -> Result<(), String> {
        if let Err(e) = self.api.disconnect() {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(format!("Couldn\u{2019}t delete GOG\u{2019}s login: {e}"));
            }
        }
        if let Err(e) = crate::store_auth::wipe_login_data(&self.data_dir, SOURCE) {
            tracing::warn!("stores: login window data not deleted: {e}");
        }
        let _ = std::fs::remove_file(owned_cache_path(&self.data_dir));
        self.write_gog_config(|gog| {
            gog.connected = false;
            gog.username.clear();
        });
        {
            let mut state = self.state.lock().unwrap();
            let login = state.login;
            *state = GogState {
                login,
                tool: self.tool_phase_now(),
                ..GogState::default()
            };
        }
        self.changed();
        Ok(())
    }

    // --- tool ---------------------------------------------------------------------

    /// Installs gogdl (pinned, SHA-256 checked), then loads the owned list
    /// when it never loaded. One at a time.
    pub async fn install_tool(self: &Arc<Self>) {
        self.install_tool_then(true, false).await;
    }

    /// `announce`: the "Installing gogdl…" card (after a connect; the
    /// frontend words its own Install button).
    async fn install_tool_then(self: &Arc<Self>, load_owned: bool, announce: bool) {
        {
            let mut state = self.state.lock().unwrap();
            if matches!(state.tool, ToolPhase::Installing { .. }) {
                return;
            }
            state.tool = ToolPhase::Installing { done: 0, total: 0 };
        }
        if let Some(problem) = self.api.tool_problem() {
            self.state.lock().unwrap().tool = ToolPhase::Failed(problem.clone());
            self.notify(notice::tool_failed(&problem));
            self.changed();
            return;
        }
        if announce {
            self.notify(notice::tool_installing(tool_size_bytes()));
        }
        self.changed();
        let this = self.clone();
        let mut last_emit = std::time::Instant::now();
        let progress = Box::new(move |done: u64, total: u64| {
            this.state.lock().unwrap().tool = ToolPhase::Installing { done, total };
            if last_emit.elapsed() >= std::time::Duration::from_millis(250) || done == total {
                last_emit = std::time::Instant::now();
                this.changed();
            }
        });
        let result = self.api.install_tool(progress).await;
        match result {
            Ok(()) => {
                let phase = self.tool_phase_now();
                if let ToolPhase::Failed(problem) = &phase {
                    let problem = problem.clone();
                    self.state.lock().unwrap().tool = phase;
                    self.notify(notice::tool_failed(&problem));
                    self.changed();
                    return;
                }
                self.state.lock().unwrap().tool = ToolPhase::Installed;
                self.notify(notice::tool_installed());
                self.changed();
                if load_owned && self.connection() == StoreConnection::Connected {
                    let owned_loaded = self.state.lock().unwrap().owned == OwnedState::Ready;
                    if !owned_loaded {
                        self.refresh_owned(true).await;
                    }
                }
            }
            Err(error) => {
                let message = error.to_string();
                self.state.lock().unwrap().tool = ToolPhase::Failed(message.clone());
                self.notify(notice::tool_failed(&message));
                self.changed();
            }
        }
    }

    // --- owned list ---------------------------------------------------------------

    /// Reloads the owned list. `announce`: the "N games found" card (after
    /// a connect, and on Refresh).
    pub async fn refresh_owned(self: &Arc<Self>, announce: bool) {
        {
            let mut state = self.state.lock().unwrap();
            if state.connection != StoreConnection::Connected || state.owned == OwnedState::Loading
            {
                return;
            }
            state.owned = OwnedState::Loading;
            state.owned_error.clear();
        }
        self.changed();
        let result = self.api.owned().await;
        match result {
            Ok(products) => {
                let installed: HashSet<String> = self
                    .installed_rows()
                    .into_iter()
                    .map(|row| row.store_id)
                    .collect();
                let to_install = products
                    .iter()
                    .filter(|product| !installed.contains(&product.id))
                    .count();
                let count = products.len();
                let at = now();
                self.owned_fetched
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                write_owned_cache(&self.data_dir, &products, at);
                {
                    let mut state = self.state.lock().unwrap();
                    state.products = products;
                    state.refreshed_at = at;
                    state.owned = OwnedState::Ready;
                }
                if announce {
                    self.notify(notice::games_found(count, to_install));
                }
            }
            Err(StoreError::LoginExpired) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.connection = StoreConnection::Expired;
                    state.owned = OwnedState::Error;
                    state.owned_error = StoreError::LoginExpired.to_string();
                }
                self.notify(notice::login_expired());
            }
            Err(error) => {
                let mut state = self.state.lock().unwrap();
                state.owned = OwnedState::Error;
                state.owned_error = error.to_string();
            }
        }
        self.changed();
    }

    /// Loads the owned list through GOG once this run (GogService keeps
    /// the products the install jobs read their details from; the disk
    /// cache only feeds the Library). Quiet: no card.
    pub async fn ensure_owned_fetched(self: &Arc<Self>) {
        if self.owned_fetched.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        self.refresh_owned(false).await;
    }

    /// The cached product for `id`.
    pub fn product(&self, id: &str) -> Option<GogProduct> {
        self.state
            .lock()
            .unwrap()
            .products
            .iter()
            .find(|product| product.id == id)
            .cloned()
    }

    // --- library ------------------------------------------------------------------

    /// GOG's Installed and Available lists. `updates`: newer versions by
    /// store id ([`crate::update_service`]).
    pub fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary {
        let config = self.gog_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap().clone();
        let status = assemble_status(&state, &config, &rows, &self.default_install_dir(&config));
        let syncing = self.syncing.lock().unwrap().clone();
        let (installed, available) = assemble_games(&state, &config, &rows, updates, &syncing);
        StoreLibrary {
            status,
            installed,
            available,
        }
    }

    // --- cloud saves --------------------------------------------------------------

    /// Whether GRID syncs this row's cloud saves: the store switch and the
    /// game's own.
    pub fn cloud_enabled(&self, row: &InstalledGame) -> bool {
        self.gog_config().cloud_saves && GogMeta::from_json(&row.store_meta).cloud_saves
    }

    /// Records a game's cloud state (the Library's cloud column).
    pub fn remember_cloud(&self, store_id: &str, cloud: &CloudState) {
        self.state
            .lock()
            .unwrap()
            .cloud
            .insert(store_id.to_string(), cloud.clone());
    }

    pub fn set_syncing(&self, store_id: &str, on: bool) {
        let mut syncing = self.syncing.lock().unwrap();
        if on {
            syncing.insert(store_id.to_string());
        } else {
            syncing.remove(store_id);
        }
    }

    pub fn is_syncing(&self, store_id: &str) -> bool {
        self.syncing.lock().unwrap().contains(store_id)
    }

    /// Syncs before a launch. `Err(conflict)` stops the launch; any other
    /// failure only logs (a store hiccup never blocks a game).
    pub async fn sync_before_launch(
        &self,
        registry: &Registry,
        row: &InstalledGame,
    ) -> Result<(), StoreSaveConflict> {
        if !self.cloud_enabled(row) || self.connection() != StoreConnection::Connected {
            return Ok(());
        }
        self.set_syncing(&row.store_id, true);
        let result = self.api.cloud_sync(registry, row, SyncMode::Auto).await;
        self.set_syncing(&row.store_id, false);
        match result {
            Ok(CloudState::Conflict(conflict)) => {
                self.remember_cloud(&row.store_id, &CloudState::Conflict(conflict.clone()));
                Err(conflict_ipc(&conflict))
            }
            Ok(state) => {
                self.remember_cloud(&row.store_id, &state);
                Ok(())
            }
            Err(e) => {
                tracing::info!("stores: cloud saves not synced before launch: {e}");
                Ok(())
            }
        }
    }

    /// Uploads after a session ends ("Cloud saves uploaded to GOG for …").
    pub async fn sync_after_session(&self, registry: &Registry, row: &InstalledGame) {
        if !self.cloud_enabled(row) || self.connection() != StoreConnection::Connected {
            return;
        }
        self.set_syncing(&row.store_id, true);
        let result = self.api.cloud_sync(registry, row, SyncMode::Auto).await;
        self.set_syncing(&row.store_id, false);
        match result {
            Ok(state @ CloudState::Synced { .. }) => {
                self.remember_cloud(&row.store_id, &state);
                self.notify(notice::cloud_uploaded(&row.title));
            }
            Ok(state @ CloudState::Conflict(_)) => {
                self.remember_cloud(&row.store_id, &state);
                self.notify(notice::cloud_conflict(&row.title));
            }
            Ok(state) => self.remember_cloud(&row.store_id, &state),
            Err(e) => {
                tracing::info!("stores: cloud saves not uploaded after the session: {e}");
                self.notify(notice::cloud_failed(&row.title, &e.to_string()));
            }
        }
        self.changed();
    }

    /// Every Downloads snapshot: a GOG job that just finished gets its card
    /// ("Grey Meridian installed" · Play, "… updated", "Verified …",
    /// "… installed from Couch kit"), a failed one its error card, and the
    /// store's counts change.
    pub fn observe_downloads(&self, snapshot: &DownloadsSnapshot) {
        self.observe_rows(
            snapshot
                .entries
                .iter()
                .map(|entry| JobRow {
                    id: entry.id,
                    store: entry.job == "store",
                    kind: entry.kind,
                    key: entry.source_id.clone(),
                    title: entry.title.clone(),
                    completed: matches!(entry.status, DownloadStatus::Completed),
                    failed: matches!(entry.status, DownloadStatus::Failed),
                    error: entry.error.clone(),
                    drive: entry.drive_name.clone(),
                    needs_wizard: entry.failure.as_ref().is_some_and(|failure| {
                        failure.code == grid_core::library::queue::FailureCode::InstallerNeedsWizard
                    }),
                    base: entry.failure.as_ref().and_then(|failure| {
                        (!failure.base_gog_id.is_empty()).then(|| {
                            (
                                failure.base_title.clone(),
                                GameKey::store(StoreKind::Gog, failure.base_gog_id.clone())
                                    .to_string(),
                            )
                        })
                    }),
                })
                .collect(),
        );
    }

    fn observe_rows(&self, rows: Vec<JobRow>) {
        let mut finished = Vec::new();
        {
            let mut announced = self.announced.lock().unwrap();
            for entry in &rows {
                if !entry.store
                    || !entry.key.starts_with("gog:")
                    || !(entry.completed || entry.failed)
                {
                    continue;
                }
                if announced.insert(entry.id) {
                    finished.push(entry);
                }
            }
            // Dismissed rows leave the snapshot; forget them.
            let live: HashSet<u64> = rows.iter().map(|entry| entry.id).collect();
            announced.retain(|id| live.contains(id));
        }
        for entry in &finished {
            // An installer that needs its setup window fails only where GRID
            // may not open one (TV mode), which words that card itself
            // ("Couldn't install <title> silently" · Retry).
            if entry.failed && entry.needs_wizard {
                continue;
            }
            let card = if entry.completed {
                notice::finished(entry.kind, &entry.title, &entry.key, &entry.drive)
            } else {
                notice::failed(
                    entry.kind,
                    &entry.title,
                    &entry.error,
                    entry.base.as_ref().map(|(t, k)| (t.as_str(), k.as_str())),
                )
            };
            if let Some(card) = card {
                self.notify(card);
            }
        }
        if !finished.is_empty() {
            self.changed();
        }
    }

    // --- notices other modules raise -------------------------------------------

    pub fn notice(&self, notice: StoreNotice) {
        self.notify(notice);
    }

    /// Sets the per-store cloud switch and says so.
    pub fn set_cloud_saves(&self, on: bool) {
        self.write_gog_config(|gog| gog.cloud_saves = on);
        self.changed();
    }

    pub fn set_install_dir(&self, path: &str) {
        let path = path.trim().to_string();
        self.write_gog_config(move |gog| gog.install_dir = path);
        self.changed();
    }

    /// Refuses an install while signed out or without gogdl, with the card
    /// that says why.
    pub fn ensure_can_install(&self) -> Result<(), String> {
        let (connection, tool) = {
            let state = self.state.lock().unwrap();
            (state.connection, state.tool.clone())
        };
        match connection {
            StoreConnection::Connected => {}
            StoreConnection::Expired => return Err(StoreError::LoginExpired.to_string()),
            StoreConnection::NotConnected => return Err(StoreError::NotSignedIn.to_string()),
        }
        if let ToolPhase::Failed(problem) = &tool {
            return Err(problem.clone());
        }
        if tool != ToolPhase::Installed {
            return Err(StoreError::ToolMissing(TOOL_NAME).to_string());
        }
        Ok(())
    }

    /// The `StoreError` a command returns, with the login state kept in
    /// step: an expired login flips the card to Login expired.
    pub fn store_error(&self, error: StoreError) -> String {
        if matches!(error, StoreError::LoginExpired) {
            let changed = {
                let mut state = self.state.lock().unwrap();
                let changed = state.connection == StoreConnection::Connected;
                state.connection = StoreConnection::Expired;
                changed
            };
            if changed {
                self.changed();
            }
        }
        error.to_string()
    }
}

// --- the commands' GOG work (StoreBackend) ---------------------------------------

/// The GOG version a request names.
pub(crate) fn parse_platform(platform: &str) -> Result<GogPlatform, String> {
    match platform {
        "linux" => Ok(GogPlatform::Linux),
        "windows" => Ok(GogPlatform::Windows),
        _ => Err("GRID installs the Linux or Windows version only.".to_string()),
    }
}

/// The row after Game settings' Save: a new executable becomes the user's
/// choice (GOG updates leave it alone); the install folder and the Wine
/// prefix are read-only, and only a Windows version takes a compat tool.
pub(crate) fn apply_game_settings(
    mut row: InstalledGame,
    settings: &StoreGameSettings,
) -> InstalledGame {
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

impl GogStore {
    /// The login window's GOG spec: GOG's login page, ended by the
    /// `embed.gog.com/on_login_success?code=` redirect.
    pub fn login_spec(&self) -> LoginSpec {
        let finish = self
            .endpoints
            .embed
            .join(crate::store_auth::GOG_LOGIN_SUCCESS_PATH)
            .unwrap_or_else(|_| self.endpoints.embed.clone());
        LoginSpec {
            store_name: StoreKind::Gog.platform_name().to_string(),
            login_url: self.api.login_url(),
            finish,
            finish_query: None,
            code_param: "code",
            user_agent: None,
        }
    }

    /// `store_install_options` for GOG.
    async fn install_options_for(
        self: &Arc<Self>,
        id: &str,
    ) -> Result<StoreInstallOptions, String> {
        self.ensure_can_install()?;
        self.ensure_owned_fetched().await;
        let options = self
            .api
            .install_options(id)
            .await
            .map_err(|e| self.store_error(e))?;
        let title = self
            .product(id)
            .map(|p| p.title)
            .unwrap_or_else(|| id.to_string());
        let compat = Config::load(&self.config_path)
            .map(|config| config.default_compat_tool)
            .unwrap_or_default();
        let base = self.install_base_dir();
        tokio::task::spawn_blocking(move || install_options_ipc(&options, &title, &base, &compat))
            .await
            .map_err(|e| e.to_string())
    }

    /// `store_install` for GOG: the job for a confirmed install.
    async fn install_job_for(
        self: &Arc<Self>,
        id: String,
        request: StoreInstallRequest,
        ctx: JobCtx,
    ) -> Result<StoreJob, String> {
        self.ensure_can_install()?;
        let platform = parse_platform(&request.platform)?;
        self.ensure_owned_fetched().await;
        let title = self
            .product(&id)
            .map(|p| p.title)
            .unwrap_or_else(|| id.clone());
        let install_dir = if request.install_dir.trim().is_empty() {
            let base = self.install_base_dir();
            if base.is_empty() {
                return Err("Choose a library folder in Settings › Library first.".to_string());
            }
            Path::new(&base).join(grid_core::library::paths::sanitize_component(
                &title, "game",
            ))
        } else {
            PathBuf::from(request.install_dir.trim())
        };
        Ok(self.api.install_job(
            GogInstallRequest {
                store_id: id,
                title,
                platform,
                language: request.language,
                dlc_ids: request.dlc_ids,
                install_dir,
                compat_tool: request.compat_tool,
            },
            ctx.gog(),
        ))
    }

    /// `store_cloud_state` for GOG.
    async fn cloud_state_for(&self, row: &InstalledGame) -> Result<StoreCloudState, String> {
        let meta = GogMeta::from_json(&row.store_meta);
        if !self.cloud_enabled(row) {
            return Ok(cloud_state_ipc(&CloudState::Off, &meta, false));
        }
        if self.is_syncing(&row.store_id) {
            return Ok(cloud_state_ipc(
                &CloudState::Synced { at: meta.last_sync },
                &meta,
                true,
            ));
        }
        let cloud = self
            .api
            .cloud_state(row)
            .await
            .map_err(|e| self.store_error(e))?;
        self.remember_cloud(&row.store_id, &cloud);
        Ok(cloud_state_ipc(&cloud, &meta, false))
    }

    /// `store_cloud_sync` / `store_cloud_resolve` for GOG: syncs in `mode`,
    /// then answers the state with the row re-read (its `last_sync` moved).
    async fn cloud_sync_for(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        mode: SyncMode,
    ) -> Result<StoreCloudState, String> {
        if !self.cloud_enabled(row) {
            return Ok(cloud_state_ipc(
                &CloudState::Off,
                &GogMeta::from_json(&row.store_meta),
                false,
            ));
        }
        self.set_syncing(&row.store_id, true);
        self.changed();
        let result = self.api.cloud_sync(registry, row, mode).await;
        self.set_syncing(&row.store_id, false);
        let cloud = result.map_err(|e| self.store_error(e));
        if let Ok(cloud) = &cloud {
            self.remember_cloud(&row.store_id, cloud);
        }
        self.changed();
        let key = GameKey::store(StoreKind::Gog, row.store_id.clone());
        let fresh = registry
            .find_key(&key)
            .ok()
            .flatten()
            .unwrap_or_else(|| row.clone());
        Ok(cloud_state_ipc(
            &cloud?,
            &GogMeta::from_json(&fresh.store_meta),
            false,
        ))
    }
}

impl StoreBackend for GogStore {
    fn kind(&self) -> StoreKind {
        StoreKind::Gog
    }
    fn status(&self) -> StoreStatus {
        GogStore::status(self)
    }
    fn connection(&self) -> StoreConnection {
        GogStore::connection(self)
    }
    fn changed(&self) {
        GogStore::changed(self)
    }
    fn load(&self) {
        GogStore::load(self)
    }
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { GogStore::check_login(&self).await })
    }
    fn observe_downloads(&self, snapshot: &DownloadsSnapshot) {
        GogStore::observe_downloads(self, snapshot)
    }
    fn login_spec(&self) -> LoginSpec {
        GogStore::login_spec(self)
    }
    fn begin_login(&self) -> Result<(), String> {
        GogStore::begin_login(self)
    }
    fn login_window_failed(&self, error: &str) {
        GogStore::login_window_failed(self, error)
    }
    fn login_cancelled(&self) {
        GogStore::login_cancelled(self)
    }
    fn login_redirect_failed(&self) {
        GogStore::login_redirect_failed(self)
    }
    fn run_finish_login(self: Arc<Self>, code: SecretString) -> BoxFut<'static, ()> {
        Box::pin(async move { GogStore::finish_login(&self, code).await })
    }
    fn disconnect(&self) -> Result<(), String> {
        GogStore::disconnect(self)
    }
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { GogStore::install_tool(&self).await })
    }
    fn run_refresh(self: Arc<Self>, announce: bool) -> BoxFut<'static, ()> {
        Box::pin(async move { GogStore::refresh_owned(&self, announce).await })
    }
    fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary {
        GogStore::library(self, updates)
    }
    fn check_updates(self: Arc<Self>, app: tauri::AppHandle) {
        use tauri::Manager as _;
        let updates = app.state::<crate::commands::AppState>().updates.clone();
        updates.spawn_store_check(app, self);
    }
    fn set_install_dir(&self, path: &str) {
        GogStore::set_install_dir(self, path)
    }
    fn set_cloud_saves(&self, on: bool) {
        GogStore::set_cloud_saves(self, on)
    }
    fn ensure_can_install(&self) -> Result<(), String> {
        GogStore::ensure_can_install(self)
    }
    fn install_options(
        self: Arc<Self>,
        store_id: String,
    ) -> BoxFut<'static, Result<StoreInstallOptions, String>> {
        Box::pin(async move { self.install_options_for(&store_id).await })
    }
    fn install_job(
        self: Arc<Self>,
        store_id: String,
        request: StoreInstallRequest,
        ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async move { self.install_job_for(store_id, request, ctx).await })
    }
    fn update_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.api.update_job(row, ctx.gog())
    }
    fn repair_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.api.repair_job(row, ctx.gog())
    }
    fn dlc_job(
        self: Arc<Self>,
        row: InstalledGame,
        dlc_id: String,
        ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async move {
            self.ensure_owned_fetched().await;
            Ok(self.api.dlc_job(&row, &dlc_id, ctx.gog()))
        })
    }
    fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc_id: &str,
    ) -> Result<(), StoreError> {
        self.api.remove_dlc(registry, row, dlc_id)
    }
    fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
        self.api.uninstall(registry, store_id)
    }
    fn game_settings(&self, row: &InstalledGame) -> StoreGameSettings {
        let meta = GogMeta::from_json(&row.store_meta);
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
        row: InstalledGame,
        settings: &StoreGameSettings,
    ) -> InstalledGame {
        apply_game_settings(row, settings)
    }
    fn windows_version(&self, row: &InstalledGame) -> bool {
        GogMeta::from_json(&row.store_meta).platform == GogPlatform::Windows
    }
    fn cloud_enabled(&self, row: &InstalledGame) -> bool {
        GogStore::cloud_enabled(self, row)
    }
    fn cloud_state<'a>(
        &'a self,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<StoreCloudState, String>> {
        Box::pin(self.cloud_state_for(row))
    }
    fn cloud_sync<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
        mode: SyncMode,
    ) -> BoxFut<'a, Result<StoreCloudState, String>> {
        Box::pin(self.cloud_sync_for(registry, row, mode))
    }
    fn sync_before_launch<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<(), StoreSaveConflict>> {
        Box::pin(GogStore::sync_before_launch(self, registry, row))
    }
    fn sync_after_session<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, ()> {
        Box::pin(GogStore::sync_after_session(self, registry, row))
    }
    fn is_art_url(&self, url: &str) -> bool {
        GogStore::is_art_url(self, url)
    }
}

/// The parts of a Downloads entry [`GogStore::observe_downloads`]
/// reads.
struct JobRow {
    id: u64,
    store: bool,
    kind: &'static str,
    key: String,
    title: String,
    completed: bool,
    failed: bool,
    error: String,
    drive: String,
    /// The installer needed its setup window, which GRID may not open here.
    needs_wizard: bool,
    /// A DLC installer's missing base game: its title and key.
    base: Option<(String, String)>,
}

/// The tool's download size for the cards ("Installing gogdl (12 MB)…").
fn tool_size_bytes() -> u64 {
    tools::host_asset(StoreTool::Gogdl)
        .map(|asset| asset.size)
        .unwrap_or(0)
}

// --- status and library assembly (pure) --------------------------------------

fn assemble_status(
    state: &GogState,
    config: &GogConfig,
    rows: &[InstalledGame],
    install_dir: &str,
) -> StoreStatus {
    let pin = tools::pin(StoreTool::Gogdl);
    let (tool_state, error, done, total) = match &state.tool {
        ToolPhase::NotInstalled => (ToolState::NotInstalled, String::new(), 0, 0),
        ToolPhase::Installing { done, total } => {
            (ToolState::Installing, String::new(), *done, *total)
        }
        ToolPhase::Installed => (ToolState::Installed, String::new(), 0, 0),
        ToolPhase::Failed(message) => (ToolState::Failed, message.clone(), 0, 0),
    };
    let installed_count = rows.len() as u32;
    let installed_bytes = rows
        .iter()
        .map(|row| row.filesize_bytes.max(0) as u64)
        .sum();
    let owned_count = state
        .products
        .iter()
        .filter(|product| !product.id.is_empty())
        .count() as u32;
    let in_rail = state.connection != StoreConnection::NotConnected || installed_count > 0;
    StoreStatus {
        source: SOURCE.to_string(),
        connection: state.connection,
        username: if state.connection == StoreConnection::NotConnected {
            String::new()
        } else if state.username.is_empty() {
            config.username.clone()
        } else {
            state.username.clone()
        },
        tool: ToolStatus {
            name: TOOL_NAME.to_string(),
            version: pin.version.to_string(),
            state: tool_state,
            size_bytes: tool_size_bytes(),
            error,
            license: pin.license.to_string(),
            source_url: pin.source_url.to_string(),
            done,
            total,
        },
        owned: state.owned,
        owned_error: state.owned_error.clone(),
        owned_count,
        installed_count,
        installed_bytes,
        refreshed_at: state.refreshed_at,
        cloud_saves: config.cloud_saves,
        install_dir: install_dir.to_string(),
        in_rail,
        last_error: state.last_error.clone(),
        steam: None,
    }
}

fn platform_name(platform: GogPlatform) -> &'static str {
    match platform {
        GogPlatform::Linux => "linux",
        GogPlatform::Windows => "windows",
    }
}

fn origin_name(origin: GogOrigin) -> &'static str {
    match origin {
        GogOrigin::Gogdl => "gogdl",
        GogOrigin::Offline => "offline",
        GogOrigin::Drive => "drive",
    }
}

fn year_of(date: &str) -> String {
    date.get(..4)
        .filter(|year| year.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or("")
        .to_string()
}

fn split_list(text: &str) -> Vec<String> {
    text.split([',', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// The cloud column: `off` when either switch is off, else what GRID last
/// saw (`unknown` before the first look).
fn cloud_column(store_on: bool, meta: &GogMeta, seen: Option<&CloudState>) -> &'static str {
    if !store_on || !meta.cloud_saves {
        return "off";
    }
    match seen {
        Some(CloudState::Off) => "off",
        Some(CloudState::None) => "none",
        Some(CloudState::Synced { .. }) => "synced",
        Some(CloudState::Conflict(_)) => "conflict",
        None if meta.last_sync > 0 => "synced",
        None => "unknown",
    }
}

fn dlc_ipc(dlc: &[GogProductDlc], meta: Option<&GogMeta>) -> Vec<StoreDlc> {
    dlc.iter()
        .map(|item| StoreDlc {
            id: item.id.clone(),
            title: item.title.clone(),
            download_bytes: 0,
            installed: meta.is_some_and(|meta| meta.dlc.iter().any(|d| d.id == item.id)),
        })
        .collect()
}

fn assemble_games(
    state: &GogState,
    config: &GogConfig,
    rows: &[InstalledGame],
    updates: &HashMap<String, String>,
    syncing: &HashSet<String>,
) -> (Vec<StoreGame>, Vec<StoreGame>) {
    let products: HashMap<&str, &GogProduct> = state
        .products
        .iter()
        .map(|product| (product.id.as_str(), product))
        .collect();
    let installed: Vec<StoreGame> = rows
        .iter()
        .map(|row| {
            let meta = GogMeta::from_json(&row.store_meta);
            let product = products.get(row.store_id.as_str()).copied();
            let mut cloud = cloud_column(config.cloud_saves, &meta, state.cloud.get(&row.store_id));
            if syncing.contains(&row.store_id) && cloud != "off" {
                cloud = "synced";
            }
            let mut dlc = product
                .map(|product| dlc_ipc(&product.dlc, Some(&meta)))
                .unwrap_or_default();
            for owned in &meta.dlc {
                if !dlc.iter().any(|d| d.id == owned.id) {
                    dlc.push(StoreDlc {
                        id: owned.id.clone(),
                        title: owned.title.clone(),
                        download_bytes: 0,
                        installed: true,
                    });
                }
            }
            StoreGame {
                key: GameKey::store(StoreKind::Gog, row.store_id.clone()).to_string(),
                source: SOURCE.to_string(),
                store_id: row.store_id.clone(),
                title: row.title.clone(),
                installed: true,
                cover: product
                    .map(|product| product.cover.clone())
                    .filter(|cover| !cover.is_empty())
                    .unwrap_or_else(|| row.cover_large_path.clone()),
                screenshots: product
                    .map(|product| product.screenshots.clone())
                    .unwrap_or_else(|| split_list(&row.screenshot_urls)),
                description: row.description.clone(),
                developer: product
                    .map(|product| product.developer.clone())
                    .unwrap_or_else(|| {
                        split_list(&row.companies)
                            .into_iter()
                            .next()
                            .unwrap_or_default()
                    }),
                genres: split_list(&row.genres),
                year: year_of(&row.first_release_date),
                platforms: product
                    .map(|product| {
                        product
                            .platforms
                            .iter()
                            .map(|p| platform_name(*p).to_string())
                            .collect()
                    })
                    .unwrap_or_else(|| vec![platform_name(meta.platform).to_string()]),
                installed_platform: Some(platform_name(meta.platform).to_string()),
                download_bytes: 0,
                disk_bytes: row.filesize_bytes.max(0) as u64,
                version: meta.version.clone(),
                update_version: updates.get(&row.store_id).cloned().unwrap_or_default(),
                origin: Some(origin_name(meta.origin).to_string()),
                not_supported: String::new(),
                dlc,
                cloud: cloud.to_string(),
                last_sync: meta.last_sync,
                install_dir: row.native_game_dir.clone(),
                compat_tool: row.native_compat_tool.clone(),
            }
        })
        .collect();
    let installed_ids: HashSet<&str> = rows.iter().map(|row| row.store_id.as_str()).collect();
    let available = state
        .products
        .iter()
        .filter(|product| !installed_ids.contains(product.id.as_str()))
        .map(|product| {
            let not_supported = if product.platforms.is_empty() {
                "GOG offers no Linux or Windows version of this game.".to_string()
            } else {
                String::new()
            };
            StoreGame {
                key: GameKey::store(StoreKind::Gog, product.id.clone()).to_string(),
                source: SOURCE.to_string(),
                store_id: product.id.clone(),
                title: product.title.clone(),
                installed: false,
                cover: product.cover.clone(),
                screenshots: product.screenshots.clone(),
                description: product.description.clone(),
                developer: product.developer.clone(),
                genres: product.genres.clone(),
                year: year_of(&product.release_date),
                platforms: product
                    .platforms
                    .iter()
                    .map(|p| platform_name(*p).to_string())
                    .collect(),
                installed_platform: None,
                download_bytes: 0,
                disk_bytes: 0,
                version: String::new(),
                update_version: String::new(),
                origin: None,
                not_supported,
                dlc: dlc_ipc(&product.dlc, None),
                cloud: "unknown".to_string(),
                last_sync: 0,
                install_dir: String::new(),
                compat_tool: String::new(),
            }
        })
        .collect();
    (installed, available)
}

/// `StoreInstallOptions` for the install dialog. Linux is recommended when
/// GOG offers it; the folder is `<base>/<title>`.
pub fn install_options_ipc(
    options: &GogInstallOptions,
    title: &str,
    base_dir: &str,
    compat_tool: &str,
) -> StoreInstallOptions {
    let has_linux = options
        .versions
        .iter()
        .any(|version| version.platform == GogPlatform::Linux);
    let install_dir = if base_dir.is_empty() {
        String::new()
    } else {
        Path::new(base_dir)
            .join(grid_core::library::paths::sanitize_component(title, "game"))
            .to_string_lossy()
            .into_owned()
    };
    let probe = if install_dir.is_empty() {
        None
    } else {
        Some(PathBuf::from(&install_dir))
    };
    let free_bytes = probe
        .as_deref()
        .and_then(grid_core::library::relocate::free_space)
        .unwrap_or(0);
    let volume = probe
        .as_deref()
        .and_then(|path| path.ancestors().find(|dir| dir.is_dir()))
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default();
    StoreInstallOptions {
        versions: options
            .versions
            .iter()
            .map(|version| StoreVersionOption {
                platform: platform_name(version.platform).to_string(),
                recommended: version.platform == GogPlatform::Linux
                    || (!has_linux && version.platform == GogPlatform::Windows),
                download_bytes: version.download_bytes,
                disk_bytes: version.disk_bytes,
            })
            .collect(),
        languages: options.languages.clone(),
        default_language: options.default_language.clone(),
        dlc: options
            .dlc
            .iter()
            .map(|dlc| StoreDlc {
                id: dlc.id.clone(),
                title: dlc.title.clone(),
                download_bytes: dlc.download_bytes,
                installed: false,
            })
            .collect(),
        install_dir,
        free_bytes,
        volume,
        compat_tool: compat_tool.to_string(),
    }
}

/// A GOG cloud state as the UI sees it.
pub fn cloud_state_ipc(state: &CloudState, meta: &GogMeta, syncing: bool) -> StoreCloudState {
    let locations = meta
        .save_locations
        .iter()
        .map(|location| location.path.clone())
        .collect();
    let (name, synced_at, conflict) = match state {
        CloudState::Off => ("off", 0, None),
        CloudState::None => ("none", 0, None),
        CloudState::Synced { at } => ("synced", *at, None),
        CloudState::Conflict(conflict) => {
            ("conflict", meta.last_sync, Some(conflict_ipc(conflict)))
        }
    };
    StoreCloudState {
        state: if syncing && name != "off" {
            "syncing"
        } else {
            name
        }
        .to_string(),
        synced_at,
        locations,
        conflict,
    }
}

pub(crate) fn conflict_ipc(conflict: &grid_core::stores::SaveConflict) -> StoreSaveConflict {
    StoreSaveConflict {
        local_at: conflict.local_at,
        local_files: conflict.local_files,
        local_bytes: conflict.local_bytes,
        cloud_at: conflict.cloud_at,
        cloud_files: conflict.cloud_files,
        cloud_bytes: conflict.cloud_bytes,
    }
}

// --- owned cache ------------------------------------------------------------------

/// One cached product: metadata only.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct CachedProduct {
    id: String,
    title: String,
    cover: String,
    screenshots: Vec<String>,
    description: String,
    developer: String,
    publisher: String,
    genres: Vec<String>,
    release_date: String,
    platforms: Vec<String>,
    dlc: Vec<(String, String)>,
}

impl From<&GogProduct> for CachedProduct {
    fn from(product: &GogProduct) -> Self {
        CachedProduct {
            id: product.id.clone(),
            title: product.title.clone(),
            cover: product.cover.clone(),
            screenshots: product.screenshots.clone(),
            description: product.description.clone(),
            developer: product.developer.clone(),
            publisher: product.publisher.clone(),
            genres: product.genres.clone(),
            release_date: product.release_date.clone(),
            platforms: product
                .platforms
                .iter()
                .map(|p| platform_name(*p).to_string())
                .collect(),
            dlc: product
                .dlc
                .iter()
                .map(|dlc| (dlc.id.clone(), dlc.title.clone()))
                .collect(),
        }
    }
}

impl From<CachedProduct> for GogProduct {
    fn from(cached: CachedProduct) -> Self {
        GogProduct {
            id: cached.id,
            title: cached.title,
            cover: cached.cover,
            screenshots: cached.screenshots,
            description: cached.description,
            developer: cached.developer,
            publisher: cached.publisher,
            genres: cached.genres,
            release_date: cached.release_date,
            platforms: cached
                .platforms
                .iter()
                .filter_map(|p| match p.as_str() {
                    "linux" => Some(GogPlatform::Linux),
                    "windows" => Some(GogPlatform::Windows),
                    _ => None,
                })
                .collect(),
            dlc: cached
                .dlc
                .into_iter()
                .map(|(id, title)| GogProductDlc { id, title })
                .collect(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct OwnedCache {
    refreshed_at: i64,
    products: Vec<CachedProduct>,
}

fn owned_cache_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(grid_core::stores::paths::STORES_DIR)
        .join("cache")
        .join(OWNED_CACHE_FILE)
}

fn read_owned_cache(data_dir: &Path) -> Option<OwnedCache> {
    let text = std::fs::read_to_string(owned_cache_path(data_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_owned_cache(data_dir: &Path, products: &[GogProduct], refreshed_at: i64) {
    let path = owned_cache_path(data_dir);
    let cache = OwnedCache {
        refreshed_at,
        products: products.iter().map(CachedProduct::from).collect(),
    };
    let write = || -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(&cache).map_err(io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)
    };
    if let Err(e) = write() {
        tracing::warn!("stores: owned list not cached: {e}");
    }
}

// --- notices --------------------------------------------------------------------

/// The `store-notice` cards, in the GameStores card's words. Only what
/// happens in the background: the frontend raises the cards for the
/// commands it calls itself (queued, refusals, sign-in outcome from
/// `store-login`, cloud choices, uninstalled, disconnected).
pub mod notice {
    use super::{StoreNotice, StoreNoticeAction, STORE_NAME, TOOL_NAME};

    fn card(message: String, error: bool, action: Option<StoreNoticeAction>) -> StoreNotice {
        StoreNotice {
            source: Some(super::SOURCE.to_string()),
            message,
            detail: String::new(),
            error,
            action,
        }
    }

    fn action(kind: &str, key: Option<&str>, title: Option<&str>) -> Option<StoreNoticeAction> {
        Some(StoreNoticeAction {
            kind: kind.to_string(),
            key: key.map(str::to_string),
            title: title.map(str::to_string),
        })
    }

    /// "12 MB", "1.1 GB"; `""` for an unknown size.
    pub fn size_label(bytes: u64) -> String {
        const MB: f64 = 1_000_000.0;
        if bytes == 0 {
            return String::new();
        }
        let mb = bytes as f64 / MB;
        if mb >= 1000.0 {
            format!("{:.1} GB", mb / 1000.0)
        } else {
            format!("{} MB", mb.round().max(1.0) as u64)
        }
    }

    /// gogdl starts installing right after a connect.
    pub fn tool_installing(size: u64) -> StoreNotice {
        let size = size_label(size);
        let size = if size.is_empty() {
            String::new()
        } else {
            format!(" ({size})")
        };
        card(
            format!("Installing {TOOL_NAME}{size} for {STORE_NAME}\u{2026}"),
            false,
            None,
        )
    }

    pub fn tool_installed() -> StoreNotice {
        card(format!("{TOOL_NAME} installed"), false, None)
    }

    pub fn tool_failed(why: &str) -> StoreNotice {
        card(
            format!("Couldn\u{2019}t install {TOOL_NAME}: {why}"),
            true,
            action("install_tool", None, None),
        )
    }

    /// The owned list loaded after a connect (or a Refresh).
    pub fn games_found(count: usize, to_install: usize) -> StoreNotice {
        let games = if count == 1 { "game" } else { "games" };
        card(
            format!("{STORE_NAME}: {count} {games} found \u{b7} {to_install} to install"),
            false,
            None,
        )
    }

    /// GRID found the login expired in the background.
    pub fn login_expired() -> StoreNotice {
        card(
            format!("Your {STORE_NAME} login expired. Sign in again to install and update games."),
            true,
            action("sign_in", None, None),
        )
    }

    pub fn installed(title: &str, key: &str) -> StoreNotice {
        card(
            format!("{title} installed"),
            false,
            action("play", Some(key), Some(title)),
        )
    }

    pub fn update_available(title: &str, version: &str) -> StoreNotice {
        let version = if version.is_empty() {
            "an update".to_string()
        } else {
            format!("update {version}")
        };
        card(
            format!("{title}: {version} is available on {STORE_NAME}"),
            false,
            action("show_updates", None, None),
        )
    }

    pub fn cloud_uploaded(title: &str) -> StoreNotice {
        card(
            format!("Cloud saves uploaded to {STORE_NAME} for {title}"),
            false,
            None,
        )
    }

    pub fn cloud_conflict(title: &str) -> StoreNotice {
        card(
            format!("{title}\u{2019}s save on this computer and its cloud save both changed. Choose which to keep in its Cloud saves."),
            true,
            None,
        )
    }

    pub fn cloud_failed(title: &str, why: &str) -> StoreNotice {
        card(
            format!("Couldn\u{2019}t sync {title}\u{2019}s cloud saves: {why}"),
            true,
            None,
        )
    }

    /// The card for a GOG Downloads row that just finished, by its kind.
    /// `drive` names the drive a restore copied from.
    pub fn finished(kind: &str, title: &str, key: &str, drive: &str) -> Option<StoreNotice> {
        match kind {
            "store_install" | "store_installer" | "store_dlc" => Some(installed(title, key)),
            "store_restore" => {
                let from = if drive.is_empty() {
                    "a backup drive".to_string()
                } else {
                    drive.to_string()
                };
                Some(card(
                    format!("{title} installed from {from}"),
                    false,
                    action("play", Some(key), Some(title)),
                ))
            }
            "store_update" => Some(card(
                format!("{title} updated"),
                false,
                action("play", Some(key), Some(title)),
            )),
            "store_repair" => Some(card(
                format!("Verified {title}\u{2019}s files"),
                false,
                None,
            )),
            _ => None,
        }
    }

    /// The card for a GOG Downloads row that just failed. An installer's
    /// DLC-without-base failure offers to install the base game.
    pub fn failed(
        kind: &str,
        title: &str,
        error: &str,
        base: Option<(&str, &str)>,
    ) -> Option<StoreNotice> {
        let what = match kind {
            "store_install" | "store_installer" | "store_dlc" | "store_restore" => "install",
            "store_update" => "update",
            "store_repair" => "verify",
            _ => return None,
        };
        let action = match base {
            Some((base_title, base_key)) => {
                action("install_base", Some(base_key), Some(base_title))
            }
            None => action("open_downloads", None, None),
        };
        let mut notice = card(format!("Couldn\u{2019}t {what} {title}"), true, action);
        notice.detail = error.to_string();
        Some(notice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::library::queue::{StoreJobRunner, StoreOp, StoreProgressSink};
    use grid_core::library::LibraryError;
    use grid_core::stores::gog::{GogDlcOption, GogVersionOption, SaveConflict};
    use std::sync::atomic::AtomicBool;

    /// A scripted GOG.
    #[derive(Default)]
    struct FakeGog {
        login: Mutex<Option<Result<GogAccount, String>>>,
        account: Mutex<Option<Result<Option<GogAccount>, &'static str>>>,
        owned: Mutex<Option<Result<Vec<GogProduct>, &'static str>>>,
        tool: Mutex<bool>,
        tool_result: Mutex<Option<Result<(), String>>>,
        sync: Mutex<Option<CloudState>>,
        disconnects: Mutex<u32>,
        problem: Mutex<Option<String>>,
    }

    fn err_of(text: &str) -> StoreError {
        match text {
            "expired" => StoreError::LoginExpired,
            other => StoreError::Network(other.to_string()),
        }
    }

    impl GogApi for FakeGog {
        fn login_url(&self) -> Url {
            Url::parse("http://127.0.0.1:1/auth").unwrap()
        }
        fn complete_login(
            &self,
            _code: SecretString,
        ) -> BoxFut<'_, Result<GogAccount, StoreError>> {
            let result = self
                .login
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Err("no script".into()));
            Box::pin(async move {
                result.map_err(|e| StoreError::ToolFailed {
                    tool: "gogdl",
                    reason: e,
                })
            })
        }
        fn account(&self) -> BoxFut<'_, Result<Option<GogAccount>, StoreError>> {
            let result = self.account.lock().unwrap().take().unwrap_or(Ok(None));
            Box::pin(async move { result.map_err(err_of) })
        }
        fn disconnect(&self) -> io::Result<()> {
            *self.disconnects.lock().unwrap() += 1;
            *self.tool.lock().unwrap() = false;
            Ok(())
        }
        fn owned(&self) -> BoxFut<'_, Result<Vec<GogProduct>, StoreError>> {
            let result = self.owned.lock().unwrap().take().unwrap_or(Ok(Vec::new()));
            Box::pin(async move { result.map_err(err_of) })
        }
        fn tool_installed(&self) -> bool {
            *self.tool.lock().unwrap()
        }
        fn tool_problem(&self) -> Option<String> {
            self.problem.lock().unwrap().clone()
        }
        fn install_tool<'a>(
            &'a self,
            mut progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
        ) -> BoxFut<'a, Result<(), StoreError>> {
            let result = self.tool_result.lock().unwrap().take().unwrap_or(Ok(()));
            Box::pin(async move {
                progress(50, 100);
                progress(100, 100);
                match result {
                    Ok(()) => {
                        *self.tool.lock().unwrap() = true;
                        Ok(())
                    }
                    Err(e) => Err(StoreError::Refused(e)),
                }
            })
        }
        fn install_options<'a>(
            &'a self,
            _id: &'a str,
        ) -> BoxFut<'a, Result<GogInstallOptions, StoreError>> {
            Box::pin(async { Ok(GogInstallOptions::default()) })
        }
        fn install_job(&self, req: GogInstallRequest, _ctx: GogJobCtx) -> StoreJob {
            job(req.store_id, StoreOp::Install)
        }
        fn update_job(&self, row: &InstalledGame, _ctx: GogJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Update)
        }
        fn repair_job(&self, row: &InstalledGame, _ctx: GogJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Repair)
        }
        fn dlc_job(&self, row: &InstalledGame, _dlc: &str, _ctx: GogJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Dlc)
        }
        fn remove_dlc(
            &self,
            _r: &Registry,
            _row: &InstalledGame,
            _d: &str,
        ) -> Result<(), StoreError> {
            Ok(())
        }
        fn uninstall(&self, _r: &Registry, _id: &str) -> Result<(), StoreError> {
            Ok(())
        }
        fn check_updates<'a>(
            &'a self,
            _rows: &'a [InstalledGame],
        ) -> BoxFut<'a, Result<Vec<GogUpdate>, StoreError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn cloud_state<'a>(
            &'a self,
            _row: &'a InstalledGame,
        ) -> BoxFut<'a, Result<CloudState, StoreError>> {
            Box::pin(async { Ok(CloudState::None) })
        }
        fn cloud_sync<'a>(
            &'a self,
            _registry: &'a Registry,
            _row: &'a InstalledGame,
            _mode: SyncMode,
        ) -> BoxFut<'a, Result<CloudState, StoreError>> {
            let state = self
                .sync
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(CloudState::Synced { at: 5 });
            Box::pin(async move { Ok(state) })
        }
    }

    struct NoRunner;
    impl StoreJobRunner for NoRunner {
        fn download(
            &self,
            _c: &AtomicBool,
            _p: &mut dyn FnMut(u64, u64, f64),
        ) -> Result<(), LibraryError> {
            Ok(())
        }
        fn finalize(&self, _s: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
            Ok(String::new())
        }
    }

    fn job(store_id: String, op: StoreOp) -> StoreJob {
        StoreJob {
            source: StoreKind::Gog,
            store_id,
            op,
            title: String::new(),
            drive: None,
            runner: Arc::new(NoRunner),
        }
    }

    struct Harness {
        service: Arc<GogStore>,
        fake: Arc<FakeGog>,
        events: Arc<Mutex<Vec<StoreEvent>>>,
        dir: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let fake = Arc::new(FakeGog::default());
        let shared = StoreShared::new();
        let service = GogStore::new(
            fake.clone(),
            GogEndpoints::e2e(Url::parse("http://127.0.0.1:1/").unwrap()),
            dir.path().to_path_buf(),
            config_path,
            shared.clone(),
        );
        shared.register(service.clone());
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        shared.set_emitter(Arc::new(move |event| sink.lock().unwrap().push(event)));
        Harness {
            service,
            fake,
            events,
            dir,
        }
    }

    impl Harness {
        fn notices(&self) -> Vec<StoreNotice> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| match event {
                    StoreEvent::Notice(notice) => Some(notice.clone()),
                    _ => None,
                })
                .collect()
        }
        fn logins(&self) -> Vec<StoreLoginEvent> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| match event {
                    StoreEvent::Login(login) => Some(login.clone()),
                    _ => None,
                })
                .collect()
        }
        fn config(&self) -> GogConfig {
            Config::load(&self.dir.path().join("config.toml"))
                .unwrap()
                .stores
                .gog
        }
    }

    fn product(id: &str, title: &str) -> GogProduct {
        GogProduct {
            id: id.into(),
            title: title.into(),
            platforms: vec![GogPlatform::Linux, GogPlatform::Windows],
            release_date: "2019-05-01".into(),
            dlc: vec![GogProductDlc {
                id: format!("{id}1"),
                title: format!("{title} DLC"),
            }],
            ..Default::default()
        }
    }

    fn code() -> SecretString {
        SecretString::from("FAKE-STORE-TOKEN-not-real".to_string())
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn a_login_connects_installs_the_tool_and_loads_the_owned_list() {
        let h = harness();
        h.service.load();
        assert_eq!(h.service.status().connection, StoreConnection::NotConnected);
        assert!(!h.service.status().in_rail);
        h.service.begin_login().unwrap();
        assert_eq!(h.service.login_phase(), LoginPhase::Open);
        *h.fake.login.lock().unwrap() = Some(Ok(GogAccount {
            username: "sam_plays".into(),
            user_id: "1".into(),
        }));
        *h.fake.owned.lock().unwrap() = Some(Ok(vec![
            product("10", "Ashen Tide"),
            product("20", "Lanternfall"),
        ]));
        rt().block_on(h.service.finish_login(code()));

        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.username, "sam_plays");
        assert_eq!(status.tool.state, ToolState::Installed);
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.owned_count, 2);
        assert!(status.in_rail);
        assert!(status.refreshed_at > 0);
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
        assert!(h.config().connected);
        assert_eq!(h.config().username, "sam_plays");
        let logins = h.logins();
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].outcome, "connected");
        let messages: Vec<String> = h.notices().into_iter().map(|n| n.message).collect();
        assert_eq!(
            messages,
            vec![
                notice::tool_installing(tool_size_bytes()).message,
                "gogdl installed".to_string(),
                "GOG: 2 games found \u{b7} 2 to install".to_string(),
            ]
        );
        // The cache holds metadata only.
        let cache = std::fs::read_to_string(owned_cache_path(h.dir.path())).unwrap();
        assert!(cache.contains("Ashen Tide"));
        assert!(!cache.contains("FAKE-STORE-TOKEN-not-real"));
    }

    #[test]
    fn a_failed_login_leaves_its_error_and_a_try_again_card() {
        let h = harness();
        h.service.begin_login().unwrap();
        *h.fake.login.lock().unwrap() = Some(Err("the code was refused".into()));
        rt().block_on(h.service.finish_login(code()));
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(
            status.last_error,
            "Couldn\u{2019}t sign in to GOG: gogdl stopped: the code was refused"
        );
        assert_eq!(h.logins()[0].outcome, "failed");
        assert!(
            h.notices().is_empty(),
            "the frontend words the sign-in outcome"
        );
        assert!(!h.config().connected);
        // The next sign-in clears it.
        h.service.begin_login().unwrap();
        *h.fake.login.lock().unwrap() = Some(Ok(GogAccount {
            username: "sam".into(),
            user_id: "1".into(),
        }));
        *h.fake.tool.lock().unwrap() = true;
        rt().block_on(h.service.finish_login(code()));
        assert_eq!(h.service.status().last_error, "");
    }

    #[test]
    fn cancel_is_silent_and_only_while_open() {
        let h = harness();
        h.service.login_cancelled();
        assert!(h.logins().is_empty());
        h.service.begin_login().unwrap();
        h.service.login_cancelled();
        assert_eq!(h.logins()[0].outcome, "cancelled");
        assert!(h.notices().is_empty());
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
    }

    #[test]
    fn a_redirect_without_a_code_fails() {
        let h = harness();
        h.service.begin_login().unwrap();
        h.service.login_redirect_failed();
        assert_eq!(h.logins()[0].outcome, "failed");
        assert_eq!(h.logins()[0].error, "GOG returned an error. Try again.");
    }

    #[test]
    fn a_second_login_waits_while_one_completes() {
        let h = harness();
        h.service.state.lock().unwrap().login = LoginPhase::Completing;
        assert!(h.service.begin_login().is_err());
    }

    #[test]
    fn a_tool_checksum_failure_is_kept_with_try_again() {
        let h = harness();
        *h.fake.tool_result.lock().unwrap() = Some(Err(
            "the download didn't match its checksum, so GRID deleted it.".into(),
        ));
        rt().block_on(h.service.install_tool());
        let status = h.service.status();
        assert_eq!(status.tool.state, ToolState::Failed);
        assert!(status.tool.error.contains("checksum"));
        let notice = h.notices().pop().unwrap();
        assert!(notice.error);
        assert_eq!(notice.action.unwrap().kind, "install_tool");
        assert!(notice
            .message
            .starts_with("Couldn\u{2019}t install gogdl: "));
    }

    #[test]
    fn a_python_problem_fails_the_tool_with_its_words() {
        let h = harness();
        *h.fake.problem.lock().unwrap() =
            Some("gogdl needs Python 3.10 or newer, and python3 was not found.".into());
        rt().block_on(h.service.install_tool());
        let status = h.service.status();
        assert_eq!(status.tool.state, ToolState::Failed);
        assert!(status.tool.error.contains("Python 3.10"));
        *h.fake.tool.lock().unwrap() = true;
        h.service.load();
        assert_eq!(h.service.status().tool.state, ToolState::Failed);
        assert!(h.service.ensure_can_install().is_err());
    }

    #[test]
    fn startup_reads_the_cache_and_checks_the_login() {
        let h = harness();
        crate::config_write::modify_config(&h.dir.path().join("config.toml"), |c| {
            c.stores.gog.connected = true;
            c.stores.gog.username = "sam".into();
            Ok(())
        })
        .unwrap();
        write_owned_cache(h.dir.path(), &[product("10", "Ashen Tide")], 123);
        *h.fake.tool.lock().unwrap() = true;
        h.service.load();
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.refreshed_at, 123);
        assert_eq!(status.owned_count, 1);

        *h.fake.account.lock().unwrap() = Some(Err("expired"));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::Expired);
        assert_eq!(h.notices().pop().unwrap().action.unwrap().kind, "sign_in");
        assert_eq!(h.service.status().username, "sam");
        assert!(h.service.status().in_rail);
    }

    #[test]
    fn a_missing_token_file_signs_out_and_network_trouble_does_not() {
        let h = harness();
        crate::config_write::modify_config(&h.dir.path().join("config.toml"), |c| {
            c.stores.gog.connected = true;
            Ok(())
        })
        .unwrap();
        h.service.load();
        *h.fake.account.lock().unwrap() = Some(Err("auth.gog.com"));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::Connected);
        *h.fake.account.lock().unwrap() = Some(Ok(None));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::NotConnected);
        assert!(!h.config().connected);
    }

    #[test]
    fn an_expired_refresh_flips_the_card() {
        let h = harness();
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        *h.fake.owned.lock().unwrap() = Some(Err("expired"));
        rt().block_on(h.service.refresh_owned(true));
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Expired);
        assert_eq!(status.owned, OwnedState::Error);
        // An install then asks to sign in again.
        assert!(h.service.ensure_can_install().is_err());
        assert_eq!(h.notices().pop().unwrap().action.unwrap().kind, "sign_in");
        assert_eq!(
            h.notices().len(),
            1,
            "one card for the expiry, none for the refusal"
        );
    }

    #[test]
    fn disconnect_forgets_the_login_and_keeps_the_switches() {
        let h = harness();
        h.service.begin_login().unwrap();
        *h.fake.login.lock().unwrap() = Some(Ok(GogAccount {
            username: "sam".into(),
            user_id: "1".into(),
        }));
        *h.fake.owned.lock().unwrap() = Some(Ok(vec![product("10", "Ashen Tide")]));
        rt().block_on(h.service.finish_login(code()));
        std::fs::create_dir_all(crate::store_auth::login_data_dir(h.dir.path(), "gog")).unwrap();
        h.service.disconnect().unwrap();
        assert_eq!(*h.fake.disconnects.lock().unwrap(), 1);
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.username, "");
        assert_eq!(status.owned_count, 0);
        assert_eq!(status.tool.state, ToolState::NotInstalled);
        assert!(status.cloud_saves);
        assert!(!h.config().connected);
        assert!(!owned_cache_path(h.dir.path()).exists());
        assert!(!crate::store_auth::login_data_dir(h.dir.path(), "gog").exists());
    }

    #[test]
    fn installs_need_a_login_and_the_tool() {
        let h = harness();
        assert_eq!(h.service.ensure_can_install().unwrap_err(), "not signed in");
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        assert_eq!(
            h.service.ensure_can_install().unwrap_err(),
            "gogdl isn't installed"
        );
        assert!(h.notices().is_empty());
        h.service.state.lock().unwrap().tool = ToolPhase::Installed;
        h.service.ensure_can_install().unwrap();
    }

    fn row(id: &str, title: &str, meta: &GogMeta) -> InstalledGame {
        InstalledGame {
            title: title.into(),
            platform: "GOG".into(),
            store_id: id.into(),
            source: grid_core::stores::GameSource::Store(StoreKind::Gog),
            store_meta: meta.to_json(),
            filesize_bytes: 1000,
            native_game_dir: format!("/games/{title}"),
            genres: "RPG, Indie".into(),
            first_release_date: "2020-02-02".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_library_splits_installed_and_available() {
        let mut state = GogState {
            connection: StoreConnection::Connected,
            products: vec![product("10", "Ashen Tide"), product("20", "Lanternfall")],
            ..GogState::default()
        };
        let meta = GogMeta {
            version: "2.1".into(),
            cloud_saves: true,
            dlc: vec![grid_core::stores::gog::meta::GogDlcMeta {
                id: "201".into(),
                title: "Lanternfall DLC".into(),
                installer: vec![],
            }],
            ..GogMeta::default()
        };
        let rows = vec![row("20", "Lanternfall", &meta)];
        let mut updates = HashMap::new();
        updates.insert("20".to_string(), "2.2".to_string());
        let config = GogConfig::default();
        let (installed, available) =
            assemble_games(&state, &config, &rows, &updates, &HashSet::new());
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].key, "gog:20");
        assert_eq!(installed[0].update_version, "2.2");
        assert_eq!(installed[0].installed_platform.as_deref(), Some("linux"));
        assert_eq!(installed[0].origin.as_deref(), Some("gogdl"));
        assert_eq!(installed[0].genres, vec!["RPG", "Indie"]);
        assert_eq!(installed[0].year, "2020");
        assert_eq!(installed[0].cloud, "unknown");
        assert!(installed[0]
            .dlc
            .iter()
            .any(|d| d.id == "201" && d.installed));
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].key, "gog:10");
        assert_eq!(available[0].year, "2019");
        assert_eq!(available[0].platforms, vec!["linux", "windows"]);

        state.cloud.insert(
            "20".into(),
            CloudState::Conflict(SaveConflict {
                local_at: 1,
                local_files: 2,
                local_bytes: 3,
                cloud_at: 4,
                cloud_files: Some(5),
                cloud_bytes: Some(6),
            }),
        );
        let (installed, _) = assemble_games(&state, &config, &rows, &updates, &HashSet::new());
        assert_eq!(installed[0].cloud, "conflict");
        let off = GogConfig {
            cloud_saves: false,
            ..GogConfig::default()
        };
        let (installed, _) = assemble_games(&state, &off, &rows, &updates, &HashSet::new());
        assert_eq!(installed[0].cloud, "off");
    }

    #[test]
    fn status_counts_installed_games_and_keeps_an_offline_store_in_the_rail() {
        let state = GogState::default();
        let rows = vec![row("20", "Lanternfall", &GogMeta::default())];
        let status = assemble_status(&state, &GogConfig::default(), &rows, "/lib/GOG");
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.installed_count, 1);
        assert_eq!(status.installed_bytes, 1000);
        assert!(status.in_rail, "an offline install keeps GOG in the rail");
        assert_eq!(status.install_dir, "/lib/GOG");
        assert_eq!(status.tool.name, "gogdl");
        assert_eq!(status.tool.license, "GPL-3.0");
    }

    #[test]
    fn install_options_recommend_linux_and_name_the_folder() {
        let options = GogInstallOptions {
            versions: vec![
                GogVersionOption {
                    platform: GogPlatform::Windows,
                    download_bytes: 5,
                    disk_bytes: 6,
                },
                GogVersionOption {
                    platform: GogPlatform::Linux,
                    download_bytes: 3,
                    disk_bytes: 4,
                },
            ],
            languages: vec!["en-US".into()],
            default_language: "en-US".into(),
            dlc: vec![GogDlcOption {
                id: "1".into(),
                title: "D".into(),
                download_bytes: 9,
            }],
        };
        let ipc = install_options_ipc(&options, "Ashen: Tide", "/nonexistent/GOG", "GE-Proton");
        assert!(!ipc.versions[0].recommended);
        assert!(ipc.versions[1].recommended);
        assert!(ipc.install_dir.starts_with("/nonexistent/GOG/"));
        assert!(
            !ipc.install_dir.ends_with("Ashen: Tide"),
            "{}",
            ipc.install_dir
        );
        assert_eq!(ipc.dlc[0].download_bytes, 9);
        let windows_only = GogInstallOptions {
            versions: vec![GogVersionOption {
                platform: GogPlatform::Windows,
                download_bytes: 1,
                disk_bytes: 1,
            }],
            ..GogInstallOptions::default()
        };
        assert!(install_options_ipc(&windows_only, "x", "", "").versions[0].recommended);
    }

    #[test]
    fn cloud_states_map_to_the_ui() {
        let meta = GogMeta {
            last_sync: 77,
            save_locations: vec![grid_core::stores::gog::meta::SaveLocation {
                name: "saves".into(),
                path: "/s".into(),
            }],
            ..GogMeta::default()
        };
        let synced = cloud_state_ipc(&CloudState::Synced { at: 9 }, &meta, false);
        assert_eq!((synced.state.as_str(), synced.synced_at), ("synced", 9));
        assert_eq!(synced.locations, vec!["/s"]);
        assert_eq!(
            cloud_state_ipc(&CloudState::Synced { at: 9 }, &meta, true).state,
            "syncing"
        );
        let conflict = cloud_state_ipc(
            &CloudState::Conflict(SaveConflict {
                local_at: 1,
                local_files: 2,
                local_bytes: 3,
                cloud_at: 4,
                cloud_files: Some(5),
                cloud_bytes: Some(6),
            }),
            &meta,
            false,
        );
        assert_eq!(conflict.state, "conflict");
        let sides = conflict.conflict.unwrap();
        assert_eq!(sides.cloud_at, 4);
        assert_eq!((sides.cloud_files, sides.cloud_bytes), (Some(5), Some(6)));
        assert_eq!(cloud_state_ipc(&CloudState::Off, &meta, true).state, "off");
    }

    #[test]
    fn a_conflict_before_launch_stops_it_and_other_failures_do_not() {
        let h = harness();
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        let registry = Registry::open(&h.dir.path().join("r.db")).unwrap();
        let meta = GogMeta {
            cloud_saves: true,
            ..GogMeta::default()
        };
        let game = row("20", "Lanternfall", &meta);
        *h.fake.sync.lock().unwrap() = Some(CloudState::Conflict(SaveConflict {
            local_at: 1,
            local_files: 1,
            local_bytes: 1,
            cloud_at: 2,
            cloud_files: Some(1),
            cloud_bytes: None,
        }));
        let result = rt().block_on(h.service.sync_before_launch(&registry, &game));
        assert_eq!(result.unwrap_err().cloud_at, 2);
        *h.fake.sync.lock().unwrap() = Some(CloudState::Synced { at: 3 });
        rt().block_on(h.service.sync_before_launch(&registry, &game))
            .unwrap();
        // Off for the game: no sync at all.
        let off = row("20", "Lanternfall", &GogMeta::default());
        *h.fake.sync.lock().unwrap() = Some(CloudState::Conflict(SaveConflict {
            local_at: 1,
            local_files: 1,
            local_bytes: 1,
            cloud_at: 2,
            cloud_files: Some(1),
            cloud_bytes: None,
        }));
        rt().block_on(h.service.sync_before_launch(&registry, &off))
            .unwrap();
    }

    #[test]
    fn an_upload_after_the_session_says_so() {
        let h = harness();
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        let registry = Registry::open(&h.dir.path().join("r.db")).unwrap();
        let game = row(
            "20",
            "Lanternfall",
            &GogMeta {
                cloud_saves: true,
                ..GogMeta::default()
            },
        );
        rt().block_on(h.service.sync_after_session(&registry, &game));
        assert_eq!(
            h.notices().pop().unwrap().message,
            "Cloud saves uploaded to GOG for Lanternfall"
        );
    }

    fn entry(id: u64, kind: &'static str, key: &str, completed: bool) -> JobRow {
        JobRow {
            id,
            store: true,
            kind,
            key: key.into(),
            title: "Grey Meridian".into(),
            completed,
            failed: false,
            error: String::new(),
            drive: "Couch kit".into(),
            needs_wizard: false,
            base: None,
        }
    }

    #[test]
    fn a_finished_gog_job_gets_one_card() {
        let h = harness();
        h.service
            .observe_rows(vec![entry(1, "store_install", "gog:5", false)]);
        assert!(h.notices().is_empty());
        let done = || {
            let mut failed = entry(3, "store_installer", "gog:7", false);
            failed.failed = true;
            failed.error = "Its archive holds a DLC installer for Pale Summit".into();
            failed.base = Some(("Pale Summit".into(), "gog:8".into()));
            vec![
                entry(1, "store_install", "gog:5", true),
                entry(2, "store_restore", "gog:6", true),
                failed,
                entry(4, "store_update", "gog:9", false),
            ]
        };
        h.service.observe_rows(done());
        h.service.observe_rows(done());
        let notices = h.notices();
        assert_eq!(notices.len(), 3);
        assert_eq!(notices[0].message, "Grey Meridian installed");
        assert_eq!(
            notices[0].action.as_ref().unwrap().key.as_deref(),
            Some("gog:5")
        );
        assert_eq!(notices[1].message, "Grey Meridian installed from Couch kit");
        assert_eq!(notices[2].message, "Couldn\u{2019}t install Grey Meridian");
        assert!(notices[2].error);
        assert_eq!(notices[2].action.as_ref().unwrap().kind, "install_base");
        assert_eq!(
            notices[2].action.as_ref().unwrap().key.as_deref(),
            Some("gog:8")
        );
        let update = notice::finished("store_update", "A", "gog:1", "").unwrap();
        assert_eq!(update.message, "A updated");
        let repair = notice::finished("store_repair", "A", "gog:1", "").unwrap();
        assert_eq!(repair.message, "Verified A\u{2019}s files");
    }

    #[test]
    fn an_installer_that_needs_its_setup_window_gets_no_backend_card() {
        // TV mode words that failure itself ("Couldn't install … silently").
        let h = harness();
        let mut row = entry(1, "store_installer", "gog:5", false);
        row.failed = true;
        row.needs_wizard = true;
        h.service.observe_rows(vec![row]);
        assert!(h.notices().is_empty());
    }

    #[test]
    fn notices_use_the_cards_words() {
        assert_eq!(
            notice::tool_installing(12_400_000).message,
            "Installing gogdl (12 MB) for GOG\u{2026}"
        );
        assert_eq!(
            notice::games_found(1, 0).message,
            "GOG: 1 game found \u{b7} 0 to install"
        );
        assert_eq!(
            notice::update_available("Lanternfall", "2.2").message,
            "Lanternfall: update 2.2 is available on GOG"
        );
        assert_eq!(
            notice::update_available("Lanternfall", "2.2")
                .action
                .unwrap()
                .kind,
            "show_updates"
        );
        let installed = notice::installed("Grey Meridian", "gog:5");
        assert_eq!(installed.action.as_ref().unwrap().kind, "play");
        assert_eq!(installed.action.unwrap().key.as_deref(), Some("gog:5"));
        assert_eq!(notice::login_expired().action.unwrap().kind, "sign_in");
        assert_eq!(
            notice::failed("store_update", "A", "x", None)
                .unwrap()
                .action
                .unwrap()
                .kind,
            "open_downloads"
        );
        assert_eq!(notice::size_label(1_100_000_000), "1.1 GB");
        assert_eq!(notice::size_label(0), "");
        let json = serde_json::to_value(notice::tool_installed()).unwrap();
        assert_eq!(json["action"], serde_json::Value::Null);
        assert_eq!(json["source"], "gog");
    }

    #[test]
    fn platforms_parse_linux_and_windows_only() {
        assert_eq!(parse_platform("linux").unwrap(), GogPlatform::Linux);
        assert_eq!(parse_platform("windows").unwrap(), GogPlatform::Windows);
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
    fn the_login_spec_ends_at_on_login_success() {
        let h = harness();
        let spec = h.service.login_spec();
        assert_eq!(spec.store_name, "GOG");
        assert_eq!(spec.finish.as_str(), "http://127.0.0.1:1/on_login_success");
        assert_eq!(spec.code_param, "code");
        assert_eq!(spec.user_agent, None);
    }
}
