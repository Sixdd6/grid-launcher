//! Platform predicates (`grid_launcher/emulator/selection.py:11-52`).

use regex::Regex;
use std::sync::OnceLock;

/// A game is "native" when its server platform runs host executables:
///
/// 1. the RomM platform slug (`platform_slug` on a ROM, `slug` on a
///    platform), trimmed and casefolded, is `win`/`linux` or starts with
///    `win`/`linux` (`win`, `win9x`, `linux`, …). The slug is the platform's
///    identity: a custom display name ("PC Fangames & Ports") keeps the slug
///    `win`. A platform's `fs_slug` is never read — it is a folder name
///    (`win-fangames-ports`), not an identity.
/// 2. else (fallback: the rule before 2026-10-10, kept so a blank slug — a
///    row not yet backfilled, an offline caller — keeps today's native set)
///    the platform NAME, trimmed and casefolded, starts with `windows` or
///    `linux` (widening `is_native_executable_platform`, selection.py:145).
///
/// Both halves read the *server* platform, never the host OS.
///
/// User ruling 2026-09-08: linux platforms are native everywhere — install
/// layout and block reason, launch routing, and cloud save scope/block
/// reasons all use this one predicate. Only the compat-tool decision is
/// narrower; that reads [`is_windows_platform`]. User bug 2026-10-10: the
/// slug half exists because a custom-named Windows platform was treated as
/// an emulated one.
pub fn is_native_platform(platform_slug: &str, platform: &str) -> bool {
    let slug = platform_slug.trim().to_lowercase();
    if slug.starts_with("win") || slug.starts_with("linux") {
        return true;
    }
    let folded = platform.trim().to_lowercase();
    folded.starts_with("windows") || folded.starts_with("linux")
}

/// Whether the server platform is a Windows one: its slug, trimmed and
/// casefolded, starts with `win`, or (fallback) its name starts with
/// `windows`. This is the narrower half of [`is_native_platform`] and exists
/// for exactly one decision: whether a native game needs a Wine/Proton
/// compat tool. A Linux platform row is native but runs its own executable
/// directly.
pub fn is_windows_platform(platform_slug: &str, platform: &str) -> bool {
    platform_slug.trim().to_lowercase().starts_with("win")
        || platform.trim().to_lowercase().starts_with("windows")
}

/// Whether `platform`, trimmed and casefolded, is exactly "playstation 3" or
/// "ps3".
pub fn is_ps3_platform(platform: &str) -> bool {
    matches!(
        platform.trim().to_lowercase().as_str(),
        "playstation 3" | "ps3"
    )
}

/// Whether `platform` names DOS: "DOS", "MS-DOS", "PC DOS", or any name
/// carrying `dos` / `msdos` as a whole token. Token-based on purpose so a
/// name that merely contains the letters does not match. A DOS game's
/// archive is the ROM itself: the DOSBox cores mount the zip directly, so
/// it is never extracted and always launched as the archive.
pub fn is_dos_platform(platform: &str) -> bool {
    let (normalized, _compact, tokens) = normalized_tokens(platform);
    if normalized.is_empty() {
        return false;
    }
    tokens.iter().any(|t| t == "dos" || t == "msdos")
}

/// Splits `platform` into a normalized (lowercased, non-alphanumeric runs
/// collapsed to single spaces, trimmed) form, its space-free "compact" form,
/// and the whitespace-separated tokens of the normalized form.
fn normalized_tokens(platform: &str) -> (String, String, Vec<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[^a-z0-9]+").unwrap());
    let lowered = platform.trim().to_lowercase();
    let normalized = re.replace_all(&lowered, " ").trim().to_string();
    let compact = normalized.replace(' ', "");
    let tokens = normalized.split_whitespace().map(str::to_string).collect();
    (normalized, compact, tokens)
}

/// Whether `platform` names a PlayStation 4, matching "playstation 4", "ps4"
/// (as a whole token), or a compact "playstation4" run.
pub fn is_ps4_platform(platform: &str) -> bool {
    let (normalized, compact, tokens) = normalized_tokens(platform);
    if normalized.is_empty() {
        return false;
    }
    if normalized == "playstation 4" || normalized == "ps4" {
        return true;
    }
    if tokens.iter().any(|t| t == "ps4") {
        return true;
    }
    compact.contains("playstation4")
}

/// Whether `platform` names a PlayStation 5, matching "playstation 5", "ps5"
/// (as a whole token), or a compact "playstation5" run — the same rule as
/// [`is_ps4_platform`].
pub fn is_ps5_platform(platform: &str) -> bool {
    let (normalized, compact, tokens) = normalized_tokens(platform);
    if normalized.is_empty() {
        return false;
    }
    if normalized == "playstation 5" || normalized == "ps5" {
        return true;
    }
    if tokens.iter().any(|t| t == "ps5") {
        return true;
    }
    compact.contains("playstation5")
}

