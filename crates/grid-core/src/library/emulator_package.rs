//! The packaging standard for emulators hosted on a RomM server
//! (`docs/server-emulator-packages.md`): reading a build's file name, comparing
//! versions, and picking the newest build for this system.
//!
//! Under RomM's Emulators platform each platform has a folder named by its
//! slug (`ps2`, `pico`), plus one shared `multi` folder. RomM sees each folder
//! as one multi-file ROM and parses tags only from a ROM's own name, so GRID
//! reads every build's tags from its file name here:
//!
//! ```text
//! <Emulator> (v<version>) (arch-<system>-<cpu>).<extension>
//! ```
//!
//! `<system>` is `linux`, `windows` or `macos`; `<cpu>` is `x64`, `arm64`,
//! `armhf` or (macOS only) `universal`.
//!
//! A file that does not follow this is not a package build: [`parse_package_file`]
//! answers `None` and the file is ignored.

use std::cmp::Ordering;

use crate::launch::binary_target::Cpu;

/// The folder under the Emulators platform for emulators that serve several
/// platforms.
pub const MULTI_FOLDER: &str = "multi";

/// The system a build runs on, from its `(arch-<system>-…)` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageOs {
    Linux,
    Windows,
    Macos,
}

impl PackageOs {
    /// The `sys.platform`-shaped host slug GRID uses elsewhere
    /// (`launch::source::HOST_PLATFORM`): `linux`, `win32`, `darwin`.
    pub fn for_host(host: &str) -> Option<PackageOs> {
        if host.starts_with("linux") {
            Some(PackageOs::Linux)
        } else if host.starts_with("win") {
            Some(PackageOs::Windows)
        } else if host.starts_with("darwin") {
            Some(PackageOs::Macos)
        } else {
            None
        }
    }
}

/// The processor a build runs on, from its `(arch-…-<cpu>)` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageCpu {
    /// 64-bit x86 (`x64`; also read: `x86_64`, `amd64`).
    X64,
    /// 64-bit ARM (`arm64`; also read: `aarch64`).
    Arm64,
    /// 32-bit ARM with hardware floating point (`armhf`): Raspberry Pi OS's
    /// 32-bit builds.
    Armhf,
    /// A macOS universal build: both Intel and Apple chips.
    Universal,
}

impl PackageCpu {
    fn parse(tag: &str) -> Option<PackageCpu> {
        match tag {
            "x64" | "x86_64" | "amd64" => Some(PackageCpu::X64),
            "arm64" | "aarch64" => Some(PackageCpu::Arm64),
            "armhf" => Some(PackageCpu::Armhf),
            "universal" => Some(PackageCpu::Universal),
            _ => None,
        }
    }

    /// Whether a build for this processor runs on a `host` processor:
    /// exactly its own, or a universal build on either Mac.
    fn serves(self, host: Cpu) -> bool {
        matches!(
            (self, host),
            (PackageCpu::X64, Cpu::X64)
                | (PackageCpu::Arm64, Cpu::Arm64)
                | (PackageCpu::Armhf, Cpu::Arm)
                | (PackageCpu::Universal, Cpu::X64 | Cpu::Arm64)
        )
    }
}

/// The extensions a build may have, longest first so `.tar.gz` wins over a
/// bare `.gz`. Compared case-insensitively.
const EXTENSIONS: [&str; 7] = [
    ".appimage",
    ".tar.gz",
    ".tar.xz",
    ".tgz",
    ".zip",
    ".7z",
    ".exe",
];

/// One build, as its file name describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    /// The file name exactly as the server lists it.
    pub file_name: String,
    /// The emulator, as written before the tags (`PCSX2`, `Pico-8`).
    pub emulator: String,
    /// The version without its `v` (`2.0.2`, `0.2.7a6`).
    pub version: String,
    pub os: PackageOs,
    pub cpu: PackageCpu,
    /// The extension, lower-cased, with its dot (`.appimage`, `.tar.gz`).
    pub extension: String,
}

/// Reads a build's file name. `None` unless it has a known extension, an
/// emulator name, exactly one version tag and exactly one arch tag naming
/// both a system and a processor (`arch-linux-x64`; a bare `arch-linux` is
/// not enough). Tags may come in any order; other bracketed tags (`(Qt)`)
/// are allowed and ignored.
pub fn parse_package_file(file_name: &str) -> Option<PackageFile> {
    let lower = file_name.to_lowercase();
    let extension = EXTENSIONS.iter().find(|ext| lower.ends_with(*ext))?;
    let stem = &file_name[..file_name.len() - extension.len()];

    let emulator = stem.split('(').next().unwrap_or("").trim();
    if emulator.is_empty() {
        return None;
    }

    let mut version: Option<String> = None;
    let mut target: Option<(PackageOs, PackageCpu)> = None;
    for tag in tags(stem) {
        let tag = tag.trim();
        let tag_lower = tag.to_lowercase();
        if let Some(arch) = tag_lower.strip_prefix("arch-") {
            let (system, cpu) = arch.split_once('-')?;
            let os = match system {
                "linux" => PackageOs::Linux,
                "windows" => PackageOs::Windows,
                "macos" => PackageOs::Macos,
                _ => return None,
            };
            let cpu = PackageCpu::parse(cpu)?;
            if cpu == PackageCpu::Universal && os != PackageOs::Macos {
                return None;
            }
            if target.replace((os, cpu)).is_some() {
                return None;
            }
        } else if is_version_tag(&tag_lower) && version.replace(tag[1..].to_string()).is_some() {
            return None;
        }
    }

    let (os, cpu) = target?;
    Some(PackageFile {
        file_name: file_name.to_string(),
        emulator: emulator.to_string(),
        version: version?,
        os,
        cpu,
        extension: (*extension).to_string(),
    })
}

