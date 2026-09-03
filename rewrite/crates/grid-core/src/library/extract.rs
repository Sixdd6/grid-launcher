//! Archive extraction engine: decides whether an archive should be
//! extracted for a given platform, then extracts zip/tar/7z archives into a
//! destination directory with a traversal guard. See
//! `docs/porting/03-library-install.md` §3-§5 for the Python behavior this
//! is scoped from (this port narrows the platform-predicate and
//! progress-accounting rules to the subset fixed by the task brief).

use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use super::platforms::{is_native_platform, is_ps3_platform};
use super::LibraryError;

// --- Platform predicates and extraction decision --------------------------

/// Whether `platform` names an arcade-style platform, whose archives are the
/// ROM itself and must never be extracted.
pub fn is_arcade_platform(platform: &str) -> bool {
    let lower = platform.to_lowercase();
    ["arcade", "mame", "fbneo", "final burn"]
        .iter()
        .any(|needle| lower.contains(needle))
}

/// Suffixes (without the dot, lowercase) that this engine knows how to
/// extract, for any platform that is neither native, arcade, nor
/// PlayStation 3.
const EXTRACTABLE_SUFFIXES: &[&str] = &["7z", "zip", "rar", "tar", "gz", "bz2", "xz"];

/// Suffixes (without the dot, lowercase) extracted for a PlayStation 3
/// archive specifically (Python `should_extract`'s PS3 branch).
const PS3_SUFFIXES: &[&str] = &["zip", "7z", "rar", "tar", "gz", "bz2", "xz"];

/// Whether `archive` should be extracted for `platform`, mirroring the
/// Python `should_extract` table (`docs/porting/03-library-install.md`):
/// a native (Windows) platform's archive is always extracted; an arcade
/// platform's archive is never extracted (the archive is the ROM itself);
/// a PlayStation 3 archive is extracted only when its suffix is one of
/// `PS3_SUFFIXES`; every other platform's archive is extracted only when
/// its suffix is one of `EXTRACTABLE_SUFFIXES` (both sets are
/// case-insensitive and now include `rar`, extracted by this engine via
/// `unrar`).
pub fn should_extract(platform: &str, archive: &Path) -> bool {
    if is_native_platform(platform) {
        return true;
    }
    if is_arcade_platform(platform) {
        return false;
    }
    let Some(suffix) = lowercase_suffix(archive) else {
        return false;
    };
    if is_ps3_platform(platform) {
        PS3_SUFFIXES.contains(&suffix.as_str())
    } else {
        EXTRACTABLE_SUFFIXES.contains(&suffix.as_str())
    }
}

/// Whether this engine can extract `archive` judging by its suffix alone,
/// with no platform rule applied.
///
/// D13: [`should_extract`]'s table says a native (Windows) platform's
/// payload is ALWAYS extracted, but the server also serves bare disc images
/// and loose executables under that platform, and neither is an archive.
/// The native finalize checks this on top of `should_extract` so such a
/// payload installs as a direct file instead of failing extraction.
pub fn is_extractable_archive(archive: &Path) -> bool {
    lowercase_suffix(archive).is_some_and(|suffix| EXTRACTABLE_SUFFIXES.contains(&suffix.as_str()))
}

/// `path`'s extension, lowercased, without the leading dot.
fn lowercase_suffix(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
}

// --- Public extraction entry point -----------------------------------------

/// Progress callback: `(processed_bytes, total_bytes)`. `total` is `0` when
/// the format cannot cheaply compute an upfront total (tar); callers should
/// treat that as "unknown" rather than "already done".
pub type ExtractProgress<'a> = &'a mut dyn FnMut(u64, u64);

/// Extracts `archive` into `dest`.
///
/// `dest` is wiped and recreated first, so a partially extracted directory
/// from a previous attempt never leaks into the new one. On any failure,
/// `dest` is deleted entirely before the error is returned. Blocking —
/// callers on an async runtime must wrap this in `spawn_blocking`.
pub fn extract_archive(
    archive: &Path,
    dest: &Path,
    progress: ExtractProgress,
) -> Result<(), LibraryError> {
    wipe_and_recreate(dest)?;
    match dispatch(archive, dest, progress) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_dir_all(dest);
            Err(err)
        }
    }
}

