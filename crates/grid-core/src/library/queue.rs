//! Install queue state machine. Pure data and transitions — no I/O, no
//! async. The async service (a later task) drives this under a mutex. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! ("Queue rules", doc 03 §1, ported) for the behavior this implements.

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::content::ContentKind;
use super::LibraryError;
use crate::backup::pack::PackOs;
use crate::stores::{GameKey, StoreKind};

/// Identifies what one queued job is for. Two admissions with an equal key
/// dedupe against each other; two admissions with unequal keys — including a
/// `Rom` and an `Emulator` — never collide, regardless of the numbers or
/// strings inside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobKey {
    /// A RomM game install, keyed by `rom_id`.
    Rom(i64),
    /// A PS4 / Xbox 360 content (update or DLC) install for an already
    /// installed game, keyed by `rom_id` and the content kind — so an
    /// update and a DLC for the same game queue side by side.
    Content(i64, ContentKind),
    /// A native (Windows) game update install, keyed by `rom_id`.
    NativeUpdate(i64),
    /// An emulator acquisition, keyed by the source's `source_id`.
    Emulator(String),
    /// A download this queue does not drive: the entry exists so the
    /// drawer can show it, but the bytes are moved by someone else (the
    /// background firmware installer). Keyed by the entry's own id, so two
    /// external entries never dedupe against each other.
    External(u64),
    /// The library move (phase 9b, `library::relocate`). There is only ever
    /// one, so the key carries nothing. Its row takes no slot.
    LibraryMove,
    /// One item of a backup to a removable drive. `drive` is the drive key
    /// (the mount path, or the drive letter on Windows), so the same item
    /// queued for two drives does not dedupe.
    Backup { drive: String, item: BackupItemKey },
    /// The summary row [`QueueState::cancel_drive_queue`] leaves behind for
    /// the waiting rows it removed. Keyed by its own id; never retried.
    BackupSummary(u64),
    /// A store operation (install, update, ...) on one store product, run
    /// through the same slots, progress and cancel machinery as a RomM
    /// install ([`StoreJob`]). An install and an update of one product
    /// queue side by side; two installs of it dedupe.
    Store {
        source: StoreKind,
        store_id: String,
        op: StoreOp,
    },
}

/// What a store job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreOp {
    Install,
    Update,
    Repair,
    /// Install one DLC of an installed game.
    Dlc,
    /// Install from an offline installer file.
    Installer,
    /// Install a store game from a backup drive.
    Restore,
}

impl StoreOp {
    /// The entry `kind` this op shows as.
    pub fn kind(self) -> &'static str {
        match self {
            StoreOp::Install => "store_install",
            StoreOp::Update => "store_update",
            StoreOp::Repair => "store_repair",
            StoreOp::Dlc => "store_dlc",
            StoreOp::Installer => "store_installer",
            StoreOp::Restore => "store_restore",
        }
    }
}

/// What a running job is doing right now, finer than its status: the
/// Downloads line words it ("Verifying files", "Running the installer
/// silently"...). `None` for every job that does not report one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    #[default]
    None,
    Downloading,
    Installing,
    Verifying,
    Unpacking,
    InstallerSilent,
    InstallerWizard,
    CopyingInstaller,
    RunningInstaller,
    CopyingFromDrive,
    Checksum,
}

/// Why a job failed, when the Downloads row words it from parts rather than
/// from the error text alone (the installer failures).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    InstallerUnrecognised,
    InstallerDlcBaseMissing,
    InstallerExit,
    InstallerNeedsWizard,
}

/// A failed entry's [`FailureCode`] and its parts; blank where a code has
/// none.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct JobFailure {
    pub code: FailureCode,
    pub base_title: String,
    pub base_gog_id: String,
    /// The download (or installer) was kept for a retry.
    pub kept: bool,
}

impl JobFailure {
    /// The failure `error` carries, `None` for an error the row words from
    /// its text alone.
    pub fn from_error(error: &LibraryError) -> Option<JobFailure> {
        let failure = |code| JobFailure {
            code,
            base_title: String::new(),
            base_gog_id: String::new(),
            kept: false,
        };
        match error {
            LibraryError::InstallerUnrecognised { .. } => {
                Some(failure(FailureCode::InstallerUnrecognised))
            }
            LibraryError::InstallerDlcBaseMissing {
                base_title,
                base_gog_id,
            } => Some(JobFailure {
                base_title: base_title.clone(),
                base_gog_id: base_gog_id.clone(),
                ..failure(FailureCode::InstallerDlcBaseMissing)
            }),
            LibraryError::InstallerExit(_) => Some(failure(FailureCode::InstallerExit)),
            LibraryError::InstallerNeedsWizard { kept } => Some(JobFailure {
                kept: *kept,
                ..failure(FailureCode::InstallerNeedsWizard)
            }),
            _ => None,
        }
    }
}

/// Where a store job's finalize reports its progress and phase.
pub trait StoreProgressSink {
    /// `progress(processed, total)` of the install half.
    fn progress(&mut self, done: u64, total: u64);
    /// The job moved to `phase`.
    fn phase(&mut self, phase: JobPhase);
}

/// The work behind a store job, supplied by each store's phase (legendary,
/// gogdl, nile, an offline installer). Both halves run on the blocking
/// pool: a store tool is a subprocess whose progress is read line by line.
///
/// Never log or forward a tool's stdout or stderr line from an
/// implementation (AGENTS.md "Secrets"): report progress through the
/// callbacks and failures as GRID's own words.
pub trait StoreJobRunner: Send + Sync {
    /// The download half. `progress(downloaded, total, speed_bps)`; stop
    /// promptly with [`LibraryError::Cancelled`] once `cancel` is set.
    fn download(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError>;

    /// The install half (registry write included): progress and phase go
    /// to `sink`. Returns a warning for the finished entry, `""` for none.
    /// Not cancellable, like a RomM finalize.
    fn finalize(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError>;
}

/// The backup drive a store job copies from (a restore).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DriveRef {
    /// The drive key: the mount path, or the drive letter on Windows.
    pub key: String,
    /// The pack name.
    pub name: String,
    /// The volume label.
    pub label: String,
}

/// One store job for [`super::InstallService::enqueue_store_job`].
#[derive(Clone)]
pub struct StoreJob {
    pub source: StoreKind,
    pub store_id: String,
    pub op: StoreOp,
    /// The Downloads row's title.
    pub title: String,
    /// The drive a restore copies from; its row shows it.
    pub drive: Option<DriveRef>,
    pub runner: Arc<dyn StoreJobRunner>,
}

impl StoreJob {
    /// The queue key this job dedupes on.
    pub fn job_key(&self) -> JobKey {
        JobKey::Store {
            source: self.source,
            store_id: self.store_id.clone(),
            op: self.op,
        }
    }

    /// The product's [`GameKey`].
    pub fn game_key(&self) -> GameKey {
        GameKey::store(self.source, self.store_id.clone())
    }
}

impl std::fmt::Debug for StoreJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreJob")
            .field("source", &self.source)
            .field("store_id", &self.store_id)
            .field("op", &self.op)
            .field("title", &self.title)
            .field("drive", &self.drive)
            .finish_non_exhaustive()
    }
}

/// What one backup row writes to a drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupItemKey {
    /// A game, by its RomM rom id.
    Game(i64),
    /// A store game (GOG), by its key.
    Store(GameKey),
    /// One emulator release for one OS, by its profile name.
    Emulator { name: String, os: PackOs },
    /// The firmware of one platform, by its slug.
    Firmware(String),
}

/// The drive and batch an entry belongs to. All blank / `0` for an entry
/// that has nothing to do with a drive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryMeta {
    /// The drive key: the mount path, or the drive letter on Windows.
    pub drive_id: String,
    /// The pack name the user gave the drive.
    pub drive_name: String,
    /// The drive's volume label.
    pub drive_label: String,
    /// The backup batch, so the frontend raises one notification per
    /// batch. `0` for none.
    pub batch: u64,
}

/// Lifecycle state of one [`DownloadEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Installing,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

/// One tracked install, from admission through a terminal status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadEntry {
    pub id: u64,
    /// `"game"` for a `JobKey::Rom` / `Content` / `NativeUpdate` entry,
    /// `"emulator"` for a `JobKey::Emulator` one, `"firmware"` for an
    /// external one, `"backup"` for a `JobKey::Backup` one and the
    /// `"backup_cancelled"` summary row, `"store"` for a `JobKey::Store`
    /// one (its `source_id` is the product's game key).
    pub job: &'static str,
    /// What this entry installs: `"base"`, `"ps4_content"`,
    /// `"xbox360_content"`, `"native_update"`, `"emulator"`,
    /// `"compat_tool"` or `"firmware"`; for a backup, `"backup_game"`,
    /// `"backup_emulator"`, `"backup_firmware"` or `"backup_cancelled"`.
    /// Finer-grained than `job`, which only says which half of the
    /// pipeline owns the entry.
    pub kind: &'static str,
    /// The rom id for a `"game"` entry and a backed-up game; `0` for an
    /// `"emulator"` entry.
    pub rom_id: i64,
    /// The source id for an `"emulator"` entry; empty for a `"game"` entry.
    pub source_id: String,
    pub title: String,
    pub platform: String,
    pub status: DownloadStatus,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: f64,
    pub install_processed_bytes: u64,
    pub install_total_bytes: u64,
    pub error: String,
    /// One line of free text under the title: what a library move is
    /// doing (`"Copying games"`). Empty for every other entry.
    #[serde(default)]
    pub detail: String,
    /// The drive this entry writes to (a backup row) or copies from (an
    /// install-from-drive row): its mount path, or its drive letter on
    /// Windows. Empty for every other entry.
    pub drive_id: String,
    /// The pack name of that drive. Empty when `drive_id` is.
    pub drive_name: String,
    /// The volume label of that drive. A non-empty label on a `"game"` row
    /// means it installs from the drive. Empty when `drive_id` is.
    pub drive_label: String,
    /// The backup batch this entry belongs to; `0` for none.
    pub batch: u64,
    /// What the job is doing right now; [`JobPhase::None`] for a job that
    /// does not say.
    pub phase: JobPhase,
    /// Why a failed entry failed, when its row words it from parts (the
    /// installer failures); `None` otherwise.
    pub failure: Option<JobFailure>,
    /// The admission key this entry was created from. Kept for dedupe and
    /// `retryable`; the public `job`/`rom_id`/`source_id` fields above are
    /// derived from it once, at insertion.
    #[serde(skip)]
    key: JobKey,
}

