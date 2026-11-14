//! The only place a store tool process is spawned. Its output lines are
//! never logged or forwarded (AGENTS.md "Secrets"): progress goes through
//! [`super::progress`], a legendary line GRID knows becomes a
//! [`ToolSignal`] (GRID's words; the line itself is dropped), and stdout is
//! kept only as a [`SecretString`] for a JSON parse.
//!
//! A secret the tool reads from stdin (legendary's login code) is written
//! by this file and nowhere else: [`ToolCommand::stdin`].
//!
//! The child runs in its own process group (a new process group on
//! Windows), so a cancel stops the tool and every worker it started.

use std::io::{BufRead, BufReader, Read, Write as _};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use secrecy::{ExposeSecret as _, SecretString};

use super::paths;
use super::progress::{
    legendary_signal, nile_signal, parse_gogdl_line, parse_legendary_line, parse_nile_line,
    ToolProgress,
};
use super::tools::StoreTool;
use super::StoreError;
use crate::library::LibraryError;

/// How often the runner looks at the child, the cancel flag and the
/// progress channel.
const POLL: Duration = Duration::from_millis(100);
/// How long a cancelled tool gets to stop after SIGTERM before SIGKILL.
#[cfg(unix)]
const TERM_GRACE: Duration = Duration::from_secs(3);
/// The most stdout the runner keeps (a JSON answer is a few KiB; anything
/// past this is dropped).
const STDOUT_CAP: usize = 16 * 1024 * 1024;
/// The longest stdout line the runner reads for progress or a signal. A
/// longer line (a JSON answer on one line) is skipped; it is still kept in
/// [`ToolExit::stdout`].
const STDOUT_LINE_CAP: usize = 8 * 1024;

/// Which tool's lines a run reads: its progress parser and, for legendary
/// and nile, its signal matcher ([`super::progress::legendary_signal`],
/// [`super::progress::nile_signal`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineParser {
    /// heroic-gogdl: [`parse_gogdl_line`] on stderr, no signals.
    #[default]
    Gogdl,
    /// legendary: [`parse_legendary_line`] and [`legendary_signal`] on
    /// stderr and on stdout's lines.
    Legendary,
    /// nile: [`parse_nile_line`] and [`nile_signal`] on stderr and on
    /// stdout's lines (its `Checksum error for` line is on stdout).
    Nile,
}

impl LineParser {
    /// The tool these lines come from (the name in an error).
    pub fn tool(self) -> StoreTool {
        match self {
            LineParser::Gogdl => StoreTool::Gogdl,
            LineParser::Legendary => StoreTool::Legendary,
            LineParser::Nile => StoreTool::Nile,
        }
    }

    fn progress(self, line: &str) -> Option<ToolProgress> {
        match self {
            LineParser::Gogdl => parse_gogdl_line(line),
            LineParser::Legendary => parse_legendary_line(line),
            LineParser::Nile => parse_nile_line(line),
        }
    }

    fn signal(self, line: &str) -> Option<ToolSignal> {
        match self {
            LineParser::Gogdl => None,
            LineParser::Legendary => legendary_signal(line),
            LineParser::Nile => nile_signal(line),
        }
    }

    /// Whether stdout's lines are read for signals too (every parser with
    /// a signal matcher).
    fn reads_stdout(self) -> bool {
        !matches!(self, LineParser::Gogdl)
    }
}

