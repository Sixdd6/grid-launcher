//! Epic Games Store, through legendary 0.21.1 (`docs/stores-phase2-plan.md`).
//!
//! Step 0 holds the contracts every work package builds on: the types, the
//! `store_meta` JSON ([`meta`]), the registry row ([`row`]) and the
//! [`EpicService`] surface. The service's bodies are the work packages'
//! (A: login, owned list, install, update, repair, uninstall; B: launch
//! prep and cloud saves); until they land every call fails with
//! [`StoreError::not_implemented`].
//!
//! Secrets: the login code arrives as a [`SecretString`] and reaches
//! legendary on stdin only (`runner::ToolCommand::stdin`), never argv.
//! legendary keeps its own token files under `<data_dir>/stores/legendary/`
//! (the store-token exception in AGENTS.md), made owner-only after every
//! run. The exchange code `launch --json` prints is read in `launch.rs`
//! only and travels as a redacted launch argument.

pub mod meta;
pub mod row;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use secrecy::SecretString;
use serde::{Deserialize, Deserializer};
use url::Url;

use super::paths;
use super::tools::StoreTool;
use super::{CloudState, StoreError, StoreKind, SyncMode};
use crate::launch::StoreLaunchPrep;
use crate::library::queue::{StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use meta::EpicMeta;

/// The user agent the login window shows Epic, so Epic ends the sign-in
/// with the launcher redirect that carries the code (Heroic's way).
pub const EPIC_LOGIN_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) EpicGamesLauncher";

/// The query parameter of [`EpicEndpoints::finish`] that carries the code.
pub const EPIC_CODE_PARAM: &str = "code";

/// The Epic addresses GRID uses. Tests and e2e point them at a mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicEndpoints {
    /// The sign-in page the login window opens.
    pub login: Url,
    /// Where Epic sends the browser when the sign-in ends:
    /// `https://localhost/launcher/authorized?code=…`. The login window
    /// matches scheme, host, port and path, and refuses the navigation.
    pub finish: Url,
    /// Origins besides Epic's CDNs that may serve art (the e2e mock).
    pub art_origins: Vec<Url>,
}

impl EpicEndpoints {
    /// Epic's own addresses.
    pub fn real() -> EpicEndpoints {
        let url = |text: &str| Url::parse(text).unwrap_or_else(|_| unreachable!("{text}"));
        EpicEndpoints {
            login: url("https://www.epicgames.com/id/login?responseType=code"),
            finish: url("https://localhost/launcher/authorized"),
            art_origins: Vec::new(),
        }
    }

    /// Everything at one mock server (e2e, tests): `<base>/id/login`,
    /// `<base>/launcher/authorized`, art from `<base>`.
    pub fn e2e(base: Url) -> EpicEndpoints {
        let join = |path: &str| base.join(path).unwrap_or_else(|_| base.clone());
        EpicEndpoints {
            login: join("id/login?responseType=code"),
            finish: join("launcher/authorized"),
            art_origins: vec![base.clone()],
        }
    }
}

/// The signed-in Epic account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpicAccount {
    /// Epic's display name.
    pub username: String,
}

/// One owned Epic game from `list --json -T`, DLC folded into its base.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpicGame {
    pub app_name: String,
    pub title: String,
    /// Cover art URL (keyImages, in the order the plan's Research names).
    pub cover: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub developer: String,
    /// The release year, or `""`.
    pub year: String,
    /// `asset_infos.Windows.build_version`: an installed game whose
    /// `meta.version` differs has an update.
    pub build_version: String,
    /// The launcher a third-party-managed title needs ("the EA app",
    /// "Ubisoft Connect"); GRID shows it as Not supported.
    pub third_party: Option<String>,
    /// `customAttributes.CloudSaveFolder`, `""` for none.
    pub cloud_save_folder: String,
    pub can_run_offline: bool,
    pub dlc: Vec<EpicGameDlc>,
}

/// One owned DLC.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpicGameDlc {
    pub app_name: String,
    pub title: String,
}

/// One `list-installed --json` entry (legendary's `InstalledGame`; the
/// fields GRID reads). Unknown keys are ignored and a `null` reads as the
/// default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct EpicInstalled {
    #[serde(deserialize_with = "nullable")]
    pub app_name: String,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    #[serde(deserialize_with = "nullable")]
    pub version: String,
    #[serde(deserialize_with = "nullable")]
    pub install_path: String,
    #[serde(deserialize_with = "nullable")]
    pub install_size: u64,
    /// Relative to `install_path`, either separator.
    #[serde(deserialize_with = "nullable")]
    pub executable: String,
    #[serde(deserialize_with = "nullable")]
    pub launch_parameters: String,
    #[serde(deserialize_with = "nullable")]
    pub is_dlc: bool,
    #[serde(deserialize_with = "nullable")]
    pub can_run_offline: bool,
    #[serde(deserialize_with = "nullable")]
    pub save_path: String,
}

