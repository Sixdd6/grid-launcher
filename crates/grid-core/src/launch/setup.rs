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
    /// A catalog download (GitHub and the like).
    Standalone,
    /// A package on the RomM server's Emulators platform.
    Server,
    Retroarch,
}

/// One emulator setup can install for a platform.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SetupOption {
    pub source_id: String,
    pub name: String,
    pub kind: OptionKind,
    /// Already configured (catalog rule: same name or same source; a server
    /// package: installed, with its entry found).
    pub installed: bool,
    /// A server package's ROM id; `None` for a catalog download.
    pub rom_id: Option<i64>,
    /// An installed server package's emulator entry name, which may carry the
    /// " (server)" suffix: what its defaults are set to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

/// One package on the server's Emulators platform, as setup sees it before
/// anything is downloaded.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerPackage {
    pub rom_id: i64,
    /// The ROM's name, which identifies the emulator ("Pico-8").
    pub title: String,
    /// The package's file names: they tell which OS a build is for.
    pub file_names: Vec<String>,
    /// Already installed (a library row for this ROM).
    pub installed: bool,
}

/// The emulator a server package is, judged before downloading it: the
/// package's name equals a profile's name, or its name without the platform
/// suffix ("Pico-8", "PCSX2" for "PCSX2 (Playstation 2)"), case-insensitive.
/// A package whose file names say it is built only for another OS is not
/// offered. RetroArch and compat tools are never matched: setup offers
/// RetroArch from the catalog, with its core rule. An unmatched package is
/// left out of setup (user decision 2026-10-28); the Server view still
/// installs it.
pub fn identify_package<'a>(
    package: &ServerPackage,
    profiles: &'a [EmulatorProfile],
    host: &str,
) -> Option<&'a EmulatorProfile> {
    if !package_fits_host(&package.file_names, host) {
        return None;
    }
    let title = package.title.trim().to_lowercase();
    if title.is_empty() {
        return None;
    }
    profiles
        .iter()
        .filter(|p| !p.is_compat_tool && !p.all_platforms && !p.platform_keywords.is_empty())
        .find(|p| {
            let full = p.name.trim().to_lowercase();
            let short = full.split(" (").next().unwrap_or(&full).trim().to_string();
            title == full || title == short
        })
}

/// The OS a package file is built for, from its name: `win32`, `linux`,
/// `darwin`, or `None` when the name does not say.
fn os_of_file(file_name: &str) -> Option<&'static str> {
    let name = file_name.to_lowercase();
    if name.contains("windows")
        || name.contains("win64")
        || name.contains("win32")
        || name.ends_with(".exe")
        || name.ends_with(".msi")
    {
        Some("win32")
    } else if name.contains("macos")
        || name.contains("darwin")
        || name.contains("osx")
        || name.ends_with(".dmg")
    {
        Some("darwin")
    } else if name.contains("linux") || name.ends_with(".appimage") {
        Some("linux")
    } else {
        None
    }
}

/// Whether a package has something for `host`: no file names an OS (it may
/// well be portable), or one of them names this one.
fn package_fits_host(file_names: &[String], host: &str) -> bool {
    let hints: Vec<&str> = file_names.iter().filter_map(|f| os_of_file(f)).collect();
    hints.is_empty() || hints.iter().any(|os| host.starts_with(os))
}

/// A server package matched to its emulator, with its existing entry when it
/// is installed.
struct MatchedPackage<'a> {
    profile: &'a EmulatorProfile,
    package: &'a ServerPackage,
    entry: Option<String>,
}

