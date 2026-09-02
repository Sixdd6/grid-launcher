//! Emulated launch core. See `docs/porting/04-emulator-launch.md` for the
//! behavior this module tree ports from `grid_launcher/emulator/` and
//! `grid_launcher/ui/mixins/emulator_ui_mixin.py`.

pub mod catalog;
pub mod emu_install;
pub mod forge;
pub mod profiles;
pub mod rom;
pub mod selection;
pub mod sessions;
pub mod source;
pub mod spawn;
pub mod template;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{Config, EmulatorEntry};
use crate::library::paths::expand_home;
use crate::library::registry::{installed_match, InstalledGame, Registry};

use profiles::{load_profiles, profile_for_entry};
use rom::resolve_rom_path;
use selection::{
    default_emulator_name_for_platform, emulator_entry_by_name, mapping_value_for_platform,
};
use sessions::SessionStore;
use spawn::{clean_env, prepare_emulator_launch};
use template::{host_os, retroarch_core_argument_path, Placeholders};

pub use sessions::{GameSession, SessionsSnapshot};

/// Errors raised while resolving or running an emulated launch.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("{0}")]
    Validation(String),
    #[error("This game is already running.")]
    AlreadyRunning,
    #[error("Game is not installed.")]
    NotInstalled,
    #[error("registry: {0}")]
    Registry(String),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("failed to launch game: {0}")]
    Io(#[from] std::io::Error),
}

// --- constants --------------------------------------------------------------

/// How often the background loop reaps exited children.
const POLL_INTERVAL: Duration = Duration::from_millis(2500);
/// How long after a spawn the early-exit check runs. A game that is already
/// gone by then almost certainly failed to start, so the user is told.
const EARLY_EXIT_DELAY: Duration = Duration::from_millis(500);

type Listener = Arc<dyn Fn(SessionsSnapshot) + Send + Sync>;

/// Owns the running-game sessions: resolves a launch, spawns the emulator,
/// tracks the child, and reaps it when it exits.
///
/// Lock discipline, matching `InstallService`: the session store's lock is
/// taken by [`SessionStore`] methods only, and every listener call happens
/// with no lock held.
pub struct LaunchService {
    sessions: SessionStore,
    registry: Arc<Registry>,
    config_path: PathBuf,
    notify: RwLock<Option<Listener>>,
    next_id: AtomicU64,
    /// Guards [`Self::spawn_poll_loop`] so at most one loop ever runs.
    poll_started: AtomicBool,
    poll_interval: Duration,
    /// Serializes reaping against [`Self::stop`]. On unix `stop` reads a pid,
    /// releases the store lock, and only then signals; without this gate a
    /// reap in that gap could `waitpid` the child, free its pid for reuse,
    /// and turn the signal into one aimed at an unrelated process. Lock
    /// order is always this gate first, the session store second — never the
    /// reverse — and no listener is ever called while it is held.
    reap_gate: Mutex<()>,
    /// Sessions still inside their early-exit window, mapped to the command
    /// line to quote if they die there. An entry is added at spawn and taken
    /// out by whichever event comes first: the session being reaped (which
    /// emits the warning), the session's own 500 ms check finding it still
    /// alive (the window is over — a later exit is a normal quit), or a
    /// [`Self::stop`] for it (the user asked; that is not a failure).
    ///
    /// It exists because the reap that observes an early exit is not always
    /// that session's own check: any reaping path — the poll loop, or a
    /// *sibling's* check running while this session is young — can be the one
    /// that finds the dead child, and the warning has to survive whichever it
    /// is. Bounded by the number of launches in flight.
    early_exit_watch: Mutex<HashMap<u64, String>>,
}

impl LaunchService {
    pub fn new(registry: Arc<Registry>, config_path: PathBuf) -> Arc<Self> {
        Self::new_with_poll_interval(registry, config_path, POLL_INTERVAL)
    }

    /// Same as [`Self::new`] with a caller-chosen reap interval. Tests use a
    /// short one so a stopped session is observed without waiting out the
    /// production 2500 ms tick; application code should call [`Self::new`].
    #[doc(hidden)]
    pub fn new_with_poll_interval(
        registry: Arc<Registry>,
        config_path: PathBuf,
        poll_interval: Duration,
    ) -> Arc<Self> {
        Arc::new(Self {
            sessions: SessionStore::default(),
            registry,
            config_path,
            notify: RwLock::new(None),
            next_id: AtomicU64::new(1),
            poll_started: AtomicBool::new(false),
            poll_interval,
            reap_gate: Mutex::new(()),
            early_exit_watch: Mutex::new(HashMap::new()),
        })
    }

