//! The space ledger: the disk space promised to queued and running work,
//! so every "does it fit" check sees what is really left.
//!
//! The free figure a volume reports already counts every byte written so
//! far. What it does not count is what queued and running jobs are still
//! going to write. Each job therefore holds a [`Reservation`] per
//! filesystem it adds bytes to, and reports its progress as it writes:
//!
//! - **outstanding** = reserved − written so far, never below 0;
//! - **available** = free − (every other job's outstanding) − [`SPACE_MARGIN`].
//!
//! A reservation is released when its guard drops: when the job finishes,
//! fails or is cancelled, and on a panic or an early return too.
//! Reservations are keyed by filesystem ([`FsKey`]): the device id on unix,
//! the volume root on Windows. The ledger lives in memory only;
//! [`super::InstallService`] owns it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub use crate::backup::write::SPACE_MARGIN;

/// The filesystem a path lives on.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FsKey {
    /// `st_dev` of the path (unix).
    Device(u64),
    /// The volume root (`GetVolumePathNameW`, or the path's root).
    Volume(String),
}

/// The filesystem of `path`, read from its nearest existing ancestor (a
/// folder a job is about to create does not exist yet). `None` when no
/// ancestor can be read. Symlinks are followed, so a platform folder linked
/// to another drive keys as that drive.
pub fn fs_key(path: &Path) -> Option<FsKey> {
    let existing = path.ancestors().find(|dir| dir.exists())?;
    platform_key(existing)
}

#[cfg(unix)]
fn platform_key(path: &Path) -> Option<FsKey> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|meta| FsKey::Device(meta.dev()))
}

#[cfg(windows)]
fn platform_key(path: &Path) -> Option<FsKey> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 1024];
    // SAFETY: `wide` is NUL-terminated and outlives the call; `buffer` is a
    // valid out-buffer of the length passed.
    let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    if ok != 0 {
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        let root = std::ffi::OsString::from_wide(&buffer[..len]);
        return Some(FsKey::Volume(root.to_string_lossy().to_lowercase()));
    }
    root_key(path)
}

#[cfg(not(any(unix, windows)))]
fn platform_key(path: &Path) -> Option<FsKey> {
    root_key(path)
}

/// The path's root component as the key: the fallback when the volume
/// cannot be asked.
#[cfg(not(unix))]
fn root_key(path: &Path) -> Option<FsKey> {
    let root: PathBuf = path.components().take(2).collect();
    Some(FsKey::Volume(root.to_string_lossy().to_lowercase()))
}

/// Free bytes for the volume holding a path.
pub type FreeProbe = dyn Fn(&Path) -> Option<u64> + Send + Sync;
/// The filesystem a path lives on.
pub type KeyProbe = dyn Fn(&Path) -> Option<FsKey> + Send + Sync;

/// How the service reads a volume: its free space and its key. The system
/// probe reads the disk; tests inject their own.
#[derive(Clone)]
pub struct SpaceProbe {
    pub free: Arc<FreeProbe>,
    pub key: Arc<KeyProbe>,
}

impl SpaceProbe {
    /// [`super::relocate::free_space`] and [`fs_key`].
    pub fn system() -> Self {
        Self {
            free: Arc::new(super::relocate::free_space),
            key: Arc::new(fs_key),
        }
    }
}

impl std::fmt::Debug for SpaceProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpaceProbe")
    }
}

/// A check that did not fit: what was needed and what was available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortfall {
    pub needed: u64,
    pub available: u64,
}

/// One reservation [`SpaceLedger::try_reserve`] makes: `bytes` on the
/// filesystem `key`, whose volume reports `free` bytes free.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub key: FsKey,
    pub bytes: u64,
    pub free: u64,
}

#[derive(Default)]
struct State {
    next: u64,
    entries: HashMap<u64, Entry>,
}

struct Entry {
    key: FsKey,
    reserved: u64,
    written: u64,
}

