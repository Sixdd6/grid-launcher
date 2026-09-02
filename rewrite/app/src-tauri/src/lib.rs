mod cloud_service;
mod commands;
mod gamepad;

use commands::AppState;
use grid_core::autoconfig::RaCredentials;
use grid_core::config::Config;
use grid_core::launch::LaunchService;
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use grid_core::secrets::{KeyringStore, RaTokenStore};
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
    let session = Arc::new(SessionManager::new(
        Config::default_path(),
        cache_dir,
        Arc::new(KeyringStore::new()),
    ));
    let cloud = cloud_service::CloudService::new();
    // A SECOND, independent keyring item from the RomM credential above
    // (secrets.rs): clearing one must never clear the other.
    let ra_store: Arc<dyn RaTokenStore> = Arc::new(KeyringStore::new());
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
    let builder = tauri::Builder::default().manage(AppState {
        session,
        install,
        launch,
        ra_store,
        cloud,
    });
    // Embedded WebDriver automation server, gated behind the `e2e` cargo
    // feature so it never ships in a release build (see
    // rewrite/scripts/check_secret_hygiene.sh, which fails the build if
    // these plugins appear in the default dependency tree).
    #[cfg(feature = "e2e")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());
    builder
        .setup(|app| {
            gamepad::spawn(app.handle().clone());
            // The static scope in tauri.conf.json only covers the default
            // ProjectDirs cache location ($CACHE/grid-launcher/covers/**/*).
            // When GRID_LAUNCHER_DATA_DIR is set (E2E harness, and any real
            // portable-mode install), covers live under <data dir>/covers
            // instead, which that static scope never grants — every cover
            // request 404s with "asset protocol not configured to allow the
            // path". Extend the scope at runtime to cover it too.
            if let Some(dir) = grid_core::config::data_dir_override() {
                let covers_dir = dir.join("covers");
                if let Err(e) = app
                    .asset_protocol_scope()
                    .allow_directory(&covers_dir, true)
                {
                    tracing::warn!(
                        "failed to extend asset protocol scope for {}: {e}",
                        covers_dir.display()
                    );
                }
            }
            let state = app.state::<AppState>();
            if let Ok(install) = &state.install {
                let handle = app.handle().clone();
                install.set_notify(Arc::new(move |snapshot| {
                    let _ = handle.emit("downloads-changed", snapshot);
                }));
                // Reads the keyring and the config so D1 (a newly installed
                // or newly added emulator) picks up an existing
                // RetroAchievements login automatically, with no separate
                // wiring at either call site.
                let ra_store = state.ra_store.clone();
                install.set_ra_provider(Arc::new(move || {
                    let token = ra_store.load().ok().flatten()?;
                    let username = Config::load(&Config::default_path())
                        .map(|c| c.retroachievements_username)
                        .unwrap_or_default();
                    Some(RaCredentials::new(username, token))
                }));
            }
            if let Ok(launch) = &state.launch {
                let handle = app.handle().clone();
                launch.set_notify(Arc::new(move |snapshot| {
                    let _ = handle.emit("sessions-changed", snapshot);
                }));
                // Cloud auto-upload trigger: fires per reaped session,
                // after the notify emit above, with no lock held (see
                // `CloudService::install_session_finished_hook`).
                if let Ok(install) = &state.install {
                    state.cloud.install_session_finished_hook(
                        launch,
                        state.session.clone(),
                        install.clone(),
                        Config::default_path(),
                    );
                }
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
            commands::list_emulator_catalog,
            commands::install_emulator,
            commands::set_retroachievements_credentials,
            commands::get_retroachievements_status,
            commands::clear_retroachievements_credentials,
            commands::cloud::cloud_panel_info,
            commands::cloud::cloud_records,
            commands::cloud::cloud_upload,
            commands::cloud::cloud_restore,
            commands::cloud::cloud_delete,
            commands::cloud::native_save_paths,
            commands::cloud::native_add_manual_save_path,
            commands::cloud::native_remove_manual_save_path,
            commands::cloud::cloud_settings,
            commands::cloud::set_cloud_settings,
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
