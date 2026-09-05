//! Tests for the ops layer. Wiremock-backed `#[tokio::test]`s cover the
//! flows that talk to RomM; the rest are pure.

use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{json, Value};
use tempfile::TempDir;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::cloud::state::SyncStateUpdate;
use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::EmulatorProfile;
use crate::romm::RommClient;
use crate::secrets::Credential;
use secrecy::SecretString;

use super::native::restore_native_cloud_save_for_game;
use super::restore::{restore_cloud_save_for_game, restore_cloud_state_for_game};
use super::upload::upload_cloud_files_for_game;
use super::*;

// --- fixtures ----------------------------------------------------------

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

fn game(title: &str, platform: &str, rom_id: &str) -> CloudGame {
    CloudGame {
        title: title.to_string(),
        platform: platform.to_string(),
        rom_id: rom_id.to_string(),
        ..Default::default()
    }
}

fn entry_named(name: &str, save_paths: &str) -> EmulatorEntry {
    EmulatorEntry {
        name: name.to_string(),
        path: String::new(),
        args: "%rom%".to_string(),
        save_paths: save_paths.to_string(),
        ..Default::default()
    }
}

/// A config with one emulator entry registered as the default for
/// `platform`.
fn config_with(entry: EmulatorEntry, platform: &str) -> Config {
    let mut config = Config::default();
    config
        .default_emulators
        .insert(platform.to_string(), entry.name.clone());
    config.emulators = vec![entry];
    config
}

struct Fixture {
    config: Config,
    profiles: Vec<EmulatorProfile>,
    games: Vec<CloudGame>,
    sessions: Vec<ActiveSessionRef>,
    pcgw: Vec<String>,
    _config_dir: TempDir,
}

impl Fixture {
    fn new(config: Config) -> Self {
        Self {
            config,
            profiles: Vec::new(),
            games: Vec::new(),
            sessions: Vec::new(),
            pcgw: Vec::new(),
            _config_dir: tempfile::tempdir().unwrap(),
        }
    }

    fn ctx(&self) -> CloudContext<'_> {
        CloudContext {
            config: &self.config,
            profiles: &self.profiles,
            all_games: &self.games,
            resolve_ctx: ResolveContext {
                emulator_dir: None,
                library_dir: "",
                config_dir: self._config_dir.path(),
                windows_documents: None,
            },
            active_sessions: &self.sessions,
            now: 1_700_000_000.0,
            pcgw_paths: &self.pcgw,
            wine_prefix: None,
        }
    }
}

fn write_file(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn set_mtime(path: &Path, secs: u64) {
    let time = UNIX_EPOCH + Duration::from_secs(secs);
    fs::File::open(path)
        .unwrap()
        .set_modified(time)
        .expect("set mtime");
}

fn texts(messages: &[CloudMessage]) -> Vec<String> {
    messages.iter().map(|m| m.text.clone()).collect()
}

async fn mock_ok(server: &MockServer, verb: &str, route: &str, body: Value) {
    Mock::given(method(verb))
        .and(wm_path(route.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

// --- upload preconditions ---------------------------------------------

/// Doc 06 "Upload planning": the six preconditions, in order, with their
/// exact messages.
#[tokio::test]
async fn upload_precondition_messages_fire_in_order() {
    let server = MockServer::start().await;
    let client = client_for(&server);

    // 2. Block reason (informational, stops before the ROM-id check even
    //    though this game HAS no ROM id).
    let root = TempDir::new().unwrap();
    let exe_dir = root.path().join("bin");
    fs::create_dir_all(&exe_dir).unwrap();
    let exe = exe_dir.join("xemu");
    fs::write(&exe, b"").unwrap();
    let blocked = EmulatorEntry {
        name: "xemu".to_string(),
        path: exe.to_string_lossy().into_owned(),
        save_paths: root.path().to_string_lossy().into_owned(),
        ..Default::default()
    };
    let fx = Fixture::new(config_with(blocked, "Xbox"));
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Halo", "Xbox", ""),
        SaveType::Save,
    )
    .await;
    assert_eq!(
        texts(&report.messages),
        vec!["No xemu HDD image is configured, so cloud sync is unavailable."]
    );
    assert_eq!(report.messages[0].severity, MessageSeverity::Info);

    // 3. No ROM id.
    let saves = TempDir::new().unwrap();
    let entry = entry_named("Dummy", saves.path().to_str().unwrap());
    let fx = Fixture::new(config_with(entry.clone(), "GameCube"));
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Zelda", "GameCube", ""),
        SaveType::Save,
    )
    .await;
    assert_eq!(texts(&report.messages), vec![MISSING_ROM_ID]);
    assert_eq!((report.uploaded, report.total), (0, 0));

    // 4. No emulator entry configured for the platform.
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Zelda", "GameCube", "7"),
        SaveType::Save,
    )
    .await;
    assert_eq!(texts(&report.messages), vec![NO_DEFAULT_EMULATOR]);

    // 5. No resolved sync directories.
    let fx = Fixture::new(config_with(entry_named("Dummy", ""), "GameCube"));
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Zelda", "GameCube", "7"),
        SaveType::Save,
    )
    .await;
    assert_eq!(
        texts(&report.messages),
        vec!["No save directories were found for emulator 'Dummy'. Configure them in Emulators."]
    );

    // 5. (state wording)
    let states = TempDir::new().unwrap();
    let mut rpcs3 = entry_named("RPCS3", saves.path().to_str().unwrap());
    rpcs3.state_paths = String::new();
    let fx = Fixture::new(config_with(rpcs3.clone(), "PS3"));
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Demon", "PS3", "7"),
        SaveType::State,
    )
    .await;
    assert_eq!(
        texts(&report.messages),
        vec!["No state directories were found for emulator 'RPCS3'. Configure them in Emulators."]
    );

    // 6. RPCS3 + state.
    let mut rpcs3 = entry_named("RPCS3", saves.path().to_str().unwrap());
    rpcs3.state_paths = states.path().to_str().unwrap().to_string();
    let fx = Fixture::new(config_with(rpcs3, "PS3"));
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Demon", "PS3", "7"),
        SaveType::State,
    )
    .await;
    assert_eq!(
        texts(&report.messages),
        vec!["RPCS3 savestate uploads are not supported yet."]
    );

    // 2. Block reason: a native Windows platform delegates instead
    //    (precondition 1) — with no configured save locations that is the
    //    native flow's own first message.
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Portal", "Windows", "7"),
        SaveType::Save,
    )
    .await;
    assert_eq!(
        texts(&report.messages),
        vec!["No save locations are configured for this game. Use 'Manage Saves' → 'Browse' to add one."]
    );
}

// --- job construction --------------------------------------------------

