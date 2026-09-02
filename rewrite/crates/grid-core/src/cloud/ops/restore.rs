//! `_restore_cloud_save_for_game` (`cloud_mixin.py:1901-2112`) and
//! `_restore_cloud_state_for_game` (`:2254-2404`).
//!
//! Deviations honored here: **D6** (a multi-record shared-slotted restore
//! is staged in a temp directory and committed only when every record
//! downloaded and unpacked cleanly — Python aborts mid-loop, leaving a
//! partial slot set), **D2** (a legacy whole-image xemu record is skipped
//! with a notice rather than restored), and **D4** (absolute-URL state
//! content candidates are skipped rather than fetched).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::EmulatorEntry;
use crate::library::paths::sanitize_component;
use crate::romm::RommClient;

use super::super::archive::extract_payload_zip;
use super::super::candidates::file_candidates;
use super::super::restore::{
    latest_server_record, latest_server_records_by_slot, preferred_restore_target_path,
    record_timestamp, restore_single_save_payload, restore_single_state_payload, stringify_id,
};
use super::super::scope::{is_native_executable_platform, SaveScope};
use super::super::state::SyncStateUpdate;
use super::super::tokens::{game_save_match_tokens, ps2_serial_tokens};
use super::super::transfer::{
    is_local_newer_than_server, screenshot_download_candidate_paths, should_skip_known_latest,
    state_content_candidate_paths,
};
use super::super::xemu_sync::{
    archive_is_udata_tdata, inject_xemu_save_archive, xemu_hdd_path_from_config,
    LEGACY_RECORD_NOTICE,
};
use super::super::{CloudGame, IgnoreSets, SaveType};
use super::*;

const NO_CLOUD_SAVE: &str = "No cloud save was found on the server for this game.";
const NO_CLOUD_STATE: &str = "No cloud save state was found on the server for this game.";
const SAVE_RECORD_MISSING_ID: &str = "Server save record is missing an id.";
const STATE_RECORD_MISSING_ID: &str = "Server state record is missing an id.";
const SAVE_CONTENT_EMPTY: &str = "Downloaded cloud save content was empty.";
const STATE_CONTENT_EMPTY: &str = "Downloaded cloud state content was empty.";
const SAVE_ARCHIVE_NOTHING: &str = "Save archive downloaded, but no files were restored.";
const SAVE_NOTHING_RESTORED: &str = "Save content downloaded, but no file was restored.";
const STATE_NOTHING_RESTORED: &str = "State content downloaded, but no file was restored.";
const STATE_PATH_UNRESOLVED: &str = "State content path could not be resolved from server record.";
const RESTORE_STATE_SUCCESS: &str = "Cloud state restored successfully.";

type RestoreOutcome = (bool, Vec<CloudMessage>, SyncStateUpdate);

fn stop(message: CloudMessage) -> RestoreOutcome {
    (false, vec![message], SyncStateUpdate::default())
}

fn silent_stop() -> RestoreOutcome {
    (false, Vec::new(), SyncStateUpdate::default())
}

fn record_id(record: &Value) -> String {
    let raw = record
        .get("id")
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()));
    stringify_id(&raw).trim().to_string()
}

