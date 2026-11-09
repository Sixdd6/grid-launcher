//! The drive write engine. Every file goes to `<name>.gridtmp` first, is
//! synced and checked, and is renamed into place only when complete, so a
//! drive pulled mid-write holds at most a `.gridtmp` leftover, which
//! [`pack::cleanup_leftovers`] removes on the drive's next appearance.
//!
//! Every path is pack-relative and goes through [`pack::resolve`]. GRID never
//! writes or deletes outside `<mount>/gridpack/`. The one delete of the whole
//! folder is [`remove_all_grid_files`], behind [`guard_grid_root`].
//!
//! A drive has no id: it is identified by its mount path. An I/O error on a
//! drive counts as "drive removed" when the mount folder or the drive's
//! `gridpack/gridpack.json` can no longer be read ([`classify_io`]).

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::PoisonError;

use md5::{Digest, Md5};

use super::drives::disk_space;
use super::pack::{
    self, PackError, PackLock, FAT32_MAX_FILE, MAX_COMPONENT_BYTES, PACK_DIR, PACK_FILE, TMP_SUFFIX,
};

/// Files are copied in chunks of this size, with a cancel check and a
/// progress report after each one.
pub const CHUNK_BYTES: usize = 1024 * 1024;
/// The free space a write leaves on the drive, on top of the file itself.
pub const SPACE_MARGIN: u64 = 64 * 1024 * 1024;

/// The drive a write goes to.
#[derive(Clone, Copy, Debug)]
pub struct WriteTarget<'a> {
    /// The drive's mount path (its drive letter's root on Windows).
    pub mount: &'a Path,
    /// The drive's label, for messages.
    pub label: &'a str,
    /// The drive is FAT32 (or FAT): no file may exceed [`FAT32_MAX_FILE`].
    pub fat32: bool,
}

/// One file to write.
#[derive(Clone, Copy, Debug)]
pub struct FileWrite<'a> {
    /// The complete local file (in staging) to copy.
    pub src: &'a Path,
    /// Where it goes, relative to `gridpack/`.
    pub rel: &'a str,
    /// The expected MD5 as hex, when known. Blank means unknown.
    pub md5: Option<&'a str>,
}

/// Why a drive write or delete failed.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// The drive is gone: its mount folder or its `gridpack.json` can no
    /// longer be read. The caller names the drive ("<label> was removed").
    #[error("the drive was removed")]
    DriveRemoved,
    /// The drive is full, or would be within [`SPACE_MARGIN`].
    #[error("there isn't enough free space on the drive")]
    NoSpace,
    /// A file over FAT32's limit, refused before anything is written.
    #[error("{name} is over 4 GB, which {label} (FAT32) can't hold")]
    TooBigForFat32 { name: String, label: String },
    /// The written file's size or MD5 differs from the source.
    #[error("{0}")]
    Mismatch(String),
    /// The cancel flag was set. The temporary file was removed.
    #[error("cancelled")]
    Cancelled,
    /// An unsafe pack-relative path, or a `gridpack` folder that is a link.
    #[error(transparent)]
    Pack(#[from] PackError),
    /// A guarded delete refused its target.
    #[error("{0}")]
    Refused(String),
    #[error("file error: {0}")]
    Io(io::Error),
}

