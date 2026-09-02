//! Integration test: a FATX `E:` partition at the retail offset inside a
//! sparse image file behaves the same as one at offset 0.

use std::fs;

use grid_core::fatx::builder::FatxImageBuilder;
use grid_core::fatx::image::FatxPartition;
use grid_core::fatx::layout::{RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE};
use grid_core::fatx::FatxError;

#[test]
fn retail_offset_integration() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("xbox_hdd.img");
    let part_size = 16 * 1024 * 1024;

    let mut b = FatxImageBuilder::new(part_size)
        .with_base_offset(RETAIL_PARTITION_E_OFFSET)
        .with_cluster_size(16384);
    b.add_dir("UDATA/4d530064/00000001");
    b.add_file(
        "UDATA/4d530064/00000001/saveimage.xbx",
        vec![0x5Au8; 40_000],
    );
    b.add_file("TDATA/4d530064/settings.bin", b"cfg".to_vec());
    b.write_to(&img).expect("build sparse retail image");

    let meta = fs::metadata(&img).unwrap();
    assert_eq!(meta.len(), RETAIL_PARTITION_E_OFFSET + part_size);

    // The 2.8 GB before the partition must stay a hole: the builder uses
    // `File::set_len`, so the image costs a few hundred KB on disk.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let allocated = meta.blocks() * 512;
        assert!(
            allocated < 64 * 1024 * 1024,
            "image is not sparse: {allocated} bytes allocated"
        );
    }

    FatxPartition::validate(&img, RETAIL_PARTITION_E_OFFSET, part_size)
        .expect("validate at retail offset");

    // The sniffer passes the size it expects. Asking for a full retail E:
    // partition from a 16 MiB one reports the shortfall instead of reading
    // a smaller filesystem by accident.
    assert!(matches!(
        FatxPartition::validate(&img, RETAIL_PARTITION_E_OFFSET, RETAIL_PARTITION_E_SIZE),
        Err(FatxError::Truncated { .. })
    ));

    let mut part =
        FatxPartition::open(&img, RETAIL_PARTITION_E_OFFSET, part_size).expect("open at offset");
    let dest = tmp.path().join("out");
    assert_eq!(part.read_tree("UDATA", &dest.join("UDATA")).unwrap(), 1);
    assert_eq!(part.read_tree("TDATA", &dest.join("TDATA")).unwrap(), 1);
    assert_eq!(
        fs::read(dest.join("UDATA/4d530064/00000001/saveimage.xbx")).unwrap(),
        vec![0x5Au8; 40_000]
    );
    assert_eq!(
        fs::read(dest.join("TDATA/4d530064/settings.bin")).unwrap(),
        b"cfg"
    );

    // The region before the partition is untouched (and stays a hole), so
    // there is no superblock at offset 0.
    assert!(FatxPartition::validate(&img, 0, part_size).is_err());
}

/// The write path at the retail offset: put a tree back, take it out
/// again, and leave the 2.8 GB before the partition a hole.
#[test]
fn retail_offset_write_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let img = tmp.path().join("xbox_hdd.img");
    let part_size = 16 * 1024 * 1024;
    FatxImageBuilder::new(part_size)
        .with_base_offset(RETAIL_PARTITION_E_OFFSET)
        .with_cluster_size(16384)
        .write_to(&img)
        .expect("build sparse retail image");

    let big: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();
    let src = tmp.path().join("src");
    for (rel, data) in [
        ("4d530064/00000001/saveimage.xbx", big.clone()),
        ("4d530064/00000001/savemeta.xbx", b"meta".to_vec()),
        ("notes.txt", b"hello xbox".to_vec()),
    ] {
        let target = src.join(rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, data).unwrap();
    }

    let mut part = FatxPartition::open_rw(&img, RETAIL_PARTITION_E_OFFSET, part_size)
        .expect("open for writing at the retail offset");
    assert_eq!(part.write_tree("UDATA", &src).expect("write_tree"), 3);
    assert_eq!(part.write_tree("TDATA", &src).expect("write_tree"), 3);
    drop(part);

    // Nothing before the partition was touched.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let allocated = fs::metadata(&img).unwrap().blocks() * 512;
        assert!(
            allocated < 64 * 1024 * 1024,
            "image is not sparse after writing: {allocated} bytes allocated"
        );
    }
    assert!(FatxPartition::validate(&img, 0, part_size).is_err());

    FatxPartition::validate(&img, RETAIL_PARTITION_E_OFFSET, part_size)
        .expect("still a valid partition after writing");
    let mut part = FatxPartition::open(&img, RETAIL_PARTITION_E_OFFSET, part_size).expect("reopen");
    let dest = tmp.path().join("out");
    assert_eq!(part.read_tree("UDATA", &dest).unwrap(), 3);
    assert_eq!(
        fs::read(dest.join("4d530064/00000001/saveimage.xbx")).unwrap(),
        big
    );
    assert_eq!(fs::read(dest.join("notes.txt")).unwrap(), b"hello xbox");

    // And removal gives every cluster back.
    let mut part = FatxPartition::open_rw(&img, RETAIL_PARTITION_E_OFFSET, part_size)
        .expect("open for removal");
    let before = part.fat().free_clusters().count();
    part.remove_tree("UDATA").expect("remove_tree");
    part.remove_tree("TDATA").expect("remove_tree");
    assert!(part.fat().free_clusters().count() > before);
    drop(part);

    let mut part = FatxPartition::open(&img, RETAIL_PARTITION_E_OFFSET, part_size)
        .expect("reopen after remove");
    assert!(part.list_dir("").unwrap().is_empty());
    assert_eq!(
        part.read_tree("UDATA", &tmp.path().join("out2")).unwrap(),
        0
    );
}
