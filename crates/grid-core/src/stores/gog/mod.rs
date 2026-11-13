//! GOG: login and the owned library through GOG's HTTP APIs, install,
//! update, repair and DLC through heroic-gogdl, and GOG cloud saves.
//! `goggame`, `meta` and `row` are shared with the offline installers and
//! the drive backups; the rest is package A's (`docs/stores-phase1-plan.md`):
//! `auth` (login, gogdl's `auth` answers, the in-memory bearer), `api`
//! (GOG's HTTP APIs), `install` (`gogdl info`, the job runners, uninstall),
//! `update` (the update check) and `cloud` (save sync).
//!
//! Secrets: the login code and the bearer token live only in memory as
//! [`SecretString`]; gogdl keeps its own token file under
//! `<data_dir>/stores/gogdl/` (the store-token exception in AGENTS.md).

pub mod api;
pub mod auth;
pub mod cloud;
pub mod goggame;
pub mod install;
pub mod meta;
pub mod row;
pub mod update;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use secrecy::SecretString;
use url::Url;

use super::paths;
use super::runner::{run_tool, ToolCommand};
use super::tools::{self, StoreTool};
use super::StoreError;
use crate::library::queue::StoreJob;
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use auth::{AuthOutput, GogToken};
use meta::GogPlatform;

