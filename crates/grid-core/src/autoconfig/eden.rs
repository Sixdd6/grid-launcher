//! Eden's `qt-config.ini`: candidate discovery and the Qt-annotated section
//! writer.
//!
//! Ports `grid_launcher/emulator/eden.py`'s `eden_config_path_candidates`
//! (eden.py:206-213) and `ensure_eden_settings` (eden.py:216-280). See
//! `docs/porting/05-emulator-autoconfig.md` ("Eden") for the behavior
//! contract.

use std::path::{Path, PathBuf};

use super::{paths, writers, EnsureResult};

/// `eden_config_path_candidates` (eden.py:206-213).
///
/// **Unlike every other candidate-list function in this crate, the portable
/// candidate is built from the RAW `emulator_path` text — no `~` expansion,
/// no trim, no blank check.** `Path(emulator_path_text).parent` in Python is
/// `PathBuf::from(emulator_path).parent()`, matched here exactly. Order:
/// `<exe parent>/user/config/qt-config.ini`;
/// `%APPDATA%/eden/config/qt-config.ini` when `APPDATA` is set and non-blank
/// once trimmed (`~`-expanded, unlike the portable candidate) — inserted
/// BETWEEN the portable and the XDG candidate;
/// `<XDG_CONFIG_HOME>/eden/qt-config.ini`. Not deduplicated — the Python
/// reference never dedupes this list.
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf> {
    let raw = PathBuf::from(emulator_path);
    let parent = raw.parent().unwrap_or(Path::new(""));
    let portable = parent.join("user").join("config").join("qt-config.ini");
    let linux = paths::xdg_config_home().join("eden").join("qt-config.ini");

    if let Ok(appdata) = std::env::var("APPDATA") {
        let trimmed = appdata.trim();
        if !trimmed.is_empty() {
            let windows = paths::expand_user(trimmed)
                .join("eden")
                .join("config")
                .join("qt-config.ini");
            return vec![portable, windows, linux];
        }
    }

    vec![portable, linux]
}

/// Create `<emulator_dir>/user/` when it does not exist — the portable
/// marker (eden.py:218-224), same rule [`super::azahar`] uses. Unlike
/// [`config_path_candidates`], this DOES trim and `~`-expand
/// `emulator_path` and uses the dir-or-parent rule. A no-op for a blank
/// `emulator_path`; any creation failure is swallowed.
fn maybe_create_user_dir(emulator_path: &str) {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return;
    }
    let expanded = paths::expand_user(trimmed);
    let dir = if expanded.is_dir() {
        expanded
    } else {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let user_dir = dir.join("user");
    if !user_dir.exists() {
        let _ = std::fs::create_dir_all(&user_dir);
    }
}

/// Read `path` (empty string when it does not exist), run `apply`, and
/// write the result back only when `apply` reports a change — creating the
/// parent directory first. `Err` on any I/O failure.
fn write_if_changed(
    path: &Path,
    apply: impl FnOnce(&str) -> (String, bool),
) -> std::io::Result<bool> {
    let content = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    let (new_content, changed) = apply(&content);
    if changed {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &new_content)?;
    }
    Ok(changed)
}

