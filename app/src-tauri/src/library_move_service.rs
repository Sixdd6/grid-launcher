//! The library move job (Settings › Library › Move existing files, phase
//! 9b) on the app side: WHEN the grid-core engine (`library::relocate`)
//! runs, and what the rest of the app sees while it does.
//!
//! Order on start, which keeps a move from ever racing an install, a
//! launch or a cloud transfer:
//!
//! 1. plan the move (read only; a refusal changes nothing);
//! 2. create the Downloads row (`begin_library_move`; `None` = a move is
//!    already live, so there are never two);
//! 3. close the [`LibraryGate`] — from here installs, launches, uninstalls,
//!    cloud transfers and firmware passes are refused;
//! 4. wait for the gate tickets (a launch that passed the gate a moment ago
//!    but has not registered yet) to drop;
//! 5. read what is running again; busy → reopen the gate, drop the row,
//!    refuse;
//! 6. `start_move` (creates the new root, writes the journal);
//! 7. run the job on the blocking pool.
//!
//! The gate stays closed while a journal exists: a failed move keeps the
//! library locked until it is resumed (Retry, or the next start) or
//! cancelled (Copy phase only). The journal is the only state that
//! survives a crash; at startup [`LibraryMoveService::new`] closes the gate
//! when it finds one, [`LibraryMoveService::check_root_at_startup`] decides
//! whether the library root can be reached at all, and
//! [`LibraryMoveService::recover_at_startup`] finishes an interrupted
//! Commit before any service reads the config. Copy and Cleanup resume in
//! the background ([`LibraryMoveService::resume`]).
//!
//! **Unreachable library.** When the library root (the old root during
//! Copy, the new root after Commit, the configured root otherwise) cannot
//! be reached and GRID has something stored there, the app must not run on
//! it: the gate closes and the frontend shows a blocking dialog with two
//! choices, [`LibraryMoveService::relocate`] (references rewritten to the
//! folder where the library now is; no file moves) or close the app.
//!
//! Nothing here carries a secret: payloads, events and errors hold paths,
//! sizes and counts only.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use grid_core::config::Config;
use grid_core::launch::profiles::load_profiles;
use grid_core::library::path_change::{LibraryActivity, PathChangeRefusal};
use grid_core::library::registry::Registry;
use grid_core::library::relocate::{
    self, FreeSpace, Journal, LibraryGate, MoveError, MoveHooks, MoveOutcome, MovePreview,
    MoveProgress, MoveRefusal, Phase, PlanEnv, Recovery, StrategyProbe, LIBRARY_MOVE_BUSY,
};
use grid_core::library::root_access::{
    library_in_use, library_root_access, validate_relocate_target, RootAccess, UnreachableReason,
};
use grid_core::library::InstallService;
use serde::Serialize;

use crate::config_write::modify_config;

/// The Downloads row's title.
pub const LIBRARY_MOVE_TITLE: &str = "Library move";
/// The move's state changed: started, phase, finished, cancelled.
pub const LIBRARY_MOVE_CHANGED_EVENT: &str = "library-move-changed";
/// A run ended; payload [`MoveFinished`].
pub const LIBRARY_MOVE_FINISHED_EVENT: &str = "library-move-finished";
/// The library root became unreachable while the app runs.
pub const LIBRARY_UNREACHABLE_EVENT: &str = "library-unreachable";
/// `library_path` changed (a finished move or a relocate).
pub const LIBRARY_PATH_CHANGED_EVENT: &str = "library-path-changed";

/// The gate's reason while the library root cannot be reached.
pub const LIBRARY_UNREACHABLE_BUSY: &str =
    "The library folder cannot be reached. Relocate the library or close GRID.";

/// How long a start waits for a launch that already passed the gate.
const TICKET_TIMEOUT: Duration = Duration::from_secs(10);
/// The Downloads row is updated at most this often while bytes move.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

const COMMIT_DETAIL: &str = "Updating paths…";
const CLEANUP_DETAIL: &str = "Removing the old copies…";

/// A refusal as the frontend sees it: a snake_case `reason` (a
/// [`MoveRefusal`] or a [`PathChangeRefusal`] tag, or `library_unreachable`)
/// and the sentence to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    pub reason: String,
    pub message: String,
}

impl Refusal {
    fn of(refusal: MoveRefusal) -> Self {
        let reason = match refusal {
            MoveRefusal::PathChange(inner) => tag(&inner),
            other => tag(&other),
        };
        Self {
            reason,
            message: refusal.message().to_string(),
        }
    }

    fn unreachable() -> Self {
        Self {
            reason: "library_unreachable".to_string(),
            message: LIBRARY_UNREACHABLE_BUSY.to_string(),
        }
    }
}

