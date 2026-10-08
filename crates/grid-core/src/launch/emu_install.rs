//! Emulator-install naming and executable selection. Ports
//! `emulator_install_directory` and `select_emulator_executable_path`
//! (`grid_launcher/emulator/autoconfig.py:14-87`), the archive/supplemental
//! naming helpers (`grid_launcher/background/workers.py:147-163` and
//! `grid_launcher/ui/mixins/emulator_ui_mixin.py:1176-1190`), and
//! `launchable_emulator_file` (`grid_launcher/emulator/launch.py:27-28`).
//! See `docs/porting/04-emulator-launch.md` §12.
//!
//! Deviation note: executable selection also accepts an extensionless file
//! whose executable bit is set (unix only) — see [`launchable_installed_file`].
//!
//! Parity note: the catalog no longer carries a `launch_executable` key (the
//! Python reference never read it either) — executable choice is only the
//! scoring ported here. The preferred-name rules in [`select_executable`] for KytyPS5, ShadPS4 and Dolphin are
//! GRID additions, not parity: the reference only had the Eden and Azahar
//! ones.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::profiles::{profile_for_entry, EmulatorProfile};
use crate::library::extract::extract_archive;
use crate::library::paths::{sanitize_component, saves_dir, EMULATORS_DIR};
use crate::library::user_data_links::{ensure_user_data_links, user_data_root};

/// `_extract_emulator_archive`'s message when extraction finished but no
/// launchable file turned up (emulator_ui_mixin.py:1394). Verbatim — the
/// Emulators view shows it unchanged.
pub const NO_LAUNCHABLE_AFTER_EXTRACT: &str = "Archive extraction finished, but no launchable \
     executable was detected. Open Config to set the executable path manually.";

/// `<library>/emulators/<sanitize_component(name, "emulator")>` — no release
/// tag in the directory name (v1 library layout; `emulator_install_directory`,
/// autoconfig.py:14-17, predates the tag being dropped).
pub fn emulator_install_dir(library: &Path, name: &str) -> PathBuf {
    library
        .join(EMULATORS_DIR)
        .join(sanitize_component(name, "emulator"))
}

/// `<root>/<sanitize_component(archive_stem, "compat-tool")>` — the managed
/// compat-tool counterpart of [`emulator_install_dir`]. `root` is the compat
/// tools managed root ([`super::compat::managed_root`]), so, unlike
/// `emulator_install_dir`, there is no `emulators` subdirectory to join.
pub fn compat_tool_install_dir(root: &Path, archive_stem: &str) -> PathBuf {
    root.join(sanitize_component(archive_stem, "compat-tool"))
}

/// Extracts a manually entered emulator archive and returns the executable
/// to store as the entry's path (`_extract_emulator_archive`,
/// emulator_ui_mixin.py:1371-1404). The caller decides that `archive` IS an
/// archive ([`crate::library::extract::is_extractable_archive`]) and that a
/// library path exists.
///
/// The destination is [`emulator_install_dir`] under the ENTRY name, not the
/// archive stem, matching the reference. Blocking.
///
/// When the extracted executable matches a profile in `profiles`
/// ([`profile_for_entry`]) with a non-empty `user_data`, the matched
/// profile's directories are linked to `saves_dir(library, &profile.name)`
/// (the saves directory is named from the PROFILE, not the entry) at the
/// executable's data root ([`user_data_root`]) before returning — the
/// autoconfig sync the caller runs afterward must read through the link, not
/// overwrite it. A link error only warns: the archive
/// extracted and the entry is still valid without it.
pub fn install_manual_archive(
    library: &Path,
    entry_name: &str,
    archive: &Path,
    profiles: &[EmulatorProfile],
) -> Result<PathBuf, String> {
    if !archive.is_file() {
        return Err(format!(
            "Archive file was not found:\n{}",
            archive.display()
        ));
    }
    let dest = emulator_install_dir(library, entry_name);
    extract_archive(archive, &dest, &mut |_, _| {})
        .map_err(|e| format!("Failed to extract emulator archive: {e}"))?;
    let executable = select_executable(entry_name, &dest, archive)
        .ok_or_else(|| NO_LAUNCHABLE_AFTER_EXTRACT.to_string())?;
    // Python's `os.chmod(path, 0o755)` off win32 (emulator_ui_mixin.py:1399).
    make_executable(&executable);

    // At the emulator's DATA root (`user_data_root`): beside the EXECUTABLE
    // for every emulator but the PCSX2 AppImage, which reads
    // `<exe dir>/PCSX2` and nothing else. Never the extraction root — every
    // reader derives its directory from the executable, and
    // `select_executable` legally picks a nested binary.
    if let Some(profile) = profile_for_entry(entry_name, &executable.to_string_lossy(), profiles) {
        if !profile.user_data.is_empty() {
            let root = user_data_root(profile, &executable);
            if fs::create_dir_all(&root).is_err()
                || ensure_user_data_links(
                    &root,
                    &saves_dir(library, &profile.name),
                    &profile.user_data,
                )
                .is_err()
            {
                tracing::warn!("user data links failed for {}", root.display());
            }
        }
    }

    Ok(executable)
}

