//! `InstallService::install_umu_launcher` and the umu-launcher auto-queue
//! behind a Proton compat-tool install: the managed copy lands under
//! `<compat::managed_root>/umu-launcher/`, is checked against the release
//! API's SHA-256 `digest`, and is recorded in `Config.umu_launcher` with the
//! RESOLVED tag.
//!
//! Its own test binary, on purpose (same pattern as `compat_tool_install.rs`):
//! `compat::managed_root` reads `GRID_LAUNCHER_DATA_DIR` and the "is a
//! umu-run available" check reads `PATH`, and `std::env::set_var` is
//! process-global. One test in one binary means no sibling test can observe
//! the mutated variables, so the scenarios run sequentially inside the same
//! `#[tokio::test]`, sharing one `InstallService` and one config. Every
//! scenario uses its own mock server, so no scenario's routes can answer
//! another's requests.
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::Config;
use grid_core::launch::profiles::EmulatorProfile;
use grid_core::launch::umu::UMU_SOURCE_ID;
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The upstream zipapp tarball's shape: a plain tar holding `umu/umu-run`
/// (written without its executable bit, so the test sees finalize set it)
/// and the `umu/umu_run.py` symlink beside it.
fn zipapp_tar() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let content = b"#!/usr/bin/env python3\n# zipapp stub\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_path("umu/umu-run").unwrap();
    header.set_cksum();
    builder.append(&header, &content[..]).unwrap();

    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_mode(0o777);
    link.set_path("umu/umu_run.py").unwrap();
    link.set_link_name("umu-run").unwrap();
    link.set_cksum();
    builder.append(&link, &[][..]).unwrap();
    builder.into_inner().unwrap()
}

/// A gzipped tar holding a top-level `proton` entry point.
fn proton_tar_gz() -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let content = b"#!/bin/sh\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o755);
    header.set_path("proton").unwrap();
    header.set_cksum();
    builder.append(&header, &content[..]).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn sha256_digest(bytes: &[u8]) -> String {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("sha256:{hex}")
}

/// The umu-launcher source, pointed at `server` (a gitea-shaped release
/// API, so the request reaches the local server).
fn umu_source(server: &MockServer) -> Value {
    json!({
        "provider": "gitea",
        "owner": "Open-Wine-Components",
        "repo": "umu-launcher",
        "base_url": server.uri(),
        "release_tag": "latest",
        "asset_patterns": ["umu-launcher-*-zipapp.tar"],
    })
}

const UMU_RELEASE_PATH: &str = "/api/v1/repos/Open-Wine-Components/umu-launcher/releases/latest";

/// A release payload for `tag` whose only asset is the zipapp tarball at
/// `<server>/dl/<tag>.tar`, with `digest` when it is `Some`.
fn umu_release(server: &MockServer, tag: &str, size: usize, digest: Option<&str>) -> Value {
    let mut asset = json!({
        "name": format!("umu-launcher-{tag}-zipapp.tar"),
        "browser_download_url": format!("{}/dl/{tag}.tar", server.uri()),
        "size": size as i64,
    });
    if let Some(digest) = digest {
        asset["digest"] = json!(digest);
    }
    json!({ "tag_name": tag, "assets": [asset] })
}

fn proton_profile(name: &str, owner: &str, server: &MockServer) -> EmulatorProfile {
    EmulatorProfile {
        name: name.to_string(),
        match_tokens: vec![],
        args: String::new(),
        all_platforms: true,
        platform_keywords: Vec::new(),
        is_compat_tool: true,
        source: Some(json!({
            "provider": "gitea",
            "owner": owner,
            "repo": "proton",
            "base_url": server.uri(),
            "release_tag": "v1",
        })),
        compat_tool_type: "proton".to_string(),
        ..Default::default()
    }
}

/// Mounts `owner/proton`'s `v1` release and its asset on `server`.
async fn mount_proton(server: &MockServer, owner: &str) {
    let bytes = proton_tar_gz();
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v1/repos/{owner}/proton/releases/tags/v1"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "v1",
            "assets": [{
                "name": "proton-v1.tar.gz",
                "browser_download_url": format!("{}/dl/{owner}-proton-v1.tar.gz", server.uri()),
                "size": bytes.len() as i64,
            }],
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{owner}-proton-v1.tar.gz")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(server)
        .await;
}

