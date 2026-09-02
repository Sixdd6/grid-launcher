//! Xemu's add-only `xemu.toml`, the default base data root, and the
//! required-BIOS-files probe.
//!
//! Ports `grid_launcher/emulator/xemu.py`'s `ensure_xemu_settings`
//! (xemu.py:243-297), `xemu_missing_bios_files` (xemu.py:329-336) and
//! `_default_base_root` (xemu.py:120-134). See
//! `docs/porting/05-emulator-autoconfig.md` ("Xemu") for the behavior
//! contract.
//!
//! doc 05's open question ("the section-writer `changed` accumulator") is
//! resolved here by following the Python bytecode rather than its surface
//! syntax: xemu.py:263-297 rebinds the local `changed` after every one of
//! the eight `_ensure_toml_section_values` calls, each time OR-folding in
//! that call's own `_changed` flag alongside every PRIOR flag still in
//! scope (`changed = changed or display_changed`, etc.) — so the net effect
//! is one accumulator OR-folded across all eight calls, which is what
//! [`ensure_settings`] implements directly with a single `bool`.

use std::path::PathBuf;

use super::{paths, writers, EnsureResult};

/// The three files [`missing_bios_files`] checks for, in order.
/// `eeprom.bin` is deliberately NOT in this list even though
/// [`ensure_settings`] writes an `eeprom_path` (xemu.py:330).
const REQUIRED_BIOS_FILES: [&str; 3] = ["mcpx_1.0.bin", "complex_4627.bin", "xbox_hdd.qcow2"];

enum Host {
    Windows,
    Macos,
    Other,
}

fn host() -> Host {
    if cfg!(target_os = "windows") {
        Host::Windows
    } else if cfg!(target_os = "macos") {
        Host::Macos
    } else {
        Host::Other
    }
}

/// `_default_base_root` (xemu.py:120-134): `%APPDATA%/xemu/xemu` on
/// Windows (when `APPDATA` is set and non-blank once trimmed) — falling
/// through to the same XDG-or-dotfile root as every other host when it is
/// not; `~/Library/Application Support/xemu/xemu` on macOS;
/// `$XDG_DATA_HOME/xemu/xemu` else `~/.local/share/xemu/xemu` elsewhere.
pub fn default_base_root() -> PathBuf {
    default_base_root_for(host())
}

/// [`default_base_root`] with an explicit host, so a test can drive all
/// three branches regardless of the host this crate is compiled for.
fn default_base_root_for(host: Host) -> PathBuf {
    if let Host::Windows = host {
        if let Some(appdata) = paths::env_dir("APPDATA") {
            return appdata.join("xemu").join("xemu");
        }
    }
    if let Host::Macos = host {
        return paths::home_dir()
            .unwrap_or_default()
            .join("Library")
            .join("Application Support")
            .join("xemu")
            .join("xemu");
    }
    paths::xdg_data_home().join("xemu").join("xemu")
}

/// `path.trim()` expanded, dir-or-parent — `None` for a blank path
/// (xemu.py:246-249's `emulator_dir` local, which stays `None` for a blank
/// or non-string `emulator_path_text`).
fn resolve_emulator_dir(emulator_path: &str) -> Option<PathBuf> {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = paths::expand_user(trimmed);
    paths::emulator_dir(&expanded)
}

/// The eight `(section, desired)` pairs, in the pinned order
/// (xemu.py:257-315). `base_dir` backs the four `[sys.files]` paths, each
/// wrapped in single quotes with no escaping.
fn sections(base_dir: &std::path::Path) -> Vec<(&'static str, writers::Desired)> {
    vec![
        ("general", crate::desired![("show_welcome", "false")]),
        ("misc", crate::desired![("check_for_updates", "false")]),
        ("display", crate::desired![("vsync", "true")]),
        (
            "display.window",
            crate::desired![("fullscreen_on_startup", "true")],
        ),
        ("display.quality", crate::desired![("surface_scale", "2")]),
        ("audio", crate::desired![("volume_limit", "0.4")]),
        (
            "input.bindings",
            crate::desired![("port1_driver", "\"usb-xbox-gamepad\"")],
        ),
        (
            "sys.files",
            crate::desired![
                (
                    "bootrom_path",
                    format!("'{}'", base_dir.join("mcpx_1.0.bin").display())
                ),
                (
                    "flashrom_path",
                    format!("'{}'", base_dir.join("complex_4627.bin").display())
                ),
                (
                    "hdd_path",
                    format!("'{}'", base_dir.join("xbox_hdd.qcow2").display())
                ),
                (
                    "eeprom_path",
                    format!("'{}'", base_dir.join("eeprom.bin").display())
                ),
            ],
        ),
    ]
}

