//! The FAT ("chainmap"): one entry per cluster plus a reserved entry 0.
//!
//! Entries are 2 bytes (FAT16X) or 4 bytes (FAT32X), little-endian, chosen
//! by cluster count in [`super::layout::geometry`]. Entry values follow the
//! FAT convention: 0 is free, a value at or above the end-of-chain minimum
//! (0xFFF8 / 0xFFFF_FFF8) terminates a chain, anything else is the next
//! cluster number.

use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};

use super::layout::Geometry;
use super::FatxError;

/// The whole FAT held in memory as 32-bit values, regardless of the on-disk
/// entry width. A retail `E:` FAT is ~1.2 MB at 16 KiB clusters, so this is
/// cheap and keeps chain walking allocation-free.
#[derive(Debug, Clone)]
pub struct Fat {
    entries: Vec<u32>,
    fat32: bool,
    /// Highest cluster number that has bytes behind it. The FAT is sized
    /// from `cluster_count`, which runs a little past the end of the data
    /// area, so allocation and chain walking stop here instead.
    usable: u32,
    end_min: u32,
    end_value: u32,
}

impl Fat {
    /// A freshly formatted FAT: every cluster free, entry 0 reserved.
    pub fn format(geo: &Geometry) -> Self {
        let mut entries = vec![0u32; geo.cluster_count as usize + 1];
        entries[0] = geo.end_of_chain();
        Self {
            entries,
            fat32: geo.fat32,
            usable: geo.usable_clusters.min(u64::from(u32::MAX)) as u32,
            end_min: geo.end_of_chain_min(),
            end_value: geo.end_of_chain(),
        }
    }

    /// Read the FAT from `io`, whose partition starts at byte `base`.
    pub fn read(io: &mut (impl Read + Seek), geo: &Geometry, base: u64) -> Result<Self, FatxError> {
        let count = geo.cluster_count as usize + 1;
        let width = geo.fat_entry_size() as usize;
        let mut raw = vec![0u8; count * width];
        io.seek(SeekFrom::Start(base + geo.fat_offset))?;
        io.read_exact(&mut raw)?;
        let entries = if geo.fat32 {
            raw.chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        } else {
            raw.chunks_exact(2)
                .map(|c| u32::from(u16::from_le_bytes([c[0], c[1]])))
                .collect()
        };
        Ok(Self {
            entries,
            fat32: geo.fat32,
            usable: geo.usable_clusters.min(u64::from(u32::MAX)) as u32,
            end_min: geo.end_of_chain_min(),
            end_value: geo.end_of_chain(),
        })
    }

    /// Number of FAT entries, including the reserved entry 0.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Highest addressable cluster number.
    pub fn usable_clusters(&self) -> u32 {
        self.usable
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raw value of one FAT entry, or `None` when out of range.
    pub fn entry(&self, cluster: u32) -> Option<u32> {
        self.entries.get(cluster as usize).copied()
    }

    /// Overwrite one FAT entry. Entry 0 is reserved and cannot be set.
    pub fn set_entry(&mut self, cluster: u32, value: u32) -> Result<(), FatxError> {
        if cluster == 0 || cluster as usize >= self.entries.len() {
            return Err(FatxError::BadCluster(cluster));
        }
        self.entries[cluster as usize] = value;
        Ok(())
    }

    fn is_end(&self, value: u32) -> bool {
        value >= self.end_min
    }

    /// Walk the cluster chain that starts at `first`.
    ///
    /// A repeated cluster, a free entry inside the chain, or a next-cluster
    /// value past the last usable cluster is [`FatxError::CorruptChain`];
    /// the visited set means a cyclic FAT errors instead of looping
    /// forever.
    pub fn chain(&self, first: u32) -> Result<Vec<u32>, FatxError> {
        if first == 0 || first > self.usable {
            return Err(FatxError::BadCluster(first));
        }
        let mut out = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut cur = first;
        loop {
            if !seen.insert(cur) {
                return Err(FatxError::CorruptChain);
            }
            out.push(cur);
            let next = self
                .entries
                .get(cur as usize)
                .copied()
                .ok_or(FatxError::CorruptChain)?;
            if self.is_end(next) {
                return Ok(out);
            }
            if next == 0 || next > self.usable {
                return Err(FatxError::CorruptChain);
            }
            cur = next;
        }
    }

    /// Every free, addressable cluster number, ascending. Entries past
    /// `usable_clusters` are never handed out: they address nothing.
    pub fn free_clusters(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries
            .iter()
            .enumerate()
            .take(self.usable as usize + 1)
            .skip(1)
            .filter(|(_, v)| **v == 0)
            .map(|(i, _)| i as u32)
    }

    /// Reserve `count` free clusters, link them into one chain and mark the
    /// last as end-of-chain. Nothing is written if there is not enough
    /// space.
    pub fn allocate(&mut self, count: usize) -> Result<Vec<u32>, FatxError> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let picked: Vec<u32> = self.free_clusters().take(count).collect();
        if picked.len() < count {
            return Err(FatxError::NoSpace);
        }
        for pair in picked.windows(2) {
            self.entries[pair[0] as usize] = pair[1];
        }
        let last = *picked.last().expect("count > 0");
        self.entries[last as usize] = self.end_value;
        Ok(picked)
    }