/// Copies `file.src` onto the drive at `file.rel`:
/// 1. validates `rel` (it must resolve under `<mount>/gridpack/`);
/// 2. refuses a file over 4 GB on FAT32;
/// 3. checks the drive keeps [`SPACE_MARGIN`] free after the write;
/// 4. copies in [`CHUNK_BYTES`] chunks to `<rel>.gridtmp`, reporting
///    `progress(written, total)` and stopping on `cancel`;
/// 5. syncs it;
/// 6. checks its size against the source's, and its MD5 when known;
/// 7. renames it onto `rel`, replacing an existing file.
///
/// On a failure after step 4 began, the temporary file is removed when the
/// drive is still there. Returns the bytes written.
pub fn write_file(
    target: &WriteTarget<'_>,
    file: &FileWrite<'_>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<u64, WriteError> {
    let mount = target.mount;
    let dest = pack::resolve(mount, file.rel)?;
    let name = file_name(&dest);
    let mut src = File::open(file.src).map_err(WriteError::Io)?;
    let size = src.metadata().map_err(WriteError::Io)?.len();
    check_fat32(size, target.fat32, &name, target.label)?;
    match disk_space(mount) {
        Some((_, free)) => check_space(free, size)?,
        None if drive_gone(mount) => return Err(WriteError::DriveRemoved),
        // The OS can't say; the write itself reports a full drive.
        None => {}
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(WriteError::Cancelled);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| classify_io(error, mount))?;
    }
    let tmp = tmp_path(&dest);
    let copied = copy_checked(
        &mut src, &tmp, size, file.md5, &name, mount, cancel, progress,
    )
    .and_then(|written| {
        std::fs::rename(&tmp, &dest)
            .map(|()| written)
            .map_err(|error| classify_io(error, mount))
    });
    match copied {
        Ok(written) => {
            sync_folder(dest.parent());
            Ok(written)
        }
        Err(error) => {
            if !matches!(error, WriteError::DriveRemoved) {
                let _ = std::fs::remove_file(&tmp);
            }
            Err(error)
        }
    }
}

/// Steps 4 to 6 of [`write_file`]: copy to `tmp`, sync, verify.
#[allow(clippy::too_many_arguments)]
fn copy_checked(
    src: &mut File,
    tmp: &Path,
    size: u64,
    md5: Option<&str>,
    name: &str,
    mount: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<u64, WriteError> {
    let on_drive = |error: io::Error| classify_io(error, mount);
    let mut out = File::create(tmp).map_err(on_drive)?;
    let mut hasher = Md5::new();
    let mut buf = vec![0u8; CHUNK_BYTES];
    let mut written: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(WriteError::Cancelled);
        }
        // A read error is the local staging copy's, never the drive's.
        let n = match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(WriteError::Io(error)),
        };
        written += n as u64;
        // A source that grows while it is copied would never reach its end.
        if written > size {
            return Err(WriteError::Mismatch(format!(
                "{name} didn't copy correctly: its size doesn't match"
            )));
        }
        out.write_all(&buf[..n]).map_err(on_drive)?;
        hasher.update(&buf[..n]);
        progress(written, size);
    }
    out.sync_all().map_err(on_drive)?;
    let on_disk = out.metadata().map_err(on_drive)?.len();
    drop(out);
    if written != size || on_disk != size {
        return Err(WriteError::Mismatch(format!(
            "{name} didn't copy correctly: its size doesn't match"
        )));
    }
    let expected = md5.unwrap_or("").trim().to_ascii_lowercase();
    if !expected.is_empty() {
        let actual: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if actual != expected {
            return Err(WriteError::Mismatch(format!(
                "{name} didn't copy correctly: its MD5 doesn't match"
            )));
        }
    }
    Ok(written)
}

/// Refuses a file over [`FAT32_MAX_FILE`] on a FAT32 drive.
fn check_fat32(size: u64, fat32: bool, name: &str, label: &str) -> Result<(), WriteError> {
    if fat32 && size > FAT32_MAX_FILE {
        return Err(WriteError::TooBigForFat32 {
            name: name.to_string(),
            label: label.to_string(),
        });
    }
    Ok(())
}

/// Refuses a write that would leave less than [`SPACE_MARGIN`] free.
fn check_space(free: u64, size: u64) -> Result<(), WriteError> {
    if free < size.saturating_add(SPACE_MARGIN) {
        return Err(WriteError::NoSpace);
    }
    Ok(())
}

