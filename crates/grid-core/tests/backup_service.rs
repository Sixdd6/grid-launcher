//! Backups through `InstallService`: a wiremock RomM server, a tempdir
//! library and a tempdir "drive" holding a set-up `gridpack/`.
//!
//! Every wait in here is bounded (`wait_for`, `hold_until`), so a broken
//! queue fails the test instead of hanging it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::backup::drives::drive_key;
use grid_core::backup::pack::{self, PackOs, PackState};
use grid_core::backup::plan::{BackupItems, GameItem};
use grid_core::library::backup_job::DriveTarget;
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use md5::{Digest, Md5};
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LABEL: &str = "SAMSUNG T7";
const REMOVED: &str = "SAMSUNG T7 was removed";
/// The smallest valid PNG: enough for the asset sniffing.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// A file name as the content route's path segment carries it: everything
/// but the unreserved set percent-encoded.
fn encode(name: &str) -> String {
    name.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

struct Harness {
    server: MockServer,
    _tmp: tempfile::TempDir,
    library: PathBuf,
    mount: PathBuf,
    service: Arc<InstallService>,
    client: Arc<RommClient>,
}

impl Harness {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        fs::create_dir_all(&library).unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(
            &config_path,
            format!(
                "schema_version = 1\nserver_url = \"http://x\"\nusername = \"u\"\nlibrary_path = {:?}\n",
                library.to_string_lossy()
            ),
        )
        .unwrap();
        let mount = tmp.path().join("media").join(LABEL);
        fs::create_dir_all(&mount).unwrap();
        pack::write_pack_atomic(&mount, &pack::new_pack("Couch kit", &[PackOs::Linux])).unwrap();

        let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
        let service = InstallService::new(registry, config_path);
        // Userinfo in the typed URL must never reach the pack.
        let typed = server.uri().replace("http://", "http://someone:secret@");
        let client = Arc::new(
            RommClient::new(
                &typed,
                Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
            )
            .unwrap(),
        );
        Harness {
            server,
            _tmp: tmp,
            library,
            mount,
            service,
            client,
        }
    }

    fn drive(&self) -> DriveTarget {
        DriveTarget {
            key: drive_key(&self.mount),
            mount: self.mount.clone(),
            label: LABEL.to_string(),
            name: "Couch kit".to_string(),
            fat32: false,
        }
    }

    async fn mount_game(
        &self,
        id: i64,
        name: &str,
        slug: &str,
        fs_name: &str,
        files: &[(i64, &str, &[u8])],
        delay_ms: u64,
    ) {
        let file_json: Vec<serde_json::Value> = files
            .iter()
            .map(|(file_id, file_name, body)| {
                json!({
                    "id": file_id,
                    "file_name": file_name,
                    "file_size_bytes": body.len(),
                    "is_top_level": true,
                    "category": null,
                    "md5_hash": md5_hex(body),
                })
            })
            .collect();
        let detail = json!({
            "id": id,
            "name": name,
            "platform_id": 7,
            "platform_slug": slug,
            "platform_display_name": slug.to_uppercase(),
            "fs_name": fs_name,
            "summary": "A good game.",
            "regions": ["USA"],
            "fs_size_bytes": 10,
            "updated_at": "2026-01-01T00:00:00Z",
            "path_cover_large": format!("/assets/romm/resources/roms/{id}/cover/big.png"),
            "ss_metadata": { "logo_path": format!("roms/{id}/logo.png") },
            "files": file_json,
            "metadatum": {
                "genres": ["Platform", "Action"],
                "companies": ["Virgin Games"],
                "first_release_date": 1234567890000i64,
            },
        });
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(detail))
            .mount(&self.server)
            .await;
        for (file_id, file_name, body) in files {
            let mut template = ResponseTemplate::new(200).set_body_bytes(body.to_vec());
            if delay_ms > 0 {
                template = template.set_delay(Duration::from_millis(delay_ms));
            }
            Mock::given(method("GET"))
                .and(path(format!(
                    "/api/roms/{id}/content/{}",
                    encode(file_name)
                )))
                .and(query_param("file_ids", file_id.to_string()))
                .respond_with(template)
                .mount(&self.server)
                .await;
        }
        for image in [
            format!("/assets/romm/resources/roms/{id}/cover/big.png"),
            format!("/assets/romm/resources/roms/{id}/logo.png"),
        ] {
            Mock::given(method("GET"))
                .and(path(image))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "image/png")
                        .set_body_bytes(PNG.to_vec()),
                )
                .mount(&self.server)
                .await;
        }
    }

    fn items(games: &[(i64, &str)]) -> BackupItems {
        BackupItems {
            games: games
                .iter()
                .map(|(rom_id, name)| GameItem {
                    rom_id: *rom_id,
                    name: name.to_string(),
                    platform_name: "GENESIS".into(),
                })
                .collect(),
            ..Default::default()
        }
    }

    fn rows(&self) -> Vec<DownloadEntry> {
        self.service.snapshot().entries
    }

    fn row_for(&self, rom_id: i64) -> Option<DownloadEntry> {
        self.rows()
            .into_iter()
            .find(|e| e.kind == "backup_game" && e.rom_id == rom_id)
    }

    async fn wait_for(&self, what: &str, pred: impl Fn(&Harness) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !pred(self) {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}; rows: {:#?}",
                self.rows()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn wait_terminal(&self, rom_id: i64) -> DownloadEntry {
        self.wait_for("a terminal row", |h| {
            h.row_for(rom_id).is_some_and(|e| {
                matches!(
                    e.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                )
            })
        })
        .await;
        self.row_for(rom_id).unwrap()
    }

    fn pack(&self) -> pack::Pack {
        match pack::read_pack(&self.mount).unwrap() {
            PackState::Ready(pack) => pack,
            other => panic!("pack not ready: {other:?}"),
        }
    }

    fn staging_is_empty(&self) -> bool {
        let root = self.library.join(".grid-staging");
        !root.exists() || fs::read_dir(&root).unwrap().next().is_none()
    }
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            out.push(path);
        }
    }
    out
}

