//! PCSX2's portable `inis/PCSX2.ini`: the settings writer and the narrow
//! RetroAchievements-credentials writer.
//!
//! Ports `grid_launcher/emulator/pcsx2.py`'s `ensure_pcsx2_settings`
//! (pcsx2.py:170-380). See `docs/porting/05-emulator-autoconfig.md`
//! ("PCSX2 — `ensure_pcsx2_settings`").
//!
//! Spec deviation D4 (binding): `emulator_dir` is computed from the
//! EXPANDED, TRIMMED path (`expand_user(text.trim()).parent()`) everywhere
//! in this module. The Python reference computes it from the raw,
//! unexpanded `emulator_path_text` (pcsx2.py:186) — `Path("~/PCSX2/x").parent`
//! stays `~/PCSX2` (a literal `~` directory) rather than resolving through
//! the home directory. That mismatch (the executable-existence check two
//! lines above it already uses the expanded path) is a bug, fixed here.
//!
//! Spec deviation D6 (binding): `bios_directory`/`[Folders] Bios` is
//! OMITTED this milestone — firmware wiring is deferred to a later task.
//!
//! Spec deviation D2 (RA-keys-only fan-out) — no direct Python counterpart:
//! [`ensure_ra_credentials`] is a narrow writer for just the three
//! `[Achievements]` keys, mirroring `retroarch::ensure_ra_credentials`.

use std::path::{Path, PathBuf};

use super::{paths, writers, EnsureResult, RaCredentials};

/// Resolve the executable path to its `inis/PCSX2.ini` target, creating an
/// empty `portable.ini` next to it when absent (pcsx2.py:178-194).
///
/// `None` for a blank path, or when the (expanded, trimmed — D4) path does
/// not exist as a file. A `portable.ini` write failure is swallowed
/// (pcsx2.py:188-192): portable mode is best-effort, never a bail-out
/// condition.
fn resolve_target(emulator_path: &str) -> Option<PathBuf> {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let expanded = paths::expand_user(trimmed);
    if !expanded.is_file() {
        return None;
    }

    let emulator_dir = expanded.parent().map(Path::to_path_buf).unwrap_or_default();

    let portable_ini = emulator_dir.join("portable.ini");
    if !portable_ini.exists() {
        let _ = std::fs::write(&portable_ini, "");
    }

    Some(emulator_dir.join("inis").join("PCSX2.ini"))
}

/// Read `target`'s current text, or `""` when it does not exist yet
/// (pcsx2.py:197).
fn read_content(target: &Path) -> std::io::Result<String> {
    if target.exists() {
        std::fs::read_to_string(target)
    } else {
        Ok(String::new())
    }
}

/// Run one `[section]` through [`writers::ini_overwrite_section`], folding
/// its `changed` bit into the running total.
fn apply_section(
    content: &mut String,
    changed: &mut bool,
    section: &str,
    desired: &writers::Desired,
) {
    let (new_content, section_changed) = writers::ini_overwrite_section(content, section, desired);
    *content = new_content;
    *changed = *changed || section_changed;
}

