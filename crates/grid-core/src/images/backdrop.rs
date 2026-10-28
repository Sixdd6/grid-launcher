//! A game card's letterbox backdrop: one tiny, pre-blurred, pre-darkened JPEG
//! per cover, built once and cached beside it.
//!
//! Square and wide covers are contained in the card's 3:4 frame, and the bars
//! are filled with a blurred, darkened copy of the cover (`GameCard.svelte`).
//! That copy used to be the cover itself under `filter: blur(2.5px)
//! brightness(0.45)` at a quarter of the card's size, scaled up by 4. The blur
//! filter was the whole cost: on a platform of square covers, cold scrolling
//! ran 35ms frames with it, 17ms with the brightness alone and 14ms with no
//! backdrop (2026-10-28, release build, software compositing). This is that
//! same picture, made once in Rust: the cover cropped to fill a
//! [`CARD_BACKDROP_WIDTH`]x[`CARD_BACKDROP_HEIGHT`] box (the quarter-size box
//! the CSS blurred), blurred at [`CARD_BACKDROP_SIGMA`] and darkened to
//! [`CARD_BACKDROP_BRIGHTNESS`]. The webview only scales a few hundred bytes up.
//!
//! Same cache directory and key scheme as `background.rs`: `<key>.cb.jpg`
//! sits beside its source, so the sweep pins and evicts it with the cover.

use super::background::ensure_built_variant;
use super::cache::{ImageCache, ImageError};
use crate::romm::RommClient;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{fast_blur, FilterType};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The extension of a card backdrop: `<key>.cb.jpg`. Not in
/// `LOOKUP_EXTENSIONS`, so `find_existing` never mistakes it for its source.
pub const CARD_BACKDROP_EXT: &str = "cb.jpg";
/// The box the cover is cropped to fill: 3:4, a quarter of a Medium card.
pub const CARD_BACKDROP_WIDTH: u32 = 48;
pub const CARD_BACKDROP_HEIGHT: u32 = 64;
/// The blur the CSS applied at that size.
pub const CARD_BACKDROP_SIGMA: f32 = 2.5;
/// The CSS `brightness()` it applied: every channel is scaled by this.
pub const CARD_BACKDROP_BRIGHTNESS: f32 = 0.45;
const CARD_BACKDROP_JPEG_QUALITY: u8 = 80;

/// Unique temp names within the process; with the pid, across processes.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Decodes `source`, crops it to fill the backdrop box, blurs and darkens it,
/// and writes `<key>.cb.jpg` into `dir` through a `.part` + rename.
///
/// Blocking: the caller runs it on `spawn_blocking`.
pub fn build_card_backdrop(source: &Path, dir: &Path, key: &str) -> Result<PathBuf, ImageError> {
    let io = |e: std::io::Error| ImageError::Io(e.to_string());
    let bytes = std::fs::read(source).map_err(io)?;
    let decoded = image::load_from_memory(&bytes).map_err(|_| ImageError::Decode)?;

    let filled = decoded
        .resize_to_fill(
            CARD_BACKDROP_WIDTH,
            CARD_BACKDROP_HEIGHT,
            FilterType::Triangle,
        )
        .to_rgb8();
    let mut blurred = fast_blur(&filled, CARD_BACKDROP_SIGMA);
    for pixel in blurred.pixels_mut() {
        for channel in pixel.0.iter_mut() {
            *channel = (f32::from(*channel) * CARD_BACKDROP_BRIGHTNESS).round() as u8;
        }
    }

    let mut encoded: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, CARD_BACKDROP_JPEG_QUALITY)
        .encode_image(&blurred)
        .map_err(|_| ImageError::Encode)?;

    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(format!("{key}.{CARD_BACKDROP_EXT}"));
    // `.cb.<pid>-<seq>.part`: never the source's `<key>.part`, never another
    // build's, and still a `.part` the sweep reaps if a process dies mid-write.
    let tmp = dir.join(format!(
        "{key}.cb.{}-{}.part",
        std::process::id(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, &encoded).map_err(io)?;
    std::fs::rename(&tmp, &target).map_err(io)?;
    Ok(target)
}

/// The card backdrop for `url`'s cover, fetching the cover (through the
/// session client on a cache miss) and building the backdrop on first use.
/// Concurrent calls share one build; an undecodable cover is remembered.
pub async fn ensure_card_backdrop(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<PathBuf, ImageError> {
    ensure_built_variant(cache, client, url, CARD_BACKDROP_EXT, build_card_backdrop).await
}
