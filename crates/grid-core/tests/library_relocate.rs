//! End-to-end tests for the library move job (`library::relocate`): a real
//! current-layout library in a temp dir, moved to a second temp folder with
//! the strategy forced to Rename or Copy, interrupted after every journal
//! write and resumed, cancelled, and checked file by file.
//!
//! Every path lives under one `tempfile::TempDir`. Nothing here reads the
//! real library or profile.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use grid_core::config::{Config, EmulatorEntry};
use grid_core::launch::profiles::{load_profiles, profile_for_entry};
use grid_core::library::path_change::LibraryActivity;
use grid_core::library::paths::saves_dir;
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::relocate::{
    self, config_file_editor, ConfigChange, Journal, MoveError, MoveHooks, MoveOutcome,
    MoveProgress, MoveRefusal, Phase, PlanEnv, Recovery, Strategy, StrategyProbe, VerifyMode,
};
use grid_core::library::user_data_links::{ensure_user_data_links, user_data_root};

const PCSX2: &str = "PCSX2 (Playstation 2)";
const RPCS3: &str = "RPCS3 (Playstation 3)";
const QT: &str = "ShadPS4 Qt Launcher";
const XEMU: &str = "Xemu (Xbox)";

/// `(entry name, executable file name)` of every emulator the fixture
/// installs.
const EMULATORS: [(&str, &str); 4] = [
    (PCSX2, "pcsx2-qt.exe"),
    (RPCS3, "rpcs3.exe"),
    (QT, "shadPS4QtLauncher.exe"),
    (XEMU, "xemu.exe"),
];

/// 3 MiB + 17 bytes: larger than the two 1 MiB samples together, so the
/// sampled hash really samples.
const BIG_LEN: usize = 3 * 1024 * 1024 + 17;

fn s(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn fwd(path: &Path) -> String {
    s(path).replace('\\', "/")
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn canonical(path: &Path) -> PathBuf {
    grid_core::platform::canonicalize(path).unwrap()
}

fn big_bytes() -> Vec<u8> {
    (0..BIG_LEN).map(|i| (i % 251) as u8).collect()
}

fn rename_probe() -> &'static StrategyProbe {
    &|_: &Path, _: &Path| Ok(Strategy::Rename)
}

fn copy_probe() -> &'static StrategyProbe {
    &|_: &Path, _: &Path| Ok(Strategy::Copy)
}

/// A current-layout library at `<base>/old-lib`, with the four emulators
/// that store absolute library paths, their user-data links, save data,
/// unrelated root entries, a config and a registry.
struct Fx {
    _tmp: tempfile::TempDir,
    base: PathBuf,
    config_path: PathBuf,
    journal: PathBuf,
    old: PathBuf,
    new: PathBuf,
    registry: Registry,
}

fn exe(root: &Path, name: &str) -> PathBuf {
    let file = EMULATORS.iter().find(|(n, _)| *n == name).unwrap().1;
    root.join("emulators").join(name).join(file)
}

