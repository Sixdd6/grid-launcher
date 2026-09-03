//! Replenish (doc 07 "Replenishment of missing covers", D6): after a
//! successful connect, back-fill image fields for rows that lack them and
//! fetch missing small-cover files. Sequential, never fails; errors skip
//! the item.

use super::cache::{image_key, ImageCache};
use super::urls::{filter_to_server_host, resolve_image_url};
use super::ImageFields;
use crate::library::registry::{InstalledGame, Registry};
use crate::romm::RommClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplenishItem {
    NeedsFields { rom_id: i64 },
    NeedsFile { rom_id: i64, url: String },
}

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

pub fn plan(rows: &[InstalledGame], cache: &ImageCache, base_url: &str) -> Vec<ReplenishItem> {
    let mut items = Vec::new();
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
    }
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
        }
    }
    report
}
