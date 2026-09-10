//! Wiremock end-to-end tests for [`grid_core::firmware::install_platform_firmware`].
//! Maps `tests/test_firmware_install.py` classes `FirmwareRoutingTests`,
//! `FirmwareInstallTests`, `FirmwareZipArchiveBehaviorTests`,
//! `FirmwareExtractZipWithPathsTests` at the client-integration layer (the
//! routing/keep-zip/write-dispatch unit cases live inline in
//! `grid-core/src/firmware/{mod.rs,write.rs}`).

use std::fs;
use std::io::Write;

use grid_core::firmware::{install_platform_firmware, FirmwareOptions, FirmwareTarget};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

fn client(server: &MockServer) -> RommClient {
    RommClient::new(&server.uri(), token_cred()).unwrap()
}

fn plain(path: &std::path::Path) -> FirmwareTarget {
    FirmwareTarget {
        path: path.to_path_buf(),
        keywords: None,
    }
}

fn routed(path: &std::path::Path, keywords: &[&str]) -> FirmwareTarget {
    FirmwareTarget {
        path: path.to_path_buf(),
        keywords: Some(keywords.iter().map(|s| s.to_string()).collect()),
    }
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for &(name, content) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }
    buf
}

#[tokio::test]
async fn no_targets_never_fetches() {
    let server = MockServer::start().await;
    // No mock mounted at all: if install_platform_firmware fetched anything,
    // the request would 404 out of wiremock and the test would still pass
    // (fetch would just error), so this also asserts zero requests reached
    // the server.
    let result =
        install_platform_firmware(&client(&server), 19, &[], FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn empty_list_no_warnings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
}

#[tokio::test]
async fn single_file_written() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "gc-ntsc-12-101.bin"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/gc-ntsc-12-101.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"FIRMWAREDATA".to_vec()))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
    assert_eq!(
        fs::read(dir.path().join("gc-ntsc-12-101.bin")).unwrap(),
        b"FIRMWAREDATA"
    );
}

#[tokio::test]
async fn skip_existing_still_downloads_but_does_not_write() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "gc-ntsc-12-101.bin"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/gc-ntsc-12-101.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"NEWDATA".to_vec()))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("gc-ntsc-12-101.bin");
    fs::write(&dest, b"ORIGINAL").unwrap();

    let targets = [plain(dir.path())];
    let opts = FirmwareOptions {
        skip_existing: true,
        extract_zip_with_paths: false,
    };
    let result = install_platform_firmware(&client(&server), 19, &targets, opts).await;
    assert_eq!(result, Vec::<String>::new());
    assert_eq!(fs::read(&dest).unwrap(), b"ORIGINAL");

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .any(|r| r.url.path() == "/api/firmware/3/content/gc-ntsc-12-101.bin"),
        "expected the content GET even though the write was skipped: {requests:?}"
    );
}

#[tokio::test]
async fn zip_extracted_flat() {
    let server = MockServer::start().await;
    let data = zip_bytes(&[("IPL.bin", b"IPLDATA")]);
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "gc_ntsc.zip"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/gc_ntsc.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
    assert_eq!(fs::read(dir.path().join("IPL.bin")).unwrap(), b"IPLDATA");
    assert!(!dir.path().join("gc_ntsc.zip").exists());
}

#[tokio::test]
async fn download_once_for_multiple_dirs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "gc-ntsc-12-101.bin"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/gc-ntsc-12-101.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"FIRMWAREDATA".to_vec()))
        .expect(1)
        .mount(&server)
        .await;

    let dir_one = tempfile::tempdir().unwrap();
    let dir_two = tempfile::tempdir().unwrap();
    let targets = [plain(dir_one.path()), plain(dir_two.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
    assert_eq!(
        fs::read(dir_one.path().join("gc-ntsc-12-101.bin")).unwrap(),
        b"FIRMWAREDATA"
    );
    assert_eq!(
        fs::read(dir_two.path().join("gc-ntsc-12-101.bin")).unwrap(),
        b"FIRMWAREDATA"
    );
    // `.expect(1)` on the content mock is verified when `server` drops.
}

#[tokio::test]
async fn download_error_is_a_warning() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "gc.bin"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/gc.bin"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result.len(), 1);
    assert!(
        result[0].starts_with("Failed to download firmware gc.bin: "),
        "unexpected warning: {}",
        result[0]
    );
}

#[tokio::test]
async fn fetch_error_is_a_warning() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result.len(), 1);
    assert!(
        result[0].starts_with("Firmware fetch failed for platform 19: "),
        "unexpected warning: {}",
        result[0]
    );
}

