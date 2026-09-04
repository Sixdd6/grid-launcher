//! Check-only launcher self-update (spec §5, doc 10 OQ14 ruling D-10-h):
//! one GitHub `releases/latest` request per process, a banner when the tag
//! is newer than the running version, nothing downloaded or installed.
//!
//! Goes through grid-core's `ForgeClient`: no RomM credential can reach the
//! forge, and the E2E forge redirect in `launch/forge.rs` applies to the
//! request. Every failure is silent at debug level, naming the host only.

use grid_core::launch::forge::ForgeClient;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub const APP_UPDATE_EVENT: &str = "app-update-available";
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/Sixdd6/grid-launcher/releases/latest";
/// The only part of [`LATEST_RELEASE_URL`] that may reach a log line.
const LATEST_RELEASE_HOST: &str = "api.github.com";
/// The most release JSON that will be read. A `releases/latest` payload is a
/// few KiB; anything past this is not the endpoint we asked for. Enforced
/// while the body streams in, so a peer that answers with an endless
/// response cannot make this process buffer it, let alone parse it.
const MAX_RELEASE_BODY: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppUpdateNotice {
    pub tag: String,
    pub url: String,
}

/// The notice the startup check produced, held so a webview that mounts
/// after the emit can still pull it (`commands::updates::app_update_notice`).
/// Tauri buffers nothing for a window with no listener, and the check never
/// repeats, so without this the banner is simply lost when the forge answers
/// faster than the frontend boots.
#[derive(Default)]
pub struct AppUpdateState(Mutex<Option<AppUpdateNotice>>);

impl AppUpdateState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn set(&self, notice: AppUpdateNotice) {
        *self.0.lock().expect("app update notice mutex") = Some(notice);
    }

    pub fn get(&self) -> Option<AppUpdateNotice> {
        self.0.lock().expect("app update notice mutex").clone()
    }
}

#[derive(Deserialize)]
struct LatestRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
}

/// Whether `tag` (with or without a leading `v`) is a newer semver than
/// `current`. Unparseable input on either side is "not newer".
pub fn is_newer(current: &str, tag: &str) -> bool {
    let tag = tag.trim();
    let tag = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag);
    match (Version::parse(current.trim()), Version::parse(tag)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => false,
    }
}

/// A source build: the pre-release carries a `dev` identifier (`0.9.0-dev`).
pub fn is_dev_build(current: &str) -> bool {
    Version::parse(current.trim())
        .map(|v| v.pre.as_str().split('.').any(|id| id == "dev"))
        .unwrap_or(false)
}

/// The gate: dev builds never check, unless the `e2e` build is told to.
pub fn should_check(current: &str, e2e_forced: bool) -> bool {
    !is_dev_build(current) || e2e_forced
}

fn e2e_forced() -> bool {
    cfg!(feature = "e2e") && std::env::var("GRID_LAUNCHER_E2E_UPDATE_CHECK").is_ok_and(|v| v == "1")
}

/// Runs the check once, on Tauri's async runtime. Call from `setup`.
/// Stores the notice in `store` BEFORE emitting, so a frontend that misses
/// the event can pull the same value afterwards.
pub fn spawn_check(app: AppHandle, store: Arc<AppUpdateState>) {
    let current = app.package_info().version.to_string();
    if !should_check(&current, e2e_forced()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Some(notice) = fetch_notice(&current).await {
            store.set(notice.clone());
            let _ = app.emit(APP_UPDATE_EVENT, notice);
        }
    });
}

/// The production check: builds the forge client and asks GitHub.
async fn fetch_notice(current: &str) -> Option<AppUpdateNotice> {
    let client = ForgeClient::new().ok()?;
    fetch_notice_from(&client, LATEST_RELEASE_URL, current).await
}