/// Doc 06 "Upload planning": `shared-single` bundles everything into ONE
/// job named `"<emulator name or 'Shared Save'> Storage"`. D1 makes xemu
/// the only shared-single emulator, and its single job is built from the
/// raw HDD image rather than from file candidates — the naming rule and
/// the "exactly one job" shape are the same either way.
#[tokio::test]
async fn shared_single_bundles_everything_into_one_named_job() {
    assert_eq!(
        super::upload::shared_single_display_name("xemu"),
        "xemu Storage"
    );
    assert_eq!(
        super::upload::shared_single_display_name(""),
        "Shared Save Storage"
    );

    let root = TempDir::new().unwrap();
    let entry = xemu_fixture(root.path(), true);
    // Two extra loose files under the save path prove nothing else is
    // picked up: xemu contributes no generic candidates (D1).
    write_file(&root.path().join("stray-a.bin"), b"a");
    write_file(&root.path().join("stray-b.bin"), b"b");
    let fx = Fixture::new(config_with(entry, "Xbox"));

    let server = MockServer::start().await;
    mock_ok(&server, "POST", "/api/saves", json!({})).await;
    mock_ok(&server, "GET", "/api/saves", json!([])).await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();

    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Halo", "Xbox", "7"),
        SaveType::Save,
    )
    .await;
    assert_eq!(
        (report.uploaded, report.total),
        (1, 1),
        "{:?}",
        report.messages
    );
}

/// Doc 06 "Upload planning" slot table.
#[test]
fn slot_assignment_table() {
    use super::upload::slot_for_job as slot;
    use crate::cloud::transfer::UploadJob;

    let job = |display: &str, files: &[&str]| UploadJob {
        display_name: display.to_string(),
        payload: files
            .iter()
            .map(|f| ("saveFile".to_string(), PathBuf::from(f)))
            .collect(),
    };

    // States never carry a slot.
    assert_eq!(
        slot(SaveScope::SharedSlotted, SaveType::State, &job("vmu2", &[])),
        None
    );
    // shared-single is always the literal shared-media.
    assert_eq!(
        slot(
            SaveScope::SharedSingle,
            SaveType::Save,
            &job("anything", &[])
        ),
        Some("shared-media".to_string())
    );
    // shared-slotted matches in the display name first ...
    assert_eq!(
        slot(
            SaveScope::SharedSlotted,
            SaveType::Save,
            &job("VMU3 backup", &["/tmp/other.bin"])
        ),
        Some("vmu3".to_string())
    );
    // ... then in a payload path's stem/name.
    assert_eq!(
        slot(
            SaveScope::SharedSlotted,
            SaveType::Save,
            &job("backup", &["/tmp/dc_vmu1.bin"])
        ),
        Some("vmu1".to_string())
    );
    // No match at all.
    assert_eq!(
        slot(
            SaveScope::SharedSlotted,
            SaveType::Save,
            &job("backup", &["/tmp/plain.bin"])
        ),
        None
    );
    // per-game never carries a slot.
    assert_eq!(
        slot(SaveScope::PerGame, SaveType::Save, &job("vmu0", &[])),
        None
    );
}

/// Doc 06 "Upload execution": a failing POST isolates to its own job, and
/// retention pruning still runs afterwards with the clamped limit.
#[tokio::test]
async fn upload_isolates_per_job_failures_and_prunes_after() {
    let server = MockServer::start().await;
    // First POST fails, every later one succeeds.
    Mock::given(method("POST"))
        .and(wm_path("/api/saves"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(wm_path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    // Retention refetch: three records in one slot group, limit 1 -> two
    // stale deletes.
    mock_ok(
        &server,
        "GET",
        "/api/saves",
        json!([
            {"id": 1, "emulator": "Dolphin", "file_name": "s.srm", "updated_at": "2026-01-03T00:00:00Z"},
            {"id": 2, "emulator": "Dolphin", "file_name": "s.srm", "updated_at": "2026-01-02T00:00:00Z"},
            {"id": 3, "emulator": "Dolphin", "file_name": "s.srm", "updated_at": "2026-01-01T00:00:00Z"}
        ]),
    )
    .await;
    mock_ok(&server, "POST", "/api/saves/delete", json!({})).await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    write_file(&saves.path().join("zelda.srm"), b"a");
    write_file(&saves.path().join("zelda-alt.sav"), b"b");

    let entry = entry_named("Dolphin", saves.path().to_str().unwrap());
    let mut config = config_with(entry, "GameCube");
    config.cloud_save_retention_limit = 0; // clamped to 1 (D7)
    let fx = Fixture::new(config);
    let mut caches = CloudCaches::default();

    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Zelda", "GameCube", "7"),
        SaveType::Save,
    )
    .await;

    assert_eq!(report.total, 2, "one job per grouped stem");
    assert_eq!(report.uploaded, 1);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.messages.len(), 1);
    assert_eq!(report.messages[0].severity, MessageSeverity::Warning);
    assert!(report.messages[0]
        .text
        .starts_with("Uploaded 1 save files. Failed: "));

    let deletes = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.url.path() == "/api/saves/delete")
        .count();
    assert_eq!(deletes, 2, "limit clamped to 1 keeps only the newest");
}

// --- restore -----------------------------------------------------------

/// Doc 06 "Restore — saves" steps 4 and 6: a supplied record is used
/// as-is, and a record naming an unconfigured emulator that differs from
/// the resolved one is refused.
#[tokio::test]
async fn save_restore_prefers_supplied_record_and_enforces_the_emulator_rule() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/11/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    let entry = entry_named("Dolphin", saves.path().to_str().unwrap());
    let fx = Fixture::new(config_with(entry, "GameCube"));
    let mut caches = CloudCaches::default();
    let target_game = game("Zelda", "GameCube", "7");

    // Unknown emulator, different from the resolved one -> refusal.
    let bad = json!({"id": 11, "emulator": "Nestopia", "file_name": "zelda.srm"});
    let (ok, messages, _) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &target_game,
        Some(&bad),
        false,
        false,
    )
    .await;
    assert!(!ok);
    assert_eq!(
        texts(&messages),
        vec!["Emulator 'Nestopia' is not configured on this device."]
    );

    // Same record with the resolved emulator name -> restored, no list
    // request needed (the supplied record wins).
    let good = json!({"id": 11, "emulator": "Dolphin", "file_name": "zelda.srm"});
    let (ok, messages, update) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &target_game,
        Some(&good),
        false,
        false,
    )
    .await;
    assert!(ok, "{messages:?}");
    assert_eq!(texts(&messages), vec![RESTORE_SUCCESS]);
    assert_eq!(update.last_downloaded_save_id.as_deref(), Some("11"));
    assert_eq!(
        fs::read(saves.path().join("zelda.srm")).unwrap(),
        b"payload"
    );
}

