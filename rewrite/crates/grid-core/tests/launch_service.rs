//! End-to-end tests for `LaunchService`: emulator resolution, spawning,
//! session bookkeeping, stop, and the early-exit warning. The "emulators" are
//! shell scripts written into a tempdir, so the tests exercise real child
//! processes without needing a real emulator.
//!
//! Unix only: the stubs are `/bin/sh` scripts.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use grid_core::config::{Config, EmulatorEntry};
use grid_core::launch::{LaunchError, LaunchService, SessionsSnapshot};
use grid_core::library::registry::{InstalledGame, Registry};

// --- fixtures ---------------------------------------------------------------

/// Poll interval used by every test service: short enough that a stopped
/// session is reaped well inside the assertion budget below.
const TEST_POLL: Duration = Duration::from_millis(50);
/// Upper bound for "the background task should have done this by now".
const BUDGET: Duration = Duration::from_secs(5);

struct Harness {
    _dir: tempfile::TempDir,
    root: PathBuf,
    library: PathBuf,
    config_path: PathBuf,
    registry: Arc<Registry>,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let library = root.join("library");
        fs::create_dir_all(&library).unwrap();
        let registry = Arc::new(Registry::open(&root.join("registry.sqlite3")).unwrap());
        Self {
            _dir: dir,
            config_path: root.join("config.toml"),
            root,
            library,
            registry,
        }
    }

    /// Writes an executable `/bin/sh` stub at `<root>/<name>` and returns it.
    fn stub(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A stub that records its argv, one argument per line, then sleeps.
    fn recording_stub(&self, name: &str) -> (PathBuf, PathBuf) {
        let record = self.root.join(format!("{name}.args"));
        let exe = self.stub(
            name,
            &format!(
                "printf '%s\\n' \"$@\" > '{}'\nsleep 30",
                record.to_string_lossy()
            ),
        );
        (exe, record)
    }

    fn write_config(&self, emulators: Vec<EmulatorEntry>, defaults: &[(&str, &str)]) {
        self.write_config_full(emulators, defaults, &[], "");
    }

    fn write_config_full(
        &self,
        emulators: Vec<EmulatorEntry>,
        defaults: &[(&str, &str)],
        cores: &[(&str, &str)],
        launch_args: &str,
    ) {
        let config = Config {
            library_path: self.library.to_string_lossy().into_owned(),
            emulators,
            default_emulators: pairs(defaults),
            retroarch_cores: pairs(cores),
            launch_args: launch_args.to_string(),
            ..Config::default()
        };
        config.save(&self.config_path).unwrap();
    }

    /// Registers an installed game whose ROM file exists on disk, and returns
    /// that ROM path.
    fn install_game(&self, rom_id: i64, title: &str, platform: &str) -> PathBuf {
        let platform_dir = self.library.join(platform);
        fs::create_dir_all(&platform_dir).unwrap();
        let rom = platform_dir.join(format!("{title}.rom"));
        fs::write(&rom, b"rom bytes").unwrap();
        self.registry
            .upsert(&InstalledGame {
                title: title.to_string(),
                platform: platform.to_string(),
                rom_id: Some(rom_id),
                rom_file_name: format!("{title}.rom"),
                archive_path: rom.to_string_lossy().into_owned(),
                ..Default::default()
            })
            .unwrap();
        rom
    }

    /// Registers a game whose recorded `archive_path` is `archive_path` and
    /// which has nothing on disk.
    fn install_row(&self, rom_id: i64, title: &str, platform: &str, archive_path: &str) {
        self.registry
            .upsert(&InstalledGame {
                title: title.to_string(),
                platform: platform.to_string(),
                rom_id: Some(rom_id),
                rom_file_name: format!("{title}.rom"),
                archive_path: archive_path.to_string(),
                ..Default::default()
            })
            .unwrap();
    }

    fn service(&self) -> Arc<LaunchService> {
        LaunchService::new_with_poll_interval(
            self.registry.clone(),
            self.config_path.clone(),
            TEST_POLL,
        )
    }
}

fn pairs(items: &[(&str, &str)]) -> BTreeMap<String, String> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn entry(name: &str, path: &Path, args: &str) -> EmulatorEntry {
    EmulatorEntry {
        name: name.to_string(),
        path: path.to_string_lossy().into_owned(),
        args: args.to_string(),
    }
}

/// Collects every snapshot the service emits.
#[derive(Default)]
struct Recorder {
    snapshots: Mutex<Vec<SessionsSnapshot>>,
}

