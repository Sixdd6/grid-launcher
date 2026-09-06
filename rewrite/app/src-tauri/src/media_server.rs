//! The app's loopback HTTP/1.1 media server: cached game videos, served to
//! the webview over `http://127.0.0.1:<port>/` with Range support.
//!
//! Why an HTTP server and not a URL the webview already understands
//! (2026-09-05, on-screen captures of a standalone WebKitGTK 2.52 window with
//! the app's `WEBKIT_DISABLE_DMABUF_RENDERER=1`, NVIDIA RTX 4070 / driver 610
//! / Wayland):
//!
//! * a custom URI scheme (`asset:`) answers correct 206 Range requests and the
//!   media element still fails with `MEDIA_ERR_SRC_NOT_SUPPORTED` — WebKitGTK
//!   refuses to decode media from a non-network scheme;
//! * a `blob:` object URL decodes but every frame renders corrupted (green or
//!   dark, blocky) in EVERY layout. `WEBKIT_DISABLE_COMPOSITING_MODE=1`,
//!   `GST_PLUGIN_FEATURE_RANK=vah264dec:0`, `WEBKIT_GST_USE_VIDEOCONVERT_SCALE=1`
//!   and `__NV_DISABLE_EXPLICIT_SYNC=1` do not help;
//! * the same file from `file://` and from a loopback `http://127.0.0.1:<port>/`
//!   range server renders perfectly in the app's exact overlay layout.
//!
//! So http(s) — WebKitGTK's normal network media path — is the only source
//! that both decodes and renders. A loopback server, rather than `file://`,
//! keeps the page's CSP to `'self' http://127.0.0.1:*` and keeps the served
//! set to exactly the cached videos.
//!
//! Exposure: the listener binds 127.0.0.1 only, on a kernel-chosen port, and
//! serves nothing but files that sit DIRECTLY inside the image cache
//! directory and end in a [`VIDEO_EXTENSIONS`] extension. Every request must
//! also carry a per-launch nonce (32 random bytes, hex) in its path. The
//! nonce is a loopback capability token, not a credential in the
//! token-secrecy sense — but it is never logged, and neither is any media URL
//! or path.

use grid_core::images::video::VIDEO_EXTENSIONS;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The most request-head bytes read before the connection is refused. A real
/// request from the webview is a few hundred bytes; the cap stops a stuck or
/// hostile peer from growing the buffer without bound.
const MAX_HEAD_BYTES: usize = 8 * 1024;

/// Body streaming granularity. The file is never read whole: a 206 for the
/// first seconds of a long video must not pull the rest into memory.
const CHUNK_BYTES: usize = 64 * 1024;

/// Serves cached video files to the webview over loopback HTTP/1.1 with
/// Range support.
pub struct MediaServer {
    port: u16,
    nonce: String,
    dir: PathBuf,
}

/// What a `Range` header asks for, once resolved against the file length.
#[derive(Debug, PartialEq, Eq)]
enum RangeSpec {
    /// No usable range: send the whole file (RFC 9110 §14.2 — a Range header
    /// that cannot be understood MUST be ignored, not rejected).
    Whole,
    /// Inclusive byte positions, both within the file.
    Slice(u64, u64),
    /// Well formed but outside the file: 416.
    Unsatisfiable,
}

