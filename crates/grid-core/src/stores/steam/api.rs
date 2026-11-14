//! The Steam Web API (`docs/stores-phase4-plan.md` "The Web API"):
//! `IPlayerService/GetOwnedGames` and `ISteamUser/GetPlayerSummaries` with
//! the user's key, and `IStoreBrowseService/GetItems` with none.
//!
//! This is the only file that reads the key (`expose_secret`), and only to
//! put it in a request's query at send time (`RequestBuilder::query`; Steam
//! has no header form). No `Url` carrying it is ever built, kept, returned
//! or formatted. Every `reqwest::Error` goes through `without_url()` before
//! anything looks at it, and a failed call becomes a [`SteamApiError`],
//! which carries no URL, no body and no key. The client follows no
//! redirect (the app builds it with `redirect::Policy::none()`), so a 3xx
//! reads as [`SteamApiError::Network`].

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use url::Url;

use super::{AppInfo, OwnedGame, OwnedGames, SteamApiError, SteamEndpoints, SteamId};

/// Each Web API request's limit.
pub const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// `GetItems` ids per request.
pub const APP_INFO_BATCH: usize = 100;

/// The cover file Steam keeps for every app, under `steam/apps/<appid>/`.
const PLAIN_COVER: &str = "library_600x900.jpg";

const OWNED_PATH: &str = "IPlayerService/GetOwnedGames/v1/";
const SUMMARIES_PATH: &str = "ISteamUser/GetPlayerSummaries/v2/";
const ITEMS_PATH: &str = "IStoreBrowseService/GetItems/v1/";

/// A request error, reduced to GRID's reading. The URL is dropped first.
fn network(error: reqwest::Error) -> SteamApiError {
    let error = error.without_url();
    tracing::debug!("Steam Web API request failed: {error}");
    SteamApiError::Network
}

/// The endpoint at `path` under the API base (no query: the key is added
/// at send time).
fn endpoint(endpoints: &SteamEndpoints, path: &str) -> Result<Url, SteamApiError> {
    endpoints.api.join(path).map_err(|_| SteamApiError::Network)
}

/// GRID's reading of a non-success status: 401 and 403 are a rejected key,
/// anything else (3xx, 429, 5xx, …) is "couldn't reach".
fn status_error(status: reqwest::StatusCode) -> SteamApiError {
    match status.as_u16() {
        status @ (401 | 403) => SteamApiError::Rejected { status },
        _ => SteamApiError::Network,
    }
}

