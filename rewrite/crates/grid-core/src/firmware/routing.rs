//! Where a platform's server firmware goes: turning an emulator entry's
//! profile `firmware_directories` into concrete [`FirmwareTarget`]s, the
//! RetroArch and Cemu reshaping applied on top, the per-game install that
//! runs during library finalize, and the platform-id list a freshly
//! installed source emulator fetches firmware for.
//!
//! Ports, verbatim unless noted:
//! - `grid_launcher/ui/mixins/cloud_mixin.py:1032-1105`
//!   (`_resolved_firmware_directories`) → [`targets_for_entry`].
//! - `grid_launcher/ui/mixins/install_mixin.py:528-697`
//!   (`_install_firmware_for_game_without_ui`) → [`shape_for_retroarch`],
//!   [`shape_for_cemu`], [`install_for_game`].
//! - `grid_launcher/ui/mixins/emulator_ui_mixin.py:1865-1890` (the
//!   platform-id list inside `_trigger_firmware_install_for_source_emulator`)
//!   → [`platform_ids_for_profile`]. Only the id list is ported here; the
//!   daemon thread and the emulator-view refresh signal around it belong to
//!   the service layer.
//!
//! Contract: `docs/porting/03-library-install.md` §18, "Per-game firmware
//! during finalize" and "Fresh source-emulator install".

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::autoconfig::cores::{
    compatibility_map, core_config_files_metadata, core_entries, core_firmware_metadata,
    core_saves_files_metadata, cores_for_platform, CoreEntry,
};
use crate::autoconfig::paths::{self, expand_user, resolve_best_effort};
use crate::autoconfig::{dolphin, is_cemu, is_dolphin, is_retroarch, retroarch};
use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::{platform_matches_keywords, profile_for_entry, EmulatorProfile};
use crate::launch::selection::{
    default_emulator_name_for_platform, emulator_entry_by_name, installed_core_resolver,
    mapping_value_for_platform,
};
use crate::romm::RommClient;

use super::{install_platform_firmware, FirmwareOptions, FirmwareTarget};

/// `${VAR}` / `$VAR` / `%VAR%` — the union of `posixpath.expandvars` and
/// `ntpath.expandvars` this crate needs, since a profile path written for
/// Windows may reach a Linux host and vice versa. An unset variable is left
/// as its literal text, matching Python (`expandvars` never raises and
/// never blanks an unresolved reference).
///
/// The three GRID tokens (`%EMULATOR_DIR%`, `%LIBRARY_DIR%`,
/// `%CONFIG_DIR%`) match the `%VAR%` arm, but no such environment variable
/// exists in practice, so they pass through untouched and are substituted
/// afterwards — the same order Python uses (`expandvars` first, then the
/// replacement table, cloud_mixin.py:1073-1081).
static ENV_VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)|%([A-Za-z_][A-Za-z0-9_]*)%",
    )
    .unwrap()
});

/// `os.path.expandvars`, in the `$VAR`/`${VAR}`/`%VAR%` union described on
/// [`ENV_VAR_RE`].
fn expand_env_vars(text: &str) -> String {
    ENV_VAR_RE
        .replace_all(text, |caps: &regex::Captures| {
            let name = caps
                .get(1)
                .or_else(|| caps.get(2))
                .or_else(|| caps.get(3))
                .expect("one alternation always captures")
                .as_str();
            std::env::var(name).unwrap_or_else(|_| caps[0].to_string())
        })
        .into_owned()
}

/// `str(path)` as Python renders it in the replacement table: an empty
/// `PathBuf` is Python's bare `Path()`, whose `str()` is `"."` (a relative
/// "current directory"), not the empty string — so a token expanded from a
/// blank emulator/library path keeps the result relative rather than
/// turning it absolute (cloud_mixin.py:1069-1071).
fn path_token(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.to_string_lossy().to_string()
    }
}

/// The directory an emulator entry's firmware paths resolve against: the
/// entry path itself when it is an existing directory, else its parent; an
/// empty `PathBuf` for a blank entry path (Python's `Path()` fallback,
/// cloud_mixin.py:1057-1059).
pub fn emulator_dir_of(entry: &EmulatorEntry) -> PathBuf {
    if entry.path.is_empty() {
        return PathBuf::new();
    }
    // The "itself when a directory, else the parent" rule lives in exactly
    // one place: `paths::emulator_dir` (paths.rs:104). Its `None` (no parent
    // at all — a filesystem root) collapses to the same empty `PathBuf` a
    // blank entry path yields.
    paths::emulator_dir(&expand_user(&entry.path)).unwrap_or_default()
}