impl Entry {
    fn outstanding(&self) -> u64 {
        self.reserved.saturating_sub(self.written)
    }
}

impl State {
    fn outstanding(&self, key: &FsKey, except: Option<u64>) -> u64 {
        self.entries
            .iter()
            .filter(|(id, entry)| &entry.key == key && Some(**id) != except)
            .fold(0u64, |sum, (_, entry)| {
                sum.saturating_add(entry.outstanding())
            })
    }

    fn available(&self, key: &FsKey, free: u64, except: Option<u64>) -> u64 {
        free.saturating_sub(self.outstanding(key, except))
            .saturating_sub(SPACE_MARGIN)
    }

    fn insert(&mut self, key: FsKey, bytes: u64) -> u64 {
        self.next += 1;
        self.entries.insert(
            self.next,
            Entry {
                key,
                reserved: bytes,
                written: 0,
            },
        );
        self.next
    }
}

/// The space promised to queued and running jobs. Cheap to clone: every
/// clone is the same ledger.
#[derive(Clone, Default)]
pub struct SpaceLedger {
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for SpaceLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpaceLedger")
    }
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    // A panic while the lock was held left plain numbers behind: still
    // usable.
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl SpaceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every reservation's outstanding bytes on `key`.
    pub fn outstanding(&self, key: &FsKey) -> u64 {
        lock(&self.state).outstanding(key, None)
    }

    /// Every reservation's outstanding bytes on the filesystem of `path`
    /// ([`fs_key`]).
    pub fn outstanding_at(&self, path: &Path) -> u64 {
        fs_key(path).map_or(0, |key| self.outstanding(&key))
    }

    /// What is left on `key` for new work: `free` less every outstanding
    /// reservation and the margin.
    pub fn available(&self, key: &FsKey, free: u64) -> u64 {
        lock(&self.state).available(key, free, None)
    }

    /// Checks every request and reserves them all, or none, under one lock,
    /// so two quick starts can never both pass. Requests on the same
    /// filesystem are checked together (their sum against the smallest
    /// `free` given for it). A zero-byte request always fits.
    ///
    /// `Err` names the first request (by index) whose filesystem does not
    /// have room, and the shortfall.
    pub fn try_reserve(
        &self,
        requests: &[Request],
    ) -> Result<Vec<Reservation>, (usize, Shortfall)> {
        let mut state = lock(&self.state);
        for (index, request) in requests.iter().enumerate() {
            let same = requests.iter().filter(|other| other.key == request.key);
            let needed = same
                .clone()
                .fold(0u64, |sum, other| sum.saturating_add(other.bytes));
            if needed == 0 {
                continue;
            }
            let free = same.map(|other| other.free).min().unwrap_or(request.free);
            let available = state.available(&request.key, free, None);
            if needed > available {
                return Err((index, Shortfall { needed, available }));
            }
        }
        Ok(requests
            .iter()
            .map(|request| Reservation {
                state: self.state.clone(),
                id: state.insert(request.key.clone(), request.bytes),
            })
            .collect())
    }

    /// Reserves `bytes` on `key` without a check: for work whose size, or
    /// whose volume, cannot be measured yet.
    pub fn reserve(&self, key: FsKey, bytes: u64) -> Reservation {
        let mut state = lock(&self.state);
        Reservation {
            state: self.state.clone(),
            id: state.insert(key, bytes),
        }
    }

    /// The number of live reservations (tests).
    pub fn len(&self) -> usize {
        lock(&self.state).entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One job's claim on one filesystem. Released when dropped.
pub struct Reservation {
    state: Arc<Mutex<State>>,
    id: u64,
}

impl std::fmt::Debug for Reservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reservation")
            .field("reserved", &self.reserved())
            .field("written", &self.written())
            .finish()
    }
}

impl Reservation {
    fn with<R>(&self, f: impl FnOnce(&mut Entry) -> R) -> Option<R> {
        lock(&self.state).entries.get_mut(&self.id).map(f)
    }

