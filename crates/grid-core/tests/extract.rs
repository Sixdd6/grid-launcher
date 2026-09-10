use std::fs;
use std::io::Write;
use std::path::Path;

use grid_core::library::extract::{extract_archive, should_extract};
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

/// Builds a zip archive at `path` from `(name, content, unix mode)` entries,
/// stamping each entry's Unix permission bits via `SimpleFileOptions::unix_permissions`.
fn write_zip_with_modes(path: &Path, entries: &[(&str, &[u8], u32)]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    for &(name, content, mode) in entries {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(mode);
        zip.start_file(name, options).unwrap();
        zip.write_all(content).unwrap();
    }
    zip.finish().unwrap();
}

/// Builds a 7z archive at `path` from `(name, content, unix mode)` entries,
/// stamping each entry's Windows attribute field with the `0x8000` Unix flag
/// and the mode in the upper 16 bits — the same layout `sevenz-rust2`'s
/// reader hands back via `ArchiveEntry::windows_attributes()`.
fn write_7z_with_modes(path: &Path, entries: &[(&str, &[u8], u32)]) {
    let file = fs::File::create(path).unwrap();
    let mut writer = sevenz_rust2::ArchiveWriter::new(file).unwrap();
    for &(name, content, mode) in entries {
        let mut entry = sevenz_rust2::ArchiveEntry::new_file(name);
        entry.has_windows_attributes = true;
        entry.windows_attributes = 0x8000 | (mode << 16);
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

/// The executable bit on a zip member (stamped via `unix_permissions`) has
/// to survive extraction, or an emulator shipping a bare ELF binary inside
/// a `.zip` can never be selected as launchable (mirrors the tar.gz
/// executable-bit test above).
#[cfg(unix)]
#[test]
fn extract_zip_preserves_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("emu.zip");
    write_zip_with_modes(
        &archive,
        &[
            ("redream", b"body" as &[u8], 0o755),
            ("readme.txt", b"body", 0o644),
        ],
    );
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    let mode_of = |name: &str| fs::metadata(dest.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode_of("redream"), 0o755);
    assert_eq!(mode_of("readme.txt"), 0o644);
}

/// End-to-end sanity check that the extraction pipeline still calls the
/// masking step for a nominal setuid-flavored mode. NOTE: this does not
/// exercise a real setuid bit end to end — `zip` 8.6.0's own writer
/// (`SimpleFileOptions::unix_permissions`, `write.rs:573-576`) already
/// masks to `mode & 0o777` before it ever reaches the archive, so a fixture
/// built via this crate's writer can never carry a real setuid/setgid/
/// sticky bit. The load-bearing proof that `& 0o777` is actually applied on
/// the extraction side lives in `mask_zip_unix_mode`'s own unit tests in
/// `extract.rs` (`mask_zip_unix_mode_strips_setuid_setgid_sticky`), which
/// exercise the mask directly against raw `unix_mode()`-shaped values a
/// hostile or third-party-written archive could actually contain.
#[cfg(unix)]
#[test]
fn extract_zip_entry_with_setuid_mode_strips_setuid_bit() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("emu.zip");
    write_zip_with_modes(&archive, &[("suspicious", b"body" as &[u8], 0o4755)]);
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    let mode = fs::metadata(dest.join("suspicious"))
        .unwrap()
        .permissions()
        .mode()
        & 0o7777;
    assert_eq!(mode, 0o755, "setuid bit must be stripped, got {mode:o}");
}

// NOTE: a "zip entry with no unix mode" case (simulating a Windows-built
// archive, where `ZipFile::unix_mode()` returns `None`) is deliberately not
// covered here: the `zip` crate's writer always stamps a `System::Unix`
// "version made by" host and a default `0o644`/`0o755` permission value on
// a unix build (see `FileOptions::normalize` in the `zip` crate), so there
// is no way to produce a fixture with `unix_mode() == None` without hand-
// crafting raw zip bytes — which the task brief says to avoid rather than
// fake. The `None` branch (leave today's default permissions untouched) is
// covered by inspection of `extract_zip`'s `if let Some(mode) = ...` guard.
// A related, hand-crafted case (`unix_mode()` returning `Some(..)` with a
// *synthesized*, Dos-origin mode) is covered below.

