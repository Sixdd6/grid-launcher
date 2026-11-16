//! Firmware without RomM (romm-storefront-plan 0.10): the expected set for a
//! system and emulator (`firmware_expected`), and Locate files — checking
//! files the user picked against it and installing the ones GRID knows
//! (`locate_firmware_files`).
//!
//! Locate files reads user-picked paths, so every path is canonicalized,
//! only regular files are read, nothing over [`super::PACK_FIRMWARE_MAX_BYTES`]
//! is read at all, a folder is scanned at most [`MAX_SCAN_DEPTH`] levels
//! deep without following links, and archives are extracted into a
//! temporary folder by the shared extractor. Nothing about a file's
//! contents is ever logged (key files are personal); paths are logged at
//! debug only.
//!
//! Writes go through [`super::install_platform_firmware_from`] with a
//! [`LocalFirmwareSource`], so routing, the zip rule, the size and MD5
//! re-check ([`super::verify_download`]) and the installed-firmware manifest
//! are the same as for RomM and drive installs.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde::Serialize;

use super::known::{KnownFile, MatchKind};
use super::manifest::{firmware_installed, FirmwareManifest};
use super::{resolve_targets, FirmwareSource, FirmwareTarget, PACK_FIRMWARE_MAX_BYTES};
use crate::romm::FirmwareRecord;

/// How deep a picked folder is searched: the folder and two levels below.
pub const MAX_SCAN_DEPTH: usize = 3;

/// Who vouches for an expected file's size and MD5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedBy {
    /// RomM's firmware list states its MD5.
    Romm,
    /// GRID's own table states it.
    Grid,
    /// Nobody: a per-console file with no fixed checksum.
    None,
}

/// One file the emulator needs for the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExpectedFile {
    pub name: String,
    /// `0` when not fixed.
    pub size: u64,
    /// `""` when there is no fixed checksum.
    pub md5: String,
    /// The system needs it now: a required file whose group has nothing
    /// installed yet.
    pub required: bool,
    pub installed: bool,
    pub verified_by: VerifiedBy,
}

/// `firmware_expected`'s answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FirmwareExpected {
    pub files: Vec<ExpectedFile>,
}

/// One candidate entry of the expected set, before the installed check.
#[derive(Debug, Clone)]
struct Entry {
    name: String,
    size: u64,
    md5: String,
    required: bool,
    group: String,
    content: bool,
    verified_by: VerifiedBy,
}

fn entries(known: &[&KnownFile], server: &[FirmwareRecord]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    // The server's list first: it is what Install from RomM puts in place.
    for record in server {
        let name = record.file_name.trim();
        if name.is_empty() {
            continue;
        }
        let known_entry = known
            .iter()
            .find(|k| {
                k.name.eq_ignore_ascii_case(name)
                    && (k.md5.is_empty() || k.md5 == record.md5_hash.to_lowercase())
            })
            .or_else(|| known.iter().find(|k| k.name.eq_ignore_ascii_case(name)));
        let md5 = record.md5_hash.trim().to_lowercase();
        out.push(Entry {
            name: name.to_string(),
            size: record.file_size_bytes,
            verified_by: if md5.is_empty() {
                VerifiedBy::None
            } else {
                VerifiedBy::Romm
            },
            md5,
            // A file only RomM lists is needed as far as GRID can tell.
            required: known_entry.map_or(true, |k| k.required),
            group: known_entry.map(|k| k.group.clone()).unwrap_or_default(),
            content: known_entry.is_some_and(|k| k.matching == MatchKind::Content),
        });
    }
    for k in known {
        if out.iter().any(|e| e.name.eq_ignore_ascii_case(&k.name)) {
            continue;
        }
        out.push(Entry {
            name: k.name.clone(),
            size: k.size,
            md5: k.md5.clone(),
            required: k.required,
            group: k.group.clone(),
            content: k.matching == MatchKind::Content,
            verified_by: if k.has_hash() {
                VerifiedBy::Grid
            } else {
                VerifiedBy::None
            },
        });
    }
    out
}

fn record_of(entry: &Entry) -> FirmwareRecord {
    FirmwareRecord {
        id: 0,
        file_name: entry.name.clone(),
        file_size_bytes: entry.size,
        md5_hash: entry.md5.clone(),
        missing_from_fs: false,
    }
}

