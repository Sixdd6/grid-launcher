//! The label-keyed platform maps the install service and grid-core's launch
//! core read (romm-storefront-plan 0.9): label → server id, and label →
//! slug. Moved out of `commands.rs` `list_platforms` (Step 1); Step 7 fills
//! the slugs from GRID's own platform catalog (`platforms.json`), so launch
//! core resolution, firmware routing and Settings › Systems work without
//! RomM.

use crate::commands::AppState;
use grid_core::library::InstallService;
use grid_core::romm::Platform;
use grid_core::systems::{self, SystemPlatform};
use std::collections::BTreeMap;

/// Records the platform maps: the catalog, plus `server` (the RomM platform
/// list) when RomM answered. Called at startup with `None` and after every
/// platform fetch with the server's list.
pub fn record(state: &AppState, server: Option<&[Platform]>) {
    if let Ok(install) = state.install.as_ref() {
        record_into(install, server);
    }
}

fn record_into(install: &InstallService, server: Option<&[Platform]>) {
    let maps = maps_for(server);
    install.set_platform_ids(maps.ids);
    install.set_platform_slugs(maps.install_slugs);
    grid_core::launch::set_platform_slugs(maps.slugs);
    if server.is_none() {
        // Without RomM nothing else names the platforms a fresh emulator
        // install assigns its defaults to (`list_platforms` does it while
        // connected): the catalog's names, which are also their labels.
        install.set_known_platforms(maps.known);
        install.set_platform_labels(BTreeMap::new());
    }
}

/// What [`record`] writes.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct PlatformMaps {
    /// Label → RomM id: server platforms only; empty without RomM.
    pub ids: BTreeMap<String, i64>,
    /// Label and alias → slug, for grid-core's process-wide registry.
    pub slugs: BTreeMap<String, String>,
    /// [`Self::slugs`] plus the server's raw-name fallback, for the install
    /// service.
    pub install_slugs: BTreeMap<String, String>,
    /// The catalog names a fresh emulator install may take defaults for.
    pub known: Vec<String>,
}

/// The maps for the catalog and, when given, the server's list. Every
/// [`SystemPlatform`] label and alias maps to its slug; the server's own
/// label → slug entries win over a catalog alias of the same spelling.
pub(crate) fn maps_for(server: Option<&[Platform]>) -> PlatformMaps {
    let systems = systems::merge(systems::catalog(), server, &[]);
    let mut slugs = slugs_by_name(&systems);
    let mut install_slugs = slugs.clone();
    let mut ids = BTreeMap::new();
    if let Some(platforms) = server {
        // The firmware triggers need the platform *id*, not just the name,
        // and grid-core holds no session to fetch it with. Recorded from the
        // FULL platform list (not the assignable subset): a platform the
        // autoconfig defaults skip can still have server firmware.
        ids = platform_id_entries(platforms);
        // Slug-first RetroArch core resolution (D-RC-2) reads a rom's own
        // display name (`launch::selection::installed_core_resolver`), so
        // the server's label wins; the install service's copy also keeps
        // the raw-name fallback (`platform_slugs_for_install`).
        slugs.extend(platform_slugs_by_label(platforms));
        install_slugs.extend(platform_slugs_for_install(platforms));
    }
    let catalog_names: Vec<String> = systems::catalog().iter().map(|p| p.name.clone()).collect();
    let known = grid_core::autoconfig::entry::assignable_platforms(&catalog_names);
    PlatformMaps {
        ids,
        slugs,
        install_slugs,
        known,
    }
}

/// Each system's label, then its aliases, → its slug. A name two systems
/// share keeps the first system's slug (labels come before aliases).
fn slugs_by_name(systems: &[SystemPlatform]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for p in systems {
        map.entry(p.label.clone()).or_insert_with(|| p.slug.clone());
    }
    for p in systems {
        for alias in &p.aliases {
            map.entry(alias.clone()).or_insert_with(|| p.slug.clone());
        }
    }
    map
}

/// The `(label, id)` pairs `install.set_platform_ids` records. Keyed by
/// `Platform::label()`, not `name`: `firmware_service.rs`'s `platform_id_for`
/// looks the id up with `record.platform`, which is `RomDetail.platform_name`
/// (the rom's own display name) — a raw-`name` key here silently missed
/// every lookup for a custom-named platform (round 9 review finding 2).
pub(crate) fn platform_id_entries(platforms: &[Platform]) -> BTreeMap<String, i64> {
    platforms
        .iter()
        .map(|p| (p.label().to_string(), p.id))
        .collect()
}

/// The `label -> slug` map fed to `grid_core::launch::set_platform_slugs`
/// (the process-wide registry `installed_core_resolver` reads via
/// `slug_for_platform`) and used as the base of
/// [`platform_slugs_for_install`]. Every real reader of that registry —
/// `resolve_launch`'s `game.platform`, `GameFirmwareContext.platform`,
/// `InstallService`'s PS3/Xenia helpers, the cloud-ops wrapper name — reads
/// a rom's own display name (`RomDetail.platform_name`/`InstalledGame.
/// platform`), so the slug has to be keyed the same way, not by the server's
/// raw platform `name` (round 9 re-review HIGH item).
pub(crate) fn platform_slugs_by_label(platforms: &[Platform]) -> BTreeMap<String, String> {
    platforms
        .iter()
        .map(|p| (p.label().to_string(), p.slug.clone()))
        .collect()
}

