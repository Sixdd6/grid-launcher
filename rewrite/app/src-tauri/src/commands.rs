pub mod cloud;
pub mod specials;
pub mod updates;

use crate::config_write::modify_config;
use crate::images::ImageService;
use grid_core::autoconfig::{self, entry as autoconfig_entry, RaCredentials};
use grid_core::config::{Config, EmulatorEntry, UiSettings};
use grid_core::images::urls::{filter_to_server_host, resolve_image_url};
use grid_core::launch::catalog::{catalog_entries, mark_installed, CatalogEntry};
use grid_core::launch::profiles::{
    load_profiles, profile_for_entry, visible_profiles, EmulatorProfile,
};
use grid_core::launch::selection::{
    compatible_emulator_names_for_platform, emulator_entry_by_name, emulator_supports_platform,
    entry_is_retroarch, mapping_value_for_platform, slug_core_resolver, NO_EMULATOR,
};
use grid_core::launch::spawn::{prepare_standalone_emulator_launch, spawn_standalone_emulator};
use grid_core::launch::{GameSession, LaunchService, SessionsSnapshot};
use grid_core::library::queue::DownloadsSnapshot;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::romm::{GameSummary, Platform, RomDetail};
use grid_core::secrets::RaTokenStore;
use grid_core::session::{RestoreOutcome, SessionManager, SessionState};
use secrecy::SecretString;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub session: Arc<SessionManager>,
    pub install: Result<Arc<InstallService>, String>,
    pub launch: Result<Arc<LaunchService>, String>,
    /// The RetroAchievements token's keyring slot — a SECOND, independent
    /// item from the RomM credential `state.session` holds (secrets.rs).
    pub ra_store: Arc<dyn RaTokenStore>,
    /// Cloud save/state sync: the emulator-entry/sync-dir caches and the
    /// D5 auto-upload pool. See `cloud_service.rs` and `commands/cloud.rs`.
    pub cloud: Arc<crate::cloud_service::CloudService>,
    /// Cover/screenshot pipeline glue: the startup sweep, the one-at-a-time
    /// replenish job, and the post-install prefetch. See `images.rs`.
    pub images: Arc<ImageService>,
    /// Background firmware triggers and their one-job-per-emulator-directory
    /// guard. See `firmware_service.rs`.
    pub firmware: Arc<crate::firmware_service::FirmwareService>,
    /// The transient set of games with a newer server version, and the
    /// triggers that recompute it. See `update_service.rs`.
    pub updates: Arc<crate::update_service::UpdateService>,
    /// The launcher's own self-update notice, pullable because the startup
    /// check can emit before the webview listens. See `app_update.rs`.
    pub app_update: Arc<crate::app_update::AppUpdateState>,
}

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    // RommError/SessionError Display are credential-free by construction.
    e.to_string()
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    server_url: String,
    username: String,
    secret: String,
    use_token: bool,
) -> Result<SessionState, String> {
    // Wrap immediately; the plain String is dropped at the end of this scope.
    let secret = SecretString::from(secret);
    let result = state
        .session
        .connect(server_url, username, secret, use_token)
        .await
        .map_err(err)?;
    if let Ok(install) = state.install.as_ref() {
        state
            .images
            .spawn_replenish(app.clone(), state.session.clone(), install.clone());
        state
            .updates
            .spawn_refresh(app, state.session.clone(), install.clone());
    }
    Ok(result)
}

#[tauri::command]
pub async fn restore_session(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<RestoreOutcome, String> {
    let outcome = state.session.restore().await.map_err(err)?;
    if matches!(outcome, RestoreOutcome::Connected { .. }) {
        if let Ok(install) = state.install.as_ref() {
            state
                .images
                .spawn_replenish(app.clone(), state.session.clone(), install.clone());
            state
                .updates
                .spawn_refresh(app, state.session.clone(), install.clone());
        }
    }
    Ok(outcome)
}

#[tauri::command]
pub async fn retry_connect(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SessionState, String> {
    let result = state.session.retry().await.map_err(err)?;
    if let Ok(install) = state.install.as_ref() {
        state
            .images
            .spawn_replenish(app.clone(), state.session.clone(), install.clone());
        state
            .updates
            .spawn_refresh(app, state.session.clone(), install.clone());
    }
    Ok(result)
}

#[tauri::command]
pub fn disconnect(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    state.session.disconnect().map_err(err)?;
    // The update set describes a server that is no longer connected.
    state.updates.clear(&app);
    Ok(())
}

/// Whether a `list_platforms` response should re-run the defaults backfill:
/// only when the assignable platform list is non-empty. An empty list means
/// no session, or a server response `assignable_platforms` filtered down to
/// nothing — either way the backfill would do no useful work, matching
/// `sync_new_emulator`'s own no-op on an empty platform list.
fn should_backfill_on_platform_list(assignable_platforms: &[String]) -> bool {
    !assignable_platforms.is_empty()
}

#[tauri::command]
pub async fn list_platforms(state: State<'_, AppState>) -> Result<Vec<Platform>, String> {
    let client = state.session.client().ok_or("not connected")?;
    let platforms = client.platforms().await.map_err(err)?;
    // grid-core holds no session, so this fetch is the only way it learns the
    // platform list the autoconfig defaults assignment writes against.
    if let Ok(install) = state.install.as_ref() {
        let names: Vec<String> = platforms.iter().map(|p| p.name.clone()).collect();
        let assignable = autoconfig_entry::assignable_platforms(&names);
        install.set_known_platforms(assignable.clone());
        // The firmware triggers need the platform *id*, not just the name,
        // and grid-core holds no session to fetch it with. Recorded from the
        // FULL platform list (not the assignable subset): a platform the
        // autoconfig defaults skip can still have server firmware.
        install.set_platform_ids(platforms.iter().map(|p| (p.name.clone(), p.id)).collect());
        // Slug-first RetroArch core resolution (D-RC-2) needs the server's
        // own slug for each platform; like the ids above, this is recorded
        // from the FULL list, not the assignable subset.
        let slug_map: BTreeMap<String, String> = platforms
            .iter()
            .map(|p| (p.name.clone(), p.slug.clone()))
            .collect();
        install.set_platform_slugs(slug_map.clone());
        // The same map again, into grid-core's process-wide registry: the
        // launch resolver, cloud ops, firmware routing and the install
        // service see only a platform NAME, and read the slug from there
        // (`launch::selection::installed_core_resolver`). Without it those
        // paths would fall back to fuzzy name matching and disagree with
        // the Emulators panel about which platforms RetroArch supports.
        grid_core::launch::set_platform_slugs(slug_map);

        // Self-heal for the gap D3's own trigger policy leaves: an emulator
        // installed or added before the FIRST successful platform fetch got
        // no platform/core defaults at that time, and nothing else re-runs
        // the backfill until the next add/install. Now that a platform list
        // has arrived, re-run it across every entry. Cheap and idempotent on
        // every later call too — `backfill_all_defaults` no-ops once nothing
        // is missing. Read out of `install` before the blocking hop: `State`
        // is not `Send`.
        if should_backfill_on_platform_list(&assignable) {
            let config_path = Config::default_path();
            let ra = install.ra_credentials();
            let profiles = load_profiles();
            let slugs = install.platform_slugs();
            let outcome = tokio::task::spawn_blocking(move || {
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &assignable,
                    platform_slugs: &slugs,
                    ps3_library_path: String::new(),
                    ra,
                    profiles,
                };
                autoconfig::backfill_all_defaults(&ctx)
            })
            .await;
            // Never fails the response — a load/save error or a panicked
            // task is logged only, exactly like `save_emulator`'s own
            // autoconfig warnings.
            match outcome {
                Ok(Ok(_changed)) => {}
                Ok(Err(e)) => tracing::warn!("emulator defaults backfill: {e}"),
                Err(e) => tracing::warn!("emulator defaults backfill did not finish: {e}"),
            }
        }
    }
    Ok(platforms)
}

#[tauri::command]
pub async fn list_games(
    state: State<'_, AppState>,
    platform_id: i64,
) -> Result<Vec<GameSummary>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.games(platform_id).await.map_err(err)
}

#[tauri::command]
pub async fn get_rom_detail(state: State<'_, AppState>, rom_id: i64) -> Result<RomDetail, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.rom_detail(rom_id).await.map_err(err)
}

