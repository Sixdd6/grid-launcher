//! `InstallService::install_compat_tool`: managed compat-tool installs land
//! under `compat::managed_root`, write a `CompatToolInstall` config record
//! (D7) with the RESOLVED release tag and fire the compat-tools hook,
//! without touching `[[emulators]]` or the SQLite registry — and a failed
//! compat-tool row's Retry routes back through the compat path rather than
//! `install_emulator`.
//!
//! Its own test binary, on purpose (same pattern as `launch_native_wine.rs`
//! and `data_dir.rs`): `compat::managed_root` reads `GRID_LAUNCHER_DATA_DIR`
//! (D15), and `std::env::set_var` is process-global — `test_env`'s
//! lock/guard live in `grid_core` as `pub(crate)`, unreachable from an
//! integration test. One test in one binary means no sibling test can
//! observe the mutated env var, so both scenarios below run sequentially
//! inside the same `#[tokio::test]` (matching `data_dir.rs`'s style),
//! sharing one `InstallService` built from two distinct compat profiles so
//! neither scenario's admissions can dedupe against the other's.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::Config;
use grid_core::launch::profiles::EmulatorProfile;
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A gzipped tar at `path` from `(name, content, mode)` entries, returning
/// its bytes — the GE-Proton release shape: a `proton` entry point, either
/// at the top level or inside a versioned subdirectory.
fn write_tar_gz(path: &Path, entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
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

#[tokio::test]
async fn install_compat_tool_resolves_the_tag_and_retries_through_the_compat_path() {
    let data_dir_holder = tempfile::tempdir().unwrap();
    let data_dir = data_dir_holder.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();
    // SAFETY (test-only, single test in this binary): `managed_root` reads
    // this var fresh on every call, and nothing else in this process reads
    // or mutates it concurrently.
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", &data_dir);
    let managed_root: PathBuf = data_dir.join("compat-tools");

    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    // No `library_path` is written: `install_compat_tool` never reads the
    // configured library (D15 — the managed root is independent of it), so
    // `Config::load`'s NotFound fallback to `Config::default()` is enough.

    // Scenario (a)'s profile: a `latest`-PINNED source, so the configured
    // tag ("latest") and the resolved tag ("GE-Proton9-1") differ — the only
    // way to catch `write_compat_tool_entry` recording the wrong one.
    let ge_proton_source = json!({
        "provider": "gitea",
        "owner": "GloriousEggroll",
        "repo": "proton-ge-custom",
        "base_url": server.uri(),
        "release_tag": "latest",
    });
    let ge_proton = EmulatorProfile {
        name: "GE-Proton".to_string(),
        match_tokens: vec![],
        args: String::new(),
        all_platforms: true,
        platform_keywords: Vec::new(),
        is_compat_tool: true,
        source: Some(ge_proton_source),
        compat_tool_type: "proton".to_string(),
        ..Default::default()
    };

    // Scenario (b)'s profile: a distinct source_id so its admissions never
    // dedupe against scenario (a)'s.
    let cachyos_source = json!({
        "provider": "gitea",
        "owner": "CachyOS",
        "repo": "proton-cachyos",
        "base_url": server.uri(),
        "release_tag": "v1",
    });
    let cachyos = EmulatorProfile {
        name: "Proton-CachyOS".to_string(),
        match_tokens: vec![],
        args: String::new(),
        all_platforms: true,
        platform_keywords: Vec::new(),
        is_compat_tool: true,
        source: Some(cachyos_source),
        compat_tool_type: "proton".to_string(),
        ..Default::default()
    };

    let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
    let service =
        InstallService::with_profiles(registry, config_path.clone(), vec![ge_proton, cachyos]);

    let hook_calls = Arc::new(AtomicUsize::new(0));
    let counted = hook_calls.clone();
    service.set_compat_tools_hook(Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));

    // --- (a) a `latest`-pinned install records the RESOLVED tag -----------

    let staging = tempfile::tempdir().unwrap();
    let ge_bytes = write_tar_gz(
        &staging.path().join("ge-proton.tar.gz"),
        &[
            ("GE-Proton9-1/proton", b"#!/bin/sh\n" as &[u8], 0o755),
            ("GE-Proton9-1/version", b"9-1", 0o644),
        ],
    );
    let ge_asset_url = format!("{}/dl/GE-Proton9-1.tar.gz", server.uri());
    Mock::given(method("GET"))
        .and(path(
            "/api/v1/repos/GloriousEggroll/proton-ge-custom/releases/latest",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "GE-Proton9-1",
            "assets": [{
                "name": "GE-Proton9-1.tar.gz",
                "browser_download_url": ge_asset_url,
                "size": ge_bytes.len() as i64,
            }],
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/GE-Proton9-1.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(ge_bytes))
        .mount(&server)
        .await;

    service
        .install_compat_tool("GloriousEggroll/proton-ge-custom".to_string())
        .await
        .unwrap();
    let ge_id = service
        .snapshot()
        .entries
        .first()
        .expect("at least one entry")
        .id;
    let ge_entry = wait_terminal(&service, ge_id).await;

    assert_eq!(
        ge_entry.status,
        DownloadStatus::Completed,
        "{}",
        ge_entry.error
    );
    assert_eq!(ge_entry.job, "emulator");
    assert_eq!(ge_entry.kind, "compat_tool");
    assert_eq!(ge_entry.source_id, "GloriousEggroll/proton-ge-custom");
    assert_eq!(ge_entry.title, "GE-Proton");
    assert_eq!(ge_entry.platform, "Compatibility Tool");

    // The install directory is named from the CONFIGURED tag ("latest"),
    // exactly like an ordinary emulator's `<name>-<configured tag>` rule.
    let ge_install_dir = managed_root.join("GE-Proton-latest");
    let ge_proton_dir = ge_install_dir.join("GE-Proton9-1");
    assert!(
        ge_proton_dir.join("proton").is_file(),
        "extracted proton entry point missing: {}",
        ge_proton_dir.join("proton").display()
    );

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.compat_tool_installs.len(), 1);
    let ge_install = &config.compat_tool_installs[0];
    assert_eq!(ge_install.name, "GE-Proton");
    assert_eq!(ge_install.path, ge_proton_dir.to_string_lossy());
    assert_eq!(ge_install.source_id, "GloriousEggroll/proton-ge-custom");
    // The RESOLVED tag, not the "latest" pin: `CompatToolInstall.release_tag`
    // must name the concrete release that is actually on disk.
    assert_eq!(ge_install.release_tag, "GE-Proton9-1");

    assert!(config.emulators.is_empty());
    assert!(service.installed().unwrap().is_empty());
    assert_eq!(hook_calls.load(Ordering::SeqCst), 1);

    // --- (b) a failed compat-tool row's Retry stays on the compat path -----

    let cachyos_bytes = write_tar_gz(
        &staging.path().join("proton-cachyos.tar.gz"),
        &[("proton", b"#!/bin/sh\n" as &[u8], 0o755)],
    );
    let cachyos_asset_url = format!("{}/dl/proton-cachyos-v1.tar.gz", server.uri());
    Mock::given(method("GET"))
        .and(path(
            "/api/v1/repos/CachyOS/proton-cachyos/releases/tags/v1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "v1",
            "assets": [{
                "name": "proton-cachyos-v1.tar.gz",
                "browser_download_url": cachyos_asset_url,
                "size": cachyos_bytes.len() as i64,
            }],
        })))
        .mount(&server)
        .await;
    // The asset GET fails once, then succeeds — the first install attempt
    // must fail, and the retry must succeed on the same bytes.
    Mock::given(method("GET"))
        .and(path("/dl/proton-cachyos-v1.tar.gz"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/proton-cachyos-v1.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(cachyos_bytes))
        .mount(&server)
        .await;

    service
        .install_compat_tool("CachyOS/proton-cachyos".to_string())
        .await
        .unwrap();
    let failed_id = service
        .snapshot()
        .entries
        .iter()
        .find(|e| e.source_id == "CachyOS/proton-cachyos")
        .expect("the CachyOS entry exists")
        .id;
    let failed_entry = wait_terminal(&service, failed_id).await;
    assert_eq!(
        failed_entry.status,
        DownloadStatus::Failed,
        "the first attempt must fail on the mocked 500"
    );
    assert!(!failed_entry.error.is_empty());

    // Before the fix, `retry` unconditionally called `install_emulator`,
    // which excludes compat profiles and would fail this with "unknown
    // emulator: CachyOS/proton-cachyos" instead of retrying.
    service.retry(None, failed_id).await.unwrap();
    let retried_id = service
        .snapshot()
        .entries
        .iter()
        .find(|e| e.source_id == "CachyOS/proton-cachyos" && e.id != failed_id)
        .expect("retry starts a fresh entry")
        .id;
    let retried_entry = wait_terminal(&service, retried_id).await;
    assert_eq!(
        retried_entry.status,
        DownloadStatus::Completed,
        "{}",
        retried_entry.error
    );
    assert_eq!(retried_entry.job, "emulator");
    assert_eq!(retried_entry.kind, "compat_tool");
    assert!(
        service.snapshot().entries.iter().all(|e| e.id != failed_id),
        "the failed row is dismissed by retry"
    );

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.compat_tool_installs.len(), 2);
    let cachyos_install = config
        .compat_tool_installs
        .iter()
        .find(|install| install.source_id == "CachyOS/proton-cachyos")
        .expect("the CachyOS compat-tool record was written");
    assert_eq!(cachyos_install.name, "Proton-CachyOS");
    assert_eq!(cachyos_install.release_tag, "v1");
    let cachyos_install_dir = managed_root.join("Proton-CachyOS-v1");
    assert_eq!(cachyos_install.path, cachyos_install_dir.to_string_lossy());
    assert!(cachyos_install_dir.join("proton").is_file());

    // The successful retry fires the hook again, on top of scenario (a)'s
    // single call; the failed first attempt fires it zero times.
    assert_eq!(hook_calls.load(Ordering::SeqCst), 2);
}
