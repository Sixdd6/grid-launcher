//! First-run emulator setup (docs/superpowers/plans/2026-10-28-first-run-emulator-setup.md).

use std::path::Path;

use grid_core::autoconfig::installed_compatible_cores;
use grid_core::cloud::scope::is_emulators_platform;
use grid_core::config::Config;
use grid_core::launch::profiles::load_profiles;
use grid_core::launch::setup::{
    apply_pending_defaults, plan_setup, platform_setup, recommendations, PendingOutcome,
    PlatformSetup, ServerPackage, SetupChoice,
};
use grid_core::launch::source::HOST_PLATFORM;
use grid_core::romm::Platform;
use serde::Serialize;
use tauri::State;

use super::{ensure_library_idle, err, list_platforms, AppState};

/// Emitted when setup has applied (or given up on) platform defaults after
/// an emulator installed: a `Vec<PendingOutcome>`.
pub const SETUP_DEFAULTS_EVENT: &str = "setup-defaults";

/// The setup screen's rows: every server platform that needs an emulator,
/// with the emulators GRID can install for it on this OS — catalog
/// downloads and the server's own Emulators-platform packages — and the one
/// it recommends. Fetches the platforms through [`list_platforms`], so the
/// install service learns them (names, labels, slugs, ids) exactly as the
/// Server view's fetch would, before setup queues anything.
#[tauri::command]
pub async fn setup_platform_options(
    state: State<'_, AppState>,
) -> Result<Vec<PlatformSetup>, String> {
    let platforms = list_platforms(state.clone()).await?;
    let packages = server_packages(&state, &platforms).await;
    let config = Config::load(&Config::default_path()).map_err(err)?;
    Ok(platform_setup(
        &platforms,
        load_profiles(),
        &config,
        HOST_PLATFORM,
        recommendations(),
        &packages,
    ))
}

/// The packages on the server's Emulators platform, with their file names
/// (which say which OS a build is for) and whether each is installed. Best
/// effort: a server with no such platform, or one that cannot be read, just
/// offers catalog downloads.
async fn server_packages(
    state: &State<'_, AppState>,
    platforms: &[Platform],
) -> Vec<ServerPackage> {
    let Some(client) = state.session.client() else {
        return Vec::new();
    };
    let Some(shelf) = platforms
        .iter()
        .find(|p| is_emulators_platform(p.label()) || is_emulators_platform(&p.name))
    else {
        return Vec::new();
    };
    let games = match client.games(shelf.id).await {
        Ok(games) => games,
        Err(e) => {
            tracing::warn!("setup: the Emulators platform could not be read: {e}");
            return Vec::new();
        }
    };
    let installed: Vec<i64> = match state.install.as_ref() {
        Ok(install) => {
            let registry = install.registry();
            tauri::async_runtime::spawn_blocking(move || registry.all())
                .await
                .ok()
                .and_then(Result::ok)
                .map(|rows| rows.iter().filter_map(|r| r.rom_id).collect())
                .unwrap_or_default()
        }
        Err(_) => Vec::new(),
    };
    let mut packages = Vec::new();
    for game in games {
        let file_names = match client.rom_detail(game.id).await {
            Ok(detail) => detail.files.into_iter().map(|f| f.file_name).collect(),
            // Without its files the OS cannot be judged: offered as portable.
            Err(_) => Vec::new(),
        };
        packages.push(ServerPackage {
            rom_id: game.id,
            title: game.name,
            file_names,
            installed: installed.contains(&game.id),
        });
    }
    packages
}

/// Whether the app should show the platform picker after connecting.
#[derive(Debug, Clone, Serialize)]
pub struct SetupStatus {
    pub needed: bool,
    /// Platform labels still waiting on their emulator's install.
    pub pending: Vec<String>,
}

/// The `e2e` build shows the picker only when this is set: every stage group
/// but `first-run-setup` connects a fresh profile and expects the shell
/// straight after (e2e.sh sets `E2E_FIRST_RUN_SETUP`, which `wdio.conf.ts`
/// turns into this). A release build never reads it.
const E2E_FIRST_RUN_SETUP_VAR: &str = "GRID_LAUNCHER_E2E_FIRST_RUN_SETUP";

/// Whether the picker is shown: a fresh install only — setup never finished
/// or skipped, no emulator configured and no game installed. Someone
/// upgrading from a build without setup has one or the other. In an `e2e`
/// build it is also off unless the stage opted in.
fn setup_needed(
    completed: bool,
    has_emulators: bool,
    has_games: bool,
    e2e: bool,
    e2e_opt_in: bool,
) -> bool {
    if e2e && !e2e_opt_in {
        return false;
    }
    !completed && !has_emulators && !has_games
}

/// `needed` only for a fresh install (see [`setup_needed`]).
#[tauri::command]
pub async fn setup_status(state: State<'_, AppState>) -> Result<SetupStatus, String> {
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let has_games = match state.install.as_ref() {
        Ok(install) => {
            let registry = install.registry();
            tauri::async_runtime::spawn_blocking(move || registry.all())
                .await
                .map_err(err)?
                .map(|rows| !rows.is_empty())
                .unwrap_or(true)
        }
        // No install service: be conservative and do not offer setup.
        Err(_) => true,
    };
    Ok(SetupStatus {
        needed: setup_needed(
            config.setup.completed,
            !config.emulators.is_empty(),
            has_games,
            cfg!(feature = "e2e"),
            std::env::var_os(E2E_FIRST_RUN_SETUP_VAR).is_some(),
        ),
        pending: config.setup.pending.keys().cloned().collect(),
    })
}