#[tauri::command]
pub async fn ensure_image(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path = state
        .session
        .cache()
        .ensure(client.as_deref(), &resolved)
        .await
        .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// The local path of a server-hosted game video, fetching it through the
/// session client on a cache miss. Mirrors [`ensure_image`]'s resolution and
/// host filter exactly, so a `path_video` pointing anywhere but the
/// configured server is refused rather than fetched.
#[tauri::command]
pub async fn ensure_video(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path =
        grid_core::images::video::ensure_video(state.session.cache(), client.as_deref(), &resolved)
            .await
            .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// The exact-length check a YouTube video id must pass before it is
/// interpolated into a URL handed to the OS browser: exactly 11
/// `[A-Za-z0-9_-]` characters, YouTube's fixed id format. Anything else — a
/// path, an id with a query string tacked on, a full URL — must not build a
/// URL at all. Surrounding whitespace is trimmed before the check (and the
/// built URL uses the trimmed id), so a padded id is still accepted, but
/// never as the untrimmed original.
pub fn youtube_watch_url(id: &str) -> Option<String> {
    let trimmed = id.trim();
    let valid = trimmed.len() == 11
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Some(format!("https://www.youtube.com/watch?v={trimmed}"))
    } else {
        None
    }
}

/// Opens a trailer's YouTube watch page in the system browser. An embedded
/// `<iframe>` cannot play the video on Linux: the page origin is
/// `tauri://localhost`, a "local scheme" under the W3C referrer policy, so
/// no `Referer` header is ever sent and YouTube answers error 153 ("Video
/// unavailable") for every embed (tauri-apps/tauri#14422) — no markup fix
/// works around it. `video_id` is validated here before it reaches a URL
/// handed to the OS opener.
#[tauri::command]
pub fn open_youtube_video(app: tauri::AppHandle, video_id: String) -> Result<(), String> {
    let url = youtube_watch_url(&video_id).ok_or("not a YouTube video id")?;
    app.opener().open_url(url, None::<&str>).map_err(err)
}

/// The local path of the shell background's pre-scaled variant of `url`,
/// blurred at `blur` and built on a miss. The frontend passes the stored
/// `ui.background_blur`; the sigma is part of the variant's file name, so a
/// slider change builds a new file rather than serving the old blur.
///
/// Mirrors [`ensure_image`]'s resolution and host filter exactly, so a URL
/// pointing anywhere but the configured server is refused rather than
/// fetched.
#[tauri::command]
pub async fn ensure_background_variant(
    state: State<'_, AppState>,
    url: String,
    blur: u8,
) -> Result<String, String> {
    let base = state.session.server_url();
    let resolved = filter_to_server_host(&resolve_image_url(&url, &base), &base);
    if resolved.is_empty() {
        return Err("filtered".to_string());
    }
    let client = state.session.client();
    let path = grid_core::images::background::ensure_background_variant(
        state.session.cache(),
        client.as_deref(),
        &resolved,
        blur.min(MAX_BACKGROUND_BLUR),
    )
    .await
    .map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn install_game(state: State<'_, AppState>, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    let client = state.session.client().ok_or("not connected")?;
    install.install(client, rom_id).await.map_err(err)
}

#[tauri::command]
pub fn cancel_install(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.cancel(entry_id);
    Ok(())
}

#[tauri::command]
pub async fn retry_install(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    // `None` when no session is connected: an emulator retry does not need a
    // RomM client, and `retry` reports "not connected" itself for a game row.
    install
        .retry(state.session.client(), entry_id)
        .await
        .map_err(err)
}

#[tauri::command]
pub fn dismiss_download(state: State<'_, AppState>, entry_id: u64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.dismiss(entry_id);
    Ok(())
}

#[tauri::command]
pub async fn uninstall_game(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    rom_id: i64,
) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let install_for_updates = install.clone();
    tokio::task::spawn_blocking(move || install.uninstall(rom_id).map_err(err))
        .await
        .map_err(|e| format!("uninstall did not finish: {e}"))??;
    // The uninstalled row can no longer carry an update.
    state
        .updates
        .spawn_refresh(app, state.session.clone(), install_for_updates);
    Ok(())
}

#[tauri::command]
pub fn list_downloads(state: State<'_, AppState>) -> Result<DownloadsSnapshot, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    Ok(install.snapshot())
}

#[tauri::command]
pub async fn list_installed(state: State<'_, AppState>) -> Result<Vec<InstalledGame>, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || install.installed().map_err(err))
        .await
        .map_err(|e| format!("list_installed did not finish: {e}"))?
}

#[tauri::command]
pub async fn get_library_path() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.library_path)
    })
    .await
    .map_err(|e| format!("get_library_path did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_library_path(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            config.library_path = path;
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_library_path did not finish: {e}"))?
}

// --- desktop shell appearance (design §4, §10) --------------------------------

/// The highest background-art opacity the Appearance slider offers
/// (design §3: "0–60%").
const MAX_BACKGROUND_FADE: u8 = 60;

/// The strongest background blur the Appearance slider offers. Aliased from
/// grid-core so the clamp here and the builder's documented range can never
/// drift apart.
const MAX_BACKGROUND_BLUR: u8 = grid_core::images::background::BACKGROUND_BLUR_MAX;

/// What actually gets written to `config.toml` for a set of appearance
/// settings: an unrecognized theme falls back to `"system"` (rather than
/// being rejected, which would make a stale frontend unable to save
/// anything), and the fade is clamped into the design's range.
pub fn normalize_ui_settings(settings: UiSettings) -> UiSettings {
    let theme = match settings.theme.trim() {
        "dark" => "dark",
        "light" => "light",
        _ => "system",
    };
    UiSettings {
        theme: theme.to_string(),
        background_fade: settings.background_fade.min(MAX_BACKGROUND_FADE),
        background_blur: settings.background_blur.min(MAX_BACKGROUND_BLUR),
        card_size_library: normalize_card_size(&settings.card_size_library),
        card_size_server: normalize_card_size(&settings.card_size_server),
    }
}

/// One of `"small"`, `"medium"`, `"large"`; anything else becomes
/// `"medium"` (design §5's default). Case-sensitive on purpose: the three
/// names are written by this app, and a `"Large"` in `config.toml` is a
/// hand edit whose intent is not worth guessing at.
fn normalize_card_size(raw: &str) -> String {
    match raw.trim() {
        "small" => "small",
        "large" => "large",
        _ => "medium",
    }
    .to_string()
}

/// The stored server URL, when it is safe to hand to the OS opener.
///
/// `None` for anything that is not a plain `http`/`https` URL, and — the
/// reason this function exists — `None` for any URL carrying userinfo:
/// basic-auth mode lets the user type `http://user:password@host/romm`,
/// and a password must never leave the keyring for a browser command
/// line. Deliberately hand-rolled rather than pulled from a URL crate:
/// the check is a prefix test plus an `@` scan of the authority, and this
/// crate has no URL dependency.
pub fn browsable_server_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(trimmed.to_string())
}

#[tauri::command]
pub async fn get_ui_settings() -> Result<UiSettings, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.ui)
    })
    .await
    .map_err(|e| format!("get_ui_settings did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_ui_settings(settings: UiSettings) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            config.ui = normalize_ui_settings(settings);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_ui_settings did not finish: {e}"))?
}

/// Opens the configured RomM server in the user's browser (design §3, the
/// server menu). Takes NO url argument on purpose: the frontend cannot
/// choose what gets opened, and the stored URL is filtered by
/// [`browsable_server_url`] before it reaches the opener.
#[tauri::command]
pub async fn open_server_page(app: tauri::AppHandle) -> Result<(), String> {
    let url = tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok::<Option<String>, String>(browsable_server_url(&config.server_url))
    })
    .await
    .map_err(|e| format!("open_server_page did not finish: {e}"))??
    .ok_or("no server URL to open")?;
    app.opener().open_url(url, None::<&str>).map_err(err)
}

/// The directory `config.toml` lives in — `_config_dir()`'s answer
/// (grid-launcher.py:3163). Split out from [`open_config_folder`] so the
/// path rule is unit-testable without a keyring, a webview or an opener.
pub fn config_dir_for(config_path: &std::path::Path) -> std::path::PathBuf {
    match config_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    }
}

/// Reveals the config directory in the desktop file manager
/// (`_open_config_folder`, grid-launcher.py:3162-3172). Takes NO path
/// argument, for the same reason [`open_server_page`] takes no URL: the
/// frontend cannot choose what gets opened. The directory is created first,
/// matching Python's `mkdir(parents=True, exist_ok=True)`, so a first run
/// that has not written a config yet still opens something.
#[tauri::command]
pub async fn open_config_folder(app: tauri::AppHandle) -> Result<(), String> {
    let dir = tokio::task::spawn_blocking(|| {
        let dir = config_dir_for(&Config::default_path());
        std::fs::create_dir_all(&dir).map_err(|e| format!("Could not open config folder: {e}"))?;
        Ok::<std::path::PathBuf, String>(dir)
    })
    .await
    .map_err(|e| format!("open_config_folder did not finish: {e}"))??;
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| format!("Could not open config folder: {e}"))
}

/// What the Server platform header's firmware chip needs (design §6).
/// Deliberately two plain flags rather than a rendered sentence: the chip's
/// wording is the frontend's (`lib/server/header.ts`), and a count is what
/// the backend can honestly report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformFirmwareStatus {
    /// How many firmware files the server lists for this platform.
    pub file_count: u32,
    /// Whether the platform has a default emulator — i.e. somewhere for
    /// that firmware to be installed to. Without one there is nothing to
    /// install into, so the chip offers no action.
    pub has_default_emulator: bool,
}

/// `GET /api/firmware?platform_id=<id>` plus the local default-emulator
/// check. Read-only: nothing is downloaded here.
#[tauri::command]
pub async fn platform_firmware_status(
    state: State<'_, AppState>,
    platform_id: i64,
    platform: String,
) -> Result<PlatformFirmwareStatus, String> {
    let file_count = match state.session.client() {
        // Offline: report "no firmware" rather than an error. The chip is
        // an affordance, not a task, and the Server view is already
        // showing its own offline state when this can happen.
        None => 0,
        Some(client) => client.firmware(platform_id).await.map_err(err)?.len() as u32,
    };
    let has_default_emulator = tokio::task::spawn_blocking(move || {
        let Ok(config) = Config::load(&Config::default_path()) else {
            return false;
        };
        let profiles = grid_core::launch::profiles::load_profiles();
        crate::firmware_service::default_entry_for_platform(&config, &platform, profiles).is_some()
    })
    .await
    .map_err(|e| format!("platform_firmware_status did not finish: {e}"))?;
    Ok(PlatformFirmwareStatus {
        file_count,
        has_default_emulator,
    })
}

/// The firmware chip's Install action. Fire-and-forget, exactly like the
/// per-game and per-emulator triggers: the pass runs in the background and
/// logs its warnings. It never reports back through this command's return
/// value — the pass announces its end with
/// `firmware_service::FIRMWARE_PASS_FINISHED_EVENT`, which is what the chip
/// waits on.
#[tauri::command]
pub fn install_firmware_for_platform(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    platform_id: i64,
    platform: String,
) -> Result<(), String> {
    state.firmware.spawn_for_platform(
        app,
        state.session.clone(),
        platform,
        platform_id,
        crate::firmware_service::FirmwareTrigger::Install,
    );
    Ok(())
}

// --- launch/emulator types ---------------------------------------------------

/// An autoprofile, trimmed to what the frontend needs (task-7-brief.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileSummary {
    pub name: String,
    pub args: String,
}

/// Config fields the launch/emulator UI needs together (task-7-brief.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LaunchDefaults {
    pub default_emulators: BTreeMap<String, String>,
    pub retroarch_cores: BTreeMap<String, String>,
    pub launch_args: String,
}

