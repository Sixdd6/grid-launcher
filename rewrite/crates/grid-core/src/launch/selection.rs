//! Emulator selection: picking a platform→emulator default and enumerating
//! compatible emulators. Ports `grid_launcher/emulator/selection.py:214-306`
//! and `grid-launcher.py:3556-3590` (`_emulator_supports_platform`). See
//! `docs/porting/04-emulator-launch.md` §2.

use std::collections::BTreeMap;

use crate::config::EmulatorEntry;

use super::profiles::{platform_matches_keywords, profile_for_entry, EmulatorProfile};

/// Looks up `platform` in `map`: exact key first, then a case-insensitive
/// key scan; a blank value (after trim) at either stage is treated as
/// absent (`mapping_value_for_platform`, selection.py:214).
pub fn mapping_value_for_platform<'a>(
    map: &'a BTreeMap<String, String>,
    platform: &str,
) -> Option<&'a str> {
    let target = platform.trim();
    if target.is_empty() {
        return None;
    }

    if let Some(value) = map.get(target) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    let folded = target.to_lowercase();
    for (key, value) in map {
        if key.trim().to_lowercase() != folded {
            continue;
        }
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

/// Finds an [`EmulatorEntry`] by name, case-insensitively. A blank query
/// (after trim) always returns `None` (`emulator_entry_by_name`,
/// selection.py:234).
pub fn emulator_entry_by_name<'a>(
    emulators: &'a [EmulatorEntry],
    name: &str,
) -> Option<&'a EmulatorEntry> {
    let target = name.trim().to_lowercase();
    if target.is_empty() {
        return None;
    }
    emulators
        .iter()
        .find(|entry| entry.name.trim().to_lowercase() == target)
}

/// Whether `name` should be treated as a RetroArch build: the entry or
/// profile name contains "retroarch", case-insensitively. A simplified
/// stand-in for `_is_retroarch_emulator_name` (emulator_ui_mixin.py:1916),
/// which additionally consults autoprofile metadata not modeled here.
fn is_retroarch_name(name: &str) -> bool {
    name.to_lowercase().contains("retroarch")
}

/// Whether `entry` supports `platform` (`_emulator_supports_platform`,
/// grid-launcher.py:3556; doc 04 §2):
///
/// 1. Blank platform → `true`.
/// 2. Resolve the profile for the entry; `all_platforms` → `true`.
/// 3. Entry or profile name contains "retroarch" → supported iff a
///    non-blank core is mapped for `platform` in `retroarch_cores`.
/// 4. No profile matched → `true`.
/// 5. Otherwise compare `platform` against the profile's
///    `platform_keywords`.
pub fn emulator_supports_platform(
    entry: &EmulatorEntry,
    platform: &str,
    profiles: &[EmulatorProfile],
    retroarch_cores: &BTreeMap<String, String>,
) -> bool {
    let selected = platform.trim();
    if selected.is_empty() {
        return true;
    }

    let profile = profile_for_entry(&entry.name, &entry.path, profiles);

    if profile.is_some_and(|p| p.all_platforms) {
        return true;
    }

    let is_retroarch =
        is_retroarch_name(&entry.name) || profile.is_some_and(|p| is_retroarch_name(&p.name));
    if is_retroarch {
        return mapping_value_for_platform(retroarch_cores, selected).is_some();
    }

    let Some(profile) = profile else {
        return true;
    };

    platform_matches_keywords(selected, &profile.platform_keywords)
}

