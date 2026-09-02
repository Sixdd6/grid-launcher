//! Forge HTTP client: fetches and scrapes emulator release/asset metadata
//! from GitHub, Gitea, and arbitrary "direct" download pages. Ports the
//! networking half of `InstallDownloadWorker._resolve_source_download` /
//! `_resolve_direct_source_download` (`grid_launcher/background/workers.py:165-320`).
//! The pure normalization/selection logic it builds on lives in
//! [`super::source`]. See `docs/porting/04-emulator-launch.md` §12.
//!
//! Two rules this file exists to enforce:
//! - No RomM credential ever reaches a forge host: this client is a
//!   separate [`reqwest::Client`] that never sends an `Authorization`
//!   header.
//! - The E2E forge redirect ([`effective_url`]) is applied at *request*
//!   time inside [`ForgeClient::get`], never while normalizing or scraping
//!   metadata — the catalog's `download_url_regex` values match absolute
//!   production URLs, so a scraped href has to keep its real URL. Only the
//!   outgoing request itself is diverted, and only when the crate is built
//!   with the `e2e` feature.

use std::collections::HashMap;

use regex::RegexBuilder;
use serde_json::Value;

use super::source::{
    merge_platform_override, normalize_source, select_asset, select_release, str_field,
    SourceError, SourceMap, HOST_PLATFORM,
};
use crate::library::download::{FileTarget, ResponseProvider};
use crate::library::LibraryError;

/// A release/asset a [`ForgeClient`] resolved down to one downloadable file.
/// `size` is the GitHub/Gitea asset's reported byte size, or `0` for a
/// `direct` source (the reference never inspects a direct URL's size ahead
/// of the download) or when the field is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDownload {
    pub provider: String,
    pub owner: String,
    pub repo: String,
    pub release_tag: String,
    pub asset_name: String,
    pub download_url: String,
    pub size: i64,
}

/// HTTP client for forge hosts (GitHub, Gitea, arbitrary direct-download
/// hosts). Deliberately a *separate* [`reqwest::Client`] from any RomM
/// client in this process: it carries no `Authorization` header, ever, and
/// no RomM credential is ever passed to it.
pub struct ForgeClient {
    http: reqwest::Client,
}

const FORGE_USER_AGENT: &str = "grid-launcher";
const FORGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

impl ForgeClient {
    pub fn new() -> Result<Self, SourceError> {
        let http = reqwest::Client::builder()
            .user_agent(FORGE_USER_AGENT)
            .timeout(FORGE_TIMEOUT)
            .build()
            .map_err(|e| SourceError(format!("Failed to build forge HTTP client: {e}")))?;
        Ok(Self { http })
    }

    /// Resolves `raw` (a profile's raw, un-normalized source JSON — or, for
    /// a `supplemental_downloads` entry, that entry's own raw spec) down to
    /// one downloadable file: normalize, merge the matching
    /// `platform_overrides` entry, then dispatch on provider.
    ///
    /// `profile_name` feeds the `direct` platforms-gate message only, as a
    /// fallback when the raw source has no `name` key of its own.
    pub async fn resolve(
        &self,
        raw: &Value,
        profile_name: &str,
    ) -> Result<ResolvedDownload, SourceError> {
        let mut source = normalize_source(raw)?;
        merge_platform_override(&mut source);
        let provider = str_field(&source, "provider");

        // `normalize_source` above already rejected a non-object `raw`, so
        // this object is always present once we get here.
        let raw_obj = raw.as_object().expect("normalize_source accepted `raw`");

        match provider.as_str() {
            "direct" => self.resolve_direct(&source, raw_obj, profile_name).await,
            "github" => {
                let owner = str_field(&source, "owner");
                let repo = str_field(&source, "repo");
                let api_base = format!("https://api.github.com/repos/{owner}/{repo}");
                self.resolve_release(&source, &api_base, true, "github", &owner, &repo)
                    .await
            }
            "gitea" => {
                let owner = str_field(&source, "owner");
                let repo = str_field(&source, "repo");
                let base_url = str_field(&source, "base_url");
                let api_base = format!("{base_url}/api/v1/repos/{owner}/{repo}");
                self.resolve_release(&source, &api_base, false, "gitea", &owner, &repo)
                    .await
            }
            other => Err(SourceError(format!(
                "Unsupported source provider '{other}'. Supported providers: github, gitea, direct."
            ))),
        }
    }

