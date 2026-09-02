//! Emulator autoprofiles: the embedded `emulator-autoprofiles.json` catalog,
//! matching an `EmulatorEntry` to a profile, and the platform/keyword
//! matcher used to decide which platforms an emulator supports. Ports
//! `grid_launcher/emulator/profiles.py` (see module doc there) — profile
//! matching is `emulator_profile_for_entry` (profiles.py:192) restricted to
//! non-compat-tool profiles, and `platform_matches_keywords` is
//! `matching_platforms_for_emulator_keywords` (profiles.py:67) restricted to
//! a single platform. See `docs/porting/04-emulator-launch.md` §3 and §4.

use std::collections::HashSet;
use std::sync::OnceLock;

/// One entry from `emulator-autoprofiles.json`, normalized at load time.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EmulatorProfile {
    pub name: String,
    pub match_tokens: Vec<String>,
    pub args: String,
    pub all_platforms: bool,
    pub platform_keywords: Vec<String>,
    pub is_compat_tool: bool,
    /// The catalog entry's raw `source` block, copied through untouched by
    /// [`normalize_one`] — read by `launch::catalog` to build the "install
    /// from catalog" listing. Never sent over IPC: existing auto-fill
    /// payloads must not change shape.
    #[serde(skip_serializing)]
    pub source: Option<serde_json::Value>,
}

/// Emulator autoprofile slugs that ship a Windows-only build and therefore
/// must not appear in the UI on non-Windows platforms
/// (`_WINDOWS_ONLY_EMULATOR_SLUGS`, profiles.py:13).
pub const WINDOWS_ONLY_SLUGS: [&str; 3] = [
    "xenia canary (xbox 360)",
    "xenia (xbox 360)",
    "shadps4 qt launcher",
];

const AUTOPROFILES_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../emulator-autoprofiles.json"
));

static PROFILES: OnceLock<Vec<EmulatorProfile>> = OnceLock::new();

/// The subset of an autoprofile JSON entry's fields this crate reads.
/// Fields the catalog carries but this crate does not use (`source`,
/// `save_strategy`, `save_directories`, ...) are ignored by serde rather
/// than rejected.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct RawProfile {
    #[serde(default)]
    name: String,
    #[serde(default)]
    match_tokens: Vec<String>,
    #[serde(default)]
    args: String,
    #[serde(default)]
    all_platforms: bool,
    #[serde(default)]
    platform_keywords: Vec<String>,
    #[serde(default)]
    is_compat_tool: bool,
    #[serde(default)]
    source: Option<serde_json::Value>,
}

/// The parsed, normalized autoprofile catalog, embedded at build time and
/// parsed once. Panics on malformed JSON — the file is a build-time asset
/// covered by [`embedded_json_parses_and_is_non_empty`](tests), so this
/// path is unreachable in a passing CI run.
pub fn load_profiles() -> &'static [EmulatorProfile] {
    PROFILES.get_or_init(|| {
        let raw: Vec<RawProfile> = serde_json::from_str(AUTOPROFILES_JSON)
            .expect("emulator-autoprofiles.json is embedded at build time and must be valid JSON");
        raw.into_iter().filter_map(normalize_one).collect()
    })
}

/// Normalizes one raw catalog entry (profiles.py:427
/// `normalize_emulator_autoprofiles`, restricted to the fields this crate
/// reads): a blank name or an entry with neither tokens nor
/// `is_compat_tool` is dropped; tokens are trimmed, casefolded and blank
/// ones dropped; blank args become `"%rom%"`; keywords are trimmed and
/// blank ones dropped, case kept as-is (matching is applied later).
fn normalize_one(raw: RawProfile) -> Option<EmulatorProfile> {
    let name = raw.name.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let match_tokens: Vec<String> = raw
        .match_tokens
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();
    if match_tokens.is_empty() && !raw.is_compat_tool {
        return None;
    }

    let args_trimmed = raw.args.trim();
    let args = if args_trimmed.is_empty() {
        "%rom%".to_string()
    } else {
        args_trimmed.to_string()
    };

    let platform_keywords: Vec<String> = raw
        .platform_keywords
        .iter()
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
        .collect();

    Some(EmulatorProfile {
        name,
        match_tokens,
        args,
        all_platforms: raw.all_platforms,
        platform_keywords,
        is_compat_tool: raw.is_compat_tool,
        source: raw.source,
    })
}

