//! Native (Windows) game archive selection, `game.json` metadata, executable
//! resolution, and update merge.
//!
//! Ports `grid_launcher/library/install_metadata.py:146-243`
//! (`parse_windows_game_json`, `apply_windows_game_json_to_game`,
//! `select_native_archive_entry`), `grid_launcher/library/install_paths.py:
//! 92-145` (`native_install_dir_for_game`,
//! `native_executable_candidates_for_game`,
//! `resolved_native_executable_path_for_game`),
//! `grid_launcher/emulator/launch.py:11-28` (native/emulator suffix sets),
//! `grid_launcher/library/archive_preparation.py:1236-1345`
//! (`merge_archive_into_directory`, `prepare_native_game_update_without_ui`),
//! and `grid_launcher/ui/mixins/install_mixin.py:806-820`
//! (`_native_update_temp_dir_for_game`); see
//! `docs/porting/03-library-install.md` §2a, §7, §14 for the behavior
//! contract this mirrors.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::merge_tree;
use super::ps4::ExtractFn;
use crate::library::launch_select::select_launch_file;
use crate::library::paths::{expand_home, sanitize_component};
use crate::library::registry::InstalledGame;
use crate::romm::{RomDetail, RomFile};

/// Lower-cased archive extensions (with leading dot) that mark a native
/// game's payload entry. Matches `_NATIVE_ARCHIVE_EXTENSIONS`
/// (`install_metadata.py:204`).
pub const NATIVE_ARCHIVE_SUFFIXES: [&str; 9] = [
    ".7z", ".zip", ".rar", ".tar", ".gz", ".tgz", ".xz", ".zst", ".bz2",
];

/// File extensions (without the leading dot) that identify a launchable
/// native game file. Matches `_NATIVE_GAME_SUFFIXES` (`emulator/launch.py:
/// 11`, stored there with a leading dot; compared here against
/// [`Path::extension`], which never returns one).
pub const NATIVE_GAME_SUFFIXES: [&str; 5] = ["exe", "bat", "cmd", "ps1", "sh"];

// ---------------------------------------------------------------------------
// Archive / game.json selection
// ---------------------------------------------------------------------------

/// Picks the server file entry holding a native game's archive: `game.json`
/// and entries whose name contains a path separator are never candidates.
/// Prefers the first candidate whose case-folded name ends with a known
/// archive suffix, else falls back to the first remaining candidate.
/// Matches `select_native_archive_entry` (`install_metadata.py:217`).
pub fn select_archive(files: &[RomFile]) -> Option<&RomFile> {
    let candidates: Vec<&RomFile> = files
        .iter()
        .filter(|f| {
            !f.file_name.is_empty()
                && f.file_name.to_lowercase() != "game.json"
                && !f.file_name.contains('/')
                && !f.file_name.contains('\\')
        })
        .collect();

    let archive = candidates.iter().find(|f| {
        let lower = f.file_name.to_lowercase();
        NATIVE_ARCHIVE_SUFFIXES
            .iter()
            .any(|suf| lower.ends_with(suf))
    });

    archive.or(candidates.first()).copied()
}

/// The top-level `game.json` entry, if present (case-folded name match).
pub fn has_game_json(files: &[RomFile]) -> Option<&RomFile> {
    files
        .iter()
        .find(|f| f.is_top_level && f.file_name.to_lowercase() == "game.json")
}

/// Parsed fields of a Windows `game.json` sidecar. Matches the dict returned
/// by `parse_windows_game_json` (`install_metadata.py:146`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameJson {
    pub revision: String,
    pub first_release_date: String,
    pub tags: String,
    pub included_dlc: String,
    pub name: String,
}

/// Whether a JSON value counts as "falsy" the way Python's `or` treats it:
/// `null`, `false`, a zero number, an empty string, or an empty array/object.
fn is_json_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64().map(|f| f == 0.0).unwrap_or(false),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

/// `int(value)` the way Python coerces it for `year`/`release_year`: a
/// number truncates toward zero, a numeric string (post-`trim`) parses as an
/// integer, a bool is 0/1, and anything else (missing, `null`, a
/// non-numeric string, an array/object) is uncoercible.
fn coerce_year(value: Option<&Value>) -> String {
    match value {
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|f| (f.trunc() as i64).to_string())
            .unwrap_or_default(),
        Some(Value::String(s)) => s
            .trim()
            .parse::<i64>()
            .map(|n| n.to_string())
            .unwrap_or_default(),
        Some(Value::Bool(b)) => if *b { "1" } else { "0" }.to_string(),
        _ => String::new(),
    }
}

