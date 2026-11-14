//! RomM games whose archive holds a GOG offline installer (package B):
//! the game stays a RomM row (`source = romm`) with the installed game's
//! launch; the Downloads row shows the installer's phase; the failures
//! carry their codes; a run that needs its setup window keeps the
//! download, and a retry installs without downloading again; a DLC runs in
//! its base game's folder and prefix.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use grid_core::library::queue::{DownloadEntry, DownloadStatus, FailureCode, JobPhase};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::InstallService;
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use grid_core::stores::offline::{InstallerCommand, InstallerProcess, InstallerSpawner, Watch};
use grid_core::stores::GameSource;
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// --- fixtures ---------------------------------------------------------------

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        for &(name, bytes) in entries {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o755);
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

fn goggame(id: &str, root: &str, name: &str, exe: &str) -> String {
    format!(
        r#"{{"gameId": "{id}", "rootGameId": "{root}", "name": "{name}",
            "playTasks": [{{"category": "game", "isPrimary": true, "path": "{exe}"}}]}}"#
    )
}

/// A GOG Linux installer: a script (that never runs) with a zip appended.
fn linux_sh(id: &str, title: &str) -> Vec<u8> {
    let mut bytes = b"#!/bin/sh\necho never\nexit 1\n".to_vec();
    bytes.extend(zip_bytes(&[
        ("data/noarch/gameinfo", format!("{title}\n2.0\n").as_bytes()),
        ("data/noarch/start.sh", b"#!/bin/sh\n"),
        (
            &format!("data/noarch/game/goggame-{id}.info"),
            goggame(id, id, title, "orbit").as_bytes(),
        ),
        ("data/noarch/game/orbit", b"\x7fELF"),
    ]));
    bytes
}

/// A `setup_*.exe` GRID can only identify after it runs.
fn opaque_exe() -> Vec<u8> {
    let mut bytes = b"MZ".to_vec();
    bytes.resize(64, 0);
    bytes.extend_from_slice(&[
        b'r', b'D', b'l', b'P', b't', b'S', 0xCD, 0xE6, 0xD7, b'{', 0x0B, b'*',
    ]);
    bytes.resize(2048, 0);
    bytes
}

fn detail(
    id: i64,
    name: &str,
    platform: &str,
    slug: &str,
    file: &str,
    size: usize,
) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "fs_name_no_ext": name,
        "platform_id": 7,
        "platform_display_name": platform,
        "platform_slug": slug,
        "fs_name": file,
        "fs_size_bytes": size,
        "updated_at": "2026-01-01T00:00:00Z",
        "files": [{
            "id": id * 10,
            "file_name": file,
            "file_size_bytes": size,
            "is_top_level": true,
            "category": null,
        }],
    })
}

#[derive(Clone, Copy, Debug)]
enum Script {
    Installs,
    Hangs,
}

/// A scripted installer: writes `goggame-<id>.info` and `bin/game.exe`
/// into its `/DIR`, or hangs.
struct Fake {
    runs: Mutex<Vec<Script>>,
    info: (String, String),
    commands: Mutex<Vec<InstallerCommand>>,
}

impl Fake {
    fn new(runs: &[Script], id: &str, info: String) -> Arc<Fake> {
        Arc::new(Fake {
            runs: Mutex::new(runs.iter().rev().copied().collect()),
            info: (id.into(), info),
            commands: Mutex::new(Vec::new()),
        })
    }
}

fn host_path(arg: &str) -> PathBuf {
    match arg.strip_prefix("Z:") {
        Some(rest) => PathBuf::from(rest.replace('\\', "/")),
        None => PathBuf::from(arg),
    }
}

fn dir_arg(cmd: &InstallerCommand) -> PathBuf {
    host_path(
        cmd.argv
            .iter()
            .find_map(|arg| arg.strip_prefix("/DIR="))
            .unwrap(),
    )
}

struct Process(Option<Option<i32>>);

