//! GOG offline installers (package B): reading and refusing installer
//! files, unpacking a Linux `.sh` without running it, a scripted Windows
//! installer (exit 0, needs the wizard, exit 1, cancelled), the offline
//! job's GOG rows, and a DLC that runs in its base game's prefix.
//!
//! Fixtures are built in the test: a `.sh` is a shell script with a zip
//! appended (like GOG's MojoSetup installers); a `.exe` GRID can't read
//! before the run carries only an Inno loader signature, so its identity
//! comes from the `goggame-*.info` the scripted installer writes.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grid_core::library::queue::{JobPhase, StoreOp, StoreProgressSink};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::LibraryError;
use grid_core::stores::gog::meta::{GogMeta, GogOrigin, GogPlatform};
use grid_core::stores::gog::GogJobCtx;
use grid_core::stores::offline::{
    self, detect_in_dir, inspect, install_job_with, run_into_with, Inspection, InstallerCommand,
    InstallerKind, InstallerProcess, InstallerSpawner, OfflineOpts, Verdict, Watch,
};
use grid_core::stores::{GameKey, GameSource, StoreKind};

// --- fixtures ---------------------------------------------------------------

const GAME_ID: &str = "1207660001";

fn goggame(id: &str, root: &str, name: &str, exe: &str) -> String {
    format!(
        r#"{{"gameId": "{id}", "rootGameId": "{root}", "name": "{name}", "buildId": "5501",
            "language": "English",
            "playTasks": [{{"category": "game", "isPrimary": true, "path": "{exe}",
                            "type": "FileTask"}}]}}"#
    )
}

/// A GOG-style Linux installer: a script that would leave `marker` behind
/// if it ever ran, with a zip of `entries` (`(name, bytes, unix mode)`)
/// appended after it.
fn write_sh(path: &Path, marker: &Path, entries: &[(&str, &[u8], u32)]) {
    let mut zip_bytes = io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut zip_bytes);
        for &(name, bytes, mode) in entries {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(mode);
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    let script = format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display());
    let mut file = fs::File::create(path).unwrap();
    file.write_all(script.as_bytes()).unwrap();
    file.write_all(&zip_bytes.into_inner()).unwrap();
}

fn linux_game_sh(dir: &Path, name: &str, id: &str, root: &str, title: &str) -> PathBuf {
    let path = dir.join(name);
    let info = goggame(id, root, title, "PaperLanterns");
    write_sh(
        &path,
        &dir.join("script-ran"),
        &[
            (
                "data/noarch/gameinfo",
                b"Paper Lanterns\n1.1.0\n2.0.0.1\n",
                0o644,
            ),
            (
                "data/noarch/start.sh",
                b"#!/bin/sh\nexec game/PaperLanterns\n",
                0o755,
            ),
            (
                &format!("data/noarch/game/goggame-{id}.info"),
                info.as_bytes(),
                0o644,
            ),
            ("data/noarch/game/PaperLanterns", b"\x7fELF", 0o755),
            ("data/noarch/game/data/level1.pak", b"level one", 0o644),
            ("scripts/config.lua", b"Setup.Package { id = \"x\" }", 0o644),
        ],
    );
    path
}

/// A `.exe` innospect can't read (an Inno loader signature and nothing
/// else): GRID runs it and learns its identity afterwards.
fn opaque_exe(path: &Path) {
    let mut bytes = b"MZ".to_vec();
    bytes.resize(64, 0);
    bytes.extend_from_slice(&[
        b'r', b'D', b'l', b'P', b't', b'S', 0xCD, 0xE6, 0xD7, b'{', 0x0B, b'*',
    ]);
    bytes.resize(4096, 0);
    fs::write(path, bytes).unwrap();
}

struct Env {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    library: PathBuf,
    registry: Arc<Registry>,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Downloads");
    fs::create_dir_all(&dir).unwrap();
    let library = tmp.path().join("library");
    let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
    Env {
        dir,
        library,
        registry,
        _tmp: tmp,
    }
}

impl Env {
    fn ctx(&self) -> GogJobCtx {
        GogJobCtx {
            registry: self.registry.clone(),
            library: self.library.clone(),
            default_compat_tool: "wine".into(),
        }
    }