/// `ensure_eden_settings` (eden.py:216-280).
///
/// Creates `<emulator_dir>/user/` first (see [`maybe_create_user_dir`]).
/// [`EnsureResult::unchanged`] when [`config_path_candidates`] is empty
/// (never happens in practice — it always returns at least the portable and
/// XDG candidates — but the Python reference guards it, so this does too).
/// Otherwise selects the first EXISTING candidate, falling back to the
/// first, and overwrites every key below unconditionally via
/// [`writers::eden_annotated_section`], which GENERATES each
/// `key\default=false` annotation line rather than taking it as a desired
/// key: `[UI]` `enable_discord_presence`=`false`, `confirmStop`=`2`,
/// `fullscreen`=`true`, `firstStart`=`false`,
/// `pauseWhenInBackground`=`true`, `enable_gamemode`=`true`,
/// `theme`=`colorful_dark`, `check_for_updates`=`false`;
/// `[WebService] enable_telemetry`=`false`; `[Audio] volume`=`40`,
/// `muteWhenInBackground`=`true`; `[Renderer] scaling_filter`=`6`.
///
/// The whole read/edit/write body is one fallible scope: any I/O error
/// reports [`EnsureResult::unchanged`].
pub fn ensure_settings(emulator_path: &str) -> EnsureResult {
    maybe_create_user_dir(emulator_path);

    let candidates = config_path_candidates(emulator_path);
    if candidates.is_empty() {
        return EnsureResult::unchanged();
    }
    let selected = candidates
        .iter()
        .find(|c| c.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone());

    match write_if_changed(&selected, |content| {
        let (content, c1) = writers::eden_annotated_section(
            content,
            "UI",
            &crate::desired![
                ("enable_discord_presence", "false"),
                ("confirmStop", "2"),
                ("fullscreen", "true"),
                ("firstStart", "false"),
                ("pauseWhenInBackground", "true"),
                ("enable_gamemode", "true"),
                ("theme", "colorful_dark"),
                ("check_for_updates", "false"),
            ],
        );
        let (content, c2) = writers::eden_annotated_section(
            &content,
            "WebService",
            &crate::desired![("enable_telemetry", "false")],
        );
        let (content, c3) = writers::eden_annotated_section(
            &content,
            "Audio",
            &crate::desired![("volume", "40"), ("muteWhenInBackground", "true")],
        );
        let (content, c4) = writers::eden_annotated_section(
            &content,
            "Renderer",
            &crate::desired![("scaling_filter", "6")],
        );
        (content, c1 || c2 || c3 || c4)
    }) {
        Ok(changed) => EnsureResult::at(selected, changed),
        Err(_) => EnsureResult::unchanged(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    fn make_exe(temp: &Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("Eden");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("eden.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    /// `APPDATA` unset so the XDG candidate is reachable; `XDG_CONFIG_HOME`
    /// and `HOME` pointed at temp dirs so nothing touches the real user's
    /// home.
    fn isolated_env(temp: &Path) -> EnvGuard {
        EnvGuard::set(&[
            ("APPDATA", None),
            (
                "XDG_CONFIG_HOME",
                Some(temp.join("xdg-config").to_str().unwrap()),
            ),
            ("HOME", Some(temp.join("home").to_str().unwrap())),
        ])
    }

    #[test]
    fn eden_annotation_format_has_no_spaces_and_the_value_line_does() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        let config_path = dir.join("user").join("config").join("qt-config.ini");
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("enable_discord_presence\\default=false"));
        assert!(text.contains("enable_discord_presence = false"));
        assert!(!text.contains("enable_discord_presence\\default = false"));
        assert!(!text.contains("enable_discord_presence=false"));
    }

    #[test]
    fn eden_writes_confirm_stop_two() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        let config_path = dir.join("user").join("config").join("qt-config.ini");
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("confirmStop\\default=false"));
        assert!(text.contains("confirmStop = 2"));
        assert!(!text.contains("confirm_before_closing"));
    }

    #[test]
    fn eden_overwrites_an_existing_audio_volume() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let config_path = dir.join("user").join("config").join("qt-config.ini");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, "[Audio]\nvolume = 80\n").unwrap();

        let result = ensure_settings(exe.to_str().unwrap());

        assert!(result.changed);
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("volume = 40"));
        assert!(text.contains("volume\\default=false"));
        assert!(!text.contains("volume = 80"));
        assert!(text.contains("muteWhenInBackground = true"));
    }

    #[test]
    fn eden_rewrites_a_malformed_existing_annotation_line() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let config_path = dir.join("user").join("config").join("qt-config.ini");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &config_path,
            "[UI]\nconfirmStop\\default = true\nconfirmStop = 1\n",
        )
        .unwrap();

        let result = ensure_settings(exe.to_str().unwrap());

        assert!(result.changed);
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("confirmStop\\default=false"));
        assert!(text.contains("confirmStop = 2"));
        assert!(!text.contains("confirmStop\\default = true"));
        assert!(!text.contains("confirmStop = 1\n"));
    }

    #[test]
    fn eden_is_idempotent() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path());

        let first = ensure_settings(exe.to_str().unwrap());
        let second = ensure_settings(exe.to_str().unwrap());

        assert!(first.changed);
        assert!(!second.changed);
    }

    #[test]
    fn eden_windows_candidate_sits_between_portable_and_xdg() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set(&[
            (
                "APPDATA",
                Some(temp.path().join("appdata").to_str().unwrap()),
            ),
            (
                "XDG_CONFIG_HOME",
                Some(temp.path().join("xdg-config").to_str().unwrap()),
            ),
            ("HOME", Some(temp.path().join("home").to_str().unwrap())),
        ]);

        let candidates = config_path_candidates("/nonexistent/eden.exe");

        assert_eq!(candidates.len(), 3);
        assert_eq!(
            candidates[0],
            PathBuf::from("/nonexistent")
                .join("user")
                .join("config")
                .join("qt-config.ini")
        );
        assert_eq!(
            candidates[1],
            temp.path()
                .join("appdata")
                .join("eden")
                .join("config")
                .join("qt-config.ini")
        );
        assert_eq!(
            candidates[2],
            temp.path()
                .join("xdg-config")
                .join("eden")
                .join("qt-config.ini")
        );
    }

    #[test]
    fn eden_creates_the_user_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        assert!(dir.join("user").is_dir());
    }

    #[test]
    fn eden_does_not_recreate_it() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let marker = dir.join("user").join("marker.txt");
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(&marker, "x").unwrap();

        ensure_settings(exe.to_str().unwrap());

        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "x");
    }
}
