//! Running a Windows GOG installer: the Inno Setup argv (silent, then the
//! wizard), the process supervisor with its stall rule, and the kill on
//! cancel. No output line of the installer is read or logged: it writes
//! only to its own log file, whose size is the progress signal.
//!
//! Spike results (docs/stores-phase1-plan.md): exit 0 is success; 2 and 5
//! mean the user cancelled; 1, 3, 4, 7 and 8 mean it failed and may work
//! with its setup window. "Needs a wizard" mostly shows as a hang, so a
//! silent run whose log and target folder have not grown for 120 s is
//! killed and treated as needing the wizard. There is no overall timeout.

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

/// The Inno Setup flags of a silent run (spike 5).
pub const SILENT_FLAGS: [&str; 6] = [
    "/SP-",
    "/VERYSILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/NOCANCEL",
    "/NOICONS",
];

/// The installer log's file name, inside a folder GRID owns.
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

/// The wizard run's arguments: the same `/LANG`, `/DIR` and `/LOG`, so the
/// setup window opens on GRID's folder, with none of the silent flags.
pub fn wizard_args(dir: &str, log: &str, lang: &str) -> Vec<String> {
    common_args(dir, log, lang)
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
    /// May work with the setup window.
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

/// Runs a Windows installer silently, then (desktop only) with its setup
/// window. `Ok` when it exited 0; the caller reads the installed
/// `goggame-*.info`.
///
/// Errors: [`LibraryError::Cancelled`] (GRID's cancel, or the installer's
/// own exit 2/5), [`LibraryError::InstallerNeedsWizard`] `{ kept: true }`
/// when the silent run failed or stalled and the wizard isn't allowed,
/// [`LibraryError::InstallerExit`] for any other failure, and
/// [`LibraryError::InstallerRefused`] when no command can be built (no
/// compat tool, umu-run missing).
pub fn run_exe(run: &ExeRun<'_>, sink: &mut dyn StoreProgressSink) -> Result<(), LibraryError> {
    let native = host_os().starts_with("win");
    fs::create_dir_all(run.dest)?;
    let logs = log_dir(run.dest)?;
    let log = logs.path().join(LOG_NAME);
    let dir_arg = windows_path(run.dest, native);
    let log_arg = windows_path(&log, native);

    let silent = command(run, silent_args(&dir_arg, &log_arg, run.lang))?;
    sink.phase(JobPhase::InstallerSilent);
    let end = run_one(run, &silent, &log, true, sink)?;
    // Falls through only when the run needs its setup window.
    match end {
        RunEnd::Exited(code) => match exit_meaning(code) {
            ExitMeaning::Success => return Ok(()),
            ExitMeaning::Cancelled => return Err(LibraryError::Cancelled),
            ExitMeaning::NeedsWizard => {}
            ExitMeaning::Failed => return Err(LibraryError::InstallerExit(code.unwrap_or(-1))),
        },
        RunEnd::Stalled => {}
        RunEnd::Cancelled => return Err(LibraryError::Cancelled),
    }
    if !run.wizard_allowed {
        return Err(LibraryError::InstallerNeedsWizard { kept: true });
    }
    let wizard = command(run, wizard_args(&dir_arg, &log_arg, run.lang))?;
    sink.phase(JobPhase::InstallerWizard);
    match run_one(run, &wizard, &log, false, sink)? {
        RunEnd::Exited(Some(0)) => Ok(()),
        RunEnd::Exited(Some(2 | 5)) | RunEnd::Cancelled => Err(LibraryError::Cancelled),
        RunEnd::Exited(code) => Err(LibraryError::InstallerExit(code.unwrap_or(-1))),
        // Never: the wizard has no stall check.
        RunEnd::Stalled => Err(LibraryError::InstallerNeedsWizard { kept: true }),
    }
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

fn to_command(launch: NativeLaunch, prefix: Option<&Path>, compat: &str) -> InstallerCommand {
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
/// ([`umu::locate`]'s order).
fn which_with_managed_umu(name: &str) -> Option<PathBuf> {
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
    fn tree_growth_counts_bytes_and_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(tree_growth(&dir.path().join("missing")), 0);
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/b/x"), b"1234").unwrap();
        fs::write(dir.path().join("y"), b"").unwrap();
        assert_eq!(tree_growth(dir.path()), 4 + 1 + 1);
    }
}
