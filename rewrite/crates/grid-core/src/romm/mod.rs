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

    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, RommError> {
        let resp = self
            .http
            .get(self.endpoint(path)?)
            .header(reqwest::header::AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(RommError::Http {
                status: status.as_u16(),
                excerpt: String::new(),
            });
        }
        Ok(resp
            .bytes()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?
            .to_vec())
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Platform {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub rom_count: i64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct GameSummary {
    pub id: i64,
    pub name: String,
    pub platform_id: i64,
    /// Server-relative cover path (small variant), when present.
    #[serde(rename = "path_cover_small")]
    pub cover_path: Option<String>,
}

/// Wire shape of a `SimpleRomSchema` entry. `name` is nullable server-side
/// (unidentified/filesystem-only ROMs commonly have no matched name) — kept
/// as `Option<String>` here rather than on the public `GameSummary` so the
/// pinned public interface (`name: String`) never fails to decode a real
/// library page. `fs_name_no_ext` is the fallback, mirroring the existing
/// Python client (`grid_launcher/server/catalog.py:284`:
/// `item.get("name") or item.get("fs_name_no_ext")`).
#[derive(Deserialize)]
struct RawGameSummary {
    id: i64,
    name: Option<String>,
    #[serde(default)]
    fs_name_no_ext: Option<String>,
    platform_id: i64,
    #[serde(rename = "path_cover_small")]
    cover_path: Option<String>,
}

impl From<RawGameSummary> for GameSummary {
    fn from(raw: RawGameSummary) -> Self {
        let name = raw
            .name
            .filter(|n| !n.is_empty())
            .or(raw.fs_name_no_ext)
            .unwrap_or_default();
        GameSummary {
            id: raw.id,
            name,
            platform_id: raw.platform_id,
            cover_path: raw.cover_path,
        }
    }
}

#[derive(Deserialize)]
struct Paged<T> {
    items: Vec<T>,
}

const PAGE_SIZE: usize = 200;

impl RommClient {
    pub async fn platforms(&self) -> Result<Vec<Platform>, RommError> {
        let all: Vec<Platform> = self.get_json("/api/platforms", &[]).await?;
        Ok(all.into_iter().filter(|p| p.rom_count > 0).collect())
    }

    /// Pages through /api/roms with limit 200 until a short page, per
    /// docs/porting/01-romm-api.md row 3. Filters by the plural, repeatable
    /// `platform_ids` query param (not a singular `platform_id`) — confirmed
    /// against openapi.json's `/api/roms` parameter list and
    /// `grid_launcher/server/catalog.py:163`'s
    /// `{"platform_ids": [platform_id]}`. Also disables the char index and
    /// filter-values side payloads (both default to `true` server-side and
    /// are unused here) to match the existing Python client's request shape.
    pub async fn games(&self, platform_id: i64) -> Result<Vec<GameSummary>, RommError> {
        let mut out = Vec::new();
        let mut offset = 0usize;
        loop {
            let page: Paged<RawGameSummary> = self
                .get_json(
                    "/api/roms",
                    &[
                        ("platform_ids", platform_id.to_string()),
                        ("limit", PAGE_SIZE.to_string()),
                        ("offset", offset.to_string()),
                        ("with_char_index", "false".to_string()),
                        ("with_filter_values", "false".to_string()),
                    ],
                )
                .await?;
            let n = page.items.len();
            out.extend(page.items.into_iter().map(GameSummary::from));
            if n < PAGE_SIZE {
                break;
            }
            offset += PAGE_SIZE;
        }
        Ok(out)
    }
}
