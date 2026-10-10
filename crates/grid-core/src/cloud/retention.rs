//! Client-side retention pruning of server save records.
//!
//! Saves: ported from `_prune_server_save_records`
//! (`grid_launcher/ui/mixins/cloud_mixin.py:1676-1765`).
//!
//! Q9 owner rule for saves (one owner per group, never both): RomM's
//! `autocleanup` groups saves by rom + slot, ignores the emulator, and never
//! cleans a null-slot save. So a record with a non-blank `slot` belongs to
//! the server (GRID sends `autocleanup` on every slotted upload, see
//! `romm::cloud::save_upload_query`), and [`saves_to_prune`] never selects
//! one. A null-slot record belongs to the client prune here. The two never
//! touch the same group, so they can never prune it to different counts.
//!
//! There is no state retention: RomM replaces a state uploaded under the
//! same file name and emulator in place, and GRID uploads each state under
//! its own file name, so each state slot file has one cloud record.

use std::collections::HashMap;

use serde_json::Value;

use crate::romm::RommClient;

use super::restore::{
    id_rank, record_timestamp, server_records_from_payload, slot_dedupe_key, stringify_id,
};

/// `true` when the record has a non-blank `slot` string.
fn has_slot(record: &Value) -> bool {
    record
        .get("slot")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
}

/// Splits `sorted` (newest first) into groups by `key` in first-seen
/// order, and returns every record after the first `keep` of each group.
fn stale_after_keep(sorted: Vec<Value>, keep: usize, key: impl Fn(&Value) -> String) -> Vec<Value> {
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
    for item in sorted {
        let k = key(&item);
        if !groups.contains_key(&k) {
            group_order.push(k.clone());
        }
        groups.entry(k).or_default().push(item);
    }
    let mut stale = Vec::new();
    for k in group_order {
        if let Some(group) = groups.remove(&k) {
            stale.extend(group.into_iter().skip(keep));
        }
    }
    stale
}

/// The save records the client prune deletes (`cloud_mixin.py:1677-1723`),
/// pure:
///
/// 1. `keep == 0` is unlimited: nothing.
/// 2. Records with a non-blank `slot` are skipped: the server owns them.
/// 3. Records whose `emulator` matches `emulator_name` case-insensitively;
///    a blank `emulator_name` passes every record (:1682-1690). Unlike
///    [`super::restore::latest_server_record`], no match prunes nothing.
/// 4. Sorted by `(timestamp, numeric id)` descending (:1701).
/// 5. Grouped by [`slot_dedupe_key`] (here: the file stem, as the slot is
///    blank); everything after the first `keep` of a group is stale.
pub fn saves_to_prune(records: &[Value], emulator_name: &str, keep: u32) -> Vec<Value> {
    if keep == 0 {
        return Vec::new();
    }
    let emulator_key = emulator_name.trim().to_lowercase();
    let mut matching: Vec<Value> = records
        .iter()
        .filter(|item| !has_slot(item))
        .filter(|item| {
            emulator_key.is_empty()
                || item
                    .get("emulator")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_lowercase() == emulator_key)
                    .unwrap_or(false)
        })
        .cloned()
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

    stale_after_keep(matching, keep as usize, slot_dedupe_key)
}

