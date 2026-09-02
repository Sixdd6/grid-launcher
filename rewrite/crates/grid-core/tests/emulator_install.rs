//! End-to-end tests for `InstallService::install_emulator`: forge
//! resolution, download, extraction/merge, executable selection and the
//! config entry it writes. Every test runs against a wiremock forge, a
//! tempdir library and a fake config file — no RomM server is involved
//! except in the one retry test that needs a failed *game* row.
//!
//! Catalog profiles are injected with `InstallService::with_profiles` so a
//! profile's `source` block can point at the local mock server. The
//! `github` provider's release API host is hard-coded to `api.github.com`
//! (only the `e2e` feature's request-time redirect can divert it), so the
//! end-to-end flows here use the `gitea` provider, which takes its
//! `base_url` from the source block. The per-download `github` header
//! wiring is covered by `ForgeProvider`'s unit tests in `launch/forge.rs`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::config::Config;
use grid_core::launch::profiles::EmulatorProfile;
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// --- fixtures ---------------------------------------------------------------

/// Builds a zip archive at `path` with `Stored` (uncompressed) entries and
/// returns its bytes.
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

fn profile(name: &str, source: Value) -> EmulatorProfile {
    EmulatorProfile {
        name: name.to_string(),
        match_tokens: vec![name.to_lowercase()],
        args: "-f %rom%".to_string(),
        all_platforms: true,
        platform_keywords: Vec::new(),
        is_compat_tool: false,
        source: Some(source),
    }
}

/// A `gitea` source block for `acme/widget`, tag `v1.0`, served by `base`.
fn gitea_source(base: &str) -> Value {
    json!({
        "provider": "gitea",
        "owner": "acme",
        "repo": "widget",
        "base_url": base,
        "release_tag": "v1.0",
    })
}

/// [`gitea_source`] plus one `acme/extras` supplemental download.
fn gitea_source_with_supplemental(base: &str) -> Value {
    let mut source = gitea_source(base);
    source["supplemental_downloads"] = json!([{
        "provider": "gitea",
        "owner": "acme",
        "repo": "extras",
        "base_url": base,
        "release_tag": "v9",
    }]);
    source
}

/// A one-asset gitea/github release payload.
fn release_json(tag: &str, asset_name: &str, url: &str, size: usize) -> Value {
    json!({
        "tag_name": tag,
        "assets": [{
            "name": asset_name,
            "browser_download_url": url,
            "size": size as i64,
        }],
    })
}

const WIDGET_RELEASE: &str = "/api/v1/repos/acme/widget/releases/tags/v1.0";
const EXTRAS_RELEASE: &str = "/api/v1/repos/acme/extras/releases/tags/v9";

// --- harness ----------------------------------------------------------------

struct Harness {
    server: MockServer,
    _tmp: tempfile::TempDir,
    library: PathBuf,
    config_path: PathBuf,
    service: Arc<InstallService>,
}

impl Harness {
    /// Starts the mock forge first, then builds the profiles from its uri so
    /// a source block can point back at it.
    async fn new(profiles: impl FnOnce(&str) -> Vec<EmulatorProfile>) -> Self {
        Self::with_config(profiles, "").await
    }

    /// `extra_config` is appended to the generated `config.toml` — used to
    /// pre-seed `[[emulators]]` entries.
    async fn with_config(
        profiles: impl FnOnce(&str) -> Vec<EmulatorProfile>,
        extra_config: &str,
    ) -> Self {
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
        let service =
            InstallService::with_profiles(registry, config_path.clone(), profiles(&server.uri()));

        Harness {
            server,
            _tmp: tmp,
            library,
            config_path,
            service,
        }
    }

    fn uri(&self) -> String {
        self.server.uri()
    }

