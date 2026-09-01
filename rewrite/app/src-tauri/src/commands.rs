use grid_core::config::Config;
use grid_core::library::queue::DownloadsSnapshot;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::romm::{GameSummary, Platform};
use grid_core::session::{SessionManager, SessionState};
use secrecy::SecretString;
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub session: SessionManager,
    pub install: Result<Arc<InstallService>, String>,
}

fn err(e: impl std::fmt::Display) -> String {
    // RommError/SessionError Display are credential-free by construction.
    e.to_string()
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    server_url: String,
    username: String,
    secret: String,
    use_token: bool,
) -> Result<SessionState, String> {
    // Wrap immediately; the plain String is dropped at the end of this scope.
    let secret = SecretString::from(secret);
    state
        .session
        .connect(server_url, username, secret, use_token)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn restore_session(state: State<'_, AppState>) -> Result<Option<SessionState>, String> {
    state.session.restore().await.map_err(err)
}

#[tauri::command]
pub fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    state.session.disconnect().map_err(err)
}

#[tauri::command]
pub async fn list_platforms(state: State<'_, AppState>) -> Result<Vec<Platform>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.platforms().await.map_err(err)
}

#[tauri::command]
pub async fn list_games(
    state: State<'_, AppState>,
    platform_id: i64,
) -> Result<Vec<GameSummary>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.games(platform_id).await.map_err(err)
}

#[tauri::command]
pub async fn ensure_cover(
    state: State<'_, AppState>,
    game_id: i64,
    cover_path: String,
) -> Result<String, String> {
    let client = state.session.client().ok_or("not connected")?;
    let path = state
        .session
        .cache()
        .ensure(&client, game_id, &cover_path)
        .await
        .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn install_game(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    let client = state.session.client().ok_or("not connected")?;
    install.install(client, rom_id).await.map_err(err)
}

#[tauri::command]
pub fn cancel_install(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.cancel(entry_id);
    Ok(())
}

#[tauri::command]
pub async fn retry_install(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    let client = state.session.client().ok_or("not connected")?;
    install.retry(client, entry_id).await.map_err(err)
}

#[tauri::command]
pub fn dismiss_download(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.dismiss(entry_id);
    Ok(())
}

#[tauri::command]
pub async fn uninstall_game(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || install.uninstall(rom_id).map_err(err))
        .await
        .map_err(|e| format!("uninstall did not finish: {e}"))?
}

#[tauri::command]
pub fn list_downloads(state: State<'_, AppState>) -> Result<DownloadsSnapshot, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    Ok(install.snapshot())
}

#[tauri::command]
pub async fn list_installed(state: State<'_, AppState>) -> Result<Vec<InstalledGame>, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || install.installed().map_err(err))
        .await
        .map_err(|e| format!("list_installed did not finish: {e}"))?
}

#[tauri::command]
pub async fn get_library_path() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.library_path)
    })
    .await
    .map_err(|e| format!("get_library_path did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_library_path(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let config_path = Config::default_path();
        let mut config = Config::load(&config_path).map_err(err)?;
        config.library_path = path;
        config.save(&config_path).map_err(err)
    })
    .await
    .map_err(|e| format!("set_library_path did not finish: {e}"))?
}
