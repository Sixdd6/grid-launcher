//! Library › RomM › Emulators: the server's Emulators platform as a list
//! (RommStorefront README "RomM › Emulators"; `docs/server-emulator-packages.md`).
//!
//! Every file of every folder lands in exactly one of three lists:
//!
//! - **Emulators**: the newest build for this computer of each emulator GRID
//!   knows, per folder. It installs through the server-emulator path
//!   ([`crate::library::InstallService::install_server_emulator`]) and
//!   registers, so Settings › Systems sets it up and offers its updates.
//! - **Other files**: files that don't register (not named to the rule, no
//!   arch tag or no processor in it, an emulator GRID doesn't know, an
//!   extension GRID doesn't unpack). They install as plain files in the
//!   library's Emulators folder.
//! - **Other builds**: named builds GRID doesn't install on this computer
//!   (another system or processor, or older than the newest).
//!
//! Pure: the caller reads the server and the disk.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::Config;
use crate::library::emulator_package::{
    compare_versions, newest_for_host, not_package_reason, parse_package_file, NotPackage,
    PackageFile, PackageOs, MULTI_FOLDER,
};
use crate::library::{server_emulator_source_id, server_entry_name};

use super::binary_target::{machine_label, Cpu, System};
use super::profiles::EmulatorProfile;
use super::setup::{profile_for_emulator, Recommendations};

/// What the view says for a multi-system emulator whose profile names no
/// system (RetroArch).
pub const MANY_SYSTEMS: &str = "Many systems, through its cores";

/// One file of a folder, as the server lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewFile {
    pub id: i64,
    pub name: String,
    pub size_bytes: i64,
}

/// One folder on the server's Emulators platform: one RomM ROM.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewPackage {
    pub rom_id: i64,
    /// The ROM's `fs_name`: a platform slug, or `multi`.
    pub folder: String,
    pub files: Vec<ViewFile>,
}

/// The whole view.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmulatorsView {
    /// This computer, in the view's words: "Linux x64".
    pub host: String,
    pub emulators: Vec<ViewEmulator>,
    pub other_files: Vec<ViewOtherFile>,
    pub other_builds: Vec<ViewOtherBuild>,
}

/// One emulator row: the newest build of an emulator GRID knows for this
/// computer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewEmulator {
    /// `romm:<folder>/<profile>`: the Downloads row's `source_id`.
    pub source_id: String,
    /// The emulator's short name: "PCSX2", "Pico-8".
    pub name: String,
    /// The GRID profile the file name named.
    pub profile_name: String,
    /// The systems it runs: "PlayStation 2", "GameCube, Wii", or
    /// [`MANY_SYSTEMS`].
    pub systems: String,
    /// Whether it runs many systems (RetroArch): "set up for its systems".
    pub many_systems: bool,
    /// The folder's platform slug for a single-system folder; `None` for
    /// `multi`.
    pub system_slug: Option<String>,
    /// From the file's `(v…)` tag, without the `v`.
    pub version: String,
    pub size_bytes: i64,
    pub file_name: String,
    pub folder: String,
    pub rom_id: i64,
    pub file_id: i64,
    /// The entry this build registered as, when installed (any version).
    pub entry: Option<String>,
    /// The version on disk, when installed.
    pub installed_version: Option<String>,
    /// Installed, and older than this build.
    pub update: bool,
    /// Not installed, and an entry not from the server already holds the
    /// profile's name (GRID's catalog copy): this build installs beside it.
    pub catalog_copy: bool,
    /// The entry name an install registers ("DuckStation (server)" beside a
    /// catalog copy).
    pub install_as: String,
}

/// Why a file is in Other files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OtherWhy {
    NotNamed,
    NoArchTag,
    NoProcessor,
    /// The emulator name as the file spells it.
    UnknownEmulator(String),
    /// The extension with its dot: `.rar`.
    Extension(String),
}

