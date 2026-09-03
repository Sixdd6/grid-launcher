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

mod write;

use std::path::PathBuf;

use crate::romm::RommClient;

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
    if targets.is_empty() {
        return Vec::new();
    }

    let records = match client.firmware(platform_id).await {
        Ok(records) => records,
        Err(e) => {
            return vec![format!(
                "Firmware fetch failed for platform {platform_id}: {e}"
            )];
        }
    };
    if records.is_empty() {
        return Vec::new();
    }

    let mut warnings = Vec::new();

    for record in records {
        let file_name = record.file_name;
        if file_name.is_empty() {
            continue;
        }

        let applicable = resolve_targets(&file_name, targets);
        if applicable.is_empty() {
            continue;
        }

        let lower = file_name.to_lowercase();
        let keep_archive = !opts.extract_zip_with_paths
            && lower.ends_with(".zip")
            && should_keep_zip(&file_name, targets, &applicable);

        let data = match client.firmware_bytes(record.id, &file_name).await {
            Ok(data) => data,
            Err(e) => {
                warnings.push(format!("Failed to download firmware {file_name}: {e}"));
                continue;
            }
        };

        for target in &applicable {
            if let Err(e) = std::fs::create_dir_all(&target.path) {
                warnings.push(format!(
                    "Could not create firmware directory {}: {e}",
                    target.path.display()
                ));
                continue;
            }
            if let Err(msg) =
                write::write_firmware_file(&file_name, &data, &target.path, keep_archive, opts)
            {
                warnings.push(msg);
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

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