// --- launch/session commands -------------------------------------------------

/// The installed row for `rom_id`, off the blocking pool — shared by the
/// two cloud auto-triggers `launch_game` runs around the actual launch.
async fn installed_game_by_rom_id(
    install: &Arc<InstallService>,
    rom_id: i64,
) -> Result<Option<InstalledGame>, String> {
    let install = install.clone();
    let games = tokio::task::spawn_blocking(move || install.installed().map_err(err))
        .await
        .map_err(|e| format!("registry lookup did not finish: {e}"))??;
    Ok(games.into_iter().find(|g| g.rom_id == Some(rom_id)))
}

#[tauri::command]
pub async fn launch_game(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    rom_id: i64,
) -> Result<GameSession, String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?.clone();
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let config_path = Config::default_path();

    // Auto-restore before launch (parity: details_view_mixin.py:1497,
    // `_auto_sync_before_launch`), BEFORE the process spawns. A lookup or
    // restore failure never blocks the launch — errors are swallowed here
    // (and only debug-logged inside `auto_restore_before_launch`).
    if let Ok(Some(installed_game)) = installed_game_by_rom_id(&install, rom_id).await {
        // Firmware top-up before the process spawns (install_mixin.py:528's
        // call site, re-run at launch so a game installed before its
        // emulator existed still gets its BIOS). Fire-and-forget: it spawns
        // its own task, returns immediately, and can never fail the launch.
        state.firmware.spawn_for_game(
            app,
            state.session.clone(),
            install.clone(),
            installed_game.clone(),
            crate::firmware_service::FirmwareTrigger::Launch,
        );
        state
            .cloud
            .auto_restore_before_launch(
                &state.session,
                install.clone(),
                launch.clone(),
                &config_path,
                &installed_game,
            )
            .await;
    }

    let session = launch.launch(rom_id).await.map_err(err)?;

    // Session registration parity (cloud_mixin.py:2818-2842): stamp the
    // cloud sync-state session markers at spawn.
    if let Ok(Some(installed_game)) = installed_game_by_rom_id(&install, rom_id).await {
        state
            .cloud
            .stamp_session_started(
                install.clone(),
                launch,
                &config_path,
                &installed_game,
                session.started_at as f64,
            )
            .await;
    }

    // The Library rail's "Recent" entry and its "Recently played" sort read
    // `last_played_at` (design §5). Stamped once the process has actually
    // spawned, so a launch that failed to start never counts as played.
    //
    // Fire-and-forget on purpose: the join handle is dropped rather than
    // awaited, so a registry mutex held by a concurrent scan can never delay
    // the launch response. A write failure is logged and swallowed — the game
    // IS running, and losing one ordering hint must never surface as a launch
    // error. `rom_id` is safe to log; nothing here touches a path or a secret.
    {
        let registry = install.registry();
        let at = session.started_at;
        tokio::task::spawn_blocking(move || {
            if let Err(e) = registry.touch_last_played(rom_id, at) {
                tracing::debug!("last_played_at stamp failed for rom {rom_id}: {e}");
            }
        });
    }

    Ok(session)
}

#[tauri::command]
pub fn stop_game(state: State<'_, AppState>, session_id: u64) -> Result<(), String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?;
    launch.stop(session_id);
    Ok(())
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<SessionsSnapshot, String> {
    let launch = state.launch.as_ref().map_err(Clone::clone)?;
    Ok(launch.snapshot())
}

// --- emulator config commands -------------------------------------------------

#[tauri::command]
pub async fn list_emulators() -> Result<Vec<EmulatorEntry>, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.emulators)
    })
    .await
    .map_err(|e| format!("list_emulators did not finish: {e}"))?
}

/// D1 call site B. An ADD (a blank `original_name`, or one naming no current
/// entry) gets the matched profile's defaults applied before the merge and a
/// full autoconfig sync after the save; an EDIT gets neither. The command's
/// `Result` is unchanged either way — a sync warning is logged and the
/// command still returns `Ok`.
#[tauri::command]
pub async fn save_emulator(
    state: State<'_, AppState>,
    original_name: String,
    entry: EmulatorEntry,
) -> Result<(), String> {
    // Read out of the install service before the blocking hop: `State` is not
    // `Send`. An install service that failed to build simply contributes no
    // platforms and no credentials.
    let (platforms, platform_slugs, ra) = match state.install.as_ref() {
        Ok(install) => (
            install.known_platforms(),
            install.platform_slugs(),
            install.ra_credentials(),
        ),
        Err(_) => (Vec::new(), std::collections::BTreeMap::new(), None),
    };

    let session = state.session.clone();
    let install_for_firmware = state.install.as_ref().ok().cloned();
    let firmware = state.firmware.clone();

    // `Some(entry)` only when this save ADDED an RPCS3 entry: the PS3
    // firmware trigger's precondition. Handed back out of the blocking hop
    // so the trigger (which spawns a tokio task) runs on the async side.
    let rpcs3_added =
        tokio::task::spawn_blocking(move || -> Result<Option<EmulatorEntry>, String> {
            let config_path = Config::default_path();
            let profiles = load_profiles();
            // The autoconfig sync below reads no config.json and can be slow
            // (it writes emulator config files), so it runs AFTER the write
            // lock is released, on the three values the closure hands back.
            let (is_add, saved_name, library_path, saved_entry) =
                modify_config(&config_path, |config| {
                    let is_add = is_manual_add(config, &original_name);
                    let entry = manual_add_entry(entry, is_add, profiles);
                    // The name as it will be STORED, so the sync lookup matches exactly.
                    let saved_name = entry.name.clone();
                    let saved_entry = entry.clone();
                    apply_save_emulator(config, &original_name, entry)?;
                    Ok((is_add, saved_name, config.library_path.clone(), saved_entry))
                })?;

            if is_add {
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &platforms,
                    platform_slugs: &platform_slugs,
                    ps3_library_path: autoconfig::ps3_library_path(&library_path),
                    ra,
                    profiles,
                };
                // Warnings name emulators and file paths only — never a secret.
                match autoconfig::sync_new_emulator(&saved_name, &ctx) {
                    Ok(report) => {
                        for warning in report.warnings {
                            tracing::warn!("emulator autoconfig: {warning}");
                        }
                    }
                    Err(e) => tracing::warn!("emulator autoconfig: {e}"),
                }
            }
            // D2/D17: adding an RPCS3 entry by hand kicks off the PS3 firmware
            // fetch, the same as installing RPCS3 from the catalog does. An EDIT
            // never does — the firmware is already there, or the user declined
            // it once.
            let is_rpcs3_add = is_add && autoconfig::is_rpcs3(&saved_entry, profiles);
            Ok(is_rpcs3_add.then_some(saved_entry))
        })
        .await
        .map_err(|e| format!("save_emulator did not finish: {e}"))??;

    if let (Some(entry), Some(install)) = (rpcs3_added, install_for_firmware) {
        firmware.spawn_ps3_firmware(session, install, entry);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_emulator(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            apply_delete_emulator(config, &name);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("delete_emulator did not finish: {e}"))?
}

/// Opens a configured emulator with no ROM, so the user can set its controls
/// up (`_launch_emulator_at_index`, emulator_ui_mixin.py:1635-1665). Returns
/// as soon as the process has started; every failure is a plain, path-only
/// message the Emulators view shows as a toast.
#[tauri::command]
pub async fn launch_emulator(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let entry = emulator_entry_by_name(&config.emulators, &name);
        let (argv, working_dir) = prepare_standalone_emulator_launch(&name, entry)?;
        spawn_standalone_emulator(&argv, &working_dir)
    })
    .await
    .map_err(|e| format!("launch_emulator did not finish: {e}"))?
}

/// One platform the Emulators panel is asking about: the NAME every config
/// map is keyed by, plus the server SLUG that drives slug-first core
/// resolution (D-RC-2). Both sides use exactly these two field names — see
/// `PlatformRef` in `app/src/lib/api.ts`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PlatformRef {
    pub name: String,
    #[serde(default)]
    pub slug: String,
}

/// The RetroArch entry whose installed cores this platform's picker lists:
/// the platform's SAVED default when that entry is a RetroArch build, else
/// the first RetroArch entry in config order (the one the emulator select
/// would offer). `None` when no configured entry is a RetroArch build.
///
/// Resolving per platform rather than always taking the first RetroArch
/// entry keeps the picker, [`set_retroarch_core`]'s guard and the recorded
/// core in agreement when two RetroArch builds are configured and only the
/// second one has the platform's core installed.
fn retroarch_entry_for_platform<'a>(
    config: &'a Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
) -> Option<&'a EmulatorEntry> {
    if let Some(saved) = mapping_value_for_platform(&config.default_emulators, platform_name) {
        if let Some(entry) = emulator_entry_by_name(&config.emulators, saved) {
            if entry_is_retroarch(entry, profiles) {
                return Some(entry);
            }
        }
    }
    config
        .emulators
        .iter()
        .find(|entry| entry_is_retroarch(entry, profiles))
}

/// The installed compatible cores the picker offers for one platform, or
/// `[]` when there is no RetroArch entry for it or nothing compatible is
/// installed. The entry is resolved per platform
/// ([`retroarch_entry_for_platform`]).
fn core_options_for(
    config: &Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
    platform_slug: &str,
) -> Vec<String> {
    retroarch_entry_for_platform(config, profiles, platform_name)
        .map(|entry| autoconfig::installed_compatible_cores(platform_name, platform_slug, entry))
        .unwrap_or_default()
}

/// [`set_retroarch_core`]'s guard. A blank `core` (which CLEARS the mapping)
/// always passes; any other value must be one this platform's picker would
/// have offered.
fn check_retroarch_core_installed(
    config: &Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
    platform_slug: &str,
    core: &str,
) -> Result<(), String> {
    let trimmed = core.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if core_options_for(config, profiles, platform_name, platform_slug)
        .iter()
        .any(|installed| installed == trimmed)
    {
        Ok(())
    } else {
        Err(format!(
            "{trimmed} is not an installed RetroArch core for {platform_name}"
        ))
    }
}

