//! Q4 commands for installed rows with no ROM id: the Details "Link to
//! server game" picker and "Remove from library (keeps files)". Thin
//! wrappers — the matching rules live in `grid_core::library::relink`, the
//! SQL in `grid_core::library::registry`.

use grid_core::library::registry::{InstalledGame, SetRomIdOutcome};
use grid_core::library::relink::server_roms_for_platform;
use grid_core::romm::GameSummary;
use tauri::{AppHandle, State};

use super::{err, AppState};

/// The server ROMs of every platform `platform` (a registry row's platform
/// text) names: the picker's list.
#[tauri::command]
pub async fn list_server_roms_for_platform(
    state: State<'_, AppState>,
    platform: String,
) -> Result<Vec<GameSummary>, String> {
    let client = state.session.client().ok_or("not connected")?;
    server_roms_for_platform(&client, &platform)
        .await
        .map_err(err)
}

/// Links the row `(title, platform)` to `rom_id` and answers with the
/// updated row. Refuses an id another installed row holds. Starts the image
/// replenish (the row's art now comes from this rom) and the update
/// recompute (the row can now carry an update).
#[tauri::command]
pub async fn link_installed_row(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
    platform: String,
    rom_id: i64,
) -> Result<InstalledGame, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let registry = install.registry();
    let linked = tokio::task::spawn_blocking(move || {
        match registry
            .set_rom_id_by_key(&title, &platform, rom_id)
            .map_err(err)?
        {
            SetRomIdOutcome::Linked => {}
            SetRomIdOutcome::RomIdHeld => {
                return Err(
                    "Another game in your library is already linked to that server game."
                        .to_string(),
                )
            }
            SetRomIdOutcome::NoSuchRow => {
                return Err("This game is no longer in your library.".to_string())
            }
        }
        registry
            .find(Some(rom_id), "", "")
            .map_err(err)?
            .ok_or_else(|| "This game is no longer in your library.".to_string())
    })
    .await
    .map_err(|e| format!("link did not finish: {e}"))??;
    tracing::info!(
        title = %linked.title,
        rom_id,
        "relink: linked an installed game to a server ROM by hand"
    );
    state
        .images
        .spawn_replenish(app.clone(), state.session.clone(), install.clone());
    state
        .updates
        .spawn_refresh(app, state.session.clone(), install);
    Ok(linked)
}

/// Removes the row `(title, platform)` from the registry. Never deletes a
/// file: the game's folder stays on disk exactly as it is.
#[tauri::command]
pub async fn remove_from_library(
    state: State<'_, AppState>,
    title: String,
    platform: String,
) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let registry = install.registry();
    let removed = tokio::task::spawn_blocking(move || registry.remove(&title, &platform))
        .await
        .map_err(|e| format!("remove did not finish: {e}"))?
        .map_err(err)?;
    if removed {
        Ok(())
    } else {
        Err("This game is no longer in your library.".to_string())
    }
}