/// The user chose "Skip for now": setup is not offered on its own again.
#[tauri::command]
pub async fn skip_platform_setup() -> Result<(), String> {
    let path = Config::default_path();
    let mut config = Config::load(&path).map_err(err)?;
    config.setup.completed = true;
    config.save(&path).map_err(err)
}

/// What starting setup did.
#[derive(Debug, Clone, Serialize)]
pub struct SetupStarted {
    /// Catalog `source_id`s queued for install.
    pub queued: Vec<String>,
    /// Server Emulators-platform packages (ROM ids) queued for install.
    pub queued_server: Vec<i64>,
    /// Defaults applied at once, for emulators that were already configured.
    pub applied: Vec<PendingOutcome>,
    /// Installs that could not be queued, as user-facing lines.
    pub errors: Vec<String>,
}

/// Starts setup: checks `choices` against what [`setup_platform_options`]
/// offers, records the wanted default for every chosen platform, marks setup
/// done, applies defaults for emulators already configured, and queues one
/// catalog install per remaining emulator. The pending defaults are saved
/// BEFORE anything is queued, so even an install that finishes at once finds
/// them (the emulator-installed hook applies them: [`apply_after_install`]).
#[tauri::command]
pub async fn start_platform_setup(
    state: State<'_, AppState>,
    choices: Vec<SetupChoice>,
) -> Result<SetupStarted, String> {
    ensure_library_idle(&state)?;
    let rows = setup_platform_options(state.clone()).await?;
    let plan = plan_setup(&rows, &choices)?;

    let path = Config::default_path();
    let mut config = Config::load(&path).map_err(err)?;
    if config.library_path.trim().is_empty() {
        return Err("Choose a library folder before installing emulators.".to_string());
    }
    config.setup.pending.extend(plan.pending);
    config.setup.completed = true;
    let mut applied = Vec::new();
    for name in &plan.already_installed {
        applied.extend(apply_pending_defaults(
            &mut config,
            name,
            None,
            &cores_on_disk,
        ));
    }
    config.save(&path).map_err(err)?;

    let install = state.install.as_ref().map_err(Clone::clone)?;
    let mut queued = Vec::new();
    let mut errors = Vec::new();
    for source_id in plan.installs {
        match install.install_emulator(source_id.clone()).await {
            Ok(()) => queued.push(source_id),
            Err(e) => errors.push(e.to_string()),
        }
    }
    // A server package installs like any other ROM; registering it as an
    // emulator happens when it lands (`server_emulator`).
    let mut queued_server = Vec::new();
    if !plan.server_installs.is_empty() {
        match state.session.client() {
            Some(client) => {
                for rom_id in plan.server_installs {
                    match install.install(client.clone(), rom_id).await {
                        Ok(()) => queued_server.push(rom_id),
                        Err(e) => errors.push(e.to_string()),
                    }
                }
            }
            None => {
                errors.push("Not connected: the server's emulators were not installed.".to_string())
            }
        }
    }
    Ok(SetupStarted {
        queued,
        queued_server,
        applied,
        errors,
    })
}

/// The emulator-installed hook's half of setup: applies the pending defaults
/// waiting on `emulator_name` (or, for a server package, on its
/// `server_rom_id`) and saves. An empty result means there was nothing
/// pending for it (the config is not rewritten). Blocking.
pub fn apply_after_install(
    config_path: &Path,
    emulator_name: &str,
    server_rom_id: Option<i64>,
) -> Vec<PendingOutcome> {
    let Ok(mut config) = Config::load(config_path) else {
        return Vec::new();
    };
    let outcomes =
        apply_pending_defaults(&mut config, emulator_name, server_rom_id, &cores_on_disk);
    if !outcomes.is_empty() {
        if let Err(e) = config.save(config_path) {
            tracing::warn!("setup defaults were not saved: {e}");
            return Vec::new();
        }
    }
    outcomes
}

/// The cores compatible with a pending platform that are installed beside
/// `entry` — the same resolution the automatic assignment and launch use.
fn cores_on_disk(
    pending: &grid_core::config::PendingDefault,
    entry: &grid_core::config::EmulatorEntry,
) -> Vec<String> {
    installed_compatible_cores(&pending.name, &pending.slug, entry)
}

#[cfg(test)]
mod tests {
    use super::setup_needed;

    #[test]
    fn a_fresh_install_needs_setup() {
        assert!(setup_needed(false, false, false, false, false));
    }

    #[test]
    fn finished_or_skipped_setup_is_not_offered_again() {
        assert!(!setup_needed(true, false, false, false, false));
    }

    #[test]
    fn an_upgrade_with_an_emulator_or_a_game_never_sees_it() {
        assert!(!setup_needed(false, true, false, false, false));
        assert!(!setup_needed(false, false, true, false, false));
    }

    #[test]
    fn an_e2e_build_shows_it_only_to_the_stage_that_opts_in() {
        assert!(!setup_needed(false, false, false, true, false));
        assert!(setup_needed(false, false, false, true, true));
    }
}
