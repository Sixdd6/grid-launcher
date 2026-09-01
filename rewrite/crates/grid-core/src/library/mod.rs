//! Library install pipeline.
//!
//! [`InstallService`] is the glue that turns the queue state machine, the
//! downloader, the extraction engine, launch-file selection, the path rules
//! and the SQLite registry into one install / uninstall flow. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! ("InstallService", "Data flow for one install") and
//! `docs/porting/03-library-install.md` for the behavior this mirrors.

pub mod download;
pub mod extract;
pub mod launch_select;
pub mod paths;
pub mod queue;
pub mod registry;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::config::Config;
use crate::romm::{RomDetail, RomFile, RommClient};
use download::{download_targets, FileTarget};
use extract::{extract_archive, should_extract};
use launch_select::select_launch_file;
use paths::{
    archive_name, candidate_archives, candidate_extracted_dirs, extraction_dir, platform_dir,
    sanitize_component,
};
use queue::{Admission, CancelAction, DownloadStatus, DownloadsSnapshot, QueueState};
use registry::{installed_match, InstalledGame, Registry};

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error(transparent)]
    Romm(#[from] crate::romm::RommError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("{0}")]
    Extract(String),
    #[error("Archive extracted but no ROM file was found")]
    NoLaunchFile,
    #[error("cancelled")]
    Cancelled,
    #[error("Set a library folder in settings before installing games")]
    LibraryPathUnset,
    #[error("registry: {0}")]
    Registry(String),
}

// --- constants --------------------------------------------------------------

/// Minimum gap between change notifications caused by download progress.
/// Status transitions ignore this and always notify.
const DOWNLOAD_NOTIFY_INTERVAL: Duration = Duration::from_millis(100);
/// Minimum gap between change notifications caused by install (extraction)
/// progress.
const INSTALL_NOTIFY_INTERVAL: Duration = Duration::from_millis(150);
/// Attempts and pause for the post-extract archive delete (doc 03: an
/// antivirus or indexer can hold the file open briefly after extraction).
const ARCHIVE_DELETE_ATTEMPTS: u32 = 20;
const ARCHIVE_DELETE_PAUSE: Duration = Duration::from_millis(250);
/// The metadata sidecar the server lists among a game's files. It is never a
/// download target.
const METADATA_FILE_NAME: &str = "game.json";
const NO_DOWNLOADABLE_FILE: &str = "the server lists no downloadable file for this game";

/// Everything outside the RFC 3986 unreserved set (`ALPHA / DIGIT / - . _ ~`)
/// is percent-encoded. Applied to the file-name segment of a content URL so a
/// name containing a space, `#`, `?` or `/` can never change the shape of the
/// request.
const FILE_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

fn encode_file_segment(name: &str) -> String {
    utf8_percent_encode(name, FILE_SEGMENT).to_string()
}

// --- install plan -----------------------------------------------------------

/// Everything one admitted install needs, computed once before admission so a
/// queued job can start later without re-fetching anything.
#[derive(Clone)]
struct InstallJob {
    rom_id: i64,
    detail: RomDetail,
    targets: Vec<FileTarget>,
    /// The primary downloaded file: the archive for a single-file install,
    /// the launch entry's path for a multi-file one.
    primary_archive: PathBuf,
    /// `Some` only for a multi-file game, which is never extracted.
    multi_file_game_dir: Option<PathBuf>,
    /// Server `file_name` of the primary file — recorded as `rom_file_name`,
    /// and for a multi-file game the launch entry's name.
    launch_entry: String,
    /// The client the download runs on, carried on the job so a queued
    /// install can start after its caller has gone away.
    client: Arc<RommClient>,
}

/// Whether the server lists `file` as something to download: a top-level
/// entry that is neither the metadata sidecar nor a nested path (the server
/// occasionally flags a nested entry as top-level; such a name would escape
/// the game directory).
fn is_download_candidate(file: &RomFile) -> bool {
    file.is_top_level
        && file.file_name != METADATA_FILE_NAME
        && !file.file_name.contains('/')
        && !file.file_name.contains('\\')
}

