//! Shared env-var lock for every test in this crate that reads or mutates a
//! process environment variable.
//!
//! Environment variables are process-global, so two tests touching the same
//! variable concurrently race — and per `std::env::set_var`'s safety
//! contract, a set/remove racing with *any* read (even a plain read in an
//! unrelated module's test) is undefined behavior, not just flakiness. Every
//! such test, in every module, must serialize on the ONE lock here rather
//! than a module-local one, or two module-local locks give each other no
//! protection at all.

#![cfg(test)]

use std::sync::{Mutex, MutexGuard, PoisonError};

/// The crate-wide env-test lock. Call [`lock`] rather than using this
/// directly so a poisoned lock (an earlier test panicking mid-mutation)
/// doesn't brick every later test.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the crate-wide env-test lock, held for the caller's entire
/// env-touching section (construct any [`EnvGuard`] after this and drop it
/// before releasing the returned guard). Poison-tolerant: a panic in an
/// earlier test while holding the lock must not fail every later test.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Sets each `(var, value)` pair — `None` removes the variable — for the
/// guard's lifetime and restores whatever preceded it on drop, so a panic
/// mid-test can never leak an override into another test.
///
/// Callers must hold the lock returned by [`lock`] for the guard's entire
/// lifetime.
pub(crate) struct EnvGuard {
    previous: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    pub(crate) fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
        let previous = pairs
            .iter()
            .map(|&(var, _)| (var, std::env::var(var).ok()))
            .collect();
        for &(var, value) in pairs {
            match value {
                // SAFETY: the crate-wide `ENV_LOCK` is held for this guard's
                // entire lifetime by every caller (see `lock()`).
                Some(v) => unsafe { std::env::set_var(var, v) },
                None => unsafe { std::env::remove_var(var) },
            }
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (var, value) in &self.previous {
            match value {
                // SAFETY: see `EnvGuard::set` above.
                Some(v) => unsafe { std::env::set_var(var, v) },
                None => unsafe { std::env::remove_var(var) },
            }
        }
    }
}
