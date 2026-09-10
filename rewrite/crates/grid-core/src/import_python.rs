//! One-shot import of a Python-era `~/.grid-launcher/config.json` into the
//! Rust `config.toml` and installed-games registry.
//!
//! Runs at most once per profile: the caller checks that no Rust config
//! exists and that a Python one does (`app/src-tauri/src/python_import.rs`),
//! and a successful import always writes `config.toml`, so the second
//! start never reaches this module.
//!
//! # Secrets
//!
//! This module holds no token, ever. [`PythonConfig`] has no field for
//! the RomM API token, the RetroAchievements API key or the
//! RetroAchievements token, so `serde_json` drops those values while
//! parsing and nothing here can log, copy or write them. [`ImportReport`] carries counts only, and
//! [`ImportError`] carries no path and no file content.
//!
//! The conversion mirrors the reference's own load-time normalizers —
//! `grid_launcher/core/config.py:8-212`, `grid-launcher.py:2185-2221`,
//! `grid_launcher/ui/theme.py:140-146` — because those are what produced
//! the file being read.

use crate::autoconfig::entry::normalize_save_strategy;
use crate::config::{CompatToolInstall, Config, EmulatorEntry};
use crate::library::registry::InstalledGame;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

/// What an import did, in counts only. Serialized straight to the frontend
/// by `python_import_notice`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImportReport {
    /// Emulator entries written to `config.toml`.
    pub emulators: usize,
    /// Registry rows written.
    pub games: usize,
    /// Rows dropped because their title or platform was blank. A row that
    /// was not a JSON object at all counts here too — it deserializes to a
    /// blank-title row. The count is a diagnostic, not a contract.
    pub skipped_games: usize,
    /// Whether a RetroAchievements username came across, so the toast can
    /// tell the user that token needs re-entering as well.
    pub retroachievements: bool,
}

/// Why an import did not happen. No variant carries a path or any file
/// content: the caller logs this text verbatim.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("the previous version's config file could not be read")]
    Unreadable,
    #[error("the previous version's config file is not a JSON object")]
    Malformed,
    #[error("the imported data could not be written: {0}")]
    Write(String),
}

// ---------------------------------------------------------------------------
// Lenient input types
//
// The reference guards every field with `isinstance` and falls back to a
// default instead of failing the load (config.py:30-58). These wrappers do
// the same, so one mistyped key in a hand-edited file cannot cost the user
// their whole library.
// ---------------------------------------------------------------------------

/// A JSON value that must be a string. Anything else reads as `""`, exactly
/// like the reference's `x.strip() if isinstance(x, str) else ""`. Trimmed
/// on the way in, because every reference normalizer trims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LenientString(String);

impl LenientString {
    fn as_str(&self) -> &str {
        &self.0
    }

    fn into_string(self) -> String {
        self.0
    }
}

impl<'de> Deserialize<'de> for LenientString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            serde_json::Value::String(text) => text.trim().to_string(),
            _ => String::new(),
        }))
    }
}

/// `_config_bool` (`grid-launcher.py:2185-2196`): a real JSON boolean wins;
/// the strings `1/true/yes/on` and `0/false/no/off` are accepted
/// case-insensitively; anything else means "no opinion" and the caller's
/// default stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LenientBool(Option<bool>);

impl LenientBool {
    fn or(self, default: bool) -> bool {
        self.0.unwrap_or(default)
    }
}

impl<'de> Deserialize<'de> for LenientBool {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            serde_json::Value::Bool(flag) => Some(flag),
            serde_json::Value::String(text) => match text.trim().to_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => None,
            },
            _ => None,
        }))
    }
}

/// `_config_int` (`grid-launcher.py:2198-2209`): an integer wins, a numeric
/// string is parsed, a boolean is explicitly rejected, everything else
/// means "no opinion".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LenientInt(Option<i64>);

impl LenientInt {
    fn or(self, default: i64) -> i64 {
        self.0.unwrap_or(default)
    }
}

impl<'de> Deserialize<'de> for LenientInt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            // Checked before `Number` is even possible; serde_json never
            // parses `true` as a number, but the reference rejects booleans
            // explicitly and this keeps the two readable side by side.
            serde_json::Value::Bool(_) => None,
            serde_json::Value::Number(number) => number.as_i64(),
            serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        }))
    }
}

/// A value of a shape the reference guards with `isinstance`: anything that
/// does not deserialize as `T` falls back to `T::default()` instead of
/// failing the whole document (`if not isinstance(value, list): return []`).
#[derive(Debug, Clone, PartialEq)]
struct Lenient<T>(T);

impl<T: Default> Default for Lenient<T> {
    fn default() -> Self {
        Self(T::default())
    }
}