fn no_tmp_files(root: &Path) -> bool {
    files_under(root)
        .iter()
        .all(|path| !path.to_string_lossy().ends_with(pack::TMP_SUFFIX))
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, body) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }
    cursor.into_inner()
}

#[tokio::test]
async fn backs_up_a_single_file_game_and_a_native_game() {
    let h = Harness::new().await;
    let rom: &[u8] = b"SEGA GENESIS ROM BYTES";
    h.mount_game(
        1,
        "Velvet Static",
        "genesis",
        "Velvet Static (USA).md",
        &[(11, "Velvet Static (USA).md", rom)],
        0,
    )
    .await;
    let archive = zip_bytes(&[("Moss.exe", b"MZ fake")]);
    let sidecar: &[u8] = br#"{"name": "Moss & Marrow"}"#;
    h.mount_game(
        2,
        "Moss & Marrow",
        "win",
        "Moss & Marrow",
        &[(21, "Moss.zip", &archive), (22, "game.json", sidecar)],
        0,
    )
    .await;

    let queued = h
        .service
        .start_backup(
            h.client.clone(),
            h.drive(),
            Harness::items(&[(1, "Velvet Static"), (2, "Moss & Marrow")]),
        )
        .unwrap();
    assert_eq!((queued.games, queued.emulators, queued.firmware), (2, 0, 0));
    assert!(queued.batch > 0);
    for rom_id in [1, 2] {
        let row = h.wait_terminal(rom_id).await;
        assert_eq!(row.status, DownloadStatus::Completed, "{row:#?}");
        assert_eq!(row.job, "backup");
        assert_eq!(row.drive_label, LABEL);
        assert_eq!(row.drive_name, "Couch kit");
        assert_eq!(row.batch, queued.batch);
    }

    let grid = h.mount.join("gridpack");
    // Single file: the file itself under the platform.
    assert_eq!(
        fs::read(grid.join("games/genesis/Velvet Static (USA).md")).unwrap(),
        rom
    );
    // Native: a folder with the archive and its sidecar.
    assert_eq!(
        fs::read(grid.join("games/win/Moss & Marrow/Moss.zip")).unwrap(),
        archive
    );
    assert_eq!(
        fs::read(grid.join("games/win/Moss & Marrow/game.json")).unwrap(),
        sidecar
    );
    assert_eq!(
        fs::read(grid.join("assets/genesis/Velvet Static (USA)/cover.png")).unwrap(),
        PNG
    );
    assert!(grid
        .join("assets/genesis/Velvet Static (USA)/logo.png")
        .is_file());

    let pack = h.pack();
    assert_eq!(pack.games.len(), 2);
    let velvet = pack.find_game(&h.server.uri(), 1).expect("game 1 indexed");
    assert!(!velvet.source.server.contains('@'));
    assert!(!velvet.source.server.contains("secret"));
    assert_eq!(velvet.files.len(), 1);
    assert_eq!(velvet.files[0].path, "games/genesis/Velvet Static (USA).md");
    assert_eq!(velvet.files[0].server_file_id, 11);
    assert_eq!(velvet.files[0].size, rom.len() as u64);
    assert_eq!(velvet.files[0].md5, md5_hex(rom));
    assert_eq!(velvet.size, rom.len() as u64);
    assert_eq!(velvet.release_date, "2009-02-13");
    assert_eq!(velvet.genres, vec!["Platform", "Action"]);
    assert_eq!(velvet.platform, "genesis");
    assert_eq!(
        velvet.assets.cover.as_deref(),
        Some("assets/genesis/Velvet Static (USA)/cover.png")
    );
    let moss = pack.find_game(&h.server.uri(), 2).expect("game 2 indexed");
    let paths: Vec<&str> = moss.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "games/win/Moss & Marrow/Moss.zip",
            "games/win/Moss & Marrow/game.json"
        ]
    );
    // The pack rebuilds the same install plan input.
    let detail = pack::pack_game_detail(moss);
    assert_eq!(detail.files.len(), 2);

    // Nothing outside gridpack/, nothing half-written, no staging left.
    let top: Vec<String> = fs::read_dir(&h.mount)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(top, vec!["gridpack"]);
    assert!(no_tmp_files(&h.mount));
    assert!(h.staging_is_empty());

    // A second backup of the same game completes with "Already on".
    let again = h
        .service
        .start_backup(
            h.client.clone(),
            h.drive(),
            Harness::items(&[(1, "Velvet Static")]),
        )
        .unwrap();
    assert_eq!(again.games, 1);
    h.wait_for("the second backup", |h| {
        h.rows()
            .iter()
            .any(|e| e.batch == again.batch && e.status == DownloadStatus::Completed)
    })
    .await;
    let row = h
        .rows()
        .into_iter()
        .find(|e| e.batch == again.batch)
        .unwrap();
    assert_eq!(row.error, "Already on Couch kit");
}