    /// The filesystem this reservation is on.
    pub fn key(&self) -> Option<FsKey> {
        self.with(|entry| entry.key.clone())
    }

    pub fn reserved(&self) -> u64 {
        self.with(|entry| entry.reserved).unwrap_or(0)
    }

    pub fn written(&self) -> u64 {
        self.with(|entry| entry.written).unwrap_or(0)
    }

    /// reserved − written, never below 0.
    pub fn outstanding(&self) -> u64 {
        self.with(|entry| entry.outstanding()).unwrap_or(0)
    }

    /// Records that the job has now written `bytes` in all to this
    /// filesystem.
    pub fn set_written(&self, bytes: u64) {
        self.with(|entry| entry.written = bytes);
    }

    /// Adds `bytes` to what the job has written.
    pub fn add_written(&self, bytes: u64) {
        self.with(|entry| entry.written = entry.written.saturating_add(bytes));
    }

    /// Gives the whole reservation back early (a backup's staging, once it
    /// is deleted). The guard stays, holding nothing.
    pub fn release(&self) {
        self.with(|entry| {
            entry.reserved = 0;
            entry.written = 0;
        });
    }

    /// Changes the reservation to `reserved` bytes in all. Shrinking
    /// always succeeds; growing is checked against `free` less every other
    /// reservation on the filesystem and the margin, under the ledger's
    /// lock.
    pub fn try_resize(&self, reserved: u64, free: u64) -> Result<(), Shortfall> {
        let mut state = lock(&self.state);
        let Some(entry) = state.entries.get(&self.id) else {
            return Ok(());
        };
        let key = entry.key.clone();
        let before = entry.outstanding();
        let after = reserved.saturating_sub(entry.written);
        if after > before {
            let available = state.available(&key, free, Some(self.id));
            if after > available {
                return Err(Shortfall {
                    needed: after,
                    available,
                });
            }
        }
        if let Some(entry) = state.entries.get_mut(&self.id) {
            entry.reserved = reserved;
        }
        Ok(())
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        lock(&self.state).entries.remove(&self.id);
    }
}

/// A byte count for people, the way the app's Downloads rows write it:
/// binary steps, no decimals for bytes, one past them (`1.2 GB`).
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0usize;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size:.0} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// The install refusal: "There isn't enough free space in the library
/// folder for <Game> (needs X, Y available)".
pub fn library_shortfall_message(title: &str, shortfall: Shortfall) -> String {
    format!(
        "There isn't enough free space in the library folder for {title} (needs {}, {} available)",
        format_size(shortfall.needed),
        format_size(shortfall.available)
    )
}

/// The backup refusal: "<label> doesn't have enough free space (needs X,
/// Y available)".
pub fn drive_shortfall_message(label: &str, shortfall: Shortfall) -> String {
    format!(
        "{label} doesn't have enough free space (needs {}, {} available)",
        format_size(shortfall.needed),
        format_size(shortfall.available)
    )
}

// --- one job's claims ---------------------------------------------------------

/// What an install expects to add to the library folder at its peak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estimate {
    /// The download's size: the sum of the server's file sizes, `None` when
    /// any is missing (the response's Content-Length stands in once it
    /// arrives).
    pub download: Option<u64>,
    /// Bytes already on disk at the download's destinations: a file the
    /// download skips because it is complete, or one it truncates and
    /// replaces (an update). They need no new space, and the download's
    /// progress counts them.
    pub existing: u64,
    /// The download is extracted next to itself: the extracted tree is
    /// estimated at the archive's size.
    pub extracts: bool,
}

impl Estimate {
    /// What the download adds: its size less what is already there. An
    /// unknown download counts as 0.
    pub fn download_need(&self) -> u64 {
        self.download.unwrap_or(0).saturating_sub(self.existing)
    }

    /// Download plus extraction.
    pub fn peak(&self) -> u64 {
        let extract = if self.extracts {
            self.download.unwrap_or(0)
        } else {
            0
        };
        self.download_need().saturating_add(extract)
    }
}

