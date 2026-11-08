//! Server › Discover's two lists, as pure functions over `/api/roms` pages.
//!
//! - **New releases:** games on the *preferred platforms* released in the
//!   last [`NEW_RELEASE_MONTHS`] months (never in the future), newest first,
//!   at most [`NEW_RELEASES_MAX`].
//! - **Recently added:** the newest games on the server by `created_at`,
//!   leaving out the New releases games, at most [`RECENTLY_ADDED_MAX`].
//!
//! Both always filter and sort here, whatever order the server sent: RomM
//! ignores an `order_by` it does not know and answers in its default order
//! (verified against RomM 5.4.1), and upcoming games carry release dates in
//! the future.
//!
//! The preferred platforms ([`preferred_platform_ids`]) are the systems with
//! a default emulator, Windows once a compat tool is the default on a
//! non-Windows computer, and the platforms native to this computer always.

use super::{GameSummary, Platform};
use crate::launch::selection::NO_EMULATOR;
use crate::library::platforms::{is_native_platform, is_windows_platform};
use chrono::{DateTime, Months, NaiveDateTime, Utc};
use std::collections::{BTreeMap, HashSet};

/// How far back a release still counts as new.
pub const NEW_RELEASE_MONTHS: u32 = 6;
/// The most games the coverflow holds.
pub const NEW_RELEASES_MAX: usize = 25;
/// The most games Recently added lists.
pub const RECENTLY_ADDED_MAX: usize = 8;

/// The computer's operating system, as far as nativeness goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Linux,
    Windows,
    /// Any other system: no server platform is native to it.
    Other,
}

impl HostOs {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            HostOs::Windows
        } else if cfg!(target_os = "linux") {
            HostOs::Linux
        } else {
            HostOs::Other
        }
    }
}

/// Whether `platform` runs natively on `host`: Linux platforms on Linux,
/// Windows platforms on Windows.
fn native_to(platform: &Platform, host: HostOs) -> bool {
    let windows = is_windows_platform(&platform.slug, &platform.name)
        || is_windows_platform("", platform.label());
    let native = is_native_platform(&platform.slug, &platform.name)
        || is_native_platform("", platform.label());
    match host {
        HostOs::Linux => native && !windows,
        HostOs::Windows => windows,
        HostOs::Other => false,
    }
}

/// The saved default emulator for `label`, matched case-insensitively the
/// way every reader of `default_emulators` folds it; `None` when unset,
/// blank or the explicit "no emulator" marker.
fn has_default_emulator(default_emulators: &BTreeMap<String, String>, label: &str) -> bool {
    let folded = label.to_lowercase();
    default_emulators
        .iter()
        .find(|(key, _)| key.to_lowercase() == folded)
        .is_some_and(|(_, value)| {
            let value = value.trim();
            !value.is_empty() && value != NO_EMULATOR
        })
}

/// The ids of the platforms New releases draws from, in `platforms` order:
/// a platform with a default emulator (keyed by its label), a Windows
/// platform once `compat_default` is set on a non-Windows host, and every
/// platform native to `host`.
pub fn preferred_platform_ids(
    platforms: &[Platform],
    default_emulators: &BTreeMap<String, String>,
    compat_default: bool,
    host: HostOs,
) -> Vec<i64> {
    platforms
        .iter()
        .filter(|p| {
            native_to(p, host)
                || has_default_emulator(default_emulators, p.label())
                || (compat_default
                    && host != HostOs::Windows
                    && (is_windows_platform(&p.slug, &p.name)
                        || is_windows_platform("", p.label())))
        })
        .map(|p| p.id)
        .collect()
}

/// The first moment that still counts as a new release: `now` less
/// [`NEW_RELEASE_MONTHS`] calendar months, in seconds.
pub fn new_release_cutoff(now: DateTime<Utc>) -> i64 {
    now.checked_sub_months(Months::new(NEW_RELEASE_MONTHS))
        .unwrap_or(now)
        .timestamp()
}

/// New releases: the games on `preferred` platforms whose release date
/// falls within the last [`NEW_RELEASE_MONTHS`] months up to `now`
/// (inclusive), newest first, at most [`NEW_RELEASES_MAX`]. A game without
/// a date, or dated in the future, is left out; a game listed twice keeps
/// its first entry.
pub fn new_releases(
    games: Vec<GameSummary>,
    preferred: &[i64],
    now: DateTime<Utc>,
) -> Vec<GameSummary> {
    let since = new_release_cutoff(now);
    let until = now.timestamp();
    let preferred: HashSet<i64> = preferred.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut out: Vec<GameSummary> = games
        .into_iter()
        .filter(|g| preferred.contains(&g.platform_id))
        .filter(|g| {
            g.first_release_date
                .is_some_and(|d| d >= since && d <= until)
        })
        .filter(|g| seen.insert(g.id))
        .collect();
    out.sort_by(|a, b| {
        b.first_release_date
            .cmp(&a.first_release_date)
            .then_with(|| b.id.cmp(&a.id))
    });
    out.truncate(NEW_RELEASES_MAX);
    out
}

/// `created_at` in seconds. RomM sends ISO 8601 with an offset
/// (`2026-11-06T07:42:09+00:00`); one without an offset is read as UTC.
/// `None` when blank or unreadable.
pub fn created_at_seconds(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.timestamp());
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|fmt| NaiveDateTime::parse_from_str(value, fmt).ok())
        .map(|dt| dt.and_utc().timestamp())
}

