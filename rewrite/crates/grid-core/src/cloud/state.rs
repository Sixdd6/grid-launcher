//! Cloud sync state: identity keys, the per-game sync state entry, the
//! tolerant normalization of the raw `cloud_sync_state` config table, and
//! the auto-upload plan/summary math.
//!
//! Ported from `grid_launcher/library/identity.py` and
//! `grid_launcher/library/cloud_sync.py`.

use super::{CloudGame, SaveType};
use crate::config::Config;
use std::collections::BTreeMap;

/// The composite cloud-sync-state key for a game: `"rom:<id>"` when the
/// game has a rom id, else `"name:<title>::<platform>"`, else `""` when
/// untrackable (neither a rom id nor a title/platform pair).
///
/// NOTE ON THE PYTHON ANCHOR: despite this function's name matching
/// `identity.py:4`'s `game_key`, that Python function only ever returns the
/// bare `(title, platform)` tuple — it never looks at `rom_id`. The
/// rom-preferring composite behavior this function actually implements is
/// `cloud_sync.py:66-77`'s `cloud_sync_state_key`, which composes
/// `identity.py`'s `rom_id_key` (8-12) and `game_key` (4-5). Recorded here
/// because the task brief's own citation pointed at `identity.py:4` for
/// this behavior.
pub fn game_key(game: &CloudGame) -> String {
    let rom = rom_id_key(&game.rom_id);
    if !rom.is_empty() {
        return rom;
    }
    let title = game.title.trim().to_lowercase();
    let platform = game.platform.trim().to_lowercase();
    if title.is_empty() && platform.is_empty() {
        return String::new();
    }
    format!("name:{title}::{platform}")
}

/// `identity.py:8-12`'s `rom_id_key` (trim + casefold, `""` for blank),
/// plus `cloud_sync.py:73`'s `"rom:"` prefix. Rust has no direct `casefold`
/// equivalent; per this rewrite's established convention (plan doc: "case-
/// insensitive path comparisons... compare `to_lowercase()`"), `to_lowercase()`
/// stands in for it.
pub fn rom_id_key(rom_id: &str) -> String {
    let trimmed = rom_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("rom:{}", trimmed.to_lowercase())
}

/// `identity.py:15-20`: two games match by rom id when BOTH have one,
/// otherwise by `(title, platform)` lowercased. The rom-id comparison here
/// is prefix-invariant (both sides go through the same `rom_id_key`
/// formatting), so reusing that public helper is equivalent to comparing
/// the raw trimmed+lowercased ids Python compares.
pub fn games_match_identity(a: &CloudGame, b: &CloudGame) -> bool {
    let a_rom = rom_id_key(&a.rom_id);
    let b_rom = rom_id_key(&b.rom_id);
    if !a_rom.is_empty() && !b_rom.is_empty() {
        return a_rom == b_rom;
    }
    let a_key = (
        a.title.trim().to_lowercase(),
        a.platform.trim().to_lowercase(),
    );
    let b_key = (
        b.title.trim().to_lowercase(),
        b.platform.trim().to_lowercase(),
    );
    a_key == b_key
}

/// One game's cloud sync state entry. All fields default to their zero
/// value when absent from the stored TOML — see `normalize_sync_state`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SyncStateEntry {
    #[serde(default)]
    pub last_downloaded_save_id: String,
    #[serde(default)]
    pub last_server_timestamp: f64,
    #[serde(default)]
    pub last_uploaded_local_mtime: f64,
    #[serde(default)]
    pub last_uploaded_at: String,
    #[serde(default)]
    pub last_downloaded_state_id: String,
    #[serde(default)]
    pub last_uploaded_save_mtime: f64,
    #[serde(default)]
    pub last_uploaded_state_mtime: f64,
    #[serde(default)]
    pub last_session_started_at: f64,
    #[serde(default)]
    pub last_session_ended_at: f64,
}