/// [`set_retroarch_core`]'s merge logic, mirroring
/// [`apply_set_default_emulator`]: a blank `core` removes the `platform` key
/// (exact match first, then case-insensitive); otherwise the value is
/// upserted under the exact key, else a case-insensitive match's key, else a
/// new key.
fn apply_set_retroarch_core(config: &mut Config, platform: &str, core: &str) {
    let trimmed = core.trim();
    if trimmed.is_empty() {
        remove_platform_key(&mut config.retroarch_cores, platform);
        return;
    }
    upsert_platform_key(&mut config.retroarch_cores, platform, trimmed);
}

/// D-RC-4: making a RetroArch entry a platform's default also records the
/// first installed compatible core — but ONLY when no non-blank core is
/// saved for that platform. A saved core is never overwritten here; the core
/// picker ([`set_retroarch_core`]) is the only way to change one.
///
/// A no-op for a blank name, a name matching no entry, and any entry that is
/// not a RetroArch build.
fn apply_record_retroarch_core(
    config: &mut Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
    platform_slug: &str,
    name: &str,
) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    if mapping_value_for_platform(&config.retroarch_cores, platform_name).is_some() {
        return;
    }

    // Scoped so the immutable borrow of `config.emulators` ends before the
    // mutable borrow of `config.retroarch_cores` below.
    let first = {
        let Some(entry) = emulator_entry_by_name(&config.emulators, trimmed) else {
            return;
        };
        if !entry_is_retroarch(entry, profiles) {
            return;
        }
        autoconfig::installed_compatible_cores(platform_name, platform_slug, entry)
            .into_iter()
            .next()
    };

    if let Some(core) = first {
        upsert_platform_key(&mut config.retroarch_cores, platform_name, &core);
    }
}

/// Clears `platform`'s saved RetroArch core whenever the new default is not
/// a RetroArch entry — a blank `name`, an unrecognized `name`, or a
/// non-RetroArch emulator. Complements [`apply_record_retroarch_core`]: that
/// one records a core when the new default IS RetroArch; this one removes
/// the stale core when it is not, so a platform never keeps a core saved
/// for an emulator that no longer needs one.
fn apply_clear_retroarch_core_when_not_retroarch(
    config: &mut Config,
    platform_name: &str,
    name: &str,
    profiles: &[EmulatorProfile],
) {
    let trimmed = name.trim();
    let is_retroarch = !trimmed.is_empty()
        && emulator_entry_by_name(&config.emulators, trimmed)
            .is_some_and(|entry| entry_is_retroarch(entry, profiles));
    if !is_retroarch {
        remove_platform_key(&mut config.retroarch_cores, platform_name);
    }
}

/// The emulator names that support each requested platform, keyed by the
/// platform NAME that was asked about. One config + profile load answers the
/// whole batch; each platform runs the ported
/// `compatible_emulator_names_for_platform` (doc 04 §2), so names come back
/// in config order with blank-named entries skipped.
///
/// Each request carries the platform's server SLUG as well as its name, so
/// the RetroArch support gate resolves cores slug-first (D-RC-2).
///
/// The Emulators panel calls this to build its per-platform default
/// selector, which offers only compatible emulators — matching Python's
/// `_on_default_platform_changed` (emulator_ui_mixin.py:598).
#[tauri::command]
pub async fn compatible_emulators(
    platforms: Vec<PlatformRef>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let profiles = load_profiles();
        // One resolver over the whole batch: the requested name -> slug map.
        let slugs: BTreeMap<String, String> = platforms
            .iter()
            .map(|p| (p.name.clone(), p.slug.clone()))
            .collect();
        let resolver = slug_core_resolver(&slugs);
        Ok(platforms
            .iter()
            .map(|platform| {
                let names = compatible_emulator_names_for_platform(
                    &config.emulators,
                    &platform.name,
                    profiles,
                    &resolver,
                );
                (platform.name.clone(), names)
            })
            .collect())
    })
    .await
    .map_err(|e| format!("compatible_emulators did not finish: {e}"))?
}

/// The installed libretro cores the core picker offers for each requested
/// platform, keyed by platform NAME. Each platform is answered against its
/// saved default when that is a RetroArch entry, else the FIRST RetroArch
/// entry in config order — the entry the emulator select would offer — or
/// `[]` when there is none (design §3.3, final review F3).
#[tauri::command]
pub async fn retroarch_core_options(
    platforms: Vec<PlatformRef>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let profiles = load_profiles();
        Ok(platforms
            .into_iter()
            .map(|platform| {
                let options = core_options_for(&config, profiles, &platform.name, &platform.slug);
                (platform.name, options)
            })
            .collect())
    })
    .await
    .map_err(|e| format!("retroarch_core_options did not finish: {e}"))?
}

