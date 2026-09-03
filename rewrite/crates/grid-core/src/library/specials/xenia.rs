//! Xbox 360 (Xenia) STFS header parsing and content apply.
//!
//! Ports `grid_launcher/emulator/xenia.py:1-95` (`_read_stfs_header`,
//! `apply_xenia_content_without_ui`) and
//! `grid_launcher/library/archive_preparation.py:781-830`
//! (`apply_xenia_content_archive_without_ui`); see
//! `docs/porting/03-library-install.md` §13 for the behavior contract this
//! mirrors.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::ps4::ExtractFn;

/// The number of bytes of an STFS package that make up its header. Matches
/// the `0x368`-byte read in `_read_stfs_header` (`xenia.py:22`).
pub const STFS_HEADER_LEN: usize = 0x368;

/// The three magic values that mark a file as an STFS package. Matches
/// `_STFS_MAGIC` (`xenia.py:12`).
const STFS_MAGIC: [&[u8; 4]; 3] = [b"CON ", b"LIVE", b"PIRS"];
/// Big-endian `u32` offset of the content type field. Matches
/// `_STFS_CONTENT_TYPE_OFFSET` (`xenia.py:13`).
const STFS_CONTENT_TYPE_OFFSET: usize = 0x344;
/// Big-endian `u32` offset of the title id field. Matches
/// `_STFS_TITLE_ID_OFFSET` (`xenia.py:14`).
const STFS_TITLE_ID_OFFSET: usize = 0x360;
/// The anonymous-XUID directory every Xenia content path is rooted under.
/// Matches `_XUID_ANONYMOUS` (`xenia.py:15`).
const XUID_ANONYMOUS: &str = "0000000000000000";

/// Reads `path`'s STFS header and returns `(title_id_hex8, content_type_hex8)`,
/// or `None` if the file is too short to hold a full header or does not
/// start with a recognized STFS magic. Matches `_read_stfs_header`
/// (`xenia.py:18`); unlike the Python version (which returns `("", "")` on
/// failure), this returns `Option::None` so callers can't mistake a failure
/// for a valid all-zero header.
pub fn read_stfs_header(path: &Path) -> Option<(String, String)> {
    let mut file = fs::File::open(path).ok()?;
    let mut header = vec![0u8; STFS_HEADER_LEN];
    let mut read_total = 0usize;
    loop {
        let n = file.read(&mut header[read_total..]).ok()?;
        if n == 0 {
            break;
        }
        read_total += n;
        if read_total == STFS_HEADER_LEN {
            break;
        }
    }
    if read_total < STFS_HEADER_LEN {
        return None;
    }

    let magic = &header[0..4];
    if !STFS_MAGIC.iter().any(|m| m.as_slice() == magic) {
        return None;
    }

    let content_type = u32::from_be_bytes(
        header[STFS_CONTENT_TYPE_OFFSET..STFS_CONTENT_TYPE_OFFSET + 4]
            .try_into()
            .expect("4-byte slice"),
    );
    let title_id = u32::from_be_bytes(
        header[STFS_TITLE_ID_OFFSET..STFS_TITLE_ID_OFFSET + 4]
            .try_into()
            .expect("4-byte slice"),
    );

    Some((format!("{title_id:08X}"), format!("{content_type:08X}")))
}

/// The result of a successful [`apply_content_file`] call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct XeniaApplied {
    pub title_id: String,
    pub content_type: String,
    pub destination: String,
}

/// Copies best-effort metadata (currently just the modified time) from
/// `src` onto `dst`, after `dst` has already been written via
/// [`fs::copy`]. Uses only stable std (`File::set_modified`), mirroring
/// Python's `shutil.copy2` without pulling in a `filetime` dependency.
/// Failures are ignored — metadata preservation is best-effort.
fn copy_with_metadata(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::copy(src, dst)?;
    if let Ok(metadata) = fs::metadata(src) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(dst_file) = fs::File::open(dst) {
                let _ = dst_file.set_modified(modified);
            }
        }
    }
    Ok(())
}

