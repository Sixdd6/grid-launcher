//! Server firmware download and install: keyword-based target routing, the
//! zip keep-vs-extract decision, and [`install_platform_firmware`]. Ports
//! `grid_launcher/library/firmware_install.py` lines 1-221
//! (`fetch_platform_firmware`, `download_firmware_bytes`,
//! `resolve_firmware_targets`, `should_keep_zip_archive`,
//! `install_platform_firmware`) — see `docs/porting/03-library-install.md`
//! §18 steps 1-9. The PS3-direct-from-Sony path
//! (firmware_install.py:224-334) is ruled out (design decision D2) and is
//! not ported here.
//!
//! Per-file write dispatch (`.7z`/`.rar`, `.zip`, raw) lives in [`write`].

pub mod manifest;
pub mod routing;
pub mod rpcs3;
mod write;

use std::future::Future;
use std::path::{Component, Path, PathBuf};

use md5::{Digest, Md5};

use crate::backup::pack::{PackFirmware, PACK_DIR};
use crate::romm::{FirmwareRecord, RommClient};

/// One destination directory a firmware file may be routed to.
///
/// `keywords: None` mirrors a plain `Path` target dir in the Python source
/// (`firmware_install.py:38-56`) — every firmware file for the platform is
/// routed here. `keywords: Some(_)` mirrors a `(path, keywords)` tuple
/// entry — the file is routed here only when one of the keywords appears
/// as a substring of the lower-cased file name. Keywords are expected
/// already lower-cased and trimmed by the caller (Task 14's routing
/// helpers build them that way); this module never re-lowercases or trims
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareTarget {
    pub path: PathBuf,
    pub keywords: Option<Vec<String>>,
}

/// Options for [`install_platform_firmware`]. `Default` matches
/// `install_platform_firmware`'s Python keyword defaults
/// (firmware_install.py:87-91).
#[derive(Debug, Clone, Copy)]
pub struct FirmwareOptions {
    /// When true (the default), a write is skipped whenever the
    /// destination already exists. This never skips the *download* — only
    /// the write.
    pub skip_existing: bool,
    /// When true, every `.zip` is extracted with member paths preserved
    /// rather than kept as an archive or flattened to a single directory.
    pub extract_zip_with_paths: bool,
}

impl Default for FirmwareOptions {
    fn default() -> Self {
        Self {
            skip_existing: true,
            extract_zip_with_paths: false,
        }
    }
}

/// The TV standard a GameCube IPL dump states in its plain-text header
/// (`"… NTSC Revision 1.2"`, `"… PAL  Revision 1.0"`, `"… MPAL Revision
/// 1.1"`), or `None` when the header names none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IplStandard {
    Ntsc,
    Pal,
}

fn ipl_standard(data: &[u8]) -> Option<IplStandard> {
    let header = String::from_utf8_lossy(&data[..data.len().min(0x100)]);
    // MPAL (Brazil) boots NTSC-U discs, and contains "PAL": test it first.
    if header.contains("NTSC") || header.contains("MPAL") {
        Some(IplStandard::Ntsc)
    } else if header.contains("PAL") {
        Some(IplStandard::Pal)
    } else {
        None
    }
}

/// Whether `IPL.bin` (`data`) belongs in `target`. Dolphin boots a game with
/// the IPL in `User/GC/<region>/`, the region being the game's own (`USA`,
/// `JPN`, or `JAP` on older setups, for NTSC; `EUR` for PAL), so an NTSC
/// IPL goes to the NTSC folders and a PAL one to `EUR`. Any other file, any
/// other folder, or a header naming no standard is always accepted.
fn ipl_fits_target(file_name: &str, data: &[u8], target: &Path) -> bool {
    if !file_name.eq_ignore_ascii_case("ipl.bin") {
        return true;
    }
    let folder = target
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    !matches!(
        (folder.as_str(), ipl_standard(data)),
        ("USA" | "JPN" | "JAP", Some(IplStandard::Pal)) | ("EUR", Some(IplStandard::Ntsc))
    )
}

