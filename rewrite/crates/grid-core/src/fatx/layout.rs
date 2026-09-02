//! FATX partition geometry: the retail partition table offsets, the
//! `FATX` superblock, and the derived FAT/data-area layout.
//!
//! Original-Xbox variant only: every multi-byte field is LITTLE-ENDIAN.
//! (The Xbox 360 `XTAF` variant is big-endian and out of scope.)
//!
//! Clean-room: written from the public format descriptions on
//! <https://xboxdevwiki.net/FATX>, <https://xboxdevwiki.net/Hard_Drive> and
//! <https://free60.org/System-Software/Systems/FATX/>. No FATX
//! implementation source was consulted.

use super::FatxError;

/// Byte offset of the retail `E:` (data) partition in a standard Xbox HDD
/// image. From the xboxdevwiki hard-drive partition table.
pub const RETAIL_PARTITION_E_OFFSET: u64 = 0xABE8_0000;

/// Size of the retail `E:` partition in the standard (8 GB) retail layout.
pub const RETAIL_PARTITION_E_SIZE: u64 = 0x1_31F0_0000;

/// The superblock occupies the first 0x1000 bytes of a FATX partition; the
/// FAT starts immediately after it.
pub const FATX_SUPERBLOCK_SIZE: u64 = 0x1000;

/// Magic at offset 0 of the superblock.
pub const FATX_SIGNATURE: [u8; 4] = *b"FATX";

/// FATX expresses cluster size as a count of 512-byte sectors.
pub const SECTOR_SIZE: u64 = 512;

/// Below this many clusters the FAT uses 2-byte entries (FAT16X); at or
/// above it, 4-byte entries (FAT32X).
pub const FAT16X_CLUSTER_LIMIT: u64 = 0xFFF0;

/// Smallest FAT entry value that terminates a cluster chain.
pub const FAT16X_END_MIN: u32 = 0xFFF8;
/// Smallest FAT entry value that terminates a cluster chain (FAT32X).
pub const FAT32X_END_MIN: u32 = 0xFFFF_FFF8;

/// Value written into the terminating entry of a chain.
pub const FAT16X_END: u32 = 0xFFFF;
/// Value written into the terminating entry of a chain (FAT32X).
pub const FAT32X_END: u32 = 0xFFFF_FFFF;

/// Parsed FATX superblock.
///
/// Layout (all little-endian):
/// `0x00` magic `"FATX"`, `0x04` volume id u32, `0x08` sectors per cluster
/// u32, `0x0C` first cluster of the root directory u32, `0x10` 64-byte
/// UTF-16LE volume name. The rest of the 0x1000-byte block is padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Superblock {
    pub volume_id: u32,
    pub sectors_per_cluster: u32,
    pub root_dir_first_cluster: u32,
}

fn le_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

impl Superblock {
    /// Parse a superblock from the first bytes of a partition. Needs at
    /// least 16 bytes; anything shorter is a truncated image.
    pub fn parse(bytes: &[u8]) -> Result<Self, FatxError> {
        if bytes.len() < 16 {
            return Err(FatxError::Truncated {
                needed: 16,
                actual: bytes.len() as u64,
            });
        }
        if bytes[0..4] != FATX_SIGNATURE {
            return Err(FatxError::BadSignature);
        }
        let sectors_per_cluster = le_u32(bytes, 8);
        // 512 B .. 64 KiB clusters, always a power of two.
        if sectors_per_cluster == 0
            || sectors_per_cluster > 128
            || !sectors_per_cluster.is_power_of_two()
        {
            return Err(FatxError::BadClusterSize(sectors_per_cluster));
        }
        Ok(Self {
            volume_id: le_u32(bytes, 4),
            sectors_per_cluster,
            root_dir_first_cluster: le_u32(bytes, 12),
        })
    }

    /// Serialize a full 0x1000-byte superblock block. Test-support for the
    /// image builder and the write path.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![0xFFu8; FATX_SUPERBLOCK_SIZE as usize];
        out[0..4].copy_from_slice(&FATX_SIGNATURE);
        out[4..8].copy_from_slice(&self.volume_id.to_le_bytes());
        out[8..12].copy_from_slice(&self.sectors_per_cluster.to_le_bytes());
        out[12..16].copy_from_slice(&self.root_dir_first_cluster.to_le_bytes());
        // Empty volume name.
        out[16..80].fill(0);
        out
    }
}

