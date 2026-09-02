//! RetroArch's flat `retroarch.cfg`: config-path candidates, the
//! save-location/sort-flag reader, and the two `ensure_*` writers.
//!
//! Ports `grid_launcher/emulator/retroarch.py`'s config-discovery and
//! writer functions (module docstring; function-level citations below). See
//! `docs/porting/05-emulator-autoconfig.md` ("RetroArch —
//! `ensure_retroarch_save_location_settings`"). Core-list machinery lives in
//! [`super::cores`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{paths, writers, EnsureResult, RaCredentials};

/// `retroarch_directory_settings`'s return shape (retroarch.py:173-229):
/// the parsed save-location/sort-flag settings. `config_path` and the two
/// directory fields are `""` when unset; the reader never returns a
/// partially populated struct (doc 05 invariant 6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetroarchSettings {
    pub config_path: String,
    pub savefile_directory: String,
    pub savestate_directory: String,
    pub savefiles_in_content_dir: bool,
    pub savestates_in_content_dir: bool,
    pub sort_savefiles_enable: bool,
    pub sort_savestates_enable: bool,
    pub sort_savefiles_by_content_enable: bool,
    pub sort_savestates_by_content_enable: bool,
}

/// `retroarch_config_path_candidates` (retroarch.py:136-165).
///
/// A blank path yields no candidates. Otherwise the search root is the
/// parent directory when the (expanded) path is an existing file OR merely
/// has a non-empty extension — so a not-yet-installed `retroarch.exe` still
/// resolves to its would-be install directory — else the path itself.
/// `<root>/retroarch.cfg` and `<root>/config/retroarch.cfg` come first, then
/// the two XDG candidates and the `~/.config` fallback; the whole list is
/// deduped case-insensitively, first occurrence wins. Existence is never
/// checked here.
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf> {
    if emulator_path.is_empty() {
        return Vec::new();
    }

    let expanded = paths::expand_user(emulator_path);
    let has_suffix = expanded
        .extension()
        .map(|ext| !ext.is_empty())
        .unwrap_or(false);
    let root = if expanded.is_file() || has_suffix {
        expanded
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| expanded.clone())
    } else {
        expanded.clone()
    };

    let mut candidates = vec![
        root.join("retroarch.cfg"),
        root.join("config").join("retroarch.cfg"),
        paths::xdg_config_home()
            .join("retroarch")
            .join("retroarch.cfg"),
        paths::xdg_data_home()
            .join("retroarch")
            .join("retroarch.cfg"),
    ];
    if let Some(home) = paths::home_dir() {
        candidates.push(home.join(".config").join("retroarch").join("retroarch.cfg"));
    }

    paths::dedupe_casefold(candidates)
}

/// The truthy set `_retroarch_config_bool` checks against, after
/// lowercasing and trimming (retroarch.py:168-170).
fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Strip one matched pair of surrounding quotes (`"` or `'`) when the
/// (character-counted) value is at least 2 characters long and its first
/// and last characters are the same quote (retroarch.py:203-204).
fn strip_matched_quotes(value: &str) -> &str {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return value;
    };
    let Some(last) = value.chars().next_back() else {
        return value;
    };
    if value.chars().count() >= 2 && first == last && (first == '"' || first == '\'') {
        let start = first.len_utf8();
        let end = value.len() - last.len_utf8();
        &value[start..end]
    } else {
        value
    }
}

/// One candidate file's parsed `key = value` lines: trim each line, skip
/// blank/`#`-prefixed/no-`=` lines, split on the FIRST `=`, trim both
/// halves, strip one matched quote pair from the value. Last duplicate key
/// wins (retroarch.py:190-205).
fn parse_flat_cfg(raw_content: &str) -> HashMap<String, String> {
    let mut parsed = HashMap::new();
    for raw_line in raw_content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(eq_index) = line.find('=') else {
            continue;
        };
        let key = line[..eq_index].trim().to_string();
        let value = strip_matched_quotes(line[eq_index + 1..].trim()).to_string();
        parsed.insert(key, value);
    }
    parsed
}

