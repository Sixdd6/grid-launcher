//! A build from the RomM server's Emulators platform, packaged to the
//! standard (`docs/server-emulator-packages.md`), end to end through
//! `InstallService::install_server_emulator`: only the chosen file is
//! downloaded, then it is kept or unpacked under `emulators/<entry>` and
//! registered with its server source and version. Runs against a wiremock
//! RomM server and a tempdir library. Unix only: the stub executables are
//! shell scripts.
#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use grid_core::config::{Config, EmulatorEntry};
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::{EmulatorInstalled, InstallService, ServerEmulatorRequest};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PCSX2: &str = "PCSX2 (Playstation 2)";
const APPIMAGE: &[u8] = b"#!/bin/sh\nexit 0\n";

struct Harness {
    server: MockServer,
    _tmp: tempfile::TempDir,
    library: PathBuf,
    config_path: PathBuf,
    service: Arc<InstallService>,
    client: Arc<RommClient>,
    installed: Arc<Mutex<Vec<EmulatorInstalled>>>,
}

impl Harness {
    async fn new(extra_config: &str) -> Self {
        let server = MockServer::start().await;
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        fs::create_dir_all(&library).unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(
            &config_path,
            format!(
                "schema_version = 1\nserver_url = \"http://x\"\nusername = \"u\"\nlibrary_path = {:?}\n{extra_config}",
                library.to_string_lossy()
            ),
        )
        .unwrap();
        let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
        let service = InstallService::new(registry, config_path.clone());
        service.set_known_platforms(vec!["Sony PlayStation 2".to_string(), "Pico-8".to_string()]);
        let installed: Arc<Mutex<Vec<EmulatorInstalled>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = installed.clone();
        service
            .set_emulator_installed_hook(Arc::new(move |event| sink.lock().unwrap().push(event)));
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
            config_path,
            service,
            client,
            installed,
        }
    }

    /// Serves `bytes` for one file of ROM `rom_id`, and expects exactly
    /// `times` requests for it.
    async fn serve_file(&self, rom_id: i64, file_id: i64, bytes: Vec<u8>, times: u64) {
        Mock::given(method("GET"))
            .and(path_regex(format!("^/api/roms/{rom_id}/content/.+$")))
            .and(query_param("file_ids", file_id.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
            .expect(times)
            .mount(&self.server)
            .await;
    }

    async fn install(&self, request: ServerEmulatorRequest) -> DownloadEntry {
        self.service
            .install_server_emulator(self.client.clone(), request)
            .await
            .unwrap();
        let id = self.service.snapshot().entries.first().unwrap().id;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let entry = self
                .service
                .snapshot()
                .entries
                .into_iter()
                .find(|e| e.id == id)
                .unwrap();
            if matches!(
                entry.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            ) {
                assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
                return entry;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting on the install"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn entry(&self, name: &str) -> EmulatorEntry {
        Config::load(&self.config_path)
            .unwrap()
            .emulators
            .into_iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("no emulator entry named {name}"))
    }
}

fn pcsx2(version: &str, file_id: i64) -> ServerEmulatorRequest {
    ServerEmulatorRequest {
        rom_id: 700,
        file_id,
        file_name: format!("PCSX2 (v{version}) (arch-linux).AppImage"),
        version: version.to_string(),
        folder: "ps2".to_string(),
        profile_name: PCSX2.to_string(),
    }
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for &(name, content) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }
    cursor.into_inner()
}

