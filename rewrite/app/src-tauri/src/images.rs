//! App-layer glue for grid-core's `images` module: the startup sweep, the
//! one-at-a-time replenish job with its `images-replenished` event, and the
//! post-install cover prefetch.

use grid_core::config::Config;
use grid_core::images::background::{BACKGROUND_BLUR_DEFAULT, BACKGROUND_BLUR_MAX};
use grid_core::images::cache::ImageCache;
use grid_core::images::replenish::{self, background_source_url, ReplenishReport};
use grid_core::images::sweep::{pinned_keys, sweep, SweepReport, IMAGE_CACHE_CAP_BYTES};
use grid_core::images::urls::{filter_to_server_host, resolve_image_url};
use grid_core::images::ImageFields;
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::session::SessionManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub const REPLENISHED_EVENT: &str = "images-replenished";

pub struct ImageService {
    replenish_running: AtomicBool,
}

impl ImageService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            replenish_running: AtomicBool::new(false),
        })
    }

    pub fn try_begin_replenish(&self) -> bool {
        self.replenish_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn end_replenish(&self) {
        self.replenish_running.store(false, Ordering::Release);
    }

    /// R3: synchronous, called from Tauri `setup` before any command runs.
    pub fn sweep_at_startup(
        cache: &ImageCache,
        rows: &[InstalledGame],
        base_url: &str,
    ) -> SweepReport {
        Self::sweep_at_startup_with_cap(cache, rows, base_url, IMAGE_CACHE_CAP_BYTES)
    }

    /// [`sweep_at_startup`](Self::sweep_at_startup) with the cap as an
    /// argument, so a test can force an eviction without writing a gigabyte.
    pub fn sweep_at_startup_with_cap(
        cache: &ImageCache,
        rows: &[InstalledGame],
        base_url: &str,
        cap_bytes: u64,
    ) -> SweepReport {
        // Both covers AND the row's background source: a fanart- or
        // screenshot-sourced variant lives under its OWN key, so pinning only
        // the covers would evict it and its source on every start above the
        // cap, and the next hover would download and blur them again.
        let backgrounds: Vec<String> = rows
            .iter()
            .map(|r| background_source_url(r, base_url))
            .collect();
        let paths = rows
            .iter()
            .flat_map(|r| [r.cover_small_path.as_str(), r.cover_large_path.as_str()])
            .chain(backgrounds.iter().map(String::as_str));
        let pinned = pinned_keys(paths, base_url);
        let report = sweep(cache.dir(), cap_bytes, &pinned);
        if report.deleted > 0 || report.stale_parts > 0 {
            tracing::info!(
                "image cache sweep: {} -> {} bytes, {} deleted, {} stale parts",
                report.total_before,
                report.total_after,
                report.deleted,
                report.stale_parts
            );
        }
        report
    }

    /// One job at a time (Python's `isRunning()` guard): a trigger while a
    /// job runs is dropped. Emits `images-replenished` when done, even with
    /// nothing to do, so the UI can clear any busy state.
    ///
    /// The guard releases `replenish_running` on every exit path, including
    /// a panic unwinding out of `replenish_once` — otherwise a poisoned lock
    /// or any other panic in there would leave the flag stuck `true` forever
    /// and drop every later trigger.
    pub fn spawn_replenish(
        self: &Arc<Self>,
        app: AppHandle,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
    ) {
        if !self.try_begin_replenish() {
            return;
        }
        let svc = self.clone();
        tauri::async_runtime::spawn(async move {
            let guard = ReplenishGuard(svc);
            let report = replenish_once(&session, &install).await;
            drop(guard);
            let _ = app.emit(REPLENISHED_EVENT, report);
        });
    }

    /// Post-install (D5): fetch the small and large covers without blocking
    /// the install. Errors are ignored — the Library's own load and the next
    /// replenish are the fallbacks.
    pub fn spawn_prefetch(session: Arc<SessionManager>, fields: ImageFields) {
        tauri::async_runtime::spawn(async move {
            let base = session.server_url();
            let Some(client) = session.client() else {
                return;
            };
            let mut background_source = String::new();
            for path in [&fields.cover_small_path, &fields.cover_large_path] {
                let url = filter_to_server_host(&resolve_image_url(path, &base), &base);
                if !url.is_empty() {
                    let _ = session.cache().ensure(Some(&client), &url).await;
                    background_source = url;
                }
            }
            // The background art's own source, preferred in the same order
            // `backgroundUrls` uses on the frontend: fanart, then the first
            // screenshot, then the large cover (already set above, because it
            // is the last URL the loop visits).
            for stored in [&fields.fanart_urls, &fields.screenshot_urls] {
                if let Some(first) = stored.lines().map(str::trim).find(|u| !u.is_empty()) {
                    let url = filter_to_server_host(&resolve_image_url(first, &base), &base);
                    if !url.is_empty() {
                        background_source = url;
                        break;
                    }
                }
            }
            if !background_source.is_empty() {
                // Read only now that there is art to build: an install with
                // no background source must not pay a `config.toml` read, and
                // it is a blocking one, so it goes off the runtime thread.
                let sigma = tokio::task::spawn_blocking(background_blur)
                    .await
                    .unwrap_or(BACKGROUND_BLUR_DEFAULT);
                // Built here so the first time this game becomes the
                // background there is nothing to wait for.
                let _ = grid_core::images::background::ensure_background_variant(
                    session.cache(),
                    Some(&client),
                    &background_source,
                    sigma,
                )
                .await;
            }
        });
    }
}

