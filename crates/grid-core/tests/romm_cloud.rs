//! Wiremock coverage for the RomM save/state endpoints
//! (`grid_core::romm::RommClient`'s cloud methods) and client-side
//! retention pruning (`grid_core::cloud::retention::prune_server_save_records`).
//! See `docs/porting/06-cloud-saves.md`, "Server endpoints" and "Retention
//! pruning", and `grid_launcher/ui/mixins/cloud_mixin.py:1593-1765,2478-
//! 2624` for the ported behavior.

use std::fs;
use std::path::PathBuf;

use grid_core::cloud::retention::prune_server_save_records;
use grid_core::romm::{RommClient, RommError};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

async fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(&server.uri(), token_cred()).unwrap()
}

fn query_pairs(url: &url::Url) -> std::collections::HashMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

// --- list saves / states -----------------------------------------------

#[tokio::test]
async fn saves_and_states_list_send_rom_id_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id": 1}])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/states"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id": 2}])))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    client.saves_for_rom("42").await.unwrap();
    client.states_for_rom("42").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let saves_req = requests
        .iter()
        .find(|r| r.url.path() == "/api/saves")
        .unwrap();
    let states_req = requests
        .iter()
        .find(|r| r.url.path() == "/api/states")
        .unwrap();
    assert_eq!(query_pairs(&saves_req.url).get("rom_id").unwrap(), "42");
    assert_eq!(query_pairs(&states_req.url).get("rom_id").unwrap(), "42");
}

// --- save content: id percent-encoding ----------------------------------

#[tokio::test]
async fn save_content_percent_encodes_the_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/saves/abc%2Fdef/content"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"save-bytes".to_vec()))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let bytes = client.save_content("abc/def").await.unwrap();
    assert_eq!(bytes, b"save-bytes");
}

// --- upload save: query + multipart shape --------------------------------

fn temp_payload(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn upload_save_sends_query_and_multipart_shape() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let temp = tempfile::tempdir().unwrap();
    let save_file = temp_payload(&temp, "Chrono Trigger.srm", b"save-bytes");
    let client = client_for(&server).await;

    // With a slot: present in the query.
    client
        .upload_save(
            "42",
            "Snes9x",
            Some("vmu0"),
            10,
            &[("saveFile".to_string(), save_file.clone())],
        )
        .await
        .unwrap();

    // Without a slot (None): absent from the query.
    client
        .upload_save(
            "42",
            "Snes9x",
            None,
            10,
            &[("saveFile".to_string(), save_file.clone())],
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);

    let with_slot = &requests[0];
    let query = query_pairs(&with_slot.url);
    assert_eq!(query.get("overwrite").unwrap(), "true");
    assert_eq!(query.get("rom_id").unwrap(), "42");
    assert_eq!(query.get("emulator").unwrap(), "Snes9x");
    assert_eq!(query.get("slot").unwrap(), "vmu0");
    // Q9: a slotted upload hands retention for its rom+slot group to the
    // server.
    assert_eq!(query.get("autocleanup").unwrap(), "true");
    assert_eq!(query.get("autocleanup_limit").unwrap(), "10");
    let body = String::from_utf8_lossy(&with_slot.body);
    assert!(body.contains("name=\"saveFile\""), "body: {body}");
    assert!(
        body.contains("filename=\"Chrono Trigger.srm\""),
        "body: {body}"
    );
    assert!(body.contains("save-bytes"));

    let without_slot = &requests[1];
    let query = query_pairs(&without_slot.url);
    assert_eq!(query.get("overwrite").unwrap(), "true");
    assert!(!query.contains_key("slot"), "query: {query:?}");
    // The server never cleans null-slot saves; the client owns them.
    assert!(!query.contains_key("autocleanup"), "query: {query:?}");
    assert!(!query.contains_key("autocleanup_limit"), "query: {query:?}");
}

// --- upload multipart: per-part Content-Type (fix round 1) ---------------

#[tokio::test]
async fn upload_save_multipart_parts_carry_a_guessed_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let temp = tempfile::tempdir().unwrap();
    let save_file = temp_payload(&temp, "Chrono Trigger.srm", b"save-bytes");
    let screenshot_file = temp_payload(&temp, "Chrono Trigger.png", b"\x89PNG\r\n\x1a\n");
    let client = client_for(&server).await;

    client
        .upload_save(
            "42",
            "Snes9x",
            None,
            10,
            &[
                ("saveFile".to_string(), save_file),
                ("screenshotFile".to_string(), screenshot_file),
            ],
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&requests[0].body);
    // `.srm` has no recognised mime type, so it falls back to the generic
    // octet stream — matching `mimetypes.guess_type` returning `None` for
    // an unrecognised extension (`api.py:96`'s `or "application/octet-stream"`).
    assert!(
        body.contains("Content-Type: application/octet-stream"),
        "body: {body}"
    );
    assert!(body.contains("Content-Type: image/png"), "body: {body}");
}

