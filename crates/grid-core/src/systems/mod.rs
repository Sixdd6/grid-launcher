//! Systems without RomM (romm-storefront-plan 0.9): GRID's own platform
//! catalog, the merge of that catalog with RomM's platforms and the
//! installed games, and the label lookup for label-keyed config.
//!
//! Nothing here assumes a server id: a later "local ROMs" source adds
//! installed rows, which join a [`SystemPlatform`] by slug like installed
//! RomM rows do.

pub mod catalog;
pub mod merge;

use std::collections::BTreeMap;

pub use catalog::{by_slug, canonical_slug, catalog, CatalogPlatform};
pub use merge::merge;

/// Where a system row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformOrigin {
    Catalog,
    Server,
    Both,
    Installed,
}

/// One system.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SystemPlatform {
    pub slug: String,
    /// The server's label when connected and known, else the catalog name,
    /// else the installed row's `platform`. The key for `default_emulators`
    /// and friends.
    pub label: String,
    /// The name emulator profiles' keywords are matched against: the catalog
    /// name, else the server's raw `name`, else the label.
    pub name: String,
    /// Every other name this slug has gone by (catalog name and aliases,
    /// server name and display name): the lookup set for label-keyed config.
    pub aliases: Vec<String>,
    pub origin: PlatformOrigin,
    /// RomM's id; `None` without RomM. Only Install from RomM reads it.
    pub server_id: Option<i64>,
    /// RomM's ROM count; `None` without RomM.
    pub server_rom_count: Option<i64>,
    /// Installed games on this platform (any source with this slug).
    pub installed_count: u32,
}

impl SystemPlatform {
    /// The label, then every alias: the order label-keyed config is read in.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.label.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

/// A label-keyed config value for `p` (`default_emulators`,
/// `retroarch_cores`, `setup.pending`): the current label first, then each
/// alias of the same slug, each exact and then ignoring case. Writers use
/// the current label; nothing is migrated.
pub fn lookup<'a, V>(map: &'a BTreeMap<String, V>, p: &SystemPlatform) -> Option<&'a V> {
    for name in p.names() {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(v) = map.get(name) {
            return Some(v);
        }
        if let Some((_, v)) = map
            .iter()
            .find(|(k, _)| k.trim().eq_ignore_ascii_case(name))
        {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psx(label: &str, aliases: &[&str]) -> SystemPlatform {
        SystemPlatform {
            slug: "psx".into(),
            label: label.into(),
            name: "PlayStation".into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            origin: PlatformOrigin::Both,
            server_id: Some(1),
            server_rom_count: Some(3),
            installed_count: 0,
        }
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn lookup_reads_the_label_first() {
        let m = map(&[("PlayStation", "Old"), ("PS1 Classics", "DuckStation")]);
        assert_eq!(
            lookup(&m, &psx("PS1 Classics", &["PlayStation"])).map(String::as_str),
            Some("DuckStation")
        );
    }

    #[test]
    fn lookup_falls_back_to_an_alias() {
        let m = map(&[("Sony PlayStation", "DuckStation")]);
        assert_eq!(
            lookup(&m, &psx("PlayStation", &["Sony PlayStation"])).map(String::as_str),
            Some("DuckStation")
        );
    }

    #[test]
    fn a_custom_server_name_still_finds_the_value_under_the_catalog_name() {
        let m = map(&[("playstation", "DuckStation")]);
        assert_eq!(
            lookup(&m, &psx("PS1 Classics", &["PlayStation"])).map(String::as_str),
            Some("DuckStation")
        );
        assert_eq!(lookup(&m, &psx("PS1 Classics", &[])), None);
    }
}
