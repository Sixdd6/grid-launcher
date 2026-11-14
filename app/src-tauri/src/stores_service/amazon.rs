//! Amazon Games in the app: the Amazon [`StoreBackend`]. grid-core's
//! `stores::amazon::AmazonService` does the store work through nile; this
//! backend owns what the app adds around it (`docs/stores-phase3-plan.md`
//! package C), the way `epic.rs` does for Epic, without the cloud saves and
//! DLC Amazon doesn't have:
//!
//! - the connection state machine (not connected → nile installed → login
//!   window open → completing → connected; expired when Amazon refuses the
//!   saved login) and the `store-login` outcome;
//! - nile's install before the login window opens: nile prints the login
//!   URL and its PKCE values (`prepare_login`), so it must be on disk first;
//! - the owned list. **nile's own `library.json` is its only source**: GRID
//!   keeps no copy on disk, reads it at startup (`owned_local`) and holds
//!   it in memory after each `library sync`;
//! - Amazon's `StoreStatus`, and its `StoreGame`s (always the Windows
//!   version, `origin: nile`, no cloud saves, no DLC);
//! - the background `store-notice` cards ([`notice`], worded as the
//!   GameStores card);
//! - the install guards (signed in, nile ready);
//! - the [`StoreLaunchHook`]: nile's launch prep (`launch --json`) gives the
//!   game its FUEL environment. Signed in or not, Amazon games launch: nile
//!   reads only its local files. After a Disconnect (nile and its files
//!   gone) they launch as they are, from the row.
//!
//! The store calls go through [`AmazonApi`], a seam the unit tests fake.
//!
//! Secrets: the login code arrives as a [`SecretString`] and goes straight
//! to `complete_login` with the [`AmazonLoginStart`] nile gave for this
//! attempt (its `client_id`, `code_verifier` and `serial` stay in a
//! `SecretString` and never cross IPC). nile's launch environment holds the
//! user's given name (`AMAZON_GAMES_FUEL_DISPLAY_NAME`); it stays inside
//! [`StoreLaunchPrep`], whose `Debug` prints counts only. Nothing here logs a
//! token, a code, a URL with a query, or nile's output; every error shown
//! to the user is GRID's own words.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use grid_core::config::{AmazonConfig, Config};
use grid_core::launch::{LaunchError, StoreLaunchHook, StoreLaunchPrep};
use grid_core::library::queue::{DownloadStatus, DownloadsSnapshot, StoreJob};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::amazon::install as amazon_install;
use grid_core::stores::amazon::launch::launch_error;
use grid_core::stores::amazon::list::LIBRARY_FILE;
use grid_core::stores::amazon::meta::AmazonMeta;
use grid_core::stores::amazon::{
    AmazonAccount, AmazonEndpoints, AmazonGame, AmazonInstallRequest, AmazonJobCtx,
    AmazonLoginStart, AmazonService, AmazonUpdate, AMAZON_ASSOC_HANDLE, AMAZON_CODE_PARAM,
    AMAZON_SIGNIN_PATH, NILE_SYSTEM_TOO_OLD,
};
use grid_core::stores::tools::{self, StoreTool};
use grid_core::stores::{GameKey, GameSource, StoreError, StoreKind, SyncMode};
use secrecy::SecretString;

use super::{default_install_dir, BoxFut, JobCtx, StoreBackend, StoreEvent, StoreShared};
use crate::commands::stores::{
    OwnedState, StoreCloudState, StoreConnection, StoreGame, StoreGameSettings,
    StoreInstallOptions, StoreInstallRequest, StoreLibrary, StoreLoginEvent, StoreNotice,
    StoreSaveConflict, StoreStatus, StoreVersionOption, ToolState, ToolStatus,
};
use crate::store_auth::LoginSpec;

/// What the app asks of Amazon. [`RealAmazon`] forwards to grid-core's
/// `AmazonService`; the unit tests fake it.
pub trait AmazonApi: Send + Sync {
    fn endpoints(&self) -> AmazonEndpoints;
    fn tool_installed(&self) -> bool;
    fn install_tool<'a>(
        &'a self,
        progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
    ) -> BoxFut<'a, Result<(), StoreError>>;
    /// `auth --login --non-interactive`: the login URL and nile's values.
    fn start_login(&self) -> BoxFut<'_, Result<AmazonLoginStart, StoreError>>;
    /// `register` with the code and the kept values.
    fn complete_login(
        &self,
        start: AmazonLoginStart,
        code: SecretString,
    ) -> BoxFut<'_, Result<AmazonAccount, StoreError>>;
    /// Whether nile has a saved login (`current_user.json`; no run).
    /// `start_login` deletes it first, so a cancelled "Sign in again"
    /// leaves none.
    fn has_login(&self) -> bool;
    /// `auth --status` (local only).
    fn saved_account(&self) -> BoxFut<'_, Result<Option<AmazonAccount>, StoreError>>;
    /// `library sync`, then nile's `library.json`.
    fn sync(&self) -> BoxFut<'_, Result<Vec<AmazonGame>, StoreError>>;
    /// nile's `library.json` as it is.
    fn owned_local(&self) -> Vec<AmazonGame>;
    /// When nile last wrote `library.json` (Unix seconds), 0 for never.
    fn owned_local_at(&self) -> i64;
    fn download_size<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<u64, StoreError>>;
    fn install_job(&self, req: AmazonInstallRequest, ctx: AmazonJobCtx) -> StoreJob;
    fn update_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob;
    fn verify_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob;
    fn uninstall(&self, registry: &Registry, id: &str) -> Result<(), StoreError>;
    fn updates(&self) -> BoxFut<'_, Result<Vec<AmazonUpdate>, StoreError>>;
    fn launch_prep<'a>(
        &'a self,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<StoreLaunchPrep, StoreError>>;
    /// `auth --logout` (best effort), then nile's whole folder.
    fn disconnect(&self) -> BoxFut<'_, io::Result<()>>;
}

/// [`AmazonApi`] over grid-core.
pub struct RealAmazon {
    service: AmazonService,
}

impl RealAmazon {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, endpoints: AmazonEndpoints) -> Self {
        RealAmazon {
            service: AmazonService::new(data_dir, http, endpoints),
        }
    }
}

impl AmazonApi for RealAmazon {
    fn endpoints(&self) -> AmazonEndpoints {
        self.service.endpoints().clone()
    }
    fn tool_installed(&self) -> bool {
        self.service.tool_installed()
    }
    fn install_tool<'a>(
        &'a self,
        mut progress: Box<dyn FnMut(u64, u64) + Send + 'a>,
    ) -> BoxFut<'a, Result<(), StoreError>> {
        Box::pin(async move { self.service.install_tool(&mut *progress).await.map(|_| ()) })
    }
    fn start_login(&self) -> BoxFut<'_, Result<AmazonLoginStart, StoreError>> {
        Box::pin(self.service.start_login())
    }
    fn complete_login(
        &self,
        start: AmazonLoginStart,
        code: SecretString,
    ) -> BoxFut<'_, Result<AmazonAccount, StoreError>> {
        Box::pin(self.service.complete_login(start, code))
    }
    fn has_login(&self) -> bool {
        self.service.has_login()
    }
    fn saved_account(&self) -> BoxFut<'_, Result<Option<AmazonAccount>, StoreError>> {
        Box::pin(self.service.saved_account())
    }
    fn sync(&self) -> BoxFut<'_, Result<Vec<AmazonGame>, StoreError>> {
        Box::pin(self.service.sync())
    }
    fn owned_local(&self) -> Vec<AmazonGame> {
        self.service.owned_local()
    }
    fn owned_local_at(&self) -> i64 {
        std::fs::metadata(self.service.config_dir().join(LIBRARY_FILE))
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |age| age.as_secs() as i64)
    }
    fn download_size<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<u64, StoreError>> {
        Box::pin(self.service.download_size(id))
    }
    fn install_job(&self, req: AmazonInstallRequest, ctx: AmazonJobCtx) -> StoreJob {
        self.service.install_job(req, ctx)
    }
    fn update_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob {
        self.service.update_job(row, ctx)
    }
    fn verify_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob {
        self.service.verify_job(row, ctx)
    }
    fn uninstall(&self, registry: &Registry, id: &str) -> Result<(), StoreError> {
        self.service.uninstall(registry, id)
    }
    fn updates(&self) -> BoxFut<'_, Result<Vec<AmazonUpdate>, StoreError>> {
        Box::pin(self.service.updates())
    }
    fn launch_prep<'a>(
        &'a self,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<StoreLaunchPrep, StoreError>> {
        Box::pin(self.service.launch_prep(row))
    }
    fn disconnect(&self) -> BoxFut<'_, io::Result<()>> {
        Box::pin(self.service.disconnect())
    }
}

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

/// The e2e build's Amazon base URL variable (`e2e/mock-romm/mock-amazon.mjs`).
#[cfg(feature = "e2e")]
pub const E2E_AMAZON_BASE_ENV: &str = "GRID_LAUNCHER_E2E_AMAZON_BASE";

/// `StoreGame.update_version` for an Amazon game nile has an update for.
/// nile's versions are opaque ids, so there is no version to name (User
/// decisions 1: the button reads "Update"); the UI only needs it non-empty.
pub const AMAZON_UPDATE_MARK: &str = "latest";

// --- state --------------------------------------------------------------------