/// Copies an STFS content package to the correct Xenia content directory.
/// Matches `apply_xenia_content_without_ui` (`xenia.py:36`).
///
/// Fails with `"Content file not found: <path>"` if `file` does not exist
/// or is not a regular file, `"File does not appear to be an STFS package
/// (bad magic)"` if it doesn't parse as STFS, or `"Title ID mismatch:
/// expected <ID>, archive contains <id>"` (case-insensitive compare,
/// `expected` upper-cased in the message) when `expected_title_id` is
/// non-empty and doesn't match. On success, copies `file` (metadata
/// preserved best-effort) to `<content_root>/0000000000000000/<TitleID>/
/// <ContentType>/<file name>`, creating parent directories as needed.
pub fn apply_content_file(
    file: &Path,
    content_root: &Path,
    expected_title_id: &str,
) -> Result<XeniaApplied, String> {
    if !file.is_file() {
        return Err(format!("Content file not found: {}", file.display()));
    }

    let Some((title_id, content_type)) = read_stfs_header(file) else {
        return Err("File does not appear to be an STFS package (bad magic)".to_string());
    };

    if !expected_title_id.is_empty() && !expected_title_id.eq_ignore_ascii_case(&title_id) {
        return Err(format!(
            "Title ID mismatch: expected {}, archive contains {}",
            expected_title_id.to_uppercase(),
            title_id
        ));
    }

    let dest_dir = content_root
        .join(XUID_ANONYMOUS)
        .join(&title_id)
        .join(&content_type);
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

    let file_name = file
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(file));
    let dest_path = dest_dir.join(&file_name);
    copy_with_metadata(file, &dest_path).map_err(|e| e.to_string())?;

    Ok(XeniaApplied {
        title_id,
        content_type,
        destination: dest_path.to_string_lossy().into_owned(),
    })
}

/// Recursively collects every regular file under `dir`, sorted by path.
fn sorted_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(dir, &mut files);
    files.sort();
    files
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(&path, out);
        } else if file_type.is_file() {
            out.push(path);
        }
    }
}

