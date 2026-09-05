//! Turning a resolved emulator + game into a command line, and the
//! environment a spawned child gets. Ports `prepare_emulator_launch_command`
//! (`grid_launcher/emulator/launch.py:270`) and `clean_subprocess_env`
//! (`grid_launcher/core/process.py:8`). See
//! `docs/porting/04-emulator-launch.md` §8.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::EmulatorEntry;
use crate::library::paths::expand_home;

use super::template::{build_args, normalized_retroarch_core_args, Placeholders};

/// Builds the argv (executable first) and working directory for an emulated
/// launch, applying the validation chain in the reference order:
///
/// 1. blank `emulator_name` → "No emulator is configured. Add one in
///    Emulators settings."
/// 2. no `entry` → "Default emulator '<name>' was not found."
/// 3. blank `entry.path` → "Emulator '<name>' has no executable path
///    configured."
/// 4. the executable is not an existing file → "Emulator executable not
///    found:\n<path>"
/// 5. blank `rom_path` → "No ROM file is available for this game."
/// 6. the ROM is not an existing file → "ROM file not found:\n<path>"
/// 7. argument-template failure → "Invalid launch arguments: <e>"
///
/// `is_retroarch` gates the RetroArch `-L` post-pass, which resolves a
/// relative core path against the AppImage portable home first, then the
/// emulator's directory. The caller decides
/// what counts as RetroArch (entry name or matched profile name), and the
/// same flag decides whether `placeholders.core` was populated at all.
///
/// The working directory is the executable's parent (`.` when the resolved
/// executable has no parent component).
pub fn prepare_emulator_launch(
    emulator_name: &str,
    entry: Option<&EmulatorEntry>,
    rom_path: &str,
    placeholders: &Placeholders,
    global_launch_args: &str,
    is_retroarch: bool,
) -> Result<(Vec<String>, PathBuf), String> {
    let name = emulator_name.trim();
    if name.is_empty() {
        return Err("No emulator is configured. Add one in Emulators settings.".to_string());
    }

    let Some(entry) = entry else {
        return Err(format!("Default emulator '{name}' was not found."));
    };

    let configured_path = entry.path.trim();
    if configured_path.is_empty() {
        return Err(format!(
            "Emulator '{name}' has no executable path configured."
        ));
    }

    let executable = expand_home(configured_path);
    if !executable.is_file() {
        return Err(format!(
            "Emulator executable not found:\n{}",
            executable.display()
        ));
    }

    if rom_path.trim().is_empty() {
        return Err("No ROM file is available for this game.".to_string());
    }

    let rom_file = expand_home(rom_path);
    if !rom_file.is_file() {
        return Err(format!("ROM file not found:\n{}", rom_file.display()));
    }

    let args = build_args(&entry.args, global_launch_args, placeholders)
        .map_err(|e| format!("Invalid launch arguments: {e}"))?;

    let working_dir = match executable.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    let args = if is_retroarch {
        normalized_retroarch_core_args(&executable, args)
    } else {
        args
    };

    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push(executable.to_string_lossy().into_owned());
    argv.extend(args);
    Ok((argv, working_dir))
}

/// Builds the argv and working directory for a ROM-less "open the emulator
/// so I can configure controls" launch — `_launch_emulator_at_index`
/// (emulator_ui_mixin.py:1635-1665). The argv is the resolved executable and
/// nothing else: Python builds `command = [str(emulator_path)]` (:1657) and
/// never templates `entry.args`, so a `%rom%` in the stored arguments cannot
/// leak into a launch that has no ROM.
///
/// The validation chain and its wording follow the reference, minus the
/// ROM checks it has no use for:
///
/// 1. no `entry` → "Emulator '<name>' was not found." (Python's index guard
///    silently returns instead, :1637-1639; a click on a row that vanished
///    is a race worth reporting rather than swallowing)
/// 2. blank `entry.path` → "Emulator '<name>' has no executable path
///    configured." (:1645)
/// 3. the executable is not an existing file → "Emulator executable not
///    found:\n<path>" (:1650)
///
/// Python also calls `_ensure_emulator_sync_settings` before spawning
/// (:1653); the rewrite does not — the autoconfig sync runs at add/install
/// time (D1 call site B) and `launch_game` does not re-run it either.
pub fn prepare_standalone_emulator_launch(
    emulator_name: &str,
    entry: Option<&EmulatorEntry>,
) -> Result<(Vec<String>, PathBuf), String> {
    let name = emulator_name.trim();

    let Some(entry) = entry else {
        return Err(format!("Emulator '{name}' was not found."));
    };

    let configured_path = entry.path.trim();
    if configured_path.is_empty() {
        return Err(format!(
            "Emulator '{name}' has no executable path configured."
        ));
    }

    let executable = expand_home(configured_path);
    if !executable.is_file() {
        return Err(format!(
            "Emulator executable not found:\n{}",
            executable.display()
        ));
    }

    let working_dir = match executable.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    Ok((vec![executable.to_string_lossy().into_owned()], working_dir))
}