/// Removes `dest` (file or directory tree) if present, then recreates it as
/// an empty directory.
fn wipe_and_recreate(dest: &Path) -> Result<(), LibraryError> {
    if dest.is_dir() {
        fs::remove_dir_all(dest).map_err(|e| {
            LibraryError::Extract(format!(
                "failed to clear extraction directory {}: {e}",
                dest.display()
            ))
        })?;
    } else if dest.exists() {
        fs::remove_file(dest).map_err(|e| {
            LibraryError::Extract(format!(
                "failed to clear extraction target {}: {e}",
                dest.display()
            ))
        })?;
    }
    fs::create_dir_all(dest).map_err(|e| {
        LibraryError::Extract(format!(
            "failed to create extraction directory {}: {e}",
            dest.display()
        ))
    })
}

/// Dispatches to the right extractor by suffix (`.7z`) then content
/// signature (zip magic bytes), falling back to tar (which sniffs its own
/// gzip/bzip2/xz wrapping).
fn dispatch(archive: &Path, dest: &Path, progress: ExtractProgress) -> Result<(), LibraryError> {
    if lowercase_suffix(archive).as_deref() == Some("rar") {
        return extract_rar(archive, dest, progress);
    }
    if lowercase_suffix(archive).as_deref() == Some("7z") {
        return extract_7z(archive, dest, progress);
    }
    if is_zip_signature(archive)? {
        return extract_zip(archive, dest, progress);
    }
    extract_tar(archive, dest, progress)
}

/// Reads the first bytes of `archive` and checks them against the zip local
/// file header / empty / spanned signatures. Content-based, not
/// suffix-based, so a `.tar.gz` that is really a zip is read as one.
fn is_zip_signature(archive: &Path) -> Result<bool, LibraryError> {
    let mut file = fs::File::open(archive)
        .map_err(|e| LibraryError::Extract(format!("failed to open archive: {e}")))?;
    let mut magic = [0u8; 4];
    let n = file
        .read(&mut magic)
        .map_err(|e| LibraryError::Extract(format!("failed to read archive: {e}")))?;
    let magic = &magic[..n];
    Ok(magic.starts_with(&[0x50, 0x4B, 0x03, 0x04])
        || magic.starts_with(&[0x50, 0x4B, 0x05, 0x06])
        || magic.starts_with(&[0x50, 0x4B, 0x07, 0x08]))
}

/// Builds the unsafe-path error text shared by every format's traversal
/// guard.
fn unsafe_path_error(raw_name: &str) -> LibraryError {
    LibraryError::Extract(format!("archive contains an unsafe path: {raw_name}"))
}

/// Whether `raw_name` (the untrusted entry name, before any format-specific
/// parsing), normalized `\` → `/`, is an absolute path.
///
/// None of `zip`'s `enclosed_name()` or `tar`'s `unpack_in` reject a
/// *purely* absolute entry on their own: both silently strip the leading
/// root and treat what remains as relative under the destination (zip only
/// rejects a root/prefix component when it appears after a `..` has
/// already popped the stack empty; tar's `unpack_in` explicitly documents
/// leading `/`s as "just ignored, treated as empty components"). Our
/// traversal policy treats an absolute path as unconditionally hostile, so
/// every format checks the raw name for this explicitly, alongside
/// whatever `..`-escape guard the format's own crate provides.
fn is_absolute_entry_path(raw_name: &str) -> bool {
    let normalized = raw_name.replace('\\', "/");
    Path::new(&normalized).is_absolute()
}

// --- ZIP --------------------------------------------------------------------

/// Masks a raw Unix mode value, as returned by `ZipFile::unix_mode()`, down
/// to permission bits only.
///
/// SECURITY: never propagate setuid (`0o4000`), setgid (`0o2000`), or
/// sticky (`0o1000`) bits recovered from an untrusted archive.
///
/// Extracted as its own pure function because `zip` 8.6.0's own writer
/// already masks to `mode & 0o777` at write time
/// (`SimpleFileOptions::unix_permissions`, see `write.rs:573-576` in the
/// crate source) — a fixture built with this crate's writer can therefore
/// never carry a real setuid/setgid/sticky bit, so this function's masking
/// is unit-tested directly against raw values instead (the shape a
/// genuinely hostile archive, or one written by a different tool, could
/// contain).
#[cfg(unix)]
fn mask_zip_unix_mode(mode: u32) -> u32 {
    mode & 0o777
}

