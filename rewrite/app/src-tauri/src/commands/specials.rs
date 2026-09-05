//! Tauri commands for the "specials" surface: extra content (update/DLC),
//! native (Windows) game launch settings, compat tools, the RPCS3 PS3
//! firmware button, and the per-game download cancel.
//!
//! Thin wrappers only, in the style of `commands/cloud.rs`: every rule lives
//! in grid-core (`library::content`, `library::specials::native`,
//! `launch::compat`, `launch::catalog`, `firmware::rpcs3`) or, for the
//! background firmware triggers, in `crate::firmware_service`. `commands.rs`
//! is already 1000+ lines, hence this submodule.
//!
//! Blocking rule (inherited from `commands.rs`): `State` is not `Send`, so
//! every command reads what it needs out of state first and only then hops
//! to `spawn_blocking` for registry, config, or filesystem work.

use std::path::{Path, PathBuf};

use grid_core::config::Config;
use grid_core::firmware::rpcs3::{rpcs3_pup_path, spawn_rpcs3_installfw};
use grid_core::launch::catalog::{
    compat_tool_catalog_entries, mark_compat_installed, CatalogEntry,
};
use grid_core::launch::compat::{discover, which_on_path, CompatTool};
use grid_core::launch::profiles::load_profiles;
use grid_core::launch::selection::emulator_entry_by_name;
use grid_core::launch::template::host_os;
use grid_core::library::content::{
    content_availability as compute_availability, ContentAvailability, ContentKind,
};
use grid_core::library::paths::{archive_name, candidate_archives, library_root};
use grid_core::library::specials::native;
use serde::Serialize;
use tauri::{Emitter, State};

use super::{err, AppState};

/// Emitted after [`set_default_compat_tool`] and after a managed compat-tool
/// install finalizes (`InstallService::set_compat_tools_hook`, wired in
/// `lib.rs`). The frontend re-runs `listCompatTools` on it.
pub const COMPAT_TOOLS_CHANGED_EVENT: &str = "compat-tools-changed";

// --- extra content ------------------------------------------------------------

#[tauri::command]
pub async fn install_content(
    state: State<'_, AppState>,
    rom_id: i64,
    kind: String,
) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    let parsed =
        ContentKind::parse(&kind).ok_or_else(|| format!("unknown content kind: {kind}"))?;
    let client = state.session.client().ok_or("not connected")?;
    install
        .install_content(client, rom_id, parsed)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn install_native_update(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    let client = state.session.client().ok_or("not connected")?;
    install
        .install_native_update(client, rom_id)
        .await
        .map_err(err)
}

/// Which extra content kinds the server lists files for. A live fetch rather
/// than a registry read: the answer changes when the server's file list does,
/// and the details view asks for it only when it is open.
#[tauri::command]
pub async fn content_availability(
    state: State<'_, AppState>,
    rom_id: i64,
) -> Result<ContentAvailability, String> {
    let client = state.session.client().ok_or("not connected")?;
    let detail = client.rom_detail(rom_id).await.map_err(err)?;
    Ok(compute_availability(&detail.files))
}

/// Why the primary Install button cannot install this game, or `""`. Config
/// + profile work only, so this runs on the blocking pool; the platform SLUG
/// comes from the process-wide registry through `installed_core_resolver`,
/// which is why the caller only has to send the platform NAME.
#[tauri::command]
pub async fn install_block_reason(platform: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(grid_core::launch::selection::install_block_reason(
            &platform,
            &config.emulators,
            load_profiles(),
            &grid_core::launch::selection::installed_core_resolver,
        ))
    })
    .await
    .map_err(|e| format!("install_block_reason did not finish: {e}"))?
}

// --- native game settings -----------------------------------------------------

/// The native-launch settings form's contents for one installed game.
/// `executable` is the RESOLVED executable (the pinned one when it is still
/// valid, else the first candidate), so the form shows what would actually
/// launch rather than a blank field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeGameSettings {
    pub executable: String,
    pub parameters: String,
    pub compat_tool: String,
    pub wineprefix: String,
    /// The game's install directory (`native::install_dir`), as a full path;
    /// `""` when neither a live extracted directory nor a candidate archive
    /// resolves one. Read-only display: the dialog shows it as its own row
    /// and labels every executable candidate relative to it
    /// (`grid_launcher/ui/dialogs.py:211-214`).
    pub install_dir: String,
    /// Every launchable file under the install directory, as full paths, in
    /// `native::executable_candidates` order (shallowest first).
    pub candidates: Vec<String>,
}

