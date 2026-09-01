mod commands;

use commands::AppState;
use grid_core::config::Config;
use grid_core::secrets::KeyringStore;
use grid_core::session::SessionManager;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging policy (spec, normative): default filter carries no request or
    // header data anywhere; secrets are structurally unloggable (SecretString).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cache_dir = directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
        .expect("home directory must exist")
        .cache_dir()
        .join("covers");
    let session = SessionManager::new(
        Config::default_path(),
        cache_dir,
        Arc::new(KeyringStore::new()),
    );
    tauri::Builder::default()
        .manage(AppState { session })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::restore_session,
            commands::disconnect,
            commands::list_platforms,
            commands::list_games,
            commands::ensure_cover,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