/// Returns the subset of `targets` that should receive `file_name`
/// (`resolve_firmware_targets`, firmware_install.py:38-56): a plain target
/// (`keywords: None`) accepts every file; a keyword target accepts the
/// file only when one of its keywords is a substring of the lower-cased
/// file name. Order is preserved and nothing is deduplicated — matching
/// the Python source, which fans a file out to every matching entry,
/// including duplicates.
pub fn resolve_targets<'a>(
    file_name: &str,
    targets: &'a [FirmwareTarget],
) -> Vec<&'a FirmwareTarget> {
    let lower = file_name.to_lowercase();
    targets
        .iter()
        .filter(|target| match &target.keywords {
            None => true,
            Some(keywords) => keywords.iter().any(|kw| lower.contains(kw.as_str())),
        })
        .collect()
}

/// Whether a `.zip` named `file_name` should be written as-is rather than
/// extracted (`should_keep_zip_archive`, firmware_install.py:60-77): true
/// only when it was routed (per `applicable`) through at least one keyword
/// target whose keyword list contains the file name's lower-cased form
/// exactly.
pub fn should_keep_zip(
    file_name: &str,
    targets: &[FirmwareTarget],
    applicable: &[&FirmwareTarget],
) -> bool {
    let lower = file_name.to_lowercase();
    targets.iter().any(|target| {
        let Some(keywords) = &target.keywords else {
            return false;
        };
        if !applicable.contains(&target) {
            return false;
        }
        let substring_match = keywords.iter().any(|kw| lower.contains(kw.as_str()));
        let exact_match = keywords.iter().any(|kw| kw.as_str() == lower);
        substring_match && exact_match
    })
}

/// Checks downloaded firmware bytes against what the server states for the
/// file: its size, then its MD5 (as Argosy, RomM's own client, does). A
/// field the server leaves unstated is not checked. `Err` is the warning to
/// report; nothing is written for a file that fails.
pub fn verify_download(record: &FirmwareRecord, data: &[u8]) -> Result<(), String> {
    let name = &record.file_name;
    if record.file_size_bytes > 0 && data.len() as u64 != record.file_size_bytes {
        return Err(format!(
            "Firmware {name} was not installed: the download is {} bytes, the server lists {}",
            data.len(),
            record.file_size_bytes
        ));
    }
    let expected = record.md5_hash.trim().to_ascii_lowercase();
    if !expected.is_empty() {
        let actual: String = Md5::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if actual != expected {
            return Err(format!(
                "Firmware {name} was not installed: its MD5 does not match the server's"
            ));
        }
    }
    Ok(())
}

/// Where a platform's firmware files come from: the RomM server
/// ([`ServerFirmware`]) or a drive's gridpack ([`PackFirmwareSource`]). The
/// install pass ([`install_platform_firmware_from`]) routes, verifies
/// ([`verify_download`]) and writes the files the same way for both.
pub trait FirmwareSource: Sync {
    /// Every firmware file the source has for its platform. `Err` is the
    /// whole warning the install pass reports.
    fn records(&self) -> impl Future<Output = Result<Vec<FirmwareRecord>, String>> + Send;

    /// The bytes of one record that [`records`](Self::records) returned.
    /// `Err` is the reason, without the file name.
    fn bytes(
        &self,
        record: &FirmwareRecord,
    ) -> impl Future<Output = Result<Vec<u8>, String>> + Send;
}

/// A platform's firmware on the RomM server: `GET /api/firmware` and each
/// file's content route.
pub struct ServerFirmware<'a> {
    pub client: &'a RommClient,
    pub platform_id: i64,
}

impl FirmwareSource for ServerFirmware<'_> {
    async fn records(&self) -> Result<Vec<FirmwareRecord>, String> {
        let platform_id = self.platform_id;
        self.client
            .firmware(platform_id)
            .await
            .map_err(|e| format!("Firmware fetch failed for platform {platform_id}: {e}"))
    }

    async fn bytes(&self, record: &FirmwareRecord) -> Result<Vec<u8>, String> {
        self.client
            .firmware_bytes(record.id, &record.file_name)
            .await
            .map_err(|e| e.to_string())
    }
}

/// The largest firmware file read from a drive. Real firmware is far
/// smaller (a PS3 update is about 200 MB); the cap keeps a hostile index
/// from making GRID read a huge file into memory.
const PACK_FIRMWARE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// A platform's firmware in a drive's gridpack: the index's
/// [`PackFirmware`] entry for it, read from `<mount>/gridpack/`.
///
/// The entry comes from the drive's `gridpack.json`, which is untrusted:
/// each file's `path` is validated, and resolved, under `<mount>/gridpack/`
/// before a byte is read ([`pack_file_path`]). The record ids are the
/// files' positions in the entry.
pub struct PackFirmwareSource {
    pub mount: PathBuf,
    pub entry: PackFirmware,
}

