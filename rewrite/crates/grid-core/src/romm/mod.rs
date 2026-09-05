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

/// One firmware file the server offers for a platform. The wire schema
/// carries more fields (`file_size_bytes`, `crc_hash`, ...) this crate does
/// not read; `#[serde(default)]` on `file_name` means an item missing it
/// still decodes rather than being dropped by [`RommClient::firmware`]'s
/// lenient per-item filter — only a missing/non-integer `id` drops an item.
#[derive(Debug, Clone, Deserialize)]
pub struct FirmwareRecord {
    pub id: i64,
    #[serde(default)]
    pub file_name: String,
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
    /// Already resolved + host-filtered absolute screenshot URLs, in source
    /// order — the same `screenshot_urls_from_payload` output `RomDetail`
    /// carries, read from the LIST payload so the Server grid's background
    /// art has screenshots without a per-card detail fetch.
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
    /// Already resolved + host-filtered absolute fanart URLs
    /// (`fanart_urls_from_payload`). Usually empty: most servers have no
    /// fanart, which is why the background falls back to screenshots.
    #[serde(default)]
    pub fanart_urls: Vec<String>,
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
    /// Every field not named above — the screenshot and fanart sources
    /// (`merged_screenshots`, `ss_metadata`, `gamelist_metadata`, …) are read
    /// from here, exactly as `RawRomDetail` does it.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl RawGameSummary {
    /// `base_url` is needed because a screenshot/fanart path is server
    /// relative and must be resolved and host-filtered before it leaves this
    /// crate — which is why this is a method rather than the `From` impl it
    /// replaces.
    fn into_summary(self, base_url: &str) -> GameSummary {
        let name = self
            .name
            .filter(|n| !n.is_empty())
            .or(self.fs_name_no_ext)
            .unwrap_or_default();
        let resolver = crate::images::urls::server_resolver(base_url);
        let extra = serde_json::Value::Object(self.extra);
        GameSummary {
            id: self.id,
            name,
            platform_id: self.platform_id,
            cover_path: self.cover_path,
            cover_large_path: self.cover_large_path,
            screenshot_urls: crate::images::urls::screenshot_urls_from_payload(&extra, &resolver),
            fanart_urls: crate::images::urls::fanart_urls_from_payload(&extra, &resolver),
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
            out.extend(
                page.items
                    .into_iter()
                    .map(|raw| raw.into_summary(&self.base)),
            );
            if n < PAGE_SIZE {
                break;
            }
            offset += PAGE_SIZE;
        }
        Ok(out)
    }

    /// `GET /api/firmware?platform_id=<id>` (`fetch_platform_firmware`,
    /// firmware_install.py:27-30). The server's `FirmwareSchema` carries
    /// more fields than this crate reads; each array item is decoded
    /// leniently so one malformed/unexpected entry doesn't fail the whole
    /// list — only items with an integer `id` are kept. A non-array body
    /// (Python's `isinstance(firmware, list)` guard) yields an empty vec
    /// rather than an error.
    pub async fn firmware(&self, platform_id: i64) -> Result<Vec<FirmwareRecord>, RommError> {
        let value: serde_json::Value = self
            .get_json("/api/firmware", &[("platform_id", platform_id.to_string())])
            .await?;
        let Some(items) = value.as_array() else {
            return Ok(Vec::new());
        };
        Ok(items
            .iter()
            .filter_map(|item| serde_json::from_value::<FirmwareRecord>(item.clone()).ok())
            .collect())
    }

