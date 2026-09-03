//! End-to-end tests for `InstallService`: the queue, download, finalize and
//! uninstall glue. Every test runs against a wiremock RomM server, a tempdir
//! library, and a snapshot-collecting notify callback.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use grid_core::library::queue::{DownloadEntry, DownloadStatus, DownloadsSnapshot};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::{InstallService, LibraryError};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// --- fixtures ---------------------------------------------------------------

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

/// Builds a zip archive at `path` with `Stored` (uncompressed) entries.
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) -> Vec<u8> {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for &(name, content) in entries {
        zip.start_file(name, options).unwrap();
        zip.write_all(content).unwrap();
    }
    zip.finish().unwrap();
    fs::read(path).unwrap()
}

/// One entry of a `DetailedRomSchema.files` array.
struct FileSpec {
    id: i64,
    file_name: String,
    size: i64,
    top_level: bool,
    category: Option<String>,
}

fn file_spec(id: i64, file_name: &str, size: usize) -> FileSpec {
    FileSpec {
        id,
        file_name: file_name.to_string(),
        size: size as i64,
        top_level: true,
        category: None,
    }
}

/// Like [`file_spec`], but with an explicit RomM file `category` (e.g.
/// `"update"`, `"dlc"`) instead of the default `null`.
fn file_spec_with_category(id: i64, file_name: &str, size: usize, category: &str) -> FileSpec {
    FileSpec {
        category: Some(category.to_string()),
        ..file_spec(id, file_name, size)
    }
}

fn detail_json(
    id: i64,
    name: &str,
    platform: &str,
    fs_name: &str,
    files: &[FileSpec],
) -> serde_json::Value {
    let files: Vec<serde_json::Value> = files
        .iter()
        .map(|f| {
            json!({
                "id": f.id,
                "file_name": f.file_name,
                "file_size_bytes": f.size,
                "is_top_level": f.top_level,
                "category": f.category,
            })
        })
        .collect();
    json!({
        "id": id,
        "name": name,
        "fs_name_no_ext": name,
        "platform_id": 7,
        "platform_display_name": platform,
        "fs_name": fs_name,
        "summary": "A very good game.",
        "regions": ["USA"],
        "languages": ["En"],
        "tags": ["favorite"],
        "revision": "1.1",
        "fs_size_bytes": 4321,
        "updated_at": "2026-01-01T00:00:00Z",
        "files": files,
        "metadatum": {
            "average_rating": 9.5,
            "genres": ["RPG"],
            "companies": ["Square"],
            "first_release_date": 1234567890i64,
        },
    })
}

// --- harness ----------------------------------------------------------------

struct Harness {
    server: MockServer,
    _tmp: tempfile::TempDir,
    library: PathBuf,
    service: Arc<InstallService>,
    client: Arc<RommClient>,
    registry: Arc<Registry>,
    snapshots: Arc<Mutex<Vec<DownloadsSnapshot>>>,
}

impl Harness {
    async fn new() -> Self {
        Self::build(true, "").await
    }

    async fn without_library_path() -> Self {
        Self::build(false, "").await
    }

    /// [`Harness::new`], with `extra_config` appended to the generated
    /// `config.toml` — used to configure emulator entries.
    async fn with_config(extra_config: &str) -> Self {
        Self::build(true, extra_config).await
    }

    async fn build(with_library: bool, extra_config: &str) -> Self {
        let server = MockServer::start().await;
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        fs::create_dir_all(&library).unwrap();

        let config_path = tmp.path().join("config.toml");
        let library_line = if with_library {
            format!("library_path = {:?}\n", library.to_string_lossy())
        } else {
            String::new()
        };
        fs::write(
            &config_path,
            format!(
                "schema_version = 1\nserver_url = \"http://x\"\nusername = \"u\"\n{library_line}{extra_config}"
            ),
        )
        .unwrap();

        let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
        let service = InstallService::new(registry.clone(), config_path);
        let snapshots: Arc<Mutex<Vec<DownloadsSnapshot>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = snapshots.clone();
        service.set_notify(Arc::new(move |snap| sink.lock().unwrap().push(snap)));

        let client = Arc::new(RommClient::new(&server.uri(), token_cred()).unwrap());
        Harness {
            server,
            _tmp: tmp,
            library,
            service,
            client,
            registry,
            snapshots,
        }
    }

    async fn mount_detail(&self, rom_id: i64, detail: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{rom_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(detail))
            .mount(&self.server)
            .await;
    }

    async fn mount_content(&self, rom_id: i64, file_name: &str, body: Vec<u8>, delay_ms: u64) {
        let mut template = ResponseTemplate::new(200).set_body_bytes(body);
        if delay_ms > 0 {
            template = template.set_delay(Duration::from_millis(delay_ms));
        }
        Mock::given(method("GET"))
            .and(path(format!("/api/roms/{rom_id}/content/{file_name}")))
            .respond_with(template)
            .mount(&self.server)
            .await;
    }

    fn newest_entry_id(&self) -> u64 {
        self.service
            .snapshot()
            .entries
            .first()
            .expect("at least one entry")
            .id
    }

    fn entry(&self, id: u64) -> DownloadEntry {
        self.service
            .snapshot()
            .entries
            .into_iter()
            .find(|e| e.id == id)
            .expect("entry exists")
    }

