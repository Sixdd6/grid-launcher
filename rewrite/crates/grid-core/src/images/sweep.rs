//! Bounded cache (D3): a startup sweep deletes the least-recently-modified
//! unpinned files until the directory is under the cap. Installed rows'
//! covers are pinned; screenshots never are.

use super::cache::image_key;
use super::urls::{filter_to_server_host, resolve_image_url};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// 1 GiB, raised from 512 MB on 2026-09-05. The background-art tier now
/// reaches full-size fanart, and every installed row's fanart source is
/// pinned together with its blurred variant. Fanart is a few MB per game
/// where a cover is tens of KB, so a large library's pinned set alone could
/// pass the old cap — at which point each startup sweep deleted every
/// unpinned cover and screenshot without ever getting under it. `sweep` warns
/// when the pinned bytes reach the cap.
pub const IMAGE_CACHE_CAP_BYTES: u64 = 1024 * 1024 * 1024;
pub const STALE_PART_AGE: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepReport {
    pub total_before: u64,
    pub total_after: u64,
    pub deleted: usize,
    pub stale_parts: usize,
}

/// Keys of every non-empty cover path that resolves to the server host.
pub fn pinned_keys<'a>(
    cover_paths: impl IntoIterator<Item = &'a str>,
    base_url: &str,
) -> HashSet<String> {
    cover_paths
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| filter_to_server_host(&resolve_image_url(p, base_url), base_url))
        .filter(|u| !u.is_empty())
        .map(|u| image_key(&u))
        .collect()
}

struct Entry {
    path: std::path::PathBuf,
    size: u64,
    mtime: SystemTime,
    pinned: bool,
}

pub fn sweep(dir: &Path, cap_bytes: u64, pinned: &HashSet<String>) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(read) = std::fs::read_dir(dir) else {
        return report;
    };
    let now = SystemTime::now();
    let mut entries = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta.modified().unwrap_or(now);
        if path.extension().is_some_and(|e| e == "part") {
            if now.duration_since(mtime).unwrap_or_default() > STALE_PART_AGE
                && std::fs::remove_file(&path).is_ok()
            {
                report.stale_parts += 1;
            }
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // `<key>.bg<sigma>.jpg`'s file stem is `<key>.bg<sigma>`, so a
        // whole-stem compare would leave every background variant unpinned
        // and let the sweep evict an installed game's art while keeping its
        // source. Keys are hex SHA-256 and never contain a dot, so the
        // prefix up to the first dot is the key.
        let key = stem.split('.').next().unwrap_or(stem);
        entries.push(Entry {
            pinned: pinned.contains(key),
            path,
            size: meta.len(),
            mtime,
        });
    }
    report.total_before = entries.iter().map(|e| e.size).sum();
    report.total_after = report.total_before;
    if report.total_before <= cap_bytes {
        return report;
    }
    // Pinned entries are never victims, so once they alone fill the cap the
    // sweep deletes every unpinned file and still ends over it — the Server
    // view's covers and screenshots are then re-downloaded on every launch.
    // Counts only; no path or URL is logged.
    let pinned_bytes: u64 = entries.iter().filter(|e| e.pinned).map(|e| e.size).sum();
    if pinned_bytes >= cap_bytes {
        tracing::warn!(
            "image cache sweep: {} pinned files hold {} bytes, at or over the {} byte cap; \
             the sweep cannot get under it and every unpinned file will be deleted",
            entries.iter().filter(|e| e.pinned).count(),
            pinned_bytes,
            cap_bytes
        );
    }
    let mut victims: Vec<&Entry> = entries.iter().filter(|e| !e.pinned).collect();
    victims.sort_by_key(|e| e.mtime);
    for victim in victims {
        if report.total_after <= cap_bytes {
            break;
        }
        match std::fs::remove_file(&victim.path) {
            Ok(()) => {
                report.total_after -= victim.size;
                report.deleted += 1;
            }
            Err(_) => {
                // Ignore deletion errors: a file that will not delete is a
                // cache entry the next sweep tries again, never a failure.
                // Not logged — the path would name a cached URL.
            }
        }
    }
    report
}