fn content_target(rom_id: i64, file: &RomFile, dest: PathBuf, expected_size: i64) -> FileTarget {
    FileTarget {
        url_path: format!(
            "/api/roms/{rom_id}/content/{}",
            encode_file_segment(&file.file_name)
        ),
        query: vec![("file_ids".to_string(), file.id.to_string())],
        dest,
        expected_size,
    }
}

/// Computes the download plan for `detail` under `library`. Pure apart from
/// cloning the client handle: no I/O, no queue access.
///
/// One candidate ⇒ a single archive at `<platform dir>/<archive name>`.
/// More than one ⇒ a multi-file game: every candidate lands in
/// `<platform dir>/<safe title>/`, and the launch entry is the first `.m3u`
/// if there is one, else the first candidate.
fn plan_install(
    detail: &RomDetail,
    library: &Path,
    client: Arc<RommClient>,
) -> Result<InstallJob, LibraryError> {
    let candidates: Vec<&RomFile> = detail
        .files
        .iter()
        .filter(|file| is_download_candidate(file))
        .collect();
    let platform_root = platform_dir(library, &detail.platform_name);

    match candidates.as_slice() {
        [] => Err(LibraryError::Extract(NO_DOWNLOADABLE_FILE.to_string())),
        [only] => {
            let dest = platform_root.join(archive_name(
                &detail.fs_name,
                &detail.name,
                &detail.platform_name,
            ));
            // A single-file game's own size is the better total when the file
            // entry does not carry one.
            let size = if only.file_size_bytes > 0 {
                only.file_size_bytes
            } else {
                detail.filesize_bytes
            };
            Ok(InstallJob {
                rom_id: detail.id,
                detail: detail.clone(),
                targets: vec![content_target(detail.id, only, dest.clone(), size)],
                primary_archive: dest,
                multi_file_game_dir: None,
                launch_entry: only.file_name.clone(),
                client,
            })
        }
        many => {
            let game_dir = platform_root.join(sanitize_component(&detail.name, "game"));
            let launch = many
                .iter()
                .copied()
                .find(|file| file.file_name.to_lowercase().ends_with(".m3u"))
                .unwrap_or(many[0]);
            let targets = many
                .iter()
                .map(|file| {
                    content_target(
                        detail.id,
                        file,
                        game_dir.join(&file.file_name),
                        file.file_size_bytes,
                    )
                })
                .collect();
            Ok(InstallJob {
                rom_id: detail.id,
                detail: detail.clone(),
                targets,
                primary_archive: game_dir.join(&launch.file_name),
                multi_file_game_dir: Some(game_dir),
                launch_entry: launch.file_name.clone(),
                client,
            })
        }
    }
}

// --- service ----------------------------------------------------------------

type Listener = Arc<dyn Fn(DownloadsSnapshot) + Send + Sync>;

/// Owns the install queue and drives one download task and one finalize task
/// at a time. Every method takes `&self`; the async entry points take
/// `&Arc<Self>` because they spawn tasks that outlive the call.
pub struct InstallService {
    queue: Mutex<QueueState>,
    registry: Arc<Registry>,
    config_path: PathBuf,
    notify: RwLock<Option<Listener>>,
    /// One cooperative cancellation flag per active download, keyed by entry
    /// id and removed when that download ends.
    cancel_flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    /// Plans for entries that were admitted as `Queued` and are waiting for a
    /// free slot.
    pending_jobs: Mutex<HashMap<u64, InstallJob>>,
    last_emit: Mutex<Instant>,
}

