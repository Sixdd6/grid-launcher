//! A server "Emulators"-platform package, end to end through `InstallService`:
//! download and extract like a game, then register, link and configure the
//! emulator it carries (plan B7 / Q3). Runs against a wiremock RomM server and
//! a tempdir library.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use grid_core::config::{Config, EmulatorEntry};
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::{EmulatorInstalled, InstallService};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PCSX2: &str = "PCSX2 (Playstation 2)";
const PS2: &str = "Sony PlayStation 2";
const WIN_EXE: &str = "PCSX2/win/pcsx2-qt.exe";
const LINUX_EXE: &str = "PCSX2/linux/pcsx2-v2.4.0-linux-appimage-x64-Qt.AppImage";

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
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

/// A package with a Windows and a Linux build of PCSX2 side by side.
fn pcsx2_package() -> Vec<u8> {
    zip_bytes(&[
        (WIN_EXE, b"MZ stub"),
        (LINUX_EXE, b"#!/bin/sh\nexit 0\n"),
        ("PCSX2/readme.txt", b"read me"),
    ])
}

fn detail(id: i64, name: &str, fs_name: &str, size: usize) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "fs_name_no_ext": name,
        "platform_id": 99,
        "platform_display_name": "Emulators",
        "fs_name": fs_name,
        "summary": "",
        "regions": [],
        "languages": [],
        "tags": [],
        "revision": null,
        "fs_size_bytes": size,
        "updated_at": "2026-01-01T00:00:00Z",
        "files": [{
            "id": id * 10,
            "file_name": fs_name,
            "file_size_bytes": size,
            "is_top_level": true,
            "category": null,
        }],
    })
}

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
        service.set_known_platforms(vec![PS2.to_string(), "Linux".to_string()]);
        let installed: Arc<Mutex<Vec<EmulatorInstalled>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = installed.clone();
        service
            .set_emulator_installed_hook(Arc::new(move |event| sink.lock().unwrap().push(event)));
        let client = Arc::new(RommClient::new(&server.uri(), token_cred()).unwrap());
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

    async fn mount(&self, id: i64, name: &str, fs_name: &str, bytes: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(detail(
                id,
                name,
                fs_name,
                bytes.len(),
            )))
            .mount(&self.server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{id}/content/{fs_name}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
            .mount(&self.server)
            .await;
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
            assert!(Instant::now() < deadline, "timed out waiting on entry {id}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn install(&self, rom_id: i64) -> DownloadEntry {
        self.service
            .install(self.client.clone(), rom_id)
            .await
            .unwrap();
        let id = self.service.snapshot().entries.first().unwrap().id;
        let entry = self.wait_terminal(id).await;
        assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
        entry
    }

    fn config(&self) -> Config {
        Config::load(&self.config_path).unwrap()
    }

    fn entry(&self, name: &str) -> EmulatorEntry {
        self.config()
            .emulators
            .into_iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("no emulator entry named {name}"))
    }

    fn package_dir(&self, stem: &str) -> PathBuf {
        self.library.join("games").join("Emulators").join(stem)
    }

    /// The executable this host's build of the package should register.
    fn expected_exe(&self, stem: &str) -> PathBuf {
        let relative = if cfg!(windows) { WIN_EXE } else { LINUX_EXE };
        relative
            .split('/')
            .fold(self.package_dir(stem), |dir, part| dir.join(part))
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn installing_a_known_package_registers_links_and_configures_it() {
    let harness = Harness::new("").await;
    harness
        .mount(1, "PCSX2", "pcsx2-pkg.zip", pcsx2_package())
        .await;

    let entry = harness.install(1).await;
    assert_eq!(
        entry.kind, "emulator",
        "the Downloads row carries the Emulator badge"
    );
    assert_eq!(entry.job, "game");

    // Registered under the profile's name, with the host's own build and the
    // profile's args.
    let exe = harness.expected_exe("pcsx2-pkg");
    let written = harness.entry(PCSX2);
    assert_eq!(written.path, path_text(&exe));
    assert_eq!(written.args, "-portable -fullscreen -batch \"%rom%\"");
    assert_eq!(
        written.source_id, "",
        "a server package has no forge source"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(fs::metadata(&exe).unwrap().permissions().mode() & 0o111, 0);
    }

    // Autoconfig ran: the profile's save fields, the defaults backfill (a
    // matching platform only — never a Linux platform, G11), and the PCSX2
    // writer, which writes through the user-data link into saves/.
    assert_eq!(written.save_strategy, "single_file");
    let config = harness.config();
    assert_eq!(
        config.default_emulators.get(PS2).map(String::as_str),
        Some(PCSX2)
    );
    assert!(!config.default_emulators.contains_key("Linux"));
    let saves = harness.library.join("saves").join(PCSX2);
    assert!(
        saves.join("inis").join("PCSX2.ini").is_file(),
        "PCSX2.ini was written through the inis link"
    );
    assert!(saves.join("memcards").is_dir());

    // The hidden registry row stays: cloud scope depends on it.
    let rows = harness.service.installed().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].platform, "Emulators");
    assert_eq!(
        rows[0].extracted_dir,
        path_text(&harness.package_dir("pcsx2-pkg"))
    );

    let events = harness.installed.lock().unwrap().clone();
    assert_eq!(
        events,
        vec![EmulatorInstalled {
            name: PCSX2.to_string(),
            fresh: true,
            compat_tool: false,
        }]
    );
}