impl InstallerProcess for Process {
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
        Ok(self.0)
    }
    fn kill(&mut self) {
        self.0 = Some(None);
    }
}

impl InstallerSpawner for Fake {
    fn spawn(&self, cmd: &InstallerCommand) -> io::Result<Box<dyn InstallerProcess>> {
        self.commands.lock().unwrap().push(cmd.clone());
        let script = self.runs.lock().unwrap().pop().unwrap_or(Script::Hangs);
        Ok(Box::new(Process(match script {
            Script::Installs => {
                let dir = dir_arg(cmd);
                fs::create_dir_all(dir.join("bin")).unwrap();
                fs::write(dir.join("bin/game.exe"), b"MZ").unwrap();
                fs::write(
                    dir.join(format!("goggame-{}.info", self.info.0)),
                    &self.info.1,
                )
                .unwrap();
                Some(Some(0))
            }
            Script::Hangs => None,
        })))
    }
}

fn fast() -> Watch {
    Watch {
        stall: Duration::from_millis(60),
        poll: Duration::from_millis(5),
    }
}

// --- harness ----------------------------------------------------------------

struct Harness {
    server: MockServer,
    _tmp: tempfile::TempDir,
    library: PathBuf,
    service: Arc<InstallService>,
    client: Arc<RommClient>,
    registry: Arc<Registry>,
}

async fn harness() -> Harness {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let library = tmp.path().join("library");
    fs::create_dir_all(&library).unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        format!(
            "schema_version = 1\nserver_url = \"http://x\"\nusername = \"u\"\nlibrary_path = {:?}\ndefault_compat_tool = \"wine\"\n",
            library.to_string_lossy()
        ),
    )
    .unwrap();
    let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
    let service = InstallService::new(registry.clone(), config_path);
    let client = Arc::new(
        RommClient::new(
            &server.uri(),
            Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
        )
        .unwrap(),
    );
    Harness {
        server,
        _tmp: tmp,
        library,
        service,
        client,
        registry,
    }
}

impl Harness {
    /// Serves rom `id`; the content answers `downloads` times at most.
    async fn serve(
        &self,
        detail: serde_json::Value,
        file: &str,
        body: Vec<u8>,
    ) -> Arc<AtomicUsize> {
        let id = detail["id"].as_i64().unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(detail))
            .mount(&self.server)
            .await;
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{id}/content/{file}")))
            .respond_with(move |_: &wiremock::Request| {
                counter.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(body.clone())
            })
            .mount(&self.server)
            .await;
        hits
    }

    async fn install(&self, rom_id: i64) -> DownloadEntry {
        self.service
            .install(self.client.clone(), rom_id)
            .await
            .unwrap();
        let id = self.service.snapshot().entries.first().unwrap().id;
        self.wait_terminal(id).await
    }

    async fn wait_terminal(&self, id: u64) -> DownloadEntry {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(entry) = self
                .service
                .snapshot()
                .entries
                .into_iter()
                .find(|e| e.id == id)
            {
                if matches!(
                    entry.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                ) {
                    return entry;
                }
            }
            assert!(Instant::now() < deadline, "entry {id} never finished");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn home(&self, platform: &str, title: &str) -> PathBuf {
        self.library.join("games").join(platform).join(title)
    }

    fn row(&self, rom_id: i64) -> InstalledGame {
        self.registry
            .find(Some(rom_id), "", "")
            .unwrap()
            .expect("a row")
    }
}

// --- tests --------------------------------------------------------------------