/// What a known tool output line means, in GRID's own words. The raw line
/// is never kept: a job decides from these (legendary's exit codes are
/// unreliable, `docs/stores-phase2-plan.md` "Spike results").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSignal {
    /// The login code was refused (`Login attempt failed`).
    LoginFailed,
    /// No saved login, or Epic refused the saved one (expired).
    NotLoggedIn,
    /// Epic, or legendary's own API, could not be reached.
    Network,
    /// The download manager ran out of shared memory (retry with
    /// `--max-shared-memory`).
    MemoryError,
    NotEnoughSpace,
    /// The title installs through another company's launcher.
    ThirdPartyManaged,
    /// The game (or the base game of a DLC) is not installed.
    NotInstalled,
    /// `Verification finished successfully.`
    VerifyOk,
    VerifyFailed {
        corrupted: u32,
        missing: u32,
    },
    /// The download size legendary announced, in bytes.
    DownloadSize(u64),
    /// Any other `ERROR`/`CRITICAL` line, or a Python traceback.
    OtherError,
    /// The tool's binary can't load on this system: a dynamic loader
    /// error such as ``version `GLIBC_2.35' not found`` (nile's frozen
    /// Linux build needs glibc 2.35).
    SystemTooOld,
}

/// One tool invocation. Its `Debug` form never prints the arguments, the
/// environment or the stdin secret (a login code is an argument for gogdl
/// and stdin for legendary).
#[derive(Clone, Default)]
pub struct ToolCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    /// Written to the tool's stdin, then a newline, then stdin is closed.
    /// `None`: stdin is null.
    pub stdin: Option<SecretString>,
    /// Which tool's progress and signal lines to read.
    pub parser: LineParser,
    /// Also read progress from stdout, split on `\r` and `\n` (legendary's
    /// verify step).
    pub progress_stdout: bool,
    /// A folder made owner-only ([`paths::make_tree_private`]) when the run
    /// ends, however it ends: legendary's config folder, whose token files
    /// it writes with no mode.
    pub private_tree: Option<PathBuf>,
    /// How long the run may take before the runner stops the process
    /// group and fails with [`TIMED_OUT`]; `None` waits for ever. nile
    /// sets no timeout on its own requests, so a short nile command can
    /// hang on a network that drops packets (`docs/stores-phase3-plan.md`
    /// "Spike results").
    pub timeout: Option<Duration>,
}

/// The reason in [`StoreError::ToolFailed`] when a run outlived its
/// [`ToolCommand::timeout`].
pub const TIMED_OUT: &str = "it didn't finish in time";

impl std::fmt::Debug for ToolCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCommand")
            .field("program", &self.program)
            .field("args", &format_args!("<{} args>", self.args.len()))
            .field("env", &format_args!("<{} vars>", self.env.len()))
            .field("cwd", &self.cwd)
            .field(
                "stdin",
                &format_args!(
                    "{}",
                    if self.stdin.is_some() {
                        "<set>"
                    } else {
                        "none"
                    }
                ),
            )
            .field("parser", &self.parser)
            .field("progress_stdout", &self.progress_stdout)
            .field("private_tree", &self.private_tree)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// How a tool run ended.
#[derive(Debug)]
pub struct ToolExit {
    /// The exit code; `-signum` when a signal ended it (unix).
    pub code: Option<i32>,
    /// Kept only for a JSON parse; never printed.
    pub stdout: SecretString,
    /// The known lines the run printed, in the order they came, each once.
    /// Always empty for [`LineParser::Gogdl`].
    pub signals: Vec<ToolSignal>,
}

impl ToolExit {
    /// Whether the tool exited 0.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Whether the run printed `signal`.
    pub fn has(&self, signal: &ToolSignal) -> bool {
        self.signals.contains(signal)
    }

    /// Hands the kept stdout to `parse` (a JSON parse); the text goes
    /// nowhere else.
    pub fn read_stdout<T>(&self, parse: impl FnOnce(&str) -> T) -> T {
        parse(self.stdout.expose_secret())
    }
}

/// Signals in arrival order, each kept once.
#[derive(Default)]
struct Signals(Mutex<Vec<ToolSignal>>);

impl Signals {
    fn push(&self, signal: ToolSignal) {
        let mut list = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !list.contains(&signal) {
            list.push(signal);
        }
    }

