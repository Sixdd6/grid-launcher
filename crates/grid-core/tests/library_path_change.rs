//! Q2 Start fresh, end to end through `InstallService` against a temp
//! library: Leave drops only the rows under the old root and touches no
//! file; Delete removes files only through the guarded uninstall path and
//! never outside the old root; the preview lists what Delete would remove.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use grid_core::config::Config;
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::InstallService;

struct Fixture {
    _dir: tempfile::TempDir,
    old: PathBuf,
    new: PathBuf,
    outside: PathBuf,
    config_path: PathBuf,
    registry: Arc<Registry>,
    service: Arc<InstallService>,
}

fn s(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![7u8; bytes]).unwrap();
}

/// An old library with two SNES games, a native game and a server
/// emulator package; a native game installed elsewhere; and a decoy folder
/// outside every library.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old-lib");
    let new = dir.path().join("new-lib");
    let outside = dir.path().join("elsewhere");

    write(&old.join("games/SNES/Mario/game.sfc"), 1000);
    write(&old.join("games/SNES/Zelda.sfc"), 300);
    write(&old.join("games/Windows/Doom/game/doom.exe"), 50);
    write(&old.join("games/Emulators/pkg/emu.exe"), 20);
    write(&old.join("emulators/RetroArch/retroarch.exe"), 10);
    write(&old.join("saves/RetroArch/saves/mario.srm"), 8);
    write(&outside.join("Quake/game/quake.exe"), 40);
    write(&outside.join("decoy/keep.txt"), 5);

    let config_path = dir.path().join("config.toml");
    Config {
        library_path: s(&old),
        library_layout_version: 2,
        ..Default::default()
    }
    .save(&config_path)
    .unwrap();

    let registry = Arc::new(Registry::open(&dir.path().join("registry.db")).unwrap());
    let rows = [
        InstalledGame {
            title: "Mario".into(),
            platform: "SNES".into(),
            rom_id: Some(1),
            rom_file_name: "Mario.zip".into(),
            extracted_dir: s(&old.join("games/SNES/Mario")),
            extracted_path: s(&old.join("games/SNES/Mario/game.sfc")),
            ..Default::default()
        },
        InstalledGame {
            title: "Zelda".into(),
            platform: "SNES".into(),
            rom_id: Some(2),
            rom_file_name: "Zelda.sfc".into(),
            archive_path: s(&old.join("games/SNES/Zelda.sfc")),
            extracted_path: s(&old.join("games/SNES/Zelda.sfc")),
            ..Default::default()
        },
        InstalledGame {
            title: "Doom".into(),
            platform: "Windows".into(),
            rom_id: Some(3),
            native_game_dir: s(&old.join("games/Windows/Doom")),
            extracted_dir: s(&old.join("games/Windows/Doom/game")),
            ..Default::default()
        },
        InstalledGame {
            title: "Some Emu".into(),
            platform: "Emulators".into(),
            rom_id: Some(4),
            extracted_dir: s(&old.join("games/Emulators/pkg")),
            ..Default::default()
        },
        InstalledGame {
            title: "Quake".into(),
            platform: "Windows".into(),
            rom_id: Some(5),
            native_game_dir: s(&outside.join("Quake")),
            extracted_dir: s(&outside.join("Quake/game")),
            ..Default::default()
        },
    ];
    for row in &rows {
        registry.upsert(row).unwrap();
    }
    let service = InstallService::new(registry.clone(), config_path.clone());
    Fixture {
        _dir: dir,
        old,
        new,
        outside,
        config_path,
        registry,
        service,
    }
}

fn titles(registry: &Registry) -> Vec<String> {
    let mut titles: Vec<String> = registry
        .all()
        .unwrap()
        .into_iter()
        .map(|row| row.title)
        .collect();
    titles.sort();
    titles
}

#[test]
fn forget_removes_only_rows_under_the_old_root_and_keeps_every_file() {
    let f = fixture();
    let report = f.service.start_fresh_forget(&f.old).unwrap();

    assert_eq!(report.rows_removed, 3);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    // The server emulator package row and the game elsewhere stay.
    assert_eq!(titles(&f.registry), vec!["Quake", "Some Emu"]);
    for kept in [
        f.old.join("games/SNES/Mario/game.sfc"),
        f.old.join("games/SNES/Zelda.sfc"),
        f.old.join("games/Windows/Doom/game/doom.exe"),
        f.old.join("games/Emulators/pkg/emu.exe"),
        f.old.join("emulators/RetroArch/retroarch.exe"),
        f.old.join("saves/RetroArch/saves/mario.srm"),
        f.outside.join("Quake/game/quake.exe"),
    ] {
        assert!(kept.is_file(), "{kept:?} must stay on disk");
    }
}

