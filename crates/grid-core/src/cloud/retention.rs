//! Client-side retention pruning of server save records.
//!
//! Ported from `_prune_server_save_records`
//! (`grid_launcher/ui/mixins/cloud_mixin.py:1676-1765`). See
//! `docs/porting/06-cloud-saves.md`, "Retention pruning", for the
//! narrative six-step version this module implements verbatim. There is
//! no client-side pruning of states (doc 06, same section) — hence this
//! module has only the one save-side entry point.

use std::collections::HashMap;

use serde_json::Value;

use crate::romm::RommClient;

use super::restore::{
    id_rank, record_timestamp, server_records_from_payload, slot_dedupe_key, stringify_id,
};

/// `_prune_server_save_records(rom_id, emulator_name, keep_latest)`
/// (`cloud_mixin.py:1676-1765`). Steps, in order:
///
/// 1. `keep = max(1, keep)` (:1677).
/// 2. Refetch `GET /api/saves?rom_id=` via [`RommClient::saves_for_rom`],
///    parsed through [`server_records_from_payload`] — the SAME function
///    `saves_for_rom`'s Python counterpart (`_server_save_records_for_rom`)
///    already runs, so a blank-id record is dropped right here, before
///    this function's own loop ever sees it (see the "BLANK id" note on
///    step 6 below).
/// 3. Keep records whose `emulator` field matches `emulator_name`
///    case-insensitively; when `emulator_name` is blank, EVERY record
///    passes (:1682-1690's `not emulator_key or (...)"`) — this is
///    DIFFERENT from [`super::restore::latest_server_record`]'s "fall back
///    to all when NOTHING matches" rule: here, a non-blank
///    `emulator_name` that matches nothing prunes nothing at all.
/// 4. Sort by `(timestamp, numeric id)` descending (:1701), reusing
///    [`record_timestamp`] and [`id_rank`] exactly like
///    [`super::restore::sort_server_records_by_recency`]'s own key.
/// 5. Group by [`slot_dedupe_key`] (:1706-1719), first-seen group order
///    preserved.
/// 6. Within each group, everything after the first `keep` entries is
///    stale (:1721-1723). For each stale record: a BLANK id (after
///    stringify + trim) is silently skipped — counted in neither return
///    value, no request sent (:1734-1736; unreachable via this function's
///    OWN fetch path per step 2's note, but kept here as the same
///    defense-in-depth Python has, and the observable contract — never
///    requested, never counted — is identical either way). A
///    NON-INTEGER id is recorded as failed WITHOUT a request
///    (:1737-1741). Otherwise `POST /api/saves/delete {"saves": [id]}`
///    via [`RommClient::delete_save`]: HTTP 404 or 410 count as a
///    SUCCESSFUL deletion (:1752-1758); any other non-2xx status, or a
///    transport/auth error, records the id as failed and the loop
///    continues (:1759-1765).
///
/// Returns `(deleted_count, failed_ids)`. Fix round 1 (controller ruling): a
/// failure to refetch the record list itself (step 2) now returns
/// `(0, vec![err.to_string()])` — one synthetic failed-id entry holding the
/// error text, `deleted_count` staying `0`. This reproduces, INSIDE this
/// function, what Python's CALLER does with the propagated exception
/// (`cloud_mixin.py:2634-2641`: `except (...) as error:
/// retention_failed_ids = [str(error)]`), since that caller — the upload
/// flow that decides whether to prune at all — is a future ops-layer task
/// not yet wired up in the port; folding its exception handling in here
/// keeps the observable outcome identical regardless of which layer ends
/// up doing it, which matters because a later task (Task 16) consumes this
/// function's return value directly. `RommError`'s `Display` impl (see
/// `romm/error.rs`) never embeds the request or its headers, so this text
/// carries no secret — same guarantee the existing `Http`/`Decode`
/// variants already give every other caller.
pub async fn prune_server_save_records(
    client: &RommClient,
    rom_id: &str,
    emulator_name: &str,
    keep: u32,
) -> (usize, Vec<String>) {
    let keep = keep.max(1) as usize;

    let payload = match client.saves_for_rom(rom_id).await {
        Ok(payload) => payload,
        Err(err) => return (0, vec![err.to_string()]),
    };
    let records = server_records_from_payload(&payload);

    let emulator_key = emulator_name.trim().to_lowercase();
    let mut matching: Vec<Value> = records
        .into_iter()
        .filter(|item| {
            emulator_key.is_empty()
                || item
                    .get("emulator")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_lowercase() == emulator_key)
                    .unwrap_or(false)
        })
        .collect();

    matching.sort_by(|a, b| {
        let a_key = (record_timestamp(a), id_rank(a));
        let b_key = (record_timestamp(b), id_rank(b));
        b_key
            .0
            .partial_cmp(&a_key.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b_key.1.cmp(&a_key.1))
    });

    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
    for item in matching {
        let key = slot_dedupe_key(&item);
        if !groups.contains_key(&key) {
            group_order.push(key.clone());
        }
        groups.entry(key).or_default().push(item);
    }

    let mut stale: Vec<Value> = Vec::new();
    for key in group_order {
        if let Some(group) = groups.remove(&key) {
            if group.len() > keep {
                stale.extend(group.into_iter().skip(keep));
            }
        }
    }

    let mut deleted_count = 0usize;
    let mut failed_ids: Vec<String> = Vec::new();
    for record in stale {
        let raw_id = record
            .get("id")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new()));
        let save_id = stringify_id(&raw_id).trim().to_string();
        if save_id.is_empty() {
            continue;
        }
        let Ok(numeric_id) = save_id.parse::<i64>() else {
            failed_ids.push(save_id);
            continue;
        };
        match client.delete_save(numeric_id).await {
            Ok(status) if (200..300).contains(&status) || status == 404 || status == 410 => {
                deleted_count += 1;
            }
            _ => failed_ids.push(save_id),
        }
    }

    (deleted_count, failed_ids)
}
