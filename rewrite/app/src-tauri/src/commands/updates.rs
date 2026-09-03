//! Update commands (doc 10): the update set for the UI, the Update action,
//! the app's own version, and the release-page opener for the self-update
//! banner. Thin wrappers — every rule lives in `update_service`,
//! `app_update`, or grid-core.

use grid_core::library::platforms::is_native_platform;
use grid_core::library::update_detection::{game_has_server_update, ServerVersion};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{err, AppState};
use crate::update_service::{UpdateRow, UPDATE_GONE};

/// The only URL prefix `open_release_page` will hand to the OS.
pub const RELEASE_URL_PREFIX: &str = "https://github.com/Sixdd6/grid-launcher/releases/";

#[tauri::command]
pub async fn list_updates(state: State<'_, AppState>) -> Result<Vec<UpdateRow>, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let updates = state.updates.clone();
    tokio::task::spawn_blocking(move || {
        let installed = install.registry().all().map_err(err)?;
        Ok(updates.rows(&installed))
    })
    .await
    .map_err(|e| format!("list_updates did not finish: {e}"))?
}

/// `_perform_game_update_action` (details_view_mixin.py:1803-1884), minus
/// the modal: the frontend confirms native updates before calling this.
#[tauri::command]
pub async fn update_game(
    state: State<'_, AppState>,
    app: AppHandle,
    rom_id: i64,
) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let client = state.session.client().ok_or("not connected")?;
    let session = state.session.clone();
    let updates = state.updates.clone();

    let row = {
        let install = install.clone();
        tokio::task::spawn_blocking(move || install.registry().find(Some(rom_id), "", ""))
            .await
            .map_err(|e| format!("update_game did not finish: {e}"))?
            .map_err(err)?
            .filter(|row| row.rom_id == Some(rom_id))
            .ok_or_else(|| grid_core::library::NOT_INSTALLED.to_string())?
    };

    let detail = match client.rom_detail(rom_id).await {
        Ok(detail) => detail,
        Err(_) => {
            updates.spawn_refresh(app, session, install);
            return Err(UPDATE_GONE.to_string());
        }
    };
    let server = ServerVersion {
        platform: &detail.platform_name,
        rom_file_name: &detail.fs_name,
        updated_at: &detail.server_updated_at,
    };
    if !game_has_server_update(&row, &server) {
        updates.spawn_refresh(app, session, install);
        return Err(UPDATE_GONE.to_string());
    }
    if is_native_platform(&row.platform) {
        install
            .install_native_update(client, rom_id)
            .await
            .map_err(err)
    } else {
        install.install_update(client, rom_id).await.map_err(err)
    }
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

pub fn is_release_url(url: &str) -> bool {
    url.starts_with(RELEASE_URL_PREFIX)
}

#[tauri::command]
pub fn open_release_page(app: AppHandle, url: String) -> Result<(), String> {
    if !is_release_url(&url) {
        return Err("refusing to open a non-release URL".to_string());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::is_release_url;

    #[test]
    fn only_the_repo_release_prefix_opens() {
        assert!(is_release_url(
            "https://github.com/Sixdd6/grid-launcher/releases/tag/v1.0.0"
        ));
        assert!(!is_release_url("https://github.com/Sixdd6/grid-launcher/"));
        assert!(!is_release_url("https://example.com/releases/"));
        assert!(!is_release_url(
            "http://github.com/Sixdd6/grid-launcher/releases/tag/v1"
        ));
    }
}
