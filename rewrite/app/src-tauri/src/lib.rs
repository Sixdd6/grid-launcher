mod commands;
mod gamepad;

use commands::AppState;
use grid_core::config::Config;
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use grid_core::secrets::KeyringStore;
use grid_core::session::SessionManager;
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// WebKitGTK's DMABUF renderer fails to allocate GBM buffers on some
/// NVIDIA/Wayland stacks ("Failed to create GBM buffer ... Invalid argument"),
/// leaving the webview blank white. Returns true when the workaround variable
/// should be set: only if the user has not already chosen a value.
fn dmabuf_override_needed(existing: Option<std::ffi::OsString>) -> bool {
    existing.is_none()
}

#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    if dmabuf_override_needed(std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER")) {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must run before the webview process is created.
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();
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
    let config_path = Config::default_path();
    // Same directory as config.toml — never re-derive ProjectDirs separately.
    let db_path = config_path
        .parent()
        .expect("config path has a parent directory")
        .join("grid-launcher.db");
    // A registry open failure must not crash startup: it is carried into
    // AppState and surfaced to the UI the first time an install command runs.
    let install = Registry::open(&db_path)
        .map(|registry| InstallService::new(Arc::new(registry), config_path))
        .map_err(|e| e.to_string());
    tauri::Builder::default()
        .manage(AppState { session, install })
        .setup(|app| {
            gamepad::spawn(app.handle().clone());
            if let Ok(install) = &app.state::<AppState>().install {
                let handle = app.handle().clone();
                install.set_notify(Arc::new(move |snapshot| {
                    let _ = handle.emit("downloads-changed", snapshot);
                }));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::restore_session,
            commands::disconnect,
            commands::list_platforms,
            commands::list_games,
            commands::ensure_cover,
            commands::install_game,
            commands::cancel_install,
            commands::retry_install,
            commands::dismiss_download,
            commands::uninstall_game,
            commands::list_downloads,
            commands::list_installed,
            commands::get_library_path,
            commands::set_library_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::dmabuf_override_needed;

    #[test]
    fn dmabuf_workaround_applies_only_when_unset() {
        assert!(dmabuf_override_needed(None));
        assert!(!dmabuf_override_needed(Some("0".into())));
        assert!(!dmabuf_override_needed(Some("1".into())));
        assert!(!dmabuf_override_needed(Some(String::new().into())));
    }
}