/// Where the login stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginPhase {
    Idle,
    /// nile installs (when missing) and prints the login URL; then the
    /// login window is open.
    Open,
    /// The redirect came; `complete_login` runs.
    Completing,
}

/// nile's state, with its download progress.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ToolPhase {
    NotInstalled,
    Installing { done: u64, total: u64 },
    Installed,
    Failed(String),
}

#[derive(Debug, Clone)]
struct AmazonState {
    connection: StoreConnection,
    username: String,
    tool: ToolPhase,
    owned: OwnedState,
    owned_error: String,
    /// nile's `library.json`, in memory only.
    games: Vec<AmazonGame>,
    refreshed_at: i64,
    /// A failed sign-in's error, left on the store's card.
    last_error: String,
    login: LoginPhase,
    /// `install --info`'s download size by product id, once asked (the
    /// disk size is the same: nile stores files as they download).
    sizes: HashMap<String, u64>,
    /// Games whose update finished since the last update check: their
    /// UPDATE tag goes until the next check says otherwise.
    updated: HashSet<String>,
}

impl Default for AmazonState {
    fn default() -> Self {
        AmazonState {
            connection: StoreConnection::NotConnected,
            username: String::new(),
            tool: ToolPhase::NotInstalled,
            owned: OwnedState::Idle,
            owned_error: String::new(),
            games: Vec::new(),
            refreshed_at: 0,
            last_error: String::new(),
            login: LoginPhase::Idle,
            sizes: HashMap::new(),
            updated: HashSet::new(),
        }
    }
}

/// The Amazon backend.
pub struct AmazonStore {
    api: Arc<dyn AmazonApi>,
    endpoints: AmazonEndpoints,
    data_dir: PathBuf,
    config_path: PathBuf,
    state: Mutex<AmazonState>,
    /// nile's answer for the sign-in in progress
    /// (`auth --login --non-interactive`), kept for `register` once the
    /// code comes back. Set by `prepare_login`, taken by
    /// `run_finish_login`; a cancel drops it.
    login: Mutex<Option<AmazonLoginStart>>,
    /// The emitter and registry every store shares.
    shared: Arc<StoreShared>,
    /// Downloads entries whose finish already had its card.
    announced: Mutex<HashSet<u64>>,
}

/// The tool's name in GRID's words.
const TOOL_NAME: &str = "nile";
const STORE_NAME: &str = "Amazon Games";
const SOURCE: &str = "amazon";
const WINDOWS: &str = "windows";
const ORIGIN: &str = "nile";

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Runs `future` to its end from synchronous code: on a blocking thread of
/// the app's runtime (`store_disconnect` calls `disconnect` through
/// `spawn_blocking`), else on a runtime of its own (tests).
fn block_on<F: Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(|runtime| runtime.block_on(future))
            .unwrap_or_else(|e| panic!("no runtime for the Amazon disconnect: {e}")),
    }
}

impl AmazonStore {
    pub fn new(
        api: Arc<dyn AmazonApi>,
        data_dir: PathBuf,
        config_path: PathBuf,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let endpoints = api.endpoints();
        Arc::new(AmazonStore {
            api,
            endpoints,
            data_dir,
            config_path,
            state: Mutex::new(AmazonState::default()),
            login: Mutex::new(None),
            shared,
            announced: Mutex::new(HashSet::new()),
        })
    }

