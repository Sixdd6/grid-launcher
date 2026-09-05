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

/// Every `.part` file left in `dir`. The temp name carries a pid and a
/// sequence, so it cannot be predicted by name.
fn part_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "part"))
        .count()
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
    // Both dimensions are inside the box, not just the width.
    assert!(decoded.width().max(decoded.height()) <= BACKGROUND_WIDTH);
    // No temp file is left behind.
    assert_eq!(part_files(dir.path()), 0);
}

/// A tall cover is capped on its HEIGHT: capping the width alone would have
/// left an 850x1122 cover at its full ~1 Mpx, which is what the CSS blur was
/// paying for in the first place.
#[test]
fn a_portrait_source_is_capped_on_its_long_edge() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://romm.example/portrait.png");
    let source = dir.path().join(format!("{key}.png"));
    std::fs::write(&source, gradient_png(850, 1122)).unwrap();

    let out = build_background_variant(&source, dir.path(), &key).unwrap();

    let decoded = image::open(&out).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (727, 960));
    assert!(decoded.width().max(decoded.height()) <= BACKGROUND_WIDTH);
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
    assert!(decoded.width().max(decoded.height()) <= BACKGROUND_WIDTH);
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

    // Deleting the source proves the second call cannot have rebuilt: a
    // rebuild would need the source and would fail with `Offline`.
    std::fs::remove_file(dir.path().join(format!("{key}.png"))).unwrap();

    let second = ensure_background_variant(&cache, None, url).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(std::fs::read(&second).unwrap(), bytes);
}

/// The shell arms two dwell timers per hovered card (150ms and 500ms), so a
/// cold variant is asked for twice at once. Both calls must return the same
/// COMPLETE file — a shared temp name would let one rename a truncated JPEG
/// over the other's.
#[tokio::test]
async fn two_concurrent_calls_for_a_cold_variant_both_get_a_complete_file() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/roms/2/fanart.png"))
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
    let url = format!("{}/assets/roms/2/fanart.png", server.uri());

    let (a, b) = tokio::join!(
        ensure_background_variant(&cache, Some(&client), &url),
        ensure_background_variant(&cache, Some(&client), &url),
    );

    let a = a.unwrap();
    assert_eq!(a, b.unwrap());
    // Decoding it back is the completeness check: a truncated JPEG fails here.
    let decoded = image::open(&a).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (960, 540));
    assert_eq!(part_files(dir.path()), 0);
    drop(mock); // expect(1): the source was fetched once, not twice
}

/// A source this build cannot decode is remembered for the session, so a card
/// hovered again does not pay for another decode attempt. The second call
/// must not even read the source — deleting it proves the answer is cached.
#[tokio::test]
async fn an_undecodable_source_is_negatively_cached_for_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let url = "https://romm.example/vector.svg";
    let key = image_key(url);
    let source = dir.path().join(format!("{key}.svg"));
    std::fs::write(&source, b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();

    assert!(matches!(
        ensure_background_variant(&cache, None, url).await,
        Err(ImageError::Decode)
    ));
    std::fs::remove_file(&source).unwrap();
    assert!(matches!(
        ensure_background_variant(&cache, None, url).await,
        Err(ImageError::Decode)
    ));
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
