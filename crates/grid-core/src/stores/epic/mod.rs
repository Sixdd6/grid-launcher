//! Epic Games Store, through legendary 0.21.1 (`docs/stores-phase2-plan.md`).
//!
//! `meta` (the `store_meta` JSON) and `row` (the registry row) are shared
//! contracts; `auth` (login, status, the first-run config), `list` (the
//! owned, installed and `info` answers, and the owned cache), `install`
//! (install options, the job runners, DLC removal and uninstall) and
//! `update` (the pure update check) are package A's; launch prep and cloud
//! saves are package B's.
//!
//! Every legendary run goes through [`EpicService::tool`]: its config
//! folder (`LEGENDARY_CONFIG_PATH`), its cache (`XDG_CACHE_HOME`, where
//! the Linux zipapp unpacks its native modules), `BROWSER=true` (so `auth`
//! never opens a browser), legendary's signal matcher, and the config
//! folder made owner-only when the run ends.
//!
//! Secrets: the login code arrives as a [`SecretString`] and reaches
//! legendary on stdin only (`runner::ToolCommand::stdin`), never argv.
//! legendary keeps its own token files under `<data_dir>/stores/legendary/`
//! (the store-token exception in AGENTS.md), made owner-only after every
//! run. The exchange code `launch --json` prints is read in `launch.rs`
//! only and travels as a redacted launch argument.

pub mod auth;
pub mod cloud;
pub mod install;
pub mod launch;
pub mod list;
pub mod meta;
pub mod row;
pub mod update;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use secrecy::SecretString;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use super::paths;
use super::runner::{run_tool, LineParser, ToolCommand, ToolExit, ToolSignal};
use super::tools::{self, StoreTool};
use super::{CloudState, StoreError, SyncMode};
use crate::launch::StoreLaunchPrep;
use crate::library::queue::{StoreJob, StoreOp};
use crate::library::registry::{InstalledGame, Registry};

pub use update::updates;

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
/// Serialized only into the owned cache (product metadata, no token).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EpicGame {
    pub app_name: String,
    pub title: String,
    /// Cover art URL (keyImages, portrait first; [`list`]).
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
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
    http: reqwest::Client,
    ep: EpicEndpoints,
    /// The last owned list, for an install's details and DLC titles.
    owned: Arc<Mutex<Vec<EpicGame>>>,
    /// A program run in place of the installed legendary (tests only).
    tool_override: Option<PathBuf>,
}

