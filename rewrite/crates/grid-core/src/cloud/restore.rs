//! Cloud restore: server-record parsing/selection and payload placement.
//!
//! Ported from `grid_launcher/library/cloud_restore.py` (the whole file):
//! `save_record_timestamp` (:14), `relative_timestamp_text` (:30),
//! `sort_server_records_by_recency` (:61), `server_records_from_payload`
//! (:79), `latest_server_record` (:96), `latest_server_records_by_slot`
//! (:115), `preferred_restore_target_path` (:150),
//! `restore_single_save_payload` (:186), `restore_single_state_payload`
//! (:219). See `docs/porting/06-cloud-saves.md`, "Restore — saves" /
//! "Restore — states", for the narrative version of the target ladder and
//! placement rules.
//!
//! **Signature shape differs from Python in two ways, both pinned by the
//! task brief:**
//! - `sort_server_records_by_recency`, `latest_server_record`, and
//!   `latest_server_records_by_slot` take no `timestamp_fn` callable —
//!   Python's tests use that parameter only to force tie-break scenarios
//!   with fabricated timestamps; here [`record_timestamp`] is always the
//!   timestamp source, and this module's own tests reach the same
//!   tie-break scenarios using real `updated_at` values instead.
//! - `restore_single_save_payload` / `restore_single_state_payload` take
//!   an already-resolved `target: &Path` instead of Python's
//!   `(directories, record, candidate_paths, fallback_name)`. Target
//!   selection is [`preferred_restore_target_path`]'s job; these two
//!   functions are placement-only, matching the "records, selection,
//!   placement" split this module is named for. The caller (a future
//!   ops-layer task) is expected to call `preferred_restore_target_path`
//!   first and only invoke placement when it returns `Some`.
//!
//! State-image-name filtering (excluding screenshot assets from a state
//! listing, `cloud_mixin.py:1653`) is explicitly OUT of scope for this
//! module — it belongs to the ops layer, not record selection.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};
use serde_json::Value;

use super::archive::{extract_payload_zip, payload_is_zip};
use super::IgnoreSets;

// ---------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------

/// Parses `text` (already `Z`-rewritten by the caller) the way Python's
/// `datetime.fromisoformat(text).timestamp()` does for the ISO-8601 shapes
/// RomM's `updated_at`/`created_at` fields actually use: an offset-aware
/// `YYYY-MM-DDTHH:MM:SS[.ffffff]<offset>` form, and a naive
/// `YYYY-MM-DDTHH:MM:SS[.ffffff]` / `YYYY-MM-DD` form with no offset.
///
/// **Naive vs aware, read carefully:** Python's `datetime.timestamp()` on
/// an AWARE datetime converts directly using its own offset — timezone-
/// independent. On a NAIVE datetime (no offset present in the text — the
/// case Python takes when `Z` was absent to begin with, since the `Z`
/// rewrite is what manufactures an offset), `.timestamp()` assumes the
/// naive value is already expressed in the *platform's local timezone*
/// and converts from there. This function reproduces exactly that split:
/// aware text is parsed and converted with its own offset;
/// offset-less text is parsed as a [`chrono::NaiveDateTime`] and
/// interpreted via [`chrono::Local`], not UTC. Getting this wrong (e.g.
/// always assuming UTC) would silently skew every naive timestamp by the
/// host's UTC offset.
///
/// This is a documented, deliberately-scoped subset of what Python 3.11+'s
/// `fromisoformat` actually accepts (which is close to the full ISO-8601:2004
/// grammar — arbitrary single-character date/time separators, basic
/// `YYYYMMDD` dates, `±HH`/`±HHMM` offsets, week dates, etc.). Real RomM
/// timestamps only ever use the `T`-separated extended form covered here;
/// anything outside that returns `None`, matching Python's fall-through to
/// the next field / `0.0` on a `ValueError`.
fn parse_iso_like_python(text: &str) -> Option<f64> {
    // Offset-aware forms: RFC3339 covers "...SS[.ffffff]+HH:MM" (and "Z",
    // though the caller has already rewritten that to "+00:00" before this
    // function ever sees it).
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(dt.timestamp() as f64 + f64::from(dt.timestamp_subsec_nanos()) / 1e9);
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f%:z", "%Y-%m-%dT%H:%M:%S%:z"] {
        if let Ok(dt) = chrono::DateTime::parse_from_str(text, fmt) {
            return Some(dt.timestamp() as f64 + f64::from(dt.timestamp_subsec_nanos()) / 1e9);
        }
    }

    // Naive forms: interpret in the platform's LOCAL timezone (see the
    // doc comment above), not UTC.
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(text, fmt) {
            if let Some(local_dt) = local_from_naive(ndt) {
                return Some(
                    local_dt.timestamp() as f64
                        + f64::from(local_dt.timestamp_subsec_nanos()) / 1e9,
                );
            }
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        if let Some(ndt) = date.and_hms_opt(0, 0, 0) {
            if let Some(local_dt) = local_from_naive(ndt) {
                return Some(local_dt.timestamp() as f64);
            }
        }
    }
    None
}

