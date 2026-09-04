//! Disk cache for covers and screenshots (spec "Cache"). One filename
//! scheme (D1): `<sha256(resolved url)>.<ext>`. In-flight dedup, a
//! per-session negative map, a download semaphore (D4) and the content
//! gate (D8).

use super::urls::{extension_for, LOOKUP_EXTENSIONS};
use crate::romm::{RommClient, RommError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

pub struct ImageCache {
    dir: PathBuf,
    downloads: Semaphore,
    in_flight: Mutex<HashMap<String, Arc<Notify>>>,
    /// Session-only negative cache: a URL that failed once replays its error.
    failed: Mutex<HashMap<String, ImageError>>,
}

impl ImageCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            downloads: Semaphore::new(MAX_CONCURRENT_DOWNLOADS),
            in_flight: Mutex::new(HashMap::new()),
            failed: Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
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
