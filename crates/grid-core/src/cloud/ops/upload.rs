//! `_upload_cloud_files_for_game` (`cloud_mixin.py:2427-2666`): the
//! preconditions, per-branch job construction, screenshot fallback, slot
//! assignment, per-job execution with error isolation, temp cleanup, and
//! retention pruning.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::config::EmulatorEntry;
use crate::library::paths::sanitize_component;
use crate::romm::RommClient;

use super::super::archive::cleanup_temp_archives;
use super::super::dirs::resolved_screenshot_directories;
use super::super::retention::prune_server_save_records;
use super::super::scope::{is_native_executable_platform, SaveScope};
use super::super::tokens::psp_id_tokens;
use super::super::transfer::{
    directory_archive_upload_jobs, filter_upload_jobs_by_session_window, grouped_file_upload_jobs,
    no_jobs_message, ppsspp_state_upload_jobs, retroarch_state_upload_jobs,
    session_screenshot_path, shared_single_upload_job, upload_completion_message, BuiltJobs,
    UploadJob, UploadOutcome,
};
use super::super::xemu_sync::{build_xemu_save_archive, xemu_hdd_path_from_config};
use super::super::{CloudGame, IgnoreSets, SaveType};
use super::*;

/// `vmu([0-3])` — the slot regex `_cloud_save_slot_for_upload_job` runs
/// over the display name and every payload path's stem/name
/// (cloud_mixin.py:1638).
static VMU_SLOT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"vmu([0-3])").unwrap());

/// The PPSSPP-specific "nothing to upload" message (`cloud_upload.py:30`).
const NO_PPSSPP_JOBS: &str = "No matching PPSSPP .ppst state files were found to upload.";

/// `(success_count, total_job_count, failed_display_names)` plus the
/// dialog text Python would have shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UploadReport {
    pub uploaded: usize,
    pub total: usize,
    pub failed: Vec<String>,
    pub messages: Vec<CloudMessage>,
}

impl UploadReport {
    pub(crate) fn stop(message: CloudMessage) -> Self {
        Self {
            messages: vec![message],
            ..Default::default()
        }
    }
}

/// `_cloud_save_slot_for_upload_job` (cloud_mixin.py:1615-1648): states
/// never carry a slot; `shared-single` is the literal `shared-media`;
/// `shared-slotted` searches the display name, then each payload path's
/// stem and full name, for `vmu[0-3]`; `per-game` is empty.
pub(super) fn slot_for_job(
    scope: SaveScope,
    save_type: SaveType,
    job: &UploadJob,
) -> Option<String> {
    if save_type != SaveType::Save {
        return None;
    }
    if scope == SaveScope::SharedSingle {
        return Some("shared-media".to_string());
    }
    if scope != SaveScope::SharedSlotted {
        return None;
    }

    let mut candidates = vec![job.display_name.trim().to_lowercase()];
    for (_, path) in &job.payload {
        candidates.push(
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .trim()
                .to_lowercase(),
        );
        candidates.push(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .trim()
                .to_lowercase(),
        );
    }
    for candidate in candidates {
        if let Some(m) = VMU_SLOT_RE.captures(&candidate) {
            return Some(format!("vmu{}", &m[1]));
        }
    }
    None
}

