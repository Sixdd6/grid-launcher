use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmulatorEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub args: String,
    /// The catalog `"{owner}/{repo}"` this entry was installed from, when
    /// it was installed from the catalog rather than added by hand — read
    /// by `launch::catalog::mark_installed`. Blank for hand-added entries
    /// and for entries installed before this field existed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_id: String,
    /// The remaining source-catalog fields below are set together with
    /// `source_id` when an entry is installed from the catalog; all are
    /// blank for hand-added entries. Kept as plain strings, matching
    /// `source_id`, rather than the richer `SourceMap` — this is a record
    /// of what was installed, not something `launch::source` re-resolves.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_provider: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_owner: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_repo: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_release_tag: String,
    /// The five fields below are written by `autoconfig::entry`'s layer-1
    /// pass (autoconfig.py:524-554) and read by the cloud-save code. They
    /// follow the `source_*` serde pattern — defaulted on load, omitted on
    /// save when blank — so a config written before they existed round-trips
    /// byte-identically.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub save_strategy: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ignore_files: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ignore_extensions: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub save_paths: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state_paths: String,
}

/// One compat tool (e.g. GE-Proton, Proton-CachyOS) GRID installed and
/// manages, recorded so the app can offer it as a launch option and detect
/// it is already present without re-downloading. All fields default to `""`
/// so a config saved before this type existed round-trips unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CompatToolInstall {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub source_id: String,
    #[serde(default)]
    pub release_tag: String,
}

/// Desktop-shell appearance settings (design §4, §10 Appearance). Both
/// fields default so a config written before this table existed loads
/// unchanged, and `Config::save` emits `[ui]` after every scalar key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettings {
    /// `"system"` (follow `prefers-color-scheme`), `"dark"` or `"light"`.
    /// Stored as a plain string rather than an enum so an unknown value
    /// written by a newer build round-trips instead of failing the whole
    /// config load; the app layer normalizes on write and the frontend
    /// normalizes on read.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Background-art opacity in percent, 0–60 (design §3). Clamped on
    /// write by `normalize_ui_settings`; read sites clamp again.
    #[serde(default = "default_background_fade")]
    pub background_fade: u8,
    /// Background-art blur sigma, 0-40, applied at the variant's 960px scale
    /// (`images::background`). Baked into the variant's file name, so a
    /// change never serves a stale blur. Clamped on write by
    /// `normalize_ui_settings`.
    #[serde(default = "default_background_blur")]
    pub background_blur: u8,
    /// Library grid card size: `"small"`, `"medium"` or `"large"`
    /// (design §5, D-UI-9 "remembered per view"). Stored as a plain string
    /// for the same forward-compatibility reason as `theme`: an unknown
    /// value written by a newer build round-trips instead of failing the
    /// whole config load, and both the app layer and the frontend
    /// normalize it.
    #[serde(default = "default_card_size")]
    pub card_size_library: String,
    /// Server grid card size. Independent of `card_size_library`: the two
    /// grids are browsed differently and D-UI-9 remembers them per view.
    #[serde(default = "default_card_size")]
    pub card_size_server: String,
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_background_fade() -> u8 {
    50
}

fn default_background_blur() -> u8 {
    2
}

