//! GOG: login and the owned library through GOG's HTTP APIs, install,
//! update, repair and DLC through heroic-gogdl, and GOG cloud saves.
//! `goggame`, `meta` and `row` are shared with the offline installers and
//! the drive backups; the rest is package A's (`docs/stores-phase1-plan.md`).
//!
//! Secrets: the login code and the bearer token live only in memory as
//! [`SecretString`]; gogdl keeps its own token file under
//! `<data_dir>/stores/gogdl/` (the store-token exception in AGENTS.md).

pub mod goggame;
pub mod meta;
pub mod row;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use secrecy::SecretString;
use url::Url;

use super::{StoreError, StoreKind};
use crate::library::queue::{StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use meta::GogPlatform;

/// The GOG hosts GRID talks to. Tests and e2e point them at a mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GogEndpoints {
    pub auth: Url,
    pub embed: Url,
    pub api: Url,
    pub content: Url,
    pub cloud: Url,
}

impl GogEndpoints {
    /// GOG's own hosts.
    pub fn real() -> GogEndpoints {
        let url = |text: &str| Url::parse(text).unwrap_or_else(|_| unreachable!("{text}"));
        GogEndpoints {
            auth: url("https://auth.gog.com/"),
            embed: url("https://embed.gog.com/"),
            api: url("https://api.gog.com/"),
            content: url("https://content-system.gog.com/"),
            cloud: url("https://cloudstorage.gog.com/"),
        }
    }

    /// Every host at one mock server (e2e, tests).
    pub fn e2e(base: Url) -> GogEndpoints {
        GogEndpoints {
            auth: base.clone(),
            embed: base.clone(),
            api: base.clone(),
            content: base.clone(),
            cloud: base,
        }
    }
}

/// The signed-in GOG account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogAccount {
    pub username: String,
    pub user_id: String,
}

/// One owned GOG product, as the owned list and product APIs give it. DLC
/// is folded into its base game's `dlc`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogProduct {
    pub id: String,
    pub title: String,
    /// Cover art URL.
    pub cover: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub developer: String,
    pub publisher: String,
    pub genres: Vec<String>,
    /// `YYYY-MM-DD`, or `""`.
    pub release_date: String,
    /// The versions GOG offers (Phase 1 installs Linux and Windows only).
    pub platforms: Vec<GogPlatform>,
    pub dlc: Vec<GogProductDlc>,
}

/// One DLC of an owned product.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogProductDlc {
    pub id: String,
    pub title: String,
}

/// What `gogdl info` offers for one product.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogInstallOptions {
    pub versions: Vec<GogVersionOption>,
    pub languages: Vec<String>,
    pub default_language: String,
    pub dlc: Vec<GogDlcOption>,
}

/// One installable version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GogVersionOption {
    pub platform: GogPlatform,
    pub download_bytes: u64,
    pub disk_bytes: u64,
}

/// One installable DLC.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogDlcOption {
    pub id: String,
    pub title: String,
    pub download_bytes: u64,
}

/// An install the user confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GogInstallRequest {
    pub store_id: String,
    pub title: String,
    pub platform: GogPlatform,
    pub language: String,
    pub dlc_ids: Vec<String>,
    /// The game's own folder.
    pub install_dir: PathBuf,
    /// Windows games on Linux; `""` for the default.
    pub compat_tool: String,
}

/// A newer build GOG has for an installed game.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogUpdate {
    pub store_id: String,
    pub version: String,
    pub build_id: String,
}

/// What every GOG job needs from the app.
#[derive(Clone)]
pub struct GogJobCtx {
    pub registry: Arc<Registry>,
    pub library: PathBuf,
    pub default_compat_tool: String,
}

/// How a cloud save sync decides its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// GRID decides from timestamps; a conflict stops the sync.
    Auto,
    PreferUpload,
    PreferDownload,
}

/// A game's cloud save state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudState {
    /// Cloud saves are off for the game (or GOG has none for it).
    Off,
    /// On, never synced.
    None,
    Synced {
        at: i64,
    },
    Conflict(SaveConflict),
}