fn extract_zip(archive: &Path, dest: &Path, progress: ExtractProgress) -> Result<(), LibraryError> {
    let file = fs::File::open(archive)
        .map_err(|e| LibraryError::Extract(format!("failed to open archive: {e}")))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;

    let mut total: u64 = 0;
    for i in 0..zip.len() {
        if let Ok(entry) = zip.by_index(i) {
            if !entry.is_dir() {
                total += entry.size();
            }
        }
    }

    let mut processed: u64 = 0;
    progress(0, total);

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;
        let raw_name = entry.name().to_string();
        // `enclosed_name()` treats both `/` and `\` as separators and
        // rejects a path that would resolve above the extraction root
        // (any `..` that pops the stack empty), satisfying the backslash
        // normalization + `..`-escape guard in one call. It does NOT
        // reject a purely absolute entry (it just strips the leading
        // root), so that is checked explicitly first.
        if is_absolute_entry_path(&raw_name) {
            return Err(unsafe_path_error(&raw_name));
        }
        let Some(relative) = entry.enclosed_name() else {
            return Err(unsafe_path_error(&raw_name));
        };
        let out_path = dest.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;
            }
            let mut out_file = fs::File::create(&out_path)
                .map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;
            io::copy(&mut entry, &mut out_file)
                .map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;
            processed += entry.size();

            // Preserve the stored Unix permission bits, when present.
            // `ZipFile::unix_mode()` returns `None` only when the entry's
            // external attributes are exactly zero; it is `Some(mode)` both
            // for a genuine Unix-origin entry AND for a Dos-origin entry
            // with nonzero (but unix-shaped-zero-high-word) attributes, in
            // which case the crate itself *synthesizes* a mode from the
            // MS-DOS directory/read-only attribute bits (`S_IFREG | 0o664`,
            // stripped to `0o444` when the DOS read-only bit is set) — see
            // `ZipFileData::unix_mode` in the `zip` crate. This crate does
            // not expose the entry's creator system on read, so there is no
            // way to apply modes for Unix-origin entries only; we accept
            // the synthesized DOS-origin mode too, matching what `unzip`
            // itself does (honoring DOS read-only is safe here: this
            // engine's delete/overwrite paths tolerate a `0o444` file on
            // unix — directory permissions, not file permissions, govern
            // removal).
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    &out_path,
                    fs::Permissions::from_mode(mask_zip_unix_mode(mode)),
                )
                .map_err(|e| LibraryError::Extract(format!("zip: {e}")))?;
            }
        }
        progress(processed, total);
    }

    Ok(())
}

// --- RAR ---------------------------------------------------------------------

/// Converts a RAR entry's raw name into a path relative to the extraction
/// root, normalizing `\` to `/` (RAR entries commonly use Windows
/// separators) and rejecting anything the traversal guard treats as
/// hostile: an absolute path (`is_absolute_entry_path`, shared with every
/// other format), a `..` component, or a NUL byte. The NUL check exists
/// because `unrar`'s own `FileHeader::extract_to` panics if the
/// destination path it is handed contains one; every path this function
/// returns is later joined onto `dest` and passed straight to
/// `extract_to`, so a NUL in the raw entry name must never survive this
/// function.
pub(crate) fn rar_entry_relative_path(raw: &str) -> Result<PathBuf, LibraryError> {
    if raw.contains('\0') {
        return Err(unsafe_path_error(raw));
    }
    if is_absolute_entry_path(raw) {
        return Err(unsafe_path_error(raw));
    }
    let normalized = raw.replace('\\', "/");
    let path = Path::new(&normalized);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(unsafe_path_error(raw));
    }
    Ok(path.to_path_buf())
}

/// Whether a RAR entry's `file_attr` describes a symbolic link.
///
/// `unrar` 0.5.8's `FileHeader` exposes no `is_symlink()`; it exposes the
/// raw `file_attr` word, which for a Unix-host entry is the `st_mode` value
/// verbatim. This is the same test the bundled unrar C++ makes
/// (`IsLink(uint Attr)` in `filefn.cpp`: `(Attr & 0xF000) == 0xA000`, i.e.
/// `S_IFMT`/`S_IFLNK`).
///
/// SECURITY: unrar's extraction path materializes a real symlink for such
/// an entry, and its payload is the link target. A link planted inside the
/// destination is something a later entry can then be written *through*,
/// which escapes a traversal guard that only inspects entry names. Nothing
/// we install needs a link, so link entries are skipped outright. (The other
/// readers cannot plant one: the `zip` crate writes a link entry's target
/// path out as a plain file, and `tar`'s `unpack_in` applies its own
/// containment check.) A Windows-host entry carries Windows attribute bits
/// instead, and no combination of those sets `0xA000`, so this never
/// misfires on one.
fn rar_entry_is_symlink(file_attr: u32) -> bool {
    file_attr & 0xF000 == 0xA000
}