/// The record-emulator override rule (cloud_mixin.py:1944-1958, mirrored
/// at :2288-2302). Returns `Err` with the refusal message when the record
/// names an emulator that is neither configured locally nor the already
/// resolved one.
fn apply_record_emulator_override(
    ctx: &CloudContext,
    record: Option<&Value>,
    name: &mut String,
    entry: &mut Option<EmulatorEntry>,
) -> Result<(), CloudMessage> {
    let requested = record
        .map(|r| record_str(r, "emulator"))
        .unwrap_or_default();
    if requested.is_empty() {
        return Ok(());
    }
    let configured = emulator_entry_by_name(&ctx.config.emulators, &requested).cloned();
    match configured {
        None if requested.to_lowercase() != name.trim().to_lowercase() => {
            Err(CloudMessage::warning(format!(
                "Emulator '{requested}' is not configured on this device."
            )))
        }
        None => Ok(()),
        Some(found) => {
            *name = requested;
            *entry = Some(found);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------
// Save restore
// ---------------------------------------------------------------------

/// `_restore_cloud_save_for_game` (cloud_mixin.py:1901). The returned
/// [`SyncStateUpdate`] carries `last_downloaded_save_id` /
/// `last_server_timestamp` on success; the caller applies and saves it
/// (parity: Python writes the whole config on every update).
pub async fn restore_cloud_save_for_game(
    client: &RommClient,
    ctx: &CloudContext<'_>,
    caches: &mut CloudCaches,
    game: &CloudGame,
    record: Option<&Value>,
    skip_if_local_newer: bool,
    skip_if_known_latest: bool,
) -> RestoreOutcome {
    // 1. Native Windows platform -> delegate.
    if is_native_executable_platform(&game.platform) {
        let (ok, messages) = super::native::restore_native_cloud_save_for_game(
            client,
            ctx,
            game,
            ctx.pcgw_paths,
            record,
        )
        .await;
        return (ok, messages, SyncStateUpdate::default());
    }

    let (mut name, mut entry) = resolved_cloud_emulator_pair(ctx, caches, game, SaveType::Save);

    // 2. Block reason.
    let reason = block_reason_for_game(ctx, game, SaveType::Save, entry.as_ref());
    if !reason.is_empty() {
        return stop(CloudMessage::info(reason));
    }

    // 3. ROM id (shared-owner indirection applies for saves).
    let Some(rom_id) = cloud_sync_rom_id_with(ctx, game, SaveType::Save, &name, entry.as_ref())
    else {
        return stop(CloudMessage::warning(MISSING_ROM_ID));
    };

    // 4. Record-emulator override.
    if let Err(message) = apply_record_emulator_override(ctx, record, &mut name, &mut entry) {
        return stop(message);
    }

    // 5. Emulator entry + directories.
    let Some(entry) = entry else {
        return stop(CloudMessage::warning(NO_DEFAULT_EMULATOR));
    };
    let (directories, explicit_file_roots) =
        resolved_sync_dirs(ctx, caches, &entry, PathKey::SavePaths);
    if directories.is_empty() {
        return stop(CloudMessage::warning(no_directories_message(
            SaveType::Save,
            &name,
        )));
    }

    // 6. Record selection.
    let scope = scope_for(ctx, &name, Some(&entry), &game.platform, SaveType::Save);
    let records: Vec<Value> = match record {
        Some(record) => vec![record.clone()],
        None => {
            let all = match fetch_cloud_records_for_rom(client, &rom_id, SaveType::Save).await {
                Ok(all) => all,
                Err(err) => {
                    return stop(CloudMessage::warning(format!(
                        "Failed to query server saves: {err}"
                    )))
                }
            };
            if scope == SaveScope::PerGame {
                latest_server_record(&all, &name)
                    .cloned()
                    .into_iter()
                    .collect()
            } else {
                latest_server_records_by_slot(&all, &name)
            }
        }
    };

    if records.is_empty() {
        return stop(CloudMessage::info(NO_CLOUD_SAVE));
    }

    let save_id = record_id(&records[0]);
    if save_id.is_empty() {
        return stop(CloudMessage::warning(SAVE_RECORD_MISSING_ID));
    }

    // 7a. Known-latest short circuit — per-game scope only.
    if skip_if_known_latest && scope == SaveScope::PerGame {
        let stored = sync_entry_for(ctx.config, &game_key(game)).last_downloaded_save_id;
        let stored = stored.trim().to_string();
        if !stored.is_empty() && stored == save_id {
            let local = latest_local_save_mtime(ctx, caches, game, &name);
            if should_skip_known_latest(&stored, &save_id, local) {
                return silent_stop();
            }
        }
    }

    let server_latest_timestamp = records.iter().map(record_timestamp).fold(0.0_f64, f64::max);

    // 7b. Local-newer short circuit — PCSX2 without serials is exempt.
    if skip_if_local_newer {
        let pcsx2_without_serials =
            is_pcsx2(ctx, &name, Some(&entry)) && ps2_serial_tokens(game).is_empty();
        if !pcsx2_without_serials {
            let local = latest_local_save_mtime(ctx, caches, game, &name);
            if is_local_newer_than_server(local, server_latest_timestamp) {
                return silent_stop();
            }
        }
    }

    let ignore = ignore_for(ctx, &entry, &name, SaveType::Save);

    // 8. xemu (D1/D2): the payload is injected into the raw HDD image.
    if is_xemu(ctx, &name, Some(&entry)) {
        return restore_xemu(client, &entry, &records, save_id, server_latest_timestamp).await;
    }

    let is_folder_save = is_ppsspp(ctx, &name, Some(&entry))
        || is_rpcs3(ctx, &name, Some(&entry))
        || is_pcsx2(ctx, &name, Some(&entry))
        || is_cemu(ctx, &name, Some(&entry));

    let plan = RestorePlan {
        directories: &directories,
        explicit_file_roots: &explicit_file_roots,
        ignore: &ignore,
        is_folder_save,
        fallback_name: format!("{}.srm", sanitize_component(&game.title, "save")),
        tokens_source: game,
    };

    let placement = if records.len() == 1 {
        place_directly(client, &plan, &records[0]).await
    } else {
        // D6: stage every record, commit only when all succeeded.
        place_staged(client, &plan, &records).await
    };
    if let Err(message) = placement {
        return stop(CloudMessage::warning(format!(
            "Failed to restore cloud save: {message}"
        )));
    }

    // 9. Persist the newest restored id + timestamp.
    let (latest_restored_id, latest_server_timestamp) =
        newest_restored(&records, save_id, server_latest_timestamp);

    (
        true,
        vec![CloudMessage::info(RESTORE_SUCCESS)],
        SyncStateUpdate {
            last_downloaded_save_id: Some(latest_restored_id),
            last_server_timestamp: Some(latest_server_timestamp),
            ..Default::default()
        },
    )
}

/// `cloud_mixin.py:2088-2091`'s running maximum, ported verbatim
/// including the `>=` comparison against a pre-seeded maximum (so only a
/// record whose timestamp equals the overall maximum ever claims the id).
fn newest_restored(records: &[Value], primary_id: String, seed_timestamp: f64) -> (String, f64) {
    let mut latest_id = primary_id;
    let mut latest_timestamp = seed_timestamp;
    for record in records {
        let id = record_id(record);
        let timestamp = record_timestamp(record);
        if timestamp >= latest_timestamp {
            latest_timestamp = timestamp;
            latest_id = id;
        }
    }
    (latest_id, latest_timestamp)
}

struct RestorePlan<'a> {
    directories: &'a [PathBuf],
    explicit_file_roots: &'a [PathBuf],
    ignore: &'a IgnoreSets,
    is_folder_save: bool,
    fallback_name: String,
    tokens_source: &'a CloudGame,
}

impl RestorePlan<'_> {
    /// The target path a single-file save record restores to
    /// (`_restore_single_save_file`, cloud_mixin.py:1839-1866).
    fn target_for(&self, record: &Value) -> Option<PathBuf> {
        let tokens = game_save_match_tokens(self.tokens_source);
        let candidates = file_candidates(
            self.directories,
            &tokens,
            SaveType::Save,
            self.ignore,
            self.explicit_file_roots,
        );
        preferred_restore_target_path(
            &record_str(record, "file_name"),
            &self.fallback_name,
            &candidates,
            self.directories,
        )
    }
}

async fn download_save(client: &RommClient, record: &Value) -> Result<Vec<u8>, String> {
    let id = record_id(record);
    if id.is_empty() {
        return Err(SAVE_RECORD_MISSING_ID.to_string());
    }
    let payload = client.save_content(&id).await.map_err(|e| e.to_string())?;
    if payload.is_empty() {
        return Err(SAVE_CONTENT_EMPTY.to_string());
    }
    Ok(payload)
}

/// Python's direct placement (cloud_mixin.py:2056-2091), used for a
/// single record.
async fn place_directly(
    client: &RommClient,
    plan: &RestorePlan<'_>,
    record: &Value,
) -> Result<(), String> {
    let payload = download_save(client, record).await?;
    if plan.is_folder_save {
        let extracted = extract_payload_zip(&payload, &plan.directories[0], plan.ignore)?;
        if extracted == 0 {
            return Err(SAVE_ARCHIVE_NOTHING.to_string());
        }
        return Ok(());
    }
    let Some(target) = plan.target_for(record) else {
        return Err(SAVE_NOTHING_RESTORED.to_string());
    };
    match restore_single_save_payload(&payload, &target, plan.ignore)? {
        Some(_) => Ok(()),
        None => Err(SAVE_NOTHING_RESTORED.to_string()),
    }
}

/// D6: download and unpack EVERY record into a staging temp directory
/// first; only when all succeed are the staged trees moved into place. A
/// failure before the commit leaves local files untouched.
///
/// **Boundary of the guarantee:** atomicity covers everything up to the
/// commit — every download, every unpack, and every target decision. The
/// commit itself is a plain per-file [`copy_tree`], not an atomic
/// rename-into-place, so an I/O failure DURING the commit (a full disk, a
/// revoked permission) can still leave some slots updated and others not.
/// Making that step atomic too would need a same-filesystem staging
/// directory plus a rename per file, which the temp-dir staging this
/// deviation specifies does not give. D6's stated goal — that a failed
/// DOWNLOAD never half-writes the slot set — holds.
async fn place_staged(
    client: &RommClient,
    plan: &RestorePlan<'_>,
    records: &[Value],
) -> Result<(), String> {
    let staging = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut commits: Vec<(PathBuf, PathBuf)> = Vec::new();

    for (index, record) in records.iter().enumerate() {
        let payload = download_save(client, record).await?;
        let stage = staging.path().join(index.to_string());
        fs::create_dir_all(&stage).map_err(|e| e.to_string())?;

        if plan.is_folder_save {
            let extracted = extract_payload_zip(&payload, &stage, plan.ignore)?;
            if extracted == 0 {
                return Err(SAVE_ARCHIVE_NOTHING.to_string());
            }
            commits.push((stage, plan.directories[0].clone()));
            continue;
        }

        let Some(target) = plan.target_for(record) else {
            return Err(SAVE_NOTHING_RESTORED.to_string());
        };
        let file_name = target
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("save"));
        let staged_target = stage.join(&file_name);
        if restore_single_save_payload(&payload, &staged_target, plan.ignore)?.is_none() {
            return Err(SAVE_NOTHING_RESTORED.to_string());
        }
        let commit_root = target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| plan.directories[0].clone());
        commits.push((stage, commit_root));
    }

    for (stage, root) in commits {
        copy_tree(&stage, &root)?;
    }
    Ok(())
}

