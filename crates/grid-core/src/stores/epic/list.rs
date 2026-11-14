//! legendary's JSON answers, parsed: the owned list (`list --json -T`),
//! the installed list (`list-installed --json`), one app's sizes
//! (`info <app> --json`), and the owned-list cache GRID keeps between runs
//! (`<data_dir>/stores/cache/epic-owned.json`, product metadata only).
//!
//! Every parser takes the text [`ToolExit::read_stdout`] hands it and
//! nothing else sees it; none of these shapes carries a token, but the
//! stdout is never logged either way.
//!
//! [`ToolExit::read_stdout`]: crate::stores::runner::ToolExit::read_stdout

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{EpicGame, EpicGameDlc, EpicInstalled};
use crate::stores::paths::{self, STORES_DIR};

// --- argv -------------------------------------------------------------------

/// `list --json -T`: every owned game with its DLC nested, third-party
/// titles (`-T`) included so GRID can show them as Not supported.
pub fn list_args() -> Vec<String> {
    vec!["list".into(), "--json".into(), "-T".into()]
}

/// `list-installed --json` (offline).
pub fn installed_args() -> Vec<String> {
    vec!["list-installed".into(), "--json".into()]
}

/// `info <app> --json`.
pub fn info_args(app: &str) -> Vec<String> {
    vec!["info".into(), app.trim().to_string(), "--json".into()]
}

// --- shared JSON helpers ----------------------------------------------------

