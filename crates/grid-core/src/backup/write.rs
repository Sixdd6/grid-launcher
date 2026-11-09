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
    /// The written file's size, or the bytes read back from the drive,
    /// differ from the staged source.
    #[error("{0}")]
    Mismatch(String),
    /// The cancel flag was set. The temporary file was removed.
    #[error("cancelled")]
    Cancelled,
    /// An unsafe pack-relative path, or a `gridpack` folder that is a link.
    #[error(transparent)]
    Pack(#[from] PackError),
    /// A local file being archived could not be read, or its length is not
    /// what the tree scan saw: the game changed it while it was copied.
    /// Never a drive error. Holds the file's path inside the tree.
    #[error("{0} changed while it was being copied; close the game and back up again")]
    SourceChanged(String),
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
/// 6. checks its size against the source's, then reads it back from the
///    drive and checks its MD5 against the MD5 of the source bytes it copied
///    ([`verify_on_drive`]);
/// 7. renames it onto `rel`, replacing an existing file.
///
/// The check is against what GRID staged, never against a hash the server
/// states: RomM hashes an archive's decompressed contents, not its bytes.
///
/// `progress(done, total)` covers the copy and the read-back as one span:
/// each moves `done` through half of `total`.
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
    let copied =
        copy_checked(&mut src, &tmp, size, &name, mount, cancel, progress).and_then(|written| {
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
fn copy_checked(
    src: &mut File,
    tmp: &Path,
    size: u64,
    name: &str,
    mount: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<u64, WriteError> {
    let on_drive = |error: io::Error| classify_io(error, mount);
    let size_mismatch = || {
        WriteError::Mismatch(format!(
            "{name} didn't copy correctly: its size doesn't match"
        ))
    };
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
            return Err(size_mismatch());
        }
        out.write_all(&buf[..n]).map_err(on_drive)?;
        hasher.update(&buf[..n]);
        progress(written / 2, size);
    }
    out.sync_all().map_err(on_drive)?;
    let on_disk = out.metadata().map_err(on_drive)?.len();
    drop(out);
    if written != size || on_disk != size {
        return Err(size_mismatch());
    }
    let source_md5: [u8; 16] = hasher.finalize().into();
    #[cfg(test)]
    tests::after_sync(tmp);
    let read_back = verify_on_drive(tmp, size, mount, cancel, &mut |read| {
        progress(size / 2 + read - read / 2, size);
    })?;
    if read_back != Some(source_md5) {
        return Err(WriteError::Mismatch(format!(
            "{name} didn't copy correctly: the copy on the drive doesn't match the download"
        )));
    }
    Ok(written)
}

/// Reads `tmp` back from the drive and returns its MD5, or `None` when the
/// file isn't `size` bytes long. `progress(read)` follows each chunk.
///
/// What this verifies: the bytes the drive returns for the file after
/// `sync_all`, against the bytes GRID copied. To make that a read from the
/// device and not from the OS's page cache, the cache is bypassed where the
/// OS allows it:
/// - Linux: the file's cached pages are dropped with
///   `posix_fadvise(POSIX_FADV_DONTNEED)` (the pages are clean after the
///   sync, so the kernel drops them) before the read.
/// - macOS: the read handle is opened with `F_NOCACHE`. Pages already cached
///   may still be served, so this is best effort.
/// - Windows: the read handle uses `FILE_FLAG_NO_BUFFERING`, with an aligned
///   buffer. If the system refuses an unbuffered read, the check falls back
///   to a normal read, which may come from the cache.
///
/// What it does not verify: a drive's own on-board cache, or a FUSE file
/// system's private cache, can still answer the read. The cost is one full
/// read of every file from the drive, which on a typical USB stick adds
/// roughly a third to a half of the copy time.
pub(super) fn verify_on_drive(
    tmp: &Path,
    size: u64,
    mount: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<Option<[u8; 16]>, WriteError> {
    let on_drive = |error: io::Error| classify_io(error, mount);
    let outcome = |hashed: io::Result<Hashed>| match hashed {
        Ok(Hashed::Done(md5)) => Ok(md5),
        Ok(Hashed::Cancelled) => Err(WriteError::Cancelled),
        Err(error) => Err(classify_io(error, mount)),
    };
    let file = open_uncached(tmp).map_err(on_drive)?;
    let hashed = hash_file(file, size, cancel, progress);
    // ERROR_INVALID_PARAMETER: the volume won't take an unbuffered read of
    // this shape. Read it the normal way.
    #[cfg(windows)]
    if matches!(&hashed, Err(error) if error.raw_os_error() == Some(87)) {
        let file = File::open(tmp).map_err(on_drive)?;
        return outcome(hash_file(file, size, cancel, progress));
    }
    outcome(hashed)
}

/// What [`hash_file`] found.
enum Hashed {
    /// The MD5, or `None` when the file isn't the expected size.
    Done(Option<[u8; 16]>),
    Cancelled,
}

/// MD5 of all of `file`, `None` when it isn't `size` bytes long.
fn hash_file(
    mut file: File,
    size: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> io::Result<Hashed> {
    // Unbuffered reads on Windows need a buffer aligned to the sector size;
    // 4096 covers every common one. CHUNK_BYTES is a multiple of it.
    const ALIGN: usize = 4096;
    let mut storage = vec![0u8; CHUNK_BYTES + ALIGN];
    let start = storage.as_ptr().align_offset(ALIGN).min(ALIGN);
    let buf = &mut storage[start..start + CHUNK_BYTES];
    let mut hasher = Md5::new();
    let mut read: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Hashed::Cancelled);
        }
        let n = match file.read(buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        read += n as u64;
        if read > size {
            return Ok(Hashed::Done(None));
        }
        hasher.update(&buf[..n]);
        progress(read);
    }
    Ok(Hashed::Done(
        (read == size).then(|| hasher.finalize().into()),
    ))
}

/// Opens `path` for a read that bypasses the OS cache where possible (see
/// [`verify_on_drive`]).
#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_uncached(path: &Path) -> io::Result<File> {
    use std::os::fd::AsRawFd;
    let file = File::open(path)?;
    // SAFETY: a valid open descriptor; the call only advises the kernel.
    // A refusal leaves the cache as it is, which only weakens the check.
    unsafe {
        libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
    }
    Ok(file)
}

#[cfg(target_os = "macos")]
fn open_uncached(path: &Path) -> io::Result<File> {
    use std::os::fd::AsRawFd;
    let file = File::open(path)?;
    // SAFETY: a valid open descriptor; F_NOCACHE takes an int flag.
    unsafe {
        libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1);
    }
    Ok(file)
}

