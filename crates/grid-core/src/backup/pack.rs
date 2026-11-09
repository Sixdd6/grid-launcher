//! The gridpack format: `gridpack/gridpack.json` at a drive's root, the index
//! of every game, emulator and firmware set the drive holds. Every path in it
//! is relative to `gridpack/`. This file holds the schema, reading and
//! atomic writing, path validation, leftover cleanup, the layout helpers,
//! the index edits and the per-drive edit lock.
//!
//! A drive's `gridpack.json` is untrusted input: unknown `os` values and the
//! old prototype shape (games keyed by "0", "1"…) fail to parse, every
//! relative path is validated before use, and the file is size-capped.

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde::{Deserialize, Serialize};

use crate::library::paths::sanitize_component;
use crate::romm::{RomDetail, RomFile};

/// The folder GRID owns at a drive's root. GRID touches nothing outside it.
pub const PACK_DIR: &str = "gridpack";
/// The index file inside [`PACK_DIR`].
pub const PACK_FILE: &str = "gridpack.json";
/// The format this build writes. A pack with a greater format is read-only.
pub const FORMAT: u32 = 1;
/// Every in-flight file is `<name>.gridtmp` until it is complete.
pub const TMP_SUFFIX: &str = ".gridtmp";
/// The largest file FAT32 can hold.
pub const FAT32_MAX_FILE: u64 = 4_294_967_295;
/// The largest `gridpack.json` this build reads.
pub const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
/// The longest path component (in bytes) a pack may hold. FAT, exFAT and
/// NTFS allow 255 UTF-16 units; 255 UTF-8 bytes never exceeds that.
pub const MAX_COMPONENT_BYTES: usize = 255;
/// The largest part of an installed copy's archive: under
/// [`FAT32_MAX_FILE`], so any drive can hold one.
pub const PART_BYTES: u64 = 4095 * 1024 * 1024;

/// An operating system a pack carries emulators for.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PackOs {
    Linux,
    Windows,
}

impl PackOs {
    /// The OS this build runs on, if a pack can target it.
    pub fn host() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(windows) {
            Some(Self::Windows)
        } else {
            None
        }
    }

    /// The forge host name: `linux` or `win32`.
    pub fn forge_host(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "win32",
        }
    }

    /// The emulator folder tag: `linux-x86_64` or `windows-x86_64`.
    pub fn arch_tag(self) -> &'static str {
        match self {
            Self::Linux => "linux-x86_64",
            Self::Windows => "windows-x86_64",
        }
    }
}

/// What reading a drive's `gridpack.json` found.
#[derive(Clone, PartialEq, Debug)]
pub enum PackState {
    /// No `gridpack/gridpack.json`: the drive is not set up.
    Absent,
    Ready(Pack),
    /// Written by a newer GRID. Shown, never rewritten.
    NewerFormat(u32),
    /// Present but not a pack this build can read.
    Unreadable(String),
}

/// The whole `gridpack.json`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Pack {
    pub format: u32,
    pub name: String,
    pub os: Vec<PackOs>,
    pub created: String,
    pub updated: String,
    #[serde(default)]
    pub games: Vec<PackGame>,
    #[serde(default)]
    pub emulators: Vec<PackEmulator>,
    #[serde(default)]
    pub firmware: Vec<PackFirmware>,
    /// Keys this build does not know, kept so a rewrite does not drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Where a game came from: enough to rebuild its detail and image cache keys.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackSource {
    /// The server URL, always without userinfo.
    pub server: String,
    pub rom_id: i64,
    #[serde(default)]
    pub igdb_id: Option<i64>,
    #[serde(default)]
    pub fs_name: String,
    #[serde(default)]
    pub server_updated_at: String,
    #[serde(default)]
    pub cover_small_path: String,
    #[serde(default)]
    pub cover_large_path: String,
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
    #[serde(default)]
    pub fanart_urls: Vec<String>,
    #[serde(default)]
    pub logo_url: String,
}

/// One backed-up game.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackGame {
    /// The RomM platform slug.
    pub platform: String,
    pub platform_name: String,
    pub source: PackSource,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub languages: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub rating: String,
    #[serde(default)]
    pub companies: Vec<String>,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub game_modes: Vec<String>,
    #[serde(default)]
    pub franchises: Vec<String>,
    #[serde(default)]
    pub players: String,
    #[serde(default)]
    pub version: String,
    /// The bytes this entry holds on the drive, art aside: the sum of
    /// [`Self::files`], or for an installed copy the sum of its parts
    /// ([`PackInstalled::parts_bytes`]). [`Pack::pack_bytes`] adds these up.
    pub size: u64,
    #[serde(default)]
    pub sha1: String,
    /// The server's files on the drive. Empty for a game backed up as an
    /// archived install ([`Self::installed`]).
    #[serde(default)]
    pub files: Vec<PackFile>,
    #[serde(default)]
    pub assets: PackAssets,
    pub added: String,
    /// Set when the game is a copy of the install on the computer that
    /// backed it up, written as an archive in parts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<PackInstalled>,
    /// Keys this build does not know, kept so a rewrite does not drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A copy of a game as installed on the computer that backed it up.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackInstalled {
    pub archive: ArchiveKind,
    pub layout: InstalledLayout,
    /// The folder (or file) name in the platform folder.
    pub name: String,
    /// The archive's parts, in order.
    pub parts: Vec<PackPart>,
    /// The space the restored tree needs.
    pub tree_bytes: u64,
    pub tree_files: u64,
    /// The launch file, relative to the tree (`""` for [`InstalledLayout::File`]).
    #[serde(default)]
    pub launch: String,
    /// The install row's ROM file name, for update detection.
    #[serde(default)]
    pub rom_file_name: String,
    /// The install row's server timestamp (the installed copy's).
    #[serde(default)]
    pub server_updated_at: String,
    /// `"v1.2"`, or `""` when the file name has no version tag.
    #[serde(default)]
    pub version: String,
    /// The server had newer files at backup time.
    #[serde(default)]
    pub outdated: bool,
    /// The OS the copy was made on. An OS this build does not know (a
    /// newer GRID's) reads as `None` rather than making the pack unreadable.
    #[serde(default, deserialize_with = "lenient_os")]
    pub from_os: Option<PackOs>,
    #[serde(default)]
    pub row: PackInstalledRow,
}

impl PackInstalled {
    /// The bytes the parts take on the drive: what [`PackGame::size`] holds
    /// for an installed copy.
    pub fn parts_bytes(&self) -> u64 {
        self.parts
            .iter()
            .fold(0_u64, |sum, part| sum.saturating_add(part.size))
    }

    /// Whether this build can restore the copy: a known archive kind and
    /// layout. A copy it cannot restore is still listed; Install says it
    /// needs a newer GRID.
    pub fn is_supported(&self) -> bool {
        self.archive != ArchiveKind::Unknown && self.layout != InstalledLayout::Unknown
    }
}

/// `from_os` read leniently: any value that is not a known [`PackOs`]
/// (another string, a number, `null`) is `None`.
fn lenient_os<'de, D>(deserializer: D) -> Result<Option<PackOs>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

/// How an installed copy is archived.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveKind {
    /// Uncompressed tar, split into parts of at most [`PART_BYTES`].
    Tar,
    /// Written by a newer GRID: listed, never installed.
    #[serde(other)]
    Unknown,
}

/// What an installed copy's tree is, which picks how it is restored.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum InstalledLayout {
    Extracted,
    Native,
    Multi,
    File,
    /// Written by a newer GRID: listed, never installed.
    #[serde(other)]
    Unknown,
}

/// One part of an installed copy's archive.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackPart {
    pub path: String,
    pub size: u64,
    /// The MD5 of the bytes GRID wrote.
    pub md5: String,
}

/// The install row values a restore needs. Relative or plain values only,
/// never an absolute path.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct PackInstalledRow {
    /// Relative to the tree.
    #[serde(default)]
    pub native_executable: String,
    #[serde(default)]
    pub native_launch_parameters: String,
    #[serde(default)]
    pub native_launch_source: String,
    #[serde(default)]
    pub native_working_dir: String,
    #[serde(default)]
    pub ps4_game_id: String,
    #[serde(default)]
    pub ps4_content: String,
    #[serde(default)]
    pub included_dlc: String,
    #[serde(default)]
    pub ra_id: String,
}

