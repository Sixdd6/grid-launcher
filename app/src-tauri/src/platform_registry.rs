//! The label-keyed platform maps the install service and grid-core's launch
//! core read (romm-storefront-plan 0.9): label → server id, and label →
//! slug. Moved out of `commands.rs` `list_platforms` with no behaviour
//! change. romm-storefront-plan Step 7 owns this file and fills the slugs
//! from GRID's own platform catalog, so they work without RomM.

use crate::commands::AppState;
use grid_core::library::InstallService;
use grid_core::romm::Platform;
use std::collections::BTreeMap;

/// Records the platform maps from `server` (the RomM platform list, or
/// `None` without RomM; with `None` nothing changes yet — Step 7 makes it
/// fill the slugs from the catalog).
pub fn record(state: &AppState, server: Option<&[Platform]>) {
    if let Ok(install) = state.install.as_ref() {
        record_into(install, server);
    }
}

fn record_into(install: &InstallService, server: Option<&[Platform]>) {
    let Some(platforms) = server else {
        return;
    };
    // The firmware triggers need the platform *id*, not just the name,
    // and grid-core holds no session to fetch it with. Recorded from the
    // FULL platform list (not the assignable subset): a platform the
    // autoconfig defaults skip can still have server firmware.
    install.set_platform_ids(platform_id_entries(platforms));
    // Slug-first RetroArch core resolution (D-RC-2) needs the server's
    // own slug for each platform; like the ids above, this is recorded
    // from the FULL list, not the assignable subset. `install`'s copy
    // keeps a name-keyed fallback (see `platform_slugs_for_install`) for
    // the autoconfig backfill in `list_platforms`, which still iterates
    // names.
    install.set_platform_slugs(platform_slugs_for_install(platforms));
    // The label-keyed map again, into grid-core's process-wide registry:
    // the launch resolver, cloud ops, firmware routing and the install
    // service's PS3/Xenia helpers all read a rom's own display name
    // (`launch::selection::installed_core_resolver`), never the server's
    // raw platform `name` — so no name fallback belongs here. Without
    // this those paths would fall back to fuzzy name matching and
    // disagree with Settings › Systems about which platforms RetroArch
    // supports.
    grid_core::launch::set_platform_slugs(platform_slugs_by_label(platforms));
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
