//! Layer 1 of emulator autoconfig: turning a matched autoprofile plus a
//! resolved executable path into a config `EmulatorEntry`, and assigning the
//! platform/core default maps that follow from it.
//!
//! Ports `grid_launcher/emulator/autoconfig.py`'s entry-writing half
//! (`auto_configure_emulator_settings` :472, `assign_profile_platform_defaults`
//! :346, `apply_manual_emulator_profile_defaults` :228) together with the
//! Dolphin-variant and platform-filter helpers it calls out to in
//! `grid_launcher/emulator/selection.py` and `profiles.py`. See
//! `docs/porting/05-emulator-autoconfig.md` "Layer 1 — entry autoconfig".

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::{platform_matches_keywords, profile_for_entry, EmulatorProfile};

/// The `[^a-z0-9]+` run-collapse the Dolphin variant rules apply to an
/// already-casefolded string (selection.py:180, selection.py:200).
static NON_ALNUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

/// The `(compact, tokens)` pair both Dolphin rules derive from a casefolded
/// string: every non-alphanumeric run becomes a space, the result is
/// trimmed, `compact` drops the spaces and `tokens` splits on them.
fn compact_and_tokens(folded: &str) -> (String, HashSet<String>) {
    let normalized = NON_ALNUM_RE.replace_all(folded, " ");
    let normalized = normalized.trim();
    let compact = normalized.replace(' ', "");
    let tokens = normalized.split_whitespace().map(str::to_string).collect();
    (compact, tokens)
}

/// The game fields the Dolphin variant rule reads (selection.py:168-190).
///
/// `None` at the layer-1 trigger points that have no game in hand — exactly
/// like the reference's backfill call site, which passes `game=None`
/// (emulator_ui_mixin.py:1820) — in which case the Dolphin variant branch is
/// inert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameFacts {
    pub title: String,
    pub platform: String,
    pub rom_file_name: String,
}

/// The inputs layer 1 needs that grid-core cannot derive on its own: the
/// server's platform list, installed-core discovery, and the "is this
/// RetroArch?" predicate the UI owns.
///
/// The reference passes these as keyword-only callables
/// (autoconfig.py:353-360); this struct is the same seam.
pub struct DefaultsContext<'a> {
    /// Assignable server platform names, already run through
    /// [`assignable_platforms`].
    pub platforms: &'a [String],
    /// `(platform, emulator_name) -> installed compatible core ids`
    /// (`_installed_retroarch_cores_for_platform`). Production passes a
    /// closure over [`super::cores::installed_core_ids`] +
    /// [`super::cores::cores_for_platform`]; tests pass a table.
    pub installed_cores: &'a dyn Fn(&str, &str) -> Vec<String>,
    /// `emulator_name -> is this RetroArch?` — the ported
    /// `_is_retroarch_emulator_name` predicate (emulator_ui_mixin.py:1916).
    pub is_retroarch: &'a dyn Fn(&str) -> bool,
}

/// Profile list values flattened the way the reference does
/// (`_multiline_profile_value`, autoconfig.py:106): blank items dropped, each
/// item trimmed, joined with the literal separator `";\n"`.
pub fn multiline_profile_value(items: &[String]) -> String {
    items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<&str>>()
        .join(";\n")
}

/// `normalize_save_strategy_value` (profiles.py:141-156). Aliases map to one
/// of `"auto"` | `"single_file"` | `"folder"`; anything unrecognized (blank
/// included) becomes `"auto"`.
pub fn normalize_save_strategy(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "" | "auto" => "auto",
        "singlefile" | "single_file" | "single-file" | "single file" | "file" => "single_file",
        "folder" | "directory" | "folder_per_game" | "folder-per-game" => "folder",
        _ => "auto",
    }
    .to_string()
}

/// `dolphin_variant_label_for_game` (selection.py:168-190). The three fields
/// are read in this order, blank ones skipped, and the survivors joined with
/// a space before the alphanumeric run-collapse. `"gamecube"` in the compact
/// form wins outright; `"wiiu"` anywhere in it is a veto that returns `""`
/// even when `wii` is a standalone token; only then does a `wii` token yield
/// `"Wii"`.
pub fn dolphin_variant_label(title: &str, platform: &str, rom_file_name: &str) -> String {
    let candidates: Vec<&str> = [title, platform, rom_file_name]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if candidates.is_empty() {
        return String::new();
    }

    let combined = candidates.join(" ").to_lowercase();
    let (compact, tokens) = compact_and_tokens(&combined);

    if compact.contains("gamecube") {
        return "GameCube".to_string();
    }
    if compact.contains("wiiu") {
        return String::new();
    }
    if tokens.contains("wii") {
        return "Wii".to_string();
    }
    String::new()
}

