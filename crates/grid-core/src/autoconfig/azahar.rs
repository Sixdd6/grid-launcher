//! Azahar's `qt-config.ini`: candidate discovery and the widened-charset
//! section writer with its `\default` companion keys.
//!
//! Ports `grid_launcher/emulator/azahar.py`'s `azahar_config_path_candidates`
//! (azahar.py:129-141) and `ensure_azahar_settings` (azahar.py:144-216). See
//! `docs/porting/05-emulator-autoconfig.md` ("Azahar") for the behavior
//! contract.

use std::path::{Path, PathBuf};

use super::{paths, writers, EnsureResult};

/// `azahar_config_path_candidates` (azahar.py:129-141).
///
/// A blank (trimmed) `emulator_path` yields no candidates. Otherwise, using
/// `emulator_path.parent()` UNCONDITIONALLY — no is-dir check, unlike the
/// `user/` directory created in [`ensure_settings`] — the candidates are, in
/// order: `<parent>/user/config/qt-config.ini`, `<parent>/qt-config.ini`,
/// `%APPDATA%/Azahar/qt-config.ini` (when `APPDATA` is set and non-blank; no
/// platform check), `~/.config/Azahar/qt-config.ini`,
/// `~/.var/app/org.azahar_emu.Azahar/config/Azahar/qt-config.ini`. Not
/// deduplicated — the Python reference never dedupes this list.
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf> {
    if emulator_path.trim().is_empty() {
        return Vec::new();
    }

    let expanded = paths::expand_user(emulator_path);
    let parent = expanded.parent().unwrap_or(Path::new(""));

    let mut candidates = vec![
        parent.join("user").join("config").join("qt-config.ini"),
        parent.join("qt-config.ini"),
    ];

    if let Ok(appdata) = std::env::var("APPDATA") {
        let trimmed = appdata.trim();
        if !trimmed.is_empty() {
            candidates.push(
                paths::expand_user(trimmed)
                    .join("Azahar")
                    .join("qt-config.ini"),
            );
        }
    }

    let home = paths::home_dir().unwrap_or_default();
    candidates.push(home.join(".config").join("Azahar").join("qt-config.ini"));
    candidates.push(
        home.join(".var")
            .join("app")
            .join("org.azahar_emu.Azahar")
            .join("config")
            .join("Azahar")
            .join("qt-config.ini"),
    );

    candidates
}

/// Create `<emulator_dir>/user/` when it does not exist — the portable
/// marker (azahar.py:146-152, same rule eden.rs uses). `emulator_dir` is
/// `emulator_path` itself when it is a directory, else its parent. A no-op
/// for a blank `emulator_path`; any creation failure is swallowed.
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

