//! What system and processor a program file was built for, from its header,
//! and whether it can run on this computer.
//!
//! An emulator build for the wrong processor (a Raspberry Pi build of Pico-8
//! on an x86-64 PC) installs without complaint and only fails at launch, with
//! the kernel's bare "Exec format error". Reading the header lets the install
//! refuse it, and the launch explain it, in words.
//!
//! Only the three native formats are read: ELF (Linux), PE (Windows) and
//! Mach-O (macOS, including universal "fat" files). Anything else — a shell
//! script, a file too short to tell — is [`BinaryTarget::Unknown`] and is
//! never refused.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// A processor family a program can be built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpu {
    X86,
    X64,
    Arm,
    Arm64,
}

impl Cpu {
    /// This computer's processor, as Rust's target names it; `None` for one
    /// GRID does not build for.
    pub fn host() -> Option<Cpu> {
        Cpu::from_target_arch(std::env::consts::ARCH)
    }

    fn from_target_arch(arch: &str) -> Option<Cpu> {
        match arch {
            "x86" => Some(Cpu::X86),
            "x86_64" => Some(Cpu::X64),
            "arm" => Some(Cpu::Arm),
            "aarch64" => Some(Cpu::Arm64),
            _ => None,
        }
    }

    /// How a user reads it: "64-bit x86", "32-bit ARM".
    pub fn describe(self) -> &'static str {
        match self {
            Cpu::X86 => "32-bit x86",
            Cpu::X64 => "64-bit x86",
            Cpu::Arm => "32-bit ARM",
            Cpu::Arm64 => "64-bit ARM",
        }
    }

    /// Whether a program built for `self` runs on a `host` processor: the
    /// same family, or its 32-bit form on a 64-bit host (WOW64, multiarch).
    fn runs_on(self, host: Cpu) -> bool {
        self == host || matches!((self, host), (Cpu::X86, Cpu::X64) | (Cpu::Arm, Cpu::Arm64))
    }
}

/// A program file's system, from its format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Linux,
    Windows,
    Macos,
}

impl System {
    /// This computer's system; `None` elsewhere (the BSDs).
    pub fn host() -> Option<System> {
        if cfg!(target_os = "linux") {
            Some(System::Linux)
        } else if cfg!(windows) {
            Some(System::Windows)
        } else if cfg!(target_os = "macos") {
            Some(System::Macos)
        } else {
            None
        }
    }

    fn describe(self) -> &'static str {
        match self {
            System::Linux => "Linux",
            System::Windows => "Windows",
            System::Macos => "macOS",
        }
    }
}

/// What a program file's header says it was built for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryTarget {
    /// A native program. A Mach-O universal file lists every processor it
    /// carries; the others list one. An unrecognised processor is left out,
    /// so `cpus` may be empty.
    Native { system: System, cpus: Vec<Cpu> },
    /// A script, or anything else whose header is not a native program's.
    Unknown,
}

impl BinaryTarget {
    /// Whether this program can run on a `system`/`cpu` computer. Unknown
    /// files, and native ones whose processor could not be read, are given
    /// the benefit of the doubt.
    pub fn runs_on(&self, system: System, cpu: Cpu) -> bool {
        match self {
            BinaryTarget::Unknown => true,
            BinaryTarget::Native {
                system: built,
                cpus,
            } => *built == system && (cpus.is_empty() || cpus.iter().any(|c| c.runs_on(cpu))),
        }
    }

    /// "32-bit ARM Linux", "64-bit x86 and 64-bit ARM macOS", "Windows".
    pub fn describe(&self) -> String {
        match self {
            BinaryTarget::Unknown => "an unknown system".to_string(),
            BinaryTarget::Native { system, cpus } if cpus.is_empty() => {
                system.describe().to_string()
            }
            BinaryTarget::Native { system, cpus } => {
                let cpus: Vec<&str> = cpus.iter().map(|c| c.describe()).collect();
                format!("{} {}", cpus.join(" and "), system.describe())
            }
        }
    }
}

/// This computer, described the way [`BinaryTarget::describe`] describes a
/// program: "64-bit x86 Linux".
pub fn describe_host() -> String {
    match (System::host(), Cpu::host()) {
        (Some(system), Some(cpu)) => format!("{} {}", cpu.describe(), system.describe()),
        (Some(system), None) => system.describe().to_string(),
        _ => std::env::consts::OS.to_string(),
    }
}