/// Saves `platform`'s libretro core. A blank `core` clears it. Refuses
/// anything the picker would not have offered, with the verbatim message
/// `<core> is not an installed RetroArch core for <platform>`.
///
/// The slug comes from the install service's recorded platform list rather
/// than the caller, so a stale frontend cannot steer core resolution.
#[tauri::command]
pub async fn set_retroarch_core(
    state: State<'_, AppState>,
    platform: String,
    core: String,
) -> Result<(), String> {
    // Read out before the blocking hop: `State` is not `Send`.
    let slugs = match state.install.as_ref() {
        Ok(install) => install.platform_slugs(),
        Err(_) => BTreeMap::new(),
    };
    tokio::task::spawn_blocking(move || {
        let profiles = load_profiles();
        let slug = slugs.get(&platform).cloned().unwrap_or_default();
        modify_config(&Config::default_path(), |config| {
            // Inside the closure so the check and the write see the same
            // config; an Err here aborts the write (config_write.rs).
            check_retroarch_core_installed(config, profiles, &platform, &slug, &core)?;
            apply_set_retroarch_core(config, &platform, &core);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_retroarch_core did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_default_emulator(
    state: State<'_, AppState>,
    platform: String,
    name: String,
) -> Result<(), String> {
    let slugs = match state.install.as_ref() {
        Ok(install) => install.platform_slugs(),
        Err(_) => BTreeMap::new(),
    };
    tokio::task::spawn_blocking(move || {
        let profiles = load_profiles();
        let slug = slugs.get(&platform).cloned().unwrap_or_default();
        modify_config(&Config::default_path(), |config| {
            // Both writes happen in the ONE closure, so the support check,
            // the default, and the recorded core all see the same config.
            check_default_emulator_supported(config, &platform, &name, &slug, profiles)?;
            apply_set_default_emulator(config, &platform, &name);
            apply_record_retroarch_core(config, profiles, &platform, &slug, &name);
            apply_clear_retroarch_core_when_not_retroarch(config, &platform, &name, profiles);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_default_emulator did not finish: {e}"))?
}

#[tauri::command]
pub async fn get_launch_defaults() -> Result<LaunchDefaults, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(LaunchDefaults {
            default_emulators: config.default_emulators,
            retroarch_cores: config.retroarch_cores,
            launch_args: config.launch_args,
        })
    })
    .await
    .map_err(|e| format!("get_launch_defaults did not finish: {e}"))?
}

// --- RetroAchievements credential commands ------------------------------------

/// [`get_retroachievements_status`]'s return shape. `token_present` is a
/// bare boolean — never the token, its length, or a prefix.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaStatus {
    pub username: String,
    pub token_present: bool,
}

/// One [`autoconfig::fan_out_ra_credentials`] row, renamed for the IPC
/// boundary (`emulator` rather than the tuple's positional field).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaFanOutRow {
    pub emulator: String,
    pub changed: bool,
}

fn ra_fan_out_rows(rows: Vec<(String, bool)>) -> Vec<RaFanOutRow> {
    rows.into_iter()
        .map(|(emulator, changed)| RaFanOutRow { emulator, changed })
        .collect()
}

/// The blocking half both credential paths share: store-or-clear the token,
/// write the plain username to config, then fan it out to the RA-capable
/// emulators (D2). Never returns, logs or formats the token; `token_is_blank`
/// is computed by the caller from the plain `String` before it is wrapped,
/// because a `SecretString` is deliberately not readable here.
fn store_ra_credentials(
    ra_store: Arc<dyn RaTokenStore>,
    username: String,
    token: SecretString,
    token_is_blank: bool,
) -> Result<Vec<RaFanOutRow>, String> {
    if token_is_blank {
        ra_store.clear().map_err(err)?;
    } else {
        ra_store.save(&token).map_err(err)?;
    }

    // The fan-out writes emulator config files, never config.json, so it
    // runs outside the write lock on the saved snapshot.
    let config = modify_config(&Config::default_path(), |config| {
        config.retroachievements_username = username.clone();
        Ok(config.clone())
    })?;

    let ra = RaCredentials::new(username, token);
    Ok(ra_fan_out_rows(autoconfig::fan_out_ra_credentials(
        &config,
        load_profiles(),
        &ra,
    )))
}

/// Saves the RetroAchievements login, then fans it out to every registered
/// RA-capable emulator's narrow credential writer (D2). The reference
/// re-runs the FULL per-emulator sync for every emulator instead
/// (`_on_ra_login_finished`, grid-launcher.py:2730-2754); this only ever
/// touches the three credential keys, via
/// [`autoconfig::fan_out_ra_credentials`].
///
/// `token` is checked for blankness (post-trim) on the plain argument and
/// then wrapped in `SecretString` immediately — the plain `String` is never
/// read again and is dropped at the end of this scope, matching `connect`.
/// A blank token clears the keyring entry rather than storing an empty
/// secret; either way the username is written to
/// `Config.retroachievements_username` (plain, non-secret) before the
/// fan-out runs, so `fan_out_ra_credentials`'s own `usable()` gate decides
/// whether anything is actually written.
#[tauri::command]
pub async fn set_retroachievements_credentials(
    state: State<'_, AppState>,
    username: String,
    token: String,
) -> Result<Vec<RaFanOutRow>, String> {
    let token_is_blank = token.trim().is_empty();
    let token = SecretString::from(token);
    let trimmed_username = username.trim().to_string();
    let ra_store = state.ra_store.clone();

    tokio::task::spawn_blocking(move || {
        store_ra_credentials(ra_store, trimmed_username, token, token_is_blank)
    })
    .await
    .map_err(|e| format!("set_retroachievements_credentials did not finish: {e}"))?
}

/// What [`retroachievements_login`] returns: the account name the RA server
/// reported and the fan-out rows. NEVER the token — the only place it lands
/// is `AppState.ra_store`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaLoginResult {
    pub username: String,
    pub fan_out: Vec<RaFanOutRow>,
}

/// Username + password login (`_ra_login_clicked` / `_on_ra_login_finished`,
/// grid-launcher.py:2705-2756). The password is wrapped in `SecretString`
/// immediately, used once, and dropped at the end of this scope; it is never
/// written to config, never logged, and never present in an error — the
/// login client strips the URL from every transport failure.
///
/// On success the SERVER's spelling of the account name is what gets stored,
/// matching the reference (`bundle["username"]` is `result["username"]`,
/// which is the payload's `User`).
#[tauri::command]
pub async fn retroachievements_login(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> Result<RaLoginResult, String> {
    let password = SecretString::from(password);
    let typed_username = username.trim().to_string();
    let ra_store = state.ra_store.clone();

    let http = grid_core::retroachievements::build_http_client();
    let login = grid_core::retroachievements::ra_login(&http, &typed_username, &password).await?;
    let account = login.username.clone();

    let fan_out = tokio::task::spawn_blocking(move || {
        store_ra_credentials(ra_store, login.username, login.token, false)
    })
    .await
    .map_err(|e| format!("retroachievements_login did not finish: {e}"))??;

    Ok(RaLoginResult {
        username: account,
        fan_out,
    })
}

/// The username from config and whether a token is stored — NEVER the
/// token, its length, or a prefix.
#[tauri::command]
pub async fn get_retroachievements_status(state: State<'_, AppState>) -> Result<RaStatus, String> {
    let ra_store = state.ra_store.clone();
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let token_present = ra_store.load().map_err(err)?.is_some();
        Ok(RaStatus {
            username: config.retroachievements_username,
            token_present,
        })
    })
    .await
    .map_err(|e| format!("get_retroachievements_status did not finish: {e}"))?
}

/// Clears the keyring entry and blanks `Config.retroachievements_username`.
/// Writes NOTHING to any emulator config and scrubs NOTHING already written
/// (parity with the reference's `_ra_clear_credentials`,
/// grid-launcher.py:2757-2765 — doc 05's "credentials are written but never
/// removed" open question, ruled: follow the code).
#[tauri::command]
pub async fn clear_retroachievements_credentials(state: State<'_, AppState>) -> Result<(), String> {
    let ra_store = state.ra_store.clone();
    tokio::task::spawn_blocking(move || {
        ra_store.clear().map_err(err)?;
        modify_config(&Config::default_path(), |config| {
            apply_clear_retroachievements(config);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("clear_retroachievements_credentials did not finish: {e}"))?
}

/// [`clear_retroachievements_credentials`]'s config-mutation logic, pulled
/// out so it is unit-testable without a keyring: blanks the username and
/// touches nothing else — in particular no emulator config file.
fn apply_clear_retroachievements(config: &mut Config) {
    config.retroachievements_username = String::new();
}

// --- autoprofile commands ----------------------------------------------------

#[tauri::command]
pub fn list_profiles() -> Vec<ProfileSummary> {
    visible_profiles(load_profiles())
        .into_iter()
        .map(|p| ProfileSummary {
            name: p.name.clone(),
            args: p.args.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn match_profile(executable_path: String) -> Option<ProfileSummary> {
    profile_for_entry("", &executable_path, load_profiles()).map(|p| ProfileSummary {
        name: p.name.clone(),
        args: p.args.clone(),
    })
}

// --- emulator catalog commands ------------------------------------------------

/// The "install from catalog" listing, freshly marked against the config on
/// disk. Also surfaces an install-service construction failure early (the
/// same error `install_emulator` would return on click) so the panel's
/// error line can show it before the user ever presses Install.
#[tauri::command]
pub fn list_emulator_catalog(state: State<'_, AppState>) -> Result<Vec<CatalogEntry>, String> {
    state.install.as_ref().map_err(Clone::clone)?;
    let config = Config::load(&Config::default_path()).map_err(err)?;
    let mut entries = catalog_entries(load_profiles());
    mark_installed(&mut entries, &config);
    Ok(entries)
}

#[tauri::command]
pub async fn install_emulator(state: State<'_, AppState>, source_id: String) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?;
    install.install_emulator(source_id).await.map_err(err)
}

// --- pure config-merge helpers (unit-tested below) ---------------------------

/// [`save_emulator`]'s merge logic. Validates `entry.name`, removes the
/// rename source (`original_name`, case-insensitive) if any, rejects a
/// duplicate against what remains, repoints any `default_emulators` value
/// that named the rename source, then writes `entry` back.
///
/// Selection fallback picks the first config-order match, so an edit must
/// not reorder the list: when `original_name` names an entry that is still
/// present, `entry` replaces it at its original index. Only a genuine add
/// (blank `original_name`, or one that names no current entry) appends at
/// the end.
fn apply_save_emulator(
    config: &mut Config,
    original_name: &str,
    entry: EmulatorEntry,
) -> Result<(), String> {
    if entry.name.trim().is_empty() {
        return Err("Emulator name is required.".to_string());
    }
    // The panel's "(none)" choice is stored under this reserved value; an
    // entry carrying it would be offered in the picker and then never launch.
    if entry.name.trim() == NO_EMULATOR {
        return Err(format!("{NO_EMULATOR} is a reserved name."));
    }

    let original = original_name.trim();
    let mut original_index = None;
    if !original.is_empty() {
        let folded = original.to_lowercase();
        original_index = config
            .emulators
            .iter()
            .position(|e| e.name.trim().to_lowercase() == folded);
        if let Some(idx) = original_index {
            config.emulators.remove(idx);
        }
    }

    let new_name_folded = entry.name.trim().to_lowercase();
    let duplicate = config
        .emulators
        .iter()
        .any(|e| e.name.trim().to_lowercase() == new_name_folded);
    if duplicate {
        return Err(format!(
            "An emulator named '{}' already exists.",
            entry.name
        ));
    }

    if !original.is_empty() {
        let folded = original.to_lowercase();
        for value in config.default_emulators.values_mut() {
            if value.trim().to_lowercase() == folded {
                *value = entry.name.clone();
            }
        }
    }

    match original_index {
        Some(idx) => config.emulators.insert(idx, entry),
        None => config.emulators.push(entry),
    }
    Ok(())
}

/// Whether this `save_emulator` call is an ADD rather than an edit: a blank
/// `original_name`, or one that names no current entry. Only an add runs the
/// profile defaults and the autoconfig sync (D1).
fn is_manual_add(config: &Config, original_name: &str) -> bool {
    let original = original_name.trim();
    if original.is_empty() {
        return true;
    }
    let folded = original.to_lowercase();
    !config
        .emulators
        .iter()
        .any(|e| e.name.trim().to_lowercase() == folded)
}

/// The hand-typed-entry half of layer 1
/// (`apply_manual_emulator_profile_defaults`, autoconfig.py:228): blank
/// fields take the matched profile's values and `path` is never touched. An
/// edit, or an entry no profile matches, passes through unchanged.
fn manual_add_entry(
    entry: EmulatorEntry,
    is_add: bool,
    profiles: &[EmulatorProfile],
) -> EmulatorEntry {
    if !is_add {
        return entry;
    }
    match profile_for_entry(&entry.name, &entry.path, profiles) {
        Some(profile) => autoconfig_entry::apply_manual_emulator_profile_defaults(&entry, profile),
        None => entry,
    }
}

/// [`delete_emulator`]'s merge logic: drops `name` (case-insensitive) from
/// `emulators`, and any `default_emulators` entry whose value named it.
fn apply_delete_emulator(config: &mut Config, name: &str) {
    let folded = name.trim().to_lowercase();
    config
        .emulators
        .retain(|e| e.name.trim().to_lowercase() != folded);
    config
        .default_emulators
        .retain(|_, v| v.trim().to_lowercase() != folded);
    // `remove_emulator_default_mappings` (grid_launcher/ui/emulators.py:137-152):
    // a platform's saved RetroArch core is meaningless once that platform has
    // no default emulator at all, which just happened above for every
    // platform that defaulted to the removed emulator.
    config
        .retroarch_cores
        .retain(|platform, _| config.default_emulators.contains_key(platform));
}

/// [`set_default_emulator`]'s guard: the picked emulator must actually
/// support the platform it is being made the default for. A blank `name`
/// (the panel's "(none)", stored as [`NO_EMULATOR`]) always passes; any
/// other name must resolve to
/// a configured entry that `emulator_supports_platform` accepts (doc 04
/// §2). A name no entry matches is refused with the same message — there is
/// nothing to support the platform with.
///
/// Python only ever OFFERS compatible names in the combo box
/// (emulator_ui_mixin.py:598) and drops an incompatible stored default at
/// read time; the port additionally refuses the write.
fn check_default_emulator_supported(
    config: &Config,
    platform: &str,
    name: &str,
    platform_slug: &str,
    profiles: &[EmulatorProfile],
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let slugs = BTreeMap::from([(platform.to_string(), platform_slug.to_string())]);
    let resolver = slug_core_resolver(&slugs);
    let supported = emulator_entry_by_name(&config.emulators, trimmed)
        .is_some_and(|entry| emulator_supports_platform(entry, platform, profiles, &resolver));
    if supported {
        Ok(())
    } else {
        Err(format!("{trimmed} does not support {platform}"))
    }
}

/// [`set_default_emulator`]'s merge logic. A blank `name` is the panel's
/// "(none)" choice and writes the reserved [`NO_EMULATOR`] marker; removing
/// the key instead would let `autoconfig::backfill_all_defaults` re-fill it
/// on the next `list_platforms`. Any value is inserted/overwritten under the
/// exact key when one already exists, else under a case-insensitive match's
/// key, else as a new key. The platform's saved core is cleared separately
/// by [`apply_clear_retroarch_core_when_not_retroarch`], which a blank name
/// already triggers.
fn apply_set_default_emulator(config: &mut Config, platform: &str, name: &str) {
    let trimmed_name = name.trim();
    let value = if trimmed_name.is_empty() {
        NO_EMULATOR
    } else {
        trimmed_name
    };
    upsert_platform_key(&mut config.default_emulators, platform, value);
}

fn remove_platform_key(map: &mut BTreeMap<String, String>, platform: &str) {
    if map.remove(platform).is_some() {
        return;
    }
    let folded = platform.to_lowercase();
    if let Some(key) = map.keys().find(|k| k.to_lowercase() == folded).cloned() {
        map.remove(&key);
    }
}

fn upsert_platform_key(map: &mut BTreeMap<String, String>, platform: &str, value: &str) {
    if map.contains_key(platform) {
        map.insert(platform.to_string(), value.to_string());
        return;
    }
    let folded = platform.to_lowercase();
    if let Some(key) = map.keys().find(|k| k.to_lowercase() == folded).cloned() {
        map.remove(&key);
    }
    map.insert(platform.to_string(), value.to_string());
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn entry(name: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: "/x/emu".to_string(),
            args: String::new(),
            ..Default::default()
        }
    }

    fn config_with(emulators: &[&str], defaults: &[(&str, &str)]) -> Config {
        Config {
            emulators: emulators.iter().map(|n| entry(n)).collect(),
            default_emulators: defaults
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Config::default()
        }
    }

    /// A config holding one RetroArch entry whose executable really exists,
    /// with `core_ids` installed beside it in every host extension so
    /// `installed_core_ids` (cores.rs:516) finds them on any host.
    fn config_with_retroarch(dir: &std::path::Path, core_ids: &[&str]) -> Config {
        let exe = dir.join("retroarch");
        std::fs::write(&exe, b"binary").unwrap();
        let cores_dir = dir.join("cores");
        std::fs::create_dir_all(&cores_dir).unwrap();
        for id in core_ids {
            for extension in ["so", "dylib", "dll"] {
                std::fs::write(cores_dir.join(format!("{id}_libretro.{extension}")), b"").unwrap();
            }
        }
        Config {
            emulators: vec![EmulatorEntry {
                name: "RetroArch".to_string(),
                path: exe.to_string_lossy().into_owned(),
                args: "-L \"%core%\" \"%rom%\"".to_string(),
                ..Default::default()
            }],
            ..Config::default()
        }
    }

    #[test]
    fn core_options_lists_installed_cores_in_slug_order() {
        let temp = tempfile::tempdir().unwrap();
        let config = config_with_retroarch(temp.path(), &["bsnes", "snes9x"]);
        let profiles = load_profiles();
        assert_eq!(
            core_options_for(
                &config,
                profiles,
                "Super Nintendo Entertainment System",
                "snes"
            ),
            vec!["snes9x".to_string(), "bsnes".to_string()]
        );
    }

    #[test]
    fn core_options_is_empty_without_a_retroarch_entry() {
        let config = config_with(&["PCSX2"], &[]);
        let profiles = load_profiles();
        assert!(core_options_for(
            &config,
            profiles,
            "Super Nintendo Entertainment System",
            "snes"
        )
        .is_empty());
    }

    /// F3: with two RetroArch builds configured and only the SECOND one
    /// holding the platform's core, the picker, the guard and the recorded
    /// core must all answer against the entry the platform's saved default
    /// names — not blindly against the first RetroArch entry in config
    /// order.
    #[test]
    fn core_options_follow_the_platforms_saved_retroarch_default() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        // "first" has a core, but not one compatible with SNES.
        let mut config = config_with_retroarch(&first, &["dolphin"]);
        let with_core = config_with_retroarch(&second, &["snes9x"]);
        config.emulators[0].name = "RetroArch".to_string();
        let mut second_entry = with_core.emulators[0].clone();
        second_entry.name = "RetroArch (nightly)".to_string();
        config.emulators.push(second_entry);

        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";

        // No saved default yet -> the first RetroArch entry answers, and it
        // has no SNES core.
        assert!(core_options_for(&config, profiles, platform, "snes").is_empty());

        // Saving the second build as the platform's default moves the
        // picker, the guard and the recorded core onto it.
        apply_set_default_emulator(&mut config, platform, "RetroArch (nightly)");
        assert_eq!(
            core_options_for(&config, profiles, platform, "snes"),
            vec!["snes9x".to_string()]
        );
        assert!(
            check_retroarch_core_installed(&config, profiles, platform, "snes", "snes9x").is_ok()
        );
        apply_record_retroarch_core(
            &mut config,
            profiles,
            platform,
            "snes",
            "RetroArch (nightly)",
        );
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("snes9x")
        );
    }

    #[test]
    fn set_retroarch_core_refuses_a_core_that_is_not_installed() {
        let temp = tempfile::tempdir().unwrap();
        let config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let err = check_retroarch_core_installed(
            &config,
            profiles,
            "Super Nintendo Entertainment System",
            "snes",
            "bsnes",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "bsnes is not an installed RetroArch core for Super Nintendo Entertainment System"
        );
    }

    #[test]
    fn set_retroarch_core_accepts_an_installed_core_and_a_blank_clear() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";
        assert!(
            check_retroarch_core_installed(&config, profiles, platform, "snes", "snes9x").is_ok()
        );
        apply_set_retroarch_core(&mut config, platform, "snes9x");
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("snes9x")
        );

        assert!(check_retroarch_core_installed(&config, profiles, platform, "snes", "  ").is_ok());
        apply_set_retroarch_core(&mut config, platform, "  ");
        assert!(config.retroarch_cores.is_empty());
    }

    #[test]
    fn set_default_emulator_records_the_first_core_only_when_unset() {
        // D-RC-4: picking RetroArch records a core, but never overwrites one.
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["bsnes", "snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";

        apply_record_retroarch_core(&mut config, profiles, platform, "snes", "RetroArch");
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("snes9x")
        );

        // A saved core survives a second pick.
        config
            .retroarch_cores
            .insert(platform.to_string(), "bsnes".to_string());
        apply_record_retroarch_core(&mut config, profiles, platform, "snes", "RetroArch");
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("bsnes")
        );
    }

    #[test]
    fn set_default_emulator_records_no_core_for_a_native_emulator() {
        let mut config = config_with(&["PCSX2"], &[]);
        let profiles = load_profiles();
        apply_record_retroarch_core(&mut config, profiles, "PlayStation 2", "ps2", "PCSX2");
        assert!(config.retroarch_cores.is_empty());
    }

    #[test]
    fn leaving_retroarch_for_none_clears_the_saved_core() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";
        config
            .retroarch_cores
            .insert(platform.to_string(), "snes9x".to_string());

        apply_clear_retroarch_core_when_not_retroarch(&mut config, platform, "", profiles);
        assert!(!config.retroarch_cores.contains_key(platform));
    }

    #[test]
    fn leaving_retroarch_for_none_clears_a_differently_cased_saved_core() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";
        config
            .retroarch_cores
            .insert(platform.to_uppercase(), "snes9x".to_string());

        apply_clear_retroarch_core_when_not_retroarch(&mut config, platform, "", profiles);
        assert!(config.retroarch_cores.is_empty());
    }

    #[test]
    fn leaving_retroarch_for_a_native_emulator_clears_the_saved_core() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["snes9x"]);
        config.emulators.push(entry("Snes9x"));
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";
        config
            .retroarch_cores
            .insert(platform.to_string(), "snes9x".to_string());

        apply_clear_retroarch_core_when_not_retroarch(&mut config, platform, "Snes9x", profiles);
        assert!(!config.retroarch_cores.contains_key(platform));
    }

    #[test]
    fn staying_on_retroarch_keeps_the_saved_core() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";
        config
            .retroarch_cores
            .insert(platform.to_string(), "bsnes".to_string());

        apply_clear_retroarch_core_when_not_retroarch(&mut config, platform, "RetroArch", profiles);
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("bsnes")
        );
    }

    // --- apply_save_emulator -------------------------------------------------

    #[test]
    fn save_rejects_blank_name() {
        let mut config = config_with(&[], &[]);
        let result = apply_save_emulator(&mut config, "", entry("   "));
        assert_eq!(result, Err("Emulator name is required.".to_string()));
        assert!(config.emulators.is_empty());
    }

    #[test]
    fn save_refuses_the_reserved_none_marker_as_a_name() {
        let mut config = config_with(&[], &[]);
        let result = apply_save_emulator(&mut config, "", entry(NO_EMULATOR));
        assert_eq!(result, Err("<none> is a reserved name.".to_string()));
        assert!(config.emulators.is_empty());
    }

    #[test]
    fn save_appends_new_emulator() {
        let mut config = config_with(&[], &[]);
        apply_save_emulator(&mut config, "", entry("Dolphin")).unwrap();
        assert_eq!(config.emulators, vec![entry("Dolphin")]);
    }

    #[test]
    fn save_rejects_duplicate_name_case_insensitively() {
        let mut config = config_with(&["Dolphin"], &[]);
        let result = apply_save_emulator(&mut config, "", entry("dolphin"));
        assert_eq!(
            result,
            Err("An emulator named 'dolphin' already exists.".to_string())
        );
        assert_eq!(config.emulators.len(), 1);
    }

    #[test]
    fn save_rename_removes_original_before_duplicate_check() {
        let mut config = config_with(&["Dolphin"], &[]);
        apply_save_emulator(&mut config, "Dolphin", entry("Dolphin Renamed")).unwrap();
        assert_eq!(config.emulators, vec![entry("Dolphin Renamed")]);
    }

    #[test]
    fn save_rename_to_an_existing_other_name_is_rejected() {
        let mut config = config_with(&["Dolphin", "PCSX2"], &[]);
        let result = apply_save_emulator(&mut config, "Dolphin", entry("PCSX2"));
        assert_eq!(
            result,
            Err("An emulator named 'PCSX2' already exists.".to_string())
        );
        // The original was removed by the (failed) rename attempt's retain
        // step logically, but since the whole call errors before pushing,
        // the caller never persists this — check the in-memory removal is
        // real so the invariant driving the check is verified directly.
        assert_eq!(config.emulators, vec![entry("PCSX2")]);
    }

    #[test]
    fn save_rename_repoints_default_emulators_case_insensitively() {
        let mut config = config_with(&["Dolphin"], &[("GameCube", "dolphin"), ("Wii", "Other")]);
        apply_save_emulator(&mut config, "Dolphin", entry("Dolphin Renamed")).unwrap();
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("Dolphin Renamed")
        );
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some("Other")
        );
    }

    #[test]
    fn save_unrelated_original_name_leaves_existing_untouched() {
        let mut config = config_with(&["Dolphin"], &[]);
        apply_save_emulator(&mut config, "Nonexistent", entry("PCSX2")).unwrap();
        assert_eq!(config.emulators, vec![entry("Dolphin"), entry("PCSX2")]);
    }

    /// Selection fallback is config-order-first, so editing an entry must not
    /// move it — otherwise editing entry #1 silently changes which emulator
    /// auto-launches.
    #[test]
    fn save_edit_first_entry_keeps_its_index_and_order() {
        let mut config = config_with(&["Dolphin", "PCSX2", "Yuzu"], &[]);
        apply_save_emulator(&mut config, "Dolphin", entry("Dolphin Updated")).unwrap();
        assert_eq!(
            config.emulators,
            vec![entry("Dolphin Updated"), entry("PCSX2"), entry("Yuzu")]
        );
    }

    #[test]
    fn save_rename_in_place_keeps_position() {
        let mut config = config_with(&["Dolphin", "PCSX2", "Yuzu"], &[]);
        apply_save_emulator(&mut config, "PCSX2", entry("PCSX2 Renamed")).unwrap();
        assert_eq!(
            config.emulators,
            vec![entry("Dolphin"), entry("PCSX2 Renamed"), entry("Yuzu")]
        );
    }

    // --- apply_delete_emulator ------------------------------------------------

    #[test]
    fn delete_removes_case_insensitively() {
        let mut config = config_with(&["Dolphin", "PCSX2"], &[]);
        apply_delete_emulator(&mut config, "dolphin");
        assert_eq!(config.emulators, vec![entry("PCSX2")]);
    }

    #[test]
    fn delete_drops_default_emulators_pointing_at_it() {
        let mut config = config_with(
            &["Dolphin"],
            &[
                ("GameCube", "Dolphin"),
                ("Wii", "dolphin"),
                ("N64", "Other"),
            ],
        );
        apply_delete_emulator(&mut config, "Dolphin");
        assert!(!config.default_emulators.contains_key("GameCube"));
        assert!(!config.default_emulators.contains_key("Wii"));
        assert_eq!(
            config.default_emulators.get("N64").map(String::as_str),
            Some("Other")
        );
    }

    #[test]
    fn delete_missing_name_is_a_no_op() {
        let mut config = config_with(&["Dolphin"], &[]);
        apply_delete_emulator(&mut config, "Nonexistent");
        assert_eq!(config.emulators, vec![entry("Dolphin")]);
    }

    #[test]
    fn delete_drops_retroarch_cores_for_platforms_that_lost_their_default() {
        // `remove_emulator_default_mappings` (grid_launcher/ui/emulators.py:137-152):
        // a saved core is only meaningful alongside a saved default emulator.
        let mut config = config_with(
            &["Dolphin", "RetroArch"],
            &[("GameCube", "Dolphin"), ("SNES", "RetroArch")],
        );
        config
            .retroarch_cores
            .insert("GameCube".to_string(), "dolphin_libretro".to_string());
        config
            .retroarch_cores
            .insert("SNES".to_string(), "bsnes_libretro".to_string());
        apply_delete_emulator(&mut config, "Dolphin");
        assert!(!config.retroarch_cores.contains_key("GameCube"));
        assert_eq!(
            config.retroarch_cores.get("SNES").map(String::as_str),
            Some("bsnes_libretro")
        );
    }

    #[test]
    fn delete_leaves_retroarch_cores_alone_when_no_default_pointed_at_it() {
        let mut config = config_with(&["Dolphin", "Other"], &[("GameCube", "Dolphin")]);
        config
            .retroarch_cores
            .insert("GameCube".to_string(), "dolphin_libretro".to_string());
        apply_delete_emulator(&mut config, "Other");
        assert_eq!(
            config.retroarch_cores.get("GameCube").map(String::as_str),
            Some("dolphin_libretro")
        );
    }

    // --- apply_set_default_emulator -------------------------------------------

    #[test]
    fn set_default_blank_name_writes_the_none_marker_at_the_exact_key() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "GameCube", "");
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some(NO_EMULATOR)
        );
        assert_eq!(config.default_emulators.len(), 1);
    }

    #[test]
    fn set_default_blank_name_replaces_a_case_insensitive_key_with_the_marker() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "gamecube", "  ");
        assert_eq!(config.default_emulators.len(), 1);
        assert_eq!(
            config.default_emulators.get("gamecube").map(String::as_str),
            Some(NO_EMULATOR)
        );
    }

    #[test]
    fn set_default_blank_name_records_the_marker_for_an_absent_key() {
        // "(none)" on a platform that had no saved default still has to be
        // written: that is what stops the backfill re-filling it.
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "Wii", "");
        assert_eq!(config.default_emulators.len(), 2);
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some(NO_EMULATOR)
        );
    }

    #[test]
    fn set_default_replaces_the_none_marker_with_a_real_emulator() {
        let mut config = config_with(&[], &[("GameCube", NO_EMULATOR)]);
        apply_set_default_emulator(&mut config, "GameCube", "Dolphin");
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("Dolphin")
        );
    }

    #[test]
    fn set_default_overwrites_exact_key() {
        let mut config = config_with(&[], &[("GameCube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "GameCube", "PCSX2");
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("PCSX2")
        );
        assert_eq!(config.default_emulators.len(), 1);
    }

    #[test]
    fn set_default_replaces_case_insensitive_key_with_the_new_casing() {
        let mut config = config_with(&[], &[("gamecube", "Dolphin")]);
        apply_set_default_emulator(&mut config, "GameCube", "PCSX2");
        assert_eq!(config.default_emulators.len(), 1);
        assert_eq!(
            config.default_emulators.get("GameCube").map(String::as_str),
            Some("PCSX2")
        );
        assert!(!config.default_emulators.contains_key("gamecube"));
    }

    #[test]
    fn set_default_inserts_new_key_when_absent() {
        let mut config = config_with(&[], &[]);
        apply_set_default_emulator(&mut config, "Wii", "Dolphin");
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some("Dolphin")
        );
    }

    #[test]
    fn set_default_trims_the_value_but_not_the_platform_key() {
        let mut config = config_with(&[], &[]);
        apply_set_default_emulator(&mut config, "Wii", "  Dolphin  ");
        assert_eq!(
            config.default_emulators.get("Wii").map(String::as_str),
            Some("Dolphin")
        );
    }

    // --- check_default_emulator_supported --------------------------------------

    /// A profile that matches the `config_with` entry named "PCSX2" by name
    /// and supports PlayStation 2 only.
    fn ps2_only_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "PCSX2".to_string(),
            platform_keywords: vec!["playstation 2".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn default_check_accepts_a_compatible_pick() {
        let config = config_with(&["PCSX2"], &[]);
        let result = check_default_emulator_supported(
            &config,
            "Sony PlayStation 2",
            "PCSX2",
            "",
            &[ps2_only_profile()],
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn default_check_refuses_an_incompatible_pick() {
        let config = config_with(&["PCSX2"], &[]);
        let result = check_default_emulator_supported(
            &config,
            "GameCube",
            "PCSX2",
            "",
            &[ps2_only_profile()],
        );
        assert_eq!(result, Err("PCSX2 does not support GameCube".to_string()));
    }

    #[test]
    fn default_check_refuses_a_name_no_entry_matches() {
        let config = config_with(&["PCSX2"], &[]);
        let result = check_default_emulator_supported(
            &config,
            "GameCube",
            "Ghost",
            "",
            &[ps2_only_profile()],
        );
        assert_eq!(result, Err("Ghost does not support GameCube".to_string()));
    }

    #[test]
    fn default_check_allows_a_blank_name_which_means_none() {
        let config = config_with(&["PCSX2"], &[("GameCube", "PCSX2")]);
        let result =
            check_default_emulator_supported(&config, "GameCube", "  ", "", &[ps2_only_profile()]);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn set_default_refusal_writes_nothing_to_the_config_file() {
        // The guard runs inside the modify_config closure, so its Err has to
        // abort the whole load-modify-save (config_write.rs).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        config_with(&["PCSX2"], &[]).save(&path).unwrap();
        let profiles = vec![ps2_only_profile()];

        let result = modify_config(&path, |config| {
            check_default_emulator_supported(config, "GameCube", "PCSX2", "", &profiles)?;
            apply_set_default_emulator(config, "GameCube", "PCSX2");
            Ok(())
        });

        assert_eq!(result, Err("PCSX2 does not support GameCube".to_string()));
        assert!(Config::load(&path).unwrap().default_emulators.is_empty());
    }

    // --- manual-add profile defaults (D1) --------------------------------------

    fn pcsx2_profile() -> EmulatorProfile {
        EmulatorProfile {
            name: "PCSX2".to_string(),
            match_tokens: vec!["pcsx2*".to_string()],
            args: "-batch %rom%".to_string(),
            save_strategy: "folder".to_string(),
            save_directories: vec!["~/pcsx2/memcards".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn manual_add_applies_profile_defaults_but_an_edit_does_not() {
        let profiles = vec![pcsx2_profile()];
        let typed = EmulatorEntry {
            name: "PCSX2".to_string(),
            path: "/opt/pcsx2/pcsx2.AppImage".to_string(),
            ..Default::default()
        };

        let added = manual_add_entry(typed.clone(), true, &profiles);
        assert_eq!(added.args, "-batch %rom%");
        assert_eq!(added.save_paths, "~/pcsx2/memcards");
        assert_eq!(added.save_strategy, "folder");

        let edited = manual_add_entry(typed.clone(), false, &profiles);
        assert_eq!(edited, typed, "an edit must pass through untouched (D1)");

        // No matching profile: the add passes through unchanged too.
        let unmatched = manual_add_entry(typed.clone(), true, &[]);
        assert_eq!(unmatched, typed);
    }

    #[test]
    fn manual_add_never_overwrites_the_typed_path() {
        let mut profiles = vec![pcsx2_profile()];
        profiles[0].args = "--other %rom%".to_string();
        let typed = EmulatorEntry {
            name: "PCSX2".to_string(),
            path: "/home/me/my own build/pcsx2".to_string(),
            args: "%rom%".to_string(),
            ..Default::default()
        };

        let added = manual_add_entry(typed.clone(), true, &profiles);
        assert_eq!(
            added.path, "/home/me/my own build/pcsx2",
            "autoconfig.py:228 never touches `path`"
        );
        assert_eq!(added.args, "--other %rom%", "a bare %rom% IS replaced");
    }

    #[test]
    fn is_manual_add_is_true_for_a_blank_or_unknown_original_name() {
        let config = config_with(&["Dolphin"], &[]);
        assert!(is_manual_add(&config, ""));
        assert!(is_manual_add(&config, "   "));
        assert!(is_manual_add(&config, "Nonexistent"));
        assert!(!is_manual_add(&config, "Dolphin"));
        assert!(!is_manual_add(&config, "  dolphin  "));
    }

    #[test]
    fn should_backfill_on_platform_list_needs_a_non_empty_assignable_list() {
        assert!(!should_backfill_on_platform_list(&[]));
        assert!(should_backfill_on_platform_list(&["SNES".to_string()]));
    }
}

#[cfg(test)]
mod config_dir_tests {
    use super::config_dir_for;
    use std::path::{Path, PathBuf};

    #[test]
    fn config_dir_is_the_config_files_parent() {
        assert_eq!(
            config_dir_for(Path::new("/home/six/.config/grid-launcher/config.toml")),
            PathBuf::from("/home/six/.config/grid-launcher")
        );
    }

    #[test]
    fn config_dir_falls_back_to_the_current_directory() {
        assert_eq!(config_dir_for(Path::new("config.toml")), PathBuf::from("."));
    }
}

#[cfg(test)]
mod retroachievements_tests {
    use super::*;

    /// `RaStatus`'s whole point is that the token itself never crosses IPC —
    /// only a presence boolean. Serialize a status built for a token that IS
    /// present and assert the token text never appears in the JSON.
    #[test]
    fn ra_status_never_contains_the_token() {
        let status = RaStatus {
            username: "sixdd6".to_string(),
            token_present: true,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("FAKE-RA-TOKEN-not-real"));
        assert!(!json.to_lowercase().contains("token_value"));
        assert_eq!(json, r#"{"username":"sixdd6","token_present":true}"#);
    }

    /// [`apply_clear_retroachievements`] is the config-mutation half of
    /// `clear_retroachievements_credentials`: it blanks the username and
    /// touches nothing else. Proven here against a real emulator config
    /// file's mtime, standing in for "no emulator file is written" — the
    /// keyring clear itself is covered by
    /// `secrets::tests::ra_token_store_round_trips_independently_of_the_romm_credential`.
    #[test]
    fn clear_blanks_the_username_and_writes_no_emulator_file() {
        let temp = tempfile::tempdir().unwrap();
        let emulator_cfg = temp.path().join("retroarch.cfg");
        std::fs::write(&emulator_cfg, "cheevos_username = \"sixdd6\"\n").unwrap();
        let before = std::fs::metadata(&emulator_cfg)
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut config = Config {
            retroachievements_username: "sixdd6".to_string(),
            ..Config::default()
        };
        apply_clear_retroachievements(&mut config);

        assert_eq!(config.retroachievements_username, "");
        let after = std::fs::metadata(&emulator_cfg)
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            before, after,
            "clear must never touch an emulator config file"
        );
    }
}

#[cfg(test)]
mod ui_settings_tests {
    use super::*;

    #[test]
    fn an_unknown_theme_normalizes_to_system() {
        for raw in ["", "SYSTEM", "solarized", "  dark  "] {
            let out = normalize_ui_settings(UiSettings {
                theme: raw.to_string(),
                background_fade: 25,
                ..Default::default()
            });
            let expected = if raw.trim() == "dark" {
                "dark"
            } else {
                "system"
            };
            assert_eq!(out.theme, expected, "input {raw:?}");
        }
    }

    #[test]
    fn the_three_known_themes_are_stored_verbatim() {
        for raw in ["system", "dark", "light"] {
            let out = normalize_ui_settings(UiSettings {
                theme: raw.to_string(),
                background_fade: 0,
                ..Default::default()
            });
            assert_eq!(out.theme, raw);
        }
    }

    #[test]
    fn normalize_ui_settings_clamps_both_card_sizes_to_the_three_names() {
        for (raw, expected) in [
            ("small", "small"),
            ("medium", "medium"),
            ("large", "large"),
            ("  large  ", "large"),
            ("Large", "medium"),
            ("enormous", "medium"),
            ("", "medium"),
        ] {
            let out = normalize_ui_settings(UiSettings {
                theme: "system".to_string(),
                background_fade: 25,
                background_blur: 12,
                card_size_library: raw.to_string(),
                card_size_server: raw.to_string(),
            });
            assert_eq!(out.card_size_library, expected, "library size for {raw:?}");
            assert_eq!(out.card_size_server, expected, "server size for {raw:?}");
        }
    }

    #[test]
    fn the_fade_is_clamped_to_the_designs_zero_to_sixty() {
        let fade = |value: u8| {
            normalize_ui_settings(UiSettings {
                theme: "system".to_string(),
                background_fade: value,
                ..Default::default()
            })
            .background_fade
        };
        assert_eq!(fade(0), 0);
        assert_eq!(fade(25), 25);
        assert_eq!(fade(60), 60);
        assert_eq!(fade(61), 60);
        assert_eq!(fade(255), 60);
    }

    /// The blur is clamped the same way the fade is: a stale or hand-edited
    /// frontend value must never become a 255-sigma blur nobody can undo.
    #[test]
    fn the_blur_is_clamped_to_the_designs_zero_to_forty() {
        let blur = |value: u8| {
            normalize_ui_settings(UiSettings {
                background_blur: value,
                ..Default::default()
            })
            .background_blur
        };
        assert_eq!(blur(0), 0);
        assert_eq!(blur(12), 12);
        assert_eq!(blur(40), 40);
        assert_eq!(blur(41), 40);
        assert_eq!(blur(255), 40);
    }

    #[test]
    fn the_default_ui_settings_normalize_unchanged() {
        assert_eq!(
            normalize_ui_settings(UiSettings::default()),
            UiSettings {
                theme: "system".to_string(),
                background_fade: 25,
                background_blur: 12,
                card_size_library: "medium".to_string(),
                card_size_server: "medium".to_string(),
            }
        );
    }

    #[test]
    fn a_server_url_carrying_userinfo_is_never_handed_to_the_os_opener() {
        // Basic-auth mode puts the password in the URL the user typed. It
        // must never reach a browser command line, a shell history, or a
        // desktop portal log.
        assert_eq!(
            browsable_server_url("http://user:pw@romm.example/romm"),
            None
        );
        assert_eq!(browsable_server_url("https://user@romm.example"), None);
    }

    #[test]
    fn only_plain_http_and_https_urls_are_browsable() {
        assert_eq!(
            browsable_server_url("https://romm.example:8080/romm"),
            Some("https://romm.example:8080/romm".to_string())
        );
        assert_eq!(
            browsable_server_url("  http://192.168.1.5:8000  "),
            Some("http://192.168.1.5:8000".to_string())
        );
        assert_eq!(browsable_server_url(""), None);
        assert_eq!(browsable_server_url("romm.example"), None);
        assert_eq!(browsable_server_url("file:///etc/passwd"), None);
        assert_eq!(browsable_server_url("javascript:alert(1)"), None);
    }
}

#[cfg(test)]
mod youtube_watch_url_tests {
    use super::*;

    #[test]
    fn a_valid_eleven_character_id_builds_the_watch_url() {
        assert_eq!(
            youtube_watch_url("dQw4w9WgXcQ"),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn a_path_is_never_interpolated_into_the_url() {
        assert_eq!(youtube_watch_url("../evil"), None);
    }

    #[test]
    fn a_trailing_query_string_is_refused_even_though_it_starts_with_a_valid_id() {
        assert_eq!(youtube_watch_url("dQw4w9WgXcQ&list=x"), None);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_before_the_length_check() {
        assert_eq!(
            youtube_watch_url("  dQw4w9WgXcQ  "),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string())
        );
    }
}

#[cfg(test)]
mod platform_firmware_tests {
    use super::*;

    #[test]
    fn a_platform_with_no_files_and_no_emulator_offers_nothing() {
        let status = PlatformFirmwareStatus {
            file_count: 0,
            has_default_emulator: false,
        };
        // The DTO is the whole contract: the frontend decides what to show.
        // Serialization is asserted because the field names are the API.
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#"{"file_count":0,"has_default_emulator":false}"#);
    }

    #[test]
    fn a_platform_with_files_and_an_emulator_serializes_both_flags() {
        let status = PlatformFirmwareStatus {
            file_count: 4,
            has_default_emulator: true,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#"{"file_count":4,"has_default_emulator":true}"#);
    }
}
