//! App-layer glue for Q4 (installed rows with no ROM id). grid-core decides
//! WHAT links (`grid_core::library::relink`); this module decides WHEN: once
//! after every successful connect, restore or retry, before the image
//! replenish and the update recompute, so a row the pass links gets its
//! covers and its update check in the same session.
//!
//! When at least one row is linked, [`INSTALLED_CHANGED_EVENT`] tells the
//! frontend to re-read the registry. Logs carry titles, rom ids and counts —
//! never a URL, a header or a token.

use std::sync::Arc;

use grid_core::library::relink::relink_pass;
use grid_core::library::InstallService;
use grid_core::session::SessionManager;
use tauri::{AppHandle, Emitter};

use crate::commands::AppState;

/// Emitted when the installed-games registry changed outside a user action
/// the frontend already awaits (today: the relink pass linked a row).
/// Payload: the number of rows that changed. The installed store re-reads
/// the registry on it.
pub const INSTALLED_CHANGED_EVENT: &str = "installed-changed";

/// Runs the relink pass once. Offline, or when no row lacks a rom id, it
/// does nothing. Never fails the caller: a failed pass is logged and the
/// rows stay as they are until the next connect.
pub async fn relink_now(app: &AppHandle, session: &SessionManager, install: &InstallService) {
    let Some(client) = session.client() else {
        return;
    };
    match relink_pass(install.registry(), &client).await {
        Ok(linked) if !linked.is_empty() => {
            let _ = app.emit(INSTALLED_CHANGED_EVENT, linked.len());
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("relink: the pass did not finish: {e}"),
    }
}

/// The background jobs a live session starts, in one place for `connect`,
/// `restore_session` and `retry_connect`: the play-session flush, then — in
/// order — the relink pass, the image replenish and the update recompute.
/// Returns at once.
pub fn spawn_connected_jobs(state: &AppState, app: AppHandle) {
    let Ok(install) = state.install.as_ref() else {
        return;
    };
    state
        .play_sessions
        .spawn_flush(state.session.clone(), install.registry());
    let session: Arc<SessionManager> = state.session.clone();
    let install: Arc<InstallService> = install.clone();
    let images = state.images.clone();
    let updates = state.updates.clone();
    tauri::async_runtime::spawn(async move {
        relink_now(&app, &session, &install).await;
        images.spawn_replenish(app.clone(), session.clone(), install.clone());
        updates.spawn_refresh(app, session, install);
    });
}