/// Stringifies `value` the way Python's `str()` does for a JSON scalar: a
/// string is used verbatim (never re-quoted), everything else falls back to
/// its JSON rendering (which matches `str()` for numbers).
fn stringify(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Leniently parses a `game.json` sidecar's bytes. Invalid JSON or a
/// top-level value that isn't an object yields `None` (Python's `{}`, an
/// empty/falsy dict — see [`apply_game_json`]). Matches
/// `parse_windows_game_json` (`install_metadata.py:146`).
pub fn parse_game_json(bytes: &[u8]) -> Option<GameJson> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let Value::Object(map) = value else {
        return None;
    };

    let revision = match map.get("version") {
        None | Some(Value::Null) => String::new(),
        Some(v) => stringify(v),
    };

    let year_source = match map.get("year") {
        Some(v) if !is_json_falsy(v) => Some(v),
        _ => map.get("release_year"),
    };
    let first_release_date = coerce_year(year_source);

    let tags = match map.get("tags") {
        Some(Value::Array(items)) if !items.is_empty() && items.iter().all(Value::is_string) => {
            items
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ")
        }
        _ => String::new(),
    };

    let included_dlc = match map.get("included_dlc") {
        Some(Value::Array(items)) => {
            serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string())
        }
        _ => "[]".to_string(),
    };

    let name = match map.get("name") {
        None | Some(Value::Null) => String::new(),
        Some(v) => stringify(v),
    };

    Some(GameJson {
        revision,
        first_release_date,
        tags,
        included_dlc,
        name,
    })
}

/// Applies a parsed `game.json` to an installed-game record: `revision`,
/// `first_release_date`, and `tags` are filled only when the row's current
/// value is blank; `included_dlc` is always overwritten. `name` is parsed
/// but never written back, matching the Python original. Matches
/// `apply_windows_game_json_to_game` (`install_metadata.py:189`); the "does
/// nothing when `parsed` is empty" branch there is expressed here by simply
/// not calling this function when [`parse_game_json`] returns `None`.
pub fn apply_game_json(row: &mut InstalledGame, parsed: &GameJson) {
    if !parsed.revision.is_empty() && row.revision.trim().is_empty() {
        row.revision = parsed.revision.clone();
    }
    if !parsed.first_release_date.is_empty() && row.first_release_date.trim().is_empty() {
        row.first_release_date = parsed.first_release_date.clone();
    }
    if !parsed.tags.is_empty() && row.tags.trim().is_empty() {
        row.tags = parsed.tags.clone();
    }
    row.included_dlc = parsed.included_dlc.clone();
}

// ---------------------------------------------------------------------------
// Install directory / executable resolution
// ---------------------------------------------------------------------------

/// Whether `path`'s extension (case-folded) identifies a launchable native
/// game file. Matches `launchable_native_game_file` (`emulator/launch.py:
/// 23`).
pub fn is_launchable_native_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            let ext = ext.to_string_lossy().to_lowercase();
            NATIVE_GAME_SUFFIXES.iter().any(|suf| *suf == ext)
        })
        .unwrap_or(false)
}

/// The installed native game's directory: `extracted_dir` if it exists as a
/// directory; else the parent of `extracted_path` if that exists as a file;
/// else the parent of the first `archive_candidates` entry that exists as a
/// file. `None` when nothing resolves. Matches
/// `native_install_dir_for_game` (`install_paths.py:92`).
pub fn install_dir(row: &InstalledGame, archive_candidates: &[PathBuf]) -> Option<PathBuf> {
    let extracted_dir_text = row.extracted_dir.trim();
    if !extracted_dir_text.is_empty() {
        let extracted_dir = expand_home(extracted_dir_text);
        if extracted_dir.is_dir() {
            return Some(extracted_dir);
        }
    }

    let extracted_path_text = row.extracted_path.trim();
    if !extracted_path_text.is_empty() {
        let extracted_path = expand_home(extracted_path_text);
        if extracted_path.is_file() {
            if let Some(parent) = extracted_path.parent() {
                return Some(parent.to_path_buf());
            }
        }
    }

    for candidate in archive_candidates {
        if candidate.is_file() {
            if let Some(parent) = candidate.parent() {
                return Some(parent.to_path_buf());
            }
        }
    }
    None
}

