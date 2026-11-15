use crate::config::Config;
use crate::images::cache::ImageCache;
use crate::romm::{strip_userinfo, RommClient, RommError};
use crate::secrets::{Credential, SecretError, SecretStore};
use secrecy::SecretString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Romm(#[from] RommError),
    #[error("config: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("secrets: {0}")]
    Secrets(#[from] SecretError),
    #[error("the token belongs to account '{actual}', not '{entered}'")]
    UsernameMismatch { entered: String, actual: String },
    #[error("no stored session")]
    NoStoredSession,
}

/// The only session shape that may cross the IPC boundary. No secrets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionState {
    pub connected: bool,
    pub username: String,
    pub server_url: String,
}

/// Which kind of credential the keyring holds — the variant only, never its
/// content. The Connect form uses it to show the token or the password field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    Token,
    Basic,
}

impl AuthKind {
    fn of(cred: &Credential) -> Self {
        match cred {
            Credential::Token(_) => AuthKind::Token,
            Credential::Basic { .. } => AuthKind::Basic,
        }
    }
}

/// RomM's connection, as its Library group and its Settings card show it
/// (romm-storefront-plan 0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RommConnection {
    /// A live client: the probe answered.
    Connected,
    /// A stored connection whose server the probe could not use (network
    /// error, 403, 5xx, ...). `last_error` says why.
    Unreachable,
    /// The server rejected the stored credential (401), at a probe or
    /// mid-session. The keyring credential is kept.
    Expired,
    /// Never connected, disconnected, or connected but not probed because
    /// `stores.romm.connected` is false.
    NotConnected,
}

/// RomM's card and rail group. No secret, ever.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RommStatus {
    pub connection: RommConnection,
    /// Credential-free (`strip_userinfo`); "" when none was ever stored.
    pub server_url: String,
    pub username: String,
    /// Which credential kind the sign-in form asks for: the keyring's kind when
    /// known (connect, restore, a 401), else None (form defaults to token).
    pub auth_kind: Option<AuthKind>,
    /// Unreachable: the probe's secret-free reason. "" otherwise.
    pub last_error: String,
    /// Installed RomM rows (source Romm, not the hidden Emulators platform).
    pub installed_count: u32,
    /// connection != NotConnected || installed_count > 0 (decisions 5 and 11).
    pub in_rail: bool,
    /// A probe (restore, reconnect, connect) is running, or the startup
    /// restore of a stored connection has not run yet.
    pub busy: bool,
}

/// What the unauthorized hook receives when the server answers 401 in the
/// middle of a session: enough to pre-fill the Connect form, and no secret.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SessionUnauthorized {
    pub server_url: String,
    pub username: String,
    pub auth_kind: AuthKind,
}

/// Called at most once per live session, the first time a request on it
/// gets a 401. Set by the app layer. Legacy: the old Connect screen listens
/// for it; the RomM storefront reads [`RommStatus`] through the change hook.
pub type UnauthorizedHook = Arc<dyn Fn(SessionUnauthorized) + Send + Sync>;

/// Called after every change to [`RommStatus`] (connection, account, busy).
/// Never while a session lock is held, so it may call back into the manager.
pub type ChangeHook = Arc<dyn Fn() + Send + Sync>;

/// The outcome of [`SessionManager::restore`] and [`SessionManager::retry`]
/// for the legacy IPC (`restore_session`, `retry_connect`), built from the
/// RomM state and the config. Removed with those commands
/// (romm-storefront-plan Step 9).
///
/// `NoSession` still carries whatever the config holds so the Connect form
/// can prefill them — the Python importer writes `server_url`/`username`
/// but no credential, and that user must retype nothing but their token.
/// Both are blank when there is no config at all.
///
/// `Unauthorized` keeps the keyring credential (user decision Q6): it is
/// replaced only when a new connect succeeds.
///
/// `Offline`: RomM is not connected (`stores.romm.connected` false) and the
/// welcome is done, so the old UI opens its "no server" shell. No probe ran
/// and the keyring was not read.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestoreOutcome {
    NoSession {
        server_url: String,
        username: String,
    },
    Connected {
        state: SessionState,
    },
    Unauthorized {
        server_url: String,
        username: String,
        auth_kind: AuthKind,
    },
    Unreachable {
        server_url: String,
        username: String,
        error: String,
    },
    Offline {
        server_url: String,
        username: String,
    },
}

