//! Server-update detection (`grid_launcher/library/update_detection.py`,
//! docs/porting/10-identity-updates.md "Update detection flow"). Pure: no
//! I/O, no clock. The app layer feeds it one installed row and the server's
//! current view of the same rom.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use regex::Regex;
use std::sync::OnceLock;

use super::registry::InstalledGame;

/// A version tag found inside a rom file name. The two kinds never compare
/// against each other (update_detection.py:64-65).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionTag {
    /// `(vNNNNN)` — exactly five digits.
    Numeric(u32),
    /// `(vX.Y[.Z…])` — at least one dot.
    Semver(Vec<u32>),
}

fn numeric_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\(v(\d{5})\)").unwrap())
}

fn semver_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\(v(\d+(?:\.\d+)+)\)").unwrap())
}

/// `rom_file_name_version` (update_detection.py:20-31): numeric first, then
/// semver, else `None`. `(v1234)` matches neither.
pub fn rom_file_name_version(rom_file_name: &str) -> Option<VersionTag> {
    if let Some(caps) = numeric_re().captures(rom_file_name) {
        return caps[1].parse().ok().map(VersionTag::Numeric);
    }
    let caps = semver_re().captures(rom_file_name)?;
    let parts: Option<Vec<u32>> = caps[1].split('.').map(|p| p.parse().ok()).collect();
    parts.map(VersionTag::Semver)
}

/// `_format_version_tag_for_ui` (grid-launcher.py:3273-3280): `v01234` for a
/// numeric tag, `v3.6.0` for a semver tag.
pub fn format_version_tag(tag: &VersionTag) -> String {
    match tag {
        VersionTag::Numeric(n) => format!("v{n:05}"),
        VersionTag::Semver(parts) => {
            let joined: Vec<String> = parts.iter().map(u32::to_string).collect();
            format!("v{}", joined.join("."))
        }
    }
}

fn semver_is_newer(installed: &[u32], server: &[u32]) -> bool {
    let len = installed.len().max(server.len());
    for i in 0..len {
        let a = installed.get(i).copied().unwrap_or(0);
        let b = server.get(i).copied().unwrap_or(0);
        if b > a {
            return true;
        }
        if b < a {
            return false;
        }
    }
    false
}

/// `has_newer_server_rom_version` (update_detection.py:56-70).
pub fn has_newer_server_rom_version(installed_name: &str, server_name: &str) -> bool {
    let (Some(installed), Some(server)) = (
        rom_file_name_version(installed_name),
        rom_file_name_version(server_name),
    ) else {
        return false;
    };
    match (installed, server) {
        (VersionTag::Numeric(a), VersionTag::Numeric(b)) => b > a,
        (VersionTag::Semver(a), VersionTag::Semver(b)) => semver_is_newer(&a, &b),
        _ => false,
    }
}

/// `_is_windows_pc_platform` (update_detection.py:73-80).
pub fn is_windows_pc_platform(platform: &str) -> bool {
    let normalized = platform.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    normalized.contains("windows") || normalized == "pc"
}

/// The default emulators-platform predicate (update_detection.py:103).
pub fn is_emulators_platform(platform: &str) -> bool {
    platform.trim().to_lowercase() == "emulators"
}

/// `_parse_timestamp` (update_detection.py:83-94): every `Z` becomes
/// `+00:00`; an offset-less value is taken as UTC; anything unparseable is
/// `None`, never an error.
pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    let candidate = text.replace('Z', "+00:00");
    if let Ok(parsed) = DateTime::parse_from_rfc3339(&candidate) {
        return Some(parsed.with_timezone(&Utc));
    }
    const NAIVE: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];
    for format in NAIVE {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&candidate, format) {
            return Some(naive.and_utc());
        }
    }
    NaiveDate::parse_from_str(&candidate, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|naive| naive.and_utc())
}

/// The server's current view of one rom — the three fields the decision
/// reads. Built by the app layer from a `RomDetail`.
#[derive(Debug, Clone, Copy)]
pub struct ServerVersion<'a> {
    pub platform: &'a str,
    pub rom_file_name: &'a str,
    pub updated_at: &'a str,
}