    /// The real Amazon backend for this run.
    pub fn real(
        data_dir: PathBuf,
        config_path: PathBuf,
        http: reqwest::Client,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let api = Arc::new(RealAmazon::new(data_dir.clone(), http, amazon_endpoints()));
        AmazonStore::new(api, data_dir, config_path, shared)
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

    fn amazon_config(&self) -> AmazonConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.amazon)
            .unwrap_or_default()
    }

    fn write_amazon_config(&self, change: impl FnOnce(&mut AmazonConfig)) {
        if let Err(e) = crate::config_write::modify_config(&self.config_path, |config| {
            change(&mut config.stores.amazon);
            Ok(())
        }) {
            tracing::warn!("stores: config write failed: {e}");
        }
    }

    // --- startup --------------------------------------------------------------

    /// Startup: nile, the config's connection and nile's own `library.json`
    /// (the owned list as nile last saved it). The login check runs next
    /// ([`Self::check_login`]).
    pub fn load(&self) {
        let config = self.amazon_config();
        let tool = self.tool_phase_now();
        let (games, at) = if config.connected {
            (self.api.owned_local(), self.api.owned_local_at())
        } else {
            (Vec::new(), 0)
        };
        let mut state = self.state.lock().unwrap();
        state.tool = tool;
        if config.connected {
            state.connection = StoreConnection::Connected;
            state.username = config.username.clone();
            if !games.is_empty() {
                state.games = games;
                state.refreshed_at = at;
                state.owned = OwnedState::Ready;
            }
        }
    }

    /// Asks nile whether it still has a login (local: `auth --status`), then
    /// syncs the owned list, which is where Amazon refuses an expired
    /// login. Signed out when nile has none. A network failure keeps the
    /// saved state (installed games play offline).
    pub async fn check_login(self: &Arc<Self>) {
        if !self.amazon_config().connected {
            return;
        }
        let result = self.api.saved_account().await;
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
                // Connected in GRID's config, but nile has no login: a
                // "Sign in again" was cancelled after nile's saved login
                // went (`start_login` deletes it first). That is the
                // expired state: installed games still launch, and Sign
                // in again brings the login back.
                Ok(None) => {
                    state.connection = StoreConnection::Expired;
                    false
                }
                Err(e) => {
                    tracing::info!("stores: Amazon login not checked: {e}");
                    false
                }
            }
        };
        if let Err(e) = &result {
            self.note_system_too_old(e);
        }
        self.changed();
        if refresh && self.tool_ready() {
            self.refresh_owned(false).await;
        }
    }

    // --- status -----------------------------------------------------------------

    /// Amazon's status.
    pub fn status(&self) -> StoreStatus {
        let config = self.amazon_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap().clone();
        assemble_status(&state, &config, &rows, &self.install_base_dir_for(&config))
    }

    fn installed_rows(&self) -> Vec<InstalledGame> {
        self.registry()
            .and_then(|registry| registry.store_rows(StoreKind::Amazon).ok())
            .unwrap_or_default()
    }

    /// Where Amazon games install: the configured folder, else
    /// `<library>/Amazon Games`. `""` when neither is set.
    fn install_base_dir_for(&self, amazon: &AmazonConfig) -> String {
        default_install_dir(&self.config_path, &amazon.install_dir, STORE_NAME)
    }

    pub fn install_base_dir(&self) -> String {
        self.install_base_dir_for(&self.amazon_config())
    }

    pub fn connection(&self) -> StoreConnection {
        self.state.lock().unwrap().connection
    }

    /// nile's state on disk. nile is a frozen binary on every host (no
    /// Python check), so it is either there or not.
    fn tool_phase_now(&self) -> ToolPhase {
        if self.api.tool_installed() {
            ToolPhase::Installed
        } else {
            ToolPhase::NotInstalled
        }
    }

    pub fn tool_ready(&self) -> bool {
        matches!(self.state.lock().unwrap().tool, ToolPhase::Installed)
    }

    // --- login ------------------------------------------------------------------

    /// The login is starting. `Err` while a sign-in is finishing.
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

    /// Before the login window opens: nile installs when it is missing
    /// (the "Installing nile (11 MB) for Amazon Games…" card), then prints
    /// this attempt's login URL and the values `register` needs, which are
    /// kept for [`Self::finish_login`]. `Err` (GRID's words) leaves no
    /// window.
    pub async fn prepare_sign_in(self: &Arc<Self>) -> Result<LoginSpec, String> {
        self.begin_login()?;
        if !self.api.tool_installed() {
            self.install_tool_then(false, true).await;
        } else {
            let phase = self.tool_phase_now();
            self.state.lock().unwrap().tool = phase;
        }
        let tool = self.state.lock().unwrap().tool.clone();
        if tool != ToolPhase::Installed {
            self.state.lock().unwrap().login = LoginPhase::Idle;
            return Err(match tool {
                ToolPhase::Failed(problem) => problem,
                _ => StoreError::ToolMissing(TOOL_NAME).to_string(),
            });
        }
        let result = self.api.start_login().await;
        // nile's saved login is gone now (it refuses a new sign-in while
        // one is saved): a connected store shows as expired until the
        // sign-in finishes, and stays so when it is cancelled.
        self.mark_expired_without_login();
        match result {
            Ok(start) => {
                *self.login.lock().unwrap() = Some(start);
                Ok(self.login_spec())
            }
            Err(error) => {
                self.state.lock().unwrap().login = LoginPhase::Idle;
                self.note_system_too_old(&error);
                Err(error.to_string())
            }
        }
    }

    /// Connected in GRID's config while nile has no saved login: the
    /// expired state ([`Self::check_login`]).
    fn mark_expired_without_login(&self) {
        if !self.amazon_config().connected || self.api.has_login() {
            return;
        }
        let changed = {
            let mut state = self.state.lock().unwrap();
            let changed = state.connection != StoreConnection::Expired;
            state.connection = StoreConnection::Expired;
            changed
        };
        if changed {
            self.changed();
        }
    }

    /// nile can't run on this computer (its glibc floor): the tool shows
    /// as failed with grid-core's words, in Settings and the store panel.
    fn note_system_too_old(&self, error: &StoreError) {
        if !matches!(error, StoreError::Refused(text) if text == NILE_SYSTEM_TOO_OLD) {
            return;
        }
        self.state.lock().unwrap().tool = ToolPhase::Failed(NILE_SYSTEM_TOO_OLD.to_string());
        self.changed();
    }

    /// The window could not open.
    pub fn login_window_failed(&self, error: &str) {
        self.login.lock().unwrap().take();
        self.state.lock().unwrap().login = LoginPhase::Idle;
        self.fail_login(&format!("the login window didn't open ({error})"));
    }

    #[cfg(test)]
    pub fn login_phase(&self) -> LoginPhase {
        self.state.lock().unwrap().login
    }

    /// ✕, Cancel or Esc: silent, nothing changes; nile's values for this
    /// attempt go.
    pub fn login_cancelled(&self) {
        self.login.lock().unwrap().take();
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
        self.login.lock().unwrap().take();
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

    /// The redirect's code: nile registers this computer with it (and the
    /// values kept from `prepare_login`), then GRID syncs the owned list.
    /// nile's `register` already syncs once; GRID's sync after it is the
    /// one that counts.
    pub async fn finish_login(self: &Arc<Self>, code: SecretString) {
        self.state.lock().unwrap().login = LoginPhase::Completing;
        let Some(start) = self.login.lock().unwrap().take() else {
            drop(code);
            self.state.lock().unwrap().login = LoginPhase::Idle;
            self.fail_login(&format!("{STORE_NAME} returned an error. Try again."));
            return;
        };
        let result = self.api.complete_login(start, code).await;
        match result {
            Ok(account) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.login = LoginPhase::Idle;
                    state.connection = StoreConnection::Connected;
                    state.username = account.username.clone();
                    state.last_error.clear();
                    state.tool = ToolPhase::Installed;
                }
                let username = account.username.clone();
                self.write_amazon_config(move |amazon| {
                    amazon.connected = true;
                    amazon.username = username;
                });
                self.emit(StoreEvent::Login(StoreLoginEvent {
                    source: SOURCE.to_string(),
                    outcome: "connected".to_string(),
                    username: account.username.clone(),
                    error: String::new(),
                }));
                self.changed();
                self.refresh_owned(true).await;
            }
            Err(error) => {
                self.state.lock().unwrap().login = LoginPhase::Idle;
                self.fail_login(&error.to_string());
            }
        }
    }

    /// Disconnect: nile deregisters this computer (best effort), then its
    /// whole folder goes (its token files, `library.json` and nile itself),
    /// with the login window's cookies. Installed games stay and still
    /// play, from their rows.
    pub fn disconnect(&self) -> Result<(), String> {
        self.login.lock().unwrap().take();
        if let Err(e) = block_on(self.api.disconnect()) {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(format!(
                    "Couldn\u{2019}t delete {STORE_NAME}\u{2019}s login: {e}"
                ));
            }
        }
        if let Err(e) = crate::store_auth::wipe_login_data(&self.data_dir, SOURCE) {
            tracing::warn!("stores: login window data not deleted: {e}");
        }
        self.write_amazon_config(|amazon| {
            amazon.connected = false;
            amazon.username.clear();
        });
        {
            let mut state = self.state.lock().unwrap();
            let login = state.login;
            *state = AmazonState {
                login,
                tool: self.tool_phase_now(),
                ..AmazonState::default()
            };
        }
        self.changed();
        Ok(())
    }

    // --- tool ---------------------------------------------------------------------

    /// Installs nile (pinned, SHA-256 checked), then loads the owned list
    /// when it never loaded. One at a time.
    pub async fn install_tool(self: &Arc<Self>) {
        self.install_tool_then(true, false).await;
    }

    /// `announce`: the "Installing nile…" card (before a sign-in; the
    /// frontend words its own Install button).
    async fn install_tool_then(self: &Arc<Self>, load_owned: bool, announce: bool) {
        {
            let mut state = self.state.lock().unwrap();
            if matches!(state.tool, ToolPhase::Installing { .. }) {
                return;
            }
            state.tool = ToolPhase::Installing { done: 0, total: 0 };
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

    /// `library sync`, then nile's `library.json`. `announce`: the "N games
    /// found" card (after a connect, and on Refresh).
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
        let result = self.api.sync().await;
        match result {
            Ok(games) => {
                let installed: HashSet<String> = self
                    .installed_rows()
                    .into_iter()
                    .map(|row| row.store_id)
                    .collect();
                let to_install = games
                    .iter()
                    .filter(|game| !installed.contains(&game.product_id))
                    .count();
                let count = games.len();
                {
                    let mut state = self.state.lock().unwrap();
                    state.games = games;
                    state.refreshed_at = now();
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
                {
                    let mut state = self.state.lock().unwrap();
                    state.owned = OwnedState::Error;
                    state.owned_error = error.to_string();
                }
                self.note_system_too_old(&error);
            }
        }
        self.changed();
    }

    /// The owned game for `product_id`.
    pub fn game(&self, product_id: &str) -> Option<AmazonGame> {
        self.state
            .lock()
            .unwrap()
            .games
            .iter()
            .find(|game| game.product_id == product_id)
            .cloned()
    }

    /// The installed games nile has an update for (`list-updates`). A new
    /// check forgets which updates finished since the last one.
    pub async fn find_updates(&self) -> Result<Vec<AmazonUpdate>, StoreError> {
        let found = self.api.updates().await?;
        self.state.lock().unwrap().updated.clear();
        Ok(found)
    }

    // --- library ------------------------------------------------------------------

    /// Amazon's Installed and Available lists. `updates`: the games with an
    /// update by product id ([`crate::update_service`]).
    pub fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary {
        let config = self.amazon_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap().clone();
        let status = assemble_status(&state, &config, &rows, &self.install_base_dir_for(&config));
        let (installed, available) = assemble_games(&state, &rows, updates);
        StoreLibrary {
            status,
            installed,
            available,
        }
    }

    // --- launch ---------------------------------------------------------------------

    /// nile's launch prep for an Amazon row (its FUEL environment). nile
    /// needs no login or network for it, so the login state doesn't
    /// matter. With no nile (after a Disconnect deleted it, with its files)
    /// the game launches as it is, from the row. Failures take nile's
    /// words; there is no retry.
    pub async fn launch_prep(
        &self,
        row: &InstalledGame,
    ) -> Result<Option<StoreLaunchPrep>, LaunchError> {
        if self.connection() == StoreConnection::NotConnected || !self.api.tool_installed() {
            return Ok(None);
        }
        match self.api.launch_prep(row).await {
            Ok(prep) => Ok(Some(prep)),
            Err(e) => {
                self.note_system_too_old(&e);
                Err(launch_error(&row.title, &e))
            }
        }
    }

    // --- Downloads ------------------------------------------------------------------

    /// Every Downloads snapshot: an Amazon job that just finished gets its
    /// card ("Fogline installed" · Play, "… updated", "Verified …"), a
    /// failed one its error card, and the store's counts change.
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
                })
                .collect(),
        );
    }

    fn observe_rows(&self, rows: Vec<JobRow>) {
        let prefix = format!("{SOURCE}:");
        let mut finished = Vec::new();
        {
            let mut announced = self.announced.lock().unwrap();
            for entry in &rows {
                if !entry.store
                    || !entry.key.starts_with(&prefix)
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
            if entry.completed && entry.kind == "store_update" {
                let id = entry.key[prefix.len()..].to_string();
                self.state.lock().unwrap().updated.insert(id);
            }
            let card = if entry.completed {
                notice::finished(entry.kind, &entry.title, &entry.key)
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

    pub fn set_install_dir(&self, path: &str) {
        let path = path.trim().to_string();
        self.write_amazon_config(move |amazon| amazon.install_dir = path);
        self.changed();
    }

    /// Refuses an install while signed out or without nile.
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
        self.note_system_too_old(&error);
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

    /// The login window's page: the URL nile printed for the sign-in in
    /// progress, else the sign-in page's address with no query.
    fn login_url(&self) -> tauri::Url {
        if let Some(start) = self.login.lock().unwrap().as_ref() {
            return start.url.clone();
        }
        let ep = &self.endpoints;
        let host = ep
            .login_hosts
            .first()
            .cloned()
            .unwrap_or_else(|| ep.finish.clone());
        host.join(AMAZON_SIGNIN_PATH).unwrap_or(host)
    }

    /// The login window's Amazon spec: the sign-in page nile printed, ended
    /// by Amazon's redirect to `https://www.amazon.com/?openid.assoc_handle=
    /// amzn_sonic_games_launcher&…` (the assoc_handle tells it from
    /// Amazon's home page, which has the same path).
    pub fn login_spec(&self) -> LoginSpec {
        LoginSpec {
            store_name: STORE_NAME.to_string(),
            login_url: self.login_url(),
            finish: self.endpoints.finish.clone(),
            finish_query: Some(AMAZON_ASSOC_HANDLE),
            code_param: AMAZON_CODE_PARAM,
            user_agent: None,
        }
    }

    /// Whether `url` is store art the UI may load (Amazon's image hosts,
    /// the e2e mock's origin).
    pub fn is_art_url(&self, url: &str) -> bool {
        grid_core::stores::art::is_store_art_url(url, &self.endpoints.art_origins)
    }

    // --- the commands' Amazon work --------------------------------------------------

    /// `store_install_options` for Amazon: the Windows version, its size
    /// (`install --info`; the disk size is the same) and the folder.
    async fn install_options_for(&self, product_id: &str) -> Result<StoreInstallOptions, String> {
        self.ensure_can_install()?;
        let size = self
            .api
            .download_size(product_id)
            .await
            .map_err(|e| self.store_error(e))?;
        self.state
            .lock()
            .unwrap()
            .sizes
            .insert(product_id.to_string(), size);
        let title = self
            .game(product_id)
            .map(|game| game.title)
            .unwrap_or_else(|| product_id.to_string());
        let compat = Config::load(&self.config_path)
            .map(|config| config.default_compat_tool)
            .unwrap_or_default();
        let base = self.install_base_dir();
        let id = product_id.to_string();
        tokio::task::spawn_blocking(move || install_options_ipc(size, &title, &id, &base, &compat))
            .await
            .map_err(|e| e.to_string())
    }

    /// `store_install` for Amazon: the job for a confirmed install.
    fn install_job_for(
        &self,
        product_id: String,
        request: StoreInstallRequest,
        ctx: JobCtx,
    ) -> Result<StoreJob, String> {
        self.ensure_can_install()?;
        let platform = request.platform.trim();
        if !platform.is_empty() && platform != WINDOWS {
            return Err(format!(
                "GRID installs the Windows version of {STORE_NAME} games."
            ));
        }
        let title = self
            .game(&product_id)
            .map(|game| game.title)
            .unwrap_or_else(|| product_id.clone());
        let install_dir = if request.install_dir.trim().is_empty() {
            let base = self.install_base_dir();
            if base.is_empty() {
                return Err("Choose a library folder in Settings › Library first.".to_string());
            }
            game_folder(&base, &title, &product_id)
        } else {
            PathBuf::from(request.install_dir.trim())
        };
        Ok(self.api.install_job(
            AmazonInstallRequest {
                product_id,
                title,
                install_dir,
                compat_tool: request.compat_tool,
            },
            ctx.amazon(),
        ))
    }
}

/// A game's own folder under the store's install folder:
/// `<base>/<the folder name nile would give the title>` (grid-core's
/// `install::folder_name`; `install --path` uses it as it is).
fn game_folder(base: &str, title: &str, product_id: &str) -> PathBuf {
    Path::new(base).join(amazon_install::folder_name(title, product_id))
}

impl StoreBackend for AmazonStore {
    fn kind(&self) -> StoreKind {
        StoreKind::Amazon
    }
    fn status(&self) -> StoreStatus {
        AmazonStore::status(self)
    }
    fn connection(&self) -> StoreConnection {
        AmazonStore::connection(self)
    }
    fn changed(&self) {
        AmazonStore::changed(self)
    }
    fn load(&self) {
        AmazonStore::load(self)
    }
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { AmazonStore::check_login(&self).await })
    }
    fn observe_downloads(&self, snapshot: &DownloadsSnapshot) {
        AmazonStore::observe_downloads(self, snapshot)
    }
    fn login_spec(&self) -> LoginSpec {
        AmazonStore::login_spec(self)
    }
    fn begin_login(&self) -> Result<(), String> {
        AmazonStore::begin_login(self)
    }
    fn prepare_login(self: Arc<Self>) -> BoxFut<'static, Result<LoginSpec, String>> {
        Box::pin(async move { AmazonStore::prepare_sign_in(&self).await })
    }
    fn login_window_failed(&self, error: &str) {
        AmazonStore::login_window_failed(self, error)
    }
    fn login_cancelled(&self) {
        AmazonStore::login_cancelled(self)
    }
    fn login_redirect_failed(&self) {
        AmazonStore::login_redirect_failed(self)
    }
    fn run_finish_login(self: Arc<Self>, code: SecretString) -> BoxFut<'static, ()> {
        Box::pin(async move { AmazonStore::finish_login(&self, code).await })
    }
    fn disconnect(&self) -> Result<(), String> {
        AmazonStore::disconnect(self)
    }
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { AmazonStore::install_tool(&self).await })
    }
    fn run_refresh(self: Arc<Self>, announce: bool) -> BoxFut<'static, ()> {
        Box::pin(async move { AmazonStore::refresh_owned(&self, announce).await })
    }
    fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary {
        AmazonStore::library(self, updates)
    }
    fn check_updates(self: Arc<Self>, app: tauri::AppHandle) {
        use tauri::Manager as _;
        let updates = app.state::<crate::commands::AppState>().updates.clone();
        updates.spawn_store_check(app, self);
    }
    fn set_install_dir(&self, path: &str) {
        AmazonStore::set_install_dir(self, path)
    }
    fn set_cloud_saves(&self, _on: bool) {
        // Amazon has no cloud saves.
    }
    fn ensure_can_install(&self) -> Result<(), String> {
        AmazonStore::ensure_can_install(self)
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
        Box::pin(async move { self.install_job_for(store_id, request, ctx) })
    }
    fn update_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.api.update_job(row, ctx.amazon())
    }
    fn repair_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob {
        self.api.verify_job(row, ctx.amazon())
    }
    fn dlc_job(
        self: Arc<Self>,
        _row: InstalledGame,
        _dlc_id: String,
        _ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>> {
        // Amazon has no DLC model: an add-on is just another product.
        Box::pin(async { Err(format!("{STORE_NAME} games have no DLC in GRID.")) })
    }
    fn remove_dlc(
        &self,
        _registry: &Registry,
        _row: &InstalledGame,
        _dlc_id: &str,
    ) -> Result<(), StoreError> {
        Err(StoreError::Refused(format!(
            "{STORE_NAME} games have no DLC in GRID."
        )))
    }
    fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
        let result = self.api.uninstall(registry, store_id);
        if result.is_ok() {
            self.state.lock().unwrap().updated.remove(store_id);
        }
        result
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
        Box::pin(async { Err(format!("{STORE_NAME} has no cloud saves.")) })
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
        AmazonStore::is_art_url(self, url)
    }
    fn launch_hook(self: Arc<Self>) -> Option<Arc<dyn StoreLaunchHook>> {
        Some(self)
    }
}