/// Recently added: `games` newest first by `created_at`, leaving out
/// `exclude` (the New releases games) and any game without a readable
/// `created_at`, at most [`RECENTLY_ADDED_MAX`]. A game listed twice keeps
/// its first entry.
pub fn recently_added(games: Vec<GameSummary>, exclude: &[i64]) -> Vec<GameSummary> {
    let exclude: HashSet<i64> = exclude.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut dated: Vec<(i64, GameSummary)> = games
        .into_iter()
        .filter(|g| !exclude.contains(&g.id))
        .filter(|g| seen.insert(g.id))
        .filter_map(|g| created_at_seconds(&g.created_at).map(|at| (at, g)))
        .collect();
    dated.sort_by(|(a_at, a), (b_at, b)| b_at.cmp(a_at).then_with(|| b.id.cmp(&a.id)));
    dated
        .into_iter()
        .take(RECENTLY_ADDED_MAX)
        .map(|(_, g)| g)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 8, 12, 0, 0).unwrap()
    }

    fn day(y: i32, m: u32, d: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap().timestamp()
    }

    fn game(id: i64, platform_id: i64, released: Option<i64>, added: &str) -> GameSummary {
        GameSummary {
            id,
            name: format!("Game {id}"),
            platform_id,
            first_release_date: released,
            created_at: added.to_string(),
            ..Default::default()
        }
    }

    fn platform(id: i64, name: &str, slug: &str) -> Platform {
        Platform {
            id,
            name: name.to_string(),
            slug: slug.to_string(),
            rom_count: 1,
            custom_name: None,
            display_name: name.to_string(),
        }
    }

    fn ids(games: &[GameSummary]) -> Vec<i64> {
        games.iter().map(|g| g.id).collect()
    }

    #[test]
    fn new_releases_keep_the_window_newest_first_and_drop_the_future() {
        let games = vec![
            game(1, 1, Some(day(2026, 10, 1)), ""),
            game(2, 1, Some(day(2026, 11, 7)), ""),
            game(3, 1, Some(day(2027, 2, 1)), ""), // upcoming
            game(4, 1, Some(day(2026, 5, 7)), ""), // older than six months
            game(5, 1, None, ""),
            game(6, 1, Some(day(2026, 5, 9)), ""), // just inside
        ];
        assert_eq!(ids(&new_releases(games, &[1], now())), vec![2, 1, 6]);
    }

    #[test]
    fn new_releases_only_take_preferred_platforms() {
        let games = vec![
            game(1, 1, Some(day(2026, 10, 1)), ""),
            game(2, 2, Some(day(2026, 10, 2)), ""),
        ];
        assert_eq!(ids(&new_releases(games, &[1], now())), vec![1]);
    }

    #[test]
    fn new_releases_stop_at_twenty_five() {
        let games: Vec<_> = (0..40)
            .map(|i| game(i, 1, Some(day(2026, 10, 1) + i), ""))
            .collect();
        let out = new_releases(games, &[1], now());
        assert_eq!(out.len(), NEW_RELEASES_MAX);
        assert_eq!(out[0].id, 39);
    }

    #[test]
    fn the_cutoff_is_six_calendar_months_back() {
        assert_eq!(
            new_release_cutoff(now()),
            Utc.with_ymd_and_hms(2026, 5, 8, 12, 0, 0)
                .unwrap()
                .timestamp()
        );
    }

    #[test]
    fn recently_added_sorts_by_created_at_and_leaves_out_the_coverflow() {
        let games = vec![
            game(1, 1, None, "2026-11-01T10:00:00+00:00"),
            game(2, 1, None, "2026-11-06T07:42:09+00:00"),
            game(3, 1, None, "2026-11-07T00:00:00+02:00"),
            game(4, 1, None, ""),
            game(5, 1, None, "2026-11-05T00:00:00"),
        ];
        assert_eq!(ids(&recently_added(games, &[2])), vec![3, 5, 1]);
    }

    #[test]
    fn recently_added_stops_at_eight() {
        let games: Vec<_> = (0..20)
            .map(|i| game(i, 1, None, &format!("2026-10-{:02}T00:00:00+00:00", i + 1)))
            .collect();
        let out = recently_added(games, &[]);
        assert_eq!(ids(&out), vec![19, 18, 17, 16, 15, 14, 13, 12]);
    }

    #[test]
    fn created_at_reads_offsets_and_naive_times() {
        assert_eq!(created_at_seconds("1970-01-01T00:01:00+00:00"), Some(60));
        assert_eq!(created_at_seconds("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(created_at_seconds("1970-01-01T00:00:30.5"), Some(30));
        assert_eq!(created_at_seconds("not a date"), None);
        assert_eq!(created_at_seconds(""), None);
    }

    #[test]
    fn preferred_platforms_are_defaults_native_and_windows_with_a_compat_tool() {
        let platforms = vec![
            platform(1, "Super Nintendo Entertainment System", "snes"),
            platform(2, "Arcade", "arcade"),
            platform(3, "Windows", "win"),
            platform(4, "Linux", "linux"),
            platform(5, "Nintendo 64", "n64"),
        ];
        let mut defaults = BTreeMap::new();
        defaults.insert(
            "super nintendo entertainment system".to_string(),
            "snes9x".to_string(),
        );
        defaults.insert("Nintendo 64".to_string(), NO_EMULATOR.to_string());
        assert_eq!(
            preferred_platform_ids(&platforms, &defaults, false, HostOs::Linux),
            vec![1, 4]
        );
        assert_eq!(
            preferred_platform_ids(&platforms, &defaults, true, HostOs::Linux),
            vec![1, 3, 4]
        );
        assert_eq!(
            preferred_platform_ids(&platforms, &defaults, false, HostOs::Windows),
            vec![1, 3]
        );
        assert_eq!(
            preferred_platform_ids(&platforms, &BTreeMap::new(), false, HostOs::Other),
            Vec::<i64>::new()
        );
    }
}
