//! Emulator selection: picking a platform→emulator default and enumerating
//! compatible emulators. Ports `grid_launcher/emulator/selection.py:214-306`
//! and `grid-launcher.py:3556-3590` (`_emulator_supports_platform`). See
//! `docs/porting/04-emulator-launch.md` §2.

use std::collections::BTreeMap;

use crate::config::EmulatorEntry;

use super::platform_slugs::slug_for_platform;
use super::profiles::{platform_matches_keywords, profile_for_entry, EmulatorProfile};

/// The reserved `default_emulators` value meaning "this platform has NO
/// emulator, and that choice is remembered". Written when the user picks
/// "(none)" in the Emulators panel. It is deliberately not a legal emulator
/// name (`<` and `>` are not produced by any autoprofile), so it can never
/// collide with a real entry. Removing the key instead would let
/// `autoconfig::backfill_all_defaults` re-fill it on the next
/// `list_platforms`.
pub const NO_EMULATOR: &str = "<none>";

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
pub fn is_retroarch_name(name: &str) -> bool {
    name.to_lowercase().contains("retroarch")
}

/// Resolves a RetroArch entry's INSTALLED compatible cores for a platform.
/// `(entry, platform_name) -> core ids`, empty when none are installed.
///
/// The predicate takes this rather than the `retroarch_cores` config map
/// (design D-RC-1): what a RetroArch build can play is decided by the core
/// files on disk, not by what the user happened to save. The app layer
/// passes a closure over the platform list it is asking about; grid-core's
/// own call sites hold no slug map and pass [`installed_core_resolver`],
/// which reads the process-wide slug registry instead.
pub type CoreResolver<'a> = &'a dyn Fn(&EmulatorEntry, &str) -> Vec<String>;

/// The production [`CoreResolver`] for callers that hold no slug map of
/// their own — the launch path, cloud ops, firmware routing, and the
/// install service, all of which see only a platform NAME. The slug comes
/// from the process-wide registry
/// ([`super::platform_slugs::set_platform_slugs`]), which `list_platforms`
/// fills from the server's platform list.
///
/// Before the first successful platform fetch the registry is empty, so the
/// slug is `""` and `installed_compatible_cores` takes its fuzzy name
/// fallback (D-RC-2).
///
/// Pass it as `&installed_core_resolver`.
pub fn installed_core_resolver(entry: &EmulatorEntry, platform: &str) -> Vec<String> {
    // `slug_for_platform` releases the registry lock before returning, so
    // nothing below runs while it is held.
    let slug = slug_for_platform(platform);
    crate::autoconfig::installed_compatible_cores(platform, &slug, entry)
}

/// A [`CoreResolver`] over a caller-supplied name -> slug map, for the app
/// layer, which holds the platform list the UI is asking about and does not
/// need the registry. An unknown name resolves to an empty slug and
/// therefore to the fuzzy fallback.
pub fn slug_core_resolver(
    slugs: &BTreeMap<String, String>,
) -> impl Fn(&EmulatorEntry, &str) -> Vec<String> + '_ {
    move |entry: &EmulatorEntry, platform: &str| -> Vec<String> {
        // The predicate hands over a trimmed name; the map is keyed on the
        // server's raw spelling, so match on the trimmed form of both.
        let wanted = platform.trim();
        let slug = slugs
            .iter()
            .find(|(name, _)| name.trim() == wanted)
            .map(|(_, slug)| slug.clone())
            .unwrap_or_default();
        crate::autoconfig::installed_compatible_cores(platform, &slug, entry)
    }
}