// --- upload multipart: missing payload path is skipped (fix round 1) -----

#[tokio::test]
async fn upload_save_skips_a_missing_payload_path_instead_of_failing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let temp = tempfile::tempdir().unwrap();
    let save_file = temp_payload(&temp, "Chrono Trigger.srm", b"save-bytes");
    let missing_screenshot = temp.path().join("does-not-exist.png");
    let client = client_for(&server).await;

    // The whole call still succeeds even though one payload path never
    // existed — Python silently skips it (`api.py:93`) rather than
    // aborting the upload.
    client
        .upload_save(
            "42",
            "Snes9x",
            None,
            10,
            &[
                ("saveFile".to_string(), save_file),
                ("screenshotFile".to_string(), missing_screenshot),
            ],
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(body.contains("name=\"saveFile\""), "body: {body}");
    assert!(
        !body.contains("name=\"screenshotFile\""),
        "the missing sidecar must never become a part: {body}"
    );
}

// --- upload state: no slot, no overwrite ---------------------------------

#[tokio::test]
async fn upload_state_sends_no_slot_and_no_overwrite() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/states"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let temp = tempfile::tempdir().unwrap();
    let state_file = temp_payload(&temp, "Chrono Trigger.state1", b"state-bytes");
    let client = client_for(&server).await;

    client
        .upload_state("42", "Snes9x", &[("stateFile".to_string(), state_file)])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let query = query_pairs(&requests[0].url);
    assert!(!query.contains_key("autocleanup"), "query: {query:?}");
    let query = query_pairs(&requests[0].url);
    assert_eq!(query.get("rom_id").unwrap(), "42");
    assert_eq!(query.get("emulator").unwrap(), "Snes9x");
    assert!(!query.contains_key("overwrite"), "query: {query:?}");
    assert!(!query.contains_key("slot"), "query: {query:?}");
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(body.contains("name=\"stateFile\""), "body: {body}");
    assert!(
        body.contains("filename=\"Chrono Trigger.state1\""),
        "body: {body}"
    );
}

// --- delete bodies: saves vs states --------------------------------------