/// Reads `path`'s header. An unreadable file is [`BinaryTarget::Unknown`]:
/// whatever opens it next reports the real error.
pub fn read_binary_target(path: &Path) -> BinaryTarget {
    let mut header = [0u8; 4096];
    let read = File::open(path).and_then(|mut file| read_up_to(&mut file, &mut header));
    match read {
        Ok(len) => parse_header(&header[..len]),
        Err(_) => BinaryTarget::Unknown,
    }
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

/// When `path` is a native program that cannot run on this computer, what it
/// was built for. `None` when it can, or when that cannot be told.
pub fn host_mismatch(path: &Path) -> Option<BinaryTarget> {
    let (system, cpu) = (System::host()?, Cpu::host()?);
    let target = read_binary_target(path);
    (!target.runs_on(system, cpu)).then_some(target)
}

/// Why `file` cannot run here, for an install or launch error: "pico8 is
/// built for 32-bit ARM Linux, and this computer is 64-bit x86 Linux."
pub fn mismatch_message(file: &Path, target: &BinaryTarget) -> String {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    format!(
        "{name} is built for {}, and this computer is {}.",
        target.describe(),
        describe_host()
    )
}

/// Reads a program header from its first bytes.
pub fn parse_header(bytes: &[u8]) -> BinaryTarget {
    if bytes.starts_with(b"\x7fELF") {
        return parse_elf(bytes);
    }
    if bytes.starts_with(b"MZ") {
        return parse_pe(bytes);
    }
    parse_mach_o(bytes).unwrap_or(BinaryTarget::Unknown)
}

fn u16_at(bytes: &[u8], offset: usize, little: bool) -> Option<u16> {
    let raw: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
    Some(if little {
        u16::from_le_bytes(raw)
    } else {
        u16::from_be_bytes(raw)
    })
}

fn u32_at(bytes: &[u8], offset: usize, little: bool) -> Option<u32> {
    let raw: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(if little {
        u32::from_le_bytes(raw)
    } else {
        u32::from_be_bytes(raw)
    })
}

/// ELF: `e_machine` at offset 18, in the byte order `EI_DATA` (offset 5)
/// gives.
fn parse_elf(bytes: &[u8]) -> BinaryTarget {
    let little = bytes.get(5) != Some(&2);
    let cpu = u16_at(bytes, 18, little).and_then(|machine| match machine {
        3 => Some(Cpu::X86),
        62 => Some(Cpu::X64),
        40 => Some(Cpu::Arm),
        183 => Some(Cpu::Arm64),
        _ => None,
    });
    native(System::Linux, cpu.into_iter().collect())
}

/// PE: the `PE\0\0` signature at `e_lfanew` (offset 0x3C), then `Machine`.
/// An `MZ` file without that signature is a DOS program: Windows still.
fn parse_pe(bytes: &[u8]) -> BinaryTarget {
    let cpu = u32_at(bytes, 0x3C, true)
        .map(|offset| offset as usize)
        .filter(|&offset| bytes.get(offset..offset + 4) == Some(b"PE\0\0"))
        .and_then(|offset| u16_at(bytes, offset + 4, true))
        .and_then(|machine| match machine {
            0x014c => Some(Cpu::X86),
            0x8664 => Some(Cpu::X64),
            0x01c0 | 0x01c4 => Some(Cpu::Arm),
            0xaa64 => Some(Cpu::Arm64),
            _ => None,
        });
    native(System::Windows, cpu.into_iter().collect())
}

const MACH_CPU_X86: u32 = 7;
const MACH_CPU_ARM: u32 = 12;
const MACH_ABI64: u32 = 0x0100_0000;

fn mach_cpu(cputype: u32) -> Option<Cpu> {
    match cputype {
        MACH_CPU_X86 => Some(Cpu::X86),
        t if t == MACH_CPU_X86 | MACH_ABI64 => Some(Cpu::X64),
        MACH_CPU_ARM => Some(Cpu::Arm),
        t if t == MACH_CPU_ARM | MACH_ABI64 => Some(Cpu::Arm64),
        _ => None,
    }
}

/// Mach-O, thin (`feedface`/`feedfacf`, either byte order) or universal
/// (`cafebabe`, big-endian, one 20-byte record per processor). `cafebabe` is
/// also a Java class file's magic; a real universal file lists a handful of
/// processors, a class file's version makes that count huge.
fn parse_mach_o(bytes: &[u8]) -> Option<BinaryTarget> {
    let magic = u32_at(bytes, 0, false)?;
    match magic {
        0xfeed_face | 0xfeed_facf => {
            let cpu = u32_at(bytes, 4, false).and_then(mach_cpu);
            Some(native(System::Macos, cpu.into_iter().collect()))
        }
        0xcefa_edfe | 0xcffa_edfe => {
            let cpu = u32_at(bytes, 4, true).and_then(mach_cpu);
            Some(native(System::Macos, cpu.into_iter().collect()))
        }
        0xcafe_babe => {
            let count = u32_at(bytes, 4, false)?;
            if count == 0 || count > 16 {
                return None;
            }
            let cpus = (0..count as usize)
                .filter_map(|i| u32_at(bytes, 8 + 20 * i, false).and_then(mach_cpu))
                .collect();
            Some(native(System::Macos, cpus))
        }
        _ => None,
    }
}

fn native(system: System, cpus: Vec<Cpu>) -> BinaryTarget {
    BinaryTarget::Native { system, cpus }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elf(machine: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2; // 64-bit class
        bytes[5] = 1; // little-endian
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    fn pe(machine: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x100];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    fn linux(cpu: Cpu) -> BinaryTarget {
        native(System::Linux, vec![cpu])
    }

    #[test]
    fn elf_headers_name_their_processor() {
        assert_eq!(parse_header(&elf(62)), linux(Cpu::X64));
        assert_eq!(parse_header(&elf(40)), linux(Cpu::Arm));
        assert_eq!(parse_header(&elf(183)), linux(Cpu::Arm64));
        assert_eq!(parse_header(&elf(3)), linux(Cpu::X86));
        assert_eq!(parse_header(&elf(243)), native(System::Linux, vec![]));
    }

    #[test]
    fn pe_headers_name_their_processor() {
        assert_eq!(
            parse_header(&pe(0x8664)),
            native(System::Windows, vec![Cpu::X64])
        );
        assert_eq!(
            parse_header(&pe(0xaa64)),
            native(System::Windows, vec![Cpu::Arm64])
        );
        // A bare DOS stub is still a Windows file.
        assert_eq!(parse_header(b"MZ\x90\0"), native(System::Windows, vec![]));
    }

    #[test]
    fn mach_o_thin_and_universal() {
        let mut thin = vec![0xcf, 0xfa, 0xed, 0xfe];
        thin.extend_from_slice(&(MACH_CPU_ARM | MACH_ABI64).to_le_bytes());
        assert_eq!(parse_header(&thin), native(System::Macos, vec![Cpu::Arm64]));

        let mut fat = 0xcafe_babeu32.to_be_bytes().to_vec();
        fat.extend_from_slice(&2u32.to_be_bytes());
        for cputype in [MACH_CPU_X86 | MACH_ABI64, MACH_CPU_ARM | MACH_ABI64] {
            fat.extend_from_slice(&cputype.to_be_bytes());
            fat.extend_from_slice(&[0u8; 16]);
        }
        let universal = parse_header(&fat);
        assert_eq!(universal, native(System::Macos, vec![Cpu::X64, Cpu::Arm64]));
        assert_eq!(universal.describe(), "64-bit x86 and 64-bit ARM macOS");

        // A Java class file shares the magic; its "count" is its version.
        let mut class = 0xcafe_babeu32.to_be_bytes().to_vec();
        class.extend_from_slice(&61u32.to_be_bytes());
        assert_eq!(parse_header(&class), BinaryTarget::Unknown);
    }

    #[test]
    fn scripts_and_short_files_are_unknown() {
        assert_eq!(parse_header(b"#!/bin/sh\nexit 0\n"), BinaryTarget::Unknown);
        assert_eq!(parse_header(b""), BinaryTarget::Unknown);
    }

    #[test]
    fn what_runs_where() {
        let arm32 = linux(Cpu::Arm);
        assert!(!arm32.runs_on(System::Linux, Cpu::X64));
        assert!(arm32.runs_on(System::Linux, Cpu::Arm64));
        assert!(linux(Cpu::X86).runs_on(System::Linux, Cpu::X64));
        assert!(!linux(Cpu::X64).runs_on(System::Linux, Cpu::Arm64));
        assert!(!linux(Cpu::X64).runs_on(System::Windows, Cpu::X64));
        assert!(native(System::Linux, vec![]).runs_on(System::Linux, Cpu::Arm64));
        assert!(BinaryTarget::Unknown.runs_on(System::Macos, Cpu::Arm64));
        assert_eq!(arm32.describe(), "32-bit ARM Linux");
    }

    #[test]
    fn a_file_on_disk_is_read_by_its_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pico8");
        std::fs::write(&path, elf(40)).unwrap();
        assert_eq!(read_binary_target(&path), linux(Cpu::Arm));
        assert_eq!(
            read_binary_target(&dir.path().join("missing")),
            BinaryTarget::Unknown
        );
    }

    #[test]
    fn rust_target_names_map_to_processors() {
        assert_eq!(Cpu::from_target_arch("x86_64"), Some(Cpu::X64));
        assert_eq!(Cpu::from_target_arch("aarch64"), Some(Cpu::Arm64));
        assert_eq!(Cpu::from_target_arch("riscv64"), None);
    }
}
