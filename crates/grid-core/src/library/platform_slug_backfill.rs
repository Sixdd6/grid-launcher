//! Fills `platform_slug` on installed rows written before registry v8 (user
//! bug 2026-10-10: a native game on a custom-named platform was treated as
//! emulated, because nativeness read only the display name).
//!
//! After a connect, the pass fetches the ROM detail for each row that has a
//! rom id and a blank slug, and stores the detail's `platform_slug`. It is
//! bounded ([`BACKFILL_LIMIT`] rows per pass; the next connect continues),
//! best-effort (a failed fetch skips that row and the pass goes on), and
//! offline-safe (the caller runs it only with a live client). Logs carry the
//! rom id and counts only — never a URL, a header or a token.

use super::registry::{InstalledGame, Registry};
use super::LibraryError;
use crate::romm::RommClient;

/// The most rows one pass fetches a detail for.
pub const BACKFILL_LIMIT: usize = 50;

/// The rom ids of the rows the pass fills, in registry order: a rom id is
/// set and the stored slug is blank. At most `limit` ids. Pure.
pub fn rows_missing_slug(rows: &[InstalledGame], limit: usize) -> Vec<i64> {
    rows.iter()
        .filter(|row| row.platform_slug.trim().is_empty())
        .filter_map(|row| row.rom_id)
        .take(limit)
        .collect()
}

/// Runs one backfill pass and returns the rom ids whose slug it stored.
/// `Err` only when the registry cannot be read; a failed detail fetch or a
/// detail with no slug skips that row.
pub async fn backfill_platform_slugs(
    registry: &Registry,
    client: &RommClient,
) -> Result<Vec<i64>, LibraryError> {
    let rows = registry.all()?;
    let mut filled = Vec::new();
    for rom_id in rows_missing_slug(&rows, BACKFILL_LIMIT) {
        let detail = match client.rom_detail(rom_id).await {
            Ok(detail) => detail,
            Err(e) => {
                tracing::debug!("platform slug backfill: rom {rom_id} skipped: {e}");
                continue;
            }
        };
        match registry.set_platform_slug(rom_id, &detail.platform_slug) {
            Ok(true) => filled.push(rom_id),
            Ok(false) => {}
            Err(e) => tracing::warn!("platform slug backfill: rom {rom_id} not stored: {e}"),
        }
    }
    if !filled.is_empty() {
        tracing::info!("platform slug backfill: filled {} row(s)", filled.len());
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str, rom_id: Option<i64>, slug: &str) -> InstalledGame {
        InstalledGame {
            title: title.to_string(),
            platform: "PC Fangames & Ports".to_string(),
            rom_id,
            platform_slug: slug.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn picks_rows_with_a_rom_id_and_a_blank_slug_up_to_the_limit() {
        let rows = vec![
            row("A", Some(1), ""),
            row("B", Some(2), "win"),
            row("C", None, ""),
            row("D", Some(4), "  "),
            row("E", Some(5), ""),
        ];
        assert_eq!(rows_missing_slug(&rows, 10), vec![1, 4, 5]);
        assert_eq!(rows_missing_slug(&rows, 2), vec![1, 4]);
        assert!(rows_missing_slug(&rows, 0).is_empty());
    }
}