    /// Polls the snapshot until `pred` holds for entry `id`, or panics after
    /// 30 s. Real awaits only — no simulated clock.
    async fn wait_for(&self, id: u64, pred: fn(&DownloadEntry) -> bool) -> DownloadEntry {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(entry) = self.service.snapshot().entries.iter().find(|e| e.id == id) {
                if pred(entry) {
                    return entry.clone();
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting on entry {id}; status {:?} error {:?}",
                    entry.status,
                    entry.error
                );
            } else {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for entry {id}"
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn wait_terminal(&self, id: u64) -> DownloadEntry {
        self.wait_for(id, |e| {
            matches!(
                e.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            )
        })
        .await
    }
}

fn is_terminal(entry: &DownloadEntry) -> bool {
    matches!(
        entry.status,
        DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
    )
}

// --- single-file install ----------------------------------------------------

#[tokio::test]
async fn single_file_zip_install_extracts_deletes_archive_and_records_row() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono.zip"),
        &[("game.sfc", b"ROMDATA")],
    );

    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "Chrono Trigger",
                "SNES",
                "chrono.zip",
                &[file_spec(11, "chrono.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(1, "chrono.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;

    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.error, "");

    let archive = harness.library.join("SNES/chrono.zip");
    let extracted_dir = harness.library.join("SNES/chrono");
    assert!(!archive.exists(), "archive should be deleted after extract");
    assert!(extracted_dir.join("game.sfc").is_file());

    let installed = harness.service.installed().unwrap();
    assert_eq!(installed.len(), 1);
    let row = &installed[0];
    assert_eq!(row.title, "Chrono Trigger");
    assert_eq!(row.platform, "SNES");
    assert_eq!(row.rom_id, Some(1));
    assert_eq!(row.rom_file_name, "chrono.zip");
    assert_eq!(row.archive_path, "");
    assert_eq!(
        row.extracted_path,
        extracted_dir.join("game.sfc").to_string_lossy()
    );
    assert_eq!(row.extracted_dir, extracted_dir.to_string_lossy());
    assert_eq!(row.multi_file_game_dir, "");
    assert_eq!(row.description, "A very good game.");
    assert_eq!(row.genres, "RPG");
    assert_eq!(row.companies, "Square");
    assert_eq!(row.regions, "USA");
    assert!(row.installed_at > 0);

    // Installed-badge lookup: the row is findable by rom_id.
    assert!(harness
        .registry
        .find(Some(1), "Chrono Trigger", "SNES")
        .unwrap()
        .is_some());

    // The notify callback saw the terminal transition.
    let snapshots = harness.snapshots.lock().unwrap();
    assert!(!snapshots.is_empty());
    let last = snapshots.last().unwrap();
    assert!(last.entries.iter().any(|e| e.id == id && is_terminal(e)));
}

#[tokio::test]
async fn arcade_zip_is_not_extracted_and_keeps_archive_path() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(&staging.path().join("mslug.zip"), &[("mslug.rom", b"ARC")]);

    harness
        .mount_detail(
            9,
            detail_json(
                9,
                "Metal Slug",
                "Arcade",
                "mslug.zip",
                &[file_spec(91, "mslug.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(9, "mslug.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 9)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let archive = harness.library.join("Arcade/mslug.zip");
    assert!(archive.is_file(), "arcade archive must be kept");
    assert!(!harness.library.join("Arcade/mslug").exists());

    let row = harness.registry.find(Some(9), "", "").unwrap().unwrap();
    assert_eq!(row.archive_path, archive.to_string_lossy());
    assert_eq!(row.extracted_path, "");
    assert_eq!(row.extracted_dir, "");
}

// --- content categories (D12) ------------------------------------------------

#[tokio::test]
async fn update_category_file_is_excluded_and_never_requested() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("game.zip"),
        &[("game.sfc", b"ROMDATA")],
    );

    harness
        .mount_detail(
            12,
            detail_json(
                12,
                "Some Game",
                "SNES",
                "game.zip",
                &[
                    file_spec(121, "game.zip", bytes.len()),
                    file_spec_with_category(122, "update.zip", 9, "update"),
                ],
            ),
        )
        .await;
    harness.mount_content(12, "game.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 12)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let extracted_dir = harness.library.join("SNES/game");
    assert!(extracted_dir.join("game.sfc").is_file());

    let row = harness.registry.find(Some(12), "", "").unwrap().unwrap();
    assert_eq!(
        row.multi_file_game_dir, "",
        "the update file must not turn this into a multi-file install"
    );

    let requests = harness.server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|r| !r.url.path().contains("update.zip")),
        "the update-category file must never be requested: {:?}",
        requests.iter().map(|r| r.url.path()).collect::<Vec<_>>()
    );
}

// --- multi-file install -----------------------------------------------------

#[tokio::test]
async fn multi_file_install_keeps_both_files_and_points_at_the_m3u() {
    let harness = Harness::new().await;
    harness
        .mount_detail(
            2,
            detail_json(
                2,
                "Final Fantasy VII",
                "PlayStation",
                "Final Fantasy VII",
                &[
                    file_spec(21, "disc1.bin", 5),
                    file_spec(22, "game.m3u", 9),
                    FileSpec {
                        id: 23,
                        file_name: "game.json".to_string(),
                        size: 2,
                        top_level: true,
                        category: None,
                    },
                    FileSpec {
                        id: 24,
                        file_name: "sub/nested.bin".to_string(),
                        size: 2,
                        top_level: true,
                        category: None,
                    },
                    FileSpec {
                        id: 25,
                        file_name: "inner.bin".to_string(),
                        size: 2,
                        top_level: false,
                        category: None,
                    },
                ],
            ),
        )
        .await;
    harness
        .mount_content(2, "disc1.bin", b"DISC1".to_vec(), 0)
        .await;
    harness
        .mount_content(2, "game.m3u", b"disc1.bin".to_vec(), 0)
        .await;

    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let game_dir = harness.library.join("PlayStation/Final Fantasy VII");
    assert!(game_dir.join("disc1.bin").is_file());
    assert!(game_dir.join("game.m3u").is_file());
    // Filtered candidates were never fetched.
    assert!(!game_dir.join("game.json").exists());
    assert!(!game_dir.join("inner.bin").exists());
    assert!(!game_dir.join("sub").exists());
    // Nothing was extracted.
    assert!(!game_dir.join("disc1").exists());

    let row = harness.registry.find(Some(2), "", "").unwrap().unwrap();
    assert_eq!(row.multi_file_game_dir, game_dir.to_string_lossy());
    assert_eq!(
        row.extracted_path,
        game_dir.join("game.m3u").to_string_lossy()
    );
    assert_eq!(row.extracted_dir, "");
    assert_eq!(row.archive_path, "");
    assert_eq!(row.rom_file_name, "game.m3u");
}

