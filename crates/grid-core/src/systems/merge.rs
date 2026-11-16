//! [`merge`]: one row per system from the catalog, the RomM server's
//! platform list (while connected) and the installed games.

use std::collections::BTreeMap;

use super::catalog::{canonical_slug, CatalogPlatform};
use super::{PlatformOrigin, SystemPlatform};
use crate::cloud::scope::is_emulators_platform;
use crate::romm::Platform;

/// The name token an installed row with no slug is keyed by: lower case,
/// every run of anything but letters and digits as one dash (the
/// frontend's `platformSlug`).
pub fn name_token(label: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in label.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    out
}

fn push_alias(row: &mut SystemPlatform, name: &str) {
    let name = name.trim();
    if name.is_empty()
        || name.eq_ignore_ascii_case(&row.label)
        || row.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    {
        return;
    }
    row.aliases.push(name.to_string());
}

/// Joins the three sources by slug (slug aliases folded to the catalog's
/// slug):
/// - every catalog platform (`Catalog`);
/// - a server platform the catalog knows takes over its label (`Both`), one
///   it does not know is added (`Server`); the Emulators shelf is dropped;
/// - an installed row counts toward its slug (`platform_slug`, else the
///   platform whose label or alias equals the row's `platform`, else the
///   name token); one nothing knows is added (`Installed`).
///
/// `installed` is one `(platform label, platform slug)` pair per installed
/// game. Sorted by label, ignoring case.
pub fn merge(
    catalog: &[CatalogPlatform],
    server: Option<&[Platform]>,
    installed: &[(String, String)],
) -> Vec<SystemPlatform> {
    let mut rows: BTreeMap<String, SystemPlatform> = BTreeMap::new();
    for p in catalog {
        let mut row = SystemPlatform {
            slug: p.slug.clone(),
            label: p.name.clone(),
            name: p.name.clone(),
            aliases: Vec::new(),
            origin: PlatformOrigin::Catalog,
            server_id: None,
            server_rom_count: None,
            installed_count: 0,
        };
        for alias in &p.aliases {
            push_alias(&mut row, alias);
        }
        rows.insert(p.slug.clone(), row);
    }

    for p in server.unwrap_or_default() {
        if is_emulators_platform(p.label()) || is_emulators_platform(&p.name) {
            continue;
        }
        let slug = canonical_slug(&p.slug);
        if slug.is_empty() {
            continue;
        }
        let label = p.label().to_string();
        let row = rows.entry(slug.clone()).or_insert_with(|| SystemPlatform {
            slug: slug.clone(),
            label: label.clone(),
            name: p.name.clone(),
            aliases: Vec::new(),
            origin: PlatformOrigin::Server,
            server_id: None,
            server_rom_count: None,
            installed_count: 0,
        });
        if row.origin == PlatformOrigin::Catalog {
            row.origin = PlatformOrigin::Both;
            // The catalog name stays an alias: config written without RomM
            // keys by it.
            let catalog_name = std::mem::replace(&mut row.label, label);
            push_alias(row, &catalog_name);
            let aliases = std::mem::take(&mut row.aliases);
            for alias in aliases {
                push_alias(row, &alias);
            }
        }
        push_alias(row, &p.name);
        push_alias(row, &p.display_name);
        if let Some(custom) = &p.custom_name {
            push_alias(row, custom);
        }
        row.server_id = Some(p.id);
        row.server_rom_count = Some(p.rom_count);
    }

    for (label, slug) in installed {
        if is_emulators_platform(label) {
            continue;
        }
        let key = if !slug.trim().is_empty() {
            canonical_slug(slug)
        } else if let Some(found) = rows.values().find(|r| {
            r.label.eq_ignore_ascii_case(label.trim())
                || r.aliases
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(label.trim()))
        }) {
            found.slug.clone()
        } else {
            name_token(label)
        };
        if key.is_empty() {
            continue;
        }
        let row = rows.entry(key.clone()).or_insert_with(|| SystemPlatform {
            slug: key.clone(),
            label: label.trim().to_string(),
            name: label.trim().to_string(),
            aliases: Vec::new(),
            origin: PlatformOrigin::Installed,
            server_id: None,
            server_rom_count: None,
            installed_count: 0,
        });
        row.installed_count += 1;
        push_alias(row, label);
    }

    let mut out: Vec<SystemPlatform> = rows.into_values().collect();
    out.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then_with(|| a.label.cmp(&b.label))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(slug: &str, name: &str, aliases: &[&str]) -> CatalogPlatform {
        CatalogPlatform {
            slug: slug.into(),
            name: name.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            slug_aliases: Vec::new(),
        }
    }

    fn server(id: i64, slug: &str, name: &str, custom: Option<&str>, roms: i64) -> Platform {
        Platform {
            id,
            name: name.into(),
            slug: slug.into(),
            rom_count: roms,
            custom_name: custom.map(str::to_string),
            display_name: custom.unwrap_or(name).into(),
        }
    }

    fn catalog() -> Vec<CatalogPlatform> {
        vec![
            cat("psx", "PlayStation", &["Sony PlayStation"]),
            cat("snes", "Super Nintendo Entertainment System", &["SNES"]),
        ]
    }

    fn find<'a>(rows: &'a [SystemPlatform], slug: &str) -> &'a SystemPlatform {
        rows.iter().find(|r| r.slug == slug).unwrap()
    }

    #[test]
    fn catalog_only_lists_every_catalog_platform_by_label() {
        let rows = merge(&catalog(), None, &[]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "PlayStation");
        assert_eq!(rows[0].origin, PlatformOrigin::Catalog);
        assert_eq!(rows[0].aliases, vec!["Sony PlayStation"]);
        assert_eq!(rows[0].server_id, None);
        assert_eq!(rows[0].server_rom_count, None);
    }

    #[test]
    fn server_only_adds_platforms_without_a_catalog() {
        let rows = merge(
            &[],
            Some(&[server(7, "jaguar", "Atari Jaguar", None, 4)]),
            &[],
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].origin, PlatformOrigin::Server);
        assert_eq!(rows[0].label, "Atari Jaguar");
        assert_eq!(rows[0].server_id, Some(7));
        assert_eq!(rows[0].server_rom_count, Some(4));
    }

    #[test]
    fn both_lets_the_server_label_win_and_keeps_the_catalog_name_as_an_alias() {
        let rows = merge(
            &catalog(),
            Some(&[server(3, "psx", "PlayStation", Some("PS1 Classics"), 48)]),
            &[],
        );
        let psx = find(&rows, "psx");
        assert_eq!(psx.origin, PlatformOrigin::Both);
        assert_eq!(psx.label, "PS1 Classics");
        assert_eq!(psx.name, "PlayStation");
        assert!(psx.aliases.iter().any(|a| a == "PlayStation"));
        assert!(psx.aliases.iter().any(|a| a == "Sony PlayStation"));
        assert_eq!(psx.server_id, Some(3));
        assert_eq!(psx.server_rom_count, Some(48));
        assert_eq!(find(&rows, "snes").origin, PlatformOrigin::Catalog);
    }

    #[test]
    fn a_slug_alias_from_the_server_joins_the_catalog_slug() {
        let ps4 = vec![CatalogPlatform {
            slug_aliases: vec!["ps4--1".into()],
            ..cat("ps4", "PlayStation 4", &[])
        }];
        // canonical_slug reads the bundled catalog, which lists ps4--1.
        let rows = merge(
            &ps4,
            Some(&[server(9, "ps4--1", "PlayStation 4", None, 2)]),
            &[],
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].origin, PlatformOrigin::Both);
    }

    #[test]
    fn the_emulators_shelf_is_dropped() {
        let rows = merge(
            &catalog(),
            Some(&[server(1, "emulators", "Emulators", None, 3)]),
            &[("Emulators".into(), "".into())],
        );
        assert!(rows.iter().all(|r| r.slug != "emulators"));
    }

    #[test]
    fn installed_rows_count_by_slug_then_label_then_name_token() {
        let rows = merge(
            &catalog(),
            None,
            &[
                ("PlayStation".into(), "psx".into()),
                ("SNES".into(), "".into()),
                ("Homebrew Box".into(), "".into()),
                ("Homebrew Box".into(), "".into()),
            ],
        );
        assert_eq!(find(&rows, "psx").installed_count, 1);
        assert_eq!(find(&rows, "snes").installed_count, 1);
        let other = find(&rows, "homebrew-box");
        assert_eq!(other.origin, PlatformOrigin::Installed);
        assert_eq!(other.label, "Homebrew Box");
        assert_eq!(other.installed_count, 2);
    }

    #[test]
    fn sorted_by_label_ignoring_case() {
        let rows = merge(
            &catalog(),
            Some(&[server(2, "amiga", "amiga", None, 1)]),
            &[],
        );
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "amiga",
                "PlayStation",
                "Super Nintendo Entertainment System"
            ]
        );
    }

    #[test]
    fn name_token_matches_the_frontend() {
        assert_eq!(
            name_token("  Sega Mega Drive/Genesis "),
            "sega-mega-drive-genesis"
        );
        assert_eq!(name_token("--"), "");
    }
}