/// The live client and a counter that changes every time the live client
/// does. A 401 acts on the session only when the client that saw it is
/// still the live one, so a late answer to an old client cannot end the
/// session that replaced it.
#[derive(Default)]
struct Live {
    client: Option<Arc<RommClient>>,
    generation: u64,
}

/// What [`RommStatus`] is built from, except the installed count (the app
/// layer counts registry rows; the session does not read the registry).
#[derive(Debug, Clone)]
struct RommState {
    connection: RommConnection,
    server_url: String,
    username: String,
    auth_kind: Option<AuthKind>,
    last_error: String,
    /// Probes running now.
    probes: u32,
    /// `prime` saw a stored connection and `restore` has not finished yet.
    restore_pending: bool,
}

impl Default for RommState {
    fn default() -> Self {
        Self {
            connection: RommConnection::NotConnected,
            server_url: String::new(),
            username: String::new(),
            auth_kind: None,
            last_error: String::new(),
            probes: 0,
            restore_pending: false,
        }
    }
}

/// What one probe with the stored or typed credential found.
enum Probe {
    Connected(SessionState, AuthKind),
    Expired(AuthKind),
    Unreachable(String),
}

pub struct SessionManager {
    config_path: PathBuf,
    secrets: Arc<dyn SecretStore>,
    cache: ImageCache,
    live: Arc<Mutex<Live>>,
    server_url: Mutex<String>,
    unauthorized_hook: Arc<Mutex<Option<UnauthorizedHook>>>,
    romm: Arc<Mutex<RommState>>,
    change_hook: Arc<Mutex<Option<ChangeHook>>>,
}

/// Runs the change hook, if one is set. Never call it with a lock held.
fn fire(slot: &Mutex<Option<ChangeHook>>) {
    let hook = slot.lock().unwrap().clone();
    if let Some(hook) = hook {
        hook();
    }
}