/// `_upload_cloud_files_for_game(game, save_type)`
/// (cloud_mixin.py:2427-2666).
///
/// A Windows-platform game delegates to
/// [`super::native::upload_native_saves_for_game`] using `ctx.pcgw_paths`
/// — Python's precondition 1 (doc 06 "Upload planning").
pub async fn upload_cloud_files_for_game(
    client: &RommClient,
    ctx: &CloudContext<'_>,
    caches: &mut CloudCaches,
    game: &CloudGame,
    save_type: SaveType,
) -> UploadReport {
    let (name, entry) = resolved_cloud_emulator_pair(ctx, caches, game, save_type);

    // 1. Native Windows platform -> delegate.
    if is_native_executable_platform(&game.platform) {
        return super::native::upload_native_saves_for_game(client, ctx, game, ctx.pcgw_paths)
            .await;
    }

    // 2. Block reason.
    let reason = block_reason_for_game(ctx, game, save_type, entry.as_ref());
    if !reason.is_empty() {
        return UploadReport::stop(CloudMessage::info(reason));
    }

    // 3. ROM id.
    let Some(rom_id) = cloud_sync_rom_id_with(ctx, game, save_type, &name, entry.as_ref()) else {
        return UploadReport::stop(CloudMessage::warning(MISSING_ROM_ID));
    };

    // 4. Emulator entry.
    let Some(entry) = entry else {
        return UploadReport::stop(CloudMessage::warning(NO_DEFAULT_EMULATOR));
    };

    // 5. Sync directories.
    let (directories, explicit_file_roots) =
        resolved_sync_dirs(ctx, caches, &entry, path_key_for(save_type));
    if directories.is_empty() {
        return UploadReport::stop(CloudMessage::warning(no_directories_message(
            save_type, &name,
        )));
    }

    // 6. RPCS3 + state.
    if is_rpcs3(ctx, &name, Some(&entry)) && save_type == SaveType::State {
        return UploadReport::stop(CloudMessage::info(
            "RPCS3 savestate uploads are not supported yet.",
        ));
    }

    let scope = scope_for(ctx, &name, Some(&entry), &game.platform, save_type);

    let BuiltJobs {
        mut jobs,
        temp_archives,
    } = match build_jobs(
        ctx,
        game,
        &entry,
        &name,
        &directories,
        &explicit_file_roots,
        save_type,
        scope,
    ) {
        Ok(built) => built,
        Err(message) => return UploadReport::stop(message),
    };

    attach_screenshot_fallback(ctx, game, &entry, &mut jobs);

    // Execution: one POST per job, per-job error isolation.
    let mut uploaded = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for job in &jobs {
        let slot = slot_for_job(scope, save_type, job);
        let result = match save_type {
            SaveType::Save => {
                client
                    .upload_save(&rom_id, &name, slot.as_deref(), &job.payload)
                    .await
            }
            SaveType::State => client.upload_state(&rom_id, &name, &job.payload).await,
        };
        match result {
            Ok(()) => uploaded += 1,
            Err(_) => failed.push(job.display_name.clone()),
        }
    }

    cleanup_temp_archives(&temp_archives);

    // Retention pruning: saves only, at least one success. D7: the
    // configured limit is clamped to a minimum of 1 HERE, and the clamped
    // value is what both the prune and the completion message use.
    let retention_limit = ctx.config.cloud_save_retention_limit.max(1);
    let mut retention_failed: Vec<String> = Vec::new();
    if save_type == SaveType::Save && uploaded > 0 {
        let (_, failed_ids) =
            prune_server_save_records(client, &rom_id, &name, retention_limit).await;
        retention_failed = failed_ids;
    }

    let outcome = UploadOutcome {
        uploaded,
        total: jobs.len(),
        failed: failed.clone(),
    };
    let (text, severity) =
        upload_completion_message(&outcome, save_type, retention_failed.len(), retention_limit);
    let messages = vec![CloudMessage { text, severity }];

    UploadReport {
        uploaded,
        total: jobs.len(),
        failed,
        messages,
    }
}

