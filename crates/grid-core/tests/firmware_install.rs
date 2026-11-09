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

/// A zip whose own name matches no keyword target is routed by its members:
/// RomM's `wiiu.zip` holds the `keys.txt` Cemu's target asks for (user bug
/// 2026-10-30). Only the asked-for member is written, and the manifest
/// records it, so the installed check then reports the firmware installed.
#[tokio::test]
async fn a_zip_is_routed_by_the_members_a_keyword_target_asks_for() {
    use grid_core::firmware::install_platform_firmware_recorded;
    use grid_core::firmware::manifest::{firmware_installed, FirmwareManifest, MANIFEST_FILE};
    use grid_core::romm::FirmwareRecord;

    let server = MockServer::start().await;
    let data = zip_bytes(&[("keys.txt", b"KEYS"), ("readme.md", b"R")]);
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 40, "file_name": "wiiu.zip"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/40/content/wiiu.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let cemu = dir.path().join("cemu");
    let manifest = dir.path().join(MANIFEST_FILE);
    let targets = [routed(&cemu, &["keys.txt"])];
    let records = [FirmwareRecord {
        id: 40,
        file_name: "wiiu.zip".into(),
        ..Default::default()
    }];
    assert!(!firmware_installed(
        &records,
        &targets,
        &FirmwareManifest::load(&manifest)
    ));

    let result = install_platform_firmware_recorded(
        &client(&server),
        42,
        &targets,
        FirmwareOptions::default(),
        Some(&manifest),
    )
    .await;
    assert_eq!(result, Vec::<String>::new());
    assert_eq!(fs::read(cemu.join("keys.txt")).unwrap(), b"KEYS");
    assert!(!cemu.join("readme.md").exists());
    assert!(!cemu.join("wiiu.zip").exists());
    assert!(firmware_installed(
        &records,
        &targets,
        &FirmwareManifest::load(&manifest)
    ));
}

/// A download that does not match the server's stated size or MD5 is not
/// written (as Argosy, RomM's own client, checks), and the warning says why.
#[tokio::test]
async fn a_download_that_fails_the_server_checksum_is_not_written() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 7, "file_name": "scph5501.bin", "file_size_bytes": 4,
             "md5_hash": "00000000000000000000000000000000"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/7/content/scph5501.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"BIOS".to_vec()))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let result = install_platform_firmware(
        &client(&server),
        1,
        &[plain(dir.path())],
        FirmwareOptions::default(),
    )
    .await;
    assert_eq!(result.len(), 1, "{result:?}");
    assert!(result[0].contains("MD5"), "{result:?}");
    assert!(!dir.path().join("scph5501.bin").exists());
}

/// The same file with the server's real size and MD5 installs as before.
#[tokio::test]
async fn a_download_matching_the_server_checksum_is_written() {
    use md5::{Digest, Md5};
    let md5: String = Md5::digest(b"BIOS")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 7, "file_name": "scph5501.bin", "file_size_bytes": 4, "md5_hash": md5}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/7/content/scph5501.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"BIOS".to_vec()))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let result = install_platform_firmware(
        &client(&server),
        1,
        &[plain(dir.path())],
        FirmwareOptions::default(),
    )
    .await;
    assert_eq!(result, Vec::<String>::new());
    assert_eq!(fs::read(dir.path().join("scph5501.bin")).unwrap(), b"BIOS");
}

/// A GameCube IPL goes to the Dolphin region folders its header names: an
/// NTSC dump to `USA` and `JPN`, never `EUR`, and a PAL dump only to `EUR`.
/// The folder it skips still counts as done, so the line reads Installed.
#[tokio::test]
async fn a_gamecube_ipl_lands_only_in_the_region_folders_its_header_names() {
    use grid_core::firmware::install_platform_firmware_recorded;
    use grid_core::firmware::manifest::{firmware_installed, FirmwareManifest, MANIFEST_FILE};
    use grid_core::romm::FirmwareRecord;

    let header = |standard: &str| {
        let mut data = format!(
            "(C) 1999-2003 Nintendo.  All rights reserved.(C) 1999 ArtX Inc.  All rights reserved.{standard} Revision 1.2  "
        )
        .into_bytes();
        data.resize(0x200, 0);
        data
    };
    for (standard, wanted, skipped) in [
        ("NTSC", &["USA", "JPN"][..], &["EUR"][..]),
        ("PAL ", &["EUR"][..], &["USA", "JPN"][..]),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/firmware"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": 3, "file_name": "IPL.bin"}
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/firmware/3/content/IPL.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(header(standard)))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let gc = dir.path().join("User").join("GC");
        let manifest = dir.path().join(MANIFEST_FILE);
        let targets: Vec<FirmwareTarget> = ["USA", "JPN", "EUR"]
            .iter()
            .map(|region| routed(&gc.join(region), &["ipl"]))
            .collect();
        let result = install_platform_firmware_recorded(
            &client(&server),
            21,
            &targets,
            FirmwareOptions::default(),
            Some(&manifest),
        )
        .await;

        assert_eq!(result, Vec::<String>::new(), "{standard}");
        for region in wanted {
            assert!(
                gc.join(region).join("IPL.bin").is_file(),
                "{standard}: {region}"
            );
        }
        for region in skipped {
            assert!(
                !gc.join(region).join("IPL.bin").exists(),
                "{standard}: {region}"
            );
        }
        let records = [FirmwareRecord {
            id: 3,
            file_name: "IPL.bin".into(),
            ..Default::default()
        }];
        assert!(
            firmware_installed(&records, &targets, &FirmwareManifest::load(&manifest)),
            "{standard}"
        );
    }
}