/// How many terminal entries (`Completed`, `Failed`, `Cancelled`) the list
/// keeps (design §8: "Completed keeps the last 50 entries"). Every terminal
/// transition drops the oldest ones past this; live entries are never
/// counted or pruned. The cap lives here, not in the frontend, because this
/// list is the source of truth `list_downloads` returns.
pub const TERMINAL_HISTORY: usize = 50;

/// The full entry list, newest first (reverse of insertion order).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadsSnapshot {
    pub entries: Vec<DownloadEntry>,
}

impl DownloadStatus {
    /// Queued, downloading, installing or cancelling: the entry may still
    /// write into the library.
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Downloading | Self::Installing | Self::Cancelling
        )
    }
}

impl DownloadsSnapshot {
    /// True when any install entry is still live
    /// ([`DownloadStatus::is_live`]), firmware rows included. A library
    /// path change waits for this. The library move's own row is not an
    /// install and does not count: see [`Self::has_live_library_move`].
    pub fn has_live_entry(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.status.is_live() && entry.key != JobKey::LibraryMove)
    }

    /// True while the library move's row is live.
    pub fn has_live_library_move(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.status.is_live() && entry.key == JobKey::LibraryMove)
    }
}

/// Result of [`QueueState::admit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Both slots were free: the entry was created `Downloading` and the
    /// caller should start the download task for this id.
    Start(u64),
    /// A slot was busy: the entry was created `Queued` and appended to
    /// `waiting`.
    Queued(u64),
    /// `rom_id` was already active or waiting; nothing was created.
    Duplicate,
}

/// Result of [`QueueState::request_cancel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelAction {
    /// The entry was the active download: status is now `Cancelling`: the
    /// caller must flip the cooperative cancellation flag.
    ActiveDownload,
    /// The entry was queued: it was removed from `waiting` and marked
    /// `Cancelled`.
    RemovedFromQueue,
    /// The entry is a backup row writing to its drive (it owns the
    /// finalize slot): status is now `Cancelling`: the caller must flip the
    /// writer's cancellation flag. Only a backup's write is cancellable.
    ActiveWrite,
    /// The entry does not exist, or is not cancellable in its current
    /// status (e.g. already finalizing or terminal).
    Ignored,
}

/// The install queue: at most one active download and one active finalize,
/// with the rest FIFO in `waiting`.
#[derive(Default)]
pub struct QueueState {
    entries: Vec<DownloadEntry>,
    next_id: u64,
    download_active: Option<u64>,
    finalize_active: Option<u64>,
    waiting: VecDeque<u64>,
}

impl QueueState {
    /// Admits a new job for `key`, tagged with `kind` (one of `"base"`,
    /// `"ps4_content"`, `"xbox360_content"`, `"native_update"`,
    /// `"emulator"`, `"compat_tool"`). Duplicate check (an equal `key`
    /// already active in either slot's entry, or waiting) happens first and
    /// creates nothing. Otherwise creates the entry: `Downloading` and takes
    /// the download slot when both slots are free, else `Queued` and
    /// appended to `waiting`.
    pub fn admit(
        &mut self,
        key: JobKey,
        title: &str,
        platform: &str,
        kind: &'static str,
    ) -> Admission {
        self.admit_with(key, title, platform, kind, EntryMeta::default())
    }

    /// [`Self::admit`], with the drive and batch the entry belongs to. A
    /// `JobKey::Backup` entry gets `job` `"backup"`; for a game item its
    /// `rom_id` is the game's.
    pub fn admit_with(
        &mut self,
        key: JobKey,
        title: &str,
        platform: &str,
        kind: &'static str,
        meta: EntryMeta,
    ) -> Admission {
        if self.has_pending(&key) {
            return Admission::Duplicate;
        }

        let id = self.alloc_id();
        let both_free = self.download_active.is_none() && self.finalize_active.is_none();
        let status = if both_free {
            DownloadStatus::Downloading
        } else {
            DownloadStatus::Queued
        };
        let (job, rom_id, source_id) = match &key {
            JobKey::Rom(rom_id) | JobKey::Content(rom_id, _) | JobKey::NativeUpdate(rom_id) => {
                ("game", *rom_id, String::new())
            }
            JobKey::Emulator(source_id) => ("emulator", 0, source_id.clone()),
            // Never reached: an external entry is created by
            // `admit_external`, which never routes through here.
            JobKey::External(_) => ("firmware", 0, String::new()),
            // Never reached either: `admit_library_move` creates its own row.
            JobKey::LibraryMove => ("library", 0, String::new()),
            JobKey::Backup { item, .. } => {
                let rom_id = match item {
                    BackupItemKey::Game(rom_id) => *rom_id,
                    BackupItemKey::Store(_)
                    | BackupItemKey::Emulator { .. }
                    | BackupItemKey::Firmware(_) => 0,
                };
                // A store game's row carries its game key, like a store job.
                let source_id = match item {
                    BackupItemKey::Store(key) => key.to_string(),
                    _ => String::new(),
                };
                ("backup", rom_id, source_id)
            }
            // Never reached: `cancel_drive_queue` creates its own row.
            JobKey::BackupSummary(_) => ("backup", 0, String::new()),
            // `source_id` carries the product's game key (`gog:1207664643`).
            JobKey::Store {
                source, store_id, ..
            } => (
                "store",
                0,
                GameKey::store(*source, store_id.clone()).to_string(),
            ),
        };
        let EntryMeta {
            drive_id,
            drive_name,
            drive_label,
            batch,
        } = meta;
        self.entries.push(DownloadEntry {
            id,
            job,
            kind,
            rom_id,
            source_id,
            title: title.to_string(),
            platform: platform.to_string(),
            status,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bps: 0.0,
            install_processed_bytes: 0,
            install_total_bytes: 0,
            error: String::new(),
            detail: String::new(),
            drive_id,
            drive_name,
            drive_label,
            batch,
            phase: JobPhase::None,
            failure: None,
            key,
        });

        if both_free {
            self.download_active = Some(id);
            Admission::Start(id)
        } else {
            self.waiting.push_back(id);
            Admission::Queued(id)
        }
    }

    /// Creates a drawer row for a transfer this queue does NOT drive — the
    /// background firmware installer moves its own bytes and reports back
    /// through [`Self::finish_external`].
    ///
    /// The entry is created `Downloading` but takes neither slot and is
    /// never appended to `waiting`, so [`Self::next_ready`] can never return
    /// it and it neither blocks nor is blocked by a real install. Its key is
    /// its own id, so two external entries never dedupe against each other.
    pub fn admit_external(&mut self, title: &str, platform: &str) -> u64 {
        let id = self.alloc_id();
        self.entries.push(DownloadEntry {
            id,
            job: "firmware",
            kind: "firmware",
            rom_id: 0,
            source_id: String::new(),
            title: title.to_string(),
            platform: platform.to_string(),
            status: DownloadStatus::Downloading,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bps: 0.0,
            install_processed_bytes: 0,
            install_total_bytes: 0,
            error: String::new(),
            detail: String::new(),
            drive_id: String::new(),
            drive_name: String::new(),
            drive_label: String::new(),
            batch: 0,
            phase: JobPhase::None,
            failure: None,
            key: JobKey::External(id),
        });
        id
    }

    /// Reports the outcome of an [`Self::admit_external`] entry: a blank
    /// `error` completes it, anything else fails it with that text. Ignores
    /// an id that is unknown or not an external entry, so a stray call can
    /// never rewrite a real install's status.
    pub fn finish_external(&mut self, id: u64, error: &str) {
        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        if !matches!(entry.key, JobKey::External(_)) {
            return;
        }
        entry.speed_bps = 0.0;
        if error.is_empty() {
            entry.status = DownloadStatus::Completed;
            entry.error.clear();
        } else {
            entry.status = DownloadStatus::Failed;
            entry.error = error.to_string();
        }
        self.prune_terminal();
    }