/// Doc 06 "Conflict and newer detection": the known-latest short circuit
/// applies to saves ONLY in the `per-game` scope.
#[tokio::test]
async fn known_latest_skip_only_for_per_game_scope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/11/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fresh".to_vec()))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    write_file(&saves.path().join("zelda.srm"), b"local");

    let entry = entry_named("Dolphin", saves.path().to_str().unwrap());
    let mut config = config_with(entry, "GameCube");
    let target_game = game("Zelda", "GameCube", "7");
    let mut sync = toml::value::Table::new();
    let mut row = toml::value::Table::new();
    row.insert(
        "last_downloaded_save_id".into(),
        toml::Value::String("11".into()),
    );
    sync.insert(game_key(&target_game), toml::Value::Table(row));
    config.cloud_sync_state = sync;

    let fx = Fixture::new(config);
    let mut caches = CloudCaches::default();
    let record = json!({"id": 11, "emulator": "Dolphin", "file_name": "zelda.srm"});

    // per-game scope: skipped silently, local file untouched.
    let (ok, messages, update) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &target_game,
        Some(&record),
        false,
        true,
    )
    .await;
    assert!(!ok);
    assert!(messages.is_empty());
    assert_eq!(update, SyncStateUpdate::default());
    assert_eq!(fs::read(saves.path().join("zelda.srm")).unwrap(), b"local");

    // A shared scope never takes the known-latest short circuit, even
    // with the same stored id and a live local file.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/11/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fresh".to_vec()))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let dc_saves = TempDir::new().unwrap();
    write_file(&dc_saves.path().join("shenmue.bin"), b"local");
    let entry = entry_named("Redream", dc_saves.path().to_str().unwrap());
    let mut config = config_with(entry, "Dreamcast");
    let shared_game = game("Shenmue", "Dreamcast", "7");
    let mut sync = toml::value::Table::new();
    let mut row = toml::value::Table::new();
    row.insert(
        "last_downloaded_save_id".into(),
        toml::Value::String("11".into()),
    );
    sync.insert(game_key(&shared_game), toml::Value::Table(row));
    config.cloud_sync_state = sync;
    let fx = Fixture::new(config);
    let mut caches = CloudCaches::default();
    assert_eq!(
        scope_for_game(
            &fx.ctx(),
            &shared_game,
            SaveType::Save,
            fx.config.emulators.first()
        ),
        SaveScope::SharedSlotted
    );
    let record = json!({"id": 11, "emulator": "Redream", "file_name": "shenmue.bin"});
    let (ok, messages, _) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &shared_game,
        Some(&record),
        false,
        true,
    )
    .await;
    assert!(
        ok,
        "shared scopes ignore the known-latest skip: {messages:?}"
    );
    assert_eq!(
        fs::read(dc_saves.path().join("shenmue.bin")).unwrap(),
        b"fresh"
    );
}

/// Doc 06 "Conflict and newer detection": the local-newer skip is
/// deliberately bypassed for PCSX2 when no PS2 serials could be derived.
#[tokio::test]
async fn local_newer_skip_exempts_pcsx2_without_serials() {
    // Non-PCSX2: a fresh local file and an old server record -> skipped.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/11/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fresh".to_vec()))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    let local = saves.path().join("metroid.srm");
    write_file(&local, b"local");
    set_mtime(&local, 2_000_000_000);
    let entry = entry_named("Nestopia", saves.path().to_str().unwrap());
    let fx = Fixture::new(config_with(entry, "NES"));
    let mut caches = CloudCaches::default();
    let record = json!({"id": 11, "emulator": "Nestopia", "file_name": "metroid.srm", "updated_at": "2001-01-01T00:00:00Z"});
    let (ok, messages, _) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Metroid", "NES", "7"),
        Some(&record),
        true,
        false,
    )
    .await;
    assert!(!ok);
    assert!(messages.is_empty());
    assert_eq!(fs::read(&local).unwrap(), b"local");

    // PCSX2 with no derivable serials: the check is skipped entirely and
    // the (folder-save) restore proceeds.
    let payload = zip_bytes(&[("BASLUS/data.bin", b"restored")]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/11/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    let local = saves.path().join("ico.srm");
    write_file(&local, b"local");
    set_mtime(&local, 2_000_000_000);
    let entry = entry_named("PCSX2", saves.path().to_str().unwrap());
    let fx = Fixture::new(config_with(entry, "PS2"));
    let mut caches = CloudCaches::default();
    let record = json!({"id": 11, "emulator": "PCSX2", "file_name": "ico.srm", "updated_at": "2001-01-01T00:00:00Z"});
    let target = game("Ico", "PS2", "7");
    assert!(
        crate::cloud::tokens::ps2_serial_tokens(&target).is_empty(),
        "fixture must have no derivable PS2 serials"
    );
    let (ok, messages, _) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &target,
        Some(&record),
        true,
        false,
    )
    .await;
    assert!(
        ok,
        "PCSX2 without serials must not take the local-newer skip: {messages:?}"
    );
    assert!(saves.path().join("BASLUS/data.bin").is_file());
}

/// D6: a multi-record restore stages everything and commits only when
/// every record downloaded cleanly.
#[tokio::test]
async fn shared_slotted_restore_is_atomic() {
    let saves = TempDir::new().unwrap();
    let a = saves.path().join("vmu0.bin");
    let b = saves.path().join("vmu1.bin");
    write_file(&a, b"old-a");
    write_file(&b, b"old-b");

    let entry = entry_named("Redream", saves.path().to_str().unwrap());
    let fx = Fixture::new(config_with(entry, "Dreamcast"));
    let target = game("Shenmue", "Dreamcast", "7");

    let records = json!([
        {"id": 21, "emulator": "Redream", "slot": "vmu0", "file_name": "vmu0.bin", "updated_at": "2026-01-02T00:00:00Z"},
        {"id": 22, "emulator": "Redream", "slot": "vmu1", "file_name": "vmu1.bin", "updated_at": "2026-01-01T00:00:00Z"}
    ]);

    // Round 1: the second record's download fails -> nothing changes.
    let server = MockServer::start().await;
    mock_ok(&server, "GET", "/api/saves", records.clone()).await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/21/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"new-a".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/22/content"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();
    let (ok, messages, _) =
        restore_cloud_save_for_game(&client, &fx.ctx(), &mut caches, &target, None, false, false)
            .await;
    assert!(!ok);
    assert!(messages[0]
        .text
        .starts_with("Failed to restore cloud save: "));
    assert_eq!(fs::read(&a).unwrap(), b"old-a", "D6: no partial commit");
    assert_eq!(fs::read(&b).unwrap(), b"old-b");

    // Round 2: both succeed -> both placed.
    let server = MockServer::start().await;
    mock_ok(&server, "GET", "/api/saves", records).await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/21/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"new-a".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/22/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"new-b".to_vec()))
        .mount(&server)
        .await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();
    let (ok, messages, update) =
        restore_cloud_save_for_game(&client, &fx.ctx(), &mut caches, &target, None, false, false)
            .await;
    assert!(ok, "{messages:?}");
    assert_eq!(fs::read(&a).unwrap(), b"new-a");
    assert_eq!(fs::read(&b).unwrap(), b"new-b");
    assert_eq!(update.last_downloaded_save_id.as_deref(), Some("21"));
}

