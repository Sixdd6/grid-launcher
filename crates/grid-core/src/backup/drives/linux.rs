//! Linux: removable drives are the block-device mounts under `/run/media`
//! (udisks) and `/media`, read from `/proc/self/mountinfo`.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use super::DetectedDrive;

const MOUNTINFO: &str = "/proc/self/mountinfo";
const BY_LABEL: &str = "/dev/disk/by-label";

/// One line of `mountinfo`, the fields a scan needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct MountEntry {
    pub mount_point: PathBuf,
    pub fstype: String,
    pub source: String,
    /// `ro` in the per-mount or the super-block options.
    pub read_only: bool,
}

/// Every well-formed line of a `mountinfo` text. Pure.
///
/// A line is `id parent major:minor root mount_point mount_options
/// [optional fields…] - fstype source super_options`; paths use octal
/// escapes (`\040` for a space).
pub(super) fn parse_mountinfo(text: &str) -> Vec<MountEntry> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<MountEntry> {
    let (left, right) = line.split_once(" - ")?;
    let mut left = left.split(' ');
    let mount_point = left.nth(4)?;
    let mount_options = left.next()?;
    let mut right = right.split(' ');
    let fstype = right.next()?;
    let source = right.next()?;
    let super_options = right.next().unwrap_or("");
    let has_ro = |options: &str| options.split(',').any(|option| option == "ro");
    Some(MountEntry {
        mount_point: PathBuf::from(OsString::from_vec(unescape_octal(mount_point))),
        fstype: String::from_utf8_lossy(&unescape_octal(fstype)).into_owned(),
        source: String::from_utf8_lossy(&unescape_octal(source)).into_owned(),
        read_only: has_ro(mount_options) || has_ro(super_options),
    })
}