impl SessionManager {
    pub fn new(config_path: PathBuf, cache_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            config_path,
            secrets,
            cache: ImageCache::new(cache_dir),
            live: Arc::new(Mutex::new(Live::default())),
            server_url: Mutex::new(String::new()),
            unauthorized_hook: Arc::new(Mutex::new(None)),
            romm: Arc::new(Mutex::new(RommState::default())),
            change_hook: Arc::new(Mutex::new(None)),
        }
    }

    pub fn cache(&self) -> &ImageCache {
        &self.cache
    }

    pub fn client(&self) -> Option<Arc<RommClient>> {
        self.live.lock().unwrap().client.clone()
    }

    /// Sets what runs when a live session's credential is rejected (401).
    /// It runs once per session: the first 401 drops the live client (so
    /// `client()` reports no connection) and later 401s on that session do
    /// nothing. A 401 on a connect/restore/retry probe never runs it — those
    /// report through their own return value. The keyring credential is
    /// kept.
    pub fn set_unauthorized_hook(&self, hook: UnauthorizedHook) {
        *self.unauthorized_hook.lock().unwrap() = Some(hook);
    }

    /// Sets what runs after every [`RommStatus`] change: connect,
    /// disconnect, reconnect, restore, a mid-session 401, and a probe
    /// starting or ending (`busy`). The app layer emits `romm-changed`.
    pub fn set_change_hook(&self, hook: ChangeHook) {
        *self.change_hook.lock().unwrap() = Some(hook);
    }

    /// RomM's status for the card and the rail. `installed_count` is the
    /// app layer's count of installed RomM rows.
    pub fn status(&self, installed_count: u32) -> RommStatus {
        let s = self.romm.lock().unwrap().clone();
        RommStatus {
            in_rail: s.connection != RommConnection::NotConnected || installed_count > 0,
            connection: s.connection,
            server_url: s.server_url,
            username: s.username,
            auth_kind: s.auth_kind,
            last_error: s.last_error,
            installed_count,
            busy: s.probes > 0 || s.restore_pending,
        }
    }

    /// Startup, before any window asks: reads the config only (never the
    /// keyring). Sets the stored server URL for image filtering, the
    /// account the card shows, and — when `stores.romm.connected` — marks
    /// the restore as pending, so the status reads "connecting" until
    /// [`SessionManager::restore_romm`] has run.
    pub fn prime(&self) -> Result<(), SessionError> {
        let cfg = Config::load(&self.config_path)?;
        let server_url = strip_userinfo(&cfg.server_url);
        if !server_url.is_empty() {
            *self.server_url.lock().unwrap() = server_url.clone();
        }
        {
            let mut s = self.romm.lock().unwrap();
            s.server_url = server_url.clone();
            s.username = cfg.username.clone();
            s.restore_pending = cfg.stores.romm.connected && !server_url.is_empty();
        }
        fire(&self.change_hook);
        Ok(())
    }

    /// The stored server URL: set in `connect` once the session is fully
    /// persisted, and in `restore` as soon as a non-empty URL is read from
    /// config — before that probe runs, so a restore whose probe fails still
    /// leaves this populated (image URL filtering needs it regardless of
    /// live-connection state).
    pub fn server_url(&self) -> String {
        self.server_url.lock().unwrap().clone()
    }

    fn begin_probe(&self) {
        self.romm.lock().unwrap().probes += 1;
        fire(&self.change_hook);
    }

    /// Ends one probe, applying `change` to the state in the same step, then
    /// fires the change hook once.
    fn end_probe(&self, change: impl FnOnce(&mut RommState)) {
        {
            let mut s = self.romm.lock().unwrap();
            s.probes = s.probes.saturating_sub(1);
            change(&mut s);
        }
        fire(&self.change_hook);
    }

    /// `use_token`: true = `secret` is an API token; false = it is the
    /// account password (HTTP basic). On success the config and credential
    /// are persisted; the plain secret is consumed and dropped here. The
    /// config gets `stores.romm.connected = true` in the same save, so the
    /// next start probes again.
    ///
    /// The live client is set only after the probe AND both persistence
    /// steps (config save, credential save) succeed — on any failure path it
    /// is left untouched, so a caller who sees `connect()` return `Err` can
    /// never observe `client()` reporting a live connection. A failed
    /// connect leaves the RomM status as it was (only `busy` changes).
    pub async fn connect(
        &self,
        server_url: String,
        username: String,
        secret: SecretString,
        use_token: bool,
    ) -> Result<SessionState, SessionError> {
        self.begin_probe();
        let result = self
            .connect_inner(server_url, username, secret, use_token)
            .await;
        match &result {
            Ok((state, auth_kind)) => {
                let (state, auth_kind) = (state.clone(), *auth_kind);
                self.end_probe(move |s| {
                    s.connection = RommConnection::Connected;
                    s.server_url = state.server_url;
                    s.username = state.username;
                    s.auth_kind = Some(auth_kind);
                    s.last_error.clear();
                });
            }
            Err(_) => self.end_probe(|_| {}),
        }
        result.map(|(state, _)| state)
    }

    async fn connect_inner(
        &self,
        server_url: String,
        username: String,
        secret: SecretString,
        use_token: bool,
    ) -> Result<(SessionState, AuthKind), SessionError> {
        let cred = if use_token {
            Credential::Token(secret)
        } else {
            Credential::Basic {
                username: username.clone(),
                password: secret,
            }
        };
        let auth_kind = AuthKind::of(&cred);
        // Normalise once, at the session boundary: everything downstream —
        // the probe, `SessionState`, config, and the base used for image host
        // filtering — sees the same credential-free URL.
        let server_url = strip_userinfo(&server_url);
        let (client, state) = self.probe(&server_url, &username, cred.clone()).await?;
        // Token auth never sends the username, so a typed one is only a claim.
        // The server-reported account is the truth: a non-empty mismatching
        // claim is rejected before anything persists, and the config stores
        // the verified name, never the typed one.
        if use_token {
            let entered = username.trim();
            if !entered.is_empty()
                && !state.username.is_empty()
                && !entered.eq_ignore_ascii_case(&state.username)
            {
                return Err(SessionError::UsernameMismatch {
                    entered: entered.to_string(),
                    actual: state.username.clone(),
                });
            }
        }
        let mut cfg = Config::load(&self.config_path)?;
        cfg.server_url = server_url.clone();
        cfg.username = state.username.clone();
        cfg.stores.romm.connected = true;
        cfg.save(&self.config_path)?;
        self.secrets.save(&cred)?;
        self.install(client, &state, auth_kind);
        *self.server_url.lock().unwrap() = server_url;
        Ok((state, auth_kind))
    }

    /// Restore at startup (`romm_restore`). Probes only when
    /// `stores.romm.connected`: otherwise RomM is "Not connected", and
    /// neither the keyring nor any server is touched. The stored server URL
    /// is set from config whenever one is stored (image filtering and cache
    /// lookups need it). Only config/secret load failures are `Err`; a
    /// failed probe is `Unreachable` or `Expired` in the status.
    pub async fn restore_romm(&self) -> Result<(), SessionError> {
        self.restore_inner().await.map(|_| ())
    }

    /// The legacy restore (`restore_session`): [`Self::restore_romm`], then
    /// the outcome the old Connect/shell flow expects.
    pub async fn restore(&self) -> Result<RestoreOutcome, SessionError> {
        let (cfg, probe) = self.restore_inner().await?;
        let server_url = strip_userinfo(&cfg.server_url);
        let username = cfg.username.clone();
        Ok(match probe {
            Some(probe) => legacy_outcome(probe, server_url, username),
            None if !cfg.stores.romm.connected && cfg.setup.welcomed => RestoreOutcome::Offline {
                server_url,
                username,
            },
            None => RestoreOutcome::NoSession {
                server_url,
                username,
            },
        })
    }

    async fn restore_inner(&self) -> Result<(Config, Option<Probe>), SessionError> {
        self.begin_probe();
        let result = self.restore_probe().await;
        match &result {
            Ok((cfg, probe)) => {
                let server_url = strip_userinfo(&cfg.server_url);
                let username = cfg.username.clone();
                let applied = probe.as_ref().map(probe_change);
                self.end_probe(move |s| {
                    s.restore_pending = false;
                    s.server_url = server_url;
                    s.username = username;
                    match applied {
                        Some(apply) => apply(s),
                        None => {
                            s.connection = RommConnection::NotConnected;
                            s.last_error.clear();
                        }
                    }
                });
            }
            Err(_) => self.end_probe(|s| s.restore_pending = false),
        }
        result
    }

    async fn restore_probe(&self) -> Result<(Config, Option<Probe>), SessionError> {
        let cfg = Config::load(&self.config_path)?;
        let server_url = strip_userinfo(&cfg.server_url);
        if !server_url.is_empty() {
            *self.server_url.lock().unwrap() = server_url.clone();
        }
        // Not connected: no keyring read, no request (plan risk "Keyring
        // prompts": an offline user must not get an unlock prompt).
        if !cfg.stores.romm.connected || server_url.is_empty() {
            return Ok((cfg, None));
        }
        let Some(cred) = self.secrets.load()? else {
            return Ok((cfg, None));
        };
        let probe = self.reprobe(&cfg, cred).await;
        Ok((cfg, Some(probe)))
    }

    /// Re-probes with the stored credential (`romm_reconnect`, the card's
    /// Retry and Reconnect). A missing stored session is
    /// `Err(NoStoredSession)`. A reconnect that connects sets
    /// `stores.romm.connected`, so the next start probes again.
    pub async fn reconnect(&self) -> Result<(), SessionError> {
        self.retry_inner().await.map(|_| ())
    }

    /// The legacy retry (`retry_connect`): [`Self::reconnect`], answering
    /// with the legacy outcome.
    pub async fn retry(&self) -> Result<RestoreOutcome, SessionError> {
        let (cfg, probe) = self.retry_inner().await?;
        Ok(legacy_outcome(
            probe,
            strip_userinfo(&cfg.server_url),
            cfg.username,
        ))
    }

    async fn retry_inner(&self) -> Result<(Config, Probe), SessionError> {
        self.begin_probe();
        let result = self.retry_probe().await;
        match &result {
            Ok((cfg, probe)) => {
                let server_url = strip_userinfo(&cfg.server_url);
                let username = cfg.username.clone();
                let apply = probe_change(probe);
                self.end_probe(move |s| {
                    s.server_url = server_url;
                    s.username = username;
                    apply(s);
                });
            }
            Err(_) => self.end_probe(|_| {}),
        }
        result
    }

    async fn retry_probe(&self) -> Result<(Config, Probe), SessionError> {
        let cfg = Config::load(&self.config_path)?;
        let Some(cred) = self.secrets.load()? else {
            return Err(SessionError::NoStoredSession);
        };
        if cfg.server_url.is_empty() {
            return Err(SessionError::NoStoredSession);
        }
        let probe = self.reprobe(&cfg, cred).await;
        if matches!(probe, Probe::Connected(..)) && !cfg.stores.romm.connected {
            // Re-read: the probe awaited, and another writer may have saved.
            let mut fresh = Config::load(&self.config_path)?;
            fresh.stores.romm.connected = true;
            fresh.save(&self.config_path)?;
            return Ok((fresh, probe));
        }
        Ok((cfg, probe))
    }

    /// Legacy "Use without a server": RomM stops being probed at startup
    /// (`stores.romm.connected = false`) and the welcome counts as done
    /// (`setup.welcomed = true`), so later starts open the shell with RomM
    /// not connected. The keyring credential is left untouched (decision
    /// 12). Any live client is dropped, so nothing keeps talking to the
    /// server.
    pub fn use_offline(&self) -> Result<(), SessionError> {
        let mut cfg = Config::load(&self.config_path)?;
        cfg.stores.romm.connected = false;
        cfg.setup.welcomed = true;
        cfg.save(&self.config_path)?;
        self.drop_live();
        let server_url = strip_userinfo(&cfg.server_url);
        if !server_url.is_empty() {
            *self.server_url.lock().unwrap() = server_url.clone();
        }
        {
            let mut s = self.romm.lock().unwrap();
            s.connection = RommConnection::NotConnected;
            s.server_url = server_url;
            s.username = cfg.username;
            s.last_error.clear();
        }
        fire(&self.change_hook);
        Ok(())
    }

    fn drop_live(&self) {
        let mut live = self.live.lock().unwrap();
        live.client = None;
        live.generation += 1;
    }

    /// The probe shared by `restore` and `retry`, for a config with a
    /// non-empty server URL and a stored credential.
    async fn reprobe(&self, cfg: &Config, cred: Credential) -> Probe {
        // A config written by an older build may still carry userinfo.
        let server_url = strip_userinfo(&cfg.server_url);
        *self.server_url.lock().unwrap() = server_url.clone();
        let auth_kind = AuthKind::of(&cred);
        match self.probe(&server_url, &cfg.username, cred).await {
            Ok((client, state)) => {
                self.install(client, &state, auth_kind);
                Probe::Connected(state, auth_kind)
            }
            Err(SessionError::Romm(RommError::Unauthorized)) => Probe::Expired(auth_kind),
            Err(e) => Probe::Unreachable(e.to_string()),
        }
    }

    /// Makes `client` the live client and arms its 401 hook for this
    /// session only.
    fn install(&self, mut client: RommClient, state: &SessionState, auth_kind: AuthKind) {
        let mut live = self.live.lock().unwrap();
        live.generation += 1;
        let generation = live.generation;
        let info = SessionUnauthorized {
            server_url: state.server_url.clone(),
            username: state.username.clone(),
            auth_kind,
        };
        // Weak, so the client (held by `live`) does not keep `live` alive
        // through its own hook.
        let live_slot: Weak<Mutex<Live>> = Arc::downgrade(&self.live);
        let romm_slot: Weak<Mutex<RommState>> = Arc::downgrade(&self.romm);
        let hook_slot = self.unauthorized_hook.clone();
        let change_slot = self.change_hook.clone();
        client.set_unauthorized_hook(Arc::new(move || {
            let Some(live_slot) = live_slot.upgrade() else {
                return;
            };
            {
                let mut live = live_slot.lock().unwrap();
                if live.generation != generation || live.client.is_none() {
                    return;
                }
                live.client = None;
            }
            // The card and the rail show "Sign-in expired"; the keyring
            // credential stays until a new connect replaces it.
            if let Some(romm_slot) = romm_slot.upgrade() {
                let mut s = romm_slot.lock().unwrap();
                s.connection = RommConnection::Expired;
                s.auth_kind = Some(auth_kind);
                s.last_error.clear();
            }
            // Outside the locks: the hooks may call back into the manager.
            fire(&change_slot);
            let hook = hook_slot.lock().unwrap().clone();
            if let Some(hook) = hook {
                hook(info.clone());
            }
        }));
        live.client = Some(Arc::new(client));
    }

    /// Builds a client and probes the server. Does NOT touch the live
    /// client — callers decide when (and whether) the probed client becomes
    /// the manager's live connection, after any persistence they require
    /// has succeeded. The probe client has no 401 hook.
    async fn probe(
        &self,
        server_url: &str,
        username: &str,
        cred: Credential,
    ) -> Result<(RommClient, SessionState), SessionError> {
        let client = RommClient::new(server_url, cred)?;
        let user = client.connect().await?;
        let state = SessionState {
            connected: true,
            username: if user.username.is_empty() {
                username.to_string()
            } else {
                user.username
            },
            server_url: server_url.to_string(),
        };
        Ok((client, state))
    }

    /// Disconnect RomM (`romm_disconnect`, decision 5): drops the client,
    /// deletes the keyring credential and sets `stores.romm.connected =
    /// false`. The server URL and username stay in the config, for the
    /// card and the sign-in form to prefill.
    pub fn disconnect(&self) -> Result<(), SessionError> {
        self.drop_live();
        self.secrets.clear()?;
        let mut cfg = Config::load(&self.config_path)?;
        if cfg.stores.romm.connected {
            cfg.stores.romm.connected = false;
            cfg.save(&self.config_path)?;
        }
        {
            let mut s = self.romm.lock().unwrap();
            s.connection = RommConnection::NotConnected;
            s.auth_kind = None;
            s.last_error.clear();
            s.server_url = strip_userinfo(&cfg.server_url);
            s.username = cfg.username;
        }
        fire(&self.change_hook);
        Ok(())
    }
}