    /// Installs the change-notification callback. Called once by the UI
    /// layer; a second call replaces the first.
    pub fn set_notify(&self, f: Listener) {
        *self.notify.write().unwrap() = Some(f);
    }

    /// The running games, newest-first (mirrors [`crate::library::queue::QueueState::snapshot`]).
    /// `warning` is always `None` here — a warning only ever reaches the
    /// listener, attached to the snapshot that reports the early exit that
    /// produced it.
    pub fn snapshot(&self) -> SessionsSnapshot {
        let mut sessions = self.sessions.list();
        sessions.reverse();
        SessionsSnapshot {
            sessions,
            warning: None,
        }
    }

    /// Resolves and starts the emulated launch for `rom_id`.
    ///
    /// Order (doc 04 §8): registry lookup, native-platform gate, duplicate
    /// gate, config load, emulator selection, placeholder build, the
    /// validation chain in [`prepare_emulator_launch`], then the spawn.
    pub async fn launch(self: &Arc<Self>, rom_id: i64) -> Result<GameSession, LaunchError> {
        let game = self.installed_game(rom_id).await?;

        if is_native_platform(&game.platform) {
            return Err(LaunchError::Validation(
                "Native Windows games are not supported yet in the Rust preview.".to_string(),
            ));
        }

        // Cheap rejection before any work; `SessionStore::register` repeats
        // the check under the store lock as the authoritative one.
        if self.sessions.contains_rom(rom_id) {
            return Err(LaunchError::AlreadyRunning);
        }

        let config = Config::load(&self.config_path)?;
        let plan = resolve_launch(&game, &config)?;

        let title = game.title.clone();
        let joined_command = plan.argv.join(" ");
        let child = spawn_child(plan.argv, plan.working_dir).await?;
        let pid = child.id();

        let session = GameSession {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            rom_id,
            title,
            emulator_name: plan.emulator_name,
            started_at: unix_now(),
            pid,
        };

        // `register` re-checks the rom under the store lock; it is the
        // authoritative duplicate gate, because the cheap check above and
        // this insert are not one critical section.
        if let Err(mut loser) = self.sessions.register(session.clone(), child) {
            // Another launch of the same rom won the race. Its process is the
            // session; this one is untracked, so kill it here rather than
            // leaving an orphan behind.
            let _ = loser.kill();
            let _ = loser.wait();
            return Err(LaunchError::AlreadyRunning);
        }

        self.early_exit_watch
            .lock()
            .unwrap()
            .insert(session.id, joined_command);

        self.emit(None);
        self.schedule_early_exit_check(session.id);
        Ok(session)
    }

    /// Asks `session_id`'s game to quit. Errors are swallowed: the process
    /// may have exited already, and the reaper is what actually removes the
    /// session either way.
    pub fn stop(&self, session_id: u64) {
        // Held across the lookup and the kill so no reap can run in between
        // (see `reap_gate`). It is not the session-store lock, which
        // `pid_of` takes and releases on its own.
        let _gate = self.reap_gate.lock().unwrap();
        // The user asked for this exit, so it is not a failure to report.
        // Dropped before the signal, and under the reap gate, so no reaping
        // path can still see the entry once the process can be gone.
        self.early_exit_watch.lock().unwrap().remove(&session_id);
        #[cfg(unix)]
        {
            // The pid is read under the store lock and signalled after that
            // lock is released — a signal is never sent with it held.
            let pid = self.sessions.pid_of(session_id);
            if let Some(pid) = pid {
                // SAFETY: `kill` with a pid this process owns as a child is
                // sound; a failure (already reaped, no permission) is
                // reported through the return value, which is ignored.
                unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            }
        }
        #[cfg(windows)]
        {
            // Windows has no pid-addressed terminate in `std`, so the kill
            // goes through the `Child` handle with the store lock held. It
            // does not wait for the process, so the hold is brief.
            self.sessions.kill(session_id);
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = session_id;
        }
    }