/// Doc 06 "Restore — saves" step 8: folder-save emulators extract into
/// `directories[0]`.
#[tokio::test]
async fn folder_save_emulators_extract_into_the_first_directory() {
    let payload = zip_bytes(&[("SLUS-123/data.bin", b"inner")]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/31/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    let entry = entry_named(
        "PCSX2",
        &format!(
            "{};{}",
            first.path().to_str().unwrap(),
            second.path().to_str().unwrap()
        ),
    );
    let fx = Fixture::new(config_with(entry, "PS2"));
    let mut caches = CloudCaches::default();
    let record = json!({"id": 31, "emulator": "PCSX2", "file_name": "memcard.zip"});
    let (ok, messages, _) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Ico", "PS2", "7"),
        Some(&record),
        false,
        false,
    )
    .await;
    assert!(ok, "{messages:?}");
    assert!(first.path().join("SLUS-123/data.bin").is_file());
    assert!(!second.path().join("SLUS-123/data.bin").exists());
}

/// D4: an absolute-URL state candidate is skipped; the next relative one
/// is fetched. When every candidate fails, the `ValueError` text is
/// returned verbatim.
#[tokio::test]
async fn state_restore_walks_candidates_and_skips_absolute_urls() {
    let server = MockServer::start().await;
    mock_ok(
        &server,
        "GET",
        "/api/states/41",
        json!({
            "id": 41,
            "download_path": "https://elsewhere.example/absolute.state",
            "file_path": "assets/saves/41.state"
        }),
    )
    .await;
    Mock::given(method("GET"))
        .and(wm_path("/assets/saves/41.state"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"state-bytes".to_vec()))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let states = TempDir::new().unwrap();
    let mut entry = entry_named("Nestopia", "");
    entry.state_paths = states.path().to_str().unwrap().to_string();
    let fx = Fixture::new(config_with(entry, "NES"));
    let mut caches = CloudCaches::default();
    let record = json!({"id": 41, "emulator": "Nestopia", "file_name": "metroid.state"});
    let (ok, messages, update) = restore_cloud_state_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Metroid", "NES", "7"),
        Some(&record),
        false,
    )
    .await;
    assert!(ok, "{messages:?}");
    assert_eq!(update.last_downloaded_state_id.as_deref(), Some("41"));
    assert_eq!(
        fs::read(states.path().join("metroid.state")).unwrap(),
        b"state-bytes"
    );

    // Every candidate absolute -> the pinned ValueError text.
    let server = MockServer::start().await;
    mock_ok(
        &server,
        "GET",
        "/api/states/41",
        json!({"id": 41, "download_path": "https://elsewhere.example/a.state"}),
    )
    .await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();
    let (ok, messages, _) = restore_cloud_state_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Metroid", "NES", "7"),
        Some(&record),
        false,
    )
    .await;
    assert!(!ok);
    assert_eq!(
        texts(&messages),
        vec![
            "Failed to download cloud state content: State content path could not be resolved from server record."
        ]
    );
}

// --- xemu --------------------------------------------------------------

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::{Cursor, Write as _};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, body) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Builds a retail-layout raw image holding one `UDATA` file and returns
/// an entry whose `xemu.toml` points at it.
fn xemu_fixture(root: &Path, populated: bool) -> EmulatorEntry {
    use crate::fatx::builder::FatxImageBuilder;
    use crate::fatx::layout::{RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE};

    let image = root.join("xbox_hdd.img");
    let mut builder = FatxImageBuilder::new(RETAIL_PARTITION_E_SIZE)
        .with_base_offset(RETAIL_PARTITION_E_OFFSET)
        .with_cluster_size(16 * 1024);
    if populated {
        builder.add_file("UDATA/4541000d/00000001/savedata.bin", vec![0xA5; 64]);
    }
    builder.write_to(&image).unwrap();

    let exe_dir = root.join("bin");
    fs::create_dir_all(&exe_dir).unwrap();
    let exe = exe_dir.join("xemu");
    fs::write(&exe, b"").unwrap();
    fs::write(
        exe_dir.join("xemu.toml"),
        format!("[sys.files]\nhdd_path = '{}'\n", image.display()),
    )
    .unwrap();

    EmulatorEntry {
        name: "xemu".to_string(),
        path: exe.to_string_lossy().into_owned(),
        args: "%rom%".to_string(),
        save_paths: root.to_string_lossy().into_owned(),
        ..Default::default()
    }
}

/// Spec "xemu flow": upload builds the UDATA/TDATA archive from the raw
/// image; restore injects one back in.
#[tokio::test]
async fn xemu_upload_builds_the_udata_archive_and_restore_injects() {
    let root = TempDir::new().unwrap();
    let entry = xemu_fixture(root.path(), true);
    let fx = Fixture::new(config_with(entry, "Xbox"));
    let target = game("Halo", "Xbox", "7");

    let server = MockServer::start().await;
    mock_ok(&server, "POST", "/api/saves", json!({})).await;
    mock_ok(&server, "GET", "/api/saves", json!([])).await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();

    let report =
        upload_cloud_files_for_game(&client, &fx.ctx(), &mut caches, &target, SaveType::Save).await;
    assert_eq!(
        (report.uploaded, report.total),
        (1, 1),
        "{:?}",
        report.messages
    );

    let posts: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method.as_str() == "POST" && r.url.path() == "/api/saves")
        .collect();
    assert_eq!(posts.len(), 1);
    let query: std::collections::HashMap<_, _> = posts[0]
        .url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert_eq!(query.get("slot").map(String::as_str), Some("shared-media"));

    // Restore: a fresh UDATA archive is injected back into the image.
    let payload = zip_bytes(&[("UDATA/4541000d/00000001/savedata.bin", b"restored")]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/51/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload))
        .mount(&server)
        .await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();
    let record = json!({"id": 51, "emulator": "xemu", "slot": "shared-media"});
    let (ok, messages, update) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &target,
        Some(&record),
        false,
        false,
    )
    .await;
    assert!(ok, "{messages:?}");
    assert_eq!(update.last_downloaded_save_id.as_deref(), Some("51"));
}

/// D2: a legacy whole-image record is skipped with the notice, reported
/// as "nothing restored" (an Info message, not an error dialog).
#[tokio::test]
async fn xemu_legacy_record_is_skipped_with_the_notice() {
    let root = TempDir::new().unwrap();
    let entry = xemu_fixture(root.path(), true);
    let fx = Fixture::new(config_with(entry, "Xbox"));

    let legacy = zip_bytes(&[("xbox_hdd.qcow2", b"legacy-image")]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/61/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(legacy))
        .mount(&server)
        .await;
    let client = client_for(&server);
    let mut caches = CloudCaches::default();
    let record = json!({"id": 61, "emulator": "xemu"});
    let (ok, messages, update) = restore_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Halo", "Xbox", "7"),
        Some(&record),
        false,
        false,
    )
    .await;
    assert!(!ok);
    assert_eq!(update, SyncStateUpdate::default());
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].severity, MessageSeverity::Info);
    assert_eq!(
        messages[0].text,
        "This cloud save is a legacy whole-image xemu backup and cannot be restored by this version. Upload a new save to replace it."
    );
}

