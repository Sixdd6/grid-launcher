//! The shared-owner install-path last resort: find the library game that
//! IS the emulator binary at a given path.
//!
//! Ported from `grid_launcher/library/install_registry.py:65`
//! (`matching_installed_emulator_games`), the three candidate builders in
//! `grid_launcher/library/install_paths.py:19-89`, and the two path
//! comparisons in `grid_launcher/core/path.py:15-31`. Used by
//! `cloud::ops::shared_cloud_sync_owner` when the free-text owner search
//! (`cloud::scope::shared_sync_owner`) finds nothing. See
//! `docs/porting/06-cloud-saves.md` ("Shared save owner").

use std::path::{Path, PathBuf};

use crate::library::launch_select::select_launch_file;
use crate::library::paths::{archive_name, expand_home, extraction_dir, platform_dir};

use super::scope::is_emulators_platform;
use super::CloudGame;

/// `path_key` (path.py:15-21): `~`-expand, resolve without requiring the
/// path to exist, case-fold. `Path::canonicalize` is the resolve step and
/// fails for missing paths, where Python's `resolve(strict=False)` would
/// still normalize; falling back to the expanded path (what Python does on
/// `OSError`) keeps non-existent candidates comparable by their literal
/// text, which is all the callers below need.
fn path_key(path: &Path) -> String {
    let expanded = expand_home(&path.to_string_lossy());
    let resolved = expanded.canonicalize().unwrap_or(expanded);
    resolved.to_string_lossy().to_lowercase()
}

/// `path_within_path` (path.py:24-31): equal keys, or `path`'s key starts
/// with `root`'s key (trailing separators stripped) plus a separator. An
/// empty root key never contains anything.
fn path_within_path(path: &Path, root: &Path) -> bool {
    let target = path_key(path);
    let root_key = path_key(root).trim_end_matches(['/', '\\']).to_string();
    if root_key.is_empty() {
        return false;
    }
    target == root_key
        || target.starts_with(&format!("{root_key}/"))
        || target.starts_with(&format!("{root_key}\\"))
}

/// Keep the first occurrence of each path, by string form.
fn dedup(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|p| seen.insert(p.to_string_lossy().into_owned()))
        .collect()
}

/// `candidate_archive_paths_for_game` (install_paths.py:19-43): the
/// recorded `archive_path`, `<platform dir>/<archive name>`,
/// `<library>/<archive name>`, then `<native game dir>/<archive name>`.
/// The two library-relative entries are skipped when `library` is `None`,
/// matching Python's `platform_library_dir()`/`library_path_dir()`
/// returning `None` on an unset library path.
fn archive_candidates(game: &CloudGame, library: Option<&Path>) -> Vec<PathBuf> {
    let name = archive_name(&game.rom_file_name, &game.title, &game.platform);
    let mut candidates = Vec::new();
    if !game.archive_path.trim().is_empty() {
        candidates.push(expand_home(&game.archive_path));
    }
    if let Some(library) = library {
        candidates.push(platform_dir(library, &game.platform).join(&name));
        candidates.push(library.join(&name));
    }
    if !game.native_game_dir.trim().is_empty() {
        candidates.push(expand_home(&game.native_game_dir).join(&name));
    }
    dedup(candidates)
}

/// `candidate_extracted_paths_for_game` (install_paths.py:46-65): the
/// launch file selected under an existing `extracted_dir` (archive stem
/// taken from `archive_path`, literal `"archive"` when blank), then
/// `extracted_path` when it exists as a file. Both are existence-gated in
/// the reference, so a missing directory contributes nothing.
fn extracted_file_candidates(game: &CloudGame) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if !game.extracted_dir.trim().is_empty() {
        let dir = expand_home(&game.extracted_dir);
        if dir.is_dir() {
            let archive = if game.archive_path.trim().is_empty() {
                PathBuf::from("archive")
            } else {
                PathBuf::from(&game.archive_path)
            };
            let stem = archive
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(selected) = select_launch_file(&dir, &stem) {
                candidates.push(selected);
            }
        }
    }
    if !game.extracted_path.trim().is_empty() {
        let path = expand_home(&game.extracted_path);
        if path.is_file() {
            candidates.push(path);
        }
    }
    dedup(candidates)
}