/// Interprets a naive datetime as platform-local wall-clock time, the same
/// way Python's `datetime.timestamp()` treats a tzinfo-less `datetime`.
/// A DST-fold ambiguity (`LocalResult::Ambiguous`) or a DST-gap
/// impossibility (`LocalResult::None`) has no single correct answer in
/// either language; this picks the earliest candidate
/// (`.earliest()`) as a deterministic tie-break rather than failing the
/// whole parse — Python's own naive `.timestamp()` similarly always
/// produces *some* answer for these cases via the platform C library,
/// never raising.
fn local_from_naive(ndt: NaiveDateTime) -> Option<chrono::DateTime<Local>> {
    Local.from_local_datetime(&ndt).earliest()
}

/// `save_record_timestamp` (`cloud_restore.py:13-27`): `updated_at`
/// preferred, `created_at` fallback. Only string values are considered;
/// blank (after `.strip()`) is skipped. The `Z`-suffix rewrite
/// (`text[:-1] + "+00:00"`) happens here, per field, before parsing — a
/// literal, case-sensitive `Z` at the end only (not `z`, matching Python's
/// `str.endswith("Z")`). Returns `0.0` when neither field parses, matching
/// Python's fall-through return.
pub fn record_timestamp(record: &Value) -> f64 {
    for key in ["updated_at", "created_at"] {
        let Some(raw) = record.get(key).and_then(Value::as_str) else {
            continue;
        };
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let rewritten = match text.strip_suffix('Z') {
            Some(stripped) => format!("{stripped}+00:00"),
            None => text.to_string(),
        };
        if let Some(ts) = parse_iso_like_python(&rewritten) {
            return ts;
        }
    }
    0.0
}

/// `relative_timestamp_text` (`cloud_restore.py:30-58`). `timestamp == 0.0`
/// (falsy in Python) renders `"Unknown"`.
///
/// **Ports the BUGGY bucket table verbatim — do not fix.** The `ranges`
/// tuple is checked largest-threshold-first: `(86_400, 3_600, "hour")`
/// comes before `(3_600, 60, "minute")`. Since the first entry's guard
/// (`elapsed_seconds < 86_400`) already covers every value the second
/// entry's guard (`elapsed_seconds < 3_600`) would ever see, the minutes
/// bucket is dead code — ANY elapsed time from 90 seconds up to a day
/// renders as `"N hours ago"` (N computed via `elapsed / 3600`, floored to
/// at least 1). Concretely: 120 seconds elapsed renders `"1 hour ago"`,
/// not `"2 minutes ago"`. See `docs/porting/06-cloud-saves.md`, "Manual
/// actions" for context on why this ships as-is.
pub fn relative_timestamp_text(timestamp: f64, now: f64) -> String {
    if timestamp == 0.0 {
        return "Unknown".to_string();
    }

    let diff = now - timestamp;
    // Python: `max(0, int(current_time - timestamp))` — int() truncates
    // toward zero; the max(0, ...) clip handles a future timestamp.
    let elapsed_seconds: i64 = if diff <= 0.0 { 0 } else { diff.trunc() as i64 };

    if elapsed_seconds < 30 {
        return "just now".to_string();
    }
    if elapsed_seconds < 90 {
        return "1 minute ago".to_string();
    }

    const RANGES: [(i64, i64, &str); 2] = [(86_400, 3_600, "hour"), (3_600, 60, "minute")];
    for (threshold, unit_seconds, label) in RANGES {
        if elapsed_seconds < threshold {
            let value = (elapsed_seconds / unit_seconds).max(1);
            let suffix = if value == 1 { "" } else { "s" };
            return format!("{value} {label}{suffix} ago");
        }
    }

    let days = elapsed_seconds / 86_400;
    if days < 7 {
        let suffix = if days == 1 { "" } else { "s" };
        return format!("{days} day{suffix} ago");
    }
    let weeks = (days / 7).max(1);
    let suffix = if weeks == 1 { "" } else { "s" };
    format!("{weeks} week{suffix} ago")
}

// ---------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------

/// Stringifies a JSON `id` value the way Python's `str(record.get("id",
/// ""))` does for the value shapes actually seen (string or integer id):
/// a JSON string passes through unchanged; a JSON number renders via its
/// own `Display` (matches `str(int)` for the integer ids RomM actually
/// sends). `null`/bool/array/object ids are not expected in practice;
/// they're given a best-effort rendering rather than a panic.
fn stringify_id(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Null => "None".to_string(),
        other => other.to_string(),
    }
}

/// `server_records_from_payload` (`cloud_restore.py:79-93`), fixed to
/// `id_key = "id"` (the only value any call site uses). Rejects a
/// non-array payload (`[]`); keeps only object items; drops any item
/// whose stringified, trimmed `id` is blank; de-duplicates on that string
/// id, first occurrence wins.
pub fn server_records_from_payload(payload: &Value) -> Vec<Value> {
    let Some(items) = payload.as_array() else {
        return Vec::new();
    };

    let mut seen_ids = std::collections::HashSet::new();
    let mut records = Vec::new();
    for item in items {
        if !item.is_object() {
            continue;
        }
        let raw_id = item
            .get("id")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new()));
        let record_id = stringify_id(&raw_id).trim().to_string();
        if record_id.is_empty() || seen_ids.contains(&record_id) {
            continue;
        }
        seen_ids.insert(record_id);
        records.push(item.clone());
    }
    records
}

