//! Session-window derivation and mtime-window filtering for cloud sync
//! upload candidates.
//!
//! Ported from `grid_launcher/library/cloud_sync.py:243-343`. `IgnoreSets`
//! and `latest_mtime_under`, which the directory filters here depend on,
//! live in `cloud/mod.rs` (ported from
//! `grid_launcher/ui/mixins/cloud_mixin.py:1490-1532`) — see the D8 note in
//! `docs/porting/06-cloud-saves.md` for the one Python branch
//! (`partition_active_game_sessions`' unpollable-drop case) this milestone
//! does not carry forward.

use std::path::PathBuf;

use super::state::{games_match_identity, SyncStateEntry};
use super::{file_mtime_secs, latest_mtime_under, CloudGame, IgnoreSets};

/// An inclusive `[start, end]` mtime window, in unix seconds.
pub type Window = (f64, f64);

/// One tracked play session, as the upload-time scan sees it. Mirrors the
/// `{"game": ..., "started_at": ...}` shape `session_window_for_state_upload`
/// reads out of `active_game_sessions` in `cloud_sync.py:250-263`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSessionRef {
    pub game: CloudGame,
    pub started_at: f64,
}

/// `cloud_sync.py:243-282`: the inclusive mtime window a state upload
/// should scan under for `game`, sourced from whichever session is live
/// for it, else the persisted `entry`.
///
/// Walks `sessions` in REVERSE (last element first) for the first entry
/// whose `game` matches by [`games_match_identity`] AND whose `started_at`
/// is `> 0.0`. That session wins immediately: earlier (older) sessions in
/// the reversed walk are never inspected once a hit is found, and a
/// session that matches by identity but has `started_at <= 0.0` is
/// SKIPPED rather than accepted with a degenerate window — the walk keeps
/// going past it. The winning session's window is
/// `(max(0.0, started_at - 2.0), now + 30.0)`: a 2s lead-in on the start,
/// and `now` (not the session's own end, since it is still active) plus a
/// 30s tail-out.
///
/// When no session matches, falls back to `entry`'s persisted
/// `last_session_started_at` / `last_session_ended_at`:
/// - `started_at <= 0.0` → `None` (nothing to scan).
/// - `ended_at` is CLAMPED up to `started_at` when it is `<= 0.0` OR less
///   than `started_at` — an unset or corrupt end can never produce a
///   window narrower than the start alone.
/// - otherwise `(max(0.0, started_at - 2.0), ended_at + 30.0)` — same 2s
///   lead-in, 30s tail-out past the (possibly clamped) end.
pub fn session_window_for_state_upload(
    sessions: &[ActiveSessionRef],
    game: &CloudGame,
    entry: &SyncStateEntry,
    now: f64,
) -> Option<Window> {
    for session in sessions.iter().rev() {
        if !games_match_identity(&session.game, game) {
            continue;
        }
        if session.started_at <= 0.0 {
            continue;
        }
        return Some(((session.started_at - 2.0).max(0.0), now + 30.0));
    }

    let started_at = entry.last_session_started_at;
    if started_at <= 0.0 {
        return None;
    }
    let mut ended_at = entry.last_session_ended_at;
    if ended_at <= 0.0 || ended_at < started_at {
        ended_at = started_at;
    }
    Some(((started_at - 2.0).max(0.0), ended_at + 30.0))
}

/// `cloud_sync.py:285-294`: keep files whose mtime falls inside the
/// INCLUSIVE `window` bounds, preserving `files`' order. `window = None`
/// is a passthrough — every file is kept, unchanged. A file whose mtime
/// can't be read (stat failure) is silently skipped, not an error.
pub fn filter_files_by_mtime_window(files: &[PathBuf], window: Option<Window>) -> Vec<PathBuf> {
    let Some((start, end)) = window else {
        return files.to_vec();
    };
    files
        .iter()
        .filter(|path| match file_mtime_secs(path) {
            Some(mtime) => start <= mtime && mtime <= end,
            None => false,
        })
        .cloned()
        .collect()
}

