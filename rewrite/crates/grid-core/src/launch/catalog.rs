//! The "install from catalog" listing: turns the embedded autoprofile
//! catalog's raw `source` blocks into rows a UI can show, independent of
//! whether an emulator with that name/source is already configured. Ports
//! `source_download_emulator_entries` (`grid_launcher/ui/emulators.py:168-231`)
//! restricted to non-compat-tool profiles (spec deviation 2 — the reference
//! showed compat tools in a separate dialog). See
//! `docs/superpowers/specs/2026-09-01-emulator-acquisition-design.md`.
//!
//! This module deliberately keeps its own provider alias map and
//! `release_tag`/`tag`/`version` fallback order, distinct from
//! [`super::source::normalize_source`]'s — the two are read at different
//! times for different purposes (listing vs. resolving a download) and the
//! Python reference itself has two separate implementations
//! (`_normalized_source_provider`, ui/emulators.py:155, vs.
//! `normalize_emulator_source_metadata`, source.py:59) with different
//! fallback rules. Do not merge them.

use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::config::Config;

use super::profiles::EmulatorProfile;
use super::source::HOST_PLATFORM;

/// One row of the "install from catalog" listing.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CatalogEntry {
    pub name: String,
    pub source_id: String,
    pub provider: String,
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub installed: bool,
}

/// Provider aliases this listing folds together — ONLY the GitHub family
/// (`_normalized_source_provider`'s own table, ui/emulators.py:159). This is
/// deliberately its own, smaller map: unlike [`super::source::normalize_source`],
/// `gitea`/`direct` spellings are left exactly as authored — an
/// unrecognized provider string is shown as-is here and only errors later,
/// at resolve time.
fn catalog_provider_alias(provider: &str) -> &str {
    match provider {
        "github-release" | "github_release" | "githubrelease" => "github",
        other => other,
    }
}

/// `source["provider"]`, trimmed, casefolded, then run through
/// [`catalog_provider_alias`] (`_normalized_source_provider`,
/// ui/emulators.py:155-165) — `""` when the key is missing or not a usable
/// string. Deliberately reads ONLY `provider`: the reference this function
/// ports has no `type` fallback (that belongs to
/// `source::normalize_source`'s separate provider read, which this module
/// does not share — see the module doc above).
fn catalog_provider(source: &Map<String, Value>) -> String {
    let provider = match source.get("provider") {
        Some(Value::String(s)) => s.trim().to_lowercase(),
        _ => String::new(),
    };
    if provider.is_empty() {
        return provider;
    }
    catalog_provider_alias(&provider).to_string()
}

/// `source["repo"]`, falling back to `source["repository"]` only when
/// `repo` itself is absent, trimmed. A non-string value reads as blank.
fn catalog_repo(source: &Map<String, Value>) -> String {
    let raw = source.get("repo").or_else(|| source.get("repository"));
    match raw {
        Some(Value::String(s)) => s.trim().to_string(),
        _ => String::new(),
    }
}

