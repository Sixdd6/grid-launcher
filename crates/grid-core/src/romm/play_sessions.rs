//! `POST /api/play-sessions` (RomM "Ingest Play Sessions"): a batch call
//! that records finished play sessions. RomM also sets the user's
//! `rom_user.last_played` to a session's `end_time`, so this is how "last
//! played" reaches the server — `PUT /api/roms/{id}/props` is never used
//! (it also sets `now_playing` and overwrites the user's status).
//!
//! The server answers one result per entry, by index: `created`,
//! `duplicate` (an exact repeat — a retry is safe) or `error`.

use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

use super::error::excerpt;
use super::{RommClient, RommError};

/// One finished session, in the openapi `PlaySessionEntry` shape. The
/// timestamps are RFC 3339 UTC with a `Z` suffix; no `device_id` and no
/// `save_slot` are sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlaySessionEntry {
    pub rom_id: i64,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: i64,
}

/// The server's verdict on one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IngestStatus {
    Created,
    Duplicate,
    Error,
    /// A status this build does not know. The entry is kept and sent again.
    #[serde(other)]
    Unknown,
}

/// One entry's result. `index` points into the request's `sessions` array.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IngestResult {
    pub index: usize,
    pub status: IngestStatus,
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Deserialize)]
struct IngestResponse {
    results: Vec<IngestResult>,
}

/// The JSON body for `entries`: `{"sessions": [...]}`, nothing else.
pub fn play_sessions_body(entries: &[PlaySessionEntry]) -> serde_json::Value {
    serde_json::json!({ "sessions": entries })
}

impl RommClient {
    /// Sends `entries` in one `POST /api/play-sessions`. 401/403 map like
    /// every other call; any other non-2xx is `Http` with a body excerpt.
    pub async fn post_play_sessions(
        &self,
        entries: &[PlaySessionEntry],
    ) -> Result<Vec<IngestResult>, RommError> {
        let resp = self
            .http
            .post(self.endpoint("/api/play-sessions")?)
            .header(AUTHORIZATION, self.auth.clone())
            .json(&play_sessions_body(entries))
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if let Some(e) = self.auth_error(status) {
            return Err(e);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RommError::Http {
                status: status.as_u16(),
                excerpt: excerpt(&body),
            });
        }
        let parsed: IngestResponse = resp
            .json()
            .await
            .map_err(|e| RommError::Decode(e.without_url().to_string()))?;
        Ok(parsed.results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_body_is_a_sessions_array_with_z_timestamps_and_duration_ms() {
        let entries = vec![PlaySessionEntry {
            rom_id: 42,
            start_time: "2026-10-10T12:00:00.000Z".into(),
            end_time: "2026-10-10T12:00:45.500Z".into(),
            duration_ms: 45_500,
        }];
        assert_eq!(
            play_sessions_body(&entries),
            json!({
                "sessions": [{
                    "rom_id": 42,
                    "start_time": "2026-10-10T12:00:00.000Z",
                    "end_time": "2026-10-10T12:00:45.500Z",
                    "duration_ms": 45_500
                }]
            })
        );
    }

    #[test]
    fn results_parse_every_status_and_tolerate_a_new_one() {
        let parsed: IngestResponse = serde_json::from_value(json!({
            "results": [
                {"index": 0, "status": "created", "id": 9},
                {"index": 1, "status": "duplicate", "id": null},
                {"index": 2, "status": "error", "detail": "rom not found"},
                {"index": 3, "status": "queued"}
            ],
            "created_count": 1,
            "skipped_count": 2
        }))
        .unwrap();
        let statuses: Vec<IngestStatus> = parsed.results.iter().map(|r| r.status).collect();
        assert_eq!(
            statuses,
            vec![
                IngestStatus::Created,
                IngestStatus::Duplicate,
                IngestStatus::Error,
                IngestStatus::Unknown
            ]
        );
        assert_eq!(parsed.results[0].id, Some(9));
        assert_eq!(parsed.results[2].detail.as_deref(), Some("rom not found"));
    }
}