    /// Creates the library move's Downloads row, `Installing` from the
    /// start. Like an external row it takes no slot and never enters
    /// `waiting`, so it neither blocks nor is blocked by the queue (the
    /// move itself refuses to start while an install is live). `None` when
    /// a live move row already exists.
    pub fn admit_library_move(&mut self, title: &str, platform: &str) -> Option<u64> {
        if self.snapshot().has_live_library_move() {
            return None;
        }
        let id = self.alloc_id();
        self.entries.push(DownloadEntry {
            id,
            job: "library",
            kind: "library_move",
            rom_id: 0,
            source_id: String::new(),
            title: title.to_string(),
            platform: platform.to_string(),
            status: DownloadStatus::Installing,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bps: 0.0,
            install_processed_bytes: 0,
            install_total_bytes: 0,
            error: String::new(),
            detail: String::new(),
            drive_id: String::new(),
            drive_name: String::new(),
            drive_label: String::new(),
            batch: 0,
            phase: JobPhase::None,
            failure: None,
            key: JobKey::LibraryMove,
        });
        Some(id)
    }

    /// Sets the free-text detail line of `id`. No-op when `id` is unknown.
    pub fn set_detail(&mut self, id: u64, detail: &str) {
        if let Some(entry) = self.entry_mut(id) {
            entry.detail = detail.to_string();
        }
    }

    /// Sets the drive and batch of `id` (an install-from-drive row, whose
    /// drive is known only once it is planned). No-op when `id` is unknown.
    pub fn set_drive_meta(&mut self, id: u64, meta: EntryMeta) {
        if let Some(entry) = self.entry_mut(id) {
            entry.drive_id = meta.drive_id;
            entry.drive_name = meta.drive_name;
            entry.drive_label = meta.drive_label;
            entry.batch = meta.batch;
        }
    }

    /// The drive `drive_id` is gone: removes every WAITING entry tied to it
    /// (backup rows and install-from-drive rows alike) from the queue and
    /// from the list, and returns their ids so the caller can drop their
    /// pending payloads. Never touches a slot owner: the caller ends the
    /// active item through its own flag.
    ///
    /// When at least one entry was removed, one `Cancelled` summary row
    /// (`job` `"backup"`, `kind` `"backup_cancelled"`) takes their place:
    /// titled with the item's own title for one, else `"<n> more items"`,
    /// with `reason` as its error and the first removed entry's drive and
    /// batch. A blank `drive_id` matches nothing.
    pub fn cancel_drive_queue(&mut self, drive_id: &str, reason: &str) -> Vec<u64> {
        if drive_id.is_empty() {
            return Vec::new();
        }
        let removed: Vec<u64> = self
            .waiting
            .iter()
            .copied()
            .filter(|&id| {
                self.entry(id)
                    .is_some_and(|entry| entry.drive_id == drive_id)
            })
            .collect();
        let Some(first) = removed.first().and_then(|&id| self.entry(id)).cloned() else {
            return removed;
        };
        self.waiting.retain(|id| !removed.contains(id));
        self.entries.retain(|entry| !removed.contains(&entry.id));

        let title = if removed.len() == 1 {
            first.title.clone()
        } else {
            format!("{} more items", removed.len())
        };
        let id = self.alloc_id();
        self.entries.push(DownloadEntry {
            id,
            job: "backup",
            kind: "backup_cancelled",
            rom_id: 0,
            source_id: String::new(),
            title,
            platform: first.platform,
            status: DownloadStatus::Cancelled,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bps: 0.0,
            install_processed_bytes: 0,
            install_total_bytes: 0,
            error: reason.to_string(),
            detail: String::new(),
            drive_id: first.drive_id,
            drive_name: first.drive_name,
            drive_label: first.drive_label,
            batch: first.batch,
            phase: JobPhase::None,
            failure: None,
            key: JobKey::BackupSummary(id),
        });
        self.prune_terminal();
        removed
    }

    /// Ends a [`Self::admit_library_move`] row: a blank `error` completes
    /// it, anything else fails it with that text. Ignores any other entry.
    pub fn finish_library_move(&mut self, id: u64, error: &str) {
        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        if entry.key != JobKey::LibraryMove {
            return;
        }
        entry.speed_bps = 0.0;
        if error.is_empty() {
            entry.status = DownloadStatus::Completed;
            entry.error.clear();
        } else {
            entry.status = DownloadStatus::Failed;
            entry.error = error.to_string();
        }
        self.prune_terminal();
    }

    /// Shows whether a live library move row is copying (`Downloading`: the
    /// Copy phase, which can be cancelled) or committing and cleaning up
    /// (`Installing`: no cancel). Ignores any other entry and any terminal
    /// row.
    pub fn set_library_move_copying(&mut self, id: u64, copying: bool) {
        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        if entry.key != JobKey::LibraryMove || !entry.status.is_live() {
            return;
        }
        entry.status = if copying {
            DownloadStatus::Downloading
        } else {
            DownloadStatus::Installing
        };
    }

    /// Ends a library move row as `Cancelled` (the user cancelled the
    /// Copy phase and the old library is unchanged). Ignores any other
    /// entry.
    pub fn cancel_library_move(&mut self, id: u64) {
        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        if entry.key != JobKey::LibraryMove {
            return;
        }
        entry.speed_bps = 0.0;
        entry.status = DownloadStatus::Cancelled;
        entry.error.clear();
        self.prune_terminal();
    }