    fn take(&self) -> Vec<ToolSignal> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Runs `cmd` to the end, or until `cancel` is set (the process group is
/// then stopped and [`LibraryError::Cancelled`] returned through
/// [`StoreError::Library`]). Every stderr line goes through the command's
/// [`LineParser`]: a progress report reaches `on_progress`, a known
/// legendary line becomes a [`ToolSignal`], any other line is dropped.
/// stdout is kept; with [`LineParser::Legendary`] (or
/// [`ToolCommand::progress_stdout`]) its lines are read the same way.
/// [`ToolCommand::stdin`] is written, then stdin closed; a tool that exits
/// before reading it is not an error. A run past [`ToolCommand::timeout`]
/// is stopped like a cancel and fails with [`TIMED_OUT`]. Blocking.
pub fn run_tool(
    cmd: &ToolCommand,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ToolProgress),
) -> Result<ToolExit, StoreError> {
    let result = run(cmd, cancel, on_progress);
    if let Some(dir) = &cmd.private_tree {
        let _ = paths::make_tree_private(dir);
    }
    result
}

fn run(
    cmd: &ToolCommand,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ToolProgress),
) -> Result<ToolExit, StoreError> {
    let tool = cmd.parser.tool().name();
    let parser = cmd.parser;
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .env_clear()
        .envs(crate::launch::spawn::clean_env())
        .envs(cmd.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(if cmd.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &cmd.cwd {
        command.current_dir(cwd);
    }
    own_process_group(&mut command);

    let mut child = command.spawn().map_err(|error| StoreError::ToolFailed {
        tool,
        reason: format!("couldn't start it ({})", error.kind()),
    })?;

    // The secret goes in on its own thread, so a tool that never reads
    // stdin cannot block the run; a closed pipe (the tool exited first) is
    // fine.
    let stdin_writer = match (cmd.stdin.clone(), child.stdin.take()) {
        (Some(secret), Some(mut pipe)) => Some(thread::spawn(move || {
            let _ = pipe.write_all(secret.expose_secret().as_bytes());
            let _ = pipe.write_all(b"\n");
            let _ = pipe.flush();
            drop(pipe);
            drop(secret);
        })),
        _ => None,
    };

    let signals = Arc::new(Signals::default());
    let (tx, rx) = mpsc::channel::<ToolProgress>();
    let stderr = child.stderr.take();
    let stderr_tx = tx.clone();
    let stderr_signals = signals.clone();
    let stderr_reader = thread::spawn(move || {
        let Some(stderr) = stderr else { return };
        let mut reader = BufReader::new(stderr);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&line);
                    if let Some(signal) = parser.signal(&text) {
                        stderr_signals.push(signal);
                    }
                    if let Some(report) = parser.progress(&text) {
                        if stderr_tx.send(report).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });
    let stdout = child.stdout.take();
    let read_lines = cmd.progress_stdout || parser.reads_stdout();
    let stdout_progress = cmd.progress_stdout;
    let stdout_signals = signals.clone();
    let stdout_reader = thread::spawn(move || {
        let mut kept = Vec::new();
        let Some(mut stdout) = stdout else {
            return kept;
        };
        let mut lines = LineSplitter::default();
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = STDOUT_CAP.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..n.min(room)]);
                    if read_lines {
                        lines.feed(&buf[..n], &mut |text| {
                            if let Some(signal) = parser.signal(text) {
                                stdout_signals.push(signal);
                            }
                            if stdout_progress {
                                if let Some(report) = parser.progress(text) {
                                    let _ = tx.send(report);
                                }
                            }
                        });
                    }
                }
            }
        }
        if read_lines {
            lines.finish(&mut |text| {
                if let Some(signal) = parser.signal(text) {
                    stdout_signals.push(signal);
                }
            });
        }
        kept
    });

    let deadline = cmd
        .timeout
        .map(|timeout| std::time::Instant::now() + timeout);
    let status = loop {
        while let Ok(report) = rx.try_recv() {
            on_progress(report);
        }
        if deadline.is_some_and(|at| std::time::Instant::now() >= at) {
            stop_group(&mut child);
            let _ = stderr_reader.join();
            let _ = stdout_reader.join();
            if let Some(writer) = stdin_writer {
                let _ = writer.join();
            }
            return Err(StoreError::ToolFailed {
                tool,
                reason: TIMED_OUT.to_string(),
            });
        }
        if cancel.load(Ordering::Relaxed) {
            stop_group(&mut child);
            let _ = stderr_reader.join();
            let _ = stdout_reader.join();
            if let Some(writer) = stdin_writer {
                let _ = writer.join();
            }
            return Err(StoreError::Library(LibraryError::Cancelled));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => match rx.recv_timeout(POLL) {
                Ok(report) => on_progress(report),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => thread::sleep(POLL),
            },
            Err(error) => {
                stop_group(&mut child);
                return Err(StoreError::ToolFailed {
                    tool,
                    reason: format!("couldn't wait for it ({})", error.kind()),
                });
            }
        }
    };
    let _ = stderr_reader.join();
    let bytes = stdout_reader.join().unwrap_or_default();
    if let Some(writer) = stdin_writer {
        let _ = writer.join();
    }
    while let Ok(report) = rx.try_recv() {
        on_progress(report);
    }
    Ok(ToolExit {
        code: exit_code(&status),
        stdout: SecretString::from(String::from_utf8_lossy(&bytes).into_owned()),
        signals: signals.take(),
    })
}

