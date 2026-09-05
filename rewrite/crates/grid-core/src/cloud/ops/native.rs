//! The native (non-emulator, Windows-platform) cloud flows:
//! `_upload_native_saves_for_game` (`cloud_mixin.py:2668-2778`) and
//! `_restore_native_cloud_save_for_game` (`:2114-2252`).

use std::path::PathBuf;

use serde_json::Value;

use crate::library::paths::sanitize_component;
use crate::romm::RommClient;

use super::super::archive::{
    cleanup_temp_archives, extract_payload_zip, zip_native_save_dirs_for_upload,
};
use super::super::native::{
    native_save_paths, resolve_native_save_dir, restore_native_multi_dir_archive,
    visible_native_paths,
};
use super::super::restore::{latest_server_record, stringify_id};
use super::super::retention::prune_server_save_records;
use super::super::transfer::{no_jobs_message, upload_completion_message, UploadOutcome};
use super::super::{CloudGame, IgnoreSets, SaveType};
use super::upload::UploadReport;
use super::*;

/// `native_multi_dir` — the `emulator` field every combined native save
/// record carries (cloud_mixin.py:2741, :2760).
const NATIVE_MULTI_DIR: &str = "native_multi_dir";

const NO_SAVE_LOCATIONS: &str =
    "No save locations are configured for this game. Use 'Manage Saves' → 'Browse' to add one.";
/// The native restore's own "nothing on the server" text — note it is NOT
/// the emulator path's "No cloud save **was** found ..." wording
/// (cloud_mixin.py:2163 vs :1985).
const NO_NATIVE_CLOUD_SAVE: &str = "No cloud save found on the server for this game.";

/// The `native_manual_save_paths` key for `game`: `_pcgw_cache_key(game)`
/// (`details_view_mixin.py:152-153`, the trimmed title) plus the
/// `"__manual"` suffix the manual list is stored under
/// (`details_view_mixin.py`'s `manual_key`).
///
/// `pub`, not `pub(crate)`: the app layer's `native_add_manual_save_path` /
/// `native_remove_save_path` commands (task 17) write into
/// `config.native_manual_save_paths` and `config.native_removed_save_paths`
/// under this exact key and must use the same derivation this module reads
/// with, rather than recomputing it.
pub fn manual_paths_key(game: &CloudGame) -> String {
    format!("{}__manual", game.title.trim())
}

fn manual_paths_for<'a>(ctx: &'a CloudContext, game: &CloudGame) -> &'a [String] {
    ctx.config
        .native_manual_save_paths
        .get(&manual_paths_key(game))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// The rows the user deleted from this game's save-location list, PCGW or
/// manual (`config.native_removed_save_paths`, same key as the manual
/// list).
fn removed_paths_for<'a>(ctx: &'a CloudContext, game: &CloudGame) -> &'a [String] {
    ctx.config
        .native_removed_save_paths
        .get(&manual_paths_key(game))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Every save location this game still has: PCGW rows then manual ones,
/// minus the rows the user removed. Upload zips exactly these directories
/// and restore targets exactly these directories, so a removed row is gone
/// from both flows, not just from the popup's list.
fn configured_save_paths(
    ctx: &CloudContext,
    game: &CloudGame,
    pcgw_paths: &[String],
) -> Vec<String> {
    let (pcgw, manual) = visible_native_paths(
        pcgw_paths,
        manual_paths_for(ctx, game),
        removed_paths_for(ctx, game),
    );
    native_save_paths(&pcgw, &manual)
}

/// `_upload_native_saves_for_game` (cloud_mixin.py:2668). The total is
/// always 1 — one combined archive, one POST.
pub async fn upload_native_saves_for_game(
    client: &RommClient,
    ctx: &CloudContext<'_>,
    game: &CloudGame,
    pcgw_paths: &[String],
) -> UploadReport {
    // 1. Configured paths (PCGW + manual).
    let all_raw = configured_save_paths(ctx, game, pcgw_paths);
    if all_raw.is_empty() {
        return UploadReport::stop(CloudMessage::warning(NO_SAVE_LOCATIONS));
    }

    // 2. ROM id.
    let Some(rom_id) = cloud_sync_rom_id_with(ctx, game, SaveType::Save, "", None) else {
        return UploadReport::stop(CloudMessage::warning(MISSING_ROM_ID));
    };

    // 3. Expand and keep only paths that exist right now.
    let windows_documents = ctx.resolve_ctx.windows_documents;
    let expanded: Vec<(String, PathBuf)> = all_raw
        .iter()
        .map(|raw| {
            (
                raw.clone(),
                resolve_native_save_dir(raw, windows_documents, ctx.wine_prefix),
            )
        })
        .collect();
    let dir_map: Vec<(String, PathBuf)> = expanded
        .iter()
        .filter(|(_, path)| path.exists())
        .cloned()
        .collect();
    if dir_map.is_empty() {
        let listing = expanded
            .iter()
            .map(|(_, path)| format!("  • {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        return UploadReport::stop(CloudMessage::warning(format!(
            "None of the configured save locations exist on this device yet.\n\nChecked:\n{listing}"
        )));
    }

    // 4. One combined archive.
    let safe_title = sanitize_component(&game.title, "save");
    let (archive_path, total_files) =
        match zip_native_save_dirs_for_upload(&dir_map, &IgnoreSets::default()) {
            Ok(built) => built,
            Err(err) => {
                return UploadReport::stop(CloudMessage::warning(format!(
                    "Failed to create save archive: {err}"
                )))
            }
        };

    // 5. Nothing to send.
    if total_files == 0 {
        cleanup_temp_archives(&[archive_path]);
        return UploadReport::stop(CloudMessage::info(no_jobs_message(SaveType::Save)));
    }

    // 6. One multipart POST, archive always deleted afterwards.
    let payload = vec![("saveFile".to_string(), archive_path.clone())];
    let posted = client
        .upload_save(&rom_id, NATIVE_MULTI_DIR, None, &payload)
        .await;
    cleanup_temp_archives(&[archive_path]);
    let (uploaded, failed) = match posted {
        Ok(()) => (1usize, Vec::new()),
        Err(_) => (0usize, vec![safe_title]),
    };

    // 7. Retention pruning keyed on `native_multi_dir`.
    let retention_limit = ctx.config.cloud_save_retention_limit.max(1);
    let mut retention_failed: Vec<String> = Vec::new();
    if uploaded > 0 {
        let (_, failed_ids) =
            prune_server_save_records(client, &rom_id, NATIVE_MULTI_DIR, retention_limit).await;
        retention_failed = failed_ids;
    }

    let outcome = UploadOutcome {
        uploaded,
        total: 1,
        failed: failed.clone(),
    };
    let (text, severity) = upload_completion_message(
        &outcome,
        SaveType::Save,
        retention_failed.len(),
        retention_limit,
    );
    let messages = vec![CloudMessage { text, severity }];

    // 8. The total is always 1.
    UploadReport {
        uploaded,
        total: 1,
        failed,
        messages,
    }
}

