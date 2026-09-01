//! Library path rules: component sanitization, archive naming, extraction
//! directory placement, and the on-disk candidate paths a game's records may
//! resolve to. See `docs/porting/03-library-install.md` for the Python
//! behavior this mirrors exactly.

use std::path::{Path, PathBuf};

const ILLEGAL_CHARACTERS: &str = "<>:\"/\\|?*";

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

/// The platform-scoped directory under the library root.
pub fn platform_dir(library: &Path, platform: &str) -> PathBuf {
    library.join(sanitize_component(platform, "Platform"))
}

/// Expand a leading `~/` in `raw` to the user's home directory. Any other
/// form (a bare `~`, `~user/...`, or no tilde at all) is left untouched —
/// this is a minimal, manual stand-in for shell tilde expansion, not a full
/// implementation.
fn expand_home(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            return base_dirs.home_dir().join(rest);
        }
    }
    PathBuf::from(raw)
}

/// Deduplicate paths by their string form, keeping the first occurrence of
/// each and preserving overall order.
fn dedup_by_string(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|p| seen.insert(p.to_string_lossy().into_owned()))
        .collect()
}

/// The ordered, deduplicated set of archive locations a game might be found
/// at: the recorded `archive_path` (`~`-expanded) first when non-blank,
/// then `<platform dir>/<archive_name>`, then `<library>/<archive_name>`.
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

    // --- platform_dir ----------------------------------------------------

    #[test]
    fn platform_dir_joins_sanitized_platform() {
        let library = Path::new("/library");
        assert_eq!(
            platform_dir(library, "Sony PlayStation"),
            PathBuf::from("/library/Sony PlayStation")
        );
    }

    #[test]
    fn platform_dir_sanitizes_illegal_characters() {
        let library = Path::new("/library");
        assert_eq!(
            platform_dir(library, "Arcade: MAME"),
            PathBuf::from("/library/Arcade_ MAME")
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
                PathBuf::from("/library/Platform/Game.zip"),
                PathBuf::from("/library/Game.zip"),
            ]
        );
    }

    #[test]
    fn candidate_archives_dedups_by_string() {
        // library/Platform/Game.zip and library/Game.zip collapse to the
        // same string when the platform sanitizes to empty-ish... use an
        // explicit duplicate instead: archive_path already points at the
        // platform-dir candidate.
        let library = Path::new("/library");
        let candidates = candidate_archives(
            library,
            "Platform",
            "/library/Platform/Game.zip",
            "Game.zip",
        );
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/library/Platform/Game.zip"),
                PathBuf::from("/library/Game.zip"),
            ]
        );
    }

    #[test]
    fn candidate_archives_expands_leading_tilde() {
        let library = Path::new("/library");
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        let candidates = candidate_archives(library, "Platform", "~/Games/Game.zip", "Game.zip");
        assert_eq!(candidates[0], home.join("Games/Game.zip"));
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

    #[test]
    fn candidate_extracted_dirs_dedups_by_string() {
        let archive_candidates = vec![
            PathBuf::from("/library/Platform/Game.zip"),
            PathBuf::from("/other/Game.zip"),
        ];
        let candidates = candidate_extracted_dirs(&archive_candidates, "/library/Platform/Game");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/library/Platform/Game"),
                PathBuf::from("/other/Game"),
            ]
        );
    }
}
