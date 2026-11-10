//! A Proton compat tool installed from a backup drive brings the drive's
//! umu-launcher when this computer has no `umu-run`, and the drive's Steam
//! Linux Runtime when umu has none, with no network at all.
//!
//! Its own test binary, on purpose (same pattern as `umu_launcher_install.rs`):
//! `compat::managed_root` reads `GRID_LAUNCHER_DATA_DIR`, the "is a umu-run
//! available" check reads `PATH`, umu's data folder comes from `HOME`, and
//! `std::env::set_var` is process-global.
//! One test in one binary means no sibling test can observe them.
#![cfg(target_os = "linux")]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use grid_core::backup::drives::drive_key;
use grid_core::backup::pack::{self, PackEmulator, PackOs};
use grid_core::config::Config;
use grid_core::launch::profiles::EmulatorProfile;
use grid_core::launch::umu::UMU_SOURCE_ID;
use grid_core::library::backup_job::DriveTarget;
use grid_core::library::queue::{DownloadEntry, DownloadStatus};
use grid_core::library::registry::Registry;
use grid_core::library::InstallService;
use serde_json::json;

fn tar_with(path: &str, content: &[u8], gzip: bool) -> Vec<u8> {
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o755);
    header.set_path(path).unwrap();
    header.set_cksum();
    if gzip {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        builder.append(&header, content).unwrap();
        builder.into_inner().unwrap().finish().unwrap()
    } else {
        let mut builder = tar::Builder::new(Vec::new());
        builder.append(&header, content).unwrap();
        builder.into_inner().unwrap()
    }
}

/// A Proton compat-tool profile. Nothing is ever fetched from its source.
fn proton_profile() -> EmulatorProfile {
    EmulatorProfile {
        name: "GE-Proton".to_string(),
        is_compat_tool: true,
        all_platforms: true,
        source: Some(json!({
            "provider": "gitea",
            "owner": "GloriousEggroll",
            "repo": "proton-ge-custom",
            "base_url": "http://127.0.0.1:9",
            "release_tag": "latest",
        })),
        compat_tool_type: "proton".to_string(),
        ..Default::default()
    }
}

/// A `SteamLinuxRuntime_sniper.tar.xz` stand-in: the top folder with the
/// entry point umu links to, its VERSIONS.txt and pressure-vessel.
fn runtime_tar_xz() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, body) in [
        (
            "SteamLinuxRuntime_sniper/_v2-entry-point",
            &b"#!/bin/sh\n"[..],
        ),
        (
            "SteamLinuxRuntime_sniper/VERSIONS.txt",
            &b"#Name\tVersion\nsniper\t3.0.1\n"[..],
        ),
        (
            "SteamLinuxRuntime_sniper/pressure-vessel/bin/pv-verify",
            &b"#!/bin/sh\n"[..],
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o755);
        header.set_path(path).unwrap();
        header.set_cksum();
        builder.append(&header, body).unwrap();
    }
    let tar = builder.into_inner().unwrap();
    let mut encoder = liblzma::write::XzEncoder::new(Vec::new(), 1);
    std::io::Write::write_all(&mut encoder, &tar).unwrap();
    encoder.finish().unwrap()
}

