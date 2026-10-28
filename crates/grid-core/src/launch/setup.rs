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
use crate::config::{Config, EmulatorEntry, PendingDefault};
use crate::romm::Platform;

use super::selection::NO_EMULATOR;

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
    /// Profile names setup never offers. Empty today: KytyPS5 was held back
    /// as experimental, and is offered for PS5 at the user's request.
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

/// One platform the user ticked, and the emulator they kept or picked.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SetupChoice {
    /// The platform's display label, as [`PlatformSetup::label`].
    pub platform: String,
    pub source_id: String,
}

/// What starting setup does: the catalog installs to queue (each emulator
/// once, and none already configured) and the defaults to apply as they land.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupPlan {
    pub installs: Vec<String>,
    /// Profile names of chosen emulators that are already configured: their
    /// defaults can be applied at once.
    pub already_installed: Vec<String>,
    pub pending: BTreeMap<String, PendingDefault>,
}

/// Checks `choices` against the options `rows` offered and plans the work.
/// A choice for an unknown platform, or an emulator not offered for it, is
/// refused: the screen only offers what [`platform_setup`] listed.
pub fn plan_setup(rows: &[PlatformSetup], choices: &[SetupChoice]) -> Result<SetupPlan, String> {
    let mut plan = SetupPlan {
        installs: Vec::new(),
        already_installed: Vec::new(),
        pending: BTreeMap::new(),
    };
    for choice in choices {
        let row = rows
            .iter()
            .find(|r| r.label == choice.platform)
            .ok_or_else(|| format!("{} is not a platform setup offers", choice.platform))?;
        let option = row
            .options
            .iter()
            .find(|o| o.source_id == choice.source_id)
            .ok_or_else(|| format!("that emulator is not offered for {}", row.label))?;
        let list = if option.installed {
            &mut plan.already_installed
        } else {
            &mut plan.installs
        };
        let key = if option.installed {
            &option.name
        } else {
            &option.source_id
        };
        if !list.contains(key) {
            list.push(key.clone());
        }
        plan.pending.insert(
            row.label.clone(),
            PendingDefault {
                emulator: option.name.clone(),
                name: row.name.clone(),
                slug: row.slug.clone(),
                retroarch: option.kind == OptionKind::Retroarch,
            },
        );
    }
    Ok(plan)
}

/// What happened to one pending default.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PendingOutcome {
    /// The platform's default is now `emulator` (and `core`, for RetroArch).
    Assigned {
        platform: String,
        emulator: String,
        core: Option<String>,
    },
    /// RetroArch installed, but no core for the platform is on disk: the
    /// platform is left without a default rather than pointed at RetroArch
    /// with nothing to run (user requirement 2026-10-28).
    CoreMissing { platform: String, emulator: String },
    /// The user had chosen "(none)" for the platform since; it is kept.
    KeptNone { platform: String },
}

