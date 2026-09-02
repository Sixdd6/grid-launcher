//! DuckStation's `settings.ini`: candidate discovery, the memory-card
//! reader, and the settings writer.
//!
//! Ports `grid_launcher/emulator/duckstation.py` (module docstring;
//! function-level citations below). See
//! `docs/porting/05-emulator-autoconfig.md` ("DuckStation —
//! `ensure_duckstation_memory_card_settings`"). DuckStation is NOT a
//! RetroAchievements credential target — it encrypts its token per machine,
//! so pre-filling one is not possible; only the `[Cheevos]` suppression
//! keys are written, unconditionally.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::{paths, writers, EnsureResult};

/// `^\[(.+?)\]\s*$` applied to the trimmed line (duckstation.py:169), same
/// pattern as `writers::SECTION_RE` — duplicated locally because that one
/// is private to its module.
static SECTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[(.+?)\]\s*$").unwrap());

/// `duckstation_config_path_candidates` (duckstation.py:10-47).
///
/// A blank `emulator_path` (untrimmed — this function never trims, unlike
/// [`ensure_memory_card_settings`]) yields no candidates. Otherwise the
/// search root is the parent directory when the expanded path is an
/// existing file or has a non-empty extension, else the path itself; then
/// `%LOCALAPPDATA%/DuckStation` (only when set and non-blank after
/// trimming), `~/Documents/DuckStation`, `~/.local/share/duckstation`,
/// `~/.config/duckstation`, `~/Library/Application Support/DuckStation`,
/// `$XDG_DATA_HOME/duckstation`, `$XDG_CONFIG_HOME/duckstation`. Every root
/// gets `settings.ini` appended, and the whole list is deduped
/// case-insensitively, first occurrence wins.
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf> {
    if emulator_path.is_empty() {
        return Vec::new();
    }

    let expanded = paths::expand_user(emulator_path);
    let has_suffix = expanded
        .extension()
        .map(|ext| !ext.is_empty())
        .unwrap_or(false);
    let root = if expanded.is_file() || has_suffix {
        expanded
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| expanded.clone())
    } else {
        expanded.clone()
    };

    let mut roots = vec![root];

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            roots.push(PathBuf::from(trimmed).join("DuckStation"));
        }
    }

    let home = paths::home_dir().unwrap_or_default();
    roots.push(home.join("Documents").join("DuckStation"));
    roots.push(home.join(".local").join("share").join("duckstation"));
    roots.push(home.join(".config").join("duckstation"));
    roots.push(
        home.join("Library")
            .join("Application Support")
            .join("DuckStation"),
    );

    roots.push(paths::xdg_data_home().join("duckstation"));
    roots.push(paths::xdg_config_home().join("duckstation"));

    let candidates: Vec<PathBuf> = roots
        .into_iter()
        .map(|root| root.join("settings.ini"))
        .collect();
    paths::dedupe_casefold(candidates)
}

/// `duckstation_memory_card_settings`'s return shape (duckstation.py:144-195),
/// defaults matching a config that named no `[MemoryCards]` values.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryCardSettings {
    config_path: String,
    directory: String,
    card1_type: String,
    card2_type: String,
}

impl Default for MemoryCardSettings {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            directory: "memcards".to_string(),
            card1_type: "PerGameTitle".to_string(),
            card2_type: "None".to_string(),
        }
    }
}

// `_duckstation_config_bool` (duckstation.py:50-51) has no port here: it
// only fed `use_playlist_title`, which this module's `MemoryCardSettings`
// omits (see its doc comment) since the writer's desired value is always
// the literal `"true"`, never the parsed one.