    /// Starts the background reaper. Idempotent: later calls do nothing.
    ///
    /// The loop holds a `Weak`, so it stops on its own once the service is
    /// dropped — a test that drops its service does not leak a task.
    pub fn spawn_poll_loop(self: &Arc<Self>) {
        if self.poll_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        let interval = self.poll_interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // The first tick fires immediately; skip it so the first reap
            // happens one interval in.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                // The upgraded handle is dropped at the end of each
                // iteration, never held across the await above, so it cannot
                // keep the service alive.
                let Some(service) = weak.upgrade() else {
                    return;
                };
                // Reaps everything. A child still inside its early-exit
                // window carries its warning in `early_exit_watch`, so
                // reaping it here reports the failure just as its own check
                // would have.
                service.reap_and_notify();
            }
        });
    }

    // --- internals ----------------------------------------------------------

    /// The installed row for `rom_id`, on the blocking pool.
    ///
    /// `find`'s title/platform fallback can hand back a row for a different
    /// game that merely shares a title and platform, so `installed_match` is
    /// the final word — the same guard `InstallService::uninstall` uses.
    async fn installed_game(&self, rom_id: i64) -> Result<InstalledGame, LaunchError> {
        let registry = self.registry.clone();
        let found = tokio::task::spawn_blocking(move || registry.find(Some(rom_id), "", ""))
            .await
            .map_err(|e| LaunchError::Registry(format!("the lookup did not finish: {e}")))?
            .map_err(|e| LaunchError::Registry(e.to_string()))?;
        found
            .filter(|row| installed_match(row, rom_id))
            .ok_or(LaunchError::NotInstalled)
    }

    /// Runs the 500 ms post-launch check: reap, which reports the failure if
    /// this session (or any sibling) died inside its window, then close this
    /// session's window so a later, ordinary quit says nothing.
    fn schedule_early_exit_check(self: &Arc<Self>, session_id: u64) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(EARLY_EXIT_DELAY).await;
            let Some(service) = weak.upgrade() else {
                return;
            };
            // The same reaping path the poll loop uses: one place removes
            // sessions, so the two can never double-remove or disagree.
            service.reap_and_notify();
            service.early_exit_watch.lock().unwrap().remove(&session_id);
        });
    }

    /// Reaps, then tells the listener what happened: one warning snapshot per
    /// session that died inside its early-exit window, or a single plain
    /// snapshot when sessions went away with nothing to report. Emits nothing
    /// when the store did not change.
    ///
    /// A warning gets its own snapshot because `SessionsSnapshot` carries at
    /// most one message; two games failing in the same tick is rare, and
    /// dropping one of the two messages would be worse than two emissions.
    fn reap_and_notify(&self) {
        let exited = self.reap_exited();
        if exited.is_empty() {
            return;
        }
        let warnings: Vec<String> = {
            let mut watch = self.early_exit_watch.lock().unwrap();
            exited
                .iter()
                .filter_map(|(id, status)| {
                    watch
                        .remove(id)
                        .map(|command| early_exit_message(*status, &command))
                })
                .collect()
        };
        if warnings.is_empty() {
            self.emit(None);
            return;
        }
        for warning in warnings {
            self.emit(Some(warning));
        }
    }

    /// Reaps under the reap gate. Never notifies: [`Self::reap_and_notify`]
    /// decides what the listener hears, always with no lock held.
    fn reap_exited(&self) -> Vec<(u64, Option<ExitStatus>)> {
        let _gate = self.reap_gate.lock().unwrap();
        self.sessions.reap()
    }

    /// Hands a snapshot to the listener with NO lock held: the callback is
    /// arbitrary UI code and must never be able to block a state change.
    fn emit(&self, warning: Option<String>) {
        let listener = self.notify.read().unwrap().clone();
        let Some(listener) = listener else {
            return;
        };
        let mut snapshot = self.snapshot();
        snapshot.warning = warning;
        listener(snapshot);
    }
}

// --- resolution -------------------------------------------------------------

/// The message for a game that died inside its early-exit window.
///
/// `status` is `None` only when `try_wait` itself failed, so no exit code was
/// ever available; the process is gone either way and the user still needs to
/// be told, so the code reads "unknown".
fn early_exit_message(status: Option<ExitStatus>, joined_command: &str) -> String {
    match status {
        Some(status) => match status.code() {
            Some(code) => format!("Game exited immediately (code {code}): {joined_command}"),
            // No exit code: killed by a signal. The `ExitStatus` display
            // already reads as "signal: 9 (SIGKILL)".
            None => format!("Game exited immediately ({status}): {joined_command}"),
        },
        None => format!("Game exited immediately (unknown): {joined_command}"),
    }
}

/// Everything the spawn step needs, once resolution has succeeded.
struct LaunchPlan {
    emulator_name: String,
    argv: Vec<String>,
    working_dir: PathBuf,
}

/// A game is "native" when its platform, trimmed and casefolded, starts with
/// `windows` (`is_native_executable_platform`, selection.py:145). This is the
/// *server* platform name, not the host OS.
fn is_native_platform(platform: &str) -> bool {
    platform.trim().to_lowercase().starts_with("windows")
}

