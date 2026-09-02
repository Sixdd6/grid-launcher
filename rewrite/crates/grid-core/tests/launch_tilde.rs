//! `~` expansion of a recorded ROM path, in its own test binary.
//!
//! This is a separate integration target on purpose: it overrides `HOME`, and
//! `std::env::set_var` is process-global. One test in one binary means no
//! sibling test can observe the change. Unix only — the stub emulator is a
//! `/bin/sh` script and `HOME` is the unix home variable.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::{Config, EmulatorEntry};
use grid_core::launch::LaunchService;
use grid_core::library::registry::{InstalledGame, Registry};

/// A recorded `archive_path` of the form `~/...` must reach the emulator as
/// an absolute path. The existence check already expanded it, so before the
/// fix the game passed validation and the emulator was then handed a literal
/// tilde it would fail to open.
#[tokio::test]
async fn a_tilde_rom_path_reaches_the_emulator_expanded() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let roms = home.join("roms");
    fs::create_dir_all(&roms).unwrap();
    // SAFETY-ish: this binary holds exactly one test, so nothing else is
    // reading the environment while it is replaced.
    std::env::set_var("HOME", &home);

    let rom = roms.join("Chrono.rom");
    fs::write(&rom, b"rom bytes").unwrap();

    let record = dir.path().join("args.txt");
    let exe = dir.path().join("stub-emu");
    fs::write(
        &exe,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nsleep 30\n",
            record.to_string_lossy()
        ),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();

    let library = dir.path().join("library");
    fs::create_dir_all(&library).unwrap();
    let config_path = dir.path().join("config.toml");
    Config {
        library_path: library.to_string_lossy().into_owned(),
        emulators: vec![EmulatorEntry {
            name: "Stub".to_string(),
            path: exe.to_string_lossy().into_owned(),
            args: "%rom%".to_string(),
            ..Default::default()
        }],
        default_emulators: [("SNES".to_string(), "Stub".to_string())]
            .into_iter()
            .collect(),
        ..Config::default()
    }
    .save(&config_path)
    .unwrap();

    let registry = Arc::new(Registry::open(&dir.path().join("registry.sqlite3")).unwrap());
    registry
        .upsert(&InstalledGame {
            title: "Chrono".to_string(),
            platform: "SNES".to_string(),
            rom_id: Some(7),
            rom_file_name: "Chrono.rom".to_string(),
            // Recorded with a tilde *and* padding. The padding is what
            // forces the resolver down its raw fallback: it expands the
            // untrimmed string, which has no leading `~/` to expand, finds
            // no such file, and hands back the trimmed `~/...` text. The
            // validation step then expands that and passes, so before the
            // fix the emulator received the literal tilde.
            archive_path: "  ~/roms/Chrono.rom  ".to_string(),
            ..Default::default()
        })
        .unwrap();

    let service =
        LaunchService::new_with_poll_interval(registry, config_path, Duration::from_millis(50));
    let session = service.launch(7).await.unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while !record.is_file() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(record.is_file(), "the stub never wrote its argv");

    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(lines, vec![rom.to_string_lossy().as_ref()]);

    service.stop(session.id);
}
