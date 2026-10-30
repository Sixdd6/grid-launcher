//! Which server firmware files are installed where, so the Emulators list
//! can say "Installed" instead of offering Install again (user ruling
//! 2026-10-30: going straight back to "Install firmware" read as a failed
//! install).
//!
//! A server file name alone cannot answer that: a `.zip`, `.7z` or `.rar` is
//! extracted, so the name never exists on disk. Each install therefore
//! records, per target directory and server file name, the files it put in
//! place (written, or already there and left alone). A file counts as
//! installed while every recorded file still exists, or, with no record,
//! when the file itself sits in the target directory (a raw or kept-archive
//! write, including one from a release before this manifest existed).
//!
//! The manifest lives beside the config file and holds local paths only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{resolve_targets, write, FirmwareTarget};
use crate::romm::FirmwareRecord;

/// The manifest's file name, in the directory that holds the config file.
pub const MANIFEST_FILE: &str = "firmware-installed.json";

/// Serializes load-modify-save across concurrent passes (one per emulator
/// directory can run at once).
static MANIFEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FirmwareManifest {
    /// Target directory -> server file name -> the files that install placed.
    #[serde(default)]
    targets: BTreeMap<String, BTreeMap<String, Vec<PathBuf>>>,
}

fn key(target: &Path) -> String {
    target.to_string_lossy().into_owned()
}

impl FirmwareManifest {
    /// The manifest at `path`; empty when it is missing or unreadable (the
    /// on-disk fallback in [`Self::installed`] still applies).
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    fn record(&mut self, target: &Path, file_name: &str, placed: Vec<PathBuf>) {
        self.targets
            .entry(key(target))
            .or_default()
            .insert(file_name.to_string(), placed);
    }

    fn recorded(&self, target: &Path, file_name: &str) -> Option<&Vec<PathBuf>> {
        self.targets
            .get(&key(target))
            .and_then(|files| files.get(file_name))
    }

    /// Whether `file_name` from the server is installed in `target`. An
    /// empty record means an install looked and had nothing to put there,
    /// so nothing is missing.
    pub fn installed(&self, target: &Path, file_name: &str) -> bool {
        if let Some(paths) = self.recorded(target, file_name) {
            if paths.iter().all(|p| p.exists()) {
                return true;
            }
        }
        write::is_plain_file_name(file_name) && target.join(file_name).is_file()
    }
}

/// Adds one pass's placed files to the manifest at `path`. A save failure
/// is logged, not returned: the firmware itself is installed, and the list
/// only decides what the Emulators view says about it.
pub(crate) fn record_installs(path: &Path, installs: Vec<(PathBuf, String, Vec<PathBuf>)>) {
    if installs.is_empty() {
        return;
    }
    let _lock = MANIFEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut manifest = FirmwareManifest::load(path);
    for (target, file_name, placed) in installs {
        manifest.record(&target, &file_name, placed);
    }
    if let Err(e) = manifest.save(path) {
        tracing::warn!("firmware: could not save {}: {e}", path.display());
    }
}

