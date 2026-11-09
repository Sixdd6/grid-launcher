//! Installs from a backup drive through `InstallService`, with no RomM
//! server at all: a tempdir "drive" holding a `gridpack/` folder, a tempdir
//! library and a fake config file.
//!
//! Every wait in here is bounded (`wait_terminal`), so a broken queue fails
//! the test instead of hanging it.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::backup::drives::drive_key;
use grid_core::backup::pack::{self, PackEmulator, PackGame, PackOs};
use grid_core::config::Config;
use grid_core::launch::profiles::EmulatorProfile;
use grid_core::library::backup_job::DriveTarget;
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use serde_json::json;

const LABEL: &str = "SAMSUNG T7";
const SERVER: &str = "https://romm.example";

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);
    for &(name, content) in entries {
        zip.start_file(name, options).unwrap();
        zip.write_all(content).unwrap();
    }
    zip.finish().unwrap();
}

/// A catalog profile whose source names `acme/widget`. Nothing is ever
/// fetched from it: an install from a drive copies, never downloads.
fn widget_profile() -> EmulatorProfile {
    EmulatorProfile {
        name: "Widget".to_string(),
        match_tokens: vec!["widget".to_string()],
        args: "-f %rom%".to_string(),
        all_platforms: true,
        source: Some(json!({
            "provider": "gitea",
            "owner": "acme",
            "repo": "widget",
            "base_url": "http://127.0.0.1:9",
            "release_tag": "v1.0",
        })),
        ..Default::default()
    }
}

