//! A screenshot's dominant shade, for Server › Discover's generated logos.
//!
//! The webview cannot read the pixels itself: images come from `asset://`,
//! and WebKitGTK taints a canvas that draws one. So the shade is computed
//! here from the cached file: the screenshot's own pixels (never the blurred
//! fill the UI draws around an off-ratio shot) are grouped into
//! 4-bit-per-channel buckets, and the most common bucket's average colour is
//! the shade. Results are kept for the life of the process, keyed by the
//! image's cache key.
//!
//! Token secrecy: the bytes come from `ImageCache::ensure`; nothing here
//! builds a URL or logs one.

use super::cache::{image_key, ImageCache, ImageError};
use crate::romm::RommClient;
use image::imageops::FilterType;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// The side of the box an image is shrunk into before it is sampled: plenty
/// for a dominant colour, and cheap to bucket.
pub const SAMPLE_SIZE: u32 = 96;

/// A pixel at or under this alpha is left out (a transparent margin is not
/// part of the picture).
const MIN_ALPHA: u8 = 128;

/// The dominant shade of `pixels` (RGBA, 4 bytes each): pixels grouped by the
/// top 4 bits of each channel, and the average of the most common group. A
/// tie goes to the group reached first. Pixels at or under [`MIN_ALPHA`] are
/// skipped unless every pixel is; `None` only for an empty slice.
pub fn dominant_shade(pixels: &[u8]) -> Option<[u8; 3]> {
    shade_of(pixels, MIN_ALPHA).or_else(|| shade_of(pixels, 0))
}

fn shade_of(pixels: &[u8], min_alpha: u8) -> Option<[u8; 3]> {
    // count, r, g, b sums per 12-bit bucket.
    let mut buckets: HashMap<u16, [u64; 4]> = HashMap::new();
    let mut best: Option<(u16, u64)> = None;
    for px in pixels.as_chunks::<4>().0 {
        if min_alpha > 0 && px[3] <= min_alpha {
            continue;
        }
        let key =
            (u16::from(px[0] >> 4) << 8) | (u16::from(px[1] >> 4) << 4) | u16::from(px[2] >> 4);
        let b = buckets.entry(key).or_insert([0; 4]);
        b[0] += 1;
        b[1] += u64::from(px[0]);
        b[2] += u64::from(px[1]);
        b[3] += u64::from(px[2]);
        if best.is_none_or(|(_, n)| b[0] > n) {
            best = Some((key, b[0]));
        }
    }
    let (key, _) = best?;
    let b = buckets[&key];
    let avg = |sum: u64| ((sum + b[0] / 2) / b[0]) as u8;
    Some([avg(b[1]), avg(b[2]), avg(b[3])])
}

/// Decodes the image at `path`, shrinks it into a [`SAMPLE_SIZE`] box and
/// returns its [`dominant_shade`]. Blocking: run it on `spawn_blocking`.
pub fn dominant_shade_of_file(path: &Path) -> Result<[u8; 3], ImageError> {
    let bytes = std::fs::read(path).map_err(|e| ImageError::Io(e.to_string()))?;
    let decoded = image::load_from_memory(&bytes).map_err(|_| ImageError::Decode)?;
    let small = if decoded.width() > SAMPLE_SIZE || decoded.height() > SAMPLE_SIZE {
        decoded.resize(SAMPLE_SIZE, SAMPLE_SIZE, FilterType::Triangle)
    } else {
        decoded
    };
    dominant_shade(small.to_rgba8().as_raw()).ok_or(ImageError::Decode)
}

fn shades() -> &'static Mutex<HashMap<String, [u8; 3]>> {
    static SHADES: OnceLock<Mutex<HashMap<String, [u8; 3]>>> = OnceLock::new();
    SHADES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The dominant shade of the image at the resolved, host-filtered `url`:
/// fetched through the cache like any image, computed once and remembered
/// for the life of the process.
pub async fn ensure_dominant_shade(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<[u8; 3], ImageError> {
    let key = image_key(url);
    if let Some(shade) = shades().lock().ok().and_then(|m| m.get(&key).copied()) {
        return Ok(shade);
    }
    let path = cache.ensure(client, url).await?;
    let shade = tokio::task::spawn_blocking(move || dominant_shade_of_file(&path))
        .await
        .map_err(|e| ImageError::Io(e.to_string()))??;
    if let Ok(mut map) = shades().lock() {
        map.insert(key, shade);
    }
    Ok(shade)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8, a: u8, n: usize) -> Vec<u8> {
        [r, g, b, a].repeat(n)
    }

    #[test]
    fn the_most_common_bucket_wins_and_is_averaged() {
        // Three pixels in one bucket (0x1_, 0x2_, 0x3_), two in another.
        let mut pixels = Vec::new();
        pixels.extend(px(16, 32, 48, 255, 1));
        pixels.extend(px(18, 34, 50, 255, 1));
        pixels.extend(px(20, 36, 52, 255, 1));
        pixels.extend(px(200, 10, 10, 255, 2));
        assert_eq!(dominant_shade(&pixels), Some([18, 34, 50]));
    }

    #[test]
    fn transparent_pixels_are_skipped_unless_all_are() {
        let mut pixels = px(250, 250, 250, 0, 10);
        pixels.extend(px(10, 20, 30, 255, 1));
        assert_eq!(dominant_shade(&pixels), Some([10, 20, 30]));
        assert_eq!(
            dominant_shade(&px(250, 250, 250, 0, 3)),
            Some([250, 250, 250])
        );
    }

    #[test]
    fn nothing_to_sample_is_none() {
        assert_eq!(dominant_shade(&[]), None);
    }

    #[test]
    fn a_tie_goes_to_the_bucket_reached_first() {
        let mut pixels = px(0, 0, 0, 255, 2);
        pixels.extend(px(255, 255, 255, 255, 2));
        assert_eq!(dominant_shade(&pixels), Some([0, 0, 0]));
    }

    #[test]
    fn a_file_is_decoded_and_sampled() {
        let dir = std::env::temp_dir().join(format!("grid-shade-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shot.png");
        let mut img = image::RgbaImage::from_pixel(320, 180, image::Rgba([40, 90, 160, 255]));
        for x in 0..60 {
            for y in 0..60 {
                img.put_pixel(x, y, image::Rgba([250, 250, 250, 255]));
            }
        }
        img.save(&path).unwrap();
        let shade = dominant_shade_of_file(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(shade, [40, 90, 160]);
    }
}
