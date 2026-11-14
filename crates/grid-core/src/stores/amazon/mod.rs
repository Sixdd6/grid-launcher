//! Amazon Games (Prime Gaming claims and Amazon Games purchases), through
//! nile 1.2.0 (`docs/stores-phase3-plan.md`).
//!
//! `meta` (the `store_meta` JSON) and `row` (the registry row and the
//! `fuel.json` reader) are Step 0's shared contracts; `auth` (login,
//! account, logout), `list` (nile's `library.json` and `installed.json`,
//! the download size and the update list), `install` (the install, update
//! and verify jobs, and uninstall) and `launch` (the dry-run launch prep)
//! are package A's.
//!
//! Every nile run goes through [`AmazonService::tool`]: its config folder
//! (`NILE_CONFIG_PATH=<data_dir>/stores/nile`; nile appends `/nile`, so its
//! files land in `<data_dir>/stores/nile/nile/`), nile's line parser, and
//! the folder made owner-only before and after the run (nile writes every
//! file with the default umask). Never `-d`: at DEBUG nile logs response
//! bodies. Every run goes through [`AmazonService::run`], which turns a
//! nile that can't start on this system (its frozen Linux build needs
//! glibc 2.35) into [`NILE_SYSTEM_TOO_OLD`], never its raw output.
//!
//! Secrets: the login code arrives as a [`SecretString`];
//! [`AmazonLoginStart`] keeps nile's `client_id`, `code_verifier` and
//! `serial` in one. Only `auth.rs` (package A) reads them, to put them on
//! `nile register`'s argv (nile has no stdin or environment path for them;
//! `ToolCommand`'s `Debug` hides every argument). nile keeps its own token
//! files under `<data_dir>/stores/nile/` (the store-token exception in
//! AGENTS.md). `launch --json` carries no token, but its
//! `AMAZON_GAMES_FUEL_DISPLAY_NAME` is the user's given name: personal data,
//! never logged.

pub mod auth;
pub mod install;
pub mod launch;
pub mod list;
pub mod meta;
pub mod row;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use secrecy::SecretString;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use super::paths;
use super::progress::ToolProgress;
use super::runner::{run_tool, LineParser, ToolCommand, ToolExit, ToolSignal};
use super::tools::{self, StoreTool};
use super::StoreError;
use crate::launch::StoreLaunchPrep;
use crate::library::queue::{StoreJob, StoreOp};
use crate::library::registry::{InstalledGame, Registry};

/// The finish URL's query parameter that carries the code.
pub const AMAZON_CODE_PARAM: &str = "openid.oa2.authorization_code";

/// The query pair that marks Amazon's finish redirect
/// (`https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher&…`).
/// Amazon's home page has the same path, so the login window needs it to
/// tell the two apart (`LoginSpec::finish_query`).
pub const AMAZON_ASSOC_HANDLE: (&str, &str) = ("openid.assoc_handle", "amzn_sonic_games_launcher");

/// The path of the sign-in page nile's login URL opens.
pub const AMAZON_SIGNIN_PATH: &str = "/ap/signin";

/// The deadline for a nile command that should answer in seconds (`auth`,
/// `launch --json`, `install --info`, `list-updates`). nile sets no timeout
/// on its requests and fetches Amazon's FuelSDK before every command until
/// it has it, so on a network that drops packets any command can hang for
/// ever (spike 1). Jobs (install, update, verify) have no deadline.
pub const QUICK_RUN_TIMEOUT: Duration = Duration::from_secs(90);

/// The deadline for `library sync` (a network call that pages through the
/// whole library on a first sync).
pub const SYNC_TIMEOUT: Duration = Duration::from_secs(300);

/// The deadline for Disconnect's best-effort `auth --logout`.
pub const LOGOUT_TIMEOUT: Duration = Duration::from_secs(30);

