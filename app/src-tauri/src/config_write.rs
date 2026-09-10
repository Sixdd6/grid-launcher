//! One writer at a time for `config.json`.
//!
//! Every config write in this app is a load-modify-save: read the whole
//! file, change one field, write the whole file back. Two of those
//! overlapping is a lost update — the second writer's `Config::load`
//! happens before the first writer's `Config::save`, so the first
//! writer's change is silently dropped when the second one saves.
//!
//! That is not hypothetical here. The D5 auto-upload pool runs up to two
//! background uploads at once, each stamping sync state through
//! [`crate::cloud_service`], while the UI can save a setting, an
//! emulator, or the library path at any moment. Nothing in the layers
//! below serializes them: `Config::save` is a plain whole-file write.
//!
//! [`modify_config`] is therefore the ONLY sanctioned way to change the
//! config on disk. It holds a process-wide lock across the whole
//! load-modify-save, so concurrent callers queue instead of racing, and
//! each one reads what the previous one wrote.
//!
//! The lock is a `std::sync::Mutex`, not a `tokio` one, because every
//! writer is synchronous and runs on the blocking pool
//! (`spawn_blocking`) — no `.await` ever happens while it is held. Work
//! that must happen AFTER the save (emulator autoconfig, the RA
//! credential fan-out) is deliberately left outside the closure: it does
//! not touch `config.json`, and holding the lock across it would stall
//! unrelated writers.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use grid_core::config::Config;

use crate::commands::err;

static CONFIG_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// A poisoned lock is recovered rather than propagated: the guarded data
/// is `()`, so a panicking writer leaves nothing inconsistent behind, and
/// refusing every later config save would be far worse than continuing.
fn lock() -> MutexGuard<'static, ()> {
    CONFIG_WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Load `path`, apply `change`, save it back — as one atomic step with
/// respect to every other `modify_config` caller in this process.
///
/// `change` returns whatever the caller needs from the config AFTER the
/// change (a name, a flag, a clone). Returning it, rather than reading
/// the config again outside, is what keeps post-save work off the lock
/// without reintroducing a second read that could see someone else's
/// write.
///
/// An error from `change` aborts the write: nothing is saved.
pub fn modify_config<T>(
    path: &Path,
    change: impl FnOnce(&mut Config) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = lock();
    let mut config = Config::load(path).map_err(err)?;
    let out = change(&mut config)?;
    config.save(path).map_err(err)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Two writers changing different fields at the same time must BOTH
    /// land. Each closure sleeps while it holds the config, so an
    /// unserialized implementation is guaranteed to have the second
    /// loader read a pre-change file and clobber the first change.
    #[test]
    fn two_concurrent_modify_config_calls_both_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        Config::default().save(&path).unwrap();

        let a = {
            let path = path.clone();
            thread::spawn(move || {
                modify_config(&path, |config| {
                    thread::sleep(Duration::from_millis(150));
                    config.library_path = "/library/from/a".to_string();
                    Ok(())
                })
                .unwrap();
            })
        };
        // Long enough that B is certainly inside the call while A holds
        // it, short enough that B still starts during A's sleep.
        thread::sleep(Duration::from_millis(30));
        let b = {
            let path = path.clone();
            thread::spawn(move || {
                modify_config(&path, |config| {
                    config.retroachievements_username = "player-b".to_string();
                    Ok(())
                })
                .unwrap();
            })
        };
        a.join().unwrap();
        b.join().unwrap();

        let saved = Config::load(&path).unwrap();
        assert_eq!(saved.library_path, "/library/from/a");
        assert_eq!(saved.retroachievements_username, "player-b");
    }

    #[test]
    fn an_error_from_the_closure_saves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let initial = Config {
            library_path: "/keep/me".to_string(),
            ..Default::default()
        };
        initial.save(&path).unwrap();

        let result: Result<(), String> = modify_config(&path, |config| {
            config.library_path = "/overwritten".to_string();
            Err("nope".to_string())
        });
        assert_eq!(result, Err("nope".to_string()));
        assert_eq!(Config::load(&path).unwrap().library_path, "/keep/me");
    }
}
