//! Streamed multi-target download with cumulative progress and cooperative
//! cancellation. See `docs/porting/03-library-install.md` invariant 5: a
//! target already fully downloaded on disk is skipped without a request,
//! which is what makes retrying after a failed finalize step cheap.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;

use super::LibraryError;
use crate::romm::{RommClient, RommError};

/// One file to fetch from the server into a destination path.
#[derive(Debug, Clone)]
pub struct FileTarget {
    pub url_path: String,
    pub query: Vec<(String, String)>,
    pub dest: PathBuf,
    /// Server-reported size in bytes, `0` when unknown.
    pub expected_size: i64,
}

/// Minimum gap between progress emissions, other than the mandatory final
/// one after the last target completes.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Downloads every target in order. Progress is cumulative across targets:
/// `(downloaded, total, avg_speed_bps)` at most every 100 ms plus a final
/// emit. `total` = sum of `expected_size` when every target's is known
/// (`> 0`); otherwise, with exactly one target, the response's
/// `Content-Length` (`0` when absent); otherwise `0`. Checks `cancel` before
/// writing each received chunk; on cancellation or any HTTP/IO error the
/// CURRENT target's partial file is deleted (removal errors ignored) and
/// `Cancelled` / the error is returned — targets completed earlier are left
/// in place. A target whose `dest` already exists with size ==
/// `expected_size` (> 0) is skipped without an HTTP request; its bytes
/// count toward cumulative progress immediately.
pub async fn download_targets(
    client: &RommClient,
    targets: &[FileTarget],
    cancel: &AtomicBool,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
) -> Result<(), LibraryError> {
    let start = Instant::now();
    let mut last_emit: Option<Instant> = None;
    let mut cumulative: u64 = 0;

    let all_known = !targets.is_empty() && targets.iter().all(|t| t.expected_size > 0);
    let mut total: u64 = if all_known {
        targets.iter().map(|t| t.expected_size as u64).sum()
    } else {
        0
    };
    // Only relevant when there's a single target whose size isn't known
    // upfront: that target can never be skip-eligible (skip requires
    // expected_size > 0), so it always reaches the request below, where its
    // Content-Length becomes the total.
    let single_unknown = !all_known && targets.len() == 1;

    for target in targets {
        if let Some(size) = existing_matching_size(target) {
            cumulative += size;
            continue;
        }

        if let Err(err) = download_one_target(
            client,
            target,
            cancel,
            &mut total,
            single_unknown,
            &mut cumulative,
            start,
            &mut last_emit,
            on_progress,
        )
        .await
        {
            let _ = fs::remove_file(&target.dest);
            return Err(err);
        }
    }

    maybe_emit(&mut last_emit, start, cumulative, total, on_progress, true);
    Ok(())
}

/// Downloads a single not-yet-skipped target, writing chunks as they
/// arrive. Cleanup of a partial file on error is the caller's
/// responsibility (`download_targets` does it once, uniformly, for every
/// failure path here).
#[allow(clippy::too_many_arguments)]
async fn download_one_target(
    client: &RommClient,
    target: &FileTarget,
    cancel: &AtomicBool,
    total: &mut u64,
    single_unknown: bool,
    cumulative: &mut u64,
    start: Instant,
    last_emit: &mut Option<Instant>,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
) -> Result<(), LibraryError> {
    if let Some(parent) = target.dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let query: Vec<(&str, String)> = target
        .query
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();
    let resp = client.get_response(&target.url_path, &query).await?;
    if single_unknown {
        *total = resp.content_length().unwrap_or(0);
    }

    let mut file = fs::File::create(&target.dest)?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        if cancel.load(Ordering::Relaxed) {
            return Err(LibraryError::Cancelled);
        }
        file.write_all(&bytes)?;
        *cumulative += bytes.len() as u64;
        maybe_emit(last_emit, start, *cumulative, *total, on_progress, false);
    }

    Ok(())
}

/// Calls `on_progress` when `force` is set, or when at least
/// [`PROGRESS_INTERVAL`] has elapsed since the last emission (always true
/// for the very first call).
fn maybe_emit(
    last_emit: &mut Option<Instant>,
    start: Instant,
    cumulative: u64,
    total: u64,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
    force: bool,
) {
    let now = Instant::now();
    let due = force
        || last_emit
            .map(|t| now.duration_since(t) >= PROGRESS_INTERVAL)
            .unwrap_or(true);
    if !due {
        return;
    }
    *last_emit = Some(now);
    let elapsed = start.elapsed().as_secs_f64();
    let speed = if elapsed > 0.0 {
        cumulative as f64 / elapsed
    } else {
        0.0
    };
    on_progress(cumulative, total, speed);
}

/// `Some(size)` when `target.dest` already exists with a length matching a
/// known (`> 0`) `expected_size`; `None` otherwise (including when
/// `expected_size` is unknown, which forces a real download).
fn existing_matching_size(target: &FileTarget) -> Option<u64> {
    if target.expected_size <= 0 {
        return None;
    }
    let meta = fs::metadata(&target.dest).ok()?;
    let expected = target.expected_size as u64;
    (meta.len() == expected).then_some(expected)
}