#[tokio::test]
async fn no_downloadable_file_fails_before_admission() {
    let harness = Harness::new().await;
    harness
        .mount_detail(
            3,
            detail_json(
                3,
                "Ghost",
                "SNES",
                "ghost.zip",
                &[FileSpec {
                    id: 31,
                    file_name: "game.json".to_string(),
                    size: 2,
                    top_level: true,
                    category: None,
                }],
            ),
        )
        .await;

    let err = harness
        .service
        .install(harness.client.clone(), 3)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, LibraryError::Extract(msg)
            if msg == "the server lists no downloadable file for this game"),
        "unexpected error: {err}"
    );
    assert!(harness.service.snapshot().entries.is_empty());
}

// --- queueing ---------------------------------------------------------------

#[tokio::test]
async fn second_install_queues_then_runs_after_the_first() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let first = write_zip(&staging.path().join("one.zip"), &[("one.sfc", b"ONE")]);
    let second = write_zip(&staging.path().join("two.zip"), &[("two.sfc", b"TWO")]);

    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "One",
                "SNES",
                "one.zip",
                &[file_spec(11, "one.zip", first.len())],
            ),
        )
        .await;
    harness
        .mount_detail(
            2,
            detail_json(
                2,
                "Two",
                "SNES",
                "two.zip",
                &[file_spec(21, "two.zip", second.len())],
            ),
        )
        .await;
    harness.mount_content(1, "one.zip", first, 400).await;
    harness.mount_content(2, "two.zip", second, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let first_id = harness.newest_entry_id();
    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    let second_id = harness.newest_entry_id();
    assert_ne!(first_id, second_id);
    assert_eq!(harness.entry(second_id).status, DownloadStatus::Queued);

    let first_entry = harness.wait_terminal(first_id).await;
    assert_eq!(
        first_entry.status,
        DownloadStatus::Completed,
        "{}",
        first_entry.error
    );
    let second_entry = harness.wait_terminal(second_id).await;
    assert_eq!(
        second_entry.status,
        DownloadStatus::Completed,
        "{}",
        second_entry.error
    );

    assert!(harness.library.join("SNES/one/one.sfc").is_file());
    assert!(harness.library.join("SNES/two/two.sfc").is_file());
    assert_eq!(harness.service.installed().unwrap().len(), 2);
}

#[tokio::test]
async fn duplicate_rom_while_queued_creates_no_new_entry() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let first = write_zip(&staging.path().join("one.zip"), &[("one.sfc", b"ONE")]);
    let second = write_zip(&staging.path().join("two.zip"), &[("two.sfc", b"TWO")]);

    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "One",
                "SNES",
                "one.zip",
                &[file_spec(11, "one.zip", first.len())],
            ),
        )
        .await;
    harness
        .mount_detail(
            2,
            detail_json(
                2,
                "Two",
                "SNES",
                "two.zip",
                &[file_spec(21, "two.zip", second.len())],
            ),
        )
        .await;
    harness.mount_content(1, "one.zip", first, 400).await;
    harness.mount_content(2, "two.zip", second, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    // Same rom while it sits in the queue: silently ignored.
    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    assert_eq!(harness.service.snapshot().entries.len(), 2);
}

// --- already installed ------------------------------------------------------

