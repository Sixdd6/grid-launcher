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
        let state = self
            .try_connect(&server_url, &username, cred.clone())
            .await?;
        let mut cfg = Config::load(&self.config_path)?;
        cfg.server_url = server_url;
        cfg.username = username;
        cfg.save(&self.config_path)?;
        self.secrets.save(&cred)?;
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
        Ok(Some(
            self.try_connect(&cfg.server_url, &cfg.username, cred)
                .await?,
        ))
    }

    async fn try_connect(
        &self,
        server_url: &str,
        username: &str,
        cred: Credential,
    ) -> Result<SessionState, SessionError> {
        let client = RommClient::new(server_url, cred)?;
        let user = client.connect().await?;
        *self.client.lock().unwrap() = Some(Arc::new(client));
        Ok(SessionState {
            connected: true,
            username: if user.username.is_empty() {
                username.to_string()
            } else {
                user.username
            },
            server_url: server_url.to_string(),
        })
    }

    pub fn disconnect(&self) -> Result<(), SessionError> {
        *self.client.lock().unwrap() = None;
        self.secrets.clear()?;
        Ok(())
    }
}