/// An installed server package's entry: `<Name> (server)` when the plain
/// name was taken, else `<Name>` — never a catalog entry (non-blank
/// `source_id`), which a server package does not own.
fn server_entry_name(config: &Config, profile_name: &str) -> Option<String> {
    let suffixed = format!(
        "{profile_name}{}",
        crate::library::server_emulator::SERVER_SUFFIX
    );
    config
        .emulators
        .iter()
        .find(|e| e.name.trim().eq_ignore_ascii_case(&suffixed))
        .or_else(|| {
            config.emulators.iter().find(|e| {
                e.name.trim().eq_ignore_ascii_case(profile_name) && e.source_id.trim().is_empty()
            })
        })
        .map(|e| e.name.trim().to_string())
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
/// (a `sys.platform`-shaped slug: `linux`, `win32`) would list them, including
/// the server's own emulator `packages`. Platforms keep the given order.
pub fn platform_setup(
    platforms: &[Platform],
    profiles: &[EmulatorProfile],
    config: &Config,
    host: &str,
    table: &Recommendations,
    packages: &[ServerPackage],
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

    let matched: Vec<MatchedPackage> = packages
        .iter()
        .filter_map(|package| {
            let profile = identify_package(package, profiles, host)?;
            let entry = package
                .installed
                .then(|| server_entry_name(config, profile.name.trim()))
                .flatten();
            Some(MatchedPackage {
                profile,
                package,
                entry,
            })
        })
        .collect();

    let assignable = assignable_server_platforms(platforms);
    platforms
        .iter()
        .filter(|p| assignable.contains(&p.name))
        .map(|p| platform_row(p, &installable, &matched, table))
        .collect()
}

/// The `source_id` a server package's option carries: unique across the
/// screen, and never a catalog `owner/repo`.
pub fn server_source_id(rom_id: i64) -> String {
    format!("romm:{rom_id}")
}

fn platform_row(
    platform: &Platform,
    installable: &[(&EmulatorProfile, &CatalogEntry)],
    server: &[MatchedPackage],
    table: &Recommendations,
) -> PlatformSetup {
    let option = |(profile, row): &(&EmulatorProfile, &CatalogEntry), kind| SetupOption {
        source_id: row.source_id.clone(),
        name: profile.name.trim().to_string(),
        kind,
        installed: row.installed,
        rom_id: None,
        entry: None,
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

    // The server's own packages come after the catalog's downloads (user
    // decision 2026-10-28: the catalog stays the recommendation), and before
    // RetroArch, which only serves platforms nothing standalone does.
    options.extend(
        server
            .iter()
            .filter(|m| platform_matches_keywords(&platform.name, &m.profile.platform_keywords))
            .map(|m| SetupOption {
                source_id: server_source_id(m.package.rom_id),
                name: m.profile.name.trim().to_string(),
                kind: OptionKind::Server,
                installed: m.entry.is_some(),
                rom_id: Some(m.package.rom_id),
                entry: m.entry.clone(),
            }),
    );

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
    /// Catalog `source_id`s to install.
    pub installs: Vec<String>,
    /// Server Emulators-platform packages (ROM ids) to install.
    pub server_installs: Vec<i64>,
    /// Entry names of chosen emulators that are already configured: their
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
        server_installs: Vec::new(),
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
        // An installed server package's entry may carry the " (server)"
        // suffix: its defaults go to that entry, by name.
        let entry_name = option.entry.clone().unwrap_or_else(|| option.name.clone());
        let mut server_rom_id = None;
        if option.installed {
            if !plan.already_installed.contains(&entry_name) {
                plan.already_installed.push(entry_name.clone());
            }
        } else if let Some(rom_id) = option.rom_id {
            // Not installed yet: its entry's name is decided at install time,
            // so the default waits for this ROM, not for a name.
            server_rom_id = Some(rom_id);
            if !plan.server_installs.contains(&rom_id) {
                plan.server_installs.push(rom_id);
            }
        } else if !plan.installs.contains(&option.source_id) {
            plan.installs.push(option.source_id.clone());
        }
        plan.pending.insert(
            row.label.clone(),
            PendingDefault {
                emulator: entry_name,
                name: row.name.clone(),
                slug: row.slug.clone(),
                retroarch: option.kind == OptionKind::Retroarch,
                server_rom_id,
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
/// list. A server package's install passes its `server_rom_id`: the defaults
/// chosen for that ROM apply to the entry whatever name it got; the others
/// wait on a name. Overrides a default the automatic assignment made, never an explicit
/// "(none)". `cores_on_disk` returns the installed cores compatible with a
/// pending platform for a RetroArch entry (production:
/// `autoconfig::installed_compatible_cores`). Pending entries for other
/// emulators, and for an emulator with no configured entry yet, are kept.
pub fn apply_pending_defaults(
    config: &mut Config,
    emulator_name: &str,
    server_rom_id: Option<i64>,
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
        .filter(|(_, p)| match (server_rom_id, p.server_rom_id) {
            (Some(installed), Some(chosen)) => installed == chosen,
            (_, None) => p.emulator.trim().eq_ignore_ascii_case(wanted),
            (None, Some(_)) => false,
        })
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
            &[],
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
            &[],
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
            &[],
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
            server_rom_id: None,
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
        let out = apply_pending_defaults(&mut config, "Xenia Edge (Xbox 360)", None, &cores(&[]));
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
            None,
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
        let out =
            apply_pending_defaults(&mut config, "RetroArch (Multi-System)", None, &cores(&[]));
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
        let out = apply_pending_defaults(&mut config, "PCSX2 (Playstation 2)", None, &cores(&[]));
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
            apply_pending_defaults(&mut config, "Redream (Sega Dreamcast)", None, &cores(&[]))
                .is_empty()
        );
        assert_eq!(config.setup.pending.len(), 2);
        apply_pending_defaults(&mut config, "PCSX2 (Playstation 2)", None, &cores(&[]));
        assert_eq!(
            config.setup.pending.keys().collect::<Vec<_>>(),
            ["Dreamcast"]
        );
    }

    // --- server Emulators-platform packages -----------------------------------

    fn package(rom_id: i64, title: &str, files: &[&str], installed: bool) -> ServerPackage {
        ServerPackage {
            rom_id,
            title: title.to_string(),
            file_names: files.iter().map(|f| f.to_string()).collect(),
            installed,
        }
    }

    fn with_packages(
        platforms: &[Platform],
        packages: &[ServerPackage],
        config: &Config,
    ) -> Vec<PlatformSetup> {
        platform_setup(
            platforms,
            load_profiles(),
            config,
            "linux",
            recommendations(),
            packages,
        )
    }

    /// The real server's Pico-8 package: a folder with a Linux and a Windows zip.
    fn pico8() -> ServerPackage {
        package(
            3432,
            "Pico-8",
            &[
                "pico-8 (linux) (v0.2.7a6).zip",
                "pico-8 (windows) (v0.2.7a6).zip",
            ],
            false,
        )
    }

    #[test]
    fn a_server_package_serves_a_platform_the_catalog_cannot() {
        let row = with_packages(
            &[platform("Pico-8", "pico", 23)],
            &[pico8()],
            &Config::default(),
        )
        .remove(0);
        assert_eq!(row.options[0].kind, OptionKind::Server);
        assert_eq!(row.options[0].name, "Pico-8");
        assert_eq!(row.options[0].rom_id, Some(3432));
        assert_eq!(row.recommended.as_deref(), Some("romm:3432"));
    }

    #[test]
    fn the_catalog_download_stays_recommended_over_a_server_package() {
        let server_pcsx2 = package(7, "PCSX2", &["pcsx2-linux.AppImage"], false);
        let row = with_packages(
            &[platform("Sony PlayStation 2", "ps2", 9)],
            &[server_pcsx2],
            &Config::default(),
        )
        .remove(0);
        let kinds: Vec<OptionKind> = row.options.iter().map(|o| o.kind).collect();
        assert_eq!(kinds[0], OptionKind::Standalone);
        assert_eq!(kinds[1], OptionKind::Server);
        assert_eq!(recommended_name(&row), Some("PCSX2 (Playstation 2)"));
        assert_ne!(row.recommended.as_deref(), Some("romm:7"));
    }

    #[test]
    fn a_package_built_only_for_another_os_is_left_out() {
        let windows_only = package(8, "Pico-8", &["pico-8 (windows).zip", "pico8.exe"], false);
        assert!(identify_package(&windows_only, load_profiles(), "linux").is_none());
        assert!(identify_package(&windows_only, load_profiles(), "win32").is_some());
        // No OS in any name: it may be portable, so it is offered.
        let unmarked = package(9, "Pico-8", &["pico8-0.2.7.zip"], false);
        assert!(identify_package(&unmarked, load_profiles(), "linux").is_some());
    }

    #[test]
    fn an_unrecognised_package_is_left_out() {
        let mystery = package(10, "Some Homebrew Thing", &["thing.zip"], false);
        assert!(identify_package(&mystery, load_profiles(), "linux").is_none());
        // RetroArch on the server is not matched: setup offers it from the
        // catalog, with its core rule.
        let retroarch = package(11, "RetroArch", &["RetroArch-linux.AppImage"], false);
        assert!(identify_package(&retroarch, load_profiles(), "linux").is_none());
    }

    #[test]
    fn an_installed_server_package_points_at_its_suffixed_entry() {
        let mut config = Config::default();
        config.emulators.push(EmulatorEntry {
            name: "Pico-8 (server)".to_string(),
            path: "/lib/games/Emulators/Pico-8/pico8".to_string(),
            ..Default::default()
        });
        let mut pkg = pico8();
        pkg.installed = true;
        let row = with_packages(&[platform("Pico-8", "pico", 23)], &[pkg], &config).remove(0);
        assert!(row.options[0].installed);
        assert_eq!(row.options[0].entry.as_deref(), Some("Pico-8 (server)"));
    }

    #[test]
    fn plan_installs_a_server_package_and_waits_for_that_rom() {
        let rows = with_packages(
            &[platform("Pico-8", "pico", 23)],
            &[pico8()],
            &Config::default(),
        );
        let plan = plan_setup(
            &rows,
            &[SetupChoice {
                platform: "Pico-8".into(),
                source_id: "romm:3432".into(),
            }],
        )
        .unwrap();
        assert_eq!(plan.server_installs, [3432]);
        assert!(plan.installs.is_empty());
        assert_eq!(plan.pending["Pico-8"].server_rom_id, Some(3432));
    }

    #[test]
    fn a_server_packages_default_lands_on_whatever_name_its_entry_got() {
        let mut p = pending("Pico-8", "Pico-8", "pico", false);
        p.server_rom_id = Some(3432);
        let mut config = config_with(&["Pico-8 (server)"], &[("Pico-8", p)]);
        // Another emulator's install, even one named like the profile, does
        // not apply it.
        assert!(
            apply_pending_defaults(&mut config, "Pico-8 (server)", None, &cores(&[])).is_empty()
        );
        let out = apply_pending_defaults(&mut config, "Pico-8 (server)", Some(3432), &cores(&[]));
        assert_eq!(config.default_emulators["Pico-8"], "Pico-8 (server)");
        assert!(matches!(out[0], PendingOutcome::Assigned { .. }));
        assert!(config.setup.pending.is_empty());
    }
}
