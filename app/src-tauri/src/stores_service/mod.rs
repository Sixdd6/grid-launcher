//! External game stores in the app. One backend per store, behind
//! [`StoreBackend`], the trait every store command calls:
//!
//! - [`gog`]: [`GogStore`], GOG through grid-core's `stores::gog` (Phase 1);
//!   its offline installers and drive backups stay GOG-only extras.
//! - [`epic`]: [`EpicStore`], Epic through grid-core's `stores::epic`
//!   (Phase 2; Step 0 stubs every call with `not implemented`).
//!
//! [`StoresService`] is the facade the app holds: it routes a store id to
//! its backend ([`StoresService::backend`]), assembles every store's
//! status (`stores-changed` carries them all, in the frontend's
//! `STORE_ORDER`), and fans out what concerns every store: startup, the
//! Downloads snapshots, the session-finished cloud upload, store art.
//! [`StoreShared`] is what the backends share with it: the emitter, the
//! registry and the art client.
//!
//! Secrets: a login code arrives as a [`SecretString`] and goes straight to
//! the backend's `finish_login`. Nothing here logs a token, a code, a URL
//! with a query, or a store tool's output; every error shown to the user
//! is GRID's own words.

pub mod epic;
pub mod gog;

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, OnceLock, RwLock, Weak};

use grid_core::config::Config;
use grid_core::library::queue::{DownloadsSnapshot, StoreJob};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::epic::EpicJobCtx;
use grid_core::stores::gog::GogJobCtx;
use grid_core::stores::{GameKey, StoreError, StoreKind, SyncMode};
use secrecy::SecretString;
use tauri::AppHandle;

use crate::commands::stores::{
    StoreCloudState, StoreConnection, StoreGameSettings, StoreInstallOptions, StoreInstallRequest,
    StoreLibrary, StoreLoginEvent, StoreNotice, StoreSaveConflict, StoreStatus,
};
use crate::store_auth::LoginSpec;

pub use epic::EpicStore;
pub use gog::{notice, GogStore};

/// A boxed, sendable future: the async methods of [`StoreBackend`] and the
/// store API seams.
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// --- events -------------------------------------------------------------------

/// What the stores tell the frontend.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreEvent {
    Changed(Vec<StoreStatus>),
    Login(StoreLoginEvent),
    Notice(StoreNotice),
}

pub type Emitter = Arc<dyn Fn(StoreEvent) + Send + Sync>;

/// What every backend needs for a store job (`GogJobCtx` / `EpicJobCtx`
/// have the same parts).
#[derive(Clone)]
pub struct JobCtx {
    pub registry: Arc<Registry>,
    pub library: PathBuf,
    pub default_compat_tool: String,
}

impl JobCtx {
    pub fn gog(&self) -> GogJobCtx {
        GogJobCtx {
            registry: self.registry.clone(),
            library: self.library.clone(),
            default_compat_tool: self.default_compat_tool.clone(),
        }
    }

    pub fn epic(&self) -> EpicJobCtx {
        EpicJobCtx {
            registry: self.registry.clone(),
            library: self.library.clone(),
            default_compat_tool: self.default_compat_tool.clone(),
        }
    }
}

// --- the backend trait -----------------------------------------------------------

/// One store as the commands see it: every method a store command uses.
/// `Err(String)` is a message for the user (GRID's words). GOG-only extras
/// (offline installers, drive backups) stay on [`GogStore`].
pub trait StoreBackend: Send + Sync {
    fn kind(&self) -> StoreKind;
    /// The store's card: connection, tool, owned list, counts.
    fn status(&self) -> StoreStatus;
    fn connection(&self) -> StoreConnection;
    /// Emits `stores-changed` with every store's status.
    fn changed(&self);

    // startup and background
    /// Startup: the tool, the cached owned list, the config's connection.
    fn load(&self);
    /// Whether the saved login still works; refreshes the owned list.
    fn run_login_check(self: Arc<Self>) -> BoxFut<'static, ()>;
    /// Every Downloads snapshot: finished-job cards, counts.
    fn observe_downloads(&self, snapshot: &DownloadsSnapshot);

