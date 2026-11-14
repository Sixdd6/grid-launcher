//! Running a Windows GOG installer: the Inno Setup argv (silent, then a
//! milder silent run, then — desktop only — the wizard), the process
//! supervisor with its stall rule, and the kill on cancel. No output line
//! of the installer is read or logged: it writes only to its own log file,
//! whose size is the progress signal and which [`super::inno_log`] matches
//! for a known reason when a run fails.
//!
//! Spike results (docs/stores-phase1-plan.md): exit 0 is success; 2 and 5
//! mean it was cancelled; 1, 3, 4, 7 and 8 mean it failed. A failing
//! silent run mostly shows as a hang, so a silent run whose log and target
//! folder have not grown for 120 s is killed and counts as failed. There is
//! no overall timeout.

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::launch::compat::{steam_roots, which_on_path};
use crate::launch::native::{build_installer_command, NativeCommandError, NativeLaunch};
use crate::launch::spawn::clean_env;
use crate::launch::template::host_os;
use crate::launch::umu;
use crate::library::queue::{JobPhase, StoreProgressSink};
use crate::library::LibraryError;

use super::inno_log::{self, InstallerReason};

/// The Inno Setup flags of a silent run (spike 5).
pub const SILENT_FLAGS: [&str; 6] = [
    "/SP-",
    "/VERYSILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/NOCANCEL",
    "/NOICONS",
];

/// The Inno Setup flags of the milder silent run GRID tries when the first
/// one fails: a progress window but still no question (no `/VERYSILENT`,
/// no `/NOCANCEL`, no `/NOICONS`).
pub const MILDER_FLAGS: [&str; 4] = ["/SP-", "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"];

/// The installer log's file name, inside a folder GRID owns. Later runs of
/// the same install write `grid-install-2.log`, `grid-install-3.log`.
pub const LOG_NAME: &str = "grid-install.log";

/// The Inno language a run asks for when the installer's own list is
/// unknown.
pub const DEFAULT_INNO_LANG: &str = "english";

/// How long a silent run may go without its log or target folder growing.
pub const STALL_AFTER: Duration = Duration::from_secs(120);

/// One installer command: what [`InstallerSpawner::spawn`] starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerCommand {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    /// The Wine prefix the run uses (`None` for a native run): what a kill
    /// stops with `wineserver -k`.
    pub prefix: Option<PathBuf>,
    /// The compat tool value (`""` for a native run).
    pub compat: String,
}

/// A started installer process.
pub trait InstallerProcess: Send {
    /// `Some(exit)` once it has ended (`None` inside for a signal), `None`
    /// while it runs.
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>>;
    /// Stops it and everything it started.
    fn kill(&mut self);
}

/// Starts installer processes. The real one spawns them; tests script one.
pub trait InstallerSpawner: Send + Sync {
    fn spawn(&self, cmd: &InstallerCommand) -> io::Result<Box<dyn InstallerProcess>>;
}

/// The supervisor's timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watch {
    /// A silent run is stalled after this long without growth.
    pub stall: Duration,
    /// How often the process and the growth signal are checked.
    pub poll: Duration,
}

impl Default for Watch {
    fn default() -> Self {
        Watch {
            stall: STALL_AFTER,
            poll: Duration::from_secs(2),
        }
    }
}

/// How one run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnd {
    /// It exited with this code (`None`: killed by a signal).
    Exited(Option<i32>),
    /// The silent run stopped growing and was killed.
    Stalled,
    /// GRID cancelled it and killed it.
    Cancelled,
}

/// A path as a Windows program under Wine sees it: `Z:` plus the absolute
/// path with backslashes. On a Windows host (`native`) the path itself.
pub fn windows_path(path: &Path, native: bool) -> String {
    let text = path.to_string_lossy();
    if native {
        text.into_owned()
    } else {
        format!("Z:{}", text.replace('/', "\\"))
    }
}