fn fixture() -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    let base = canonical(tmp.path());
    let data = base.join("data");
    fs::create_dir_all(&data).unwrap();
    let old = base.join("old-lib");
    let new = base.join("new-lib");

    write(&old.join("games/SNES/Mario/mario.sfc"), "mario rom");
    write(&old.join("games/SNES/Mario/big.bin"), big_bytes());
    write(&old.join("games/Windows/Doom/doom.exe"), "doom");
    write(&old.join("games/Windows/Doom/prefix/user.reg"), "reg");
    write(
        &old.join("games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/EBOOT.BIN"),
        "eboot",
    );

    let profiles = load_profiles();
    for (name, _) in EMULATORS {
        let path = exe(&old, name);
        write(&path, "binary");
        let profile = profile_for_entry(name, &s(&path), profiles).unwrap();
        if !profile.user_data.is_empty() {
            let root = user_data_root(profile, &path);
            ensure_user_data_links(&root, &saves_dir(&old, &profile.name), &profile.user_data)
                .unwrap();
        }
    }
    write(
        &old.join("saves").join(PCSX2).join("memcards/Mcd001.ps2"),
        "card",
    );

    let old_canon = canonical(&old);
    write(
        &old.join("saves").join(PCSX2).join("inis/PCSX2.ini"),
        format!(
            "[Folders]\nBios = {}\n",
            s(&old_canon.join("saves").join(PCSX2).join("bios"))
        ),
    );
    write(
        &old.join("saves").join(RPCS3).join("portable/config/vfs.yml"),
        format!(
            "\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"{0}/games/PlayStation 3/.vfs/dev_hdd0/\"\n\"/games/\": \"{0}/games/PlayStation 3/.vfs/games/\"\n",
            fwd(&old_canon)
        ),
    );
    write(
        &old.join("saves")
            .join(RPCS3)
            .join("portable/config/games.yml"),
        format!(
            "BLUS00001: {}/games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n",
            fwd(&old_canon)
        ),
    );
    write(
        &old.join("saves").join(QT).join("launcher/qt_ui.ini"),
        format!(
            "[version_manager]\nversionSelected={}/emulators/ShadPS4 (Playstation 4)/shadps4.exe\n",
            fwd(&old)
        ),
    );
    let xemu_dir = old.join("emulators").join(XEMU);
    write(
        &xemu_dir.join("xemu.toml"),
        format!(
            "[sys.files]\nbootrom_path = '{}'\nhdd_path = '{}'\n",
            s(&xemu_dir.join("mcpx_1.0.bin")),
            s(&xemu_dir.join("xbox_hdd.qcow2"))
        ),
    );

    write(&old.join("notes.txt"), "keep me");
    write(&old.join(".hidden/x"), "x");

    let mut config = Config {
        library_path: s(&old),
        library_layout_version: 2,
        ..Default::default()
    };
    for (name, _) in EMULATORS {
        config.emulators.push(EmulatorEntry {
            name: name.into(),
            path: s(&exe(&old, name)),
            ..Default::default()
        });
    }
    config.emulators[0].save_paths = format!(
        "{};/elsewhere/x",
        s(&old.join("saves").join(PCSX2).join("memcards"))
    );
    config.native_manual_save_paths.insert(
        "Doom".into(),
        vec![s(&old.join("games/Windows/Doom/saves"))],
    );
    let config_path = data.join("config.toml");
    config.save(&config_path).unwrap();

    let registry = Registry::open(&data.join("grid-launcher.db")).unwrap();
    registry
        .upsert(&InstalledGame {
            title: "Mario".into(),
            platform: "SNES".into(),
            rom_id: Some(1),
            extracted_dir: s(&old.join("games/SNES/Mario")),
            extracted_path: s(&old.join("games/SNES/Mario/mario.sfc")),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();
    registry
        .upsert(&InstalledGame {
            title: "Doom".into(),
            platform: "Windows".into(),
            rom_id: Some(2),
            native_game_dir: s(&old.join("games/Windows/Doom")),
            native_executable_path: s(&old.join("games/Windows/Doom/doom.exe")),
            native_wineprefix: s(&old.join("games/Windows/Doom/prefix")),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();
    let trophies = vec![s(&old.join("games/PlayStation 3/.vfs/dev_hdd0/home/trophy"))];
    registry
        .upsert(&InstalledGame {
            title: "PS3 Game".into(),
            platform: "PlayStation 3".into(),
            rom_id: Some(3),
            ps3_trophy_paths: serde_json::to_string(&trophies).unwrap(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();
    registry
        .upsert(&InstalledGame {
            title: "Elsewhere".into(),
            platform: "SNES".into(),
            rom_id: Some(4),
            extracted_dir: s(&base.join("elsewhere/game")),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();

    Fx {
        journal: relocate::journal_path(&config_path),
        _tmp: tmp,
        base,
        config_path,
        old,
        new,
        registry,
    }
}

fn env(probe: &'static StrategyProbe) -> PlanEnv<'static> {
    PlanEnv {
        probe,
        free_space: &|_: &Path| Some(u64::MAX),
    }
}

fn start(fx: &Fx, probe: &'static StrategyProbe) -> Journal {
    let config = Config::load(&fx.config_path).unwrap();
    relocate::start_move(
        &config,
        &s(&fx.new),
        LibraryActivity::default(),
        &fx.journal,
        &env(probe),
    )
    .unwrap()
}

/// Runs `body` with a [`MoveHooks`] over `fx`, no resync and unlimited
/// space. `failpoint` and `progress` are optional test hooks.
fn with_hooks<T>(
    fx: &Fx,
    cancel: &AtomicBool,
    progress: &dyn Fn(&MoveProgress),
    failpoint: Option<&dyn Fn(&Journal) -> bool>,
    body: impl FnOnce(&MoveHooks) -> T,
) -> T {
    let editor = config_file_editor(&fx.config_path);
    let resync = |_: &Config| Ok(());
    let free = |_: &Path| Some(u64::MAX);
    let hooks = MoveHooks {
        config_path: &fx.config_path,
        registry: &fx.registry,
        profiles: load_profiles(),
        edit_config: &editor,
        resync: &resync,
        free_space: &free,
        cancel,
        progress,
        after_journal_write: failpoint,
    };
    body(&hooks)
}

fn run(fx: &Fx) -> Result<MoveOutcome, MoveError> {
    let cancel = AtomicBool::new(false);
    with_hooks(fx, &cancel, &|_| {}, None, |hooks| {
        relocate::run(&fx.journal, hooks)
    })
}

fn recover(fx: &Fx) -> Result<Recovery, MoveError> {
    let cancel = AtomicBool::new(false);
    with_hooks(fx, &cancel, &|_| {}, None, |hooks| {
        relocate::recover(&fx.journal, hooks)
    })
}

/// `recover`, then `run` for whatever it leaves, as the app does at start.
fn restart(fx: &Fx) -> MoveOutcome {
    match recover(fx).unwrap() {
        Recovery::None => panic!("a journal was expected"),
        Recovery::Copy(_) | Recovery::Cleanup(_) => run(fx).unwrap(),
    }
}

/// Every name directly in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

fn is_link(path: &Path) -> bool {
    grid_core::library::user_data_links::is_link_path(path)
}

/// The end state every completed move must reach, whatever the strategy
/// and wherever it was interrupted.
fn assert_moved(fx: &Fx) {
    let new = &fx.new;
    let old = &fx.old;
    let new_canon = canonical(new);
    assert!(!fx.journal.exists(), "the journal is deleted at the end");
    assert!(!new.join(relocate::STAGING_DIR).exists(), "no staging left");

    // Files.
    assert_eq!(read(&new.join("games/SNES/Mario/mario.sfc")), "mario rom");
    assert_eq!(
        fs::read(new.join("games/SNES/Mario/big.bin")).unwrap(),
        big_bytes()
    );
    assert_eq!(
        read(&new.join("saves").join(PCSX2).join("memcards/Mcd001.ps2")),
        "card"
    );

    // The user-data links point into the NEW saves folder.
    for (link, target) in [
        (
            new.join("emulators").join(PCSX2).join("inis"),
            new.join("saves").join(PCSX2).join("inis"),
        ),
        (
            new.join("emulators").join(RPCS3).join("portable"),
            new.join("saves").join(RPCS3).join("portable"),
        ),
        (
            new.join("emulators").join(QT).join("launcher"),
            new.join("saves").join(QT).join("launcher"),
        ),
    ] {
        assert!(is_link(&link), "{} is a link", link.display());
        assert_eq!(canonical(&link), canonical(&target), "{}", link.display());
        #[cfg(unix)]
        assert!(
            fs::read_link(&link).unwrap().is_relative(),
            "{} stays relative",
            link.display()
        );
    }

    // Text files, read through the links.
    assert_eq!(
        read(&new.join("emulators").join(PCSX2).join("inis/PCSX2.ini")),
        format!(
            "[Folders]\nBios = {}\n",
            s(&new_canon.join("saves").join(PCSX2).join("bios"))
        )
    );
    assert_eq!(
        read(&new.join("emulators").join(RPCS3).join("portable/config/vfs.yml")),
        format!(
            "\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"{0}/games/PlayStation 3/.vfs/dev_hdd0/\"\n\"/games/\": \"{0}/games/PlayStation 3/.vfs/games/\"\n",
            fwd(&new_canon)
        )
    );
    assert_eq!(
        read(
            &new.join("emulators")
                .join(RPCS3)
                .join("portable/config/games.yml")
        ),
        format!(
            "BLUS00001: {}/games/PlayStation 3/.vfs/dev_hdd0/game/BLUS00001/\n",
            fwd(&new_canon)
        )
    );
    assert_eq!(
        read(&new.join("emulators").join(QT).join("launcher/qt_ui.ini")),
        format!(
            "[version_manager]\nversionSelected={}/emulators/ShadPS4 (Playstation 4)/shadps4.exe\n",
            fwd(new)
        )
    );
    let xemu_dir = new.join("emulators").join(XEMU);
    assert_eq!(
        read(&xemu_dir.join("xemu.toml")),
        format!(
            "[sys.files]\nbootrom_path = '{}'\nhdd_path = '{}'\n",
            s(&xemu_dir.join("mcpx_1.0.bin")),
            s(&xemu_dir.join("xbox_hdd.qcow2"))
        )
    );

    // Config.
    let config = Config::load(&fx.config_path).unwrap();
    assert_eq!(config.library_path, s(new));
    assert_eq!(config.former_library_paths, vec![s(old)]);
    assert_eq!(config.library_layout_version, 2, "the layout stays current");
    for (index, (name, _)) in EMULATORS.iter().enumerate() {
        assert_eq!(config.emulators[index].path, s(&exe(new, name)));
    }
    assert_eq!(
        config.emulators[0].save_paths,
        format!(
            "{};/elsewhere/x",
            s(&new.join("saves").join(PCSX2).join("memcards"))
        )
    );
    assert_eq!(
        config.native_manual_save_paths["Doom"],
        vec![s(&new.join("games/Windows/Doom/saves"))]
    );

    // Registry.
    let rows: BTreeMap<String, InstalledGame> = fx
        .registry
        .all()
        .unwrap()
        .into_iter()
        .map(|row| (row.title.clone(), row))
        .collect();
    assert_eq!(
        rows["Mario"].extracted_dir,
        s(&new.join("games/SNES/Mario"))
    );
    assert_eq!(
        rows["Mario"].extracted_path,
        s(&new.join("games/SNES/Mario/mario.sfc"))
    );
    assert_eq!(
        rows["Doom"].native_wineprefix,
        s(&new.join("games/Windows/Doom/prefix"))
    );
    assert_eq!(
        rows["PS3 Game"].ps3_trophy_paths,
        serde_json::to_string(&vec![s(
            &new.join("games/PlayStation 3/.vfs/dev_hdd0/home/trophy")
        )])
        .unwrap()
    );
    assert_eq!(
        rows["Elsewhere"].extracted_dir,
        s(&fx.base.join("elsewhere/game"))
    );

    // The old root keeps only what did not move.
    assert_eq!(names(old), vec![".hidden", "notes.txt"]);
    assert_eq!(read(&old.join("notes.txt")), "keep me");
}

// --- complete runs -----------------------------------------------------------

#[test]
fn a_move_by_rename_completes() {
    let fx = fixture();
    let journal = start(&fx, rename_probe());
    assert!(journal
        .items
        .iter()
        .all(|item| item.strategy == Strategy::Rename));
    let outcome = run(&fx).unwrap();
    let MoveOutcome::Finished(report) = outcome else {
        panic!("finished expected");
    };
    assert!(report.report.is_empty(), "{:?}", report.report);
    assert!(!report.old_root_removed, "notes.txt keeps the old root");
    assert_moved(&fx);
}

#[test]
fn a_move_by_forced_copy_completes() {
    let fx = fixture();
    let journal = start(&fx, copy_probe());
    assert!(journal
        .items
        .iter()
        .all(|item| item.strategy == Strategy::Copy));
    let MoveOutcome::Finished(report) = run(&fx).unwrap() else {
        panic!("finished expected");
    };
    assert!(report.report.is_empty(), "{:?}", report.report);
    assert_moved(&fx);
}

#[test]
fn an_empty_old_root_is_removed() {
    let fx = fixture();
    fs::remove_file(fx.old.join("notes.txt")).unwrap();
    fs::remove_dir_all(fx.old.join(".hidden")).unwrap();
    start(&fx, copy_probe());
    let MoveOutcome::Finished(report) = run(&fx).unwrap() else {
        panic!("finished expected");
    };
    assert!(report.old_root_removed);
    assert!(!fx.old.exists());
}

/// Crash right after journal write N (N = 0 is the write `start_move`
/// makes), restart the way the app does, and reach the same end state.
fn crash_after_every_write(probe: &'static StrategyProbe) {
    let mut crash_at = 0usize;
    loop {
        let fx = fixture();
        start(&fx, probe);
        if crash_at == 0 {
            restart(&fx);
            assert_moved(&fx);
            crash_at += 1;
            continue;
        }
        let writes = Cell::new(0usize);
        let failpoint = |_: &Journal| {
            writes.set(writes.get() + 1);
            writes.get() == crash_at
        };
        let cancel = AtomicBool::new(false);
        let first = with_hooks(&fx, &cancel, &|_| {}, Some(&failpoint), |hooks| {
            relocate::run(&fx.journal, hooks)
        });
        match first {
            Err(MoveError::Interrupted) => {
                restart(&fx);
                assert_moved(&fx);
            }
            Ok(MoveOutcome::Finished(_)) => {
                assert_moved(&fx);
                assert!(crash_at > 3, "the run writes the journal several times");
                return;
            }
            other => panic!("crash {crash_at}: {other:?}"),
        }
        crash_at += 1;
    }
}

#[test]
fn a_crash_after_any_journal_write_resumes_to_the_same_end_by_rename() {
    crash_after_every_write(rename_probe());
}

#[test]
fn a_crash_after_any_journal_write_resumes_to_the_same_end_by_copy() {
    crash_after_every_write(copy_probe());
}

// --- cancel ------------------------------------------------------------------

/// Path -> (kind, bytes, mtime) for everything under `root`, links not
/// followed.
fn tree(root: &Path) -> BTreeMap<String, (String, Vec<u8>, Option<SystemTime>)> {
    fn walk(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, (String, Vec<u8>, Option<SystemTime>)>,
    ) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = fwd(path.strip_prefix(root).unwrap());
            let meta = fs::symlink_metadata(&path).unwrap();
            if is_link(&path) {
                out.insert(rel, ("link".into(), Vec::new(), None));
            } else if meta.is_dir() {
                out.insert(rel, ("dir".into(), Vec::new(), None));
                walk(root, &path, out);
            } else {
                out.insert(
                    rel,
                    (
                        "file".into(),
                        fs::read(&path).unwrap(),
                        meta.modified().ok(),
                    ),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn cancel_during_copy_leaves_the_old_library_exactly_as_it_was() {
    let fx = fixture();
    let before_tree = tree(&fx.old);
    let before_config = fs::read(&fx.config_path).unwrap();
    let before_rows = fx.registry.all().unwrap();

    start(&fx, copy_probe());
    let cancel = AtomicBool::new(false);
    let progress = |_: &MoveProgress| cancel.store(true, Ordering::SeqCst);
    let outcome = with_hooks(&fx, &cancel, &progress, None, |hooks| {
        relocate::run(&fx.journal, hooks)
    })
    .unwrap();
    assert!(matches!(outcome, MoveOutcome::Cancelled));

    assert_eq!(tree(&fx.old), before_tree);
    assert_eq!(fs::read(&fx.config_path).unwrap(), before_config);
    assert_eq!(fx.registry.all().unwrap(), before_rows);
    assert!(!fx.journal.exists());
    assert!(!fx.new.exists(), "GRID created the new folder, so it goes");
}

#[test]
fn cancel_keeps_a_new_folder_the_user_made() {
    let fx = fixture();
    write(&fx.new.join("mine.txt"), "mine");
    start(&fx, copy_probe());
    relocate::cancel(&fx.journal).unwrap();
    assert_eq!(names(&fx.new), vec!["mine.txt"]);
    assert!(!fx.journal.exists());
}

#[test]
fn cancel_is_refused_once_the_move_commits() {
    let fx = fixture();
    start(&fx, copy_probe());
    let writes = Cell::new(0usize);
    // Crash on the write that enters Cleanup: the commit is done.
    let failpoint = |journal: &Journal| {
        writes.set(writes.get() + 1);
        journal.phase == Phase::Cleanup
    };
    let cancel = AtomicBool::new(false);
    let first = with_hooks(&fx, &cancel, &|_| {}, Some(&failpoint), |hooks| {
        relocate::run(&fx.journal, hooks)
    });
    assert!(matches!(first, Err(MoveError::Interrupted)));
    assert!(matches!(
        relocate::cancel(&fx.journal),
        Err(MoveError::NotCancellable)
    ));
    run(&fx).unwrap();
    assert_moved(&fx);
}

// --- refusals and safety -------------------------------------------------------

#[test]
fn a_target_with_library_folders_is_refused_untouched() {
    let fx = fixture();
    write(&fx.new.join("games/theirs.bin"), "theirs");
    let config = Config::load(&fx.config_path).unwrap();
    let refused = relocate::start_move(
        &config,
        &s(&fx.new),
        LibraryActivity::default(),
        &fx.journal,
        &env(copy_probe()),
    );
    assert!(matches!(
        refused,
        Err(MoveError::Refused(MoveRefusal::TargetHasLibraryFolders))
    ));
    assert_eq!(names(&fx.new), vec!["games"]);
    assert_eq!(names(&fx.new.join("games")), vec!["theirs.bin"]);
    assert!(!fx.journal.exists());
}

#[test]
fn a_source_changed_before_cleanup_is_kept() {
    let fx = fixture();
    start(&fx, copy_probe());
    let failpoint = |journal: &Journal| journal.phase == Phase::Cleanup;
    let cancel = AtomicBool::new(false);
    let first = with_hooks(&fx, &cancel, &|_| {}, Some(&failpoint), |hooks| {
        relocate::run(&fx.journal, hooks)
    });
    assert!(matches!(first, Err(MoveError::Interrupted)));

    let card = fx.old.join("saves").join(PCSX2).join("memcards/Mcd001.ps2");
    write(&card, "card, changed after the copy");

    let MoveOutcome::Finished(report) = restart(&fx) else {
        panic!("finished expected");
    };
    assert_eq!(report.report.len(), 1, "{:?}", report.report);
    assert!(report.report[0].contains(&s(&fx.old.join("saves"))));
    assert_eq!(read(&card), "card, changed after the copy");
    // The unchanged items were cleaned up.
    assert!(!fx.old.join("games").exists());
    assert!(!fx.old.join("emulators").exists());
    // The copy is the one the library uses.
    assert_eq!(
        read(&fx.new.join("saves").join(PCSX2).join("memcards/Mcd001.ps2")),
        "card"
    );
}

/// A value still under the old root after the commit (a writer slipped in)
/// keeps that item's source and is reported.
#[test]
fn a_value_left_under_the_old_root_keeps_that_item() {
    let fx = fixture();
    start(&fx, copy_probe());
    let base_editor = config_file_editor(&fx.config_path);
    let stale = s(&fx.old.join("saves/stale/save.bin"));
    let stale_for_editor = stale.clone();
    let editor = move |change: &mut ConfigChange| {
        base_editor(&mut |config: &mut Config| {
            change(config)?;
            config.emulators[1].state_paths = stale_for_editor.clone();
            Ok(())
        })
    };
    let resync = |_: &Config| Ok(());
    let free = |_: &Path| Some(u64::MAX);
    let cancel = AtomicBool::new(false);
    let hooks = MoveHooks {
        config_path: &fx.config_path,
        registry: &fx.registry,
        profiles: load_profiles(),
        edit_config: &editor,
        resync: &resync,
        free_space: &free,
        cancel: &cancel,
        progress: &|_| {},
        after_journal_write: None,
    };
    let MoveOutcome::Finished(report) = relocate::run(&fx.journal, &hooks).unwrap() else {
        panic!("finished expected");
    };
    assert!(
        report.report.iter().any(|line| line.contains(&stale)),
        "{:?}",
        report.report
    );
    assert!(fx.old.join("saves").exists(), "the saves source is kept");
    assert!(!fx.old.join("games").exists());
}

#[test]
fn the_space_check_counts_copied_items_and_a_margin() {
    let fx = fixture();
    let config = Config::load(&fx.config_path).unwrap();
    let tight = PlanEnv {
        probe: copy_probe(),
        free_space: &|_: &Path| Some(1024),
    };
    assert!(matches!(
        relocate::plan_move(
            &config,
            &s(&fx.new),
            LibraryActivity::default(),
            &fx.journal,
            &tight
        ),
        Err(MoveError::Refused(MoveRefusal::NotEnoughSpace))
    ));
    // A same-drive move copies nothing, so it needs no free space.
    let tight_rename = PlanEnv {
        probe: rename_probe(),
        free_space: &|_: &Path| Some(0),
    };
    let preview = relocate::plan_move(
        &config,
        &s(&fx.new),
        LibraryActivity::default(),
        &fx.journal,
        &tight_rename,
    )
    .unwrap();
    assert_eq!(preview.copy_bytes, 0);
    assert!(preview.total_bytes >= BIG_LEN as u64);
}

#[test]
fn a_second_move_is_refused_while_a_journal_exists() {
    let fx = fixture();
    start(&fx, copy_probe());
    let config = Config::load(&fx.config_path).unwrap();
    assert!(matches!(
        relocate::start_move(
            &config,
            &s(&fx.base.join("third")),
            LibraryActivity::default(),
            &fx.journal,
            &env(copy_probe())
        ),
        Err(MoveError::Refused(MoveRefusal::MoveActive))
    ));
}

// --- copy details ----------------------------------------------------------------

#[test]
fn a_copy_keeps_the_modified_time_and_read_only_flag() {
    let fx = fixture();
    let rom = fx.old.join("games/SNES/Mario/mario.sfc");
    let past = SystemTime::now() - Duration::from_secs(86_400 * 400);
    fs::File::options()
        .write(true)
        .open(&rom)
        .unwrap()
        .set_modified(past)
        .unwrap();
    let mut permissions = fs::metadata(&rom).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&rom, permissions).unwrap();

    start(&fx, copy_probe());
    run(&fx).unwrap();
    let moved = fx.new.join("games/SNES/Mario/mario.sfc");
    let meta = fs::metadata(&moved).unwrap();
    assert_eq!(meta.modified().unwrap(), past);
    assert!(meta.permissions().readonly());
    assert!(!rom.exists(), "the read-only source was cleaned up");
}

#[test]
fn verify_catches_a_truncated_and_a_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write(&src.join("a/big.bin"), big_bytes());
    write(&src.join("b.txt"), "bee");
    let copy_tree = |dst: &Path| {
        write(&dst.join("a/big.bin"), big_bytes());
        write(&dst.join("b.txt"), "bee");
    };

    let good = tmp.path().join("good");
    copy_tree(&good);
    relocate::verify_copy(&src, &good, VerifyMode::Full).unwrap();
    relocate::verify_copy(&src, &good, VerifyMode::Sampled).unwrap();

    let truncated = tmp.path().join("truncated");
    copy_tree(&truncated);
    let mut bytes = big_bytes();
    bytes.truncate(BIG_LEN - 1);
    write(&truncated.join("a/big.bin"), bytes);
    assert!(matches!(
        relocate::verify_copy(&src, &truncated, VerifyMode::Sampled),
        Err(MoveError::VerifyFailed(_))
    ));

    // Same length, one byte changed in the last MiB: the sample sees it.
    let changed = tmp.path().join("changed");
    copy_tree(&changed);
    let mut bytes = big_bytes();
    bytes[BIG_LEN - 5] ^= 0xff;
    write(&changed.join("a/big.bin"), bytes);
    assert!(matches!(
        relocate::verify_copy(&src, &changed, VerifyMode::Sampled),
        Err(MoveError::VerifyFailed(_))
    ));

    let missing = tmp.path().join("missing");
    copy_tree(&missing);
    fs::remove_file(missing.join("b.txt")).unwrap();
    assert!(matches!(
        relocate::verify_copy(&src, &missing, VerifyMode::Full),
        Err(MoveError::VerifyFailed(_))
    ));
}

/// A top-level moved folder that is itself a link (here `saves`, kept on
/// another disk) moves as a link, whatever the strategy, and still points
/// at the same place afterwards.
fn a_top_level_link_moves_as_a_link(probe: &'static StrategyProbe) {
    let fx = fixture();
    let outside = fx.base.join("elsewhere-saves");
    fs::rename(fx.old.join("saves"), &outside).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, fx.old.join("saves")).unwrap();
    #[cfg(windows)]
    junction::create(&outside, fx.old.join("saves")).unwrap();

    start(&fx, probe);
    let MoveOutcome::Finished(report) = run(&fx).unwrap() else {
        panic!("finished expected");
    };
    assert!(report.report.is_empty(), "{:?}", report.report);
    let link = fx.new.join("saves");
    assert!(is_link(&link));
    assert_eq!(canonical(&link), canonical(&outside));
    assert_eq!(
        read(&link.join(PCSX2).join("memcards/Mcd001.ps2")),
        "card",
        "the data stayed where it was"
    );
    assert!(fs::symlink_metadata(fx.old.join("saves")).is_err());
}

#[test]
fn a_top_level_link_moves_as_a_link_by_rename() {
    a_top_level_link_moves_as_a_link(rename_probe());
}

#[test]
fn a_top_level_link_moves_as_a_link_by_copy() {
    a_top_level_link_moves_as_a_link(copy_probe());
}

#[cfg(unix)]
#[test]
fn a_symlink_is_copied_as_a_link() {
    let fx = fixture();
    std::os::unix::fs::symlink("Mario", fx.old.join("games/SNES/latest")).unwrap();
    start(&fx, copy_probe());
    run(&fx).unwrap();
    let link = fx.new.join("games/SNES/latest");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("Mario"));
    assert_eq!(read(&link.join("mario.sfc")), "mario rom");
}

/// A junction GRID did not make (not a catalog user-data link) is
/// recreated by the forced copy, pointing at the moved folder.
#[cfg(windows)]
#[test]
fn a_junction_is_recreated_after_a_forced_copy() {
    let fx = fixture();
    junction::create(
        fx.old.join("games/SNES/Mario"),
        fx.old.join("games/SNES/latest"),
    )
    .unwrap();
    start(&fx, copy_probe());
    run(&fx).unwrap();
    let link = fx.new.join("games/SNES/latest");
    assert!(junction::exists(&link).unwrap());
    assert_eq!(
        canonical(&link),
        canonical(&fx.new.join("games/SNES/Mario"))
    );
    assert_eq!(read(&link.join("mario.sfc")), "mario rom");
}

// --- relocate references only ------------------------------------------------

/// The unreachable-library fix: the user moved the folder outside GRID;
/// pointing GRID at it rewrites every reference and moves no file.
#[test]
fn relocate_references_rewrites_without_moving_files() {
    let fx = fixture();
    // The user's own move: same names, at the new place.
    fs::rename(&fx.old, &fx.new).unwrap();
    let report = with_hooks(&fx, &AtomicBool::new(false), &|_| {}, None, |hooks| {
        relocate::relocate_references(&s(&fx.old), &fx.new, hooks)
    })
    .unwrap();
    assert!(report.is_empty(), "{report:?}");

    let config = Config::load(&fx.config_path).unwrap();
    assert_eq!(config.library_path, s(&fx.new));
    assert_eq!(config.library_layout_version, 2);
    assert_eq!(config.emulators[3].path, s(&exe(&fx.new, XEMU)));
    let mario = fx.registry.find(Some(1), "", "").unwrap().unwrap();
    assert_eq!(mario.extracted_dir, s(&fx.new.join("games/SNES/Mario")));
    let xemu_dir = fx.new.join("emulators").join(XEMU);
    assert!(read(&xemu_dir.join("xemu.toml")).contains(&s(&xemu_dir.join("xbox_hdd.qcow2"))));
    let link = fx.new.join("emulators").join(PCSX2).join("inis");
    assert_eq!(
        canonical(&link),
        canonical(&fx.new.join("saves").join(PCSX2).join("inis"))
    );
}
