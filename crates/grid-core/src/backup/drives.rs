//! Removable-drive detection. Each OS has its own probe:
//! `/proc/self/mountinfo` on Linux (`drives/linux.rs`), the Win32 volume
//! APIs on Windows (`drives/windows.rs`) and `/Volumes` on macOS
//! (`drives/macos.rs`, best effort). The app polls [`scan`] and diffs the
//! keys; this module only answers what is mounted now.
//!
//! With the `e2e` feature, [`DRIVE_ROOTS_ENV`] replaces the probe: every
//! child folder of its roots is a drive, so a test plugs one in with `mkdir`
//! and unplugs it with `rename` or `rm -r`.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub use crate::platform::disk_space;

/// With the `e2e` feature only: a path list (the OS's `PATH` separator)
/// whose child folders are the mounted drives, replacing the OS probe.
#[cfg(feature = "e2e")]
pub const DRIVE_ROOTS_ENV: &str = "GRID_LAUNCHER_E2E_DRIVE_ROOTS";
/// With the `e2e` feature only: an optional `<drive>/.e2e-drive.json` with
/// `{label, fs, total_bytes, free_bytes, read_only}` (every field optional)
/// that fakes what the OS would report for that drive.
#[cfg(feature = "e2e")]
pub const FAKE_DRIVE_FILE: &str = ".e2e-drive.json";

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

impl DetectedDrive {
    /// This drive's key: see [`drive_key`].
    pub fn key(&self) -> DriveKey {
        drive_key(&self.mount)
    }

    /// A drive from its probed parts, with the display name filled in.
    fn new(mount: PathBuf, label: String, fs: String, read_only: bool) -> Self {
        let (total_bytes, free_bytes) = disk_space(&mount).unwrap_or((0, 0));
        let (fs_display, fat32) = fs_display(&fs);
        Self {
            mount,
            label,
            fs,
            fs_display,
            fat32,
            total_bytes,
            free_bytes,
            read_only,
        }
    }
}

/// What changed between two scans.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DriveDiff {
    pub appeared: Vec<DriveKey>,
    pub removed: Vec<DriveKey>,
}

/// The key of the drive mounted at `mount`: the drive letter (`E:`) for a
/// Windows drive root (`E:\`), the mount path itself everywhere else.
pub fn drive_key(mount: &Path) -> DriveKey {
    let text = mount.to_string_lossy();
    match text.as_bytes() {
        [letter, b':', b'\\' | b'/'] | [letter, b':'] if letter.is_ascii_alphabetic() => {
            format!("{}:", letter.to_ascii_uppercase() as char)
        }
        _ => text.into_owned(),
    }
}

/// The folders whose mounts count as removable drives, per OS: `/run/media`
/// and `/media` on Linux, `/Volumes` on macOS. Windows enumerates drive
/// letters instead, so it has none.
pub fn default_roots() -> Vec<PathBuf> {
    if cfg!(target_os = "linux") {
        vec![PathBuf::from("/run/media"), PathBuf::from("/media")]
    } else if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Volumes")]
    } else {
        Vec::new()
    }
}

/// Every mounted removable drive, sorted by mount path. Honours the e2e
/// drive-roots override.
pub fn scan() -> Vec<DetectedDrive> {
    #[cfg(feature = "e2e")]
    if let Some(roots) = e2e_roots() {
        return sorted(scan_fake(&roots));
    }
    sorted(scan_host())
}

fn sorted(mut drives: Vec<DetectedDrive>) -> Vec<DetectedDrive> {
    drives.sort_by(|a, b| a.mount.cmp(&b.mount));
    drives.dedup_by(|a, b| a.mount == b.mount);
    drives
}

#[cfg(target_os = "linux")]
fn scan_host() -> Vec<DetectedDrive> {
    linux::scan(&default_roots())
}

#[cfg(windows)]
fn scan_host() -> Vec<DetectedDrive> {
    windows::scan()
}

#[cfg(target_os = "macos")]
fn scan_host() -> Vec<DetectedDrive> {
    macos::scan(&default_roots())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn scan_host() -> Vec<DetectedDrive> {
    Vec::new()
}

/// The override roots, when [`DRIVE_ROOTS_ENV`] is set and not blank.
#[cfg(feature = "e2e")]
fn e2e_roots() -> Option<Vec<PathBuf>> {
    let raw = std::env::var_os(DRIVE_ROOTS_ENV)?;
    let roots: Vec<PathBuf> = std::env::split_paths(&raw)
        .filter(|root| !root.as_os_str().is_empty())
        .collect();
    (!roots.is_empty()).then_some(roots)
}

/// What a [`FAKE_DRIVE_FILE`] may fake.
#[cfg(feature = "e2e")]
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct FakeDrive {
    label: Option<String>,
    fs: Option<String>,
    total_bytes: Option<u64>,
    free_bytes: Option<u64>,
    read_only: bool,
}

/// Every child folder of every root (dot-folders excepted) as a drive.
/// Without a faked size the drive reports its real volume; with a faked
/// `total_bytes` the free space is that total less what the folder holds.
#[cfg(feature = "e2e")]
fn scan_fake(roots: &[PathBuf]) -> Vec<DetectedDrive> {
    let mut drives = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let mount = entry.path();
            let fake: FakeDrive = std::fs::read(mount.join(FAKE_DRIVE_FILE))
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            let fs = fake.fs.unwrap_or_else(|| "exfat".to_string());
            let mut drive =
                DetectedDrive::new(mount, fake.label.unwrap_or(name), fs, fake.read_only);
            if let Some(total) = fake.total_bytes {
                let used = folder_bytes(&drive.mount);
                drive.free_bytes = total.saturating_sub(used).min(drive.free_bytes);
                drive.total_bytes = total;
            }
            if let Some(free) = fake.free_bytes {
                drive.free_bytes = free;
            }
            drives.push(drive);
        }
    }
    drives
}