struct Harness {
    _tmp: tempfile::TempDir,
    library: PathBuf,
    mount: PathBuf,
    config_path: PathBuf,
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
                "schema_version = 1\nserver_url = \"http://x\"\nusername = \"u\"\nlibrary_path = {:?}\n",
                library.to_string_lossy()
            ),
        )
        .unwrap();
        let mount = tmp.path().join("media").join(LABEL);
        fs::create_dir_all(&mount).unwrap();
        pack::write_pack_atomic(&mount, &pack::new_pack("Couch kit", &[PackOs::Linux])).unwrap();
        let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
        let service = InstallService::with_profiles(
            registry.clone(),
            config_path.clone(),
            vec![widget_profile()],
        );
        Harness {
            _tmp: tmp,
            library,
            mount,
            config_path,
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

    /// Writes `body` at the pack-relative `rel` on the drive.
    fn put(&self, rel: &str, body: &[u8]) {
        let path = pack::resolve(&self.mount, rel).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn newest_entry_id(&self) -> u64 {
        self.service
            .snapshot()
            .entries
            .first()
            .expect("at least one entry")
            .id
    }

    async fn wait_terminal(&self, id: u64) -> DownloadEntry {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(entry) = self.service.snapshot().entries.iter().find(|e| e.id == id) {
                if matches!(
                    entry.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                ) {
                    return entry.clone();
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting on entry {id}: {:?}",
                self.service.snapshot().entries
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn config(&self) -> Config {
        Config::load(&self.config_path).unwrap()
    }
}

/// A pack entry for a game whose files are `(id, name, pack path, size)`.
fn pack_game(
    rom_id: i64,
    name: &str,
    platform: &str,
    platform_name: &str,
    files: &[(i64, &str, &str, usize)],
) -> PackGame {
    serde_json::from_value(json!({
        "platform": platform,
        "platform_name": platform_name,
        "source": {
            "server": SERVER,
            "rom_id": rom_id,
            "fs_name": name,
            "cover_small_path": "/assets/romm/resources/roms/1/cover/small.png",
            "cover_large_path": "/assets/romm/resources/roms/1/cover/big.png",
            "screenshot_urls": [format!("{SERVER}/assets/romm/resources/roms/1/shot.png")],
            "fanart_urls": [],
        },
        "name": name,
        "size": files.iter().map(|f| f.3).sum::<usize>(),
        "files": files.iter().map(|(id, file_name, rel, size)| json!({
            "path": rel,
            "size": size,
            "server_file_id": id,
            "name": file_name,
        })).collect::<Vec<_>>(),
        "added": "2026-11-09T00:00:00Z",
    }))
    .unwrap()
}

/// A single-file zip game on the drive: Chrono Trigger, rom 7.
fn chrono(h: &Harness) -> PackGame {
    let staging = tempfile::tempdir().unwrap();
    let zip = staging.path().join("Chrono Trigger.zip");
    write_zip(&zip, &[("Chrono Trigger.sfc", b"SNES ROM BYTES")]);
    let bytes = fs::read(&zip).unwrap();
    let rel = "games/snes/Chrono Trigger.zip";
    h.put(rel, &bytes);
    pack_game(
        7,
        "Chrono Trigger",
        "snes",
        "SNES",
        &[(71, "Chrono Trigger.zip", rel, bytes.len())],
    )
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[tokio::test]
async fn a_single_file_game_installs_with_its_rom_id_and_absolute_covers() {
    let h = Harness::new();
    let game = chrono(&h);
    h.service
        .install_from_drive(h.drive(), &game, None)
        .unwrap();
    let id = h.newest_entry_id();
    let entry = h.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.drive_label, LABEL);
    assert_eq!(entry.rom_id, 7);

    let row = h.registry.find(Some(7), "", "").unwrap().unwrap();
    assert_eq!(row.rom_id, Some(7));
    assert_eq!(row.source_server, SERVER);
    assert_eq!(
        row.cover_small_path,
        format!("{SERVER}/assets/romm/resources/roms/1/cover/small.png")
    );
    assert_eq!(
        row.cover_large_path,
        format!("{SERVER}/assets/romm/resources/roms/1/cover/big.png")
    );
    assert_eq!(
        row.screenshot_urls,
        format!("{SERVER}/assets/romm/resources/roms/1/shot.png")
    );
    let launch = PathBuf::from(&row.extracted_path);
    assert_eq!(launch.file_name().unwrap(), "Chrono Trigger.sfc");
    assert_eq!(fs::read(&launch).unwrap(), b"SNES ROM BYTES");
    assert!(launch.starts_with(h.library.join("games").join("SNES")));
    // The drive is only read.
    assert!(h
        .mount
        .join("gridpack/games/snes/Chrono Trigger.zip")
        .is_file());
}

#[tokio::test]
async fn a_drive_install_while_connected_to_another_server_drops_the_rom_id() {
    let h = Harness::new();
    let game = chrono(&h);
    h.service
        .install_from_drive(h.drive(), &game, Some("https://other.example"))
        .unwrap();
    let entry = h.wait_terminal(h.newest_entry_id()).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let row = h
        .registry
        .find(None, "Chrono Trigger", "SNES")
        .unwrap()
        .unwrap();
    assert_eq!(row.rom_id, None);
    assert_eq!(row.source_server, SERVER);
}

#[tokio::test]
async fn a_native_game_installs_through_the_native_finalize() {
    let h = Harness::new();
    let staging = tempfile::tempdir().unwrap();
    let zip = staging.path().join("mygame.zip");
    write_zip(&zip, &[("MyGame/mygame.exe", b"MZ")]);
    let bytes = fs::read(&zip).unwrap();
    let sidecar = br#"{"version": "1.2"}"#;
    h.put("games/win/My Game/mygame.zip", &bytes);
    h.put("games/win/My Game/game.json", sidecar);
    let game = pack_game(
        5,
        "My Game",
        "win",
        "Windows",
        &[
            (
                51,
                "mygame.zip",
                "games/win/My Game/mygame.zip",
                bytes.len(),
            ),
            (
                52,
                "game.json",
                "games/win/My Game/game.json",
                sidecar.len(),
            ),
        ],
    );
    h.service
        .install_from_drive(h.drive(), &game, None)
        .unwrap();
    let entry = h.wait_terminal(h.newest_entry_id()).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let game_dir = h.library.join("games").join("Windows").join("My Game");
    let extracted_dir = game_dir.join("game");
    assert!(extracted_dir.join("MyGame").join("mygame.exe").is_file());
    assert!(!game_dir.join("mygame.zip").exists());
    let row = h.registry.find(Some(5), "", "").unwrap().unwrap();
    assert!(row.is_native());
    assert_eq!(row.native_game_dir, game_dir.to_string_lossy());
    assert_eq!(row.extracted_dir, extracted_dir.to_string_lossy());
    assert_eq!(row.revision, "1.2");
}

/// The drive goes away in the middle of the copy (Unix only: Windows
/// refuses to delete a folder holding an open file).
#[cfg(unix)]
#[tokio::test]
async fn a_drive_removed_mid_copy_fails_the_row_and_leaves_nothing_behind() {
    let h = Harness::new();
    let game = chrono(&h);
    let mount = h.mount.clone();
    h.service.set_drive_copy_hook(Some(Arc::new(move || {
        let _ = fs::remove_dir_all(&mount);
        Err(io::Error::other("Input/output error"))
    })));
    h.service
        .install_from_drive(h.drive(), &game, None)
        .unwrap();
    let entry = h.wait_terminal(h.newest_entry_id()).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(entry.error, format!("{LABEL} was removed"));
    assert!(h.registry.all().unwrap().is_empty(), "no registry row");
    assert_eq!(
        files_under(&h.library),
        Vec::<PathBuf>::new(),
        "no partial file in the library"
    );
}

/// An emulator release from the pack: a stub AppImage for this computer.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn an_emulator_from_the_pack_installs_with_the_packs_source_id() {
    let h = Harness::new();
    let rel = "emulators/Widget/v1.0/linux-x86_64/widget-x86_64.AppImage";
    h.put(rel, b"#!/bin/sh\necho widget\n");
    let entry = PackEmulator {
        name: "Widget".to_string(),
        kind: "emulator".to_string(),
        source_id: "acme/widget".to_string(),
        version: "v1.0".to_string(),
        program: String::new(),
        args: String::new(),
        os: PackOs::Linux.arch_tag().to_string(),
        platforms: vec!["snes".to_string()],
        path: rel.to_string(),
        supplementals: Vec::new(),
        asset_name: "widget-x86_64.AppImage".to_string(),
        size: 22,
        extra: Default::default(),
    };
    h.service
        .install_emulator_from_drive(h.drive(), &entry)
        .unwrap();
    let row = h.wait_terminal(h.newest_entry_id()).await;
    assert_eq!(row.status, DownloadStatus::Completed, "{}", row.error);
    assert_eq!(row.kind, "emulator");

    let config = h.config();
    let installed = config
        .emulators
        .iter()
        .find(|e| e.name == "Widget")
        .expect("an [[emulators]] entry");
    assert_eq!(installed.source_id, "acme/widget");
    assert_eq!(installed.source_installed_tag, "v1.0");
    assert_eq!(installed.source_release_tag, "v1.0");
    assert_eq!(installed.args, "-f %rom%");
    let exe = h
        .library
        .join("emulators")
        .join("Widget")
        .join("widget-x86_64.AppImage");
    assert_eq!(PathBuf::from(&installed.path), exe);
    assert!(exe.is_file());
}

/// A release for another OS is refused before anything is queued.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn an_emulator_for_another_os_is_refused() {
    let h = Harness::new();
    let entry = PackEmulator {
        name: "Widget".to_string(),
        kind: "emulator".to_string(),
        source_id: "acme/widget".to_string(),
        version: "v1.0".to_string(),
        program: String::new(),
        args: String::new(),
        os: PackOs::Windows.arch_tag().to_string(),
        platforms: Vec::new(),
        path: "emulators/Widget/v1.0/windows-x86_64/widget.zip".to_string(),
        supplementals: Vec::new(),
        asset_name: "widget.zip".to_string(),
        size: 1,
        extra: Default::default(),
    };
    assert!(h
        .service
        .install_emulator_from_drive(h.drive(), &entry)
        .is_err());
    assert!(h.service.snapshot().entries.is_empty());
}

/// An archived manual emulator: extracted under `emulators/<name>` and
/// registered as a manual entry with the pack's program and options.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_local_zip_emulator_installs_as_a_manual_entry() {
    let h = Harness::new();
    let rel = "emulators/My Emu/local/linux-x86_64/My Emu.zip";
    let zip = pack::resolve(&h.mount, rel).unwrap();
    write_zip(
        &zip,
        &[
            ("bin/myemu", b"#!/bin/sh\necho emu\n"),
            ("data/readme.txt", b"hello"),
        ],
    );
    let entry = PackEmulator {
        name: "My Emu".to_string(),
        kind: "emulator".to_string(),
        source_id: "local:My Emu".to_string(),
        version: "local".to_string(),
        program: "bin/myemu".to_string(),
        args: "--fullscreen %rom%".to_string(),
        os: PackOs::Linux.arch_tag().to_string(),
        platforms: vec!["snes".to_string()],
        path: rel.to_string(),
        supplementals: Vec::new(),
        asset_name: "My Emu.zip".to_string(),
        size: fs::metadata(&zip).unwrap().len(),
        extra: Default::default(),
    };
    h.service
        .install_emulator_from_drive(h.drive(), &entry)
        .unwrap();
    let row = h.wait_terminal(h.newest_entry_id()).await;
    assert_eq!(row.status, DownloadStatus::Completed, "{}", row.error);

    let install_dir = h.library.join("emulators").join("My Emu");
    let exe = install_dir.join("bin").join("myemu");
    assert!(exe.is_file());
    assert!(install_dir.join("data").join("readme.txt").is_file());
    assert!(
        !install_dir.join("My Emu.zip").exists(),
        "the archive is deleted once extracted"
    );
    let config = h.config();
    let installed = config
        .emulators
        .iter()
        .find(|e| e.name == "My Emu")
        .expect("an [[emulators]] entry");
    assert_eq!(PathBuf::from(&installed.path), exe);
    assert_eq!(installed.args, "--fullscreen %rom%");
    assert_eq!(
        installed.source_id, "",
        "a manual entry has no update source"
    );
    assert_eq!(installed.source_provider, "");
    assert_eq!(installed.source_installed_tag, "");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(fs::metadata(&exe).unwrap().permissions().mode() & 0o111, 0);
    }
}

