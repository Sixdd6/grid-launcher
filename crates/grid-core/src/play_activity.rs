//! Play activity (Q10): finished game sessions go to RomM's play-session
//! ingest, which also moves the user's "last played" for that ROM to the
//! session end. Always on; there is no setting.
//!
//! The flow is an outbox. A finished session that qualifies
//! ([`entry_for_session`]) is written to the registry's
//! `pending_play_sessions` table first, and [`flush`] sends the queue in
//! batches whenever the app has a live client. Offline, the rows wait.
//!
//! What happens to a row after a send ([`settle_batch`]):
//! - `created` or `duplicate` — the server has it: delete the row.
//! - per-entry `error`, or the whole batch answered 4xx (not 401/403) — the
//!   server will never accept it: drop the row and log it.
//! - a network error, a 5xx, an unreadable answer, 401 or 403 — keep the
//!   row and stop; the next flush sends it again. A repeat is safe because
//!   the server answers an exact repeat with `duplicate`.
//!
//! Logs carry counts and rom ids only — never a URL, a header or a token.

use std::sync::Arc;

use crate::launch::GameSession;
use crate::library::registry::{PendingPlaySession, Registry};
use crate::library::update_detection::is_emulators_platform;
use crate::library::LibraryError;
use crate::romm::{IngestResult, IngestStatus, PlaySessionEntry, RommClient, RommError};

/// Sessions shorter than this are not play activity (a crash at start, a
/// quick look at a menu).
pub const MIN_PLAY_SESSION_MS: i64 = 30_000;

/// Entries per `POST /api/play-sessions`.
pub const FLUSH_BATCH_SIZE: usize = 50;

/// Unix milliseconds as RFC 3339 UTC with millisecond precision and a `Z`
/// suffix (`2026-10-10T12:00:00.250Z`). `None` when out of chrono's range.
pub fn rfc3339_utc(ms: i64) -> Option<String> {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

/// The ingest entry for a finished session, or `None` when the session is
/// not play activity: no rom id, the Emulators platform, not finished, a
/// start or end that is not a valid time, an end before the start, a zero
/// or negative duration, or a duration under `min_duration_ms`.
pub fn entry_for_session(session: &GameSession, min_duration_ms: i64) -> Option<PlaySessionEntry> {
    if session.rom_id <= 0 || is_emulators_platform(&session.platform) {
        return None;
    }
    let start = session.started_at_ms;
    let end = session.ended_at_ms?;
    let duration_ms = session.duration_ms?;
    if start <= 0 || end < start || duration_ms <= 0 || duration_ms < min_duration_ms {
        return None;
    }
    Some(PlaySessionEntry {
        rom_id: session.rom_id,
        start_time: rfc3339_utc(start)?,
        end_time: rfc3339_utc(end)?,
        duration_ms,
    })
}

/// What to do with one sent batch's rows. `accepted` and `dropped` are both
/// deleted; every other row stays queued. `stop` ends this flush.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BatchSettlement {
    pub accepted: Vec<i64>,
    pub dropped: Vec<i64>,
    pub stop: bool,
}

/// Decides each row's fate from the send result. `ids[i]` is the row id
/// of the batch's entry `i`.
pub fn settle_batch(ids: &[i64], result: &Result<Vec<IngestResult>, RommError>) -> BatchSettlement {
    let mut settled = BatchSettlement::default();
    match result {
        Ok(results) => {
            for r in results {
                let Some(&id) = ids.get(r.index) else {
                    continue;
                };
                match r.status {
                    IngestStatus::Created | IngestStatus::Duplicate => settled.accepted.push(id),
                    IngestStatus::Error => settled.dropped.push(id),
                    IngestStatus::Unknown => {}
                }
            }
        }
        // A request the server rejects as a whole will be rejected again.
        // 401 and 403 are their own variants, so they never land here.
        Err(RommError::Http { status, .. }) if (400..500).contains(status) => {
            settled.dropped.extend_from_slice(ids);
        }
        Err(_) => settled.stop = true,
    }
    settled
}

