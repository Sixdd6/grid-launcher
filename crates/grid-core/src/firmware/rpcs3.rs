//! RPCS3 PS3-firmware helpers: locating `PS3UPDAT.PUP` beside the
//! executable, launching `rpcs3 --installfw`, telling whether RPCS3 has
//! actually installed the firmware (its `dev_flash` markers), and picking
//! the PS3 platform id out of the server's platform map.
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
use crate::autoconfig::readers::rpcs3_dev_flash_candidates;
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

/// The firmware RPCS3 itself has installed (what `--installfw` extracted
/// into its `dev_flash`), as opposed to a PUP merely sitting on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rpcs3InstalledFirmware {
    /// `dev_flash/sys/external/liblv2.sprx` exists — the file RPCS3 refuses
    /// to boot a game without ("PS3 firmware is not installed").
    pub installed: bool,
    /// The version from `dev_flash/vsh/etc/version.txt` (`"4.93"` from
    /// `release:04.9300:`), or `None` when the file is missing or garbled.
    /// Always `None` when `installed` is false.
    pub version: Option<String>,
}

/// The marker rule over an explicit list of `dev_flash` candidates: the
/// FIRST one holding `sys/external/liblv2.sprx` decides. `liblv2.sprx` alone
/// means installed — it is RPCS3's own boot gate — and `vsh/etc/version.txt`
/// only supplies the version, so a missing or unreadable version file yields
/// "installed, version unknown" rather than "not installed". A version file
/// without `liblv2.sprx` counts for nothing.
pub fn installed_firmware_in(dev_flash_dirs: &[PathBuf]) -> Rpcs3InstalledFirmware {
    for dir in dev_flash_dirs {
        if !dir
            .join("sys")
            .join("external")
            .join("liblv2.sprx")
            .is_file()
        {
            continue;
        }
        let version = std::fs::read(dir.join("vsh").join("etc").join("version.txt"))
            .ok()
            .and_then(|bytes| parse_firmware_version(&String::from_utf8_lossy(&bytes)));
        return Rpcs3InstalledFirmware {
            installed: true,
            version,
        };
    }
    Rpcs3InstalledFirmware::default()
}

/// The firmware RPCS3 at `emulator_path` has installed, searched through
/// every `dev_flash` its config roots may name (portable dir, the exe's
/// folder, `$RPCS3_CONFIG_DIR`, `~/.config/rpcs3`; each honouring its
/// `vfs.yml` `/dev_flash/` entry). A blank path never resolves.
pub fn rpcs3_installed_firmware(emulator_path: &str) -> Rpcs3InstalledFirmware {
    if emulator_path.trim().is_empty() {
        return Rpcs3InstalledFirmware::default();
    }
    installed_firmware_in(&rpcs3_dev_flash_candidates(emulator_path))
}

/// `"4.93"` from version.txt's `release:04.9300:` line: the major without
/// leading zeros, the minor without trailing zeros but at least two digits
/// (`03.5500` -> `3.55`, `04.9000` -> `4.90`). `None` when no line carries a
/// numeric `major.minor` release field.
fn parse_firmware_version(text: &str) -> Option<String> {
    let field = text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("release:")?;
        Some(rest.split(':').next().unwrap_or("").trim().to_string())
    })?;
    let (major, minor) = field.split_once('.')?;
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(major) || !all_digits(minor) {
        return None;
    }
    let major = major.trim_start_matches('0');
    let major = if major.is_empty() { "0" } else { major };
    let mut minor = minor.trim_end_matches('0').to_string();
    while minor.len() < 2 {
        minor.push('0');
    }
    Some(format!("{major}.{minor}"))
}

/// The three paths `rpcs3 --installfw` is started with: the executable, the
/// PUP argument, and the working directory (the executable's parent).
#[derive(Debug, PartialEq, Eq)]
struct InstallFwInvocation {
    exe: PathBuf,
    pup: PathBuf,
    working_dir: PathBuf,
}

/// Resolves the `--installfw` paths, or `None` when either does not
/// canonicalize to an existing file. Uses [`crate::platform::canonicalize`]
/// so on Windows none of the paths carries the `\\?\` verbatim prefix, which
/// RPCS3 and `current_dir` handle poorly.
fn installfw_invocation(exe: &Path, pup: &Path) -> Option<InstallFwInvocation> {
    let exe = crate::platform::canonicalize(exe).ok()?;
    let pup = crate::platform::canonicalize(pup).ok()?;
    if !exe.is_file() || !pup.is_file() {
        return None;
    }
    let working_dir = exe.parent()?.to_path_buf();
    Some(InstallFwInvocation {
        exe,
        pup,
        working_dir,
    })
}