#[tokio::test]
async fn a_linux_installer_archive_stays_a_romm_game() {
    let h = harness().await;
    let archive = zip_bytes(&[("paper_orbit_2_0_1.sh", &linux_sh("900", "Paper Orbit"))]);
    h.serve(
        detail(
            9,
            "Paper Orbit",
            "Linux",
            "linux",
            "paper_orbit.zip",
            archive.len(),
        ),
        "paper_orbit.zip",
        archive,
    )
    .await;
    let entry = h.install(9).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.phase, JobPhase::Unpacking);
    assert_eq!(entry.kind, "base", "no new kind badge: a game install");

    let home = h.home("Linux", "Paper Orbit");
    let game = home.join("game");
    let row = h.row(9);
    assert_eq!(row.source, GameSource::Romm, "the game stays a RomM game");
    assert_eq!(row.rom_id, Some(9));
    assert_eq!(row.store_id, "");
    assert_eq!(row.extracted_dir, game.to_string_lossy());
    assert_eq!(
        row.native_executable_path,
        game.join("start.sh").to_string_lossy()
    );
    assert!(game.join("game/orbit").is_file());
    assert!(
        !home.join("installer").exists(),
        "the extracted installer is gone"
    );
    assert!(
        !home.join("paper_orbit.zip").exists(),
        "the archive is gone"
    );
}

#[tokio::test]
async fn a_windows_installer_archive_runs_silently_into_the_game_folder() {
    let h = harness().await;
    let fake = Fake::new(
        &[Script::Installs],
        "910",
        goggame("910", "910", "Grey Meridian", "bin\\\\game.exe"),
    );
    h.service.set_installer_runner(fake.clone(), fast());
    let archive = zip_bytes(&[
        ("setup_grey_meridian_(1).exe", &opaque_exe()),
        ("setup_grey_meridian_(1)-1.bin", b"part"),
    ]);
    h.serve(
        detail(
            10,
            "Grey Meridian",
            "Windows",
            "win",
            "grey.zip",
            archive.len(),
        ),
        "grey.zip",
        archive,
    )
    .await;
    let entry = h.install(10).await;
    if cfg!(windows) {
        return;
    }
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.phase, JobPhase::InstallerSilent);
    let home = h.home("Windows", "Grey Meridian");
    let cmd = fake.commands.lock().unwrap()[0].clone();
    assert_eq!(dir_arg(&cmd), home.join("game"));
    assert!(cmd.argv.iter().any(|arg| arg == "/VERYSILENT"));
    assert!(cmd.env.contains(&(
        "WINEPREFIX".into(),
        home.join("prefix").to_string_lossy().into_owned()
    )));
    let row = h.row(10);
    assert_eq!(row.source, GameSource::Romm);
    assert_eq!(row.native_wineprefix, home.join("prefix").to_string_lossy());
    assert_eq!(
        row.native_executable_path,
        home.join("game/bin/game.exe").to_string_lossy()
    );
    assert!(!home.join("grey.zip").exists());
}

#[tokio::test]
async fn tv_mode_keeps_the_download_and_a_retry_does_not_download_again() {
    let h = harness().await;
    // TV mode: the silent run and the milder one both hang; the retry's
    // first run installs.
    let fake = Fake::new(
        &[Script::Hangs, Script::Hangs, Script::Installs],
        "920",
        goggame("920", "920", "Foxglove", "bin\\\\game.exe"),
    );
    h.service.set_installer_runner(fake.clone(), fast());
    h.service.set_wizard_allowed(Arc::new(|| false));
    let archive = zip_bytes(&[("setup_foxglove.exe", &opaque_exe())]);
    let hits = h
        .serve(
            detail(
                11,
                "Foxglove",
                "Windows",
                "win",
                "foxglove.zip",
                archive.len(),
            ),
            "foxglove.zip",
            archive,
        )
        .await;
    let entry = h.install(11).await;
    if cfg!(windows) {
        return;
    }
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(
        entry.error,
        "Couldn't run the installer silently: the installer stopped without saying why. The installer was kept."
    );
    assert!(!entry.error.contains("desktop"));
    let commands = fake.commands.lock().unwrap().clone();
    assert_eq!(commands.len(), 2, "two silent runs, no setup window");
    assert!(commands[0].argv.iter().any(|arg| arg == "/VERYSILENT"));
    assert!(commands[1].argv.iter().any(|arg| arg == "/SILENT"));
    assert!(!commands[1].argv.iter().any(|arg| arg == "/VERYSILENT"));
    let home = h.home("Windows", "Foxglove");
    assert!(home.join("foxglove.zip").is_file(), "the download was kept");
    assert!(!home.join("installer").exists());
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Retry runs it again from the kept download.
    h.service
        .retry(Some(h.client.clone()), entry.id)
        .await
        .unwrap();
    let id = h.service.snapshot().entries.first().unwrap().id;
    let entry = h.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(hits.load(Ordering::SeqCst), 1, "no second download");
    assert_eq!(h.row(11).source, GameSource::Romm);
}

