//! App-layer firmware wiring: the three background firmware triggers and the
//! one-job-per-emulator-directory guard they share.
//!
//! grid-core owns every rule about *what* firmware goes where
//! (`firmware::routing`, `firmware::install_platform_firmware`,
//! `firmware::rpcs3`); this module owns *when* those run and the Tauri
//! plumbing they need — a live `RommClient` out of `SessionManager`, the
//! drawer row for the PS3 PUP transfer, and `tauri::async_runtime::spawn`.
//! Nothing here is reachable from grid-core, which never imports Tauri.
//!
//! The three triggers, and their reference call sites:
//!
//! - **after a game install finalizes** — [`FirmwareService::spawn_for_game`],
//!   hung off `InstallService::set_game_finalized_hook`
//!   (`_install_firmware_for_game_without_ui`, install_mixin.py:528-697);
//! - **after a fresh emulator install** — [`FirmwareService::spawn_for_emulator`],
//!   hung off `InstallService::set_emulator_installed_hook`
//!   (emulator_ui_mixin.py:1865-1912);
//! - **after an RPCS3 entry is added by hand** —
//!   [`FirmwareService::spawn_ps3_firmware`], called from
//!   `commands::save_emulator` and from `spawn_for_emulator` when the
//!   installed emulator is RPCS3 (emulator_ui_mixin.py:1747-1795).
//!
//! Concurrency: at most one firmware job per emulator directory at a time
//! ([`FirmwareService::try_begin`]), released by a drop guard so a panic in
//! the spawned task cannot wedge that directory forever. Two *different*
//! emulators may install firmware at the same time.
//!
//! Token secrecy: the only text this module logs is a platform name and
//! grid-core's own warning strings, which name local paths and platform ids
//! and never a URL, header, or token. Every HTTP call goes through
//! `RommClient`, which redacts.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use grid_core::config::{Config, EmulatorEntry};
use grid_core::firmware::routing::{
    emulator_dir_of, install_for_game, platform_ids_for_profile, targets_for_entry,
    GameFirmwareContext,
};
use grid_core::firmware::rpcs3::{ps3_platform_id, rpcs3_pup_path};
use grid_core::firmware::{install_platform_firmware, FirmwareOptions};
use grid_core::launch::profiles::{load_profiles, profile_for_entry};
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::session::SessionManager;

/// The drawer row title for a background PS3 firmware transfer. Verbatim
/// from the reference (emulator_ui_mixin.py:1760) — the frontend matches on
/// it, so it must not be reworded.
pub const PS3_FIRMWARE_TITLE: &str = "PS3 Firmware";
/// The drawer row platform for that transfer. Verbatim, same reason.
pub const PS3_FIRMWARE_PLATFORM: &str = "PlayStation 3";

/// Serializes background firmware jobs per emulator directory.
pub struct FirmwareService {
    in_flight: StdMutex<HashSet<PathBuf>>,
}

