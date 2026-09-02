//! RPCS3's three portable-config files, its `/dev_hdd0` VFS redirect, and
//! the per-game `games.yml` entry: [`ensure_settings`] writes
//! `config.yml`/`GuiSettings.ini`/`CurrentSettings.ini` in one call (chaining
//! [`ensure_vfs_settings`] when a PS3 library path is given), and
//! [`update_games_yml`] is called once per game install.
//!
//! Ports `grid_launcher/emulator/rpcs3.py`'s `ensure_rpcs3_settings`
//! (rpcs3.py:533-602), `ensure_rpcs3_vfs_settings` (rpcs3.py:389-468) and
//! `update_rpcs3_games_yml` (rpcs3.py:307-362). See
//! `docs/porting/05-emulator-autoconfig.md` ("RPCS3") for the behavior
//! contract.

use std::collections::HashSet;
use std::path::Path;

use super::{paths, writers, EnsureResult};
use paths::resolve_best_effort;

/// Forward-slash form with a guaranteed trailing `/` — `as_posix()` plus
/// the manual "does it already end in `/`" guard every RPCS3 path string
/// gets (rpcs3.py:412-417, rpcs3.py:325-327).
fn forward_slash_with_trailing_slash(path: &Path) -> String {
    let mut text = path.to_string_lossy().replace('\\', "/");
    if !text.ends_with('/') {
        text.push('/');
    }
    text
}

/// The four `main_window` suppression keys GuiSettings.ini and
/// CurrentSettings.ini both manage, in their pinned order
/// (rpcs3.py:565-570, rpcs3.py:580-585).
fn main_window_desired() -> writers::Desired {
    crate::desired![
        ("infoBoxEnabledWelcome", "false"),
        ("confirmationBoxExitGame", "false"),
        ("confirmationBoxBootGame", "false"),
        ("infoBoxEnabledInstallPUP", "false"),
    ]
}

/// Read `path`'s contents, or `""` when it does not exist yet
/// (`config_path.read_text(...) if config_path.exists() else ""`,
/// rpcs3.py:557 and its two siblings at rpcs3.py:564/575). `Err` on a read
/// failure — every caller folds that into the whole function's
/// `unchanged()` bail-out, mirroring the Python `try/except OSError`
/// wrapping the entire write sequence.
fn read_existing(path: &Path) -> std::io::Result<String> {
    if path.exists() {
        std::fs::read_to_string(path)
    } else {
        Ok(String::new())
    }
}

