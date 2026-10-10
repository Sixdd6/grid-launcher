//! App-layer play activity (Q10). grid-core decides WHAT a finished session
//! sends and how the outbox settles (`grid_core::play_activity`); this
//! module decides WHEN that runs:
//!
//! - **a game session ends** — the session-finished listener
//!   ([`PlaySessionService::install_session_finished_hook`]) queues a
//!   qualifying session in the registry outbox, then tries a flush;
//! - **a session comes up** — `connect`, `restore_session` and
//!   `retry_connect` call [`PlaySessionService::spawn_flush`] on success, so
//!   sessions queued offline (or in an earlier run) go out.
//!
//! Always on; there is no setting. Nothing here blocks the UI: every
//! trigger spawns a task and returns. With no live client the queue simply
//! waits. Flushes run one at a time.
//!
//! Token secrecy: logs carry counts and the `RommError` display, which is
//! credential-free by construction — never a URL, a header or a token.

use std::sync::Arc;

use grid_core::launch::{GameSession, LaunchService};
use grid_core::library::registry::Registry;
use grid_core::play_activity::{entry_for_session, flush, MIN_PLAY_SESSION_MS};
use grid_core::session::SessionManager;
use tokio::sync::Mutex as AsyncMutex;

/// The `e2e` build's override for the minimum session length, in whole
/// seconds. The `launch` stage group sets it (through `wdio.conf.ts`) so a
/// stub game that runs for a moment still counts; a release build never
/// reads it.
const E2E_MIN_SECS_VAR: &str = "GRID_LAUNCHER_E2E_PLAY_SESSION_MIN_SECS";

/// The minimum session length in milliseconds: [`MIN_PLAY_SESSION_MS`],
/// unless this is an `e2e` build and `override_secs` parses as a
/// non-negative whole number of seconds.
fn min_duration_ms(e2e: bool, override_secs: Option<&str>) -> i64 {
    if !e2e {
        return MIN_PLAY_SESSION_MS;
    }
    override_secs
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|secs| i64::from(secs) * 1000)
        .unwrap_or(MIN_PLAY_SESSION_MS)
}

pub struct PlaySessionService {
    /// One flush at a time. A second trigger waits and then sends whatever
    /// the first left behind (often nothing).
    flush_gate: AsyncMutex<()>,
    min_duration_ms: i64,
}

impl PlaySessionService {
    pub fn new() -> Arc<Self> {
        let override_secs = std::env::var(E2E_MIN_SECS_VAR).ok();
        Arc::new(Self {
            flush_gate: AsyncMutex::new(()),
            min_duration_ms: min_duration_ms(cfg!(feature = "e2e"), override_secs.as_deref()),
        })
    }

    /// Adds the play-activity listener on `launch`. Called once from
    /// `lib.rs`'s `.setup()`. The listener is a plain closure (the
    /// `LaunchService` contract), so it hands the work to
    /// `tauri::async_runtime::spawn` and returns at once.
    pub fn install_session_finished_hook(
        self: &Arc<Self>,
        launch: &Arc<LaunchService>,
        session_mgr: Arc<SessionManager>,
        registry: Arc<Registry>,
    ) {
        let service = self.clone();
        launch.add_session_finished_hook(Arc::new(move |session: GameSession| {
            let Some(entry) = entry_for_session(&session, service.min_duration_ms) else {
                return;
            };
            let service = service.clone();
            let session_mgr = session_mgr.clone();
            let registry = registry.clone();
            tauri::async_runtime::spawn(async move {
                let reg = registry.clone();
                let queued =
                    tokio::task::spawn_blocking(move || reg.enqueue_play_session(&entry)).await;
                match queued {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        tracing::warn!("play activity: could not queue a session: {e}");
                        return;
                    }
                    Err(e) => {
                        tracing::warn!("play activity: the queue task did not finish: {e}");
                        return;
                    }
                }
                service.flush_now(&session_mgr, registry).await;
            });
        }));
    }

    /// Flushes the outbox in the background. Returns at once.
    pub fn spawn_flush(
        self: &Arc<Self>,
        session_mgr: Arc<SessionManager>,
        registry: Arc<Registry>,
    ) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            service.flush_now(&session_mgr, registry).await;
        });
    }

    /// Sends the queue when there is a live client; does nothing offline.
    async fn flush_now(&self, session_mgr: &SessionManager, registry: Arc<Registry>) {
        let Some(client) = session_mgr.client() else {
            return;
        };
        let _gate = self.flush_gate.lock().await;
        match flush(registry, &client).await {
            Ok(report) => {
                if report.accepted > 0 || report.dropped > 0 {
                    tracing::info!(
                        accepted = report.accepted,
                        dropped = report.dropped,
                        "play activity: sessions sent to the server"
                    );
                }
                if let Some(reason) = report.stopped {
                    tracing::debug!("play activity: queue kept for later: {reason}");
                }
            }
            Err(e) => tracing::warn!("play activity: queue read failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_build_always_uses_thirty_seconds() {
        assert_eq!(min_duration_ms(false, None), 30_000);
        assert_eq!(min_duration_ms(false, Some("0")), 30_000);
    }

    #[test]
    fn an_e2e_build_takes_a_whole_second_override() {
        assert_eq!(min_duration_ms(true, Some("0")), 0);
        assert_eq!(min_duration_ms(true, Some(" 2 ")), 2_000);
        assert_eq!(min_duration_ms(true, None), 30_000);
        assert_eq!(min_duration_ms(true, Some("-1")), 30_000);
        assert_eq!(min_duration_ms(true, Some("soon")), 30_000);
    }
}