/// Whether `name` should be treated as a RetroArch build (doc 04 §2): the
/// name contains "retroarch", case-insensitively.
fn is_retroarch_name(name: &str) -> bool {
    name.to_lowercase().contains("retroarch")
}

/// Picks the emulator, builds the placeholders, and runs the validation
/// chain. Pure apart from the on-disk existence checks inside
/// [`prepare_emulator_launch`] and the ROM resolver.
fn resolve_launch(game: &InstalledGame, config: &Config) -> Result<LaunchPlan, LaunchError> {
    let platform = game.platform.trim();
    let profiles = load_profiles();

    let emulator_name = default_emulator_name_for_platform(
        &config.emulators,
        &config.default_emulators,
        platform,
        profiles,
        &config.retroarch_cores,
    );
    let entry = emulator_entry_by_name(&config.emulators, &emulator_name);
    let is_retroarch = entry.is_some_and(|e| entry_is_retroarch(e, profiles));

    // Expanded once, here: the same string becomes both the `%rom%`
    // placeholder and the path the existence check runs on, so a recorded
    // `~/...` archive path can never pass validation and then reach the
    // emulator as a literal tilde.
    let rom_path = expand_home(&resolve_rom_path(game, &expand_home(&config.library_path)))
        .to_string_lossy()
        .into_owned();

    // The RetroArch gate decides both halves: no core placeholder value for a
    // non-RetroArch emulator, and no `-L` post-pass either.
    let core = if is_retroarch {
        mapping_value_for_platform(&config.retroarch_cores, platform)
            .map(|value| retroarch_core_argument_path(value, host_os()))
            .unwrap_or_default()
    } else {
        String::new()
    };

    let placeholders = Placeholders {
        rom: rom_path.clone(),
        core,
        // PS3 targets need the RPCS3 metadata a later milestone adds.
        ps3_launch_target: String::new(),
    };

    let (argv, working_dir) = prepare_emulator_launch(
        &emulator_name,
        entry,
        &rom_path,
        &placeholders,
        &config.launch_args,
        is_retroarch,
    )
    .map_err(LaunchError::Validation)?;

    Ok(LaunchPlan {
        emulator_name,
        argv,
        working_dir,
    })
}

/// Whether `entry` is a RetroArch build: its own name, or the name of the
/// autoprofile it resolves to, mentions RetroArch.
fn entry_is_retroarch(entry: &EmulatorEntry, profiles: &[profiles::EmulatorProfile]) -> bool {
    is_retroarch_name(&entry.name)
        || profile_for_entry(&entry.name, &entry.path, profiles)
            .is_some_and(|profile| is_retroarch_name(&profile.name))
}

/// Spawns `argv` in `working_dir` on the blocking pool. The child gets a
/// clean environment ([`clean_env`]) and, on Windows, its own process group
/// so a console signal to the launcher does not reach the game.
async fn spawn_child(
    argv: Vec<String>,
    working_dir: PathBuf,
) -> Result<std::process::Child, LaunchError> {
    let spawned = tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .current_dir(&working_dir)
            .env_clear()
            .envs(clean_env());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            command.creation_flags(CREATE_NEW_PROCESS_GROUP);
        }
        command.spawn()
    })
    .await
    .map_err(|e| std::io::Error::other(format!("the spawn did not finish: {e}")))?;
    Ok(spawned?)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_platform_matches_every_windows_spelling() {
        assert!(is_native_platform("Windows"));
        assert!(is_native_platform("  windows  "));
        assert!(is_native_platform("Windows 3.x"));
        assert!(!is_native_platform("Nintendo Wii"));
        assert!(!is_native_platform(""));
    }

    #[test]
    fn retroarch_name_detection_ignores_case_and_matches_substrings() {
        assert!(is_retroarch_name("RetroArch"));
        assert!(is_retroarch_name("my retroarch build"));
        assert!(!is_retroarch_name("Dolphin"));
    }

    #[test]
    fn entry_is_retroarch_uses_the_matched_profile_name() {
        let profiles = load_profiles();
        let by_entry_name = EmulatorEntry {
            name: "RetroArch".to_string(),
            path: "/x/whatever".to_string(),
            args: String::new(),
            ..Default::default()
        };
        assert!(entry_is_retroarch(&by_entry_name, profiles));

        let unrelated = EmulatorEntry {
            name: "Dolphin".to_string(),
            path: "/x/dolphin-emu".to_string(),
            args: String::new(),
            ..Default::default()
        };
        assert!(!entry_is_retroarch(&unrelated, profiles));
    }
}
