use grid_core::images::cache::{image_key, ImageCache, ImageError};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

#[test]
fn image_key_is_stable_sha256() {
    assert_eq!(image_key("https://h/a.png"), image_key("https://h/a.png"));
    assert_eq!(image_key("https://h/a.png").len(), 64);
    assert_ne!(image_key("https://h/a.png"), image_key("https://h/b.png"));
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
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/cover.png", server.uri());

    let first = cache.ensure(Some(&client), &url).await.unwrap();
    let second = cache.ensure(Some(&client), &url).await.unwrap();
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
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/missing.png", server.uri());

    let first = cache.ensure(Some(&client), &url).await;
    let second = cache.ensure(Some(&client), &url).await;
    assert!(first.is_err());
    assert!(second.is_err());
    assert_eq!(
        first.unwrap_err().to_string(),
        second.unwrap_err().to_string()
    );
    drop(mock); // expect(1) verified on drop: the second call never left the process
}

/// Regression test for a lost-wakeup hang: 8 callers race for the same URL
/// while the server response is delayed, so at least one waiter's
/// `.notified()` call and the owner's `notify_waiters()` are genuinely
/// concurrent (real OS-thread parallelism via the multi-thread runtime, not
/// just cooperative interleaving). The whole join is wrapped in a timeout so
/// a reintroduced lost-wakeup bug fails the test instead of hanging CI.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn ensure_dedups_concurrent_callers_for_same_url() {
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
    let cache = Arc::new(ImageCache::new(dir.path().to_path_buf()));
    let client = Arc::new(client_for(&server));
    let url = format!("{}/assets/cover.png", server.uri());

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let cache = cache.clone();
            let client = client.clone();
            let url = url.clone();
            tokio::spawn(async move { cache.ensure(Some(&client), &url).await })
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

#[tokio::test]
async fn offline_miss_is_not_recorded_as_failure() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/c.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let url = format!("{}/assets/c.png", server.uri());
    let offline = cache.ensure(None, &url).await;
    assert!(matches!(offline, Err(ImageError::Offline)));
    let client = client_for(&server);
    let got = cache.ensure(Some(&client), &url).await.unwrap();
    assert!(got.exists());
    assert_eq!(cache.ensure(None, &url).await.unwrap(), got); // offline hit
    drop(mock);
}

#[tokio::test]
async fn content_gate_rejects_non_images_and_writes_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/assets/login"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html>login</html>")
                .insert_header("content-type", "text/html"),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let err = cache
        .ensure(Some(&client), &format!("{}/assets/login", server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, ImageError::NotAnImage));
    assert_eq!(
        std::fs::read_dir(dir.path())
            .map(|d| d.count())
            .unwrap_or(0),
        0
    );
}

#[tokio::test]
async fn image_content_type_alone_is_accepted() {
    // An `image/*` body the sniffers don't recognize is still written (suffix rule picks the ext).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/assets/x.avif"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"not-a-known-magic".to_vec())
                .insert_header("content-type", "image/avif"),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let p = cache
        .ensure(Some(&client), &format!("{}/assets/x.avif", server.uri()))
        .await
        .unwrap();
    assert_eq!(p.extension().unwrap(), "avif");
}

#[tokio::test]
async fn cache_hit_refreshes_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let url = "https://h/assets/old.png";
    let file = dir.path().join(format!("{}.png", image_key(url)));
    std::fs::write(&file, PNG_MAGIC).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(old)
        .unwrap();
    cache.ensure(None, url).await.unwrap();
    let mtime = std::fs::metadata(&file).unwrap().modified().unwrap();
    assert!(mtime > old + std::time::Duration::from_secs(1800));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn downloads_are_limited_to_six_in_flight() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(PNG_MAGIC)
                .set_delay(Duration::from_millis(300)),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = Arc::new(ImageCache::new(dir.path().to_path_buf()));
    let client = Arc::new(client_for(&server));
    let started = std::time::Instant::now();
    let tasks: Vec<_> = (0..12)
        .map(|i| {
            let cache = cache.clone();
            let client = client.clone();
            let base = server.uri();
            tokio::spawn(async move {
                cache
                    .ensure(Some(&client), &format!("{base}/assets/{i}.png"))
                    .await
            })
        })
        .collect();
    for t in futures::future::join_all(tasks).await {
        t.unwrap().unwrap();
    }
    // 12 fetches at 300 ms each through 6 permits: two waves, so >= ~600 ms.
    assert!(
        started.elapsed() >= Duration::from_millis(550),
        "elapsed {:?}",
        started.elapsed()
    );
}