/// Spawns a standalone emulator and returns as soon as it has started —
/// Python's bare `subprocess.Popen` (emulator_ui_mixin.py:1655-1661). The
/// child gets [`clean_env`] and, on Windows, its own process group, exactly
/// like `spawn_child` in `launch/mod.rs` and Python's
/// `CREATE_NEW_PROCESS_GROUP` (:1660).
///
/// A detached thread owns the [`std::process::Child`] and blocks in `wait()`
/// purely so the process is reaped when the emulator exits — the same
/// arrangement (and the same reason) as
/// [`crate::firmware::rpcs3::spawn_rpcs3_installfw`]. There is no session
/// row for a ROM-less launch, so nothing else is watching it.
///
/// Python then warns 500ms later if the process already died
/// (`_warn_if_process_exited_early`, :1662). That is not ported: the rewrite
/// has no modal warning surface, and a deliberate deviation is better than a
/// half-modelled one.
pub fn spawn_standalone_emulator(argv: &[String], working_dir: &Path) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err("Failed to launch emulator:\nno executable to run".to_string());
    };

    let mut command = std::process::Command::new(program);
    command
        .args(&argv[1..])
        .current_dir(working_dir)
        .env_clear()
        .envs(clean_env());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(e) => Err(format!("Failed to launch emulator:\n{e}")),
    }
}

/// The environment a spawned host binary gets: a copy of this process's
/// environment with `LD_LIBRARY_PATH` restored from `LD_LIBRARY_PATH_ORIG`
/// when that variable is present.
///
/// A bundled build points `LD_LIBRARY_PATH` at its own private library
/// directory. A host binary started with that value can resolve its C++
/// runtime against the bundle's older libraries and fail to start, so the
/// bundler's saved original wins for children.
///
/// The returned map contains the whole parent environment and must never be
/// logged or put in an error message.
pub fn clean_env() -> HashMap<String, String> {
    clean_env_from(std::env::vars().collect())
}

