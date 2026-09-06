//! Bounded cache (D3): a startup sweep deletes the least-recently-modified
//! unpinned files until the directory is under the cap. Installed rows'
//! covers are pinned; screenshots never are.

use super::cache::image_key;
use super::urls::{filter_to_server_host, resolve_image_url};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub const IMAGE_CACHE_CAP_BYTES: u64 = 512 * 1024 * 1024;
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
                // Ignore deletion errors silently; tracing is not a grid-core dependency.
            }
        }
    }
    report
}
