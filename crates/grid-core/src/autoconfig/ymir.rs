//! Ymir's add-only `Ymir.toml`: one save file per game.
//!
//! Ymir keeps its profile (settings, BIOS under `roms/ipl`, saves, save
//! states) in its working directory, which GRID sets to the emulator's own
//! folder for every launch (`launch::spawn`), so the profile is
//! `<emulator dir>`. By default every game shares one internal backup RAM,
//! `state/bup-int.bin`; with `[System] InternalBackupRAMPerGame = true` each
//! game gets `backup/games/bup-int-<title> [<serial>].bin`, which the
//! profile's `single_file` save strategy syncs per game like DuckStation's
//! memory cards.
//!
//! Add-only: a `InternalBackupRAMPerGame` the user already set survives. A
//! file GRID creates starts with Ymir's current `ConfigVersion` (6, Ymir
//! v0.4.1), since Ymir reads a missing one as 0.

use super::{writers, xemu::resolve_emulator_dir, EnsureResult};

/// Ymir's settings version as of v0.4.1 (`kConfigVersion`).
const CONFIG_VERSION: &str = "ConfigVersion = 6";

/// Turns on per-game internal backup RAM in `<emulator dir>/Ymir.toml`. A
/// blank path, or any I/O error, changes nothing.
pub fn ensure_settings(emulator_path: &str) -> EnsureResult {
    let Some(dir) = resolve_emulator_dir(emulator_path) else {
        return EnsureResult::unchanged();
    };
    let config_path = dir.join("Ymir.toml");
    let content = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => return EnsureResult::unchanged(),
        }
    } else {
        format!("{CONFIG_VERSION}\n")
    };

    let (content, changed) = writers::toml_add_only_section(
        &content,
        "System",
        &crate::desired![("InternalBackupRAMPerGame", "true")],
    );
    if changed && std::fs::write(&config_path, &content).is_err() {
        return EnsureResult::unchanged();
    }
    EnsureResult::at(config_path, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(dir: &std::path::Path) -> String {
        let exe = dir.join("ymir-sdl3");
        std::fs::write(&exe, b"").unwrap();
        exe.to_string_lossy().into_owned()
    }

    #[test]
    fn a_fresh_install_gets_per_game_saves_and_the_config_version() {
        let dir = tempfile::tempdir().unwrap();
        let result = ensure_settings(&exe(dir.path()));
        assert!(result.changed);
        let text = std::fs::read_to_string(dir.path().join("Ymir.toml")).unwrap();
        assert!(text.starts_with("ConfigVersion = 6\n"), "{text}");
        assert!(text.contains("[System]"), "{text}");
        assert!(text.contains("InternalBackupRAMPerGame = true"), "{text}");
    }

    #[test]
    fn a_setting_the_user_already_chose_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Ymir.toml");
        let mine = "ConfigVersion = 6\n\n[System]\nInternalBackupRAMPerGame = false\n";
        std::fs::write(&path, mine).unwrap();
        let result = ensure_settings(&exe(dir.path()));
        assert!(!result.changed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), mine);
    }

    #[test]
    fn other_sections_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Ymir.toml");
        std::fs::write(&path, "ConfigVersion = 6\n\n[Video]\nFullScreen = true\n").unwrap();
        assert!(ensure_settings(&exe(dir.path())).changed);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[Video]\nFullScreen = true"), "{text}");
        assert!(text.contains("InternalBackupRAMPerGame = true"), "{text}");
    }

    #[test]
    fn a_blank_path_changes_nothing() {
        assert!(!ensure_settings("   ").changed);
    }
}
