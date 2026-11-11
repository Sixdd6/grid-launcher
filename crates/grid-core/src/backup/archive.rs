//! The uncompressed tar parts an installed copy is written as, and reading
//! them back on restore.
//!
//! Writing ([`write_installed_tree`]): a `tar::Builder` streams the tree
//! into a [`TarPartWriter`], which cuts the stream into
//! `installed.tar.001`, `.002`, … parts of at most `part_bytes` each. Every
//! part goes to `<part>.gridtmp`, is hashed as it is written, synced, read
//! back from the drive past the page cache and compared, the same check as
//! [`super::write::write_file`]. Only when every part is checked are they
//! renamed to their final names.
//!
//! Restoring ([`restore_tree`]): a [`TarPartReader`] chains the parts off the
//! drive as one stream, checking each part's size and MD5 at its end, and
//! the entries are written into a fresh folder. Only regular files and
//! folders with plain relative names are accepted.
//!
//! Anyone can rebuild an archive without GRID: `cat installed.tar.* | tar x`.

use std::cell::{Cell, RefCell};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use md5::{Digest, Md5};

use super::drives::disk_space;
use super::local::{self, TreeEntry, TreeEntryKind, TreePlan};
use super::pack::{self, PackPart, PART_BYTES};
use super::write::{self, classify_io, WriteError, WriteTarget, CHUNK_BYTES};
use crate::library::extract::{is_absolute_entry_path, unsafe_path_error};

/// A part size tests set in place of [`PART_BYTES`]; 0 when unset.
static PART_BYTES_OVERRIDE: AtomicU64 = AtomicU64::new(0);

/// Makes [`part_bytes`] return `bytes` (or [`PART_BYTES`] again for
/// `None`), so tests can write several small parts. Process-wide.
#[doc(hidden)]
pub fn set_part_bytes_override(bytes: Option<u64>) {
    PART_BYTES_OVERRIDE.store(bytes.unwrap_or(0), Ordering::Relaxed);
}

/// The part size a backup uses: [`PART_BYTES`], unless a test overrode it.
pub fn part_bytes() -> u64 {
    match PART_BYTES_OVERRIDE.load(Ordering::Relaxed) {
        0 => PART_BYTES,
        bytes => bytes,
    }
}

fn hex(md5: [u8; 16]) -> String {
    md5.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn last_part(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

// --- Writing -----------------------------------------------------------------

/// The part being written.
struct OpenPart {
    file: File,
    rel: String,
    tmp: PathBuf,
    hasher: Md5,
    len: u64,
}

/// An [`io::Write`] that splits what it is given into checked part files on
/// the drive (see the module docs). Parts stay under their `.gridtmp` names
/// until [`TarPartWriter::finish_parts`] renames them.
///
/// The first error is kept ([`TarPartWriter::take_failure`]); the
/// `io::Error` it returns only says the write stopped.
pub struct TarPartWriter<'a> {
    mount: &'a Path,
    rel_base: String,
    /// The archive's name: `installed` ([`pack::INSTALLED_TAR`]) unless
    /// [`Self::with_stem`] set another.
    stem: &'static str,
    part_bytes: u64,
    /// The archive's expected size, for the space check.
    expected: u64,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(u64, u64),
    /// Runs before each part is created (a test hook; see
    /// [`write_installed_tree_with`]).
    before_part: Option<&'a mut dyn FnMut()>,
    open: Option<OpenPart>,
    /// The checked parts, in order.
    parts: Vec<PackPart>,
    /// `(tmp, dest)` of every part begun.
    files: Vec<(PathBuf, PathBuf)>,
    /// How many of `files` are renamed onto `dest`.
    renamed: usize,
    written: u64,
    verified: u64,
    reported: u64,
    failure: Option<WriteError>,
    stopped: bool,
}

impl<'a> TarPartWriter<'a> {
    /// A writer of `<rel_base>/installed.tar.NNN` parts of at most
    /// `part_bytes` (at least 1). `progress(done, total)` runs over
    /// `2 × archive_bytes`: the write, then each part's read-back.
    pub fn new(
        target: &WriteTarget<'a>,
        rel_base: &str,
        part_bytes: u64,
        archive_bytes: u64,
        cancel: &'a AtomicBool,
        progress: &'a mut dyn FnMut(u64, u64),
    ) -> Self {
        Self {
            mount: target.mount,
            rel_base: rel_base.to_string(),
            stem: pack::INSTALLED_TAR,
            part_bytes: part_bytes.max(1),
            expected: archive_bytes,
            cancel,
            progress,
            before_part: None,
            open: None,
            parts: Vec::new(),
            files: Vec::new(),
            renamed: 0,
            written: 0,
            verified: 0,
            reported: 0,
            failure: None,
            stopped: false,
        }
    }

    /// Names the parts `<rel_base>/<stem>.tar.NNN` ([`pack::tar_part_rel`]).
    pub fn with_stem(mut self, stem: &'static str) -> Self {
        self.stem = stem;
        self
    }

    /// Runs `hook` before each part is created.
    pub fn with_before_part(mut self, hook: &'a mut dyn FnMut()) -> Self {
        self.before_part = Some(hook);
        self
    }

    fn total(&self) -> u64 {
        self.expected.saturating_mul(2)
    }

    /// The error that stopped the writer, if any.
    pub fn take_failure(&mut self) -> Option<WriteError> {
        self.failure.take()
    }

    /// Refuses every later write, so nothing more reaches the drive (a
    /// dropped `tar::Builder` still tries to write its end blocks).
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    fn report(&mut self, force: bool) {
        let total = self.total();
        let done = (self.written + self.verified).min(total);
        if force || done >= self.reported + CHUNK_BYTES as u64 {
            self.reported = done;
            (self.progress)(done, total);
        }
    }

    fn open_part(&mut self) -> Result<(), WriteError> {
        let mount = self.mount;
        if let Some(hook) = self.before_part.as_mut() {
            hook();
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(WriteError::Cancelled);
        }
        let size = self
            .expected
            .saturating_sub(self.written)
            .min(self.part_bytes);
        match disk_space(mount) {
            Some((_, free)) => write::check_space(free, size)?,
            None if write::drive_gone(mount) => return Err(WriteError::DriveRemoved),
            None => {}
        }
        let rel = pack::tar_part_rel(&self.rel_base, self.stem, self.parts.len());
        let dest = pack::resolve(mount, &rel)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|error| classify_io(error, mount))?;
        }
        let tmp = write::tmp_path(&dest);
        self.files.push((tmp.clone(), dest));
        let file = File::create(&tmp).map_err(|error| classify_io(error, mount))?;
        self.open = Some(OpenPart {
            file,
            rel,
            tmp,
            hasher: Md5::new(),
            len: 0,
        });
        Ok(())
    }

    /// Syncs the open part, checks its size, reads it back from the drive
    /// and compares the MD5 with the bytes written.
    fn finish_part(&mut self) -> Result<(), WriteError> {
        let Some(OpenPart {
            file,
            rel,
            tmp,
            hasher,
            len,
        }) = self.open.take()
        else {
            return Ok(());
        };
        let mount = self.mount;
        let on_drive = |error: io::Error| classify_io(error, mount);
        let name = last_part(&rel).to_string();
        file.sync_all().map_err(on_drive)?;
        let on_disk = file.metadata().map_err(on_drive)?.len();
        drop(file);
        if on_disk != len {
            return Err(WriteError::Mismatch(format!(
                "{name} didn't copy correctly: its size doesn't match"
            )));
        }
        let md5: [u8; 16] = hasher.finalize().into();
        #[cfg(test)]
        tests::after_part_sync(&tmp);
        let base = self.written + self.verified;
        let total = self.total();
        let progress = &mut *self.progress;
        let read_back = write::verify_on_drive(&tmp, len, mount, self.cancel, &mut |read| {
            progress((base + read).min(total), total);
        })?;
        self.verified += len;
        if read_back != Some(md5) {
            return Err(WriteError::Mismatch(format!(
                "{name} didn't copy correctly: the copy on the drive doesn't match what GRID wrote"
            )));
        }
        self.parts.push(PackPart {
            path: rel,
            size: len,
            md5: hex(md5),
        });
        Ok(())
    }

    fn write_inner(&mut self, buf: &[u8]) -> Result<usize, WriteError> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(WriteError::Cancelled);
        }
        if buf.is_empty() {
            return Ok(0);
        }
        if self.open.is_none() {
            self.open_part()?;
        }
        let mount = self.mount;
        let part_bytes = self.part_bytes;
        let Some(part) = self.open.as_mut() else {
            return Err(WriteError::Io(io::Error::other("no part is open")));
        };
        // Never 0: a part is closed as soon as it is full.
        let room = part_bytes - part.len;
        let n = (buf.len() as u64).min(room) as usize;
        part.file
            .write_all(&buf[..n])
            .map_err(|error| classify_io(error, mount))?;
        part.hasher.update(&buf[..n]);
        part.len += n as u64;
        self.written += n as u64;
        if part.len == part_bytes {
            self.finish_part()?;
        }
        self.report(false);
        Ok(n)
    }

    /// Checks the last part, then renames every part onto its final name.
    /// Returns the parts in order.
    pub fn finish_parts(&mut self) -> Result<Vec<PackPart>, WriteError> {
        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }
        self.finish_part()?;
        let mount = self.mount;
        for (tmp, dest) in &self.files[self.renamed..] {
            fs::rename(tmp, dest).map_err(|error| classify_io(error, mount))?;
            self.renamed += 1;
        }
        if let Some((_, dest)) = self.files.first() {
            write::sync_folder(dest.parent());
        }
        self.report(true);
        Ok(std::mem::take(&mut self.parts))
    }

    /// Removes every part this writer wrote, under either name.
    pub fn remove_written(&mut self) {
        self.open = None;
        for (index, (tmp, dest)) in self.files.iter().enumerate() {
            let _ = fs::remove_file(tmp);
            if index < self.renamed {
                let _ = fs::remove_file(dest);
            }
        }
    }
}