/// The firmware target directories for one emulator entry
/// (`_resolved_firmware_directories`, cloud_mixin.py:1032-1105).
///
/// No profile, or a profile with no `firmware_directories`, yields no
/// targets. Every spec path is expanded for environment variables, then for
/// the `%EMULATOR_DIR%` / `%LIBRARY_DIR%` / `%CONFIG_DIR%` tokens, then for
/// `~`; a relative result is joined onto the emulator directory. Both are
/// then `.resolve()`d. The list is deduplicated case-insensitively by the
/// resolved path text, first occurrence winning.
pub fn targets_for_entry(
    entry: &EmulatorEntry,
    profile: Option<&EmulatorProfile>,
    library_dir: &str,
    config_dir: &Path,
) -> Vec<FirmwareTarget> {
    let Some(profile) = profile else {
        return Vec::new();
    };
    if profile.firmware_directories.is_empty() {
        return Vec::new();
    }

    let emulator_dir = emulator_dir_of(entry);
    let library_path = if library_dir.trim().is_empty() {
        PathBuf::new()
    } else {
        expand_user(library_dir)
    };
    let emulator_token = path_token(&emulator_dir);
    let library_token = path_token(&library_path);
    let config_token = path_token(config_dir);

    let mut resolved: Vec<FirmwareTarget> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for spec in &profile.firmware_directories {
        let raw = spec.path.trim();
        if raw.is_empty() {
            continue;
        }
        // A `{path, match}` entry whose keyword list normalized to empty is
        // dropped outright (cloud_mixin.py:1064-1066); the profile loader
        // already drops blank keywords, so this only guards an all-blank list.
        if spec.keywords.as_ref().is_some_and(Vec::is_empty) {
            continue;
        }

        let mut expanded = expand_env_vars(raw);
        expanded = expanded.replace("%EMULATOR_DIR%", &emulator_token);
        expanded = expanded.replace("%LIBRARY_DIR%", &library_token);
        expanded = expanded.replace("%CONFIG_DIR%", &config_token);

        let candidate = expand_user(&expanded);
        let path = if candidate.is_absolute() {
            resolve_best_effort(&candidate)
        } else {
            resolve_best_effort(&emulator_dir.join(&candidate))
        };

        if !seen.insert(path.to_string_lossy().to_lowercase()) {
            continue;
        }
        resolved.push(FirmwareTarget {
            path,
            keywords: spec.keywords.clone(),
        });
    }

    resolved
}

/// The RetroArch-specific reshaping of a per-game firmware install
/// (install_mixin.py:552-631): firmware targets after the core's
/// subdirectory and file-name filter, the `extract_zip_with_paths` flag,
/// and the separate config-file and saves-file target lists.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RetroArchPlan {
    pub firmware: Vec<FirmwareTarget>,
    pub extract_with_paths: bool,
    pub configs: Vec<FirmwareTarget>,
    pub saves: Vec<FirmwareTarget>,
}

/// The string elements of a metadata `files` array, in order. A missing key,
/// a non-array value, and an array with no strings all read as empty —
/// Python's `isinstance(file_names, list) and file_names` guard plus the
/// implicit "these are file names" assumption.
fn string_list(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// The saves directory a RetroArch `savefile_directory` setting names,
/// relative to `emulator_dir` (install_mixin.py:610-628).
///
/// Blank or `default` (case-insensitively) ⇒ `<emulator_dir>/saves`. The
/// RetroArch root-relative notations `:\rest` and `:/rest` drop the 2-char
/// prefix and resolve `rest` under the emulator directory. Anything else is
/// `~`-expanded and, when still relative, resolved under the emulator
/// directory; an absolute path is used as written (Python does NOT
/// `.resolve()` that branch).
fn saves_directory(emulator_dir: &Path, savefile_directory: &str) -> PathBuf {
    let stripped = savefile_directory.trim();
    if stripped.is_empty() || stripped.to_lowercase() == "default" {
        return emulator_dir.join("saves");
    }
    if let Some(rest) = stripped
        .strip_prefix(":\\")
        .or_else(|| stripped.strip_prefix(":/"))
    {
        return resolve_best_effort(&emulator_dir.join(rest));
    }
    let candidate = expand_user(stripped);
    if candidate.is_absolute() {
        candidate
    } else {
        resolve_best_effort(&emulator_dir.join(candidate))
    }
}

/// Applies the configured RetroArch core's metadata to a per-game firmware
/// install (install_mixin.py:552-631).
///
/// `None` mirrors Python's early `return ""` for a blank configured core —
/// nothing at all is installed. Otherwise a plan always comes back, and a
/// core with no `firmware` metadata simply yields an empty firmware list
/// (install_mixin.py:562-563) while its config and saves metadata are still
/// consulted.
pub fn shape_for_retroarch(
    core_id: &str,
    entries: &[CoreEntry],
    emulator_dir: &Path,
    savefile_directory: &str,
    firmware: Vec<FirmwareTarget>,
) -> Option<RetroArchPlan> {
    if core_id.trim().is_empty() {
        return None;
    }
    let mut plan = RetroArchPlan::default();

    if let Some(metadata) = core_firmware_metadata(core_id, entries) {
        let mut targets = firmware;

        if let Some(subdirectory) = metadata.get("subdirectory").and_then(Value::as_str) {
            if !subdirectory.trim().is_empty() {
                for target in &mut targets {
                    target.path = target.path.join(subdirectory);
                }
            }
        }

        // A plain target becomes a keyword target restricted to the core's
        // firmware file names; a target that already carries keywords keeps
        // them. Names are lower-cased because `resolve_targets` matches
        // against the lower-cased file name and never re-folds its keywords.
        let file_names = string_list(metadata.get("files"));
        if !file_names.is_empty() {
            let keywords: Vec<String> = file_names.iter().map(|n| n.to_lowercase()).collect();
            for target in &mut targets {
                if target.keywords.is_none() {
                    target.keywords = Some(keywords.clone());
                }
            }
        }

        plan.extract_with_paths = metadata
            .get("extract_with_paths")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        plan.firmware = targets;
    }

    if let Some(metadata) = core_config_files_metadata(core_id, entries) {
        if let Some(base_dir) = metadata.get("base_dir").and_then(Value::as_str) {
            if !base_dir.trim().is_empty() {
                let file_names = string_list(metadata.get("files"));
                plan.configs = vec![FirmwareTarget {
                    path: emulator_dir.join(base_dir),
                    keywords: (!file_names.is_empty()).then_some(file_names),
                }];
            }
        }
    }

    // The saves branch — and only that branch — is skipped outright when
    // there is no emulator directory to resolve against (Python's `if
    // emulator_dir:` guard, install_mixin.py:609; the config branch above
    // has no such guard and happily builds a relative path).
    if let Some(metadata) =
        core_saves_files_metadata(core_id, entries).filter(|_| !emulator_dir.as_os_str().is_empty())
    {
        if let Some(file_name) = metadata.get("file").and_then(Value::as_str) {
            if !file_name.trim().is_empty() {
                plan.saves = vec![FirmwareTarget {
                    path: saves_directory(emulator_dir, savefile_directory),
                    keywords: Some(vec![file_name.to_string()]),
                }];
            }
        }
    }

    Some(plan)
}

/// Cemu's firmware reshaping (install_mixin.py:632-636): a plain target is
/// restricted to `keys.txt`; a target that already carries keywords is left
/// alone.
pub fn shape_for_cemu(firmware: Vec<FirmwareTarget>) -> Vec<FirmwareTarget> {
    firmware
        .into_iter()
        .map(|mut target| {
            if target.keywords.is_none() {
                target.keywords = Some(vec!["keys.txt".to_string()]);
            }
            target
        })
        .collect()
}

/// Everything [`install_for_game`] needs beyond the HTTP client: the game's
/// platform name and server id, the loaded config (emulators, defaults,
/// RetroArch core map and library path), the autoprofile catalog, and the
/// directory holding the config file (the `%CONFIG_DIR%` token).
pub struct GameFirmwareContext<'a> {
    pub platform: &'a str,
    pub platform_id: i64,
    pub config: &'a Config,
    pub profiles: &'a [EmulatorProfile],
    pub config_dir: &'a Path,
}

