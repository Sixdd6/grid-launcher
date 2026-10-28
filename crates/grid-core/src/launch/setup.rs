//! First-run emulator setup: for each server platform, the emulators GRID can
//! install for it and the one it recommends.
//!
//! The rules (user decisions 2026-10-28):
//! - **Standalone first.** The options are the catalog emulators installable
//!   on this OS whose `platform_keywords` match the platform's raw name, in
//!   the order `platform-emulators.json` gives for the platform's slug, then
//!   by name. The first is the recommendation.
//! - **RetroArch only where nothing standalone serves the platform, and only
//!   with a core.** It is offered when a core is known for the platform (the
//!   slug map, else the core list by name, the same order the launch path
//!   uses) and recommended only when there is no standalone option. Whether
//!   the core is actually on disk is checked again after the install, before
//!   RetroArch is made the platform's default.
//! - Native PC platforms and the server's "Emulators" shelf never appear.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::autoconfig::cores;
use crate::autoconfig::entry::assignable_server_platforms;
use crate::config::Config;
use crate::romm::Platform;

use super::catalog::{catalog_entries_for_host, mark_installed, CatalogEntry};
use super::profiles::{platform_matches_keywords, profile_available_on_host, EmulatorProfile};

const PLATFORM_EMULATORS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../platform-emulators.json"
));

/// `platform-emulators.json`.
#[derive(Debug, Default, Deserialize)]
pub struct Recommendations {
    /// RomM slug → emulator profile names, most preferred first.
    #[serde(default)]
    pub recommended: BTreeMap<String, Vec<String>>,
    /// Profile names setup never offers (experimental emulators).
    #[serde(default)]
    pub not_offered: Vec<String>,
}

/// The bundled recommendation table, parsed once.
pub fn recommendations() -> &'static Recommendations {
    static TABLE: OnceLock<Recommendations> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(PLATFORM_EMULATORS_JSON).expect("platform-emulators.json parses")
    })
}

/// How an option serves its platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionKind {
    Standalone,
    Retroarch,
}

/// One emulator setup can install for a platform.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SetupOption {
    pub source_id: String,
    pub name: String,
    pub kind: OptionKind,
    /// Already configured (catalog rule: same name or same source).
    pub installed: bool,
}

/// One row of the setup screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlatformSetup {
    /// The display label: what the screen shows and what defaults key by.
    pub label: String,
    pub name: String,
    pub slug: String,
    pub rom_count: i64,
    /// Standalone options first, in recommendation order; RetroArch last.
    pub options: Vec<SetupOption>,
    /// The pre-selected option's `source_id`; `None` when nothing installable
    /// serves the platform.
    pub recommended: Option<String>,
    /// The core RetroArch would run this platform with, when one is known.
    /// `None` means RetroArch is not an option here.
    pub core: Option<String>,
}

/// Every assignable platform of `platforms` with its setup options, as `host`
/// (a `sys.platform`-shaped slug: `linux`, `win32`) would list them. Platforms
/// keep the server's order; the screen sorts.
pub fn platform_setup(
    platforms: &[Platform],
    profiles: &[EmulatorProfile],
    config: &Config,
    host: &str,
    table: &Recommendations,
) -> Vec<PlatformSetup> {
    let mut rows = catalog_entries_for_host(profiles, host);
    mark_installed(&mut rows, config);
    let installable: Vec<(&EmulatorProfile, &CatalogEntry)> = profiles
        .iter()
        .filter(|p| !p.is_compat_tool && profile_available_on_host(p, host))
        .filter(|p| {
            !table
                .not_offered
                .iter()
                .any(|n| n.eq_ignore_ascii_case(p.name.trim()))
        })
        .filter_map(|p| {
            rows.iter()
                .find(|r| r.name.eq_ignore_ascii_case(p.name.trim()))
                .map(|r| (p, r))
        })
        .collect();

    let assignable = assignable_server_platforms(platforms);
    platforms
        .iter()
        .filter(|p| assignable.contains(&p.name))
        .map(|p| platform_row(p, &installable, table))
        .collect()
}