// --- Installed copies (an archive in parts) -----------------------------------

/// One entry of a hand-built installed-copy archive: a folder (`body` is
/// `None`) or a file with its unix mode.
struct TreeItem {
    name: &'static str,
    body: Option<&'static [u8]>,
    mode: u32,
}

fn dir(name: &'static str) -> TreeItem {
    TreeItem {
        name,
        body: None,
        mode: 0o755,
    }
}

fn file(name: &'static str, body: &'static [u8]) -> TreeItem {
    // 0o644 on purpose: the restore must mark the launch file executable.
    TreeItem {
        name,
        body: Some(body),
        mode: 0o644,
    }
}

/// The part size the hand-built archives are cut at, so every copy below
/// spans several parts. The archives are cut here, not by the backup
/// writer, so the process-wide part-size override is never touched.
const PART: usize = 1024;

fn md5_hex(bytes: &[u8]) -> String {
    use md5::{Digest, Md5};
    Md5::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Writes `items` as an uncompressed tar cut into [`PART`]-byte parts at
/// `<rel_base>/installed.tar.NNN` on the drive, and returns the pack's
/// `parts` entries.
fn put_installed(h: &Harness, rel_base: &str, items: &[TreeItem]) -> Vec<serde_json::Value> {
    let mut builder = tar::Builder::new(Vec::new());
    for item in items {
        let mut header = tar::Header::new_gnu();
        header.set_mode(item.mode);
        header.set_mtime(0);
        match item.body {
            None => {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                builder
                    .append_data(&mut header, item.name, io::empty())
                    .unwrap();
            }
            Some(body) => {
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(body.len() as u64);
                builder.append_data(&mut header, item.name, body).unwrap();
            }
        }
    }
    let archive = builder.into_inner().unwrap();
    assert!(archive.len() > PART, "the copy spans several parts");
    archive
        .chunks(PART)
        .enumerate()
        .map(|(index, chunk)| {
            let rel = format!("{rel_base}/installed.tar.{:03}", index + 1);
            h.put(&rel, chunk);
            json!({ "path": rel, "size": chunk.len(), "md5": md5_hex(chunk) })
        })
        .collect()
}

/// A pack entry for an installed copy of `layout` named `name`, whose
/// parts are `parts`.
#[allow(clippy::too_many_arguments)]
fn installed_game(
    rom_id: i64,
    title: &str,
    platform: &str,
    platform_name: &str,
    layout: &str,
    name: &str,
    launch: &str,
    parts: Vec<serde_json::Value>,
    row: serde_json::Value,
) -> PackGame {
    let size: u64 = parts.iter().map(|p| p["size"].as_u64().unwrap()).sum();
    serde_json::from_value(json!({
        "platform": platform,
        "platform_name": platform_name,
        "source": {
            "server": SERVER,
            "rom_id": rom_id,
            "fs_name": title,
            "server_updated_at": "2026-06-01T00:00:00Z",
            "cover_small_path": "/assets/romm/resources/roms/1/cover/small.png",
        },
        "name": title,
        "size": size,
        "added": "2026-11-09T00:00:00Z",
        "installed": {
            "archive": "tar",
            "layout": layout,
            "name": name,
            "parts": parts,
            "tree_bytes": 4096,
            "tree_files": 3,
            "launch": launch,
            "rom_file_name": format!("{title} (v1.01).zip"),
            "server_updated_at": "2026-01-01T00:00:00Z",
            "version": "v1.01",
            "outdated": true,
            "row": row,
        },
    }))
    .unwrap()
}

#[cfg(unix)]
fn assert_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "{} is executable", path.display());
}

