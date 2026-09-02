//! The xemu raw-disk cloud-save bridge: HDD image classification, block
//! reasons, `hdd_path` resolution from `xemu.toml`, and the `UDATA`/`TDATA`
//! archive build/inject pair.
//!
//! Spec: `docs/superpowers/specs/2026-09-02-cloud-saves-design.md`, "xemu
//! flow" and "Deviations" D1-D3. There is no Python original for any of
//! this — the Python app synced the whole HDD image as a single archive
//! (the path deviation D1 removes entirely); this module is the raw-disk
//! replacement, built on the clean-room [`crate::fatx`] module.
//!
//! Every image touched here is the standard retail `E:` (data) partition —
//! [`RETAIL_PARTITION_E_OFFSET`]/[`RETAIL_PARTITION_E_SIZE`] — of a raw
//! (non-qcow2) HDD image (D1: GRID ships no qcow2 decoder or conversion).

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::fatx::image::FatxPartition;
use crate::fatx::layout::{RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE};

use super::archive::{extract_payload_zip, zip_dirs_with_prefixes};
use super::IgnoreSets;

/// The four bytes at offset 0 of a qcow2 image: `"QFI\xfb"`.
const QCOW2_MAGIC: [u8; 4] = [0x51, 0x46, 0x49, 0xFB];

/// [`inject_xemu_save_archive`]'s D2 notice for a legacy whole-image
/// record — byte-exact, user-facing (spec "xemu flow").
const LEGACY_RECORD_NOTICE: &str = "This cloud save is a legacy whole-image xemu backup and cannot be restored by this version. Upload a new save to replace it.";

/// The outcome of sniffing a configured `hdd_path` for xemu cloud sync
/// (spec "xemu flow", block reasons `xemu-image-not-raw` /
/// `xemu-image-unsupported-layout` / `xemu-image-missing`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XemuImageStatus {
    /// A raw image whose `E:` FATX superblock validates: sync ready.
    Ready,
    /// The qcow2 magic was found at offset 0 (or any other non-FATX-raw
    /// leading bytes reached by the same sniff) — not a raw image.
    NotRaw,
    /// Raw-looking (no qcow2 magic), but the `E:` superblock/geometry does
    /// not validate as a standard retail-layout FATX partition.
    UnsupportedLayout,
    /// `hdd_path` is blank, or names a file that does not exist.
    Missing,
}

/// Classify `hdd_path` for xemu cloud sync. Order (spec "xemu flow"):
/// blank/absent path -> [`XemuImageStatus::Missing`]; a leading qcow2
/// magic -> [`XemuImageStatus::NotRaw`]; else
/// [`FatxPartition::validate`] at the retail `E:` offset/size decides
/// [`XemuImageStatus::Ready`] vs. [`XemuImageStatus::UnsupportedLayout`].
///
/// The sniff reads at most 4 bytes regardless of image size — cheap even
/// on a multi-gigabyte HDD image — and a read failure (permissions, a
/// file shorter than 4 bytes, ...) is treated the same as "not the qcow2
/// magic": classification falls through to the FATX validation, which has
/// its own, more informative failure modes.
pub fn classify_hdd_image(hdd_path: &str) -> XemuImageStatus {
    let trimmed = hdd_path.trim();
    if trimmed.is_empty() {
        return XemuImageStatus::Missing;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        return XemuImageStatus::Missing;
    }

    if let Ok(mut file) = File::open(path) {
        let mut magic = [0u8; 4];
        if file.read_exact(&mut magic).is_ok() && magic == QCOW2_MAGIC {
            return XemuImageStatus::NotRaw;
        }
    }

    match FatxPartition::validate(path, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE) {
        Ok(()) => XemuImageStatus::Ready,
        Err(_) => XemuImageStatus::UnsupportedLayout,
    }
}

/// The user-facing block reason for `status`, `None` for
/// [`XemuImageStatus::Ready`]. Byte-exact strings pinned by the task brief
/// / spec "xemu flow".
pub fn block_reason_for_status(status: &XemuImageStatus) -> Option<String> {
    match status {
        XemuImageStatus::Ready => None,
        XemuImageStatus::NotRaw => Some(
            "xemu cloud sync needs a raw HDD image (xbox_hdd.img). Convert your qcow2 once with: qemu-img convert -O raw xbox_hdd.qcow2 xbox_hdd.img"
                .to_string(),
        ),
        XemuImageStatus::UnsupportedLayout => Some(
            "The xemu HDD image is not a standard retail-layout FATX image, so cloud sync is unavailable."
                .to_string(),
        ),
        XemuImageStatus::Missing => Some(
            "No xemu HDD image is configured, so cloud sync is unavailable.".to_string(),
        ),
    }
}