/// GRID's words when nile can't start on this computer: its frozen Linux
/// build needs glibc 2.35 or later (spike 1). Settings and the store panel
/// show it as it is.
pub const NILE_SYSTEM_TOO_OLD: &str =
    "nile can't run on this computer: it needs a newer Linux system (glibc 2.35 or later).";

/// The Amazon addresses GRID uses. Tests and e2e point them at a mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmazonEndpoints {
    /// Where Amazon sends the browser when the sign-in ends:
    /// `https://www.amazon.com/?openid.assoc_handle=…&openid.oa2.authorization_code=…`.
    pub finish: Url,
    /// The origins a login URL from nile may have (nile prints
    /// `https://amazon.com/ap/signin?…`).
    pub login_hosts: Vec<Url>,
    /// Origins besides Amazon's image hosts that may serve art (the e2e
    /// mock).
    pub art_origins: Vec<Url>,
}

impl AmazonEndpoints {
    /// Amazon's own addresses.
    pub fn real() -> AmazonEndpoints {
        let url = |text: &str| Url::parse(text).unwrap_or_else(|_| unreachable!("{text}"));
        AmazonEndpoints {
            finish: url("https://www.amazon.com/"),
            login_hosts: vec![url("https://amazon.com/"), url("https://www.amazon.com/")],
            art_origins: Vec::new(),
        }
    }

    /// Everything at one mock server (e2e, tests): the finish is `<base>/`,
    /// the login URL's origin is the base's, art comes from `<base>`.
    pub fn e2e(base: Url) -> AmazonEndpoints {
        let root = base.join("/").unwrap_or_else(|_| base.clone());
        AmazonEndpoints {
            finish: root.clone(),
            login_hosts: vec![root.clone()],
            art_origins: vec![root],
        }
    }

    /// Whether `url` is a sign-in page the login window may open: one of
    /// [`Self::login_hosts`]' origins (scheme, host and port) and the path
    /// [`AMAZON_SIGNIN_PATH`]. A login URL nile prints is checked with this
    /// before the window loads it.
    pub fn allows_login_url(&self, url: &Url) -> bool {
        url.path() == AMAZON_SIGNIN_PATH
            && self
                .login_hosts
                .iter()
                .any(|host| host.origin() == url.origin())
    }
}

/// What `nile auth --login --non-interactive` gave for one sign-in: the
/// page the login window opens, and nile's `client_id`, `code_verifier`
/// and `serial` (as one JSON object in a [`SecretString`]) for
/// `nile register` once the code comes back. Its `Debug` prints the url's
/// host only: the url's query holds the client id and the PKCE challenge.
pub struct AmazonLoginStart {
    pub url: Url,
    secret: SecretString,
}

impl AmazonLoginStart {
    /// A start from nile's answer (package A's `auth.rs`).
    pub fn new(url: Url, secret: SecretString) -> AmazonLoginStart {
        AmazonLoginStart { url, secret }
    }

    /// The kept values, for `auth.rs` only (the one file allowed to read
    /// them: `scripts/check_secret_hygiene.sh`).
    pub fn secret(&self) -> &SecretString {
        &self.secret
    }
}

impl std::fmt::Debug for AmazonLoginStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmazonLoginStart")
            .field("host", &self.url.host_str().unwrap_or(""))
            .finish_non_exhaustive()
    }
}

/// The signed-in Amazon account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AmazonAccount {
    /// nile's `Username`: the account's given name.
    pub username: String,
}

/// One owned Amazon game from nile's `library.json` (product metadata, no
/// token).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AmazonGame {
    /// `product.id` (`amzn1.adg.product.<uuid>`): the id every nile command
    /// takes, and GRID's `store_id`.
    pub product_id: String,
    /// The entitlement `id`.
    pub entitlement_id: String,
    /// `product.title`, the product id when Amazon gives none.
    pub title: String,
    /// Cover art URL.
    pub cover: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub developer: String,
    /// The release year, or `""`.
    pub year: String,
    pub genres: Vec<String>,
}

