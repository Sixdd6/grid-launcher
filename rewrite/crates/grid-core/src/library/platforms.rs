//! Platform predicates (`grid_launcher/emulator/selection.py:11-52`).

use regex::Regex;
use std::sync::OnceLock;

/// A game is "native" when its platform, trimmed and casefolded, starts with
/// `windows` (`is_native_executable_platform`, selection.py:145). This is the
/// *server* platform name, not the host OS.
pub fn is_native_platform(platform: &str) -> bool {
    platform.trim().to_lowercase().starts_with("windows")
}

/// Whether `platform`, trimmed and casefolded, is exactly "playstation 3" or
/// "ps3".
pub fn is_ps3_platform(platform: &str) -> bool {
    matches!(
        platform.trim().to_lowercase().as_str(),
        "playstation 3" | "ps3"
    )
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
    fn native_platform_matches_windows_case_and_whitespace_insensitively() {
        assert!(is_native_platform("Windows"));
        assert!(is_native_platform(" windows 10"));
        assert!(!is_native_platform("Nintendo Wii"));
        assert!(!is_native_platform(""));
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
    fn xbox360_platform_matches_common_spellings() {
        assert!(is_xbox360_platform("Xbox 360"));
        assert!(is_xbox360_platform("Microsoft Xbox360"));
        assert!(!is_xbox360_platform("Xbox"));
        assert!(!is_xbox360_platform("Xbox One"));
        assert!(!is_xbox360_platform(""));
    }
}