/// `ensure_rpcs3_settings` (rpcs3.py:533-602): writes RPCS3's three
/// always-portable config files under `<exe parent>/portable/` — this is
/// the writer's target even for a non-portable install (doc 05's open
/// question, resolved by following the code).
///
/// A blank `emulator_path`, or one that is not an existing file, reports
/// [`EnsureResult::unchanged`]. Otherwise `portable/`, `portable/config/`
/// and `portable/GuiConfigs/` are created eagerly, before any file is
/// read. `config.yml` is add-only (`Miscellaneous."Start games in
/// fullscreen mode"` = `true`, `Audio."Master Volume"` = `40`, written
/// once when either call reports changed); `GuiSettings.ini` overwrites
/// its four `main_window` keys with `key\default=false` / `key = value`
/// pairs; `CurrentSettings.ini` overwrites `Meta` (`checkUpdateStart`,
/// `useRichPresence`) and the same four `main_window` keys as bare
/// `key=value` lines with every managed annotation line deleted. When
/// `ps3_library_path.trim()` is non-blank, [`ensure_vfs_settings`] also
/// runs and its `changed` is folded in, and `extras["vfs_path"]` is set
/// when that step produced a path. Any I/O error along the way reports
/// [`EnsureResult::unchanged`] — matching the Python function's single
/// `try/except OSError` around the whole write sequence. Spec deviation
/// D8: `extras["current_settings_path"]` is always set, unlike the Python
/// dict which omits it.
pub fn ensure_settings(emulator_path: &str, ps3_library_path: &str) -> EnsureResult {
    let trimmed_path = emulator_path.trim();
    if trimmed_path.is_empty() {
        return EnsureResult::unchanged();
    }

    let exe_path = paths::expand_user(trimmed_path);
    if !exe_path.is_file() {
        return EnsureResult::unchanged();
    }
    let Some(emulator_dir) = exe_path.parent() else {
        return EnsureResult::unchanged();
    };

    let portable_dir = emulator_dir.join("portable");
    let config_dir = portable_dir.join("config");
    let gui_dir = portable_dir.join("GuiConfigs");
    if std::fs::create_dir_all(&portable_dir).is_err()
        || std::fs::create_dir_all(&config_dir).is_err()
        || std::fs::create_dir_all(&gui_dir).is_err()
    {
        return EnsureResult::unchanged();
    }

    let config_path = config_dir.join("config.yml");
    let gui_path = gui_dir.join("GuiSettings.ini");
    let current_settings_path = gui_dir.join("CurrentSettings.ini");
    let mut changed = false;

    let Ok(yml_content) = read_existing(&config_path) else {
        return EnsureResult::unchanged();
    };
    let (yml_content, c1) = writers::yaml_add_only_section(
        &yml_content,
        "Miscellaneous",
        &crate::desired![("Start games in fullscreen mode", "true")],
    );
    let (yml_content, c2) = writers::yaml_add_only_section(
        &yml_content,
        "Audio",
        &crate::desired![("Master Volume", "40")],
    );
    if c1 || c2 {
        if std::fs::write(&config_path, &yml_content).is_err() {
            return EnsureResult::unchanged();
        }
        changed = true;
    }

    let main_window = main_window_desired();

    let Ok(gui_content) = read_existing(&gui_path) else {
        return EnsureResult::unchanged();
    };
    let (gui_content, g1) =
        writers::rpcs3_gui_section(&gui_content, "main_window", &main_window, true);
    if g1 {
        if std::fs::write(&gui_path, &gui_content).is_err() {
            return EnsureResult::unchanged();
        }
        changed = true;
    }

    let Ok(current_content) = read_existing(&current_settings_path) else {
        return EnsureResult::unchanged();
    };
    let (current_content, cs1) = writers::rpcs3_gui_section(
        &current_content,
        "Meta",
        &crate::desired![("checkUpdateStart", "false"), ("useRichPresence", "false")],
        false,
    );
    let (current_content, cs2) =
        writers::rpcs3_gui_section(&current_content, "main_window", &main_window, false);
    if cs1 || cs2 {
        if std::fs::write(&current_settings_path, &current_content).is_err() {
            return EnsureResult::unchanged();
        }
        changed = true;
    }

    let mut result = EnsureResult::at(config_path, changed)
        .with_extra("gui_config_path", gui_path)
        .with_extra("current_settings_path", current_settings_path);

    if !ps3_library_path.trim().is_empty() {
        let vfs_result = ensure_vfs_settings(emulator_path, ps3_library_path);
        if vfs_result.changed {
            changed = true;
        }
        if let Some(vfs_path) = vfs_result.config_path {
            result = result.with_extra("vfs_path", vfs_path);
        }
    }

    result.changed = changed;
    result
}

