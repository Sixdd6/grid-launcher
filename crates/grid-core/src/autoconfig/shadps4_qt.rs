//! The ShadPS4 Qt launcher's version pointer, plus shadPS4's portable
//! `user/` directory.
//!
//! The Qt launcher reads `<cwd>/launcher/qt_ui.ini` whenever that `launcher/`
//! directory exists, so GRID writes the file next to the launcher executable:
//! GRID's standalone launch sets the child's working directory to the
//! executable's own parent (`launch/spawn.rs`). The INI group is
//! `[version_manager]` and the key GRID owns is `versionSelected`, the full
//! path of the shadPS4 executable the launcher should run. Nothing else in
//! the file is touched — in particular `versionPath`, the folder the launcher
//! downloads its OWN emulator builds into, is left to the launcher.
//!
//! Values are written with forward slashes on every OS. Qt's QSettings
//! escapes `\` in INI values, and a forward-slash path is valid on Windows
//! too, so `C:\shadps4\shadps4.exe` is written `C:/shadps4/shadps4.exe`.
//! The lines are `key=value` with no spaces and no `key\default=`
//! annotations, Qt's runtime form ([`writers::qt_plain_section`]).
//!
//! Both shadPS4 and the launcher prefer `<cwd>/user` as their user/save
//! directory when it exists, so [`ensure_portable_user_dir`] creates
//! `<shadps4-dir>/user/`; the catalog profile's
//! `save_directories: ["user/home/1000/savedata"]` resolve there.
//!
//! Known limitation: the launcher has no known setting for its user
//! directory and lives in a DIFFERENT directory from shadPS4, so the
//! launcher's own game list may come from the XDG/APPDATA location while
//! games GRID launches (through the shadPS4 entry) use the portable one.

use std::path::{Path, PathBuf};

use super::{paths, writers, EnsureResult};
use crate::desired;

/// `path.trim()` expanded, dir-or-parent — `None` for a blank path, the
/// same rule every sibling module uses (`paths::emulator_dir`).
fn resolve_dir(emulator_path: &str) -> Option<PathBuf> {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    paths::emulator_dir(&paths::expand_user(trimmed))
}

/// The value form Qt's INI writer round-trips without escaping: `~`
/// expanded, every `\` turned into `/`.
fn ini_value(path_text: &str) -> String {
    paths::expand_user(path_text.trim())
        .to_string_lossy()
        .replace('\\', "/")
}

/// An unreadable existing file degrades to "no write happened" rather than
/// propagating an error (the `read_guarded` rule in `ppsspp.rs`).
fn read_guarded(path: &Path) -> Option<String> {
    if !path.exists() {
        return Some(String::new());
    }
    std::fs::read_to_string(path).ok()
}

/// Point the Qt launcher at the installed shadPS4 build: writes
/// `[version_manager] versionSelected=<shadps4 exe>` into
/// `<qt-dir>/launcher/qt_ui.ini`, creating `launcher/` when missing.
///
/// Every other section, and every other key in `[version_manager]`, is
/// preserved. A blank path on either side, or any I/O error, yields
/// [`EnsureResult::unchanged`]. A second call with the same inputs reports
/// `changed = false` and leaves the bytes identical.
pub fn ensure_version_selected(qt_launcher_path: &str, shadps4_exe_path: &str) -> EnsureResult {
    let Some(qt_dir) = resolve_dir(qt_launcher_path) else {
        return EnsureResult::unchanged();
    };
    if shadps4_exe_path.trim().is_empty() {
        return EnsureResult::unchanged();
    }

    let launcher_dir = qt_dir.join("launcher");
    if std::fs::create_dir_all(&launcher_dir).is_err() {
        return EnsureResult::unchanged();
    }
    let ini_path = launcher_dir.join("qt_ui.ini");

    let Some(raw) = read_guarded(&ini_path) else {
        return EnsureResult::unchanged();
    };
    let (updated, changed) = writers::qt_plain_section(
        &raw,
        "version_manager",
        &desired![("versionSelected", ini_value(shadps4_exe_path))],
    );
    if !changed {
        return EnsureResult::at(ini_path, false);
    }
    if std::fs::write(&ini_path, updated).is_err() {
        return EnsureResult::unchanged();
    }
    EnsureResult::at(ini_path, true)
}

