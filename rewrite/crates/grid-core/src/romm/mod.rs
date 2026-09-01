mod error;
pub use error::RommError;

use crate::secrets::Credential;
use base64::Engine;
use secrecy::ExposeSecret;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
}

pub struct RommClient {
    http: reqwest::Client,
    /// Base URL with any trailing slash trimmed. Kept as a string (not a
    /// parsed `Url`) so that `endpoint()` can concatenate it with a path
    /// verbatim — `Url::join` would silently drop a base subpath (e.g. a
    /// server hosted at `https://host/romm`) because a leading-slash path
    /// resets a join to the URL's origin root.
    base: String,
    /// Prebuilt Authorization header value. Held as a reqwest HeaderValue
    /// marked sensitive so reqwest's own debug output redacts it.
    auth: reqwest::header::HeaderValue,
}

impl RommClient {
    /// The ONLY place (besides KeyringStore serialization) where a secret is
    /// exposed. Builds the Authorization header value once.
    pub fn new(base_url: &str, cred: Credential) -> Result<Self, RommError> {
        let parsed = url::Url::parse(base_url).map_err(|_| RommError::InvalidUrl)?;
        let base = parsed.as_str().trim_end_matches('/').to_string();
        let raw = match &cred {
            Credential::Token(t) => format!("Bearer {}", t.expose_secret()),
            Credential::Basic { username, password } => {
                let joined = format!("{username}:{}", password.expose_secret());
                format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD.encode(joined)
                )
            }
        };
        let mut auth =
            reqwest::header::HeaderValue::from_str(&raw).map_err(|_| RommError::InvalidUrl)?;
        auth.set_sensitive(true);
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| RommError::Connection(e.to_string()))?;
        Ok(Self { http, base, auth })
    }

    /// Appends `path` to the base URL verbatim, preserving any base subpath
    /// (see the `base` field doc for why this can't use `Url::join`).
    fn endpoint(&self, path: &str) -> Result<url::Url, RommError> {
        if !path.starts_with('/') {
            return Err(RommError::InvalidUrl);
        }
        let combined = format!("{}{path}", self.base);
        url::Url::parse(&combined).map_err(|_| RommError::InvalidUrl)
    }

    pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, RommError> {
        let resp = self
            .http
            .get(self.endpoint(path)?)
            .query(query)
            .header(reqwest::header::AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(RommError::Unauthorized);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RommError::Http {
                status: status.as_u16(),
                excerpt: error::excerpt(&body),
            });
        }
        resp.json::<T>()
            .await
            .map_err(|e| RommError::Decode(e.without_url().to_string()))
    }

    pub async fn connect(&self) -> Result<UserInfo, RommError> {
        self.get_json("/api/users/me", &[]).await
    }
}
