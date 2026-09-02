//! Clean-room FATX support: the filesystem the original Xbox uses on its
//! hard disk, and therefore inside a raw xemu HDD image.
//!
//! Scope of this module: the `E:` (data) partition of a standard retail
//! layout, both directions. It exists so cloud save sync can pull
//! `E:/UDATA` and `E:/TDATA` out of a raw image and put them back.
//!
//! **Clean room.** Every structure here comes from public format
//! descriptions and from first principles. No FATX implementation source
//! was read, fetched, or consulted. The pages used are:
//!
//! - <https://xboxdevwiki.net/FATX> (superblock header fields)
//! - <https://xboxdevwiki.net/Hard_Drive> (retail partition table offsets)
//! - <https://free60.org/System-Software/Systems/FATX/> (directory entry
//!   layout, chainmap widths, cluster numbering, timestamp bit packing)
//!
//! **Endianness.** Original Xbox only. Every multi-byte field is
//! little-endian and the superblock magic is `FATX`. The Xbox 360 variant
//! (`XTAF`, big-endian) is out of scope.
//!
//! Module map:
//! - [`layout`] — partition constants, superblock, derived geometry
//! - [`fat`] — the cluster chain map
//! - [`dir`] — 64-byte directory entries
//! - [`image`] — [`image::FatxPartition`], the read path over a raw image
//! - [`builder`] — test-support image generator (see its module docs)
//! - `write` — the write path: `write_tree` / `remove_tree`, and the
//!   [`DurableWrite`] barrier they order their phases with
pub mod builder;
pub mod dir;
pub mod fat;
pub mod image;
pub mod layout;
mod write;

pub use write::DurableWrite;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FatxError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a FATX partition: bad signature")]
    BadSignature,
    #[error("invalid sectors-per-cluster value {0}")]
    BadClusterSize(u32),
    #[error("partition too small to hold a FATX filesystem")]
    PartitionTooSmall,
    #[error("image truncated: need {needed} bytes, file has {actual}")]
    Truncated { needed: u64, actual: u64 },
    #[error("corrupt cluster chain")]
    CorruptChain,
    #[error("cluster {0} out of range")]
    BadCluster(u32),
    #[error("name {0:?} is not a valid FATX name")]
    InvalidName(String),
    #[error("directory is full")]
    DirectoryFull,
    #[error("no free clusters left")]
    NoSpace,
    #[error("{0:?} is not a directory")]
    NotADirectory(String),
    #[error("{0:?} already exists as a file")]
    NotAFile(String),
    #[error("partition geometry changed since it was opened")]
    GeometryChanged,
    #[error("directory nesting deeper than {0} levels")]
    TooDeep(usize),
    #[error("file is {0} bytes, too large for a FATX directory entry")]
    FileTooLarge(u64),
    #[error("wrote {written} file(s), then failed: {source}")]
    PartialWrite {
        written: usize,
        source: Box<FatxError>,
    },
}
