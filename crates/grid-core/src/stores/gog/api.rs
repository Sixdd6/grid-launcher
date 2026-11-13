//! GOG's HTTP APIs: the account, the owned library (galaxy-library, then
//! embed's owned list), product details (gamesdb, then api.gog.com), the
//! content-system builds, the Linux installer versions, the Galaxy remote
//! config (save locations) and the cloudstorage listing.
//!
//! The bearer is a [`SecretString`] passed in by the caller and turned into
//! a header only here (one of the two allowed `expose_secret` sites for
//! GOG). Errors are GRID's words: [`StoreError::LoginExpired`] on a 401,
//! [`StoreError::Network`] ("couldn't reach GOG") otherwise. Response
//! bodies are never logged.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read as _;

use futures_util::StreamExt as _;
use secrecy::{ExposeSecret as _, SecretString};
use serde_json::Value;
use url::Url;

use super::meta::{GogPlatform, SaveLocation};
use super::{GogAccount, GogEndpoints, GogProduct, GogProductDlc};
use crate::stores::art;
use crate::stores::StoreError;

/// The name every GOG network error carries.
pub const GOG: &str = "GOG";

/// How many products one api.gog.com batch asks for.
const PRODUCT_BATCH: usize = 50;
/// How many gamesdb requests run at once.
const GAMESDB_PARALLEL: usize = 8;
/// The Galaxy client version remote-config answers for.
const REMOTE_CONFIG_VERSION: &str = "2.0.45";
/// The md5 cloudstorage gives a deleted-file marker.
pub const EMPTY_CLOUD_MD5: &str = "aadd86936a80ee8a369579c3926f1b3c";

fn network() -> StoreError {
    StoreError::Network(GOG.to_string())
}

fn join(base: &Url, path: &str) -> Result<Url, StoreError> {
    base.join(path).map_err(|_| network())
}

/// GETs `url` (with the bearer when given) and parses JSON. `Ok(None)` on
/// a 404.
async fn get_json(
    http: &reqwest::Client,
    url: Url,
    bearer: Option<&SecretString>,
) -> Result<Option<Value>, StoreError> {
    let mut request = http.get(url).header("Accept", "application/json");
    if let Some(token) = bearer {
        request = request.header("Authorization", format!("Bearer {}", token.expose_secret()));
    }
    let resp = request.send().await.map_err(|_| network())?;
    match resp.status().as_u16() {
        200..=299 => {}
        401 => return Err(StoreError::LoginExpired),
        404 => return Ok(None),
        _ => return Err(network()),
    }
    resp.json::<Value>().await.map(Some).map_err(|_| network())
}

async fn get_bytes(
    http: &reqwest::Client,
    url: Url,
    bearer: Option<&SecretString>,
) -> Result<Vec<u8>, StoreError> {
    let mut request = http.get(url);
    if let Some(token) = bearer {
        request = request.header("Authorization", format!("Bearer {}", token.expose_secret()));
    }
    let resp = request.send().await.map_err(|_| network())?;
    match resp.status().as_u16() {
        200..=299 => {}
        401 => return Err(StoreError::LoginExpired),
        _ => return Err(network()),
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|_| network())
}

// --- small JSON helpers -----------------------------------------------------

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// A gamesdb localized field: `{"*": "x", "en-US": "y"}` → the `*` value,
/// else `en-US`, else any; a plain string passes through.
fn localized(value: Option<&Value>) -> String {
    match value {
        Some(Value::Object(map)) => ["*", "en-US"]
            .iter()
            .find_map(|key| map.get(*key).and_then(Value::as_str))
            .or_else(|| map.values().find_map(Value::as_str))
            .unwrap_or("")
            .trim()
            .to_string(),
        other => text(other),
    }
}

