//! Wiremock coverage for `POST /api/play-sessions`
//! (`RommClient::post_play_sessions`) and the outbox flush
//! (`grid_core::play_activity::flush`). The body and result shapes follow
//! the B4 live probe (docs/superpowers/plans/2026-10-10-parity-decisions-milestone.md).

use std::sync::Arc;

use grid_core::library::registry::Registry;
use grid_core::play_activity::{flush, FLUSH_BATCH_SIZE};
use grid_core::romm::{IngestStatus, PlaySessionEntry, RommClient, RommError};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::{json, Value};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

fn entry(rom_id: i64, minute: u32) -> PlaySessionEntry {
    PlaySessionEntry {
        rom_id,
        start_time: format!("2026-10-10T12:{minute:02}:00.000Z"),
        end_time: format!("2026-10-10T12:{minute:02}:45.000Z"),
        duration_ms: 45_000,
    }
}

fn temp_registry() -> (tempfile::TempDir, Arc<Registry>) {
    let dir = tempfile::tempdir().unwrap();
    let registry = Arc::new(Registry::open(&dir.path().join("grid-launcher.db")).unwrap());
    (dir, registry)
}

/// Answers every entry of the request body with `status`, by index.
struct AnswerAll(&'static str);

impl Respond for AnswerAll {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let count = body["sessions"].as_array().unwrap().len();
        let results: Vec<Value> = (0..count)
            .map(|index| json!({"index": index, "status": self.0, "id": index + 1}))
            .collect();
        ResponseTemplate::new(201).set_body_json(json!({
            "results": results,
            "created_count": count,
            "skipped_count": 0
        }))
    }
}

#[tokio::test]
async fn post_play_sessions_sends_the_probe_shape_and_reads_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .and(header("authorization", "Bearer FAKE-TEST-TOKEN-not-real"))
        .and(body_json(json!({
            "sessions": [{
                "rom_id": 42,
                "start_time": "2026-10-10T12:00:00.000Z",
                "end_time": "2026-10-10T12:00:45.000Z",
                "duration_ms": 45_000
            }]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "results": [{"index": 0, "status": "created", "id": 77}],
            "created_count": 1,
            "skipped_count": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    let results = client_for(&server)
        .post_play_sessions(&[entry(42, 0)])
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, IngestStatus::Created);
    assert_eq!(results[0].id, Some(77));
}

#[tokio::test]
async fn post_play_sessions_maps_401_and_5xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
        .mount(&server)
        .await;
    let client = client_for(&server);
    assert!(matches!(
        client.post_play_sessions(&[entry(42, 0)]).await,
        Err(RommError::Unauthorized)
    ));
    assert!(matches!(
        client.post_play_sessions(&[entry(42, 0)]).await,
        Err(RommError::Http { status: 502, .. })
    ));
}

#[tokio::test]
async fn flush_deletes_created_and_duplicate_rows() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "results": [
                {"index": 0, "status": "created", "id": 1},
                {"index": 1, "status": "duplicate", "id": null}
            ],
            "created_count": 1,
            "skipped_count": 1
        })))
        .expect(1)
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    registry.enqueue_play_session(&entry(42, 0)).unwrap();
    registry.enqueue_play_session(&entry(43, 1)).unwrap();

    let report = flush(registry.clone(), &client_for(&server)).await.unwrap();
    assert_eq!(report.accepted, 2);
    assert_eq!(report.dropped, 0);
    assert_eq!(report.stopped, None);
    assert!(registry.pending_play_sessions(0, 10).unwrap().is_empty());
}

#[tokio::test]
async fn flush_keeps_every_row_on_a_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    registry.enqueue_play_session(&entry(42, 0)).unwrap();
    registry.enqueue_play_session(&entry(43, 1)).unwrap();

    let report = flush(registry.clone(), &client_for(&server)).await.unwrap();
    assert_eq!(report.accepted, 0);
    assert_eq!(report.dropped, 0);
    assert_eq!(report.stopped.as_deref(), Some("server error 500"));
    assert_eq!(registry.pending_play_sessions(0, 10).unwrap().len(), 2);
}

