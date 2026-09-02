//! FATX directory entries: fixed 64-byte slots packed into the clusters of
//! a directory's cluster chain.
//!
//! Layout (little-endian, from the public format descriptions):
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0x00   | 1    | name length; `0xFF` end of directory, `0xE5` deleted |
//! | 0x01   | 1    | attributes (`0x10` = directory) |
//! | 0x02   | 42   | name, padded with `0xFF` |
//! | 0x2C   | 4    | first cluster (u32) |
//! | 0x30   | 4    | file size in bytes (u32) |
//! | 0x34   | 2+2  | creation date, creation time |
//! | 0x38   | 2+2  | last-write date, last-write time |
//! | 0x3C   | 2+2  | last-access date, last-access time |
//!
//! Date bits: 15..9 year, 8..5 month, 4..0 day. Time bits: 15..11 hour,
//! 10..5 minute, 4..0 second in two-second units.
//!
//! Names are stored as raw bytes, not `String`. The console writes OEM
//! bytes, and the write path has to put a name back exactly as it found it,
//! so decoding to UTF-8 happens only for display ([`DirEntry::display_name`]).
//!
//! # Oracle checklist
//!
//! Three details the public documentation does not settle. Each is isolated
//! to one place so a `pyfatx` oracle run on a generated image can decide it
//! empirically, and each is a one-line change.
//!
//! The oracle lives in `tests/fatx_oracle.rs`. It self-skips when `python3
//! -c "import fatx"` fails, which is the state it was left in: pyfatx
//! could not be installed on the development machine (its build needs
//! CMake), so **none of the three has been settled empirically yet**. The
//! values below remain the reasoned defaults from the format docs.
//!
//!
//! 1. **Timestamp epoch.** xboxdevwiki and Wikipedia say the original-Xbox
//!    FATX epoch is the year 2000; the Free60 page documents the MS-DOS
//!    year-1980 base, and notes a reverse engineer reading valid dates with
//!    1980. This module uses 1980 — [`FATX_EPOCH_YEAR`], the one constant to
//!    change. No read behavior depends on it.
//! 2. **The reserved FAT entry.** The FAT is sized `cluster_count + 1`
//!    entries (entry 0 reserved, clusters numbered from 1), where Free60
//!    describes it as `cluster_count` entries. The two agree after the
//!    rounding to 0x1000 except when `cluster_count * width` is already an
//!    exact page multiple, where the `+ 1` costs one more page and moves
//!    `data_offset`. See `fat_size_for` in [`super::layout`].
//! 3. **Timestamp field order.** This module writes date then time at 0x34,
//!    0x38 and 0x3C, per Free60's table. Nothing verifies the order against
//!    a console-written image yet; see [`encode_dir_entry`].

use std::borrow::Cow;

/// Every directory slot is 64 bytes.
pub const DIR_ENTRY_SIZE: usize = 64;
/// Names are at most 42 bytes.
pub const MAX_NAME_LEN: usize = 42;
/// Name-length byte value that ends the directory.
pub const END_OF_DIRECTORY: u8 = 0xFF;
/// Name-length byte value marking a deleted slot.
pub const DELETED_ENTRY: u8 = 0xE5;
/// Attribute bit for a directory.
pub const ATTR_DIRECTORY: u8 = 0x10;
/// Attribute bit for a normal file with no special flags.
pub const ATTR_ARCHIVE: u8 = 0x20;
/// Base year of the packed date field. See the oracle checklist above.
pub const FATX_EPOCH_YEAR: u32 = 1980;

use super::FatxError;

/// One directory entry, decoded. `name` holds the raw on-disk bytes so the
/// write path can round-trip a name it did not create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: Vec<u8>,
    pub is_dir: bool,
    pub first_cluster: u32,
    pub size: u32,
}

impl DirEntry {
    /// Build an entry from a Rust string. The name is not validated here;
    /// [`encode_dir_entry`] rejects an unusable one.
    pub fn new(name: &str, is_dir: bool, first_cluster: u32, size: u32) -> Self {
        Self {
            name: name.as_bytes().to_vec(),
            is_dir,
            first_cluster,
            size,
        }
    }

    /// Build an entry from raw on-disk name bytes.
    pub fn new_bytes(name: &[u8], is_dir: bool, first_cluster: u32, size: u32) -> Self {
        Self {
            name: name.to_vec(),
            is_dir,
            first_cluster,
            size,
        }
    }