/// The bytes of every file under `dir`, links not followed.
#[cfg(feature = "e2e")]
fn folder_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => folder_bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |meta| meta.len()),
            _ => 0,
        })
        .sum()
}

/// The display name of a raw file system name, and whether it is FAT, whose
/// files stop at 4 GiB. Linux's `vfat` cannot tell FAT16 from FAT32, so it
/// shows as FAT32. An unknown name shows as it is.
pub fn fs_display(raw: &str) -> (String, bool) {
    let trimmed = raw.trim();
    let (name, fat) = match trimmed.to_ascii_lowercase().as_str() {
        "vfat" | "fat32" | "msdos" => ("FAT32", true),
        "fat" | "fat12" | "fat16" => ("FAT", true),
        "exfat" => ("exFAT", false),
        "ntfs" | "ntfs3" | "fuseblk" => ("NTFS", false),
        "ext2" => ("ext2", false),
        "ext3" => ("ext3", false),
        "ext4" => ("ext4", false),
        "btrfs" => ("Btrfs", false),
        "xfs" => ("XFS", false),
        "f2fs" => ("F2FS", false),
        "hfs" | "hfsplus" => ("HFS+", false),
        "apfs" => ("APFS", false),
        "iso9660" | "cdfs" => ("ISO 9660", false),
        "udf" => ("UDF", false),
        "refs" => ("ReFS", false),
        _ => return (trimmed.to_string(), false),
    };
    (name.to_string(), fat)
}

/// The drives that appeared and the drives that were removed between two
/// scans, each in the order its scan listed them.
pub fn diff(prev: &[DriveKey], next: &[DriveKey]) -> DriveDiff {
    DriveDiff {
        appeared: next
            .iter()
            .filter(|key| !prev.contains(key))
            .cloned()
            .collect(),
        removed: prev
            .iter()
            .filter(|key| !next.contains(key))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_display_table() {
        for (raw, name, fat) in [
            ("vfat", "FAT32", true),
            ("FAT32", "FAT32", true),
            ("msdos", "FAT32", true),
            ("FAT", "FAT", true),
            ("exfat", "exFAT", false),
            ("exFAT", "exFAT", false),
            ("ntfs3", "NTFS", false),
            ("ntfs", "NTFS", false),
            ("fuseblk", "NTFS", false),
            ("NTFS", "NTFS", false),
            ("ext4", "ext4", false),
            ("btrfs", "Btrfs", false),
            ("hfsplus", "HFS+", false),
            ("apfs", "APFS", false),
            ("zfs", "zfs", false),
        ] {
            assert_eq!(fs_display(raw), (name.to_string(), fat), "{raw}");
        }
    }

    #[test]
    fn diff_lists_appeared_and_removed() {
        let keys = |list: &[&str]| list.iter().map(|key| key.to_string()).collect::<Vec<_>>();
        let prev = keys(&["/run/media/u/A", "/run/media/u/B"]);
        let next = keys(&["/run/media/u/B", "/run/media/u/C", "/run/media/u/D"]);
        assert_eq!(
            diff(&prev, &next),
            DriveDiff {
                appeared: keys(&["/run/media/u/C", "/run/media/u/D"]),
                removed: keys(&["/run/media/u/A"]),
            }
        );
        assert_eq!(diff(&prev, &prev), DriveDiff::default());
        assert_eq!(diff(&[], &[]), DriveDiff::default());
    }

    #[test]
    fn drive_keys() {
        assert_eq!(
            drive_key(Path::new("/run/media/samm/ClasPlatf")),
            "/run/media/samm/ClasPlatf"
        );
        assert_eq!(drive_key(Path::new("E:\\")), "E:");
        assert_eq!(drive_key(Path::new("e:")), "E:");
        assert_eq!(drive_key(Path::new("/Volumes/STICK")), "/Volumes/STICK");
    }

    #[test]
    fn default_roots_per_os() {
        let roots = default_roots();
        if cfg!(target_os = "linux") {
            assert_eq!(
                roots,
                vec![PathBuf::from("/run/media"), PathBuf::from("/media")]
            );
        } else if cfg!(windows) {
            assert!(roots.is_empty());
        }
    }

    /// The windows-native gate runs this against the real volume APIs.
    #[cfg(windows)]
    #[test]
    fn windows_scan_does_not_panic() {
        for drive in scan_host() {
            assert_eq!(drive.key().len(), 2, "{:?}", drive.mount);
        }
    }
}
