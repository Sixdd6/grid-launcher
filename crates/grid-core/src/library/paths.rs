//! Library path rules: component sanitization, archive naming, extraction
//! directory placement, and the on-disk candidate paths a game's records may
//! resolve to. See `docs/porting/03-library-install.md` for the Python
//! behavior this mirrors exactly.

use std::path::{Path, PathBuf};

const ILLEGAL_CHARACTERS: &str = "<>:\"/\\|?*";

/// Top-level directory names in the v1 library layout.
pub const GAMES_DIR: &str = "games";
pub const EMULATORS_DIR: &str = "emulators";
/// The flat layout's emulator install directory name (capitalized, matching
/// the reference app), used only to detect a pre-v1 library root.
pub const LEGACY_EMULATORS_DIR: &str = "Emulators";
pub const SAVES_DIR: &str = "saves";
/// `Config::library_layout_version` for the `games`/`emulators`/`saves` split.
pub const LAYOUT_VERSION_V1: u32 = 1;
/// `Config::library_layout_version` for the DATA ROOT repair: every
/// `user_data` link sits at the directory the emulator really reads, which
/// for PCSX2's AppImage is `<exe dir>/PCSX2` and not the executable's own
/// directory (`autoconfig::paths::pcsx2_data_root`).
pub const LAYOUT_VERSION_V2: u32 = 2;
/// The layout this build writes. A library stamped lower than this runs
/// `library::layout_migration` on the next start.
pub const LAYOUT_VERSION_CURRENT: u32 = LAYOUT_VERSION_V2;

/// Sanitize one path component (a title, platform, or emulator name) for use
/// as a file/directory name.
///
/// Every character in `<>:"/\|?*`, and every control character (code point
/// < 32), becomes `_`. If the *last* character is then a trailing space or
/// dot, it is also replaced with `_` — matching the Python original, which
/// only ever converts a single trailing character (the loop there always
/// terminates after one pass, because `_` is never itself a space or dot).
/// If what remains, once leading/trailing spaces, underscores and dots are
/// stripped, is empty, `fallback` is returned instead.
pub fn sanitize_component(raw: &str, fallback: &str) -> String {
    let mut chars: Vec<char> = raw
        .chars()
        .map(|c| {
            if ILLEGAL_CHARACTERS.contains(c) || (c as u32) < 32 {
                '_'
            } else {
                c
            }
        })
        .collect();
    if matches!(chars.last(), Some(' ') | Some('.')) {
        let last = chars.len() - 1;
        chars[last] = '_';
    }
    let sanitized: String = chars.into_iter().collect();
    if sanitized
        .trim_matches(|c| c == ' ' || c == '_' || c == '.')
        .is_empty()
    {
        fallback.to_string()
    } else {
        sanitized
    }
}

/// Compute the on-disk archive file name for a game.
///
/// `fs_name` is the server's reported file name (e.g. `rom_file_name`),
/// which may use `\` as a path separator; only its last segment is used.
/// When that is empty, falls back to `<safe title>-<safe platform>.zip`.
pub fn archive_name(fs_name: &str, title: &str, platform: &str) -> String {
    let normalized = fs_name.replace('\\', "/");
    let last_segment = normalized.rsplit('/').next().unwrap_or("");
    if !last_segment.is_empty() {
        return last_segment.to_string();
    }
    let safe_title = sanitize_component(title, "game");
    let safe_platform = sanitize_component(platform, "platform");
    format!("{safe_title}-{safe_platform}.zip")
}

/// Compute the extraction directory for an archive: `<parent>/<stem>`,
/// unless that path equals the archive itself or already exists as a file,
/// in which case `<parent>/<stem>_extracted` is used instead.
pub fn extraction_dir(archive: &Path) -> PathBuf {
    let parent = archive.parent().unwrap_or_else(|| Path::new(""));
    let extracted_name = archive
        .file_stem()
        .or_else(|| archive.file_name())
        .unwrap_or_default();
    let candidate = parent.join(extracted_name);
    if candidate == archive || candidate.is_file() {
        let mut fallback_name = extracted_name.to_os_string();
        fallback_name.push("_extracted");
        parent.join(fallback_name)
    } else {
        candidate
    }
}

/// The platform-scoped directory under the library root's `games/` tree
/// (v1 library layout).
pub fn platform_dir(library: &Path, platform: &str) -> PathBuf {
    library
        .join(GAMES_DIR)
        .join(sanitize_component(platform, "Platform"))
}

