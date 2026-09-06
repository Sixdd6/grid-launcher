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
/// in `config.rs` defaults to the same value).
pub const BACKGROUND_BLUR_DEFAULT: u8 = 12;
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
    let blurred = if sigma == 0 {
        rgb
    } else {
        fast_blur(&rgb, f32::from(sigma))
    };

    let mut encoded: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, BACKGROUND_JPEG_QUALITY)
        .encode_image(&blurred)
        .map_err(|_| ImageError::Encode)?;

    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(format!("{key}.{}", background_variant_ext(sigma)));
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
    remove_stale_variants(dir, key, &target);
    Ok(target)
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
/// fetch: the shell arms two dwell timers (150ms and 500ms) per hovered card,
/// so a cold variant is asked for twice in a row and only one build must run.
/// A [`ImageError::Decode`] is recorded for the session — the source will
/// never decode, and rebuilding it on every hover would burn a full decode
/// attempt each time.
pub async fn ensure_background_variant(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    sigma: u8,
) -> Result<PathBuf, ImageError> {
    let key = image_key(url);
    let ext = background_variant_ext(sigma);
    // Keyed by key AND sigma, not by key alone: two blur levels of one source
    // are two separate files, so one must never wait on — or inherit the
    // failure of — the other.
    let slot = format!("{key}.{ext}");
    loop {
        if let Some(e) = cache.variant_failed().lock().await.get(&slot) {
            return Err(e.clone());
        }
        if let Some(path) = cache.find_with_extension(&key, &ext) {
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

        let result = build_once(cache, client, url, key.clone(), sigma).await;
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
async fn build_once(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
    key: String,
    sigma: u8,
) -> Result<PathBuf, ImageError> {
    let source = cache.ensure(client, url).await?;
    let dir = cache.dir().to_path_buf();
    tokio::task::spawn_blocking(move || build_background_variant(&source, &dir, &key, sigma))
        .await
        .map_err(|e| ImageError::Io(format!("background variant did not finish: {e}")))?
}