/// Derived on-disk layout of a FATX partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub cluster_size: u64,
    pub cluster_count: u64,
    pub fat_offset: u64,
    pub fat_size: u64,
    pub data_offset: u64,
    pub fat32: bool,
}

impl Geometry {
    /// Bytes per FAT entry: 2 for FAT16X, 4 for FAT32X.
    pub fn fat_entry_size(&self) -> u64 {
        if self.fat32 {
            4
        } else {
            2
        }
    }

    /// Byte offset of a data cluster inside the partition. Clusters are
    /// numbered from 1 (entry 0 of the FAT is reserved), so cluster N lives
    /// at `data_offset + (N - 1) * cluster_size`.
    pub fn cluster_offset(&self, cluster: u32) -> Result<u64, FatxError> {
        if cluster < 1 || u64::from(cluster) > self.cluster_count {
            return Err(FatxError::BadCluster(cluster));
        }
        Ok(self.data_offset + (u64::from(cluster) - 1) * self.cluster_size)
    }

    /// Smallest FAT value that ends a chain, for this FAT width.
    pub fn end_of_chain_min(&self) -> u32 {
        if self.fat32 {
            FAT32X_END_MIN
        } else {
            FAT16X_END_MIN
        }
    }

    /// Value to write when terminating a chain.
    pub fn end_of_chain(&self) -> u32 {
        if self.fat32 {
            FAT32X_END
        } else {
            FAT16X_END
        }
    }
}

fn fat_size_for(cluster_count: u64) -> (bool, u64) {
    let fat32 = cluster_count >= FAT16X_CLUSTER_LIMIT;
    let entry = if fat32 { 4 } else { 2 };
    // One entry per cluster plus the reserved entry 0, padded out to a
    // 0x1000 boundary so the data area stays page aligned.
    let raw = (cluster_count + 1) * entry;
    (
        fat32,
        raw.div_ceil(FATX_SUPERBLOCK_SIZE) * FATX_SUPERBLOCK_SIZE,
    )
}