    /// The name for display and for building a host path. Invalid UTF-8 is
    /// replaced, so this is never the value to write back to the image.
    pub fn display_name(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.name)
    }
}

/// Pack a wall-clock time into the `u32` that [`encode_dir_entry`] writes:
/// the date in the high 16 bits, the time in the low 16.
pub fn pack_timestamp(year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u32 {
    let years = year.saturating_sub(FATX_EPOCH_YEAR) & 0x7F;
    let date = (years << 9) | ((month & 0x0F) << 5) | (day & 0x1F);
    let time = ((hour & 0x1F) << 11) | ((minute & 0x3F) << 5) | ((second / 2) & 0x1F);
    (date << 16) | time
}

/// FATX names are compared without regard to case (ASCII case folding, the
/// character set the console writes) but stored with their case intact.
pub fn names_equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.eq_ignore_ascii_case(b)
}

/// True when `name` fits a FATX directory entry and is safe to use as a
/// single host path component.
///
/// Rejects the path separators of both host families, the drive/stream
/// separator `:`, NUL, `.` and `..`, anything non-ASCII, and anything over
/// 42 bytes.
pub fn name_is_valid(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name.is_ascii()
        && !name.iter().any(|b| matches!(b, b'/' | b'\\' | b':' | 0))
        && name != b"."
        && name != b".."
}

/// Decode every live entry of one directory cluster.
///
/// Returns `(slot offset within the cluster, entry)` pairs. Deleted slots
/// are skipped; parsing stops at the first end-of-directory marker (a name
/// length of `0xFF`, or `0x00` for a zero-filled slot), so bytes after it
/// are never interpreted.
pub fn parse_dir_cluster(bytes: &[u8]) -> Vec<(usize, DirEntry)> {
    let mut out = Vec::new();
    for (index, slot) in bytes.chunks_exact(DIR_ENTRY_SIZE).enumerate() {
        let name_len = slot[0];
        if name_len == END_OF_DIRECTORY || name_len == 0 {
            break;
        }
        if name_len == DELETED_ENTRY {
            continue;
        }
        let len = name_len as usize;
        if len > MAX_NAME_LEN {
            continue; // corrupt slot: skip it rather than trust the length
        }
        out.push((
            index * DIR_ENTRY_SIZE,
            DirEntry {
                name: slot[2..2 + len].to_vec(),
                is_dir: slot[1] & ATTR_DIRECTORY != 0,
                first_cluster: u32::from_le_bytes([slot[44], slot[45], slot[46], slot[47]]),
                size: u32::from_le_bytes([slot[48], slot[49], slot[50], slot[51]]),
            },
        ));
    }
    out
}

