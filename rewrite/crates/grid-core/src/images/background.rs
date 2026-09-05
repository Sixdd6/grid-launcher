//! The shell's background art variant: one pre-scaled, pre-blurred JPEG per
//! source image, built once and cached beside it.
//!
//! `BackgroundArt.svelte` used to hand the raw large cover (up to 850x1122)
//! to two `filter: blur(40px)` layers, so the compositor blurred ~2.4 Mpx per
//! layer per frame for the whole 360ms cross-fade. Python's TV background
//! blurred ONCE on arrival instead (`_blur_pixmap`,
//! `grid_launcher/tv/widgets/components/fanart_background.py`); this is that,
//! moved into Rust so the webview only ever composites a ~0.3 Mpx still.
//!
//! Shares the image cache's directory and key scheme, like `video.rs`: the
//! variant for `<key>` is `<key>.bg.jpg`, which keeps it with its source for
//! the sweep (`sweep::sweep` pins by key PREFIX for exactly this).
//!
//! Token secrecy: the source bytes come from `ImageCache::ensure` (the
//! session's `RommClient`); nothing here builds a URL or logs one.

use super::cache::{image_key, ImageCache, ImageError};
use crate::romm::RommClient;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{fast_blur, FilterType};
use std::path::{Path, PathBuf};

/// The variant's extension. Not in `LOOKUP_EXTENSIONS`, so `find_existing`
/// never mistakes a variant for its own source.
pub const BACKGROUND_VARIANT_EXT: &str = "bg.jpg";
/// Wide enough for a 1080p window's background, small enough to blur once and
/// composite for free.
pub const BACKGROUND_WIDTH: u32 = 960;
/// The blur radius, chosen so a 960px-wide variant looks like the 40px CSS
/// blur did on a full-resolution cover.
pub const BACKGROUND_BLUR_SIGMA: f32 = 12.0;
pub const BACKGROUND_JPEG_QUALITY: u8 = 80;

/// Decodes `source`, scales it to at most [`BACKGROUND_WIDTH`], blurs it and
/// writes `<key>.bg.jpg` into `dir` through a `.part` + rename, so a killed
/// process never leaves a half-written JPEG that a later run would serve.
///
/// Blocking: the caller runs it on `spawn_blocking`.
pub fn build_background_variant(
    source: &Path,
    dir: &Path,
    key: &str,
) -> Result<PathBuf, ImageError> {
    let io = |e: std::io::Error| ImageError::Io(e.to_string());
    let bytes = std::fs::read(source).map_err(io)?;
    let decoded = image::load_from_memory(&bytes).map_err(|_| ImageError::Decode)?;

    // Never upscale: a small source blurred and blown up is worse than the
    // small source blurred.
    let scaled = if decoded.width() > BACKGROUND_WIDTH {
        let height = ((decoded.height() as u64 * BACKGROUND_WIDTH as u64)
            / decoded.width().max(1) as u64)
            .max(1) as u32;
        decoded.resize_exact(BACKGROUND_WIDTH, height, FilterType::Triangle)
    } else {
        decoded
    };

    // RGB8: the background is opaque behind the whole shell, and JPEG has no
    // alpha channel anyway.
    let blurred = fast_blur(&scaled.to_rgb8(), BACKGROUND_BLUR_SIGMA);

    let mut encoded: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, BACKGROUND_JPEG_QUALITY)
        .encode_image(&blurred)
        .map_err(|_| ImageError::Decode)?;

    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(format!("{key}.{BACKGROUND_VARIANT_EXT}"));
    // `.bg.part`, NOT `.part`: `ImageCache::fetch_and_store` uses `<key>.part`
    // for the source, and a concurrent fetch of the same image would
    // otherwise rename our half-written JPEG over the source.
    let tmp = dir.join(format!("{key}.bg.part"));
    std::fs::write(&tmp, &encoded).map_err(io)?;
    std::fs::rename(&tmp, &target).map_err(io)?;
    Ok(target)
}

/// The local path of `url`'s background variant, building it (and fetching
/// the source through `ImageCache::ensure`, with its dedup, negative cache and
/// download semaphore) on a miss.
pub async fn ensure_background_variant(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<PathBuf, ImageError> {
    let key = image_key(url);
    if let Some(path) = cache.find_with_extension(&key, BACKGROUND_VARIANT_EXT) {
        return Ok(path);
    }
    let source = cache.ensure(client, url).await?;
    let dir = cache.dir().to_path_buf();
    tokio::task::spawn_blocking(move || build_background_variant(&source, &dir, &key))
        .await
        .map_err(|e| ImageError::Io(format!("background variant did not finish: {e}")))?
}
