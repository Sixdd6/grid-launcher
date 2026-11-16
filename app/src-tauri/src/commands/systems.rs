//! Systems and firmware without RomM (romm-storefront-plan 0.9, 0.10):
//! `system_platforms`, `firmware_expected`, `locate_firmware_files` and
//! `locate_ps3_firmware`. None of them needs RomM; each uses it when it is
//! connected (its platform list, its firmware list for the platform).
//!
//! Mounted from `lib.rs` (`commands.rs` is frozen while the storefront steps
//! run in parallel); it belongs under `commands` once that file is free.

use std::path::{Path, PathBuf};

use grid_core::config::Config;
use grid_core::firmware::known::{files_for, known_files};
use grid_core::firmware::local::{
    expected_set, finish_report, plan_locate, FirmwareExpected, LocateNames, LocateReport,
};
use grid_core::firmware::manifest::{FirmwareManifest, MANIFEST_FILE};
use grid_core::firmware::routing::{config_dir_of, firmware_plan, GameFirmwareContext};
use grid_core::firmware::rpcs3::{
    picked_pup_problem, ps3_platform_id, rpcs3_installed_firmware, spawn_rpcs3_installfw,
};
use grid_core::firmware::{install_platform_firmware_from, FirmwareOptions, LocalFirmwareSource};
use grid_core::launch::profiles::{load_profiles, profile_for_entry};
use grid_core::launch::selection::emulator_entry_by_name;
use grid_core::romm::FirmwareRecord;
use grid_core::systems::{self, SystemPlatform};
use tauri::State;

use crate::commands::{ensure_library_idle, err, AppState};

/// Every system: GRID's catalog, RomM's platforms while it is connected and
/// answers, and the installed games' platforms (`systems::merge`). Records
/// the platform maps on the way (`platform_registry::record`). Never fails
/// for lack of RomM: an unreachable server just leaves its platforms out.
pub async fn systems_list(state: &State<'_, AppState>) -> Vec<SystemPlatform> {
    let server = match state.session.client() {
        Some(client) => match client.platforms().await {
            Ok(mut platforms) => {
                grid_core::romm::sort_by_label(&mut platforms);
                crate::platform_registry::record(state, Some(&platforms));
                Some(platforms)
            }
            Err(e) => {
                tracing::warn!("systems: RomM's platform list could not be read: {e}");
                None
            }
        },
        None => {
            crate::platform_registry::record(state, None);
            None
        }
    };
    let installed = match state.install.as_ref() {
        Ok(install) => {
            let registry = install.registry();
            tauri::async_runtime::spawn_blocking(move || registry.all())
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default()
                .into_iter()
                .map(|row| (row.platform, row.platform_slug))
                .collect()
        }
        Err(_) => Vec::new(),
    };
    systems::merge(systems::catalog(), server.as_deref(), &installed)
}

#[tauri::command]
pub async fn system_platforms(state: State<'_, AppState>) -> Result<Vec<SystemPlatform>, String> {
    Ok(systems_list(&state).await)
}

/// The label config is keyed by for `slug`, and RomM's id for it: the
/// server's label and id while RomM's list is recorded, else the catalog
/// name (else the slug) and no id.
fn label_and_id(state: &State<'_, AppState>, slug: &str) -> (String, Option<i64>) {
    let slug = systems::canonical_slug(slug);
    if let Ok(install) = state.install.as_ref() {
        let slugs = install.platform_slugs();
        for (label, id) in install.platform_ids() {
            let found = slugs.get(&label).map(|s| systems::canonical_slug(s));
            if found.as_deref() == Some(slug.as_str()) {
                return (label, Some(id));
            }
        }
    }
    let label = systems::by_slug(&slug)
        .map(|p| p.name.clone())
        .unwrap_or(slug);
    (label, None)
}

