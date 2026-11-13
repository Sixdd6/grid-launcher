//! Running-game bookkeeping: the session records the UI sees and the store
//! that owns the live [`std::process::Child`] handles. Ports the session half
//! of `_register_game_session_for_auto_upload`
//! (`grid_launcher/ui/mixins/details_view_mixin.py:1472`); see
//! `docs/porting/04-emulator-launch.md` §8.
//!
//! Nothing in this module notifies anyone. Every method takes the store lock,
//! does its work, and releases it; emitting a snapshot to the listener is the
//! caller's job, always with no lock held.

use std::process::{Child, ExitStatus};
use std::sync::Mutex;

use crate::stores::GameKey;

/// One running game, as handed to the UI. Serialized straight to the
/// frontend, so field names are part of the IPC contract. The fields marked
/// `#[serde(skip)]` stay on the Rust side: the session-finished hooks read
/// them, the UI never sees them.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GameSession {
    pub id: u64,
    /// The game's identity (`romm:123`, `gog:1207664643`). One live
    /// session per key.
    pub key: GameKey,
    /// The RomM rom id, `None` for a store game. Play activity and the
    /// cloud auto-upload skip a session without one.
    pub rom_id: Option<i64>,
    pub title: String,
    pub emulator_name: String,
    /// Unix seconds at spawn time.
    pub started_at: i64,
    pub pid: u32,
    /// The installed row's platform. Play activity skips the Emulators
    /// platform with it.
    #[serde(skip)]
    pub platform: String,
    /// Unix milliseconds at spawn time. `started_at` is this value in seconds.
    #[serde(skip)]
    pub started_at_ms: i64,
    /// Unix milliseconds when the reaper removed the session. `None` while
    /// the game runs.
    #[serde(skip)]
    pub ended_at_ms: Option<i64>,
    /// `ended_at_ms - started_at_ms`. `None` while the game runs.
    #[serde(skip)]
    pub duration_ms: Option<i64>,
}

impl GameSession {
    /// This record, stamped as finished at `ended_at_ms` (unix ms). The
    /// duration is the wall-clock difference, so it agrees with the two
    /// timestamps even when it is negative (a clock that went back); play
    /// activity skips such a session.
    pub fn finished_at(mut self, ended_at_ms: i64) -> Self {
        self.ended_at_ms = Some(ended_at_ms);
        self.duration_ms = Some(ended_at_ms - self.started_at_ms);
        self
    }
}

/// Unix milliseconds now, `0` if the clock is before the epoch.
pub(crate) fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The running-game list plus an optional one-shot message (today: the
/// early-exit warning). `warning` is `None` for every snapshot except the one
/// the 500 ms post-launch check emits for a game that died immediately.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionsSnapshot {
    pub sessions: Vec<GameSession>,
    pub warning: Option<String>,
}

/// Answers whether an externally watched game is still running. Called by
/// every reap, under the session store's lock, so it must be cheap and must
/// not block (a `/proc` scan, not a network call).
pub type AliveCheck = Box<dyn FnMut() -> bool + Send>;

/// What backs a live session.
enum Tracked {
    /// A process GRID spawned: reaped through its handle.
    Child(Child),
    /// A process GRID did not spawn and holds no handle to (a Steam game,
    /// started by the Steam client): live while the check says so. Such a
    /// session has no pid GRID may signal, so `stop` never reaches it.
    Watched(AliveCheck),
}

/// A live session: the public record plus what backs it.
struct Entry {
    info: GameSession,
    tracked: Tracked,
}

/// The live-session table. One mutex guards both the records and the child
/// handles, so "is this rom running?" and "reap it" can never disagree.
#[derive(Default)]
pub(crate) struct SessionStore {
    entries: Mutex<Vec<Entry>>,
}

impl SessionStore {
    /// Adds `info`/`child` unless a session for `info.key` is already
    /// live. On rejection the child is handed back, because the caller — not
    /// this store — has to decide what to do with a process nobody is
    /// tracking (it kills it).
    ///
    /// The duplicate check is repeated here — `launch` also checks before it
    /// spawns — because that earlier check and this insert are not one
    /// critical section: two concurrent launches of the same rom both pass
    /// the pre-check, and only this one can reject the loser.
    pub(crate) fn register(&self, info: GameSession, child: Child) -> Result<(), Child> {
        let mut entries = self.entries.lock().unwrap();
        if entries.iter().any(|e| e.info.key == info.key) {
            return Err(child);
        }
        entries.push(Entry {
            info,
            tracked: Tracked::Child(child),
        });
        Ok(())
    }

    /// Adds an externally watched session ([`Tracked::Watched`]) unless a
    /// session for `info.key` is already live; on rejection the check is
    /// handed back. `info.pid` is display-only: it is never signalled.
    pub(crate) fn register_watched(
        &self,
        info: GameSession,
        alive: AliveCheck,
    ) -> Result<(), AliveCheck> {
        let mut entries = self.entries.lock().unwrap();
        if entries.iter().any(|e| e.info.key == info.key) {
            return Err(alive);
        }
        entries.push(Entry {
            info,
            tracked: Tracked::Watched(alive),
        });
        Ok(())
    }

