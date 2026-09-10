//! The one-shot import of a Python-era configuration, run at startup.
//!
//! The trigger is file presence, not a stored flag: no Rust `config.toml`
//! and a Python `~/.grid-launcher/config.json`. A successful import always
//! writes `config.toml`, so this runs at most once per profile even when
//! some rows were skipped.
//!
//! Every log line here carries counts only — never a path, a key or a value
//! from the file being read.

use grid_core::import_python::{self, ImportReport};
use grid_core::library::registry::Registry;
use std::path::{Path, PathBuf};

/// `~/.grid-launcher/config.json`, the reference's persistence root on every
/// platform (doc 02, "Persistence root"; grid-launcher.py:2386-2393).
///
/// `GRID_LAUNCHER_DATA_DIR` deliberately does NOT apply: it redirects the
/// Rust side's own state, and the Python app never read it.
///
/// `GRID_LAUNCHER_PYTHON_CONFIG` does: when set and non-empty, that path IS
/// the Python config path, in every build. The e2e harness points it at a
/// fixture (and, for every other stage, at a path that does not exist) so a
/// throwaway profile never reads a developer's real `~/.grid-launcher`.
/// Presence is still decided by [`should_import`], so a missing override
/// path simply means "nothing to import".
pub fn python_config_path() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var("GRID_LAUNCHER_PYTHON_CONFIG")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Some(PathBuf::from(override_path));
    }
    grid_core::autoconfig::paths::home_dir()
        .map(|home| home.join(".grid-launcher").join("config.json"))
}

/// Whether the importer should run. Pure, so the startup decision is
/// testable without a Tauri app.
pub fn should_import(rust_config: &Path, python_config: &Path) -> bool {
    !rust_config.exists() && python_config.exists()
}

/// Runs the import when [`should_import`] says to, and returns the report
/// for `AppState.python_import`.
///
/// Returns `None` for every non-event — nothing to import, or an import that
/// failed. A failure is logged once and the app starts normally: a fresh
/// profile is a working profile.
pub fn startup_import(
    rust_config: &Path,
    python_config: &Path,
    registry: &Registry,
    now: i64,
) -> Option<ImportReport> {
    if !should_import(rust_config, python_config) {
        return None;
    }
    match import_python::import(python_config, rust_config, registry, now) {
        Ok(report) => {
            tracing::info!(
                "imported the previous version's settings: {} emulators, {} games, {} rows skipped",
                report.emulators,
                report.games,
                report.skipped_games
            );
            Some(report)
        }
        Err(e) => {
            // `ImportError`'s Display carries no path and no file content.
            tracing::warn!("the previous version's settings were not imported: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::config::Config;

    const MINIMAL: &str = r#"{
      "server_url": "https://romm.example.test",
      "api_token": "SECRET-DO-NOT-IMPORT",
      "retroachievements_username": "ashley",
      "emulators": [{ "name": "RetroArch", "path": "/opt/retroarch" }],
      "installed_games": [{ "title": "Chrono Trigger", "platform": "SNES", "rom_id": "4321" }]
    }"#;

    struct Scratch {
        _dir: tempfile::TempDir,
        rust_config: std::path::PathBuf,
        python_config: std::path::PathBuf,
        registry: Registry,
    }

    fn scratch() -> Scratch {
        let dir = tempfile::tempdir().expect("a tempdir");
        let python_config = dir.path().join("python-config.json");
        std::fs::write(&python_config, MINIMAL).expect("the fixture writes");
        let registry = Registry::open(&dir.path().join("grid-launcher.db")).expect("a registry");
        Scratch {
            rust_config: dir.path().join("config.toml"),
            python_config,
            registry,
            _dir: dir,
        }
    }

    #[test]
    fn a_python_file_and_no_rust_config_means_import() {
        let s = scratch();
        assert!(should_import(&s.rust_config, &s.python_config));
    }

    #[test]
    fn an_existing_rust_config_means_no_import() {
        let s = scratch();
        Config::default().save(&s.rust_config).unwrap();
        assert!(!should_import(&s.rust_config, &s.python_config));
    }

    #[test]
    fn no_python_file_means_no_import() {
        let s = scratch();
        std::fs::remove_file(&s.python_config).unwrap();
        assert!(!should_import(&s.rust_config, &s.python_config));
    }

    #[test]
    fn startup_import_reports_counts_and_leaves_a_rust_config() {
        let s = scratch();
        let report = startup_import(&s.rust_config, &s.python_config, &s.registry, 42)
            .expect("a fresh profile with a Python config imports");
        assert_eq!(report.emulators, 1);
        assert_eq!(report.games, 1);
        assert_eq!(report.skipped_games, 0);
        assert!(report.retroachievements);
        assert!(s.rust_config.exists());
        assert_eq!(s.registry.all().unwrap().len(), 1);
    }

    #[test]
    fn startup_import_is_a_no_op_once_a_rust_config_exists() {
        let s = scratch();
        Config::default().save(&s.rust_config).unwrap();
        assert!(startup_import(&s.rust_config, &s.python_config, &s.registry, 42).is_none());
        assert!(s.registry.all().unwrap().is_empty());
    }

    #[test]
    fn a_broken_python_config_yields_no_notice_and_no_rust_config() {
        let s = scratch();
        std::fs::write(&s.python_config, "{ not json").unwrap();
        assert!(startup_import(&s.rust_config, &s.python_config, &s.registry, 42).is_none());
        assert!(!s.rust_config.exists());
    }

    #[test]
    fn the_python_path_is_the_fixed_dot_directory() {
        let _lock = crate::test_env::lock();
        let _env = crate::test_env::EnvGuard::set(&[("GRID_LAUNCHER_PYTHON_CONFIG", None)]);
        let path = python_config_path().expect("a home directory");
        assert!(path.ends_with(".grid-launcher/config.json"));
    }

    #[test]
    fn the_env_override_replaces_the_python_path() {
        let _lock = crate::test_env::lock();
        let _env = crate::test_env::EnvGuard::set(&[(
            "GRID_LAUNCHER_PYTHON_CONFIG",
            Some("/nonexistent/python-config.json"),
        )]);
        assert_eq!(
            python_config_path(),
            Some(PathBuf::from("/nonexistent/python-config.json"))
        );
    }

    #[test]
    fn an_empty_env_override_falls_back_to_the_dot_directory() {
        let _lock = crate::test_env::lock();
        let _env = crate::test_env::EnvGuard::set(&[("GRID_LAUNCHER_PYTHON_CONFIG", Some(""))]);
        let path = python_config_path().expect("a home directory");
        assert!(path.ends_with(".grid-launcher/config.json"));
    }
}
