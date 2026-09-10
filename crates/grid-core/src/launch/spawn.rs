//! Turning a resolved emulator + game into a command line, and the
//! environment a spawned child gets. Ports `prepare_emulator_launch_command`
//! (`grid_launcher/emulator/launch.py:270`) and `clean_subprocess_env`
//! (`grid_launcher/core/process.py:8`). See
//! `docs/porting/04-emulator-launch.md` §8.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus};

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
/// (:1653). The rewrite runs that sync for a RetroArch entry in its caller
/// (`commands::launch_emulator`), between this function and
/// [`spawn_standalone_emulator`], so this one stays pure.
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

/// Spawns a standalone emulator and hands the caller its [`Child`] —
/// Python's bare `subprocess.Popen` (emulator_ui_mixin.py:1655-1661). The
/// child gets [`clean_env`] and, on Windows, its own process group, exactly
/// like `spawn_child` in `launch/mod.rs` and Python's
/// `CREATE_NEW_PROCESS_GROUP` (:1660).
///
/// The caller owns the child and must hand it to [`wait_for_early_exit`],
/// which runs Python's 500 ms check (`_warn_if_process_exited_early`,
/// :1662) and takes over reaping.
pub fn spawn_standalone_emulator(argv: &[String], working_dir: &Path) -> Result<Child, String> {
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

    command
        .spawn()
        .map_err(|e| format!("Failed to launch emulator:\n{e}"))
}

/// Blocks for the early-exit window and reports a child that is already
/// gone by the end of it — Python's `QTimer.singleShot(500, …)` around
/// `_warn_if_process_exited_early` (emulator_ui_mixin.py:1662), run inline
/// because this path has no event loop to defer onto and its caller is
/// already on the blocking pool.
///
/// A child still running when the window closes is handed to a detached
/// thread that blocks in `wait()` purely so the process is reaped when the
/// emulator eventually exits — the same arrangement (and the same reason) as
/// [`crate::firmware::rpcs3::spawn_rpcs3_installfw`]. There is no session
/// row for a ROM-less launch, so nothing else is watching it.
pub fn wait_for_early_exit(mut child: Child, argv: &[String]) -> Option<String> {
    std::thread::sleep(super::EARLY_EXIT_DELAY);
    // A `try_wait` error means no status was ever available; treat the child
    // as running and let the reaper thread deal with it.
    if let Ok(Some(status)) = child.try_wait() {
        return Some(process_exited_early_message(Some(status), argv));
    }
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    None
}

/// The message for a process that died inside its early-exit window, shared
/// by both surfaces: the Details strip for a game launch and the Emulators
/// toast for a standalone one. Verbatim from Python's
/// `process_exited_early_message` (`grid_launcher/emulator/launch.py:320`),
/// which space-joins the command.
///
/// `status` is `None` only when `try_wait` itself failed, so no exit code was
/// ever available; the process is gone either way and the user still needs to
/// be told, so the code reads "unknown".
pub fn process_exited_early_message(status: Option<ExitStatus>, argv: &[String]) -> String {
    let detail = match status {
        // No exit code: killed by a signal. The `ExitStatus` display already
        // reads as "signal: 9 (SIGKILL)".
        Some(status) => match status.code() {
            Some(code) => format!("code {code}"),
            None => status.to_string(),
        },
        None => "unknown".to_string(),
    };
    format!(
        "Process exited immediately ({detail}).\nCommand:\n{}",
        argv.join(" ")
    )
}