/// Splits a byte stream into lines on `\r` and `\n` (a progress line that
/// rewrites itself with `\r` counts as a line each time). A line longer
/// than [`STDOUT_LINE_CAP`] is skipped whole.
#[derive(Default)]
struct LineSplitter {
    line: Vec<u8>,
    too_long: bool,
}

impl LineSplitter {
    fn feed(&mut self, bytes: &[u8], on_line: &mut dyn FnMut(&str)) {
        for &byte in bytes {
            if byte == b'\n' || byte == b'\r' {
                self.emit(on_line);
                continue;
            }
            if self.too_long {
                continue;
            }
            if self.line.len() >= STDOUT_LINE_CAP {
                self.too_long = true;
                self.line.clear();
                continue;
            }
            self.line.push(byte);
        }
    }

    fn finish(&mut self, on_line: &mut dyn FnMut(&str)) {
        self.emit(on_line);
    }

    fn emit(&mut self, on_line: &mut dyn FnMut(&str)) {
        if !self.too_long && !self.line.is_empty() {
            on_line(&String::from_utf8_lossy(&self.line));
        }
        self.line.clear();
        self.too_long = false;
    }
}
#[cfg(unix)]
fn own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn own_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(not(any(unix, windows)))]
fn own_process_group(_command: &mut Command) {}

/// Stops the child and its process group: SIGTERM to the group, then
/// SIGKILL after [`TERM_GRACE`] (unix); `taskkill /T /F` (Windows).
#[cfg(unix)]
fn stop_group(child: &mut Child) {
    let Ok(pid) = i32::try_from(child.id()) else {
        let _ = child.kill();
        let _ = child.wait();
        return;
    };
    // SAFETY: kill(2) with a negative pid signals the process group the
    // child leads (process_group(0) made its pgid its pid).
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
    }
    let deadline = Instant::now() + TERM_GRACE;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    // SAFETY: as above; the group may already be gone (ESRCH is fine).
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn stop_group(child: &mut Child) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(any(unix, windows)))]
fn stop_group(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| -signal))
}

#[cfg(not(unix))]
fn exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    status.code()
}

#[cfg(test)]
mod splitter_tests {
    use super::*;

