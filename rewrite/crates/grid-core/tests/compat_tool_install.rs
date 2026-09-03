//! `InstallService::install_compat_tool`: managed compat-tool installs land
//! under `compat::managed_root`, write a `CompatToolInstall` config record
//! (D7) and fire the compat-tools hook, without touching `[[emulators]]` or
//! the SQLite registry.
//!
//! Its own test binary, on purpose (same pattern as `launch_native_wine.rs`):
//! `compat::managed_root` reads `GRID_LAUNCHER_DATA_DIR` (D15), and
//! `std::env::set_var` is process-global — `test_env`'s lock/guard live in
//! `grid_core` as `pub(crate)`, unreachable from an integration test. One
//! test in one binary means no sibling test can observe the mutated env var.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::Config;
use grid_core::launch::profiles::EmulatorProfile;
use grid_core::library::queue::DownloadStatus;
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A gzipped tar at `path` from `(name, content, mode)` entries, returning
/// its bytes — the GE-Proton release shape: a versioned top-level directory
/// holding a `proton` entry point.
fn write_tar_gz(path: &std::path::Path, entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let file = fs::File::create(path).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for &(name, content, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(mode);
        header.set_path(name).unwrap();
        header.set_cksum();
        builder.append(&header, content).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
    fs::read(path).unwrap()
}

#[tokio::test]
async fn install_compat_tool_lands_under_the_managed_root_and_records_it_in_config() {
    let data_dir_holder = tempfile::tempdir().unwrap();
    let data_dir = data_dir_holder.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();
    // SAFETY (test-only, single test in this binary): `managed_root` reads
    // this var fresh on every call, and nothing else in this process reads
    // or mutates it concurrently.
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", &data_dir);

    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    // No `library_path` is written: `install_compat_tool` never reads the
    // configured library (D15 — the managed root is independent of it), so
    // `Config::load`'s NotFound fallback to `Config::default()` is enough.

    let source = json!({
        "provider": "gitea",
        "owner": "GloriousEggroll",
        "repo": "proton-ge-custom",
        "base_url": server.uri(),
        "release_tag": "GE-Proton9-1",
    });
    let profile = EmulatorProfile {
        name: "GE-Proton".to_string(),
        match_tokens: vec![],
        args: String::new(),
        all_platforms: true,
        platform_keywords: Vec::new(),
        is_compat_tool: true,
        source: Some(source),
        compat_tool_type: "proton".to_string(),
        ..Default::default()
    };

    let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
    let service = InstallService::with_profiles(registry, config_path.clone(), vec![profile]);

    let staging = tempfile::tempdir().unwrap();
    let bytes = write_tar_gz(
        &staging.path().join("ge-proton.tar.gz"),
        &[
            ("GE-Proton9-1/proton", b"#!/bin/sh\n" as &[u8], 0o755),
            ("GE-Proton9-1/version", b"9-1", 0o644),
        ],
    );
    let asset_url = format!("{}/dl/GE-Proton9-1.tar.gz", server.uri());
    Mock::given(method("GET"))
        .and(path(
            "/api/v1/repos/GloriousEggroll/proton-ge-custom/releases/tags/GE-Proton9-1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "GE-Proton9-1",
            "assets": [{
                "name": "GE-Proton9-1.tar.gz",
                "browser_download_url": asset_url,
                "size": bytes.len() as i64,
            }],
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/GE-Proton9-1.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;

    let hook_calls = Arc::new(AtomicUsize::new(0));
    let counted = hook_calls.clone();
    service.set_compat_tools_hook(Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));

    service
        .install_compat_tool("GloriousEggroll/proton-ge-custom".to_string())
        .await
        .unwrap();

    let id = service
        .snapshot()
        .entries
        .first()
        .expect("at least one entry")
        .id;
    let deadline = Instant::now() + Duration::from_secs(30);
    let entry = loop {
        if let Some(entry) = service.snapshot().entries.iter().find(|e| e.id == id) {
            if matches!(
                entry.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            ) {
                break entry.clone();
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting on entry {id}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.job, "emulator");
    assert_eq!(entry.kind, "compat_tool");
    assert_eq!(entry.source_id, "GloriousEggroll/proton-ge-custom");
    assert_eq!(entry.title, "GE-Proton");
    assert_eq!(entry.platform, "Compatibility Tool");

    let managed_root: PathBuf = data_dir.join("compat-tools");
    let install_dir = managed_root.join("GE-Proton-GE-Proton9-1");
    let proton_dir = install_dir.join("GE-Proton9-1");
    assert!(
        proton_dir.join("proton").is_file(),
        "extracted proton entry point missing: {}",
        proton_dir.join("proton").display()
    );

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.compat_tool_installs.len(), 1);
    let install = &config.compat_tool_installs[0];
    assert_eq!(install.name, "GE-Proton");
    assert_eq!(install.path, proton_dir.to_string_lossy());
    assert_eq!(install.source_id, "GloriousEggroll/proton-ge-custom");
    assert_eq!(install.release_tag, "GE-Proton9-1");

    // No `[[emulators]]` entry is ever written for a compat-tool install.
    assert!(config.emulators.is_empty());

    // The registry stays games-only.
    assert!(service.installed().unwrap().is_empty());

    assert_eq!(hook_calls.load(Ordering::SeqCst), 1);
}