/// Polls `service`'s snapshot until `id` reaches a terminal status.
async fn wait_terminal(service: &Arc<InstallService>, id: u64) -> DownloadEntry {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(entry) = service.snapshot().entries.iter().find(|e| e.id == id) {
            if matches!(
                entry.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            ) {
                return entry.clone();
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting on entry {id}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The newest row for `source_id`, if any.
fn newest_row(service: &Arc<InstallService>, source_id: &str) -> Option<DownloadEntry> {
    service
        .snapshot()
        .entries
        .into_iter()
        .filter(|e| e.source_id == source_id)
        .max_by_key(|e| e.id)
}

fn requests_to(received: &[wiremock::Request], wanted: &str) -> usize {
    received.iter().filter(|r| r.url.path() == wanted).count()
}

#[tokio::test]
async fn umu_launcher_installs_verifies_its_digest_and_follows_proton_installs() {
    let data_dir_holder = tempfile::tempdir().unwrap();
    let data_dir = data_dir_holder.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();
    // No `umu-run` on PATH: an empty directory is the whole search path.
    let empty_path = tempfile::tempdir().unwrap();
    // SAFETY (test-only, single test in this binary): nothing else in this
    // process reads or mutates these concurrently.
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", &data_dir);
    std::env::set_var("PATH", empty_path.path());
    let install_dir: PathBuf = data_dir.join("compat-tools").join("umu-launcher");
    let umu_run = install_dir.join("umu").join("umu-run");

    let proton_server = MockServer::start().await;
    mount_proton(&proton_server, "first").await;
    mount_proton(&proton_server, "second").await;

    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
    let service = InstallService::with_profiles(
        registry,
        config_path.clone(),
        vec![
            proton_profile("First-Proton", "first", &proton_server),
            proton_profile("Second-Proton", "second", &proton_server),
        ],
    );
    let hook_calls = Arc::new(AtomicUsize::new(0));
    let counted = hook_calls.clone();
    service.set_compat_tools_hook(Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));

    // --- (a) a Proton install queues umu-launcher; its 404 fails only its row

    let missing_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(UMU_RELEASE_PATH))
        .respond_with(ResponseTemplate::new(404))
        .mount(&missing_server)
        .await;
    service.set_umu_source(umu_source(&missing_server));

    service
        .install_compat_tool("first/proton".to_string())
        .await
        .unwrap();
    let proton_row = newest_row(&service, "first/proton").expect("the Proton row exists");
    let umu_row = newest_row(&service, UMU_SOURCE_ID).expect("umu-launcher was auto-queued");
    assert!(
        proton_row.id < umu_row.id,
        "the Proton tool stays first in the queue"
    );
    assert_eq!(umu_row.kind, "compat_tool");
    assert_eq!(umu_row.title, "umu-launcher");
    assert_eq!(umu_row.platform, "Compatibility Tool");

    let proton_done = wait_terminal(&service, proton_row.id).await;
    let umu_failed = wait_terminal(&service, umu_row.id).await;
    assert_eq!(
        proton_done.status,
        DownloadStatus::Completed,
        "{}",
        proton_done.error
    );
    assert_eq!(umu_failed.status, DownloadStatus::Failed);
    assert!(!umu_failed.error.is_empty());
    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.compat_tool_installs.len(), 1);
    assert!(config.umu_launcher.is_none());

    // --- (b) a digest mismatch fails and deletes the archive; Retry fetches again

    let digest_server = MockServer::start().await;
    let bytes = zipapp_tar();
    let good = sha256_digest(&bytes);
    let bad = sha256_digest(b"something else");
    Mock::given(method("GET"))
        .and(path(UMU_RELEASE_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(umu_release(
            &digest_server,
            "1.4.4",
            bytes.len(),
            Some(&bad),
        )))
        .up_to_n_times(1)
        .mount(&digest_server)
        .await;
    Mock::given(method("GET"))
        .and(path(UMU_RELEASE_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(umu_release(
            &digest_server,
            "1.4.4",
            bytes.len(),
            Some(&good),
        )))
        .mount(&digest_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/1.4.4.tar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
        .mount(&digest_server)
        .await;
    service.set_umu_source(umu_source(&digest_server));
    let hooks_before = hook_calls.load(Ordering::SeqCst);

    service.install_umu_launcher().await.unwrap();
    let mismatch_row = newest_row(&service, UMU_SOURCE_ID).unwrap();
    assert_ne!(mismatch_row.id, umu_row.id, "a fresh row");
    let mismatch = wait_terminal(&service, mismatch_row.id).await;
    assert_eq!(mismatch.status, DownloadStatus::Failed);
    assert!(mismatch.error.contains("SHA-256"), "{}", mismatch.error);
    let archive = install_dir.join("umu-launcher-latest.tar");
    assert!(!archive.exists(), "the mismatched archive is deleted");
    assert!(!umu_run.exists(), "nothing was extracted");
    assert!(Config::load(&config_path).unwrap().umu_launcher.is_none());
    assert_eq!(hook_calls.load(Ordering::SeqCst), hooks_before);

    service.retry(None, mismatch_row.id).await.unwrap();
    let retried_row = newest_row(&service, UMU_SOURCE_ID).unwrap();
    assert_ne!(retried_row.id, mismatch_row.id, "retry starts a fresh row");
    let retried = wait_terminal(&service, retried_row.id).await;
    assert_eq!(
        retried.status,
        DownloadStatus::Completed,
        "{}",
        retried.error
    );
    assert!(
        service
            .snapshot()
            .entries
            .iter()
            .all(|e| e.id != mismatch_row.id),
        "the failed row is dismissed by retry"
    );
    let received = digest_server.received_requests().await.unwrap();
    assert_eq!(
        requests_to(&received, "/dl/1.4.4.tar"),
        2,
        "the retry downloads the archive again"
    );

    assert!(umu_run.is_file(), "umu-run missing: {}", umu_run.display());
    let mode = fs::metadata(&umu_run).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "umu-run is executable: {mode:o}");
    assert!(
        fs::symlink_metadata(install_dir.join("umu").join("umu_run.py"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!archive.exists(), "the archive is deleted after extraction");
    let config = Config::load(&config_path).unwrap();
    let record = config.umu_launcher.expect("the umu-launcher record");
    assert_eq!(record.path, umu_run.to_string_lossy());
    assert_eq!(record.release_tag, "1.4.4");
    assert!(config.emulators.is_empty());
    assert_eq!(config.compat_tool_installs.len(), 1);
    assert!(service.installed().unwrap().is_empty());
    assert_eq!(hook_calls.load(Ordering::SeqCst), hooks_before + 1);

    // --- (c) with a managed copy present, a Proton install queues nothing else

    let rows_before = service.snapshot().entries.len();
    service
        .install_compat_tool("second/proton".to_string())
        .await
        .unwrap();
    let second = newest_row(&service, "second/proton").unwrap();
    assert_eq!(service.snapshot().entries.len(), rows_before + 1);
    assert_eq!(
        newest_row(&service, UMU_SOURCE_ID).unwrap().id,
        retried_row.id,
        "no umu-launcher row was added"
    );
    let second_done = wait_terminal(&service, second.id).await;
    assert_eq!(
        second_done.status,
        DownloadStatus::Completed,
        "{}",
        second_done.error
    );

    // --- (d) a release with no digest installs unchecked (Reinstall path)

    let undigested_server = MockServer::start().await;
    let bytes = zipapp_tar();
    Mock::given(method("GET"))
        .and(path(UMU_RELEASE_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(umu_release(
            &undigested_server,
            "1.4.5",
            bytes.len(),
            None,
        )))
        .mount(&undigested_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/1.4.5.tar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&undigested_server)
        .await;
    service.set_umu_source(umu_source(&undigested_server));

    service.install_umu_launcher().await.unwrap();
    let reinstall_row = newest_row(&service, UMU_SOURCE_ID).unwrap();
    let reinstall = wait_terminal(&service, reinstall_row.id).await;
    assert_eq!(
        reinstall.status,
        DownloadStatus::Completed,
        "{}",
        reinstall.error
    );
    let record = Config::load(&config_path)
        .unwrap()
        .umu_launcher
        .expect("the umu-launcher record");
    assert_eq!(
        record.release_tag, "1.4.5",
        "the reinstall re-resolved latest"
    );
    assert_eq!(record.path, umu_run.to_string_lossy());
    assert!(umu_run.is_file());
    assert!(Path::new(&record.path).is_file());
}
