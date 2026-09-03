//! Check-only launcher self-update (spec §5, doc 10 OQ14 ruling D-10-h):
//! one GitHub `releases/latest` request per process, a banner when the tag
//! is newer than the running version, nothing downloaded or installed.
//!
//! Goes through grid-core's `ForgeClient`: no RomM credential can reach the
//! forge, and the E2E build's `GRID_LAUNCHER_E2E_FORGE_BASE` redirect
//! applies. Every failure is silent at debug level, naming the host only.

use grid_core::launch::forge::ForgeClient;
use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub const APP_UPDATE_EVENT: &str = "app-update-available";
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/Sixdd6/grid-launcher/releases/latest";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppUpdateNotice {
    pub tag: String,
    pub url: String,
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
pub fn spawn_check(app: AppHandle) {
    let current = app.package_info().version.to_string();
    if !should_check(&current, e2e_forced()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Some(notice) = fetch_notice(&current).await {
            let _ = app.emit(APP_UPDATE_EVENT, notice);
        }
    });
}

async fn fetch_notice(current: &str) -> Option<AppUpdateNotice> {
    let client = ForgeClient::new().ok()?;
    let response = match client.get(LATEST_RELEASE_URL, true).await {
        Ok(response) => response,
        Err(_) => {
            tracing::debug!("self-update check: request to api.github.com failed");
            return None;
        }
    };
    let release: LatestRelease = match response.json().await {
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
}

/// Only meaningful when built with `--features e2e`: that is the only build
/// where `ForgeClient::get` honors `GRID_LAUNCHER_E2E_FORGE_BASE`, which is
/// how this test points `fetch_notice` at a `MockServer` instead of the
/// real `api.github.com`. `cargo test` runs tests in the same binary on
/// separate threads by default, and this process-global env var would race
/// against a second test that also set it — so both the success and 404
/// cases live in this one test function, the only place in the crate that
/// touches the variable.
#[cfg(all(test, feature = "e2e"))]
mod e2e_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const VAR: &str = "GRID_LAUNCHER_E2E_FORGE_BASE";
    const RELEASE_PATH: &str = "/api.github.com/repos/Sixdd6/grid-launcher/releases/latest";

    #[tokio::test]
    async fn fetch_notice_reflects_the_mocked_forge_response() {
        let ok_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(RELEASE_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v9.9.9",
                "html_url": "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9"
            })))
            .mount(&ok_server)
            .await;
        std::env::set_var(VAR, ok_server.uri());
        let newer = fetch_notice("0.9.0").await;
        assert_eq!(
            newer,
            Some(AppUpdateNotice {
                tag: "v9.9.9".to_string(),
                url: "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9".to_string(),
            })
        );

        let not_found_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(RELEASE_PATH))
            .respond_with(ResponseTemplate::new(404))
            .mount(&not_found_server)
            .await;
        std::env::set_var(VAR, not_found_server.uri());
        let none = fetch_notice("0.9.0").await;

        std::env::remove_var(VAR);
        assert_eq!(none, None);
    }
}
