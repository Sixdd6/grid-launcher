//! Firmware without RomM (romm-storefront-plan Step 7): GRID's known table
//! cross-checked against RomM 5.4.1's, and Locate files end to end through
//! `install_platform_firmware_from(LocalFirmwareSource)`.
//!
//! The fixtures are fake files with fake hashes. No real BIOS content is
//! written anywhere; the cross-check reads only names, sizes and hashes.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use grid_core::autoconfig::cores::core_entries;
use grid_core::firmware::known::{files_for, known_files, parse_known, KnownFile};
use grid_core::firmware::local::{
    expected_set, finish_report, plan_locate, LocalFirmwareSource, LocateNames,
};
use grid_core::firmware::manifest::{FirmwareManifest, MANIFEST_FILE};
use grid_core::firmware::routing::shape_for_retroarch;
use grid_core::firmware::{install_platform_firmware_from, FirmwareOptions, FirmwareTarget};
use md5::{Digest, Md5};
use serde::Deserialize;

/// RomM 5.4.1's `backend/models/fixtures/known_bios_files.json` (AGPL-3.0),
/// copied verbatim: names, sizes and hashes only.
const ROMM_KNOWN: &str = include_str!("fixtures/romm-5.4.1-known_bios_files.json");

#[derive(Deserialize)]
struct RommEntry {
    size: String,
    md5: String,
}

/// RomM keys that name no RomM platform, and the slug GRID files them under.
const DEAD_KEYS: &[(&str, &str)] = &[("odyssey-2-slash-videopac-g7000", "odyssey-2")];

/// Files GRID keeps without a checksum on purpose (research: per-console
/// files are never given a hash; Complex_4627.bin's md5 differs between
/// RomM and Batocera, so only its size is checked).
const NAME_ONLY: &[(&str, &str)] = &[("nds", "firmware.bin"), ("xbox", "complex_4627.bin")];

/// GRID's verdict for a file RomM verifies, by RomM's rule (`models/
/// firmware.py`: the key exists, the size equals, and a hash matches).
#[test]
fn every_file_romm_verifies_for_a_catalog_platform_grid_verifies_too() {
    let romm: BTreeMap<String, RommEntry> = serde_json::from_str(ROMM_KNOWN).unwrap();
    let catalog: Vec<String> = grid_core::systems::catalog()
        .iter()
        .map(|p| p.slug.clone())
        .collect();
    let known = known_files();
    let mut checked = 0;
    for (key, entry) in &romm {
        let (slug, name) = key.split_once(':').unwrap();
        let slug = DEAD_KEYS
            .iter()
            .find(|(d, _)| *d == slug)
            .map_or(slug, |(_, s)| *s);
        if !catalog.iter().any(|c| c == slug) {
            continue;
        }
        let lower = name.to_lowercase();
        let grid: Vec<&KnownFile> = files_for(known, slug, None)
            .into_iter()
            .filter(|f| f.name.to_lowercase() == lower)
            .collect();
        assert!(!grid.is_empty(), "GRID does not know {key}");
        if NAME_ONLY.contains(&(slug, lower.as_str())) {
            assert!(
                grid.iter().all(|f| !f.has_hash()),
                "{key} must stay name-only"
            );
            continue;
        }
        let size: u64 = entry.size.parse().unwrap();
        assert!(
            grid.iter()
                .any(|f| f.size == size && f.md5 == entry.md5.to_lowercase()),
            "GRID's {key} differs from RomM's"
        );
        checked += 1;
    }
    assert!(checked > 250, "only {checked} RomM entries were checked");
}