/// What one [`flush`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FlushReport {
    /// Rows the server now has (`created` or `duplicate`).
    pub accepted: usize,
    /// Rows the server refused for good.
    pub dropped: usize,
    /// Why the flush stopped early, if it did. Credential-free: it is a
    /// `RommError` display.
    pub stopped: Option<String>,
}

/// Sends every queued row, oldest first, in batches of
/// [`FLUSH_BATCH_SIZE`], and deletes the rows [`settle_batch`] settles.
/// Registry work runs on the blocking pool. Callers serialize flushes; two
/// at once would only produce `duplicate` answers, never a double count.
pub async fn flush(
    registry: Arc<Registry>,
    client: &RommClient,
) -> Result<FlushReport, LibraryError> {
    let mut report = FlushReport::default();
    let mut after_id = 0;
    loop {
        let reg = registry.clone();
        let batch: Vec<PendingPlaySession> = tokio::task::spawn_blocking(move || {
            reg.pending_play_sessions(after_id, FLUSH_BATCH_SIZE)
        })
        .await
        .map_err(join_err)??;
        let Some(last) = batch.last() else {
            break;
        };
        after_id = last.id;

        let ids: Vec<i64> = batch.iter().map(|p| p.id).collect();
        let entries: Vec<PlaySessionEntry> = batch.into_iter().map(|p| p.entry).collect();
        let result = client.post_play_sessions(&entries).await;
        let settled = settle_batch(&ids, &result);

        for (id, entry) in ids.iter().zip(&entries) {
            if settled.dropped.contains(id) {
                tracing::warn!(
                    rom_id = entry.rom_id,
                    "play session refused by the server; dropped from the queue"
                );
            }
        }
        report.accepted += settled.accepted.len();
        report.dropped += settled.dropped.len();

        let done: Vec<i64> = settled
            .accepted
            .iter()
            .chain(&settled.dropped)
            .copied()
            .collect();
        if !done.is_empty() {
            let reg = registry.clone();
            tokio::task::spawn_blocking(move || reg.delete_pending_play_sessions(&done))
                .await
                .map_err(join_err)??;
        }
        if settled.stop {
            report.stopped = result.err().map(|e| e.to_string());
            break;
        }
    }
    Ok(report)
}

