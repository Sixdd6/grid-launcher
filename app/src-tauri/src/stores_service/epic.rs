//! Epic Games Store in the app: the Epic [`StoreBackend`]. grid-core's
//! `stores::epic::EpicService` does the store work through legendary; this
//! backend owns what the app adds around it (`docs/stores-phase2-plan.md`
//! package C), the same way `gog.rs` does for GOG:
//!
//! - the connection state machine (not connected → login window open →
//!   completing → connected; expired when Epic refuses the saved login) and
//!   the `store-login` outcome;
//! - the legendary install right after a connect, with its progress in
//!   `StoreStatus.tool`;
//! - the owned-list cache (`<data_dir>/stores/cache/epic-owned.json`,
//!   product metadata only, never a token) and its refresh;
//! - Epic's `StoreStatus`, and its `StoreGame`s (always the Windows version,
//!   `origin: legendary`, `not_supported` naming the third-party launcher);
//! - the background `store-notice` cards ([`notice`], worded as the
//!   GameStores card; the frontend raises the cards for what it asks for);
//! - the install guards (signed in, legendary ready, not a third-party
//!   title);
//! - the cloud hooks around a session, and the [`StoreLaunchHook`] that asks
//!   legendary for the launch ticket (`EpicService::launch_prep`).
//!
//! The store calls go through [`EpicApi`], a seam the unit tests fake.
//!
//! Secrets: the login code arrives as a [`SecretString`] and goes straight
//! to `complete_login` (legendary's stdin). The launch ticket's exchange
//! code stays inside [`StoreLaunchPrep::secret_args`], which never reaches
//! a log or IPC. Nothing here logs a token, a code, a URL with a query, or
//! legendary's output; every error shown to the user is GRID's own words.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use grid_core::config::{Config, EpicConfig};
use grid_core::launch::{LaunchError, StoreLaunchHook, StoreLaunchPrep};
use grid_core::library::queue::{DownloadStatus, DownloadsSnapshot, StoreJob};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::epic::launch::launch_ticket_error;
use grid_core::stores::epic::list::{self as epic_list, read_owned_cache};
use grid_core::stores::epic::meta::EpicMeta;
use grid_core::stores::epic::{
    EpicAccount, EpicEndpoints, EpicGame, EpicGameDlc, EpicInstallOptions, EpicInstallRequest,
    EpicJobCtx, EpicService, EpicUpdate, EPIC_CODE_PARAM, EPIC_LOGIN_USER_AGENT,
};
use grid_core::stores::tools::{self, StoreTool};
use grid_core::stores::{CloudState, GameKey, GameSource, StoreError, StoreKind, SyncMode};
use secrecy::SecretString;

use super::gog::conflict_ipc;
use super::{default_install_dir, BoxFut, JobCtx, StoreBackend, StoreEvent, StoreShared};
use crate::commands::stores::{
    OwnedState, StoreCloudState, StoreConnection, StoreDlc, StoreGame, StoreGameSettings,
    StoreInstallOptions, StoreInstallRequest, StoreLibrary, StoreLoginEvent, StoreNotice,
    StoreSaveConflict, StoreStatus, StoreVersionOption, ToolState, ToolStatus,
};
use crate::store_auth::LoginSpec;

/// What the app asks of Epic. [`RealEpic`] forwards to grid-core's
/// `EpicService` and the tool installer; the unit tests fake it.
pub trait EpicApi: Send + Sync {
    fn endpoints(&self) -> EpicEndpoints;
    fn complete_login(&self, code: SecretString) -> BoxFut<'_, Result<EpicAccount, StoreError>>;
    fn account(&self) -> BoxFut<'_, Result<Option<EpicAccount>, StoreError>>;
    fn disconnect(&self) -> io::Result<()>;
    fn owned(&self) -> BoxFut<'_, Result<Vec<EpicGame>, StoreError>>;
    /// Seeds the service's kept owned list (from the cache at startup).
    fn set_owned(&self, _games: Vec<EpicGame>) {}
    fn tool_installed(&self) -> bool;
    /// Why legendary can't run here (the Linux zipapp needs Python 3.10),
    /// `None` when it can.
    fn tool_problem(&self) -> Option<String>;
    fn install_tool<'a>(
        &'a self,
        progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
    ) -> BoxFut<'a, Result<(), StoreError>>;
    fn install_options<'a>(
        &'a self,
        app: &'a str,
    ) -> BoxFut<'a, Result<EpicInstallOptions, StoreError>>;
    fn install_job(&self, req: EpicInstallRequest, ctx: EpicJobCtx) -> StoreJob;
    fn update_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob;
    fn repair_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob;
    fn dlc_job(&self, row: &InstalledGame, dlc: &str, ctx: EpicJobCtx) -> StoreJob;
    fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc: &str,
    ) -> Result<(), StoreError>;
    fn uninstall(&self, registry: &Registry, app: &str) -> Result<(), StoreError>;
    fn launch_prep<'a>(
        &'a self,
        row: &'a InstalledGame,
        offline: bool,
    ) -> BoxFut<'a, Result<StoreLaunchPrep, StoreError>>;
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

/// [`EpicApi`] over grid-core.
pub struct RealEpic {
    service: EpicService,
    data_dir: PathBuf,
}

impl RealEpic {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, endpoints: EpicEndpoints) -> Self {
        RealEpic {
            service: EpicService::new(data_dir.clone(), http, endpoints),
            data_dir,
        }
    }
}

impl EpicApi for RealEpic {
    fn endpoints(&self) -> EpicEndpoints {
        self.service.endpoints().clone()
    }
    fn complete_login(&self, code: SecretString) -> BoxFut<'_, Result<EpicAccount, StoreError>> {
        Box::pin(self.service.complete_login(code))
    }
    fn account(&self) -> BoxFut<'_, Result<Option<EpicAccount>, StoreError>> {
        Box::pin(self.service.account())
    }
    fn disconnect(&self) -> io::Result<()> {
        self.service.disconnect()
    }
    fn owned(&self) -> BoxFut<'_, Result<Vec<EpicGame>, StoreError>> {
        Box::pin(self.service.owned())
    }
    fn set_owned(&self, games: Vec<EpicGame>) {
        self.service.set_owned(games);
    }
    fn tool_installed(&self) -> bool {
        self.service.tool_installed()
    }
    fn tool_problem(&self) -> Option<String> {
        if self.tool_installed() {
            return match tools::tool_command(
                &self.data_dir,
                StoreTool::Legendary,
                Vec::new(),
                Vec::new(),
            ) {
                Err(StoreError::Refused(problem)) => Some(problem),
                _ => None,
            };
        }
        let asset = tools::host_asset(StoreTool::Legendary)?;
        if asset.launcher != tools::AssetLauncher::Python3 {
            return None;
        }
        let env = grid_core::launch::spawn::clean_env();
        tools::python_problem(
            StoreTool::Legendary,
            grid_core::launch::umu::probe_python(&env),
        )
    }
    fn install_tool<'a>(
        &'a self,
        mut progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
    ) -> BoxFut<'a, Result<(), StoreError>> {
        Box::pin(async move { self.service.install_tool(&mut *progress).await.map(|_| ()) })
    }
    fn install_options<'a>(
        &'a self,
        app: &'a str,
    ) -> BoxFut<'a, Result<EpicInstallOptions, StoreError>> {
        Box::pin(self.service.install_options(app))
    }
    fn install_job(&self, req: EpicInstallRequest, ctx: EpicJobCtx) -> StoreJob {
        self.service.install_job(req, ctx)
    }
    fn update_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob {
        self.service.update_job(row, ctx)
    }
    fn repair_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob {
        self.service.repair_job(row, ctx)
    }
    fn dlc_job(&self, row: &InstalledGame, dlc: &str, ctx: EpicJobCtx) -> StoreJob {
        self.service.dlc_job(row, dlc, ctx)
    }
    fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc: &str,
    ) -> Result<(), StoreError> {
        self.service.remove_dlc(registry, row, dlc)
    }
    fn uninstall(&self, registry: &Registry, app: &str) -> Result<(), StoreError> {
        self.service.uninstall(registry, app)
    }
    fn launch_prep<'a>(
        &'a self,
        row: &'a InstalledGame,
        offline: bool,
    ) -> BoxFut<'a, Result<StoreLaunchPrep, StoreError>> {
        Box::pin(self.service.launch_prep(row, offline))
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

/// The e2e build's Epic base URL variable (`e2e/mock-romm/mock-epic.mjs`).
#[cfg(feature = "e2e")]
pub const E2E_EPIC_BASE_ENV: &str = "GRID_LAUNCHER_E2E_EPIC_BASE";

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

/// legendary's state, with its download progress.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ToolPhase {
    NotInstalled,
    Installing { done: u64, total: u64 },
    Installed,
    Failed(String),
}

#[derive(Debug, Clone)]
struct EpicState {
    connection: StoreConnection,
    username: String,
    tool: ToolPhase,
    owned: OwnedState,
    owned_error: String,
    games: Vec<EpicGame>,
    refreshed_at: i64,
    /// A failed sign-in's error, left on the store's card.
    last_error: String,
    login: LoginPhase,
    cloud: HashMap<String, CloudState>,
    /// `info --json`'s sizes by app name (download, disk), once asked.
    sizes: HashMap<String, (u64, u64)>,
}