/// Every launchable native file recursively under `install_dir`, sorted by
/// (path component count, case-folded path). Matches
/// `native_executable_candidates_for_game` (`install_paths.py:114`).
pub fn executable_candidates(install_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    collect_files(install_dir, &mut candidates);
    candidates.retain(|path| is_launchable_native_file(path));
    candidates.sort_by_key(|path| {
        (
            path.components().count(),
            path.to_string_lossy().to_lowercase(),
        )
    });
    candidates
}

/// Recursively collects every regular file under `dir` into `out`.
/// Unreadable directories are skipped rather than causing an error.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(&path, out);
        } else if file_type.is_file() {
            out.push(path);
        }
    }
}

/// The executable to launch: the row's pinned `native_executable_path` when
/// it exists, is a file, and is launchable; else the first of `candidates`.
/// Matches `resolved_native_executable_path_for_game` (`install_paths.py:
/// 130`).
pub fn resolved_executable(row: &InstalledGame, candidates: &[PathBuf]) -> Option<PathBuf> {
    let pinned_text = row.native_executable_path.trim();
    if !pinned_text.is_empty() {
        let pinned = expand_home(pinned_text);
        if pinned.is_file() && is_launchable_native_file(&pinned) {
            return Some(pinned);
        }
    }
    candidates.first().cloned()
}

// ---------------------------------------------------------------------------
// Update merge
// ---------------------------------------------------------------------------

/// The temp extraction directory to use when merging an update archive into
/// `row`'s install: `<extracted_dir parent>/<safe title>-temp` when
/// `extracted_dir` is set, so extraction and merge stay on the same
/// filesystem; else `<system temp>/grid-launcher-<safe title>-temp`. Matches
/// `_native_update_temp_dir_for_game` (`install_mixin.py:806`).
pub fn update_temp_dir(row: &InstalledGame) -> PathBuf {
    let safe_title = sanitize_component(&row.title, "game");
    let extracted_dir_text = row.extracted_dir.trim();
    if !extracted_dir_text.is_empty() {
        let extracted_dir = Path::new(extracted_dir_text);
        let parent = extracted_dir.parent().unwrap_or_else(|| Path::new(""));
        return parent.join(format!("{safe_title}-temp"));
    }
    std::env::temp_dir().join(format!("grid-launcher-{safe_title}-temp"))
}

/// The result of a successful [`apply_update`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeUpdate {
    pub row: InstalledGame,
    pub warning: String,
}