/// `<dest>.gridtmp`. When that name would exceed [`MAX_COMPONENT_BYTES`],
/// the name is cut first so the temporary name stays legal on FAT, exFAT
/// and NTFS. It always ends in [`TMP_SUFFIX`], so cleanup still finds it.
fn tmp_path(dest: &Path) -> PathBuf {
    let name = file_name(dest);
    let room = MAX_COMPONENT_BYTES - TMP_SUFFIX.len();
    let mut cut = name.len().min(room);
    while !name.is_char_boundary(cut) {
        cut -= 1;
    }
    dest.with_file_name(format!("{}{TMP_SUFFIX}", &name[..cut]))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Syncs a folder after a rename, on unix. Some file systems refuse; the
/// rename is done either way.
fn sync_folder(dir: Option<&Path>) {
    #[cfg(unix)]
    if let Some(folder) = dir.and_then(|dir| File::open(dir).ok()) {
        let _ = folder.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Sorts an I/O error from a drive write: [`WriteError::DriveRemoved`] when
/// the mount folder or `<mount>/gridpack/gridpack.json` no longer exists or
/// can't be read, [`WriteError::NoSpace`] for a full drive, else
/// [`WriteError::Io`]. It looks only at the file system, never at a scan.
pub fn classify_io(error: io::Error, mount: &Path) -> WriteError {
    if drive_gone(mount) {
        WriteError::DriveRemoved
    } else if is_no_space(&error) {
        WriteError::NoSpace
    } else {
        WriteError::Io(error)
    }
}

/// The mount folder is gone, or its index can't be opened for reading.
fn drive_gone(mount: &Path) -> bool {
    if !mount.is_dir() {
        return true;
    }
    let index = pack::index_path(mount);
    let is_file = std::fs::metadata(&index).is_ok_and(|meta| meta.is_file());
    !is_file || File::open(&index).is_err()
}

fn is_no_space(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::StorageFull {
        return true;
    }
    #[cfg(unix)]
    {
        matches!(error.raw_os_error(), Some(libc::ENOSPC | libc::EDQUOT))
    }
    #[cfg(windows)]
    {
        // ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL.
        matches!(error.raw_os_error(), Some(39 | 112))
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

/// The index is never removed through [`remove_rel`] or [`remove_dir_rel`].
/// Compared without case: exFAT, FAT and NTFS fold it.
fn refuse_index(rel: &str) -> Result<(), WriteError> {
    if rel.eq_ignore_ascii_case(PACK_FILE) {
        return Err(WriteError::Refused(format!(
            "{PACK_FILE} can't be removed this way"
        )));
    }
    Ok(())
}

/// Removes one file under `gridpack/`. A missing file is not an error; a
/// folder is refused (see [`remove_dir_rel`]). A link is removed itself,
/// never followed.
pub fn remove_rel(mount: &Path, rel: &str) -> Result<(), WriteError> {
    refuse_index(rel)?;
    let path = pack::resolve(mount, rel)?;
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(classify_io(error, mount)),
        Ok(meta) if meta.is_dir() => Err(WriteError::Refused(format!(
            "{} is a folder",
            file_name(&path)
        ))),
        Ok(_) => remove_entry(&path).map_err(|error| classify_io(error, mount)),
    }
}

/// Removes one folder under `gridpack/` with everything in it. A missing
/// folder is not an error; a file is refused (see [`remove_rel`]). A link is
/// removed itself, never followed.
pub fn remove_dir_rel(mount: &Path, rel: &str) -> Result<(), WriteError> {
    refuse_index(rel)?;
    let path = pack::resolve(mount, rel)?;
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(classify_io(error, mount)),
        Ok(meta) if meta.file_type().is_symlink() => {
            remove_entry(&path).map_err(|error| classify_io(error, mount))
        }
        Ok(meta) if meta.is_dir() => {
            std::fs::remove_dir_all(&path).map_err(|error| classify_io(error, mount))
        }
        Ok(_) => Err(WriteError::Refused(format!(
            "{} is not a folder",
            file_name(&path)
        ))),
    }
}

/// Removes a file or a link without following it. A folder link on Windows
/// needs `remove_dir`.
fn remove_entry(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path).or_else(|error| std::fs::remove_dir(path).map_err(|_| error))
}

/// The checks before "Remove GRID files…" deletes `path`. It passes only
/// when:
/// - `mount` is absolute, with no `.` or `..` part;
/// - `path` is exactly `<mount>/gridpack`;
/// - `path` is a real folder, not a link (or junction);
/// - it holds `gridpack.json` as a file.
pub fn guard_grid_root(mount: &Path, path: &Path) -> Result<(), WriteError> {
    let refuse = |why: &str| {
        Err(WriteError::Refused(format!(
            "GRID files not removed: {why}"
        )))
    };
    let plain_mount = mount.is_absolute()
        && mount
            .components()
            .all(|part| !matches!(part, Component::CurDir | Component::ParentDir));
    if !plain_mount {
        return refuse("the drive's path is not a plain absolute path");
    }
    if path != pack::pack_dir(mount) {
        return refuse(&format!(
            "only the {PACK_DIR} folder at the drive's root can be removed"
        ));
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return refuse(&format!("{PACK_DIR} is a link"));
        }
        Ok(meta) if !meta.is_dir() => return refuse(&format!("{PACK_DIR} is not a folder")),
        Ok(_) => {}
        Err(_) => return refuse(&format!("there is no {PACK_DIR} folder")),
    }
    let index = path.join(PACK_FILE);
    if !std::fs::symlink_metadata(&index).is_ok_and(|meta| meta.is_file()) {
        return refuse(&format!("{PACK_DIR} has no {PACK_FILE}"));
    }
    Ok(())
}

/// "Remove GRID files…": deletes `<mount>/gridpack` and everything in it,
/// under the drive's [`PackLock`], after [`guard_grid_root`]. Nothing outside
/// that folder is touched.
pub fn remove_all_grid_files(mount: &Path) -> Result<(), WriteError> {
    let lock = PackLock::for_mount(mount);
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = pack::pack_dir(mount);
    guard_grid_root(mount, &dir)?;
    std::fs::remove_dir_all(&dir).map_err(WriteError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::pack::{
        cleanup_leftovers, index_path, new_pack, pack_dir, write_pack_atomic,
    };

    /// A tempdir "drive" set up for GRID.
    fn drive() -> tempfile::TempDir {
        let drive = tempfile::tempdir().unwrap();
        write_pack_atomic(drive.path(), &new_pack("Test", &[])).unwrap();
        drive
    }

    fn target(mount: &Path) -> WriteTarget<'_> {
        WriteTarget {
            mount,
            label: "TESTDRIVE",
            fat32: false,
        }
    }

    /// A staged source file of `len` bytes with varied content.
    fn source(dir: &Path, len: usize) -> PathBuf {
        let path = dir.join("src.bin");
        let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn md5_hex(bytes: &[u8]) -> String {
        Md5::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn no_progress() -> impl FnMut(u64, u64) {
        |_, _| {}
    }

    #[test]
    fn writes_a_file_through_a_temporary_name() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES * 2 + 17);
        let expected = std::fs::read(&src).unwrap();
        let md5 = md5_hex(&expected);
        let mut reports = Vec::new();
        let written = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/genesis/Game.bin",
                md5: Some(&md5.to_ascii_uppercase()),
            },
            &AtomicBool::new(false),
            &mut |done, total| reports.push((done, total)),
        )
        .unwrap();
        assert_eq!(written, expected.len() as u64);
        let dest = pack_dir(drive.path()).join("games/genesis/Game.bin");
        assert_eq!(std::fs::read(&dest).unwrap(), expected);
        assert!(!tmp_path(&dest).exists());
        assert_eq!(reports.len(), 3);
        assert_eq!(reports.last(), Some(&(written, written)));
    }

    #[test]
    fn fat32_refuses_a_file_over_4_gb() {
        let error = check_fat32(FAT32_MAX_FILE + 1, true, "Big.iso", "STICK").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Big.iso is over 4 GB, which STICK (FAT32) can't hold"
        );
        assert!(check_fat32(FAT32_MAX_FILE, true, "Ok.iso", "STICK").is_ok());
        assert!(check_fat32(FAT32_MAX_FILE + 1, false, "Big.iso", "STICK").is_ok());
    }

    /// The real check, with a sparse source, refuses before writing anything.
    #[cfg(unix)]
    #[test]
    fn write_file_refuses_a_big_file_on_fat32() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = staging.path().join("big.iso");
        File::create(&src)
            .unwrap()
            .set_len(FAT32_MAX_FILE + 1)
            .unwrap();
        let fat32 = WriteTarget {
            fat32: true,
            ..target(drive.path())
        };
        let error = write_file(
            &fat32,
            &FileWrite {
                src: &src,
                rel: "games/ps2/big.iso",
                md5: None,
            },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
        assert!(
            matches!(error, WriteError::TooBigForFat32 { .. }),
            "{error:?}"
        );
        assert!(!pack_dir(drive.path()).join("games").exists());
    }

    #[test]
    fn space_check_keeps_the_margin() {
        assert!(check_space(SPACE_MARGIN + 100, 100).is_ok());
        assert!(matches!(
            check_space(SPACE_MARGIN + 99, 100),
            Err(WriteError::NoSpace)
        ));
        // The sum saturates instead of wrapping to a small number.
        assert!(check_space(u64::MAX - 1, u64::MAX).is_err());
    }

    #[test]
    fn an_interrupted_copy_leaves_only_the_temporary_file_and_cleanup_removes_it() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES * 3);
        let dest = pack_dir(drive.path()).join("games/x/Game.bin");
        // A crash mid-copy: the process stops after the first chunk.
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = write_file(
                &target(drive.path()),
                &FileWrite {
                    src: &src,
                    rel: "games/x/Game.bin",
                    md5: None,
                },
                &AtomicBool::new(false),
                &mut |_, _| panic!("power cut"),
            );
        }));
        assert!(crashed.is_err());
        assert!(!dest.exists());
        let tmp = tmp_path(&dest);
        assert!(tmp.exists());
        let names: Vec<_> = std::fs::read_dir(dest.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![tmp.file_name().unwrap().to_owned()]);

        assert_eq!(cleanup_leftovers(drive.path()).unwrap(), 1);
        assert!(!tmp.exists());
        assert!(index_path(drive.path()).exists());
    }

    #[test]
    fn a_size_mismatch_fails_and_leaves_no_file() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES * 2);
        let dest = pack_dir(drive.path()).join("games/x/Game.bin");
        // The source grows once mid-copy, so more bytes arrive than it had.
        let mut grown = false;
        let mut grow = |_, _| {
            if !grown {
                grown = true;
                let mut file = std::fs::OpenOptions::new().append(true).open(&src).unwrap();
                file.write_all(b"extra").unwrap();
            }
        };
        let error = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/x/Game.bin",
                md5: None,
            },
            &AtomicBool::new(false),
            &mut grow,
        )
        .unwrap_err();
        assert!(matches!(error, WriteError::Mismatch(_)), "{error:?}");
        assert!(error.to_string().contains("size"));
        assert!(!dest.exists());
        assert!(!tmp_path(&dest).exists());
    }

    #[test]
    fn an_md5_mismatch_fails_and_leaves_no_file() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), 1000);
        let dest = pack_dir(drive.path()).join("bios/x.bin");
        let error = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "bios/x.bin",
                md5: Some("00000000000000000000000000000000"),
            },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("MD5"), "{error}");
        assert!(!dest.exists());
        assert!(!tmp_path(&dest).exists());
    }

    #[test]
    fn rename_replaces_an_existing_file() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let dest = pack_dir(drive.path()).join("games/x/Game.bin");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"old content that is longer").unwrap();
        let src = staging.path().join("new.bin");
        std::fs::write(&src, b"new").unwrap();
        write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/x/Game.bin",
                md5: None,
            },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        assert!(!tmp_path(&dest).exists());
    }

    #[test]
    fn a_cancel_stops_the_copy_and_removes_the_temporary_file() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES * 3);
        let dest = pack_dir(drive.path()).join("games/x/Game.bin");
        let cancel = AtomicBool::new(false);
        let error = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/x/Game.bin",
                md5: None,
            },
            &cancel,
            &mut |_, _| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap_err();
        assert!(matches!(error, WriteError::Cancelled), "{error:?}");
        assert!(!dest.exists());
        assert!(!tmp_path(&dest).exists());
    }

    #[test]
    fn an_unsafe_path_is_refused() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), 10);
        for rel in ["../outside.bin", "/etc/x", "games/../../x"] {
            let error = write_file(
                &target(drive.path()),
                &FileWrite {
                    src: &src,
                    rel,
                    md5: None,
                },
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
            assert!(matches!(error, WriteError::Pack(_)), "{rel}: {error:?}");
        }
        assert!(!drive.path().join("outside.bin").exists());
    }

    #[test]
    fn the_temporary_name_stays_within_the_component_limit() {
        let dest = Path::new("gridpack/games").join("é".repeat(127));
        let tmp = tmp_path(&dest);
        let name = tmp.file_name().unwrap().to_str().unwrap();
        assert!(name.len() <= MAX_COMPONENT_BYTES);
        assert!(name.ends_with(TMP_SUFFIX));
        assert_eq!(
            tmp_path(Path::new("a/Game.bin")),
            Path::new("a/Game.bin.gridtmp")
        );
    }

    #[test]
    fn classify_io_says_removed_when_the_mount_folder_is_gone() {
        let drive = drive();
        let mount = drive.path().to_path_buf();
        std::fs::remove_dir_all(&mount).unwrap();
        let storage_full = io::Error::from(io::ErrorKind::StorageFull);
        assert!(matches!(
            classify_io(storage_full, &mount),
            WriteError::DriveRemoved
        ));
        assert!(matches!(
            classify_io(io::Error::other("eio"), &mount),
            WriteError::DriveRemoved
        ));
    }

    #[test]
    fn classify_io_says_removed_when_the_index_is_gone() {
        let drive = drive();
        std::fs::remove_file(index_path(drive.path())).unwrap();
        assert!(matches!(
            classify_io(io::Error::other("eio"), drive.path()),
            WriteError::DriveRemoved
        ));
    }

    #[test]
    fn classify_io_sorts_errors_on_a_present_drive() {
        let drive = drive();
        assert!(matches!(
            classify_io(io::Error::from(io::ErrorKind::StorageFull), drive.path()),
            WriteError::NoSpace
        ));
        assert!(matches!(
            classify_io(io::Error::other("eio"), drive.path()),
            WriteError::Io(_)
        ));
    }

    /// The drive disappears mid-write: the error says removed, not I/O.
    #[cfg(unix)]
    #[test]
    fn a_drive_removed_mid_write_reports_removed() {
        let drive = drive();
        let mount = drive.path().join("mnt");
        std::fs::create_dir(&mount).unwrap();
        write_pack_atomic(&mount, &new_pack("Test", &[])).unwrap();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES * 2);
        let mut yank = |_, _| {
            let _ = std::fs::remove_dir_all(&mount);
        };
        let error = write_file(
            &target(&mount),
            &FileWrite {
                src: &src,
                rel: "games/x/Game.bin",
                md5: None,
            },
            &AtomicBool::new(false),
            &mut yank,
        )
        .unwrap_err();
        assert!(matches!(error, WriteError::DriveRemoved), "{error:?}");
    }

    #[test]
    fn remove_rel_and_remove_dir_rel_stay_under_the_pack() {
        let drive = drive();
        let pack = pack_dir(drive.path());
        std::fs::create_dir_all(pack.join("games/x")).unwrap();
        std::fs::write(pack.join("games/x/a.bin"), b"a").unwrap();
        std::fs::write(drive.path().join("outside.txt"), b"keep").unwrap();

        assert!(matches!(
            remove_rel(drive.path(), "../outside.txt"),
            Err(WriteError::Pack(_))
        ));
        assert!(matches!(
            remove_dir_rel(drive.path(), "games/.."),
            Err(WriteError::Pack(_))
        ));
        assert!(matches!(
            remove_rel(drive.path(), "GRIDPACK.JSON"),
            Err(WriteError::Refused(_))
        ));
        assert!(matches!(
            remove_dir_rel(drive.path(), "games/x/a.bin"),
            Err(WriteError::Refused(_))
        ));
        assert!(matches!(
            remove_rel(drive.path(), "games/x"),
            Err(WriteError::Refused(_))
        ));

        remove_rel(drive.path(), "games/x/a.bin").unwrap();
        assert!(!pack.join("games/x/a.bin").exists());
        remove_rel(drive.path(), "games/x/a.bin").unwrap();
        remove_dir_rel(drive.path(), "games").unwrap();
        assert!(!pack.join("games").exists());
        remove_dir_rel(drive.path(), "games").unwrap();

        assert!(drive.path().join("outside.txt").exists());
        assert!(index_path(drive.path()).exists());
    }

    #[test]
    fn remove_all_grid_files_removes_only_the_pack_folder() {
        let drive = drive();
        std::fs::create_dir_all(pack_dir(drive.path()).join("games")).unwrap();
        std::fs::write(drive.path().join("photo.jpg"), b"keep").unwrap();
        remove_all_grid_files(drive.path()).unwrap();
        assert!(!pack_dir(drive.path()).exists());
        assert!(drive.path().join("photo.jpg").exists());
    }

    #[test]
    fn the_guard_refuses_a_folder_without_its_index() {
        let drive = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(pack_dir(drive.path()).join("games")).unwrap();
        assert!(matches!(
            remove_all_grid_files(drive.path()),
            Err(WriteError::Refused(_))
        ));
        assert!(pack_dir(drive.path()).join("games").exists());
        // No gridpack folder at all.
        let empty = tempfile::tempdir().unwrap();
        assert!(remove_all_grid_files(empty.path()).is_err());
        // An index that is a folder is not an index.
        let odd = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(index_path(odd.path())).unwrap();
        assert!(remove_all_grid_files(odd.path()).is_err());
        assert!(index_path(odd.path()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_guard_refuses_a_linked_pack_folder() {
        let elsewhere = drive();
        let linked = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(pack_dir(elsewhere.path()), pack_dir(linked.path())).unwrap();
        assert!(matches!(
            remove_all_grid_files(linked.path()),
            Err(WriteError::Refused(_))
        ));
        assert!(index_path(elsewhere.path()).exists());
    }

    #[test]
    fn the_guard_refuses_any_other_path() {
        let drive = drive();
        let mount = drive.path();
        std::fs::create_dir_all(pack_dir(mount).join("games")).unwrap();
        let other = tempfile::tempdir().unwrap();
        for path in [
            mount.to_path_buf(),
            mount.join("games"),
            pack_dir(mount).join("games"),
            mount.join("GRIDPACK2"),
            pack_dir(other.path()),
            mount.join("x").join("..").join(PACK_DIR),
        ] {
            assert!(
                matches!(guard_grid_root(mount, &path), Err(WriteError::Refused(_))),
                "{path:?} must be refused"
            );
        }
        // A mount that is relative or has a `..` part.
        assert!(guard_grid_root(Path::new("rel"), &Path::new("rel").join(PACK_DIR)).is_err());
        let dotted = mount.join("sub").join("..");
        assert!(guard_grid_root(&dotted, &dotted.join(PACK_DIR)).is_err());
        // The one path it accepts.
        guard_grid_root(mount, &pack_dir(mount)).unwrap();
        assert!(index_path(mount).exists());
    }
}