impl Default for EpicState {
    fn default() -> Self {
        EpicState {
            connection: StoreConnection::NotConnected,
            username: String::new(),
            tool: ToolPhase::NotInstalled,
            owned: OwnedState::Idle,
            owned_error: String::new(),
            games: Vec::new(),
            refreshed_at: 0,
            last_error: String::new(),
            login: LoginPhase::Idle,
            cloud: HashMap::new(),
            sizes: HashMap::new(),
        }
    }
}

/// The Epic backend.
pub struct EpicStore {
    api: Arc<dyn EpicApi>,
    endpoints: EpicEndpoints,
    data_dir: PathBuf,
    config_path: PathBuf,
    state: Mutex<EpicState>,
    /// The emitter and registry every store shares.
    shared: Arc<StoreShared>,
    /// Games whose cloud saves are syncing right now.
    syncing: Mutex<HashSet<String>>,
    /// Downloads entries whose finish already had its card.
    announced: Mutex<HashSet<u64>>,
    /// `owned()` answered this run.
    owned_fetched: AtomicBool,
}

/// The tool's name in GRID's words.
const TOOL_NAME: &str = "legendary";
const STORE_NAME: &str = "Epic Games";
const SOURCE: &str = "epic";

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl EpicStore {
    pub fn new(
        api: Arc<dyn EpicApi>,
        data_dir: PathBuf,
        config_path: PathBuf,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let endpoints = api.endpoints();
        Arc::new(EpicStore {
            api,
            endpoints,
            data_dir,
            config_path,
            state: Mutex::new(EpicState::default()),
            shared,
            syncing: Mutex::new(HashSet::new()),
            announced: Mutex::new(HashSet::new()),
            owned_fetched: AtomicBool::new(false),
        })
    }

    /// The real Epic backend for this run.
    pub fn real(
        data_dir: PathBuf,
        config_path: PathBuf,
        http: reqwest::Client,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let api = Arc::new(RealEpic::new(data_dir.clone(), http, epic_endpoints()));
        EpicStore::new(api, data_dir, config_path, shared)
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

    fn epic_config(&self) -> EpicConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.epic)
            .unwrap_or_default()
    }

    fn write_epic_config(&self, change: impl FnOnce(&mut EpicConfig)) {
        if let Err(e) = crate::config_write::modify_config(&self.config_path, |config| {
            change(&mut config.stores.epic);
            Ok(())
        }) {
            tracing::warn!("stores: config write failed: {e}");
        }
    }

    // --- startup --------------------------------------------------------------

    /// Startup: legendary, the cached owned list and the config's
    /// connection. The login check runs next ([`Self::check_login`]).
    pub fn load(&self) {
        let config = self.epic_config();
        let cache = read_owned_cache(&self.data_dir);
        let tool = self.tool_phase_now();
        let mut state = self.state.lock().unwrap();
        state.tool = tool;
        if config.connected {
            state.connection = StoreConnection::Connected;
            state.username = config.username.clone();
            if let Some(cache) = cache {
                // The service's kept list too: the jobs read titles, DLC
                // and the expected build from it before a refresh answers.
                self.api.set_owned(cache.games.clone());
                state.games = cache.games;
                state.refreshed_at = cache.refreshed_at;
                state.owned = OwnedState::Ready;
            }
        }
    }

    /// Asks legendary whether the saved login still works: expired when
    /// Epic refuses it, signed out when legendary has no login. A network
    /// failure keeps the saved state (installed games play offline). Then
    /// refreshes the owned list when signed in.
    pub async fn check_login(self: &Arc<Self>) {
        if !self.epic_config().connected {
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
                    tracing::info!("stores: Epic login not checked: {e}");
                    false
                }
            }
        };
        if let Ok(None) = result {
            self.write_epic_config(|epic| epic.connected = false);
        }
        if expired {
            self.notify(notice::login_expired());
        }
        self.changed();
        if refresh && self.tool_ready() {
            self.refresh_owned(false).await;
            self.refresh_cloud_states().await;
        }
    }

    /// Looks at every installed game's cloud saves (no sync), so the
    /// Library shows SAVE CONFLICT before anyone presses Play. Signed in
    /// only; failures only log.
    pub async fn refresh_cloud_states(self: &Arc<Self>) {
        if self.connection() != StoreConnection::Connected || !self.epic_config().cloud_saves {
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
                Err(e) => tracing::info!("stores: Epic cloud state not read: {e}"),
            }
        }
        if changed {
            self.changed();
        }
    }

    // --- status -----------------------------------------------------------------

    /// Epic's status.
    pub fn status(&self) -> StoreStatus {
        let config = self.epic_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap().clone();
        assemble_status(&state, &config, &rows, &self.install_base_dir_for(&config))
    }

    fn installed_rows(&self) -> Vec<InstalledGame> {
        self.registry()
            .and_then(|registry| registry.store_rows(StoreKind::Epic).ok())
            .unwrap_or_default()
    }

    /// Where Epic games install: the configured folder, else
    /// `<library>/Epic Games`. `""` when neither is set.
    fn install_base_dir_for(&self, epic: &EpicConfig) -> String {
        default_install_dir(&self.config_path, &epic.install_dir, STORE_NAME)
    }

    pub fn install_base_dir(&self) -> String {
        self.install_base_dir_for(&self.epic_config())
    }

    pub fn connection(&self) -> StoreConnection {
        self.state.lock().unwrap().connection
    }

    /// legendary's state on disk: installed, missing, or installed but
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
            LoginPhase::Completing => {
                Err(format!("GRID is finishing the sign-in to {STORE_NAME}."))
            }
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

    /// The finish redirect carried no code (the user denied access).
    pub fn login_redirect_failed(&self) {
        self.state.lock().unwrap().login = LoginPhase::Idle;
        self.fail_login(&format!("{STORE_NAME} returned an error. Try again."));
    }

    fn fail_login(&self, why: &str) {
        self.state.lock().unwrap().last_error =
            format!("Couldn\u{2019}t sign in to {STORE_NAME}: {why}");
        self.emit(StoreEvent::Login(StoreLoginEvent {
            source: SOURCE.to_string(),
            outcome: "failed".to_string(),
            username: String::new(),
            error: why.to_string(),
        }));
        self.changed();
    }

    /// The redirect's code: legendary needs to be installed to redeem it,
    /// so it installs first when it is missing; then signs in, then loads
    /// the owned list.
    pub async fn finish_login(self: &Arc<Self>, code: SecretString) {
        self.state.lock().unwrap().login = LoginPhase::Completing;
        // legendary redeems the code itself (`auth`, the code on stdin), so
        // unlike GOG it must be on disk before the sign-in can finish.
        if !self.api.tool_installed() {
            self.install_tool_then(false, true).await;
        } else {
            let phase = self.tool_phase_now();
            self.state.lock().unwrap().tool = phase;
        }
        let tool = self.state.lock().unwrap().tool.clone();
        if tool != ToolPhase::Installed {
            drop(code);
            self.state.lock().unwrap().login = LoginPhase::Idle;
            let why = match tool {
                ToolPhase::Failed(problem) => problem,
                _ => StoreError::ToolMissing(TOOL_NAME).to_string(),
            };
            self.fail_login(&why);
            return;
        }
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
                self.write_epic_config(move |epic| {
                    epic.connected = true;
                    epic.username = username;
                });
                self.emit(StoreEvent::Login(StoreLoginEvent {
                    source: SOURCE.to_string(),
                    outcome: "connected".to_string(),
                    username: account.username.clone(),
                    error: String::new(),
                }));
                self.changed();
                self.refresh_owned(true).await;
                self.refresh_cloud_states().await;
            }
            Err(error) => {
                self.state.lock().unwrap().login = LoginPhase::Idle;
                self.fail_login(&error.to_string());
            }
        }
    }

    /// Disconnect: legendary's folder (its token files and legendary with
    /// it), the login window's cookies and the owned cache go; installed
    /// games stay.
    pub fn disconnect(&self) -> Result<(), String> {
        if let Err(e) = self.api.disconnect() {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(format!(
                    "Couldn\u{2019}t delete {STORE_NAME}\u{2019}s login: {e}"
                ));
            }
        }
        if let Err(e) = crate::store_auth::wipe_login_data(&self.data_dir, SOURCE) {
            tracing::warn!("stores: login window data not deleted: {e}");
        }
        if let Err(e) = epic_list::remove_owned_cache(&self.data_dir) {
            tracing::warn!("stores: Epic owned cache not deleted: {e}");
        }
        self.write_epic_config(|epic| {
            epic.connected = false;
            epic.username.clear();
        });
        self.owned_fetched.store(false, Ordering::SeqCst);
        {
            let mut state = self.state.lock().unwrap();
            let login = state.login;
            *state = EpicState {
                login,
                tool: self.tool_phase_now(),
                ..EpicState::default()
            };
        }
        self.changed();
        Ok(())
    }

    // --- tool ---------------------------------------------------------------------

    /// Installs legendary (pinned, SHA-256 checked), then loads the owned
    /// list when it never loaded. One at a time.
    pub async fn install_tool(self: &Arc<Self>) {
        self.install_tool_then(true, false).await;
    }

    /// `announce`: the "Installing legendary…" card (during a sign-in; the
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
            Ok(games) => {
                let installed: HashSet<String> = self
                    .installed_rows()
                    .into_iter()
                    .map(|row| row.store_id)
                    .collect();
                let to_install = games
                    .iter()
                    .filter(|game| {
                        !installed.contains(&game.app_name) && game.third_party.is_none()
                    })
                    .count();
                let count = games.len();
                let at = now();
                self.owned_fetched.store(true, Ordering::SeqCst);
                write_owned_cache(&self.data_dir, &games, at);
                {
                    let mut state = self.state.lock().unwrap();
                    state.games = games;
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

    /// Loads the owned list through legendary once this run (the install
    /// dialog and the update check read it). Quiet: no card.
    pub async fn ensure_owned_fetched(self: &Arc<Self>) {
        if self.owned_fetched.load(Ordering::SeqCst) {
            return;
        }
        self.refresh_owned(false).await;
    }

    /// The owned game for `app_name`.
    pub fn game(&self, app_name: &str) -> Option<EpicGame> {
        self.state
            .lock()
            .unwrap()
            .games
            .iter()
            .find(|game| game.app_name == app_name)
            .cloned()
    }

    /// The installed games the owned list has a different build for (the
    /// update check; no network beyond the owned refresh).
    pub async fn find_updates(self: &Arc<Self>, rows: &[InstalledGame]) -> Vec<EpicUpdate> {
        self.ensure_owned_fetched().await;
        let games = self.state.lock().unwrap().games.clone();
        grid_core::stores::epic::updates(rows, &games)
    }

    // --- library ------------------------------------------------------------------

    /// Epic's Installed and Available lists. `updates`: newer versions by
    /// app name ([`crate::update_service`]).
    pub fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary {
        let config = self.epic_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap().clone();
        let status = assemble_status(&state, &config, &rows, &self.install_base_dir_for(&config));
        let syncing = self.syncing.lock().unwrap().clone();
        let (installed, available) = assemble_games(&state, &config, &rows, updates, &syncing);
        StoreLibrary {
            status,
            installed,
            available,
        }
    }

    // --- cloud saves --------------------------------------------------------------

    /// Whether GRID syncs this row's cloud saves: the store switch, the
    /// game's own, and a cloud save folder Epic names.
    pub fn cloud_enabled(&self, row: &InstalledGame) -> bool {
        let meta = EpicMeta::from_json(&row.store_meta);
        self.epic_config().cloud_saves && meta.cloud_saves && has_cloud(&meta)
    }

    /// Records a game's cloud state (the Library's cloud column).
    pub fn remember_cloud(&self, app_name: &str, cloud: &CloudState) {
        self.state
            .lock()
            .unwrap()
            .cloud
            .insert(app_name.to_string(), cloud.clone());
    }

    pub fn set_syncing(&self, app_name: &str, on: bool) {
        let mut syncing = self.syncing.lock().unwrap();
        if on {
            syncing.insert(app_name.to_string());
        } else {
            syncing.remove(app_name);
        }
    }

    pub fn is_syncing(&self, app_name: &str) -> bool {
        self.syncing.lock().unwrap().contains(app_name)
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
                tracing::info!("stores: Epic cloud saves not synced before launch: {e}");
                Ok(())
            }
        }
    }

    /// Uploads after a session ends ("Cloud saves uploaded to Epic Games
    /// for …").
    pub async fn sync_after_session(&self, registry: &Registry, row: &InstalledGame) {
        if !self.cloud_enabled(row) || self.connection() != StoreConnection::Connected {
            return;
        }
        // The row's meta moved during the launch (the pre-launch sync's
        // `last_sync`, `save_path`): read it again.
        let key = GameKey::store(StoreKind::Epic, row.store_id.clone());
        let fresh = registry
            .find_key(&key)
            .ok()
            .flatten()
            .unwrap_or_else(|| row.clone());
        self.set_syncing(&row.store_id, true);
        let result = self.api.cloud_sync(registry, &fresh, SyncMode::Auto).await;
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
                tracing::info!("stores: Epic cloud saves not uploaded after the session: {e}");
                self.notify(notice::cloud_failed(&row.title, &e.to_string()));
            }
        }
        self.changed();
    }

    // --- launch ---------------------------------------------------------------------

    /// legendary's launch ticket for an Epic row. Signed out (or offline)
    /// a game that may run offline starts with `--offline`; a network
    /// failure retries that way once. This is the only offline retry:
    /// `EpicService::launch_prep` runs exactly what it is asked. Errors
    /// take grid-core's words (`launch_ticket_error`).
    pub async fn launch_ticket(&self, row: &InstalledGame) -> Result<StoreLaunchPrep, LaunchError> {
        let meta = EpicMeta::from_json(&row.store_meta);
        let signed_in = self.connection() == StoreConnection::Connected;
        if !signed_in && !meta.can_run_offline {
            return Err(LaunchError::Validation(match self.connection() {
                StoreConnection::Expired => format!(
                    "Your {STORE_NAME} login expired. Sign in again to play {}.",
                    row.title
                ),
                _ => format!("Sign in to {STORE_NAME} to play {}.", row.title),
            }));
        }
        let first = self.api.launch_prep(row, !signed_in).await;
        let result = match first {
            Err(StoreError::Network(_)) if signed_in && meta.can_run_offline => {
                tracing::info!("stores: Epic unreachable; launching offline");
                self.api.launch_prep(row, true).await
            }
            other => other,
        };
        result.map_err(|e| {
            if matches!(e, StoreError::LoginExpired) {
                let _ = self.store_error(StoreError::LoginExpired);
            }
            launch_ticket_error(&e)
        })
    }

    // --- Downloads ------------------------------------------------------------------

    /// Every Downloads snapshot: an Epic job that just finished gets its
    /// card ("Starlight Courier installed" · Play, "… updated", "Verified
    /// …"), a failed one its error card, and the store's counts change.
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
                    detail: entry.detail.clone(),
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
                    || !entry.key.starts_with("epic:")
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
            let card = if entry.completed {
                notice::finished(entry.kind, &entry.title, &entry.key, &entry.detail)
            } else {
                notice::failed(entry.kind, &entry.title, &entry.error)
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
        self.write_epic_config(|epic| epic.cloud_saves = on);
        self.changed();
    }

    pub fn set_install_dir(&self, path: &str) {
        let path = path.trim().to_string();
        self.write_epic_config(move |epic| epic.install_dir = path);
        self.changed();
    }

    /// Refuses an install while signed out or without legendary.
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

    /// The login window's Epic spec: Epic's sign-in page, ended by the
    /// launcher redirect (`https://localhost/launcher/authorized?code=`),
    /// which Epic sends only to the Epic Games Launcher's user agent.
    pub fn login_spec(&self) -> LoginSpec {
        LoginSpec {
            store_name: STORE_NAME.to_string(),
            login_url: self.endpoints.login.clone(),
            finish: self.endpoints.finish.clone(),
            finish_query: None,
            code_param: EPIC_CODE_PARAM,
            user_agent: Some(EPIC_LOGIN_USER_AGENT.to_string()),
        }
    }

    /// Whether `url` is store art the UI may load (Epic's CDNs, the e2e
    /// mock's origin).
    pub fn is_art_url(&self, url: &str) -> bool {
        grid_core::stores::art::is_store_art_url(url, &self.endpoints.art_origins)
    }

    // --- the commands' Epic work --------------------------------------------------

    /// Refuses a game that starts through another company's launcher.
    fn refuse_third_party(&self, app_name: &str) -> Result<(), String> {
        match self.game(app_name).and_then(|game| game.third_party) {
            Some(launcher) => Err(third_party_refusal(&launcher)),
            None => Ok(()),
        }
    }

    /// `store_install_options` for Epic.
    async fn install_options_for(
        self: &Arc<Self>,
        app_name: &str,
    ) -> Result<StoreInstallOptions, String> {
        self.ensure_can_install()?;
        self.ensure_owned_fetched().await;
        self.refuse_third_party(app_name)?;
        let options = self
            .api
            .install_options(app_name)
            .await
            .map_err(|e| self.store_error(e))?;
        {
            let mut state = self.state.lock().unwrap();
            state.sizes.insert(
                app_name.to_string(),
                (options.download_bytes, options.disk_bytes),
            );
            for (dlc, bytes) in &options.dlc {
                state.sizes.insert(dlc.app_name.clone(), (*bytes, 0));
            }
        }
        let title = self
            .game(app_name)
            .map(|game| game.title)
            .unwrap_or_else(|| app_name.to_string());
        let compat = Config::load(&self.config_path)
            .map(|config| config.default_compat_tool)
            .unwrap_or_default();
        let base = self.install_base_dir();
        tokio::task::spawn_blocking(move || install_options_ipc(&options, &title, &base, &compat))
            .await
            .map_err(|e| e.to_string())
    }

    /// `store_install` for Epic: the job for a confirmed install.
    async fn install_job_for(
        self: &Arc<Self>,
        app_name: String,
        request: StoreInstallRequest,
        ctx: JobCtx,
    ) -> Result<StoreJob, String> {
        self.ensure_can_install()?;
        let platform = request.platform.trim();
        if !platform.is_empty() && platform != "windows" {
            return Err("GRID installs the Windows version of Epic Games games.".to_string());
        }
        self.ensure_owned_fetched().await;
        self.refuse_third_party(&app_name)?;
        let title = self
            .game(&app_name)
            .map(|game| game.title)
            .unwrap_or_else(|| app_name.clone());
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
            EpicInstallRequest {
                app_name,
                title,
                dlc: request.dlc_ids,
                install_dir,
                compat_tool: request.compat_tool,
            },
            ctx.epic(),
        ))
    }

    /// `store_cloud_state` for Epic.
    async fn cloud_state_for(&self, row: &InstalledGame) -> Result<StoreCloudState, String> {
        let meta = EpicMeta::from_json(&row.store_meta);
        if !has_cloud(&meta) {
            return Ok(cloud_state_ipc(&CloudState::None, &meta, false));
        }
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

    /// `store_cloud_sync` / `store_cloud_resolve` for Epic: syncs in
    /// `mode`, then answers the state with the row re-read (its
    /// `last_sync` moved).
    async fn cloud_sync_for(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        mode: SyncMode,
    ) -> Result<StoreCloudState, String> {
        let meta = EpicMeta::from_json(&row.store_meta);
        if !has_cloud(&meta) {
            return Ok(cloud_state_ipc(&CloudState::None, &meta, false));
        }
        if !self.cloud_enabled(row) {
            return Ok(cloud_state_ipc(&CloudState::Off, &meta, false));
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
        let key = GameKey::store(StoreKind::Epic, row.store_id.clone());
        let fresh = registry
            .find_key(&key)
            .ok()
            .flatten()
            .unwrap_or_else(|| row.clone());
        Ok(cloud_state_ipc(
            &cloud?,
            &EpicMeta::from_json(&fresh.store_meta),
            false,
        ))
    }
}

/// Whether Epic keeps cloud saves for the game (it names a save folder).
fn has_cloud(meta: &EpicMeta) -> bool {
    !meta.cloud_save_folder.trim().is_empty() || !meta.save_path.trim().is_empty()
}

/// The words for a game that starts through another company's launcher
/// (GameStores Details "Not supported").
pub fn third_party_refusal(launcher: &str) -> String {
    format!(
        "Needs {launcher}. GRID can\u{2019}t install games that start through another company\u{2019}s launcher; install it from the Epic Games Launcher instead."
    )
}

impl StoreBackend for EpicStore {
    fn kind(&self) -> StoreKind {
        StoreKind::Epic
    }
    fn status(&self) -> StoreStatus {
        EpicStore::status(self)
    }
    fn connection(&self) -> StoreConnection {
        EpicStore::connection(self)
    }
    fn changed(&self) {
        EpicStore::changed(self)
    }
    fn load(&self) {
        EpicStore::load(self)
    }
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { EpicStore::check_login(&self).await })
    }
    fn observe_downloads(&self, snapshot: &DownloadsSnapshot) {
        EpicStore::observe_downloads(self, snapshot)
    }
    fn login_spec(&self) -> LoginSpec {
        EpicStore::login_spec(self)
    }
    fn begin_login(&self) -> Result<(), String> {
        EpicStore::begin_login(self)
    }
    fn login_window_failed(&self, error: &str) {
        EpicStore::login_window_failed(self, error)
    }
    fn login_cancelled(&self) {
        EpicStore::login_cancelled(self)
    }
    fn login_redirect_failed(&self) {
        EpicStore::login_redirect_failed(self)
    }
    fn run_finish_login(self: Arc<Self>, code: SecretString) -> BoxFut<'static, ()> {
        Box::pin(async move { EpicStore::finish_login(&self, code).await })
    }
    fn disconnect(&self) -> Result<(), String> {
        EpicStore::disconnect(self)
    }
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { EpicStore::install_tool(&self).await })
    }
    fn run_refresh(self: Arc<Self>, announce: bool) -> BoxFut<'static, ()> {
        Box::pin(async move { EpicStore::refresh_owned(&self, announce).await })
    }
    fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary {
        EpicStore::library(self, updates)
    }
    fn check_updates(self: Arc<Self>, app: tauri::AppHandle) {
        use tauri::Manager as _;
        let updates = app.state::<crate::commands::AppState>().updates.clone();
        updates.spawn_store_check(app, self);
    }
    fn set_install_dir(&self, path: &str) {
        EpicStore::set_install_dir(self, path)
    }
    fn set_cloud_saves(&self, on: bool) {
        EpicStore::set_cloud_saves(self, on)
    }
    fn ensure_can_install(&self) -> Result<(), String> {
        EpicStore::ensure_can_install(self)
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
        self.api.update_job(row, ctx.epic())
    }
    fn repair_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.api.repair_job(row, ctx.epic())
    }
    fn dlc_job(
        self: Arc<Self>,
        row: InstalledGame,
        dlc_id: String,
        ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        Box::pin(async move {
            self.ensure_owned_fetched().await;
            Ok(self.api.dlc_job(&row, &dlc_id, ctx.epic()))
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
        let result = self.api.uninstall(registry, store_id);
        if result.is_ok() {
            self.state.lock().unwrap().cloud.remove(store_id);
        }
        result
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
        row: InstalledGame,
        settings: &StoreGameSettings,
    ) -> InstalledGame {
        apply_game_settings(row, settings)
    }
    fn windows_version(&self, _row: &InstalledGame) -> bool {
        true
    }
    fn cloud_enabled(&self, row: &InstalledGame) -> bool {
        EpicStore::cloud_enabled(self, row)
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
        Box::pin(EpicStore::sync_before_launch(self, registry, row))
    }
    fn sync_after_session<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, ()> {
        Box::pin(EpicStore::sync_after_session(self, registry, row))
    }
    fn is_art_url(&self, url: &str) -> bool {
        EpicStore::is_art_url(self, url)
    }
    fn launch_hook(self: Arc<Self>) -> Option<Arc<dyn StoreLaunchHook>> {
        Some(self)
    }
}

/// The launch hook: an Epic row starts with legendary's launch ticket (its
/// executable, working folder, arguments and the redacted exchange code);
/// every other row launches as it is.
impl StoreLaunchHook for EpicStore {
    fn prepare<'a>(
        &'a self,
        game: &'a InstalledGame,
    ) -> BoxFut<'a, Result<Option<StoreLaunchPrep>, LaunchError>> {
        Box::pin(async move {
            if game.source != GameSource::Store(StoreKind::Epic) {
                return Ok(None);
            }
            self.launch_ticket(game).await.map(Some)
        })
    }
}