impl InstallService {
    pub fn new(registry: Arc<Registry>, config_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(QueueState::default()),
            registry,
            config_path,
            notify: RwLock::new(None),
            cancel_flags: Mutex::new(HashMap::new()),
            pending_jobs: Mutex::new(HashMap::new()),
            last_emit: Mutex::new(Instant::now()),
        })
    }

    /// Installs the change-notification callback. Called once by the UI
    /// layer; a second call replaces the first.
    pub fn set_notify(&self, f: Listener) {
        *self.notify.write().unwrap() = Some(f);
    }

    /// The current entry list, newest first.
    pub fn snapshot(&self) -> DownloadsSnapshot {
        self.queue.lock().unwrap().snapshot()
    }

    /// Every installed game in the registry.
    pub fn installed(&self) -> Result<Vec<InstalledGame>, LibraryError> {
        self.registry.all()
    }

    /// Starts (or queues) an install for `rom_id`.
    ///
    /// `Err` is returned only for failures before admission — no library
    /// path, an unreachable server, or a game the server lists no
    /// downloadable file for. Once an entry exists, every later failure shows
    /// up on that entry instead. A `rom_id` that is already downloading,
    /// finalizing or queued is ignored silently.
    pub async fn install(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        rom_id: i64,
    ) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        let detail = client.rom_detail(rom_id).await?;
        let job = plan_install(&detail, &library, client)?;
        self.admit(job);
        Ok(())
    }

    /// Cancels `entry_id`: an active download gets its flag flipped and ends
    /// as `Cancelled` once the task observes it; a queued entry is dropped
    /// immediately. Anything else is ignored.
    pub fn cancel(&self, entry_id: u64) {
        let action = self.queue.lock().unwrap().request_cancel(entry_id);
        match action {
            CancelAction::ActiveDownload => {
                if let Some(flag) = self.cancel_flags.lock().unwrap().get(&entry_id) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
            CancelAction::RemovedFromQueue => {
                self.pending_jobs.lock().unwrap().remove(&entry_id);
            }
            CancelAction::Ignored => return,
        }
        self.notify_now();
    }

    /// Retries a failed or cancelled entry: the old entry is dismissed and a
    /// fresh install starts for the same rom. Any other entry is ignored.
    pub async fn retry(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        entry_id: u64,
    ) -> Result<(), LibraryError> {
        let retryable = self.queue.lock().unwrap().retryable(entry_id);
        let Some(rom_id) = retryable else {
            return Ok(());
        };
        self.dismiss(entry_id);
        self.install(client, rom_id).await
    }

    /// Removes `entry_id` from the list. An entry that still owns the
    /// download or finalize slot is left alone (the queue refuses it), so
    /// this is a no-op for anything still running.
    pub fn dismiss(&self, entry_id: u64) {
        // Bound to a `let` so the queue lock is definitely released before
        // `notify_now` below takes it again to build the snapshot.
        let removed = self.queue.lock().unwrap().dismiss(entry_id);
        if !removed {
            return;
        }
        self.pending_jobs.lock().unwrap().remove(&entry_id);
        self.cancel_flags.lock().unwrap().remove(&entry_id);
        self.notify_now();
    }

    /// Deletes an installed game's files and its registry row.
    ///
    /// A multi-file game's directory is removed wholesale; anything else has
    /// every existing candidate archive and extraction directory removed. The
    /// row is deleted only after the files are gone, so a failure leaves the
    /// game installed rather than orphaning it.
    pub fn uninstall(&self, rom_id: i64) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        // `find` falls back to the (title, platform) identity when no row
        // carries the rom id; blank keys must not match a blank-titled row,
        // and a row that fell back with a *different* rom_id must not be
        // treated as this game's install, so `installed_match` is the final
        // word on what comes back (a null-rom_id row is still accepted,
        // matching the frontend's `matchesInstalled`).
        let record = self
            .registry
            .find(Some(rom_id), "", "")?
            .filter(|found| installed_match(found, rom_id))
            .ok_or_else(|| LibraryError::Registry("not installed".to_string()))?;

        let game_dir = record.multi_file_game_dir.trim();
        if !game_dir.is_empty() && Path::new(game_dir).is_dir() {
            remove_dir_tree(Path::new(game_dir))?;
        } else {
            let name = archive_name(&record.rom_file_name, &record.title, &record.platform);
            let archives =
                candidate_archives(&library, &record.platform, &record.archive_path, &name);
            let extracted = candidate_extracted_dirs(&archives, &record.extracted_dir);
            for archive in &archives {
                if archive.is_file() {
                    fs::remove_file(archive)?;
                }
            }
            for dir in &extracted {
                if dir.is_dir() {
                    remove_dir_tree(dir)?;
                }
            }
        }

        self.registry.remove(&record.title, &record.platform)?;
        Ok(())
    }

    // --- internals ----------------------------------------------------------

    /// The configured library root, with a leading `~/` expanded.
    fn library_root(&self) -> Result<PathBuf, LibraryError> {
        let config = Config::load(&self.config_path)?;
        if config.library_path.trim().is_empty() {
            return Err(LibraryError::LibraryPathUnset);
        }
        Ok(paths::expand_home(&config.library_path))
    }

    /// Admits a planned job: starts it, stashes it for a free slot, or drops
    /// it as a duplicate.
    ///
    /// The stash happens while the queue lock is still held. Releasing it
    /// first would let a finishing download's [`Self::pump`] pop the brand
    /// new id out of `waiting` before its job exists, fail the entry as lost
    /// and leave the job stranded in `pending_jobs`.
    fn admit(self: &Arc<Self>, job: InstallJob) {
        let (admitted, start) = {
            let mut queue = self.queue.lock().unwrap();
            match queue.admit(job.rom_id, &job.detail.name, &job.detail.platform_name) {
                Admission::Start(id) => (true, Some((id, job))),
                Admission::Queued(id) => {
                    self.pending_jobs.lock().unwrap().insert(id, job);
                    (true, None)
                }
                Admission::Duplicate => (false, None),
            }
        };
        // Nothing below runs under the queue lock: `spawn_download` takes it
        // again, and the listener must never see it held.
        if !admitted {
            return;
        }
        if let Some((id, job)) = start {
            self.spawn_download(id, job);
        }
        self.notify_now();
    }

    /// Starts the download task for `id`, registering its cancellation flag
    /// before the task exists so an immediate `cancel` is never lost.
    fn spawn_download(self: &Arc<Self>, id: u64, job: InstallJob) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flags.lock().unwrap().insert(id, cancel.clone());
        // A cancel that arrived after the entry took the download slot but
        // before the flag above existed found nothing to flip. The
        // `Cancelling` status it left behind is the record of it, so honour
        // that here instead of downloading a file nobody wants.
        let cancelled = matches!(
            self.queue.lock().unwrap().entry(id).map(|e| e.status),
            Some(DownloadStatus::Cancelling)
        );
        if cancelled {
            cancel.store(true, Ordering::Relaxed);
        }
        let service = self.clone();
        tokio::spawn(async move {
            // The fallible half runs in its own task so a panic anywhere in
            // it — including inside the caller's notify listener, reached
            // through `on_download_progress` — comes back as a `JoinError`
            // instead of leaving the download slot taken forever.
            let reporter = service.clone();
            let worker = tokio::spawn(async move {
                let mut on_progress = move |downloaded, total, speed| {
                    reporter.on_download_progress(id, downloaded, total, speed);
                };
                let result =
                    download_targets(&job.client, &job.targets, &cancel, &mut on_progress).await;
                (job, result)
            });
            let outcome = worker.await;
            service.cancel_flags.lock().unwrap().remove(&id);
            match outcome {
                Ok((job, result)) => service.finish_download(id, job, result).await,
                Err(e) => service.fail_download(
                    id,
                    LibraryError::Extract(format!("the download did not finish: {e}")),
                ),
            }
        });
    }

    /// Fails `id` with no job in hand — the download task itself died, so
    /// there is nothing left to finalize. Frees the download slot so the
    /// queue keeps moving instead of wedging on a task that will never
    /// report back.
    fn fail_download(self: &Arc<Self>, id: u64, error: LibraryError) {
        self.queue
            .lock()
            .unwrap()
            .download_finished(id, Err(error), false);
        self.pump();
        self.notify_now();
    }

    /// Records the download outcome, starts finalizing when there is
    /// something to finalize, and lets the queue move on. Called exactly once
    /// by the task that owns the download slot.
    async fn finish_download(
        self: &Arc<Self>,
        id: u64,
        job: InstallJob,
        result: Result<(), LibraryError>,
    ) {
        // doc 03 §1 step 4: a game that is already in the registry completes
        // straight away — the bytes are on disk, nothing needs installing.
        let skip_finalize = result.is_ok() && self.already_installed(&job.detail).await;
        // A cancel that lands after the last chunk arrived is too late: the
        // download succeeded, so `Cancelling` gives way to `Installing` here
        // and the entry finishes. Extraction is not cancellable (doc 03 §1).
        let finalize = {
            let mut queue = self.queue.lock().unwrap();
            queue.download_finished(id, result, skip_finalize);
            matches!(
                queue.entry(id).map(|entry| entry.status),
                Some(DownloadStatus::Installing)
            )
        };
        if finalize {
            self.spawn_finalize(id, job);
        }
        self.pump();
        self.notify_now();
    }

    /// Registry lookup on the blocking pool. A lookup error is treated as
    /// "not installed": finalizing again is harmless, and the same error will
    /// surface from the upsert with a real message.
    ///
    /// `find`'s title/platform fallback can return a row for a different game
    /// that merely shares a title and platform; `installed_match` is what
    /// keeps that row from being reported as this rom_id's install (doc 03
    /// identity rules; mirrored in the frontend's `matchesInstalled`).
    async fn already_installed(&self, detail: &RomDetail) -> bool {
        let registry = self.registry.clone();
        let rom_id = detail.id;
        let title = detail.name.clone();
        let platform = detail.platform_name.clone();
        tokio::task::spawn_blocking(move || registry.find(Some(rom_id), &title, &platform))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten()
            .is_some_and(|row| installed_match(&row, rom_id))
    }

    /// Starts the finalize task for `id`. Called exactly once by the task
    /// that owns the finalize slot.
    fn spawn_finalize(self: &Arc<Self>, id: u64, job: InstallJob) {
        let service = self.clone();
        tokio::spawn(async move {
            let worker = service.clone();
            let (result, warning) =
                match tokio::task::spawn_blocking(move || worker.finalize(id, &job)).await {
                    Ok(outcome) => outcome,
                    Err(e) => (
                        Err(LibraryError::Extract(format!(
                            "the install step did not finish: {e}"
                        ))),
                        String::new(),
                    ),
                };
            service
                .queue
                .lock()
                .unwrap()
                .finalize_finished(id, result, &warning);
            service.pump();
            service.notify_now();
        });
    }

    /// Starts the next ready job when both slots are free.
    ///
    /// Taking the job out of `pending_jobs` under the queue lock is what
    /// makes the "no stashed plan" branch below unreachable: every id in
    /// `waiting` had its job stashed inside the same critical section that
    /// put it there. The lock ordering — queue first, `pending_jobs` second,
    /// never the reverse — is shared with [`Self::admit`].
    fn pump(self: &Arc<Self>) {
        loop {
            let start = {
                let mut queue = self.queue.lock().unwrap();
                let Some(id) = queue.next_ready() else {
                    return;
                };
                match self.pending_jobs.lock().unwrap().remove(&id) {
                    Some(job) => Some((id, job)),
                    None => {
                        // Defensive: a queued entry with no stashed plan can
                        // never run. Fail it so the slot frees and the queue
                        // keeps moving instead of wedging.
                        queue.download_finished(
                            id,
                            Err(LibraryError::Extract(
                                "the queued install was lost".to_string(),
                            )),
                            false,
                        );
                        None
                    }
                }
            };
            // `spawn_download` takes the queue lock itself, so it can only be
            // called once the guard above has been released.
            if let Some((id, job)) = start {
                self.spawn_download(id, job);
                return;
            }
        }
    }

    /// The blocking half of finalizing: extraction, launch selection,
    /// registry write and archive cleanup. Returns the outcome plus a
    /// warning string that is empty unless the install succeeded with a
    /// caveat.
    fn finalize(&self, id: u64, job: &InstallJob) -> (Result<(), LibraryError>, String) {
        let mut warning = String::new();
        let result = self.finalize_inner(id, job, &mut warning);
        (result, warning)
    }

    fn finalize_inner(
        &self,
        id: u64,
        job: &InstallJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let detail = &job.detail;
        let mut record = new_record(detail, &job.launch_entry);
        let mut archive_to_delete: Option<&Path> = None;

        if let Some(game_dir) = &job.multi_file_game_dir {
            // A multi-file game is already laid out on disk: the files are
            // the install, and the launch entry is what gets started.
            record.multi_file_game_dir = path_string(game_dir);
            record.extracted_path = path_string(&job.primary_archive);
        } else {
            let archive = job.primary_archive.as_path();
            if should_extract(&detail.platform_name, archive) {
                let dest = extraction_dir(archive);
                let mut progress =
                    |processed, total| self.on_install_progress(id, processed, total);
                extract_archive(archive, &dest, &mut progress)?;
                let stem = archive
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let Some(launch) = select_launch_file(&dest, &stem) else {
                    let _ = fs::remove_dir_all(&dest);
                    return Err(LibraryError::NoLaunchFile);
                };
                make_executable(&launch);
                record.extracted_path = path_string(&launch);
                record.extracted_dir = path_string(&dest);
                archive_to_delete = Some(archive);
            } else {
                // Nothing to extract: the downloaded file is the install.
                if is_appimage(archive) {
                    make_executable(archive);
                }
                record.archive_path = path_string(archive);
            }
        }

        self.registry.upsert(&record)?;

        // Only after a successful write, and only when the archive has been
        // superseded by an extraction — a finalize failure keeps the archive
        // so a retry skips the re-download (doc 03 invariant 5).
        if let Some(archive) = archive_to_delete {
            if !delete_with_retry(archive) {
                *warning = format!("could not delete archive: {}", archive.display());
            }
        }
        Ok(())
    }

    fn on_download_progress(&self, id: u64, downloaded: u64, total: u64, speed: f64) {
        self.queue
            .lock()
            .unwrap()
            .set_progress(id, downloaded, total, speed);
        self.notify_throttled(DOWNLOAD_NOTIFY_INTERVAL);
    }

    fn on_install_progress(&self, id: u64, processed: u64, total: u64) {
        self.queue
            .lock()
            .unwrap()
            .set_install_progress(id, processed, total);
        self.notify_throttled(INSTALL_NOTIFY_INTERVAL);
    }

    /// Notifies only when at least `min_gap` has passed since the last
    /// notification, so a fast download cannot flood the UI.
    fn notify_throttled(&self, min_gap: Duration) {
        let due = {
            let mut last = self.last_emit.lock().unwrap();
            let due = last.elapsed() >= min_gap;
            if due {
                *last = Instant::now();
            }
            due
        };
        if due {
            self.emit();
        }
    }

    /// Notifies unconditionally — used for every status transition.
    fn notify_now(&self) {
        *self.last_emit.lock().unwrap() = Instant::now();
        self.emit();
    }

    /// Hands the snapshot to the listener with NO lock held: the callback is
    /// arbitrary UI code and must never be able to block a state change.
    fn emit(&self) {
        let listener = self.notify.read().unwrap().clone();
        let Some(listener) = listener else {
            return;
        };
        let snapshot = self.snapshot();
        listener(snapshot);
    }
}

