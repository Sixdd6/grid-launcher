//! First-run emulator setup (docs/superpowers/plans/2026-10-28-first-run-emulator-setup.md).

use grid_core::config::Config;
use grid_core::launch::profiles::load_profiles;
use grid_core::launch::setup::{platform_setup, recommendations, PlatformSetup};
use grid_core::launch::source::HOST_PLATFORM;
use tauri::State;

use super::{err, list_platforms, AppState};

/// The setup screen's rows: every server platform that needs an emulator,
/// with the emulators GRID can install for it on this OS and the one it
/// recommends. Fetches the platforms through [`list_platforms`], so the
/// install service learns them (names, labels, slugs, ids) exactly as the
/// Server view's fetch would, before setup queues anything.
#[tauri::command]
pub async fn setup_platform_options(
    state: State<'_, AppState>,
) -> Result<Vec<PlatformSetup>, String> {
    let platforms = list_platforms(state.clone()).await?;
    let config = Config::load(&Config::default_path()).map_err(err)?;
    Ok(platform_setup(
        &platforms,
        load_profiles(),
        &config,
        HOST_PLATFORM,
        recommendations(),
    ))
}