/// The row after Game Settings' Save. Every Epic game is a Windows version,
/// so it always takes a compat tool. The executable is Epic's (legendary's
/// launch ticket names it); a different one becomes the user's pin.
pub(crate) fn apply_game_settings(
    mut row: InstalledGame,
    settings: &StoreGameSettings,
) -> InstalledGame {
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

/// The parts of a Downloads entry [`EpicStore::observe_downloads`] reads.
struct JobRow {
    id: u64,
    store: bool,
    kind: &'static str,
    key: String,
    title: String,
    completed: bool,
    failed: bool,
    error: String,
    /// The finished row's line ("Verified · 2 files repaired").
    detail: String,
}

/// legendary's download size for the cards ("Installing legendary (3 MB)…").
fn tool_size_bytes() -> u64 {
    tools::host_asset(StoreTool::Legendary)
        .map(|asset| asset.size)
        .unwrap_or(0)
}

// --- status and library assembly (pure) --------------------------------------

fn assemble_status(
    state: &EpicState,
    config: &EpicConfig,
    rows: &[InstalledGame],
    install_dir: &str,
) -> StoreStatus {
    let pin = tools::pin(StoreTool::Legendary);
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
        .games
        .iter()
        .filter(|game| !game.app_name.is_empty())
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
    }
}