/// Resolve `sys.files.hdd_path` out of the resolved `xemu.toml` (the same
/// file [`crate::autoconfig::xemu::ensure_settings`] targets for
/// `emulator_path`), via the milestone-5 xemu module's minimal reader.
/// `None` when the file, section, or key is absent, or the value is
/// blank.
pub fn xemu_hdd_path_from_config(emulator_path: &str) -> Option<String> {
    crate::autoconfig::xemu::hdd_path_from_config(emulator_path)
}

/// Extract `E:/UDATA` and `E:/TDATA` from the raw image at `hdd_path` into
/// a temp directory laid out as `UDATA/...`/`TDATA/...`, then zip that
/// directory into one temp archive (spec "xemu flow" upload). `title`
/// names the archive via [`super::archive::temp_archive_path`].
///
/// Zero files across both trees -> `Ok(None)`, with no archive left
/// behind (the zip is never even created in that case).
///
/// Any failure — opening the image, extracting a tree, or zipping —
/// yields `Err` with a description; the image is opened read-only
/// throughout, so a failure here never touches it.
pub fn build_xemu_save_archive(
    hdd_path: &str,
    title: &str,
) -> Result<Option<(PathBuf, usize)>, String> {
    let mut part = FatxPartition::open(
        Path::new(hdd_path),
        RETAIL_PARTITION_E_OFFSET,
        RETAIL_PARTITION_E_SIZE,
    )
    .map_err(|e| e.to_string())?;

    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let udata_dir = scratch.path().join("UDATA");
    let tdata_dir = scratch.path().join("TDATA");
    let udata_files = part
        .read_tree("UDATA", &udata_dir)
        .map_err(|e| e.to_string())?;
    let tdata_files = part
        .read_tree("TDATA", &tdata_dir)
        .map_err(|e| e.to_string())?;

    if udata_files + tdata_files == 0 {
        return Ok(None);
    }

    let (archive_path, files) = zip_dirs_with_prefixes(
        &[
            ("UDATA", udata_dir.as_path()),
            ("TDATA", tdata_dir.as_path()),
        ],
        &IgnoreSets::default(),
        title,
    )
    .map_err(|e| e.to_string())?;

    Ok(Some((archive_path, files)))
}

/// True when `payload`'s zip top level contains a `UDATA/` or `TDATA/`
/// member — the new-format check for D2. Non-zip bytes, and a zip whose
/// members are all something else (a legacy whole-image record's single
/// `*.qcow2`/`*.img` member), both return `false`.
pub fn archive_is_udata_tdata(payload: &[u8]) -> bool {
    let Ok(mut archive) = ZipArchive::new(Cursor::new(payload)) else {
        return false;
    };
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name();
        if name == "UDATA"
            || name == "TDATA"
            || name.starts_with("UDATA/")
            || name.starts_with("TDATA/")
        {
            return true;
        }
    }
    false
}