/// Extracts a RAR archive via the vendored `unrar` library (there is no
/// pure-Rust RAR decoder; RAR's format is proprietary and not documented
/// well enough to reimplement safely). Two passes: `open_for_listing`
/// first, to sum every file entry's `unpacked_size` into `total` for the
/// progress callback (listing does not touch payload bytes, so this is
/// cheap); then `open_for_processing`, which walks the header/payload
/// cursor pair the crate exposes and either creates a directory or
/// extracts a file for each entry.
fn extract_rar(archive: &Path, dest: &Path, progress: ExtractProgress) -> Result<(), LibraryError> {
    let listing = unrar::Archive::new(archive)
        .open_for_listing()
        .map_err(|e| LibraryError::Extract(e.to_string()))?;
    let mut total: u64 = 0;
    for entry in listing {
        let entry = entry.map_err(|e| LibraryError::Extract(e.to_string()))?;
        if entry.is_file() {
            total += entry.unpacked_size;
        }
    }
    progress(0, total);

    let mut cursor = unrar::Archive::new(archive)
        .open_for_processing()
        .map_err(|e| LibraryError::Extract(e.to_string()))?;
    let mut processed: u64 = 0;
    while let Some(header) = cursor
        .read_header()
        .map_err(|e| LibraryError::Extract(e.to_string()))?
    {
        let entry = header.entry();
        let raw_name = entry.filename.to_string_lossy().into_owned();
        let is_directory = entry.is_directory();
        let is_symlink = rar_entry_is_symlink(entry.file_attr);
        let size = entry.unpacked_size;
        let relative = rar_entry_relative_path(&raw_name)?;
        let out_path = dest.join(&relative);

        cursor = if is_symlink {
            // SECURITY: never materialize a link out of an untrusted
            // archive — see `rar_entry_is_symlink`. Skipped, not an error:
            // the rest of the archive still extracts. The skipped entry
            // still counts toward `processed`, since the listing pass
            // counted it toward `total`.
            processed += size;
            progress(processed, total);
            header
                .skip()
                .map_err(|e| LibraryError::Extract(e.to_string()))?
        } else if is_directory {
            fs::create_dir_all(&out_path)
                .map_err(|e| LibraryError::Extract(format!("rar: {e}")))?;
            header
                .skip()
                .map_err(|e| LibraryError::Extract(e.to_string()))?
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| LibraryError::Extract(format!("rar: {e}")))?;
            }
            let next = header
                .extract_to(&out_path)
                .map_err(|e| LibraryError::Extract(e.to_string()))?;
            processed += size;
            progress(processed, total);
            next
        };
    }

    Ok(())
}

// --- tar (plain, .gz, .bz2, .xz) --------------------------------------------

