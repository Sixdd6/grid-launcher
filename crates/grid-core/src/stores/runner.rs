//! The only place a store tool process is spawned. Its output lines are
//! never logged or forwarded (AGENTS.md "Secrets"): progress goes through
//! [`super::progress`], and stdout is kept only as a [`SecretString`] for a
//! JSON parse. Package A fills the body.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use secrecy::SecretString;

use super::progress::ToolProgress;
use super::StoreError;

/// One tool invocation.
#[derive(Debug, Clone, Default)]
pub struct ToolCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
}

/// How a tool run ended.
#[derive(Debug)]
pub struct ToolExit {
    pub code: Option<i32>,
    /// Kept only for a JSON parse; never printed.
    pub stdout: SecretString,
}

/// Runs `cmd` to the end, or until `cancel` is set (the process is then
/// killed and [`crate::library::LibraryError::Cancelled`] returned through
/// [`StoreError::Library`]).
pub fn run_tool(
    cmd: &ToolCommand,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ToolProgress),
) -> Result<ToolExit, StoreError> {
    let _ = (cmd, cancel, on_progress);
    Err(StoreError::not_implemented())
}
