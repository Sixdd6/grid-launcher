//! PS4 title-id detection, launch-file (eboot) ranking, and update/DLC
//! content apply.
//!
//! Ports `grid_launcher/library/archive_preparation.py`'s PS4 helpers
//! function-for-function; see `docs/porting/03-library-install.md` §12 for
//! the behavior contract this mirrors.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::Value;

use super::merge_tree;
use crate::library::content::ContentKind;
use crate::library::platforms::is_ps4_platform;
use crate::library::registry::InstalledGame;
use crate::library::LibraryError;

/// A normalized PS4 title id: four upper-case letters followed by five
/// digits (`CUSA12345`). Matches `_PS4_GAME_ID_PATTERN`
/// (`archive_preparation.py:54`).
static TITLE_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]{4}\d{5}$").unwrap());

/// An extraction function: given an archive path and an empty destination
/// directory, extracts the archive's contents into that directory or
/// reports a [`LibraryError`]. The install service binds this to
/// [`crate::library::extract::extract_archive`] with a progress sink.
pub type ExtractFn<'a> = &'a dyn Fn(&Path, &Path) -> Result<(), LibraryError>;

/// The result of a successful [`apply_content`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ps4Applied {
    /// The title id the content was applied under, upper-cased (by
    /// construction of [`normalize_title_id`]). This is what the caller
    /// should persist as `ps4_game_id` via
    /// [`crate::library::registry::Registry::update_ps4_content`].
    pub game_id: String,
    /// The updated `ps4_content` column value: the existing entries plus
    /// the new one, serialized as a compact JSON array.
    pub content_json: String,
    /// Non-empty when the content merged successfully but the source
    /// archive could not be deleted afterwards.
    pub warning: String,
}

// ---------------------------------------------------------------------------
// Id helpers
// ---------------------------------------------------------------------------

/// Strips every non-alphanumeric character from `value`, upper-cases the
/// rest, and returns it only if it matches a PS4 title id shape (four
/// letters, five digits). Matches `_ps4_game_id_from_text`
/// (`archive_preparation.py:54`).
pub fn normalize_title_id(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if TITLE_ID_RE.is_match(&cleaned) {
        Some(cleaned)
    } else {
        None
    }
}