// --- record + filesystem helpers --------------------------------------------

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A registry record carrying every metadata field from `detail`. The path
/// fields are left empty for the finalize branch to fill in.
fn new_record(detail: &RomDetail, rom_file_name: &str) -> InstalledGame {
    InstalledGame {
        title: detail.name.clone(),
        platform: detail.platform_name.clone(),
        rom_id: Some(detail.id),
        rom_file_name: rom_file_name.to_string(),
        description: detail.description.clone(),
        rating: detail.rating.clone(),
        genres: detail.genres.clone(),
        regions: detail.regions.clone(),
        languages: detail.languages.clone(),
        tags: detail.tags.clone(),
        revision: detail.revision.clone(),
        companies: detail.companies.clone(),
        first_release_date: detail.first_release_date.clone(),
        filesize_bytes: detail.filesize_bytes,
        server_updated_at: detail.server_updated_at.clone(),
        installed_at: unix_now(),
        ..Default::default()
    }
}

fn is_appimage(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("appimage"))
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Deletes `path`, retrying every [`ARCHIVE_DELETE_PAUSE`] for up to
/// [`ARCHIVE_DELETE_ATTEMPTS`] attempts. Returns whether the file is gone.
/// Blocking — only ever called from the finalize task's blocking half.
fn delete_with_retry(path: &Path) -> bool {
    for attempt in 0..ARCHIVE_DELETE_ATTEMPTS {
        match fs::remove_file(path) {
            Ok(()) => return true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => {}
        }
        if attempt + 1 < ARCHIVE_DELETE_ATTEMPTS {
            std::thread::sleep(ARCHIVE_DELETE_PAUSE);
        }
    }
    false
}

