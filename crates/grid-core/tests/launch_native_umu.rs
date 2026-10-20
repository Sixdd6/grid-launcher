//! `LaunchService::launch`'s Proton branch and the `umu-run` it resolves:
//! none at all is `LaunchError::UmuMissing`; GRID's managed copy
//! (`Config.umu_launcher`) is used when `PATH` has none, but only once the
//! `python3` the child would find is 3.10 or newer.
//!
//! Its own test binary, on purpose (same pattern as `launch_native_wine.rs`):
//! the lookups read the real process `PATH`, and `std::env::set_var` is
//! process-global. One test in one binary means no sibling test can observe
//! the change. Unix only — the stubs are `/bin/sh` scripts.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::{Config, UmuLauncherInstall};
use grid_core::launch::{LaunchError, LaunchService};
use grid_core::library::registry::{InstalledGame, Registry};

fn write_script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn python_stub(dir: &Path, version: &str) {
    write_script(
        &dir.join("python3"),
        &format!("#!/bin/sh\necho 'Python {version}'\n"),
    );
}

#[tokio::test]
async fn a_proton_row_needs_a_umu_run_and_the_managed_copy_needs_python_3_10() {
    let dir = tempfile::tempdir().unwrap();

    let install_dir = dir.path().join("library").join("Windows").join("MyGame");
    fs::create_dir_all(&install_dir).unwrap();
    let game_exe = install_dir.join("mygame.exe");
    fs::write(&game_exe, b"pe bytes").unwrap();

    // The "managed umu-run": records its argv and PROTONPATH, then sleeps
    // so the session stays observable. `/bin/sleep` by absolute path: the
    // test's PATH holds only the stub directory.
    let record = dir.path().join("umu.args");
    let managed = dir
        .path()
        .join("compat-tools")
        .join("umu-launcher")
        .join("umu");
    fs::create_dir_all(&managed).unwrap();
    let umu_run = managed.join("umu-run");
    write_script(
        &umu_run,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$PROTONPATH\" \"$@\" > '{}'\nexec /bin/sleep 30\n",
            record.to_string_lossy()
        ),
    );

    let library = dir.path().join("library");
    let config_path = dir.path().join("config.toml");
    let proton = "/opt/GE-Proton9-1";
    let base_config = Config {
        library_path: library.to_string_lossy().into_owned(),
        default_compat_tool: proton.to_string(),
        ..Config::default()
    };
    base_config.save(&config_path).unwrap();

    let registry = Arc::new(Registry::open(&dir.path().join("registry.sqlite3")).unwrap());
    registry
        .upsert(&InstalledGame {
            title: "MyGame".to_string(),
            platform: "Windows".to_string(),
            rom_id: Some(9),
            rom_file_name: "MyGame.zip".to_string(),
            extracted_dir: install_dir.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .unwrap();

    // PATH is only `bin`: no system umu-run, and python3 is whatever stub
    // the scenario puts there.
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let original_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", &bin);

    let service = LaunchService::new_with_poll_interval(
        registry,
        config_path.clone(),
        Duration::from_millis(50),
    );

    // --- (a) no umu-run anywhere ---------------------------------------------
    let missing = service.launch(9).await;

    // --- (b) the managed copy, with a Python that is too old -----------------
    Config {
        umu_launcher: Some(UmuLauncherInstall {
            path: umu_run.to_string_lossy().into_owned(),
            release_tag: "1.4.4".to_string(),
        }),
        ..base_config.clone()
    }
    .save(&config_path)
    .unwrap();
    python_stub(&bin, "3.9.18");
    let old_python = service.launch(9).await;

    // --- (c) no python3 at all ------------------------------------------------
    fs::remove_file(bin.join("python3")).unwrap();
    let no_python = service.launch(9).await;

    // --- (d) a new enough Python: the launch runs through the managed copy ----
    python_stub(&bin, "3.12.3");
    let launched = service.launch(9).await;

    std::env::set_var("PATH", &original_path);

    assert!(
        matches!(missing, Err(LaunchError::UmuMissing)),
        "expected UmuMissing, got {missing:?}"
    );
    match old_python {
        Err(LaunchError::Validation(message)) => {
            assert!(message.contains("3.10 or newer"), "{message}");
            assert!(message.contains("python3 is 3.9"), "{message}");
        }
        other => panic!("expected the Python version error, got {other:?}"),
    }
    match no_python {
        Err(LaunchError::Validation(message)) => {
            assert!(message.contains("python3 was not found"), "{message}")
        }
        other => panic!("expected the missing-Python error, got {other:?}"),
    }

    let session = launched.unwrap();
    assert_eq!(session.emulator_name, proton);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !record.is_file() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(record.is_file(), "the managed umu-run never ran");
    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(lines, vec![proton, game_exe.to_string_lossy().as_ref()]);

    service.stop(session.id);
}
