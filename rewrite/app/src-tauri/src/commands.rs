use grid_core::romm::{GameSummary, Platform};
use grid_core::session::{SessionManager, SessionState};
use secrecy::SecretString;
use tauri::State;

pub struct AppState {
    pub session: SessionManager,
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