/// A string field is kept only when the raw TOML value is a string and its
/// trimmed form is non-blank; the trimmed form is what's stored.
fn string_field(table: &toml::value::Table, key: &str) -> Option<String> {
    let raw = table.get(key)?.as_str()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// A float field is kept when the raw TOML value is a Float (used as-is) or
/// an Integer (coerced to f64); any other type — including a String or a
/// Boolean — is dropped. (`cloud_sync.py:8-56`'s `isinstance(value, (int,
/// float))` check is, in real Python, also true for `bool` since `bool` is
/// an `int` subclass; this port does not extend the coercion to TOML
/// booleans — see the task report for why.)
fn float_field(table: &toml::value::Table, key: &str) -> Option<f64> {
    match table.get(key)? {
        toml::Value::Float(f) => Some(*f),
        toml::Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

/// `cloud_sync.py:8-56`'s tolerant normalization of the raw
/// `cloud_sync_state` config table: drop entries whose key is blank after
/// trimming or whose value isn't a table; within each entry, keep only the
/// well-typed known fields (case-preserving key, trimmed); drop an entry
/// that ends up with nothing recognized at all — but keep one where a
/// field was explicitly set to its zero value (`0.0`/`""` set is still
/// "set").
pub fn normalize_sync_state(raw: &toml::value::Table) -> BTreeMap<String, SyncStateEntry> {
    let mut normalized = BTreeMap::new();

    for (raw_key, raw_state) in raw {
        let key = raw_key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(table) = raw_state.as_table() else {
            continue;
        };

        let mut entry = SyncStateEntry::default();
        let mut any_set = false;

        if let Some(v) = string_field(table, "last_downloaded_save_id") {
            entry.last_downloaded_save_id = v;
            any_set = true;
        }
        if let Some(v) = float_field(table, "last_server_timestamp") {
            entry.last_server_timestamp = v;
            any_set = true;
        }
        if let Some(v) = float_field(table, "last_uploaded_local_mtime") {
            entry.last_uploaded_local_mtime = v;
            any_set = true;
        }
        if let Some(v) = string_field(table, "last_uploaded_at") {
            entry.last_uploaded_at = v;
            any_set = true;
        }
        if let Some(v) = string_field(table, "last_downloaded_state_id") {
            entry.last_downloaded_state_id = v;
            any_set = true;
        }
        if let Some(v) = float_field(table, "last_uploaded_save_mtime") {
            entry.last_uploaded_save_mtime = v;
            any_set = true;
        }
        if let Some(v) = float_field(table, "last_uploaded_state_mtime") {
            entry.last_uploaded_state_mtime = v;
            any_set = true;
        }
        if let Some(v) = float_field(table, "last_session_started_at") {
            entry.last_session_started_at = v;
            any_set = true;
        }
        if let Some(v) = float_field(table, "last_session_ended_at") {
            entry.last_session_ended_at = v;
            any_set = true;
        }

        if any_set {
            normalized.insert(key.to_string(), entry);
        }
    }

    normalized
}

fn table_from_normalized(map: &BTreeMap<String, SyncStateEntry>) -> toml::value::Table {
    let mut table = toml::value::Table::new();
    for (key, entry) in map {
        if let Ok(value) = toml::Value::try_from(entry) {
            table.insert(key.clone(), value);
        }
    }
    table
}

/// The normalized sync state entry for `key` (empty entry, all zero/blank,
/// when `key` is `""` or has no stored entry). `sync_entry_for` is a pure
/// read: unlike Python's `_cloud_sync_state()` getter (which writes the
/// normalized form back into `self.config` as a side effect of every
/// read), this takes `&Config` and never mutates — the normalized form is
/// only committed via `apply_sync_update`.
pub fn sync_entry_for(config: &Config, key: &str) -> SyncStateEntry {
    if key.is_empty() {
        return SyncStateEntry::default();
    }
    normalize_sync_state(&config.cloud_sync_state)
        .get(key)
        .cloned()
        .unwrap_or_default()
}

/// A shallow, `Option`-per-field patch for a `SyncStateEntry`: only `Some`
/// fields overwrite the stored value, everything else is left as-is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncStateUpdate {
    pub last_downloaded_save_id: Option<String>,
    pub last_server_timestamp: Option<f64>,
    pub last_uploaded_local_mtime: Option<f64>,
    pub last_uploaded_at: Option<String>,
    pub last_downloaded_state_id: Option<String>,
    pub last_uploaded_save_mtime: Option<f64>,
    pub last_uploaded_state_mtime: Option<f64>,
    pub last_session_started_at: Option<f64>,
    pub last_session_ended_at: Option<f64>,
}

/// Shallow-merges only the `Some` fields of `update` into the stored entry
/// for `key`, then writes the WHOLE normalized state map back into
/// `config.cloud_sync_state` (parity: Python's `_cloud_sync_state()`
/// getter normalizes-and-overwrites on every read, so any update discards
/// foreign junk on OTHER keys too — `details_view_mixin.py:361-384`). A
/// blank `key` is a no-op (parity: `cloud_sync.py:100`). An all-`None`
/// `update` is also a no-op — no map read, no write, no phantom entry
/// created for a previously-absent key (parity:
/// `update_cloud_sync_state_for_game`'s `not updates` early return,
/// `cloud_sync.py:100`; normalization keeps an entry that was explicitly
/// set to all-zero, so unconditionally inserting a default entry here
/// would persist a spurious permanent record). The caller is still
/// responsible for persisting `config` to disk (`Config::save`) — this
/// only updates the in-memory value.
pub fn apply_sync_update(config: &mut Config, key: &str, update: SyncStateUpdate) {
    if key.is_empty() || update == SyncStateUpdate::default() {
        return;
    }

    let mut map = normalize_sync_state(&config.cloud_sync_state);
    let mut entry = map.get(key).cloned().unwrap_or_default();

    if let Some(v) = update.last_downloaded_save_id {
        entry.last_downloaded_save_id = v;
    }
    if let Some(v) = update.last_server_timestamp {
        entry.last_server_timestamp = v;
    }
    if let Some(v) = update.last_uploaded_local_mtime {
        entry.last_uploaded_local_mtime = v;
    }
    if let Some(v) = update.last_uploaded_at {
        entry.last_uploaded_at = v;
    }
    if let Some(v) = update.last_downloaded_state_id {
        entry.last_downloaded_state_id = v;
    }
    if let Some(v) = update.last_uploaded_save_mtime {
        entry.last_uploaded_save_mtime = v;
    }
    if let Some(v) = update.last_uploaded_state_mtime {
        entry.last_uploaded_state_mtime = v;
    }
    if let Some(v) = update.last_session_started_at {
        entry.last_session_started_at = v;
    }
    if let Some(v) = update.last_session_ended_at {
        entry.last_session_ended_at = v;
    }

    map.insert(key.to_string(), entry);
    config.cloud_sync_state = table_from_normalized(&map);
}

/// The set of save types to auto-upload plus the local mtime planned for
/// each (`cloud_sync.py:154-184`'s `(upload_types, latest_mtimes)`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UploadPlan {
    pub types: Vec<SaveType>,
    pub latest_mtimes: BTreeMap<SaveType, f64>,
}

/// `cloud_sync.py:154-184`: plan `Save` when `save_mtime > 0.0` and
/// `save_mtime > previous + 1.0`, where `previous` is
/// `entry.last_uploaded_save_mtime`, falling back to
/// `entry.last_uploaded_local_mtime` when the former is `0.0`. Plan `State`
/// under the same rule against `entry.last_uploaded_state_mtime`, only when
/// `include_state` is true. The `+ 1.0` boundary is exclusive: a delta of
/// exactly one second is NOT planned, matching Python's `>` (not `>=`).
///
/// Python's `previous_save_mtime_raw = sync_state.get("last_uploaded_save_mtime",
/// sync_state.get("last_uploaded_local_mtime", 0))` falls back on KEY
/// ABSENCE, not on a zero VALUE — a distinction the normalized
/// `SyncStateEntry` (plain `f64`, not `Option<f64>`) cannot represent. This
/// port falls back on a zero value instead; see the task report for the
/// (very narrow) case where that differs from Python.
pub fn auto_cloud_upload_plan(
    entry: &SyncStateEntry,
    save_mtime: f64,
    state_mtime: f64,
    include_state: bool,
) -> UploadPlan {
    let mut types = Vec::new();
    let mut latest_mtimes = BTreeMap::new();

    if save_mtime > 0.0 {
        let previous = if entry.last_uploaded_save_mtime != 0.0 {
            entry.last_uploaded_save_mtime
        } else {
            entry.last_uploaded_local_mtime
        };
        if save_mtime > previous + 1.0 {
            types.push(SaveType::Save);
            latest_mtimes.insert(SaveType::Save, save_mtime);
        }
    }

    if include_state && state_mtime > 0.0 {
        let previous = entry.last_uploaded_state_mtime;
        if state_mtime > previous + 1.0 {
            types.push(SaveType::State);
            latest_mtimes.insert(SaveType::State, state_mtime);
        }
    }

    UploadPlan {
        types,
        latest_mtimes,
    }
}

/// Per-save-type upload outcome counters (`cloud_sync.py:186-241`'s
/// `per_type[save_type]`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PerTypeResult {
    pub uploaded: i64,
    pub total: i64,
    pub failed: Vec<String>,
}