    /// GET `url` (after the E2E rewrite, see [`effective_url`]) with the
    /// forge headers this request needs, returning the streamed response.
    /// Never sends `Authorization`. A non-2xx status becomes a
    /// [`SourceError`] here (`error_for_status`), same as a connect failure.
    pub async fn get(
        &self,
        url: &str,
        github_headers: bool,
    ) -> Result<reqwest::Response, SourceError> {
        let target = effective_url(url);
        let mut request = self.http.get(&target);
        if github_headers {
            request = request
                .header(reqwest::header::ACCEPT, "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28");
        }
        let response = request.send().await.map_err(|e| http_error(url, e))?;
        response.error_for_status().map_err(|e| http_error(url, e))
    }

    /// The `github`/`gitea` branch of [`Self::resolve`]: fetch the release
    /// endpoint `release_tag` selects, then hand the parsed payload to
    /// [`select_release`] and [`select_asset`] (`workers.py:188-242`).
    async fn resolve_release(
        &self,
        source: &SourceMap,
        api_base: &str,
        github_headers: bool,
        provider: &str,
        owner: &str,
        repo: &str,
    ) -> Result<ResolvedDownload, SourceError> {
        let release_tag = str_field(source, "release_tag");
        let endpoint = release_endpoint(api_base, &release_tag);

        let response = self.get(&endpoint, github_headers).await?;
        let text = response
            .text()
            .await
            .map_err(|e| http_error(&endpoint, e))?;
        let payload: Value = serde_json::from_str(&text)
            .map_err(|e| SourceError(format!("Source release API returned invalid JSON: {e}")))?;
        if !(payload.is_object() || payload.is_array()) {
            return Err(SourceError(
                "Source release API returned an unsupported payload shape.".to_string(),
            ));
        }

        let release = select_release(source, &payload)?;
        let asset = select_asset(source, release)?;
        let size = asset.get("size").and_then(Value::as_i64).unwrap_or(0);

        Ok(ResolvedDownload {
            provider: provider.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            release_tag: str_field(release, "tag_name"),
            asset_name: str_field(asset, "name"),
            download_url: str_field(asset, "browser_download_url"),
            size,
        })
    }

    /// The `direct` branch of [`Self::resolve`]
    /// (`_resolve_direct_source_download`, `workers.py:244-293`, plus the
    /// platforms gate at `workers.py:177-184`).
    async fn resolve_direct(
        &self,
        source: &SourceMap,
        raw: &SourceMap,
        profile_name: &str,
    ) -> Result<ResolvedDownload, SourceError> {
        if let Some(message) = direct_platforms_gate_message(raw, profile_name) {
            return Err(SourceError(message));
        }

        let mut download_url = str_field(source, "download_url");
        let page_url = str_field(source, "page_url");
        let download_url_regex = str_field(source, "download_url_regex");
        let mut asset_name = str_field(source, "asset_name");

        if download_url.is_empty() && !page_url.is_empty() {
            let response = self.get(&page_url, false).await?;
            let bytes = response
                .bytes()
                .await
                .map_err(|e| http_error(&page_url, e))?;
            let page_text = String::from_utf8_lossy(&bytes);

            if !download_url_regex.is_empty() {
                download_url = scrape_download_url(&page_text, &download_url_regex, &page_url)?;
            }

            if download_url.is_empty() {
                return Err(SourceError(format!(
                    "Direct source metadata did not resolve a download URL from the configured page. page_url='{page_url}'"
                )));
            }
        }

        if download_url.is_empty() {
            return Err(SourceError(
                "Direct source metadata did not include a download URL.".to_string(),
            ));
        }

        if asset_name.is_empty() {
            asset_name = basename_of_url(&download_url);
        }

        let release_tag = {
            let tag = str_field(source, "release_tag");
            if tag.is_empty() {
                "latest".to_string()
            } else {
                tag
            }
        };

        Ok(ResolvedDownload {
            provider: "direct".to_string(),
            owner: str_field(source, "owner"),
            repo: str_field(source, "repo"),
            release_tag,
            asset_name,
            download_url,
            size: 0,
        })
    }
}

// --- download provider ---------------------------------------------------------