/// Every file under `root`, as (path relative to `root`, bytes), sorted.
fn tree(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

#[tokio::test]
async fn a_pack_source_routes_its_files_to_the_same_targets_as_the_server() {
    use grid_core::backup::pack::{PackFirmware, PackFirmwareFile};
    use grid_core::firmware::{install_platform_firmware_from, PackFirmwareSource};

    let bios = b"PS1BIOS".to_vec();
    let naomi = zip_bytes(&[("epr-21576.ic27", b"NAOMI")]);
    let ntsc = zip_bytes(&[("IPL.bin", b"NTSC IPL")]);
    let files: [(&str, &[u8]); 3] = [
        ("scph5501.bin", &bios),
        ("naomi.zip", &naomi),
        ("gc-ntsc.zip", &ntsc),
    ];

    // The server serves the three files.
    let server = MockServer::start().await;
    let records: Vec<serde_json::Value> = files
        .iter()
        .enumerate()
        .map(|(i, (name, data))| {
            serde_json::json!({"id": i + 10, "file_name": name, "file_size_bytes": data.len()})
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(records))
        .mount(&server)
        .await;
    for (i, (name, data)) in files.iter().enumerate() {
        Mock::given(method("GET"))
            .and(path(format!("/api/firmware/{}/content/{name}", i + 10)))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(data.to_vec()))
            .mount(&server)
            .await;
    }

    // The drive holds the same three files under gridpack/firmware/ps/.
    let drive = tempfile::tempdir().unwrap();
    let fw_dir = drive.path().join("gridpack").join("firmware").join("ps");
    fs::create_dir_all(&fw_dir).unwrap();
    let mut pack_files = Vec::new();
    for (name, data) in &files {
        fs::write(fw_dir.join(name), data).unwrap();
        pack_files.push(PackFirmwareFile {
            name: name.to_string(),
            path: format!("firmware/ps/{name}"),
            size: data.len() as u64,
            md5: String::new(),
        });
    }
    let source = PackFirmwareSource {
        mount: drive.path().to_path_buf(),
        entry: PackFirmware {
            platform: "ps".into(),
            platform_name: "PlayStation".into(),
            files: pack_files,
            extra: Default::default(),
        },
    };

    let targets_in = |root: &std::path::Path| {
        vec![
            plain(&root.join("bios")),
            routed(&root.join("naomi"), &["naomi.zip"]),
            routed(&root.join("GC").join("USA"), &["ntsc"]),
        ]
    };
    let from_server = tempfile::tempdir().unwrap();
    let from_drive = tempfile::tempdir().unwrap();
    let server_warnings = install_platform_firmware(
        &client(&server),
        19,
        &targets_in(from_server.path()),
        FirmwareOptions::default(),
    )
    .await;
    let drive_warnings = install_platform_firmware_from(
        &source,
        &targets_in(from_drive.path()),
        FirmwareOptions::default(),
        None,
    )
    .await;

    assert_eq!(server_warnings, Vec::<String>::new());
    assert_eq!(drive_warnings, Vec::<String>::new());
    let placed = tree(from_drive.path());
    assert!(!placed.is_empty());
    assert_eq!(placed, tree(from_server.path()));
}

#[tokio::test]
async fn a_pack_source_never_reads_outside_gridpack() {
    use grid_core::backup::pack::{PackFirmware, PackFirmwareFile};
    use grid_core::firmware::{install_platform_firmware_from, PackFirmwareSource};

    let drive = tempfile::tempdir().unwrap();
    fs::create_dir_all(drive.path().join("gridpack")).unwrap();
    fs::write(drive.path().join("secret.bin"), b"NOT FIRMWARE").unwrap();
    let entry = |path: &str| PackFirmware {
        platform: "ps".into(),
        platform_name: "PlayStation".into(),
        files: vec![PackFirmwareFile {
            name: "scph5501.bin".into(),
            path: path.into(),
            size: 12,
            md5: String::new(),
        }],
        extra: Default::default(),
    };
    let absolute = drive
        .path()
        .join("secret.bin")
        .to_string_lossy()
        .into_owned();
    for path in [
        "../secret.bin",
        "firmware/../../secret.bin",
        absolute.as_str(),
    ] {
        let source = PackFirmwareSource {
            mount: drive.path().to_path_buf(),
            entry: entry(path),
        };
        let out = tempfile::tempdir().unwrap();
        let warnings = install_platform_firmware_from(
            &source,
            &[plain(out.path())],
            FirmwareOptions::default(),
            None,
        )
        .await;
        assert_eq!(warnings.len(), 1, "{path}: {warnings:?}");
        assert!(!out.path().join("scph5501.bin").exists(), "{path}");
    }
}
