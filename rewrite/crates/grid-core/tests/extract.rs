use std::fs;
use std::io::Write;
use std::path::Path;

use grid_core::library::extract::extract_archive;
use grid_core::library::LibraryError;

// --- fixture builders -------------------------------------------------------

/// Builds a zip archive at `path` containing `entries` (relative path,
/// content), using `Stored` (uncompressed) entries so fixture bytes are
/// predictable.
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for &(name, content) in entries {
        zip.start_file(name, options).unwrap();
        zip.write_all(content).unwrap();
    }
    zip.finish().unwrap();
}

/// Sets a tar header's name directly on the raw byte field, bypassing
/// `tar`'s own `Header::set_path` traversal check. Needed so tests can
/// build a fixture containing a `..` entry to exercise *our* traversal
/// guard rather than the crate's.
fn set_raw_name(header: &mut tar::Header, name: &str) {
    let bytes = name.as_bytes();
    let old = header.as_old_mut();
    old.name[..bytes.len()].copy_from_slice(bytes);
}

fn append_entries<W: Write>(builder: &mut tar::Builder<W>, entries: &[(&str, &[u8])]) {
    for &(name, content) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        set_raw_name(&mut header, name);
        header.set_cksum();
        builder.append(&header, content).unwrap();
    }
}

fn write_tar_gz(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    append_entries(&mut builder, entries);
    builder.into_inner().unwrap().finish().unwrap();
}

fn write_tar_xz(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let encoder = liblzma::write::XzEncoder::new(file, 6);
    let mut builder = tar::Builder::new(encoder);
    append_entries(&mut builder, entries);
    builder.into_inner().unwrap().finish().unwrap();
}

fn write_7z(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut writer = sevenz_rust2::ArchiveWriter::new(file).unwrap();
    for &(name, content) in entries {
        let entry = sevenz_rust2::ArchiveEntry::new_file(name);
        writer.push_archive_entry(entry, Some(content)).unwrap();
    }
    writer.finish().unwrap();
}

fn read_to_string(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

// --- zip ----------------------------------------------------------------------

#[test]
fn extract_zip_writes_files_and_directories() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("game.zip");
    write_zip(
        &archive,
        &[
            ("root.txt", b"root content" as &[u8]),
            ("sub/nested.txt", b"nested content"),
        ],
    );
    let dest = dir.path().join("out");

    let mut calls = Vec::new();
    extract_archive(&archive, &dest, &mut |processed, total| {
        calls.push((processed, total));
    })
    .unwrap();

    assert_eq!(read_to_string(&dest.join("root.txt")), "root content");
    assert_eq!(
        read_to_string(&dest.join("sub").join("nested.txt")),
        "nested content"
    );
    let (last_processed, last_total) = *calls.last().unwrap();
    assert_eq!(last_processed, last_total);
    assert!(last_total > 0);
}

#[test]
fn extract_zip_wipes_pre_existing_junk_in_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("game.zip");
    write_zip(&archive, &[("game.txt", b"game data" as &[u8])]);
    let dest = dir.path().join("out");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("stale.junk"), b"leftover from a previous attempt").unwrap();

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    assert!(!dest.join("stale.junk").exists());
    assert_eq!(read_to_string(&dest.join("game.txt")), "game data");
}

#[test]
fn extract_truncated_zip_deletes_dest_and_returns_extract_error() {
    let dir = tempfile::tempdir().unwrap();
    let valid = dir.path().join("valid.zip");
    write_zip(&valid, &[("game.txt", b"game data" as &[u8])]);
    let full_bytes = fs::read(&valid).unwrap();

    let archive = dir.path().join("truncated.zip");
    fs::write(&archive, &full_bytes[..full_bytes.len() / 2]).unwrap();

    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(_)) => {}
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