/// Releases [`ImageService::replenish_running`] when dropped — on the normal
/// completion path (dropped explicitly before the event emit) and on a panic
/// unwinding out of the spawned task alike.
struct ReplenishGuard(Arc<ImageService>);

impl Drop for ReplenishGuard {
    fn drop(&mut self) {
        self.0.end_replenish();
    }
}

async fn replenish_once(session: &SessionManager, install: &InstallService) -> ReplenishReport {
    let Some(client) = session.client() else {
        return ReplenishReport::default();
    };
    let base = session.server_url();
    let registry = install.registry();
    // The rows and the blur sigma are both blocking reads (SQLite, then
    // `config.toml`), so they share the one `spawn_blocking` hop.
    let (rows, sigma) = {
        let registry = registry.clone();
        tokio::task::spawn_blocking(move || (registry.all().unwrap_or_default(), background_blur()))
            .await
            .unwrap_or_else(|_| (Vec::new(), BACKGROUND_BLUR_DEFAULT))
    };
    let items = replenish::plan(&rows, session.cache(), &base, sigma);
    replenish::run(&client, session.cache(), &registry, &base, items, sigma).await
}

/// The configured background blur sigma, or the default when the config will
/// not load. A background image is not worth failing a replenish or a
/// post-install prefetch over.
///
/// Clamped here as well as in `normalize_ui_settings`: `Config::load` does no
/// clamping, so a hand-edited `background_blur = 200` would otherwise reach
/// the builder through this path and name a variant the command path can
/// never ask for again.
///
/// Blocking (`std::fs`): call it from a blocking context.
fn background_blur() -> u8 {
    Config::load(&Config::default_path())
        .map(|c| c.ui.background_blur)
        .unwrap_or(BACKGROUND_BLUR_DEFAULT)
        .min(BACKGROUND_BLUR_MAX)
}

#[cfg(test)]
mod tests {
    use super::{ImageService, ReplenishGuard};
    use grid_core::images::cache::{image_key, ImageCache};
    use grid_core::library::registry::InstalledGame;

    fn write(dir: &std::path::Path, name: &str, size: usize) {
        std::fs::write(dir.join(name), vec![0u8; size]).unwrap();
    }

    /// A fanart-sourced variant lives under the FANART's key, so pinning only
    /// the cover keys would evict it and its source on every start above the
    /// cap — and the next hover would download and blur them again.
    #[test]
    fn the_startup_sweep_pins_a_fanart_sourced_background_and_its_variant() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::new(dir.path().to_path_buf());
        let base = "https://h";
        let row = InstalledGame {
            title: "G".to_string(),
            platform: "SNES".to_string(),
            rom_id: Some(1),
            cover_small_path: "/assets/1.png".to_string(),
            cover_large_path: "/assets/1l.png".to_string(),
            fanart_urls: "https://h/assets/1f.png".to_string(),
            ..Default::default()
        };
        let fanart = image_key("https://h/assets/1f.png");
        write(dir.path(), &format!("{fanart}.png"), 4096);
        write(dir.path(), &format!("{fanart}.bg12.jpg"), 4096);
        write(dir.path(), "loose.png", 8192);

        let report = ImageService::sweep_at_startup_with_cap(&cache, &[row], base, 8192);

        assert!(dir.path().join(format!("{fanart}.png")).exists());
        assert!(dir.path().join(format!("{fanart}.bg12.jpg")).exists());
        assert!(!dir.path().join("loose.png").exists());
        assert_eq!(report.deleted, 1);
    }

    #[test]
    fn only_one_replenish_runs_at_a_time() {
        let svc = ImageService::new();
        assert!(svc.try_begin_replenish());
        assert!(!svc.try_begin_replenish());
        svc.end_replenish();
        assert!(svc.try_begin_replenish());
    }

    /// Proves the drop guard itself releases the flag — the mechanism
    /// `spawn_replenish` relies on to survive a panic unwinding out of
    /// `replenish_once` (see its doc comment).
    #[test]
    fn replenish_guard_releases_on_drop() {
        let svc = ImageService::new();
        assert!(svc.try_begin_replenish());
        let guard = ReplenishGuard(svc.clone());
        drop(guard);
        assert!(svc.try_begin_replenish());
    }
}