    /// `GET /api/firmware/{id}/content/{file_name}` (`download_firmware_bytes`,
    /// firmware_install.py:33-34). `file_name` is percent-encoded the same
    /// way a ROM content file name is (see `library::encode_file_segment`)
    /// so a name containing a space or reserved character can't change the
    /// shape of the request.
    pub async fn firmware_bytes(&self, id: i64, file_name: &str) -> Result<Vec<u8>, RommError> {
        self.get_bytes(&format!(
            "/api/firmware/{id}/content/{}",
            crate::library::encode_file_segment(file_name)
        ))
        .await
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
    /// The file's own last-modified timestamp as the server states it
    /// (ISO 8601, e.g. `2026-02-03T11:22:33`). `""` when the server does
    /// not send one. D-UI-10 falls back to this when a file name carries
    /// no version tag, so it is kept verbatim and formatted in the UI.
    #[serde(default, deserialize_with = "null_to_empty")]
    pub last_modified: String,
    /// RomM's file category (e.g. "update", "dlc"); blank for an ordinary
    /// game file. The server sends `null` for no category.
    #[serde(default, deserialize_with = "null_to_empty")]
    pub category: String,
}

/// Deserializes a nullable string field as an empty string when absent or
/// `null`, rather than failing or requiring `Option<String>` at every call
/// site.
fn null_to_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// RomM stores IGDB's `first_release_date` in **milliseconds** (the value
/// the user's server sends renders as year 56322 when read as seconds),
/// while older payloads and the IGDB source use seconds. Anything above
/// 100_000_000_000 in magnitude (year 5138 in seconds, and the same
/// distance before 1970 for the pre-1970 titles IGDB lists) can only be
/// milliseconds, so it is divided down; the frontend then always receives
/// seconds.
fn release_date_seconds(raw: i64) -> i64 {
    if raw.abs() > 100_000_000_000 {
        raw / 1000
    } else {
        raw
    }
}

/// One entry of the details Overview "Related" row. IGDB's own cover URLs
/// live on `images.igdb.com`, which `filter_to_server_host` (doc 07) drops,
/// so only the title and which list it came from are carried.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RelatedGame {
    pub name: String,
    /// `"similar"`, `"remake"`, `"remaster"`, `"dlc"` or `"expansion"`.
    pub kind: String,
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
    /// `metadatum.franchises`, comma-joined — same convention as `genres`.
    pub franchises: String,
    /// `metadatum.game_modes`, comma-joined.
    pub game_modes: String,
    /// `metadatum.player_count`, verbatim (the server sends a free-form
    /// string such as `"1"` or `"1-4"`).
    pub player_count: String,
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
    /// Already resolved + host-filtered absolute fanart URLs
    /// (`images::urls::fanart_urls_from_payload`). The shell's background art
    /// prefers these over screenshots (user ruling 2026-09-05).
    pub fanart_urls: Vec<String>,
    /// `youtube_video_id`, `""` when the server has none. The frontend
    /// embeds it; it is never resolved to a URL here.
    pub youtube_video_id: String,
    /// `path_video`, verbatim from the server (server-relative) — resolved
    /// lazily against the server URL by `ensure_video`, exactly as the
    /// cover paths are by `ensure_image`. Never a token-bearing URL.
    pub video_path: String,
    /// RomM's `is_identified`: the game was matched against a metadata
    /// provider. `false` when the server omits the flag.
    pub is_identified: bool,
    /// The Overview "Related" row, in source-list order.
    pub related: Vec<RelatedGame>,
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
    #[serde(default)]
    franchises: Vec<String>,
    #[serde(default)]
    game_modes: Vec<String>,
    #[serde(default)]
    player_count: String,
}

/// One `IGDBRelatedGame`. Only `name` is used; the rest of the wire shape
/// (`id`, `slug`, `type`, `cover_url`) is ignored by omission.
#[derive(Deserialize, Default)]
struct RawRelatedGame {
    #[serde(default)]
    name: String,
}

/// Wire shape of the `RomIGDBMetadata` lists §7's Related row reads. Every
/// field defaulted: a null `igdb_metadata`, or one with only some lists,
/// must never fail the outer decode.
#[derive(Deserialize, Default)]
struct RawIgdbMetadata {
    #[serde(default)]
    similar_games: Vec<RawRelatedGame>,
    #[serde(default)]
    remakes: Vec<RawRelatedGame>,
    #[serde(default)]
    remasters: Vec<RawRelatedGame>,
    #[serde(default)]
    dlcs: Vec<RawRelatedGame>,
    #[serde(default)]
    expansions: Vec<RawRelatedGame>,
}