/// Job construction per branch (cloud_mixin.py:2492-2595). `Err(message)`
/// is a stop-with-message; every "nothing to upload" path is one of
/// those, and it is raised exactly where Python raises it — after the
/// whole save-branch build, after the PPSSPP window filter, and BEFORE
/// building for the generic state branch (so a generic state branch that
/// builds zero jobs from non-empty candidates still falls through to the
/// completion message, exactly like Python).
#[allow(clippy::too_many_arguments)]
fn build_jobs(
    ctx: &CloudContext,
    game: &CloudGame,
    entry: &EmulatorEntry,
    name: &str,
    directories: &[PathBuf],
    explicit_file_roots: &[PathBuf],
    save_type: SaveType,
    scope: SaveScope,
) -> Result<BuiltJobs, CloudMessage> {
    let ignore = ignore_for(ctx, entry, name, save_type);

    if save_type == SaveType::Save {
        let (files, folders) = cloud_sync_targets_in(
            ctx,
            game,
            entry,
            name,
            directories,
            explicit_file_roots,
            SaveType::Save,
        );

        let mut built = directory_archive_upload_jobs(&folders, &ignore)
            .map_err(|e| CloudMessage::warning(format!("Failed to create save archive: {e}")))?;

        if is_xemu(ctx, name, Some(entry)) {
            // D1: xemu's single job comes from the raw HDD image, never
            // from generic file candidates.
            let hdd = xemu_hdd_path_from_config(&entry.path).unwrap_or_default();
            match build_xemu_save_archive(&hdd, &sanitize_component(name, "save")) {
                Ok(Some((archive, _))) => {
                    built.jobs.push(UploadJob {
                        display_name: shared_single_display_name(name),
                        payload: vec![("saveFile".to_string(), archive.clone())],
                    });
                    built.temp_archives.push(archive);
                }
                Ok(None) => {}
                Err(e) => {
                    cleanup_temp_archives(&built.temp_archives);
                    return Err(CloudMessage::warning(format!(
                        "Failed to create save archive: {e}"
                    )));
                }
            }
        } else if scope == SaveScope::SharedSingle && !files.is_empty() {
            let stem = if name.is_empty() {
                sanitize_component(&game.title, "save")
            } else {
                sanitize_component(name, "save")
            };
            let shared = shared_single_upload_job(&files, &shared_single_display_name(name), &stem)
                .map_err(|e| {
                    CloudMessage::warning(format!("Failed to create save archive: {e}"))
                })?;
            built.jobs.extend(shared.jobs);
            built.temp_archives.extend(shared.temp_archives);
        } else if !files.is_empty() {
            let grouped = grouped_file_upload_jobs(
                &files,
                "saveFile",
                &sanitize_component(&game.title, "save"),
            )
            .map_err(|e| CloudMessage::warning(format!("Failed to create save archive: {e}")))?;
            built.jobs.extend(grouped.jobs);
            built.temp_archives.extend(grouped.temp_archives);
        }

        if built.jobs.is_empty() {
            cleanup_temp_archives(&built.temp_archives);
            return Err(CloudMessage::info(no_jobs_message(SaveType::Save)));
        }
        return Ok(built);
    }

    if is_ppsspp(ctx, name, Some(entry)) {
        let jobs = ppsspp_state_upload_jobs(directories, &psp_id_tokens(game), &ignore);
        let filtered = filter_upload_jobs_by_session_window(jobs, window_for(ctx, game));
        if filtered.jobs.is_empty() {
            cleanup_temp_archives(&filtered.temp_archives);
            return Err(CloudMessage::info(NO_PPSSPP_JOBS));
        }
        return Ok(filtered);
    }

    let (files, _) = cloud_sync_targets_in(
        ctx,
        game,
        entry,
        name,
        directories,
        explicit_file_roots,
        SaveType::State,
    );
    if files.is_empty() {
        return Err(CloudMessage::info(no_jobs_message(SaveType::State)));
    }

    if is_retroarch(ctx, name, Some(entry)) {
        return Ok(retroarch_state_upload_jobs(&files, &ignore));
    }

    grouped_file_upload_jobs(
        &files,
        "stateFile",
        &sanitize_component(&game.title, "state"),
    )
    .map_err(|e| CloudMessage::warning(format!("Failed to create state archive: {e}")))
}

/// `f"{emulator_name or 'Shared Save'} Storage"` (cloud_mixin.py:2525).
pub(super) fn shared_single_display_name(name: &str) -> String {
    if name.is_empty() {
        "Shared Save Storage".to_string()
    } else {
        format!("{name} Storage")
    }
}

/// The screenshot fallback (cloud_mixin.py:2597-2604): the newest
/// in-window supported image from the profile's screenshot directories is
/// attached to EVERY job that does not already carry one.
///
/// Fix round 1 (ruling: Python wins): the scan uses
/// [`IgnoreSets::default`], NOT the emulator's resolved ignore sets —
/// `cloud_mixin.py:2600` calls `session_screenshot_path(screenshot_dirs,
/// session_win)` with no blocked names at all, so a configured
/// `ignore_files` entry must not narrow the screenshot search.
fn attach_screenshot_fallback(
    ctx: &CloudContext,
    game: &CloudGame,
    entry: &EmulatorEntry,
    jobs: &mut [UploadJob],
) {
    let profile = profile_for(ctx, entry);
    let emulator_dir = emulator_dir_for(entry);
    let rctx = resolve_ctx_for(ctx, emulator_dir.as_deref());
    let screenshot_dirs = resolved_screenshot_directories(entry, profile, &rctx);
    if screenshot_dirs.is_empty() {
        return;
    }
    let Some(shot) = session_screenshot_path(
        &screenshot_dirs,
        window_for(ctx, game),
        &IgnoreSets::default(),
    ) else {
        return;
    };
    for job in jobs.iter_mut() {
        if job
            .payload
            .iter()
            .any(|(field, _)| field == "screenshotFile")
        {
            continue;
        }
        job.payload
            .push(("screenshotFile".to_string(), shot.clone()));
    }
}