#[tokio::test]
async fn record_missing_id_or_blank_name_skipped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": ""},
            {"file_name": "no-id.bin"},
            {"id": "not-an-int", "file_name": "bad-id.bin"}
        ])))
        .mount(&server)
        .await;
    // No content mock mounted: any attempted download 404s and would show
    // up as a warning, so an empty warning list also proves nothing was
    // downloaded for the skipped records.

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}

/// SECURITY: a hostile server record whose `file_name` escapes the target
/// directory is rejected before the content GET, so it costs no bytes and
/// nothing is written anywhere.
#[tokio::test]
async fn a_traversal_file_name_is_warned_and_never_downloaded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "../../outside.bin"},
            {"id": 4, "file_name": "/etc/outside.bin"},
            {"id": 5, "file_name": "nested/inside.bin"}
        ])))
        .mount(&server)
        .await;
    // Every content route is mounted and would succeed: an untouched
    // request log is therefore proof the guard runs BEFORE the download,
    // not after it.
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/../../outside.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"HOSTILE".to_vec()))
        .mount(&server)
        .await;

    let root = tempfile::tempdir().unwrap();
    let target_dir = root.path().join("target");
    fs::create_dir_all(&target_dir).unwrap();
    let targets = [plain(&target_dir)];

    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(
        result,
        vec![
            format!(
                "Failed to write firmware ../../outside.bin to {}: invalid firmware file name",
                target_dir.display()
            ),
            format!(
                "Failed to write firmware /etc/outside.bin to {}: invalid firmware file name",
                target_dir.display()
            ),
            format!(
                "Failed to write firmware nested/inside.bin to {}: invalid firmware file name",
                target_dir.display()
            ),
        ]
    );
    assert!(fs::read_dir(&target_dir).unwrap().next().is_none());
    // Only the listing GET reached the server.
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests.iter().all(|r| r.url.path() == "/api/firmware"),
        "a rejected record was downloaded: {requests:?}"
    );
    // Nothing landed beside the target directory either.
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn routed_zip_to_correct_region_dir() {
    let server = MockServer::start().await;
    let data = zip_bytes(&[("IPL.bin", b"IPLDATA")]);
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": "gc_ntsc.zip"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/3/content/gc_ntsc.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data))
        .mount(&server)
        .await;

    let root = tempfile::tempdir().unwrap();
    let jap_dir = root.path().join("JAP");
    let usa_dir = root.path().join("USA");
    let eur_dir = root.path().join("EUR");
    let targets = [
        routed(&jap_dir, &["ntsc_j", "ntsc-j", "jap", "jpn"]),
        routed(&usa_dir, &["ntsc", "usa"]),
        routed(&eur_dir, &["pal", "eur"]),
    ];

    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
    assert!(!jap_dir.exists());
    assert!(usa_dir.join("IPL.bin").exists());
    assert!(!eur_dir.exists());
}

/// `extract_zip_with_paths` overrides the keep-as-archive decision even
/// when the zip is routed through a keyword entry whose list contains the
/// exact file name (the case `should_keep_zip` alone would say "keep").
/// Mirrors `FirmwareExtractZipWithPathsTests.test_extract_with_paths_overrides_keep_archive_tuple`.
#[tokio::test]
async fn extract_with_paths_overrides_keep_archive_tuple() {
    let server = MockServer::start().await;
    let file_name = "dolphin-gc-bios.zip";
    let data = zip_bytes(&[
        ("dolphin-emu/User/GC/USA/IPL.bin", b"USA"),
        ("dolphin-emu/User/GC/EUR/IPL.bin", b"EUR"),
    ]);
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 3, "file_name": file_name}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/firmware/3/content/{file_name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [routed(dir.path(), &[file_name])];
    let opts = FirmwareOptions {
        skip_existing: true,
        extract_zip_with_paths: true,
    };
    let result = install_platform_firmware(&client(&server), 19, &targets, opts).await;
    assert_eq!(result, Vec::<String>::new());
    assert!(!dir.path().join(file_name).exists());
    assert!(dir.path().join("dolphin-emu/User/GC/USA/IPL.bin").exists());
    assert!(dir.path().join("dolphin-emu/User/GC/EUR/IPL.bin").exists());
}

#[tokio::test]
async fn non_list_body_yields_no_warnings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"detail": "nope"})),
        )
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
}

#[tokio::test]
async fn fetch_uses_platform_id_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .and(query_param("platform_id", "19"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let targets = [plain(dir.path())];
    let result =
        install_platform_firmware(&client(&server), 19, &targets, FirmwareOptions::default()).await;
    assert_eq!(result, Vec::<String>::new());
}