/// The launch hook: an Amazon row starts with nile's launch prep (the
/// executable, its arguments and the five FUEL variables); every other row
/// launches as it is.
impl StoreLaunchHook for AmazonStore {
    fn prepare<'a>(
        &'a self,
        game: &'a InstalledGame,
    ) -> BoxFut<'a, Result<Option<StoreLaunchPrep>, LaunchError>> {
        Box::pin(async move {
            if game.source != GameSource::Store(StoreKind::Amazon) {
                return Ok(None);
            }
            self.launch_prep(game).await
        })
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

/// The parts of a Downloads entry [`AmazonStore::observe_downloads`] reads.
struct JobRow {
    id: u64,
    store: bool,
    kind: &'static str,
    key: String,
    title: String,
    completed: bool,
    failed: bool,
    error: String,
}

/// nile's download size for the cards ("Installing nile (11 MB)…").
fn tool_size_bytes() -> u64 {
    tools::host_asset(StoreTool::Nile).map_or(0, |asset| asset.size)
}

// --- status and library assembly (pure) --------------------------------------

fn assemble_status(
    state: &AmazonState,
    config: &AmazonConfig,
    rows: &[InstalledGame],
    install_dir: &str,
) -> StoreStatus {
    let pin = tools::pin(StoreTool::Nile);
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
        .filter(|game| !game.product_id.is_empty())
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
        cloud_saves: false,
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

fn assemble_games(
    state: &AmazonState,
    rows: &[InstalledGame],
    updates: &HashMap<String, String>,
) -> (Vec<StoreGame>, Vec<StoreGame>) {
    let games: HashMap<&str, &AmazonGame> = state
        .games
        .iter()
        .map(|game| (game.product_id.as_str(), game))
        .collect();
    let installed: Vec<StoreGame> = rows
        .iter()
        .map(|row| {
            let meta = AmazonMeta::from_json(&row.store_meta);
            let game = games.get(row.store_id.as_str()).copied();
            let has_update =
                updates.contains_key(&row.store_id) && !state.updated.contains(&row.store_id);
            StoreGame {
                key: GameKey::store(StoreKind::Amazon, row.store_id.clone()).to_string(),
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
                genres: game
                    .map(|game| game.genres.clone())
                    .filter(|genres| !genres.is_empty())
                    .unwrap_or_else(|| split_list(&row.genres)),
                year: game
                    .map(|game| game.year.clone())
                    .filter(|year| !year.is_empty())
                    .unwrap_or_else(|| year_of(&row.first_release_date)),
                platforms: vec![WINDOWS.to_string()],
                installed_platform: Some(WINDOWS.to_string()),
                download_bytes: 0,
                disk_bytes: row.filesize_bytes.max(0) as u64,
                version: meta.version.clone(),
                update_version: if has_update {
                    AMAZON_UPDATE_MARK.to_string()
                } else {
                    String::new()
                },
                origin: Some(ORIGIN.to_string()),
                not_supported: String::new(),
                dlc: Vec::new(),
                cloud: "none".to_string(),
                last_sync: 0,
                install_dir: row.native_game_dir.clone(),
                compat_tool: row.native_compat_tool.clone(),
            }
        })
        .collect();
    let installed_ids: HashSet<&str> = rows.iter().map(|row| row.store_id.as_str()).collect();
    let available = state
        .games
        .iter()
        .filter(|game| !game.product_id.is_empty())
        .filter(|game| !installed_ids.contains(game.product_id.as_str()))
        .map(|game| {
            let size = state.sizes.get(&game.product_id).copied().unwrap_or(0);
            StoreGame {
                key: GameKey::store(StoreKind::Amazon, game.product_id.clone()).to_string(),
                source: SOURCE.to_string(),
                store_id: game.product_id.clone(),
                title: game.title.clone(),
                installed: false,
                cover: game.cover.clone(),
                screenshots: game.screenshots.clone(),
                description: game.description.clone(),
                developer: game.developer.clone(),
                genres: game.genres.clone(),
                year: game.year.clone(),
                platforms: vec![WINDOWS.to_string()],
                installed_platform: None,
                download_bytes: size,
                disk_bytes: size,
                version: String::new(),
                update_version: String::new(),
                origin: None,
                not_supported: String::new(),
                dlc: Vec::new(),
                cloud: "none".to_string(),
                last_sync: 0,
                install_dir: String::new(),
                compat_tool: String::new(),
            }
        })
        .collect();
    (installed, available)
}

/// `StoreInstallOptions` for the install dialog: the Windows version only
/// (no Version or Language choice, no DLC), `size` for both the download
/// and the disk (nile stores files as they download), and the folder
/// `<base>/<title>`.
pub fn install_options_ipc(
    size: u64,
    title: &str,
    product_id: &str,
    base_dir: &str,
    compat_tool: &str,
) -> StoreInstallOptions {
    let install_dir = if base_dir.is_empty() {
        String::new()
    } else {
        game_folder(base_dir, title, product_id)
            .to_string_lossy()
            .into_owned()
    };
    let probe = (!install_dir.is_empty()).then(|| PathBuf::from(&install_dir));
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
            download_bytes: size,
            disk_bytes: size,
        }],
        languages: Vec::new(),
        default_language: String::new(),
        dlc: Vec::new(),
        install_dir,
        free_bytes,
        volume,
        compat_tool: compat_tool.to_string(),
    }
}

