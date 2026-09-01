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
use std::time::{Duration, Instant};

/// One running game, as handed to the UI. Serialized straight to the
/// frontend, so field names are part of the IPC contract.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GameSession {
    pub id: u64,
    pub rom_id: i64,
    pub title: String,
    pub emulator_name: String,
    /// Unix seconds at spawn time.
    pub started_at: i64,
    pub pid: u32,
}

/// The running-game list plus an optional one-shot message (today: the
/// early-exit warning). `warning` is `None` for every snapshot except the one
/// the 500 ms post-launch check emits for a game that died immediately.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionsSnapshot {
    pub sessions: Vec<GameSession>,
    pub warning: Option<String>,
}

/// A live session: the public record, the child handle that backs it, and
/// the moment it was registered (monotonic, so the early-exit window can be
/// measured; `GameSession::started_at` is only second-resolution).
struct Entry {
    info: GameSession,
    child: Child,
    registered: Instant,
}

/// The live-session table. One mutex guards both the records and the child
/// handles, so "is this rom running?" and "reap it" can never disagree.
#[derive(Default)]
pub(crate) struct SessionStore {
    entries: Mutex<Vec<Entry>>,
}

impl SessionStore {
    /// Adds `info`/`child` unless a session for `info.rom_id` is already
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
        if entries.iter().any(|e| e.info.rom_id == info.rom_id) {
            return Err(child);
        }
        entries.push(Entry {
            info,
            child,
            registered: Instant::now(),
        });
        Ok(())
    }

    /// Whether a session for `rom_id` is live.
    pub(crate) fn contains_rom(&self, rom_id: i64) -> bool {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.info.rom_id == rom_id)
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

    /// `try_wait`s every child registered more than `min_age` ago and drops
    /// the ones that have exited, returning `(session id, exit status)` for
    /// each. This is the only place a session is removed, so a pid can never
    /// be reaped while another caller still believes the session is live.
    ///
    /// `min_age` is what keeps the 500 ms early-exit check meaningful: the
    /// background loop passes the early-exit delay, so it never reaps a
    /// brand-new child out from under the check that owes the user a warning
    /// for it. The check itself passes `Duration::ZERO`.
    pub(crate) fn reap(&self, min_age: Duration) -> Vec<(u64, ExitStatus)> {
        let mut entries = self.entries.lock().unwrap();
        let mut exited = Vec::new();
        let now = Instant::now();
        entries.retain_mut(|entry| {
            if now.duration_since(entry.registered) < min_age {
                return true;
            }
            match entry.child.try_wait() {
                Ok(Some(status)) => {
                    exited.push((entry.info.id, status));
                    false
                }
                // A `try_wait` error means the child is unusable; treat it as
                // gone rather than leaking a row nothing will ever remove. The
                // status is unavailable, so nothing is reported for it.
                Err(_) => false,
                Ok(None) => true,
            }
        });
        exited
    }

    /// The pid of `session_id`, if it is still live.
    #[cfg(unix)]
    pub(crate) fn pid_of(&self, session_id: u64) -> Option<u32> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.info.id == session_id)
            .map(|e| e.info.pid)
    }

    /// Kills `session_id`'s child through its handle. Windows only: there is
    /// no pid-addressed terminate in `std`, so this has to run with the store
    /// lock held (`Child::kill` needs `&mut Child`). The call does not block
    /// on the process exiting, so the lock is held only briefly.
    #[cfg(windows)]
    pub(crate) fn kill(&self, session_id: u64) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.info.id == session_id) {
            let _ = entry.child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn session(id: u64, rom_id: i64, pid: u32) -> GameSession {
        GameSession {
            id,
            rom_id,
            title: "Chrono".to_string(),
            emulator_name: "Stub".to_string(),
            started_at: 1,
            pid,
        }
    }

    #[cfg(unix)]
    fn sleeper() -> Child {
        Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .unwrap()
    }

    #[cfg(unix)]
    fn quitter() -> Child {
        Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .spawn()
            .unwrap()
    }

    #[cfg(unix)]
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
        assert!(store.contains_rom(7));
        assert!(!store.contains_rom(9));

        // Kill both tracked children and reap them through the store.
        for id in [1, 3] {
            let pid = store.pid_of(id).unwrap();
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
        drain(&store);
        assert!(store.list().is_empty());
    }

    /// Reaps until the store is empty or the budget runs out.
    #[cfg(unix)]
    fn drain(store: &SessionStore) {
        for _ in 0..200 {
            store.reap(Duration::ZERO);
            if store.list().is_empty() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
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
            exited = store.reap(Duration::ZERO);
            if !exited.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].0, 2);
        assert_eq!(exited[0].1.code(), Some(7));
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].id, 1);
        assert_eq!(store.pid_of(1), Some(alive_pid));
        assert_eq!(store.pid_of(2), None);

        unsafe { libc::kill(alive_pid as i32, libc::SIGKILL) };
        drain(&store);
    }

    #[cfg(unix)]
    #[test]
    fn reap_leaves_children_younger_than_the_minimum_age_alone() {
        let store = SessionStore::default();
        let dead = quitter();
        let dead_pid = dead.id();
        assert!(store.register(session(1, 7, dead_pid), dead).is_ok());

        // Wait for the child to really be gone, then prove the age gate — not
        // the child's state — is what holds the session in place.
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(store.reap(Duration::from_secs(30)).is_empty());
        assert_eq!(store.list().len(), 1);

        let exited = store.reap(Duration::ZERO);
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].1.code(), Some(7));
        assert!(store.list().is_empty());
    }
}