/// Whether every server firmware file that routes to one of `targets` is
/// installed there. A zip whose name routes nowhere is routed by its members
/// at install time (`install_platform_firmware_recorded`), so until an
/// install has looked inside it, it counts as not installed. `false` when
/// nothing routes anywhere: then Install has nothing to put in place, and
/// "Installed" would claim otherwise.
pub fn firmware_installed(
    records: &[FirmwareRecord],
    targets: &[FirmwareTarget],
    manifest: &FirmwareManifest,
) -> bool {
    let mut any = false;
    for record in records {
        let name = &record.file_name;
        if name.is_empty() {
            continue;
        }
        let applicable = resolve_targets(name, targets);
        if !applicable.is_empty() {
            for target in applicable {
                any = true;
                if !manifest.installed(&target.path, name) {
                    return false;
                }
            }
            continue;
        }
        let keyword_targets: Vec<&FirmwareTarget> =
            targets.iter().filter(|t| t.keywords.is_some()).collect();
        if !name.to_lowercase().ends_with(".zip") || keyword_targets.is_empty() {
            continue;
        }
        let records: Vec<Option<&Vec<PathBuf>>> = keyword_targets
            .iter()
            .map(|t| manifest.recorded(&t.path, name))
            .collect();
        if records.iter().all(Option::is_none) {
            // Not looked inside yet.
            return false;
        }
        for paths in records.into_iter().flatten() {
            if !paths.is_empty() {
                any = true;
                if !paths.iter().all(|p| p.exists()) {
                    return false;
                }
            }
        }
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: i64, name: &str) -> FirmwareRecord {
        FirmwareRecord {
            id,
            file_name: name.to_string(),
        }
    }

    fn plain(path: &Path) -> FirmwareTarget {
        FirmwareTarget {
            path: path.to_path_buf(),
            keywords: None,
        }
    }

    #[test]
    fn a_raw_file_in_the_target_counts_without_a_record() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("scph5501.bin"), b"x").unwrap();
        let manifest = FirmwareManifest::default();
        assert!(firmware_installed(
            &[record(1, "scph5501.bin")],
            &[plain(dir.path())],
            &manifest
        ));
    }

    #[test]
    fn an_extracted_archive_counts_while_its_recorded_files_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MANIFEST_FILE);
        let target = dir.path().join("bios");
        std::fs::create_dir_all(&target).unwrap();
        let member = target.join("IPL.bin");
        std::fs::write(&member, b"x").unwrap();
        record_installs(
            &path,
            vec![(target.clone(), "gc.zip".into(), vec![member.clone()])],
        );

        let records = [record(1, "gc.zip")];
        let targets = [plain(&target)];
        assert!(firmware_installed(
            &records,
            &targets,
            &FirmwareManifest::load(&path)
        ));

        std::fs::remove_file(&member).unwrap();
        assert!(!firmware_installed(
            &records,
            &targets,
            &FirmwareManifest::load(&path)
        ));
    }

    #[test]
    fn one_missing_file_means_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"x").unwrap();
        let records = [record(1, "a.bin"), record(2, "b.bin")];
        assert!(!firmware_installed(
            &records,
            &[plain(dir.path())],
            &FirmwareManifest::default()
        ));
    }

    #[test]
    fn nothing_routable_is_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let keyword = FirmwareTarget {
            path: dir.path().to_path_buf(),
            keywords: Some(vec!["dc_".into()]),
        };
        assert!(!firmware_installed(
            &[record(1, "scph5501.bin")],
            &[keyword],
            &FirmwareManifest::default()
        ));
        assert!(!firmware_installed(
            &[],
            &[plain(dir.path())],
            &FirmwareManifest::default()
        ));
    }

    #[test]
    fn a_zip_routed_by_its_members_counts_once_installed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MANIFEST_FILE);
        let cemu = dir.path().join("cemu");
        std::fs::create_dir_all(&cemu).unwrap();
        let targets = [FirmwareTarget {
            path: cemu.clone(),
            keywords: Some(vec!["keys.txt".into()]),
        }];
        let records = [record(40, "wiiu.zip")];
        // Before any install has looked inside: not installed.
        assert!(!firmware_installed(
            &records,
            &targets,
            &FirmwareManifest::load(&path)
        ));

        let keys = cemu.join("keys.txt");
        std::fs::write(&keys, b"k").unwrap();
        record_installs(
            &path,
            vec![(cemu.clone(), "wiiu.zip".into(), vec![keys.clone()])],
        );
        assert!(firmware_installed(
            &records,
            &targets,
            &FirmwareManifest::load(&path)
        ));

        std::fs::remove_file(&keys).unwrap();
        assert!(!firmware_installed(
            &records,
            &targets,
            &FirmwareManifest::load(&path)
        ));
    }

    #[test]
    fn an_unreadable_manifest_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MANIFEST_FILE);
        std::fs::write(&path, b"not json").unwrap();
        assert!(FirmwareManifest::load(&path).targets.is_empty());
    }
}
