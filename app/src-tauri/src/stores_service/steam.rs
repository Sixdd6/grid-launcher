//! Steam in the app: the Steam [`StoreBackend`]
//! (`docs/stores-phase4-plan.md` package C). Steam is the first store with
//! no tool, no login window and no GRID-run job, so most of the trait
//! answers "Steam does that itself":
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
//! What this backend owns around grid-core's `stores::steam::SteamService`:
//!
//! - **the key.** It comes from the [`SteamKeyStore`] (the OS keyring) when
//!   needed, is kept in memory as a [`SecretString`] after the first load,
//!   and is dropped on Remove key and on Disconnect. Its state on the card
//!   ([`SteamKeyState`]) is the last Web API answer;
//! - **the owned list** (`IPlayerService/GetOwnedGames`), filtered to games
//!   (`filter::is_game`), and the keyless `GetItems` metadata. This file is
//!   the one owner of `<data_dir>/stores/cache/steam-owned.json` and
//!   `steam-apps.json`: written by a refresh, read at startup, deleted on
//!   Disconnect (the owned list also on Remove key);
//! - **installed rows.** Steam games are registry rows (source `steam`),
//!   reconciled from Steam's own manifests by every scan: at startup, after
//!   Save, on Refresh and Check again, every 15 s in the background (only
//!   when a library's `steamapps/` changed) and after each install watch.
//!   Rows are only ever rows: no GRID path removes a Steam game's files;
//! - **install watches.** `steam_open(install)` admits a Downloads row the
//!   queue shows but does not drive (`admit_store_external`) and polls the
//!   game's manifest every 2 s; a manifest Steam is installing that GRID did
//!   not start (Steam itself, or a restart) gets a row and a watch too;
//! - the background `store-notice` cards ([`notice`]).
//!
//! The store calls go through [`SteamApi`], a seam the unit tests fake.
//!
//! Secrets: the Web API key lives only in the OS keyring and in memory as a
//! [`SecretString`]. It is never read here (only grid-core's
//! `stores/steam/api.rs` exposes it), and never in a status, an event, an
//! error, a log line, a cache file or the config. The Steam login name is
//! never read (grid-core's `LocalAccount` has no field for it). Nothing the
//! Steam client prints is read.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use grid_core::config::{Config, SteamConfig};
use grid_core::launch::{AliveCheck, GameSession, LaunchError, LaunchService};
use grid_core::library::queue::{
    DownloadsSnapshot, ExternalOutcome, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink,
};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::{InstallService, LibraryError};
use grid_core::secrets::SteamKeyStore;
use grid_core::stores::steam::filter;
use grid_core::stores::steam::meta::SteamMeta;
use grid_core::stores::steam::row::{install_path, steam_row};
use grid_core::stores::steam::{
    AppInfo, AppPhase, InstallWatch, LocalAccount, ManifestRead, ManifestState, OwnedGame,
    OwnedGames, SteamApiError, SteamClient, SteamEndpoints, SteamId, SteamLink, SteamScan,
    SteamService, WatchEvent,
};
use grid_core::stores::{GameKey, StoreError, StoreKind, SyncMode};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use super::{BoxFut, JobCtx, StoreBackend, StoreEvent, StoreShared};
use crate::commands::stores::{
    OwnedState, SteamAccount, SteamCard, SteamKeyState, SteamOpenAction, SteamTestResult,
    StoreCloudState, StoreConnection, StoreGame, StoreGameSettings, StoreInstallOptions,
    StoreInstallRequest, StoreLibrary, StoreNotice, StoreSaveConflict, StoreStatus, ToolState,
    ToolStatus,
};
use crate::store_auth::LoginSpec;

/// The store's id.
const SOURCE: &str = "steam";
const STORE_NAME: &str = "Steam";

/// Every login path's answer: Steam has no login window.
pub const CONNECTS_IN_SETTINGS: &str = "Steam connects in Settings \u{203a} Stores.";

/// Every install, update, repair, DLC and uninstall path's answer.
pub const STEAM_DOES_IT: &str = "Steam installs, updates and uninstalls its games itself.";

/// When the Steam client isn't found.
pub const STEAM_NOT_INSTALLED: &str =
    "Steam isn\u{2019}t installed. Install Steam, then try again.";

/// A Test with no key given and none saved (Choices taken 1).
pub const PASTE_KEY_FIRST: &str = "Paste your key first.";

/// A finished install's Downloads line.
pub const INSTALLED_BY_STEAM: &str = "Installed by Steam";

/// The owned-list cache (`<data_dir>/stores/cache/`).
pub const OWNED_CACHE: &str = "steam-owned.json";
/// The `GetItems` metadata cache (`<data_dir>/stores/cache/`).
pub const APPS_CACHE: &str = "steam-apps.json";

/// The Web API's per-request deadline.
const API_TIMEOUT: Duration = Duration::from_secs(20);
/// How often a live install watch reads its manifest.
const WATCH_POLL: Duration = grid_core::stores::steam::watch::STEAM_INSTALL_POLL;
/// How long an install watch waits for Steam to write the manifest (the
/// user may leave Steam's install dialog open a while, or close it).
pub use grid_core::stores::steam::watch::STEAM_INSTALL_START_TIMEOUT;
/// How long an uninstall watch waits for the manifest to go.
const UNINSTALL_WATCH_FOR: Duration = Duration::from_secs(10 * 60);
/// The background scan's period.
const SCAN_PERIOD: Duration = Duration::from_secs(15);

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

// --- the seam -------------------------------------------------------------------

/// What the app asks of Steam. [`RealSteam`] forwards to grid-core's
/// `SteamService`; the unit tests fake it.
pub trait SteamApi: Send + Sync {
    fn endpoints(&self) -> SteamEndpoints;
    fn find_client(&self) -> Option<SteamClient>;
    fn local_accounts(&self, client: &SteamClient) -> Vec<LocalAccount>;
    /// Every manifest in every library, the unreadable ones and the
    /// libraries that couldn't be read.
    fn scan(&self, client: &SteamClient) -> SteamScan;
    fn manifest(&self, client: &SteamClient, appid: u32) -> Option<ManifestState>;
    /// The manifest, telling a missing file from one Steam is writing (what
    /// [`InstallWatch::step`] takes).
    fn manifest_read(&self, client: &SteamClient, appid: u32) -> ManifestRead {
        match self.manifest(client, appid) {
            Some(manifest) => ManifestRead::Found(manifest),
            None => ManifestRead::Missing,
        }
    }
    fn open(&self, client: &SteamClient, link: SteamLink) -> io::Result<()>;
    fn alive_check(&self, client: &SteamClient, appid: u32) -> AliveCheck;
    fn owned<'a>(
        &'a self,
        key: &'a SecretString,
        id: SteamId,
    ) -> BoxFut<'a, Result<OwnedGames, SteamApiError>>;
    fn persona<'a>(
        &'a self,
        key: &'a SecretString,
        id: SteamId,
    ) -> BoxFut<'a, Result<String, SteamApiError>>;
    fn app_info<'a>(&'a self, ids: &'a [u32]) -> BoxFut<'a, Vec<AppInfo>>;
}

/// The real Steam: grid-core's [`SteamService`].
pub struct RealSteam(pub SteamService);

impl SteamApi for RealSteam {
    fn endpoints(&self) -> SteamEndpoints {
        self.0.endpoints().clone()
    }
    fn find_client(&self) -> Option<SteamClient> {
        self.0.find_client()
    }
    fn local_accounts(&self, client: &SteamClient) -> Vec<LocalAccount> {
        self.0.local_accounts(client)
    }
    fn scan(&self, client: &SteamClient) -> SteamScan {
        self.0.scan_report(client)
    }
    fn manifest(&self, client: &SteamClient, appid: u32) -> Option<ManifestState> {
        self.0.manifest(client, appid)
    }
    fn manifest_read(&self, client: &SteamClient, appid: u32) -> ManifestRead {
        self.0.manifest_read(client, appid)
    }
    fn open(&self, client: &SteamClient, link: SteamLink) -> io::Result<()> {
        self.0.open(client, link)
    }
    fn alive_check(&self, client: &SteamClient, appid: u32) -> AliveCheck {
        self.0.alive_check(client, appid)
    }
    fn owned<'a>(
        &'a self,
        key: &'a SecretString,
        id: SteamId,
    ) -> BoxFut<'a, Result<OwnedGames, SteamApiError>> {
        Box::pin(self.0.owned(key, id))
    }
    fn persona<'a>(
        &'a self,
        key: &'a SecretString,
        id: SteamId,
    ) -> BoxFut<'a, Result<String, SteamApiError>> {
        Box::pin(self.0.persona(key, id))
    }
    fn app_info<'a>(&'a self, ids: &'a [u32]) -> BoxFut<'a, Vec<AppInfo>> {
        Box::pin(self.0.app_info(ids))
    }
}

// --- state ----------------------------------------------------------------------

/// A live install watch.
struct Watch {
    stop: Arc<AtomicBool>,
    /// The Downloads row, `None` when no install service is set (tests).
    entry: Option<u64>,
}

/// What the backend holds in memory. Never `Debug` or `Clone`: it holds
/// the key.
struct SteamState {
    /// The key, after the first load from the keyring.
    key: Option<SecretString>,
    /// Whether the keyring was read (a `None` key is then "no key").
    key_loaded: bool,
    /// The last Web API answer about the key; `None` before the first.
    answer: Option<SteamKeyState>,
    owned: OwnedState,
    owned_error: String,
    /// `GetOwnedGames`, unfiltered (the filter needs the store types).
    games: Vec<OwnedGame>,
    refreshed_at: i64,
    /// `GetItems` answers by app id.
    apps: HashMap<u32, AppInfo>,
    /// App ids `GetItems` was asked about in this run (an id with no store
    /// page gets no answer; it is not asked again).
    asked: HashSet<u32>,
    client: Option<SteamClient>,
    accounts: Vec<LocalAccount>,
    /// Games GRID saw installed in this run: their row's removal raises the
    /// uninstalled card.
    seen_installed: HashSet<u32>,
    /// Installs GRID's own Install in Steam started and saw finish (their
    /// rows are not "outside GRID").
    grid_installs: HashSet<u32>,
    watches: HashMap<u32, Watch>,
    /// The mtimes the background scan compares (`None`: scan next time).
    fingerprint: Option<Vec<(PathBuf, Option<SystemTime>)>>,
    /// The library folders the last scan found.
    libraries: Vec<PathBuf>,
}

impl Default for SteamState {
    fn default() -> Self {
        SteamState {
            key: None,
            key_loaded: false,
            answer: None,
            owned: OwnedState::Idle,
            owned_error: String::new(),
            games: Vec::new(),
            refreshed_at: 0,
            apps: HashMap::new(),
            asked: HashSet::new(),
            client: None,
            accounts: Vec::new(),
            seen_installed: HashSet::new(),
            grid_installs: HashSet::new(),
            watches: HashMap::new(),
            fingerprint: None,
            libraries: Vec::new(),
        }
    }
}

impl SteamState {
    /// The key's state on the card.
    fn key_state(&self) -> SteamKeyState {
        if self.key.is_none() {
            SteamKeyState::None
        } else {
            self.answer.unwrap_or(SteamKeyState::Ok)
        }
    }
}