#[tokio::test]
async fn a_drive_removed_mid_download_fails_the_row_and_folds_the_rest() {
    let h = Harness::new().await;
    h.mount_game(
        1,
        "One",
        "genesis",
        "One.md",
        &[(11, "One.md", b"one")],
        1500,
    )
    .await;
    h.mount_game(2, "Two", "genesis", "Two.md", &[(21, "Two.md", b"two")], 0)
        .await;
    h.mount_game(
        3,
        "Three",
        "genesis",
        "Three.md",
        &[(31, "Three.md", b"three")],
        0,
    )
    .await;
    h.service
        .start_backup(
            h.client.clone(),
            h.drive(),
            Harness::items(&[(1, "One"), (2, "Two"), (3, "Three")]),
        )
        .unwrap();
    h.wait_for("row 1 downloading", |h| {
        h.row_for(1)
            .is_some_and(|e| e.status == DownloadStatus::Downloading)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Unplug: the folder goes away and the watcher reports it.
    let away = h.mount.with_file_name("unplugged");
    fs::rename(&h.mount, &away).unwrap();
    let report = h.service.abort_drive(&drive_key(&h.mount), REMOVED);
    assert_eq!((report.failed, report.cancelled), (1, 2));

    let row = h.wait_terminal(1).await;
    assert_eq!(row.status, DownloadStatus::Failed);
    assert_eq!(row.error, REMOVED);
    let rows = h.rows();
    let summaries: Vec<&DownloadEntry> = rows
        .iter()
        .filter(|e| e.kind == "backup_cancelled")
        .collect();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].title, "2 more items");
    assert_eq!(summaries[0].error, REMOVED);
    assert_eq!(summaries[0].status, DownloadStatus::Cancelled);
    assert!(h.row_for(2).is_none() && h.row_for(3).is_none());

    // Plugged back in: nothing half-written survives cleanup, nothing indexed.
    fs::rename(&away, &h.mount).unwrap();
    pack::cleanup_leftovers(&h.mount).unwrap();
    assert!(no_tmp_files(&h.mount));
    assert!(h.pack().games.is_empty());
    h.wait_for("staging removed", |h| h.staging_is_empty())
        .await;
}

