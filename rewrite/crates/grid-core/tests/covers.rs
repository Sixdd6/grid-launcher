use grid_core::covers::{cover_key, CoverCache};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
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