/// A value that may be JSON `null`, read as its default.
fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// What `info --json` says an install takes: the base game's sizes and each
/// owned DLC with its download size.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpicInstallOptions {
    pub download_bytes: u64,
    pub disk_bytes: u64,
    pub dlc: Vec<(EpicGameDlc, u64)>,
}

/// An install the user confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicInstallRequest {
    pub app_name: String,
    pub title: String,
    /// The DLC app names to install with it.
    pub dlc: Vec<String>,
    /// The game's own folder.
    pub install_dir: PathBuf,
    /// `""` for the default.
    pub compat_tool: String,
}

/// A newer build Epic has for an installed game.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpicUpdate {
    pub app_name: String,
    /// The owned list's `build_version`.
    pub version: String,
}

/// What every Epic job needs from the app.
#[derive(Clone)]
pub struct EpicJobCtx {
    pub registry: Arc<Registry>,
    pub library: PathBuf,
    pub default_compat_tool: String,
}

/// The Epic service: cheap to clone, shared by the app's commands.
#[derive(Clone)]
pub struct EpicService {
    data_dir: PathBuf,
    #[allow(dead_code)] // package A: legendary's install and art
    http: reqwest::Client,
    ep: EpicEndpoints,
}

impl EpicService {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: EpicEndpoints) -> Self {
        EpicService { data_dir, http, ep }
    }

    /// The sign-in page the login window opens.
    pub fn login_url(&self) -> Url {
        self.ep.login.clone()
    }

    /// The addresses this service uses.
    pub fn endpoints(&self) -> &EpicEndpoints {
        &self.ep
    }

    /// `<data_dir>/stores/legendary`: legendary's config folder
    /// (`LEGENDARY_CONFIG_PATH`).
    pub fn tool_dir(&self) -> PathBuf {
        paths::tool_dir(&self.data_dir, StoreTool::Legendary)
    }

    /// Signs in with the code: `auth --disable-webview`, the code on stdin
    /// (package A).
    pub async fn complete_login(&self, code: SecretString) -> Result<EpicAccount, StoreError> {
        drop(code);
        Err(StoreError::not_implemented())
    }

    /// The signed-in account, `None` when signed out;
    /// [`StoreError::LoginExpired`] when Epic refuses the saved login
    /// (package A: `status --json`).
    pub async fn account(&self) -> Result<Option<EpicAccount>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// Signs out: legendary's folder (its token files with it) goes.
    /// Package A runs `auth --delete` first.
    pub fn disconnect(&self) -> io::Result<()> {
        paths::remove_tool_dir(&self.data_dir, StoreTool::Legendary)
    }

    /// The owned games, DLC folded into their base (package A:
    /// `list --json -T`).
    pub async fn owned(&self) -> Result<Vec<EpicGame>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// What legendary has installed (package A: `list-installed --json`).
    pub async fn installed(&self) -> Result<Vec<EpicInstalled>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// The base game's and its DLC's sizes (package A: `info --json`).
    pub async fn install_options(&self, app: &str) -> Result<EpicInstallOptions, StoreError> {
        let _ = app;
        Err(StoreError::not_implemented())
    }

    pub fn install_job(&self, req: EpicInstallRequest, ctx: EpicJobCtx) -> StoreJob {
        let _ = ctx;
        not_implemented_job(req.app_name, req.title, StoreOp::Install)
    }

    pub fn update_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob {
        let _ = ctx;
        not_implemented_job(row.store_id.clone(), row.title.clone(), StoreOp::Update)
    }

    pub fn repair_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob {
        let _ = ctx;
        not_implemented_job(row.store_id.clone(), row.title.clone(), StoreOp::Repair)
    }

    pub fn dlc_job(&self, row: &InstalledGame, dlc: &str, ctx: EpicJobCtx) -> StoreJob {
        let _ = (dlc, ctx);
        not_implemented_job(row.store_id.clone(), row.title.clone(), StoreOp::Dlc)
    }

    pub fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc: &str,
    ) -> Result<(), StoreError> {
        let _ = (registry, row, dlc);
        Err(StoreError::not_implemented())
    }

    pub fn uninstall(&self, registry: &Registry, app: &str) -> Result<(), StoreError> {
        let _ = (registry, app);
        Err(StoreError::not_implemented())
    }

    /// The installed games the owned list has a different build for. Pure:
    /// `build_version` against each row's `meta.version`; a row with no
    /// version, or a game with no build version, never shows an update.
    pub fn updates(&self, rows: &[InstalledGame], owned: &[EpicGame]) -> Vec<EpicUpdate> {
        updates(rows, owned)
    }

    /// The launch ticket: `launch --json`, its `egl_parameters` as
    /// redacted arguments (package B).
    pub async fn launch_prep(
        &self,
        row: &InstalledGame,
        offline: bool,
    ) -> Result<StoreLaunchPrep, StoreError> {
        let _ = (row, offline);
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

/// [`EpicService::updates`]: the pure comparison.
pub fn updates(rows: &[InstalledGame], owned: &[EpicGame]) -> Vec<EpicUpdate> {
    rows.iter()
        .filter_map(|row| {
            let meta = EpicMeta::from_json(&row.store_meta);
            let installed = meta.version.trim();
            let game = owned.iter().find(|game| game.app_name == row.store_id)?;
            let latest = game.build_version.trim();
            (!installed.is_empty() && !latest.is_empty() && installed != latest).then(|| {
                EpicUpdate {
                    app_name: row.store_id.clone(),
                    version: latest.to_string(),
                }
            })
        })
        .collect()
}

/// A job that fails with [`StoreError::not_implemented`] (Step 0).
fn not_implemented_job(store_id: String, title: String, op: StoreOp) -> StoreJob {
    StoreJob {
        source: StoreKind::Epic,
        store_id,
        op,
        title,
        drive: None,
        runner: Arc::new(NotImplemented),
    }
}

struct NotImplemented;

impl StoreJobRunner for NotImplemented {
    fn download(
        &self,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Extract(
            StoreError::not_implemented().to_string(),
        ))
    }

    fn finalize(&self, _sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        Err(LibraryError::Extract(
            StoreError::not_implemented().to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_and_e2e_endpoints() {
        let real = EpicEndpoints::real();
        assert_eq!(real.login.host_str(), Some("www.epicgames.com"));
        assert_eq!(
            real.finish.as_str(),
            "https://localhost/launcher/authorized"
        );
        assert!(real.art_origins.is_empty());
        let base = Url::parse("http://127.0.0.1:4300/").unwrap();
        let e2e = EpicEndpoints::e2e(base.clone());
        assert_eq!(
            e2e.login.as_str(),
            "http://127.0.0.1:4300/id/login?responseType=code"
        );
        assert_eq!(
            e2e.finish.as_str(),
            "http://127.0.0.1:4300/launcher/authorized"
        );
        assert_eq!(e2e.art_origins, vec![base]);
    }

    #[test]
    fn list_installed_entries_parse_with_nulls() {
        let json = r#"[{"app_name": "Starlight", "install_path": "/g/S", "title": "Starlight Courier",
            "version": "1.0.2", "base_urls": [], "can_run_offline": true, "egl_guid": "",
            "executable": "Binaries/Win64/S.exe", "install_size": 1234, "install_tags": [],
            "is_dlc": false, "launch_parameters": "", "manifest_path": "", "needs_verification": false,
            "platform": "Windows", "prereq_info": null, "uninstaller": null, "requires_ot": false,
            "save_path": null, "is_preloaded": false}]"#;
        let rows: Vec<EpicInstalled> = serde_json::from_str(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].app_name, "Starlight");
        assert_eq!(rows[0].install_size, 1234);
        assert!(rows[0].can_run_offline);
        assert_eq!(rows[0].save_path, "");
    }

    #[test]
    fn updates_compare_build_versions() {
        let row = |id: &str, version: &str| InstalledGame {
            store_id: id.into(),
            source: crate::stores::GameSource::Store(StoreKind::Epic),
            store_meta: EpicMeta {
                version: version.into(),
                ..Default::default()
            }
            .to_json(),
            ..Default::default()
        };
        let game = |id: &str, build: &str| EpicGame {
            app_name: id.into(),
            build_version: build.into(),
            ..Default::default()
        };
        let rows = [row("a", "1"), row("b", "2"), row("c", ""), row("d", "1")];
        let owned = [
            game("a", "1"),
            game("b", "3"),
            game("c", "9"),
            game("e", "1"),
        ];
        assert_eq!(
            updates(&rows, &owned),
            vec![EpicUpdate {
                app_name: "b".into(),
                version: "3".into()
            }]
        );
    }

    #[tokio::test]
    async fn step_0_calls_say_not_implemented() {
        let dir = tempfile::tempdir().unwrap();
        let svc = EpicService::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            EpicEndpoints::real(),
        );
        let error = svc
            .complete_login(SecretString::from("FAKE-STORE-TOKEN-not-real"))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), crate::stores::NOT_IMPLEMENTED);
        assert!(svc.account().await.is_err());
        assert!(svc.owned().await.is_err());
        assert!(svc.tool_dir().ends_with("stores/legendary"));
        svc.disconnect().unwrap();
        let job = svc.update_job(
            &InstalledGame {
                store_id: "a".into(),
                ..Default::default()
            },
            EpicJobCtx {
                registry: Arc::new(Registry::open(&dir.path().join("registry.db")).unwrap()),
                library: dir.path().to_path_buf(),
                default_compat_tool: String::new(),
            },
        );
        assert_eq!(job.source, StoreKind::Epic);
        assert!(job
            .runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .is_err());
    }
}