/// `game_has_server_update` (update_detection.py:97-122).
pub fn game_has_server_update(installed: &InstalledGame, server: &ServerVersion<'_>) -> bool {
    if is_emulators_platform(&installed.platform) || is_emulators_platform(server.platform) {
        return false;
    }
    if (is_windows_pc_platform(&installed.platform) || is_windows_pc_platform(server.platform))
        && has_newer_server_rom_version(&installed.rom_file_name, server.rom_file_name)
    {
        return true;
    }
    // Legacy installs carry no install-time server timestamp.
    let Some(installed_at) = parse_timestamp(&installed.server_updated_at) else {
        return false;
    };
    let Some(server_at) = parse_timestamp(server.updated_at) else {
        return false;
    };
    server_at > installed_at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(platform: &str, rom_file_name: &str, server_updated_at: &str) -> InstalledGame {
        InstalledGame {
            title: "Game".to_string(),
            platform: platform.to_string(),
            rom_file_name: rom_file_name.to_string(),
            server_updated_at: server_updated_at.to_string(),
            ..Default::default()
        }
    }

    fn server<'a>(
        platform: &'a str,
        rom_file_name: &'a str,
        updated_at: &'a str,
    ) -> ServerVersion<'a> {
        ServerVersion {
            platform,
            rom_file_name,
            updated_at,
        }
    }

    // tests/test_update_detection.py:162-176
    #[test]
    fn extracts_v_five_digits() {
        assert_eq!(
            rom_file_name_version("My Game (v00042).zip"),
            Some(VersionTag::Numeric(42))
        );
    }

    #[test]
    fn extracts_semver_from_real_filename() {
        assert_eq!(
            rom_file_name_version("A Little to the Left (v3.6.0) (2022) (W_P).7z"),
            Some(VersionTag::Semver(vec![3, 6, 0]))
        );
    }

    #[test]
    fn none_without_matching_tag() {
        assert_eq!(rom_file_name_version("My Game (v1234).zip"), None);
        assert_eq!(rom_file_name_version("My Game.zip"), None);
    }

    #[test]
    fn tag_match_is_case_insensitive() {
        assert_eq!(
            rom_file_name_version("x (V00007).zip"),
            Some(VersionTag::Numeric(7))
        );
    }

    #[test]
    fn numeric_is_preferred_over_semver_when_both_present() {
        assert_eq!(
            rom_file_name_version("x (v1.2) (v00003).zip"),
            Some(VersionTag::Numeric(3))
        );
    }

    #[test]
    fn formats_numeric_zero_padded_and_semver_verbatim() {
        assert_eq!(format_version_tag(&VersionTag::Numeric(42)), "v00042");
        assert_eq!(
            format_version_tag(&VersionTag::Semver(vec![3, 6, 0])),
            "v3.6.0"
        );
    }

    // tests/test_update_detection.py:178-238
    #[test]
    fn compares_numerically() {
        assert!(has_newer_server_rom_version(
            "My Game (v00009).zip",
            "My Game (v00010).zip"
        ));
        assert!(!has_newer_server_rom_version(
            "My Game (v00010).zip",
            "My Game (v00010).zip"
        ));
        assert!(!has_newer_server_rom_version(
            "My Game (v00011).zip",
            "My Game (v00010).zip"
        ));
    }

    #[test]
    fn false_when_missing_tags() {
        assert!(!has_newer_server_rom_version(
            "My Game.zip",
            "My Game (v00010).zip"
        ));
        assert!(!has_newer_server_rom_version(
            "My Game (v00010).zip",
            "My Game.zip"
        ));
    }

    #[test]
    fn compares_dotted_semver_parts() {
        let a = "A Little to the Left (v3.5.9) (2022) (W_P).7z";
        let b = "A Little to the Left (v3.6.0) (2022) (W_P).7z";
        let c = "A Little to the Left (v3.6.0.1) (2022) (W_P).7z";
        assert!(has_newer_server_rom_version(a, b));
        assert!(!has_newer_server_rom_version(b, a));
        assert!(!has_newer_server_rom_version(b, b));
        assert!(has_newer_server_rom_version(b, c));
        assert!(!has_newer_server_rom_version(
            "x (v1.2).zip",
            "x (v1.2.0).zip"
        ));
        assert!(has_newer_server_rom_version(
            "x (v1.2).zip",
            "x (v1.2.1).zip"
        ));
    }

    #[test]
    fn mixed_numeric_and_semver_is_false() {
        assert!(!has_newer_server_rom_version(
            "My Game (v01234).zip",
            "My Game (v3.6.0).zip"
        ));
        assert!(!has_newer_server_rom_version(
            "My Game (v3.6.0).zip",
            "My Game (v01234).zip"
        ));
    }

    #[test]
    fn windows_pc_platform_predicate() {
        assert!(is_windows_pc_platform("Windows"));
        assert!(is_windows_pc_platform(" windows 10 "));
        assert!(is_windows_pc_platform("PC"));
        assert!(!is_windows_pc_platform("PC Engine"));
        assert!(!is_windows_pc_platform(""));
        assert!(!is_windows_pc_platform("PS2"));
    }

    #[test]
    fn parses_timestamps_z_naive_and_garbage() {
        let z = parse_timestamp("2026-04-10T14:30:00Z").unwrap();
        let offset = parse_timestamp("2026-04-10T16:30:00+02:00").unwrap();
        let naive = parse_timestamp("2026-04-10T14:30:00").unwrap();
        let spaced = parse_timestamp(" 2026-04-10 14:30:00 ").unwrap();
        let fractional = parse_timestamp("2026-04-10T14:30:00.250Z").unwrap();
        assert_eq!(z, offset);
        assert_eq!(z, naive);
        assert_eq!(z, spaced);
        assert!(fractional > z);
        assert_eq!(
            parse_timestamp("2026-04-10").unwrap().to_rfc3339(),
            "2026-04-10T00:00:00+00:00"
        );
        assert_eq!(parse_timestamp(""), None);
        assert_eq!(parse_timestamp("   "), None);
        assert_eq!(parse_timestamp("not a date"), None);
        assert_eq!(parse_timestamp("2026-13-45T00:00:00Z"), None);
    }

    // tests/test_update_detection.py:110-160
    #[test]
    fn true_when_server_timestamp_is_newer() {
        let installed = row("PS2", "", "2026-04-09T14:30:00Z");
        assert!(game_has_server_update(
            &installed,
            &server("PS2", "", "2026-04-10T14:30:00Z")
        ));
    }

    #[test]
    fn equal_timestamps_are_not_an_update() {
        let installed = row("PS2", "", "2026-04-10T14:30:00Z");
        assert!(!game_has_server_update(
            &installed,
            &server("PS2", "", "2026-04-10T14:30:00Z")
        ));
    }

    #[test]
    fn legacy_install_without_timestamp_is_false() {
        let installed = row("PS2", "", "");
        assert!(!game_has_server_update(
            &installed,
            &server("PS2", "", "2026-04-10T14:30:00Z")
        ));
    }

    #[test]
    fn unparseable_server_timestamp_is_false() {
        let installed = row("PS2", "", "2026-04-09T14:30:00Z");
        assert!(!game_has_server_update(
            &installed,
            &server("PS2", "", "soon")
        ));
        assert!(!game_has_server_update(&installed, &server("PS2", "", "")));
    }

    #[test]
    fn emulators_platform_is_vetoed_on_either_side() {
        let installed = row("Emulators", "", "2026-04-09T14:30:00Z");
        assert!(!game_has_server_update(
            &installed,
            &server("Emulators", "", "2026-04-10T14:30:00Z")
        ));
        let installed = row("PS2", "", "2026-04-09T14:30:00Z");
        assert!(!game_has_server_update(
            &installed,
            &server(" emulators ", "", "2026-04-10T14:30:00Z")
        ));
    }

    #[test]
    fn windows_uses_rom_file_version_without_timestamps() {
        let installed = row("Windows", "Windows Game (v00009).zip", "");
        assert!(game_has_server_update(
            &installed,
            &server("Windows", "Windows Game (v00010).zip", "")
        ));
    }

    #[test]
    fn windows_older_tag_falls_through_to_timestamps() {
        let installed = row(
            "Windows",
            "Windows Game (v00010).zip",
            "2026-04-09T14:30:00Z",
        );
        // The tag says "not newer"; the timestamp still decides.
        assert!(game_has_server_update(
            &installed,
            &server(
                "Windows",
                "Windows Game (v00010).zip",
                "2026-04-10T14:30:00Z"
            )
        ));
        assert!(!game_has_server_update(
            &installed,
            &server(
                "Windows",
                "Windows Game (v00009).zip",
                "2026-04-09T14:30:00Z"
            )
        ));
    }

    #[test]
    fn non_windows_ignores_rom_file_version() {
        let installed = row("PS2", "PS2 Game (v00009).zip", "");
        assert!(!game_has_server_update(
            &installed,
            &server("PS2", "PS2 Game (v00010).zip", "")
        ));
    }

    #[test]
    fn pc_platform_on_the_server_side_enables_the_tag_check() {
        let installed = row("PS2", "Game (v00009).zip", "");
        assert!(game_has_server_update(
            &installed,
            &server("PC", "Game (v00010).zip", "")
        ));
    }
}