/// The environment a spawned host binary gets: a copy of this process's
/// environment with the bundle's own setup removed.
///
/// A bundled build rewrites the environment to point at its own private
/// copies of libraries, Python, Perl, Qt, GStreamer and GTK. A host binary
/// started with those values resolves against the bundle's older copies
/// and fails to start: umu-run is a system Python script, and with the
/// AppImage's `PYTHONHOME` it cannot find its own standard library, so it
/// exits 1 before Proton runs (2026-09-10). Two bundlers, two conventions:
///
/// - PyInstaller saves the original in `LD_LIBRARY_PATH_ORIG`; when that is
///   present it wins outright.
/// - The linuxdeploy AppRun inside the Tauri AppImage saves nothing, but
///   exports `APPDIR`. Every `:`-separated entry under that directory is
///   dropped from every variable (`LD_LIBRARY_PATH`, `PATH`, `PYTHONHOME`,
///   `PYTHONPATH`, `XDG_DATA_DIRS`, `PERLLIB`, `QT_PLUGIN_PATH`,
///   `GST_PLUGIN_SYSTEM_PATH*`, `GSETTINGS_SCHEMA_DIR`, the GTK hook's
///   module paths, and whatever a newer AppRun adds), a variable with
///   nothing left is removed, and the flag-style variables the bundle sets
///   ([`APPIMAGE_FLAG_VARS`]) are removed too.
///
/// Independently of either bundler, a `DBUS_SESSION_BUS_ADDRESS` whose
/// socket does not exist is repaired ([`repair_leaked_bus_address`]).
/// Otherwise, outside a bundle, the environment passes through untouched.
///
/// The returned map contains the whole parent environment and must never be
/// logged or put in an error message.
pub fn clean_env() -> HashMap<String, String> {
    clean_env_from(std::env::vars().collect())
}

/// Non-path variables the AppRun and its `linuxdeploy-plugin-gtk.sh` hook
/// export for the bundled runtime, none of which a child should inherit.
const APPIMAGE_FLAG_VARS: [&str; 3] = ["PYTHONDONTWRITEBYTECODE", "GDK_BACKEND", "GTK_THEME"];

/// The pure half of [`clean_env`], so the rule can be tested without mutating
/// the process environment (which is racy across parallel tests).
fn clean_env_from(env: HashMap<String, String>) -> HashMap<String, String> {
    clean_env_with(env, &|path| path.exists())
}

/// [`clean_env_from`] with the filesystem check injected, so the bus-address
/// repair can be tested against paths that do not exist on the test host.
fn clean_env_with(
    mut env: HashMap<String, String>,
    exists: &dyn Fn(&Path) -> bool,
) -> HashMap<String, String> {
    strip_bundle_paths(&mut env);
    repair_leaked_bus_address(&mut env, exists);
    env
}

/// The bundle half of [`clean_env`]: the PyInstaller restore, else the
/// AppImage strip.
fn strip_bundle_paths(env: &mut HashMap<String, String>) {
    if let Some(original) = env.get("LD_LIBRARY_PATH_ORIG").cloned() {
        env.insert("LD_LIBRARY_PATH".to_string(), original);
        return;
    }
    let Some(appdir) = env.get("APPDIR").cloned() else {
        return;
    };
    let keys: Vec<String> = env.keys().cloned().collect();
    for key in keys {
        let kept = without_entries_under(&env[&key], &appdir);
        if kept == env[&key] {
            continue;
        }
        if kept.is_empty() {
            env.remove(&key);
        } else {
            env.insert(key, kept);
        }
    }
    for key in APPIMAGE_FLAG_VARS {
        env.remove(key);
    }
}

/// Variables a flatpak sandbox sets for itself; meaningless on the host.
const FLATPAK_SANDBOX_VARS: [&str; 3] = ["FLATPAK_ID", "FLATPAK_SANDBOX_DIR", "container"];

/// Repairs a `DBUS_SESSION_BUS_ADDRESS` whose socket does not exist.
///
/// A launcher started from inside a flatpak (Gear Lever's Launch button
/// runs the AppImage through `flatpak-spawn --host`) inherits the sandbox's
/// own bus address, `unix:path=/run/flatpak/bus`, which exists only inside
/// that sandbox. pressure-vessel bind-mounts the session bus socket into
/// Proton's container and fails on the missing path, so the game dies one
/// second in, mid prefix creation (2026-09-10). The address is repointed at
/// `$XDG_RUNTIME_DIR/bus` when that socket exists, else removed so D-Bus
/// falls back to its own default; the sandbox's marker variables go with
/// it. A live socket, or a non-socket address, is left alone.
fn repair_leaked_bus_address(env: &mut HashMap<String, String>, exists: &dyn Fn(&Path) -> bool) {
    let Some(address) = env.get("DBUS_SESSION_BUS_ADDRESS") else {
        return;
    };
    let Some(path) = address.strip_prefix("unix:path=") else {
        return;
    };
    // An address may carry `,guid=...` after the path.
    let path = path.split(',').next().unwrap_or_default();
    if exists(Path::new(path)) {
        return;
    }
    let fallback = env
        .get("XDG_RUNTIME_DIR")
        .map(|dir| Path::new(dir).join("bus"))
        .filter(|bus| exists(bus));
    match fallback {
        Some(bus) => {
            env.insert(
                "DBUS_SESSION_BUS_ADDRESS".to_string(),
                format!("unix:path={}", bus.display()),
            );
        }
        None => {
            env.remove("DBUS_SESSION_BUS_ADDRESS");
        }
    }
    for key in FLATPAK_SANDBOX_VARS {
        env.remove(key);
    }
}