/// `candidate_extracted_dirs_for_game` (install_paths.py:68-89): the
/// recorded `extracted_dir`, then for every archive candidate its
/// extraction directory plus, when the game has a `native_game_dir`, that
/// same directory name under it.
fn extracted_dir_candidates(game: &CloudGame, archives: &[PathBuf]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if !game.extracted_dir.trim().is_empty() {
        candidates.push(expand_home(&game.extracted_dir));
    }
    let native_game_dir =
        (!game.native_game_dir.trim().is_empty()).then(|| expand_home(&game.native_game_dir));
    for archive in archives {
        let extracted = extraction_dir(archive);
        if let (Some(native), Some(name)) = (native_game_dir.as_ref(), extracted.file_name()) {
            candidates.push(extracted.clone());
            candidates.push(native.join(name));
        } else {
            candidates.push(extracted);
        }
    }
    dedup(candidates)
}

/// `matching_installed_emulator_games` (install_registry.py:65): every
/// `Emulators`-platform game in `games` whose own install paths ARE
/// `emulator_path` — either one of its archive/extracted FILE candidates
/// resolves to the same path, or `emulator_path` lives inside one of its
/// extracted DIRECTORY candidates. Order follows `games`.
///
/// `library` is the configured library root
/// ([`crate::library::paths::library_root`]), `None` when the library path
/// is unset.
pub fn matching_installed_emulator_games<'a>(
    games: &'a [CloudGame],
    emulator_path: &Path,
    library: Option<&Path>,
) -> Vec<&'a CloudGame> {
    let target_key = path_key(emulator_path);
    let mut matches = Vec::new();
    for game in games {
        if !is_emulators_platform(&game.platform) {
            continue;
        }
        let archives = archive_candidates(game, library);
        let files = archives
            .iter()
            .cloned()
            .chain(extracted_file_candidates(game))
            .collect::<Vec<_>>();
        if files.iter().any(|c| path_key(c) == target_key) {
            matches.push(game);
            continue;
        }
        if extracted_dir_candidates(game, &archives)
            .iter()
            .any(|c| path_within_path(emulator_path, c))
        {
            matches.push(game);
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn emulator_game(title: &str) -> CloudGame {
        CloudGame {
            title: title.to_string(),
            platform: "Emulators".to_string(),
            rom_id: "42".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn matches_the_recorded_archive_path() {
        let root = TempDir::new().unwrap();
        let archive = root.path().join("xemu.AppImage");
        fs::write(&archive, b"").unwrap();

        let mut game = emulator_game("xemu build");
        game.archive_path = archive.to_string_lossy().into_owned();
        let games = vec![game];

        assert_eq!(
            matching_installed_emulator_games(&games, &archive, None),
            vec![&games[0]]
        );
        assert!(
            matching_installed_emulator_games(&games, &root.path().join("other"), None).is_empty()
        );
    }

    #[test]
    fn matches_the_platform_and_library_archive_names_only_with_a_library() {
        let root = TempDir::new().unwrap();
        let library = root.path().join("library");
        let mut game = emulator_game("xemu build");
        game.rom_file_name = "xemu.AppImage".to_string();
        let games = vec![game];

        let in_platform_dir = library.join("Emulators").join("xemu.AppImage");
        assert_eq!(
            matching_installed_emulator_games(&games, &in_platform_dir, Some(&library)),
            vec![&games[0]]
        );
        assert_eq!(
            matching_installed_emulator_games(
                &games,
                &library.join("xemu.AppImage"),
                Some(&library)
            ),
            vec![&games[0]]
        );
        // Python's platform_library_dir()/library_path_dir() return None
        // when the library path is unset: no candidates, no match.
        assert!(matching_installed_emulator_games(&games, &in_platform_dir, None).is_empty());
    }

    #[test]
    fn matches_the_selected_launch_file_under_the_extracted_dir() {
        let root = TempDir::new().unwrap();
        let extracted = root.path().join("xemu");
        fs::create_dir_all(&extracted).unwrap();
        let binary = extracted.join("xemu");
        fs::write(&binary, b"").unwrap();
        fs::write(extracted.join("readme.txt"), b"").unwrap();

        let mut game = emulator_game("xemu build");
        game.extracted_dir = extracted.to_string_lossy().into_owned();
        game.archive_path = root.path().join("xemu.zip").to_string_lossy().into_owned();
        let games = vec![game];

        assert_eq!(
            matching_installed_emulator_games(&games, &binary, None),
            vec![&games[0]],
            "the launch file selected under extracted_dir is a file candidate"
        );
    }

    #[test]
    fn matches_an_emulator_nested_under_an_extracted_dir() {
        let root = TempDir::new().unwrap();
        let extracted = root.path().join("xemu");
        fs::create_dir_all(extracted.join("bin")).unwrap();
        let nested = extracted.join("bin").join("xemu");
        fs::write(&nested, b"").unwrap();

        let mut game = emulator_game("xemu build");
        game.extracted_dir = extracted.to_string_lossy().into_owned();
        let games = vec![game];

        assert_eq!(
            matching_installed_emulator_games(&games, &nested, None),
            vec![&games[0]]
        );
        // A sibling directory sharing a name prefix is NOT within it.
        let sibling = root.path().join("xemu-old").join("xemu");
        assert!(matching_installed_emulator_games(&games, &sibling, None).is_empty());
    }

    #[test]
    fn matches_the_extraction_dir_of_a_candidate_archive() {
        let root = TempDir::new().unwrap();
        let mut game = emulator_game("xemu build");
        game.archive_path = root.path().join("xemu.zip").to_string_lossy().into_owned();
        let games = vec![game];

        // extraction_dir("<root>/xemu.zip") == "<root>/xemu".
        let nested = root.path().join("xemu").join("xemu");
        assert_eq!(
            matching_installed_emulator_games(&games, &nested, None),
            vec![&games[0]]
        );
    }

    #[test]
    fn matches_under_the_native_game_dir() {
        let root = TempDir::new().unwrap();
        let mut game = emulator_game("xemu build");
        game.rom_file_name = "xemu.zip".to_string();
        game.native_game_dir = root.path().to_string_lossy().into_owned();
        let games = vec![game];

        // <native_game_dir>/<archive name> as a file candidate ...
        assert_eq!(
            matching_installed_emulator_games(&games, &root.path().join("xemu.zip"), None),
            vec![&games[0]]
        );
        // ... and <native_game_dir>/<extraction dir name> as a dir one.
        assert_eq!(
            matching_installed_emulator_games(&games, &root.path().join("xemu").join("run"), None),
            vec![&games[0]]
        );
    }

    #[test]
    fn compares_case_insensitively_and_expands_a_leading_tilde() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        let mut game = emulator_game("xemu build");
        game.archive_path = "~/GridLauncherTest/XEMU.AppImage".to_string();
        let games = vec![game];

        assert_eq!(
            matching_installed_emulator_games(
                &games,
                &home.join("GridLauncherTest/xemu.appimage"),
                None
            ),
            vec![&games[0]],
            "~ expands and the comparison is case-folded"
        );
    }

    #[test]
    fn ignores_games_that_are_not_on_the_emulators_platform() {
        let root = TempDir::new().unwrap();
        let archive = root.path().join("xemu.AppImage");
        let mut game = emulator_game("xemu build");
        game.platform = "Xbox".to_string();
        game.archive_path = archive.to_string_lossy().into_owned();
        let games = vec![game];

        assert!(matching_installed_emulator_games(&games, &archive, None).is_empty());
    }

    #[test]
    fn returns_every_match_in_pool_order() {
        let root = TempDir::new().unwrap();
        let archive = root.path().join("xemu.AppImage");
        let mut first = emulator_game("first");
        first.archive_path = archive.to_string_lossy().into_owned();
        let mut second = emulator_game("second");
        second.archive_path = archive.to_string_lossy().into_owned();
        let games = vec![first, second];

        assert_eq!(
            matching_installed_emulator_games(&games, &archive, None),
            vec![&games[0], &games[1]]
        );
    }
}