/// The forge-backed [`ResponseProvider`] the install pipeline downloads
/// emulator archives through. `target.url_path` holds an ABSOLUTE URL (the
/// asset's `browser_download_url`) and `target.query` is always empty —
/// unlike the RomM provider, which resolves a path against a base URL.
///
/// Whether a given request needs the GitHub API headers is a property of the
/// *download*, not of the client: one emulator install can pull its primary
/// asset from GitHub and a supplemental from a Gitea host, or the other way
/// round. That per-URL choice is therefore carried here, keyed by the URL
/// [`FileTarget::url_path`] holds. A URL this map does not know is requested
/// without the GitHub headers.
///
/// No `Authorization` header is ever sent: [`ForgeClient`] has no credential
/// to send, and no RomM credential is ever handed to it.
pub struct ForgeProvider<'a> {
    client: &'a ForgeClient,
    github_headers: HashMap<String, bool>,
}

impl<'a> ForgeProvider<'a> {
    /// `github_headers` maps each target URL to whether that request needs
    /// the GitHub API headers (true exactly when that download's provider
    /// normalized to `github`).
    pub fn new(
        client: &'a ForgeClient,
        github_headers: impl IntoIterator<Item = (String, bool)>,
    ) -> Self {
        Self {
            client,
            github_headers: github_headers.into_iter().collect(),
        }
    }
}

impl ResponseProvider for ForgeProvider<'_> {
    async fn get(&self, target: &FileTarget) -> Result<reqwest::Response, LibraryError> {
        let github = self
            .github_headers
            .get(&target.url_path)
            .copied()
            .unwrap_or(false);
        self.client
            .get(&target.url_path, github)
            .await
            .map_err(|e| LibraryError::Extract(e.0))
    }
}

// --- direct-provider helpers -------------------------------------------------

/// The `direct` platforms gate (`workers.py:177-184`): when the RAW
/// source's `platforms` array is present and non-empty, [`HOST_PLATFORM`]
/// must start with at least one of its entries. Returns the (trimmed)
/// error message when the gate fails, `None` when it passes or does not
/// apply.
fn direct_platforms_gate_message(raw: &SourceMap, profile_name: &str) -> Option<String> {
    let Some(Value::Array(platforms)) = raw.get("platforms") else {
        return None;
    };
    if platforms.is_empty() {
        return None;
    }
    let allowed = platforms.iter().any(|entry| {
        let entry = json_value_as_display_string(entry);
        HOST_PLATFORM.starts_with(entry.as_str())
    });
    if allowed {
        return None;
    }

    let name = match raw.get("name") {
        Some(Value::String(s)) => s.clone(),
        _ => {
            let trimmed_profile = profile_name.trim();
            if !trimmed_profile.is_empty() {
                profile_name.to_string()
            } else {
                "This emulator".to_string()
            }
        }
    };
    let hint = match raw.get("manual_install_hint") {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    Some(
        format!("{name} has no auto-install source available for this platform. {hint}")
            .trim()
            .to_string(),
    )
}

/// `str(value)` for a JSON scalar that is not itself a string — used only
/// for the rare non-string `platforms` entry; every real catalog entry uses
/// plain strings.
fn json_value_as_display_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Scrapes `page_text` for a download URL matching `download_url_regex`
/// (`_resolve_direct_source_download`, `workers.py:250-273`): href-order
/// precedence first, then a whole-page regex search fallback. An href that
/// cannot be joined onto the page URL is skipped rather than failing the
/// scrape. Returns `""` (never an error) when nothing matches — the caller
/// turns that into the "did not resolve" error, matching the reference,
/// which only raises once after both passes.
fn scrape_download_url(
    page_text: &str,
    download_url_regex: &str,
    page_url: &str,
) -> Result<String, SourceError> {
    let pattern = RegexBuilder::new(download_url_regex)
        .case_insensitive(true)
        .build()
        .map_err(|e| SourceError(e.to_string()))?;
    let base = url::Url::parse(page_url)
        .map_err(|e| SourceError(format!("Invalid direct source page_url '{page_url}': {e}")))?;

    // href\s*=\s*["']([^"']+)["'], case-insensitive, in page order.
    let href_re = regex::Regex::new(r#"(?i)href\s*=\s*["']([^"']+)["']"#).unwrap();
    for caps in href_re.captures_iter(page_text) {
        let href = caps[1].trim();
        if href.is_empty() {
            continue;
        }
        // A malformed href is skipped, never fatal: Python's `urljoin`
        // effectively never raises, so the reference walks past a broken
        // decoy to the real link further down the page.
        let Ok(resolved) = base.join(href) else {
            continue;
        };
        let resolved_str = resolved.to_string();
        if pattern.is_match(href) || pattern.is_match(&resolved_str) {
            return Ok(resolved_str);
        }
    }

    if let Some(caps) = pattern.captures(page_text) {
        let mut chosen: Option<&str> = None;
        for i in 1..caps.len() {
            if let Some(group) = caps.get(i) {
                let trimmed = group.as_str().trim();
                if !trimmed.is_empty() {
                    chosen = Some(trimmed);
                    break;
                }
            }
        }
        let target = chosen.unwrap_or_else(|| caps.get(0).unwrap().as_str().trim());
        let resolved = base.join(target).map_err(|e| {
            SourceError(format!(
                "Invalid scraped match '{target}' on page '{page_url}': {e}"
            ))
        })?;
        return Ok(resolved.to_string());
    }

    Ok(String::new())
}

/// The last path segment of `url` (`Url::parse` → `path_segments().last()`),
/// or `""` when `url` cannot be parsed as an absolute URL or has no path
/// segments — the `direct` provider's default `asset_name`.
fn basename_of_url(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut s| s.next_back().map(str::to_string))
        })
        .unwrap_or_default()
}