/// The GOG hosts GRID talks to. Tests and e2e point them at a mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GogEndpoints {
    pub auth: Url,
    pub embed: Url,
    pub api: Url,
    pub content: Url,
    pub cloud: Url,
    /// galaxy-library: the owned list.
    pub library: Url,
    /// gamesdb: product details and art.
    pub gamesdb: Url,
    /// Galaxy remote config: cloud save locations.
    pub remote_config: Url,
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
            library: url("https://galaxy-library.gog.com/"),
            gamesdb: url("https://gamesdb.gog.com/"),
            remote_config: url("https://remote-config.gog.com/"),
        }
    }

    /// Every host at one mock server (e2e, tests). The paths GRID uses on
    /// each host do not overlap.
    pub fn e2e(base: Url) -> GogEndpoints {
        GogEndpoints {
            auth: base.clone(),
            embed: base.clone(),
            api: base.clone(),
            content: base.clone(),
            cloud: base.clone(),
            library: base.clone(),
            gamesdb: base.clone(),
            remote_config: base,
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

/// How many seconds before expiry a cached bearer is refreshed.
const TOKEN_MARGIN: i64 = 120;

/// The GOG service: cheap to clone, shared by the app's commands.
#[derive(Clone)]
pub struct GogService {
    data_dir: PathBuf,
    http: reqwest::Client,
    ep: GogEndpoints,
    /// The Galaxy bearer, in memory only.
    token: Arc<Mutex<Option<GogToken>>>,
    /// The last owned list, for product details in a job's row.
    owned: Arc<Mutex<Vec<GogProduct>>>,
    /// A program run in place of the installed gogdl (unit tests only).
    tool_override: Option<PathBuf>,
}

impl GogService {
    pub fn new(data_dir: PathBuf, http: reqwest::Client, ep: GogEndpoints) -> Self {
        GogService {
            data_dir,
            http,
            ep,
            token: Arc::new(Mutex::new(None)),
            owned: Arc::new(Mutex::new(Vec::new())),
            tool_override: None,
        }
    }

    /// The login page the login window opens.
    pub fn login_url(&self) -> Url {
        auth::login_url(&self.ep)
    }

    /// The login code in GOG's redirect
    /// (`embed.gog.com/on_login_success?code=`), `None` for any other URL.
    pub fn code_from_redirect(&self, url: &Url) -> Option<SecretString> {
        auth::code_from_redirect(&self.ep, url)
    }

    /// Signs in with the code (`gogdl auth --code`).
    pub async fn complete_login(&self, code: SecretString) -> Result<GogAccount, StoreError> {
        let dir = self.tool_dir();
        paths::ensure_private_dir(&dir).map_err(|e| {
            StoreError::Refused(format!("Couldn't create GRID's GOG folder ({}).", e.kind()))
        })?;
        let auth_file = self.auth_file();
        let cmd = self.tool(auth::code_args(&auth_file, &code))?;
        drop(code);
        let exit = self.run_blocking(cmd).await?;
        let _ = paths::make_private_file(&auth_file);
        if !exit.success() {
            return Err(tool_exit_error(exit.code));
        }
        match auth::read_stdout(&exit.stdout, auth::parse_auth_output) {
            Some(AuthOutput::Token(token)) => {
                *self.token.lock().unwrap() = Some(token.clone());
                api::user_data(&self.http, &self.ep, &token.access).await
            }
            Some(AuthOutput::Error) | Some(AuthOutput::Null) => Err(StoreError::Refused(
                "GOG didn't accept the sign-in. Try again.".to_string(),
            )),
            None => Err(StoreError::ToolFailed {
                tool: StoreTool::Gogdl.name(),
                reason: "it gave an answer GRID doesn't understand".to_string(),
            }),
        }
    }

    /// The signed-in account, `None` when signed out;
    /// [`StoreError::LoginExpired`] when the refresh fails.
    pub async fn account(&self) -> Result<Option<GogAccount>, StoreError> {
        if !self.auth_file().is_file() {
            *self.token.lock().unwrap() = None;
            return Ok(None);
        }
        let token = self.bearer().await?;
        match api::user_data(&self.http, &self.ep, &token.access).await {
            Ok(account) => Ok(Some(account)),
            Err(StoreError::LoginExpired) => {
                *self.token.lock().unwrap() = None;
                Err(StoreError::LoginExpired)
            }
            Err(other) => Err(other),
        }
    }

    /// Signs out: deletes gogdl's folder (its token file with it).
    pub fn disconnect(&self) -> io::Result<()> {
        *self.token.lock().unwrap() = None;
        self.owned.lock().unwrap().clear();
        paths::remove_tool_dir(&self.data_dir, StoreTool::Gogdl)
    }

    /// The owned games, DLC folded into their base game.
    pub async fn owned(&self) -> Result<Vec<GogProduct>, StoreError> {
        let token = self.bearer().await?;
        let mut user_id = token.user_id.clone();
        if user_id.is_empty() {
            user_id = api::user_data(&self.http, &self.ep, &token.access)
                .await?
                .user_id;
        }
        let products = api::owned_products(&self.http, &self.ep, &token.access, &user_id).await?;
        *self.owned.lock().unwrap() = products.clone();
        Ok(products)
    }

    /// What can be installed for `id` (`gogdl info`).
    pub async fn install_options(&self, id: &str) -> Result<GogInstallOptions, StoreError> {
        let known = self
            .product(id)
            .map(|p| p.platforms)
            .filter(|p| !p.is_empty());
        let platforms = known.unwrap_or_else(|| vec![GogPlatform::Windows, GogPlatform::Linux]);
        let mut infos = Vec::new();
        let mut last_error = None;
        for platform in platforms {
            let cmd = self.gogdl(install::info_args(id, platform, None))?;
            match self.run_blocking(cmd).await {
                Ok(exit) if exit.success() => {
                    match auth::read_stdout(&exit.stdout, install::parse_info_output) {
                        Some(info) => infos.push((platform, info)),
                        None => last_error = Some(not_understood()),
                    }
                }
                Ok(exit) => last_error = Some(tool_exit_error(exit.code)),
                Err(error) => last_error = Some(error),
            }
        }
        if infos.is_empty() {
            return Err(last_error.unwrap_or_else(|| {
                StoreError::Refused("GOG has no Linux or Windows version of this game.".into())
            }));
        }
        Ok(install::options_from(&infos))
    }

    pub fn install_job(&self, req: GogInstallRequest, ctx: GogJobCtx) -> StoreJob {
        install::install_job(self.clone(), req, ctx)
    }

    pub fn update_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob {
        install::row_job(
            self.clone(),
            row,
            crate::library::queue::StoreOp::Update,
            None,
            ctx,
        )
    }

    pub fn repair_job(&self, row: &InstalledGame, ctx: GogJobCtx) -> StoreJob {
        install::row_job(
            self.clone(),
            row,
            crate::library::queue::StoreOp::Repair,
            None,
            ctx,
        )
    }

    pub fn dlc_job(&self, row: &InstalledGame, dlc_id: &str, ctx: GogJobCtx) -> StoreJob {
        install::row_job(
            self.clone(),
            row,
            crate::library::queue::StoreOp::Dlc,
            Some(dlc_id),
            ctx,
        )
    }

    pub fn remove_dlc(
        &self,
        registry: &Registry,
        row: &InstalledGame,
        dlc_id: &str,
    ) -> Result<(), StoreError> {
        install::remove_dlc(self, registry, row, dlc_id)
    }

    pub fn uninstall(&self, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
        install::uninstall(self, registry, store_id)
    }

    /// Newer builds for `rows` (content-system builds against
    /// `meta.build_id`).
    pub async fn check_updates(
        &self,
        rows: &[InstalledGame],
    ) -> Result<Vec<GogUpdate>, StoreError> {
        update::check_updates(self, rows).await
    }

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

    // --- shared helpers for the submodules -----------------------------------

    /// `<data_dir>/stores/gogdl`.
    pub(crate) fn tool_dir(&self) -> PathBuf {
        paths::tool_dir(&self.data_dir, StoreTool::Gogdl)
    }

    /// gogdl's token file.
    pub(crate) fn auth_file(&self) -> PathBuf {
        self.tool_dir().join(auth::AUTH_FILE)
    }

    /// Where gogdl writes its Windows (content-system) manifests.
    pub(crate) fn manifests_dir(&self) -> PathBuf {
        self.tool_dir().join("heroic_gogdl").join("manifests")
    }

    /// The support files folder gogdl writes for one product.
    pub(crate) fn support_dir(&self, id: &str) -> PathBuf {
        self.tool_dir().join("support").join(id.trim())
    }

    pub(crate) fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub(crate) fn endpoints(&self) -> &GogEndpoints {
        &self.ep
    }

    /// The environment every gogdl run gets: its config folder (manifests),
    /// and on Linux its cache (the vendored xdelta3).
    pub(crate) fn tool_env(&self) -> Vec<(String, String)> {
        let dir = self.tool_dir();
        let mut env = vec![(
            "GOGDL_CONFIG_PATH".to_string(),
            dir.to_string_lossy().into_owned(),
        )];
        if !cfg!(windows) {
            env.push((
                "XDG_CACHE_HOME".to_string(),
                dir.join("cache").to_string_lossy().into_owned(),
            ));
        }
        env
    }

    /// The gogdl command for `args` (a full argv after the program: the
    /// argv builders put `--auth-config-path` first).
    pub(crate) fn tool(&self, args: Vec<String>) -> Result<ToolCommand, StoreError> {
        if let Some(program) = &self.tool_override {
            return Ok(ToolCommand {
                program: program.clone(),
                args,
                env: self.tool_env(),
                cwd: None,
            });
        }
        tools::tool_command(&self.data_dir, StoreTool::Gogdl, args, self.tool_env())
    }

    /// The gogdl command for a subcommand's arguments: `--auth-config-path`
    /// goes in front of them ([`install::gogdl_argv`]).
    pub(crate) fn gogdl(&self, sub: Vec<String>) -> Result<ToolCommand, StoreError> {
        self.tool(install::gogdl_argv(&self.auth_file(), sub))
    }

    /// `run_tool` on the blocking pool, not cancellable.
    pub(crate) async fn run_blocking(
        &self,
        cmd: ToolCommand,
    ) -> Result<super::runner::ToolExit, StoreError> {
        tokio::task::spawn_blocking(move || run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}))
            .await
            .map_err(|_| StoreError::ToolFailed {
                tool: StoreTool::Gogdl.name(),
                reason: "it stopped unexpectedly".to_string(),
            })?
    }

    /// The Galaxy bearer: the cached one while fresh, else gogdl's
    /// (refreshed) credentials. [`StoreError::NotSignedIn`] without a token
    /// file; [`StoreError::LoginExpired`] when the refresh fails.
    pub(crate) async fn bearer(&self) -> Result<GogToken, StoreError> {
        let now = chrono::Utc::now().timestamp();
        if let Some(token) = self.token.lock().unwrap().as_ref() {
            if token.fresh_at(now, TOKEN_MARGIN) {
                return Ok(token.clone());
            }
        }
        if !self.auth_file().is_file() {
            return Err(StoreError::NotSignedIn);
        }
        let cmd = self.tool(auth::credentials_args(&self.auth_file(), None))?;
        let exit = self.run_blocking(cmd).await?;
        let _ = paths::make_private_file(&self.auth_file());
        if !exit.success() {
            return Err(tool_exit_error(exit.code));
        }
        match auth::read_stdout(&exit.stdout, auth::parse_auth_output) {
            Some(AuthOutput::Token(token)) => {
                *self.token.lock().unwrap() = Some(token.clone());
                Ok(token)
            }
            Some(AuthOutput::Null) | Some(AuthOutput::Error) => {
                *self.token.lock().unwrap() = None;
                Err(StoreError::LoginExpired)
            }
            None => Err(not_understood()),
        }
    }

    /// A game client's bearer (cloudstorage): `gogdl auth --client-id
    /// --client-secret`.
    pub(crate) async fn game_bearer(
        &self,
        client_id: &str,
        secret: &SecretString,
    ) -> Result<GogToken, StoreError> {
        let cmd = self.tool(auth::credentials_args(
            &self.auth_file(),
            Some((client_id, secret)),
        ))?;
        let exit = self.run_blocking(cmd).await?;
        let _ = paths::make_private_file(&self.auth_file());
        if !exit.success() {
            return Err(tool_exit_error(exit.code));
        }
        match auth::read_stdout(&exit.stdout, auth::parse_auth_output) {
            Some(AuthOutput::Token(token)) => Ok(token),
            Some(_) => Err(StoreError::LoginExpired),
            None => Err(not_understood()),
        }
    }

    /// The owned product `id` from the last owned list.
    pub(crate) fn product(&self, id: &str) -> Option<GogProduct> {
        self.owned
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.id == id.trim())
            .cloned()
    }

    #[cfg(all(test, unix))]
    pub(crate) fn with_tool_for_test(mut self, program: PathBuf) -> Self {
        self.tool_override = Some(program);
        self
    }

    #[cfg(all(test, unix))]
    pub(crate) fn set_owned_for_test(&self, products: Vec<GogProduct>) {
        *self.owned.lock().unwrap() = products;
    }
}