/// Whether some regular file directly in a plain target directory is this
/// BIOS by content (same size and MD5), whatever it is called: emulators
/// that match by content find it there.
fn content_present(entry: &Entry, targets: &[FirmwareTarget]) -> bool {
    if entry.md5.is_empty() || entry.size == 0 {
        return false;
    }
    targets.iter().filter(|t| t.keywords.is_none()).any(|t| {
        let Ok(dir) = std::fs::read_dir(&t.path) else {
            return false;
        };
        dir.flatten().any(|f| {
            f.file_type().is_ok_and(|ty| ty.is_file())
                && f.metadata().is_ok_and(|m| m.len() == entry.size)
                && hash_file(&f.path()).is_ok_and(|(_, md5)| md5 == entry.md5)
        })
    })
}

/// The expected set: RomM's firmware list for the platform (when it is
/// connected) unioned by name with GRID's known files for the slug and the
/// emulator's profile, each marked installed or not for `targets` (the
/// emulator's firmware folders). When `targets` is not empty a file that
/// routes to none of them is left out: the emulator does not use it.
///
/// A required file of a `group` stays required only while nothing of its
/// group is installed (any one region's BIOS serves). Missing optional
/// files come after the rest; otherwise the order is RomM's, then GRID's.
pub fn expected_set(
    known: &[&KnownFile],
    server: &[FirmwareRecord],
    targets: &[FirmwareTarget],
    manifest: &FirmwareManifest,
) -> FirmwareExpected {
    let mut list = entries(known, server);
    if !targets.is_empty() {
        let keyword_targets = targets.iter().any(|t| t.keywords.is_some());
        list.retain(|e| {
            !resolve_targets(&e.name, targets).is_empty()
                || (keyword_targets
                    && e.name.to_lowercase().ends_with(".zip")
                    && e.verified_by == VerifiedBy::Romm)
        });
    }
    let installed: Vec<bool> = list
        .iter()
        .map(|e| {
            !targets.is_empty()
                && (firmware_installed(&[record_of(e)], targets, manifest)
                    || (e.content && content_present(e, targets)))
        })
        .collect();
    let mut satisfied: BTreeMap<&str, bool> = BTreeMap::new();
    for (e, inst) in list.iter().zip(&installed) {
        if !e.group.is_empty() {
            *satisfied.entry(e.group.as_str()).or_default() |= *inst;
        }
    }
    let mut files: Vec<ExpectedFile> = list
        .iter()
        .zip(&installed)
        .map(|(e, inst)| ExpectedFile {
            name: e.name.clone(),
            size: e.size,
            md5: e.md5.clone(),
            required: e.required
                && !inst
                && !(satisfied.get(e.group.as_str()).copied().unwrap_or(false)),
            installed: *inst,
            verified_by: e.verified_by,
        })
        .collect();
    files.sort_by_key(|f| !(f.installed || f.required));
    FirmwareExpected { files }
}

// --- Locate files ----------------------------------------------------------

/// A picked file and the name it was installed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocatedFile {
    /// The picked file's name (an archive's, for a member found in it).
    pub file: String,
    pub as_name: String,
}

/// A file Locate files did not install, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefusedFile {
    pub file: String,
    pub reason: String,
}

/// `locate_firmware_files`' answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LocateReport {
    pub installed: Vec<LocatedFile>,
    /// Installed, but GRID has no checksum to check it (decision 13).
    pub unverified: Vec<LocatedFile>,
    pub refused: Vec<RefusedFile>,
    /// Files in a picked folder that matched nothing: counted, not listed.
    pub unmatched: u32,
}

/// The words a report uses for the system and the emulator.
#[derive(Debug, Clone)]
pub struct LocateNames<'a> {
    pub system: &'a str,
    pub emulator: &'a str,
}

/// "This isn't the <name> GRID knows (size or MD5 differs)".
pub fn differs_reason(name: &str) -> String {
    format!("This isn't the {name} GRID knows (size or MD5 differs)")
}

/// "GRID doesn't know this file for <system>".
pub fn unknown_reason(system: &str) -> String {
    format!("GRID doesn't know this file for {system}")
}

/// "Nothing in it is <system> firmware GRID knows" (an archive).
pub fn archive_reason(system: &str) -> String {
    format!("Nothing in it is {system} firmware GRID knows")
}