/// One file of a game, as the server lists it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackFile {
    pub path: String,
    pub size: u64,
    pub server_file_id: i64,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default = "yes")]
    pub top_level: bool,
    /// RomM's MD5 for the file, kept for identification only. RomM hashes
    /// an archive's decompressed contents (for No-Intro matching), not its
    /// bytes, so this is never used to check the file on the drive: the
    /// write is checked against the bytes GRID downloaded
    /// ([`super::write::write_file`]).
    #[serde(default)]
    pub md5: String,
    /// RomM's SHA-1 for the file; informational, like [`Self::md5`].
    #[serde(default)]
    pub sha1: String,
    /// Keys this build does not know, kept so a rewrite does not drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn yes() -> bool {
    true
}

/// A game's art on the drive.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct PackAssets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_small: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
}

/// One emulator release for one OS.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackEmulator {
    /// The emulator profile name.
    pub name: String,
    /// `emulator` or `compat_tool`.
    pub kind: String,
    /// A catalog `owner/repo`, `romm:<folder>/<emu>`, or `local:<name>` for an
    /// archived manual emulator.
    pub source_id: String,
    pub version: String,
    /// `local:` only: the program's path inside the archive.
    #[serde(default)]
    pub program: String,
    /// `local:` only: the launch options.
    #[serde(default)]
    pub args: String,
    /// [`PackOs::arch_tag`].
    pub os: String,
    pub platforms: Vec<String>,
    pub path: String,
    #[serde(default)]
    pub supplementals: Vec<String>,
    pub asset_name: String,
    pub size: u64,
    /// Keys this build does not know, kept so a rewrite does not drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// The firmware of one platform.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackFirmware {
    pub platform: String,
    pub platform_name: String,
    pub files: Vec<PackFirmwareFile>,
    /// Keys this build does not know, kept so a rewrite does not drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One firmware file, as the server serves it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PackFirmwareFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    /// The server's MD5 of the raw firmware file. RomM serves firmware
    /// as is, so this is the file's own hash, and an install from the drive
    /// checks it (`firmware::verify_download`).
    #[serde(default)]
    pub md5: String,
}

/// Why a pack operation failed.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// A relative path that is not a plain path under `gridpack/`.
    #[error("unsafe path in gridpack: {0}")]
    InvalidPath(String),
    /// No `gridpack/gridpack.json`.
    #[error("the drive is not set up for GRID")]
    NotSetUp,
    /// Written by a newer GRID: read-only.
    #[error("the drive was set up by a newer GRID (format {0}) and is read-only")]
    NewerFormat(u32),
    /// Present but not a pack this build can read.
    #[error("the drive's gridpack can't be read: {0}")]
    Unreadable(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// The current time as an RFC 3339 UTC string, the form every pack
/// timestamp uses.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// `<mount>/gridpack`.
pub fn pack_dir(mount: &Path) -> PathBuf {
    mount.join(PACK_DIR)
}

/// `<mount>/gridpack/gridpack.json`.
pub fn index_path(mount: &Path) -> PathBuf {
    pack_dir(mount).join(PACK_FILE)
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// A shortened copy of untrusted text for an error message.
fn excerpt(text: &str) -> String {
    const MAX: usize = 120;
    match text.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

/// Reads a drive's `gridpack.json`. A missing file is [`PackState::Absent`];
/// a file over [`MAX_INDEX_BYTES`], one that does not parse, or one holding
/// an unsafe path is [`PackState::Unreadable`]. Other I/O errors are errors.
pub fn read_pack(mount: &Path) -> Result<PackState, PackError> {
    if is_link(&pack_dir(mount)) {
        return Ok(PackState::Unreadable(format!("{PACK_DIR} is a link")));
    }
    let file = match std::fs::File::open(index_path(mount)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(PackState::Absent),
        Err(error) => return Err(error.into()),
    };
    let too_big = || {
        PackState::Unreadable(format!(
            "{PACK_FILE} is larger than {} MiB",
            MAX_INDEX_BYTES >> 20
        ))
    };
    if file.metadata()?.len() > MAX_INDEX_BYTES {
        return Ok(too_big());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INDEX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Ok(too_big());
    }
    Ok(parse_pack(&bytes))
}

/// Parses the bytes of a `gridpack.json`. Pure: [`read_pack`] without the
/// file system.
pub fn parse_pack(bytes: &[u8]) -> PackState {
    let value: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => return PackState::Unreadable(error.to_string()),
    };
    let Some(format) = value.get("format").and_then(serde_json::Value::as_u64) else {
        return PackState::Unreadable("no format number".to_string());
    };
    if format > u64::from(FORMAT) {
        return PackState::NewerFormat(u32::try_from(format).unwrap_or(u32::MAX));
    }
    if format != u64::from(FORMAT) {
        return PackState::Unreadable(format!("unknown format {format}"));
    }
    let pack: Pack = match serde_json::from_value(value) {
        Ok(pack) => pack,
        Err(error) => return PackState::Unreadable(error.to_string()),
    };
    if let Some(bad) = pack.paths().find(|rel| validate_rel(rel).is_err()) {
        return PackState::Unreadable(format!("unsafe path {}", excerpt(bad)));
    }
    PackState::Ready(pack)
}

/// Checks a pack-relative path: `/`-separated, relative, only plain
/// components (no `.`, `..`, empty, drive prefix, `:` or backslash), each at
/// most [`MAX_COMPONENT_BYTES`] bytes. Returns it as a native relative path.
pub fn validate_rel(rel: &str) -> Result<PathBuf, PackError> {
    let bad = || PackError::InvalidPath(excerpt(rel));
    if rel.is_empty() || rel.starts_with('/') || rel.contains(['\\', ':', '\0']) {
        return Err(bad());
    }
    let mut out = PathBuf::new();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.len() > MAX_COMPONENT_BYTES {
            return Err(bad());
        }
        out.push(part);
    }
    if !out
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(bad());
    }
    Ok(out)
}

/// The absolute path of a pack-relative path on a drive. Validates `rel`,
/// refuses a `gridpack` folder that is a link, and refuses a path whose
/// deepest existing part resolves (through links) outside the pack folder.
pub fn resolve(mount: &Path, rel: &str) -> Result<PathBuf, PackError> {
    let rel_path = validate_rel(rel)?;
    let base = pack_dir(mount);
    if is_link(&base) {
        return Err(PackError::InvalidPath(format!("{PACK_DIR} is a link")));
    }
    let target = base.join(&rel_path);
    if let Ok(canonical_base) = crate::platform::canonicalize(&base) {
        let existing = target
            .ancestors()
            .take_while(|path| path.starts_with(&base))
            .find(|path| std::fs::symlink_metadata(path).is_ok());
        if let Some(existing) = existing {
            let inside = crate::platform::canonicalize(existing)
                .is_ok_and(|canonical| canonical.starts_with(&canonical_base));
            if !inside {
                return Err(PackError::InvalidPath(excerpt(rel)));
            }
        }
    }
    Ok(target)
}

/// Writes `gridpack.json` atomically: the JSON goes to
/// `gridpack.json.gridtmp`, is synced, then renamed over the index, and on
/// unix the folder is synced too. Creates `gridpack/` when it is missing.
pub fn write_pack_atomic(mount: &Path, pack: &Pack) -> Result<(), PackError> {
    if pack.format != FORMAT {
        return Err(PackError::NewerFormat(pack.format));
    }
    let dir = pack_dir(mount);
    if is_link(&dir) {
        return Err(PackError::InvalidPath(format!("{PACK_DIR} is a link")));
    }
    std::fs::create_dir_all(&dir)?;
    let mut json = serde_json::to_vec_pretty(pack).map_err(io::Error::other)?;
    json.push(b'\n');
    let tmp = dir.join(format!("{PACK_FILE}{TMP_SUFFIX}"));
    if let Err(error) = write_synced_then_rename(&tmp, &json, &dir.join(PACK_FILE)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }
    #[cfg(unix)]
    {
        // Some file systems refuse to sync a folder; the rename is done.
        if let Ok(folder) = std::fs::File::open(&dir) {
            let _ = folder.sync_all();
        }
    }
    Ok(())
}

