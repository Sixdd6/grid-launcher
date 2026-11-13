//! GOG games on backup drives through `InstallService`: a tempdir library
//! with GOG rows in the registry, and a tempdir "drive" holding a set-up
//! `gridpack/`. No server and no store tool: a store backup needs neither,
//! and the installer step of a restore is a fake (`InstallerRun`) until the
//! offline installers (package B) land.
//!
//! Every wait is bounded, so a broken queue fails the test instead of
//! hanging it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::backup::drives::drive_key;
use grid_core::backup::local::REASON_STORE;
use grid_core::backup::pack::{self, Pack, PackOs, PackState, PackStoreGame, StoreLayout};
use grid_core::backup::store::{installing_refusal, running_refusal};
use grid_core::library::backup_job::DriveTarget;
use grid_core::library::drive_restore::{store_restore_job, InstallerRun, StoreRestoreCtx};
use grid_core::library::queue::{
    DownloadEntry, DownloadStatus, JobPhase, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink,
};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::{InstallService, LibraryError};
use grid_core::stores::gog::goggame::parse_info;
use grid_core::stores::gog::meta::{GogMeta, GogOrigin, SaveLocation};
use grid_core::stores::gog::row::gog_row;
use grid_core::stores::{GameKey, StoreKind};

const LABEL: &str = "SAMSUNG T7";
const BUDGET: Duration = Duration::from_secs(30);
const ID: &str = "1207664643";

fn info_json(id: &str, name: &str) -> String {
    format!(
        r#"{{"gameId": "{id}", "rootGameId": "{id}", "name": "{name}", "buildId": "7",
            "playTasks": [{{"category": "game", "isPrimary": true, "path": "bin/start.sh",
                            "type": "FileTask"}}]}}"#
    )
}

struct Harness {
    tmp: tempfile::TempDir,
    library: PathBuf,
    mount: PathBuf,
    registry: Arc<Registry>,
    service: Arc<InstallService>,
}

