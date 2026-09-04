//! App-layer firmware wiring: the three background firmware triggers and the
//! one-job-per-pass guard they share.
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
//! Concurrency ([`FirmwareService::try_begin`], released by a drop guard so
//! a panic in the spawned task cannot wedge anything forever): a job is
//! claimed by a [`PassKey`] — an emulator directory plus, for a per-game
//! pass, the platform it is fetching for. A whole-directory pass
//! ([`FirmwareService::spawn_for_emulator`], [`FirmwareService::spawn_ps3_firmware`])
//! claims the directory itself, which also blocks every per-game pass for
//! that directory — and, in the other direction, cannot start while any
//! per-game pass for that directory is still running; two per-game passes
//! for *different* platforms in the
//! same directory may run at the same time, because one emulator can serve
//! many platforms (RetroArch serves all of them from one directory). Two
//! different emulators never block each other at all.
//!
//! Repetition (D19, amended by the final review): `install_platform_firmware`
//! downloads every firmware record BEFORE its `skip_existing` write check,
//! so a per-game pass that ran once already costs the full BIOS set again on
//! the next launch of the same platform. [`FirmwareService`] therefore
//! remembers which `(emulator directory, platform id)` pairs have completed
//! a per-game pass in this process and skips a [`FirmwareTrigger::Launch`]
//! pass for those. Keying by directory alone was wrong: with RetroArch the
//! first completed pass would suppress every later platform's.
//! [`FirmwareTrigger::Install`] always runs: a fresh install is exactly the
//! moment the answer can have changed.
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

/// Why a per-game firmware pass is being requested — the D19 gate's only
/// input. See the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareTrigger {
    /// A game install just finalized. Always runs: this is the moment the
    /// firmware answer can have changed.
    Install,
    /// A game is about to launch. Runs at most once per
    /// `(emulator directory, platform id)` pair per process.
    Launch,
}

/// The text a [`RowGuard`] fails its drawer row with when the task owning it
/// unwound instead of reporting an outcome. Never a normal-path value.
const ABORTED_ROW_ERROR: &str = "firmware job aborted";

/// What one firmware pass is claimed and remembered by: the emulator
/// directory, plus the platform id for a per-game pass ([`None`] for a pass
/// that covers the whole directory — [`FirmwareService::spawn_for_emulator`]
/// walks every platform the profile claims, and
/// [`FirmwareService::spawn_ps3_firmware`] is the directory's one PS3 PUP).
type PassKey = (PathBuf, Option<i64>);

/// Builds a [`PassKey`] without cloning the caller's path twice.
fn pass_key(dir: &Path, platform_id: Option<i64>) -> PassKey {
    (dir.to_path_buf(), platform_id)
}

/// Serializes background firmware jobs per [`PassKey`], and gates repeat
/// per-game passes (D19).
pub struct FirmwareService {
    in_flight: StdMutex<HashSet<PassKey>>,
    /// The `(emulator directory, platform id)` pairs whose per-game pass has
    /// run to completion in this process. Never persisted: a restart is a
    /// deliberate re-check.
    completed: StdMutex<HashSet<PassKey>>,
}

