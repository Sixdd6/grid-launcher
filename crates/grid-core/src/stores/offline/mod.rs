//! GOG offline installers: a Linux `.sh` (read as a zip; `data/noarch/` is
//! copied and the script never runs), a Windows `setup_*.exe` (silent under
//! Wine/umu, native on Windows, the wizard only when allowed) and a macOS
//! `.pkg` (refused). Shared by the offline-installer flow, RomM archives
//! that hold an installer, and drive restores. Package B fills the bodies.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::gog::goggame::GogGameInfo;
use super::gog::GogJobCtx;
use super::{StoreError, StoreKind};
use crate::library::queue::{StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::Registry;
use crate::library::LibraryError;

/// What kind of installer a file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallerKind {
    LinuxSh,
    /// `setup_*.exe` and its `.bin` parts, in order.
    WindowsExe {
        parts: Vec<PathBuf>,
    },
    MacPkg,
}

/// What GRID found in an installer file before running it.
#[derive(Debug, Clone, PartialEq)]
pub struct Inspection {
    pub path: PathBuf,
    pub kind: InstallerKind,
    /// `None` when the identity is only known after the run.
    pub info: Option<GogGameInfo>,
    pub version: String,
    /// All parts together.
    pub bytes: u64,
    pub verdict: Verdict,
}

/// What GRID will do with an inspected installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Install,
    Update { installed_version: String },
    Dlc { base_title: String },
    Refused { title: String, body: String },
}

/// The user's choices for an offline install.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OfflineOpts {
    /// `None`: `<GOG install dir>/<name>`.
    pub install_dir: Option<PathBuf>,
    pub compat_tool: String,
}

/// Reads `path` and decides what installing it would do.
pub fn inspect(
    path: &Path,
    registry: &Registry,
    tools_dir: &Path,
) -> Result<Inspection, StoreError> {
    let _ = (path, registry, tools_dir);
    Err(StoreError::not_implemented())
}

/// The installer inside an extracted RomM archive, `None` when it holds
/// loose game files.
pub fn detect_in_dir(dir: &Path) -> Option<PathBuf> {
    let _ = dir;
    None
}

/// The Downloads job (`StoreOp::Installer`) that installs `insp`.
pub fn install_job(
    insp: Inspection,
    opts: OfflineOpts,
    ctx: GogJobCtx,
    wizard_allowed: Arc<dyn Fn() -> bool + Send + Sync>,
) -> StoreJob {
    let _ = (opts, ctx, wizard_allowed);
    let store_id = insp
        .info
        .as_ref()
        .map(|info| info.game_id.clone())
        .unwrap_or_default();
    let title = insp
        .info
        .as_ref()
        .map(|info| info.name.clone())
        .unwrap_or_default();
    StoreJob {
        source: StoreKind::Gog,
        store_id,
        op: StoreOp::Installer,
        title,
        drive: None,
        runner: Arc::new(Unimplemented),
    }
}

/// Runs (or unpacks) the installer into `dest` and returns the installed
/// game's info. Shared by the RomM path and the drive restore.
pub fn run_into(
    insp: &Inspection,
    dest: &Path,
    prefix: Option<&Path>,
    compat: &str,
    wizard_allowed: bool,
    sink: &mut dyn StoreProgressSink,
) -> Result<GogGameInfo, LibraryError> {
    let _ = (insp, dest, prefix, compat, wizard_allowed, sink);
    Err(LibraryError::Extract(super::NOT_IMPLEMENTED.to_string()))
}

struct Unimplemented;

impl StoreJobRunner for Unimplemented {
    fn download(
        &self,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Extract(super::NOT_IMPLEMENTED.to_string()))
    }

    fn finalize(&self, _sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        Err(LibraryError::Extract(super::NOT_IMPLEMENTED.to_string()))
    }
}