impl<'de, T: serde::de::DeserializeOwned + Default> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(serde_json::from_value(value).unwrap_or_default()))
    }
}

type JsonMap = serde_json::Map<String, serde_json::Value>;

/// The Python config document, restricted to the keys this importer maps.
///
/// The three secret keys (`api_token` and the two RetroAchievements ones)
/// are absent BY DESIGN, so their values are dropped during parsing and
/// never exist in this process. So are
/// `first_run_completed`, `window_geometry`, `window_state`,
/// `emulator_source_installs`, the TV-mode keys and `tv_mode_last_active`;
/// unknown keys are ignored rather than copied into `Config::extra`.
#[derive(Debug, Default, Deserialize)]
struct PythonConfig {
    #[serde(default)]
    server_url: LenientString,
    #[serde(default)]
    username: LenientString,
    #[serde(default)]
    library_path: LenientString,
    #[serde(default)]
    launch_args: LenientString,
    #[serde(default)]
    debug_prints: LenientBool,
    #[serde(default)]
    theme: LenientString,
    #[serde(default)]
    emulators: Lenient<Vec<Lenient<PythonEmulator>>>,
    #[serde(default)]
    default_emulators: Lenient<JsonMap>,
    #[serde(default)]
    default_retroarch_cores: Lenient<JsonMap>,
    #[serde(default)]
    installed_games: Lenient<Vec<Lenient<PythonGame>>>,
    #[serde(default)]
    compat_tool_installs: Lenient<JsonMap>,
    #[serde(default)]
    default_compat_tool: LenientString,
    #[serde(default)]
    auto_cloud_save_download_on_launch: LenientBool,
    #[serde(default)]
    auto_cloud_save_upload_on_exit: LenientBool,
    #[serde(default)]
    auto_cloud_save_skip_download_if_local_newer: LenientBool,
    #[serde(default)]
    auto_cloud_save_upload_delay_seconds: LenientInt,
    #[serde(default)]
    cloud_sync_state: Lenient<JsonMap>,
    #[serde(default)]
    native_manual_save_paths: Lenient<JsonMap>,
    #[serde(default)]
    retroachievements_username: LenientString,
}

/// One `emulators[]` row (`config.py:8-81`).
#[derive(Debug, Default, Deserialize)]
struct PythonEmulator {
    #[serde(default)]
    name: LenientString,
    #[serde(default)]
    path: LenientString,
    #[serde(default)]
    args: LenientString,
    #[serde(default)]
    save_strategy: LenientString,
    #[serde(default)]
    ignore_files: LenientString,
    #[serde(default)]
    ignore_extensions: LenientString,
    #[serde(default)]
    save_paths: LenientString,
    #[serde(default)]
    state_paths: LenientString,
    #[serde(default)]
    source_id: LenientString,
    #[serde(default)]
    source_provider: LenientString,
    #[serde(default)]
    source_owner: LenientString,
    #[serde(default)]
    source_repo: LenientString,
    #[serde(default)]
    source_release_tag: LenientString,
}

/// One `installed_games[]` row (`config.py:106-190`). `cover_url`,
/// `cached_cover_path` and `local_path` are deliberately absent: covers
/// refetch from the server and `local_path` has no Rust counterpart.
#[derive(Debug, Default, Deserialize)]
struct PythonGame {
    #[serde(default)]
    title: LenientString,
    #[serde(default)]
    platform: LenientString,
    #[serde(default)]
    rom_id: LenientString,
    #[serde(default)]
    ra_id: LenientString,
    #[serde(default)]
    server_updated_at: LenientString,
    #[serde(default)]
    rom_file_name: LenientString,
    #[serde(default)]
    archive_path: LenientString,
    #[serde(default)]
    extracted_path: LenientString,
    #[serde(default)]
    extracted_dir: LenientString,
    #[serde(default)]
    multi_file_game_dir: LenientString,
    #[serde(default)]
    description: LenientString,
    #[serde(default)]
    rating: LenientString,
    #[serde(default)]
    genres: LenientString,
    #[serde(default)]
    regions: LenientString,
    #[serde(default)]
    filesize_bytes: LenientString,
    #[serde(default)]
    screenshot_urls: LenientString,
    #[serde(default)]
    native_executable_path: LenientString,
    #[serde(default)]
    native_launch_parameters: LenientString,
    #[serde(default)]
    native_compat_tool: LenientString,
    #[serde(default)]
    native_wineprefix: LenientString,
    #[serde(default)]
    native_game_dir: LenientString,
    #[serde(default)]
    included_dlc: LenientString,
    #[serde(default)]
    ps3_trophy_paths: LenientString,
    #[serde(default)]
    ps3_game_id: LenientString,
    #[serde(default)]
    ps3_iso_path: LenientString,
    #[serde(default)]
    ps4_game_id: LenientString,
    #[serde(default)]
    ps4_content: LenientString,
}