/// The Steam backend.
pub struct SteamStore {
    api: Arc<dyn SteamApi>,
    endpoints: SteamEndpoints,
    data_dir: PathBuf,
    config_path: PathBuf,
    /// The keyring's Steam slot.
    keys: Arc<dyn SteamKeyStore>,
    /// The emitter and registry every store shares.
    shared: Arc<StoreShared>,
    state: Mutex<SteamState>,
    /// The Downloads queue, for the rows of installs Steam drives.
    install: OnceLock<Arc<InstallService>>,
    /// One scan at a time.
    scan_lock: tokio::sync::Mutex<()>,
    /// One refresh at a time.
    refresh_lock: tokio::sync::Mutex<()>,
    background: AtomicBool,
    /// How long an install watch waits for its manifest (tests shorten it).
    start_timeout: Mutex<Duration>,
    /// The install watches' poll period (tests shorten it).
    poll: Mutex<Duration>,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl SteamStore {
    pub fn new(
        api: Arc<dyn SteamApi>,
        config_path: PathBuf,
        keys: Arc<dyn SteamKeyStore>,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        let endpoints = api.endpoints();
        let data_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Arc::new(SteamStore {
            api,
            endpoints,
            data_dir,
            config_path,
            keys,
            shared,
            state: Mutex::new(SteamState::default()),
            install: OnceLock::new(),
            scan_lock: tokio::sync::Mutex::new(()),
            refresh_lock: tokio::sync::Mutex::new(()),
            background: AtomicBool::new(false),
            start_timeout: Mutex::new(STEAM_INSTALL_START_TIMEOUT),
            poll: Mutex::new(WATCH_POLL),
        })
    }

    /// The real Steam backend for this run. `keys` is the OS keyring's
    /// Steam slot (`KeyringStore` in `lib.rs`; `MemoryStore` in tests).
    pub fn real(
        config_path: PathBuf,
        keys: Arc<dyn SteamKeyStore>,
        shared: Arc<StoreShared>,
    ) -> Arc<Self> {
        SteamStore::new(
            Arc::new(RealSteam(steam_service())),
            config_path,
            keys,
            shared,
        )
    }

    /// The Downloads queue the install rows go to (`lib.rs`).
    pub fn set_install(&self, install: Arc<InstallService>) {
        let _ = self.install.set(install);
    }

    fn registry(&self) -> Option<Arc<Registry>> {
        self.shared.registry()
    }

    fn steam_config(&self) -> SteamConfig {
        Config::load(&self.config_path)
            .map(|config| config.stores.steam)
            .unwrap_or_default()
    }

    fn write_steam_config(&self, change: impl FnOnce(&mut SteamConfig)) -> Result<(), String> {
        crate::config_write::modify_config(&self.config_path, |config| {
            change(&mut config.stores.steam);
            Ok(())
        })
    }

    fn connected(&self) -> bool {
        self.steam_config().connected
    }

    /// Emits `stores-changed` with every store's status.
    pub fn changed(&self) {
        self.shared.changed();
    }

    fn emit(&self, event: StoreEvent) {
        self.shared.emit(event);
    }

    fn notify(&self, notice: StoreNotice) {
        self.emit(StoreEvent::Notice(notice));
    }

    pub fn connection(&self) -> StoreConnection {
        if self.connected() {
            StoreConnection::Connected
        } else {
            StoreConnection::NotConnected
        }
    }

    fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("stores").join("cache")
    }

    // --- startup and background ---------------------------------------------

    /// Startup: the client and its accounts, and when connected the key
    /// (from the keyring) and both caches. The first scan and refresh run
    /// next ([`Self::check_login`]).
    pub fn load(&self) {
        let client = self.api.find_client();
        let accounts = client
            .as_ref()
            .map(|client| self.api.local_accounts(client))
            .unwrap_or_default();
        let connected = self.connected();
        let (games, refreshed_at) = if connected {
            read_owned_cache(&self.cache_dir())
        } else {
            (Vec::new(), 0)
        };
        let apps = read_apps_cache(&self.cache_dir());
        let key = if connected {
            match self.keys.load() {
                Ok(key) => Some(key),
                Err(e) => {
                    tracing::warn!("stores: Steam key not read from the OS keyring: {e}");
                    None
                }
            }
        } else {
            None
        };
        let mut state = self.state.lock().unwrap();
        state.client = client;
        state.accounts = accounts;
        state.apps = apps;
        if let Some(key) = key {
            state.key = key;
            state.key_loaded = true;
        }
        if connected && !games.is_empty() {
            state.games = games;
            state.refreshed_at = refreshed_at;
            state.owned = OwnedState::Ready;
        }
    }

    /// Startup, after [`Self::load`]: a scan, then (connected) the owned
    /// list.
    pub async fn check_login(self: &Arc<Self>) {
        self.scan_now().await;
        if self.connected() {
            self.refresh_owned(false).await;
        }
    }

