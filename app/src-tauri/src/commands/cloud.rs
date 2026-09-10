//! Cloud save/state Tauri commands. Thin wrappers only: every real
//! decision (context building, `ops` calls, bookkeeping) lives in
//! `CloudService` (`../cloud_service.rs`) — see its module doc for the
//! Python anchors. `commands.rs` is already 1000+ lines, hence this
//! submodule (task-17-brief.md's "or a `commands/cloud.rs` submodule").

use grid_core::cloud::SaveType;
use grid_core::config::Config;
use tauri::State;

use super::AppState;
use crate::cloud_service::{
    CloudGameInput, CloudPanelInfoDto, CloudRecordDto, CloudSettingsDto, NativeSavePathsDto,
    RestoreReportDto, UploadReportDto,
};

#[tauri::command]
pub async fn cloud_panel_info(
    state: State<'_, AppState>,
    game: CloudGameInput,
    save_type: SaveType,
) -> Result<CloudPanelInfoDto, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .panel_info(install, launch, &Config::default_path(), game, save_type)
        .await
}

#[tauri::command]
pub async fn cloud_records(
    state: State<'_, AppState>,
    game: CloudGameInput,
    save_type: SaveType,
) -> Result<Vec<CloudRecordDto>, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .records(
            &state.session,
            install,
            launch,
            &Config::default_path(),
            game,
            save_type,
        )
        .await
}

#[tauri::command]
pub async fn cloud_upload(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    game: CloudGameInput,
    save_type: SaveType,
) -> Result<UploadReportDto, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .upload(
            &app,
            &state.session,
            install,
            launch,
            &Config::default_path(),
            game,
            save_type,
        )
        .await
}

#[tauri::command]
pub async fn cloud_restore(
    state: State<'_, AppState>,
    game: CloudGameInput,
    save_type: SaveType,
    record_id: Option<String>,
) -> Result<RestoreReportDto, String> {
    let record_id = match record_id {
        None => None,
        Some(raw) => Some(
            raw.trim()
                .parse::<i64>()
                .map_err(|_| format!("invalid record id: {raw}"))?,
        ),
    };
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .restore(
            &state.session,
            install,
            launch,
            &Config::default_path(),
            game,
            save_type,
            record_id,
        )
        .await
}

#[tauri::command]
pub async fn cloud_delete(
    state: State<'_, AppState>,
    save_type: SaveType,
    record_id: i64,
) -> Result<(), String> {
    state
        .cloud
        .delete(&state.session, save_type, record_id)
        .await
}

#[tauri::command]
pub async fn native_save_paths(
    state: State<'_, AppState>,
    game: CloudGameInput,
) -> Result<NativeSavePathsDto, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .native_save_paths(install, &Config::default_path(), game)
        .await
}

#[tauri::command]
pub async fn native_add_manual_save_path(
    state: State<'_, AppState>,
    game: CloudGameInput,
    path: String,
) -> Result<(), String> {
    state
        .cloud
        .native_add_manual_save_path(&Config::default_path(), game, path)
        .await
}

#[tauri::command]
pub async fn native_remove_save_path(
    state: State<'_, AppState>,
    game: CloudGameInput,
    path: String,
) -> Result<(), String> {
    state
        .cloud
        .native_remove_save_path(&Config::default_path(), game, path)
        .await
}

#[tauri::command]
pub async fn cloud_settings(state: State<'_, AppState>) -> Result<CloudSettingsDto, String> {
    state.cloud.settings(&Config::default_path()).await
}

#[tauri::command]
pub async fn set_cloud_settings(
    state: State<'_, AppState>,
    settings: CloudSettingsDto,
) -> Result<(), String> {
    state
        .cloud
        .set_settings(&Config::default_path(), settings)
        .await
}