/// `_id_rank` (`cloud_restore.py:65-70`): `int(record.get("id", 0))`,
/// `0` on any conversion failure (missing key, non-numeric string, wrong
/// type). Distinct from [`stringify_id`] above — this reads the RAW `id`
/// value again (not the deduped string form), matching Python's own
/// re-read inside the sort key.
fn id_rank(record: &Value) -> i64 {
    match record.get("id") {
        None => 0,
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i
            } else if let Some(u) = n.as_u64() {
                i64::try_from(u).unwrap_or(i64::MAX)
            } else if let Some(f) = n.as_f64() {
                f.trunc() as i64
            } else {
                0
            }
        }
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn record_key(record: &Value) -> (f64, i64) {
    (record_timestamp(record), id_rank(record))
}

/// `sort_server_records_by_recency` (`cloud_restore.py:61-76`), fixed to
/// always use [`record_timestamp`] as the timestamp source (see module
/// doc comment). Sorts by `(timestamp, numeric id)` descending, in place.
/// Non-object entries are dropped first, matching Python's
/// `isinstance(item, dict)` filter ahead of the sort.
///
/// Uses [`slice::sort_by`], which is a STABLE sort: for a comparator that
/// reports two elements equal, their original relative order is kept.
/// That matches Python's `sorted(..., reverse=True)`, which is also
/// stable — CPython implements `reverse=True` as a stable ascending sort
/// with the final result reversed as a whole, so elements with EQUAL keys
/// keep their original relative order (they are not individually
/// reversed against each other).
pub fn sort_server_records_by_recency(records: &mut Vec<Value>) {
    records.retain(Value::is_object);
    records.sort_by(|a, b| {
        let (a_ts, a_id) = record_key(a);
        let (b_ts, b_id) = record_key(b);
        b_ts.partial_cmp(&a_ts)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b_id.cmp(&a_id))
    });
}

fn emulator_key_matches(record: &Value, emulator_key: &str) -> bool {
    record
        .get("emulator")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase() == emulator_key)
        .unwrap_or(false)
}

/// The element of `items` with the greatest [`record_key`], first
/// occurrence winning on a tie — the same result `sort_server_records_by_
/// recency(...)[0]` would give without needing an owned, sorted copy (see
/// that function's doc comment on stability: a tie's winner is whichever
/// tied element appeared earliest in `items`, since `Iterator::max_by`'s
/// "last element wins" tie-break is the OPPOSITE of what a stable
/// descending sort produces, hence the manual strict-`>` fold instead of
/// `max_by`/`max_by_key`).
fn pick_first_max<'a, I: Iterator<Item = &'a Value>>(items: I) -> Option<&'a Value> {
    let mut best: Option<(&Value, (f64, i64))> = None;
    for item in items {
        let key = record_key(item);
        let take = match &best {
            None => true,
            Some((_, best_key)) => {
                key.0 > best_key.0 || (key.0 == best_key.0 && key.1 > best_key.1)
            }
        };
        if take {
            best = Some((item, key));
        }
    }
    best.map(|(item, _)| item)
}

/// `latest_server_record` (`cloud_restore.py:96-112`). Filters to records
/// whose `emulator` field matches `emulator_name` (trim + casefold-via-
/// `to_lowercase`, this rewrite's established casefold stand-in — see
/// `cloud/state.rs`'s `rom_id_key` doc comment). **Falls back to ALL
/// records when the emulator filter matches nothing** — this is
/// deliberate (not a bug): it's what lets a save uploaded from a
/// differently-named emulator still restore. `None` only when `records`
/// is empty.
pub fn latest_server_record<'a>(records: &'a [Value], emulator_name: &str) -> Option<&'a Value> {
    if records.is_empty() {
        return None;
    }
    let emulator_key = emulator_name.trim().to_lowercase();
    let filtered: Vec<&Value> = records
        .iter()
        .filter(|r| emulator_key_matches(r, &emulator_key))
        .collect();
    if !filtered.is_empty() {
        pick_first_max(filtered.into_iter())
    } else {
        pick_first_max(records.iter())
    }
}

/// The per-slot dedupe key `latest_server_records_by_slot` uses
/// (`cloud_restore.py:132-145`): the record's `slot` field, trimmed +
/// casefolded; else, when that's blank, the file-stem of `file_name`
/// (only when `file_name` is a non-blank string), trimmed + casefolded;
/// else the literal `"__default__"`.
fn slot_dedupe_key(record: &Value) -> String {
    let slot_key = record
        .get("slot")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    let slot_key = slot_key.or_else(|| {
        record
            .get("file_name")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| Path::new(s).file_stem())
            .and_then(|stem| stem.to_str())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
    });

    slot_key.unwrap_or_else(|| "__default__".to_string())
}