/// Spec "xemu flow": the image block reasons reach
/// [`block_reason_for_game`].
#[test]
fn xemu_block_reasons_surface_through_block_reason_for_game() {
    let root = TempDir::new().unwrap();

    // No hdd_path configured at all.
    let exe_dir = root.path().join("bin");
    fs::create_dir_all(&exe_dir).unwrap();
    let exe = exe_dir.join("xemu");
    fs::write(&exe, b"").unwrap();
    let bare = EmulatorEntry {
        name: "xemu".to_string(),
        path: exe.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let fx = Fixture::new(config_with(bare.clone(), "Xbox"));
    assert_eq!(
        block_reason_for_game(
            &fx.ctx(),
            &game("Halo", "Xbox", "7"),
            SaveType::Save,
            Some(&bare)
        ),
        "No xemu HDD image is configured, so cloud sync is unavailable."
    );

    // A qcow2 image.
    let qcow = root.path().join("xbox_hdd.qcow2");
    fs::write(&qcow, [0x51, 0x46, 0x49, 0xFB, 0, 0, 0, 0]).unwrap();
    fs::write(
        exe_dir.join("xemu.toml"),
        format!("[sys.files]\nhdd_path = '{}'\n", qcow.display()),
    )
    .unwrap();
    assert_eq!(
        block_reason_for_game(&fx.ctx(), &game("Halo", "Xbox", "7"), SaveType::Save, Some(&bare)),
        "xemu cloud sync needs a raw HDD image (xbox_hdd.img). Convert your qcow2 once with: qemu-img convert -O raw xbox_hdd.qcow2 xbox_hdd.img"
    );

    // States are never blocked by the image status.
    assert_eq!(
        block_reason_for_game(
            &fx.ctx(),
            &game("Halo", "Xbox", "7"),
            SaveType::State,
            Some(&bare)
        ),
        ""
    );
}

/// D11: an in-place FATX write to an image xemu still holds open risks
/// cross-linking, so a live xemu session blocks upload AND restore.
#[test]
fn a_running_xemu_session_blocks_syncing_its_saves() {
    let root = TempDir::new().unwrap();
    let entry = xemu_fixture(root.path(), true);
    let mut config = config_with(entry.clone(), "Xbox");
    config.emulators.push(entry_named("redream", ""));
    config
        .default_emulators
        .insert("Dreamcast".to_string(), "redream".to_string());
    let mut fx = Fixture::new(config);
    let target = game("Halo", "Xbox", "7");

    // Nothing running: the image is Ready, so nothing blocks.
    assert_eq!(
        block_reason_for_game(&fx.ctx(), &target, SaveType::Save, Some(&entry)),
        ""
    );

    // A session for an unrelated, non-xemu game changes nothing.
    fx.sessions.push(ActiveSessionRef {
        game: game("Sonic", "Dreamcast", "12"),
        started_at: 1_699_999_000.0,
    });
    assert_eq!(
        block_reason_for_game(&fx.ctx(), &target, SaveType::Save, Some(&entry)),
        ""
    );

    // A session for ANOTHER game that resolves to the same xemu entry
    // blocks — the image is shared, not per-game.
    fx.sessions.push(ActiveSessionRef {
        game: game("Halo 2", "Xbox", "9"),
        started_at: 1_699_999_500.0,
    });
    assert_eq!(
        block_reason_for_game(&fx.ctx(), &target, SaveType::Save, Some(&entry)),
        "xemu is running — close it before syncing its saves."
    );
}

// --- native ------------------------------------------------------------

/// Doc 06 "Upload — native games": the combined manifest archive, an
/// `emulator=native_multi_dir` POST, a total that is always 1, and
/// retention pruning keyed on the same name.
#[tokio::test]
async fn native_upload_manifest_flow_and_retention_key() {
    let server = MockServer::start().await;
    mock_ok(&server, "POST", "/api/saves", json!({})).await;
    mock_ok(
        &server,
        "GET",
        "/api/saves",
        json!([
            {"id": 1, "emulator": "native_multi_dir", "file_name": "a.zip", "updated_at": "2026-01-02T00:00:00Z"},
            {"id": 2, "emulator": "native_multi_dir", "file_name": "a.zip", "updated_at": "2026-01-01T00:00:00Z"}
        ]),
    )
    .await;
    mock_ok(&server, "POST", "/api/saves/delete", json!({})).await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    write_file(&saves.path().join("profile/save1.dat"), b"x");

    let config = Config {
        cloud_save_retention_limit: 1,
        ..Default::default()
    };
    let mut fx = Fixture::new(config);
    fx.pcgw = vec![saves.path().to_string_lossy().into_owned()];

    let report = super::native::upload_native_saves_for_game(
        &client,
        &fx.ctx(),
        &game("Portal", "Windows", "7"),
        &fx.pcgw,
    )
    .await;

    assert_eq!((report.uploaded, report.total), (1, 1));
    assert_eq!(texts(&report.messages), vec!["Uploaded 1 save files."]);

    let requests = server.received_requests().await.unwrap();
    let post = requests
        .iter()
        .find(|r| r.method.as_str() == "POST" && r.url.path() == "/api/saves")
        .unwrap();
    let query: std::collections::HashMap<_, _> = post
        .url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert_eq!(
        query.get("emulator").map(String::as_str),
        Some("native_multi_dir")
    );
    assert_eq!(query.get("overwrite").map(String::as_str), Some("true"));
    assert!(!query.contains_key("slot"));
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path() == "/api/saves/delete")
            .count(),
        1
    );
}