/// This listing's own tag chain, `release_tag` first (ui/emulators.py:206 —
/// deliberately a different order from `source::normalize_source`'s
/// `tag`-first chain; see the module doc above).
fn catalog_tag(source: &Map<String, Value>) -> String {
    for key in ["release_tag", "tag", "version"] {
        if let Some(Value::String(s)) = source.get(key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "latest".to_string()
}

/// Whether `source`'s raw `platforms` list (when present and non-empty)
/// allows this host: some entry must be a prefix of [`HOST_PLATFORM`]. A
/// missing, empty, or non-array `platforms` never gates
/// (ui/emulators.py:189-192).
fn platforms_allow_host(source: &Map<String, Value>) -> bool {
    match source.get("platforms") {
        Some(Value::Array(items)) if !items.is_empty() => items.iter().any(|item| {
            let text = match item {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            HOST_PLATFORM.starts_with(text.as_str())
        }),
        _ => true,
    }
}

/// One profile's catalog row (`installed: false`), or `None` when it is
/// skipped: blank name, no object `source`, platform-gated out, an unusable
/// provider, or a missing `owner`/`repo`. The shared row builder for
/// [`catalog_entries`] and [`find_profile`].
fn catalog_row(profile: &EmulatorProfile) -> Option<CatalogEntry> {
    let name = profile.name.trim();
    if name.is_empty() {
        return None;
    }
    let source = profile.source.as_ref()?.as_object()?;

    if !platforms_allow_host(source) {
        return None;
    }

    let provider = catalog_provider(source);
    if provider.is_empty() {
        return None;
    }

    let owner = super::source::str_field(source, "owner");
    let repo = catalog_repo(source);
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    let tag = catalog_tag(source);

    Some(CatalogEntry {
        name: name.to_string(),
        source_id: format!("{owner}/{repo}"),
        provider,
        owner,
        repo,
        tag,
        installed: false,
    })
}

/// `profiles` turned into catalog rows, deduped and sorted
/// (`source_download_emulator_entries`, ui/emulators.py:168-231), restricted
/// to non-compat-tool profiles (spec deviation 2 — compat tools get their
/// own dialog). `installed` is always `false` here — call
/// [`mark_installed`] to fill it in.
pub fn catalog_entries(profiles: &[EmulatorProfile]) -> Vec<CatalogEntry> {
    let mut seen: HashSet<(String, String, String, String)> = HashSet::new();
    let mut rows: Vec<CatalogEntry> = Vec::new();

    for profile in profiles {
        if profile.is_compat_tool {
            continue;
        }
        let Some(row) = catalog_row(profile) else {
            continue;
        };
        let key = (
            row.name.to_lowercase(),
            row.provider.to_lowercase(),
            row.owner.to_lowercase(),
            row.repo.to_lowercase(),
        );
        if !seen.insert(key) {
            continue;
        }
        rows.push(row);
    }

    rows.sort_by(|a, b| {
        (a.name.to_lowercase(), a.source_id.to_lowercase())
            .cmp(&(b.name.to_lowercase(), b.source_id.to_lowercase()))
    });
    rows
}

/// Marks each entry installed when its `name` casefold-matches any
/// `config.emulators` entry's name, or its `source_id` casefold-matches any
/// config emulator's `source_id`. A blank config `source_id` never
/// matches — every catalog `source_id` is non-blank by construction
/// ([`catalog_row`] requires `owner` and `repo`), so this only guards
/// against a config emulator whose `source_id` is still unset.
pub fn mark_installed(entries: &mut [CatalogEntry], config: &Config) {
    for entry in entries {
        entry.installed = config.emulators.iter().any(|emulator| {
            emulator.name.to_lowercase() == entry.name.to_lowercase()
                || (!emulator.source_id.is_empty()
                    && emulator.source_id.to_lowercase() == entry.source_id.to_lowercase())
        });
    }
}

/// The first non-compat-tool profile whose source normalizes far enough to
/// have an `owner/repo` matching `source_id`, casefolded — the same raw
/// field reads as [`catalog_entries`] ([`catalog_row`]).
pub fn find_profile<'a>(
    profiles: &'a [EmulatorProfile],
    source_id: &str,
) -> Option<&'a EmulatorProfile> {
    let target = source_id.to_lowercase();
    profiles.iter().find(|profile| {
        if profile.is_compat_tool {
            return false;
        }
        match catalog_row(profile) {
            Some(row) => row.source_id.to_lowercase() == target,
            None => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EmulatorEntry;
    use crate::launch::profiles::load_profiles;
    use serde_json::json;

    fn profile(name: &str, compat: bool, source: Option<Value>) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            match_tokens: vec![],
            args: "%rom%".to_string(),
            all_platforms: false,
            platform_keywords: vec![],
            is_compat_tool: compat,
            source,
        }
    }

    fn emu(name: &str, source_id: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: String::new(),
            args: String::new(),
            source_id: source_id.to_string(),
            ..Default::default()
        }
    }

    fn config_with(emulators: Vec<EmulatorEntry>) -> Config {
        Config {
            emulators,
            ..Config::default()
        }
    }

    // --- catalog_entries: real embedded catalog, linux -----------------------

    #[test]
    fn real_catalog_excludes_compat_tools() {
        let entries = catalog_entries(load_profiles());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"GE-Proton"));
        assert!(!names.contains(&"Proton-CachyOS"));
    }

    #[test]
    fn real_catalog_excludes_win32_only_platform_gated_profiles() {
        let entries = catalog_entries(load_profiles());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"ShadPS4 Qt Launcher"));
        assert!(!names.contains(&"Xenia Canary (Xbox 360)"));
    }

    #[test]
    fn real_catalog_includes_expected_rows_with_expected_fields() {
        let entries = catalog_entries(load_profiles());
        let by_name = |n: &str| entries.iter().find(|e| e.name == n).unwrap();

        let retroarch = by_name("RetroArch (Multi-System)");
        assert_eq!(retroarch.provider, "direct");
        assert_eq!(retroarch.source_id, "libretro/retroarch-nightly");
        assert!(!retroarch.installed);

        let eden = by_name("Eden (Nintendo Switch)");
        assert_eq!(eden.provider, "gitea");

        let pcsx2 = by_name("PCSX2 (Playstation 2)");
        assert_eq!(pcsx2.provider, "github");
        assert_eq!(pcsx2.source_id, "PCSX2/pcsx2");
    }

    #[test]
    fn real_catalog_every_source_id_is_owner_slash_repo() {
        let entries = catalog_entries(load_profiles());
        assert!(!entries.is_empty());
        for entry in &entries {
            assert_eq!(entry.source_id, format!("{}/{}", entry.owner, entry.repo));
        }
    }

    #[test]
    fn real_catalog_is_sorted_by_casefolded_name_then_source_id() {
        let entries = catalog_entries(load_profiles());
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| {
            (a.name.to_lowercase(), a.source_id.to_lowercase())
                .cmp(&(b.name.to_lowercase(), b.source_id.to_lowercase()))
        });
        assert_eq!(entries, sorted);
    }

    // --- catalog_entries: synthetic tables ------------------------------------

    #[test]
    fn dedupe_casefolded_keeps_first() {
        let profiles = vec![
            profile(
                "Foo",
                false,
                Some(
                    json!({"provider": "github", "owner": "Acme", "repo": "Foo", "release_tag": "v1"}),
                ),
            ),
            profile(
                "foo",
                false,
                Some(
                    json!({"provider": "GITHUB", "owner": "acme", "repo": "foo", "release_tag": "v2"}),
                ),
            ),
        ];
        let entries = catalog_entries(&profiles);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tag, "v1");
    }

    #[test]
    fn tag_chain_prefers_release_tag_over_tag_over_version() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(
                json!({"provider": "github", "owner": "o", "repo": "r", "release_tag": "a", "tag": "b", "version": "c"}),
            ),
        )];
        assert_eq!(catalog_entries(&profiles)[0].tag, "a");
    }

    #[test]
    fn tag_chain_falls_back_to_tag_then_version_then_latest() {
        let by_tag = vec![profile(
            "Foo",
            false,
            Some(
                json!({"provider": "github", "owner": "o", "repo": "r", "tag": "b", "version": "c"}),
            ),
        )];
        assert_eq!(catalog_entries(&by_tag)[0].tag, "b");

        let by_version = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o", "repo": "r", "version": "c"})),
        )];
        assert_eq!(catalog_entries(&by_version)[0].tag, "c");

        let none = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o", "repo": "r"})),
        )];
        assert_eq!(catalog_entries(&none)[0].tag, "latest");
    }

    #[test]
    fn unknown_provider_passes_through_unchanged() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "custom-thing", "owner": "o", "repo": "r"})),
        )];
        assert_eq!(catalog_entries(&profiles)[0].provider, "custom-thing");
    }

    #[test]
    fn gitea_provider_is_not_aliased() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "gitea", "owner": "o", "repo": "r"})),
        )];
        assert_eq!(catalog_entries(&profiles)[0].provider, "gitea");
    }

    #[test]
    fn github_family_aliases_fold_to_github() {
        for alias in ["github-release", "github_release", "githubrelease"] {
            let profiles = vec![profile(
                "Foo",
                false,
                Some(json!({"provider": alias, "owner": "o", "repo": "r"})),
            )];
            assert_eq!(catalog_entries(&profiles)[0].provider, "github", "{alias}");
        }
    }

    #[test]
    fn provider_key_only_at_type_skips_row_no_fallback() {
        // ui/emulators.py:194 reads only `source_value.get("provider", "")`
        // — no `type` fallback (that belongs solely to
        // `source::normalize_source`'s separate, unrelated provider read).
        // An entry with only `type` set therefore resolves to an empty
        // provider and is skipped, exactly like a `provider`-less,
        // `type`-less entry (ui/emulators.py:194-196 `if not provider:
        // continue`).
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"type": "github-release", "owner": "o", "repo": "r"})),
        )];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn missing_repo_skips_row_even_with_repository_absent_too() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o"})),
        )];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn repo_falls_back_to_repository_key() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o", "repository": "r"})),
        )];
        let entries = catalog_entries(&profiles);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].repo, "r");
        assert_eq!(entries[0].source_id, "o/r");
    }

    #[test]
    fn missing_owner_skips_row() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "repo": "r"})),
        )];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn missing_provider_and_type_skips_row() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"owner": "o", "repo": "r"})),
        )];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn compat_tool_is_skipped_even_with_a_valid_source() {
        let profiles = vec![profile(
            "Foo",
            true,
            Some(json!({"provider": "github", "owner": "o", "repo": "r"})),
        )];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn non_object_source_is_skipped() {
        let profiles = vec![profile("Foo", false, Some(json!("not an object")))];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn absent_source_is_skipped() {
        let profiles = vec![profile("Foo", false, None)];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn platforms_gate_hides_entry_with_no_matching_prefix() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o", "repo": "r", "platforms": ["win32"]})),
        )];
        assert!(catalog_entries(&profiles).is_empty());
    }

    #[test]
    fn platforms_gate_keeps_entry_with_a_matching_prefix() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o", "repo": "r", "platforms": ["lin"]})),
        )];
        assert_eq!(catalog_entries(&profiles).len(), 1);
    }

    #[test]
    fn platforms_gate_is_a_noop_when_the_list_is_empty() {
        let profiles = vec![profile(
            "Foo",
            false,
            Some(json!({"provider": "github", "owner": "o", "repo": "r", "platforms": []})),
        )];
        assert_eq!(catalog_entries(&profiles).len(), 1);
    }

    // --- mark_installed --------------------------------------------------------

    fn stub_entry(name: &str, source_id: &str) -> CatalogEntry {
        CatalogEntry {
            name: name.to_string(),
            source_id: source_id.to_string(),
            provider: "github".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            tag: "latest".to_string(),
            installed: false,
        }
    }

    #[test]
    fn mark_installed_matches_by_name_casefold() {
        let mut entries = vec![stub_entry("PCSX2 (Playstation 2)", "PCSX2/pcsx2")];
        let config = config_with(vec![emu("pcsx2 (playstation 2)", "")]);
        mark_installed(&mut entries, &config);
        assert!(entries[0].installed);
    }

    #[test]
    fn mark_installed_matches_by_source_id_casefold() {
        let mut entries = vec![stub_entry("Some Other Name", "PCSX2/pcsx2")];
        let config = config_with(vec![emu("Whatever", "pcsx2/PCSX2")]);
        mark_installed(&mut entries, &config);
        assert!(entries[0].installed);
    }

    #[test]
    fn mark_installed_blank_config_source_id_never_matches() {
        let mut entries = vec![stub_entry("Foo", "")];
        let config = config_with(vec![emu("Unrelated", "")]);
        mark_installed(&mut entries, &config);
        assert!(!entries[0].installed);
    }

    #[test]
    fn mark_installed_leaves_unmatched_entries_uninstalled() {
        let mut entries = vec![stub_entry("Foo", "o/r")];
        let config = config_with(vec![emu("Bar", "x/y")]);
        mark_installed(&mut entries, &config);
        assert!(!entries[0].installed);
    }

    // --- find_profile ------------------------------------------------------------

    #[test]
    fn find_profile_hits_by_casefolded_source_id() {
        let found = find_profile(load_profiles(), "pcsx2/PCSX2");
        assert_eq!(
            found.map(|p| p.name.as_str()),
            Some("PCSX2 (Playstation 2)")
        );
    }

    #[test]
    fn find_profile_misses_unknown_source_id() {
        assert!(find_profile(load_profiles(), "nobody/nothing").is_none());
    }

    #[test]
    fn find_profile_skips_compat_tools() {
        // GE-Proton's real owner/repo, confirmed against the embedded catalog.
        assert!(find_profile(load_profiles(), "GloriousEggroll/proton-ge-custom").is_none());
    }
}