/// `dolphin_target_platforms_for_variant` (selection.py:192-212). `[]` for
/// any variant but `gamecube`/`wii`. A GameCube platform is one whose
/// compact form contains `"gamecube"`; a Wii platform is one with a `wii`
/// token whose compact form does NOT contain `"wiiu"`, which is what keeps
/// "Nintendo Wii U" out of the Wii list.
pub fn dolphin_target_platforms(variant: &str, platforms: &[String]) -> Vec<String> {
    let selected = variant.trim().to_lowercase();
    if selected != "gamecube" && selected != "wii" {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for platform in platforms {
        let (compact, tokens) = compact_and_tokens(&platform.to_lowercase());
        let hit = if selected == "gamecube" {
            compact.contains("gamecube")
        } else {
            tokens.contains("wii") && !compact.contains("wiiu")
        };
        if hit {
            matches.push(platform.clone());
        }
    }
    matches
}

/// `auto_configured_emulator_name` (autoconfig.py:90-103). The trimmed
/// `base_name` unchanged unless it case-folds to `"dolphin"` and `variant`
/// is non-empty, in which case `"{base} ({variant})"`.
pub fn auto_configured_emulator_name(base_name: &str, variant: &str) -> String {
    let normalized = base_name.trim();
    if normalized.to_lowercase() != "dolphin" || variant.is_empty() {
        return normalized.to_string();
    }
    format!("{normalized} ({variant})")
}

/// `default_assignable_server_platforms` (selection.py:157-165): drops any
/// platform whose trimmed casefolded name starts with `"windows"` or equals
/// `"emulators"`. Order and the original (untrimmed) spelling are kept.
pub fn assignable_platforms(platforms: &[String]) -> Vec<String> {
    platforms
        .iter()
        .filter(|platform| {
            let folded = platform.trim().to_lowercase();
            !folded.starts_with("windows") && folded != "emulators"
        })
        .cloned()
        .collect()
}

/// The platforms one profile claims (autoconfig.py:355-379): every
/// assignable platform when `all_platforms` is set — narrowed, for RetroArch
/// only, to those with at least one INSTALLED compatible core — otherwise
/// the keyword-matched ones, which a Dolphin profile's non-empty
/// variant-platform list replaces outright.
fn target_platforms(
    game: Option<&GameFacts>,
    emulator_name: &str,
    profile: &EmulatorProfile,
    ctx: &DefaultsContext,
) -> Vec<String> {
    if profile.all_platforms {
        let mut targets = ctx.platforms.to_vec();
        if (ctx.is_retroarch)(emulator_name) {
            targets.retain(|platform| !(ctx.installed_cores)(platform, emulator_name).is_empty());
        }
        return targets;
    }

    let mut targets: Vec<String> = ctx
        .platforms
        .iter()
        .filter(|platform| platform_matches_keywords(platform, &profile.platform_keywords))
        .cloned()
        .collect();

    if let Some(game) = game {
        if profile.name.trim().to_lowercase() == "dolphin" {
            let variant = dolphin_variant_label(&game.title, &game.platform, &game.rom_file_name);
            let variant_platforms = dolphin_target_platforms(&variant, ctx.platforms);
            if !variant_platforms.is_empty() {
                targets = variant_platforms;
            }
        }
    }
    targets
}

/// `assign_profile_platform_defaults` (autoconfig.py:346-402).
///
/// Per target platform: a blank current default is filled with
/// `emulator_name`; a non-blank one is replaced ONLY when the incoming
/// emulator is not RetroArch and the current default IS — a native emulator
/// outranks RetroArch, never the reverse. Then, for RetroArch only, each
/// platform whose default now case-folds equal to `emulator_name` and that
/// has no core default yet records the FIRST installed compatible core.
///
/// Note on the `game` argument: Python tests it for truthiness, so an empty
/// game dict skips the Dolphin branch while `Some(GameFacts::default())`
/// enters it here. The outcome is identical — an all-blank `GameFacts`
/// yields no variant, hence no variant platforms, hence no replacement.
pub fn assign_profile_platform_defaults(
    game: Option<&GameFacts>,
    emulator_name: &str,
    profile: &EmulatorProfile,
    defaults: &BTreeMap<String, String>,
    core_defaults: &BTreeMap<String, String>,
    ctx: &DefaultsContext,
) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let mut resolved_defaults = defaults.clone();
    let mut resolved_core_defaults = core_defaults.clone();
    let targets = target_platforms(game, emulator_name, profile, ctx);
    let incoming_is_retroarch = (ctx.is_retroarch)(emulator_name);

    for platform in &targets {
        let current_default = resolved_defaults
            .get(platform)
            .map(|value| value.trim())
            .unwrap_or("");
        if current_default.is_empty() {
            resolved_defaults.insert(platform.clone(), emulator_name.to_string());
            continue;
        }
        if !incoming_is_retroarch && (ctx.is_retroarch)(current_default) {
            resolved_defaults.insert(platform.clone(), emulator_name.to_string());
        }
    }

    if incoming_is_retroarch {
        let folded_name = emulator_name.to_lowercase();
        for platform in &targets {
            let claimed = resolved_defaults
                .get(platform)
                .map(|value| value.trim().to_lowercase())
                .unwrap_or_default();
            if claimed != folded_name {
                continue;
            }
            let existing_core = resolved_core_defaults
                .get(platform)
                .map(|value| value.trim())
                .unwrap_or("");
            if !existing_core.is_empty() {
                continue;
            }
            if let Some(first) = (ctx.installed_cores)(platform, emulator_name).first() {
                resolved_core_defaults.insert(platform.clone(), first.clone());
            }
        }
    }

    (resolved_defaults, resolved_core_defaults)
}