/// Writes `bytes` to `tmp`, syncs it, then renames it over `dest`.
fn write_synced_then_rename(tmp: &Path, bytes: &[u8], dest: &Path) -> io::Result<()> {
    let mut file = std::fs::File::create(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(tmp, dest)
}

/// Removes every `*.gridtmp` file or folder under `<mount>/gridpack/`, and
/// nothing else. Links are never followed. Returns how many it removed.
pub fn cleanup_leftovers(mount: &Path) -> io::Result<u64> {
    let dir = pack_dir(mount);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() => remove_tmp_under(&dir),
        Ok(_) => Ok(0),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

fn remove_tmp_under(dir: &Path) -> io::Result<u64> {
    let mut removed = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        let is_tmp = entry
            .file_name()
            .as_encoded_bytes()
            .ends_with(TMP_SUFFIX.as_bytes());
        if is_tmp {
            if kind.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else if kind.is_symlink() {
                // A folder link on Windows needs remove_dir; neither follows it.
                std::fs::remove_file(&path).or_else(|_| std::fs::remove_dir(&path))?;
            } else {
                std::fs::remove_file(&path)?;
            }
            removed += 1;
        } else if kind.is_dir() {
            removed += remove_tmp_under(&path)?;
        }
    }
    Ok(removed)
}

/// A new, empty pack. A pack has no id: a drive is identified by its mount
/// path (Linux, macOS) or drive letter (Windows), which the OS keeps unique.
pub fn new_pack(name: &str, os: &[PackOs]) -> Pack {
    let now = now_rfc3339();
    let mut targets = Vec::new();
    for &one in os {
        if !targets.contains(&one) {
            targets.push(one);
        }
    }
    Pack {
        format: FORMAT,
        name: name.to_string(),
        os: targets,
        created: now.clone(),
        updated: now,
        games: Vec::new(),
        emulators: Vec::new(),
        firmware: Vec::new(),
        extra: BTreeMap::new(),
    }
}

/// Windows reserved device names, which no FAT, exFAT or NTFS file may use
/// as its stem.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether `stem` (a name up to its first `.`) is a Windows reserved
/// device name, ignoring case and trailing spaces.
fn is_reserved_stem(stem: &str) -> bool {
    RESERVED_NAMES
        .iter()
        .any(|reserved| stem.trim_end().eq_ignore_ascii_case(reserved))
}

/// Whether a file name is a Windows reserved device name (`CON`,
/// `aux.txt`, `Com1.tar.gz`…), which no FAT, exFAT or NTFS file may use.
pub(crate) fn is_reserved_name(name: &str) -> bool {
    is_reserved_stem(name.split('.').next().unwrap_or(name))
}

/// One path component as a pack writes it: [`sanitize_component`], then a
/// Windows reserved device name (`CON`, `aux.txt`…) gets `_` after its stem,
/// then the name is cut to [`MAX_COMPONENT_BYTES`], keeping a short extension.
pub fn pack_component(raw: &str, fallback: &str) -> String {
    let name = sanitize_component(raw, fallback);
    let (stem, rest) = match name.find('.') {
        Some(dot) => name.split_at(dot),
        None => (name.as_str(), ""),
    };
    let reserved = is_reserved_stem(stem);
    let name = if reserved {
        format!("{}_{rest}", stem.trim_end())
    } else {
        name
    };
    truncate_component(&name)
}

/// Cuts a component to [`MAX_COMPONENT_BYTES`] bytes on a char boundary,
/// keeping an extension of up to 16 bytes.
fn truncate_component(name: &str) -> String {
    if name.len() <= MAX_COMPONENT_BYTES {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 && name.len() - dot <= 16 => name.split_at(dot),
        _ => (name, ""),
    };
    let mut cut = MAX_COMPONENT_BYTES - ext.len();
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut stem = stem[..cut].to_string();
    // A cut may leave a trailing space or dot, which Windows drops.
    if stem.ends_with([' ', '.']) {
        stem.pop();
        stem.push('_');
    }
    format!("{stem}{ext}")
}

/// The path a game owns in the pack, relative to `gridpack/`:
/// - a single-file game (`single_file_name` is `Some`) owns the file
///   `games/<slug>/<file>`;
/// - a multi-file or native game owns the folder `games/<slug>/<fs_name>`,
///   and its files go inside it ([`game_file_rel`]).
///
/// Pass the result through [`unique_rel`] before writing.
pub fn game_dir_rel(platform_slug: &str, fs_name: &str, single_file_name: Option<&str>) -> String {
    let slug = pack_component(platform_slug, "platform");
    let leaf = match single_file_name {
        Some(file) => pack_component(file, "game"),
        None => pack_component(fs_name, "game"),
    };
    format!("games/{slug}/{leaf}")
}

/// Part `index` (zero-based: the position in [`PackInstalled::parts`]) of
/// an installed copy's archive: `<game folder>/installed.tar.001` for index
/// 0, `.002` for index 1, and so on. The game folder comes from
/// [`game_dir_rel`] with no single file, through [`unique_rel`] with
/// `is_file = false`.
pub fn installed_part_rel(game_dir: &str, index: usize) -> String {
    format!("{game_dir}/installed.tar.{:03}", index.saturating_add(1))
}

/// A file of a multi-file or native game: `<game folder>/<file>`.
pub fn game_file_rel(game_dir: &str, file_name: &str) -> String {
    format!("{game_dir}/{}", pack_component(file_name, "file"))
}

/// A game's art folder: `assets/<slug>/<stem>`.
pub fn asset_dir_rel(platform_slug: &str, game_stem: &str) -> String {
    format!(
        "assets/{}/{}",
        pack_component(platform_slug, "platform"),
        pack_component(game_stem, "game")
    )
}

/// One emulator release for one OS: `emulators/<name>/<version>/<arch tag>`.
/// The name is lowercased, as in `emulators/duckstation/…`.
pub fn emulator_dir_rel(name: &str, version: &str, os: PackOs) -> String {
    format!(
        "emulators/{}/{}/{}",
        pack_component(&name.to_lowercase(), "emulator"),
        pack_component(version, "version"),
        os.arch_tag()
    )
}

/// A platform's firmware folder: `firmware/<slug>`.
pub fn firmware_dir_rel(platform_slug: &str) -> String {
    format!("firmware/{}", pack_component(platform_slug, "platform"))
}

/// `candidate`, or `candidate` with ` [rom <id>]` added when another game in
/// the pack already owns that path, a path inside it, or a folder above it.
/// The compare folds case, as FAT, exFAT and NTFS do. For a file
/// (`is_file`) the suffix goes before the extension.
pub fn unique_rel(
    pack: &Pack,
    candidate: &str,
    server: &str,
    rom_id: i64,
    is_file: bool,
) -> String {
    let own = server_key(server);
    let taken: Vec<String> = pack
        .games
        .iter()
        .filter(|game| !(game.source.rom_id == rom_id && server_key(&game.source.server) == own))
        .flat_map(PackGame::paths)
        .map(str::to_lowercase)
        .collect();
    let folded = candidate.to_lowercase();
    let collides = taken.iter().any(|path| {
        *path == folded
            || path.starts_with(&format!("{folded}/"))
            || folded.starts_with(&format!("{path}/"))
    });
    if !collides {
        return candidate.to_string();
    }
    let (parent, leaf) = match candidate.rfind('/') {
        Some(slash) => (&candidate[..=slash], &candidate[slash + 1..]),
        None => ("", candidate),
    };
    let suffix = format!(" [rom {rom_id}]");
    let (stem, ext) = match leaf.rfind('.') {
        Some(dot) if is_file && dot > 0 => leaf.split_at(dot),
        _ => (leaf, ""),
    };
    let mut cut = stem
        .len()
        .min(MAX_COMPONENT_BYTES.saturating_sub(suffix.len() + ext.len()));
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{parent}{}{suffix}{ext}", &stem[..cut])
}

/// A server URL reduced to what identifies the server: no userinfo, the
/// host lowercased, the port made explicit, no trailing slash. Two URLs
/// with the same key name the same server.
pub fn server_key(url: &str) -> String {
    let stripped = crate::romm::strip_userinfo(url.trim());
    match url::Url::parse(&stripped) {
        Ok(parsed) => format!(
            "{}:{}{}",
            parsed.host_str().unwrap_or("").to_ascii_lowercase(),
            parsed.port_or_known_default().unwrap_or(0),
            parsed.path().trim_end_matches('/')
        ),
        Err(_) => stripped.trim_end_matches('/').to_ascii_lowercase(),
    }
}

/// A pack release date as the seconds string `RomDetail` carries: a number
/// passes through, `YYYY-MM-DD` is converted, anything else is blank.
fn release_date_seconds(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.parse::<i64>().is_ok() {
        return trimmed.to_string();
    }
    chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|time| time.and_utc().timestamp().to_string())
        .unwrap_or_default()
}

