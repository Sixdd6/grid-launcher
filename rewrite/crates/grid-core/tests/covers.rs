use grid_core::covers::{cover_key, CoverCache};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

#[test]
fn cover_key_is_stable_sha256() {
    assert_eq!(cover_key(42), cover_key(42));
    assert_eq!(cover_key(42).len(), 64);
    assert_ne!(cover_key(42), cover_key(43));
}

#[tokio::test]
async fn ensure_fetches_once_then_hits_cache() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/cover.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = CoverCache::new(dir.path().to_path_buf());
    let client = RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap();

    let first = cache
        .ensure(&client, 42, "/assets/cover.png")
        .await
        .unwrap();
    let second = cache
        .ensure(&client, 42, "/assets/cover.png")
        .await
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.extension().unwrap(), "png");
    assert!(first.starts_with(dir.path()));
    drop(mock); // expect(1) verified on drop: second call hit the disk cache
}

/// A miss that fails stays failed for the session: the second ensure() must
/// replay the recorded error instead of hitting the server again.
#[tokio::test]
async fn ensure_caches_failures_for_the_session() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/missing.png"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = CoverCache::new(dir.path().to_path_buf());
    let client = RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap();

    let first = cache.ensure(&client, 42, "/assets/missing.png").await;
    let second = cache.ensure(&client, 42, "/assets/missing.png").await;
    assert!(first.is_err());
    assert!(second.is_err());
    assert_eq!(
        first.unwrap_err().to_string(),
        second.unwrap_err().to_string()
    );
    drop(mock); // expect(1) verified on drop: the second call never left the process
}

/// Regression test for a lost-wakeup hang: 8 callers race for the same
/// game_id while the server response is delayed, so at least one waiter's
/// `.notified()` call and the owner's `notify_waiters()` are genuinely
/// concurrent (real OS-thread parallelism via the multi-thread runtime, not
/// just cooperative interleaving). The whole join is wrapped in a timeout so
/// a reintroduced lost-wakeup bug fails the test instead of hanging CI.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn ensure_dedups_concurrent_callers_for_same_game() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/cover.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(PNG_MAGIC)
                .set_delay(Duration::from_millis(200)),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Arc::new(CoverCache::new(dir.path().to_path_buf()));
    let client = Arc::new(
        RommClient::new(
            &server.uri(),
            Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
        )
        .unwrap(),
    );

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let cache = cache.clone();
            let client = client.clone();
            tokio::spawn(async move { cache.ensure(&client, 42, "/assets/cover.png").await })
        })
        .collect();

    let results = tokio::time::timeout(Duration::from_secs(5), futures::future::join_all(tasks))
        .await
        .expect("ensure() calls hung: likely the dedup lost-wakeup bug");

    let paths: Vec<_> = results
        .into_iter()
        .map(|joined| joined.unwrap().unwrap())
        .collect();
    let first = &paths[0];
    assert!(paths.iter().all(|p| p == first));
    drop(mock); // expect(1) verified on drop: exactly one download for 8 concurrent callers
}
