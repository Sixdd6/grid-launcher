//! Host-platform lookups that need OS APIs.

use std::path::{Path, PathBuf};

/// The user's home directory, or `None` when it cannot be determined.
///
/// `directories::UserDirs` first (`$HOME` on unix, the known-folder API on
/// Windows), then a direct `$HOME` read. A release build on Windows never
/// reads `HOME` before the known folder, like Python's `expanduser` since
/// 3.8. Under `cfg(test)` a non-blank `HOME` wins on every OS, so a test
/// that points `HOME` at a temp dir can never write into the real profile.
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Some(home) = env_home() {
            return Some(home);
        }
    }
    if let Some(user_dirs) = directories::UserDirs::new() {
        return Some(user_dirs.home_dir().to_path_buf());
    }
    env_home()
}

/// The library folder first-run setup suggests: `Games/GRID` in the home
/// folder (`~/Games/GRID`, `%USERPROFILE%\Games\GRID`). Nothing is created
/// here; the first install creates it.
pub fn suggested_library_path(home: &Path) -> PathBuf {
    home.join("Games").join("GRID")
}

/// `$HOME` when it is set and non-blank once trimmed.
fn env_home() -> Option<PathBuf> {
    let raw = std::env::var("HOME").ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// `std::fs::canonicalize` without the Windows verbatim `\\?\` prefix
/// whenever the plain form names the same file (`dunce`). Off Windows it
/// is `std::fs::canonicalize` itself.
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    dunce::canonicalize(path)
}

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
///
/// Under `cfg(test)` a non-blank `HOME` gives `HOME\Documents`, the same
/// test seam as [`home_dir`].
#[cfg(windows)]
pub fn windows_documents_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Some(home) = env_home() {
            return Some(home.join("Documents"));
        }
    }
    directories::UserDirs::new()?
        .document_dir()
        .map(std::path::Path::to_path_buf)
}

#[cfg(not(windows))]
pub fn windows_documents_dir() -> Option<PathBuf> {
    None
}

/// The total bytes and the bytes free to this user on the volume that holds
/// `path`, which must exist. `None` when they cannot be read.
#[cfg(windows)]
pub fn disk_space(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available: u64 = 0;
    let mut total: u64 = 0;
    // SAFETY: `wide` is NUL-terminated and outlives the call; the two out
    // pointers are valid and the third optional one may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            &mut total,
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some((total, available))
}

/// The total bytes this user can use and the bytes free to this user on the
/// volume that holds `path`, which must exist. Blocks kept for root are left
/// out of both. `None` when they cannot be read.
#[cfg(unix)]
// The statvfs field types differ between targets (u32 on some 32-bit
// ones), so the casts are needed there even where they are no-ops here.
#[allow(clippy::unnecessary_cast)]
pub fn disk_space(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: an all-zero `statvfs` is a valid value to be overwritten.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is NUL-terminated and `stat` is a valid out-pointer.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    (rc == 0).then(|| {
        let block = stat.f_frsize as u64;
        let blocks = stat.f_blocks as u64;
        let free = stat.f_bfree as u64;
        let avail = stat.f_bavail as u64;
        (
            usable_blocks(blocks, free, avail).saturating_mul(block),
            avail.saturating_mul(block),
        )
    })
}

/// The blocks this user can ever use: all of them less the ones the file
/// system keeps for root (ext4 keeps 5% by default). Those are never free
/// to GRID, so counting them would show them as other files in a usage bar.
#[cfg(unix)]
fn usable_blocks(blocks: u64, free: u64, avail: u64) -> u64 {
    blocks.saturating_sub(free.saturating_sub(avail))
}

#[cfg(not(any(unix, windows)))]
pub fn disk_space(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    /// Off Windows there is no Known Folder API and no redirection to
    /// correct for, so callers get the `None` that keeps
    /// `resolve_native_save_dir` on its plain-expansion path.
    #[cfg(not(windows))]
    #[test]
    fn windows_documents_dir_is_none_off_windows() {
        assert_eq!(windows_documents_dir(), None);
    }

    /// Under test a non-blank `HOME` wins on every OS, so no test can
    /// reach the real user profile through the Windows known-folder API.
    #[test]
    fn home_dir_honors_a_home_override_under_test() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set(&[("HOME", Some(temp.path().to_str().unwrap()))]);

        assert_eq!(home_dir(), Some(temp.path().to_path_buf()));
    }

    /// A blank `HOME` is no override: the OS lookup answers instead.
    /// Windows only: on unix the OS lookup (`directories`) is `$HOME` itself.
    #[cfg(windows)]
    #[test]
    fn home_dir_ignores_a_blank_home_override() {
        let _lock = crate::test_env::lock();
        let _guard = EnvGuard::set(&[("HOME", Some("   "))]);

        let home = home_dir().expect("the OS reports a home directory");
        assert!(!home.as_os_str().is_empty());
        assert_ne!(home, PathBuf::from("   "));
    }

    /// Under test the Documents folder follows the `HOME` override, so
    /// PCSX2 and DuckStation candidates stay inside the test's temp dir.
    #[cfg(windows)]
    #[test]
    fn windows_documents_dir_follows_the_home_override_under_test() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set(&[("HOME", Some(temp.path().to_str().unwrap()))]);

        assert_eq!(windows_documents_dir(), Some(temp.path().join("Documents")));
    }

    /// `std::fs::canonicalize` returns a verbatim `\\?\C:\...` path on
    /// Windows, which many emulators reject on their command line.
    #[test]
    fn canonicalize_never_returns_a_verbatim_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("real");
        std::fs::create_dir_all(&dir).unwrap();

        let canonical = canonicalize(&dir).unwrap();

        assert!(canonical.is_absolute(), "{canonical:?}");
        assert!(
            !canonical.to_string_lossy().starts_with(r"\\?\"),
            "{canonical:?}"
        );
        assert!(canonical.ends_with("real"), "{canonical:?}");
    }

    #[cfg(not(windows))]
    #[test]
    fn canonicalize_equals_the_std_canonicalize_off_windows() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("real");
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            canonicalize(&dir).unwrap(),
            std::fs::canonicalize(&dir).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn usable_blocks_leaves_out_the_root_reserve() {
        // 1000 blocks, 300 free of which 250 are free to this user: 50 are
        // kept for root.
        assert_eq!(usable_blocks(1000, 300, 250), 950);
        assert_eq!(usable_blocks(1000, 300, 300), 1000);
        assert_eq!(usable_blocks(10, 300, 0), 0);
    }

    #[test]
    fn disk_space_reads_the_volume_of_an_existing_folder() {
        let temp = tempfile::tempdir().unwrap();
        let (total, free) = disk_space(temp.path()).expect("space is readable");
        assert!(total > 0);
        assert!(free <= total);
        assert_eq!(disk_space(&temp.path().join("missing")), None);
    }

    #[test]
    fn the_suggested_library_is_games_grid_in_home() {
        assert_eq!(
            suggested_library_path(Path::new("/home/me")),
            Path::new("/home/me").join("Games").join("GRID")
        );
    }
}
