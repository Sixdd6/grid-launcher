//! `goggame-<id>.info`: the JSON file every GOG install (gogdl or offline
//! installer) carries at its root. It names the product, its base game (for
//! a DLC), the build and the play tasks the launch columns come from.
//!
//! The file is untrusted input: anything that does not parse is `None`, and
//! a play task without a launchable path (a `URLTask`) is dropped.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// What GRID reads from one `goggame-*.info`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GogGameInfo {
    pub game_id: String,
    /// The base game's id. Equal to `game_id` for a base game (or blank in
    /// an old file); another product's id for a DLC.
    pub root_game_id: String,
    pub name: String,
    pub build_id: String,
    pub client_id: String,
    pub language: String,
    /// The launchable play tasks (`FileTask`s with a path), in file order.
    pub play_tasks: Vec<PlayTask>,
}

/// One launchable play task.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayTask {
    pub is_primary: bool,
    /// The executable, relative to the install folder, as the file writes
    /// it (either separator).
    pub path: String,
    pub arguments: String,
    /// Relative to the install folder; `""` for the executable's own.
    pub working_dir: String,
    /// `game`, `tool`, `document`… (`""` when the file has none).
    pub category: String,
}

impl GogGameInfo {
    /// Whether this is a DLC: its base game is another product.
    pub fn is_dlc(&self) -> bool {
        let root = self.root_game_id.trim();
        !root.is_empty() && root != self.game_id.trim()
    }

    /// The task the game launches with: the primary task, else the first
    /// `game` task, else `None`.
    pub fn primary_task(&self) -> Option<&PlayTask> {
        self.play_tasks
            .iter()
            .find(|task| task.is_primary)
            .or_else(|| {
                self.play_tasks
                    .iter()
                    .find(|task| task.category.eq_ignore_ascii_case("game"))
            })
    }
}

/// The largest `goggame-*.info` GRID reads.
const MAX_INFO_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInfo {
    #[serde(default, deserialize_with = "id_string")]
    game_id: String,
    #[serde(default, deserialize_with = "id_string")]
    root_game_id: String,
    #[serde(default)]
    name: String,
    #[serde(default, deserialize_with = "id_string")]
    build_id: String,
    #[serde(default, deserialize_with = "id_string")]
    client_id: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    play_tasks: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTask {
    #[serde(default)]
    is_primary: bool,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    arguments: String,
    #[serde(default)]
    working_dir: String,
    #[serde(default)]
    category: String,
}

/// An id GOG writes as a string or a number.
fn id_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(text) => text.trim().to_string(),
        serde_json::Value::Number(number) => number.to_string(),
        _ => String::new(),
    })
}

/// Parses one `goggame-*.info`. `None` when it is not JSON, is not an
/// object, or has no game id. A UTF-8 byte-order mark is allowed.
pub fn parse_info(bytes: &[u8]) -> Option<GogGameInfo> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let raw: RawInfo = serde_json::from_slice(bytes).ok()?;
    if raw.game_id.is_empty() {
        return None;
    }
    let play_tasks = raw
        .play_tasks
        .into_iter()
        .filter_map(|value| serde_json::from_value::<RawTask>(value).ok())
        .filter(|task| {
            (task.kind.is_empty() || task.kind.eq_ignore_ascii_case("FileTask"))
                && !task.path.trim().is_empty()
        })
        .map(|task| PlayTask {
            is_primary: task.is_primary,
            path: task.path.trim().to_string(),
            arguments: task.arguments,
            working_dir: task.working_dir.trim().to_string(),
            category: task.category,
        })
        .collect();
    Some(GogGameInfo {
        game_id: raw.game_id,
        root_game_id: raw.root_game_id,
        name: raw.name.trim().to_string(),
        build_id: raw.build_id,
        client_id: raw.client_id,
        language: raw.language,
        play_tasks,
    })
}

