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
/// extract.
const EXTRACTABLE_SUFFIXES: &[&str] = &["7z", "zip", "tar", "gz", "bz2", "xz"];

/// Whether `archive` should be extracted for `platform`: never for an
/// arcade platform, otherwise only when its suffix is one of
/// `EXTRACTABLE_SUFFIXES` (case-insensitive; notably `.rar` is not
/// extracted by this engine).
pub fn should_extract(platform: &str, archive: &Path) -> bool {
    if is_arcade_platform(platform) {
        return false;
    }
    match lowercase_suffix(archive) {
        Some(suffix) => EXTRACTABLE_SUFFIXES.contains(&suffix.as_str()),
        None => false,
    }
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

// --- ZIP --------------------------------------------------------------------

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
        // rejects NUL bytes, absolute paths, and any path that would
        // resolve above the extraction root, satisfying the backslash
        // normalization + traversal guard in one call.
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
        }
        progress(processed, total);
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
        }
        Ok(true)
    });

    if let Some(raw_name) = unsafe_name {
        return Err(SevenZFailure::UnsafePath(format!(
            "archive contains an unsafe path: {raw_name}"
        )));
    }
    result.map_err(|e| SevenZFailure::Other(format!("7z: {e}")))?;

    progress(total, total);
    Ok(())
}

/// Whether a 7z entry's name (after normalizing `\` to `/`) is absolute or
/// contains a `..` component.
fn is_unsafe_7z_path(raw_name: &str) -> bool {
    let normalized = raw_name.replace('\\', "/");
    let path = Path::new(&normalized);
    path.is_absolute()
        || path
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

fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn extract_7z_system_fallback(archive: &Path, dest: &Path) -> Result<(), String> {
    let Some(executable) = find_system_7z() else {
        return Err("no system 7-Zip executable found on this system".to_string());
    };
    wipe_and_recreate(dest).map_err(|e| format!("failed to reset destination: {e}"))?;

    let output = Command::new(&executable)
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

#[cfg(test)]
mod tests {
    use super::*;

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
            ("SNES", "game.rar", false),
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
}