impl MediaServer {
    /// Binds 127.0.0.1:0 (kernel-chosen port), spawns the accept loop on the
    /// Tauri async runtime, and returns the handle. `dir` is the image cache
    /// directory; only files directly inside it with a video extension are
    /// ever served.
    ///
    /// The listener is bound synchronously and handed to the accept task as a
    /// std socket, so it registers with the runtime that will actually poll
    /// it rather than with whatever runtime happened to call `start`.
    pub async fn start(dir: PathBuf) -> std::io::Result<Arc<MediaServer>> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let server = Arc::new(MediaServer {
            port,
            nonce: random_nonce(),
            dir,
        });
        let accept = server.clone();
        tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::warn!("media server accept loop did not start: {e}");
                    return;
                }
            };
            loop {
                match listener.accept().await {
                    Ok((stream, _peer)) => {
                        let server = accept.clone();
                        tauri::async_runtime::spawn(async move {
                            // A dropped connection is the normal end of a
                            // seek: never logged, never retried.
                            let _ = server.serve(stream).await;
                        });
                    }
                    // EMFILE and friends: yielding rather than spinning keeps
                    // the loop alive without a busy wait.
                    Err(_) => tokio::task::yield_now().await,
                }
            }
        });
        Ok(server)
    }

    /// `http://127.0.0.1:<port>/<nonce>/<file name>` for a file the server
    /// will serve, `None` when `path` is not directly inside `dir` or has no
    /// video extension.
    pub fn url_for(&self, path: &Path) -> Option<String> {
        if path.parent() != Some(self.dir.as_path()) {
            return None;
        }
        let name = path.file_name()?.to_str()?;
        if !is_servable_name(name) {
            return None;
        }
        Some(format!(
            "http://127.0.0.1:{}/{}/{}",
            self.port, self.nonce, name
        ))
    }

    #[cfg(test)]
    fn port(&self) -> u16 {
        self.port
    }

    /// The request path for `name`, nonce included — tests only.
    #[cfg(test)]
    fn test_path(&self, name: &str) -> String {
        format!("/{}/{}", self.nonce, name)
    }

    /// One request per connection: read the head, answer, close. `Connection:
    /// close` on every response, so the webview opens a fresh socket per
    /// range request and no keep-alive state is kept here.
    async fn serve(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let head = match read_head(&mut stream).await? {
            Some(head) => head,
            None => return write_head(&mut stream, &not_found()).await,
        };
        let (method, target) = match request_line(&head) {
            Some(parsed) => parsed,
            None => return write_head(&mut stream, &not_found()).await,
        };
        // Anything that is not a read of a servable file gets the same empty
        // 404: a wrong nonce, a wrong method and a wrong name must not be
        // distinguishable from each other.
        let body_wanted = match method {
            "GET" => true,
            "HEAD" => false,
            _ => return write_head(&mut stream, &not_found()).await,
        };
        let path = match self.resolve(target) {
            Some(path) => path,
            None => return write_head(&mut stream, &not_found()).await,
        };
        let len = match tokio::fs::metadata(&path).await {
            Ok(meta) if meta.is_file() => meta.len(),
            _ => return write_head(&mut stream, &not_found()).await,
        };
        let content_type = content_type_for(&path);
        match parse_range(header_value(&head, "range").as_deref(), len) {
            RangeSpec::Unsatisfiable => {
                write_head(
                    &mut stream,
                    &format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Range: bytes */{len}\r\n\
                         Content-Length: 0\r\n\
                         Cache-Control: no-store\r\n\
                         Connection: close\r\n\r\n"
                    ),
                )
                .await
            }
            RangeSpec::Whole => {
                write_head(
                    &mut stream,
                    &format!(
                        "HTTP/1.1 200 OK\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Type: {content_type}\r\n\
                         Content-Length: {len}\r\n\
                         Cache-Control: no-store\r\n\
                         Connection: close\r\n\r\n"
                    ),
                )
                .await?;
                if body_wanted && len > 0 {
                    stream_body(&mut stream, &path, 0, len).await?;
                }
                Ok(())
            }
            RangeSpec::Slice(start, end) => {
                // end is INCLUSIVE in both the header and the count, which is
                // where a range server usually goes wrong by one byte.
                let count = end - start + 1;
                write_head(
                    &mut stream,
                    &format!(
                        "HTTP/1.1 206 Partial Content\r\n\
                         Accept-Ranges: bytes\r\n\
                         Content-Type: {content_type}\r\n\
                         Content-Range: bytes {start}-{end}/{len}\r\n\
                         Content-Length: {count}\r\n\
                         Cache-Control: no-store\r\n\
                         Connection: close\r\n\r\n"
                    ),
                )
                .await?;
                if body_wanted {
                    stream_body(&mut stream, &path, start, count).await?;
                }
                Ok(())
            }
        }
    }

    /// `/<nonce>/<name>` → the file it names, or `None`. Nothing else parses:
    /// no query string, no extra path segment, no percent-decoding (cache
    /// file names are `<sha256 hex>.<ext>`, so there is nothing to decode).
    fn resolve(&self, target: &str) -> Option<PathBuf> {
        let rest = target.strip_prefix('/')?;
        let (nonce, name) = rest.split_once('/')?;
        if nonce != self.nonce || !is_servable_name(name) {
            return None;
        }
        Some(self.dir.join(name))
    }
}