fn platform_row(
    platform: &Platform,
    installable: &[(&EmulatorProfile, &CatalogEntry)],
    table: &Recommendations,
) -> PlatformSetup {
    let option = |(profile, row): &(&EmulatorProfile, &CatalogEntry), kind| SetupOption {
        source_id: row.source_id.clone(),
        name: profile.name.trim().to_string(),
        kind,
        installed: row.installed,
    };

    let mut standalone: Vec<&(&EmulatorProfile, &CatalogEntry)> = installable
        .iter()
        .filter(|(p, _)| !p.all_platforms)
        .filter(|(p, _)| platform_matches_keywords(&platform.name, &p.platform_keywords))
        .collect();
    let preferred = table.recommended.get(platform.slug.trim());
    let rank = |name: &str| -> usize {
        preferred
            .and_then(|list| list.iter().position(|n| n.eq_ignore_ascii_case(name)))
            .unwrap_or(usize::MAX)
    };
    standalone.sort_by(|(a, _), (b, _)| {
        rank(a.name.trim()).cmp(&rank(b.name.trim())).then_with(|| {
            a.name
                .trim()
                .to_lowercase()
                .cmp(&b.name.trim().to_lowercase())
        })
    });
    let mut options: Vec<SetupOption> = standalone
        .iter()
        .map(|entry| option(entry, OptionKind::Standalone))
        .collect();

    let core = retroarch_core_for(&platform.slug, &platform.name);
    if core.is_some() {
        if let Some(retroarch) = installable.iter().find(|(p, _)| p.all_platforms) {
            options.push(option(retroarch, OptionKind::Retroarch));
        }
    }
    let core = core.filter(|_| options.iter().any(|o| o.kind == OptionKind::Retroarch));

    PlatformSetup {
        label: platform.label().to_string(),
        name: platform.name.clone(),
        slug: platform.slug.clone(),
        rom_count: platform.rom_count,
        recommended: options.first().map(|o| o.source_id.clone()),
        options,
        core,
    }
}

