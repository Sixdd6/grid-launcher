mod cloud;
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
        let resp = self.get_response(path, query).await?;
        resp.json::<T>()
            .await
            .map_err(|e| RommError::Decode(e.without_url().to_string()))
    }

    /// Status-checked GET returning the raw response for streaming (or for
    /// `get_json` to decode). 401/403 map to `Unauthorized`; any other
    /// non-2xx maps to `Http` with a body excerpt — the body is consumed
    /// here so callers never see the excerpt logic duplicated.
    pub(crate) async fn get_response(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<reqwest::Response, RommError> {
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
        Ok(resp)
    }

    pub async fn connect(&self) -> Result<UserInfo, RommError> {
        self.get_json("/api/users/me", &[]).await
    }

    /// `path_or_url` is either a server-relative `/path` or an absolute
    /// `http(s)://` URL (host filtering happens before this is called — see
    /// `images::urls`).
    fn target(&self, path_or_url: &str) -> Result<url::Url, RommError> {
        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            url::Url::parse(path_or_url).map_err(|_| RommError::InvalidUrl)
        } else {
            self.endpoint(path_or_url)
        }
    }

    /// Bytes plus the response Content-Type (empty when absent). Accepts a
    /// server-relative `/path` or an absolute same-host URL (host filtering
    /// happens before this is called — see images::urls). Same 401/403 →
    /// Unauthorized mapping as `get_bytes`.
    ///
    /// Task 11 fix: this used to skip the 401/403 -> Unauthorized mapping
    /// that `get_response` applies, so a save/cover download against an
    /// expired token surfaced as a generic `Http{401,..}` instead of the
    /// dedicated auth error every other client method returns. Bytes
    /// endpoints (save content, relative download candidates, covers)
    /// now match that mapping exactly.
    pub async fn get_bytes_with_type(
        &self,
        path_or_url: &str,
    ) -> Result<(Vec<u8>, String), RommError> {
        let resp = self
            .http
            .get(self.target(path_or_url)?)
            .header(reqwest::header::AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(RommError::Unauthorized);
        }
        if !status.is_success() {
            return Err(RommError::Http {
                status: status.as_u16(),
                excerpt: String::new(),
            });
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?
            .to_vec();
        Ok((bytes, content_type))
    }

    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, RommError> {
        self.get_bytes_with_type(path).await.map(|(b, _)| b)
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
    /// Server-relative cover path (large variant), when present.
    #[serde(rename = "path_cover_large")]
    pub cover_large_path: Option<String>,
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
    #[serde(rename = "path_cover_large", default)]
    cover_large_path: Option<String>,
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
            cover_large_path: raw.cover_large_path,
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

    pub async fn rom_detail(&self, rom_id: i64) -> Result<RomDetail, RommError> {
        let raw: RawRomDetail = self.get_json(&format!("/api/roms/{rom_id}"), &[]).await?;
        Ok(raw.into_detail(&self.base))
    }
}

/// Wire shape of `RomFileSchema`'s fields we use. Matches the public
/// `RomFile` exactly, so it's decoded directly with no `Raw`/`From` pair.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct RomFile {
    pub id: i64,
    pub file_name: String,
    #[serde(default)]
    pub file_size_bytes: i64,
    #[serde(default)]
    pub is_top_level: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RomDetail {
    pub id: i64,
    pub name: String,
    pub platform_id: i64,
    pub platform_name: String,
    pub fs_name: String,
    pub description: String,
    pub regions: String,
    pub languages: String,
    pub tags: String,
    pub revision: String,
    pub rating: String,
    pub genres: String,
    pub companies: String,
    pub first_release_date: String,
    pub filesize_bytes: i64,
    pub server_updated_at: String,
    pub files: Vec<RomFile>,
    /// Server-relative cover path (small variant), verbatim from the
    /// server — resolved lazily against the server URL.
    pub cover_small_path: String,
    /// Server-relative cover path (large variant), verbatim from the
    /// server — resolved lazily against the server URL.
    pub cover_large_path: String,
    /// Already resolved + host-filtered absolute screenshot URLs, in
    /// source order (see `images::urls::screenshot_urls_from_payload`).
    pub screenshot_urls: Vec<String>,
}

/// Wire shape of `RomMetadataSchema` fields we use. Every field defaulted so
/// a missing/null `metadatum` (or sparse fields within it) never fails the
/// outer decode.
#[derive(Deserialize, Default)]
struct RawRomMetadata {
    #[serde(default)]
    average_rating: Option<f64>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    companies: Vec<String>,
    #[serde(default)]
    first_release_date: Option<i64>,
}

/// Wire shape of `DetailedRomSchema`'s fields we use. All optionals are
/// defaulted so a sparse payload never fails the decode — `From<RawRomDetail>
/// for RomDetail` below applies the string-fallback conventions.
#[derive(Deserialize)]
struct RawRomDetail {
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    fs_name_no_ext: String,
    platform_id: i64,
    #[serde(default)]
    platform_display_name: String,
    #[serde(default)]
    fs_name: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    regions: Vec<String>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    fs_size_bytes: i64,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    files: Vec<RomFile>,
    #[serde(default)]
    metadatum: Option<RawRomMetadata>,
    #[serde(default)]
    path_cover_small: Option<String>,
    #[serde(default)]
    path_cover_large: Option<String>,
    /// Every field not named above — the screenshot sources
    /// (`merged_screenshots`, `user_screenshots`, metadata blocks…) are read
    /// from here by `screenshot_urls_from_payload`.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl RawRomDetail {
    fn into_detail(self, base_url: &str) -> RomDetail {
        let name = self
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or(self.fs_name_no_ext);
        let metadatum = self.metadatum.unwrap_or_default();
        let resolver = crate::images::urls::server_resolver(base_url);
        let screenshot_urls = crate::images::urls::screenshot_urls_from_payload(
            &serde_json::Value::Object(self.extra),
            &resolver,
        );
        RomDetail {
            id: self.id,
            name,
            platform_id: self.platform_id,
            platform_name: self.platform_display_name,
            fs_name: self.fs_name,
            description: self.summary.unwrap_or_default(),
            regions: self.regions.join(", "),
            languages: self.languages.join(", "),
            tags: self.tags.join(", "),
            revision: self.revision.unwrap_or_default(),
            rating: metadatum
                .average_rating
                .map(|r| format!("{r:.1}"))
                .unwrap_or_default(),
            genres: metadatum.genres.join(", "),
            companies: metadatum.companies.join(", "),
            first_release_date: metadatum
                .first_release_date
                .map(|d| d.to_string())
                .unwrap_or_default(),
            filesize_bytes: self.fs_size_bytes,
            server_updated_at: self.updated_at,
            files: self.files,
            cover_small_path: self.path_cover_small.unwrap_or_default(),
            cover_large_path: self.path_cover_large.unwrap_or_default(),
            screenshot_urls,
        }
    }
}