/// `ensure_azahar_settings` (azahar.py:144-216).
///
/// Creates `<emulator_dir>/user/` first (see [`maybe_create_user_dir`]).
/// [`EnsureResult::unchanged`] when [`config_path_candidates`] is empty
/// (blank path). Otherwise selects the first EXISTING candidate, falling
/// back to the first, and overwrites every key below unconditionally via
/// [`writers::azahar_section`] — every real key preceded by its
/// `<key>\default` companion, written as an ORDINARY key (which is exactly
/// why the widened key charset exists):
///
/// - `[Renderer]`: `resolution_factor\default`=`false`,
///   `resolution_factor`=`4`, `use_vsync\default`=`false`, `use_vsync`=`true`.
/// - `[Audio]`: `volume\default`=`false`, `volume`=`0.4`.
/// - `[UI]`: `enable_discord_presence\default`=`false`,
///   `enable_discord_presence`=`false`, `confirmClose\default`=`false`,
///   `confirmClose`=`false`, `fullscreen\default`=`false`,
///   `fullscreen`=`true`, `pauseWhenInBackground\default`=`false`,
///   `pauseWhenInBackground`=`true`, `hideInactiveMouse\default`=`false`,
///   `hideInactiveMouse`=`true`, the Fullscreen shortcut
///   (`Shortcuts\Main%20Window\Fullscreen\KeySeq`, default `false`/value
///   `F1`), the Stop Emulation shortcut (same pattern, value `Escape`).
///
/// Any I/O error along the way reports [`EnsureResult::unchanged`].
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
        let (content, c1) = writers::azahar_section(
            content,
            "Renderer",
            &crate::desired![
                (r"resolution_factor\default", "false"),
                ("resolution_factor", "4"),
                (r"use_vsync\default", "false"),
                ("use_vsync", "true"),
            ],
        );
        let (content, c2) = writers::azahar_section(
            &content,
            "Audio",
            &crate::desired![(r"volume\default", "false"), ("volume", "0.4")],
        );
        let (content, c3) = writers::azahar_section(
            &content,
            "UI",
            &crate::desired![
                (r"enable_discord_presence\default", "false"),
                ("enable_discord_presence", "false"),
                (r"confirmClose\default", "false"),
                ("confirmClose", "false"),
                (r"fullscreen\default", "false"),
                ("fullscreen", "true"),
                (r"pauseWhenInBackground\default", "false"),
                ("pauseWhenInBackground", "true"),
                (r"hideInactiveMouse\default", "false"),
                ("hideInactiveMouse", "true"),
                (
                    r"Shortcuts\Main%20Window\Fullscreen\KeySeq\default",
                    "false"
                ),
                (r"Shortcuts\Main%20Window\Fullscreen\KeySeq", "F1"),
                (
                    r"Shortcuts\Main%20Window\Stop%20Emulation\KeySeq\default",
                    "false"
                ),
                (r"Shortcuts\Main%20Window\Stop%20Emulation\KeySeq", "Escape"),
            ],
        );
        (content, c1 || c2 || c3)
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
        let dir = temp.join("Azahar");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("azahar.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    fn isolated_env(temp: &Path) -> EnvGuard {
        EnvGuard::set(&[
            ("APPDATA", Some(temp.join("appdata").to_str().unwrap())),
            ("HOME", Some(temp.join("home").to_str().unwrap())),
        ])
    }

    #[test]
    fn azahar_creates_the_user_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        assert!(dir.join("user").is_dir());
    }

    #[test]
    fn azahar_does_not_recreate_it() {
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

    #[test]
    fn azahar_writes_companion_keys_without_duplication() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());
        ensure_settings(exe.to_str().unwrap());

        let config_path = dir.join("user").join("config").join("qt-config.ini");
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert_eq!(text.matches(r"resolution_factor\default").count(), 1);
        assert_eq!(text.matches(r"use_vsync\default").count(), 1);
        assert_eq!(text.matches(r"volume\default").count(), 1);
        assert!(text.contains("resolution_factor = 4"));
        assert!(text.contains(r"resolution_factor\default = false"));
        assert!(text.contains("use_vsync = true"));
        assert!(text.contains("volume = 0.4"));
    }

    #[test]
    fn azahar_manages_the_shortcut_keys_with_backslashes_and_percent() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        let config_path = dir.join("user").join("config").join("qt-config.ini");
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains(r"Shortcuts\Main%20Window\Fullscreen\KeySeq\default = false"));
        assert!(text.contains(r"Shortcuts\Main%20Window\Fullscreen\KeySeq = F1"));
        assert!(text.contains(r"Shortcuts\Main%20Window\Stop%20Emulation\KeySeq\default = false"));
        assert!(text.contains(r"Shortcuts\Main%20Window\Stop%20Emulation\KeySeq = Escape"));
        assert!(text.contains("[UI]"));
        assert!(text.contains("fullscreen = true"));
        assert!(text.contains("pauseWhenInBackground = true"));
        assert!(text.contains("hideInactiveMouse = true"));
    }

    #[test]
    fn azahar_is_idempotent() {
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
    fn azahar_blank_path_is_unchanged() {
        let _lock = crate::test_env::lock();
        assert_eq!(ensure_settings(""), EnsureResult::unchanged());
        assert_eq!(ensure_settings("   "), EnsureResult::unchanged());
    }

    #[test]
    fn azahar_config_path_candidates_include_appdata_and_home_fallbacks() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let candidates = config_path_candidates("/nonexistent/azahar.exe");

        assert_eq!(
            candidates[0],
            PathBuf::from("/nonexistent")
                .join("user")
                .join("config")
                .join("qt-config.ini")
        );
        assert_eq!(
            candidates[1],
            PathBuf::from("/nonexistent").join("qt-config.ini")
        );
        assert!(candidates.contains(
            &temp
                .path()
                .join("appdata")
                .join("Azahar")
                .join("qt-config.ini")
        ));
        assert!(candidates.contains(
            &temp
                .path()
                .join("home")
                .join(".config")
                .join("Azahar")
                .join("qt-config.ini")
        ));
    }
}