#[tokio::test]
async fn flush_keeps_every_row_when_the_server_is_unreachable() {
    // A port that was bound and released refuses connections. (A dropped
    // `MockServer` is not enough: wiremock pools its servers, so the port
    // keeps answering 404.)
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let client = RommClient::new(
        &format!("http://127.0.0.1:{port}"),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap();
    let (_dir, registry) = temp_registry();
    registry.enqueue_play_session(&entry(42, 0)).unwrap();

    let report = flush(registry.clone(), &client).await.unwrap();
    assert_eq!(report.accepted, 0);
    assert_eq!(report.dropped, 0);
    let reason = report
        .stopped
        .expect("an unreachable server stops the flush");
    assert!(
        reason.starts_with("could not reach the server"),
        "a connection error, not an HTTP answer: {reason}"
    );
    assert_eq!(registry.pending_play_sessions(0, 10).unwrap().len(), 1);
}

#[tokio::test]
async fn flush_keeps_rows_on_401() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    registry.enqueue_play_session(&entry(42, 0)).unwrap();

    let report = flush(registry.clone(), &client_for(&server)).await.unwrap();
    assert_eq!(
        report.stopped.as_deref(),
        Some("the server rejected the credentials")
    );
    assert_eq!(registry.pending_play_sessions(0, 10).unwrap().len(), 1);
}

#[tokio::test]
async fn flush_drops_rows_the_server_refuses_for_good() {
    let server = MockServer::start().await;
    // First batch: 422 for the whole request. Nothing else is queued.
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({"detail": "bad"})))
        .expect(1)
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    registry.enqueue_play_session(&entry(42, 0)).unwrap();
    registry.enqueue_play_session(&entry(43, 1)).unwrap();

    let report = flush(registry.clone(), &client_for(&server)).await.unwrap();
    assert_eq!(report.dropped, 2);
    assert_eq!(report.stopped, None);
    assert!(registry.pending_play_sessions(0, 10).unwrap().is_empty());
}

#[tokio::test]
async fn flush_drops_a_per_entry_error_and_keeps_going() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(AnswerAll("error"))
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    registry.enqueue_play_session(&entry(42, 0)).unwrap();

    let report = flush(registry.clone(), &client_for(&server)).await.unwrap();
    assert_eq!(report.dropped, 1);
    assert!(registry.pending_play_sessions(0, 10).unwrap().is_empty());
}

#[tokio::test]
async fn flush_sends_in_batches_until_the_queue_is_empty() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(AnswerAll("created"))
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    let total = FLUSH_BATCH_SIZE + 3;
    for i in 0..total {
        registry
            .enqueue_play_session(&PlaySessionEntry {
                rom_id: 1000 + i as i64,
                ..entry(0, 0)
            })
            .unwrap();
    }

    let report = flush(registry.clone(), &client_for(&server)).await.unwrap();
    assert_eq!(report.accepted, total);
    assert!(registry.pending_play_sessions(0, 10).unwrap().is_empty());

    let requests = server.received_requests().await.unwrap();
    let sizes: Vec<usize> = requests
        .iter()
        .map(|r| {
            let body: Value = serde_json::from_slice(&r.body).unwrap();
            body["sessions"].as_array().unwrap().len()
        })
        .collect();
    assert_eq!(sizes, vec![FLUSH_BATCH_SIZE, 3]);
}

#[tokio::test]
async fn an_empty_queue_sends_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/play-sessions"))
        .respond_with(AnswerAll("created"))
        .expect(0)
        .mount(&server)
        .await;
    let (_dir, registry) = temp_registry();
    let report = flush(registry, &client_for(&server)).await.unwrap();
    assert_eq!(report, Default::default());
}
