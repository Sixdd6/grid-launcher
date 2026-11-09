//! App-layer glue for backup drives: WHEN drives are scanned and what
//! follows. grid-core owns every rule about drives and packs
//! (`grid_core::backup`); this module owns the 2 s poll loop, the
//! `drives-changed` event, the remembered-drive list in config and the
//! firmware a drive install waits to put in place.
//!
//! Each tick scans on the blocking pool and reads each drive's
//! `gridpack.json` (cached by size and modified time, so an unchanged index
//! is not parsed again). Against the previous scan:
//! - a drive that appeared is connected again in `InstallService`, and its
//!   leftover `*.gridtmp` files are removed, unless a live row targets it;
//! - a drive that disappeared aborts every row for it ("<label> was
//!   removed").
//!
//! The remembered list in config is written only when it changes, and the
//! event is emitted only when the snapshot changes. Logs carry drive paths
//! and labels, never a server URL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use grid_core::backup::drives::{self, DetectedDrive};
use grid_core::backup::pack::{self, Pack, PackFirmware, PackOs, PackState};
use grid_core::config::{Config, RememberedDrive};
use grid_core::library::backup_job::DriveTarget;
use grid_core::library::InstallService;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Emitted with a [`DrivesSnapshot`] whenever the snapshot changes.
pub const DRIVES_CHANGED_EVENT: &str = "drives-changed";

/// Emitted with a [`BackupNotice`] for a toast the backend decides on
/// (firmware installed from a drive, after the emulator it waited for).
pub const BACKUP_NOTICE_EVENT: &str = "backup-notice";

/// How often drives are scanned.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// A toast: its text and whether it reports a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupNotice {
    pub message: String,
    pub error: bool,
}

/// What a connected drive is to GRID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveStatus {
    /// Set up and writable.
    Ready,
    /// No `gridpack/gridpack.json`.
    NotSetUp,
    /// Mounted read-only: games install from it, nothing is written.
    ReadOnly,
    /// A `gridpack.json` this build can't read.
    Unreadable,
    /// Written by a newer GRID: read-only.
    NewerFormat,
}

/// One emulator release on a drive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackEmulatorSummary {
    pub name: String,
    pub version: String,
    /// `None` for an OS tag this build does not know.
    pub os: Option<PackOs>,
}

/// One backed-up game's identity, for "On <pack>" in Details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackRomRef {
    pub server: String,
    pub rom_id: i64,
}

/// What a set-up drive holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackSummary {
    pub name: String,
    pub os: Vec<PackOs>,
    pub game_count: u32,
    pub gridpack_bytes: u64,
    pub emulators: Vec<PackEmulatorSummary>,
    /// Platform names.
    pub firmware: Vec<String>,
    pub updated: String,
    pub rom_ids: Vec<PackRomRef>,
}

/// A connected drive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriveInfo {
    /// The mount path, or the drive letter on Windows. Every command takes it.
    pub key: String,
    pub mount: String,
    pub label: String,
    /// The display file system name (`FAT32`, `exFAT`, `NTFS`…).
    pub fs: String,
    pub fat32: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub read_only: bool,
    pub status: DriveStatus,
    /// Why the index can't be read (`unreadable`), else empty.
    pub problem: String,
    pub pack: Option<PackSummary>,
}

/// Every connected drive, and every remembered drive that is not connected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DrivesSnapshot {
    pub connected: Vec<DriveInfo>,
    pub remembered: Vec<RememberedDrive>,
}

/// One drive of a scan and what reading its index found (`Err` is an I/O
/// error reading it).
#[derive(Debug, Clone)]
pub struct ScannedDrive {
    pub drive: DetectedDrive,
    pub pack: Result<PackState, String>,
}

impl ScannedDrive {
    pub fn key(&self) -> String {
        self.drive.key()
    }

    /// The pack, when the drive is set up and readable.
    pub fn ready_pack(&self) -> Option<&Pack> {
        match &self.pack {
            Ok(PackState::Ready(pack)) => Some(pack),
            _ => None,
        }
    }

    /// The target a job for this drive records.
    pub fn target(&self) -> DriveTarget {
        DriveTarget {
            key: self.key(),
            mount: self.drive.mount.clone(),
            label: self.drive.label.clone(),
            name: self
                .ready_pack()
                .map(|pack| pack.name.clone())
                .unwrap_or_default(),
            fat32: self.drive.fat32,
        }
    }
}

