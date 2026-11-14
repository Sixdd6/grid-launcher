//! GOG's Windows installers: an Inno Setup `setup_*.exe`, alone or with
//! `<stem>-1.bin`, `<stem>-2.bin`… beside it.
//!
//! Identity before running (spike 5): the `innospect` crate reads the
//! installer's header and file list. A `goggame-<id>.info` in that list
//! gives the product id; when its bytes are inside the `.exe` (single-file
//! installers up to [`WHOLE_READ_MAX`]) the file itself is read. An
//! installer innospect can't read falls back to its file name, and GRID
//! reads `goggame-*.info` from the install folder after the run.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use innospect::{Error as InnoError, InnoInstaller};

use crate::stores::gog::goggame::{parse_info, GogGameInfo};

/// How much of the `.exe` is read for its header.
pub const HEADER_READ: u64 = 64 * 1024 * 1024;

/// The largest `.exe` read whole to pull a goggame file out of it.
pub const WHOLE_READ_MAX: u64 = 256 * 1024 * 1024;

/// What GRID learned from an installer's own data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExeIdentity {
    /// The goggame file's contents, or an id-only identity (from its file
    /// name in the installer's list) when the bytes are out of reach.
    pub info: Option<GogGameInfo>,
    /// `AppVersion`, `""` when unknown.
    pub version: String,
    /// `AppName`, `""` when unknown.
    pub app_name: String,
    /// The Inno language GRID asks for: `english` when the installer has
    /// it, else its first; `""` when unknown.
    pub lang: String,
    /// How many `.bin` parts the installer's data spans (0: all inside the
    /// `.exe`, or unknown).
    pub bin_parts: u32,
}

/// What reading the `.exe` gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExeRead {
    /// An Inno Setup installer.
    Inno(Box<ExeIdentity>),
    /// Not a Windows program, or not an Inno Setup installer: not GOG's.
    NotGog,
    /// innospect could not read it (an unsupported version, a broken
    /// header): identity only after the run.
    Unknown,
}

/// Reads the installer at `path`.
pub fn read(path: &Path) -> ExeRead {
    let size = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let Some(head) = read_prefix(path, HEADER_READ) else {
        return ExeRead::Unknown;
    };
    let outcome = identify(&head, size <= HEADER_READ);
    match outcome {
        Identified::Done(read) => read,
        Identified::NeedsWhole(identity) => {
            if size > HEADER_READ && size <= WHOLE_READ_MAX {
                if let Some(whole) = read_prefix(path, WHOLE_READ_MAX) {
                    if let Identified::Done(read) = identify(&whole, true) {
                        return read;
                    }
                }
            }
            ExeRead::Inno(identity)
        }
    }
}

enum Identified {
    Done(ExeRead),
    /// A goggame file is listed but its bytes were not in `bytes`.
    NeedsWhole(Box<ExeIdentity>),
}

fn read_prefix(path: &Path, max: u64) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// Identifies an installer from (a prefix of) its bytes. `whole`: `bytes`
/// is the entire file.
fn identify(bytes: &[u8], whole: bool) -> Identified {
    let installer = match InnoInstaller::from_bytes(bytes) {
        Ok(installer) => installer,
        Err(InnoError::NotPe | InnoError::NotInnoSetup) => {
            return Identified::Done(ExeRead::NotGog)
        }
        Err(_) => return Identified::Done(ExeRead::Unknown),
    };
    let mut identity = ExeIdentity::default();
    if let Some(header) = installer.header() {
        identity.app_name = header.app_name().unwrap_or_default().trim().to_string();
        identity.version = header.app_version().unwrap_or_default().trim().to_string();
    }
    identity.lang = pick_lang(
        installer
            .languages()
            .iter()
            .map(|lang| String::from_utf8_lossy(&lang.name).into_owned()),
    );
    if installer.offset_table().offset_setup1 == 0 {
        let per_disk = installer
            .header()
            .map(|h| h.tail().slices_per_disk.max(1))
            .unwrap_or(1);
        let last = installer
            .file_locations()
            .iter()
            .map(|loc| loc.last_slice)
            .max()
            .unwrap_or(0);
        identity.bin_parts = last / per_disk + 1;
    }
    let goggame: Vec<_> = installer
        .files()
        .iter()
        .filter_map(|file| goggame_id(&file.destination).map(|id| (id, file)))
        .collect();
    if goggame.is_empty() {
        // An Inno installer without GOG's game info is not GOG's.
        return Identified::Done(ExeRead::NotGog);
    }
    let mut infos = Vec::new();
    let mut unreadable = false;
    for (_, file) in &goggame {
        match installer.extract_to_vec(file) {
            Ok(bytes) => {
                if let Some(info) = parse_info(&bytes) {
                    infos.push(info);
                }
            }
            Err(_) => unreadable = true,
        }
    }
    if let Some(info) = pick_base(infos) {
        identity.info = Some(info);
        return Identified::Done(ExeRead::Inno(Box::new(identity)));
    }
    // Only the file name: an id-only identity when there is one candidate.
    if goggame.len() == 1 {
        identity.info = Some(GogGameInfo {
            game_id: goggame[0].0.clone(),
            name: identity.app_name.clone(),
            ..Default::default()
        });
    }
    if unreadable && !whole {
        Identified::NeedsWhole(Box::new(identity))
    } else {
        Identified::Done(ExeRead::Inno(Box::new(identity)))
    }
}