/// One Other files row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewOtherFile {
    pub rom_id: i64,
    pub file_id: i64,
    pub file_name: String,
    pub folder: String,
    pub size_bytes: i64,
    pub why: OtherWhy,
    /// On disk in the library's Emulators folder.
    pub installed: bool,
}

/// Why GRID doesn't install a build on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OtherReason {
    /// For another computer: "Windows x64", "macOS (universal)".
    For(String),
    /// Older than the newest build here: that build's version.
    Older(String),
}

/// One Other builds row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewOtherBuild {
    pub file_name: String,
    pub folder: String,
    pub size_bytes: i64,
    pub reason: OtherReason,
}

impl EmulatorsView {
    /// The rail row's count: what this computer can install.
    pub fn count(&self) -> usize {
        self.emulators.len() + self.other_files.len()
    }
}

/// Sorts every file of `packages` into the view, for the `(os, cpu)` computer
/// `host` names.
///
/// - `platform_labels`: the server's platforms, slug → label, for a
///   single-system folder's "PlayStation 2".
/// - `file_installed(folder, file_name)`: whether an Other file is on disk.
pub fn emulators_view(
    packages: &[ViewPackage],
    profiles: &[EmulatorProfile],
    config: &Config,
    table: &Recommendations,
    platform_labels: &BTreeMap<String, String>,
    host: (PackageOs, Cpu),
    file_installed: &dyn Fn(&str, &str) -> bool,
) -> EmulatorsView {
    let (os, cpu) = host;
    let host_system = match os {
        PackageOs::Linux => System::Linux,
        PackageOs::Windows => System::Windows,
        PackageOs::Macos => System::Macos,
    };
    let mut view = EmulatorsView {
        host: machine_label(host_system, &[cpu]),
        emulators: Vec::new(),
        other_files: Vec::new(),
        other_builds: Vec::new(),
    };

    for package in packages {
        let folder = package.folder.trim().to_lowercase();
        let mut parsed: Vec<(&ViewFile, PackageFile)> = Vec::new();
        for file in &package.files {
            match parse_package_file(&file.name) {
                Some(build) => parsed.push((file, build)),
                None => view.other_files.push(other_file(
                    package,
                    &folder,
                    file,
                    match not_package_reason(&file.name) {
                        NotPackage::NotNamed => OtherWhy::NotNamed,
                        NotPackage::NoArchTag => OtherWhy::NoArchTag,
                        NotPackage::NoProcessor => OtherWhy::NoProcessor,
                        NotPackage::Extension(ext) => OtherWhy::Extension(ext),
                    },
                    file_installed,
                )),
            }
        }

        // A build of an emulator GRID doesn't know: an Other file when it is
        // for this computer, else just a build for another one.
        let (known, unknown): (Vec<_>, Vec<_>) = parsed
            .into_iter()
            .partition(|(_, b)| profile_for_emulator(&b.emulator, profiles, table).is_some());
        for (file, build) in unknown {
            if build.runs_on(os, cpu) {
                view.other_files.push(other_file(
                    package,
                    &folder,
                    file,
                    OtherWhy::UnknownEmulator(build.emulator.clone()),
                    file_installed,
                ));
            } else {
                view.other_builds.push(other_build(&folder, file, &build));
            }
        }

        let builds: Vec<PackageFile> = known.iter().map(|(_, b)| b.clone()).collect();
        let newest: Vec<PackageFile> = newest_for_host(&builds, os, cpu)
            .into_iter()
            .cloned()
            .collect();
        for (file, build) in &known {
            let chosen = newest
                .iter()
                .find(|n| n.emulator.eq_ignore_ascii_case(&build.emulator));
            match chosen {
                Some(n) if n.file_name == build.file_name => {
                    let Some(profile) = profile_for_emulator(&build.emulator, profiles, table)
                    else {
                        continue;
                    };
                    view.emulators.push(emulator_row(
                        package,
                        &folder,
                        file,
                        build,
                        profile,
                        config,
                        platform_labels,
                    ));
                }
                Some(n)
                    if build.runs_on(os, cpu)
                        && compare_versions(&build.version, &n.version) == Ordering::Less =>
                {
                    view.other_builds.push(ViewOtherBuild {
                        file_name: file.name.clone(),
                        folder: folder.clone(),
                        size_bytes: file.size_bytes,
                        reason: OtherReason::Older(n.version.clone()),
                    });
                }
                _ => view.other_builds.push(other_build(&folder, file, build)),
            }
        }
    }

    let key = |s: &str| s.to_lowercase();
    view.emulators.sort_by(|a, b| {
        key(&a.name)
            .cmp(&key(&b.name))
            .then(a.folder.cmp(&b.folder))
    });
    view.other_files.sort_by_key(|f| key(&f.file_name));
    view.other_builds.sort_by_key(|b| key(&b.file_name));
    view
}