/// `duckstation_memory_card_settings` (duckstation.py:144-195): walk the
/// candidates, skipping any that are not a file or fail to read; the first
/// candidate with at least one parsed `[MemoryCards]` line wins.
///
/// `UsePlaylistTitle` is parsed by the Python reference too but never read
/// back by the writer (its desired value is always the literal `"true"`),
/// so it is omitted from this port's return shape.
fn memory_card_settings(emulator_path: &str) -> MemoryCardSettings {
    let mut settings = MemoryCardSettings::default();

    for candidate in config_path_candidates(emulator_path) {
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
            continue;
        };

        let mut current_section = String::new();
        let mut parsed_any = false;

        for raw_line in raw_content.lines() {
            let stripped = raw_line.trim();
            if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with(';') {
                continue;
            }
            if let Some(caps) = SECTION_RE.captures(stripped) {
                current_section = caps[1].trim().to_lowercase();
                continue;
            }
            if current_section != "memorycards" {
                continue;
            }
            let Some(eq_index) = raw_line.find('=') else {
                continue;
            };
            let key = raw_line[..eq_index].trim();
            let value = raw_line[eq_index + 1..].trim();
            if value.is_empty() {
                continue;
            }
            parsed_any = true;
            match key {
                "Directory" => settings.directory = value.to_string(),
                "Card1Type" => settings.card1_type = value.to_string(),
                "Card2Type" => settings.card2_type = value.to_string(),
                _ => {}
            }
        }

        if parsed_any {
            settings.config_path = candidate.to_string_lossy().to_string();
            break;
        }
    }

    settings
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

/// The write-failure fallback: the pre-write reader's `config_path` (which
/// may be blank — never the write target), with `changed = false`
/// (duckstation.py:379-381).
fn result_from_settings(settings: &MemoryCardSettings) -> EnsureResult {
    if settings.config_path.is_empty() {
        EnsureResult::unchanged()
    } else {
        EnsureResult::at(PathBuf::from(&settings.config_path), false)
    }
}

