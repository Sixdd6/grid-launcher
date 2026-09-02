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

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde_json::Value;

use crate::config::{Config, EmulatorEntry};
use crate::launch::forge::{ForgeClient, ForgeProvider, ResolvedDownload};
use crate::launch::profiles::{load_profiles, EmulatorProfile};
use crate::launch::{catalog, emu_install};
use crate::romm::{RomDetail, RomFile, RommClient};
use download::{download_targets, FileTarget, RommProvider};
use extract::{extract_archive, should_extract};
use launch_select::select_launch_file;
use paths::{
    archive_name, candidate_archives, candidate_extracted_dirs, extraction_dir, platform_dir,
    sanitize_component,
};
use queue::{Admission, CancelAction, DownloadStatus, DownloadsSnapshot, JobKey, QueueState};
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

/// The `platform` column an emulator entry shows in the downloads drawer.
/// Emulators are config entries, never registry rows, so this is a label
/// only — nothing looks a platform directory up from it.
const EMULATOR_PLATFORM: &str = "Emulator";
/// Where an emulator archive is extracted before its contents are merged
/// into the install directory. A sibling of the archive inside the install
/// directory, so the merge is a rename and never a cross-device copy; the
/// extraction engine wipes its destination, which is why it cannot be the
/// install directory itself (that would delete the downloaded supplementals
/// sitting next to the archive).
const EXTRACT_TMP_DIR: &str = ".extract-tmp";
const NO_EMULATOR_EXECUTABLE: &str = "No launchable emulator executable was found after install";

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

/// Everything one admitted emulator acquisition needs. Unlike an
/// [`InstallJob`], the download plan is NOT computed before admission: the
/// forge round trips that resolve a release down to one asset happen inside
/// the download task, so a resolution failure lands on the drawer row
/// exactly like a download failure does (matching the reference, which
/// resolves inside `InstallDownloadWorker.run`).
struct EmulatorJob {
    source_id: String,
    profile_name: String,
    profile_args: String,
    /// The tag the catalog source block CONFIGURES
    /// ([`catalog::configured_tag`]) — `"latest"` when it pins nothing.
    ///
    /// This, not the tag the release resolves to, names the install
    /// directory and is recorded as `source_release_tag`
    /// (install_mixin.py:1444, emulator_ui_mixin.py:1168-1190). A
    /// `latest`-pinned emulator therefore reinstalls over one stable
    /// `<name>-latest` directory instead of leaving one directory per
    /// release behind.
    configured_tag: String,
    /// The profile's RAW `source` block. Supplemental specs are read from
    /// here, not from the normalized/merged map: `workers.py:128` reads
    /// `self.source_metadata`, and each spec carries its own
    /// `platform_overrides` that `_resolve_source_download` merges per-spec.
    raw_source: Value,
    /// The library root, already `~`-expanded by [`InstallService::library_root`].
    library: PathBuf,
    forge: Arc<ForgeClient>,
    /// Filled in by the download task once the forge has resolved the
    /// release; consumed by finalize.
    resolved: Option<ResolvedPaths>,
}

/// What the download task resolved an [`EmulatorJob`] down to.
struct ResolvedPaths {
    install_dir: PathBuf,
    /// The primary archive, inside `install_dir`.
    archive: PathBuf,
    /// Downloaded supplemental archives, siblings of `archive`.
    supplementals: Vec<PathBuf>,
    resolved: ResolvedDownload,
}

/// The two job kinds the queue drives. One `pending_jobs` map and one
/// download/finalize pair serve both, so an emulator acquisition and a game
/// install queue behind each other rather than racing for the same slot.
enum JobPayload {
    Game(InstallJob),
    Emulator(EmulatorJob),
}

/// Whether a download from `provider` needs the GitHub API headers. Read
/// per download, not per job: a GitHub primary can carry a Gitea or direct
/// supplemental.
fn needs_github_headers(provider: &str) -> bool {
    provider == "github"
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
    pending_jobs: Mutex<HashMap<u64, JobPayload>>,
    /// The autoprofile catalog emulator `source_id`s resolve against.
    profiles: Cow<'static, [EmulatorProfile]>,
    /// Built on the first emulator install and shared by every later one, so
    /// all forge traffic goes through one connection pool. Deliberately not
    /// built in [`Self::new`]: constructing it can fail, and a process that
    /// never installs an emulator should never pay for it.
    forge: OnceLock<Arc<ForgeClient>>,
    last_emit: Mutex<Instant>,
}