    /// Starts the background scan (every 15 s; only libraries whose
    /// `steamapps/` changed). Once per run.
    pub fn spawn_background(self: &Arc<Self>) {
        if self.background.swap(true, Ordering::SeqCst) {
            return;
        }
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(SCAN_PERIOD).await;
                this.background_tick().await;
            }
        });
    }

    /// One background pass: the client (Steam may have been installed or
    /// removed), then a scan when anything in the libraries changed.
    async fn background_tick(self: &Arc<Self>) {
        let api = self.api.clone();
        let client = tokio::task::spawn_blocking(move || api.find_client())
            .await
            .ok()
            .flatten();
        let (client_changed, stale) = {
            let state = self.state.lock().unwrap();
            let client_changed = state.client != client;
            let stale = match (&client, &state.fingerprint) {
                (Some(client), Some(old)) => &fingerprint(&client.root, &state.libraries) != old,
                _ => true,
            };
            (client_changed, stale)
        };
        if client_changed || stale {
            self.scan_now().await;
        }
    }

    // --- status ----------------------------------------------------------------

    fn installed_rows(&self) -> Vec<InstalledGame> {
        self.registry()
            .and_then(|registry| registry.store_rows(StoreKind::Steam).ok())
            .unwrap_or_default()
    }

    /// Steam's status.
    pub fn status(&self) -> StoreStatus {
        let config = self.steam_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap();
        assemble_status(&config, &view(&state), &rows)
    }

    /// Whether `url` is store art the UI may load (Steam's image hosts, the
    /// e2e mock's origin).
    pub fn is_art_url(&self, url: &str) -> bool {
        grid_core::stores::art::is_store_art_url(url, &self.endpoints.art_origins)
    }

    /// Steam's Installed and Available lists.
    pub fn library(&self) -> StoreLibrary {
        let config = self.steam_config();
        let rows = self.installed_rows();
        let state = self.state.lock().unwrap();
        let view = view(&state);
        let status = assemble_status(&config, &view, &rows);
        let (installed, available) =
            assemble_games(&view, &rows, config.connected, &self.endpoints);
        StoreLibrary {
            status,
            installed,
            available,
        }
    }

    // --- the key ------------------------------------------------------------------

    /// The saved key: in memory after the first read of the keyring.
    async fn saved_key(&self) -> Result<Option<SecretString>, String> {
        {
            let state = self.state.lock().unwrap();
            if state.key_loaded {
                return Ok(state.key.clone());
            }
        }
        let keys = self.keys.clone();
        let key = tokio::task::spawn_blocking(move || keys.load())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("Couldn\u{2019}t read your key from the OS keyring: {e}"))?;
        let mut state = self.state.lock().unwrap();
        state.key = key.clone();
        state.key_loaded = true;
        Ok(key)
    }

    // --- metadata -----------------------------------------------------------------

    /// Asks `GetItems` about the ids it doesn't know yet (rules 1–2 first:
    /// a denylisted tool needs no store type) and keeps the answers in
    /// `steam-apps.json`. Failures leave the ids unknown.
    async fn learn_apps(&self, ids: impl IntoIterator<Item = (u32, String, String)>) {
        let unknown: Vec<u32> = {
            let mut state = self.state.lock().unwrap();
            let mut unknown = Vec::new();
            for (appid, name, dir) in ids {
                if state.apps.contains_key(&appid)
                    || state.asked.contains(&appid)
                    || !filter::is_game(appid, &name, &dir, None)
                    || unknown.contains(&appid)
                {
                    continue;
                }
                unknown.push(appid);
            }
            for appid in &unknown {
                state.asked.insert(*appid);
            }
            unknown
        };
        if unknown.is_empty() {
            return;
        }
        let answers = self.api.app_info(&unknown).await;
        if answers.is_empty() {
            // Offline: ask again next time.
            let mut state = self.state.lock().unwrap();
            for appid in &unknown {
                state.asked.remove(appid);
            }
            return;
        }
        let apps = {
            let mut state = self.state.lock().unwrap();
            for info in answers {
                state.apps.insert(info.appid, info);
            }
            state.apps.clone()
        };
        if let Err(e) = write_apps_cache(&self.cache_dir(), &apps) {
            tracing::warn!("stores: Steam metadata cache not written: {e}");
        }
    }

    // --- the Steam commands --------------------------------------------------------

    /// `steam_test`: the SteamID64's form (no request), then one
    /// `GetOwnedGames` call with `key`, or with the saved key when `None`.
    /// The count is the owned games after the games-only filter (the
    /// keyless `GetItems` names the store types).
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
            None => self
                .saved_key()
                .await?
                .ok_or_else(|| PASTE_KEY_FIRST.to_string())?,
        };
        let owned = match self.api.owned(&key, id).await {
            Ok(owned) => owned,
            Err(e) => return Ok(test_result(Err(e))),
        };
        self.learn_apps(
            owned
                .games
                .iter()
                .map(|g| (g.appid, g.name.clone(), String::new())),
        )
        .await;
        let count = {
            let state = self.state.lock().unwrap();
            owned_games(&owned.games, &state.apps).count() as u32
        };
        Ok(test_result(Ok(count)))
    }

    /// `steam_save`: the test; on `ok` or `private`, the persona (from
    /// `loginusers.vdf`, else one `GetPlayerSummaries` call), the key into
    /// the keyring (when given), `[stores.steam]`, the owned list, a scan,
    /// then the connected card. Anything else saves nothing.
    pub async fn save(
        self: &Arc<Self>,
        steam_id: &str,
        key: Option<SecretString>,
    ) -> Result<SteamTestResult, String> {
        let Ok(id) = steam_id.parse::<SteamId>() else {
            return Ok(SteamTestResult::BadSteamId);
        };
        let given = key.is_some();
        let key = match key {
            Some(key) => key,
            None => self
                .saved_key()
                .await?
                .ok_or_else(|| PASTE_KEY_FIRST.to_string())?,
        };
        let answer = self.api.owned(&key, id).await;
        let games = match answer {
            Ok(owned) => Some(owned.games),
            Err(SteamApiError::Private) => None,
            Err(e) => return Ok(test_result(Err(e))),
        };
        // The persona: a local account's, else Steam's.
        let local = {
            let state = self.state.lock().unwrap();
            state
                .accounts
                .iter()
                .find(|account| account.steam_id == id)
                .map(|account| account.persona.clone())
        };
        let persona = match local {
            Some(persona) => persona,
            None => match self.api.persona(&key, id).await {
                Ok(persona) => persona,
                Err(e) => return Ok(test_result(Err(e))),
            },
        };
        if given {
            let keys = self.keys.clone();
            let saved = key.clone();
            tokio::task::spawn_blocking(move || keys.save(&saved))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| format!("Couldn\u{2019}t save your key in the OS keyring: {e}"))?;
        }
        let steam_id = id.to_string();
        let persona_saved = persona.clone();
        self.write_steam_config(move |steam| {
            steam.connected = true;
            steam.steam_id = steam_id;
            steam.persona = persona_saved;
        })?;
        {
            let mut state = self.state.lock().unwrap();
            state.key = Some(key);
            state.key_loaded = true;
        }
        let result = match games {
            Some(games) => {
                self.learn_apps(
                    games
                        .iter()
                        .map(|g| (g.appid, g.name.clone(), String::new())),
                )
                .await;
                let count = self.take_owned(games);
                SteamTestResult::Ok {
                    owned_count: count as u32,
                }
            }
            None => {
                self.take_private();
                SteamTestResult::Private
            }
        };
        self.scan_now().await;
        self.notify(match result {
            SteamTestResult::Ok { owned_count } => notice::connected(owned_count as usize),
            _ => notice::connected_private(),
        });
        self.changed();
        Ok(result)
    }

    /// Keeps a `GetOwnedGames` answer (memory and `steam-owned.json`);
    /// answers the owned count after the filter.
    fn take_owned(&self, games: Vec<OwnedGame>) -> usize {
        let at = now();
        let count = {
            let mut state = self.state.lock().unwrap();
            state.games = games.clone();
            state.refreshed_at = at;
            state.owned = OwnedState::Ready;
            state.owned_error.clear();
            state.answer = Some(SteamKeyState::Ok);
            owned_games(&state.games, &state.apps).count()
        };
        if let Err(e) = write_owned_cache(&self.cache_dir(), &games, at) {
            tracing::warn!("stores: Steam owned-list cache not written: {e}");
        }
        count
    }

    /// The key works but the game details are private: no owned list.
    fn take_private(&self) {
        {
            let mut state = self.state.lock().unwrap();
            state.games.clear();
            state.refreshed_at = now();
            state.owned = OwnedState::Ready;
            state.owned_error.clear();
            state.answer = Some(SteamKeyState::Private);
        }
        remove_file(&self.cache_dir().join(OWNED_CACHE));
    }

    /// `steam_remove_key`: clears the keyring slot and the in-memory key;
    /// Steam stays connected with installed games only. The owned list
    /// goes too (it needs the key). The frontend raises its card.
    pub fn remove_key(&self) -> Result<(), String> {
        self.keys.clear().map_err(|e| {
            format!("Couldn\u{2019}t delete your Web API key from the OS keyring: {e}")
        })?;
        {
            let mut state = self.state.lock().unwrap();
            state.key = None;
            state.key_loaded = true;
            state.answer = None;
            state.games.clear();
            state.refreshed_at = 0;
            state.owned = OwnedState::Idle;
            state.owned_error.clear();
        }
        remove_file(&self.cache_dir().join(OWNED_CACHE));
        self.changed();
        Ok(())
    }

    /// `steam_recheck_client`: finds the client again and scans. The
    /// frontend reads the card and raises its own card.
    pub async fn recheck_client(self: &Arc<Self>) -> Result<(), String> {
        self.scan_now().await;
        Ok(())
    }

    /// `steam_open`: hands the client an install, uninstall or validate
    /// link. Install also admits the Downloads row and starts the watch;
    /// uninstall watches for the manifest to go. The frontend raises the
    /// design's toast.
    pub fn open(self: &Arc<Self>, store_id: &str, action: SteamOpenAction) -> Result<(), String> {
        let appid: u32 = store_id
            .trim()
            .parse()
            .map_err(|_| "That isn\u{2019}t a Steam game.".to_string())?;
        let client = self
            .api
            .find_client()
            .ok_or_else(|| STEAM_NOT_INSTALLED.to_string())?;
        let link = match action {
            SteamOpenAction::Install => SteamLink::Install(appid),
            SteamOpenAction::Uninstall => SteamLink::Uninstall(appid),
            SteamOpenAction::Validate => SteamLink::Validate(appid),
        };
        self.api
            .open(&client, link)
            .map_err(|e| format!("Couldn\u{2019}t open Steam: {e}"))?;
        match action {
            SteamOpenAction::Install => {
                let title = self.title_of(appid, None);
                self.start_watch(client, appid, title, true);
            }
            SteamOpenAction::Uninstall => self.watch_uninstall(client, appid),
            SteamOpenAction::Validate => {}
        }
        Ok(())
    }

    /// A game's title: its row's, the store's, the owned list's, the
    /// manifest's, else the app id.
    fn title_of(&self, appid: u32, manifest: Option<&ManifestState>) -> String {
        let store_id = appid.to_string();
        if let Some(row) = self
            .registry()
            .and_then(|registry| registry.find_store(StoreKind::Steam, &store_id).ok())
            .flatten()
        {
            return row.title;
        }
        let state = self.state.lock().unwrap();
        state
            .apps
            .get(&appid)
            .map(|info| info.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                state
                    .games
                    .iter()
                    .find(|g| g.appid == appid)
                    .map(|g| g.name.trim().to_string())
                    .filter(|name| !name.is_empty())
            })
            .or_else(|| {
                manifest
                    .map(|m| m.name.trim().to_string())
                    .filter(|name| !name.is_empty())
            })
            .unwrap_or(store_id)
    }

    // --- install watches ------------------------------------------------------------

    /// Starts watching `appid`'s install (the Downloads row, then a 2 s poll
    /// of its manifest). `by_grid`: GRID's Install in Steam started it. One
    /// watch per game.
    fn start_watch(
        self: &Arc<Self>,
        client: SteamClient,
        appid: u32,
        title: String,
        by_grid: bool,
    ) {
        let stop = Arc::new(AtomicBool::new(false));
        if self.state.lock().unwrap().watches.contains_key(&appid) {
            return;
        }
        // Outside the state lock: admitting notifies the Downloads
        // observers, which read every store's status.
        let entry = self.install.get().and_then(|install| {
            install.admit_store_external(StoreKind::Steam, &appid.to_string(), &title)
        });
        {
            let mut state = self.state.lock().unwrap();
            if state.watches.contains_key(&appid) {
                return;
            }
            state.watches.insert(
                appid,
                Watch {
                    stop: stop.clone(),
                    entry,
                },
            );
        }
        let this = self.clone();
        let start_timeout = *self.start_timeout.lock().unwrap();
        let poll = *self.poll.lock().unwrap();
        tauri::async_runtime::spawn(async move {
            let started = Instant::now();
            // GRID's Install in Steam waits for Steam's dialog; an install
            // Steam started by itself was already seen under way.
            let watch = if by_grid {
                InstallWatch::default()
            } else {
                InstallWatch::resumed()
            };
            let mut watch = watch.with_start_timeout(start_timeout);
            loop {
                tokio::time::sleep(poll).await;
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let api = this.api.clone();
                let probe = client.clone();
                let read = tokio::task::spawn_blocking(move || api.manifest_read(&probe, appid))
                    .await
                    .unwrap_or(ManifestRead::Unreadable);
                let step = watch.step(&read, started.elapsed());
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                match step {
                    WatchEvent::Waiting => {}
                    WatchEvent::Progress(done, total) => this.watch_progress(appid, done, total),
                    WatchEvent::Installed => {
                        this.watch_installed(appid, &title, by_grid).await;
                        return;
                    }
                    WatchEvent::Cancelled => {
                        this.watch_cancelled(appid);
                        return;
                    }
                }
            }
        });
    }

    fn watch_entry(&self, appid: u32) -> Option<u64> {
        self.state
            .lock()
            .unwrap()
            .watches
            .get(&appid)
            .and_then(|watch| watch.entry)
    }

    fn watch_progress(&self, appid: u32, done: u64, total: u64) {
        if let (Some(install), Some(entry)) = (self.install.get(), self.watch_entry(appid)) {
            install.store_external_progress(entry, done, total);
        }
    }

    async fn watch_installed(self: &Arc<Self>, appid: u32, title: &str, by_grid: bool) {
        let entry = {
            let mut state = self.state.lock().unwrap();
            if by_grid {
                state.grid_installs.insert(appid);
            }
            state.fingerprint = None;
            state.watches.remove(&appid).and_then(|watch| watch.entry)
        };
        if let (Some(install), Some(entry)) = (self.install.get(), entry) {
            install.finish_store_external(
                entry,
                ExternalOutcome::Completed {
                    detail: INSTALLED_BY_STEAM.to_string(),
                },
            );
        }
        self.scan_now().await;
        let key = GameKey::store(StoreKind::Steam, appid.to_string()).to_string();
        let title = {
            let known = self.title_of(appid, None);
            if known == appid.to_string() && !title.trim().is_empty() {
                title.to_string()
            } else {
                known
            }
        };
        self.notify(notice::installed(&title, &key));
    }

    fn watch_cancelled(&self, appid: u32) {
        let entry = {
            let mut state = self.state.lock().unwrap();
            state.watches.remove(&appid).and_then(|watch| watch.entry)
        };
        if let (Some(install), Some(entry)) = (self.install.get(), entry) {
            install.finish_store_external(entry, ExternalOutcome::Cancelled);
        }
    }

    /// After Uninstall in Steam: polls the manifest every 2 s (for up to 10
    /// minutes) and scans as soon as Steam removed it, so the game leaves
    /// the Library without waiting for the background scan.
    fn watch_uninstall(self: &Arc<Self>, client: SteamClient, appid: u32) {
        let this = self.clone();
        let poll = *self.poll.lock().unwrap();
        tauri::async_runtime::spawn(async move {
            let started = Instant::now();
            while started.elapsed() < UNINSTALL_WATCH_FOR {
                tokio::time::sleep(poll).await;
                let api = this.api.clone();
                let probe = client.clone();
                let manifest = tokio::task::spawn_blocking(move || api.manifest(&probe, appid))
                    .await
                    .ok()
                    .flatten();
                let gone = manifest.map_or(true, |m| filter::phase(m.flags) == AppPhase::Gone);
                if gone {
                    this.state.lock().unwrap().fingerprint = None;
                    this.scan_now().await;
                    return;
                }
            }
        });
    }

    /// Ends every live watch (Disconnect): their rows end Cancelled.
    fn stop_watches(&self) {
        let watches: Vec<Watch> = {
            let mut state = self.state.lock().unwrap();
            state.watches.drain().map(|(_, watch)| watch).collect()
        };
        for watch in watches {
            watch.stop.store(true, Ordering::SeqCst);
            if let (Some(install), Some(entry)) = (self.install.get(), watch.entry) {
                install.finish_store_external(entry, ExternalOutcome::Cancelled);
            }
        }
    }

    // --- scans ------------------------------------------------------------------------

    /// Finds the client and its accounts, then (connected, client found)
    /// reconciles the Steam registry rows with Steam's manifests, raises
    /// the uninstalled cards, and gives every install Steam runs that GRID
    /// doesn't watch yet a Downloads row and a watch. Emits `changed`.
    pub async fn scan_now(self: &Arc<Self>) {
        let _scan = self.scan_lock.lock().await;
        let api = self.api.clone();
        let found = tokio::task::spawn_blocking(move || {
            let client = api.find_client();
            let accounts = client
                .as_ref()
                .map(|client| api.local_accounts(client))
                .unwrap_or_default();
            let report = client
                .as_ref()
                .map(|client| api.scan(client))
                .unwrap_or_default();
            (client, accounts, report)
        })
        .await;
        let Ok((client, accounts, report)) = found else {
            return;
        };
        let SteamScan {
            manifests,
            unreadable,
            missing_libraries,
        } = report;
        let unreadable: HashSet<u32> = unreadable.into_iter().collect();
        let libraries: Vec<PathBuf> = {
            let mut libraries: Vec<PathBuf> = Vec::new();
            for m in &manifests {
                if !libraries.contains(&m.library) {
                    libraries.push(m.library.clone());
                }
            }
            libraries
        };
        {
            let mut state = self.state.lock().unwrap();
            state.fingerprint = client
                .as_ref()
                .map(|client| fingerprint(&client.root, &libraries));
            state.libraries = libraries;
            state.client = client.clone();
            state.accounts = accounts;
        }
        let (Some(client), true) = (client, self.connected()) else {
            self.changed();
            return;
        };
        self.learn_apps(
            manifests
                .iter()
                .map(|m| (m.appid, m.name.clone(), m.install_dir.clone())),
        )
        .await;
        let Some(registry) = self.registry() else {
            self.changed();
            return;
        };
        let rows = registry.store_rows(StoreKind::Steam).unwrap_or_default();
        let plan = {
            let state = self.state.lock().unwrap();
            let owned: HashMap<u32, &OwnedGame> =
                state.games.iter().map(|g| (g.appid, g)).collect();
            let watched: HashSet<u32> = state.watches.keys().copied().collect();
            plan_scan(&ScanInput {
                manifests: &manifests,
                unreadable: &unreadable,
                missing_libraries: &missing_libraries,
                rows: &rows,
                apps: &state.apps,
                owned: &owned,
                grid_installs: &state.grid_installs,
                seen: &state.seen_installed,
                watched: &watched,
            })
        };
        for row in &plan.upserts {
            if let Err(e) = registry.upsert_store(row) {
                tracing::warn!("stores: Steam row not written for {}: {e}", row.store_id);
            }
        }
        for (store_id, at) in &plan.touches {
            let key = GameKey::store(StoreKind::Steam, store_id.clone());
            if let Err(e) = registry.touch_last_played_key(&key, *at) {
                tracing::warn!("stores: Steam last played not stamped for {store_id}: {e}");
            }
        }
        for removal in &plan.removals {
            if let Err(e) = registry.remove_store(StoreKind::Steam, &removal.store_id) {
                tracing::warn!(
                    "stores: Steam row not removed for {}: {e}",
                    removal.store_id
                );
            }
        }
        {
            let mut state = self.state.lock().unwrap();
            for appid in &plan.installed {
                state.seen_installed.insert(*appid);
            }
            for removal in &plan.removals {
                if let Ok(appid) = removal.store_id.parse::<u32>() {
                    state.seen_installed.remove(&appid);
                }
            }
        }
        for removal in plan.removals.iter().filter(|r| r.card) {
            self.notify(notice::uninstalled(&removal.title));
        }
        for (appid, manifest) in &plan.installing {
            let title = self.title_of(*appid, Some(manifest));
            self.start_watch(client.clone(), *appid, title, false);
        }
        self.changed();
    }

    // --- owned list ---------------------------------------------------------------------

    /// `GetOwnedGames` with the saved key. `announce` (Refresh): the "N games
    /// found" card, or, still private, the error card. No key: the no-key
    /// state. A rejected key flips the card to `bad`; private to `private`;
    /// no network keeps the last state and the cached list.
    pub async fn refresh_owned(self: &Arc<Self>, announce: bool) {
        let _refresh = self.refresh_lock.lock().await;
        let config = self.steam_config();
        if !config.connected {
            return;
        }
        let Ok(id) = config.steam_id.parse::<SteamId>() else {
            return;
        };
        let key = match self.saved_key().await {
            Ok(Some(key)) => key,
            Ok(None) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.owned = OwnedState::Idle;
                    state.games.clear();
                }
                self.changed();
                return;
            }
            Err(e) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.owned = OwnedState::Error;
                    state.owned_error = e;
                }
                self.changed();
                return;
            }
        };
        let had_list = {
            let mut state = self.state.lock().unwrap();
            let had = state.owned == OwnedState::Ready && !state.games.is_empty();
            state.owned = OwnedState::Loading;
            state.owned_error.clear();
            had
        };
        self.changed();
        match self.api.owned(&key, id).await {
            Ok(owned) => {
                self.learn_apps(
                    owned
                        .games
                        .iter()
                        .map(|g| (g.appid, g.name.clone(), String::new())),
                )
                .await;
                let count = self.take_owned(owned.games);
                if announce {
                    let installed: HashSet<String> = self
                        .installed_rows()
                        .into_iter()
                        .map(|row| row.store_id)
                        .collect();
                    let to_install = {
                        let state = self.state.lock().unwrap();
                        owned_games(&state.games, &state.apps)
                            .filter(|g| !installed.contains(&g.appid.to_string()))
                            .count()
                    };
                    self.notify(notice::games_found(count, to_install));
                }
                // The owned list moves last played forward.
                self.state.lock().unwrap().fingerprint = None;
                self.scan_now().await;
            }
            Err(SteamApiError::Private) => {
                self.take_private();
                if announce {
                    self.notify(notice::still_private());
                }
            }
            Err(error @ SteamApiError::Rejected { .. }) => {
                let mut state = self.state.lock().unwrap();
                state.answer = Some(SteamKeyState::Bad);
                state.owned = OwnedState::Error;
                state.owned_error = error.to_string();
            }
            Err(error) => {
                let mut state = self.state.lock().unwrap();
                if had_list {
                    state.owned = OwnedState::Ready;
                } else {
                    state.owned = OwnedState::Error;
                }
                state.owned_error = error.to_string();
            }
        }
        self.changed();
    }

    // --- Disconnect -------------------------------------------------------------------------

    /// Disconnect: the key's keyring slot (a missing item is fine; any other
    /// error stops it), both caches, every Steam registry row (rows only:
    /// the games stay installed in Steam), `[stores.steam]`, the in-memory
    /// key and every watch (live rows end Cancelled). The frontend raises
    /// its card.
    pub fn disconnect(&self) -> Result<(), String> {
        self.keys.clear().map_err(|e| {
            format!("Couldn\u{2019}t delete your Web API key from the OS keyring: {e}")
        })?;
        self.stop_watches();
        remove_file(&self.cache_dir().join(OWNED_CACHE));
        remove_file(&self.cache_dir().join(APPS_CACHE));
        if let Some(registry) = self.registry() {
            for row in registry.store_rows(StoreKind::Steam).unwrap_or_default() {
                if let Err(e) = registry.remove_store(StoreKind::Steam, &row.store_id) {
                    tracing::warn!("stores: Steam row not removed for {}: {e}", row.store_id);
                }
            }
        }
        self.write_steam_config(|steam| *steam = SteamConfig::default())?;
        {
            let mut state = self.state.lock().unwrap();
            let client = state.client.take();
            let accounts = std::mem::take(&mut state.accounts);
            *state = SteamState {
                client,
                accounts,
                key_loaded: true,
                ..SteamState::default()
            };
        }
        self.changed();
        Ok(())
    }

    // --- launch -----------------------------------------------------------------------------

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
            .registry()
            .ok_or_else(|| LaunchError::Registry("not ready".to_string()))?;
        let lookup = key.clone();
        let row = tokio::task::spawn_blocking(move || registry.find_key(&lookup))
            .await
            .map_err(|e| LaunchError::Registry(e.to_string()))?
            .map_err(|e| LaunchError::Registry(e.to_string()))?
            .ok_or(LaunchError::NotInstalled)?;
        let api = self.api.clone();
        let client = tokio::task::spawn_blocking(move || api.find_client())
            .await
            .map_err(|e| LaunchError::Registry(e.to_string()))?
            .ok_or_else(|| LaunchError::Validation(STEAM_NOT_INSTALLED.to_string()))?;
        self.api
            .open(&client, SteamLink::Run(appid))
            .map_err(|e| LaunchError::Validation(format!("Couldn\u{2019}t open Steam: {e}")))?;
        let alive = self.api.alive_check(&client, appid);
        launch.register_watched(key, &row.title, STORE_NAME, STORE_NAME, 0, alive)
    }
}