/// Undoes mountinfo's `\NNN` octal escapes.
fn unescape_octal(field: &str) -> Vec<u8> {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let digits = bytes.get(index + 1..index + 4);
        if bytes[index] == b'\\' {
            if let Some(digits) = digits.filter(|d| d.iter().all(|b| (b'0'..=b'7').contains(b))) {
                let value = digits
                    .iter()
                    .fold(0u32, |acc, digit| acc * 8 + u32::from(digit - b'0'));
                if let Ok(byte) = u8::try_from(value) {
                    out.push(byte);
                    index += 4;
                    continue;
                }
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

/// Undoes udev's `\xHH` escapes in a `/dev/disk/by-label` name.
fn unescape_udev(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && bytes.get(index + 1) == Some(&b'x') {
            let hex = bytes
                .get(index + 2..index + 4)
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok());
            if let Some(byte) = hex {
                out.push(byte);
                index += 4;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The entries that are removable drives: a block device (`/dev/…`, loop
/// devices excepted, which are mounted image files) mounted strictly below
/// one of `roots`. Pure. A tmpfs or a FUSE network mount has no `/dev`
/// source and is left out.
pub(super) fn removable_mounts(entries: Vec<MountEntry>, roots: &[PathBuf]) -> Vec<MountEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            entry.source.starts_with("/dev/")
                && !entry.source.starts_with("/dev/loop")
                && roots
                    .iter()
                    .any(|root| entry.mount_point.starts_with(root) && entry.mount_point != *root)
        })
        .collect()
}

/// The label udev lists for `source` under `/dev/disk/by-label`.
fn by_label(source: &str) -> Option<String> {
    let device = std::fs::canonicalize(source).ok()?;
    std::fs::read_dir(BY_LABEL)
        .ok()?
        .flatten()
        .find(|entry| std::fs::canonicalize(entry.path()).is_ok_and(|target| target == device))
        .map(|entry| unescape_udev(&entry.file_name().to_string_lossy()))
}

/// A drive's label: udev's label for its device, else the mount folder's
/// name (udisks names the folder after the label).
fn label_for(entry: &MountEntry) -> String {
    by_label(&entry.source)
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| folder_name(&entry.mount_point))
}

fn folder_name(mount: &Path) -> String {
    mount
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Every removable drive mounted below `roots`.
pub(super) fn scan(roots: &[PathBuf]) -> Vec<DetectedDrive> {
    let Ok(text) = std::fs::read_to_string(MOUNTINFO) else {
        return Vec::new();
    };
    removable_mounts(parse_mountinfo(&text), roots)
        .into_iter()
        .map(|entry| {
            let label = label_for(&entry);
            DetectedDrive::new(entry.mount_point, label, entry.fstype, entry.read_only)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from a Fedora desktop with the ClasPlatf stick in, plus
    /// hand-made lines for the other cases.
    const SAMPLE: &str = "\
22 1 0:21 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p3 rw,seclabel,compress=zstd:1,ssd,space_cache=v2,subvolid=257,subvol=/root
25 22 0:23 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw
53 22 0:26 / /run rw,nosuid,nodev shared:23 - tmpfs tmpfs rw,seclabel,size=6538816k,nr_inodes=819200,mode=755,inode64
725 53 8:17 / /run/media/samm/ClasPlatf rw,nosuid,nodev,relatime shared:894 - exfat /dev/sdb1 rw,uid=1000,gid=1000,fmask=0022,dmask=0022,iocharset=utf8,errors=remount-ro,sys_tz
726 53 8:33 / /run/media/samm/Couch\\040Kit rw,nosuid,nodev,relatime shared:895 - ntfs3 /dev/sdc1 rw,uid=1000,gid=1000,iocharset=utf8
727 53 8:49 / /run/media/samm/OLDSTICK ro,nosuid,nodev,relatime shared:896 - vfat /dev/sdd1 ro,uid=1000,gid=1000,fmask=0022,dmask=0022,codepage=437
728 22 0:60 / /media/scratch rw,nosuid,nodev,relatime shared:897 - tmpfs tmpfs rw,size=1024k
729 53 0:61 / /run/media/samm/nas rw,nosuid,nodev,relatime shared:898 - fuse.sshfs samm@nas:/srv rw,user_id=1000,group_id=1000
730 22 8:65 / /media/usb rw,relatime shared:899 master:5 - vfat /dev/sde1 ro,fmask=0022
731 53 7:0 / /run/media/samm/Ubuntu\\04024.04 ro,nosuid,nodev,relatime shared:900 - iso9660 /dev/loop0 ro,nojoliet
732 22 0:62 / /run/media rw,relatime shared:901 - ext4 /dev/sdf1 rw
";

    #[test]
    fn parses_every_line() {
        let entries = parse_mountinfo(SAMPLE);
        assert_eq!(entries.len(), 11);
        assert_eq!(
            entries[3],
            MountEntry {
                mount_point: PathBuf::from("/run/media/samm/ClasPlatf"),
                fstype: "exfat".to_string(),
                source: "/dev/sdb1".to_string(),
                read_only: false,
            }
        );
    }

    #[test]
    fn keeps_only_block_devices_below_the_roots() {
        let roots = vec![PathBuf::from("/run/media"), PathBuf::from("/media")];
        let kept = removable_mounts(parse_mountinfo(SAMPLE), &roots);
        let mounts: Vec<&Path> = kept
            .iter()
            .map(|entry| entry.mount_point.as_path())
            .collect();
        assert_eq!(
            mounts,
            vec![
                Path::new("/run/media/samm/ClasPlatf"),
                Path::new("/run/media/samm/Couch Kit"),
                Path::new("/run/media/samm/OLDSTICK"),
                Path::new("/media/usb"),
            ]
        );
        assert_eq!(kept[1].fstype, "ntfs3");
        assert!(!kept[1].read_only);
        assert!(kept[2].read_only, "an ro vfat mount is read-only");
        assert!(kept[3].read_only, "ro in the super options counts too");
    }

    #[test]
    fn the_real_mountinfo_parses() {
        let text = std::fs::read_to_string(MOUNTINFO).unwrap();
        let entries = parse_mountinfo(&text);
        assert!(entries
            .iter()
            .any(|entry| entry.mount_point == Path::new("/")));
    }

    #[test]
    fn octal_and_udev_escapes() {
        assert_eq!(unescape_octal("Couch\\040Kit"), b"Couch Kit");
        assert_eq!(unescape_octal("a\\011b\\134c"), b"a\tb\\c");
        assert_eq!(unescape_octal("trailing\\04"), b"trailing\\04");
        assert_eq!(unescape_octal("bad\\089"), b"bad\\089");
        assert_eq!(unescape_udev("Couch\\x20Kit"), "Couch Kit");
        assert_eq!(unescape_udev("a\\x2fb"), "a/b");
        assert_eq!(unescape_udev("odd\\xZZ"), "odd\\xZZ");
    }

    #[test]
    fn a_short_or_malformed_line_is_skipped() {
        assert!(parse_mountinfo("garbage\n1 2 3\n").is_empty());
    }
}