fn names(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| match item {
                    Value::Object(map) => localized(map.get("name")),
                    other => localized(Some(other)),
                })
                .filter(|name| !name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

// --- account ----------------------------------------------------------------

/// The account `embed.gog.com/userData.json` describes, `None` when it
/// says the user is not logged in.
pub fn parse_user_data(value: &Value) -> Option<GogAccount> {
    if value.get("isLoggedIn").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let username = text(value.get("username"));
    let user_id = [value.get("galaxyUserId"), value.get("userId")]
        .into_iter()
        .map(text)
        .find(|id| !id.is_empty())
        .unwrap_or_default();
    (!username.is_empty() || !user_id.is_empty()).then_some(GogAccount { username, user_id })
}

/// The signed-in account. A 401 or "not logged in" is
/// [`StoreError::LoginExpired`].
pub async fn user_data(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: &SecretString,
) -> Result<GogAccount, StoreError> {
    let url = join(&ep.embed, "userData.json")?;
    let value = get_json(http, url, Some(bearer))
        .await?
        .ok_or_else(network)?;
    parse_user_data(&value).ok_or(StoreError::LoginExpired)
}

// --- owned list -------------------------------------------------------------

/// One galaxy-library page: the GOG product ids, and the next page's
/// token.
pub fn parse_library_page(value: &Value) -> (Vec<String>, Option<String>) {
    let ids = value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("platform_id")
                        .and_then(Value::as_str)
                        .is_none_or(|p| p == "gog")
                })
                .map(|item| text(item.get("external_id")))
                .filter(|id| !id.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let next = Some(text(value.get("next_page_token"))).filter(|t| !t.is_empty());
    (ids, next)
}