/// `ensure_rpcs3_vfs_settings` (rpcs3.py:389-468): add-only writer for
/// `<exe parent>/portable/config/vfs.yml`'s three redirect entries.
///
/// [`EnsureResult::unchanged`] for a blank `emulator_path` or
/// `ps3_library_path`, or when `emulator_path` is not an existing file.
/// Otherwise the config directory is created unconditionally (even when
/// nothing ends up changed), `ps3_library_path` is expanded and
/// canonicalized where possible, and `/dev_hdd0/` / `/games/` are derived
/// from it as forward-slash strings with a trailing `/`. Desired entries
/// (`$(EmulatorDir)` -> `""`, `/dev_hdd0/` -> the dev_hdd0 string,
/// `/games/` -> the games string) that already have a line in the file are
/// never touched; missing ones are appended via [`vfs_add_only`] as
/// `"{key}": "{value}"`, both sides always double-quoted. The write is
/// skipped entirely when nothing changed.
pub fn ensure_vfs_settings(emulator_path: &str, ps3_library_path: &str) -> EnsureResult {
    let trimmed_path = emulator_path.trim();
    let trimmed_library = ps3_library_path.trim();
    if trimmed_path.is_empty() || trimmed_library.is_empty() {
        return EnsureResult::unchanged();
    }

    let exe_path = paths::expand_user(trimmed_path);
    if !exe_path.is_file() {
        return EnsureResult::unchanged();
    }
    let Some(emulator_dir) = exe_path.parent() else {
        return EnsureResult::unchanged();
    };

    let config_dir = emulator_dir.join("portable").join("config");
    let vfs_path = config_dir.join("vfs.yml");
    if std::fs::create_dir_all(&config_dir).is_err() {
        return EnsureResult::unchanged();
    }

    let library_expanded = paths::expand_user(trimmed_library);
    let library_path = resolve_best_effort(&library_expanded);
    let dev_hdd0_str =
        forward_slash_with_trailing_slash(&library_path.join(".vfs").join("dev_hdd0"));
    let games_str = forward_slash_with_trailing_slash(&library_path.join(".vfs").join("games"));

    let desired: Vec<(String, String)> = vec![
        ("$(EmulatorDir)".to_string(), String::new()),
        ("/dev_hdd0/".to_string(), dev_hdd0_str),
        ("/games/".to_string(), games_str),
    ];

    let Ok(existing_content) = read_existing(&vfs_path) else {
        return EnsureResult::unchanged();
    };
    let (output, changed) = vfs_add_only(&existing_content, &desired);

    if changed && std::fs::write(&vfs_path, &output).is_err() {
        return EnsureResult::unchanged();
    }

    EnsureResult::at(vfs_path, changed)
}

/// The add-only vfs.yml comparison/append rule (rpcs3.py:429-458): for
/// each existing line, trim, skip blank and `#`-prefixed lines, find the
/// first `:` (skip the line for key collection when absent), and take the
/// text before it trimmed then `trim_matches('"')` then
/// `trim_matches('\'')` IN THAT ORDER as the existing key. Any `desired`
/// key already present is left untouched; the rest are appended at the end
/// as `"{key}": "{value}"`. Output is `lines.join("\n")` plus one `\n` when
/// non-empty and not already newline-terminated — deliberately NOT run
/// through [`writers`]'s `trim_end()` normalization, so a pre-existing
/// trailing blank line or trailing whitespace survives verbatim.
fn vfs_add_only(existing_content: &str, desired: &[(String, String)]) -> (String, bool) {
    let mut lines: Vec<String> = existing_content.lines().map(str::to_string).collect();

    let mut existing_keys: HashSet<String> = HashSet::new();
    for line in &lines {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let Some(colon_index) = stripped.find(':') else {
            continue;
        };
        let key = stripped[..colon_index]
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if !key.is_empty() {
            existing_keys.insert(key.to_string());
        }
    }

    let mut changed = false;
    for (key, value) in desired {
        if existing_keys.contains(key) {
            continue;
        }
        lines.push(format!("\"{key}\": \"{value}\""));
        changed = true;
    }

    let mut output = lines.join("\n");
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    (output, changed)
}