/// Launches `rpcs3 --installfw <pup>` and returns whether the process
/// started (`trigger_rpcs3_firmware_install`, rpcs3.py:365-386).
///
/// Both paths must canonicalize (without a Windows `\?` prefix) to existing files. This call does not block:
/// RPCS3 shows its own install dialog and outlives it, exactly like Python's
/// bare `subprocess.Popen`. A detached thread owns the
/// [`std::process::Child`] and blocks in `wait()` on it, purely so the
/// process is reaped once RPCS3 exits — a long-lived launcher would
/// otherwise leave one zombie behind per firmware install. Python needs no
/// such thread because `Popen`'s finalizer reaps the child when the object
/// is collected. The environment is rebuilt from [`clean_env`] so an
/// AppImage's `LD_LIBRARY_PATH` shim does not leak into the child.
pub fn spawn_rpcs3_installfw(exe: &Path, pup: &Path) -> bool {
    let Some(invocation) = installfw_invocation(exe, pup) else {
        return false;
    };

    let spawned = Command::new(&invocation.exe)
        .arg("--installfw")
        .arg(&invocation.pup)
        .current_dir(&invocation.working_dir)
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

    /// Writes RPCS3's two firmware markers under `dev_flash`; `version`
    /// `None` leaves version.txt out.
    fn write_firmware(dev_flash: &Path, liblv2: bool, version: Option<&[u8]>) {
        if liblv2 {
            let external = dev_flash.join("sys").join("external");
            std::fs::create_dir_all(&external).unwrap();
            std::fs::write(external.join("liblv2.sprx"), b"sce").unwrap();
        }
        if let Some(text) = version {
            let etc = dev_flash.join("vsh").join("etc");
            std::fs::create_dir_all(&etc).unwrap();
            std::fs::write(etc.join("version.txt"), text).unwrap();
        }
    }

    #[test]
    fn firmware_version_parses_rpcs3_release_lines() {
        assert_eq!(
            parse_firmware_version("release:04.9300:\nbuild:12345,67890:\n").as_deref(),
            Some("4.93")
        );
        assert_eq!(
            parse_firmware_version("release:03.5500:").as_deref(),
            Some("3.55")
        );
        assert_eq!(
            parse_firmware_version("release:04.9000:").as_deref(),
            Some("4.90")
        );
        assert_eq!(
            parse_firmware_version("release:04.8150:").as_deref(),
            Some("4.815")
        );
        assert_eq!(parse_firmware_version("garbage"), None);
        assert_eq!(parse_firmware_version("release:abc:"), None);
        assert_eq!(parse_firmware_version("release::"), None);
        assert_eq!(parse_firmware_version(""), None);
    }

    #[test]
    fn installed_firmware_needs_liblv2_and_reads_the_version() {
        let temp = tempfile::tempdir().unwrap();
        let flash = temp.path().join("dev_flash");
        write_firmware(&flash, true, Some(b"release:04.9300:\n"));

        assert_eq!(
            installed_firmware_in(std::slice::from_ref(&flash)),
            Rpcs3InstalledFirmware {
                installed: true,
                version: Some("4.93".into())
            }
        );
    }

    #[test]
    fn installed_firmware_without_liblv2_is_not_installed() {
        let temp = tempfile::tempdir().unwrap();
        let flash = temp.path().join("dev_flash");
        write_firmware(&flash, false, Some(b"release:04.9300:\n"));

        assert_eq!(
            installed_firmware_in(&[flash]),
            Rpcs3InstalledFirmware::default()
        );
        assert_eq!(
            installed_firmware_in(&[]),
            Rpcs3InstalledFirmware::default()
        );
    }

    #[test]
    fn installed_firmware_with_a_missing_or_garbled_version_is_installed_unknown() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("a");
        write_firmware(&missing, true, None);
        let garbled = temp.path().join("b");
        write_firmware(&garbled, true, Some(&[0xff, 0xfe, 0x00]));

        for flash in [missing, garbled] {
            assert_eq!(
                installed_firmware_in(&[flash]),
                Rpcs3InstalledFirmware {
                    installed: true,
                    version: None
                }
            );
        }
    }

    #[test]
    fn installed_firmware_takes_the_first_candidate_with_liblv2() {
        let temp = tempfile::tempdir().unwrap();
        let empty = temp.path().join("empty");
        let second = temp.path().join("second");
        write_firmware(&second, true, Some(b"release:04.9200:"));

        assert_eq!(
            installed_firmware_in(&[empty, second]).version.as_deref(),
            Some("4.92")
        );
    }

    #[test]
    fn rpcs3_installed_firmware_finds_a_portable_dev_flash() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated(temp.path());
        let dir = temp.path().join("RPCS3");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("rpcs3.AppImage");
        std::fs::write(&exe, b"").unwrap();
        assert!(!rpcs3_installed_firmware(exe.to_str().unwrap()).installed);

        write_firmware(
            &dir.join("portable").join("dev_flash"),
            true,
            Some(b"release:04.9300:"),
        );
        assert_eq!(
            rpcs3_installed_firmware(exe.to_str().unwrap())
                .version
                .as_deref(),
            Some("4.93")
        );
        assert!(!rpcs3_installed_firmware("  ").installed);
    }

    #[test]
    fn rpcs3_installed_firmware_honours_a_custom_vfs_dev_flash() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated(temp.path());
        let dir = temp.path().join("RPCS3");
        let config = dir.join("config");
        std::fs::create_dir_all(&config).unwrap();
        let exe = dir.join("rpcs3.exe");
        std::fs::write(&exe, b"").unwrap();
        let flash = temp.path().join("custom").join("flash");
        // The default location holds no firmware; the configured one does.
        std::fs::write(
            config.join("vfs.yml"),
            format!(
                "/dev_flash/: {}/\n",
                flash.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        write_firmware(&flash, true, Some(b"release:04.9100:"));

        assert_eq!(
            rpcs3_installed_firmware(exe.to_str().unwrap())
                .version
                .as_deref(),
            Some("4.91")
        );
    }

    /// GRID's own layout: `portable` is a symlink into the saves folder.
    #[cfg(unix)]
    #[test]
    fn rpcs3_installed_firmware_follows_a_symlinked_portable_dir() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated(temp.path());
        let dir = temp.path().join("emulators").join("RPCS3");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("rpcs3.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let saves = temp.path().join("saves").join("RPCS3").join("portable");
        std::fs::create_dir_all(&saves).unwrap();
        std::os::unix::fs::symlink(&saves, dir.join("portable")).unwrap();
        write_firmware(&saves.join("dev_flash"), true, Some(b"release:04.9300:"));

        assert_eq!(
            rpcs3_installed_firmware(exe.to_str().unwrap()),
            Rpcs3InstalledFirmware {
                installed: true,
                version: Some("4.93".into())
            }
        );
    }

    /// Points every RPCS3 config root GRID consults at `dir`, so the host's
    /// own `~/.config/rpcs3` cannot leak into a test.
    fn isolated(dir: &Path) -> crate::test_env::EnvGuard {
        let dir = dir.to_str().unwrap();
        crate::test_env::EnvGuard::set(&[
            ("HOME", Some(dir)),
            ("XDG_CONFIG_HOME", None),
            ("USERPROFILE", None),
            ("APPDATA", None),
            ("RPCS3_CONFIG_DIR", None),
        ])
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
            crate::platform::canonicalize(&pup).unwrap(),
            "the PUP is passed canonicalized"
        );
        assert!(
            wait_until_reaped(lines[0]),
            "the spawned child was never reaped (pid {} still in /proc)",
            lines[0]
        );
    }

    /// The executable, the PUP argument and the working directory are the
    /// plain canonical paths — on Windows none starts with the `\\?\`
    /// verbatim prefix — and the working directory is the executable's parent.
    #[test]
    fn installfw_invocation_paths_are_canonical_and_never_verbatim() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("rpcs3.exe");
        let pup = temp.path().join("PS3UPDAT.PUP");
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(&pup, b"").unwrap();

        let invocation = installfw_invocation(&exe, &pup).unwrap();

        let expected_exe = crate::platform::canonicalize(&exe).unwrap();
        assert_eq!(
            invocation,
            InstallFwInvocation {
                working_dir: expected_exe.parent().unwrap().to_path_buf(),
                exe: expected_exe,
                pup: crate::platform::canonicalize(&pup).unwrap(),
            }
        );
        for path in [&invocation.exe, &invocation.pup, &invocation.working_dir] {
            assert!(
                !path.to_string_lossy().starts_with(r"\\?\"),
                "verbatim path handed to rpcs3: {path:?}"
            );
        }
    }

    #[test]
    fn installfw_invocation_rejects_missing_files_and_directories() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("rpcs3.exe");
        let pup = temp.path().join("PS3UPDAT.PUP");
        assert!(installfw_invocation(&exe, &pup).is_none());
        std::fs::write(&exe, b"").unwrap();
        assert!(installfw_invocation(&exe, &pup).is_none());
        std::fs::write(&pup, b"").unwrap();
        assert!(installfw_invocation(&exe, &pup).is_some());
        assert!(installfw_invocation(&exe, temp.path()).is_none());
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
