//! Per-file write dispatch for [`super::install_platform_firmware`]:
//! `.7z`/`.rar` (flat copy via the shared extractor), `.zip` (keep-as-archive
//! or member extraction, flat or with paths), and everything else (a raw
//! write). Ports firmware_install.py:138-215
//! (`docs/porting/03-library-install.md` §18 step 7).

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use super::FirmwareOptions;
use crate::library::extract::extract_archive;

/// Whether `data` starts with one of the three zip local-file-header /
/// empty-archive / spanned-archive signatures (`"PK\x03\x04"`,
/// `"PK\x05\x06"`, `"PK\x07\x08"`) — the same content sniff Python's
/// `zipfile.is_zipfile` performs (firmware_install.py:171). Used so a zip
/// whose file name doesn't end in `.zip` is still extracted as one.
pub(crate) fn is_zip_bytes(data: &[u8]) -> bool {
    data.starts_with(b"PK\x03\x04")
        || data.starts_with(b"PK\x05\x06")
        || data.starts_with(b"PK\x07\x08")
}

/// Whether `file_name` is safe to join onto a firmware target directory:
/// a single plain path component, so the join can only ever produce a file
/// directly inside that directory.
///
/// SECURITY: `file_name` comes from the server's firmware record and is
/// never validated by the API layer. A record named `../../x`, `/abs/x` or
/// `a/b.bin` would otherwise write outside the routed firmware directory.
/// The check is deliberately strict — no separator of either flavour, no
/// `.`/`..`, and the name must survive a `Path::file_name()` round trip
/// unchanged — because firmware records legitimately carry bare file names
/// only.
pub(crate) fn is_plain_file_name(file_name: &str) -> bool {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains('\0')
        || file_name == "."
        || file_name == ".."
    {
        return false;
    }
    Path::new(file_name).file_name() == Some(std::ffi::OsStr::new(file_name))
}

/// The warning text for a firmware record whose file name failed
/// [`is_plain_file_name`]. Uses the same
/// `"Failed to write firmware <name> to <dest>: <error>"` shape as every
/// other write failure (firmware_install.py:180-183) so the drawer and the
/// log read the same either way.
pub(crate) fn invalid_name_warning(file_name: &str, target_dir: &Path) -> String {
    format!(
        "Failed to write firmware {file_name} to {}: invalid firmware file name",
        target_dir.display()
    )
}

/// Writes one downloaded firmware file into `target_dir`, dispatching by
/// extension/content: `.7z`/`.rar` extract-and-flatten via the shared
/// extractor; a `.zip` (by name or by sniffed content) is kept as-is when
/// `keep_archive`, else its members are extracted flat or with paths per
/// `opts.extract_zip_with_paths`; anything else is written raw. Every
/// branch honors `opts.skip_existing`. Returns the fully-formatted warning
/// string on failure (never a raw error) so the caller only has to push it
/// onto its warning list.
pub(crate) fn write_firmware_file(
    file_name: &str,
    data: &[u8],
    target_dir: &Path,
    keep_archive: bool,
    opts: FirmwareOptions,
) -> Result<(), String> {
    // Defense in depth: `install_platform_firmware` already rejects a
    // hostile record before it downloads anything, so this only fires for a
    // direct caller. Both paths produce the identical warning text.
    if !is_plain_file_name(file_name) {
        return Err(invalid_name_warning(file_name, target_dir));
    }
    let lower = file_name.to_lowercase();

    if lower.ends_with(".7z") {
        return write_archive_flat(file_name, data, target_dir, opts, ".7z");
    }
    if lower.ends_with(".rar") {
        return write_archive_flat(file_name, data, target_dir, opts, ".rar");
    }

    if lower.ends_with(".zip") || is_zip_bytes(data) {
        if keep_archive && lower.ends_with(".zip") {
            let dest = target_dir.join(file_name);
            return write_raw_bytes(file_name, data, &dest, opts);
        }
        return write_zip_members(file_name, data, target_dir, opts);
    }

    let dest = target_dir.join(file_name);
    write_raw_bytes(file_name, data, &dest, opts)
}