#[test]
fn preview_lists_the_paths_inside_the_old_root_and_their_size() {
    let f = fixture();
    let preview = f.service.start_fresh_preview(&f.old).unwrap();

    assert_eq!(preview.old_root, s(&f.old));
    let mut games: Vec<(String, Vec<PathBuf>, u64)> = preview
        .games
        .iter()
        .map(|g| {
            (
                g.title.clone(),
                g.paths.iter().map(PathBuf::from).collect(),
                g.bytes,
            )
        })
        .collect();
    games.sort();
    assert_eq!(
        games,
        vec![
            (
                "Doom".to_string(),
                vec![f.old.join("games/Windows/Doom")],
                50
            ),
            (
                "Mario".to_string(),
                vec![f.old.join("games/SNES/Mario")],
                1000
            ),
            (
                "Zelda".to_string(),
                vec![f.old.join("games/SNES/Zelda.sfc")],
                300
            ),
        ]
    );
    assert_eq!(preview.total_bytes, 1350);
    assert!(preview.left_outside.is_empty());
    // A preview changes nothing.
    assert_eq!(titles(&f.registry).len(), 5);
    assert!(f.old.join("games/SNES/Mario/game.sfc").is_file());
}

#[test]
fn delete_removes_files_through_uninstall_and_never_outside_the_old_root() {
    let f = fixture();
    // Switch first, the way the app does: the old root is now a former root.
    let mut config = Config::load(&f.config_path).unwrap();
    grid_core::library::path_change::switch_library_root(&mut config, &f.new);
    config.save(&f.config_path).unwrap();

    let report = f.service.start_fresh_delete(&f.old).unwrap();

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.rows_removed, 3);
    assert_eq!(titles(&f.registry), vec!["Quake", "Some Emu"]);
    for gone in [
        f.old.join("games/SNES/Mario"),
        f.old.join("games/SNES/Zelda.sfc"),
        f.old.join("games/Windows/Doom"),
    ] {
        assert!(!gone.exists(), "{gone:?} must be removed");
    }
    // Library folders, emulators, saves, the server package and everything
    // outside the old root stay.
    for kept in [
        f.old.join("games/SNES"),
        f.old.join("games/Windows"),
        f.old.join("games/Emulators/pkg/emu.exe"),
        f.old.join("emulators/RetroArch/retroarch.exe"),
        f.old.join("saves/RetroArch/saves/mario.srm"),
        f.outside.join("Quake/game/quake.exe"),
        f.outside.join("decoy/keep.txt"),
    ] {
        assert!(kept.exists(), "{kept:?} must stay on disk");
    }
}

/// A row under the old root whose record points a removal at a folder
/// outside it (here: its PS3 trophy list names the decoy) gives up the
/// files inside the old root only; the decoy stays.
#[test]
fn delete_leaves_a_recorded_path_outside_the_old_root() {
    let f = fixture();
    write(&f.old.join("games/PlayStation 3/Game.iso"), 70);
    let decoy = f.outside.join("decoy");
    f.registry
        .upsert(&InstalledGame {
            title: "PS3 Game".into(),
            platform: "PlayStation 3".into(),
            rom_id: Some(6),
            ps3_iso_path: s(&f.old.join("games/PlayStation 3/Game.iso")),
            ps3_trophy_paths: serde_json_list(&[&decoy]),
            ..Default::default()
        })
        .unwrap();

    let preview = f.service.start_fresh_preview(&f.old).unwrap();
    assert_eq!(preview.left_outside, vec![s(&decoy)]);
    let ps3 = preview
        .games
        .iter()
        .find(|g| g.title == "PS3 Game")
        .expect("the PS3 row is under the old root");
    assert_eq!(
        ps3.paths.iter().map(PathBuf::from).collect::<Vec<_>>(),
        vec![f.old.join("games/PlayStation 3/Game.iso")]
    );

    let report = f.service.start_fresh_delete(&f.old).unwrap();
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(!f.old.join("games/PlayStation 3/Game.iso").exists());
    assert!(decoy.join("keep.txt").is_file(), "the decoy must stay");
}

/// After a change, an ordinary uninstall of a row whose record names the
/// old root itself as its folder removes nothing there: the guard protects
/// every former root.
#[test]
fn uninstall_after_a_change_refuses_the_former_root() {
    let f = fixture();
    let mut config = Config::load(&f.config_path).unwrap();
    grid_core::library::path_change::switch_library_root(&mut config, &f.new);
    config.save(&f.config_path).unwrap();
    fs::create_dir_all(&f.new).unwrap();

    f.registry
        .upsert(&InstalledGame {
            title: "Broken".into(),
            platform: "SNES".into(),
            rom_id: Some(9),
            multi_file_game_dir: s(&f.old),
            ..Default::default()
        })
        .unwrap();
    f.service.uninstall(9).unwrap();

    assert!(f.old.join("games/SNES/Mario/game.sfc").is_file());
    assert!(f.old.join("emulators/RetroArch/retroarch.exe").is_file());
}

fn serde_json_list(paths: &[&Path]) -> String {
    let list: Vec<String> = paths.iter().map(|p| s(p)).collect();
    serde_json::to_string(&list).unwrap()
}