#[tokio::test]
async fn an_unknown_package_falls_back_to_the_rom_title() {
    let harness = Harness::new("").await;
    let exe = if cfg!(windows) {
        "odd/odd-emu.exe"
    } else {
        "odd/odd-emu.AppImage"
    };
    harness
        .mount(2, "Odd Emu", "odd.zip", zip_bytes(&[(exe, b"stub")]))
        .await;

    harness.install(2).await;
    let written = harness.entry("Odd Emu");
    assert_eq!(written.args, "%rom%");
    assert!(written.path.ends_with(exe.rsplit('/').next().unwrap()));
    assert!(
        harness.config().default_emulators.is_empty(),
        "no profile: no platforms"
    );
}

#[tokio::test]
async fn a_package_named_like_a_catalog_entry_is_added_with_the_server_suffix() {
    let catalog_path = "/opt/emulators/pcsx2.AppImage";
    let harness = Harness::new(&format!(
        "[[emulators]]\nname = \"{PCSX2}\"\npath = \"{catalog_path}\"\nargs = \"%rom%\"\nsource_id = \"PCSX2/pcsx2\"\nsource_release_tag = \"latest\"\n"
    ))
    .await;
    harness
        .mount(1, "PCSX2", "pcsx2-pkg.zip", pcsx2_package())
        .await;

    harness.install(1).await;

    let catalog = harness.entry(PCSX2);
    assert_eq!(
        catalog.path, catalog_path,
        "the catalog entry is never overwritten"
    );
    assert_eq!(catalog.source_id, "PCSX2/pcsx2");
    assert_eq!(catalog.source_release_tag, "latest");
    let server = harness.entry("PCSX2 (Playstation 2) (server)");
    assert_eq!(server.path, path_text(&harness.expected_exe("pcsx2-pkg")));
    assert_eq!(
        harness.installed.lock().unwrap()[0].name,
        "PCSX2 (Playstation 2) (server)"
    );
}

#[tokio::test]
async fn reinstalling_a_package_keeps_the_user_args() {
    let harness = Harness::new("").await;
    harness
        .mount(1, "PCSX2", "pcsx2-pkg.zip", pcsx2_package())
        .await;
    harness.install(1).await;

    let mut config = harness.config();
    config.emulators[0].args = "--mine \"%rom%\"".to_string();
    config.save(&harness.config_path).unwrap();

    harness
        .service
        .install_update(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.service.snapshot().entries.first().unwrap().id;
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let config = harness.config();
    assert_eq!(
        config.emulators.len(),
        1,
        "the reinstall updated the entry in place"
    );
    assert_eq!(config.emulators[0].args, "--mine \"%rom%\"");
    let events = harness.installed.lock().unwrap().clone();
    assert_eq!(events.len(), 2);
    assert!(!events[1].fresh);
}

#[tokio::test]
async fn deleting_the_emulator_uninstalls_its_package_and_nothing_else() {
    let harness = Harness::new("").await;
    harness
        .mount(1, "PCSX2", "pcsx2-pkg.zip", pcsx2_package())
        .await;
    let cemu = if cfg!(windows) {
        "cemu/cemu.exe"
    } else {
        "cemu/Cemu-2.6-x86_64.AppImage"
    };
    harness
        .mount(3, "Cemu", "cemu-pkg.zip", zip_bytes(&[(cemu, b"stub")]))
        .await;
    harness.install(1).await;
    harness.install(3).await;
    assert_eq!(harness.service.installed().unwrap().len(), 2);

    let config = harness.config();
    let pcsx2 = harness.entry(PCSX2);
    let removed = harness
        .service
        .uninstall_server_emulator_rows(&pcsx2, &config.emulators)
        .unwrap();
    assert_eq!(removed, 1);

    assert!(
        !harness.package_dir("pcsx2-pkg").exists(),
        "the package files are gone"
    );
    assert!(
        harness.package_dir("cemu-pkg").exists(),
        "the other package is untouched"
    );
    let rows = harness.service.installed().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Cemu");
    // The user data the links pointed at survives under saves/.
    assert!(harness
        .library
        .join("saves")
        .join(PCSX2)
        .join("memcards")
        .is_dir());
}

#[tokio::test]
async fn a_package_with_no_build_for_this_system_installs_but_registers_nothing() {
    let harness = Harness::new("").await;
    // Only the OTHER system's build.
    let foreign = if cfg!(windows) {
        "emu/emu.AppImage"
    } else {
        "emu/emu.exe"
    };
    harness
        .mount(
            4,
            "Foreign Emu",
            "foreign.zip",
            zip_bytes(&[(foreign, b"stub")]),
        )
        .await;

    let entry = harness.install(4).await;
    assert!(
        entry
            .error
            .contains("No launchable emulator executable for this system"),
        "the Completed row carries the warning: {}",
        entry.error
    );
    assert!(harness.config().emulators.is_empty());
    assert!(harness.installed.lock().unwrap().is_empty());
    assert_eq!(
        harness.service.installed().unwrap().len(),
        1,
        "the package row stays"
    );
}
