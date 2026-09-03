//! Platform-specific archive routing ("specials"): the logic that turns an
//! extracted archive into the layout a particular emulator's own virtual
//! filesystem expects — RPCS3's `dev_hdd0`, PS4 content, Xenia, and plain
//! native installs each get their own submodule.
//!
//! See `docs/porting/03-library-install.md` and the sibling Python modules
//! under `grid_launcher/library/` for the behavior each submodule ports.

pub mod native;
pub mod ps3;
pub mod ps4;
pub mod xenia;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Removes a staging/extraction directory (recursively, ignoring errors)
/// when dropped — used so every exit path out of a content-apply function,
/// after it has extracted an archive into a scratch directory, cleans that
/// directory up. Mirrors the Python `finally: shutil.rmtree(..., ignore_errors=True)`
/// blocks in `archive_preparation.py` (e.g. line 777 for PS4 content, line
/// 818 for Xenia content). Shared by `ps4::apply_content` and
/// `xenia::apply_content_archive`.
pub(crate) struct StagingGuard(pub PathBuf);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Copies every entry of `src` into `dst`, merging with anything already
/// there: directories are created as needed and recursed into, files
/// overwrite an existing file of the same name. Mirrors Python's
/// `shutil.copytree(src, dst, dirs_exist_ok=True)`
/// (`grid_launcher/library/ps3_install.py:282-285`, `_copytree_merge`).
///
/// Unlike [`crate::library::merge_tree_into`] in the parent module, this
/// COPIES rather than moves: `src` here is (part of) the extracted archive
/// being routed into place, not a disposable staging directory that is
/// about to be deleted wholesale.
pub(crate) fn copy_tree_merge(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree_merge(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Copies every file of `src` into `dst`, creating directories as needed,
/// and never deletes anything already under `dst`. Mirrors Python's
/// `_merge_tree` (`grid_launcher/library/archive_preparation.py:258-268`).
///
/// Not called by `ps3` — `ps4::apply_content` uses it to merge a matching
/// title-ID root from a content archive into the installed game directory;
/// the Xenia/native routing modules in later tasks may use it too.
pub(crate) fn merge_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_tree_merge_overwrites_files_and_keeps_untouched_siblings() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested/shared.txt"), "new").unwrap();
        fs::write(src.join("top.txt"), "top").unwrap();

        fs::create_dir_all(dst.join("nested")).unwrap();
        fs::write(dst.join("nested/shared.txt"), "old").unwrap();
        fs::write(dst.join("keep.txt"), "keep").unwrap();

        copy_tree_merge(&src, &dst).unwrap();

        assert_eq!(
            fs::read_to_string(dst.join("nested/shared.txt")).unwrap(),
            "new"
        );
        assert_eq!(fs::read_to_string(dst.join("top.txt")).unwrap(), "top");
        assert_eq!(fs::read_to_string(dst.join("keep.txt")).unwrap(), "keep");
        // src is untouched — this is a copy, not a move.
        assert!(src.join("top.txt").is_file());
    }

    #[test]
    fn merge_tree_copies_without_deleting_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir_all(src.join("a")).unwrap();
        fs::write(src.join("a/file.txt"), "from src").unwrap();

        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("existing.txt"), "untouched").unwrap();

        merge_tree(&src, &dst).unwrap();

        assert_eq!(
            fs::read_to_string(dst.join("a/file.txt")).unwrap(),
            "from src"
        );
        assert_eq!(
            fs::read_to_string(dst.join("existing.txt")).unwrap(),
            "untouched"
        );
    }
}