// --- github/gitea-provider helpers -------------------------------------------

/// Everything outside `ALPHA DIGIT - . _ ~` — the character set
/// `urllib.parse.quote(tag, safe="")` percent-encodes.
const TAG_ENCODE_SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// The release endpoint `release_tag` selects under `api_base`
/// (`workers.py:204-221`): an explicit non-`latest` tag goes to
/// `/releases/tags/{tag}` (percent-encoded), the literal `latest`
/// (case-insensitive) to `/releases/latest`, and an unset tag to
/// `/releases`.
fn release_endpoint(api_base: &str, release_tag: &str) -> String {
    if !release_tag.is_empty() && !release_tag.eq_ignore_ascii_case("latest") {
        let encoded = percent_encoding::utf8_percent_encode(release_tag, TAG_ENCODE_SET);
        format!("{api_base}/releases/tags/{encoded}")
    } else if release_tag.eq_ignore_ascii_case("latest") {
        format!("{api_base}/releases/latest")
    } else {
        format!("{api_base}/releases")
    }
}

// --- HTTP error formatting ----------------------------------------------------

/// Formats a forge HTTP failure (connect, non-2xx, or body read) so the
/// message names the host the request was going to but never repeats any
/// header content — the forge client sends no secrets, but nothing here
/// should get in the habit of echoing headers back into user-facing text.
fn http_error(url: &str, err: reqwest::Error) -> SourceError {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "unknown host".to_string());
    SourceError(format!(
        "Forge request to '{host}' failed: {}",
        err.without_url()
    ))
}

// --- E2E request-time redirect ------------------------------------------------

/// The URL [`ForgeClient::get`] actually requests. Identity in a release
/// build (the `e2e` feature is off by default and never turned on for a
/// shipped binary — see `scripts/check_secret_hygiene.sh`); under the `e2e`
/// feature, `GRID_LAUNCHER_E2E_FORGE_BASE` (when set to a non-blank value)
/// redirects every forge request to `<base>/<host>[:<port>]/<path>?<query>`
/// — an explicit, non-default port is kept — letting the E2E harness serve
/// fixture files instead of hitting real forges.
///
/// Deliberately request-time only: metadata normalization and page
/// scraping keep working with the *original* URLs, because the catalog's
/// `download_url_regex` patterns match those real, absolute URLs.
#[cfg(feature = "e2e")]
fn effective_url(url: &str) -> String {
    match std::env::var("GRID_LAUNCHER_E2E_FORGE_BASE") {
        Ok(base) if !base.trim().is_empty() => {
            let Ok(parsed) = url::Url::parse(url) else {
                return url.to_string();
            };
            // The full authority, not just the host: a fixture forge served
            // on a non-default port would otherwise collapse onto the same
            // segment as the same host on another port.
            let mut authority = parsed.host_str().unwrap_or("").to_string();
            if let Some(port) = parsed.port() {
                authority.push(':');
                authority.push_str(&port.to_string());
            }
            let mut suffix = parsed.path().to_string();
            if let Some(query) = parsed.query() {
                suffix.push('?');
                suffix.push_str(query);
            }
            format!("{}/{authority}{suffix}", base.trim().trim_end_matches('/'))
        }
        _ => url.to_string(),
    }
}