/// Overwrites `path`'s zip archive's (single-entry) central directory
/// record in place: sets the "version made by" host byte to `0` (MS-DOS/FAT,
/// `zip::System::Dos as u8`) and the external-attributes field to
/// `low_attr_byte` with its upper 24 bits zeroed — i.e. nonzero external
/// attributes with a zero high 16-bit word, the exact shape a real
/// DOS/Windows-native zip tool (not `zip`'s own writer, which always ORs
/// `S_IFREG` into a nonzero high word — see `mask_zip_unix_mode`'s doc
/// comment) produces, and which `ZipFile::unix_mode()` synthesizes a mode
/// for rather than returning `None`. Central directory field offsets are
/// from APPNOTE 4.3.12; overwriting existing bytes in place needs no
/// length/offset bookkeeping elsewhere in the file. Callers must build the
/// fixture with exactly one entry — this patches the first (and only)
/// central directory record it finds.
#[cfg(unix)]
fn patch_central_directory_dos_attrs(path: &Path, low_attr_byte: u8) {
    let mut bytes = fs::read(path).unwrap();
    let signature = [0x50, 0x4B, 0x01, 0x02]; // "PK\x01\x02"
    let pos = bytes
        .windows(4)
        .position(|window| window == signature)
        .expect("central directory record not found");
    bytes[pos + 5] = 0; // version made by: host system byte -> MS-DOS/FAT
    bytes[pos + 38] = low_attr_byte; // external attributes, byte 0 (low)
    bytes[pos + 39] = 0; // external attributes, byte 1
    bytes[pos + 40] = 0; // external attributes, byte 2 (high word low byte)
    bytes[pos + 41] = 0; // external attributes, byte 3 (high word high byte)
    fs::write(path, bytes).unwrap();
}

/// A Dos-origin zip entry (no unix "version made by" host) with the DOS
/// read-only attribute bit set still carries a nonzero `unix_mode()` —
/// `zip` synthesizes `S_IFREG | 0o444` for it rather than returning `None`.
/// This port accepts that synthesized mode (see the doc comment on the
/// permission-application block in `extract_zip`), matching what `unzip`
/// itself does; it does not restrict permission application to Unix-origin
/// archives only, since the `zip` crate does not expose the creator system
/// on read.
#[cfg(unix)]
#[test]
fn extract_zip_entry_with_dos_origin_readonly_attributes_applies_synthesized_mode() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("dos.zip");
    write_zip(&archive, &[("readonly.txt", b"body" as &[u8])]);
    patch_central_directory_dos_attrs(&archive, 0x01); // DOS read-only bit
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    let mode = fs::metadata(dest.join("readonly.txt"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o444,
        "expected the synthesized DOS read-only mode, got {mode:o}"
    );
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

/// The executable bit on a 7z member, stored in the Windows attribute
/// field's upper 16 bits behind the `0x8000` Unix flag, has to survive
/// extraction — same rationale as the tar.gz and zip executable-bit tests
/// above.
#[cfg(unix)]
#[test]
fn extract_7z_preserves_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("emu.7z");
    write_7z_with_modes(
        &archive,
        &[
            ("redream", b"body" as &[u8], 0o755),
            ("readme.txt", b"body", 0o644),
        ],
    );
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    let mode_of = |name: &str| fs::metadata(dest.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode_of("redream"), 0o755);
    assert_eq!(mode_of("readme.txt"), 0o644);
}

/// A 7z entry's attribute-encoded mode may carry the setuid bit; only
/// `mode & 0o777` may be applied, mirroring the zip setuid-stripping test.
#[cfg(unix)]
#[test]
fn extract_7z_entry_with_setuid_mode_strips_setuid_bit() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("emu.7z");
    write_7z_with_modes(&archive, &[("suspicious", b"body" as &[u8], 0o4755)]);
    let dest = dir.path().join("out");

    extract_archive(&archive, &dest, &mut |_, _| {}).unwrap();

    let mode = fs::metadata(dest.join("suspicious"))
        .unwrap()
        .permissions()
        .mode()
        & 0o7777;
    assert_eq!(mode, 0o755, "setuid bit must be stripped, got {mode:o}");
}

// --- should_extract / is_arcade_platform (behavioral smoke test; the full
// table lives as a unit test inside extract.rs) --------------------------------

#[test]
fn should_extract_is_reexported_and_callable() {
    assert!(should_extract("SNES", Path::new("game.zip")));
    assert!(!should_extract("Arcade", Path::new("game.zip")));
    assert!(should_extract("SNES", Path::new("game.rar")));
}

#[test]
fn should_extract_follows_the_python_table() {
    let cases: &[(&str, &str, bool)] = &[
        ("Windows", "game.exe", true),
        ("Windows", "game.iso", true),
        ("Arcade", "game.zip", false),
        ("PlayStation 3", "game.rar", true),
        ("SNES", "game.rar", true),
        ("SNES", "game.bin", false),
    ];
    for (platform, archive, expected) in cases {
        assert_eq!(
            should_extract(platform, Path::new(archive)),
            *expected,
            "platform={platform} archive={archive}"
        );
    }
}

// --- RAR ---------------------------------------------------------------------

#[test]
fn extracts_the_rar_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rar/version.rar");
    let archive = dir.path().join("version.rar");
    fs::copy(&fixture, &archive).unwrap();
    let dest = dir.path().join("out");

    let mut calls: Vec<(u64, u64)> = Vec::new();
    extract_archive(&archive, &dest, &mut |processed, total| {
        calls.push((processed, total));
    })
    .unwrap();

    assert_eq!(fs::read(dest.join("VERSION")).unwrap(), b"unrar-0.4.0");

    let (last_processed, last_total) = *calls.last().unwrap();
    assert_eq!(last_processed, 11);
    assert_eq!(last_total, 11);
}