/// `cloud_sync.py:318-322`: `window = None` passes `files` through
/// untouched. Otherwise apply [`filter_files_by_mtime_window`]; if that
/// filter yields nothing — every candidate fell outside the window, or
/// none could be stat'd — FALL BACK to the original, unfiltered `files`
/// rather than uploading nothing.
pub fn session_filtered_file_candidates(
    files: Vec<PathBuf>,
    window: Option<Window>,
) -> Vec<PathBuf> {
    if window.is_none() {
        return files;
    }
    let filtered = filter_files_by_mtime_window(&files, window);
    if filtered.is_empty() {
        files
    } else {
        filtered
    }
}

/// `cloud_sync.py:297-315`: keep directories whose newest non-blocked file
/// beneath them (via [`latest_mtime_under`]) falls inside the INCLUSIVE
/// `window` bounds, preserving `dirs`' order. `window = None` is a
/// passthrough, mirroring [`filter_files_by_mtime_window`].
pub fn filter_directories_by_mtime_window(
    dirs: &[PathBuf],
    window: Option<Window>,
    ignore: &IgnoreSets,
) -> Vec<PathBuf> {
    let Some((start, end)) = window else {
        return dirs.to_vec();
    };
    dirs.iter()
        .filter(|dir| {
            let latest = latest_mtime_under(dir, ignore);
            start <= latest && latest <= end
        })
        .cloned()
        .collect()
}