/// The status of a drive. Pure. A newer or unreadable index wins over the
/// mount's read-only flag, which wins over set up or not.
pub fn classify(read_only: bool, pack: &Result<PackState, String>) -> DriveStatus {
    match pack {
        Ok(PackState::NewerFormat(_)) => DriveStatus::NewerFormat,
        Ok(PackState::Unreadable(_)) | Err(_) => DriveStatus::Unreadable,
        _ if read_only => DriveStatus::ReadOnly,
        Ok(PackState::Absent) => DriveStatus::NotSetUp,
        Ok(PackState::Ready(_)) => DriveStatus::Ready,
    }
}

fn os_of_tag(tag: &str) -> Option<PackOs> {
    [PackOs::Linux, PackOs::Windows]
        .into_iter()
        .find(|os| os.arch_tag() == tag)
}

/// The summary of a pack. Pure.
pub fn pack_summary(pack: &Pack) -> PackSummary {
    PackSummary {
        name: pack.name.clone(),
        os: pack.os.clone(),
        game_count: pack.games.len() as u32,
        gridpack_bytes: pack.pack_bytes(),
        emulators: pack
            .emulators
            .iter()
            .map(|emulator| PackEmulatorSummary {
                name: emulator.name.clone(),
                version: emulator.version.clone(),
                os: os_of_tag(&emulator.os),
            })
            .collect(),
        firmware: pack
            .firmware
            .iter()
            .filter(|set| !set.files.is_empty())
            .map(|set| set.platform_name.clone())
            .collect(),
        updated: pack.updated.clone(),
        rom_ids: pack
            .games
            .iter()
            .map(|game| PackRomRef {
                server: game.source.server.clone(),
                rom_id: game.source.rom_id,
            })
            .collect(),
    }
}

/// The [`DriveInfo`] of one scanned drive. Pure.
pub fn drive_info(scanned: &ScannedDrive) -> DriveInfo {
    let drive = &scanned.drive;
    let problem = match &scanned.pack {
        Ok(PackState::Unreadable(why)) => why.clone(),
        Err(why) => why.clone(),
        _ => String::new(),
    };
    DriveInfo {
        key: drive.key(),
        mount: drive.mount.to_string_lossy().into_owned(),
        label: drive.label.clone(),
        fs: drive.fs_display.clone(),
        fat32: drive.fat32,
        total_bytes: drive.total_bytes,
        free_bytes: drive.free_bytes,
        read_only: drive.read_only,
        status: classify(drive.read_only, &scanned.pack),
        problem,
        pack: scanned.ready_pack().map(pack_summary),
    }
}

/// What config remembers about a connected, set-up drive (`last_seen`
/// left blank). Pure.
pub fn remembered_of(drive: &DetectedDrive, pack: &Pack) -> RememberedDrive {
    RememberedDrive {
        mount: drive.key(),
        name: pack.name.clone(),
        label: drive.label.clone(),
        fs: drive.fs_display.clone(),
        total_bytes: drive.total_bytes,
        game_count: pack.games.len() as u32,
        last_seen: String::new(),
    }
}

/// Merges the connected, set-up drives into the remembered list. Pure.
/// A drive not remembered yet is added; a remembered one has its name,
/// label, file system, size and game count refreshed. `last_seen` becomes
/// `now` for a drive that is new, changed, or in `appeared` (it was just
/// connected), so a drive that stays plugged in does not rewrite config
/// every poll. Returns `None` when nothing changed.
pub fn merge_remembered(
    existing: &[RememberedDrive],
    connected: &[RememberedDrive],
    appeared: &[String],
    now: &str,
) -> Option<Vec<RememberedDrive>> {
    let mut merged = existing.to_vec();
    for fresh in connected {
        let stamp = |mut drive: RememberedDrive| {
            drive.last_seen = now.to_string();
            drive
        };
        match merged.iter_mut().find(|known| known.mount == fresh.mount) {
            None => merged.push(stamp(fresh.clone())),
            Some(known) => {
                let same = known.name == fresh.name
                    && known.label == fresh.label
                    && known.fs == fresh.fs
                    && known.total_bytes == fresh.total_bytes
                    && known.game_count == fresh.game_count;
                if !same || appeared.contains(&fresh.mount) {
                    *known = stamp(fresh.clone());
                }
            }
        }
    }
    (merged != existing).then_some(merged)
}