impl FirmwareService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            in_flight: StdMutex::new(HashSet::new()),
            completed: StdMutex::new(HashSet::new()),
        })
    }

    /// The D19 gate: whether a per-game pass for `platform_id` under `dir`
    /// should run at all. Checked BEFORE [`Self::try_begin`], so a skipped
    /// launch pass never touches the in-flight set.
    ///
    /// The key carries the platform because one emulator directory can
    /// serve many platforms: completing PlayStation's pass must not suppress
    /// Nintendo 64's in the same RetroArch directory.
    ///
    /// A poisoned lock is recovered rather than propagated, same rule as
    /// [`Self::try_begin`]: the guarded set is plain paths and ids, and
    /// refusing every later pass would be worse than continuing.
    pub fn should_run(&self, dir: &Path, platform_id: i64, trigger: FirmwareTrigger) -> bool {
        match trigger {
            FirmwareTrigger::Install => true,
            FirmwareTrigger::Launch => !self
                .completed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(&pass_key(dir, Some(platform_id))),
        }
    }

    /// Records that a per-game pass for `platform_id` under `dir` finished.
    /// Called on the task's normal path only — a panicking pass did not
    /// complete, so the next launch is allowed to retry it.
    pub fn mark_completed(&self, dir: &Path, platform_id: i64) {
        self.completed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(pass_key(dir, Some(platform_id)));
    }

    /// Claims one firmware job. `false` when a job already holds the claim —
    /// the caller must then do nothing at all, not even spawn.
    ///
    /// `platform_id` is `Some` for a per-game pass and `None` for a pass
    /// that covers the whole directory. Because a whole-directory pass
    /// covers every platform there, the two kinds exclude each other in
    /// BOTH directions: a per-game pass is refused while a whole-directory
    /// pass for the same directory is in flight, and a whole-directory pass
    /// is refused while ANY pass for that directory is in flight. Without
    /// the second half a fresh-emulator pass could start on top of a
    /// per-game pass already writing into the same firmware directories.
    ///
    /// A poisoned lock is recovered rather than propagated: the guarded set
    /// is a plain `HashSet`, so a panicking holder leaves nothing
    /// inconsistent behind, and refusing every later firmware job would be
    /// worse than continuing.
    pub fn try_begin(&self, dir: &Path, platform_id: Option<i64>) -> bool {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        match platform_id {
            None if in_flight.iter().any(|(key, _)| key.as_path() == dir) => false,
            Some(_) if in_flight.contains(&pass_key(dir, None)) => false,
            _ => in_flight.insert(pass_key(dir, platform_id)),
        }
    }

    /// Releases a [`Self::try_begin`] claim.
    pub fn end(&self, dir: &Path, platform_id: Option<i64>) {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&pass_key(dir, platform_id));
    }

    /// Installs the platform firmware a just-finalized game needs, in the
    /// background (`_install_firmware_for_game_without_ui`,
    /// install_mixin.py:528-697).
    ///
    /// Returns immediately, and silently, whenever there is nothing to do:
    /// no connected session, no platform id for the row's platform (the
    /// guard `install_for_game` itself does NOT carry — it takes an
    /// `i64`), no default emulator for that platform, no config entry by
    /// that name, a [`FirmwareTrigger::Launch`] pass for a
    /// `(directory, platform)` pair that already completed one (D19), a pass
    /// for that same pair already running, or a whole-directory pass running
    /// for that emulator. Never fails and never blocks the caller.
    pub fn spawn_for_game(
        self: &Arc<Self>,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
        record: InstalledGame,
        trigger: FirmwareTrigger,
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
        // D19, before the in-flight claim: a launch pass for a directory
        // that already completed one this process is a pure re-download.
        if !self.should_run(&dir, platform_id, trigger) {
            return;
        }
        if !self.try_begin(&dir, Some(platform_id)) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir.clone(), Some(platform_id));
        let service = self.clone();
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
            // Completion means "the pass finished", warnings included: a
            // warning is a per-file outcome, and re-downloading the whole
            // set on the next launch would not change it.
            service.mark_completed(&dir, platform_id);
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
        // A whole-directory claim: this pass walks every platform the
        // profile claims, so no per-game pass for the same directory may
        // start alongside it — nor may this one start beside a per-game
        // pass that is already running there.
        if !self.try_begin(&dir, None) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir, None);
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
    /// connected, or when that emulator directory already has any firmware
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
        // The PS3 PUP is the directory's single firmware item, so this too
        // claims the whole directory rather than one platform of it — which
        // also means it will not start while any pass for that directory is
        // still running.
        if !self.try_begin(&dir, None) {
            return;
        }
        let guard = FirmwareGuard::new(self.clone(), dir, None);
        let row_id = install.admit_external(PS3_FIRMWARE_TITLE, PS3_FIRMWARE_PLATFORM);
        // The drawer row is admitted BEFORE the task exists, so it must be
        // closed by a drop guard too: a panic unwinding out of the task
        // would otherwise release the directory but leave the row stuck in
        // `downloading` for the life of the process.
        let mut row = RowGuard::new(install, row_id);
        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            let warnings = install_platform_firmware(
                &client,
                platform_id,
                &targets,
                FirmwareOptions::default(),
            )
            .await;
            row.complete(first_warning(&warnings));
        });
    }
}