/// Names of emulators in `emulators` that support `platform`, in config
/// order, skipping entries with a blank name
/// (`compatible_emulator_names_for_platform`, selection.py:254).
pub fn compatible_emulator_names_for_platform(
    emulators: &[EmulatorEntry],
    platform: &str,
    profiles: &[EmulatorProfile],
    retroarch_cores: &BTreeMap<String, String>,
) -> Vec<String> {
    emulators
        .iter()
        .filter_map(|entry| {
            let name = entry.name.trim();
            if name.is_empty() {
                return None;
            }
            if emulator_supports_platform(entry, platform, profiles, retroarch_cores) {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// The default emulator name for `platform` (`default_emulator_name_for_platform`,
/// selection.py:270; doc 04 §2):
///
/// 1. Look up `platform` in `default_emulators` via
///    [`mapping_value_for_platform`].
/// 2. If configured, find the entry by case-insensitive name and keep it
///    only when it supports `platform`.
/// 3. Otherwise fall back to the first name in
///    [`compatible_emulator_names_for_platform`].
/// 4. If nothing matches, `""`.
pub fn default_emulator_name_for_platform(
    emulators: &[EmulatorEntry],
    default_emulators: &BTreeMap<String, String>,
    platform: &str,
    profiles: &[EmulatorProfile],
    retroarch_cores: &BTreeMap<String, String>,
) -> String {
    if let Some(configured) = mapping_value_for_platform(default_emulators, platform) {
        if let Some(entry) = emulator_entry_by_name(emulators, configured) {
            if emulator_supports_platform(entry, platform, profiles, retroarch_cores) {
                return configured.to_string();
            }
        }
    }

    compatible_emulator_names_for_platform(emulators, platform, profiles, retroarch_cores)
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string(),
            args: String::new(),
            ..Default::default()
        }
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn profile_with(name: &str, all_platforms: bool, keywords: &[&str]) -> EmulatorProfile {
        profile_with_tokens(name, all_platforms, keywords, &[])
    }

    fn profile_with_tokens(
        name: &str,
        all_platforms: bool,
        keywords: &[&str],
        tokens: &[&str],
    ) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            match_tokens: tokens.iter().map(|t| t.to_lowercase()).collect(),
            args: "%rom%".to_string(),
            all_platforms,
            platform_keywords: keywords.iter().map(|k| k.to_string()).collect(),
            is_compat_tool: false,
            ..Default::default()
        }
    }

    // --- mapping_value_for_platform -----------------------------------

    #[test]
    fn mapping_exact_key_wins() {
        let m = map(&[("SNES", "snes9x"), ("snes", "other")]);
        assert_eq!(mapping_value_for_platform(&m, "SNES"), Some("snes9x"));
    }

    #[test]
    fn mapping_falls_back_to_case_insensitive_scan() {
        let m = map(&[("SNES", "snes9x")]);
        assert_eq!(mapping_value_for_platform(&m, "snes"), Some("snes9x"));
    }

    #[test]
    fn mapping_blank_value_is_ignored() {
        let m = map(&[("SNES", "   ")]);
        assert_eq!(mapping_value_for_platform(&m, "SNES"), None);
    }

    #[test]
    fn mapping_blank_value_at_exact_key_falls_through_to_scan() {
        // Exact key hits but is blank; a differently-cased key with a real
        // value should still be found by the fallback scan.
        let m = map(&[("SNES", ""), ("snes", "snes9x")]);
        assert_eq!(mapping_value_for_platform(&m, "SNES"), Some("snes9x"));
    }

    #[test]
    fn mapping_blank_platform_returns_none() {
        let m = map(&[("SNES", "snes9x")]);
        assert_eq!(mapping_value_for_platform(&m, "  "), None);
    }

    #[test]
    fn mapping_no_match_returns_none() {
        let m = map(&[("SNES", "snes9x")]);
        assert_eq!(mapping_value_for_platform(&m, "N64"), None);
    }

    // --- emulator_entry_by_name -----------------------------------------

    #[test]
    fn entry_by_name_blank_query_returns_none() {
        let emulators = vec![entry("RetroArch", "/x/retroarch")];
        assert_eq!(emulator_entry_by_name(&emulators, "   "), None);
    }

    #[test]
    fn entry_by_name_matches_case_insensitively() {
        let emulators = vec![entry("RetroArch", "/x/retroarch")];
        let found = emulator_entry_by_name(&emulators, "retroarch");
        assert_eq!(found.map(|e| e.name.as_str()), Some("RetroArch"));
    }

    #[test]
    fn entry_by_name_no_match_returns_none() {
        let emulators = vec![entry("RetroArch", "/x/retroarch")];
        assert_eq!(emulator_entry_by_name(&emulators, "Dolphin"), None);
    }

    // --- emulator_supports_platform --------------------------------------

    #[test]
    fn supports_blank_platform_is_always_true() {
        let e = entry("Anything", "");
        assert!(emulator_supports_platform(&e, "  ", &[], &BTreeMap::new()));
    }

    #[test]
    fn supports_all_platforms_profile_is_always_true() {
        let profiles = vec![profile_with("Cemu", true, &[])];
        let e = entry("Cemu", "/x/cemu.exe");
        assert!(emulator_supports_platform(
            &e,
            "Wii U",
            &profiles,
            &BTreeMap::new()
        ));
    }

    #[test]
    fn supports_no_profile_is_always_true() {
        let e = entry("Homebrew Tool", "/x/tool");
        assert!(emulator_supports_platform(
            &e,
            "PlayStation 2",
            &[],
            &BTreeMap::new()
        ));
    }

    #[test]
    fn supports_retroarch_entry_true_when_core_mapped() {
        let e = entry("RetroArch", "/x/retroarch");
        let cores = map(&[("SNES", "snes9x_libretro")]);
        assert!(emulator_supports_platform(&e, "SNES", &[], &cores));
    }

    #[test]
    fn supports_retroarch_entry_false_when_no_core_mapped() {
        let e = entry("RetroArch", "/x/retroarch");
        assert!(!emulator_supports_platform(
            &e,
            "SNES",
            &[],
            &BTreeMap::new()
        ));
    }

    #[test]
    fn supports_retroarch_gate_applies_when_only_the_profile_name_carries_it() {
        // The entry's own name has no "retroarch" substring; it resolves to
        // a profile purely by executable-basename token match, and that
        // profile's name mentions RetroArch. The gate must still apply —
        // this isolates the `profile.is_some_and(is_retroarch_name)` half
        // of the OR from the entry-name half (already covered by
        // `supports_retroarch_entry_true/false_when_...`).
        let profiles = vec![profile_with_tokens(
            "RetroArch (Multi-System)",
            false,
            &[],
            &["retroarch.appimage"],
        )];
        let e = entry("Multi-System Frontend", "/x/RetroArch.AppImage");
        assert!(!is_retroarch_name(&e.name));
        let matched = profile_for_entry(&e.name, &e.path, &profiles)
            .expect("executable basename should match the profile's token");
        assert!(is_retroarch_name(&matched.name));

        assert!(!emulator_supports_platform(
            &e,
            "SNES",
            &profiles,
            &BTreeMap::new()
        ));
        let cores = map(&[("SNES", "snes9x_libretro")]);
        assert!(emulator_supports_platform(&e, "SNES", &profiles, &cores));
    }

    #[test]
    fn supports_keyword_matcher_used_when_profile_present_and_not_retroarch() {
        let profiles = vec![profile_with("PCSX2", false, &["playstation 2"])];
        let e = entry("PCSX2", "/x/pcsx2-qt");
        assert!(emulator_supports_platform(
            &e,
            "PlayStation 2",
            &profiles,
            &BTreeMap::new()
        ));
        assert!(!emulator_supports_platform(
            &e,
            "PlayStation 3",
            &profiles,
            &BTreeMap::new()
        ));
    }

    // --- compatible_emulator_names_for_platform --------------------------

    #[test]
    fn compatible_names_preserve_config_order_and_skip_blank_names() {
        // Two entries both support the platform (Dolphin via keyword,
        // RetroArch via all_platforms) so the assertion actually exercises
        // order preservation rather than degenerating to a single element.
        let profiles = vec![
            profile_with("PCSX2", false, &["playstation 2"]),
            profile_with("Dolphin", false, &["gamecube"]),
            profile_with("RetroArch", true, &[]),
        ];
        let emulators = vec![
            entry("PCSX2", "/x/pcsx2-qt"),
            entry("", "/x/blank"),
            entry("Dolphin", "/x/dolphin"),
            entry("RetroArch", "/x/retroarch"),
        ];
        let names = compatible_emulator_names_for_platform(
            &emulators,
            "GameCube",
            &profiles,
            &BTreeMap::new(),
        );
        assert_eq!(names, vec!["Dolphin".to_string(), "RetroArch".to_string()]);
    }

    #[test]
    fn compatible_names_filters_unsupported_entries() {
        let profiles = vec![profile_with("PCSX2", false, &["playstation 2"])];
        let emulators = vec![entry("PCSX2", "/x/pcsx2-qt")];
        let names = compatible_emulator_names_for_platform(
            &emulators,
            "PlayStation 3",
            &profiles,
            &BTreeMap::new(),
        );
        assert!(names.is_empty());
    }

    // --- default_emulator_name_for_platform -------------------------------

    #[test]
    fn default_uses_configured_mapping_when_supported() {
        let profiles = vec![profile_with("PCSX2", false, &["playstation 2"])];
        let emulators = vec![entry("PCSX2", "/x/pcsx2-qt")];
        let defaults = map(&[("PlayStation 2", "PCSX2")]);
        let name = default_emulator_name_for_platform(
            &emulators,
            &defaults,
            "PlayStation 2",
            &profiles,
            &BTreeMap::new(),
        );
        assert_eq!(name, "PCSX2");
    }

    #[test]
    fn default_falls_through_to_first_compatible_when_configured_is_unsupported() {
        let profiles = vec![
            profile_with("PCSX2", false, &["playstation 2"]),
            profile_with("Dolphin", false, &["gamecube"]),
        ];
        let emulators = vec![
            entry("PCSX2", "/x/pcsx2-qt"),
            entry("Dolphin", "/x/dolphin"),
        ];
        // Configured default is PCSX2, but the platform is GameCube, which
        // PCSX2 does not support -> falls through to the first compatible
        // entry, Dolphin.
        let defaults = map(&[("GameCube", "PCSX2")]);
        let name = default_emulator_name_for_platform(
            &emulators,
            &defaults,
            "GameCube",
            &profiles,
            &BTreeMap::new(),
        );
        assert_eq!(name, "Dolphin");
    }

    #[test]
    fn default_falls_through_when_configured_entry_missing() {
        let profiles = vec![profile_with("Dolphin", false, &["gamecube"])];
        let emulators = vec![entry("Dolphin", "/x/dolphin")];
        let defaults = map(&[("GameCube", "Nonexistent")]);
        let name = default_emulator_name_for_platform(
            &emulators,
            &defaults,
            "GameCube",
            &profiles,
            &BTreeMap::new(),
        );
        assert_eq!(name, "Dolphin");
    }

    #[test]
    fn default_is_blank_when_nothing_matches() {
        let name = default_emulator_name_for_platform(
            &[],
            &BTreeMap::new(),
            "GameCube",
            &[],
            &BTreeMap::new(),
        );
        assert_eq!(name, "");
    }

    #[test]
    fn default_retroarch_gate_participates_in_fallback() {
        // A RetroArch default is configured but no core is mapped for the
        // platform, so it is treated as unsupported and the algorithm falls
        // back to the next compatible emulator.
        let profiles = vec![profile_with("RetroArch", false, &[])];
        let emulators = vec![
            entry("RetroArch", "/x/retroarch"),
            entry("Dolphin", "/x/dolphin"),
        ];
        let defaults = map(&[("GameCube", "RetroArch")]);
        let name = default_emulator_name_for_platform(
            &emulators,
            &defaults,
            "GameCube",
            &profiles,
            &BTreeMap::new(),
        );
        assert_eq!(name, "Dolphin");
    }
}