impl RawIgdbMetadata {
    /// Flattens the five lists into one row, in a FIXED order, dropping
    /// blank names and any title already present (IGDB repeats a title
    /// across lists often enough that the row would otherwise stutter).
    fn into_related(self) -> Vec<RelatedGame> {
        let lists = [
            ("similar", self.similar_games),
            ("remake", self.remakes),
            ("remaster", self.remasters),
            ("dlc", self.dlcs),
            ("expansion", self.expansions),
        ];
        let mut out: Vec<RelatedGame> = Vec::new();
        for (kind, list) in lists {
            for entry in list {
                let name = entry.name.trim().to_string();
                if name.is_empty() || out.iter().any(|r| r.name == name) {
                    continue;
                }
                out.push(RelatedGame {
                    name,
                    kind: kind.to_string(),
                });
            }
        }
        out
    }
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
    #[serde(default)]
    igdb_metadata: Option<RawIgdbMetadata>,
    #[serde(default)]
    youtube_video_id: Option<String>,
    #[serde(default)]
    path_video: Option<String>,
    #[serde(default)]
    is_identified: bool,
    /// Every field not named above — the screenshot and fanart sources
    /// (`merged_screenshots`, `user_screenshots`, metadata blocks…) are read
    /// from here by `screenshot_urls_from_payload` and
    /// `fanart_urls_from_payload`.
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
        let igdb = self.igdb_metadata.unwrap_or_default();
        let resolver = crate::images::urls::server_resolver(base_url);
        let extra = serde_json::Value::Object(self.extra);
        let screenshot_urls = crate::images::urls::screenshot_urls_from_payload(&extra, &resolver);
        let fanart_urls = crate::images::urls::fanart_urls_from_payload(&extra, &resolver);
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
                .map(|d| release_date_seconds(d).to_string())
                .unwrap_or_default(),
            franchises: metadatum.franchises.join(", "),
            game_modes: metadatum.game_modes.join(", "),
            player_count: metadatum.player_count,
            filesize_bytes: self.fs_size_bytes,
            server_updated_at: self.updated_at,
            files: self.files,
            cover_small_path: self.path_cover_small.unwrap_or_default(),
            cover_large_path: self.path_cover_large.unwrap_or_default(),
            screenshot_urls,
            fanart_urls,
            youtube_video_id: self.youtube_video_id.unwrap_or_default(),
            video_path: self.path_video.unwrap_or_default(),
            is_identified: self.is_identified,
            related: igdb.into_related(),
        }
    }
}

#[cfg(test)]
mod release_date_tests {
    use super::release_date_seconds;

    #[test]
    fn milliseconds_are_divided_down_to_seconds() {
        assert_eq!(release_date_seconds(653_529_600_000), 653_529_600);
    }

    #[test]
    fn seconds_pass_through_unchanged() {
        assert_eq!(release_date_seconds(631_152_000), 631_152_000);
    }

    #[test]
    fn zero_passes_through_unchanged() {
        assert_eq!(release_date_seconds(0), 0);
    }

    #[test]
    fn pre_1970_milliseconds_are_divided_down_too() {
        // 1958-06-01T00:00:00Z
        assert_eq!(release_date_seconds(-365_558_400_000), -365_558_400);
    }
}

#[cfg(test)]
mod summary_tests {
    use super::{GameSummary, RawGameSummary};

    fn parse(value: serde_json::Value) -> GameSummary {
        let raw: RawGameSummary = serde_json::from_value(value).expect("summary decodes");
        raw.into_summary("https://romm.example")
    }

    #[test]
    fn a_summary_carries_its_screenshots_and_fanart_resolved_and_filtered() {
        let summary = parse(serde_json::json!({
            "id": 101,
            "name": "Super Mario World",
            "platform_id": 1,
            "path_cover_small": "/assets/small.png",
            "path_cover_large": "/assets/large.png",
            "merged_screenshots": [
                "/assets/shots/1.png",
                "https://img.elsewhere/box-front.jpg"
            ],
            "ss_metadata": { "fanart_path": "/assets/art/fanart.jpg" }
        }));
        assert_eq!(summary.id, 101);
        assert_eq!(
            summary.screenshot_urls,
            vec!["https://romm.example/assets/shots/1.png".to_string()]
        );
        assert_eq!(
            summary.fanart_urls,
            vec!["https://romm.example/assets/art/fanart.jpg".to_string()]
        );
    }

    /// The pinned public contract from before this change: a null `name`
    /// still falls back to `fs_name_no_ext`, and a payload with none of the
    /// new fields still decodes.
    #[test]
    fn an_older_payload_still_decodes_with_empty_lists() {
        let summary = parse(serde_json::json!({
            "id": 102,
            "name": null,
            "fs_name_no_ext": "Chrono Trigger (USA)",
            "platform_id": 1,
            "path_cover_small": "/assets/small.png"
        }));
        assert_eq!(summary.name, "Chrono Trigger (USA)");
        assert!(summary.screenshot_urls.is_empty());
        assert!(summary.fanart_urls.is_empty());
        assert_eq!(summary.cover_large_path, None);
    }
}

#[cfg(test)]
mod detail_fanart_tests {
    use super::RawRomDetail;

    #[test]
    fn a_detail_carries_its_fanart() {
        let raw: RawRomDetail = serde_json::from_value(serde_json::json!({
            "id": 101,
            "fs_name_no_ext": "Super Mario World",
            "platform_id": 1,
            "gamelist_metadata": { "fanart_path": "/assets/art/fanart.jpg" }
        }))
        .expect("detail decodes");
        let detail = raw.into_detail("https://romm.example");
        assert_eq!(
            detail.fanart_urls,
            vec!["https://romm.example/assets/art/fanart.jpg".to_string()]
        );
    }
}