#[tauri::command]
pub async fn native_game_settings(
    state: State<'_, AppState>,
    rom_id: i64,
) -> Result<NativeGameSettings, String> {
    let registry = state.install.as_ref().map_err(Clone::clone)?.registry();
    tokio::task::spawn_blocking(move || {
        let row = registry
            .find(Some(rom_id), "", "")
            .map_err(err)?
            .ok_or("game is not installed")?;
        let config = Config::load(&Config::default_path()).map_err(err)?;
        // No library path configured only costs the archive fallback: a row
        // with a live `extracted_dir` still resolves.
        let archives = match library_root(&config) {
            Some(library) => candidate_archives(
                &library,
                &row.platform,
                &row.archive_path,
                &archive_name(&row.rom_file_name, &row.title, &row.platform),
            ),
            None => Vec::new(),
        };
        let dir = native::install_dir(&row, &archives);
        let candidates = match dir.as_ref() {
            Some(dir) => native::executable_candidates(dir),
            None => Vec::new(),
        };
        let executable = native::resolved_executable(&row, &candidates)
            .map(path_string)
            .unwrap_or_default();
        Ok(NativeGameSettings {
            executable,
            parameters: row.native_launch_parameters.clone(),
            compat_tool: row.native_compat_tool.clone(),
            wineprefix: row.native_wineprefix.clone(),
            install_dir: dir.map(path_string).unwrap_or_default(),
            candidates: candidates.iter().map(|p| path_string(p.clone())).collect(),
        })
    })
    .await
    .map_err(|e| format!("native_game_settings did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_native_game_settings(
    state: State<'_, AppState>,
    rom_id: i64,
    executable: String,
    parameters: String,
    compat_tool: String,
) -> Result<(), String> {
    let registry = state.install.as_ref().map_err(Clone::clone)?.registry();
    let compat_tool = normalize_compat_for_host(host_os(), &compat_tool);
    tokio::task::spawn_blocking(move || {
        // The `bool` is "a row matched"; a miss means the game was
        // uninstalled between opening the form and saving it, which is a UI
        // race rather than an error worth surfacing.
        registry
            .update_native_settings(rom_id, &executable, &parameters, &compat_tool)
            .map_err(err)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("set_native_game_settings did not finish: {e}"))?
}

/// The compat tool value to STORE for `host`. A Windows host runs native
/// Windows games directly, and [`discover`] offers no tools there at all, so
/// any value the frontend sends is dropped rather than persisted — otherwise
/// a config carried over from Linux would keep a Proton name that the launch
/// path would then try to honor.
fn normalize_compat_for_host(host: &str, value: &str) -> String {
    if host.trim().to_lowercase().starts_with("win") {
        return String::new();
    }
    value.to_string()
}

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

// --- compat tools -------------------------------------------------------------

/// [`list_compat_tools`]'s return shape: the discovered tools plus the
/// configured default, so the picker can render and preselect in one call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatToolsDto {
    pub tools: Vec<CompatTool>,
    pub default_tool: String,
}

#[tauri::command]
pub async fn list_compat_tools() -> Result<CompatToolsDto, String> {
    // `discover` walks three Steam roots and canonicalizes every candidate:
    // filesystem work, so the blocking pool.
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let home = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .unwrap_or_default();
        let tools = discover(&home, &config.compat_tool_installs, host_os(), &|name| {
            which_on_path(name)
        });
        Ok(CompatToolsDto {
            tools,
            default_tool: config.default_compat_tool,
        })
    })
    .await
    .map_err(|e| format!("list_compat_tools did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_default_compat_tool(app: tauri::AppHandle, value: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        crate::config_write::modify_config(&Config::default_path(), |config| {
            config.default_compat_tool = value;
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_default_compat_tool did not finish: {e}"))??;
    let _ = app.emit(COMPAT_TOOLS_CHANGED_EVENT, ());
    Ok(())
}

/// The compat-tool half of the install-from-catalog listing, marked against
/// `config.compat_tool_installs` (NOT `config.emulators` — a managed compat
/// tool never gets an emulator entry).
#[tauri::command]
pub fn list_compat_tool_catalog(state: State<'_, AppState>) -> Result<Vec<CatalogEntry>, String> {
    // Surfaces an install-service construction failure early, exactly like
    // `list_emulator_catalog`: the same error the Install button would hit.
    state.install.as_ref().map_err(Clone::clone)?;
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let mut entries = compat_tool_catalog_entries(load_profiles());
    mark_compat_installed(&mut entries, &config);
    Ok(entries)
}

#[tauri::command]
pub async fn install_compat_tool(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.install_compat_tool(source_id).await.map_err(err)
}

// --- RPCS3 PS3 firmware -------------------------------------------------------

/// Whether an RPCS3 install already has its `PS3UPDAT.PUP`, and where.
/// `None` means there is no PUP to install yet, so the frontend renders no
/// firmware note and no Install Firmware button — the button exists to hand
/// an already-downloaded PUP to RPCS3 (Python parity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rpcs3FirmwareStatus {
    pub pup_path: Option<String>,
}

#[tauri::command]
pub async fn rpcs3_firmware_status(emulator_name: String) -> Result<Rpcs3FirmwareStatus, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let pup = emulator_entry_by_name(&config.emulators, &emulator_name)
            .and_then(|entry| rpcs3_pup_path(&entry.path));
        Ok(Rpcs3FirmwareStatus {
            pup_path: pup.map(path_string),
        })
    })
    .await
    .map_err(|e| format!("rpcs3_firmware_status did not finish: {e}"))?
}