/// How far a job has got.
#[derive(Default)]
struct Tally {
    /// Bytes the download phase has written (cumulative).
    downloaded: u64,
    /// Finalize progress before the current pass (extraction can run once
    /// per archive, each pass counting from 0).
    finalized_base: u64,
    /// The current finalize pass's progress.
    finalized_last: u64,
    estimate: Option<Estimate>,
    /// Set when the job no longer fits: the message it fails with.
    shortfall: Option<String>,
}

impl Tally {
    fn finalized(&self) -> u64 {
        self.finalized_base.saturating_add(self.finalized_last)
    }
}

/// Every reservation one queue job holds, and the progress that lowers
/// them. Rides on the job's payload, so it is released with the payload
/// whatever way the job ends.
pub struct JobSpace {
    /// The library folder: an install's download plus extraction, or a
    /// backup's staging.
    library: Option<Reservation>,
    /// A backup's drive: the bytes it writes there.
    drive: Option<Reservation>,
    /// A backup credits its finalize (write) progress to `drive`, and lets
    /// go of `library` once staging is deleted.
    backup: bool,
    /// Where the install writes, for the free-space reads a later resize
    /// needs.
    at: PathBuf,
    title: String,
    probe: SpaceProbe,
    tally: Mutex<Tally>,
}

impl std::fmt::Debug for JobSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobSpace")
            .field("library", &self.library)
            .field("drive", &self.drive)
            .finish()
    }
}

impl JobSpace {
    /// An install's claim: `library` holds its estimated peak on the
    /// library folder at `at`.
    pub fn install(
        library: Option<Reservation>,
        estimate: Estimate,
        at: PathBuf,
        title: &str,
        probe: SpaceProbe,
    ) -> Self {
        Self {
            library,
            drive: None,
            backup: false,
            at,
            title: title.to_string(),
            probe,
            tally: Mutex::new(Tally {
                estimate: Some(estimate),
                ..Tally::default()
            }),
        }
    }

    /// A backup row's claim: `staging` in the library folder, `drive` on
    /// the drive.
    pub fn backup(
        staging: Option<Reservation>,
        drive: Option<Reservation>,
        title: &str,
        probe: SpaceProbe,
    ) -> Self {
        Self {
            library: staging,
            drive,
            backup: true,
            at: PathBuf::new(),
            title: title.to_string(),
            probe,
            tally: Mutex::new(Tally::default()),
        }
    }

    fn tally(&self) -> MutexGuard<'_, Tally> {
        self.tally
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The library-folder reservation's outstanding bytes.
    pub fn library_outstanding(&self) -> u64 {
        self.library.as_ref().map_or(0, Reservation::outstanding)
    }

    /// The drive reservation's outstanding bytes.
    pub fn drive_outstanding(&self) -> u64 {
        self.drive.as_ref().map_or(0, Reservation::outstanding)
    }

    /// The library-folder reservation's size.
    pub fn library_reserved(&self) -> u64 {
        self.library.as_ref().map_or(0, Reservation::reserved)
    }

    /// The message the job fails with when it no longer fits.
    pub fn shortfall(&self) -> Option<String> {
        self.tally().shortfall.clone()
    }

    /// Download progress: `downloaded` bytes written so far, of `total`
    /// (0 when unknown). An install whose size was unknown at admission
    /// grows its reservation to the response's size the first time one is
    /// known. Returns `true` when the job no longer fits and must stop.
    pub fn on_download(&self, downloaded: u64, total: u64) -> bool {
        let mut tally = self.tally();
        tally.downloaded = downloaded;
        if self.backup {
            if let Some(staging) = &self.library {
                staging.set_written(downloaded);
            }
            return false;
        }
        let Some(library) = &self.library else {
            return false;
        };
        let existing = tally.estimate.map_or(0, |estimate| estimate.existing);
        library.set_written(
            downloaded
                .saturating_sub(existing)
                .saturating_add(tally.finalized()),
        );
        let grow = match &mut tally.estimate {
            Some(estimate) if estimate.download.is_none() && total > 0 => {
                estimate.download = Some(total);
                Some(estimate.peak())
            }
            _ => None,
        };
        let Some(peak) = grow else {
            return false;
        };
        let free = (self.probe.free)(&self.at).unwrap_or(u64::MAX);
        match library.try_resize(peak, free) {
            Ok(()) => false,
            Err(shortfall) => {
                tally.shortfall = Some(library_shortfall_message(&self.title, shortfall));
                true
            }
        }
    }