// --- pure parts --------------------------------------------------------------------

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

/// The owned games that are games (rules 1–3; rule 3 when the store type
/// is known).
fn owned_games<'a>(
    games: &'a [OwnedGame],
    apps: &'a HashMap<u32, AppInfo>,
) -> impl Iterator<Item = &'a OwnedGame> + 'a {
    games.iter().filter(move |g| {
        filter::is_game(
            g.appid,
            &g.name,
            "",
            apps.get(&g.appid).and_then(|a| a.kind),
        )
    })
}

/// What [`assemble_status`] and [`assemble_games`] read from the state.
struct View<'a> {
    key: SteamKeyState,
    owned: OwnedState,
    owned_error: &'a str,
    games: &'a [OwnedGame],
    apps: &'a HashMap<u32, AppInfo>,
    refreshed_at: i64,
    client: Option<&'a SteamClient>,
    accounts: &'a [LocalAccount],
}

fn view(state: &SteamState) -> View<'_> {
    View {
        key: state.key_state(),
        owned: state.owned,
        owned_error: &state.owned_error,
        games: &state.games,
        apps: &state.apps,
        refreshed_at: state.refreshed_at,
        client: state.client.as_ref(),
        accounts: &state.accounts,
    }
}

/// Steam's [`StoreStatus`]: no tool, the owned list while the key works,
/// the installed rows while the client is found, the card's Steam part.
/// Never the key.
fn assemble_status(config: &SteamConfig, view: &View<'_>, rows: &[InstalledGame]) -> StoreStatus {
    let connection = if config.connected {
        StoreConnection::Connected
    } else {
        StoreConnection::NotConnected
    };
    let shown_rows: &[InstalledGame] = if config.connected && view.client.is_some() {
        rows
    } else {
        &[]
    };
    let owned_count = if config.connected && view.key == SteamKeyState::Ok {
        owned_games(view.games, view.apps).count() as u32
    } else {
        0
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
        owned: if config.connected {
            view.owned
        } else {
            OwnedState::Idle
        },
        owned_error: if config.connected {
            view.owned_error.to_string()
        } else {
            String::new()
        },
        owned_count,
        installed_count: shown_rows.len() as u32,
        installed_bytes: shown_rows
            .iter()
            .map(|row| row.filesize_bytes.max(0) as u64)
            .sum(),
        refreshed_at: if config.connected {
            view.refreshed_at
        } else {
            0
        },
        cloud_saves: false,
        install_dir: String::new(),
        in_rail: config.connected,
        last_error: String::new(),
        steam: Some(SteamCard {
            client_path: view
                .client
                .map(|client| client.display.clone())
                .unwrap_or_default(),
            steam_id: config.steam_id.clone(),
            key: if config.connected {
                view.key
            } else {
                SteamKeyState::None
            },
            key_tail: String::new(),
            accounts: view
                .accounts
                .iter()
                .map(|account| SteamAccount {
                    persona: account.persona.clone(),
                    steam_id: account.steam_id.to_string(),
                    most_recent: account.most_recent,
                })
                .collect(),
        }),
    }
}

/// A game's cover: the store's capsule, else the plain
/// `steam/apps/<id>/library_600x900.jpg` under the art base.
fn cover_of(appid: u32, info: Option<&AppInfo>, endpoints: &SteamEndpoints) -> String {
    info.map(|info| info.cover.trim().to_string())
        .filter(|cover| !cover.is_empty())
        .unwrap_or_else(|| {
            endpoints
                .art_base
                .join(&format!("steam/apps/{appid}/library_600x900.jpg"))
                .map(|art| art.to_string())
                .unwrap_or_default()
        })
}

fn first_of(text: &str) -> String {
    text.split([',', '\n'])
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string()
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Installed (the rows, while connected and the client is found) and
/// Available (owned games that are games and have no row, while the key
/// works). No platform, version, size for Available, DLC or cloud.
fn assemble_games(
    view: &View<'_>,
    rows: &[InstalledGame],
    connected: bool,
    endpoints: &SteamEndpoints,
) -> (Vec<StoreGame>, Vec<StoreGame>) {
    if !connected {
        return (Vec::new(), Vec::new());
    }
    let apps = view.apps;
    let base = |key: String, store_id: String, title: String| StoreGame {
        key,
        source: SOURCE.to_string(),
        store_id,
        title,
        installed: false,
        cover: String::new(),
        screenshots: Vec::new(),
        description: String::new(),
        developer: String::new(),
        genres: Vec::new(),
        year: String::new(),
        platforms: Vec::new(),
        installed_platform: None,
        download_bytes: 0,
        disk_bytes: 0,
        version: String::new(),
        update_version: String::new(),
        origin: None,
        not_supported: String::new(),
        dlc: Vec::new(),
        cloud: "none".to_string(),
        last_sync: 0,
        install_dir: String::new(),
        compat_tool: String::new(),
    };
    let installed: Vec<StoreGame> = if view.client.is_some() {
        rows.iter()
            .map(|row| {
                let meta = SteamMeta::from_json(&row.store_meta);
                let appid: u32 = row.store_id.parse().unwrap_or(0);
                let info = apps.get(&appid);
                let mut game = base(
                    GameKey::store(StoreKind::Steam, row.store_id.clone()).to_string(),
                    row.store_id.clone(),
                    row.title.clone(),
                );
                game.installed = true;
                game.cover = cover_of(appid, info, endpoints);
                game.screenshots = info
                    .map(|info| info.screenshots.clone())
                    .filter(|shots| !shots.is_empty())
                    .unwrap_or_else(|| lines_of(&row.screenshot_urls));
                game.description = info
                    .map(|info| info.description.clone())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| row.description.clone());
                game.developer = info
                    .and_then(|info| info.developers.first().cloned())
                    .unwrap_or_else(|| first_of(&row.companies));
                game.year = info
                    .map(|info| info.year.clone())
                    .filter(|year| !year.is_empty())
                    .unwrap_or_else(|| row.first_release_date.clone());
                game.disk_bytes = row.filesize_bytes.max(0) as u64;
                game.origin = Some(
                    if meta.outside {
                        "steam_outside"
                    } else {
                        "steam"
                    }
                    .to_string(),
                );
                game.install_dir = install_path(&meta.library, &meta.install_dir)
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default();
                game
            })
            .collect()
    } else {
        Vec::new()
    };
    let available = if view.key == SteamKeyState::Ok {
        let installed_ids: HashSet<&str> = rows.iter().map(|row| row.store_id.as_str()).collect();
        owned_games(view.games, apps)
            .filter(|g| !installed_ids.contains(g.appid.to_string().as_str()))
            .map(|g| {
                let info = apps.get(&g.appid);
                let title = info
                    .map(|info| info.name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| g.name.clone());
                let mut game = base(
                    GameKey::store(StoreKind::Steam, g.appid.to_string()).to_string(),
                    g.appid.to_string(),
                    title,
                );
                game.cover = cover_of(g.appid, info, endpoints);
                if let Some(info) = info {
                    game.screenshots = info.screenshots.clone();
                    game.description = info.description.clone();
                    game.developer = info.developers.first().cloned().unwrap_or_default();
                    game.year = info.year.clone();
                }
                game
            })
            .collect()
    } else {
        Vec::new()
    };
    (installed, available)
}