/// The pre-v1 platform-scoped directory directly under the library root,
/// kept as a fallback so an existing (unmigrated) install is still found.
pub fn legacy_platform_dir(library: &Path, platform: &str) -> PathBuf {
    library.join(sanitize_component(platform, "Platform"))
}

/// `<library>/emulators` (v1 library layout).
pub fn emulators_dir(library: &Path) -> PathBuf {
    library.join(EMULATORS_DIR)
}

/// `<library>/Emulators` — the pre-v1 emulator root, kept as a removal-guard
/// and lookup fallback.
pub fn legacy_emulators_dir(library: &Path) -> PathBuf {
    library.join(LEGACY_EMULATORS_DIR)
}

/// `<library>/saves/<sanitize_component(emulator_name, "emulator")>` — the
/// per-emulator cloud-save directory (v1 library layout).
pub fn saves_dir(library: &Path, emulator_name: &str) -> PathBuf {
    library
        .join(SAVES_DIR)
        .join(sanitize_component(emulator_name, "emulator"))
}

/// Expand a leading `~/` in `raw` to the user's home directory. Any other
/// form (a bare `~`, `~user/...`, or no tilde at all) is left untouched —
/// this is a minimal, manual stand-in for shell tilde expansion, not a full
/// implementation. The home directory comes from
/// [`crate::platform::home_dir`].
pub(crate) fn expand_home(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = crate::platform::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

/// The configured library root with a leading `~/` expanded, or `None` when
/// `Config::library_path` is blank.
///
/// [`expand_home`] is `pub(crate)`, so the app layer cannot reach it; this is
/// the one public way it derives the same root [`crate::library::InstallService`]
/// installs into (its own `library_root` is private and returns
/// `LibraryError::LibraryPathUnset` instead of `None`).
pub fn library_root(config: &crate::config::Config) -> Option<PathBuf> {
    if config.library_path.trim().is_empty() {
        return None;
    }
    Some(expand_home(&config.library_path))
}

/// Decide which library layout a path (new or existing) should use.
///
/// Returns `LAYOUT_VERSION_CURRENT` when `raw` is blank, does not exist, is
/// not a directory, or is empty (no entries other than dot-entries) — i.e.
/// whenever it is safe to start fresh in the current layout, with nothing
/// to repair. Returns `LAYOUT_VERSION_V1` when the directory already has a
/// top-level `games`, `emulators`, or `saves` entry: it is shaped v1, so the
/// v2 data-root step runs on the next start. Returns `0` for any other
/// non-empty directory, which is treated as an existing flat/legacy library
/// root that must not be reorganized silently.
pub fn layout_version_for_library_path(raw: &str) -> u32 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return LAYOUT_VERSION_CURRENT;
    }
    let path = expand_home(trimmed);
    if !path.is_dir() {
        return LAYOUT_VERSION_CURRENT;
    }
    let Ok(entries) = std::fs::read_dir(&path) else {
        return LAYOUT_VERSION_CURRENT;
    };
    let mut saw_entry = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if name == GAMES_DIR || name == EMULATORS_DIR || name == SAVES_DIR {
            return LAYOUT_VERSION_V1;
        }
        saw_entry = true;
    }
    if saw_entry {
        0
    } else {
        LAYOUT_VERSION_CURRENT
    }
}

/// Deduplicate paths by their string form, keeping the first occurrence of
/// each and preserving overall order.
pub(crate) fn dedup_by_string(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|p| seen.insert(p.to_string_lossy().into_owned()))
        .collect()
}

/// The ordered, deduplicated set of archive locations a game might be found
/// at: the recorded `archive_path` (`~`-expanded) first when non-blank, then
/// `<games platform dir>/<archive_name>`, then the legacy (pre-v1)
/// `<platform dir>/<archive_name>`, then `<library>/<archive_name>`.
///
/// This takes plain parameters rather than an `InstalledGame` record because
/// the registry type is introduced in a later task; a `library::registry`
/// wrapper will likely be added on top of this once that type exists.
pub fn candidate_archives(
    library: &Path,
    platform: &str,
    archive_path: &str,
    archive_name: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if !archive_path.trim().is_empty() {
        candidates.push(expand_home(archive_path));
    }
    candidates.push(platform_dir(library, platform).join(archive_name));
    candidates.push(legacy_platform_dir(library, platform).join(archive_name));
    candidates.push(library.join(archive_name));
    dedup_by_string(candidates)
}