/// Applies the pending defaults waiting on `emulator_name`, which has just
/// installed (or was already configured), and drops them from the pending
/// list. Overrides a default the automatic assignment made, never an explicit
/// "(none)". `cores_on_disk` returns the installed cores compatible with a
/// pending platform for a RetroArch entry (production:
/// `autoconfig::installed_compatible_cores`). Pending entries for other
/// emulators, and for an emulator with no configured entry yet, are kept.
pub fn apply_pending_defaults(
    config: &mut Config,
    emulator_name: &str,
    cores_on_disk: &dyn Fn(&PendingDefault, &EmulatorEntry) -> Vec<String>,
) -> Vec<PendingOutcome> {
    let wanted = emulator_name.trim();
    let Some(entry) = config
        .emulators
        .iter()
        .find(|e| e.name.trim().eq_ignore_ascii_case(wanted))
        .cloned()
    else {
        return Vec::new();
    };
    let labels: Vec<String> = config
        .setup
        .pending
        .iter()
        .filter(|(_, p)| p.emulator.trim().eq_ignore_ascii_case(wanted))
        .map(|(label, _)| label.clone())
        .collect();

    let mut outcomes = Vec::new();
    for label in labels {
        let Some(pending) = config.setup.pending.remove(&label) else {
            continue;
        };
        let current = config.default_emulators.get(&label).map(|v| v.trim());
        if current == Some(NO_EMULATOR) {
            outcomes.push(PendingOutcome::KeptNone { platform: label });
            continue;
        }
        let core = if pending.retroarch {
            let found = cores_on_disk(&pending, &entry);
            let Some(first) = found.first().cloned() else {
                outcomes.push(PendingOutcome::CoreMissing {
                    platform: label,
                    emulator: entry.name.clone(),
                });
                continue;
            };
            let current_core = config.retroarch_cores.get(&label).cloned();
            let core = match current_core {
                Some(c) if found.contains(&c) => c,
                _ => first,
            };
            config.retroarch_cores.insert(label.clone(), core.clone());
            Some(core)
        } else {
            None
        };
        config
            .default_emulators
            .insert(label.clone(), entry.name.clone());
        outcomes.push(PendingOutcome::Assigned {
            platform: label,
            emulator: entry.name.clone(),
            core,
        });
    }
    outcomes
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
    fn ps5_recommends_kytyps5() {
        let row = one("PlayStation 5", "ps5", "linux");
        assert_eq!(recommended_name(&row), Some("KytyPS5 (Playstation 5)"));
    }

    #[test]
    fn a_profile_the_table_does_not_offer_is_left_out() {
        let table = Recommendations {
            not_offered: vec!["KytyPS5 (Playstation 5)".to_string()],
            ..Default::default()
        };
        let row = platform_setup(
            &[platform("PlayStation 5", "ps5", 2)],
            load_profiles(),
            &Config::default(),
            "linux",
            &table,
        )
        .remove(0);
        assert!(row.options.is_empty(), "{:?}", names(&row));
        assert_eq!(row.recommended, None);
    }

    #[test]
    fn zeebo_has_nothing_to_offer() {
        // The e2e first-run-setup fixture's "No emulator available" platform.
        let row = one("Zeebo", "zeebo", "linux");
        assert!(row.options.is_empty(), "{:?}", names(&row));
        assert_eq!(row.core, None);
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

    // --- plan_setup ----------------------------------------------------------

    fn rows() -> Vec<PlatformSetup> {
        setup_for(
            &[
                platform("Sony PlayStation 2", "ps2", 9),
                platform("Super Nintendo Entertainment System", "snes", 40),
                platform("Nintendo Entertainment System", "nes", 30),
            ],
            "linux",
        )
    }

    fn choose(rows: &[PlatformSetup], label: &str) -> SetupChoice {
        let row = rows.iter().find(|r| r.label == label).unwrap();
        SetupChoice {
            platform: label.to_string(),
            source_id: row.recommended.clone().unwrap(),
        }
    }

    #[test]
    fn plan_installs_each_emulator_once_and_records_every_platform() {
        let rows = rows();
        let plan = plan_setup(
            &rows,
            &[
                choose(&rows, "Sony PlayStation 2"),
                choose(&rows, "Super Nintendo Entertainment System"),
                choose(&rows, "Nintendo Entertainment System"),
            ],
        )
        .unwrap();
        // PCSX2, and RetroArch once for both SNES and NES.
        assert_eq!(plan.installs.len(), 2, "{:?}", plan.installs);
        assert_eq!(plan.pending.len(), 3);
        assert!(plan.pending["Super Nintendo Entertainment System"].retroarch);
        assert_eq!(
            plan.pending["Super Nintendo Entertainment System"].slug,
            "snes"
        );
        assert!(!plan.pending["Sony PlayStation 2"].retroarch);
    }

    #[test]
    fn plan_refuses_an_emulator_not_offered_for_the_platform() {
        let rows = rows();
        let ps2_id = rows[0].recommended.clone().unwrap();
        let err = plan_setup(
            &rows,
            &[SetupChoice {
                platform: "Super Nintendo Entertainment System".into(),
                source_id: ps2_id,
            }],
        )
        .unwrap_err();
        assert!(err.contains("not offered"), "{err}");
        assert!(plan_setup(
            &rows,
            &[SetupChoice {
                platform: "Nope".into(),
                source_id: "x/y".into()
            }]
        )
        .is_err());
    }

    // --- apply_pending_defaults ------------------------------------------------

    fn pending(emulator: &str, name: &str, slug: &str, retroarch: bool) -> PendingDefault {
        PendingDefault {
            emulator: emulator.into(),
            name: name.into(),
            slug: slug.into(),
            retroarch,
        }
    }

    fn config_with(entries: &[&str], pend: &[(&str, PendingDefault)]) -> Config {
        let mut config = Config::default();
        for name in entries {
            config.emulators.push(EmulatorEntry {
                name: name.to_string(),
                path: format!("/emu/{name}"),
                ..Default::default()
            });
        }
        for (label, p) in pend {
            config.setup.pending.insert(label.to_string(), p.clone());
        }
        config
    }

    fn cores(
        list: &'static [&'static str],
    ) -> impl Fn(&PendingDefault, &EmulatorEntry) -> Vec<String> {
        move |_, _| list.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn a_standalone_choice_becomes_the_default_and_overrides_an_automatic_one() {
        let mut config = config_with(
            &["Xenia Edge (Xbox 360)"],
            &[(
                "Xbox 360",
                pending(
                    "Xenia Edge (Xbox 360)",
                    "Microsoft Xbox 360",
                    "xbox360",
                    false,
                ),
            )],
        );
        config
            .default_emulators
            .insert("Xbox 360".into(), "Xenia Canary (Xbox 360)".into());
        let out = apply_pending_defaults(&mut config, "Xenia Edge (Xbox 360)", &cores(&[]));
        assert_eq!(
            config.default_emulators["Xbox 360"],
            "Xenia Edge (Xbox 360)"
        );
        assert!(config.setup.pending.is_empty());
        assert!(matches!(
            out[0],
            PendingOutcome::Assigned { core: None, .. }
        ));
    }

    #[test]
    fn retroarch_is_assigned_with_its_core_when_the_core_is_on_disk() {
        let mut config = config_with(
            &["RetroArch (Multi-System)"],
            &[(
                "SNES",
                pending("RetroArch (Multi-System)", "Super Nintendo", "snes", true),
            )],
        );
        let out = apply_pending_defaults(
            &mut config,
            "RetroArch (Multi-System)",
            &cores(&["snes9x", "bsnes"]),
        );
        assert_eq!(config.default_emulators["SNES"], "RetroArch (Multi-System)");
        assert_eq!(config.retroarch_cores["SNES"], "snes9x");
        assert_eq!(
            out,
            [PendingOutcome::Assigned {
                platform: "SNES".into(),
                emulator: "RetroArch (Multi-System)".into(),
                core: Some("snes9x".into())
            }]
        );
    }

    #[test]
    fn retroarch_without_a_core_on_disk_is_not_made_the_default() {
        let mut config = config_with(
            &["RetroArch (Multi-System)"],
            &[(
                "Arcade",
                pending("RetroArch (Multi-System)", "Arcade", "arcade", true),
            )],
        );
        let out = apply_pending_defaults(&mut config, "RetroArch (Multi-System)", &cores(&[]));
        assert!(!config.default_emulators.contains_key("Arcade"));
        assert!(!config.retroarch_cores.contains_key("Arcade"));
        assert!(config.setup.pending.is_empty(), "not retried forever");
        assert!(matches!(out[0], PendingOutcome::CoreMissing { .. }));
    }

    #[test]
    fn an_explicit_none_is_kept() {
        let mut config = config_with(
            &["PCSX2 (Playstation 2)"],
            &[(
                "PS2",
                pending("PCSX2 (Playstation 2)", "Sony PlayStation 2", "ps2", false),
            )],
        );
        config
            .default_emulators
            .insert("PS2".into(), NO_EMULATOR.into());
        let out = apply_pending_defaults(&mut config, "PCSX2 (Playstation 2)", &cores(&[]));
        assert_eq!(config.default_emulators["PS2"], NO_EMULATOR);
        assert_eq!(
            out,
            [PendingOutcome::KeptNone {
                platform: "PS2".into()
            }]
        );
    }

    #[test]
    fn pending_defaults_wait_for_their_own_emulator() {
        let mut config = config_with(
            &["PCSX2 (Playstation 2)"],
            &[
                (
                    "PS2",
                    pending("PCSX2 (Playstation 2)", "Sony PlayStation 2", "ps2", false),
                ),
                (
                    "Dreamcast",
                    pending("Redream (Sega Dreamcast)", "Sega Dreamcast", "dc", false),
                ),
            ],
        );
        // Redream has not installed (no entry): nothing happens for it.
        assert!(
            apply_pending_defaults(&mut config, "Redream (Sega Dreamcast)", &cores(&[])).is_empty()
        );
        assert_eq!(config.setup.pending.len(), 2);
        apply_pending_defaults(&mut config, "PCSX2 (Playstation 2)", &cores(&[]));
        assert_eq!(
            config.setup.pending.keys().collect::<Vec<_>>(),
            ["Dreamcast"]
        );
    }
}