fn extract_tar(archive: &Path, dest: &Path, progress: ExtractProgress) -> Result<(), LibraryError> {
    let mut file = fs::File::open(archive)
        .map_err(|e| LibraryError::Extract(format!("failed to open archive: {e}")))?;
    let mut magic = [0u8; 6];
    let n = file
        .read(&mut magic)
        .map_err(|e| LibraryError::Extract(format!("failed to read archive: {e}")))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|e| LibraryError::Extract(format!("failed to read archive: {e}")))?;
    let magic = &magic[..n];

    let reader: Box<dyn Read> = if magic.starts_with(&[0x1F, 0x8B]) {
        Box::new(flate2::read::GzDecoder::new(file))
    } else if magic.starts_with(b"BZh") {
        Box::new(bzip2::read::BzDecoder::new(file))
    } else if magic.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        Box::new(liblzma::read::XzDecoder::new(file))
    } else {
        Box::new(file)
    };

    let mut archive_reader = tar::Archive::new(reader);
    let mut processed: u64 = 0;
    // No metadata pre-pass: total is always reported as 0 (unknown) for
    // tar, per the task brief.
    progress(0, 0);

    let entries = archive_reader
        .entries()
        .map_err(|e| LibraryError::Extract(format!("tar: {e}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| LibraryError::Extract(format!("tar: {e}")))?;
        let size = entry.header().size().unwrap_or(0);
        let display_name = entry
            .path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        // `unpack_in`'s own guard only rejects a `..` component; a purely
        // absolute entry name is treated as "just ignored, empty
        // components" and silently rebased under `dest`. Check the raw
        // name explicitly before ever calling into it.
        if is_absolute_entry_path(&display_name) {
            return Err(unsafe_path_error(&display_name));
        }

        let extracted = entry
            .unpack_in(dest)
            .map_err(|e| LibraryError::Extract(format!("tar: {e}")))?;
        if !extracted {
            return Err(unsafe_path_error(&display_name));
        }

        processed += size;
        progress(processed, 0);
    }

    Ok(())
}

// --- 7z ----------------------------------------------------------------------

/// Distinguishes a traversal rejection (never handed to the system
/// fallback — the archive is hostile, not merely unreadable by this crate)
/// from any other failure of the pure-Rust extractor (which does fall
/// back).
enum SevenZFailure {
    UnsafePath(String),
    Other(String),
}

fn extract_7z(archive: &Path, dest: &Path, progress: ExtractProgress) -> Result<(), LibraryError> {
    match extract_7z_pure(archive, dest, progress) {
        Ok(()) => Ok(()),
        Err(SevenZFailure::UnsafePath(message)) => Err(LibraryError::Extract(message)),
        Err(SevenZFailure::Other(primary)) => match extract_7z_system_fallback(archive, dest) {
            Ok(()) => Ok(()),
            Err(fallback) => Err(LibraryError::Extract(format!(
                "{primary}; system 7-Zip fallback also failed: {fallback}"
            ))),
        },
    }
}

/// Derives the Unix permission bits stored in a 7z entry's Windows
/// attribute field, if any.
///
/// 7z stores Unix permissions by setting the `0x8000` "Unix extension" flag
/// in the (32-bit) attribute field and packing the mode into its upper 16
/// bits (`attrs >> 16`) — 7-Zip's own convention for round-tripping POSIX
/// permissions through what is otherwise a Windows-attribute field. Returns
/// `None` when the entry carries no Windows attributes at all, or when the
/// `0x8000` flag is absent (a Windows-built archive with no Unix mode to
/// recover).
///
/// SECURITY: the returned mode is always masked to `0o777`; callers must
/// not apply setuid (`0o4000`), setgid (`0o2000`), or sticky (`0o1000`)
/// bits recovered from an untrusted archive.
#[cfg(unix)]
fn unix_mode_from_7z_attributes(has_windows_attributes: bool, attrs: u32) -> Option<u32> {
    if has_windows_attributes && attrs & 0x8000 != 0 {
        Some((attrs >> 16) & 0o777)
    } else {
        None
    }
}

fn extract_7z_pure(
    archive: &Path,
    dest: &Path,
    progress: ExtractProgress,
) -> Result<(), SevenZFailure> {
    let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty())
        .map_err(|e| SevenZFailure::Other(format!("7z: {e}")))?;

    let total: u64 = reader
        .archive()
        .files
        .iter()
        .filter(|entry| entry.has_stream())
        .map(|entry| entry.size())
        .sum();
    let mut processed: u64 = 0;
    progress(0, total);

    let mut unsafe_name: Option<String> = None;
    let result = reader.for_each_entries(|entry, content| {
        let raw_name = entry.name();
        if is_unsafe_7z_path(raw_name) {
            unsafe_name = Some(raw_name.to_string());
            // Stop iterating; this is not an error for `for_each_entries`
            // itself, just an early exit — the caller checks `unsafe_name`.
            return Ok(false);
        }
        let relative = PathBuf::from(raw_name.replace('\\', "/"));
        let out_path = dest.join(&relative);

        if entry.is_directory() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            io::copy(content, &mut out_file)?;
            processed += entry.size();

            // Preserve the stored Unix permission bits, when present. 7z
            // stores them in the Windows attribute field's upper 16 bits,
            // gated by the 0x8000 "Unix extension" flag (see
            // `unix_mode_from_7z_attributes`). SECURITY: mask to
            // `mode & 0o777` only — never propagate setuid (0o4000), setgid
            // (0o2000), or sticky (0o1000) bits from an untrusted archive.
            #[cfg(unix)]
            if let Some(mode) =
                unix_mode_from_7z_attributes(entry.has_windows_attributes, entry.windows_attributes)
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&out_path, fs::Permissions::from_mode(mode))?;
            }
        }
        // Emit after every member (directory or file), mirroring the zip
        // extractor: the last call here — for the archive's final entry —
        // is what leaves the caller with processed == total.
        progress(processed, total);
        Ok(true)
    });

    if let Some(raw_name) = unsafe_name {
        return Err(SevenZFailure::UnsafePath(format!(
            "archive contains an unsafe path: {raw_name}"
        )));
    }
    result.map_err(|e| SevenZFailure::Other(format!("7z: {e}")))?;

    Ok(())
}