/// Merges a new archive into an already-installed native game, in place of
/// replacing it: files the archive doesn't touch (saves, configs,
/// keybindings) are preserved. Matches
/// `prepare_native_game_update_without_ui` (`archive_preparation.py:1264`)
/// composed with `merge_archive_into_directory` (`archive_preparation.py:
/// 1236`).
///
/// 1. Overwrites server metadata from `detail` onto a copy of `row`, only
///    with non-empty values (`rom_id` is always set); `ra_id` is left alone
///    (`RomDetail` carries no `ra_id` today).
/// 2. Requires a non-blank `extracted_dir` that exists as a directory.
/// 3. Extracts `archive` into `temp_dir` (any pre-existing `temp_dir` is
///    removed first, ignoring errors) and merges it into `extracted_dir`
///    via [`merge_tree`]; `temp_dir` is always removed afterwards, success
///    or failure.
/// 4. Re-detects the launch file via [`select_launch_file`] and updates
///    `extracted_path` only when the row has no manual
///    `native_executable_path` pinned.
/// 5. Deletes `archive` when it is a file; a failure becomes a warning
///    rather than an error.
pub fn apply_update(
    row: &InstalledGame,
    detail: &RomDetail,
    archive: &Path,
    temp_dir: &Path,
    extract: ExtractFn,
) -> Result<NativeUpdate, String> {
    let mut prepared = row.clone();

    prepared.rom_id = Some(detail.id);
    if !detail.fs_name.is_empty() {
        prepared.rom_file_name = detail.fs_name.clone();
    }
    if !detail.server_updated_at.is_empty() {
        prepared.server_updated_at = detail.server_updated_at.clone();
    }
    if !detail.description.is_empty() {
        prepared.description = detail.description.clone();
    }
    if !detail.rating.is_empty() {
        prepared.rating = detail.rating.clone();
    }
    if !detail.genres.is_empty() {
        prepared.genres = detail.genres.clone();
    }
    if !detail.regions.is_empty() {
        prepared.regions = detail.regions.clone();
    }
    if detail.filesize_bytes != 0 {
        prepared.filesize_bytes = detail.filesize_bytes;
    }
    if !detail.screenshot_urls.is_empty() {
        prepared.screenshot_urls = detail.screenshot_urls.join("\n");
    }
    if !detail.fanart_urls.is_empty() {
        prepared.fanart_urls = detail.fanart_urls.join("\n");
    }
    // ra_id: RomDetail has none, so `prepared.ra_id` keeps row's value as-is.

    let extracted_dir_text = prepared.extracted_dir.trim();
    if extracted_dir_text.is_empty() {
        return Err(
            "Installed game directory not found - reinstall the game and try again.".to_string(),
        );
    }
    let extracted_dir = PathBuf::from(extracted_dir_text);
    if !extracted_dir.is_dir() {
        return Err(format!(
            "Installed game directory does not exist: {}",
            extracted_dir.display()
        ));
    }

    if temp_dir.exists() {
        let _ = fs::remove_dir_all(temp_dir);
    }
    let merge_result = extract(archive, temp_dir)
        .map_err(|e| e.to_string())
        .and_then(|()| merge_tree(temp_dir, &extracted_dir).map_err(|e| e.to_string()));
    let _ = fs::remove_dir_all(temp_dir);
    merge_result?;

    let archive_stem = archive
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(new_launch_file) = select_launch_file(&extracted_dir, &archive_stem) {
        if prepared.native_executable_path.trim().is_empty() {
            prepared.extracted_path = new_launch_file.to_string_lossy().into_owned();
        }
    }

    let mut warning = String::new();
    if archive.is_file() {
        if let Err(error) = fs::remove_file(archive) {
            warning = format!(
                "Updated {}, but could not delete archive:\n{}\n{error}",
                prepared.title,
                archive.display()
            );
        }
    }

    Ok(NativeUpdate {
        row: prepared,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::LibraryError;

    fn rom_file(file_name: &str, is_top_level: bool) -> RomFile {
        RomFile {
            id: 1,
            file_name: file_name.to_string(),
            file_size_bytes: 0,
            is_top_level,
            last_modified: String::new(),
            category: String::new(),
        }
    }

    // -- select_archive -------------------------------------------------------

    #[test]
    fn select_archive_skips_game_json_and_nested_names() {
        let files = vec![
            rom_file("game.json", true),
            rom_file("sub/nested.zip", true),
            rom_file("Game.7z", true),
        ];
        let selected = select_archive(&files).unwrap();
        assert_eq!(selected.file_name, "Game.7z");
    }

    #[test]
    fn select_archive_prefers_archive_suffix_over_earlier_extra() {
        let files = vec![rom_file("soundtrack.mp3", true), rom_file("Game.zip", true)];
        let selected = select_archive(&files).unwrap();
        assert_eq!(selected.file_name, "Game.zip");
    }

    #[test]
    fn select_archive_falls_back_to_first_top_level_when_no_archive_suffix() {
        let files = vec![rom_file("game.iso", true), rom_file("extra.txt", true)];
        let selected = select_archive(&files).unwrap();
        assert_eq!(selected.file_name, "game.iso");
    }

    #[test]
    fn select_archive_returns_none_when_only_game_json() {
        let files = vec![rom_file("game.json", true)];
        assert!(select_archive(&files).is_none());
    }

    // -- has_game_json ----------------------------------------------------------

    #[test]
    fn has_game_json_matches_case_folded_top_level_entry() {
        let files = vec![rom_file("Game.zip", true), rom_file("GAME.JSON", true)];
        let found = has_game_json(&files).unwrap();
        assert_eq!(found.file_name, "GAME.JSON");
    }

    #[test]
    fn has_game_json_ignores_non_top_level_entry() {
        let files = vec![rom_file("sub/game.json", false)];
        assert!(has_game_json(&files).is_none());
    }

    // -- parse_game_json ----------------------------------------------------------

    #[test]
    fn parse_game_json_returns_none_for_invalid_json() {
        assert_eq!(parse_game_json(b"not json"), None);
    }

    #[test]
    fn parse_game_json_returns_none_for_non_object_top_level() {
        assert_eq!(parse_game_json(b"[1, 2, 3]"), None);
    }

    #[test]
    fn parse_game_json_stringifies_numeric_version() {
        let parsed = parse_game_json(br#"{"version": 2}"#).unwrap();
        assert_eq!(parsed.revision, "2");
    }

    #[test]
    fn parse_game_json_null_version_is_blank() {
        let parsed = parse_game_json(br#"{"version": null}"#).unwrap();
        assert_eq!(parsed.revision, "");
    }

    #[test]
    fn parse_game_json_parses_numeric_string_year() {
        let parsed = parse_game_json(br#"{"year": "1998"}"#).unwrap();
        assert_eq!(parsed.first_release_date, "1998");
    }

    #[test]
    fn parse_game_json_non_numeric_year_is_blank() {
        let parsed = parse_game_json(br#"{"year": "x"}"#).unwrap();
        assert_eq!(parsed.first_release_date, "");
    }

    #[test]
    fn parse_game_json_falls_back_to_release_year() {
        let parsed = parse_game_json(br#"{"release_year": 2001}"#).unwrap();
        assert_eq!(parsed.first_release_date, "2001");
    }

    #[test]
    fn parse_game_json_missing_year_fields_is_blank() {
        let parsed = parse_game_json(br#"{}"#).unwrap();
        assert_eq!(parsed.first_release_date, "");
    }

    #[test]
    fn parse_game_json_joins_string_tags() {
        let parsed = parse_game_json(br#"{"tags": ["rpg", "indie"]}"#).unwrap();
        assert_eq!(parsed.tags, "rpg, indie");
    }

    #[test]
    fn parse_game_json_non_string_tags_is_blank() {
        let parsed = parse_game_json(br#"{"tags": ["rpg", 5]}"#).unwrap();
        assert_eq!(parsed.tags, "");
    }

    #[test]
    fn parse_game_json_empty_tags_list_is_blank() {
        let parsed = parse_game_json(br#"{"tags": []}"#).unwrap();
        assert_eq!(parsed.tags, "");
    }

    #[test]
    fn parse_game_json_included_dlc_is_compact_json() {
        let parsed = parse_game_json(br#"{"included_dlc": ["dlc1", "dlc2"]}"#).unwrap();
        assert_eq!(parsed.included_dlc, r#"["dlc1","dlc2"]"#);
    }

    #[test]
    fn parse_game_json_missing_included_dlc_is_empty_array() {
        let parsed = parse_game_json(br#"{}"#).unwrap();
        assert_eq!(parsed.included_dlc, "[]");
    }

    #[test]
    fn parse_game_json_stringifies_name() {
        let parsed = parse_game_json(br#"{"name": "My Game"}"#).unwrap();
        assert_eq!(parsed.name, "My Game");
    }

    // -- apply_game_json ----------------------------------------------------------

    #[test]
    fn apply_game_json_fills_only_blank_fields() {
        let mut row = InstalledGame {
            revision: "existing".to_string(),
            ..Default::default()
        };
        let parsed = GameJson {
            revision: "new".to_string(),
            first_release_date: "1999".to_string(),
            tags: "rpg".to_string(),
            included_dlc: "[]".to_string(),
            name: "".to_string(),
        };
        apply_game_json(&mut row, &parsed);
        assert_eq!(row.revision, "existing");
        assert_eq!(row.first_release_date, "1999");
        assert_eq!(row.tags, "rpg");
    }

    #[test]
    fn apply_game_json_always_sets_included_dlc() {
        let mut row = InstalledGame {
            included_dlc: r#"["old"]"#.to_string(),
            ..Default::default()
        };
        let parsed = GameJson {
            included_dlc: "[]".to_string(),
            ..Default::default()
        };
        apply_game_json(&mut row, &parsed);
        assert_eq!(row.included_dlc, "[]");
    }

    // -- executable_candidates ----------------------------------------------------

    #[test]
    fn executable_candidates_orders_by_depth_then_casefold() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a")).unwrap();
        fs::create_dir_all(dir.path().join("b/c")).unwrap();
        fs::write(dir.path().join("a/z.exe"), b"x").unwrap();
        fs::write(dir.path().join("b/c/a.exe"), b"x").unwrap();

        let candidates = executable_candidates(dir.path());
        assert_eq!(
            candidates,
            vec![dir.path().join("a/z.exe"), dir.path().join("b/c/a.exe")]
        );
    }

    #[test]
    fn executable_candidates_accepts_uppercase_suffix() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Game.EXE"), b"x").unwrap();

        let candidates = executable_candidates(dir.path());
        assert_eq!(candidates, vec![dir.path().join("Game.EXE")]);
    }

    #[test]
    fn executable_candidates_ignores_non_launchable_extensions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), b"x").unwrap();

        assert!(executable_candidates(dir.path()).is_empty());
    }

    // -- resolved_executable ----------------------------------------------------

    #[test]
    fn resolved_executable_prefers_existing_launchable_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pinned = dir.path().join("Custom.exe");
        fs::write(&pinned, b"x").unwrap();
        let candidate = dir.path().join("other.exe");
        fs::write(&candidate, b"x").unwrap();

        let row = InstalledGame {
            native_executable_path: pinned.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(resolved_executable(&row, &[candidate]), Some(pinned));
    }

    #[test]
    fn resolved_executable_falls_back_when_pin_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("other.exe");
        fs::write(&candidate, b"x").unwrap();

        let row = InstalledGame {
            native_executable_path: dir
                .path()
                .join("missing.exe")
                .to_string_lossy()
                .into_owned(),
            ..Default::default()
        };
        assert_eq!(
            resolved_executable(&row, std::slice::from_ref(&candidate)),
            Some(candidate)
        );
    }

    #[test]
    fn resolved_executable_falls_back_when_pin_is_not_launchable() {
        let dir = tempfile::tempdir().unwrap();
        let pinned = dir.path().join("notes.txt");
        fs::write(&pinned, b"x").unwrap();
        let candidate = dir.path().join("other.exe");
        fs::write(&candidate, b"x").unwrap();

        let row = InstalledGame {
            native_executable_path: pinned.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(
            resolved_executable(&row, std::slice::from_ref(&candidate)),
            Some(candidate)
        );
    }

    #[test]
    fn resolved_executable_none_when_no_pin_and_no_candidates() {
        let row = InstalledGame::default();
        assert_eq!(resolved_executable(&row, &[]), None);
    }

    // -- install_dir ----------------------------------------------------------

    #[test]
    fn install_dir_prefers_extracted_dir_when_it_is_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("install");
        fs::create_dir_all(&extracted_dir).unwrap();

        let row = InstalledGame {
            extracted_dir: extracted_dir.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(install_dir(&row, &[]), Some(extracted_dir));
    }

    #[test]
    fn install_dir_falls_back_to_extracted_path_parent() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let extracted_path = install.join("game.exe");
        fs::write(&extracted_path, b"x").unwrap();

        let row = InstalledGame {
            extracted_path: extracted_path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(install_dir(&row, &[]), Some(install));
    }

    #[test]
    fn install_dir_falls_back_to_first_existing_archive_parent() {
        let dir = tempfile::tempdir().unwrap();
        let archive_dir = dir.path().join("archives");
        fs::create_dir_all(&archive_dir).unwrap();
        let archive = archive_dir.join("game.zip");
        fs::write(&archive, b"x").unwrap();
        let missing = dir.path().join("missing.zip");

        let row = InstalledGame::default();
        assert_eq!(install_dir(&row, &[missing, archive]), Some(archive_dir));
    }

    #[test]
    fn install_dir_none_when_nothing_resolves() {
        let row = InstalledGame::default();
        assert_eq!(install_dir(&row, &[]), None);
    }

    // -- update_temp_dir ----------------------------------------------------------

    #[test]
    fn update_temp_dir_uses_extracted_dir_parent() {
        let row = InstalledGame {
            title: "My Game".to_string(),
            extracted_dir: "/library/Windows/My Game".to_string(),
            ..Default::default()
        };
        assert_eq!(
            update_temp_dir(&row),
            PathBuf::from("/library/Windows/My Game-temp")
        );
    }

    #[test]
    fn update_temp_dir_falls_back_to_system_temp() {
        let row = InstalledGame {
            title: "My Game".to_string(),
            ..Default::default()
        };
        assert_eq!(
            update_temp_dir(&row),
            std::env::temp_dir().join("grid-launcher-My Game-temp")
        );
    }

    // -- apply_update ----------------------------------------------------------

    fn rom_detail(files: Vec<RomFile>) -> RomDetail {
        RomDetail {
            id: 99,
            name: "Updated Game".to_string(),
            platform_id: 1,
            platform_name: "Windows".to_string(),
            fs_name: "".to_string(),
            description: "".to_string(),
            regions: "".to_string(),
            languages: "".to_string(),
            tags: "".to_string(),
            revision: "".to_string(),
            rating: "".to_string(),
            genres: "".to_string(),
            companies: "".to_string(),
            first_release_date: "".to_string(),
            franchises: "".to_string(),
            game_modes: "".to_string(),
            player_count: "".to_string(),
            filesize_bytes: 0,
            server_updated_at: "".to_string(),
            files,
            cover_small_path: "".to_string(),
            cover_large_path: "".to_string(),
            screenshot_urls: Vec::new(),
            fanart_urls: Vec::new(),
            youtube_video_id: "".to_string(),
            video_path: "".to_string(),
            is_identified: false,
            related: Vec::new(),
        }
    }

    /// An [`ExtractFn`]-shaped closure that just copies a prepared directory
    /// into the destination, standing in for the real extractor.
    fn copy_extract(prepared: &Path) -> impl Fn(&Path, &Path) -> Result<(), LibraryError> + '_ {
        move |_archive: &Path, dest: &Path| {
            super::super::copy_tree_merge(prepared, dest).map_err(LibraryError::Io)
        }
    }

    fn setup_install(dir: &Path) -> (InstalledGame, PathBuf) {
        let extracted_dir = dir.join("install");
        fs::create_dir_all(&extracted_dir).unwrap();
        fs::write(extracted_dir.join("keep.dat"), b"keep me").unwrap();
        let row = InstalledGame {
            title: "My Game".to_string(),
            platform: "Windows".to_string(),
            extracted_dir: extracted_dir.to_string_lossy().into_owned(),
            ..Default::default()
        };
        (row, extracted_dir)
    }

    #[test]
    fn apply_update_merges_new_file_and_preserves_unrelated_file() {
        let dir = tempfile::tempdir().unwrap();
        let (row, extracted_dir) = setup_install(dir.path());

        let prepared_archive_contents = dir.path().join("prepared");
        fs::create_dir_all(&prepared_archive_contents).unwrap();
        fs::write(prepared_archive_contents.join("new.exe"), b"exe bytes").unwrap();

        let archive = dir.path().join("update.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let temp_dir = dir.path().join("temp");

        let extract = copy_extract(&prepared_archive_contents);
        let detail = rom_detail(Vec::new());
        let result = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap();

        assert_eq!(result.warning, "");
        assert!(!archive.exists());
        assert!(!temp_dir.exists());
        assert!(extracted_dir.join("new.exe").is_file());
        assert!(extracted_dir.join("keep.dat").is_file());
        assert_eq!(result.row.rom_id, Some(99));
    }

    #[test]
    fn apply_update_redetects_launch_file_when_no_pin() {
        let dir = tempfile::tempdir().unwrap();
        let (row, extracted_dir) = setup_install(dir.path());

        let prepared_archive_contents = dir.path().join("prepared");
        fs::create_dir_all(&prepared_archive_contents).unwrap();
        fs::write(prepared_archive_contents.join("update.exe"), b"exe bytes").unwrap();

        let archive = dir.path().join("update.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let temp_dir = dir.path().join("temp");

        let extract = copy_extract(&prepared_archive_contents);
        let detail = rom_detail(Vec::new());
        let result = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap();

        assert_eq!(
            result.row.extracted_path,
            extracted_dir.join("update.exe").to_string_lossy()
        );
    }

    #[test]
    fn apply_update_keeps_extracted_path_when_pinned() {
        let dir = tempfile::tempdir().unwrap();
        let (mut row, _extracted_dir) = setup_install(dir.path());
        row.native_executable_path = "/somewhere/manual.exe".to_string();

        let prepared_archive_contents = dir.path().join("prepared");
        fs::create_dir_all(&prepared_archive_contents).unwrap();
        fs::write(prepared_archive_contents.join("update.exe"), b"exe bytes").unwrap();

        let archive = dir.path().join("update.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let temp_dir = dir.path().join("temp");

        let extract = copy_extract(&prepared_archive_contents);
        let detail = rom_detail(Vec::new());
        let result = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap();

        assert_eq!(result.row.extracted_path, "");
    }

    #[test]
    fn apply_update_errors_when_extracted_dir_blank() {
        let dir = tempfile::tempdir().unwrap();
        let row = InstalledGame {
            title: "My Game".to_string(),
            ..Default::default()
        };
        let archive = dir.path().join("update.zip");
        let temp_dir = dir.path().join("temp");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };
        let detail = rom_detail(Vec::new());

        let error = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap_err();
        assert_eq!(
            error,
            "Installed game directory not found - reinstall the game and try again."
        );
    }

    #[test]
    fn apply_update_errors_when_extracted_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nowhere");
        let row = InstalledGame {
            title: "My Game".to_string(),
            extracted_dir: missing.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let archive = dir.path().join("update.zip");
        let temp_dir = dir.path().join("temp");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };
        let detail = rom_detail(Vec::new());

        let error = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap_err();
        assert_eq!(
            error,
            format!(
                "Installed game directory does not exist: {}",
                missing.display()
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_update_archive_delete_failure_becomes_a_warning() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let (row, extracted_dir) = setup_install(dir.path());

        let prepared_archive_contents = dir.path().join("prepared");
        fs::create_dir_all(&prepared_archive_contents).unwrap();
        fs::write(prepared_archive_contents.join("update.exe"), b"exe bytes").unwrap();

        let archive_dir = dir.path().join("archive_dir");
        fs::create_dir_all(&archive_dir).unwrap();
        let archive = archive_dir.join("update.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        fs::set_permissions(&archive_dir, fs::Permissions::from_mode(0o555)).unwrap();
        let temp_dir = dir.path().join("temp");

        let extract = copy_extract(&prepared_archive_contents);
        let detail = rom_detail(Vec::new());
        let outcome = apply_update(&row, &detail, &archive, &temp_dir, &extract);

        fs::set_permissions(&archive_dir, fs::Permissions::from_mode(0o755)).unwrap();

        let result = outcome.unwrap();
        assert!(result.warning.starts_with(&format!(
            "Updated My Game, but could not delete archive:\n{}\n",
            archive.display()
        )));
        assert!(archive.exists());
        assert!(extracted_dir.join("keep.dat").is_file());
    }

    #[test]
    fn apply_update_missing_archive_yields_no_warning() {
        let dir = tempfile::tempdir().unwrap();
        let (row, _extracted_dir) = setup_install(dir.path());

        let prepared_archive_contents = dir.path().join("prepared");
        fs::create_dir_all(&prepared_archive_contents).unwrap();
        fs::write(prepared_archive_contents.join("update.exe"), b"exe bytes").unwrap();

        let archive = dir.path().join("does-not-exist.zip");
        let temp_dir = dir.path().join("temp");

        let extract = copy_extract(&prepared_archive_contents);
        let detail = rom_detail(Vec::new());
        let result = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap();

        assert_eq!(result.warning, "");
    }

    #[test]
    fn apply_update_overwrites_metadata_only_with_non_empty_values() {
        let dir = tempfile::tempdir().unwrap();
        let (mut row, _extracted_dir) = setup_install(dir.path());
        row.description = "existing description".to_string();
        row.filesize_bytes = 12345;

        let prepared_archive_contents = dir.path().join("prepared");
        fs::create_dir_all(&prepared_archive_contents).unwrap();

        let archive = dir.path().join("update.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let temp_dir = dir.path().join("temp");

        let extract = copy_extract(&prepared_archive_contents);
        let mut detail = rom_detail(Vec::new());
        detail.description = String::new();
        detail.filesize_bytes = 0;
        detail.rating = "9.5".to_string();
        detail.screenshot_urls = vec!["http://a".to_string(), "http://b".to_string()];

        let result = apply_update(&row, &detail, &archive, &temp_dir, &extract).unwrap();

        assert_eq!(result.row.description, "existing description");
        assert_eq!(result.row.filesize_bytes, 12345);
        assert_eq!(result.row.rating, "9.5");
        assert_eq!(result.row.screenshot_urls, "http://a\nhttp://b");
    }
}