/// A drawer row completes on a blank error and fails on anything else, so
/// only the FIRST warning is reported: the rest would be truncated in the
/// row's one error line anyway.
fn first_warning(warnings: &[String]) -> &str {
    warnings.first().map(String::as_str).unwrap_or("")
}

/// Closes an [`InstallService::admit_external`] drawer row exactly once.
///
/// The row is created before the task that fills it, so nothing else can
/// close it if that task unwinds. [`Self::complete`] reports the real
/// outcome and disarms the guard; a drop with the guard still armed fails
/// the row with [`ABORTED_ROW_ERROR`] rather than leaving it `downloading`
/// forever.
struct RowGuard {
    install: Arc<InstallService>,
    id: u64,
    /// `Some` while the row is still open: the text `Drop` would fail it
    /// with. `None` once [`Self::complete`] has reported the real outcome,
    /// which makes the drop a no-op.
    error: Option<String>,
}

impl RowGuard {
    fn new(install: Arc<InstallService>, id: u64) -> Self {
        Self {
            install,
            id,
            error: Some(ABORTED_ROW_ERROR.to_string()),
        }
    }

    /// Reports the real outcome (blank completes the row, anything else
    /// fails it) and disarms the guard.
    fn complete(&mut self, error: &str) {
        self.error = None;
        self.install.complete_external(self.id, error);
    }
}

impl Drop for RowGuard {
    fn drop(&mut self) {
        if let Some(error) = self.error.take() {
            self.install.complete_external(self.id, &error);
        }
    }
}

/// Releases a [`FirmwareService::try_begin`] claim when dropped — on the
/// normal completion path and on a panic unwinding out of the spawned task
/// alike. Without it a panicking job would block that pass key for the life
/// of the process.
struct FirmwareGuard {
    service: Arc<FirmwareService>,
    dir: PathBuf,
    platform_id: Option<i64>,
}

impl FirmwareGuard {
    fn new(service: Arc<FirmwareService>, dir: PathBuf, platform_id: Option<i64>) -> Self {
        Self {
            service,
            dir,
            platform_id,
        }
    }
}

impl Drop for FirmwareGuard {
    fn drop(&mut self) {
        self.service.end(&self.dir, self.platform_id);
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
        &grid_core::launch::selection::installed_core_resolver,
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
    fn one_whole_directory_job_per_directory() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/rpcs3");
        assert!(svc.try_begin(&dir, None));
        assert!(!svc.try_begin(&dir, None));
        svc.end(&dir, None);
        assert!(svc.try_begin(&dir, None));
    }