/// Whether a 7z entry's name (after normalizing `\` to `/`) is absolute or
/// contains a `..` component.
fn is_unsafe_7z_path(raw_name: &str) -> bool {
    if is_absolute_entry_path(raw_name) {
        return true;
    }
    let normalized = raw_name.replace('\\', "/");
    Path::new(&normalized)
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
}

/// Absolute paths searched (after `PATH`) for a system 7-Zip executable,
/// covering the Linux and macOS locations this engine targets.
const SYSTEM_7Z_CANDIDATES: &[&str] = &[
    "/usr/bin/7z",
    "/usr/bin/7za",
    "/usr/bin/7zz",
    "/usr/lib/p7zip/7za",
    "/opt/homebrew/bin/7z",
    "/usr/local/bin/7z",
    "/usr/local/bin/7za",
];

fn find_system_7z() -> Option<PathBuf> {
    for name in ["7z", "7za", "7zz"] {
        if let Some(path) = which_on_path(name) {
            return Some(path);
        }
    }
    SYSTEM_7Z_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

/// `pub(crate)`: reused by `cloud::archive`'s system-7z fallback so it can
/// probe `PATH` for `7z`/`7za`/`7zz` in Python's exact order
/// (`cloud_transfer.py:150-165`) without duplicating this lookup.
pub(crate) fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn extract_7z_system_fallback(archive: &Path, dest: &Path) -> Result<(), String> {
    let Some(executable) = find_system_7z() else {
        return Err("no system 7-Zip executable found on this system".to_string());
    };
    run_7z_extract(&executable, archive, dest)
}

/// Shells out to `executable` (`7z`/`7za`/`7zz`-compatible CLI) to extract
/// `archive` into `dest`, wiping `dest` first. Shared by
/// `extract_7z_system_fallback` and `extract_iso_with_7z` so both go
/// through one place that builds the argument list and maps a
/// nonzero exit / spawn failure to a message naming the executable.
fn run_7z_extract(executable: &Path, archive: &Path, dest: &Path) -> Result<(), String> {
    wipe_and_recreate(dest).map_err(|e| format!("failed to reset destination: {e}"))?;

    let output = Command::new(executable)
        .arg("x")
        .arg(archive)
        .arg(format!("-o{}", dest.display()))
        .arg("-y")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run {}: {e}", executable.display()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{} exited with {}: {}",
            executable.display(),
            output.status,
            stderr.trim()
        ))
    }
}

// --- ISO (via system 7-Zip only; no pure-Rust ISO 9660 extractor is wired
// into this engine) ------------------------------------------------------

/// Extracts an ISO 9660 image via the system 7-Zip binary. There is no
/// pure-Rust fallback for ISOs (unlike `.7z`, which tries `sevenz-rust2`
/// first), so this only ever succeeds by shelling out; it exists as a
/// thin, ISO-specific entry point so the caller gets an error naming the
/// ISO file itself when no 7-Zip binary can be found at all, before ever
/// reaching `extract_7z_system_fallback`'s own (more generic) "no system
/// 7-Zip executable found" message.
///
/// Still `pub(crate)` and exercised only by its own unit tests today — no
/// install-pipeline caller wires ISO extraction through it yet; that is
/// out of scope for this task and lands in a later one.
#[allow(dead_code)]
pub(crate) fn extract_iso_with_system_7z(iso: &Path, dest: &Path) -> Result<(), String> {
    extract_iso_with_7z(iso, dest, find_system_7z().as_deref())
}

