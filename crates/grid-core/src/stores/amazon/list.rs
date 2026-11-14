//! nile's own files and short answers: the owned list (`library.json`,
//! after `library sync`), the installed list (`installed.json`), the
//! download size (`install --info --json`) and the update list
//! (`list-updates --json`).
//!
//! GRID reads `library.json` itself, as Heroic does: `library list
//! --json` crashes when the file is missing (spike 1). It is the only
//! source of the owned list (no GRID copy). Every parser ignores unknown
//! keys and nulls, and a JSON answer on stdout is the first JSON value
//! there (the e2e fake prints other lines and `fake_session` keys too).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{AmazonGame, AmazonInstalled};
use crate::stores::paths;

/// nile's owned list in its config folder.
pub const LIBRARY_FILE: &str = "library.json";
/// nile's installed list in its config folder.
pub const INSTALLED_FILE: &str = "installed.json";
/// nile's folder of installed manifests (`<product id>.raw`).
pub const MANIFESTS_DIR: &str = "manifests";

pub(crate) use crate::stores::epic::list::json_value;

/// `library sync`: refreshes the login when it has expired, then the
/// owned list into `library.json`.
pub fn sync_args() -> Vec<String> {
    vec!["library".into(), "sync".into()]
}

/// `install --info --json <id>`: `{"download_size": <bytes>}`.
pub fn info_args(id: &str) -> Vec<String> {
    vec![
        "install".into(),
        "--info".into(),
        "--json".into(),
        id.trim().to_string(),
    ]
}

/// `list-updates --json`: a JSON array of product ids.
pub fn updates_args() -> Vec<String> {
    vec!["list-updates".into(), "--json".into()]
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn texts(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| text(Some(v)))
        .filter(|s| !s.is_empty())
        .collect()
}

/// The year a `releaseDate` starts with, `""` when it starts with none.
fn year_of(date: &str) -> String {
    let year = date.trim().get(..4).unwrap_or_default();
    match year.parse::<u32>() {
        Ok(n) if year.chars().all(|c| c.is_ascii_digit()) && (1970..=2200).contains(&n) => {
            year.to_string()
        }
        _ => String::new(),
    }
}

/// One `library.json` entry as GRID's [`AmazonGame`], `None` when it names
/// no product id.
fn game_of(entry: &Value) -> Option<AmazonGame> {
    let product = entry.get("product")?;
    let product_id = text(product.get("id"));
    if product_id.is_empty() {
        return None;
    }
    let detail = product.get("productDetail");
    let details = detail.and_then(|d| d.get("details"));
    let field = |key: &str| text(details.and_then(|d| d.get(key)));
    let title = text(product.get("title"));
    let cover = [
        text(detail.and_then(|d| d.get("iconUrl"))),
        field("backgroundUrl2"),
        field("backgroundUrl1"),
    ]
    .into_iter()
    .find(|url| !url.is_empty())
    .unwrap_or_default();
    let mut screenshots = texts(details.and_then(|d| d.get("screenshots")));
    if screenshots.is_empty() {
        let hero = field("backgroundUrl1");
        if !hero.is_empty() {
            screenshots.push(hero);
        }
    }
    Some(AmazonGame {
        title: if title.is_empty() {
            product_id.clone()
        } else {
            title
        },
        product_id,
        entitlement_id: text(entry.get("id")),
        cover,
        screenshots,
        description: field("shortDescription"),
        developer: field("developer"),
        year: year_of(&field("releaseDate")),
        genres: texts(details.and_then(|d| d.get("genres"))),
    })
}

/// The owned games in a `library.json` text, once each: by product id,
/// then by ASIN. `None` when the text is not a JSON list.
pub fn parse_library(text_in: &str) -> Option<Vec<AmazonGame>> {
    let value: Value = serde_json::from_str(text_in.trim_start_matches('\u{feff}')).ok()?;
    let entries = value.as_array()?;
    let mut games: Vec<AmazonGame> = Vec::new();
    let mut asins: Vec<String> = Vec::new();
    for entry in entries {
        let Some(game) = game_of(entry) else { continue };
        let asin = text(entry.get("product").and_then(|p| p.get("asin")));
        if games.iter().any(|g| g.product_id == game.product_id)
            || (!asin.is_empty() && asins.contains(&asin))
        {
            continue;
        }
        if !asin.is_empty() {
            asins.push(asin);
        }
        games.push(game);
    }
    Some(games)
}

/// The owned list in nile's config folder, `None` when `library.json` is
/// missing or unreadable.
pub fn read_library(config_dir: &Path) -> Option<Vec<AmazonGame>> {
    parse_library(&fs::read_to_string(config_dir.join(LIBRARY_FILE)).ok()?)
}