fn default_card_size() -> String {
    "medium".to_string()
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            background_fade: default_background_fade(),
            background_blur: default_background_blur(),
            card_size_library: default_card_size(),
            card_size_server: default_card_size(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// App configuration. Secrets are NEVER part of this struct — they live in
/// the OS keyring only (see secrets.rs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub library_path: String,
    #[serde(default)]
    pub emulators: Vec<EmulatorEntry>,
    #[serde(default)]
    pub default_emulators: BTreeMap<String, String>,
    #[serde(default)]
    pub retroarch_cores: BTreeMap<String, String>,
    #[serde(default)]
    pub launch_args: String,
    /// The RetroAchievements account name the `ensure_*` writers log the
    /// emulator in with. Plain, non-secret: the matching token lives in the
    /// OS keyring only (see `secrets.rs`) and never in this struct.
    #[serde(default)]
    pub retroachievements_username: String,
    /// Whether cloud save/state restore runs automatically before launch.
    /// `grid-launcher.py:2212`.
    #[serde(default = "default_true")]
    pub auto_cloud_save_download_on_launch: bool,
    /// Whether an auto upload is scheduled after a session ends.
    /// `grid-launcher.py:2215`.
    #[serde(default = "default_true")]
    pub auto_cloud_save_upload_on_exit: bool,
    /// Whether the pre-launch save restore skips downloading when the local
    /// copy is already newer than the server's. `grid-launcher.py:2218`.
    #[serde(default = "default_true")]
    pub auto_cloud_save_skip_download_if_local_newer: bool,
    /// Seconds to wait after a session ends before the auto upload runs;
    /// `0` means immediate. Read sites clamp this to `0..=60`
    /// (`grid-launcher.py:2221`'s `max(0, min(value, 60))`) — the stored
    /// value itself is not clamped on write.
    #[serde(default = "default_upload_delay_seconds")]
    pub auto_cloud_save_upload_delay_seconds: u64,
    /// How many server save records `cloud::retention` keeps per game
    /// (saves only; states are never pruned). Deviation D7 (doc 06 /
    /// `2026-09-02-cloud-saves-design.md`): Python hardcodes this to `3`
    /// (`grid-launcher.py:2224`); the rewrite makes it a config key with
    /// the same default. Read sites clamp to a minimum of `1`.
    #[serde(default = "default_retention_limit")]
    pub cloud_save_retention_limit: u32,
    /// Raw per-game cloud sync state, keyed by `cloud::state::game_key`.
    /// Stored as an untyped TOML table (not `SyncStateEntry`s) so foreign/
    /// future keys and mistyped fields round-trip byte-for-byte through a
    /// plain load/save; `cloud::state::normalize_sync_state` is the
    /// tolerant read-side view. See doc 06 "Sync state entry".
    #[serde(default)]
    pub cloud_sync_state: toml::value::Table,
    /// User-entered native (non-emulator) save directories, keyed by the
    /// same cache key scheme as `_pcgw_paths_cache`'s `"<key>__manual"`
    /// entries. `grid-launcher.py:434`.
    #[serde(default)]
    pub native_manual_save_paths: BTreeMap<String, Vec<String>>,
    /// Save paths the user removed from a native game's save-location list,
    /// keyed exactly like `native_manual_save_paths`. Filtered out of the
    /// PCGamingWiki list every time it is read, so a removed PCGW row does
    /// not come back on the next lookup.
    ///
    /// Deliberate improvement over the reference, which mutated only the
    /// in-memory `_pcgw_paths_cache` (`_pcgw_remove_path_for_game`,
    /// details_view_mixin.py:1218-1230) and therefore forgot the removal as
    /// soon as the cache was rebuilt. Adding a path back through
    /// `native_add_manual_save_path` clears it from here, so a removal is
    /// never permanent.
    #[serde(default)]
    pub native_removed_save_paths: BTreeMap<String, Vec<String>>,
    /// The compat tool (by name) offered as the default for Windows-only
    /// content on Linux/macOS, e.g. `"GE-Proton"`. Blank when unset.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_compat_tool: String,
    /// Compat tools GRID has installed and manages, keyed by nothing in
    /// particular — matched by `name` at read sites. Empty when none are
    /// installed, so a config saved before this field existed round-trips
    /// unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compat_tool_installs: Vec<CompatToolInstall>,
    /// Desktop shell appearance. A TOML table, so it must stay after every
    /// scalar key in this struct.
    #[serde(default)]
    pub ui: UiSettings,
    /// Unknown keys survive load/save round trips for forward compatibility.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

fn default_true() -> bool {
    true
}

fn default_upload_delay_seconds() -> u64 {
    3
}

fn default_retention_limit() -> u32 {
    3
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators: Vec::new(),
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            auto_cloud_save_download_on_launch: true,
            auto_cloud_save_upload_on_exit: true,
            auto_cloud_save_skip_download_if_local_newer: true,
            auto_cloud_save_upload_delay_seconds: 3,
            cloud_save_retention_limit: 3,
            cloud_sync_state: toml::value::Table::new(),
            native_manual_save_paths: BTreeMap::new(),
            native_removed_save_paths: BTreeMap::new(),
            default_compat_tool: String::new(),
            compat_tool_installs: Vec::new(),
            ui: UiSettings::default(),
            extra: BTreeMap::new(),
        }
    }
}