    /// Finalize progress: extraction for an install, the drive write for a
    /// backup. `processed` counts from 0 for each pass (each archive); a
    /// smaller number than last time starts a new pass.
    pub fn on_finalize(&self, processed: u64) {
        let mut tally = self.tally();
        if processed < tally.finalized_last {
            tally.finalized_base = tally.finalized_base.saturating_add(tally.finalized_last);
        }
        tally.finalized_last = processed;
        let finalized = tally.finalized();
        if self.backup {
            if let Some(drive) = &self.drive {
                drive.set_written(finalized);
            }
        } else if let Some(library) = &self.library {
            let existing = tally.estimate.map_or(0, |estimate| estimate.existing);
            library.set_written(
                tally
                    .downloaded
                    .saturating_sub(existing)
                    .saturating_add(finalized),
            );
        }
    }

    /// A backup's staging folder was deleted: its library-folder share goes
    /// back.
    pub fn release_staging(&self) {
        if self.backup {
            if let Some(staging) = &self.library {
                staging.release();
            }
        }
    }

    /// The archive is down and its real uncompressed size is known: the
    /// reservation becomes the download plus `extracted`. `Err` is the
    /// message the job fails with when that no longer fits.
    pub fn resize_for_extract(&self, extracted: u64) -> Result<(), String> {
        let mut tally = self.tally();
        let Some(library) = &self.library else {
            return Ok(());
        };
        let existing = tally.estimate.map_or(0, |estimate| estimate.existing);
        let download = tally
            .estimate
            .and_then(|estimate| estimate.download)
            .unwrap_or(0)
            .max(tally.downloaded)
            .saturating_sub(existing);
        let free = (self.probe.free)(&self.at).unwrap_or(u64::MAX);
        match library.try_resize(download.saturating_add(extracted), free) {
            Ok(()) => Ok(()),
            Err(shortfall) => {
                let message = library_shortfall_message(&self.title, shortfall);
                tally.shortfall = Some(message.clone());
                Err(message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    const MB: u64 = 1024 * 1024;

    fn dev(n: u64) -> FsKey {
        FsKey::Device(n)
    }

    fn one(key: FsKey, bytes: u64, free: u64) -> Request {
        Request { key, bytes, free }
    }

    #[test]
    fn a_reservation_counts_until_it_is_dropped() {
        let ledger = SpaceLedger::new();
        let free = SPACE_MARGIN + 100 * MB;
        assert_eq!(ledger.available(&dev(1), free), 100 * MB);
        let held = ledger.reserve(dev(1), 30 * MB);
        assert_eq!(ledger.outstanding(&dev(1)), 30 * MB);
        assert_eq!(ledger.available(&dev(1), free), 70 * MB);
        drop(held);
        assert_eq!(ledger.outstanding(&dev(1)), 0);
        assert!(ledger.is_empty());
    }

    #[test]
    fn progress_lowers_the_outstanding_amount_and_never_below_zero() {
        let ledger = SpaceLedger::new();
        let held = ledger.reserve(dev(1), 100);
        held.set_written(40);
        assert_eq!(held.outstanding(), 60);
        held.add_written(50);
        assert_eq!(ledger.outstanding(&dev(1)), 10);
        held.add_written(500);
        assert_eq!(held.outstanding(), 0);
        held.release();
        assert_eq!(held.reserved(), 0);
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn reservations_are_kept_per_filesystem() {
        let ledger = SpaceLedger::new();
        let _a = ledger.reserve(dev(1), 10);
        let _b = ledger.reserve(dev(2), 20);
        let _c = ledger.reserve(FsKey::Volume("d:\\".into()), 40);
        assert_eq!(ledger.outstanding(&dev(1)), 10);
        assert_eq!(ledger.outstanding(&dev(2)), 20);
        assert_eq!(ledger.outstanding(&FsKey::Volume("d:\\".into())), 40);
        assert_eq!(ledger.outstanding(&dev(3)), 0);
    }

    #[test]
    fn try_reserve_is_all_or_nothing_and_sums_one_filesystem() {
        let ledger = SpaceLedger::new();
        let free = SPACE_MARGIN + 100;
        let refused = ledger
            .try_reserve(&[
                one(dev(1), 60, free),
                one(dev(2), 1, free),
                one(dev(1), 41, free),
            ])
            .unwrap_err();
        assert_eq!(
            refused,
            (
                0,
                Shortfall {
                    needed: 101,
                    available: 100
                }
            )
        );
        assert!(ledger.is_empty());
        let held = ledger
            .try_reserve(&[one(dev(1), 60, free), one(dev(1), 40, free)])
            .unwrap();
        assert_eq!(held.len(), 2);
        assert_eq!(ledger.available(&dev(1), free), 0);
        // A zero-byte request always fits.
        assert!(ledger.try_reserve(&[one(dev(1), 0, free)]).is_ok());
        assert!(ledger.try_reserve(&[one(dev(1), 1, free)]).is_err());
    }

    #[test]
    fn try_resize_grows_only_into_what_others_leave() {
        let ledger = SpaceLedger::new();
        let free = SPACE_MARGIN + 100;
        let _other = ledger.reserve(dev(1), 30);
        let mine = ledger.reserve(dev(1), 10);
        assert!(mine.try_resize(70, free).is_ok());
        assert_eq!(
            mine.try_resize(71, free),
            Err(Shortfall {
                needed: 71,
                available: 70
            })
        );
        assert_eq!(mine.reserved(), 70);
        // Written bytes are already in `free`: only the rest is checked.
        mine.set_written(50);
        assert!(mine.try_resize(120, free).is_ok());
        // Shrinking is always allowed, even past what fits.
        let _hog = ledger.reserve(dev(1), 1000);
        assert!(mine.try_resize(60, free).is_ok());
    }

    #[test]
    fn check_and_reserve_is_atomic_across_threads() {
        let ledger = SpaceLedger::new();
        let free = SPACE_MARGIN + 100;
        let threads = 8;
        let barrier = Arc::new(Barrier::new(threads));
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let ledger = ledger.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    ledger.try_reserve(&[one(dev(1), 60, free)]).ok()
                })
            })
            .collect();
        let won: Vec<Reservation> = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap())
            .flatten()
            .collect();
        assert_eq!(won.len(), 1, "only one 60-byte job fits in 100 bytes");
        assert_eq!(ledger.outstanding(&dev(1)), 60);
    }

