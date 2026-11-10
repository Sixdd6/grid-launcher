//! macOS (best effort, not a release target): every volume under
//! `/Volumes` except the one that is the startup disk.

use std::path::{Path, PathBuf};

use super::DetectedDrive;

/// The file system name and read-only flag of the volume mounted at `path`.
fn statfs(path: &Path) -> Option<(String, bool)> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is NUL-terminated and `stat` is a valid out-pointer.
    if unsafe { libc::statfs(c_path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    // SAFETY: the kernel NUL-terminates `f_fstypename` within its length.
    let name = unsafe { std::ffi::CStr::from_ptr(stat.f_fstypename.as_ptr()) };
    let read_only = u64::from(stat.f_flags) & (libc::MNT_RDONLY as u64) != 0;
    Some((name.to_string_lossy().into_owned(), read_only))
}

pub(super) fn scan(roots: &[PathBuf]) -> Vec<DetectedDrive> {
    let mut drives = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let mount = entry.path();
            if std::fs::canonicalize(&mount).is_ok_and(|real| real == Path::new("/")) {
                continue;
            }
            let Some((fs, read_only)) = statfs(&mount) else {
                continue;
            };
            let label = entry.file_name().to_string_lossy().into_owned();
            drives.push(DetectedDrive::new(mount, label, fs, read_only));
        }
    }
    drives
}