/// Removes a directory tree. A read-only directory inside the tree makes the
/// first attempt fail with `PermissionDenied`; that case chmods everything in
/// the tree writable once and retries.
fn remove_dir_tree(dir: &Path) -> Result<(), LibraryError> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            make_tree_writable(dir);
            fs::remove_dir_all(dir).map_err(LibraryError::Io)
        }
        Err(e) => Err(LibraryError::Io(e)),
    }
}

/// Adds owner write (and, for directories, traverse) permission to every
/// entry under `dir`, `dir` itself included. Symlinks are skipped so the
/// walk can never change permissions outside the tree. Best effort: an entry
/// that cannot be changed is left as it is and the retry reports it.
#[cfg(unix)]
fn make_tree_writable(dir: &Path) {
    add_mode(dir, 0o700);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            make_tree_writable(&path);
        } else {
            add_mode(&path, 0o600);
        }
    }
}

#[cfg(unix)]
fn add_mode(path: &Path, extra: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mode = meta.permissions().mode();
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode | extra));
    }
}

#[cfg(not(unix))]
fn make_tree_writable(_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Credential;
    use secrecy::SecretString;

    fn client() -> Arc<RommClient> {
        Arc::new(
            RommClient::new(
                "http://localhost:1",
                Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
            )
            .unwrap(),
        )
    }

    fn rom_file(id: i64, file_name: &str, top_level: bool) -> RomFile {
        RomFile {
            id,
            file_name: file_name.to_string(),
            file_size_bytes: 10,
            is_top_level: top_level,
        }
    }

    fn detail(files: Vec<RomFile>) -> RomDetail {
        RomDetail {
            id: 42,
            name: "Chrono Trigger".to_string(),
            platform_id: 7,
            platform_name: "SNES".to_string(),
            fs_name: "chrono.zip".to_string(),
            description: String::new(),
            regions: String::new(),
            languages: String::new(),
            tags: String::new(),
            revision: String::new(),
            rating: String::new(),
            genres: String::new(),
            companies: String::new(),
            first_release_date: String::new(),
            filesize_bytes: 999,
            server_updated_at: String::new(),
            files,
        }
    }

    // --- encode_file_segment ------------------------------------------------

    #[test]
    fn encodes_everything_but_the_unreserved_set() {
        assert_eq!(encode_file_segment("game.zip"), "game.zip");
        assert_eq!(encode_file_segment("A-b_c.~d"), "A-b_c.~d");
        assert_eq!(encode_file_segment("my game.zip"), "my%20game.zip");
        assert_eq!(
            encode_file_segment("a/b?c#d&e.zip"),
            "a%2Fb%3Fc%23d%26e.zip"
        );
        assert_eq!(encode_file_segment("naïve.zip"), "na%C3%AFve.zip");
    }

    // --- plan_install --------------------------------------------------------

    #[test]
    fn plan_rejects_a_game_with_no_downloadable_file() {
        let detail = detail(vec![
            rom_file(1, "game.json", true),
            rom_file(2, "nested/rom.bin", true),
            rom_file(3, "inner.bin", false),
        ]);
        let Err(err) = plan_install(&detail, Path::new("/library"), client()) else {
            panic!("expected an error");
        };
        assert!(
            matches!(&err, LibraryError::Extract(msg) if msg == NO_DOWNLOADABLE_FILE),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn plan_single_file_targets_the_platform_dir_archive() {
        let detail = detail(vec![rom_file(11, "chrono.zip", true)]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();

        assert_eq!(job.rom_id, 42);
        assert!(job.multi_file_game_dir.is_none());
        assert_eq!(job.launch_entry, "chrono.zip");
        assert_eq!(
            job.primary_archive,
            PathBuf::from("/library/SNES/chrono.zip")
        );
        assert_eq!(job.targets.len(), 1);
        assert_eq!(job.targets[0].url_path, "/api/roms/42/content/chrono.zip");
        assert_eq!(
            job.targets[0].query,
            vec![("file_ids".to_string(), "11".to_string())]
        );
        assert_eq!(job.targets[0].dest, job.primary_archive);
        assert_eq!(job.targets[0].expected_size, 10);
    }

    #[test]
    fn plan_single_file_falls_back_to_the_game_size() {
        let mut detail = detail(vec![rom_file(11, "chrono.zip", true)]);
        detail.files[0].file_size_bytes = 0;
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();
        assert_eq!(job.targets[0].expected_size, 999);
    }

    #[test]
    fn plan_multi_file_uses_a_game_dir_and_prefers_the_m3u() {
        let detail = detail(vec![
            rom_file(21, "disc1.bin", true),
            rom_file(22, "Game.M3U", true),
            rom_file(23, "game.json", true),
        ]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();

        let game_dir = PathBuf::from("/library/SNES/Chrono Trigger");
        assert_eq!(job.multi_file_game_dir, Some(game_dir.clone()));
        assert_eq!(job.launch_entry, "Game.M3U");
        assert_eq!(job.primary_archive, game_dir.join("Game.M3U"));
        assert_eq!(job.targets.len(), 2);
        assert_eq!(job.targets[0].dest, game_dir.join("disc1.bin"));
        assert_eq!(job.targets[1].dest, game_dir.join("Game.M3U"));
        assert_eq!(job.targets[1].url_path, "/api/roms/42/content/Game.M3U");
    }

    #[test]
    fn plan_multi_file_without_an_m3u_launches_the_first_candidate() {
        let detail = detail(vec![
            rom_file(21, "disc1.bin", true),
            rom_file(22, "disc2.bin", true),
        ]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();
        assert_eq!(job.launch_entry, "disc1.bin");
    }

    // --- helpers -------------------------------------------------------------

    #[test]
    fn appimage_detection_ignores_case() {
        assert!(is_appimage(Path::new("/x/Game.AppImage")));
        assert!(is_appimage(Path::new("/x/game.appimage")));
        assert!(!is_appimage(Path::new("/x/game.zip")));
    }

    #[test]
    fn delete_with_retry_reports_success_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(delete_with_retry(&dir.path().join("nope.zip")));
    }

    #[test]
    fn delete_with_retry_removes_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archive.zip");
        fs::write(&path, b"bytes").unwrap();
        assert!(delete_with_retry(&path));
        assert!(!path.exists());
    }
}
