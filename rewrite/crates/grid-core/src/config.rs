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
    /// Unknown keys survive load/save round trips for forward compatibility.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
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
            extra: Default::default(),
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
            extra: Default::default(),
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
            extra: BTreeMap::new(),
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
            extra: BTreeMap::new(),
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
            }],
            default_emulators: BTreeMap::new(),
            retroarch_cores: BTreeMap::new(),
            launch_args: String::new(),
            extra: BTreeMap::new(),
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
            extra: BTreeMap::new(),
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
            extra: BTreeMap::new(),
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
            extra: BTreeMap::new(),
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
}
