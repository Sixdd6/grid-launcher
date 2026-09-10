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

/// A source `download_targets` can pull bytes from: a RomM server today, a
/// forge host (Task 6) tomorrow. NO Authorization header travels to a forge
/// host — only [`RommProvider`] attaches one, and it does so exclusively for
/// requests aimed at the RomM server that issued the credential.
pub trait ResponseProvider: Sync {
    fn get(
        &self,
        target: &FileTarget,
    ) -> impl std::future::Future<Output = Result<reqwest::Response, LibraryError>> + Send;
}

/// The RomM-backed [`ResponseProvider`]: `target.url_path` is a path relative
/// to the client's base URL, with `target.query` sent as query parameters —
/// today's behavior, unchanged.
pub struct RommProvider<'a>(pub &'a RommClient);

impl ResponseProvider for RommProvider<'_> {
    async fn get(&self, target: &FileTarget) -> Result<reqwest::Response, LibraryError> {
        let query: Vec<(&str, String)> = target
            .query
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        Ok(self.0.get_response(&target.url_path, &query).await?)
    }
}

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
pub async fn download_targets<P: ResponseProvider>(
    provider: &P,
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
            provider,
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
async fn download_one_target<P: ResponseProvider>(
    provider: &P,
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

    let resp = provider.get(target).await?;
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

#[cfg(test)]
pub(crate) mod slow_server {
    //! A single-connection HTTP/1.1 server that paces its response body,
    //! for exercising read (inactivity) timeouts against a real socket.
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    /// Serves one 200 response whose body arrives as `chunks` writes,
    /// sleeping `gaps[i]` before write `i`. Returns the base URL.
    pub(crate) fn serve(chunks: Vec<Vec<u8>>, gaps: Vec<Duration>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(sock.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
                line.clear();
            }
            let total: usize = chunks.iter().map(Vec::len).sum();
            let head =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n");
            sock.write_all(head.as_bytes()).unwrap();
            for (chunk, gap) in chunks.iter().zip(gaps) {
                thread::sleep(gap);
                if sock.write_all(chunk).is_err() {
                    return;
                }
                sock.flush().ok();
            }
        });
        format!("http://{addr}")
    }

    /// `n` 1 KiB chunks, each preceded by `gap`.
    pub(crate) fn steady(n: usize, gap: Duration) -> (Vec<Vec<u8>>, Vec<Duration>) {
        ((0..n).map(|_| vec![7u8; 1024]).collect(), vec![gap; n])
    }

    /// Two 1 KiB chunks with a `stall` before the second.
    pub(crate) fn stalled(stall: Duration) -> (Vec<Vec<u8>>, Vec<Duration>) {
        (
            vec![vec![7u8; 1024], vec![7u8; 1024]],
            vec![Duration::ZERO, stall],
        )
    }
}

#[cfg(test)]
mod read_timeout_tests {
    use super::slow_server;
    use super::*;
    use crate::secrets::Credential;
    use secrecy::SecretString;

    fn client(base: &str, read_timeout: Duration) -> RommClient {
        RommClient::with_read_timeout(
            base,
            Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
            read_timeout,
        )
        .unwrap()
    }

    fn target(dir: &tempfile::TempDir, size: i64) -> FileTarget {
        FileTarget {
            url_path: "/api/roms/1/content/rom.bin".into(),
            query: vec![],
            dest: dir.path().join("rom.bin"),
            expected_size: size,
        }
    }

    /// A transfer that takes far longer than the read timeout must succeed
    /// as long as bytes keep arriving: the timeout is an inactivity limit,
    /// not a cap on total transfer time.
    #[tokio::test]
    async fn long_transfer_survives_when_bytes_keep_arriving() {
        let (chunks, gaps) = slow_server::steady(10, Duration::from_millis(100));
        let base = slow_server::serve(chunks, gaps);
        let client = client(&base, Duration::from_millis(300));
        let dir = tempfile::tempdir().unwrap();
        let t = target(&dir, 10 * 1024);

        let result = download_targets(
            &RommProvider(&client),
            std::slice::from_ref(&t),
            &AtomicBool::new(false),
            &mut |_, _, _| {},
        )
        .await;

        assert!(result.is_ok(), "expected success, got {result:?}");
        assert_eq!(fs::metadata(&t.dest).unwrap().len(), 10 * 1024);
    }

    /// When the server stalls for longer than the read timeout the
    /// download fails and the partial file is removed.
    #[tokio::test]
    async fn stalled_transfer_fails_after_read_timeout() {
        let (chunks, gaps) = slow_server::stalled(Duration::from_millis(1500));
        let base = slow_server::serve(chunks, gaps);
        let client = client(&base, Duration::from_millis(200));
        let dir = tempfile::tempdir().unwrap();
        let t = target(&dir, 2 * 1024);

        let started = Instant::now();
        let result = download_targets(
            &RommProvider(&client),
            std::slice::from_ref(&t),
            &AtomicBool::new(false),
            &mut |_, _, _| {},
        )
        .await;

        assert!(
            matches!(result, Err(LibraryError::Romm(RommError::Connection(_)))),
            "expected connection error, got {result:?}"
        );
        assert!(started.elapsed() < Duration::from_millis(1200));
        assert!(!t.dest.exists());
    }
}