/// Both sides of a save conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveConflict {
    pub local_at: i64,
    pub local_files: u32,
    pub local_bytes: u64,
    pub cloud_at: i64,
}

/// The GOG service: cheap to clone, shared by the app's commands.
#[derive(Clone)]
pub struct GogService {
    data_dir: PathBuf,
    http: reqwest::Client,
    ep: GogEndpoints,
}

impl GogService {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: GogEndpoints) -> Self {
        GogService { data_dir, http, ep }
    }

    /// The login page the login window opens.
    pub fn login_url(&self) -> Url {
        self.ep.auth.clone()
    }

    /// The login code in GOG's redirect
    /// (`embed.gog.com/on_login_success?code=`), `None` for any other URL.
    pub fn code_from_redirect(&self, url: &Url) -> Option<SecretString> {
        let _ = url;
        None
    }

    /// Signs in with the code (`gogdl auth --code`).
    pub async fn complete_login(&self, code: SecretString) -> Result<GogAccount, StoreError> {
        let _ = code;
        Err(StoreError::not_implemented())
    }

    /// The signed-in account, `None` when signed out;
    /// [`StoreError::LoginExpired`] when the refresh fails.
    pub async fn account(&self) -> Result<Option<GogAccount>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// Signs out: deletes gogdl's folder (its token file with it).
    pub fn disconnect(&self) -> io::Result<()> {
        let _ = (&self.data_dir, &self.http);
        Err(io::Error::other(super::NOT_IMPLEMENTED))
    }

    /// The owned games, DLC folded into their base game.
    pub async fn owned(&self) -> Result<Vec<GogProduct>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// What can be installed for `id` (`gogdl info`).
    pub async fn install_options(&self, id: &str) -> Result<GogInstallOptions, StoreError> {
        let _ = id;
        Err(StoreError::not_implemented())
    }

    pub fn install_job(&self, req: GogInstallRequest, ctx: GogJobCtx) -> StoreJob {
        let _ = ctx;
        unimplemented_job(req.store_id, StoreOp::Install, req.title)
    }

    pub fn update_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob {
        let _ = ctx;
        unimplemented_job(row.store_id.clone(), StoreOp::Update, row.title.clone())
    }

    pub fn repair_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob {
        let _ = ctx;
        unimplemented_job(row.store_id.clone(), StoreOp::Repair, row.title.clone())
    }

    pub fn dlc_job(&self, row: &InstalledGame, dlc_id: &str, ctx: GogJobCtx) -> StoreJob {
        let _ = (dlc_id, ctx);
        unimplemented_job(row.store_id.clone(), StoreOp::Dlc, row.title.clone())
    }

    pub fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc_id: &str,
    ) -> Result<(), StoreError> {
        let _ = (registry, row, dlc_id);
        Err(StoreError::not_implemented())
    }

    pub fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
        let _ = (registry, store_id);
        Err(StoreError::not_implemented())
    }

    /// Newer builds for `rows` (content-system builds against
    /// `meta.build_id`).
    pub async fn check_updates(
        &self,
        rows: &[InstalledGame],
    ) -> Result<Vec<GogUpdate>, StoreError> {
        let _ = rows;
        Err(StoreError::not_implemented())
    }

    pub async fn cloud_state(&self, row: &InstalledGame) -> Result<CloudState, StoreError> {
        let _ = row;
        Err(StoreError::not_implemented())
    }

    pub async fn cloud_sync(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        mode: SyncMode,
    ) -> Result<CloudState, StoreError> {
        let _ = (registry, row, mode);
        Err(StoreError::not_implemented())
    }
}

/// A Step 0 job whose runner fails "not implemented".
fn unimplemented_job(store_id: String, op: StoreOp, title: String) -> StoreJob {
    StoreJob {
        source: StoreKind::Gog,
        store_id,
        op,
        title,
        drive: None,
        runner: Arc::new(Unimplemented),
    }
}

struct Unimplemented;

impl StoreJobRunner for Unimplemented {
    fn download(
        &self,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Extract(super::NOT_IMPLEMENTED.to_string()))
    }

    fn finalize(&self, _sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        Err(LibraryError::Extract(super::NOT_IMPLEMENTED.to_string()))
    }
}