/// Writes `data` to `dest` as-is, skipping when `opts.skip_existing` and
/// `dest` already exists. Shared by the raw (non-archive) branch and the
/// zip keep-as-archive branch — both use the exact same warning text
/// (firmware_install.py:180-183, :211-215) — built here exactly once.
fn write_raw_bytes(
    file_name: &str,
    data: &[u8],
    dest: &Path,
    opts: FirmwareOptions,
) -> Result<(), String> {
    if opts.skip_existing && dest.exists() {
        return Ok(());
    }
    fs::write(dest, data).map_err(|e| {
        format!(
            "Failed to write firmware {file_name} to {}: {e}",
            dest.display()
        )
    })
}

/// Builds the "Failed to extract firmware archive" warning text — shared by
/// the `.7z`/`.rar` branch and the `.zip` member-extraction branch, the
/// only two places that can fail partway through an archive read
/// (firmware_install.py:166-167, :208-210). Built here exactly once so the
/// two call sites can never drift apart.
fn extract_warning(file_name: &str, e: String) -> String {
    format!("Failed to extract firmware archive {file_name}: {e}")
}

/// `.7z`/`.rar` branch (firmware_install.py:138-168): write `data` to a
/// temp file with the matching suffix (so the shared extractor's
/// suffix-based dispatch picks the right format), extract it into a fresh
/// temp staging directory, then copy every regular file flat into
/// `target_dir`, skipping any member with a `__MACOSX` path component or a
/// `.DS_Store` file name, and (honoring `skip_existing`) files that
/// already exist. The temp file and staging directory are removed
/// automatically when they drop, mirroring the Python `finally: unlink`.
fn write_archive_flat(
    file_name: &str,
    data: &[u8],
    target_dir: &Path,
    opts: FirmwareOptions,
    suffix: &str,
) -> Result<(), String> {
    let fail = |e: String| extract_warning(file_name, e);

    let mut tmp = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .map_err(|e| fail(e.to_string()))?;
    tmp.write_all(data).map_err(|e| fail(e.to_string()))?;

    let staging = tempfile::tempdir().map_err(|e| fail(e.to_string()))?;
    let mut progress = |_processed: u64, _total: u64| {};
    extract_archive(tmp.path(), staging.path(), &mut progress).map_err(|e| fail(e.to_string()))?;

    let extracted = collect_regular_files(staging.path()).map_err(|e| fail(e.to_string()))?;
    for entry in extracted {
        if entry.components().any(|c| c.as_os_str() == "__MACOSX") {
            continue;
        }
        let Some(name) = entry.file_name() else {
            continue;
        };
        if name == std::ffi::OsStr::new(".DS_Store") {
            continue;
        }
        let dest = target_dir.join(name);
        if opts.skip_existing && dest.exists() {
            continue;
        }
        fs::copy(&entry, &dest).map_err(|e| fail(e.to_string()))?;
    }
    Ok(())
}

/// Recursively lists every regular file under `dir`. Does not follow
/// symlinks — a symlinked entry is neither a plain file nor a plain
/// directory by `fs::read_dir`'s (non-following) `file_type()`, so it is
/// silently skipped rather than copied or descended into. The staging
/// directory is always freshly produced by our own extractor, so this is
/// purely defensive.
fn collect_regular_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                out.push(entry.path());
            }
        }
    }
    Ok(out)
}

/// `.zip` member-extraction branch (firmware_install.py:184-210), used
/// whenever the archive is not being kept as-is. Skips directory entries
/// (names ending `/`) and anything under `__MACOSX`. With
/// `opts.extract_zip_with_paths`, backslashes are normalized to `/` and a
/// member that is empty, absolute, or contains a `..` component is
/// skipped (path-traversal guard) and parent directories are created;
/// otherwise every member is flattened to its base name via
/// [`flat_member_name`]. Every write honors `opts.skip_existing`.
fn write_zip_members(
    file_name: &str,
    data: &[u8],
    target_dir: &Path,
    opts: FirmwareOptions,
) -> Result<(), String> {
    let fail = |e: String| extract_warning(file_name, e);

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| fail(e.to_string()))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| fail(e.to_string()))?;
        let raw_name = entry.name().to_string();
        if raw_name.ends_with('/') || raw_name.starts_with("__MACOSX") {
            continue;
        }

        let dest = if opts.extract_zip_with_paths {
            match safe_relative_path(&raw_name) {
                Some(relative) => target_dir.join(relative),
                None => continue,
            }
        } else {
            match flat_member_name(&raw_name) {
                Some(name) => target_dir.join(name),
                None => continue,
            }
        };

        if opts.skip_existing && dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| fail(e.to_string()))?;
        }
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| fail(e.to_string()))?;
        fs::write(&dest, &buf).map_err(|e| fail(e.to_string()))?;
    }

    Ok(())
}

