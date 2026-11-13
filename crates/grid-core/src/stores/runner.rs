//! The only place a store tool process is spawned. Its output lines are
//! never logged or forwarded (AGENTS.md "Secrets"): progress goes through
//! [`super::progress`], and stdout is kept only as a [`SecretString`] for a
//! JSON parse.
//!
//! The child runs in its own process group (a new process group on
//! Windows), so a cancel stops the tool and every worker it started.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use secrecy::SecretString;

use super::progress::{parse_gogdl_line, ToolProgress};
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

/// One tool invocation. Its `Debug` form never prints the arguments or the
/// environment (a login code is an argument).
#[derive(Clone, Default)]
pub struct ToolCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
}

impl std::fmt::Debug for ToolCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCommand")
            .field("program", &self.program)
            .field("args", &format_args!("<{} args>", self.args.len()))
            .field("env", &format_args!("<{} vars>", self.env.len()))
            .field("cwd", &self.cwd)
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
}

impl ToolExit {
    /// Whether the tool exited 0.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs `cmd` to the end, or until `cancel` is set (the process group is
/// then stopped and [`LibraryError::Cancelled`] returned through
/// [`StoreError::Library`]). Every stderr line goes through
/// [`parse_gogdl_line`]; a progress report reaches `on_progress`, any
/// other line is dropped. Blocking.
pub fn run_tool(
    cmd: &ToolCommand,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ToolProgress),
) -> Result<ToolExit, StoreError> {
    let tool = tool_label(cmd);
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .env_clear()
        .envs(crate::launch::spawn::clean_env())
        .envs(cmd.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
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

    let (tx, rx) = mpsc::channel::<ToolProgress>();
    let stderr = child.stderr.take();
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
                    if let Some(report) = parse_gogdl_line(&text) {
                        if tx.send(report).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });
    let stdout = child.stdout.take();
    let stdout_reader = thread::spawn(move || {
        let mut kept = Vec::new();
        let Some(mut stdout) = stdout else {
            return kept;
        };
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = STDOUT_CAP.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..n.min(room)]);
                }
            }
        }
        kept
    });

    let status = loop {
        while let Ok(report) = rx.try_recv() {
            on_progress(report);
        }
        if cancel.load(Ordering::Relaxed) {
            stop_group(&mut child);
            let _ = stderr_reader.join();
            let _ = stdout_reader.join();
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
    while let Ok(report) = rx.try_recv() {
        on_progress(report);
    }
    let bytes = stdout_reader.join().unwrap_or_default();
    Ok(ToolExit {
        code: exit_code(&status),
        stdout: SecretString::from(String::from_utf8_lossy(&bytes).into_owned()),
    })
}

/// The tool's name for an error: `gogdl` for anything that runs gogdl.
fn tool_label(cmd: &ToolCommand) -> &'static str {
    let _ = cmd;
    super::tools::StoreTool::Gogdl.name()
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
        // The test reads the kept stdout the way a JSON parse would.
        let out = crate::stores::gog::auth::read_stdout(&exit.stdout, str::to_string);
        assert_eq!(out, r#"{"ok": "arg-one"}"#);
        assert_eq!(reports.len(), 1);
        assert_eq!((reports[0].done, reports[0].total), (5, 10));
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
    }

    #[test]
    fn debug_never_prints_arguments() {
        let cmd = ToolCommand {
            program: PathBuf::from("gogdl"),
            args: vec!["--code".into(), "FAKE-STORE-TOKEN-not-real".into()],
            env: vec![("X".into(), "FAKE-STORE-TOKEN-not-real".into())],
            cwd: None,
        };
        let text = format!("{cmd:?}");
        assert!(!text.contains("FAKE-STORE-TOKEN"), "{text}");
        assert!(text.contains("<2 args>"), "{text}");
    }
}
