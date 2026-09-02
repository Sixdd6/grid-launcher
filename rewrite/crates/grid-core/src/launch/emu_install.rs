//! Emulator-install naming and executable selection. Ports
//! `emulator_install_directory` and `select_emulator_executable_path`
//! (`grid_launcher/emulator/autoconfig.py:14-87`), the archive/supplemental
//! naming helpers (`grid_launcher/background/workers.py:147-163` and
//! `grid_launcher/ui/mixins/emulator_ui_mixin.py:1176-1190`), and
//! `launchable_emulator_file` (`grid_launcher/emulator/launch.py:27-28`).
//! See `docs/porting/04-emulator-launch.md` §12.
//!
//! Parity note: several catalog `source` blocks carry a `launch_executable`
//! key. The Python reference never reads it — executable choice is only the
//! scoring ported here — so this module never reads it either.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::library::paths::sanitize_component;

/// `<library>/Emulators/<sanitize_component(archive_stem, "emulator")>`
/// (`emulator_install_directory`, autoconfig.py:14-17).
pub fn emulator_install_dir(library: &Path, archive_stem: &str) -> PathBuf {
    library
        .join("Emulators")
        .join(sanitize_component(archive_stem, "emulator"))
}

/// Splits `name` (a bare file name, no directory component) into
/// `(stem, suffix)` matching `pathlib.Path.stem` / `.suffix`: the suffix is
/// the run of characters from the last `.` onward, but only when that `.`
/// is neither the first nor the last character — so `.hidden`, `noext`, and
/// `trailing.` all have an empty suffix.
fn split_suffix(name: &str) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    let n = chars.len();
    let split_at = match chars.iter().rposition(|&c| c == '.') {
        Some(i) if i > 0 && i < n - 1 => i,
        _ => n,
    };
    (
        chars[..split_at].iter().collect(),
        chars[split_at..].iter().collect(),
    )
}

/// `Path(name).suffix` (see [`split_suffix`]).
fn suffix_of(name: &str) -> String {
    split_suffix(name).1
}

/// `Path(name).with_suffix(new_suffix)`: `name`'s stem, followed by
/// `new_suffix` verbatim.
fn with_suffix(name: &str, new_suffix: &str) -> String {
    format!("{}{new_suffix}", split_suffix(name).0)
}

/// The base archive file name for a source-catalog install
/// (`_build_source_emulator_install_game`'s `_archive_name_override`,
/// emulator_ui_mixin.py:1187-1189), then rewritten to match `asset_name`'s
/// suffix (`_archive_path_with_asset_suffix`, workers.py:153-163).
pub fn archive_file_name(profile_name: &str, tag: &str, asset_name: &str) -> String {
    let base = format!(
        "{}-{}.zip",
        sanitize_component(profile_name, "emulator"),
        sanitize_component(tag, "latest")
    );
    apply_asset_suffix(&base, asset_name)
}

/// `_archive_path_with_asset_suffix` (workers.py:153-163), operating on a
/// bare file name rather than a full path.
fn apply_asset_suffix(base: &str, asset_name: &str) -> String {
    if asset_name.is_empty() {
        return base.to_string();
    }
    if asset_name.to_lowercase().ends_with(".appimage") {
        return asset_name.to_string();
    }
    let asset_suffix = suffix_of(asset_name);
    if asset_suffix.is_empty() {
        return base.to_string();
    }
    if suffix_of(base).to_lowercase() == asset_suffix.to_lowercase() {
        return base.to_string();
    }
    with_suffix(base, &asset_suffix)
}

/// A supplemental download's file name, alongside `primary`'s (already
/// asset-suffix-rewritten) archive name (`_supplemental_archive_path`,
/// workers.py:147-151). `index` is 1-based.
pub fn supplemental_file_name(primary: &Path, index: usize, asset_name: &str) -> String {
    let primary_name = primary
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let primary_stem = split_suffix(&primary_name).0;

    if asset_name.to_lowercase().ends_with(".appimage") {
        return format!("{primary_stem}-supplemental-{index}-{asset_name}");
    }
    let mut suffix = suffix_of(asset_name);
    if suffix.is_empty() {
        suffix = suffix_of(&primary_name);
    }
    if suffix.is_empty() {
        suffix = ".zip".to_string();
    }
    format!("{primary_stem}-supplemental-{index}{suffix}")
}