/// A file over the size cap.
pub const OVERSIZE_REASON: &str = "Over 2 GB, so GRID didn't read it";
const UNREADABLE_REASON: &str = "GRID couldn't read this file";
const NOT_A_FILE_REASON: &str = "This isn't a regular file";

/// What one file's size and MD5 match in the expected set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileMatch {
    /// Size and MD5 match: install under this name.
    Verified(String),
    /// Its name matches a file with no checksum: install, not verified.
    Unverified(String),
    /// Its name matches a file GRID knows, but size or MD5 differ.
    Differs(String),
    Unknown,
}

/// Matches one file (its name, size and MD5) against `expected`.
pub fn match_file(name: &str, size: u64, md5: &str, expected: &[ExpectedFile]) -> FileMatch {
    if let Some(e) = expected
        .iter()
        .find(|e| !e.md5.is_empty() && e.md5 == md5 && (e.size == 0 || e.size == size))
    {
        return FileMatch::Verified(e.name.clone());
    }
    let named: Vec<&ExpectedFile> = expected
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case(name.trim()))
        .collect();
    if named.is_empty() {
        return FileMatch::Unknown;
    }
    if let Some(e) = named
        .iter()
        .find(|e| e.md5.is_empty() && (e.size == 0 || e.size == size))
    {
        return FileMatch::Unverified(e.name.clone());
    }
    FileMatch::Differs(named[0].name.clone())
}

/// Streams `path` once: its size and lower-case hex MD5.
pub fn hash_file(path: &Path) -> std::io::Result<(u64, String)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Md5::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hasher.update(&buf[..n]);
    }
    Ok((
        size,
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    ))
}

/// One file found by [`collect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    /// Picked by hand (listed when refused), not found in a picked folder
    /// (only counted when it matches nothing).
    pub picked: bool,
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The regular files behind `paths`: each path canonicalized; a folder
/// scanned [`MAX_SCAN_DEPTH`] levels deep (links not followed, hidden
/// entries skipped); anything that is neither refused.
pub fn collect(paths: &[PathBuf]) -> (Vec<Found>, Vec<RefusedFile>) {
    let mut found = Vec::new();
    let mut refused = Vec::new();
    for raw in paths {
        let path = match crate::platform::canonicalize(raw) {
            Ok(p) => p,
            Err(_) => {
                refused.push(RefusedFile {
                    file: display_name(raw),
                    reason: UNREADABLE_REASON.into(),
                });
                continue;
            }
        };
        let Ok(meta) = std::fs::metadata(&path) else {
            refused.push(RefusedFile {
                file: display_name(raw),
                reason: UNREADABLE_REASON.into(),
            });
            continue;
        };
        if meta.is_dir() {
            scan(&path, 1, &mut found);
        } else if meta.is_file() {
            if !found.iter().any(|f: &Found| f.path == path) {
                found.push(Found { path, picked: true });
            }
        } else {
            refused.push(RefusedFile {
                file: display_name(raw),
                reason: NOT_A_FILE_REASON.into(),
            });
        }
    }
    (found, refused)
}

fn scan(dir: &Path, depth: usize, out: &mut Vec<Found>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        // `file_type` does not follow links: a link is neither.
        let Ok(ty) = entry.file_type() else { continue };
        if ty.is_file() {
            let path = entry.path();
            if !out.iter().any(|f| f.path == path) {
                out.push(Found {
                    path,
                    picked: false,
                });
            }
        } else if ty.is_dir() && depth < MAX_SCAN_DEPTH {
            scan(&entry.path(), depth + 1, out);
        }
    }
}

fn is_archive(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".7z") || lower.ends_with(".rar")
}

/// One file Locate files will install.
#[derive(Debug, Clone)]
pub struct LocalInstall {
    /// What it installs as: the expected name, size and MD5.
    pub record: FirmwareRecord,
    /// Where its bytes are (a picked file, or a member in a temp folder).
    pub source: PathBuf,
    /// The report line once it is in place.
    pub line: LocatedFile,
    pub verified: bool,
}

/// What [`plan_locate`] decided: the installs, and the report so far (the
/// refusals and the unmatched count). Holds the temp folders archives were
/// extracted into until it drops.
pub struct LocatePlan {
    pub installs: Vec<LocalInstall>,
    pub report: LocateReport,
    _temp: Vec<tempfile::TempDir>,
}