    /// Mark every cluster of a chain free. A corrupt or cyclic chain frees
    /// what it can reach and stops; it never loops.
    ///
    /// Bounded by `usable_clusters`, not by the FAT length: the trailing
    /// entries address no bytes, so a chain that runs into them stops here
    /// rather than handing those entries back to the allocator.
    pub fn free_chain(&mut self, first: u32) {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut cur = first;
        while cur != 0 && cur <= self.usable && seen.insert(cur) {
            let next = self.entries[cur as usize];
            self.entries[cur as usize] = 0;
            if self.is_end(next) {
                break;
            }
            cur = next;
        }
    }

    /// Every entry either free, end-of-chain, or a cluster that has bytes
    /// behind it. This is the FAT-bounds re-check the write path runs
    /// before it touches the image.
    pub fn check_bounds(&self) -> Result<(), FatxError> {
        for value in self.entries.iter().skip(1) {
            if *value != 0 && !self.is_end(*value) && *value > self.usable {
                return Err(FatxError::CorruptChain);
            }
        }
        Ok(())
    }

    /// Write the FAT back out at `base + geo.fat_offset`, padded with zeroes
    /// to the full `geo.fat_size`.
    pub fn write(
        &self,
        io: &mut (impl Write + Seek),
        geo: &Geometry,
        base: u64,
    ) -> Result<(), FatxError> {
        let mut raw = Vec::with_capacity(geo.fat_size as usize);
        if self.fat32 {
            for v in &self.entries {
                raw.extend_from_slice(&v.to_le_bytes());
            }
        } else {
            for v in &self.entries {
                raw.extend_from_slice(&(*v as u16).to_le_bytes());
            }
        }
        raw.resize(geo.fat_size as usize, 0);
        io.seek(SeekFrom::Start(base + geo.fat_offset))?;
        io.write_all(&raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fatx::layout::{geometry, Superblock};

    fn geo() -> Geometry {
        let sb = Superblock {
            volume_id: 1,
            sectors_per_cluster: 32,
            root_dir_first_cluster: 1,
        };
        geometry(8 * 1024 * 1024, &sb).expect("geometry")
    }

    #[test]
    fn chain_loop_detection_errors_instead_of_hanging() {
        let g = geo();
        let mut fat = Fat::format(&g);
        // 1 -> 2 -> 3 -> 1 is a cycle.
        fat.set_entry(1, 2).unwrap();
        fat.set_entry(2, 3).unwrap();
        fat.set_entry(3, 1).unwrap();
        assert!(matches!(fat.chain(1), Err(FatxError::CorruptChain)));

        // A self-referencing entry is also a cycle.
        let mut fat = Fat::format(&g);
        fat.set_entry(4, 4).unwrap();
        assert!(matches!(fat.chain(4), Err(FatxError::CorruptChain)));

        // A well-formed chain still walks.
        let mut fat = Fat::format(&g);
        fat.set_entry(1, 2).unwrap();
        fat.set_entry(2, 3).unwrap();
        fat.set_entry(3, 0xFFFF_FFFF).unwrap();
        assert_eq!(fat.chain(1).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn free_chain_stops_at_the_last_usable_cluster() {
        let g = geo();
        let mut fat = Fat::format(&g);
        // The FAT is sized from cluster_count, which runs past the data
        // area, so entries above usable_clusters address nothing.
        assert!(g.usable_clusters < g.cluster_count);
        let last = g.usable_clusters as u32;
        let past = last + 1;
        assert!((past as usize) < fat.len(), "the trailing entries exist");

        // A chain that runs off the end of the data area must stop rather
        // than free entries that address nothing.
        fat.set_entry(2, last).unwrap();
        fat.set_entry(last, past).unwrap();
        fat.set_entry(past, 0xFFFF_FFFF).unwrap();
        fat.free_chain(2);
        assert_eq!(fat.entry(2), Some(0));
        assert_eq!(fat.entry(last), Some(0));
        assert_eq!(
            fat.entry(past),
            Some(0xFFFF_FFFF),
            "entries past usable_clusters must be left alone"
        );

        // A normal chain is still fully freed.
        let mut fat = Fat::format(&g);
        let picked = fat.allocate(3).unwrap();
        for c in &picked {
            assert_ne!(fat.entry(*c), Some(0));
        }
        fat.free_chain(picked[0]);
        for c in &picked {
            assert_eq!(fat.entry(*c), Some(0));
        }
    }
}
