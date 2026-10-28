use grid_core::images::backdrop::{
    build_card_backdrop, ensure_card_backdrop, CARD_BACKDROP_EXT, CARD_BACKDROP_HEIGHT,
    CARD_BACKDROP_WIDTH,
};
use grid_core::images::background::{background_variant_ext, ensure_background_variant};
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

/// A PNG of `width`x`height` in hard black and white vertical stripes, 4px
/// wide: sharp edges for the blur to soften, and a known mean (127.5).
fn striped_png(width: u32, height: u32) -> Vec<u8> {
    let mut buf = image::RgbImage::new(width, height);
    for (x, _, pixel) in buf.enumerate_pixels_mut() {
        let v = if (x / 4) % 2 == 0 { 0 } else { 255 };
        *pixel = image::Rgb([v, v, v]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    buf.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .expect("the fixture PNG encodes");
    bytes
}

fn write_source(dir: &Path, key: &str, width: u32, height: u32) -> PathBuf {
    let path = dir.join(format!("{key}.png"));
    std::fs::write(&path, striped_png(width, height)).unwrap();
    path
}

#[test]
fn a_square_cover_becomes_a_small_3_4_backdrop_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/cover.png");
    let source = write_source(dir.path(), &key, 512, 512);

    let out = build_card_backdrop(&source, dir.path(), &key).unwrap();

    assert_eq!(out, dir.path().join(format!("{key}.{CARD_BACKDROP_EXT}")));
    let img = image::open(&out).unwrap();
    assert_eq!(
        (img.width(), img.height()),
        (CARD_BACKDROP_WIDTH, CARD_BACKDROP_HEIGHT)
    );
    // A few hundred bytes to a few KB: the point is that it costs nothing.
    assert!(std::fs::metadata(&out).unwrap().len() < 8 * 1024);
}

#[test]
fn the_backdrop_is_blurred_and_darkened() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/stripes.png");
    let source = write_source(dir.path(), &key, 512, 512);

    let out = build_card_backdrop(&source, dir.path(), &key).unwrap();
    let luma = image::open(&out).unwrap().to_luma8();
    let values: Vec<f32> = luma.pixels().map(|p| f32::from(p.0[0])).collect();
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let max = values.iter().cloned().fold(0.0, f32::max);

    // Darkened: the stripes average 127.5; at 0.45 brightness about 57.
    assert!((45.0..70.0).contains(&mean), "mean {mean}");
    // Blurred and darkened: no pixel anywhere near the source's pure white.
    assert!(max < 160.0, "max {max}");
}

#[test]
fn a_body_that_is_not_an_image_reports_decode() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/not-an-image.png");
    let source = dir.path().join(format!("{key}.png"));
    std::fs::write(&source, b"<html>not an image</html>").unwrap();

    let err = build_card_backdrop(&source, dir.path(), &key).unwrap_err();

    assert!(matches!(err, ImageError::Decode));
    assert!(!dir
        .path()
        .join(format!("{key}.{CARD_BACKDROP_EXT}"))
        .exists());
}

#[tokio::test]
async fn the_cover_is_fetched_once_and_the_backdrop_built_once() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/roms/1/cover/small.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(striped_png(256, 256))
                .insert_header("content-type", "image/png"),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/roms/1/cover/small.png", server.uri());

    let first = ensure_card_backdrop(&cache, Some(&client), &url)
        .await
        .unwrap();
    let bytes = std::fs::read(&first).unwrap();
    let second = ensure_card_backdrop(&cache, Some(&client), &url)
        .await
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(std::fs::read(&second).unwrap(), bytes);
    drop(mock); // expect(1) verified on drop: exactly one fetch
}

#[tokio::test]
async fn a_backdrop_and_a_background_of_one_cover_are_separate_files() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/assets/roms/2/cover/small.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(striped_png(256, 256))
                .insert_header("content-type", "image/png"),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let client = client_for(&server);
    let url = format!("{}/assets/roms/2/cover/small.png", server.uri());

    let (backdrop, background) = tokio::join!(
        ensure_card_backdrop(&cache, Some(&client), &url),
        ensure_background_variant(&cache, Some(&client), &url, 2),
    );
    let key = image_key(&url);

    assert_eq!(
        backdrop.unwrap(),
        dir.path().join(format!("{key}.{CARD_BACKDROP_EXT}"))
    );
    assert_eq!(
        background.unwrap(),
        dir.path()
            .join(format!("{key}.{}", background_variant_ext(2)))
    );
}