    // login
    /// What the login window opens and which redirect ends it.
    fn login_spec(&self) -> LoginSpec;
    /// The login window is opening. `Err` while a sign-in is finishing.
    fn begin_login(&self) -> Result<(), String>;
    fn login_window_failed(&self, error: &str);
    /// ✕, Cancel or Esc: silent.
    fn login_cancelled(&self);
    /// The finish redirect carried no code.
    fn login_redirect_failed(&self);
    /// The finish redirect's code: signs in, installs the tool, loads the
    /// owned list.
    fn run_finish_login(self: Arc<Self>, code: SecretString) -> BoxFut<'static, ()>;
    fn disconnect(&self) -> Result<(), String>;

    // tool, owned list, updates
    fn run_install_tool(self: Arc<Self>) -> BoxFut<'static, ()>;
    /// Reloads the owned list (`announce`: the "N games found" card).
    fn run_refresh(self: Arc<Self>, announce: bool) -> BoxFut<'static, ()>;
    /// Installed and Available. `updates`: newer versions by store id.
    fn library(&self, updates: &HashMap<String, String>) -> StoreLibrary;
    /// Starts the store's update check; results come as events.
    fn check_updates(self: Arc<Self>, app: AppHandle);

    // store settings
    fn set_install_dir(&self, path: &str);
    fn set_cloud_saves(&self, on: bool);

    // installs
    /// Refuses an install while signed out or without the tool.
    fn ensure_can_install(&self) -> Result<(), String>;
    /// What the install dialog offers for `store_id`.
    fn install_options(
        self: Arc<Self>,
        store_id: String,
    ) -> BoxFut<'static, Result<StoreInstallOptions, String>>;
    /// The job for a confirmed install of `store_id`.
    fn install_job(
        self: Arc<Self>,
        store_id: String,
        request: StoreInstallRequest,
        ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>>;
    fn update_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob;
    fn repair_job(&self, row: &InstalledGame, ctx: JobCtx) -> StoreJob;
    fn dlc_job(
        self: Arc<Self>,
        row: InstalledGame,
        dlc_id: String,
        ctx: JobCtx,
    ) -> BoxFut<'static, Result<StoreJob, String>>;
    fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc_id: &str,
    ) -> Result<(), StoreError>;
    fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError>;

    // game settings
    fn game_settings(&self, row: &InstalledGame) -> StoreGameSettings;
    /// The row after Game Settings' Save.
    fn apply_game_settings(
        &self,
        row: InstalledGame,
        settings: &StoreGameSettings,
    ) -> InstalledGame;
    /// Whether the installed version is the Windows one (Game Settings'
    /// executable list).
    fn windows_version(&self, row: &InstalledGame) -> bool;

    // cloud saves
    /// The store switch and the game's own.
    fn cloud_enabled(&self, row: &InstalledGame) -> bool;
    fn cloud_state<'a>(
        &'a self,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<StoreCloudState, String>>;
    /// Syncs in `mode` and answers the new state (`off` when cloud saves
    /// are off for the game).
    fn cloud_sync<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
        mode: SyncMode,
    ) -> BoxFut<'a, Result<StoreCloudState, String>>;
    /// Before a launch: `Err(conflict)` stops it; any other failure only
    /// logs.
    fn sync_before_launch<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, Result<(), StoreSaveConflict>>;
    /// After a session: the upload, and its card.
    fn sync_after_session<'a>(
        &'a self,
        registry: &'a Registry,
        row: &'a InstalledGame,
    ) -> BoxFut<'a, ()>;

    // art
    /// Whether `url` is this store's art the UI may load.
    fn is_art_url(&self, url: &str) -> bool;
}

// --- what the backends share --------------------------------------------------------

/// The emitter, the registry and the art client every backend uses, and
/// the backends themselves (weakly), so any backend's `changed()` emits
/// every store's status.
#[derive(Default)]
pub struct StoreShared {
    emitter: RwLock<Option<Emitter>>,
    registry: RwLock<Option<Arc<Registry>>>,
    backends: RwLock<Vec<Weak<dyn StoreBackend>>>,
    /// The client store art is fetched with (no credential, ever).
    art_http: OnceLock<reqwest::Client>,
}

