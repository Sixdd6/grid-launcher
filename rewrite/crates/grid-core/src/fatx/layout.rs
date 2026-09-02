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
///
/// Two cluster counts, and they mean different things:
///
/// - `cluster_count` sizes the FAT. It is `partition_size / cluster_size`,
///   the documented rule, and it is what selects FAT16X vs FAT32X.
/// - `usable_clusters` is how many clusters actually have bytes behind
///   them once the superblock and the FAT are subtracted. It is always
///   smaller, and it is the bound for cluster addressing and allocation.
///
/// The FAT therefore has a few trailing entries that address nothing; a
/// real formatter leaves them free and so do we. Bounds checks use
/// `usable_clusters`, never `cluster_count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub cluster_size: u64,
    pub cluster_count: u64,
    pub usable_clusters: u64,
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
    ///
    /// Bounded by `usable_clusters`: the last few FAT entries address
    /// bytes the partition does not have, and asking for them is
    /// [`FatxError::BadCluster`], never a read past the end.
    pub fn cluster_offset(&self, cluster: u32) -> Result<u64, FatxError> {
        if cluster < 1 || u64::from(cluster) > self.usable_clusters {
            return Err(FatxError::BadCluster(cluster));
        }
        Ok(self.data_offset + (u64::from(cluster) - 1) * self.cluster_size)
    }

    /// Bytes of the partition the data area occupies.
    pub fn data_size(&self) -> u64 {
        self.usable_clusters * self.cluster_size
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

/// Size of the FAT for `cluster_count` clusters, and whether it is FAT32X.
///
/// One entry per cluster **plus the reserved entry 0**, padded up to a
/// 0x1000 boundary so the data area stays page aligned. See the
/// "oracle checklist" in [`super::dir`]: the `+ 1` is the brief's rule and
/// differs from a plain `cluster_count` entries only when
/// `cluster_count * width` is already an exact multiple of 0x1000, in which
/// case it costs one more page.
fn fat_size_for(cluster_count: u64) -> (bool, u64) {
    let fat32 = cluster_count >= FAT16X_CLUSTER_LIMIT;
    let entry = if fat32 { 4 } else { 2 };
    let raw = (cluster_count + 1) * entry;
    (
        fat32,
        raw.div_ceil(FATX_SUPERBLOCK_SIZE) * FATX_SUPERBLOCK_SIZE,
    )
}

/// Derive the layout of a partition of `partition_size` bytes described by
/// `sb`.
///
/// Straight from the documented rule, in one pass and no iteration: the
/// cluster count is the partition size divided by the cluster size, the FAT
/// width follows from that count, and the data area is whatever is left
/// after the superblock and the FAT.
pub fn geometry(partition_size: u64, sb: &Superblock) -> Result<Geometry, FatxError> {
    let cluster_size = u64::from(sb.sectors_per_cluster) * SECTOR_SIZE;
    let fat_offset = FATX_SUPERBLOCK_SIZE;
    if partition_size <= fat_offset + cluster_size {
        return Err(FatxError::PartitionTooSmall);
    }
    let cluster_count = partition_size / cluster_size;
    let (fat32, fat_size) = fat_size_for(cluster_count);
    let data_offset = fat_offset + fat_size;
    if partition_size <= data_offset {
        return Err(FatxError::PartitionTooSmall);
    }
    let usable_clusters = (partition_size - data_offset) / cluster_size;
    if usable_clusters == 0 {
        return Err(FatxError::PartitionTooSmall);
    }
    Ok(Geometry {
        cluster_size,
        cluster_count,
        usable_clusters,
        fat_offset,
        fat_size,
        data_offset,
        fat32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn sb_bytes(sig: &[u8; 4], spc: u32) -> Vec<u8> {
        let mut b = vec![0u8; FATX_SUPERBLOCK_SIZE as usize];
        b[0..4].copy_from_slice(sig);
        b[4..8].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        b[8..12].copy_from_slice(&spc.to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        b
    }

    fn sb(spc: u32) -> Superblock {
        Superblock {
            volume_id: 1,
            sectors_per_cluster: spc,
            root_dir_first_cluster: 1,
        }
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
        let cluster = 32 * SECTOR_SIZE; // 16 KiB

        // The cluster count is the partition size divided by the cluster
        // size, so these two sizes sit either side of the 0xFFF0 threshold
        // by construction.
        let g = geometry((FAT16X_CLUSTER_LIMIT - 1) * cluster, &sb(32)).expect("geometry");
        assert_eq!(g.cluster_count, FAT16X_CLUSTER_LIMIT - 1);
        assert!(!g.fat32);
        assert_eq!(g.fat_entry_size(), 2);
        assert_eq!(g.end_of_chain_min(), FAT16X_END_MIN);

        let g = geometry(FAT16X_CLUSTER_LIMIT * cluster, &sb(32)).expect("geometry");
        assert_eq!(g.cluster_count, FAT16X_CLUSTER_LIMIT);
        assert!(g.fat32);
        assert_eq!(g.fat_entry_size(), 4);
        assert_eq!(g.end_of_chain_min(), FAT32X_END_MIN);

        // 512-byte clusters cross the threshold in a 33 MiB partition, which
        // is what the cheap FAT32X test fixture uses.
        let g = geometry(33 * 1024 * 1024, &sb(1)).expect("geometry");
        assert!(g.fat32, "cluster_count {}", g.cluster_count);
    }

    #[test]
    fn geometry_pins_known_layouts() {
        // 1 GiB at 16 KiB clusters: 65536 clusters, over the threshold.
        //
        // fat_size is (65536 + 1) entries x 4 bytes = 262148, rounded up to
        // 0x41000, so data_offset is 0x42000. Under a FAT of exactly
        // cluster_count entries it would be 0x41000 instead: 1 GiB is one of
        // the sizes where `cluster_count * width` is already page aligned and
        // the reserved entry 0 costs a whole extra page. That is oracle
        // checklist item 2 in `super::dir` — if pyfatx says otherwise, the
        // `+ 1` in `fat_size_for` is the single line to change.
        let g = geometry(GIB, &sb(32)).expect("geometry");
        assert!(g.fat32);
        assert_eq!(g.cluster_count, 65_536);
        assert_eq!(g.fat_size, 0x41000);
        assert_eq!(g.data_offset, 0x42000);
        assert_eq!(g.usable_clusters, 65_519);

        // The retail E: partition. Here the two rules agree: 313280 x 4 is
        // not page aligned, so the reserved entry rounds into the same page.
        let g = geometry(RETAIL_PARTITION_E_SIZE, &sb(32)).expect("geometry");
        assert!(g.fat32);
        assert_eq!(g.cluster_count, 313_280);
        assert_eq!(g.fat_size, 0x132000);
        assert_eq!(g.data_offset, 0x133000);
        assert_eq!(g.usable_clusters, 313_203);
    }

    #[test]
    fn geometry_rounds_fat_size_to_a_page_boundary() {
        for (size, spc) in [
            (8u64 * 1024 * 1024, 8u32),
            (33 * 1024 * 1024, 1),
            (64 * 1024 * 1024, 32),
            (GIB, 32),
            (RETAIL_PARTITION_E_SIZE, 32),
        ] {
            let g = geometry(size, &sb(spc)).expect("geometry");
            assert_eq!(g.cluster_count, size / g.cluster_size);
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
            // The data area fits, and it is what usable_clusters counts.
            assert!(g.data_offset + g.data_size() <= size);
            assert_eq!(g.usable_clusters, (size - g.data_offset) / g.cluster_size);
            assert!(g.usable_clusters < g.cluster_count);
            // Cluster numbering starts at 1, and stops at usable_clusters.
            assert_eq!(g.cluster_offset(1).unwrap(), g.data_offset);
            assert_eq!(g.cluster_offset(2).unwrap(), g.data_offset + g.cluster_size);
            assert!(g.cluster_offset(0).is_err());
            let last = g.usable_clusters as u32;
            assert!(g.cluster_offset(last).is_ok());
            assert!(g.cluster_offset(last + 1).is_err());
        }
        // A partition that cannot hold a single cluster is rejected.
        assert!(matches!(
            geometry(0x2000, &sb(32)),
            Err(FatxError::PartitionTooSmall)
        ));
        // Nor one where the superblock plus the FAT leave no room for even
        // one whole cluster (8500 bytes, 512-byte clusters: data starts at
        // 0x2000 and only 308 bytes follow it).
        assert!(matches!(
            geometry(8500, &sb(1)),
            Err(FatxError::PartitionTooSmall)
        ));
    }
}