#[tokio::test]
async fn only_the_chosen_build_is_downloaded_and_it_registers_with_its_source() {
    let harness = Harness::new("").await;
    harness.serve_file(700, 72, APPIMAGE.to_vec(), 1).await;
    // The Windows build of the same folder must never be requested.
    harness.serve_file(700, 71, b"MZ".to_vec(), 0).await;

    let row = harness.install(pcsx2("2.0.2", 72)).await;
    assert_eq!(
        row.kind, "emulator",
        "the Downloads row carries the Emulator badge"
    );
    assert_eq!(row.source_id, "romm:ps2/pcsx2 (playstation 2)");

    let entry = harness.entry(PCSX2);
    let expected = harness
        .library
        .join("emulators")
        .join(PCSX2)
        .join("PCSX2 (v2.0.2) (arch-linux).AppImage");
    assert_eq!(PathBuf::from(&entry.path), expected);
    assert_eq!(entry.source_id, "romm:ps2/pcsx2 (playstation 2)");
    assert_eq!(entry.source_provider, "romm");
    assert_eq!(entry.source_repo, "ps2");
    assert_eq!(entry.source_installed_tag, "2.0.2");

    let events = harness.installed.lock().unwrap().clone();
    assert_eq!(
        events,
        [EmulatorInstalled {
            name: PCSX2.to_string(),
            fresh: true,
            compat_tool: false,
            server_source: Some("romm:ps2/pcsx2 (playstation 2)".to_string()),
        }]
    );
}

#[tokio::test]
async fn a_catalog_entry_keeps_its_name_and_the_server_build_takes_the_suffix() {
    let harness = Harness::new(&format!(
        "\n[[emulators]]\nname = {PCSX2:?}\npath = \"/opt/pcsx2/catalog.AppImage\"\nargs = \"%rom%\"\nsource_id = \"PCSX2/pcsx2\"\n"
    ))
    .await;
    harness.serve_file(700, 72, APPIMAGE.to_vec(), 1).await;

    harness.install(pcsx2("2.0.2", 72)).await;

    assert_eq!(harness.entry(PCSX2).path, "/opt/pcsx2/catalog.AppImage");
    let server = harness.entry(&format!("{PCSX2} (server)"));
    assert_eq!(server.source_id, "romm:ps2/pcsx2 (playstation 2)");
}

#[tokio::test]
async fn a_newer_version_updates_the_same_entry() {
    let harness = Harness::new("").await;
    harness.serve_file(700, 72, APPIMAGE.to_vec(), 1).await;
    harness.serve_file(700, 73, APPIMAGE.to_vec(), 1).await;

    harness.install(pcsx2("2.0.2", 72)).await;
    harness
        .service
        .dismiss(harness.service.snapshot().entries[0].id);
    harness.install(pcsx2("2.1.0", 73)).await;

    let config = Config::load(&harness.config_path).unwrap();
    let ours: Vec<&EmulatorEntry> = config
        .emulators
        .iter()
        .filter(|e| e.source_id == "romm:ps2/pcsx2 (playstation 2)")
        .collect();
    assert_eq!(ours.len(), 1, "one entry, updated in place");
    assert_eq!(ours[0].name, PCSX2);
    assert_eq!(ours[0].source_installed_tag, "2.1.0");
}

#[tokio::test]
async fn a_zipped_build_is_unpacked_and_its_linux_executable_registered() {
    let harness = Harness::new("").await;
    let zip = zip_bytes(&[
        ("pico-8/pico8", b"#!/bin/sh\nexit 0\n"),
        ("pico-8/pico8.dat", b"data"),
        ("pico-8/readme.txt", b"read me"),
    ]);
    harness.serve_file(3432, 1, zip, 1).await;

    harness
        .install(ServerEmulatorRequest {
            rom_id: 3432,
            file_id: 1,
            file_name: "Pico-8 (v0.2.7a6) (arch-linux).zip".to_string(),
            version: "0.2.7a6".to_string(),
            folder: "pico".to_string(),
            profile_name: "Pico-8".to_string(),
        })
        .await;

    let entry = harness.entry("Pico-8");
    assert!(entry.path.ends_with("/pico8"), "{}", entry.path);
    assert_eq!(entry.source_id, "romm:pico/pico-8");
    assert_eq!(entry.source_installed_tag, "0.2.7a6");
}
