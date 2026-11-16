//! Library › RomM › Emulators (RommStorefront README "RomM › Emulators"): the
//! server's Emulators platform as an install list. Emulators named to
//! `docs/server-emulator-packages.md` install through the server-emulator
//! path and register; every other file installs as a plain file in the
//! library's Emulators folder.

use std::collections::BTreeMap;
use std::path::PathBuf;

use grid_core::cloud::scope::is_emulators_platform;
use grid_core::config::Config;
use grid_core::launch::binary_target::Cpu;
use grid_core::launch::emulators_view::{emulators_view, EmulatorsView, ViewFile, ViewPackage};
use grid_core::launch::profiles::load_profiles;
use grid_core::launch::setup::recommendations;
use grid_core::launch::source::HOST_PLATFORM;
use grid_core::library::emulator_package::PackageOs;
use grid_core::library::paths::{library_root, platform_dir};
use grid_core::library::{
    server_file_installed, ServerEmulatorRequest, ServerFileRequest, SERVER_FILES_PLATFORM,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{ensure_library_idle, err, list_platforms, AppState};

/// The packaging guide "How to name builds" opens (decision after Step 8).
pub const PACKAGING_GUIDE_URL: &str =
    "https://github.com/Sixdd6/grid-launcher/blob/main/docs/server-emulator-packages.md";

/// The Emulators view, or `present: false` when the server has no
/// Emulators platform (the rail row is hidden).
#[derive(Debug, Serialize)]
pub struct RommEmulators {
    pub present: bool,
    /// The platform's id on the server; `0` when absent.
    pub platform_id: i64,
    /// Every file on the platform; `0` hides the row ("Empty").
    pub file_count: usize,
    pub view: Option<EmulatorsView>,
}

/// Reads the server's Emulators platform and sorts its files into the view.
/// `Err` when RomM is not connected or the platform could not be read
/// (the view's "Can't load the list").
#[tauri::command]
pub async fn romm_emulators(state: State<'_, AppState>) -> Result<RommEmulators, String> {
    let client = state.session.client().ok_or("Not connected to RomM.")?;
    let platforms = list_platforms(state.clone()).await?;
    let Some(shelf) = platforms
        .iter()
        .find(|p| is_emulators_platform(p.label()) || is_emulators_platform(&p.name))
    else {
        return Ok(RommEmulators {
            present: false,
            platform_id: 0,
            file_count: 0,
            view: None,
        });
    };
    let games = client.games(shelf.id).await.map_err(err)?;
    let mut packages = Vec::new();
    for game in games {
        let detail = client.rom_detail(game.id).await.map_err(err)?;
        packages.push(ViewPackage {
            rom_id: detail.id,
            folder: detail.fs_name,
            files: detail
                .files
                .into_iter()
                .map(|f| ViewFile {
                    id: f.id,
                    name: f.file_name,
                    size_bytes: f.file_size_bytes,
                })
                .collect(),
        });
    }
    let labels: BTreeMap<String, String> = platforms
        .iter()
        .filter(|p| !p.slug.trim().is_empty())
        .map(|p| (p.slug.trim().to_lowercase(), p.label().to_string()))
        .collect();
    let file_count = packages.iter().map(|p| p.files.len()).sum();
    let platform_id = shelf.id;
    let view = tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).unwrap_or_default();
        let library = library_root(&config);
        let installed = |folder: &str, name: &str| {
            library
                .as_ref()
                .is_some_and(|lib| server_file_installed(lib, folder, name))
        };
        let (Some(os), Some(cpu)) = (PackageOs::for_host(HOST_PLATFORM), Cpu::host()) else {
            return None;
        };
        Some(emulators_view(
            &packages,
            load_profiles(),
            &config,
            recommendations(),
            &labels,
            (os, cpu),
            &installed,
        ))
    })
    .await
    .map_err(|e| format!("romm_emulators did not finish: {e}"))?;
    Ok(RommEmulators {
        present: true,
        platform_id,
        file_count,
        view,
    })
}

/// One emulator build to install, as the view listed it.
#[derive(Debug, Deserialize)]
pub struct EmulatorBuildArgs {
    pub rom_id: i64,
    pub file_id: i64,
    pub file_name: String,
    pub version: String,
    pub folder: String,
    pub profile_name: String,
}

/// Installs (or updates) an emulator from the view: the server-emulator
/// path, registered as the profile its file name named.
#[tauri::command]
pub async fn romm_emulator_install(
    state: State<'_, AppState>,
    build: EmulatorBuildArgs,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let client = state.session.client().ok_or("Not connected to RomM.")?;
    install
        .install_server_emulator(
            client,
            ServerEmulatorRequest {
                rom_id: build.rom_id,
                file_id: build.file_id,
                file_name: build.file_name,
                version: build.version,
                folder: build.folder,
                profile_name: build.profile_name,
            },
        )
        .await
        .map_err(err)
}

/// One Other file to install, as the view listed it.
#[derive(Debug, Deserialize)]
pub struct ServerFileArgs {
    pub rom_id: i64,
    pub file_id: i64,
    pub file_name: String,
    pub folder: String,
}

/// Installs an Other file as a plain file in the library's Emulators folder.
#[tauri::command]
pub async fn romm_emulator_file_install(
    state: State<'_, AppState>,
    file: ServerFileArgs,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let client = state.session.client().ok_or("Not connected to RomM.")?;
    install
        .install_server_file(
            client,
            ServerFileRequest {
                rom_id: file.rom_id,
                file_id: file.file_id,
                file_name: file.file_name,
                folder: file.folder,
            },
        )
        .await
        .map_err(err)
}

/// Deletes an Other file from the library's Emulators folder.
#[tauri::command]
pub async fn romm_emulator_file_uninstall(
    state: State<'_, AppState>,
    folder: String,
    file_name: String,
) -> Result<(), String> {
    ensure_library_idle(&state)?;
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || install.uninstall_server_file(&folder, &file_name))
        .await
        .map_err(|e| format!("romm_emulator_file_uninstall did not finish: {e}"))?
        .map(|_| ())
        .map_err(err)
}

/// Opens the library's Emulators folder (where Other files land) in the
/// file manager, creating it first. Takes no path: the frontend cannot
/// choose what gets opened.
#[tauri::command]
pub async fn open_emulators_folder(app: AppHandle) -> Result<(), String> {
    let dir = tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let library = library_root(&config).ok_or("Set a library folder first.")?;
        let dir = platform_dir(&library, SERVER_FILES_PLATFORM);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok::<PathBuf, String>(dir)
    })
    .await
    .map_err(|e| format!("open_emulators_folder did not finish: {e}"))??;
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// "How to name builds": the packaging guide in the system browser. Takes
/// no URL: the page is fixed.
#[tauri::command]
pub fn open_packaging_guide(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(PACKAGING_GUIDE_URL, None::<&str>)
        .map_err(|e| e.to_string())
}
