//! The byte source of an install from a backup drive: copies a game's files
//! off the drive into the library, with the same cumulative progress,
//! cancellation and skip rules as [`super::download::download_targets`].
//!
//! Only where the bytes come from differs from a server install. The plan,
//! the targets and the finalize route are the server install's own
//! (`docs/superpowers/plans/2026-11-09-game-backups.md`, Part 4).

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::LibraryError;

/// Bytes read and written per step. Cancel is checked once per chunk.
const CHUNK: usize = 1024 * 1024;
/// Minimum gap between progress emissions, other than the final one.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// One file to copy: its path on the drive, its destination in the library,
/// and its expected size in bytes (`0` when unknown).
pub type CopyFile = (PathBuf, PathBuf, u64);

/// Copies every file of `files` from the drive to its destination.
///
/// - `root` is the folder whose disappearance means the drive was removed
///   (the drive's `gridpack/` folder). An I/O error while it no longer
///   exists becomes [`LibraryError::DriveRemoved`] with "`label` was
///   removed".
/// - `on_progress` gets `(copied, total, avg_speed_bps)` at most every
///   100 ms, plus a final emission. `total` is the sum of the sizes.
/// - A destination that already exists with the expected size (> 0) is
///   skipped, and its bytes count toward progress at once.
/// - On cancel or any error, the CURRENT destination is deleted. Files
///   completed earlier stay.
pub fn copy_targets(
    root: &Path,
    label: &str,
    files: &[CopyFile],
    cancel: &AtomicBool,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
) -> Result<(), LibraryError> {
    copy_targets_with(root, label, files, cancel, on_progress, &mut || Ok(()))
}

/// [`copy_targets`] with a hook called after each chunk is written. Tests use
/// it to remove the drive in the middle of a copy.
fn copy_targets_with(
    root: &Path,
    label: &str,
    files: &[CopyFile],
    cancel: &AtomicBool,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
    after_chunk: &mut dyn FnMut() -> io::Result<()>,
) -> Result<(), LibraryError> {
    let start = Instant::now();
    let mut last_emit: Option<Instant> = None;
    let total: u64 = files.iter().map(|(_, _, size)| size).sum();
    let mut copied: u64 = 0;
    let mut buf = vec![0u8; CHUNK];

    for (src, dest, size) in files {
        if *size > 0 && fs::metadata(dest).is_ok_and(|meta| meta.len() == *size) {
            copied += size;
            continue;
        }
        let mut step = |bytes: u64| {
            copied += bytes;
            emit(&mut last_emit, start, copied, total, on_progress, false);
        };
        if let Err(error) = copy_one(src, dest, *size, cancel, &mut buf, &mut step, after_chunk) {
            let _ = fs::remove_file(dest);
            return Err(classify(error, root, label));
        }
    }

    emit(&mut last_emit, start, copied, total, on_progress, true);
    Ok(())
}

/// Why one file's copy stopped.
enum CopyError {
    Cancelled,
    Io(io::Error),
    /// The drive's file ended before its expected size.
    Short {
        name: String,
        got: u64,
        want: u64,
    },
}

impl From<io::Error> for CopyError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Copies `src` to `dest` in [`CHUNK`]-sized steps, calling `step` with the
/// bytes of each chunk. The handles are closed when this returns, so the
/// caller can delete `dest` and look at the drive.
fn copy_one(
    src: &Path,
    dest: &Path,
    size: u64,
    cancel: &AtomicBool,
    buf: &mut [u8],
    step: &mut dyn FnMut(u64),
    after_chunk: &mut dyn FnMut() -> io::Result<()>,
) -> Result<(), CopyError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CopyError::Cancelled);
    }
    let mut input = fs::File::open(src)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = fs::File::create(dest)?;
    let mut written: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CopyError::Cancelled);
        }
        let read = match input.read(buf) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        output.write_all(&buf[..read])?;
        written += read as u64;
        step(read as u64);
        after_chunk()?;
    }
    output.flush()?;
    if size > 0 && written != size {
        let name = src
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(CopyError::Short {
            name,
            got: written,
            want: size,
        });
    }
    Ok(())
}