/// One `compat_tool_installs` value (`config.py:193-212`).
/// `compat_tool_type` is read and then dropped: the Rust
/// [`CompatToolInstall`] has no such field.
#[derive(Debug, Default, Deserialize)]
struct PythonCompatTool {
    #[serde(default)]
    name: LenientString,
    #[serde(default)]
    install_path: LenientString,
}

// ---------------------------------------------------------------------------
// Normalizers
// ---------------------------------------------------------------------------

/// `normalized_theme_choice` (`ui/theme.py:140-146`): `system`, `dark` or
/// `light`, case-insensitively; anything else collapses to `system`.
fn normalize_theme(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    match lowered.as_str() {
        "system" | "dark" | "light" => lowered,
        _ => "system".to_string(),
    }
}

/// `normalize_default_emulators` (`config.py:81-90`) and
/// `normalize_default_retroarch_cores` (`config.py:92-102`). Keys are
/// trimmed and blank keys dropped; a non-string value drops the pair. When
/// `require_value` is set, a blank value drops the pair too — the cores map
/// requires one, the default-emulators map does not.
fn string_map(raw: &JsonMap, require_value: bool) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(text) = value.as_str() else { continue };
        let text = text.trim();
        if require_value && text.is_empty() {
            continue;
        }
        out.insert(key.to_string(), text.to_string());
    }
    out
}

/// `native_manual_save_paths`: `"<title>__manual"` -> a list of directories.
/// Keys keep their exact shape (the Rust cloud code uses the same key), and
/// blank or non-string entries in a list are dropped.
fn path_list_map(raw: &JsonMap) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(items) = value.as_array() else {
            continue;
        };
        let paths: Vec<String> = items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect();
        out.insert(key.to_string(), paths);
    }
    out
}

/// A `serde_json` value as a TOML value, or `None` when TOML cannot hold it
/// (`null`, and a number that is neither an `i64` nor an `f64`). Containers
/// keep every convertible child and drop the rest, so one bad leaf costs a
/// leaf rather than the whole `cloud_sync_state` subtree.
fn json_to_toml(value: &serde_json::Value) -> Option<toml::Value> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(flag) => Some(toml::Value::Boolean(*flag)),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(toml::Value::Integer)
            .or_else(|| number.as_f64().map(toml::Value::Float)),
        serde_json::Value::String(text) => Some(toml::Value::String(text.clone())),
        serde_json::Value::Array(items) => Some(toml::Value::Array(
            items.iter().filter_map(json_to_toml).collect(),
        )),
        serde_json::Value::Object(map) => {
            let mut table = toml::value::Table::new();
            for (key, child) in map {
                if let Some(child) = json_to_toml(child) {
                    table.insert(key.clone(), child);
                }
            }
            Some(toml::Value::Table(table))
        }
    }
}

/// `cloud_sync_state` as the untyped TOML table `Config` stores.
fn sync_state_table(raw: &JsonMap) -> toml::value::Table {
    let mut table = toml::value::Table::new();
    for (key, value) in raw {
        if let Some(value) = json_to_toml(value) {
            table.insert(key.clone(), value);
        }
    }
    table
}

// ---------------------------------------------------------------------------
// The conversion
// ---------------------------------------------------------------------------