/// [`platform_slugs_by_label`], plus a `name`-keyed fallback entry for any
/// platform whose custom name makes its label differ from its raw `name`.
/// Fed to `install.set_platform_slugs`, which has two kinds of reader:
/// `set_default_emulator` / `set_retroarch_core`, which key by the label
/// like everything else above — and the autoconfig backfill's
/// `SyncContext.platforms` (`install.known_platforms()` /
/// `autoconfig_entry::assignable_platforms`, both built from the raw server
/// `name`), which still iterates by name. The fallback keeps that second
/// caller resolving a slug too, without requiring it to change.
pub(crate) fn platform_slugs_for_install(platforms: &[Platform]) -> BTreeMap<String, String> {
    let mut map = platform_slugs_by_label(platforms);
    for p in platforms {
        let label = p.label();
        if p.name != label {
            map.entry(p.name.clone()).or_insert_with(|| p.slug.clone());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_none_fills_slugs_from_the_catalog_and_leaves_ids_empty() {
        let maps = maps_for(None);
        assert!(maps.ids.is_empty());
        assert_eq!(
            maps.slugs.get("PlayStation").map(String::as_str),
            Some("psx")
        );
        assert_eq!(
            maps.slugs.get("Sony PlayStation").map(String::as_str),
            Some("psx")
        );
        assert_eq!(
            maps.slugs
                .get("Super Nintendo Entertainment System")
                .map(String::as_str),
            Some("snes")
        );
        assert_eq!(maps.slugs, maps.install_slugs);
        assert!(maps.known.iter().any(|n| n == "PlayStation 2"));
    }

    #[test]
    fn record_some_lets_server_entries_win() {
        let server = vec![
            Platform {
                id: 5,
                name: "PlayStation".to_string(),
                slug: "psx".to_string(),
                rom_count: 2,
                custom_name: Some("PS1 Classics".to_string()),
                display_name: "PS1 Classics".to_string(),
            },
            // A custom label spelled like another platform's catalog name:
            // the server's own entry wins.
            Platform {
                id: 6,
                name: "Nintendo 64".to_string(),
                slug: "n64".to_string(),
                rom_count: 1,
                custom_name: Some("SNES".to_string()),
                display_name: "SNES".to_string(),
            },
        ];
        let maps = maps_for(Some(&server));
        assert_eq!(maps.ids.get("PS1 Classics"), Some(&5));
        assert_eq!(maps.ids.len(), 2);
        assert_eq!(
            maps.slugs.get("PS1 Classics").map(String::as_str),
            Some("psx")
        );
        // The catalog name still resolves (label lookup without a fetch).
        assert_eq!(
            maps.slugs.get("PlayStation").map(String::as_str),
            Some("psx")
        );
        assert_eq!(maps.slugs.get("SNES").map(String::as_str), Some("n64"));
        assert_eq!(
            maps.install_slugs.get("Nintendo 64").map(String::as_str),
            Some("n64")
        );
    }

    #[test]
    fn platform_id_entries_keys_by_the_display_label_not_the_raw_name() {
        let platforms = vec![Platform {
            id: 3,
            name: "Windows".to_string(),
            slug: "win".to_string(),
            rom_count: 1,
            custom_name: Some("Windows 9x".to_string()),
            display_name: "Windows 9x".to_string(),
        }];
        assert_eq!(
            platform_id_entries(&platforms),
            BTreeMap::from([("Windows 9x".to_string(), 3)])
        );
    }

    fn windows_9x_platform() -> Platform {
        Platform {
            id: 3,
            name: "Windows".to_string(),
            slug: "win".to_string(),
            rom_count: 1,
            custom_name: Some("Windows 9x".to_string()),
            display_name: "Windows 9x".to_string(),
        }
    }

    #[test]
    fn platform_slugs_by_label_keys_by_the_display_label_not_the_raw_name() {
        let platforms = vec![windows_9x_platform()];
        assert_eq!(
            platform_slugs_by_label(&platforms),
            BTreeMap::from([("Windows 9x".to_string(), "win".to_string())])
        );
    }

    #[test]
    fn platform_slugs_for_install_keeps_a_name_fallback_when_it_differs_from_the_label() {
        let platforms = vec![windows_9x_platform()];
        // Both the label (what `set_default_emulator`/`set_retroarch_core`
        // pass) and the raw name (what the autoconfig backfill's
        // `SyncContext.platforms` still iterates) resolve to the same slug.
        assert_eq!(
            platform_slugs_for_install(&platforms),
            BTreeMap::from([
                ("Windows".to_string(), "win".to_string()),
                ("Windows 9x".to_string(), "win".to_string()),
            ])
        );
    }

    #[test]
    fn platform_slugs_for_install_has_one_entry_when_the_label_matches_the_name() {
        let platforms = vec![Platform {
            id: 1,
            name: "Super Nintendo Entertainment System".to_string(),
            slug: "snes".to_string(),
            rom_count: 3,
            custom_name: None,
            display_name: "Super Nintendo Entertainment System".to_string(),
        }];
        assert_eq!(
            platform_slugs_for_install(&platforms),
            BTreeMap::from([(
                "Super Nintendo Entertainment System".to_string(),
                "snes".to_string()
            )])
        );
    }
}