/// Create `<shadps4-dir>/user/` so shadPS4 runs portable. `changed` is true
/// only when the directory did not already exist; a blank path or any I/O
/// error yields [`EnsureResult::unchanged`].
pub fn ensure_portable_user_dir(shadps4_path: &str) -> EnsureResult {
    let Some(dir) = resolve_dir(shadps4_path) else {
        return EnsureResult::unchanged();
    };
    let user_dir = dir.join("user");
    if user_dir.is_dir() {
        return EnsureResult::at(user_dir, false);
    }
    if std::fs::create_dir_all(&user_dir).is_err() {
        return EnsureResult::unchanged();
    }
    EnsureResult::at(user_dir, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<temp>/ShadPS4 Qt Launcher-latest/shadPS4QtLauncher-qt.AppImage`
    /// and the `launcher/qt_ui.ini` it targets.
    fn qt_exe(temp: &Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("ShadPS4 Qt Launcher-latest");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("shadPS4QtLauncher-qt.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let ini = dir.join("launcher").join("qt_ui.ini");
        (exe, ini)
    }

    #[test]
    fn writes_version_selected_with_forward_slashes() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, ini) = qt_exe(temp.path());

        let result = ensure_version_selected(exe.to_str().unwrap(), r"C:\ShadPS4\shadps4.exe");

        assert!(result.changed);
        assert_eq!(result.config_path.as_deref(), Some(ini.as_path()));
        let text = std::fs::read_to_string(&ini).unwrap();
        assert_eq!(
            text, "[version_manager]\nversionSelected=C:/ShadPS4/shadps4.exe\n",
            "backslashes must be written as forward slashes, with no spaces"
        );
        assert!(
            !text.contains("versionPath"),
            "versionPath is the launcher's own download folder: {text:?}"
        );
    }

    #[test]
    fn is_idempotent_on_the_second_call() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, ini) = qt_exe(temp.path());
        let shadps4 = temp
            .path()
            .join("ShadPS4-latest")
            .join("Shadps4-sdl.AppImage");

        ensure_version_selected(exe.to_str().unwrap(), shadps4.to_str().unwrap());
        let before = std::fs::read(&ini).unwrap();

        let result = ensure_version_selected(exe.to_str().unwrap(), shadps4.to_str().unwrap());

        assert!(!result.changed);
        assert_eq!(
            std::fs::read(&ini).unwrap(),
            before,
            "bytes must be identical"
        );
    }

    #[test]
    fn preserves_other_sections_and_keys() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, ini) = qt_exe(temp.path());
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(
            &ini,
            "[General]\ntheme=dark\n[version_manager]\nautoUpdate=true\nversionSelected=/old\n",
        )
        .unwrap();

        let result = ensure_version_selected(exe.to_str().unwrap(), "/opt/shadps4/shadps4");

        assert!(result.changed);
        let text = std::fs::read_to_string(&ini).unwrap();
        assert!(text.contains("theme=dark"), "{text:?}");
        assert!(text.contains("autoUpdate=true"), "{text:?}");
        assert!(
            text.contains("versionSelected=/opt/shadps4/shadps4"),
            "{text:?}"
        );
        assert!(!text.contains("/old"), "{text:?}");
    }

    #[test]
    fn blank_paths_are_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, _) = qt_exe(temp.path());

        assert!(ensure_version_selected("   ", "/opt/shadps4/shadps4")
            .config_path
            .is_none());
        assert!(ensure_version_selected(exe.to_str().unwrap(), "  ")
            .config_path
            .is_none());
        assert!(ensure_portable_user_dir("").config_path.is_none());
    }

    #[test]
    fn creates_the_launcher_directory() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, ini) = qt_exe(temp.path());
        assert!(!ini.parent().unwrap().exists());

        ensure_version_selected(exe.to_str().unwrap(), "/opt/shadps4/shadps4");

        assert!(ini.parent().unwrap().is_dir());
        assert!(ini.is_file());
    }

    #[test]
    fn user_dir_is_created_once() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("ShadPS4-latest");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Shadps4-sdl.AppImage");
        std::fs::write(&exe, b"").unwrap();

        let first = ensure_portable_user_dir(exe.to_str().unwrap());
        assert!(first.changed);
        assert_eq!(
            first.config_path.as_deref(),
            Some(dir.join("user").as_path())
        );
        assert!(dir.join("user").is_dir());

        let second = ensure_portable_user_dir(exe.to_str().unwrap());
        assert!(!second.changed);
        assert_eq!(
            second.config_path.as_deref(),
            Some(dir.join("user").as_path())
        );
    }
}
