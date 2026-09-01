use crate::config::Config;
use crate::covers::CoverCache;
use crate::romm::{RommClient, RommError};
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
}

/// The only session shape that may cross the IPC boundary. No secrets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionState {
    pub connected: bool,
    pub username: String,
    pub server_url: String,
}

pub struct SessionManager {
    config_path: PathBuf,
    secrets: Arc<dyn SecretStore>,
    cache: CoverCache,
    client: Mutex<Option<Arc<RommClient>>>,
}

impl SessionManager {
    pub fn new(config_path: PathBuf, cache_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            config_path,
            secrets,
            cache: CoverCache::new(cache_dir),
            client: Mutex::new(None),
        }
    }

    pub fn cache(&self) -> &CoverCache {
        &self.cache
    }

    pub fn client(&self) -> Option<Arc<RommClient>> {
        self.client.lock().unwrap().clone()
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
        cfg.server_url = server_url;
        cfg.username = state.username.clone();
        cfg.save(&self.config_path)?;
        self.secrets.save(&cred)?;
        *self.client.lock().unwrap() = Some(Arc::new(client));
        Ok(state)
    }

    pub async fn restore(&self) -> Result<Option<SessionState>, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        if cfg.server_url.is_empty() {
            return Ok(None);
        }
        let Some(cred) = self.secrets.load()? else {
            return Ok(None);
        };
        let (client, state) = self.probe(&cfg.server_url, &cfg.username, cred).await?;
        *self.client.lock().unwrap() = Some(Arc::new(client));
        Ok(Some(state))
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