#[tokio::test]
async fn delete_installer_removes_the_kept_download_and_the_row() {
    let h = harness().await;
    let fake = Fake::new(
        &[Script::Hangs, Script::Hangs],
        "921",
        goggame("921", "921", "Bramble", "bin\\\\game.exe"),
    );
    h.service.set_installer_runner(fake, fast());
    h.service.set_wizard_allowed(Arc::new(|| false));
    let archive = zip_bytes(&[("setup_bramble.exe", &opaque_exe())]);
    h.serve(
        detail(
            15,
            "Bramble",
            "Windows",
            "win",
            "bramble.zip",
            archive.len(),
        ),
        "bramble.zip",
        archive,
    )
    .await;
    let entry = h.install(15).await;
    if cfg!(windows) {
        return;
    }
    assert_eq!(entry.status, DownloadStatus::Failed);
    let home = h.home("Windows", "Bramble");
    assert!(home.join("bramble.zip").is_file());
    h.service.delete_installer(entry.id).unwrap();
    assert!(
        !home.join("bramble.zip").exists(),
        "the kept download is gone"
    );
    assert!(
        h.service
            .snapshot()
            .entries
            .iter()
            .all(|e| e.id != entry.id),
        "the row is dismissed"
    );
    assert!(h.service.delete_installer(entry.id).is_err());
}

#[tokio::test]
async fn an_unrecognised_installer_fails_and_removes_the_download() {
    let h = harness().await;
    let archive = zip_bytes(&[("setup.exe", b"#!/bin/sh\nnot a windows program\n")]);
    h.serve(
        detail(
            12,
            "Iron Lullaby",
            "Windows",
            "win",
            "iron.zip",
            archive.len(),
        ),
        "iron.zip",
        archive,
    )
    .await;
    let entry = h.install(12).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(
        entry.failure.unwrap().code,
        FailureCode::InstallerUnrecognised
    );
    assert_eq!(
        entry.error,
        "Its archive holds an installer GRID doesn't recognise (setup.exe has no GOG game info). The download was removed."
    );
    let home = h.home("Windows", "Iron Lullaby");
    assert!(!home.join("iron.zip").exists(), "the download was removed");
    assert!(h.registry.find(Some(12), "", "").unwrap().is_none());
}

