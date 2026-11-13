//! The GRID-managed store tools (heroic-gogdl in Phase 1): their pinned
//! releases and SHA-256s,
//! and the install that checks them. Package A fills the pins and bodies.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::StoreError;

/// One store tool GRID downloads and runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreTool {
    Gogdl,
}

impl StoreTool {
    /// The lowercase name: its folder under `<data_dir>/stores/` and its
    /// serde form.
    pub fn name(self) -> &'static str {
        match self {
            StoreTool::Gogdl => "gogdl",
        }
    }
}

/// How a downloaded asset is started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetLauncher {
    /// A native executable, run as itself.
    Direct,
    /// A Python zipapp, run as `python3 <file>` (gogdl on Linux).
    Python3,
}

/// One upstream release asset, for one OS and architecture.
#[derive(Debug, PartialEq, Eq)]
pub struct PinnedAsset {
    pub os: &'static str,
    pub arch: &'static str,
    pub url: &'static str,
    pub file_name: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    pub launcher: AssetLauncher,
}

/// A tool's pinned release.
#[derive(Debug, PartialEq, Eq)]
pub struct ToolPin {
    pub tool: StoreTool,
    pub version: &'static str,
    pub license: &'static str,
    pub source_url: &'static str,
    pub assets: &'static [PinnedAsset],
}

// Step 0 placeholders: no assets pinned yet (spike 1 picks the release).
static GOGDL_PIN: ToolPin = ToolPin {
    tool: StoreTool::Gogdl,
    version: "",
    license: "GPL-3.0",
    source_url: "https://github.com/Heroic-Games-Launcher/heroic-gogdl",
    assets: &[],
};

/// The tool's pinned release.
pub fn pin(tool: StoreTool) -> &'static ToolPin {
    match tool {
        StoreTool::Gogdl => &GOGDL_PIN,
    }
}

/// The asset for this computer's OS and architecture, `None` when the tool
/// has none for it.
pub fn host_asset(tool: StoreTool) -> Option<&'static PinnedAsset> {
    pin(tool)
        .assets
        .iter()
        .find(|asset| asset.os == std::env::consts::OS && asset.arch == std::env::consts::ARCH)
}

/// The installed tool's executable, `None` when it is not installed. With
/// the `e2e` feature, `GRID_LAUNCHER_E2E_TOOL_<TOOL>` names a fake tool.
pub fn installed_path(data_dir: &Path, tool: StoreTool) -> Option<PathBuf> {
    let _ = (data_dir, tool);
    None
}

/// Downloads the host asset into the tool's folder and checks its SHA-256
/// and size. A mismatch deletes the file and fails with
/// [`StoreError::Refused`]. `progress(done, total)`.
pub async fn install_tool(
    http: &reqwest::Client,
    data_dir: &Path,
    tool: StoreTool,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<PathBuf, StoreError> {
    let _ = (http, data_dir, tool, progress);
    Err(StoreError::not_implemented())
}
