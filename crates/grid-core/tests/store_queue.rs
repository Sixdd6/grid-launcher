//! Store jobs (`JobKey::Store` / `JobPayload::Store`) run through the
//! install queue's slots, progress, cancel and retry machinery. The runner
//! is a fake: no store tool is involved (phase 0 scaffolding).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use grid_core::library::queue::{
    DownloadStatus, DownloadsSnapshot, StoreJob, StoreJobRunner, StoreOp,
};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::{InstallService, LibraryError};
use grid_core::stores::{GameKey, GameSource, StoreKind};

const BUDGET: Duration = Duration::from_secs(10);

/// A runner that reports download progress, optionally waits for a cancel
/// on its first run, and writes the store row on finalize.
struct FakeRunner {
    registry: Arc<Registry>,
    block_first_download: bool,
    downloads: AtomicUsize,
    finalized: AtomicBool,
}

impl StoreJobRunner for FakeRunner {
    fn download(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        let run = self.downloads.fetch_add(1, Ordering::SeqCst);
        progress(50, 100, 10.0);
        if self.block_first_download && run == 0 {
            let deadline = Instant::now() + BUDGET;
            while !cancel.load(Ordering::Relaxed) {
                if Instant::now() > deadline {
                    return Err(LibraryError::Extract("never cancelled".into()));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            return Err(LibraryError::Cancelled);
        }
        progress(100, 100, 10.0);
        Ok(())
    }

    fn finalize(&self, progress: &mut dyn FnMut(u64, u64)) -> Result<String, LibraryError> {
        progress(1, 2);
        self.registry.upsert_store(&InstalledGame {
            title: "The Witcher".into(),
            platform: StoreKind::Gog.platform_name().into(),
            source: GameSource::Store(StoreKind::Gog),
            store_id: "1207664643".into(),
            installed_at: 1,
            ..Default::default()
        })?;
        progress(2, 2);
        self.finalized.store(true, Ordering::SeqCst);
        Ok(String::new())
    }
}

struct Harness {
    _dir: tempfile::TempDir,
    registry: Arc<Registry>,
    service: Arc<InstallService>,
    snapshots: Arc<Mutex<Vec<DownloadsSnapshot>>>,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "schema_version = 1\n").unwrap();
    let registry = Arc::new(Registry::open(&dir.path().join("registry.db")).unwrap());
    let service = InstallService::new(registry.clone(), config_path);
    let snapshots: Arc<Mutex<Vec<DownloadsSnapshot>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = snapshots.clone();
    service.set_notify(Arc::new(move |snap| sink.lock().unwrap().push(snap)));
    Harness {
        _dir: dir,
        registry,
        service,
        snapshots,
    }
}

fn job(runner: Arc<FakeRunner>) -> StoreJob {
    StoreJob {
        source: StoreKind::Gog,
        store_id: "1207664643".into(),
        op: StoreOp::Install,
        runner,
    }
}

async fn wait_for_status(service: &InstallService, id: u64, status: DownloadStatus) -> bool {
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if service
            .snapshot()
            .entries
            .iter()
            .any(|e| e.id == id && e.status == status)
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_job_downloads_finalizes_and_completes() {
    let h = harness();
    let runner = Arc::new(FakeRunner {
        registry: h.registry.clone(),
        block_first_download: false,
        downloads: AtomicUsize::new(0),
        finalized: AtomicBool::new(false),
    });
    let id = h
        .service
        .enqueue_store_job(job(runner.clone()), "The Witcher")
        .unwrap()
        .expect("admitted");
    assert!(
        wait_for_status(&h.service, id, DownloadStatus::Completed).await,
        "the store job never completed: {:?}",
        h.service.snapshot()
    );
    assert!(runner.finalized.load(Ordering::SeqCst));
    let entry = h
        .service
        .snapshot()
        .entries
        .into_iter()
        .find(|e| e.id == id)
        .unwrap();
    assert_eq!(entry.job, "store");
    assert_eq!(entry.kind, "store_install");
    assert_eq!(entry.platform, "GOG");
    assert_eq!(entry.source_id, "gog:1207664643");
    assert_eq!(entry.error, "");
    // Progress reached the entry (the listener is throttled, so the entry
    // itself is the record), and the listener heard about the row.
    assert_eq!((entry.downloaded_bytes, entry.total_bytes), (100, 100));
    assert!(h
        .snapshots
        .lock()
        .unwrap()
        .iter()
        .flat_map(|s| s.entries.iter())
        .any(|e| e.id == id));
    assert!(h
        .registry
        .find_key(&GameKey::store(StoreKind::Gog, "1207664643"))
        .unwrap()
        .is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_job_cancels_and_retries_with_its_runner() {
    let h = harness();
    let runner = Arc::new(FakeRunner {
        registry: h.registry.clone(),
        block_first_download: true,
        downloads: AtomicUsize::new(0),
        finalized: AtomicBool::new(false),
    });
    let id = h
        .service
        .enqueue_store_job(job(runner.clone()), "The Witcher")
        .unwrap()
        .expect("admitted");
    // A second admission of the same product and op is a duplicate.
    assert_eq!(
        h.service
            .enqueue_store_job(job(runner.clone()), "The Witcher")
            .unwrap(),
        None
    );
    assert!(wait_for_status(&h.service, id, DownloadStatus::Downloading).await);
    h.service.cancel(id);
    assert!(
        wait_for_status(&h.service, id, DownloadStatus::Cancelled).await,
        "the store job never cancelled: {:?}",
        h.service.snapshot()
    );
    assert!(!runner.finalized.load(Ordering::SeqCst));

    h.service.retry(None, id).await.unwrap();
    let retried = h
        .service
        .snapshot()
        .entries
        .into_iter()
        .find(|e| e.job == "store")
        .expect("a new store row")
        .id;
    assert_ne!(retried, id, "the old row was dismissed");
    assert!(wait_for_status(&h.service, retried, DownloadStatus::Completed).await);
    assert!(runner.finalized.load(Ordering::SeqCst));
    assert_eq!(runner.downloads.load(Ordering::SeqCst), 2);
}