/// The product id in an Inno destination like `{app}\goggame-1207658924.info`.
pub fn goggame_id(destination: &str) -> Option<String> {
    let file = destination.rsplit(['\\', '/']).next()?;
    let lower = file.to_ascii_lowercase();
    let id = lower.strip_prefix("goggame-")?.strip_suffix(".info")?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then(|| id.to_string())
}

/// The base game among several infos (a bundle with DLC), else the first.
pub fn pick_base(mut infos: Vec<GogGameInfo>) -> Option<GogGameInfo> {
    let base = infos.iter().position(|info| !info.is_dlc());
    match base {
        Some(index) => Some(infos.swap_remove(index)),
        None => infos.into_iter().next(),
    }
}

fn pick_lang(names: impl Iterator<Item = String>) -> String {
    let names: Vec<String> = names.map(|n| n.trim().to_string()).collect();
    names
        .iter()
        .find(|name| name.eq_ignore_ascii_case("english") || name.eq_ignore_ascii_case("en"))
        .or_else(|| names.first())
        .cloned()
        .unwrap_or_default()
}

/// The `.bin` parts beside `setup`: `<stem>-1.bin`, `<stem>-2.bin`… in
/// order, as long as they run on; `expected` (from the installer) asks for
/// that many. Returns `(found, first missing name)`.
pub fn bin_parts(setup: &Path, expected: u32) -> (Vec<PathBuf>, Option<String>) {
    let dir = setup.parent().unwrap_or(Path::new(""));
    let stem = setup
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let names = sibling_names(dir);
    let find = |n: u32| -> Option<PathBuf> {
        let wanted = format!("{stem}-{n}.bin").to_lowercase();
        names
            .iter()
            .find(|name| name.to_lowercase() == wanted)
            .map(|name| dir.join(name))
    };
    // The highest part number present, so a gap shows as a missing part.
    let highest = names
        .iter()
        .filter_map(|name| part_number(name, &stem))
        .max()
        .unwrap_or(0)
        .max(expected);
    let mut parts = Vec::new();
    for n in 1..=highest {
        match find(n) {
            Some(path) => parts.push(path),
            None => return (parts, Some(format!("{stem}-{n}.bin"))),
        }
    }
    (parts, None)
}