/// `ensure_duckstation_memory_card_settings` (duckstation.py:198-386).
///
/// Every preserve-if-present probe below reads the ORIGINAL, pre-write
/// `raw_content` — frozen before any of this call's own writes — the
/// opposite of PCSX2's progressively-rewritten probe (see
/// `pcsx2::ensure_settings`).
///
/// `emulator_dir` (and therefore the write target, always
/// `<emulator_dir>/settings.ini`) is the trimmed, expanded path itself when
/// it is a directory, else its parent — computed independently of
/// [`config_path_candidates`]'s own (untrimmed, file-or-suffix) root rule,
/// exactly as the Python reference keeps them as two separate helpers
/// (duckstation.py:206 vs. duckstation.py:16-19).
pub fn ensure_memory_card_settings(emulator_path: &str, enable_fullscreen: bool) -> EnsureResult {
    let trimmed = emulator_path.trim();
    let emulator_dir = if trimmed.is_empty() {
        None
    } else {
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
        Some(dir)
    };

    let settings = memory_card_settings(emulator_path);
    let candidates = config_path_candidates(emulator_path);
    if candidates.is_empty() {
        return result_from_settings(&settings);
    }

    let config_path = if let Some(dir) = &emulator_dir {
        dir.join("settings.ini")
    } else {
        let configured = settings.config_path.trim();
        if !configured.is_empty() {
            paths::expand_user(configured)
        } else {
            candidates[0].clone()
        }
    };

    let per_game_types: HashSet<&str> = ["PerGame", "PerGameTitle", "PerGameFileTitle"]
        .into_iter()
        .collect();
    let current_card1 = settings.card1_type.trim();
    let current_card2 = settings.card2_type.trim();

    let directory_value = {
        let dir_trimmed = settings.directory.trim();
        if dir_trimmed.is_empty() {
            "memcards".to_string()
        } else {
            dir_trimmed.to_string()
        }
    };
    let card1_value = if per_game_types.contains(current_card1) {
        current_card1.to_string()
    } else {
        "PerGameTitle".to_string()
    };
    let card2_value = if per_game_types.contains(current_card2) || current_card2 == "None" {
        current_card2.to_string()
    } else {
        "None".to_string()
    };

    let created = !config_path.exists();
    let raw_content = if config_path.exists() {
        std::fs::read_to_string(&config_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut changed = created;
    let mut content = raw_content.clone();

    // 1: [MemoryCards], forced keys with value-level preservation
    // (duckstation.py:231-249).
    apply_section(
        &mut content,
        &mut changed,
        "MemoryCards",
        &crate::desired![
            ("Directory", directory_value.as_str()),
            ("Card1Type", card1_value.as_str()),
            ("Card2Type", card2_value.as_str()),
            ("UsePlaylistTitle", "true"),
        ],
    );

    // 2: [Main] InhibitScreensaver/SetupWizardIncomplete forced;
    // ConfirmPowerOff preserve-if-present, probed against raw_content
    // (duckstation.py:252-261).
    let mut main_desired = crate::desired![
        ("InhibitScreensaver", "true"),
        ("SetupWizardIncomplete", "false")
    ];
    if !writers::section_has_key(&raw_content, "Main", "ConfirmPowerOff") {
        main_desired.push(("ConfirmPowerOff".to_string(), "false".to_string()));
    }
    apply_section(&mut content, &mut changed, "Main", &main_desired);

    // 3: [Display] FullscreenMode forced; Scaling/Scaling24Bit preserve
    // (duckstation.py:263-272).
    let mut display_desired = crate::desired![("FullscreenMode", "Borderless Windowed")];
    if !writers::section_has_key(&raw_content, "Display", "Scaling") {
        display_desired.push(("Scaling".to_string(), "Lanczos".to_string()));
    }
    if !writers::section_has_key(&raw_content, "Display", "Scaling24Bit") {
        display_desired.push(("Scaling24Bit".to_string(), "Lanczos".to_string()));
    }
    apply_section(&mut content, &mut changed, "Display", &display_desired);

    // 4: [AutoUpdater] CheckAtStartup, forced (duckstation.py:275-280).
    apply_section(
        &mut content,
        &mut changed,
        "AutoUpdater",
        &crate::desired![("CheckAtStartup", "false")],
    );

    // 5: [GPU] the 9 keys, per-key preserve, probed against raw_content
    // (duckstation.py:284-301).
    let mut gpu_desired: writers::Desired = Vec::new();
    for (key, value) in [
        ("ResolutionScale", "4"),
        ("PGXPEnable", "true"),
        ("PGXPColorCorrection", "true"),
        ("TextureFilter", "Scale2x"),
        ("SpriteTextureFilter", "Scale2x"),
        ("DitheringMode", "TrueColorFull"),
        ("LineDetectMode", "BasicTriangles"),
        ("DownsampleMode", "Box"),
        ("DownsampleScale", "2"),
    ] {
        if !writers::section_has_key(&raw_content, "GPU", key) {
            gpu_desired.push((key.to_string(), value.to_string()));
        }
    }
    apply_section(&mut content, &mut changed, "GPU", &gpu_desired);

    // 6: [Audio] OutputVolume, preserve; when present the desired set is
    // empty and the writer early-returns without normalizing
    // (duckstation.py:304-309).
    let mut audio_desired: writers::Desired = Vec::new();
    if !writers::section_has_key(&raw_content, "Audio", "OutputVolume") {
        audio_desired.push(("OutputVolume".to_string(), "60".to_string()));
    }
    apply_section(&mut content, &mut changed, "Audio", &audio_desired);

    // 7: [Hotkeys] OpenPauseMenu, same pattern (duckstation.py:312-317).
    let mut hotkeys_desired: writers::Desired = Vec::new();
    if !writers::section_has_key(&raw_content, "Hotkeys", "OpenPauseMenu") {
        hotkeys_desired.push(("OpenPauseMenu".to_string(), "SDL-0/Guide".to_string()));
    }
    apply_section(&mut content, &mut changed, "Hotkeys", &hotkeys_desired);

    // 8: [Pad1], whole 27-key SDL map, gated on `Pad1.Type` being absent
    // from raw_content (duckstation.py:320-353).
    if !writers::section_has_key(&raw_content, "Pad1", "Type") {
        apply_section(
            &mut content,
            &mut changed,
            "Pad1",
            &crate::desired![
                ("Type", "AnalogController"),
                ("Up", "SDL-0/DPadUp"),
                ("Down", "SDL-0/DPadDown"),
                ("Left", "SDL-0/DPadLeft"),
                ("Right", "SDL-0/DPadRight"),
                ("Triangle", "SDL-0/Y"),
                ("Circle", "SDL-0/B"),
                ("Cross", "SDL-0/A"),
                ("Square", "SDL-0/X"),
                ("L1", "SDL-0/LeftShoulder"),
                ("R1", "SDL-0/RightShoulder"),
                ("L2", "SDL-0/+LeftTrigger"),
                ("R2", "SDL-0/+RightTrigger"),
                ("L3", "SDL-0/LeftStick"),
                ("R3", "SDL-0/RightStick"),
                ("Select", "SDL-0/Back"),
                ("Start", "SDL-0/Start"),
                ("LLeft", "SDL-0/-LeftX"),
                ("LRight", "SDL-0/+LeftX"),
                ("LUp", "SDL-0/-LeftY"),
                ("LDown", "SDL-0/+LeftY"),
                ("RLeft", "SDL-0/-RightX"),
                ("RRight", "SDL-0/+RightX"),
                ("RUp", "SDL-0/-RightY"),
                ("RDown", "SDL-0/+RightY"),
                ("LargeMotor", "SDL-0/LargeMotor"),
                ("SmallMotor", "SDL-0/SmallMotor"),
            ],
        );
    }

    // 9: [Main] StartFullscreen, only when enabled (duckstation.py:355-361).
    if enable_fullscreen {
        apply_section(
            &mut content,
            &mut changed,
            "Main",
            &crate::desired![("StartFullscreen", "true")],
        );
    }

    // 10: [Cheevos], forced, unconditional — no credential gate; DuckStation
    // is not an RA target (duckstation.py:363-373).
    apply_section(
        &mut content,
        &mut changed,
        "Cheevos",
        &crate::desired![
            ("Enabled", "true"),
            ("ChallengeMode", "false"),
            ("LeaderboardNotifications", "false"),
            ("LeaderboardTrackers", "false"),
        ],
    );

    if changed {
        if let Some(parent) = config_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return result_from_settings(&settings);
            }
        }
        if std::fs::write(&config_path, &content).is_err() {
            return result_from_settings(&settings);
        }
    }

    // The Python reference re-reads via `duckstation_memory_card_settings`
    // here (duckstation.py:383) but immediately overwrites the result's
    // `config_path`/`changed` with the write target and this call's own
    // `changed` (duckstation.py:384-385) — the re-read's other fields have
    // no `EnsureResult` counterpart (D8), so it is pure dead work and is
    // not ported.
    EnsureResult::at(config_path, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    /// `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`HOME`/`LOCALAPPDATA` all pointed at
    /// `dir` (`LOCALAPPDATA` unset), mirroring `tests/test_duckstation_config.py`.
    fn isolated_env(dir: &Path) -> EnvGuard {
        let dir_str = dir.to_str().unwrap();
        EnvGuard::set(&[
            ("XDG_CONFIG_HOME", Some(dir_str)),
            ("XDG_DATA_HOME", Some(dir_str)),
            ("HOME", Some(dir_str)),
            ("LOCALAPPDATA", None),
        ])
    }

    /// A `<temp>/DuckStation/duckstation.exe` emulator file and its sibling
    /// `settings.ini` path (not created — callers write it themselves).
    fn setup_emulator(temp: &Path) -> (String, PathBuf, PathBuf) {
        let dir = temp.join("DuckStation");
        std::fs::create_dir_all(&dir).unwrap();
        let emulator_path = dir.join("duckstation.exe");
        std::fs::write(&emulator_path, b"").unwrap();
        let config_path = dir.join("settings.ini");
        (
            emulator_path.to_string_lossy().to_string(),
            dir,
            config_path,
        )
    }

    #[test]
    fn duckstation_creates_portable_txt() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, dir, _) = setup_emulator(temp.path());

        ensure_memory_card_settings(&emulator_path, false);

        assert!(dir.join("portable.txt").exists());
    }

    #[test]
    fn duckstation_does_not_overwrite_it() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, dir, _) = setup_emulator(temp.path());
        std::fs::write(dir.join("portable.txt"), "custom").unwrap();

        ensure_memory_card_settings(&emulator_path, false);

        assert_eq!(
            std::fs::read_to_string(dir.join("portable.txt")).unwrap(),
            "custom"
        );
    }

    #[test]
    fn duckstation_forces_per_game_memory_card_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::write(
            &config_path,
            "[MemoryCards]\nCard1Type = Shared\nCard2Type = Shared\nUsePlaylistTitle = false\n",
        )
        .unwrap();

        let result = ensure_memory_card_settings(&emulator_path, false);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("Card1Type = PerGameTitle"));
        assert!(text.contains("Card2Type = None"));
        assert!(text.contains("UsePlaylistTitle = true"));
        assert!(text.contains("Directory = memcards"));
    }

    #[test]
    fn duckstation_preserves_an_explicit_memcard_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::write(
            &config_path,
            "[MemoryCards]\nDirectory = D:/CustomMemcards\nCard1Type = Shared\n",
        )
        .unwrap();

        let result = ensure_memory_card_settings(&emulator_path, false);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("Directory = D:/CustomMemcards"));
        assert!(text.contains("Card1Type = PerGameTitle"));
        assert!(text.contains("Card2Type = None"));
    }

    #[test]
    fn duckstation_forces_fullscreen_mode_and_cheevos_suppression() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::write(
            &config_path,
            "[Main]\nStartFullscreen = false\n[Cheevos]\nEnabled = false\n",
        )
        .unwrap();

        let result = ensure_memory_card_settings(&emulator_path, true);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("FullscreenMode = Borderless Windowed"));
        assert!(text.contains("StartFullscreen = true"));
        assert!(text.contains("Enabled = true"));
        assert!(text.contains("ChallengeMode = false"));
        assert!(text.contains("LeaderboardNotifications = false"));
        assert!(text.contains("LeaderboardTrackers = false"));
    }

    #[test]
    fn duckstation_disables_the_auto_updater() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        let result = ensure_memory_card_settings(&emulator_path, false);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("[AutoUpdater]"));
        assert!(text.contains("CheckAtStartup = false"));
    }

    #[test]
    fn duckstation_preserves_gpu_display_audio_hotkey_and_pad1_when_present() {
        let _lock = crate::test_env::lock();

        // GPU
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let (emulator_path, _, config_path) = setup_emulator(temp.path());
            std::fs::write(&config_path, "[GPU]\nResolutionScale = 2\n").unwrap();
            ensure_memory_card_settings(&emulator_path, false);
            let text = std::fs::read_to_string(&config_path).unwrap();
            assert!(text.contains("ResolutionScale = 2"));
            assert!(!text.contains("ResolutionScale = 4"));
            assert!(
                text.contains("DitheringMode = TrueColorFull"),
                "the other 8 keys still get their defaults"
            );
        }

        // Display
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let (emulator_path, _, config_path) = setup_emulator(temp.path());
            std::fs::write(
                &config_path,
                "[Display]\nScaling = Bilinear\nScaling24Bit = Bilinear\n",
            )
            .unwrap();
            ensure_memory_card_settings(&emulator_path, false);
            let text = std::fs::read_to_string(&config_path).unwrap();
            assert!(text.contains("Scaling = Bilinear"));
            assert!(text.contains("Scaling24Bit = Bilinear"));
            assert!(!text.contains("Scaling = Lanczos"));
        }

        // Audio
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let (emulator_path, _, config_path) = setup_emulator(temp.path());
            std::fs::write(&config_path, "[Audio]\nOutputVolume = 80\n").unwrap();
            ensure_memory_card_settings(&emulator_path, false);
            let text = std::fs::read_to_string(&config_path).unwrap();
            assert!(text.contains("OutputVolume = 80"));
            assert!(!text.contains("OutputVolume = 60"));
        }

        // Hotkeys
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let (emulator_path, _, config_path) = setup_emulator(temp.path());
            std::fs::write(&config_path, "[Hotkeys]\nOpenPauseMenu = Keyboard/Escape\n").unwrap();
            ensure_memory_card_settings(&emulator_path, false);
            let text = std::fs::read_to_string(&config_path).unwrap();
            assert!(text.contains("OpenPauseMenu = Keyboard/Escape"));
            assert!(!text.contains("OpenPauseMenu = SDL-0/Guide"));
        }

        // Pad1
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let (emulator_path, _, config_path) = setup_emulator(temp.path());
            std::fs::write(
                &config_path,
                "[Pad1]\nType = DigitalController\nCross = Keyboard/Z\n",
            )
            .unwrap();
            ensure_memory_card_settings(&emulator_path, false);
            let text = std::fs::read_to_string(&config_path).unwrap();
            assert!(text.contains("Type = DigitalController"));
            assert!(!text.contains("Type = AnalogController"));
            assert!(text.contains("Cross = Keyboard/Z"));
            assert!(!text.contains("Cross = SDL-0/A"));
        }
    }

    #[test]
    fn duckstation_forces_setup_wizard_incomplete_from_true_to_false() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::write(&config_path, "[Main]\nSetupWizardIncomplete = true\n").unwrap();

        let result = ensure_memory_card_settings(&emulator_path, false);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("SetupWizardIncomplete = false"));
        assert!(!text.contains("SetupWizardIncomplete = true"));
    }

    #[test]
    fn duckstation_preserves_existing_cheevos_credentials_through_a_portable_write() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::write(
            &config_path,
            "[Cheevos]\nEnabled = true\nUsername = portable_user\nToken = portable_token\n",
        )
        .unwrap();

        let result = ensure_memory_card_settings(&emulator_path, false);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(result.changed);
        assert!(text.contains("[Cheevos]"));
        assert!(text.contains("Enabled = true"));
        assert!(text.contains("ChallengeMode = false"));
        assert!(text.contains("LeaderboardNotifications = false"));
        assert!(text.contains("LeaderboardTrackers = false"));
        assert!(text.contains("Username = portable_user"));
        assert!(text.contains("Token = portable_token"));
    }

    #[test]
    fn duckstation_probes_the_original_content_not_the_rewritten_one() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        // No [Main] ConfirmPowerOff, but [Audio] OutputVolume is present.
        std::fs::write(&config_path, "[Audio]\nOutputVolume = 80\n").unwrap();

        ensure_memory_card_settings(&emulator_path, false);
        let text = std::fs::read_to_string(&config_path).unwrap();

        assert!(
            text.contains("ConfirmPowerOff = false"),
            "ConfirmPowerOff is absent from the original content, so it must be written: {text}"
        );
        assert!(text.contains("OutputVolume = 80"));
        assert!(!text.contains("OutputVolume = 60"));
    }

    #[test]
    fn duckstation_writes_to_the_emulator_dir_even_when_read_elsewhere() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, dir, config_path) = setup_emulator(temp.path());

        // Seed an XDG config candidate that WOULD be read first if the
        // writer used the read candidate order instead of the emulator dir.
        let xdg_dir = temp.path().join(".config").join("duckstation");
        std::fs::create_dir_all(&xdg_dir).unwrap();
        std::fs::write(
            xdg_dir.join("settings.ini"),
            "[MemoryCards]\nDirectory = /xdg/memcards\n",
        )
        .unwrap();

        let result = ensure_memory_card_settings(&emulator_path, false);

        assert!(result.changed);
        assert_eq!(result.config_path, Some(config_path.clone()));
        assert!(
            config_path.exists(),
            "the write must land in the emulator dir"
        );
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(
            written.contains("Directory = /xdg/memcards"),
            "the XDG value is still migrated in: {written}"
        );
        assert!(
            !dir.join("settings.ini").eq(&xdg_dir.join("settings.ini")),
            "sanity: the two paths must differ"
        );
    }

    #[test]
    fn duckstation_candidate_order_under_xdg_overrides() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let xdg_data = temp.path().join("xdg-data");
        let xdg_config = temp.path().join("xdg-config");
        let home = temp.path().join("home");
        let _guard = EnvGuard::set(&[
            ("XDG_DATA_HOME", Some(xdg_data.to_str().unwrap())),
            ("XDG_CONFIG_HOME", Some(xdg_config.to_str().unwrap())),
            ("HOME", Some(home.to_str().unwrap())),
            ("LOCALAPPDATA", None),
        ]);

        let candidates = config_path_candidates("/nonexistent/duckstation.exe");

        assert!(candidates.contains(&xdg_data.join("duckstation").join("settings.ini")));
        assert!(candidates.contains(&xdg_config.join("duckstation").join("settings.ini")));
        assert!(candidates.contains(
            &home
                .join("Documents")
                .join("DuckStation")
                .join("settings.ini")
        ));
        assert_eq!(
            candidates[0],
            PathBuf::from("/nonexistent").join("settings.ini"),
            "candidate 0 must be the emulator dir"
        );
    }

    #[test]
    fn duckstation_is_idempotent() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, _) = setup_emulator(temp.path());

        let first = ensure_memory_card_settings(&emulator_path, true);
        assert!(first.changed);

        let second = ensure_memory_card_settings(&emulator_path, true);
        assert!(!second.changed, "a second identical run must be a no-op");
    }
}
