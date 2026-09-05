//! The shell's background art variant: one pre-scaled, pre-blurred JPEG per
//! source image, built once and cached beside it.
//!
//! `BackgroundArt.svelte` used to hand the raw large cover (up to 850x1122)
//! to two `filter: blur(40px)` layers, so the compositor blurred ~2.4 Mpx per
//! layer per frame for the whole 360ms cross-fade. Python's TV background
//! blurred ONCE on arrival instead (`_blur_pixmap`,
//! `grid_launcher/tv/widgets/components/fanart_background.py`); this is that,
//! moved into Rust so the webview only ever composites a small still.
//!
//! The variant is fitted inside a [`BACKGROUND_WIDTH`]-square box, so BOTH
//! dimensions are capped: a 1600x900 fanart becomes 960x540 and a 850x1122
//! portrait cover becomes 727x960. Capping the width alone would have left
//! tall covers at their full ~1 Mpx.
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// The variant's extension. Not in `LOOKUP_EXTENSIONS`, so `find_existing`
/// never mistakes a variant for its own source.
pub const BACKGROUND_VARIANT_EXT: &str = "bg.jpg";
/// The side of the box the variant is fitted inside. Wide enough for a 1080p
/// window's background, small enough to blur once and composite for free.
pub const BACKGROUND_WIDTH: u32 = 960;
/// The blur radius. The CSS it replaces was `blur(40px)` on a layer stretched
/// across a ~1920px viewport; at the 960px the variant is built at, the same
/// visual radius is half that. Lower values (12 was tried) leave the cover's
/// title legible through the background.
pub const BACKGROUND_BLUR_SIGMA: f32 = 20.0;
pub const BACKGROUND_JPEG_QUALITY: u8 = 80;

/// Makes each temp file unique within the process, so two builds of the same
/// key can never rename each other's half-written bytes into place. Combined
/// with the pid it is unique across concurrently running launchers too.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Decodes `source`, fits it inside a [`BACKGROUND_WIDTH`] box, blurs it and
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

    // Fit inside the box, never upscale: a small source blurred and blown up
    // is worse than the small source blurred. `resize` preserves the aspect
    // ratio and caps both dimensions.
    let scaled = if decoded.width() > BACKGROUND_WIDTH || decoded.height() > BACKGROUND_WIDTH {
        decoded.resize(BACKGROUND_WIDTH, BACKGROUND_WIDTH, FilterType::Triangle)
    } else {
        decoded
    };

    // RGB8: the background is opaque behind the whole shell, and JPEG has no
    // alpha channel anyway.
    let blurred = fast_blur(&scaled.to_rgb8(), BACKGROUND_BLUR_SIGMA);

    let mut encoded: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, BACKGROUND_JPEG_QUALITY)
        .encode_image(&blurred)
        .map_err(|_| ImageError::Encode)?;

    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(format!("{key}.{BACKGROUND_VARIANT_EXT}"));
    // `.bg.<pid>-<seq>.part`, NOT `.part`: `ImageCache::fetch_and_store` uses
    // `<key>.part` for the source, so a shared name would let a concurrent
    // fetch rename our half-written JPEG over the source. The pid and
    // sequence make it unique per build; the `.part` suffix still lets the
    // sweep reap a stray left by a killed process.
    let tmp = dir.join(format!(
        "{key}.bg.{}-{}.part",
        std::process::id(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, &encoded).map_err(io)?;
    std::fs::rename(&tmp, &target).map_err(io)?;
    Ok(target)
}

/// The local path of `url`'s background variant, building it (and fetching
/// the source through `ImageCache::ensure`, with its dedup, negative cache and
/// download semaphore) on a miss.
///
/// Deduplicated per key the same way `ImageCache::ensure` deduplicates a
/// fetch: the shell arms two dwell timers (150ms and 500ms) per hovered card,
/// so a cold variant is asked for twice in a row and only one build must run.
/// A [`ImageError::Decode`] is recorded for the session — the source will
/// never decode, and rebuilding it on every hover would burn a full decode
/// attempt each time.
pub async fn ensure_background_variant(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<PathBuf, ImageError> {
    let key = image_key(url);
    loop {
        if let Some(e) = cache.variant_failed().lock().await.get(&key) {
            return Err(e.clone());
        }
        if let Some(path) = cache.find_with_extension(&key, BACKGROUND_VARIANT_EXT) {
            return Ok(path);
        }

        let mut map = cache.variant_in_flight().lock().await;
        if let Some(existing) = map.get(&key).cloned() {
            // Register interest while still holding the map lock, exactly as
            // `ImageCache::ensure` does: the owner takes this same lock
            // before notify_waiters(), so no wakeup can be lost.
            let notified = existing.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            drop(map);
            notified.await;
            continue;
        }
        map.insert(key.clone(), Arc::new(Notify::new()));
        drop(map);

        let result = build_once(cache, client, url, key.clone()).await;
        if let Err(e @ ImageError::Decode) = &result {
            cache
                .variant_failed()
                .lock()
                .await
                .insert(key.clone(), e.clone());
        }
        if let Some(n) = cache.variant_in_flight().lock().await.remove(&key) {
            n.notify_waiters();
        }
        return result;
    }
}

/// Fetches the source (or takes the cache hit) and builds the variant off the
/// async runtime. Called with `key` registered in the in-flight map.
async fn build_once(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    key: String,
) -> Result<PathBuf, ImageError> {
    let source = cache.ensure(client, url).await?;
    let dir = cache.dir().to_path_buf();
    tokio::task::spawn_blocking(move || build_background_variant(&source, &dir, &key))
        .await
        .map_err(|e| ImageError::Io(format!("background variant did not finish: {e}")))?
}
