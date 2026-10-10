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

/// What the unauthorized hook receives when the server answers 401 in the
/// middle of a session: enough to pre-fill the Connect form, and no secret.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SessionUnauthorized {
    pub server_url: String,
    pub username: String,
    pub auth_kind: AuthKind,
}

/// Called at most once per live session, the first time a request on it
/// gets a 401. Set by the app layer (the Tauri shell emits an event).
pub type UnauthorizedHook = Arc<dyn Fn(SessionUnauthorized) + Send + Sync>;

/// The outcome of [`SessionManager::restore`] and [`SessionManager::retry`]
/// (spec "App layer"): no stored session, a live reconnect, a stored
/// credential the server rejected (401), or a stored session whose server
/// the probe could not use (offline, 403, 5xx, ...).
///
/// `NoSession` still carries whatever the config holds so the Connect form
/// can prefill them — the Python importer writes `server_url`/`username`
/// but no credential, and that user must retype nothing but their token.
/// Both are blank when there is no config at all.
///
/// `Unauthorized` keeps the keyring credential (user decision Q6): it is
/// replaced only when a new connect succeeds.
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

pub struct SessionManager {
    config_path: PathBuf,
    secrets: Arc<dyn SecretStore>,
    cache: ImageCache,
    live: Arc<Mutex<Live>>,
    server_url: Mutex<String>,
    unauthorized_hook: Arc<Mutex<Option<UnauthorizedHook>>>,
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

    /// The stored server URL: set in `connect` once the session is fully
    /// persisted, and in `restore` as soon as a non-empty URL is read from
    /// config — before that probe runs, so a restore whose probe fails still
    /// leaves this populated (image URL filtering needs it regardless of
    /// live-connection state).
    pub fn server_url(&self) -> String {
        self.server_url.lock().unwrap().clone()
    }

    /// `use_token`: true = `secret` is an API token; false = it is the
    /// account password (HTTP basic). On success the config and credential
    /// are persisted; the plain secret is consumed and dropped here.
    ///
    /// The live client is set only after the probe AND both persistence
    /// steps (config save, credential save) succeed — on any failure path it
    /// is left untouched, so a caller who sees `connect()` return `Err` can
    /// never observe `client()` reporting a live connection.
    pub async fn connect(
        &self,
        server_url: String,
        username: String,
        secret: SecretString,
        use_token: bool,
    ) -> Result<SessionState, SessionError> {
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
        cfg.save(&self.config_path)?;
        self.secrets.save(&cred)?;
        self.install(client, &state, auth_kind);
        *self.server_url.lock().unwrap() = server_url;
        Ok(state)
    }

    /// Restore at startup (spec "App layer"): no stored session, connected,
    /// rejected (401), or stored-but-unusable with the probe error's text
    /// (SessionError Display is secret-free by construction). Only
    /// config/secret load failures are `Err`.
    pub async fn restore(&self) -> Result<RestoreOutcome, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        if cfg.server_url.is_empty() {
            return Ok(RestoreOutcome::NoSession {
                server_url: String::new(),
                username: cfg.username,
            });
        }
        let Some(cred) = self.secrets.load()? else {
            return Ok(RestoreOutcome::NoSession {
                server_url: strip_userinfo(&cfg.server_url),
                username: cfg.username,
            });
        };
        Ok(self.reprobe(cfg, cred).await)
    }

    /// Re-probes with the stored credentials (the chip's Retry). Answers
    /// with the same outcomes as `restore`, except that a missing stored
    /// session is `Err(NoStoredSession)`. Sets the stored server URL as soon
    /// as it is known non-empty, before the probe — same placement as
    /// `restore`, so the chip's Retry works even after a fresh start where
    /// `restore` itself already failed to connect.
    pub async fn retry(&self) -> Result<RestoreOutcome, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        let Some(cred) = self.secrets.load()? else {
            return Err(SessionError::NoStoredSession);
        };
        if cfg.server_url.is_empty() {
            return Err(SessionError::NoStoredSession);
        }
        Ok(self.reprobe(cfg, cred).await)
    }

    /// The probe shared by `restore` and `retry`, for a config with a
    /// non-empty server URL and a stored credential.
    async fn reprobe(&self, cfg: Config, cred: Credential) -> RestoreOutcome {
        // A config written by an older build may still carry userinfo.
        let server_url = strip_userinfo(&cfg.server_url);
        *self.server_url.lock().unwrap() = server_url.clone();
        let auth_kind = AuthKind::of(&cred);
        match self.probe(&server_url, &cfg.username, cred).await {
            Ok((client, state)) => {
                self.install(client, &state, auth_kind);
                RestoreOutcome::Connected { state }
            }
            Err(SessionError::Romm(RommError::Unauthorized)) => RestoreOutcome::Unauthorized {
                server_url,
                username: cfg.username,
                auth_kind,
            },
            Err(e) => RestoreOutcome::Unreachable {
                server_url,
                username: cfg.username,
                error: e.to_string(),
            },
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
        let hook_slot = self.unauthorized_hook.clone();
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
            // Outside the lock: the hook may call back into the manager.
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

    pub fn disconnect(&self) -> Result<(), SessionError> {
        {
            let mut live = self.live.lock().unwrap();
            live.client = None;
            live.generation += 1;
        }
        self.secrets.clear()?;
        Ok(())
    }
}