/// Extract `payload` to a temp directory, then [`FatxPartition::write_tree`]
/// its `UDATA` and `TDATA` subdirectories into the raw image at
/// `hdd_path`'s `E:` partition — overwrite semantics, matching
/// `write_tree`'s own (spec "xemu flow" restore).
///
/// A legacy payload ([`archive_is_udata_tdata`] false, whether because it
/// isn't a zip at all or because it's an old whole-image record) is
/// rejected up front with the D2 notice, before the image is opened for
/// writing at all — the image is never touched for a legacy payload.
///
/// A payload holding only one of the two roots (a real image can have an
/// empty `TDATA`, say) writes only that one; the other is left alone.
/// Returns the total files written.
pub fn inject_xemu_save_archive(hdd_path: &str, payload: &[u8]) -> Result<usize, String> {
    if !archive_is_udata_tdata(payload) {
        return Err(LEGACY_RECORD_NOTICE.to_string());
    }

    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    extract_payload_zip(payload, scratch.path(), &IgnoreSets::default())?;

    let mut part = FatxPartition::open_rw(
        Path::new(hdd_path),
        RETAIL_PARTITION_E_OFFSET,
        RETAIL_PARTITION_E_SIZE,
    )
    .map_err(|e| e.to_string())?;

    let mut written = 0usize;
    for root in ["UDATA", "TDATA"] {
        let src = scratch.path().join(root);
        if src.is_dir() {
            written += part.write_tree(root, &src).map_err(|e| e.to_string())?;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fatx::builder::FatxImageBuilder;
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn empty_retail_image(dir: &Path) -> PathBuf {
        let img = dir.join("xbox_hdd.img");
        FatxImageBuilder::new(RETAIL_PARTITION_E_SIZE)
            .with_base_offset(RETAIL_PARTITION_E_OFFSET)
            .with_cluster_size(16 * 1024)
            .write_to(&img)
            .expect("build empty retail image");
        img
    }

    fn populated_retail_image(dir: &Path) -> PathBuf {
        let img = dir.join("xbox_hdd.img");
        let mut b = FatxImageBuilder::new(RETAIL_PARTITION_E_SIZE)
            .with_base_offset(RETAIL_PARTITION_E_OFFSET)
            .with_cluster_size(16 * 1024);
        b.add_file("UDATA/4541000d/00000001/savedata.bin", vec![0xA5; 100]);
        b.add_file("TDATA/4541000d/settings.bin", b"cfg".to_vec());
        b.write_to(&img).expect("build populated retail image");
        img
    }

    fn zip_with_one_member(name: &str, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut bytes));
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }

    // --- classify_hdd_image -------------------------------------------

    #[test]
    fn classify_detects_qcow2_magic_ready_missing_and_bad_layout() {
        let tmp = tempfile::tempdir().unwrap();

        assert_eq!(classify_hdd_image(""), XemuImageStatus::Missing);
        assert_eq!(classify_hdd_image("   "), XemuImageStatus::Missing);
        assert_eq!(
            classify_hdd_image(tmp.path().join("nope.img").to_str().unwrap()),
            XemuImageStatus::Missing
        );

        let qcow2 = tmp.path().join("xbox_hdd.qcow2");
        let mut bytes = QCOW2_MAGIC.to_vec();
        bytes.extend_from_slice(b"rest of a fake qcow2 header, contents irrelevant");
        std::fs::write(&qcow2, &bytes).unwrap();
        assert_eq!(
            classify_hdd_image(qcow2.to_str().unwrap()),
            XemuImageStatus::NotRaw
        );

        let bad = tmp.path().join("bad.img");
        std::fs::write(&bad, b"not a fatx image at all, and not qcow2 either").unwrap();
        assert_eq!(
            classify_hdd_image(bad.to_str().unwrap()),
            XemuImageStatus::UnsupportedLayout
        );

        let good = empty_retail_image(tmp.path());
        assert_eq!(
            classify_hdd_image(good.to_str().unwrap()),
            XemuImageStatus::Ready
        );
    }

    // --- block_reason_for_status ---------------------------------------

    #[test]
    fn block_reason_strings_are_exact() {
        assert_eq!(block_reason_for_status(&XemuImageStatus::Ready), None);
        assert_eq!(
            block_reason_for_status(&XemuImageStatus::NotRaw),
            Some(
                "xemu cloud sync needs a raw HDD image (xbox_hdd.img). Convert your qcow2 once with: qemu-img convert -O raw xbox_hdd.qcow2 xbox_hdd.img"
                    .to_string()
            )
        );
        assert_eq!(
            block_reason_for_status(&XemuImageStatus::UnsupportedLayout),
            Some(
                "The xemu HDD image is not a standard retail-layout FATX image, so cloud sync is unavailable."
                    .to_string()
            )
        );
        assert_eq!(
            block_reason_for_status(&XemuImageStatus::Missing),
            Some("No xemu HDD image is configured, so cloud sync is unavailable.".to_string())
        );
    }

    // --- xemu_hdd_path_from_config --------------------------------------

    #[test]
    fn hdd_path_read_from_xemu_toml_single_quotes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("xemu");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("xemu.exe");
        std::fs::write(&exe, b"").unwrap();

        // No xemu.toml yet.
        assert_eq!(xemu_hdd_path_from_config(exe.to_str().unwrap()), None);

        let raw_path = dir.join("xbox_hdd.img");
        std::fs::write(
            dir.join("xemu.toml"),
            format!(
                "[general]\nshow_welcome = false\n\n[sys.files]\nhdd_path = '{}'\neeprom_path = '{}'\n",
                raw_path.display(),
                dir.join("eeprom.bin").display()
            ),
        )
        .unwrap();

        assert_eq!(
            xemu_hdd_path_from_config(exe.to_str().unwrap()),
            Some(raw_path.to_string_lossy().to_string())
        );
    }

    // --- build_xemu_save_archive -----------------------------------------

    #[test]
    fn build_archive_lays_out_udata_and_tdata_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let img = populated_retail_image(tmp.path());

        let (archive_path, files) = build_xemu_save_archive(img.to_str().unwrap(), "My Game")
            .expect("build archive")
            .expect("non-empty trees produce an archive");
        assert_eq!(files, 2);
        assert!(archive_path.exists());

        let bytes = std::fs::read(&archive_path).unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "TDATA/4541000d/settings.bin".to_string(),
                "UDATA/4541000d/00000001/savedata.bin".to_string(),
            ]
        );

        std::fs::remove_file(&archive_path).ok();
    }

    #[test]
    fn build_archive_returns_none_when_both_trees_are_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let img = empty_retail_image(tmp.path());

        let result = build_xemu_save_archive(img.to_str().unwrap(), "Empty Game Xyzzy").unwrap();
        assert!(result.is_none());

        let leftovers: Vec<_> = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("Empty Game Xyzzy-")
            })
            .collect();
        assert!(leftovers.is_empty(), "leftover archive(s): {leftovers:?}");
    }

    // --- archive_is_udata_tdata -------------------------------------------

    #[test]
    fn archive_format_check_accepts_new_and_rejects_legacy() {
        assert!(!archive_is_udata_tdata(b"not a zip at all"));

        let legacy = zip_with_one_member("something.qcow2", b"fake whole-image bytes");
        assert!(!archive_is_udata_tdata(&legacy));

        let new_format = zip_with_one_member("UDATA/notes.txt", b"hi");
        assert!(archive_is_udata_tdata(&new_format));

        let tdata_only = zip_with_one_member("TDATA/settings.bin", b"cfg");
        assert!(archive_is_udata_tdata(&tdata_only));
    }

    // --- inject_xemu_save_archive -------------------------------------------

    #[test]
    fn inject_writes_both_trees_and_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let img = populated_retail_image(tmp.path());

        let (archive_path, files) = build_xemu_save_archive(img.to_str().unwrap(), "Roundtrip")
            .unwrap()
            .unwrap();
        assert_eq!(files, 2);
        let payload = std::fs::read(&archive_path).unwrap();
        std::fs::remove_file(&archive_path).ok();

        // Wipe both trees on the image.
        {
            let mut part =
                FatxPartition::open_rw(&img, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE)
                    .unwrap();
            part.remove_tree("UDATA").unwrap();
            part.remove_tree("TDATA").unwrap();
        }
        {
            let mut part =
                FatxPartition::open(&img, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE)
                    .unwrap();
            assert!(part.list_dir("").unwrap().is_empty());
        }

        let written = inject_xemu_save_archive(img.to_str().unwrap(), &payload).unwrap();
        assert_eq!(written, 2);

        {
            let mut part =
                FatxPartition::open(&img, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE)
                    .unwrap();
            let dest = tmp.path().join("out");
            assert_eq!(part.read_tree("UDATA", &dest.join("UDATA")).unwrap(), 1);
            assert_eq!(part.read_tree("TDATA", &dest.join("TDATA")).unwrap(), 1);
            assert_eq!(
                std::fs::read(dest.join("UDATA/4541000d/00000001/savedata.bin")).unwrap(),
                vec![0xA5u8; 100]
            );
            assert_eq!(
                std::fs::read(dest.join("TDATA/4541000d/settings.bin")).unwrap(),
                b"cfg"
            );
        }

        // Inject again: overwrite, not "already exists".
        let written_again = inject_xemu_save_archive(img.to_str().unwrap(), &payload).unwrap();
        assert_eq!(written_again, 2);
    }

    #[test]
    fn inject_rejects_a_legacy_archive_with_the_d2_notice() {
        let tmp = tempfile::tempdir().unwrap();
        let img = populated_retail_image(tmp.path());

        let legacy = zip_with_one_member("xbox_hdd.qcow2", b"whole legacy image bytes");

        let err = inject_xemu_save_archive(img.to_str().unwrap(), &legacy).unwrap_err();
        assert_eq!(err, LEGACY_RECORD_NOTICE);

        // Non-zip bytes are rejected the same way.
        let err_non_zip =
            inject_xemu_save_archive(img.to_str().unwrap(), b"not a zip").unwrap_err();
        assert_eq!(err_non_zip, LEGACY_RECORD_NOTICE);

        // The image is untouched by either rejected attempt.
        let mut part =
            FatxPartition::open(&img, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE).unwrap();
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest.join("UDATA")).unwrap(), 1);
        assert_eq!(part.read_tree("TDATA", &dest.join("TDATA")).unwrap(), 1);
    }
}
