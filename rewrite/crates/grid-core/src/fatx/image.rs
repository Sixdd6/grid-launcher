//! Read access to a FATX partition inside a raw disk image.
//!
//! A partition is a `File` plus a base byte offset and a partition size, so
//! the same code serves an image that holds only the partition (offset 0,
//! what the builder makes for tests) and a full retail HDD image, where the
//! `E:` partition starts at [`super::layout::RETAIL_PARTITION_E_OFFSET`].
//!
//! This module is read-only; the write path lands in a later task.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::dir::{
    name_is_valid, names_equal, parse_dir_cluster, DirEntry, DIR_ENTRY_SIZE, END_OF_DIRECTORY,
};
use super::fat::Fat;
use super::layout::{geometry, Geometry, Superblock, FATX_SUPERBLOCK_SIZE};
use super::FatxError;

/// An opened FATX partition.
#[derive(Debug)]
pub struct FatxPartition {
    file: File,
    base: u64,
    geo: Geometry,
    fat: Fat,
    root_cluster: u32,
}

fn read_superblock(file: &mut File, base: u64, file_len: u64) -> Result<Superblock, FatxError> {
    if file_len < base + 16 {
        return Err(FatxError::Truncated {
            needed: base + 16,
            actual: file_len,
        });
    }
    let want = FATX_SUPERBLOCK_SIZE.min(file_len - base) as usize;
    let mut buf = vec![0u8; want];
    file.seek(SeekFrom::Start(base))?;
    file.read_exact(&mut buf)?;
    Superblock::parse(&buf)
}

/// True when this directory cluster holds an end-of-directory marker, so no
/// further cluster of the chain carries entries.
fn cluster_terminated(bytes: &[u8]) -> bool {
    bytes
        .chunks_exact(DIR_ENTRY_SIZE)
        .any(|slot| slot[0] == END_OF_DIRECTORY || slot[0] == 0)
}

fn split_path(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect()
}

/// Superblock and FAT/data bounds check shared by `open` and `validate`:
/// the image must actually contain the superblock, the whole FAT and the
/// whole data area, and the root directory must be an addressable cluster.
fn check_bounds(
    file_len: u64,
    base: u64,
    geo: &Geometry,
    sb: &Superblock,
) -> Result<(), FatxError> {
    let fat_end = base + geo.fat_offset + geo.fat_size;
    if file_len < fat_end {
        return Err(FatxError::Truncated {
            needed: fat_end,
            actual: file_len,
        });
    }
    let data_end = base + geo.data_offset + geo.data_size();
    if file_len < data_end {
        return Err(FatxError::Truncated {
            needed: data_end,
            actual: file_len,
        });
    }
    if sb.root_dir_first_cluster < 1 || u64::from(sb.root_dir_first_cluster) > geo.usable_clusters {
        return Err(FatxError::BadCluster(sb.root_dir_first_cluster));
    }
    Ok(())
}