    #[test]
    fn lines_split_on_both_breaks_and_long_lines_are_skipped() {
        let mut seen = Vec::new();
        let mut splitter = LineSplitter::default();
        splitter.feed(b"a\rb\r\nc", &mut |line| seen.push(line.to_string()));
        splitter.feed(b"d\n", &mut |line| seen.push(line.to_string()));
        let long = vec![b'x'; STDOUT_LINE_CAP + 10];
        splitter.feed(&long, &mut |line| seen.push(line.to_string()));
        splitter.feed(b"\nlast", &mut |line| seen.push(line.to_string()));
        splitter.finish(&mut |line| seen.push(line.to_string()));
        assert_eq!(seen, ["a", "b", "cd", "last"]);
    }

    #[test]
    fn debug_never_prints_arguments_or_stdin() {
        let cmd = ToolCommand {
            program: PathBuf::from("gogdl"),
            args: vec!["--code".into(), "FAKE-STORE-TOKEN-not-real".into()],
            env: vec![("X".into(), "FAKE-STORE-TOKEN-not-real".into())],
            stdin: Some(SecretString::from("FAKE-STORE-TOKEN-not-real")),
            parser: LineParser::Legendary,
            ..Default::default()
        };
        let text = format!("{cmd:?}");
        assert!(!text.contains("FAKE-STORE-TOKEN"), "{text}");
        assert!(text.contains("<2 args>"), "{text}");
        assert!(text.contains("stdin: <set>"), "{text}");
        let exit = ToolExit {
            code: Some(0),
            stdout: SecretString::from("FAKE-STORE-TOKEN-not-real"),
            signals: vec![ToolSignal::LoginFailed],
        };
        assert!(!format!("{exit:?}").contains("FAKE-STORE-TOKEN"));
    }

    #[test]
    fn parsers_name_their_tool() {
        assert_eq!(LineParser::default(), LineParser::Gogdl);
        assert_eq!(LineParser::Gogdl.tool().name(), "gogdl");
        assert_eq!(LineParser::Legendary.tool().name(), "legendary");
        assert_eq!(LineParser::Nile.tool().name(), "nile");
        assert_eq!(
            LineParser::Nile.signal("ERROR [CLI]:\t User not logged in"),
            Some(ToolSignal::NotLoggedIn)
        );
        assert!(LineParser::Nile.reads_stdout());
        assert!(LineParser::Legendary.reads_stdout());
        assert!(!LineParser::Gogdl.reads_stdout());
        assert_eq!(LineParser::Gogdl.signal("[cli] ERROR: x"), None);
        assert_eq!(
            LineParser::Legendary.signal("[cli] ERROR: x"),
            Some(ToolSignal::OtherError)
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("tool.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn stdout_is_kept_and_progress_is_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"echo '[PROGRESS] INFO: = Progress: 50.00 5/10, Running for: 00:00:01, ETA: 00:00:01' >&2
echo '[AUTH] INFO: secret-ish line' >&2
printf '{"ok": "%s"}' "$1"
exit 3"#,
        );
        let cmd = ToolCommand {
            program,
            args: vec!["arg-one".into()],
            ..Default::default()
        };
        let mut reports = Vec::new();
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |p| reports.push(p)).unwrap();
        assert_eq!(exit.code, Some(3));
        assert!(!exit.success());
        let out = exit.read_stdout(str::to_string);
        assert_eq!(out, r#"{"ok": "arg-one"}"#);
        assert_eq!(reports.len(), 1);
        assert_eq!((reports[0].done, reports[0].total), (5, 10));
        assert!(exit.signals.is_empty(), "gogdl has no signals");
    }

    #[test]
    fn a_cancel_stops_the_whole_group() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("child-alive");
        let program = script(
            dir.path(),
            &format!(
                "(sleep 2; touch '{}') &\nsleep 30",
                marker.to_string_lossy()
            ),
        );
        let cmd = ToolCommand {
            program,
            ..Default::default()
        };
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        let result = thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(300));
                cancel.store(true, Ordering::Relaxed);
            });
            run_tool(&cmd, &cancel, &mut |_| {})
        });
        assert!(matches!(
            result,
            Err(StoreError::Library(LibraryError::Cancelled))
        ));
        assert!(started.elapsed() < Duration::from_secs(10));
        thread::sleep(Duration::from_millis(2500));
        assert!(!marker.exists(), "the background worker was stopped too");
    }

    #[test]
    fn a_missing_program_fails_in_grids_words() {
        let cmd = ToolCommand {
            program: PathBuf::from("/nonexistent/gogdl"),
            ..Default::default()
        };
        let error = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(error.to_string().starts_with("gogdl stopped:"), "{error}");
        let cmd = ToolCommand {
            program: PathBuf::from("/nonexistent/legendary"),
            parser: LineParser::Legendary,
            ..Default::default()
        };
        let error = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(
            error.to_string().starts_with("legendary stopped:"),
            "{error}"
        );
    }

    /// The way legendary's `auth --disable-webview` reads its code (spike
    /// 2): a prompt on stdout, then one line from stdin.
    #[test]
    fn the_stdin_secret_reaches_the_tool_and_nothing_prints_it() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"printf 'Please enter the "authorizationCode" value from the JSON response: '