/// What [`plan_scan`] reads.
struct ScanInput<'a> {
    manifests: &'a [ManifestState],
    /// Manifests that didn't parse (Steam writing them): their rows stay.
    unreadable: &'a HashSet<u32>,
    /// Libraries that couldn't be read (an unplugged drive): their games'
    /// rows stay.
    missing_libraries: &'a [PathBuf],
    rows: &'a [InstalledGame],
    apps: &'a HashMap<u32, AppInfo>,
    owned: &'a HashMap<u32, &'a OwnedGame>,
    grid_installs: &'a HashSet<u32>,
    seen: &'a HashSet<u32>,
    watched: &'a HashSet<u32>,
}

/// A row a scan removes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Removal {
    store_id: String,
    title: String,
    /// GRID saw it installed in this run: the uninstalled card.
    card: bool,
}

/// What one scan does to the registry and the watches.
#[derive(Debug, Default)]
struct ScanPlan {
    upserts: Vec<InstalledGame>,
    /// `last_played_at` moves forward to these.
    touches: Vec<(String, i64)>,
    removals: Vec<Removal>,
    /// Installed games (seen in this run).
    installed: Vec<u32>,
    /// Installs Steam runs that no watch follows yet.
    installing: Vec<(u32, ManifestState)>,
}

/// The fields a scan compares before it rewrites a row.
fn same_row(a: &InstalledGame, b: &InstalledGame) -> bool {
    a.title == b.title
        && a.store_meta == b.store_meta
        && a.filesize_bytes == b.filesize_bytes
        && a.revision == b.revision
        && a.installed_at == b.installed_at
        && a.description == b.description
        && a.companies == b.companies
        && a.first_release_date == b.first_release_date
        && a.screenshot_urls == b.screenshot_urls
}

/// One scan's reconciliation (pure): an installed game that passes the
/// filter gets its row (`outside` unless GRID's watcher saw the install,
/// kept from the old row otherwise) and its last played moved forward; a
/// row whose manifest is gone, uninstalling or filtered out goes (with the
/// card when GRID saw it installed in this run); a row whose manifest is
/// mid-install stays. Installs no watch follows are listed.
fn plan_scan(input: &ScanInput<'_>) -> ScanPlan {
    let mut plan = ScanPlan::default();
    // One manifest per app: the installed one first.
    let mut by_app: HashMap<u32, &ManifestState> = HashMap::new();
    for m in input.manifests {
        let better = match by_app.get(&m.appid) {
            None => true,
            Some(old) => {
                filter::phase(old.flags) != AppPhase::Installed
                    && filter::phase(m.flags) == AppPhase::Installed
            }
        };
        if better {
            by_app.insert(m.appid, m);
        }
    }
    let rows: HashMap<&str, &InstalledGame> = input
        .rows
        .iter()
        .map(|row| (row.store_id.as_str(), row))
        .collect();
    let mut appids: Vec<u32> = by_app.keys().copied().collect();
    appids.sort_unstable();
    let mut keep: HashSet<String> = HashSet::new();
    for appid in appids {
        let m = by_app[&appid];
        let info = input.apps.get(&appid);
        if !filter::is_game(appid, &m.name, &m.install_dir, info.and_then(|i| i.kind)) {
            continue;
        }
        let store_id = appid.to_string();
        let old = rows.get(store_id.as_str()).copied();
        match filter::phase(m.flags) {
            AppPhase::Installed => {
                let outside = if input.grid_installs.contains(&appid) {
                    false
                } else {
                    old.map(|row| SteamMeta::from_json(&row.store_meta).outside)
                        .unwrap_or(true)
                };
                let meta = SteamMeta {
                    library: m.library.to_string_lossy().into_owned(),
                    install_dir: m.install_dir.clone(),
                    build_id: m.build_id,
                    outside,
                };
                let mut row = steam_row(m, info, &meta);
                if m.last_updated <= 0 {
                    if let Some(old) = old {
                        row.installed_at = old.installed_at;
                    }
                }
                if old.map_or(true, |old| !same_row(old, &row)) {
                    plan.upserts.push(row);
                }
                let owned_played = input.owned.get(&appid).map_or(0, |g| g.last_played);
                let played = m.last_played.max(owned_played);
                if played > old.map_or(0, |old| old.last_played_at) {
                    plan.touches.push((store_id.clone(), played));
                }
                plan.installed.push(appid);
                keep.insert(store_id);
            }
            AppPhase::Installing => {
                if old.is_some() {
                    keep.insert(store_id);
                }
                if !input.watched.contains(&appid) {
                    plan.installing.push((appid, m.clone()));
                }
            }
            AppPhase::Gone => {}
        }
    }
    for row in input.rows {
        if keep.contains(&row.store_id) {
            continue;
        }
        // Not known to be gone: a manifest Steam is writing, or a library
        // that is unplugged.
        let unreadable = row
            .store_id
            .parse::<u32>()
            .is_ok_and(|appid| input.unreadable.contains(&appid));
        let library = PathBuf::from(SteamMeta::from_json(&row.store_meta).library);
        if unreadable
            || (!library.as_os_str().is_empty() && input.missing_libraries.contains(&library))
        {
            continue;
        }
        let card = row
            .store_id
            .parse::<u32>()
            .is_ok_and(|appid| input.seen.contains(&appid));
        plan.removals.push(Removal {
            store_id: row.store_id.clone(),
            title: row.title.clone(),
            card,
        });
    }
    plan
}

/// The mtimes that tell the background scan something changed: the root's
/// `libraryfolders.vdf` and `steamapps/`, and each library's `steamapps/`
/// (Steam writes a manifest by renaming a new file over it).
fn fingerprint(root: &Path, libraries: &[PathBuf]) -> Vec<(PathBuf, Option<SystemTime>)> {
    let mut paths = vec![
        root.join("steamapps"),
        root.join("steamapps").join("libraryfolders.vdf"),
    ];
    for library in libraries {
        let dir = library.join("steamapps");
        if !paths.contains(&dir) {
            paths.push(dir);
        }
    }
    paths
        .into_iter()
        .map(|path| {
            let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            (path, mtime)
        })
        .collect()
}

// --- caches ---------------------------------------------------------------------------

/// `steam-owned.json`: the owned list as `GetOwnedGames` gave it.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct OwnedCache {
    refreshed_at: i64,
    games: Vec<OwnedCacheGame>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct OwnedCacheGame {
    appid: u32,
    name: String,
    last_played: i64,
}

/// `steam-apps.json`: the `GetItems` answers.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct AppsCache {
    apps: Vec<AppsCacheEntry>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct AppsCacheEntry {
    appid: u32,
    kind: Option<u8>,
    name: String,
    description: String,
    developers: Vec<String>,
    year: String,
    cover: String,
    screenshots: Vec<String>,
}

fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

fn remove_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != io::ErrorKind::NotFound {
            tracing::warn!("stores: Steam cache not deleted: {e}");
        }
    }
}

fn read_owned_cache(dir: &Path) -> (Vec<OwnedGame>, i64) {
    let Ok(text) = std::fs::read_to_string(dir.join(OWNED_CACHE)) else {
        return (Vec::new(), 0);
    };
    let cache: OwnedCache = serde_json::from_str(&text).unwrap_or_default();
    let games = cache
        .games
        .into_iter()
        .map(|g| OwnedGame {
            appid: g.appid,
            name: g.name,
            last_played: g.last_played,
        })
        .collect();
    (games, cache.refreshed_at)
}

fn write_owned_cache(dir: &Path, games: &[OwnedGame], at: i64) -> io::Result<()> {
    let cache = OwnedCache {
        refreshed_at: at,
        games: games
            .iter()
            .map(|g| OwnedCacheGame {
                appid: g.appid,
                name: g.name.clone(),
                last_played: g.last_played,
            })
            .collect(),
    };
    let text = serde_json::to_string(&cache).map_err(io::Error::other)?;
    write_atomic(&dir.join(OWNED_CACHE), &text)
}

fn read_apps_cache(dir: &Path) -> HashMap<u32, AppInfo> {
    let Ok(text) = std::fs::read_to_string(dir.join(APPS_CACHE)) else {
        return HashMap::new();
    };
    let cache: AppsCache = serde_json::from_str(&text).unwrap_or_default();
    cache
        .apps
        .into_iter()
        .map(|e| {
            (
                e.appid,
                AppInfo {
                    appid: e.appid,
                    kind: e.kind,
                    name: e.name,
                    description: e.description,
                    developers: e.developers,
                    year: e.year,
                    cover: e.cover,
                    screenshots: e.screenshots,
                },
            )
        })
        .collect()
}

fn write_apps_cache(dir: &Path, apps: &HashMap<u32, AppInfo>) -> io::Result<()> {
    let mut entries: Vec<AppsCacheEntry> = apps
        .values()
        .map(|info| AppsCacheEntry {
            appid: info.appid,
            kind: info.kind,
            name: info.name.clone(),
            description: info.description.clone(),
            developers: info.developers.clone(),
            year: info.year.clone(),
            cover: info.cover.clone(),
            screenshots: info.screenshots.clone(),
        })
        .collect();
    entries.sort_by_key(|e| e.appid);
    let text = serde_json::to_string(&AppsCache { apps: entries }).map_err(io::Error::other)?;
    write_atomic(&dir.join(APPS_CACHE), &text)
}

// --- notices --------------------------------------------------------------------------

/// Steam's `store-notice` cards, in the GameStores card's words: what
/// happens in the background (an install Steam finished, a game Steam
/// removed, a Refresh's result) and the connect outcome of `steam_save`.
/// The frontend raises the cards for the other commands it calls itself
/// (the install/verify/uninstall toasts, Remove key, Check again,
/// Disconnect). No cloud cards: Steam Cloud is Steam's.
pub mod notice {
    use super::{StoreNotice, STORE_NAME};
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

    /// "Steam connected · 4 games owned".
    pub fn connected(count: usize) -> StoreNotice {
        let games = if count == 1 { "game" } else { "games" };
        card(
            format!("{STORE_NAME} connected \u{b7} {count} {games} owned"),
            false,
            None,
        )
    }

    /// Connected, but the profile's game details are private.
    pub fn connected_private() -> StoreNotice {
        card(
            format!(
                "{STORE_NAME} connected, but your game details are private, so Available is empty"
            ),
            true,
            None,
        )
    }

    /// A Refresh's owned list.
    pub fn games_found(count: usize, to_install: usize) -> StoreNotice {
        let games = if count == 1 { "game" } else { "games" };
        card(
            format!("{STORE_NAME}: {count} {games} found \u{b7} {to_install} to install"),
            false,
            None,
        )
    }

    /// A Refresh while the game details are still private.
    pub fn still_private() -> StoreNotice {
        card(
            format!(
                "{STORE_NAME} still lists no games: the profile\u{2019}s game details are private"
            ),
            true,
            None,
        )
    }

    /// Steam finished an install GRID watched.
    pub fn installed(title: &str, key: &str) -> StoreNotice {
        card(
            format!("{title} installed"),
            false,
            Some(StoreNoticeAction {
                kind: "play".to_string(),
                key: Some(key.to_string()),
                title: Some(title.to_string()),
            }),
        )
    }

    /// Steam removed a game GRID saw installed (no cloud sentence).
    pub fn uninstalled(title: &str) -> StoreNotice {
        card(format!("{title} uninstalled"), false, None)
    }
}