/// Checks every file behind `paths` against `expected` (pure but for file
/// reads): which install under which name, which are refused and why.
/// A file matched twice (the same name from two files) installs once.
pub fn plan_locate(
    paths: &[PathBuf],
    expected: &[ExpectedFile],
    names: &LocateNames<'_>,
) -> LocatePlan {
    let (found, mut refused) = collect(paths);
    let mut installs: Vec<LocalInstall> = Vec::new();
    let mut temp = Vec::new();
    let mut unmatched = 0u32;
    for f in found {
        let file = display_name(&f.path);
        tracing::debug!("firmware: checking {}", f.path.display());
        let len = match std::fs::metadata(&f.path) {
            Ok(m) => m.len(),
            Err(_) => {
                refused.push(RefusedFile {
                    file,
                    reason: UNREADABLE_REASON.into(),
                });
                continue;
            }
        };
        if len > PACK_FIRMWARE_MAX_BYTES {
            refused.push(RefusedFile {
                file,
                reason: OVERSIZE_REASON.into(),
            });
            continue;
        }
        let Ok((size, md5)) = hash_file(&f.path) else {
            refused.push(RefusedFile {
                file,
                reason: UNREADABLE_REASON.into(),
            });
            continue;
        };
        match match_file(&file, size, &md5, expected) {
            FileMatch::Verified(name) => {
                push_install(&mut installs, &name, size, &md5, &f.path, &file, true)
            }
            FileMatch::Unverified(name) => {
                push_install(&mut installs, &name, size, "", &f.path, &file, false)
            }
            FileMatch::Differs(name) => refused.push(RefusedFile {
                file,
                reason: differs_reason(&name),
            }),
            FileMatch::Unknown if is_archive(&file) => {
                let before = installs.len();
                if let Some(dir) = members_of(&f.path, &file, expected, &mut installs) {
                    temp.push(dir);
                }
                if installs.len() == before {
                    if f.picked {
                        refused.push(RefusedFile {
                            file,
                            reason: archive_reason(names.system),
                        });
                    } else {
                        unmatched += 1;
                    }
                }
            }
            FileMatch::Unknown if f.picked => refused.push(RefusedFile {
                file,
                reason: unknown_reason(names.system),
            }),
            FileMatch::Unknown => unmatched += 1,
        }
    }
    LocatePlan {
        installs,
        report: LocateReport {
            refused,
            unmatched,
            ..Default::default()
        },
        _temp: temp,
    }
}

fn push_install(
    installs: &mut Vec<LocalInstall>,
    name: &str,
    size: u64,
    md5: &str,
    source: &Path,
    file: &str,
    verified: bool,
) {
    if installs
        .iter()
        .any(|i| i.record.file_name.eq_ignore_ascii_case(name))
    {
        return;
    }
    installs.push(LocalInstall {
        record: FirmwareRecord {
            id: installs.len() as i64,
            file_name: name.to_string(),
            // An unverified file is installed as it is; only a verified
            // one carries the size and MD5 `verify_download` re-checks.
            file_size_bytes: if verified { size } else { 0 },
            md5_hash: md5.to_string(),
            missing_from_fs: false,
        },
        source: source.to_path_buf(),
        line: LocatedFile {
            file: file.to_string(),
            as_name: name.to_string(),
        },
        verified,
    });
}

/// Extracts `archive` into a temp folder and matches each member (by its
/// own name, size and MD5). The temp folder is returned so it outlives the
/// install; `None` when the archive could not be extracted.
fn members_of(
    archive: &Path,
    file: &str,
    expected: &[ExpectedFile],
    installs: &mut Vec<LocalInstall>,
) -> Option<tempfile::TempDir> {
    let dir = tempfile::tempdir().ok()?;
    let dest = dir.path().join("x");
    let mut progress = |_: u64, _: u64| {};
    if crate::library::extract::extract_archive(archive, &dest, &mut progress).is_err() {
        return None;
    }
    let mut members = Vec::new();
    scan(&dest, 1, &mut members);
    for m in members {
        let Ok(meta) = std::fs::metadata(&m.path) else {
            continue;
        };
        if meta.len() > PACK_FIRMWARE_MAX_BYTES {
            continue;
        }
        let Ok((size, md5)) = hash_file(&m.path) else {
            continue;
        };
        let member = display_name(&m.path);
        match match_file(&member, size, &md5, expected) {
            FileMatch::Verified(name) => {
                push_install(installs, &name, size, &md5, &m.path, file, true)
            }
            FileMatch::Unverified(name) => {
                push_install(installs, &name, size, "", &m.path, file, false)
            }
            _ => {}
        }
    }
    Some(dir)
}