/// `auto_configure_emulator_settings` (autoconfig.py:472-582): writes the
/// entry a matched autoprofile plus a resolved executable path imply, then
/// assigns the platform defaults that follow from it.
///
/// An existing entry is matched by name, the LEFT side trimmed and the right
/// side not (autoconfig.py:505) — an asymmetry this port keeps. That entry is
/// then REBUILT: `name` and `path` are always overwritten, `args` is replaced
/// for RetroArch or when the current value is blank or exactly `%rom%` (and
/// otherwise kept, trimmed), `save_strategy` keeps a non-blank current value
/// but re-normalizes it, and the four path/ignore fields keep a non-blank
/// trimmed current value or else take the profile's.
///
/// Deviation D12: the reference rebuilds with exactly eight keys, dropping
/// anything else the entry carried. This port preserves the five `source_*`
/// fields — the rewrite's own install provenance, which the reference has no
/// equivalent for.
pub fn auto_configure_emulator_settings(
    game: Option<&GameFacts>,
    executable_path: &str,
    profile: &EmulatorProfile,
    emulators: &[EmulatorEntry],
    defaults: &BTreeMap<String, String>,
    core_defaults: &BTreeMap<String, String>,
    ctx: &DefaultsContext,
) -> (
    Vec<EmulatorEntry>,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
) {
    // `profile.get("name", "Emulator")` (autoconfig.py:489). Rust cannot tell
    // an absent key from a present-but-empty one, so a blank name takes the
    // fallback here where Python would keep the blank. Unreachable in
    // practice: `profiles::normalize_one` drops every blank-named profile.
    let base_name = if profile.name.is_empty() {
        "Emulator"
    } else {
        profile.name.as_str()
    };
    let variant = game
        .map(|g| dolphin_variant_label(&g.title, &g.platform, &g.rom_file_name))
        .unwrap_or_default();
    let emulator_name = auto_configured_emulator_name(base_name, &variant);

    let args_trimmed = profile.args.trim();
    let args_template = if args_trimmed.is_empty() {
        "%rom%"
    } else {
        args_trimmed
    };
    let profile_save_strategy = normalize_save_strategy(&profile.save_strategy);
    let profile_ignore_files = multiline_profile_value(&profile.ignore_files);
    let profile_ignore_extensions = multiline_profile_value(&profile.ignore_extensions);
    let profile_save_paths = multiline_profile_value(&profile.save_directories);
    let profile_state_paths = multiline_profile_value(&profile.state_directories);

    let folded_name = emulator_name.to_lowercase();
    let mut resolved_emulators = emulators.to_vec();
    let target_index = resolved_emulators
        .iter()
        .position(|entry| entry.name.trim().to_lowercase() == folded_name);

    /// A non-blank trimmed current value, else the profile's.
    fn kept_or_profile(current: &str, from_profile: &str) -> String {
        let trimmed = current.trim();
        if trimmed.is_empty() {
            from_profile.to_string()
        } else {
            trimmed.to_string()
        }
    }

    match target_index {
        Some(index) => {
            let existing = &resolved_emulators[index];
            let existing_args = existing.args.trim();
            let should_update_args = (ctx.is_retroarch)(&emulator_name)
                || existing_args.is_empty()
                || existing_args == "%rom%";
            let rebuilt = EmulatorEntry {
                name: emulator_name.clone(),
                path: executable_path.to_string(),
                args: if should_update_args {
                    args_template.to_string()
                } else {
                    existing_args.to_string()
                },
                save_strategy: if existing.save_strategy.trim().is_empty() {
                    profile_save_strategy.clone()
                } else {
                    normalize_save_strategy(&existing.save_strategy)
                },
                ignore_files: kept_or_profile(&existing.ignore_files, &profile_ignore_files),
                ignore_extensions: kept_or_profile(
                    &existing.ignore_extensions,
                    &profile_ignore_extensions,
                ),
                save_paths: kept_or_profile(&existing.save_paths, &profile_save_paths),
                state_paths: kept_or_profile(&existing.state_paths, &profile_state_paths),
                // D12: install provenance survives the rebuild.
                source_id: existing.source_id.clone(),
                source_provider: existing.source_provider.clone(),
                source_owner: existing.source_owner.clone(),
                source_repo: existing.source_repo.clone(),
                source_release_tag: existing.source_release_tag.clone(),
            };
            resolved_emulators[index] = rebuilt;
        }
        None => resolved_emulators.push(EmulatorEntry {
            name: emulator_name.clone(),
            path: executable_path.to_string(),
            args: args_template.to_string(),
            save_strategy: profile_save_strategy,
            ignore_files: profile_ignore_files,
            ignore_extensions: profile_ignore_extensions,
            save_paths: profile_save_paths,
            state_paths: profile_state_paths,
            ..Default::default()
        }),
    }

    let (resolved_defaults, resolved_core_defaults) = assign_profile_platform_defaults(
        game,
        &emulator_name,
        profile,
        defaults,
        core_defaults,
        ctx,
    );

    (
        resolved_emulators,
        resolved_defaults,
        resolved_core_defaults,
    )
}

/// `apply_manual_emulator_profile_defaults` (autoconfig.py:228-270): the
/// hand-typed-entry path. Unlike layer 1's rebuild this COPIES the entry, so
/// unlisted fields survive. `name` is filled only when blank; `args` is
/// replaced when blank or exactly `%rom%`; `save_strategy` is replaced
/// whenever the current value normalizes to `"auto"`, so `"auto"` itself (and
/// any unrecognized alias) counts as unset. The four list-backed fields are
/// filled only when blank, and the profile's value is written even when it is
/// `""`. `path` is NEVER touched.
pub fn apply_manual_emulator_profile_defaults(
    entry: &EmulatorEntry,
    profile: &EmulatorProfile,
) -> EmulatorEntry {
    let mut resolved = entry.clone();

    if resolved.name.trim().is_empty() && !profile.name.trim().is_empty() {
        resolved.name = profile.name.trim().to_string();
    }

    let current_args = resolved.args.trim();
    if (current_args.is_empty() || current_args == "%rom%") && !profile.args.trim().is_empty() {
        resolved.args = profile.args.trim().to_string();
    }

    if normalize_save_strategy(&resolved.save_strategy) == "auto" {
        resolved.save_strategy = normalize_save_strategy(&profile.save_strategy);
    }

    // autoconfig.py:258 — `save_paths` takes the profile's `save_directories`
    // and `state_paths` its `state_directories`; the other two share a name.
    let field_map: [(&mut String, &Vec<String>); 4] = [
        (&mut resolved.ignore_files, &profile.ignore_files),
        (&mut resolved.ignore_extensions, &profile.ignore_extensions),
        (&mut resolved.save_paths, &profile.save_directories),
        (&mut resolved.state_paths, &profile.state_directories),
    ];
    for (current, from_profile) in field_map {
        if current.trim().is_empty() {
            *current = multiline_profile_value(from_profile);
        }
    }

    resolved
}