impl Recorder {
    fn attach(service: &Arc<LaunchService>) -> Arc<Self> {
        let recorder = Arc::new(Self::default());
        let sink = recorder.clone();
        service.set_notify(Arc::new(move |snapshot| {
            sink.snapshots.lock().unwrap().push(snapshot);
        }));
        recorder
    }

    fn warnings(&self) -> Vec<String> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .filter_map(|s| s.warning.clone())
            .collect()
    }
}

/// Polls `check` until it returns true or [`BUDGET`] elapses.
async fn wait_until(mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    check()
}

fn validation_message(error: LaunchError) -> String {
    match error {
        LaunchError::Validation(message) => message,
        other => panic!("expected a validation error, got: {other:?}"),
    }
}

// --- tests ------------------------------------------------------------------

#[tokio::test]
async fn launch_registers_a_session_and_passes_the_rom_path() {
    let h = Harness::new();
    let (exe, record) = h.recording_stub("stub-emu");
    h.write_config(
        vec![entry("Stub", &exe, "--run %rom%")],
        &[("SNES", "Stub")],
    );
    let rom = h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    service.spawn_poll_loop();
    let session = service.launch(7).await.unwrap();

    assert_eq!(session.rom_id, 7);
    assert_eq!(session.title, "Chrono");
    assert_eq!(session.emulator_name, "Stub");
    assert!(session.pid > 0, "expected a real pid");
    assert!(session.started_at > 0, "expected a unix timestamp");

    let snapshot = service.snapshot();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].id, session.id);
    assert!(snapshot.warning.is_none());

    let recorded = wait_until(|| record.is_file()).await;
    assert!(recorded, "the stub never wrote its argv");
    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(lines, vec!["--run", rom.to_string_lossy().as_ref()]);

    service.stop(session.id);
}

#[tokio::test]
async fn stop_removes_the_session_within_the_poll_budget() {
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    service.spawn_poll_loop();
    let session = service.launch(7).await.unwrap();
    assert_eq!(service.snapshot().sessions.len(), 1);

    service.stop(session.id);

    let gone = wait_until(|| service.snapshot().sessions.is_empty()).await;
    assert!(gone, "the stopped session was never reaped");
}

#[tokio::test]
async fn an_instant_exit_removes_the_session_and_warns() {
    let h = Harness::new();
    let exe = h.stub("quitter", "exit 3");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    let rom = h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let recorder = Recorder::attach(&service);
    // No poll loop here on purpose: the 500 ms early-exit check alone must
    // remove the session and produce the warning.
    let _session = service.launch(7).await.unwrap();

    let gone = wait_until(|| service.snapshot().sessions.is_empty()).await;
    assert!(gone, "the exited session was never removed");

    let warnings = recorder.warnings();
    assert_eq!(warnings.len(), 1, "expected exactly one warning");
    assert_eq!(
        warnings[0],
        format!(
            "Game exited immediately (code 3): {} {}",
            exe.to_string_lossy(),
            rom.to_string_lossy()
        )
    );
}

#[tokio::test]
async fn a_running_poll_loop_does_not_swallow_the_early_exit_warning() {
    // Regression guard: the poll loop ticks every 50 ms here, well inside the
    // 500 ms early-exit window. If it were allowed to reap a brand-new child,
    // the early-exit check would find nothing and the user would never be
    // told the game failed to start.
    let h = Harness::new();
    let exe = h.stub("quitter", "exit 3");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    let rom = h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let recorder = Recorder::attach(&service);
    service.spawn_poll_loop();
    service.launch(7).await.unwrap();

    let warned = wait_until(|| !recorder.warnings().is_empty()).await;
    assert!(warned, "the early-exit warning was never emitted");
    assert_eq!(
        recorder.warnings()[0],
        format!(
            "Game exited immediately (code 3): {} {}",
            exe.to_string_lossy(),
            rom.to_string_lossy()
        )
    );
    assert!(service.snapshot().sessions.is_empty());
}

#[tokio::test]
async fn a_second_launch_of_the_same_rom_is_already_running() {
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let session = service.launch(7).await.unwrap();
    let error = service.launch(7).await.unwrap_err();

    assert!(
        matches!(error, LaunchError::AlreadyRunning),
        "unexpected error: {error:?}"
    );
    assert_eq!(error.to_string(), "This game is already running.");
    assert_eq!(service.snapshot().sessions.len(), 1);

    service.stop(session.id);
}