/// `_restore_native_cloud_save_for_game` (cloud_mixin.py:2114). The whole
/// body is one `except Exception` in Python — any failure reports and
/// returns `false`.
pub async fn restore_native_cloud_save_for_game(
    client: &RommClient,
    ctx: &CloudContext<'_>,
    game: &CloudGame,
    pcgw_paths: &[String],
    record: Option<&Value>,
) -> (bool, Vec<CloudMessage>) {
    let windows_documents = ctx.resolve_ctx.windows_documents;
    let all_raw = configured_save_paths(ctx, game, pcgw_paths);
    let fallback_dirs: Vec<PathBuf> = all_raw
        .iter()
        .map(|raw| resolve_native_save_dir(raw, windows_documents, ctx.wine_prefix))
        .collect();

    let Some(rom_id) = cloud_sync_rom_id_with(ctx, game, SaveType::Save, "", None) else {
        return (false, vec![CloudMessage::warning(MISSING_ROM_ID)]);
    };

    // Record selection: Python calls `_latest_server_save_records_for_game`
    // with an EMPTY emulator name, whose scope is therefore `per-game` —
    // so exactly one (the latest) record is restored, not one per slot.
    // (doc 06 "Restore — native games" describes per-slot selection; the
    // code disagrees and the code wins.)
    let records: Vec<Value> = match record {
        Some(record) => vec![record.clone()],
        None => match fetch_cloud_records_for_rom(client, &rom_id, SaveType::Save).await {
            Ok(all) => latest_server_record(&all, "")
                .cloned()
                .into_iter()
                .collect(),
            Err(err) => {
                return (
                    false,
                    vec![CloudMessage::warning(format!(
                        "Failed to query server saves: {err}"
                    ))],
                )
            }
        },
    };

    if records.is_empty() {
        return (false, vec![CloudMessage::info(NO_NATIVE_CLOUD_SAVE)]);
    }

    for record in &records {
        let raw_id = record
            .get("id")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new()));
        let save_id = stringify_id(&raw_id).trim().to_string();
        if save_id.is_empty() {
            continue;
        }
        let payload = match client.save_content(&save_id).await {
            Ok(payload) => payload,
            Err(err) => return (false, vec![restore_failure(&err.to_string())]),
        };
        if payload.is_empty() {
            return (
                false,
                vec![restore_failure("Downloaded cloud save content was empty.")],
            );
        }

        let emulator_field = record_str(record, "emulator");
        let result = if emulator_field == NATIVE_MULTI_DIR {
            restore_native_multi_dir_archive(
                &payload,
                &fallback_dirs,
                windows_documents,
                ctx.wine_prefix,
            )
        } else if let Some(raw_path) = emulator_field.strip_prefix("native_dir:") {
            // Legacy per-directory format.
            let restore_dir = resolve_native_save_dir(raw_path, windows_documents, ctx.wine_prefix);
            match std::fs::create_dir_all(&restore_dir) {
                Ok(()) => extract_payload_zip(&payload, &restore_dir, &IgnoreSets::default()),
                Err(e) => Err(format!("failed to create directory: {e}")),
            }
        } else {
            match fallback_dirs.first() {
                None => Err("No restore directories configured.".to_string()),
                Some(restore_dir) => match std::fs::create_dir_all(restore_dir) {
                    Ok(()) => extract_payload_zip(&payload, restore_dir, &IgnoreSets::default()),
                    Err(e) => Err(format!("failed to create directory: {e}")),
                },
            }
        };
        if let Err(err) = result {
            return (false, vec![restore_failure(&err)]);
        }
    }

    (true, vec![CloudMessage::info(RESTORE_SUCCESS)])
}

fn restore_failure(error: &str) -> CloudMessage {
    CloudMessage::warning(format!("Failed to restore cloud save: {error}"))
}