fn join_err(e: tokio::task::JoinError) -> LibraryError {
    LibraryError::Registry(format!("the outbox task did not finish: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START_MS: i64 = 1_760_097_600_000; // 2025-10-10T12:00:00Z

    fn finished(duration_ms: i64) -> GameSession {
        GameSession {
            id: 1,
            rom_id: 42,
            title: "Chrono".into(),
            emulator_name: "Stub".into(),
            started_at: START_MS / 1000,
            platform: "Super Nintendo Entertainment System".into(),
            started_at_ms: START_MS,
            ..Default::default()
        }
        .finished_at(START_MS + duration_ms)
    }

    #[test]
    fn timestamps_are_utc_with_a_z_suffix() {
        assert_eq!(
            rfc3339_utc(START_MS).as_deref(),
            Some("2025-10-10T12:00:00.000Z")
        );
        assert_eq!(
            rfc3339_utc(START_MS + 45_250).as_deref(),
            Some("2025-10-10T12:00:45.250Z")
        );
    }

    #[test]
    fn a_qualifying_session_becomes_an_entry() {
        assert_eq!(
            entry_for_session(&finished(45_500), MIN_PLAY_SESSION_MS),
            Some(PlaySessionEntry {
                rom_id: 42,
                start_time: "2025-10-10T12:00:00.000Z".into(),
                end_time: "2025-10-10T12:00:45.500Z".into(),
                duration_ms: 45_500,
            })
        );
        // Exactly the threshold counts.
        assert!(entry_for_session(&finished(30_000), MIN_PLAY_SESSION_MS).is_some());
    }

    #[test]
    fn skip_rules() {
        let mut no_rom = finished(60_000);
        no_rom.rom_id = 0;
        let mut emulators = finished(60_000);
        emulators.platform = " emulators ".into();
        let running = GameSession {
            ended_at_ms: None,
            duration_ms: None,
            ..finished(60_000)
        };
        let mut no_start = finished(60_000);
        no_start.started_at_ms = 0;
        let mut end_before_start = finished(60_000);
        end_before_start.ended_at_ms = Some(START_MS - 1);
        let mut inconsistent = finished(60_000);
        inconsistent.duration_ms = Some(-60_000);

        let cases: Vec<(&str, GameSession, i64)> = vec![
            ("under the threshold", finished(29_999), MIN_PLAY_SESSION_MS),
            ("zero duration", finished(0), 0),
            ("negative duration", finished(-5_000), 0),
            ("no rom id", no_rom, MIN_PLAY_SESSION_MS),
            ("Emulators platform", emulators, MIN_PLAY_SESSION_MS),
            ("still running", running, MIN_PLAY_SESSION_MS),
            ("no start time", no_start, MIN_PLAY_SESSION_MS),
            ("end before start", end_before_start, MIN_PLAY_SESSION_MS),
            (
                "negative recorded duration",
                inconsistent,
                MIN_PLAY_SESSION_MS,
            ),
        ];
        for (label, session, min) in cases {
            assert_eq!(entry_for_session(&session, min), None, "{label}");
        }
    }

    #[test]
    fn a_lowered_threshold_still_needs_a_positive_duration() {
        assert!(entry_for_session(&finished(1), 0).is_some());
        assert!(entry_for_session(&finished(0), 0).is_none());
    }

    fn result(index: usize, status: IngestStatus) -> IngestResult {
        IngestResult {
            index,
            status,
            id: None,
            detail: None,
        }
    }

    #[test]
    fn created_and_duplicate_are_accepted_error_is_dropped_unknown_is_kept() {
        let ids = [10, 11, 12, 13];
        let settled = settle_batch(
            &ids,
            &Ok(vec![
                result(0, IngestStatus::Created),
                result(1, IngestStatus::Duplicate),
                result(2, IngestStatus::Error),
                result(3, IngestStatus::Unknown),
            ]),
        );
        assert_eq!(
            settled,
            BatchSettlement {
                accepted: vec![10, 11],
                dropped: vec![12],
                stop: false,
            }
        );
    }

    #[test]
    fn a_missing_or_out_of_range_result_keeps_the_row() {
        let settled = settle_batch(
            &[10, 11],
            &Ok(vec![
                result(1, IngestStatus::Created),
                result(7, IngestStatus::Created),
            ]),
        );
        assert_eq!(settled.accepted, vec![11]);
        assert!(settled.dropped.is_empty());
        assert!(!settled.stop);
    }

    #[test]
    fn transport_server_and_auth_failures_keep_every_row_and_stop() {
        let failures = [
            RommError::Connection("refused".into()),
            RommError::Http {
                status: 500,
                excerpt: String::new(),
            },
            RommError::Http {
                status: 503,
                excerpt: String::new(),
            },
            RommError::Decode("eof".into()),
            RommError::Unauthorized,
            RommError::Forbidden,
        ];
        for failure in failures {
            let label = failure.to_string();
            let settled = settle_batch(&[10, 11], &Err(failure));
            assert_eq!(
                settled,
                BatchSettlement {
                    accepted: vec![],
                    dropped: vec![],
                    stop: true,
                },
                "{label}"
            );
        }
    }

    #[test]
    fn a_client_error_drops_the_whole_batch() {
        for status in [400u16, 404, 422] {
            let settled = settle_batch(
                &[10, 11],
                &Err(RommError::Http {
                    status,
                    excerpt: String::new(),
                }),
            );
            assert_eq!(
                settled,
                BatchSettlement {
                    accepted: vec![],
                    dropped: vec![10, 11],
                    stop: false,
                },
                "{status}"
            );
        }
    }
}
