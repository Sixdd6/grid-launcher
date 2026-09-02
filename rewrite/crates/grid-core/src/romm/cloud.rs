//! RomM save/state endpoints: list, download, upload, delete.
//!
//! Ported from `grid_launcher/ui/mixins/cloud_mixin.py`:
//! `_server_save_records_for_rom` (:1592-1594), `_server_state_records_for_rom`
//! (:1650-1651), `_download_server_save_content` (:1774-1775),
//! `_download_server_state_content`'s record fetch (:1778-1779) and relative
//! candidate branch (:1787-1791), the upload query/multipart shape
//! (:2478-2479, :2606-2624), and `_prune_server_save_records`'s two delete
//! calls (:1745-1750). See `docs/porting/06-cloud-saves.md`, "Server
//! endpoints", for the full table this module implements verbatim.
//!
//! `saves_for_rom`/`states_for_rom` return the RAW JSON payload — record
//! parsing (`server_records_from_payload` and friends) is
//! [`super::super::cloud::restore`]'s job, not this module's; this keeps the
//! HTTP layer and the record-shape logic independently testable, matching
//! how the rest of `RommClient` is split.

use std::path::PathBuf;
use std::sync::LazyLock;

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::header::AUTHORIZATION;
use reqwest::multipart::{Form, Part};
use reqwest::StatusCode;
use serde_json::Value;

use super::error::excerpt;
use super::{RommClient, RommError};
use crate::cloud::transfer::normalize_candidate_url;

/// Percent-encodes an id the way Python's `quote(id, safe="")` does: the
/// RFC 3986 unreserved characters (letters, digits, `-._~`) are the ONLY
/// bytes left untouched — even `/` is escaped, unlike a path segment's
/// usual `safe="/%"`. Same character set as `cloud::transfer`'s private
/// `QUERY_ENCODE_SET` (kept as its own local static here rather than
/// exported, matching that module's own precedent of small local
/// duplication over cross-module plumbing for a one-off `AsciiSet`).
static ID_ENCODE_SET: LazyLock<AsciiSet> = LazyLock::new(|| {
    NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~')
});

fn encode_id(id: &str) -> String {
    utf8_percent_encode(id, &ID_ENCODE_SET).to_string()
}

/// Reads every payload file into memory and builds a `multipart::Form`
/// preserving payload order (main file first, optional screenshot
/// sidecar second) — mirrors `multipart_payload`'s dict-iteration order
/// (`grid_launcher/core/api.py:88-108`; Python dicts are insertion-ordered).
/// Field names come straight from `payload`; each part's file name is the
/// path's own file name, matching `Content-Disposition: ...;
/// filename="{file_path.name}"` there. Payloads are small (a save/state
/// file plus an optional screenshot), so reading them fully into memory
/// up front — rather than streaming — mirrors the Python client's own
/// `file_path.read_bytes()` and keeps this synchronous with no extra
/// tokio `fs` feature.
fn build_multipart_form(payload: &[(String, PathBuf)]) -> Result<Form, RommError> {
    let mut form = Form::new();
    for (field, path) in payload {
        let bytes = std::fs::read(path)
            .map_err(|e| RommError::Connection(format!("failed to read upload payload: {e}")))?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let part = Part::bytes(bytes).file_name(file_name);
        form = form.part(field.clone(), part);
    }
    Ok(form)
}

impl RommClient {
    /// `GET /api/saves?rom_id=` (`cloud_mixin.py:1593`). Returns the raw
    /// JSON payload — see this module's doc comment for why record parsing
    /// lives elsewhere.
    pub async fn saves_for_rom(&self, rom_id: &str) -> Result<Value, RommError> {
        self.get_json("/api/saves", &[("rom_id", rom_id.to_string())])
            .await
    }

    /// `GET /api/states?rom_id=` (`cloud_mixin.py:1651`).
    pub async fn states_for_rom(&self, rom_id: &str) -> Result<Value, RommError> {
        self.get_json("/api/states", &[("rom_id", rom_id.to_string())])
            .await
    }

    /// `GET /api/saves/{id}/content` (`cloud_mixin.py:1774-1775`), `id`
    /// percent-encoded with `safe=""`.
    pub async fn save_content(&self, id: &str) -> Result<Vec<u8>, RommError> {
        self.get_bytes(&format!("/api/saves/{}/content", encode_id(id)))
            .await
    }

    /// `GET /api/states/{id}` (`cloud_mixin.py:1779`), `id` percent-encoded
    /// with `safe=""`. Used only to read the record's own download
    /// candidate paths — the actual content fetch is a follow-up call.
    pub async fn state_record(&self, id: &str) -> Result<Value, RommError> {
        self.get_json(&format!("/api/states/{}", encode_id(id)), &[])
            .await
    }