/// Whether `entry` is a RetroArch build: its own name, or the name of the
/// autoprofile it resolves to, mentions RetroArch (design D-RC-1 step 2).
/// The single spelling of that test for both crates.
pub fn entry_is_retroarch(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool {
    is_retroarch_name(&entry.name)
        || profile_for_entry(&entry.name, &entry.path, profiles)
            .is_some_and(|profile| is_retroarch_name(&profile.name))
}

/// Whether `entry` supports `platform` (`_emulator_supports_platform`,
/// grid-launcher.py:3556; doc 04 §2, as amended by design D-RC-1):
///
/// 1. Blank platform → `true`.
/// 2. Entry or profile name contains "retroarch" → supported iff `cores`
///    resolves at least one installed compatible core. This runs BEFORE the
///    `all_platforms` shortcut, which is the whole point of D-RC-1: the
///    shipped RetroArch autoprofile sets `all_platforms: true`, and the old
///    order let that mark RetroArch compatible with Windows and PS5.
/// 3. Profile `all_platforms` → `true`.
/// 4. No profile matched → `true`.
/// 5. Otherwise compare `platform` against the profile's
///    `platform_keywords`.
pub fn emulator_supports_platform(
    entry: &EmulatorEntry,
    platform: &str,
    profiles: &[EmulatorProfile],
    cores: CoreResolver<'_>,
) -> bool {
    let selected = platform.trim();
    if selected.is_empty() {
        return true;
    }

    let profile = profile_for_entry(&entry.name, &entry.path, profiles);

    if entry_is_retroarch(entry, profiles) {
        return !cores(entry, selected).is_empty();
    }

    if profile.is_some_and(|p| p.all_platforms) {
        return true;
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
    cores: CoreResolver<'_>,
) -> Vec<String> {
    emulators
        .iter()
        .filter_map(|entry| {
            let name = entry.name.trim();
            if name.is_empty() {
                return None;
            }
            if emulator_supports_platform(entry, platform, profiles, cores) {
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
/// 2. If the configured value is [`NO_EMULATOR`], return `""` at once — the
///    user chose "(none)" and no fallback may override it.
/// 3. If configured, find the entry by case-insensitive name and keep it
///    only when it supports `platform`.
/// 4. Otherwise fall back to the first name in
///    [`compatible_emulator_names_for_platform`].
/// 5. If nothing matches, `""`.
pub fn default_emulator_name_for_platform(
    emulators: &[EmulatorEntry],
    default_emulators: &BTreeMap<String, String>,
    platform: &str,
    profiles: &[EmulatorProfile],
    cores: CoreResolver<'_>,
) -> String {
    if let Some(configured) = mapping_value_for_platform(default_emulators, platform) {
        if configured == NO_EMULATOR {
            return String::new();
        }
        if let Some(entry) = emulator_entry_by_name(emulators, configured) {
            if emulator_supports_platform(entry, platform, profiles, cores) {
                return configured.to_string();
            }
        }
    }

    compatible_emulator_names_for_platform(emulators, platform, profiles, cores)
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

    /// A resolver that answers `cores` for every entry and platform.
    fn cores_always(cores: &[&str]) -> impl Fn(&EmulatorEntry, &str) -> Vec<String> {
        let cores: Vec<String> = cores.iter().map(|c| c.to_string()).collect();
        move |_entry, _platform| cores.clone()
    }

    /// A resolver that answers nothing for anything — "no core installed".
    fn no_cores(_entry: &EmulatorEntry, _platform: &str) -> Vec<String> {
        Vec::new()
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

    // --- is_retroarch_name / entry_is_retroarch ---------------------------

    #[test]
    fn retroarch_name_detection_ignores_case_and_matches_substrings() {
        assert!(is_retroarch_name("RetroArch"));
        assert!(is_retroarch_name("my retroarch build"));
        assert!(!is_retroarch_name("Dolphin"));
    }

    #[test]
    fn entry_is_retroarch_matches_the_entry_name_or_the_matched_profile_name() {
        let profiles = vec![profile_with_tokens(
            "RetroArch (Multi-System)",
            false,
            &[],
            &["retroarch.appimage"],
        )];
        // Entry name half of the OR.
        assert!(entry_is_retroarch(
            &entry("RetroArch", "/x/whatever"),
            &profiles
        ));
        // Profile name half: the entry name says nothing about RetroArch.
        assert!(entry_is_retroarch(
            &entry("Multi-System Frontend", "/x/RetroArch.AppImage"),
            &profiles
        ));
        assert!(!entry_is_retroarch(
            &entry("Dolphin", "/x/dolphin-emu"),
            &profiles
        ));
    }

    // --- emulator_supports_platform --------------------------------------

    #[test]
    fn supports_blank_platform_is_always_true() {
        let e = entry("Anything", "");
        assert!(emulator_supports_platform(&e, "  ", &[], &no_cores));
    }

    #[test]
    fn supports_all_platforms_profile_is_always_true() {
        let profiles = vec![profile_with("Cemu", true, &[])];
        let e = entry("Cemu", "/x/cemu.exe");
        assert!(emulator_supports_platform(
            &e, "Wii U", &profiles, &no_cores
        ));
    }

    #[test]
    fn supports_no_profile_is_always_true() {
        let e = entry("Homebrew Tool", "/x/tool");
        assert!(emulator_supports_platform(
            &e,
            "PlayStation 2",
            &[],
            &no_cores
        ));
    }

    #[test]
    fn supports_retroarch_entry_true_when_a_core_is_installed() {
        let e = entry("RetroArch", "/x/retroarch");
        assert!(emulator_supports_platform(
            &e,
            "SNES",
            &[],
            &cores_always(&["snes9x"])
        ));
    }

    #[test]
    fn supports_retroarch_entry_false_when_no_core_is_installed() {
        let e = entry("RetroArch", "/x/retroarch");
        assert!(!emulator_supports_platform(&e, "SNES", &[], &no_cores));
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
            &e, "SNES", &profiles, &no_cores
        ));
        assert!(emulator_supports_platform(
            &e,
            "SNES",
            &profiles,
            &cores_always(&["snes9x"])
        ));
    }

    #[test]
    fn supports_keyword_matcher_used_when_profile_present_and_not_retroarch() {
        let profiles = vec![profile_with("PCSX2", false, &["playstation 2"])];
        let e = entry("PCSX2", "/x/pcsx2-qt");
        assert!(emulator_supports_platform(
            &e,
            "PlayStation 2",
            &profiles,
            &no_cores
        ));
        assert!(!emulator_supports_platform(
            &e,
            "PlayStation 3",
            &profiles,
            &no_cores
        ));
    }

    #[test]
    fn supports_retroarch_gate_beats_all_platforms_when_no_core_is_installed() {
        // Design D-RC-1 and the root cause of report 1: the shipped
        // RetroArch autoprofile sets all_platforms: true, which used to
        // short-circuit ahead of the core gate and make RetroArch the
        // apparent default for every platform, PS5 and Windows included.
        let profiles = vec![profile_with("RetroArch (Multi-System)", true, &[])];
        let e = entry("RetroArch", "/x/retroarch");
        assert!(!emulator_supports_platform(
            &e,
            "PlayStation 5",
            &profiles,
            &no_cores
        ));
    }

    #[test]
    fn supports_retroarch_all_platforms_profile_is_supported_once_a_core_is_installed() {
        let profiles = vec![profile_with("RetroArch (Multi-System)", true, &[])];
        let e = entry("RetroArch", "/x/retroarch");
        let cores = cores_always(&["snes9x"]);
        assert!(emulator_supports_platform(&e, "SNES", &profiles, &cores));
    }

    #[test]
    fn supports_all_platforms_still_wins_for_a_non_retroarch_profile() {
        // The reorder must not touch the native all_platforms path.
        let profiles = vec![profile_with("MAME", true, &[])];
        let e = entry("MAME", "/x/mame");
        assert!(emulator_supports_platform(
            &e,
            "PlayStation 5",
            &profiles,
            &no_cores
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
        // A resolver that answers a core for everything keeps RetroArch compatible, so this still tests ORDER rather than the D-RC-1 gate.
        let names = compatible_emulator_names_for_platform(
            &emulators,
            "GameCube",
            &profiles,
            &cores_always(&["dolphin_core"]),
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
            &no_cores,
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
            &no_cores,
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
            &emulators, &defaults, "GameCube", &profiles, &no_cores,
        );
        assert_eq!(name, "Dolphin");
    }

    #[test]
    fn default_falls_through_when_configured_entry_missing() {
        let profiles = vec![profile_with("Dolphin", false, &["gamecube"])];
        let emulators = vec![entry("Dolphin", "/x/dolphin")];
        let defaults = map(&[("GameCube", "Nonexistent")]);
        let name = default_emulator_name_for_platform(
            &emulators, &defaults, "GameCube", &profiles, &no_cores,
        );
        assert_eq!(name, "Dolphin");
    }

    #[test]
    fn default_is_blank_when_nothing_matches() {
        let name =
            default_emulator_name_for_platform(&[], &BTreeMap::new(), "GameCube", &[], &no_cores);
        assert_eq!(name, "");
    }

    #[test]
    fn default_marker_means_no_emulator_and_never_falls_back() {
        // "(none)" is remembered: a compatible entry exists, but the saved
        // `<none>` marker wins and the caller sees "no emulator".
        let profiles = vec![profile_with("Dolphin", false, &["gamecube"])];
        let emulators = vec![entry("Dolphin", "/x/dolphin")];
        let defaults = map(&[("GameCube", NO_EMULATOR)]);
        let name = default_emulator_name_for_platform(
            &emulators, &defaults, "GameCube", &profiles, &no_cores,
        );
        assert_eq!(name, "");
    }

    #[test]
    fn default_marker_is_returned_verbatim_by_mapping_value() {
        // The raw map keeps the marker; only the name resolver interprets it.
        let m = map(&[("GameCube", NO_EMULATOR)]);
        assert_eq!(
            mapping_value_for_platform(&m, "GameCube"),
            Some(NO_EMULATOR)
        );
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
            &emulators, &defaults, "GameCube", &profiles, &no_cores,
        );
        assert_eq!(name, "Dolphin");
    }

    #[test]
    fn slug_core_resolver_matches_padded_map_keys_on_the_trimmed_name() {
        // The predicate passes a trimmed platform name; a map keyed on the
        // server's padded spelling must still yield the slug (final review G3).
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("retroarch");
        std::fs::write(&exe, b"").unwrap();
        let cores = dir.path().join("cores");
        std::fs::create_dir_all(&cores).unwrap();
        for ext in ["so", "dll", "dylib"] {
            std::fs::write(cores.join(format!("pcsx2_libretro.{ext}")), b"").unwrap();
        }
        let ra = entry("RetroArch", exe.to_str().unwrap());

        let mut slugs = BTreeMap::new();
        slugs.insert(" PlayStation 2 ".to_string(), "ps2".to_string());
        let resolver = slug_core_resolver(&slugs);

        // "PlayStation 2" has no fuzzy hit (best Jaccard 2/3 < 0.7), so only
        // the slug path can find the installed core.
        assert_eq!(resolver(&ra, "PlayStation 2"), vec!["pcsx2".to_string()]);
    }
}