/// Python's `try: warnings.extend(install_platform_firmware(...)) except
/// Exception as e: warnings.append(f"Firmware install error: {e}")`
/// (install_mixin.py:648-698, three identical blocks).
///
/// [`install_platform_firmware`] returns its failures as warnings and has no
/// error channel of its own, so in practice only the `Ok` arm is taken. The
/// wrap point is kept — and unit-tested — so the warning text stays in one
/// place if that contract ever grows an `Err`.
fn wrap_install(result: Result<Vec<String>, String>) -> Vec<String> {
    match result {
        Ok(warnings) => warnings,
        Err(error) => vec![format!("Firmware install error: {error}")],
    }
}

/// Installs a game's platform firmware during library finalize
/// (`_install_firmware_for_game_without_ui`, install_mixin.py:528-697).
///
/// Returns the accumulated warnings joined by newlines; `""` means either
/// success or "nothing to do" — a blank platform, no default emulator for
/// it, no matching entry, a RetroArch install with no configured core, or
/// no targets at all. Never fails.
pub async fn install_for_game(client: &RommClient, ctx: &GameFirmwareContext<'_>) -> String {
    if ctx.platform.trim().is_empty() {
        return String::new();
    }

    let emulator_name = default_emulator_name_for_platform(
        &ctx.config.emulators,
        &ctx.config.default_emulators,
        ctx.platform,
        ctx.profiles,
        &installed_core_resolver,
    );
    if emulator_name.trim().is_empty() {
        return String::new();
    }
    let Some(entry) = emulator_entry_by_name(&ctx.config.emulators, &emulator_name) else {
        return String::new();
    };

    let profile = profile_for_entry(&entry.name, &entry.path, ctx.profiles);
    let mut firmware = targets_for_entry(entry, profile, &ctx.config.library_path, ctx.config_dir);
    let mut configs: Vec<FirmwareTarget> = Vec::new();
    let mut saves: Vec<FirmwareTarget> = Vec::new();
    let mut extract_with_paths = false;

    if is_retroarch(entry, ctx.profiles) {
        let Some(core_id) = mapping_value_for_platform(&ctx.config.retroarch_cores, ctx.platform)
        else {
            return String::new();
        };
        let settings = retroarch::directory_settings(&entry.path);
        let Some(plan) = shape_for_retroarch(
            core_id,
            core_entries(),
            &emulator_dir_of(entry),
            &settings.savefile_directory,
            firmware,
        ) else {
            return String::new();
        };
        firmware = plan.firmware;
        configs = plan.configs;
        saves = plan.saves;
        extract_with_paths = plan.extract_with_paths;
    } else if is_cemu(entry, ctx.profiles) {
        firmware = shape_for_cemu(firmware);
    }

    if firmware.is_empty() && configs.is_empty() && saves.is_empty() {
        return String::new();
    }

    let mut warnings: Vec<String> = Vec::new();

    if !firmware.is_empty() {
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: extract_with_paths,
        };
        warnings.extend(wrap_install(Ok(install_platform_firmware(
            client,
            ctx.platform_id,
            &firmware,
            opts,
        )
        .await)));
    }

    if !configs.is_empty() {
        warnings.extend(wrap_install(Ok(install_platform_firmware(
            client,
            ctx.platform_id,
            &configs,
            FirmwareOptions::default(),
        )
        .await)));
    }

    if !saves.is_empty() {
        let opts = FirmwareOptions {
            skip_existing: true,
            extract_zip_with_paths: true,
        };
        warnings.extend(wrap_install(Ok(install_platform_firmware(
            client,
            ctx.platform_id,
            &saves,
            opts,
        )
        .await)));
    }

    if is_dolphin(entry, ctx.profiles) {
        // Both results are deliberately discarded: Python wraps each in a
        // bare `except Exception: pass` (install_mixin.py:689-696).
        let _ = dolphin::ensure_skip_ipl(&entry.path);
        let _ = dolphin::ensure_gcpad_config(&entry.path);
    }

    warnings.join("\n")
}