#[tokio::test]
async fn a_missing_emulator_executable_is_reported_verbatim() {
    let h = Harness::new();
    let missing = h.root.join("not-there");
    h.write_config(vec![entry("Stub", &missing, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let error = service.launch(7).await.unwrap_err();
    assert_eq!(
        validation_message(error),
        format!(
            "Emulator executable not found:\n{}",
            missing.to_string_lossy()
        )
    );
}

#[tokio::test]
async fn a_missing_rom_file_is_reported_verbatim() {
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_row(7, "Chrono", "SNES", "/nowhere/Chrono.rom");

    let service = h.service();
    let error = service.launch(7).await.unwrap_err();
    assert_eq!(
        validation_message(error),
        "ROM file not found:\n/nowhere/Chrono.rom"
    );
}

#[tokio::test]
async fn a_blank_rom_path_is_reported_verbatim() {
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    // Nothing on disk and no recorded archive path: the resolver yields "".
    // The platform is arcade so the extracted-candidate branch is skipped
    // and the blank archive path falls straight through.
    h.install_row(7, "Chrono", "Arcade", "");

    let service = h.service();
    let error = service.launch(7).await.unwrap_err();
    assert_eq!(
        validation_message(error),
        "No ROM file is available for this game."
    );
}

#[tokio::test]
async fn no_configured_emulator_is_reported_verbatim() {
    let h = Harness::new();
    h.write_config(Vec::new(), &[]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let error = service.launch(7).await.unwrap_err();
    assert_eq!(
        validation_message(error),
        "No emulator is configured. Add one in Emulators settings."
    );
}

#[tokio::test]
async fn a_rom_that_is_not_installed_is_rejected() {
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);

    let service = h.service();
    let error = service.launch(404).await.unwrap_err();
    assert!(
        matches!(error, LaunchError::NotInstalled),
        "unexpected error: {error:?}"
    );
    assert_eq!(error.to_string(), "Game is not installed.");
}

#[tokio::test]
async fn a_windows_platform_row_is_not_supported_yet() {
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("Windows", "Stub")]);
    h.install_game(7, "Chrono", "Windows");

    let service = h.service();
    let error = service.launch(7).await.unwrap_err();
    assert_eq!(
        validation_message(error),
        "Native Windows games are not supported yet in the Rust preview."
    );
}

#[tokio::test]
async fn a_retroarch_relative_core_is_rewritten_to_an_absolute_path() {
    let h = Harness::new();
    let (exe, record) = h.recording_stub("retroarch");
    let cores = h.root.join("cores");
    fs::create_dir_all(&cores).unwrap();
    let core = cores.join("snes9x_libretro.so");
    fs::write(&core, b"core bytes").unwrap();

    h.write_config_full(
        vec![entry("RetroArch", &exe, "-L \"%core%\" \"%rom%\"")],
        &[("SNES", "RetroArch")],
        &[("SNES", "snes9x")],
        "",
    );
    let rom = h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let session = service.launch(7).await.unwrap();

    let recorded = wait_until(|| record.is_file()).await;
    assert!(recorded, "the stub never wrote its argv");
    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    let expected_core = fs::canonicalize(&core).unwrap();
    assert_eq!(
        lines,
        vec![
            "-L",
            expected_core.to_string_lossy().as_ref(),
            rom.to_string_lossy().as_ref(),
        ]
    );

    service.stop(session.id);
}

#[tokio::test]
async fn global_launch_args_are_appended() {
    let h = Harness::new();
    let (exe, record) = h.recording_stub("stub-emu");
    h.write_config_full(
        vec![entry("Stub", &exe, "%rom%")],
        &[("SNES", "Stub")],
        &[],
        "-fullscreen",
    );
    let rom = h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let session = service.launch(7).await.unwrap();

    let recorded = wait_until(|| record.is_file()).await;
    assert!(recorded, "the stub never wrote its argv");
    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(lines, vec![rom.to_string_lossy().as_ref(), "-fullscreen"]);

    service.stop(session.id);
}

#[tokio::test]
async fn the_poll_loop_reaps_a_child_that_exits_on_its_own() {
    let h = Harness::new();
    // Outlives the 500 ms early-exit check, so only the poll loop can reap it.
    let exe = h.stub("short-runner", "sleep 1");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let recorder = Recorder::attach(&service);
    service.spawn_poll_loop();
    // A second call must not start a second loop.
    service.spawn_poll_loop();
    service.launch(7).await.unwrap();

    let gone = wait_until(|| service.snapshot().sessions.is_empty()).await;
    assert!(gone, "the exited child was never reaped");
    assert!(
        recorder.warnings().is_empty(),
        "the poll loop must not emit an early-exit warning"
    );
}
