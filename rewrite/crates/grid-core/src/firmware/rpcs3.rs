//! RPCS3 PS3-firmware helpers: locating `PS3UPDAT.PUP` beside the
//! executable, launching `rpcs3 --installfw`, and picking the PS3 platform
//! id out of the server's platform map.
//!
//! Ports `grid_launcher/emulator/rpcs3.py:274-283` (`rpcs3_pup_path`),
//! `rpcs3.py:365-386` (`trigger_rpcs3_firmware_install`) and the PS3
//! platform-id scan at `grid_launcher/ui/mixins/emulator_ui_mixin.py:1747-1753`.
//! The direct-from-Sony PUP download those call sites sit next to is ruled
//! out by design decision D2 and is not ported.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::autoconfig::paths::{expand_user, resolve_best_effort};
use crate::launch::spawn::clean_env;

/// The `PS3UPDAT.PUP` sitting beside `emulator_path`, or `None` when there
/// is none (`rpcs3_pup_path`, rpcs3.py:274-283).
///
/// The search directory is `emulator_path` itself when it is an existing
/// directory, else its parent — the same rule
/// [`crate::autoconfig::paths::emulator_dir`] applies. A blank path never
/// resolves. The returned path is `.resolve()`d, matching Python.
pub fn rpcs3_pup_path(emulator_path: &str) -> Option<PathBuf> {
    let text = emulator_path.trim();
    if text.is_empty() {
        return None;
    }
    let expanded = expand_user(text);
    let emulator_dir = if expanded.is_dir() {
        expanded
    } else {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let pup = emulator_dir.join("PS3UPDAT.PUP");
    // `is_file()` already implies `exists()` — Python checks both.
    pup.is_file().then(|| resolve_best_effort(&pup))
}

/// Launches `rpcs3 --installfw <pup>` and returns whether the process
/// started (`trigger_rpcs3_firmware_install`, rpcs3.py:365-386).
///
/// Both paths must canonicalize to existing files. The child is spawned and
/// deliberately NOT waited on: RPCS3 shows its own install dialog and
/// outlives this call, exactly like Python's bare `subprocess.Popen`. The
/// environment is rebuilt from [`clean_env`] so an AppImage's
/// `LD_LIBRARY_PATH` shim does not leak into the child.
pub fn spawn_rpcs3_installfw(exe: &Path, pup: &Path) -> bool {
    let Ok(exe) = std::fs::canonicalize(exe) else {
        return false;
    };
    let Ok(pup) = std::fs::canonicalize(pup) else {
        return false;
    };
    if !exe.is_file() || !pup.is_file() {
        return false;
    }
    let Some(working_dir) = exe.parent() else {
        return false;
    };

    Command::new(&exe)
        .arg("--installfw")
        .arg(&pup)
        .current_dir(working_dir)
        .env_clear()
        .envs(clean_env())
        .spawn()
        .is_ok()
}

/// The server platform id for PlayStation 3, or `None`
/// (emulator_ui_mixin.py:1747-1753): the first key whose lower-cased form
/// contains `"playstation 3"` or equals `"ps3"`.
pub fn ps3_platform_id(platforms: &BTreeMap<String, i64>) -> Option<i64> {
    platforms
        .iter()
        .find(|(key, _)| {
            let lower = key.to_lowercase();
            lower.contains("playstation 3") || lower == "ps3"
        })
        .map(|(_, id)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pup_path_uses_the_directory_itself_when_the_entry_path_is_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("PS3UPDAT.PUP"), b"").unwrap();

        assert_eq!(
            rpcs3_pup_path(temp.path().to_str().unwrap()),
            Some(resolve_best_effort(&temp.path().join("PS3UPDAT.PUP")))
        );
    }

    #[test]
    fn pup_path_ignores_a_directory_named_like_the_pup() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("rpcs3.exe");
        std::fs::write(&exe, b"").unwrap();
        std::fs::create_dir_all(temp.path().join("PS3UPDAT.PUP")).unwrap();

        assert!(rpcs3_pup_path(exe.to_str().unwrap()).is_none());
    }

    #[test]
    fn installfw_refuses_paths_that_are_not_files() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("rpcs3.exe");
        let pup = temp.path().join("PS3UPDAT.PUP");

        // Neither exists yet.
        assert!(!spawn_rpcs3_installfw(&exe, &pup));

        std::fs::write(&pup, b"").unwrap();
        // The executable is still missing.
        assert!(!spawn_rpcs3_installfw(&exe, &pup));

        std::fs::write(&exe, b"").unwrap();
        // A directory is never an acceptable PUP.
        assert!(!spawn_rpcs3_installfw(&exe, temp.path()));
    }
}