    /// Ties entry `id` to the server ROM it comes from. A server emulator
    /// build is keyed by its source id, but it is still a file of one ROM:
    /// with the ROM id on its row, that ROM's details panel follows it like a
    /// game install (progress, Cancel).
    pub fn set_rom_id(&mut self, id: u64, rom_id: i64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.rom_id = rom_id;
        }
    }

    /// The id of the oldest entry for `rom_id` that has not reached a
    /// terminal status yet (`Queued`, `Downloading` or `Installing`), or
    /// `None` when the game has no live entry. A backup row is not an
    /// install of the game, so it never counts.
    pub fn first_live_for_rom(&self, rom_id: i64) -> Option<u64> {
        self.entries
            .iter()
            .find(|entry| {
                entry.rom_id == rom_id
                    && !matches!(entry.key, JobKey::Backup { .. })
                    && matches!(
                        entry.status,
                        DownloadStatus::Queued
                            | DownloadStatus::Downloading
                            | DownloadStatus::Installing
                    )
            })
            .map(|entry| entry.id)
    }

    /// Updates download progress for `id`. No-op when `id` is unknown.
    /// `speed` clamps to `>= 0.0`.
    pub fn set_progress(&mut self, id: u64, downloaded: u64, total: u64, speed: f64) {
        if let Some(entry) = self.entry_mut(id) {
            entry.downloaded_bytes = downloaded;
            entry.total_bytes = total;
            entry.speed_bps = speed.max(0.0);
        }
    }

    /// Sets the phase of `id`. No-op when `id` is unknown.
    pub fn set_phase(&mut self, id: u64, phase: JobPhase) {
        if let Some(entry) = self.entry_mut(id) {
            entry.phase = phase;
        }
    }

    /// Updates install (extraction/finalize) progress for `id`. No-op when
    /// `id` is unknown.
    pub fn set_install_progress(&mut self, id: u64, processed: u64, total: u64) {
        if let Some(entry) = self.entry_mut(id) {
            entry.install_processed_bytes = processed;
            entry.install_total_bytes = total;
        }
    }

    /// Download task ended. No-op unless `id` currently owns the download
    /// slot (guards against a stale/duplicate completion call arriving
    /// after the slot has already moved on). Frees the download slot.
    /// `Ok(())` with `skip_finalize` false → `Installing`, `finalize_active
    /// = Some(id)`. `Ok(())` with `skip_finalize` true (already installed)
    /// → `Completed` directly. `Err(LibraryError::Cancelled)` → `Cancelled`.
    /// Any other `Err(e)` → `Failed` with `e`'s `Display` text as `error`
    /// (`DriveRemoved` included: "SAMSUNG T7 was removed").
    pub fn download_finished(
        &mut self,
        id: u64,
        result: Result<(), LibraryError>,
        skip_finalize: bool,
    ) {
        if self.download_active != Some(id) {
            return;
        }
        self.download_active = None;

        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        entry.speed_bps = 0.0;
        match result {
            Ok(()) if skip_finalize => {
                entry.status = DownloadStatus::Completed;
            }
            Ok(()) => {
                entry.status = DownloadStatus::Installing;
                self.finalize_active = Some(id);
            }
            Err(LibraryError::Cancelled) => {
                entry.status = DownloadStatus::Cancelled;
            }
            Err(e) => {
                entry.status = DownloadStatus::Failed;
                entry.failure = JobFailure::from_error(&e);
                entry.error = e.to_string();
            }
        }
        self.prune_terminal();
    }

    /// Finalize task ended. No-op unless `id` currently owns the finalize
    /// slot (guards against a stale/duplicate completion call arriving
    /// after the slot has already moved on). Frees the finalize slot.
    /// `Ok(())` → `Completed`; when `warning` is non-empty it is stored in
    /// `error` even though the entry completed. `Err(LibraryError::Cancelled)`
    /// (a backup write cancelled through [`CancelAction::ActiveWrite`]) →
    /// `Cancelled`, with `warning` as `error`. Any other `Err(e)` → `Failed`
    /// with `e`'s `Display` text as `error` (`DriveRemoved` included), with
    /// `warning` appended after a newline when non-empty.
    pub fn finalize_finished(&mut self, id: u64, result: Result<(), LibraryError>, warning: &str) {
        if self.finalize_active != Some(id) {
            return;
        }
        self.finalize_active = None;

        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        entry.speed_bps = 0.0;
        match result {
            Ok(()) => {
                entry.status = DownloadStatus::Completed;
                entry.error = warning.to_string();
            }
            Err(LibraryError::Cancelled) => {
                entry.status = DownloadStatus::Cancelled;
                entry.error = warning.to_string();
            }
            Err(e) => {
                entry.status = DownloadStatus::Failed;
                entry.failure = JobFailure::from_error(&e);
                entry.error = if warning.is_empty() {
                    e.to_string()
                } else {
                    format!("{e}\n{warning}")
                };
            }
        }
        self.prune_terminal();
    }

    /// When both slots are free and `waiting` is non-empty, pops the front
    /// id, marks its entry `Downloading`, takes the download slot, and
    /// returns it. Otherwise returns `None`.
    pub fn next_ready(&mut self) -> Option<u64> {
        if self.download_active.is_some() || self.finalize_active.is_some() {
            return None;
        }
        let id = self.waiting.pop_front()?;
        if let Some(entry) = self.entry_mut(id) {
            entry.status = DownloadStatus::Downloading;
        }
        self.download_active = Some(id);
        Some(id)
    }

    /// Requests cancellation of `id`. See [`CancelAction`] for the
    /// outcomes. A finalize that is not a backup's write is not cancellable.
    pub fn request_cancel(&mut self, id: u64) -> CancelAction {
        if self.download_active == Some(id) {
            if let Some(entry) = self.entry_mut(id) {
                entry.status = DownloadStatus::Cancelling;
            }
            return CancelAction::ActiveDownload;
        }

        if self.finalize_active == Some(id) {
            return match self.entry_mut(id) {
                Some(entry) if matches!(entry.key, JobKey::Backup { .. }) => {
                    entry.status = DownloadStatus::Cancelling;
                    CancelAction::ActiveWrite
                }
                _ => CancelAction::Ignored,
            };
        }

        if let Some(pos) = self.waiting.iter().position(|&waiting_id| waiting_id == id) {
            self.waiting.remove(pos);
            if let Some(entry) = self.entry_mut(id) {
                entry.status = DownloadStatus::Cancelled;
                entry.error = "Cancelled while queued".to_string();
                entry.speed_bps = 0.0;
            }
            self.prune_terminal();
            return CancelAction::RemovedFromQueue;
        }

        CancelAction::Ignored
    }

    /// Removes the entry for `id` from the list (and from `waiting` if
    /// still present, so a dismissed queued entry is never started later).
    /// Refuses (returns `false`, leaves everything intact) when `id`
    /// currently owns the download or finalize slot: dismissing it would
    /// leave that slot pointing at a dead id, breaking duplicate detection
    /// and wedging the queue. Callers only offer dismiss for
    /// terminal entries, but this method is the trust boundary — it does
    /// not rely on that convention.
    pub fn dismiss(&mut self, id: u64) -> bool {
        if self.download_active == Some(id) || self.finalize_active == Some(id) {
            return false;
        }
        if let Some(pos) = self.waiting.iter().position(|&waiting_id| waiting_id == id) {
            self.waiting.remove(pos);
        }
        if let Some(pos) = self.entries.iter().position(|entry| entry.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// `Some(key)` when `id`'s entry is `Failed` or `Cancelled`; `None`
    /// otherwise (including when `id` is unknown, and for a
    /// `"backup_cancelled"` summary row, which offers Dismiss only).
    pub fn retryable(&self, id: u64) -> Option<JobKey> {
        let entry = self.entry(id)?;
        if matches!(entry.key, JobKey::BackupSummary(_)) {
            return None;
        }
        match entry.status {
            DownloadStatus::Failed | DownloadStatus::Cancelled => Some(entry.key.clone()),
            _ => None,
        }
    }

    /// The full entry list, newest first.
    pub fn snapshot(&self) -> DownloadsSnapshot {
        let mut entries = self.entries.clone();
        entries.reverse();
        DownloadsSnapshot { entries }
    }

    /// Looks up an entry by id.
    pub fn entry(&self, id: u64) -> Option<&DownloadEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn entry_mut(&mut self, id: u64) -> Option<&mut DownloadEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// Whether `key` is already active (download or finalize slot) or
    /// sitting in `waiting`.
    pub fn has_pending(&self, key: &JobKey) -> bool {
        let active = [self.download_active, self.finalize_active]
            .into_iter()
            .flatten()
            .any(|id| self.entry(id).is_some_and(|entry| &entry.key == key));
        if active {
            return true;
        }
        self.waiting
            .iter()
            .any(|&id| self.entry(id).is_some_and(|entry| &entry.key == key))
    }

    fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn is_terminal(status: DownloadStatus) -> bool {
        matches!(
            status,
            DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
        )
    }

    /// Drops the oldest terminal entries past [`TERMINAL_HISTORY`]. Entries
    /// are stored oldest first, so a forward `retain` evicts in age order.
    /// A terminal entry never owns a slot and never sits in `waiting`, so
    /// nothing else needs updating.
    fn prune_terminal(&mut self) {
        let terminal = self
            .entries
            .iter()
            .filter(|entry| Self::is_terminal(entry.status))
            .count();
        let mut excess = terminal.saturating_sub(TERMINAL_HISTORY);
        if excess == 0 {
            return;
        }
        self.entries.retain(|entry| {
            if excess > 0 && Self::is_terminal(entry.status) {
                excess -= 1;
                false
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_key(store_id: &str, op: StoreOp) -> JobKey {
        JobKey::Store {
            source: StoreKind::Gog,
            store_id: store_id.to_string(),
            op,
        }
    }

    #[test]
    fn a_store_job_takes_a_slot_and_dedupes_by_product_and_op() {
        let mut state = QueueState::default();
        let Admission::Start(first) = state.admit(
            store_key("1207664643", StoreOp::Install),
            "The Witcher",
            "GOG",
            StoreOp::Install.kind(),
        ) else {
            panic!("an idle queue starts the store job");
        };
        let entry = state.entry(first).unwrap();
        assert_eq!(entry.job, "store");
        assert_eq!(entry.kind, "store_install");
        assert_eq!(entry.rom_id, 0);
        assert_eq!(entry.source_id, "gog:1207664643");
        assert_eq!(entry.status, DownloadStatus::Downloading);

        // The same product and op again: a duplicate.
        assert_eq!(
            state.admit(
                store_key("1207664643", StoreOp::Install),
                "The Witcher",
                "GOG",
                "store_install"
            ),
            Admission::Duplicate
        );
        // An update of it, another product, and a RomM game all queue.
        assert!(matches!(
            state.admit(
                store_key("1207664643", StoreOp::Update),
                "The Witcher",
                "GOG",
                "store_update"
            ),
            Admission::Queued(_)
        ));
        assert!(matches!(
            state.admit(
                store_key("2", StoreOp::Install),
                "Doom",
                "GOG",
                "store_install"
            ),
            Admission::Queued(_)
        ));
        assert!(matches!(
            state.admit(JobKey::Rom(1207664643), "Doom", "SNES", "base"),
            Admission::Queued(_)
        ));
        assert_eq!(
            state.retryable(first),
            None,
            "a live entry is not retryable"
        );
    }

    fn admit_idle(state: &mut QueueState, rom_id: i64) -> u64 {
        match state.admit(JobKey::Rom(rom_id), "Title", "Platform", "base") {
            Admission::Start(id) => id,
            other => panic!("expected Start, got {other:?}"),
        }
    }

    // --- has_live_entry ---------------------------------------------------

    #[test]
    fn live_statuses_are_the_non_terminal_ones() {
        let live: Vec<DownloadStatus> = [
            DownloadStatus::Queued,
            DownloadStatus::Downloading,
            DownloadStatus::Installing,
            DownloadStatus::Cancelling,
            DownloadStatus::Completed,
            DownloadStatus::Failed,
            DownloadStatus::Cancelled,
        ]
        .into_iter()
        .filter(|status| status.is_live())
        .collect();
        assert_eq!(
            live,
            vec![
                DownloadStatus::Queued,
                DownloadStatus::Downloading,
                DownloadStatus::Installing,
                DownloadStatus::Cancelling,
            ]
        );
    }

    /// A library move has its own Downloads row: live while it runs, but
    /// not a live INSTALL, so the path-change and move checks can tell the
    /// two apart.
    #[test]
    fn a_library_move_row_is_live_but_not_an_install() {
        let mut state = QueueState::default();
        let id = state
            .admit_library_move("Library move", "D:\\Games")
            .unwrap();
        assert_eq!(
            state.admit_library_move("Library move", "x"),
            None,
            "one at a time"
        );
        let entry = state.entry(id).unwrap().clone();
        assert_eq!(entry.job, "library");
        assert_eq!(entry.kind, "library_move");
        assert_eq!(entry.status, DownloadStatus::Installing);
        assert_eq!(entry.detail, "");
        let snapshot = state.snapshot();
        assert!(!snapshot.has_live_entry(), "a move is not a live install");
        assert!(snapshot.has_live_library_move());

        state.set_detail(id, "Copying games");
        state.set_install_progress(id, 5, 10);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.detail, "Copying games");
        assert_eq!(entry.install_processed_bytes, 5);

        // Cancel is the move's own command, not the queue's.
        assert_eq!(state.request_cancel(id), CancelAction::Ignored);
        assert!(state.next_ready().is_none(), "it never takes a slot");

        state.finish_library_move(id, "");
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Completed);
        assert!(!state.snapshot().has_live_library_move());
        assert_eq!(state.retryable(id), None);

        let failed = state.admit_library_move("Library move", "x").unwrap();
        state.finish_library_move(failed, "disk full");
        let entry = state.entry(failed).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, "disk full");
        assert_eq!(state.retryable(failed), Some(JobKey::LibraryMove));

        // A stray finish for an ordinary install changes nothing.
        let rom = admit_idle(&mut state, 9);
        state.finish_library_move(rom, "nope");
        assert_eq!(
            state.entry(rom).unwrap().status,
            DownloadStatus::Downloading
        );
        assert!(state.snapshot().has_live_entry());
    }

    #[test]
    fn a_library_move_row_shows_its_phase_and_can_end_cancelled() {
        let mut state = QueueState::default();
        let id = state.admit_library_move("Library move", "x").unwrap();
        state.set_library_move_copying(id, true);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Downloading);
        assert!(state.snapshot().has_live_library_move());
        assert!(!state.snapshot().has_live_entry());
        // Still not the queue's to cancel, and still no slot.
        assert_eq!(state.request_cancel(id), CancelAction::Ignored);
        assert!(state.next_ready().is_none());
        state.set_library_move_copying(id, false);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);

        state.cancel_library_move(id);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Cancelled);
        assert_eq!(entry.error, "");
        assert!(!state.snapshot().has_live_library_move());
        // A terminal row keeps its status.
        state.set_library_move_copying(id, true);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Cancelled);

        // Neither touches an ordinary install.
        let rom = admit_idle(&mut state, 9);
        state.set_library_move_copying(rom, false);
        state.cancel_library_move(rom);
        assert_eq!(
            state.entry(rom).unwrap().status,
            DownloadStatus::Downloading
        );
    }

    /// The live-install check sees a live install even when a move row is
    /// there too.
    #[test]
    fn a_live_install_beside_a_move_row_still_counts() {
        let mut state = QueueState::default();
        state.admit_library_move("Library move", "x").unwrap();
        admit_idle(&mut state, 1);
        assert!(state.snapshot().has_live_entry());
    }

    #[test]
    fn a_snapshot_with_an_external_row_is_live_until_it_finishes() {
        let mut state = QueueState::default();
        assert!(!state.snapshot().has_live_entry());
        let id = state.admit_external("Firmware", "PS3");
        assert!(state.snapshot().has_live_entry());
        state.finish_external(id, "");
        assert!(!state.snapshot().has_live_entry());
    }

    // --- admit --------------------------------------------------------

    #[test]
    fn admit_on_idle_starts_downloading() {
        let mut state = QueueState::default();
        let admission = state.admit(JobKey::Rom(1), "Game", "SNES", "base");
        let Admission::Start(id) = admission else {
            panic!("expected Start, got {admission:?}");
        };
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Downloading);
        assert_eq!(entry.job, "game");
        assert_eq!(entry.rom_id, 1);
        assert_eq!(entry.source_id, "");
        assert_eq!(entry.title, "Game");
        assert_eq!(entry.platform, "SNES");
    }

    #[test]
    fn admit_emulator_on_idle_starts_downloading_with_job_and_source_id() {
        let mut state = QueueState::default();
        let admission = state.admit(
            JobKey::Emulator("retroarch/win-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );
        let Admission::Start(id) = admission else {
            panic!("expected Start, got {admission:?}");
        };
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Downloading);
        assert_eq!(entry.job, "emulator");
        assert_eq!(entry.rom_id, 0);
        assert_eq!(entry.source_id, "retroarch/win-x64");
        assert_eq!(entry.title, "RetroArch");
    }

    #[test]
    fn duplicate_emulator_same_source_id_is_rejected() {
        let mut state = QueueState::default();
        state.admit(
            JobKey::Emulator("retroarch/win-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );
        assert_eq!(
            state.admit(
                JobKey::Emulator("retroarch/win-x64".to_string()),
                "RetroArch",
                "",
                "base"
            ),
            Admission::Duplicate
        );
    }

    #[test]
    fn different_source_id_for_emulator_queues_independently() {
        let mut state = QueueState::default();
        state.admit(
            JobKey::Emulator("retroarch/win-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );
        let admission = state.admit(
            JobKey::Emulator("retroarch/linux-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );
        let Admission::Queued(id) = admission else {
            panic!("expected Queued, got {admission:?}");
        };
        assert_eq!(state.entry(id).unwrap().source_id, "retroarch/linux-x64");
    }

    #[test]
    fn rom_5_and_emulator_x_y_are_independent_keys() {
        let mut state = QueueState::default();
        let rom_admission = state.admit(JobKey::Rom(5), "Game", "SNES", "base");
        let Admission::Start(_) = rom_admission else {
            panic!("expected Start, got {rom_admission:?}");
        };
        // The download slot is now taken by the rom job, so an unrelated
        // emulator key must queue rather than being rejected as a
        // duplicate — the two `JobKey` variants never collide.
        let emu_admission = state.admit(JobKey::Emulator("x/y".to_string()), "Emu", "", "base");
        let Admission::Queued(id) = emu_admission else {
            panic!("expected Queued, got {emu_admission:?}");
        };
        assert_eq!(state.entry(id).unwrap().source_id, "x/y");
    }

    #[test]
    fn admit_while_busy_queues() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        let admission = state.admit(JobKey::Rom(2), "Other", "SNES", "base");
        let Admission::Queued(id) = admission else {
            panic!("expected Queued, got {admission:?}");
        };
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Queued);
    }

    #[test]
    fn duplicate_rom_id_active_is_rejected() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        assert_eq!(
            state.admit(JobKey::Rom(1), "Game", "SNES", "base"),
            Admission::Duplicate
        );
    }

    #[test]
    fn duplicate_rom_id_queued_is_rejected() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        state.admit(JobKey::Rom(2), "Other", "SNES", "base");
        assert_eq!(
            state.admit(JobKey::Rom(2), "Other", "SNES", "base"),
            Admission::Duplicate
        );
    }

    #[test]
    fn duplicate_rom_id_in_finalize_active_is_rejected() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);
        assert_eq!(
            state.admit(JobKey::Rom(1), "Game", "SNES", "base"),
            Admission::Duplicate
        );
    }

    // --- download_finished ---------------------------------------------

    #[test]
    fn download_ok_moves_to_installing_and_busies_finalize_slot() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Installing);
        assert_eq!(state.next_ready(), None); // finalize slot busy, download slot free but waiting is empty anyway
    }

    #[test]
    fn download_ok_frees_download_slot_so_next_ready_can_start_a_waiter() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let second = match state.admit(JobKey::Rom(2), "Other", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        state.download_finished(first, Ok(()), false);
        // finalize_active is busy with `first`, so next_ready must not fire yet.
        assert_eq!(state.next_ready(), None);
        assert_eq!(state.entry(second).unwrap().status, DownloadStatus::Queued);
    }

    #[test]
    fn download_err_cancelled_sets_cancelled_status_with_no_error_text() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.set_progress(id, 100, 200, 5.0);
        state.download_finished(id, Err(LibraryError::Cancelled), false);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Cancelled);
        assert_eq!(entry.error, "");
        assert_eq!(entry.speed_bps, 0.0);
    }

    #[test]
    fn download_err_other_sets_failed_with_display_text() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Err(LibraryError::NoLaunchFile), false);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, LibraryError::NoLaunchFile.to_string());
    }

    #[test]
    fn download_ok_skip_finalize_completes_directly_and_frees_both_slots() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        let waiter = match state.admit(JobKey::Rom(2), "Other", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        state.download_finished(id, Ok(()), true);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Completed);
        // Both slots are free now (finalize was never taken), so the
        // waiter should be pulled in.
        assert_eq!(state.next_ready(), Some(waiter));
        assert_eq!(
            state.entry(waiter).unwrap().status,
            DownloadStatus::Downloading
        );
    }

    #[test]
    fn stale_download_finished_on_a_completed_entry_is_a_no_op() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        state.download_finished(first, Ok(()), false);
        state.finalize_finished(first, Ok(()), "");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );

        // The download slot has moved on to a different entry.
        let second = admit_idle(&mut state, 2);
        assert_eq!(state.download_active, Some(second));

        // A stale completion for the old id must not resurrect it, steal
        // the finalize slot, or disturb the entry that now owns the
        // download slot.
        state.download_finished(first, Ok(()), false);
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.finalize_active, None);
        assert_eq!(state.download_active, Some(second));
    }

    // --- finalize_finished ------------------------------------------------

    #[test]
    fn finalize_ok_completes_and_next_ready_pops_fifo() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let second = match state.admit(JobKey::Rom(2), "Second", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        let third = match state.admit(JobKey::Rom(3), "Third", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };

        state.download_finished(first, Ok(()), false);
        // finalize slot busy: next_ready must not fire.
        assert_eq!(state.next_ready(), None);

        state.finalize_finished(first, Ok(()), "");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.entry(first).unwrap().error, "");

        // Both slots free now: FIFO order pulls `second` before `third`.
        assert_eq!(state.next_ready(), Some(second));
        assert_eq!(
            state.entry(second).unwrap().status,
            DownloadStatus::Downloading
        );
        assert_eq!(state.entry(third).unwrap().status, DownloadStatus::Queued);
    }

    #[test]
    fn finalize_ok_with_warning_stores_it_on_the_completed_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        state.finalize_finished(id, Ok(()), "could not delete archive");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Completed);
        assert_eq!(entry.error, "could not delete archive");
    }

    #[test]
    fn finalize_err_sets_failed_with_display_text() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        state.finalize_finished(id, Err(LibraryError::NoLaunchFile), "");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, LibraryError::NoLaunchFile.to_string());
        assert_eq!(entry.speed_bps, 0.0);
    }

    #[test]
    fn an_installer_failure_carries_its_parts_and_serializes() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        assert_eq!(state.entry(id).unwrap().phase, JobPhase::None);
        state.set_phase(id, JobPhase::InstallerSilent);
        state.download_finished(id, Ok(()), false);
        let error = LibraryError::InstallerDlcBaseMissing {
            base_title: "Pale Summit".into(),
            base_gog_id: "42".into(),
        };
        state.finalize_finished(id, Err(error), "");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(
            entry.error,
            "Its archive holds a DLC installer for Pale Summit, which isn't installed. \
             Install Pale Summit first; the download was removed."
        );
        let json = serde_json::to_value(entry).unwrap();
        assert_eq!(json["phase"], "installer_silent");
        assert_eq!(
            json["failure"],
            serde_json::json!({
                "code": "installer_dlc_base_missing",
                "base_title": "Pale Summit",
                "base_gog_id": "42",
                "kept": false
            })
        );

        let plain = admit_idle(&mut state, 2);
        state.download_finished(plain, Err(LibraryError::NoLaunchFile), false);
        let json = serde_json::to_value(state.entry(plain).unwrap()).unwrap();
        assert_eq!(json["phase"], "none");
        assert!(json["failure"].is_null());
    }

    #[test]
    fn installer_errors_use_the_design_words() {
        assert_eq!(
            LibraryError::InstallerUnrecognised {
                file: "setup.exe".into()
            }
            .to_string(),
            "Its archive holds an installer GRID doesn't recognise (setup.exe has no GOG game info). \
             The download was removed."
        );
        assert_eq!(
            LibraryError::InstallerExit(1).to_string(),
            "The installer stopped with an error (exit code 1). The download was removed."
        );
        assert_eq!(
            LibraryError::InstallerNeedsWizard { kept: true }.to_string(),
            "This installer needs its setup window. Install it in desktop mode. The download was kept."
        );
        assert_eq!(
            LibraryError::InstallerNeedsWizard { kept: false }.to_string(),
            "This installer needs its setup window. Install it in desktop mode."
        );
        assert_eq!(
            JobFailure::from_error(&LibraryError::InstallerNeedsWizard { kept: true }),
            Some(JobFailure {
                code: FailureCode::InstallerNeedsWizard,
                base_title: String::new(),
                base_gog_id: String::new(),
                kept: true
            })
        );
        assert_eq!(
            JobFailure::from_error(&LibraryError::InstallerRefused("no".into())),
            None
        );
    }

    #[test]
    fn store_ops_and_backup_rows_name_their_kind() {
        for (op, kind) in [
            (StoreOp::Repair, "store_repair"),
            (StoreOp::Dlc, "store_dlc"),
            (StoreOp::Installer, "store_installer"),
            (StoreOp::Restore, "store_restore"),
        ] {
            assert_eq!(op.kind(), kind);
        }
        let mut state = QueueState::default();
        let key = GameKey::store(StoreKind::Gog, "7");
        let id = admit_backup(&mut state, T7, BackupItemKey::Store(key), 1);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.kind, "backup_store_game");
        assert_eq!(entry.source_id, "gog:7");
        assert_eq!(entry.rom_id, 0);
    }

    #[test]
    fn finalize_err_appends_warning_after_newline() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        state.finalize_finished(id, Err(LibraryError::NoLaunchFile), "cleanup also failed");
        let entry = state.entry(id).unwrap();
        assert_eq!(
            entry.error,
            format!("{}\ncleanup also failed", LibraryError::NoLaunchFile)
        );
    }

    #[test]
    fn stale_finalize_finished_on_a_completed_entry_is_a_no_op() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        state.download_finished(first, Ok(()), false);
        state.finalize_finished(first, Ok(()), "");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.entry(first).unwrap().error, "");

        // A different entry now owns the finalize slot.
        let second = admit_idle(&mut state, 2);
        state.download_finished(second, Ok(()), false);
        assert_eq!(state.finalize_active, Some(second));

        // A stale finalize completion for the old id must not touch the
        // already-completed entry or the slot now owned by `second`.
        state.finalize_finished(first, Err(LibraryError::NoLaunchFile), "stale warning");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.entry(first).unwrap().error, "");
        assert_eq!(state.finalize_active, Some(second));
    }

    // --- request_cancel -----------------------------------------------

    #[test]
    fn cancel_queued_removes_from_waiting_with_exact_error() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        let waiter = match state.admit(JobKey::Rom(2), "Other", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(state.request_cancel(waiter), CancelAction::RemovedFromQueue);
        let entry = state.entry(waiter).unwrap();
        assert_eq!(entry.status, DownloadStatus::Cancelled);
        assert_eq!(entry.error, "Cancelled while queued");
    }

    #[test]
    fn cancel_active_download_marks_cancelling() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        assert_eq!(state.request_cancel(id), CancelAction::ActiveDownload);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Cancelling);
    }

    #[test]
    fn cancel_unknown_id_is_ignored() {
        let mut state = QueueState::default();
        assert_eq!(state.request_cancel(999), CancelAction::Ignored);
    }

    #[test]
    fn cancel_installing_entry_is_ignored() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert_eq!(state.request_cancel(id), CancelAction::Ignored);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);
    }

    // --- dismiss --------------------------------------------------------

    #[test]
    fn dismiss_removes_the_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Err(LibraryError::Cancelled), false);
        assert!(state.dismiss(id));
        assert!(state.entry(id).is_none());
    }

    #[test]
    fn dismiss_unknown_id_returns_false() {
        let mut state = QueueState::default();
        assert!(!state.dismiss(999));
    }

    #[test]
    fn dismiss_queued_entry_removes_it_from_waiting_so_it_never_starts() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let waiter = match state.admit(JobKey::Rom(2), "Other", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert!(state.dismiss(waiter));
        state.download_finished(first, Ok(()), true);
        // The dismissed id must never be picked up by next_ready.
        assert_eq!(state.next_ready(), None);
    }

    #[test]
    fn dismiss_refuses_the_active_download_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        assert!(!state.dismiss(id));
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Downloading);
        assert_eq!(state.download_active, Some(id));
    }

    #[test]
    fn dismiss_refuses_the_finalize_active_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert!(!state.dismiss(id));
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Installing);
        assert_eq!(state.finalize_active, Some(id));
    }

    // --- retryable --------------------------------------------------------

    #[test]
    fn retryable_only_for_failed_or_cancelled() {
        let mut state = QueueState::default();
        let failed = admit_idle(&mut state, 1);
        state.download_finished(failed, Err(LibraryError::NoLaunchFile), false);
        assert_eq!(state.retryable(failed), Some(JobKey::Rom(1)));

        let mut state2 = QueueState::default();
        let cancelled = admit_idle(&mut state2, 2);
        state2.download_finished(cancelled, Err(LibraryError::Cancelled), false);
        assert_eq!(state2.retryable(cancelled), Some(JobKey::Rom(2)));

        let mut state3 = QueueState::default();
        let active = admit_idle(&mut state3, 3);
        assert_eq!(state3.retryable(active), None);
    }

    #[test]
    fn retryable_unknown_id_is_none() {
        let state = QueueState::default();
        assert_eq!(state.retryable(999), None);
    }

    #[test]
    fn retryable_returns_emulator_key_for_a_failed_emulator_job() {
        let mut state = QueueState::default();
        let admission = state.admit(
            JobKey::Emulator("retroarch/win-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );
        let Admission::Start(id) = admission else {
            panic!("expected Start, got {admission:?}");
        };
        state.download_finished(id, Err(LibraryError::NoLaunchFile), false);
        assert_eq!(
            state.retryable(id),
            Some(JobKey::Emulator("retroarch/win-x64".to_string()))
        );
    }

    // --- snapshot --------------------------------------------------------

    #[test]
    fn snapshot_lists_entries_newest_first() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let second = match state.admit(JobKey::Rom(2), "Second", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        let third = match state.admit(JobKey::Rom(3), "Third", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };

        let ids: Vec<u64> = state.snapshot().entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![third, second, first]);
    }

    #[test]
    fn snapshot_exposes_job_and_source_id_for_both_kinds() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        state.admit(
            JobKey::Emulator("retroarch/win-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );

        let entries = state.snapshot().entries;
        let emulator_entry = entries.iter().find(|e| e.job == "emulator").unwrap();
        assert_eq!(emulator_entry.source_id, "retroarch/win-x64");
        assert_eq!(emulator_entry.rom_id, 0);
        let game_entry = entries.iter().find(|e| e.job == "game").unwrap();
        assert_eq!(game_entry.rom_id, 1);
        assert_eq!(game_entry.source_id, "");
    }

    #[test]
    fn snapshot_serializes_job_as_the_expected_string() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        state.admit(
            JobKey::Emulator("retroarch/win-x64".to_string()),
            "RetroArch",
            "",
            "base",
        );

        let json = serde_json::to_value(state.snapshot()).unwrap();
        let jobs: Vec<&str> = json["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["job"].as_str().unwrap())
            .collect();
        assert_eq!(jobs, vec!["emulator", "game"]);
    }

    // --- progress setters -------------------------------------------------

    #[test]
    fn set_progress_updates_fields_and_clamps_negative_speed() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.set_progress(id, 50, 100, -3.0);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.downloaded_bytes, 50);
        assert_eq!(entry.total_bytes, 100);
        assert_eq!(entry.speed_bps, 0.0);
    }

    #[test]
    fn set_progress_on_unknown_id_is_a_no_op() {
        let mut state = QueueState::default();
        state.set_progress(999, 1, 2, 3.0); // must not panic
    }

    #[test]
    fn set_install_progress_updates_fields() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.set_install_progress(id, 10, 20);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.install_processed_bytes, 10);
        assert_eq!(entry.install_total_bytes, 20);
    }

    #[test]
    fn set_install_progress_on_unknown_id_is_a_no_op() {
        let mut state = QueueState::default();
        state.set_install_progress(999, 1, 2); // must not panic
    }

    // --- ids ----------------------------------------------------------

    // --- kinds and external entries -----------------------------------

    #[test]
    fn admit_records_the_kind_verbatim() {
        let mut state = QueueState::default();
        let Admission::Start(id) = state.admit(JobKey::Rom(1), "Game", "SNES", "base") else {
            panic!("expected Start");
        };
        assert_eq!(state.entry(id).unwrap().kind, "base");
    }

    #[test]
    fn content_and_native_update_keys_report_the_game_job_and_rom_id() {
        let mut state = QueueState::default();
        let Admission::Start(update) = state.admit(
            JobKey::Content(7, ContentKind::Update),
            "Game",
            "PlayStation 4",
            "ps4_content",
        ) else {
            panic!("expected Start");
        };
        let entry = state.entry(update).unwrap();
        assert_eq!(entry.job, "game");
        assert_eq!(entry.rom_id, 7);
        assert_eq!(entry.kind, "ps4_content");

        // A DLC for the same rom is a different key, so it queues rather
        // than deduping against the update.
        assert!(matches!(
            state.admit(
                JobKey::Content(7, ContentKind::Dlc),
                "Game",
                "PlayStation 4",
                "ps4_content",
            ),
            Admission::Queued(_)
        ));
        assert!(matches!(
            state.admit(JobKey::NativeUpdate(7), "Game", "Windows", "native_update"),
            Admission::Queued(_)
        ));
    }

    #[test]
    fn admit_external_creates_a_downloading_row_that_never_becomes_ready() {
        let mut state = QueueState::default();
        let id = state.admit_external("PS3 Firmware", "PlayStation 3");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Downloading);
        assert_eq!(entry.job, "firmware");
        assert_eq!(entry.kind, "firmware");
        assert_eq!(entry.rom_id, 0);
        assert_eq!(entry.title, "PS3 Firmware");

        // It took no slot, so a real install still starts immediately...
        let Admission::Start(rom) = state.admit(JobKey::Rom(1), "Game", "SNES", "base") else {
            panic!("expected Start");
        };
        assert_ne!(rom, id);
        // ...and the external row is never handed out as ready work.
        assert_eq!(state.next_ready(), None);
    }

    #[test]
    fn two_external_entries_never_dedupe_against_each_other() {
        let mut state = QueueState::default();
        let first = state.admit_external("Firmware", "PlayStation 3");
        let second = state.admit_external("Firmware", "PlayStation 3");
        assert_ne!(first, second);
        assert_eq!(state.snapshot().entries.len(), 2);
    }

    #[test]
    fn finish_external_completes_on_a_blank_error_and_fails_otherwise() {
        let mut state = QueueState::default();
        let ok = state.admit_external("Firmware", "PlayStation 3");
        state.finish_external(ok, "");
        let entry = state.entry(ok).unwrap();
        assert_eq!(entry.status, DownloadStatus::Completed);
        assert_eq!(entry.error, "");

        let bad = state.admit_external("Firmware", "PlayStation 3");
        state.finish_external(bad, "server said no");
        let entry = state.entry(bad).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, "server said no");
    }

    #[test]
    fn finish_external_ignores_an_unknown_id_and_a_real_install() {
        let mut state = QueueState::default();
        let rom = admit_idle(&mut state, 1);
        state.finish_external(rom, "boom");
        state.finish_external(9999, "boom");
        assert_eq!(
            state.entry(rom).unwrap().status,
            DownloadStatus::Downloading
        );
        assert_eq!(state.entry(rom).unwrap().error, "");
    }

    #[test]
    fn first_live_for_rom_finds_the_oldest_unfinished_entry_only() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 5);
        let Admission::Queued(second) = state.admit(JobKey::Rom(6), "Other", "SNES", "base") else {
            panic!("expected Queued");
        };
        assert_eq!(state.first_live_for_rom(5), Some(first));
        assert_eq!(state.first_live_for_rom(6), Some(second));
        assert_eq!(state.first_live_for_rom(404), None);

        state.download_finished(first, Ok(()), true);
        assert_eq!(
            state.first_live_for_rom(5),
            None,
            "a completed entry is not live"
        );
    }

    #[test]
    fn snapshot_serializes_the_kind_field() {
        let mut state = QueueState::default();
        state.admit(JobKey::Rom(1), "Game", "SNES", "base");
        let json = serde_json::to_value(state.snapshot()).unwrap();
        assert_eq!(json["entries"][0]["kind"], "base");
        assert_eq!(json["entries"][0]["job"], "game");
    }

    #[test]
    fn ids_start_at_one_and_increment_never_reused() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        assert_eq!(first, 1);
        let second = match state.admit(JobKey::Rom(2), "Other", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(second, 2);
        state.dismiss(second);
        let third = match state.admit(JobKey::Rom(3), "Third", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(third, 3);
    }

    // --- backups ----------------------------------------------------------

    const T7: &str = "/run/media/samm/SAMSUNG T7";
    const SANDISK: &str = "/run/media/samm/SANDISK";

    fn meta(drive: &str, batch: u64) -> EntryMeta {
        EntryMeta {
            drive_id: drive.to_string(),
            drive_name: "Couch kit".to_string(),
            drive_label: drive.rsplit('/').next().unwrap().to_string(),
            batch,
        }
    }

    fn backup_key(drive: &str, item: BackupItemKey) -> JobKey {
        JobKey::Backup {
            drive: drive.to_string(),
            item,
        }
    }

    fn admit_backup(state: &mut QueueState, drive: &str, item: BackupItemKey, batch: u64) -> u64 {
        let kind = match item {
            BackupItemKey::Game(_) => "backup_game",
            BackupItemKey::Emulator { .. } => "backup_emulator",
            BackupItemKey::Firmware(_) => "backup_firmware",
            BackupItemKey::Store(_) => "backup_store_game",
        };
        match state.admit_with(
            backup_key(drive, item),
            "Item",
            "Platform",
            kind,
            meta(drive, batch),
        ) {
            Admission::Start(id) | Admission::Queued(id) => id,
            Admission::Duplicate => panic!("unexpected duplicate"),
        }
    }

    #[test]
    fn a_backup_row_carries_its_drive_and_batch_and_job_backup() {
        let mut state = QueueState::default();
        let id = admit_backup(&mut state, T7, BackupItemKey::Game(42), 7);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.job, "backup");
        assert_eq!(entry.kind, "backup_game");
        assert_eq!(entry.rom_id, 42);
        assert_eq!(entry.drive_id, T7);
        assert_eq!(entry.drive_name, "Couch kit");
        assert_eq!(entry.drive_label, "SAMSUNG T7");
        assert_eq!(entry.batch, 7);

        let emu = admit_backup(
            &mut state,
            T7,
            BackupItemKey::Emulator {
                name: "DuckStation".into(),
                os: PackOs::Windows,
            },
            7,
        );
        assert_eq!(state.entry(emu).unwrap().rom_id, 0);
        assert_eq!(state.entry(emu).unwrap().job, "backup");

        // A plain admit leaves every drive field blank.
        let rom = match state.admit(JobKey::Rom(1), "Game", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        let entry = state.entry(rom).unwrap();
        assert_eq!(entry.drive_id, "");
        assert_eq!(entry.drive_label, "");
        assert_eq!(entry.batch, 0);

        // The serde output carries the new fields.
        let json = serde_json::to_value(state.entry(id).unwrap()).unwrap();
        assert_eq!(json["job"], "backup");
        assert_eq!(json["drive_id"], T7);
        assert_eq!(json["drive_name"], "Couch kit");
        assert_eq!(json["drive_label"], "SAMSUNG T7");
        assert_eq!(json["batch"], 7);
    }

    #[test]
    fn a_backup_row_is_not_a_live_install_of_its_game() {
        let mut state = QueueState::default();
        admit_backup(&mut state, T7, BackupItemKey::Game(42), 1);
        assert_eq!(state.first_live_for_rom(42), None);
    }

    #[test]
    fn backup_rows_dedupe_per_drive_and_item() {
        let mut state = QueueState::default();
        admit_backup(&mut state, T7, BackupItemKey::Game(42), 1);
        let again = state.admit_with(
            backup_key(T7, BackupItemKey::Game(42)),
            "Item",
            "Platform",
            "backup_game",
            meta(T7, 2),
        );
        assert_eq!(again, Admission::Duplicate);

        // The same game for another drive, an install of the game, and
        // another OS of the same emulator are all different jobs.
        assert!(matches!(
            state.admit_with(
                backup_key(SANDISK, BackupItemKey::Game(42)),
                "Item",
                "Platform",
                "backup_game",
                meta(SANDISK, 3),
            ),
            Admission::Queued(_)
        ));
        assert!(matches!(
            state.admit(JobKey::Rom(42), "Game", "SNES", "base"),
            Admission::Queued(_)
        ));
        let linux = BackupItemKey::Emulator {
            name: "DuckStation".into(),
            os: PackOs::Linux,
        };
        let windows = BackupItemKey::Emulator {
            name: "DuckStation".into(),
            os: PackOs::Windows,
        };
        admit_backup(&mut state, T7, linux.clone(), 1);
        admit_backup(&mut state, T7, windows, 1);
        assert_eq!(
            state.admit_with(
                backup_key(T7, linux),
                "Item",
                "Platform",
                "backup_emulator",
                meta(T7, 1),
            ),
            Admission::Duplicate
        );
        admit_backup(&mut state, T7, BackupItemKey::Firmware("psx".into()), 1);
        assert_eq!(
            state.admit_with(
                backup_key(T7, BackupItemKey::Firmware("psx".into())),
                "Item",
                "Platform",
                "backup_firmware",
                meta(T7, 1),
            ),
            Admission::Duplicate
        );
    }

    #[test]
    fn set_drive_meta_tags_an_install_from_drive_row() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 5);
        state.set_drive_meta(id, meta(T7, 0));
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.job, "game");
        assert_eq!(entry.drive_id, T7);
        assert_eq!(entry.drive_label, "SAMSUNG T7");
        state.set_drive_meta(999, meta(T7, 0));
    }

    #[test]
    fn cancel_drive_queue_collapses_that_drives_waiting_rows_into_one_summary() {
        let mut state = QueueState::default();
        // The slot owner is a backup row for the removed drive.
        let active = admit_backup(&mut state, T7, BackupItemKey::Game(1), 4);
        let waiting_backup = admit_backup(&mut state, T7, BackupItemKey::Game(2), 4);
        let other_drive = admit_backup(&mut state, SANDISK, BackupItemKey::Game(3), 5);
        let plain = match state.admit(JobKey::Rom(9), "Game", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        // An install-from-drive row for the removed drive.
        let from_drive = match state.admit(JobKey::Rom(10), "Game", "SNES", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        state.set_drive_meta(from_drive, meta(T7, 0));
        let firmware = admit_backup(&mut state, T7, BackupItemKey::Firmware("psx".into()), 4);

        let removed = state.cancel_drive_queue(T7, "SAMSUNG T7 was removed");
        assert_eq!(removed, vec![waiting_backup, from_drive, firmware]);
        for id in &removed {
            assert!(state.entry(*id).is_none(), "{id} left the list");
        }

        // The slot owner and the other rows are untouched.
        assert_eq!(
            state.entry(active).unwrap().status,
            DownloadStatus::Downloading
        );
        assert_eq!(
            state.entry(other_drive).unwrap().status,
            DownloadStatus::Queued
        );
        assert_eq!(state.entry(plain).unwrap().status, DownloadStatus::Queued);

        let summaries: Vec<DownloadEntry> = state
            .snapshot()
            .entries
            .into_iter()
            .filter(|entry| entry.kind == "backup_cancelled")
            .collect();
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.job, "backup");
        assert_eq!(summary.status, DownloadStatus::Cancelled);
        assert_eq!(summary.title, "3 more items");
        assert_eq!(summary.error, "SAMSUNG T7 was removed");
        assert_eq!(summary.batch, 4, "the first removed row's batch");
        assert_eq!(summary.drive_id, T7);
        assert_eq!(summary.drive_label, "SAMSUNG T7");
        assert_eq!(state.retryable(summary.id), None, "Dismiss only");

        // The queue moves on past the removed rows, in order.
        state.download_finished(
            active,
            Err(LibraryError::DriveRemoved("SAMSUNG T7 was removed".into())),
            false,
        );
        assert_eq!(state.next_ready(), Some(other_drive));

        // A second call for the same drive finds nothing and adds no row.
        assert!(state.cancel_drive_queue(T7, "again").is_empty());
        assert!(state.cancel_drive_queue("", "blank").is_empty());
        assert_eq!(
            state
                .snapshot()
                .entries
                .iter()
                .filter(|entry| entry.kind == "backup_cancelled")
                .count(),
            1
        );
    }

    #[test]
    fn cancel_drive_queue_names_a_single_removed_item() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        let id = match state.admit_with(
            backup_key(T7, BackupItemKey::Game(2)),
            "Aladdin",
            "Sega Genesis",
            "backup_game",
            meta(T7, 3),
        ) {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(state.cancel_drive_queue(T7, "gone"), vec![id]);
        let summary = state.snapshot().entries[0].clone();
        assert_eq!(summary.kind, "backup_cancelled");
        assert_eq!(summary.title, "Aladdin");
        assert_eq!(summary.platform, "Sega Genesis");
    }

    #[test]
    fn a_backup_write_can_be_cancelled_and_ends_cancelled() {
        let mut state = QueueState::default();
        let id = admit_backup(&mut state, T7, BackupItemKey::Game(1), 1);
        state.download_finished(id, Ok(()), false);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);

        assert_eq!(state.request_cancel(id), CancelAction::ActiveWrite);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Cancelling);
        assert!(state.snapshot().has_live_entry());

        state.finalize_finished(id, Err(LibraryError::Cancelled), "");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Cancelled);
        assert_eq!(entry.error, "");
        assert_eq!(
            state.retryable(id),
            Some(backup_key(T7, BackupItemKey::Game(1)))
        );
    }

    #[test]
    fn a_non_backup_finalize_stays_uncancellable() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert_eq!(state.request_cancel(id), CancelAction::Ignored);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);
    }

    #[test]
    fn a_removed_drive_fails_the_row_with_its_text() {
        let removed = || LibraryError::DriveRemoved("SAMSUNG T7 was removed".into());
        let mut state = QueueState::default();

        // While downloading into staging.
        let downloading = admit_backup(&mut state, T7, BackupItemKey::Game(1), 1);
        state.download_finished(downloading, Err(removed()), false);
        let entry = state.entry(downloading).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, "SAMSUNG T7 was removed");

        // While writing to the drive.
        let writing = admit_backup(&mut state, T7, BackupItemKey::Game(2), 1);
        state.download_finished(writing, Ok(()), false);
        state.finalize_finished(writing, Err(removed()), "");
        let entry = state.entry(writing).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, "SAMSUNG T7 was removed");
        assert!(state.retryable(writing).is_some());
    }

    #[test]
    fn a_drive_summary_row_counts_toward_the_history_cap() {
        let mut state = QueueState::default();
        for rom in 1..=(TERMINAL_HISTORY as i64) {
            let id = admit_idle(&mut state, rom);
            state.download_finished(id, Ok(()), true);
        }
        admit_idle(&mut state, 1000);
        admit_backup(&mut state, T7, BackupItemKey::Game(1001), 1);
        state.cancel_drive_queue(T7, "gone");
        assert!(
            state.entry(1).is_none(),
            "the oldest terminal row was pruned"
        );
        let terminal = state
            .snapshot()
            .entries
            .iter()
            .filter(|entry| !entry.status.is_live())
            .count();
        assert_eq!(terminal, TERMINAL_HISTORY);
    }

    // --- terminal history cap (design §8: "Completed keeps the last 50") --

    #[test]
    fn terminal_entries_are_capped_at_the_history_limit_oldest_first() {
        let mut state = QueueState::default();
        for rom in 1..=(TERMINAL_HISTORY as i64 + 5) {
            let id = admit_idle(&mut state, rom);
            state.download_finished(id, Ok(()), true);
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.entries.len(), TERMINAL_HISTORY);
        // Ids are allocated in admission order, so the five oldest are 1..=5.
        assert!(snapshot.entries.iter().all(|entry| entry.id > 5));
        // Newest first, untouched.
        assert_eq!(snapshot.entries[0].id, TERMINAL_HISTORY as u64 + 5);
    }

    #[test]
    fn every_terminal_transition_prunes_including_cancel_and_external() {
        let mut state = QueueState::default();
        // Fill the history with completed installs.
        for rom in 1..=(TERMINAL_HISTORY as i64) {
            let id = admit_idle(&mut state, rom);
            state.download_finished(id, Ok(()), true);
        }
        // A cancelled finalize is terminal and evicts id 1.
        let failed = admit_idle(&mut state, 1000);
        state.download_finished(failed, Ok(()), false);
        state.finalize_finished(failed, Err(LibraryError::Cancelled), "");
        assert!(state.entry(1).is_none());
        assert_eq!(state.snapshot().entries.len(), TERMINAL_HISTORY);

        // A queued entry cancelled out of the queue is terminal and evicts id 2.
        let live = admit_idle(&mut state, 1001);
        let queued = match state.admit(JobKey::Rom(1002), "Title", "Platform", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(state.request_cancel(queued), CancelAction::RemovedFromQueue);
        assert!(state.entry(2).is_none());

        // An external (firmware) row finishing is terminal and evicts id 3.
        let external = state.admit_external("PS3 Firmware", "PS3");
        state.finish_external(external, "");
        assert!(state.entry(3).is_none());

        // The live download was never touched and still owns its slot.
        assert_eq!(
            state.entry(live).unwrap().status,
            DownloadStatus::Downloading
        );
        assert_eq!(state.next_ready(), None);
        let terminal = state
            .snapshot()
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                )
            })
            .count();
        assert_eq!(terminal, TERMINAL_HISTORY);
    }
}