/// The rows of an `installed.json` text (entries with no id are dropped),
/// `None` when it is not a JSON list.
pub fn parse_installed(text_in: &str) -> Option<Vec<AmazonInstalled>> {
    let value: Value = serde_json::from_str(text_in.trim_start_matches('\u{feff}')).ok()?;
    let rows = value
        .as_array()?
        .iter()
        .filter_map(|row| serde_json::from_value::<AmazonInstalled>(row.clone()).ok())
        .filter(|row| !row.id.trim().is_empty())
        .collect();
    Some(rows)
}

/// What nile has installed, empty when `installed.json` is missing or
/// unreadable.
pub fn read_installed(config_dir: &Path) -> Vec<AmazonInstalled> {
    fs::read_to_string(config_dir.join(INSTALLED_FILE))
        .ok()
        .and_then(|text| parse_installed(&text))
        .unwrap_or_default()
}

/// The `installed.json` entry for `id`.
pub fn installed_entry(config_dir: &Path, id: &str) -> Option<AmazonInstalled> {
    read_installed(config_dir)
        .into_iter()
        .find(|row| row.id.trim() == id.trim())
}

/// The manifest nile keeps for an installed `id`.
pub fn manifest_path(config_dir: &Path, id: &str) -> PathBuf {
    config_dir
        .join(MANIFESTS_DIR)
        .join(format!("{}.raw", id.trim()))
}

