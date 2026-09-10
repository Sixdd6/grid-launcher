//! Replenish (doc 07 "Replenishment of missing covers", D6): after a
//! successful connect, back-fill image fields for rows that lack them and
//! fetch missing small-cover files. Sequential, never fails; errors skip
//! the item.

use super::background::{background_variant_ext, ensure_background_variant};
use super::cache::{image_key, ImageCache};
use super::urls::{filter_to_server_host, resolve_image_url};
use super::ImageFields;
use crate::library::registry::{InstalledGame, Registry, IMAGES_VERSION};
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
    /// screenshot, else its large cover) has no variant at the configured
    /// blur (`<key>.bg<sigma>.jpg`) yet. Planned
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

/// A row whose `images_version` is below [`IMAGES_VERSION`] is planned as
/// `NeedsFields` before anything else is looked at. The stamp is the ONLY
/// test for a row written by older image rules, and it has to be: the image
/// fields are stored already resolved, so a resolver change cannot be
/// detected by inspecting the stored value. Inspecting it is also wrong —
/// a URL's shape depends on the server's base URL (a RomM behind a reverse
/// proxy at `https://host/romm` resolves to `https://host/romm/assets/...`)
/// and on what `path_cover_*`/`fanart_path` the server chose, so any literal
/// path test false-positives forever on a deployment that never matched it.
/// The stamp also covers the case nothing else can see: a `fanart_urls` that
/// is EMPTY because the rules of the day could not resolve the server's
/// fanart, which is otherwise indistinguishable from "this game has no
/// fanart".
///
/// The cost is bounded: one `rom_detail` fetch per pre-stamp row, after which
/// `update_images` writes the stamp and the row is never re-planned. A row
/// whose detail fetch fails keeps its old stamp and is retried on the next
/// replenish pass — an extra request per pass while the server is unhealthy,
/// which is acceptable against never repairing the row.
///
/// A `NeedsFields` row `continue`s, so it contributes no `NeedsVariant`: the
/// FIRST pass after the v6 migration stamps the whole library and warms no
/// background variants at all. The next connect sees every row stamped and
/// warms the top [`BACKGROUND_VARIANT_LIMIT`] as designed; until then a
/// background is built lazily the first time the shell shows it.
///
/// `sigma` is the configured background blur: the variant's file name
/// carries it, so a row whose art exists only at another sigma still needs a
/// build.
pub fn plan(
    rows: &[InstalledGame],
    cache: &ImageCache,
    base_url: &str,
    sigma: u8,
) -> Vec<ReplenishItem> {
    let ext = background_variant_ext(sigma);
    let mut items = Vec::new();
    let mut variants = Vec::new();
    for row in rows {
        let Some(rom_id) = row.rom_id else { continue };
        if row.images_version < IMAGES_VERSION
            || (row.cover_small_path.is_empty()
                && row.cover_large_path.is_empty()
                && row.screenshot_urls.is_empty())
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
                .find_with_extension(&image_key(&background), &ext)
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
    sigma: u8,
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
                match ensure_background_variant(cache, Some(client), &url, sigma).await {
                    Ok(_) => report.fetched_files += 1,
                    Err(_) => report.skipped += 1,
                }
            }
        }
    }
    report
}