/// `latest_server_records_by_slot` (`cloud_restore.py:115-147`). Same
/// emulator-filter-with-fallback as [`latest_server_record`], then sorted
/// by recency and walked in that order keeping the first (i.e. newest)
/// record seen per [`slot_dedupe_key`].
pub fn latest_server_records_by_slot(records: &[Value], emulator_name: &str) -> Vec<Value> {
    if records.is_empty() {
        return Vec::new();
    }
    let emulator_key = emulator_name.trim().to_lowercase();
    let filtered: Vec<Value> = records
        .iter()
        .filter(|r| emulator_key_matches(r, &emulator_key))
        .cloned()
        .collect();
    let mut selection = if !filtered.is_empty() {
        filtered
    } else {
        records.to_vec()
    };
    sort_server_records_by_recency(&mut selection);

    let mut latest = Vec::new();
    let mut seen_slots = std::collections::HashSet::new();
    for item in selection {
        let slot_key = slot_dedupe_key(&item);
        if seen_slots.contains(&slot_key) {
            continue;
        }
        seen_slots.insert(slot_key);
        latest.push(item);
    }
    latest
}

// ---------------------------------------------------------------------
// Target selection
// ---------------------------------------------------------------------

/// The final path component of `raw`, trimmed — mirrors Python's
/// `Path(raw).name.strip()`, applied here to both the record filename and
/// the fallback filename. `""` when `raw` has no final component (e.g.
/// itself blank).
fn normalized_file_name(raw: &str) -> String {
    Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// `preferred_restore_target_path` (`cloud_restore.py:150-177`), the
/// 7-step target ladder from doc 06 "Restore — saves":
///
/// 1. No `directories` at all → `None`.
/// 2. For the record filename, then the fallback filename: the first
///    `candidates` entry whose own filename matches case-insensitively —
///    this is what keeps a restored save in the nested folder it already
///    lives in.
/// 3. Record filename present and `candidates` non-empty →
///    `candidates[0].parent() / record_filename`.
/// 4. `candidates` non-empty → `candidates[0]` (overwrite it).
/// 5. Record filename present → `directories[0] / record_filename`.
/// 6. Fallback filename present → `directories[0] / fallback_filename`.
/// 7. Else `None`.
///
/// Note the Rust parameter order (`record_file_name, fallback_name,
/// candidates, directories`) is NOT the same as Python's
/// `(directories, record_file_name, candidate_paths, fallback_name)` —
/// pinned by the task brief's signature list.
pub fn preferred_restore_target_path(
    record_file_name: &str,
    fallback_name: &str,
    candidates: &[PathBuf],
    directories: &[PathBuf],
) -> Option<PathBuf> {
    if directories.is_empty() {
        return None;
    }

    let normalized_record_name = normalized_file_name(record_file_name);
    let normalized_fallback_name = normalized_file_name(fallback_name);

    for preferred_name in [&normalized_record_name, &normalized_fallback_name] {
        if preferred_name.is_empty() {
            continue;
        }
        let preferred_lower = preferred_name.to_lowercase();
        for candidate in candidates {
            let candidate_name = candidate
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if candidate_name == preferred_lower {
                return Some(candidate.clone());
            }
        }
    }

    if !normalized_record_name.is_empty() && !candidates.is_empty() {
        let parent = candidates[0]
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        return Some(parent.join(&normalized_record_name));
    }
    if !candidates.is_empty() {
        return Some(candidates[0].clone());
    }
    if !normalized_record_name.is_empty() {
        return Some(directories[0].join(&normalized_record_name));
    }
    if !normalized_fallback_name.is_empty() {
        return Some(directories[0].join(&normalized_fallback_name));
    }
    None
}

// ---------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------

/// `target`'s parent directory, defaulting to `"."` (matching Python's
/// `Path("bare-name").parent == Path(".")`, which `mkdir(exist_ok=True)`
/// happily no-ops on) when `target` has no parent component at all — an
/// edge case real callers won't hit (targets always come from
/// [`preferred_restore_target_path`], which always joins onto a real
/// directory), kept only for parity with Python's own behavior on a bare
/// relative filename.
fn parent_or_dot(target: &Path) -> PathBuf {
    match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// `restore_single_save_payload` (`cloud_restore.py:186-216`), placement
/// only — target selection already happened (see module doc comment).
/// Empty `payload` → `Ok(None)`. Otherwise `target`'s parent is created;
/// a zip-sniffing payload ([`payload_is_zip`]) is extracted into that
/// PARENT directory via [`extract_payload_zip`], returning the parent on
/// success (`Some`) or `None` when zero members were extracted; a
/// non-zip payload overwrites `target` unconditionally.
pub fn restore_single_save_payload(
    payload: &[u8],
    target: &Path,
    ignore: &IgnoreSets,
) -> Result<Option<PathBuf>, String> {
    if payload.is_empty() {
        return Ok(None);
    }

    let parent = parent_or_dot(target);
    fs::create_dir_all(&parent).map_err(|e| format!("failed to prepare destination: {e}"))?;

    if payload_is_zip(payload) {
        let extracted = extract_payload_zip(payload, &parent, ignore)?;
        return Ok(if extracted > 0 { Some(parent) } else { None });
    }

    fs::write(target, payload).map_err(|e| format!("failed to write save file: {e}"))?;
    Ok(Some(target.to_path_buf()))
}

/// `restore_single_state_payload` (`cloud_restore.py:219-253`), placement
/// only. Identical to [`restore_single_save_payload`] except that after a
/// **non-zip** write, an optional screenshot sidecar is written to
/// `"<target><extension>"` (string concatenation, not a path join —
/// matches Python's `Path(str(target_path) + screenshot_extension)`).
/// `screenshot` is `Some((bytes, extension))`; a zip payload never gets a
/// sidecar, and empty screenshot bytes are treated as "no screenshot"
/// (mirrors Python's `if screenshot_bytes:` truthiness check on the
/// bytes). The `.png` default extension named in doc 06 is a caller-side
/// convention (Python default-argument value) — this function always
/// takes the extension explicitly since Rust has no default parameters.
pub fn restore_single_state_payload(
    payload: &[u8],
    target: &Path,
    screenshot: Option<(&[u8], &str)>,
    ignore: &IgnoreSets,
) -> Result<Option<PathBuf>, String> {
    if payload.is_empty() {
        return Ok(None);
    }

    let parent = parent_or_dot(target);
    fs::create_dir_all(&parent).map_err(|e| format!("failed to prepare destination: {e}"))?;

    if payload_is_zip(payload) {
        let extracted = extract_payload_zip(payload, &parent, ignore)?;
        return Ok(if extracted > 0 { Some(parent) } else { None });
    }

    fs::write(target, payload).map_err(|e| format!("failed to write state file: {e}"))?;

    if let Some((bytes, extension)) = screenshot {
        if !bytes.is_empty() {
            let sidecar = PathBuf::from(format!("{}{}", target.display(), extension));
            fs::write(&sidecar, bytes).map_err(|e| format!("failed to write screenshot: {e}"))?;
        }
    }

    Ok(Some(target.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ignore_none() -> IgnoreSets {
        IgnoreSets::default()
    }

    // --- record_timestamp -------------------------------------------

    #[test]
    fn record_timestamp_prefers_updated_at_over_created_at() {
        let record = json!({
            "updated_at": "2026-04-08T10:00:00Z",
            "created_at": "2020-01-01T00:00:00Z",
        });
        let updated = record_timestamp(&json!({"updated_at": "2026-04-08T10:00:00Z"}));
        let both = record_timestamp(&record);
        assert_eq!(updated, both);
    }

    #[test]
    fn record_timestamp_falls_back_to_created_at() {
        let record = json!({"created_at": "2026-04-08T10:00:00Z"});
        assert!(record_timestamp(&record) > 0.0);
    }

    #[test]
    fn record_timestamp_zero_when_missing_or_blank() {
        assert_eq!(record_timestamp(&json!({})), 0.0);
        assert_eq!(record_timestamp(&json!({"updated_at": "  "})), 0.0);
        assert_eq!(record_timestamp(&json!({"updated_at": "not-a-date"})), 0.0);
    }

    /// Self-review: a NAIVE timestamp (no `Z`, no offset) must be
    /// interpreted as platform-LOCAL time, exactly like Python's
    /// `datetime.fromisoformat(text).timestamp()` on a tzinfo-less
    /// `datetime` — never assumed to already be UTC.
    #[test]
    fn record_timestamp_naive_text_uses_local_timezone_not_utc() {
        let record = json!({"updated_at": "2026-04-08T10:00:00"});
        let naive = chrono::NaiveDate::from_ymd_opt(2026, 4, 8)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let expected_local = Local.from_local_datetime(&naive).earliest().unwrap();
        let expected = expected_local.timestamp() as f64;

        assert_eq!(record_timestamp(&record), expected);
    }

    // --- relative_timestamp_text (test_cloud_restore.py:21) ----------

    #[test]
    fn relative_timestamp_text_uses_human_readable_ranges() {
        assert_eq!(relative_timestamp_text(0.0, 1_000.0), "Unknown");
        assert_eq!(relative_timestamp_text(995.0, 1_000.0), "just now");
        assert_eq!(relative_timestamp_text(940.0, 1_000.0), "1 minute ago");
        assert_eq!(
            relative_timestamp_text(1_000.0 - (3.0 * 3_600.0), 1_000.0),
            "3 hours ago"
        );
        assert_eq!(
            relative_timestamp_text(1_000.0 - (2.0 * 86_400.0), 1_000.0),
            "2 days ago"
        );
    }

    #[test]
    fn relative_timestamp_text_120_seconds_renders_1_hour_ago_bug() {
        // The pinned QUIRK: the minutes bucket is unreachable above 90s.
        assert_eq!(
            relative_timestamp_text(1_000.0 - 120.0, 1_000.0),
            "1 hour ago"
        );
    }

    #[test]
    fn relative_timestamp_text_weeks() {
        assert_eq!(
            relative_timestamp_text(1_000.0 - (10.0 * 86_400.0), 1_000.0),
            "1 week ago"
        );
        assert_eq!(
            relative_timestamp_text(1_000.0 - (20.0 * 86_400.0), 1_000.0),
            "2 weeks ago"
        );
    }

    // --- sort_server_records_by_recency (test_cloud_restore.py:28) ---

    #[test]
    fn sort_server_records_by_recency_prefers_newest_timestamp_then_id() {
        let mut records = vec![
            json!({"id": "4", "updated_at": "2026-04-08T10:00:00Z"}),
            json!({"id": "9", "updated_at": "2026-04-08T10:00:00Z"}),
            json!({"id": "2", "updated_at": "2026-04-07T09:00:00Z"}),
        ];

        sort_server_records_by_recency(&mut records);

        let ids: Vec<&str> = records.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["9", "4", "2"]);
    }

    // --- latest_server_records_by_slot (test_cloud_restore.py:39) ----

    #[test]
    fn latest_server_records_by_slot_keeps_newest_entry_per_slot() {
        let records = vec![
            json!({"id": "1", "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-04-08T09:00:00Z"}),
            json!({"id": "2", "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-04-08T10:00:00Z"}),
            json!({"id": "3", "emulator": "Redream", "slot": "vmu1", "updated_at": "2026-04-08T08:30:00Z"}),
            json!({"id": "4", "emulator": "Other", "slot": "vmu0", "updated_at": "2026-04-08T11:00:00Z"}),
        ];

        let grouped = latest_server_records_by_slot(&records, "Redream");

        let ids: Vec<&str> = grouped.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["2", "3"]);
    }

    /// Self-review: full slot-key precedence — a record with neither a
    /// usable `slot` nor a usable `file_name` stem collapses to the
    /// literal `"__default__"` bucket, and multiple such records dedupe
    /// down to just the newest one.
    #[test]
    fn latest_server_records_by_slot_falls_back_to_default_bucket() {
        let records = vec![
            json!({"id": "1", "emulator": "Redream", "updated_at": "2026-04-08T09:00:00Z"}),
            json!({"id": "2", "emulator": "Redream", "slot": "  ", "file_name": "  ", "updated_at": "2026-04-08T10:00:00Z"}),
        ];

        let grouped = latest_server_records_by_slot(&records, "Redream");

        let ids: Vec<&str> = grouped.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["2"]);
    }

    // --- server_records_from_payload ----------------------------------

    #[test]
    fn records_from_payload_dedupes_and_drops_blank_ids() {
        let payload = json!([
            {"id": "1", "name": "first"},
            {"id": "1", "name": "duplicate-should-be-dropped"},
            {"id": "", "name": "blank-id"},
            {"name": "missing-id"},
            "not-an-object",
            {"id": 2, "name": "int-id"},
        ]);

        let records = server_records_from_payload(&payload);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["name"], "first");
        assert_eq!(records[1]["id"], 2);
    }

    #[test]
    fn records_from_payload_rejects_non_array() {
        assert!(server_records_from_payload(&json!({"id": "1"})).is_empty());
        assert!(server_records_from_payload(&json!("string")).is_empty());
        assert!(server_records_from_payload(&Value::Null).is_empty());
    }

    // --- latest_server_record ------------------------------------------

    #[test]
    fn latest_record_falls_back_to_all_when_emulator_never_matches() {
        let records = vec![
            json!({"id": "1", "emulator": "Snes9x", "updated_at": "2026-04-08T09:00:00Z"}),
            json!({"id": "2", "emulator": "Snes9x", "updated_at": "2026-04-08T10:00:00Z"}),
        ];

        // "Redream" matches nothing in this list, so ALL records are
        // considered instead — this is the contrast-retention fallback,
        // not a bug.
        let latest = latest_server_record(&records, "Redream").expect("falls back to all");
        assert_eq!(latest["id"], "2");
    }

    #[test]
    fn latest_record_none_when_records_empty() {
        assert!(latest_server_record(&[], "Redream").is_none());
    }

    // --- preferred_restore_target_path: candidate filename preferred
    // (test_cloud_restore.py:51) ----------------------------------------

    #[test]
    fn preferred_target_prefers_exact_candidate_filename() {
        let core_dir = PathBuf::from("/saves/Snes9x");
        let rtc_path = core_dir.join("Chrono Trigger.rtc");
        let srm_path = core_dir.join("Chrono Trigger.srm");

        let target = preferred_restore_target_path(
            "Chrono Trigger.srm",
            "Chrono Trigger.srm",
            &[rtc_path, srm_path.clone()],
            &[PathBuf::from("/saves")],
        );

        assert_eq!(target, Some(srm_path));
    }

    // --- preferred_restore_target_path: the remaining ladder steps -----

    #[test]
    fn target_ladder_step_1_no_directories_is_none() {
        let target = preferred_restore_target_path(
            "Chrono Trigger.srm",
            "Chrono Trigger.srm",
            &[PathBuf::from("/saves/Chrono Trigger.rtc")],
            &[],
        );
        assert_eq!(target, None);
    }

    #[test]
    fn target_ladder_step_3_record_name_with_candidates_joins_first_candidates_parent() {
        // No candidate's own filename matches the record name, so step 2
        // never fires; step 3 rejoins the record's filename onto
        // candidates[0]'s parent.
        let target = preferred_restore_target_path(
            "Chrono Trigger.srm",
            "fallback.srm",
            &[PathBuf::from("/saves/Snes9x/Other.rtc")],
            &[PathBuf::from("/saves")],
        );
        assert_eq!(
            target,
            Some(PathBuf::from("/saves/Snes9x/Chrono Trigger.srm"))
        );
    }

    #[test]
    fn target_ladder_step_4_no_record_name_overwrites_first_candidate() {
        let candidate = PathBuf::from("/saves/Snes9x/Other.rtc");
        let target = preferred_restore_target_path(
            "",
            "fallback.srm",
            std::slice::from_ref(&candidate),
            &[PathBuf::from("/saves")],
        );
        assert_eq!(target, Some(candidate));
    }

    #[test]
    fn target_ladder_step_5_record_name_no_candidates_joins_first_directory() {
        let target = preferred_restore_target_path(
            "Chrono Trigger.srm",
            "fallback.srm",
            &[],
            &[PathBuf::from("/saves")],
        );
        assert_eq!(target, Some(PathBuf::from("/saves/Chrono Trigger.srm")));
    }

    #[test]
    fn target_ladder_step_6_fallback_name_no_record_name_no_candidates() {
        let target =
            preferred_restore_target_path("", "fallback.srm", &[], &[PathBuf::from("/saves")]);
        assert_eq!(target, Some(PathBuf::from("/saves/fallback.srm")));
    }

    #[test]
    fn target_ladder_step_7_nothing_available_is_none() {
        let target = preferred_restore_target_path("", "", &[], &[PathBuf::from("/saves")]);
        assert_eq!(target, None);
    }

    // --- restore_single_save_payload (test_cloud_restore.py:51 covers
    // selection; placement itself is exercised through target directly)

    #[test]
    fn restore_single_save_payload_overwrites_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Chrono Trigger.srm");
        fs::write(&target, b"old").unwrap();

        let restored = restore_single_save_payload(b"new-bytes", &target, &ignore_none()).unwrap();

        assert_eq!(restored, Some(target.clone()));
        assert_eq!(fs::read(&target).unwrap(), b"new-bytes");
    }

    #[test]
    fn restore_single_save_payload_empty_payload_is_none() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Chrono Trigger.srm");
        let restored = restore_single_save_payload(b"", &target, &ignore_none()).unwrap();
        assert_eq!(restored, None);
        assert!(!target.exists());
    }

    // --- restore_single_state_payload: nested candidate kept
    // (test_cloud_restore.py:123) ----------------------------------------

    #[test]
    fn restore_single_state_payload_keeps_nested_candidate_folder() {
        let temp = tempfile::tempdir().unwrap();
        let core_dir = temp.path().join("Snes9x");
        fs::create_dir_all(&core_dir).unwrap();
        let existing_state = core_dir.join("Chrono Trigger.state1");
        fs::write(&existing_state, b"slot-1").unwrap();

        let target = preferred_restore_target_path(
            "Chrono Trigger.state.auto",
            "Chrono Trigger.state",
            std::slice::from_ref(&existing_state),
            &[temp.path().to_path_buf()],
        )
        .unwrap();

        let restored =
            restore_single_state_payload(b"auto-state", &target, None, &ignore_none()).unwrap();

        let expected_path = core_dir.join("Chrono Trigger.state.auto");
        assert_eq!(restored, Some(expected_path.clone()));
        assert_eq!(fs::read(&expected_path).unwrap(), b"auto-state");
    }

    // --- screenshot sidecar: written (test_cloud_restore.py:144) -------

    #[test]
    fn restore_single_state_payload_writes_screenshot_sidecar() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Chrono Trigger.state.auto");

        let restored = restore_single_state_payload(
            b"auto-state",
            &target,
            Some((b"\x89PNG\r\n\x1a\n", ".png")),
            &ignore_none(),
        )
        .unwrap();

        let sidecar = PathBuf::from(format!("{}.png", target.display()));
        assert_eq!(restored, Some(target.clone()));
        assert!(target.exists());
        assert!(sidecar.exists());
        assert_eq!(fs::read(&sidecar).unwrap(), b"\x89PNG\r\n\x1a\n");
    }

    // --- screenshot sidecar: omitted when no screenshot
    // (test_cloud_restore.py:169) -----------------------------------------

    #[test]
    fn restore_single_state_payload_omits_sidecar_when_no_screenshot() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Chrono Trigger.state.auto");

        let restored =
            restore_single_state_payload(b"auto-state", &target, None, &ignore_none()).unwrap();

        let sidecar = PathBuf::from(format!("{}.png", target.display()));
        assert_eq!(restored, Some(target.clone()));
        assert!(target.exists());
        assert!(!sidecar.exists());
    }

    // --- screenshot sidecar: custom extension
    // (test_cloud_restore.py:192) -----------------------------------------

    #[test]
    fn restore_single_state_payload_uses_custom_screenshot_extension() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Chrono Trigger.state.auto");

        let restored = restore_single_state_payload(
            b"auto-state",
            &target,
            Some((b"fake", ".jpg")),
            &ignore_none(),
        )
        .unwrap();

        let jpg_sidecar = PathBuf::from(format!("{}.jpg", target.display()));
        let png_sidecar = PathBuf::from(format!("{}.png", target.display()));
        assert_eq!(restored, Some(target));
        assert!(jpg_sidecar.exists());
        assert!(!png_sidecar.exists());
    }

    // --- screenshot: empty bytes treated as "no screenshot" -------------

    #[test]
    fn restore_single_state_payload_treats_empty_screenshot_bytes_as_absent() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Chrono Trigger.state.auto");

        restore_single_state_payload(b"auto-state", &target, Some((b"", ".png")), &ignore_none())
            .unwrap();

        let sidecar = PathBuf::from(format!("{}.png", target.display()));
        assert!(!sidecar.exists());
    }

    // --- zip payloads: no sidecar (test_cloud_restore.py:217) -----------

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::{Cursor, Write};
        use zip::write::SimpleFileOptions;
        use zip::{CompressionMethod, ZipWriter};

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for &(name, content) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn restore_single_state_payload_no_sidecar_for_zip_payload() {
        let temp = tempfile::tempdir().unwrap();
        let core_dir = temp.path().join("Snes9x");
        fs::create_dir_all(&core_dir).unwrap();
        let existing_state = core_dir.join("Chrono Trigger.state1");
        fs::write(&existing_state, b"slot-1").unwrap();

        let payload = build_zip(&[("Chrono Trigger.state2", b"new-slot" as &[u8])]);

        let target = preferred_restore_target_path(
            "Chrono Trigger.state.zip",
            "Chrono Trigger.state",
            &[existing_state],
            &[temp.path().to_path_buf()],
        )
        .unwrap();

        let restored = restore_single_state_payload(
            &payload,
            &target,
            Some((b"\x89PNG\r\n\x1a\n", ".png")),
            &ignore_none(),
        )
        .unwrap();

        let expected_extracted_path = core_dir.join("Chrono Trigger.state2");
        let sidecar = PathBuf::from(format!(
            "{}.png",
            core_dir.join("Chrono Trigger.state.zip").display()
        ));
        assert_eq!(restored, Some(core_dir));
        assert!(expected_extracted_path.exists());
        assert!(!sidecar.exists());
    }

    // --- zip payloads: unpacked into matching directory
    // (test_cloud_restore.py:244) -----------------------------------------

    #[test]
    fn restore_single_state_payload_unpacks_zip_archive_into_matching_directory() {
        let temp = tempfile::tempdir().unwrap();
        let core_dir = temp.path().join("Snes9x");
        fs::create_dir_all(&core_dir).unwrap();
        let existing_slot = core_dir.join("Chrono Trigger.state1");
        let existing_auto = core_dir.join("Chrono Trigger.state.auto");
        fs::write(&existing_slot, b"old-slot").unwrap();
        fs::write(&existing_auto, b"old-auto").unwrap();

        let payload = build_zip(&[
            ("Chrono Trigger.state1", b"new-slot" as &[u8]),
            ("Chrono Trigger.state.auto", b"new-auto"),
        ]);

        let target = preferred_restore_target_path(
            "Chrono Trigger.state.zip",
            "Chrono Trigger.state",
            &[existing_slot.clone(), existing_auto.clone()],
            &[temp.path().to_path_buf()],
        )
        .unwrap();

        let restored =
            restore_single_state_payload(&payload, &target, None, &ignore_none()).unwrap();

        assert_eq!(restored, Some(core_dir));
        assert_eq!(fs::read(&existing_slot).unwrap(), b"new-slot");
        assert_eq!(fs::read(&existing_auto).unwrap(), b"new-auto");
    }

    #[test]
    fn restore_single_save_payload_zip_extracts_into_parent_and_returns_it() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("saves");
        fs::create_dir_all(&dest).unwrap();
        let payload = build_zip(&[("a.sav", b"aaa" as &[u8]), ("b.sav", b"bbb")]);
        let target = dest.join("placeholder.sav");

        let restored = restore_single_save_payload(&payload, &target, &ignore_none()).unwrap();

        assert_eq!(restored, Some(dest.clone()));
        assert_eq!(fs::read(dest.join("a.sav")).unwrap(), b"aaa");
        assert_eq!(fs::read(dest.join("b.sav")).unwrap(), b"bbb");
    }

    #[test]
    fn restore_single_save_payload_zip_with_zero_members_returns_none() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("saves");
        fs::create_dir_all(&dest).unwrap();
        // A well-formed but empty zip: nothing to extract.
        let payload = build_zip(&[]);
        let target = dest.join("placeholder.sav");

        let restored = restore_single_save_payload(&payload, &target, &ignore_none()).unwrap();

        assert_eq!(restored, None);
    }
}