/// The `RomDetail` a backed-up game stands for: the inverse of the backup
/// mapping. Its `files` rebuild the server's file list (id, name, size,
/// category, top level), so `plan_install` picks the targets a server
/// install would. What a pack does not carry (platform id, video, related
/// games, achievements) is blank.
pub fn pack_game_detail(game: &PackGame) -> RomDetail {
    let source = &game.source;
    RomDetail {
        id: source.rom_id,
        name: game.name.clone(),
        platform_id: 0,
        platform_name: game.platform_name.clone(),
        platform_slug: game.platform.clone(),
        fs_name: source.fs_name.clone(),
        description: game.summary.clone(),
        regions: game.region.clone(),
        languages: game.languages.clone(),
        tags: game.tags.clone(),
        revision: game.revision.clone(),
        rating: game.rating.clone(),
        genres: game.genres.join(", "),
        companies: game.companies.join(", "),
        first_release_date: release_date_seconds(&game.release_date),
        franchises: game.franchises.join(", "),
        game_modes: game.game_modes.join(", "),
        player_count: game.players.clone(),
        filesize_bytes: i64::try_from(game.size).unwrap_or(i64::MAX),
        server_updated_at: source.server_updated_at.clone(),
        files: game
            .files
            .iter()
            .map(|file| RomFile {
                id: file.server_file_id,
                file_name: file.name.clone(),
                file_size_bytes: i64::try_from(file.size).unwrap_or(i64::MAX),
                is_top_level: file.top_level,
                last_modified: String::new(),
                category: file.category.clone(),
                md5_hash: file.md5.clone(),
                sha1_hash: file.sha1.clone(),
            })
            .collect(),
        cover_small_path: source.cover_small_path.clone(),
        cover_large_path: source.cover_large_path.clone(),
        screenshot_urls: source.screenshot_urls.clone(),
        fanart_urls: source.fanart_urls.clone(),
        youtube_video_id: String::new(),
        video_path: String::new(),
        is_identified: source.igdb_id.is_some(),
        related: Vec::new(),
        achievement_count: 0,
        logo_url: source.logo_url.clone(),
    }
}

impl PackGame {
    /// Every pack-relative path this game's entry names: files, an
    /// installed copy's archive parts, and art.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        let assets = &self.assets;
        self.files
            .iter()
            .map(|file| file.path.as_str())
            .chain(
                self.installed
                    .iter()
                    .flat_map(|copy| copy.parts.iter().map(|part| part.path.as_str())),
            )
            .chain(
                [
                    &assets.cover,
                    &assets.cover_small,
                    &assets.fanart,
                    &assets.logo,
                ]
                .into_iter()
                .flatten()
                .map(String::as_str),
            )
            .chain(assets.screenshots.iter().map(String::as_str))
    }

    /// Whether the entry is a copy of an install (an archive in parts)
    /// rather than the server's files.
    pub fn is_installed_copy(&self) -> bool {
        self.installed.is_some()
    }

    fn is(&self, key: &str, rom_id: i64) -> bool {
        self.source.rom_id == rom_id && server_key(&self.source.server) == key
    }
}

impl Pack {
    /// Every pack-relative path the index names.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.games
            .iter()
            .flat_map(PackGame::paths)
            .chain(self.emulators.iter().flat_map(|emulator| {
                std::iter::once(emulator.path.as_str())
                    .chain(emulator.supplementals.iter().map(String::as_str))
            }))
            .chain(
                self.firmware
                    .iter()
                    .flat_map(|set| set.files.iter().map(|file| file.path.as_str())),
            )
    }

    /// The game backed up from `rom_id` on `server` (compared by
    /// [`server_key`]).
    pub fn find_game(&self, server: &str, rom_id: i64) -> Option<&PackGame> {
        let key = server_key(server);
        self.games.iter().find(|game| game.is(&key, rom_id))
    }

    /// Whether the pack holds `rom_id` from `server` (compared by
    /// [`server_key`]).
    pub fn has_game(&self, server: &str, rom_id: i64) -> bool {
        self.find_game(server, rom_id).is_some()
    }

    /// Adds a game, or replaces the entry for the same server and rom id.
    /// Returns the entry it replaced.
    pub fn upsert_game(&mut self, game: PackGame) -> Option<PackGame> {
        let key = server_key(&game.source.server);
        let rom_id = game.source.rom_id;
        match self.games.iter_mut().find(|old| old.is(&key, rom_id)) {
            Some(old) => Some(std::mem::replace(old, game)),
            None => {
                self.games.push(game);
                None
            }
        }
    }

    /// Removes the game backed up from `rom_id` on `server`.
    pub fn remove_game(&mut self, server: &str, rom_id: i64) -> Option<PackGame> {
        let key = server_key(server);
        let index = self.games.iter().position(|game| game.is(&key, rom_id))?;
        Some(self.games.remove(index))
    }

    /// Adds an emulator release, or replaces the one with the same name for
    /// the same OS. Returns the entry it replaced.
    pub fn upsert_emulator(&mut self, emulator: PackEmulator) -> Option<PackEmulator> {
        match self
            .emulators
            .iter_mut()
            .find(|old| old.name == emulator.name && old.os == emulator.os)
        {
            Some(old) => Some(std::mem::replace(old, emulator)),
            None => {
                self.emulators.push(emulator);
                None
            }
        }
    }

    /// Removes every emulator release for `os`, and returns them so their
    /// files can be deleted. The pack's `os` list is left to the caller.
    pub fn remove_emulators_for_os(&mut self, os: PackOs) -> Vec<PackEmulator> {
        let tag = os.arch_tag();
        let (removed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.emulators)
            .into_iter()
            .partition(|emulator| emulator.os == tag);
        self.emulators = kept;
        removed
    }

    /// Adds a platform's firmware, or replaces the set for that platform.
    /// Returns the set it replaced.
    pub fn upsert_firmware(&mut self, firmware: PackFirmware) -> Option<PackFirmware> {
        match self
            .firmware
            .iter_mut()
            .find(|old| old.platform == firmware.platform)
        {
            Some(old) => Some(std::mem::replace(old, firmware)),
            None => {
                self.firmware.push(firmware);
                None
            }
        }
    }

    /// The bytes the index accounts for: every game, emulator and firmware
    /// file. The usage bar's "GRID" share.
    pub fn pack_bytes(&self) -> u64 {
        let games: u64 = self.games.iter().map(|game| game.size).sum();
        let emulators: u64 = self.emulators.iter().map(|emulator| emulator.size).sum();
        let firmware: u64 = self
            .firmware
            .iter()
            .flat_map(|set| set.files.iter().map(|file| file.size))
            .sum();
        games.saturating_add(emulators).saturating_add(firmware)
    }
}

/// The process-wide lock per drive: every edit of a drive's index is one
/// read-modify-write under that drive's lock. Keyed by mount path.
pub struct PackLock;

impl PackLock {
    /// The lock for the drive mounted at `mount`. The same mount path always
    /// gives the same lock.
    pub fn for_mount(mount: &Path) -> Arc<Mutex<()>> {
        static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
        let mut locks = LOCKS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        locks.entry(mount.to_path_buf()).or_default().clone()
    }
}

