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

    /// Registers an installed game like [`Self::install_game`], plus the two
    /// PS3 launch-target fields.
    fn install_ps3_game(
        &self,
        rom_id: i64,
        title: &str,
        platform: &str,
        ps3_game_id: &str,
        ps3_iso_path: &str,
    ) -> PathBuf {
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
                ps3_game_id: ps3_game_id.to_string(),
                ps3_iso_path: ps3_iso_path.to_string(),
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
        ..Default::default()
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
async fn snapshot_lists_sessions_newest_first() {
    // Mirrors QueueState::snapshot's newest-first convention (see
    // crates/grid-core/src/library/queue.rs).
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");
    h.install_game(8, "Turok", "SNES");

    let service = h.service();
    let first = service.launch(7).await.unwrap();
    let second = service.launch(8).await.unwrap();

    let sessions = service.snapshot().sessions;
    assert_eq!(sessions.len(), 2);
    let ids: Vec<u64> = sessions.iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![second.id, first.id]);

    service.stop(first.id);
    service.stop(second.id);
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
async fn a_siblings_check_still_reports_this_games_early_exit() {
    // Regression guard: the reap that observes an early exit is not always
    // that session's own check. A long-running game starts first, a second
    // game starts 400 ms later and dies at once, and the *first* game's
    // 500 ms check is what reaps it. The warning must still name the second
    // game's command line, and must be emitted exactly once.
    let h = Harness::new();
    let sleeper = h.stub("sleeper", "sleep 30");
    let quitter = h.stub("quitter", "exit 3");
    h.write_config(
        vec![
            entry("Sleeper", &sleeper, "%rom%"),
            entry("Quitter", &quitter, "%rom%"),
        ],
        &[("SNES", "Sleeper"), ("N64", "Quitter")],
    );
    h.install_game(7, "Chrono", "SNES");
    let dying_rom = h.install_game(8, "Turok", "N64");

    let service = h.service();
    let recorder = Recorder::attach(&service);
    let long_runner = service.launch(7).await.unwrap();

    // Land inside the long-runner's early-exit window, so its check at
    // t=500 ms is the reap that sees the second game's dead child.
    tokio::time::sleep(Duration::from_millis(400)).await;
    service.launch(8).await.unwrap();

    let warned = wait_until(|| !recorder.warnings().is_empty()).await;
    assert!(warned, "the early-exit warning was never emitted");
    // Give the dying game's own check (t=900 ms) time to run too, so a
    // duplicate would show up.
    tokio::time::sleep(Duration::from_millis(700)).await;

    let warnings = recorder.warnings();
    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one warning: {warnings:?}"
    );
    assert_eq!(
        warnings[0],
        format!(
            "Game exited immediately (code 3): {} {}",
            quitter.to_string_lossy(),
            dying_rom.to_string_lossy()
        )
    );

    // The long runner is untouched by its sibling's death.
    let sessions = service.snapshot().sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, long_runner.id);

    service.stop(long_runner.id);
}

#[tokio::test]
async fn a_stop_inside_the_early_exit_window_does_not_warn() {
    // A game the user stopped on purpose died "immediately", but that is not
    // a failure and must not be reported as one.
    let h = Harness::new();
    let exe = h.stub("sleeper", "sleep 30");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();
    let recorder = Recorder::attach(&service);
    let session = service.launch(7).await.unwrap();
    service.stop(session.id);

    let gone = wait_until(|| service.snapshot().sessions.is_empty()).await;
    assert!(gone, "the stopped session was never reaped");
    // Past the early-exit check, so a warning would have been emitted by now.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        recorder.warnings().is_empty(),
        "a deliberate stop must not warn: {:?}",
        recorder.warnings()
    );
}

