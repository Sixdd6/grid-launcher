mod commands;
mod gamepad;

use commands::AppState;
use grid_core::config::Config;
use grid_core::launch::LaunchService;
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
    let cache_dir = grid_core::config::data_dir_override()
        .map(|d| d.join("covers"))
        .unwrap_or_else(|| {
            directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
                .expect("home directory must exist")
                .cache_dir()
                .join("covers")
        });
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
    // AppState and surfaced to the UI the first time an install/launch
    // command runs. Opened once and shared: install and launch each get
    // their own Arc clone of the same registry, and on failure both hold
    // the same error string.
    let registry = Registry::open(&db_path)
        .map(Arc::new)
        .map_err(|e| e.to_string());
    let install = registry
        .clone()
        .map(|registry| InstallService::new(registry, config_path.clone()));
    let launch = registry.map(|registry| LaunchService::new(registry, config_path));
    tauri::Builder::default()
        .manage(AppState {
            session,
            install,
            launch,
        })
        .setup(|app| {
            gamepad::spawn(app.handle().clone());
            let state = app.state::<AppState>();
            if let Ok(install) = &state.install {
                let handle = app.handle().clone();
                install.set_notify(Arc::new(move |snapshot| {
                    let _ = handle.emit("downloads-changed", snapshot);
                }));
            }
            if let Ok(launch) = &state.launch {
                let handle = app.handle().clone();
                launch.set_notify(Arc::new(move |snapshot| {
                    let _ = handle.emit("sessions-changed", snapshot);
                }));
                // `.setup` runs on the main thread with no tokio runtime
                // entered, but `spawn_poll_loop` calls `tokio::spawn`
                // internally, which panics ("there is no reactor running")
                // outside of one. Rule: any non-command Tauri code that
                // reaches `tokio::spawn` must go through
                // `tauri::async_runtime::spawn` (or otherwise run inside
                // Tauri's async runtime) first, so the inner `tokio::spawn`
                // inherits a valid context.
                let launch_for_loop = launch.clone();
                tauri::async_runtime::spawn(async move {
                    launch_for_loop.spawn_poll_loop();
                });
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
            commands::launch_game,
            commands::stop_game,
            commands::list_sessions,
            commands::list_emulators,
            commands::save_emulator,
            commands::delete_emulator,
            commands::list_profiles,
            commands::match_profile,
            commands::get_launch_defaults,
            commands::set_default_emulator,
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