    /// Fetches ONE server-relative download candidate: mirrors
    /// `cloud_mixin.py:1789-1791`'s relative branch — a leading `/` is
    /// added when missing, then the result is run through
    /// [`normalize_candidate_url`] before the GET.
    ///
    /// D4: an absolute `http(s)://` candidate is rejected with `Err`
    /// rather than fetched — that shape takes an entirely different code
    /// path in Python (`cloud_mixin.py:1786-1788`: a directly `urlopen`'d
    /// authorized GET against the absolute URL itself, no server-relative
    /// prefixing), which is out of this task's scope. The caller (a future
    /// ops-layer task, mirroring Python's per-candidate
    /// `try/except ...: continue` loop at :1785-1794) is expected to try
    /// each candidate path in turn and move to the next one on any `Err`
    /// from this function — an absolute candidate simply needs its own,
    /// different fetch path rather than being silently mishandled here.
    /// Reuses [`RommError::InvalidUrl`] for this rejection: no dedicated
    /// variant exists for "not a relative candidate", and the distinction
    /// is invisible past the caller's `continue` regardless.
    pub async fn get_relative_bytes(&self, candidate: &str) -> Result<Vec<u8>, RommError> {
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            return Err(RommError::InvalidUrl);
        }
        let relative = if candidate.starts_with('/') {
            candidate.to_string()
        } else {
            format!("/{candidate}")
        };
        self.get_bytes(&normalize_candidate_url(&relative)).await
    }

    /// `POST /api/saves` multipart (`cloud_mixin.py:2478-2479,2606-2624`):
    /// query `rom_id`, `emulator`, `overwrite=true` (the literal string,
    /// matching Python's `params["overwrite"] = "true"`), and `slot` ONLY
    /// when `slot` is `Some` and non-empty (`cloud_mixin.py:2610-2612`'s
    /// `if slot_value:` truthiness check).
    pub async fn upload_save(
        &self,
        rom_id: &str,
        emulator: &str,
        slot: Option<&str>,
        payload: &[(String, PathBuf)],
    ) -> Result<(), RommError> {
        let mut query: Vec<(&str, String)> = vec![
            ("rom_id", rom_id.to_string()),
            ("emulator", emulator.to_string()),
            ("overwrite", "true".to_string()),
        ];
        if let Some(slot) = slot {
            if !slot.is_empty() {
                query.push(("slot", slot.to_string()));
            }
        }
        self.post_multipart("/api/saves", &query, payload).await
    }

    /// `POST /api/states` multipart (`cloud_mixin.py:2478-2479,2606-2624`):
    /// query `rom_id`, `emulator` only — NO `slot`, NO `overwrite`. States
    /// never carry those two params; the Python upload code only adds them
    /// inside the `save_type == "save"` branch (`cloud_mixin.py:2609-2613`).
    pub async fn upload_state(
        &self,
        rom_id: &str,
        emulator: &str,
        payload: &[(String, PathBuf)],
    ) -> Result<(), RommError> {
        let query: Vec<(&str, String)> = vec![
            ("rom_id", rom_id.to_string()),
            ("emulator", emulator.to_string()),
        ];
        self.post_multipart("/api/states", &query, payload).await
    }

    async fn post_multipart(
        &self,
        path: &str,
        query: &[(&str, String)],
        payload: &[(String, PathBuf)],
    ) -> Result<(), RommError> {
        let form = build_multipart_form(payload)?;
        let resp = self
            .http
            .post(self.endpoint(path)?)
            .query(query)
            .header(AUTHORIZATION, self.auth.clone())
            .multipart(form)
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(RommError::Unauthorized);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RommError::Http {
                status: status.as_u16(),
                excerpt: excerpt(&body),
            });
        }
        Ok(())
    }

    /// `POST /api/saves/delete {"saves": [id]}` (`cloud_mixin.py:1745-
    /// 1750`). Returns the raw status — UNLIKE every other client method,
    /// a non-2xx status here is NOT converted to `Err(Http)`, because
    /// retention pruning (`cloud::retention::prune_server_save_records`)
    /// needs to tell 404/410 (treated as a successful delete —
    /// `cloud_mixin.py:1752-1758`) apart from every other status (a
    /// failure — :1759-1765) — a distinction `Err` alone can't carry once
    /// collapsed to one variant. 401/403 are the one exception: they still
    /// map to [`RommError::Unauthorized`], matching every other method
    /// (Python's `HTTPError` there isn't in `{404, 410}` either, so it
    /// falls into the same "failed" bucket the caller already handles —
    /// this just gives that specific case a sharper error instead of a
    /// bare status code).
    pub async fn delete_save(&self, id: i64) -> Result<u16, RommError> {
        self.post_delete("/api/saves/delete", serde_json::json!({"saves": [id]}))
            .await
    }

    /// `POST /api/states/delete {"states": [id]}`
    /// (`grid_launcher/ui/mixins/details_view_mixin.py:1303`). Same status
    /// contract as [`Self::delete_save`].
    pub async fn delete_state(&self, id: i64) -> Result<u16, RommError> {
        self.post_delete("/api/states/delete", serde_json::json!({"states": [id]}))
            .await
    }

    async fn post_delete(&self, path: &str, body: Value) -> Result<u16, RommError> {
        let resp = self
            .http
            .post(self.endpoint(path)?)
            .header(AUTHORIZATION, self.auth.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(RommError::Unauthorized);
        }
        Ok(status.as_u16())
    }
}