#[cfg(not(unix))]
fn assert_executable(path: &Path) {
    assert!(path.is_file());
}

async fn install_and_wait(h: &Harness, game: &PackGame) -> DownloadEntry {
    h.service.install_from_drive(h.drive(), game, None).unwrap();
    h.wait_terminal(h.newest_entry_id()).await
}

#[tokio::test]
async fn an_extracted_copy_restores_into_the_platform_folder() {
    let h = Harness::new();
    let parts = put_installed(
        &h,
        "games/n64/Ember Run/Ember Run",
        &[
            dir("data/"),
            file("data/levels.bin", &[7u8; 1500]),
            file("Ember Run.z64", &[1u8; 900]),
        ],
    );
    let game = installed_game(
        8,
        "Ember Run",
        "n64",
        "Nintendo 64",
        "extracted",
        "Ember Run",
        "Ember Run.z64",
        parts,
        json!({ "ra_id": "123" }),
    );
    let entry = install_and_wait(&h, &game).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.kind, "base");
    assert_eq!(entry.drive_label, LABEL);

    let platform = h.library.join("games").join("Nintendo 64");
    let dest = platform.join("Ember Run");
    let launch = dest.join("Ember Run.z64");
    assert_eq!(fs::read(&launch).unwrap(), vec![1u8; 900]);
    assert_eq!(
        fs::read(dest.join("data/levels.bin")).unwrap(),
        vec![7u8; 1500]
    );
    assert_executable(&launch);
    assert!(!platform.join(".grid-restore-8").exists());

    let row = h.registry.find(Some(8), "", "").unwrap().unwrap();
    assert_eq!(row.rom_id, Some(8));
    assert_eq!(row.source_server, SERVER);
    assert_eq!(row.extracted_dir, dest.to_string_lossy());
    assert_eq!(row.extracted_path, launch.to_string_lossy());
    assert_eq!(row.rom_file_name, "Ember Run (v1.01).zip");
    assert_eq!(row.server_updated_at, "2026-01-01T00:00:00Z");
    assert_eq!(row.ra_id, "123");
    assert_eq!(
        row.cover_small_path,
        format!("{SERVER}/assets/romm/resources/roms/1/cover/small.png")
    );
    assert!(row.archive_path.is_empty());
}