/// Firmware files on this computer, picked by the user: the records Locate
/// files matched, read from their local paths.
pub struct LocalFirmwareSource {
    pub files: Vec<(FirmwareRecord, PathBuf)>,
}

impl LocalFirmwareSource {
    pub fn from_installs(installs: &[LocalInstall]) -> Self {
        Self {
            files: installs
                .iter()
                .map(|i| (i.record.clone(), i.source.clone()))
                .collect(),
        }
    }
}

impl FirmwareSource for LocalFirmwareSource {
    async fn records(&self) -> Result<Vec<FirmwareRecord>, String> {
        Ok(self.files.iter().map(|(r, _)| r.clone()).collect())
    }

    async fn bytes(&self, record: &FirmwareRecord) -> Result<Vec<u8>, String> {
        let path = self
            .files
            .iter()
            .find(|(r, _)| r.id == record.id && r.file_name == record.file_name)
            .map(|(_, p)| p.clone())
            .ok_or_else(|| "it is not one of the picked files".to_string())?;
        tokio::task::spawn_blocking(move || {
            let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
            if len > PACK_FIRMWARE_MAX_BYTES {
                return Err(OVERSIZE_REASON.to_string());
            }
            std::fs::read(&path).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

/// Puts the report together after the install: each planned file is
/// installed (verified or not) when the manifest now says it is in place
/// for `targets`, else refused with `failed_reason`.
pub fn finish_report(
    plan: LocatePlan,
    targets: &[FirmwareTarget],
    manifest: &FirmwareManifest,
    failed_reason: &str,
) -> LocateReport {
    let mut report = plan.report;
    for i in plan.installs {
        let record = FirmwareRecord {
            md5_hash: String::new(),
            file_size_bytes: 0,
            ..i.record.clone()
        };
        if firmware_installed(&[record], targets, manifest) {
            if i.verified {
                report.installed.push(i.line);
            } else {
                report.unverified.push(i.line);
            }
        } else {
            report.refused.push(RefusedFile {
                file: i.line.file,
                reason: failed_reason.to_string(),
            });
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn md5_of(data: &[u8]) -> String {
        Md5::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn exp(name: &str, data: Option<&[u8]>, required: bool) -> ExpectedFile {
        ExpectedFile {
            name: name.into(),
            size: data.map_or(0, |d| d.len() as u64),
            md5: data.map(md5_of).unwrap_or_default(),
            required,
            installed: false,
            verified_by: if data.is_some() {
                VerifiedBy::Grid
            } else {
                VerifiedBy::None
            },
        }
    }

    const NAMES: LocateNames<'static> = LocateNames {
        system: "PlayStation",
        emulator: "DuckStation",
    };

    fn expected() -> Vec<ExpectedFile> {
        vec![
            exp("scph5501.bin", Some(b"FAKE-BIOS-US-not-real"), true),
            exp("scph5502.bin", Some(b"FAKE-BIOS-EU-not-real"), false),
            exp("prod.keys", None, true),
        ]
    }

    #[test]
    fn md5_match_under_another_name_installs_under_the_known_name() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("scph5501 (1).bin");
        fs::write(&p, b"FAKE-BIOS-US-not-real").unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert_eq!(plan.installs.len(), 1);
        assert_eq!(plan.installs[0].record.file_name, "scph5501.bin");
        assert_eq!(
            plan.installs[0].line,
            LocatedFile {
                file: "scph5501 (1).bin".into(),
                as_name: "scph5501.bin".into()
            }
        );
        assert!(plan.installs[0].verified);
        assert_eq!(
            plan.installs[0].record.md5_hash,
            md5_of(b"FAKE-BIOS-US-not-real")
        );
    }

    #[test]
    fn a_size_mismatch_under_a_known_name_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("scph5502.bin");
        fs::write(&p, b"FAKE-BIOS-EU-not-real-but-longer").unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert!(plan.installs.is_empty());
        assert_eq!(
            plan.report.refused[0].reason,
            "This isn't the scph5502.bin GRID knows (size or MD5 differs)"
        );
    }

    #[test]
    fn an_md5_mismatch_under_a_known_name_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("SCPH5502.BIN");
        fs::write(&p, b"FAKE-BIOS-XX-not-real").unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert!(plan.installs.is_empty());
        assert_eq!(plan.report.refused[0].file, "SCPH5502.BIN");
        assert!(plan.report.refused[0].reason.contains("scph5502.bin"));
    }

    #[test]
    fn a_no_hash_entry_installs_unverified() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("prod.keys");
        fs::write(&p, b"FAKE-STORE-TOKEN-not-real").unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert_eq!(plan.installs.len(), 1);
        assert!(!plan.installs[0].verified);
        assert_eq!(plan.installs[0].record.md5_hash, "");
        assert_eq!(plan.installs[0].record.file_size_bytes, 0);
    }

    #[test]
    fn an_unknown_picked_file_is_refused_and_a_folder_file_is_counted() {
        let dir = tempfile::tempdir().unwrap();
        let picked = dir.path().join("readme.txt");
        fs::write(&picked, b"hello").unwrap();
        let plan = plan_locate(&[picked], &expected(), &NAMES);
        assert_eq!(
            plan.report.refused[0].reason,
            "GRID doesn't know this file for PlayStation"
        );

        let folder = dir.path().join("bios");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("notes.txt"), b"x").unwrap();
        fs::write(folder.join("scph5501.bin"), b"FAKE-BIOS-US-not-real").unwrap();
        let plan = plan_locate(&[folder], &expected(), &NAMES);
        assert_eq!(plan.installs.len(), 1);
        assert!(plan.report.refused.is_empty());
        assert_eq!(plan.report.unmatched, 1);
    }

    #[test]
    fn an_oversize_file_is_refused_before_it_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("huge.bin");
        let f = fs::File::create(&p).unwrap();
        // A sparse file: no bytes are written, and none may be read.
        f.set_len(PACK_FIRMWARE_MAX_BYTES + 1).unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert_eq!(plan.report.refused[0].reason, OVERSIZE_REASON);
    }