/// Extracts `archive` into `staging` (removed on every exit afterwards)
/// and applies every STFS package found inside it to `content_root`.
/// Matches `apply_xenia_content_archive_without_ui`
/// (`archive_preparation.py:781`).
///
/// If extraction fails, returns `Err(e.to_string())`. Otherwise walks
/// every regular file under `staging` in sorted order, applying each via
/// [`apply_content_file`]; successes and error strings are collected
/// separately. If there were errors and no successes, returns
/// `Err(errors joined by "\n")`. Otherwise returns `Ok((successes, warning))`
/// where `warning` is the errors joined by `"\n"` (empty if there were
/// none).
pub fn apply_content_archive(
    archive: &Path,
    content_root: &Path,
    staging: &Path,
    expected_title_id: &str,
    extract: ExtractFn,
) -> Result<(Vec<XeniaApplied>, String), String> {
    extract(archive, staging).map_err(|e| e.to_string())?;
    let _staging_guard = super::StagingGuard(staging.to_path_buf());

    let mut successes = Vec::new();
    let mut errors = Vec::new();
    for file in sorted_files(staging) {
        match apply_content_file(&file, content_root, expected_title_id) {
            Ok(applied) => successes.push(applied),
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() && successes.is_empty() {
        return Err(errors.join("\n"));
    }
    let warning = errors.join("\n");
    Ok((successes, warning))
}

/// Builds a `0x368`-byte fake STFS header (zero-filled, with `magic`,
/// `title_id` at [`STFS_TITLE_ID_OFFSET`] and `content_type` at
/// [`STFS_CONTENT_TYPE_OFFSET`] set, both big-endian) for tests. `pub` so
/// downstream E2E-parity tests (Task 10) can fabricate STFS packages
/// without duplicating this layout.
pub fn build_stfs_bytes(magic: &[u8; 4], title_id: u32, content_type: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; STFS_HEADER_LEN];
    bytes[0..4].copy_from_slice(magic);
    bytes[STFS_CONTENT_TYPE_OFFSET..STFS_CONTENT_TYPE_OFFSET + 4]
        .copy_from_slice(&content_type.to_be_bytes());
    bytes[STFS_TITLE_ID_OFFSET..STFS_TITLE_ID_OFFSET + 4].copy_from_slice(&title_id.to_be_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::LibraryError;

    // -- read_stfs_header -----------------------------------------------

    #[test]
    fn read_stfs_header_parses_a_good_live_package() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package.bin");
        fs::write(&path, build_stfs_bytes(b"LIVE", 0x415608C3, 0x000B0000)).unwrap();

        let result = read_stfs_header(&path);

        assert_eq!(
            result,
            Some(("415608C3".to_string(), "000B0000".to_string()))
        );
    }

    #[test]
    fn read_stfs_header_rejects_a_short_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.bin");
        let mut bytes = build_stfs_bytes(b"LIVE", 0x415608C3, 0x000B0000);
        bytes.truncate(STFS_HEADER_LEN - 1);
        fs::write(&path, bytes).unwrap();

        assert_eq!(read_stfs_header(&path), None);
    }

    #[test]
    fn read_stfs_header_rejects_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_magic.bin");
        fs::write(&path, build_stfs_bytes(b"NOPE", 0x415608C3, 0x000B0000)).unwrap();

        assert_eq!(read_stfs_header(&path), None);
    }

    #[test]
    fn read_stfs_header_missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.bin");

        assert_eq!(read_stfs_header(&path), None);
    }

    // -- apply_content_file -----------------------------------------------

    #[test]
    fn apply_content_file_rejects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.bin");
        let content_root = dir.path().join("content");

        let result = apply_content_file(&missing, &content_root, "");

        assert_eq!(
            result,
            Err(format!("Content file not found: {}", missing.display()))
        );
    }

    #[test]
    fn apply_content_file_rejects_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.bin");
        fs::write(&file, build_stfs_bytes(b"BAD ", 0x415608C3, 0x000B0000)).unwrap();
        let content_root = dir.path().join("content");

        let result = apply_content_file(&file, &content_root, "");

        assert_eq!(
            result,
            Err("File does not appear to be an STFS package (bad magic)".to_string())
        );
    }

    #[test]
    fn apply_content_file_rejects_title_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.bin");
        fs::write(&file, build_stfs_bytes(b"LIVE", 0x415608C3, 0x000B0000)).unwrap();
        let content_root = dir.path().join("content");

        let result = apply_content_file(&file, &content_root, "41560000");

        assert_eq!(
            result,
            Err("Title ID mismatch: expected 41560000, archive contains 415608C3".to_string())
        );
    }

    #[test]
    fn apply_content_file_succeeds_and_lays_out_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.bin");
        fs::write(&file, build_stfs_bytes(b"LIVE", 0x415608C3, 0x000B0000)).unwrap();
        let content_root = dir.path().join("content");

        let result = apply_content_file(&file, &content_root, "415608c3").unwrap();

        assert_eq!(result.title_id, "415608C3");
        assert_eq!(result.content_type, "000B0000");
        let expected_dest = content_root
            .join("0000000000000000")
            .join("415608C3")
            .join("000B0000")
            .join("package.bin");
        assert_eq!(result.destination, expected_dest.to_string_lossy());
        assert!(expected_dest.is_file());
        assert_eq!(fs::read(&expected_dest).unwrap(), fs::read(&file).unwrap());
    }

    // -- apply_content_archive ---------------------------------------------

    #[test]
    fn apply_content_archive_mixed_success_and_error_returns_warning() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"fake archive").unwrap();
        let content_root = dir.path().join("content");
        let staging = dir.path().join("staging");

        let extract = |_archive: &Path, dest: &Path| -> Result<(), LibraryError> {
            fs::create_dir_all(dest).unwrap();
            fs::write(
                dest.join("good.bin"),
                build_stfs_bytes(b"LIVE", 0x415608C3, 0x000B0000),
            )
            .unwrap();
            fs::write(dest.join("bad.bin"), b"not an stfs package").unwrap();
            Ok(())
        };

        let result = apply_content_archive(&archive, &content_root, &staging, "", &extract);

        let (successes, warning) = result.unwrap();
        assert_eq!(successes.len(), 1);
        assert_eq!(successes[0].title_id, "415608C3");
        assert_eq!(
            warning,
            "File does not appear to be an STFS package (bad magic)"
        );
        assert!(!staging.exists());
    }

    #[test]
    fn apply_content_archive_all_errors_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"fake archive").unwrap();
        let content_root = dir.path().join("content");
        let staging = dir.path().join("staging");

        let extract = |_archive: &Path, dest: &Path| -> Result<(), LibraryError> {
            fs::create_dir_all(dest).unwrap();
            fs::write(dest.join("bad.bin"), b"not an stfs package").unwrap();
            Ok(())
        };

        let result = apply_content_archive(&archive, &content_root, &staging, "", &extract);

        assert_eq!(
            result,
            Err("File does not appear to be an STFS package (bad magic)".to_string())
        );
        assert!(!staging.exists());
    }

    #[test]
    fn apply_content_archive_extract_failure_propagates_and_leaves_staging_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"fake archive").unwrap();
        let content_root = dir.path().join("content");
        let staging = dir.path().join("staging");
        fs::create_dir_all(&staging).unwrap();

        let extract = |_archive: &Path, _dest: &Path| -> Result<(), LibraryError> {
            Err(LibraryError::Extract("boom".to_string()))
        };

        let result = apply_content_archive(&archive, &content_root, &staging, "", &extract);

        assert_eq!(result, Err("boom".to_string()));
        // The extraction error return happens before the staging guard is
        // constructed, so staging is left untouched on this path — matching
        // Python, which only wraps `shutil.rmtree` in the `finally` around
        // the extraction call, not before it.
        assert!(staging.exists());
    }

    #[test]
    fn apply_content_archive_all_success_has_empty_warning() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("archive.zip");
        fs::write(&archive, b"fake archive").unwrap();
        let content_root = dir.path().join("content");
        let staging = dir.path().join("staging");

        let extract = |_archive: &Path, dest: &Path| -> Result<(), LibraryError> {
            fs::create_dir_all(dest).unwrap();
            fs::write(
                dest.join("good.bin"),
                build_stfs_bytes(b"CON ", 0x11112222, 0x00030000),
            )
            .unwrap();
            Ok(())
        };

        let result = apply_content_archive(&archive, &content_root, &staging, "", &extract);

        let (successes, warning) = result.unwrap();
        assert_eq!(successes.len(), 1);
        assert_eq!(warning, "");
    }
}