/// Whether `path` is launchable as an emulator binary: its suffix,
/// casefolded, is one of `.exe .bat .cmd .ps1 .sh .appimage`
/// (`launchable_emulator_file`, launch.py:27-28).
pub fn launchable_emulator_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    matches!(
        suffix_of(&name).to_lowercase().as_str(),
        ".exe" | ".bat" | ".cmd" | ".ps1" | ".sh" | ".appimage"
    )
}

/// `title`, trimmed, casefolded, and split on runs of non-`[a-z0-9]`
/// characters, keeping tokens longer than 2 characters
/// (`select_emulator_executable_path`, autoconfig.py:26-27).
fn title_tokens(title_casefold: &str) -> Vec<String> {
    title_casefold
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|token| token.chars().count() > 2)
        .map(str::to_string)
        .collect()
}

/// Recursively collects every launchable file under `dir` (`rglob("*")`
/// filtered by [`launchable_emulator_file`], autoconfig.py:59-62).
/// Directory and file symlinks are followed, matching `rglob`; an unreadable
/// directory contributes nothing rather than failing the walk.
fn collect_launchable_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => collect_launchable_files(&path, out),
            Ok(meta) if meta.is_file() && launchable_emulator_file(&path) => out.push(path),
            _ => {}
        }
    }
}

/// Sort key for [`select_executable`]'s candidate scoring: lower wins on
/// each field in turn — (preferred-name 0/1, negated token-hit count,
/// `.exe`-preference 0/1, path component count, casefolded path string).
type ExecutableRank = (u8, i64, u8, usize, String);

fn score_candidate(
    candidate: &Path,
    preferred_names: &HashSet<&str>,
    tokens: &[String],
) -> ExecutableRank {
    let file_name = candidate
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file_name_lower = file_name.to_lowercase();
    let preferred_name = u8::from(!preferred_names.contains(file_name_lower.as_str()));

    let (stem, suffix) = split_suffix(&file_name);
    let candidate_name = stem.to_lowercase();
    let token_hits = tokens
        .iter()
        .filter(|token| candidate_name.contains(token.as_str()))
        .count() as i64;
    let preferred_binary = u8::from(suffix.to_lowercase() != ".exe");

    let component_count = candidate.components().count();
    let path_lower = candidate.to_string_lossy().to_lowercase();

    (
        preferred_name,
        -token_hits,
        preferred_binary,
        component_count,
        path_lower,
    )
}