impl Write for TarPartWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.stopped || self.failure.is_some() {
            return Err(io::Error::other("the drive write stopped"));
        }
        self.write_inner(buf).map_err(|error| {
            self.failure = Some(error);
            io::Error::other("the drive write stopped")
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Reads one local file for the archive: exactly `left` bytes, or it fails
/// and sets `changed`. A read error, an early end or extra bytes all mean
/// the file changed since the scan.
struct SourceReader<'s> {
    file: File,
    left: u64,
    changed: &'s Cell<bool>,
}

impl SourceReader<'_> {
    fn changed(&self) -> io::Error {
        self.changed.set(true);
        io::Error::other("the file changed while it was read")
    }
}

impl Read for SourceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            // The file must end where the scan said it did.
            let mut probe = [0u8; 1];
            return match self.file.read(&mut probe) {
                Ok(0) => Ok(0),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => Err(error),
                _ => Err(self.changed()),
            };
        }
        let want = buf
            .len()
            .min(usize::try_from(self.left).unwrap_or(usize::MAX));
        match self.file.read(&mut buf[..want]) {
            Ok(0) => Err(self.changed()),
            Ok(n) => {
                self.left -= n as u64;
                Ok(n)
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => Err(error),
            Err(_) => Err(self.changed()),
        }
    }
}

/// The entry's name as shown in an error: a folder without its `/`.
fn shown_name(entry: &TreeEntry) -> String {
    entry.name.trim_end_matches('/').to_string()
}

/// Refuses an entry name that is empty, absolute, or has an empty, `.` or
/// `..` part. A folder's name may end in `/`; a file's may not.
fn check_entry_name(entry: &TreeEntry) -> Result<(), WriteError> {
    let rel = match entry.kind {
        TreeEntryKind::Dir => entry.name.strip_suffix('/').unwrap_or(&entry.name),
        TreeEntryKind::File => entry.name.as_str(),
    };
    let plain = !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains('\0')
        && rel
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if plain {
        Ok(())
    } else {
        Err(WriteError::Refused(format!(
            "{} is not a plain path inside the game's folder",
            entry.name
        )))
    }
}

/// Appends one entry with [`local::tar_header`] under its scanned name, as
/// [`TreePlan::archive_bytes`] counts it. An error from the local file is
/// returned as [`WriteError::SourceChanged`] directly, or flagged in
/// `changed` when it happens inside the builder.
fn append_entry<W: Write>(
    builder: &mut tar::Builder<W>,
    entry: &TreeEntry,
    changed: &Cell<bool>,
) -> Result<io::Result<()>, WriteError> {
    let source_changed = || WriteError::SourceChanged(shown_name(entry));
    let mut header = local::tar_header(entry);
    match entry.kind {
        TreeEntryKind::Dir => {
            let meta = fs::symlink_metadata(&entry.path).map_err(|_| source_changed())?;
            if !meta.is_dir() {
                return Err(source_changed());
            }
            Ok(builder.append_data(&mut header, &entry.name, io::empty()))
        }
        TreeEntryKind::File => {
            let file = File::open(&entry.path).map_err(|_| source_changed())?;
            let meta = file.metadata().map_err(|_| source_changed())?;
            if !meta.is_file() || meta.len() != entry.size {
                return Err(source_changed());
            }
            let source = SourceReader {
                file,
                left: entry.size,
                changed,
            };
            Ok(builder.append_data(&mut header, &entry.name, source))
        }
    }
}