/// A file name the server will serve: one path segment of
/// `[A-Za-z0-9._-]`, no `..`, ending in a known video extension. The
/// character allowlist is what rules out `/`, `\`, NUL and `%` at once, so
/// no separate traversal check is needed beyond `..`.
fn is_servable_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 || name.contains("..") {
        return false;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    VIDEO_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// The `Content-Type` for a servable name. Only the three extensions the
/// cache stores exist here; the fallback never fires for a name that passed
/// [`is_servable_name`].
fn content_type_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        _ => "video/mp4",
    }
}

/// 32 random bytes as hex. A failure to draw from the OS entropy source is
/// unrecoverable for a capability token, so it panics rather than falling
/// back to something guessable.
fn random_nonce() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the OS entropy source must be available");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The request head as text, or `None` when the peer closed early or sent
/// more than [`MAX_HEAD_BYTES`] without terminating the head.
async fn read_head(stream: &mut TcpStream) -> std::io::Result<Option<String>> {
    let mut head = Vec::new();
    let mut buf = [0u8; 2048];
    loop {
        if let Some(end) = head.windows(4).position(|w| w == b"\r\n\r\n") {
            return Ok(String::from_utf8(head[..end].to_vec()).ok());
        }
        if head.len() >= MAX_HEAD_BYTES {
            return Ok(None);
        }
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Ok(None);
        }
        head.extend_from_slice(&buf[..n]);
    }
}

/// `(method, request-target)` from the first line. The HTTP version is not
/// checked: the only client is the webview on this machine.
fn request_line(head: &str) -> Option<(&str, &str)> {
    let line = head.split("\r\n").next()?;
    let mut parts = line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    if method.is_empty() || target.is_empty() {
        return None;
    }
    Some((method, target))
}