    async fn mount_json(&self, at: &str, body: Value) {
        Mock::given(method("GET"))
            .and(path(at.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&self.server)
            .await;
    }

    async fn mount_bytes(&self, at: &str, body: Vec<u8>, delay_ms: u64) {
        let mut template = ResponseTemplate::new(200).set_body_bytes(body);
        if delay_ms > 0 {
            template = template.set_delay(Duration::from_millis(delay_ms));
        }
        Mock::given(method("GET"))
            .and(path(at.to_string()))
            .respond_with(template)
            .mount(&self.server)
            .await;
    }

    /// Mounts the `acme/widget` release plus its asset bytes at
    /// `/dl/<asset_name>`. The release reports `tag_name` `"v1.0"` — the
    /// same tag the source block configures.
    async fn mount_widget(&self, asset_name: &str, body: Vec<u8>, delay_ms: u64) {
        self.mount_widget_at(WIDGET_RELEASE, "v1.0", asset_name, body, delay_ms)
            .await;
    }

    /// [`Self::mount_widget`] with the release endpoint and the tag the
    /// release RESOLVES to spelled out, so a test can make the resolved tag
    /// differ from the configured one.
    async fn mount_widget_at(
        &self,
        endpoint: &str,
        resolved_tag: &str,
        asset_name: &str,
        body: Vec<u8>,
        delay_ms: u64,
    ) {
        let url = format!("{}/dl/{asset_name}", self.uri());
        self.mount_json(
            endpoint,
            release_json(resolved_tag, asset_name, &url, body.len()),
        )
        .await;
        self.mount_bytes(&format!("/dl/{asset_name}"), body, delay_ms)
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

    async fn wait_terminal(&self, id: u64) -> DownloadEntry {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(entry) = self.service.snapshot().entries.iter().find(|e| e.id == id) {
                if matches!(
                    entry.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                ) {
                    return entry.clone();
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting on entry {id}: {:?}",
                self.service.snapshot().entries
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn config(&self) -> Config {
        Config::load(&self.config_path).unwrap()
    }

    fn install_dir(&self, name: &str) -> PathBuf {
        self.library.join("Emulators").join(name)
    }

    /// Fails when any request the mock forge received carried an
    /// `Authorization` header — a RomM credential must never reach a forge.
    async fn assert_no_authorization_headers(&self) {
        let received = self.server.received_requests().await.unwrap();
        assert!(!received.is_empty(), "no forge requests were made");
        for request in &received {
            assert!(
                request.headers.get("authorization").is_none(),
                "an Authorization header reached the forge for {}",
                request.url
            );
        }
    }
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn zip_bytes(staging: &tempfile::TempDir, name: &str, entries: &[(&str, &[u8])]) -> Vec<u8> {
    write_zip(&staging.path().join(name), entries)
}

// --- (a) end-to-end -----------------------------------------------------------

#[tokio::test]
async fn zip_install_extracts_writes_config_entry_and_deletes_the_archive() {
    let staging = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(
        &staging,
        "widget.zip",
        &[
            ("bin/testemu.sh", b"#!/bin/sh\n"),
            ("data/readme.txt", b"hello"),
        ],
    );

    let harness = Harness::new(|uri| vec![profile("Test Emu", gitea_source(uri))]).await;
    // An explicitly pinned tag must match what the release reports, so the
    // configured and resolved tags can only differ under a `latest` pin —
    // see `a_latest_pinned_source_reuses_one_install_directory_across_releases`.
    harness.mount_widget("widget-linux.zip", bytes, 0).await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;

    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.error, "");
    assert_eq!(entry.job, "emulator");
    assert_eq!(entry.source_id, "acme/widget");
    assert_eq!(entry.title, "Test Emu");
    assert_eq!(entry.platform, "Emulator");

    let install_dir = harness.install_dir("Test Emu-v1.0");
    let exe = install_dir.join("bin/testemu.sh");
    assert!(exe.is_file(), "extracted tree missing: {}", exe.display());
    assert!(install_dir.join("data/readme.txt").is_file());
    assert!(
        !install_dir.join("Test Emu-v1.0.zip").exists(),
        "the extracted archive should be deleted"
    );
    assert!(!install_dir.join(".extract-tmp").exists());
    #[cfg(unix)]
    assert_eq!(mode_of(&exe), 0o755);

    let config = harness.config();
    assert_eq!(config.emulators.len(), 1);
    let saved = &config.emulators[0];
    assert_eq!(saved.name, "Test Emu");
    assert_eq!(saved.path, exe.to_string_lossy());
    assert_eq!(saved.args, "-f %rom%");
    assert_eq!(saved.source_id, "acme/widget");
    assert_eq!(saved.source_provider, "gitea");
    assert_eq!(saved.source_owner, "acme");
    assert_eq!(saved.source_repo, "widget");
    assert_eq!(saved.source_release_tag, "v1.0");

    // (i) no RomM credential ever reaches the forge.
    harness.assert_no_authorization_headers().await;

    // The registry stays games-only: an emulator install writes no row.
    assert!(harness.service.installed().unwrap().is_empty());
}

// --- (b) dedupe ---------------------------------------------------------------

#[tokio::test]
async fn a_second_install_of_the_same_source_id_while_active_is_ignored() {
    let staging = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(
        &staging,
        "widget.zip",
        &[("bin/testemu.sh", b"#!/bin/sh\n")],
    );

    let harness = Harness::new(|uri| vec![profile("Test Emu", gitea_source(uri))]).await;
    harness.mount_widget("widget-linux.zip", bytes, 400).await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();

    assert_eq!(
        harness.service.snapshot().entries.len(),
        1,
        "the duplicate admission should be ignored silently"
    );
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
}

// --- (c) resolution failure ----------------------------------------------------

#[tokio::test]
async fn a_resolution_failure_fails_the_row_with_the_source_error_and_writes_nothing() {
    let harness = Harness::new(|_| {
        vec![profile(
            "Test Emu",
            json!({"provider": "carrier-pigeon", "owner": "acme", "repo": "widget"}),
        )]
    })
    .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;

    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(
        entry.error,
        "Unsupported source provider 'carrier-pigeon'. Supported providers: github, gitea, direct."
    );
    assert!(
        !harness.library.join("Emulators").exists(),
        "nothing should be written when resolution fails"
    );
    assert!(harness.config().emulators.is_empty());
}

#[tokio::test]
async fn an_unknown_source_id_errors_before_admission() {
    let harness = Harness::new(|_| Vec::new()).await;
    let err = harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "registry: unknown emulator: acme/widget");
    assert!(harness.service.snapshot().entries.is_empty());
}

// --- (d) supplemental merge -----------------------------------------------------

#[tokio::test]
async fn a_supplemental_archive_merges_over_the_primary_and_is_deleted() {
    let staging = tempfile::tempdir().unwrap();
    let primary = zip_bytes(
        &staging,
        "primary.zip",
        &[
            ("bin/testemu.sh", b"PRIMARY"),
            ("data/keep.txt", b"KEEP-ME"),
        ],
    );
    let supplemental = zip_bytes(
        &staging,
        "supp.zip",
        &[
            ("bin/testemu.sh", b"SUPPLEMENTAL"),
            ("sys/firmware.bin", b"FIRMWARE"),
        ],
    );

    let harness =
        Harness::new(|uri| vec![profile("Test Emu", gitea_source_with_supplemental(uri))]).await;
    harness.mount_widget("widget-linux.zip", primary, 0).await;
    let extras_url = format!("{}/dl/extras.zip", harness.uri());
    harness
        .mount_json(
            EXTRAS_RELEASE,
            release_json("v9", "extras.zip", &extras_url, supplemental.len()),
        )
        .await;
    harness.mount_bytes("/dl/extras.zip", supplemental, 0).await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let install_dir = harness.install_dir("Test Emu-v1.0");
    assert_eq!(
        fs::read(install_dir.join("bin/testemu.sh")).unwrap(),
        b"SUPPLEMENTAL",
        "the supplemental copy must overwrite the primary's file"
    );
    assert_eq!(
        fs::read(install_dir.join("data/keep.txt")).unwrap(),
        b"KEEP-ME",
        "files the supplemental does not carry must survive"
    );
    assert_eq!(
        fs::read(install_dir.join("sys/firmware.bin")).unwrap(),
        b"FIRMWARE"
    );
    assert!(!install_dir.join("Test Emu-v1.0.zip").exists());
    assert!(!install_dir
        .join("Test Emu-v1.0-supplemental-1.zip")
        .exists());
    assert!(!install_dir.join(".supp-tmp-1").exists());
}

// --- (e) supplemental download failure -------------------------------------------

#[tokio::test]
async fn a_failed_supplemental_download_fails_the_row_and_keeps_the_primary_archive() {
    let staging = tempfile::tempdir().unwrap();
    let primary = zip_bytes(&staging, "primary.zip", &[("bin/testemu.sh", b"PRIMARY")]);

    let harness =
        Harness::new(|uri| vec![profile("Test Emu", gitea_source_with_supplemental(uri))]).await;
    harness.mount_widget("widget-linux.zip", primary, 0).await;
    // The supplemental resolves, but its asset URL is never mounted → 404.
    let missing_url = format!("{}/dl/missing.zip", harness.uri());
    harness
        .mount_json(
            EXTRAS_RELEASE,
            release_json("v9", "extras.zip", &missing_url, 10),
        )
        .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;

    assert_eq!(entry.status, DownloadStatus::Failed);
    assert!(!entry.error.is_empty());
    let install_dir = harness.install_dir("Test Emu-v1.0");
    assert!(
        install_dir.join("Test Emu-v1.0.zip").is_file(),
        "the finished primary archive must survive so a retry can skip it"
    );
    assert!(harness.config().emulators.is_empty());
}

// --- (f) AppImage primary ---------------------------------------------------------

#[tokio::test]
async fn an_appimage_primary_is_kept_in_place_made_executable_and_recorded() {
    let harness = Harness::new(|uri| vec![profile("Test Emu", gitea_source(uri))]).await;
    harness
        .mount_widget("TestEmu-x86_64.AppImage", b"APPIMAGE-BYTES".to_vec(), 0)
        .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    // The asset name renames the FILE, never the directory: the install dir
    // is still <profile name>-<configured tag> (install_mixin.py:1444).
    let appimage = harness
        .install_dir("Test Emu-v1.0")
        .join("TestEmu-x86_64.AppImage");
    assert!(appimage.is_file(), "the AppImage is the install; keep it");
    assert!(
        !harness.install_dir("TestEmu-x86_64").exists(),
        "the install directory must not be named from the asset"
    );
    #[cfg(unix)]
    assert_eq!(mode_of(&appimage), 0o755);
    assert_eq!(
        harness.config().emulators[0].path,
        appimage.to_string_lossy()
    );
}

// --- configured tag names the install directory ---------------------------------------

#[tokio::test]
async fn a_latest_pinned_source_reuses_one_install_directory_across_releases() {
    let staging = tempfile::tempdir().unwrap();
    let first = zip_bytes(&staging, "first.zip", &[("bin/testemu.sh", b"FIRST")]);
    let second = zip_bytes(&staging, "second.zip", &[("bin/testemu.sh", b"SECOND")]);

    let harness = Harness::new(|uri| {
        let mut source = gitea_source(uri);
        source["release_tag"] = json!("latest");
        vec![profile("Test Emu", source)]
    })
    .await;
    // Two consecutive releases under the same `latest` pin.
    harness
        .mount_widget_at(
            "/api/v1/repos/acme/widget/releases/latest",
            "v3.2.1",
            "widget-v3.2.1.zip",
            first,
            0,
        )
        .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let install_dir = harness.install_dir("Test Emu-latest");
    assert!(
        install_dir.join("bin/testemu.sh").is_file(),
        "a 'latest' pin installs into one stable directory, not one per resolved tag"
    );
    assert!(!harness.install_dir("Test Emu-v3.2.1").exists());
    assert_eq!(
        harness.config().emulators[0].source_release_tag,
        "latest",
        "the configured pin is recorded so the entry keeps tracking the newest release"
    );

    // The next release reinstalls over the same directory instead of leaving
    // the old one behind.
    harness.server.reset().await;
    harness
        .mount_widget_at(
            "/api/v1/repos/acme/widget/releases/latest",
            "v4.0.0",
            "widget-v4.0.0.zip",
            second,
            0,
        )
        .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    assert_eq!(
        fs::read(install_dir.join("bin/testemu.sh")).unwrap(),
        b"SECOND"
    );
    let dirs: Vec<String> = fs::read_dir(harness.library.join("Emulators"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(dirs, vec!["Test Emu-latest".to_string()]);
    assert_eq!(harness.config().emulators.len(), 1);
}

// --- hostile asset names ----------------------------------------------------------------

#[tokio::test]
async fn an_asset_name_that_is_not_a_plain_file_name_fails_before_anything_is_written() {
    let harness = Harness::new(|uri| vec![profile("Test Emu", gitea_source(uri))]).await;
    // An AppImage asset name is copied through verbatim by the naming
    // helper, so a separator in it would escape the install directory.
    harness
        .mount_widget("../../evil.AppImage", b"PWNED".to_vec(), 0)
        .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;

    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(
        entry.error,
        "Refusing to install release asset '../../evil.AppImage': it does not name a plain file."
    );
    assert!(
        !harness.library.join("evil.AppImage").exists(),
        "the escaped path must never be written"
    );
    assert!(
        !harness.library.join("Emulators").exists(),
        "the name is rejected before any request goes out"
    );
    assert!(harness.config().emulators.is_empty());
}

// --- (g) replace in place -----------------------------------------------------------

#[tokio::test]
async fn an_existing_config_entry_with_the_same_name_is_replaced_at_its_index() {
    let staging = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(
        &staging,
        "widget.zip",
        &[("bin/testemu.sh", b"#!/bin/sh\n")],
    );

    let extra = concat!(
        "\n[[emulators]]\nname = \"Test Emu\"\npath = \"/old/emu.sh\"\nargs = \"old-args\"\n",
        "\n[[emulators]]\nname = \"Other\"\npath = \"/other/emu.sh\"\nargs = \"%rom%\"\n",
    );
    let harness =
        Harness::with_config(|uri| vec![profile("Test Emu", gitea_source(uri))], extra).await;
    harness.mount_widget("widget-linux.zip", bytes, 0).await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let config = harness.config();
    assert_eq!(config.emulators.len(), 2);
    assert_eq!(config.emulators[0].name, "Test Emu");
    assert_eq!(
        config.emulators[0].path,
        harness
            .install_dir("Test Emu-v1.0")
            .join("bin/testemu.sh")
            .to_string_lossy()
    );
    assert_eq!(config.emulators[0].args, "-f %rom%");
    assert_eq!(config.emulators[1].name, "Other");
    assert_eq!(config.emulators[1].path, "/other/emu.sh");
}

// --- (h) retry ----------------------------------------------------------------------

#[tokio::test]
async fn retrying_a_failed_emulator_row_reinstalls_without_a_romm_client() {
    let staging = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(
        &staging,
        "widget.zip",
        &[("bin/testemu.sh", b"#!/bin/sh\n")],
    );

    let harness = Harness::new(|uri| vec![profile("Test Emu", gitea_source(uri))]).await;

    // No release mock yet → the forge returns 404 and resolution fails.
    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let failed_id = harness.newest_entry_id();
    let entry = harness.wait_terminal(failed_id).await;
    assert_eq!(entry.status, DownloadStatus::Failed);

    harness.server.reset().await;
    harness.mount_widget("widget-linux.zip", bytes, 0).await;

    harness.service.retry(None, failed_id).await.unwrap();
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
    assert!(harness
        .install_dir("Test Emu-v1.0")
        .join("bin/testemu.sh")
        .is_file());
}

#[tokio::test]
async fn retrying_a_failed_game_row_without_a_client_reports_not_connected() {
    let harness = Harness::new(|_| Vec::new()).await;

    // A game whose detail resolves but whose content 404s leaves a Failed row.
    harness
        .mount_json(
            "/api/roms/1",
            json!({
                "id": 1,
                "name": "Chrono Trigger",
                "fs_name_no_ext": "Chrono Trigger",
                "platform_id": 7,
                "platform_display_name": "SNES",
                "fs_name": "chrono.zip",
                "fs_size_bytes": 7,
                "files": [{
                    "id": 11,
                    "file_name": "chrono.zip",
                    "file_size_bytes": 7,
                    "is_top_level": true,
                }],
            }),
        )
        .await;
    let client = Arc::new(
        RommClient::new(
            &harness.uri(),
            Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
        )
        .unwrap(),
    );

    harness.service.install(client, 1).await.unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Failed);

    let err = harness.service.retry(None, id).await.unwrap_err();
    assert_eq!(err.to_string(), "registry: not connected");
    // The row is left alone so the user can retry once connected.
    assert!(harness
        .service
        .snapshot()
        .entries
        .iter()
        .any(|e| e.id == id));
}

#[tokio::test]
async fn retrying_an_unknown_entry_is_a_no_op() {
    let harness = Harness::new(|_| Vec::new()).await;
    assert!(harness.service.retry(None, 999).await.is_ok());
}