#[cfg(not(feature = "e2e"))]
#[inline]
fn effective_url(url: &str) -> String {
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn map_of(value: Value) -> SourceMap {
        value.as_object().unwrap().clone()
    }

    // --- release_endpoint: tag → endpoint choice --------------------------

    #[test]
    fn release_endpoint_unset_tag_goes_to_plain_releases() {
        assert_eq!(
            release_endpoint("https://api.github.com/repos/o/r", ""),
            "https://api.github.com/repos/o/r/releases"
        );
    }

    #[test]
    fn release_endpoint_latest_tag_is_case_insensitive() {
        assert_eq!(
            release_endpoint("https://api.github.com/repos/o/r", "Latest"),
            "https://api.github.com/repos/o/r/releases/latest"
        );
    }

    #[test]
    fn release_endpoint_explicit_tag_goes_to_tags_path() {
        assert_eq!(
            release_endpoint("https://api.github.com/repos/o/r", "v1.2.3"),
            "https://api.github.com/repos/o/r/releases/tags/v1.2.3"
        );
    }

    #[test]
    fn release_endpoint_percent_encodes_a_tag_with_a_slash() {
        assert_eq!(
            release_endpoint("https://api.github.com/repos/o/r", "channel/v1"),
            "https://api.github.com/repos/o/r/releases/tags/channel%2Fv1"
        );
    }

    // --- ForgeClient::get: headers ------------------------------------------

    #[tokio::test]
    async fn github_headers_present_and_authorization_always_absent() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/o/r/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"tag_name": "v1"})))
            .mount(&mock_server)
            .await;

        let client = ForgeClient::new().unwrap();
        let url = format!("{}/repos/o/r/releases/latest", mock_server.uri());
        let response = client.get(&url, true).await.unwrap();
        assert!(response.status().is_success());

        let received = mock_server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        let headers = &received[0].headers;
        assert_eq!(
            headers.get("accept").unwrap(),
            "application/vnd.github+json"
        );
        assert_eq!(headers.get("x-github-api-version").unwrap(), "2022-11-28");
        assert_eq!(headers.get("user-agent").unwrap(), "grid-launcher");
        assert!(headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn non_github_request_carries_only_user_agent() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&mock_server)
            .await;

        let client = ForgeClient::new().unwrap();
        let response = client.get(&mock_server.uri(), false).await.unwrap();
        assert!(response.status().is_success());

        let received = mock_server.received_requests().await.unwrap();
        let headers = &received[0].headers;
        // reqwest's own default `Accept: */*` is fine — what must never
        // appear is the GitHub-specific header set we only add when asked.
        assert_ne!(
            headers.get("accept").map(|v| v.to_str().unwrap()),
            Some("application/vnd.github+json")
        );
        assert!(headers.get("x-github-api-version").is_none());
        assert!(headers.get("authorization").is_none());
        assert_eq!(headers.get("user-agent").unwrap(), "grid-launcher");
    }

    #[tokio::test]
    async fn non_2xx_status_becomes_a_source_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = ForgeClient::new().unwrap();
        let err = client.get(&mock_server.uri(), false).await.unwrap_err();
        // Must name the host, never any header content.
        let host = url::Url::parse(&mock_server.uri())
            .unwrap()
            .host_str()
            .unwrap()
            .to_string();
        assert!(err.0.contains(&host), "error was: {}", err.0);
        assert!(!err.0.to_lowercase().contains("user-agent"));
    }

    // --- resolve: gitea end to end -------------------------------------------

    #[tokio::test]
    async fn gitea_resolve_hits_base_url_endpoint_with_github_headers_absent() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/repos/acme/widget/releases/tags/v2.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tag_name": "v2.0",
                "assets": [
                    {"name": "widget-linux.zip", "browser_download_url": "https://cdn.example.com/widget-linux.zip", "size": 42}
                ]
            })))
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "gitea", "owner": "acme", "repo": "widget",
            "base_url": mock_server.uri(), "release_tag": "v2.0"
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Widget").await.unwrap();

        assert_eq!(
            resolved,
            ResolvedDownload {
                provider: "gitea".to_string(),
                owner: "acme".to_string(),
                repo: "widget".to_string(),
                release_tag: "v2.0".to_string(),
                asset_name: "widget-linux.zip".to_string(),
                download_url: "https://cdn.example.com/widget-linux.zip".to_string(),
                size: 42,
            }
        );

        let received = mock_server.received_requests().await.unwrap();
        let headers = &received[0].headers;
        // reqwest's own default `Accept: */*` is fine — what must never
        // appear is the GitHub-specific header set.
        assert_ne!(
            headers.get("accept").map(|v| v.to_str().unwrap()),
            Some("application/vnd.github+json")
        );
        assert!(headers.get("x-github-api-version").is_none());
        assert!(headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn non_json_top_level_payload_errors_verbatim() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("5"))
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "gitea", "owner": "o", "repo": "r",
            "base_url": mock_server.uri()
        });
        let client = ForgeClient::new().unwrap();
        let err = client.resolve(&raw, "Thing").await.unwrap_err();
        assert_eq!(
            err.0,
            "Source release API returned an unsupported payload shape."
        );
    }

    // --- resolve: unsupported provider ---------------------------------------

    #[tokio::test]
    async fn unsupported_provider_errors_verbatim() {
        let raw = json!({"provider": "carrier-pigeon", "owner": "o", "repo": "r"});
        let client = ForgeClient::new().unwrap();
        let err = client.resolve(&raw, "Thing").await.unwrap_err();
        assert_eq!(
            err.0,
            "Unsupported source provider 'carrier-pigeon'. Supported providers: github, gitea, direct."
        );
    }

    // --- resolve: direct platforms gate --------------------------------------

    #[tokio::test]
    async fn direct_platforms_gate_uses_raw_name_and_appends_trimmed_hint() {
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": "https://example.com/downloads",
            "platforms": ["win32"],
            "name": "Frobnicator", "manual_install_hint": "Grab it from the website."
        });
        let client = ForgeClient::new().unwrap();
        let err = client.resolve(&raw, "profile-name").await.unwrap_err();
        assert_eq!(
            err.0,
            "Frobnicator has no auto-install source available for this platform. Grab it from the website."
        );
    }

    #[tokio::test]
    async fn direct_platforms_gate_falls_back_to_profile_name_then_this_emulator() {
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": "https://example.com/downloads",
            "platforms": ["win32"]
        });
        let client = ForgeClient::new().unwrap();

        let err = client.resolve(&raw, "Widget Profile").await.unwrap_err();
        assert_eq!(
            err.0,
            "Widget Profile has no auto-install source available for this platform."
        );

        let err = client.resolve(&raw, "").await.unwrap_err();
        assert_eq!(
            err.0,
            "This emulator has no auto-install source available for this platform."
        );
    }

    #[tokio::test]
    async fn direct_platforms_gate_passes_when_host_platform_listed() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"<a href="https://cdn.example.com/build-linux.zip">build</a>"#,
                ),
            )
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": mock_server.uri(),
            "download_url_regex": "linux.*\\.zip$",
            "platforms": [HOST_PLATFORM, "win32"]
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            "https://cdn.example.com/build-linux.zip"
        );
    }

    // --- resolve: direct page scrape -----------------------------------------

    #[tokio::test]
    async fn direct_scrape_href_matches_the_raw_href_takes_precedence() {
        let mock_server = MockServer::start().await;
        let page = r#"
            <a href="https://cdn.example.com/build-windows.zip">windows</a>
            <a href="https://cdn.example.com/build-linux.zip">linux</a>
        "#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(page))
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": mock_server.uri(),
            "download_url_regex": "linux.*\\.zip$"
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            "https://cdn.example.com/build-linux.zip"
        );
        assert_eq!(resolved.asset_name, "build-linux.zip");
    }

    #[tokio::test]
    async fn direct_scrape_skips_a_malformed_href_and_keeps_walking() {
        let mock_server = MockServer::start().await;
        // `https://[bad` has an unterminated IPv6 host, so `Url::join`
        // rejects it. Python's urljoin never raises, so the reference walks
        // past a decoy like this to the real link — and so must this.
        let page = r#"
            <a href="https://[bad">broken</a>
            <a href="https://cdn.example.com/build-linux.zip">linux</a>
        "#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(page))
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": mock_server.uri(),
            "download_url_regex": "linux.*\\.zip$"
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            "https://cdn.example.com/build-linux.zip"
        );
    }

    #[tokio::test]
    async fn direct_scrape_relative_href_matches_only_after_urljoin() {
        let mock_server = MockServer::start().await;
        let page = r#"<a href="build-linux.zip">linux</a>"#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(page))
            .mount(&mock_server)
            .await;

        // The pattern requires the mock server's own host, which is never
        // present in the raw (relative) href — only the urljoin'd form
        // matches.
        let page_url = mock_server.uri();
        let host = url::Url::parse(&page_url)
            .unwrap()
            .host_str()
            .unwrap()
            .to_string();
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": page_url,
            "download_url_regex": format!("{}.*linux.*\\.zip$", regex::escape(&host))
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            format!("{}/build-linux.zip", mock_server.uri())
        );
    }

    #[tokio::test]
    async fn direct_scrape_falls_back_to_whole_page_capture_group() {
        let mock_server = MockServer::start().await;
        // No `href=` anywhere; the download filename only appears in plain
        // text, inside a capture group.
        let page = "Latest build: download-linux-build.zip is ready.";
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(page))
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": mock_server.uri(),
            "download_url_regex": r"download-(\S+\.zip)"
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            format!("{}/linux-build.zip", mock_server.uri())
        );
    }

    #[tokio::test]
    async fn direct_scrape_falls_back_to_whole_match_when_pattern_has_no_groups() {
        let mock_server = MockServer::start().await;
        let page = "Latest build: linux-build.zip is ready.";
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(page))
            .mount(&mock_server)
            .await;

        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": mock_server.uri(),
            "download_url_regex": r"linux-build\.zip"
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            format!("{}/linux-build.zip", mock_server.uri())
        );
    }

    #[tokio::test]
    async fn direct_scrape_failure_message_is_verbatim() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("nothing useful here"))
            .mount(&mock_server)
            .await;

        let page_url = mock_server.uri();
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "page_url": page_url,
            "download_url_regex": r"linux-build\.zip"
        });
        let client = ForgeClient::new().unwrap();
        let err = client.resolve(&raw, "Thing").await.unwrap_err();
        assert_eq!(
            err.0,
            format!(
                "Direct source metadata did not resolve a download URL from the configured page. page_url='{page_url}'"
            )
        );
    }

    #[tokio::test]
    async fn direct_missing_download_url_and_page_url_errors_verbatim() {
        // normalize_source itself rejects this shape before any resolve
        // logic runs, but resolve() must surface that message unchanged.
        let raw = json!({"provider": "direct", "owner": "o", "repo": "r"});
        let client = ForgeClient::new().unwrap();
        let err = client.resolve(&raw, "Thing").await.unwrap_err();
        assert_eq!(
            err.0,
            "Direct source metadata must include either 'download_url' or 'page_url'."
        );
    }

    #[tokio::test]
    async fn direct_download_url_present_skips_the_page_entirely() {
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r",
            "download_url": "https://cdn.example.com/dl/widget-1.0.zip"
        });
        let client = ForgeClient::new().unwrap();
        let resolved = client.resolve(&raw, "Thing").await.unwrap();
        assert_eq!(
            resolved.download_url,
            "https://cdn.example.com/dl/widget-1.0.zip"
        );
        assert_eq!(resolved.asset_name, "widget-1.0.zip");
        assert_eq!(resolved.release_tag, "latest");
    }

    // --- ForgeProvider: per-download header choice ---------------------------

    fn target(url: &str) -> FileTarget {
        FileTarget {
            url_path: url.to_string(),
            query: Vec::new(),
            dest: std::path::PathBuf::from("/dev/null"),
            expected_size: 0,
        }
    }

    #[tokio::test]
    async fn provider_sends_github_headers_only_for_the_urls_flagged_github() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("bytes"))
            .mount(&mock_server)
            .await;

        let github_url = format!("{}/gh/asset.zip", mock_server.uri());
        let gitea_url = format!("{}/gitea/asset.zip", mock_server.uri());
        let client = ForgeClient::new().unwrap();
        let provider = ForgeProvider::new(
            &client,
            [(github_url.clone(), true), (gitea_url.clone(), false)],
        );

        provider.get(&target(&github_url)).await.unwrap();
        provider.get(&target(&gitea_url)).await.unwrap();
        // A URL the map does not know defaults to no GitHub headers.
        let unknown_url = format!("{}/other/asset.zip", mock_server.uri());
        provider.get(&target(&unknown_url)).await.unwrap();

        let received = mock_server.received_requests().await.unwrap();
        assert_eq!(received.len(), 3);
        for request in &received {
            assert!(
                request.headers.get("authorization").is_none(),
                "the forge provider must never send Authorization"
            );
        }
        assert_eq!(
            received[0].headers.get("accept").unwrap(),
            "application/vnd.github+json"
        );
        assert_eq!(
            received[0].headers.get("x-github-api-version").unwrap(),
            "2022-11-28"
        );
        for request in &received[1..] {
            assert_ne!(
                request.headers.get("accept").map(|v| v.to_str().unwrap()),
                Some("application/vnd.github+json")
            );
            assert!(request.headers.get("x-github-api-version").is_none());
        }
    }

    #[tokio::test]
    async fn provider_maps_a_forge_failure_to_a_library_error_with_the_source_text() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let url = format!("{}/gone.zip", mock_server.uri());
        let client = ForgeClient::new().unwrap();
        let provider = ForgeProvider::new(&client, [(url.clone(), false)]);
        let err = provider.get(&target(&url)).await.unwrap_err();
        let expected = client.get(&url, false).await.unwrap_err();
        assert!(
            matches!(&err, LibraryError::Extract(msg) if *msg == expected.0),
            "unexpected error: {err}"
        );
    }

    // --- select_release/select_asset plumbing sanity -------------------------

    #[test]
    fn map_of_helper_roundtrips() {
        let m = map_of(json!({"a": 1}));
        assert_eq!(m.get("a"), Some(&json!(1)));
    }
}

