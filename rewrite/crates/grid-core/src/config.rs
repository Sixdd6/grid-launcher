use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
            extra: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
            .expect("home directory must exist")
            .config_dir()
            .join("config.toml")
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic: write `<path>.tmp`, then rename over the target.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
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
}