/// `retroarch_directory_settings` (retroarch.py:173-229).
///
/// Walks the candidates, skipping any that do not exist, are not a file, or
/// are unreadable. The first candidate with at least one parseable line
/// wins: `config_path` is set, `savefile_directory`/`savestate_directory`
/// are taken from the parsed value when non-blank and not the
/// case-insensitive sentinel `default`, and the six booleans are set from
/// the truthy set `{"1", "true", "yes", "on"}` when the key is present with
/// a non-blank value. A candidate with nothing parseable is skipped
/// entirely, continuing to the next one.
pub fn directory_settings(emulator_path: &str) -> RetroarchSettings {
    let mut settings = RetroarchSettings::default();

    for candidate in config_path_candidates(emulator_path) {
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
            continue;
        };

        let parsed = parse_flat_cfg(&raw_content);
        if parsed.is_empty() {
            continue;
        }

        settings.config_path = candidate.to_string_lossy().to_string();

        if let Some(raw) = parsed.get("savefile_directory") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() && trimmed.to_lowercase() != "default" {
                settings.savefile_directory = trimmed.to_string();
            }
        }
        if let Some(raw) = parsed.get("savestate_directory") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() && trimmed.to_lowercase() != "default" {
                settings.savestate_directory = trimmed.to_string();
            }
        }

        if let Some(raw) = parsed.get("savefiles_in_content_dir") {
            if !raw.trim().is_empty() {
                settings.savefiles_in_content_dir = is_truthy(raw);
            }
        }
        if let Some(raw) = parsed.get("savestates_in_content_dir") {
            if !raw.trim().is_empty() {
                settings.savestates_in_content_dir = is_truthy(raw);
            }
        }
        if let Some(raw) = parsed.get("sort_savefiles_enable") {
            if !raw.trim().is_empty() {
                settings.sort_savefiles_enable = is_truthy(raw);
            }
        }
        if let Some(raw) = parsed.get("sort_savestates_enable") {
            if !raw.trim().is_empty() {
                settings.sort_savestates_enable = is_truthy(raw);
            }
        }
        if let Some(raw) = parsed.get("sort_savefiles_by_content_enable") {
            if !raw.trim().is_empty() {
                settings.sort_savefiles_by_content_enable = is_truthy(raw);
            }
        }
        if let Some(raw) = parsed.get("sort_savestates_by_content_enable") {
            if !raw.trim().is_empty() {
                settings.sort_savestates_by_content_enable = is_truthy(raw);
            }
        }

        break;
    }

    settings
}

/// The candidate/target resolution shared by [`ensure_settings`] and
/// [`ensure_ra_credentials`] (retroarch.py:240-256): the current settings
/// (needed by both the desired-value construction and the write-failure
/// fallback), and the write target — the parsed `config_path` when
/// non-blank (expanded), else the first candidate. `None` when there are no
/// candidates at all (a blank path).
fn resolve_target(emulator_path: &str) -> Option<(RetroarchSettings, PathBuf)> {
    let settings = directory_settings(emulator_path);
    let candidates = config_path_candidates(emulator_path);
    if candidates.is_empty() {
        return None;
    }

    let configured = settings.config_path.trim();
    let target = if !configured.is_empty() {
        paths::expand_user(configured)
    } else {
        candidates[0].clone()
    };

    Some((settings, target))
}

/// The write-error fallback (doc 05 invariant 4): the pre-write settings'
/// `config_path`, with `changed = false` — never the write target, which
/// may never have existed (retroarch.py:348-350).
fn write_failure_result(settings: &RetroarchSettings) -> EnsureResult {
    let configured = settings.config_path.trim();
    if configured.is_empty() {
        EnsureResult::unchanged()
    } else {
        EnsureResult::at(PathBuf::from(configured), false)
    }
}

