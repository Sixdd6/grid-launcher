//! Amazon Games (Prime Gaming claims and Amazon Games purchases), through
//! nile 1.2.0 (`docs/stores-phase3-plan.md`).
//!
//! Step 0 holds the contracts every work package builds on: the types, the
//! `store_meta` JSON ([`meta`]), the registry row and the `fuel.json`
//! reader ([`row`]), the tool helpers ([`AmazonService::tool`], the one
//! nile command builder) and the [`AmazonService`] surface. The rest of the
//! service's bodies are package A's (login, owned list, install, update,
//! verify, uninstall, launch prep); until they land every such call fails
//! with [`StoreError::not_implemented`].
//!
//! Every nile run goes through [`AmazonService::tool`]: its config folder
//! (`NILE_CONFIG_PATH=<data_dir>/stores/nile`; nile appends `/nile`, so its
//! files land in `<data_dir>/stores/nile/nile/`), nile's line parser, and
//! the folder made owner-only before and after the run (nile writes every
//! file with the default umask). Never `-d`: at DEBUG nile logs response
//! bodies.
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
use super::runner::{LineParser, ToolCommand};
use super::tools::{self, StoreTool};
use super::{StoreError, StoreKind};
use crate::launch::StoreLaunchPrep;
use crate::library::queue::{StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;

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

    // --- login and account (package A) ------------------------------------------

    /// `auth --login --non-interactive`: the login URL (checked with
    /// [`AmazonEndpoints::allows_login_url`]) and the values `register`
    /// needs.
    pub async fn start_login(&self) -> Result<AmazonLoginStart, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// `register` with the code and the kept values, then the account.
    pub async fn complete_login(
        &self,
        start: AmazonLoginStart,
        code: SecretString,
    ) -> Result<AmazonAccount, StoreError> {
        drop((start, code));
        Err(StoreError::not_implemented())
    }

    /// The account nile has saved (`auth --status`, local only): `None`
    /// when signed out.
    pub async fn saved_account(&self) -> Result<Option<AmazonAccount>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// Signs out: `auth --logout` (best effort, short timeout), then nile's
    /// whole folder (its token files with it) goes. Step 0 only removes the
    /// folder.
    pub async fn disconnect(&self) -> io::Result<()> {
        paths::remove_tool_dir(&self.data_dir, StoreTool::Nile)
    }

    // --- owned and installed (package A) --------------------------------------

    /// `library sync`, then nile's `library.json`.
    /// [`StoreError::LoginExpired`] when Amazon refuses the saved login,
    /// [`StoreError::Network`] when it can't be reached.
    pub async fn sync(&self) -> Result<Vec<AmazonGame>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// nile's `library.json` as it is (startup, offline).
    pub fn owned_local(&self) -> Vec<AmazonGame> {
        Vec::new()
    }

    /// nile's `installed.json`.
    pub fn installed(&self) -> Vec<AmazonInstalled> {
        Vec::new()
    }

    /// `install --info --json <id>`: the download size in bytes (also the
    /// disk size).
    pub async fn download_size(&self, id: &str) -> Result<u64, StoreError> {
        let _ = id;
        Err(StoreError::not_implemented())
    }

    // --- jobs (package A) ------------------------------------------------------

    pub fn install_job(&self, req: AmazonInstallRequest, ctx: AmazonJobCtx) -> StoreJob {
        let _ = ctx;
        not_implemented_job(req.product_id, req.title, StoreOp::Install)
    }

    pub fn update_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob {
        let _ = ctx;
        not_implemented_job(row.store_id.clone(), row.title.clone(), StoreOp::Update)
    }

    /// Verify and repair (`verify <id>`).
    pub fn verify_job(&self, row: &InstalledGame, ctx: AmazonJobCtx) -> StoreJob {
        let _ = ctx;
        not_implemented_job(row.store_id.clone(), row.title.clone(), StoreOp::Repair)
    }

    /// `uninstall <id>`, then the leftover folder and its prefix.
    pub fn uninstall(&self, registry: &Registry, id: &str) -> Result<(), StoreError> {
        let _ = (registry, id);
        Err(StoreError::not_implemented())
    }

    /// `list-updates --json`, installed ids only.
    pub async fn updates(&self) -> Result<Vec<AmazonUpdate>, StoreError> {
        Err(StoreError::not_implemented())
    }

    /// `launch --json --no-wine <id>`: the executable, arguments and the
    /// five FUEL variables. No network, no token.
    pub async fn launch_prep(&self, row: &InstalledGame) -> Result<StoreLaunchPrep, StoreError> {
        let _ = row;
        Err(StoreError::not_implemented())
    }
}

/// A job that fails with [`StoreError::not_implemented`] (Step 0).
fn not_implemented_job(store_id: String, title: String, op: StoreOp) -> StoreJob {
    StoreJob {
        source: StoreKind::Amazon,
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

    #[tokio::test]
    async fn step_0_calls_say_not_implemented() {
        let dir = tempfile::tempdir().unwrap();
        let svc = AmazonService::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            AmazonEndpoints::real(),
        );
        let error = svc.start_login().await.unwrap_err();
        assert_eq!(error.to_string(), crate::stores::NOT_IMPLEMENTED);
        assert!(svc.sync().await.is_err());
        assert!(svc.saved_account().await.is_err());
        assert!(svc.owned_local().is_empty());
        assert!(svc.installed().is_empty());
        svc.disconnect().await.unwrap();
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
        assert_eq!(job.source, StoreKind::Amazon);
        assert!(job
            .runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .is_err());
    }
}