/// Hands the already-downloaded PUP to RPCS3's own `--installfw`. `false`
/// when the emulator name matches no entry, no PUP is present yet, or the
/// spawn failed — the caller re-reads [`rpcs3_firmware_status`] either way.
///
/// The PUP itself is fetched by
/// [`crate::firmware_service::FirmwareService::spawn_ps3_firmware`], not
/// here: this command only runs the emulator-side install step.
#[tauri::command]
pub async fn install_ps3_firmware(emulator_name: String) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let Some(entry) = emulator_entry_by_name(&config.emulators, &emulator_name) else {
            return Ok(false);
        };
        let Some(pup) = rpcs3_pup_path(&entry.path) else {
            return Ok(false);
        };
        let exe = grid_core::autoconfig::paths::expand_user(&entry.path);
        Ok(spawn_rpcs3_installfw(Path::new(&exe), &pup))
    })
    .await
    .map_err(|e| format!("install_ps3_firmware did not finish: {e}"))?
}

// --- per-game cancel ----------------------------------------------------------

/// Cancels whatever this game currently has in flight — a base install, an
/// update, or a DLC — without the caller having to know the drawer entry id.
#[tauri::command]
pub fn cancel_download_for_rom(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.cancel_for_rom(rom_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_game_settings_carries_the_install_directory() {
        let dto = NativeGameSettings {
            executable: "/games/My Game/game/MyGame/mygame.exe".to_string(),
            parameters: "--fullscreen".to_string(),
            compat_tool: "wine".to_string(),
            wineprefix: "/games/My Game/prefix".to_string(),
            install_dir: "/games/My Game/game".to_string(),
            candidates: vec!["/games/My Game/game/MyGame/mygame.exe".to_string()],
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["install_dir"], "/games/My Game/game");
    }

    #[test]
    fn a_windows_host_stores_no_compat_tool() {
        assert_eq!(normalize_compat_for_host("windows", "GE-Proton"), "");
        assert_eq!(normalize_compat_for_host("Windows", "wine"), "");
        assert_eq!(normalize_compat_for_host(" WIN32 ", "wine"), "");
    }

    #[test]
    fn other_hosts_store_the_value_verbatim() {
        assert_eq!(
            normalize_compat_for_host("linux", "GE-Proton9-20"),
            "GE-Proton9-20"
        );
        assert_eq!(normalize_compat_for_host("macos", "wine"), "wine");
        assert_eq!(normalize_compat_for_host("linux", ""), "");
    }
}