/// One `installed.json` entry. Unknown keys are ignored and a `null` reads
/// as the default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct AmazonInstalled {
    /// The product id.
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// An opaque version id.
    #[serde(deserialize_with = "nullable")]
    pub version: String,
    /// The install folder.
    #[serde(deserialize_with = "nullable")]
    pub path: String,
    /// Bytes on disk.
    #[serde(deserialize_with = "nullable")]
    pub size: u64,
}

/// A value that may be JSON `null`, read as its default.
fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// An install the user confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmazonInstallRequest {
    pub product_id: String,
    pub title: String,
    /// The game's own folder (`install --path`).
    pub install_dir: PathBuf,
    /// `""` for the default.
    pub compat_tool: String,
}

/// An installed game nile has an update for (`list-updates --json`). The
/// version is opaque, so there is none to show (User decisions 1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AmazonUpdate {
    pub product_id: String,
}

/// What every Amazon job needs from the app.
#[derive(Clone)]
pub struct AmazonJobCtx {
    pub registry: Arc<Registry>,
    pub library: PathBuf,
    pub default_compat_tool: String,
}

/// The Amazon service: cheap to clone, shared by the app's commands.
#[derive(Clone)]
pub struct AmazonService {
    data_dir: PathBuf,
    http: reqwest::Client,
    ep: AmazonEndpoints,
    /// A program run in place of the installed nile (tests only).
    tool_override: Option<PathBuf>,
}