#[tokio::test]
async fn a_dlc_without_its_base_fails_and_with_it_runs_in_the_base_prefix() {
    let h = harness().await;
    let dlc_info = goggame("931", "930", "Pale Summit - Low Tide Radio", "");
    let fake = Fake::new(&[Script::Installs, Script::Installs], "931", dlc_info);
    h.service.set_installer_runner(fake.clone(), fast());
    let archive = zip_bytes(&[("setup_low_tide_radio.exe", &opaque_exe())]);
    h.serve(
        detail(
            13,
            "Low Tide Radio",
            "Windows",
            "win",
            "ltr.zip",
            archive.len(),
        ),
        "ltr.zip",
        archive,
    )
    .await;
    let entry = h.install(13).await;
    if cfg!(windows) {
        return;
    }
    assert_eq!(entry.status, DownloadStatus::Failed, "{}", entry.error);
    let failure = entry.failure.clone().unwrap();
    assert_eq!(failure.code, FailureCode::InstallerDlcBaseMissing);
    assert_eq!(failure.base_title, "Pale Summit");
    assert_eq!(failure.base_gog_id, "930");
    let home = h.home("Windows", "Low Tide Radio");
    assert!(!home.join("ltr.zip").exists());
    assert!(!home.join("game").exists());

    // Pale Summit is installed (a RomM game that ran its own installer).
    let base_home = h.library.join("games/Windows/Pale Summit");
    let base_game = base_home.join("game");
    let base_prefix = base_home.join("prefix");
    fs::create_dir_all(base_game.join("bin")).unwrap();
    fs::write(base_game.join("bin/game.exe"), b"MZ").unwrap();
    fs::write(
        base_game.join("goggame-930.info"),
        goggame("930", "930", "Pale Summit", "bin\\\\game.exe"),
    )
    .unwrap();
    h.registry
        .upsert(&InstalledGame {
            rom_id: Some(14),
            title: "Pale Summit".into(),
            platform: "Windows".into(),
            platform_slug: "win".into(),
            native_game_dir: base_home.to_string_lossy().into_owned(),
            extracted_dir: base_game.to_string_lossy().into_owned(),
            native_executable_path: base_game
                .join("bin/game.exe")
                .to_string_lossy()
                .into_owned(),
            native_wineprefix: base_prefix.to_string_lossy().into_owned(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();

    h.service
        .retry(Some(h.client.clone()), entry.id)
        .await
        .unwrap();
    let id = h.service.snapshot().entries.first().unwrap().id;
    let entry = h.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    // Identity was only known after the run: a fresh folder first, then
    // its files moved into the base game's folder.
    assert!(base_game.join("goggame-931.info").is_file());
    assert!(!home.join("game").exists());
    let row = h.row(13);
    assert_eq!(row.source, GameSource::Romm);
    assert_eq!(
        row.native_executable_path,
        base_game.join("bin/game.exe").to_string_lossy()
    );
    assert_eq!(row.native_wineprefix, base_prefix.to_string_lossy());
    assert_eq!(
        row.extracted_dir,
        home.to_string_lossy(),
        "its own home, never the base's"
    );
}

/// A DLC known before the run (a Linux `.sh` carries its goggame file)
/// goes straight into its base game's folder.
#[tokio::test]
async fn a_known_linux_dlc_installs_into_its_base_game() {
    let h = harness().await;
    let base_home = h.library.join("games/Linux/Paper Orbit");
    let base_game = base_home.join("game");
    fs::create_dir_all(base_game.join("game")).unwrap();
    fs::write(base_game.join("start.sh"), b"#!/bin/sh\n").unwrap();
    fs::write(
        base_game.join("game/goggame-940.info"),
        goggame("940", "940", "Paper Orbit", "orbit"),
    )
    .unwrap();
    h.registry
        .upsert(&InstalledGame {
            rom_id: Some(15),
            title: "Paper Orbit".into(),
            platform: "Linux".into(),
            platform_slug: "linux".into(),
            native_game_dir: base_home.to_string_lossy().into_owned(),
            extracted_dir: base_game.to_string_lossy().into_owned(),
            native_executable_path: base_game.join("start.sh").to_string_lossy().into_owned(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();
    let mut dlc = b"#!/bin/sh\nexit 1\n".to_vec();
    dlc.extend(zip_bytes(&[(
        "data/noarch/game/goggame-941.info",
        goggame("941", "940", "Paper Orbit - Soundtrack", "").as_bytes(),
    )]));
    let archive = zip_bytes(&[("paper_orbit_soundtrack_1.sh", &dlc)]);
    h.serve(
        detail(
            16,
            "Paper Orbit Soundtrack",
            "Linux",
            "linux",
            "ost.zip",
            archive.len(),
        ),
        "ost.zip",
        archive,
    )
    .await;
    let entry = h.install(16).await;
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert!(base_game.join("game/goggame-941.info").is_file());
    let home = h.home("Linux", "Paper Orbit Soundtrack");
    assert!(!home.join("game").exists());
    let row = h.row(16);
    assert_eq!(row.source, GameSource::Romm);
    assert_eq!(
        row.native_executable_path,
        base_game.join("start.sh").to_string_lossy()
    );
}