/// Encode one directory slot. `timestamp` is a packed date/time from
/// [`pack_timestamp`], written to all three timestamp pairs as date then
/// time (oracle checklist item 3).
///
/// An unusable name is [`FatxError::InvalidName`] — never silently
/// truncated, because a truncated name would name a different file.
pub fn encode_dir_entry(
    entry: &DirEntry,
    timestamp: u32,
) -> Result<[u8; DIR_ENTRY_SIZE], FatxError> {
    if !name_is_valid(&entry.name) {
        return Err(FatxError::InvalidName(entry.display_name().into_owned()));
    }
    let mut out = [0xFFu8; DIR_ENTRY_SIZE];
    let name = &entry.name;
    out[0] = name.len() as u8;
    out[1] = if entry.is_dir {
        ATTR_DIRECTORY
    } else {
        ATTR_ARCHIVE
    };
    out[2..2 + name.len()].copy_from_slice(name);
    out[44..48].copy_from_slice(&entry.first_cluster.to_le_bytes());
    let size = if entry.is_dir { 0 } else { entry.size };
    out[48..52].copy_from_slice(&size.to_le_bytes());
    let date = ((timestamp >> 16) as u16).to_le_bytes();
    let time = ((timestamp & 0xFFFF) as u16).to_le_bytes();
    for pair in [52usize, 56, 60] {
        out[pair..pair + 2].copy_from_slice(&date);
        out[pair + 2..pair + 4].copy_from_slice(&time);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_and_end_markers_terminate_directory_parsing() {
        let mut cluster = vec![0xFFu8; 4 * DIR_ENTRY_SIZE];
        let alive = DirEntry::new("KEEP.SAV", false, 5, 12);
        let gone = DirEntry::new("GONE.SAV", false, 6, 34);
        let after = DirEntry::new("UDATA", true, 7, 0);
        cluster[0..64].copy_from_slice(&encode_dir_entry(&alive, 0).unwrap());
        let mut deleted = encode_dir_entry(&gone, 0).unwrap();
        deleted[0] = DELETED_ENTRY;
        cluster[64..128].copy_from_slice(&deleted);
        cluster[128..192].copy_from_slice(&encode_dir_entry(&after, 0).unwrap());
        // Slot 3 keeps the 0xFF end-of-directory marker.

        let parsed = parse_dir_cluster(&cluster);
        assert_eq!(parsed.len(), 2, "deleted entry must be skipped");
        assert_eq!(parsed[0].0, 0);
        assert_eq!(parsed[0].1, alive);
        assert_eq!(parsed[1].0, 128);
        assert_eq!(parsed[1].1, after);
        assert!(parsed[1].1.is_dir);

        // Anything after the end-of-directory marker is ignored.
        let mut cluster2 = vec![0xFFu8; 3 * DIR_ENTRY_SIZE];
        cluster2[128..192].copy_from_slice(&encode_dir_entry(&alive, 0).unwrap());
        assert!(parse_dir_cluster(&cluster2).is_empty());

        // So is a zero-filled slot.
        let mut cluster3 = vec![0u8; 2 * DIR_ENTRY_SIZE];
        cluster3[64..128].copy_from_slice(&encode_dir_entry(&alive, 0).unwrap());
        assert!(parse_dir_cluster(&cluster3).is_empty());
    }

    #[test]
    fn names_compare_case_insensitively_on_lookup() {
        assert!(names_equal(b"UDATA", b"udata"));
        assert!(names_equal(b"Save.Bin", b"SAVE.BIN"));
        assert!(!names_equal(b"UDATA", b"TDATA"));
        assert!(!names_equal(b"UDATA", b"UDATA2"));

        // Case is preserved on the wire, and round-trips byte for byte.
        let e = DirEntry::new("MixedCase.Sav", false, 2, 1);
        let raw = encode_dir_entry(&e, 0).unwrap();
        assert_eq!(&raw[2..15], b"MixedCase.Sav");
        let back = parse_dir_cluster(&raw);
        assert_eq!(back[0].1.name, b"MixedCase.Sav".to_vec());
        assert_eq!(back[0].1.display_name(), "MixedCase.Sav");
    }

    #[test]
    fn encode_rejects_unusable_names_instead_of_truncating() {
        let long = "X".repeat(MAX_NAME_LEN + 1);
        assert!(matches!(
            encode_dir_entry(&DirEntry::new(&long, false, 2, 0), 0),
            Err(FatxError::InvalidName(_))
        ));
        // Exactly 42 bytes is fine.
        let ok = "X".repeat(MAX_NAME_LEN);
        assert!(encode_dir_entry(&DirEntry::new(&ok, false, 2, 0), 0).is_ok());

        for bad in ["", "a/b", "a\\b", "C:stream", ".", "..", "caf\u{e9}"] {
            assert!(
                matches!(
                    encode_dir_entry(&DirEntry::new(bad, false, 2, 0), 0),
                    Err(FatxError::InvalidName(_))
                ),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn timestamps_pack_date_into_the_high_half_and_time_into_the_low() {
        // 2024-01-02 03:04:06, DOS-style 1980 base: year offset 44.
        let stamp = pack_timestamp(2024, 1, 2, 3, 4, 6);
        let date = (stamp >> 16) as u16;
        let time = (stamp & 0xFFFF) as u16;
        assert_eq!(date >> 9, 44);
        assert_eq!((date >> 5) & 0x0F, 1);
        assert_eq!(date & 0x1F, 2);
        assert_eq!(time >> 11, 3);
        assert_eq!((time >> 5) & 0x3F, 4);
        assert_eq!(time & 0x1F, 3); // two-second units

        // Date first, then time, at each of the three pairs.
        let raw = encode_dir_entry(&DirEntry::new("A", false, 1, 0), stamp).unwrap();
        for at in [52usize, 56, 60] {
            assert_eq!(u16::from_le_bytes([raw[at], raw[at + 1]]), date);
            assert_eq!(u16::from_le_bytes([raw[at + 2], raw[at + 3]]), time);
        }
    }
}