/// Derive the layout of a partition of `partition_size` bytes described by
/// `sb`.
///
/// The FAT size depends on the cluster count and the cluster count depends
/// on the FAT size, so this iterates to a fixed point (a handful of steps;
/// the loop is capped and always yields a layout that fits).
pub fn geometry(partition_size: u64, sb: &Superblock) -> Result<Geometry, FatxError> {
    let cluster_size = u64::from(sb.sectors_per_cluster) * SECTOR_SIZE;
    if partition_size <= FATX_SUPERBLOCK_SIZE + cluster_size {
        return Err(FatxError::PartitionTooSmall);
    }
    let fat_offset = FATX_SUPERBLOCK_SIZE;
    let mut cluster_count = (partition_size - fat_offset) / cluster_size;
    for _ in 0..64 {
        let (_, fat_size) = fat_size_for(cluster_count);
        let available = partition_size.saturating_sub(fat_offset + fat_size) / cluster_size;
        if available == cluster_count || available == 0 {
            break;
        }
        cluster_count = available;
    }
    // Shrink until the chosen FAT plus the data area genuinely fit.
    loop {
        if cluster_count == 0 {
            return Err(FatxError::PartitionTooSmall);
        }
        let (fat32, fat_size) = fat_size_for(cluster_count);
        let data_offset = fat_offset + fat_size;
        if data_offset + cluster_count * cluster_size <= partition_size {
            return Ok(Geometry {
                cluster_size,
                cluster_count,
                fat_offset,
                fat_size,
                data_offset,
                fat32,
            });
        }
        cluster_count -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sb_bytes(sig: &[u8; 4], spc: u32) -> Vec<u8> {
        let mut b = vec![0u8; FATX_SUPERBLOCK_SIZE as usize];
        b[0..4].copy_from_slice(sig);
        b[4..8].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        b[8..12].copy_from_slice(&spc.to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        b
    }

    #[test]
    fn superblock_rejects_bad_signature_and_bad_cluster_size() {
        let good = Superblock::parse(&sb_bytes(b"FATX", 32)).expect("valid superblock");
        assert_eq!(good.volume_id, 0x1234_5678);
        assert_eq!(good.sectors_per_cluster, 32);
        assert_eq!(good.root_dir_first_cluster, 1);

        assert!(matches!(
            Superblock::parse(&sb_bytes(b"XTAF", 32)),
            Err(FatxError::BadSignature)
        ));
        // Not a power of two.
        assert!(matches!(
            Superblock::parse(&sb_bytes(b"FATX", 24)),
            Err(FatxError::BadClusterSize(24))
        ));
        // Zero and out of the 1..=128 sector range.
        assert!(matches!(
            Superblock::parse(&sb_bytes(b"FATX", 0)),
            Err(FatxError::BadClusterSize(0))
        ));
        assert!(matches!(
            Superblock::parse(&sb_bytes(b"FATX", 256)),
            Err(FatxError::BadClusterSize(256))
        ));
        // Too short to hold the fields.
        assert!(Superblock::parse(&[0u8; 8]).is_err());
    }

    #[test]
    fn geometry_selects_fat16x_below_the_threshold_and_fat32x_above() {
        let sb = Superblock {
            volume_id: 1,
            sectors_per_cluster: 32, // 16 KiB clusters
            root_dir_first_cluster: 1,
        };
        let cluster = 32 * SECTOR_SIZE;

        // Sized so that just under 0xFFF0 clusters fit.
        let small = FATX_SUPERBLOCK_SIZE + 0x2_0000 + (FAT16X_CLUSTER_LIMIT - 1) * cluster;
        let g = geometry(small, &sb).expect("geometry");
        assert!(
            g.cluster_count < FAT16X_CLUSTER_LIMIT,
            "{}",
            g.cluster_count
        );
        assert!(!g.fat32);
        assert_eq!(g.fat_entry_size(), 2);

        // Sized so that at least 0xFFF0 clusters fit.
        let big = FATX_SUPERBLOCK_SIZE + 0x4_0000 + FAT16X_CLUSTER_LIMIT * cluster;
        let g = geometry(big, &sb).expect("geometry");
        assert!(
            g.cluster_count >= FAT16X_CLUSTER_LIMIT,
            "{}",
            g.cluster_count
        );
        assert!(g.fat32);
        assert_eq!(g.fat_entry_size(), 4);

        // The retail E: partition is a FAT32X volume with 16 KiB clusters.
        let g = geometry(RETAIL_PARTITION_E_SIZE, &sb).expect("geometry");
        assert!(g.fat32);
        assert!(g.data_offset + g.cluster_count * g.cluster_size <= RETAIL_PARTITION_E_SIZE);
    }

    #[test]
    fn geometry_rounds_fat_size_to_a_page_boundary() {
        let sb = Superblock {
            volume_id: 1,
            sectors_per_cluster: 32,
            root_dir_first_cluster: 1,
        };
        for size in [
            8u64 * 1024 * 1024,
            64 * 1024 * 1024,
            1024 * 1024 * 1024,
            RETAIL_PARTITION_E_SIZE,
        ] {
            let g = geometry(size, &sb).expect("geometry");
            assert_eq!(g.fat_offset, FATX_SUPERBLOCK_SIZE);
            assert_eq!(
                g.fat_size % FATX_SUPERBLOCK_SIZE,
                0,
                "fat_size {}",
                g.fat_size
            );
            let raw = (g.cluster_count + 1) * g.fat_entry_size();
            assert_eq!(
                g.fat_size,
                raw.div_ceil(FATX_SUPERBLOCK_SIZE) * FATX_SUPERBLOCK_SIZE
            );
            assert_eq!(g.data_offset, g.fat_offset + g.fat_size);
            assert!(g.data_offset + g.cluster_count * g.cluster_size <= size);
            // Cluster numbering starts at 1.
            assert_eq!(g.cluster_offset(1).unwrap(), g.data_offset);
            assert_eq!(g.cluster_offset(2).unwrap(), g.data_offset + g.cluster_size);
            assert!(g.cluster_offset(0).is_err());
        }
        // A partition that cannot hold a single cluster is rejected.
        assert!(matches!(
            geometry(0x2000, &sb),
            Err(FatxError::PartitionTooSmall)
        ));
    }
}