type PartBuilder<'w, 'a> = tar::Builder<BufWriter<&'w mut TarPartWriter<'a>>>;

/// Writes the scanned `tree` ([`local::scan_tree`]) to the drive as checked
/// tar parts `<rel_base>/installed.tar.001`, `.002`, … (named by
/// [`pack::installed_part_rel`]), each at most `part_bytes` (use
/// [`part_bytes`]). `rel_base` is a pack-relative folder.
///
/// - Entries go in [`TreePlan::entries`] order, each with
///   [`local::tar_header`] and its scanned name, so the parts together
///   hold exactly [`TreePlan::archive_bytes`] bytes.
/// - A local file that can't be read, or whose length differs from
///   [`TreeEntry::size`], fails with [`WriteError::SourceChanged`], never
///   a drive error.
/// - `cancel` is checked on every write; the space margin at each part.
/// - `progress(done, total)` runs over `2 × tree.archive_bytes`.
/// - On success every part has its final name and the parts are returned
///   in order. On any failure the parts written are removed, unless the
///   drive is gone.
pub fn write_installed_tree(
    target: &WriteTarget<'_>,
    tree: &TreePlan,
    rel_base: &str,
    part_bytes: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<PackPart>, WriteError> {
    write_installed_tree_with(
        target,
        tree,
        rel_base,
        part_bytes,
        cancel,
        progress,
        &mut || {},
    )
}

/// [`write_installed_tree`], running `before_part` before each part file
/// is created (the backup job's per-file test hook).
pub fn write_installed_tree_with(
    target: &WriteTarget<'_>,
    tree: &TreePlan,
    rel_base: &str,
    part_bytes: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    before_part: &mut dyn FnMut(),
) -> Result<Vec<PackPart>, WriteError> {
    write_tree_parts(
        target,
        tree,
        rel_base,
        pack::INSTALLED_TAR,
        part_bytes,
        cancel,
        progress,
        before_part,
    )
}

/// A game's saves ([`super::saves::scan_saves`]) written as checked tar
/// parts `<rel_base>/saves.tar.001`, … ([`pack::saves_part_rel`]), exactly
/// as [`write_installed_tree`] writes an installed copy.
pub fn write_saves_tree(
    target: &WriteTarget<'_>,
    tree: &TreePlan,
    rel_base: &str,
    part_bytes: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<PackPart>, WriteError> {
    write_tree_parts(
        target,
        tree,
        rel_base,
        pack::SAVES_TAR,
        part_bytes,
        cancel,
        progress,
        &mut || {},
    )
}

/// [`write_installed_tree_with`] for the tar named `stem`.
#[allow(clippy::too_many_arguments)]
fn write_tree_parts(
    target: &WriteTarget<'_>,
    tree: &TreePlan,
    rel_base: &str,
    stem: &'static str,
    part_bytes: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    before_part: &mut dyn FnMut(),
) -> Result<Vec<PackPart>, WriteError> {
    pack::validate_rel(rel_base)?;
    for entry in &tree.entries {
        check_entry_name(entry)?;
    }
    let mut writer = TarPartWriter::new(
        target,
        rel_base,
        part_bytes,
        tree.archive_bytes,
        cancel,
        progress,
    )
    .with_stem(stem)
    .with_before_part(before_part);
    let changed = Cell::new(false);
    let result = {
        let mut builder = tar::Builder::new(BufWriter::with_capacity(CHUNK_BYTES, &mut writer));
        let result = build(&mut builder, &tree.entries, &changed);
        if result.is_err() {
            builder.get_mut().get_mut().stop();
        }
        result
    };
    match result {
        Ok(()) => match writer.finish_parts() {
            Ok(parts) => Ok(parts),
            Err(error) => Err(discard(&mut writer, error)),
        },
        Err(error) => Err(discard(&mut writer, error)),
    }
}

/// Removes what `writer` wrote, unless the drive is gone.
fn discard(writer: &mut TarPartWriter<'_>, error: WriteError) -> WriteError {
    writer.stop();
    if !matches!(error, WriteError::DriveRemoved) {
        writer.remove_written();
    }
    error
}

/// Streams every entry and the end blocks into the parts.
fn build(
    builder: &mut PartBuilder<'_, '_>,
    entries: &[TreeEntry],
    changed: &Cell<bool>,
) -> Result<(), WriteError> {
    for entry in entries {
        changed.set(false);
        if let Err(error) = append_entry(builder, entry, changed)? {
            return Err(stopped(builder, error, changed.get().then_some(entry)));
        }
    }
    if let Err(error) = builder.finish() {
        return Err(stopped(builder, error, None));
    }
    if let Err(error) = builder.get_mut().flush() {
        return Err(stopped(builder, error, None));
    }
    Ok(())
}

/// The real reason a builder call failed: the drive's error, else the local
/// file's, else the builder's own.
fn stopped(
    builder: &mut PartBuilder<'_, '_>,
    error: io::Error,
    source: Option<&TreeEntry>,
) -> WriteError {
    if let Some(failure) = builder.get_mut().get_mut().take_failure() {
        return failure;
    }
    match source {
        Some(entry) => WriteError::SourceChanged(shown_name(entry)),
        None => WriteError::Io(error),
    }
}

// --- Reading and restoring -----------------------------------------------------

/// One part to read back: where it is on the drive, and what was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartSource {
    pub path: PathBuf,
    pub size: u64,
    /// Hex MD5 of the bytes GRID wrote.
    pub md5: String,
}

/// Why a restore failed. The restore folder is removed in every case.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    /// The drive is gone: its `gridpack` folder no longer exists.
    #[error("the drive was removed")]
    DriveRemoved,
    /// A part's size or MD5 isn't what was written. Holds its file name.
    #[error("{0} on the drive doesn't match what GRID wrote")]
    PartMismatch(String),
    /// A part is missing from the drive. Holds its file name.
    #[error("{0} is missing from the drive")]
    Incomplete(String),
    /// An entry that isn't a plain file or folder with a plain relative
    /// name.
    #[error("{0}")]
    Unsafe(String),
    #[error("cancelled")]
    Cancelled,
    #[error("file error: {0}")]
    Io(io::Error),
}

