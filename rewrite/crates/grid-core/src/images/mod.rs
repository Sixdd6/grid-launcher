//! Image pipeline: URL rules (doc 07 "URL resolution rules"), the disk
//! cache, the startup sweep, and the replenish job. Spec:
//! docs/superpowers/specs/2026-09-02-covers-images-design.md.

pub mod cache;
pub mod replenish;
pub mod sweep;
pub mod urls;
pub mod video;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageVariant {
    CoverSmall,
    CoverLarge,
    Screenshot,
}

/// The registry image columns, as stored: cover paths verbatim from the
/// server (resolved lazily against the server URL), screenshots and fanart as
/// newline-joined lists of already resolved + host-filtered absolute URLs.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImageFields {
    pub cover_small_path: String,
    pub cover_large_path: String,
    pub screenshot_urls: String,
    /// Fanart URLs, newline-joined — same convention as `screenshot_urls`.
    pub fanart_urls: String,
}

impl ImageFields {
    /// Builds the registry image columns from a freshly fetched
    /// [`crate::romm::RomDetail`]: cover paths verbatim, screenshots
    /// newline-joined.
    pub fn from_detail(detail: &crate::romm::RomDetail) -> Self {
        Self {
            cover_small_path: detail.cover_small_path.clone(),
            cover_large_path: detail.cover_large_path.clone(),
            screenshot_urls: detail.screenshot_urls.join("\n"),
            fanart_urls: detail.fanart_urls.join("\n"),
        }
    }
}