#[tokio::test]
async fn a_ps4_copy_restores_with_its_eboot_and_title_id() {
    let h = Harness::new();
    let parts = put_installed(
        &h,
        "games/ps4/Velvet Static/CUSA00001",
        &[
            dir("CUSA00001/"),
            dir("CUSA00001/sce_sys/"),
            file("CUSA00001/sce_sys/param.sfo", &[3u8; 700]),
            file("CUSA00001/eboot.bin", &[4u8; 1200]),
        ],
    );
    let game = installed_game(
        9,
        "Velvet Static",
        "ps4",
        "PlayStation 4",
        "extracted",
        "CUSA00001",
        "CUSA00001/eboot.bin",
        parts,
        json!({ "ps4_game_id": "CUSA00001", "ps4_content": "{\"update\":\"1.01\"}" }),
    );
    let entry = install_and_wait(&h, &game).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let dest = h
        .library
        .join("games")
        .join("PlayStation 4")
        .join("CUSA00001");
    let eboot = dest.join("CUSA00001").join("eboot.bin");
    assert_executable(&eboot);
    let row = h.registry.find(Some(9), "", "").unwrap().unwrap();
    assert_eq!(row.extracted_dir, dest.to_string_lossy());
    assert_eq!(row.extracted_path, eboot.to_string_lossy());
    assert_eq!(row.ps4_game_id, "CUSA00001");
    assert_eq!(row.ps4_content, "{\"update\":\"1.01\"}");
}