    #[test]
    fn a_folder_scan_stops_three_levels_deep() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        let deep = root.join("b").join("c");
        fs::create_dir_all(deep.join("d")).unwrap();
        fs::write(deep.join("level3.bin"), b"3").unwrap();
        fs::write(deep.join("d").join("level4.bin"), b"4").unwrap();
        let (found, _) = collect(&[root]);
        let names: Vec<String> = found.iter().map(|f| display_name(&f.path)).collect();
        assert_eq!(names, vec!["level3.bin"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_scan_does_not_follow_links() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("scph5501.bin"), b"FAKE-BIOS-US-not-real").unwrap();
        let root = dir.path().join("bios");
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        std::os::unix::fs::symlink(outside.join("scph5501.bin"), root.join("file.bin")).unwrap();
        let (found, _) = collect(&[root]);
        assert!(found.is_empty(), "{found:?}");
    }

    fn zip_with(members: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (name, data) in members {
                w.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn an_archive_matching_nothing_whole_is_matched_by_its_members() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bios-pack.zip");
        fs::write(
            &p,
            zip_with(&[
                ("scph5502.bin", b"FAKE-BIOS-EU-not-real"),
                ("readme.txt", b"x"),
            ]),
        )
        .unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert_eq!(plan.installs.len(), 1);
        assert_eq!(
            plan.installs[0].line,
            LocatedFile {
                file: "bios-pack.zip".into(),
                as_name: "scph5502.bin".into()
            }
        );
        // The member's bytes stay readable until the plan drops.
        assert_eq!(
            fs::read(&plan.installs[0].source).unwrap(),
            b"FAKE-BIOS-EU-not-real"
        );
    }

    #[test]
    fn an_archive_matching_whole_is_installed_as_itself() {
        let data = zip_with(&[("x.bin", b"x")]);
        let mut exp_list = expected();
        exp_list.push(exp("pack.zip", Some(&data), false));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("pack (2).zip");
        fs::write(&p, &data).unwrap();
        let plan = plan_locate(&[p], &exp_list, &NAMES);
        assert_eq!(plan.installs.len(), 1);
        assert_eq!(plan.installs[0].record.file_name, "pack.zip");
    }

    #[test]
    fn an_archive_with_nothing_known_is_refused_with_its_own_reason() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Firmware.zip");
        fs::write(&p, zip_with(&[("a.nca", b"x")])).unwrap();
        let plan = plan_locate(&[p], &expected(), &NAMES);
        assert_eq!(
            plan.report.refused[0].reason,
            "Nothing in it is PlayStation firmware GRID knows"
        );
    }