/// The contents of every `( … )` in `stem`, in order.
fn tags(stem: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = stem;
    while let Some(open) = rest.find('(') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(')') else {
            break;
        };
        out.push(&after[..close]);
        rest = &after[close + 1..];
    }
    out
}

/// RomM's version-tag style: `v`, then dot-separated parts that each start
/// with a digit (`v2.0.2`, `v1.21`, `v0.2.7a6`, `v2412`).
fn is_version_tag(tag: &str) -> bool {
    let Some(body) = tag.strip_prefix('v') else {
        return false;
    };
    !body.is_empty()
        && body.split('.').all(|part| {
            part.chars().next().is_some_and(|c| c.is_ascii_digit())
                && part.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

/// Compares two versions part by part: each dot-separated part by its
/// leading number, then by what follows it. A missing part counts as 0, and a
/// trailing letter means a later build of the same number (`0.2.7a6` is
/// newer than `0.2.7`, as Pico-8 numbers its releases).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts_a: Vec<&str> = a.split('.').collect();
    let parts_b: Vec<&str> = b.split('.').collect();
    for i in 0..parts_a.len().max(parts_b.len()) {
        let (num_a, rest_a) = split_part(parts_a.get(i).copied().unwrap_or("0"));
        let (num_b, rest_b) = split_part(parts_b.get(i).copied().unwrap_or("0"));
        let ordering = num_a
            .cmp(&num_b)
            .then_with(|| rest_a.to_lowercase().cmp(&rest_b.to_lowercase()));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

/// A version part's leading number and the text after it (`7a6` → 7, `a6`).
fn split_part(part: &str) -> (u64, &str) {
    let digits = part.len() - part.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let number = part[..digits].parse().unwrap_or(0);
    (number, &part[digits..])
}

/// The newest build of each emulator that runs on an `os`/`cpu` computer,
/// in the order the emulators first appear. Emulator names compare
/// case-insensitively. Between a universal build and one for this processor
/// of the same version, the processor's own wins.
pub fn newest_for_host(files: &[PackageFile], os: PackageOs, cpu: Cpu) -> Vec<&PackageFile> {
    let mut newest: Vec<&PackageFile> = Vec::new();
    for file in files.iter().filter(|f| f.os == os && f.cpu.serves(cpu)) {
        match newest
            .iter_mut()
            .find(|seen| seen.emulator.eq_ignore_ascii_case(&file.emulator))
        {
            Some(seen) => {
                let ordering = compare_versions(&file.version, &seen.version);
                let more_specific =
                    seen.cpu == PackageCpu::Universal && file.cpu != PackageCpu::Universal;
                if ordering == Ordering::Greater || (ordering == Ordering::Equal && more_specific) {
                    *seen = file;
                }
            }
            None => newest.push(file),
        }
    }
    newest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(name: &str) -> PackageFile {
        parse_package_file(name).unwrap_or_else(|| panic!("{name:?} should parse"))
    }

    #[test]
    fn a_standard_name_parses() {
        let file = parse("PCSX2 (v2.0.2) (arch-linux-x64).AppImage");
        assert_eq!(file.emulator, "PCSX2");
        assert_eq!(file.version, "2.0.2");
        assert_eq!(file.os, PackageOs::Linux);
        assert_eq!(file.cpu, PackageCpu::X64);
        assert_eq!(file.extension, ".appimage");
    }

    #[test]
    fn tags_and_extensions_are_case_insensitive_and_extra_tags_are_ignored() {
        let file = parse("Xenia Canary (Qt) (ARCH-Windows-X64) (V1.0.2).ZIP");
        assert_eq!(file.emulator, "Xenia Canary");
        assert_eq!(file.version, "1.0.2");
        assert_eq!(file.os, PackageOs::Windows);
        assert_eq!(file.extension, ".zip");
        assert_eq!(
            parse("Dolphin (v2412) (arch-macos-universal).tar.gz").extension,
            ".tar.gz"
        );
        assert_eq!(
            parse("Pico-8 (arch-linux-armhf) (v0.2.7a6).zip").version,
            "0.2.7a6"
        );
    }

    #[test]
    fn a_name_missing_a_tag_or_with_two_is_not_a_package_build() {
        for name in [
            "PCSX2 (arch-linux-x64).AppImage",      // no version
            "PCSX2 (v2.0.2).AppImage",              // no arch
            "PCSX2 (v2.0.2) (arch-linux).AppImage", // no processor
            "PCSX2 (v2.0.2) (arch-linux-x64) (arch-windows-x64).zip", // two archs
            "PCSX2 (v2.0.2) (v2.0.3) (arch-linux-x64).zip", // two versions
            "PCSX2 (v2.0.2) (arch-amiga-x64).zip",  // unknown system
            "PCSX2 (v2.0.2) (arch-linux-mips).zip", // unknown processor
            "PCSX2 (v2.0.2) (arch-linux-universal).zip", // universal is macOS's
            "(v2.0.2) (arch-linux-x64).zip",        // no emulator
            "PCSX2 (v2.0.2) (arch-linux-x64).dmg",  // unknown extension
            "pico-8 (linux) (v0.2.7a6).zip",        // the old layout
        ] {
            assert_eq!(parse_package_file(name), None, "{name}");
        }
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert_eq!(compare_versions("2.0.10", "2.0.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.21", "1.3"), Ordering::Greater);
        assert_eq!(compare_versions("2.0", "2.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("2412", "2409"), Ordering::Greater);
        assert_eq!(compare_versions("0.2.7a6", "0.2.7"), Ordering::Greater);
        assert_eq!(compare_versions("0.2.7a6", "0.2.7a5"), Ordering::Greater);
        assert_eq!(compare_versions("0.2.8", "0.2.7a6"), Ordering::Greater);
    }

    #[test]
    fn processor_aliases_are_read() {
        assert_eq!(parse("A (v1) (arch-linux-amd64).zip").cpu, PackageCpu::X64);
        assert_eq!(parse("A (v1) (arch-linux-x86_64).zip").cpu, PackageCpu::X64);
        assert_eq!(
            parse("A (v1) (arch-linux-aarch64).zip").cpu,
            PackageCpu::Arm64
        );
    }

    fn newest(files: &[PackageFile], os: PackageOs, cpu: Cpu) -> Vec<(&str, &str)> {
        newest_for_host(files, os, cpu)
            .iter()
            .map(|f| (f.emulator.as_str(), f.version.as_str()))
            .collect()
    }

    #[test]
    fn the_newest_build_for_this_system_and_processor_wins_per_emulator() {
        let files: Vec<PackageFile> = [
            "PCSX2 (v1.6.0) (arch-linux-x64).AppImage",
            "PCSX2 (v2.0.2) (arch-linux-x64).AppImage",
            "PCSX2 (v2.1.0) (arch-windows-x64).zip",
            "pcsx2 (v2.0.1) (arch-linux-x64).AppImage",
            "PCSX2 (v2.2.0) (arch-linux-arm64).AppImage",
            "DuckStation (v0.1) (arch-linux-x64).AppImage",
        ]
        .iter()
        .map(|n| parse(n))
        .collect();
        assert_eq!(
            newest(&files, PackageOs::Linux, Cpu::X64),
            [("PCSX2", "2.0.2"), ("DuckStation", "0.1")]
        );
        assert_eq!(
            newest(&files, PackageOs::Linux, Cpu::Arm64),
            [("PCSX2", "2.2.0")]
        );
        assert!(newest(&files, PackageOs::Macos, Cpu::Arm64).is_empty());
    }

    #[test]
    fn a_raspberry_pi_build_is_not_offered_to_a_pc() {
        let files = vec![parse("Pico-8 (v0.2.7a6) (arch-linux-armhf).zip")];
        assert!(newest(&files, PackageOs::Linux, Cpu::X64).is_empty());
        assert_eq!(
            newest(&files, PackageOs::Linux, Cpu::Arm),
            [("Pico-8", "0.2.7a6")]
        );
    }

    #[test]
    fn a_universal_mac_build_serves_both_macs_and_a_native_one_wins_a_tie() {
        let files: Vec<PackageFile> = [
            "Dolphin (v2412) (arch-macos-universal).7z",
            "Dolphin (v2412) (arch-macos-arm64).7z",
            "Dolphin (v2409) (arch-macos-x64).7z",
        ]
        .iter()
        .map(|n| parse(n))
        .collect();
        let pick = |cpu| {
            newest_for_host(&files, PackageOs::Macos, cpu)
                .first()
                .map(|f| f.file_name.clone())
        };
        assert_eq!(
            pick(Cpu::Arm64).as_deref(),
            Some("Dolphin (v2412) (arch-macos-arm64).7z")
        );
        assert_eq!(
            pick(Cpu::X64).as_deref(),
            Some("Dolphin (v2412) (arch-macos-universal).7z")
        );
    }

    #[test]
    fn the_host_slug_maps_to_a_system() {
        assert_eq!(PackageOs::for_host("linux"), Some(PackageOs::Linux));
        assert_eq!(PackageOs::for_host("win32"), Some(PackageOs::Windows));
        assert_eq!(PackageOs::for_host("darwin"), Some(PackageOs::Macos));
    }
}
