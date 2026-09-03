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

use crate::autoconfig::paths::{self, expand_user, resolve_best_effort};
use crate::launch::spawn::clean_env;

/// The `PS3UPDAT.PUP` sitting beside `emulator_path`, or `None` when there
/// is none (`rpcs3_pup_path`, rpcs3.py:274-283).
///
/// The search directory is `emulator_path` itself when it is an existing
/// directory, else its parent — [`crate::autoconfig::paths::emulator_dir`],
/// the crate's single home for that rule. A blank path never resolves. The
/// returned path is `.resolve()`d, matching Python.
pub fn rpcs3_pup_path(emulator_path: &str) -> Option<PathBuf> {
    let text = emulator_path.trim();
    if text.is_empty() {
        return None;
    }
    let emulator_dir = paths::emulator_dir(&expand_user(text)).unwrap_or_default();
    let pup = emulator_dir.join("PS3UPDAT.PUP");
    // `is_file()` already implies `exists()` — Python checks both.
    pup.is_file().then(|| resolve_best_effort(&pup))
}

/// Launches `rpcs3 --installfw <pup>` and returns whether the process
/// started (`trigger_rpcs3_firmware_install`, rpcs3.py:365-386).
///
/// Both paths must canonicalize to existing files. This call does not block:
/// RPCS3 shows its own install dialog and outlives it, exactly like Python's
/// bare `subprocess.Popen`. A detached thread owns the
/// [`std::process::Child`] and blocks in `wait()` on it, purely so the
/// process is reaped once RPCS3 exits — a long-lived launcher would
/// otherwise leave one zombie behind per firmware install. Python needs no
/// such thread because `Popen`'s finalizer reaps the child when the object
/// is collected. The environment is rebuilt from [`clean_env`] so an
/// AppImage's `LD_LIBRARY_PATH` shim does not leak into the child.
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

    let spawned = Command::new(&exe)
        .arg("--installfw")
        .arg(&pup)
        .current_dir(working_dir)
        .env_clear()
        .envs(clean_env())
        .spawn();

    match spawned {
        Ok(mut child) => {
            // Detached reaper: the thread owns the `Child`, blocks in
            // `wait()` until RPCS3 exits, and then ends. It never keeps the
            // process alive on its own and never reports back — the caller
            // has already been told the launch succeeded.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
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

    /// Polls `path` until it holds non-blank text, up to ~2s.
    #[cfg(target_os = "linux")]
    fn wait_for_text(path: &Path) -> Option<String> {
        for _ in 0..200 {
            if let Ok(text) = std::fs::read_to_string(path) {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }

    /// Whether `/proc/<pid>` disappears within ~2s. An exited-but-unreaped
    /// child keeps its entry forever (state `Z`); a reaped one is gone.
    #[cfg(target_os = "linux")]
    fn wait_until_reaped(pid: &str) -> bool {
        let stat = std::path::PathBuf::from(format!("/proc/{pid}/stat"));
        for _ in 0..200 {
            if !stat.exists() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// The spawn path end to end: the child really runs with
    /// `--installfw <pup>` and is reaped after it exits.
    ///
    /// Linux-only because the reaping half is asserted through procfs. The
    /// assertion is exact rather than incidental: nothing else in this test
    /// binary ever calls `wait()`, so without the detached reaper thread the
    /// exited stub would sit in `Z` state — and `/proc/<pid>` would still be
    /// there — for the whole run, and the poll would time out.
    #[cfg(target_os = "linux")]
    #[test]
    fn installfw_spawns_the_child_and_reaps_it() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("argv.txt");
        let exe = temp.path().join("rpcs3-stub.sh");
        std::fs::write(
            &exe,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$$\" \"$@\" > '{}'\n",
                marker.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let pup = temp.path().join("PS3UPDAT.PUP");
        std::fs::write(&pup, b"").unwrap();

        assert!(spawn_rpcs3_installfw(&exe, &pup));

        let written = wait_for_text(&marker).expect("the stub never wrote its argv");
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 3, "expected pid + two arguments: {lines:?}");
        assert_eq!(lines[1], "--installfw");
        assert_eq!(
            Path::new(lines[2]),
            std::fs::canonicalize(&pup).unwrap(),
            "the PUP is passed canonicalized"
        );
        assert!(
            wait_until_reaped(lines[0]),
            "the spawned child was never reaped (pid {} still in /proc)",
            lines[0]
        );
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
