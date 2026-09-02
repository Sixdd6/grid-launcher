//! Integration test: a FATX `E:` partition at the retail offset inside a
//! sparse image file behaves the same as one at offset 0.

use std::fs;

use grid_core::fatx::builder::FatxImageBuilder;
use grid_core::fatx::image::FatxPartition;
use grid_core::fatx::layout::RETAIL_PARTITION_E_OFFSET;

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

    FatxPartition::validate(&img, RETAIL_PARTITION_E_OFFSET).expect("validate at retail offset");

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

    // The region before the partition is untouched (and stays a hole).
    assert!(!FatxPartition::validate(&img, 0).is_ok());
}