/// `ensure_xemu_settings` (xemu.py:243-297). Target:
/// `<emulator_dir>/xemu.toml` when a non-blank path is given (trimmed,
/// expanded, dir-or-parent; **no existence check on the file itself**),
/// else `<default_base_root()>/xemu.toml`.
///
/// Every one of the eight sections is written add-only via
/// [`writers::toml_add_only_section`], unconditionally, chaining the
/// content through each call; `changed` is the OR of all eight. The file is
/// written back only when something changed, with the parent directory
/// created lazily. Any I/O error — reading the existing file, creating the
/// parent, or writing — yields [`EnsureResult::unchanged`]
/// (xemu.py:296-297's bare `except OSError`).
pub fn ensure_settings(emulator_path: &str) -> EnsureResult {
    let emulator_dir = resolve_emulator_dir(emulator_path);
    let config_path = match &emulator_dir {
        Some(dir) => dir.join("xemu.toml"),
        None => default_base_root().join("xemu.toml"),
    };

    let content = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => return EnsureResult::unchanged(),
        }
    } else {
        String::new()
    };

    let base_dir = emulator_dir.clone().unwrap_or_else(default_base_root);
    let mut content = content;
    let mut changed = false;
    for (section, desired) in sections(&base_dir) {
        let (new_content, section_changed) =
            writers::toml_add_only_section(&content, section, &desired);
        content = new_content;
        changed |= section_changed;
    }

    if changed {
        if let Some(parent) = config_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return EnsureResult::unchanged();
            }
        }
        if std::fs::write(&config_path, &content).is_err() {
            return EnsureResult::unchanged();
        }
    }

    EnsureResult::at(config_path, changed)
}