/// `cloud_sync.py:186-241`: walks `Save` then `State`. A type entry with
/// `total <= 0 && uploaded <= 0 && failed.is_empty()` is skipped entirely
/// (no debug segment, no state update). When `uploaded > 0`, the planned
/// latest mtime for that type is written back — for `Save` that's BOTH
/// `last_uploaded_save_mtime` and the legacy `last_uploaded_local_mtime`;
/// for `State`, just `last_uploaded_state_mtime`. `last_uploaded_at` is set
/// once, only when at least one type uploaded and the string is non-blank
/// (trimmed). Each surviving type produces a debug segment
/// `"<type>=<uploaded>/<max(total,uploaded)> failed=<first 3 comma-joined>"`.
///
/// DEVIATION FROM PYTHON: Python's f-string embeds `failed[:3]` via its
/// Python `list` `repr` (e.g. `"failed=['a.sav', 'b.sav']"` — brackets,
/// quotes). The task brief's own pinned-rule text instead specifies
/// "`failed=<first 3 joined ','>`" — a plain comma-joined list with no
/// brackets/quotes. This port follows the brief's explicit wording (a
/// debug-only string, not asserted elsewhere byte-for-byte); see the task
/// report.
pub fn summarize_auto_cloud_upload_result(
    per_type: &BTreeMap<SaveType, PerTypeResult>,
    latest_mtimes: &BTreeMap<SaveType, f64>,
    uploaded_at: &str,
) -> (SyncStateUpdate, Vec<String>) {
    let mut update = SyncStateUpdate::default();
    let mut debug_segments = Vec::new();
    let mut any_uploaded = false;
    let empty = PerTypeResult::default();

    for save_type in [SaveType::Save, SaveType::State] {
        let result = per_type.get(&save_type).unwrap_or(&empty);

        if result.total <= 0 && result.uploaded <= 0 && result.failed.is_empty() {
            continue;
        }

        if result.uploaded > 0 {
            any_uploaded = true;
            let latest = latest_mtimes.get(&save_type).copied().unwrap_or(0.0);
            match save_type {
                SaveType::Save => {
                    update.last_uploaded_save_mtime = Some(latest);
                    update.last_uploaded_local_mtime = Some(latest);
                }
                SaveType::State => {
                    update.last_uploaded_state_mtime = Some(latest);
                }
            }
        }

        let failed_preview = result
            .failed
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        debug_segments.push(format!(
            "{}={}/{} failed={}",
            save_type.as_str(),
            result.uploaded,
            result.total.max(result.uploaded),
            failed_preview
        ));
    }

    let trimmed_uploaded_at = uploaded_at.trim();
    if any_uploaded && !trimmed_uploaded_at.is_empty() {
        update.last_uploaded_at = Some(trimmed_uploaded_at.to_string());
    }

    (update, debug_segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(title: &str, platform: &str, rom_id: &str) -> CloudGame {
        CloudGame {
            title: title.to_string(),
            platform: platform.to_string(),
            rom_id: rom_id.to_string(),
            ..Default::default()
        }
    }

    // -- game_key / rom_id_key ------------------------------------------

    #[test]
    fn game_key_prefers_rom_id_and_casefolds() {
        let g = game("Ignored Title", "Ignored Platform", " AbC-123 ");
        assert_eq!(game_key(&g), "rom:abc-123");
    }

    #[test]
    fn game_key_falls_back_to_title_platform() {
        let g = game(" Chrono Trigger ", " SNES ", "");
        assert_eq!(game_key(&g), "name:chrono trigger::snes");
    }

    #[test]
    fn game_key_is_empty_when_untrackable() {
        let g = game("", "", "");
        assert_eq!(game_key(&g), "");
    }

    #[test]
    fn rom_id_key_is_empty_for_blank() {
        assert_eq!(rom_id_key(""), "");
        assert_eq!(rom_id_key("   "), "");
    }

    // -- games_match_identity ---------------------------------------------

    #[test]
    fn games_match_identity_prefers_rom_ids() {
        let a = game("Title A", "Platform A", "RomID");
        // Different title/platform, but rom ids match (case-insensitively).
        let b = game("Title B", "Platform B", "romid");
        assert!(games_match_identity(&a, &b));

        let c = game("Title A", "Platform A", "OtherRomID");
        assert!(!games_match_identity(&a, &c));
    }

    #[test]
    fn games_match_identity_compares_title_platform_when_either_lacks_a_rom_id() {
        let a = game("Chrono Trigger", "SNES", "RomID");
        let b = game(" chrono trigger ", " snes ", "");
        assert!(games_match_identity(&a, &b));

        let c = game("Different Title", "SNES", "");
        assert!(!games_match_identity(&a, &c));
    }

    // -- normalize_sync_state ----------------------------------------------

    #[test]
    fn normalize_drops_wrong_typed_fields_and_empty_entries() {
        let toml_text = r#"
            [good]
            last_downloaded_save_id = "abc123"
            last_server_timestamp = 12.5
            last_uploaded_save_mtime = 42
            last_uploaded_at = "not-a-bool-field"
        "#;
        let mut root: toml::value::Table = toml::from_str(toml_text).unwrap();
        let mut bad_value = toml::value::Table::new();
        bad_value.insert(
            "last_downloaded_save_id".to_string(),
            toml::Value::Boolean(true),
        );
        root.insert("bad_value".to_string(), toml::Value::Table(bad_value));
        root.insert(
            "string_value".to_string(),
            toml::Value::String("junk".into()),
        );

        let mut empty_after_normalize = toml::value::Table::new();
        empty_after_normalize.insert(
            "unknown_field".to_string(),
            toml::Value::String("junk".into()),
        );
        root.insert(
            "empty_after_normalize".to_string(),
            toml::Value::Table(empty_after_normalize),
        );

        let normalized = normalize_sync_state(&root);

        let good = normalized.get("good").expect("good entry kept");
        assert_eq!(good.last_downloaded_save_id, "abc123");
        assert_eq!(good.last_server_timestamp, 12.5);
        assert_eq!(good.last_uploaded_save_mtime, 42.0); // int coerced to f64
        assert_eq!(good.last_uploaded_at, "not-a-bool-field");

        assert!(
            !normalized.contains_key("bad_value"),
            "entry whose only field was wrong-typed (bool in a string field) must be dropped entirely"
        );
        assert!(
            !normalized.contains_key("string_value"),
            "non-table value dropped"
        );
        assert!(
            !normalized.contains_key("empty_after_normalize"),
            "entry left empty is dropped"
        );
    }

    #[test]
    fn normalize_drops_bool_in_string_field_and_prunes_resulting_empty_entry() {
        let mut root = toml::value::Table::new();
        let mut bad_entry = toml::value::Table::new();
        bad_entry.insert(
            "last_downloaded_save_id".to_string(),
            toml::Value::Boolean(true),
        );
        root.insert("bad_value".to_string(), toml::Value::Table(bad_entry));

        let mut empty_entry = toml::value::Table::new();
        empty_entry.insert(
            "unknown_field".to_string(),
            toml::Value::String("junk".into()),
        );
        root.insert(
            "empty_after_normalize".to_string(),
            toml::Value::Table(empty_entry),
        );

        root.insert(
            "not_a_table".to_string(),
            toml::Value::String("nope".into()),
        );
        root.insert(String::new(), {
            let mut t = toml::value::Table::new();
            t.insert(
                "last_downloaded_save_id".to_string(),
                toml::Value::String("orphan".into()),
            );
            toml::Value::Table(t)
        });

        let normalized = normalize_sync_state(&root);
        assert!(
            !normalized.contains_key("bad_value"),
            "entry whose only field was wrong-typed must be dropped entirely"
        );
        assert!(!normalized.contains_key("empty_after_normalize"));
        assert!(!normalized.contains_key("not_a_table"));
        assert!(!normalized.contains_key(""));
    }

    #[test]
    fn normalize_keeps_an_entry_explicitly_set_to_zero() {
        let mut root = toml::value::Table::new();
        let mut entry = toml::value::Table::new();
        entry.insert("last_server_timestamp".to_string(), toml::Value::Float(0.0));
        root.insert("zeroed".to_string(), toml::Value::Table(entry));

        let normalized = normalize_sync_state(&root);
        let kept = normalized
            .get("zeroed")
            .expect("explicit zero value keeps the entry");
        assert_eq!(kept.last_server_timestamp, 0.0);
    }

    // -- apply_sync_update ---------------------------------------------------

    #[test]
    fn apply_sync_update_merges_shallowly_and_preserves_other_fields() {
        let mut config = Config::default();
        let mut initial = toml::value::Table::new();
        let mut entry = toml::value::Table::new();
        entry.insert(
            "last_downloaded_save_id".to_string(),
            toml::Value::String("old-id".into()),
        );
        entry.insert(
            "last_server_timestamp".to_string(),
            toml::Value::Float(10.0),
        );
        initial.insert("rom:abc".to_string(), toml::Value::Table(entry));
        config.cloud_sync_state = initial;

        apply_sync_update(
            &mut config,
            "rom:abc",
            SyncStateUpdate {
                last_server_timestamp: Some(99.0),
                ..Default::default()
            },
        );

        let updated = sync_entry_for(&config, "rom:abc");
        assert_eq!(
            updated.last_downloaded_save_id, "old-id",
            "untouched field preserved"
        );
        assert_eq!(
            updated.last_server_timestamp, 99.0,
            "Some field overwritten"
        );
    }

    #[test]
    fn apply_sync_update_is_a_noop_for_a_blank_key() {
        let mut config = Config::default();
        apply_sync_update(
            &mut config,
            "",
            SyncStateUpdate {
                last_server_timestamp: Some(99.0),
                ..Default::default()
            },
        );
        assert!(config.cloud_sync_state.is_empty());
    }

    #[test]
    fn apply_sync_update_with_an_empty_update_creates_no_entry() {
        let mut config = Config::default();
        apply_sync_update(&mut config, "rom:new-game", SyncStateUpdate::default());
        assert!(
            config.cloud_sync_state.is_empty(),
            "an all-None update must not fabricate a phantom entry for a previously-absent key"
        );
        assert_eq!(
            sync_entry_for(&config, "rom:new-game"),
            SyncStateEntry::default()
        );
    }

    // -- auto_cloud_upload_plan --------------------------------------------

    #[test]
    fn upload_plan_requires_more_than_one_second_of_drift() {
        let entry = SyncStateEntry {
            last_uploaded_save_mtime: 100.0,
            ..Default::default()
        };
        let boundary = auto_cloud_upload_plan(&entry, 101.0, 0.0, false);
        assert!(
            boundary.types.is_empty(),
            "previous+1.0 exactly is NOT planned"
        );

        let over = auto_cloud_upload_plan(&entry, 101.000001, 0.0, false);
        assert_eq!(over.types, vec![SaveType::Save]);
        assert_eq!(over.latest_mtimes.get(&SaveType::Save), Some(&101.000001));
    }

    #[test]
    fn upload_plan_falls_back_to_the_legacy_mtime_field() {
        let entry = SyncStateEntry {
            last_uploaded_save_mtime: 0.0,
            last_uploaded_local_mtime: 50.0,
            ..Default::default()
        };
        let not_planned = auto_cloud_upload_plan(&entry, 51.0, 0.0, false);
        assert!(not_planned.types.is_empty());

        let planned = auto_cloud_upload_plan(&entry, 51.5, 0.0, false);
        assert_eq!(planned.types, vec![SaveType::Save]);
    }

    #[test]
    fn upload_plan_skips_state_when_include_state_is_false() {
        let entry = SyncStateEntry::default();
        let plan = auto_cloud_upload_plan(&entry, 0.0, 500.0, false);
        assert!(plan.types.is_empty());
        assert!(plan.latest_mtimes.is_empty());

        let plan_included = auto_cloud_upload_plan(&entry, 0.0, 500.0, true);
        assert_eq!(plan_included.types, vec![SaveType::State]);
    }

    // -- summarize_auto_cloud_upload_result ----------------------------------

    #[test]
    fn summarize_writes_both_save_mtime_fields_and_uploaded_at() {
        let mut per_type = BTreeMap::new();
        per_type.insert(
            SaveType::Save,
            PerTypeResult {
                uploaded: 2,
                total: 2,
                failed: Vec::new(),
            },
        );
        let mut latest_mtimes = BTreeMap::new();
        latest_mtimes.insert(SaveType::Save, 12345.0);

        let (update, _segments) =
            summarize_auto_cloud_upload_result(&per_type, &latest_mtimes, "2026-09-02T00:00:00Z");

        assert_eq!(update.last_uploaded_save_mtime, Some(12345.0));
        assert_eq!(update.last_uploaded_local_mtime, Some(12345.0));
        assert_eq!(
            update.last_uploaded_at,
            Some("2026-09-02T00:00:00Z".to_string())
        );
        assert_eq!(update.last_uploaded_state_mtime, None);
    }

    #[test]
    fn summarize_skips_an_all_zero_type_and_builds_debug_segments() {
        let mut per_type = BTreeMap::new();
        per_type.insert(
            SaveType::Save,
            PerTypeResult {
                uploaded: 1,
                total: 3,
                failed: vec![
                    "a.sav".to_string(),
                    "b.sav".to_string(),
                    "c.sav".to_string(),
                    "d.sav".to_string(),
                ],
            },
        );
        // State entry is all-zero/no-failures: must be skipped entirely.
        per_type.insert(
            SaveType::State,
            PerTypeResult {
                uploaded: 0,
                total: 0,
                failed: Vec::new(),
            },
        );
        let mut latest_mtimes = BTreeMap::new();
        latest_mtimes.insert(SaveType::Save, 99.0);

        let (update, segments) = summarize_auto_cloud_upload_result(&per_type, &latest_mtimes, "");

        assert_eq!(
            segments.len(),
            1,
            "the all-zero state entry produces no segment"
        );
        assert_eq!(segments[0], "save=1/3 failed=a.sav,b.sav,c.sav");
        assert_eq!(
            update.last_uploaded_at, None,
            "blank uploaded_at is never written"
        );
    }

    #[test]
    fn summarize_skips_last_uploaded_at_when_nothing_uploaded_even_if_failures_exist() {
        let mut per_type = BTreeMap::new();
        per_type.insert(
            SaveType::Save,
            PerTypeResult {
                uploaded: 0,
                total: 0,
                failed: vec!["a.sav".to_string()],
            },
        );
        let (update, segments) =
            summarize_auto_cloud_upload_result(&per_type, &BTreeMap::new(), "2026-09-02T00:00:00Z");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0], "save=0/0 failed=a.sav");
        assert_eq!(update.last_uploaded_at, None);
        assert_eq!(update.last_uploaded_save_mtime, None);
    }
}