fn put(mount: &Path, rel: &str, body: &[u8]) {
    let path = pack::resolve(mount, rel).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn entry(name: &str, source_id: &str, version: &str, rel: &str, asset: &str) -> PackEmulator {
    PackEmulator {
        name: name.to_string(),
        kind: "compat_tool".to_string(),
        source_id: source_id.to_string(),
        version: version.to_string(),
        program: String::new(),
        args: String::new(),
        os: PackOs::Linux.arch_tag().to_string(),
        platforms: Vec::new(),
        path: rel.to_string(),
        supplementals: Vec::new(),
        asset_name: asset.to_string(),
        size: 0,
        extra: Default::default(),
    }
}

async fn wait_terminal(service: &Arc<InstallService>, id: u64) -> DownloadEntry {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(entry) = service.snapshot().entries.iter().find(|e| e.id == id) {
            if matches!(
                entry.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            ) {
                return entry.clone();
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting on entry {id}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn newest_row(service: &Arc<InstallService>, source_id: &str) -> Option<DownloadEntry> {
    service
        .snapshot()
        .entries
        .into_iter()
        .filter(|e| e.source_id == source_id)
        .max_by_key(|e| e.id)
}

#[tokio::test]
async fn a_proton_tool_from_a_drive_brings_the_drives_umu_launcher() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();
    let empty_path = tmp.path().join("empty-path");
    fs::create_dir_all(&empty_path).unwrap();
    // SAFETY (test-only, single test in this binary): nothing else in this
    // process reads or mutates these concurrently.
    std::env::set_var("GRID_LAUNCHER_DATA_DIR", &data_dir);
    std::env::set_var("PATH", &empty_path);
    // umu's data folder: <HOME>/.local/share/umu, in a temp HOME.
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    std::env::remove_var("XDG_DATA_HOME");
    std::env::remove_var("UMU_FOLDERS_PATH");
    let umu_local = home.join(".local").join("share").join("umu");
    // umu-run makes an empty steamrt3 before it downloads: still missing.
    fs::create_dir_all(umu_local.join("steamrt3")).unwrap();

    let library = tmp.path().join("library");
    fs::create_dir_all(&library).unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        format!(
            "schema_version = 1\nlibrary_path = {:?}\n",
            library.to_string_lossy()
        ),
    )
    .unwrap();

    let mount = tmp.path().join("media").join("SAMSUNG T7");
    fs::create_dir_all(&mount).unwrap();
    let mut kit = pack::new_pack("Couch kit", &[PackOs::Linux]);
    let proton_rel = "emulators/GE-Proton/GE-Proton10-1/linux-x86_64/GE-Proton10-1.tar.gz";
    let umu_rel = "emulators/umu-launcher/1.2.6/linux-x86_64/umu-launcher-1.2.6-zipapp.tar";
    put(
        &mount,
        proton_rel,
        &tar_with("GE-Proton10-1/proton", b"#!/bin/sh\n", true),
    );
    put(
        &mount,
        umu_rel,
        &tar_with("umu/umu-run", b"#!/usr/bin/env python3\n", false),
    );
    let proton = entry(
        "GE-Proton",
        "GloriousEggroll/proton-ge-custom",
        "GE-Proton10-1",
        proton_rel,
        "GE-Proton10-1.tar.gz",
    );
    let umu = entry(
        "umu-launcher",
        UMU_SOURCE_ID,
        "1.2.6",
        umu_rel,
        "umu-launcher-1.2.6-zipapp.tar",
    );
    let runtime_rel = "emulators/steam-runtime/3.0.1/linux-x86_64/SteamLinuxRuntime_sniper.tar.xz";
    put(&mount, runtime_rel, &runtime_tar_xz());
    let runtime = entry(
        "steam-runtime",
        grid_core::backup::steam_runtime::RUNTIME_SOURCE_ID,
        "3.0.1",
        runtime_rel,
        "SteamLinuxRuntime_sniper.tar.xz",
    );
    kit.emulators = vec![proton.clone(), umu, runtime];
    pack::write_pack_atomic(&mount, &kit).unwrap();

    let registry = Arc::new(Registry::open(&tmp.path().join("registry.db")).unwrap());
    let service =
        InstallService::with_profiles(registry, config_path.clone(), vec![proton_profile()]);
    let drive = DriveTarget {
        key: drive_key(&mount),
        mount: mount.clone(),
        label: "SAMSUNG T7".to_string(),
        name: "Couch kit".to_string(),
        fat32: false,
    };

    service.install_emulator_from_drive(drive, &proton).unwrap();
    let proton_row =
        newest_row(&service, "GloriousEggroll/proton-ge-custom").expect("the Proton row");
    let umu_row = newest_row(&service, UMU_SOURCE_ID).expect("umu-launcher from the drive");
    assert!(proton_row.id < umu_row.id, "the tool stays first");
    assert_eq!(umu_row.drive_label, "SAMSUNG T7");

    let proton_done = wait_terminal(&service, proton_row.id).await;
    assert_eq!(
        proton_done.status,
        DownloadStatus::Completed,
        "{}",
        proton_done.error
    );
    let umu_done = wait_terminal(&service, umu_row.id).await;
    assert_eq!(
        umu_done.status,
        DownloadStatus::Completed,
        "{}",
        umu_done.error
    );

    let config = Config::load(&config_path).unwrap();
    assert!(config
        .compat_tool_installs
        .iter()
        .any(|install| install.source_id == "GloriousEggroll/proton-ge-custom"));
    let record = config.umu_launcher.expect("a umu-launcher record");
    let umu_run = data_dir
        .join("compat-tools")
        .join("umu-launcher")
        .join("umu")
        .join("umu-run");
    assert_eq!(Path::new(&record.path), umu_run);
    assert!(umu_run.is_file());
    assert_eq!(record.release_tag, "1.2.6");

    // The runtime went where umu-run looks, laid out as umu lays it out.
    let runtime_row = newest_row(
        &service,
        grid_core::backup::steam_runtime::RUNTIME_SOURCE_ID,
    )
    .expect("the Steam Linux Runtime from the drive");
    assert!(umu_row.id < runtime_row.id);
    assert_eq!(runtime_row.title, "Steam Linux Runtime");
    let runtime_done = wait_terminal(&service, runtime_row.id).await;
    assert_eq!(
        runtime_done.status,
        DownloadStatus::Completed,
        "{}",
        runtime_done.error
    );
    let steamrt3 = umu_local.join("steamrt3");
    assert!(steamrt3.join("_v2-entry-point").is_file());
    assert!(steamrt3.join("VERSIONS.txt").is_file());
    assert!(steamrt3.join("pressure-vessel/bin/pv-verify").is_file());
    assert_eq!(
        fs::read_link(steamrt3.join("umu")).unwrap(),
        Path::new("_v2-entry-point")
    );
    // umu writes its marker itself, after pv-verify passes.
    assert!(!steamrt3.join(".installed.ok").exists());
    let leftovers: Vec<_> = fs::read_dir(&umu_local)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, vec!["steamrt3".to_string()]);

    // With umu-run now present, a second install of the tool brings nothing.
    let before = service.snapshot().entries.len();
    let again = DriveTarget {
        key: drive_key(&mount),
        mount: mount.clone(),
        label: "SAMSUNG T7".to_string(),
        name: "Couch kit".to_string(),
        fat32: false,
    };
    service.install_emulator_from_drive(again, &proton).unwrap();
    let rows: Vec<_> = service
        .snapshot()
        .entries
        .into_iter()
        .filter(|e| e.source_id == UMU_SOURCE_ID)
        .collect();
    assert_eq!(rows.len(), 1, "no second umu-launcher row");
    let runtime_rows = service
        .snapshot()
        .entries
        .into_iter()
        .filter(|e| e.source_id == grid_core::backup::steam_runtime::RUNTIME_SOURCE_ID)
        .count();
    assert_eq!(runtime_rows, 1, "no second runtime row");
    assert!(service.snapshot().entries.len() > before);
}
