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
//! variant for `<key>` at blur sigma `<sigma>` is `<key>.bg<sigma>.jpg`,
//! which keeps it with its source for the sweep (`sweep::sweep` pins by key
//! PREFIX for exactly this). The sigma is part of the name so each blur
//! level is its own cache entry and a changed setting can never serve a
//! stale blur.
//!
//! One variant per source: a successful build deletes the source's OTHER
//! `<key>.bg<N>.jpg` files, and any `<key>.bg.jpg` from before the sigma was
//! in the name (`remove_stale_variants`). This is not housekeeping the sweep
//! could do instead — the sweep evicts only UNPINNED entries
//! (`sweep::sweep`) and pins by key prefix, so every variant of an installed
//! game's background source is pinned and unreclaimable at any cache size.
//! Without the delete, each sigma the user's slider passes through would
//! mint another permanently pinned JPEG per installed game.
//!
//! What the delete does NOT reach: a source that is never built again keeps
//! the one variant last built for it, at whatever sigma was current then.
//! That residue is bounded at one file per source and is reclaimed only when
//! the source itself leaves the pinned set and the cache is over its cap.
//! Accepted: it is the same file the source would have had anyway.
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

/// The default blur sigma when no setting can be read (`ui.background_blur`
/// in `config.rs` defaults to the same value). 2 keeps the art recognisable
/// with a slight softening — user ruling 2026-09-05.
pub const BACKGROUND_BLUR_DEFAULT: u8 = 2;
/// The strongest blur the Appearance slider offers. The app layer clamps to
/// this before any variant is built.
pub const BACKGROUND_BLUR_MAX: u8 = 40;
/// The side of the box the variant is fitted inside. Wide enough for a 1080p
/// window's background, small enough to blur once and composite for free.
pub const BACKGROUND_WIDTH: u32 = 960;
pub const BACKGROUND_JPEG_QUALITY: u8 = 80;

/// The variant's extension for blur `sigma`: `bg<sigma>.jpg`. Not in
/// `LOOKUP_EXTENSIONS`, so `find_existing` never mistakes a variant for its
/// own source.
pub fn background_variant_ext(sigma: u8) -> String {
    format!("bg{sigma}.jpg")
}

/// Makes each temp file unique within the process, so two builds of the same
/// key can never rename each other's half-written bytes into place. Combined
/// with the pid it is unique across concurrently running launchers too.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Decodes `source`, fits it inside a [`BACKGROUND_WIDTH`] box, blurs it at
/// `sigma` and writes `<key>.bg<sigma>.jpg` into `dir` through a `.part` +
/// rename, so a killed process never leaves a half-written JPEG that a later
/// run would serve. A `sigma` of 0 skips the blur but still downscales.
///
/// Blocking: the caller runs it on `spawn_blocking`.
pub fn build_background_variant(
    source: &Path,
    dir: &Path,
    key: &str,
    sigma: u8,
) -> Result<PathBuf, ImageError> {
    let blurred = blurred_small(source, sigma)?;
    let target = dir.join(format!("{key}.{}", background_variant_ext(sigma)));
    write_jpeg(&blurred, dir, key, &target)?;
    remove_stale_variants(dir, key, &target);
    Ok(target)
}

/// The source decoded, fitted inside the [`BACKGROUND_WIDTH`] box and blurred
/// at `sigma`: the part every variant shares.
fn blurred_small(source: &Path, sigma: u8) -> Result<image::RgbImage, ImageError> {
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
    // alpha channel anyway. Sigma 0 must mean NO blur at all, and skipping is
    // the only way to get that: `fast_blur(_, 0.0)` is not a no-op — its
    // `boxes_for_gauss` still yields radius-1 boxes, so passing 0 through
    // would soften the image.
    let rgb = scaled.to_rgb8();
    Ok(if sigma == 0 {
        rgb
    } else {
        fast_blur(&rgb, f32::from(sigma))
    })
}

/// Encodes `image` and writes it to `target` in `dir` through a `.part` +
/// rename, so a killed process never leaves a half-written JPEG that a later
/// run would serve.
fn write_jpeg(
    image: &image::RgbImage,
    dir: &Path,
    key: &str,
    target: &Path,
) -> Result<(), ImageError> {
    let io = |e: std::io::Error| ImageError::Io(e.to_string());
    let mut encoded: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, BACKGROUND_JPEG_QUALITY)
        .encode_image(image)
        .map_err(|_| ImageError::Encode)?;

    std::fs::create_dir_all(dir).map_err(io)?;
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
    std::fs::rename(&tmp, target).map_err(io)?;
    Ok(())
}