impl AmazonService {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: AmazonEndpoints) -> Self {
        AmazonService {
            data_dir,
            http,
            ep,
            tool_override: None,
        }
    }

    /// Runs `program` in place of the installed nile, as is (no pin
    /// check). For tests with a fake nile; the app never calls it. The one
    /// test override: every nile run goes through [`Self::tool`].
    #[doc(hidden)]
    pub fn with_tool_override(mut self, program: PathBuf) -> Self {
        self.tool_override = Some(program);
        self
    }

    /// The addresses this service uses.
    pub fn endpoints(&self) -> &AmazonEndpoints {
        &self.ep
    }

    /// `<data_dir>/stores/nile`: the binary's folder and
    /// `NILE_CONFIG_PATH`.
    pub fn tool_dir(&self) -> PathBuf {
        paths::tool_dir(&self.data_dir, StoreTool::Nile)
    }

    /// `<data_dir>/stores/nile/nile`: where nile keeps its files
    /// (`current_user.json`, the token file, `library.json`,
    /// `installed.json`, `manifests/`, `SDK/`).
    pub fn config_dir(&self) -> PathBuf {
        self.tool_dir().join("nile")
    }

    // --- the tool -----------------------------------------------------------

    /// Whether nile is ready to run (the pinned file, or a fake).
    pub fn tool_installed(&self) -> bool {
        self.tool_override.is_some()
            || tools::installed_path(&self.data_dir, StoreTool::Nile).is_some()
    }

    /// Downloads the pinned nile and checks its SHA-256 (a mismatch deletes
    /// it: [`StoreError::Refused`]). `progress(done, total)`.
    pub async fn install_tool(
        &self,
        progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<PathBuf, StoreError> {
        match &self.tool_override {
            Some(program) => {
                progress(1, 1);
                Ok(program.clone())
            }
            None => {
                tools::install_tool(&self.http, &self.data_dir, StoreTool::Nile, progress).await
            }
        }
    }

    /// The environment every nile run gets.
    pub fn tool_env(&self) -> Vec<(String, String)> {
        vec![(
            "NILE_CONFIG_PATH".to_string(),
            self.tool_dir().to_string_lossy().into_owned(),
        )]
    }

    /// The nile command for `args`: the installed tool (or the test
    /// override), its environment, nile's line parser, and its folder made
    /// owner-only now (so it is 0700 while nile writes 0644 files) and
    /// again when the run ends. No timeout: a short command sets
    /// [`QUICK_RUN_TIMEOUT`] itself. [`StoreError::ToolMissing`] when nile
    /// is not installed.
    pub fn tool(&self, args: Vec<String>) -> Result<ToolCommand, StoreError> {
        let mut cmd = match &self.tool_override {
            Some(program) => ToolCommand {
                program: program.clone(),
                args,
                env: self.tool_env(),
                ..Default::default()
            },
            None => tools::tool_command(&self.data_dir, StoreTool::Nile, args, self.tool_env())?,
        };
        let dir = self.tool_dir();
        paths::ensure_private_dir(&dir).map_err(|e| {
            StoreError::Refused(format!(
                "Couldn't create GRID's Amazon Games folder ({}).",
                e.kind()
            ))
        })?;
        cmd.parser = LineParser::Nile;
        cmd.private_tree = Some(dir);
        Ok(cmd)
    }

    // --- running nile ---------------------------------------------------------

    /// Runs `cmd` (a [`Self::tool`] command) through the runner. A nile
    /// that can't start on this system fails with [`NILE_SYSTEM_TOO_OLD`]:
    /// on Linux an exec error for the installed file (a missing loader), or
    /// a loader line naming a missing glibc version
    /// ([`ToolSignal::SystemTooOld`]). nile's own lines are never in the
    /// error. Blocking.
    pub(crate) fn run(
        &self,
        cmd: &ToolCommand,
        cancel: &AtomicBool,
        on_progress: &mut dyn FnMut(ToolProgress),
    ) -> Result<ToolExit, StoreError> {
        match run_tool(cmd, cancel, on_progress) {
            Ok(exit) if exit.has(&ToolSignal::SystemTooOld) => Err(system_too_old()),
            Err(StoreError::ToolFailed { reason, .. })
                if cfg!(target_os = "linux")
                    && reason.starts_with(START_FAILED)
                    && cmd.program.is_file() =>
            {
                Err(system_too_old())
            }
            other => other,
        }
    }

    /// [`Self::run`] on the blocking pool, not cancellable.
    pub(crate) async fn run_blocking(&self, cmd: ToolCommand) -> Result<ToolExit, StoreError> {
        let svc = self.clone();
        tokio::task::spawn_blocking(move || svc.run(&cmd, &AtomicBool::new(false), &mut |_| {}))
            .await
            .map_err(|_| StoreError::ToolFailed {
                tool: StoreTool::Nile.name(),
                reason: "it stopped unexpectedly".to_string(),
            })?
    }

    /// A [`Self::tool`] command with [`QUICK_RUN_TIMEOUT`].
    fn quick(&self, args: Vec<String>) -> Result<ToolCommand, StoreError> {
        let mut cmd = self.tool(args)?;
        cmd.timeout = Some(QUICK_RUN_TIMEOUT);
        Ok(cmd)
    }

    /// Whether nile has a saved login (`current_user.json` names a user).
    pub fn has_login(&self) -> bool {
        auth::has_saved_login(&self.config_dir())
    }

    /// A failed run of a command that needs the login, in GRID's words:
    /// signed out or expired, Amazon unreachable, else nile's failure.
    fn command_error(&self, exit: &ToolExit) -> StoreError {
        if exit.has(&ToolSignal::NotLoggedIn) || exit.has(&ToolSignal::LoginFailed) {
            return if self.has_login() {
                StoreError::LoginExpired
            } else {
                StoreError::NotSignedIn
            };
        }
        if exit.has(&ToolSignal::Network) {
            return StoreError::Network("Amazon Games".to_string());
        }
        if !self.has_login() {
            // Signed out, nile crashes reading the token.
            return StoreError::NotSignedIn;
        }
        install::step_failure(exit).unwrap_or_else(not_understood)
    }

    // --- login and account -------------------------------------------------------

    /// `auth --login --non-interactive`: the login URL (checked with
    /// [`AmazonEndpoints::allows_login_url`]) and the values `register`
    /// needs. nile's saved login is removed first: nile refuses a new
    /// sign-in while one is saved, even an expired one.
    pub async fn start_login(&self) -> Result<AmazonLoginStart, StoreError> {
        let cmd = self.quick(auth::login_args())?;
        self.clear_login()?;
        let exit = self.run_blocking(cmd).await?;
        match exit.read_stdout(|stdout| auth::parse_login_start(stdout, &self.ep)) {
            Ok(start) => Ok(start),
            Err(error @ StoreError::Refused(_)) => Err(error),
            Err(error) => Err(install::step_failure(&exit).unwrap_or(error)),
        }
    }

    fn clear_login(&self) -> Result<(), StoreError> {
        auth::clear_saved_login(&self.config_dir()).map_err(|e| {
            StoreError::Refused(format!(
                "Couldn't clear the old Amazon Games login ({}).",
                e.kind()
            ))
        })
    }

    /// `register` with the code and the kept values, then the account
    /// from `auth --status`.
    pub async fn complete_login(
        &self,
        start: AmazonLoginStart,
        code: SecretString,
    ) -> Result<AmazonAccount, StoreError> {
        let args = auth::register_args(&start, &code)
            .ok_or_else(|| StoreError::Refused(auth::LOGIN_REFUSED.to_string()))?;
        drop((start, code));
        let cmd = self.quick(args)?;
        self.clear_login()?;
        let exit = self.run_blocking(cmd).await?;
        auth::login_outcome(&exit, self.has_login())?;
        let exit = self.run_blocking(self.quick(auth::status_args())?).await?;
        match exit.read_stdout(auth::parse_status) {
            Some(Some(account)) => Ok(account),
            Some(None) => Err(StoreError::Refused(auth::LOGIN_REFUSED.to_string())),
            None => Err(not_understood()),
        }
    }

    /// The account nile has saved (`auth --status`, local only): `None`
    /// when signed out. It does not check the token with Amazon
    /// ([`Self::sync`] does).
    pub async fn saved_account(&self) -> Result<Option<AmazonAccount>, StoreError> {
        if !self.has_login() {
            return Ok(None);
        }
        let exit = self.run_blocking(self.quick(auth::status_args())?).await?;
        exit.read_stdout(auth::parse_status)
            .ok_or_else(|| self.command_error(&exit))
    }

    /// Signs out: `auth --logout` (best effort, [`LOGOUT_TIMEOUT`]), then
    /// nile's whole folder (its token files with it) goes, whatever Amazon
    /// answered.
    pub async fn disconnect(&self) -> io::Result<()> {
        let svc = self.clone();
        tokio::task::spawn_blocking(move || {
            if svc.has_login() {
                if let Ok(mut cmd) = svc.tool(auth::logout_args()) {
                    cmd.timeout = Some(LOGOUT_TIMEOUT);
                    let _ = svc.run(&cmd, &AtomicBool::new(false), &mut |_| {});
                }
            }
            paths::remove_tool_dir(&svc.data_dir, StoreTool::Nile)
        })
        .await
        .map_err(io::Error::other)?
    }

    // --- owned and installed --------------------------------------------------

    /// `library sync`, then nile's `library.json`.
    /// [`StoreError::NotSignedIn`] with no saved login,
    /// [`StoreError::LoginExpired`] when Amazon refuses the saved login
    /// (or nile's token file is gone), [`StoreError::Network`] when it
    /// can't be reached.
    pub async fn sync(&self) -> Result<Vec<AmazonGame>, StoreError> {
        if !self.has_login() {
            return Err(StoreError::NotSignedIn);
        }
        let mut cmd = self.tool(list::sync_args())?;
        cmd.timeout = Some(SYNC_TIMEOUT);
        let exit = self.run_blocking(cmd).await?;
        if exit.has(&ToolSignal::NotLoggedIn) || !auth::has_token_file(&self.config_dir()) {
            return Err(StoreError::LoginExpired);
        }
        if exit.has(&ToolSignal::Network) {
            return Err(StoreError::Network("Amazon Games".to_string()));
        }
        if let Some(error) = install::step_failure(&exit) {
            return Err(error);
        }
        list::read_library(&self.config_dir()).ok_or_else(not_understood)
    }

    /// nile's `library.json` as it is (startup, offline); empty when there
    /// is none.
    pub fn owned_local(&self) -> Vec<AmazonGame> {
        list::read_library(&self.config_dir()).unwrap_or_default()
    }

    /// nile's `installed.json`.
    pub fn installed(&self) -> Vec<AmazonInstalled> {
        list::read_installed(&self.config_dir())
    }

    /// `install --info --json <id>`: the download size in bytes (also the
    /// disk size).
    pub async fn download_size(&self, id: &str) -> Result<u64, StoreError> {
        let exit = self.run_blocking(self.quick(list::info_args(id))?).await?;
        match exit.read_stdout(list::parse_info) {
            Some(size) if install::step_failure(&exit).is_none() => Ok(size),
            _ => Err(self.command_error(&exit)),
        }
    }

    // --- jobs --------------------------------------------------------------------

    pub fn install_job(&self, req: AmazonInstallRequest, ctx: AmazonJobCtx) -> StoreJob {
        install::install_job(self.clone(), req, ctx)
    }

    pub fn update_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob {
        install::row_job(self.clone(), row, StoreOp::Update, ctx)
    }

    /// Verify and repair (`verify <id>`).
    pub fn verify_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob {
        install::row_job(self.clone(), row, StoreOp::Repair, ctx)
    }

    /// `uninstall <id>`, then the leftover folder and its prefix. Blocking.
    pub fn uninstall(&self, registry: &Registry, id: &str) -> Result<(), StoreError> {
        install::uninstall(self, registry, id)
    }

    /// `list-updates --json`, installed ids only. Nothing installed needs
    /// no run.
    pub async fn updates(&self) -> Result<Vec<AmazonUpdate>, StoreError> {
        let installed = self.installed();
        if installed.is_empty() {
            return Ok(Vec::new());
        }
        let exit = self.run_blocking(self.quick(list::updates_args())?).await?;
        let ids = match exit.read_stdout(list::parse_updates) {
            Some(ids) if install::step_failure(&exit).is_none() => ids,
            _ => return Err(self.command_error(&exit)),
        };
        let mut updates: Vec<AmazonUpdate> = Vec::new();
        for id in ids {
            let known = installed.iter().any(|row| row.id.trim() == id);
            if known && !updates.iter().any(|u| u.product_id == id) {
                updates.push(AmazonUpdate { product_id: id });
            }
        }
        Ok(updates)
    }

    /// `launch --json --no-wine <id>`: the executable, arguments and the
    /// five FUEL variables. No network, no token. A failure reads as the
    /// launch's words through [`launch::launch_error`].
    pub async fn launch_prep(&self, row: &InstalledGame) -> Result<StoreLaunchPrep, StoreError> {
        launch::launch_prep(self, row).await
    }
}