/// The first core RetroArch would use for a platform, before anything is
/// installed: the slug map when it knows the slug, else the core list by
/// name — the candidate order `autoconfig::installed_compatible_cores` uses.
pub fn retroarch_core_for(slug: &str, name: &str) -> Option<String> {
    let mut candidates = cores::cores_for_slug(slug.trim(), cores::slug_core_map());
    if candidates.is_empty() {
        candidates = cores::cores_for_platform(name, cores::compatibility_map());
    }
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EmulatorEntry;
    use crate::launch::profiles::load_profiles;

    fn platform(name: &str, slug: &str, rom_count: i64) -> Platform {
        Platform {
            id: 1,
            name: name.to_string(),
            slug: slug.to_string(),
            rom_count,
            custom_name: None,
            display_name: name.to_string(),
        }
    }

    fn setup_for(platforms: &[Platform], host: &str) -> Vec<PlatformSetup> {
        platform_setup(
            platforms,
            load_profiles(),
            &Config::default(),
            host,
            recommendations(),
        )
    }

    fn one(name: &str, slug: &str, host: &str) -> PlatformSetup {
        setup_for(&[platform(name, slug, 10)], host).remove(0)
    }

    fn names(row: &PlatformSetup) -> Vec<&str> {
        row.options.iter().map(|o| o.name.as_str()).collect()
    }

    fn recommended_name(row: &PlatformSetup) -> Option<&str> {
        let id = row.recommended.as_deref()?;
        row.options
            .iter()
            .find(|o| o.source_id == id)
            .map(|o| o.name.as_str())
    }

    #[test]
    fn each_playstation_gets_only_its_own_standalone_emulator() {
        for (name, slug, want) in [
            ("Sony PlayStation", "psx", "DuckStation (Playstation 1)"),
            ("Sony PlayStation 2", "ps2", "PCSX2 (Playstation 2)"),
            (
                "Sony PlayStation Portable",
                "psp",
                "PPSSPP (Playstation Portable)",
            ),
        ] {
            let row = one(name, slug, "linux");
            let standalone: Vec<&str> = row
                .options
                .iter()
                .filter(|o| o.kind == OptionKind::Standalone)
                .map(|o| o.name.as_str())
                .collect();
            assert_eq!(standalone, [want], "{name}");
        }
    }

    #[test]
    fn every_table_entry_names_a_real_installable_profile() {
        let profiles = load_profiles();
        let table = recommendations();
        for (slug, list) in &table.recommended {
            for name in list {
                let profile = profiles.iter().find(|p| p.name.trim() == name);
                assert!(profile.is_some(), "{slug}: no profile named {name:?}");
                assert!(
                    profile.unwrap().source.is_some(),
                    "{slug}: {name:?} has no download source"
                );
                assert!(
                    !profile.unwrap().all_platforms,
                    "{slug}: RetroArch is never listed"
                );
            }
        }
        for name in &table.not_offered {
            assert!(
                profiles.iter().any(|p| p.name.trim() == name),
                "not_offered names no profile: {name:?}"
            );
        }
    }

    #[test]
    fn a_platform_with_a_standalone_emulator_recommends_it_over_retroarch() {
        let row = one("Sony PlayStation 2", "ps2", "linux");
        assert_eq!(recommended_name(&row), Some("PCSX2 (Playstation 2)"));
        assert_eq!(row.options[0].kind, OptionKind::Standalone);
    }

    #[test]
    fn xbox_360_prefers_canary_then_edge() {
        let row = one("Microsoft Xbox 360", "xbox360", "linux");
        assert_eq!(recommended_name(&row), Some("Xenia Canary (Xbox 360)"));
        let n = names(&row);
        assert!(
            n.iter().position(|x| x.starts_with("Xenia Canary"))
                < n.iter().position(|x| x.starts_with("Xenia Edge"))
        );
        // The source-less "Xenia (Xbox 360)" is never installable.
        assert!(!n.contains(&"Xenia (Xbox 360)"));
    }

    #[test]
    fn plain_xbox_does_not_pick_up_the_xbox_360_emulators() {
        let row = one("Microsoft Xbox", "xbox", "linux");
        assert_eq!(recommended_name(&row), Some("Xemu (Xbox)"));
        assert!(!names(&row).iter().any(|n| n.contains("Xenia")));
    }

    #[test]
    fn a_platform_with_no_standalone_emulator_falls_back_to_retroarch_with_its_core() {
        let row = one("Super Nintendo Entertainment System", "snes", "linux");
        assert_eq!(row.options.len(), 1);
        assert_eq!(row.options[0].kind, OptionKind::Retroarch);
        assert_eq!(
            row.recommended.as_deref(),
            Some(row.options[0].source_id.as_str())
        );
        assert!(row.core.is_some(), "RetroArch is offered only with a core");
    }

    #[test]
    fn arcade_uses_retroarch_because_fbneo_has_no_source() {
        let row = one("Arcade", "arcade", "linux");
        assert_eq!(row.options.len(), 1, "{:?}", names(&row));
        assert_eq!(row.options[0].kind, OptionKind::Retroarch);
        assert!(row.core.is_some());
    }

    #[test]
    fn retroarch_is_never_offered_without_a_known_core() {
        // Not a slug the core map knows, and no core list entry by name.
        let row = one("Totally Unknown Console", "totally-unknown", "linux");
        assert!(row.options.is_empty(), "{:?}", names(&row));
        assert_eq!(row.recommended, None);
        assert_eq!(row.core, None);
    }

    #[test]
    fn an_experimental_emulator_is_not_offered() {
        let row = one("PlayStation 5", "ps5", "linux");
        assert!(
            !names(&row).iter().any(|n| n.starts_with("KytyPS5")),
            "{:?}",
            names(&row)
        );
    }

    #[test]
    fn native_platforms_and_the_emulators_shelf_are_left_out() {
        let rows = setup_for(
            &[
                platform("Windows", "win", 5),
                platform("Emulators", "emulators", 3),
                platform("Sony PlayStation 2", "ps2", 9),
            ],
            "linux",
        );
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["Sony PlayStation 2"]
        );
    }

    #[test]
    fn a_custom_named_platform_is_matched_by_name_and_labelled_by_its_label() {
        let mut p = platform("Sony PlayStation 2", "ps2", 4);
        p.custom_name = Some("My PS2".to_string());
        p.display_name = "My PS2".to_string();
        let row = setup_for(&[p], "linux").remove(0);
        assert_eq!(row.label, "My PS2");
        assert_eq!(recommended_name(&row), Some("PCSX2 (Playstation 2)"));
    }

    #[test]
    fn an_already_configured_emulator_is_marked_installed() {
        let mut config = Config::default();
        config.emulators.push(EmulatorEntry {
            name: "PCSX2 (Playstation 2)".to_string(),
            path: "/emu/pcsx2.AppImage".to_string(),
            ..Default::default()
        });
        let row = platform_setup(
            &[platform("Sony PlayStation 2", "ps2", 9)],
            load_profiles(),
            &config,
            "linux",
            recommendations(),
        )
        .remove(0);
        assert!(row.options[0].installed);
    }

    #[test]
    fn the_rom_count_and_slug_come_through() {
        let row = one("Sony PlayStation 2", "ps2", "win32");
        assert_eq!((row.slug.as_str(), row.rom_count), ("ps2", 10));
    }
}