/// Copies every file under `from` into `to`, preserving relative paths and
/// overwriting whatever is already there.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("failed to prepare destination: {e}"))?;
    let mut stack = vec![from.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("failed to read staged files: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("failed to read staged files: {e}"))?;
            let path = entry.path();
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                stack.push(path);
                continue;
            }
            let relative = path.strip_prefix(from).map_err(|e| e.to_string())?;
            let destination = to.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create directory: {e}"))?;
            }
            fs::copy(&path, &destination).map_err(|e| format!("failed to place file: {e}"))?;
        }
    }
    Ok(())
}

/// The xemu save restore (spec "xemu flow"): every record's payload is
/// downloaded and legacy-checked FIRST, then injected into the raw image.
///
/// D2: a legacy whole-image record is not restored — the notice is
/// returned as an Info message and the restore reports "nothing restored"
/// (`false`), never an error dialog.
async fn restore_xemu(
    client: &RommClient,
    entry: &EmulatorEntry,
    records: &[Value],
    primary_id: String,
    seed_timestamp: f64,
) -> RestoreOutcome {
    let hdd = xemu_hdd_path_from_config(&entry.path).unwrap_or_default();
    let mut payloads: Vec<Vec<u8>> = Vec::new();
    for record in records {
        match download_save(client, record).await {
            Ok(payload) => payloads.push(payload),
            Err(message) => {
                return stop(CloudMessage::warning(format!(
                    "Failed to restore cloud save: {message}"
                )))
            }
        }
    }

    // D2: legacy-check everything before touching the image at all.
    if payloads.iter().any(|p| !archive_is_udata_tdata(p)) {
        return (
            false,
            vec![CloudMessage::info(LEGACY_RECORD_NOTICE)],
            SyncStateUpdate::default(),
        );
    }

    for payload in &payloads {
        if let Err(message) = inject_xemu_save_archive(&hdd, payload) {
            return stop(CloudMessage::warning(format!(
                "Failed to restore cloud save: {message}"
            )));
        }
    }

    let (latest_restored_id, latest_server_timestamp) =
        newest_restored(records, primary_id, seed_timestamp);
    (
        true,
        vec![CloudMessage::info(RESTORE_SUCCESS)],
        SyncStateUpdate {
            last_downloaded_save_id: Some(latest_restored_id),
            last_server_timestamp: Some(latest_server_timestamp),
            ..Default::default()
        },
    )
}

