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
//! **Timestamp epoch.** The public sources disagree: the xboxdevwiki/
//! Wikipedia text says the original-Xbox FATX epoch is the year 2000, while
//! the Free60 page documents the MS-DOS year-1980 base and notes a reverse
//! engineer reading valid dates with a 1980 base. Per the task brief this
//! module uses the DOS-style 1980 epoch ([`FATX_EPOCH_YEAR`]); a later
//! `pyfatx` oracle test settles it empirically, and only this one constant
//! has to change.

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
/// Base year of the packed date field. See the module docs.
pub const FATX_EPOCH_YEAR: u32 = 1980;

/// One directory entry, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub first_cluster: u32,
    pub size: u32,
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
pub fn names_equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.eq_ignore_ascii_case(b)
}

/// True when `name` fits a FATX directory entry.
pub fn name_is_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && !name.contains(['/', '\\', '\0'])
        && name.is_ascii()
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
        let name = String::from_utf8_lossy(&slot[2..2 + len]).into_owned();
        out.push((
            index * DIR_ENTRY_SIZE,
            DirEntry {
                name,
                is_dir: slot[1] & ATTR_DIRECTORY != 0,
                first_cluster: u32::from_le_bytes([slot[44], slot[45], slot[46], slot[47]]),
                size: u32::from_le_bytes([slot[48], slot[49], slot[50], slot[51]]),
            },
        ));
    }
    out
}

/// Encode one directory slot. `timestamp` is a packed date/time from
/// [`pack_timestamp`], written to all three timestamp pairs.
///
/// The name is truncated to 42 bytes; callers reject over-long names before
/// they get here (see [`name_is_valid`]).
pub fn encode_dir_entry(entry: &DirEntry, timestamp: u32) -> [u8; DIR_ENTRY_SIZE] {
    let mut out = [0xFFu8; DIR_ENTRY_SIZE];
    let name = entry.name.as_bytes();
    let len = name.len().min(MAX_NAME_LEN);
    out[0] = len as u8;
    out[1] = if entry.is_dir {
        ATTR_DIRECTORY
    } else {
        ATTR_ARCHIVE
    };
    out[2..2 + len].copy_from_slice(&name[..len]);
    out[44..48].copy_from_slice(&entry.first_cluster.to_le_bytes());
    let size = if entry.is_dir { 0 } else { entry.size };
    out[48..52].copy_from_slice(&size.to_le_bytes());
    let date = ((timestamp >> 16) as u16).to_le_bytes();
    let time = ((timestamp & 0xFFFF) as u16).to_le_bytes();
    for pair in [52usize, 56, 60] {
        out[pair..pair + 2].copy_from_slice(&date);
        out[pair + 2..pair + 4].copy_from_slice(&time);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_and_end_markers_terminate_directory_parsing() {
        let mut cluster = vec![0xFFu8; 4 * DIR_ENTRY_SIZE];
        let alive = DirEntry {
            name: "KEEP.SAV".to_string(),
            is_dir: false,
            first_cluster: 5,
            size: 12,
        };
        let gone = DirEntry {
            name: "GONE.SAV".to_string(),
            is_dir: false,
            first_cluster: 6,
            size: 34,
        };
        let after = DirEntry {
            name: "UDATA".to_string(),
            is_dir: true,
            first_cluster: 7,
            size: 0,
        };
        cluster[0..64].copy_from_slice(&encode_dir_entry(&alive, 0));
        let mut deleted = encode_dir_entry(&gone, 0);
        deleted[0] = DELETED_ENTRY;
        cluster[64..128].copy_from_slice(&deleted);
        cluster[128..192].copy_from_slice(&encode_dir_entry(&after, 0));
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
        cluster2[128..192].copy_from_slice(&encode_dir_entry(&alive, 0));
        assert!(parse_dir_cluster(&cluster2).is_empty());
    }

    #[test]
    fn names_compare_case_insensitively_on_lookup() {
        assert!(names_equal("UDATA", "udata"));
        assert!(names_equal("Save.Bin", "SAVE.BIN"));
        assert!(!names_equal("UDATA", "TDATA"));
        assert!(!names_equal("UDATA", "UDATA2"));

        // Case is preserved on the wire.
        let e = DirEntry {
            name: "MixedCase.Sav".to_string(),
            is_dir: false,
            first_cluster: 2,
            size: 1,
        };
        let raw = encode_dir_entry(&e, 0);
        assert_eq!(&raw[2..15], b"MixedCase.Sav");
        assert_eq!(parse_dir_cluster(&raw)[0].1.name, "MixedCase.Sav");
    }
}