    /// Whether a session for `key` is live.
    pub(crate) fn contains_key(&self, key: &GameKey) -> bool {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .any(|e| &e.info.key == key)
    }

    /// The current session records, in start order.
    pub(crate) fn list(&self) -> Vec<GameSession> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.info.clone())
            .collect()
    }

    /// `try_wait`s every child (and asks every watched session's check)
    /// and drops the ones that have exited,
    /// returning one entry per *removed* session. This is the only place a
    /// session is removed, so a pid can never be reaped while another caller
    /// still believes the session is live.
    ///
    /// The result distinguishes two things a caller needs to tell apart:
    ///
    /// - **the store changed** — the result is non-empty, so the caller owes
    ///   the listener a snapshot;
    /// - **a status is available** — the entry is `Some(status)`. `None`
    ///   means `try_wait` itself failed, which leaves the child unusable; it
    ///   is removed anyway rather than left as a row nothing will ever clear,
    ///   and the caller has no exit code to report for it.
    ///
    /// A child removed on the `try_wait` failure path is `wait`ed *after* the
    /// store lock is released, so it cannot become a zombie for the lifetime
    /// of the process. In practice such a failure is `ECHILD` — the child is
    /// already reaped — and the `wait` returns immediately; it is done off
    /// the lock so that even a pathological blocking `wait` cannot stall
    /// every other session.
    ///
    /// Carries the full [`GameSession`] record (not just its id): the
    /// session-finished hook `LaunchService` fires per reaped session needs
    /// `rom_id`/`title`/`started_at` to build the cloud auto-upload trigger,
    /// and once an entry is removed here that data is gone.
    pub(crate) fn reap(&self) -> Vec<(GameSession, Option<ExitStatus>)> {
        let mut exited = Vec::new();
        let mut unusable: Vec<Child> = Vec::new();
        // One stamp for the whole pass: every session removed here ended
        // at most one poll interval before it.
        let now_ms = unix_now_ms();
        {
            let mut entries = self.entries.lock().unwrap();
            let mut kept = Vec::with_capacity(entries.len());
            // Drained rather than retained so an unusable child can be moved
            // out of its entry and waited once the lock is gone. Order is
            // preserved: survivors are pushed back in the order they came.
            for mut entry in entries.drain(..) {
                let child = match &mut entry.tracked {
                    Tracked::Child(child) => child,
                    // A watched game has no exit status to report.
                    Tracked::Watched(alive) => {
                        if alive() {
                            kept.push(entry);
                        } else {
                            exited.push((entry.info.clone().finished_at(now_ms), None));
                        }
                        continue;
                    }
                };
                match child.try_wait() {
                    Ok(Some(status)) => {
                        exited.push((entry.info.clone().finished_at(now_ms), Some(status)))
                    }
                    Ok(None) => kept.push(entry),
                    Err(_) => {
                        exited.push((entry.info.clone().finished_at(now_ms), None));
                        if let Tracked::Child(child) = entry.tracked {
                            unusable.push(child);
                        }
                    }
                }
            }
            *entries = kept;
        }
        for mut child in unusable {
            let _ = child.wait();
        }
        exited
    }

    /// The pid of `session_id`, if it is still live and is a child GRID
    /// spawned. A watched session has no pid GRID may signal (its `pid`
    /// field may be `0`, and `kill(0, ..)` would hit GRID's own process
    /// group), so it answers `None`.
    #[cfg(unix)]
    pub(crate) fn pid_of(&self, session_id: u64) -> Option<u32> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.info.id == session_id)
            .filter(|e| matches!(e.tracked, Tracked::Child(_)))
            .map(|e| e.info.pid)
            .filter(|pid| *pid > 0)
    }

    /// Kills `session_id`'s child through its handle. Windows only: there is
    /// no pid-addressed terminate in `std`, so this has to run with the store
    /// lock held (`Child::kill` needs `&mut Child`). The call does not block
    /// on the process exiting, so the lock is held only briefly.
    #[cfg(windows)]
    pub(crate) fn kill(&self, session_id: u64) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.info.id == session_id) {
            if let Tracked::Child(child) = &mut entry.tracked {
                let _ = child.kill();
            }
        }
    }
}

#[cfg(test)]
mod finish_tests {
    use super::*;

