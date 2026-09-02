//! The FATX write path: put a host directory tree into a partition, and
//! take one out again.
//!
//! # Crash ordering
//!
//! Nothing here rewrites the superblock. For every file the order is
//! fixed:
//!
//! 1. the file's data clusters (and, when a directory has to grow, the new
//!    directory cluster, filled with the `0xFF` end marker),
//! 2. the FAT,
//! 3. the directory entry.
//!
//! A crash therefore leaves at worst orphaned clusters — allocated in the
//! FAT, named by nothing, and reclaimable by an fsck — never a directory
//! entry pointing at clusters the FAT has not committed. Removal runs the
//! same rule backwards: the directory entries go to `0xE5` first, then the
//! chains are freed, so a crash again leaks clusters rather than leaving a
//! live entry pointing at reusable ones.
//!
//! # Durability
//!
//! Issuing the writes in that order is not enough. `Write::flush` on a
//! `File` is a no-op in std, and the kernel is free to write back dirty
//! pages in any order, so a power cut could still land the directory entry
//! before the FAT — exactly the state the ordering rule exists to prevent.
//! Every phase boundary is therefore a real barrier, [`DurableWrite`]:
//!
//! - the data clusters are **durable before** the FAT is written,
//! - the FAT is **durable before** the directory entry is written,
//! - on removal, the `0xE5` marks are **durable before** the chains are
//!   freed,
//! - and on an overwrite that shrinks a file, the surplus clusters stay
//!   allocated in a first FAT commit, and are only freed by a second
//!   commit after the new directory entry is durable — so no window exists
//!   where a free cluster is still named by a live entry.
//!
//! `DurableWrite for File` calls `sync_data`. The default method is a
//! no-op, which is what in-memory test stores want.
//!
//! Out of space is decided before any byte is written: the allocation runs
//! against a scratch copy of the in-memory FAT, and a
//! [`FatxError::NoSpace`] drops that copy with the image untouched. So is
//! every name check, so an unusable name can never orphan a committed
//! cluster.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use chrono::{Datelike, Local, Timelike};

use super::dir::{
    encode_dir_entry, name_is_valid, names_equal, pack_timestamp, DirEntry, ATTR_DIRECTORY,
    DELETED_ENTRY, DIR_ENTRY_SIZE, END_OF_DIRECTORY, MAX_NAME_LEN,
};
use super::fat::Fat;
use super::image::{check_bounds, read_superblock, split_path, FatxPartition};
use super::layout::geometry;
use super::FatxError;

