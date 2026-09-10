//! Host-platform lookups that need OS APIs.

use std::path::PathBuf;

/// The Shell-resolved Windows Documents folder, or `None` off Windows.
///
/// Ports `pcsx2_windows_documents_folder` (`pcsx2.py:10-49`), whose
/// `SHGetKnownFolderPath(FOLDERID_Documents)` call honours User Shell
/// Folders redirection (a Documents folder moved to another drive or into
/// OneDrive). `directories` resolves the same known folder through the same
/// Shell call (`directories 6.0` -> `dirs-sys 0.5`
/// `known_folder_documents` -> `SHGetKnownFolderPath`), so `%USERPROFILE%`
/// string expansion is never used for it.
///
/// The Python reference returns `None` when `sys.platform != "win32"`; here
/// that platform gate is the `#[cfg(not(windows))]` arm, and callers treat
/// `None` as "no redirection to correct for".
#[cfg(windows)]
pub fn windows_documents_dir() -> Option<PathBuf> {
    directories::UserDirs::new()?
        .document_dir()
        .map(std::path::Path::to_path_buf)
}

#[cfg(not(windows))]
pub fn windows_documents_dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    use super::*;

    /// Off Windows there is no Known Folder API and no redirection to
    /// correct for, so callers get the `None` that keeps
    /// `resolve_native_save_dir` on its plain-expansion path.
    #[cfg(not(windows))]
    #[test]
    fn windows_documents_dir_is_none_off_windows() {
        assert_eq!(windows_documents_dir(), None);
    }
}