impl FirmwareService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            in_flight: StdMutex::new(HashSet::new()),
        })
    }

    /// Claims `dir` for one firmware job. `false` when a job already holds
    /// it — the caller must then do nothing at all, not even spawn.
    ///
    /// A poisoned lock is recovered rather than propagated: the guarded set
    /// is a plain `HashSet` of paths, so a panicking holder leaves nothing
    /// inconsistent behind, and refusing every later firmware job would be
    /// worse than continuing.
    pub fn try_begin(&self, dir: &Path) -> bool {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(dir.to_path_buf())
    }

    /// Releases a [`Self::try_begin`] claim.
    pub fn end(&self, dir: &Path) {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(dir);
    }

    /// Installs the platform firmware a just-finalized game needs, in the
    /// background (`_install_firmware_for_game_without_ui`,
    /// install_mixin.py:528-697).
    ///
    /// Returns immediately, and silently, whenever there is nothing to do:
    /// no connected session, no platform id for the row's platform (the
    /// guard `install_for_game` itself does NOT carry — it takes an
    /// `i64`), no default emulator for that platform, no config entry by
    /// that name, or a firmware job already running for that emulator
    /// directory. Never fails and never blocks the caller.
    pub fn spawn_for_game(
        self: &Arc<Self>,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
        record: InstalledGame,
    ) {
        let Some(client) = session.client() else {
            return;
        };
        let Some(platform_id) = platform_id_for(&install.platform_ids(), &record.platform) else {
            return;
        };
        let config_path = Config::default_path();
        let Ok(config) = Config::load(&config_path) else {
            return;
        };
        let profiles = load_profiles();
        let Some(entry) = default_entry_for_platform(&config, &record.platform, profiles) else {
            return;
        };
        let dir = emulator_dir_of(entry);
        if !self.try_begin(&dir) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir);
        let config_dir = config_dir_of(&config_path);
        let platform = record.platform.clone();
        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            let ctx = GameFirmwareContext {
                platform: &platform,
                platform_id,
                config: &config,
                profiles,
                config_dir: &config_dir,
            };
            let warnings = install_for_game(&client, &ctx).await;
            if !warnings.is_empty() {
                // D14: warnings are logged, never surfaced as a dialog —
                // the install already succeeded. Local paths and platform
                // ids only; grid-core builds no URL into them.
                tracing::warn!("firmware for {platform}: {warnings}");
            }
        });
    }

    /// Installs the platform firmware a freshly installed emulator wants,
    /// across every platform its profile claims
    /// (emulator_ui_mixin.py:1865-1912).
    ///
    /// RPCS3 routes to [`Self::spawn_ps3_firmware`] instead — its firmware
    /// is a single PUP the emulator itself installs, not per-platform
    /// server firmware. Returns immediately and silently when there is
    /// nothing to do.
    pub fn spawn_for_emulator(
        self: &Arc<Self>,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
        name: String,
    ) {
        let Some(client) = session.client() else {
            return;
        };
        let config_path = Config::default_path();
        let Ok(config) = Config::load(&config_path) else {
            return;
        };
        let profiles = load_profiles();
        let Some(entry) =
            grid_core::launch::selection::emulator_entry_by_name(&config.emulators, &name)
        else {
            return;
        };
        let Some(profile) = profile_for_entry(&entry.name, &entry.path, profiles) else {
            return;
        };
        if grid_core::autoconfig::is_rpcs3(entry, profiles) {
            let entry = entry.clone();
            self.spawn_ps3_firmware(session, install, entry);
            return;
        }
        let config_dir = config_dir_of(&config_path);
        let targets = targets_for_entry(entry, Some(profile), &config.library_path, &config_dir);
        if targets.is_empty() {
            return;
        }
        let ids = platform_ids_for_profile(profile, entry, &install.platform_ids(), profiles);
        if ids.is_empty() {
            return;
        }
        let dir = emulator_dir_of(entry);
        if !self.try_begin(&dir) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir);
        let label = entry.name.clone();
        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            let mut warnings = Vec::new();
            for id in ids {
                warnings.extend(
                    install_platform_firmware(&client, id, &targets, FirmwareOptions::default())
                        .await,
                );
            }
            if !warnings.is_empty() {
                tracing::warn!("firmware for {label}: {}", warnings.join("\n"));
            }
        });
    }

    /// Downloads the PS3 firmware PUP into an RPCS3 install that has none
    /// (emulator_ui_mixin.py:1747-1795).
    ///
    /// Unlike the other two triggers this one is user-visible: the transfer
    /// gets its own drawer row through
    /// [`InstallService::admit_external`]/`complete_external`, because the
    /// PUP is large and nothing else in the queue accounts for it.
    ///
    /// Does nothing when a PUP is already installed beside the executable,
    /// when the profile routes no firmware directory, when the server's
    /// platform map carries no PS3 platform (D17), when no session is
    /// connected, or when that emulator directory already has a firmware
    /// job running.
    pub fn spawn_ps3_firmware(
        self: &Arc<Self>,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
        entry: EmulatorEntry,
    ) {
        let Some(client) = session.client() else {
            return;
        };
        if rpcs3_pup_path(&entry.path).is_some() {
            return;
        }
        let config_path = Config::default_path();
        let Ok(config) = Config::load(&config_path) else {
            return;
        };
        let profiles = load_profiles();
        let Some(profile) = profile_for_entry(&entry.name, &entry.path, profiles) else {
            return;
        };
        let config_dir = config_dir_of(&config_path);
        let targets = targets_for_entry(&entry, Some(profile), &config.library_path, &config_dir);
        if targets.is_empty() {
            return;
        }
        // D17: no PS3 platform on this server means there is no firmware to
        // fetch — admit no drawer row rather than one that instantly fails.
        let Some(platform_id) = ps3_platform_id(&install.platform_ids()) else {
            return;
        };
        let dir = emulator_dir_of(&entry);
        if !self.try_begin(&dir) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir);
        let row_id = install.admit_external(PS3_FIRMWARE_TITLE, PS3_FIRMWARE_PLATFORM);
        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            let warnings = install_platform_firmware(
                &client,
                platform_id,
                &targets,
                FirmwareOptions::default(),
            )
            .await;
            install.complete_external(row_id, first_warning(&warnings));
        });
    }
}