/// The pure half of [`clean_env`], so the rule can be tested without mutating
/// the process environment (which is racy across parallel tests).
fn clean_env_from(mut env: HashMap<String, String>) -> HashMap<String, String> {
    if let Some(original) = env.get("LD_LIBRARY_PATH_ORIG").cloned() {
        env.insert("LD_LIBRARY_PATH".to_string(), original);
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, args: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: "Stub".to_string(),
            path: path.to_string(),
            args: args.to_string(),
            ..Default::default()
        }
    }

    fn placeholders(rom: &str, core: &str) -> Placeholders {
        Placeholders {
            rom: rom.to_string(),
            core: core.to_string(),
            ps3_launch_target: String::new(),
        }
    }

    fn env_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// A tempdir holding an executable stub and a ROM file.
    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("emu");
        std::fs::write(&exe, b"stub").unwrap();
        let rom = dir.path().join("game.rom");
        std::fs::write(&rom, b"rom").unwrap();
        (dir, exe, rom)
    }

    // --- clean_env ----------------------------------------------------------

    #[test]
    fn clean_env_copies_the_saved_original_over_the_bundle_path() {
        let env = clean_env_from(env_of(&[
            ("LD_LIBRARY_PATH", "/bundle/lib"),
            ("LD_LIBRARY_PATH_ORIG", "/usr/lib"),
            ("PATH", "/usr/bin"),
        ]));
        assert_eq!(env.get("LD_LIBRARY_PATH").unwrap(), "/usr/lib");
        assert_eq!(env.get("LD_LIBRARY_PATH_ORIG").unwrap(), "/usr/lib");
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin");
    }

    #[test]
    fn clean_env_without_the_saved_original_passes_the_environment_through() {
        let base = env_of(&[("LD_LIBRARY_PATH", "/bundle/lib"), ("PATH", "/usr/bin")]);
        assert_eq!(clean_env_from(base.clone()), base);
    }

    #[test]
    fn clean_env_reads_the_real_process_environment() {
        // PATH is set in every environment these tests run in; this only
        // checks that the real accessor is wired to `clean_env_from`.
        let env = clean_env();
        assert_eq!(env.get("PATH"), std::env::var("PATH").ok().as_ref());
    }

    // --- prepare_emulator_launch: the validation chain, in order ------------

    #[test]
    fn blank_emulator_name_is_rejected_first() {
        let error =
            prepare_emulator_launch("  ", None, "", &placeholders("", ""), "", false).unwrap_err();
        assert_eq!(
            error,
            "No emulator is configured. Add one in Emulators settings."
        );
    }

    #[test]
    fn a_missing_entry_names_the_emulator() {
        let error = prepare_emulator_launch("Dolphin", None, "", &placeholders("", ""), "", false)
            .unwrap_err();
        assert_eq!(error, "Default emulator 'Dolphin' was not found.");
    }

    #[test]
    fn a_blank_configured_path_names_the_emulator() {
        let entry = entry("   ", "%rom%");
        let error = prepare_emulator_launch(
            "Dolphin",
            Some(&entry),
            "/roms/game.rom",
            &placeholders("/roms/game.rom", ""),
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(
            error,
            "Emulator 'Dolphin' has no executable path configured."
        );
    }

    #[test]
    fn a_missing_executable_reports_the_resolved_path() {
        let error = prepare_emulator_launch(
            "Stub",
            Some(&entry("/nowhere/emu", "%rom%")),
            "/roms/game.rom",
            &placeholders("/roms/game.rom", ""),
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(error, "Emulator executable not found:\n/nowhere/emu");
    }

    #[test]
    fn a_blank_rom_path_is_rejected_after_the_executable_checks() {
        let (_dir, exe, _rom) = fixture();
        let entry = entry(&exe.to_string_lossy(), "%rom%");
        let error = prepare_emulator_launch(
            "Stub",
            Some(&entry),
            "   ",
            &placeholders("", ""),
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(error, "No ROM file is available for this game.");
    }

    #[test]
    fn a_missing_rom_file_reports_the_resolved_path() {
        let (_dir, exe, _rom) = fixture();
        let entry = entry(&exe.to_string_lossy(), "%rom%");
        let error = prepare_emulator_launch(
            "Stub",
            Some(&entry),
            "/nowhere/game.rom",
            &placeholders("/nowhere/game.rom", ""),
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(error, "ROM file not found:\n/nowhere/game.rom");
    }

    #[test]
    fn an_argument_failure_is_wrapped() {
        let (_dir, exe, rom) = fixture();
        let rom_text = rom.to_string_lossy().into_owned();
        // `-L %core%` with no core configured is the template layer's own
        // validation failure; this asserts the wrapping prefix.
        let entry = entry(&exe.to_string_lossy(), "-L %core% %rom%");
        let error = prepare_emulator_launch(
            "Stub",
            Some(&entry),
            &rom_text,
            &placeholders(&rom_text, ""),
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(
            error,
            "Invalid launch arguments: No RetroArch core is configured for this platform. \
             Set one in Emulators > Defaults."
        );
    }

    // --- prepare_emulator_launch: success ------------------------------------

    #[test]
    fn a_valid_launch_returns_argv_and_the_executable_directory() {
        let (dir, exe, rom) = fixture();
        let rom_text = rom.to_string_lossy().into_owned();
        let entry = entry(&exe.to_string_lossy(), "%rom%");
        let (argv, cwd) = prepare_emulator_launch(
            "Stub",
            Some(&entry),
            &rom_text,
            &placeholders(&rom_text, ""),
            "-fullscreen",
            false,
        )
        .unwrap();
        assert_eq!(
            argv,
            vec![
                exe.to_string_lossy().into_owned(),
                rom_text,
                "-fullscreen".to_string()
            ]
        );
        assert_eq!(cwd, dir.path());
    }

    #[test]
    fn the_retroarch_post_pass_runs_only_when_the_flag_is_set() {
        let (dir, exe, rom) = fixture();
        let rom_text = rom.to_string_lossy().into_owned();
        let cores = dir.path().join("cores");
        std::fs::create_dir_all(&cores).unwrap();
        let core = cores.join("snes9x_libretro.so");
        std::fs::write(&core, b"core").unwrap();

        let entry = entry(&exe.to_string_lossy(), "-L %core% %rom%");
        let ph = placeholders(&rom_text, "cores/snes9x_libretro.so");

        let (argv, _) =
            prepare_emulator_launch("RetroArch", Some(&entry), &rom_text, &ph, "", true).unwrap();
        let expected = std::fs::canonicalize(&core).unwrap();
        assert_eq!(argv[2], expected.to_string_lossy());

        let (argv, _) =
            prepare_emulator_launch("RetroArch", Some(&entry), &rom_text, &ph, "", false).unwrap();
        assert_eq!(argv[2], "cores/snes9x_libretro.so");
    }

    // --- standalone (ROM-less) launch ---------------------------------------

    #[test]
    fn standalone_launch_rejects_an_unknown_entry() {
        assert_eq!(
            prepare_standalone_emulator_launch("Ghost", None).unwrap_err(),
            "Emulator 'Ghost' was not found."
        );
    }

    #[test]
    fn standalone_launch_rejects_a_blank_path() {
        let e = entry("   ", "%rom%");
        assert_eq!(
            prepare_standalone_emulator_launch("Dolphin", Some(&e)).unwrap_err(),
            "Emulator 'Dolphin' has no executable path configured."
        );
    }

    #[test]
    fn standalone_launch_rejects_a_missing_executable() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let e = entry(missing.to_str().unwrap(), "%rom%");
        assert_eq!(
            prepare_standalone_emulator_launch("Dolphin", Some(&e)).unwrap_err(),
            format!("Emulator executable not found:\n{}", missing.display())
        );
    }

    #[test]
    fn standalone_launch_drops_every_argument_and_uses_the_executables_parent() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("dolphin");
        std::fs::write(&exe, b"").unwrap();
        // Args that would normally be templated: a ROM-less launch takes none.
        let e = entry(exe.to_str().unwrap(), "-b \"%rom%\"");

        let (argv, working_dir) = prepare_standalone_emulator_launch("Dolphin", Some(&e)).unwrap();
        assert_eq!(argv, vec![exe.to_string_lossy().into_owned()]);
        assert_eq!(working_dir, dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn spawn_standalone_runs_the_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let exe = dir.path().join("stub.sh");
        std::fs::write(&exe, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

        let argv = vec![exe.to_string_lossy().into_owned()];
        spawn_standalone_emulator(&argv, dir.path()).unwrap();

        // The reaper thread owns the child; poll for the marker rather than
        // waiting on a handle this API deliberately does not hand back.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(marker.exists(), "the stub never ran");
    }

    #[test]
    fn spawn_standalone_reports_a_failed_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("not-there");
        let argv = vec![missing.to_string_lossy().into_owned()];
        let err = spawn_standalone_emulator(&argv, dir.path()).unwrap_err();
        assert!(
            err.starts_with("Failed to launch emulator:\n"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn spawn_standalone_rejects_an_empty_argv() {
        assert_eq!(
            spawn_standalone_emulator(&[], Path::new(".")).unwrap_err(),
            "Failed to launch emulator:\nno executable to run"
        );
    }
}
