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

use super::dir::{parse_dir_cluster, DirEntry, DIR_ENTRY_SIZE, END_OF_DIRECTORY};
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

impl FatxPartition {
    /// Open the partition of `partition_size` bytes that starts at
    /// `base_offset`. Validates the superblock and that the image really
    /// holds the superblock and the whole FAT.
    pub fn open(path: &Path, base_offset: u64, partition_size: u64) -> Result<Self, FatxError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let sb = read_superblock(&mut file, base_offset, file_len)?;
        let geo = geometry(partition_size, &sb)?;
        let needed = base_offset + geo.fat_offset + geo.fat_size;
        if file_len < needed {
            return Err(FatxError::Truncated {
                needed,
                actual: file_len,
            });
        }
        if sb.root_dir_first_cluster < 1 || u64::from(sb.root_dir_first_cluster) > geo.cluster_count
        {
            return Err(FatxError::BadCluster(sb.root_dir_first_cluster));
        }
        let fat = Fat::read(&mut file, &geo, base_offset)?;
        Ok(Self {
            file,
            base: base_offset,
            geo,
            fat,
            root_cluster: sb.root_dir_first_cluster,
        })
    }

    /// Read-only superblock and FAT-bounds check, with the partition size
    /// taken as "everything from `base_offset` to the end of the file".
    ///
    /// This is the backend of the xemu image sniffer: it never reads the
    /// FAT itself, so it stays cheap on a multi-gigabyte image.
    pub fn validate(path: &Path, base_offset: u64) -> Result<(), FatxError> {
        let mut file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let sb = read_superblock(&mut file, base_offset, file_len)?;
        let partition_size = file_len - base_offset;
        let geo = geometry(partition_size, &sb)?;
        let needed = base_offset + geo.fat_offset + geo.fat_size;
        if file_len < needed {
            return Err(FatxError::Truncated {
                needed,
                actual: file_len,
            });
        }
        if base_offset + geo.data_offset + geo.cluster_count * geo.cluster_size > file_len {
            return Err(FatxError::Truncated {
                needed: base_offset + geo.data_offset + geo.cluster_count * geo.cluster_size,
                actual: file_len,
            });
        }
        if sb.root_dir_first_cluster < 1 || u64::from(sb.root_dir_first_cluster) > geo.cluster_count
        {
            return Err(FatxError::BadCluster(sb.root_dir_first_cluster));
        }
        Ok(())
    }

    /// Derived layout of the open partition.
    pub fn geometry(&self) -> &Geometry {
        &self.geo
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
                .find(|e| super::dir::names_equal(&e.name, component));
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
        let mut out = Vec::with_capacity(size as usize);
        let mut left = size as usize;
        for cluster in self.fat.chain(first_cluster)? {
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
                if entry.name == "." || entry.name == ".." || entry.name.contains(['/', '\\']) {
                    continue;
                }
                let target = here.join(&entry.name);
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
    use crate::fatx::FatxError;
    use std::fs;

    const PART_SIZE: u64 = 8 * 1024 * 1024;

    fn sample_image(dir: &Path) -> std::path::PathBuf {
        let img = dir.join("xbox_hdd.img");
        let mut b = FatxImageBuilder::new(PART_SIZE).with_cluster_size(4096);
        b.add_dir("UDATA/4541000d/00000001");
        b.add_file("UDATA/4541000d/00000001/savedata.bin", vec![0xA5; 100]);
        // Spans several clusters so the FAT chain is exercised.
        b.add_file(
            "UDATA/4541000d/00000001/savemeta.xbx",
            (0..10_000u32).map(|i| (i % 251) as u8).collect(),
        );
        b.add_file("UDATA/notes.txt", b"hello xbox".to_vec());
        b.add_dir("TDATA/4541000d");
        b.write_to(&img).expect("build image");
        img
    }

    #[test]
    fn builder_roundtrip_read_tree_extracts_placed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let img = sample_image(tmp.path());
        FatxPartition::validate(&img, 0).expect("validate");

        let mut part = FatxPartition::open(&img, 0, PART_SIZE).expect("open");
        let root: Vec<String> = part
            .list_dir("")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(root.contains(&"UDATA".to_string()), "{root:?}");
        assert!(root.contains(&"TDATA".to_string()), "{root:?}");

        let dest = tmp.path().join("out");
        let n = part.read_tree("UDATA", &dest).expect("read_tree");
        assert_eq!(n, 3);

        assert_eq!(
            fs::read(dest.join("4541000d/00000001/savedata.bin")).unwrap(),
            vec![0xA5u8; 100]
        );
        let expected: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(
            fs::read(dest.join("4541000d/00000001/savemeta.xbx")).unwrap(),
            expected
        );
        assert_eq!(fs::read(dest.join("notes.txt")).unwrap(), b"hello xbox");
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
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "00000001");

        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UdAtA", &dest).unwrap(), 3);
    }

    #[test]
    fn validate_rejects_truncated_images() {
        let tmp = tempfile::tempdir().unwrap();
        let img = sample_image(tmp.path());
        FatxPartition::validate(&img, 0).expect("intact image validates");

        // Shorter than the superblock.
        let short = tmp.path().join("short.img");
        fs::write(&short, [0u8; 16]).unwrap();
        assert!(FatxPartition::validate(&short, 0).is_err());

        // Superblock present, but the file cannot hold the FAT plus a cluster.
        let cut = tmp.path().join("cut.img");
        fs::copy(&img, &cut).unwrap();
        let f = fs::OpenOptions::new().write(true).open(&cut).unwrap();
        f.set_len(0x1500).unwrap();
        drop(f);
        assert!(matches!(
            FatxPartition::validate(&cut, 0),
            Err(FatxError::PartitionTooSmall) | Err(FatxError::Truncated { .. })
        ));

        // open() with the declared partition size rejects a file that is
        // shorter than fat_offset + fat_size.
        assert!(matches!(
            FatxPartition::open(&cut, 0, PART_SIZE),
            Err(FatxError::Truncated { .. })
        ));

        // A base offset past the end of the file is not a FATX partition.
        assert!(FatxPartition::validate(&img, PART_SIZE * 4).is_err());
    }
}