/// The snapshot of a scan: every connected drive, and the remembered drives
/// that are not connected. Pure.
pub fn snapshot_of(scanned: &[ScannedDrive], remembered: &[RememberedDrive]) -> DrivesSnapshot {
    let connected: Vec<DriveInfo> = scanned.iter().map(drive_info).collect();
    let remembered = remembered
        .iter()
        .filter(|drive| !connected.iter().any(|info| info.key == drive.mount))
        .cloned()
        .collect();
    DrivesSnapshot {
        connected,
        remembered,
    }
}

/// Firmware a drive install will put in place once the platform's emulator
/// (installed from the same drive) lands.
#[derive(Debug, Clone)]
pub struct PendingDriveFirmware {
    /// The platform label the game's registry row carries.
    pub platform: String,
    pub mount: PathBuf,
    pub pack_name: String,
    pub entry: PackFirmware,
}

type CachedPack = (SystemTime, u64, PackState);

pub struct DriveService {
    latest: Mutex<Vec<ScannedDrive>>,
    snapshot: Mutex<Option<DrivesSnapshot>>,
    remembered: Mutex<Option<Vec<RememberedDrive>>>,
    pack_cache: Mutex<HashMap<PathBuf, CachedPack>>,
    pending_firmware: Mutex<Vec<PendingDriveFirmware>>,
    /// One tick at a time: the poll loop and a command's rescan.
    tick_lock: tokio::sync::Mutex<()>,
}