/// Doc 06 "Restore — native games": the legacy `native_dir:<raw path>`
/// record extracts the whole archive into that one directory.
#[tokio::test]
async fn native_restore_supports_the_legacy_native_dir_record() {
    let target = TempDir::new().unwrap();
    let payload = zip_bytes(&[("save1.dat", b"restored")]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/71/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let fx = Fixture::new(Config::default());
    let record = json!({
        "id": 71,
        "emulator": format!("native_dir:{}", target.path().display()),
    });
    let (ok, messages) = restore_native_cloud_save_for_game(
        &client,
        &fx.ctx(),
        &game("Portal", "Windows", "7"),
        &[],
        Some(&record),
    )
    .await;
    assert!(ok, "{messages:?}");
    assert_eq!(texts(&messages), vec![RESTORE_SUCCESS]);
    assert_eq!(
        fs::read(target.path().join("save1.dat")).unwrap(),
        b"restored"
    );
}

/// N1: removing a PCGW row suppresses it for UPLOAD, not just in the
/// popup's list — with the game's only PCGW row removed there is nothing
/// left to zip and the flow stops before it touches the server.
#[tokio::test]
async fn native_upload_skips_a_removed_pcgw_path() {
    let server = MockServer::start().await;
    let client = client_for(&server);

    let saves = TempDir::new().unwrap();
    write_file(&saves.path().join("profile/save1.dat"), b"x");
    let raw = saves.path().to_string_lossy().into_owned();

    let target = game("Portal", "Windows", "7");
    let mut config = Config::default();
    config
        .native_removed_save_paths
        .insert(super::native::manual_paths_key(&target), vec![raw.clone()]);
    let mut fx = Fixture::new(config);
    fx.pcgw = vec![raw];

    let report =
        super::native::upload_native_saves_for_game(&client, &fx.ctx(), &target, &fx.pcgw).await;

    assert_eq!((report.uploaded, report.total), (0, 0));
    assert_eq!(
        texts(&report.messages),
        vec![
            "No save locations are configured for this game. Use 'Manage Saves' → 'Browse' to add one."
        ]
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

/// N1, the other half: a removed PCGW row is not a RESTORE target either.
/// The record names neither `native_multi_dir` nor a `native_dir:` path, so
/// the archive lands in the first configured directory — which must be the
/// surviving row, not the removed one.
#[tokio::test]
async fn native_restore_skips_a_removed_pcgw_path() {
    let removed_dir = TempDir::new().unwrap();
    let kept_dir = TempDir::new().unwrap();
    let payload = zip_bytes(&[("save1.dat", b"restored")]);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/api/saves/71/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload))
        .mount(&server)
        .await;
    let client = client_for(&server);

    let target = game("Portal", "Windows", "7");
    let mut config = Config::default();
    config.native_removed_save_paths.insert(
        super::native::manual_paths_key(&target),
        vec![removed_dir.path().to_string_lossy().into_owned()],
    );
    let mut fx = Fixture::new(config);
    fx.pcgw = vec![
        removed_dir.path().to_string_lossy().into_owned(),
        kept_dir.path().to_string_lossy().into_owned(),
    ];

    let record = json!({"id": 71, "emulator": ""});
    let (ok, messages) =
        restore_native_cloud_save_for_game(&client, &fx.ctx(), &target, &fx.pcgw, Some(&record))
            .await;

    assert!(ok, "{messages:?}");
    assert_eq!(
        fs::read(kept_dir.path().join("save1.dat")).unwrap(),
        b"restored"
    );
    assert!(!removed_dir.path().join("save1.dat").exists());
}

// --- caches / gates ----------------------------------------------------

/// Doc 06 recorded quirk 9: the cloud emulator cache key omits the ROM id,
/// so two rows differing only by ROM id share an entry — and
/// [`CloudCaches::clear`] drops it.
#[test]
fn emulator_cache_key_omits_rom_id_and_clears_with_caches() {
    let saves = TempDir::new().unwrap();
    let entry = entry_named("Dolphin", saves.path().to_str().unwrap());
    let mut fx = Fixture::new(config_with(entry, "GameCube"));
    let mut caches = CloudCaches::default();

    let first = game("Zelda", "GameCube", "1");
    assert_eq!(
        resolved_cloud_emulator_entry(&fx.ctx(), &mut caches, &first, SaveType::Save)
            .map(|e| e.name),
        Some("Dolphin".to_string())
    );

    // Swap the config out entirely: the cached answer survives for a row
    // with the same title+platform but a different ROM id.
    fx.config = Config::default();
    let second = game("Zelda", "GameCube", "999");
    assert_eq!(
        resolved_cloud_emulator_entry(&fx.ctx(), &mut caches, &second, SaveType::Save)
            .map(|e| e.name),
        Some("Dolphin".to_string()),
        "the cache key ignores the ROM id"
    );

    caches.clear();
    assert_eq!(
        resolved_cloud_emulator_entry(&fx.ctx(), &mut caches, &second, SaveType::Save),
        None,
        "clear() drops the memo"
    );
}

/// Doc 06 "Block reasons", `_details_cloud_mode_supported`'s bullet list.
#[test]
fn details_cloud_mode_supported_gate_table() {
    let saves = TempDir::new().unwrap();
    let states = TempDir::new().unwrap();
    let mut entry = entry_named("Dolphin", saves.path().to_str().unwrap());
    entry.state_paths = states.path().to_str().unwrap().to_string();
    let fx = Fixture::new(config_with(entry.clone(), "GameCube"));
    let mut caches = CloudCaches::default();
    let ctx = fx.ctx();

    // native + state -> false; native + save -> installed only.
    let native = game("Portal", "Windows", "7");
    assert!(!details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &native,
        SaveType::State,
        true
    ));
    assert!(details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &native,
        SaveType::Save,
        true
    ));
    assert!(!details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &native,
        SaveType::Save,
        false
    ));

    // not installed and not on the Emulators platform -> false.
    let gc = game("Zelda", "GameCube", "7");
    assert!(!details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &gc,
        SaveType::Save,
        false
    ));

    // installed, resolvable, unblocked, directories present -> true.
    assert!(details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &gc,
        SaveType::Save,
        true
    ));
    assert!(details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &gc,
        SaveType::State,
        true
    ));

    // no resolvable emulator entry -> false.
    let other = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    assert!(!details_cloud_mode_supported(
        &other.ctx(),
        &mut caches,
        &gc,
        SaveType::Save,
        true
    ));

    // `state` on RPCS3 -> false.
    let mut rpcs3 = entry_named("RPCS3", saves.path().to_str().unwrap());
    rpcs3.state_paths = states.path().to_str().unwrap().to_string();
    let ps3 = Fixture::new(config_with(rpcs3, "PS3"));
    let mut caches = CloudCaches::default();
    assert!(!details_cloud_mode_supported(
        &ps3.ctx(),
        &mut caches,
        &game("Demon", "PS3", "7"),
        SaveType::State,
        true
    ));
    assert!(details_cloud_mode_supported(
        &ps3.ctx(),
        &mut caches,
        &game("Demon", "PS3", "7"),
        SaveType::Save,
        true
    ));

    // `save` on the Emulators platform with per-game scope -> false.
    // "Dolphin" is per-game, and the game's text names no shared-sync
    // emulator, so the shared-token scan finds nothing either.
    let emu_game = game("Dolphin package", "Emulators", "7");
    let mut caches = CloudCaches::default();
    assert_eq!(
        scope_for_game(&ctx, &emu_game, SaveType::Save, fx.config.emulators.first()),
        SaveScope::PerGame
    );
    assert!(!details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &emu_game,
        SaveType::Save,
        true
    ));

    // `state` on the Emulators platform -> false.
    let mut caches = CloudCaches::default();
    assert!(!details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &emu_game,
        SaveType::State,
        true
    ));

    // no resolved sync directories -> false.
    let no_dirs = Fixture::new(config_with(entry_named("Dolphin", ""), "GameCube"));
    let mut caches = CloudCaches::default();
    assert!(!details_cloud_mode_supported(
        &no_dirs.ctx(),
        &mut caches,
        &gc,
        SaveType::Save,
        true
    ));
}

/// The record listing drops screenshot assets from state results
/// (cloud_mixin.py:1653).
#[tokio::test]
async fn state_record_listing_filters_image_file_names() {
    let server = MockServer::start().await;
    mock_ok(
        &server,
        "GET",
        "/api/states",
        json!([
            {"id": 1, "file_name": "game.state"},
            {"id": 2, "file_name": "game.state.PNG"},
            {"id": 3}
        ]),
    )
    .await;
    let client = client_for(&server);

    let records = fetch_cloud_records_for_rom(&client, "7", SaveType::State)
        .await
        .unwrap();
    let ids: Vec<String> = records.iter().map(record_id_for_tests).collect();
    assert_eq!(ids, vec!["1".to_string(), "3".to_string()]);
}