/// Every `goggame-*.info` file directly in `dir`, sorted by name. Empty
/// when `dir` cannot be read. Folders and files over 1 MiB are skipped.
pub fn find_info_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy().to_ascii_lowercase();
            name.starts_with("goggame-") && name.ends_with(".info")
        })
        .filter(|entry| {
            entry
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_INFO_BYTES)
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const WITCHER: &str = r#"{
        "buildId": "56398707826926290",
        "clientId": "50225266424144145",
        "gameId": "1207658924",
        "language": "English",
        "languages": ["en-US"],
        "name": "The Witcher: Enhanced Edition ",
        "playTasks": [
            {"category": "document", "isPrimary": false, "path": "manual.pdf", "type": "FileTask"},
            {"category": "game", "isPrimary": true, "name": "The Witcher", "path": "System\\witcher.exe",
             "arguments": "-nointro", "workingDir": "System", "type": "FileTask"},
            {"category": "tool", "link": "https://www.gog.com/forum", "type": "URLTask"}
        ],
        "rootGameId": "1207658924",
        "version": 1
    }"#;

    #[test]
    fn a_base_game_parses() {
        let info = parse_info(WITCHER.as_bytes()).unwrap();
        assert_eq!(info.game_id, "1207658924");
        assert_eq!(info.root_game_id, "1207658924");
        assert_eq!(info.name, "The Witcher: Enhanced Edition");
        assert_eq!(info.build_id, "56398707826926290");
        assert_eq!(info.client_id, "50225266424144145");
        assert_eq!(info.language, "English");
        assert_eq!(info.play_tasks.len(), 2, "the URLTask is dropped");
        assert!(!info.is_dlc());
        let primary = info.primary_task().unwrap();
        assert_eq!(primary.path, "System\\witcher.exe");
        assert_eq!(primary.arguments, "-nointro");
        assert_eq!(primary.working_dir, "System");
        assert_eq!(primary.category, "game");
    }

    #[test]
    fn a_dlc_names_its_base_game() {
        let info = parse_info(
            br#"{"gameId": 1207659063, "rootGameId": "1207658924", "name": "Bonus", "playTasks": []}"#,
        )
        .unwrap();
        assert_eq!(info.game_id, "1207659063", "a numeric id reads as text");
        assert!(info.is_dlc());
        assert!(info.primary_task().is_none());
    }

    #[test]
    fn a_blank_root_is_a_base_game() {
        let info = parse_info(br#"{"gameId": "5", "name": "Old"}"#).unwrap();
        assert!(!info.is_dlc());
    }

    #[test]
    fn without_a_primary_task_the_first_game_task_launches() {
        let info = parse_info(
            br#"{"gameId": "5", "playTasks": [
                {"category": "tool", "path": "config.exe"},
                {"category": "game", "path": "game.exe"},
                {"category": "game", "path": "other.exe"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(info.primary_task().unwrap().path, "game.exe");
        let none =
            parse_info(br#"{"gameId": "5", "playTasks": [{"category": "tool", "path": "x.exe"}]}"#)
                .unwrap();
        assert!(none.primary_task().is_none());
    }

    #[test]
    fn a_byte_order_mark_is_allowed() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice(br#"{"gameId": "7"}"#);
        assert_eq!(parse_info(&bytes).unwrap().game_id, "7");
    }

    #[test]
    fn bad_files_are_none() {
        for bad in [
            &b""[..],
            b"not json",
            b"[]",
            b"{}",
            br#"{"gameId": ""}"#,
            br#"{"gameId": null}"#,
        ] {
            assert!(
                parse_info(bad).is_none(),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }

    #[test]
    fn a_malformed_task_is_dropped_not_fatal() {
        let info = parse_info(
            br#"{"gameId": "5", "playTasks": [{"isPrimary": "yes", "path": 3}, {"isPrimary": true, "path": "ok.exe"}]}"#,
        )
        .unwrap();
        assert_eq!(info.play_tasks.len(), 1);
        assert_eq!(info.primary_task().unwrap().path, "ok.exe");
    }

    #[test]
    fn info_files_are_found_sorted_and_filtered() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("goggame-2.info"), "{}").unwrap();
        fs::write(dir.path().join("goggame-1.info"), "{}").unwrap();
        fs::write(dir.path().join("goggame-1.hashdb"), "{}").unwrap();
        fs::write(dir.path().join("readme.info"), "{}").unwrap();
        fs::create_dir(dir.path().join("goggame-3.info")).unwrap();
        let found = find_info_files(dir.path());
        assert_eq!(
            found,
            vec![
                dir.path().join("goggame-1.info"),
                dir.path().join("goggame-2.info")
            ]
        );
        assert!(find_info_files(&dir.path().join("missing")).is_empty());
    }
}
