use crate::config::Config;
use crate::images::cache::ImageCache;
use crate::romm::{strip_userinfo, RommClient, RommError};
use crate::secrets::{Credential, SecretError, SecretStore};
use secrecy::SecretString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

/// The three-way outcome of [`SessionManager::restore`] (spec "App layer"):
/// no stored session, a live reconnect, or a stored session whose server the
/// probe could not reach.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestoreOutcome {
    NoSession,
    Connected {
        state: SessionState,
    },
    Unreachable {
        server_url: String,
        username: String,
        error: String,
    },
}

pub struct SessionManager {
    config_path: PathBuf,
    secrets: Arc<dyn SecretStore>,
    cache: ImageCache,
    client: Mutex<Option<Arc<RommClient>>>,
    server_url: Mutex<String>,
}

impl SessionManager {
    pub fn new(config_path: PathBuf, cache_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            config_path,
            secrets,
            cache: ImageCache::new(cache_dir),
            client: Mutex::new(None),
            server_url: Mutex::new(String::new()),
        }
    }

    pub fn cache(&self) -> &ImageCache {
        &self.cache
    }

    pub fn client(&self) -> Option<Arc<RommClient>> {
        self.client.lock().unwrap().clone()
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
    /// `self.client` is set only after the probe AND both persistence steps
    /// (config save, credential save) succeed — on any failure path it is
    /// left untouched, so a caller who sees `connect()` return `Err` can
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
        *self.client.lock().unwrap() = Some(Arc::new(client));
        *self.server_url.lock().unwrap() = server_url;
        Ok(state)
    }

    /// Three-way restore (spec "App layer"): no stored session, connected,
    /// or stored-but-unreachable with the probe error's text (SessionError
    /// Display is secret-free by construction). Only config/secret load
    /// failures are `Err`.
    pub async fn restore(&self) -> Result<RestoreOutcome, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        if cfg.server_url.is_empty() {
            return Ok(RestoreOutcome::NoSession);
        }
        let Some(cred) = self.secrets.load()? else {
            return Ok(RestoreOutcome::NoSession);
        };
        // A config written by an older build may still carry userinfo.
        let server_url = strip_userinfo(&cfg.server_url);
        *self.server_url.lock().unwrap() = server_url.clone();
        match self.probe(&server_url, &cfg.username, cred).await {
            Ok((client, state)) => {
                *self.client.lock().unwrap() = Some(Arc::new(client));
                Ok(RestoreOutcome::Connected { state })
            }
            Err(e) => Ok(RestoreOutcome::Unreachable {
                server_url,
                username: cfg.username,
                error: e.to_string(),
            }),
        }
    }

    /// Re-probes with the stored credentials (the chip's Retry). Sets the
    /// stored server URL as soon as it is known non-empty, before the probe
    /// — same placement as `restore`, so the chip's Retry works even after
    /// a fresh start where `restore` itself already failed to connect.
    pub async fn retry(&self) -> Result<SessionState, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        let Some(cred) = self.secrets.load()? else {
            return Err(SessionError::NoStoredSession);
        };
        if cfg.server_url.is_empty() {
            return Err(SessionError::NoStoredSession);
        }
        let server_url = strip_userinfo(&cfg.server_url);
        *self.server_url.lock().unwrap() = server_url.clone();
        let (client, state) = self.probe(&server_url, &cfg.username, cred).await?;
        *self.client.lock().unwrap() = Some(Arc::new(client));
        Ok(state)
    }

    /// Builds a client and probes the server. Does NOT touch `self.client` —
    /// callers decide when (and whether) the probed client becomes the
    /// manager's live connection, after any persistence they require has
    /// succeeded.
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
        *self.client.lock().unwrap() = None;
        self.secrets.clear()?;
        Ok(())
    }
}
