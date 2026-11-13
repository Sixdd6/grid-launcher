//! Where the store tools keep their files: `<data_dir>/stores/<tool>/`.
//! The folder is 0700 and its files 0600 (the store-token exception in
//! AGENTS.md); Disconnect deletes it. Package A fills the bodies.

use std::io;
use std::path::{Path, PathBuf};

use super::tools::StoreTool;

/// The folder every store tool's files live under.
pub const STORES_DIR: &str = "stores";

/// `<data_dir>/stores/<tool>`. Does not create it.
pub fn tool_dir(data_dir: &Path, tool: StoreTool) -> PathBuf {
    data_dir.join(STORES_DIR).join(tool.name())
}

/// Creates `dir` (and its parents) and makes it 0700 on unix.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    let _ = dir;
    Err(io::Error::other(super::NOT_IMPLEMENTED))
}

/// Deletes `<data_dir>/stores/<tool>` and everything in it. A missing
/// folder is not an error.
pub fn remove_tool_dir(data_dir: &Path, tool: StoreTool) -> io::Result<()> {
    let _ = (data_dir, tool);
    Err(io::Error::other(super::NOT_IMPLEMENTED))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_dir_is_under_stores() {
        assert_eq!(
            tool_dir(Path::new("/data"), StoreTool::Gogdl),
            Path::new("/data").join("stores").join("gogdl")
        );
    }
}