    #[test]
    fn one_per_game_job_per_directory_and_platform() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/retroarch");
        assert!(svc.try_begin(&dir, Some(7)));
        assert!(!svc.try_begin(&dir, Some(7)));
        svc.end(&dir, Some(7));
        assert!(svc.try_begin(&dir, Some(7)));
    }

    /// One emulator directory can serve many platforms (RetroArch serves
    /// every platform from one directory), so two per-game passes for
    /// different platforms must both be allowed to start.
    #[test]
    fn two_platforms_in_one_directory_both_run() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/retroarch");
        assert!(svc.try_begin(&dir, Some(7)));
        assert!(svc.try_begin(&dir, Some(19)));
    }

    /// A whole-directory pass covers every platform there, so no per-game
    /// pass for that directory may run beside it.
    #[test]
    fn a_directory_wide_pass_blocks_a_per_game_pass() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/retroarch");
        assert!(svc.try_begin(&dir, None));
        assert!(!svc.try_begin(&dir, Some(7)));
        // ...and a different directory is unaffected.
        assert!(svc.try_begin(Path::new("/emulators/duckstation"), Some(7)));
        // Releasing the directory-wide claim unblocks the per-game pass.
        svc.end(&dir, None);
        assert!(svc.try_begin(&dir, Some(7)));
    }

    /// The other direction: a whole-directory pass writes into the same
    /// firmware directories a per-game pass is already using, so it must not
    /// start beside one.
    #[test]
    fn a_per_game_pass_blocks_a_directory_wide_pass() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/retroarch");
        assert!(svc.try_begin(&dir, Some(7)));
        assert!(!svc.try_begin(&dir, None));
        // ...and a different directory is unaffected.
        assert!(svc.try_begin(Path::new("/emulators/duckstation"), None));
        // Releasing the last per-game claim unblocks the directory-wide one.
        svc.end(&dir, Some(7));
        assert!(svc.try_begin(&dir, None));
    }

    /// Every per-game claim for the directory has to be gone, not just one.
    #[test]
    fn a_directory_wide_pass_waits_for_every_per_game_claim() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/retroarch");
        assert!(svc.try_begin(&dir, Some(7)));
        assert!(svc.try_begin(&dir, Some(19)));
        svc.end(&dir, Some(7));
        assert!(!svc.try_begin(&dir, None));
        svc.end(&dir, Some(19));
        assert!(svc.try_begin(&dir, None));
    }

    #[test]
    fn different_directories_do_not_block_each_other() {
        let svc = FirmwareService::new();
        assert!(svc.try_begin(Path::new("/emulators/rpcs3"), None));
        assert!(svc.try_begin(Path::new("/emulators/duckstation"), None));
    }

    /// Proves the drop guard itself releases the claim — the mechanism every
    /// `spawn_*` relies on to survive a panic unwinding out of its task.
    #[test]
    fn guard_releases_on_drop() {
        let svc = FirmwareService::new();
        let dir = PathBuf::from("/emulators/rpcs3");
        assert!(svc.try_begin(&dir, None));
        drop(FirmwareGuard::new(svc.clone(), dir.clone(), None));
        assert!(svc.try_begin(&dir, None));
        svc.end(&dir, None);

        assert!(svc.try_begin(&dir, Some(7)));
        drop(FirmwareGuard::new(svc.clone(), dir.clone(), Some(7)));
        assert!(svc.try_begin(&dir, Some(7)));
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

    /// D19: the first launch pass for a `(directory, platform)` pair runs;
    /// the second is skipped once the first completed.
    #[test]
    fn a_launch_pass_runs_once_per_directory_and_platform() {
        let svc = FirmwareService::new();
        let dir = Path::new("/emulators/duckstation");
        assert!(svc.should_run(dir, 7, FirmwareTrigger::Launch));
        svc.mark_completed(dir, 7);
        assert!(!svc.should_run(dir, 7, FirmwareTrigger::Launch));
    }

    /// D19: the gate is per directory — completing one emulator's pass
    /// never suppresses another's.
    #[test]
    fn the_launch_gate_is_per_directory() {
        let svc = FirmwareService::new();
        svc.mark_completed(Path::new("/emulators/duckstation"), 7);
        assert!(svc.should_run(Path::new("/emulators/xemu"), 7, FirmwareTrigger::Launch));
    }

    /// D19 as amended: the gate is per platform too. RetroArch serves every
    /// platform out of one directory, so a completed PlayStation pass must
    /// not suppress Nintendo 64's on the next launch.
    #[test]
    fn the_launch_gate_is_per_platform_within_one_directory() {
        let svc = FirmwareService::new();
        let dir = Path::new("/emulators/retroarch");
        svc.mark_completed(dir, 7);
        assert!(!svc.should_run(dir, 7, FirmwareTrigger::Launch));
        assert!(svc.should_run(dir, 19, FirmwareTrigger::Launch));
    }

    /// D19: an install trigger is never gated — a fresh install is exactly
    /// the moment the firmware answer can have changed.
    #[test]
    fn an_install_pass_runs_even_after_completion() {
        let svc = FirmwareService::new();
        let dir = Path::new("/emulators/duckstation");
        assert!(svc.should_run(dir, 7, FirmwareTrigger::Install));
        svc.mark_completed(dir, 7);
        assert!(svc.should_run(dir, 7, FirmwareTrigger::Install));
        // ...and the install pass's own completion still does not unblock
        // the launch gate in the other direction.
        assert!(!svc.should_run(dir, 7, FirmwareTrigger::Launch));
    }

    /// A second Install-trigger pass for a *different* platform in the same
    /// directory is not dropped by the in-flight claim while the first one
    /// runs — the case that made an Install pass for one game silently skip
    /// while another game's pass was still going.
    #[test]
    fn a_second_install_pass_for_another_platform_is_not_dropped() {
        let svc = FirmwareService::new();
        let dir = Path::new("/emulators/retroarch");
        assert!(svc.should_run(dir, 7, FirmwareTrigger::Install));
        assert!(svc.try_begin(dir, Some(7)));
        assert!(svc.should_run(dir, 19, FirmwareTrigger::Install));
        assert!(svc.try_begin(dir, Some(19)));
    }

    /// The launch gate and the in-flight claim are independent sets: a
    /// completed pass must not leave the pass key claimed.
    #[test]
    fn completing_a_pass_does_not_hold_the_claim() {
        let svc = FirmwareService::new();
        let dir = Path::new("/emulators/duckstation");
        assert!(svc.try_begin(dir, Some(7)));
        svc.mark_completed(dir, 7);
        svc.end(dir, Some(7));
        assert!(svc.try_begin(dir, Some(7)));
    }

    /// Builds a real `InstallService` over a temp registry, so the two
    /// `RowGuard` paths are exercised against the actual queue rather than
    /// a stand-in.
    fn install_service(dir: &tempfile::TempDir) -> Arc<InstallService> {
        let registry = Arc::new(
            grid_core::library::registry::Registry::open(&dir.path().join("registry.db")).unwrap(),
        );
        InstallService::new(registry, dir.path().join("config.json"))
    }

    fn row_status(install: &InstallService, id: u64) -> (String, String) {
        let snapshot = install.snapshot();
        let entry = snapshot
            .entries
            .iter()
            .find(|e| e.id == id)
            .expect("the admitted row is in the snapshot");
        (format!("{:?}", entry.status), entry.error.clone())
    }

    #[test]
    fn a_completed_row_guard_reports_the_real_outcome_and_does_not_fire_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let install = install_service(&dir);
        let id = install.admit_external(PS3_FIRMWARE_TITLE, PS3_FIRMWARE_PLATFORM);
        {
            let mut row = RowGuard::new(install.clone(), id);
            row.complete("");
            assert_eq!(
                row_status(&install, id),
                ("Completed".into(), String::new())
            );
        }
        // The drop above must have been a no-op — a re-fail here would show
        // up as a changed status.
        assert_eq!(
            row_status(&install, id),
            ("Completed".into(), String::new())
        );
    }

    #[test]
    fn an_abandoned_row_guard_fails_the_row_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let install = install_service(&dir);
        let id = install.admit_external(PS3_FIRMWARE_TITLE, PS3_FIRMWARE_PLATFORM);
        drop(RowGuard::new(install.clone(), id));
        assert_eq!(
            row_status(&install, id),
            ("Failed".into(), ABORTED_ROW_ERROR.to_string())
        );
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