impl Harness {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        fs::create_dir_all(&library).unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(
            &config_path,
            format!(
                "schema_version = 1\nlibrary_path = {:?}\ndefault_compat_tool = \"GE-Proton10-17\"\n",
                library.to_string_lossy()
            ),
        )
        .unwrap();
        let mount = tmp.path().join("media").join(LABEL);
        fs::create_dir_all(&mount).unwrap();
        pack::write_pack_atomic(&mount, &pack::new_pack("Couch kit", &[PackOs::Linux])).unwrap();
        let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
        let service = InstallService::new(registry.clone(), config_path);
        Harness {
            tmp,
            library,
            mount,
            registry,
            service,
        }
    }

    fn drive(&self) -> DriveTarget {
        DriveTarget {
            key: drive_key(&self.mount),
            mount: self.mount.clone(),
            label: LABEL.to_string(),
            name: "Couch kit".to_string(),
            fat32: false,
        }
    }

    fn pack(&self) -> Pack {
        match pack::read_pack(&self.mount).unwrap() {
            PackState::Ready(pack) => pack,
            other => panic!("pack not ready: {other:?}"),
        }
    }

    fn install_dir(&self, name: &str) -> PathBuf {
        self.library.join("GOG").join(name)
    }

    /// Installs a GOG game on "this computer": its folder (with its
    /// `goggame-<id>.info`, a launcher and a data file) and its row.
    fn install_game(&self, name: &str, meta: &GogMeta, prefix: Option<&Path>) -> InstalledGame {
        let dir = self.install_dir(name);
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("data")).unwrap();
        fs::write(dir.join(format!("goggame-{ID}.info")), info_json(ID, name)).unwrap();
        fs::write(dir.join("bin/start.sh"), b"#!/bin/sh\necho hi\n").unwrap();
        fs::write(dir.join("data/level1.pak"), vec![7u8; 5000]).unwrap();
        let info = parse_info(info_json(ID, name).as_bytes()).unwrap();
        let mut row = gog_row(&info, &dir, meta, None, prefix);
        row.description = "A tide.".into();
        row.genres = "Adventure, Puzzle".into();
        self.registry.upsert_store(&row).unwrap();
        row
    }

    fn rows(&self) -> Vec<DownloadEntry> {
        self.service.snapshot().entries
    }

    fn row(&self, kind: &str) -> Option<DownloadEntry> {
        let key = key().to_string();
        self.rows()
            .into_iter()
            .find(|e| e.kind == kind && e.source_id == key)
    }

    async fn wait_terminal(&self, kind: &str) -> DownloadEntry {
        let deadline = Instant::now() + BUDGET;
        loop {
            if let Some(row) = self.row(kind).filter(|row| {
                matches!(
                    row.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                )
            }) {
                return row;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {kind}; rows: {:#?}",
                self.rows()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn plan(&self) -> grid_core::backup::plan::StorePlanned {
        self.service
            .plan_store_backup(&self.drive(), &self.pack(), &[key()], 1 << 40, &|_| None)
    }

    async fn back_up(&self) -> DownloadEntry {
        let planned = self.plan();
        assert_eq!(planned.items.len(), 1, "{:#?}", planned.plan);
        let queued = self
            .service
            .start_store_backup(self.drive(), planned.items, None)
            .unwrap();
        assert_eq!(queued.games, 1);
        self.wait_terminal("backup_store_game").await
    }
}

fn key() -> GameKey {
    GameKey::store(StoreKind::Gog, ID)
}

/// Every regular file under `dir`, relative, `/`-separated, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap();
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(dir, dir, &mut out);
    }
    out.sort();
    out
}

struct NoSink;

impl StoreProgressSink for NoSink {
    fn progress(&mut self, _done: u64, _total: u64) {}
    fn phase(&mut self, _phase: JobPhase) {}
}

#[tokio::test]
async fn an_installed_copy_goes_to_the_drive_and_comes_back() {
    let h = Harness::new();
    let meta = GogMeta {
        origin: GogOrigin::Gogdl,
        version: "1.1.0".into(),
        ..GogMeta::default()
    };
    h.install_game("Ashen Tide", &meta, None);

    let planned = h.plan();
    let row = &planned.plan.rows[0];
    assert_eq!(row.layout, StoreLayout::Installed);
    assert!(
        row.detail.starts_with(
            "Installed with gogdl, which keeps no installer, so the installed files are copied · "
        ),
        "{}",
        row.detail
    );
    assert!(row.bytes > 5000);
    assert!(!row.already && row.refused.is_empty() && !row.windows_prefix_note);
    assert!(planned.plan.fits);

    let done = h.back_up().await;
    assert_eq!(done.status, DownloadStatus::Completed, "{done:#?}");
    assert_eq!(done.platform, "GOG");
    assert_eq!(done.title, "Ashen Tide");

    let pack = h.pack();
    let game = pack.find_store_game(&key()).unwrap().clone();
    assert_eq!(game.layout, StoreLayout::Installed);
    assert_eq!(game.version, "1.1.0");
    assert_eq!(game.genres, vec!["Adventure", "Puzzle"]);
    let copy = game.installed.as_ref().unwrap();
    assert_eq!(copy.tree_files, 3);
    assert_eq!(
        copy.parts[0].path,
        format!("stores/gog/Ashen Tide-{ID}/installed/installed.tar.001")
    );
    assert!(pack::resolve(&h.mount, &copy.parts[0].path)
        .unwrap()
        .is_file());
    assert_eq!(pack.pack_bytes(), game.size);

    // Planned again, it is skipped.
    let again = h.plan();
    assert!(again.plan.rows[0].already);
    assert!(again.items.is_empty());

    // Uninstalled here, then restored from the drive.
    h.registry.remove_key(&key()).unwrap();
    fs::remove_dir_all(h.install_dir("Ashen Tide")).unwrap();
    let id = h
        .service
        .install_store_from_drive(&h.drive(), &game)
        .unwrap();
    assert!(id.is_some());
    let restored = h.wait_terminal("store_restore").await;
    assert_eq!(restored.status, DownloadStatus::Completed, "{restored:#?}");
    assert_eq!(restored.drive_label, LABEL);

    let row = h.registry.find_key(&key()).unwrap().expect("restored row");
    let dir = h.install_dir("Ashen Tide");
    assert_eq!(Path::new(&row.native_game_dir), dir);
    assert_eq!(
        Path::new(&row.native_executable_path),
        dir.join("bin").join("start.sh")
    );
    let meta = GogMeta::from_json(&row.store_meta);
    assert_eq!(meta.origin, GogOrigin::Drive);
    assert!(meta.installer.is_empty());
    assert_eq!(meta.version, "1.1.0");
    assert_eq!(
        files_under(&dir),
        vec![
            "bin/start.sh".to_string(),
            "data/level1.pak".into(),
            format!("goggame-{ID}.info"),
        ]
    );
    assert_eq!(
        fs::read(dir.join("data/level1.pak")).unwrap(),
        vec![7u8; 5000]
    );
    // Nothing is left beside it.
    assert_eq!(
        fs::read_dir(h.library.join("GOG")).unwrap().count(),
        1,
        "leftovers in the GOG folder"
    );

    // A second restore is refused: it is installed.
    let error = h
        .service
        .install_store_from_drive(&h.drive(), &game)
        .unwrap_err();
    assert_eq!(error.to_string(), "Ashen Tide is already installed");
}

/// The installer step of a restore, until package B's `run_into` lands:
/// checks the staged installer's bytes and lays the game down.
fn fake_installer(expected: Vec<u8>, ran: Arc<AtomicBool>) -> InstallerRun {
    Arc::new(move |staged, dest, prefix, _compat, _wizard, sink| {
        assert_eq!(fs::read(staged).unwrap(), expected, "the staged installer");
        assert!(prefix.is_none(), "a Linux game has no prefix");
        sink.phase(JobPhase::RunningInstaller);
        fs::create_dir_all(dest.join("bin")).unwrap();
        fs::write(
            dest.join(format!("goggame-{ID}.info")),
            info_json(ID, "Brightwater"),
        )
        .unwrap();
        fs::write(dest.join("bin/start.sh"), b"#!/bin/sh\n").unwrap();
        ran.store(true, Ordering::SeqCst);
        Ok(parse_info(info_json(ID, "Brightwater").as_bytes()).unwrap())
    })
}

#[tokio::test]
async fn installer_files_go_to_the_drive_with_their_saves_and_run_again() {
    let h = Harness::new();
    let downloads = h.tmp.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    let installer = downloads.join("brightwater_1_3_58810.sh");
    let bytes: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    fs::write(&installer, &bytes).unwrap();
    let meta = GogMeta {
        origin: GogOrigin::Offline,
        version: "1.3".into(),
        installer: vec![installer.to_string_lossy().into_owned()],
        installer_bytes: bytes.len() as u64,
        save_locations: vec![SaveLocation {
            name: "saves".into(),
            path: "<?INSTALL?>/saves".into(),
        }],
        ..GogMeta::default()
    };
    h.install_game("Brightwater", &meta, None);
    let dir = h.install_dir("Brightwater");
    fs::create_dir_all(dir.join("saves")).unwrap();
    fs::write(dir.join("saves/slot1.sav"), b"level 9").unwrap();

    // Moved away, the installer can't be used: the installed copy goes.
    let moved = downloads.join("elsewhere.sh");
    fs::rename(&installer, &moved).unwrap();
    let row = h.plan().plan.rows.remove(0);
    assert_eq!(row.layout, StoreLayout::Installed);
    assert!(
        row.detail.starts_with("Its installer isn’t at "),
        "{}",
        row.detail
    );
    fs::rename(&moved, &installer).unwrap();

    let row = h.plan().plan.rows.remove(0);
    assert_eq!(row.layout, StoreLayout::Installer);
    assert!(
        row.detail.starts_with("brightwater_1_3_58810.sh · ")
            && row.detail.ends_with(", and its saves"),
        "{}",
        row.detail
    );

    let done = h.back_up().await;
    assert_eq!(done.status, DownloadStatus::Completed, "{done:#?}");
    let game = h.pack().find_store_game(&key()).unwrap().clone();
    assert_eq!(game.layout, StoreLayout::Installer);
    assert!(game.installed.is_none());
    assert_eq!(
        game.installer[0].path,
        format!("stores/gog/Brightwater-{ID}/installer/brightwater_1_3_58810.sh")
    );
    assert_eq!(game.size, bytes.len() as u64);
    let on_drive = pack::resolve(&h.mount, &game.installer[0].path).unwrap();
    assert_eq!(fs::read(on_drive).unwrap(), bytes);
    let saves = game.saves.as_ref().unwrap();
    assert_eq!(saves.folders, vec!["<?INSTALL?>\\saves".to_string()]);
    assert!(saves.has_copy());
    assert_eq!(
        saves.parts[0].path,
        format!("stores/gog/Brightwater-{ID}/saves/saves.tar.001")
    );

    // Restore: the installer is copied off the drive and run again.
    h.registry.remove_key(&key()).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let ctx = StoreRestoreCtx {
        registry: h.registry.clone(),
        install_root: h.library.join("GOG"),
        staging_root: h.library.join(".grid-staging"),
        compat_tool: String::new(),
        wizard_allowed: Arc::new(|| false),
        free: None,
    };
    let job: StoreJob =
        store_restore_job(&h.drive(), &game, ctx, fake_installer(bytes, ran.clone())).unwrap();
    assert_eq!(job.op, StoreOp::Restore);
    assert_eq!(job.drive.as_ref().unwrap().label, LABEL);
    job.runner
        .download(&AtomicBool::new(false), &mut |_, _, _| {})
        .unwrap();
    let warning = job.runner.finalize(&mut NoSink).unwrap();
    assert!(ran.load(Ordering::SeqCst));
    assert_eq!(warning, "");

    let row = h.registry.find_key(&key()).unwrap().expect("restored row");
    let meta = GogMeta::from_json(&row.store_meta);
    assert_eq!(meta.origin, GogOrigin::Offline);
    assert!(meta.installer.is_empty(), "the staged copy is gone");
    assert_eq!(meta.installer_bytes, 0);
    assert_eq!(fs::read(dir.join("saves/slot1.sav")).unwrap(), b"level 9");
    assert!(!h.library.join(".grid-staging").exists(), "staging removed");
}

#[cfg(unix)]
#[tokio::test]
async fn a_windows_game_is_copied_without_its_prefix_but_with_its_saves() {
    let h = Harness::new();
    let dir = h.install_dir("Lanternfall");
    let prefix = dir.join("prefix");
    let roaming = prefix.join("drive_c/users/steamuser/AppData/Roaming/Lanternfall");
    fs::create_dir_all(&roaming).unwrap();
    fs::write(roaming.join("slot.sav"), b"chapter 3").unwrap();
    fs::create_dir_all(prefix.join("drive_c/windows/system32")).unwrap();
    fs::write(
        prefix.join("drive_c/windows/system32/big.dll"),
        vec![1u8; 9000],
    )
    .unwrap();
    let meta = GogMeta {
        platform: grid_core::stores::gog::meta::GogPlatform::Windows,
        save_locations: vec![SaveLocation {
            name: "saves".into(),
            path: "<?APPLICATION_DATA_ROAMING?>/Lanternfall".into(),
        }],
        ..GogMeta::default()
    };
    let mut row = h.install_game("Lanternfall", &meta, Some(&prefix));
    row.native_compat_tool = "GE-Proton10-17".into();
    h.registry.upsert_store(&row).unwrap();

    let planned = h.plan();
    assert!(planned.plan.rows[0].windows_prefix_note);
    let done = h.back_up().await;
    assert_eq!(done.status, DownloadStatus::Completed, "{done:#?}");
    let game = h.pack().find_store_game(&key()).unwrap().clone();
    assert_eq!(
        game.platform,
        grid_core::stores::gog::meta::GogPlatform::Windows
    );
    assert_eq!(game.installed.as_ref().unwrap().tree_files, 3, "no prefix");
    let saves = game.saves.as_ref().unwrap();
    assert_eq!(saves.folders, vec!["%APPDATA%\\Lanternfall".to_string()]);
    assert_eq!(saves.tree_files, 1);

    h.registry.remove_key(&key()).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    h.service
        .install_store_from_drive(&h.drive(), &game)
        .unwrap();
    let restored = h.wait_terminal("store_restore").await;
    assert_eq!(restored.status, DownloadStatus::Completed, "{restored:#?}");
    let row = h.registry.find_key(&key()).unwrap().unwrap();
    assert_eq!(Path::new(&row.native_wineprefix), prefix);
    assert_eq!(row.platform_slug, "win");
    let files = files_under(&dir);
    assert!(
        files.iter().all(|file| !file.ends_with("big.dll")),
        "the prefix was copied: {files:?}"
    );
    let save = files
        .iter()
        .find(|file| file.ends_with("AppData/Roaming/Lanternfall/slot.sav"))
        .unwrap_or_else(|| panic!("saves not restored: {files:?}"));
    assert_eq!(fs::read(dir.join(save)).unwrap(), b"chapter 3");
}

/// A store job that holds its download until released: "being installed".
struct HeldRunner(Arc<AtomicBool>);

impl StoreJobRunner for HeldRunner {
    fn download(
        &self,
        cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        let deadline = Instant::now() + BUDGET;
        while !self.0.load(Ordering::SeqCst) && !cancel.load(Ordering::Relaxed) {
            if Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn finalize(&self, _sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        Ok(String::new())
    }
}

#[tokio::test]
async fn running_installing_and_other_stores_are_refused() {
    let h = Harness::new();
    h.install_game("Copper Saints", &GogMeta::default(), None);
    let epic = GameKey::store(StoreKind::Epic, "fn");
    let missing = GameKey::store(StoreKind::Gog, "999");
    let planned = h.service.plan_store_backup(
        &h.drive(),
        &h.pack(),
        &[epic, missing, key()],
        1 << 40,
        &|_| None,
    );
    let rows = &planned.plan.rows;
    assert_eq!(rows[0].refused, REASON_STORE);
    assert_eq!(
        rows[1].refused,
        "999 is no longer installed on this computer; backup again"
    );
    assert!(rows[2].refused.is_empty());
    assert_eq!(planned.items.len(), 1);

    // Being installed or updated.
    let release = Arc::new(AtomicBool::new(false));
    h.service
        .enqueue_store_job(StoreJob {
            source: StoreKind::Gog,
            store_id: ID.into(),
            op: StoreOp::Update,
            title: "Copper Saints".into(),
            drive: None,
            runner: Arc::new(HeldRunner(release.clone())),
        })
        .unwrap();
    assert!(h.service.store_job_live(&key()));
    let row = h.plan().plan.rows.remove(0);
    assert_eq!(row.refused, installing_refusal("Copper Saints"));
    assert_eq!(
        row.refused,
        "Copper Saints is being installed or updated. Wait for it to finish."
    );
    release.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + BUDGET;
    while h.service.store_job_live(&key()) {
        assert!(Instant::now() < deadline, "the update never finished");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Running.
    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    h.service
        .set_store_running_check(Some(Arc::new(move |k: &GameKey| {
            flag.load(Ordering::SeqCst) && *k == key()
        })));
    let planned = h.plan();
    assert_eq!(
        planned.plan.rows[0].refused,
        "Copper Saints is running right now; close it to copy it."
    );
    assert!(planned.items.is_empty());

    // Launched after it was queued: the row fails with the same reason.
    running.store(false, Ordering::SeqCst);
    let planned = h.plan();
    running.store(true, Ordering::SeqCst);
    h.service
        .start_store_backup(h.drive(), planned.items, None)
        .unwrap();
    let failed = h.wait_terminal("backup_store_game").await;
    assert_eq!(failed.status, DownloadStatus::Failed);
    assert_eq!(failed.error, running_refusal("Copper Saints"));
    assert!(h.pack().store_games.is_empty());
}

/// The index an older build writes back keeps `store_games` (it reads it
/// into its `extra`), so a GOG backup survives that build's next write.
#[tokio::test]
async fn an_older_build_keeps_the_store_games() {
    let h = Harness::new();
    h.install_game("Paper Lanterns", &GogMeta::default(), None);
    let done = h.back_up().await;
    assert_eq!(done.status, DownloadStatus::Completed, "{done:#?}");

    #[derive(serde::Serialize, serde::Deserialize)]
    struct OlderPack {
        format: u32,
        name: String,
        os: Vec<PackOs>,
        created: String,
        updated: String,
        #[serde(default)]
        games: Vec<serde_json::Value>,
        #[serde(flatten)]
        extra: BTreeMap<String, serde_json::Value>,
    }
    let index = pack::index_path(&h.mount);
    let older: OlderPack = serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
    assert!(older.extra.contains_key("store_games"));
    fs::write(&index, serde_json::to_vec_pretty(&older).unwrap()).unwrap();
    let game: &PackStoreGame = &h.pack().store_games[0];
    assert_eq!(game.name, "Paper Lanterns");
    assert!(game.installed.is_some());
}
