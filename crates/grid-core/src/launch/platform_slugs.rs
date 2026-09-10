//! Process-wide platform name -> server slug registry.
//!
//! grid-core holds no session, so it cannot fetch the server's platform
//! list itself. Slug-first RetroArch core resolution (design D-RC-2) still
//! has to work on every grid-core path that sees only a platform NAME: the
//! launch resolver, cloud ops, firmware routing, and the install service.
//! Rather than threading a slug map through six signatures, the app layer
//! fills this registry once per `list_platforms` response and every reader
//! picks the slug up from here.
//!
//! Before the first successful platform fetch the registry is empty, so
//! [`slug_for_platform`] answers `""` and `installed_compatible_cores`
//! takes its fuzzy name fallback.

use std::collections::BTreeMap;
use std::sync::RwLock;

static SLUGS: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

/// Replaces the whole registry with the app's latest platform list.
pub fn set_platform_slugs(slugs: BTreeMap<String, String>) {
    *SLUGS.write().unwrap() = slugs;
}

/// The recorded server slug for `name`: the exact key first, then a
/// case-insensitive key scan. `""` when the platform is unknown, which is
/// also the answer before the first platform fetch.
///
/// The lock is released before the value is returned; no caller runs while
/// it is held.
pub fn slug_for_platform(name: &str) -> String {
    let target = name.trim();
    if target.is_empty() {
        return String::new();
    }

    let slugs = SLUGS.read().unwrap();
    if let Some(slug) = slugs.get(target) {
        return slug.clone();
    }
    let folded = target.to_lowercase();
    for (key, slug) in slugs.iter() {
        if key.trim().to_lowercase() == folded {
            return slug.clone();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// One test for the whole registry: it is process-wide, so separate
    /// `#[test]` functions would race each other inside the same test
    /// binary. The platform names are deliberately fictional so that a
    /// unit test running in parallel never sees a slug it did not set.
    #[test]
    fn registry_lookup_is_exact_then_folded_and_blank_when_unknown() {
        set_platform_slugs(map(&[
            ("Registry Test Console", "rtc"),
            ("Registry Test Handheld", "rth"),
        ]));

        assert_eq!(slug_for_platform("Registry Test Console"), "rtc");
        assert_eq!(slug_for_platform("registry test handheld"), "rth");
        assert_eq!(slug_for_platform("  Registry Test Handheld  "), "rth");
        assert_eq!(slug_for_platform("Registry Test Unknown"), "");
        assert_eq!(slug_for_platform("   "), "");

        // A later fetch REPLACES the map rather than merging into it.
        set_platform_slugs(map(&[("Registry Test Console", "rtc2")]));
        assert_eq!(slug_for_platform("Registry Test Console"), "rtc2");
        assert_eq!(slug_for_platform("Registry Test Handheld"), "");

        set_platform_slugs(BTreeMap::new());
        assert_eq!(slug_for_platform("Registry Test Console"), "");
    }
}