/// Doc 06 "Candidate discovery": the dispatch table's first row (state ->
/// files only, returned straight after session filtering), the folder
/// strategy, and the explicit-file-root rescan.
#[test]
fn cloud_sync_targets_dispatch_table() {
    let saves = TempDir::new().unwrap();
    let states = TempDir::new().unwrap();
    write_file(&saves.path().join("zelda/memcard.bin"), b"m");
    write_file(&states.path().join("zelda.state"), b"s");

    let mut entry = entry_named("Nestopia", saves.path().to_str().unwrap());
    entry.state_paths = states.path().to_str().unwrap().to_string();
    let fx = Fixture::new(config_with(entry.clone(), "NES"));
    let mut caches = CloudCaches::default();
    let target = game("Zelda", "NES", "7");

    // Row 1: state -> file candidates only, never folder targets.
    let (files, folders) =
        cloud_sync_targets(&fx.ctx(), &mut caches, &target, &entry, SaveType::State);
    assert_eq!(files, vec![states.path().join("zelda.state")]);
    assert!(folders.is_empty());

    // Row 4: an explicit `folder` strategy yields generic folder targets.
    let mut folder_entry = entry.clone();
    folder_entry.save_strategy = "folder".to_string();
    let fx = Fixture::new(config_with(folder_entry.clone(), "NES"));
    let mut caches = CloudCaches::default();
    let (files, folders) = cloud_sync_targets(
        &fx.ctx(),
        &mut caches,
        &target,
        &folder_entry,
        SaveType::Save,
    );
    assert!(files.is_empty());
    assert_eq!(folders, vec![saves.path().join("zelda")]);

    // The explicit-file-root rescan: a configured path that IS a file is
    // rescanned as a root when the chosen branch found nothing.
    let loose = TempDir::new().unwrap();
    let file_root = loose.path().join("unrelated-name.srm");
    write_file(&file_root, b"x");
    let root_entry = entry_named("Nestopia", file_root.to_str().unwrap());
    let fx = Fixture::new(config_with(root_entry.clone(), "NES"));
    let mut caches = CloudCaches::default();
    let (files, folders) =
        cloud_sync_targets(&fx.ctx(), &mut caches, &target, &root_entry, SaveType::Save);
    assert_eq!(files, vec![file_root]);
    assert!(folders.is_empty());
}

/// D1: xemu contributes no generic save candidates, and its local mtime
/// stands in as the raw image file's own.
#[test]
fn xemu_contributes_no_generic_candidates_and_uses_the_image_mtime() {
    let root = TempDir::new().unwrap();
    let entry = xemu_fixture(root.path(), true);
    write_file(&root.path().join("halo.bin"), b"x");
    let fx = Fixture::new(config_with(entry.clone(), "Xbox"));
    let mut caches = CloudCaches::default();
    let target = game("Halo", "Xbox", "7");

    let (files, folders) =
        cloud_sync_targets(&fx.ctx(), &mut caches, &target, &entry, SaveType::Save);
    assert!(files.is_empty());
    assert!(folders.is_empty());

    let image = root.path().join("xbox_hdd.img");
    set_mtime(&image, 1_234_567_890);
    let mtime = latest_local_save_mtime(&fx.ctx(), &mut caches, &target, "xemu");
    assert_eq!(mtime as u64, 1_234_567_890);
}

/// `_latest_local_state_mtime_for_game` returns `0.0` for RPCS3
/// (cloud_mixin.py:1545).
#[test]
fn latest_local_state_mtime_is_zero_for_rpcs3() {
    let states = TempDir::new().unwrap();
    let file = states.path().join("demon.state");
    write_file(&file, b"s");
    set_mtime(&file, 1_600_000_000);

    let mut rpcs3 = entry_named("RPCS3", "");
    rpcs3.state_paths = states.path().to_str().unwrap().to_string();
    let mut plain = entry_named("Nestopia", "");
    plain.state_paths = states.path().to_str().unwrap().to_string();

    let target = game("Demon", "PS3", "7");
    let fx = Fixture::new(config_with(rpcs3, "PS3"));
    let mut caches = CloudCaches::default();
    assert_eq!(
        latest_local_state_mtime(&fx.ctx(), &mut caches, &target, "RPCS3"),
        0.0
    );

    let fx = Fixture::new(config_with(plain, "PS3"));
    let mut caches = CloudCaches::default();
    assert_eq!(
        latest_local_state_mtime(&fx.ctx(), &mut caches, &target, "Nestopia") as u64,
        1_600_000_000
    );
}

/// Doc 06 "Save scope": for saves a shared-sync owner's ROM id replaces
/// the game's own; states never take the indirection.
#[test]
fn shared_owner_rom_id_applies_to_saves_only() {
    let root = TempDir::new().unwrap();
    let entry = xemu_fixture(root.path(), true);
    let mut fx = Fixture::new(config_with(entry, "Xbox"));
    fx.games = vec![CloudGame {
        title: "xemu".to_string(),
        platform: "Emulators".to_string(),
        rom_id: "999".to_string(),
        ..Default::default()
    }];
    let mut caches = CloudCaches::default();
    let target = game("Halo", "Xbox", "7");

    assert_eq!(
        cloud_sync_rom_id(&fx.ctx(), &mut caches, &target, SaveType::Save),
        Some("999".to_string())
    );
    assert_eq!(
        cloud_sync_rom_id(&fx.ctx(), &mut caches, &target, SaveType::State),
        Some("7".to_string())
    );
}

/// Fix round 1 (FIX 1, ruling: Python wins): the generic state branch
/// guards only on "no candidates" (`cloud_mixin.py:2565`), so a run that
/// builds zero jobs must still fall through to the completion table and
/// report `"Uploaded 0 save states."` — never the "no matching states"
/// info, and never silence.
///
/// The ops layer forwards `upload_completion_message`'s result
/// unconditionally: the two assertions below pin the message for a
/// zero-attempt outcome and the absence of any ops-side guard around it.
///
/// NOTE on reachability, found while writing this test: with the guard
/// removed, no public-API input can currently produce a zero-job state
/// run. Candidate discovery (`file_candidates`) and both state job
/// builders (`retroarch_state_upload_jobs`,
/// `grouped_file_upload_jobs`) apply the SAME ignore sets and the same
/// `is_file()` existence filter — in Python too (`cloud_mixin.py:2568`
/// re-uses the discovery sets verbatim) — so non-empty candidates always
/// yield at least one job. The guard removal is therefore defensive as
/// well as correct: it makes the port match Python's shape at a seam
/// neither codebase can reach today, instead of hard-coding an
/// assumption that could silently swallow a future builder's empty
/// result.
#[tokio::test]
async fn zero_job_state_upload_still_reports_uploaded_zero() {
    // The exact string the ruling names, straight off the seam ops uses.
    assert_eq!(
        crate::cloud::transfer::upload_completion_message(
            &crate::cloud::transfer::UploadOutcome::default(),
            SaveType::State,
            0,
            3,
        ),
        ("Uploaded 0 save states.".to_string(), MessageSeverity::Info)
    );

    // And ops forwards that table's output with no guard of its own: a
    // real state run reports the completion message, not a "no matching"
    // info, for every non-empty candidate set.
    let server = MockServer::start().await;
    mock_ok(&server, "POST", "/api/states", json!({})).await;
    let client = client_for(&server);

    let states = TempDir::new().unwrap();
    write_file(&states.path().join("metroid.state"), b"s");
    let mut entry = entry_named("Nestopia", "");
    entry.state_paths = states.path().to_str().unwrap().to_string();
    let fx = Fixture::new(config_with(entry, "NES"));
    let mut caches = CloudCaches::default();

    let report = upload_cloud_files_for_game(
        &client,
        &fx.ctx(),
        &mut caches,
        &game("Metroid", "NES", "7"),
        SaveType::State,
    )
    .await;
    assert_eq!((report.uploaded, report.total), (1, 1));
    assert_eq!(texts(&report.messages), vec!["Uploaded 1 save states."]);
    assert_eq!(report.messages[0].severity, MessageSeverity::Info);
}

