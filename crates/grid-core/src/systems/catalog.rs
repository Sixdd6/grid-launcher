//! GRID's own platform catalog, `platforms.json` at the repo root
//! (romm-storefront-plan 0.9). Generated from RomM 5.4.1's slug and provider
//! lists by `scripts/data/gen_platforms_json.py`, so a platform's catalog
//! `name` is the label a RomM server with no custom names shows for it, and
//! config keyed by label resolves the same with and without RomM.

use std::sync::OnceLock;

use serde::Deserialize;

const PLATFORMS_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../platforms.json"));

/// One catalog platform.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CatalogPlatform {
    /// RomM's slug.
    pub slug: String,
    /// RomM 5.4.1's default name for the slug.
    pub name: String,
    /// Other names the metadata providers give it (old config keys).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Slugs older data used for the same platform (`ps4--1`).
    #[serde(default)]
    pub slug_aliases: Vec<String>,
}

#[derive(Deserialize)]
struct CatalogFile {
    platforms: Vec<CatalogPlatform>,
}

/// Parses a `platforms.json` body. `Err` names what is wrong.
pub fn parse_catalog(json: &str) -> Result<Vec<CatalogPlatform>, String> {
    let file: CatalogFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(file.platforms)
}

/// The bundled catalog, parsed once.
pub fn catalog() -> &'static [CatalogPlatform] {
    static CATALOG: OnceLock<Vec<CatalogPlatform>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_catalog(PLATFORMS_JSON)
            .expect("platforms.json is embedded at build time and must be valid")
    })
}

/// The catalog's own slug for `slug`: itself when the catalog lists it, the
/// platform a slug alias belongs to (`ps4--1` → `ps4`), else `slug` as given
/// (trimmed, lower-cased).
pub fn canonical_slug(slug: &str) -> String {
    let wanted = slug.trim().to_lowercase();
    catalog()
        .iter()
        .find(|p| {
            p.slug_aliases
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&wanted))
        })
        .map(|p| p.slug.clone())
        .unwrap_or(wanted)
}

/// The catalog platform for `slug` (slug aliases included).
pub fn by_slug(slug: &str) -> Option<&'static CatalogPlatform> {
    let wanted = canonical_slug(slug);
    catalog().iter().find(|p| p.slug == wanted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_catalog_parses_and_its_slugs_and_names_are_unique() {
        let all = catalog();
        assert!(all.len() > 50);
        let slugs: BTreeSet<&str> = all.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs.len(), all.len());
        let names: BTreeSet<String> = all.iter().map(|p| p.name.to_lowercase()).collect();
        assert_eq!(names.len(), all.len());
    }

    #[test]
    fn the_catalog_covers_every_slug_of_the_two_slug_files() {
        let recommended: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../platform-emulators.json"
        )))
        .unwrap();
        let cores: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../romm-platform-cores.json"
        )))
        .unwrap();
        let mut wanted: Vec<String> = recommended["recommended"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        wanted.extend(cores.as_object().unwrap().keys().cloned());
        for slug in wanted {
            assert!(
                by_slug(&slug).is_some(),
                "platforms.json has no entry for {slug}"
            );
            assert_eq!(
                canonical_slug(&slug),
                slug,
                "{slug} is only a slug alias; use its catalog slug"
            );
        }
    }

    #[test]
    fn every_profile_with_keywords_matches_a_catalog_name() {
        use crate::launch::profiles::{load_profiles, platform_matches_keywords};
        // FBNeo's keywords name the emulator, not a platform: RomM has no
        // "FinalBurn" platform, so nothing in the catalog can match them.
        const NO_PLATFORM: &[&str] = &["FBNeo (Arcade)"];
        for profile in load_profiles() {
            if profile.platform_keywords.is_empty() || NO_PLATFORM.contains(&profile.name.trim()) {
                continue;
            }
            let hit = catalog().iter().any(|p| {
                std::iter::once(&p.name)
                    .chain(p.aliases.iter())
                    .any(|n| platform_matches_keywords(n, &profile.platform_keywords))
            });
            assert!(hit, "{} matches no catalog platform", profile.name);
        }
    }

    #[test]
    fn pico_is_pico_8_and_the_sega_pico_has_its_own_slug() {
        assert_eq!(by_slug("pico").unwrap().name, "Pico-8");
        assert_eq!(by_slug("sega-pico").unwrap().name, "Sega Pico");
        assert_eq!(by_slug("model3").unwrap().name, "Sega Model 3");
    }

    #[test]
    fn slug_aliases_resolve_to_their_catalog_slug() {
        assert_eq!(canonical_slug("ps4--1"), "ps4");
        assert_eq!(canonical_slug("SG-1000"), "sg1000");
        assert_eq!(canonical_slug("x68000"), "sharp-x68000");
        assert_eq!(canonical_slug("snes"), "snes");
        assert_eq!(canonical_slug("not-a-slug"), "not-a-slug");
    }
}