/// One locked read-modify-write of a drive's index: reads it, applies
/// `edit`, stamps `updated` and writes it atomically. A drive that is not
/// set up, is unreadable or was written by a newer GRID is refused.
pub fn edit_pack<R>(mount: &Path, edit: impl FnOnce(&mut Pack) -> R) -> Result<R, PackError> {
    let lock = PackLock::for_mount(mount);
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let mut pack = match read_pack(mount)? {
        PackState::Ready(pack) => pack,
        PackState::Absent => return Err(PackError::NotSetUp),
        PackState::NewerFormat(format) => return Err(PackError::NewerFormat(format)),
        PackState::Unreadable(why) => return Err(PackError::Unreadable(why)),
    };
    let result = edit(&mut pack);
    pack.updated = now_rfc3339();
    write_pack_atomic(mount, &pack)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's draft, with the fields the schema requires filled in, plus an
    /// unknown top-level key.
    const DRAFT: &str = r#"{
      "format": 1,
      "name": "Couch kit",
      "os": ["linux", "windows"],
      "created": "2026-10-12T18:20:00Z",
      "updated": "2026-11-07T09:02:00Z",
      "games": [
        {
          "platform": "genesis", "platform_name": "Sega Genesis",
          "source": { "server": "https://romm.home.lan", "rom_id": 1234, "igdb_id": 1991 },
          "name": "Aladdin", "summary": "A street rat.", "region": "USA",
          "companies": ["Virgin Games"], "release_date": "1993-11-11",
          "genres": ["Platform"], "game_modes": ["Single player"], "players": "1",
          "version": "1.0", "size": 2097152, "sha1": "abc",
          "files": [ { "path": "games/genesis/Aladdin (USA).zip", "size": 2097152, "server_file_id": 5678, "name": "Aladdin (USA).zip", "category": "game" } ],
          "assets": { "cover": "assets/genesis/Aladdin (USA)/cover.jpg", "fanart": "assets/genesis/Aladdin (USA)/fanart.jpg", "logo": "assets/genesis/Aladdin (USA)/logo.png", "screenshots": ["assets/genesis/Aladdin (USA)/screenshots/1.jpg"] },
          "added": "2026-11-07T09:01:00Z"
        }
      ],
      "emulators": [ { "name": "DuckStation", "kind": "emulator", "source_id": "stenzek/duckstation", "version": "v0.1-9226", "os": "linux-x86_64", "platforms": ["psx"], "path": "emulators/duckstation/v0.1-9226/linux-x86_64/DuckStation.AppImage", "asset_name": "DuckStation.AppImage", "size": 40000000 } ],
      "firmware": [ { "platform": "psx", "platform_name": "Sony PlayStation", "files": [ { "name": "scph5501.bin", "path": "firmware/psx/scph5501.bin", "size": 524288, "md5": "490f666e1afb15b7362b406ed1cea246" } ] } ],
      "future_key": { "kept": [1, 2, 3] }
    }"#;

    /// The ClasPlatf example drive's prototype shape: games keyed by "0", a
    /// nested "platform" object, no format.
    const CLASPLATF: &str = r#"{
      "gridpack": { "name": "Classic Platformers", "desc": "A bundle of classic platformers for several platforms." },
      "games": {
        "platform": {
          "name": "genesis", "num-games": "33",
          "games": {
            "0": {
              "name": "Aladdin", "region": "USA", "release-date": "1999-01-01", "players": "1",
              "assets": { "rom": "games/genesis/Aladdin (USA).zip", "cover": "assets/genesis/Aladdin-cover.jpg" }
            }
          }
        }
      }
    }"#;

    #[test]
    fn spec_draft_round_trips_with_unknown_keys() {
        let pack: Pack = serde_json::from_str(DRAFT).expect("draft parses");
        assert_eq!(pack.format, FORMAT);
        assert_eq!(pack.os, vec![PackOs::Linux, PackOs::Windows]);
        let game = &pack.games[0];
        assert_eq!(game.source.rom_id, 1234);
        assert_eq!(game.source.igdb_id, Some(1991));
        assert_eq!(game.files[0].server_file_id, 5678);
        assert!(game.files[0].top_level, "top_level defaults to true");
        assert_eq!(game.assets.cover_small, None);
        assert_eq!(pack.emulators[0].program, "");
        assert_eq!(pack.firmware[0].files[0].size, 524288);
        assert_eq!(
            pack.extra.get("future_key"),
            Some(&serde_json::json!({ "kept": [1, 2, 3] }))
        );

        let written = serde_json::to_value(&pack).expect("serializes");
        assert_eq!(
            written["future_key"],
            serde_json::json!({ "kept": [1, 2, 3] })
        );
        assert!(written.get("id").is_none(), "a pack has no id");
        let again: Pack = serde_json::from_value(written).expect("reparses");
        assert_eq!(again, pack);
    }

    #[test]
    fn clasplatf_prototype_shape_is_unreadable() {
        assert!(serde_json::from_str::<Pack>(CLASPLATF).is_err());
        let drive = tempfile::tempdir().unwrap();
        write_index(drive.path(), CLASPLATF);
        assert!(matches!(
            read_pack(drive.path()).unwrap(),
            PackState::Unreadable(_)
        ));
    }

    #[test]
    fn unknown_os_does_not_parse() {
        let bad = DRAFT.replace(r#"["linux", "windows"]"#, r#"["linux", "macos"]"#);
        assert!(serde_json::from_str::<Pack>(&bad).is_err());
    }

    #[test]
    fn os_tags() {
        assert_eq!(PackOs::Linux.forge_host(), "linux");
        assert_eq!(PackOs::Windows.forge_host(), "win32");
        assert_eq!(PackOs::Linux.arch_tag(), "linux-x86_64");
        assert_eq!(PackOs::Windows.arch_tag(), "windows-x86_64");
        assert_eq!(
            serde_json::to_string(&PackOs::Windows).unwrap(),
            "\"windows\""
        );
    }

    /// A game backed up from this computer's install: no `files`, an
    /// `installed` block with its parts.
    fn installed_game() -> serde_json::Value {
        serde_json::json!({
            "platform": "ps4", "platform_name": "PlayStation 4",
            "source": { "server": "https://romm.home.lan", "rom_id": 77 },
            "name": "Ember Run", "size": 6442450944_u64, "added": "2026-11-09T10:00:00Z",
            "installed": {
                "archive": "tar", "layout": "extracted", "name": "Ember Run",
                "parts": [
                    { "path": "games/ps4/Ember Run/installed.tar.001", "size": 4293918720_u64, "md5": "aa" },
                    { "path": "games/ps4/Ember Run/installed.tar.002", "size": 2148532224_u64, "md5": "bb" }
                ],
                "tree_bytes": 6400000000_u64, "tree_files": 1200,
                "launch": "eboot.bin", "rom_file_name": "Ember Run (v1.02).pkg",
                "server_updated_at": "2026-10-01T00:00:00Z", "version": "v1.02",
                "outdated": true, "from_os": "linux",
                "row": { "ps4_game_id": "CUSA00001", "ra_id": "12" }
            }
        })
    }

    #[test]
    fn an_installed_block_round_trips() {
        let game: PackGame = serde_json::from_value(installed_game()).expect("parses");
        assert!(game.files.is_empty(), "files defaults to empty");
        let copy = game.installed.as_ref().expect("installed block");
        assert_eq!(copy.archive, ArchiveKind::Tar);
        assert_eq!(copy.layout, InstalledLayout::Extracted);
        assert_eq!(copy.parts.len(), 2);
        assert_eq!(copy.parts[1].path, "games/ps4/Ember Run/installed.tar.002");
        assert_eq!(copy.from_os, Some(PackOs::Linux));
        assert!(copy.outdated);
        assert_eq!(copy.row.ps4_game_id, "CUSA00001");
        assert_eq!(copy.row.native_executable, "");

        let written = serde_json::to_value(&game).unwrap();
        assert_eq!(written["installed"]["archive"], "tar");
        assert_eq!(written["installed"]["layout"], "extracted");
        let again: PackGame = serde_json::from_value(written).unwrap();
        assert_eq!(again, game);
    }

    #[test]
    fn a_game_without_an_installed_block_writes_none() {
        let written = serde_json::to_value(&draft().games[0]).unwrap();
        assert!(written.get("installed").is_none());
    }

    #[test]
    fn unknown_archive_and_layout_parse_as_unknown() {
        let mut value = installed_game();
        value["installed"]["archive"] = serde_json::json!("zstd-tar");
        value["installed"]["layout"] = serde_json::json!("hologram");
        let game: PackGame = serde_json::from_value(value).expect("still parses");
        let copy = game.installed.unwrap();
        assert_eq!(copy.archive, ArchiveKind::Unknown);
        assert_eq!(copy.layout, InstalledLayout::Unknown);
    }

    /// A pack whose only game is [`installed_game`], as `gridpack.json` bytes.
    fn installed_pack(game: serde_json::Value) -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_str(DRAFT).unwrap();
        value["games"] = serde_json::json!([game]);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn an_unknown_from_os_reads_as_none() {
        for odd in [
            serde_json::json!("macos"),
            serde_json::json!(7),
            serde_json::Value::Null,
        ] {
            let mut value = installed_game();
            value["installed"]["from_os"] = odd.clone();
            let PackState::Ready(pack) = parse_pack(&installed_pack(value)) else {
                panic!("from_os {odd} made the pack unreadable");
            };
            assert_eq!(pack.games[0].installed.as_ref().unwrap().from_os, None);
        }
        let mut value = installed_game();
        value["installed"]
            .as_object_mut()
            .unwrap()
            .remove("from_os");
        let game: PackGame = serde_json::from_value(value).unwrap();
        assert_eq!(game.installed.unwrap().from_os, None);
        let mut value = installed_game();
        value["installed"]["from_os"] = serde_json::json!("windows");
        let game: PackGame = serde_json::from_value(value).unwrap();
        assert_eq!(game.installed.unwrap().from_os, Some(PackOs::Windows));
    }

    #[test]
    fn an_unknown_archive_or_layout_keeps_the_pack_ready() {
        let mut value = installed_game();
        value["installed"]["archive"] = serde_json::json!("zstd-tar");
        value["installed"]["layout"] = serde_json::json!("hologram");
        let PackState::Ready(pack) = parse_pack(&installed_pack(value)) else {
            panic!("an unknown archive or layout made the pack unreadable");
        };
        let game = &pack.games[0];
        assert!(game.is_installed_copy());
        assert!(!game.installed.as_ref().unwrap().is_supported());
        assert_eq!(game.paths().count(), 2, "its parts are still known");

        let PackState::Ready(pack) = parse_pack(&installed_pack(installed_game())) else {
            panic!("ready");
        };
        assert!(pack.games[0].installed.as_ref().unwrap().is_supported());
    }

    #[test]
    fn an_unsafe_part_path_is_unreadable() {
        for bad in [
            "../../etc/passwd",
            "/abs/installed.tar.001",
            "games\\x.tar.001",
            "",
        ] {
            let mut value = installed_game();
            value["installed"]["parts"][1]["path"] = serde_json::json!(bad);
            assert!(
                matches!(parse_pack(&installed_pack(value)), PackState::Unreadable(_)),
                "part path {bad:?} was accepted"
            );
        }
    }

    #[test]
    fn paths_include_the_installed_parts() {
        let mut game: PackGame = serde_json::from_value(installed_game()).unwrap();
        game.assets.cover = Some("assets/ps4/Ember Run/cover.jpg".to_string());
        let paths: Vec<&str> = game.paths().collect();
        assert_eq!(
            paths,
            [
                "games/ps4/Ember Run/installed.tar.001",
                "games/ps4/Ember Run/installed.tar.002",
                "assets/ps4/Ember Run/cover.jpg",
            ]
        );
        let mut pack = new_pack("Couch kit", &[PackOs::Linux]);
        pack.games.push(game.clone());
        assert_eq!(pack.paths().count(), 3);
        assert!(!draft().games[0].is_installed_copy());
        assert!(game.is_installed_copy());
    }

    #[test]
    fn parts_collide_like_a_folder_game() {
        let mut pack = new_pack("Couch kit", &[PackOs::Linux]);
        pack.games
            .push(serde_json::from_value(installed_game()).unwrap());
        let candidate = game_dir_rel("ps4", "Ember Run", None);
        assert_eq!(
            unique_rel(&pack, &candidate, "https://other.lan", 9, false),
            "games/ps4/Ember Run [rom 9]"
        );
        assert_eq!(
            unique_rel(&pack, &candidate, "https://romm.home.lan", 77, false),
            candidate,
            "a game's own parts never collide with it"
        );
    }

    #[test]
    fn installed_part_names() {
        let dir = game_dir_rel("ps4", "Ember Run", None);
        assert_eq!(
            installed_part_rel(&dir, 0),
            "games/ps4/Ember Run/installed.tar.001"
        );
        assert_eq!(
            installed_part_rel(&dir, 1),
            "games/ps4/Ember Run/installed.tar.002"
        );
        assert_eq!(
            installed_part_rel(&dir, 998),
            "games/ps4/Ember Run/installed.tar.999"
        );
        assert_eq!(
            installed_part_rel(&dir, 999),
            "games/ps4/Ember Run/installed.tar.1000"
        );
        assert!(validate_rel(&installed_part_rel(&dir, 0)).is_ok());
        assert!(PART_BYTES < FAT32_MAX_FILE);
    }

    #[test]
    fn an_installed_copy_counts_its_parts() {
        let game: PackGame = serde_json::from_value(installed_game()).unwrap();
        let copy = game.installed.as_ref().unwrap();
        assert_eq!(copy.parts_bytes(), 4293918720 + 2148532224);
        assert_eq!(
            copy.parts_bytes(),
            game.size,
            "the fixture's size is the parts' sum"
        );
        let mut pack = new_pack("Couch kit", &[PackOs::Linux]);
        pack.games.push(game);
        assert_eq!(pack.pack_bytes(), 4293918720 + 2148532224);
    }

    #[test]
    fn edit_pack_keeps_installed_blocks_and_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let mut game = installed_game();
        game["future_game"] = serde_json::json!({ "kept": true });
        write_index(
            dir.path(),
            std::str::from_utf8(&installed_pack(game)).unwrap(),
        );
        edit_pack(dir.path(), |pack| pack.name = "Renamed".to_string()).unwrap();
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(index_path(dir.path())).unwrap()).unwrap();
        assert_eq!(written["name"], "Renamed");
        assert_eq!(written["games"][0]["future_game"]["kept"], true);
        assert_eq!(written["games"][0]["installed"]["parts"][1]["md5"], "bb");
        assert_eq!(written["future_key"]["kept"][0], 1);
    }

    #[test]
    fn a_pack_without_game_files_parses() {
        let mut value: serde_json::Value = serde_json::from_str(DRAFT).unwrap();
        value["games"][0].as_object_mut().unwrap().remove("files");
        let pack: Pack = serde_json::from_value(value).expect("parses");
        assert!(pack.games[0].files.is_empty());
        assert!(pack.games[0].installed.is_none());
    }

    #[test]
    fn unknown_entry_keys_round_trip() {
        let mut value: serde_json::Value = serde_json::from_str(DRAFT).unwrap();
        let kept = serde_json::json!({ "kept": true });
        value["games"][0]["future_game"] = kept.clone();
        value["games"][0]["files"][0]["future_file"] = kept.clone();
        value["emulators"][0]["future_emulator"] = kept.clone();
        value["firmware"][0]["future_firmware"] = kept.clone();
        let pack: Pack = serde_json::from_value(value).expect("parses");
        assert_eq!(pack.games[0].extra.get("future_game"), Some(&kept));
        assert_eq!(pack.games[0].files[0].extra.get("future_file"), Some(&kept));
        assert_eq!(pack.emulators[0].extra.get("future_emulator"), Some(&kept));
        assert_eq!(pack.firmware[0].extra.get("future_firmware"), Some(&kept));

        let written = serde_json::to_value(&pack).unwrap();
        assert_eq!(written["games"][0]["future_game"], kept);
        assert_eq!(written["games"][0]["files"][0]["future_file"], kept);
        assert_eq!(written["emulators"][0]["future_emulator"], kept);
        assert_eq!(written["firmware"][0]["future_firmware"], kept);
        assert!(
            !pack.extra.contains_key("future_game"),
            "kept on the entry, not the pack"
        );
        let again: Pack = serde_json::from_value(written).unwrap();
        assert_eq!(again, pack);
    }

    fn write_index(mount: &Path, json: &str) {
        std::fs::create_dir_all(pack_dir(mount)).unwrap();
        std::fs::write(index_path(mount), json).unwrap();
    }

    fn draft() -> Pack {
        serde_json::from_str(DRAFT).unwrap()
    }

    fn game(server: &str, rom_id: i64, files: &[&str]) -> PackGame {
        let mut game = draft().games.remove(0);
        game.source.server = server.to_string();
        game.source.rom_id = rom_id;
        game.files = files
            .iter()
            .enumerate()
            .map(|(index, path)| PackFile {
                path: path.to_string(),
                size: 10,
                server_file_id: index as i64 + 1,
                name: path.rsplit('/').next().unwrap().to_string(),
                category: String::new(),
                top_level: true,
                md5: String::new(),
                sha1: String::new(),
                extra: Default::default(),
            })
            .collect();
        game.assets = PackAssets::default();
        game
    }

    #[test]
    fn validate_rel_accepts_plain_relative_paths() {
        assert_eq!(
            validate_rel("games/genesis/Aladdin (USA).zip").unwrap(),
            Path::new("games").join("genesis").join("Aladdin (USA).zip")
        );
        assert!(validate_rel("a").is_ok());
        assert!(validate_rel(&"x".repeat(MAX_COMPONENT_BYTES)).is_ok());
    }

    #[test]
    fn validate_rel_refuses_unsafe_paths() {
        for bad in [
            "",
            "..",
            "../outside",
            "games/../../etc/passwd",
            "games/./x",
            "/etc/passwd",
            "C:\\Windows",
            "C:/Windows",
            "C:",
            "games\\genesis\\x.zip",
            "games//x",
            "games/x/",
            "nul\0byte",
        ] {
            assert!(validate_rel(bad).is_err(), "{bad:?} must be refused");
        }
        let long = format!("games/{}", "x".repeat(MAX_COMPONENT_BYTES + 1));
        assert!(validate_rel(&long).is_err());
    }

    #[test]
    fn resolve_joins_under_the_pack_folder() {
        let drive = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(pack_dir(drive.path())).unwrap();
        assert_eq!(
            resolve(drive.path(), "games/x.zip").unwrap(),
            pack_dir(drive.path()).join("games").join("x.zip")
        );
        assert!(resolve(drive.path(), "../x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_refuses_a_link_out_of_the_pack() {
        let drive = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(pack_dir(drive.path())).unwrap();
        std::os::unix::fs::symlink(outside.path(), pack_dir(drive.path()).join("games")).unwrap();
        assert!(resolve(drive.path(), "games/x.zip").is_err());
        assert!(resolve(drive.path(), "games").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_pack_folder_is_refused() {
        let drive = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), pack_dir(drive.path())).unwrap();
        assert!(resolve(drive.path(), "games/x.zip").is_err());
        assert!(write_pack_atomic(drive.path(), &draft()).is_err());
        assert!(matches!(
            read_pack(drive.path()).unwrap(),
            PackState::Unreadable(_)
        ));
        assert_eq!(cleanup_leftovers(drive.path()).unwrap(), 0);
    }

    #[test]
    fn a_missing_index_is_absent() {
        let drive = tempfile::tempdir().unwrap();
        assert_eq!(read_pack(drive.path()).unwrap(), PackState::Absent);
    }

    #[test]
    fn atomic_write_round_trips_and_leaves_no_tmp() {
        let drive = tempfile::tempdir().unwrap();
        let pack = draft();
        write_pack_atomic(drive.path(), &pack).unwrap();
        assert_eq!(read_pack(drive.path()).unwrap(), PackState::Ready(pack));
        let names: Vec<String> = std::fs::read_dir(pack_dir(drive.path()))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![PACK_FILE.to_string()]);
    }

    #[test]
    fn unknown_keys_survive_a_write() {
        let drive = tempfile::tempdir().unwrap();
        write_index(drive.path(), DRAFT);
        let PackState::Ready(pack) = read_pack(drive.path()).unwrap() else {
            panic!("draft reads");
        };
        write_pack_atomic(drive.path(), &pack).unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(index_path(drive.path())).unwrap()).unwrap();
        assert_eq!(raw["future_key"], serde_json::json!({ "kept": [1, 2, 3] }));
    }

    #[test]
    fn a_crash_leftover_is_ignored_then_cleaned() {
        let drive = tempfile::tempdir().unwrap();
        let pack = draft();
        write_pack_atomic(drive.path(), &pack).unwrap();
        let tmp = pack_dir(drive.path()).join(format!("{PACK_FILE}{TMP_SUFFIX}"));
        std::fs::write(&tmp, b"{ \"format\": 1, \"name\": \"half writ").unwrap();

        assert_eq!(read_pack(drive.path()).unwrap(), PackState::Ready(pack));
        assert_eq!(cleanup_leftovers(drive.path()).unwrap(), 1);
        assert!(!tmp.exists());
        assert!(index_path(drive.path()).exists());
    }

    #[test]
    fn cleanup_removes_only_gridtmp_under_the_pack() {
        let drive = tempfile::tempdir().unwrap();
        let root = drive.path();
        let pack = pack_dir(root);
        let games = pack.join("games").join("genesis");
        std::fs::create_dir_all(&games).unwrap();
        std::fs::write(games.join("Aladdin.zip"), b"rom").unwrap();
        std::fs::write(games.join("Sonic.zip.gridtmp"), b"half").unwrap();
        std::fs::write(games.join("notes.gridtmp.txt"), b"keep").unwrap();
        let half_dir = pack.join("emulators").join("x.gridtmp");
        std::fs::create_dir_all(half_dir.join("inner")).unwrap();
        std::fs::write(half_dir.join("inner").join("a"), b"a").unwrap();
        std::fs::write(root.join("outside.gridtmp"), b"user").unwrap();
        std::fs::create_dir_all(root.join("Photos")).unwrap();
        std::fs::write(root.join("Photos").join("p.gridtmp"), b"user").unwrap();

        assert_eq!(cleanup_leftovers(root).unwrap(), 2);
        assert!(games.join("Aladdin.zip").exists());
        assert!(games.join("notes.gridtmp.txt").exists());
        assert!(!games.join("Sonic.zip.gridtmp").exists());
        assert!(!half_dir.exists());
        assert!(pack.join("emulators").exists());
        assert!(root.join("outside.gridtmp").exists());
        assert!(root.join("Photos").join("p.gridtmp").exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_does_not_follow_links() {
        let drive = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("user.gridtmp"), b"user").unwrap();
        std::fs::create_dir_all(pack_dir(drive.path())).unwrap();
        std::os::unix::fs::symlink(outside.path(), pack_dir(drive.path()).join("link")).unwrap();

        assert_eq!(cleanup_leftovers(drive.path()).unwrap(), 0);
        assert!(outside.path().join("user.gridtmp").exists());
    }

    #[test]
    fn a_newer_format_is_read_only() {
        let drive = tempfile::tempdir().unwrap();
        write_index(
            drive.path(),
            &DRAFT.replace("\"format\": 1", "\"format\": 2"),
        );
        assert_eq!(read_pack(drive.path()).unwrap(), PackState::NewerFormat(2));
        assert!(matches!(
            edit_pack(drive.path(), |_| ()),
            Err(PackError::NewerFormat(2))
        ));
        let mut pack = draft();
        pack.format = 2;
        assert!(write_pack_atomic(drive.path(), &pack).is_err());
    }

    #[test]
    fn a_missing_or_zero_format_is_unreadable() {
        assert!(matches!(
            parse_pack(DRAFT.replace("\"format\": 1,", "").as_bytes()),
            PackState::Unreadable(_)
        ));
        assert!(matches!(
            parse_pack(DRAFT.replace("\"format\": 1", "\"format\": 0").as_bytes()),
            PackState::Unreadable(_)
        ));
        assert!(matches!(parse_pack(b"not json"), PackState::Unreadable(_)));
    }

    #[test]
    fn an_unsafe_path_in_the_index_is_unreadable() {
        let bad = DRAFT.replace("firmware/psx/scph5501.bin", "../../home/user/.bashrc");
        assert!(matches!(
            parse_pack(bad.as_bytes()),
            PackState::Unreadable(_)
        ));
    }

    #[test]
    fn an_oversized_index_is_unreadable() {
        let drive = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(pack_dir(drive.path())).unwrap();
        let file = std::fs::File::create(index_path(drive.path())).unwrap();
        file.set_len(MAX_INDEX_BYTES + 1).unwrap();
        assert!(matches!(
            read_pack(drive.path()).unwrap(),
            PackState::Unreadable(_)
        ));
    }

    #[test]
    fn new_pack_has_no_id_and_deduplicates_os() {
        let pack = new_pack(
            "Couch kit",
            &[PackOs::Linux, PackOs::Linux, PackOs::Windows],
        );
        assert_eq!(pack.format, FORMAT);
        assert_eq!(pack.os, vec![PackOs::Linux, PackOs::Windows]);
        assert_eq!(pack.created, pack.updated);
        assert!(chrono::DateTime::parse_from_rfc3339(&pack.created).is_ok());
        let value = serde_json::to_value(&pack).unwrap();
        assert!(value.get("id").is_none());
    }

    #[test]
    fn edit_pack_is_a_locked_read_modify_write() {
        let drive = tempfile::tempdir().unwrap();
        assert!(matches!(
            edit_pack(drive.path(), |_| ()),
            Err(PackError::NotSetUp)
        ));
        let mut pack = new_pack("Kit", &[PackOs::Linux]);
        pack.updated = "2000-01-01T00:00:00Z".to_string();
        write_pack_atomic(drive.path(), &pack).unwrap();
        let replaced = edit_pack(drive.path(), |pack| {
            pack.upsert_game(game("https://romm.lan", 7, &["games/a/x.zip"]))
        })
        .unwrap();
        assert!(replaced.is_none());
        let PackState::Ready(after) = read_pack(drive.path()).unwrap() else {
            panic!("ready");
        };
        assert!(after.has_game("https://romm.lan", 7));
        assert_ne!(after.updated, "2000-01-01T00:00:00Z");
    }

    #[test]
    fn pack_lock_is_shared_per_mount() {
        let a = PackLock::for_mount(Path::new("/run/media/u/A"));
        let again = PackLock::for_mount(Path::new("/run/media/u/A"));
        let b = PackLock::for_mount(Path::new("/run/media/u/B"));
        assert!(Arc::ptr_eq(&a, &again));
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn layout_paths() {
        assert_eq!(
            game_dir_rel("genesis", "Aladdin (USA).zip", Some("Aladdin (USA).zip")),
            "games/genesis/Aladdin (USA).zip"
        );
        let dir = game_dir_rel("ngc", "Wario World (igdb-1700)", None);
        assert_eq!(dir, "games/ngc/Wario World (igdb-1700)");
        assert_eq!(
            game_file_rel(&dir, "Wario World.rvz"),
            "games/ngc/Wario World (igdb-1700)/Wario World.rvz"
        );
        assert_eq!(
            asset_dir_rel("genesis", "Aladdin (USA)"),
            "assets/genesis/Aladdin (USA)"
        );
        assert_eq!(
            emulator_dir_rel("DuckStation", "v0.1-9226", PackOs::Linux),
            "emulators/duckstation/v0.1-9226/linux-x86_64"
        );
        assert_eq!(firmware_dir_rel("psx"), "firmware/psx");
        assert_eq!(
            game_dir_rel("pc", "Half: Life?", None),
            "games/pc/Half_ Life_"
        );
        for rel in [dir, game_dir_rel("a/b", "..", None)] {
            assert!(validate_rel(&rel).is_ok(), "{rel}");
        }
    }

    #[test]
    fn pack_component_handles_reserved_names_and_length() {
        assert_eq!(pack_component("CON", "x"), "CON_");
        assert_eq!(pack_component("aux.txt", "x"), "aux_.txt");
        assert_eq!(pack_component("Com1 .zip", "x"), "Com1_.zip");
        assert_eq!(pack_component("Console.zip", "x"), "Console.zip");
        let long = format!("{}.zip", "é".repeat(200));
        let cut = pack_component(&long, "x");
        assert!(cut.len() <= MAX_COMPONENT_BYTES, "{}", cut.len());
        assert!(cut.ends_with(".zip"));
        assert!(validate_rel(&cut).is_ok());
    }

    #[test]
    fn collisions_fold_case_and_get_the_rom_suffix() {
        let mut pack = draft();
        pack.games = vec![
            game("https://romm.lan", 1, &["games/genesis/Aladdin (USA).zip"]),
            game("https://romm.lan", 2, &["games/ngc/Wario World/disc.rvz"]),
        ];
        let server = "https://romm.lan";
        // A different game, same name in another case: suffixed before the extension.
        assert_eq!(
            unique_rel(&pack, "games/genesis/ALADDIN (usa).zip", server, 9, true),
            "games/genesis/ALADDIN (usa) [rom 9].zip"
        );
        // A folder that another game's file lives in: the folder is suffixed.
        assert_eq!(
            unique_rel(&pack, "games/ngc/wario world", server, 9, false),
            "games/ngc/wario world [rom 9]"
        );
        // The game's own entry is no collision.
        assert_eq!(
            unique_rel(
                &pack,
                "games/genesis/Aladdin (USA).zip",
                "https://ROMM.lan/",
                1,
                true
            ),
            "games/genesis/Aladdin (USA).zip"
        );
        // No collision at all.
        assert_eq!(
            unique_rel(&pack, "games/genesis/Sonic.zip", server, 9, true),
            "games/genesis/Sonic.zip"
        );
    }

    #[test]
    fn index_edits() {
        let mut pack = new_pack("Kit", &[PackOs::Linux, PackOs::Windows]);
        assert!(pack
            .upsert_game(game("https://user:pw@ROMM.lan/", 5, &["games/a/x.zip"]))
            .is_none());
        assert!(pack.has_game("https://romm.lan", 5));
        assert!(pack.has_game("https://romm.lan:443/", 5));
        assert!(!pack.has_game("https://other.lan", 5));
        assert!(!pack.has_game("https://romm.lan", 6));
        assert!(pack
            .upsert_game(game("https://romm.lan", 5, &["games/a/y.zip"]))
            .is_some());
        assert_eq!(pack.games.len(), 1);
        assert_eq!(pack.games[0].files[0].path, "games/a/y.zip");

        let mut emulator = draft().emulators.remove(0);
        assert!(pack.upsert_emulator(emulator.clone()).is_none());
        emulator.version = "v2".to_string();
        assert!(pack.upsert_emulator(emulator.clone()).is_some());
        emulator.os = PackOs::Windows.arch_tag().to_string();
        assert!(pack.upsert_emulator(emulator).is_none());
        assert_eq!(pack.emulators.len(), 2);

        let firmware = draft().firmware.remove(0);
        assert!(pack.upsert_firmware(firmware.clone()).is_none());
        assert!(pack.upsert_firmware(firmware).is_some());

        assert_eq!(pack.pack_bytes(), 2_097_152 + 2 * 40_000_000 + 524_288);

        let removed = pack.remove_emulators_for_os(PackOs::Windows);
        assert_eq!(removed.len(), 1);
        assert_eq!(pack.emulators.len(), 1);
        assert_eq!(pack.emulators[0].os, "linux-x86_64");

        assert!(pack.remove_game("https://ROMM.lan", 5).is_some());
        assert!(pack.remove_game("https://romm.lan", 5).is_none());
        assert!(pack.games.is_empty());
    }

    #[test]
    fn pack_game_detail_rebuilds_the_rom_detail() {
        let mut game = draft().games.remove(0);
        game.source.fs_name = "Aladdin (USA).zip".to_string();
        game.source.server_updated_at = "2026-01-02T03:04:05".to_string();
        game.source.cover_large_path = "/assets/romm/resources/roms/1/1234/cover/big.png".into();
        game.files.push(PackFile {
            path: "games/genesis/Aladdin (USA)/patch.ips".to_string(),
            size: 12,
            server_file_id: 5679,
            name: "patch.ips".to_string(),
            category: "update".to_string(),
            top_level: false,
            md5: String::new(),
            sha1: String::new(),
            extra: Default::default(),
        });
        let detail = pack_game_detail(&game);
        assert_eq!(detail.id, 1234);
        assert_eq!(detail.name, "Aladdin");
        assert_eq!(detail.platform_slug, "genesis");
        assert_eq!(detail.platform_name, "Sega Genesis");
        assert_eq!(detail.fs_name, "Aladdin (USA).zip");
        assert_eq!(detail.description, "A street rat.");
        assert_eq!(detail.regions, "USA");
        assert_eq!(detail.companies, "Virgin Games");
        assert_eq!(detail.genres, "Platform");
        assert_eq!(detail.game_modes, "Single player");
        assert_eq!(detail.player_count, "1");
        assert_eq!(detail.first_release_date, "752976000");
        assert_eq!(detail.filesize_bytes, 2_097_152);
        assert_eq!(detail.server_updated_at, "2026-01-02T03:04:05");
        assert_eq!(detail.cover_large_path, game.source.cover_large_path);
        assert!(detail.is_identified);
        assert_eq!(detail.files.len(), 2);
        let first = &detail.files[0];
        assert_eq!(
            (first.id, first.file_name.as_str(), first.file_size_bytes),
            (5678, "Aladdin (USA).zip", 2_097_152)
        );
        assert!(first.is_top_level);
        assert_eq!(first.category, "game");
        let second = &detail.files[1];
        assert_eq!((second.id, second.category.as_str()), (5679, "update"));
        assert!(!second.is_top_level);
    }

    #[test]
    fn release_dates_become_seconds() {
        assert_eq!(release_date_seconds("1993-11-11"), "752976000");
        assert_eq!(release_date_seconds("752976000"), "752976000");
        assert_eq!(release_date_seconds("soon"), "");
        assert_eq!(release_date_seconds(""), "");
    }
}
