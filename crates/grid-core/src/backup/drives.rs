//! Removable-drive detection. The per-OS probes (`/proc/self/mountinfo` on
//! Linux, the Win32 volume APIs on Windows, `/Volumes` on macOS) land in
//! Task 2; until then every probe reports nothing.

use std::path::{Path, PathBuf};

/// A drive key: the mount path, or the drive letter on Windows. The OS keeps
/// it unique, so it identifies a drive (a pack has no id).
pub type DriveKey = String;

/// A mounted removable drive found by a scan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DetectedDrive {
    pub mount: PathBuf,
    pub label: String,
    /// The raw file system name: `vfat`, `exfat`, `ntfs3`, `fuseblk`, `FAT32`, `NTFS`…
    pub fs: String,
    /// The display name: `FAT32`, `exFAT`, `NTFS`, `ext4`…
    pub fs_display: String,
    pub fat32: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub read_only: bool,
}

/// What changed between two scans.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DriveDiff {
    pub appeared: Vec<DriveKey>,
    pub removed: Vec<DriveKey>,
}

/// The folders whose children are mounted removable drives, per OS.
pub fn default_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// Every mounted removable drive. Honours the e2e drive-roots override.
pub fn scan() -> Vec<DetectedDrive> {
    Vec::new()
}

/// The display name of a raw file system name, and whether it is FAT32.
pub fn fs_display(_raw: &str) -> (String, bool) {
    (String::new(), false)
}

/// The drives that appeared and the drives that were removed between two scans.
pub fn diff(_prev: &[DriveKey], _next: &[DriveKey]) -> DriveDiff {
    DriveDiff::default()
}

/// The total and free bytes of the volume that holds `path`.
pub fn disk_space(_path: &Path) -> Option<(u64, u64)> {
    None
}