IFS= read -r code
if [ "$code" = "FAKE-STORE-TOKEN-not-real" ]; then
  echo '[cli] INFO: Successfully logged in as "SamPlays"' >&2
else
  echo '[cli] ERROR: Login attempt failed, please see log for details.' >&2
fi"#,
        );
        let cmd = ToolCommand {
            program,
            stdin: Some(SecretString::from("FAKE-STORE-TOKEN-not-real")),
            parser: LineParser::Legendary,
            ..Default::default()
        };
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(exit.success());
        assert!(exit.signals.is_empty(), "{:?}", exit.signals);
        assert!(!exit.read_stdout(|out| out.contains("FAKE-STORE-TOKEN")));

        let wrong = ToolCommand {
            stdin: Some(SecretString::from("wrong")),
            ..cmd
        };
        let exit = run_tool(&wrong, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(exit.success(), "legendary exits 0 on a failed login");
        assert_eq!(exit.signals, vec![ToolSignal::LoginFailed]);
    }

    #[test]
    fn a_tool_that_never_reads_stdin_is_fine_and_no_stdin_is_null() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "exit 0");
        let cmd = ToolCommand {
            program,
            stdin: Some(SecretString::from("x".repeat(256 * 1024))),
            ..Default::default()
        };
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(exit.success());

        let program = script(dir.path(), "if read -r x; then echo got; else echo eof; fi");
        let cmd = ToolCommand {
            program,
            ..Default::default()
        };
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert_eq!(exit.read_stdout(|out| out.trim().to_string()), "eof");
    }

    #[test]
    fn legendary_signals_come_from_both_streams_once_each() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"echo '[Core] WARNING: Getting locale failed' >&2
echo '[cli] INFO: Download size: 2.00 MiB (Compression savings: 1.0%)' >&2
echo '[DLManager] INFO: = Progress: 50.00% (5/10), Running for 00:00:01, ETA: 00:00:01' >&2
echo '[DLManager] INFO:  + Download	- 1.00 MiB/s (raw) / 2.00 MiB/s (decompressed)' >&2
echo ' ! Failure: Not enough available disk space! 1.00 GiB < 2.00 GiB'
echo '[cli] CRITICAL: Installation cannot proceed, exiting.' >&2
echo '[cli] CRITICAL: Installation cannot proceed, exiting.' >&2
exit 1"#,
        );
        let cmd = ToolCommand {
            program,
            parser: LineParser::Legendary,
            ..Default::default()
        };
        let mut reports = Vec::new();
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |p| reports.push(p)).unwrap();
        assert_eq!(exit.code, Some(1));
        assert!(exit.has(&ToolSignal::DownloadSize(2 * 1024 * 1024)));
        assert!(exit.has(&ToolSignal::NotEnoughSpace));
        assert!(exit.has(&ToolSignal::OtherError));
        assert_eq!(exit.signals.len(), 3, "{:?}", exit.signals);
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].done, 5000);
        assert!(reports[1].is_speed_only());
    }

    #[test]
    fn verify_progress_is_read_from_stdout_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"printf 'Verification progress: 1/4 (25.0%%) [1.0 MiB/s]\t\r'
