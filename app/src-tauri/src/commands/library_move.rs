//! Settings › Library › Move existing files (phase 9b) and the
//! unreachable-library dialog. Thin wrappers around
//! `library_move_service.rs`; every rule lives there and in
//! `grid_core::library::relocate` / `root_access`. Payloads carry paths,
//! sizes, counts and sentences only.

use std::sync::Arc;

use grid_core::config::Config;
use tauri::{AppHandle, Emitter, State};

use super::AppState;
use crate::library_move_service::{
    CancelOutcome, MoveContext, MoveEvent, MoveStatus, PreviewOutcome, RelocateOutcome,
    ResumeOutcome, StartOutcome, UnreachableStatus, LIBRARY_MOVE_CHANGED_EVENT,
    LIBRARY_MOVE_FINISHED_EVENT, LIBRARY_PATH_CHANGED_EVENT, LIBRARY_UNREACHABLE_EVENT,
};
use crate::relink_service::INSTALLED_CHANGED_EVENT;

/// The service's view of the app: the install service, what is running,
/// the managed-key resync, and the events the frontend listens to.
pub(crate) fn move_context(state: &AppState, app: &AppHandle) -> Result<MoveContext, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let activity = {
        let install = install.clone();
        let launch = state.launch.as_ref().ok().cloned();
        let cloud = state.cloud.clone();
        Arc::new(move || super::library_path::activity_of(Some(&install), launch.as_ref(), &cloud))
    };
    let resync = {
        let install = install.clone();
        Arc::new(move |config: &Config| {
            super::resync_all_emulators(config, Some(&install));
            Ok(())
        })
    };
    let events = {
        let app = app.clone();
        let service = state.library_move.clone();
        let session = state.session.clone();
        let updates = state.updates.clone();
        let install = install.clone();
        Arc::new(move |event: MoveEvent| match event {
            MoveEvent::Changed => {
                let _ = app.emit(LIBRARY_MOVE_CHANGED_EVENT, service.status());
            }
            MoveEvent::Finished(finished) => {
                let _ = app.emit(LIBRARY_MOVE_FINISHED_EVENT, finished);
            }
            MoveEvent::Unreachable => {
                let _ = app.emit(LIBRARY_UNREACHABLE_EVENT, service.unreachable_status());
            }
            MoveEvent::LibraryChanged => {
                let _ = app.emit(INSTALLED_CHANGED_EVENT, 0usize);
                let _ = app.emit(LIBRARY_PATH_CHANGED_EVENT, ());
                updates.spawn_refresh(app.clone(), session.clone(), install.clone());
            }
        })
    };
    Ok(MoveContext {
        install,
        activity,
        resync,
        events,
        on_progress: None,
    })
}

async fn blocking<T: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| format!("{name} did not finish: {e}"))?
}

/// What a move to `path` would do: per folder, same drive (rename) or
/// copy, with the sizes and the free space. Reads only.
#[tauri::command]
pub async fn library_move_preview(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<PreviewOutcome, String> {
    let ctx = move_context(&state, &app)?;
    let service = state.library_move.clone();
    blocking("library_move_preview", move || service.preview(&ctx, &path)).await
}

/// Starts moving the library to `path` in the background.
#[tauri::command]
pub async fn library_move_start(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<StartOutcome, String> {
    let ctx = move_context(&state, &app)?;
    let service = state.library_move.clone();
    blocking("library_move_start", move || service.start(&ctx, &path)).await
}

/// Cancels the move while it copies.
#[tauri::command]
pub async fn library_move_cancel(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<CancelOutcome, String> {
    let ctx = move_context(&state, &app)?;
    let service = state.library_move.clone();
    blocking("library_move_cancel", move || service.cancel(&ctx)).await
}

/// Retry on a failed move row: runs the waiting move again.
#[tauri::command]
pub async fn library_move_resume(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ResumeOutcome, String> {
    let ctx = move_context(&state, &app)?;
    let service = state.library_move.clone();
    blocking("library_move_resume", move || service.resume(&ctx)).await
}

#[tauri::command]
pub async fn library_move_status(state: State<'_, AppState>) -> Result<MoveStatus, String> {
    let service = state.library_move.clone();
    blocking("library_move_status", move || Ok(service.status())).await
}

#[tauri::command]
pub fn library_unreachable_status(state: State<'_, AppState>) -> UnreachableStatus {
    state.library_move.unreachable_status()
}

/// The unreachable-library dialog's "Relocate the library…": the library
/// now lives at `path`. References are rewritten; no file moves.
#[tauri::command]
pub async fn library_relocate(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<RelocateOutcome, String> {
    let ctx = move_context(&state, &app)?;
    let service = state.library_move.clone();
    blocking("library_relocate", move || service.relocate(&ctx, &path)).await
}

/// The unreachable-library dialog's "Close without changing anything":
/// exits the app. Nothing is written.
#[tauri::command]
pub fn library_unreachable_close(app: AppHandle) {
    tracing::info!("closing: the library folder cannot be reached");
    app.exit(0);
}

#[cfg(test)]
mod tests {
    /// Every command that writes into the library (or starts something
    /// that does) refuses while the gate is closed. A source check, so a
    /// new early return cannot silently skip the call: the call must sit in
    /// the command's own body.
    #[test]
    fn every_library_command_checks_the_gate() {
        let sources: &[(&str, &str, &[&str])] = &[
            (
                "commands.rs",
                include_str!("../commands.rs"),
                &[
                    "uninstall_game",
                    "set_library_path",
                    "save_emulator",
                    "delete_emulator",
                    "launch_emulator",
                    "install_emulator",
                    "update_emulator",
                    "install_firmware_for_platform",
                    "set_retroachievements_credentials",
                    "retry_install",
                    "install_game",
                    "launch_game",
                ],
            ),
            (
                "library_path.rs",
                include_str!("library_path.rs"),
                &[
                    "library_start_fresh_preview",
                    "library_start_fresh_keep",
                    "library_start_fresh_delete",
                ],
            ),
            (
                "specials.rs",
                include_str!("specials.rs"),
                &["install_compat_tool", "install_ps3_firmware"],
            ),
            (
                "cloud.rs",
                include_str!("cloud.rs"),
                &[
                    "cloud_upload",
                    "cloud_restore",
                    "native_add_manual_save_path",
                    "native_remove_save_path",
                ],
            ),
            (
                "relink.rs",
                include_str!("relink.rs"),
                &["link_installed_row", "remove_from_library"],
            ),
            ("updates.rs", include_str!("updates.rs"), &["update_game"]),
        ];
        for (file, source, names) in sources {
            for name in *names {
                let needle = format!("fn {name}(");
                let start = source
                    .find(&needle)
                    .unwrap_or_else(|| panic!("{file}: no `{needle}`"));
                let rest = &source[start..];
                let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
                assert!(
                    rest[..end].contains("ensure_library_idle(&state)"),
                    "{file}: `{name}` does not call ensure_library_idle"
                );
            }
        }
    }
}