/// `xemu_missing_bios_files` (xemu.py:329-336): the subset of
/// [`REQUIRED_BIOS_FILES`] that does not exist under the base directory —
/// the resolved `emulator_dir` when `emulator_path` is non-blank, else
/// [`default_base_root`]. `eeprom.bin` is never checked.
pub fn missing_bios_files(emulator_path: &str) -> Vec<&'static str> {
    let base_dir = resolve_emulator_dir(emulator_path).unwrap_or_else(default_base_root);
    REQUIRED_BIOS_FILES
        .into_iter()
        .filter(|name| !base_dir.join(name).exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    fn make_exe(temp: &std::path::Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("xemu");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("xemu.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    // --- ensure_settings -----------------------------------------------

    #[test]
    fn xemu_writes_all_eight_sections_on_a_fresh_file() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap());

        assert!(result.changed);
        let config_path = result.config_path.clone().unwrap();
        assert_eq!(config_path, dir.join("xemu.toml"));
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("[general]"));
        assert!(text.contains("show_welcome = false"));
        assert!(text.contains("[misc]"));
        assert!(text.contains("check_for_updates = false"));
        assert!(text.contains("[display]"));
        assert!(text.contains("vsync = true"));
        assert!(text.contains("[display.window]"));
        assert!(text.contains("fullscreen_on_startup = true"));
        assert!(text.contains("[display.quality]"));
        assert!(text.contains("surface_scale = 2"));
        assert!(text.contains("[audio]"));
        assert!(text.contains("volume_limit = 0.4"));
        assert!(text.contains("[input.bindings]"));
        assert!(text.contains("[sys.files]"));
        assert!(text.contains("bootrom_path ="));
        assert!(text.contains("flashrom_path ="));
        assert!(text.contains("hdd_path ="));
        assert!(text.contains("eeprom_path ="));
    }

    #[test]
    fn xemu_input_driver_value_keeps_its_double_quotes() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, _dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        let text = std::fs::read_to_string(temp.path().join("xemu").join("xemu.toml")).unwrap();
        assert!(text.contains(r#"port1_driver = "usb-xbox-gamepad""#));
    }

    #[test]
    fn xemu_sys_files_paths_are_single_quoted_absolute() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        let text = std::fs::read_to_string(dir.join("xemu.toml")).unwrap();
        let expected = format!("'{}'", dir.join("mcpx_1.0.bin").display());
        assert!(
            text.contains(&format!("bootrom_path = {expected}")),
            "expected {expected:?} in {text}"
        );
        assert!(dir.is_absolute());
    }

    #[test]
    fn xemu_add_only_per_key() {
        let cases: &[(&str, &str, &str)] = &[
            ("general", "show_welcome", "true"),
            ("misc", "check_for_updates", "true"),
            ("display", "vsync", "false"),
            ("display.window", "fullscreen_on_startup", "false"),
            ("display.quality", "surface_scale", "1"),
            ("audio", "volume_limit", "1.0"),
            ("input.bindings", "port1_driver", "\"other\""),
        ];

        for (section, key, existing_value) in cases {
            let temp = tempfile::tempdir().unwrap();
            let (exe, dir) = make_exe(temp.path());
            std::fs::write(
                dir.join("xemu.toml"),
                format!("[{section}]\n{key} = {existing_value}\n"),
            )
            .unwrap();

            ensure_settings(exe.to_str().unwrap());

            let text = std::fs::read_to_string(dir.join("xemu.toml")).unwrap();
            assert!(
                text.contains(&format!("{key} = {existing_value}")),
                "{section}.{key} must survive as {existing_value:?}: {text}"
            );
        }
    }

    #[test]
    fn xemu_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());
        let text_after_first = std::fs::read_to_string(dir.join("xemu.toml")).unwrap();

        let second = ensure_settings(exe.to_str().unwrap());

        assert!(!second.changed);
        assert_eq!(
            std::fs::read_to_string(dir.join("xemu.toml")).unwrap(),
            text_after_first
        );
    }

    #[test]
    fn xemu_blank_path_targets_default_base_root() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set(&[
            ("HOME", Some(temp.path().to_str().unwrap())),
            ("XDG_DATA_HOME", None),
        ]);

        let result = ensure_settings("");

        assert!(result.changed);
        assert_eq!(
            result.config_path,
            Some(
                temp.path()
                    .join(".local")
                    .join("share")
                    .join("xemu")
                    .join("xemu")
                    .join("xemu.toml")
            )
        );
    }

    // --- missing_bios_files -----------------------------------------------

    #[test]
    fn xemu_missing_bios_files_excludes_eeprom() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let all_absent = missing_bios_files(exe.to_str().unwrap());
        assert!(all_absent.contains(&"mcpx_1.0.bin"));
        assert!(all_absent.contains(&"complex_4627.bin"));
        assert!(all_absent.contains(&"xbox_hdd.qcow2"));
        assert!(!all_absent.contains(&"eeprom.bin"));

        for name in ["mcpx_1.0.bin", "complex_4627.bin", "xbox_hdd.qcow2"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let all_present = missing_bios_files(exe.to_str().unwrap());
        assert!(all_present.is_empty());
    }

    // --- default_base_root --------------------------------------------------

    #[test]
    fn default_base_root_for_each_host() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set(&[
            ("HOME", Some(temp.path().to_str().unwrap())),
            (
                "APPDATA",
                Some(temp.path().join("appdata").to_str().unwrap()),
            ),
            ("XDG_DATA_HOME", None),
        ]);

        assert_eq!(
            default_base_root_for(Host::Windows),
            temp.path().join("appdata").join("xemu").join("xemu")
        );
        assert_eq!(
            default_base_root_for(Host::Macos),
            temp.path()
                .join("Library")
                .join("Application Support")
                .join("xemu")
                .join("xemu")
        );
        assert_eq!(
            default_base_root_for(Host::Other),
            temp.path()
                .join(".local")
                .join("share")
                .join("xemu")
                .join("xemu")
        );
    }
}