/// `_prune_server_save_records(rom_id, emulator_name, keep_latest)`
/// (`cloud_mixin.py:1676-1765`): `keep == 0` returns at once without a
/// request. Otherwise it lists `GET /api/saves?rom_id=` (parsed through
/// [`server_records_from_payload`], which drops blank ids), selects
/// [`saves_to_prune`], and deletes each stale record with one request
/// ([`RommClient::delete_save_record`]: 404/410 count as deleted). A
/// non-integer id is recorded as failed WITHOUT a request (:1737-1741); any
/// other failure records the id and the loop continues (:1759-1765).
///
/// Returns `(deleted_count, failed_ids)`. A failure to list the records
/// returns `(0, vec![err.to_string()])`. `RommError`'s `Display` never
/// embeds the request, its URL, or its headers, so this text carries no
/// secret.
pub async fn prune_server_save_records(
    client: &RommClient,
    rom_id: &str,
    emulator_name: &str,
    keep: u32,
) -> (usize, Vec<String>) {
    if keep == 0 {
        return (0, Vec::new());
    }
    let payload = match client.saves_for_rom(rom_id).await {
        Ok(payload) => payload,
        Err(err) => return (0, vec![err.to_string()]),
    };
    let records = server_records_from_payload(&payload);

    let mut deleted_count = 0usize;
    let mut failed_ids: Vec<String> = Vec::new();
    for record in saves_to_prune(&records, emulator_name, keep) {
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
        match client.delete_save_record(numeric_id).await {
            Ok(()) => deleted_count += 1,
            Err(_) => failed_ids.push(save_id),
        }
    }

    (deleted_count, failed_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ids(records: &[Value]) -> Vec<i64> {
        let mut out: Vec<i64> = records.iter().map(id_rank).collect();
        out.sort_unstable();
        out
    }

    // --- saves_to_prune ---------------------------------------------------

    #[test]
    fn saves_limit_zero_prunes_nothing() {
        let records = vec![
            json!({"id": 1, "emulator": "Snes9x", "file_name": "a.srm", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 2, "emulator": "Snes9x", "file_name": "a.srm", "updated_at": "2026-01-02T00:00:00Z"}),
        ];
        assert!(saves_to_prune(&records, "Snes9x", 0).is_empty());
    }

    #[test]
    fn saves_with_a_slot_belong_to_the_server_and_are_never_pruned_here() {
        let records = vec![
            json!({"id": 1, "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 2, "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-01-02T00:00:00Z"}),
            json!({"id": 3, "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-01-03T00:00:00Z"}),
            json!({"id": 4, "emulator": "xemu", "slot": "shared-media", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 5, "emulator": "xemu", "slot": "shared-media", "updated_at": "2026-01-02T00:00:00Z"}),
        ];
        assert!(saves_to_prune(&records, "Redream", 1).is_empty());
        assert!(saves_to_prune(&records, "xemu", 1).is_empty());
        assert!(saves_to_prune(&records, "", 1).is_empty());
    }

    #[test]
    fn saves_without_a_slot_keep_n_per_file_stem() {
        let records = vec![
            json!({"id": 1, "emulator": "Snes9x", "slot": null, "file_name": "a.srm", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 2, "emulator": "Snes9x", "slot": null, "file_name": "a.srm", "updated_at": "2026-01-02T00:00:00Z"}),
            json!({"id": 3, "emulator": "Snes9x", "slot": "", "file_name": "a.srm", "updated_at": "2026-01-03T00:00:00Z"}),
            json!({"id": 4, "emulator": "Snes9x", "file_name": "b.srm", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 5, "emulator": "Snes9x", "file_name": "b.srm", "updated_at": "2026-01-02T00:00:00Z"}),
            // A slotted record in the same rom stays out of every group.
            json!({"id": 6, "emulator": "Snes9x", "slot": "auto", "file_name": "a.srm", "updated_at": "2025-01-01T00:00:00Z"}),
        ];
        assert_eq!(ids(&saves_to_prune(&records, "Snes9x", 1)), vec![1, 2, 4]);
        assert_eq!(ids(&saves_to_prune(&records, "Snes9x", 2)), vec![1]);
    }

    #[test]
    fn saves_equal_timestamps_keep_the_higher_id() {
        let records = vec![
            json!({"id": 7, "emulator": "Snes9x", "file_name": "a.srm", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 9, "emulator": "Snes9x", "file_name": "a.srm", "updated_at": "2026-01-01T00:00:00Z"}),
            json!({"id": 8, "emulator": "Snes9x", "file_name": "a.srm", "updated_at": "2026-01-01T00:00:00Z"}),
        ];
        assert_eq!(ids(&saves_to_prune(&records, "Snes9x", 1)), vec![7, 8]);
    }
}