impl StoreShared {
    pub fn new() -> Arc<Self> {
        Arc::new(StoreShared::default())
    }

    /// Adds a backend to [`Self::statuses`], in call order.
    pub fn register(&self, backend: Arc<dyn StoreBackend>) {
        self.backends
            .write()
            .unwrap()
            .push(Arc::downgrade(&backend));
    }

    pub fn set_emitter(&self, emitter: Emitter) {
        *self.emitter.write().unwrap() = Some(emitter);
    }

    pub fn set_registry(&self, registry: Arc<Registry>) {
        *self.registry.write().unwrap() = Some(registry);
    }

    pub fn registry(&self) -> Option<Arc<Registry>> {
        self.registry.read().unwrap().clone()
    }

    pub fn emit(&self, event: StoreEvent) {
        let emitter = self.emitter.read().unwrap().clone();
        if let Some(emitter) = emitter {
            emitter(event);
        }
    }

    /// Every registered store's status, in registration order.
    pub fn statuses(&self) -> Vec<StoreStatus> {
        let backends: Vec<Arc<dyn StoreBackend>> = self
            .backends
            .read()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        backends.iter().map(|backend| backend.status()).collect()
    }

    /// Emits `stores-changed` with every store's status.
    pub fn changed(&self) {
        self.emit(StoreEvent::Changed(self.statuses()));
    }

    /// The cached copy of a store art `url`, fetched with no credential on
    /// a miss. The caller checks the URL first.
    pub async fn art(&self, data_dir: &Path, url: &str) -> Option<PathBuf> {
        let http = self.art_http.get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("grid-launcher/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default()
        });
        grid_core::stores::art::cached(http, data_dir, url).await
    }
}

// --- the facade ------------------------------------------------------------------

/// The external stores: GOG and Epic.
pub struct StoresService {
    shared: Arc<StoreShared>,
    data_dir: PathBuf,
    pub gog: Arc<GogStore>,
    pub epic: Arc<EpicStore>,
}

impl StoresService {
    /// The facade over `gog` and `epic`, both registered on `shared` in the
    /// frontend's `STORE_ORDER` (amazon, epic, gog, steam).
    pub fn new(
        shared: Arc<StoreShared>,
        data_dir: PathBuf,
        gog: Arc<GogStore>,
        epic: Arc<EpicStore>,
    ) -> Arc<Self> {
        shared.register(epic.clone());
        shared.register(gog.clone());
        Arc::new(StoresService {
            shared,
            data_dir,
            gog,
            epic,
        })
    }