impl EpicService {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: EpicEndpoints) -> Self {
        EpicService {
            data_dir,
            http,
            ep,
            owned: Arc::new(Mutex::new(Vec::new())),
            tool_override: None,
        }
    }

    /// Runs `program` in place of the installed legendary, as is (no
    /// `python3`, no pin check). For tests with a fake legendary; the app
    /// never calls it.
    #[doc(hidden)]
    pub fn with_tool_override(mut self, program: PathBuf) -> Self {
        self.tool_override = Some(program);
        self
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

    // --- the tool -----------------------------------------------------------

    /// Whether legendary is ready to run (the pinned file, or a fake).
    pub fn tool_installed(&self) -> bool {
        self.tool_override.is_some()
            || tools::installed_path(&self.data_dir, StoreTool::Legendary).is_some()
    }

    /// Downloads the pinned legendary and checks its SHA-256 (a mismatch
    /// deletes it: [`StoreError::Refused`]), then writes the first-run
    /// `config.ini`. `progress(done, total)`.
    pub async fn install_tool(
        &self,
        progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<PathBuf, StoreError> {
        let path = match &self.tool_override {
            Some(program) => {
                progress(1, 1);
                program.clone()
            }
            None => {
                tools::install_tool(&self.http, &self.data_dir, StoreTool::Legendary, progress)
                    .await?
            }
        };
        self.prepare_dir()?;
        Ok(path)
    }

    /// legendary's folder, owner-only, with the first-run `config.ini`.
    fn prepare_dir(&self) -> Result<(), StoreError> {
        let dir = self.tool_dir();
        auth::ensure_first_run_config(&dir)
            .and_then(|()| paths::make_tree_private(&dir))
            .map_err(|e| {
                StoreError::Refused(format!(
                    "Couldn't create GRID's Epic Games folder ({}).",
                    e.kind()
                ))
            })
    }

    /// The environment every legendary run gets.
    pub fn tool_env(&self) -> Vec<(String, String)> {
        let dir = self.tool_dir();
        let mut env = vec![
            (
                "LEGENDARY_CONFIG_PATH".to_string(),
                dir.to_string_lossy().into_owned(),
            ),
            ("BROWSER".to_string(), "true".to_string()),
        ];
        if !cfg!(windows) {
            env.push((
                "XDG_CACHE_HOME".to_string(),
                dir.join("cache").to_string_lossy().into_owned(),
            ));
        }
        env
    }

    /// The legendary command for `args`: the installed tool (or the test
    /// override), its environment, legendary's line parser, and its folder
    /// made private when the run ends. The first-run `config.ini` is
    /// written first. [`StoreError::ToolMissing`] when legendary is not
    /// installed.
    pub fn tool(&self, args: Vec<String>) -> Result<ToolCommand, StoreError> {
        let mut cmd = match &self.tool_override {
            Some(program) => ToolCommand {
                program: program.clone(),
                args,
                env: self.tool_env(),
                ..Default::default()
            },
            None => {
                tools::tool_command(&self.data_dir, StoreTool::Legendary, args, self.tool_env())?
            }
        };
        let dir = self.tool_dir();
        if !dir.join(auth::CONFIG_FILE).exists() {
            self.prepare_dir()?;
        }
        cmd.parser = LineParser::Legendary;
        cmd.private_tree = Some(dir);
        Ok(cmd)
    }

    /// `run_tool` on the blocking pool, not cancellable.
    async fn run_blocking(&self, cmd: ToolCommand) -> Result<ToolExit, StoreError> {
        tokio::task::spawn_blocking(move || run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}))
            .await
            .map_err(|_| StoreError::ToolFailed {
                tool: StoreTool::Legendary.name(),
                reason: "it stopped unexpectedly".to_string(),
            })?
    }

    /// Whether legendary has a saved login (`user.json`).
    fn has_login(&self) -> bool {
        self.tool_dir().join(auth::USER_FILE).is_file()
    }

    /// A failed run of a command that needs the login, in GRID's words:
    /// signed out or expired, Epic unreachable, else legendary's failure.
    fn command_error(&self, exit: &ToolExit) -> StoreError {
        if exit.has(&ToolSignal::NotLoggedIn) || exit.has(&ToolSignal::LoginFailed) {
            return if self.has_login() {
                StoreError::LoginExpired
            } else {
                StoreError::NotSignedIn
            };
        }
        if exit.has(&ToolSignal::Network) {
            return StoreError::Network("Epic Games".to_string());
        }
        install::step_failure(exit).unwrap_or_else(not_understood)
    }

    // --- login and account ----------------------------------------------------

    /// Signs in with the code: `auth --disable-webview`, the code on stdin.
    /// Done only when legendary exited 0, printed no failure and wrote its
    /// token file; then the account comes from `status --json --offline`.
    pub async fn complete_login(&self, code: SecretString) -> Result<EpicAccount, StoreError> {
        let mut cmd = self.tool(auth::login_args())?;
        cmd.stdin = Some(code);
        let exit = self.run_blocking(cmd).await?;
        auth::login_outcome(&exit, self.has_login())?;
        let cmd = self.tool(auth::status_args(true))?;
        let exit = self.run_blocking(cmd).await?;
        match exit.read_stdout(auth::parse_status) {
            Some(Some(account)) => Ok(account),
            Some(None) => Err(StoreError::Refused(
                "Epic Games didn't accept the sign-in. Try again.".to_string(),
            )),
            None => Err(not_understood()),
        }
    }

    /// The signed-in account, `None` when signed out (no saved login);
    /// [`StoreError::LoginExpired`] when Epic refuses the saved one,
    /// [`StoreError::Network`] when Epic (or legendary's API) is
    /// unreachable. `status --json` refreshes the login.
    pub async fn account(&self) -> Result<Option<EpicAccount>, StoreError> {
        if !self.has_login() {
            return Ok(None);
        }
        let cmd = self.tool(auth::status_args(false))?;
        let exit = self.run_blocking(cmd).await?;
        if exit.has(&ToolSignal::NotLoggedIn) || exit.has(&ToolSignal::LoginFailed) {
            return Err(StoreError::LoginExpired);
        }
        if exit.has(&ToolSignal::Network) {
            return Err(StoreError::Network("Epic Games".to_string()));
        }
        if !exit.success() {
            return Err(self.command_error(&exit));
        }
        match exit.read_stdout(auth::parse_status) {
            Some(Some(account)) => Ok(Some(account)),
            Some(None) => Err(StoreError::LoginExpired),
            None => Err(not_understood()),
        }
    }

    /// The account legendary has saved, without asking Epic
    /// (`status --json --offline`): `None` when signed out.
    pub async fn saved_account(&self) -> Result<Option<EpicAccount>, StoreError> {
        if !self.has_login() {
            return Ok(None);
        }
        let cmd = self.tool(auth::status_args(true))?;
        let exit = self.run_blocking(cmd).await?;
        exit.read_stdout(auth::parse_status)
            .ok_or_else(|| self.command_error(&exit))
    }

    /// Signs out: `auth --delete` (best effort), then legendary's whole
    /// folder (its token files with it) and the owned cache go. Blocking.
    pub fn disconnect(&self) -> io::Result<()> {
        if let Ok(cmd) = self.tool(auth::logout_args()) {
            let _ = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {});
        }
        self.owned.lock().unwrap_or_else(|e| e.into_inner()).clear();
        let _ = list::remove_owned_cache(&self.data_dir);
        paths::remove_tool_dir(&self.data_dir, StoreTool::Legendary)
    }

    // --- owned and installed --------------------------------------------------

    /// The owned games, DLC folded into their base (`list --json -T`). The
    /// list is kept for the jobs ([`Self::game`]); the caller writes the
    /// owned cache ([`list::write_owned_cache`]).
    pub async fn owned(&self) -> Result<Vec<EpicGame>, StoreError> {
        if !self.has_login() {
            return Err(StoreError::NotSignedIn);
        }
        let cmd = self.tool(list::list_args())?;
        let exit = self.run_blocking(cmd).await?;
        let games = match exit.read_stdout(list::parse_owned) {
            Some(games) if exit.success() => games,
            _ => return Err(self.command_error(&exit)),
        };
        self.set_owned(games.clone());
        Ok(games)
    }

    /// Seeds the kept owned list (from the owned cache at startup).
    pub fn set_owned(&self, games: Vec<EpicGame>) {
        *self.owned.lock().unwrap_or_else(|e| e.into_inner()) = games;
    }

    /// The owned game `app` from the kept owned list.
    pub fn game(&self, app: &str) -> Option<EpicGame> {
        self.owned
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|game| game.app_name == app.trim())
            .cloned()
    }

    /// What legendary has installed (`list-installed --json`, offline).
    pub async fn installed(&self) -> Result<Vec<EpicInstalled>, StoreError> {
        let cmd = self.tool(list::installed_args())?;
        let exit = self.run_blocking(cmd).await?;
        exit.read_stdout(list::parse_installed)
            .ok_or_else(|| self.command_error(&exit))
    }

    /// The base game's sizes and its DLC with theirs: `info --json` for the
    /// base, then for each DLC it (or the owned list) names. A DLC whose
    /// `info` fails is offered with no size.
    pub async fn install_options(&self, app: &str) -> Result<EpicInstallOptions, StoreError> {
        let game = self.game(app);
        if let Some(launcher) = game.as_ref().and_then(|g| g.third_party.clone()) {
            return Err(StoreError::Refused(format!(
                "Needs {launcher}. GRID can't install games that start through another company's launcher."
            )));
        }
        let cmd = self.tool(list::info_args(app))?;
        let exit = self.run_blocking(cmd).await?;
        let base = match exit.read_stdout(list::parse_info) {
            Some(info) if install::step_failure(&exit).is_none() => info,
            _ => return Err(self.command_error(&exit)),
        };
        let mut dlc = Vec::new();
        for item in install::dlc_to_offer(&base, game.as_ref()) {
            let info = match self.tool(list::info_args(&item.app_name)) {
                Ok(cmd) => self
                    .run_blocking(cmd)
                    .await
                    .ok()
                    .filter(|exit| install::step_failure(exit).is_none())
                    .and_then(|exit| exit.read_stdout(list::parse_info)),
                Err(_) => None,
            };
            dlc.push((item, info));
        }
        Ok(install::options_from(&base, dlc))
    }

    // --- jobs -------------------------------------------------------------------

    pub fn install_job(&self, req: EpicInstallRequest, ctx: EpicJobCtx) -> StoreJob {
        install::install_job(self.clone(), req, ctx)
    }

    pub fn update_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob {
        install::row_job(self.clone(), row, StoreOp::Update, None, ctx)
    }

    pub fn repair_job(&self, row: &InstalledGame, ctx: EpicJobCtx) -> StoreJob {
        install::row_job(self.clone(), row, StoreOp::Repair, None, ctx)
    }

    pub fn dlc_job(&self, row: &InstalledGame, dlc: &str, ctx: EpicJobCtx) -> StoreJob {
        install::row_job(self.clone(), row, StoreOp::Dlc, Some(dlc), ctx)
    }

    /// Removes one installed DLC (`uninstall <dlc>`). Blocking.
    pub fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc: &str,
    ) -> Result<(), StoreError> {
        install::remove_dlc(self, registry, row, dlc)
    }

    /// Uninstalls a game: legendary's `uninstall`, then the folder (prefix
    /// inside it) through the removal guard, then the row. Blocking.
    pub fn uninstall(&self, registry: &Registry, app: &str) -> Result<(), StoreError> {
        install::uninstall(self, registry, app)
    }

    /// The installed games the owned list has a different build for. Pure:
    /// `build_version` against each row's `meta.version`; a row with no
    /// version, or a game with no build version, never shows an update.
    pub fn updates(&self, rows: &[InstalledGame], owned: &[EpicGame]) -> Vec<EpicUpdate> {
        updates(rows, owned)
    }

    // --- package B --------------------------------------------------------------

    /// The launch ticket: `launch --json`, its `egl_parameters` as
    /// redacted arguments (package B).
    pub async fn launch_prep(
        &self,
        row: &InstalledGame,
        offline: bool,
    ) -> Result<StoreLaunchPrep, StoreError> {
        launch::launch_prep(self, row, offline).await
    }

    /// The game's cloud save state, without syncing (`list-saves`).
    pub async fn cloud_state(&self, row: &InstalledGame) -> Result<CloudState, StoreError> {
        cloud::cloud_state(self, row).await
    }

    pub async fn cloud_sync(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        mode: SyncMode,
    ) -> Result<CloudState, StoreError> {
        cloud::cloud_sync(self, registry, row, mode).await
    }
}

