//! RomM as a storefront (romm-storefront-plan 0.1, 0.6): the `romm_*`
//! commands and the `romm-changed` event. A facade over
//! `grid_core::session::SessionManager`, which owns the connection state and
//! the live client. Payloads carry the server URL, the username, the
//! credential kind and a secret-free error only — never a token or password.

use super::{err, AppState};
use grid_core::library::registry::InstalledGame;
use grid_core::session::{RommConnection, RommStatus};
use grid_core::stores::GameSource;
use secrecy::SecretString;
use tauri::{AppHandle, State};

/// Emitted after every RomM status change: connect, disconnect, reconnect,
/// restore, a mid-session 401, and a probe starting or ending. Payload:
/// [`RommStatus`].
pub const ROMM_CHANGED_EVENT: &str = "romm-changed";

/// Whether `row` counts as an installed RomM game: source RomM, and not the
/// hidden Emulators shelf (the frontend's `isHiddenLibraryPlatform`).
fn is_romm_game(row: &InstalledGame) -> bool {
    if row.source != GameSource::Romm {
        return false;
    }
    let platform = row.platform.trim().to_lowercase();
    platform != "emulator" && platform != "emulators"
}

fn count_romm_games(rows: &[InstalledGame]) -> u32 {
    rows.iter().filter(|r| is_romm_game(r)).count() as u32
}

/// RomM's status now. The installed count comes from the registry; a read
/// failure counts zero (the rail also reads the installed rows itself).
pub(crate) fn status_of(state: &AppState) -> RommStatus {
    let installed = state
        .install
        .as_ref()
        .ok()
        .and_then(|install| install.registry().all().ok())
        .map(|rows| count_romm_games(&rows))
        .unwrap_or(0);
    state.session.status(installed)
}

/// After a probe: a live client starts the connected jobs (relink, play
/// session flush, image replenish, update check), exactly as before.
fn after_probe(state: &AppState, app: AppHandle) -> RommStatus {
    let status = status_of(state);
    if status.connection == RommConnection::Connected {
        crate::relink_service::spawn_connected_jobs(state, app);
    }
    status
}

/// Pull, for stores that mount late.
#[tauri::command]
pub fn romm_status(state: State<'_, AppState>) -> RommStatus {
    status_of(&state)
}

/// Startup restore, run after the shell is up. Probes only when
/// `stores.romm.connected`; otherwise RomM is Not connected and neither the
/// keyring nor any server is touched. Sets the server URL for image
/// filtering whenever the config has one.
#[tauri::command]
pub async fn romm_restore(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<RommStatus, String> {
    state.session.restore_romm().await.map_err(err)?;
    Ok(after_probe(&state, app))
}

/// Connect (or re-sign-in) with a server URL and a token or password. Sets
/// `stores.romm.connected` in the same save. `UsernameMismatch` and the
/// probe's errors come back as the error text, secret-free.
#[tauri::command]
pub async fn romm_connect(
    state: State<'_, AppState>,
    app: AppHandle,
    server_url: String,
    username: String,
    secret: String,
    use_token: bool,
) -> Result<RommStatus, String> {
    // Wrap immediately; the plain String is dropped at the end of this scope.
    let secret = SecretString::from(secret);
    state
        .session
        .connect(server_url, username, secret, use_token)
        .await
        .map_err(err)?;
    Ok(after_probe(&state, app))
}

/// Re-probes with the stored credential (Retry, Reconnect). `Err` when no
/// credential is stored.
#[tauri::command]
pub async fn romm_reconnect(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<RommStatus, String> {
    state.session.reconnect().await.map_err(err)?;
    Ok(after_probe(&state, app))
}

/// Disconnect RomM (decision 5): drops the client, deletes the keyring
/// credential, sets `stores.romm.connected = false` and clears the update
/// set. Keeps the server URL and username for prefill. The shell stays.
#[tauri::command]
pub fn romm_disconnect(state: State<'_, AppState>, app: AppHandle) -> Result<RommStatus, String> {
    state.session.disconnect().map_err(err)?;
    // The update set describes a server that is no longer connected.
    state.updates.clear(&app);
    Ok(status_of(&state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::stores::StoreKind;

    fn row(platform: &str, source: GameSource) -> InstalledGame {
        InstalledGame {
            title: "A game".into(),
            platform: platform.into(),
            source,
            ..Default::default()
        }
    }

    #[test]
    fn only_romm_rows_off_the_emulators_shelf_count() {
        let rows = vec![
            row("Nintendo 64", GameSource::Romm),
            row("PlayStation", GameSource::Romm),
            row("Emulators", GameSource::Romm),
            row(" emulator ", GameSource::Romm),
            row("PC (Windows)", GameSource::Store(StoreKind::Gog)),
        ];
        assert_eq!(count_romm_games(&rows), 2);
    }
}