    /// The real stores for this run.
    pub fn real(config_path: PathBuf) -> Arc<Self> {
        let data_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let shared = StoreShared::new();
        let http = reqwest::Client::builder()
            .user_agent(concat!("grid-launcher/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        let gog = GogStore::real(
            data_dir.clone(),
            config_path.clone(),
            http.clone(),
            shared.clone(),
        );
        let epic = EpicStore::real(data_dir.clone(), config_path, http, shared.clone());
        StoresService::new(shared, data_dir, gog, epic)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn set_emitter(&self, emitter: Emitter) {
        self.shared.set_emitter(emitter);
    }

    pub fn set_registry(&self, registry: Arc<Registry>) {
        self.shared.set_registry(registry);
    }

    pub fn registry(&self) -> Option<Arc<Registry>> {
        self.shared.registry()
    }

    /// Every store's status (`stores_status`, `stores-changed`).
    pub fn statuses(&self) -> Vec<StoreStatus> {
        self.shared.statuses()
    }

    /// The backend for `kind`; `None` for a store GRID doesn't support yet
    /// (Amazon, Steam).
    pub fn backend(&self, kind: StoreKind) -> Option<Arc<dyn StoreBackend>> {
        match kind {
            StoreKind::Gog => Some(self.gog.clone()),
            StoreKind::Epic => Some(self.epic.clone()),
            StoreKind::Amazon | StoreKind::Steam => None,
        }
    }

    fn backends(&self) -> [Arc<dyn StoreBackend>; 2] {
        [self.epic.clone(), self.gog.clone()]
    }

    /// Startup: every store's tool, cache and connection.
    pub fn load(&self) {
        for backend in self.backends() {
            backend.load();
        }
    }

    /// Every store's login check (and owned-list refresh when signed in).
    pub async fn check_login(&self) {
        for backend in self.backends() {
            backend.run_login_check().await;
        }
    }

    /// Every Downloads snapshot, to every store (each reads its own rows).
    pub fn observe_downloads(&self, snapshot: &DownloadsSnapshot) {
        for backend in self.backends() {
            backend.observe_downloads(snapshot);
        }
    }

    /// Whether `url` is store art the UI may load (any store's).
    pub fn is_art_url(&self, url: &str) -> bool {
        self.backends()
            .iter()
            .any(|backend| backend.is_art_url(url))
    }

    /// The cached copy of a store art `url`, fetched with no credential on
    /// a miss; `None` for an address that isn't store art or a failed fetch.
    pub async fn art(&self, url: &str) -> Option<PathBuf> {
        if !self.is_art_url(url) {
            return None;
        }
        self.shared.art(&self.data_dir, url).await
    }

    /// After a store game's session ends: its backend uploads the cloud
    /// saves (a listener on the launch service's session-finished hook,
    /// beside the RomM cloud upload, which skips store sessions).
    pub fn install_session_finished_hook(
        self: &Arc<Self>,
        launch: &grid_core::launch::LaunchService,
    ) {
        let this = self.clone();
        launch.add_session_finished_hook(Arc::new(move |session| {
            let Some(kind) = session.key.store_kind() else {
                return;
            };
            let Some(backend) = this.backend(kind) else {
                return;
            };
            let Some(registry) = this.registry() else {
                return;
            };
            let key: GameKey = session.key.clone();
            tauri::async_runtime::spawn(async move {
                let lookup = registry.clone();
                let row = tokio::task::spawn_blocking(move || lookup.find_key(&key)).await;
                if let Ok(Ok(Some(row))) = row {
                    backend.sync_after_session(&registry, &row).await;
                }
            });
        }));
    }

    /// Installs the stores' launch hook (Epic's launch ticket) on `launch`.
    pub fn install_launch_hook(&self, launch: &grid_core::launch::LaunchService) {
        launch.set_store_launch_hook(self.epic.clone());
    }
}

/// The config's library folder joined with `name` (`<library>/GOG`,
/// `<library>/Epic Games`), `""` when no library folder is set.
pub fn default_install_dir(config_path: &Path, configured: &str, name: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.to_string();
    }
    Config::load(config_path)
        .ok()
        .and_then(|config| grid_core::library::paths::library_root(&config))
        .map(|root| root.join(name).to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facade() -> (Arc<StoresService>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        Config::default().save(&config_path).unwrap();
        let stores = StoresService::real(config_path);
        (stores, dir)
    }

    #[test]
    fn statuses_come_in_store_order_and_backends_route() {
        let (stores, _dir) = facade();
        let sources: Vec<String> = stores.statuses().into_iter().map(|s| s.source).collect();
        assert_eq!(sources, ["epic", "gog"]);
        assert_eq!(
            stores.backend(StoreKind::Gog).unwrap().kind(),
            StoreKind::Gog
        );
        assert_eq!(
            stores.backend(StoreKind::Epic).unwrap().kind(),
            StoreKind::Epic
        );
        assert!(stores.backend(StoreKind::Steam).is_none());
        assert!(stores.backend(StoreKind::Amazon).is_none());
    }

    #[test]
    fn any_backends_change_emits_every_status() {
        let (stores, _dir) = facade();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        stores.set_emitter(Arc::new(move |event| sink.lock().unwrap().push(event)));
        stores.epic.changed();
        let events = events.lock().unwrap();
        match events.as_slice() {
            [StoreEvent::Changed(statuses)] => assert_eq!(statuses.len(), 2),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn art_urls_are_any_stores() {
        let (stores, _dir) = facade();
        assert!(stores.is_art_url("https://images.gog-statics.com/a.jpg"));
        assert!(stores.is_art_url("https://cdn1.epicgames.com/a.jpg"));
        assert!(!stores.is_art_url("https://romm.example/a.jpg"));
    }
}