#[tokio::test]
async fn a_drive_removed_during_the_write_is_found_by_the_writer() {
    let h = Harness::new().await;
    h.mount_game(1, "One", "genesis", "One.md", &[(11, "One.md", b"one")], 0)
        .await;
    h.mount_game(2, "Two", "genesis", "Two.md", &[(21, "Two.md", b"two")], 0)
        .await;
    // The first write finds the drive gone: no watcher call at all.
    let mount = h.mount.clone();
    let away = h.mount.with_file_name("unplugged");
    let target = away.clone();
    let done = Arc::new(AtomicBool::new(false));
    let once = done.clone();
    h.service.set_backup_write_hook(Some(Arc::new(move || {
        if !once.swap(true, Ordering::SeqCst) {
            fs::rename(&mount, &target).unwrap();
        }
    })));
    h.service
        .start_backup(
            h.client.clone(),
            h.drive(),
            Harness::items(&[(1, "One"), (2, "Two")]),
        )
        .unwrap();
    let row = h.wait_terminal(1).await;
    assert_eq!(row.status, DownloadStatus::Failed, "{row:#?}");
    assert_eq!(row.error, REMOVED);
    h.wait_for("the summary row", |h| {
        h.rows().iter().any(|e| e.kind == "backup_cancelled")
    })
    .await;
    let summary = h
        .rows()
        .into_iter()
        .find(|e| e.kind == "backup_cancelled")
        .unwrap();
    assert_eq!(summary.title, "Two");
    assert_eq!(summary.error, REMOVED);
    fs::rename(&away, &h.mount).unwrap();
    pack::cleanup_leftovers(&h.mount).unwrap();
    assert!(no_tmp_files(&h.mount));
    assert!(h.pack().games.is_empty());
}

#[tokio::test]
async fn a_cancel_during_the_write_stops_it() {
    let h = Harness::new().await;
    h.mount_game(1, "One", "genesis", "One.md", &[(11, "One.md", b"one")], 0)
        .await;
    // Hold the first write until the test has cancelled (bounded).
    let release = Arc::new(AtomicBool::new(false));
    let held = Arc::new(AtomicUsize::new(0));
    let (gate, count) = (release.clone(), held.clone());
    h.service.set_backup_write_hook(Some(Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !gate.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    })));
    h.service
        .start_backup(h.client.clone(), h.drive(), Harness::items(&[(1, "One")]))
        .unwrap();
    h.wait_for("the write to start", |_| held.load(Ordering::SeqCst) > 0)
        .await;
    let id = h.row_for(1).unwrap().id;
    assert_eq!(h.row_for(1).unwrap().status, DownloadStatus::Installing);
    h.service.cancel(id);
    assert_eq!(h.row_for(1).unwrap().status, DownloadStatus::Cancelling);
    release.store(true, Ordering::SeqCst);

    let row = h.wait_terminal(1).await;
    assert_eq!(row.status, DownloadStatus::Cancelled, "{row:#?}");
    assert!(h.pack().games.is_empty());
    assert!(!h.mount.join("gridpack/games/genesis/One.md").exists());
    assert!(no_tmp_files(&h.mount));
    assert!(h.staging_is_empty());
}

#[tokio::test]
async fn retry_after_a_failure_writes_the_game() {
    let h = Harness::new().await;
    // The first content request fails, every later one succeeds.
    Mock::given(method("GET"))
        .and(path("/api/roms/1/content/One.md"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&h.server)
        .await;
    h.mount_game(1, "One", "genesis", "One.md", &[(11, "One.md", b"one")], 0)
        .await;
    h.service
        .start_backup(h.client.clone(), h.drive(), Harness::items(&[(1, "One")]))
        .unwrap();
    let failed = h.wait_terminal(1).await;
    assert_eq!(failed.status, DownloadStatus::Failed, "{failed:#?}");
    assert!(h.staging_is_empty());

    h.service
        .retry(Some(h.client.clone()), failed.id)
        .await
        .unwrap();
    h.wait_for("the retried row", |h| {
        h.row_for(1)
            .is_some_and(|e| e.id != failed.id && e.status == DownloadStatus::Completed)
    })
    .await;
    assert!(h.rows().iter().all(|e| e.id != failed.id));
    assert!(h.pack().has_game(&h.server.uri(), 1));
    assert_eq!(
        fs::read(h.mount.join("gridpack/games/genesis/One.md")).unwrap(),
        b"one"
    );

    // A row whose drive is gone cannot be retried, and survives.
    h.mount_game(2, "Two", "genesis", "Two.md", &[], 0).await;
    h.service
        .start_backup(h.client.clone(), h.drive(), Harness::items(&[(2, "Two")]))
        .unwrap();
    let row = h.wait_terminal(2).await;
    assert_eq!(row.status, DownloadStatus::Failed);
    let away = h.mount.with_file_name("unplugged");
    fs::rename(&h.mount, &away).unwrap();
    let error = h
        .service
        .retry(Some(h.client.clone()), row.id)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), format!("Connect {LABEL} to retry"));
    assert!(h.rows().iter().any(|e| e.id == row.id));
}

