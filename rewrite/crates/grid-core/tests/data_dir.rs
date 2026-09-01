//! `GRID_LAUNCHER_DATA_DIR` override, in its own test binary.
//!
//! `std::env::set_var` is process-global. One test in one binary means no
//! sibling test can observe the change mid-flight (same pattern as
//! `launch_tilde.rs`). The four states — set, unset, empty, whitespace-only
//! — are driven sequentially within the single test so each starts from a
//! known env state.

use grid_core::config::{data_dir_override, Config};

#[test]
fn data_dir_override_drives_default_path() {
    let project_dirs_path = directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
        .unwrap()
        .config_dir()
        .join("config.toml");

    // Set and non-empty: default_path lands under the override.
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", dir.path());
    assert_eq!(data_dir_override(), Some(dir.path().to_path_buf()));
    assert_eq!(Config::default_path(), dir.path().join("config.toml"));

    // Unset: falls back to the ProjectDirs path, unchanged.
    std::env::remove_var("GRID_LAUNCHER_DATA_DIR");
    assert_eq!(data_dir_override(), None);
    assert_eq!(Config::default_path(), project_dirs_path);

    // Empty: treated the same as unset.
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", "");
    assert_eq!(data_dir_override(), None);
    assert_eq!(Config::default_path(), project_dirs_path);

    // Whitespace-only: trims to empty, so None.
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", "   ");
    assert_eq!(data_dir_override(), None);
    assert_eq!(Config::default_path(), project_dirs_path);

    std::env::remove_var("GRID_LAUNCHER_DATA_DIR");
}