/// Read `target` (or treat it as absent on any I/O error), run it through
/// `writers::flat_cfg`, and write the result when changed — `create_dir_all`
/// on the parent first, and reporting `changed = false` against the
/// pre-write settings on any write error (retroarch.py:301-350).
fn write_desired(
    target: &Path,
    settings: &RetroarchSettings,
    desired: &writers::Desired,
) -> EnsureResult {
    let existed = target.exists();
    let raw_content = if existed {
        std::fs::read_to_string(target).unwrap_or_default()
    } else {
        String::new()
    };

    let (new_text, flat_changed) = writers::flat_cfg(&raw_content, desired, &["audio_volume"]);
    let changed = !existed || flat_changed;

    if changed {
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() {
                return write_failure_result(settings);
            }
        }
        if std::fs::write(target, &new_text).is_err() {
            return write_failure_result(settings);
        }
    }

    EnsureResult::at(target.to_path_buf(), changed)
}

/// The full managed-key desired-value set, in the pinned order: the 22
/// static keys, then `netplay_nickname` (from the trimmed `romm_username`,
/// when non-blank), then `video_fullscreen` (only when `enable_fullscreen`
/// — omitted, not `false`, so an existing line survives), then the three
/// `cheevos_*` credential keys (only when BOTH RA fields are non-blank
/// after trimming) (retroarch.py:258-299).
fn build_desired(
    settings: &RetroarchSettings,
    enable_fullscreen: bool,
    romm_username: &str,
    ra: Option<&RaCredentials>,
) -> writers::Desired {
    let savefile_directory = {
        let trimmed = settings.savefile_directory.trim();
        if trimmed.is_empty() {
            "saves".to_string()
        } else {
            trimmed.to_string()
        }
    };
    let savestate_directory = {
        let trimmed = settings.savestate_directory.trim();
        if trimmed.is_empty() {
            "states".to_string()
        } else {
            trimmed.to_string()
        }
    };

    let mut desired: writers::Desired = crate::desired![
        ("savefile_directory", savefile_directory.as_str()),
        ("savestate_directory", savestate_directory.as_str()),
        ("video_windowed_fullscreen", "true"),
        ("audio_volume", "-18.000000"),
        ("discord_enable", "false"),
        ("pause_nonactive", "true"),
        ("video_vsync", "true"),
        ("input_menu_toggle_gamepad_combo", "2"),
        ("savestate_auto_save", "false"),
        ("savestate_auto_load", "false"),
        ("rgui_show_start_screen", "false"),
        ("menu_show_core_updater", "false"),
        ("sort_savefiles_enable", "false"),
        ("sort_savestates_enable", "false"),
        ("sort_savefiles_by_content_enable", "false"),
        ("sort_savestates_by_content_enable", "false"),
        ("savefiles_in_content_dir", "false"),
        ("savestates_in_content_dir", "false"),
        ("cheevos_hardcore_mode_enable", "false"),
        ("cheevos_visibility_lboard_start", "false"),
        ("cheevos_visibility_lboard_submit", "false"),
        ("cheevos_visibility_lboard_trackers", "false"),
    ];

    let nickname = romm_username.trim();
    if !nickname.is_empty() {
        desired.push(("netplay_nickname".to_string(), nickname.to_string()));
    }

    if enable_fullscreen {
        desired.push(("video_fullscreen".to_string(), "true".to_string()));
    }

    if let Some(ra) = ra {
        let ra_username = ra.username().trim();
        let ra_token = ra.token().trim();
        if !ra_username.is_empty() && !ra_token.is_empty() {
            desired.push(("cheevos_enable".to_string(), "true".to_string()));
            desired.push(("cheevos_username".to_string(), ra_username.to_string()));
            desired.push(("cheevos_token".to_string(), ra_token.to_string()));
        }
    }

    desired
}

/// `ensure_retroarch_save_location_settings` (retroarch.py:232-355).
///
/// `romm_username` is the netplay nickname; `ra` is the RetroAchievements
/// pair — two DISTINCT parameters (the Python rebinds one local variable
/// mid-function at retroarch.py:294; the port keeps them separate). A blank
/// path (no candidates) reports [`EnsureResult::unchanged`].
pub fn ensure_settings(
    emulator_path: &str,
    enable_fullscreen: bool,
    romm_username: &str,
    ra: Option<&RaCredentials>,
) -> EnsureResult {
    let Some((settings, target)) = resolve_target(emulator_path) else {
        return EnsureResult::unchanged();
    };
    let desired = build_desired(&settings, enable_fullscreen, romm_username, ra);
    write_desired(&target, &settings, &desired)
}