/// The largest window-sized variant built, per side, in px (an 8K screen).
pub const BACKGROUND_FILL_MAX: u32 = 7680;

/// The window-sized variant's extension for blur `sigma` at `width` x
/// `height`: `bg<sigma>.<width>x<height>.jpg`. Never matches
/// [`background_variant_ext`]'s pattern, so the two kinds of variant never
/// delete each other (`remove_stale_variants`, `remove_stale_fills`).
pub fn background_fill_ext(sigma: u8, width: u32, height: u32) -> String {
    format!("bg{sigma}.{width}x{height}.jpg")
}

/// The variant at the window's own size, `width` x `height` device pixels:
/// the blurred [`BACKGROUND_WIDTH`] image (`blurred_small`) enlarged ONCE,
/// smoothly, to cover that box, and cropped to it from the centre.
///
/// The shell then draws it 1:1, unscaled. Scaling the 960px variant up to the
/// window in CSS cost every scrolled frame a full-window resample when the
/// webview composites on the CPU: at 3840x2160 it was the largest single cost
/// of a frame (perf probe A/B, 2026-10-30: 106ms against 63ms unscaled).
/// The blur still runs at 960px, so building it costs one resize and one
/// encode more than the small variant.
///
/// One per source: a build deletes the source's other window-sized variants
/// (other sizes or sigmas), but never the small variant the background
/// replenish pass keeps (`replenish.rs`).
///
/// Blocking: the caller runs it on `spawn_blocking`.
pub fn build_background_fill(
    source: &Path,
    dir: &Path,
    key: &str,
    sigma: u8,
    width: u32,
    height: u32,
) -> Result<PathBuf, ImageError> {
    let width = width.clamp(1, BACKGROUND_FILL_MAX);
    let height = height.clamp(1, BACKGROUND_FILL_MAX);
    let small = blurred_small(source, sigma)?;
    let (sw, sh) = (small.width().max(1), small.height().max(1));
    let scale = (f64::from(width) / f64::from(sw)).max(f64::from(height) / f64::from(sh));
    let cover_w = ((f64::from(sw) * scale).ceil() as u32).max(width);
    let cover_h = ((f64::from(sh) * scale).ceil() as u32).max(height);
    let covered = image::imageops::resize(&small, cover_w, cover_h, FilterType::Triangle);
    let x = (cover_w - width) / 2;
    let y = (cover_h - height) / 2;
    let filled = image::imageops::crop_imm(&covered, x, y, width, height).to_image();
    let target = dir.join(format!(
        "{key}.{}",
        background_fill_ext(sigma, width, height)
    ));
    write_jpeg(&filled, dir, key, &target)?;
    remove_stale_fills(dir, key, &target);
    Ok(target)
}