/// The state change one probe result makes.
fn probe_change(probe: &Probe) -> Box<dyn FnOnce(&mut RommState) + Send> {
    match probe {
        Probe::Connected(state, kind) => {
            let (state, kind) = (state.clone(), *kind);
            Box::new(move |s| {
                s.connection = RommConnection::Connected;
                s.auth_kind = Some(kind);
                s.username = state.username;
                s.last_error.clear();
            })
        }
        Probe::Expired(kind) => {
            let kind = *kind;
            Box::new(move |s| {
                s.connection = RommConnection::Expired;
                s.auth_kind = Some(kind);
                s.last_error.clear();
            })
        }
        Probe::Unreachable(error) => {
            let error = error.clone();
            Box::new(move |s| {
                s.connection = RommConnection::Unreachable;
                s.last_error = error;
            })
        }
    }
}

fn legacy_outcome(probe: Probe, server_url: String, username: String) -> RestoreOutcome {
    match probe {
        Probe::Connected(state, _) => RestoreOutcome::Connected { state },
        Probe::Expired(auth_kind) => RestoreOutcome::Unauthorized {
            server_url,
            username,
            auth_kind,
        },
        Probe::Unreachable(error) => RestoreOutcome::Unreachable {
            server_url,
            username,
            error,
        },
    }
}
