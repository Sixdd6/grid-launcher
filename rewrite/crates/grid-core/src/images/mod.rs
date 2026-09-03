//! Image pipeline: URL rules (doc 07 "URL resolution rules"), the disk
//! cache, the startup sweep, and the replenish job. Spec:
//! docs/superpowers/specs/2026-09-02-covers-images-design.md.

pub mod cache;
pub mod urls;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageVariant {
    CoverSmall,
    CoverLarge,
    Screenshot,
}

/// The three registry image columns, as stored: cover paths verbatim from
/// the server (resolved lazily against the server URL), screenshots as a
/// newline-joined list of already resolved + host-filtered absolute URLs.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImageFields {
    pub cover_small_path: String,
    pub cover_large_path: String,
    pub screenshot_urls: String,
}