/// The runner's reason when a tool could not be spawned.
const START_FAILED: &str = "couldn't start it";

fn system_too_old() -> StoreError {
    StoreError::Refused(NILE_SYSTEM_TOO_OLD.to_string())
}

/// nile printed something GRID can't read.
fn not_understood() -> StoreError {
    StoreError::ToolFailed {
        tool: StoreTool::Nile.name(),
        reason: "it gave an answer GRID doesn't understand".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_and_e2e_endpoints() {
        let real = AmazonEndpoints::real();
        assert_eq!(real.finish.as_str(), "https://www.amazon.com/");
        assert!(real.art_origins.is_empty());
        let base = Url::parse("http://127.0.0.1:4400/").unwrap();
        let e2e = AmazonEndpoints::e2e(base.clone());
        assert_eq!(e2e.finish.as_str(), "http://127.0.0.1:4400/");
        assert_eq!(e2e.login_hosts, vec![base.clone()]);
        assert_eq!(e2e.art_origins, vec![base]);
    }

    #[test]
    fn only_amazons_signin_page_is_a_login_url() {
        let real = AmazonEndpoints::real();
        let ok = |text: &str| real.allows_login_url(&Url::parse(text).unwrap());
        assert!(ok("https://amazon.com/ap/signin?openid.ns=x&openid.assoc_handle=amzn_sonic_games_launcher"));
        assert!(ok("https://www.amazon.com/ap/signin?x=1"));
        for bad in [
            "http://amazon.com/ap/signin",
            "https://amazon.com/ap/signout",
            "https://amazon.com/",
            "https://evil.example/ap/signin",
            "https://amazon.com.evil.example/ap/signin",
            "https://www.amazon.co.uk/ap/signin",
            "https://amazon.com:8443/ap/signin",
        ] {
            assert!(!ok(bad), "{bad}");
        }
        let e2e = AmazonEndpoints::e2e(Url::parse("http://127.0.0.1:4400/").unwrap());
        assert!(e2e.allows_login_url(&Url::parse("http://127.0.0.1:4400/ap/signin?a=b").unwrap()));
        assert!(!e2e.allows_login_url(&Url::parse("http://127.0.0.1:4401/ap/signin").unwrap()));
        assert!(!e2e.allows_login_url(&Url::parse("https://amazon.com/ap/signin").unwrap()));
    }

    #[test]
    fn a_login_start_never_prints_its_values() {
        let start = AmazonLoginStart::new(
            Url::parse("https://amazon.com/ap/signin?openid.oa2.client_id=device:FAKE-STORE-TOKEN-not-real").unwrap(),
            SecretString::from(r#"{"code_verifier": "FAKE-STORE-TOKEN-not-real"}"#),
        );
        let text = format!("{start:?}");
        assert!(!text.contains("FAKE-STORE-TOKEN"), "{text}");
        assert!(!text.contains("client_id"), "{text}");
        assert!(text.contains("amazon.com"), "{text}");
    }

    #[test]
    fn installed_json_parses_with_nulls_and_unknown_keys() {
        let json = r#"[{"id": "amzn1.adg.product.1", "version": "v1", "path": "/g/F", "size": 1234, "extra": true},
                       {"id": "amzn1.adg.product.2", "version": null, "path": null, "size": null}]"#;
        let rows: Vec<AmazonInstalled> = serde_json::from_str(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].size, 1234);
        assert_eq!(rows[1].version, "");
        assert_eq!(rows[1].size, 0);
    }

    #[cfg(unix)]
    #[test]
    fn the_nile_command_has_its_env_parser_and_private_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let svc = AmazonService::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            AmazonEndpoints::real(),
        )
        .with_tool_override(PathBuf::from("/bin/true"));
        assert!(svc.tool_installed());
        let cmd = svc.tool(vec!["auth".into(), "--status".into()]).unwrap();
        assert_eq!(cmd.parser, LineParser::Nile);
        assert_eq!(cmd.args, ["auth", "--status"]);
        assert!(!cmd.args.iter().any(|a| a == "-d"));
        assert_eq!(cmd.timeout, None);
        let tool_dir = dir.path().join("stores").join("nile");
        assert_eq!(svc.tool_dir(), tool_dir);
        assert_eq!(svc.config_dir(), tool_dir.join("nile"));
        assert_eq!(cmd.private_tree.as_deref(), Some(tool_dir.as_path()));
        assert_eq!(
            cmd.env,
            vec![(
                "NILE_CONFIG_PATH".to_string(),
                tool_dir.to_string_lossy().into_owned()
            )]
        );
        let mode = std::fs::metadata(&tool_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "made private before the run");
    }

    #[test]
    fn signed_out_calls_need_no_nile_and_a_missing_nile_is_tool_missing() {
        #[cfg(feature = "e2e")]
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let svc = AmazonService::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            AmazonEndpoints::real(),
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(!svc.has_login());
        assert_eq!(rt.block_on(svc.saved_account()).unwrap(), None);
        assert!(matches!(
            rt.block_on(svc.sync()),
            Err(StoreError::NotSignedIn)
        ));
        assert!(
            rt.block_on(svc.updates()).unwrap().is_empty(),
            "nothing installed"
        );
        assert!(svc.owned_local().is_empty());
        assert!(svc.installed().is_empty());
        if tools::host_asset(StoreTool::Nile).is_some() {
            assert!(!svc.tool_installed());
            assert!(matches!(
                rt.block_on(svc.start_login()),
                Err(StoreError::ToolMissing("nile"))
            ));
        }
        rt.block_on(svc.disconnect()).unwrap();
        assert!(!svc.tool_dir().exists());
        let job = svc.update_job(
            &InstalledGame {
                store_id: "amzn1.adg.product.1".into(),
                ..Default::default()
            },
            AmazonJobCtx {
                registry: Arc::new(Registry::open(&dir.path().join("registry.db")).unwrap()),
                library: dir.path().to_path_buf(),
                default_compat_tool: String::new(),
            },
        );
        assert_eq!(job.source, crate::stores::StoreKind::Amazon);
        assert_eq!(job.op, StoreOp::Update);
        let error = job
            .runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .unwrap_err();
        assert_eq!(error.to_string(), "not signed in");
    }

    #[tokio::test]
    async fn a_nile_download_that_fails_its_checksum_is_deleted() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/nile"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not nile".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("nile_linux_x86_64");
        let pin = tools::host_asset(StoreTool::Nile)
            .map(|asset| asset.sha256)
            .unwrap_or("d73519514df9d52c50c6feec629373cc3a0d9504e087b6c7372a8ca8a95e7bfe");
        let error = tools::download_verified(
            &reqwest::Client::new(),
            &format!("{}/nile", server.uri()),
            &dest,
            pin,
            8,
            StoreTool::Nile,
            &mut |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(error, StoreError::Refused(_)), "{error}");
        assert_eq!(
            error.to_string(),
            "The nile download didn't match its checksum, so GRID deleted it."
        );
        assert!(!dest.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_nile_that_cant_start_says_it_needs_a_newer_system() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fake = |name: &str, body: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        // PyInstaller's loader line on an old glibc, after the token marker.
        let loader = fake(
            "loader.sh",
            "#!/bin/sh\necho FAKE-STORE-TOKEN-not-real\necho FAKE-STORE-TOKEN-not-real >&2\n\
             echo \"[PYI-4242:ERROR] Failed to load Python shared library '/tmp/_MEIx/libpython3.8.so.1.0': \
             /lib64/libc.so.6: version \\`GLIBC_2.35' not found (required by /tmp/_MEIx/libpython3.8.so.1.0)\" >&2\n\
             exit 255\n",
        );
        // An exec error: the interpreter (like a missing ELF loader) isn't there.
        let exec = fake("exec.sh", "#!/nonexistent/ld-linux-x86-64.so.2\n");
        for program in [loader, exec] {
            let svc = AmazonService::new(
                dir.path().join("data"),
                reqwest::Client::new(),
                AmazonEndpoints::real(),
            )
            .with_tool_override(program.clone());
            let cmd = svc.tool(auth::status_args()).unwrap();
            let result = svc.run(&cmd, &AtomicBool::new(false), &mut |_| {});
            if program.ends_with("exec.sh") && !cfg!(target_os = "linux") {
                assert!(result.is_err());
                continue;
            }
            let error = result.unwrap_err();
            assert_eq!(error.to_string(), NILE_SYSTEM_TOO_OLD, "{program:?}");
            assert!(!error.to_string().contains("FAKE"));
            assert!(!error.to_string().contains("PYI"));
        }
    }
}
