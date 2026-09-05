//! Replenish (doc 07 "Replenishment of missing covers", D6): after a
//! successful connect, back-fill image fields for rows that lack them and
//! fetch missing small-cover files. Sequential, never fails; errors skip
//! the item.

use super::background::{ensure_background_variant, BACKGROUND_VARIANT_EXT};
use super::cache::{image_key, ImageCache};
use super::urls::{filter_to_server_host, resolve_image_url};
use super::ImageFields;
use crate::library::registry::{InstalledGame, Registry};
use crate::romm::RommClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplenishItem {
    NeedsFields {
        rom_id: i64,
    },
    NeedsFile {
        rom_id: i64,
        url: String,
    },
    /// The row's background source (its first fanart, else its first
    /// screenshot, else its large cover) has no `<key>.bg.jpg` yet. Planned
    /// LAST, after every `NeedsFields`/`NeedsFile` item, so building variants
    /// never delays the grid covers the user is actually looking at.
    NeedsVariant {
        rom_id: i64,
        url: String,
    },
}

/// How many background variants one replenish pass may build. Each one is a
/// full-size image download, so building every installed row's variant on the
/// first connect after an upgrade would saturate the link; the rest are built
/// lazily when the shell first shows them.
pub const BACKGROUND_VARIANT_LIMIT: usize = 32;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReplenishReport {
    pub updated_rows: usize,
    pub fetched_files: usize,
    pub skipped: usize,
}

fn small_cover_url(row: &InstalledGame, base_url: &str) -> String {
    filter_to_server_host(
        &resolve_image_url(&row.cover_small_path, base_url),
        base_url,
    )
}

/// The URL the background art would show for `row`: its first fanart, else
/// its first screenshot (both already resolved + host-filtered when they were
/// stored), else its large cover. Mirrors `backgroundUrls`' priority on the
/// frontend.
///
/// Public because the startup sweep pins this URL's key too: pinning only the
/// cover keys would evict a fanart-sourced variant AND its source on every
/// start above the cache cap, so the next hover rebuilds them, forever.
pub fn background_source_url(row: &InstalledGame, base_url: &str) -> String {
    for stored in [&row.fanart_urls, &row.screenshot_urls] {
        if let Some(first) = stored.lines().map(str::trim).find(|u| !u.is_empty()) {
            return filter_to_server_host(&resolve_image_url(first, base_url), base_url);
        }
    }
    filter_to_server_host(
        &resolve_image_url(&row.cover_large_path, base_url),
        base_url,
    )
}

pub fn plan(rows: &[InstalledGame], cache: &ImageCache, base_url: &str) -> Vec<ReplenishItem> {
    let mut items = Vec::new();
    let mut variants = Vec::new();
    for row in rows {
        let Some(rom_id) = row.rom_id else { continue };
        if row.cover_small_path.is_empty()
            && row.cover_large_path.is_empty()
            && row.screenshot_urls.is_empty()
        {
            items.push(ReplenishItem::NeedsFields { rom_id });
            continue;
        }
        let url = small_cover_url(row, base_url);
        if !url.is_empty() && cache.find_existing(&image_key(&url)).is_none() {
            items.push(ReplenishItem::NeedsFile { rom_id, url });
        }
        let background = background_source_url(row, base_url);
        if !background.is_empty()
            && cache
                .find_with_extension(&image_key(&background), BACKGROUND_VARIANT_EXT)
                .is_none()
        {
            variants.push((
                row.last_played_at,
                row.installed_at,
                ReplenishItem::NeedsVariant {
                    rom_id,
                    url: background,
                },
            ));
        }
    }
    // Most recently played first, then most recently installed — the rows the
    // shell is most likely to put behind the grid next. `sort_by` is stable,
    // so rows with no history keep their registry order.
    variants.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    variants.truncate(BACKGROUND_VARIANT_LIMIT);
    items.extend(variants.into_iter().map(|(_, _, item)| item));
    items
}

pub async fn run(
    client: &RommClient,
    cache: &ImageCache,
    registry: &Registry,
    base_url: &str,
    items: Vec<ReplenishItem>,
) -> ReplenishReport {
    let mut report = ReplenishReport::default();
    for item in items {
        match item {
            ReplenishItem::NeedsFields { rom_id } => {
                let detail = match client.rom_detail(rom_id).await {
                    Ok(d) => d,
                    Err(_) => {
                        report.skipped += 1;
                        continue;
                    }
                };
                let fields = ImageFields::from_detail(&detail);
                match registry.update_images(rom_id, &fields) {
                    Ok(true) => report.updated_rows += 1,
                    _ => {
                        report.skipped += 1;
                        continue;
                    }
                }
                let url = filter_to_server_host(
                    &resolve_image_url(&fields.cover_small_path, base_url),
                    base_url,
                );
                if !url.is_empty() {
                    match cache.ensure(Some(client), &url).await {
                        Ok(_) => report.fetched_files += 1,
                        Err(_) => report.skipped += 1,
                    }
                }
            }
            ReplenishItem::NeedsFile { url, .. } => match cache.ensure(Some(client), &url).await {
                Ok(_) => report.fetched_files += 1,
                Err(_) => report.skipped += 1,
            },
            ReplenishItem::NeedsVariant { url, .. } => {
                match ensure_background_variant(cache, Some(client), &url).await {
                    Ok(_) => report.fetched_files += 1,
                    Err(_) => report.skipped += 1,
                }
            }
        }
    }
    report
}
