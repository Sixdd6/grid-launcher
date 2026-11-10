//! Disk cache for covers and screenshots (spec "Cache"). One filename
//! scheme (D1): `<sha256(resolved url)>.<ext>`. In-flight dedup, a
//! per-session negative map, a download semaphore (D4) and the content
//! gate (D8).

use super::urls::{extension_for, filter_to_server_host, resolve_image_url, LOOKUP_EXTENSIONS};
use crate::romm::{RommClient, RommError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{Mutex, Notify, Semaphore};

pub const MAX_CONCURRENT_DOWNLOADS: usize = 6;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ImageError {
    #[error("not connected")]
    Offline,
    #[error("the server did not return an image")]
    NotAnImage,
    /// The video endpoint answered with a body that is not a video (an HTML
    /// error page, say). Distinct from [`ImageError::NotAnImage`] so the
    /// message names what was actually asked for.
    #[error("the server did not return a video")]
    NotAVideo,
    /// The cached bytes are not a format this build can decode. Distinct
    /// from [`ImageError::NotAnImage`]: the server DID return an image (an
    /// SVG, an AVIF), it just cannot be turned into a background variant.
    /// Deterministic, so `images::background` records it per session and
    /// stops re-decoding the same source on every hover.
    #[error("the image could not be decoded")]
    Decode,
    /// The re-encode of a successfully decoded image failed. Separate from
    /// [`ImageError::Decode`] so the message never blames the source for a
    /// fault on our side.
    #[error("the image could not be re-encoded")]
    Encode,
    #[error(transparent)]
    Http(#[from] RommError),
    #[error("file error: {0}")]
    Io(String),
}

/// Lowercase hex SHA-256 of the resolved absolute URL.
pub fn image_key(url: &str) -> String {
    Sha256::digest(url.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Where an image request goes, decided by [`ImageCache::lookup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageLookup {
    /// Already on disk. No request is made.
    Hit(PathBuf),
    /// Not cached, and the resolved URL may be fetched with the session
    /// client (it passed the server-host filter).
    Fetch(String),
    /// Not cached, and the URL must not be fetched: it resolves to nothing,
    /// or to a host other than the connected server, where the session's
    /// credential must never go.
    Refused,
}

/// Makes each seed's temp name unique, so two seeds of one key (or a seed
/// racing a fetch, which uses `<key>.part`) never share a file.
static SEED_SEQ: AtomicU64 = AtomicU64::new(0);

/// The in-flight map shape shared by the source fetch and the derived-variant
/// build: one `Notify` per key, woken when its owner finishes.
pub type InFlightMap = Mutex<HashMap<String, Arc<Notify>>>;

pub struct ImageCache {
    dir: PathBuf,
    downloads: Semaphore,
    in_flight: InFlightMap,
    /// Session-only negative cache: a URL that failed once replays its error.
    failed: Mutex<HashMap<String, ImageError>>,
    /// The same two maps again, for the derived background variants
    /// (`images::background`). Kept separate from `in_flight`/`failed`
    /// because they are keyed on the same key but gate a different file:
    /// a source that is cached fine can still fail to become a variant.
    variant_in_flight: InFlightMap,
    variant_failed: Mutex<HashMap<String, ImageError>>,
}

impl ImageCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            downloads: Semaphore::new(MAX_CONCURRENT_DOWNLOADS),
            in_flight: Mutex::new(HashMap::new()),
            failed: Mutex::new(HashMap::new()),
            variant_in_flight: Mutex::new(HashMap::new()),
            variant_failed: Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Dedup state for `images::background`: without it the shell's two
    /// dwell timers (150ms and 500ms) start two builds of the same cold
    /// variant, which duplicates the decode+blur and races on the temp file.
    pub fn variant_in_flight(&self) -> &InFlightMap {
        &self.variant_in_flight
    }

    /// Session-only negative cache for `images::background`, holding only
    /// deterministic failures ([`ImageError::Decode`]).
    pub fn variant_failed(&self) -> &Mutex<HashMap<String, ImageError>> {
        &self.variant_failed
    }

    pub fn find_existing(&self, key: &str) -> Option<PathBuf> {
        LOOKUP_EXTENSIONS
            .iter()
            .map(|ext| self.dir.join(format!("{key}.{ext}")))
            .find(|p| p.is_file())
    }

    /// The cached file for `key` under one exact extension, refreshing its
    /// mtime so the startup sweep treats it as recently used.
    /// [`find_existing`](Self::find_existing) only looks at the image
    /// extensions; video files live in the same directory under the same
    /// key scheme and need their own lookup.
    pub fn find_with_extension(&self, key: &str, ext: &str) -> Option<PathBuf> {
        let path = self.dir.join(format!("{key}.{ext}"));
        if !path.is_file() {
            return None;
        }
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(path)
    }

    /// The cached file for `url`, refreshing its mtime so the sweep treats
    /// it as recently used. No network.
    pub fn cached_path(&self, url: &str) -> Option<PathBuf> {
        let path = self.find_existing(&image_key(url))?;
        // Best effort: a read-only cache dir must not turn a hit into a miss.
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(path)
    }

    /// Resolves `url` against `base` (the stored server URL) and checks the
    /// cache BEFORE the server-host filter. A hit makes no request, so it may
    /// serve a URL on any host: a row installed from a drive keeps the
    /// absolute URL of the server it was backed up from, and its image must
    /// show offline and on a computer connected to another server. A miss
    /// applies today's filter unchanged: a URL not on `base`'s host is
    /// `Refused`, never fetched, so the session credential cannot leak.
    ///
    /// One deliberate tightening: with no server URL at all, a miss is
    /// `Refused`. `filter_to_server_host` passes anything through on an empty
    /// base, which was harmless only because no client exists then; this
    /// does not rely on that.
    pub fn lookup(&self, url: &str, base: &str) -> ImageLookup {
        let resolved = resolve_image_url(url, base);
        if resolved.is_empty() {
            return ImageLookup::Refused;
        }
        if let Some(path) = self.cached_path(&resolved) {
            return ImageLookup::Hit(path);
        }
        if base.is_empty() {
            return ImageLookup::Refused;
        }
        let allowed = filter_to_server_host(&resolved, base);
        if allowed.is_empty() {
            ImageLookup::Refused
        } else {
            ImageLookup::Fetch(allowed)
        }
    }

    /// Puts a local image file (a drive pack's asset) into the cache under
    /// `url`'s key, so later lookups of `url` hit without a request. The
    /// extension is sniffed from the bytes, then from `url`'s path, as for a
    /// download. The copy is atomic: a temp file in the cache directory, then
    /// a rename. Any older copy of the key under another extension is
    /// removed, so the lookup cannot return a stale file.
    pub fn seed(&self, url: &str, src_file: &Path) -> Result<PathBuf, ImageError> {
        let io = |e: std::io::Error| ImageError::Io(e.to_string());
        let bytes = std::fs::read(src_file).map_err(io)?;
        if bytes.is_empty() {
            return Err(ImageError::NotAnImage);
        }
        let sniff = extension_for(url, &bytes, "");
        // No magic bytes and no image suffix on the URL: not an image.
        if !sniff.identified && sniff.ext == "img" {
            return Err(ImageError::NotAnImage);
        }
        let key = image_key(url);
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let target = self.dir.join(format!("{key}.{}", sniff.ext));
        let tmp = self.dir.join(format!(
            "{key}.seed.{}-{}.part",
            std::process::id(),
            SEED_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        if let Err(e) = std::fs::write(&tmp, &bytes) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io(e));
        }
        if let Err(e) = std::fs::rename(&tmp, &target) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io(e));
        }
        for ext in LOOKUP_EXTENSIONS {
            if ext != sniff.ext {
                let _ = std::fs::remove_file(self.dir.join(format!("{key}.{ext}")));
            }
        }
        Ok(target)
    }

    /// Cache hit → path. Miss with no client → `Offline` (not recorded).
    /// Miss with a client → fetch (deduplicated per URL, at most
    /// `MAX_CONCURRENT_DOWNLOADS` in flight), gate, write atomically.
    pub async fn ensure(
        &self,
        client: Option<&RommClient>,
        url: &str,
    ) -> Result<PathBuf, ImageError> {
        let key = image_key(url);
        loop {
            if let Some(e) = self.failed.lock().await.get(&key) {
                return Err(e.clone());
            }
            if let Some(p) = self.cached_path(url) {
                return Ok(p);
            }
            let Some(client) = client else {
                return Err(ImageError::Offline);
            };

            let mut map = self.in_flight.lock().await;
            if let Some(existing) = map.get(&key).cloned() {
                // Register interest while still holding the map lock: the
                // owner takes this same lock before notify_waiters(), so
                // enable() happens-before any notification and no wakeup
                // is lost (regression-tested by
                // ensure_dedups_concurrent_callers_for_same_url).
                let notified = existing.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                drop(map);
                notified.await;
                continue;
            }
            map.insert(key.clone(), Arc::new(Notify::new()));
            drop(map);

            let result = self.fetch_and_store(client, &key, url).await;
            if let Err(e) = &result {
                self.failed.lock().await.insert(key.clone(), e.clone());
            }
            if let Some(n) = self.in_flight.lock().await.remove(&key) {
                n.notify_waiters();
            }
            return result;
        }
    }

    async fn fetch_and_store(
        &self,
        client: &RommClient,
        key: &str,
        url: &str,
    ) -> Result<PathBuf, ImageError> {
        let (bytes, content_type) = {
            let _permit = self
                .downloads
                .acquire()
                .await
                .expect("image semaphore is never closed");
            client.get_bytes_with_type(url).await?
        };
        if bytes.is_empty() {
            return Err(ImageError::NotAnImage);
        }
        let sniff = extension_for(url, &bytes, &content_type);
        if !sniff.identified
            && !content_type
                .trim()
                .to_ascii_lowercase()
                .starts_with("image/")
        {
            return Err(ImageError::NotAnImage);
        }
        let io = |e: std::io::Error| ImageError::Io(e.to_string());
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let target = self.dir.join(format!("{key}.{}", sniff.ext));
        let tmp = self.dir.join(format!("{key}.part"));
        std::fs::write(&tmp, &bytes).map_err(io)?;
        std::fs::rename(&tmp, &target).map_err(io)?;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-a-png";

    fn src(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn seed_then_cached_path_hits() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        let file = src(tmp.path(), "cover", PNG);
        let url = "https://romm.example/assets/romm/resources/roms/1/cover/big.png";
        let seeded = cache.seed(url, &file).unwrap();
        assert_eq!(
            seeded,
            tmp.path()
                .join("covers")
                .join(format!("{}.png", image_key(url)))
        );
        assert_eq!(std::fs::read(&seeded).unwrap(), PNG);
        assert_eq!(cache.cached_path(url), Some(seeded));
        // No temp file is left behind.
        let names: Vec<_> = std::fs::read_dir(tmp.path().join("covers"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    #[test]
    fn seed_sniffs_the_bytes_over_the_url_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        let file = src(tmp.path(), "cover", b"\xff\xd8\xffjpeg-bytes");
        let url = "gridpack:///media/usb/gridpack/assets/1/cover.png";
        let seeded = cache.seed(url, &file).unwrap();
        assert_eq!(seeded.extension().unwrap(), "jpg");
        assert_eq!(cache.cached_path(url), Some(seeded));
    }

    #[test]
    fn seed_replaces_a_copy_under_another_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        let url = "https://romm.example/cover";
        let old = cache
            .seed(url, &src(tmp.path(), "a", b"\xff\xd8\xffjpeg"))
            .unwrap();
        let new = cache.seed(url, &src(tmp.path(), "b", PNG)).unwrap();
        assert!(!old.exists());
        assert_eq!(cache.cached_path(url), Some(new));
    }

    #[test]
    fn seed_refuses_a_file_that_is_not_an_image() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        let url = "https://romm.example/cover";
        let file = src(tmp.path(), "page", b"<html>nope</html>");
        assert!(matches!(
            cache.seed(url, &file),
            Err(ImageError::NotAnImage)
        ));
        let empty = src(tmp.path(), "empty", b"");
        assert!(matches!(
            cache.seed(url, &empty),
            Err(ImageError::NotAnImage)
        ));
        assert_eq!(cache.cached_path(url), None);
    }

    #[test]
    fn lookup_serves_a_cached_cross_host_url_without_a_fetch() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        let url = "https://other-server.example/assets/romm/resources/cover.png";
        let seeded = cache.seed(url, &src(tmp.path(), "c", PNG)).unwrap();
        assert_eq!(
            cache.lookup(url, "https://romm.example"),
            ImageLookup::Hit(seeded.clone())
        );
        // No server at all (offline, never connected): still a hit.
        assert_eq!(cache.lookup(url, ""), ImageLookup::Hit(seeded));
    }

    /// The token-leak guard: a cross-host URL that is NOT cached is refused,
    /// so the session client never sends its credential to that host.
    #[test]
    fn lookup_refuses_an_uncached_cross_host_url() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        assert_eq!(
            cache.lookup("https://attacker.example/steal.png", "https://romm.example"),
            ImageLookup::Refused
        );
        // A cached image for some other URL does not let this one through.
        cache
            .seed(
                "https://attacker.example/other.png",
                &src(tmp.path(), "c", PNG),
            )
            .unwrap();
        assert_eq!(
            cache.lookup("https://attacker.example/steal.png", "https://romm.example"),
            ImageLookup::Refused
        );
    }

    #[test]
    fn lookup_fetches_an_uncached_same_host_url_exactly_as_before() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        let base = "https://romm.example";
        for url in [
            "/assets/romm/resources/roms/1/cover/big.png",
            "https://romm.example/assets/romm/resources/roms/1/cover/big.png",
            "roms/1/fanart.jpg",
        ] {
            let expected = filter_to_server_host(&resolve_image_url(url, base), base);
            assert!(!expected.is_empty());
            assert_eq!(cache.lookup(url, base), ImageLookup::Fetch(expected));
        }
    }

    #[test]
    fn lookup_refuses_what_resolves_to_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(tmp.path().join("covers"));
        assert_eq!(
            cache.lookup("", "https://romm.example"),
            ImageLookup::Refused
        );
        // A relative path with no server cannot be resolved.
        assert_eq!(cache.lookup("/cover.png", ""), ImageLookup::Refused);
        // No server and not cached: an absolute URL is refused too.
        assert_eq!(
            cache.lookup("https://romm.example/cover.png", ""),
            ImageLookup::Refused
        );
    }
}
