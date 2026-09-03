//! `LaunchService::launch`'s native branch, resolving Wine through the real
//! `PATH`, in its own test binary.
//!
//! This is a separate integration target on purpose (same pattern as
//! `launch_tilde.rs` and `data_dir.rs`): the native branch calls the
//! production `which_on_path`, so making it find a fake "wine" means
//! mutating the real process `PATH`. `std::env::set_var` is process-global;
//! one test in one binary means no sibling test can observe the change.
//! Unix only — the stub is a `/bin/sh` script.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::Config;
use grid_core::launch::LaunchService;
use grid_core::library::registry::{InstalledGame, Registry};

#[tokio::test]
async fn a_native_windows_row_launches_through_a_path_resolved_wine() {
    let dir = tempfile::tempdir().unwrap();

    // The game's own "executable": never itself spawned (Wine is the
    // process; the exe path is just an argument to it), so a plain file is
    // enough.
    let install_dir = dir.path().join("library").join("Windows").join("MyGame");
    fs::create_dir_all(&install_dir).unwrap();
    let game_exe = install_dir.join("mygame.exe");
    fs::write(&game_exe, b"pe bytes").unwrap();

    // A "wine" stub that records its argv, then sleeps so the session stays
    // observable. Placed in its own directory, prepended to PATH — not
    // wherever a real `wine` might already live.
    let wine_dir = dir.path().join("winebin");
    fs::create_dir_all(&wine_dir).unwrap();
    let record = dir.path().join("wine.args");
    let wine_exe = wine_dir.join("wine");
    fs::write(
        &wine_exe,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nsleep 30\n",
            record.to_string_lossy()
        ),
    )
    .unwrap();
    fs::set_permissions(&wine_exe, fs::Permissions::from_mode(0o755)).unwrap();

    let library = dir.path().join("library");
    let config_path = dir.path().join("config.toml");
    Config {
        library_path: library.to_string_lossy().into_owned(),
        default_compat_tool: "wine".to_string(),
        ..Config::default()
    }
    .save(&config_path)
    .unwrap();

    let registry = Arc::new(Registry::open(&dir.path().join("registry.sqlite3")).unwrap());
    registry
        .upsert(&InstalledGame {
            title: "MyGame".to_string(),
            platform: "Windows".to_string(),
            rom_id: Some(9),
            rom_file_name: "MyGame.zip".to_string(),
            extracted_dir: install_dir.to_string_lossy().into_owned(),
            native_launch_parameters: "--windowed --fast".to_string(),
            ..Default::default()
        })
        .unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{original_path}", wine_dir.to_string_lossy());
    std::env::set_var("PATH", &new_path);

    let service =
        LaunchService::new_with_poll_interval(registry, config_path, Duration::from_millis(50));
    let launch_result = service.launch(9).await;

    // Restore PATH immediately, whether or not the launch succeeded, so a
    // panic below still leaves the process env sane.
    std::env::set_var("PATH", &original_path);

    let session = launch_result.unwrap();
    assert_eq!(session.emulator_name, "wine");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !record.is_file() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(record.is_file(), "the wine stub never wrote its argv");

    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(
        lines,
        vec![game_exe.to_string_lossy().as_ref(), "--windowed", "--fast",]
    );

    service.stop(session.id);
}