impl FirmwareSource for PackFirmwareSource {
    async fn records(&self) -> Result<Vec<FirmwareRecord>, String> {
        Ok(self
            .entry
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| FirmwareRecord {
                id: index as i64,
                file_name: file.name.clone(),
                file_size_bytes: file.size,
                md5_hash: file.md5.clone(),
                missing_from_fs: false,
            })
            .collect())
    }

    async fn bytes(&self, record: &FirmwareRecord) -> Result<Vec<u8>, String> {
        let file = usize::try_from(record.id)
            .ok()
            .and_then(|index| self.entry.files.get(index))
            .filter(|file| file.name == record.file_name)
            .ok_or_else(|| "it is not in the drive's index".to_string())?;
        let mount = self.mount.clone();
        let rel = file.path.clone();
        let stated = file.size;
        tokio::task::spawn_blocking(move || read_pack_file(&mount, &rel, stated))
            .await
            .map_err(|e| e.to_string())?
    }
}

/// Reads one firmware file of a gridpack, after [`pack_file_path`] has
/// placed it under `<mount>/gridpack/`. A file whose size differs from the
/// stated one is refused before it is read.
fn read_pack_file(mount: &Path, rel: &str, stated: u64) -> Result<Vec<u8>, String> {
    let path = pack_file_path(mount, rel)?;
    let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if len > PACK_FIRMWARE_MAX_BYTES {
        return Err(format!("the file on the drive is too big ({len} bytes)"));
    }
    if stated > 0 && len != stated {
        return Err(format!(
            "the file on the drive is {len} bytes, the drive's index lists {stated}"
        ));
    }
    std::fs::read(&path).map_err(|e| e.to_string())
}

/// `rel` (a path from a drive's `gridpack.json`) as a regular file under
/// `<mount>/gridpack/`, or why it is refused: it must be relative, made of
/// plain components only (no `..`, no root, no drive prefix, no backslash or
/// colon), each at most 255 bytes, and once symlinks are resolved it must
/// still lie under `<mount>/gridpack/`.
///
/// A local guard: `backup::pack::resolve` (Task 1) is being written in
/// parallel and should replace the lexical half of this once it lands.
fn pack_file_path(mount: &Path, rel: &str) -> Result<PathBuf, String> {
    let refused = || "its path in the drive's index is not allowed".to_string();
    if rel.is_empty() || rel.contains(['\\', ':', '\0']) {
        return Err(refused());
    }
    let rel_path = Path::new(rel);
    for component in rel_path.components() {
        match component {
            Component::Normal(part) if part.len() <= 255 => {}
            _ => return Err(refused()),
        }
    }
    let root = mount.join(PACK_DIR);
    let joined = root.join(rel_path);
    let root = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
    let real = std::fs::canonicalize(&joined).map_err(|e| e.to_string())?;
    if !real.starts_with(&root) || !real.is_file() {
        return Err(refused());
    }
    Ok(real)
}

/// Downloads and installs `platform_id`'s server firmware into `targets`
/// (`install_platform_firmware`, firmware_install.py:83-221; steps 1-9 of
/// `docs/porting/03-library-install.md` §18). Firmware files are small, so
/// this performs its file writes synchronously inline rather than via
/// `spawn_blocking` — the only `.await` points are the two `RommClient`
/// calls. Returns the accumulated warning list; an empty vec means every
/// applicable write succeeded (or there was nothing to do).
pub async fn install_platform_firmware(
    client: &RommClient,
    platform_id: i64,
    targets: &[FirmwareTarget],
    opts: FirmwareOptions,
) -> Vec<String> {
    install_platform_firmware_recorded(client, platform_id, targets, opts, None).await
}

/// [`install_platform_firmware`], also recording in the manifest at
/// `manifest` ([`manifest::MANIFEST_FILE`]) which files each server file put
/// in place, so Settings › Systems can tell installed firmware apart.
pub async fn install_platform_firmware_recorded(
    client: &RommClient,
    platform_id: i64,
    targets: &[FirmwareTarget],
    opts: FirmwareOptions,
    manifest: Option<&Path>,
) -> Vec<String> {
    let source = ServerFirmware {
        client,
        platform_id,
    };
    install_platform_firmware_from(&source, targets, opts, manifest).await
}