impl DriveService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            latest: Mutex::new(Vec::new()),
            snapshot: Mutex::new(None),
            remembered: Mutex::new(None),
            pack_cache: Mutex::new(HashMap::new()),
            pending_firmware: Mutex::new(Vec::new()),
            tick_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// Starts the poll loop. Call once, from inside Tauri's async runtime.
    pub fn spawn_poll_loop(self: &Arc<Self>, app: AppHandle, install: Option<Arc<InstallService>>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                service.tick(&app, install.as_ref()).await;
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
    }

    /// The last snapshot, or a fresh scan when none has run yet.
    pub async fn current(
        self: &Arc<Self>,
        app: &AppHandle,
        install: Option<&Arc<InstallService>>,
    ) -> DrivesSnapshot {
        let known = self.snapshot.lock().unwrap().clone();
        match known {
            Some(snapshot) => snapshot,
            None => self.tick(app, install).await,
        }
    }

    /// The connected drive `key` in the latest scan.
    pub fn connected(&self, key: &str) -> Result<ScannedDrive, String> {
        self.latest
            .lock()
            .unwrap()
            .iter()
            .find(|scanned| scanned.key() == key)
            .cloned()
            .ok_or_else(|| "That drive is not connected".to_string())
    }

    /// Reads a drive's index, reusing the parsed copy while the file's size
    /// and modified time are unchanged.
    fn read_cached(&self, mount: &Path) -> Result<PackState, String> {
        let index = pack::index_path(mount);
        let stamp = std::fs::metadata(&index)
            .ok()
            .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
        if let Some((modified, len)) = stamp {
            if let Some((when, size, state)) = self.pack_cache.lock().unwrap().get(mount) {
                if *when == modified && *size == len {
                    return Ok(state.clone());
                }
            }
        }
        let state = pack::read_pack(mount).map_err(|e| e.to_string())?;
        let mut cache = self.pack_cache.lock().unwrap();
        match stamp {
            Some((modified, len)) => {
                cache.insert(mount.to_path_buf(), (modified, len, state.clone()));
            }
            None => {
                cache.remove(mount);
            }
        }
        Ok(state)
    }

    fn scan_blocking(&self) -> Vec<ScannedDrive> {
        drives::scan()
            .into_iter()
            .map(|drive| {
                let pack = self.read_cached(&drive.mount);
                ScannedDrive { drive, pack }
            })
            .collect()
    }

    /// One scan and everything that follows it. Returns the new snapshot.
    pub async fn tick(
        self: &Arc<Self>,
        app: &AppHandle,
        install: Option<&Arc<InstallService>>,
    ) -> DrivesSnapshot {
        let _tick = self.tick_lock.lock().await;
        let service = self.clone();
        let scanned = match tokio::task::spawn_blocking(move || service.scan_blocking()).await {
            Ok(scanned) => scanned,
            Err(e) => {
                tracing::warn!("drive scan did not finish: {e}");
                return self.snapshot.lock().unwrap().clone().unwrap_or_default();
            }
        };
        let previous = std::mem::replace(&mut *self.latest.lock().unwrap(), scanned.clone());
        let prev_keys: Vec<String> = previous.iter().map(ScannedDrive::key).collect();
        let next_keys: Vec<String> = scanned.iter().map(ScannedDrive::key).collect();
        let change = drives::diff(&prev_keys, &next_keys);

        if let Some(install) = install {
            for key in &change.removed {
                let label = previous
                    .iter()
                    .find(|scanned| scanned.key() == *key)
                    .map(|scanned| scanned.drive.label.clone())
                    .unwrap_or_else(|| key.clone());
                let report = install.abort_drive(key, &format!("{label} was removed"));
                tracing::info!(
                    drive = %key,
                    failed = report.failed,
                    cancelled = report.cancelled,
                    "backup drive removed"
                );
            }
            let live: Vec<String> = install
                .snapshot()
                .entries
                .iter()
                .filter(|entry| entry.status.is_live() && !entry.drive_id.is_empty())
                .map(|entry| entry.drive_id.clone())
                .collect();
            for key in &change.appeared {
                install.drive_connected(key);
                let Some(scanned) = scanned.iter().find(|scanned| scanned.key() == *key) else {
                    continue;
                };
                if scanned.ready_pack().is_none() || scanned.drive.read_only || live.contains(key) {
                    continue;
                }
                let mount = scanned.drive.mount.clone();
                let key = key.clone();
                let cleaned =
                    tokio::task::spawn_blocking(move || pack::cleanup_leftovers(&mount)).await;
                match cleaned {
                    Ok(Ok(0)) => {}
                    Ok(Ok(count)) => {
                        tracing::info!(drive = %key, count, "removed unfinished backup files")
                    }
                    Ok(Err(e)) => tracing::warn!(drive = %key, "leftover cleanup failed: {e}"),
                    Err(e) => tracing::warn!(drive = %key, "leftover cleanup did not finish: {e}"),
                }
            }
        }

        let remembered = self.merge_into_config(&scanned, &change.appeared).await;
        let snapshot = snapshot_of(&scanned, &remembered);
        let changed = {
            let mut last = self.snapshot.lock().unwrap();
            let changed = last.as_ref() != Some(&snapshot);
            *last = Some(snapshot.clone());
            changed
        };
        if changed {
            let _ = app.emit(DRIVES_CHANGED_EVENT, &snapshot);
        }
        snapshot
    }

    /// The remembered list after merging this scan into it; config is
    /// written only when the list changed.
    async fn merge_into_config(
        &self,
        scanned: &[ScannedDrive],
        appeared: &[String],
    ) -> Vec<RememberedDrive> {
        let known = self.remembered.lock().unwrap().clone();
        let existing = match known {
            Some(list) => list,
            None => {
                let loaded = tokio::task::spawn_blocking(|| {
                    Config::load(&Config::default_path())
                        .map(|config| config.backup.drives)
                        .unwrap_or_default()
                })
                .await
                .unwrap_or_default();
                *self.remembered.lock().unwrap() = Some(loaded.clone());
                loaded
            }
        };
        let connected: Vec<RememberedDrive> = scanned
            .iter()
            .filter_map(|s| s.ready_pack().map(|pack| remembered_of(&s.drive, pack)))
            .collect();
        let Some(merged) = merge_remembered(&existing, &connected, appeared, &pack::now_rfc3339())
        else {
            return existing;
        };
        self.save_remembered(merged.clone()).await;
        merged
    }

    async fn save_remembered(&self, list: Vec<RememberedDrive>) {
        *self.remembered.lock().unwrap() = Some(list.clone());
        let saved = tokio::task::spawn_blocking(move || {
            crate::config_write::modify_config(&Config::default_path(), |config| {
                config.backup.drives = list;
                Ok(())
            })
        })
        .await;
        match saved {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("remembered drives not saved: {e}"),
            Err(e) => tracing::warn!("remembered drives not saved: {e}"),
        }
    }

    /// Forgets the remembered drive `mount`.
    pub async fn forget(&self, mount: &str) {
        let known = self.remembered.lock().unwrap().clone();
        let mut list = match known {
            Some(list) => list,
            None => Config::load(&Config::default_path())
                .map(|config| config.backup.drives)
                .unwrap_or_default(),
        };
        let before = list.len();
        list.retain(|drive| drive.mount != mount);
        if list.len() != before {
            self.save_remembered(list).await;
        }
    }

    /// Records firmware to install from a drive once `platform`'s emulator
    /// is installed.
    pub fn add_pending_firmware(&self, pending: PendingDriveFirmware) {
        let mut list = self.pending_firmware.lock().unwrap();
        list.retain(|known| known.platform != pending.platform);
        list.push(pending);
    }

    /// The pending firmware whose platform now has a usable emulator,
    /// removed from the list. `has_emulator` answers for a platform label.
    pub fn take_ready_firmware(
        &self,
        has_emulator: impl Fn(&str) -> bool,
    ) -> Vec<PendingDriveFirmware> {
        let mut list = self.pending_firmware.lock().unwrap();
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut *list)
            .into_iter()
            .partition(|pending| has_emulator(&pending.platform));
        *list = waiting;
        ready
    }
}