#[tokio::test]
async fn a_running_poll_loop_does_not_swallow_the_early_exit_warning() {
    // Regression guard: the poll loop ticks every 50 ms here, well inside the
    // 500 ms early-exit window. Whichever path reaps the dead child, the
    // warning it owes the user has to survive.
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
async fn the_session_finished_hook_fires_once_per_reaped_session_after_the_snapshot() {
    // The cloud auto-upload trigger (installed by the app layer) must see
    // every reaped session exactly once, with the full `GameSession` record
    // — and only after that reap's snapshot(s) already reached the notify
    // listener (order asserted via a single shared log both callbacks
    // append to).
    let h = Harness::new();
    let exe = h.stub("quitter", "exit 3");
    h.write_config(vec![entry("Stub", &exe, "%rom%")], &[("SNES", "Stub")]);
    h.install_game(7, "Chrono", "SNES");

    let service = h.service();

    #[derive(Debug, Clone, PartialEq)]
    enum Event {
        Snapshot,
        Hook(u64, i64, String),
    }
    let log: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));

    let notify_log = log.clone();
    service.set_notify(Arc::new(move |_snapshot| {
        notify_log.lock().unwrap().push(Event::Snapshot);
    }));

    let hook_log = log.clone();
    service.set_session_finished_hook(Arc::new(move |session| {
        hook_log
            .lock()
            .unwrap()
            .push(Event::Hook(session.id, session.rom_id, session.title));
    }));

    // No poll loop: the 500 ms early-exit check alone must reap, notify,
    // and then fire the hook.
    let session = service.launch(7).await.unwrap();

    let fired = wait_until(|| {
        log.lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, Event::Hook(..)))
    })
    .await;
    assert!(fired, "the session-finished hook never fired");

    let events = log.lock().unwrap().clone();
    // Two snapshots precede the hook: `launch()`'s own registration emit,
    // then the early-exit check's warning emit — the hook must come after
    // BOTH, and must fire exactly once.
    assert_eq!(
        events,
        vec![
            Event::Snapshot,
            Event::Snapshot,
            Event::Hook(session.id, 7, "Chrono".to_string()),
        ],
        "the hook must fire exactly once, after the snapshot emit(s): {events:?}"
    );
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
async fn a_retroarch_relative_core_is_rewritten_to_an_absolute_path() {
    let h = Harness::new();
    let (exe, record) = h.recording_stub("retroarch");
    let cores = h.root.join("cores");
    fs::create_dir_all(&cores).unwrap();
    let core = cores.join("snes9x_libretro.so");
    fs::write(&core, b"core bytes").unwrap();

    // D-RC-1: the core gate now resolves against cores actually installed
    // on disk (grid_core::autoconfig::installed_compatible_cores), which
    // fuzzy-matches the platform name against the bundled RetroArch
    // compatibility map. That map only recognizes real platform names, not
    // the "SNES" shorthand used elsewhere in this file, so this one test
    // uses the full name.
    let platform = "Super Nintendo Entertainment System";
    h.write_config_full(
        vec![entry("RetroArch", &exe, "-L \"%core%\" \"%rom%\"")],
        &[(platform, "RetroArch")],
        &[(platform, "snes9x")],
        "",
    );
    let rom = h.install_game(7, "Chrono", platform);

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

#[tokio::test]
async fn ps3_launch_target_falls_back_to_the_gameid_placeholder() {
    let h = Harness::new();
    let (exe, record) = h.recording_stub("rpcs3");
    h.write_config(
        vec![entry("RPCS3", &exe, "%ps3_launch_target%")],
        &[("PS3", "RPCS3")],
    );
    h.install_ps3_game(7, "Demons Souls", "PS3", "BLUS30336", "");

    let service = h.service();
    let session = service.launch(7).await.unwrap();

    let recorded = wait_until(|| record.is_file()).await;
    assert!(recorded, "the stub never wrote its argv");
    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(lines, vec!["%RPCS3_GAMEID%:BLUS30336"]);

    service.stop(session.id);
}

#[tokio::test]
async fn ps3_launch_target_prefers_the_iso_path_when_set() {
    let h = Harness::new();
    let (exe, record) = h.recording_stub("rpcs3");
    h.write_config(
        vec![entry("RPCS3", &exe, "%ps3_launch_target%")],
        &[("PS3", "RPCS3")],
    );
    h.install_ps3_game(
        7,
        "Demons Souls",
        "PS3",
        "BLUS30336",
        "/isos/Demons Souls.iso",
    );

    let service = h.service();
    let session = service.launch(7).await.unwrap();

    let recorded = wait_until(|| record.is_file()).await;
    assert!(recorded, "the stub never wrote its argv");
    let argv = fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(lines, vec!["/isos/Demons Souls.iso"]);

    service.stop(session.id);
}