/// What a [`TarPartReader`] shares with its caller while a `tar::Archive`
/// owns it: the bytes read and the first error.
#[derive(Default)]
pub struct ReadState {
    bytes: Cell<u64>,
    failure: RefCell<Option<RestoreError>>,
}

impl ReadState {
    /// The part bytes read so far.
    pub fn bytes(&self) -> u64 {
        self.bytes.get()
    }

    /// The reader's own error if it stopped, else `error` as it is.
    pub fn error(&self, error: io::Error) -> RestoreError {
        self.failure
            .borrow_mut()
            .take()
            .unwrap_or(RestoreError::Io(error))
    }
}

struct OpenRead {
    file: File,
    hasher: Md5,
    len: u64,
}

/// Reads the parts as one stream. At the end of each part it checks the
/// part's size and MD5. Errors are kept in [`ReadState`]: a part that
/// doesn't match is [`RestoreError::PartMismatch`], a missing one
/// [`RestoreError::Incomplete`], and any of them while `root` (the drive's
/// `gridpack` folder) is gone is [`RestoreError::DriveRemoved`].
pub struct TarPartReader {
    root: PathBuf,
    parts: Vec<PartSource>,
    index: usize,
    open: Option<OpenRead>,
    state: Rc<ReadState>,
}

impl TarPartReader {
    pub fn new(root: &Path, parts: &[PartSource]) -> Self {
        Self {
            root: root.to_path_buf(),
            parts: parts.to_vec(),
            index: 0,
            open: None,
            state: Rc::default(),
        }
    }

    /// The shared state, to read after the reader is moved into an archive.
    pub fn state(&self) -> Rc<ReadState> {
        Rc::clone(&self.state)
    }

    fn part_name(&self) -> String {
        self.parts
            .get(self.index)
            .and_then(|part| part.path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn drive_gone(&self) -> bool {
        !self.root.is_dir()
    }

    fn mismatch(&self) -> RestoreError {
        if self.drive_gone() {
            RestoreError::DriveRemoved
        } else {
            RestoreError::PartMismatch(self.part_name())
        }
    }

    fn drive_error(&self, error: io::Error) -> RestoreError {
        if self.drive_gone() {
            RestoreError::DriveRemoved
        } else {
            RestoreError::Io(error)
        }
    }

    fn read_inner(&mut self, buf: &mut [u8]) -> Result<usize, RestoreError> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let Some(part) = self.parts.get(self.index) else {
                return Ok(0);
            };
            let (size, md5) = (part.size, part.md5.clone());
            if self.open.is_none() {
                let file = match File::open(&part.path) {
                    Ok(file) => file,
                    Err(_) if self.drive_gone() => return Err(RestoreError::DriveRemoved),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Err(RestoreError::Incomplete(self.part_name()));
                    }
                    Err(error) => return Err(RestoreError::Io(error)),
                };
                self.open = Some(OpenRead {
                    file,
                    hasher: Md5::new(),
                    len: 0,
                });
            }
            let Some(open) = self.open.as_mut() else {
                continue;
            };
            let n = match open.file.read(buf) {
                Ok(n) => n,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(self.drive_error(error)),
            };
            if n == 0 {
                // The end of this part: check it, then go on to the next.
                let Some(done) = self.open.take() else {
                    continue;
                };
                let got = hex(done.hasher.finalize().into());
                if done.len != size || !got.eq_ignore_ascii_case(&md5) {
                    return Err(self.mismatch());
                }
                self.index += 1;
                continue;
            }
            open.len += n as u64;
            if open.len > size {
                return Err(self.mismatch());
            }
            open.hasher.update(&buf[..n]);
            self.state.bytes.set(self.state.bytes.get() + n as u64);
            return Ok(n);
        }
    }
}

impl Read for TarPartReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.state.failure.borrow().is_some() {
            return Err(io::Error::other("the backup couldn't be read"));
        }
        self.read_inner(buf).map_err(|error| {
            *self.state.failure.borrow_mut() = Some(error);
            io::Error::other("the backup couldn't be read")
        })
    }
}

/// The entry name as a plain relative path, or the refusal.
fn safe_rel(raw: &[u8]) -> Result<PathBuf, RestoreError> {
    let display = String::from_utf8_lossy(raw);
    let refuse = || RestoreError::Unsafe(unsafe_path_error(&display).to_string());
    let name = std::str::from_utf8(raw).map_err(|_| refuse())?;
    if name.is_empty() || name.contains('\0') || is_absolute_entry_path(name) {
        return Err(refuse());
    }
    // A folder's name may end in `/`.
    let name = name.strip_suffix('/').unwrap_or(name);
    let mut out = PathBuf::new();
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(refuse());
        }
        #[cfg(windows)]
        if part.contains(['\\', ':']) {
            return Err(refuse());
        }
        out.push(part);
    }
    if !out
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(refuse());
    }
    Ok(out)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if mode == 0 {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Restores the archive in `parts` (in order) into `dest`.
///
/// - `root` is the drive's `gridpack` folder: an error while it is gone is
///   [`RestoreError::DriveRemoved`].
/// - `dest` must not exist, or be an empty folder. It is removed with
///   everything in it on any failure or cancel.
/// - Only regular files and folders are accepted. Links (symbolic or hard),
///   devices and FIFOs are refused, and so is any name that isn't plain and
///   relative (absolute, `..`, `.`, empty parts).
/// - Unix modes are kept, masked with `0o777`.
/// - Files are copied in [`CHUNK_BYTES`] chunks; after each one `cancel` is
///   checked, `progress(read, total)` runs over the parts' total size, and
///   `after_chunk` runs (tests use it to remove the drive).
/// - Every part's size and MD5 is checked, including the last part's
///   padding after the archive's end.
pub fn restore_tree(
    root: &Path,
    parts: &[PartSource],
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    after_chunk: &mut dyn FnMut() -> io::Result<()>,
) -> Result<(), RestoreError> {
    match fs::read_dir(dest) {
        Ok(mut existing) => {
            if existing.next().is_some() {
                return Err(RestoreError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} isn't empty", dest.display()),
                )));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(RestoreError::Io(error)),
    }
    fs::create_dir_all(dest).map_err(RestoreError::Io)?;
    let result = restore_into(root, parts, dest, cancel, progress, after_chunk);
    if result.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    result
}