/// A drawer row completes on a blank error and fails on anything else, so
/// only the FIRST warning is reported: the rest would be truncated in the
/// row's one error line anyway.
fn first_warning(warnings: &[String]) -> &str {
    warnings.first().map(String::as_str).unwrap_or("")
}

/// Releases a [`FirmwareService::try_begin`] claim when dropped — on the
/// normal completion path and on a panic unwinding out of the spawned task
/// alike. Without it a panicking job would block that emulator directory
/// for the life of the process.
struct FirmwareGuard {
    service: Arc<FirmwareService>,
    dir: PathBuf,
}

impl FirmwareGuard {
    fn new(service: Arc<FirmwareService>, dir: PathBuf) -> Self {
        Self { service, dir }
    }
}

impl Drop for FirmwareGuard {
    fn drop(&mut self) {
        self.service.end(&self.dir);
    }
}

/// The server platform id for `platform`: an exact key match first, then a
/// case-insensitive one. grid-core stores the map exactly as the server
/// spelled it, while a registry row's `platform` is whatever was recorded at
/// install time, so the two can differ only in case.
fn platform_id_for(ids: &std::collections::BTreeMap<String, i64>, platform: &str) -> Option<i64> {
    let name = platform.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(id) = ids.get(name) {
        return Some(*id);
    }
    let folded = name.to_lowercase();
    ids.iter()
        .find(|(key, _)| key.trim().to_lowercase() == folded)
        .map(|(_, id)| *id)
}

/// The default emulator entry for `platform`, or `None` when none is
/// configured or the configured name matches no entry.
fn default_entry_for_platform<'a>(
    config: &'a Config,
    platform: &str,
    profiles: &[grid_core::launch::profiles::EmulatorProfile],
) -> Option<&'a EmulatorEntry> {
    let name = grid_core::launch::selection::default_emulator_name_for_platform(
        &config.emulators,
        &config.default_emulators,
        platform,
        profiles,
        &config.retroarch_cores,
    );
    grid_core::launch::selection::emulator_entry_by_name(&config.emulators, &name)
}

/// The `%CONFIG_DIR%` token's value: the directory holding `config.json`.
fn config_dir_of(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_firmware_job_per_directory() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/rpcs3");
        assert!(svc.try_begin(&dir));
        assert!(!svc.try_begin(&dir));
        svc.end(&dir);
        assert!(svc.try_begin(&dir));
    }

    #[test]
    fn different_directories_do_not_block_each_other() {
        let svc = FirmwareService::new();
        assert!(svc.try_begin(Path::new("/emulators/rpcs3")));
        assert!(svc.try_begin(Path::new("/emulators/duckstation")));
    }

    /// Proves the drop guard itself releases the claim — the mechanism every
    /// `spawn_*` relies on to survive a panic unwinding out of its task.
    #[test]
    fn guard_releases_on_drop() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/rpcs3");
        assert!(svc.try_begin(&dir));
        let guard = FirmwareGuard::new(svc.clone(), dir.clone());
        drop(guard);
        assert!(svc.try_begin(&dir));
    }

    #[test]
    fn platform_id_lookup_is_exact_then_case_insensitive() {
        let mut ids = std::collections::BTreeMap::new();
        ids.insert("PlayStation 3".to_string(), 7);
        assert_eq!(platform_id_for(&ids, "PlayStation 3"), Some(7));
        assert_eq!(platform_id_for(&ids, "playstation 3"), Some(7));
        assert_eq!(platform_id_for(&ids, "  PLAYSTATION 3 "), Some(7));
        assert_eq!(platform_id_for(&ids, "Xbox 360"), None);
        assert_eq!(platform_id_for(&ids, "   "), None);
    }

    #[test]
    fn only_the_first_warning_reaches_the_drawer_row() {
        assert_eq!(first_warning(&[]), "");
        assert_eq!(
            first_warning(&["first".to_string(), "second".to_string()]),
            "first"
        );
    }
}
