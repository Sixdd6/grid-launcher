//! Dolphin's `Dolphin.ini`/`GFX.ini` overwrite pair, the `SkipIPL` toggle,
//! and the `[GCPad1]` default controller block.
//!
//! Ports `grid_launcher/emulator/dolphin.py`'s `dolphin_ini_path_candidates`
//! (dolphin.py:228-249), `ensure_dolphin_settings` (dolphin.py:253-315),
//! `ensure_dolphin_skip_ipl` (dolphin.py:318-336) and
//! `ensure_dolphin_gcpad_config` (dolphin.py:371-406). See
//! `docs/porting/05-emulator-autoconfig.md` ("Dolphin") for the behavior
//! contract.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};

use super::{paths, writers, EnsureResult};

/// The `[GCPad1]` XInput default block, transcribed verbatim from
/// dolphin.py:340-368: 27 lines, no leading newline, exactly one trailing
/// newline, values wrapped in backticks.
const DEFAULT_GCPAD_CONFIG: &str = "[GCPad1]\n\
Device = XInput/0/Gamepad\n\
Buttons/A = `Button A`\n\
Buttons/B = `Button B`\n\
Buttons/X = `Button X`\n\
Buttons/Y = `Button Y`\n\
Buttons/Z = `Shoulder R`\n\
Buttons/Start = `Start`\n\
Main Stick/Up = `Left Y+`\n\
Main Stick/Down = `Left Y-`\n\
Main Stick/Left = `Left X-`\n\
Main Stick/Right = `Left X+`\n\
Main Stick/Calibration = 100.00\n\
C-Stick/Up = `Right Y+`\n\
C-Stick/Down = `Right Y-`\n\
C-Stick/Left = `Right X-`\n\
C-Stick/Right = `Right X+`\n\
C-Stick/Calibration = 100.00\n\
Triggers/L = `Trigger L`\n\
Triggers/R = `Trigger R`\n\
Triggers/L-Analog = `Trigger L`\n\
Triggers/R-Analog = `Trigger R`\n\
D-Pad/Up = `Pad N`\n\
D-Pad/Down = `Pad S`\n\
D-Pad/Left = `Pad W`\n\
D-Pad/Right = `Pad E`\n\
Rumble/Motor = `Motor L` | `Motor R`\n";

/// `^\[GCPad1\]` case-insensitive, multiline (dolphin.py:390's
/// `re.search(..., re.MULTILINE | re.IGNORECASE)`) — a `search`, not a
/// full-line match, so `[GCPad1] trailing` still counts as present.
static GCPAD_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"^\[GCPad1\]")
        .case_insensitive(true)
        .multi_line(true)
        .build()
        .unwrap()
});

/// `dolphin_ini_path_candidates` (dolphin.py:228-249): candidates for an ini
/// file named `ini_name`, in order, deduped case-insensitively (first
/// occurrence wins):
///
/// 1. `<exe parent>/User/Config/<ini_name>` — ONLY when the expanded
///    `emulator_path` is absolute (unlike every other candidate in this
///    crate, this one has no is-dir check on the exe path; it always uses
///    the parent).
/// 2. `%APPDATA%/Dolphin Emulator/Config/<ini_name>`, when `APPDATA` is set
///    and non-blank once trimmed.
/// 3. `~/.local/share/dolphin-emu/<ini_name>`.
/// 4. `~/Library/Application Support/Dolphin/<ini_name>`.
/// 5. `~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/<ini_name>`.
///
/// Candidates 3-5 have no `Config` path component — unlike 1 and 2, they are
/// Dolphin's own non-portable data roots, which keep their ini files flat.
pub fn ini_path_candidates(emulator_path: &str, ini_name: &str) -> Vec<PathBuf> {
    let expanded = paths::expand_user(emulator_path);
    let mut candidates = Vec::new();

    if expanded.is_absolute() {
        let parent = expanded.parent().unwrap_or(expanded.as_path());
        candidates.push(parent.join("User").join("Config").join(ini_name));
    }

    if let Ok(appdata) = std::env::var("APPDATA") {
        let trimmed = appdata.trim();
        if !trimmed.is_empty() {
            candidates.push(
                paths::expand_user(trimmed)
                    .join("Dolphin Emulator")
                    .join("Config")
                    .join(ini_name),
            );
        }
    }

    let home = paths::home_dir().unwrap_or_default();
    candidates.push(
        home.join(".local")
            .join("share")
            .join("dolphin-emu")
            .join(ini_name),
    );
    candidates.push(
        home.join("Library")
            .join("Application Support")
            .join("Dolphin")
            .join(ini_name),
    );
    candidates.push(
        home.join(".var")
            .join("app")
            .join("org.DolphinEmu.dolphin-emu")
            .join("data")
            .join("dolphin-emu")
            .join(ini_name),
    );

    paths::dedupe_casefold(candidates)
}

