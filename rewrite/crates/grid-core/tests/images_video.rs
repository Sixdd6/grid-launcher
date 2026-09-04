use grid_core::images::cache::{ImageCache, ImageError};
use grid_core::images::video::{ensure_video, video_extension_for};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// An ISO base media file: 4 size bytes, then the `ftyp` box type.
const MP4_MAGIC: &[u8] = &[
    0, 0, 0, 0x18, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm',
];
const WEBM_MAGIC: &[u8] = &[0x1A, 0x45, 0xDF, 0xA3, 0x01, 0x00, 0x00, 0x00];

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

#[test]
fn video_extension_reads_the_content_type_first() {
    assert_eq!(
        video_extension_for("http://h/clip", MP4_MAGIC, "video/mp4"),
        Some("mp4".to_string())
    );
    assert_eq!(
        video_extension_for("http://h/clip", WEBM_MAGIC, "video/webm; charset=binary"),
        Some("webm".to_string())
    );
}

#[test]
fn video_extension_falls_back_to_the_magic_bytes() {
    assert_eq!(
        video_extension_for("http://h/clip", MP4_MAGIC, "application/octet-stream"),
        Some("mp4".to_string())
    );
    assert_eq!(
        video_extension_for("http://h/clip", WEBM_MAGIC, ""),
        Some("webm".to_string())
    );
}

#[test]
fn video_extension_rejects_anything_that_is_not_a_video() {
    // An HTML error page served with a 200 is the failure mode that matters:
    // without the gate it would be cached and handed to a <video> element.
    assert_eq!(
        video_extension_for("http://h/clip.mp4", b"<!doctype html>", "text/html"),
        None
    );
    assert_eq!(
        video_extension_for("http://h/clip.mp4", b"", "video/mp4"),
        None
    );
}

#[tokio::test]
async fn ensure_video_fetches_once_then_hits_the_cache() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/romm/resources/roms/1/video.mp4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(MP4_MAGIC)
                .insert_header("content-type", "video/mp4"),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/romm/resources/roms/1/video.mp4", server.uri());

    let first = ensure_video(&cache, Some(&client), &url).await.unwrap();
    let second = ensure_video(&cache, Some(&client), &url).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first.extension().unwrap(), "mp4");
    assert!(first.starts_with(dir.path()));
    drop(mock);
}

#[tokio::test]
async fn ensure_video_with_no_client_is_offline_rather_than_a_bare_url() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    match ensure_video(&cache, None, "http://server/video.mp4").await {
        Err(ImageError::Offline) => {}
        other => panic!("expected Offline, got {other:?}"),
    }
}

#[tokio::test]
async fn ensure_video_refuses_a_non_video_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/assets/not-a-video.mp4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"<!doctype html>".as_slice())
                .insert_header("content-type", "text/html"),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/not-a-video.mp4", server.uri());
    match ensure_video(&cache, Some(&client), &url).await {
        Err(ImageError::NotAnImage) => {}
        other => panic!("expected NotAnImage, got {other:?}"),
    }
}