/// Picks the emulator executable for a fresh install
/// (`select_emulator_executable_path`, autoconfig.py:19-87), specialized to
/// this pipeline: there is no separately tracked `extracted_path`, only an
/// `extracted_dir` (here, `install_dir`) and the original `archive`. When
/// `install_dir` exists, every launchable file under it is scored and the
/// lowest-ranked one wins; otherwise (or when it has no launchable files),
/// `archive` itself is used if it is a launchable file. `None` when neither
/// yields a candidate.
pub fn select_executable(title: &str, install_dir: &Path, archive: &Path) -> Option<PathBuf> {
    let title_casefold = title.trim().to_lowercase();
    let tokens = title_tokens(&title_casefold);

    let mut preferred_names: HashSet<&str> = HashSet::new();
    if title_casefold.contains("nintendo switch") || title_casefold.contains("switch") {
        preferred_names.insert("eden.exe");
    }
    if title_casefold.contains("nintendo 3ds") || title_casefold.contains("3ds") {
        preferred_names.insert("azahar.exe");
    }

    if install_dir.is_dir() {
        let mut candidates = Vec::new();
        collect_launchable_files(install_dir, &mut candidates);
        if !candidates.is_empty() {
            return candidates.into_iter().min_by(|a, b| {
                score_candidate(a, &preferred_names, &tokens).cmp(&score_candidate(
                    b,
                    &preferred_names,
                    &tokens,
                ))
            });
        }
    }

    if archive.is_file() && launchable_emulator_file(archive) {
        return Some(archive.to_path_buf());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    // --- emulator_install_dir -------------------------------------------

    #[test]
    fn install_dir_joins_library_emulators_and_sanitized_stem() {
        let dir = emulator_install_dir(Path::new("/lib"), "PCSX2");
        assert_eq!(dir, Path::new("/lib/Emulators/PCSX2"));
    }

    #[test]
    fn install_dir_sanitizes_illegal_characters() {
        let dir = emulator_install_dir(Path::new("/lib"), "Emu: <bad>*chars");
        assert_eq!(dir, Path::new("/lib/Emulators/Emu_ _bad__chars"));
    }

    // --- archive_file_name -------------------------------------------------

    #[test]
    fn archive_file_name_naming_table() {
        let cases: &[(&str, &str, &str, &str)] = &[
            // No asset: base name as-is.
            ("PCSX2", "v2.1.0", "", "PCSX2-v2.1.0.zip"),
            // AppImage asset: whole-name replace.
            (
                "eden",
                "nightly",
                "eden-linux-0.0.5-amd64-clang-pgo.AppImage",
                "eden-linux-0.0.5-amd64-clang-pgo.AppImage",
            ),
            // Asset with no suffix: base unchanged.
            ("Dolphin", "5.0", "dolphin-linux-x64", "Dolphin-5.0.zip"),
            // Asset suffix casefold matches base suffix: base unchanged.
            ("RPCS3", "v1", "rpcs3-linux.ZIP", "RPCS3-v1.zip"),
            // Different suffix: base with suffix replaced. Pinned row: a
            // `.tar.gz` asset's Python `Path.suffix` is only `.gz` (the
            // last dot-separated segment), so this is correct-by-parity
            // even though extraction still sniffs gzip.
            (
                "Redream (Sega Dreamcast)",
                "nightly",
                "redream.x86_64-linux-v1.5.0-1000-gabc.tar.gz",
                "Redream (Sega Dreamcast)-nightly.gz",
            ),
        ];
        for (profile_name, tag, asset_name, expected) in cases {
            assert_eq!(
                archive_file_name(profile_name, tag, asset_name),
                *expected,
                "profile_name={profile_name:?} tag={tag:?} asset_name={asset_name:?}"
            );
        }
    }

    // --- supplemental_file_name ---------------------------------------------

    #[test]
    fn supplemental_file_name_naming_table() {
        let primary = Path::new("/lib/Emulators/PCSX2/PCSX2-v2.1.0.zip");
        let cases: &[(usize, &str, &str)] = &[
            // AppImage form: primary stem + asset name verbatim.
            (
                1,
                "extra-linux.AppImage",
                "PCSX2-v2.1.0-supplemental-1-extra-linux.AppImage",
            ),
            // Asset has its own suffix: use it.
            (2, "bios.bin", "PCSX2-v2.1.0-supplemental-2.bin"),
            // Asset has no suffix: fall back to primary's suffix.
            (3, "biosnosuffix", "PCSX2-v2.1.0-supplemental-3.zip"),
        ];
        for (index, asset_name, expected) in cases {
            assert_eq!(
                supplemental_file_name(primary, *index, asset_name),
                *expected,
                "index={index} asset_name={asset_name:?}"
            );
        }
    }

    #[test]
    fn supplemental_file_name_falls_back_to_zip_when_neither_has_a_suffix() {
        let primary = Path::new("/lib/Emulators/PCSX2/PCSX2-nosuffix");
        assert_eq!(
            supplemental_file_name(primary, 1, "nosuffix"),
            "PCSX2-nosuffix-supplemental-1.zip"
        );
    }

    // --- launchable_emulator_file --------------------------------------------

    #[test]
    fn launchable_emulator_file_suffix_table() {
        let launchable = [
            "a.exe",
            "a.EXE",
            "a.bat",
            "a.cmd",
            "a.ps1",
            "a.sh",
            "a.AppImage",
            "a.appimage",
        ];
        for name in launchable {
            assert!(
                launchable_emulator_file(Path::new(name)),
                "expected {name:?} to be launchable"
            );
        }
        let not_launchable = ["a.zip", "a.txt", "a", ".hidden", "a."];
        for name in not_launchable {
            assert!(
                !launchable_emulator_file(Path::new(name)),
                "expected {name:?} to not be launchable"
            );
        }
    }

    // --- select_executable ---------------------------------------------------

    #[test]
    fn select_executable_picks_highest_token_hit_over_unrelated_file() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("pcsx2-qt.exe"));
        touch(&install_dir.join("updater.sh"));

        let picked = select_executable(
            "PCSX2 (Playstation 2)",
            &install_dir,
            &dir.path().join("archive.zip"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("pcsx2-qt.exe"));
    }

    #[test]
    fn select_executable_prefers_eden_exe_for_switch_title_even_against_higher_token_hits() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        // "switch" token hits this file, but eden.exe is the preferred name
        // for a Switch title and wins regardless.
        touch(&install_dir.join("switch-launcher.sh"));
        touch(&install_dir.join("eden.exe"));

        let picked = select_executable(
            "Super Mario Odyssey (Nintendo Switch)",
            &install_dir,
            &dir.path().join("archive.zip"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("eden.exe"));
    }

    #[test]
    fn select_executable_prefers_exe_suffix_on_a_tie() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("dolphin.sh"));
        touch(&install_dir.join("dolphin.exe"));

        let picked =
            select_executable("Dolphin", &install_dir, &dir.path().join("archive.zip")).unwrap();
        assert_eq!(picked, install_dir.join("dolphin.exe"));
    }

    #[test]
    fn select_executable_prefers_shallower_path_on_a_tie() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("nested").join("emu.sh"));
        touch(&install_dir.join("emu.sh"));

        let picked =
            select_executable("Emu", &install_dir, &dir.path().join("archive.zip")).unwrap();
        assert_eq!(picked, install_dir.join("emu.sh"));
    }

    #[test]
    fn select_executable_breaks_remaining_ties_on_casefolded_path() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("Zeta.sh"));
        touch(&install_dir.join("alpha.sh"));

        let picked =
            select_executable("Emu", &install_dir, &dir.path().join("archive.zip")).unwrap();
        assert_eq!(picked, install_dir.join("alpha.sh"));
    }

    #[test]
    fn select_executable_finds_appimage_only_directory() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("MyEmu-x86_64.AppImage"));
        touch(&install_dir.join("readme.txt"));

        let picked =
            select_executable("MyEmu", &install_dir, &dir.path().join("archive.zip")).unwrap();
        assert_eq!(picked, install_dir.join("MyEmu-x86_64.AppImage"));
    }

    #[test]
    fn select_executable_falls_back_to_archive_when_install_dir_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("does-not-exist");
        let archive = dir.path().join("Emu.AppImage");
        touch(&archive);

        let picked = select_executable("Emu", &install_dir, &archive).unwrap();
        assert_eq!(picked, archive);
    }

    #[test]
    fn select_executable_falls_back_to_archive_when_install_dir_has_no_launchable_files() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("readme.txt"));
        let archive = dir.path().join("Emu.sh");
        touch(&archive);

        let picked = select_executable("Emu", &install_dir, &archive).unwrap();
        assert_eq!(picked, archive);
    }

    #[test]
    fn select_executable_none_when_nothing_launchable() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("readme.txt"));
        let archive = dir.path().join("archive.zip");
        touch(&archive);

        assert!(select_executable("Emu", &install_dir, &archive).is_none());
    }
}