/// The silent run's arguments: [`SILENT_FLAGS`], `/LANG`, `/DIR` and
/// `/LOG` (paths in the installer's own form, [`windows_path`]).
pub fn silent_args(dir: &str, log: &str, lang: &str) -> Vec<String> {
    let mut args: Vec<String> = SILENT_FLAGS.iter().map(|flag| flag.to_string()).collect();
    args.extend(common_args(dir, log, lang));
    args
}

/// The milder silent run's arguments: [`MILDER_FLAGS`], `/LANG`, `/DIR`
/// and `/LOG`.
pub fn milder_args(dir: &str, log: &str, lang: &str) -> Vec<String> {
    let mut args: Vec<String> = MILDER_FLAGS.iter().map(|flag| flag.to_string()).collect();
    args.extend(common_args(dir, log, lang));
    args
}

/// The wizard run's arguments: the same `/LANG`, `/DIR` and `/LOG`, so the
/// setup window opens on GRID's folder, with none of the silent flags.
pub fn wizard_args(dir: &str, log: &str, lang: &str) -> Vec<String> {
    common_args(dir, log, lang)
}

/// One run of a Windows installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    /// [`SILENT_FLAGS`].
    Silent,
    /// [`MILDER_FLAGS`]: still silent, no input needed.
    Milder,
    /// The installer's own setup window (desktop only).
    Wizard,
}

/// The runs GRID tries, in order, until one succeeds: the silent run, the
/// milder silent run, and the setup window only where it may open
/// (`wizard_allowed`: desktop mode).
pub fn attempts(wizard_allowed: bool) -> &'static [Attempt] {
    if wizard_allowed {
        &[Attempt::Silent, Attempt::Milder, Attempt::Wizard]
    } else {
        &[Attempt::Silent, Attempt::Milder]
    }
}

impl Attempt {
    /// This run's arguments.
    pub fn args(self, dir: &str, log: &str, lang: &str) -> Vec<String> {
        match self {
            Attempt::Silent => silent_args(dir, log, lang),
            Attempt::Milder => milder_args(dir, log, lang),
            Attempt::Wizard => wizard_args(dir, log, lang),
        }
    }
}

/// The log file name of the `n`th run (0-based).
pub fn log_name(n: usize) -> String {
    if n == 0 {
        LOG_NAME.to_string()
    } else {
        format!("grid-install-{}.log", n + 1)
    }
}

/// How a silent run went, from how it ended and its log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentOutcome {
    Installed,
    /// GRID cancelled it.
    Cancelled,
    /// It failed, for this reason.
    Failed(InstallerReason),
}

/// Judges a silent run: exit 0 is installed unless its log says it rolled
/// back; any other end is a failure whose reason the log names (an exit 2
/// or 5 with no other reason: the installer cancelled itself). A cancel
/// from GRID is [`SilentOutcome::Cancelled`].
pub fn judge_silent(end: RunEnd, log: &str) -> SilentOutcome {
    let found = inno_log::reason(log);
    match end {
        RunEnd::Cancelled => SilentOutcome::Cancelled,
        RunEnd::Exited(Some(0)) => {
            if inno_log::says_failed(log) {
                SilentOutcome::Failed(found.unwrap_or(InstallerReason::Unknown))
            } else {
                SilentOutcome::Installed
            }
        }
        RunEnd::Exited(code) => SilentOutcome::Failed(found.unwrap_or(
            if exit_meaning(code) == ExitMeaning::Cancelled {
                InstallerReason::Cancelled
            } else {
                InstallerReason::Unknown
            },
        )),
        RunEnd::Stalled => SilentOutcome::Failed(found.unwrap_or(InstallerReason::Unknown)),
    }
}

fn common_args(dir: &str, log: &str, lang: &str) -> Vec<String> {
    let lang = if lang.trim().is_empty() {
        DEFAULT_INNO_LANG
    } else {
        lang.trim()
    };
    vec![
        format!("/LANG={lang}"),
        format!("/DIR={dir}"),
        format!("/LOG={log}"),
    ]
}