/// Spec deviation D2 (RA-keys-only fan-out) — no direct Python counterpart:
/// a narrow writer for the three RetroAchievements credential keys alone,
/// reusing [`ensure_settings`]'s candidate/target resolution and
/// [`writers::flat_cfg`] write policy without its other 19 managed keys.
/// Only `cheevos_enable`, `cheevos_username`, `cheevos_token` — never
/// `savefile_directory`, the sort booleans, or the four suppression keys.
/// A no-op ([`EnsureResult::unchanged`]) when either RA field is blank
/// after trimming, checked before any candidate resolution.
pub fn ensure_ra_credentials(emulator_path: &str, ra: &RaCredentials) -> EnsureResult {
    let ra_username = ra.username().trim();
    let ra_token = ra.token().trim();
    if ra_username.is_empty() || ra_token.is_empty() {
        return EnsureResult::unchanged();
    }

    let Some((settings, target)) = resolve_target(emulator_path) else {
        return EnsureResult::unchanged();
    };

    let desired = crate::desired![
        ("cheevos_enable", "true"),
        ("cheevos_username", ra_username),
        ("cheevos_token", ra_token),
    ];

    write_desired(&target, &settings, &desired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes access to `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`HOME` across
    /// every test in this module — they are process-global, so two tests
    /// mutating them concurrently would race (and, per `std::env::set_var`'s
    /// safety contract, that race is UB, not just flakiness). Every test
    /// here takes this lock, even ones that do not override the variables
    /// themselves, because `config_path_candidates` always reads them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Sets each `(var, value)` pair — `None` removes the variable — for
    /// the guard's lifetime and restores whatever preceded it on drop, so a
    /// panic mid-test can never leak an override into another test. Callers
    /// must hold `ENV_LOCK` for the guard's whole lifetime.
    struct EnvGuard {
        previous: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let previous = pairs
                .iter()
                .map(|&(var, _)| (var, std::env::var(var).ok()))
                .collect();
            for &(var, value) in pairs {
                match value {
                    // SAFETY: `ENV_LOCK` is held for the guard's entire
                    // lifetime by every caller in this module.
                    Some(v) => unsafe { std::env::set_var(var, v) },
                    None => unsafe { std::env::remove_var(var) },
                }
            }
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (var, value) in &self.previous {
                match value {
                    // SAFETY: see `EnvGuard::set` above.
                    Some(v) => unsafe { std::env::set_var(var, v) },
                    None => unsafe { std::env::remove_var(var) },
                }
            }
        }
    }

    /// `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`HOME` all pointed at `dir`, mirroring
    /// `tests/test_retroarch_config.py`'s `_isolated_env`.
    fn isolated_env(dir: &Path) -> EnvGuard {
        let dir_str = dir.to_str().unwrap();
        EnvGuard::set(&[
            ("XDG_CONFIG_HOME", Some(dir_str)),
            ("XDG_DATA_HOME", Some(dir_str)),
            ("HOME", Some(dir_str)),
        ])
    }

    /// A `<temp>/RetroArch/retroarch.exe` emulator file and its sibling
    /// `retroarch.cfg` path (not yet created — callers write it themselves).
    fn setup_emulator(temp: &Path) -> (String, PathBuf) {
        let dir = temp.join("RetroArch");
        std::fs::create_dir_all(&dir).unwrap();
        let emulator_path = dir.join("retroarch.exe");
        std::fs::write(&emulator_path, b"").unwrap();
        let config_path = dir.join("retroarch.cfg");
        (emulator_path.to_string_lossy().to_string(), config_path)
    }

    // --- config_path_candidates ---------------------------------------------

    #[test]
    fn candidates_use_parent_for_a_file_path_and_self_for_a_directory() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("retroarch");
        std::fs::write(&file, b"").unwrap();

        let file_candidates = config_path_candidates(file.to_str().unwrap());
        assert_eq!(file_candidates[0], dir.join("retroarch.cfg"));

        let dir_candidates = config_path_candidates(dir.to_str().unwrap());
        assert_eq!(dir_candidates[0], dir.join("retroarch.cfg"));
    }

    #[test]
    fn candidates_use_parent_for_a_nonexistent_suffixed_path() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let missing = temp.path().join("RetroArch").join("retroarch.exe");
        let candidates = config_path_candidates(missing.to_str().unwrap());
        assert_eq!(
            candidates[0],
            temp.path().join("RetroArch").join("retroarch.cfg")
        );
    }

    #[test]
    fn candidates_are_deduped_case_insensitively() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let temp_path = temp.path();
        let xdg_data = temp_path.join("other");
        let alt_home = temp_path.join("otherhome");
        let _guard = EnvGuard::set(&[
            ("XDG_CONFIG_HOME", Some(temp_path.to_str().unwrap())),
            ("XDG_DATA_HOME", Some(xdg_data.to_str().unwrap())),
            ("HOME", Some(alt_home.to_str().unwrap())),
        ]);

        // Not created on disk — candidates never check existence.
        let dir = temp_path.join("RetroArch");

        let candidates = config_path_candidates(dir.to_str().unwrap());

        assert_eq!(
            candidates,
            vec![
                dir.join("retroarch.cfg"),
                dir.join("config").join("retroarch.cfg"),
                xdg_data.join("retroarch").join("retroarch.cfg"),
                alt_home
                    .join(".config")
                    .join("retroarch")
                    .join("retroarch.cfg"),
            ],
            "the XDG_CONFIG_HOME candidate (<temp>/retroarch/retroarch.cfg) must fold onto \
             candidates[0] (<temp>/RetroArch/retroarch.cfg) case-insensitively"
        );
    }

    #[test]
    fn candidates_are_empty_for_a_blank_path() {
        assert_eq!(config_path_candidates(""), Vec::<PathBuf>::new());
    }

    // --- directory_settings --------------------------------------------------

    #[test]
    fn directory_settings_strips_one_quote_pair() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            "savefile_directory = \"/mnt/saves\"\nsavestate_directory = '/mnt/states'\n",
        )
        .unwrap();

        let settings = directory_settings(&emulator_path);
        assert_eq!(settings.savefile_directory, "/mnt/saves");
        assert_eq!(settings.savestate_directory, "/mnt/states");
    }

    #[test]
    fn directory_settings_treats_default_as_unset() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            "savefile_directory = \"default\"\nsavestate_directory = \"DEFAULT\"\n",
        )
        .unwrap();

        let settings = directory_settings(&emulator_path);
        assert_eq!(settings.savefile_directory, "");
        assert_eq!(settings.savestate_directory, "");
    }

    #[test]
    fn directory_settings_parses_the_six_booleans() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            concat!(
                "savefiles_in_content_dir = \"true\"\n",
                "savestates_in_content_dir = \"YES\"\n",
                "sort_savefiles_enable = \"on\"\n",
                "sort_savestates_enable = \"1\"\n",
                "sort_savefiles_by_content_enable = \"false\"\n",
                "sort_savestates_by_content_enable = \"nope\"\n",
            ),
        )
        .unwrap();

        let settings = directory_settings(&emulator_path);
        assert!(settings.savefiles_in_content_dir);
        assert!(settings.savestates_in_content_dir);
        assert!(settings.sort_savefiles_enable);
        assert!(settings.sort_savestates_enable);
        assert!(!settings.sort_savefiles_by_content_enable);
        assert!(!settings.sort_savestates_by_content_enable);
    }

    #[test]
    fn directory_settings_skips_a_candidate_with_no_parseable_line() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        // candidate 0 (root/retroarch.cfg): exists but nothing parseable.
        std::fs::write(&config_path, "# just a comment\n\n").unwrap();
        // candidate 1 (root/config/retroarch.cfg): a real setting.
        let config_dir = temp.path().join("RetroArch").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("retroarch.cfg"),
            "savefile_directory = \"/mnt/saves\"\n",
        )
        .unwrap();

        let settings = directory_settings(&emulator_path);
        assert_eq!(settings.savefile_directory, "/mnt/saves");
        assert_eq!(
            settings.config_path,
            config_dir.join("retroarch.cfg").to_string_lossy()
        );
    }

    // --- ensure_settings -------------------------------------------------

    #[test]
    fn ensure_writes_defaults_and_disables_all_six_sort_flags() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            concat!(
                "savefile_directory = \"default\"\n",
                "savestate_directory = \"default\"\n",
                "sort_savefiles_enable = \"true\"\n",
                "sort_savestates_enable = \"true\"\n",
                "sort_savefiles_by_content_enable = \"true\"\n",
                "sort_savestates_by_content_enable = \"true\"\n",
                "savefiles_in_content_dir = \"true\"\n",
                "savestates_in_content_dir = \"true\"\n",
            ),
        )
        .unwrap();

        let result = ensure_settings(&emulator_path, false, "", None);
        let settings = directory_settings(&emulator_path);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert_eq!(settings.savefile_directory, "saves");
        assert_eq!(settings.savestate_directory, "states");
        assert!(!settings.sort_savefiles_enable);
        assert!(!settings.sort_savestates_enable);
        assert!(!settings.sort_savefiles_by_content_enable);
        assert!(!settings.sort_savestates_by_content_enable);
        assert!(!settings.savefiles_in_content_dir);
        assert!(!settings.savestates_in_content_dir);
        assert!(text.contains("savefile_directory = \"saves\""));
        assert!(text.contains("savestate_directory = \"states\""));
    }

    #[test]
    fn ensure_preserves_explicit_directories() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            concat!(
                "savefile_directory = \"D:/Custom Saves\"\n",
                "savestate_directory = \"E:/Custom States\"\n",
                "sort_savefiles_enable = \"true\"\n",
                "savestates_in_content_dir = \"true\"\n",
            ),
        )
        .unwrap();

        let result = ensure_settings(&emulator_path, false, "", None);
        let settings = directory_settings(&emulator_path);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert_eq!(settings.savefile_directory, "D:/Custom Saves");
        assert_eq!(settings.savestate_directory, "E:/Custom States");
        assert!(!settings.sort_savefiles_enable);
        assert!(!settings.savestates_in_content_dir);
        assert!(text.contains("savefile_directory = \"D:/Custom Saves\""));
        assert!(text.contains("savestate_directory = \"E:/Custom States\""));
    }

    #[test]
    fn ensure_preserves_an_existing_audio_volume_line_verbatim() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(&config_path, "audio_volume = \"3.500000\"\n").unwrap();

        ensure_settings(&emulator_path, false, "", None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(text.contains("audio_volume = \"3.500000\""));
        assert!(
            !text.contains("-18.000000"),
            "the user's own volume must never be overwritten"
        );
    }

    #[test]
    fn ensure_writes_fullscreen_and_ra_credentials_when_enabled() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            "video_fullscreen = \"false\"\ncheevos_enable = \"false\"\n",
        )
        .unwrap();

        let ra = RaCredentials::new("retro_user", "retro_token");
        let result = ensure_settings(&emulator_path, true, "", Some(&ra));
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("video_fullscreen = \"true\""));
        assert!(text.contains("cheevos_enable = \"true\""));
        assert!(text.contains("cheevos_username = \"retro_user\""));
        assert!(text.contains("cheevos_token = \"retro_token\""));
    }

    #[test]
    fn ensure_omits_video_fullscreen_when_disabled() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(&config_path, "video_fullscreen = \"true\"\n").unwrap();

        ensure_settings(&emulator_path, false, "", None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        // enable_fullscreen=false omits the key from desired entirely, so
        // the existing line survives untouched rather than being flipped.
        assert!(text.contains("video_fullscreen = \"true\""));
    }

    #[test]
    fn ensure_writes_the_four_cheevos_suppression_keys_unconditionally() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());
        std::fs::write(&config_path, "").unwrap();

        let result = ensure_settings(&emulator_path, false, "", None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("cheevos_hardcore_mode_enable = \"false\""));
        assert!(text.contains("cheevos_visibility_lboard_start = \"false\""));
        assert!(text.contains("cheevos_visibility_lboard_submit = \"false\""));
        assert!(text.contains("cheevos_visibility_lboard_trackers = \"false\""));
    }

    #[test]
    fn ensure_skips_ra_keys_when_only_the_username_is_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());
        std::fs::write(&config_path, "").unwrap();

        let ra = RaCredentials::new("retro_user", "");
        ensure_settings(&emulator_path, false, "", Some(&ra));
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(!text.contains("cheevos_enable"));
        assert!(!text.contains("cheevos_username"));
        assert!(!text.contains("cheevos_token"));
    }

    #[test]
    fn ensure_writes_netplay_nickname_from_the_romm_username_not_the_ra_one() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());
        std::fs::write(&config_path, "").unwrap();

        let ra = RaCredentials::new("sixdd6", "ra-token-FAKE");
        let result = ensure_settings(&emulator_path, false, "six", Some(&ra));
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(
            text.contains("netplay_nickname = \"six\""),
            "netplay_nickname must come from the RomM username, not RA: {text}"
        );
        assert!(
            text.contains("cheevos_username = \"sixdd6\""),
            "cheevos_username must come from the RA username, not RomM: {text}"
        );
    }

    #[test]
    fn ensure_is_idempotent_on_a_second_run() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());
        std::fs::write(&config_path, "").unwrap();

        let ra = RaCredentials::new("retro_user", "retro_token");
        let first = ensure_settings(&emulator_path, true, "six", Some(&ra));
        assert!(first.changed);

        let second = ensure_settings(&emulator_path, true, "six", Some(&ra));
        assert!(!second.changed, "a second identical run must be a no-op");
    }

    #[test]
    fn ensure_returns_unchanged_for_a_blank_path() {
        let result = ensure_settings("", false, "", None);
        assert_eq!(result, EnsureResult::unchanged());
    }

    // --- ensure_ra_credentials (D2) ---------------------------------------

    #[test]
    fn ensure_ra_credentials_touches_only_the_three_cheevos_keys() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, config_path) = setup_emulator(temp.path());

        std::fs::write(
            &config_path,
            "savefile_directory = \"/mnt/saves\"\nmy_unmanaged_key = \"keep\"\n",
        )
        .unwrap();

        let ra = RaCredentials::new("retro_user", "retro_token");
        let result = ensure_ra_credentials(&emulator_path, &ra);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(
            text.contains("savefile_directory = \"/mnt/saves\""),
            "the pre-existing directory setting must survive byte-identically: {text}"
        );
        assert!(
            text.contains("my_unmanaged_key = \"keep\""),
            "the sentinel unmanaged key must survive byte-identically: {text}"
        );
        assert!(text.contains("cheevos_enable = \"true\""));
        assert!(text.contains("cheevos_username = \"retro_user\""));
        assert!(text.contains("cheevos_token = \"retro_token\""));

        // Narrowness: none of the full writer's other managed keys appear.
        assert!(!text.contains("sort_savefiles_enable"));
        assert!(!text.contains("video_windowed_fullscreen"));
        assert!(!text.contains("discord_enable"));
        assert!(!text.contains("cheevos_hardcore_mode_enable"));
    }

    #[test]
    fn ensure_ra_credentials_is_a_no_op_when_either_field_is_blank() {
        let blank_token = RaCredentials::new("retro_user", "");
        assert_eq!(
            ensure_ra_credentials("/does/not/matter", &blank_token),
            EnsureResult::unchanged()
        );

        let blank_username = RaCredentials::new("", "retro_token");
        assert_eq!(
            ensure_ra_credentials("/does/not/matter", &blank_username),
            EnsureResult::unchanged()
        );
    }
}
