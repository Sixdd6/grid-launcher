//! Test-support: build a valid FATX partition image from scratch.
//!
//! **This is not production code.** It exists so the read path, the write
//! path and the xemu integration tests have real FATX images to work on
//! without shipping a binary fixture or running a GPL formatter. It is
//! `pub` (not `#[cfg(test)]`) because integration tests in `tests/` and the
//! write-path tests of later tasks need it.
//!
//! Simplifications it is allowed to make, being a fixture generator:
//! files and directories are laid out in one depth-first pass, clusters are
//! handed out in ascending order, and a directory that does not fit its
//! reserved clusters is an error rather than a re-layout.

use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use super::dir::{encode_dir_entry, name_is_valid, pack_timestamp, DirEntry, DIR_ENTRY_SIZE};
use super::fat::Fat;
use super::layout::{geometry, Geometry, Superblock, SECTOR_SIZE};
use super::FatxError;

#[derive(Debug)]
struct Node {
    name: String,
    is_dir: bool,
    data: Vec<u8>,
    children: Vec<Node>,
}

impl Node {
    fn dir(name: &str) -> Self {
        Self {
            name: name.to_string(),
            is_dir: true,
            data: Vec::new(),
            children: Vec::new(),
        }
    }
}

/// Builds a raw file holding one FATX partition at a configurable base
/// offset (0 for unit tests, the retail `E:` offset for integration tests).
#[derive(Debug)]
pub struct FatxImageBuilder {
    partition_size: u64,
    base_offset: u64,
    cluster_size: u64,
    volume_id: u32,
    root: Node,
}

/// One placed object: the clusters it owns and the bytes to put in them.
struct Placement {
    clusters: Vec<u32>,
    data: Vec<u8>,
    pad: bool,
}

impl FatxImageBuilder {
    /// A partition of `partition_size` bytes at offset 0 with 16 KiB
    /// clusters (the retail `E:` cluster size).
    pub fn new(partition_size: u64) -> Self {
        Self {
            partition_size,
            base_offset: 0,
            cluster_size: 16 * 1024,
            volume_id: 0x4752_4944,
            root: Node::dir(""),
        }
    }

    /// Place the partition at `base` inside the image file. The bytes
    /// before it are never written, so the file stays sparse.
    pub fn with_base_offset(mut self, base: u64) -> Self {
        self.base_offset = base;
        self
    }

    /// Cluster size in bytes; must be a power of two from 512 to 65536.
    pub fn with_cluster_size(mut self, bytes: u64) -> Self {
        self.cluster_size = bytes;
        self
    }

    /// Volume id written into the superblock.
    pub fn with_volume_id(mut self, id: u32) -> Self {
        self.volume_id = id;
        self
    }

    fn dir_node(&mut self, components: &[&str]) -> &mut Node {
        let mut node = &mut self.root;
        for name in components {
            let existing = node.children.iter().position(|c| c.name == *name);
            let index = match existing {
                Some(i) => i,
                None => {
                    node.children.push(Node::dir(name));
                    node.children.len() - 1
                }
            };
            node = &mut node.children[index];
        }
        node
    }

    /// Create a directory and every parent it needs.
    pub fn add_dir(&mut self, path: &str) -> &mut Self {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        self.dir_node(&parts);
        self
    }

    /// Create a file, and every parent directory it needs.
    pub fn add_file(&mut self, path: &str, data: Vec<u8>) -> &mut Self {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let (name, parents) = parts.split_last().expect("file path must not be empty");
        let name = name.to_string();
        let parent = self.dir_node(parents);
        parent.children.retain(|c| c.name != name);
        parent.children.push(Node {
            name,
            is_dir: false,
            data,
            children: Vec::new(),
        });
        self
    }

    /// Format the image and write it to `path`, replacing any existing file.
    pub fn write_to(&self, path: &Path) -> Result<(), FatxError> {
        if self.cluster_size < SECTOR_SIZE
            || self.cluster_size > 64 * 1024
            || !self.cluster_size.is_power_of_two()
        {
            return Err(FatxError::BadClusterSize(self.cluster_size as u32));
        }
        let sb = Superblock {
            volume_id: self.volume_id,
            sectors_per_cluster: (self.cluster_size / SECTOR_SIZE) as u32,
            root_dir_first_cluster: 1,
        };
        let geo = geometry(self.partition_size, &sb)?;
        let mut fat = Fat::format(&geo);
        let mut placements = Vec::new();
        place_dir(&self.root, &mut fat, &geo, &mut placements)?;

        let mut file = File::create(path)?;
        // Sparse: reserve the whole image without writing the hole.
        file.set_len(self.base_offset + self.partition_size)?;
        file.seek(SeekFrom::Start(self.base_offset))?;
        file.write_all(&sb.encode())?;
        fat.write(&mut file, &geo, self.base_offset)?;
        for placement in &placements {
            let mut left: &[u8] = &placement.data;
            for cluster in &placement.clusters {
                let take = left.len().min(geo.cluster_size as usize);
                let (chunk, rest) = left.split_at(take);
                let mut buf = chunk.to_vec();
                if placement.pad {
                    buf.resize(geo.cluster_size as usize, 0xFF);
                }
                file.seek(SeekFrom::Start(
                    self.base_offset + geo.cluster_offset(*cluster)?,
                ))?;
                file.write_all(&buf)?;
                left = rest;
            }
        }
        file.flush()?;
        Ok(())
    }
}

fn clusters_for(bytes: u64, cluster_size: u64) -> usize {
    bytes.div_ceil(cluster_size).max(1) as usize
}

/// Depth-first placement. The directory's own clusters are reserved before
/// its children's, so the root directory always lands on cluster 1.
fn place_dir(
    node: &Node,
    fat: &mut Fat,
    geo: &Geometry,
    out: &mut Vec<Placement>,
) -> Result<u32, FatxError> {
    // One slot per child plus the end-of-directory marker slot.
    let needed = ((node.children.len() + 1) * DIR_ENTRY_SIZE) as u64;
    let clusters = fat.allocate(clusters_for(needed, geo.cluster_size))?;
    let first = clusters[0];

    let mut table = Vec::with_capacity(node.children.len() * DIR_ENTRY_SIZE);
    let stamp = pack_timestamp(2024, 1, 1, 0, 0, 0);
    for child in &node.children {
        if !name_is_valid(child.name.as_bytes()) {
            return Err(FatxError::InvalidName(child.name.clone()));
        }
        let (child_first, size) = if child.is_dir {
            (place_dir(child, fat, geo, out)?, 0u32)
        } else {
            (place_file(child, fat, geo, out)?, child.data.len() as u32)
        };
        let entry = DirEntry::new(&child.name, child.is_dir, child_first, size);
        table.extend_from_slice(&encode_dir_entry(&entry, stamp)?);
    }
    if table.len() as u64 > clusters.len() as u64 * geo.cluster_size {
        return Err(FatxError::DirectoryFull);
    }
    out.push(Placement {
        clusters,
        data: table,
        // 0xFF padding leaves the end-of-directory marker after the last
        // entry and through every unused slot.
        pad: true,
    });
    Ok(first)
}

fn place_file(
    node: &Node,
    fat: &mut Fat,
    geo: &Geometry,
    out: &mut Vec<Placement>,
) -> Result<u32, FatxError> {
    if node.data.is_empty() {
        return Ok(0);
    }
    let clusters = fat.allocate(clusters_for(node.data.len() as u64, geo.cluster_size))?;
    let first = clusters[0];
    out.push(Placement {
        clusters,
        data: node.data.clone(),
        pad: false,
    });
    Ok(first)
}