// --- the trait ----------------------------------------------------------------------------

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
    fn load(&self) {
        SteamStore::load(self)
    }
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()> {
        Box::pin(async move { self.check_login().await })
    }
    fn observe_downloads(&self, _snapshot: &DownloadsSnapshot) {
        // The install watches end their own rows and raise their cards.
    }
    fn login_spec(&self) -> LoginSpec {
        // Never opened: `begin_login` and `prepare_login` refuse first.
        LoginSpec {
            store_name: STORE_NAME.to_string(),
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
        SteamStore::disconnect(self)
    }
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()> {
        // Steam has no tool.
        Box::pin(async {})
    }
    fn run_refresh(self: Arc<Self>, announce: bool) -> BoxFut<'static, ()> {
        Box::pin(async move { self.refresh_owned(announce).await })
    }
    fn library(&self, _updates: &HashMap<String, String>) -> StoreLibrary {
        SteamStore::library(self)
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
    fn game_settings(&self, _row: &InstalledGame) -> StoreGameSettings {
        // The design gives Steam games no Game Settings.
        StoreGameSettings {
            install_dir: String::new(),
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
        // Steam syncs its own cloud saves.
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
    use grid_core::library::queue::DownloadStatus;
    use grid_core::secrets::MemoryStore;
    use std::sync::atomic::AtomicUsize;

    const KEY: &str = "FAKE-STEAM-KEY-not-real";
    const SAM: &str = "76561198000000001";
    const GUEST: &str = "76561198000000002";
    const MANUAL: &str = "76561198000000005";

    struct FakeSteam {
        client: Mutex<Option<SteamClient>>,
        accounts: Mutex<Vec<LocalAccount>>,
        manifests: Mutex<Vec<ManifestState>>,
        unreadable: Mutex<Vec<u32>>,
        owned: Mutex<Result<OwnedGames, SteamApiError>>,
        persona: Mutex<Result<String, SteamApiError>>,
        apps: Mutex<Vec<AppInfo>>,
        opened: Mutex<Vec<String>>,
        alive: Arc<AtomicBool>,
        owned_calls: AtomicUsize,
        persona_calls: AtomicUsize,
    }

    fn owned(appid: u32, name: &str, last_played: i64) -> OwnedGame {
        OwnedGame {
            appid,
            name: name.into(),
            last_played,
        }
    }

    fn app(appid: u32, name: &str, kind: Option<u8>) -> AppInfo {
        AppInfo {
            appid,
            kind,
            name: name.into(),
            description: format!("About {name}"),
            developers: vec!["Slow Orbit".into()],
            year: "2025".into(),
            cover: format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{appid}/library_600x900.jpg"),
            screenshots: vec![],
        }
    }

    fn manifest(appid: u32, name: &str, flags: u32) -> ManifestState {
        ManifestState {
            appid,
            name: name.into(),
            library: "/fake/steam".into(),
            install_dir: name.into(),
            flags,
            size_on_disk: 7_000_000_000,
            build_id: 77,
            last_updated: 1_700_000_000,
            last_played: 0,
            ..Default::default()
        }
    }

    impl Default for FakeSteam {
        fn default() -> Self {
            FakeSteam {
                client: Mutex::new(Some(SteamClient {
                    root: "/fake/steam".into(),
                    exe: grid_core::stores::steam::ClientExe::Path("/usr/bin/steam".into()),
                    display: "/usr/bin/steam".into(),
                })),
                accounts: Mutex::new(vec![
                    LocalAccount {
                        steam_id: SAM.parse().unwrap(),
                        persona: "SamPlays".into(),
                        most_recent: true,
                    },
                    LocalAccount {
                        steam_id: GUEST.parse().unwrap(),
                        persona: "couchguest".into(),
                        most_recent: false,
                    },
                ]),
                manifests: Mutex::new(vec![
                    {
                        let mut m = manifest(3_000_010, "Moonlit Ferry", 4);
                        m.last_played = 1_700_000_500;
                        m
                    },
                    manifest(3_000_020, "Deep Field Radio", 4),
                    manifest(2_805_730, "Proton 9.0", 4),
                    manifest(228_980, "Steamworks Common Redistributables", 4),
                ]),
                unreadable: Mutex::new(Vec::new()),
                owned: Mutex::new(Ok(OwnedGames {
                    games: vec![
                        owned(3_000_040, "Quiet Comet", 1_700_100_000),
                        owned(3_000_050, "Harbor of Bells", 0),
                        owned(3_000_060, "Rime Assembly", 0),
                        owned(3_000_070, "Signal Lost", 0),
                        owned(3_000_080, "Desk Lights", 0),
                        owned(2_805_730, "Proton 9.0", 0),
                    ],
                })),
                persona: Mutex::new(Ok("Manual Person".into())),
                apps: Mutex::new(vec![
                    app(3_000_010, "Moonlit Ferry", Some(0)),
                    app(3_000_020, "Deep Field Radio", Some(0)),
                    app(3_000_040, "Quiet Comet", Some(0)),
                    app(3_000_050, "Harbor of Bells", Some(0)),
                    app(3_000_060, "Rime Assembly", Some(0)),
                    app(3_000_070, "Signal Lost", Some(0)),
                    app(3_000_080, "Desk Lights", Some(6)),
                ]),
                opened: Mutex::new(Vec::new()),
                alive: Arc::new(AtomicBool::new(true)),
                owned_calls: AtomicUsize::new(0),
                persona_calls: AtomicUsize::new(0),
            }
        }
    }

    impl SteamApi for FakeSteam {
        fn endpoints(&self) -> SteamEndpoints {
            SteamEndpoints::real()
        }
        fn find_client(&self) -> Option<SteamClient> {
            self.client.lock().unwrap().clone()
        }
        fn local_accounts(&self, _client: &SteamClient) -> Vec<LocalAccount> {
            self.accounts.lock().unwrap().clone()
        }
        fn scan(&self, _client: &SteamClient) -> SteamScan {
            SteamScan {
                manifests: self.manifests.lock().unwrap().clone(),
                unreadable: self.unreadable.lock().unwrap().clone(),
                missing_libraries: Vec::new(),
            }
        }
        fn manifest(&self, _client: &SteamClient, appid: u32) -> Option<ManifestState> {
            self.manifests
                .lock()
                .unwrap()
                .iter()
                .find(|m| m.appid == appid)
                .cloned()
        }
        fn open(&self, _client: &SteamClient, link: SteamLink) -> io::Result<()> {
            let shown = if link.silent() {
                format!("-silent {link}")
            } else {
                link.to_string()
            };
            self.opened.lock().unwrap().push(shown);
            Ok(())
        }
        fn alive_check(&self, _client: &SteamClient, _appid: u32) -> AliveCheck {
            let alive = self.alive.clone();
            Box::new(move || alive.load(Ordering::SeqCst))
        }
        fn owned<'a>(
            &'a self,
            _key: &'a SecretString,
            _id: SteamId,
        ) -> BoxFut<'a, Result<OwnedGames, SteamApiError>> {
            self.owned_calls.fetch_add(1, Ordering::SeqCst);
            let answer = self.owned.lock().unwrap().clone();
            Box::pin(async move { answer })
        }
        fn persona<'a>(
            &'a self,
            _key: &'a SecretString,
            _id: SteamId,
        ) -> BoxFut<'a, Result<String, SteamApiError>> {
            self.persona_calls.fetch_add(1, Ordering::SeqCst);
            let answer = self.persona.lock().unwrap().clone();
            Box::pin(async move { answer })
        }
        fn app_info<'a>(&'a self, ids: &'a [u32]) -> BoxFut<'a, Vec<AppInfo>> {
            let answer: Vec<AppInfo> = self
                .apps
                .lock()
                .unwrap()
                .iter()
                .filter(|info| ids.contains(&info.appid))
                .cloned()
                .collect();
            Box::pin(async move { answer })
        }
    }

    struct Harness {
        store: Arc<SteamStore>,
        fake: Arc<FakeSteam>,
        keys: Arc<MemoryStore>,
        registry: Arc<Registry>,
        install: Arc<InstallService>,
        events: Arc<Mutex<Vec<StoreEvent>>>,
        dir: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let fake = Arc::new(FakeSteam::default());
        let keys = Arc::new(MemoryStore::default());
        let shared = StoreShared::new();
        let registry = Arc::new(Registry::open(&dir.path().join("grid-launcher.db")).unwrap());
        shared.set_registry(registry.clone());
        let store = SteamStore::new(
            fake.clone(),
            config_path.clone(),
            keys.clone(),
            shared.clone(),
        );
        let install = InstallService::new(registry.clone(), config_path);
        store.set_install(install.clone());
        *store.poll.lock().unwrap() = Duration::from_millis(20);
        shared.register(store.clone());
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        shared.set_emitter(Arc::new(move |event| sink.lock().unwrap().push(event)));
        Harness {
            store,
            fake,
            keys,
            registry,
            install,
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
        fn config(&self) -> SteamConfig {
            self.store.steam_config()
        }
        fn rows(&self) -> Vec<InstalledGame> {
            self.registry.store_rows(StoreKind::Steam).unwrap()
        }
        async fn connect(&self) {
            let result = self
                .store
                .save(SAM, Some(SecretString::from(KEY)))
                .await
                .unwrap();
            assert_eq!(result, SteamTestResult::Ok { owned_count: 4 });
        }
        /// Everything the frontend could ever see, as JSON.
        fn visible(&self) -> String {
            let events: Vec<String> = self
                .events
                .lock()
                .unwrap()
                .iter()
                .map(|event| format!("{event:?}"))
                .collect();
            format!(
                "{} {} {:?} {}",
                serde_json::to_string(&self.store.status()).unwrap(),
                serde_json::to_string(&self.store.library()).unwrap(),
                events,
                std::fs::read_to_string(self.store.config_path.clone()).unwrap_or_default(),
            )
        }
        fn cache_files(&self) -> String {
            let dir = self.dir.path().join("stores").join("cache");
            [OWNED_CACHE, APPS_CACHE]
                .iter()
                .map(|name| std::fs::read_to_string(dir.join(name)).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    async fn wait_for(mut test: impl FnMut() -> bool, what: &str) {
        for _ in 0..300 {
            if test() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn not_connected_the_card_shows_the_client_and_the_accounts_but_no_key() {
        let h = harness();
        h.store.load();
        let status = h.store.status();
        assert_eq!(status.source, "steam");
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert_eq!(status.tool.name, "");
        assert!(!status.in_rail);
        assert_eq!(status.installed_count, 0);
        let card = status.steam.unwrap();
        assert_eq!(card.client_path, "/usr/bin/steam");
        assert_eq!(card.key, SteamKeyState::None);
        assert_eq!(card.key_tail, "");
        assert_eq!(card.steam_id, "");
        let personas: Vec<(&str, bool)> = card
            .accounts
            .iter()
            .map(|a| (a.persona.as_str(), a.most_recent))
            .collect();
        assert_eq!(personas, [("SamPlays", true), ("couchguest", false)]);
        let library = h.store.library();
        assert!(library.installed.is_empty() && library.available.is_empty());
    }

    #[tokio::test]
    async fn test_checks_the_form_then_one_owned_call_counting_games_only() {
        let h = harness();
        assert_eq!(
            h.store.test("123", None).await,
            Ok(SteamTestResult::BadSteamId)
        );
        assert_eq!(
            h.fake.owned_calls.load(Ordering::SeqCst),
            0,
            "no request for a bad form"
        );
        assert_eq!(
            h.store.test(SAM, None).await,
            Err(PASTE_KEY_FIRST.to_string())
        );
        // Desk Lights is software (type 6) and Proton a tool: 4 games.
        assert_eq!(
            h.store.test(SAM, Some(SecretString::from(KEY))).await,
            Ok(SteamTestResult::Ok { owned_count: 4 })
        );
        assert_eq!(h.fake.owned_calls.load(Ordering::SeqCst), 1);
        for (answer, result) in [
            (
                SteamApiError::Rejected { status: 401 },
                SteamTestResult::BadKey { status: 401 },
            ),
            (SteamApiError::Private, SteamTestResult::Private),
            (SteamApiError::Network, SteamTestResult::Network),
            (SteamApiError::NoSuchAccount, SteamTestResult::BadSteamId),
        ] {
            *h.fake.owned.lock().unwrap() = Err(answer);
            assert_eq!(
                h.store.test(SAM, Some(SecretString::from(KEY))).await,
                Ok(result)
            );
        }
        // A test saves nothing.
        assert!(h.keys.load().unwrap().is_none());
        assert!(!h.config().connected);
        assert_eq!(h.fake.persona_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn every_api_answer_is_a_test_result() {
        assert_eq!(test_result(Ok(4)), SteamTestResult::Ok { owned_count: 4 });
        assert_eq!(
            test_result(Err(SteamApiError::Rejected { status: 403 })),
            SteamTestResult::BadKey { status: 403 }
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
    async fn connect_saves_the_key_the_config_the_list_and_the_rows() {
        let h = harness();
        h.store.load();
        h.connect().await;
        assert!(
            h.keys.load().unwrap().is_some(),
            "the key is in the keyring"
        );
        assert_eq!(
            h.config(),
            SteamConfig {
                connected: true,
                steam_id: SAM.into(),
                persona: "SamPlays".into(),
            }
        );
        assert_eq!(
            h.fake.persona_calls.load(Ordering::SeqCst),
            0,
            "a local account"
        );
        assert_eq!(h.messages(), ["Steam connected \u{b7} 4 games owned"]);
        let status = h.store.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.username, "SamPlays");
        assert!(status.in_rail);
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.owned_count, 4);
        assert_eq!(status.installed_count, 2, "tools never show");
        assert_eq!(status.installed_bytes, 14_000_000_000);
        assert_eq!(status.steam.as_ref().unwrap().key, SteamKeyState::Ok);
        let library = h.store.library();
        let available: Vec<&str> = library.available.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(
            available,
            [
                "Quiet Comet",
                "Harbor of Bells",
                "Rime Assembly",
                "Signal Lost"
            ]
        );
        let installed: Vec<(&str, Option<&str>)> = library
            .installed
            .iter()
            .map(|g| (g.title.as_str(), g.origin.as_deref()))
            .collect();
        assert_eq!(
            installed,
            [
                ("Deep Field Radio", Some("steam_outside")),
                ("Moonlit Ferry", Some("steam_outside"))
            ]
        );
        let ferry = &library.installed[1];
        assert_eq!(ferry.key, "steam:3000010");
        assert!(ferry.platforms.is_empty() && ferry.installed_platform.is_none());
        assert_eq!(ferry.cloud, "none");
        assert_eq!(ferry.update_version, "");
        assert_eq!(ferry.disk_bytes, 7_000_000_000);
        assert!(ferry.cover.contains("3000010"));
        // The manifest's LastPlayed moved last played forward.
        let rows = h.rows();
        let ferry_row = rows.iter().find(|r| r.store_id == "3000010").unwrap();
        assert_eq!(ferry_row.last_played_at, 1_700_000_500);
        assert!(ferry_row.extracted_dir.is_empty() && ferry_row.native_game_dir.is_empty());
        // Both caches were written; neither, nor anything shown, holds the key.
        assert!(h.cache_files().contains("Quiet Comet"));
        assert!(!h.cache_files().contains(KEY));
        assert!(!h.visible().contains(KEY));
    }

    #[tokio::test]
    async fn a_manual_steam_id_asks_steam_for_the_persona() {
        let h = harness();
        h.store.load();
        assert_eq!(
            h.store.save(MANUAL, Some(SecretString::from(KEY))).await,
            Ok(SteamTestResult::Ok { owned_count: 4 })
        );
        assert_eq!(h.fake.persona_calls.load(Ordering::SeqCst), 1);
        assert_eq!(h.config().persona, "Manual Person");

        let h = harness();
        h.store.load();
        *h.fake.persona.lock().unwrap() = Err(SteamApiError::NoSuchAccount);
        assert_eq!(
            h.store.save(MANUAL, Some(SecretString::from(KEY))).await,
            Ok(SteamTestResult::BadSteamId)
        );
        assert!(!h.config().connected);
        assert!(h.keys.load().unwrap().is_none());
    }

    #[tokio::test]
    async fn a_rejected_key_or_no_network_saves_nothing() {
        for answer in [
            SteamApiError::Rejected { status: 401 },
            SteamApiError::Network,
        ] {
            let h = harness();
            h.store.load();
            *h.fake.owned.lock().unwrap() = Err(answer);
            let result = h
                .store
                .save(SAM, Some(SecretString::from(KEY)))
                .await
                .unwrap();
            assert!(
                matches!(
                    result,
                    SteamTestResult::BadKey { .. } | SteamTestResult::Network
                ),
                "{result:?}"
            );
            assert!(!h.config().connected);
            assert!(h.keys.load().unwrap().is_none());
            assert!(h.rows().is_empty());
            assert!(h.messages().is_empty());
        }
    }

    #[tokio::test]
    async fn connecting_with_private_game_details_connects_with_an_empty_available() {
        let h = harness();
        h.store.load();
        *h.fake.owned.lock().unwrap() = Err(SteamApiError::Private);
        assert_eq!(
            h.store.save(SAM, Some(SecretString::from(KEY))).await,
            Ok(SteamTestResult::Private)
        );
        assert!(h.config().connected);
        let notices = h.notices();
        assert_eq!(
            notices[0].message,
            "Steam connected, but your game details are private, so Available is empty"
        );
        assert!(notices[0].error);
        let status = h.store.status();
        assert_eq!(status.steam.unwrap().key, SteamKeyState::Private);
        assert!(h.store.library().available.is_empty());
        assert_eq!(
            h.store.library().installed.len(),
            2,
            "installed games still show"
        );
        // A Refresh while still private says so.
        h.store.refresh_owned(true).await;
        assert_eq!(
            h.messages().last().unwrap(),
            "Steam still lists no games: the profile\u{2019}s game details are private"
        );
    }

    #[tokio::test]
    async fn save_without_a_key_keeps_the_saved_one() {
        let h = harness();
        h.store.load();
        h.connect().await;
        assert_eq!(
            h.store.save(GUEST, None).await,
            Ok(SteamTestResult::Ok { owned_count: 4 })
        );
        assert_eq!(h.config().steam_id, GUEST);
        assert_eq!(h.config().persona, "couchguest");
        assert!(h.keys.load().unwrap().is_some());
    }

    #[tokio::test]
    async fn refresh_reads_the_key_state_from_steams_answer() {
        let h = harness();
        h.store.load();
        h.connect().await;
        // No network keeps the list and the state.
        *h.fake.owned.lock().unwrap() = Err(SteamApiError::Network);
        h.store.refresh_owned(false).await;
        let status = h.store.status();
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.steam.as_ref().unwrap().key, SteamKeyState::Ok);
        assert_eq!(h.store.library().available.len(), 4);
        assert!(status.owned_error.contains("api.steampowered.com"));
        // A revoked key.
        *h.fake.owned.lock().unwrap() = Err(SteamApiError::Rejected { status: 401 });
        h.store.refresh_owned(false).await;
        let status = h.store.status();
        assert_eq!(status.steam.unwrap().key, SteamKeyState::Bad);
        assert_eq!(status.owned, OwnedState::Error);
        assert!(!status.owned_error.contains("401"));
        assert!(h.store.library().available.is_empty());
        // Back to working: a Refresh announces the list.
        *h.fake.owned.lock().unwrap() = Ok(OwnedGames {
            games: vec![owned(3_000_040, "Quiet Comet", 0)],
        });
        h.store.refresh_owned(true).await;
        assert_eq!(h.store.status().steam.unwrap().key, SteamKeyState::Ok);
        assert_eq!(
            h.messages().last().unwrap(),
            "Steam: 1 game found \u{b7} 1 to install"
        );
    }

    #[tokio::test]
    async fn startup_reads_the_caches_and_the_key_when_connected() {
        let h = harness();
        h.store.load();
        h.connect().await;
        // A new run on the same files.
        let shared = StoreShared::new();
        shared.set_registry(h.registry.clone());
        let again = SteamStore::new(
            h.fake.clone(),
            h.store.config_path.clone(),
            h.keys.clone(),
            shared,
        );
        again.load();
        let status = again.status();
        assert_eq!(status.owned, OwnedState::Ready);
        assert_eq!(status.owned_count, 4);
        assert_eq!(status.steam.unwrap().key, SteamKeyState::Ok);
        assert_eq!(again.library().available.len(), 4);
    }

    #[tokio::test]
    async fn remove_key_keeps_steam_connected_with_installed_games_only() {
        let h = harness();
        h.store.load();
        h.connect().await;
        h.store.remove_key().unwrap();
        assert!(h.keys.load().unwrap().is_none());
        assert!(h.config().connected);
        let status = h.store.status();
        assert_eq!(status.connection, StoreConnection::Connected);
        assert_eq!(status.steam.unwrap().key, SteamKeyState::None);
        assert_eq!(status.owned_count, 0);
        let library = h.store.library();
        assert!(library.available.is_empty());
        assert_eq!(library.installed.len(), 2);
        assert!(
            !h.dir.path().join("stores/cache").join(OWNED_CACHE).exists(),
            "the owned list went"
        );
        // A refresh with no key is the no-key state, no request.
        let calls = h.fake.owned_calls.load(Ordering::SeqCst);
        h.store.refresh_owned(true).await;
        assert_eq!(h.fake.owned_calls.load(Ordering::SeqCst), calls);
        assert_eq!(h.store.status().owned, OwnedState::Idle);
    }

    #[tokio::test]
    async fn a_missing_client_keeps_the_rows_but_shows_none() {
        let h = harness();
        h.store.load();
        h.connect().await;
        *h.fake.client.lock().unwrap() = None;
        h.store.recheck_client().await.unwrap();
        assert_eq!(h.rows().len(), 2, "rows stay for Check again");
        let status = h.store.status();
        assert_eq!(status.installed_count, 0);
        assert_eq!(status.steam.unwrap().client_path, "");
        assert!(h.store.library().installed.is_empty());
        assert_eq!(
            h.store.open("3000040", SteamOpenAction::Install),
            Err(STEAM_NOT_INSTALLED.to_string())
        );
    }

    #[tokio::test]
    async fn a_game_steam_removed_leaves_with_the_uninstalled_card() {
        let h = harness();
        h.store.load();
        h.connect().await;
        h.fake
            .manifests
            .lock()
            .unwrap()
            .retain(|m| m.appid != 3_000_020);
        h.store.scan_now().await;
        let ids: Vec<String> = h.rows().into_iter().map(|r| r.store_id).collect();
        assert_eq!(ids, ["3000010"]);
        assert_eq!(h.messages().last().unwrap(), "Deep Field Radio uninstalled");
        // A game installed in Steam itself shows on the next scan.
        h.fake
            .manifests
            .lock()
            .unwrap()
            .push(manifest(3_000_050, "Harbor of Bells", 4));
        h.store.scan_now().await;
        assert_eq!(h.rows().len(), 2);
        assert_eq!(h.store.library().available.len(), 3);
    }

    #[tokio::test]
    async fn install_in_steam_opens_the_link_and_watches_the_manifest() {
        let h = harness();
        h.store.load();
        h.connect().await;
        h.store.open("3000040", SteamOpenAction::Install).unwrap();
        assert_eq!(
            h.fake.opened.lock().unwrap().as_slice(),
            ["steam://install/3000040"]
        );
        let entry = || {
            h.install
                .snapshot()
                .entries
                .into_iter()
                .find(|e| e.source_id == "steam:3000040")
        };
        let row = entry().expect("the Downloads row");
        assert_eq!(row.kind, "store_external");
        assert_eq!(row.title, "Quiet Comet");
        // Steam writes the manifest, then the bytes come in.
        let mut m = manifest(3_000_040, "Quiet Comet", 1026);
        m.bytes_to_download = 100;
        m.bytes_downloaded = 52;
        h.fake.manifests.lock().unwrap().push(m);
        wait_for(
            || entry().is_some_and(|e| e.downloaded_bytes == 52),
            "progress",
        )
        .await;
        assert_eq!(entry().unwrap().total_bytes, 100);
        for m in h.fake.manifests.lock().unwrap().iter_mut() {
            if m.appid == 3_000_040 {
                m.flags = 4;
            }
        }
        wait_for(
            || entry().is_some_and(|e| e.status == DownloadStatus::Completed),
            "completion",
        )
        .await;
        assert_eq!(entry().unwrap().detail, INSTALLED_BY_STEAM);
        wait_for(
            || h.messages().iter().any(|m| m == "Quiet Comet installed"),
            "the card",
        )
        .await;
        let card = h
            .notices()
            .into_iter()
            .find(|n| n.message == "Quiet Comet installed")
            .unwrap();
        assert_eq!(card.action.unwrap().kind, "play");
        let library = h.store.library();
        let comet = library
            .installed
            .iter()
            .find(|g| g.store_id == "3000040")
            .unwrap();
        assert_eq!(comet.origin.as_deref(), Some("steam"), "GRID installed it");
        assert!(library.available.iter().all(|g| g.store_id != "3000040"));
    }

    #[tokio::test]
    async fn an_install_whose_manifest_goes_away_ends_cancelled() {
        let h = harness();
        h.store.load();
        h.connect().await;
        h.store.open("3000050", SteamOpenAction::Install).unwrap();
        h.fake
            .manifests
            .lock()
            .unwrap()
            .push(manifest(3_000_050, "Harbor of Bells", 1026));
        let entry = || {
            h.install
                .snapshot()
                .entries
                .into_iter()
                .find(|e| e.source_id == "steam:3000050")
        };
        wait_for(
            || {
                entry().is_some_and(|e| {
                    e.status == DownloadStatus::Downloading || e.downloaded_bytes == 0
                })
            },
            "the row",
        )
        .await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        h.fake
            .manifests
            .lock()
            .unwrap()
            .retain(|m| m.appid != 3_000_050);
        wait_for(
            || entry().is_some_and(|e| e.status == DownloadStatus::Cancelled),
            "cancelled",
        )
        .await;
        assert!(h.rows().iter().all(|r| r.store_id != "3000050"));
    }

    #[tokio::test]
    async fn an_install_steam_runs_by_itself_gets_a_downloads_row() {
        let h = harness();
        h.store.load();
        let mut m = manifest(3_000_060, "Rime Assembly", 1_048_576 | 1024);
        m.bytes_to_download = 10;
        m.bytes_downloaded = 3;
        h.fake.manifests.lock().unwrap().push(m);
        h.connect().await;
        let entries = h.install.snapshot().entries;
        let row = entries
            .iter()
            .find(|e| e.source_id == "steam:3000060")
            .expect("re-admitted");
        assert_eq!(row.kind, "store_external");
        assert!(h.rows().iter().all(|r| r.store_id != "3000060"));
    }

    #[tokio::test]
    async fn verify_and_uninstall_only_open_links() {
        let h = harness();
        h.store.load();
        h.connect().await;
        h.store.open("3000010", SteamOpenAction::Validate).unwrap();
        h.store.open("3000010", SteamOpenAction::Uninstall).unwrap();
        assert_eq!(
            h.fake.opened.lock().unwrap().as_slice(),
            ["steam://validate/3000010", "steam://uninstall/3000010"]
        );
        assert!(h.install.snapshot().entries.is_empty());
        // The uninstall watch notices the manifest go.
        h.fake
            .manifests
            .lock()
            .unwrap()
            .retain(|m| m.appid != 3_000_010);
        wait_for(
            || h.rows().iter().all(|r| r.store_id != "3000010"),
            "the row to go",
        )
        .await;
        assert!(h
            .messages()
            .iter()
            .any(|m| m == "Moonlit Ferry uninstalled"));
        assert_eq!(
            h.store.open("not-a-number", SteamOpenAction::Install),
            Err("That isn\u{2019}t a Steam game.".to_string())
        );
    }

    #[tokio::test]
    async fn disconnect_forgets_everything_but_the_games() {
        let h = harness();
        h.store.load();
        h.connect().await;
        h.store.open("3000070", SteamOpenAction::Install).unwrap();
        h.store.disconnect().unwrap();
        assert!(h.keys.load().unwrap().is_none());
        assert_eq!(h.config(), SteamConfig::default());
        assert!(h.rows().is_empty(), "rows only; the games stay in Steam");
        assert_eq!(h.cache_files().trim(), "");
        let status = h.store.status();
        assert_eq!(status.connection, StoreConnection::NotConnected);
        assert!(!status.in_rail);
        assert_eq!(status.steam.as_ref().unwrap().key, SteamKeyState::None);
        assert_eq!(
            status.steam.unwrap().client_path,
            "/usr/bin/steam",
            "the client stays found for Settings"
        );
        let watch = h
            .install
            .snapshot()
            .entries
            .into_iter()
            .find(|e| e.source_id == "steam:3000070")
            .unwrap();
        assert_eq!(watch.status, DownloadStatus::Cancelled);
        assert!(h.store.library().installed.is_empty());
        // No key in memory either: a test needs a key again.
        assert_eq!(
            h.store.test(SAM, None).await,
            Err(PASTE_KEY_FIRST.to_string())
        );
    }

    #[tokio::test]
    async fn launch_opens_rungameid_silently_and_watches_the_game() {
        let h = harness();
        h.store.load();
        h.connect().await;
        let launch = LaunchService::new(h.registry.clone(), h.store.config_path.clone());
        let key = GameKey::store(StoreKind::Steam, "3000010".to_string());
        let session = h
            .store
            .clone()
            .launch(key.clone(), launch.clone())
            .await
            .unwrap();
        assert_eq!(session.title, "Moonlit Ferry");
        assert_eq!(
            h.fake.opened.lock().unwrap().as_slice(),
            ["-silent steam://rungameid/3000010"]
        );
        // Not installed, and no client.
        let missing = GameKey::store(StoreKind::Steam, "3000040".to_string());
        assert!(matches!(
            h.store.clone().launch(missing, launch.clone()).await,
            Err(LaunchError::NotInstalled)
        ));
        *h.fake.client.lock().unwrap() = None;
        let other = GameKey::store(StoreKind::Steam, "3000020".to_string());
        match h.store.clone().launch(other, launch).await {
            Err(LaunchError::Validation(words)) => assert_eq!(words, STEAM_NOT_INSTALLED),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn steam_refuses_the_tool_and_login_paths() {
        let h = harness();
        assert_eq!(h.store.begin_login(), Err(CONNECTS_IN_SETTINGS.to_string()));
        assert!(h.store.clone().prepare_login().await.is_err());
        assert_eq!(h.store.ensure_can_install(), Err(STEAM_DOES_IT.to_string()));
        assert!(h.store.clone().install_options("620".into()).await.is_err());
        assert!(h
            .store
            .is_art_url("https://shared.akamai.steamstatic.com/a.jpg"));
        assert!(!h.store.is_art_url("https://evil.akamaihd.net/a.jpg"));
    }

    #[test]
    fn the_scan_plan_filters_tools_keeps_installs_and_marks_removals() {
        let apps: HashMap<u32, AppInfo> = [
            (3_000_010, app(3_000_010, "Moonlit Ferry", Some(0))),
            (3_000_080, app(3_000_080, "Desk Lights", Some(6))),
        ]
        .into_iter()
        .collect();
        let old = |id: &str, title: &str| InstalledGame {
            title: title.into(),
            source: grid_core::stores::GameSource::Store(StoreKind::Steam),
            store_id: id.into(),
            store_meta: SteamMeta {
                outside: false,
                ..Default::default()
            }
            .to_json(),
            last_played_at: 1_800_000_000,
            ..Default::default()
        };
        let mut unplugged_row = old("3000120", "Far Shore");
        unplugged_row.store_meta = SteamMeta {
            library: "/unplugged".into(),
            ..Default::default()
        }
        .to_json();
        let rows = vec![
            old("3000010", "Moonlit Ferry"),
            old("3000030", "Old Brick Road"),
            old("3000090", "Lantern Row"),
            old("3000080", "Desk Lights"),
            old("3000110", "Being Written"),
            unplugged_row,
        ];
        let manifests = vec![
            manifest(3_000_010, "Moonlit Ferry", 4),
            manifest(2_805_730, "Proton 9.0", 4),
            manifest(3_000_080, "Desk Lights", 4),
            manifest(3_000_090, "Lantern Row", 1026),
            manifest(3_000_100, "New One", 1_048_576),
            manifest(1_234_567, "SteamLinuxRuntime_sniper", 4),
        ];
        let owned = HashMap::new();
        let seen: HashSet<u32> = [3_000_030].into_iter().collect();
        let watched: HashSet<u32> = [3_000_090].into_iter().collect();
        let unreadable: HashSet<u32> = [3_000_110].into_iter().collect();
        let unplugged = vec![PathBuf::from("/unplugged")];
        let plan = plan_scan(&ScanInput {
            manifests: &manifests,
            unreadable: &unreadable,
            missing_libraries: &unplugged,
            rows: &rows,
            apps: &apps,
            owned: &owned,
            grid_installs: &HashSet::new(),
            seen: &seen,
            watched: &watched,
        });
        let upserts: Vec<(&str, bool)> = plan
            .upserts
            .iter()
            .map(|r| {
                (
                    r.title.as_str(),
                    SteamMeta::from_json(&r.store_meta).outside,
                )
            })
            .collect();
        assert_eq!(
            upserts,
            [("Moonlit Ferry", false)],
            "outside kept from the old row"
        );
        assert!(plan.touches.is_empty(), "last played only moves forward");
        let removals: Vec<(&str, bool)> = plan
            .removals
            .iter()
            .map(|r| (r.store_id.as_str(), r.card))
            .collect();
        assert_eq!(removals, [("3000030", true), ("3000080", false)]);
        let installing: Vec<u32> = plan.installing.iter().map(|(id, _)| *id).collect();
        assert_eq!(installing, [3_000_100], "Lantern Row is watched already");
        assert_eq!(plan.installed, [3_000_010]);
    }

    #[test]
    fn the_install_watch_steps() {
        // grid-core's `InstallWatch`, as the app drives it.
        let new = || InstallWatch::default().with_start_timeout(Duration::from_secs(60));
        let missing = ManifestRead::Missing;
        let mut w = new();
        let at = Duration::from_secs(1);
        assert_eq!(w.step(&missing, at), WatchEvent::Waiting);
        assert_eq!(
            w.step(&missing, Duration::from_secs(61)),
            WatchEvent::Cancelled,
            "the dialog was closed"
        );
        let mut w = new();
        let mut m = manifest(1, "A", 1026);
        m.bytes_to_download = 10;
        m.bytes_downloaded = 4;
        let found = |m: &ManifestState| ManifestRead::Found(m.clone());
        assert_eq!(w.step(&found(&m), at), WatchEvent::Progress(4, 10));
        assert_eq!(
            w.step(&ManifestRead::Unreadable, at),
            WatchEvent::Progress(4, 10),
            "one unreadable poll: Steam is writing it"
        );
        assert_eq!(w.step(&missing, at), WatchEvent::Waiting, "one miss");
        assert_eq!(w.step(&found(&m), at), WatchEvent::Progress(4, 10));
        assert_eq!(w.step(&missing, at), WatchEvent::Waiting);
        assert_eq!(w.step(&missing, at), WatchEvent::Cancelled);
        let mut w = new();
        m.flags = 4;
        assert_eq!(w.step(&found(&m), at), WatchEvent::Installed);
        m.flags = 6;
        assert_eq!(
            w.step(&found(&m), at),
            WatchEvent::Installed,
            "an update queued still counts as installed"
        );
        // An install Steam started by itself was seen already: gone ends it.
        let mut w = InstallWatch::resumed();
        assert_eq!(w.step(&missing, at), WatchEvent::Waiting);
        assert_eq!(w.step(&missing, at), WatchEvent::Cancelled);
    }

    #[test]
    fn caches_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        write_owned_cache(dir.path(), &[owned(1, "A", 5)], 42).unwrap();
        assert_eq!(read_owned_cache(dir.path()), (vec![owned(1, "A", 5)], 42));
        let apps: HashMap<u32, AppInfo> = [(1, app(1, "A", Some(0)))].into_iter().collect();
        write_apps_cache(dir.path(), &apps).unwrap();
        assert_eq!(read_apps_cache(dir.path()), apps);
        assert_eq!(read_owned_cache(&dir.path().join("none")), (Vec::new(), 0));
    }

    #[test]
    fn notices_are_the_designs_words() {
        assert_eq!(
            notice::connected(4).message,
            "Steam connected \u{b7} 4 games owned"
        );
        assert_eq!(
            notice::connected(1).message,
            "Steam connected \u{b7} 1 game owned"
        );
        assert!(notice::connected_private().error);
        assert_eq!(
            notice::installed("Quiet Comet", "steam:1").message,
            "Quiet Comet installed"
        );
        assert_eq!(
            notice::uninstalled("Quiet Comet").message,
            "Quiet Comet uninstalled"
        );
        assert_eq!(
            notice::games_found(4, 4).message,
            "Steam: 4 games found \u{b7} 4 to install"
        );
    }
}