/// A backing store that can make its writes durable.
///
/// The write path orders its phases with this, not with `Write::flush`:
/// `flush` on a `File` is a no-op in std, so on its own it guarantees
/// nothing about what reaches the platter first. The default `barrier` is
/// a no-op, which is correct for an in-memory store — there is no
/// write-back to order.
pub trait DurableWrite {
    /// Return once every byte written so far is durable.
    fn barrier(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl DurableWrite for File {
    /// `sync_data` rather than `sync_all`: the image's own metadata (its
    /// length, its timestamps) does not order anything inside it, and
    /// skipping it saves a second seek per phase.
    fn barrier(&mut self) -> std::io::Result<()> {
        self.sync_data()
    }
}

/// In-memory stores have no write-back to order.
impl<T> DurableWrite for Cursor<T> {}

/// How deep a tree either direction will walk. Save data nests a few
/// levels; anything past this is a pathological host tree or a crafted
/// image, and recursing on it would blow the stack.
const MAX_DEPTH: usize = 64;

/// Where one 64-byte directory slot lives: a cluster and a byte offset
/// inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    cluster: u32,
    offset: usize,
}

/// One walk of a directory: its cluster chain, its live entries, and the
/// first slot a new entry could use.
struct DirLayout {
    chain: Vec<u32>,
    live: Vec<(Slot, DirEntry)>,
    /// First reusable slot, and whether it currently holds the
    /// end-of-directory marker (in which case the marker has to move on).
    free: Option<(Slot, bool)>,
}

impl DirLayout {
    fn find(&self, name: &[u8]) -> Option<(Slot, DirEntry)> {
        self.live
            .iter()
            .find(|(_, e)| names_equal(&e.name, name))
            .map(|(s, e)| (*s, e.clone()))
    }
}

/// Wall clock, packed for a directory entry.
fn now_timestamp() -> u32 {
    let now = Local::now();
    pack_timestamp(
        now.year().max(0) as u32,
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Which slot a new directory entry is going into, and what else that
/// implies.
#[derive(Debug, Clone, Copy)]
enum SlotPlan {
    /// Reuse the slot an entry of the same name already occupies.
    Existing(Slot),
    /// A deleted slot, or the slot the end marker sits in (`true`).
    Free(Slot, bool),
    /// The directory had no room, so it grew by this cluster and the entry
    /// goes in its first slot.
    Grown(u32),
}

impl SlotPlan {
    fn slot(&self) -> Slot {
        match self {
            SlotPlan::Existing(s) | SlotPlan::Free(s, _) => *s,
            SlotPlan::Grown(cluster) => Slot {
                cluster: *cluster,
                offset: 0,
            },
        }
    }
}

impl<S: Read + Write + Seek + DurableWrite> FatxPartition<S> {
    /// Inject the contents of the local directory `src` into `dir_path`,
    /// the mirror image of [`FatxPartition::read_tree`].
    ///
    /// `dir_path` and every directory under it are created when missing. A
    /// file that already exists is overwritten: its old chain is freed and
    /// a fresh one allocated, and its existing directory slot is updated in
    /// place. Symlinks in `src` are skipped. Returns the number of files
    /// written.
    ///
    /// Atomicity is per file, not per tree. Each file, and each directory
    /// created along the way, is committed on its own in the order above,
    /// so a [`FatxError::NoSpace`] partway through leaves the files already
    /// written in place and the image consistent — never a half-written
    /// file or an entry pointing at clusters the FAT does not own.
    pub fn write_tree(&mut self, dir_path: &str, src: &Path) -> Result<usize, FatxError> {
        self.revalidate()?;
        // Every component is checked before anything is allocated, so an
        // unusable destination path cannot commit a cluster and then fail.
        let components = split_path(dir_path);
        for component in &components {
            if !name_is_valid(component.as_bytes()) {
                return Err(FatxError::InvalidName((*component).to_string()));
            }
        }
        let mut cluster = self.root_cluster;
        for component in &components {
            cluster = self.ensure_dir(cluster, component.as_bytes())?;
        }
        let mut written = 0usize;
        match self.write_dir_contents(cluster, src, 0, &mut written) {
            Ok(()) => {
                self.io.flush()?;
                Ok(written)
            }
            // The files already written are durable and readable. Say how
            // many, so a caller is not left guessing what landed.
            Err(source) if written > 0 => Err(FatxError::PartialWrite {
                written,
                source: Box::new(source),
            }),
            Err(source) => Err(source),
        }
    }

    /// Remove `dir_path` — a directory and everything under it, or a single
    /// file — freeing every cluster chain it owns.
    ///
    /// A path that is not there is not an error, so a caller can clear
    /// `UDATA` and `TDATA` without probing first. The root itself cannot be
    /// removed.
    pub fn remove_tree(&mut self, dir_path: &str) -> Result<(), FatxError> {
        self.revalidate()?;
        let parts = split_path(dir_path);
        let Some((last, parents)) = parts.split_last() else {
            return Err(FatxError::NotADirectory(dir_path.to_string()));
        };
        let mut cluster = self.root_cluster;
        for component in parents {
            match self.dir_layout(cluster)?.find(component.as_bytes()) {
                Some((_, entry)) if entry.is_dir && entry.first_cluster != 0 => {
                    cluster = entry.first_cluster
                }
                _ => return Ok(()),
            }
        }
        let Some((slot, entry)) = self.dir_layout(cluster)?.find(last.as_bytes()) else {
            return Ok(());
        };

        // Everything the subtree owns, collected before a single byte moves.
        let mut chains: Vec<u32> = Vec::new();
        let mut slots: Vec<Slot> = Vec::new();
        if entry.is_dir {
            self.collect_subtree(entry.first_cluster, &mut chains, &mut slots)?;
        } else if entry.first_cluster != 0 {
            chains.push(entry.first_cluster);
        }

        // The entry in the parent goes first: that alone makes the whole
        // subtree unreachable, so a crash after it leaks clusters instead
        // of leaving anything dangling.
        self.mark_deleted(slot)?;
        for doomed in slots {
            self.mark_deleted(doomed)?;
        }
        // Durable before a single cluster is handed back, so no free
        // cluster is ever still named by a live entry.
        self.barrier()?;
        for first in chains {
            self.fat.free_chain(first);
        }
        self.commit_fat()?;
        self.io.flush()?;
        Ok(())
    }

    /// Re-run the open-time checks against the live image: signature,
    /// geometry, that the image still holds the whole FAT and data area,
    /// and that no FAT entry points outside the data area.
    ///
    /// The superblock and the bounds are re-read from the store, but the
    /// FAT check is against the **cached** copy this partition holds — the
    /// copy every write is derived from, and the one that would corrupt the
    /// image if it were wrong. It is not a re-read of the on-disk FAT, so
    /// it does not detect another writer changing the image underneath us;
    /// nothing in this crate opens the same image twice for writing.
    fn revalidate(&mut self) -> Result<(), FatxError> {
        let len = self.io.seek(SeekFrom::End(0))?;
        let sb = read_superblock(&mut self.io, self.base, len)?;
        let geo = geometry(self.partition_size, &sb)?;
        check_bounds(len, self.base, &geo, &sb)?;
        if geo != self.geo || sb.root_dir_first_cluster != self.root_cluster {
            return Err(FatxError::GeometryChanged);
        }
        self.fat.check_bounds()
    }

    /// Copy every child of the host directory `src` into the image
    /// directory whose chain starts at `dir_cluster`.
    fn write_dir_contents(
        &mut self,
        dir_cluster: u32,
        src: &Path,
        depth: usize,
        written: &mut usize,
    ) -> Result<(), FatxError> {
        if depth >= MAX_DEPTH {
            return Err(FatxError::TooDeep(MAX_DEPTH));
        }
        let mut children: Vec<(std::ffi::OsString, bool)> = Vec::new();
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            // `file_type` does not follow symlinks, so a link (and any loop
            // it could form) is skipped rather than copied.
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() || kind.is_file() {
                children.push((entry.file_name(), kind.is_dir()));
            }
        }
        // Deterministic order, so two runs over the same tree lay clusters
        // out the same way.
        children.sort();

        for (name, is_dir) in children {
            let bytes = name.as_encoded_bytes().to_vec();
            if !name_is_valid(&bytes) {
                return Err(FatxError::InvalidName(name.to_string_lossy().into_owned()));
            }
            let path = src.join(&name);
            if is_dir {
                let child = self.ensure_dir(dir_cluster, &bytes)?;
                self.write_dir_contents(child, &path, depth + 1, written)?;
            } else {
                let data = std::fs::read(&path)?;
                self.write_file(dir_cluster, &bytes, &data)?;
                *written += 1;
            }
        }
        Ok(())
    }

    /// Look up `name` in the directory at `dir_cluster`, creating it as a
    /// directory when it is missing. Returns its first cluster.
    fn ensure_dir(&mut self, dir_cluster: u32, name: &[u8]) -> Result<u32, FatxError> {
        // Before anything is allocated: an unusable name that only
        // `encode_dir_entry` caught would already have committed a cluster
        // to the FAT that nothing then names.
        if !name_is_valid(name) {
            return Err(FatxError::InvalidName(
                String::from_utf8_lossy(name).into_owned(),
            ));
        }
        let layout = self.dir_layout(dir_cluster)?;
        if let Some((_, entry)) = layout.find(name) {
            if !entry.is_dir {
                return Err(FatxError::NotADirectory(
                    String::from_utf8_lossy(name).into_owned(),
                ));
            }
            if entry.first_cluster == 0 {
                return Err(FatxError::CorruptChain);
            }
            return Ok(entry.first_cluster);
        }

        // Allocate against a scratch FAT, so out of space changes nothing.
        let mut fat = self.fat.clone();
        let own = fat.allocate(1)?[0];
        let plan = self.plan_slot(&layout, &mut fat, None)?;

        // 1. Data-area bytes: the new directory's cluster, and the parent's
        //    new cluster when it had to grow. Both are all-0xFF, so they
        //    read as an empty directory the moment the FAT names them.
        self.fill_cluster(own, END_OF_DIRECTORY)?;
        if let SlotPlan::Grown(cluster) = plan {
            self.fill_cluster(cluster, END_OF_DIRECTORY)?;
        }
        self.barrier()?;
        // 2. The FAT, durable before anything names these clusters.
        self.fat = fat;
        self.commit_fat()?;
        // 3. The directory entry.
        self.place_entry(&layout, plan, &DirEntry::new_bytes(name, true, own, 0))?;
        Ok(own)
    }

    /// Write `data` as the file `name` in the directory at `dir_cluster`,
    /// replacing any file of that name.
    fn write_file(&mut self, dir_cluster: u32, name: &[u8], data: &[u8]) -> Result<(), FatxError> {
        if !name_is_valid(name) {
            return Err(FatxError::InvalidName(
                String::from_utf8_lossy(name).into_owned(),
            ));
        }
        if data.len() > u32::MAX as usize {
            // A FATX size field is 32 bits; this is the file's fault, not
            // the volume's, so it is not NoSpace.
            return Err(FatxError::FileTooLarge(data.len() as u64));
        }
        let layout = self.dir_layout(dir_cluster)?;
        let existing = layout.find(name);
        if let Some((_, entry)) = &existing {
            if entry.is_dir {
                return Err(FatxError::NotAFile(
                    String::from_utf8_lossy(name).into_owned(),
                ));
            }
        }

        let cluster_size = self.geo.cluster_size as usize;
        let needed = data.len().div_ceil(cluster_size);

        // Overwrite: free the old chain in the scratch FAT first, so the
        // fresh allocation may reuse those clusters instead of needing
        // twice the space.
        // The old chain is freed unconditionally — `free_chain` reclaims
        // what it can reach even when the chain is corrupt. `old_clusters`
        // is only used to work out the surplus below, so a chain we cannot
        // walk simply yields no surplus and one FAT commit.
        let old_first = match &existing {
            Some((_, entry)) => entry.first_cluster,
            None => 0,
        };
        let old_clusters: Vec<u32> = if old_first == 0 {
            Vec::new()
        } else {
            self.fat.chain(old_first).unwrap_or_default()
        };
        let mut fat = self.fat.clone();
        if old_first != 0 {
            fat.free_chain(old_first);
        }
        let chain = fat.allocate(needed)?;
        let plan = self.plan_slot(&layout, &mut fat, existing.as_ref().map(|(s, _)| *s))?;

        // Clusters the old file held that the new one does not. They must
        // stay allocated until the new directory entry is durable: freeing
        // them earlier leaves a window where a free cluster is still named
        // by the entry on disk, and a concurrent allocation could hand the
        // same cluster to a second file.
        let mut kept: HashSet<u32> = chain.iter().copied().collect();
        if let SlotPlan::Grown(cluster) = plan {
            kept.insert(cluster);
        }
        let surplus: Vec<u32> = old_clusters
            .iter()
            .copied()
            .filter(|c| !kept.contains(c))
            .collect();
        let mut interim = fat.clone();
        for cluster in &surplus {
            interim.set_entry(*cluster, self.geo.end_of_chain())?;
        }

        // 1. Data clusters, and the parent's new cluster if it grew.
        for (index, cluster) in chain.iter().enumerate() {
            let start = index * cluster_size;
            let end = (start + cluster_size).min(data.len());
            self.write_cluster(*cluster, &data[start..end], 0)?;
        }
        if let SlotPlan::Grown(cluster) = plan {
            self.fill_cluster(cluster, END_OF_DIRECTORY)?;
        }
        self.barrier()?;
        // 2. The FAT, with the surplus still allocated, durable before the
        //    entry names the new chain.
        self.fat = interim;
        self.commit_fat()?;
        // 3. The directory entry.
        let first = chain.first().copied().unwrap_or(0);
        let entry = DirEntry::new_bytes(name, false, first, data.len() as u32);
        self.place_entry(&layout, plan, &entry)?;
        // 4. Only now is it safe to give the surplus back.
        if !surplus.is_empty() {
            self.barrier()?;
            self.fat = fat;
            self.commit_fat()?;
        }
        Ok(())
    }

    /// Decide which slot a new or updated entry goes in, growing the
    /// directory in `fat` when every slot is taken. Nothing is written.
    fn plan_slot(
        &mut self,
        layout: &DirLayout,
        fat: &mut Fat,
        existing: Option<Slot>,
    ) -> Result<SlotPlan, FatxError> {
        if let Some(slot) = existing {
            return Ok(SlotPlan::Existing(slot));
        }
        if let Some((slot, was_marker)) = layout.free {
            return Ok(SlotPlan::Free(slot, was_marker));
        }
        let grown = fat.allocate(1)?[0];
        let last = *layout.chain.last().ok_or(FatxError::CorruptChain)?;
        fat.set_entry(last, grown)?;
        Ok(SlotPlan::Grown(grown))
    }

    /// Write the 64-byte entry, moving the end-of-directory marker along
    /// first when the slot used to hold it.
    fn place_entry(
        &mut self,
        layout: &DirLayout,
        plan: SlotPlan,
        entry: &DirEntry,
    ) -> Result<(), FatxError> {
        let slot = plan.slot();
        // The marker goes down before the entry does, so no state in
        // between can be read as "entry, then garbage".
        if let SlotPlan::Free(_, true) = plan {
            if let Some(next) = self.next_slot(layout, slot) {
                self.write_at(self.slot_offset(next)?, &[END_OF_DIRECTORY])?;
            }
        }
        let raw = encode_dir_entry(entry, now_timestamp())?;
        self.write_at(self.slot_offset(slot)?, &raw)
    }

    /// The slot after `slot`, within the cluster or on the next cluster of
    /// the directory's chain. `None` when the chain ends there.
    fn next_slot(&self, layout: &DirLayout, slot: Slot) -> Option<Slot> {
        let next_offset = slot.offset + DIR_ENTRY_SIZE;
        if (next_offset as u64) < self.geo.cluster_size {
            return Some(Slot {
                cluster: slot.cluster,
                offset: next_offset,
            });
        }
        let at = layout.chain.iter().position(|c| *c == slot.cluster)?;
        layout.chain.get(at + 1).map(|cluster| Slot {
            cluster: *cluster,
            offset: 0,
        })
    }

    fn slot_offset(&self, slot: Slot) -> Result<u64, FatxError> {
        Ok(self.geo.cluster_offset(slot.cluster)? + slot.offset as u64)
    }

    /// Mark one directory slot deleted. One byte, and it is what makes the
    /// entry unreachable.
    fn mark_deleted(&mut self, slot: Slot) -> Result<(), FatxError> {
        self.write_at(self.slot_offset(slot)?, &[DELETED_ENTRY])
    }

    /// Every cluster chain and every directory slot under the directory at
    /// `first`, including the directory's own chain.
    ///
    /// Iterative on purpose: a crafted image can nest directories as deeply
    /// as it has clusters, and recursion on that would blow the stack. The
    /// visited set means a directory pointing back at an ancestor is
    /// counted once instead of looping.
    fn collect_subtree(
        &mut self,
        first: u32,
        chains: &mut Vec<u32>,
        slots: &mut Vec<Slot>,
    ) -> Result<(), FatxError> {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut queue = vec![first];
        while let Some(cluster) = queue.pop() {
            if cluster == 0 || !seen.insert(cluster) {
                continue;
            }
            chains.push(cluster);
            for (slot, entry) in self.dir_layout(cluster)?.live {
                slots.push(slot);
                if entry.is_dir {
                    queue.push(entry.first_cluster);
                } else if entry.first_cluster != 0 {
                    chains.push(entry.first_cluster);
                }
            }
        }
        Ok(())
    }

    /// Walk a directory once: its chain, its live entries with the slot
    /// each sits in, and the first slot a new entry could take.
    fn dir_layout(&mut self, first: u32) -> Result<DirLayout, FatxError> {
        let chain = self.fat.chain(first)?;
        let mut live = Vec::new();
        let mut free: Option<(Slot, bool)> = None;
        'outer: for cluster in &chain {
            let bytes = self.read_cluster(*cluster)?;
            for (index, raw) in bytes.chunks_exact(DIR_ENTRY_SIZE).enumerate() {
                let slot = Slot {
                    cluster: *cluster,
                    offset: index * DIR_ENTRY_SIZE,
                };
                match raw[0] {
                    END_OF_DIRECTORY | 0 => {
                        free.get_or_insert((slot, true));
                        break 'outer;
                    }
                    DELETED_ENTRY => {
                        free.get_or_insert((slot, false));
                    }
                    len if len as usize <= MAX_NAME_LEN => live.push((
                        slot,
                        DirEntry {
                            name: raw[2..2 + len as usize].to_vec(),
                            is_dir: raw[1] & ATTR_DIRECTORY != 0,
                            first_cluster: u32::from_le_bytes([raw[44], raw[45], raw[46], raw[47]]),
                            size: u32::from_le_bytes([raw[48], raw[49], raw[50], raw[51]]),
                        },
                    )),
                    // A length byte that cannot be a name: leave the slot
                    // alone rather than trust it or hand it out.
                    _ => {}
                }
            }
        }
        Ok(DirLayout { chain, live, free })
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), FatxError> {
        self.io.seek(SeekFrom::Start(self.base + offset))?;
        self.io.write_all(data)?;
        Ok(())
    }

    /// Write `data` into one cluster, padding the rest of it with `fill` so
    /// no bytes of whatever used to live there survive.
    fn write_cluster(&mut self, cluster: u32, data: &[u8], fill: u8) -> Result<(), FatxError> {
        let offset = self.geo.cluster_offset(cluster)?;
        let mut buf = data.to_vec();
        buf.resize(self.geo.cluster_size as usize, fill);
        self.write_at(offset, &buf)
    }

    fn fill_cluster(&mut self, cluster: u32, fill: u8) -> Result<(), FatxError> {
        self.write_cluster(cluster, &[], fill)
    }

    /// Write the FAT out and wait for it to be durable. Every phase
    /// boundary in this module goes through here or through
    /// [`FatxPartition::barrier`].
    fn commit_fat(&mut self) -> Result<(), FatxError> {
        self.fat.write(&mut self.io, &self.geo, self.base)?;
        self.barrier()
    }

    /// Return once everything written so far is durable.
    fn barrier(&mut self) -> Result<(), FatxError> {
        self.io.barrier()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fatx::builder::FatxImageBuilder;
    use crate::fatx::dir::DirEntry;
    use crate::fatx::layout::Geometry as Geo;
    use std::fs;
    use std::io::SeekFrom;
    use std::ops::Range;
    use std::path::PathBuf;

    const PART_SIZE: u64 = 8 * 1024 * 1024;
    const CLUSTER: u64 = 4096;

    /// A partition backed by an in-memory image, so a test can look at the
    /// bytes afterwards.
    type MemPart = FatxPartition<Cursor<Vec<u8>>>;

    fn blank_image(dir: &Path) -> PathBuf {
        let img = dir.join("blank.img");
        FatxImageBuilder::new(PART_SIZE)
            .with_cluster_size(CLUSTER)
            .write_to(&img)
            .expect("build image");
        img
    }

    fn open_mem(img: &Path) -> MemPart {
        let bytes = fs::read(img).expect("read image");
        FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("open")
    }

    fn free_count(part: &MemPart) -> usize {
        part.fat().free_clusters().count()
    }

    fn names(entries: &[DirEntry]) -> Vec<String> {
        let mut out: Vec<String> = entries
            .iter()
            .map(|e| e.display_name().into_owned())
            .collect();
        out.sort();
        out
    }

    fn pattern(len: usize, seed: u8) -> Vec<u8> {
        (0..len)
            .map(|i| ((i as u32 + seed as u32) % 251) as u8)
            .collect()
    }

    /// Lay out a host tree under `root`: `(relative path, contents)`.
    fn host_tree(root: &Path, files: &[(&str, Vec<u8>)]) {
        for (rel, data) in files {
            let target = root.join(rel);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, data).unwrap();
        }
    }

    #[test]
    fn write_tree_roundtrips_through_read_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());