/// embed's `user/data/games`: `{"owned": [ids]}`.
pub fn parse_owned_ids(value: &Value) -> Vec<String> {
    value
        .get("owned")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .map(|id| text(Some(id)))
                .filter(|id| !id.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Every owned product id: galaxy-library (paged), falling back to
/// embed's owned list when the library fails for anything but a 401.
pub async fn owned_ids(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: &SecretString,
    user_id: &str,
) -> Result<Vec<String>, StoreError> {
    match library_ids(http, ep, bearer, user_id).await {
        Ok(ids) => Ok(ids),
        Err(StoreError::LoginExpired) => Err(StoreError::LoginExpired),
        Err(_) => {
            let url = join(&ep.embed, "user/data/games")?;
            let value = get_json(http, url, Some(bearer))
                .await?
                .ok_or_else(network)?;
            Ok(parse_owned_ids(&value))
        }
    }
}

async fn library_ids(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: &SecretString,
    user_id: &str,
) -> Result<Vec<String>, StoreError> {
    if user_id.trim().is_empty() {
        return Err(network());
    }
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    let mut token: Option<String> = None;
    // A guard against a server that never stops paging.
    for _ in 0..500 {
        let mut url = join(&ep.library, &format!("users/{}/releases", user_id.trim()))?;
        if let Some(page) = &token {
            url.query_pairs_mut().append_pair("page_token", page);
        }
        let value = get_json(http, url, Some(bearer))
            .await?
            .ok_or_else(network)?;
        let (page_ids, next) = parse_library_page(&value);
        for id in page_ids {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        }
        match next {
            Some(next) if Some(&next) != token.as_ref() => token = Some(next),
            _ => break,
        }
    }
    Ok(ids)
}

/// What api.gog.com says about one product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiProduct {
    pub id: String,
    pub title: String,
    pub is_dlc: bool,
    pub dlc: Vec<GogProductDlc>,
    pub cover: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub release_date: String,
    pub platforms: Vec<GogPlatform>,
}

/// One api.gog.com product object.
pub fn parse_api_product(value: &Value) -> Option<ApiProduct> {
    let id = text(value.get("id"));
    if id.is_empty() {
        return None;
    }
    let mut dlc: Vec<GogProductDlc> = Vec::new();
    let mut push_dlc = |id: String, title: String| {
        if !id.is_empty() && !dlc.iter().any(|d| d.id == id) {
            dlc.push(GogProductDlc { id, title });
        }
    };
    if let Some(items) = value.get("expanded_dlcs").and_then(Value::as_array) {
        for item in items {
            push_dlc(text(item.get("id")), text(item.get("title")));
        }
    }
    if let Some(items) = value
        .get("dlcs")
        .and_then(|d| d.get("products"))
        .and_then(Value::as_array)
    {
        for item in items {
            push_dlc(text(item.get("id")), String::new());
        }
    }
    let images = value.get("images");
    let cover = images
        .and_then(|i| i.get("logo2x").or_else(|| i.get("logo")))
        .and_then(Value::as_str)
        .map(art::absolute_url)
        .unwrap_or_default();
    let screenshots = value
        .get("screenshots")
        .and_then(Value::as_array)
        .map(|shots| {
            shots
                .iter()
                .filter_map(|shot| shot.get("formatter_template_url").and_then(Value::as_str))
                .map(|template| art::fill_template(template, art::SCREENSHOT_FORMATTER, "jpg"))
                .filter(|url| !url.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let description = value
        .get("description")
        .map(|d| {
            let lead = text(d.get("lead"));
            if lead.is_empty() {
                text(d.get("full"))
            } else {
                lead
            }
        })
        .unwrap_or_default();
    let compat = value.get("content_system_compatibility");
    let mut platforms = Vec::new();
    if compat
        .and_then(|c| c.get("windows"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        platforms.push(GogPlatform::Windows);
    }
    if compat
        .and_then(|c| c.get("linux"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        platforms.push(GogPlatform::Linux);
    }
    Some(ApiProduct {
        id,
        title: text(value.get("title")),
        is_dlc: text(value.get("game_type")) == "dlc",
        dlc,
        cover,
        screenshots,
        description,
        release_date: date_only(&text(value.get("release_date"))),
        platforms,
    })
}

/// A batch answer (an array) or a single product.
pub fn parse_api_products(value: &Value) -> Vec<ApiProduct> {
    match value {
        Value::Array(items) => items.iter().filter_map(parse_api_product).collect(),
        other => parse_api_product(other).into_iter().collect(),
    }
}

/// What gamesdb says about one GOG release.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GamesDbRelease {
    pub title: String,
    pub is_dlc: Option<bool>,
    pub dlc_ids: Vec<String>,
    pub cover: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub developer: String,
    pub publisher: String,
    pub genres: Vec<String>,
    pub release_date: String,
    pub platforms: Vec<GogPlatform>,
}

/// One gamesdb `external_releases` answer.
pub fn parse_gamesdb(value: &Value) -> Option<GamesDbRelease> {
    let game = value.get("game").unwrap_or(&Value::Null);
    let title = [localized(value.get("title")), localized(game.get("title"))]
        .into_iter()
        .find(|t| !t.is_empty())
        .unwrap_or_default();
    if title.is_empty() && game.is_null() {
        return None;
    }
    let format = |node: Option<&Value>| {
        node.and_then(|n| n.get("url_format"))
            .and_then(Value::as_str)
            .map(|f| art::fill_template(f, "", "jpg"))
            .unwrap_or_default()
    };
    let cover = [
        format(game.get("vertical_cover")),
        format(game.get("cover")),
    ]
    .into_iter()
    .find(|c| !c.is_empty())
    .unwrap_or_default();
    let screenshots = game
        .get("screenshots")
        .and_then(Value::as_array)
        .map(|shots| {
            shots
                .iter()
                .map(|shot| format(Some(shot)))
                .filter(|url| !url.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let platforms = value
        .get("supported_operating_systems")
        .and_then(Value::as_array)
        .map(|systems| {
            let mut out = Vec::new();
            for system in systems {
                let slug = match system {
                    Value::Object(map) => text(map.get("slug")),
                    other => text(Some(other)),
                };
                let platform = match slug.as_str() {
                    "windows" => Some(GogPlatform::Windows),
                    "linux" => Some(GogPlatform::Linux),
                    _ => None,
                };
                if let Some(p) = platform {
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
            out
        })
        .unwrap_or_default();
    let kind = text(value.get("type"));
    Some(GamesDbRelease {
        title,
        is_dlc: (!kind.is_empty()).then(|| kind == "dlc"),
        dlc_ids: value
            .get("dlcs_ids")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .map(|id| text(Some(id)))
                    .filter(|id| !id.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        cover,
        screenshots,
        description: localized(game.get("summary").or_else(|| value.get("summary"))),
        developer: names(game.get("developers"))
            .into_iter()
            .next()
            .unwrap_or_default(),
        publisher: names(game.get("publishers"))
            .into_iter()
            .next()
            .unwrap_or_default(),
        genres: names(game.get("genres")),
        release_date: date_only(&text(
            game.get("first_release_date")
                .or_else(|| value.get("release_date")),
        )),
        platforms,
    })
}

/// `2008-09-16T00:00:00+0300` → `2008-09-16`; anything not starting with a
/// date → `""`.
fn date_only(value: &str) -> String {
    let date: String = value.trim().chars().take(10).collect();
    let bytes = date.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && date
            .chars()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if shaped {
        date
    } else {
        String::new()
    }
}

/// Merges the two sources into the owned list: gamesdb's details first,
/// api.gog.com filling the gaps; DLC folded into their base game (only
/// owned DLC, and a DLC no owned base lists is dropped). Sorted by title.
pub fn merge_owned(
    ids: &[String],
    api: &HashMap<String, ApiProduct>,
    gamesdb: &HashMap<String, GamesDbRelease>,
) -> Vec<GogProduct> {
    let owned: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let title_of = |id: &str| {
        gamesdb
            .get(id)
            .map(|g| g.title.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| api.get(id).map(|a| a.title.clone()))
            .unwrap_or_default()
    };
    let is_dlc = |id: &str| {
        api.get(id)
            .map(|a| a.is_dlc)
            .or_else(|| gamesdb.get(id).and_then(|g| g.is_dlc))
            .unwrap_or(false)
    };
    let mut products = Vec::new();
    for id in ids {
        if is_dlc(id) {
            continue;
        }
        let a = api.get(id);
        let g = gamesdb.get(id);
        if a.is_none() && g.is_none() {
            continue;
        }
        let pick = |from_g: Option<String>, from_a: Option<String>| {
            from_g
                .filter(|v| !v.is_empty())
                .or(from_a)
                .unwrap_or_default()
        };
        let mut dlc_ids: Vec<String> = Vec::new();
        for d in a
            .map(|a| a.dlc.iter().map(|d| d.id.clone()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .chain(g.map(|g| g.dlc_ids.clone()).unwrap_or_default())
        {
            if owned.contains(d.as_str()) && !dlc_ids.contains(&d) {
                dlc_ids.push(d);
            }
        }
        let dlc = dlc_ids
            .into_iter()
            .map(|d| {
                let mut title = title_of(&d);
                if title.is_empty() {
                    title = a
                        .and_then(|a| a.dlc.iter().find(|x| x.id == d))
                        .map(|x| x.title.clone())
                        .unwrap_or_default();
                }
                GogProductDlc { id: d, title }
            })
            .collect();
        let platforms = g
            .map(|g| g.platforms.clone())
            .filter(|p| !p.is_empty())
            .or_else(|| a.map(|a| a.platforms.clone()))
            .unwrap_or_default();
        products.push(GogProduct {
            id: id.clone(),
            title: title_of(id),
            cover: pick(g.map(|g| g.cover.clone()), a.map(|a| a.cover.clone())),
            screenshots: g
                .map(|g| g.screenshots.clone())
                .filter(|s| !s.is_empty())
                .or_else(|| a.map(|a| a.screenshots.clone()))
                .unwrap_or_default(),
            description: pick(
                g.map(|g| g.description.clone()),
                a.map(|a| a.description.clone()),
            ),
            developer: g.map(|g| g.developer.clone()).unwrap_or_default(),
            publisher: g.map(|g| g.publisher.clone()).unwrap_or_default(),
            genres: g.map(|g| g.genres.clone()).unwrap_or_default(),
            release_date: pick(
                g.map(|g| g.release_date.clone()),
                a.map(|a| a.release_date.clone()),
            ),
            platforms,
            dlc,
        });
    }
    products.sort_by(|x, y| {
        x.title
            .to_lowercase()
            .cmp(&y.title.to_lowercase())
            .then_with(|| x.id.cmp(&y.id))
    });
    products
}

/// The owned games with their details, DLC folded in.
pub async fn owned_products(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: &SecretString,
    user_id: &str,
) -> Result<Vec<GogProduct>, StoreError> {
    let ids = owned_ids(http, ep, bearer, user_id).await?;
    let mut api = HashMap::new();
    for chunk in ids.chunks(PRODUCT_BATCH) {
        let mut url = join(&ep.api, "products")?;
        url.query_pairs_mut()
            .append_pair("ids", &chunk.join(","))
            .append_pair("expand", "description,screenshots,expanded_dlcs");
        match get_json(http, url, Some(bearer)).await {
            Ok(Some(value)) => {
                for product in parse_api_products(&value) {
                    api.insert(product.id.clone(), product);
                }
            }
            Ok(None) => {}
            Err(StoreError::LoginExpired) => return Err(StoreError::LoginExpired),
            Err(_) => {}
        }
    }
    let gamesdb: HashMap<String, GamesDbRelease> = futures_util::stream::iter(ids.iter().cloned())
        .map(|id| async move {
            let url = join(
                &ep.gamesdb,
                &format!("platforms/gog/external_releases/{id}"),
            )
            .ok()?;
            let value = get_json(http, url, None).await.ok()??;
            parse_gamesdb(&value).map(|release| (id, release))
        })
        .buffer_unordered(GAMESDB_PARALLEL)
        .filter_map(|found| async move { found })
        .collect()
        .await;
    if !ids.is_empty() && api.is_empty() && gamesdb.is_empty() {
        return Err(network());
    }
    Ok(merge_owned(&ids, &api, &gamesdb))
}

/// One product from api.gog.com (Linux installer versions and fallbacks).
pub async fn product_json(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: Option<&SecretString>,
    id: &str,
    expand: &str,
) -> Result<Option<Value>, StoreError> {
    let mut url = join(&ep.api, &format!("products/{}", id.trim()))?;
    if !expand.is_empty() {
        url.query_pairs_mut().append_pair("expand", expand);
    }
    get_json(http, url, bearer).await
}

/// `downloads.installers[os=linux].version` of an api.gog.com product
/// (`?expand=downloads`), the English installer first.
pub fn parse_linux_installer_version(value: &Value) -> Option<String> {
    let installers = value
        .get("downloads")
        .and_then(|d| d.get("installers"))
        .and_then(Value::as_array)?;
    let linux: Vec<&Value> = installers
        .iter()
        .filter(|i| i.get("os").and_then(Value::as_str) == Some("linux"))
        .collect();
    linux
        .iter()
        .find(|i| i.get("language").and_then(Value::as_str) == Some("en"))
        .or_else(|| linux.first())
        .map(|i| text(i.get("version")))
        .filter(|v| !v.is_empty())
}

// --- builds -----------------------------------------------------------------

/// One content-system build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Build {
    pub build_id: String,
    pub branch: Option<String>,
    pub version_name: String,
    pub generation: u32,
    pub link: String,
}

/// `products/{id}/os/{os}/builds?generation=2`'s items.
pub fn parse_builds(value: &Value) -> Vec<Build> {
    value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| Build {
                    build_id: text(item.get("build_id")),
                    branch: item
                        .get("branch")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .filter(|b| !b.is_empty()),
                    version_name: text(item.get("version_name")),
                    generation: item.get("generation").and_then(Value::as_u64).unwrap_or(2) as u32,
                    link: text(item.get("link")),
                })
                .filter(|b| !b.build_id.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The build gogdl installs by default: the first with no branch, else
/// the first.
pub fn default_build(builds: &[Build]) -> Option<&Build> {
    builds
        .iter()
        .find(|b| b.branch.is_none())
        .or_else(|| builds.first())
}

/// The content-system builds of `id` for `os` (`windows` | `osx`).
pub async fn builds(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: Option<&SecretString>,
    id: &str,
    os: &str,
) -> Result<Vec<Build>, StoreError> {
    let mut url = join(
        &ep.content,
        &format!("products/{}/os/{os}/builds", id.trim()),
    )?;
    url.query_pairs_mut().append_pair("generation", "2");
    Ok(get_json(http, url, bearer)
        .await?
        .map(|v| parse_builds(&v))
        .unwrap_or_default())
}

/// A build's meta (`link`, zlib-compressed JSON) → its game client id and
/// secret. `None` when it holds neither.
pub fn parse_build_meta(bytes: &[u8]) -> Option<(String, SecretString)> {
    let mut text_bytes = Vec::new();
    let plain = if flate2::read::ZlibDecoder::new(bytes)
        .read_to_end(&mut text_bytes)
        .is_ok()
    {
        text_bytes
    } else {
        bytes.to_vec()
    };
    let value: Value = serde_json::from_slice(&plain).ok()?;
    let id = text(value.get("clientId"));
    let secret = text(value.get("clientSecret"));
    (!id.is_empty() && !secret.is_empty()).then(|| (id, SecretString::from(secret)))
}

/// The game client id and secret from the default Windows build's meta.
pub async fn game_client(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: &SecretString,
    id: &str,
) -> Result<Option<(String, SecretString)>, StoreError> {
    let list = builds(http, ep, Some(bearer), id, "windows").await?;
    let Some(build) = default_build(&list) else {
        return Ok(None);
    };
    let Ok(link) = Url::parse(&build.link) else {
        return Ok(None);
    };
    let bytes = get_bytes(http, link, Some(bearer)).await?;
    Ok(parse_build_meta(&bytes))
}

// --- cloud saves ------------------------------------------------------------

/// Galaxy remote config: whether cloud saves are on for the client, and
/// its save locations, for `Windows` or `MacOS`.
pub fn parse_remote_config(value: &Value, os: &str) -> Option<(bool, Vec<SaveLocation>)> {
    let storage = value
        .get("content")
        .and_then(|c| c.get(os))
        .and_then(|o| o.get("cloudStorage"))?;
    let enabled = storage
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let locations = storage
        .get("locations")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| SaveLocation {
                    name: text(item.get("name")),
                    path: text(item.get("location")),
                })
                .filter(|l| !l.name.is_empty() && !l.path.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some((enabled, locations))
}

/// The save locations remote-config gives `client_id` on Windows; empty
/// when cloud saves are off for it or it has none.
pub async fn save_locations(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    client_id: &str,
) -> Result<Vec<SaveLocation>, StoreError> {
    let mut url = join(
        &ep.remote_config,
        &format!("components/galaxy_client/clients/{}", client_id.trim()),
    )?;
    url.query_pairs_mut()
        .append_pair("component_version", REMOTE_CONFIG_VERSION);
    let Some(value) = get_json(http, url, None).await? else {
        return Ok(Vec::new());
    };
    Ok(match parse_remote_config(&value, "Windows") {
        Some((true, locations)) => locations,
        _ => Vec::new(),
    })
}

/// One file in cloudstorage.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudFile {
    /// `<location name>/<relative path>`.
    pub name: String,
    pub hash: String,
    /// Epoch seconds.
    pub modified: i64,
}

/// The cloudstorage listing (`[{name, hash, last_modified, ...}]`),
/// deleted-file markers dropped.
pub fn parse_cloud_listing(value: &Value) -> Vec<CloudFile> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| CloudFile {
                    name: text(item.get("name")),
                    hash: text(item.get("hash")),
                    modified: parse_time(&text(item.get("last_modified"))),
                })
                .filter(|f| !f.name.is_empty() && f.hash != EMPTY_CLOUD_MD5)
                .collect()
        })
        .unwrap_or_default()
}

/// An RFC 3339 / ISO 8601 time → epoch seconds, `0` when unreadable.
fn parse_time(value: &str) -> i64 {
    let value = value.trim();
    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(value) {
        return time.timestamp();
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%dT%H:%M:%S%z"] {
        if let Ok(time) = chrono::DateTime::parse_from_str(value, format) {
            return time.timestamp();
        }
    }
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .map(|t| t.and_utc().timestamp())
        .unwrap_or(0)
}

/// The files cloudstorage holds for `client_id` (game-scope `bearer`).
pub async fn cloud_files(
    http: &reqwest::Client,
    ep: &GogEndpoints,
    bearer: &SecretString,
    user_id: &str,
    client_id: &str,
) -> Result<Vec<CloudFile>, StoreError> {
    let url = join(
        &ep.cloud,
        &format!("v1/{}/{}", user_id.trim(), client_id.trim()),
    )?;
    Ok(get_json(http, url, Some(bearer))
        .await?
        .map(|v| parse_cloud_listing(&v))
        .unwrap_or_default())
}

/// The cloud files grouped by location name: `(newest time, count)`.
pub fn cloud_by_location(files: &[CloudFile]) -> BTreeMap<String, (i64, u32)> {
    let mut out: BTreeMap<String, (i64, u32)> = BTreeMap::new();
    for file in files {
        let location = file.name.split('/').next().unwrap_or("").to_string();
        let entry = out.entry(location).or_insert((0, 0));
        entry.0 = entry.0.max(file.modified);
        entry.1 += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_data_reads_the_name_and_id() {
        let account = parse_user_data(&json!({
            "isLoggedIn": true, "username": "fakeuser", "galaxyUserId": "4862", "userId": "1"
        }))
        .unwrap();
        assert_eq!(account.username, "fakeuser");
        assert_eq!(account.user_id, "4862");
        assert!(parse_user_data(&json!({"isLoggedIn": false, "username": "x"})).is_none());
        assert!(parse_user_data(&json!({})).is_none());
    }

    #[test]
    fn library_pages_and_owned_ids() {
        let (ids, next) = parse_library_page(&json!({
            "items": [
                {"platform_id": "gog", "external_id": "1207658924"},
                {"platform_id": "steam", "external_id": "620"},
                {"platform_id": "gog", "external_id": 42}
            ],
            "next_page_token": "abc"
        }));
        assert_eq!(ids, ["1207658924", "42"]);
        assert_eq!(next.as_deref(), Some("abc"));
        let (_, last) = parse_library_page(&json!({"items": [], "next_page_token": ""}));
        assert_eq!(last, None);
        assert_eq!(parse_owned_ids(&json!({"owned": [1, "2"]})), ["1", "2"]);
    }

    #[test]
    fn an_api_product_parses() {
        let product = parse_api_product(&json!({
            "id": 1207658924, "title": "The Witcher", "game_type": "game",
            "release_date": "2008-09-16T00:00:00+0300",
            "images": {"logo2x": "//images-1.gog-statics.com/abc_glx_logo_2x.jpg"},
            "content_system_compatibility": {"windows": true, "osx": false, "linux": false},
            "description": {"lead": "Geralt.", "full": "<p>Long</p>"},
            "screenshots": [{"formatter_template_url": "https://images.gog-statics.com/s1_{formatter}.jpg"}],
            "expanded_dlcs": [{"id": 9, "title": "Bonus"}],
            "dlcs": {"products": [{"id": 9}, {"id": 10}]}
        }))
        .unwrap();
        assert_eq!(product.id, "1207658924");
        assert!(!product.is_dlc);
        assert_eq!(
            product.cover,
            "https://images-1.gog-statics.com/abc_glx_logo_2x.jpg"
        );
        assert_eq!(product.release_date, "2008-09-16");
        assert_eq!(product.platforms, [GogPlatform::Windows]);
        assert_eq!(product.description, "Geralt.");
        assert_eq!(product.dlc.len(), 2);
        assert_eq!(product.dlc[0].title, "Bonus");
        assert!(product.screenshots[0].starts_with("https://images.gog-statics.com/s1_"));
        assert!(!product.screenshots[0].contains("{formatter}"));
        assert_eq!(
            parse_api_products(&json!([{"id": 1}, {"title": "no id"}])).len(),
            1
        );
    }

    #[test]
    fn a_gamesdb_release_parses() {
        let release = parse_gamesdb(&json!({
            "title": {"*": "The Witcher: Enhanced Edition"},
            "type": "game",
            "dlcs_ids": ["9"],
            "supported_operating_systems": [{"slug": "windows"}, {"slug": "osx"}, {"slug": "linux"}],
            "game": {
                "vertical_cover": {"url_format": "https://images.gog.com/v{formatter}.{ext}"},
                "screenshots": [{"url_format": "https://images.gog.com/s{formatter}.{ext}"}],
                "summary": {"*": "Geralt of Rivia."},
                "developers": [{"name": "CD PROJEKT RED"}],
                "publishers": [{"name": "CD PROJEKT"}],
                "genres": [{"name": {"*": "Role-playing", "en-US": "RPG"}}],
                "first_release_date": "2008-09-16T00:00:00+0000"
            }
        }))
        .unwrap();
        assert_eq!(release.title, "The Witcher: Enhanced Edition");
        assert_eq!(release.is_dlc, Some(false));
        assert_eq!(release.cover, "https://images.gog.com/v.jpg");
        assert_eq!(release.screenshots, ["https://images.gog.com/s.jpg"]);
        assert_eq!(release.developer, "CD PROJEKT RED");
        assert_eq!(release.publisher, "CD PROJEKT");
        assert_eq!(release.genres, ["Role-playing"]);
        assert_eq!(release.release_date, "2008-09-16");
        assert_eq!(
            release.platforms,
            [GogPlatform::Windows, GogPlatform::Linux]
        );
        assert_eq!(release.dlc_ids, ["9"]);
    }

    #[test]
    fn owned_merges_and_folds_dlc() {
        let ids: Vec<String> = ["2", "9", "1", "77"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut api = HashMap::new();
        api.insert(
            "1".to_string(),
            ApiProduct {
                id: "1".into(),
                title: "Zeta".into(),
                dlc: vec![
                    GogProductDlc {
                        id: "9".into(),
                        title: "Bonus".into(),
                    },
                    GogProductDlc {
                        id: "10".into(),
                        title: "Not owned".into(),
                    },
                ],
                cover: "api-cover".into(),
                ..Default::default()
            },
        );
        api.insert(
            "9".to_string(),
            ApiProduct {
                id: "9".into(),
                title: "Bonus Pack".into(),
                is_dlc: true,
                ..Default::default()
            },
        );
        api.insert(
            "77".to_string(),
            ApiProduct {
                id: "77".into(),
                title: "Orphan DLC".into(),
                is_dlc: true,
                ..Default::default()
            },
        );
        let mut db = HashMap::new();
        db.insert(
            "2".to_string(),
            GamesDbRelease {
                title: "alpha".into(),
                cover: "db-cover".into(),
                ..Default::default()
            },
        );
        db.insert(
            "1".to_string(),
            GamesDbRelease {
                title: "Zeta GOTY".into(),
                developer: "Dev".into(),
                ..Default::default()
            },
        );
        let products = merge_owned(&ids, &api, &db);
        assert_eq!(products.len(), 2, "DLC are folded, orphans dropped");
        assert_eq!(products[0].id, "2");
        assert_eq!(products[0].cover, "db-cover");
        assert_eq!(products[1].title, "Zeta GOTY");
        assert_eq!(products[1].cover, "api-cover", "api fills the gap");
        assert_eq!(products[1].developer, "Dev");
        assert_eq!(
            products[1].dlc,
            [GogProductDlc {
                id: "9".into(),
                title: "Bonus Pack".into()
            }]
        );
    }

    #[test]
    fn builds_and_the_default() {
        let list = parse_builds(&json!({"total_count": 3, "items": [
            {"build_id": "3", "branch": "beta", "version_name": "2.0b", "generation": 2, "link": "https://x/3"},
            {"build_id": "2", "branch": null, "version_name": "1.5", "generation": 2, "link": "https://x/2"},
            {"build_id": "1", "branch": null, "version_name": "1.0", "generation": 1, "link": "https://x/1"}
        ]}));
        assert_eq!(list.len(), 3);
        let build = default_build(&list).unwrap();
        assert_eq!(build.build_id, "2");
        assert_eq!(build.version_name, "1.5");
        assert!(default_build(&[]).is_none());
    }

    #[test]
    fn linux_installer_version() {
        let value = json!({"downloads": {"installers": [
            {"os": "windows", "language": "en", "version": "1.5"},
            {"os": "linux", "language": "de", "version": "2.1 (de)"},
            {"os": "linux", "language": "en", "version": "2.1"}
        ]}});
        assert_eq!(
            parse_linux_installer_version(&value).as_deref(),
            Some("2.1")
        );
        assert_eq!(parse_linux_installer_version(&json!({})), None);
    }

    #[test]
    fn build_meta_gives_the_game_client() {
        use std::io::Write as _;
        let meta = br#"{"clientId": "5051", "clientSecret": "fake-client-secret", "buildId": "2"}"#;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(meta).unwrap();
        let (id, secret) = parse_build_meta(&enc.finish().unwrap()).unwrap();
        assert_eq!(id, "5051");
        assert_eq!(secret.expose_secret(), "fake-client-secret");
        assert!(
            parse_build_meta(meta).is_some(),
            "plain JSON is accepted too"
        );
        assert!(parse_build_meta(b"{}").is_none());
    }

    #[test]
    fn remote_config_and_cloud_listing() {
        let config = json!({"content": {"Windows": {"cloudStorage": {"enabled": true, "locations": [
            {"name": "saves", "location": "<?SAVED_GAMES?>/Witcher"}
        ]}}, "MacOS": {"cloudStorage": {"enabled": false, "locations": []}}}});
        let (enabled, locations) = parse_remote_config(&config, "Windows").unwrap();
        assert!(enabled);
        assert_eq!(locations[0].path, "<?SAVED_GAMES?>/Witcher");
        assert!(!parse_remote_config(&config, "MacOS").unwrap().0);
        assert!(parse_remote_config(&json!({}), "Windows").is_none());

        let files = parse_cloud_listing(&json!([
            {"name": "saves/slot1.sav", "hash": "abc", "last_modified": "2024-01-02T03:04:05+00:00"},
            {"name": "saves/old.sav", "hash": EMPTY_CLOUD_MD5, "last_modified": "2024-01-01T00:00:00+00:00"},
            {"name": "config/a.ini", "hash": "def", "last_modified": "2023-01-01T00:00:00.000000+00:00"}
        ]));
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].modified, 1_704_164_645);
        let by = cloud_by_location(&files);
        assert_eq!(by["saves"], (1_704_164_645, 1));
        assert_eq!(by["config"].1, 1);
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(date_only("2008-09-16T00:00:00+0300"), "2008-09-16");
        assert_eq!(date_only("soon"), "");
        assert_eq!(parse_time("2024-01-02T03:04:05Z"), 1_704_164_645);
        assert_eq!(parse_time("nope"), 0);
    }
}