impl InstallService {
    pub fn new(registry: Arc<Registry>, config_path: PathBuf) -> Arc<Self> {
        Self::build(registry, config_path, Cow::Borrowed(load_profiles()))
    }

    /// [`Self::new`], but resolving emulator `source_id`s against `profiles`
    /// instead of the embedded autoprofile catalog. The integration tests use
    /// this to point a profile's `source` block at a local server; production
    /// code calls [`Self::new`].
    pub fn with_profiles(
        registry: Arc<Registry>,
        config_path: PathBuf,
        profiles: Vec<EmulatorProfile>,
    ) -> Arc<Self> {
        Self::build(registry, config_path, Cow::Owned(profiles))
    }

    fn build(
        registry: Arc<Registry>,
        config_path: PathBuf,
        profiles: Cow<'static, [EmulatorProfile]>,
    ) -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(QueueState::default()),
            registry,
            config_path,
            notify: RwLock::new(None),
            cancel_flags: Mutex::new(HashMap::new()),
            pending_jobs: Mutex::new(HashMap::new()),
            profiles,
            forge: OnceLock::new(),
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
        let key = JobKey::Rom(job.rom_id);
        let title = job.detail.name.clone();
        let platform = job.detail.platform_name.clone();
        self.admit(key, &title, &platform, JobPayload::Game(job));
        Ok(())
    }

    /// Starts (or queues) an emulator acquisition for the catalog
    /// `source_id` (`"{owner}/{repo}"`).
    ///
    /// `Err` is returned only for failures before admission: no library
    /// path, or a `source_id` no catalog profile carries. Everything the
    /// forge is involved in — resolving the release, picking the asset,
    /// downloading, extracting — happens on the admitted entry, so those
    /// failures show up on the drawer row. A `source_id` that is already
    /// downloading, finalizing or queued is ignored silently.
    ///
    /// Nothing here touches the SQLite registry: an installed emulator is a
    /// config entry only.
    pub async fn install_emulator(self: &Arc<Self>, source_id: String) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        fn unknown(source_id: &str) -> LibraryError {
            LibraryError::Registry(format!("unknown emulator: {source_id}"))
        }

        let profile =
            catalog::find_profile(&self.profiles, &source_id).ok_or_else(|| unknown(&source_id))?;
        // `find_profile` only matches a profile whose `source` is an object
        // carrying an owner and a repo, so this clone always succeeds; the
        // fallback keeps that assumption from turning into a panic.
        let raw_source = profile.source.clone().ok_or_else(|| unknown(&source_id))?;
        let configured_tag = raw_source
            .as_object()
            .map(catalog::configured_tag)
            .unwrap_or_else(|| "latest".to_string());

        let job = EmulatorJob {
            source_id: source_id.clone(),
            profile_name: profile.name.clone(),
            profile_args: profile.args.clone(),
            configured_tag,
            raw_source,
            library,
            forge: self.forge()?,
            resolved: None,
        };
        let title = job.profile_name.clone();
        self.admit(
            JobKey::Emulator(source_id),
            &title,
            EMULATOR_PLATFORM,
            JobPayload::Emulator(job),
        );
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
    /// fresh install starts for the same rom or emulator. Any other entry is
    /// ignored.
    ///
    /// `client` is `None` when no RomM session is connected. An emulator
    /// retry never needs one — the forge client is separate and
    /// unauthenticated — so only a game retry fails with `"not connected"`,
    /// and it fails BEFORE the entry is dismissed so the row survives for a
    /// later attempt.
    pub async fn retry(
        self: &Arc<Self>,
        client: Option<Arc<RommClient>>,
        entry_id: u64,
    ) -> Result<(), LibraryError> {
        let retryable = self.queue.lock().unwrap().retryable(entry_id);
        match retryable {
            Some(JobKey::Rom(rom_id)) => {
                let client =
                    client.ok_or_else(|| LibraryError::Registry("not connected".to_string()))?;
                self.dismiss(entry_id);
                self.install(client, rom_id).await
            }
            Some(JobKey::Emulator(source_id)) => {
                self.dismiss(entry_id);
                self.install_emulator(source_id).await
            }
            None => Ok(()),
        }
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
        // carries the rom id, but never for this call: title and platform
        // are passed blank, so `find` itself refuses the fallback and this
        // only succeeds on a genuine rom_id match. `installed_match` then
        // has the final word on what comes back — a null-rom_id row is
        // still accepted (matching the frontend's `matchesInstalled`), but
        // a row that carries a *different* rom_id is rejected.
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

    /// The shared forge client, built on first use. Separate from every RomM
    /// client in the process and never given a credential.
    fn forge(&self) -> Result<Arc<ForgeClient>, LibraryError> {
        if let Some(client) = self.forge.get() {
            return Ok(client.clone());
        }
        let built = Arc::new(ForgeClient::new().map_err(|e| LibraryError::Extract(e.0))?);
        // A concurrent caller may have won the race; whichever client is
        // stored is the one everybody uses.
        Ok(self.forge.get_or_init(|| built).clone())
    }

    /// Admits a planned job: starts it, stashes it for a free slot, or drops
    /// it as a duplicate.
    ///
    /// The stash happens while the queue lock is still held. Releasing it
    /// first would let a finishing download's [`Self::pump`] pop the brand
    /// new id out of `waiting` before its job exists, fail the entry as lost
    /// and leave the job stranded in `pending_jobs`.
    fn admit(self: &Arc<Self>, key: JobKey, title: &str, platform: &str, payload: JobPayload) {
        let (admitted, start) = {
            let mut queue = self.queue.lock().unwrap();
            match queue.admit(key, title, platform) {
                Admission::Start(id) => (true, Some((id, payload))),
                Admission::Queued(id) => {
                    self.pending_jobs.lock().unwrap().insert(id, payload);
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
        if let Some((id, payload)) = start {
            self.spawn_download(id, payload);
        }
        self.notify_now();
    }

    /// Starts the download task for `id`, registering its cancellation flag
    /// before the task exists so an immediate `cancel` is never lost.
    fn spawn_download(self: &Arc<Self>, id: u64, payload: JobPayload) {
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
                let mut payload = payload;
                let mut on_progress = move |downloaded, total, speed| {
                    reporter.on_download_progress(id, downloaded, total, speed);
                };
                let result = match &mut payload {
                    JobPayload::Game(job) => {
                        download_targets(
                            &RommProvider(&job.client),
                            &job.targets,
                            &cancel,
                            &mut on_progress,
                        )
                        .await
                    }
                    JobPayload::Emulator(job) => {
                        download_emulator(job, &cancel, &mut on_progress).await
                    }
                };
                (payload, result)
            });
            let outcome = worker.await;
            service.cancel_flags.lock().unwrap().remove(&id);
            match outcome {
                Ok((payload, result)) => service.finish_download(id, payload, result).await,
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
        payload: JobPayload,
        result: Result<(), LibraryError>,
    ) {
        // doc 03 §1 step 4: a game that is already in the registry completes
        // straight away — the bytes are on disk, nothing needs installing.
        // An emulator never short-circuits: the registry is games-only, so
        // there is no "already installed" row to find, and the extract /
        // config-write half still has to run.
        let skip_finalize = match &payload {
            JobPayload::Game(job) => result.is_ok() && self.already_installed(&job.detail).await,
            JobPayload::Emulator(_) => false,
        };
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
            self.spawn_finalize(id, payload);
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
    fn spawn_finalize(self: &Arc<Self>, id: u64, payload: JobPayload) {
        let service = self.clone();
        tokio::spawn(async move {
            let worker = service.clone();
            let (result, warning) =
                match tokio::task::spawn_blocking(move || worker.finalize(id, &payload)).await {
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
                    Some(payload) => Some((id, payload)),
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
            if let Some((id, payload)) = start {
                self.spawn_download(id, payload);
                return;
            }
        }
    }

    /// The blocking half of finalizing: extraction, launch selection,
    /// registry write and archive cleanup. Returns the outcome plus a
    /// warning string that is empty unless the install succeeded with a
    /// caveat.
    fn finalize(&self, id: u64, payload: &JobPayload) -> (Result<(), LibraryError>, String) {
        let mut warning = String::new();
        let result = match payload {
            JobPayload::Game(job) => self.finalize_inner(id, job, &mut warning),
            JobPayload::Emulator(job) => self.finalize_emulator(id, job, &mut warning),
        };
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

    /// The blocking half of finalizing an emulator: extraction into the
    /// install directory, supplemental merges, executable selection, the
    /// config entry, and archive cleanup. Ports the emulator half of the
    /// reference's post-download install path (`install_mixin.py:690-780`,
    /// `autoconfig.py:19-87`); nothing here writes to the SQLite registry.
    fn finalize_emulator(
        &self,
        id: u64,
        job: &EmulatorJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        // Unreachable: the download task fills `resolved` in before it can
        // succeed, and only a successful download reaches finalize.
        let Some(paths) = job.resolved.as_ref() else {
            return Err(LibraryError::Extract(
                "the emulator download plan was lost".to_string(),
            ));
        };
        let install_dir = paths.install_dir.as_path();
        let archive = paths.archive.as_path();
        let mut progress = |processed, total| self.on_install_progress(id, processed, total);
        // Archives that have been superseded by their extracted contents.
        // Anything left out of this list stays on disk: an AppImage IS the
        // install, and a failure before this point must keep every archive so
        // a retry skips the finished downloads (doc 03 invariant 5).
        let mut extracted_archives: Vec<&Path> = Vec::new();

        if should_extract(EMULATOR_PLATFORM, archive) {
            let staging = install_dir.join(EXTRACT_TMP_DIR);
            extract_archive(archive, &staging, &mut progress)?;
            let merged = merge_tree_into(&staging, install_dir);
            let _ = fs::remove_dir_all(&staging);
            merged?;
            extracted_archives.push(archive);
        }

        for (index, supplemental) in paths.supplementals.iter().enumerate() {
            if !supplemental.is_file() {
                continue;
            }
            // A supplemental that is not an archive (an AppImage, a firmware
            // blob) is already where it belongs: a sibling of the primary.
            if !should_extract(EMULATOR_PLATFORM, supplemental) {
                continue;
            }
            // Numbered by position in the downloaded list, which is a
            // scratch name only — the file names themselves carry the
            // reference's spec index, assigned when the plan was built.
            let staging = install_dir.join(format!(".supp-tmp-{}", index + 1));
            extract_archive(supplemental, &staging, &mut progress)?;
            let merged = merge_tree_into(&staging, install_dir);
            let _ = fs::remove_dir_all(&staging);
            merged?;
            extracted_archives.push(supplemental);
        }

        let Some(exe) = emu_install::select_executable(&job.profile_name, install_dir, archive)
        else {
            return Err(LibraryError::Extract(NO_EMULATOR_EXECUTABLE.to_string()));
        };
        make_executable(&exe);

        self.write_emulator_entry(job, &paths.resolved, &exe)?;

        // Only after a successful config write, matching the game path.
        for path in extracted_archives {
            if !delete_with_retry(path) {
                append_warning(
                    warning,
                    &format!("could not delete archive: {}", path.display()),
                );
            }
        }
        Ok(())
    }

    /// Writes (or replaces) `job`'s emulator entry in the config file.
    ///
    /// An existing entry with the same name is replaced AT ITS INDEX, so the
    /// user's ordering survives a reinstall; the match is exact, mirroring
    /// the `save_emulator` command's replace rule.
    fn write_emulator_entry(
        &self,
        job: &EmulatorJob,
        resolved: &ResolvedDownload,
        exe: &Path,
    ) -> Result<(), LibraryError> {
        let mut config = Config::load(&self.config_path)?;
        let entry = EmulatorEntry {
            name: job.profile_name.clone(),
            path: path_string(exe),
            args: job.profile_args.clone(),
            source_id: job.source_id.clone(),
            source_provider: resolved.provider.clone(),
            source_owner: resolved.owner.clone(),
            source_repo: resolved.repo.clone(),
            // The CONFIGURED tag, not the resolved one — a `latest` pin is
            // recorded as `latest` so the entry keeps meaning "track the
            // newest release" (emulator_ui_mixin.py:1168-1175).
            source_release_tag: job.configured_tag.clone(),
            // The layer-1 autoconfig fields are not this writer's concern:
            // `autoconfig::entry` fills them on the next configure pass.
            ..Default::default()
        };
        match config
            .emulators
            .iter()
            .position(|existing| existing.name == entry.name)
        {
            Some(index) => config.emulators[index] = entry,
            None => config.emulators.push(entry),
        }
        config.save(&self.config_path)?;
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

// --- emulator download ------------------------------------------------------

/// Resolves `job` against its forge and downloads the primary asset plus
/// every supplemental, recording the resulting paths on `job` for finalize.
///
/// Ports `InstallDownloadWorker.run`'s resolve-then-download order
/// (`workers.py:57-138`): the release lookups happen here, inside the
/// download task, so a resolution failure ends up on the drawer row.
async fn download_emulator(
    job: &mut EmulatorJob,
    cancel: &AtomicBool,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
) -> Result<(), LibraryError> {
    let forge = job.forge.clone();
    let primary = forge
        .resolve(&job.raw_source, &job.profile_name)
        .await
        .map_err(|e| LibraryError::Extract(e.0))?;
    // A cancel that arrived while the forge was being queried must not turn
    // into a download.
    if cancel.load(Ordering::Relaxed) {
        return Err(LibraryError::Cancelled);
    }

    let supplementals = resolve_supplementals(&forge, job).await?;

    // The install directory is named from the profile name and the
    // CONFIGURED tag, fixed before the asset is known — `_archive_name_override`
    // with no asset suffix applied, then its stem (emulator_ui_mixin.py:1186-1190
    // + install_mixin.py:1444). The asset-suffix rewrite below renames only
    // the file INSIDE that already-fixed directory.
    let dir_name = emu_install::archive_file_name(&job.profile_name, &job.configured_tag, "");
    let stem = file_stem_of(&dir_name);
    let install_dir = emu_install::emulator_install_dir(&job.library, &stem);

    let archive_name =
        emu_install::archive_file_name(&job.profile_name, &job.configured_tag, &primary.asset_name);
    let archive = install_dir.join(safe_file_name(&archive_name, &primary.asset_name)?);

    // Every destination name is validated before ANY request goes out, so a
    // hostile asset name cannot write a byte anywhere.
    let mut supplemental_paths = Vec::new();
    for (index, supplemental) in &supplementals {
        let name = emu_install::supplemental_file_name(&archive, *index, &supplemental.asset_name);
        supplemental_paths.push(install_dir.join(safe_file_name(&name, &supplemental.asset_name)?));
    }

    let mut targets = vec![FileTarget {
        url_path: primary.download_url.clone(),
        query: Vec::new(),
        dest: archive.clone(),
        expected_size: primary.size,
    }];
    let mut github_headers = vec![(
        primary.download_url.clone(),
        needs_github_headers(&primary.provider),
    )];

    for ((_, supplemental), dest) in supplementals.iter().zip(&supplemental_paths) {
        targets.push(FileTarget {
            url_path: supplemental.download_url.clone(),
            query: Vec::new(),
            dest: dest.clone(),
            expected_size: supplemental.size,
        });
        github_headers.push((
            supplemental.download_url.clone(),
            needs_github_headers(&supplemental.provider),
        ));
    }

    job.resolved = Some(ResolvedPaths {
        install_dir,
        archive,
        supplementals: supplemental_paths,
        resolved: primary,
    });

    let provider = ForgeProvider::new(&forge, github_headers);
    download_targets(&provider, &targets, cancel, on_progress).await
}

/// Resolves every `supplemental_downloads` entry of `job`'s RAW source,
/// paired with its 1-based index (`_download_supplemental_archives`,
/// workers.py:124-138).
///
/// The reference numbers the entries BEFORE skipping the unusable ones, so
/// a skipped spec still consumes its index and the file names of the
/// supplementals after it do not shift. A spec that resolves to no download
/// URL is skipped; a spec that fails to resolve fails the whole install.
async fn resolve_supplementals(
    forge: &ForgeClient,
    job: &EmulatorJob,
) -> Result<Vec<(usize, ResolvedDownload)>, LibraryError> {
    let Some(Value::Array(specs)) = job.raw_source.get("supplemental_downloads") else {
        return Ok(Vec::new());
    };
    let mut resolved = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if !spec.is_object() {
            continue;
        }
        let supplemental = forge
            .resolve(spec, &job.profile_name)
            .await
            .map_err(|e| LibraryError::Extract(e.0))?;
        if supplemental.download_url.trim().is_empty() {
            continue;
        }
        resolved.push((index + 1, supplemental));
    }
    Ok(resolved)
}

/// `name`'s file stem, or the whole name when it has none.
fn file_stem_of(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// Returns `name` when it is a single, ordinary file name, and an error
/// naming the offending asset otherwise.
///
/// The naming helpers copy a release asset's own name into the result (whole
/// for an AppImage, its suffix otherwise), and that name is remote data: it
/// comes from a release payload served by whatever host a `gitea` or
/// `direct` source's `base_url`/`page_url` points at. Joining `../../x` onto
/// the install directory would write outside the library, so it is rejected
/// here, in the wiring, before the path is built.
///
/// The reference gets this for free — `Path.with_name`/`with_suffix` raise
/// `ValueError` on a name containing a separator (workers.py:147-163) — so
/// failing the job is also the reference's behavior. Both separators are
/// rejected on every platform: a config written on one OS is read on the
/// other.
fn safe_file_name<'a>(name: &'a str, asset_name: &str) -> Result<&'a str, LibraryError> {
    let is_plain_component = !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && Path::new(name).file_name().is_some_and(|only| only == name);
    if is_plain_component {
        return Ok(name);
    }
    Err(LibraryError::Extract(format!(
        "Refusing to install release asset '{asset_name}': it does not name a plain file."
    )))
}

// --- record + filesystem helpers --------------------------------------------

/// Appends `line` to `warning`, keeping any warning already there.
fn append_warning(warning: &mut String, line: &str) {
    if !warning.is_empty() {
        warning.push('\n');
    }
    warning.push_str(line);
}

/// Merges every entry of `src` into `dest`: directories are created (and
/// recursed into when they already exist), files overwrite their
/// counterparts, and anything in `dest` the tree does not carry is left
/// alone — the merge semantics of doc 03 / `_merge_tree`
/// (`archive_preparation.py:258-267`).
///
/// Entries are MOVED rather than copied. `src` is always a staging directory
/// inside `dest` that is deleted immediately afterwards, so a rename cannot
/// cross a filesystem boundary and the result is identical to a copy with
/// half the writes.
fn merge_tree_into(src: &Path, dest: &Path) -> Result<(), LibraryError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        // `file_type` does not follow symlinks, so a symlink is moved as
        // itself rather than being descended into.
        let from_is_dir = entry.file_type()?.is_dir();

        if from_is_dir && to.is_dir() {
            merge_tree_into(&from, &to)?;
            // Now empty; a failure to remove the husk is not worth failing
            // the install for, and the whole staging tree is deleted next.
            let _ = fs::remove_dir(&from);
            continue;
        }
        if to.is_dir() {
            remove_dir_tree(&to)?;
        } else if to.exists() {
            fs::remove_file(&to)?;
        }
        fs::rename(&from, &to)?;
    }
    Ok(())
}

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
    fn safe_file_name_accepts_a_plain_component() {
        for name in [
            "Test Emu-v1.0.zip",
            "eden-linux.AppImage",
            ".hidden",
            "no-extension",
            "spaced out name.zip",
        ] {
            assert_eq!(safe_file_name(name, "asset").unwrap(), name);
        }
    }

    #[test]
    fn safe_file_name_rejects_anything_that_is_not_one_file_name() {
        for name in [
            "../../evil.AppImage",
            "../evil.zip",
            "a/b.zip",
            "/abs.zip",
            "dir/",
            "a\\b.zip",
            "..",
            ".",
            "",
        ] {
            let Err(err) = safe_file_name(name, "hostile-asset") else {
                panic!("expected {name:?} to be rejected");
            };
            assert_eq!(
                err.to_string(),
                "Refusing to install release asset 'hostile-asset': it does not name a plain file."
            );
        }
    }

    #[test]
    fn file_stem_of_drops_only_the_last_extension() {
        assert_eq!(file_stem_of("Test Emu-v1.0.zip"), "Test Emu-v1.0");
        assert_eq!(file_stem_of("no-extension"), "no-extension");
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