/// Fix round 1 (FIX 2): an xemu image problem blocks the ACTION but must
/// not hide the panel — `details_cloud_mode_supported` consults the BASE
/// reason, `block_reason_for_game` adds the image guidance.
#[test]
fn xemu_image_status_blocks_the_action_but_not_the_panel() {
    let root = TempDir::new().unwrap();
    let exe_dir = root.path().join("bin");
    fs::create_dir_all(&exe_dir).unwrap();
    let exe = exe_dir.join("xemu");
    fs::write(&exe, b"").unwrap();

    // A qcow2 image: NotRaw.
    let qcow = root.path().join("xbox_hdd.qcow2");
    fs::write(&qcow, [0x51, 0x46, 0x49, 0xFB, 0, 0, 0, 0]).unwrap();
    fs::write(
        exe_dir.join("xemu.toml"),
        format!("[sys.files]\nhdd_path = '{}'\n", qcow.display()),
    )
    .unwrap();

    let entry = EmulatorEntry {
        name: "xemu".to_string(),
        path: exe.to_string_lossy().into_owned(),
        save_paths: root.path().to_string_lossy().into_owned(),
        ..Default::default()
    };
    let fx = Fixture::new(config_with(entry.clone(), "Xbox"));
    let ctx = fx.ctx();
    let target = game("Halo", "Xbox", "7");

    // The action gate carries the conversion guidance ...
    assert_eq!(
        block_reason_for_game(&ctx, &target, SaveType::Save, Some(&entry)),
        "xemu cloud sync needs a raw HDD image (xbox_hdd.img). Convert your qcow2 once with: qemu-img convert -O raw xbox_hdd.qcow2 xbox_hdd.img"
    );
    // ... while the base reason stays empty ...
    assert_eq!(
        base_block_reason_for_game(&ctx, &target, SaveType::Save, Some(&entry)),
        ""
    );
    // ... so the panel still shows, and the user can read the guidance.
    let mut caches = CloudCaches::default();
    assert!(details_cloud_mode_supported(
        &ctx,
        &mut caches,
        &target,
        SaveType::Save,
        true
    ));
}

fn record_id_for_tests(record: &Value) -> String {
    crate::cloud::restore::stringify_id(record.get("id").unwrap_or(&Value::Null))
}

// ---------------------------------------------------------------------
// restore_enabled_for_record (fix round 1, FIX 4)
// ---------------------------------------------------------------------

fn record_with_emulator(emulator: &str) -> Value {
    json!({"id": 1, "emulator": emulator, "file_name": "save.srm"})
}

#[test]
fn restore_enabled_native_multi_dir_is_always_enabled_with_no_lookup() {
    // No emulators configured at all: any other record would fail every
    // later gate, but `native_multi_dir` short-circuits before any of
    // them run (tests/test_details_cloud_native_panel.py:100).
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let target = game("Alpha", "Windows", "7");
    let record = record_with_emulator("native_multi_dir");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::Save, &record);
    assert!(enabled);
    assert_eq!(text, "");
}

#[test]
fn restore_enabled_refuses_on_the_compatibility_block_reason() {
    // A native-executable platform's own block reason fires regardless
    // of the record's named emulator.
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let target = game("Alpha", "Windows", "7");
    let record = record_with_emulator("Dolphin");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::Save, &record);
    assert!(!enabled);
    assert_eq!(
        text,
        "Cloud save management is only available for emulator-based games."
    );
}

#[test]
fn restore_enabled_refuses_rpcs3_state_restore() {
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let target = game("Demon's Souls", "PS3", "7");
    let record = record_with_emulator("RPCS3");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::State, &record);
    assert!(!enabled);
    assert_eq!(text, "RPCS3 savestate restore is not supported yet.");
}

#[test]
fn restore_enabled_refuses_an_unconfigured_record_emulator() {
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let target = game("Chrono Trigger", "SNES", "7");
    let record = record_with_emulator("SomeUnknownEmu");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::Save, &record);
    assert!(!enabled);
    assert_eq!(
        text,
        "Configure emulator 'SomeUnknownEmu' in Emulators to restore this entry."
    );
}

#[test]
fn restore_enabled_refuses_when_no_default_emulator_is_configured() {
    let fx = Fixture::new(Config::default());
    let mut caches = CloudCaches::default();
    let target = game("Chrono Trigger", "SNES", "7");
    // Blank emulator field: falls back to the (nonexistent) platform
    // default.
    let record = record_with_emulator("");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::Save, &record);
    assert!(!enabled);
    assert_eq!(text, "No default emulator is configured for this platform.");
}

#[test]
fn restore_enabled_refuses_when_the_emulator_has_no_configured_directories() {
    let entry = entry_named("Dolphin", "");
    let fx = Fixture::new(config_with(entry, "GameCube"));
    let mut caches = CloudCaches::default();
    let target = game("Zelda", "GameCube", "7");
    let record = record_with_emulator("Dolphin");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::Save, &record);
    assert!(!enabled);
    assert_eq!(
        text,
        "No configured save directories were found for emulator 'Dolphin'."
    );
}

#[test]
fn restore_enabled_true_with_the_shared_scope_notice_as_tooltip() {
    let saves = TempDir::new().unwrap();
    let entry = entry_named("xemu", saves.path().to_str().unwrap());
    let fx = Fixture::new(config_with(entry, "Xbox"));
    let mut caches = CloudCaches::default();
    let target = game("Halo", "Xbox", "7");
    // Blank emulator field: resolves to the configured default (xemu).
    let record = record_with_emulator("");

    let (enabled, text) =
        restore_enabled_for_record(&fx.ctx(), &mut caches, &target, SaveType::Save, &record);
    assert!(enabled);
    assert_eq!(
        text,
        "These cloud saves are shared xemu media. Restoring or deleting one affects every game using this emulator."
    );
}