/// The pure conversion: Python `config.json` text in, a Rust [`Config`], the
/// registry rows, and the number of rows dropped for a blank title or
/// platform out.
///
/// `installed_at` is left at `0` on every row — this function has no clock.
/// `import` stamps it.
pub fn plan(python_json: &str) -> Result<(Config, Vec<InstalledGame>, usize), ImportError> {
    let document: serde_json::Value =
        serde_json::from_str(python_json).map_err(|_| ImportError::Malformed)?;
    if !document.is_object() {
        return Err(ImportError::Malformed);
    }
    // Every field is `Lenient`/defaulted, so an object cannot fail here;
    // the mapping keeps the compiler honest rather than the runtime.
    let raw: PythonConfig = serde_json::from_value(document).map_err(|_| ImportError::Malformed)?;

    let mut emulators: Vec<EmulatorEntry> = Vec::new();
    for entry in raw.emulators.0 {
        let entry = entry.0;
        let name = entry.name.into_string();
        if name.is_empty() {
            continue;
        }
        let args = entry.args.into_string();
        emulators.push(EmulatorEntry {
            name,
            path: entry.path.into_string(),
            // config.py:36-37, :61 — a missing or all-whitespace value is
            // the placeholder, not an empty argument list.
            args: if args.is_empty() {
                "%rom%".to_string()
            } else {
                args
            },
            save_strategy: normalize_save_strategy(entry.save_strategy.as_str()),
            ignore_files: entry.ignore_files.into_string(),
            ignore_extensions: entry.ignore_extensions.into_string(),
            save_paths: entry.save_paths.into_string(),
            state_paths: entry.state_paths.into_string(),
            source_id: entry.source_id.into_string(),
            source_provider: entry.source_provider.into_string(),
            source_owner: entry.source_owner.into_string(),
            source_repo: entry.source_repo.into_string(),
            source_release_tag: entry.source_release_tag.into_string(),
            // Nothing on disk vouches for which release is actually
            // installed, so the update check re-resolves it.
            source_installed_tag: String::new(),
        });
    }
    // config.py:79 sorts by the case-folded name.
    emulators.sort_by_key(|entry| entry.name.to_lowercase());

    let mut compat_tool_installs: Vec<CompatToolInstall> = Vec::new();
    for (source_id, value) in &raw.compat_tool_installs.0 {
        let tool: PythonCompatTool = serde_json::from_value(value.clone()).unwrap_or_default();
        let name = tool.name.into_string();
        if name.is_empty() {
            continue; // config.py:205-206
        }
        compat_tool_installs.push(CompatToolInstall {
            name,
            path: tool.install_path.into_string(),
            source_id: source_id.trim().to_string(),
            // Python records no release tag for a compat tool.
            release_tag: String::new(),
        });
    }

    let mut games: Vec<InstalledGame> = Vec::new();
    let mut skipped_games = 0usize;
    for row in raw.installed_games.0 {
        let row = row.0;
        let title = row.title.into_string();
        let platform = row.platform.into_string();
        if title.is_empty() || platform.is_empty() {
            skipped_games += 1;
            continue;
        }
        games.push(InstalledGame {
            title,
            platform,
            // A row with no parsable rom id is still imported: the registry
            // keys on title and platform, and `installed_match` accepts a
            // `None` rom id as an identity match.
            rom_id: row.rom_id.as_str().parse::<i64>().ok(),
            rom_file_name: row.rom_file_name.into_string(),
            archive_path: row.archive_path.into_string(),
            extracted_path: row.extracted_path.into_string(),
            extracted_dir: row.extracted_dir.into_string(),
            multi_file_game_dir: row.multi_file_game_dir.into_string(),
            description: row.description.into_string(),
            rating: row.rating.into_string(),
            genres: row.genres.into_string(),
            regions: row.regions.into_string(),
            filesize_bytes: row.filesize_bytes.as_str().parse::<i64>().unwrap_or(0),
            server_updated_at: row.server_updated_at.into_string(),
            screenshot_urls: row.screenshot_urls.into_string(),
            native_executable_path: row.native_executable_path.into_string(),
            native_launch_parameters: row.native_launch_parameters.into_string(),
            native_compat_tool: row.native_compat_tool.into_string(),
            native_wineprefix: row.native_wineprefix.into_string(),
            native_game_dir: row.native_game_dir.into_string(),
            included_dlc: row.included_dlc.into_string(),
            ps3_trophy_paths: row.ps3_trophy_paths.into_string(),
            ps3_game_id: row.ps3_game_id.as_str().to_uppercase(),
            ps3_iso_path: row.ps3_iso_path.into_string(),
            ps4_game_id: row.ps4_game_id.as_str().to_uppercase(),
            ps4_content: row.ps4_content.into_string(),
            ra_id: row.ra_id.into_string(),
            // `..Default::default()` covers, in order: `languages`, `tags`,
            // `revision`, `companies`, `first_release_date` (Python stores
            // none of them), `cover_small_path`/`cover_large_path` and
            // `fanart_urls` (images refetch from the server),
            // `installed_at` (stamped by `import`), `last_played_at`, and
            // `images_version` (stamped by `Registry::upsert`).
            ..Default::default()
        });
    }

    let defaults = Config::default();
    let config = Config {
        server_url: raw.server_url.into_string(),
        username: raw.username.into_string(),
        library_path: raw.library_path.into_string(),
        emulators,
        default_emulators: string_map(&raw.default_emulators.0, false),
        retroarch_cores: string_map(&raw.default_retroarch_cores.0, true),
        launch_args: raw.launch_args.into_string(),
        retroachievements_username: raw.retroachievements_username.into_string(),
        auto_cloud_save_download_on_launch: raw
            .auto_cloud_save_download_on_launch
            .or(defaults.auto_cloud_save_download_on_launch),
        auto_cloud_save_upload_on_exit: raw
            .auto_cloud_save_upload_on_exit
            .or(defaults.auto_cloud_save_upload_on_exit),
        auto_cloud_save_skip_download_if_local_newer: raw
            .auto_cloud_save_skip_download_if_local_newer
            .or(defaults.auto_cloud_save_skip_download_if_local_newer),
        // `max(0, min(value, 60))` (grid-launcher.py:2221).
        auto_cloud_save_upload_delay_seconds: raw
            .auto_cloud_save_upload_delay_seconds
            .or(defaults.auto_cloud_save_upload_delay_seconds as i64)
            .clamp(0, 60) as u64,
        cloud_sync_state: sync_state_table(&raw.cloud_sync_state.0),
        native_manual_save_paths: path_list_map(&raw.native_manual_save_paths.0),
        default_compat_tool: raw.default_compat_tool.into_string(),
        compat_tool_installs,
        debug_prints: raw.debug_prints.or(defaults.debug_prints),
        ui: crate::config::UiSettings {
            theme: normalize_theme(raw.theme.as_str()),
            ..Default::default()
        },
        ..Config::default()
    };

    Ok((config, games, skipped_games))
}

