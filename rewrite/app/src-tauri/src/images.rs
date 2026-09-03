//! App-layer glue for grid-core's `images` module: the startup sweep, the
//! one-at-a-time replenish job with its `images-replenished` event, and the
//! post-install cover prefetch.

use grid_core::images::cache::ImageCache;
use grid_core::images::replenish::{self, ReplenishReport};
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
        let paths = rows
            .iter()
            .flat_map(|r| [r.cover_small_path.as_str(), r.cover_large_path.as_str()]);
        let pinned = pinned_keys(paths, base_url);
        let report = sweep(cache.dir(), IMAGE_CACHE_CAP_BYTES, &pinned);
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
            let report = replenish_once(&session, &install).await;
            svc.end_replenish();
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
            for path in [&fields.cover_small_path, &fields.cover_large_path] {
                let url = filter_to_server_host(&resolve_image_url(path, &base), &base);
                if !url.is_empty() {
                    let _ = session.cache().ensure(Some(&client), &url).await;
                }
            }
        });
    }
}

async fn replenish_once(session: &SessionManager, install: &InstallService) -> ReplenishReport {
    let Some(client) = session.client() else {
        return ReplenishReport::default();
    };
    let base = session.server_url();
    let registry = install.registry();
    let rows = {
        let registry = registry.clone();
        tokio::task::spawn_blocking(move || registry.all())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
    };
    let items = replenish::plan(&rows, session.cache(), &base);
    replenish::run(&client, session.cache(), &registry, &base, items).await
}

#[cfg(test)]
mod tests {
    use super::ImageService;
    #[test]
    fn only_one_replenish_runs_at_a_time() {
        let svc = ImageService::new();
        assert!(svc.try_begin_replenish());
        assert!(!svc.try_begin_replenish());
        svc.end_replenish();
        assert!(svc.try_begin_replenish());
    }
}