/// Deletes every window-sized variant of `key` in `dir` except `keep`
/// (`<key>.bg<digits>.<digits>x<digits>.jpg`). Best effort, like
/// [`remove_stale_variants`].
fn remove_stale_fills(dir: &Path, key: &str, keep: &Path) {
    let prefix = format!("{key}.bg");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let Some(rest) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix(&prefix))
            .and_then(|rest| rest.strip_suffix(".jpg"))
        else {
            continue;
        };
        let is_fill = rest.split_once('.').is_some_and(|(sigma, size)| {
            !sigma.is_empty()
                && sigma.bytes().all(|b| b.is_ascii_digit())
                && size.split_once('x').is_some_and(|(w, h)| {
                    !w.is_empty()
                        && !h.is_empty()
                        && w.bytes().all(|b| b.is_ascii_digit())
                        && h.bytes().all(|b| b.is_ascii_digit())
                })
        });
        if is_fill {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Deletes every variant of `key` in `dir` except `keep`: the other sigmas,
/// and the legacy extensionless-sigma `<key>.bg.jpg` written before this
/// scheme existed. Best effort — a variant that will not delete is a wasted
/// file, never a failed build.
///
/// Matches `<key>.bg<digits>.jpg` and `<key>.bg.jpg` only, so a concurrent
/// build's `<key>.bg.<pid>-<seq>.part` is never touched.
///
/// Two sigmas of one source CAN be in flight at once: the shell asks for one
/// sigma at a time, but `replenish_once` reads its own sigma from
/// `config.toml` and holds it for the whole pass, so a blur committed
/// mid-pass puts the pass's sigma and the shell's sigma against the same
/// source and each deletes the other's output. The names are deterministic,
/// so the loser is rebuilt on its next miss. The frontend must not memoise
/// across a sigma change or that rebuild never happens — see
/// `clearVariantMemo` in `app/src/lib/backgroundPrefetch.ts`.
fn remove_stale_variants(dir: &Path, key: &str, keep: &Path) {
    let prefix = format!("{key}.bg");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let Some(sigma) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix(&prefix))
            .and_then(|rest| rest.strip_suffix(".jpg"))
        else {
            continue;
        };
        // "" is the legacy `<key>.bg.jpg`; digits are this scheme's sigmas.
        if sigma.is_empty() || sigma.bytes().all(|b| b.is_ascii_digit()) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The local path of `url`'s background variant, building it (and fetching
/// the source through `ImageCache::ensure`, with its dedup, negative cache and
/// download semaphore) on a miss.
///
/// Deduplicated per key the same way `ImageCache::ensure` deduplicates a
/// fetch: the shell asks once when the pointer enters a card and again at the
/// 120ms swap, so a cold variant is asked for twice in a row and only one
/// build must run.
/// A [`ImageError::Decode`] is recorded for the session — the source will
/// never decode, and rebuilding it on every hover would burn a full decode
/// attempt each time.
pub async fn ensure_background_variant(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    sigma: u8,
) -> Result<PathBuf, ImageError> {
    ensure_built_variant(
        cache,
        client,
        url,
        &background_variant_ext(sigma),
        move |source, dir, key| build_background_variant(source, dir, key, sigma),
    )
    .await
}

/// [`ensure_background_variant`] for the window-sized variant
/// ([`build_background_fill`]), with the same dedup and failure memory.
pub async fn ensure_background_fill(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    sigma: u8,
    width: u32,
    height: u32,
) -> Result<PathBuf, ImageError> {
    let width = width.clamp(1, BACKGROUND_FILL_MAX);
    let height = height.clamp(1, BACKGROUND_FILL_MAX);
    ensure_built_variant(
        cache,
        client,
        url,
        &background_fill_ext(sigma, width, height),
        move |source, dir, key| build_background_fill(source, dir, key, sigma, width, height),
    )
    .await
}

/// The file `<key>.<ext>` for `url`'s source, built by `build` (blocking) on
/// first use and shared by every caller after: concurrent requests for the
/// same file wait on one build, and a source that fails to decode is
/// remembered so it is not fetched and decoded again on every request.
/// Keyed by key AND extension, so two variants of one source (two blur
/// levels, a background and a card backdrop) never wait on — or inherit the
/// failure of — each other.
pub(crate) async fn ensure_built_variant<F>(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    ext: &str,
    build: F,
) -> Result<PathBuf, ImageError>
where
    F: Fn(&Path, &Path, &str) -> Result<PathBuf, ImageError> + Send + Copy + 'static,
{
    let key = image_key(url);
    let slot = format!("{key}.{ext}");
    loop {
        if let Some(e) = cache.variant_failed().lock().await.get(&slot) {
            return Err(e.clone());
        }
        if let Some(path) = cache.find_with_extension(&key, ext) {
            return Ok(path);
        }

        let mut map = cache.variant_in_flight().lock().await;
        if let Some(existing) = map.get(&slot).cloned() {
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
        map.insert(slot.clone(), Arc::new(Notify::new()));
        drop(map);

        let result = build_once(cache, client, url, key.clone(), build).await;
        if let Err(e @ ImageError::Decode) = &result {
            cache
                .variant_failed()
                .lock()
                .await
                .insert(slot.clone(), e.clone());
        }
        if let Some(n) = cache.variant_in_flight().lock().await.remove(&slot) {
            n.notify_waiters();
        }
        return result;
    }
}

/// Fetches the source (or takes the cache hit) and builds the variant off the
/// async runtime. Called with `<key>.<ext>` registered in the in-flight map.
async fn build_once<F>(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    key: String,
    build: F,
) -> Result<PathBuf, ImageError>
where
    F: Fn(&Path, &Path, &str) -> Result<PathBuf, ImageError> + Send + Copy + 'static,
{
    let source = cache.ensure(client, url).await?;
    let dir = cache.dir().to_path_buf();
    tokio::task::spawn_blocking(move || build(&source, &dir, &key))
        .await
        .map_err(|e| ImageError::Io(format!("image variant did not finish: {e}")))?
}