/// `_backfill_missing_emulator_defaults` (emulator_ui_mixin.py:1790-1839).
///
/// Re-runs the platform assignment for every registered emulator with a
/// matching profile, each iteration building on the maps the previous one
/// left behind, and writes both results back into `config`. Entries with a
/// blank name or no matching profile are skipped. Returns `true` when either
/// map ended up different from where it started — the caller saves only then.
pub fn backfill_missing_defaults(
    config: &mut Config,
    profiles: &[EmulatorProfile],
    ctx: &DefaultsContext,
) -> bool {
    let original_defaults = config.default_emulators.clone();
    let original_core_defaults = config.retroarch_cores.clone();

    for entry in config.emulators.clone() {
        let entry_name = entry.name.trim();
        if entry_name.is_empty() {
            continue;
        }
        let Some(profile) = profile_for_entry(&entry.name, &entry.path, profiles) else {
            continue;
        };
        let (defaults, core_defaults) = assign_profile_platform_defaults(
            None,
            entry_name,
            profile,
            &config.default_emulators,
            &config.retroarch_cores,
            ctx,
        );
        config.default_emulators = defaults;
        config.retroarch_cores = core_defaults;
    }

    config.default_emulators != original_defaults
        || config.retroarch_cores != original_core_defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn no_cores(_platform: &str, _emulator: &str) -> Vec<String> {
        Vec::new()
    }

    fn is_retroarch_name(name: &str) -> bool {
        name.trim().to_lowercase().contains("retroarch")
    }

    /// A profile carrying every field layer 1 reads.
    fn full_profile(name: &str) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            args: " -batch %rom% ".to_string(),
            save_strategy: "single-file".to_string(),
            save_directories: strings(&["~/a", "~/b"]),
            state_directories: strings(&["~/s"]),
            ignore_files: strings(&["thumbs.db"]),
            ignore_extensions: strings(&[".jpg"]),
            ..Default::default()
        }
    }

    // --- pure helpers -------------------------------------------------------

    #[test]
    fn multiline_profile_value_joins_with_semicolon_newline_and_drops_blanks() {
        let items = strings(&["  ~/a  ", "", "   ", "~/b"]);
        assert_eq!(multiline_profile_value(&items), "~/a;\n~/b");
        assert_eq!(multiline_profile_value(&[]), "");
        assert_eq!(multiline_profile_value(&strings(&["", "  "])), "");
    }

    #[test]
    fn normalize_save_strategy_alias_table() {
        let cases: &[(&str, &str)] = &[
            ("", "auto"),
            ("auto", "auto"),
            ("singlefile", "single_file"),
            ("single_file", "single_file"),
            ("single-file", "single_file"),
            ("single file", "single_file"),
            ("file", "single_file"),
            ("folder", "folder"),
            ("directory", "folder"),
            ("folder_per_game", "folder"),
            ("folder-per-game", "folder"),
        ];
        assert_eq!(cases.len(), 11, "profiles.py:143-155 has 11 aliases");
        for (input, expected) in cases {
            assert_eq!(normalize_save_strategy(input), *expected, "input={input:?}");
            // Trimmed and casefolded before the lookup.
            let padded = format!("  {}  ", input.to_uppercase());
            assert_eq!(
                normalize_save_strategy(&padded),
                *expected,
                "input={padded:?}"
            );
        }
        assert_eq!(normalize_save_strategy("not-a-strategy"), "auto");
    }

    #[test]
    fn dolphin_variant_gamecube_wii_and_wii_u_exclusion() {
        assert_eq!(
            dolphin_variant_label("Mario Kart", "Nintendo GameCube", ""),
            "GameCube"
        );
        assert_eq!(dolphin_variant_label("", "Nintendo Wii", ""), "Wii");
        assert_eq!(dolphin_variant_label("", "", "SuperGame.wii"), "Wii");
        // "wiiu" anywhere in the compacted text is an outright veto, even
        // though "wii" is a token (selection.py:186).
        assert_eq!(dolphin_variant_label("", "Nintendo Wii U", ""), "");
        assert_eq!(dolphin_variant_label("", "WiiU", ""), "");
        // GameCube is checked first, so it wins over a Wii hint.
        assert_eq!(
            dolphin_variant_label("Wii Sports", "Nintendo GameCube", ""),
            "GameCube"
        );
        // No candidate fields at all.
        assert_eq!(dolphin_variant_label("  ", "", "   "), "");
    }

    #[test]
    fn dolphin_target_platforms_excludes_wii_u() {
        let platforms = strings(&["Nintendo GameCube", "Nintendo Wii", "Nintendo Wii U"]);
        assert_eq!(
            dolphin_target_platforms("Wii", &platforms),
            strings(&["Nintendo Wii"])
        );
        assert_eq!(
            dolphin_target_platforms("gamecube", &platforms),
            strings(&["Nintendo GameCube"])
        );
        // Anything but gamecube/wii yields nothing at all.
        assert_eq!(
            dolphin_target_platforms("", &platforms),
            Vec::<String>::new()
        );
        assert_eq!(
            dolphin_target_platforms("Wii U", &platforms),
            Vec::<String>::new()
        );
    }

    #[test]
    fn auto_configured_name_appends_the_variant_only_for_dolphin() {
        assert_eq!(
            auto_configured_emulator_name("Dolphin", "Wii"),
            "Dolphin (Wii)"
        );
        assert_eq!(
            auto_configured_emulator_name("  dolphin  ", "GameCube"),
            "dolphin (GameCube)"
        );
        assert_eq!(auto_configured_emulator_name("Dolphin", ""), "Dolphin");
        assert_eq!(auto_configured_emulator_name("PCSX2", "Wii"), "PCSX2");
        assert_eq!(auto_configured_emulator_name("  PCSX2  ", ""), "PCSX2");
    }

    #[test]
    fn assignable_platforms_drops_windows_prefixed_and_emulators() {
        let platforms = strings(&[
            "Nintendo 64",
            "Windows",
            "  windows games ",
            "Emulators",
            " emulators ",
            "PlayStation 2",
        ]);
        assert_eq!(
            assignable_platforms(&platforms),
            strings(&["Nintendo 64", "PlayStation 2"])
        );
    }

    // --- auto_configure_emulator_settings -----------------------------------

    /// Runs layer 1 with no server platforms, so only the entry list moves.
    fn configure(
        profile: &EmulatorProfile,
        path: &str,
        emulators: &[EmulatorEntry],
    ) -> Vec<EmulatorEntry> {
        let platforms: Vec<String> = Vec::new();
        let ctx = DefaultsContext {
            platforms: &platforms,
            installed_cores: &no_cores,
            is_retroarch: &is_retroarch_name,
        };
        let (entries, _, _) = auto_configure_emulator_settings(
            None,
            path,
            profile,
            emulators,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ctx,
        );
        entries
    }

    #[test]
    fn auto_configure_creates_a_new_entry_with_all_profile_values() {
        let profile = full_profile("PCSX2 (Playstation 2)");
        let entries = configure(&profile, "/x/pcsx2-qt", &[]);

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.name, "PCSX2 (Playstation 2)");
        assert_eq!(entry.path, "/x/pcsx2-qt");
        assert_eq!(entry.args, "-batch %rom%");
        assert_eq!(entry.save_strategy, "single_file");
        assert_eq!(entry.ignore_files, "thumbs.db");
        assert_eq!(entry.ignore_extensions, ".jpg");
        assert_eq!(entry.save_paths, "~/a;\n~/b");
        assert_eq!(entry.state_paths, "~/s");
    }

    #[test]
    fn auto_configure_new_entry_falls_back_to_the_rom_placeholder_for_blank_args() {
        let mut profile = full_profile("PCSX2");
        profile.args = "   ".to_string();
        let entries = configure(&profile, "/x/pcsx2", &[]);
        assert_eq!(entries[0].args, "%rom%");
    }

    #[test]
    fn auto_configure_always_overwrites_name_and_path() {
        let profile = full_profile("PCSX2");
        let existing = EmulatorEntry {
            name: "  pcsx2  ".into(),
            path: "/old/pcsx2".into(),
            args: "--custom".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/new/pcsx2", std::slice::from_ref(&existing));

        assert_eq!(entries.len(), 1, "the existing entry is updated in place");
        assert_eq!(entries[0].name, "PCSX2");
        assert_eq!(entries[0].path, "/new/pcsx2");
    }

    #[test]
    fn auto_configure_replaces_args_for_retroarch() {
        let mut profile = full_profile("RetroArch (Multi-System)");
        profile.args = "-L %core% %rom%".into();
        let existing = EmulatorEntry {
            name: "RetroArch (Multi-System)".into(),
            path: "/x/retroarch".into(),
            args: "--my --own --flags".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/x/retroarch", std::slice::from_ref(&existing));
        assert_eq!(entries[0].args, "-L %core% %rom%");
    }

    #[test]
    fn auto_configure_replaces_blank_args() {
        let profile = full_profile("PCSX2");
        let existing = EmulatorEntry {
            name: "PCSX2".into(),
            path: "/x".into(),
            args: "   ".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/x", std::slice::from_ref(&existing));
        assert_eq!(entries[0].args, "-batch %rom%");
    }

    #[test]
    fn auto_configure_replaces_the_bare_rom_placeholder() {
        let profile = full_profile("PCSX2");
        let existing = EmulatorEntry {
            name: "PCSX2".into(),
            path: "/x".into(),
            args: "  %rom%  ".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/x", std::slice::from_ref(&existing));
        assert_eq!(entries[0].args, "-batch %rom%");
    }

    #[test]
    fn auto_configure_preserves_custom_args_trimmed() {
        let profile = full_profile("PCSX2");
        let existing = EmulatorEntry {
            name: "PCSX2".into(),
            path: "/x".into(),
            args: "  --my --own --flags  ".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/x", std::slice::from_ref(&existing));
        assert_eq!(entries[0].args, "--my --own --flags");
    }

    #[test]
    fn auto_configure_preserves_nonblank_fields_and_fills_blank_ones() {
        let profile = full_profile("PCSX2");
        let existing = EmulatorEntry {
            name: "PCSX2".into(),
            path: "/x".into(),
            args: "--custom".into(),
            save_strategy: "  Folder  ".into(),
            ignore_files: "  keep.txt  ".into(),
            ignore_extensions: String::new(),
            save_paths: "   ".into(),
            state_paths: " ~/keep-states ".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/x", std::slice::from_ref(&existing));
        let entry = &entries[0];

        // Kept, but re-normalized / trimmed.
        assert_eq!(entry.save_strategy, "folder");
        assert_eq!(entry.ignore_files, "keep.txt");
        assert_eq!(entry.state_paths, "~/keep-states");
        // Blank -> profile value.
        assert_eq!(entry.ignore_extensions, ".jpg");
        assert_eq!(entry.save_paths, "~/a;\n~/b");
    }

    #[test]
    fn auto_configure_preserves_the_source_fields_on_an_existing_entry() {
        // Deviation D12: the reference rebuilds the entry with exactly eight
        // keys, which would drop the rewrite's own install provenance.
        let profile = full_profile("PCSX2");
        let existing = EmulatorEntry {
            name: "PCSX2".into(),
            path: "/old".into(),
            args: "--custom".into(),
            source_id: "PCSX2/pcsx2".into(),
            source_provider: "github".into(),
            source_owner: "PCSX2".into(),
            source_repo: "pcsx2".into(),
            source_release_tag: "v2.1.0".into(),
            ..Default::default()
        };
        let entries = configure(&profile, "/new", std::slice::from_ref(&existing));
        let entry = &entries[0];

        assert_eq!(entry.source_id, "PCSX2/pcsx2");
        assert_eq!(entry.source_provider, "github");
        assert_eq!(entry.source_owner, "PCSX2");
        assert_eq!(entry.source_repo, "pcsx2");
        assert_eq!(entry.source_release_tag, "v2.1.0");
        assert_eq!(entry.path, "/new", "the rebuild still happened");
    }

    #[test]
    fn auto_configure_matches_an_existing_entry_case_insensitively() {
        let profile = full_profile("RetroArch (Multi-System)");
        let entries_before = vec![
            EmulatorEntry {
                name: "PPSSPP".into(),
                path: "/x/ppsspp".into(),
                ..Default::default()
            },
            EmulatorEntry {
                name: "  RETROARCH (multi-system)  ".into(),
                path: "/old/retroarch".into(),
                ..Default::default()
            },
        ];
        let entries = configure(&profile, "/new/retroarch", &entries_before);

        assert_eq!(entries.len(), 2, "no new entry is appended");
        assert_eq!(entries[0].name, "PPSSPP", "unrelated entries are untouched");
        assert_eq!(entries[1].name, "RetroArch (Multi-System)");
        assert_eq!(entries[1].path, "/new/retroarch");
    }

    // --- assign_profile_platform_defaults -----------------------------------

    fn assign(
        game: Option<&GameFacts>,
        emulator_name: &str,
        profile: &EmulatorProfile,
        defaults: &BTreeMap<String, String>,
        core_defaults: &BTreeMap<String, String>,
        platforms: &[String],
        installed_cores: &dyn Fn(&str, &str) -> Vec<String>,
    ) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
        let ctx = DefaultsContext {
            platforms,
            installed_cores,
            is_retroarch: &is_retroarch_name,
        };
        assign_profile_platform_defaults(
            game,
            emulator_name,
            profile,
            defaults,
            core_defaults,
            &ctx,
        )
    }

    fn keyword_profile(name: &str, keywords: &[&str]) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            args: "%rom%".to_string(),
            platform_keywords: strings(keywords),
            ..Default::default()
        }
    }

    #[test]
    fn assign_defaults_fills_an_empty_platform_default() {
        let profile = keyword_profile("PPSSPP", &["playstation portable"]);
        let platforms = strings(&["PlayStation Portable", "Nintendo 64"]);
        let (defaults, cores) = assign(
            None,
            "PPSSPP",
            &profile,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &platforms,
            &no_cores,
        );
        assert_eq!(defaults, map(&[("PlayStation Portable", "PPSSPP")]));
        assert!(
            cores.is_empty(),
            "a native emulator records no core default"
        );
    }

    #[test]
    fn assign_defaults_lets_a_native_emulator_displace_retroarch() {
        let profile = keyword_profile("PPSSPP", &["playstation portable"]);
        let platforms = strings(&["PlayStation Portable"]);
        let (defaults, _) = assign(
            None,
            "PPSSPP",
            &profile,
            &map(&[("PlayStation Portable", " RetroArch ")]),
            &BTreeMap::new(),
            &platforms,
            &no_cores,
        );
        assert_eq!(
            defaults.get("PlayStation Portable").map(String::as_str),
            Some("PPSSPP")
        );
    }

    #[test]
    fn assign_defaults_never_lets_retroarch_displace_a_native_emulator() {
        let profile = keyword_profile("RetroArch (Multi-System)", &["playstation portable"]);
        let platforms = strings(&["PlayStation Portable"]);
        let cores = |_: &str, _: &str| strings(&["ppsspp"]);
        let (defaults, core_defaults) = assign(
            None,
            "RetroArch (Multi-System)",
            &profile,
            &map(&[("PlayStation Portable", "PPSSPP")]),
            &BTreeMap::new(),
            &platforms,
            &cores,
        );
        assert_eq!(
            defaults.get("PlayStation Portable").map(String::as_str),
            Some("PPSSPP")
        );
        assert!(
            core_defaults.is_empty(),
            "the platform's default is not this emulator, so no core is recorded"
        );
    }

    #[test]
    fn assign_defaults_filters_all_platforms_by_installed_cores_for_retroarch() {
        let profile = EmulatorProfile {
            name: "RetroArch (Multi-System)".into(),
            args: "%rom%".into(),
            all_platforms: true,
            ..Default::default()
        };
        let platforms = strings(&["Super Nintendo", "Sega Saturn"]);
        let cores = |platform: &str, _: &str| {
            if platform == "Super Nintendo" {
                strings(&["snes9x", "bsnes"])
            } else {
                Vec::new()
            }
        };
        let (defaults, core_defaults) = assign(
            None,
            "RetroArch (Multi-System)",
            &profile,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &platforms,
            &cores,
        );
        assert_eq!(
            defaults,
            map(&[("Super Nintendo", "RetroArch (Multi-System)")])
        );
        assert_eq!(core_defaults, map(&[("Super Nintendo", "snes9x")]));
    }

    #[test]
    fn assign_defaults_all_platforms_is_unfiltered_for_a_native_emulator() {
        let profile = EmulatorProfile {
            name: "MAME".into(),
            args: "%rom%".into(),
            all_platforms: true,
            ..Default::default()
        };
        let platforms = strings(&["Super Nintendo", "Sega Saturn"]);
        let (defaults, _) = assign(
            None,
            "MAME",
            &profile,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &platforms,
            &no_cores,
        );
        assert_eq!(
            defaults,
            map(&[("Sega Saturn", "MAME"), ("Super Nintendo", "MAME")])
        );
    }

    #[test]
    fn assign_defaults_records_the_first_installed_core_only_when_unset() {
        let profile = EmulatorProfile {
            name: "RetroArch (Multi-System)".into(),
            args: "%rom%".into(),
            all_platforms: true,
            ..Default::default()
        };
        let platforms = strings(&["Super Nintendo", "Nintendo 64"]);
        let cores = |platform: &str, _: &str| match platform {
            "Super Nintendo" => strings(&["snes9x", "bsnes"]),
            _ => strings(&["mupen64plus_next", "parallel_n64"]),
        };
        let (_, core_defaults) = assign(
            None,
            "RetroArch (Multi-System)",
            &profile,
            &BTreeMap::new(),
            &map(&[("Super Nintendo", "bsnes")]),
            &platforms,
            &cores,
        );
        assert_eq!(
            core_defaults,
            map(&[
                ("Nintendo 64", "mupen64plus_next"),
                ("Super Nintendo", "bsnes"),
            ])
        );
    }

    #[test]
    fn assign_defaults_dolphin_variant_platforms_replace_the_keyword_match() {
        let profile = keyword_profile("Dolphin", &["gamecube"]);
        let platforms = strings(&["Nintendo GameCube", "Nintendo Wii", "Nintendo Wii U"]);
        let game = GameFacts {
            title: "Wii Sports".into(),
            platform: "Nintendo Wii".into(),
            rom_file_name: "wii-sports.wbfs".into(),
        };
        let (defaults, _) = assign(
            Some(&game),
            "Dolphin (Wii)",
            &profile,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &platforms,
            &no_cores,
        );
        assert_eq!(defaults, map(&[("Nintendo Wii", "Dolphin (Wii)")]));
    }

    #[test]
    fn assign_defaults_keyword_match_survives_an_empty_variant_list() {
        let profile = keyword_profile("Dolphin", &["gamecube"]);
        let platforms = strings(&["Nintendo GameCube", "Nintendo Wii"]);
        // No GameCube/Wii hint anywhere, so the variant is "" and
        // dolphin_target_platforms returns nothing.
        let game = GameFacts {
            title: "Some Game".into(),
            platform: "Unknown Platform".into(),
            rom_file_name: "some-game.iso".into(),
        };
        let (defaults, _) = assign(
            Some(&game),
            "Dolphin",
            &profile,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &platforms,
            &no_cores,
        );
        assert_eq!(defaults, map(&[("Nintendo GameCube", "Dolphin")]));
    }

    #[test]
    fn assign_defaults_dolphin_variant_branch_is_inert_without_a_game() {
        let profile = keyword_profile("Dolphin", &["gamecube"]);
        let platforms = strings(&["Nintendo GameCube", "Nintendo Wii"]);
        let (defaults, _) = assign(
            None,
            "Dolphin",
            &profile,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &platforms,
            &no_cores,
        );
        assert_eq!(defaults, map(&[("Nintendo GameCube", "Dolphin")]));
    }

    // --- apply_manual_emulator_profile_defaults -----------------------------

    #[test]
    fn manual_defaults_fill_blank_fields_only_and_never_touch_path() {
        let profile = full_profile("PCSX2");
        let entry = EmulatorEntry {
            name: "   ".into(),
            path: "/keep/me".into(),
            args: "--custom".into(),
            save_strategy: "folder".into(),
            ignore_files: "  keep.txt  ".into(),
            source_id: "PCSX2/pcsx2".into(),
            ..Default::default()
        };
        let filled = apply_manual_emulator_profile_defaults(&entry, &profile);

        assert_eq!(filled.name, "PCSX2");
        assert_eq!(filled.path, "/keep/me", "path is never touched");
        assert_eq!(filled.args, "--custom");
        assert_eq!(filled.save_strategy, "folder");
        assert_eq!(
            filled.ignore_files, "  keep.txt  ",
            "a non-blank value is kept verbatim, not trimmed"
        );
        assert_eq!(filled.ignore_extensions, ".jpg");
        assert_eq!(filled.save_paths, "~/a;\n~/b");
        assert_eq!(filled.state_paths, "~/s");
        assert_eq!(
            filled.source_id, "PCSX2/pcsx2",
            "unlisted fields survive the copy"
        );
    }

    #[test]
    fn manual_defaults_write_a_blank_profile_value_over_a_blank_field() {
        let mut profile = full_profile("PCSX2");
        profile.save_directories = Vec::new();
        let entry = EmulatorEntry {
            name: "PCSX2".into(),
            save_paths: "   ".into(),
            ..Default::default()
        };
        let filled = apply_manual_emulator_profile_defaults(&entry, &profile);
        assert_eq!(filled.save_paths, "");
    }

    #[test]
    fn manual_defaults_replace_auto_save_strategy() {
        let profile = full_profile("PCSX2");
        for current in ["", "auto", "  AUTO  ", "not-a-strategy"] {
            let entry = EmulatorEntry {
                name: "PCSX2".into(),
                save_strategy: current.into(),
                ..Default::default()
            };
            let filled = apply_manual_emulator_profile_defaults(&entry, &profile);
            assert_eq!(filled.save_strategy, "single_file", "current={current:?}");
        }
    }

    #[test]
    fn manual_defaults_replace_the_bare_rom_placeholder_args() {
        let profile = full_profile("PCSX2");
        for current in ["%rom%", "  %rom%  ", "", "   "] {
            let entry = EmulatorEntry {
                name: "PCSX2".into(),
                args: current.into(),
                ..Default::default()
            };
            let filled = apply_manual_emulator_profile_defaults(&entry, &profile);
            assert_eq!(filled.args, "-batch %rom%", "current={current:?}");
        }

        // A blank profile args leaves the entry's args alone.
        let mut blank_args = full_profile("PCSX2");
        blank_args.args = "   ".into();
        let entry = EmulatorEntry {
            name: "PCSX2".into(),
            args: "%rom%".into(),
            ..Default::default()
        };
        let filled = apply_manual_emulator_profile_defaults(&entry, &blank_args);
        assert_eq!(filled.args, "%rom%");
    }

    // --- backfill_missing_defaults ------------------------------------------

    fn ppsspp_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "PPSSPP".into(),
            match_tokens: strings(&["ppsspp"]),
            args: "%rom%".into(),
            platform_keywords: strings(&["playstation portable"]),
            ..Default::default()
        }
    }

    fn retroarch_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "RetroArch (Multi-System)".into(),
            match_tokens: strings(&["retroarch"]),
            args: "-L %core% %rom%".into(),
            all_platforms: true,
            ..Default::default()
        }
    }

    fn backfill(
        config: &mut Config,
        profiles: &[EmulatorProfile],
        platforms: &[String],
        installed_cores: &dyn Fn(&str, &str) -> Vec<String>,
    ) -> bool {
        let ctx = DefaultsContext {
            platforms,
            installed_cores,
            is_retroarch: &is_retroarch_name,
        };
        backfill_missing_defaults(config, profiles, &ctx)
    }

    #[test]
    fn backfill_is_a_no_op_when_nothing_is_missing() {
        // Mirrors tests/test_emulator_autoconfig_settings.py:3022
        // (test_backfill_does_not_overwrite_existing_default).
        let mut config = Config {
            emulators: vec![EmulatorEntry {
                name: "PPSSPP".into(),
                path: "/usr/bin/ppsspp".into(),
                ..Default::default()
            }],
            default_emulators: map(&[("PlayStation Portable", "AnotherEmulator")]),
            ..Default::default()
        };
        let platforms = strings(&["PlayStation Portable"]);
        let changed = backfill(&mut config, &[ppsspp_profile()], &platforms, &no_cores);

        assert!(!changed, "nothing changed, so the caller must not save");
        assert_eq!(
            config.default_emulators.get("PlayStation Portable"),
            Some(&"AnotherEmulator".to_string())
        );
    }

    #[test]
    fn backfill_fills_a_platform_whose_cores_appeared_after_install() {
        // Mirrors tests/test_emulator_autoconfig_settings.py:3009
        // (test_backfill_assigns_default_for_emulator_with_no_default): the
        // install left no defaults behind, and the RetroArch cores only
        // became discoverable later.
        let mut config = Config {
            emulators: vec![EmulatorEntry {
                name: "RetroArch (Multi-System)".into(),
                path: "/x/retroarch".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let platforms = strings(&["Super Nintendo", "Sega Saturn"]);
        let cores = |platform: &str, _: &str| {
            if platform == "Super Nintendo" {
                strings(&["snes9x"])
            } else {
                Vec::new()
            }
        };
        let changed = backfill(&mut config, &[retroarch_profile()], &platforms, &cores);

        assert!(changed);
        assert_eq!(
            config.default_emulators,
            map(&[("Super Nintendo", "RetroArch (Multi-System)")])
        );
        assert_eq!(config.retroarch_cores, map(&[("Super Nintendo", "snes9x")]));
    }

    #[test]
    fn backfill_accumulates_across_entries() {
        let mut config = Config {
            emulators: vec![
                EmulatorEntry {
                    name: "  ".into(),
                    path: "/x/blank".into(),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "Unmatched Emulator".into(),
                    path: "/x/unmatched".into(),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "PPSSPP".into(),
                    path: "/usr/bin/ppsspp".into(),
                    ..Default::default()
                },
                EmulatorEntry {
                    name: "RetroArch (Multi-System)".into(),
                    path: "/x/retroarch".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let platforms = strings(&["PlayStation Portable", "Super Nintendo"]);
        let cores = |_: &str, _: &str| strings(&["some_core"]);
        let profiles = vec![ppsspp_profile(), retroarch_profile()];
        let changed = backfill(&mut config, &profiles, &platforms, &cores);

        assert!(changed);
        // PPSSPP claimed PSP first; RetroArch must not displace it, but it
        // does claim the platform still free.
        assert_eq!(
            config.default_emulators,
            map(&[
                ("PlayStation Portable", "PPSSPP"),
                ("Super Nintendo", "RetroArch (Multi-System)"),
            ])
        );
        assert_eq!(
            config.retroarch_cores,
            map(&[("Super Nintendo", "some_core")])
        );
    }
}