/// Whether `platform` names an Xbox 360: it must mention "xbox" (as a token
/// or within a compact "xbox360" run) and separately carry a "360" marker.
pub fn is_xbox360_platform(platform: &str) -> bool {
    let (normalized, compact, tokens) = normalized_tokens(platform);
    if normalized.is_empty() {
        return false;
    }
    let has_xbox = tokens.iter().any(|t| t == "xbox") || compact.contains("xbox360");
    if !has_xbox {
        return false;
    }
    compact.contains("xbox360") || tokens.iter().any(|t| t == "360")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_platform_matches_dos_names_by_token() {
        for platform in ["DOS", "dos", " MS-DOS ", "PC DOS", "MSDOS", "IBM PC (DOS)"] {
            assert!(is_dos_platform(platform), "{platform} should be DOS");
        }
    }

    #[test]
    fn dos_platform_false_for_names_that_only_contain_the_letters() {
        for platform in ["Windows", "Nintendo DS", "Dosukoi", "Amiga CDOS", "", "  "] {
            assert!(!is_dos_platform(platform), "{platform} should not be DOS");
        }
    }

    #[test]
    fn native_platform_name_fallback_matches_windows_case_and_whitespace_insensitively() {
        assert!(is_native_platform("", "Windows"));
        assert!(is_native_platform("", " windows 10"));
        assert!(!is_native_platform("", "Nintendo Wii"));
        assert!(!is_native_platform("", ""));
    }

    #[test]
    fn native_platform_name_fallback_also_matches_linux() {
        assert!(is_native_platform("", "Linux"));
        assert!(is_native_platform("", " linux x86_64"));
        assert!(!is_native_platform("", "Nintendo Linux"));
    }

    /// User bug 2026-10-10: RomM platform 52 is slug `win`, fs_slug
    /// `win-fangames-ports`, display name "PC Fangames & Ports".
    #[test]
    fn native_platform_table_follows_the_slug() {
        let cases: &[(&str, &str, bool)] = &[
            ("win", "Windows", true),
            ("win", "PC Fangames & Ports", true),
            (" WIN ", "My PC Shelf", true),
            ("win9x", "Windows 9x", true),
            ("win9x", "Old PC", true),
            ("linux", "Linux", true),
            ("linux", "Steam Deck Ports", true),
            ("dos", "DOS", false),
            ("dos", "PC (DOS)", false),
            ("snes", "Super Nintendo", false),
            // The fs_slug is never the identity: seen as a name it is not
            // native, and the real slug decides.
            ("snes", "win-fangames-ports", false),
            // Name fallback: no slug, or a slug that does not decide.
            ("", "Windows", true),
            ("", "PC Fangames & Ports", false),
            ("pc", "Windows Games", true),
        ];
        for &(slug, name, expected) in cases {
            assert_eq!(
                is_native_platform(slug, name),
                expected,
                "slug {slug:?} name {name:?}"
            );
        }
    }

    #[test]
    fn windows_platform_excludes_linux() {
        assert!(is_windows_platform("", "Windows"));
        assert!(is_windows_platform("", "  windows 10 "));
        assert!(!is_windows_platform("", "Linux"));
        assert!(!is_windows_platform("", ""));
    }

    #[test]
    fn windows_platform_follows_the_slug() {
        assert!(is_windows_platform("win", "PC Fangames & Ports"));
        assert!(is_windows_platform("win9x", "Old PC"));
        assert!(!is_windows_platform("linux", "Steam Deck Ports"));
        assert!(!is_windows_platform("dos", "PC (DOS)"));
    }

    #[test]
    fn ps3_platform_matches_exact_names_only() {
        assert!(is_ps3_platform("PlayStation 3"));
        assert!(is_ps3_platform("PS3"));
        assert!(!is_ps3_platform("Sony PlayStation 3"));
        assert!(!is_ps3_platform(""));
    }

    #[test]
    fn ps4_platform_matches_common_spellings() {
        assert!(is_ps4_platform("PlayStation 4"));
        assert!(is_ps4_platform("Sony PS4"));
        assert!(is_ps4_platform("PlayStation4"));
        assert!(!is_ps4_platform("PlayStation 3"));
        assert!(!is_ps4_platform(""));
    }

    #[test]
    fn ps5_platform_matches_common_spellings() {
        assert!(is_ps5_platform("PlayStation 5"));
        assert!(is_ps5_platform("Sony PS5"));
        assert!(is_ps5_platform("PlayStation5"));
        assert!(is_ps5_platform("PS5 Pro"));
        assert!(!is_ps5_platform("PlayStation 4"));
        assert!(!is_ps5_platform("PS4"));
        assert!(!is_ps5_platform(""));
    }

    #[test]
    fn xbox360_platform_matches_common_spellings() {
        assert!(is_xbox360_platform("Xbox 360"));
        assert!(is_xbox360_platform("Microsoft Xbox360"));
        assert!(!is_xbox360_platform("Xbox"));
        assert!(!is_xbox360_platform("Xbox One"));
        assert!(!is_xbox360_platform(""));
    }
}
