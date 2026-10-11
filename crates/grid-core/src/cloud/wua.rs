//! Wii U title ids inside a `.wua` file (Cemu's ZArchive format).
//!
//! A `.wua` is one ZArchive file, so its name and folder hold no id. The
//! archive's top-level folders are named `<16 hex title id>_v<version>`
//! (`00050000101c9400_v0`). Only the footer and the uncompressed names
//! section are read; the multi-GB data section is never touched.
//!
//! ZArchive layout (github.com/Exzap/ZArchive): the last 144 bytes are a
//! big-endian footer of six `(offset u64, size u64)` sections — compressed
//! data, offset records, names, file tree, meta directory, meta data — then
//! a 32-byte hash, the u64 total size, the u32 version, and the u32 magic.
//! Each name is length-prefixed: one byte, or, when its high bit is set,
//! two bytes holding a 15-bit length (`(b0 & 0x7F) | (b1 << 7)`).

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

const FOOTER_SIZE: u64 = 144;
const MAGIC: u32 = 0x169f_52d6;
/// Byte offset of the names section's `(offset, size)` pair in the footer.
const NAMES_SECTION_AT: usize = 32;
const MAGIC_AT: usize = 140;
/// A real names section is about 1 MB; anything this large is not a game.
const MAX_NAMES_SIZE: u64 = 16 * 1024 * 1024;

static TITLE_FOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([0-9a-f]{16})_v\d+$").unwrap());

/// The title ids (16 hex digits, uppercase, sorted) named by the
/// `<id>_v<version>` folders inside the `.wua` at `path`. Any I/O or format
/// error yields no ids.
pub fn wua_title_ids(path: &Path) -> Vec<String> {
    match read_names_section(path) {
        Ok(names) => title_ids_in_names(&names),
        Err(err) => {
            tracing::debug!(path = %path.display(), error = %err, "wua: no title ids read");
            Vec::new()
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn be_u64(bytes: &[u8], at: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    u64::from_be_bytes(word)
}

fn read_names_section(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();
    if file_size < FOOTER_SIZE {
        return Err(invalid("file is smaller than a ZArchive footer"));
    }
    let mut footer = [0u8; FOOTER_SIZE as usize];
    file.seek(SeekFrom::Start(file_size - FOOTER_SIZE))?;
    file.read_exact(&mut footer)?;

    let mut magic = [0u8; 4];
    magic.copy_from_slice(&footer[MAGIC_AT..MAGIC_AT + 4]);
    if u32::from_be_bytes(magic) != MAGIC {
        return Err(invalid("not a ZArchive (bad magic)"));
    }

    let offset = be_u64(&footer, NAMES_SECTION_AT);
    let size = be_u64(&footer, NAMES_SECTION_AT + 8);
    if size > MAX_NAMES_SIZE {
        return Err(invalid("names section is too large"));
    }
    let end = offset
        .checked_add(size)
        .ok_or_else(|| invalid("names section overflows"))?;
    if end > file_size - FOOTER_SIZE {
        return Err(invalid("names section runs past the footer"));
    }

    let mut names = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut names)?;
    Ok(names)
}

/// Every `<16 hex>_v<n>` name in a names section, as an uppercase id. A
/// malformed tail ends the scan.
fn title_ids_in_names(names: &[u8]) -> Vec<String> {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut at = 0usize;
    while at < names.len() {
        let first = names[at] as usize;
        let (length, header) = if first & 0x80 != 0 {
            let Some(&second) = names.get(at + 1) else {
                break;
            };
            ((first & 0x7F) | ((second as usize) << 7), 2)
        } else {
            (first, 1)
        };
        let start = at + header;
        let Some(name) = names.get(start..start + length) else {
            break;
        };
        if let Some(caps) = std::str::from_utf8(name)
            .ok()
            .and_then(|text| TITLE_FOLDER_RE.captures(text))
        {
            ids.insert(caps[1].to_uppercase());
        }
        at = start + length;
    }
    ids.into_iter().collect()
}

#[cfg(test)]
/// A minimal `.wua`: a stand-in data section, the names section, then the
/// 144-byte footer. Only the names section's footer entry is filled in.
pub(crate) fn test_wua(names: &[&str]) -> Vec<u8> {
    let mut out = vec![0xAB; 32];
    let names_offset = out.len() as u64;
    for name in names {
        let length = name.len();
        if length >= 0x80 {
            out.push((length & 0x7F) as u8 | 0x80);
            out.push((length >> 7) as u8);
        } else {
            out.push(length as u8);
        }
        out.extend_from_slice(name.as_bytes());
    }
    let names_size = out.len() as u64 - names_offset;
    let mut footer = vec![0u8; FOOTER_SIZE as usize];
    footer[NAMES_SECTION_AT..NAMES_SECTION_AT + 8].copy_from_slice(&names_offset.to_be_bytes());
    footer[NAMES_SECTION_AT + 8..NAMES_SECTION_AT + 16].copy_from_slice(&names_size.to_be_bytes());
    let total = out.len() as u64 + FOOTER_SIZE;
    footer[128..136].copy_from_slice(&total.to_be_bytes());
    footer[136..140].copy_from_slice(&0x61bf_3a01u32.to_be_bytes());
    footer[MAGIC_AT..MAGIC_AT + 4].copy_from_slice(&MAGIC.to_be_bytes());
    out.extend_from_slice(&footer);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wua(dir: &Path, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join("Some Wii U Game (USA) (v208).wua");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn sample() -> Vec<u8> {
        let long_name = "x".repeat(200);
        test_wua(&[
            "",
            long_name.as_str(),
            "00050000101c9400_v0",
            "0005000E101C9400_v208",
            "content",
            "meta",
            "meta.xml",
            "0005000010145d00",
        ])
    }

    #[test]
    fn wua_title_ids_reads_the_top_level_folder_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_wua(dir.path(), &sample());
        assert_eq!(
            wua_title_ids(&path),
            vec![
                "00050000101C9400".to_string(),
                "0005000E101C9400".to_string()
            ]
        );
    }

    #[test]
    fn wua_title_ids_is_empty_for_a_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = sample();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let path = write_wua(dir.path(), &bytes);
        assert!(wua_title_ids(&path).is_empty());
    }

    #[test]
    fn wua_title_ids_is_empty_for_a_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = sample();
        let path = write_wua(dir.path(), &bytes[..bytes.len() - 1]);
        assert!(wua_title_ids(&path).is_empty());
        let path = write_wua(dir.path(), &bytes[..10]);
        assert!(wua_title_ids(&path).is_empty());
    }

    #[test]
    fn wua_title_ids_is_empty_for_an_oversize_names_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = sample();
        let size_at = bytes.len() - FOOTER_SIZE as usize + NAMES_SECTION_AT + 8;
        bytes[size_at..size_at + 8].copy_from_slice(&(MAX_NAMES_SIZE + 1).to_be_bytes());
        let path = write_wua(dir.path(), &bytes);
        assert!(wua_title_ids(&path).is_empty());
    }

    #[test]
    fn wua_title_ids_is_empty_for_a_names_section_past_the_footer() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = sample();
        let offset_at = bytes.len() - FOOTER_SIZE as usize + NAMES_SECTION_AT;
        bytes[offset_at..offset_at + 8].copy_from_slice(&u64::MAX.to_be_bytes());
        let path = write_wua(dir.path(), &bytes);
        assert!(wua_title_ids(&path).is_empty());
    }

    #[test]
    fn wua_title_ids_is_empty_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(wua_title_ids(&dir.path().join("missing.wua")).is_empty());
    }
}
