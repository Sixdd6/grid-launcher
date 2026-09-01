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
}

impl CoverCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            in_flight: Arc::new(Mutex::new(HashMap::new())),
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
    /// same key wait for the first fetch instead of re-downloading.
    pub async fn ensure(
        &self,
        client: &RommClient,
        game_id: i64,
        cover_path: &str,
    ) -> Result<PathBuf, RommError> {
        let key = cover_key(game_id);
        loop {
            if let Some(p) = self.find_existing(&key) {
                return Ok(p);
            }
            let notify = {
                let mut map = self.in_flight.lock().await;
                if let Some(existing) = map.get(&key) {
                    // Someone else is fetching: wait, then re-check the disk.
                    Some(existing.clone())
                } else {
                    map.insert(key.clone(), Arc::new(tokio::sync::Notify::new()));
                    None
                }
            };
            if let Some(n) = notify {
                n.notified().await;
                continue;
            }
            // We own the fetch.
            let result = self.fetch_and_store(client, &key, cover_path).await;
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