// ---------------------------------------------------------------------
// State restore
// ---------------------------------------------------------------------

/// `_restore_cloud_state_for_game` (cloud_mixin.py:2254-2404).
pub async fn restore_cloud_state_for_game(
    client: &RommClient,
    ctx: &CloudContext<'_>,
    caches: &mut CloudCaches,
    game: &CloudGame,
    record: Option<&Value>,
    skip_if_known_latest: bool,
) -> RestoreOutcome {
    let (mut name, mut entry) = resolved_cloud_emulator_pair(ctx, caches, game, SaveType::State);

    let reason = block_reason_for_game(ctx, game, SaveType::State, entry.as_ref());
    if !reason.is_empty() {
        return stop(CloudMessage::info(reason));
    }

    // States take the ROM id straight from the game — never the
    // shared-owner indirection.
    let rom_id = game.rom_id.trim().to_string();
    if rom_id.is_empty() {
        return stop(CloudMessage::warning(MISSING_ROM_ID));
    }

    if let Err(message) = apply_record_emulator_override(ctx, record, &mut name, &mut entry) {
        return stop(message);
    }

    let Some(entry) = entry else {
        return stop(CloudMessage::warning(NO_DEFAULT_EMULATOR));
    };

    if is_rpcs3(ctx, &name, Some(&entry)) {
        return stop(CloudMessage::info(
            "RPCS3 savestate restore is not supported yet.",
        ));
    }

    let (directories, explicit_file_roots) =
        resolved_sync_dirs(ctx, caches, &entry, PathKey::StatePaths);
    if directories.is_empty() {
        return stop(CloudMessage::warning(no_directories_message(
            SaveType::State,
            &name,
        )));
    }

    let selected: Option<Value> = match record {
        Some(record) => Some(record.clone()),
        None => match fetch_cloud_records_for_rom(client, &rom_id, SaveType::State).await {
            Ok(all) => latest_server_record(&all, &name).cloned(),
            Err(err) => {
                return stop(CloudMessage::warning(format!(
                    "Failed to query server states: {err}"
                )))
            }
        },
    };
    let Some(state_record) = selected else {
        return stop(CloudMessage::info(NO_CLOUD_STATE));
    };

    let state_id = record_id(&state_record);
    if state_id.is_empty() {
        return stop(CloudMessage::warning(STATE_RECORD_MISSING_ID));
    }

    // The known-latest short circuit always applies to states.
    if skip_if_known_latest {
        let stored = sync_entry_for(ctx.config, &game_key(game))
            .last_downloaded_state_id
            .trim()
            .to_string();
        if !stored.is_empty() && stored == state_id {
            let local = latest_local_state_mtime(ctx, caches, game, &name);
            if should_skip_known_latest(&stored, &state_id, local) {
                return silent_stop();
            }
        }
    }

    let payload = match download_state_content(client, &state_id).await {
        Ok(payload) => payload,
        Err(err) => {
            return stop(CloudMessage::warning(format!(
                "Failed to download cloud state content: {err}"
            )))
        }
    };
    if payload.is_empty() {
        return stop(CloudMessage::warning(STATE_CONTENT_EMPTY));
    }

    // Best effort: a screenshot failure is ignored entirely.
    let screenshot = download_state_screenshot(client, &state_record).await;

    let ignore = ignore_for(ctx, &entry, &name, SaveType::State);
    let tokens = game_save_match_tokens(game);
    let candidates = file_candidates(
        &directories,
        &tokens,
        SaveType::State,
        &ignore,
        &explicit_file_roots,
    );
    let fallback_name = format!("{}.state", sanitize_component(&game.title, "state"));
    let Some(target) = preferred_restore_target_path(
        &record_str(&state_record, "file_name"),
        &fallback_name,
        &candidates,
        &directories,
    ) else {
        return stop(CloudMessage::warning(STATE_NOTHING_RESTORED));
    };

    let screenshot_ref = screenshot
        .as_ref()
        .map(|(bytes, ext)| (bytes.as_slice(), ext.as_str()));
    match restore_single_state_payload(&payload, &target, screenshot_ref, &ignore) {
        Ok(Some(_)) => {}
        Ok(None) => return stop(CloudMessage::warning(STATE_NOTHING_RESTORED)),
        Err(err) => {
            return stop(CloudMessage::warning(format!(
                "Failed to restore cloud state: {err}"
            )))
        }
    }

    (
        true,
        vec![CloudMessage::info(RESTORE_STATE_SUCCESS)],
        SyncStateUpdate {
            last_downloaded_state_id: Some(state_id),
            ..Default::default()
        },
    )
}