/// `ensure_pcsx2_settings` (pcsx2.py:170-380), minus `bios_directory` (D6).
///
/// The whole read-transform-write pass is guarded as one unit: any I/O
/// error along the way (an unreadable existing file, a `create_dir_all` or
/// `write` failure) reports [`EnsureResult::unchanged`] rather than a
/// partial write (pcsx2.py:196-378's single `try`/`except OSError`).
///
/// Every preserve-if-present probe below reads `content` as it stands AFTER
/// the writes that precede it — the opposite of DuckStation's frozen
/// `raw_content` probe (see `duckstation::ensure_memory_card_settings`).
pub fn ensure_settings(
    emulator_path: &str,
    enable_fullscreen: bool,
    ra: Option<&RaCredentials>,
) -> EnsureResult {
    let Some(config_path) = resolve_target(emulator_path) else {
        return EnsureResult::unchanged();
    };
    let Ok(mut content) = read_content(&config_path) else {
        return EnsureResult::unchanged();
    };
    let mut changed = false;

    // 1: [UI] SetupWizardIncomplete/SettingsVersion, forced (pcsx2.py:200).
    apply_section(
        &mut content,
        &mut changed,
        "UI",
        &crate::desired![("SetupWizardIncomplete", "false"), ("SettingsVersion", "1")],
    );

    // 2: [AutoUpdater] CheckAtStartup, forced (pcsx2.py:205).
    apply_section(
        &mut content,
        &mut changed,
        "AutoUpdater",
        &crate::desired![("CheckAtStartup", "false")],
    );

    // 3: [UI] InhibitScreensaver forced; ConfirmShutdown/PauseOnFocusLoss/
    // HideMouseCursor each preserve-if-present (pcsx2.py:210-217).
    let mut ui_desired = crate::desired![("InhibitScreensaver", "true")];
    if !writers::section_has_key(&content, "UI", "ConfirmShutdown") {
        ui_desired.push(("ConfirmShutdown".to_string(), "false".to_string()));
    }
    if !writers::section_has_key(&content, "UI", "PauseOnFocusLoss") {
        ui_desired.push(("PauseOnFocusLoss".to_string(), "true".to_string()));
    }
    if !writers::section_has_key(&content, "UI", "HideMouseCursor") {
        ui_desired.push(("HideMouseCursor".to_string(), "true".to_string()));
    }
    apply_section(&mut content, &mut changed, "UI", &ui_desired);

    // 4: [EmuCore] EnableDiscordPresence, forced (pcsx2.py:222).
    apply_section(
        &mut content,
        &mut changed,
        "EmuCore",
        &crate::desired![("EnableDiscordPresence", "false")],
    );

    // 5: [EmuCore] EnableWideScreenPatches/EnableNoInterlacingPatches,
    // per-key preserve (pcsx2.py:231-242).
    let mut emu_desired: writers::Desired = Vec::new();
    for (key, value) in [
        ("EnableWideScreenPatches", "true"),
        ("EnableNoInterlacingPatches", "true"),
    ] {
        if !writers::section_has_key(&content, "EmuCore", key) {
            emu_desired.push((key.to_string(), value.to_string()));
        }
    }
    apply_section(&mut content, &mut changed, "EmuCore", &emu_desired);

    // 6: [Achievements], whole block, only when BOTH RA fields are
    // non-blank after trimming (pcsx2.py:244-252).
    if let Some(ra) = ra {
        let ra_username = ra.username().trim();
        let ra_token = ra.token().trim();
        if !ra_username.is_empty() && !ra_token.is_empty() {
            apply_section(
                &mut content,
                &mut changed,
                "Achievements",
                &crate::desired![
                    ("Enabled", "true"),
                    ("Username", ra_username),
                    ("Token", ra_token)
                ],
            );
        }
    }

    // 7: [EmuCore/GS] pcrtc_antiblur/pcrtc_offsets, forced (pcsx2.py:254).
    apply_section(
        &mut content,
        &mut changed,
        "EmuCore/GS",
        &crate::desired![("pcrtc_antiblur", "true"), ("pcrtc_offsets", "false")],
    );

    // 8: [EmuCore/GS] the 10 quality keys, per-key preserve (pcsx2.py:265-276).
    let mut gs_desired: writers::Desired = Vec::new();
    for (key, value) in [
        ("VsyncEnable", "true"),
        ("Renderer", "14"),
        ("filter", "2"),
        ("accurate_blending_unit", "3"),
        ("MaxAnisotropy", "4"),
        ("dithering_ps2", "2"),
        ("CASMode", "2"),
        ("CASSharpness", "50"),
        ("hw_mipmap", "true"),
        ("texture_preloading", "2"),
    ] {
        if !writers::section_has_key(&content, "EmuCore/GS", key) {
            gs_desired.push((key.to_string(), value.to_string()));
        }
    }
    apply_section(&mut content, &mut changed, "EmuCore/GS", &gs_desired);

    // 9: [EmuCore/Speedhacks] fastCDVD/vuThread/vu1Instant, per-key
    // preserve (pcsx2.py:285-297).
    let mut speedhack_desired: writers::Desired = Vec::new();
    for (key, value) in [
        ("fastCDVD", "false"),
        ("vuThread", "true"),
        ("vu1Instant", "true"),
    ] {
        if !writers::section_has_key(&content, "EmuCore/Speedhacks", key) {
            speedhack_desired.push((key.to_string(), value.to_string()));
        }
    }
    apply_section(
        &mut content,
        &mut changed,
        "EmuCore/Speedhacks",
        &speedhack_desired,
    );

    // 10: [Pad1], whole 35-key SDL map, gated on `Pad1.Type` being absent
    // (pcsx2.py:299-341).
    if !writers::section_has_key(&content, "Pad1", "Type") {
        apply_section(
            &mut content,
            &mut changed,
            "Pad1",
            &crate::desired![
                ("Type", "DualShock2"),
                ("InvertL", "0"),
                ("InvertR", "0"),
                ("Deadzone", "0"),
                ("AxisScale", "1.33"),
                ("LargeMotorScale", "1"),
                ("SmallMotorScale", "1"),
                ("ButtonDeadzone", "0"),
                ("PressureModifier", "0.5"),
                ("Up", "SDL-0/DPadUp"),
                ("Right", "SDL-0/DPadRight"),
                ("Down", "SDL-0/DPadDown"),
                ("Left", "SDL-0/DPadLeft"),
                ("Triangle", "SDL-0/FaceNorth"),
                ("Circle", "SDL-0/FaceEast"),
                ("Cross", "SDL-0/FaceSouth"),
                ("Square", "SDL-0/FaceWest"),
                ("Select", "SDL-0/Back"),
                ("Start", "SDL-0/Start"),
                ("L1", "SDL-0/LeftShoulder"),
                ("L2", "SDL-0/+LeftTrigger"),
                ("R1", "SDL-0/RightShoulder"),
                ("R2", "SDL-0/+RightTrigger"),
                ("L3", "SDL-0/LeftStick"),
                ("R3", "SDL-0/RightStick"),
                ("LUp", "SDL-0/-LeftY"),
                ("LRight", "SDL-0/+LeftX"),
                ("LDown", "SDL-0/+LeftY"),
                ("LLeft", "SDL-0/-LeftX"),
                ("RUp", "SDL-0/-RightY"),
                ("RRight", "SDL-0/+RightX"),
                ("RDown", "SDL-0/+RightY"),
                ("RLeft", "SDL-0/-RightX"),
                ("LargeMotor", "SDL-0/LargeMotor"),
                ("SmallMotor", "SDL-0/SmallMotor"),
            ],
        );
    }

    // 11: [Hotkeys] OpenPauseMenu, gated on absence (pcsx2.py:343-347).
    if !writers::section_has_key(&content, "Hotkeys", "OpenPauseMenu") {
        apply_section(
            &mut content,
            &mut changed,
            "Hotkeys",
            &crate::desired![("OpenPauseMenu", "SDL-0/Guide")],
        );
    }

    // 12: [SPU2/Output] StandardVolume, gated on absence (pcsx2.py:349-353).
    if !writers::section_has_key(&content, "SPU2/Output", "StandardVolume") {
        apply_section(
            &mut content,
            &mut changed,
            "SPU2/Output",
            &crate::desired![("StandardVolume", "40")],
        );
    }

    // 13: [EmuCore/GS] upscale_multiplier, gated on absence (pcsx2.py:355-359).
    if !writers::section_has_key(&content, "EmuCore/GS", "upscale_multiplier") {
        apply_section(
            &mut content,
            &mut changed,
            "EmuCore/GS",
            &crate::desired![("upscale_multiplier", "3")],
        );
    }

    // 14: [UI] StartFullscreen, only when enabled (pcsx2.py:361-365).
    if enable_fullscreen {
        apply_section(
            &mut content,
            &mut changed,
            "UI",
            &crate::desired![("StartFullscreen", "true")],
        );
    }

    // 15 ([Folders] Bios) is OMITTED — D6.

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

/// Spec deviation D2 (RA-keys-only fan-out) — no direct Python counterpart:
/// a narrow writer for the three `[Achievements]` credential keys alone,
/// reusing [`resolve_target`]'s target resolution (portable.ini included)
/// without any of `ensure_settings`'s other managed keys. A no-op
/// ([`EnsureResult::unchanged`]) when either RA field is blank after
/// trimming, checked before any target resolution — so a blank pair never
/// creates `portable.ini` or `inis/`.
pub fn ensure_ra_credentials(emulator_path: &str, ra: &RaCredentials) -> EnsureResult {
    let ra_username = ra.username().trim();
    let ra_token = ra.token().trim();
    if ra_username.is_empty() || ra_token.is_empty() {
        return EnsureResult::unchanged();
    }

    let Some(config_path) = resolve_target(emulator_path) else {
        return EnsureResult::unchanged();
    };
    let Ok(mut content) = read_content(&config_path) else {
        return EnsureResult::unchanged();
    };
    let mut changed = false;

    apply_section(
        &mut content,
        &mut changed,
        "Achievements",
        &crate::desired![
            ("Enabled", "true"),
            ("Username", ra_username),
            ("Token", ra_token)
        ],
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A `<temp>/PCSX2/pcsx2-qt.exe` emulator file and its `inis/PCSX2.ini`
    /// path (not created — callers write it themselves).
    fn setup_emulator(temp: &Path) -> (String, PathBuf, PathBuf) {
        let dir = temp.join("PCSX2");
        std::fs::create_dir_all(&dir).unwrap();
        let emulator_path = dir.join("pcsx2-qt.exe");
        std::fs::write(&emulator_path, b"").unwrap();
        let config_path = dir.join("inis").join("PCSX2.ini");
        (
            emulator_path.to_string_lossy().to_string(),
            dir,
            config_path,
        )
    }

    #[test]
    fn pcsx2_creates_portable_ini() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, dir, _) = setup_emulator(temp.path());

        ensure_settings(&emulator_path, false, None);

        assert!(dir.join("portable.ini").exists());
    }

    #[test]
    fn pcsx2_does_not_overwrite_portable_ini() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, dir, _) = setup_emulator(temp.path());
        std::fs::write(dir.join("portable.ini"), "custom").unwrap();

        ensure_settings(&emulator_path, false, None);

        assert_eq!(
            std::fs::read_to_string(dir.join("portable.ini")).unwrap(),
            "custom"
        );
    }

    #[test]
    fn pcsx2_requires_an_existing_executable_file() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("PCSX2");
        std::fs::create_dir_all(&dir).unwrap();

        // Missing entirely.
        let missing = dir.join("pcsx2-qt.exe");
        assert_eq!(
            ensure_settings(missing.to_str().unwrap(), false, None),
            EnsureResult::unchanged()
        );

        // Exists but is a directory, not a file.
        let as_dir = dir.join("pcsx2-qt-dir");
        std::fs::create_dir_all(&as_dir).unwrap();
        assert_eq!(
            ensure_settings(as_dir.to_str().unwrap(), false, None),
            EnsureResult::unchanged()
        );

        assert_eq!(ensure_settings("", false, None), EnsureResult::unchanged());
        assert_eq!(
            ensure_settings("   ", false, None),
            EnsureResult::unchanged()
        );
    }

    #[test]
    fn pcsx2_expands_a_tilde_path_and_creates_no_literal_tilde_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let _guard = crate::test_env::EnvGuard::set(&[("HOME", Some(home.to_str().unwrap()))]);

        let dir = home.join("PCSX2");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("pcsx2-qt.exe");
        std::fs::write(&exe, b"").unwrap();

        let result = ensure_settings("~/PCSX2/pcsx2-qt.exe", false, None);

        assert!(result.changed);
        assert_eq!(result.config_path, Some(dir.join("inis").join("PCSX2.ini")));
        assert!(
            !temp.path().join("~").exists(),
            "D4: a literal ~ directory must never be created"
        );
        assert!(dir.join("portable.ini").exists());
        assert!(dir.join("inis").join("PCSX2.ini").exists());
    }

    #[test]
    fn pcsx2_writes_the_forced_ui_and_gs_keys() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        let result = ensure_settings(&emulator_path, false, None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert_eq!(result.config_path, Some(config_path));
        assert!(text.contains("SetupWizardIncomplete = false"));
        assert!(text.contains("SettingsVersion = 1"));
        assert!(text.contains("CheckAtStartup = false"));
        assert!(text.contains("InhibitScreensaver = true"));
        assert!(text.contains("EnableDiscordPresence = false"));
        assert!(text.contains("pcrtc_antiblur = true"));
        assert!(text.contains("pcrtc_offsets = false"));
    }

    #[test]
    fn pcsx2_fullscreen_key_only_when_enabled() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        ensure_settings(&emulator_path, false, None);
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(!text.contains("StartFullscreen"));

        let temp2 = tempfile::tempdir().unwrap();
        let (emulator_path2, _, config_path2) = setup_emulator(temp2.path());
        let result = ensure_settings(&emulator_path2, true, None);
        let text2 = std::fs::read_to_string(&config_path2).unwrap();
        assert!(result.changed);
        assert!(text2.contains("StartFullscreen = true"));
    }

    #[test]
    fn pcsx2_ra_block_requires_both_fields() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        let blank_token = RaCredentials::new("user", "");
        ensure_settings(&emulator_path, false, Some(&blank_token));
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(!text.contains("[Achievements]"));

        let temp2 = tempfile::tempdir().unwrap();
        let (emulator_path2, _, config_path2) = setup_emulator(temp2.path());
        let blank_username = RaCredentials::new("", "tok");
        ensure_settings(&emulator_path2, false, Some(&blank_username));
        let text2 = std::fs::read_to_string(&config_path2).unwrap();
        assert!(!text2.contains("[Achievements]"));

        let temp3 = tempfile::tempdir().unwrap();
        let (emulator_path3, _, config_path3) = setup_emulator(temp3.path());
        let both = RaCredentials::new("retro_user", "retro_token");
        let result = ensure_settings(&emulator_path3, false, Some(&both));
        let text3 = std::fs::read_to_string(&config_path3).unwrap();
        assert!(result.changed);
        assert!(text3.contains("[Achievements]"));
        assert!(text3.contains("Enabled = true"));
        assert!(text3.contains("Username = retro_user"));
        assert!(text3.contains("Token = retro_token"));
    }

    #[test]
    fn pcsx2_preserves_an_existing_pad1_block() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, "[Pad1]\nType = DigitalController\n").unwrap();

        ensure_settings(&emulator_path, false, None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(text.contains("Type = DigitalController"));
        assert!(!text.contains("Type = DualShock2"));
        assert!(
            !text.contains("InvertL"),
            "none of the 35 keys should be added when Pad1.Type is already present: {text}"
        );
    }

    #[test]
    fn pcsx2_preserves_hotkey_volume_and_upscale_when_present() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &config_path,
            concat!(
                "[Hotkeys]\nOpenPauseMenu = Keyboard/Escape\n",
                "[SPU2/Output]\nStandardVolume = 80\n",
                "[EmuCore/GS]\nupscale_multiplier = 5\n",
            ),
        )
        .unwrap();

        ensure_settings(&emulator_path, false, None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(text.contains("OpenPauseMenu = Keyboard/Escape"));
        assert!(!text.contains("OpenPauseMenu = SDL-0/Guide"));
        assert!(text.contains("StandardVolume = 80"));
        assert!(!text.contains("StandardVolume = 40"));
        assert!(text.contains("upscale_multiplier = 5"));
        assert!(!text.contains("upscale_multiplier = 3"));
    }

    #[test]
    fn pcsx2_writes_all_ten_gs_quality_keys_when_absent() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        ensure_settings(&emulator_path, false, None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        for line in [
            "VsyncEnable = true",
            "Renderer = 14",
            "filter = 2",
            "accurate_blending_unit = 3",
            "MaxAnisotropy = 4",
            "dithering_ps2 = 2",
            "CASMode = 2",
            "CASSharpness = 50",
            "hw_mipmap = true",
            "texture_preloading = 2",
        ] {
            assert!(text.contains(line), "missing {line} in:\n{text}");
        }
    }

    #[test]
    fn pcsx2_never_writes_folders_bios() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        ensure_settings(&emulator_path, true, None);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(!text.contains("[Folders]"));
        assert!(!text.contains("Bios"));
    }

    #[test]
    fn pcsx2_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, _) = setup_emulator(temp.path());
        let ra = RaCredentials::new("retro_user", "retro_token");

        let first = ensure_settings(&emulator_path, true, Some(&ra));
        assert!(first.changed);

        let second = ensure_settings(&emulator_path, true, Some(&ra));
        assert!(!second.changed, "a second identical run must be a no-op");
    }

    #[test]
    fn pcsx2_ensure_ra_credentials_touches_only_the_achievements_keys() {
        let temp = tempfile::tempdir().unwrap();
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &config_path,
            "[UI]\nMyKey = keep\n[EmuCore/GS]\nRenderer = 99\n",
        )
        .unwrap();

        let ra = RaCredentials::new("retro_user", "retro_token");
        let result = ensure_ra_credentials(&emulator_path, &ra);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(
            text.contains("MyKey = keep"),
            "sentinel must survive: {text}"
        );
        assert!(
            text.contains("Renderer = 99"),
            "non-default GS key must survive: {text}"
        );
        assert!(text.contains("[Achievements]"));
        assert!(text.contains("Enabled = true"));
        assert!(text.contains("Username = retro_user"));
        assert!(text.contains("Token = retro_token"));

        // Narrowness: none of the full writer's other managed keys appear.
        assert!(!text.contains("SetupWizardIncomplete"));
        assert!(!text.contains("EnableDiscordPresence"));
        assert!(!text.contains("[Pad1]"));
    }

    #[test]
    fn pcsx2_ensure_ra_credentials_is_a_no_op_when_either_field_is_blank() {
        let blank_token = RaCredentials::new("retro_user", "");
        assert_eq!(
            ensure_ra_credentials("/does/not/matter", &blank_token),
            EnsureResult::unchanged()
        );

        let blank_username = RaCredentials::new("", "retro_token");
        assert_eq!(
            ensure_ra_credentials("/does/not/matter", &blank_username),
            EnsureResult::unchanged()
        );
    }
}