/// RomM's firmware list for platform `id`, or none (no client, no id, or
/// an error — logged, the expected set then comes from GRID's table).
async fn server_records(state: &State<'_, AppState>, id: Option<i64>) -> Vec<FirmwareRecord> {
    let (Some(client), Some(id)) = (state.session.client(), id) else {
        return Vec::new();
    };
    match client.firmware(id).await {
        Ok(records) => records,
        Err(e) => {
            tracing::warn!("systems: RomM's firmware list could not be read: {e}");
            Vec::new()
        }
    }
}

/// The expected set for `slug` and the emulator named `emulator` (blank:
/// none chosen), with the firmware targets its plan routes to.
fn expected_for(
    config: &Config,
    label: &str,
    slug: &str,
    emulator: &str,
    server: &[FirmwareRecord],
) -> FirmwareExpected {
    let profiles = load_profiles();
    let config_path = Config::default_path();
    let config_dir = config_dir_of(&config_path);
    let entry = emulator_entry_by_name(&config.emulators, emulator);
    let profile = entry.and_then(|e| profile_for_entry(&e.name, &e.path, profiles));
    let known = files_for(known_files(), slug, profile.map(|p| p.name.as_str()));
    let targets = match entry {
        Some(_) => {
            let ctx = GameFirmwareContext {
                platform: label,
                platform_id: 0,
                emulator: Some(emulator),
                config,
                profiles,
                config_dir: &config_dir,
            };
            firmware_plan(&ctx).map(|p| p.targets()).unwrap_or_default()
        }
        None => Vec::new(),
    };
    let manifest = FirmwareManifest::load(&config_dir.join(MANIFEST_FILE));
    let mut expected = expected_set(&known, server, &targets, &manifest);
    // RPCS3 installs the PUP into its own dev_flash: that is installed
    // whether or not the PUP file is still on disk.
    if let Some(entry) = entry {
        if grid_core::autoconfig::is_rpcs3(entry, profiles)
            && rpcs3_installed_firmware(&entry.path).installed
        {
            for f in &mut expected.files {
                if f.name.eq_ignore_ascii_case("PS3UPDAT.PUP") {
                    f.installed = true;
                    f.required = false;
                }
            }
        }
    }
    expected
}

#[tauri::command]
pub async fn firmware_expected(
    state: State<'_, AppState>,
    slug: String,
    emulator: String,
) -> Result<FirmwareExpected, String> {
    let (label, id) = label_and_id(&state, &slug);
    let server = server_records(&state, id).await;
    tauri::async_runtime::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(expected_for(&config, &label, &slug, &emulator, &server))
    })
    .await
    .map_err(|e| format!("firmware_expected did not finish: {e}"))?
}

/// "Install an emulator for <system> first. GRID puts firmware in its folders."
fn no_emulator_reason(system: &str) -> String {
    format!("Install an emulator for {system} first. GRID puts firmware in its folders.")
}