fn md5_of(data: &[u8]) -> String {
    Md5::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A test-only table, like the one the e2e suite points the app at.
fn fake_table() -> Vec<KnownFile> {
    let boot = b"FAKE-DC-BOOT-not-real";
    let flash = b"FAKE-DC-FLASH-not-real";
    parse_known(&format!(
        r#"{{"files":[
          {{"slug":"dc","name":"dc_boot.bin","path_hint":"dc/","size":{},"md5":"{}","required":true,"group":"dc-1"}},
          {{"slug":"dc","name":"dc_flash.bin","path_hint":"dc/","size":{},"md5":"{}","required":false}}
        ]}}"#,
        boot.len(),
        md5_of(boot),
        flash.len(),
        md5_of(flash)
    ))
    .unwrap()
}

#[tokio::test]
async fn locate_files_installs_into_a_retroarch_layout_and_the_manifest_says_installed() {
    let root = tempfile::tempdir().unwrap();
    let retroarch = root.path().join("RetroArch");
    let system = retroarch.join("system");
    fs::create_dir_all(&system).unwrap();
    let plan = shape_for_retroarch(
        "flycast",
        core_entries(),
        &retroarch,
        "",
        vec![FirmwareTarget {
            path: system.clone(),
            keywords: None,
        }],
    )
    .expect("flycast has firmware metadata");
    let targets = plan.firmware.clone();
    assert!(targets.iter().all(|t| t.path.ends_with("dc")));

    let table = fake_table();
    let known: Vec<&KnownFile> = table.iter().collect();
    let manifest_path = root.path().join(MANIFEST_FILE);
    let expected = expected_set(
        &known,
        &[],
        &targets,
        &FirmwareManifest::load(&manifest_path),
    );
    assert!(expected
        .files
        .iter()
        .any(|f| f.name == "dc_boot.bin" && f.required && !f.installed));

    // A renamed dump and an unknown file, picked by hand.
    let picks = root.path().join("Downloads");
    fs::create_dir_all(&picks).unwrap();
    let renamed = picks.join("Dreamcast BIOS (1).bin");
    fs::write(&renamed, b"FAKE-DC-BOOT-not-real").unwrap();
    let unknown = picks.join("notes.txt");
    fs::write(&unknown, b"x").unwrap();
    let names = LocateNames {
        system: "Dreamcast",
        emulator: "RetroArch",
    };
    let located = plan_locate(&[renamed, unknown], &expected.files, &names);
    assert_eq!(located.installs.len(), 1);

    let source = LocalFirmwareSource::from_installs(&located.installs);
    let opts = FirmwareOptions {
        skip_existing: false,
        extract_zip_with_paths: plan.extract_with_paths,
    };
    let warnings =
        install_platform_firmware_from(&source, &targets, opts, Some(&manifest_path)).await;
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        fs::read(system.join("dc").join("dc_boot.bin")).unwrap(),
        b"FAKE-DC-BOOT-not-real"
    );

    let manifest = FirmwareManifest::load(&manifest_path);
    let report = finish_report(located, &targets, &manifest, "not placed");
    assert_eq!(report.installed.len(), 1);
    assert_eq!(report.installed[0].as_name, "dc_boot.bin");
    assert_eq!(report.installed[0].file, "Dreamcast BIOS (1).bin");
    assert_eq!(report.refused.len(), 1);
    assert_eq!(
        report.refused[0].reason,
        "GRID doesn't know this file for Dreamcast"
    );

    let after = expected_set(&known, &[], &targets, &manifest);
    let boot = after
        .files
        .iter()
        .find(|f| f.name == "dc_boot.bin")
        .unwrap();
    assert!(boot.installed && !boot.required);
}

#[test]
fn no_emulator_means_no_targets_and_nothing_routes() {
    // Without an emulator there are no targets: the expected set is the
    // whole table for the system, and nothing is installed.
    let table = fake_table();
    let known: Vec<&KnownFile> = table.iter().collect();
    let set = expected_set(&known, &[], &[], &FirmwareManifest::default());
    assert_eq!(set.files.len(), 2);
    assert!(set.files.iter().all(|f| !f.installed));
    let _ = PathBuf::new();
}