#[tokio::test]
async fn already_installed_rom_completes_without_finalizing() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono.zip"),
        &[("game.sfc", b"ROMDATA")],
    );
    harness
        .registry
        .upsert(&InstalledGame {
            title: "Chrono Trigger".to_string(),
            platform: "SNES".to_string(),
            rom_id: Some(1),
            rom_file_name: "chrono.zip".to_string(),
            archive_path: "/somewhere/chrono.zip".to_string(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();

    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "Chrono Trigger",
                "SNES",
                "chrono.zip",
                &[file_spec(11, "chrono.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(1, "chrono.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    // Downloaded once, but never finalized: no extraction, row untouched.
    assert!(harness.library.join("SNES/chrono.zip").is_file());
    assert!(!harness.library.join("SNES/chrono").exists());
    let row = harness.registry.find(Some(1), "", "").unwrap().unwrap();
    assert_eq!(row.archive_path, "/somewhere/chrono.zip");
    assert_eq!(row.installed_at, 1);
}

#[tokio::test]
async fn a_same_title_platform_row_with_a_different_rom_id_does_not_skip_finalize() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono.zip"),
        &[("game.sfc", b"ROMDATA")],
    );
    // Same title/platform as the game being installed, but a different
    // rom_id: `find`'s (title_key, platform_key) fallback would hand this
    // row back for rom_id 1, but it must not be accepted as that install.
    harness
        .registry
        .upsert(&InstalledGame {
            title: "Chrono Trigger".to_string(),
            platform: "SNES".to_string(),
            rom_id: Some(999),
            rom_file_name: "chrono.zip".to_string(),
            archive_path: "/somewhere/chrono.zip".to_string(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();

    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "Chrono Trigger",
                "SNES",
                "chrono.zip",
                &[file_spec(11, "chrono.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(1, "chrono.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    // Finalize actually ran: the archive was extracted, not left in place.
    let archive = harness.library.join("SNES/chrono.zip");
    let extracted_dir = harness.library.join("SNES/chrono");
    assert!(
        !archive.exists(),
        "archive should be deleted after extract, not skipped as already-installed"
    );
    assert!(extracted_dir.join("game.sfc").is_file());

    // The upsert replaced the (title_key, platform_key) identity row: one
    // row remains, and it now carries the new rom_id.
    let installed = harness.service.installed().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].rom_id, Some(1));
    assert_eq!(
        installed[0].extracted_path,
        extracted_dir.join("game.sfc").to_string_lossy()
    );
}

// --- failure paths ----------------------------------------------------------

#[tokio::test]
async fn finalize_failure_marks_failed_and_keeps_the_archive() {
    let harness = Harness::new().await;
    let bytes = b"this is not a real archive at all".to_vec();
    harness
        .mount_detail(
            4,
            detail_json(
                4,
                "Corrupt",
                "SNES",
                "corrupt.zip",
                &[file_spec(41, "corrupt.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(4, "corrupt.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 4)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;

    assert_eq!(entry.status, DownloadStatus::Failed);
    assert!(!entry.error.is_empty());
    assert!(
        harness.library.join("SNES/corrupt.zip").is_file(),
        "a finalize failure must keep the archive for a cheap retry"
    );
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn cancel_mid_download_marks_cancelled_and_removes_the_partial() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(&staging.path().join("slow.zip"), &[("slow.sfc", b"SLOW")]);
    harness
        .mount_detail(
            5,
            detail_json(
                5,
                "Slow",
                "SNES",
                "slow.zip",
                &[file_spec(51, "slow.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(5, "slow.zip", bytes, 800).await;

    harness
        .service
        .install(harness.client.clone(), 5)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    harness.service.cancel(id);
    assert_eq!(harness.entry(id).status, DownloadStatus::Cancelling);

    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Cancelled);
    assert!(!harness.library.join("SNES/slow.zip").exists());
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn cancelling_a_queued_entry_removes_it_and_lets_the_rest_run() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let first = write_zip(&staging.path().join("one.zip"), &[("one.sfc", b"ONE")]);
    let second = write_zip(&staging.path().join("two.zip"), &[("two.sfc", b"TWO")]);
    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "One",
                "SNES",
                "one.zip",
                &[file_spec(11, "one.zip", first.len())],
            ),
        )
        .await;
    harness
        .mount_detail(
            2,
            detail_json(
                2,
                "Two",
                "SNES",
                "two.zip",
                &[file_spec(21, "two.zip", second.len())],
            ),
        )
        .await;
    harness.mount_content(1, "one.zip", first, 400).await;
    harness.mount_content(2, "two.zip", second, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let first_id = harness.newest_entry_id();
    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    let second_id = harness.newest_entry_id();

    harness.service.cancel(second_id);
    let cancelled = harness.entry(second_id);
    assert_eq!(cancelled.status, DownloadStatus::Cancelled);
    assert_eq!(cancelled.error, "Cancelled while queued");

    let first_entry = harness.wait_terminal(first_id).await;
    assert_eq!(
        first_entry.status,
        DownloadStatus::Completed,
        "{}",
        first_entry.error
    );
    assert!(!harness.library.join("SNES/two.zip").exists());
    assert_eq!(harness.service.installed().unwrap().len(), 1);
}

#[tokio::test]
async fn dismiss_removes_a_terminal_entry_but_refuses_an_active_one() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(&staging.path().join("slow.zip"), &[("slow.sfc", b"SLOW")]);
    harness
        .mount_detail(
            6,
            detail_json(
                6,
                "Slow",
                "SNES",
                "slow.zip",
                &[file_spec(61, "slow.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(6, "slow.zip", bytes, 400).await;

    harness
        .service
        .install(harness.client.clone(), 6)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    // Owns the download slot: dismiss is a no-op.
    harness.service.dismiss(id);
    assert_eq!(harness.service.snapshot().entries.len(), 1);

    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    harness.service.dismiss(id);
    assert!(harness.service.snapshot().entries.is_empty());
}

#[tokio::test]
async fn retry_after_failure_dismisses_the_old_entry_and_starts_a_new_one() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let good = write_zip(&staging.path().join("retry.zip"), &[("retry.sfc", b"OK")]);

    // First attempt: a body that cannot be extracted.
    harness
        .mount_detail(
            7,
            detail_json(
                7,
                "Retry Me",
                "SNES",
                "retry.zip",
                &[file_spec(71, "retry.zip", 33)],
            ),
        )
        .await;
    let bad_mock = Mock::given(method("GET"))
        .and(path("/api/roms/7/content/retry.zip"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"this is not a real archive at all".to_vec()),
        );
    let bad_guard = harness.server.register_as_scoped(bad_mock).await;

    harness
        .service
        .install(harness.client.clone(), 7)
        .await
        .unwrap();
    let failed_id = harness.newest_entry_id();
    let entry = harness.wait_terminal(failed_id).await;
    assert_eq!(entry.status, DownloadStatus::Failed);

    // Second attempt with a valid archive of a different size, so the
    // already-downloaded archive is refetched rather than skipped.
    drop(bad_guard);
    harness.server.reset().await;
    harness
        .mount_detail(
            7,
            detail_json(
                7,
                "Retry Me",
                "SNES",
                "retry.zip",
                &[file_spec(71, "retry.zip", good.len())],
            ),
        )
        .await;
    harness.mount_content(7, "retry.zip", good, 0).await;

    harness
        .service
        .retry(Some(harness.client.clone()), failed_id)
        .await
        .unwrap();
    let new_id = harness.newest_entry_id();
    assert_ne!(new_id, failed_id);
    let entry = harness.wait_terminal(new_id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert!(harness
        .service
        .snapshot()
        .entries
        .iter()
        .all(|e| e.id != failed_id));
    assert!(harness.library.join("SNES/retry/retry.sfc").is_file());
}

#[tokio::test]
async fn a_panicking_notify_listener_fails_the_entry_without_wedging_the_queue() {
    let harness = Harness::new().await;
    // Replace the collector with a listener that panics exactly once, on the
    // first snapshot carrying download progress — i.e. from inside the
    // download task, via on_download_progress. The panicking task prints a
    // backtrace line to stderr; that noise is expected.
    let fired = Arc::new(AtomicBool::new(false));
    let armed = fired.clone();
    harness
        .service
        .set_notify(Arc::new(move |snap: DownloadsSnapshot| {
            let progressing = snap.entries.iter().any(|e| e.downloaded_bytes > 0);
            if progressing && !armed.swap(true, Ordering::SeqCst) {
                panic!("listener blew up");
            }
        }));

    let staging = tempfile::tempdir().unwrap();
    let first = write_zip(&staging.path().join("boom.zip"), &[("boom.sfc", b"BOOM")]);
    let second = write_zip(&staging.path().join("after.zip"), &[("after.sfc", b"OK")]);
    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "Boom",
                "SNES",
                "boom.zip",
                &[file_spec(11, "boom.zip", first.len())],
            ),
        )
        .await;
    harness
        .mount_detail(
            2,
            detail_json(
                2,
                "After",
                "SNES",
                "after.zip",
                &[file_spec(21, "after.zip", second.len())],
            ),
        )
        .await;
    // Long enough that the progress emission clears the 100 ms notify
    // throttle, so the listener is reached from inside the download task.
    harness.mount_content(1, "boom.zip", first, 300).await;
    harness.mount_content(2, "after.zip", second, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let boom_id = harness.newest_entry_id();
    let entry = harness.wait_terminal(boom_id).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert!(!entry.error.is_empty());
    assert!(fired.load(Ordering::SeqCst), "the listener never panicked");

    // The download slot was freed, so the subsystem still works.
    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    let after_id = harness.newest_entry_id();
    let entry = harness.wait_terminal(after_id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert!(harness.library.join("SNES/after/after.sfc").is_file());
}

// --- uninstall --------------------------------------------------------------

#[tokio::test]
async fn uninstall_removes_the_extracted_dir_and_the_row() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono.zip"),
        &[("game.sfc", b"ROMDATA")],
    );
    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "Chrono Trigger",
                "SNES",
                "chrono.zip",
                &[file_spec(11, "chrono.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(1, "chrono.zip", bytes, 0).await;
    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    harness.wait_terminal(id).await;

    let extracted_dir = harness.library.join("SNES/chrono");
    assert!(extracted_dir.is_dir());

    harness.service.uninstall(1).unwrap();
    assert!(!extracted_dir.exists());
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn uninstall_removes_a_multi_file_game_dir() {
    let harness = Harness::new().await;
    harness
        .mount_detail(
            2,
            detail_json(
                2,
                "Final Fantasy VII",
                "PlayStation",
                "Final Fantasy VII",
                &[file_spec(21, "disc1.bin", 5), file_spec(22, "game.m3u", 9)],
            ),
        )
        .await;
    harness
        .mount_content(2, "disc1.bin", b"DISC1".to_vec(), 0)
        .await;
    harness
        .mount_content(2, "game.m3u", b"disc1.bin".to_vec(), 0)
        .await;
    harness
        .service
        .install(harness.client.clone(), 2)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    harness.wait_terminal(id).await;

    let game_dir = harness.library.join("PlayStation/Final Fantasy VII");
    assert!(game_dir.is_dir());
    harness.service.uninstall(2).unwrap();
    assert!(!game_dir.exists());
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn uninstall_succeeds_when_a_subdirectory_is_read_only() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("locked.zip"),
        &[("sub/game.sfc", b"ROMDATA")],
    );
    harness
        .mount_detail(
            8,
            detail_json(
                8,
                "Locked",
                "SNES",
                "locked.zip",
                &[file_spec(81, "locked.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(8, "locked.zip", bytes, 0).await;
    harness
        .service
        .install(harness.client.clone(), 8)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let extracted_dir = harness.library.join("SNES/locked");
    let sub = extracted_dir.join("sub");
    assert!(sub.join("game.sfc").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sub, fs::Permissions::from_mode(0o555)).unwrap();
    }

    harness.service.uninstall(8).unwrap();
    assert!(!extracted_dir.exists());
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn uninstall_of_an_unknown_rom_reports_not_installed() {
    let harness = Harness::new().await;
    let err = harness.service.uninstall(404).unwrap_err();
    assert!(
        matches!(&err, LibraryError::Registry(msg) if msg == "not installed"),
        "unexpected error: {err}"
    );
}

// --- configuration ----------------------------------------------------------

#[tokio::test]
async fn install_without_a_library_path_errors_before_any_request() {
    let harness = Harness::without_library_path().await;
    let err = harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap_err();
    assert!(matches!(err, LibraryError::LibraryPathUnset));
    assert!(harness.service.snapshot().entries.is_empty());
    assert!(harness.server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn uninstall_without_a_library_path_errors() {
    let harness = Harness::without_library_path().await;
    let err = harness.service.uninstall(1).unwrap_err();
    assert!(matches!(err, LibraryError::LibraryPathUnset));
}

// --- typed drawer entries ----------------------------------------------------

#[tokio::test]
async fn entry_carries_kind_base() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono.zip"),
        &[("game.sfc", b"ROMDATA")],
    );
    harness
        .mount_detail(
            1,
            detail_json(
                1,
                "Chrono Trigger",
                "SNES",
                "chrono.zip",
                &[file_spec(11, "chrono.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(1, "chrono.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    assert_eq!(harness.entry(id).kind, "base");
    assert_eq!(harness.entry(id).job, "game");
    harness.wait_terminal(id).await;
    assert_eq!(
        harness.entry(id).kind,
        "base",
        "the kind survives every status change"
    );
}

#[tokio::test]
async fn cancel_for_rom_cancels_the_live_entry() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("slow.zip"),
        &[("game.sfc", b"ROMDATA")],
    );
    harness
        .mount_detail(
            4,
            detail_json(
                4,
                "Slow Game",
                "SNES",
                "slow.zip",
                &[file_spec(41, "slow.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(4, "slow.zip", bytes, 3000).await;

    harness
        .service
        .install(harness.client.clone(), 4)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    harness
        .wait_for(id, |e| e.status == DownloadStatus::Downloading)
        .await;

    // An unrelated rom id is ignored; the live entry is not touched.
    harness.service.cancel_for_rom(999);
    assert_eq!(harness.entry(id).status, DownloadStatus::Downloading);

    harness.service.cancel_for_rom(4);
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Cancelled);
}

#[tokio::test]
async fn admit_external_and_complete_external_round_trip() {
    let harness = Harness::new().await;

    let ok = harness
        .service
        .admit_external("PS3 Firmware", "PlayStation 3");
    let entry = harness.entry(ok);
    assert_eq!(entry.kind, "firmware");
    assert_eq!(entry.job, "firmware");
    assert_eq!(entry.status, DownloadStatus::Downloading);
    assert_eq!(entry.title, "PS3 Firmware");
    assert_eq!(entry.platform, "PlayStation 3");

    harness.service.complete_external(ok, "");
    assert_eq!(harness.entry(ok).status, DownloadStatus::Completed);
    assert_eq!(harness.entry(ok).error, "");

    let bad = harness
        .service
        .admit_external("PS3 Firmware", "PlayStation 3");
    harness.service.complete_external(bad, "download failed");
    assert_eq!(harness.entry(bad).status, DownloadStatus::Failed);
    assert_eq!(harness.entry(bad).error, "download failed");

    // Both transitions reached the notify listener.
    let snapshots = harness.snapshots.lock().unwrap();
    assert!(snapshots
        .last()
        .unwrap()
        .entries
        .iter()
        .any(|e| e.id == bad && e.status == DownloadStatus::Failed));
}

// --- native (Windows) installs ----------------------------------------------

/// [`detail_json`] with the metadata `game.json` is allowed to fill in left
/// blank, so an `apply_game_json` write is visible rather than being skipped
/// as "the row already has a value".
fn detail_json_without_metadata(
    id: i64,
    name: &str,
    platform: &str,
    fs_name: &str,
    files: &[FileSpec],
) -> serde_json::Value {
    let mut detail = detail_json(id, name, platform, fs_name, files);
    detail["revision"] = json!(null);
    detail["tags"] = json!([]);
    detail["metadatum"]["first_release_date"] = json!(null);
    detail
}

#[tokio::test]
async fn native_install_lays_out_game_dir_prefix_and_game_json() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("mygame.zip"),
        &[
            ("MyGame/mygame.exe", b"MZ"),
            ("readme.txt", b"read me first"),
        ],
    );
    let sidecar = br#"{"version": "1.2", "year": 2001, "tags": ["a", "b"], "included_dlc": ["x"]}"#;

    harness
        .mount_detail(
            5,
            detail_json_without_metadata(
                5,
                "My Game",
                "Windows",
                "mygame.zip",
                &[
                    file_spec(51, "mygame.zip", bytes.len()),
                    file_spec(52, "game.json", sidecar.len()),
                ],
            ),
        )
        .await;
    harness.mount_content(5, "mygame.zip", bytes, 0).await;
    harness
        .mount_content(5, "game.json", sidecar.to_vec(), 0)
        .await;

    harness
        .service
        .install(harness.client.clone(), 5)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let game_dir = harness.library.join("Windows/My Game");
    let extracted_dir = game_dir.join("game");
    assert!(extracted_dir.join("MyGame/mygame.exe").is_file());
    assert!(
        !game_dir.join("mygame.zip").exists(),
        "the archive is deleted once it has been extracted"
    );
    #[cfg(unix)]
    assert!(game_dir.join("prefix").is_dir());

    let row = harness.registry.find(Some(5), "", "").unwrap().unwrap();
    assert_eq!(row.native_game_dir, game_dir.to_string_lossy());
    assert_eq!(row.extracted_dir, extracted_dir.to_string_lossy());
    assert_eq!(
        row.extracted_path,
        extracted_dir.join("MyGame/mygame.exe").to_string_lossy()
    );
    assert_eq!(row.archive_path, "");
    assert_eq!(row.multi_file_game_dir, "");
    #[cfg(unix)]
    assert_eq!(
        row.native_wineprefix,
        game_dir.join("prefix").to_string_lossy()
    );
    assert_eq!(row.revision, "1.2");
    assert_eq!(row.first_release_date, "2001");
    assert_eq!(row.tags, "a, b");
    assert_eq!(row.included_dlc, "[\"x\"]");
}

#[tokio::test]
async fn native_non_archive_payload_installs_as_direct_file() {
    let harness = Harness::new().await;

    harness
        .mount_detail(
            6,
            detail_json(
                6,
                "Disc Game",
                "Windows",
                "game.iso",
                &[file_spec(61, "game.iso", 4)],
            ),
        )
        .await;
    harness
        .mount_content(6, "game.iso", b"ISO!".to_vec(), 0)
        .await;

    harness
        .service
        .install(harness.client.clone(), 6)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let game_dir = harness.library.join("Windows/Disc Game");
    let archive = game_dir.join("game.iso");
    assert!(archive.is_file(), "D13: the payload IS the install");
    assert!(!game_dir.join("game").exists(), "nothing was extracted");

    let row = harness.registry.find(Some(6), "", "").unwrap().unwrap();
    assert_eq!(row.archive_path, archive.to_string_lossy());
    assert_eq!(row.native_game_dir, game_dir.to_string_lossy());
    assert_eq!(row.extracted_dir, "");
    assert_eq!(row.extracted_path, "");
    assert_eq!(row.native_wineprefix, "");
}

#[tokio::test]
async fn uninstall_native_removes_the_game_dir() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("mygame.zip"),
        &[("MyGame/mygame.exe", b"MZ")],
    );
    harness
        .mount_detail(
            7,
            detail_json(
                7,
                "My Game",
                "Windows",
                "mygame.zip",
                &[file_spec(71, "mygame.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(7, "mygame.zip", bytes, 0).await;
    harness
        .service
        .install(harness.client.clone(), 7)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let game_dir = harness.library.join("Windows/My Game");
    assert!(game_dir.is_dir());

    harness.service.uninstall(7).unwrap();
    assert!(
        !game_dir.exists(),
        "the whole home directory goes, prefix included"
    );
    assert!(harness.service.installed().unwrap().is_empty());
}

// --- PlayStation 3 installs --------------------------------------------------

/// The `dev_hdd0` root a PS3 install falls back to when no emulator is
/// configured: `<library>/PlayStation 3/.vfs/dev_hdd0`, canonicalized the
/// way the VFS reader canonicalizes it.
fn library_dev_hdd0(library: &Path) -> PathBuf {
    let ps3 = library.join("PlayStation 3");
    fs::canonicalize(&ps3)
        .unwrap_or(ps3)
        .join(".vfs")
        .join("dev_hdd0")
}

#[tokio::test]
async fn ps3_install_routes_into_the_library_vfs_fallback() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("demons.zip"),
        &[("BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN", b"EBOOT")],
    );

    harness
        .mount_detail(
            10,
            detail_json(
                10,
                "Demons Souls",
                "PlayStation 3",
                "demons.zip",
                &[file_spec(101, "demons.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(10, "demons.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 10)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let dev_hdd0 = library_dev_hdd0(&harness.library);
    let routed = dev_hdd0.join("game/BLUS30336");
    assert!(routed.join("PS3_GAME/USRDIR/EBOOT.BIN").is_file());
    assert!(
        !harness.library.join("PlayStation 3/demons").exists(),
        "the staging directory is removed once routing succeeded"
    );
    assert!(!harness.library.join("PlayStation 3/demons.zip").exists());

    let row = harness.registry.find(Some(10), "", "").unwrap().unwrap();
    assert_eq!(row.ps3_game_id, "BLUS30336");
    assert_eq!(row.extracted_dir, routed.to_string_lossy());
    assert_eq!(row.extracted_path, routed.to_string_lossy());
    assert_eq!(row.ps3_trophy_paths, "[]");
    assert_eq!(row.ps3_iso_path, "");
}

#[tokio::test]
async fn ps3_iso_only_archive_short_circuits() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(&staging.path().join("disc.zip"), &[("game.iso", b"ISO!")]);

    harness
        .mount_detail(
            11,
            detail_json(
                11,
                "Disc Only",
                "PlayStation 3",
                "disc.zip",
                &[file_spec(111, "disc.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(11, "disc.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 11)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let iso = harness.library.join("PlayStation 3/game.iso");
    assert!(iso.is_file(), "the ISO is moved next to the archive");
    assert!(!harness.library.join("PlayStation 3/disc").exists());
    assert!(!harness.library.join("PlayStation 3/disc.zip").exists());

    let row = harness.registry.find(Some(11), "", "").unwrap().unwrap();
    assert_eq!(row.ps3_iso_path, iso.to_string_lossy());
    assert_eq!(row.extracted_path, iso.to_string_lossy());
    assert_eq!(row.extracted_dir, "");
    assert_eq!(row.ps3_game_id, "");
}

#[tokio::test]
async fn ps3_game_id_missing_fails() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("junk.zip"),
        &[("misc/readme.txt", b"nothing to see")],
    );

    harness
        .mount_detail(
            12,
            detail_json(
                12,
                "Mystery Disc",
                "PlayStation 3",
                "junk.zip",
                &[file_spec(121, "junk.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(12, "junk.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 12)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(
        entry.error,
        "No PS3 game ID found in archive for Mystery Disc"
    );
    assert!(
        harness.library.join("PlayStation 3/junk.zip").is_file(),
        "a failed finalize keeps the archive so a retry skips the download"
    );
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn ps3_archive_of_empty_directories_reports_no_rom_file() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(&staging.path().join("hollow.zip"), &[("empty/", b"")]);

    harness
        .mount_detail(
            13,
            detail_json(
                13,
                "Hollow",
                "PlayStation 3",
                "hollow.zip",
                &[file_spec(131, "hollow.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(13, "hollow.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 13)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(entry.error, "Archive extracted but no ROM file was found");
    assert!(!harness.library.join("PlayStation 3/hollow").exists());
}

#[tokio::test]
async fn uninstall_ps3_removes_iso_trophies_and_the_routed_dir() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("trophies.zip"),
        &[
            ("BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN", b"EBOOT"),
            ("NPWR12345/TROPCONF.SFM", b"TROPHY"),
        ],
    );

    harness
        .mount_detail(
            14,
            detail_json(
                14,
                "Trophy Game",
                "PlayStation 3",
                "trophies.zip",
                &[file_spec(141, "trophies.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(14, "trophies.zip", bytes, 0).await;
    harness
        .service
        .install(harness.client.clone(), 14)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let dev_hdd0 = library_dev_hdd0(&harness.library);
    let routed = dev_hdd0.join("game/BLUS30336");
    let trophy = dev_hdd0.join("home/00000001/trophy/NPWR12345");
    assert!(routed.is_dir());
    assert!(trophy.is_dir());

    let row = harness.registry.find(Some(14), "", "").unwrap().unwrap();
    assert!(
        row.ps3_trophy_paths.contains("NPWR12345"),
        "trophy paths: {}",
        row.ps3_trophy_paths
    );

    // A stale ISO recorded on the row is removed too.
    let iso = harness.library.join("PlayStation 3/stale.iso");
    fs::write(&iso, b"ISO!").unwrap();
    let mut updated = row.clone();
    updated.ps3_iso_path = iso.to_string_lossy().into_owned();
    harness.registry.upsert(&updated).unwrap();

    harness.service.uninstall(14).unwrap();
    assert!(!iso.exists());
    assert!(!trophy.exists());
    assert!(!routed.exists());
    assert!(harness.service.installed().unwrap().is_empty());
}

#[tokio::test]
async fn games_yml_written_for_ps3_with_configured_rpcs3() {
    // A configured RPCS3 whose `portable/` directory holds a `vfs.yml`
    // naming an explicit dev_hdd0: both the data root (where `games.yml`
    // goes) and the VFS root are then fixed by the config, not guessed.
    let emulator_dir = tempfile::tempdir().unwrap();
    let executable = emulator_dir.path().join("rpcs3");
    fs::write(&executable, b"#!/bin/sh\n").unwrap();
    let portable = emulator_dir.path().join("portable");
    fs::create_dir_all(portable.join("config")).unwrap();
    let dev_hdd0 = emulator_dir.path().join("hdd0");
    fs::create_dir_all(&dev_hdd0).unwrap();
    fs::write(
        portable.join("config/vfs.yml"),
        format!(
            "/dev_hdd0/: \"{}/\"\n/games/: \"{}/\"\n",
            dev_hdd0.to_string_lossy(),
            emulator_dir.path().join("games").to_string_lossy()
        ),
    )
    .unwrap();

    let harness = Harness::with_config(&format!(
        "[default_emulators]\n\"PlayStation 3\" = \"RPCS3 (Playstation 3)\"\n\n\
         [[emulators]]\nname = \"RPCS3 (Playstation 3)\"\npath = {:?}\nargs = \"\"\n",
        executable.to_string_lossy()
    ))
    .await;

    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("demons.zip"),
        &[("BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN", b"EBOOT")],
    );
    harness
        .mount_detail(
            15,
            detail_json(
                15,
                "Demons Souls",
                "PlayStation 3",
                "demons.zip",
                &[file_spec(151, "demons.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(15, "demons.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 15)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let row = harness.registry.find(Some(15), "", "").unwrap().unwrap();
    assert_eq!(row.ps3_game_id, "BLUS30336");
    assert!(
        row.extracted_dir.contains("hdd0"),
        "routed into the configured VFS, not the library fallback: {}",
        row.extracted_dir
    );

    let games_yml = fs::read_to_string(portable.join("config/games.yml")).unwrap();
    assert!(
        games_yml.contains("BLUS30336:"),
        "games.yml should name the installed game: {games_yml}"
    );
}

// --- PlayStation 4 installs --------------------------------------------------

#[tokio::test]
async fn ps4_install_detects_title_id_and_prefers_eboot() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("ps4game.zip"),
        &[
            ("CUSA12345/sce_sys/param.sfo", b"SFO"),
            ("CUSA12345/eboot.bin", b"EBOOT"),
        ],
    );

    harness
        .mount_detail(
            16,
            detail_json(
                16,
                "PS4 Game",
                "PlayStation 4",
                "ps4game.zip",
                &[file_spec(161, "ps4game.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(16, "ps4game.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 16)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let extracted_dir = harness.library.join("PlayStation 4/ps4game");
    let row = harness.registry.find(Some(16), "", "").unwrap().unwrap();
    assert_eq!(row.ps4_game_id, "CUSA12345");
    assert_eq!(row.extracted_dir, extracted_dir.to_string_lossy());
    assert!(
        row.extracted_path.ends_with("eboot.bin"),
        "eboot.bin wins over the generic ranking: {}",
        row.extracted_path
    );
    assert!(!harness.library.join("PlayStation 4/ps4game.zip").exists());
}

// --- post-install hook -------------------------------------------------------

#[tokio::test]
async fn the_game_finalized_hook_sees_the_written_row() {
    let harness = Harness::new().await;
    let seen: Arc<Mutex<Vec<InstalledGame>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    harness
        .service
        .set_game_finalized_hook(Arc::new(move |row| sink.lock().unwrap().push(row)));

    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono.zip"),
        &[("game.sfc", b"ROMDATA")],
    );
    harness
        .mount_detail(
            17,
            detail_json(
                17,
                "Chrono Trigger",
                "SNES",
                "chrono.zip",
                &[file_spec(171, "chrono.zip", bytes.len())],
            ),
        )
        .await;
    harness.mount_content(17, "chrono.zip", bytes, 0).await;

    harness
        .service
        .install(harness.client.clone(), 17)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].title, "Chrono Trigger");
    assert_eq!(seen[0].rom_id, Some(17));
    assert_eq!(
        seen[0].extracted_dir,
        harness.library.join("SNES/chrono").to_string_lossy(),
        "the hook sees the finished row, not a half-filled one"
    );
}

// --- platform ids ------------------------------------------------------------

#[tokio::test]
async fn platform_ids_default_to_empty_and_round_trip() {
    let harness = Harness::new().await;
    assert!(harness.service.platform_ids().is_empty());

    let mut ids = std::collections::BTreeMap::new();
    ids.insert("PlayStation 3".to_string(), 12i64);
    harness.service.set_platform_ids(ids.clone());
    assert_eq!(harness.service.platform_ids(), ids);
}