/// Splits `raw` on the last `/` or `\`, whichever comes later, regardless
/// of host OS — an emulator path saved on Windows must still resolve to
/// the right profile when the config is read on Linux.
fn windows_tolerant_basename(raw: &str) -> &str {
    raw.rsplit(['/', '\\']).next().unwrap_or("")
}

/// Everything before the last `.`, or the whole string when there is none.
fn stem_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(idx) => &name[..idx],
        None => name,
    }
}

/// `pattern` against `text`, `*` matching any run of characters (including
/// none) and `?` matching exactly one — the two `fnmatch.fnmatchcase`
/// wildcards `match_tokens` entries use (profiles.py:178). Both arguments
/// are expected to already be casefolded, so the comparison itself is
/// plain character equality.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut star_match = 0usize;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            star_match = ti;
            pi += 1;
        } else if let Some(star_pi) = star {
            pi = star_pi + 1;
            star_match += 1;
            ti = star_match;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// A `match_tokens` entry against an executable's already-casefolded
/// basename (`_match_token_matches_executable`, profiles.py:178): a token
/// containing `*` or `?` is matched as a glob, everything else by exact
/// equality.
fn token_matches_executable(token: &str, executable_name: &str) -> bool {
    if token.is_empty() || executable_name.is_empty() {
        return false;
    }
    if token.contains('*') || token.contains('?') {
        glob_match(token, executable_name)
    } else {
        token == executable_name
    }
}

/// The profile an `EmulatorEntry` resolves to (`emulator_profile_for_entry`,
/// profiles.py:192; doc 04 §3). Profiles are walked in file order; for each
/// one (compat-tool profiles skipped outright) the entry name is compared
/// against the profile name, then the executable's basename against every
/// token (glob-aware), then the executable's stem against every token's
/// stem — the first stage that matches, on the first profile it matches on,
/// wins.
pub fn profile_for_entry<'a>(
    entry_name: &str,
    exe_path: &str,
    profiles: &'a [EmulatorProfile],
) -> Option<&'a EmulatorProfile> {
    let name = entry_name.trim().to_lowercase();
    let executable_name = windows_tolerant_basename(exe_path).trim().to_lowercase();
    let executable_stem = if executable_name.is_empty() {
        String::new()
    } else {
        stem_of(&executable_name).to_string()
    };

    for profile in profiles {
        if profile.is_compat_tool {
            continue;
        }

        let profile_name = profile.name.to_lowercase();
        if !name.is_empty() && profile_name == name {
            return Some(profile);
        }

        if profile.match_tokens.is_empty() {
            continue;
        }
        if !executable_name.is_empty()
            && profile
                .match_tokens
                .iter()
                .any(|token| token_matches_executable(token, &executable_name))
        {
            return Some(profile);
        }
        if !executable_stem.is_empty()
            && profile
                .match_tokens
                .iter()
                .any(|token| stem_of(token) == executable_stem)
        {
            return Some(profile);
        }
    }
    None
}

/// Whether `t` is a run of one or more ASCII digits (`str.isdigit`, applied
/// to a token built only from `[A-Za-z0-9]` runs).
fn is_digit_token(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
}

/// Whether `t` is a run of one or more ASCII letters (`str.isalpha`, same
/// caveat as [`is_digit_token`]).
fn is_alpha_token(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_alphabetic())
}

/// The maximal runs of ASCII letters/digits in `value`, in order — the
/// `[A-Za-z0-9]+` regex matches of `token_set` (profiles.py:73).
fn ascii_alnum_chunks(value: &str) -> Vec<Vec<char>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            current.push(c);
        } else if !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Splits one alnum chunk at a letter→digit, digit→letter or lower→upper