/// legendary printed something GRID can't read.
fn not_understood() -> StoreError {
    StoreError::ToolFailed {
        tool: StoreTool::Legendary.name(),
        reason: "it gave an answer GRID doesn't understand".to_string(),
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

    fn service(data: &std::path::Path) -> EpicService {
        EpicService::new(
            data.to_path_buf(),
            reqwest::Client::new(),
            EpicEndpoints::real(),
        )
    }

    #[test]
    fn the_tool_env_names_legendarys_folders_and_no_browser() {
        let svc = service(std::path::Path::new("/data"));
        let env = svc.tool_env();
        assert_eq!(env[0].0, "LEGENDARY_CONFIG_PATH");
        assert!(PathBuf::from(&env[0].1).ends_with("stores/legendary"));
        assert!(env.contains(&("BROWSER".to_string(), "true".to_string())));
        if !cfg!(windows) {
            let cache = env.iter().find(|(k, _)| k == "XDG_CACHE_HOME").unwrap();
            assert!(PathBuf::from(&cache.1).ends_with("stores/legendary/cache"));
        }
    }

    #[test]
    fn a_missing_legendary_is_tool_missing_and_signed_out_needs_no_run() {
        #[cfg(feature = "e2e")]
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let svc = service(dir.path());
        if tools::host_asset(StoreTool::Legendary).is_some() {
            assert!(!svc.tool_installed());
            assert!(matches!(
                svc.tool(auth::status_args(true)),
                Err(StoreError::ToolMissing("legendary"))
            ));
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(rt.block_on(svc.account()).unwrap(), None);
        assert!(matches!(
            rt.block_on(svc.owned()),
            Err(StoreError::NotSignedIn)
        ));
        svc.disconnect().unwrap();
        assert!(!svc.tool_dir().exists());
    }

    #[test]
    fn the_kept_owned_list_answers_game() {
        let dir = tempfile::tempdir().unwrap();
        let svc = service(dir.path());
        assert_eq!(svc.game("A"), None);
        svc.set_owned(vec![EpicGame {
            app_name: "A".into(),
            title: "Alpha".into(),
            ..Default::default()
        }]);
        assert_eq!(svc.game(" A ").unwrap().title, "Alpha");
        let clone = svc.clone();
        assert!(clone.game("A").is_some(), "clones share the list");
    }
}