impl FatxPartition {
    /// Open the partition of `partition_size` bytes that starts at
    /// `base_offset`. Validates the superblock and that the image really
    /// holds the superblock and the whole FAT.
    pub fn open(path: &Path, base_offset: u64, partition_size: u64) -> Result<Self, FatxError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let sb = read_superblock(&mut file, base_offset, file_len)?;
        let geo = geometry(partition_size, &sb)?;
        check_bounds(file_len, base_offset, &geo, &sb)?;
        let fat = Fat::read(&mut file, &geo, base_offset)?;
        Ok(Self {
            file,
            base: base_offset,
            geo,
            fat,
            root_cluster: sb.root_dir_first_cluster,
        })
    }

    /// Read-only superblock and FAT-bounds check for the partition of
    /// `partition_size` bytes at `base_offset`.
    ///
    /// This is the backend of the xemu image sniffer. It never reads the
    /// FAT itself, so it stays cheap on a multi-gigabyte image, and the
    /// caller passes the size it expects — for a retail image that is
    /// [`super::layout::RETAIL_PARTITION_E_SIZE`] — so a truncated image is
    /// caught here rather than silently read as a smaller filesystem.
    pub fn validate(path: &Path, base_offset: u64, partition_size: u64) -> Result<(), FatxError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let sb = read_superblock(&mut file, base_offset, file_len)?;
        let geo = geometry(partition_size, &sb)?;
        check_bounds(file_len, base_offset, &geo, &sb)
    }

    /// Derived layout of the open partition.
    pub fn geometry(&self) -> &Geometry {
        &self.geo
    }

    /// The partition's FAT, as read at open time.
    pub fn fat(&self) -> &Fat {
        &self.fat
    }

    fn read_cluster(&mut self, cluster: u32) -> Result<Vec<u8>, FatxError> {
        let offset = self.geo.cluster_offset(cluster)?;
        let mut buf = vec![0u8; self.geo.cluster_size as usize];
        self.file.seek(SeekFrom::Start(self.base + offset))?;
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn entries_at(&mut self, first_cluster: u32) -> Result<Vec<DirEntry>, FatxError> {
        if first_cluster == 0 {
            return Ok(Vec::new());
        }
        let chain = self.fat.chain(first_cluster)?;
        let mut out = Vec::new();
        for cluster in chain {
            let bytes = self.read_cluster(cluster)?;
            out.extend(parse_dir_cluster(&bytes).into_iter().map(|(_, e)| e));
            if cluster_terminated(&bytes) {
                break;
            }
        }
        Ok(out)
    }

    /// Resolve a slash-separated directory path to its first cluster.
    /// `Ok(None)` means "no such directory" — including a path component
    /// that names a file.
    fn resolve_dir(&mut self, dir_path: &str) -> Result<Option<u32>, FatxError> {
        let mut cluster = self.root_cluster;
        for component in split_path(dir_path) {
            let found = self
                .entries_at(cluster)?
                .into_iter()
                .find(|e| names_equal(&e.name, component.as_bytes()));
            match found {
                Some(e) if e.is_dir => cluster = e.first_cluster,
                _ => return Ok(None),
            }
            if cluster == 0 {
                return Ok(None);
            }
        }
        Ok(Some(cluster))
    }

    /// List one directory. An empty path (or `"/"`) lists the root.
    /// A missing directory is [`FatxError::NotADirectory`].
    pub fn list_dir(&mut self, dir_path: &str) -> Result<Vec<DirEntry>, FatxError> {
        match self.resolve_dir(dir_path)? {
            Some(cluster) => self.entries_at(cluster),
            None => Err(FatxError::NotADirectory(dir_path.to_string())),
        }
    }

    fn read_file(&mut self, first_cluster: u32, size: u32) -> Result<Vec<u8>, FatxError> {
        if size == 0 || first_cluster == 0 {
            return Ok(Vec::new());
        }
        let chain = self.fat.chain(first_cluster)?;
        // Cap the reservation by what the chain can actually hold, so a
        // corrupt 4 GB size field on a two-cluster file cannot make us ask
        // the allocator for 4 GB.
        let reachable = (chain.len() as u64).saturating_mul(self.geo.cluster_size);
        let mut out = Vec::with_capacity(u64::from(size).min(reachable) as usize);
        let mut left = size as usize;
        for cluster in chain {
            if left == 0 {
                break;
            }
            let bytes = self.read_cluster(cluster)?;
            let take = left.min(bytes.len());
            out.extend_from_slice(&bytes[..take]);
            left -= take;
        }
        if left > 0 {
            return Err(FatxError::CorruptChain);
        }
        Ok(out)
    }

    /// Extract the contents of `dir_path` into `dest`, recursively.
    ///
    /// Returns the number of files written. A missing directory (or one
    /// whose path names a file) is not an error: it returns `Ok(0)` and
    /// leaves `dest` untouched, so callers can ask for `UDATA` and `TDATA`
    /// without probing first.
    pub fn read_tree(&mut self, dir_path: &str, dest: &Path) -> Result<usize, FatxError> {
        let Some(root) = self.resolve_dir(dir_path)? else {
            return Ok(0);
        };
        std::fs::create_dir_all(dest)?;
        let mut files = 0usize;
        let mut seen: HashSet<u32> = HashSet::from([root]);
        let mut queue: Vec<(u32, PathBuf)> = vec![(root, dest.to_path_buf())];
        while let Some((cluster, here)) = queue.pop() {
            for entry in self.entries_at(cluster)? {
                // A name that is not a single safe path component (a
                // separator, a `:`, `.`, `..`) must never reach `join`.
                if !name_is_valid(&entry.name) {
                    continue;
                }
                let target = here.join(entry.display_name().as_ref());
                if entry.is_dir {
                    std::fs::create_dir_all(&target)?;
                    // A directory entry that points back at an ancestor
                    // would otherwise walk forever.
                    if entry.first_cluster != 0 && seen.insert(entry.first_cluster) {
                        queue.push((entry.first_cluster, target));
                    }
                } else {
                    let data = self.read_file(entry.first_cluster, entry.size)?;
                    std::fs::write(&target, data)?;
                    files += 1;
                }
            }
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fatx::builder::FatxImageBuilder;
    use crate::fatx::layout::FAT32X_END_MIN;
    use crate::fatx::FatxError;
    use std::fs;

    const PART_SIZE: u64 = 8 * 1024 * 1024;
    /// 512-byte clusters put this partition over the 0xFFF0-cluster
    /// threshold, so it formats as FAT32X while staying a cheap sparse file.
    const FAT32X_PART_SIZE: u64 = 33 * 1024 * 1024;

    fn big_file() -> Vec<u8> {
        (0..10_000u32).map(|i| (i % 251) as u8).collect()
    }

    fn sample_image(dir: &Path) -> std::path::PathBuf {
        let img = dir.join("xbox_hdd.img");
        let mut b = FatxImageBuilder::new(PART_SIZE).with_cluster_size(4096);
        b.add_dir("UDATA/4541000d/00000001");
        b.add_file("UDATA/4541000d/00000001/savedata.bin", vec![0xA5; 100]);
        // Spans several clusters so the FAT chain is exercised.
        b.add_file("UDATA/4541000d/00000001/savemeta.xbx", big_file());
        b.add_file("UDATA/notes.txt", b"hello xbox".to_vec());
        b.add_dir("TDATA/4541000d");
        b.write_to(&img).expect("build image");
        img
    }

    fn names(entries: &[DirEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| e.display_name().into_owned())
            .collect()
    }

    #[test]
    fn builder_roundtrip_read_tree_extracts_placed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let img = sample_image(tmp.path());
        FatxPartition::validate(&img, 0, PART_SIZE).expect("validate");

        let mut part = FatxPartition::open(&img, 0, PART_SIZE).expect("open");
        assert!(!part.geometry().fat32, "8 MiB / 4 KiB is a FAT16X volume");
        let root = names(&part.list_dir("").unwrap());
        assert!(root.contains(&"UDATA".to_string()), "{root:?}");
        assert!(root.contains(&"TDATA".to_string()), "{root:?}");

        let dest = tmp.path().join("out");
        let n = part.read_tree("UDATA", &dest).expect("read_tree");
        assert_eq!(n, 3);

        assert_eq!(
            fs::read(dest.join("4541000d/00000001/savedata.bin")).unwrap(),
            vec![0xA5u8; 100]
        );
        assert_eq!(
            fs::read(dest.join("4541000d/00000001/savemeta.xbx")).unwrap(),
            big_file()
        );
        assert_eq!(fs::read(dest.join("notes.txt")).unwrap(), b"hello xbox");
    }

    #[test]
    fn fat32x_images_roundtrip_through_the_four_byte_fat() {
        let tmp = tempfile::tempdir().unwrap();
        let img = tmp.path().join("fat32x.img");
        let spanning: Vec<u8> = (0..1500u32).map(|i| (i % 97) as u8).collect();
        let mut b = FatxImageBuilder::new(FAT32X_PART_SIZE).with_cluster_size(512);
        b.add_file("UDATA/small.bin", vec![0x7E; 40]);
        b.add_file("UDATA/spanning.bin", spanning.clone());
        b.write_to(&img).expect("build FAT32X image");

        FatxPartition::validate(&img, 0, FAT32X_PART_SIZE).expect("validate");
        let mut part = FatxPartition::open(&img, 0, FAT32X_PART_SIZE).expect("open");
        let geo = *part.geometry();
        assert!(geo.fat32, "cluster_count {}", geo.cluster_count);
        assert_eq!(geo.fat_entry_size(), 4);

        let listed = part.list_dir("UDATA").unwrap();
        assert_eq!(names(&listed), vec!["small.bin", "spanning.bin"]);

        // A one-cluster file ends with the 32-bit end-of-chain marker, and a
        // 1500-byte file needs three 512-byte clusters.
        let small = listed.iter().find(|e| e.name == b"small.bin").unwrap();
        assert!(part.fat().entry(small.first_cluster).unwrap() >= FAT32X_END_MIN);
        let big = listed.iter().find(|e| e.name == b"spanning.bin").unwrap();
        assert_eq!(part.fat().chain(big.first_cluster).unwrap().len(), 3);

        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 2);
        assert_eq!(fs::read(dest.join("small.bin")).unwrap(), vec![0x7E; 40]);
        assert_eq!(fs::read(dest.join("spanning.bin")).unwrap(), spanning);
    }

    #[test]
    fn directories_spanning_several_clusters_list_every_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let img = tmp.path().join("wide.img");
        // 4096-byte clusters hold 64 slots, so 100 entries need two.
        let mut b = FatxImageBuilder::new(PART_SIZE).with_cluster_size(4096);
        for i in 0..100 {
            b.add_file(&format!("UDATA/save{i:03}.bin"), vec![i as u8; 8]);
        }
        b.write_to(&img).expect("build image");

        let mut part = FatxPartition::open(&img, 0, PART_SIZE).expect("open");
        let listed = part.list_dir("UDATA").unwrap();
        assert_eq!(listed.len(), 100);
        assert_eq!(part.fat().chain(listed[0].first_cluster).unwrap().len(), 1);
        let udata = part
            .list_dir("")
            .unwrap()
            .into_iter()
            .find(|e| e.name == b"UDATA")
            .unwrap();
        assert_eq!(
            part.fat().chain(udata.first_cluster).unwrap().len(),
            2,
            "the directory itself must span two clusters"
        );
        // The last entry is on the second cluster.
        assert_eq!(names(&listed)[99], "save099.bin");

        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 100);
        for i in 0..100 {
            assert_eq!(
                fs::read(dest.join(format!("save{i:03}.bin"))).unwrap(),
                vec![i as u8; 8]
            );
        }
    }

    #[test]
    fn read_tree_of_a_missing_dir_returns_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let img = sample_image(tmp.path());
        let mut part = FatxPartition::open(&img, 0, PART_SIZE).expect("open");
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("NOPE", &dest).unwrap(), 0);
        assert_eq!(part.read_tree("UDATA/nothing/here", &dest).unwrap(), 0);
        assert!(
            !dest.exists(),
            "a missing dir must not create the destination"
        );
        // A file where a directory was expected is also "nothing to extract".
        assert_eq!(part.read_tree("UDATA/notes.txt", &dest).unwrap(), 0);
    }

    #[test]
    fn names_compare_case_insensitively_on_lookup() {
        let tmp = tempfile::tempdir().unwrap();
        let img = sample_image(tmp.path());
        let mut part = FatxPartition::open(&img, 0, PART_SIZE).expect("open");

        let listed = part.list_dir("udata/4541000D").unwrap();
        assert_eq!(names(&listed), vec!["00000001"]);

        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UdAtA", &dest).unwrap(), 3);
    }

    #[test]
    fn validate_rejects_truncated_images() {
        let tmp = tempfile::tempdir().unwrap();
        let img = sample_image(tmp.path());
        FatxPartition::validate(&img, 0, PART_SIZE).expect("intact image validates");

        // Shorter than the superblock.
        let short = tmp.path().join("short.img");
        fs::write(&short, [0u8; 16]).unwrap();
        assert!(FatxPartition::validate(&short, 0, PART_SIZE).is_err());

        // Half an image: the superblock and most of the FAT are there, so
        // only the declared partition size catches this.
        let half = tmp.path().join("half.img");
        fs::copy(&img, &half).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&half)
            .unwrap()
            .set_len(PART_SIZE / 2)
            .unwrap();
        assert!(matches!(
            FatxPartition::open(&half, 0, PART_SIZE),
            Err(FatxError::Truncated { .. })
        ));
        let err = FatxPartition::validate(&half, 0, PART_SIZE).unwrap_err();
        match err {
            FatxError::Truncated { needed, actual } => {
                assert_eq!(needed, PART_SIZE);
                assert_eq!(actual, PART_SIZE / 2);
            }
            other => panic!("expected Truncated, got {other:?}"),
        }

        // Cut back to just past the superblock: now even the FAT is missing.
        let cut = tmp.path().join("cut.img");
        fs::copy(&img, &cut).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&cut)
            .unwrap()
            .set_len(0x1500)
            .unwrap();
        assert!(matches!(
            FatxPartition::validate(&cut, 0, PART_SIZE),
            Err(FatxError::Truncated { .. })
        ));
        assert!(matches!(
            FatxPartition::open(&cut, 0, PART_SIZE),
            Err(FatxError::Truncated { .. })
        ));

        // A partition size too small to hold anything is its own error.
        assert!(matches!(
            FatxPartition::validate(&img, 0, 0x2000),
            Err(FatxError::PartitionTooSmall)
        ));

        // A base offset past the end of the file is not a FATX partition.
        assert!(FatxPartition::validate(&img, PART_SIZE * 4, PART_SIZE).is_err());
    }
}