    fn known(
        name: &str,
        data: Option<&[u8]>,
        required: bool,
        group: &str,
        content: bool,
    ) -> KnownFile {
        KnownFile {
            slug: "psx".into(),
            name: name.into(),
            path_hint: String::new(),
            size: data.map_or(0, |d| d.len() as u64),
            md5: data.map(md5_of).unwrap_or_default(),
            sha1: String::new(),
            crc: String::new(),
            required,
            group: group.into(),
            matching: if content {
                MatchKind::Content
            } else {
                MatchKind::Name
            },
            profiles: Vec::new(),
            source: "grid".into(),
        }
    }

    fn plain(path: &Path) -> FirmwareTarget {
        FirmwareTarget {
            path: path.to_path_buf(),
            keywords: None,
        }
    }

    #[test]
    fn expected_set_marks_installed_files_and_satisfies_a_group() {
        let dir = tempfile::tempdir().unwrap();
        let bios = dir.path().join("bios");
        fs::create_dir(&bios).unwrap();
        let k = [
            known("scph5501.bin", Some(b"US"), true, "psx-bios", true),
            known("scph5500.bin", Some(b"JP"), false, "psx-bios", true),
            known("extra.bin", Some(b"EX"), false, "", false),
        ];
        let refs: Vec<&KnownFile> = k.iter().collect();
        let manifest = FirmwareManifest::default();
        let before = expected_set(&refs, &[], &[plain(&bios)], &manifest);
        assert!(
            before
                .files
                .iter()
                .find(|f| f.name == "scph5501.bin")
                .unwrap()
                .required
        );
        assert!(before.files.iter().all(|f| !f.installed));

        // The JP BIOS under another name: found by content, and the group
        // no longer needs the US one.
        fs::write(bios.join("japan.bin"), b"JP").unwrap();
        let after = expected_set(&refs, &[], &[plain(&bios)], &manifest);
        let jp = after
            .files
            .iter()
            .find(|f| f.name == "scph5500.bin")
            .unwrap();
        assert!(jp.installed);
        assert!(!after.files.iter().any(|f| f.required));
    }

    #[test]
    fn expected_set_unions_the_server_list_by_name() {
        let k = [known("scph5501.bin", Some(b"US"), true, "", false)];
        let refs: Vec<&KnownFile> = k.iter().collect();
        let server = vec![
            FirmwareRecord {
                id: 1,
                file_name: "SCPH5501.BIN".into(),
                file_size_bytes: 2,
                md5_hash: md5_of(b"US"),
                missing_from_fs: false,
            },
            FirmwareRecord {
                id: 2,
                file_name: "custom.bin".into(),
                file_size_bytes: 1,
                md5_hash: md5_of(b"C"),
                missing_from_fs: false,
            },
        ];
        let set = expected_set(&refs, &server, &[], &FirmwareManifest::default());
        assert_eq!(set.files.len(), 2);
        assert!(set.files.iter().all(|f| f.verified_by == VerifiedBy::Romm));
        assert!(set.files.iter().all(|f| f.required));
    }

    #[test]
    fn expected_set_drops_files_the_emulator_does_not_route() {
        let k = [
            known("scph5501.bin", Some(b"US"), true, "", false),
            known("dc_boot.bin", Some(b"DC"), true, "", false),
        ];
        let refs: Vec<&KnownFile> = k.iter().collect();
        let only = FirmwareTarget {
            path: PathBuf::from("/bios"),
            keywords: Some(vec!["scph".into()]),
        };
        let set = expected_set(&refs, &[], &[only], &FirmwareManifest::default());
        let names: Vec<&str> = set.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["scph5501.bin"]);
    }

    #[test]
    fn no_hash_entries_are_verified_by_nobody() {
        let k = [known("prod.keys", None, true, "", false)];
        let refs: Vec<&KnownFile> = k.iter().collect();
        let set = expected_set(&refs, &[], &[], &FirmwareManifest::default());
        assert_eq!(set.files[0].verified_by, VerifiedBy::None);
        assert_eq!(set.files[0].md5, "");
    }
}