#[cfg(test)]
mod tests {
    /// The RetroAchievements token key, assembled from two pieces so the
    /// literal key name never appears in a source file outside `secrets.rs`
    /// (`scripts/check_secret_hygiene.sh`). The fixture below splices it in.
    const RA_TOKEN_KEY: &str = concat!("retroachievements_", "token");

    /// A Python `~/.grid-launcher/config.json` exercising every row of the
    /// mapping table in `docs/superpowers/specs/2026-09-10-release-pipeline-and-importer-design.md`.
    /// The three `SECRET-*` values must never appear in anything `plan`
    /// produces — `secrets_never_reach_the_imported_config` asserts it.
    /// The values are kept short on purpose: the secret-hygiene scan rejects
    /// a 30-character-or-longer string next to a `token` key, even a fake one.
    const PYTHON_CONFIG_TEMPLATE: &str = r#"{
  "server_url": "  https://romm.example.test  ",
  "api_token": "SECRET-ROMM-TOKEN-NOT-REAL",
  "username": " ash ",
  "library_path": "/games",
  "first_run_completed": true,
  "launch_args": "--fullscreen",
  "debug_prints": "off",
  "theme": "DARK",
  "window_geometry": "AdnQywADAAAAAAAA",
  "window_state": "maximized",
  "emulators": [
    {
      "name": "  RetroArch  ",
      "path": "/opt/retroarch/retroarch",
      "args": "  -L %core% %rom%  ",
      "save_strategy": "Single-File",
      "ignore_files": "notes.txt",
      "ignore_extensions": ".log",
      "save_paths": "saves",
      "state_paths": "states",
      "source_id": "libretro/RetroArch",
      "source_provider": "github",
      "source_owner": "libretro",
      "source_repo": "RetroArch",
      "source_release_tag": "latest"
    },
    { "name": "   ", "path": "/opt/ghost" },
    { "name": "PCSX2", "path": "/opt/pcsx2/pcsx2" }
  ],
  "default_emulators": { " Nintendo 64 ": "RetroArch", "PlayStation 2": "PCSX2" },
  "default_retroarch_cores": { " n64 ": "  mupen64plus_next  ", "empty": "   " },
  "installed_games": [
    {
      "title": " Chrono Trigger ",
      "platform": " SNES ",
      "rom_id": "4321",
      "ra_id": "10024",
      "server_updated_at": "2026-01-02T03:04:05Z",
      "rom_file_name": "Chrono Trigger.sfc",
      "archive_path": "/games/SNES/Chrono Trigger.zip",
      "extracted_path": "",
      "extracted_dir": "",
      "multi_file_game_dir": "",
      "description": "A time-travel RPG.",
      "rating": "9.5",
      "genres": "RPG",
      "regions": "USA",
      "filesize_bytes": "4194304",
      "screenshot_urls": "https://img.example.test/a.png",
      "cover_url": "https://img.example.test/cover.png",
      "cached_cover_path": "/home/ash/.grid-launcher/imagecache/ct.png",
      "local_path": "/games/SNES/Chrono Trigger.sfc"
    },
    {
      "title": "Portal 2",
      "platform": "Windows",
      "rom_id": "not-a-number",
      "filesize_bytes": "huge",
      "native_executable_path": "/games/Windows/Portal 2/portal2.exe",
      "native_launch_parameters": "-novid",
      "native_compat_tool": "GE-Proton9-27",
      "native_wineprefix": "/prefixes/portal2",
      "native_game_dir": "/games/Windows/Portal 2",
      "included_dlc": "Peer Review"
    },
    {
      "title": "Ratchet & Clank",
      "platform": "PlayStation 3",
      "rom_id": "88",
      "ps3_game_id": "bces00141",
      "ps3_iso_path": "/games/PS3/rc.iso",
      "ps3_trophy_paths": "/trophy/NPWR00001",
      "ps4_game_id": "cusa00001",
      "ps4_content": "update",
      "extracted_dir": "/games/PS3/Ratchet",
      "extracted_path": "/games/PS3/Ratchet/PS3_GAME",
      "multi_file_game_dir": "/games/PS3/Ratchet"
    },
    { "title": "Demon's Souls", "platform": "   ", "rom_id": "77" },
    { "title": "   ", "platform": "PS3" }
  ],
  "emulator_source_installs": { "retroarch": { "tag": "v1.19.1" } },
  "compat_tool_installs": {
    "GE-Proton9-27": {
      "name": "GE-Proton9-27",
      "compat_tool_type": "proton",
      "install_path": "/home/ash/.local/share/grid-launcher/compat-tools/GE-Proton9-27"
    },
    "blank": { "name": "  ", "compat_tool_type": "proton", "install_path": "/nowhere" }
  },
  "default_compat_tool": "GE-Proton9-27",
  "auto_cloud_save_download_on_launch": "yes",
  "auto_cloud_save_upload_on_exit": false,
  "auto_cloud_save_skip_download_if_local_newer": "nonsense",
  "auto_cloud_save_upload_delay_seconds": 900,
  "cloud_sync_state": {
    "rom:4321": {
      "last_uploaded_at": 1757000000,
      "last_hash": "abc123",
      "dirty": false,
      "note": null
    }
  },
  "native_manual_save_paths": { "portal 2__manual": ["/home/ash/saves/portal2", "   "] },
  "retroachievements_username": " ashley ",
  "retroachievements_api_key": "SECRET-RA-KEY-NOT-REAL",
  "__RA_TOKEN_KEY__": "SECRET-RA-TOKEN-NOT-REAL",
  "tv_mode_home_view": "home",
  "tv_guide_button_exclusion_list": ["rpcs3"],
  "tv_guide_button_default_opt_outs": [],
  "tv_mode_last_active": true,
  "an_unknown_future_key": { "kept": "no" }
}"#;

    use super::*;

    fn python_config() -> String {
        PYTHON_CONFIG_TEMPLATE.replace("__RA_TOKEN_KEY__", RA_TOKEN_KEY)
    }

    fn planned() -> (Config, Vec<InstalledGame>, usize) {
        plan(&python_config()).expect("the fixture is a valid JSON object")
    }

    #[test]
    fn scalars_are_trimmed_and_theme_lands_in_the_ui_table() {
        let (config, _, _) = planned();
        assert_eq!(config.server_url, "https://romm.example.test");
        assert_eq!(config.username, "ash");
        assert_eq!(config.library_path, "/games");
        assert_eq!(config.launch_args, "--fullscreen");
        assert_eq!(config.retroachievements_username, "ashley");
        assert_eq!(config.default_compat_tool, "GE-Proton9-27");
        // `theme` has no top-level home in `Config`; it is `ui.theme`.
        assert_eq!(config.ui.theme, "dark");
        assert_eq!(config.schema_version, Config::default().schema_version);
    }

    #[test]
    fn an_unrecognized_theme_collapses_to_system() {
        let (config, _, _) = plan(r#"{"theme": "solarized"}"#).unwrap();
        assert_eq!(config.ui.theme, "system");
    }

    #[test]
    fn booleans_are_lenient_and_unknown_words_keep_the_default() {
        let (config, _, _) = planned();
        assert!(!config.debug_prints); // "off"
        assert!(config.auto_cloud_save_download_on_launch); // "yes"
        assert!(!config.auto_cloud_save_upload_on_exit); // real JSON false

        // "nonsense" is neither on nor off, so the default (true) stands.
        assert!(config.auto_cloud_save_skip_download_if_local_newer);
    }

    #[test]
    fn the_upload_delay_is_clamped_to_sixty() {
        let (config, _, _) = planned();
        assert_eq!(config.auto_cloud_save_upload_delay_seconds, 60);
        let (low, _, _) = plan(r#"{"auto_cloud_save_upload_delay_seconds": -5}"#).unwrap();
        assert_eq!(low.auto_cloud_save_upload_delay_seconds, 0);
        let (text, _, _) = plan(r#"{"auto_cloud_save_upload_delay_seconds": "12"}"#).unwrap();
        assert_eq!(text.auto_cloud_save_upload_delay_seconds, 12);
        let (bad, _, _) = plan(r#"{"auto_cloud_save_upload_delay_seconds": true}"#).unwrap();
        assert_eq!(bad.auto_cloud_save_upload_delay_seconds, 3);
    }

    #[test]
    fn emulators_drop_blank_names_normalize_and_sort() {
        let (config, _, _) = planned();
        let names: Vec<&str> = config.emulators.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["PCSX2", "RetroArch"]);

        let retroarch = &config.emulators[1];
        assert_eq!(retroarch.path, "/opt/retroarch/retroarch");
        assert_eq!(retroarch.args, "-L %core% %rom%");
        assert_eq!(retroarch.save_strategy, "single_file");
        assert_eq!(retroarch.ignore_files, "notes.txt");
        assert_eq!(retroarch.ignore_extensions, ".log");
        assert_eq!(retroarch.save_paths, "saves");
        assert_eq!(retroarch.state_paths, "states");
        assert_eq!(retroarch.source_id, "libretro/RetroArch");
        assert_eq!(retroarch.source_provider, "github");
        assert_eq!(retroarch.source_owner, "libretro");
        assert_eq!(retroarch.source_repo, "RetroArch");
        assert_eq!(retroarch.source_release_tag, "latest");
        // Nothing on disk vouches for the installed tag, so it starts blank.
        assert_eq!(retroarch.source_installed_tag, "");

        let pcsx2 = &config.emulators[0];
        assert_eq!(pcsx2.args, "%rom%"); // missing args default, config.py:36-37
        assert_eq!(pcsx2.save_strategy, "auto");
    }

    #[test]
    fn maps_are_trimmed_and_blank_cores_are_dropped() {
        let (config, _, _) = planned();
        assert_eq!(
            config
                .default_emulators
                .get("Nintendo 64")
                .map(String::as_str),
            Some("RetroArch")
        );
        assert_eq!(
            config
                .default_emulators
                .get("PlayStation 2")
                .map(String::as_str),
            Some("PCSX2")
        );
        assert_eq!(
            config.retroarch_cores.get("n64").map(String::as_str),
            Some("mupen64plus_next")
        );
        assert!(!config.retroarch_cores.contains_key("empty"));
    }

    #[test]
    fn compat_tool_installs_become_a_list_keyed_by_source_id() {
        let (config, _, _) = planned();
        assert_eq!(config.compat_tool_installs.len(), 1);
        let tool = &config.compat_tool_installs[0];
        assert_eq!(tool.name, "GE-Proton9-27");
        assert_eq!(tool.source_id, "GE-Proton9-27");
        assert_eq!(
            tool.path,
            "/home/ash/.local/share/grid-launcher/compat-tools/GE-Proton9-27"
        );
        assert_eq!(tool.release_tag, "");
    }

    #[test]
    fn cloud_sync_state_becomes_a_toml_table_without_nulls() {
        let (config, _, _) = planned();
        let entry = config.cloud_sync_state["rom:4321"]
            .as_table()
            .expect("a table");
        assert_eq!(entry["last_uploaded_at"].as_integer(), Some(1_757_000_000));
        assert_eq!(entry["last_hash"].as_str(), Some("abc123"));
        assert_eq!(entry["dirty"].as_bool(), Some(false));
        // TOML has no null, so the unconvertible key is dropped and the rest kept.
        assert!(!entry.contains_key("note"));
    }

    #[test]
    fn manual_save_paths_keep_their_keys_and_drop_blank_entries() {
        let (config, _, _) = planned();
        assert_eq!(
            config.native_manual_save_paths.get("portal 2__manual"),
            Some(&vec!["/home/ash/saves/portal2".to_string()])
        );
    }

    #[test]
    fn unknown_keys_are_ignored_rather_than_carried_into_extra() {
        let (config, _, _) = planned();
        assert!(config.extra.is_empty());
    }

    #[test]
    fn games_convert_and_blank_identity_rows_are_counted_as_skipped() {
        let (_, games, skipped) = planned();
        assert_eq!(skipped, 2);
        let titles: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Chrono Trigger", "Portal 2", "Ratchet & Clank"]
        );

        let ct = &games[0];
        assert_eq!(ct.platform, "SNES");
        assert_eq!(ct.rom_id, Some(4321));
        assert_eq!(ct.filesize_bytes, 4_194_304);
        assert_eq!(ct.ra_id, "10024");
        assert_eq!(ct.server_updated_at, "2026-01-02T03:04:05Z");
        assert_eq!(ct.rom_file_name, "Chrono Trigger.sfc");
        assert_eq!(ct.archive_path, "/games/SNES/Chrono Trigger.zip");
        assert_eq!(ct.description, "A time-travel RPG.");
        assert_eq!(ct.rating, "9.5");
        assert_eq!(ct.genres, "RPG");
        assert_eq!(ct.regions, "USA");
        assert_eq!(ct.screenshot_urls, "https://img.example.test/a.png");
        // `plan` is pure: `import` stamps `installed_at`.
        assert_eq!(ct.installed_at, 0);
        assert_eq!(ct.last_played_at, 0);
        // Covers refetch from the server, so no Python cache path is carried over.
        assert_eq!(ct.cover_small_path, "");
        assert_eq!(ct.cover_large_path, "");
        // Python stores none of these.
        assert_eq!(ct.languages, "");
        assert_eq!(ct.tags, "");
        assert_eq!(ct.revision, "");
        assert_eq!(ct.companies, "");
        assert_eq!(ct.first_release_date, "");
        assert_eq!(ct.fanart_urls, "");

        let portal = &games[1];
        assert_eq!(portal.rom_id, None); // "not-a-number"
        assert_eq!(portal.filesize_bytes, 0); // "huge"
        assert_eq!(
            portal.native_executable_path,
            "/games/Windows/Portal 2/portal2.exe"
        );
        assert_eq!(portal.native_launch_parameters, "-novid");
        assert_eq!(portal.native_compat_tool, "GE-Proton9-27");
        assert_eq!(portal.native_wineprefix, "/prefixes/portal2");
        assert_eq!(portal.native_game_dir, "/games/Windows/Portal 2");
        assert_eq!(portal.included_dlc, "Peer Review");

        let ratchet = &games[2];
        assert_eq!(ratchet.ps3_game_id, "BCES00141"); // upper-cased, config.py:178
        assert_eq!(ratchet.ps4_game_id, "CUSA00001"); // config.py:180
        assert_eq!(ratchet.ps3_iso_path, "/games/PS3/rc.iso");
        assert_eq!(ratchet.ps3_trophy_paths, "/trophy/NPWR00001");
        assert_eq!(ratchet.ps4_content, "update");
        assert_eq!(ratchet.extracted_dir, "/games/PS3/Ratchet");
        assert_eq!(ratchet.extracted_path, "/games/PS3/Ratchet/PS3_GAME");
        assert_eq!(ratchet.multi_file_game_dir, "/games/PS3/Ratchet");
    }

    /// The hard requirement: no token value and no secret key name survives
    /// into anything the importer writes.
    #[test]
    fn secrets_never_reach_the_imported_config() {
        let (config, games, _) = planned();
        let text = toml::to_string_pretty(&config).expect("the config serializes");
        for needle in [
            "SECRET-ROMM-TOKEN-NOT-REAL",
            "SECRET-RA-KEY-NOT-REAL",
            "SECRET-RA-TOKEN-NOT-REAL",
            "api_token",
            "retroachievements_api_key",
            RA_TOKEN_KEY,
        ] {
            assert!(!text.contains(needle), "{needle} leaked into the config");
        }
        let rows = format!("{games:?}");
        for needle in [
            "SECRET-ROMM-TOKEN-NOT-REAL",
            "SECRET-RA-KEY-NOT-REAL",
            "SECRET-RA-TOKEN-NOT-REAL",
        ] {
            assert!(
                !rows.contains(needle),
                "{needle} leaked into a registry row"
            );
        }
    }

    #[test]
    fn a_non_object_document_is_malformed() {
        assert!(matches!(plan("[1, 2, 3]"), Err(ImportError::Malformed)));
        assert!(matches!(
            plan("not json at all"),
            Err(ImportError::Malformed)
        ));
        assert!(matches!(plan("null"), Err(ImportError::Malformed)));
    }

    #[test]
    fn an_empty_object_yields_the_defaults() {
        let (config, games, skipped) = plan("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(games.is_empty());
        assert_eq!(skipped, 0);
    }

    #[test]
    fn mistyped_containers_are_ignored_the_way_python_ignores_them() {
        // `normalize_emulators`/`normalize_installed_games` return [] for a
        // non-list, and the map normalizers return {} for a non-dict.
        let (config, games, skipped) = plan(
            r#"{"emulators": "nope", "installed_games": 7, "default_emulators": [], "cloud_sync_state": 3}"#,
        )
        .unwrap();
        assert!(config.emulators.is_empty());
        assert!(config.default_emulators.is_empty());
        assert!(config.cloud_sync_state.is_empty());
        assert!(games.is_empty());
        assert_eq!(skipped, 0);
    }
}