/// The server platform ids a freshly installed source emulator fetches
/// firmware for (emulator_ui_mixin.py:1865-1890).
///
/// An `all_platforms` RetroArch-style emulator takes every platform that
/// has at least one compatible core; any other `all_platforms` profile
/// takes every id; otherwise the profile's `platform_keywords` select the
/// platforms, and an empty keyword list selects nothing.
///
/// Signature note: the brief's three parameters cannot answer
/// [`is_retroarch`], which needs the autoprofile catalog, so `profiles` is
/// carried alongside `entry`.
pub fn platform_ids_for_profile(
    profile: &EmulatorProfile,
    entry: &EmulatorEntry,
    platforms: &BTreeMap<String, i64>,
    profiles: &[EmulatorProfile],
) -> Vec<i64> {
    if profile.all_platforms {
        if is_retroarch(entry, profiles) {
            return platforms
                .iter()
                .filter(|(name, _)| !cores_for_platform(name, compatibility_map()).is_empty())
                .map(|(_, id)| *id)
                .collect();
        }
        return platforms.values().copied().collect();
    }

    if profile.platform_keywords.is_empty() {
        return Vec::new();
    }
    platforms
        .iter()
        .filter(|(name, _)| platform_matches_keywords(name, &profile.platform_keywords))
        .map(|(_, id)| *id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::rpcs3::{ps3_platform_id, rpcs3_pup_path};
    use super::*;
    use crate::autoconfig::cores::{core_entries, CoreEntry};
    use crate::autoconfig::paths::resolve_best_effort;
    use crate::config::{Config, EmulatorEntry};
    use crate::launch::profiles::{load_profiles, EmulatorProfile, FirmwareDirSpec};
    use crate::romm::RommClient;
    use crate::secrets::Credential;
    use secrecy::SecretString;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use wiremock::matchers::{method, path as path_matcher};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // --- helpers ---------------------------------------------------------

    fn entry(name: &str, path: &Path) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            args: "%rom%".to_string(),
            ..Default::default()
        }
    }

    fn plain(path: PathBuf) -> FirmwareTarget {
        FirmwareTarget {
            path,
            keywords: None,
        }
    }

    fn routed(path: PathBuf, keywords: &[&str]) -> FirmwareTarget {
        FirmwareTarget {
            path,
            keywords: Some(keywords.iter().map(|k| k.to_string()).collect()),
        }
    }

    fn spec(path: &str, keywords: Option<&[&str]>) -> FirmwareDirSpec {
        FirmwareDirSpec {
            path: path.to_string(),
            keywords: keywords.map(|k| k.iter().map(|s| s.to_lowercase()).collect()),
        }
    }

    fn profile_with(specs: &[FirmwareDirSpec]) -> EmulatorProfile {
        EmulatorProfile {
            name: "Test Profile".to_string(),
            match_tokens: vec!["emu.exe".to_string()],
            args: "%rom%".to_string(),
            firmware_directories: specs.to_vec(),
            ..Default::default()
        }
    }

    fn catalog_profile(name: &str) -> &'static EmulatorProfile {
        load_profiles()
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("catalog profile {name} missing"))
    }

    fn core(core_file: &str, firmware: Option<serde_json::Value>) -> CoreEntry {
        CoreEntry {
            core_file: core_file.to_string(),
            platforms: Vec::new(),
            supports_save_states: None,
            supports_saves: None,
            cloud_sync_safe: None,
            vmu_shared_saves: None,
            firmware,
            config_files: None,
            saves_files: None,
        }
    }

    fn client(server: &MockServer) -> RommClient {
        RommClient::new(
            &server.uri(),
            Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
        )
        .unwrap()
    }

    // --- targets_for_entry -----------------------------------------------

    #[test]
    fn eden_profile_yields_two_routed_targets() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("Eden");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("eden.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let e = entry("Eden (Nintendo Switch)", &exe);

        let targets = targets_for_entry(
            &e,
            Some(catalog_profile("Eden (Nintendo Switch)")),
            "",
            temp.path(),
        );

        assert_eq!(
            targets,
            vec![
                routed(resolve_best_effort(&dir.join("user/keys")), &["keys"]),
                routed(
                    resolve_best_effort(&dir.join("user/nand/system/Contents/registered")),
                    &["firmware"]
                ),
            ]
        );
    }

    #[test]
    fn retroarch_profile_yields_a_plain_system_target() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("retroarch.exe");
        std::fs::write(&exe, b"").unwrap();
        let e = entry("RetroArch (Multi-System)", &exe);

        let targets = targets_for_entry(
            &e,
            Some(catalog_profile("RetroArch (Multi-System)")),
            "",
            temp.path(),
        );

        assert_eq!(
            targets,
            vec![plain(resolve_best_effort(&dir.join("system")))]
        );
    }

    #[test]
    fn xemu_profile_dot_target_is_the_emulator_dir() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("Xemu");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("xemu.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let e = entry("Xemu (Xbox)", &exe);

        let targets = targets_for_entry(&e, Some(catalog_profile("Xemu (Xbox)")), "", temp.path());

        assert_eq!(targets, vec![plain(resolve_best_effort(&dir))]);
        // Xbox firmware routes to that one plain target.
        assert_eq!(
            crate::firmware::resolve_targets("xbox-firmware.zip", &targets),
            vec![&targets[0]]
        );
    }

    #[test]
    fn rpcs3_profile_dot_target_is_the_emulator_dir() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("RPCS3");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("rpcs3.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let e = entry("RPCS3 (Playstation 3)", &exe);

        let targets = targets_for_entry(
            &e,
            Some(catalog_profile("RPCS3 (Playstation 3)")),
            "",
            temp.path(),
        );

        assert_eq!(targets, vec![plain(resolve_best_effort(&dir))]);
        // The PS3 firmware file routes to that one plain target.
        assert_eq!(
            crate::firmware::resolve_targets("PS3UPDAT.PUP", &targets),
            vec![&targets[0]]
        );
    }

    #[test]
    fn library_and_config_tokens_expand() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("Emu");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("emu.exe");
        std::fs::write(&exe, b"").unwrap();
        let library = temp.path().join("Library");
        let config_dir = temp.path().join("cfg");

        let profile = profile_with(&[
            spec("%LIBRARY_DIR%/bios", Some(&["bios"])),
            spec("%CONFIG_DIR%/firmware", None),
            spec("%EMULATOR_DIR%/sys", None),
        ]);

        let targets = targets_for_entry(
            &entry("Emu", &exe),
            Some(&profile),
            library.to_str().unwrap(),
            &config_dir,
        );

        assert_eq!(
            targets,
            vec![
                routed(resolve_best_effort(&library.join("bios")), &["bios"]),
                plain(resolve_best_effort(&config_dir.join("firmware"))),
                plain(resolve_best_effort(&dir.join("sys"))),
            ]
        );
    }

    #[test]
    fn tilde_expands_to_home() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let _env = crate::test_env::EnvGuard::set(&[("HOME", home.to_str())]);

        let exe = temp.path().join("Emu").join("emu.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();

        let profile = profile_with(&[spec("~/fw", None)]);
        let targets = targets_for_entry(&entry("Emu", &exe), Some(&profile), "", temp.path());

        assert_eq!(targets, vec![plain(resolve_best_effort(&home.join("fw")))]);
    }

    #[test]
    fn env_vars_expand_in_all_three_notations() {
        let _lock = crate::test_env::lock();
        let _env = crate::test_env::EnvGuard::set(&[("GRID_FW_TEST_DIR", Some("/opt/fw"))]);

        assert_eq!(expand_env_vars("$GRID_FW_TEST_DIR/a"), "/opt/fw/a");
        assert_eq!(expand_env_vars("${GRID_FW_TEST_DIR}/b"), "/opt/fw/b");
        assert_eq!(expand_env_vars("%GRID_FW_TEST_DIR%/c"), "/opt/fw/c");
        // Unset references are left as literal text, like os.path.expandvars.
        assert_eq!(
            expand_env_vars("$GRID_FW_TEST_UNSET/%GRID_FW_TEST_UNSET%"),
            "$GRID_FW_TEST_UNSET/%GRID_FW_TEST_UNSET%"
        );
        // The GRID tokens are not env vars, so they survive expansion intact.
        assert_eq!(expand_env_vars("%EMULATOR_DIR%/x"), "%EMULATOR_DIR%/x");
    }

    #[test]
    fn duplicate_targets_are_deduped_case_insensitively() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("Emu");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("emu.exe");
        std::fs::write(&exe, b"").unwrap();

        let profile = profile_with(&[
            spec("System", None),
            spec("system", Some(&["bios"])),
            spec("SYSTEM", None),
        ]);
        let targets = targets_for_entry(&entry("Emu", &exe), Some(&profile), "", temp.path());

        assert_eq!(
            targets,
            vec![plain(resolve_best_effort(&dir.join("System")))]
        );
    }

    #[test]
    fn no_profile_or_no_firmware_dirs_yields_no_targets() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("emu.exe");
        std::fs::write(&exe, b"").unwrap();
        let e = entry("Emu", &exe);

        assert!(targets_for_entry(&e, None, "", temp.path()).is_empty());
        assert!(targets_for_entry(&e, Some(&profile_with(&[])), "", temp.path()).is_empty());
    }

    // --- shape_for_retroarch ---------------------------------------------

    #[test]
    fn blank_core_id_has_no_plan() {
        let entries = [core("flycast_libretro.dll", None)];
        assert!(shape_for_retroarch("", &entries, Path::new("/ra"), "", Vec::new()).is_none());
        assert!(shape_for_retroarch("   ", &entries, Path::new("/ra"), "", Vec::new()).is_none());
    }

    #[test]
    fn missing_firmware_metadata_empties_the_targets() {
        let entries = [core("flycast_libretro.dll", None)];
        let plan = shape_for_retroarch(
            "flycast",
            &entries,
            Path::new("/ra"),
            "",
            vec![plain(PathBuf::from("/ra/system"))],
        )
        .unwrap();

        assert_eq!(plan.firmware, Vec::<FirmwareTarget>::new());
        assert!(!plan.extract_with_paths);
    }

    #[test]
    fn subdirectory_is_appended_to_plain_and_routed_targets() {
        let entries = [core(
            "flycast_libretro.dll",
            Some(serde_json::json!({"subdirectory": "dc"})),
        )];
        let plan = shape_for_retroarch(
            "flycast",
            &entries,
            Path::new("/ra"),
            "",
            vec![
                plain(PathBuf::from("/ra/system")),
                routed(PathBuf::from("/ra/system"), &["ntsc"]),
            ],
        )
        .unwrap();

        assert_eq!(
            plan.firmware,
            vec![
                plain(PathBuf::from("/ra/system/dc")),
                routed(PathBuf::from("/ra/system/dc"), &["ntsc"]),
            ]
        );
    }

    #[test]
    fn null_subdirectory_leaves_targets_unchanged() {
        let entries = [core(
            "mgba_libretro.dll",
            Some(serde_json::json!({"subdirectory": null})),
        )];
        let plan = shape_for_retroarch(
            "mgba",
            &entries,
            Path::new("/ra"),
            "",
            vec![plain(PathBuf::from("/ra/system"))],
        )
        .unwrap();

        assert_eq!(plan.firmware, vec![plain(PathBuf::from("/ra/system"))]);
    }

    #[test]
    fn nested_subdirectory_path_is_joined_whole() {
        let entries = [core(
            "ep128emu_core_libretro.dll",
            Some(serde_json::json!({"subdirectory": "ep128emu/rom"})),
        )];
        let plan = shape_for_retroarch(
            "ep128emu_core",
            &entries,
            Path::new("/ra"),
            "",
            vec![plain(PathBuf::from("/ra/system"))],
        )
        .unwrap();

        assert_eq!(
            plan.firmware,
            vec![plain(PathBuf::from("/ra/system/ep128emu/rom"))]
        );
    }

    #[test]
    fn files_list_keywords_plain_targets_only() {
        let entries = [core(
            "flycast_libretro.dll",
            Some(serde_json::json!({
                "subdirectory": "dc",
                "files": ["dc_boot.bin", "DC_FLASH.BIN"]
            })),
        )];
        let plan = shape_for_retroarch(
            "flycast",
            &entries,
            Path::new("/ra"),
            "",
            vec![
                plain(PathBuf::from("/ra/system")),
                routed(PathBuf::from("/ra/other"), &["some_filter"]),
            ],
        )
        .unwrap();

        assert_eq!(
            plan.firmware,
            vec![
                routed(
                    PathBuf::from("/ra/system/dc"),
                    &["dc_boot.bin", "dc_flash.bin"]
                ),
                routed(PathBuf::from("/ra/other/dc"), &["some_filter"]),
            ]
        );
    }

    #[test]
    fn empty_or_absent_files_list_leaves_plain_targets_plain() {
        for firmware in [
            serde_json::json!({"files": []}),
            serde_json::json!({"subdirectory": null}),
        ] {
            let entries = [core("mgba_libretro.dll", Some(firmware))];
            let plan = shape_for_retroarch(
                "mgba",
                &entries,
                Path::new("/ra"),
                "",
                vec![plain(PathBuf::from("/ra/system"))],
            )
            .unwrap();
            assert_eq!(plan.firmware, vec![plain(PathBuf::from("/ra/system"))]);
        }
    }

    #[test]
    fn real_dolphin_core_sets_extract_with_paths_and_subdirectory() {
        let plan = shape_for_retroarch(
            "dolphin",
            core_entries(),
            Path::new("/ra"),
            "",
            vec![plain(PathBuf::from("/ra/system"))],
        )
        .unwrap();

        assert!(plan.extract_with_paths);
        assert_eq!(
            plan.firmware,
            vec![routed(
                PathBuf::from("/ra/system/dolphin-emu/Sys"),
                &["dolphin-gc-bios.zip"]
            )]
        );
        assert_eq!(
            plan.configs,
            vec![routed(
                PathBuf::from("/ra/config/dolphin-emu"),
                &["dolphin-emu.opt"]
            )]
        );
        assert_eq!(plan.saves, Vec::<FirmwareTarget>::new());
    }

    // --- config-file target assembly -------------------------------------

    #[test]
    fn config_files_metadata_builds_a_routed_target() {
        let mut e = core("mgba_libretro.dll", None);
        e.config_files =
            Some(serde_json::json!({"base_dir": "config/mGBA", "files": ["mGBA.opt"]}));
        let plan =
            shape_for_retroarch("mgba", &[e], Path::new("/retroarch"), "", Vec::new()).unwrap();

        assert_eq!(
            plan.configs,
            vec![routed(
                PathBuf::from("/retroarch/config/mGBA"),
                &["mGBA.opt"]
            )]
        );
    }

    #[test]
    fn config_files_metadata_without_files_builds_a_plain_target() {
        for cfg in [
            serde_json::json!({"base_dir": "config/mGBA", "files": []}),
            serde_json::json!({"base_dir": "config/mGBA"}),
        ] {
            let mut e = core("mgba_libretro.dll", None);
            e.config_files = Some(cfg);
            let plan =
                shape_for_retroarch("mgba", &[e], Path::new("/retroarch"), "", Vec::new()).unwrap();
            assert_eq!(
                plan.configs,
                vec![plain(PathBuf::from("/retroarch/config/mGBA"))]
            );
        }
    }

    #[test]
    fn config_files_metadata_absent_or_blank_base_dir_builds_nothing() {
        for cfg in [None, Some(serde_json::json!({"base_dir": "  "}))] {
            let mut e = core("mgba_libretro.dll", None);
            e.config_files = cfg;
            let plan =
                shape_for_retroarch("mgba", &[e], Path::new("/retroarch"), "", Vec::new()).unwrap();
            assert_eq!(plan.configs, Vec::<FirmwareTarget>::new());
        }
    }

    // --- saves-file target assembly --------------------------------------

    fn saves_plan(savefile_directory: &str) -> RetroArchPlan {
        let mut e = core("citra_libretro.dll", None);
        e.saves_files = Some(serde_json::json!({"file": "citra-sysdata.zip"}));
        shape_for_retroarch(
            "citra",
            &[e],
            Path::new("/retroarch"),
            savefile_directory,
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn saves_dir_defaults_to_emulator_saves() {
        for value in ["", "   ", "default", "DEFAULT"] {
            assert_eq!(
                saves_plan(value).saves,
                vec![routed(
                    PathBuf::from("/retroarch/saves"),
                    &["citra-sysdata.zip"]
                )],
                "savefile_directory = {value:?}"
            );
        }
    }

    #[test]
    fn saves_dir_honors_retroarch_root_relative_notation() {
        for value in [":\\mysaves", ":/mysaves"] {
            assert_eq!(
                saves_plan(value).saves,
                vec![routed(
                    resolve_best_effort(Path::new("/retroarch/mysaves")),
                    &["citra-sysdata.zip"]
                )],
                "savefile_directory = {value:?}"
            );
        }
    }

    #[test]
    fn saves_dir_honors_absolute_and_relative_paths() {
        assert_eq!(
            saves_plan("/elsewhere/saves").saves,
            vec![routed(
                PathBuf::from("/elsewhere/saves"),
                &["citra-sysdata.zip"]
            )]
        );
        assert_eq!(
            saves_plan("relative/saves").saves,
            vec![routed(
                resolve_best_effort(Path::new("/retroarch/relative/saves")),
                &["citra-sysdata.zip"]
            )]
        );
    }

    #[test]
    fn saves_metadata_without_a_file_key_builds_nothing() {
        for saves in [
            None,
            Some(serde_json::json!({})),
            Some(serde_json::json!({"file": " "})),
        ] {
            let mut e = core("citra_libretro.dll", None);
            e.saves_files = saves;
            let plan = shape_for_retroarch("citra", &[e], Path::new("/retroarch"), "", Vec::new())
                .unwrap();
            assert_eq!(plan.saves, Vec::<FirmwareTarget>::new());
        }
    }

    #[test]
    fn saves_are_skipped_without_an_emulator_directory() {
        let mut e = core("citra_libretro.dll", None);
        e.saves_files = Some(serde_json::json!({"file": "citra-sysdata.zip"}));
        e.config_files = Some(serde_json::json!({"base_dir": "config/citra"}));
        let plan = shape_for_retroarch("citra", &[e], Path::new(""), "", Vec::new()).unwrap();

        assert_eq!(plan.saves, Vec::<FirmwareTarget>::new());
        // The config branch has no such guard: it stays relative.
        assert_eq!(plan.configs, vec![plain(PathBuf::from("config/citra"))]);
    }

    // --- shape_for_cemu ---------------------------------------------------

    #[test]
    fn cemu_wraps_plain_targets_and_preserves_routed_ones() {
        let shaped = shape_for_cemu(vec![
            plain(PathBuf::from("/cemu/portable")),
            routed(PathBuf::from("/cemu/other"), &["existing_filter"]),
        ]);

        assert_eq!(
            shaped,
            vec![
                routed(PathBuf::from("/cemu/portable"), &["keys.txt"]),
                routed(PathBuf::from("/cemu/other"), &["existing_filter"]),
            ]
        );
    }

    // --- platform_ids_for_profile ----------------------------------------

    fn platform_map(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn retroarch_all_platforms_keeps_only_platforms_with_cores() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("retroarch.exe");
        std::fs::write(&exe, b"").unwrap();
        let e = entry("RetroArch (Multi-System)", &exe);
        let platforms = platform_map(&[
            ("Nintendo 64", 1),
            ("Zzz Unknown Platform", 2),
            ("Game Boy Advance", 3),
        ]);

        let ids = platform_ids_for_profile(
            catalog_profile("RetroArch (Multi-System)"),
            &e,
            &platforms,
            load_profiles(),
        );

        assert_eq!(ids, vec![3, 1]);
    }

    #[test]
    fn non_retroarch_all_platforms_keeps_every_id() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("emu.exe");
        std::fs::write(&exe, b"").unwrap();
        let mut profile = profile_with(&[]);
        profile.all_platforms = true;
        let platforms = platform_map(&[("Nintendo 64", 1), ("Zzz Unknown Platform", 2)]);

        let ids = platform_ids_for_profile(&profile, &entry("Emu", &exe), &platforms, &[]);

        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn keyword_profile_keeps_matching_platform_ids() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("emu.exe");
        std::fs::write(&exe, b"").unwrap();
        let mut profile = profile_with(&[]);
        profile.platform_keywords = vec!["playstation 3".to_string(), "ps3".to_string()];
        let platforms = platform_map(&[("PlayStation 3", 7), ("PlayStation 2", 8)]);

        let ids = platform_ids_for_profile(&profile, &entry("Emu", &exe), &platforms, &[]);

        assert_eq!(ids, vec![7]);
    }

    #[test]
    fn profile_without_keywords_or_all_platforms_yields_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("emu.exe");
        std::fs::write(&exe, b"").unwrap();
        let profile = profile_with(&[]);
        let platforms = platform_map(&[("PlayStation 3", 7)]);

        assert_eq!(
            platform_ids_for_profile(&profile, &entry("Emu", &exe), &platforms, &[]),
            Vec::<i64>::new()
        );
    }

    // --- rpcs3 helpers -----------------------------------------------------

    #[test]
    fn ps3_platform_id_matches_name_or_ps3_alias() {
        assert_eq!(
            ps3_platform_id(&platform_map(&[("Sony PlayStation 3", 5), ("PS2", 6)])),
            Some(5)
        );
        assert_eq!(ps3_platform_id(&platform_map(&[("ps3", 9)])), Some(9));
        assert_eq!(
            ps3_platform_id(&platform_map(&[("PlayStation 2", 6)])),
            None
        );
        assert_eq!(ps3_platform_id(&BTreeMap::new()), None);
    }

    #[test]
    fn rpcs3_pup_path_finds_the_pup_beside_the_executable() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("rpcs3.exe");
        std::fs::write(&exe, b"").unwrap();

        assert!(rpcs3_pup_path(exe.to_str().unwrap()).is_none());

        let pup = temp.path().join("PS3UPDAT.PUP");
        std::fs::write(&pup, b"").unwrap();
        assert_eq!(
            rpcs3_pup_path(exe.to_str().unwrap()),
            Some(resolve_best_effort(&pup))
        );
        // A blank path never resolves.
        assert!(rpcs3_pup_path("   ").is_none());
    }

    // --- install_for_game --------------------------------------------------

    fn game_config(temp: &Path, exe: &Path, name: &str) -> Config {
        Config {
            library_path: temp.join("Library").to_string_lossy().to_string(),
            emulators: vec![entry(name, exe)],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn install_for_game_writes_firmware_into_the_profile_target() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/api/firmware"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": 4, "file_name": "scph5501.bin"}
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/api/firmware/4/content/scph5501.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"BIOSDATA".to_vec()))
            .mount(&server)
            .await;

        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("DuckStation");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("duckstation-qt-x64-ReleaseLTCG.exe");
        std::fs::write(&exe, b"").unwrap();
        let config = game_config(temp.path(), &exe, "DuckStation (Playstation 1)");

        let ctx = GameFirmwareContext {
            platform: "PlayStation",
            platform_id: 19,
            config: &config,
            profiles: load_profiles(),
            config_dir: temp.path(),
        };
        let warnings = install_for_game(&client(&server), &ctx).await;

        assert_eq!(warnings, "");
        assert_eq!(
            std::fs::read(dir.join("bios").join("scph5501.bin")).unwrap(),
            b"BIOSDATA"
        );
    }

    #[tokio::test]
    async fn install_for_game_returns_early_without_requests() {
        let server = MockServer::start().await;
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("DuckStation").join("duckstation.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        let config = game_config(temp.path(), &exe, "DuckStation (Playstation 1)");
        let empty = Config::default();

        let cases: [(&str, &Config); 3] = [
            // Blank platform.
            ("", &config),
            // No emulator configured for the platform at all.
            ("PlayStation", &empty),
            // A platform no configured emulator supports.
            ("Nintendo Switch", &config),
        ];
        for (platform, cfg) in cases {
            let ctx = GameFirmwareContext {
                platform,
                platform_id: 19,
                config: cfg,
                profiles: load_profiles(),
                config_dir: temp.path(),
            };
            assert_eq!(
                install_for_game(&client(&server), &ctx).await,
                "",
                "platform = {platform:?}"
            );
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn install_for_game_stops_when_retroarch_has_no_configured_core() {
        let server = MockServer::start().await;
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("retroarch.exe");
        std::fs::write(&exe, b"").unwrap();
        let config = game_config(temp.path(), &exe, "RetroArch (Multi-System)");

        let ctx = GameFirmwareContext {
            platform: "Nintendo 64",
            platform_id: 3,
            config: &config,
            profiles: load_profiles(),
            config_dir: temp.path(),
        };

        assert_eq!(install_for_game(&client(&server), &ctx).await, "");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[test]
    fn install_error_wrap_produces_the_verbatim_warning() {
        assert_eq!(
            wrap_install(Err("boom".to_string())),
            vec!["Firmware install error: boom".to_string()]
        );
        assert_eq!(wrap_install(Ok(vec!["kept".to_string()])), vec!["kept"]);
    }
}