/// boundary (the regex substitution in `token_set`, profiles.py:78-82) —
/// e.g. `PlayStation4` -> `["Play", "Station", "4"]`.
fn split_at_boundaries(chunk: &[char]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for (i, &c) in chunk.iter().enumerate() {
        if i > 0 {
            let prev = chunk[i - 1];
            let boundary = (prev.is_ascii_alphabetic() && c.is_ascii_digit())
                || (prev.is_ascii_digit() && c.is_ascii_alphabetic())
                || (prev.is_ascii_lowercase() && c.is_ascii_uppercase());
            if boundary {
                parts.push(std::mem::take(&mut current));
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// The token set of `value` (`token_set`, profiles.py:71): for every
/// `[A-Za-z0-9]+` run, the casefolded run itself, each casefolded
/// boundary-split part, and the concatenation of the run's alphabetic parts
/// (also casefolded) — the last of these is what makes `"GameCube"` yield
/// `"gamecube"` even though the whole-run token already covers that case;
/// its purpose is chunks that mix letters and digits, e.g. `"PlayStation4"`
/// also yielding `"playstation"`.
fn token_set(value: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for chunk in ascii_alnum_chunks(value) {
        let whole: String = chunk.iter().collect::<String>().to_lowercase();
        if !whole.is_empty() {
            tokens.insert(whole);
        }

        let parts = split_at_boundaries(&chunk);
        let folded_parts: Vec<String> = parts.iter().map(|p| p.to_lowercase()).collect();
        for part in &folded_parts {
            tokens.insert(part.clone());
        }

        let compact_alpha: String = folded_parts
            .iter()
            .filter(|p| is_alpha_token(p))
            .cloned()
            .collect();
        if !compact_alpha.is_empty() {
            tokens.insert(compact_alpha);
        }
    }
    tokens
}

/// Byte offsets where non-overlapping occurrences of `needle` start in
/// `haystack`, scanning left to right (matches `re.finditer` on a literal,
/// escaped pattern).
fn match_start_indices(haystack: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    haystack
        .match_indices(needle)
        .map(|(start, _)| start)
        .collect()
}

/// Whether `platform` matches any of `keywords`
/// (`matching_platforms_for_emulator_keywords`, profiles.py:67; doc 04 §3),
/// restricted to a single platform: a keyword matches when its token set is
/// non-empty and a subset of the platform's, no *extra* platform token is
/// purely numeric unless the keyword itself has a numeric token (the
/// "Xbox360" vs "xbox" guard), and no extra *alphabetic* platform token
/// occurs at or after the end of every occurrence of the keyword's
/// alphabetic tokens (the "Playstation Portable" vs "playstation" guard —
/// an extra token appearing strictly *before* the keyword, e.g. "Sony"
/// before "PlayStation", is allowed).
pub fn platform_matches_keywords(platform: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let platform_tokens = token_set(platform);
    if platform_tokens.is_empty() {
        return false;
    }
    let normalized_platform = platform.to_lowercase();

    for keyword in keywords {
        let keyword_tokens = token_set(keyword.trim());
        if keyword_tokens.is_empty() {
            continue;
        }
        if !keyword_tokens.is_subset(&platform_tokens) {
            continue;
        }

        let extra_tokens: Vec<&String> = platform_tokens.difference(&keyword_tokens).collect();
        let keyword_has_numeric = keyword_tokens.iter().any(|t| is_digit_token(t));
        let extra_has_numeric = extra_tokens.iter().any(|t| is_digit_token(t));
        if extra_has_numeric && !keyword_has_numeric {
            continue;
        }

        let extra_alpha_tokens: Vec<&&String> =
            extra_tokens.iter().filter(|t| is_alpha_token(t)).collect();
        if !extra_alpha_tokens.is_empty() {
            let keyword_end = keyword_tokens
                .iter()
                .filter(|t| is_alpha_token(t))
                .flat_map(|kw_tok| {
                    normalized_platform
                        .match_indices(kw_tok.as_str())
                        .map(|(start, _)| start + kw_tok.len())
                })
                .max()
                .unwrap_or(0);
            let blocked = extra_alpha_tokens.iter().any(|extra| {
                match_start_indices(&normalized_platform, extra)
                    .into_iter()
                    .any(|start| start >= keyword_end)
            });
            if blocked {
                continue;
            }
        }

        return true;
    }
    false
}

/// `profiles` filtered for display: compat-tool profiles are always
/// dropped; [`WINDOWS_ONLY_SLUGS`] profiles are additionally dropped when
/// `windows` is false (doc 04 §4, restricted to the name-based gate — the
/// full `source.platforms` allowlist is a later task's concern).
fn visible_profiles_for(profiles: &[EmulatorProfile], windows: bool) -> Vec<&EmulatorProfile> {
    profiles
        .iter()
        .filter(|p| {
            if p.is_compat_tool {
                return false;
            }
            if !windows {
                let name = p.name.to_lowercase();
                if WINDOWS_ONLY_SLUGS.contains(&name.as_str()) {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// [`visible_profiles_for`] using this build's actual target OS.
pub fn visible_profiles(profiles: &[EmulatorProfile]) -> Vec<&EmulatorProfile> {
    visible_profiles_for(profiles, cfg!(windows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, tokens: &[&str], compat: bool) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            match_tokens: tokens.iter().map(|t| t.to_lowercase()).collect(),
            args: "%rom%".to_string(),
            all_platforms: false,
            platform_keywords: vec![],
            is_compat_tool: compat,
            source: None,
        }
    }

    // --- load_profiles: embedded JSON --------------------------------------

    #[test]
    fn embedded_json_parses_and_is_non_empty() {
        let profiles = load_profiles();
        assert!(!profiles.is_empty());
    }

    #[test]
    fn embedded_json_has_at_least_one_compat_tool() {
        let profiles = load_profiles();
        assert!(profiles.iter().any(|p| p.is_compat_tool));
    }

    #[test]
    fn embedded_tokens_are_casefolded() {
        let profiles = load_profiles();
        for profile in profiles {
            for token in &profile.match_tokens {
                assert_eq!(token, &token.to_lowercase());
            }
        }
    }

    #[test]
    fn embedded_profiles_carry_source_when_present_and_none_when_absent() {
        let profiles = load_profiles();
        let pico8 = profiles.iter().find(|p| p.name == "Pico-8").unwrap();
        assert_eq!(pico8.source, None);
        let pcsx2 = profiles
            .iter()
            .find(|p| p.name == "PCSX2 (Playstation 2)")
            .unwrap();
        assert!(pcsx2.source.as_ref().unwrap().is_object());
    }

    #[test]
    fn embedded_blank_args_default_to_rom_placeholder() {
        let profiles = load_profiles();
        for profile in profiles {
            assert!(!profile.args.trim().is_empty());
        }
    }

    // --- profile_for_entry: matching order ----------------------------------

    #[test]
    fn name_match_wins_when_reached_first_in_file_order() {
        let profiles = load_profiles();
        let found = profile_for_entry("Pico-8", "", profiles);
        assert_eq!(found.map(|p| p.name.as_str()), Some("Pico-8"));
    }

    #[test]
    fn glob_token_matches_versioned_appimage_basename() {
        let profiles = load_profiles();
        let found = profile_for_entry("", "/x/RetroArch-1.19.1-x86_64.AppImage", profiles);
        assert_eq!(
            found.map(|p| p.name.as_str()),
            Some("RetroArch (Multi-System)")
        );
    }

    #[test]
    fn stem_match_when_basename_has_no_extension() {
        let profiles = load_profiles();
        let found = profile_for_entry("", "/x/PCSX2-QT", profiles);
        assert_eq!(
            found.map(|p| p.name.as_str()),
            Some("PCSX2 (Playstation 2)")
        );
    }

    #[test]
    fn basename_split_is_windows_tolerant_even_on_this_host() {
        // Deliberate deviation from profiles.py:199, which parses the
        // executable path with plain `Path(...)`: on a non-Windows host
        // that only splits on `/`, so this exact backslash-only input would
        // NOT resolve there. The Rust port always tolerates both
        // separators (task-2-brief.md), which is what this asserts.
        let profiles = load_profiles();
        let found = profile_for_entry("", r"C:\Emulators\pcsx2\pcsx2-qt.exe", profiles);
        assert_eq!(
            found.map(|p| p.name.as_str()),
            Some("PCSX2 (Playstation 2)")
        );
    }

    #[test]
    fn compat_tool_is_skipped_even_on_exact_name_match() {
        // Deliberate deviation from profiles.py:192 (task-2-brief.md): the
        // Python reference has no is_compat_tool check at all here and would
        // return the GE-Proton profile for this input. Compat-tool selection
        // is handled by a separate flow, so this port excludes them outright.
        let profiles = load_profiles();
        let found = profile_for_entry("GE-Proton", "", profiles);
        assert_eq!(found, None);
    }

    #[test]
    fn no_match_returns_none() {
        let profiles = load_profiles();
        let found = profile_for_entry("", "/x/unknown-tool", profiles);
        assert_eq!(found, None);
    }

    #[test]
    fn earlier_profile_token_stage_wins_over_a_later_profile_name_match() {
        // Verified against the Python reference: emulator_profile_for_entry
        // walks profiles in file order, checking every stage of one profile
        // before moving to the next — an earlier profile's token match wins
        // even when a later profile's name would also have matched.
        let profiles = vec![
            profile("RetroArch (Multi-System)", &["retroarch.exe"], false),
            profile("PCSX2 (Playstation 2)", &[], false),
        ];
        let found = profile_for_entry("PCSX2 (Playstation 2)", "/x/retroarch.exe", &profiles);
        assert_eq!(
            found.map(|p| p.name.as_str()),
            Some("RetroArch (Multi-System)")
        );
    }

    // --- visible_profiles ----------------------------------------------------

    #[test]
    fn visible_profiles_always_excludes_compat_tools() {
        let profiles = load_profiles();
        let visible = visible_profiles(profiles);
        assert!(visible.iter().all(|p| !p.is_compat_tool));
    }

    #[test]
    fn visible_profiles_for_drops_windows_only_slugs_off_windows() {
        let profiles = vec![
            profile("Xenia (Xbox 360)", &[], false),
            profile("Redream (Sega Dreamcast)", &["redream.exe"], false),
        ];
        let visible = visible_profiles_for(&profiles, false);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "Redream (Sega Dreamcast)");
    }

    #[test]
    fn visible_profiles_for_keeps_windows_only_slugs_on_windows() {
        let profiles = vec![profile("Xenia (Xbox 360)", &[], false)];
        let visible = visible_profiles_for(&profiles, true);
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn visible_profiles_for_drops_compat_tools_on_either_platform() {
        let profiles = vec![profile("GE-Proton", &[], true)];
        assert!(visible_profiles_for(&profiles, true).is_empty());
        assert!(visible_profiles_for(&profiles, false).is_empty());
    }

    // --- normalization ---------------------------------------------------------

    fn raw(name: &str, tokens: &[&str], args: &str, compat: bool, keywords: &[&str]) -> RawProfile {
        RawProfile {
            name: name.to_string(),
            match_tokens: tokens.iter().map(|t| t.to_string()).collect(),
            args: args.to_string(),
            all_platforms: false,
            platform_keywords: keywords.iter().map(|k| k.to_string()).collect(),
            is_compat_tool: compat,
            source: None,
        }
    }

    #[test]
    fn normalize_drops_blank_name() {
        assert!(normalize_one(raw("  ", &["x.exe"], "", false, &[])).is_none());
    }

    #[test]
    fn normalize_drops_entries_with_no_tokens_and_not_compat() {
        assert!(normalize_one(raw("Name", &[], "", false, &[])).is_none());
    }

    #[test]
    fn normalize_keeps_compat_tool_with_no_tokens() {
        assert!(normalize_one(raw("GE-Proton", &[], "", true, &[])).is_some());
    }

    #[test]
    fn normalize_defaults_blank_args_to_rom_placeholder() {
        let profile = normalize_one(raw("Name", &["x.exe"], "  ", false, &[])).unwrap();
        assert_eq!(profile.args, "%rom%");
    }

    #[test]
    fn normalize_keeps_a_non_blank_args_value() {
        let profile =
            normalize_one(raw("Name", &["x.exe"], " -L \"%core%\" ", false, &[])).unwrap();
        assert_eq!(profile.args, "-L \"%core%\"");
    }

    #[test]
    fn normalize_copies_source_through_untouched() {
        let mut entry = raw("Name", &["x.exe"], "", false, &[]);
        entry.source = Some(serde_json::json!({"provider": "github", "owner": "o", "repo": "r"}));
        let expected = entry.source.clone();
        let profile = normalize_one(entry).unwrap();
        assert_eq!(profile.source, expected);
    }

    #[test]
    fn normalize_source_defaults_to_none_when_absent() {
        let profile = normalize_one(raw("Name", &["x.exe"], "", false, &[])).unwrap();
        assert_eq!(profile.source, None);
    }

    #[test]
    fn normalize_casefolds_and_trims_tokens_dropping_blanks() {
        let profile = normalize_one(raw("Name", &["  X.EXE", "", "  "], "", false, &[])).unwrap();
        assert_eq!(profile.match_tokens, vec!["x.exe".to_string()]);
    }

    #[test]
    fn normalize_trims_keywords_dropping_blanks_but_keeps_case() {
        let profile = normalize_one(raw(
            "Name",
            &["x.exe"],
            "",
            false,
            &["  PlayStation 2  ", ""],
        ))
        .unwrap();
        assert_eq!(profile.platform_keywords, vec!["PlayStation 2".to_string()]);
    }

    // --- platform_matches_keywords: verified against the Python oracle -----

    #[test]
    fn keyword_matcher_table() {
        let cases: &[(&str, &str, bool)] = &[
            ("PlayStation 2", "playstation 2", true),
            ("PlayStation", "playstation 2", false),
            ("PlayStation 3", "playstation 2", false),
            ("Nintendo GameCube", "gamecube", true),
            ("Nintendo 64DD", "nintendo 64", false),
            ("Nintendo 64", "nintendo 64", true),
            ("GameCube", "cube", true),
            ("GameCube", "game", false),
            ("Sony PlayStation4", "playstation 4", true),
            ("PlayStation4", "playstation 4", true),
            ("Xbox360", "playstation 4", false),
            ("Xbox360", "xbox", false),
            ("WiiU", "wii u", true),
            ("Xbox360", "xbox 360", true),
            ("Playstation Portable", "playstation", false),
            ("Sony PlayStation", "playstation", true),
            ("Super Nintendo Entertainment System", "snes", false),
            ("SNES", "snes", true),
        ];
        for (platform, keyword, expected) in cases {
            let keywords = vec![keyword.to_string()];
            assert_eq!(
                platform_matches_keywords(platform, &keywords),
                *expected,
                "platform={platform:?} keyword={keyword:?}"
            );
        }
    }

    #[test]
    fn empty_keywords_never_match() {
        assert!(!platform_matches_keywords("PlayStation 2", &[]));
    }

    #[test]
    fn any_keyword_in_the_list_matching_is_enough() {
        let keywords = vec!["nope".to_string(), "playstation 2".to_string()];
        assert!(platform_matches_keywords("PlayStation 2", &keywords));
    }

    // --- glob_match: the */? mini-matcher --------------------------------------

    #[test]
    fn glob_match_star_matches_zero_or_more_chars() {
        assert!(glob_match("retroarch*.appimage", "retroarch.appimage"));
        assert!(glob_match(
            "retroarch*.appimage",
            "retroarch-1.19.1-x86_64.appimage"
        ));
        assert!(!glob_match("retroarch*.appimage", "retroarch.zip"));
    }

    #[test]
    fn glob_match_question_mark_matches_exactly_one_char() {
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
        assert!(!glob_match("a?c", "abbc"));
    }

    #[test]
    fn glob_match_is_case_sensitive_over_already_lowered_strings() {
        assert!(!glob_match("retroarch*.appimage", "RetroArch.AppImage"));
    }

    // --- basename / stem helpers -------------------------------------------

    #[test]
    fn windows_tolerant_basename_splits_on_forward_and_back_slash() {
        assert_eq!(windows_tolerant_basename("a/b/c.exe"), "c.exe");
        assert_eq!(windows_tolerant_basename(r"a\b\c.exe"), "c.exe");
        assert_eq!(windows_tolerant_basename(r"a/b\c.exe"), "c.exe");
        assert_eq!(windows_tolerant_basename("c.exe"), "c.exe");
    }

    #[test]
    fn stem_of_takes_everything_before_the_last_dot() {
        assert_eq!(stem_of("pcsx2-qt.exe"), "pcsx2-qt");
        assert_eq!(stem_of("archive.tar.gz"), "archive.tar");
        assert_eq!(stem_of("no-extension"), "no-extension");
    }
}