#[tokio::test]
async fn delete_bodies_use_the_right_keys() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .and(body_json(json!({"saves": [7]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([7])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/states/delete"))
        .and(body_json(json!({"states": [9]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([9])))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    assert_eq!(client.delete_save(7).await.unwrap(), 200);
    assert_eq!(client.delete_state(9).await.unwrap(), 200);
}

// --- one-id delete helpers: 404 is "already gone" ------------------------

#[tokio::test]
async fn delete_record_helpers_send_one_id_and_treat_404_as_success() {
    let server = MockServer::start().await;
    for (route, key, id, status) in [
        ("/api/states/delete", "states", 5, 200),
        ("/api/states/delete", "states", 6, 404),
        ("/api/states/delete", "states", 7, 410),
        ("/api/states/delete", "states", 8, 500),
        ("/api/saves/delete", "saves", 15, 404),
        ("/api/saves/delete", "saves", 16, 500),
    ] {
        Mock::given(method("POST"))
            .and(path(route))
            .and(body_json(json!({ key: [id] })))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;
    }
    let client = client_for(&server).await;

    assert!(client.delete_state_record(5).await.is_ok());
    assert!(client.delete_state_record(6).await.is_ok());
    assert!(client.delete_state_record(7).await.is_ok());
    assert!(matches!(
        client.delete_state_record(8).await,
        Err(RommError::Http { status: 500, .. })
    ));
    assert!(client.delete_save_record(15).await.is_ok());
    assert!(matches!(
        client.delete_save_record(16).await,
        Err(RommError::Http { status: 500, .. })
    ));

    // One id per request, never a batch.
    for request in server.received_requests().await.unwrap() {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let list = body
            .get("states")
            .or_else(|| body.get("saves"))
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(list.len(), 1, "{body}");
    }
}

// --- get_relative_bytes: leading slash + D4 absolute rejection -----------

#[tokio::test]
async fn get_relative_bytes_prefixes_a_slash_and_rejects_absolute_urls() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/downloads/save.dat"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"relative-bytes".to_vec()))
        .mount(&server)
        .await;

    let client = client_for(&server).await;

    // No leading slash: one gets added before the request.
    let bytes = client
        .get_relative_bytes("downloads/save.dat")
        .await
        .unwrap();
    assert_eq!(bytes, b"relative-bytes");

    // D4: an absolute http(s) candidate is rejected outright, never
    // fetched from here — the caller is expected to skip to the next
    // candidate on Err.
    let err = client
        .get_relative_bytes("http://evil.example/save.dat")
        .await
        .unwrap_err();
    assert!(matches!(err, RommError::InvalidUrl));
    let err = client
        .get_relative_bytes("https://evil.example/save.dat")
        .await
        .unwrap_err();
    assert!(matches!(err, RommError::InvalidUrl));
}

// --- retention pruning ----------------------------------------------------

/// A null-slot save record, grouped by its file stem. Q9: the client prune
/// owns only null-slot groups; a slotted record belongs to the server's
/// `autocleanup` (see `prune_never_deletes_slotted_records`).
fn save_record(id: i64, stem: &str, updated_at: &str, emulator: &str) -> serde_json::Value {
    json!({"id": id, "slot": null, "file_name": format!("{stem}.bin"), "updated_at": updated_at, "emulator": emulator})
}

#[tokio::test]
async fn prune_keeps_n_per_slot_and_treats_404_as_success() {
    let server = MockServer::start().await;
    let records = json!([
        save_record(10, "vmu0", "2026-04-08T09:00:00Z", "Redream"),
        save_record(11, "vmu0", "2026-04-08T10:00:00Z", "Redream"),
        save_record(12, "vmu0", "2026-04-08T11:00:00Z", "Redream"), // newest in vmu0: kept
        save_record(20, "vmu1", "2026-04-08T08:00:00Z", "Redream"),
        save_record(21, "vmu1", "2026-04-08T09:00:00Z", "Redream"), // newest in vmu1: kept
    ]);
    Mock::given(method("GET"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(records))
        .mount(&server)
        .await;
    // Stale: 11 (200 success), 10 (404 -> treated as success), 20 (200 success).
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .and(body_json(json!({"saves": [11]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([11])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .and(body_json(json!({"saves": [10]})))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .and(body_json(json!({"saves": [20]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([20])))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let (deleted, failed) = prune_server_save_records(&client, "42", "Redream", 1).await;

    assert_eq!(deleted, 3, "11, 10 (404), and 20 all count as deleted");
    assert!(failed.is_empty(), "failed: {failed:?}");
}

#[tokio::test]
async fn prune_mismatched_emulator_prunes_nothing() {
    let server = MockServer::start().await;
    let records = json!([
        save_record(1, "vmu0", "2026-04-08T09:00:00Z", "Snes9x"),
        save_record(2, "vmu0", "2026-04-08T10:00:00Z", "Snes9x"),
        save_record(3, "vmu0", "2026-04-08T11:00:00Z", "Snes9x"),
    ]);
    Mock::given(method("GET"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(records))
        .mount(&server)
        .await;
    // No delete mock mounted at all: any delete request would 404
    // (wiremock's unmatched-request default) and, since 404 counts as a
    // successful delete, would silently corrupt `deleted` — so a stray
    // delete call is still caught by the count assertions below even
    // without inspecting `received_requests()` directly.

    let client = client_for(&server).await;
    // "Redream" matches nothing in this list — unlike
    // `latest_server_record`'s fall-back-to-all, retention prunes NOTHING
    // rather than falling back to every record.
    let (deleted, failed) = prune_server_save_records(&client, "42", "Redream", 1).await;

    assert_eq!(deleted, 0);
    assert!(failed.is_empty());
    let delete_requests: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.url.path() == "/api/saves/delete")
        .collect();
    assert!(
        delete_requests.is_empty(),
        "a mismatched emulator must never issue a delete request"
    );
}

#[tokio::test]
async fn prune_blank_id_skipped_non_integer_failed() {
    let server = MockServer::start().await;
    let records = json!([
        // Newest: kept (within the keep=1 budget).
        {"id": 5, "file_name": "vmu0.bin", "updated_at": "2026-04-08T11:00:00Z", "emulator": "Redream"},
        // Blank id: dropped upstream by `server_records_from_payload`
        // itself (it never becomes a matching record at all) — the
        // observable contract is identical to being dropped in this
        // function's own loop: never requested, never counted.
        {"id": "", "file_name": "vmu0.bin", "updated_at": "2026-04-08T10:00:00Z", "emulator": "Redream"},
        // Non-integer id, oldest: stale, and fails WITHOUT a delete
        // request ever being sent.
        {"id": "abc", "file_name": "vmu0.bin", "updated_at": "2026-04-08T09:00:00Z", "emulator": "Redream"},
    ]);
    Mock::given(method("GET"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(records))
        .mount(&server)
        .await;
    // No delete mock mounted: if the implementation wrongly sends a
    // request for either the blank or the non-integer id, wiremock's
    // unmatched-request 404 would (wrongly) count as a successful
    // deletion, so a bug here still shows up as a wrong `deleted` count.

    let client = client_for(&server).await;
    let (deleted, failed) = prune_server_save_records(&client, "42", "Redream", 1).await;

    assert_eq!(deleted, 0);
    assert_eq!(failed, vec!["abc".to_string()]);
    let delete_requests: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.url.path() == "/api/saves/delete")
        .collect();
    assert!(
        delete_requests.is_empty(),
        "neither a blank nor a non-integer id should ever reach a delete request"
    );
}

// --- retention: refetch failure surfaces as a synthetic failed id
// (fix round 1, controller ruling) --------------------------------------

#[tokio::test]
async fn prune_refetch_failure_returns_a_synthetic_failed_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(500).set_body_string("db is down"))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let (deleted, failed) = prune_server_save_records(&client, "42", "Redream", 1).await;

    assert_eq!(deleted, 0);
    assert_eq!(failed.len(), 1, "failed: {failed:?}");
    assert!(!failed[0].is_empty(), "failed id text must not be blank");
    // Token secrecy: the error text must never carry the credential this
    // client authenticated with.
    assert!(!failed[0].contains("Authorization"), "failed: {failed:?}");
    assert!(!failed[0].contains("Bearer"), "failed: {failed:?}");
}

// --- Q9: owner per group, limit 0, and state pruning ---------------------

async fn delete_requests(server: &MockServer, route: &str) -> Vec<serde_json::Value> {
    server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.url.path() == route)
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect()
}

#[tokio::test]
async fn prune_never_deletes_slotted_records() {
    let server = MockServer::start().await;
    let records = json!([
        {"id": 1, "slot": "vmu0", "file_name": "vmu0.bin", "updated_at": "2026-04-08T09:00:00Z", "emulator": "Redream"},
        {"id": 2, "slot": "vmu0", "file_name": "vmu0.bin", "updated_at": "2026-04-08T10:00:00Z", "emulator": "Redream"},
        {"id": 3, "slot": "vmu0", "file_name": "vmu0.bin", "updated_at": "2026-04-08T11:00:00Z", "emulator": "Redream"},
        // Null slot, same emulator: this group is the client's.
        {"id": 4, "slot": null, "file_name": "sonic.bin", "updated_at": "2026-04-08T09:00:00Z", "emulator": "Redream"},
        {"id": 5, "slot": null, "file_name": "sonic.bin", "updated_at": "2026-04-08T10:00:00Z", "emulator": "Redream"},
    ]);
    Mock::given(method("GET"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(200).set_body_json(records))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let (deleted, failed) = prune_server_save_records(&client, "42", "Redream", 1).await;

    assert_eq!(deleted, 1);
    assert!(failed.is_empty(), "failed: {failed:?}");
    assert_eq!(
        delete_requests(&server, "/api/saves/delete").await,
        vec![json!({"saves": [4]})]
    );
}

#[tokio::test]
async fn prune_limit_zero_is_unlimited_and_sends_nothing() {
    let server = MockServer::start().await;
    let client = client_for(&server).await;

    assert_eq!(
        prune_server_save_records(&client, "42", "Redream", 0).await,
        (0, Vec::new())
    );
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "limit 0 must not even list the records"
    );
}

// --- 401 vs 403 on the write paths ----------------------------------------

#[tokio::test]
async fn delete_save_maps_401_to_unauthorized_and_403_to_forbidden() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .and(body_json(json!({"saves": [7]})))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/saves/delete"))
        .and(body_json(json!({"saves": [8]})))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let client = client_for(&server).await;
    assert!(matches!(
        client.delete_save(7).await,
        Err(RommError::Unauthorized)
    ));
    assert!(matches!(
        client.delete_save(8).await,
        Err(RommError::Forbidden)
    ));
}

#[tokio::test]
async fn upload_save_maps_403_to_forbidden() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/saves"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let file = temp_payload(&dir, "game.srm", b"save");
    let client = client_for(&server).await;
    let err = client
        .upload_save(
            "1",
            "retroarch",
            None,
            10,
            &[("game.srm".to_string(), file)],
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RommError::Forbidden), "{err:?}");
}