fn restore_into(
    root: &Path,
    parts: &[PartSource],
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    after_chunk: &mut dyn FnMut() -> io::Result<()>,
) -> Result<(), RestoreError> {
    let total: u64 = parts.iter().map(|part| part.size).sum();
    let reader = TarPartReader::new(root, parts);
    let state = reader.state();
    let hook_error = |error: io::Error| {
        if root.is_dir() {
            RestoreError::Io(error)
        } else {
            RestoreError::DriveRemoved
        }
    };
    let mut archive = tar::Archive::new(reader);
    let mut buf = vec![0u8; CHUNK_BYTES];
    let mut folders: Vec<(PathBuf, u32)> = Vec::new();
    let entries = archive.entries().map_err(|error| state.error(error))?;
    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return Err(RestoreError::Cancelled);
        }
        let mut entry = entry.map_err(|error| state.error(error))?;
        let raw = entry.path_bytes().into_owned();
        let rel = safe_rel(&raw)?;
        let kind = entry.header().entry_type();
        let mode = entry.header().mode().unwrap_or(0) & 0o777;
        let path = dest.join(&rel);
        let shown = String::from_utf8_lossy(&raw);
        match kind {
            tar::EntryType::Directory => {
                fs::create_dir_all(&path).map_err(RestoreError::Io)?;
                folders.push((path, mode));
            }
            tar::EntryType::Regular => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(RestoreError::Io)?;
                }
                let mut out = File::create(&path).map_err(RestoreError::Io)?;
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(RestoreError::Cancelled);
                    }
                    let n = match entry.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(state.error(error)),
                    };
                    out.write_all(&buf[..n]).map_err(RestoreError::Io)?;
                    progress(state.bytes().min(total), total);
                    after_chunk().map_err(hook_error)?;
                }
                drop(out);
                set_mode(&path, mode).map_err(RestoreError::Io)?;
            }
            tar::EntryType::Symlink | tar::EntryType::Link => {
                return Err(RestoreError::Unsafe(format!(
                    "archive contains a link: {shown}"
                )));
            }
            _ => {
                return Err(RestoreError::Unsafe(format!(
                    "archive contains a special file: {shown}"
                )));
            }
        }
    }
    // Read to the end, so the last part's size and MD5 are checked too.
    let mut reader = archive.into_inner();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(RestoreError::Cancelled);
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(state.error(error)),
        }
    }
    // Deepest first, so a read-only folder is set after what it holds.
    for (path, mode) in folders.iter().rev() {
        set_mode(path, *mode).map_err(RestoreError::Io)?;
    }
    progress(total, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::pack::{new_pack, pack_dir, write_pack_atomic};

    type Hook = Box<dyn Fn(&Path)>;

    thread_local! {
        /// Runs on each synced part, before its read-back.
        static AFTER_PART_SYNC: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    /// The test hook [`TarPartWriter::finish_part`] calls between the sync
    /// and the read-back, on this thread only.
    pub(super) fn after_part_sync(tmp: &Path) {
        AFTER_PART_SYNC.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(tmp);
            }
        });
    }

    fn set_after_part_sync(hook: impl Fn(&Path) + 'static) {
        AFTER_PART_SYNC.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
    }

    fn clear_after_part_sync() {
        AFTER_PART_SYNC.with(|slot| *slot.borrow_mut() = None);
    }

    const BASE: &str = "games/ps4/Velvet Static";
    const SMALL: u64 = 4096;

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

    fn bytes(len: usize, seed: usize) -> Vec<u8> {
        (0..len).map(|i| ((i + seed) % 251) as u8).collect()
    }

    /// A game tree with nested and empty folders, a long name, an empty
    /// file, a unicode name and an executable.
    fn game_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::write(root.join("a/one.bin"), bytes(5000, 1)).unwrap();
        fs::write(root.join("a/b/eboot.bin"), bytes(3000, 2)).unwrap();
        fs::write(root.join(format!("{}.dat", "n".repeat(150))), bytes(700, 3)).unwrap();
        fs::write(root.join("zero.txt"), b"").unwrap();
        fs::write(root.join("Écran ✓.txt"), bytes(64, 4)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                root.join("a/b/eboot.bin"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            fs::set_permissions(root.join("zero.txt"), fs::Permissions::from_mode(0o640)).unwrap();
        }
        dir
    }

    #[cfg(unix)]
    fn mode_of(meta: &fs::Metadata) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o777
    }

    /// The tree at `root` as the backup scans it.
    fn scan(root: &Path) -> TreePlan {
        let source = local::LocalSource {
            layout: crate::backup::pack::InstalledLayout::Extracted,
            root: root.to_path_buf(),
            name: root.file_name().unwrap().to_string_lossy().into_owned(),
            excludes: Vec::new(),
            launch: String::new(),
        };
        local::scan_tree(&source, false).unwrap()
    }

    /// A hand-made plan holding `entries`.
    fn plan_of(entries: Vec<TreeEntry>, archive_bytes: u64) -> TreePlan {
        TreePlan {
            root: PathBuf::from("/nonexistent"),
            layout: crate::backup::pack::InstalledLayout::Extracted,
            excludes: Vec::new(),
            entries,
            archive_bytes,
            tree_bytes: 0,
            tree_files: 0,
        }
    }

    /// Every file under `dir`, relative, `/`-separated.
    fn files_under(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if !dir.exists() {
            return out;
        }
        let mut stack = vec![dir.to_path_buf()];
        while let Some(next) = stack.pop() {
            for entry in fs::read_dir(&next).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path.strip_prefix(dir).unwrap().to_string_lossy();
                    out.push(rel.replace('\\', "/"));
                }
            }
        }
        out.sort();
        out
    }

    fn write_small(
        mount: &Path,
        tree: &TreePlan,
        cancel: &AtomicBool,
    ) -> Result<Vec<PackPart>, WriteError> {
        write_installed_tree(&target(mount), tree, BASE, SMALL, cancel, &mut |_, _| {})
    }

    fn sources(mount: &Path, parts: &[PackPart]) -> Vec<PartSource> {
        parts
            .iter()
            .map(|part| PartSource {
                path: pack::resolve(mount, &part.path).unwrap(),
                size: part.size,
                md5: part.md5.clone(),
            })
            .collect()
    }

    fn restore(
        mount: &Path,
        parts: &[PartSource],
        dest: &Path,
        cancel: &AtomicBool,
        after_chunk: &mut dyn FnMut() -> io::Result<()>,
    ) -> Result<(), RestoreError> {
        restore_tree(
            &pack_dir(mount),
            parts,
            dest,
            cancel,
            &mut |_, _| {},
            after_chunk,
        )
    }

    fn md5_hex(data: &[u8]) -> String {
        hex(Md5::digest(data).into())
    }

    #[test]
    fn parts_break_at_the_boundary_with_an_md5_per_part() {
        let drive = drive();
        let game = game_tree();
        let tree = scan(game.path());
        let mut reports = Vec::new();
        let parts = write_installed_tree(
            &target(drive.path()),
            &tree,
            BASE,
            SMALL,
            &AtomicBool::new(false),
            &mut |done, total| reports.push((done, total)),
        )
        .unwrap();

        let expected_parts = tree.archive_bytes.div_ceil(SMALL) as usize;
        assert!(expected_parts >= 3, "{expected_parts}");
        assert_eq!(parts.len(), expected_parts);
        let mut joined = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            assert_eq!(part.path, pack::installed_part_rel(BASE, index));
            assert_eq!(part.path, format!("{BASE}/installed.tar.{:03}", index + 1));
            let data = fs::read(pack::resolve(drive.path(), &part.path).unwrap()).unwrap();
            assert_eq!(data.len() as u64, part.size);
            if index + 1 < parts.len() {
                assert_eq!(part.size, SMALL);
            }
            assert_eq!(part.md5, md5_hex(&data));
            joined.extend_from_slice(&data);
        }
        assert_eq!(joined.len() as u64, tree.archive_bytes);
        // No temporary names are left.
        let dir = pack::resolve(drive.path(), BASE).unwrap();
        assert!(files_under(&dir)
            .iter()
            .all(|name| !name.ends_with(".gridtmp")));

        // The joined parts are an ordinary tar of the tree.
        let out = tempfile::tempdir().unwrap();
        tar::Archive::new(joined.as_slice())
            .unpack(out.path())
            .unwrap();
        assert_eq!(files_under(out.path()), files_under(game.path()));
        assert!(out.path().join("empty").is_dir());

        // Progress runs over the write and the read-back, up to the end.
        let total = tree.archive_bytes * 2;
        assert!(reports.iter().all(|&(_, t)| t == total));
        assert!(reports.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        assert_eq!(reports.last(), Some(&(total, total)));
    }

    #[test]
    fn the_override_sets_the_part_size() {
        set_part_bytes_override(Some(SMALL));
        assert_eq!(part_bytes(), SMALL);
        set_part_bytes_override(None);
        assert_eq!(part_bytes(), PART_BYTES);
    }

    #[test]
    fn a_cancel_removes_the_temporary_parts() {
        let drive = drive();
        let game = game_tree();
        let tree = scan(game.path());
        let cancel = AtomicBool::new(false);
        let error = write_installed_tree(
            &target(drive.path()),
            &tree,
            BASE,
            SMALL,
            &cancel,
            &mut |done, _| {
                // Once the first part is written and checked.
                if done >= SMALL * 2 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        // Progress is reported at part ends and every chunk; with small
        // parts a part end comes first.
        let error = error.unwrap_err();
        assert!(matches!(error, WriteError::Cancelled), "{error:?}");
        let dir = pack::resolve(drive.path(), BASE).unwrap();
        assert_eq!(files_under(&dir), Vec::<String>::new());
    }

    #[test]
    fn a_part_damaged_after_its_sync_is_caught() {
        let drive = drive();
        let game = game_tree();
        let tree = scan(game.path());
        set_after_part_sync(|tmp| {
            if tmp.to_string_lossy().ends_with(".002.gridtmp") {
                let mut data = fs::read(tmp).unwrap();
                data[100] ^= 0xff;
                fs::write(tmp, data).unwrap();
            }
        });
        let error = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap_err();
        clear_after_part_sync();
        assert!(matches!(error, WriteError::Mismatch(_)), "{error:?}");
        assert_eq!(
            error.to_string(),
            "installed.tar.002 didn't copy correctly: the copy on the drive doesn't match what GRID wrote"
        );
        let dir = pack::resolve(drive.path(), BASE).unwrap();
        assert_eq!(files_under(&dir), Vec::<String>::new());
    }

    #[test]
    fn a_file_changed_since_the_scan_gives_source_changed() {
        let drive = drive();
        let game = game_tree();
        let tree = scan(game.path());
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(game.path().join("a/one.bin"))
            .unwrap();
        file.write_all(b"a save written mid-backup").unwrap();
        drop(file);
        let error = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap_err();
        assert!(
            matches!(&error, WriteError::SourceChanged(name) if name == "a/one.bin"),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "a/one.bin changed while it was being copied; close the game and backup again"
        );
        let dir = pack::resolve(drive.path(), BASE).unwrap();
        assert_eq!(files_under(&dir), Vec::<String>::new());

        // A file removed since the scan is the same.
        fs::remove_file(game.path().join("zero.txt")).unwrap();
        let mut tree = scan(game.path());
        tree.entries.push(TreeEntry {
            path: game.path().join("zero.txt"),
            name: "zero.txt".into(),
            kind: TreeEntryKind::File,
            size: 0,
            mode: 0o644,
            mtime: 0,
        });
        let error = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap_err();
        assert!(
            matches!(&error, WriteError::SourceChanged(name) if name == "zero.txt"),
            "{error:?}"
        );
    }

    /// A file that grows or shrinks while the builder reads it.
    #[test]
    fn the_source_reader_refuses_a_length_other_than_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        fs::write(&path, bytes(10, 0)).unwrap();
        for left in [5, 20] {
            let changed = Cell::new(false);
            let mut reader = SourceReader {
                file: File::open(&path).unwrap(),
                left,
                changed: &changed,
            };
            assert!(reader.read_to_end(&mut Vec::new()).is_err());
            assert!(changed.get(), "{left}");
        }
        let changed = Cell::new(false);
        let mut reader = SourceReader {
            file: File::open(&path).unwrap(),
            left: 10,
            changed: &changed,
        };
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, bytes(10, 0));
        assert!(!changed.get());
    }

    #[test]
    fn an_unplain_entry_path_is_refused_before_writing() {
        let drive = drive();
        let cases = [
            ("../x", TreeEntryKind::File),
            ("/abs", TreeEntryKind::File),
            ("a//b", TreeEntryKind::File),
            ("a/./b", TreeEntryKind::File),
            ("", TreeEntryKind::File),
            ("dir/", TreeEntryKind::File),
            ("/", TreeEntryKind::Dir),
            ("../", TreeEntryKind::Dir),
        ];
        for (rel, kind) in cases {
            let tree = plan_of(
                vec![TreeEntry {
                    path: PathBuf::from("/nonexistent"),
                    name: rel.into(),
                    kind,
                    size: 0,
                    mode: 0o644,
                    mtime: 0,
                }],
                2048,
            );
            let error = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap_err();
            assert!(matches!(error, WriteError::Refused(_)), "{rel}: {error:?}");
        }
        assert!(!pack_dir(drive.path()).join("games").exists());
    }

    #[test]
    fn restore_round_trips_the_tree_with_modes() {
        let drive = drive();
        let game = game_tree();
        let tree = scan(game.path());
        let parts = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap();
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        let mut reports = Vec::new();
        restore_tree(
            &pack_dir(drive.path()),
            &sources(drive.path(), &parts),
            &dest,
            &AtomicBool::new(false),
            &mut |done, total| reports.push((done, total)),
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(files_under(&dest), files_under(game.path()));
        for name in files_under(game.path()) {
            let original = game.path().join(&name);
            let restored = dest.join(&name);
            assert_eq!(fs::read(&original).unwrap(), fs::read(&restored).unwrap());
            #[cfg(unix)]
            assert_eq!(
                mode_of(&fs::metadata(&original).unwrap()),
                mode_of(&fs::metadata(&restored).unwrap()),
                "{name}"
            );
        }
        assert!(dest.join("empty").is_dir());
        let total = tree.archive_bytes;
        assert_eq!(reports.last(), Some(&(total, total)));
    }

    /// A tar made by hand, with raw names and entry types the writer never
    /// makes.
    fn raw_entry(
        builder: &mut tar::Builder<Vec<u8>>,
        name: &[u8],
        kind: tar::EntryType,
        mode: u32,
        data: &[u8],
    ) {
        let mut header = tar::Header::new_gnu();
        header.as_old_mut().name[..name.len()].copy_from_slice(name);
        header.set_entry_type(kind);
        header.set_mode(mode);
        header.set_size(data.len() as u64);
        if matches!(kind, tar::EntryType::Symlink | tar::EntryType::Link) {
            header.set_link_name("target.txt").unwrap();
        }
        header.set_cksum();
        builder.append(&header, data).unwrap();
    }

    /// Puts `archive` on the drive as one part and returns its source.
    fn one_part(mount: &Path, archive: &[u8]) -> Vec<PartSource> {
        let rel = format!("{BASE}/installed.tar.001");
        let path = pack::resolve(mount, &rel).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, archive).unwrap();
        vec![PartSource {
            path,
            size: archive.len() as u64,
            md5: md5_hex(archive),
        }]
    }

    #[test]
    fn restore_refuses_unsafe_entries() {
        use tar::EntryType;
        let cases: [(&[u8], EntryType, &str); 7] = [
            (b"../evil.txt", EntryType::Regular, "unsafe path"),
            (b"a/../../evil.txt", EntryType::Regular, "unsafe path"),
            (b"/tmp/evil.txt", EntryType::Regular, "unsafe path"),
            (b"link", EntryType::Symlink, "link"),
            (b"hard", EntryType::Link, "link"),
            (b"fifo", EntryType::Fifo, "special file"),
            (b"dev", EntryType::Char, "special file"),
        ];
        for (name, kind, why) in cases {
            let drive = drive();
            let mut builder = tar::Builder::new(Vec::new());
            raw_entry(&mut builder, b"ok.txt", EntryType::Regular, 0o644, b"fine");
            raw_entry(&mut builder, name, kind, 0o644, b"");
            let archive = builder.into_inner().unwrap();
            let parts = one_part(drive.path(), &archive);
            let out = tempfile::tempdir().unwrap();
            let dest = out.path().join("game").join("restored");
            let error = restore(
                drive.path(),
                &parts,
                &dest,
                &AtomicBool::new(false),
                &mut || Ok(()),
            )
            .unwrap_err();
            let shown = String::from_utf8_lossy(name);
            assert!(
                matches!(error, RestoreError::Unsafe(_)),
                "{shown}: {error:?}"
            );
            assert!(error.to_string().contains(why), "{shown}: {error}");
            assert!(!dest.exists(), "{shown}");
            assert!(!out.path().join("evil.txt").exists());
            assert!(!out.path().join("game/evil.txt").exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn restore_masks_modes_to_permission_bits() {
        let drive = drive();
        let mut builder = tar::Builder::new(Vec::new());
        raw_entry(
            &mut builder,
            b"run.sh",
            tar::EntryType::Regular,
            0o4755,
            b"#!/bin/sh",
        );
        let archive = builder.into_inner().unwrap();
        let parts = one_part(drive.path(), &archive);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        restore(
            drive.path(),
            &parts,
            &dest,
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(dest.join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o7777, 0o755);
    }

    fn written(drive: &Path) -> (tempfile::TempDir, Vec<PartSource>) {
        let game = game_tree();
        let tree = scan(game.path());
        let parts = write_small(drive, &tree, &AtomicBool::new(false)).unwrap();
        (game, sources(drive, &parts))
    }

    #[test]
    fn a_wrong_part_md5_fails_and_cleans_up() {
        let drive = drive();
        let (_game, mut parts) = written(drive.path());
        parts[1].md5 = "0".repeat(32);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        let error = restore(
            drive.path(),
            &parts,
            &dest,
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap_err();
        assert!(
            matches!(&error, RestoreError::PartMismatch(name) if name == "installed.tar.002"),
            "{error:?}"
        );
        assert!(!dest.exists());
    }

    /// The last part is checked too, though the archive ends before its
    /// padding.
    #[test]
    fn a_wrong_last_part_md5_fails() {
        let drive = drive();
        let (_game, mut parts) = written(drive.path());
        let last = parts.len() - 1;
        parts[last].md5 = "0".repeat(32);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        let error = restore(
            drive.path(),
            &parts,
            &dest,
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap_err();
        assert!(matches!(error, RestoreError::PartMismatch(_)), "{error:?}");
        assert!(!dest.exists());
    }

    #[test]
    fn a_missing_or_short_part_fails_and_cleans_up() {
        let first = drive();
        let (_game, parts) = written(first.path());
        fs::remove_file(&parts[1].path).unwrap();
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        let error = restore(
            first.path(),
            &parts,
            &dest,
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap_err();
        assert!(
            matches!(&error, RestoreError::Incomplete(name) if name == "installed.tar.002"),
            "{error:?}"
        );
        assert!(!dest.exists());

        let drive = drive();
        let (_game, parts) = written(drive.path());
        File::options()
            .write(true)
            .open(&parts[0].path)
            .unwrap()
            .set_len(SMALL - 1)
            .unwrap();
        let error = restore(
            drive.path(),
            &parts,
            &dest,
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap_err();
        assert!(
            matches!(&error, RestoreError::PartMismatch(name) if name == "installed.tar.001"),
            "{error:?}"
        );
        assert!(!dest.exists());
    }

    #[test]
    fn a_cancel_mid_restore_stops_and_cleans_up() {
        let drive = drive();
        let (_game, parts) = written(drive.path());
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        let cancel = AtomicBool::new(false);
        let mut chunks = 0;
        let error = restore(drive.path(), &parts, &dest, &cancel, &mut || {
            chunks += 1;
            cancel.store(true, Ordering::Relaxed);
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, RestoreError::Cancelled), "{error:?}");
        assert_eq!(chunks, 1);
        assert!(!dest.exists());
    }

    #[test]
    fn restore_refuses_a_folder_that_isnt_empty() {
        let drive = drive();
        let (_game, parts) = written(drive.path());
        let out = tempfile::tempdir().unwrap();
        fs::write(out.path().join("keep.txt"), b"keep").unwrap();
        let error = restore(
            drive.path(),
            &parts,
            out.path(),
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap_err();
        assert!(matches!(error, RestoreError::Io(_)), "{error:?}");
        assert!(out.path().join("keep.txt").exists());
    }

    /// The drive disappears mid-restore: the error says removed, and the
    /// half-restored folder is gone.
    #[cfg(unix)]
    #[test]
    fn a_drive_removed_mid_restore_reports_removed() {
        let outer = tempfile::tempdir().unwrap();
        let mount = outer.path().join("mnt");
        fs::create_dir(&mount).unwrap();
        write_pack_atomic(&mount, &new_pack("Test", &[])).unwrap();
        let (_game, parts) = written(&mount);
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        let error = restore(&mount, &parts, &dest, &AtomicBool::new(false), &mut || {
            let _ = fs::remove_dir_all(&mount);
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, RestoreError::DriveRemoved), "{error:?}");
        assert!(!dest.exists());
    }

    /// Every folder (`name/`) and file (with its bytes) under `dir`, sorted.
    fn snapshot(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(next) = stack.pop() {
            for entry in fs::read_dir(&next).unwrap() {
                let path = entry.unwrap().path();
                let rel = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/");
                if path.is_dir() {
                    out.push((format!("{rel}/"), None));
                    stack.push(path);
                } else {
                    out.push((rel, Some(fs::read(&path).unwrap())));
                }
            }
        }
        out.sort();
        out
    }

    /// The whole path: the scan's tree through the writer gives exactly
    /// `archive_bytes` across the parts, and the parts give the tree back,
    /// both through any tar reader and through [`restore_tree`].
    #[test]
    fn a_scanned_tree_writes_exactly_its_estimate_and_restores() {
        let drive = drive();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Velvet Static");
        let put = |rel: &str, len: usize, seed: usize| {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes(len, seed)).unwrap();
        };
        // Names around the 100-byte header field, files and folders.
        for (seed, len) in [99, 100, 101, 150].into_iter().enumerate() {
            put(&"f".repeat(len), 10 + seed, seed);
            fs::create_dir_all(root.join("d".repeat(len))).unwrap();
        }
        // Folder names of 99 and 100 bytes put the `/` at byte 100 and 101.
        put(&format!("{}/x.bin", "p".repeat(99)), 3, 9);
        // Short parts making a long path.
        put(&format!("{}leaf.dat", "deep/".repeat(25)), 700, 11);
        // Unicode, one name 100 bytes long and one 102.
        put("ünïcødé/日本語のファイル名.txt", 77, 12);
        put(&format!("ünïcødé/{}", "ü".repeat(50)), 5, 13);
        put(&format!("ünïcødé/{}", "ü".repeat(51)), 6, 14);
        // Files on and around the 512-byte block edges.
        for (seed, len) in [0, 1, 511, 512, 513, 1023, 1024, 1025, 10_000]
            .into_iter()
            .enumerate()
        {
            put(&format!("sizes/{len}.bin"), len, 20 + seed);
        }
        fs::create_dir_all(root.join("empty/nested empty")).unwrap();

        let tree = scan(&root);
        let parts = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap();
        assert!(parts.len() > 3, "{}", parts.len());
        let total: u64 = parts.iter().map(|part| part.size).sum();
        assert_eq!(total, tree.archive_bytes);

        let mut joined = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            assert_eq!(part.path, pack::installed_part_rel(BASE, index));
            joined.extend(fs::read(pack::resolve(drive.path(), &part.path).unwrap()).unwrap());
        }
        let expected = snapshot(&root);
        let unpacked = tempfile::tempdir().unwrap();
        tar::Archive::new(joined.as_slice())
            .unpack(unpacked.path())
            .unwrap();
        assert_eq!(snapshot(unpacked.path()), expected);

        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("restored");
        restore(
            drive.path(),
            &sources(drive.path(), &parts),
            &dest,
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(snapshot(&dest), expected);
    }

    /// The scan names folders with a trailing `/`; the archive keeps it and
    /// restore takes it.
    #[test]
    fn folder_names_keep_their_slash_and_restore() {
        let drive = drive();
        let game = game_tree();
        let tree = scan(game.path());
        assert!(tree
            .entries
            .iter()
            .filter(|e| e.kind == TreeEntryKind::Dir)
            .all(|e| e.name.ends_with('/')));
        let parts = write_small(drive.path(), &tree, &AtomicBool::new(false)).unwrap();
        let mut joined = Vec::new();
        for part in &parts {
            joined.extend(fs::read(pack::resolve(drive.path(), &part.path).unwrap()).unwrap());
        }
        let mut archive = tar::Archive::new(joined.as_slice());
        let dirs: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.header().entry_type().is_dir())
            .map(|e| String::from_utf8(e.path_bytes().into_owned()).unwrap())
            .collect();
        assert!(dirs.contains(&"a/b/".to_string()), "{dirs:?}");
        assert!(dirs.iter().all(|name| name.ends_with('/')), "{dirs:?}");
        assert!(safe_rel(b"a/b/").is_ok());
    }
}