printf 'Verification progress: 4/4 (100.0%%) [2.0 MiB/s]\t\n'
echo '[cli] ERROR: File is missing: "x"' >&2
echo '[cli] ERROR: Verification failed, 0 file(s) corrupted, 1 file(s) are missing.' >&2"#,
        );
        let cmd = ToolCommand {
            program,
            parser: LineParser::Legendary,
            progress_stdout: true,
            ..Default::default()
        };
        let mut reports = Vec::new();
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |p| reports.push(p)).unwrap();
        let fractions: Vec<u64> = reports.iter().map(|r| r.done).collect();
        assert_eq!(fractions, [2500, 10_000]);
        assert!(reports
            .iter()
            .all(|r| r.phase == crate::library::queue::JobPhase::Verifying));
        assert_eq!(
            exit.signals,
            vec![ToolSignal::VerifyFailed {
                corrupted: 0,
                missing: 1
            }]
        );
    }

    #[test]
    fn the_private_tree_is_made_owner_only_after_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("legendary");
        let program = script(
            dir.path(),
            &format!(
                "mkdir -p '{tree}/tmp'; echo '{{}}' > '{tree}/user.json'; chmod 644 '{tree}/user.json'; chmod 755 '{tree}' '{tree}/tmp'",
                tree = tree.to_string_lossy()
            ),
        );
        let cmd = ToolCommand {
            program,
            parser: LineParser::Legendary,
            private_tree: Some(tree.clone()),
            ..Default::default()
        };
        run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&tree), 0o700);
        assert_eq!(mode(&tree.join("tmp")), 0o700);
        assert_eq!(mode(&tree.join("user.json")), 0o600);
    }

    #[test]
    fn nile_signals_and_progress_come_from_both_streams() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"echo 'WARNING [CLI]:	 There was an error getting sdk' >&2
echo 'INFO [PROGRESS]:	 = Progress: 50.00 512/1024, Running for: 00:00:01, ETA: 00:00:01' >&2
echo 'INFO [PROGRESS]:	  + Download	- 2.00 MiB/s' >&2
echo 'Checksum error for /games/x/a.dll'
echo 'INFO [CLI]:	 Download complete' >&2"#,
        );
        let cmd = ToolCommand {
            program,
            parser: LineParser::Nile,
            ..Default::default()
        };
        let mut reports = Vec::new();
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |p| reports.push(p)).unwrap();
        assert!(exit.success(), "nile exits 0 after a checksum error");
        assert_eq!(exit.signals, vec![ToolSignal::OtherError]);
        assert_eq!(reports.len(), 2);
        assert_eq!((reports[0].done, reports[0].total), (512, 1024));
        assert!(reports[1].is_speed_only());
    }

    #[test]
    fn a_run_past_its_timeout_stops_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "sleep 30");
        let cmd = ToolCommand {
            program,
            parser: LineParser::Nile,
            timeout: Some(Duration::from_millis(300)),
            ..Default::default()
        };
        let started = Instant::now();
        let error = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(10));
        match error {
            StoreError::ToolFailed { tool, reason } => {
                assert_eq!(tool, "nile");
                assert_eq!(reason, TIMED_OUT);
            }
            other => panic!("{other}"),
        }
        let quick = ToolCommand {
            program: script(dir.path(), "exit 0"),
            timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        };
        assert!(run_tool(&quick, &AtomicBool::new(false), &mut |_| {})
            .unwrap()
            .success());
    }
}