/// One `releases/latest` request against `url`, decoded and compared against
/// `current`. The endpoint is a parameter so tests can point it at a local
/// mock server; production always passes [`LATEST_RELEASE_URL`].
///
/// Returns `Some` only for a release that carries both a tag and a page URL
/// and whose tag is newer than `current`. Every failure — transport, non-2xx
/// status, a body over [`MAX_RELEASE_BODY`], undecodable body, missing
/// fields — is a silent `None` logged at debug level, naming
/// [`LATEST_RELEASE_HOST`] and never the request URL.
async fn fetch_notice_from(
    client: &ForgeClient,
    url: &str,
    current: &str,
) -> Option<AppUpdateNotice> {
    let mut response = match client.get(url, true).await {
        Ok(response) => response,
        Err(_) => {
            tracing::debug!("self-update check: request to {LATEST_RELEASE_HOST} failed");
            return None;
        }
    };
    // Chunk by chunk rather than `bytes()`: the cap has to stop the READ, not
    // just the parse, or an endless body is buffered in full before anyone
    // objects. Dropping `response` here closes the connection.
    let mut body: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > MAX_RELEASE_BODY {
                    tracing::debug!(
                        "self-update check: release body from {LATEST_RELEASE_HOST} over the cap"
                    );
                    return None;
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => {
                tracing::debug!(
                    "self-update check: response from {LATEST_RELEASE_HOST} did not read"
                );
                return None;
            }
        }
    }
    let release: LatestRelease = match serde_json::from_slice(&body) {
        Ok(release) => release,
        Err(_) => {
            tracing::debug!("self-update check: release JSON did not decode");
            return None;
        }
    };
    if release.tag_name.is_empty()
        || release.html_url.is_empty()
        || !is_newer(current, &release.tag_name)
    {
        return None;
    }
    Some(AppUpdateNotice {
        tag: release.tag_name,
        url: release.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn newer_compares_semver_with_prerelease_precedence() {
        assert!(is_newer("0.9.0", "v0.9.1"));
        assert!(is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "v0.9.0"));
        assert!(!is_newer("0.9.1", "v0.9.0"));
        assert!(is_newer("0.9.0-beta1", "v0.9.0"));
        assert!(!is_newer("0.9.0", "v0.9.0-beta1"));
        assert!(is_newer("0.9.0-dev", "v9.9.9-e2e"));
        assert!(is_newer("0.9.0-dev", "V0.9.0"));
    }

    #[test]
    fn the_notice_store_starts_empty_and_round_trips() {
        let store = AppUpdateState::new();
        assert_eq!(store.get(), None);
        let notice = AppUpdateNotice {
            tag: "v9.9.9".to_string(),
            url: "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9".to_string(),
        };
        store.set(notice.clone());
        assert_eq!(store.get(), Some(notice));
    }

    #[test]
    fn garbage_is_never_newer() {
        assert!(!is_newer("0.9.0", "latest"));
        assert!(!is_newer("0.9.0", ""));
        assert!(!is_newer("not-a-version", "v1.0.0"));
    }

    #[test]
    fn dev_builds_are_recognised_and_gated() {
        assert!(is_dev_build("0.9.0-dev"));
        assert!(is_dev_build("0.9.0-dev.3"));
        assert!(!is_dev_build("0.9.0-beta4"));
        assert!(!is_dev_build("0.9.0"));
        assert!(!should_check("0.9.0-dev", false));
        assert!(should_check("0.9.0-dev", true));
        assert!(should_check("0.9.0", false));
    }

    /// Mounts `GET /releases/latest` on a fresh mock server and returns its
    /// full URL, so each test owns its own server and no shared state.
    async fn mock_release(server: &MockServer, response: ResponseTemplate) -> String {
        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(response)
            .mount(server)
            .await;
        format!("{}/releases/latest", server.uri())
    }

    #[tokio::test]
    async fn newer_release_becomes_a_notice() {
        let server = MockServer::start().await;
        let url = mock_release(
            &server,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v9.9.9",
                "html_url": "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9"
            })),
        )
        .await;
        let client = ForgeClient::new().unwrap();
        assert_eq!(
            fetch_notice_from(&client, &url, "0.9.0").await,
            Some(AppUpdateNotice {
                tag: "v9.9.9".to_string(),
                url: "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn a_failed_request_is_no_notice() {
        let server = MockServer::start().await;
        let url = mock_release(&server, ResponseTemplate::new(404)).await;
        let client = ForgeClient::new().unwrap();
        assert_eq!(fetch_notice_from(&client, &url, "0.9.0").await, None);
    }

    /// The cap exists so a peer that answers the update check with an
    /// endless body cannot make this process buffer it: the read is aborted
    /// as soon as the running total passes the cap. Valid JSON, so only the
    /// size can be what rejects it.
    #[tokio::test]
    async fn an_oversized_body_is_no_notice() {
        let server = MockServer::start().await;
        let url = mock_release(
            &server,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v9.9.9",
                "html_url": "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9",
                "body": "x".repeat(MAX_RELEASE_BODY + 1),
            })),
        )
        .await;
        let client = ForgeClient::new().unwrap();
        assert_eq!(fetch_notice_from(&client, &url, "0.9.0").await, None);
    }

    #[tokio::test]
    async fn an_older_release_is_no_notice() {
        let server = MockServer::start().await;
        let url = mock_release(
            &server,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v0.1.0",
                "html_url": "https://github.com/Sixdd6/grid-launcher/releases/tag/v0.1.0"
            })),
        )
        .await;
        let client = ForgeClient::new().unwrap();
        assert_eq!(fetch_notice_from(&client, &url, "0.9.0").await, None);
    }
}