/// Test/portable override: when `GRID_LAUNCHER_DATA_DIR` is set and
/// non-empty, all app state (config.toml, grid-launcher.db, covers/) lives
/// under it. Trims whitespace; unset, empty, or whitespace-only yields
/// `None` so callers fall back to the platform `ProjectDirs` location.
pub fn data_dir_override() -> Option<PathBuf> {
    let value = std::env::var("GRID_LAUNCHER_DATA_DIR").ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        if let Some(dir) = data_dir_override() {
            return dir.join("config.toml");
        }
        directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
            .expect("home directory must exist")
            .config_dir()
            .join("config.toml")
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let mut config = match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.into()),
        };
        // Filter out emulators with blank names
        config.emulators.retain(|e| !e.name.trim().is_empty());
        Ok(config)
    }

    /// Atomic + durable: write `<path>.tmp`, fsync it, then rename over the
    /// target.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        let text = toml::to_string_pretty(self)?;
        // fsync the tmp file before renaming: a rename alone can land in the
        // directory while the file's contents are still only in the page
        // cache, so a crash would leave an empty/truncated config.
        let mut file = File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_settings_default_to_system_and_a_50_percent_fade() {
        let ui = UiSettings::default();
        assert_eq!(ui.theme, "system");
        assert_eq!(ui.background_fade, 50);
        assert_eq!(ui.background_blur, 2);
        assert_eq!(Config::default().ui, ui);
    }

    #[test]
    fn a_config_written_before_the_ui_table_existed_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\nserver_url = \"https://romm.example\"\n",
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "system");
        assert_eq!(loaded.ui.background_fade, 50);
        assert_eq!(loaded.ui.background_blur, 2);
    }

    #[test]
    fn ui_settings_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            ui: UiSettings {
                theme: "dark".to_string(),
                background_fade: 60,
                ..Default::default()
            },
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("[ui]"), "written config:\n{written}");
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "dark");
        assert_eq!(loaded.ui.background_fade, 60);
    }

    #[test]
    fn round_trips_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            schema_version: 1,
            server_url: "https://romm.example".into(),
            username: "six".into(),
            library_path: String::new(),
            emulators: Vec::new(),
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: Default::default(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), cfg);
    }

    #[test]
    fn missing_file_yields_default() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(&dir.path().join("nope.toml")).unwrap();
        assert_eq!(cfg.schema_version, 1);
        assert_eq!(cfg.server_url, "");
    }

    #[test]
    fn preserves_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\nserver_url = \"s\"\nusername = \"u\"\nfuture_key = \"kept\"\n",
        )
        .unwrap();
        let cfg = Config::load(&path).unwrap();
        cfg.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("future_key"));
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save(&path).unwrap();
        assert!(!dir.path().join("config.toml.tmp").exists());
        assert!(path.exists());
    }

    #[test]
    fn library_path_defaults_to_empty() {
        let cfg = Config::default();
        assert_eq!(cfg.library_path, "");
    }

    #[test]
    fn library_path_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            schema_version: 1,
            server_url: "https://romm.example".into(),
            username: "six".into(),
            library_path: "/path/to/library".into(),
            emulators: Vec::new(),
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: Default::default(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.library_path, "/path/to/library");
    }

    #[test]
    fn library_path_field_not_in_extra() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\nserver_url = \"s\"\nusername = \"u\"\nlibrary_path = \"/lib\"\nfuture_key = \"kept\"\n",
        )
        .unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.library_path, "/lib");
        assert!(!cfg.extra.contains_key("library_path"));
        assert!(cfg.extra.contains_key("future_key"));
    }

    #[test]
    fn emulator_array_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let emulators = vec![
            EmulatorEntry {
                name: "emulator1".into(),
                path: "/path/to/emu1".into(),
                args: "--arg1 --arg2".into(),
                ..Default::default()
            },
            EmulatorEntry {
                name: "emulator2".into(),
                path: "/path/to/emu2".into(),
                args: String::new(),
                ..Default::default()
            },
        ];
        let cfg = Config {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators,
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: BTreeMap::new(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.emulators.len(), 2);
        assert_eq!(loaded.emulators[0].name, "emulator1");
        assert_eq!(loaded.emulators[0].path, "/path/to/emu1");
        assert_eq!(loaded.emulators[0].args, "--arg1 --arg2");
        assert_eq!(loaded.emulators[1].name, "emulator2");
        assert_eq!(loaded.emulators[1].path, "/path/to/emu2");
        assert_eq!(loaded.emulators[1].args, "");
    }

    #[test]
    fn emulator_entry_without_source_fields_writes_no_source_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators: vec![EmulatorEntry {
                name: "emulator1".into(),
                path: "/path/to/emu1".into(),
                args: "--arg1 --arg2".into(),
                ..Default::default()
            }],
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: BTreeMap::new(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("source_id"));
        assert!(!written.contains("source_provider"));
        assert!(!written.contains("source_owner"));
        assert!(!written.contains("source_repo"));
        assert!(!written.contains("source_release_tag"));
    }

    #[test]
    fn emulator_entry_source_fields_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators: vec![EmulatorEntry {
                name: "PCSX2".into(),
                path: "/path/to/pcsx2".into(),
                args: String::new(),
                source_id: "PCSX2/pcsx2".into(),
                source_provider: "github".into(),
                source_owner: "PCSX2".into(),
                source_repo: "pcsx2".into(),
                source_release_tag: "v2.1.0".into(),
                ..Default::default()
            }],
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: BTreeMap::new(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        let entry = &loaded.emulators[0];
        assert_eq!(entry.source_id, "PCSX2/pcsx2");
        assert_eq!(entry.source_provider, "github");
        assert_eq!(entry.source_owner, "PCSX2");
        assert_eq!(entry.source_repo, "pcsx2");
        assert_eq!(entry.source_release_tag, "v2.1.0");
    }

    #[test]
    fn config_round_trips_the_five_new_emulator_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            emulators: vec![EmulatorEntry {
                name: "RetroArch".into(),
                path: "/x/retroarch".into(),
                args: "-L %core% %rom%".into(),
                save_strategy: "folder".into(),
                ignore_files: "thumbs.db".into(),
                ignore_extensions: ".jpg;\n.png".into(),
                save_paths: "~/saves;\n~/more".into(),
                state_paths: "~/states".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        let entry = &loaded.emulators[0];
        assert_eq!(entry.save_strategy, "folder");
        assert_eq!(entry.ignore_files, "thumbs.db");
        assert_eq!(entry.ignore_extensions, ".jpg;\n.png");
        assert_eq!(entry.save_paths, "~/saves;\n~/more");
        assert_eq!(entry.state_paths, "~/states");
    }

    #[test]
    fn config_without_the_new_fields_writes_no_new_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            emulators: vec![EmulatorEntry {
                name: "emulator1".into(),
                path: "/path/to/emu1".into(),
                args: "--arg1".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        // Match the field as a TOML key assignment (`"\n<key> ="`), not a
        // bare substring: `native_manual_save_paths` (added alongside the
        // other new Config-level fields) legitimately contains "save_paths"
        // as a substring, which a bare `contains` would false-positive on.
        for key in [
            "save_strategy",
            "ignore_files",
            "ignore_extensions",
            "save_paths",
            "state_paths",
        ] {
            let needle = format!("\n{key} =");
            assert!(!written.contains(&needle), "unexpected {key} in {written}");
        }
    }

    #[test]
    fn config_round_trips_retroachievements_username() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            retroachievements_username: "sixdd6".into(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.retroachievements_username, "sixdd6");
        assert!(!loaded.extra.contains_key("retroachievements_username"));
    }

    #[test]
    fn blank_name_emulator_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\n[[emulators]]\nname = \"valid\"\npath = \"/path\"\nargs = \"\"\n[[emulators]]\nname = \"\"\npath = \"/path\"\nargs = \"\"\n[[emulators]]\nname = \"   \"\npath = \"/path\"\nargs = \"\"\n",
        )
        .unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.emulators.len(), 1);
        assert_eq!(cfg.emulators[0].name, "valid");
    }

    #[test]
    fn native_removed_save_paths_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut removed = BTreeMap::new();
        removed.insert(
            "my game|windows".to_string(),
            vec!["%APPDATA%\\MyGame\\saves".to_string()],
        );
        let cfg = Config {
            native_removed_save_paths: removed,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.native_removed_save_paths.get("my game|windows"),
            Some(&vec!["%APPDATA%\\MyGame\\saves".to_string()])
        );
    }

    #[test]
    fn native_removed_save_paths_defaults_to_empty_for_an_older_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "schema_version = 1\n").unwrap();
        let cfg = Config::load(&path).unwrap();
        assert!(cfg.native_removed_save_paths.is_empty());
    }

    #[test]
    fn default_emulators_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut default_emulators = BTreeMap::new();
        default_emulators.insert("n64".into(), "mupen64plus".into());
        default_emulators.insert("snes".into(), "snes9x".into());
        let cfg = Config {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators: Vec::new(),
            default_emulators,
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: BTreeMap::new(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.default_emulators.get("n64").map(|s| s.as_str()),
            Some("mupen64plus")
        );
        assert_eq!(
            loaded.default_emulators.get("snes").map(|s| s.as_str()),
            Some("snes9x")
        );
    }

    #[test]
    fn retroarch_cores_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut retroarch_cores = BTreeMap::new();
        retroarch_cores.insert("n64".into(), "mupen64plus_next".into());
        retroarch_cores.insert("gba".into(), "mgba".into());
        let cfg = Config {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators: Vec::new(),
            default_emulators: BTreeMap::new(),
            retroarch_cores,
            launch_args: String::new(),
            retroachievements_username: String::new(),
            extra: BTreeMap::new(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.retroarch_cores.get("n64").map(|s| s.as_str()),
            Some("mupen64plus_next")
        );
        assert_eq!(
            loaded.retroarch_cores.get("gba").map(|s| s.as_str()),
            Some("mgba")
        );
    }

    #[test]
    fn launch_args_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            schema_version: 1,
            server_url: String::new(),
            username: String::new(),
            library_path: String::new(),
            emulators: Vec::new(),
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: "--fullscreen --no-menu".into(),
            retroachievements_username: String::new(),
            extra: BTreeMap::new(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.launch_args, "--fullscreen --no-menu");
    }

    #[test]
    fn new_fields_not_in_extra() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\nemulators = []\ndefault_emulators = {}\nretroarch_cores = {}\nlaunch_args = \"\"\nfuture_key = \"kept\"\n",
        )
        .unwrap();
        let cfg = Config::load(&path).unwrap();
        assert!(!cfg.extra.contains_key("emulators"));
        assert!(!cfg.extra.contains_key("default_emulators"));
        assert!(!cfg.extra.contains_key("retroarch_cores"));
        assert!(!cfg.extra.contains_key("launch_args"));
        assert!(cfg.extra.contains_key("future_key"));
    }

    #[test]
    fn config_defaults_for_the_seven_new_fields() {
        let cfg = Config::default();
        assert!(cfg.auto_cloud_save_download_on_launch);
        assert!(cfg.auto_cloud_save_upload_on_exit);
        assert!(cfg.auto_cloud_save_skip_download_if_local_newer);
        assert_eq!(cfg.auto_cloud_save_upload_delay_seconds, 3);
        assert_eq!(cfg.cloud_save_retention_limit, 3);
        assert!(cfg.cloud_sync_state.is_empty());
        assert!(cfg.native_manual_save_paths.is_empty());
    }

    #[test]
    fn seven_new_fields_round_trip_and_preserve_unknown_cloud_sync_state_junk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut native_manual_save_paths = BTreeMap::new();
        native_manual_save_paths.insert(
            "Chrono Trigger__manual".to_string(),
            vec!["/mnt/saves/ct".to_string()],
        );
        let cfg = Config {
            auto_cloud_save_download_on_launch: false,
            auto_cloud_save_upload_on_exit: false,
            auto_cloud_save_skip_download_if_local_newer: false,
            auto_cloud_save_upload_delay_seconds: 45,
            cloud_save_retention_limit: 7,
            native_manual_save_paths,
            ..Default::default()
        };
        cfg.save(&path).unwrap();

        // Simulate foreign/future junk landing in cloud_sync_state between
        // saves: an entry with an extra unknown key alongside known ones,
        // and a wrong-typed field. A plain load/save round trip must
        // preserve it byte-for-byte — only `normalize_sync_state` (a
        // read-side transform, not part of Config::load/save) is tolerant.
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(
            "\n[cloud_sync_state.\"rom:abc\"]\nlast_downloaded_save_id = \"srv-1\"\nfrom_the_future = \"kept\"\nlast_server_timestamp = \"not-a-number\"\n",
        );
        std::fs::write(&path, &text).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert!(!loaded.auto_cloud_save_download_on_launch);
        assert!(!loaded.auto_cloud_save_upload_on_exit);
        assert!(!loaded.auto_cloud_save_skip_download_if_local_newer);
        assert_eq!(loaded.auto_cloud_save_upload_delay_seconds, 45);
        assert_eq!(loaded.cloud_save_retention_limit, 7);
        assert_eq!(
            loaded
                .native_manual_save_paths
                .get("Chrono Trigger__manual")
                .map(|v| v.as_slice()),
            Some(["/mnt/saves/ct".to_string()].as_slice())
        );

        loaded.save(&path).unwrap();
        let rewritten = std::fs::read_to_string(&path).unwrap();
        assert!(
            rewritten.contains("from_the_future"),
            "unknown field in a sync-state entry round-trips"
        );
        assert!(
            rewritten.contains("not-a-number"),
            "wrong-typed field round-trips raw, unnormalized"
        );
        assert!(!loaded.extra.contains_key("cloud_sync_state"));
    }

    #[test]
    fn compat_tool_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.default_compat_tool, "");
        assert!(cfg.compat_tool_installs.is_empty());
    }

    #[test]
    fn config_without_compat_tool_fields_writes_no_new_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            emulators: vec![EmulatorEntry {
                name: "emulator1".into(),
                path: "/path/to/emu1".into(),
                args: "--arg1".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("\ndefault_compat_tool ="));
        assert!(!written.contains("\ncompat_tool_installs"));
    }

    #[test]
    fn compat_tool_install_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            default_compat_tool: "GE-Proton".into(),
            compat_tool_installs: vec![CompatToolInstall {
                name: "GE-Proton".into(),
                path: "/home/six/.local/share/compat-tools/GE-Proton9-20".into(),
                source_id: "GloriousEggroll/proton-ge-custom".into(),
                release_tag: "GE-Proton9-20".into(),
            }],
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.default_compat_tool, "GE-Proton");
        assert_eq!(loaded.compat_tool_installs.len(), 1);
        let install = &loaded.compat_tool_installs[0];
        assert_eq!(install.name, "GE-Proton");
        assert_eq!(
            install.path,
            "/home/six/.local/share/compat-tools/GE-Proton9-20"
        );
        assert_eq!(install.source_id, "GloriousEggroll/proton-ge-custom");
        assert_eq!(install.release_tag, "GE-Proton9-20");
    }

    #[test]
    fn card_sizes_default_to_medium_for_both_views() {
        let ui = UiSettings::default();
        assert_eq!(ui.card_size_library, "medium");
        assert_eq!(ui.card_size_server, "medium");
    }

    #[test]
    fn a_ui_table_written_before_the_card_sizes_existed_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\n\n[ui]\ntheme = \"dark\"\nbackground_fade = 40\n",
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "dark");
        assert_eq!(loaded.ui.background_fade, 40);
        assert_eq!(loaded.ui.card_size_library, "medium");
        assert_eq!(loaded.ui.card_size_server, "medium");
    }

    /// A `[ui]` table written before the blur setting existed loads the
    /// default rather than failing: `background_blur` is `serde(default)`.
    #[test]
    fn a_ui_table_written_before_the_blur_existed_loads_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\n\n[ui]\ntheme = \"dark\"\nbackground_fade = 40\n",
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.background_blur, 2);
    }

    #[test]
    fn the_background_blur_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            ui: UiSettings {
                background_blur: 30,
                ..Default::default()
            },
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("background_blur"),
            "written config:\n{written}"
        );
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.background_blur, 30);
    }

    #[test]
    fn card_sizes_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            ui: UiSettings {
                theme: "system".to_string(),
                background_fade: 50,
                background_blur: 2,
                card_size_library: "large".to_string(),
                card_size_server: "small".to_string(),
            },
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.card_size_library, "large");
        assert_eq!(loaded.ui.card_size_server, "small");
    }
}
