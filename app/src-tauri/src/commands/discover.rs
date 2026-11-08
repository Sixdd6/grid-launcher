//! Server › Discover: the feed (New releases and Recently added) and the
//! dominant shade a generated logo is coloured from. Thin wrappers — the
//! preferred platforms, the date window and the exclusions live in
//! `grid_core::romm::discover`, the shade in `grid_core::images::shade`.

use grid_core::config::Config;
use grid_core::images::shade::ensure_dominant_shade;
use grid_core::images::urls::{filter_to_server_host, resolve_image_url};
use grid_core::romm::discover::{
    new_releases, preferred_platform_ids, recently_added, HostOs, NEW_RELEASES_MAX,
    RECENTLY_ADDED_MAX,
};
use grid_core::romm::GameSummary;
use serde::Serialize;
use tauri::State;

use super::{err, AppState};

/// How many games the release-date page asks for. Upcoming games sort
/// first (their dates are in the future) and are dropped, so the page is
/// well over [`NEW_RELEASES_MAX`].
const NEW_RELEASES_PAGE: usize = 200;

/// How many newest games the added-date page asks for: enough to fill
/// Recently added after every New releases game is left out of it.
const RECENTLY_ADDED_PAGE: usize = NEW_RELEASES_MAX + RECENTLY_ADDED_MAX + 17;

/// [`discover_feed`]'s answer.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoverFeed {
    /// The coverflow, newest release first.
    pub new_releases: Vec<GameSummary>,
    /// Newest on the server first, without the coverflow's games.
    pub recently_added: Vec<GameSummary>,
}

/// The Discover feed: the preferred platforms from the saved defaults, then
/// two `/api/roms` pages (by release date on those platforms, by added date
/// on all), each filtered and sorted again in grid-core — the server's order
/// is never trusted alone.
#[tauri::command]
pub async fn discover_feed(state: State<'_, AppState>) -> Result<DiscoverFeed, String> {
    let client = state.session.client().ok_or("not connected")?;
    let config = tokio::task::spawn_blocking(|| Config::load(&Config::default_path()))
        .await
        .map_err(|e| format!("discover_feed did not finish: {e}"))?
        .map_err(err)?;
    let platforms = client.platforms().await.map_err(err)?;
    let preferred = preferred_platform_ids(
        &platforms,
        &config.default_emulators,
        !config.default_compat_tool.trim().is_empty(),
        HostOs::current(),
    );
    let releases = async {
        if preferred.is_empty() {
            return Ok::<_, grid_core::romm::RommError>(Vec::new());
        }
        client
            .recent_games("first_release_date", &preferred, NEW_RELEASES_PAGE)
            .await
    };
    let added = client.recent_games("created_at", &[], RECENTLY_ADDED_PAGE);
    let (releases, added) = tokio::try_join!(releases, added).map_err(err)?;
    let new_releases = new_releases(releases, &preferred, chrono::Utc::now());
    let shown: Vec<i64> = new_releases.iter().map(|g| g.id).collect();
    Ok(DiscoverFeed {
        recently_added: recently_added(added, &shown),
        new_releases,
    })
}

/// The dominant shade `[r, g, b]` of the screenshot at `url`, for a
/// generated logo's colours. Same resolution and host filter as
/// `ensure_image`; computed once per image and remembered.
#[tauri::command]
pub async fn image_dominant_shade(
    state: State<'_, AppState>,
    url: String,
) -> Result<[u8; 3], String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("the image is not hosted on the connected server".to_string());
    }
    let client = state.session.client();
    ensure_dominant_shade(state.session.cache(), client.as_deref(), &resolved)
        .await
        .map_err(err)
}