/// The library error for a stopped copy. Any I/O error while `root` is gone
/// means the drive was removed.
fn classify(error: CopyError, root: &Path, label: &str) -> LibraryError {
    match error {
        CopyError::Cancelled => LibraryError::Cancelled,
        CopyError::Io(_) if fs::metadata(root).is_err() => {
            LibraryError::DriveRemoved(format!("{label} was removed"))
        }
        CopyError::Io(error) => LibraryError::Io(error),
        CopyError::Short { name, got, want } => LibraryError::Extract(format!(
            "The backup of {name} on {label} is incomplete ({got} of {want} bytes)"
        )),
    }
}

/// Calls `on_progress` when `force` is set, or when at least
/// [`PROGRESS_INTERVAL`] has passed since the last emission.
fn emit(
    last_emit: &mut Option<Instant>,
    start: Instant,
    copied: u64,
    total: u64,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
    force: bool,
) {
    let now = Instant::now();
    let due = force || last_emit.is_none_or(|last| now.duration_since(last) >= PROGRESS_INTERVAL);
    if !due {
        return;
    }
    *last_emit = Some(now);
    let elapsed = start.elapsed().as_secs_f64();
    let speed = if elapsed > 0.0 {
        copied as f64 / elapsed
    } else {
        0.0
    };
    on_progress(copied, total, speed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A drive at `<tmp>/mount` with a `gridpack/` folder: returns the
    /// tempdir, the root (`gridpack/`) and the library folder.
    fn drive() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mount").join("gridpack");
        fs::create_dir_all(&root).unwrap();
        let library = tmp.path().join("library");
        (tmp, root, library)
    }

    fn put(root: &Path, name: &str, len: usize) -> PathBuf {
        let path = root.join(name);
        let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        fs::write(&path, bytes).unwrap();
        path
    }

    /// Every `(copied, total)` progress emission, in order.
    type Seen = std::sync::Arc<Mutex<Vec<(u64, u64)>>>;

    fn recorder() -> (Seen, impl FnMut(u64, u64, f64) + Send) {
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (seen, move |copied, total, _| {
            sink.lock().unwrap().push((copied, total))
        })
    }

    #[test]
    fn copies_every_file_and_reports_cumulative_progress() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", CHUNK * 2 + 17);
        let b = put(&root, "b.bin", 5);
        let files = vec![
            (
                a.clone(),
                library.join("g").join("a.bin"),
                (CHUNK * 2 + 17) as u64,
            ),
            (b.clone(), library.join("g").join("b.bin"), 5),
        ];
        let (seen, mut on_progress) = recorder();
        copy_targets(
            &root,
            "T7",
            &files,
            &AtomicBool::new(false),
            &mut on_progress,
        )
        .unwrap();

        assert_eq!(
            fs::read(library.join("g/a.bin")).unwrap(),
            fs::read(&a).unwrap()
        );
        assert_eq!(
            fs::read(library.join("g/b.bin")).unwrap(),
            fs::read(&b).unwrap()
        );
        let seen = seen.lock().unwrap();
        let total = (CHUNK * 2 + 22) as u64;
        assert_eq!(*seen.last().unwrap(), (total, total));
        assert!(seen.iter().all(|&(_, t)| t == total));
        assert!(
            seen.windows(2).all(|w| w[0].0 <= w[1].0),
            "progress only grows"
        );
    }

    #[test]
    fn a_cancel_stops_the_copy_and_deletes_the_partial_file() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", CHUNK * 3);
        let dest = library.join("a.bin");
        let files = vec![(a, dest.clone(), (CHUNK * 3) as u64)];
        let cancel = AtomicBool::new(false);
        let (_, mut on_progress) = recorder();
        let mut hook = || {
            cancel.store(true, Ordering::Relaxed);
            Ok(())
        };
        let result = copy_targets_with(&root, "T7", &files, &cancel, &mut on_progress, &mut hook);

        assert!(matches!(result, Err(LibraryError::Cancelled)), "{result:?}");
        assert!(!dest.exists(), "the partial file is deleted");
    }

    #[test]
    fn an_existing_complete_file_is_skipped_but_counted() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", 10);
        let dest = library.join("a.bin");
        fs::create_dir_all(&library).unwrap();
        // Same size, different bytes: a skipped file is never read again.
        fs::write(&dest, [7u8; 10]).unwrap();
        let (seen, mut on_progress) = recorder();
        copy_targets(
            &root,
            "T7",
            &[(a, dest.clone(), 10)],
            &AtomicBool::new(false),
            &mut on_progress,
        )
        .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), [7u8; 10]);
        assert_eq!(*seen.lock().unwrap().last().unwrap(), (10, 10));
    }

    #[test]
    fn a_file_of_the_wrong_size_is_copied_again() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", 10);
        let dest = library.join("a.bin");
        fs::create_dir_all(&library).unwrap();
        fs::write(&dest, [7u8; 4]).unwrap();
        let (_, mut on_progress) = recorder();
        copy_targets(
            &root,
            "T7",
            &[(a.clone(), dest.clone(), 10)],
            &AtomicBool::new(false),
            &mut on_progress,
        )
        .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), fs::read(&a).unwrap());
    }

    #[test]
    fn a_source_root_that_is_gone_means_the_drive_was_removed() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", 10);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
        let (_, mut on_progress) = recorder();
        let result = copy_targets(
            &root,
            "SAMSUNG T7",
            &[(a, library.join("a.bin"), 10)],
            &AtomicBool::new(false),
            &mut on_progress,
        );

        match result {
            Err(LibraryError::DriveRemoved(message)) => {
                assert_eq!(message, "SAMSUNG T7 was removed")
            }
            other => panic!("expected DriveRemoved, got {other:?}"),
        }
    }

    /// The drive goes away in the middle of a file: the read fails (as it
    /// does when a USB drive is pulled) and the root is gone. Unix only:
    /// Windows refuses to delete a folder that holds an open file.
    #[cfg(unix)]
    #[test]
    fn a_drive_removed_mid_copy_fails_with_drive_removed_and_deletes_the_partial_file() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", CHUNK * 3);
        let dest = library.join("a.bin");
        let files = vec![(a, dest.clone(), (CHUNK * 3) as u64)];
        let mount = root.parent().unwrap().to_path_buf();
        let (_, mut on_progress) = recorder();
        let mut hook = || {
            fs::remove_dir_all(&mount)?;
            Err(io::Error::other("Input/output error"))
        };
        let result = copy_targets_with(
            &root,
            "SAMSUNG T7",
            &files,
            &AtomicBool::new(false),
            &mut on_progress,
            &mut hook,
        );

        assert!(
            matches!(&result, Err(LibraryError::DriveRemoved(m)) if m == "SAMSUNG T7 was removed"),
            "{result:?}"
        );
        assert!(!dest.exists(), "the partial file is deleted");
    }

    /// The drive goes away after the first file: opening the next one fails.
    #[cfg(unix)]
    #[test]
    fn a_drive_removed_between_files_fails_with_drive_removed() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", 10);
        let b = put(&root, "b.bin", 10);
        let mount = root.parent().unwrap().to_path_buf();
        let files = vec![
            (a, library.join("a.bin"), 10),
            (b, library.join("b.bin"), 10),
        ];
        let (_, mut on_progress) = recorder();
        let mut hook = || {
            let _ = fs::remove_dir_all(&mount);
            Ok(())
        };
        let result = copy_targets_with(
            &root,
            "T7",
            &files,
            &AtomicBool::new(false),
            &mut on_progress,
            &mut hook,
        );

        assert!(
            matches!(result, Err(LibraryError::DriveRemoved(_))),
            "{result:?}"
        );
        assert!(library.join("a.bin").exists(), "a finished file stays");
        assert!(!library.join("b.bin").exists());
    }

    #[test]
    fn an_io_error_with_the_drive_present_stays_an_io_error() {
        let (_tmp, root, library) = drive();
        let missing = root.join("missing.bin");
        let (_, mut on_progress) = recorder();
        let result = copy_targets(
            &root,
            "T7",
            &[(missing, library.join("x.bin"), 10)],
            &AtomicBool::new(false),
            &mut on_progress,
        );

        assert!(matches!(result, Err(LibraryError::Io(_))), "{result:?}");
    }

    #[test]
    fn a_short_file_on_the_drive_is_an_error() {
        let (_tmp, root, library) = drive();
        let a = put(&root, "a.bin", 4);
        let dest = library.join("a.bin");
        let (_, mut on_progress) = recorder();
        let result = copy_targets(
            &root,
            "T7",
            &[(a, dest.clone(), 10)],
            &AtomicBool::new(false),
            &mut on_progress,
        );

        assert!(
            matches!(result, Err(LibraryError::Extract(_))),
            "{result:?}"
        );
        assert!(!dest.exists());
    }
}