/// What an exit code of a silent run means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitMeaning {
    Success,
    Cancelled,
    /// May work with the milder run or the setup window.
    NeedsWizard,
    Failed,
}

/// Spike 5's exit-code table.
pub fn exit_meaning(code: Option<i32>) -> ExitMeaning {
    match code {
        Some(0) => ExitMeaning::Success,
        Some(2 | 5) => ExitMeaning::Cancelled,
        Some(1 | 3 | 4 | 7 | 8) => ExitMeaning::NeedsWizard,
        _ => ExitMeaning::Failed,
    }
}

/// Waits for `process` to end, polling every `watch.poll`. `growth` is a
/// counter that rises while the installer works (the log's size plus the
/// target folder's size); with `stall_check`, a run whose counter has not
/// changed for `watch.stall` is killed as [`RunEnd::Stalled`]. `cancelled`
/// is asked on every poll; a yes kills the run as [`RunEnd::Cancelled`].
/// `on_tick` gets the counter on every poll.
pub fn supervise(
    process: &mut dyn InstallerProcess,
    growth: &mut dyn FnMut() -> u64,
    watch: Watch,
    stall_check: bool,
    cancelled: &dyn Fn() -> bool,
    on_tick: &mut dyn FnMut(u64),
) -> io::Result<RunEnd> {
    let mut last = growth();
    let mut since = Instant::now();
    loop {
        if let Some(code) = process.try_wait()? {
            return Ok(RunEnd::Exited(code));
        }
        if cancelled() {
            process.kill();
            return Ok(RunEnd::Cancelled);
        }
        let now = growth();
        on_tick(now);
        if now != last {
            last = now;
            since = Instant::now();
        } else if stall_check && since.elapsed() >= watch.stall {
            process.kill();
            return Ok(RunEnd::Stalled);
        }
        std::thread::sleep(watch.poll);
    }
}

/// Total bytes and file count under `dir` (missing: zero), as one number
/// that changes whenever a file is added or grows.
pub fn tree_growth(dir: &Path) -> u64 {
    fn walk(dir: &Path, sum: &mut u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                walk(&entry.path(), sum);
            } else if let Ok(meta) = entry.metadata() {
                *sum = sum.saturating_add(meta.len()).saturating_add(1);
            }
        }
    }
    let mut sum = 0;
    walk(dir, &mut sum);
    sum
}

/// Everything a Windows installer run needs.
pub struct ExeRun<'a> {
    pub setup: &'a Path,
    /// The install folder (`/DIR`).
    pub dest: &'a Path,
    pub prefix: Option<&'a Path>,
    pub compat: &'a str,
    pub lang: &'a str,
    pub wizard_allowed: bool,
    /// The installer's size: the progress total until the folder outgrows it.
    pub expected_bytes: u64,
    pub spawner: &'a dyn InstallerSpawner,
    pub watch: Watch,
}