/// Installs the firmware drive installs waited on for every platform whose
/// default emulator now resolves (called after an emulator install's
/// defaults are applied). Blocking: it reads the config.
pub fn install_waiting_firmware(
    drives: &Arc<DriveService>,
    firmware: &Arc<crate::firmware_service::FirmwareService>,
    app: &AppHandle,
) {
    let Ok(config) = Config::load(&Config::default_path()) else {
        return;
    };
    let profiles = grid_core::launch::profiles::load_profiles();
    let ready = drives.take_ready_firmware(|platform| {
        crate::firmware_service::default_entry_for_platform(&config, platform, profiles).is_some()
    });
    for pending in ready {
        firmware.spawn_from_pack(
            app.clone(),
            pending.platform,
            pending.mount,
            pending.pack_name,
            pending.entry,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected(mount: &str, read_only: bool) -> DetectedDrive {
        DetectedDrive {
            mount: PathBuf::from(mount),
            label: "SAMSUNG T7".to_string(),
            fs: "exfat".to_string(),
            fs_display: "exFAT".to_string(),
            fat32: false,
            total_bytes: 1000,
            free_bytes: 400,
            read_only,
        }
    }

    fn remembered(mount: &str, games: u32, seen: &str) -> RememberedDrive {
        RememberedDrive {
            mount: mount.to_string(),
            name: "Couch kit".to_string(),
            label: "SAMSUNG T7".to_string(),
            fs: "exFAT".to_string(),
            total_bytes: 1000,
            game_count: games,
            last_seen: seen.to_string(),
        }
    }

    #[test]
    fn classify_orders_format_problems_before_read_only() {
        let ready = Ok(PackState::Ready(pack::new_pack("Kit", &[PackOs::Linux])));
        assert_eq!(classify(false, &ready), DriveStatus::Ready);
        assert_eq!(classify(true, &ready), DriveStatus::ReadOnly);
        assert_eq!(
            classify(false, &Ok(PackState::Absent)),
            DriveStatus::NotSetUp
        );
        assert_eq!(
            classify(true, &Ok(PackState::Absent)),
            DriveStatus::ReadOnly
        );
        assert_eq!(
            classify(true, &Ok(PackState::NewerFormat(2))),
            DriveStatus::NewerFormat
        );
        assert_eq!(
            classify(false, &Ok(PackState::Unreadable("bad".into()))),
            DriveStatus::Unreadable
        );
        assert_eq!(
            classify(false, &Err("denied".into())),
            DriveStatus::Unreadable
        );
    }

    #[test]
    fn drive_info_carries_the_pack_summary_and_the_problem() {
        let mut kit = pack::new_pack("Couch kit", &[PackOs::Linux, PackOs::Windows]);
        kit.emulators.push(pack::PackEmulator {
            name: "DuckStation".into(),
            kind: "emulator".into(),
            source_id: "stenzek/duckstation".into(),
            version: "v1".into(),
            program: String::new(),
            args: String::new(),
            os: PackOs::Windows.arch_tag().into(),
            platforms: vec!["psx".into()],
            path: "emulators/DuckStation/v1/windows-x86_64/d.zip".into(),
            supplementals: Vec::new(),
            asset_name: "d.zip".into(),
            size: 7,
        });
        let info = drive_info(&ScannedDrive {
            drive: detected("/run/media/u/T7", false),
            pack: Ok(PackState::Ready(kit)),
        });
        assert_eq!(info.key, "/run/media/u/T7");
        assert_eq!(info.status, DriveStatus::Ready);
        let summary = info.pack.expect("a summary");
        assert_eq!(summary.name, "Couch kit");
        assert_eq!(summary.gridpack_bytes, 7);
        assert_eq!(summary.emulators[0].os, Some(PackOs::Windows));

        let broken = drive_info(&ScannedDrive {
            drive: detected("/run/media/u/T7", false),
            pack: Ok(PackState::Unreadable("no format number".into())),
        });
        assert_eq!(broken.status, DriveStatus::Unreadable);
        assert_eq!(broken.problem, "no format number");
        assert!(broken.pack.is_none());
    }

    #[test]
    fn merge_adds_new_drives_and_stamps_them() {
        let merged = merge_remembered(
            &[],
            &[remembered("/run/media/u/T7", 3, "")],
            &[],
            "2026-11-09T10:00:00Z",
        )
        .expect("a change");
        assert_eq!(
            merged,
            vec![remembered("/run/media/u/T7", 3, "2026-11-09T10:00:00Z")]
        );
    }

    #[test]
    fn merge_writes_nothing_while_a_drive_stays_plugged_in_unchanged() {
        let existing = vec![remembered("/run/media/u/T7", 3, "2026-11-08T10:00:00Z")];
        let connected = vec![remembered("/run/media/u/T7", 3, "")];
        assert_eq!(
            merge_remembered(&existing, &connected, &[], "2026-11-09T10:00:00Z"),
            None
        );
    }

    #[test]
    fn merge_refreshes_counts_and_last_seen_on_change_or_reconnect() {
        let existing = vec![
            remembered("/run/media/u/T7", 3, "2026-11-08T10:00:00Z"),
            remembered("E:", 1, "2026-11-01T10:00:00Z"),
        ];
        // A new game: the count and last seen change; the unplugged drive stays.
        let merged = merge_remembered(
            &existing,
            &[remembered("/run/media/u/T7", 4, "")],
            &[],
            "2026-11-09T10:00:00Z",
        )
        .expect("a change");
        assert_eq!(
            merged[0],
            remembered("/run/media/u/T7", 4, "2026-11-09T10:00:00Z")
        );
        assert_eq!(merged[1], existing[1]);

        // Plugged in again, unchanged: only last seen moves.
        let merged = merge_remembered(
            &existing,
            &[remembered("E:", 1, "")],
            &["E:".to_string()],
            "2026-11-09T11:00:00Z",
        )
        .expect("a change");
        assert_eq!(merged[1].last_seen, "2026-11-09T11:00:00Z");
        assert_eq!(merged[0], existing[0]);
    }

    #[test]
    fn snapshot_lists_only_remembered_drives_that_are_not_connected() {
        let scanned = vec![ScannedDrive {
            drive: detected("/run/media/u/T7", false),
            pack: Ok(PackState::Absent),
        }];
        let snapshot = snapshot_of(
            &scanned,
            &[
                remembered("/run/media/u/T7", 3, "x"),
                remembered("/run/media/u/Stick", 1, "y"),
            ],
        );
        assert_eq!(snapshot.connected.len(), 1);
        assert_eq!(snapshot.connected[0].status, DriveStatus::NotSetUp);
        assert_eq!(snapshot.remembered.len(), 1);
        assert_eq!(snapshot.remembered[0].mount, "/run/media/u/Stick");
    }

    #[test]
    fn pending_firmware_is_taken_once_its_platform_has_an_emulator() {
        let service = DriveService::new();
        let pending = |platform: &str| PendingDriveFirmware {
            platform: platform.to_string(),
            mount: PathBuf::from("/run/media/u/T7"),
            pack_name: "Couch kit".into(),
            entry: PackFirmware {
                platform: "psx".into(),
                platform_name: "PlayStation".into(),
                files: Vec::new(),
            },
        };
        service.add_pending_firmware(pending("Sony PlayStation"));
        service.add_pending_firmware(pending("Nintendo 64"));
        service.add_pending_firmware(pending("Sony PlayStation"));
        let ready = service.take_ready_firmware(|platform| platform == "Sony PlayStation");
        assert_eq!(ready.len(), 1);
        assert!(service
            .take_ready_firmware(|p| p == "Sony PlayStation")
            .is_empty());
        assert_eq!(service.take_ready_firmware(|_| true).len(), 1);
    }
}
