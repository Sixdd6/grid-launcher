mod commands;
mod gamepad;

use commands::AppState;
use grid_core::config::Config;
use grid_core::secrets::KeyringStore;
use grid_core::session::SessionManager;
use std::sync::Arc;

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
    tauri::Builder::default()
        .manage(AppState { session })
        .setup(|app| {
            gamepad::spawn(app.handle().clone());
            Ok(())
        })
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