/// `cloud_sync.py:325-343`: `window = None` passes `dirs` through
/// untouched. Otherwise apply [`filter_directories_by_mtime_window`]; an
/// empty result FALLS BACK to the original, unfiltered `dirs`, same as
/// [`session_filtered_file_candidates`].
pub fn session_filtered_directory_candidates(
    dirs: Vec<PathBuf>,
    window: Option<Window>,
    ignore: &IgnoreSets,
) -> Vec<PathBuf> {
    if window.is_none() {
        return dirs;
    }
    let filtered = filter_directories_by_mtime_window(&dirs, window, ignore);
    if filtered.is_empty() {
        dirs
    } else {
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, UNIX_EPOCH};

    fn game(title: &str, platform: &str, rom_id: &str) -> CloudGame {
        CloudGame {
            title: title.to_string(),
            platform: platform.to_string(),
            rom_id: rom_id.to_string(),
            ..Default::default()
        }
    }

    fn touch_at(path: &std::path::Path, unix_secs: f64) {
        fs::write(path, b"x").unwrap();
        let modified = UNIX_EPOCH + Duration::from_secs_f64(unix_secs);
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(modified).unwrap();
    }

    fn ignore(basenames: &[&str], extensions: &[&str]) -> IgnoreSets {
        IgnoreSets {
            basenames: basenames.iter().map(|s| s.to_lowercase()).collect(),
            extensions: extensions.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    // -- session_window_for_state_upload -----------------------------------

    #[test]
    fn window_uses_the_most_recent_matching_active_session() {
        let g = game("Chrono Trigger", "SNES", "rom-1");
        let older = ActiveSessionRef {
            game: g.clone(),
            started_at: 1_000.0,
        };
        let newer = ActiveSessionRef {
            game: g.clone(),
            started_at: 5_000.0,
        };
        let entry = SyncStateEntry::default();

        // Reverse walk: `newer` (last in the slice) is inspected first and
        // wins, even though `older` also matches by identity.
        let window = session_window_for_state_upload(&[older, newer], &g, &entry, 5_100.0).unwrap();
        assert_eq!(window, (5_000.0 - 2.0, 5_100.0 + 30.0));
    }

    #[test]
    fn window_applies_the_2s_leadin_and_30s_tailout() {
        let g = game("Chrono Trigger", "SNES", "rom-1");
        let session = ActiveSessionRef {
            game: g.clone(),
            started_at: 100.0,
        };
        let entry = SyncStateEntry::default();

        let window = session_window_for_state_upload(&[session], &g, &entry, 200.0).unwrap();
        assert_eq!(window.0, 98.0, "start - 2.0 exactly");
        assert_eq!(window.1, 230.0, "now + 30.0 exactly");
    }

    #[test]
    fn window_leadin_clamps_at_zero_for_an_early_start() {
        let g = game("Chrono Trigger", "SNES", "rom-1");
        let session = ActiveSessionRef {
            game: g.clone(),
            started_at: 1.0,
        };
        let entry = SyncStateEntry::default();

        let window = session_window_for_state_upload(&[session], &g, &entry, 50.0).unwrap();
        assert_eq!(window.0, 0.0, "start - 2.0 would be negative, clamped to 0");
    }

    #[test]
    fn window_skips_a_matching_session_with_a_nonpositive_start() {
        let g = game("Chrono Trigger", "SNES", "rom-1");
        let unstarted = ActiveSessionRef {
            game: g.clone(),
            started_at: 0.0,
        };
        let entry = SyncStateEntry::default();

        // No persisted fallback either: overall result is None, proving the
        // zero-started session was skipped rather than accepted.
        assert_eq!(
            session_window_for_state_upload(&[unstarted], &g, &entry, 100.0),
            None
        );
    }

    #[test]
    fn window_falls_back_to_persisted_state_and_clamps_ended() {
        let g = game("Chrono Trigger", "SNES", "rom-1");

        // ended < started: clamp ended up to started.
        let entry_before_end = SyncStateEntry {
            last_session_started_at: 500.0,
            last_session_ended_at: 100.0,
            ..Default::default()
        };
        let window = session_window_for_state_upload(&[], &g, &entry_before_end, 9_999.0).unwrap();
        assert_eq!(
            window,
            (498.0, 530.0),
            "ended clamped up to started, then +30"
        );

        // ended == 0: also clamps to started.
        let entry_zero_end = SyncStateEntry {
            last_session_started_at: 500.0,
            last_session_ended_at: 0.0,
            ..Default::default()
        };
        let window = session_window_for_state_upload(&[], &g, &entry_zero_end, 9_999.0).unwrap();
        assert_eq!(
            window,
            (498.0, 530.0),
            "unset ended clamped to started, then +30"
        );
    }

    #[test]
    fn window_is_none_when_no_session_and_no_persisted_start() {
        let g = game("Chrono Trigger", "SNES", "rom-1");
        let entry = SyncStateEntry::default();
        assert_eq!(
            session_window_for_state_upload(&[], &g, &entry, 100.0),
            None
        );

        // A non-matching session present doesn't change the outcome.
        let other = ActiveSessionRef {
            game: game("Other Game", "PS2", "rom-2"),
            started_at: 10.0,
        };
        assert_eq!(
            session_window_for_state_upload(&[other], &g, &entry, 100.0),
            None
        );
    }

    // -- filter_files_by_mtime_window / session_filtered_file_candidates ---

    #[test]
    fn filter_files_is_inclusive_on_both_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let at_start = dir.path().join("at_start.sav");
        let at_end = dir.path().join("at_end.sav");
        let before = dir.path().join("before.sav");
        let after = dir.path().join("after.sav");
        touch_at(&at_start, 100.0);
        touch_at(&at_end, 200.0);
        touch_at(&before, 99.0);
        touch_at(&after, 201.0);

        let files = vec![
            at_start.clone(),
            at_end.clone(),
            before.clone(),
            after.clone(),
        ];
        let result = filter_files_by_mtime_window(&files, Some((100.0, 200.0)));
        assert_eq!(result, vec![at_start, at_end]);
    }

    #[test]
    fn filter_files_passthrough_on_none_window() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.sav");
        let f2 = dir.path().join("b.sav");
        touch_at(&f1, 1.0);
        touch_at(&f2, 999_999.0);

        let files = vec![f1.clone(), f2.clone()];
        assert_eq!(filter_files_by_mtime_window(&files, None), files);
    }

    #[test]
    fn filter_files_skips_a_path_that_cannot_be_statted() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does_not_exist.sav");
        let result = filter_files_by_mtime_window(&[missing], Some((0.0, 1_000_000_000.0)));
        assert!(result.is_empty());
    }

    #[test]
    fn session_filtered_files_fall_back_when_everything_is_out_of_window() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.sav");
        let f2 = dir.path().join("b.sav");
        touch_at(&f1, 1.0);
        touch_at(&f2, 2.0);

        let files = vec![f1.clone(), f2.clone()];
        // Window excludes both files entirely.
        let result = session_filtered_file_candidates(files.clone(), Some((1_000.0, 2_000.0)));
        assert_eq!(result, files, "empty filter result falls back to input");
    }

    #[test]
    fn session_filtered_files_passthrough_on_none_window() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.sav");
        touch_at(&f1, 1.0);
        let files = vec![f1.clone()];
        assert_eq!(session_filtered_file_candidates(files.clone(), None), files);
    }

    // -- filter_directories_by_mtime_window / directory candidates ---------

    #[test]
    fn filter_directories_compares_the_newest_non_blocked_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("save_dir");
        fs::create_dir(&target).unwrap();

        // The newest file in the directory is a blocked basename (e.g. a
        // lock file); the newest NON-blocked file is older, and must be
        // what decides membership.
        let blocked = target.join("lock.tmp");
        let good = target.join("slot1.sav");
        touch_at(&blocked, 500.0);
        touch_at(&good, 150.0);

        let ignore = ignore(&["lock.tmp"], &[]);

        // Window covers the good file's mtime but not the blocked file's.
        let in_window = filter_directories_by_mtime_window(
            std::slice::from_ref(&target),
            Some((100.0, 200.0)),
            &ignore,
        );
        assert_eq!(
            in_window,
            vec![target.clone()],
            "newest non-blocked file (150.0) decides, not the blocked file (500.0)"
        );

        // A window that only covers the blocked file's mtime excludes the
        // directory, proving the blocked file was genuinely ignored.
        let out_of_window =
            filter_directories_by_mtime_window(&[target], Some((400.0, 600.0)), &ignore);
        assert!(out_of_window.is_empty());
    }

    #[test]
    fn filter_directories_passthrough_on_none_window() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("save_dir");
        fs::create_dir(&target).unwrap();
        let ignore = ignore(&[], &[]);
        assert_eq!(
            filter_directories_by_mtime_window(std::slice::from_ref(&target), None, &ignore),
            vec![target]
        );
    }

    #[test]
    fn session_filtered_directories_fall_back_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("save_dir");
        fs::create_dir(&target).unwrap();
        let file = target.join("slot1.sav");
        touch_at(&file, 1.0);

        let ignore = ignore(&[], &[]);
        let dirs = vec![target.clone()];
        let result =
            session_filtered_directory_candidates(dirs.clone(), Some((1_000.0, 2_000.0)), &ignore);
        assert_eq!(result, dirs, "empty filter result falls back to input");
    }

    #[test]
    fn session_filtered_directories_passthrough_on_none_window() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("save_dir");
        fs::create_dir(&target).unwrap();
        let ignore = ignore(&[], &[]);
        let dirs = vec![target.clone()];
        assert_eq!(
            session_filtered_directory_candidates(dirs.clone(), None, &ignore),
            dirs
        );
    }

    // -- IgnoreSets::blocks / latest_mtime_under ----------------------------

    #[test]
    fn ignore_sets_blocks_by_basename_and_extension_case_insensitively() {
        let ignore = ignore(&["lock.tmp"], &[".bak"]);
        assert!(ignore.blocks(std::path::Path::new("/x/LOCK.TMP")));
        assert!(ignore.blocks(std::path::Path::new("/x/whatever.BAK")));
        assert!(!ignore.blocks(std::path::Path::new("/x/slot1.sav")));
    }

    #[test]
    fn latest_mtime_under_skips_blocked_and_unstatable_entries() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();

        let blocked = root.join("lock.tmp");
        let good_old = nested.join("old.sav");
        let good_new = root.join("new.sav");
        touch_at(&blocked, 900.0);
        touch_at(&good_old, 50.0);
        touch_at(&good_new, 300.0);

        // An entry the walk can list but cannot stat: a symlink to a
        // nonexistent target. `Path::is_file()` on a broken symlink is
        // `false`, so the walk excludes it before ever attempting a stat —
        // mirroring Python's `candidate.is_file()` gate ahead of `.stat()`.
        #[cfg(unix)]
        {
            let broken = root.join("broken.sav");
            std::os::unix::fs::symlink(root.join("nonexistent_target"), &broken).unwrap();
        }

        let ignore = ignore(&["lock.tmp"], &[]);
        let latest = latest_mtime_under(&root, &ignore);
        assert_eq!(
            latest, 300.0,
            "blocked file (900.0) and unstatable entry excluded; newest good file (300.0, nested) wins"
        );
    }

    #[test]
    fn latest_mtime_under_is_zero_for_a_missing_directory() {
        let ignore = ignore(&[], &[]);
        assert_eq!(
            latest_mtime_under(std::path::Path::new("/does/not/exist"), &ignore),
            0.0
        );
    }
}