/// Runs a Windows installer through [`attempts`]: silently, then with the
/// milder silent flags, then (desktop only) with its setup window. `Ok`
/// when a run installed; the caller reads the installed `goggame-*.info`.
///
/// A silent run that exits non-zero, stalls, or exits 0 with a log that
/// says it rolled back is followed by the next run. Each run writes its
/// own log; a failed run's log is matched for a known reason
/// ([`super::inno_log`]) and never read otherwise.
///
/// Errors: [`LibraryError::Cancelled`] (GRID's cancel, or the user's in the
/// setup window), [`LibraryError::InstallerSilentFailed`] `{ kept: true }`
/// when both silent runs failed and the wizard isn't allowed (the reason
/// of the last run that named one), [`LibraryError::InstallerExit`] when
/// the setup window failed, and [`LibraryError::InstallerRefused`] when no
/// command can be built (no compat tool, umu-run missing).
pub fn run_exe(run: &ExeRun<'_>, sink: &mut dyn StoreProgressSink) -> Result<(), LibraryError> {
    let native = host_os().starts_with("win");
    fs::create_dir_all(run.dest)?;
    let logs = log_dir(run.dest)?;
    let dir_arg = windows_path(run.dest, native);
    let mut reason = InstallerReason::Unknown;
    for (n, attempt) in attempts(run.wizard_allowed).iter().enumerate() {
        let log = logs.path().join(log_name(n));
        let log_arg = windows_path(&log, native);
        let cmd = command(run, attempt.args(&dir_arg, &log_arg, run.lang))?;
        if *attempt == Attempt::Wizard {
            sink.phase(JobPhase::InstallerWizard);
            return match run_one(run, &cmd, &log, false, sink)? {
                RunEnd::Exited(Some(0)) => Ok(()),
                RunEnd::Exited(Some(2 | 5)) | RunEnd::Cancelled => Err(LibraryError::Cancelled),
                RunEnd::Exited(code) => Err(LibraryError::InstallerExit(code.unwrap_or(-1))),
                // Never: the wizard has no stall check.
                RunEnd::Stalled => Err(LibraryError::InstallerExit(-1)),
            };
        }
        sink.phase(JobPhase::InstallerSilent);
        let end = run_one(run, &cmd, &log, true, sink)?;
        match judge_silent(end, &inno_log::read(&log)) {
            SilentOutcome::Installed => return Ok(()),
            SilentOutcome::Cancelled => return Err(LibraryError::Cancelled),
            SilentOutcome::Failed(found) => {
                if found != InstallerReason::Unknown {
                    reason = found;
                }
            }
        }
    }
    Err(LibraryError::InstallerSilentFailed { reason, kept: true })
}

/// The folder the installer log goes to: a fresh one beside `dest`, so the
/// install folder holds only the game.
fn log_dir(dest: &Path) -> Result<tempfile::TempDir, LibraryError> {
    let parent = dest.parent().unwrap_or(dest);
    Ok(tempfile::Builder::new()
        .prefix(".grid-installer-")
        .tempdir_in(parent)
        .or_else(|_| tempfile::tempdir())?)
}

fn command(run: &ExeRun<'_>, args: Vec<String>) -> Result<InstallerCommand, LibraryError> {
    let launch = build_installer_command(
        run.setup,
        args,
        run.compat,
        run.prefix,
        host_os(),
        &which_with_managed_umu,
    )
    .map_err(|error: NativeCommandError| LibraryError::InstallerRefused(error.to_string()))?;
    Ok(to_command(launch, run.prefix, run.compat))
}

/// The [`InstallerCommand`] for a built launch: the prefix and compat
/// value are kept only when the run is wrapped (Wine or umu).
pub fn to_command(launch: NativeLaunch, prefix: Option<&Path>, compat: &str) -> InstallerCommand {
    let wrapped = !launch.tool_label.is_empty();
    InstallerCommand {
        argv: launch.argv,
        env: launch.env,
        cwd: launch.cwd,
        prefix: prefix.filter(|_| wrapped).map(Path::to_path_buf),
        compat: if wrapped {
            compat.trim().to_string()
        } else {
            String::new()
        },
    }
}

/// `which`, with GRID's managed umu-run as the fallback for `umu-run`
/// ([`umu::locate`]'s order). Amazon's post-install steps use it too.
pub fn which_with_managed_umu(name: &str) -> Option<PathBuf> {
    if name != umu::UMU_RUN {
        return which_on_path(name);
    }
    let managed = umu::find_umu_run(&umu::install_dir(&crate::launch::compat::managed_root()));
    umu::locate(&which_on_path, managed.as_deref()).map(|(path, _)| path)
}