/// Sends a key-carrying GET with `params` and the key in the query, and
/// decodes a 2xx JSON body.
async fn get_with_key<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    endpoint: Url,
    secret: &SecretString,
    params: &[(&str, String)],
) -> Result<T, SteamApiError> {
    let response = http
        .get(endpoint)
        .query(&[("key", secret.expose_secret())])
        .query(params)
        .timeout(API_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(network)?;
    let status = response.status();
    if !status.is_success() {
        return Err(status_error(status));
    }
    response.json::<T>().await.map_err(network)
}

#[derive(Deserialize)]
struct Envelope<T> {
    response: T,
}

#[derive(Deserialize)]
struct OwnedResponse {
    /// Absent when the profile's game details are private.
    game_count: Option<u64>,
    #[serde(default)]
    games: Vec<RawOwned>,
}

#[derive(Deserialize)]
struct RawOwned {
    appid: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    rtime_last_played: i64,
}

/// The owned list from a decoded `GetOwnedGames` answer: no `game_count`
/// is a private profile.
fn owned_from(response: OwnedResponse) -> Result<OwnedGames, SteamApiError> {
    if response.game_count.is_none() {
        return Err(SteamApiError::Private);
    }
    Ok(OwnedGames {
        games: response
            .games
            .into_iter()
            .filter(|game| game.appid != 0)
            .map(|game| OwnedGame {
                appid: game.appid,
                name: game.name.trim().to_string(),
                last_played: game.rtime_last_played.max(0),
            })
            .collect(),
    })
}

/// `IPlayerService/GetOwnedGames`: the account's games, free games played
/// included, with their names and last-played times.
pub async fn owned(
    http: &reqwest::Client,
    endpoints: &SteamEndpoints,
    secret: &SecretString,
    id: SteamId,
) -> Result<OwnedGames, SteamApiError> {
    let params = [
        ("steamid", id.to_string()),
        ("include_appinfo", "1".to_string()),
        ("include_played_free_games", "1".to_string()),
        ("include_free_sub", "0".to_string()),
        ("skip_unvetted_apps", "0".to_string()),
        ("format", "json".to_string()),
    ];
    let envelope: Envelope<OwnedResponse> =
        get_with_key(http, endpoint(endpoints, OWNED_PATH)?, secret, &params).await?;
    owned_from(envelope.response)
}

#[derive(Deserialize)]
struct SummariesResponse {
    #[serde(default)]
    players: Vec<RawPlayer>,
}

#[derive(Deserialize)]
struct RawPlayer {
    #[serde(default)]
    steamid: String,
    #[serde(default)]
    personaname: String,
}

/// `ISteamUser/GetPlayerSummaries`: the persona name. No player is
/// [`SteamApiError::NoSuchAccount`].
pub async fn persona(
    http: &reqwest::Client,
    endpoints: &SteamEndpoints,
    secret: &SecretString,
    id: SteamId,
) -> Result<String, SteamApiError> {
    let params = [("steamids", id.to_string()), ("format", "json".to_string())];
    let envelope: Envelope<SummariesResponse> =
        get_with_key(http, endpoint(endpoints, SUMMARIES_PATH)?, secret, &params).await?;
    let wanted = id.to_string();
    envelope
        .response
        .players
        .into_iter()
        .find(|player| player.steamid == wanted)
        .map(|player| player.personaname.trim().to_string())
        .ok_or(SteamApiError::NoSuchAccount)
}

#[derive(Deserialize, Default)]
struct ItemsResponse {
    #[serde(default)]
    store_items: Vec<RawItem>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawItem {
    /// The asked id (kept even when the app has no store page).
    id: u32,
    success: u32,
    appid: u32,
    #[serde(rename = "type")]
    kind: Option<u8>,
    name: String,
    basic_info: RawBasicInfo,
    release: RawRelease,
    assets: RawAssets,
    screenshots: RawScreenshots,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawBasicInfo {
    short_description: String,
    developers: Vec<RawNamed>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawNamed {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawRelease {
    steam_release_date: i64,
    original_release_date: i64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawAssets {
    asset_url_format: String,
    library_capsule: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawScreenshots {
    all_ages_screenshots: Vec<RawScreenshot>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawScreenshot {
    filename: String,
}

/// `<art base>steam/apps/<appid>/library_600x900.jpg`.
fn plain_cover(art_base: &Url, appid: u32) -> String {
    art_base
        .join(&format!("steam/apps/{appid}/{PLAIN_COVER}"))
        .map(String::from)
        .unwrap_or_default()
}

/// The year of a Unix time, `""` for none.
fn year(seconds: i64) -> String {
    if seconds <= 0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|time| time.format("%Y").to_string())
        .unwrap_or_default()
}

/// GRID's reading of one `GetItems` entry for the asked `appid`. An entry
/// with no store page (`success` 15, `appid` 0) gives the plain cover and
/// no type, so the filter's rules 1–2 decide alone.
fn app_info_from(art_base: &Url, appid: u32, item: RawItem) -> AppInfo {
    if item.success != 1 || item.appid != appid {
        return AppInfo {
            appid,
            cover: plain_cover(art_base, appid),
            ..Default::default()
        };
    }
    let cover = if item.assets.library_capsule.is_empty() || item.assets.asset_url_format.is_empty()
    {
        plain_cover(art_base, appid)
    } else {
        let relative = item
            .assets
            .asset_url_format
            .replace("${FILENAME}", &item.assets.library_capsule);
        art_base
            .join(&relative)
            .map(String::from)
            .unwrap_or_else(|_| plain_cover(art_base, appid))
    };
    let release = if item.release.steam_release_date > 0 {
        item.release.steam_release_date
    } else {
        item.release.original_release_date
    };
    AppInfo {
        appid,
        kind: item.kind,
        name: item.name.trim().to_string(),
        description: item.basic_info.short_description.trim().to_string(),
        developers: item
            .basic_info
            .developers
            .into_iter()
            .map(|developer| developer.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect(),
        year: year(release),
        cover,
        screenshots: item
            .screenshots
            .all_ages_screenshots
            .into_iter()
            .filter(|shot| !shot.filename.is_empty())
            .filter_map(|shot| art_base.join(&shot.filename).ok().map(String::from))
            .collect(),
    }
}

/// Matches `GetItems`' entries to the asked ids. Steam answers one entry
/// per asked id, in order, and an app with no store page comes back with
/// `appid` 0, so entries are matched by position; when the count differs,
/// by each entry's `id`.
fn match_items(art_base: &Url, asked: &[u32], items: Vec<RawItem>) -> Vec<AppInfo> {
    if items.len() == asked.len() {
        return asked
            .iter()
            .zip(items)
            .map(|(&appid, item)| app_info_from(art_base, appid, item))
            .collect();
    }
    let mut items = items;
    asked
        .iter()
        .filter_map(|&appid| {
            let at = items.iter().position(|item| item.id == appid)?;
            Some(app_info_from(art_base, appid, items.swap_remove(at)))
        })
        .collect()
}

/// The `input_json` for one `GetItems` request.
fn items_input(ids: &[u32]) -> String {
    serde_json::json!({
        "ids": ids.iter().map(|appid| serde_json::json!({ "appid": appid })).collect::<Vec<_>>(),
        "context": { "language": "english", "country_code": "US" },
        "data_request": {
            "include_basic_info": true,
            "include_release": true,
            "include_screenshots": true,
            "include_assets": true,
        },
    })
    .to_string()
}

/// One `GetItems` request (no key). A failure gives no entries.
async fn app_info_batch(
    http: &reqwest::Client,
    endpoints: &SteamEndpoints,
    ids: &[u32],
) -> Vec<AppInfo> {
    let Ok(endpoint) = endpoint(endpoints, ITEMS_PATH) else {
        return Vec::new();
    };
    let sent = http
        .get(endpoint)
        .query(&[("input_json", items_input(ids))])
        .timeout(API_REQUEST_TIMEOUT)
        .send()
        .await;
    let response = match sent {
        Ok(response) if response.status().is_success() => response,
        Ok(_) => return Vec::new(),
        Err(error) => {
            let _ = network(error);
            return Vec::new();
        }
    };
    match response.json::<Envelope<ItemsResponse>>().await {
        Ok(envelope) => match_items(&endpoints.art_base, ids, envelope.response.store_items),
        Err(error) => {
            let _ = network(error);
            Vec::new()
        }
    }
}

/// `IStoreBrowseService/GetItems` for `ids` (no key), [`APP_INFO_BATCH`]
/// per request, one entry per answered id; a failed batch gives none.
pub async fn app_info(
    http: &reqwest::Client,
    endpoints: &SteamEndpoints,
    ids: &[u32],
) -> Vec<AppInfo> {
    let mut wanted: Vec<u32> = Vec::new();
    for &appid in ids {
        if appid != 0 && !wanted.contains(&appid) {
            wanted.push(appid);
        }
    }
    let mut found = Vec::new();
    for batch in wanted.chunks(APP_INFO_BATCH) {
        found.extend(app_info_batch(http, endpoints, batch).await);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art() -> Url {
        Url::parse(super::super::STEAM_ART_BASE).unwrap()
    }

    #[test]
    fn rejected_private_and_network_by_status() {
        use reqwest::StatusCode;
        assert_eq!(
            status_error(StatusCode::UNAUTHORIZED),
            SteamApiError::Rejected { status: 401 }
        );
        assert_eq!(
            status_error(StatusCode::FORBIDDEN),
            SteamApiError::Rejected { status: 403 }
        );
        for other in [302, 404, 429, 500, 503] {
            assert_eq!(
                status_error(StatusCode::from_u16(other).unwrap()),
                SteamApiError::Network
            );
        }
        let private: Envelope<OwnedResponse> = serde_json::from_str(r#"{"response":{}}"#).unwrap();
        assert_eq!(owned_from(private.response), Err(SteamApiError::Private));
        let empty: Envelope<OwnedResponse> =
            serde_json::from_str(r#"{"response":{"game_count":0}}"#).unwrap();
        assert_eq!(owned_from(empty.response), Ok(OwnedGames::default()));
    }

    #[test]
    fn get_items_entries_match_by_position() {
        let items: Envelope<ItemsResponse> = serde_json::from_str(
            r#"{"response":{"store_items":[
              {"item_type":0,"id":620,"success":1,"visible":true,"name":"Portal 2","appid":620,"type":0,
               "basic_info":{"short_description":"Testing.","developers":[{"name":"Valve"}]},
               "release":{"steam_release_date":1303171200},
               "assets":{"asset_url_format":"steam/apps/620/${FILENAME}?t=1","library_capsule":"library_600x900.jpg"},
               "screenshots":{"all_ages_screenshots":[{"filename":"steam/apps/620/ss_a.jpg?t=1","ordinal":1}]}},
              {"item_type":0,"id":2805730,"success":15,"visible":false,"name":"","appid":0},
              {"item_type":0,"id":2138330,"success":1,"name":"A DLC","appid":2138330,"type":4,
               "assets":{"asset_url_format":"steam/apps/2138330/${FILENAME}"}},
              {"item_type":0,"id":1245620,"success":1,"name":"Hash Folder Game","appid":1245620,"type":0,
               "assets":{"asset_url_format":"steam/apps/1245620/${FILENAME}?t=2","library_capsule":"0123abcd/library_capsule.jpg"}}
            ]}}"#,
        )
        .unwrap();
        let infos = match_items(
            &art(),
            &[620, 2805730, 2138330, 1245620],
            items.response.store_items,
        );
        assert_eq!(infos.len(), 4);
        assert_eq!(infos[0].appid, 620);
        assert_eq!(infos[0].kind, Some(0));
        assert_eq!(infos[0].name, "Portal 2");
        assert_eq!(infos[0].description, "Testing.");
        assert_eq!(infos[0].developers, ["Valve"]);
        assert_eq!(infos[0].year, "2011");
        assert_eq!(
            infos[0].cover,
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg?t=1"
        );
        assert_eq!(
            infos[0].screenshots,
            ["https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/ss_a.jpg?t=1"]
        );
        // No store page: matched by position, no type, the plain cover.
        assert_eq!(infos[1].appid, 2805730);
        assert_eq!(infos[1].kind, None);
        assert_eq!(
            infos[1].cover,
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/2805730/library_600x900.jpg"
        );
        // A DLC with no capsule: the plain cover.
        assert_eq!(infos[2].kind, Some(4));
        assert!(infos[2]
            .cover
            .ends_with("/steam/apps/2138330/library_600x900.jpg"));
        assert!(infos[3]
            .cover
            .ends_with("/steam/apps/1245620/0123abcd/library_capsule.jpg?t=2"));
    }

    #[test]
    fn a_short_answer_matches_by_id() {
        let items: Envelope<ItemsResponse> = serde_json::from_str(
            r#"{"response":{"store_items":[{"id":440,"success":1,"appid":440,"type":0,"name":"Second"}]}}"#,
        )
        .unwrap();
        let infos = match_items(&art(), &[620, 440], items.response.store_items);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].appid, 440);
        assert_eq!(infos[0].name, "Second");
    }

    #[test]
    fn get_items_input_names_every_id() {
        let input: serde_json::Value = serde_json::from_str(&items_input(&[620, 440])).unwrap();
        assert_eq!(input["ids"][0]["appid"], 620);
        assert_eq!(input["ids"][1]["appid"], 440);
        assert_eq!(input["data_request"]["include_assets"], true);
        assert_eq!(year(0), "");
    }
}