#[cfg(windows)]
fn open_uncached(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_NO_BUFFERING;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING)
        .open(path)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
)))]
fn open_uncached(path: &Path) -> io::Result<File> {
    File::open(path)
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
pub(super) fn check_space(free: u64, size: u64) -> Result<(), WriteError> {
    if free < size.saturating_add(SPACE_MARGIN) {
        return Err(WriteError::NoSpace);
    }
    Ok(())
}

/// `<dest>.gridtmp`. When that name would exceed [`MAX_COMPONENT_BYTES`],
/// the name is cut first so the temporary name stays legal on FAT, exFAT
/// and NTFS. It always ends in [`TMP_SUFFIX`], so cleanup still finds it.
pub(super) fn tmp_path(dest: &Path) -> PathBuf {
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
pub(super) fn sync_folder(dir: Option<&Path>) {
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
pub(super) fn drive_gone(mount: &Path) -> bool {
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

    /// A hook run on a synced temporary file.
    type Hook = Box<dyn Fn(&Path)>;

    thread_local! {
        /// Runs on the synced temporary file, before the read-back.
        static AFTER_SYNC: std::cell::RefCell<Option<Hook>> =
            const { std::cell::RefCell::new(None) };
    }

    /// The test hook [`copy_checked`] calls between the sync and the
    /// read-back, on this thread only.
    pub(super) fn after_sync(tmp: &Path) {
        AFTER_SYNC.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(tmp);
            }
        });
    }

    fn set_after_sync(hook: impl Fn(&Path) + 'static) {
        AFTER_SYNC.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
    }

    fn clear_after_sync() {
        AFTER_SYNC.with(|slot| *slot.borrow_mut() = None);
    }

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

    fn no_progress() -> impl FnMut(u64, u64) {
        |_, _| {}
    }

    #[test]
    fn writes_a_file_through_a_temporary_name() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES * 2 + 17);
        let expected = std::fs::read(&src).unwrap();
        let mut reports = Vec::new();
        let written = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/genesis/Game.bin",
            },
            &AtomicBool::new(false),
            &mut |done, total| reports.push((done, total)),
        )
        .unwrap();
        assert_eq!(written, expected.len() as u64);
        let dest = pack_dir(drive.path()).join("games/genesis/Game.bin");
        assert_eq!(std::fs::read(&dest).unwrap(), expected);
        assert!(!tmp_path(&dest).exists());
        // Three chunks copied, then the same three read back.
        assert_eq!(reports.len(), 6);
        assert!(reports.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        assert_eq!(reports[2], (written / 2, written));
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

    /// The file on the drive is damaged after the sync: the read-back
    /// catches it, whatever the server says about the file.
    #[test]
    fn a_copy_damaged_on_the_drive_fails_and_leaves_no_file() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), CHUNK_BYTES + 1000);
        let dest = pack_dir(drive.path()).join("games/x/Game.zip");
        set_after_sync(|tmp| {
            let mut bytes = std::fs::read(tmp).unwrap();
            bytes[CHUNK_BYTES + 10] ^= 0xff;
            std::fs::write(tmp, bytes).unwrap();
        });
        let error = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/x/Game.zip",
            },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
        clear_after_sync();
        assert!(matches!(error, WriteError::Mismatch(_)), "{error:?}");
        assert_eq!(
            error.to_string(),
            "Game.zip didn't copy correctly: the copy on the drive doesn't match the download"
        );
        assert!(!dest.exists());
        assert!(!tmp_path(&dest).exists());
    }

    /// A file cut short on the drive after the sync fails the read-back.
    #[test]
    fn a_copy_cut_short_on_the_drive_fails() {
        let drive = drive();
        let staging = tempfile::tempdir().unwrap();
        let src = source(staging.path(), 1000);
        set_after_sync(|tmp| {
            File::options()
                .write(true)
                .open(tmp)
                .unwrap()
                .set_len(999)
                .unwrap();
        });
        let error = write_file(
            &target(drive.path()),
            &FileWrite {
                src: &src,
                rel: "games/x/Short.bin",
            },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
        clear_after_sync();
        assert!(matches!(error, WriteError::Mismatch(_)), "{error:?}");
        assert!(!pack_dir(drive.path()).join("games/x/Short.bin").exists());
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
                &FileWrite { src: &src, rel },
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