fn entry_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The `Normal` (non-root, non-anchor) components of `path`, as owned
/// strings — the Rust analog of Python's `Path.parts`, minus any leading
/// root/prefix anchor (which can never match a title-id pattern anyway).
fn normal_parts(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(os) => Some(os.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// Detects the title id for a freshly extracted PS4 install, in order: the
/// first title-id-shaped segment of `launch_file`'s path relative to
/// `extracted_dir` (excluding the file name itself); then the first
/// top-level directory of `extracted_dir` whose name is a valid id; then
/// walking `launch_file`'s parents up to and including `extracted_dir`;
/// finally `archive`'s file stem. Returns an empty string if none of these
/// yield an id. Matches `_detected_ps4_game_id_for_layout`
/// (`archive_preparation.py:95`).
pub fn detect_title_id(extracted_dir: &Path, launch_file: &Path, archive: &Path) -> String {
    let relative_parts = match launch_file.strip_prefix(extracted_dir) {
        Ok(relative) => normal_parts(relative),
        Err(_) => Vec::new(),
    };
    if !relative_parts.is_empty() {
        for part in &relative_parts[..relative_parts.len() - 1] {
            if let Some(id) = normalize_title_id(part) {
                return id;
            }
        }
    }

    if let Ok(read_dir) = fs::read_dir(extracted_dir) {
        for entry in read_dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(id) = normalize_title_id(&entry_name(&path)) {
                return id;
            }
        }
    }

    for parent in launch_file.ancestors().skip(1) {
        if let Some(id) = normalize_title_id(&entry_name(parent)) {
            return id;
        }
        if parent == extracted_dir {
            break;
        }
    }

    normalize_title_id(
        &archive
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
    .unwrap_or_default()
}

/// Picks the PS4 launch file out of `pool`: only `eboot.bin` (case
/// insensitive) is ever a candidate. When several exist, candidates sort by
/// (a) whether any directory segment of their path relative to
/// `extracted_dir` is one of `extracted_dir`'s top-level title ids — a
/// match sorts first, (b) path depth (shallower first), (c) the
/// case-folded full path. Matches `_select_ps4_launch_file`
/// (`archive_preparation.py:61`).
pub fn select_ps4_launch_file(extracted_dir: &Path, pool: &[PathBuf]) -> Option<PathBuf> {
    let mut eboot_candidates: Vec<&PathBuf> = pool
        .iter()
        .filter(|p| entry_name(p).eq_ignore_ascii_case("eboot.bin"))
        .collect();
    if eboot_candidates.is_empty() {
        return None;
    }

    let mut top_level_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Ok(read_dir) = fs::read_dir(extracted_dir) {
        for entry in read_dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(id) = normalize_title_id(&entry_name(&path)) {
                top_level_ids.insert(id);
            }
        }
    }

    let sort_key = |candidate: &PathBuf| -> (u8, usize, String) {
        let relative_parts = match candidate.strip_prefix(extracted_dir) {
            Ok(relative) => normal_parts(relative),
            Err(_) => normal_parts(candidate),
        };
        let relative_ids: std::collections::HashSet<String> = if relative_parts.is_empty() {
            std::collections::HashSet::new()
        } else {
            relative_parts[..relative_parts.len() - 1]
                .iter()
                .filter_map(|part| normalize_title_id(part))
                .collect()
        };
        let top_level_match_rank: u8 = if !top_level_ids.is_empty()
            && relative_ids.intersection(&top_level_ids).next().is_some()
        {
            0
        } else {
            1
        };
        (
            top_level_match_rank,
            relative_parts.len(),
            candidate.to_string_lossy().to_lowercase(),
        )
    };

    eboot_candidates.sort_by_key(|c| sort_key(c));
    Some(eboot_candidates[0].clone())
}

/// The immediate subdirectories of `directory` whose name is a valid PS4
/// title id, sorted by case-folded name. Returns an empty vector if
/// `directory` can't be read. Matches `_ps4_title_id_roots`
/// (`archive_preparation.py:128`).
pub fn title_id_roots(directory: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    match fs::read_dir(directory) {
        Ok(read_dir) => {
            for entry in read_dir {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if !path.is_dir() {
                            continue;
                        }
                        if normalize_title_id(&entry_name(&path)).is_some() {
                            roots.push(path);
                        }
                    }
                    Err(_) => return Vec::new(),
                }
            }
        }
        Err(_) => return Vec::new(),
    }
    roots.sort_by_key(|p| entry_name(p).to_lowercase());
    roots
}

/// The expected title id for an already-installed PS4 game: its explicit
/// `ps4_game_id`, else the first valid id among the parents of
/// `extracted_path`, else the name of the first title-id root directory
/// inside `extracted_dir`. Returns an empty string when none of these
/// yield an id. Matches `_detected_ps4_game_id_from_installed_game`
/// (`archive_preparation.py:204`).
pub fn expected_title_id(row: &InstalledGame) -> String {
    if let Some(id) = normalize_title_id(&row.ps4_game_id) {
        return id;
    }

    let extracted_path_text = row.extracted_path.trim();
    if !extracted_path_text.is_empty() {
        let extracted_path = Path::new(extracted_path_text);
        for parent in extracted_path.ancestors().skip(1) {
            if let Some(id) = normalize_title_id(&entry_name(parent)) {
                return id;
            }
        }
    }

    let extracted_dir_text = row.extracted_dir.trim();
    if !extracted_dir_text.is_empty() {
        let extracted_dir = Path::new(extracted_dir_text);
        if let Some(first_root) = title_id_roots(extracted_dir).first() {
            return entry_name(first_root).to_uppercase();
        }
    }

    String::new()
}

// ---------------------------------------------------------------------------
// Content entries (the `ps4_content` JSON column)
// ---------------------------------------------------------------------------