    #[test]
    fn finished_at_records_the_end_and_the_duration() {
        let running = GameSession {
            id: 1,
            key: GameKey::Rom(7),
            rom_id: Some(7),
            started_at: 1_700_000_000,
            started_at_ms: 1_700_000_000_250,
            ..Default::default()
        };
        assert_eq!(running.ended_at_ms, None);
        assert_eq!(running.duration_ms, None);

        let done = running.finished_at(1_700_000_045_750);
        assert_eq!(done.ended_at_ms, Some(1_700_000_045_750));
        assert_eq!(done.duration_ms, Some(45_500));
        // None of the Rust-side fields reach JSON; the key is a string.
        assert_eq!(serde_json::to_value(&done).unwrap()["key"], "romm:7");
        let json = serde_json::to_value(&done).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "emulator_name",
                "id",
                "key",
                "pid",
                "rom_id",
                "started_at",
                "title"
            ]
        );
    }

    #[test]
    fn finished_at_keeps_a_negative_duration_for_the_skip_rule() {
        let done = GameSession {
            started_at_ms: 10_000,
            ..Default::default()
        }
        .finished_at(4_000);
        assert_eq!(done.ended_at_ms, Some(4_000));
        assert_eq!(done.duration_ms, Some(-6_000));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::Command;

    fn session(id: u64, rom_id: i64, pid: u32) -> GameSession {
        GameSession {
            id,
            key: GameKey::Rom(rom_id),
            rom_id: Some(rom_id),
            title: "Chrono".to_string(),
            emulator_name: "Stub".to_string(),
            started_at: 1,
            pid,
            ..Default::default()
        }
    }

    fn sleeper() -> Child {
        Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .unwrap()
    }

    fn quitter() -> Child {
        Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .spawn()
            .unwrap()
    }

    #[test]
    fn register_rejects_a_second_session_for_the_same_rom() {
        let store = SessionStore::default();
        let first = sleeper();
        let first_pid = first.id();
        let duplicate = sleeper();
        let duplicate_pid = duplicate.id();

        assert!(store.register(session(1, 7, first_pid), first).is_ok());
        let mut rejected = store
            .register(session(2, 7, duplicate_pid), duplicate)
            .expect_err("a second session for rom 7 must be refused");
        // The refused child comes back so the caller can clean it up.
        let _ = rejected.kill();
        let _ = rejected.wait();

        let other = sleeper();
        let other_pid = other.id();
        assert!(store.register(session(3, 8, other_pid), other).is_ok());

        assert_eq!(store.list().len(), 2);
        assert!(store.contains_key(&GameKey::Rom(7)));
        assert!(!store.contains_key(&GameKey::Rom(9)));

        // Kill both tracked children and reap them through the store.
        for id in [1, 3] {
            let pid = store.pid_of(id).unwrap();
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
        drain(&store);
        assert!(store.list().is_empty());
    }

    /// Reaps until the store is empty or the budget runs out.
    fn drain(store: &SessionStore) {
        for _ in 0..200 {
            store.reap();
            if store.list().is_empty() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn reap_removes_only_exited_children() {
        let store = SessionStore::default();
        let alive = sleeper();
        let alive_pid = alive.id();
        let dead = quitter();
        let dead_pid = dead.id();
        assert!(store.register(session(1, 7, alive_pid), alive).is_ok());
        assert!(store.register(session(2, 8, dead_pid), dead).is_ok());

        // Give the short-lived child time to actually exit.
        let mut exited = Vec::new();
        for _ in 0..200 {
            exited = store.reap();
            if !exited.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // One removal reported, with a status — the "store changed" and
        // "status available" halves of the result are both present.
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].0.id, 2);
        assert_eq!(exited[0].1.map(|status| status.code()), Some(Some(7)));
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].id, 1);
        assert_eq!(store.pid_of(1), Some(alive_pid));
        assert_eq!(store.pid_of(2), None);

        // A reap that removes nothing reports nothing, so a caller can use
        // "non-empty" as "the store changed".
        assert!(store.reap().is_empty());

        unsafe { libc::kill(alive_pid as i32, libc::SIGKILL) };
        drain(&store);
    }
}

#[cfg(test)]
mod watched_tests {
    use super::*;
    use crate::stores::StoreKind;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn steam_session(id: u64) -> GameSession {
        GameSession {
            id,
            key: GameKey::store(StoreKind::Steam, "620"),
            rom_id: None,
            title: "Portal 2".to_string(),
            emulator_name: "Steam".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn a_watched_session_lives_until_its_check_says_otherwise() {
        let store = SessionStore::default();
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        assert!(store
            .register_watched(
                steam_session(1),
                Box::new(move || flag.load(Ordering::SeqCst))
            )
            .is_ok());
        let key = GameKey::store(StoreKind::Steam, "620");
        assert!(store.contains_key(&key));
        assert!(!store.contains_key(&GameKey::Rom(620)));

        // A second session for the same key is refused, check handed back.
        let mut refused = store
            .register_watched(steam_session(2), Box::new(|| true))
            .expect_err("one session per key");
        assert!(refused());

        assert!(store.reap().is_empty(), "still running");
        assert_eq!(store.list().len(), 1);

        running.store(false, Ordering::SeqCst);
        let exited = store.reap();
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].0.id, 1);
        assert_eq!(exited[0].0.rom_id, None);
        assert!(exited[0].0.ended_at_ms.is_some());
        assert!(exited[0].1.is_none(), "no exit status for a watched game");
        assert!(store.list().is_empty());
        assert!(!store.contains_key(&key));
    }

    #[cfg(unix)]
    #[test]
    fn a_watched_session_has_no_pid_to_signal() {
        let store = SessionStore::default();
        let mut info = steam_session(9);
        info.pid = 0;
        store
            .register_watched(info, Box::new(|| true))
            .ok()
            .unwrap();
        assert_eq!(store.pid_of(9), None);
    }
}