#[test]
fn extract_zip_entry_with_parent_dir_traversal_fails_and_deletes_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.zip");
    write_zip(&archive, &[("../evil.txt", b"pwned" as &[u8])]);
    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(message)) => {
            assert!(
                message.contains("unsafe path"),
                "unexpected message: {message}"
            );
            assert!(
                message.contains("evil.txt"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

#[test]
fn extract_zip_entry_with_absolute_path_fails_and_deletes_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil-absolute.zip");
    write_zip(&archive, &[("/etc/evil.txt", b"pwned" as &[u8])]);
    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(message)) => {
            assert!(
                message.contains("unsafe path"),
                "unexpected message: {message}"
            );
            assert!(
                message.contains("evil.txt"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

// --- tar.gz / tar.xz ------------------------------------------------------------

#[test]
fn extract_tar_gz_writes_files() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("game.tar.gz");
    write_tar_gz(
        &archive,
        &[
            ("root.txt", b"root content" as &[u8]),
            ("sub/nested.txt", b"nested content"),
        ],
    );
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    assert_eq!(read_to_string(&dest.join("root.txt")), "root content");
    assert_eq!(
        read_to_string(&dest.join("sub").join("nested.txt")),
        "nested content"
    );
}

/// The executable bit on a tar member has to survive extraction, or an
/// emulator shipping a bare ELF binary can never be selected as launchable
/// (`launchable_emulator_file`'s unix executable-bit rule, launch/emu_install.rs).
#[cfg(unix)]
#[test]
fn extract_tar_gz_preserves_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("emu.tar.gz");
    let file = fs::File::create(&archive).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (name, mode) in [("redream", 0o755u32), ("readme.txt", 0o644)] {
        let content: &[u8] = b"body";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(mode);
        set_raw_name(&mut header, name);
        header.set_cksum();
        builder.append(&header, content).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    let mode_of = |name: &str| fs::metadata(dest.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode_of("redream"), 0o755);
    assert_eq!(mode_of("readme.txt"), 0o644);
}

#[test]
fn extract_tar_xz_writes_files() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("game.tar.xz");
    write_tar_xz(&archive, &[("root.txt", b"xz content" as &[u8])]);
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    assert_eq!(read_to_string(&dest.join("root.txt")), "xz content");
}

#[test]
fn extract_tar_entry_with_parent_dir_traversal_fails_and_deletes_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.tar.gz");
    write_tar_gz(&archive, &[("../evil.txt", b"pwned" as &[u8])]);
    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(message)) => {
            assert!(
                message.contains("unsafe path"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

#[test]
fn extract_tar_entry_with_absolute_path_fails_and_deletes_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil-absolute.tar.gz");
    write_tar_gz(&archive, &[("/etc/evil.txt", b"pwned" as &[u8])]);
    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(message)) => {
            assert!(
                message.contains("unsafe path"),
                "unexpected message: {message}"
            );
            assert!(
                message.contains("evil.txt"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

// --- 7z --------------------------------------------------------------------

#[test]
fn extract_7z_writes_files() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("game.7z");
    write_7z(
        &archive,
        &[
            ("root.txt", b"root content" as &[u8]),
            ("sub/nested.txt", b"nested content"),
        ],
    );
    let dest = dir.path().join("out");

    let mut calls = Vec::new();
    extract_archive(&archive, &dest, &mut |processed, total| {
        calls.push((processed, total));
    })
    .unwrap();

    assert_eq!(read_to_string(&dest.join("root.txt")), "root content");
    assert_eq!(
        read_to_string(&dest.join("sub").join("nested.txt")),
        "nested content"
    );
    let (last_processed, last_total) = *calls.last().unwrap();
    assert_eq!(last_processed, last_total);
    assert!(last_total > 0);
}

#[test]
fn extract_7z_reports_intermediate_progress_for_multi_entry_archive() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("multi.7z");
    write_7z(
        &archive,
        &[
            ("first.txt", b"first entry content" as &[u8]),
            (
                "second.txt",
                b"second entry content, deliberately a bit longer",
            ),
        ],
    );
    let dest = dir.path().join("out");

    let mut calls: Vec<(u64, u64)> = Vec::new();
    extract_archive(&archive, &dest, &mut |processed, total| {
        calls.push((processed, total));
    })
    .unwrap();

    assert!(
        calls.len() > 2,
        "expected an initial call plus one per entry, got {calls:?}"
    );

    let (last_processed, last_total) = *calls.last().unwrap();
    assert_eq!(last_processed, last_total);
    assert!(last_total > 0);

    let mut prev_processed = 0u64;
    for &(processed, total) in &calls {
        assert!(
            processed >= prev_processed,
            "processed must not decrease: {calls:?}"
        );
        assert_eq!(total, last_total, "total must stay stable: {calls:?}");
        prev_processed = processed;
    }

    assert!(
        calls.iter().any(|&(processed, total)| processed < total),
        "expected at least one intermediate (not-yet-complete) progress call, got {calls:?}"
    );
}

#[test]
fn extract_7z_entry_with_parent_dir_traversal_fails_and_deletes_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.7z");
    write_7z(&archive, &[("../evil.txt", b"pwned" as &[u8])]);
    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(message)) => {
            assert!(
                message.contains("unsafe path"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

#[test]
fn extract_7z_entry_with_absolute_path_fails_and_deletes_dest() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil-absolute.7z");
    write_7z(&archive, &[("/etc/evil.txt", b"pwned" as &[u8])]);
    let dest = dir.path().join("out");

    let result = extract_archive(&archive, &dest, &mut |_, _| {});

    assert!(!dest.exists());
    match result {
        Err(LibraryError::Extract(message)) => {
            assert!(
                message.contains("unsafe path"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected LibraryError::Extract, got {other:?}"),
    }
}

// --- should_extract / is_arcade_platform (behavioral smoke test; the full
// table lives as a unit test inside extract.rs) --------------------------------

#[test]
fn should_extract_is_reexported_and_callable() {
    use grid_core::library::extract::should_extract;
    assert!(should_extract("SNES", Path::new("game.zip")));
    assert!(!should_extract("Arcade", Path::new("game.zip")));
    assert!(!should_extract("SNES", Path::new("game.rar")));
}