    /// An installed GOG row for `id`, `platform`, in `dir`.
    fn seed_gog(&self, id: &str, title: &str, platform: GogPlatform, dir: &Path, prefix: &str) {
        fs::create_dir_all(dir).unwrap();
        let meta = GogMeta {
            platform,
            version: "1.0.4".into(),
            ..Default::default()
        };
        self.registry
            .upsert_store(&InstalledGame {
                title: title.into(),
                platform: "GOG".into(),
                platform_slug: platform.slug().into(),
                extracted_dir: dir.to_string_lossy().into_owned(),
                native_game_dir: dir.to_string_lossy().into_owned(),
                native_wineprefix: prefix.into(),
                source: GameSource::Store(StoreKind::Gog),
                store_id: id.into(),
                store_meta: meta.to_json(),
                installed_at: 1,
                ..Default::default()
            })
            .unwrap();
    }

    fn gog_row(&self, id: &str) -> Option<InstalledGame> {
        self.registry
            .find_key(&GameKey::store(StoreKind::Gog, id))
            .unwrap()
    }
}

#[derive(Default)]
struct Sink {
    phases: Vec<JobPhase>,
    progress: Vec<(u64, u64)>,
    cancel: Arc<AtomicBool>,
}

impl StoreProgressSink for Sink {
    fn progress(&mut self, done: u64, total: u64) {
        self.progress.push((done, total));
    }
    fn phase(&mut self, phase: JobPhase) {
        self.phases.push(phase);
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// How a scripted installer run behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Script {
    /// Writes the game (and its goggame file) into `/DIR`, exits 0.
    Installs,
    /// Exits with this code without writing anything.
    Exits(i32),
    /// Never ends and never writes (needs its setup window).
    Hangs,
}

/// A scripted installer: one [`Script`] per run, in order; it records
/// every command it was given.
struct FakeInstaller {
    runs: Mutex<Vec<Script>>,
    commands: Mutex<Vec<InstallerCommand>>,
    /// The goggame file a successful run writes.
    info: String,
    info_id: String,
    kills: Arc<AtomicUsize>,
}

impl FakeInstaller {
    fn new(runs: &[Script], id: &str, info: String) -> Arc<Self> {
        Arc::new(FakeInstaller {
            runs: Mutex::new(runs.iter().rev().copied().collect()),
            commands: Mutex::new(Vec::new()),
            info,
            info_id: id.into(),
            kills: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn commands(&self) -> Vec<InstallerCommand> {
        self.commands.lock().unwrap().clone()
    }
}

/// `/DIR=` of an argv, as a host path (`Z:\a\b` is `/a/b`).
fn dir_arg(argv: &[String]) -> PathBuf {
    path_arg(argv, "/DIR=")
}

/// The path argument after `flag`, as a host path.
fn path_arg(argv: &[String], flag: &str) -> PathBuf {
    let value = argv
        .iter()
        .find_map(|arg| arg.strip_prefix(flag))
        .unwrap_or_else(|| panic!("a {flag} argument"));
    match value.strip_prefix("Z:") {
        Some(rest) => PathBuf::from(rest.replace('\\', "/")),
        None => PathBuf::from(value),
    }
}

struct FakeProcess {
    end: Option<Option<i32>>,
    kills: Arc<AtomicUsize>,
}

impl InstallerProcess for FakeProcess {
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
        Ok(self.end)
    }
    fn kill(&mut self) {
        self.kills.fetch_add(1, Ordering::SeqCst);
        self.end = Some(None);
    }
}

impl InstallerSpawner for FakeInstaller {
    fn spawn(&self, cmd: &InstallerCommand) -> io::Result<Box<dyn InstallerProcess>> {
        self.commands.lock().unwrap().push(cmd.clone());
        let script = self.runs.lock().unwrap().pop().unwrap_or(Script::Exits(9));
        let end = match script {
            Script::Installs => {
                let dir = dir_arg(&cmd.argv);
                fs::create_dir_all(dir.join("bin")).unwrap();
                fs::write(dir.join("bin").join("game.exe"), b"MZ").unwrap();
                fs::write(
                    dir.join(format!("goggame-{}.info", self.info_id)),
                    &self.info,
                )
                .unwrap();
                Some(Some(0))
            }
            Script::Exits(code) => Some(Some(code)),
            Script::Hangs => None,
        };
        Ok(Box::new(FakeProcess {
            end,
            kills: self.kills.clone(),
        }))
    }
}

fn fast() -> Watch {
    Watch {
        stall: Duration::from_millis(60),
        poll: Duration::from_millis(5),
    }
}

/// An inspection of an opaque `.exe` (identity after the run), or with
/// `info` filled as innospect would have found it.
fn exe_inspection(env: &Env, name: &str, info: Option<&str>) -> Inspection {
    let path = env.dir.join(name);
    opaque_exe(&path);
    let mut insp = inspect(&path, &env.registry, &env.dir).unwrap();
    assert!(matches!(insp.kind, InstallerKind::WindowsExe { .. }));
    if let Some(info) = info {
        insp.info = grid_core::stores::gog::goggame::parse_info(info.as_bytes());
    }
    insp
}

fn run_job(
    env: &Env,
    insp: Inspection,
    opts: OfflineOpts,
    wizard: bool,
    spawner: Arc<FakeInstaller>,
) -> (Result<String, LibraryError>, Sink) {
    let job = install_job_with(
        insp,
        opts,
        env.ctx(),
        Arc::new(move || wizard),
        spawner,
        fast(),
    );
    assert_eq!(job.op, StoreOp::Installer);
    assert_eq!(job.source, StoreKind::Gog);
    job.runner
        .download(&AtomicBool::new(false), &mut |_, _, _| {})
        .unwrap();
    let mut sink = Sink::default();
    let result = job.runner.finalize(&mut sink);
    (result, sink)
}

// --- the Linux installer --------------------------------------------------------

#[test]
fn a_linux_installer_unpacks_into_a_gog_row_and_its_script_never_runs() {
    let env = env();
    let sh = linux_game_sh(
        &env.dir,
        "paper_lanterns_1_1_0_60321.sh",
        GAME_ID,
        GAME_ID,
        "Paper Lanterns",
    );
    let insp = inspect(&sh, &env.registry, &env.dir).unwrap();
    assert_eq!(insp.kind, InstallerKind::LinuxSh);
    assert_eq!(insp.version, "1.1.0");
    assert_eq!(insp.info.as_ref().unwrap().game_id, GAME_ID);
    assert_eq!(insp.bytes, fs::metadata(&sh).unwrap().len());
    if !cfg!(target_os = "linux") {
        assert!(matches!(insp.verdict, Verdict::Refused { .. }));
        return;
    }
    assert_eq!(insp.verdict, Verdict::Install);

    let spawner = FakeInstaller::new(&[], GAME_ID, String::new());
    let (result, sink) = run_job(&env, insp, OfflineOpts::default(), true, spawner.clone());
    result.unwrap();
    assert_eq!(sink.phases, vec![JobPhase::Unpacking]);
    assert_eq!(sink.progress.last().map(|(d, t)| d == t), Some(true));
    assert!(spawner.commands().is_empty(), "nothing is run for a .sh");
    assert!(!env.dir.join("script-ran").exists(), "the script never ran");

    let dest = env.library.join("GOG").join("Paper Lanterns");
    assert!(dest.join("game/data/level1.pak").is_file());
    assert!(!dest.join("scripts").exists(), "only data/noarch is copied");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(dest.join("start.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "start.sh stays executable");
    }

    let row = env.gog_row(GAME_ID).expect("a GOG row");
    assert_eq!(row.source, GameSource::Store(StoreKind::Gog));
    assert_eq!(row.platform_slug, "linux");
    assert_eq!(
        row.native_executable_path,
        dest.join("start.sh").to_string_lossy()
    );
    assert_eq!(row.native_wineprefix, "");
    let meta = GogMeta::from_json(&row.store_meta);
    assert_eq!(meta.origin, GogOrigin::Offline);
    assert_eq!(meta.version, "1.1.0");
    assert_eq!(meta.build_id, "5501");
    assert_eq!(meta.installer, vec![sh.to_string_lossy().into_owned()]);
    assert_eq!(meta.installer_bytes, fs::metadata(&sh).unwrap().len());
    assert!(row.filesize_bytes > 0);
}

#[test]
fn a_newer_linux_installer_updates_in_place_and_linux_dlc_joins_its_base() {
    let env = env();
    let dest = env.dir.join("installed");
    env.seed_gog(GAME_ID, "Paper Lanterns", GogPlatform::Linux, &dest, "");
    fs::write(dest.join("save.dat"), b"keep me").unwrap();

    let sh = linux_game_sh(
        &env.dir,
        "paper_lanterns_1_1_0_60321.sh",
        GAME_ID,
        GAME_ID,
        "Paper Lanterns",
    );
    let insp = inspect(&sh, &env.registry, &env.dir).unwrap();
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_eq!(
        insp.verdict,
        Verdict::Update {
            installed_version: "1.0.4".into()
        }
    );
    let spawner = FakeInstaller::new(&[], GAME_ID, String::new());
    run_job(&env, insp, OfflineOpts::default(), true, spawner.clone())
        .0
        .unwrap();
    assert!(dest.join("start.sh").is_file());
    assert_eq!(fs::read(dest.join("save.dat")).unwrap(), b"keep me");
    let row = env.gog_row(GAME_ID).unwrap();
    assert_eq!(row.title, "Paper Lanterns");
    assert_eq!(GogMeta::from_json(&row.store_meta).version, "1.1.0");

    // A Linux DLC for it.
    let dlc = linux_game_sh(
        &env.dir,
        "paper_lanterns_artbook_71299.sh",
        "1207660099",
        GAME_ID,
        "Paper Lanterns - Artbook",
    );
    let insp = inspect(&dlc, &env.registry, &env.dir).unwrap();
    assert_eq!(
        insp.verdict,
        Verdict::Dlc {
            base_title: "Paper Lanterns".into()
        }
    );
    run_job(&env, insp, OfflineOpts::default(), true, spawner)
        .0
        .unwrap();
    let meta = GogMeta::from_json(&env.gog_row(GAME_ID).unwrap().store_meta);
    assert_eq!(meta.dlc.len(), 1);
    assert_eq!(meta.dlc[0].id, "1207660099");
    assert_eq!(
        meta.dlc[0].installer,
        vec![dlc.to_string_lossy().into_owned()]
    );
    assert!(
        env.gog_row("1207660099").is_none(),
        "a DLC gets no row of its own"
    );
}

// --- refusals ---------------------------------------------------------------------

fn refusal_title(verdict: &Verdict) -> &str {
    match verdict {
        Verdict::Refused { title, .. } => title,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn the_refusal_table() {
    let env = env();
    let check = |name: &str, bytes: &[u8]| {
        let path = env.dir.join(name);
        fs::write(&path, bytes).unwrap();
        inspect(&path, &env.registry, &env.dir).unwrap()
    };

    let mac = check("ashen_tide_1.0.4_71234.pkg", b"xar!");
    assert_eq!(mac.kind, InstallerKind::MacPkg);
    assert_eq!(refusal_title(&mac.verdict), "This is a macOS installer");

    for (name, bytes) in [
        ("manual_ashen_tide.pdf", &b"%PDF-1.4"[..]),
        ("not_a_zip.sh", &b"#!/bin/sh\necho hi\n"[..]),
        ("setup.exe", &b"#!/bin/sh\n"[..]),
    ] {
        let insp = check(name, bytes);
        assert_eq!(
            refusal_title(&insp.verdict),
            "This isn't a GOG installer",
            "{name}"
        );
        let Verdict::Refused { body, .. } = &insp.verdict else {
            unreachable!()
        };
        assert!(
            body.starts_with(&format!("{name} has no GOG game info")),
            "{body}"
        );
    }

    // A .sh with data/noarch but no GOG identity.
    let path = env.dir.join("nameless.sh");
    write_sh(&path, &env.dir.join("m"), &[("data/noarch/x", b"x", 0o644)]);
    let insp = inspect(&path, &env.registry, &env.dir).unwrap();
    assert_eq!(refusal_title(&insp.verdict), "This isn't a GOG installer");

    // A multi-part installer with a gap in its parts.
    let setup = env.dir.join("setup_marrowind_keep_1.2_(64bit)_(70555).exe");
    opaque_exe(&setup);
    fs::write(
        env.dir
            .join("setup_marrowind_keep_1.2_(64bit)_(70555)-2.bin"),
        b"2",
    )
    .unwrap();
    let insp = inspect(&setup, &env.registry, &env.dir).unwrap();
    assert_eq!(
        refusal_title(&insp.verdict),
        "Part of this installer is missing"
    );
    let Verdict::Refused { body, .. } = &insp.verdict else {
        unreachable!()
    };
    assert!(
        body.contains("setup_marrowind_keep_1.2_(64bit)_(70555)-1.bin isn't in the same folder"),
        "{body}"
    );
    fs::write(
        env.dir
            .join("setup_marrowind_keep_1.2_(64bit)_(70555)-1.bin"),
        b"1",
    )
    .unwrap();
    let insp = inspect(&setup, &env.registry, &env.dir).unwrap();
    assert_eq!(
        insp.verdict,
        Verdict::Install,
        "identity only after the run"
    );
    let InstallerKind::WindowsExe { parts } = &insp.kind else {
        unreachable!()
    };
    assert_eq!(parts.len(), 2);
    assert_eq!(insp.bytes, 4096 + 2);
    assert_eq!(insp.version, "1.2", "from the file name");
    assert_eq!(insp.title(), "Marrowind Keep");
}

#[test]
fn registry_refusals_other_os_and_dlc_without_or_with_the_wrong_base() {
    let env = env();
    // Ashen Tide is installed as the Linux version.
    env.seed_gog(
        "100",
        "Ashen Tide",
        GogPlatform::Linux,
        &env.dir.join("at"),
        "",
    );
    let mut windows = exe_inspection(
        &env,
        "setup_ashen_tide.exe",
        Some(&goggame("100", "100", "Ashen Tide", "a.exe")),
    );
    windows.verdict = Verdict::Install;
    let verdict = offline_decide(&env, &windows);
    assert_eq!(
        refusal_title(&verdict),
        "Ashen Tide is installed here as the Linux version"
    );
    let Verdict::Refused { body, .. } = &verdict else {
        unreachable!()
    };
    assert_eq!(
        body,
        "This is the Windows version's installer. To switch, uninstall Ashen Tide first, then open this installer again."
    );

    // Windows DLC for the Linux game.
    let dlc = exe_inspection(
        &env,
        "setup_ashen_tide_dlc.exe",
        Some(&goggame("101", "100", "Ashen Tide - Artbook", "")),
    );
    let verdict = offline_decide(&env, &dlc);
    assert_eq!(
        refusal_title(&verdict),
        "Ashen Tide is installed here as the Linux version"
    );
    let Verdict::Refused { body, .. } = &verdict else {
        unreachable!()
    };
    assert!(
        body.contains("A Linux game takes Linux DLC installers only."),
        "{body}"
    );

    // DLC for a game that isn't installed.
    let orphan = exe_inspection(
        &env,
        "setup_tidewater_tales_dlc_(55120).exe",
        Some(&goggame("201", "200", "Tidewater Tales - Soundtrack", "")),
    );
    let verdict = offline_decide(&env, &orphan);
    assert_eq!(
        refusal_title(&verdict),
        "This is DLC for Tidewater Tales, which isn't installed"
    );
}

/// The verdict `inspect` gives an inspection whose info a test filled in.
fn offline_decide(env: &Env, insp: &Inspection) -> Verdict {
    // `inspect` decides from the file; re-decide with the test's info by
    // running the job's own check through a refused finalize.
    let job = install_job_with(
        insp.clone(),
        OfflineOpts::default(),
        env.ctx(),
        Arc::new(|| true),
        FakeInstaller::new(&[], "0", String::new()),
        fast(),
    );
    let mut sink = Sink::default();
    match job.runner.finalize(&mut sink) {
        Err(LibraryError::InstallerRefused(message)) => {
            let (title, body) = message.split_once(". ").unwrap();
            Verdict::Refused {
                title: title.into(),
                body: body.into(),
            }
        }
        Err(LibraryError::InstallerDlcBaseMissing { base_title, .. }) => Verdict::Refused {
            title: format!("This is DLC for {base_title}, which isn't installed"),
            body: String::new(),
        },
        other => panic!("not refused: {other:?}"),
    }
}

// --- the Windows installer ---------------------------------------------------------

#[test]
fn a_silent_windows_install_reads_its_identity_after_the_run() {
    let env = env();
    let insp = exe_inspection(&env, "setup_quarry_nine_1.4_(64bit)_(69001).exe", None);
    assert_eq!(insp.verdict, Verdict::Install);
    let info = goggame("300", "300", "Quarry Nine", "bin\\\\game.exe");
    let spawner = FakeInstaller::new(&[Script::Installs], "300", info);
    let dest = env.dir.join("Q9");
    // `wine`, so no umu-run is needed here (the umu argv is
    // `launch::native`'s unit test).
    let opts = OfflineOpts {
        install_dir: Some(dest.clone()),
        compat_tool: "wine".into(),
    };
    let (result, sink) = run_job(&env, insp, opts, false, spawner.clone());
    result.unwrap();
    if cfg!(windows) {
        // Native run: no Wine, no prefix.
        return;
    }
    assert_eq!(sink.phases, vec![JobPhase::InstallerSilent]);
    let commands = spawner.commands();
    let cmd = &commands[0];
    assert!(cmd.argv.contains(&"/VERYSILENT".to_string()));
    assert_eq!(dir_arg(&cmd.argv), dest);
    let prefix = offline::default_prefix(&dest);
    assert_eq!(
        prefix,
        dest.join("prefix"),
        "every GOG prefix is <install dir>/prefix"
    );
    assert!(cmd
        .env
        .contains(&("WINEPREFIX".into(), prefix.to_string_lossy().into_owned())));
    let log = path_arg(&cmd.argv, "/LOG=");
    assert!(log.ends_with("grid-install.log"), "{log:?}");
    assert!(
        !log.starts_with(&dest),
        "the log is kept out of the game folder"
    );

    let row = env.gog_row("300").expect("a GOG row");
    assert_eq!(row.platform_slug, "win");
    assert_eq!(row.native_compat_tool, "wine");
    assert_eq!(row.native_wineprefix, prefix.to_string_lossy());
    assert_eq!(
        row.native_executable_path,
        dest.join("bin").join("game.exe").to_string_lossy()
    );
    assert_eq!(
        GogMeta::from_json(&row.store_meta).origin,
        GogOrigin::Offline
    );
}

/// Runs a Windows installer straight through `run_into_with` with `wine`
/// (no umu needed) and returns the result, the commands and the phases.
fn run_wine(
    runs: &[Script],
    wizard_allowed: bool,
    cancel: bool,
) -> (
    Result<String, LibraryError>,
    Vec<InstallerCommand>,
    Vec<JobPhase>,
    usize,
    PathBuf,
) {
    let env = env();
    let insp = exe_inspection(&env, "setup_foxglove.exe", None);
    let info = goggame("400", "400", "Foxglove", "fox.exe");
    let spawner = FakeInstaller::new(runs, "400", info);
    let dest = env.dir.join("Foxglove");
    let prefix = env.dir.join("prefix");
    let mut sink = Sink::default();
    sink.cancel.store(cancel, Ordering::SeqCst);
    let result = run_into_with(
        &insp,
        &dest,
        Some(&prefix),
        "wine",
        wizard_allowed,
        &mut sink,
        spawner.as_ref(),
        fast(),
    )
    .map(|info| info.game_id);
    let kills = spawner.kills.load(Ordering::SeqCst);
    (result, spawner.commands(), sink.phases, kills, dest)
}

#[test]
fn a_stalled_silent_run_falls_back_to_the_wizard_on_the_desktop() {
    let (result, commands, phases, kills, _) =
        run_wine(&[Script::Hangs, Script::Installs], true, false);
    assert_eq!(result.unwrap(), "400");
    assert_eq!(
        phases,
        vec![JobPhase::InstallerSilent, JobPhase::InstallerWizard]
    );
    assert_eq!(kills, 1, "the stalled run was killed");
    assert!(commands[0].argv.contains(&"/VERYSILENT".to_string()));
    assert!(!commands[1].argv.contains(&"/VERYSILENT".to_string()));
    assert_eq!(dir_arg(&commands[1].argv), dir_arg(&commands[0].argv));
}

#[test]
fn tv_mode_never_opens_the_wizard_and_keeps_the_installer() {
    for first in [Script::Hangs, Script::Exits(1), Script::Exits(7)] {
        let (result, commands, phases, _, _) = run_wine(&[first], false, false);
        assert!(
            matches!(
                result,
                Err(LibraryError::InstallerNeedsWizard { kept: true })
            ),
            "{first:?}: {result:?}"
        );
        assert_eq!(commands.len(), 1, "no wizard run");
        assert_eq!(phases, vec![JobPhase::InstallerSilent]);
    }
}

#[test]
fn installer_errors_cancels_and_a_missing_goggame() {
    let (result, ..) = run_wine(&[Script::Exits(6)], true, false);
    assert!(
        matches!(result, Err(LibraryError::InstallerExit(6))),
        "{result:?}"
    );
    // The wizard itself fails.
    let (result, ..) = run_wine(&[Script::Exits(1), Script::Exits(1)], true, false);
    assert!(
        matches!(result, Err(LibraryError::InstallerExit(1))),
        "{result:?}"
    );
    // The user cancelled the installer (exit 2), or GRID's cancel.
    let (result, ..) = run_wine(&[Script::Exits(2)], true, false);
    assert!(matches!(result, Err(LibraryError::Cancelled)), "{result:?}");
    let (result, _, _, kills, _) = run_wine(&[Script::Hangs], true, true);
    assert!(matches!(result, Err(LibraryError::Cancelled)), "{result:?}");
    assert_eq!(kills, 1);
    // Exit 0 but no game info in the folder.
    let (result, ..) = run_wine(&[Script::Exits(0)], true, false);
    assert!(
        matches!(&result, Err(LibraryError::InstallerUnrecognised { file }) if file == "setup_foxglove.exe"),
        "{result:?}"
    );
}

#[test]
fn a_failed_fresh_install_leaves_nothing_behind() {
    let env = env();
    let insp = exe_inspection(&env, "setup_iron.exe", None);
    let dest = env.dir.join("Iron");
    let spawner = FakeInstaller::new(&[Script::Exits(6)], "500", String::new());
    let opts = OfflineOpts {
        install_dir: Some(dest.clone()),
        compat_tool: "wine".into(),
    };
    let (result, _) = run_job(&env, insp, opts, true, spawner);
    assert!(
        matches!(result, Err(LibraryError::InstallerExit(6))),
        "{result:?}"
    );
    assert!(!dest.exists());
    assert!(!offline::default_prefix(&dest).exists());
    assert!(env.registry.store_rows(StoreKind::Gog).unwrap().is_empty());
    assert!(
        env.dir.join("setup_iron.exe").is_file(),
        "the user's installer stays"
    );
}

#[test]
fn a_windows_dlc_runs_in_its_base_games_folder_and_prefix() {
    let env = env();
    let base_dir = env.dir.join("Harbor Lights");
    let base_prefix = env.dir.join("hl-prefix");
    env.seed_gog(
        "600",
        "Harbor Lights",
        GogPlatform::Windows,
        &base_dir,
        &base_prefix.to_string_lossy(),
    );
    let dlc_info = goggame("601", "600", "Harbor Lights - Ashgrove", "");
    let insp = exe_inspection(&env, "setup_ashgrove.exe", Some(&dlc_info));
    let spawner = FakeInstaller::new(&[Script::Installs], "601", dlc_info.clone());
    let (result, _) = run_job(
        &env,
        insp,
        OfflineOpts {
            install_dir: None,
            compat_tool: "wine".into(),
        },
        true,
        spawner.clone(),
    );
    if cfg!(windows) {
        result.unwrap();
        return;
    }
    result.unwrap();
    let cmd = &spawner.commands()[0];
    assert_eq!(dir_arg(&cmd.argv), base_dir, "the base game's /DIR");
    assert_eq!(cmd.prefix.as_deref(), Some(base_prefix.as_path()));
    assert!(cmd.env.contains(&(
        "WINEPREFIX".into(),
        base_prefix.to_string_lossy().into_owned()
    )));
    let meta = GogMeta::from_json(&env.gog_row("600").unwrap().store_meta);
    assert_eq!(meta.dlc.len(), 1);
    assert_eq!(meta.dlc[0].title, "Harbor Lights - Ashgrove");
    assert!(env.gog_row("601").is_none());
}

#[test]
fn an_unknown_exe_that_turns_out_to_be_dlc_joins_its_base_or_fails() {
    let env = env();
    let dlc_info = goggame("701", "700", "Pale Summit - Low Tide Radio", "");
    // No base: the fresh folder is removed and the card offers the base.
    let insp = exe_inspection(&env, "setup_low_tide_radio.exe", None);
    let dest = env.dir.join("LTR");
    let spawner = FakeInstaller::new(&[Script::Installs], "701", dlc_info.clone());
    let opts = OfflineOpts {
        install_dir: Some(dest.clone()),
        compat_tool: "wine".into(),
    };
    let (result, _) = run_job(&env, insp.clone(), opts.clone(), true, spawner);
    match result {
        Err(LibraryError::InstallerDlcBaseMissing {
            base_title,
            base_gog_id,
        }) => {
            assert_eq!(base_title, "Pale Summit");
            assert_eq!(base_gog_id, "700");
        }
        other => panic!("{other:?}"),
    }
    assert!(!dest.exists());

    // With the base installed, its files move into the base's folder.
    let base_dir = env.dir.join("Pale Summit");
    env.seed_gog("700", "Pale Summit", GogPlatform::Windows, &base_dir, "");
    let spawner = FakeInstaller::new(&[Script::Installs], "701", dlc_info);
    let (result, _) = run_job(&env, insp, opts, true, spawner);
    result.unwrap();
    assert!(base_dir.join("goggame-701.info").is_file());
    assert!(!dest.exists());
    let meta = GogMeta::from_json(&env.gog_row("700").unwrap().store_meta);
    assert_eq!(meta.dlc[0].id, "701");
}

// --- RomM archives ------------------------------------------------------------

#[test]
fn detect_in_dir_finds_installers_and_leaves_loose_games_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let make = |name: &str, files: &[&str]| {
        let dir = tmp.path().join(name);
        for file in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            if file.ends_with(".exe") {
                opaque_exe(&path);
            } else {
                fs::write(&path, b"x").unwrap();
            }
        }
        dir
    };
    let exe = make(
        "a",
        &[
            "Wrapper/setup_grey_meridian_(1).exe",
            "Wrapper/setup_grey_meridian_(1)-1.bin",
            "Wrapper/readme.txt",
        ],
    );
    assert_eq!(
        detect_in_dir(&exe),
        Some(exe.join("Wrapper").join("setup_grey_meridian_(1).exe"))
    );
    let plain = make("b", &["setup.exe"]);
    assert_eq!(detect_in_dir(&plain), Some(plain.join("setup.exe")));
    assert_eq!(detect_in_dir(&make("c", &["game.exe", "data/x.pak"])), None);
    assert_eq!(
        detect_in_dir(&make("d", &["setup_x.exe", "data/x.pak"])),
        None
    );
    assert_eq!(
        detect_in_dir(&make("e", &["setup_x.exe", "game.dll"])),
        None
    );
    assert_eq!(
        detect_in_dir(&make("f", &["run.sh", "notes.txt"])),
        None,
        "a script that isn't a GOG installer"
    );

    let g = tmp.path().join("g");
    fs::create_dir_all(&g).unwrap();
    let sh = linux_game_sh(&g, "paper_orbit_1_0_1.sh", "9", "9", "Paper Orbit");
    let _ = fs::remove_file(g.join("script-ran"));
    assert_eq!(detect_in_dir(&g), Some(sh));
}