/// gogdl's exit code in GRID's words.
pub(crate) fn tool_exit_error(code: Option<i32>) -> StoreError {
    let reason = match code {
        Some(2) => "there isn't enough disk space".to_string(),
        Some(1) => "GOG has no build of it for this platform".to_string(),
        Some(code) if code < 0 => format!("it was stopped (signal {})", -code),
        Some(code) => format!("it exited with code {code}"),
        None => "it was stopped".to_string(),
    };
    StoreError::ToolFailed {
        tool: StoreTool::Gogdl.name(),
        reason,
    }
}

fn not_understood() -> StoreError {
    StoreError::ToolFailed {
        tool: StoreTool::Gogdl.name(),
        reason: "it gave an answer GRID doesn't understand".to_string(),
    }
}

/// A store error as the queue's error.
pub(crate) fn library_error(error: StoreError) -> LibraryError {
    match error {
        StoreError::Library(inner) => inner,
        other => LibraryError::Extract(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e2e_points_every_host_at_the_mock() {
        let base = Url::parse("http://127.0.0.1:9/").unwrap();
        let ep = GogEndpoints::e2e(base.clone());
        for url in [
            &ep.auth,
            &ep.embed,
            &ep.api,
            &ep.content,
            &ep.cloud,
            &ep.library,
            &ep.gamesdb,
            &ep.remote_config,
        ] {
            assert_eq!(url, &base);
        }
        assert_eq!(
            GogEndpoints::real().gamesdb.host_str(),
            Some("gamesdb.gog.com")
        );
    }

    #[test]
    fn exit_codes_read_as_grids_words() {
        assert!(tool_exit_error(Some(2)).to_string().contains("disk space"));
        assert!(tool_exit_error(Some(1)).to_string().contains("no build"));
        assert!(tool_exit_error(Some(-9)).to_string().contains("signal 9"));
        assert!(library_error(StoreError::Library(LibraryError::Cancelled))
            .to_string()
            .contains("cancelled"));
    }

    /// A fake gogdl `auth`: with `--code` it writes the token file and
    /// prints a token; without, it prints `$(cat refresh)` (a token, or
    /// `null` for a failed refresh). Its argv is recorded.
    #[cfg(unix)]
    fn fake_auth(dir: &std::path::Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("gogdl.sh");
        let token = r#"{"access_token": "FAKE-STORE-TOKEN-not-real", "expires_in": 3600, "user_id": "4862", "loginTime": 4000000000}"#;
        let body = format!(
            r#"#!/bin/sh
echo "$@" >> '{log}'
auth="$2"
case "$*" in
  *--code*) printf '%s' '{token}' > "$auth"; printf '%s\n' '{token}';;
  *) cat '{refresh}';;
esac
"#,
            log = dir.join("argv.log").to_string_lossy(),
            refresh = dir.join("refresh").to_string_lossy(),
        );
        std::fs::write(&script, body).unwrap();
        std::fs::write(dir.join("refresh"), token).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn login_account_and_an_expired_refresh() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/userData.json"))
            .and(header("Authorization", "Bearer FAKE-STORE-TOKEN-not-real"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"username": "fakeuser", "galaxyUserId": "4862"}),
                ),
            )
            .mount(&server)
            .await;
        let root = tempfile::tempdir().unwrap();
        let ep = GogEndpoints::e2e(Url::parse(&format!("{}/", server.uri())).unwrap());
        let svc = GogService::new(root.path().join("data"), reqwest::Client::new(), ep)
            .with_tool_for_test(fake_auth(root.path()));

        assert_eq!(
            svc.account().await.unwrap(),
            None,
            "no token file: signed out"
        );
        let redirect = Url::parse(&format!(
            "{}/on_login_success?code=FAKE-STORE-TOKEN-not-real",
            server.uri()
        ))
        .unwrap();
        let code = svc.code_from_redirect(&redirect).unwrap();
        let account = svc.complete_login(code).await.unwrap();
        assert_eq!(account.username, "fakeuser");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(svc.auth_file())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
            let dir_mode = std::fs::metadata(svc.tool_dir())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(dir_mode & 0o777, 0o700);
        }
        let argv = std::fs::read_to_string(root.path().join("argv.log")).unwrap();
        assert!(argv.starts_with("--auth-config-path "), "{argv}");
        assert!(argv.contains(" auth --code "), "{argv}");

        // A fresh in-memory token answers without gogdl.
        assert_eq!(svc.account().await.unwrap().unwrap().user_id, "4862");

        // A failed refresh is LoginExpired.
        *svc.token.lock().unwrap() = None;
        std::fs::write(root.path().join("refresh"), "null\n").unwrap();
        let error = svc.account().await.unwrap_err();
        assert!(matches!(error, StoreError::LoginExpired), "{error}");

        svc.disconnect().unwrap();
        assert!(!svc.tool_dir().exists());
        assert_eq!(svc.account().await.unwrap(), None);
    }

    #[test]
    fn the_tool_env_names_gogdls_folders() {
        let svc = GogService::new(
            PathBuf::from("/data"),
            reqwest::Client::new(),
            GogEndpoints::real(),
        );
        let env = svc.tool_env();
        assert_eq!(env[0].0, "GOGDL_CONFIG_PATH");
        assert!(PathBuf::from(&env[0].1).ends_with("stores/gogdl"));
        assert!(svc
            .manifests_dir()
            .ends_with("gogdl/heroic_gogdl/manifests"));
    }
}