/// The ordered, deduplicated set of extraction directories a game might be
/// found at: the recorded `extracted_dir` first when non-blank, then the
/// `extraction_dir()` of every candidate archive path.
pub fn candidate_extracted_dirs(
    archive_candidates: &[PathBuf],
    extracted_dir: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if !extracted_dir.trim().is_empty() {
        candidates.push(PathBuf::from(extracted_dir));
    }
    for archive in archive_candidates {
        candidates.push(extraction_dir(archive));
    }
    dedup_by_string(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    // --- sanitize_component -------------------------------------------

    #[test]
    fn sanitize_replaces_illegal_characters() {
        assert_eq!(sanitize_component("a<b>c", "fallback"), "a_b_c");
    }

    #[test]
    fn sanitize_replaces_all_illegal_character_classes() {
        assert_eq!(
            sanitize_component("a<b>c:d\"e/f\\g|h?i*jZ", "fallback"),
            "a_b_c_d_e_f_g_h_i_jZ"
        );
    }

    #[test]
    fn sanitize_falls_back_when_input_is_only_illegal_characters() {
        assert_eq!(sanitize_component("<>:\"/\\|?*", "fallback"), "fallback");
    }

    #[test]
    fn sanitize_converts_trailing_dot_to_underscore() {
        assert_eq!(sanitize_component("CON.", "fallback"), "CON_");
    }

    #[test]
    fn sanitize_falls_back_when_only_dots() {
        assert_eq!(sanitize_component("...", "fallback"), "fallback");
    }

    #[test]
    fn sanitize_falls_back_when_only_spaces_and_underscores() {
        assert_eq!(sanitize_component("  __  ", "fallback"), "fallback");
    }

    #[test]
    fn sanitize_replaces_control_characters() {
        assert_eq!(sanitize_component("a\u{0001}b", "fallback"), "a_b");
    }

    #[test]
    fn sanitize_handles_titan_ae_trailing_dot_case() {
        assert_eq!(sanitize_component("Titan A.E.", "fallback"), "Titan A.E_");
    }

    #[test]
    fn sanitize_keeps_clean_input_unchanged() {
        assert_eq!(
            sanitize_component("Chrono Trigger", "fallback"),
            "Chrono Trigger"
        );
    }

    // --- archive_name ----------------------------------------------------

    #[test]
    fn archive_name_takes_last_segment_of_backslash_path() {
        assert_eq!(
            archive_name("dir\\sub\\Game.zip", "Some Title", "Some Platform"),
            "Game.zip"
        );
    }

    #[test]
    fn archive_name_takes_last_segment_of_forward_slash_path() {
        assert_eq!(
            archive_name("dir/sub/Game.zip", "Some Title", "Some Platform"),
            "Game.zip"
        );
    }

    #[test]
    fn archive_name_falls_back_to_title_platform_shape_when_empty() {
        assert_eq!(
            archive_name("", "Safe Title", "Safe Platform"),
            "Safe Title-Safe Platform.zip"
        );
    }

    #[test]
    fn archive_name_sanitizes_fallback_components() {
        assert_eq!(
            archive_name("", "Titan A.E.", "Windows"),
            "Titan A.E_-Windows.zip"
        );
    }

    // --- extraction_dir ----------------------------------------------------

    #[test]
    fn extraction_dir_is_parent_join_stem() {
        let archive = Path::new("/library/Platform/Game.zip");
        assert_eq!(
            extraction_dir(archive),
            PathBuf::from("/library/Platform/Game")
        );
    }

    #[test]
    fn extraction_dir_falls_back_when_it_equals_the_archive() {
        // No extension => stem == file name, so parent/stem == the archive
        // itself.
        let archive = Path::new("/library/Platform/Game");
        assert_eq!(
            extraction_dir(archive),
            PathBuf::from("/library/Platform/Game_extracted")
        );
    }

    #[test]
    fn extraction_dir_falls_back_when_it_exists_as_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("Game.zip");
        std::fs::write(&archive, b"archive bytes").unwrap();
        // Something else already occupies the would-be extraction dir, as a
        // plain file rather than a directory.
        std::fs::write(dir.path().join("Game"), b"collision").unwrap();

        assert_eq!(extraction_dir(&archive), dir.path().join("Game_extracted"));
    }

    #[test]
    fn extraction_dir_is_unaffected_when_collision_is_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("Game.zip");
        std::fs::write(&archive, b"archive bytes").unwrap();
        std::fs::create_dir(dir.path().join("Game")).unwrap();

        assert_eq!(extraction_dir(&archive), dir.path().join("Game"));
    }

    // --- platform_dir / legacy_platform_dir -------------------------------

    #[test]
    fn platform_dir_joins_sanitized_platform() {
        let library = Path::new("/library");
        assert_eq!(
            platform_dir(library, "Sony PlayStation"),
            PathBuf::from("/library/games/Sony PlayStation")
        );
    }

    #[test]
    fn platform_dir_sanitizes_illegal_characters() {
        let library = Path::new("/library");
        assert_eq!(
            platform_dir(library, "Arcade: MAME"),
            PathBuf::from("/library/games/Arcade_ MAME")
        );
    }

    #[test]
    fn legacy_platform_dir_is_the_bare_platform_directory() {
        let library = Path::new("/library");
        assert_eq!(
            legacy_platform_dir(library, "Sony PlayStation"),
            PathBuf::from("/library/Sony PlayStation")
        );
    }

    // --- emulators_dir / saves_dir -----------------------------------------

    #[test]
    fn emulators_dir_is_lowercase() {
        let library = Path::new("/library");
        assert_eq!(emulators_dir(library), PathBuf::from("/library/emulators"));
    }

    #[test]
    fn saves_dir_sanitizes_the_emulator_name() {
        let library = Path::new("/library");
        assert_eq!(
            saves_dir(library, "PCSX2: <bad>"),
            PathBuf::from("/library/saves/PCSX2_ _bad_")
        );
    }

    // --- candidate_archives ----------------------------------------------

    #[test]
    fn candidate_archives_orders_archive_path_platform_dir_then_library() {
        let library = Path::new("/library");
        let candidates = candidate_archives(library, "Platform", "/other/Game.zip", "Game.zip");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/other/Game.zip"),
                PathBuf::from("/library/games/Platform/Game.zip"),
                PathBuf::from("/library/Platform/Game.zip"),
                PathBuf::from("/library/Game.zip"),
            ]
        );
    }

    #[test]
    fn candidate_archives_skips_blank_archive_path() {
        let library = Path::new("/library");
        let candidates = candidate_archives(library, "Platform", "  ", "Game.zip");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/library/games/Platform/Game.zip"),
                PathBuf::from("/library/Platform/Game.zip"),
                PathBuf::from("/library/Game.zip"),
            ]
        );
    }

    #[test]
    fn candidate_archives_dedups_by_string() {
        // archive_path already points at the games-platform-dir candidate,
        // so it collapses into one entry instead of appearing twice. The
        // recorded path is built with `Path::join` like the product's own
        // candidate, so its text matches on every OS's separator.
        let library = Path::new("/library");
        let recorded = library.join("games").join("Platform").join("Game.zip");
        let candidates =
            candidate_archives(library, "Platform", recorded.to_str().unwrap(), "Game.zip");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/library/games/Platform/Game.zip"),
                PathBuf::from("/library/Platform/Game.zip"),
                PathBuf::from("/library/Game.zip"),
            ]
        );
    }

    #[test]
    fn candidate_archives_expands_leading_tilde() {
        // `expand_home` reads `$HOME` under test; see `crate::test_env`.
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard =
            crate::test_env::EnvGuard::set(&[("HOME", Some(temp.path().to_str().unwrap()))]);
        let library = Path::new("/library");
        let candidates = candidate_archives(library, "Platform", "~/Games/Game.zip", "Game.zip");
        assert_eq!(candidates[0], temp.path().join("Games/Game.zip"));
    }

    #[test]
    fn candidate_archives_does_not_expand_bare_tilde_without_slash() {
        let library = Path::new("/library");
        let candidates = candidate_archives(library, "Platform", "~backup/Game.zip", "Game.zip");
        assert_eq!(candidates[0], PathBuf::from("~backup/Game.zip"));
    }

    // --- candidate_extracted_dirs -----------------------------------------

    #[test]
    fn candidate_extracted_dirs_puts_extracted_dir_first() {
        let archive_candidates = vec![PathBuf::from("/library/Platform/Game.zip")];
        let candidates = candidate_extracted_dirs(&archive_candidates, "/custom/extracted");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/custom/extracted"),
                PathBuf::from("/library/Platform/Game"),
            ]
        );
    }

    #[test]
    fn candidate_extracted_dirs_skips_blank_extracted_dir() {
        let archive_candidates = vec![
            PathBuf::from("/library/Platform/Game.zip"),
            PathBuf::from("/library/Game.zip"),
        ];
        let candidates = candidate_extracted_dirs(&archive_candidates, "");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/library/Platform/Game"),
                PathBuf::from("/library/Game"),
            ]
        );
    }

    // --- layout_version_for_library_path -----------------------------------

    #[test]
    fn layout_version_for_library_path_table() {
        let dir = tempfile::tempdir().unwrap();
        // Nothing to migrate and nothing to repair: the current layout.
        let current = LAYOUT_VERSION_CURRENT;

        assert_eq!(layout_version_for_library_path(""), current, "blank");
        assert_eq!(
            layout_version_for_library_path(&dir.path().join("nope").to_string_lossy()),
            current,
            "nonexistent"
        );

        let file_path = dir.path().join("a_file");
        std::fs::write(&file_path, b"x").unwrap();
        assert_eq!(
            layout_version_for_library_path(&file_path.to_string_lossy()),
            current,
            "a file path"
        );

        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(
            layout_version_for_library_path(&empty.to_string_lossy()),
            current,
            "empty temp dir"
        );

        let only_hidden = dir.path().join("only_hidden");
        std::fs::create_dir(&only_hidden).unwrap();
        std::fs::write(only_hidden.join(".hidden"), b"x").unwrap();
        assert_eq!(
            layout_version_for_library_path(&only_hidden.to_string_lossy()),
            current,
            "dir with only .hidden"
        );

        // Already v1-shaped: the v2 data-root step still has to run.
        let with_games = dir.path().join("with_games");
        std::fs::create_dir(&with_games).unwrap();
        std::fs::create_dir(with_games.join("games")).unwrap();
        assert_eq!(
            layout_version_for_library_path(&with_games.to_string_lossy()),
            LAYOUT_VERSION_V1,
            "dir with games/"
        );

        let with_emulators = dir.path().join("with_emulators");
        std::fs::create_dir(&with_emulators).unwrap();
        std::fs::create_dir(with_emulators.join("emulators")).unwrap();
        assert_eq!(
            layout_version_for_library_path(&with_emulators.to_string_lossy()),
            LAYOUT_VERSION_V1,
            "dir with emulators/"
        );

        let with_saves = dir.path().join("with_saves");
        std::fs::create_dir(&with_saves).unwrap();
        std::fs::create_dir(with_saves.join("saves")).unwrap();
        assert_eq!(
            layout_version_for_library_path(&with_saves.to_string_lossy()),
            LAYOUT_VERSION_V1,
            "dir with saves/ only"
        );

        let with_legacy_emulators = dir.path().join("with_legacy_emulators");
        std::fs::create_dir(&with_legacy_emulators).unwrap();
        std::fs::create_dir(with_legacy_emulators.join("Emulators")).unwrap();
        assert_eq!(
            layout_version_for_library_path(&with_legacy_emulators.to_string_lossy()),
            0,
            "dir with Emulators/"
        );

        let with_platform = dir.path().join("with_platform");
        std::fs::create_dir(&with_platform).unwrap();
        std::fs::create_dir(with_platform.join("Sony PlayStation 2")).unwrap();
        assert_eq!(
            layout_version_for_library_path(&with_platform.to_string_lossy()),
            0,
            "dir with Sony PlayStation 2/"
        );

        let with_loose_archive = dir.path().join("with_loose_archive");
        std::fs::create_dir(&with_loose_archive).unwrap();
        std::fs::write(with_loose_archive.join("foo.zip"), b"x").unwrap();
        assert_eq!(
            layout_version_for_library_path(&with_loose_archive.to_string_lossy()),
            0,
            "dir with a loose foo.zip file and no directories"
        );
    }

    #[test]
    fn the_current_layout_version_is_v2() {
        assert_eq!(LAYOUT_VERSION_CURRENT, LAYOUT_VERSION_V2);
        assert_eq!(LAYOUT_VERSION_V2, 2);
    }

    #[test]
    fn candidate_extracted_dirs_dedups_by_string() {
        let archive_candidates = vec![
            PathBuf::from("/library/Platform/Game.zip"),
            PathBuf::from("/other/Game.zip"),
        ];
        // Built with `Path::join` like `extraction_dir`, so the recorded
        // text matches the derived candidate on every OS's separator.
        let recorded = Path::new("/library/Platform").join("Game");
        let candidates = candidate_extracted_dirs(&archive_candidates, recorded.to_str().unwrap());
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/library/Platform/Game"),
                PathBuf::from("/other/Game"),
            ]
        );
    }
}