#[tokio::test]
async fn a_native_copy_keeps_its_sidecar_and_gets_a_fresh_prefix() {
    let h = Harness::new();
    let parts = put_installed(
        &h,
        "games/win/My Game/My Game",
        &[
            file("game.json", br#"{"version": "1.2"}"#),
            dir("game/"),
            dir("game/MyGame/"),
            file("game/MyGame/mygame.exe", &[0x4du8; 1400]),
            file("game/MyGame/data.pak", &[9u8; 600]),
        ],
    );
    let game = installed_game(
        5,
        "My Game",
        "win",
        "Windows",
        "native",
        "My Game",
        "game/MyGame/mygame.exe",
        parts,
        json!({
            "native_executable": "game/MyGame/mygame.exe",
            "native_launch_parameters": "-windowed",
            "native_launch_source": "sidecar",
            "native_working_dir": "game/MyGame",
        }),
    );
    let entry = install_and_wait(&h, &game).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let game_dir = h.library.join("games").join("Windows").join("My Game");
    let exe = game_dir.join("game").join("MyGame").join("mygame.exe");
    assert!(game_dir.join("game.json").is_file(), "the sidecar is kept");
    assert_executable(&exe);
    let row = h.registry.find(Some(5), "", "").unwrap().unwrap();
    assert!(row.is_native());
    assert_eq!(row.native_game_dir, game_dir.to_string_lossy());
    assert_eq!(row.extracted_dir, game_dir.join("game").to_string_lossy());
    assert_eq!(row.extracted_path, exe.to_string_lossy());
    assert_eq!(row.native_executable_path, exe.to_string_lossy());
    assert_eq!(row.native_launch_parameters, "-windowed");
    assert_eq!(row.native_launch_source, "sidecar");
    assert_eq!(row.native_working_dir, "game/MyGame");
    assert_eq!(row.native_compat_tool, "");
    assert_eq!(row.revision, "1.2", "the sidecar's metadata applies");
    if cfg!(windows) {
        assert_eq!(row.native_wineprefix, "");
    } else {
        let prefix = game_dir.join("prefix");
        assert!(prefix.is_dir(), "a fresh prefix is created");
        assert_eq!(row.native_wineprefix, prefix.to_string_lossy());
    }
}

#[tokio::test]
async fn a_restored_out_of_date_copy_shows_a_server_update() {
    use grid_core::library::update_detection::{game_has_server_update, ServerVersion};
    let h = Harness::new();
    let parts = put_installed(
        &h,
        "games/n64/Ember Run/Ember Run",
        &[file("Ember Run.z64", &[1u8; 2000])],
    );
    let game = installed_game(
        8,
        "Ember Run",
        "n64",
        "Nintendo 64",
        "extracted",
        "Ember Run",
        "Ember Run.z64",
        parts,
        json!({}),
    );
    let entry = install_and_wait(&h, &game).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    let row = h.registry.find(Some(8), "", "").unwrap().unwrap();
    let server = ServerVersion {
        platform: "Nintendo 64",
        platform_slug: "n64",
        rom_file_name: "Ember Run (v1.02).zip",
        updated_at: &game.source.server_updated_at,
    };
    assert!(game_has_server_update(&row, &server));
}

#[tokio::test]
async fn a_damaged_part_fails_the_row_and_leaves_nothing_behind() {
    let h = Harness::new();
    let mut parts = put_installed(
        &h,
        "games/n64/Ember Run/Ember Run",
        &[
            file("data.bin", &[2u8; 1500]),
            file("Ember Run.z64", &[1u8; 900]),
        ],
    );
    parts[1]["md5"] = json!("0".repeat(32));
    let game = installed_game(
        8,
        "Ember Run",
        "n64",
        "Nintendo 64",
        "extracted",
        "Ember Run",
        "Ember Run.z64",
        parts,
        json!({}),
    );
    let entry = install_and_wait(&h, &game).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(
        entry.error,
        format!("The backup of Ember Run on {LABEL} is damaged")
    );
    assert!(h.registry.all().unwrap().is_empty(), "no registry row");
    assert_eq!(
        files_under(&h.library),
        Vec::<PathBuf>::new(),
        "no tree in the library"
    );
    let platform = h.library.join("games").join("Nintendo 64");
    assert!(!platform.join("Ember Run").exists());
    assert!(!platform.join(".grid-restore-8").exists());
}

#[tokio::test]
async fn a_copy_from_a_newer_grid_is_refused_before_admission() {
    let h = Harness::new();
    let parts = put_installed(
        &h,
        "games/n64/Ember Run/Ember Run",
        &[file("Ember Run.z64", &[1u8; 2000])],
    );
    let game = installed_game(
        8,
        "Ember Run",
        "n64",
        "Nintendo 64",
        "squashfs",
        "Ember Run",
        "Ember Run.z64",
        parts,
        json!({}),
    );
    let error = h
        .service
        .install_from_drive(h.drive(), &game, None)
        .unwrap_err();
    assert_eq!(error.to_string(), "This backup needs a newer GRID");
    assert!(h.service.snapshot().entries.is_empty());
}

/// The drive goes away in the middle of a restore (Unix only: Windows
/// refuses to delete a folder holding an open file).
#[cfg(unix)]
#[tokio::test]
async fn a_drive_removed_mid_restore_fails_the_row_and_leaves_nothing_behind() {
    let h = Harness::new();
    let parts = put_installed(
        &h,
        "games/n64/Ember Run/Ember Run",
        &[file("Ember Run.z64", &[1u8; 3000])],
    );
    let game = installed_game(
        8,
        "Ember Run",
        "n64",
        "Nintendo 64",
        "extracted",
        "Ember Run",
        "Ember Run.z64",
        parts,
        json!({}),
    );
    let mount = h.mount.clone();
    h.service.set_drive_copy_hook(Some(Arc::new(move || {
        let _ = fs::remove_dir_all(&mount);
        Err(io::Error::other("Input/output error"))
    })));
    let entry = install_and_wait(&h, &game).await;
    assert_eq!(entry.status, DownloadStatus::Failed);
    assert_eq!(entry.error, format!("{LABEL} was removed"));
    assert!(h.registry.all().unwrap().is_empty(), "no registry row");
    assert_eq!(files_under(&h.library), Vec::<PathBuf>::new());
    assert!(!h.library.join("games/Nintendo 64/.grid-restore-8").exists());
}