/// Create an empty `portable.txt` next to the executable when absent
/// (dolphin.py:257-262). A no-op for a blank `emulator_path`; any write
/// failure is swallowed.
fn maybe_create_portable_txt(emulator_path: &str) {
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
    let portable_txt = dir.join("portable.txt");
    if !portable_txt.exists() {
        let _ = std::fs::write(&portable_txt, "");
    }
}

/// The first candidate when `force_first` (a non-blank emulator path was
/// given), else the first EXISTING candidate, falling back to the first —
/// `candidates` must be non-empty.
fn select_candidate(candidates: &[PathBuf], force_first: bool) -> PathBuf {
    if force_first {
        return candidates[0].clone();
    }
    candidates
        .iter()
        .find(|c| c.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}

/// Read `path` (empty string when it does not exist), run `apply` over the
/// content, and write the result back only when `apply` reports a change —
/// creating the parent directory first. `Err` on any I/O failure, mirroring
/// each Python entry point's single `try/except OSError` around its own
/// read/write.
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

/// `ensure_dolphin_settings` (dolphin.py:253-315): writes `Dolphin.ini` and
/// `GFX.ini` as two independently fallible scopes.
///
/// Creates `portable.txt` first (see [`maybe_create_portable_txt`]).
/// **Selection rule:** when `emulator_path.trim()` is non-blank,
/// `candidates[0]` is used UNCONDITIONALLY for both files — the portable
/// `User/Config` path; otherwise the first EXISTING candidate, falling back
/// to `candidates[0]`.
///
/// `Dolphin.ini`'s forced overwrites, in order: `[Analytics] Enabled` =
/// `False`, `PermissionAsked` = `True`; `[Display] Fullscreen` = `True`,
/// `RenderToMain` = `True`; `[General] ShowLaunchWarning` = `False`;
/// `[DSP] Volume` = `70`. `GFX.ini`: `[Settings] UseVerticalSync` = `True`.
/// Capitalized `True`/`False` is Dolphin's own convention.
///
/// A read/write failure on one file sets its own path to `None` in the
/// result WITHOUT aborting the other. `config_path` is the `Dolphin.ini`
/// path; `extras["gfx_ini_path"]` is the `GFX.ini` path; `changed` is the OR
/// of both files' writes, true only when a write actually happened.
pub fn ensure_settings(emulator_path: &str) -> EnsureResult {
    maybe_create_portable_txt(emulator_path);

    let force_first = !emulator_path.trim().is_empty();
    let mut changed = false;
    let mut config_path: Option<PathBuf> = None;
    let mut extras = std::collections::BTreeMap::new();

    let dolphin_candidates = ini_path_candidates(emulator_path, "Dolphin.ini");
    if !dolphin_candidates.is_empty() {
        let selected = select_candidate(&dolphin_candidates, force_first);
        if let Ok(file_changed) = write_if_changed(&selected, |content| {
            let (content, c1) = writers::ini_overwrite_section(
                content,
                "Analytics",
                &crate::desired![("Enabled", "False"), ("PermissionAsked", "True")],
            );
            let (content, c2) = writers::ini_overwrite_section(
                &content,
                "Display",
                &crate::desired![("Fullscreen", "True"), ("RenderToMain", "True")],
            );
            let (content, c3) = writers::ini_overwrite_section(
                &content,
                "General",
                &crate::desired![("ShowLaunchWarning", "False")],
            );
            let (content, c4) =
                writers::ini_overwrite_section(&content, "DSP", &crate::desired![("Volume", "70")]);
            (content, c1 || c2 || c3 || c4)
        }) {
            changed = changed || file_changed;
            config_path = Some(selected);
        }
    }

    let gfx_candidates = ini_path_candidates(emulator_path, "GFX.ini");
    if !gfx_candidates.is_empty() {
        let selected = select_candidate(&gfx_candidates, force_first);
        if let Ok(file_changed) = write_if_changed(&selected, |content| {
            writers::ini_overwrite_section(
                content,
                "Settings",
                &crate::desired![("UseVerticalSync", "True")],
            )
        }) {
            changed = changed || file_changed;
            extras.insert("gfx_ini_path".to_string(), selected);
        }
    }

    EnsureResult {
        changed,
        config_path,
        extras,
    }
}

/// `ensure_dolphin_skip_ipl` (dolphin.py:318-336): `[Core] SkipIPL` =
/// `False`, re-enabling the GameCube boot animation.
///
/// **Selects the first EXISTING candidate, falling back to `candidates[0]`
/// — regardless of whether `emulator_path` is blank.** This diverges from
/// [`ensure_settings`]'s selection rule (doc 05's open question, resolved by
/// following the code): the two entry points can target different files
/// for the same emulator path.
pub fn ensure_skip_ipl(emulator_path: &str) -> EnsureResult {
    let candidates = ini_path_candidates(emulator_path, "Dolphin.ini");
    if candidates.is_empty() {
        return EnsureResult::unchanged();
    }
    let selected = select_candidate(&candidates, false);

    match write_if_changed(&selected, |content| {
        writers::ini_overwrite_section(content, "Core", &crate::desired![("SkipIPL", "False")])
    }) {
        Ok(changed) => EnsureResult::at(selected, changed),
        Err(_) => EnsureResult::unchanged(),
    }
}

/// `ensure_dolphin_gcpad_config` (dolphin.py:371-406): appends the fixed
/// `[GCPad1]` XInput block ([`DEFAULT_GCPAD_CONFIG`]) to `GCPadNew.ini` only
/// when no `[GCPad1]` header exists yet.
///
/// **Target selection, three tiers:** the first EXISTING `GCPadNew.ini`
/// candidate; else the parent directory of the first EXISTING `Dolphin.ini`
/// candidate joined with `GCPadNew.ini`; else `candidates[0]` for
/// `GCPadNew.ini`. Present (matched by [`GCPAD_MARKER`]) → no write,
/// `changed = false`; otherwise the block is appended via
/// [`writers::append_block_if_absent`], which also supplies a trailing
/// newline before the block when the existing content lacks one.
///
/// `config_path` is always `None`; `extras["gcpad_ini_path"]` names the
/// target file.
pub fn ensure_gcpad_config(emulator_path: &str) -> EnsureResult {
    let candidates = ini_path_candidates(emulator_path, "GCPadNew.ini");
    if candidates.is_empty() {
        return EnsureResult::unchanged();
    }

    let selected = match candidates.iter().find(|c| c.exists()) {
        Some(existing) => existing.clone(),
        None => {
            let dolphin_candidates = ini_path_candidates(emulator_path, "Dolphin.ini");
            match dolphin_candidates
                .iter()
                .find(|c| c.exists())
                .and_then(|c| c.parent())
            {
                Some(dir) => dir.join("GCPadNew.ini"),
                None => candidates[0].clone(),
            }
        }
    };

    match write_if_changed(&selected, |content| {
        writers::append_block_if_absent(content, &GCPAD_MARKER, DEFAULT_GCPAD_CONFIG)
    }) {
        Ok(changed) => {
            let mut result = EnsureResult::unchanged().with_extra("gcpad_ini_path", selected);
            result.changed = changed;
            result
        }
        Err(_) => EnsureResult::unchanged(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    fn make_exe(temp: &Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("Dolphin");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("dolphin.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    /// `APPDATA` set to a dedicated directory so the fallback candidates
    /// used by an empty `emulator_path` are deterministic; `HOME` pointed at
    /// a temp dir too so the home-based candidates don't touch the real
    /// user's home.
    fn isolated_env(temp: &Path) -> EnvGuard {
        EnvGuard::set(&[
            ("APPDATA", Some(temp.join("appdata").to_str().unwrap())),
            ("HOME", Some(temp.join("home").to_str().unwrap())),
        ])
    }

    // --- ensure_settings ------------------------------------------------

    #[test]
    fn dolphin_writes_both_ini_files() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap());

        assert!(result.changed);
        let dolphin_ini = result.config_path.clone().unwrap();
        let gfx_ini = result.extras.get("gfx_ini_path").cloned().unwrap();
        assert_eq!(
            dolphin_ini,
            dir.join("User").join("Config").join("Dolphin.ini")
        );
        assert_eq!(gfx_ini, dir.join("User").join("Config").join("GFX.ini"));

        let dolphin_text = std::fs::read_to_string(&dolphin_ini).unwrap();
        let gfx_text = std::fs::read_to_string(&gfx_ini).unwrap();
        assert!(dolphin_text.contains("[Display]"));
        assert!(dolphin_text.contains("Fullscreen = True"));
        assert!(dolphin_text.contains("[Analytics]"));
        assert!(dolphin_text.contains("Enabled = False"));
        assert!(dolphin_text.contains("PermissionAsked = True"));
        assert!(dolphin_text.contains("RenderToMain = True"));
        assert!(dolphin_text.contains("[General]"));
        assert!(dolphin_text.contains("ShowLaunchWarning = False"));
        assert!(dolphin_text.contains("[DSP]"));
        assert!(dolphin_text.contains("Volume = 70"));
        assert!(gfx_text.contains("[Settings]"));
        assert!(gfx_text.contains("UseVerticalSync = True"));
    }

    #[test]
    fn dolphin_creates_portable_txt() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap());

        assert!(dir.join("portable.txt").exists());
    }

    #[test]
    fn dolphin_creates_portable_txt_does_not_overwrite_it() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        std::fs::write(dir.join("portable.txt"), "custom").unwrap();

        ensure_settings(exe.to_str().unwrap());

        assert_eq!(
            std::fs::read_to_string(dir.join("portable.txt")).unwrap(),
            "custom"
        );
    }

    #[test]
    fn dolphin_uses_candidate_zero_when_a_path_is_given() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        // Seed a DIFFERENT candidate (the APPDATA one) that exists — must
        // not be selected, since a non-blank path forces candidate[0].
        let appdata_ini = temp
            .path()
            .join("appdata")
            .join("Dolphin Emulator")
            .join("Config")
            .join("Dolphin.ini");
        std::fs::create_dir_all(appdata_ini.parent().unwrap()).unwrap();
        std::fs::write(&appdata_ini, "[Analytics]\nEnabled = True\n").unwrap();

        let result = ensure_settings(exe.to_str().unwrap());

        assert_eq!(
            result.config_path,
            Some(dir.join("User").join("Config").join("Dolphin.ini"))
        );
        assert!(
            !std::fs::read_to_string(&appdata_ini)
                .unwrap()
                .contains("Enabled = False"),
            "the appdata candidate must be untouched"
        );
    }

    // --- ensure_skip_ipl / divergence from ensure_settings ---------------

    #[test]
    fn dolphin_skip_ipl_uses_the_first_existing_candidate() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path());

        // Only the APPDATA candidate exists on disk; the portable
        // User/Config candidate (candidate[0]) does not.
        let appdata_ini = temp
            .path()
            .join("appdata")
            .join("Dolphin Emulator")
            .join("Config")
            .join("Dolphin.ini");
        std::fs::create_dir_all(appdata_ini.parent().unwrap()).unwrap();
        std::fs::write(&appdata_ini, "[Core]\nSkipIPL = True\n").unwrap();

        let result = ensure_skip_ipl(exe.to_str().unwrap());

        assert!(result.changed);
        assert_eq!(result.config_path, Some(appdata_ini.clone()));
        let text = std::fs::read_to_string(&appdata_ini).unwrap();
        assert!(text.contains("SkipIPL = False"));
    }

    #[test]
    fn dolphin_skip_ipl_and_settings_can_target_different_files() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        // Only the APPDATA candidate exists.
        let appdata_ini = temp
            .path()
            .join("appdata")
            .join("Dolphin Emulator")
            .join("Config")
            .join("Dolphin.ini");
        std::fs::create_dir_all(appdata_ini.parent().unwrap()).unwrap();
        std::fs::write(&appdata_ini, "[Core]\nSkipIPL = True\n").unwrap();

        let skip_result = ensure_skip_ipl(exe.to_str().unwrap());
        let settings_result = ensure_settings(exe.to_str().unwrap());

        assert_eq!(skip_result.config_path, Some(appdata_ini));
        assert_eq!(
            settings_result.config_path,
            Some(dir.join("User").join("Config").join("Dolphin.ini")),
            "a non-blank path forces candidate[0] regardless of what exists"
        );
        assert_ne!(skip_result.config_path, settings_result.config_path);
    }

    #[test]
    fn dolphin_skip_ipl_with_empty_path_uses_appdata_fallback() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let result = ensure_skip_ipl("");

        let expected = temp
            .path()
            .join("appdata")
            .join("Dolphin Emulator")
            .join("Config")
            .join("Dolphin.ini");
        assert_eq!(result.config_path, Some(expected));
    }

    // --- ensure_gcpad_config ----------------------------------------------

    #[test]
    fn dolphin_gcpad_appends_the_block_when_absent() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_gcpad_config(exe.to_str().unwrap());

        assert!(result.changed);
        assert_eq!(result.config_path, None);
        let gcpad_path = result.extras.get("gcpad_ini_path").cloned().unwrap();
        assert_eq!(
            gcpad_path,
            dir.join("User").join("Config").join("GCPadNew.ini")
        );
        let text = std::fs::read_to_string(&gcpad_path).unwrap();
        assert!(text.contains("[GCPad1]"));
        assert!(text.contains("Device = XInput/0/Gamepad"));
    }

    #[test]
    fn dolphin_gcpad_skips_when_the_header_exists_case_insensitively() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let gcpad_path = dir.join("User").join("Config").join("GCPadNew.ini");
        std::fs::create_dir_all(gcpad_path.parent().unwrap()).unwrap();
        std::fs::write(&gcpad_path, "[gcpad1]\nDevice = DInput/0/Custom\n").unwrap();

        let result = ensure_gcpad_config(exe.to_str().unwrap());

        assert!(!result.changed);
        let text = std::fs::read_to_string(&gcpad_path).unwrap();
        assert_eq!(text, "[gcpad1]\nDevice = DInput/0/Custom\n");
    }

    #[test]
    fn dolphin_gcpad_adds_a_newline_before_appending_only_when_missing() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let gcpad_path = dir.join("User").join("Config").join("GCPadNew.ini");
        std::fs::create_dir_all(gcpad_path.parent().unwrap()).unwrap();
        std::fs::write(&gcpad_path, "[GCPad2]\nDevice = XInput/1/Gamepad").unwrap();

        ensure_gcpad_config(exe.to_str().unwrap());

        let text = std::fs::read_to_string(&gcpad_path).unwrap();
        assert!(text.starts_with("[GCPad2]\nDevice = XInput/1/Gamepad\n[GCPad1]\n"));
    }

    #[test]
    fn dolphin_gcpad_block_matches_the_reference_byte_for_byte() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_gcpad_config(exe.to_str().unwrap());

        let gcpad_path = dir.join("User").join("Config").join("GCPadNew.ini");
        let text = std::fs::read_to_string(&gcpad_path).unwrap();
        assert_eq!(
            text, DEFAULT_GCPAD_CONFIG,
            "the block is the whole file here"
        );
        assert_eq!(DEFAULT_GCPAD_CONFIG.lines().count(), 27);
        assert!(!DEFAULT_GCPAD_CONFIG.starts_with('\n'));
        assert!(DEFAULT_GCPAD_CONFIG.ends_with('\n'));
        assert!(!DEFAULT_GCPAD_CONFIG.ends_with("\n\n"));
    }

    #[test]
    fn dolphin_gcpad_falls_back_next_to_an_existing_dolphin_ini() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path());

        // No GCPadNew.ini anywhere, but a Dolphin.ini exists at the APPDATA
        // candidate — GCPadNew.ini must land beside it.
        let appdata_dolphin_ini = temp
            .path()
            .join("appdata")
            .join("Dolphin Emulator")
            .join("Config")
            .join("Dolphin.ini");
        std::fs::create_dir_all(appdata_dolphin_ini.parent().unwrap()).unwrap();
        std::fs::write(&appdata_dolphin_ini, "[Core]\n").unwrap();

        let result = ensure_gcpad_config(exe.to_str().unwrap());

        let expected = appdata_dolphin_ini.parent().unwrap().join("GCPadNew.ini");
        assert_eq!(result.extras.get("gcpad_ini_path"), Some(&expected));
        assert!(expected.is_file());
    }
}