fn other_file(
    package: &ViewPackage,
    folder: &str,
    file: &ViewFile,
    why: OtherWhy,
    file_installed: &dyn Fn(&str, &str) -> bool,
) -> ViewOtherFile {
    ViewOtherFile {
        rom_id: package.rom_id,
        file_id: file.id,
        file_name: file.name.clone(),
        folder: folder.to_string(),
        size_bytes: file.size_bytes,
        installed: file_installed(folder, &file.name),
        why,
    }
}

/// A build for another computer (or, at the same version, the universal
/// build a Mac's own build wins over).
fn other_build(folder: &str, file: &ViewFile, build: &PackageFile) -> ViewOtherBuild {
    let (system, cpus) = build.machine();
    ViewOtherBuild {
        file_name: file.name.clone(),
        folder: folder.to_string(),
        size_bytes: file.size_bytes,
        reason: OtherReason::For(machine_label(system, &cpus)),
    }
}

fn emulator_row(
    package: &ViewPackage,
    folder: &str,
    file: &ViewFile,
    build: &PackageFile,
    profile: &EmulatorProfile,
    config: &Config,
    platform_labels: &BTreeMap<String, String>,
) -> ViewEmulator {
    let full = profile.name.trim();
    let (short, systems_in_name) = match full.split_once(" (") {
        Some((short, rest)) => (short.trim(), Some(rest.trim_end_matches(')').trim())),
        None => (full, None),
    };
    let many_systems = profile.all_platforms
        || systems_in_name.is_some_and(|s| s.eq_ignore_ascii_case("multi-system"));
    let single = folder != MULTI_FOLDER;
    let systems = if many_systems {
        MANY_SYSTEMS.to_string()
    } else if let Some(label) = single.then(|| platform_labels.get(folder)).flatten() {
        label.clone()
    } else {
        systems_in_name.unwrap_or(short).to_string()
    };

    let source_id = server_emulator_source_id(folder, &profile.name);
    let entry = config.emulators.iter().find(|e| e.source_id == source_id);
    let installed_version = entry.map(|e| {
        if e.source_installed_tag.trim().is_empty() {
            e.source_release_tag.trim().to_string()
        } else {
            e.source_installed_tag.trim().to_string()
        }
    });
    let update = installed_version.as_deref().is_some_and(|have| {
        !have.is_empty()
            && compare_versions(have.trim_start_matches('v'), &build.version) == Ordering::Less
    });
    let install_as = server_entry_name(config, &profile.name, &source_id);
    let catalog_copy = entry.is_none() && install_as != profile.name;

    ViewEmulator {
        source_id,
        name: short.to_string(),
        profile_name: profile.name.clone(),
        systems,
        many_systems,
        system_slug: single.then(|| folder.to_string()),
        version: build.version.clone(),
        size_bytes: file.size_bytes,
        file_name: file.name.clone(),
        folder: folder.to_string(),
        rom_id: package.rom_id,
        file_id: file.id,
        entry: entry.map(|e| e.name.clone()),
        installed_version,
        update,
        catalog_copy,
        install_as,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EmulatorEntry;

    fn profile(name: &str) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            ..Default::default()
        }
    }

    fn profiles() -> Vec<EmulatorProfile> {
        let mut retro = profile("RetroArch (Multi-System)");
        retro.all_platforms = true;
        vec![
            profile("PCSX2 (Playstation 2)"),
            profile("Pico-8"),
            retro,
            profile("Ymir (Sega Saturn)"),
            profile("DuckStation (Playstation 1)"),
            profile("Dolphin (GameCube, Wii)"),
        ]
    }

    fn file(id: i64, name: &str) -> ViewFile {
        ViewFile {
            id,
            name: name.to_string(),
            size_bytes: id * 1000,
        }
    }

    fn packages() -> Vec<ViewPackage> {
        vec![
            ViewPackage {
                rom_id: 1,
                folder: "ps2".into(),
                files: vec![
                    file(11, "PCSX2 (v2.0.2) (arch-linux-x64).AppImage"),
                    file(12, "PCSX2 (v2.0.2) (arch-windows-x64).zip"),
                    file(13, "PCSX2 (v1.6.0) (arch-linux-x64).AppImage"),
                ],
            },
            ViewPackage {
                rom_id: 2,
                folder: "multi".into(),
                files: vec![
                    file(21, "RetroArch (v1.19.1) (arch-linux-x64).AppImage"),
                    file(22, "Dolphin (v2412) (arch-windows-x64).7z"),
                    file(23, "Dolphin (v2412) (arch-macos-universal).7z"),
                    file(24, "Ares Nightly (v141) (arch-linux-x64).AppImage"),
                    file(25, "Ares Nightly (v141) (arch-windows-x64).zip"),
                ],
            },
            ViewPackage {
                rom_id: 3,
                folder: "xbox".into(),
                files: vec![file(31, "xemu-v0.8.5-x86_64.AppImage")],
            },
            ViewPackage {
                rom_id: 4,
                folder: "wiiu".into(),
                files: vec![
                    file(41, "Cemu (v2.4) (arch-linux).AppImage"),
                    file(42, "Cemu (v2.4).AppImage"),
                    file(43, "Cemu (v2.4) (arch-linux-x64).rar"),
                ],
            },
            ViewPackage {
                rom_id: 5,
                folder: "psx".into(),
                files: vec![file(51, "DuckStation (v0.1.9226) (arch-linux-x64).zip")],
            },
        ]
    }

    fn labels() -> BTreeMap<String, String> {
        [("ps2", "PlayStation 2"), ("psx", "PlayStation")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    fn view(config: &Config, os: PackageOs, cpu: Cpu) -> EmulatorsView {
        let installed = |folder: &str, name: &str| folder == "xbox" && name.starts_with("xemu");
        emulators_view(
            &packages(),
            &profiles(),
            config,
            &Recommendations::default(),
            &labels(),
            (os, cpu),
            &installed,
        )
    }

    #[test]
    fn every_file_lands_in_one_list_for_a_linux_pc() {
        let v = view(&Config::default(), PackageOs::Linux, Cpu::X64);
        assert_eq!(v.host, "Linux x64");
        let names: Vec<&str> = v.emulators.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["DuckStation", "PCSX2", "RetroArch"]);
        let pcsx2 = &v.emulators[1];
        assert_eq!(pcsx2.systems, "PlayStation 2");
        assert_eq!(pcsx2.version, "2.0.2");
        assert_eq!(pcsx2.file_id, 11);
        assert_eq!(pcsx2.source_id, "romm:ps2/pcsx2 (playstation 2)");
        assert_eq!(pcsx2.system_slug.as_deref(), Some("ps2"));
        assert_eq!(v.emulators[2].systems, MANY_SYSTEMS);
        assert!(v.emulators[2].many_systems);
        assert_eq!(v.emulators[2].system_slug, None);

        let whys: Vec<(&str, &OtherWhy)> = v
            .other_files
            .iter()
            .map(|f| (f.file_name.as_str(), &f.why))
            .collect();
        assert_eq!(
            whys,
            [
                (
                    "Ares Nightly (v141) (arch-linux-x64).AppImage",
                    &OtherWhy::UnknownEmulator("Ares Nightly".into())
                ),
                ("Cemu (v2.4) (arch-linux).AppImage", &OtherWhy::NoProcessor),
                (
                    "Cemu (v2.4) (arch-linux-x64).rar",
                    &OtherWhy::Extension(".rar".into())
                ),
                ("Cemu (v2.4).AppImage", &OtherWhy::NoArchTag),
                ("xemu-v0.8.5-x86_64.AppImage", &OtherWhy::NotNamed),
            ]
        );
        assert!(
            v.other_files
                .iter()
                .find(|f| f.folder == "xbox")
                .unwrap()
                .installed
        );
        assert!(!v.other_files[0].installed);

        let reasons: Vec<(&str, &OtherReason)> = v
            .other_builds
            .iter()
            .map(|b| (b.file_name.as_str(), &b.reason))
            .collect();
        assert_eq!(
            reasons,
            [
                (
                    "Ares Nightly (v141) (arch-windows-x64).zip",
                    &OtherReason::For("Windows x64".into())
                ),
                (
                    "Dolphin (v2412) (arch-macos-universal).7z",
                    &OtherReason::For("macOS (universal)".into())
                ),
                (
                    "Dolphin (v2412) (arch-windows-x64).7z",
                    &OtherReason::For("Windows x64".into())
                ),
                (
                    "PCSX2 (v1.6.0) (arch-linux-x64).AppImage",
                    &OtherReason::Older("2.0.2".into())
                ),
                (
                    "PCSX2 (v2.0.2) (arch-windows-x64).zip",
                    &OtherReason::For("Windows x64".into())
                ),
            ]
        );
        assert_eq!(v.count(), 8);
    }

    #[test]
    fn a_mac_takes_the_universal_build_and_nothing_else_is_for_a_raspberry_pi() {
        let mac = view(&Config::default(), PackageOs::Macos, Cpu::Arm64);
        assert_eq!(mac.host, "macOS (Apple silicon)");
        let dolphin = mac.emulators.iter().find(|e| e.name == "Dolphin").unwrap();
        assert_eq!(dolphin.systems, "GameCube, Wii");
        assert_eq!(dolphin.file_id, 23);

        let pi = view(&Config::default(), PackageOs::Linux, Cpu::Arm);
        assert!(pi.emulators.is_empty());
        assert_eq!(pi.host, "Linux 32-bit ARM");
    }

    #[test]
    fn installed_update_and_catalog_copy_states() {
        let mut config = Config::default();
        config.emulators.push(EmulatorEntry {
            name: "PCSX2 (Playstation 2)".into(),
            source_id: "romm:ps2/pcsx2 (playstation 2)".into(),
            source_installed_tag: "1.6.0".into(),
            ..Default::default()
        });
        config.emulators.push(EmulatorEntry {
            name: "RetroArch (Multi-System)".into(),
            source_id: "romm:multi/retroarch (multi-system)".into(),
            source_installed_tag: "1.19.1".into(),
            ..Default::default()
        });
        config.emulators.push(EmulatorEntry {
            name: "DuckStation (Playstation 1)".into(),
            source_id: "stenzek/duckstation".into(),
            ..Default::default()
        });
        let v = view(&config, PackageOs::Linux, Cpu::X64);
        let get = |n: &str| v.emulators.iter().find(|e| e.name == n).unwrap();
        let pcsx2 = get("PCSX2");
        assert_eq!(pcsx2.installed_version.as_deref(), Some("1.6.0"));
        assert!(pcsx2.update);
        assert!(!pcsx2.catalog_copy);
        let retro = get("RetroArch");
        assert!(retro.entry.is_some());
        assert!(!retro.update);
        let duck = get("DuckStation");
        assert!(duck.entry.is_none());
        assert!(duck.catalog_copy);
        assert_eq!(duck.install_as, "DuckStation (Playstation 1) (server)");
    }
}