// --- notices --------------------------------------------------------------------

/// Amazon's `store-notice` cards, in the GameStores card's words. Only what
/// happens in the background: the frontend raises the cards for the
/// commands it calls itself (queued, refusals, sign-in outcome from
/// `store-login`, uninstalled, disconnected). No cloud cards: Amazon has
/// no cloud saves.
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

    /// nile starts installing before the login window opens.
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

    /// nile names no version (its ids are opaque): "Fogline: an update is
    /// available" (User decisions 1).
    pub fn update_available(title: &str) -> StoreNotice {
        card(
            format!("{title}: an update is available"),
            false,
            action("show_updates", None, None),
        )
    }

    /// The card for an Amazon Downloads row that just finished, by its
    /// kind. nile gives no repaired-file count, so a verify reads
    /// "Verified …'s files".
    pub fn finished(kind: &str, title: &str, key: &str) -> Option<StoreNotice> {
        match kind {
            "store_install" => Some(installed(title, key)),
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

    /// The card for an Amazon Downloads row that just failed.
    pub fn failed(kind: &str, title: &str, error: &str) -> Option<StoreNotice> {
        let what = match kind {
            "store_install" => "install",
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
    use std::sync::atomic::AtomicBool;
    use tauri::Url;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";
    const FOG: &str = "amzn1.adg.product.f0g11ne0-0000-4000-8000-000000000001";
    const KETTLE: &str = "amzn1.adg.product.ke77le00-0000-4000-8000-000000000002";

    /// A scripted Amazon.
    #[derive(Default)]
    struct FakeAmazon {
        tool: Mutex<bool>,
        tool_result: Mutex<Option<Result<(), String>>>,
        start: Mutex<Option<Result<(), &'static str>>>,
        starts: Mutex<u32>,
        login: Mutex<Option<Result<AmazonAccount, &'static str>>>,
        /// The login URL each `complete_login` got (its start's).
        completed: Mutex<Vec<String>>,
        account: Mutex<Option<Result<Option<AmazonAccount>, &'static str>>>,
        sync: Mutex<Option<Result<Vec<AmazonGame>, &'static str>>>,
        local: Mutex<Vec<AmazonGame>>,
        size: Mutex<u64>,
        installs: Mutex<Vec<AmazonInstallRequest>>,
        updates: Mutex<Option<Result<Vec<AmazonUpdate>, &'static str>>>,
        preps: Mutex<Vec<Result<StoreLaunchPrep, &'static str>>>,
        prep_calls: Mutex<u32>,
        disconnects: Mutex<u32>,
        disconnect_error: Mutex<Option<io::ErrorKind>>,
        /// nile's saved login (`current_user.json`).
        saved: Mutex<bool>,
    }

    fn err_of(text: &str) -> StoreError {
        match text {
            "expired" => StoreError::LoginExpired,
            "network" => StoreError::Network("Amazon Games".into()),
            "too_old" => StoreError::Refused(NILE_SYSTEM_TOO_OLD.to_string()),
            other => StoreError::ToolFailed {
                tool: "nile",
                reason: other.to_string(),
            },
        }
    }

    fn base() -> Url {
        Url::parse("http://127.0.0.1:4400/").unwrap()
    }

    impl AmazonApi for FakeAmazon {
        fn endpoints(&self) -> AmazonEndpoints {
            AmazonEndpoints::e2e(base())
        }
        fn tool_installed(&self) -> bool {
            *self.tool.lock().unwrap()
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
        fn start_login(&self) -> BoxFut<'_, Result<AmazonLoginStart, StoreError>> {
            *self.starts.lock().unwrap() += 1;
            let n = *self.starts.lock().unwrap();
            let result = self.start.lock().unwrap().take().unwrap_or(Ok(()));
            // nile's saved login goes first, as grid-core's `start_login`.
            *self.saved.lock().unwrap() = false;
            Box::pin(async move {
                result.map_err(err_of)?;
                Ok(AmazonLoginStart::new(
                    base()
                        .join(&format!(
                            "/ap/signin?openid.assoc_handle=amzn_sonic_games_launcher&attempt={n}"
                        ))
                        .unwrap(),
                    SecretString::from(format!(r#"{{"code_verifier":"{FAKE}"}}"#)),
                ))
            })
        }
        fn complete_login(
            &self,
            start: AmazonLoginStart,
            _code: SecretString,
        ) -> BoxFut<'_, Result<AmazonAccount, StoreError>> {
            self.completed.lock().unwrap().push(start.url.to_string());
            let result = self
                .login
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Err("no script"));
            if result.is_ok() {
                *self.saved.lock().unwrap() = true;
            }
            Box::pin(async move { result.map_err(err_of) })
        }
        fn has_login(&self) -> bool {
            *self.saved.lock().unwrap()
        }
        fn saved_account(&self) -> BoxFut<'_, Result<Option<AmazonAccount>, StoreError>> {
            let result = self.account.lock().unwrap().take().unwrap_or(Ok(None));
            Box::pin(async move { result.map_err(err_of) })
        }
        fn sync(&self) -> BoxFut<'_, Result<Vec<AmazonGame>, StoreError>> {
            let result = self.sync.lock().unwrap().take().unwrap_or(Ok(Vec::new()));
            Box::pin(async move { result.map_err(err_of) })
        }
        fn owned_local(&self) -> Vec<AmazonGame> {
            self.local.lock().unwrap().clone()
        }
        fn owned_local_at(&self) -> i64 {
            77
        }
        fn download_size<'a>(&'a self, _id: &'a str) -> BoxFut<'a, Result<u64, StoreError>> {
            let size = *self.size.lock().unwrap();
            Box::pin(async move { Ok(size) })
        }
        fn install_job(&self, req: AmazonInstallRequest, _ctx: AmazonJobCtx) -> StoreJob {
            let id = req.product_id.clone();
            self.installs.lock().unwrap().push(req);
            job(id, StoreOp::Install)
        }
        fn update_job(&self, row: &InstalledGame, _ctx: AmazonJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Update)
        }
        fn verify_job(&self, row: &InstalledGame, _ctx: AmazonJobCtx) -> StoreJob {
            job(row.store_id.clone(), StoreOp::Repair)
        }
        fn uninstall(&self, _r: &Registry, _id: &str) -> Result<(), StoreError> {
            Ok(())
        }
        fn updates(&self) -> BoxFut<'_, Result<Vec<AmazonUpdate>, StoreError>> {
            let result = self
                .updates
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(Vec::new()));
            Box::pin(async move { result.map_err(err_of) })
        }
        fn launch_prep<'a>(
            &'a self,
            _row: &'a InstalledGame,
        ) -> BoxFut<'a, Result<StoreLaunchPrep, StoreError>> {
            *self.prep_calls.lock().unwrap() += 1;
            let mut preps = self.preps.lock().unwrap();
            let next = if preps.is_empty() {
                Err("no script")
            } else {
                preps.remove(0)
            };
            Box::pin(async move { next.map_err(err_of) })
        }
        fn disconnect(&self) -> BoxFut<'_, io::Result<()>> {
            *self.disconnects.lock().unwrap() += 1;
            let error = *self.disconnect_error.lock().unwrap();
            if error.is_none() {
                *self.tool.lock().unwrap() = false;
            }
            Box::pin(async move {
                match error {
                    Some(kind) => Err(io::Error::from(kind)),
                    None => Ok(()),
                }
            })
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
            source: StoreKind::Amazon,
            store_id,
            op,
            title: String::new(),
            drive: None,
            runner: Arc::new(NoRunner),
        }
    }

    struct Harness {
        service: Arc<AmazonStore>,
        fake: Arc<FakeAmazon>,
        events: Arc<Mutex<Vec<StoreEvent>>>,
        dir: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let fake = Arc::new(FakeAmazon::default());
        let shared = StoreShared::new();
        let service = AmazonStore::new(
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
        fn messages(&self) -> Vec<String> {
            self.notices().into_iter().map(|n| n.message).collect()
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
        fn config(&self) -> AmazonConfig {
            Config::load(&self.dir.path().join("config.toml"))
                .unwrap()
                .stores
                .amazon
        }
        fn connect_config(&self) {
            crate::config_write::modify_config(&self.dir.path().join("config.toml"), |c| {
                c.stores.amazon.connected = true;
                c.stores.amazon.username = "sam.plays".into();
                Ok(())
            })
            .unwrap();
        }
        fn connect(&self) {
            let mut state = self.service.state.lock().unwrap();
            state.connection = StoreConnection::Connected;
            state.tool = ToolPhase::Installed;
        }
    }

    fn game(id: &str, title: &str) -> AmazonGame {
        AmazonGame {
            product_id: id.into(),
            entitlement_id: format!("ent-{title}"),
            title: title.into(),
            cover: format!("http://127.0.0.1:4400/art/{title}.jpg"),
            screenshots: vec![format!("http://127.0.0.1:4400/art/{title}_shot.jpg")],
            description: format!("{title}, a game."),
            developer: "Mistral".into(),
            year: "2020".into(),
            genres: vec!["Adventure".into()],
        }
    }

    fn three() -> Vec<AmazonGame> {
        vec![
            game(FOG, "Fogline"),
            game(KETTLE, "Kettle Drum"),
            game("amzn1.adg.product.lowsun", "Lowsun"),
        ]
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

    fn account(name: &str) -> AmazonAccount {
        AmazonAccount {
            username: name.into(),
        }
    }

    #[test]
    fn nile_installs_before_the_window_and_its_url_opens_it() {
        let h = harness();
        h.service.load();
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.tool.state, ToolState::NotInstalled);
        assert_eq!(status.tool.name, "nile");
        assert_eq!(status.tool.version, "1.2.0");
        assert!(!status.cloud_saves);
        assert!(!status.in_rail);

        let spec = rt().block_on(h.service.prepare_sign_in()).unwrap();
        assert_eq!(h.service.login_phase(), LoginPhase::Open);
        assert_eq!(h.service.status().tool.state, ToolState::Installed);
        assert_eq!(
            h.messages(),
            vec![
                notice::tool_installing(tool_size_bytes()).message,
                "nile installed".to_string(),
            ]
        );
        assert!(h.messages()[0].starts_with("Installing nile"));
        assert!(h.messages()[0].ends_with(" for Amazon Games\u{2026}"));
        // The window opens nile's URL for this attempt.
        assert!(
            spec.login_url.as_str().contains("attempt=1"),
            "{}",
            spec.login_url
        );
        assert_eq!(spec.finish.as_str(), "http://127.0.0.1:4400/");
        assert_eq!(spec.finish_query, Some(AMAZON_ASSOC_HANDLE));
        assert_eq!(spec.code_param, "openid.oa2.authorization_code");
        assert_eq!(spec.user_agent, None);
        assert_eq!(h.service.login_spec().login_url, spec.login_url);
        // No login event yet.
        assert!(h.logins().is_empty());
    }

    #[test]
    fn prepare_login_fails_with_nile_s_words_and_leaves_no_window() {
        let h = harness();
        *h.fake.tool_result.lock().unwrap() = Some(Err(
            "The nile download didn't match its checksum, so GRID deleted it.".into(),
        ));
        let error = rt().block_on(h.service.prepare_sign_in()).unwrap_err();
        assert!(error.contains("checksum"), "{error}");
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
        assert_eq!(h.service.status().tool.state, ToolState::Failed);
        assert_eq!(
            h.notices().pop().unwrap().action.unwrap().kind,
            "install_tool"
        );
        assert_eq!(*h.fake.starts.lock().unwrap(), 0, "nile never asked");

        // nile there, but it can't print a login URL.
        *h.fake.tool.lock().unwrap() = true;
        *h.fake.start.lock().unwrap() = Some(Err("auth failed"));
        let error = rt().block_on(h.service.prepare_sign_in()).unwrap_err();
        assert_eq!(error, "nile stopped: auth failed");
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
        assert_eq!(
            h.service.login_spec().login_url.as_str(),
            "http://127.0.0.1:4400/ap/signin"
        );
        // Busy while a sign-in finishes.
        h.service.state.lock().unwrap().login = LoginPhase::Completing;
        assert!(rt().block_on(h.service.prepare_sign_in()).is_err());
    }

    #[test]
    fn a_login_registers_with_this_attempt_s_values_then_syncs() {
        let h = harness();
        *h.fake.tool.lock().unwrap() = true;
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        *h.fake.login.lock().unwrap() = Some(Ok(account("sam.plays")));
        *h.fake.sync.lock().unwrap() = Some(Ok(three()));
        rt().block_on(h.service.finish_login(code()));

        assert_eq!(
            h.fake.completed.lock().unwrap().as_slice(),
            ["http://127.0.0.1:4400/ap/signin?openid.assoc_handle=amzn_sonic_games_launcher&attempt=1"]
        );
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.username, "sam.plays");
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.owned_count, 3);
        assert!(status.in_rail);
        assert_eq!(status.last_error, "");
        assert_eq!(h.service.login_phase(), LoginPhase::Idle);
        assert!(h.config().connected);
        assert_eq!(h.config().username, "sam.plays");
        let login = &h.logins()[0];
        assert_eq!(login.outcome, "connected");
        assert_eq!(login.username, "sam.plays");
        assert_eq!(
            h.messages(),
            vec!["Amazon Games: 3 games found \u{b7} 3 to install".to_string()]
        );
        // The start is used once: the spec falls back to the bare page.
        assert_eq!(
            h.service.login_spec().login_url.as_str(),
            "http://127.0.0.1:4400/ap/signin"
        );
        // No owned-list copy of GRID's own: nile's library.json is the one.
        assert!(!h.dir.path().join("stores").join("cache").exists());
        let events = format!("{:?}", h.events.lock().unwrap());
        assert!(!events.contains(FAKE), "{events}");
    }

    #[test]
    fn a_refused_code_fails_the_login_with_its_words() {
        let h = harness();
        *h.fake.tool.lock().unwrap() = true;
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        *h.fake.login.lock().unwrap() = Some(Err("Amazon didn't accept the sign-in. Try again."));
        rt().block_on(h.service.finish_login(code()));
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(
            status.last_error,
            "Couldn\u{2019}t sign in to Amazon Games: nile stopped: Amazon didn't accept the sign-in. Try again."
        );
        assert_eq!(h.logins()[0].outcome, "failed");
        assert!(h.notices().is_empty());
        assert!(!h.config().connected);
        // A code with no kept start (the window outlived it) fails too.
        rt().block_on(h.service.finish_login(code()));
        assert_eq!(h.logins()[1].outcome, "failed");
        assert_eq!(
            h.logins()[1].error,
            "Amazon Games returned an error. Try again."
        );
    }

    #[test]
    fn cancel_is_silent_and_drops_nile_s_values() {
        let h = harness();
        h.service.login_cancelled();
        assert!(h.logins().is_empty());
        *h.fake.tool.lock().unwrap() = true;
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        h.service.login_cancelled();
        assert_eq!(h.logins()[0].outcome, "cancelled");
        assert!(h.service.login.lock().unwrap().is_none());
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        h.service.login_redirect_failed();
        assert_eq!(h.logins()[1].outcome, "failed");
        assert_eq!(
            h.logins()[1].error,
            "Amazon Games returned an error. Try again."
        );
        assert!(h.service.login.lock().unwrap().is_none());
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        h.service.login_window_failed("no display");
        assert_eq!(h.logins()[2].outcome, "failed");
        assert!(h.service.login.lock().unwrap().is_none());
        assert!(h.notices().is_empty());
        // Each attempt asked nile for a new URL.
        assert_eq!(*h.fake.starts.lock().unwrap(), 3);
    }

    #[test]
    fn startup_reads_nile_s_library_and_an_expired_sync_flips_the_card() {
        let h = harness();
        h.connect_config();
        *h.fake.tool.lock().unwrap() = true;
        *h.fake.local.lock().unwrap() = three();
        h.service.load();
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.owned_count, 3);
        assert_eq!(status.refreshed_at, 77);
        assert_eq!(status.username, "sam.plays");

        *h.fake.account.lock().unwrap() = Some(Ok(Some(account("sam.plays"))));
        *h.fake.sync.lock().unwrap() = Some(Err("expired"));
        rt().block_on(h.service.check_login());
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Expired);
        assert_eq!(status.username, "sam.plays");
        assert_eq!(status.owned_count, 3, "the list nile saved stays");
        let card = h.notices().pop().unwrap();
        assert_eq!(card.action.unwrap().kind, "sign_in");
        assert_eq!(
            card.message,
            "Your Amazon Games login expired. Sign in again to install and update games."
        );
        assert_eq!(
            h.service.ensure_can_install().unwrap_err(),
            StoreError::LoginExpired.to_string()
        );

        // Network trouble keeps the state; connected with no saved login
        // (a cancelled "Sign in again") is the expired state.
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        *h.fake.account.lock().unwrap() = Some(Err("network"));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::Connected);
        *h.fake.account.lock().unwrap() = Some(Ok(None));
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::Expired);
        assert!(h.config().connected);
    }

    #[test]
    fn a_cancelled_sign_in_again_leaves_the_expired_state() {
        let h = harness();
        h.connect_config();
        *h.fake.tool.lock().unwrap() = true;
        *h.fake.saved.lock().unwrap() = true;
        *h.fake.local.lock().unwrap() = three();
        h.service.load();
        assert_eq!(h.service.status().connection, StoreConnection::Connected);
        // start_login deletes nile's saved login before it prints the URL.
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        assert_eq!(h.service.status().connection, StoreConnection::Expired);
        h.service.login_cancelled();
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::Expired);
        assert_eq!(status.username, "sam.plays");
        assert!(status.in_rail);
        assert!(h.config().connected);
        // A first Connect that is cancelled stays not connected.
        let h = harness();
        *h.fake.tool.lock().unwrap() = true;
        rt().block_on(h.service.prepare_sign_in()).unwrap();
        h.service.login_cancelled();
        assert_eq!(h.service.status().connection, StoreConnection::NotConnected);
    }

    #[test]
    fn nile_too_old_for_this_system_shows_as_the_tool_error() {
        let h = harness();
        *h.fake.tool.lock().unwrap() = true;
        *h.fake.start.lock().unwrap() = Some(Err("too_old"));
        let error = rt().block_on(h.service.prepare_sign_in()).unwrap_err();
        assert_eq!(error, NILE_SYSTEM_TOO_OLD);
        let status = h.service.status();
        assert_eq!(status.tool.state, ToolState::Failed);
        assert_eq!(status.tool.error, NILE_SYSTEM_TOO_OLD);
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        assert_eq!(
            h.service.ensure_can_install().unwrap_err(),
            NILE_SYSTEM_TOO_OLD
        );
    }

    #[test]
    fn startup_without_a_connection_reads_nothing() {
        let h = harness();
        *h.fake.local.lock().unwrap() = three();
        h.service.load();
        let status = h.service.status();
        assert_eq!(status.owned_count, 0);
        assert!(!status.in_rail);
        rt().block_on(h.service.check_login());
        assert_eq!(h.service.status().connection, StoreConnection::NotConnected);
    }

    #[test]
    fn disconnect_deletes_nile_and_the_login_even_after_a_failed_logout() {
        let h = harness();
        h.connect_config();
        *h.fake.tool.lock().unwrap() = true;
        *h.fake.local.lock().unwrap() = three();
        h.service.load();
        std::fs::create_dir_all(crate::store_auth::login_data_dir(h.dir.path(), "amazon")).unwrap();
        h.service.disconnect().unwrap();
        assert_eq!(*h.fake.disconnects.lock().unwrap(), 1);
        let status = h.service.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.username, "");
        assert_eq!(status.owned_count, 0);
        assert_eq!(status.tool.state, ToolState::NotInstalled);
        assert!(!h.config().connected);
        assert_eq!(h.config().username, "");
        assert!(!crate::store_auth::login_data_dir(h.dir.path(), "amazon").exists());

        // The folder was already gone: still disconnected.
        *h.fake.disconnect_error.lock().unwrap() = Some(io::ErrorKind::NotFound);
        h.service.disconnect().unwrap();
        // A folder GRID can't delete is an error in GRID's words.
        *h.fake.disconnect_error.lock().unwrap() = Some(io::ErrorKind::PermissionDenied);
        let error = h.service.disconnect().unwrap_err();
        assert!(
            error.starts_with("Couldn\u{2019}t delete Amazon Games\u{2019}s login"),
            "{error}"
        );
    }

    #[test]
    fn installs_need_a_login_and_nile_and_take_the_windows_version() {
        let h = harness();
        assert_eq!(h.service.ensure_can_install().unwrap_err(), "not signed in");
        h.service.state.lock().unwrap().connection = StoreConnection::Connected;
        assert_eq!(
            h.service.ensure_can_install().unwrap_err(),
            "nile isn't installed"
        );
        h.service.state.lock().unwrap().tool = ToolPhase::Installed;
        h.service.ensure_can_install().unwrap();

        h.service.state.lock().unwrap().games = three();
        let dir = h.dir.path().to_path_buf();
        let ctx = JobCtx {
            registry: Arc::new(Registry::open(&dir.join("r.db")).unwrap()),
            library: dir.clone(),
            default_compat_tool: String::new(),
        };
        let request = |dir: &str| StoreInstallRequest {
            key: format!("amazon:{KETTLE}"),
            platform: "windows".into(),
            language: String::new(),
            dlc_ids: Vec::new(),
            install_dir: dir.into(),
            compat_tool: "GE-Proton9-20".into(),
        };
        let folder = dir.join("Amazon Games/Kettle Drum");
        let job = h
            .service
            .install_job_for(
                KETTLE.into(),
                request(&folder.to_string_lossy()),
                ctx.clone(),
            )
            .unwrap();
        assert_eq!(job.source, StoreKind::Amazon);
        let req = h.fake.installs.lock().unwrap().pop().unwrap();
        assert_eq!(req.product_id, KETTLE);
        assert_eq!(req.title, "Kettle Drum");
        assert_eq!(req.install_dir, folder);
        assert_eq!(req.compat_tool, "GE-Proton9-20");
        let mut linux = request("");
        linux.platform = "linux".into();
        assert!(h
            .service
            .install_job_for(KETTLE.into(), linux, ctx.clone())
            .is_err());

        // No folder given: <library>/Amazon Games/<title>.
        crate::config_write::modify_config(&dir.join("config.toml"), |c| {
            c.library_path = dir.join("lib").to_string_lossy().into_owned();
            Ok(())
        })
        .unwrap();
        h.service
            .install_job_for(KETTLE.into(), request(""), ctx)
            .unwrap();
        let req = h.fake.installs.lock().unwrap().pop().unwrap();
        assert_eq!(req.install_dir, dir.join("lib/Amazon Games/Kettle Drum"));
    }

    #[test]
    fn install_options_are_the_windows_version_at_nile_s_size() {
        let h = harness();
        h.connect();
        h.service.state.lock().unwrap().games = three();
        *h.fake.size.lock().unwrap() = 2_200_000;
        let options = rt()
            .block_on(h.service.install_options_for(KETTLE))
            .unwrap();
        assert_eq!(options.versions.len(), 1);
        assert_eq!(options.versions[0].platform, "windows");
        assert_eq!(options.versions[0].download_bytes, 2_200_000);
        assert_eq!(options.versions[0].disk_bytes, 2_200_000);
        assert!(options.languages.is_empty());
        assert!(options.dlc.is_empty());
        // The size stays for the Available card.
        let library = h.service.library(&HashMap::new());
        let kettle = library
            .available
            .iter()
            .find(|g| g.store_id == KETTLE)
            .unwrap();
        assert_eq!(kettle.download_bytes, 2_200_000);
        assert_eq!(kettle.disk_bytes, 2_200_000);
        let ipc = install_options_ipc(5, "Kettle: Drum?", KETTLE, "/games/Amazon Games", "");
        assert_eq!(ipc.install_dir, "/games/Amazon Games/Kettle Drum");
        // Nothing left of the title: nile's folder is the product id.
        let ipc = install_options_ipc(5, "???", KETTLE, "/games/Amazon Games", "");
        assert_eq!(ipc.install_dir, format!("/games/Amazon Games/{KETTLE}"));
        assert_eq!(install_options_ipc(5, "K", KETTLE, "", "").install_dir, "");
    }

    fn row(id: &str, title: &str, version: &str) -> InstalledGame {
        InstalledGame {
            title: title.into(),
            platform: "Amazon Games".into(),
            store_id: id.into(),
            source: GameSource::Store(StoreKind::Amazon),
            store_meta: AmazonMeta {
                product_id: id.into(),
                version: version.into(),
                exe_user_pinned: false,
            }
            .to_json(),
            filesize_bytes: 7_000_000,
            native_game_dir: format!("/games/{title}"),
            native_compat_tool: "GE-Proton9-20".into(),
            first_release_date: "2020".into(),
            companies: "Mistral".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_library_maps_amazon_games_with_no_cloud_and_no_dlc() {
        let state = AmazonState {
            connection: StoreConnection::Connected,
            games: three(),
            ..AmazonState::default()
        };
        let rows = vec![row(FOG, "Fogline", "v-1")];
        let mut updates = HashMap::new();
        updates.insert(FOG.to_string(), AMAZON_UPDATE_MARK.to_string());
        let (installed, available) = assemble_games(&state, &rows, &updates);
        assert_eq!(installed.len(), 1);
        let fog = &installed[0];
        assert_eq!(fog.key, format!("amazon:{FOG}"));
        assert_eq!(fog.source, "amazon");
        assert_eq!(fog.origin.as_deref(), Some("nile"));
        assert_eq!(fog.platforms, vec!["windows"]);
        assert_eq!(fog.installed_platform.as_deref(), Some("windows"));
        assert_eq!(fog.version, "v-1");
        assert_eq!(fog.update_version, AMAZON_UPDATE_MARK);
        assert_eq!(fog.cloud, "none");
        assert!(fog.dlc.is_empty());
        assert_eq!(fog.not_supported, "");
        assert_eq!(fog.disk_bytes, 7_000_000);
        assert_eq!(fog.cover, "http://127.0.0.1:4400/art/Fogline.jpg");
        assert_eq!(fog.genres, vec!["Adventure"]);
        assert_eq!(fog.compat_tool, "GE-Proton9-20");
        let titles: Vec<&str> = available.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Kettle Drum", "Lowsun"]);
        assert!(available.iter().all(|g| g.origin.is_none()
            && g.cloud == "none"
            && g.platforms == ["windows"]
            && g.not_supported.is_empty()));

        // An update that finished shows no UPDATE tag until the next check.
        let mut done = state.clone();
        done.updated.insert(FOG.to_string());
        let (installed, _) = assemble_games(&done, &rows, &updates);
        assert_eq!(installed[0].update_version, "");

        // A row nile's list doesn't know keeps its own details.
        let (installed, _) = assemble_games(&AmazonState::default(), &rows, &HashMap::new());
        assert_eq!(installed[0].developer, "Mistral");
        assert_eq!(installed[0].year, "2020");
        assert_eq!(installed[0].update_version, "");
    }

    #[test]
    fn status_reads_the_pin_and_keeps_installed_games_in_the_rail() {
        let rows = vec![row(FOG, "Fogline", "v-1")];
        let status = assemble_status(
            &AmazonState::default(),
            &AmazonConfig::default(),
            &rows,
            "/lib/Amazon Games",
        );
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert!(status.in_rail, "an installed game keeps the rail group");
        assert_eq!(status.installed_count, 1);
        assert_eq!(status.installed_bytes, 7_000_000);
        assert_eq!(status.install_dir, "/lib/Amazon Games");
        assert_eq!(status.tool.license, "GPL-3.0");
        assert_eq!(status.tool.source_url, "https://github.com/imLinguin/nile");
        assert_eq!(status.tool.size_bytes, tool_size_bytes());
        assert!(!status.cloud_saves);
        let expired = AmazonState {
            connection: StoreConnection::Expired,
            ..AmazonState::default()
        };
        let config = AmazonConfig {
            connected: true,
            username: "sam.plays".into(),
            install_dir: String::new(),
        };
        let status = assemble_status(&expired, &config, &[], "");
        assert!(status.in_rail);
        assert_eq!(status.username, "sam.plays");
    }

    fn prep() -> StoreLaunchPrep {
        StoreLaunchPrep {
            executable: "/games/Fogline/bin/Fogline.exe".into(),
            env: vec![("AMAZON_GAMES_FUEL_DISPLAY_NAME".into(), "sam.plays".into())],
            ..Default::default()
        }
    }

    #[test]
    fn the_launch_hook_asks_nile_for_amazon_rows_only() {
        let h = harness();
        let fog = row(FOG, "Fogline", "v-1");
        // Not connected (after a Disconnect): the row launches as it is.
        let r = rt();
        assert!(r.block_on(h.service.prepare(&fog)).unwrap().is_none());
        assert_eq!(*h.fake.prep_calls.lock().unwrap(), 0);

        // Expired, nile there: nile needs no login for the launch prep.
        h.connect();
        h.service.state.lock().unwrap().connection = StoreConnection::Expired;
        *h.fake.tool.lock().unwrap() = true;
        h.fake.preps.lock().unwrap().push(Ok(prep()));
        let got = r.block_on(h.service.prepare(&fog)).unwrap().unwrap();
        assert_eq!(got.env.len(), 1);
        // Its Debug never prints the display name.
        assert!(!format!("{got:?}").contains("sam.plays"));

        // A failure takes nile's words, no retry.
        h.fake.preps.lock().unwrap().push(Err("network"));
        let error = r.block_on(h.service.prepare(&fog)).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Couldn\u{2019}t start Fogline through nile: "),
            "{error}"
        );
        assert_eq!(*h.fake.prep_calls.lock().unwrap(), 2);

        // No nile: launches as it is.
        *h.fake.tool.lock().unwrap() = false;
        assert!(r.block_on(h.service.prepare(&fog)).unwrap().is_none());

        let gog = InstalledGame {
            source: GameSource::Store(StoreKind::Gog),
            ..Default::default()
        };
        assert!(r.block_on(h.service.prepare(&gog)).unwrap().is_none());
        assert!(h.service.clone().launch_hook().is_some());
    }

    #[test]
    fn game_settings_take_a_compat_tool_and_have_no_cloud() {
        let h = harness();
        let row = InstalledGame {
            native_executable_path: "/g/F/bin/Fogline.exe".into(),
            store_meta: AmazonMeta::default().to_json(),
            ..Default::default()
        };
        assert!(!h.service.game_settings(&row).cloud_saves);
        assert!(!StoreBackend::cloud_enabled(h.service.as_ref(), &row));
        let settings = StoreGameSettings {
            install_dir: String::new(),
            executable: "/g/F/bin/Other.exe".into(),
            exe_user_pinned: false,
            launch_parameters: " -windowed ".into(),
            compat_tool: "GE-Proton9-20".into(),
            wineprefix: String::new(),
            cloud_saves: true,
        };
        let out = StoreBackend::apply_game_settings(h.service.as_ref(), row, &settings);
        assert_eq!(out.native_compat_tool, "GE-Proton9-20");
        assert_eq!(out.native_launch_parameters, "-windowed");
        assert_eq!(out.native_executable_path, "/g/F/bin/Other.exe");
        assert_eq!(out.native_launch_source, "user");
        assert!(AmazonMeta::from_json(&out.store_meta).exe_user_pinned);
        assert!(StoreBackend::windows_version(h.service.as_ref(), &out));
        let cloud = rt()
            .block_on(StoreBackend::cloud_state(h.service.as_ref(), &out))
            .unwrap();
        assert_eq!(cloud.state, "none");
    }

    fn entry(id: u64, kind: &'static str, key: &str, completed: bool) -> JobRow {
        JobRow {
            id,
            store: true,
            kind,
            key: key.into(),
            title: "Fogline".into(),
            completed,
            failed: !completed,
            error: if completed {
                String::new()
            } else {
                "Not enough space".into()
            },
        }
    }

    #[test]
    fn a_finished_amazon_job_gets_one_card() {
        let h = harness();
        let key = format!("amazon:{FOG}");
        h.service.observe_rows(vec![
            entry(1, "store_install", &key, true),
            entry(2, "store_update", &key, true),
            entry(3, "store_repair", &key, true),
            entry(4, "store_install", &key, false),
            entry(5, "store_install", "epic:Starlight", true),
        ]);
        assert_eq!(
            h.messages(),
            vec![
                "Fogline installed".to_string(),
                "Fogline updated".to_string(),
                "Verified Fogline\u{2019}s files".to_string(),
                "Couldn\u{2019}t install Fogline".to_string(),
            ]
        );
        assert_eq!(h.notices()[3].detail, "Not enough space");
        assert_eq!(h.notices()[0].action.as_ref().unwrap().kind, "play");
        // Seen once: the same snapshot again adds nothing.
        h.service
            .observe_rows(vec![entry(1, "store_install", &key, true)]);
        assert_eq!(h.notices().len(), 4);
        // The finished update hides its UPDATE tag until the next check.
        assert!(h.service.state.lock().unwrap().updated.contains(FOG));
        *h.fake.updates.lock().unwrap() = Some(Ok(vec![AmazonUpdate {
            product_id: FOG.into(),
        }]));
        let found = rt().block_on(h.service.find_updates()).unwrap();
        assert_eq!(found.len(), 1);
        assert!(h.service.state.lock().unwrap().updated.is_empty());
    }

    #[test]
    fn notices_use_the_cards_words_with_no_cloud_and_no_version() {
        assert_eq!(
            notice::update_available("Fogline").message,
            "Fogline: an update is available"
        );
        assert_eq!(
            notice::update_available("Fogline").action.unwrap().kind,
            "show_updates"
        );
        assert_eq!(
            notice::games_found(1, 0).message,
            "Amazon Games: 1 game found \u{b7} 0 to install"
        );
        assert_eq!(notice::tool_installed().message, "nile installed");
        assert_eq!(
            notice::tool_failed("no build").message,
            "Couldn\u{2019}t install nile: no build"
        );
        assert_eq!(
            notice::tool_installing(11_113_264).message,
            "Installing nile (11 MB) for Amazon Games\u{2026}"
        );
        assert!(notice::finished("store_dlc", "F", "amazon:x").is_none());
        assert!(notice::failed("store_dlc", "F", "e").is_none());
        assert_eq!(
            notice::installed("F", "amazon:x").source.as_deref(),
            Some("amazon")
        );
    }

    #[test]
    fn refresh_counts_what_is_left_to_install() {
        let h = harness();
        h.connect();
        let registry = Arc::new(Registry::open(&h.dir.path().join("r.db")).unwrap());
        registry.upsert_store(&row(FOG, "Fogline", "v-1")).unwrap();
        h.service.shared.set_registry(registry);
        *h.fake.sync.lock().unwrap() = Some(Ok(three()));
        rt().block_on(h.service.refresh_owned(true));
        assert_eq!(
            h.messages(),
            vec!["Amazon Games: 3 games found \u{b7} 2 to install".to_string()]
        );
        // A network failure keeps the list and shows its error.
        *h.fake.sync.lock().unwrap() = Some(Err("network"));
        rt().block_on(h.service.refresh_owned(true));
        let status = h.service.status();
        assert_eq!(status.owned, OwnedState::Error);
        assert_eq!(status.owned_count, 3);
        assert_eq!(status.connection, StoreConnection::Connected);
        assert!(status.in_rail);
        assert_eq!(status.installed_count, 1);
    }
}