/// `value` unchanged when no entry is under `root`; otherwise `value` as a
/// `:`-separated list minus blank entries and entries that name `root` or
/// something inside it.
fn without_entries_under(value: &str, root: &str) -> String {
    let root = root.trim_end_matches('/');
    let under_root = |entry: &str| {
        let entry = entry.trim_end_matches('/');
        entry == root || entry.starts_with(&format!("{root}/"))
    };
    if !value.split(':').any(under_root) {
        return value.to_string();
    }
    value
        .split(':')
        .filter(|entry| !entry.is_empty())
        .filter(|entry| !under_root(entry))
        .collect::<Vec<_>>()
        .join(":")
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
    fn clean_env_without_the_saved_original_or_appdir_passes_the_environment_through() {
        let base = env_of(&[("LD_LIBRARY_PATH", "/bundle/lib"), ("PATH", "/usr/bin")]);
        assert_eq!(clean_env_from(base.clone()), base);
    }

    #[test]
    fn clean_env_strips_the_appimage_library_dirs_from_the_library_path() {
        let env = clean_env_from(env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_grid/usr/lib/:/tmp/.mount_grid/usr/lib64/:/opt/lib:",
            ),
            ("PATH", "/usr/bin"),
        ]));
        assert_eq!(env.get("LD_LIBRARY_PATH").unwrap(), "/opt/lib");
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin");
    }

    #[test]
    fn clean_env_drops_the_library_path_when_only_appimage_dirs_remain() {
        let env = clean_env_from(env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_grid/usr/lib/:/tmp/.mount_grid/lib64/:",
            ),
        ]));
        assert!(!env.contains_key("LD_LIBRARY_PATH"));
    }

    #[test]
    fn clean_env_drops_the_appimage_gtk_hook_variables() {
        let env = clean_env_from(env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            (
                "GTK_PATH",
                "/tmp/.mount_grid//usr/lib/x86_64-linux-gnu/gtk-3.0",
            ),
            (
                "GIO_EXTRA_MODULES",
                "/tmp/.mount_grid/usr/lib/x86_64-linux-gnu/gio/modules",
            ),
            (
                "GDK_PIXBUF_MODULE_FILE",
                "/tmp/.mount_grid//usr/lib/loaders.cache",
            ),
            (
                "GSETTINGS_SCHEMA_DIR",
                "/tmp/.mount_grid//usr/share/glib-2.0/schemas",
            ),
            ("GTK_EXE_PREFIX", "/tmp/.mount_grid//usr"),
            (
                "GTK_IM_MODULE_FILE",
                "/tmp/.mount_grid//usr/lib/immodules.cache",
            ),
            ("GTK_DATA_PREFIX", "/tmp/.mount_grid"),
            ("GDK_BACKEND", "x11"),
            ("GTK_THEME", "Adwaita:dark"),
            ("PATH", "/usr/bin"),
        ]));
        for key in [
            "GTK_PATH",
            "GIO_EXTRA_MODULES",
            "GDK_PIXBUF_MODULE_FILE",
            "GSETTINGS_SCHEMA_DIR",
            "GTK_EXE_PREFIX",
            "GTK_IM_MODULE_FILE",
            "GTK_DATA_PREFIX",
            "GDK_BACKEND",
            "GTK_THEME",
        ] {
            assert!(!env.contains_key(key), "{key} leaked");
        }
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin");
    }

    #[test]
    fn clean_env_drops_the_appimage_python_home_and_path() {
        // The wrapped AppRun exports these; the system Python that runs
        // umu-run then cannot find its own standard library (2026-09-10).
        let env = clean_env_from(env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            ("PYTHONHOME", "/tmp/.mount_grid/usr/"),
            ("PYTHONPATH", "/tmp/.mount_grid/usr/share/pyshared/:"),
            ("PYTHONDONTWRITEBYTECODE", "1"),
            (
                "PATH",
                "/tmp/.mount_grid/usr/bin/:/tmp/.mount_grid/usr/sbin/:/usr/bin:/bin",
            ),
            (
                "PERLLIB",
                "/tmp/.mount_grid/usr/share/perl5/:/tmp/.mount_grid/usr/lib/perl5/:",
            ),
            (
                "QT_PLUGIN_PATH",
                "/tmp/.mount_grid/usr/lib/qt5/plugins/:/usr/lib64/qt5/plugins",
            ),
            (
                "GST_PLUGIN_SYSTEM_PATH_1_0",
                "/tmp/.mount_grid/usr/lib/gstreamer-1.0:",
            ),
        ]));
        for key in [
            "PYTHONHOME",
            "PYTHONPATH",
            "PYTHONDONTWRITEBYTECODE",
            "PERLLIB",
            "GST_PLUGIN_SYSTEM_PATH_1_0",
        ] {
            assert!(!env.contains_key(key), "{key} leaked");
        }
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin:/bin");
        assert_eq!(env.get("QT_PLUGIN_PATH").unwrap(), "/usr/lib64/qt5/plugins");
    }

    #[test]
    fn clean_env_keeps_a_users_own_python_path_outside_the_appimage() {
        let base = env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            ("PYTHONPATH", "/home/me/lib"),
            ("PYTHONHOME", "/opt/py"),
        ]);
        let env = clean_env_from(base);
        assert_eq!(env.get("PYTHONPATH").unwrap(), "/home/me/lib");
        assert_eq!(env.get("PYTHONHOME").unwrap(), "/opt/py");
    }

    // --- a D-Bus address leaked from a flatpak sandbox ------------------------

    fn exists_only<'a>(paths: &'a [&'a str]) -> impl Fn(&Path) -> bool + 'a {
        move |p| paths.iter().any(|known| Path::new(known) == p)
    }

    #[test]
    fn clean_env_repoints_a_dead_bus_address_at_the_runtime_dir_bus() {
        // Gear Lever launches the AppImage through `flatpak-spawn --host`,
        // which forwards its sandbox's bus address. pressure-vessel then
        // fails to bind that socket and Proton dies mid prefix creation
        // (2026-09-10).
        let env = clean_env_with(
            env_of(&[
                ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/flatpak/bus"),
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
                ("FLATPAK_ID", "it.mijorus.gearlever"),
                (
                    "FLATPAK_SANDBOX_DIR",
                    "/home/me/.var/app/it.mijorus.gearlever/sandbox",
                ),
                ("container", "flatpak"),
                ("PATH", "/usr/bin"),
            ]),
            &exists_only(&["/run/user/1000/bus"]),
        );
        assert_eq!(
            env.get("DBUS_SESSION_BUS_ADDRESS").unwrap(),
            "unix:path=/run/user/1000/bus"
        );
        for key in ["FLATPAK_ID", "FLATPAK_SANDBOX_DIR", "container"] {
            assert!(!env.contains_key(key), "{key} leaked");
        }
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin");
    }

    #[test]
    fn clean_env_drops_a_dead_bus_address_when_there_is_no_runtime_dir_bus() {
        let env = clean_env_with(
            env_of(&[
                ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/flatpak/bus"),
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ]),
            &exists_only(&[]),
        );
        assert!(!env.contains_key("DBUS_SESSION_BUS_ADDRESS"));
    }

    #[test]
    fn clean_env_keeps_a_live_bus_address_and_the_flatpak_markers() {
        let base = env_of(&[
            (
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/run/user/1000/bus,guid=abc",
            ),
            ("FLATPAK_ID", "org.example.App"),
            ("container", "flatpak"),
        ]);
        let env = clean_env_with(base.clone(), &exists_only(&["/run/user/1000/bus"]));
        assert_eq!(env, base);
    }

    #[test]
    fn clean_env_leaves_a_non_socket_bus_address_alone() {
        let base = env_of(&[("DBUS_SESSION_BUS_ADDRESS", "tcp:host=localhost,port=1234")]);
        assert_eq!(clean_env_with(base.clone(), &exists_only(&[])), base);
    }

    #[test]
    fn clean_env_repairs_the_bus_address_even_with_the_saved_original_present() {
        let env = clean_env_with(
            env_of(&[
                ("LD_LIBRARY_PATH_ORIG", "/usr/lib"),
                ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/flatpak/bus"),
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ]),
            &exists_only(&["/run/user/1000/bus"]),
        );
        assert_eq!(env.get("LD_LIBRARY_PATH").unwrap(), "/usr/lib");
        assert_eq!(
            env.get("DBUS_SESSION_BUS_ADDRESS").unwrap(),
            "unix:path=/run/user/1000/bus"
        );
    }

    #[test]
    fn clean_env_keeps_gtk_variables_outside_an_appimage() {
        let base = env_of(&[("GDK_BACKEND", "wayland"), ("GTK_THEME", "Breeze")]);
        assert_eq!(clean_env_from(base.clone()), base);
    }

    #[test]
    fn clean_env_removes_the_appimage_share_dir_from_xdg_data_dirs() {
        let env = clean_env_from(env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_grid/usr/share:/usr/share:/usr/local/share",
            ),
        ]));
        assert_eq!(
            env.get("XDG_DATA_DIRS").unwrap(),
            "/usr/share:/usr/local/share"
        );
    }

    #[test]
    fn clean_env_prefers_the_saved_original_over_appdir_stripping() {
        let env = clean_env_from(env_of(&[
            ("APPDIR", "/tmp/.mount_grid"),
            ("LD_LIBRARY_PATH", "/tmp/.mount_grid/usr/lib/"),
            ("LD_LIBRARY_PATH_ORIG", "/usr/lib"),
        ]));
        assert_eq!(env.get("LD_LIBRARY_PATH").unwrap(), "/usr/lib");
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

    /// Writes an executable `#!/bin/sh` stub and returns its path.
    #[cfg(unix)]
    fn shell_stub(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let exe = dir.join(name);
        std::fs::write(&exe, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        exe
    }

    #[cfg(unix)]
    #[test]
    fn spawn_standalone_runs_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let exe = shell_stub(
            dir.path(),
            "stub.sh",
            &format!("touch '{}'", marker.display()),
        );

        let argv = vec![exe.to_string_lossy().into_owned()];
        let mut child = spawn_standalone_emulator(&argv, dir.path()).unwrap();
        // The caller owns the child now, so the stub can simply be waited on.
        child.wait().unwrap();
        assert!(marker.exists(), "the stub never ran");
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_early_exit_reports_a_stub_that_exits_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let exe = shell_stub(dir.path(), "instant-exit.sh", "exit 3");

        let argv = vec![exe.to_string_lossy().into_owned()];
        let child = spawn_standalone_emulator(&argv, dir.path()).unwrap();
        assert_eq!(
            wait_for_early_exit(child, &argv),
            Some(format!(
                "Process exited immediately (code 3).\nCommand:\n{}",
                exe.display()
            ))
        );
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_early_exit_says_nothing_about_a_process_that_is_still_running() {
        let dir = tempfile::tempdir().unwrap();
        let exe = shell_stub(dir.path(), "long-runner.sh", "sleep 2");

        let argv = vec![exe.to_string_lossy().into_owned()];
        let child = spawn_standalone_emulator(&argv, dir.path()).unwrap();
        let started = std::time::Instant::now();
        assert_eq!(wait_for_early_exit(child, &argv), None);
        // It waited for the window, not for the process.
        assert!(
            started.elapsed() < std::time::Duration::from_millis(1500),
            "wait_for_early_exit blocked until the child exited"
        );
    }

    #[test]
    fn the_early_exit_message_joins_the_command_with_spaces() {
        let argv = vec!["/emu/run".to_string(), "--flag".to_string()];
        assert_eq!(
            process_exited_early_message(None, &argv),
            "Process exited immediately (unknown).\nCommand:\n/emu/run --flag"
        );
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