fn sibling_names(dir: &Path) -> Vec<String> {
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn part_number(name: &str, stem: &str) -> Option<u32> {
    let lower = name.to_lowercase();
    let rest = lower.strip_prefix(&format!("{}-", stem.to_lowercase()))?;
    rest.strip_suffix(".bin")?.parse().ok()
}

/// A title and version guessed from a GOG installer's file name:
/// `setup_quarry_nine_1.4_(64bit)_(69001).exe` → ("Quarry Nine", "1.4");
/// `paper_lanterns_1_1_0_60321.sh` → ("Paper Lanterns", "1.1.0").
pub fn guess_from_name(file_name: &str) -> (String, String) {
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let stem = stem
        .strip_prefix("setup_")
        .or_else(|| stem.strip_prefix("Setup_"))
        .unwrap_or(stem);
    let tokens: Vec<&str> = stem
        .split('_')
        .filter(|t| !t.is_empty() && !(t.starts_with('(') && t.ends_with(')')))
        .collect();
    let starts_digit = |t: &&str| t.chars().next().is_some_and(|c| c.is_ascii_digit());
    let Some(first) = tokens.iter().position(starts_digit) else {
        return (title_case(&tokens), String::new());
    };
    let title = title_case(&tokens[..first]);
    let version_tokens: Vec<&str> = tokens[first..]
        .iter()
        .copied()
        .take_while(|t| t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.'))
        .collect();
    let version = if version_tokens.iter().any(|t| t.contains('.')) {
        version_tokens[0].to_string()
    } else {
        // Underscored numbers; the last is the build number GOG appends.
        let digits: Vec<&str> = version_tokens
            .iter()
            .copied()
            .take_while(|t| t.bytes().all(|b| b.is_ascii_digit()))
            .collect();
        match digits.len() {
            0 => String::new(),
            1 => String::new(),
            n => digits[..n - 1].join("."),
        }
    };
    (title, version)
}

fn title_case(tokens: &[&str]) -> String {
    tokens
        .iter()
        .map(|token| {
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_give_a_title_and_version() {
        assert_eq!(
            guess_from_name("setup_quarry_nine_1.4_(64bit)_(69001).exe"),
            ("Quarry Nine".into(), "1.4".into())
        );
        assert_eq!(
            guess_from_name("paper_lanterns_1_1_0_60321.sh"),
            ("Paper Lanterns".into(), "1.1.0".into())
        );
        assert_eq!(
            guess_from_name("setup_tidewater_tales_dlc_(55120).exe"),
            ("Tidewater Tales Dlc".into(), String::new())
        );
        assert_eq!(
            guess_from_name("Moonrise_Setup.exe"),
            ("Moonrise Setup".into(), String::new())
        );
    }

    #[test]
    fn goggame_ids_from_destinations() {
        assert_eq!(
            goggame_id("{app}\\goggame-1207658924.info"),
            Some("1207658924".into())
        );
        assert_eq!(goggame_id("{app}/GOGGAME-5.INFO"), Some("5".into()));
        assert_eq!(goggame_id("{app}\\goggame-abc.info"), None);
        assert_eq!(goggame_id("{app}\\game.exe"), None);
    }

    #[test]
    fn a_non_windows_file_is_not_gogs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("setup.exe");
        fs::write(&path, b"#!/bin/sh\necho not a PE\n").unwrap();
        assert_eq!(read(&path), ExeRead::NotGog);
    }

    #[test]
    fn an_inno_signature_innospect_cannot_parse_is_unknown() {
        // The e2e suite's fake installers (gog-installers.mjs) have this
        // shape: identity only after the run.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("setup_foxglove_1.0_(55001).exe");
        let mut bytes = b"MZ".to_vec();
        bytes.resize(64, 0);
        bytes.extend_from_slice(&[
            b'r', b'D', b'l', b'P', b't', b'S', 0xCD, 0xE6, 0xD7, b'{', 0x0B, b'*',
        ]);
        bytes.resize(2048, 0);
        bytes.extend_from_slice(b"\nGRID-E2E-FAKE-INSTALLER\n{\"mode\":\"silent\"}");
        fs::write(&path, bytes).unwrap();
        assert_eq!(read(&path), ExeRead::Unknown);
    }

    #[test]
    fn bin_parts_run_on_and_report_the_first_gap() {
        let dir = tempfile::tempdir().unwrap();
        let setup = dir.path().join("setup_x_1.0_(1).exe");
        fs::write(&setup, b"MZ").unwrap();
        assert_eq!(bin_parts(&setup, 0), (vec![], None));
        fs::write(dir.path().join("setup_x_1.0_(1)-1.bin"), b"1").unwrap();
        fs::write(dir.path().join("setup_x_1.0_(1)-3.bin"), b"3").unwrap();
        let (found, missing) = bin_parts(&setup, 0);
        assert_eq!(found, vec![dir.path().join("setup_x_1.0_(1)-1.bin")]);
        assert_eq!(missing.as_deref(), Some("setup_x_1.0_(1)-2.bin"));
        fs::write(dir.path().join("setup_x_1.0_(1)-2.bin"), b"2").unwrap();
        assert_eq!(bin_parts(&setup, 3).0.len(), 3);
        assert_eq!(
            bin_parts(&setup, 4).1.as_deref(),
            Some("setup_x_1.0_(1)-4.bin"),
            "the installer asks for more parts than are there"
        );
    }
}
