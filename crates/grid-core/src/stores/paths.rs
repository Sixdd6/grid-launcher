//! Where the store tools keep their files: `<data_dir>/stores/<tool>/`.
//! The folder is 0700 and its files 0600 (the store-token exception in
//! AGENTS.md); Disconnect deletes it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::tools::StoreTool;

/// The folder every store tool's files live under.
pub const STORES_DIR: &str = "stores";

/// The folder GRID copies local saves into before a cloud download
/// replaces them: `<data_dir>/stores/save-backups/<source>/<id>/<epoch>/`.
/// Outside every tool folder, so Disconnect keeps it.
pub const SAVE_BACKUPS_DIR: &str = "save-backups";

/// `<data_dir>/stores/<tool>`. Does not create it.
pub fn tool_dir(data_dir: &Path, tool: StoreTool) -> PathBuf {
    data_dir.join(STORES_DIR).join(tool.name())
}

/// `<data_dir>/stores/save-backups`. Does not create it.
pub fn save_backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(STORES_DIR).join(SAVE_BACKUPS_DIR)
}

/// Creates `dir` (and its parents) and makes it 0700 on unix.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    set_mode(dir, 0o700)
}

/// Makes the file at `path` 0600 on unix (a token file, a tool's state).
/// A missing file is not an error.
pub fn make_private_file(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    set_mode(path, 0o600)
}

/// Makes the file at `path` 0700 on unix: a downloaded tool executable,
/// owner-only like every other file in the folder.
pub fn make_private_executable(path: &Path) -> io::Result<()> {
    set_mode(path, 0o700)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Deletes `<data_dir>/stores/<tool>` and everything in it. A missing
/// folder is not an error.
pub fn remove_tool_dir(data_dir: &Path, tool: StoreTool) -> io::Result<()> {
    match fs::remove_dir_all(tool_dir(data_dir, tool)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
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

    #[test]
    fn the_private_dir_is_created_and_removed() {
        let data = tempfile::tempdir().unwrap();
        let dir = tool_dir(data.path(), StoreTool::Gogdl);
        ensure_private_dir(&dir).unwrap();
        fs::write(dir.join("auth.json"), "{}").unwrap();
        make_private_file(&dir.join("auth.json")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(&dir.join("auth.json")), 0o600);
        }
        remove_tool_dir(data.path(), StoreTool::Gogdl).unwrap();
        assert!(!dir.exists());
        remove_tool_dir(data.path(), StoreTool::Gogdl).unwrap();
        make_private_file(&dir.join("gone")).unwrap();
    }
}
