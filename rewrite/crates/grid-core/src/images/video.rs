//! Server-hosted game videos (`DetailedRomSchema.path_video`, design §7
//! Media tab). They share the image cache's directory and key scheme, but
//! not its content gate: `ImageCache::ensure` refuses anything that is not
//! an image, which is exactly right for covers and exactly wrong here.
//!
//! Token secrecy: the bytes are fetched through the session's `RommClient`,
//! which carries the credential in a header, and the frontend only ever
//! sees the resulting local path. No video URL built here or in the UI
//! carries a token.

use super::cache::{image_key, ImageCache, ImageError};
use super::urls::urlsplit;
use crate::romm::RommClient;
use std::path::PathBuf;

/// The extensions a cached video can be stored under, in the order the
/// cache probes them on a hit.
pub const VIDEO_EXTENSIONS: [&str; 3] = ["mp4", "webm", "mov"];

/// The extension to store a fetched body under, or `None` when the body is
/// not a video. Content-Type first (RomM serves these off disk with a real
/// type), then magic bytes, then — only if both are silent — the URL
/// suffix. An empty body is never a video.
pub fn video_extension_for(url: &str, body: &[u8], content_type: &str) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    let normalized = content_type.trim().to_lowercase();
    let normalized = normalized.split(';').next().unwrap_or("");
    match normalized {
        "video/mp4" => return Some("mp4".to_string()),
        "video/quicktime" => return Some("mov".to_string()),
        "video/webm" => return Some("webm".to_string()),
        _ => {}
    }
    // ISO base media: a 4-byte box size, then `ftyp`.
    if body.len() >= 12 && &body[4..8] == b"ftyp" {
        return Some("mp4".to_string());
    }
    // Matroska/WebM EBML header.
    if body.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("webm".to_string());
    }
    if normalized.starts_with("video/") {
        // A video type this build does not name specifically still plays in
        // the webview more often than not; store it under the URL's own
        // extension when that is one we know, else mp4.
        let path = urlsplit(url).path.to_lowercase();
        for ext in VIDEO_EXTENSIONS {
            if path.ends_with(&format!(".{ext}")) {
                return Some(ext.to_string());
            }
        }
        return Some("mp4".to_string());
    }
    None
}

/// Cache hit → path. Miss with no client → [`ImageError::Offline`]. Miss
/// with a client → fetch through the session client, gate on the body, and
/// write atomically beside the covers.
///
/// Deliberately simpler than [`ImageCache::ensure`]: no in-flight dedup and
/// no negative cache. At most one video is on screen at a time, so there is
/// nothing to deduplicate, and a video that failed once should be
/// retryable by reopening the tab.
pub async fn ensure_video(
    cache: &ImageCache,
    client: Option<&RommClient>,
    url: &str,
) -> Result<PathBuf, ImageError> {
    let key = image_key(url);
    for ext in VIDEO_EXTENSIONS {
        if let Some(path) = cache.find_with_extension(&key, ext) {
            return Ok(path);
        }
    }
    let Some(client) = client else {
        return Err(ImageError::Offline);
    };
    let (bytes, content_type) = client.get_bytes_with_type(url).await?;
    let Some(ext) = video_extension_for(url, &bytes, &content_type) else {
        return Err(ImageError::NotAnImage);
    };
    let io = |e: std::io::Error| ImageError::Io(e.to_string());
    std::fs::create_dir_all(cache.dir()).map_err(io)?;
    let target = cache.dir().join(format!("{key}.{ext}"));
    let tmp = cache.dir().join(format!("{key}.part"));
    std::fs::write(&tmp, &bytes).map_err(io)?;
    std::fs::rename(&tmp, &target).map_err(io)?;
    Ok(target)
}