/// The JSON object or list in a tool's stdout: the first line that starts
/// one and parses (to the end of the value, which may span lines). Other
/// lines before or after it (a prompt, a notice) are skipped.
pub(crate) fn json_value(stdout: &str) -> Option<Value> {
    let mut offset = 0;
    for line in stdout.split_inclusive('\n') {
        let start = offset + (line.len() - line.trim_start().len());
        offset += line.len();
        let rest = &stdout[start..];
        if !(rest.starts_with('{') || rest.starts_with('[')) {
            continue;
        }
        let parsed = serde_json::Deserializer::from_str(rest)
            .into_iter::<Value>()
            .next();
        if let Some(Ok(value)) = parsed {
            if value.is_object() || value.is_array() {
                return Some(value);
            }
        }
    }
    None
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// A `customAttributes` entry's value (`{"type": "STRING", "value": …}`).
fn attribute(metadata: &Value, key: &str) -> String {
    text(
        metadata
            .get("customAttributes")
            .and_then(|attrs| attrs.get(key))
            .and_then(|attr| attr.get("value")),
    )
}

// --- owned list -------------------------------------------------------------

/// The cover types GRID tries, best first: a portrait box art (the
/// Library's cards are portrait), then the wide ones.
const COVER_TYPES: [&str; 5] = [
    "DieselGameBoxTall",
    "OfferImageTall",
    "DieselStoreFrontTall",
    "DieselGameBox",
    "OfferImageWide",
];

/// The screenshot types, then the wide art used when a game has none.
const SCREENSHOT_TYPES: [&str; 2] = ["Screenshot", "GalleryImage"];
const WIDE_TYPES: [&str; 3] = ["DieselGameBox", "OfferImageWide", "DieselStoreFrontWide"];

/// The categories of an item that is not a game (a mod or an app, which
/// legendary also lists).
const NOT_GAME_CATEGORIES: [&str; 2] = ["mods", "applications"];

/// The `(type, url)` pairs of an item's keyImages that GRID may fetch:
/// `http(s)` only (Epic's video entries use `com.epicgames.video://`).
fn key_images(metadata: &Value) -> Vec<(String, String)> {
    metadata
        .get("keyImages")
        .and_then(Value::as_array)
        .map(|images| {
            images
                .iter()
                .filter_map(|image| {
                    let kind = text(image.get("type"));
                    let url = text(image.get("url"));
                    let lower = url.to_ascii_lowercase();
                    (lower.starts_with("https://") || lower.starts_with("http://"))
                        .then_some((kind, url))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn cover_of(images: &[(String, String)]) -> String {
    COVER_TYPES
        .iter()
        .find_map(|want| images.iter().find(|(kind, _)| kind == want))
        .map(|(_, url)| url.clone())
        .unwrap_or_default()
}

fn screenshots_of(images: &[(String, String)]) -> Vec<String> {
    let mut shots: Vec<String> = Vec::new();
    for (kind, url) in images {
        if SCREENSHOT_TYPES.contains(&kind.as_str()) && !shots.contains(url) {
            shots.push(url.clone());
        }
    }
    if shots.is_empty() {
        if let Some((_, url)) = WIDE_TYPES
            .iter()
            .find_map(|want| images.iter().find(|(kind, _)| kind == want))
        {
            shots.push(url.clone());
        }
    }
    shots
}

/// The launcher a third-party-managed title needs, in GRID's words: "the
/// EA app" for Origin / the EA app, "Ubisoft Connect" for Ubisoft, else
/// Epic's own value. `None` for a title legendary installs.
pub fn third_party_name(metadata: &Value) -> Option<String> {
    let raw = Some(attribute(metadata, "ThirdPartyManagedApp"))
        .filter(|v| !v.is_empty())
        .or_else(|| {
            Some(attribute(metadata, "ThirdPartyManagedProvider")).filter(|v| !v.is_empty())
        })?;
    let key: String = raw
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    Some(match key.as_str() {
        "origin" | "theeaapp" | "eaapp" | "ea" => "the EA app".to_string(),
        "ubisoftconnect" | "uplay" | "ubisoft" => "Ubisoft Connect".to_string(),
        _ => raw,
    })
}

/// The release year: the first `releaseInfo` date (`releaseDate`, else
/// `dateAdded`) that starts with a plausible year, `""` when none does.
fn year_of(metadata: &Value) -> String {
    let dates = metadata
        .get("releaseInfo")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|info| {
            ["releaseDate", "releasedDate", "dateAdded"]
                .into_iter()
                .map(|key| text(info.get(key)))
        })
        .chain(["releaseDate", "creationDate"].map(|key| text(metadata.get(key))));
    for date in dates {
        let year = date.get(..4).unwrap_or_default();
        if year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = year.parse::<u32>() {
                if (1970..=2200).contains(&n) {
                    return year.to_string();
                }
            }
        }
    }
    String::new()
}

/// Whether an item is a mod or an app rather than a game: every category
/// is `mods`/`applications`, or it is a mod with no `games` category.
fn not_a_game(metadata: &Value) -> bool {
    let paths: Vec<String> = metadata
        .get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|c| text(c.get("path")))
        .filter(|p| !p.is_empty())
        .collect();
    if paths.is_empty() {
        return false;
    }
    let only_other = paths
        .iter()
        .all(|p| NOT_GAME_CATEGORIES.contains(&p.as_str()));
    let is_mod = paths.iter().any(|p| p == "mods") && !paths.iter().any(|p| p == "games");
    only_other || is_mod
}

/// The Windows build version (`asset_infos.Windows`, else `Win32`), `None`
/// when the item has no Windows asset.
fn windows_build(item: &Value) -> Option<String> {
    let assets = item.get("asset_infos")?.as_object()?;
    ["Windows", "Win32"]
        .iter()
        .find_map(|platform| assets.get(*platform))
        .map(|asset| text(asset.get("build_version")))
}

fn title_of(item: &Value) -> String {
    let title = text(item.get("app_title"));
    if title.is_empty() {
        text(item.get("metadata").and_then(|m| m.get("title")))
    } else {
        title
    }
}

/// Parses `list --json -T`. Kept: items with a Windows asset, and
/// third-party titles (no asset; shown as Not supported). Dropped: mods
/// and apps, DLC listed on its own (it comes nested under its base), and
/// items with no app name. `None` when stdout is not a JSON list.
pub fn parse_owned(stdout: &str) -> Option<Vec<EpicGame>> {
    let value = json_value(stdout)?;
    let items = value.as_array()?;
    let mut games = Vec::new();
    for item in items {
        let app_name = text(item.get("app_name"));
        if app_name.is_empty() {
            continue;
        }
        let empty = Value::Null;
        let metadata = item.get("metadata").unwrap_or(&empty);
        if metadata.get("mainGameItem").is_some() || not_a_game(metadata) {
            continue;
        }
        let third_party = third_party_name(metadata);
        let build = windows_build(item);
        if build.is_none() && third_party.is_none() {
            continue;
        }
        let images = key_images(metadata);
        let dlc = item
            .get("dlcs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|dlc| windows_build(dlc).is_some())
            .filter_map(|dlc| {
                let app_name = text(dlc.get("app_name"));
                (!app_name.is_empty()).then(|| EpicGameDlc {
                    app_name,
                    title: title_of(dlc),
                })
            })
            .collect();
        games.push(EpicGame {
            title: title_of(item),
            cover: cover_of(&images),
            screenshots: screenshots_of(&images),
            description: text(metadata.get("description")),
            developer: text(metadata.get("developer")),
            year: year_of(metadata),
            build_version: build.unwrap_or_default(),
            third_party,
            cloud_save_folder: attribute(metadata, "CloudSaveFolder"),
            can_run_offline: attribute(metadata, "CanRunOffline").eq_ignore_ascii_case("true"),
            dlc,
            app_name,
        });
    }
    Some(games)
}

// --- installed list ---------------------------------------------------------

/// Parses `list-installed --json`. `None` when stdout is not a JSON list.
pub fn parse_installed(stdout: &str) -> Option<Vec<EpicInstalled>> {
    let value = json_value(stdout)?;
    if !value.is_array() {
        return None;
    }
    serde_json::from_value(value).ok()
}

// --- info -------------------------------------------------------------------

/// What `info <app> --json` says about one app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpicInfo {
    /// `manifest.download_size` (bytes), `0` when there is no manifest.
    pub download_bytes: u64,
    /// `manifest.disk_size` (bytes).
    pub disk_bytes: u64,
    /// `manifest.launch_exe`.
    pub launch_exe: String,
    /// `manifest.build_version`.
    pub build_version: String,
    /// `game.cloud_save_folder`.
    pub cloud_save_folder: String,
    /// `game.owned_dlc` entries that install (have an app name).
    pub dlc: Vec<EpicGameDlc>,
}

/// Parses `info --json` (`{"game": …, "install": …, "manifest": …}`, an
/// empty part being `null`). `None` when stdout is not that object or
/// legendary knows nothing about the app (`game` and `manifest` null).
pub fn parse_info(stdout: &str) -> Option<EpicInfo> {
    let value = json_value(stdout)?;
    let object = value.as_object()?;
    if !object.contains_key("game") && !object.contains_key("manifest") {
        return None;
    }
    let game = object.get("game").filter(|g| g.is_object());
    let manifest = object.get("manifest").filter(|m| m.is_object());
    if game.is_none() && manifest.is_none() {
        return None;
    }
    let number = |part: Option<&Value>, key: &str| {
        part.and_then(|p| p.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let dlc = game
        .and_then(|g| g.get("owned_dlc"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let app_name = text(d.get("app_name"));
            (!app_name.is_empty()).then(|| EpicGameDlc {
                app_name,
                title: text(d.get("title")),
            })
        })
        .collect();
    Some(EpicInfo {
        download_bytes: number(manifest, "download_size"),
        disk_bytes: number(manifest, "disk_size"),
        launch_exe: text(manifest.and_then(|m| m.get("launch_exe"))),
        build_version: text(manifest.and_then(|m| m.get("build_version"))),
        cloud_save_folder: text(game.and_then(|g| g.get("cloud_save_folder"))),
        dlc,
    })
}

// --- owned cache ------------------------------------------------------------

/// The owned-list cache file name under `<data_dir>/stores/cache/`.
pub const OWNED_CACHE_FILE: &str = "epic-owned.json";

/// The cached owned list: product metadata only, never a token.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OwnedCache {
    /// Epoch seconds of the refresh that wrote it.
    pub refreshed_at: i64,
    pub games: Vec<EpicGame>,
}

/// `<data_dir>/stores/cache/epic-owned.json`.
pub fn owned_cache_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(STORES_DIR)
        .join("cache")
        .join(OWNED_CACHE_FILE)
}

/// The cached owned list, `None` when there is none or it can't be read.
pub fn read_owned_cache(data_dir: &Path) -> Option<OwnedCache> {
    let text = fs::read_to_string(owned_cache_path(data_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes the owned list to the cache (through a temporary file).
pub fn write_owned_cache(data_dir: &Path, games: &[EpicGame], refreshed_at: i64) -> io::Result<()> {
    let path = owned_cache_path(data_dir);
    if let Some(dir) = path.parent() {
        paths::ensure_private_dir(dir)?;
    }
    let cache = OwnedCache {
        refreshed_at,
        games: games.to_vec(),
    };
    let json = serde_json::to_string(&cache).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)
}

/// Deletes the owned-list cache (Disconnect).
pub fn remove_owned_cache(data_dir: &Path) -> io::Result<()> {
    match fs::remove_file(owned_cache_path(data_dir)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recorded-shape `list --json -T` answer: a game with a DLC, a
    /// third-party title with no asset, a mod, a Mac-only item and a DLC
    /// listed on its own.
    const OWNED: &str = r#"[
      {"app_name": "Starlight", "app_title": "Starlight Courier",
       "asset_infos": {"Windows": {"app_name": "Starlight", "asset_id": "a1", "build_version": "1.0.3-CL-99",
                                   "catalog_item_id": "c1", "label_name": "Live-Windows", "namespace": "ns1",
                                   "metadata": {}, "sidecar_rev": 0}},
       "base_urls": [], "sidecar": null, "achievements": null,
       "metadata": {"id": "c1", "namespace": "ns1", "title": "Starlight Courier",
         "description": "Deliver mail across a dying star.", "developer": "Lanternfish",
         "categories": [{"path": "games"}, {"path": "applications"}],
         "keyImages": [
           {"type": "DieselGameBox", "url": "https://cdn1.epicgames.com/ns1/wide.jpg"},
           {"type": "Featured", "url": "com.epicgames.video://abc"},
           {"type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/ns1/tall.jpg"},
           {"type": "Screenshot", "url": "https://cdn2.unrealengine.com/s1.jpg"},
           {"type": "Screenshot", "url": "https://cdn2.unrealengine.com/s2.jpg"}],
         "customAttributes": {"CloudSaveFolder": {"type": "STRING", "value": "{AppData}/Starlight/Saved"},
                              "CanRunOffline": {"type": "STRING", "value": "true"},
                              "FolderName": {"type": "STRING", "value": "StarlightCourier"}},
         "releaseInfo": [{"id": "r1", "appId": "Starlight", "platform": ["Windows"], "dateAdded": "2021-06-15T16:00:00.000Z"}]},
       "dlcs": [
         {"app_name": "StarlightDLC1", "app_title": "Starlight Courier: Night Route",
          "asset_infos": {"Windows": {"build_version": "1.0"}},
          "metadata": {"mainGameItem": {"id": "c1"}, "title": "Night Route"}},
         {"app_name": "StarlightSoundtrack", "app_title": "Soundtrack",
          "asset_infos": {}, "metadata": {"mainGameItem": {"id": "c1"}}}]},
      {"app_name": "IronOrchard", "app_title": "Iron Orchard", "asset_infos": {},
       "metadata": {"title": "Iron Orchard", "categories": [{"path": "games"}],
         "customAttributes": {"ThirdPartyManagedApp": {"type": "STRING", "value": "Origin"}},
         "keyImages": [{"type": "OfferImageTall", "url": "https://cdn1.epicgames.com/io/tall.jpg"}]},
       "dlcs": []},
      {"app_name": "NorthernStatic", "app_title": "Northern Static",
       "asset_infos": {"Windows": {"build_version": "7"}},
       "metadata": {"customAttributes": {"ThirdPartyManagedApp": {"type": "STRING", "value": "UbisoftConnect"}}},
       "dlcs": []},
      {"app_name": "ModThing", "app_title": "A Mod", "asset_infos": {"Windows": {"build_version": "1"}},
       "metadata": {"categories": [{"path": "mods"}, {"path": "addons"}]}, "dlcs": []},
      {"app_name": "Editor", "app_title": "An App", "asset_infos": {"Windows": {"build_version": "1"}},
       "metadata": {"categories": [{"path": "applications"}]}, "dlcs": []},
      {"app_name": "MacOnly", "app_title": "Mac Only", "asset_infos": {"Mac": {"build_version": "1"}},
       "metadata": {}, "dlcs": []},
      {"app_name": "LooseDLC", "app_title": "Loose", "asset_infos": {"Windows": {"build_version": "1"}},
       "metadata": {"mainGameItem": {"id": "zz"}}, "dlcs": []},
      {"app_name": "", "app_title": "No name"}
    ]"#;

    #[test]
    fn the_owned_list_keeps_games_and_third_party_titles() {
        let games = parse_owned(&format!("[cli] INFO: noise\n{OWNED}")).unwrap();
        let names: Vec<&str> = games.iter().map(|g| g.app_name.as_str()).collect();
        assert_eq!(names, ["Starlight", "IronOrchard", "NorthernStatic"]);
        let s = &games[0];
        assert_eq!(s.title, "Starlight Courier");
        assert_eq!(s.cover, "https://cdn1.epicgames.com/ns1/tall.jpg");
        assert_eq!(
            s.screenshots,
            [
                "https://cdn2.unrealengine.com/s1.jpg",
                "https://cdn2.unrealengine.com/s2.jpg"
            ]
        );
        assert_eq!(s.description, "Deliver mail across a dying star.");
        assert_eq!(s.developer, "Lanternfish");
        assert_eq!(s.year, "2021");
        assert_eq!(s.build_version, "1.0.3-CL-99");
        assert_eq!(s.third_party, None);
        assert_eq!(s.cloud_save_folder, "{AppData}/Starlight/Saved");
        assert!(s.can_run_offline);
        assert_eq!(
            s.dlc,
            vec![EpicGameDlc {
                app_name: "StarlightDLC1".into(),
                title: "Starlight Courier: Night Route".into()
            }],
            "a DLC with no Windows asset is not installable"
        );
        assert_eq!(games[1].third_party.as_deref(), Some("the EA app"));
        assert_eq!(games[1].build_version, "");
        assert_eq!(games[1].cover, "https://cdn1.epicgames.com/io/tall.jpg");
        assert_eq!(games[2].third_party.as_deref(), Some("Ubisoft Connect"));
        assert!(games[2].screenshots.is_empty());
        assert!(!games[2].can_run_offline);
    }

    #[test]
    fn third_party_names_read_in_grids_words() {
        let meta = |key: &str, value: &str| serde_json::json!({"customAttributes": {key: {"type": "STRING", "value": value}}});
        assert_eq!(
            third_party_name(&meta("ThirdPartyManagedApp", "The EA App")).as_deref(),
            Some("the EA app")
        );
        assert_eq!(
            third_party_name(&meta("ThirdPartyManagedProvider", "Ubisoft Connect")).as_deref(),
            Some("Ubisoft Connect")
        );
        assert_eq!(
            third_party_name(&meta("ThirdPartyManagedApp", "Battle.net")).as_deref(),
            Some("Battle.net")
        );
        assert_eq!(third_party_name(&meta("FolderName", "x")), None);
        assert_eq!(third_party_name(&serde_json::json!({})), None);
    }

    #[test]
    fn wide_art_stands_in_when_there_is_no_tall_cover_or_screenshot() {
        let games = parse_owned(
            r#"[{"app_name": "A", "app_title": "A", "asset_infos": {"Windows": {"build_version": "1"}},
                 "metadata": {"keyImages": [{"type": "OfferImageWide", "url": "https://cdn1.epicgames.com/w.jpg"},
                                            {"type": "Thumbnail", "url": "https://cdn1.epicgames.com/t.jpg"}],
                              "releaseInfo": [{"dateAdded": "n/a"}]}}]"#,
        )
        .unwrap();
        assert_eq!(games[0].cover, "https://cdn1.epicgames.com/w.jpg");
        assert_eq!(games[0].screenshots, ["https://cdn1.epicgames.com/w.jpg"]);
        assert_eq!(games[0].year, "");
    }

    #[test]
    fn junk_is_not_an_owned_list() {
        assert_eq!(parse_owned(""), None);
        assert_eq!(parse_owned("{\"a\": 1}"), None);
        assert_eq!(parse_owned("[]"), Some(Vec::new()));
    }

    #[test]
    fn installed_list_parses_and_junk_does_not() {
        let rows = parse_installed(
            r#"[{"app_name": "Starlight", "title": "Starlight Courier", "version": "1.0.3-CL-99",
                 "install_path": "/g/Starlight", "install_size": 10, "executable": "S.exe",
                 "launch_parameters": null, "is_dlc": false, "can_run_offline": true, "save_path": null},
                {"app_name": "StarlightDLC1", "title": "Night Route", "version": "1.0", "install_path": "/g/Starlight",
                 "is_dlc": true}]"#,
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].version, "1.0.3-CL-99");
        assert!(rows[1].is_dlc);
        assert_eq!(parse_installed("[]"), Some(Vec::new()));
        assert_eq!(parse_installed("not json"), None);
        assert_eq!(parse_installed("{}"), None);
    }

    #[test]
    fn info_reads_sizes_cloud_folder_and_installable_dlc() {
        let info = parse_info(
            r#"{"game": {"app_name": "Starlight", "title": "Starlight Courier", "version": "1.0.3",
                         "cloud_saves_supported": true, "cloud_save_folder": "{AppData}/Starlight/Saved",
                         "is_dlc": false,
                         "owned_dlc": [{"app_name": "StarlightDLC1", "title": "Night Route", "installable": [{"appId": "StarlightDLC1"}], "id": "d1"},
                                       {"app_name": null, "title": "Outfit", "installable": null, "id": "d2"}]},
                "install": null,
                "manifest": {"size": 100, "type": "binary", "version": 21, "launch_exe": "Binaries/Win64/S.exe",
                             "build_version": "1.0.3", "disk_size": 24000000000, "download_size": 9000000000}}"#,
        )
        .unwrap();
        assert_eq!(info.download_bytes, 9_000_000_000);
        assert_eq!(info.disk_bytes, 24_000_000_000);
        assert_eq!(info.launch_exe, "Binaries/Win64/S.exe");
        assert_eq!(info.build_version, "1.0.3");
        assert_eq!(info.cloud_save_folder, "{AppData}/Starlight/Saved");
        assert_eq!(
            info.dlc,
            vec![EpicGameDlc {
                app_name: "StarlightDLC1".into(),
                title: "Night Route".into()
            }]
        );
        assert_eq!(
            parse_info(r#"{"game": null, "install": null, "manifest": null}"#),
            None,
            "an unknown app"
        );
        assert_eq!(parse_info("[]"), None);
    }

    #[test]
    fn the_owned_cache_round_trips_and_goes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_owned_cache(dir.path()), None);
        let games = parse_owned(OWNED).unwrap();
        write_owned_cache(dir.path(), &games, 1_700_000_000).unwrap();
        assert!(owned_cache_path(dir.path()).ends_with("stores/cache/epic-owned.json"));
        let cache = read_owned_cache(dir.path()).unwrap();
        assert_eq!(cache.refreshed_at, 1_700_000_000);
        assert_eq!(cache.games, games);
        remove_owned_cache(dir.path()).unwrap();
        remove_owned_cache(dir.path()).unwrap();
        assert_eq!(read_owned_cache(dir.path()), None);
    }

    #[test]
    fn argv_builders() {
        assert_eq!(list_args(), ["list", "--json", "-T"]);
        assert_eq!(installed_args(), ["list-installed", "--json"]);
        assert_eq!(info_args(" Starlight "), ["info", "Starlight", "--json"]);
    }
}