/// [`install_platform_firmware_recorded`] with the files taken from any
/// [`FirmwareSource`]: the server, or a drive's gridpack. Routing,
/// verification and writes do not depend on the source.
pub async fn install_platform_firmware_from<S: FirmwareSource>(
    source: &S,
    targets: &[FirmwareTarget],
    opts: FirmwareOptions,
    manifest: Option<&Path>,
) -> Vec<String> {
    if targets.is_empty() {
        return Vec::new();
    }

    let records = match source.records().await {
        Ok(records) => records,
        Err(warning) => return vec![warning],
    };
    if records.is_empty() {
        return Vec::new();
    }

    let mut warnings = Vec::new();
    let mut installs: Vec<(PathBuf, String, Vec<PathBuf>)> = Vec::new();

    for record in records {
        let file_name = record.file_name.clone();
        if file_name.is_empty() {
            continue;
        }

        let applicable = resolve_targets(&file_name, targets);
        let lower = file_name.to_lowercase();
        // A zip whose own name matches no keyword target may still hold the
        // file one asks for (a `wiiu.zip` holding Cemu's `keys.txt`), so it
        // is downloaded and routed by its members instead.
        let route_inside = applicable.is_empty()
            && lower.ends_with(".zip")
            && targets.iter().any(|t| t.keywords.is_some());
        if applicable.is_empty() && !route_inside {
            // No target's keywords match, so this file has nowhere to go.
            // Logged, not a warning to the caller: the pass still
            // succeeded. The file name only — never a URL or a header.
            tracing::warn!("firmware: {file_name} matched no target directory");
            continue;
        }

        // SECURITY: the record's file name is server-supplied and is joined
        // onto each routed target directory. Reject anything that is not a
        // single plain path component BEFORE the download, so a hostile
        // record costs no bytes and never reaches a write path.
        if !write::is_plain_file_name(&file_name) {
            let named: Vec<&FirmwareTarget> = if route_inside {
                targets.iter().collect()
            } else {
                applicable.clone()
            };
            for target in named {
                warnings.push(write::invalid_name_warning(&file_name, &target.path));
            }
            continue;
        }

        if route_inside {
            let data = match source.bytes(&record).await {
                Ok(data) => data,
                Err(e) => {
                    warnings.push(format!("Failed to download firmware {file_name}: {e}"));
                    continue;
                }
            };
            if let Err(warning) = verify_download(&record, &data) {
                warnings.push(warning);
                continue;
            }
            let members = write::zip_member_names(&data);
            let mut any = false;
            for target in targets {
                let Some(keywords) = &target.keywords else {
                    continue;
                };
                let wanted = members.iter().any(|m| {
                    let m = m.to_lowercase();
                    keywords.iter().any(|kw| m.contains(kw.as_str()))
                });
                if !wanted {
                    // Looked inside and found nothing for this target: the
                    // empty record tells the installed check there is
                    // nothing left to put here.
                    installs.push((target.path.clone(), file_name.clone(), Vec::new()));
                    continue;
                }
                any = true;
                if let Err(e) = std::fs::create_dir_all(&target.path) {
                    warnings.push(format!(
                        "Could not create firmware directory {}: {e}",
                        target.path.display()
                    ));
                    continue;
                }
                match write::write_zip_members_matching(
                    &file_name,
                    &data,
                    &target.path,
                    keywords,
                    opts,
                ) {
                    Ok(placed) => installs.push((target.path.clone(), file_name.clone(), placed)),
                    Err(msg) => warnings.push(msg),
                }
            }
            if !any {
                tracing::warn!("firmware: nothing in {file_name} matched a target directory");
            }
            continue;
        }

        let keep_archive = !opts.extract_zip_with_paths
            && lower.ends_with(".zip")
            && should_keep_zip(&file_name, targets, &applicable);

        let data = match source.bytes(&record).await {
            Ok(data) => data,
            Err(e) => {
                warnings.push(format!("Failed to download firmware {file_name}: {e}"));
                continue;
            }
        };
        if let Err(warning) = verify_download(&record, &data) {
            warnings.push(warning);
            continue;
        }

        for target in &applicable {
            if !ipl_fits_target(&file_name, &data, &target.path) {
                // A GameCube IPL is routed to every region folder by name;
                // its own header says which ones it belongs in. The empty
                // record tells the installed check this folder is done.
                installs.push((target.path.clone(), file_name.clone(), Vec::new()));
                continue;
            }
            if let Err(e) = std::fs::create_dir_all(&target.path) {
                warnings.push(format!(
                    "Could not create firmware directory {}: {e}",
                    target.path.display()
                ));
                continue;
            }
            match write::write_firmware_file(&file_name, &data, &target.path, keep_archive, opts) {
                Ok(placed) => installs.push((target.path.clone(), file_name.clone(), placed)),
                Err(msg) => warnings.push(msg),
            }
        }
    }

    if let Some(path) = manifest {
        manifest::record_installs(path, installs);
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ipl_fits_the_region_folders_of_its_tv_standard() {
        let ntsc = b"(C) 1999-2003 Nintendo.  All rights reserved.NTSC Revision 1.2  ";
        let pal = b"(C) 1999-2001 Nintendo.  All rights reserved.PAL  Revision 1.0  ";
        let mpal = b"(C) 1999-2001 Nintendo.  All rights reserved.MPAL Revision 1.1  ";
        let gc = Path::new("/user/GC");
        assert!(ipl_fits_target("IPL.bin", ntsc, &gc.join("USA")));
        assert!(ipl_fits_target("ipl.bin", ntsc, &gc.join("JPN")));
        assert!(ipl_fits_target("IPL.bin", ntsc, &gc.join("JAP")));
        assert!(!ipl_fits_target("IPL.bin", ntsc, &gc.join("EUR")));
        assert!(ipl_fits_target("IPL.bin", pal, &gc.join("EUR")));
        assert!(!ipl_fits_target("IPL.bin", pal, &gc.join("USA")));
        assert!(!ipl_fits_target("IPL.bin", mpal, &gc.join("EUR")));
        // No standard in the header, another file, another folder: placed.
        assert!(ipl_fits_target("IPL.bin", b"garbage", &gc.join("EUR")));
        assert!(ipl_fits_target("scph5501.bin", pal, &gc.join("USA")));
        assert!(ipl_fits_target("IPL.bin", pal, Path::new("/bios")));
    }

    fn stated(size: u64, md5: &str) -> FirmwareRecord {
        FirmwareRecord {
            id: 1,
            file_name: "scph5501.bin".into(),
            file_size_bytes: size,
            md5_hash: md5.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_download_matching_the_server_size_and_md5_passes() {
        let actual: String = Md5::digest(b"BIOS")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(verify_download(&stated(4, &actual), b"BIOS"), Ok(()));
        assert_eq!(
            verify_download(&stated(4, &actual.to_uppercase()), b"BIOS"),
            Ok(())
        );
    }

    #[test]
    fn a_download_of_the_wrong_size_is_rejected() {
        let err = verify_download(&stated(5, ""), b"BIOS").unwrap_err();
        assert!(err.contains("4 bytes") && err.contains("5"), "{err}");
    }

    #[test]
    fn a_download_with_a_different_md5_is_rejected() {
        let err =
            verify_download(&stated(4, "00000000000000000000000000000000"), b"BIOS").unwrap_err();
        assert!(err.contains("MD5"), "{err}");
    }

    // --- pack_file_path: the drive's index is untrusted ---------------------

    #[test]
    fn a_pack_path_must_be_plain_relative_and_inside_gridpack() {
        let drive = tempfile::tempdir().unwrap();
        let fw = drive.path().join(PACK_DIR).join("firmware");
        std::fs::create_dir_all(&fw).unwrap();
        std::fs::write(fw.join("bios.bin"), b"BIOS").unwrap();
        std::fs::write(drive.path().join("outside.bin"), b"NO").unwrap();

        let found = pack_file_path(drive.path(), "firmware/bios.bin").unwrap();
        assert_eq!(std::fs::read(found).unwrap(), b"BIOS");
        let long = "a".repeat(256);
        for rel in [
            "",
            "../outside.bin",
            "firmware/../../outside.bin",
            "/etc/passwd",
            "C:/outside.bin",
            "firmware\\bios.bin",
            "./firmware/bios.bin",
            "firmware",
            long.as_str(),
        ] {
            assert!(pack_file_path(drive.path(), rel).is_err(), "{rel:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_gridpack_is_refused() {
        let drive = tempfile::tempdir().unwrap();
        let root = drive.path().join(PACK_DIR);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(drive.path().join("outside.bin"), b"NO").unwrap();
        std::os::unix::fs::symlink(drive.path().join("outside.bin"), root.join("link.bin"))
            .unwrap();
        assert!(pack_file_path(drive.path(), "link.bin").is_err());
    }

    #[test]
    fn a_pack_file_of_another_size_than_listed_is_not_read() {
        let drive = tempfile::tempdir().unwrap();
        let root = drive.path().join(PACK_DIR);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("bios.bin"), b"BIOS").unwrap();
        assert_eq!(
            read_pack_file(drive.path(), "bios.bin", 4).unwrap(),
            b"BIOS"
        );
        assert!(read_pack_file(drive.path(), "bios.bin", 5).is_err());
    }

    #[test]
    fn unstated_size_and_md5_are_not_checked() {
        assert_eq!(verify_download(&stated(0, ""), b"BIOS"), Ok(()));
    }

    fn plain(path: &str) -> FirmwareTarget {
        FirmwareTarget {
            path: PathBuf::from(path),
            keywords: None,
        }
    }

    fn routed(path: &str, keywords: &[&str]) -> FirmwareTarget {
        FirmwareTarget {
            path: PathBuf::from(path),
            keywords: Some(keywords.iter().map(|s| s.to_string()).collect()),
        }
    }

    // --- resolve_targets (FirmwareRoutingTests) ----------------------------

    #[test]
    fn resolve_plain_path_accepts_all() {
        let targets = [plain("/bios")];
        let result = resolve_targets("anything.bin", &targets);
        assert_eq!(result, vec![&targets[0]]);
    }

    #[test]
    fn resolve_tuple_match_hit() {
        let targets = [routed("/gc/usa", &["ntsc"])];
        let result = resolve_targets("gc_ntsc.zip", &targets);
        assert_eq!(result, vec![&targets[0]]);
    }

    #[test]
    fn resolve_tuple_match_miss() {
        let targets = [routed("/gc/usa", &["pal"])];
        let result = resolve_targets("gc_ntsc.zip", &targets);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_all_matches_win() {
        let targets = [
            routed("/gc/jap", &["ntsc_j", "jap"]),
            routed("/gc/usa", &["ntsc"]),
        ];
        let result = resolve_targets("gc_ntsc_j.zip", &targets);
        assert_eq!(result, vec![&targets[0], &targets[1]]);
    }

    #[test]
    fn resolve_ntsc_goes_to_jap_and_usa() {
        let targets = [
            routed("Sys/GC/JAP", &["ntsc"]),
            routed("Sys/GC/USA", &["ntsc"]),
        ];
        let result = resolve_targets("gc-ntsc.zip", &targets);
        assert_eq!(result, vec![&targets[0], &targets[1]]);
    }

    #[test]
    fn resolve_case_insensitive() {
        let targets = [routed("/gc/usa", &["ntsc"])];
        let result = resolve_targets("GC_NTSC.ZIP", &targets);
        assert_eq!(result, vec![&targets[0]]);
    }

    #[test]
    fn resolve_no_routed_match_returns_empty() {
        let targets = [
            routed("/gc/jap", &["ntsc_j", "jap"]),
            routed("/gc/usa", &["ntsc", "usa"]),
        ];
        let result = resolve_targets("gc_unknown.zip", &targets);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_mixed_plain_and_routed() {
        let targets = [plain("/shared"), routed("/gc/usa", &["ntsc"])];
        let hit = resolve_targets("gc_ntsc.zip", &targets);
        let miss = resolve_targets("gc_pal.zip", &targets);
        assert_eq!(hit, vec![&targets[0], &targets[1]]);
        assert_eq!(miss, vec![&targets[0]]);
    }

    // --- should_keep_zip -----------------------------------------------------

    #[test]
    fn keep_zip_when_routed_through_exact_keyword() {
        let targets = [routed("/naomi", &["naomi.zip"])];
        let applicable = resolve_targets("naomi.zip", &targets);
        assert!(should_keep_zip("naomi.zip", &targets, &applicable));
    }

    #[test]
    fn extract_zip_when_target_is_plain() {
        let targets = [plain("/naomi")];
        let applicable = resolve_targets("naomi.zip", &targets);
        assert!(!should_keep_zip("naomi.zip", &targets, &applicable));
    }

    #[test]
    fn extract_zip_when_keyword_is_substring_only() {
        let targets = [routed("/naomi", &["naomi"])];
        let applicable = resolve_targets("naomi.zip", &targets);
        assert!(!should_keep_zip("naomi.zip", &targets, &applicable));
    }
}