/// Path-traversal guard for an `extract_zip_with_paths` member
/// (firmware_install.py:187-194): backslashes normalized to `/`, then
/// `None` when the result is empty, absolute, or contains a `..`
/// component.
fn safe_relative_path(raw_name: &str) -> Option<PathBuf> {
    let normalized = raw_name.replace('\\', "/");
    let path = PathBuf::from(&normalized);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return None;
    }
    let mut any_component = false;
    for component in path.components() {
        any_component = true;
        if component == Component::ParentDir {
            return None;
        }
    }
    if !any_component {
        return None;
    }
    Some(path)
}

/// Flattens a zip member name to its base (file) name
/// (firmware_install.py:205-206): `Path(member).name`. `None` when that is
/// empty (a member with no base-name component) — note this uses the raw,
/// non-backslash-normalized member name, matching the Python source's flat
/// branch exactly.
fn flat_member_name(raw_name: &str) -> Option<&str> {
    let name = Path::new(raw_name).file_name()?.to_str()?;
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for &(name, content) in entries {
                zip.start_file(name, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
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

    // --- is_zip_bytes ---------------------------------------------------------

    #[test]
    fn recognizes_all_three_zip_signatures() {
        assert!(is_zip_bytes(b"PK\x03\x04rest"));
        assert!(is_zip_bytes(b"PK\x05\x06rest"));
        assert!(is_zip_bytes(b"PK\x07\x08rest"));
        assert!(!is_zip_bytes(b"not-a-zip"));
    }

    // --- hostile file names -----------------------------------------------------

    /// SECURITY: the server-supplied record name must never escape the
    /// routed firmware directory. Every rejected shape produces the write
    /// warning and leaves the target directory untouched.
    #[test]
    fn a_file_name_that_is_not_a_plain_component_is_rejected() {
        for name in [
            "../x", "../../x", "/abs/x", "a/b.bin", ".", "..", "", "a\\b.bin",
        ] {
            let root = tempfile::tempdir().unwrap();
            let target_dir = root.path().join("target");
            fs::create_dir_all(&target_dir).unwrap();

            let err = write_firmware_file(
                name,
                b"HOSTILE",
                &target_dir,
                false,
                FirmwareOptions::default(),
            )
            .unwrap_err();
            assert_eq!(
                err,
                format!(
                    "Failed to write firmware {name} to {}: invalid firmware file name",
                    target_dir.display()
                ),
                "unexpected warning for {name:?}"
            );
            // Nothing anywhere under the temp root was created — neither
            // inside the target directory nor beside it.
            assert_eq!(
                fs::read_dir(&target_dir).unwrap().count(),
                0,
                "{name:?} wrote into the target directory"
            );
            assert_eq!(
                fs::read_dir(root.path()).unwrap().count(),
                1,
                "{name:?} wrote outside the target directory"
            );
        }
    }

    #[test]
    fn a_plain_file_name_is_accepted() {
        assert!(is_plain_file_name("gc-ntsc-12-101.bin"));
        assert!(is_plain_file_name("PS3UPDAT.PUP"));
        assert!(is_plain_file_name("...bin"));
        assert!(!is_plain_file_name("./x"));
        assert!(!is_plain_file_name("x\0y"));

        let dir = tempfile::tempdir().unwrap();
        write_firmware_file(
            "gc-ntsc-12-101.bin",
            b"RAWDATA",
            dir.path(),
            false,
            FirmwareOptions::default(),
        )
        .unwrap();
        assert_eq!(
            fs::read(dir.path().join("gc-ntsc-12-101.bin")).unwrap(),
            b"RAWDATA"
        );
    }

    // --- flat write -------------------------------------------------------------

    #[test]
    fn non_zip_file_is_written_raw() {
        let dir = tempfile::tempdir().unwrap();
        let opts = FirmwareOptions::default();
        write_firmware_file("gc.bin", b"RAWDATA", dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("gc.bin")).unwrap(), b"RAWDATA");
    }

    #[test]
    fn skip_existing_true_leaves_existing_raw_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("gc.bin"), b"ORIGINAL").unwrap();
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: false,
        };
        write_firmware_file("gc.bin", b"NEWDATA", dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("gc.bin")).unwrap(), b"ORIGINAL");
    }

    #[test]
    fn skip_existing_false_overwrites_raw_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("gc.bin"), b"ORIGINAL").unwrap();
        let opts = FirmwareOptions {
            skip_existing: false,
            extract_zip_with_paths: false,
        };
        write_firmware_file("gc.bin", b"NEWDATA", dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("gc.bin")).unwrap(), b"NEWDATA");
    }

    // --- zip: flat --------------------------------------------------------------

    #[test]
    fn zip_flat_extracts_member_to_base_name() {
        let dir = tempfile::tempdir().unwrap();
        let data = zip_bytes(&[("nested/IPL.bin", b"IPLDATA")]);
        let opts = FirmwareOptions::default();
        write_firmware_file("gc.zip", &data, dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("IPL.bin")).unwrap(), b"IPLDATA");
        assert!(!dir.path().join("nested").exists());
    }

    #[test]
    fn zip_flat_skip_existing_per_member() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("IPL.bin"), b"ORIGINAL").unwrap();
        let data = zip_bytes(&[("IPL.bin", b"NEW"), ("other.bin", b"OTHER")]);
        let opts = FirmwareOptions::default();
        write_firmware_file("gc.zip", &data, dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("IPL.bin")).unwrap(), b"ORIGINAL");
        assert_eq!(fs::read(dir.path().join("other.bin")).unwrap(), b"OTHER");
    }

    #[test]
    fn zip_flat_skips_macosx_entries() {
        let dir = tempfile::tempdir().unwrap();
        let data = zip_bytes(&[("__MACOSX/._IPL.bin", b"META"), ("IPL.bin", b"IPLDATA")]);
        let opts = FirmwareOptions::default();
        write_firmware_file("gc.zip", &data, dir.path(), false, opts).unwrap();
        assert!(fs::read(dir.path().join("IPL.bin")).is_ok());
        assert!(!dir.path().join("._IPL.bin").exists());
        assert!(!dir.path().join("__MACOSX").exists());
    }

    #[test]
    fn keep_archive_writes_zip_bytes_raw() {
        let dir = tempfile::tempdir().unwrap();
        let data = zip_bytes(&[("epr-21576h.ic27", b"ROMDATA")]);
        let opts = FirmwareOptions::default();
        write_firmware_file("naomi.zip", &data, dir.path(), true, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("naomi.zip")).unwrap(), data);
        assert!(!dir.path().join("epr-21576h.ic27").exists());
    }

    #[test]
    fn bad_zip_bytes_yield_the_exact_warning_text() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"PK\x03\x04this-is-not-a-valid-archive".to_vec();
        let opts = FirmwareOptions::default();
        let err = write_firmware_file("gc_ntsc.zip", &data, dir.path(), false, opts).unwrap_err();
        assert!(
            err.starts_with("Failed to extract firmware archive gc_ntsc.zip: "),
            "unexpected warning: {err}"
        );
    }

    // --- zip: with paths --------------------------------------------------------

    #[test]
    fn zip_with_paths_extracts_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let data = zip_bytes(&[
            ("dolphin-emu/User/GC/USA/IPL.bin", b"USA"),
            ("dolphin-emu/User/GC/EUR/IPL.bin", b"EUR"),
        ]);
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: true,
        };
        write_firmware_file("dolphin-gc-bios.zip", &data, dir.path(), false, opts).unwrap();
        assert_eq!(
            fs::read(dir.path().join("dolphin-emu/User/GC/USA/IPL.bin")).unwrap(),
            b"USA"
        );
        assert_eq!(
            fs::read(dir.path().join("dolphin-emu/User/GC/EUR/IPL.bin")).unwrap(),
            b"EUR"
        );
    }

    #[test]
    fn zip_with_paths_skips_traversal_members() {
        let temp_root = tempfile::tempdir().unwrap();
        let target_dir = temp_root.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        let outside_path = temp_root.path().join("outside_passwd");

        let data = zip_bytes(&[
            ("../../outside_passwd", b"BAD"),
            ("dolphin-emu/User/GC/USA/IPL.bin", b"USA"),
        ]);
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: true,
        };
        write_firmware_file("dolphin-gc-bios.zip", &data, &target_dir, false, opts).unwrap();
        assert!(!outside_path.exists());
        assert_eq!(
            fs::read(target_dir.join("dolphin-emu/User/GC/USA/IPL.bin")).unwrap(),
            b"USA"
        );
    }

    #[test]
    fn zip_with_paths_skips_macosx_entries() {
        let dir = tempfile::tempdir().unwrap();
        let data = zip_bytes(&[
            ("__MACOSX/._IPL.bin", b"META"),
            ("dolphin-emu/User/GC/USA/IPL.bin", b"USA"),
        ]);
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: true,
        };
        write_firmware_file("dolphin-gc-bios.zip", &data, dir.path(), false, opts).unwrap();
        assert!(!dir.path().join("__MACOSX").exists());
        assert_eq!(
            fs::read(dir.path().join("dolphin-emu/User/GC/USA/IPL.bin")).unwrap(),
            b"USA"
        );
    }

    #[test]
    fn zip_with_paths_skip_existing_respects_nested_path() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("dolphin-emu/User/GC/USA/IPL.bin");
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, b"ORIGINAL").unwrap();
        let data = zip_bytes(&[("dolphin-emu/User/GC/USA/IPL.bin", b"NEW")]);
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: true,
        };
        write_firmware_file("dolphin-gc-bios.zip", &data, dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(&existing).unwrap(), b"ORIGINAL");
    }

    // --- 7z: flat copy ------------------------------------------------------------

    #[test]
    fn sevenz_archive_is_copied_flat_skipping_ds_store_and_macosx() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("unrelated.bin");
        fs::write(&existing, b"KEEP ME").unwrap();

        let archive_dir = tempfile::tempdir().unwrap();
        let archive_path = archive_dir.path().join("firmware.7z");
        write_7z(
            &archive_path,
            &[
                ("nested/IPL.bin", b"IPLDATA"),
                ("nested/.DS_Store", b"DSSTORE"),
                ("__MACOSX/._IPL.bin", b"META"),
            ],
        );
        let data = fs::read(&archive_path).unwrap();

        let opts = FirmwareOptions::default();
        write_firmware_file("firmware.7z", &data, dir.path(), false, opts).unwrap();

        assert_eq!(fs::read(dir.path().join("IPL.bin")).unwrap(), b"IPLDATA");
        assert!(!dir.path().join(".DS_Store").exists());
        assert!(!dir.path().join("__MACOSX").exists());
        assert!(!dir.path().join("._IPL.bin").exists());
        // The unrelated pre-existing file in the target directory is
        // untouched by the extraction.
        assert_eq!(fs::read(&existing).unwrap(), b"KEEP ME");
    }

    #[test]
    fn sevenz_archive_skip_existing_true_keeps_original() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("IPL.bin"), b"ORIGINAL").unwrap();

        let archive_dir = tempfile::tempdir().unwrap();
        let archive_path = archive_dir.path().join("firmware.7z");
        write_7z(&archive_path, &[("IPL.bin", b"NEWDATA")]);
        let data = fs::read(&archive_path).unwrap();

        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: false,
        };
        write_firmware_file("firmware.7z", &data, dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("IPL.bin")).unwrap(), b"ORIGINAL");
    }

    #[test]
    fn sevenz_archive_skip_existing_false_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("IPL.bin"), b"ORIGINAL").unwrap();

        let archive_dir = tempfile::tempdir().unwrap();
        let archive_path = archive_dir.path().join("firmware.7z");
        write_7z(&archive_path, &[("IPL.bin", b"NEWDATA")]);
        let data = fs::read(&archive_path).unwrap();

        let opts = FirmwareOptions {
            skip_existing: false,
            extract_zip_with_paths: false,
        };
        write_firmware_file("firmware.7z", &data, dir.path(), false, opts).unwrap();
        assert_eq!(fs::read(dir.path().join("IPL.bin")).unwrap(), b"NEWDATA");
    }
}