        let big = pattern(10_000, 7); // > 2 clusters at 4096 bytes
        let src = tmp.path().join("src");
        host_tree(
            &src,
            &[
                ("4541000d/00000001/savedata.bin", vec![0xA5; 100]),
                ("4541000d/00000001/savemeta.xbx", big.clone()),
                ("notes.txt", b"hello xbox".to_vec()),
            ],
        );

        let mut part = open_mem(&img);
        assert_eq!(part.write_tree("UDATA", &src).unwrap(), 3);

        // Read back through the reader, on a freshly opened partition, so
        // nothing in memory can paper over a bad on-disk layout.
        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");

        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 3);
        assert_eq!(
            fs::read(dest.join("4541000d/00000001/savedata.bin")).unwrap(),
            vec![0xA5u8; 100]
        );
        assert_eq!(
            fs::read(dest.join("4541000d/00000001/savemeta.xbx")).unwrap(),
            big
        );
        assert_eq!(fs::read(dest.join("notes.txt")).unwrap(), b"hello xbox");

        // The multi-cluster file really does span three clusters.
        let listed = part.list_dir("UDATA/4541000d/00000001").unwrap();
        assert_eq!(names(&listed), vec!["savedata.bin", "savemeta.xbx"]);
        let meta = listed
            .iter()
            .find(|e| e.name == b"savemeta.xbx")
            .expect("savemeta.xbx");
        assert_eq!(meta.size, 10_000);
        assert_eq!(part.fat().chain(meta.first_cluster).unwrap().len(), 3);
    }

    #[test]
    fn write_overwrites_an_existing_file_and_frees_its_old_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());

        let src = tmp.path().join("src");
        host_tree(&src, &[("save.bin", pattern(10_000, 1))]);
        let mut part = open_mem(&img);
        assert_eq!(part.write_tree("UDATA", &src).unwrap(), 1);
        let after_first = free_count(&part);
        let first_cluster = part.list_dir("UDATA").unwrap()[0].first_cluster;
        let slots_before = part.list_dir("UDATA").unwrap().len();

        // Same name, same length: the old chain must be freed and a fresh
        // one allocated, so the free-cluster count comes out identical and
        // the directory gains no second entry.
        let src2 = tmp.path().join("src2");
        host_tree(&src2, &[("save.bin", pattern(10_000, 2))]);
        assert_eq!(part.write_tree("UDATA", &src2).unwrap(), 1);
        assert_eq!(
            free_count(&part),
            after_first,
            "the old chain must be freed, not leaked"
        );
        let listed = part.list_dir("UDATA").unwrap();
        assert_eq!(listed.len(), slots_before, "the slot must be reused");
        assert_eq!(names(&listed), vec!["save.bin"]);
        assert_eq!(listed[0].size, 10_000);
        // Freed first, allocated after, so the same clusters come back.
        assert_eq!(listed[0].first_cluster, first_cluster);

        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 1);
        assert_eq!(fs::read(dest.join("save.bin")).unwrap(), pattern(10_000, 2));
    }

    #[test]
    fn write_grows_and_shrinks_files_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let mut part = open_mem(&img);
        let pristine_free = free_count(&part);

        for (len, want_clusters) in [(100usize, 1usize), (10_000, 3), (5, 1), (0, 0)] {
            let src = tmp.path().join(format!("src{len}"));
            host_tree(&src, &[("save.bin", pattern(len, len as u8))]);
            assert_eq!(part.write_tree("UDATA", &src).unwrap(), 1);

            let listed = part.list_dir("UDATA").unwrap();
            assert_eq!(names(&listed), vec!["save.bin"], "len {len}");
            assert_eq!(listed[0].size, len as u32, "len {len}");
            let chain = if listed[0].first_cluster == 0 {
                Vec::new()
            } else {
                part.fat().chain(listed[0].first_cluster).unwrap()
            };
            assert_eq!(chain.len(), want_clusters, "len {len}");
            // The directory itself plus the file's clusters, nothing else.
            assert_eq!(
                free_count(&part),
                pristine_free - 1 - want_clusters,
                "len {len}: leaked or over-freed clusters"
            );

            let bytes = part.io().get_ref().clone();
            let mut fresh =
                FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
            let dest = tmp.path().join(format!("out{len}"));
            assert_eq!(fresh.read_tree("UDATA", &dest).unwrap(), 1);
            assert_eq!(
                fs::read(dest.join("save.bin")).unwrap(),
                pattern(len, len as u8),
                "len {len}"
            );
        }
    }

    #[test]
    fn write_creates_intermediate_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let src = tmp.path().join("src");
        host_tree(&src, &[("deep/deeper/leaf.bin", vec![0x42; 20])]);

        let mut part = open_mem(&img);
        assert_eq!(part.write_tree("UDATA/4541000D/00000001", &src).unwrap(), 1);

        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        assert_eq!(names(&part.list_dir("").unwrap()), vec!["UDATA"]);
        assert_eq!(names(&part.list_dir("UDATA").unwrap()), vec!["4541000D"]);
        assert_eq!(
            names(
                &part
                    .list_dir("UDATA/4541000D/00000001/deep/deeper")
                    .unwrap()
            ),
            vec!["leaf.bin"]
        );
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 1);
        assert_eq!(
            fs::read(dest.join("4541000D/00000001/deep/deeper/leaf.bin")).unwrap(),
            vec![0x42u8; 20]
        );
    }

    #[test]
    fn write_grows_a_directory_across_clusters() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        // 4096-byte clusters hold 64 slots, so 100 entries need two.
        let files: Vec<(String, Vec<u8>)> = (0..100)
            .map(|i| (format!("save{i:03}.bin"), vec![i as u8; 8]))
            .collect();
        let src = tmp.path().join("src");
        let refs: Vec<(&str, Vec<u8>)> =
            files.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
        host_tree(&src, &refs);

        let mut part = open_mem(&img);
        assert_eq!(part.write_tree("UDATA", &src).unwrap(), 100);

        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        let listed = part.list_dir("UDATA").unwrap();
        assert_eq!(listed.len(), 100);
        let udata = part
            .list_dir("")
            .unwrap()
            .into_iter()
            .find(|e| e.name == b"UDATA")
            .unwrap();
        assert_eq!(
            part.fat().chain(udata.first_cluster).unwrap().len(),
            2,
            "the directory itself must have grown to two clusters"
        );
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
    fn remove_tree_frees_every_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let mut part = open_mem(&img);
        let pristine_free = free_count(&part);

        let src = tmp.path().join("src");
        host_tree(
            &src,
            &[
                ("a/b/one.bin", pattern(10_000, 3)),
                ("a/b/two.bin", vec![1; 10]),
                ("a/three.bin", vec![2; 10]),
                ("four.bin", vec![3; 10]),
            ],
        );
        assert_eq!(part.write_tree("UDATA", &src).unwrap(), 4);
        assert_eq!(part.write_tree("TDATA", &src).unwrap(), 4);
        assert!(free_count(&part) < pristine_free);

        part.remove_tree("UDATA").unwrap();
        assert!(matches!(
            part.list_dir("UDATA"),
            Err(FatxError::NotADirectory(_))
        ));
        assert_eq!(names(&part.list_dir("").unwrap()), vec!["TDATA"]);

        part.remove_tree("TDATA").unwrap();
        assert_eq!(
            free_count(&part),
            pristine_free,
            "every cluster of both trees must come back"
        );
        // Removing what is not there is not an error.
        part.remove_tree("UDATA").unwrap();

        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        assert!(part.list_dir("").unwrap().is_empty());
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 0);
    }

    #[test]
    fn no_space_errors_cleanly() {
        let tmp = tempfile::tempdir().unwrap();
        // 512-byte clusters, 10240 bytes: the superblock and FAT take
        // 0x2000, leaving four addressable clusters. The root directory and
        // UDATA take one each, so two are free.
        let img = tmp.path().join("tiny.img");
        let mut b = FatxImageBuilder::new(10_240).with_cluster_size(512);
        b.add_dir("UDATA");
        b.write_to(&img).expect("build tiny image");

        let open_tiny = |bytes: Vec<u8>| {
            FatxPartition::from_io(Cursor::new(bytes), 0, 10_240).expect("open tiny image")
        };
        let mut part = open_tiny(fs::read(&img).unwrap());
        assert_eq!(part.geometry().usable_clusters, 4);
        let free_before = part.fat().free_clusters().count();
        assert_eq!(free_before, 2);

        // Ten clusters wanted, two free.
        let src = tmp.path().join("src");
        host_tree(&src, &[("huge.bin", vec![0x5A; 5_000])]);
        assert!(matches!(
            part.write_tree("UDATA", &src),
            Err(FatxError::NoSpace)
        ));

        // Nothing partially applied: the FAT never moved and no directory
        // entry names the file that did not fit.
        assert_eq!(part.fat().free_clusters().count(), free_before);
        let mut reopened = open_tiny(part.io().get_ref().clone());
        assert_eq!(reopened.fat().free_clusters().count(), free_before);
        assert!(reopened.list_dir("UDATA").unwrap().is_empty());
        let dest = tmp.path().join("out");
        assert_eq!(reopened.read_tree("UDATA", &dest).unwrap(), 0);

        // What fits still goes in, and the next write fails just as
        // cleanly with the volume now completely full.
        let fits = tmp.path().join("fits");
        host_tree(&fits, &[("small.bin", vec![0x11; 600])]);
        assert_eq!(part.write_tree("UDATA", &fits).unwrap(), 1);
        assert_eq!(part.fat().free_clusters().count(), 0);

        let more = tmp.path().join("more");
        host_tree(&more, &[("another.bin", vec![0x22; 1])]);
        assert!(matches!(
            part.write_tree("UDATA", &more),
            Err(FatxError::NoSpace)
        ));
        assert_eq!(part.fat().free_clusters().count(), 0);

        let mut reopened = open_tiny(part.into_io().into_inner());
        assert_eq!(
            names(&reopened.list_dir("UDATA").unwrap()),
            vec!["small.bin"]
        );
        let dest = tmp.path().join("out2");
        assert_eq!(reopened.read_tree("UDATA", &dest).unwrap(), 1);
        assert_eq!(fs::read(dest.join("small.bin")).unwrap(), vec![0x11u8; 600]);
    }

    #[test]
    fn absurd_nesting_stops_instead_of_blowing_the_stack() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let deep: String = (0..MAX_DEPTH + 2)
            .map(|i| format!("d{i}/"))
            .collect::<String>()
            + "leaf.bin";
        let src = tmp.path().join("src");
        host_tree(&src, &[(deep.as_str(), vec![0x01; 4])]);

        let mut part = open_mem(&img);
        assert!(matches!(
            part.write_tree("UDATA", &src),
            Err(FatxError::TooDeep(MAX_DEPTH))
        ));
        // The image is still readable, with the directories it did manage.
        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        assert_eq!(names(&part.list_dir("UDATA").unwrap()), vec!["d0"]);
    }

    /// One thing the write path did to the store, in order.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Event {
        /// A write starting at this partition offset. `fat` carries the
        /// bytes when the write landed in the FAT, so a test can read the
        /// entries as they were committed.
        Write {
            at: u64,
            len: u64,
            fat: Option<Vec<u8>>,
        },
        Barrier,
    }

    /// Backing store that records the sequence of writes and barriers, so a
    /// test can assert what was made durable before what.
    #[derive(Debug)]
    struct RecordingIo {
        inner: Cursor<Vec<u8>>,
        fat_range: Range<u64>,
        events: Vec<Event>,
    }

    impl RecordingIo {
        fn new(bytes: Vec<u8>, geo: &Geo) -> Self {
            Self {
                inner: Cursor::new(bytes),
                fat_range: geo.fat_offset..geo.data_offset,
                events: Vec::new(),
            }
        }

        /// Every FAT image committed, oldest first.
        fn fat_commits(&self) -> Vec<&Vec<u8>> {
            self.events
                .iter()
                .filter_map(|e| match e {
                    Event::Write {
                        fat: Some(bytes), ..
                    } => Some(bytes),
                    _ => None,
                })
                .collect()
        }
    }

    impl Read for RecordingIo {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Seek for RecordingIo {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    impl Write for RecordingIo {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let at = self.inner.position();
            let n = self.inner.write(buf)?;
            let fat = if self.fat_range.contains(&at) {
                Some(buf[..n].to_vec())
            } else {
                None
            };
            self.events.push(Event::Write {
                at,
                len: n as u64,
                fat,
            });
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.inner.flush()
        }
    }

    impl DurableWrite for RecordingIo {
        fn barrier(&mut self) -> std::io::Result<()> {
            self.events.push(Event::Barrier);
            Ok(())
        }
    }

    fn is_fat(event: &Event, geo: &Geo) -> bool {
        matches!(event, Event::Write { at, .. } if (geo.fat_offset..geo.data_offset).contains(at))
    }

    fn is_data(event: &Event, geo: &Geo) -> bool {
        matches!(event, Event::Write { at, .. } if *at >= geo.data_offset)
    }

    /// Read one FAT entry out of a committed FAT image.
    fn fat_entry(image: &[u8], geo: &Geo, cluster: u32) -> u32 {
        let width = geo.fat_entry_size() as usize;
        let at = cluster as usize * width;
        if width == 4 {
            u32::from_le_bytes([image[at], image[at + 1], image[at + 2], image[at + 3]])
        } else {
            u32::from(u16::from_le_bytes([image[at], image[at + 1]]))
        }
    }

    #[test]
    fn barriers_separate_data_fat_and_direntry() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        // Get UDATA in place with an ordinary write, so the recorded run is
        // one file and nothing else.
        let mut part = open_mem(&img);
        let setup = tmp.path().join("setup");
        host_tree(&setup, &[("existing.bin", vec![0x11; 32])]);
        part.write_tree("UDATA", &setup).unwrap();
        let geo = *part.geometry();
        let bytes = part.into_io().into_inner();

        let src = tmp.path().join("src");
        host_tree(&src, &[("newfile.bin", pattern(10_000, 9))]);
        let mut part = FatxPartition::from_io(RecordingIo::new(bytes, &geo), 0, PART_SIZE)
            .expect("open recording");
        assert_eq!(part.write_tree("UDATA", &src).unwrap(), 1);
        let events = part.io().events.clone();

        // Exactly one FAT commit for a file with no surplus to release.
        let fat_at: Vec<usize> = (0..events.len())
            .filter(|i| is_fat(&events[*i], &geo))
            .collect();
        assert_eq!(fat_at.len(), 1, "one FAT commit expected: {events:?}");
        let fat_at = fat_at[0];

        // The data clusters are durable before the FAT is written.
        let last_data = (0..fat_at)
            .rev()
            .find(|i| is_data(&events[*i], &geo))
            .expect("a data write before the FAT");
        assert!(
            events[last_data..fat_at].contains(&Event::Barrier),
            "no barrier between the last data write and the FAT write: {events:?}"
        );

        // The FAT is durable before the directory entry is written.
        let dirent = (fat_at + 1..events.len())
            .find(|i| is_data(&events[*i], &geo))
            .expect("the directory entry write");
        assert!(
            events[fat_at..dirent].contains(&Event::Barrier),
            "no barrier between the FAT write and the directory entry: {events:?}"
        );

        // And the image is still correct.
        let bytes = part.into_io().inner.into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 2);
        assert_eq!(
            fs::read(dest.join("newfile.bin")).unwrap(),
            pattern(10_000, 9)
        );
    }

    #[test]
    fn overwrite_to_smaller_keeps_surplus_allocated_until_the_entry_lands() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let mut part = open_mem(&img);
        let big = tmp.path().join("big");
        host_tree(&big, &[("save.bin", pattern(10_000, 1))]);
        part.write_tree("UDATA", &big).unwrap();
        let geo = *part.geometry();
        let chain = {
            let first = part.list_dir("UDATA").unwrap()[0].first_cluster;
            part.fat().chain(first).unwrap()
        };
        assert_eq!(chain.len(), 3);
        let surplus = vec![chain[1], chain[2]];
        let bytes = part.into_io().into_inner();

        // Overwrite with a single-cluster file: two clusters become surplus.
        let small = tmp.path().join("small");
        host_tree(&small, &[("save.bin", vec![0x42; 100])]);
        let mut part = FatxPartition::from_io(RecordingIo::new(bytes, &geo), 0, PART_SIZE)
            .expect("open recording");
        assert_eq!(part.write_tree("UDATA", &small).unwrap(), 1);

        let events = part.io().events.clone();
        let commits = part.io().fat_commits();
        assert_eq!(
            commits.len(),
            2,
            "shrinking needs two FAT commits, got {}",
            commits.len()
        );
        for cluster in &surplus {
            assert_ne!(
                fat_entry(commits[0], &geo, *cluster),
                0,
                "cluster {cluster} was freed before the new entry landed"
            );
            assert_eq!(
                fat_entry(commits[1], &geo, *cluster),
                0,
                "cluster {cluster} was never freed"
            );
        }
        // The entry write sits between the two commits, each fenced.
        let fat_at: Vec<usize> = (0..events.len())
            .filter(|i| is_fat(&events[*i], &geo))
            .collect();
        let dirent = (fat_at[0] + 1..fat_at[1])
            .find(|i| is_data(&events[*i], &geo))
            .expect("the directory entry between the two FAT commits");
        assert!(events[fat_at[0]..dirent].contains(&Event::Barrier));
        assert!(events[dirent..fat_at[1]].contains(&Event::Barrier));

        let bytes = part.into_io().inner.into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        let listed = part.list_dir("UDATA").unwrap();
        assert_eq!(listed[0].size, 100);
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 1);
        assert_eq!(fs::read(dest.join("save.bin")).unwrap(), vec![0x42u8; 100]);
    }

    #[test]
    fn an_invalid_dir_path_component_allocates_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let src = tmp.path().join("src");
        host_tree(&src, &[("ok.bin", vec![0x33; 16])]);

        let mut part = open_mem(&img);
        let before = part.io().get_ref().clone();
        let err = part
            .write_tree("UDATA/bad\u{ff}name", &src)
            .expect_err("an unusable path component must be refused");
        assert!(matches!(err, FatxError::InvalidName(_)), "got {err:?}");
        assert_eq!(
            part.io().get_ref(),
            &before,
            "the image must be byte-identical after a refused path"
        );
        assert!(part.list_dir("").unwrap().is_empty());
    }

    #[test]
    fn a_bad_name_reports_the_files_already_written() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        // Sorted order puts the good files first, so the failure lands with
        // progress already made.
        let src = tmp.path().join("src");
        host_tree(
            &src,
            &[
                ("aaa.bin", vec![0x01; 10]),
                ("bbb.bin", vec![0x02; 10]),
                ("zz\u{ff}bad.bin", vec![0x03; 10]),
            ],
        );

        let mut part = open_mem(&img);
        let err = part.write_tree("UDATA", &src).expect_err("bad name");
        match err {
            FatxError::PartialWrite { written, source } => {
                assert_eq!(written, 2, "the two good files landed");
                assert!(matches!(*source, FatxError::InvalidName(_)), "{source:?}");
            }
            other => panic!("expected PartialWrite, got {other:?}"),
        }

        // And what it says landed really did.
        let bytes = part.into_io().into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 2);
        assert_eq!(fs::read(dest.join("aaa.bin")).unwrap(), vec![0x01u8; 10]);
        assert_eq!(fs::read(dest.join("bbb.bin")).unwrap(), vec![0x02u8; 10]);
    }

    /// Backing store that lets every write through until one lands inside
    /// the FAT, then fails every write after it — a crash between the FAT
    /// update and the directory entry.
    #[derive(Debug)]
    struct FailAfterFatWrite {
        inner: Cursor<Vec<u8>>,
        fat: Range<u64>,
        armed: bool,
    }

    impl Read for FailAfterFatWrite {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Seek for FailAfterFatWrite {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    impl DurableWrite for FailAfterFatWrite {}

    impl Write for FailAfterFatWrite {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.armed {
                return Err(std::io::Error::other("simulated crash after the FAT write"));
            }
            let at = self.inner.position();
            let n = self.inner.write(buf)?;
            if self.fat.contains(&at) {
                self.armed = true;
            }
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.inner.flush()
        }
    }

    #[test]
    fn crash_ordering_fat_before_direntry() {
        let tmp = tempfile::tempdir().unwrap();
        let img = blank_image(tmp.path());
        let src = tmp.path().join("src");
        host_tree(&src, &[("newfile.bin", pattern(10_000, 9))]);

        // Establish UDATA first, with its own successful write.
        let mut part = open_mem(&img);
        let setup = tmp.path().join("setup");
        host_tree(&setup, &[("existing.bin", vec![0x11; 32])]);
        part.write_tree("UDATA", &setup).unwrap();
        let good = part.into_io().into_inner();
        let free_before = FatxPartition::from_io(Cursor::new(good.clone()), 0, PART_SIZE)
            .map(|p| p.fat().free_clusters().count())
            .unwrap();

        let geo = FatxPartition::from_io(Cursor::new(good.clone()), 0, PART_SIZE)
            .map(|p| *p.geometry())
            .unwrap();
        let io = FailAfterFatWrite {
            inner: Cursor::new(good),
            fat: geo.fat_offset..geo.data_offset,
            armed: false,
        };
        let mut part = FatxPartition::from_io(io, 0, PART_SIZE).expect("open");
        let err = part
            .write_tree("UDATA", &src)
            .expect_err("the write must fail");
        assert!(matches!(err, FatxError::Io(_)), "got {err:?}");

        let bytes = part.into_io().inner.into_inner();
        let mut part = FatxPartition::from_io(Cursor::new(bytes), 0, PART_SIZE).expect("reopen");
        // The FAT reached the disk, so the clusters are allocated...
        assert!(
            part.fat().free_clusters().count() < free_before,
            "the FAT write must have landed before the crash"
        );
        // ...but no directory entry names the file, which is the invariant
        // that matters: the orphan is fsck-able, a dangling entry is not.
        let listed = part.list_dir("UDATA").unwrap();
        assert_eq!(names(&listed), vec!["existing.bin"]);
        let dest = tmp.path().join("out");
        assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 1);
        assert_eq!(
            fs::read(dest.join("existing.bin")).unwrap(),
            vec![0x11u8; 32]
        );
    }
}