#[tokio::test]
async fn backs_up_a_manual_emulator_archive_and_firmware() {
    use grid_core::backup::plan::{
        EmulatorItem, EmulatorKind, EmulatorPick, EmulatorSource, FirmwareItem,
    };
    let h = Harness::new().await;
    let bios: &[u8] = b"BIOS BYTES";
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .and(query_param("platform_id", "9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 1, "file_name": "scph1001.bin", "file_size_bytes": bios.len(), "md5_hash": md5_hex(bios) },
            { "id": 2, "file_name": "bad.bin", "file_size_bytes": 3, "md5_hash": "00" },
        ])))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/1/content/scph1001.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bios.to_vec()))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/2/content/bad.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bad".to_vec()))
        .mount(&h.server)
        .await;
    let folder = h.library.parent().unwrap().join("emus").join("myemu");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("myemu"), b"#!/bin/sh\n").unwrap();

    let host = PackOs::host().expect("a supported OS");
    let items = BackupItems {
        games: Vec::new(),
        emulators: vec![EmulatorItem {
            pick: EmulatorPick {
                name: "MyEmu".into(),
                kind: EmulatorKind::Emulator,
                source: EmulatorSource::Local {
                    program: folder.join("myemu"),
                    args: "-fullscreen {rom}".into(),
                },
                platforms: vec!["psx".into()],
            },
            os: host,
        }],
        firmware: vec![FirmwareItem {
            platform_id: 9,
            slug: "psx".into(),
            platform_name: "PlayStation".into(),
        }],
    };
    let queued = h
        .service
        .start_backup(h.client.clone(), h.drive(), items)
        .unwrap();
    assert_eq!((queued.games, queued.emulators, queued.firmware), (0, 1, 1));
    h.wait_for("both rows to finish", |h| {
        let rows = h.rows();
        rows.len() == 2
            && rows
                .iter()
                .all(|e| matches!(e.status, DownloadStatus::Completed | DownloadStatus::Failed))
    })
    .await;
    let rows = h.rows();
    assert!(
        rows.iter().all(|e| e.status == DownloadStatus::Completed),
        "{rows:#?}"
    );
    let firmware_row = rows.iter().find(|e| e.kind == "backup_firmware").unwrap();
    assert!(firmware_row.error.contains("bad.bin"), "{firmware_row:#?}");

    let pack = h.pack();
    let emulator = &pack.emulators[0];
    let dir = format!("emulators/myemu/local/{}", host.arch_tag());
    assert_eq!(emulator.path, format!("{dir}/MyEmu.zip"));
    assert_eq!(emulator.source_id, "local:MyEmu");
    assert_eq!(emulator.program, "myemu");
    assert_eq!(emulator.args, "-fullscreen {rom}");
    assert_eq!(emulator.kind, "emulator");
    assert!(emulator.size > 0);
    assert!(h.mount.join("gridpack").join(&emulator.path).is_file());
    assert_eq!(pack.firmware.len(), 1);
    assert_eq!(pack.firmware[0].files.len(), 1);
    assert_eq!(pack.firmware[0].files[0].path, "firmware/psx/scph1001.bin");
    assert_eq!(
        fs::read(h.mount.join("gridpack/firmware/psx/scph1001.bin")).unwrap(),
        bios
    );
    assert!(h.staging_is_empty());
    // Its size is now known to the planner.
    assert!(h
        .service
        .emulator_sizes()
        .contains_key(&("MyEmu".to_string(), host)));
}