/// The first value of `name` (lowercase, no colon), trimmed.
fn header_value(head: &str, name: &str) -> Option<String> {
    head.split("\r\n").skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim().to_ascii_lowercase() == name {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

/// RFC 9110 §14.1.2, the three single-range forms: `bytes=a-b`, `bytes=a-`
/// and `bytes=-n`. Anything else — another unit, a multi-range set, a
/// malformed spec, `last < first` — is ignored, which means the whole file.
/// A well-formed range that starts past the end (or any range at all on an
/// empty file) is unsatisfiable.
fn parse_range(header: Option<&str>, len: u64) -> RangeSpec {
    let spec = match header.and_then(|h| h.trim().strip_prefix("bytes=")) {
        Some(spec) => spec.trim(),
        None => return RangeSpec::Whole,
    };
    if spec.contains(',') {
        return RangeSpec::Whole;
    }
    let (first, last) = match spec.split_once('-') {
        Some(parts) => parts,
        None => return RangeSpec::Whole,
    };
    let (first, last) = (first.trim(), last.trim());
    if first.is_empty() {
        // Suffix form: the last `n` bytes.
        let n: u64 = match last.parse() {
            Ok(n) => n,
            Err(_) => return RangeSpec::Whole,
        };
        if n == 0 || len == 0 {
            return RangeSpec::Unsatisfiable;
        }
        return RangeSpec::Slice(len.saturating_sub(n), len - 1);
    }
    let start: u64 = match first.parse() {
        Ok(start) => start,
        Err(_) => return RangeSpec::Whole,
    };
    if start >= len {
        return RangeSpec::Unsatisfiable;
    }
    if last.is_empty() {
        return RangeSpec::Slice(start, len - 1);
    }
    let end: u64 = match last.parse() {
        Ok(end) => end,
        Err(_) => return RangeSpec::Whole,
    };
    if end < start {
        return RangeSpec::Whole;
    }
    RangeSpec::Slice(start, end.min(len - 1))
}

/// The 404 every rejection shares: empty body, no reason. A wrong nonce must
/// not be distinguishable from a missing file.
fn not_found() -> String {
    "HTTP/1.1 404 Not Found\r\n\
     Content-Length: 0\r\n\
     Cache-Control: no-store\r\n\
     Connection: close\r\n\r\n"
        .to_string()
}

async fn write_head(stream: &mut TcpStream, head: &str) -> std::io::Result<()> {
    stream.write_all(head.as_bytes()).await?;
    stream.flush().await
}

/// `count` bytes from `start`, in [`CHUNK_BYTES`] pieces. The file is opened
/// per request and seeked, so nothing here holds a whole video in memory.
async fn stream_body(
    stream: &mut TcpStream,
    path: &Path,
    start: u64,
    count: u64,
) -> std::io::Result<()> {
    let mut file = tokio::fs::File::open(path).await?;
    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start)).await?;
    }
    let mut remaining = count;
    let mut buf = vec![0u8; CHUNK_BYTES];
    while remaining > 0 {
        let want = remaining.min(CHUNK_BYTES as u64) as usize;
        let n = file.read(&mut buf[..want]).await?;
        if n == 0 {
            // The file shrank under us: stop rather than pad. The declared
            // Content-Length is now wrong, and closing the connection is how
            // the client learns the body ended early.
            break;
        }
        stream.write_all(&buf[..n]).await?;
        remaining -= n as u64;
    }
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::MediaServer;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    /// 4 KiB of known, non-repeating-per-byte content so a slice assertion
    /// fails loudly on an off-by-one.
    fn fixture_bytes() -> Vec<u8> {
        (0..4096u32).map(|i| (i % 251) as u8).collect()
    }

    async fn server() -> (tempfile::TempDir, Arc<MediaServer>, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let bytes = fixture_bytes();
        std::fs::write(dir.path().join("a.mp4"), &bytes).unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not a video").unwrap();
        let server = MediaServer::start(dir.path().to_path_buf()).await.unwrap();
        (dir, server, bytes)
    }

    /// Raw HTTP/1.1 client: writes `raw` verbatim and reads to EOF, which the
    /// server's `Connection: close` guarantees. Returns (head, body).
    async fn request(port: u16, raw: &str) -> (String, Vec<u8>) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(raw.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        let mut all = Vec::new();
        stream.read_to_end(&mut all).await.unwrap();
        let split = all
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("the response has a header terminator");
        (
            String::from_utf8(all[..split].to_vec()).unwrap(),
            all[split + 4..].to_vec(),
        )
    }

    fn get(path: &str) -> String {
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
    }

    fn ranged(path: &str, range: &str) -> String {
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nRange: {range}\r\n\r\n")
    }

    #[tokio::test]
    async fn get_returns_the_whole_file() {
        let (_dir, server, bytes) = server().await;
        let (head, body) = request(server.port(), &get(&server.test_path("a.mp4"))).await;
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
        assert!(head.contains("Accept-Ranges: bytes\r\n"), "{head}");
        assert!(head.contains("Content-Type: video/mp4\r\n"), "{head}");
        assert!(head.contains("Content-Length: 4096\r\n"), "{head}");
        assert!(head.contains("Cache-Control: no-store\r\n"), "{head}");
        assert_eq!(body, bytes);
    }

    #[tokio::test]
    async fn closed_range_returns_exactly_that_slice() {
        let (_dir, server, bytes) = server().await;
        let (head, body) = request(
            server.port(),
            &ranged(&server.test_path("a.mp4"), "bytes=100-199"),
        )
        .await;
        assert!(
            head.starts_with("HTTP/1.1 206 Partial Content\r\n"),
            "{head}"
        );
        assert!(
            head.contains("Content-Range: bytes 100-199/4096\r\n"),
            "{head}"
        );
        assert!(head.contains("Content-Length: 100\r\n"), "{head}");
        assert_eq!(body, bytes[100..200]);
    }

    #[tokio::test]
    async fn open_ended_range_returns_the_tail() {
        let (_dir, server, bytes) = server().await;
        let (head, body) = request(
            server.port(),
            &ranged(&server.test_path("a.mp4"), "bytes=4000-"),
        )
        .await;
        assert!(
            head.starts_with("HTTP/1.1 206 Partial Content\r\n"),
            "{head}"
        );
        assert!(
            head.contains("Content-Range: bytes 4000-4095/4096\r\n"),
            "{head}"
        );
        assert!(head.contains("Content-Length: 96\r\n"), "{head}");
        assert_eq!(body, bytes[4000..]);
    }

    #[tokio::test]
    async fn suffix_range_returns_the_last_bytes() {
        let (_dir, server, bytes) = server().await;
        let (head, body) = request(
            server.port(),
            &ranged(&server.test_path("a.mp4"), "bytes=-100"),
        )
        .await;
        assert!(
            head.starts_with("HTTP/1.1 206 Partial Content\r\n"),
            "{head}"
        );
        assert!(
            head.contains("Content-Range: bytes 3996-4095/4096\r\n"),
            "{head}"
        );
        assert_eq!(body, bytes[3996..]);
    }

    #[tokio::test]
    async fn range_past_the_end_is_416() {
        let (_dir, server, _bytes) = server().await;
        let (head, body) = request(
            server.port(),
            &ranged(&server.test_path("a.mp4"), "bytes=5000-"),
        )
        .await;
        assert!(
            head.starts_with("HTTP/1.1 416 Range Not Satisfiable\r\n"),
            "{head}"
        );
        assert!(head.contains("Content-Range: bytes */4096\r\n"), "{head}");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn head_sends_headers_without_a_body() {
        let (_dir, server, _bytes) = server().await;
        let path = server.test_path("a.mp4");
        let (head, body) = request(
            server.port(),
            &format!("HEAD {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
        assert!(head.contains("Content-Length: 4096\r\n"), "{head}");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn wrong_nonce_is_404() {
        let (_dir, server, _bytes) = server().await;
        let (head, body) = request(server.port(), &get("/0123456789abcdef/a.mp4")).await;
        assert!(head.starts_with("HTTP/1.1 404 Not Found\r\n"), "{head}");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn a_non_video_file_is_404() {
        let (_dir, server, _bytes) = server().await;
        let (head, _body) = request(server.port(), &get(&server.test_path("notes.txt"))).await;
        assert!(head.starts_with("HTTP/1.1 404 Not Found\r\n"), "{head}");
    }

    #[tokio::test]
    async fn a_traversal_path_is_404() {
        let (_dir, server, _bytes) = server().await;
        let (head, _body) = request(server.port(), &get(&server.test_path("../a.mp4"))).await;
        assert!(head.starts_with("HTTP/1.1 404 Not Found\r\n"), "{head}");
    }

    #[tokio::test]
    async fn a_missing_file_is_404() {
        let (_dir, server, _bytes) = server().await;
        let (head, _body) = request(server.port(), &get(&server.test_path("absent.mp4"))).await;
        assert!(head.starts_with("HTTP/1.1 404 Not Found\r\n"), "{head}");
    }

    #[tokio::test]
    async fn a_write_method_is_404() {
        let (_dir, server, _bytes) = server().await;
        let path = server.test_path("a.mp4");
        let (head, _body) = request(
            server.port(),
            &format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n"),
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 404 Not Found\r\n"), "{head}");
    }

    #[tokio::test]
    async fn url_for_accepts_only_video_files_directly_inside_the_directory() {
        let (dir, server, _bytes) = server().await;
        let url = server.url_for(&dir.path().join("a.mp4")).unwrap();
        assert_eq!(
            url,
            format!(
                "http://127.0.0.1:{}{}",
                server.port(),
                server.test_path("a.mp4")
            )
        );
        assert!(server.url_for(&dir.path().join("notes.txt")).is_none());
        assert!(server
            .url_for(&dir.path().join("sub").join("a.mp4"))
            .is_none());
        assert!(server
            .url_for(&PathBuf::from("/tmp/elsewhere/a.mp4"))
            .is_none());
    }
}