/// Marks `path` `0o755`. A no-op on Windows, which has no executable bit.
#[cfg(unix)]
pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
pub fn make_executable(_path: &Path) {}

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

/// Whether `name` (a bare file name) is an "extensionless executable": it
/// carries no `.` at all, so `libfoo.so` (a suffix) and `.hidden` (a dot
/// file) are both out, while `redream` and `pcsx2-qt` qualify.
fn extensionless_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('.')
}

/// Whether an already-extracted file is launchable, given its filesystem
/// metadata: either [`launchable_emulator_file`]'s suffix rule (all
/// platforms), or — unix only — an extensionless name whose executable bit
/// is set.
///
/// DEVIATION from the reference, which is name-only
/// (`launchable_emulator_file`, launch.py:27-28) and therefore can never
/// install an emulator shipping a bare ELF binary (Redream). The metadata is
/// only available where real paths are walked, so this predicate lives
/// alongside [`collect_launchable_files`] and the name-only
/// [`launchable_emulator_file`] keeps serving its other callers unchanged.
/// See `docs/porting/04-emulator-launch.md` "Rust port deviations
/// (milestone 4)".
fn launchable_installed_file(path: &Path, meta: &fs::Metadata) -> bool {
    if launchable_emulator_file(path) {
        return true;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    extensionless_name(&name) && has_executable_bit(meta)
}

#[cfg(unix)]
fn has_executable_bit(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

/// Windows has no executable bit; the suffix set is the whole rule there.
#[cfg(not(unix))]
fn has_executable_bit(_meta: &fs::Metadata) -> bool {
    false
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
/// filtered by [`launchable_installed_file`], autoconfig.py:59-62).
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
            Ok(meta) if meta.is_file() && launchable_installed_file(&path, &meta) => out.push(path),
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
    if title_casefold.contains("kyty") {
        // KytyPS5 ships `launcher` next to the emulator; without this the
        // pick would rest on the casefolded-path tie-break.
        preferred_names.insert("kyty_emulator");
        preferred_names.insert("kyty_emulator.exe");
    }
    if title_casefold.contains("shadps4") && !title_casefold.contains("launcher") {
        preferred_names.insert("shadps4.exe");
        preferred_names.insert("shadps4-sdl.appimage");
    }
    if title_casefold.contains("dolphin") {
        // The Windows build ships DolphinTool.exe and Updater.exe beside
        // Dolphin.exe, and every one of them carries the title token.
        preferred_names.insert("dolphin.exe");
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

    /// [`touch`], then sets `mode` on the created file.
    #[cfg(unix)]
    fn touch_with_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        touch(path);
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    // --- emulator_install_dir -------------------------------------------

    #[test]
    fn install_dir_joins_library_emulators_and_sanitized_stem() {
        let dir = emulator_install_dir(Path::new("/lib"), "PCSX2");
        assert_eq!(dir, Path::new("/lib/emulators/PCSX2"));
    }

    #[test]
    fn install_dir_sanitizes_illegal_characters() {
        let dir = emulator_install_dir(Path::new("/lib"), "Emu: <bad>*chars");
        assert_eq!(dir, Path::new("/lib/emulators/Emu_ _bad__chars"));
    }

    // --- compat_tool_install_dir -----------------------------------------

    #[test]
    fn compat_tool_install_dir_joins_root_and_sanitized_stem_with_no_emulators_subdir() {
        let dir = compat_tool_install_dir(Path::new("/root"), "GE-Proton-latest");
        assert_eq!(dir, Path::new("/root/GE-Proton-latest"));
    }

    #[test]
    fn compat_tool_install_dir_sanitizes_illegal_characters() {
        let dir = compat_tool_install_dir(Path::new("/root"), "Proton: <bad>*chars");
        assert_eq!(dir, Path::new("/root/Proton_ _bad__chars"));
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
        let primary = Path::new("/lib/emulators/PCSX2/PCSX2-v2.1.0.zip");
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
        let primary = Path::new("/lib/emulators/PCSX2/PCSX2-nosuffix");
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

    #[cfg(unix)]
    #[test]
    fn select_executable_prefers_kyty_emulator_over_the_bundled_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join("kyty_emulator"), 0o755);
        touch_with_mode(&install_dir.join("launcher"), 0o755);

        let picked = select_executable(
            "KytyPS5 (Playstation 5)",
            &install_dir,
            &dir.path().join("archive.gz"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("kyty_emulator"));
    }

    #[cfg(unix)]
    #[test]
    fn select_executable_prefers_kyty_emulator_even_when_a_decoy_sorts_first() {
        // Without the preferred name, the casefolded-path tie-break would
        // pick `aaa_launcher`: both files are bare executables, equally
        // deep, with no token hit.
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join("kyty_emulator"), 0o755);
        touch_with_mode(&install_dir.join("aaa_launcher"), 0o755);

        let picked = select_executable(
            "KytyPS5 (Playstation 5)",
            &install_dir,
            &dir.path().join("archive.gz"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("kyty_emulator"));
    }

    #[test]
    fn select_executable_prefers_kyty_emulator_exe_over_launcher_exe() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("kyty_emulator.exe"));
        touch(&install_dir.join("launcher.exe"));

        let picked = select_executable(
            "KytyPS5 (Playstation 5)",
            &install_dir,
            &dir.path().join("archive.zip"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("kyty_emulator.exe"));
    }

    #[test]
    fn select_executable_prefers_shadps4_exe_for_the_shadps4_title() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("shadps4.exe"));
        // Two token hits ("shadps4", "playstation") against shadps4.exe's
        // one: only the preferred name keeps the real emulator on top.
        touch(&install_dir.join("shadps4-playstation-updater.exe"));

        let picked = select_executable(
            "ShadPS4 (Playstation 4)",
            &install_dir,
            &dir.path().join("archive.zip"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("shadps4.exe"));
    }

    #[test]
    fn select_executable_prefers_the_shadps4_sdl_appimage_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("Shadps4-sdl.AppImage"));
        touch(&install_dir.join("shadps4-playstation-tools.sh"));

        let picked = select_executable(
            "ShadPS4 (Playstation 4)",
            &install_dir,
            &dir.path().join("archive.zip"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("Shadps4-sdl.AppImage"));
    }

    #[test]
    fn select_executable_does_not_apply_the_shadps4_names_to_the_qt_launcher_title() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch(&install_dir.join("shadPS4QtLauncher.exe"));
        touch(&install_dir.join("shadps4.exe"));

        let picked = select_executable(
            "ShadPS4 Qt Launcher",
            &install_dir,
            &dir.path().join("archive.zip"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("shadPS4QtLauncher.exe"));
    }

    /// The Windows build ships helper executables beside `Dolphin.exe`; the
    /// title tokens score them all the same, so without the preferred name
    /// the shallower `DolphinTool.exe` would win the path tie-break.
    #[test]
    fn dolphin_title_prefers_dolphin_exe_over_its_bundled_tools() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path();
        touch(&install.join("DolphinTool.exe"));
        touch(&install.join("Dolphin-x64").join("Dolphin.exe"));
        touch(&install.join("Dolphin-x64").join("Updater.exe"));
        let picked = select_executable(
            "Dolphin (GameCube, Wii)",
            install,
            &install.join("absent.7z"),
        );
        assert_eq!(
            picked,
            Some(install.join("Dolphin-x64").join("Dolphin.exe"))
        );
    }

    /// The profile name's comma is a legal path character: it survives
    /// `sanitize_component` unchanged in the install directory name.
    #[test]
    fn a_comma_in_the_profile_name_survives_the_install_dir() {
        assert_eq!(
            emulator_install_dir(Path::new("/lib"), "Dolphin (GameCube, Wii)"),
            Path::new("/lib/emulators/Dolphin (GameCube, Wii)")
        );
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

    // --- select_executable: extensionless executable-bit files (unix) -------

    #[cfg(unix)]
    #[test]
    fn select_executable_accepts_a_bare_executable_bit_file() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join("redream"), 0o755);
        touch(&install_dir.join("readme.txt"));

        let picked = select_executable(
            "Redream (Sega Dreamcast)",
            &install_dir,
            &dir.path().join("archive.gz"),
        )
        .unwrap();
        assert_eq!(picked, install_dir.join("redream"));
    }

    #[cfg(unix)]
    #[test]
    fn select_executable_rejects_an_extensionless_file_without_the_executable_bit() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join("redream"), 0o644);

        assert!(
            select_executable("Redream", &install_dir, &dir.path().join("archive.gz")).is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn select_executable_rejects_an_executable_shared_object() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join("libredream.so"), 0o755);

        assert!(
            select_executable("Redream", &install_dir, &dir.path().join("archive.gz")).is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn select_executable_rejects_a_hidden_executable_file() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join(".hidden"), 0o755);

        assert!(select_executable("Emu", &install_dir, &dir.path().join("archive.gz")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn select_executable_accepts_a_suffix_match_without_the_executable_bit() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        touch_with_mode(&install_dir.join("emu.sh"), 0o644);

        let picked =
            select_executable("Emu", &install_dir, &dir.path().join("archive.gz")).unwrap();
        assert_eq!(picked, install_dir.join("emu.sh"));
    }

    #[cfg(unix)]
    #[test]
    fn select_executable_scores_an_extensionless_candidate_below_an_exe_on_a_tie() {
        let dir = tempfile::tempdir().unwrap();
        let install_dir = dir.path().join("install");
        // Same token hits and same depth: the `.exe` preference still wins,
        // because an extensionless candidate goes through the same tuple.
        touch_with_mode(&install_dir.join("emu"), 0o755);
        touch(&install_dir.join("emu.exe"));

        let picked =
            select_executable("Emu", &install_dir, &dir.path().join("archive.gz")).unwrap();
        assert_eq!(picked, install_dir.join("emu.exe"));
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

    // --- install_manual_archive -------------------------------------------

    /// Writes a zip at `path` holding `(name, contents, unix mode)` entries.
    fn write_zip(path: &Path, entries: &[(&str, &str, u32)]) {
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer = ZipWriter::new(fs::File::create(path).unwrap());
        for (name, contents, mode) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default().unix_permissions(*mode))
                .unwrap();
            std::io::Write::write_all(&mut writer, contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    /// Writes a gzipped tar at `path` holding `(name, contents)` entries.
    fn write_tar_gz(path: &Path, entries: &[(&str, &str)]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let encoder = flate2::write::GzEncoder::new(
            fs::File::create(path).unwrap(),
            flate2::Compression::fast(),
        );
        let mut builder = tar::Builder::new(encoder);
        for (name, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, contents.as_bytes())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
    }

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn install_manual_archive_extracts_under_the_entry_name_and_marks_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let archive = dir.path().join("downloads/emu-v1.zip");
        write_zip(
            &archive,
            &[
                ("bin/emu.sh", "#!/bin/sh\n", 0o644),
                ("readme.txt", "hello", 0o644),
            ],
        );

        let executable = install_manual_archive(&library, "My Emu", &archive, &[]).unwrap();

        // The ENTRY name names the directory, not the archive stem.
        assert_eq!(executable, library.join("emulators/My Emu/bin/emu.sh"));
        assert_eq!(mode_of(&executable) & 0o111, 0o111);
    }

    #[cfg(unix)]
    #[test]
    fn install_manual_archive_handles_a_tar_gz() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let archive = dir.path().join("emu.tar.gz");
        write_tar_gz(&archive, &[("emu.sh", "#!/bin/sh\n")]);

        let executable = install_manual_archive(&library, "Tarred", &archive, &[]).unwrap();

        assert_eq!(executable, library.join("emulators/Tarred/emu.sh"));
        assert_eq!(mode_of(&executable) & 0o111, 0o111);
    }

    #[test]
    fn install_manual_archive_reports_a_missing_archive_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("gone.zip");

        let err =
            install_manual_archive(&dir.path().join("library"), "Emu", &archive, &[]).unwrap_err();

        assert_eq!(
            err,
            format!("Archive file was not found:\n{}", archive.display())
        );
    }

    #[test]
    fn install_manual_archive_reports_no_launchable_file_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let archive = dir.path().join("docs.zip");
        write_zip(&archive, &[("readme.txt", "hello", 0o644)]);

        let err = install_manual_archive(&library, "Emu", &archive, &[]).unwrap_err();

        assert_eq!(err, NO_LAUNCHABLE_AFTER_EXTRACT);
    }

    #[cfg(unix)]
    #[test]
    fn a_manual_archive_matching_a_profile_gets_its_user_data_links() {
        use crate::launch::profiles::load_profiles;

        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let archive = dir.path().join("pcsx2.zip");
        write_zip(
            &archive,
            &[
                ("pcsx2-qt", "#!/bin/sh\n", 0o755),
                ("memcards/x.mcd", "card", 0o644),
            ],
        );

        install_manual_archive(&library, "PCSX2 (Playstation 2)", &archive, load_profiles())
            .unwrap();

        let link = library.join("emulators/PCSX2 (Playstation 2)/memcards");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert!(library
            .join("saves/PCSX2 (Playstation 2)/memcards/x.mcd")
            .is_file());
    }

    /// The links follow the chosen EXECUTABLE, which may sit in a
    /// subdirectory: that is the directory `autoconfig::paths::emulator_dir`
    /// and `cloud::ops::emulator_dir_for` derive from the entry's path.
    #[cfg(unix)]
    #[test]
    fn a_manual_archive_links_beside_a_nested_executable() {
        use crate::launch::profiles::load_profiles;

        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let archive = dir.path().join("pcsx2.zip");
        write_zip(
            &archive,
            &[
                ("bin/pcsx2-qt", "#!/bin/sh\n", 0o755),
                ("bin/memcards/x.mcd", "card", 0o644),
            ],
        );

        install_manual_archive(&library, "PCSX2 (Playstation 2)", &archive, load_profiles())
            .unwrap();

        let install = library.join("emulators/PCSX2 (Playstation 2)");
        assert!(install
            .join("bin/memcards")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(
            !install.join("memcards").exists(),
            "nothing may be created at the install root"
        );
        assert!(library
            .join("saves/PCSX2 (Playstation 2)/memcards/x.mcd")
            .is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_manual_archive_with_no_matching_profile_gets_no_links() {
        use crate::launch::profiles::load_profiles;

        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        let archive = dir.path().join("mystery.zip");
        write_zip(&archive, &[("mystery.sh", "#!/bin/sh\n", 0o755)]);

        install_manual_archive(&library, "Mystery", &archive, load_profiles()).unwrap();

        assert!(!library.join("emulators/Mystery/memcards").exists());
        assert!(!library.join("saves").exists());
    }
}