/// A unit enum's serde tag (`"game_running"`).
fn tag(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PreviewOutcome {
    Ok { preview: MovePreview },
    Refused(Refusal),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StartOutcome {
    /// The job runs; its Downloads row shows the progress.
    Started {
        new_root: String,
    },
    Refused(Refusal),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CancelOutcome {
    /// The running copy stops at the next chunk and rolls back.
    Cancelling,
    /// A stopped move was rolled back: the old library is unchanged.
    Cancelled,
    /// The files have already moved.
    NotCancellable {
        message: String,
    },
    NoMove {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ResumeOutcome {
    Resumed { new_root: String },
    Refused(Refusal),
}

/// What `library_move_status` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MoveStatus {
    /// The journal's phase; `None` when no move is in progress.
    pub phase: Option<Phase>,
    /// The job runs right now.
    pub running: bool,
    pub old_root: String,
    pub new_root: String,
    /// The move was resumed at startup (the frontend shows "Resuming the
    /// library move to <new>" once).
    pub resumed: bool,
    /// Why the last run failed, while its journal waits.
    pub error: Option<String>,
}

/// Payload of [`LIBRARY_MOVE_FINISHED_EVENT`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MoveFinished {
    Finished {
        new_root: String,
        report: Vec<String>,
        old_root_removed: bool,
    },
    Cancelled,
    Failed {
        error: String,
    },
}

/// What `library_unreachable_status` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UnreachableStatus {
    Reachable,
    Unreachable {
        root: String,
        reason: UnreachableReason,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RelocateOutcome {
    Relocated {
        library_path: String,
        report: Vec<String>,
    },
    Refused(Refusal),
}

/// What the service tells the rest of the app. The app turns these into
/// Tauri events (and, for `LibraryChanged`, the update refresh).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveEvent {
    Changed,
    Finished(MoveFinished),
    Unreachable,
    /// `library_path` and the stored paths changed.
    LibraryChanged,
}

type ActivityFn = dyn Fn() -> LibraryActivity + Send + Sync;
type ResyncFn = dyn Fn(&Config) -> Result<(), String> + Send + Sync;
type EventFn = dyn Fn(MoveEvent) + Send + Sync;
type ProgressFn = dyn Fn(&MoveProgress) + Send + Sync;

/// What one operation needs from the app. Built per call from `AppState`
/// and the `AppHandle`; the tests build their own.
#[derive(Clone)]
pub struct MoveContext {
    pub install: Arc<InstallService>,
    /// What runs right now (downloads, games, cloud transfers).
    pub activity: Arc<ActivityFn>,
    /// The managed-key resync after Commit: `run_emulator_sync`,
    /// `fresh_install = false`, for every emulator entry.
    pub resync: Arc<ResyncFn>,
    pub events: Arc<EventFn>,
    /// Test hook: sees every progress report.
    pub on_progress: Option<Arc<ProgressFn>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Unreachable {
    root: String,
    reason: UnreachableReason,
}

/// Owns the gate, the journal path, the cancel flag and the
/// unreachable-library state. One per process, in `AppState`.
pub struct LibraryMoveService {
    gate: LibraryGate,
    config_path: PathBuf,
    journal_path: PathBuf,
    cancel: AtomicBool,
    running: AtomicBool,
    /// Serializes start, resume, cancel and relocate decisions.
    control: Mutex<()>,
    unreachable: Mutex<Option<Unreachable>>,
    pending_resume: AtomicBool,
    resumed: AtomicBool,
    last_error: Mutex<Option<String>>,
    probe: Arc<StrategyProbe>,
    free_space: Arc<FreeSpace>,
    ticket_timeout: Duration,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `1.2 of 5.6 GB`: both numbers in the unit of the total.
pub fn size_pair(done: u64, total: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut unit = 0;
    let mut scale = 1u64;
    while total / scale >= 1024 && unit < UNITS.len() - 1 {
        scale *= 1024;
        unit += 1;
    }
    if unit == 0 {
        return format!("{done} of {total} B");
    }
    let f = |bytes: u64| bytes as f64 / scale as f64;
    format!("{:.1} of {:.1} {}", f(done), f(total), UNITS[unit])
}

/// The Downloads row's detail line while an item copies:
/// `games/ · 1.2 of 5.6 GB`.
pub fn copy_detail(item: &str, done: u64, total: u64) -> String {
    format!("{item}/ · {}", size_pair(done, total))
}

impl LibraryMoveService {
    /// The service for the config at `config_path`. The gate starts closed
    /// when a journal exists: nothing may touch the library before the
    /// move resumes or is cancelled.
    pub fn new(config_path: PathBuf) -> Arc<Self> {
        static PROBE: fn(&Path, &Path) -> std::io::Result<relocate::Strategy> =
            relocate::probe_strategy;
        static FREE: fn(&Path) -> Option<u64> = relocate::free_space;
        Self::with_env(config_path, Arc::new(PROBE), Arc::new(FREE), TICKET_TIMEOUT)
    }

    /// [`Self::new`] with the strategy probe, the free-space reader and
    /// the ticket wait injected (tests).
    pub fn with_env(
        config_path: PathBuf,
        probe: Arc<StrategyProbe>,
        free_space: Arc<FreeSpace>,
        ticket_timeout: Duration,
    ) -> Arc<Self> {
        let journal_path = relocate::journal_path(&config_path);
        let gate = if journal_path.exists() {
            LibraryGate::closed(LIBRARY_MOVE_BUSY)
        } else {
            LibraryGate::default()
        };
        Arc::new(Self {
            gate,
            config_path,
            journal_path,
            cancel: AtomicBool::new(false),
            running: AtomicBool::new(false),
            control: Mutex::new(()),
            unreachable: Mutex::new(None),
            pending_resume: AtomicBool::new(false),
            resumed: AtomicBool::new(false),
            last_error: Mutex::new(None),
            probe,
            free_space,
            ticket_timeout,
        })
    }

    /// The gate every library-touching command and service checks.
    pub fn gate(&self) -> &LibraryGate {
        &self.gate
    }

    /// `Err` with the gate's reason while a move runs or waits, or while
    /// the library cannot be reached.
    pub fn ensure_idle(&self) -> Result<(), String> {
        self.gate.check().map_err(|busy| busy.reason)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    // --- startup ------------------------------------------------------------

    /// Startup, before the layout migration: can the library root be
    /// reached? `false` (and the gate closed, the unreachable state set)
    /// when it cannot and GRID has something stored there. The caller then
    /// skips everything that would write into the library.
    ///
    /// The root checked is the one the app would run on: the old root while
    /// a move is still copying, the new root once it has committed, the
    /// configured root otherwise. A blank root is a first run, never
    /// unreachable.
    pub fn check_root_at_startup(&self, registry: &Registry) -> bool {
        let Ok(config) = Config::load(&self.config_path) else {
            // An unreadable config is reported where it is read; it says
            // nothing about the library folder.
            return true;
        };
        let journal = Journal::load(&self.journal_path).ok().flatten();
        let root = library_root_of(journal.as_ref(), &config);
        let home = grid_core::platform::home_dir();
        let RootAccess::Unreachable(reason) = library_root_access(&root, home.as_deref()) else {
            return true;
        };
        // A folder GRID never stored anything in is created again by the
        // next install. A registry that cannot be read counts as in use:
        // the app must not guess.
        let in_use = journal.is_some()
            || registry.all().map_or(true, |rows| {
                library_in_use(&config, &rows, &expand_root(&root), home.as_deref())
            });
        if !in_use {
            return true;
        }
        tracing::warn!(
            reason = ?reason,
            "the library folder cannot be reached: {}",
            root.trim()
        );
        self.set_unreachable(root.trim().to_string(), reason);
        false
    }

    /// Startup, after the layout migration and before the services: a move
    /// interrupted in Commit is finished now; Copy and Cleanup are left for
    /// [`Self::resume`] (`pending_resume`).
    pub fn recover_at_startup(&self, registry: &Registry, resync: &ResyncFn) {
        if lock(&self.unreachable).is_some() {
            return;
        }
        match Journal::load(&self.journal_path) {
            Ok(Some(_)) => {}
            Ok(None) => return,
            Err(e) => {
                // The gate stays closed: a move record GRID cannot read
                // must not be guessed at.
                tracing::warn!("library move record not read: {e}");
                *lock(&self.last_error) = Some(e.to_string());
                return;
            }
        }
        let result = self.with_hooks(registry, resync, &|_| {}, None, |hooks| {
            relocate::recover(&self.journal_path, hooks)
        });
        match result {
            Ok(Recovery::None) => self.open_gate_if_clear(),
            Ok(Recovery::Copy(_) | Recovery::Cleanup(_)) => {
                self.pending_resume.store(true, Ordering::SeqCst);
            }
            Err(MoveError::RootUnreachable(path)) => {
                tracing::warn!("library move not recovered: {path} cannot be reached");
                self.note_unreachable(&path);
            }
            Err(e) => {
                // The background run retries the Commit and fails visibly.
                tracing::warn!("library move not recovered at startup: {e}");
                *lock(&self.last_error) = Some(e.to_string());
                self.pending_resume.store(true, Ordering::SeqCst);
            }
        }
    }

    /// True when startup left a Copy or Cleanup phase to resume.
    pub fn has_pending_resume(&self) -> bool {
        self.pending_resume.load(Ordering::SeqCst)
    }

    // --- the move -----------------------------------------------------------

    fn plan_env(&self) -> PlanEnv<'_> {
        PlanEnv {
            probe: &*self.probe,
            free_space: &*self.free_space,
        }
    }

    /// What a move to `path` would do. Reads only.
    pub fn preview(&self, ctx: &MoveContext, path: &str) -> Result<PreviewOutcome, String> {
        if lock(&self.unreachable).is_some() {
            return Ok(PreviewOutcome::Refused(Refusal::unreachable()));
        }
        let config = Config::load(&self.config_path).map_err(|e| e.to_string())?;
        let activity = (ctx.activity)();
        match relocate::plan_move(
            &config,
            path,
            activity,
            &self.journal_path,
            &self.plan_env(),
        ) {
            Ok(preview) => Ok(PreviewOutcome::Ok { preview }),
            Err(MoveError::Refused(refusal)) => Ok(PreviewOutcome::Refused(Refusal::of(refusal))),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Starts a move to `path` (see the module docs for the order).
    pub fn start(self: &Arc<Self>, ctx: &MoveContext, path: &str) -> Result<StartOutcome, String> {
        let _control = lock(&self.control);
        if lock(&self.unreachable).is_some() {
            return Ok(StartOutcome::Refused(Refusal::unreachable()));
        }
        if self.is_running() {
            return Ok(StartOutcome::Refused(Refusal::of(MoveRefusal::MoveActive)));
        }
        // 1. Plan, before anything is created.
        let config = Config::load(&self.config_path).map_err(|e| e.to_string())?;
        let preview = match relocate::plan_move(
            &config,
            path,
            (ctx.activity)(),
            &self.journal_path,
            &self.plan_env(),
        ) {
            Ok(preview) => preview,
            Err(MoveError::Refused(refusal)) => {
                return Ok(StartOutcome::Refused(Refusal::of(refusal)))
            }
            Err(e) => return Err(e.to_string()),
        };
        // 2. The row. `None`: a move row is already live.
        let Some(id) = ctx
            .install
            .begin_library_move(LIBRARY_MOVE_TITLE, &preview.new_root)
        else {
            return Ok(StartOutcome::Refused(Refusal::of(MoveRefusal::MoveActive)));
        };
        let drop_row = |message: &str| {
            ctx.install.finish_library_move(id, message);
            ctx.install.dismiss(id);
        };
        let refuse = |refusal: MoveRefusal, reopen: bool| {
            if reopen {
                self.gate.open();
            }
            let refusal = Refusal::of(refusal);
            drop_row(&refusal.message);
            Ok(StartOutcome::Refused(refusal))
        };
        // 3. Close the gate. Already closed: someone else holds the library.
        if !self.gate.close(LIBRARY_MOVE_BUSY) {
            return refuse(MoveRefusal::MoveActive, false);
        }
        // 4. A launch that passed the gate a moment ago registers first.
        if !self.gate.wait_no_tickets(self.ticket_timeout) {
            return refuse(
                MoveRefusal::PathChange(PathChangeRefusal::GameRunning),
                true,
            );
        }
        // 5 + 6. Read the activity again, with the gate closed. Our own
        // row is not activity; the journal check covers a second move.
        let mut activity = (ctx.activity)();
        activity.library_move = false;
        let started = Config::load(&self.config_path)
            .map_err(|e| MoveError::Io(e.to_string()))
            .and_then(|config| {
                relocate::start_move(
                    &config,
                    path,
                    activity,
                    &self.journal_path,
                    &self.plan_env(),
                )
            });
        match started {
            Ok(_) => {}
            Err(MoveError::Refused(refusal)) => return refuse(refusal, true),
            Err(e) => {
                let message = e.to_string();
                if !self.journal_path.exists() {
                    self.gate.open();
                }
                drop_row(&message);
                return Err(message);
            }
        }
        tracing::info!(
            items = preview.items.len(),
            copy_bytes = preview.copy_bytes,
            "library move started: {} -> {}",
            preview.old_root,
            preview.new_root
        );
        *lock(&self.last_error) = None;
        self.resumed.store(false, Ordering::SeqCst);
        // 7. Run.
        self.spawn_run(ctx.clone(), id);
        (ctx.events)(MoveEvent::Changed);
        Ok(StartOutcome::Started {
            new_root: preview.new_root,
        })
    }

    /// Resumes the move in the journal: at startup, or Retry on a failed
    /// row.
    pub fn resume(self: &Arc<Self>, ctx: &MoveContext) -> Result<ResumeOutcome, String> {
        let _control = lock(&self.control);
        if lock(&self.unreachable).is_some() {
            return Ok(ResumeOutcome::Refused(Refusal::unreachable()));
        }
        if self.is_running() {
            return Ok(ResumeOutcome::Refused(Refusal::of(MoveRefusal::MoveActive)));
        }
        let journal = match Journal::load(&self.journal_path) {
            Ok(Some(journal)) => journal,
            Ok(None) => {
                return Ok(ResumeOutcome::Refused(Refusal {
                    reason: "no_move".to_string(),
                    message: MoveError::NoJournal.to_string(),
                }))
            }
            Err(e) => return Err(e.to_string()),
        };
        // Closed since the journal was written; closing again is a no-op.
        self.gate.close(LIBRARY_MOVE_BUSY);
        let Some(id) = ctx
            .install
            .begin_library_move(LIBRARY_MOVE_TITLE, &journal.new_root)
        else {
            return Ok(ResumeOutcome::Refused(Refusal::of(MoveRefusal::MoveActive)));
        };
        if self.pending_resume.swap(false, Ordering::SeqCst) {
            self.resumed.store(true, Ordering::SeqCst);
        }
        *lock(&self.last_error) = None;
        tracing::info!("library move resumed: {}", journal.new_root);
        self.spawn_run(ctx.clone(), id);
        (ctx.events)(MoveEvent::Changed);
        Ok(ResumeOutcome::Resumed {
            new_root: journal.new_root,
        })
    }

    /// Cancels the move while it copies: a running copy stops at its next
    /// chunk and rolls back; a stopped one rolls back now. After Copy:
    /// `NotCancellable`.
    pub fn cancel(&self, ctx: &MoveContext) -> Result<CancelOutcome, String> {
        let _control = lock(&self.control);
        let journal = match Journal::load(&self.journal_path) {
            Ok(Some(journal)) => journal,
            Ok(None) => {
                return Ok(CancelOutcome::NoMove {
                    message: MoveError::NoJournal.to_string(),
                })
            }
            Err(e) => return Err(e.to_string()),
        };
        if journal.phase != Phase::Copy {
            return Ok(CancelOutcome::NotCancellable {
                message: MoveError::NotCancellable.to_string(),
            });
        }
        if self.is_running() {
            self.cancel.store(true, Ordering::SeqCst);
            return Ok(CancelOutcome::Cancelling);
        }
        relocate::cancel(&self.journal_path).map_err(|e| e.to_string())?;
        tracing::info!("library move cancelled: {}", journal.new_root);
        *lock(&self.last_error) = None;
        self.open_gate_if_clear();
        (ctx.events)(MoveEvent::Finished(MoveFinished::Cancelled));
        (ctx.events)(MoveEvent::Changed);
        Ok(CancelOutcome::Cancelled)
    }

    pub fn status(&self) -> MoveStatus {
        let journal = Journal::load(&self.journal_path).ok().flatten();
        let (phase, old_root, new_root) = match journal {
            Some(journal) => (Some(journal.phase), journal.old_root, journal.new_root),
            None => (None, String::new(), String::new()),
        };
        MoveStatus {
            phase,
            running: self.is_running(),
            old_root,
            new_root,
            resumed: self.resumed.load(Ordering::SeqCst),
            error: lock(&self.last_error).clone(),
        }
    }

    // --- the run ------------------------------------------------------------

    fn spawn_run(self: &Arc<Self>, ctx: MoveContext, id: u64) {
        self.cancel.store(false, Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);
        let service = self.clone();
        tauri::async_runtime::spawn_blocking(move || service.run_job(&ctx, id));
    }

    fn run_job(&self, ctx: &MoveContext, id: u64) {
        /// Clears `running` even when the run panics.
        struct Running<'a>(&'a AtomicBool);
        impl Drop for Running<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _running = Running(&self.running);

        // The bar covers the copied bytes: each item starts where the
        // copied items before it end.
        let journal = Journal::load(&self.journal_path).ok().flatten();
        let mut offsets = std::collections::HashMap::new();
        let mut copy_total = 0u64;
        let mut phase = Phase::Copy;
        if let Some(journal) = &journal {
            phase = journal.phase;
            for item in &journal.items {
                if item.strategy == relocate::Strategy::Copy {
                    offsets.insert(item.name.clone(), copy_total);
                    copy_total += item.bytes;
                }
            }
        }
        let show_phase = |phase: Phase| match phase {
            Phase::Copy => ctx.install.set_library_move_copying(id, true),
            Phase::Commit | Phase::Cleanup => {
                ctx.install.set_library_move_copying(id, false);
                let detail = if phase == Phase::Commit {
                    COMMIT_DETAIL
                } else {
                    CLEANUP_DETAIL
                };
                ctx.install
                    .set_library_move_progress(id, detail, copy_total, copy_total);
            }
        };
        show_phase(phase);

        let last = Mutex::new((None::<Instant>, String::new()));
        let progress = |p: &MoveProgress| {
            if let Some(hook) = &ctx.on_progress {
                hook(p);
            }
            let now = Instant::now();
            let mut last = lock(&last);
            let due = last.1 != p.item
                || p.done_bytes >= p.total_bytes
                || last
                    .0
                    .map_or(true, |at| now.duration_since(at) >= PROGRESS_INTERVAL);
            if !due {
                return;
            }
            *last = (Some(now), p.item.clone());
            let offset = offsets.get(&p.item).copied().unwrap_or(0);
            ctx.install.set_library_move_progress(
                id,
                &copy_detail(&p.item, p.done_bytes, p.total_bytes),
                offset + p.done_bytes,
                copy_total,
            );
        };
        // Watches the journal writes for the phase changes; never stops
        // the run.
        let seen = Mutex::new(phase);
        let observer = |journal: &Journal| {
            let mut seen = lock(&seen);
            if *seen != journal.phase {
                *seen = journal.phase;
                show_phase(journal.phase);
                (ctx.events)(MoveEvent::Changed);
            }
            false
        };
        let registry = ctx.install.registry();
        let result = self.with_hooks(
            &registry,
            &*ctx.resync,
            &progress,
            Some(&observer),
            |hooks| relocate::run(&self.journal_path, hooks),
        );
        self.finish_run(ctx, id, result);
    }

    fn finish_run(&self, ctx: &MoveContext, id: u64, result: Result<MoveOutcome, MoveError>) {
        match result {
            Ok(MoveOutcome::Finished(report)) => {
                tracing::info!(
                    notes = report.report.len(),
                    old_root_removed = report.old_root_removed,
                    "library move finished: {}",
                    report.new_root
                );
                ctx.install.finish_library_move(id, "");
                *lock(&self.last_error) = None;
                self.open_gate_if_clear();
                (ctx.events)(MoveEvent::LibraryChanged);
                (ctx.events)(MoveEvent::Finished(MoveFinished::Finished {
                    new_root: report.new_root,
                    report: report.report,
                    old_root_removed: report.old_root_removed,
                }));
            }
            Ok(MoveOutcome::Cancelled) => {
                tracing::info!("library move cancelled");
                ctx.install.cancel_library_move(id);
                *lock(&self.last_error) = None;
                self.open_gate_if_clear();
                (ctx.events)(MoveEvent::Finished(MoveFinished::Cancelled));
            }
            Err(error) => {
                let message = error.to_string();
                tracing::warn!("library move stopped: {message}");
                ctx.install.finish_library_move(id, &message);
                *lock(&self.last_error) = Some(message.clone());
                let journal = Journal::load(&self.journal_path).ok().flatten();
                if let MoveError::RootUnreachable(path) = &error {
                    // Only the root the app runs on blocks the app; an
                    // unplugged target during Copy is an ordinary failure.
                    let is_library_root = journal.as_ref().is_some_and(|journal| {
                        let root = library_root_of(Some(journal), &Config::default());
                        text(&expand_root(&root)) == *path
                    });
                    if is_library_root {
                        self.note_unreachable(path);
                        (ctx.events)(MoveEvent::Unreachable);
                    }
                }
                match &journal {
                    // Gone: nothing waits, the library is free again.
                    None => self.open_gate_if_clear(),
                    // Commit may have switched the root before it stopped.
                    Some(journal) if journal.phase != Phase::Copy => {
                        (ctx.events)(MoveEvent::LibraryChanged)
                    }
                    Some(_) => {}
                }
                (ctx.events)(MoveEvent::Finished(MoveFinished::Failed { error: message }));
            }
        }
        self.running.store(false, Ordering::SeqCst);
        (ctx.events)(MoveEvent::Changed);
    }

    /// Builds the engine's hooks around the app's single config writer.
    fn with_hooks<R>(
        &self,
        registry: &Registry,
        resync: &ResyncFn,
        progress: &(dyn Fn(&MoveProgress) + '_),
        observer: Option<&(dyn Fn(&Journal) -> bool + '_)>,
        f: impl FnOnce(&MoveHooks) -> R,
    ) -> R {
        let config_path = self.config_path.clone();
        let edit = move |change: &mut relocate::ConfigChange| {
            modify_config(&config_path, |config| change(config))
        };
        let free = |path: &Path| (self.free_space)(path);
        let hooks = MoveHooks {
            config_path: &self.config_path,
            registry,
            profiles: load_profiles(),
            edit_config: &edit,
            resync,
            free_space: &free,
            cancel: &self.cancel,
            progress,
            after_journal_write: observer,
        };
        f(&hooks)
    }

    /// Opens the gate unless a journal still waits or the library cannot
    /// be reached.
    fn open_gate_if_clear(&self) {
        if lock(&self.unreachable).is_none() && !self.journal_path.exists() {
            self.gate.open();
        }
    }

    // --- unreachable library --------------------------------------------------

    fn set_unreachable(&self, root: String, reason: UnreachableReason) {
        *lock(&self.unreachable) = Some(Unreachable { root, reason });
        // Keeps a move's reason when the gate is already closed.
        self.gate.close(LIBRARY_UNREACHABLE_BUSY);
    }

    /// The run or the recovery reported `path` unreachable: record why.
    fn note_unreachable(&self, path: &str) {
        let home = grid_core::platform::home_dir();
        let reason = match library_root_access(path, home.as_deref()) {
            RootAccess::Unreachable(reason) => reason,
            // Back already, or never set: still the root the move needs.
            RootAccess::Reachable | RootAccess::NotSet => UnreachableReason::Unreadable,
        };
        self.set_unreachable(path.to_string(), reason);
    }

    pub fn unreachable_status(&self) -> UnreachableStatus {
        match lock(&self.unreachable).clone() {
            None => UnreachableStatus::Reachable,
            Some(Unreachable { root, reason }) => UnreachableStatus::Unreachable {
                root,
                reason,
                message: reason.message().to_string(),
            },
        }
    }

    /// The dialog's "Relocate the library…": the library now lives at
    /// `chosen`. Rewrites every stored reference to the unreachable root;
    /// moves no file. With a move in progress: a Copy is rolled back (its
    /// source was the unreachable root), a Commit or Cleanup carries on to
    /// the chosen folder.
    pub fn relocate(
        self: &Arc<Self>,
        ctx: &MoveContext,
        chosen: &str,
    ) -> Result<RelocateOutcome, String> {
        let (report, resume_after) = {
            let _control = lock(&self.control);
            if self.is_running() {
                return Ok(RelocateOutcome::Refused(Refusal::of(
                    MoveRefusal::MoveActive,
                )));
            }
            if lock(&self.unreachable).is_none() {
                return Ok(RelocateOutcome::Refused(Refusal {
                    reason: "library_reachable".to_string(),
                    message: "The library folder can be reached. There is nothing to relocate."
                        .to_string(),
                }));
            }
            let home = grid_core::platform::home_dir();
            let target = match validate_relocate_target(chosen, home.as_deref()) {
                Ok(target) => target,
                Err(refusal) => {
                    return Ok(RelocateOutcome::Refused(Refusal {
                        reason: tag(&refusal),
                        message: refusal.message().to_string(),
                    }))
                }
            };
            let config = Config::load(&self.config_path).map_err(|e| e.to_string())?;
            let journal = Journal::load(&self.journal_path).map_err(|e| e.to_string())?;
            let registry = ctx.install.registry();
            let (report, resume_after) = self
                .with_hooks(&registry, &*ctx.resync, &|_| {}, None, |hooks| {
                    self.relocate_with(&config, journal, &target, hooks)
                })
                .map_err(|e| e.to_string())?;
            tracing::info!(
                notes = report.len(),
                "library relocated to {}",
                target.display()
            );
            // Paths only; the dialog's toast points here.
            for line in &report {
                tracing::warn!("library relocate: {line}");
            }
            *lock(&self.unreachable) = None;
            *lock(&self.last_error) = None;
            if resume_after {
                self.pending_resume.store(true, Ordering::SeqCst);
            } else {
                self.open_gate_if_clear();
            }
            (report, resume_after)
        };
        if resume_after {
            self.resume(ctx)?;
        }
        (ctx.events)(MoveEvent::LibraryChanged);
        (ctx.events)(MoveEvent::Changed);
        let library_path = Config::load(&self.config_path)
            .map_err(|e| e.to_string())?
            .library_path;
        Ok(RelocateOutcome::Relocated {
            library_path,
            report,
        })
    }

    /// The relocate's file work; `true` when a Cleanup is left to resume.
    fn relocate_with(
        &self,
        config: &Config,
        journal: Option<Journal>,
        target: &Path,
        hooks: &MoveHooks,
    ) -> Result<(Vec<String>, bool), MoveError> {
        let references = |old: &str| {
            if same_folder(old, target) {
                Ok(Vec::new())
            } else {
                relocate::relocate_references(old, target, hooks)
            }
        };
        match journal {
            None => Ok((references(&config.library_path)?, false)),
            // The copy's source was the unreachable root: drop the copy.
            Some(journal) if journal.phase == Phase::Copy => {
                relocate::cancel(&self.journal_path)?;
                Ok((references(&config.library_path)?, false))
            }
            // Committing or committed to a root that is now elsewhere:
            // whatever already points at the journal's new root follows it,
            // then the Commit finishes against the chosen folder.
            Some(mut journal) => {
                let notes = references(&journal.new_root)?;
                if !same_folder(&journal.new_root, target) {
                    journal.new_root = text(target);
                    journal.new_canonical = grid_core::platform::canonicalize(target)
                        .map(|resolved| text(&resolved))
                        .unwrap_or_default();
                    journal.save(&self.journal_path)?;
                }
                let recovery = relocate::recover(&self.journal_path, hooks)?;
                Ok((notes, recovery != Recovery::None))
            }
        }
    }
}

/// The root the app runs on: the old root while a move copies, the new
/// root once it committed, the configured root otherwise.
fn library_root_of(journal: Option<&Journal>, config: &Config) -> String {
    match journal {
        None => config.library_path.clone(),
        Some(journal) if journal.phase == Phase::Copy => journal.old_root.clone(),
        Some(journal) => journal.new_root.clone(),
    }
}

/// `raw` trimmed with a leading `~/` expanded.
fn expand_root(raw: &str) -> PathBuf {
    grid_core::autoconfig::paths::expand_user(raw.trim())
}

/// Both name one existing folder.
fn same_folder(raw: &str, target: &Path) -> bool {
    match (
        grid_core::platform::canonicalize(&expand_root(raw)),
        grid_core::platform::canonicalize(target),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::library::queue::{DownloadEntry, DownloadStatus};
    use grid_core::library::registry::InstalledGame;
    use grid_core::library::relocate::Strategy;
    use grid_core::library::root_access::RelocateRefusal;
    use std::fs;

    const IDLE: LibraryActivity = LibraryActivity {
        installs: false,
        running_games: false,
        cloud_transfers: false,
        library_move: false,
    };

    fn s(path: &Path) -> String {
        text(path)
    }

    struct Setup {
        dir: tempfile::TempDir,
        config_path: PathBuf,
        registry: Arc<Registry>,
        install: Arc<InstallService>,
        old: PathBuf,
        events: Arc<Mutex<Vec<MoveEvent>>>,
    }

    /// A current-layout library at `<tmp>/old` with one game, one emulator
    /// and one save, and the config, registry and install service for it.
    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let game_dir = old.join("games").join("SNES").join("Mario");
        fs::create_dir_all(&game_dir).unwrap();
        fs::write(game_dir.join("game.sfc"), vec![7u8; 3 * 1024 * 1024 + 17]).unwrap();
        fs::create_dir_all(old.join("emulators").join("Emu")).unwrap();
        fs::write(old.join("emulators").join("Emu").join("emu.exe"), b"exe").unwrap();
        fs::create_dir_all(old.join("saves").join("Emu")).unwrap();
        fs::write(old.join("saves").join("Emu").join("slot.sav"), b"save").unwrap();
        let config_path = dir.path().join("data").join("config.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        Config {
            library_path: s(&old),
            library_layout_version: 2,
            emulators: vec![grid_core::config::EmulatorEntry {
                name: "Emu".into(),
                path: s(&old.join("emulators").join("Emu").join("emu.exe")),
                ..Default::default()
            }],
            ..Default::default()
        }
        .save(&config_path)
        .unwrap();
        let registry =
            Arc::new(Registry::open(&config_path.with_file_name("grid-launcher.db")).unwrap());
        registry
            .upsert(&InstalledGame {
                title: "Mario".into(),
                platform: "SNES".into(),
                rom_id: Some(1),
                extracted_dir: s(&game_dir),
                ..Default::default()
            })
            .unwrap();
        let install = InstallService::new(registry.clone(), config_path.clone());
        Setup {
            dir,
            config_path,
            registry,
            install,
            old,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn probe_copy() -> Arc<StrategyProbe> {
        Arc::new(|_: &Path, _: &Path| Ok(Strategy::Copy))
    }

    fn plenty() -> Arc<FreeSpace> {
        Arc::new(|_: &Path| Some(u64::MAX / 4))
    }

    fn service(t: &Setup, probe: Arc<StrategyProbe>) -> Arc<LibraryMoveService> {
        LibraryMoveService::with_env(
            t.config_path.clone(),
            probe,
            plenty(),
            Duration::from_millis(200),
        )
    }

    fn ctx_with(t: &Setup, activity: Arc<ActivityFn>) -> MoveContext {
        let events = t.events.clone();
        MoveContext {
            install: t.install.clone(),
            activity,
            resync: Arc::new(|_: &Config| Ok(())),
            events: Arc::new(move |event| lock(&events).push(event)),
            on_progress: None,
        }
    }

    fn ctx(t: &Setup) -> MoveContext {
        ctx_with(t, Arc::new(|| IDLE))
    }

    fn move_rows(install: &InstallService) -> Vec<DownloadEntry> {
        install
            .snapshot()
            .entries
            .into_iter()
            .filter(|e| e.kind == "library_move")
            .collect()
    }

    fn wait_until_stopped(service: &LibraryMoveService) {
        let deadline = Instant::now() + Duration::from_secs(30);
        // The run sets `running` before `start` returns, so a fresh start
        // is never mistaken for a finished one.
        while service.is_running() {
            assert!(Instant::now() < deadline, "the move did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn config(t: &Setup) -> Config {
        Config::load(&t.config_path).unwrap()
    }

    #[test]
    fn size_pair_and_detail_use_the_unit_of_the_total() {
        const GIB: u64 = 1 << 30;
        assert_eq!(size_pair(0, 0), "0 of 0 B");
        assert_eq!(size_pair(512, 1000), "512 of 1000 B");
        assert_eq!(size_pair(1024, 4096), "1.0 of 4.0 KB");
        assert_eq!(
            size_pair(GIB + GIB / 5, 5 * GIB + 3 * GIB / 5),
            "1.2 of 5.6 GB"
        );
        assert_eq!(size_pair(3 << 20, GIB), "0.0 of 1.0 GB");
        assert_eq!(
            copy_detail("games", GIB + GIB / 5, 5 * GIB + 3 * GIB / 5),
            "games/ · 1.2 of 5.6 GB"
        );
    }

    #[test]
    fn a_plan_refusal_creates_no_row_and_leaves_the_gate_open() {
        let t = setup();
        let svc = service(&t, probe_copy());
        let target = t.dir.path().join("new");
        fs::create_dir_all(target.join("games")).unwrap();
        let outcome = svc.start(&ctx(&t), &s(&target)).unwrap();
        assert_eq!(
            outcome,
            StartOutcome::Refused(Refusal {
                reason: "target_has_library_folders".into(),
                message: MoveRefusal::TargetHasLibraryFolders.message().into(),
            })
        );
        assert!(
            move_rows(&t.install).is_empty(),
            "the plan runs before the row"
        );
        assert!(!svc.gate().is_closed());
        assert!(!svc.journal_path().exists());
        // The preview gives the same refusal and also changes nothing.
        assert_eq!(
            svc.preview(&ctx(&t), &s(&target)).unwrap(),
            PreviewOutcome::Refused(Refusal {
                reason: "target_has_library_folders".into(),
                message: MoveRefusal::TargetHasLibraryFolders.message().into(),
            })
        );
    }

    #[test]
    fn busy_after_the_gate_closes_is_refused_and_the_gate_reopens() {
        let t = setup();
        let svc = service(&t, probe_copy());
        // Idle while the plan reads it, busy on the re-check after the gate
        // closed: the start must notice.
        let calls = Arc::new(Mutex::new(0u32));
        let seen_closed = Arc::new(AtomicBool::new(false));
        let activity: Arc<ActivityFn> = {
            let calls = calls.clone();
            let seen_closed = seen_closed.clone();
            let svc = svc.clone();
            Arc::new(move || {
                let mut n = lock(&calls);
                *n += 1;
                if svc.gate().is_closed() {
                    seen_closed.store(true, Ordering::SeqCst);
                    LibraryActivity {
                        running_games: true,
                        ..IDLE
                    }
                } else {
                    IDLE
                }
            })
        };
        let target = t.dir.path().join("new");
        let outcome = svc.start(&ctx_with(&t, activity), &s(&target)).unwrap();
        assert_eq!(
            outcome,
            StartOutcome::Refused(Refusal {
                reason: "game_running".into(),
                message: PathChangeRefusal::GameRunning.message().into(),
            })
        );
        assert!(
            seen_closed.load(Ordering::SeqCst),
            "re-read with the gate closed"
        );
        assert!(*lock(&calls) >= 2);
        assert!(!svc.gate().is_closed(), "the gate reopens");
        assert!(move_rows(&t.install).is_empty(), "the row is dropped");
        assert!(!svc.journal_path().exists());
        assert!(!target.exists(), "nothing created");
    }

    #[test]
    fn a_launch_holding_a_ticket_refuses_the_start() {
        let t = setup();
        let svc = service(&t, probe_copy());
        let ticket = svc.gate().enter().unwrap();
        let outcome = svc.start(&ctx(&t), &s(&t.dir.path().join("new"))).unwrap();
        assert!(matches!(
            outcome,
            StartOutcome::Refused(Refusal { ref reason, .. }) if reason == "game_running"
        ));
        assert!(!svc.gate().is_closed());
        drop(ticket);
        assert!(move_rows(&t.install).is_empty());
    }

    #[test]
    fn a_move_runs_to_the_end_and_opens_the_gate() {
        let t = setup();
        let svc = service(&t, probe_copy());
        let target = t.dir.path().join("new");
        let outcome = svc.start(&ctx(&t), &s(&target)).unwrap();
        assert_eq!(
            outcome,
            StartOutcome::Started {
                new_root: s(&target)
            }
        );
        // A second start while the first runs is refused: never two moves.
        let again = svc
            .start(&ctx(&t), &s(&t.dir.path().join("other")))
            .unwrap();
        assert!(matches!(again, StartOutcome::Refused(_)));
        wait_until_stopped(&svc);

        assert!(!svc.gate().is_closed());
        assert!(!svc.journal_path().exists());
        let config = config(&t);
        assert_eq!(config.library_path, s(&target));
        assert_eq!(config.library_layout_version, 2);
        assert!(target
            .join("games")
            .join("SNES")
            .join("Mario")
            .join("game.sfc")
            .is_file());
        assert!(!t.old.join("games").exists(), "the source is cleaned up");
        let rows = move_rows(&t.install);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, DownloadStatus::Completed);
        let row = &t.registry.all().unwrap()[0];
        assert!(row.extracted_dir.starts_with(&s(&target)));
        let events = lock(&t.events).clone();
        assert!(events.contains(&MoveEvent::LibraryChanged));
        assert!(events.iter().any(|e| matches!(
            e,
            MoveEvent::Finished(MoveFinished::Finished { new_root, .. }) if *new_root == s(&target)
        )));
        assert_eq!(svc.status().phase, None);
    }

    #[test]
    fn cancel_during_the_copy_restores_the_old_library() {
        let t = setup();
        let svc = service(&t, probe_copy());
        let target = t.dir.path().join("new");
        let cancelled = Arc::new(Mutex::new(None::<CancelOutcome>));
        let mut ctx = ctx(&t);
        {
            let svc = svc.clone();
            let cancelled = cancelled.clone();
            let inner = ctx.clone();
            ctx.on_progress = Some(Arc::new(move |p: &MoveProgress| {
                let mut slot = lock(&cancelled);
                if slot.is_none() && p.done_bytes > 0 {
                    // The row says Downloading (cancellable) while it copies.
                    let row = &move_rows(&inner.install)[0];
                    assert_eq!(row.status, DownloadStatus::Downloading);
                    *slot = Some(svc.cancel(&inner).unwrap());
                }
            }));
        }
        assert!(matches!(
            svc.start(&ctx, &s(&target)).unwrap(),
            StartOutcome::Started { .. }
        ));
        wait_until_stopped(&svc);

        assert_eq!(*lock(&cancelled), Some(CancelOutcome::Cancelling));
        assert!(!svc.gate().is_closed());
        assert!(!svc.journal_path().exists());
        assert!(!target.exists(), "GRID created it, so it goes");
        assert_eq!(config(&t).library_path, s(&t.old));
        assert!(t
            .old
            .join("games")
            .join("SNES")
            .join("Mario")
            .join("game.sfc")
            .is_file());
        let rows = move_rows(&t.install);
        assert_eq!(rows[0].status, DownloadStatus::Cancelled);
        assert!(lock(&t.events).contains(&MoveEvent::Finished(MoveFinished::Cancelled)));
        assert_eq!(
            svc.cancel(&ctx).unwrap(),
            CancelOutcome::NoMove {
                message: MoveError::NoJournal.to_string()
            }
        );
    }

    /// A journal seeded in the Copy phase (a crash or a close mid-copy):
    /// the next start closes the gate, recovers nothing synchronously and
    /// resumes in the background.
    #[test]
    fn a_copy_phase_journal_resumes_at_startup() {
        let t = setup();
        let target = t.dir.path().join("new");
        let env = PlanEnv {
            probe: &|_: &Path, _: &Path| Ok(Strategy::Copy),
            free_space: &|_: &Path| Some(u64::MAX / 4),
        };
        relocate::start_move(
            &config(&t),
            &s(&target),
            IDLE,
            &relocate::journal_path(&t.config_path),
            &env,
        )
        .unwrap();

        let svc = service(&t, probe_copy());
        assert!(svc.gate().is_closed(), "closed at construction");
        assert!(svc.check_root_at_startup(&t.registry));
        svc.recover_at_startup(&t.registry, &|_: &Config| Ok(()));
        assert!(svc.has_pending_resume());
        assert_eq!(config(&t).library_path, s(&t.old), "Copy does not switch");
        assert_eq!(svc.status().phase, Some(Phase::Copy));

        assert_eq!(
            svc.resume(&ctx(&t)).unwrap(),
            ResumeOutcome::Resumed {
                new_root: s(&target)
            }
        );
        assert!(svc.status().resumed);
        wait_until_stopped(&svc);
        assert!(!svc.gate().is_closed());
        assert_eq!(config(&t).library_path, s(&target));
        assert!(target.join("saves").join("Emu").join("slot.sav").is_file());
        assert_eq!(move_rows(&t.install)[0].status, DownloadStatus::Completed);
    }

    /// A crash in Commit: startup finishes the Commit before any service
    /// reads the config, and the Cleanup resumes in the background.
    #[test]
    fn a_commit_phase_journal_is_finished_at_startup() {
        let t = setup();
        let target = t.dir.path().join("new");
        let journal = relocate::journal_path(&t.config_path);
        let env = PlanEnv {
            probe: &|_: &Path, _: &Path| Ok(Strategy::Copy),
            free_space: &|_: &Path| Some(u64::MAX / 4),
        };
        relocate::start_move(&config(&t), &s(&target), IDLE, &journal, &env).unwrap();
        let cancel = AtomicBool::new(false);
        let editor = relocate::config_file_editor(&t.config_path);
        let profiles = load_profiles();
        let stop_in_commit = |j: &Journal| j.phase == Phase::Commit;
        let hooks = MoveHooks {
            config_path: &t.config_path,
            registry: &t.registry,
            profiles,
            edit_config: &editor,
            resync: &|_: &Config| Ok(()),
            free_space: &|_: &Path| Some(u64::MAX / 4),
            cancel: &cancel,
            progress: &|_: &MoveProgress| {},
            after_journal_write: Some(&stop_in_commit),
        };
        assert!(matches!(
            relocate::run(&journal, &hooks),
            Err(MoveError::Interrupted)
        ));
        assert_eq!(config(&t).library_path, s(&t.old));

        let svc = service(&t, probe_copy());
        assert!(svc.check_root_at_startup(&t.registry));
        svc.recover_at_startup(&t.registry, &|_: &Config| Ok(()));
        assert_eq!(
            config(&t).library_path,
            s(&target),
            "switched before services"
        );
        assert!(svc.has_pending_resume());
        assert_eq!(svc.status().phase, Some(Phase::Cleanup));
        assert!(svc.gate().is_closed());

        svc.resume(&ctx(&t)).unwrap();
        wait_until_stopped(&svc);
        assert!(!svc.journal_path().exists());
        assert!(!svc.gate().is_closed());
        assert!(!t.old.join("games").exists());
    }

    #[test]
    fn an_unreachable_root_in_use_blocks_and_relocate_rewrites_references() {
        let t = setup();
        // The user moved the library outside GRID: the old root is gone.
        let moved = t.dir.path().join("moved");
        fs::rename(&t.old, &moved).unwrap();

        let svc = service(&t, probe_copy());
        assert!(!svc.check_root_at_startup(&t.registry));
        assert!(svc.gate().is_closed());
        assert_eq!(svc.ensure_idle(), Err(LIBRARY_UNREACHABLE_BUSY.to_string()));
        assert_eq!(
            svc.unreachable_status(),
            UnreachableStatus::Unreachable {
                root: s(&t.old),
                reason: UnreachableReason::Missing,
                message: UnreachableReason::Missing.message().into(),
            }
        );
        // A move cannot start on an unreachable library.
        assert_eq!(
            svc.start(&ctx(&t), &s(&t.dir.path().join("x"))).unwrap(),
            StartOutcome::Refused(Refusal::unreachable())
        );

        // A folder with no library folders is refused; nothing changes.
        let empty = t.dir.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        assert_eq!(
            svc.relocate(&ctx(&t), &s(&empty)).unwrap(),
            RelocateOutcome::Refused(Refusal {
                reason: "no_library_folders".into(),
                message: RelocateRefusal::NoLibraryFolders.message().into(),
            })
        );
        assert_eq!(config(&t).library_path, s(&t.old));
        assert!(svc.gate().is_closed());

        let outcome = svc.relocate(&ctx(&t), &s(&moved)).unwrap();
        assert!(matches!(
            outcome,
            RelocateOutcome::Relocated { ref library_path, .. } if *library_path == s(&moved)
        ));
        assert_eq!(svc.unreachable_status(), UnreachableStatus::Reachable);
        assert!(!svc.gate().is_closed());
        let config = config(&t);
        assert_eq!(config.library_path, s(&moved));
        assert!(config.emulators[0].path.starts_with(&s(&moved)));
        assert!(t.registry.all().unwrap()[0]
            .extracted_dir
            .starts_with(&s(&moved)));
        assert!(lock(&t.events).contains(&MoveEvent::LibraryChanged));
        // Nothing to relocate any more.
        assert!(matches!(
            svc.relocate(&ctx(&t), &s(&moved)).unwrap(),
            RelocateOutcome::Refused(_)
        ));
    }

    #[test]
    fn a_missing_root_that_holds_nothing_of_ours_does_not_block() {
        let t = setup();
        fs::remove_dir_all(&t.old).unwrap();
        for row in t.registry.all().unwrap() {
            t.registry.remove(&row.title, &row.platform).unwrap();
        }
        modify_config(&t.config_path, |c| {
            c.emulators.clear();
            Ok(())
        })
        .unwrap();
        let svc = service(&t, probe_copy());
        assert!(svc.check_root_at_startup(&t.registry));
        assert!(!svc.gate().is_closed());
        assert_eq!(svc.unreachable_status(), UnreachableStatus::Reachable);
    }

    #[test]
    fn a_blank_library_path_is_a_first_run_not_an_unreachable_library() {
        let t = setup();
        modify_config(&t.config_path, |c| {
            c.library_path.clear();
            Ok(())
        })
        .unwrap();
        let svc = service(&t, probe_copy());
        assert!(svc.check_root_at_startup(&t.registry));
        assert_eq!(svc.unreachable_status(), UnreachableStatus::Reachable);
    }

    #[test]
    fn ensure_idle_refuses_with_the_move_message_while_the_gate_is_closed() {
        let t = setup();
        let svc = service(&t, probe_copy());
        assert_eq!(svc.ensure_idle(), Ok(()));
        svc.gate().close(LIBRARY_MOVE_BUSY);
        let refused = svc.ensure_idle().unwrap_err();
        assert_eq!(refused, LIBRARY_MOVE_BUSY);
        assert!(refused.contains("being moved"));
        svc.gate().open();
        assert_eq!(svc.ensure_idle(), Ok(()));
    }
}