fn split_list(text: &str) -> Vec<String> {
    text.split([',', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn year_of(date: &str) -> String {
    date.get(..4)
        .filter(|year| year.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or("")
        .to_string()
}

/// The cloud column: `none` when Epic keeps no cloud saves for the game,
/// `off` when either switch is off, else what GRID last saw (`unknown`
/// before the first look).
fn cloud_column(store_on: bool, meta: &EpicMeta, seen: Option<&CloudState>) -> &'static str {
    if !has_cloud(meta) {
        return "none";
    }
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

fn dlc_ipc(
    dlc: &[EpicGameDlc],
    meta: Option<&EpicMeta>,
    sizes: &HashMap<String, (u64, u64)>,
) -> Vec<StoreDlc> {
    dlc.iter()
        .map(|item| StoreDlc {
            id: item.app_name.clone(),
            title: item.title.clone(),
            download_bytes: sizes.get(&item.app_name).map_or(0, |size| size.0),
            installed: meta
                .is_some_and(|meta| meta.dlc.iter().any(|d| d.app_name == item.app_name)),
        })
        .collect()
}

const WINDOWS: &str = "windows";
const ORIGIN: &str = "legendary";

fn assemble_games(
    state: &EpicState,
    config: &EpicConfig,
    rows: &[InstalledGame],
    updates: &HashMap<String, String>,
    syncing: &HashSet<String>,
) -> (Vec<StoreGame>, Vec<StoreGame>) {
    let games: HashMap<&str, &EpicGame> = state
        .games
        .iter()
        .map(|game| (game.app_name.as_str(), game))
        .collect();
    let installed: Vec<StoreGame> = rows
        .iter()
        .map(|row| {
            let mut meta = EpicMeta::from_json(&row.store_meta);
            let game = games.get(row.store_id.as_str()).copied();
            if meta.cloud_save_folder.trim().is_empty() {
                if let Some(game) = game {
                    meta.cloud_save_folder = game.cloud_save_folder.clone();
                }
            }
            let mut cloud = cloud_column(config.cloud_saves, &meta, state.cloud.get(&row.store_id));
            if syncing.contains(&row.store_id) && cloud != "off" && cloud != "none" {
                cloud = "synced";
            }
            let mut dlc = game
                .map(|game| dlc_ipc(&game.dlc, Some(&meta), &state.sizes))
                .unwrap_or_default();
            for owned in &meta.dlc {
                if !dlc.iter().any(|d| d.id == owned.app_name) {
                    dlc.push(StoreDlc {
                        id: owned.app_name.clone(),
                        title: owned.title.clone(),
                        download_bytes: 0,
                        installed: true,
                    });
                }
            }
            StoreGame {
                key: GameKey::store(StoreKind::Epic, row.store_id.clone()).to_string(),
                source: SOURCE.to_string(),
                store_id: row.store_id.clone(),
                title: row.title.clone(),
                installed: true,
                cover: game
                    .map(|game| game.cover.clone())
                    .filter(|cover| !cover.is_empty())
                    .unwrap_or_else(|| row.cover_large_path.clone()),
                screenshots: game
                    .map(|game| game.screenshots.clone())
                    .filter(|shots| !shots.is_empty())
                    .unwrap_or_else(|| split_list(&row.screenshot_urls)),
                description: game
                    .map(|game| game.description.clone())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| row.description.clone()),
                developer: game
                    .map(|game| game.developer.clone())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| {
                        split_list(&row.companies)
                            .into_iter()
                            .next()
                            .unwrap_or_default()
                    }),
                genres: split_list(&row.genres),
                year: game
                    .map(|game| game.year.clone())
                    .filter(|year| !year.is_empty())
                    .unwrap_or_else(|| year_of(&row.first_release_date)),
                platforms: vec![WINDOWS.to_string()],
                installed_platform: Some(WINDOWS.to_string()),
                download_bytes: 0,
                disk_bytes: row.filesize_bytes.max(0) as u64,
                version: meta.version.clone(),
                update_version: updates.get(&row.store_id).cloned().unwrap_or_default(),
                origin: Some(ORIGIN.to_string()),
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
        .games
        .iter()
        .filter(|game| !installed_ids.contains(game.app_name.as_str()))
        .map(|game| {
            let (download_bytes, disk_bytes) =
                state.sizes.get(&game.app_name).copied().unwrap_or((0, 0));
            StoreGame {
                key: GameKey::store(StoreKind::Epic, game.app_name.clone()).to_string(),
                source: SOURCE.to_string(),
                store_id: game.app_name.clone(),
                title: game.title.clone(),
                installed: false,
                cover: game.cover.clone(),
                screenshots: game.screenshots.clone(),
                description: game.description.clone(),
                developer: game.developer.clone(),
                genres: Vec::new(),
                year: game.year.clone(),
                platforms: vec![WINDOWS.to_string()],
                installed_platform: None,
                download_bytes,
                disk_bytes,
                version: String::new(),
                update_version: String::new(),
                origin: None,
                not_supported: game.third_party.clone().unwrap_or_default(),
                dlc: dlc_ipc(&game.dlc, None, &state.sizes),
                cloud: if game.cloud_save_folder.trim().is_empty() {
                    "none"
                } else {
                    "unknown"
                }
                .to_string(),
                last_sync: 0,
                install_dir: String::new(),
                compat_tool: String::new(),
            }
        })
        .collect();
    (installed, available)
}

/// `StoreInstallOptions` for the install dialog: the Windows version only
/// (no Version or Language choice), the DLC you own with their sizes, and
/// the folder `<base>/<title>`.
pub fn install_options_ipc(
    options: &EpicInstallOptions,
    title: &str,
    base_dir: &str,
    compat_tool: &str,
) -> StoreInstallOptions {
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
        versions: vec![StoreVersionOption {
            platform: WINDOWS.to_string(),
            recommended: true,
            download_bytes: options.download_bytes,
            disk_bytes: options.disk_bytes,
        }],
        languages: Vec::new(),
        default_language: String::new(),
        dlc: options
            .dlc
            .iter()
            .map(|(dlc, bytes)| StoreDlc {
                id: dlc.app_name.clone(),
                title: dlc.title.clone(),
                download_bytes: *bytes,
                installed: false,
            })
            .collect(),
        install_dir,
        free_bytes,
        volume,
        compat_tool: compat_tool.to_string(),
    }
}

/// An Epic cloud state as the UI sees it. Its one location is the save
/// folder GRID resolved.
pub fn cloud_state_ipc(state: &CloudState, meta: &EpicMeta, syncing: bool) -> StoreCloudState {
    let locations = Some(meta.save_path.trim())
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .into_iter()
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
        state: if syncing && name != "off" && name != "none" {
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

// --- owned cache ------------------------------------------------------------------

/// Writes the owned-list cache (grid-core's format,
/// `stores::epic::list`). This backend is the cache's only writer;
/// `EpicService::owned` never writes it.
fn write_owned_cache(data_dir: &Path, games: &[EpicGame], refreshed_at: i64) {
    if let Err(e) = epic_list::write_owned_cache(data_dir, games, refreshed_at) {
        tracing::warn!("stores: Epic owned list not cached: {e}");
    }
}

// --- notices --------------------------------------------------------------------

/// Epic's `store-notice` cards, in the GameStores card's words. Only what
/// happens in the background: the frontend raises the cards for the
/// commands it calls itself (queued, refusals, sign-in outcome from
/// `store-login`, cloud choices, uninstalled, disconnected).
pub mod notice {
    use super::{StoreNotice, STORE_NAME, TOOL_NAME};
    use crate::commands::stores::StoreNoticeAction;

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

    /// legendary starts installing during a sign-in.
    pub fn tool_installing(size: u64) -> StoreNotice {
        let size = crate::stores_service::gog::notice::size_label(size);
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

    /// The card for an Epic Downloads row that just finished, by its kind.
    /// `detail` is the finished row's line (a repair's "Verified · 2 files
    /// repaired").
    pub fn finished(kind: &str, title: &str, key: &str, detail: &str) -> Option<StoreNotice> {
        match kind {
            "store_install" | "store_dlc" => Some(installed(title, key)),
            "store_update" => Some(card(
                format!("{title} updated"),
                false,
                action("play", Some(key), Some(title)),
            )),
            "store_repair" => Some(card(repaired_words(title, detail), false, None)),
            _ => None,
        }
    }

    /// "Verified Starlight Courier's files", or with the repaired count
    /// from the row's line: "Starlight Courier: 2 damaged files were
    /// downloaded again".
    fn repaired_words(title: &str, detail: &str) -> String {
        let repaired = detail
            .split('\u{b7}')
            .nth(1)
            .and_then(|part| part.split_whitespace().next())
            .and_then(|count| count.parse::<u32>().ok())
            .filter(|count| *count > 0);
        match repaired {
            Some(1) => format!("{title}: 1 damaged file was downloaded again"),
            Some(n) => format!("{title}: {n} damaged files were downloaded again"),
            None => format!("Verified {title}\u{2019}s files"),
        }
    }

    /// The card for an Epic Downloads row that just failed.
    pub fn failed(kind: &str, title: &str, error: &str) -> Option<StoreNotice> {
        let what = match kind {
            "store_install" | "store_dlc" => "install",
            "store_update" => "update",
            "store_repair" => "verify",
            _ => return None,
        };
        let mut notice = card(
            format!("Couldn\u{2019}t {what} {title}"),
            true,
            action("open_downloads", None, None),
        );
        notice.detail = error.to_string();
        Some(notice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::library::queue::{StoreJobRunner, StoreOp, StoreProgressSink};
    use grid_core::library::LibraryError;
    use grid_core::stores::SaveConflict;
    use tauri::Url;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

    /// A scripted Epic.
    #[derive(Default)]
    struct FakeEpic {
        login: Mutex<Option<Result<EpicAccount, String>>>,
        account: Mutex<Option<Result<Option<EpicAccount>, &'static str>>>,
        owned: Mutex<Option<Result<Vec<EpicGame>, &'static str>>>,
        tool: Mutex<bool>,
        tool_result: Mutex<Option<Result<(), String>>>,
        sync: Mutex<Option<CloudState>>,
        disconnects: Mutex<u32>,
        problem: Mutex<Option<String>>,
        /// `launch_prep` answers, in order; each call records its offline
        /// flag.
        preps: Mutex<Vec<Result<StoreLaunchPrep, &'static str>>>,
        prep_calls: Mutex<Vec<bool>>,
        options: Mutex<Option<EpicInstallOptions>>,
        installs: Mutex<Vec<EpicInstallRequest>>,
    }

    fn err_of(text: &str) -> StoreError {
        match text {
            "expired" => StoreError::LoginExpired,
            "network" => StoreError::Network("Epic".into()),
            other => StoreError::ToolFailed {
                tool: "legendary",
                reason: other.to_string(),
            },
        }
    }

    impl EpicApi for FakeEpic {
        fn endpoints(&self) -> EpicEndpoints {
            EpicEndpoints::e2e(Url::parse("http://127.0.0.1:4300/").unwrap())
        }
        fn complete_login(
            &self,
            _code: SecretString,
        ) -> BoxFut<'_, Result<EpicAccount, StoreError>> {
            let result = self
                .login
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Err("no script".into()));
            Box::pin(async move { result.map_err(|e| err_of(&e)) })
        }
        fn account(&self) -> BoxFut<'_, Result<Option<EpicAccount>, StoreError>> {
            let result = self.account.lock().unwrap().take().unwrap_or(Ok(None));
            Box::pin(async move { result.map_err(err_of) })
        }
        fn disconnect(&self) -> io::Result<()> {
            *self.disconnects.lock().unwrap() += 1;
            *self.tool.lock().unwrap() = false;
            Ok(())
        }
        fn owned(&self) -> BoxFut<'_, Result<Vec<EpicGame>, StoreError>> {
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
            _app: &'a str,
        ) -> BoxFut<'a, Result<EpicInstallOptions, StoreError>> {
            let options = self.options.lock().unwrap().clone().unwrap_or_default();
            Box::pin(async move { Ok(options) })
        }
        fn install_job(&self, req: EpicInstallRequest, _ctx: EpicJobCtx) -> StoreJob {
            let id = req.app_name.clone();
            self.installs.lock().unwrap().push(req);
            job(id, StoreOp::Install)
        }
        fn update_job(&self, row: &InstalledGame, _ctx: EpicJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Update)
        }
        fn repair_job(&self, row: &InstalledGame, _ctx: EpicJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Repair)
        }
        fn dlc_job(&self, row: &InstalledGame, _dlc: &str, _ctx: EpicJobCtx) -> StoreJob {
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
        fn launch_prep<'a>(
            &'a self,
            _row: &'a InstalledGame,
            offline: bool,
        ) -> BoxFut<'a, Result<StoreLaunchPrep, StoreError>> {
            self.prep_calls.lock().unwrap().push(offline);
            let mut preps = self.preps.lock().unwrap();
            let next = if preps.is_empty() {
                Err("no script")
            } else {
                preps.remove(0)
            };
            Box::pin(async move { next.map_err(err_of) })
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
            source: StoreKind::Epic,
            store_id,
            op,
            title: String::new(),
            drive: None,
            runner: Arc::new(NoRunner),
        }
    }

    struct Harness {
        service: Arc<EpicStore>,
        fake: Arc<FakeEpic>,
        events: Arc<Mutex<Vec<StoreEvent>>>,
        dir: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let fake = Arc::new(FakeEpic::default());
        let shared = StoreShared::new();
        let service = EpicStore::new(
            fake.clone(),
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
        fn config(&self) -> EpicConfig {
            Config::load(&self.dir.path().join("config.toml"))
                .unwrap()
                .stores
                .epic
        }
        fn connect(&self) {
            let mut state = self.service.state.lock().unwrap();
            state.connection = StoreConnection::Connected;
            state.tool = ToolPhase::Installed;
        }
    }

    fn game(app: &str, title: &str) -> EpicGame {
        EpicGame {
            app_name: app.into(),
            title: title.into(),
            cover: format!("http://127.0.0.1:4300/art/{app}.jpg"),
            year: "2024".into(),
            build_version: "1.0.2".into(),
            cloud_save_folder: "{AppData}/Starlight/Saved".into(),
            can_run_offline: true,
            dlc: vec![EpicGameDlc {
                app_name: format!("{app}Dlc"),
                title: format!("{title} DLC"),
            }],
            ..Default::default()
        }
    }

    fn code() -> SecretString {
        SecretString::from(FAKE.to_string())
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn account(name: &str) -> EpicAccount {
        EpicAccount {
            username: name.into(),
        }
    }

    #[test]
    fn a_login_installs_legendary_first_then_signs_in_and_loads_the_owned_list() {
        let h = harness();
        h.service.load();
        assert_eq!(h.service.status().connection, StoreConnection::NotConnected);
        assert!(!h.service.status().in_rail);
        h.service.begin_login().unwrap();
        assert_eq!(h.service.login_phase(), LoginPhase::Open);
        *h.fake.login.lock().unwrap() = Some(Ok(account("SamPlays")));
        let mut iron = game("IronOrchard", "Iron Orchard");
        iron.third_party = Some("the EA app".into());
        *h.fake.owned.lock().unwrap() = Some(Ok(vec![
            game("Starlight", "Starlight Courier"),
            game("Glimmer", "Glimmer Deep"),
            iron,
        ]));
        rt().block_on(h.service.finish_login(code()));

        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.username, "SamPlays");
        assert_eq!(status.tool.state, ToolState::Installed);
        assert_eq!(status.tool.name, "legendary");
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.owned_count, 3);
        assert!(status.in_rail);
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
        assert!(h.config().connected);
        assert_eq!(h.config().username, "SamPlays");
        assert_eq!(h.logins()[0].outcome, "connected");
        let messages: Vec<String> = h.notices().into_iter().map(|n| n.message).collect();
        assert_eq!(
            messages,
            vec![
                notice::tool_installing(tool_size_bytes()).message,
                "legendary installed".to_string(),
                "Epic Games: 3 games found \u{b7} 2 to install".to_string(),
            ]
        );
        let cache = std::fs::read_to_string(epic_list::owned_cache_path(h.dir.path())).unwrap();
        assert!(cache.contains("Starlight Courier"));
        assert!(!cache.contains(FAKE));
    }

    #[test]
    fn a_failed_login_leaves_its_error_and_no_card() {
        let h = harness();
        *h.fake.tool.lock().unwrap() = true;
        h.service.begin_login().unwrap();
        *h.fake.login.lock().unwrap() = Some(Err("the sign-in code was refused".into()));
        rt().block_on(h.service.finish_login(code()));
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(
            status.last_error,
            "Couldn\u{2019}t sign in to Epic Games: legendary stopped: the sign-in code was refused"
        );
        assert_eq!(h.logins()[0].outcome, "failed");
        assert!(h.notices().is_empty());
        assert!(!h.config().connected);
    }

    #[test]
    fn a_login_without_legendary_fails_with_its_words() {
        let h = harness();
        h.service.begin_login().unwrap();
        *h.fake.tool_result.lock().unwrap() = Some(Err(
            "The legendary download didn't match its checksum, so GRID deleted it.".into(),
        ));
        rt().block_on(h.service.finish_login(code()));
        let login = &h.logins()[0];
        assert_eq!(login.outcome, "failed");
        assert!(login.error.contains("checksum"), "{}", login.error);
        assert_eq!(h.service.status().tool.state, ToolState::Failed);
        let card = h.notices().pop().unwrap();
        assert_eq!(card.action.unwrap().kind, "install_tool");
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
    }

    #[test]
    fn cancel_is_silent_and_denied_access_fails() {
        let h = harness();
        h.service.login_cancelled();
        assert!(h.logins().is_empty());
        h.service.begin_login().unwrap();
        h.service.login_cancelled();
        assert_eq!(h.logins()[0].outcome, "cancelled");
        h.service.begin_login().unwrap();
        h.service.login_redirect_failed();
        assert_eq!(h.logins()[1].outcome, "failed");
        assert_eq!(
            h.logins()[1].error,
            "Epic Games returned an error. Try again."
        );
        assert!(h.notices().is_empty());
        h.service.state.lock().unwrap().login = LoginPhase::Completing;
        assert!(h.service.begin_login().is_err());
    }

    #[test]
    fn startup_reads_the_cache_and_an_expired_login_flips_the_card() {
        let h = harness();
        crate::config_write::modify_config(&h.dir.path().join("config.toml"), |c| {
            c.stores.epic.connected = true;
            c.stores.epic.username = "SamPlays".into();
            Ok(())
        })
        .unwrap();
        write_owned_cache(h.dir.path(), &[game("Starlight", "Starlight Courier")], 123);
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
        assert_eq!(h.service.status().username, "SamPlays");
        assert!(h.service.ensure_can_install().is_err());

        // Network trouble keeps the state; no login signs out.
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        *h.fake.account.lock().unwrap() = Some(Err("network"));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::Connected);
        *h.fake.account.lock().unwrap() = Some(Ok(None));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::NotConnected);
        assert!(!h.config().connected);
    }

    #[test]
    fn disconnect_forgets_the_login_and_legendary() {
        let h = harness();
        *h.fake.tool.lock().unwrap() = true;
        h.service.begin_login().unwrap();
        *h.fake.login.lock().unwrap() = Some(Ok(account("SamPlays")));
        *h.fake.owned.lock().unwrap() = Some(Ok(vec![game("Starlight", "Starlight Courier")]));
        rt().block_on(h.service.finish_login(code()));
        std::fs::create_dir_all(crate::store_auth::login_data_dir(h.dir.path(), "epic")).unwrap();
        h.service.disconnect().unwrap();
        assert_eq!(*h.fake.disconnects.lock().unwrap(), 1);
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.username, "");
        assert_eq!(status.owned_count, 0);
        assert_eq!(status.tool.state, ToolState::NotInstalled);
        assert!(status.cloud_saves);
        assert!(!h.config().connected);
        assert!(!epic_list::owned_cache_path(h.dir.path()).exists());
        assert!(!crate::store_auth::login_data_dir(h.dir.path(), "epic").exists());
    }

    #[test]
    fn installs_need_a_login_legendary_and_a_supported_game() {
        let h = harness();
        assert_eq!(h.service.ensure_can_install().unwrap_err(), "not signed in");
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        assert_eq!(
            h.service.ensure_can_install().unwrap_err(),
            "legendary isn't installed"
        );
        h.service.state.lock().unwrap().tool = ToolPhase::Installed;
        h.service.ensure_can_install().unwrap();

        let mut iron = game("IronOrchard", "Iron Orchard");
        iron.third_party = Some("the EA app".into());
        h.service.state.lock().unwrap().games = vec![iron, game("Glimmer", "Glimmer Deep")];
        h.service.owned_fetched.store(true, Ordering::SeqCst);
        let dir = h.dir.path().to_path_buf();
        let ctx = JobCtx {
            registry: Arc::new(Registry::open(&dir.join("r.db")).unwrap()),
            library: dir.clone(),
            default_compat_tool: String::new(),
        };
        let request = |key: &str| StoreInstallRequest {
            key: key.into(),
            platform: "windows".into(),
            language: String::new(),
            dlc_ids: vec!["GlimmerDlc".into()],
            install_dir: dir.join("Epic Games/Glimmer Deep").to_string_lossy().into(),
            compat_tool: "GE-Proton9-20".into(),
        };
        let refused = rt()
            .block_on(h.service.clone().install_job_for(
                "IronOrchard".into(),
                request("epic:IronOrchard"),
                ctx.clone(),
            ))
            .map(|_| ())
            .unwrap_err();
        assert!(refused.starts_with("Needs the EA app."), "{refused}");
        let job = rt()
            .block_on(h.service.clone().install_job_for(
                "Glimmer".into(),
                request("epic:Glimmer"),
                ctx.clone(),
            ))
            .unwrap();
        assert_eq!(job.source, StoreKind::Epic);
        let req = h.fake.installs.lock().unwrap().pop().unwrap();
        assert_eq!(req.title, "Glimmer Deep");
        assert_eq!(req.dlc, vec!["GlimmerDlc"]);
        assert_eq!(req.compat_tool, "GE-Proton9-20");
        let mut linux = request("epic:Glimmer");
        linux.platform = "linux".into();
        assert!(rt()
            .block_on(
                h.service
                    .clone()
                    .install_job_for("Glimmer".into(), linux, ctx)
            )
            .is_err());
    }

    fn row(app: &str, title: &str, meta: &EpicMeta) -> InstalledGame {
        InstalledGame {
            title: title.into(),
            platform: "Epic Games".into(),
            store_id: app.into(),
            source: GameSource::Store(StoreKind::Epic),
            store_meta: meta.to_json(),
            filesize_bytes: 1000,
            native_game_dir: format!("/games/{title}"),
            native_compat_tool: "GE-Proton9-20".into(),
            first_release_date: "2023".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_library_maps_epic_games() {
        let mut iron = game("IronOrchard", "Iron Orchard");
        iron.third_party = Some("the EA app".into());
        let mut vector = game("Vector", "Vector Bloom");
        vector.cloud_save_folder = String::new();
        let mut state = EpicState {
            connection: StoreConnection::Connected,
            games: vec![game("Starlight", "Starlight Courier"), vector, iron],
            ..EpicState::default()
        };
        state.sizes.insert("IronOrchard".into(), (11, 22));
        let meta = EpicMeta {
            version: "1.0.1".into(),
            cloud_saves: true,
            cloud_save_folder: "{AppData}/Starlight/Saved".into(),
            dlc: vec![grid_core::stores::epic::meta::EpicDlcMeta {
                app_name: "StarlightDlc".into(),
                title: "Starlight Courier DLC".into(),
            }],
            ..EpicMeta::default()
        };
        let vector_meta = EpicMeta {
            cloud_saves: true,
            ..EpicMeta::default()
        };
        let rows = vec![
            row("Starlight", "Starlight Courier", &meta),
            row("Vector", "Vector Bloom", &vector_meta),
        ];
        let mut updates = HashMap::new();
        updates.insert("Starlight".to_string(), "1.0.2".to_string());
        let config = EpicConfig {
            cloud_saves: true,
            ..EpicConfig::default()
        };
        let (installed, available) =
            assemble_games(&state, &config, &rows, &updates, &HashSet::new());
        assert_eq!(installed.len(), 2);
        let star = &installed[0];
        assert_eq!(star.key, "epic:Starlight");
        assert_eq!(star.platforms, vec!["windows"]);
        assert_eq!(star.installed_platform.as_deref(), Some("windows"));
        assert_eq!(star.origin.as_deref(), Some("legendary"));
        assert_eq!(star.version, "1.0.1");
        assert_eq!(star.update_version, "1.0.2");
        assert_eq!(star.cloud, "unknown");
        assert_eq!(star.year, "2024");
        assert!(star.cover.ends_with("/art/Starlight.jpg"));
        assert!(star
            .dlc
            .iter()
            .any(|d| d.id == "StarlightDlc" && d.installed));
        assert_eq!(installed[1].cloud, "none", "Vector Bloom has no cloud save");
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].not_supported, "the EA app");
        assert_eq!(available[0].download_bytes, 11);
        assert_eq!(available[0].disk_bytes, 22);
        assert_eq!(available[0].origin, None);

        state.cloud.insert(
            "Starlight".into(),
            CloudState::Conflict(SaveConflict {
                local_at: 1,
                local_files: 2,
                local_bytes: 3,
                cloud_at: 4,
                cloud_files: None,
                cloud_bytes: None,
            }),
        );
        let (installed, _) = assemble_games(&state, &config, &rows, &updates, &HashSet::new());
        assert_eq!(installed[0].cloud, "conflict");
        let (installed, _) = assemble_games(
            &state,
            &EpicConfig {
                cloud_saves: false,
                ..EpicConfig::default()
            },
            &rows,
            &updates,
            &HashSet::new(),
        );
        assert_eq!(installed[0].cloud, "off");
    }

    #[test]
    fn status_reads_the_pin_and_keeps_installed_games_in_the_rail() {
        let rows = vec![row("Starlight", "Starlight Courier", &EpicMeta::default())];
        let status = assemble_status(
            &EpicState::default(),
            &EpicConfig::default(),
            &rows,
            "/lib/Epic Games",
        );
        assert_eq!(status.source, "epic");
        assert_eq!(status.installed_count, 1);
        assert!(status.in_rail);
        assert_eq!(status.install_dir, "/lib/Epic Games");
        assert_eq!(status.tool.name, "legendary");
        assert_eq!(status.tool.version, "0.21.1");
        assert_eq!(status.tool.license, "GPL-3.0");
        assert_eq!(
            status.tool.source_url,
            "https://github.com/legendary-gl/legendary"
        );
        let none = assemble_status(&EpicState::default(), &EpicConfig::default(), &[], "");
        assert!(!none.in_rail, "a not-connected Epic with nothing installed");
    }

    #[test]
    fn install_options_are_windows_only_with_dlc_sizes() {
        let options = EpicInstallOptions {
            download_bytes: 5,
            disk_bytes: 6,
            dlc: vec![(
                EpicGameDlc {
                    app_name: "D".into(),
                    title: "Dlc".into(),
                },
                9,
            )],
        };
        let ipc = install_options_ipc(&options, "Glimmer: Deep", "/nonexistent/Epic Games", "GE");
        assert_eq!(ipc.versions.len(), 1);
        assert_eq!(ipc.versions[0].platform, "windows");
        assert!(ipc.versions[0].recommended);
        assert!(ipc.languages.is_empty());
        assert_eq!(ipc.dlc[0].download_bytes, 9);
        assert!(ipc.install_dir.starts_with("/nonexistent/Epic Games/"));
        assert!(!ipc.install_dir.ends_with("Glimmer: Deep"));
    }

    #[test]
    fn cloud_states_map_to_the_ui() {
        let meta = EpicMeta {
            last_sync: 77,
            save_path: "/s".into(),
            ..EpicMeta::default()
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
                cloud_files: None,
                cloud_bytes: None,
            }),
            &meta,
            false,
        );
        assert_eq!(conflict.state, "conflict");
        assert_eq!(conflict.conflict.unwrap().cloud_files, None);
        assert!(
            cloud_state_ipc(&CloudState::None, &EpicMeta::default(), false)
                .locations
                .is_empty()
        );
    }

    #[test]
    fn a_conflict_before_launch_stops_it_and_an_upload_after_says_so() {
        let h = harness();
        h.connect();
        let registry = Registry::open(&h.dir.path().join("r.db")).unwrap();
        let meta = EpicMeta {
            cloud_saves: true,
            cloud_save_folder: "{AppData}/S".into(),
            ..EpicMeta::default()
        };
        let starlight = row("Starlight", "Starlight Courier", &meta);
        *h.fake.sync.lock().unwrap() = Some(CloudState::Conflict(SaveConflict {
            local_at: 1,
            local_files: 1,
            local_bytes: 1,
            cloud_at: 2,
            cloud_files: None,
            cloud_bytes: None,
        }));
        let result = rt().block_on(h.service.sync_before_launch(&registry, &starlight));
        assert_eq!(result.unwrap_err().cloud_at, 2);
        *h.fake.sync.lock().unwrap() = Some(CloudState::Synced { at: 3 });
        rt().block_on(h.service.sync_before_launch(&registry, &starlight))
            .unwrap();
        rt().block_on(h.service.sync_after_session(&registry, &starlight));
        assert_eq!(
            h.notices().pop().unwrap().message,
            "Cloud saves uploaded to Epic Games for Starlight Courier"
        );
        // No cloud save folder: no sync at all.
        let vector = row(
            "Vector",
            "Vector Bloom",
            &EpicMeta {
                cloud_saves: true,
                ..EpicMeta::default()
            },
        );
        let before = h.notices().len();
        rt().block_on(h.service.sync_after_session(&registry, &vector));
        assert_eq!(h.notices().len(), before);
    }

    fn prep() -> StoreLaunchPrep {
        StoreLaunchPrep {
            executable: PathBuf::from("/games/S/S.exe"),
            working_dir: PathBuf::from("/games/S"),
            game_args: vec![],
            secret_args: vec![format!("-AUTH_PASSWORD={FAKE}")],
            env: vec![],
        }
    }

    #[test]
    fn the_launch_hook_asks_legendary_for_epic_rows_only() {
        let h = harness();
        h.connect();
        let meta = EpicMeta {
            can_run_offline: true,
            ..EpicMeta::default()
        };
        let starlight = row("Starlight", "Starlight Courier", &meta);
        h.fake.preps.lock().unwrap().push(Ok(prep()));
        let ticket = rt()
            .block_on(h.service.prepare(&starlight))
            .unwrap()
            .unwrap();
        assert_eq!(ticket.executable, PathBuf::from("/games/S/S.exe"));
        assert_eq!(*h.fake.prep_calls.lock().unwrap(), vec![false]);

        // Epic unreachable: offline once, for a game that may.
        h.fake.prep_calls.lock().unwrap().clear();
        h.fake
            .preps
            .lock()
            .unwrap()
            .extend([Err("network"), Ok(prep())]);
        rt().block_on(h.service.prepare(&starlight))
            .unwrap()
            .unwrap();
        assert_eq!(*h.fake.prep_calls.lock().unwrap(), vec![false, true]);

        // Other failures say so in GRID's words, never with the code.
        h.fake.preps.lock().unwrap().push(Err("Epic refused"));
        let error = rt()
            .block_on(h.service.prepare(&starlight))
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("Couldn\u{2019}t get a launch ticket from Epic Games: "),
            "{error}"
        );
        assert!(!error.contains(FAKE));

        // Signed out: offline when the game may, else a sign-in refusal.
        h.service.state.lock().unwrap().connection = StoreConnection::NotConnected;
        h.fake.prep_calls.lock().unwrap().clear();
        h.fake.preps.lock().unwrap().push(Ok(prep()));
        rt().block_on(h.service.prepare(&starlight))
            .unwrap()
            .unwrap();
        assert_eq!(*h.fake.prep_calls.lock().unwrap(), vec![true]);
        let online_only = row("Vector", "Vector Bloom", &EpicMeta::default());
        let error = rt()
            .block_on(h.service.prepare(&online_only))
            .unwrap_err()
            .to_string();
        assert_eq!(error, "Sign in to Epic Games to play Vector Bloom.");

        let gog = InstalledGame {
            source: GameSource::Store(StoreKind::Gog),
            ..Default::default()
        };
        assert!(rt().block_on(h.service.prepare(&gog)).unwrap().is_none());
    }

    #[test]
    fn the_login_spec_is_epics_launcher_redirect() {
        let h = harness();
        let spec = h.service.login_spec();
        assert_eq!(spec.store_name, "Epic Games");
        assert_eq!(
            spec.finish.as_str(),
            "http://127.0.0.1:4300/launcher/authorized"
        );
        assert_eq!(spec.code_param, "code");
        assert_eq!(spec.user_agent.as_deref(), Some(EPIC_LOGIN_USER_AGENT));
        assert!(h.service.is_art_url("http://127.0.0.1:4300/art/e1.jpg"));
        assert!(h.service.is_art_url("https://cdn2.unrealengine.com/a.jpg"));
        assert!(!h.service.is_art_url("https://romm.example/a.jpg"));
    }

    #[test]
    fn game_settings_always_take_a_compat_tool() {
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
        let out = apply_game_settings(row, &settings);
        assert_eq!(out.native_compat_tool, "GE-Proton9-20");
        assert_eq!(out.native_launch_parameters, "-dx11");
        assert!(!EpicMeta::from_json(&out.store_meta).cloud_saves);
        assert!(!EpicMeta::from_json(&out.store_meta).exe_user_pinned);
        let pinned = StoreGameSettings {
            executable: "/g/S/Other.exe".into(),
            ..settings
        };
        let out = apply_game_settings(out, &pinned);
        assert!(EpicMeta::from_json(&out.store_meta).exe_user_pinned);
        assert_eq!(out.native_launch_source, "user");
    }

    fn entry(id: u64, kind: &'static str, key: &str, completed: bool) -> JobRow {
        JobRow {
            id,
            store: true,
            kind,
            key: key.into(),
            title: "Starlight Courier".into(),
            completed,
            failed: false,
            error: String::new(),
            detail: String::new(),
        }
    }

    #[test]
    fn a_finished_epic_job_gets_one_card() {
        let h = harness();
        h.service
            .observe_rows(vec![entry(1, "store_install", "epic:S", false)]);
        assert!(h.notices().is_empty());
        let mut repair = entry(2, "store_repair", "epic:S", true);
        repair.detail = "Verified \u{b7} 2 files repaired".into();
        let mut failed = entry(3, "store_update", "epic:S", false);
        failed.failed = true;
        failed.error =
            "legendary stopped: Epic\u{2019}s download server didn\u{2019}t answer".into();
        let rows = || {
            vec![
                entry(1, "store_install", "epic:S", true),
                repair.clone_row(),
                failed.clone_row(),
                entry(4, "store_install", "gog:5", true),
            ]
        };
        h.service.observe_rows(rows());
        h.service.observe_rows(rows());
        let notices = h.notices();
        let messages: Vec<&str> = notices.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(
            messages,
            vec![
                "Starlight Courier installed",
                "Starlight Courier: 2 damaged files were downloaded again",
                "Couldn\u{2019}t update Starlight Courier",
            ]
        );
        assert_eq!(notices[0].action.as_ref().unwrap().kind, "play");
        assert!(notices[2].detail.contains("download server"));
    }

    impl JobRow {
        fn clone_row(&self) -> JobRow {
            JobRow {
                id: self.id,
                store: self.store,
                kind: self.kind,
                key: self.key.clone(),
                title: self.title.clone(),
                completed: self.completed,
                failed: self.failed,
                error: self.error.clone(),
                detail: self.detail.clone(),
            }
        }
    }

    #[test]
    fn notices_use_the_cards_words() {
        assert_eq!(
            notice::update_available("Starlight Courier", "1.0.3").message,
            "Starlight Courier: update 1.0.3 is available on Epic Games"
        );
        assert_eq!(
            notice::login_expired().message,
            "Your Epic Games login expired. Sign in again to install and update games."
        );
        assert_eq!(
            notice::finished(
                "store_repair",
                "S",
                "epic:S",
                "Verified \u{b7} 0 files repaired"
            )
            .unwrap()
            .message,
            "Verified S\u{2019}s files"
        );
        assert!(notice::tool_installing(3_093_622)
            .message
            .starts_with("Installing legendary (3 MB) for Epic Games"));
    }
}
