use crate::romm::{RommClient, RommError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// One filename scheme for the whole cache: sha256 of the game id.
pub fn cover_key(game_id: i64) -> String {
    let mut h = Sha256::new();
    h.update(game_id.to_le_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sniff_extension(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        _ => "img",
    }
}

pub struct CoverCache {
    dir: PathBuf,
    in_flight: Arc<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
    /// Negative results, kept for the session only: a key that failed once is
    /// not re-fetched, and the stored error is replayed to later callers.
    failed: Arc<Mutex<HashMap<String, RommError>>>,
}

impl CoverCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            failed: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn find_existing(&self, key: &str) -> Option<PathBuf> {
        for ext in ["png", "jpg", "webp", "gif", "img"] {
            let p = self.dir.join(format!("{key}.{ext}"));
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    /// Fetch-on-miss with in-flight deduplication: concurrent calls for the
    /// same key wait for the first fetch instead of re-downloading. A key that
    /// failed earlier in this session returns the recorded error immediately,
    /// without touching the network.
    pub async fn ensure(
        &self,
        client: &RommClient,
        game_id: i64,
        cover_path: &str,
    ) -> Result<PathBuf, RommError> {
        let key = cover_key(game_id);
        loop {
            if let Some(e) = self.failed.lock().await.get(&key) {
                return Err(e.clone());
            }
            if let Some(p) = self.find_existing(&key) {
                return Ok(p);
            }

            let mut map = self.in_flight.lock().await;
            if let Some(existing) = map.get(&key).cloned() {
                // Someone else is fetching: register interest in the
                // notification while STILL holding the map lock. The owner
                // must also acquire this same lock to remove the entry and
                // call notify_waiters(), so `enable()` here happens-before
                // any possible notification. Without this, the gap between
                // dropping the lock and awaiting `.notified()` would let a
                // fast owner's notify_waiters() race ahead of our
                // registration; notify_waiters() stores no permit for late
                // registrants, so a waiter that misses it hangs forever even
                // though the file it's waiting for already exists on disk.
                let notified = existing.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                drop(map);
                notified.await;
                continue;
            }
            map.insert(key.clone(), Arc::new(tokio::sync::Notify::new()));
            drop(map);

            // We own the fetch.
            let result = self.fetch_and_store(client, &key, cover_path).await;
            if let Err(e) = &result {
                self.failed.lock().await.insert(key.clone(), e.clone());
            }
            let n = self.in_flight.lock().await.remove(&key);
            if let Some(n) = n {
                n.notify_waiters();
            }
            return result;
        }
    }

    async fn fetch_and_store(
        &self,
        client: &RommClient,
        key: &str,
        cover_path: &str,
    ) -> Result<PathBuf, RommError> {
        let bytes = client.get_bytes(cover_path).await?;
        std::fs::create_dir_all(&self.dir).map_err(|e| RommError::Connection(e.to_string()))?;
        let target = self.dir.join(format!("{key}.{}", sniff_extension(&bytes)));
        let tmp = target.with_extension("part");
        std::fs::write(&tmp, &bytes).map_err(|e| RommError::Connection(e.to_string()))?;
        std::fs::rename(&tmp, &target).map_err(|e| RommError::Connection(e.to_string()))?;
        Ok(target)
    }
}