    #[test]
    fn a_panicking_job_still_releases_its_reservation() {
        let ledger = SpaceLedger::new();
        let inner = ledger.clone();
        let result = std::thread::spawn(move || {
            let _held = inner.reserve(dev(1), 50);
            panic!("the job died");
        })
        .join();
        assert!(result.is_err());
        assert!(ledger.is_empty());
    }

    #[test]
    fn fs_key_reads_the_nearest_existing_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let here = fs_key(tmp.path()).unwrap();
        assert_eq!(fs_key(&tmp.path().join("not/yet/made")), Some(here));
    }

    #[test]
    fn format_size_matches_the_downloads_rows() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1_288_490_189), "1.2 GB");
    }

    fn probe(free: u64) -> SpaceProbe {
        SpaceProbe {
            free: Arc::new(move |_| Some(free)),
            key: Arc::new(|_| Some(FsKey::Device(1))),
        }
    }

    #[test]
    fn an_install_credits_download_and_extraction_to_one_reservation() {
        let ledger = SpaceLedger::new();
        let estimate = Estimate {
            download: Some(100),
            existing: 0,
            extracts: true,
        };
        let space = JobSpace::install(
            Some(ledger.reserve(dev(1), estimate.peak())),
            estimate,
            PathBuf::new(),
            "Ember",
            probe(u64::MAX),
        );
        assert_eq!(space.library_outstanding(), 200);
        assert!(!space.on_download(100, 100));
        assert_eq!(space.library_outstanding(), 100);
        // Two archives: the second pass starts again from 0.
        space.on_finalize(60);
        space.on_finalize(10);
        assert_eq!(space.library_outstanding(), 30);
    }

    #[test]
    fn an_unknown_download_grows_to_the_content_length_or_stops() {
        let ledger = SpaceLedger::new();
        let free = SPACE_MARGIN + 150;
        let estimate = Estimate {
            download: None,
            existing: 0,
            extracts: true,
        };
        let space = JobSpace::install(
            Some(ledger.reserve(dev(1), 0)),
            estimate,
            PathBuf::new(),
            "Ember",
            probe(free),
        );
        assert!(!space.on_download(0, 0));
        assert_eq!(space.library_reserved(), 0);
        // 100 to download and 100 to extract do not fit in 150.
        assert!(space.on_download(10, 100));
        assert_eq!(
            space.shortfall().as_deref(),
            Some("There isn't enough free space in the library folder for Ember (needs 190 B, 150 B available)")
        );
    }

    #[test]
    fn the_real_extracted_size_replaces_the_estimate() {
        let ledger = SpaceLedger::new();
        // The volume after the 100-byte download: written bytes are in `free`.
        let free = SPACE_MARGIN + 150;
        let estimate = Estimate {
            download: Some(100),
            existing: 0,
            extracts: true,
        };
        let space = JobSpace::install(
            Some(ledger.reserve(dev(1), 200)),
            estimate,
            PathBuf::new(),
            "Ember",
            probe(free),
        );
        space.on_download(100, 100);
        assert!(space.resize_for_extract(150).is_ok());
        assert_eq!(space.library_reserved(), 250);
        assert!(space.resize_for_extract(160).is_err());
    }

    #[test]
    fn bytes_already_at_the_destination_need_no_new_space() {
        let ledger = SpaceLedger::new();
        // An update replacing a 60-byte archive with a 100-byte one, not
        // extracted: only 40 new bytes.
        let estimate = Estimate {
            download: Some(100),
            existing: 60,
            extracts: false,
        };
        assert_eq!(estimate.peak(), 40);
        let space = JobSpace::install(
            Some(ledger.reserve(dev(1), estimate.peak())),
            estimate,
            PathBuf::new(),
            "Ember",
            probe(u64::MAX),
        );
        space.on_download(80, 100);
        assert_eq!(space.library_outstanding(), 20);
        space.on_download(100, 100);
        assert_eq!(space.library_outstanding(), 0);
        // Extracted, the tree still needs the whole archive's size.
        let extracted = Estimate {
            extracts: true,
            ..estimate
        };
        assert_eq!(extracted.peak(), 140);
    }

    #[test]
    fn a_backup_credits_staging_and_drive_separately() {
        let ledger = SpaceLedger::new();
        let space = JobSpace::backup(
            Some(ledger.reserve(dev(1), 100)),
            Some(ledger.reserve(dev(2), 100)),
            "Ember",
            probe(u64::MAX),
        );
        space.on_download(100, 100);
        assert_eq!(ledger.outstanding(&dev(1)), 0);
        assert_eq!(ledger.outstanding(&dev(2)), 100);
        space.on_finalize(40);
        assert_eq!(space.drive_outstanding(), 60);
        space.release_staging();
        assert_eq!(space.library_reserved(), 0);
        drop(space);
        assert!(ledger.is_empty());
    }
}