/// `update_rpcs3_games_yml` (rpcs3.py:307-362): writes or updates one
/// `<game_id>: "<dir>"` line in `<data_root>/config/games.yml`.
///
/// `false` for a blank (trimmed) `game_id`. The game directory is
/// `<games_root>/<game_id>` when `games_root` is `Some`, else
/// `<dev_hdd0_root>/game/<game_id>`, canonicalized where possible and
/// rendered forward-slash with a trailing `/`. `games.yml`'s parent
/// directory is created; the file's existing lines are scanned, keeping
/// any line with no `:` verbatim, and for a line that has one, comparing
/// its trimmed, `"`-then-`'`-unquoted key text against `game_id` — EVERY
/// matching line is replaced with the new entry (so a file with duplicate
/// keys ends up with duplicate updated lines), and the new entry is
/// appended only when no line matched. Any I/O error reports `false`.
pub fn update_games_yml(
    data_root: &Path,
    game_id: &str,
    dev_hdd0_root: &Path,
    games_root: Option<&Path>,
) -> bool {
    let trimmed_id = game_id.trim();
    if trimmed_id.is_empty() {
        return false;
    }

    let game_dir = match games_root {
        Some(root) => root.join(trimmed_id),
        None => dev_hdd0_root.join("game").join(trimmed_id),
    };
    let game_dir = resolve_best_effort(&game_dir);
    let game_dir_str = forward_slash_with_trailing_slash(&game_dir);

    let config_dir = data_root.join("config");
    let games_yml_path = config_dir.join("games.yml");
    if std::fs::create_dir_all(&config_dir).is_err() {
        return false;
    }

    let Ok(existing_content) = read_existing(&games_yml_path) else {
        return false;
    };

    let updated_line = format!("{trimmed_id}: \"{game_dir_str}\"");
    let mut found = false;
    let mut next_lines: Vec<String> = Vec::new();
    for line in existing_content.lines() {
        let stripped = line.trim();
        let Some(colon_index) = stripped.find(':') else {
            next_lines.push(line.to_string());
            continue;
        };
        let raw_key = stripped[..colon_index]
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if raw_key == trimmed_id {
            next_lines.push(updated_line.clone());
            found = true;
        } else {
            next_lines.push(line.to_string());
        }
    }
    if !found {
        next_lines.push(updated_line);
    }

    let mut output = next_lines.join("\n");
    if !next_lines.is_empty() {
        output.push('\n');
    }

    std::fs::write(&games_yml_path, &output).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_exe(temp: &Path) -> PathBuf {
        let exe = temp.join("rpcs3.exe");
        std::fs::write(&exe, b"").unwrap();
        exe
    }

    // --- ensure_settings: bail-out and directory creation -------------------

    #[test]
    fn ensure_requires_an_existing_executable_file() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("rpcs3.exe");
        assert_eq!(
            ensure_settings(missing.to_str().unwrap(), ""),
            EnsureResult::unchanged()
        );
        // A directory is not a file either.
        assert_eq!(
            ensure_settings(temp.path().to_str().unwrap(), ""),
            EnsureResult::unchanged()
        );
        assert_eq!(ensure_settings("", ""), EnsureResult::unchanged());
    }

    #[test]
    fn ensure_creates_the_three_portable_directories() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        ensure_settings(exe.to_str().unwrap(), "");
        assert!(temp.path().join("portable").is_dir());
        assert!(temp.path().join("portable").join("config").is_dir());
        assert!(temp.path().join("portable").join("GuiConfigs").is_dir());
    }

    // --- ensure_settings: first/second run -----------------------------------

    #[test]
    fn ensure_first_run_writes_all_three_files_and_reports_changed() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let result = ensure_settings(exe.to_str().unwrap(), "");
        assert!(result.changed);

        let portable = temp.path().join("portable");
        assert_eq!(
            result.config_path,
            Some(portable.join("config").join("config.yml"))
        );
        assert_eq!(
            result.extras.get("gui_config_path"),
            Some(&portable.join("GuiConfigs").join("GuiSettings.ini"))
        );
        assert_eq!(
            result.extras.get("current_settings_path"),
            Some(&portable.join("GuiConfigs").join("CurrentSettings.ini"))
        );
        assert!(portable.join("config").join("config.yml").is_file());
        assert!(portable
            .join("GuiConfigs")
            .join("GuiSettings.ini")
            .is_file());
        assert!(portable
            .join("GuiConfigs")
            .join("CurrentSettings.ini")
            .is_file());
    }

    #[test]
    fn ensure_second_run_reports_unchanged_and_writes_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        ensure_settings(exe.to_str().unwrap(), "");

        let portable = temp.path().join("portable");
        let config_path = portable.join("config").join("config.yml");
        let gui_path = portable.join("GuiConfigs").join("GuiSettings.ini");
        let current_path = portable.join("GuiConfigs").join("CurrentSettings.ini");

        fn snapshot(p: &Path) -> (std::time::SystemTime, Vec<u8>) {
            (
                std::fs::metadata(p).unwrap().modified().unwrap(),
                std::fs::read(p).unwrap(),
            )
        }
        let before = (
            snapshot(&config_path),
            snapshot(&gui_path),
            snapshot(&current_path),
        );

        let result = ensure_settings(exe.to_str().unwrap(), "");
        assert!(!result.changed);

        let after = (
            snapshot(&config_path),
            snapshot(&gui_path),
            snapshot(&current_path),
        );
        assert_eq!(before, after, "a second run must not touch any file");
    }

    // --- config.yml (add-only YAML) ------------------------------------------

    #[test]
    fn config_yml_preserves_an_existing_master_volume() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let config_dir = temp.path().join("portable").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.yml"),
            "Audio:\n  Master Volume: 77\n",
        )
        .unwrap();

        ensure_settings(exe.to_str().unwrap(), "");

        let text = std::fs::read_to_string(config_dir.join("config.yml")).unwrap();
        assert!(text.contains("Master Volume: 77"));
        assert!(!text.contains("Master Volume: 40"));
    }

    #[test]
    fn config_yml_appends_a_missing_section_with_a_blank_separator() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let config_dir = temp.path().join("portable").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.yml"), "Other:\n  X: 1\n").unwrap();

        ensure_settings(exe.to_str().unwrap(), "");

        let text = std::fs::read_to_string(config_dir.join("config.yml")).unwrap();
        assert_eq!(
            text,
            "Other:\n  X: 1\n\nMiscellaneous:\n  Start games in fullscreen mode: true\n\nAudio:\n  Master Volume: 40\n"
        );
    }

    // --- GuiSettings.ini (annotated overwrite) -------------------------------

    #[test]
    fn gui_settings_writes_annotation_and_value_line_pairs() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        ensure_settings(exe.to_str().unwrap(), "");
        let text = std::fs::read_to_string(
            temp.path()
                .join("portable")
                .join("GuiConfigs")
                .join("GuiSettings.ini"),
        )
        .unwrap();
        assert!(
            text.contains("infoBoxEnabledWelcome\\default=false\ninfoBoxEnabledWelcome = false")
        );
    }

    // --- CurrentSettings.ini (bare, unannotated overwrite) -------------------

    #[test]
    fn current_settings_writes_bare_key_equals_value_with_no_spaces() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        ensure_settings(exe.to_str().unwrap(), "");
        let text = std::fs::read_to_string(
            temp.path()
                .join("portable")
                .join("GuiConfigs")
                .join("CurrentSettings.ini"),
        )
        .unwrap();
        assert!(text.contains("checkUpdateStart=false"));
        assert!(!text.contains("checkUpdateStart = false"));
    }

    #[test]
    fn current_settings_deletes_managed_default_annotation_lines() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let gui_dir = temp.path().join("portable").join("GuiConfigs");
        std::fs::create_dir_all(&gui_dir).unwrap();
        std::fs::write(
            gui_dir.join("CurrentSettings.ini"),
            "[main_window]\nconfirmationBoxExitGame\\default=false\nconfirmationBoxExitGame=true\n",
        )
        .unwrap();

        ensure_settings(exe.to_str().unwrap(), "");

        let text = std::fs::read_to_string(gui_dir.join("CurrentSettings.ini")).unwrap();
        assert!(!text.contains("confirmationBoxExitGame\\default"));
        assert!(text.contains("confirmationBoxExitGame=false"));
    }

    #[test]
    fn current_settings_overwrites_an_existing_managed_value() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let gui_dir = temp.path().join("portable").join("GuiConfigs");
        std::fs::create_dir_all(&gui_dir).unwrap();
        std::fs::write(
            gui_dir.join("CurrentSettings.ini"),
            "[Meta]\ncheckUpdateStart=true\nuseRichPresence=true\n",
        )
        .unwrap();

        ensure_settings(exe.to_str().unwrap(), "");

        let text = std::fs::read_to_string(gui_dir.join("CurrentSettings.ini")).unwrap();
        assert!(text.contains("checkUpdateStart=false"));
        assert!(!text.contains("checkUpdateStart=true"));
        assert!(text.contains("useRichPresence=false"));
        assert!(!text.contains("useRichPresence=true"));
    }

    // --- VFS chaining from ensure_settings ------------------------------------

    #[test]
    fn ensure_folds_vfs_changed_in_when_a_library_path_is_given() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("PS3 Library");

        let result = ensure_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        assert!(result.changed);
        let vfs_path = temp.path().join("portable").join("config").join("vfs.yml");
        assert_eq!(result.extras.get("vfs_path"), Some(&vfs_path));
        assert!(vfs_path.is_file());
    }

    // --- ensure_vfs_settings ---------------------------------------------------

    #[test]
    fn vfs_writes_the_three_quoted_entries() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("PS3 Library");

        let result = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        assert!(result.changed);
        let text = std::fs::read_to_string(result.config_path.clone().unwrap()).unwrap();
        assert!(text.contains("\"$(EmulatorDir)\": \"\""));
        assert!(text.contains("\"/dev_hdd0/\": \""));
        assert!(text.contains("\"/games/\": \""));
        assert!(text.contains(".vfs/dev_hdd0/"));
        assert!(text.contains(".vfs/games/"));
    }

    /// Exercises the shared `paths::resolve_best_effort` through
    /// `ensure_vfs_settings`'s `library_path` resolution: `library` is
    /// built with a literal `..` segment through a directory (`sub`) that
    /// is never created, and neither `sub` nor the final `PS3 Library`
    /// directory exists on disk. A `..`-blind resolver would leave the
    /// literal `..` in the written path; the shared helper must collapse it
    /// lexically before any existence check, matching Python's
    /// `Path.resolve(strict=False)`.
    #[test]
    fn vfs_library_path_collapses_parent_dir_through_a_nonexistent_directory() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("sub").join("..").join("PS3 Library");
        let expected_library = resolve_best_effort(temp.path()).join("PS3 Library");

        let result = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());

        assert!(result.changed);
        let text = std::fs::read_to_string(result.config_path.unwrap()).unwrap();
        assert!(!text.contains(".."), "{text}");
        let expected_dev_hdd0 =
            forward_slash_with_trailing_slash(&expected_library.join(".vfs").join("dev_hdd0"));
        assert!(
            text.contains(&format!("\"/dev_hdd0/\": \"{expected_dev_hdd0}\"")),
            "{text}"
        );
    }

    #[test]
    fn vfs_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("PS3 Library");

        let first = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        let second = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        assert!(first.changed);
        assert!(!second.changed);

        let text = std::fs::read_to_string(first.config_path.unwrap()).unwrap();
        assert_eq!(text.matches("\"/dev_hdd0/\":").count(), 1);
        assert_eq!(text.matches("\"/games/\":").count(), 1);
    }

    #[test]
    fn vfs_never_overwrites_an_existing_key() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("PS3 Library");
        let config_dir = temp.path().join("portable").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("vfs.yml"),
            "\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"/somewhere/else/\"\n",
        )
        .unwrap();

        let result = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        assert!(result.changed, "/games/ is still missing");

        let text = std::fs::read_to_string(config_dir.join("vfs.yml")).unwrap();
        assert!(text.contains("\"/dev_hdd0/\": \"/somewhere/else/\""));
        assert!(text.contains("\"/games/\":"));
    }

    #[test]
    fn vfs_matches_an_unquoted_existing_key() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("PS3 Library");
        let config_dir = temp.path().join("portable").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("vfs.yml"),
            "$(EmulatorDir): \n/dev_hdd0/: /custom/\n/games/: /customgames/\n",
        )
        .unwrap();

        let result = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        assert!(!result.changed);

        let text = std::fs::read_to_string(config_dir.join("vfs.yml")).unwrap();
        assert!(text.contains("/dev_hdd0/: /custom/"));
        assert!(!text.contains("\"/dev_hdd0/\":"));
    }

    #[test]
    fn vfs_output_is_not_trailing_whitespace_normalized() {
        let temp = tempfile::tempdir().unwrap();
        let exe = make_exe(temp.path());
        let library = temp.path().join("PS3 Library");
        let config_dir = temp.path().join("portable").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let seed = "\"$(EmulatorDir)\": \"\"   \n\n";
        std::fs::write(config_dir.join("vfs.yml"), seed).unwrap();

        let result = ensure_vfs_settings(exe.to_str().unwrap(), library.to_str().unwrap());
        assert!(result.changed);

        let text = std::fs::read_to_string(config_dir.join("vfs.yml")).unwrap();
        assert!(
            text.starts_with(seed),
            "the pre-existing trailing whitespace and blank line must survive verbatim, \
             not be trim_end()-normalized away: {text:?}"
        );
        assert!(text.contains("\"/dev_hdd0/\":"));
        assert!(text.contains("\"/games/\":"));
    }

    // --- update_games_yml -------------------------------------------------------

    #[test]
    fn games_yml_appends_a_new_entry_with_the_dev_hdd0_layout() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("portable");
        let dev_hdd0 = temp.path().join("dev_hdd0");
        let game_dir = dev_hdd0.join("game").join("BLUS30336");
        std::fs::create_dir_all(&game_dir).unwrap();

        let result = update_games_yml(&data_root, "BLUS30336", &dev_hdd0, None);
        assert!(result);

        let content = std::fs::read_to_string(data_root.join("config").join("games.yml")).unwrap();
        assert!(content.contains("BLUS30336:"));
        let expected_dir = std::fs::canonicalize(&game_dir).unwrap();
        let expected_str = format!("{}/", expected_dir.to_string_lossy().replace('\\', "/"));
        assert!(content.contains(&expected_str));
    }

    #[test]
    fn games_yml_uses_the_games_root_layout_when_given() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("portable");
        let dev_hdd0 = temp.path().join("dev_hdd0");
        let games_root = temp.path().join("games");
        let game_dir = games_root.join("BLUS30336");
        std::fs::create_dir_all(&game_dir).unwrap();

        let result = update_games_yml(&data_root, "BLUS30336", &dev_hdd0, Some(&games_root));
        assert!(result);

        let content = std::fs::read_to_string(data_root.join("config").join("games.yml")).unwrap();
        let expected_dir = std::fs::canonicalize(&game_dir).unwrap();
        let expected_str = format!("{}/", expected_dir.to_string_lossy().replace('\\', "/"));
        assert!(content.contains(&expected_str));
        assert!(!content.contains("dev_hdd0"));
    }

    #[test]
    fn games_yml_updates_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("portable");
        let dev_hdd0 = temp.path().join("dev_hdd0");
        let config_dir = data_root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("games.yml"),
            "BLUS30336: \"/old/path/EBOOT.BIN\"\nOTHER: \"/keep/\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dev_hdd0.join("game").join("BLUS30336")).unwrap();

        let result = update_games_yml(&data_root, "BLUS30336", &dev_hdd0, None);
        assert!(result);

        let content = std::fs::read_to_string(config_dir.join("games.yml")).unwrap();
        assert!(!content.contains("/old/path"));
        assert!(content.contains("OTHER: \"/keep/\""));
        assert_eq!(content.matches("BLUS30336:").count(), 1);
    }

    #[test]
    fn games_yml_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("portable");
        let dev_hdd0 = temp.path().join("dev_hdd0");
        std::fs::create_dir_all(dev_hdd0.join("game").join("BLUS30336")).unwrap();

        let first = update_games_yml(&data_root, "BLUS30336", &dev_hdd0, None);
        let second = update_games_yml(&data_root, "BLUS30336", &dev_hdd0, None);
        assert!(first);
        assert!(second);

        let content = std::fs::read_to_string(data_root.join("config").join("games.yml")).unwrap();
        assert_eq!(content.matches("BLUS30336:").count(), 1);
    }

    #[test]
    fn games_yml_returns_false_for_a_blank_game_id() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("portable");
        let dev_hdd0 = temp.path().join("dev_hdd0");
        assert!(!update_games_yml(&data_root, "", &dev_hdd0, None));
        assert!(!update_games_yml(&data_root, "   ", &dev_hdd0, None));
    }
}