/// `_download_server_state_content` (cloud_mixin.py:1777-1795): fetch the
/// record, then walk its download candidates through
/// [`RommClient::get_relative_bytes`], moving to the next candidate on any
/// error. D4: an absolute `http(s)://` candidate is rejected by the client
/// and therefore skipped. Every candidate failing yields Python's
/// `ValueError` text verbatim.
async fn download_state_content(client: &RommClient, state_id: &str) -> Result<Vec<u8>, String> {
    let record = client
        .state_record(state_id)
        .await
        .map_err(|e| e.to_string())?;
    if !record.is_object() {
        return Err("State record payload is invalid.".to_string());
    }
    for candidate in state_content_candidate_paths(&record) {
        if let Ok(bytes) = client.get_relative_bytes(&candidate).await {
            return Ok(bytes);
        }
    }
    Err(STATE_PATH_UNRESOLVED.to_string())
}

/// `_download_screenshot_from_state_record` (cloud_mixin.py:1797-1824):
/// the LIST record's own `screenshot` object, skipped when
/// `missing_from_fs` is true or no candidate resolves. The extension
/// defaults to `.png`.
async fn download_state_screenshot(
    client: &RommClient,
    state_record: &Value,
) -> Option<(Vec<u8>, String)> {
    let screenshot = state_record.get("screenshot")?;
    if !screenshot.is_object() {
        return None;
    }
    if screenshot.get("missing_from_fs") == Some(&Value::Bool(true)) {
        return None;
    }
    let candidates = screenshot_download_candidate_paths(screenshot);
    if candidates.is_empty() {
        return None;
    }
    let extension = {
        let raw = record_str(screenshot, "file_extension");
        if raw.is_empty() {
            ".png".to_string()
        } else {
            raw
        }
    };
    for candidate in candidates {
        if let Ok(bytes) = client.get_relative_bytes(&candidate).await {
            return Some((bytes, extension));
        }
    }
    None
}
