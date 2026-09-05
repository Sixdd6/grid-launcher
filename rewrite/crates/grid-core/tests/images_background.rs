use grid_core::images::background::{
    build_background_variant, ensure_background_variant, BACKGROUND_VARIANT_EXT, BACKGROUND_WIDTH,
};
use grid_core::images::cache::{image_key, ImageCache, ImageError};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use std::path::{Path, PathBuf};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

/// A gradient PNG of `width`x`height`. A gradient rather than a solid fill so
/// the blur has something to actually average.
fn gradient_png(width: u32, height: u32) -> Vec<u8> {
    let mut buf = image::RgbImage::new(width, height);
    for (x, y, pixel) in buf.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    buf.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .expect("the fixture PNG encodes");
    bytes
}

/// Writes a 1200x800 PNG, wider than `BACKGROUND_WIDTH`, so the resize branch
/// actually runs.
fn write_source(dir: &Path, key: &str) -> PathBuf {
    let path = dir.join(format!("{key}.png"));
    std::fs::write(&path, gradient_png(1200, 800)).unwrap();
    path
}

#[test]
fn the_variant_is_written_beside_the_source_and_is_960_wide() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/cover.png");
    let source = write_source(dir.path(), &key);

    let out = build_background_variant(&source, dir.path(), &key).unwrap();

    assert_eq!(
        out,
        dir.path().join(format!("{key}.{BACKGROUND_VARIANT_EXT}"))
    );
    assert!(out.is_file());
    let decoded = image::open(&out).unwrap();
    assert_eq!(decoded.width(), BACKGROUND_WIDTH);
    // 1200x800 scaled to 960 wide keeps its 3:2 ratio.
    assert_eq!(decoded.height(), 640);
    // No `.part` file is left behind.
    assert!(!dir.path().join(format!("{key}.bg.part")).exists());
}

#[test]
fn a_source_narrower_than_the_target_is_not_upscaled() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/small.png");
    let source = dir.path().join(format!("{key}.png"));
    std::fs::write(&source, gradient_png(320, 240)).unwrap();

    let out = build_background_variant(&source, dir.path(), &key).unwrap();
    let decoded = image::open(&out).unwrap();
    assert_eq!(decoded.width(), 320);
    assert_eq!(decoded.height(), 240);
}

#[test]
fn a_body_that_is_not_an_image_reports_decode_rather_than_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/broken.png");
    let source = dir.path().join(format!("{key}.png"));
    std::fs::write(&source, b"<html>not an image</html>").unwrap();
    assert!(matches!(
        build_background_variant(&source, dir.path(), &key),
        Err(ImageError::Decode)
    ));
}

/// The whole path from a cold cache: one GET for the source, then the
/// variant. A second call must serve the variant off disk without touching
/// the network — `expect(1)` on the mock proves it.
#[tokio::test]
async fn a_cold_url_is_fetched_once_and_the_second_call_is_a_cache_hit() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/roms/1/fanart.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(gradient_png(1600, 900))
                .insert_header("content-type", "image/png"),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/roms/1/fanart.png", server.uri());

    let first = ensure_background_variant(&cache, Some(&client), &url)
        .await
        .unwrap();
    // Compared instead of the mtime: `find_with_extension` refreshes the
    // mtime on a hit, so only the content proves nothing was rebuilt.
    let bytes = std::fs::read(&first).unwrap();

    let second = ensure_background_variant(&cache, Some(&client), &url)
        .await
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(
        first,
        dir.path()
            .join(format!("{}.{BACKGROUND_VARIANT_EXT}", image_key(&url)))
    );
    assert_eq!(std::fs::read(&second).unwrap(), bytes);
    let decoded = image::open(&second).unwrap();
    assert_eq!(decoded.width(), BACKGROUND_WIDTH);
    assert!(decoded.width() <= BACKGROUND_WIDTH);
    drop(mock); // expect(1) verified on drop: exactly one fetch
}

#[tokio::test]
async fn a_second_call_is_a_cache_hit_and_does_not_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let url = "https://romm.example/cover.png";
    let key = image_key(url);
    write_source(dir.path(), &key);

    // No client: `ImageCache::ensure` finds the source already cached, so
    // the whole path runs offline.
    let first = ensure_background_variant(&cache, None, url).await.unwrap();
    let bytes = std::fs::read(&first).unwrap();
    let second = ensure_background_variant(&cache, None, url).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(std::fs::read(&second).unwrap(), bytes);
}

#[tokio::test]
async fn a_cold_url_with_no_client_is_offline() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    match ensure_background_variant(&cache, None, "https://romm.example/cold.png").await {
        Err(ImageError::Offline) => {}
        other => panic!("expected Offline, got {other:?}"),
    }
}