/// Drops `id` from `installed.json` (other entries kept as they are) and
/// deletes its manifest, as nile's own uninstall does. Used when nile
/// could not uninstall (missing manifest, missing nile): a manifest left
/// behind would make a reinstall download only the files that changed.
/// The file holds no token.
pub fn forget_installed(config_dir: &Path, id: &str) -> io::Result<()> {
    let path = config_dir.join(INSTALLED_FILE);
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(Value::Array(rows)) = serde_json::from_str::<Value>(&text) {
            let kept: Vec<Value> = rows
                .iter()
                .filter(|row| text_of_id(row) != id.trim())
                .cloned()
                .collect();
            if kept.len() != rows.len() {
                let json = serde_json::to_string(&kept).map_err(io::Error::other)?;
                fs::write(&path, json)?;
                paths::make_private_file(&path)?;
            }
        }
    }
    match fs::remove_file(manifest_path(config_dir, id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn text_of_id(row: &Value) -> String {
    text(row.get("id"))
}

/// The download size in `install --info --json`'s stdout.
pub fn parse_info(stdout: &str) -> Option<u64> {
    json_value(stdout)?.get("download_size")?.as_u64()
}

/// The product ids in `list-updates --json`'s stdout.
pub fn parse_updates(stdout: &str) -> Option<Vec<String>> {
    let value = json_value(stdout)?;
    let ids = value
        .as_array()?
        .iter()
        .map(|v| text(Some(v)))
        .filter(|id| !id.is_empty())
        .collect();
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

    /// A recorded-shape `library.json`: two entitlements of one product
    /// (deduped), an ASIN duplicate, a product with no title and nulls.
    const LIBRARY: &str = r#"[
      {"id": "ent-1", "fake_session": "FAKE-STORE-TOKEN-not-real",
       "product": {"id": "amzn1.adg.product.aaaa", "sku": "SKU1", "asin": "B000A", "title": "Fogline",
         "productDetail": {"iconUrl": "https://m.media-amazon.com/images/I/icon.png",
           "details": {"shortDescription": "Fog and lines.", "developer": "Dune Owl", "publisher": "Owl",
             "genres": ["Adventure", null, "Puzzle"], "releaseDate": "2021-03-04T00:00:00Z",
             "backgroundUrl1": "https://m.media-amazon.com/images/I/bg1.jpg",
             "backgroundUrl2": "https://m.media-amazon.com/images/I/bg2.jpg",
             "screenshots": ["https://m.media-amazon.com/images/I/s1.jpg"], "videos": []}}}},
      {"id": "ent-1b", "product": {"id": "amzn1.adg.product.aaaa", "asin": "B000A", "title": "Fogline (again)"}},
      {"id": "ent-2", "product": {"id": "amzn1.adg.product.bbbb", "asin": "B000A", "title": "Same ASIN"}},
      {"id": "ent-3", "product": {"id": "amzn1.adg.product.cccc", "asin": null, "title": null,
         "productDetail": {"iconUrl": null, "details": {"backgroundUrl1": "https://m.media-amazon.com/b1.jpg",
           "releaseDate": "soon", "genres": null}}}},
      {"id": "ent-4", "product": {"title": "No id"}},
      {"id": "ent-5", "product": {"id": "amzn1.adg.product.dddd", "asin": "B000D", "title": "Lowsun",
         "productDetail": {"details": {"backgroundUrl2": "https://m.media-amazon.com/b2.jpg", "releaseDate": "1999"}}}}
    ]"#;

    #[test]
    fn the_owned_list_dedups_and_falls_back() {
        let games = parse_library(LIBRARY).unwrap();
        let ids: Vec<&str> = games.iter().map(|g| g.product_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "amzn1.adg.product.aaaa",
                "amzn1.adg.product.cccc",
                "amzn1.adg.product.dddd"
            ]
        );
        let fog = &games[0];
        assert_eq!(fog.title, "Fogline");
        assert_eq!(fog.entitlement_id, "ent-1");
        assert_eq!(fog.cover, "https://m.media-amazon.com/images/I/icon.png");
        assert_eq!(
            fog.screenshots,
            ["https://m.media-amazon.com/images/I/s1.jpg"]
        );
        assert_eq!(fog.description, "Fog and lines.");
        assert_eq!(fog.developer, "Dune Owl");
        assert_eq!(fog.year, "2021");
        assert_eq!(fog.genres, ["Adventure", "Puzzle"]);
        let untitled = &games[1];
        assert_eq!(untitled.title, "amzn1.adg.product.cccc");
        assert_eq!(untitled.cover, "https://m.media-amazon.com/b1.jpg");
        assert_eq!(untitled.screenshots, ["https://m.media-amazon.com/b1.jpg"]);
        assert_eq!(untitled.year, "");
        assert_eq!(games[2].cover, "https://m.media-amazon.com/b2.jpg");
        assert_eq!(games[2].year, "1999");
        assert!(games[2].screenshots.is_empty());
        for game in &games {
            assert!(!serde_json::to_string(game).unwrap().contains(FAKE));
        }
        assert_eq!(parse_library("{}"), None);
        assert_eq!(parse_library("not json"), None);
        assert_eq!(parse_library("[]"), Some(Vec::new()));
    }

    #[test]
    fn installed_json_and_forgetting_a_game() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path();
        assert!(read_installed(cfg).is_empty());
        let json = r#"[{"id": "amzn1.adg.product.aaaa", "version": "v1", "path": "/g/Fogline", "size": 10},
                       {"id": null, "version": "x"},
                       {"id": "amzn1.adg.product.bbbb", "version": "v2", "path": "/g/K", "size": 5, "extra": 1}]"#;
        fs::write(cfg.join(INSTALLED_FILE), json).unwrap();
        let rows = read_installed(cfg);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            installed_entry(cfg, " amzn1.adg.product.bbbb ")
                .unwrap()
                .size,
            5
        );
        fs::create_dir_all(cfg.join(MANIFESTS_DIR)).unwrap();
        fs::write(manifest_path(cfg, "amzn1.adg.product.aaaa"), b"pb").unwrap();
        forget_installed(cfg, "amzn1.adg.product.aaaa").unwrap();
        assert!(!manifest_path(cfg, "amzn1.adg.product.aaaa").exists());
        let left = read_installed(cfg);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "amzn1.adg.product.bbbb");
        let text = fs::read_to_string(cfg.join(INSTALLED_FILE)).unwrap();
        assert!(
            text.contains("\"extra\":1"),
            "other entries kept as is: {text}"
        );
        forget_installed(cfg, "amzn1.adg.product.zzzz").unwrap();
        forget_installed(&cfg.join("none"), "x").unwrap();
    }

    #[test]
    fn short_answers_parse_amid_other_lines() {
        assert_eq!(
            info_args(" amzn1.adg.product.aaaa "),
            ["install", "--info", "--json", "amzn1.adg.product.aaaa"]
        );
        assert_eq!(sync_args(), ["library", "sync"]);
        assert_eq!(updates_args(), ["list-updates", "--json"]);
        let info =
            format!("{FAKE}\n{{\"download_size\": 7000000000, \"fake_session\": \"{FAKE}\"}}\n");
        assert_eq!(parse_info(&info), Some(7_000_000_000));
        assert_eq!(parse_info("Traceback (most recent call last):"), None);
        let updates = format!("{FAKE}\n[\"amzn1.adg.product.aaaa\", 3, \"\"]\n");
        assert_eq!(
            parse_updates(&updates).unwrap(),
            ["amzn1.adg.product.aaaa", "3"]
        );
        assert_eq!(parse_updates("[]").unwrap(), Vec::<String>::new());
        assert_eq!(parse_updates("{}"), None);
        assert_eq!(parse_updates(""), None);
    }
}