/// Leniently parses the `ps4_content` column: blank or non-JSON input, or a
/// non-array, yields an empty list; non-object items are skipped; string
/// values are trimmed, other values are stringified. Matches
/// `_read_ps4_content_entries` (`archive_preparation.py:227`).
pub(crate) fn read_content_entries(text: &str) -> Vec<BTreeMap<String, String>> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Value::Array(items) = parsed else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    for item in items {
        let Value::Object(map) = item else {
            continue;
        };
        let mut normalized: BTreeMap<String, String> = BTreeMap::new();
        for (key, value) in map {
            let normalized_value = match value {
                Value::String(s) => s.trim().to_string(),
                other => other.to_string(),
            };
            normalized.insert(key, normalized_value);
        }
        if !normalized.is_empty() {
            entries.push(normalized);
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Content apply
// ---------------------------------------------------------------------------

/// Removes `staging` (recursively, ignoring errors) when dropped — used so
/// every exit path out of [`apply_content`] after extraction cleans up the
/// extracted content directory, mirroring the Python `finally:
/// shutil.rmtree(content_extract_dir, ignore_errors=True)`
/// (`archive_preparation.py:777`).
struct StagingGuard<'a>(&'a Path);

impl Drop for StagingGuard<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0);
    }
}

/// Applies a PS4 update/DLC content archive to an already-installed game.
/// Extracts `archive` into `staging` (removed on every exit afterwards),
/// requires a title-id root inside it matching the installed game's
/// detected title id, merges that root into
/// `<row.extracted_dir>/<title id>`, appends a metadata entry to the
/// returned `content_json`, and deletes `archive`. Matches
/// `apply_ps4_content_archive_without_ui` (`archive_preparation.py:696`).
pub fn apply_content(
    row: &InstalledGame,
    archive: &Path,
    kind: ContentKind,
    staging: &Path,
    extract: ExtractFn,
) -> Result<Ps4Applied, String> {
    if !is_ps4_platform(&row.platform) {
        return Err("PS4 content apply is only supported for PS4 games".to_string());
    }

    let expected_game_id = expected_title_id(row);
    if expected_game_id.is_empty() {
        return Err("Installed PS4 game is missing a detectable title ID".to_string());
    }

    let extracted_dir_value = row.extracted_dir.trim();
    if extracted_dir_value.is_empty() {
        return Err("Installed PS4 game is missing an extracted install directory".to_string());
    }

    let installed_root = Path::new(extracted_dir_value);
    if !installed_root.is_dir() {
        return Err(format!(
            "Installed PS4 directory does not exist: {}",
            installed_root.display()
        ));
    }

    let target_title_dir = installed_root.join(&expected_game_id);
    if !target_title_dir.is_dir() {
        return Err(format!(
            "Installed PS4 title directory was not found: {}",
            target_title_dir.display()
        ));
    }

    extract(archive, staging).map_err(|e| e.to_string())?;
    let _staging_guard = StagingGuard(staging);

    let content_roots = title_id_roots(staging);
    if content_roots.is_empty() {
        return Err("PS4 content archive must include a title-ID root folder".to_string());
    }

    let matching_root = content_roots
        .iter()
        .find(|root| entry_name(root).to_uppercase() == expected_game_id);

    let source_title_dir = match matching_root {
        Some(root) => root,
        None => {
            let detected_ids = content_roots
                .iter()
                .map(|root| entry_name(root).to_uppercase())
                .collect::<Vec<_>>()
                .join(", ");
            let detected_ids = if detected_ids.is_empty() {
                "unknown".to_string()
            } else {
                detected_ids
            };
            return Err(format!(
                "PS4 content title ID mismatch: expected {expected_game_id}, archive contains {detected_ids}"
            ));
        }
    };

    merge_tree(source_title_dir, &target_title_dir)
        .map_err(|e| format!("Failed to merge PS4 content into installed game: {e}"))?;

    let mut entries = read_content_entries(&row.ps4_content);
    let mut entry: BTreeMap<String, String> = BTreeMap::new();
    entry.insert("kind".to_string(), kind.as_str().to_string());
    entry.insert("title_id".to_string(), expected_game_id.clone());
    entry.insert("archive_name".to_string(), entry_name(archive));
    let applied_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    entry.insert("applied_at".to_string(), applied_at);
    entries.push(entry);
    let content_json = serde_json::to_string(&entries).expect("entries always serialize");

    let mut warning = String::new();
    if let Err(error) = fs::remove_file(archive) {
        warning = format!(
            "Applied PS4 content, but could not delete archive:\n{}\n{error}",
            archive.display()
        );
    }

    Ok(Ps4Applied {
        game_id: expected_game_id,
        content_json,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ps4_row() -> InstalledGame {
        InstalledGame {
            platform: "PS4".to_string(),
            ..Default::default()
        }
    }

    /// An [`ExtractFn`]-shaped closure that just copies a prepared
    /// directory into the destination, standing in for the real extractor.
    fn copy_extract(prepared: &Path) -> impl Fn(&Path, &Path) -> Result<(), LibraryError> + '_ {
        move |_archive: &Path, dest: &Path| {
            super::super::copy_tree_merge(prepared, dest).map_err(LibraryError::Io)
        }
    }

    // -- normalize_title_id --------------------------------------------------

    #[test]
    fn normalize_title_id_strips_and_upper_cases() {
        assert_eq!(
            normalize_title_id("cusa-12345"),
            Some("CUSA12345".to_string())
        );
    }

    #[test]
    fn normalize_title_id_rejects_wrong_shape() {
        assert_eq!(normalize_title_id("CUSA1234"), None);
        assert_eq!(normalize_title_id(""), None);
        assert_eq!(normalize_title_id("CUSA123456"), None);
    }

    // -- detect_title_id ------------------------------------------------------

    #[test]
    fn detect_title_id_prefers_launch_path_segment_over_top_level_dir() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(extracted_dir.join("CUSA11111/deep")).unwrap();
        fs::create_dir_all(extracted_dir.join("CUSA22222")).unwrap();
        let launch_file = extracted_dir.join("CUSA11111/deep/eboot.bin");
        fs::write(&launch_file, b"eboot").unwrap();
        let archive = dir.path().join("archive.zip");

        assert_eq!(
            detect_title_id(&extracted_dir, &launch_file, &archive),
            "CUSA11111"
        );
    }

    #[test]
    fn detect_title_id_falls_back_to_archive_stem() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(extracted_dir.join("misc")).unwrap();
        let launch_file = extracted_dir.join("misc/eboot.bin");
        fs::write(&launch_file, b"eboot").unwrap();
        let archive = dir.path().join("CUSA33333.zip");

        assert_eq!(
            detect_title_id(&extracted_dir, &launch_file, &archive),
            "CUSA33333"
        );
    }

    // -- select_ps4_launch_file ------------------------------------------------

    #[test]
    fn select_launch_file_prefers_top_level_title_id_dir_even_if_deeper() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(extracted_dir.join("CUSA10000/deep/nested")).unwrap();
        fs::create_dir_all(extracted_dir.join("other")).unwrap();
        let matching = extracted_dir.join("CUSA10000/deep/nested/eboot.bin");
        let shallow_non_matching = extracted_dir.join("other/eboot.bin");
        fs::write(&matching, b"eboot").unwrap();
        fs::write(&shallow_non_matching, b"eboot").unwrap();

        let pool = vec![shallow_non_matching, matching.clone()];
        assert_eq!(
            select_ps4_launch_file(&extracted_dir, &pool),
            Some(matching)
        );
    }

    #[test]
    fn select_launch_file_prefers_shallower_when_neither_matches_top_level() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(extracted_dir.join("a/b")).unwrap();
        fs::create_dir_all(extracted_dir.join("c")).unwrap();
        let deep = extracted_dir.join("a/b/eboot.bin");
        let shallow = extracted_dir.join("c/eboot.bin");
        fs::write(&deep, b"eboot").unwrap();
        fs::write(&shallow, b"eboot").unwrap();

        let pool = vec![deep, shallow.clone()];
        assert_eq!(select_ps4_launch_file(&extracted_dir, &pool), Some(shallow));
    }

    #[test]
    fn select_launch_file_breaks_depth_ties_with_casefolded_path() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(extracted_dir.join("B")).unwrap();
        fs::create_dir_all(extracted_dir.join("a")).unwrap();
        let b_path = extracted_dir.join("B/eboot.bin");
        let a_path = extracted_dir.join("a/eboot.bin");
        fs::write(&b_path, b"eboot").unwrap();
        fs::write(&a_path, b"eboot").unwrap();

        let pool = vec![b_path, a_path.clone()];
        assert_eq!(select_ps4_launch_file(&extracted_dir, &pool), Some(a_path));
    }

    #[test]
    fn select_launch_file_returns_none_without_an_eboot() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(&extracted_dir).unwrap();
        let readme = extracted_dir.join("readme.txt");
        fs::write(&readme, b"not an eboot").unwrap();

        let pool = vec![readme];
        assert_eq!(select_ps4_launch_file(&extracted_dir, &pool), None);
        assert_eq!(select_ps4_launch_file(&extracted_dir, &[]), None);
    }

    // -- expected_title_id -----------------------------------------------------

    #[test]
    fn expected_title_id_prefers_explicit_field() {
        let row = InstalledGame {
            ps4_game_id: "cusa-12345".to_string(),
            ..ps4_row()
        };
        assert_eq!(expected_title_id(&row), "CUSA12345");
    }

    #[test]
    fn expected_title_id_falls_back_to_extracted_path_parents() {
        let row = InstalledGame {
            extracted_path: "/library/PS4/CUSA22222/eboot.bin".to_string(),
            ..ps4_row()
        };
        assert_eq!(expected_title_id(&row), "CUSA22222");
    }

    #[test]
    fn expected_title_id_falls_back_to_extracted_dir_roots() {
        let dir = tempfile::tempdir().unwrap();
        let extracted_dir = dir.path().join("extracted");
        fs::create_dir_all(extracted_dir.join("CUSA33333")).unwrap();
        let row = InstalledGame {
            extracted_dir: extracted_dir.to_string_lossy().into_owned(),
            ..ps4_row()
        };
        assert_eq!(expected_title_id(&row), "CUSA33333");
    }

    #[test]
    fn expected_title_id_empty_when_nothing_detects() {
        assert_eq!(expected_title_id(&ps4_row()), "");
    }

    // -- apply_content ----------------------------------------------------------

    fn setup_install(dir: &Path) -> (InstalledGame, PathBuf) {
        let installed_root = dir.join("installed");
        let target_title_dir = installed_root.join("CUSA12345");
        fs::create_dir_all(&target_title_dir).unwrap();
        let row = InstalledGame {
            ps4_game_id: "CUSA12345".to_string(),
            extracted_dir: installed_root.to_string_lossy().into_owned(),
            ..ps4_row()
        };
        (row, installed_root)
    }

    #[test]
    fn apply_content_happy_path_merges_and_returns_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let (row, installed_root) = setup_install(dir.path());

        let prepared = dir.path().join("prepared");
        fs::create_dir_all(prepared.join("CUSA12345")).unwrap();
        fs::write(prepared.join("CUSA12345/patch.txt"), b"patch data").unwrap();

        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let staging = dir.path().join("staging");

        let extract = copy_extract(&prepared);
        let result =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap();

        assert_eq!(result.game_id, "CUSA12345");
        assert_eq!(result.warning, "");
        assert!(!archive.exists());
        assert!(!staging.exists());
        assert_eq!(
            fs::read_to_string(installed_root.join("CUSA12345/patch.txt")).unwrap(),
            "patch data"
        );

        let entries = read_content_entries(&result.content_json);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].get("kind").map(String::as_str), Some("update"));
        assert_eq!(
            entries[0].get("title_id").map(String::as_str),
            Some("CUSA12345")
        );
        assert_eq!(
            entries[0].get("archive_name").map(String::as_str),
            Some("archive.zip")
        );
        assert!(entries[0].contains_key("applied_at"));
    }

    #[test]
    fn apply_content_rejects_non_ps4_platform() {
        let dir = tempfile::tempdir().unwrap();
        let row = InstalledGame {
            platform: "Nintendo Switch".to_string(),
            ..Default::default()
        };
        let archive = dir.path().join("archive.zip");
        let staging = dir.path().join("staging");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };

        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        assert_eq!(error, "PS4 content apply is only supported for PS4 games");
    }

    #[test]
    fn apply_content_requires_detectable_title_id() {
        let dir = tempfile::tempdir().unwrap();
        let row = ps4_row();
        let archive = dir.path().join("archive.zip");
        let staging = dir.path().join("staging");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };

        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        assert_eq!(error, "Installed PS4 game is missing a detectable title ID");
    }

    #[test]
    fn apply_content_requires_extracted_dir_field() {
        let dir = tempfile::tempdir().unwrap();
        let row = InstalledGame {
            ps4_game_id: "CUSA12345".to_string(),
            ..ps4_row()
        };
        let archive = dir.path().join("archive.zip");
        let staging = dir.path().join("staging");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };

        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        assert_eq!(
            error,
            "Installed PS4 game is missing an extracted install directory"
        );
    }

    #[test]
    fn apply_content_requires_extracted_dir_to_exist() {
        let dir = tempfile::tempdir().unwrap();
        let missing_root = dir.path().join("nowhere");
        let row = InstalledGame {
            ps4_game_id: "CUSA12345".to_string(),
            extracted_dir: missing_root.to_string_lossy().into_owned(),
            ..ps4_row()
        };
        let archive = dir.path().join("archive.zip");
        let staging = dir.path().join("staging");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };

        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        assert_eq!(
            error,
            format!(
                "Installed PS4 directory does not exist: {}",
                missing_root.display()
            )
        );
    }

    #[test]
    fn apply_content_requires_title_dir_to_exist() {
        let dir = tempfile::tempdir().unwrap();
        let installed_root = dir.path().join("installed");
        fs::create_dir_all(&installed_root).unwrap();
        let row = InstalledGame {
            ps4_game_id: "CUSA12345".to_string(),
            extracted_dir: installed_root.to_string_lossy().into_owned(),
            ..ps4_row()
        };
        let archive = dir.path().join("archive.zip");
        let staging = dir.path().join("staging");
        let extract = |_a: &Path, _d: &Path| -> Result<(), LibraryError> { unreachable!() };

        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        let expected_title_dir = installed_root.join("CUSA12345");
        assert_eq!(
            error,
            format!(
                "Installed PS4 title directory was not found: {}",
                expected_title_dir.display()
            )
        );
    }

    #[test]
    fn apply_content_requires_a_title_id_root_in_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let (row, _installed_root) = setup_install(dir.path());

        let prepared = dir.path().join("prepared");
        fs::create_dir_all(&prepared).unwrap();
        fs::write(prepared.join("readme.txt"), b"no roots here").unwrap();

        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let staging = dir.path().join("staging");

        let extract = copy_extract(&prepared);
        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        assert_eq!(
            error,
            "PS4 content archive must include a title-ID root folder"
        );
        assert!(!staging.exists());
    }

    #[test]
    fn apply_content_reports_title_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let (row, _installed_root) = setup_install(dir.path());

        let prepared = dir.path().join("prepared");
        fs::create_dir_all(prepared.join("CUSA00001")).unwrap();

        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let staging = dir.path().join("staging");

        let extract = copy_extract(&prepared);
        let error =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap_err();
        assert_eq!(
            error,
            "PS4 content title ID mismatch: expected CUSA12345, archive contains CUSA00001"
        );
        assert!(!staging.exists());
    }

    #[test]
    fn apply_content_archive_delete_failure_becomes_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let (row, installed_root) = setup_install(dir.path());

        let prepared = dir.path().join("prepared");
        fs::create_dir_all(prepared.join("CUSA12345")).unwrap();
        fs::write(prepared.join("CUSA12345/patch.txt"), b"patch data").unwrap();

        // A directory in place of the archive file: `fs::remove_file` can
        // never remove it, forcing the delete-failure path.
        let archive = dir.path().join("archive.zip");
        fs::create_dir_all(&archive).unwrap();
        let staging = dir.path().join("staging");

        let extract = copy_extract(&prepared);
        let result =
            apply_content(&row, &archive, ContentKind::Update, &staging, &extract).unwrap();

        assert_eq!(result.game_id, "CUSA12345");
        assert!(result.warning.starts_with(&format!(
            "Applied PS4 content, but could not delete archive:\n{}\n",
            archive.display()
        )));
        assert!(archive.exists());
        assert!(
            installed_root.join("CUSA12345/patch.txt").exists(),
            "the merge itself must still have succeeded"
        );
    }

    #[test]
    fn apply_content_returns_game_id_upper_cased() {
        let dir = tempfile::tempdir().unwrap();
        let (row, _installed_root) = setup_install(dir.path());

        let prepared = dir.path().join("prepared");
        fs::create_dir_all(prepared.join("CUSA12345")).unwrap();

        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"zip bytes").unwrap();
        let staging = dir.path().join("staging");

        let extract = copy_extract(&prepared);
        let result = apply_content(&row, &archive, ContentKind::Dlc, &staging, &extract).unwrap();

        assert_eq!(result.game_id, result.game_id.to_uppercase());
    }
}