#[cfg(all(test, feature = "e2e"))]
mod e2e_tests {
    use super::*;
    use crate::test_env::EnvGuard;

    /// `GRID_LAUNCHER_E2E_FORGE_BASE` is process-global like every other env
    /// var these tests could race on, so it goes through the crate-wide
    /// `crate::test_env` lock rather than a module-local one — see there.
    const VAR: &str = "GRID_LAUNCHER_E2E_FORGE_BASE";

    #[test]
    fn maps_host_path_and_query_under_the_configured_base() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[(VAR, Some("http://127.0.0.1:9999"))]);

        let effective = effective_url("https://api.github.com/repos/o/r/releases?x=1&y=2");
        assert_eq!(
            effective,
            "http://127.0.0.1:9999/api.github.com/repos/o/r/releases?x=1&y=2"
        );
    }

    #[test]
    fn keeps_the_port_in_the_mapped_path_segment() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[(VAR, Some("http://127.0.0.1:9999"))]);

        // A fixture forge on a non-default port must stay addressable: the
        // segment is the full authority, host AND port.
        let effective = effective_url("http://gitea.local:3000/x");
        assert_eq!(effective, "http://127.0.0.1:9999/gitea.local:3000/x");
    }

    #[test]
    fn strips_a_trailing_slash_on_the_configured_base() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[(VAR, Some("http://127.0.0.1:9999/"))]);

        let effective = effective_url("https://api.github.com/repos/o/r/releases");
        assert_eq!(
            effective,
            "http://127.0.0.1:9999/api.github.com/repos/o/r/releases"
        );
    }

    #[test]
    fn empty_env_var_is_a_passthrough() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[(VAR, Some("   "))]);

        let url = "https://api.github.com/repos/o/r/releases";
        assert_eq!(effective_url(url), url);
    }

    #[test]
    fn unset_env_var_is_a_passthrough() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[(VAR, None)]);

        let url = "https://api.github.com/repos/o/r/releases";
        assert_eq!(effective_url(url), url);
    }
}