fn run_one(
    run: &ExeRun<'_>,
    cmd: &InstallerCommand,
    log: &Path,
    stall_check: bool,
    sink: &mut dyn StoreProgressSink,
) -> Result<RunEnd, LibraryError> {
    let mut process = run.spawner.spawn(cmd).map_err(|e| {
        LibraryError::InstallerRefused(format!("The installer could not be started: {e}"))
    })?;
    let mut growth = || {
        fs::metadata(log)
            .map(|meta| meta.len())
            .unwrap_or(0)
            .saturating_add(tree_growth(run.dest))
    };
    let total = run.expected_bytes.max(1);
    // The cancel check and the progress ticks both need the sink.
    let sink = RefCell::new(sink);
    let cancelled = || sink.borrow().cancelled();
    let mut on_tick = |now: u64| sink.borrow_mut().progress(now.min(total), total);
    let end = supervise(
        process.as_mut(),
        &mut growth,
        run.watch,
        stall_check,
        &cancelled,
        &mut on_tick,
    )
    .map_err(LibraryError::Io)?;
    if matches!(end, RunEnd::Stalled | RunEnd::Cancelled) {
        stop_prefix(cmd);
    }
    Ok(end)
}

/// Stops whatever is left in the run's Wine prefix (`wineserver -k`), so a
/// killed installer leaves no setup process behind. A native run has none.
pub fn stop_prefix(cmd: &InstallerCommand) {
    let Some(prefix) = &cmd.prefix else {
        return;
    };
    let Some(wineserver) = find_wineserver(&cmd.compat) else {
        return;
    };
    let mut kill = Command::new(wineserver);
    kill.arg("-k")
        .env_clear()
        .envs(clean_env())
        .env("WINEPREFIX", prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Ok(mut child) = kill.spawn() {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// The `wineserver` that belongs to `compat`: the system one for `wine`;
/// for Proton, the build's own (`files/bin` or `dist/bin`) when `compat`
/// is its folder or names one under the Steam compat-tool roots; else the
/// system one.
pub fn find_wineserver(compat: &str) -> Option<PathBuf> {
    let compat = compat.trim();
    if compat.is_empty() {
        return None;
    }
    if compat != "wine" {
        let mut dirs = vec![PathBuf::from(compat)];
        if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
            dirs.extend(steam_roots(&home).iter().map(|root| root.join(compat)));
        }
        for dir in dirs {
            for sub in ["files", "dist"] {
                let candidate = dir.join(sub).join("bin").join("wineserver");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    which_on_path("wineserver")
}

/// The real spawner: a child with a clean environment, no stdin and its
/// output discarded (never read, never logged). On unix it leads its own
/// process group, so a kill reaches everything it started; on Windows the
/// kill takes its whole tree (`taskkill /T`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSpawner;

impl InstallerSpawner for SystemSpawner {
    fn spawn(&self, cmd: &InstallerCommand) -> io::Result<Box<dyn InstallerProcess>> {
        let (program, args) = cmd
            .argv
            .split_first()
            .ok_or_else(|| io::Error::other("empty installer command"))?;
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(&cmd.cwd)
            .env_clear()
            .envs(clean_env())
            .envs(cmd.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let child = command.spawn()?;
        Ok(Box::new(SystemProcess { child }))
    }
}

struct SystemProcess {
    child: Child,
}

impl InstallerProcess for SystemProcess {
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
        Ok(self.child.try_wait()?.map(|status| status.code()))
    }

    fn kill(&mut self) {
        kill_tree(&mut self.child);
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
fn kill_tree(child: &mut Child) {
    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
        // SAFETY: `kill` with a negative pid signals the process group the
        // child leads (spawned with `process_group(0)`); it touches no
        // memory.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
}

#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    // Inno's setup.exe starts a setup.tmp child: take the whole tree.
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

#[cfg(not(any(unix, windows)))]
fn kill_tree(child: &mut Child) {
    let _ = child.kill();
}

/// The spawner [`super::run_into`] uses.
pub fn system_spawner() -> Arc<dyn InstallerSpawner> {
    Arc::new(SystemSpawner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn silent_and_wizard_argv() {
        let dir = windows_path(Path::new("/games/GOG/Quarry Nine"), false);
        assert_eq!(dir, "Z:\\games\\GOG\\Quarry Nine");
        let log = windows_path(
            Path::new("/games/GOG/.grid-installer-x/grid-install.log"),
            false,
        );
        assert_eq!(
            silent_args(&dir, &log, "french"),
            vec![
                "/SP-",
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/NOCANCEL",
                "/NOICONS",
                "/LANG=french",
                "/DIR=Z:\\games\\GOG\\Quarry Nine",
                "/LOG=Z:\\games\\GOG\\.grid-installer-x\\grid-install.log",
            ]
        );
        assert_eq!(
            wizard_args("C:\\Games\\Q", "C:\\t\\grid-install.log", ""),
            vec![
                "/LANG=english",
                "/DIR=C:\\Games\\Q",
                "/LOG=C:\\t\\grid-install.log"
            ]
        );
        assert_eq!(
            windows_path(Path::new("C:\\Games\\Q"), true),
            "C:\\Games\\Q"
        );
    }

    #[test]
    fn exit_codes_follow_the_spike_table() {
        assert_eq!(exit_meaning(Some(0)), ExitMeaning::Success);
        for code in [2, 5] {
            assert_eq!(exit_meaning(Some(code)), ExitMeaning::Cancelled);
        }
        for code in [1, 3, 4, 7, 8] {
            assert_eq!(exit_meaning(Some(code)), ExitMeaning::NeedsWizard);
        }
        for code in [Some(6), Some(-1), None] {
            assert_eq!(exit_meaning(code), ExitMeaning::Failed);
        }
    }

    struct Scripted {
        polls_left: usize,
        exit: Option<i32>,
        killed: Arc<AtomicUsize>,
    }

    impl InstallerProcess for Scripted {
        fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
            if self.polls_left == 0 {
                return Ok(Some(self.exit));
            }
            self.polls_left -= 1;
            Ok(None)
        }
        fn kill(&mut self) {
            self.killed.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn fast() -> Watch {
        Watch {
            stall: Duration::from_millis(30),
            poll: Duration::from_millis(5),
        }
    }

    #[test]
    fn a_growing_run_is_never_stalled() {
        let killed = Arc::new(AtomicUsize::new(0));
        let mut process = Scripted {
            polls_left: 40,
            exit: Some(0),
            killed: killed.clone(),
        };
        let mut n = 0;
        let end = supervise(
            &mut process,
            &mut || {
                n += 1;
                n
            },
            fast(),
            true,
            &|| false,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(end, RunEnd::Exited(Some(0)));
        assert_eq!(killed.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_quiet_run_stalls_and_is_killed() {
        let killed = Arc::new(AtomicUsize::new(0));
        let mut process = Scripted {
            polls_left: usize::MAX,
            exit: None,
            killed: killed.clone(),
        };
        let end = supervise(
            &mut process,
            &mut || 7,
            fast(),
            true,
            &|| false,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(end, RunEnd::Stalled);
        assert_eq!(killed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_wizard_has_no_stall_check_and_cancel_kills() {
        let killed = Arc::new(AtomicUsize::new(0));
        let mut process = Scripted {
            polls_left: 20,
            exit: Some(1),
            killed: killed.clone(),
        };
        let end = supervise(
            &mut process,
            &mut || 7,
            fast(),
            false,
            &|| false,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(end, RunEnd::Exited(Some(1)));

        let mut process = Scripted {
            polls_left: usize::MAX,
            exit: None,
            killed: killed.clone(),
        };
        let end = supervise(
            &mut process,
            &mut || 7,
            fast(),
            false,
            &|| true,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(end, RunEnd::Cancelled);
        assert_eq!(killed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_milder_run_drops_verysilent_nocancel_and_noicons() {
        let args = milder_args("Z:\\g", "Z:\\l\\grid-install-2.log", "");
        assert_eq!(
            args,
            vec![
                "/SP-",
                "/SILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/LANG=english",
                "/DIR=Z:\\g",
                "/LOG=Z:\\l\\grid-install-2.log",
            ]
        );
        for flag in ["/VERYSILENT", "/NOCANCEL", "/NOICONS"] {
            assert!(!args.iter().any(|a| a == flag));
        }
        assert_eq!(log_name(0), "grid-install.log");
        assert_eq!(log_name(1), "grid-install-2.log");
    }

    #[test]
    fn tv_mode_never_reaches_the_wizard() {
        assert_eq!(attempts(false), &[Attempt::Silent, Attempt::Milder]);
        assert_eq!(
            attempts(true),
            &[Attempt::Silent, Attempt::Milder, Attempt::Wizard]
        );
    }

    #[test]
    fn a_silent_run_is_judged_by_its_end_and_its_log() {
        let rolled_back = "Rolling back changes.\nThere is not enough space on the disk.\n";
        assert_eq!(
            judge_silent(RunEnd::Exited(Some(0)), "Installation process succeeded.\n"),
            SilentOutcome::Installed
        );
        assert_eq!(
            judge_silent(RunEnd::Exited(Some(0)), rolled_back),
            SilentOutcome::Failed(InstallerReason::DiskFull)
        );
        assert_eq!(
            judge_silent(RunEnd::Exited(Some(1)), ""),
            SilentOutcome::Failed(InstallerReason::Unknown)
        );
        assert_eq!(
            judge_silent(RunEnd::Exited(Some(5)), ""),
            SilentOutcome::Failed(InstallerReason::Cancelled)
        );
        assert_eq!(
            judge_silent(RunEnd::Exited(Some(5)), "Access is denied.\n"),
            SilentOutcome::Failed(InstallerReason::FileInUse)
        );
        assert_eq!(
            judge_silent(RunEnd::Stalled, ""),
            SilentOutcome::Failed(InstallerReason::Unknown)
        );
        assert_eq!(
            judge_silent(RunEnd::Cancelled, rolled_back),
            SilentOutcome::Cancelled
        );
    }

    // A Windows host runs the installer natively: these script Wine's
    // `Z:` paths.
    #[cfg(not(windows))]
    mod sequence {
        use super::*;

        /// Exits each run with the next scripted code, writing the scripted
        /// log text to the run's `/LOG` first; records every argv.
        struct Runs {
            script: Vec<(Option<i32>, &'static str)>,
            argvs: std::sync::Mutex<Vec<Vec<String>>>,
        }

        impl InstallerSpawner for Runs {
            fn spawn(&self, cmd: &InstallerCommand) -> io::Result<Box<dyn InstallerProcess>> {
                let mut argvs = self.argvs.lock().unwrap();
                let (exit, log) = self.script[argvs.len().min(self.script.len() - 1)];
                if let Some(path) = cmd.argv.iter().find_map(|a| a.strip_prefix("/LOG=")) {
                    let path = path.strip_prefix("Z:").unwrap_or(path).replace('\\', "/");
                    fs::write(path, log)?;
                }
                argvs.push(cmd.argv.clone());
                Ok(Box::new(Scripted {
                    polls_left: 0,
                    exit,
                    killed: Arc::new(AtomicUsize::new(0)),
                }))
            }
        }

        struct Sink {
            phases: Vec<JobPhase>,
        }

        impl StoreProgressSink for Sink {
            fn progress(&mut self, _done: u64, _total: u64) {}
            fn phase(&mut self, phase: JobPhase) {
                self.phases.push(phase);
            }
        }

        fn run_with(
            script: Vec<(Option<i32>, &'static str)>,
            wizard_allowed: bool,
        ) -> (Result<(), LibraryError>, Vec<Vec<String>>, Vec<JobPhase>) {
            let dir = tempfile::tempdir().unwrap();
            let setup = dir.path().join("setup_quarry_nine.exe");
            fs::write(&setup, b"MZ").unwrap();
            let dest = dir.path().join("GOG").join("Quarry Nine");
            let runs = Runs {
                script,
                argvs: std::sync::Mutex::new(Vec::new()),
            };
            let mut sink = Sink { phases: Vec::new() };
            let result = run_exe(
                &ExeRun {
                    setup: &setup,
                    dest: &dest,
                    prefix: None,
                    compat: "wine",
                    lang: "",
                    wizard_allowed,
                    expected_bytes: 1,
                    spawner: &runs,
                    watch: fast(),
                },
                &mut sink,
            );
            let argvs = runs.argvs.into_inner().unwrap();
            (result, argvs, sink.phases)
        }

        fn has(argv: &[String], flag: &str) -> bool {
            argv.iter().any(|a| a == flag)
        }

        #[test]
        fn a_failed_silent_run_is_retried_with_milder_flags() {
            let (result, argvs, phases) = run_with(
                vec![
                    (Some(1), "Rolling back changes.\n"),
                    (Some(0), "Log closed.\n"),
                ],
                false,
            );
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(argvs.len(), 2);
            assert!(has(&argvs[0], "/VERYSILENT"));
            assert!(has(&argvs[1], "/SILENT") && !has(&argvs[1], "/VERYSILENT"));
            assert!(argvs[1].iter().any(|a| a.ends_with("grid-install-2.log")));
            assert_eq!(
                phases,
                vec![JobPhase::InstallerSilent, JobPhase::InstallerSilent]
            );
        }

        #[test]
        fn tv_mode_fails_with_the_logs_reason_after_two_silent_runs() {
            let (result, argvs, phases) = run_with(
                vec![
                    (Some(1), "Access is denied.\n"),
                    (Some(1), "There is not enough space on the disk.\n"),
                ],
                false,
            );
            assert_eq!(argvs.len(), 2, "no setup window in TV mode");
            assert!(!phases.contains(&JobPhase::InstallerWizard));
            match result {
                Err(LibraryError::InstallerSilentFailed { reason, kept }) => {
                    assert_eq!(reason, InstallerReason::DiskFull, "the last run's reason");
                    assert!(kept);
                }
                other => panic!("{other:?}"),
            }
            // The last run named none: the first run's reason stays.
            let (result, _, _) =
                run_with(vec![(Some(1), "Access is denied.\n"), (None, "")], false);
            assert!(matches!(
                result,
                Err(LibraryError::InstallerSilentFailed {
                    reason: InstallerReason::FileInUse,
                    ..
                })
            ));
            let (result, _, _) = run_with(vec![(Some(1), "")], false);
            let text = result.unwrap_err().to_string();
            assert_eq!(
            text,
            "Couldn't run the installer silently: the installer stopped without saying why. The installer was kept."
        );
            assert!(!text.contains("desktop"));
        }

        #[test]
        fn desktop_mode_opens_the_setup_window_after_both_silent_runs() {
            let (result, argvs, phases) =
                run_with(vec![(Some(1), ""), (Some(3), ""), (Some(0), "")], true);
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(argvs.len(), 3);
            assert!(!has(&argvs[2], "/SILENT") && !has(&argvs[2], "/VERYSILENT"));
            assert_eq!(phases.last(), Some(&JobPhase::InstallerWizard));
            let (result, _, _) = run_with(vec![(Some(1), ""), (Some(1), ""), (Some(5), "")], true);
            assert!(matches!(result, Err(LibraryError::Cancelled)));
            let (result, _, _) = run_with(vec![(Some(1), ""), (Some(1), ""), (Some(6), "")], true);
            assert!(matches!(result, Err(LibraryError::InstallerExit(6))));
        }
    }

    #[test]
    fn tree_growth_counts_bytes_and_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(tree_growth(&dir.path().join("missing")), 0);
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/b/x"), b"1234").unwrap();
        fs::write(dir.path().join("y"), b"").unwrap();
        assert_eq!(tree_growth(dir.path()), 4 + 1 + 1);
    }
}