/// Locate files: checks the picked files (or every file in a picked folder)
/// against the expected set and installs the ones GRID knows into the
/// emulator's folders, under their known names (romm-storefront-plan 0.10).
/// Refused before any file is read when the emulator is not installed.
#[tauri::command]
pub async fn locate_firmware_files(
    state: State<'_, AppState>,
    slug: String,
    emulator: String,
    paths: Vec<String>,
) -> Result<LocateReport, String> {
    ensure_library_idle(&state)?;
    let (label, id) = label_and_id(&state, &slug);
    let system = systems::by_slug(&slug)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| label.clone());
    let server = server_records(&state, id).await;
    let config = tauri::async_runtime::spawn_blocking(|| Config::load(&Config::default_path()))
        .await
        .map_err(err)?
        .map_err(err)?;
    let profiles = load_profiles();
    let config_dir = config_dir_of(&Config::default_path());
    let Some(entry) = emulator_entry_by_name(&config.emulators, &emulator) else {
        return Err(no_emulator_reason(&label));
    };
    let emulator_name = entry.name.clone();
    let ctx = GameFirmwareContext {
        platform: &label,
        platform_id: 0,
        emulator: Some(&emulator_name),
        config: &config,
        profiles,
        config_dir: &config_dir,
    };
    let Some(plan) = firmware_plan(&ctx) else {
        return Err(format!(
            "{emulator_name} has no firmware folders GRID knows for {label}."
        ));
    };
    let targets = plan.targets();
    let passes: Vec<_> = plan
        .passes
        .iter()
        .map(|(t, opts)| {
            (
                t.clone(),
                FirmwareOptions {
                    skip_existing: false,
                    ..*opts
                },
            )
        })
        .collect();
    let expected = expected_for(&config, &label, &slug, &emulator_name, &server);
    let picked: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let shown = short_name(&emulator_name);
    let planned = {
        let system = system.clone();
        let shown = shown.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let names = LocateNames {
                system: &system,
                emulator: &shown,
            };
            plan_locate(&picked, &expected.files, &names)
        })
        .await
        .map_err(err)?
    };

    let source = LocalFirmwareSource::from_installs(&planned.installs);
    let manifest_path = config_dir.join(MANIFEST_FILE);
    let mut warnings = Vec::new();
    if !source.files.is_empty() {
        for (targets, opts) in &passes {
            warnings.extend(
                install_platform_firmware_from(&source, targets, *opts, Some(&manifest_path)).await,
            );
        }
    }
    if !warnings.is_empty() {
        // Local paths and file names only, never contents.
        tracing::debug!("locate firmware for {label}: {}", warnings.join("\n"));
    }
    let manifest = FirmwareManifest::load(&manifest_path);
    let failed = format!("GRID couldn't put it in {shown}'s folders");
    Ok(finish_report(planned, &targets, &manifest, &failed))
}

/// "PCSX2 (Playstation 2)" → "PCSX2".
fn short_name(name: &str) -> String {
    match name.find(" (") {
        Some(i) if name.trim_end().ends_with(')') => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// Locate PS3UPDAT.PUP: hands a picked PUP to `rpcs3 --installfw`, after
/// checking it is a PS3 system update and, when RomM lists one, that its
/// size and MD5 match. `Err` is the reason a file can't be used; `Ok(false)`
/// when the emulator is not installed or RPCS3 could not be started.
#[tauri::command]
pub async fn locate_ps3_firmware(
    state: State<'_, AppState>,
    emulator: String,
    path: String,
) -> Result<bool, String> {
    ensure_library_idle(&state)?;
    let id = state
        .install
        .as_ref()
        .ok()
        .and_then(|install| ps3_platform_id(&install.platform_ids()));
    let listed = server_records(&state, id)
        .await
        .into_iter()
        .find(|r| r.file_name.eq_ignore_ascii_case("PS3UPDAT.PUP"));
    tauri::async_runtime::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let Some(entry) = emulator_entry_by_name(&config.emulators, &emulator) else {
            return Ok(false);
        };
        let picked = grid_core::platform::canonicalize(Path::new(&path))
            .map_err(|_| "GRID couldn't read this file".to_string())?;
        let expected = listed
            .as_ref()
            .map(|r| (r.file_size_bytes, r.md5_hash.as_str()));
        if let Some(problem) = picked_pup_problem(&picked, expected) {
            return Err(problem);
        }
        let exe = grid_core::autoconfig::paths::expand_user(&entry.path);
        Ok(spawn_rpcs3_installfw(Path::new(&exe), &picked))
    })
    .await
    .map_err(|e| format!("locate_ps3_firmware did not finish: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_drop_the_system_suffix() {
        assert_eq!(short_name("PCSX2 (Playstation 2)"), "PCSX2");
        assert_eq!(short_name("RetroArch"), "RetroArch");
    }

    #[test]
    fn no_emulator_words() {
        assert_eq!(
            no_emulator_reason("Sega Saturn"),
            "Install an emulator for Sega Saturn first. GRID puts firmware in its folders."
        );
    }
}