/// [`extract_iso_with_system_7z`]'s logic, with the 7-Zip binary supplied
/// by the caller instead of resolved via [`find_system_7z`]. Split out so
/// the "no binary found" branch can be exercised deterministically in
/// tests: `find_system_7z` also probes a handful of hardcoded absolute
/// paths (`SYSTEM_7Z_CANDIDATES`) that don't depend on `PATH`, so on a
/// machine that has a system 7-Zip installed at one of them, no amount of
/// `PATH` manipulation can make `find_system_7z()` return `None` — this
/// function lets a test bypass that resolution step entirely and drive
/// both branches directly.
pub(crate) fn extract_iso_with_7z(
    iso: &Path,
    dest: &Path,
    seven_zip: Option<&Path>,
) -> Result<(), String> {
    let Some(executable) = seven_zip else {
        let name = iso
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(format!("Cannot extract ISO {name}: no 7-Zip binary found"));
    };
    run_7z_extract(executable, iso, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_extractable_archive_ignores_platform_rules() {
        assert!(is_extractable_archive(Path::new("/x/game.zip")));
        assert!(is_extractable_archive(Path::new("/x/game.7Z")));
        assert!(is_extractable_archive(Path::new("/x/game.rar")));
        assert!(!is_extractable_archive(Path::new("/x/game.iso")));
        assert!(!is_extractable_archive(Path::new("/x/game.exe")));
        assert!(!is_extractable_archive(Path::new("/x/game")));
        // The native platform's "always extract" rule does not apply here.
        assert!(should_extract("Windows", Path::new("/x/game.iso")));
    }

    // --- is_arcade_platform -------------------------------------------------

    #[test]
    fn is_arcade_platform_matches_known_substrings() {
        for platform in [
            "Arcade",
            "MAME",
            "FBNeo",
            "Final Burn Neo",
            "arcade cabinet",
        ] {
            assert!(is_arcade_platform(platform), "{platform} should be arcade");
        }
    }

    #[test]
    fn is_arcade_platform_false_for_non_arcade() {
        assert!(!is_arcade_platform("Sony PlayStation"));
    }

    // --- mask_zip_unix_mode -----------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn mask_zip_unix_mode_strips_setuid_setgid_sticky() {
        // 0o104755 = S_IFREG (0o100000) | setuid (0o4000) | 0o755 — the raw
        // shape `ZipFile::unix_mode()` would hand back for a genuine setuid
        // regular file from some other zip tool (the `zip` crate's own
        // writer can never produce one; see `mask_zip_unix_mode`'s doc
        // comment). Also covers setgid and sticky.
        assert_eq!(mask_zip_unix_mode(0o104755), 0o755);
        assert_eq!(mask_zip_unix_mode(0o102755), 0o755);
        assert_eq!(mask_zip_unix_mode(0o101755), 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn mask_zip_unix_mode_preserves_permission_bits() {
        assert_eq!(mask_zip_unix_mode(0o100644), 0o644);
        assert_eq!(mask_zip_unix_mode(0o100755), 0o755);
    }

    // --- unix_mode_from_7z_attributes ------------------------------------------

    #[cfg(unix)]
    #[test]
    fn unix_mode_from_7z_attributes_recovers_masked_mode() {
        // 0x8000 flag set, mode 0o755 in the upper 16 bits.
        assert_eq!(
            unix_mode_from_7z_attributes(true, 0x8000 | (0o755 << 16)),
            Some(0o755)
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_mode_from_7z_attributes_strips_setuid_setgid_sticky() {
        assert_eq!(
            unix_mode_from_7z_attributes(true, 0x8000 | (0o4755 << 16)),
            Some(0o755)
        );
        assert_eq!(
            unix_mode_from_7z_attributes(true, 0x8000 | (0o2755 << 16)),
            Some(0o755)
        );
        assert_eq!(
            unix_mode_from_7z_attributes(true, 0x8000 | (0o1755 << 16)),
            Some(0o755)
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_mode_from_7z_attributes_none_without_the_unix_flag() {
        // Windows attributes present, but no 0x8000 Unix-extension flag.
        assert_eq!(unix_mode_from_7z_attributes(true, 0x20), None);
    }

    #[cfg(unix)]
    #[test]
    fn unix_mode_from_7z_attributes_none_without_windows_attributes() {
        assert_eq!(
            unix_mode_from_7z_attributes(false, 0x8000 | (0o755 << 16)),
            None
        );
    }

    // --- should_extract -------------------------------------------------------

    #[test]
    fn should_extract_table() {
        let cases: &[(&str, &str, bool)] = &[
            ("SNES", "game.zip", true),
            ("SNES", "game.7z", true),
            ("SNES", "game.tar", true),
            ("SNES", "game.tar.gz", true),
            ("SNES", "game.bz2", true),
            ("SNES", "game.xz", true),
            ("SNES", "game.ZIP", true),
            ("SNES", "game.rar", true),
            ("Arcade", "game.zip", false),
            ("MAME", "game.zip", false),
            ("SNES", "game", false),
        ];
        for (platform, archive, expected) in cases {
            assert_eq!(
                should_extract(platform, Path::new(archive)),
                *expected,
                "platform={platform} archive={archive}"
            );
        }
    }

    // --- rar_entry_relative_path ---------------------------------------------

    #[test]
    fn rar_entry_relative_path_rejects_traversal() {
        for bad in ["../x", "/abs", "a/../../b"] {
            assert!(
                rar_entry_relative_path(bad).is_err(),
                "{bad} should be rejected"
            );
        }
        assert_eq!(
            rar_entry_relative_path("dir/file.bin").unwrap(),
            PathBuf::from("dir/file.bin")
        );
        // Backslashes (the separator RAR entries commonly use) normalize to
        // `/` rather than being treated as a literal filename character.
        assert_eq!(
            rar_entry_relative_path("dir\\file.bin").unwrap(),
            PathBuf::from("dir/file.bin")
        );
    }

    #[test]
    fn rar_entry_relative_path_rejects_nul_bytes() {
        assert!(rar_entry_relative_path("dir/evil\0.bin").is_err());
    }

    // --- rar_entry_is_symlink -------------------------------------------------

    /// Tested against raw attribute words rather than a real archive: there
    /// is no `rar` binary on the build machines, so a link-carrying fixture
    /// cannot be produced here. The values are the ones a Unix-host RAR
    /// writes into `file_attr` (the `st_mode` verbatim).
    #[test]
    fn rar_entry_is_symlink_matches_the_unix_link_mode() {
        // S_IFLNK | 0o777 — what a Unix-host RAR stores for a symlink.
        assert!(rar_entry_is_symlink(0o120_777));
        assert!(rar_entry_is_symlink(0xA1FF));
        // Plain files, directories, and a setuid file are not links.
        assert!(!rar_entry_is_symlink(0o100_644));
        assert!(!rar_entry_is_symlink(0o040_755));
        assert!(!rar_entry_is_symlink(0o104_755));
        // Windows-host attribute words (READONLY, ARCHIVE, DIRECTORY,
        // REPARSE_POINT) never match the Unix link mode.
        for attr in [0x0001, 0x0010, 0x0020, 0x0400, 0x0080] {
            assert!(
                !rar_entry_is_symlink(attr),
                "matched windows attr {attr:#x}"
            );
        }
    }

    // --- extract_iso_with_7z --------------------------------------------------

    /// Drives the "no binary" branch directly through `extract_iso_with_7z`
    /// with `seven_zip: None`, rather than by trying to make
    /// `find_system_7z()` itself return `None` (unreliable: it also probes
    /// hardcoded absolute paths such as `/usr/bin/7z` that don't depend on
    /// `PATH`, so on a machine that has a system 7-Zip installed at one of
    /// them — this dev box among them — no environment manipulation can
    /// force that branch through `extract_iso_with_system_7z`). This makes
    /// the assertion deterministic on every machine.
    #[test]
    fn iso_helper_reports_missing_7z() {
        let result =
            extract_iso_with_7z(Path::new("game.iso"), Path::new("/nonexistent-dest"), None);
        assert_eq!(
            result,
            Err("Cannot extract ISO game.iso: no 7-Zip binary found".to_string())
        );
    }

    /// Drives the "binary found" branch with a fake `7z`-shaped script
    /// (rather than depending on a real system 7-Zip, which may or may not
    /// be installed) so this is deterministic on every machine too: the
    /// script parses the `-o<dest>` argument `run_7z_extract` passes and
    /// writes a known file into it, standing in for a real extraction.
    #[cfg(unix)]
    #[test]
    fn iso_helper_extracts_via_a_provided_7z_binary() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-7z.sh");
        fs::write(
            &script,
            "#!/bin/sh\n\
             dest=\n\
             for arg in \"$@\"; do\n\
             \x20\x20case \"$arg\" in\n\
             \x20\x20\x20\x20-o*) dest=\"${arg#-o}\" ;;\n\
             \x20\x20esac\n\
             done\n\
             mkdir -p \"$dest\"\n\
             printf 'extracted' > \"$dest/GAME.BIN\"\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        let iso = dir.path().join("game.iso");
        fs::write(&iso, b"fake iso bytes").unwrap();
        let dest = dir.path().join("out");

        let result = extract_iso_with_7z(&iso, &dest, Some(&script));

        assert_eq!(result, Ok(()));
        assert_eq!(fs::read(dest.join("GAME.BIN")).unwrap(), b"extracted");
    }
}
