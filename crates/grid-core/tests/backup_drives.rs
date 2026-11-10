//! The e2e drive-roots override: every child folder of a root is a drive,
//! and `.e2e-drive.json` fakes what the OS would report. Run with
//! `cargo test -p grid-core --features e2e`.
//!
//! `std::env::set_var` is process-global. One test in one binary means no
//! other test can race it.

#![cfg(feature = "e2e")]

use grid_core::backup::drives::{self, DRIVE_ROOTS_ENV, FAKE_DRIVE_FILE};

#[test]
fn the_override_root_gives_one_drive_per_child_folder() {
    let root = tempfile::tempdir().unwrap();
    let couch = root.path().join("COUCH");
    let stick = root.path().join("STICK");
    std::fs::create_dir(&couch).unwrap();
    std::fs::create_dir(&stick).unwrap();
    std::fs::write(
        stick.join(FAKE_DRIVE_FILE),
        r#"{ "label": "Old stick", "fs": "vfat", "total_bytes": 8000000000 }"#,
    )
    .unwrap();
    std::fs::write(stick.join("photo.jpg"), vec![0u8; 1000]).unwrap();
    // Neither a dot-folder nor a plain file is a drive.
    std::fs::create_dir(root.path().join(".unplugged")).unwrap();
    std::fs::write(root.path().join("notes.txt"), b"x").unwrap();

    std::env::set_var(DRIVE_ROOTS_ENV, root.path());
    let found = drives::scan();

    assert_eq!(found.len(), 2, "{found:?}");
    let (first, second) = (&found[0], &found[1]);
    assert_eq!(first.mount, couch);
    assert_eq!(first.label, "COUCH");
    assert_eq!(first.fs, "exfat");
    assert_eq!(first.fs_display, "exFAT");
    assert!(!first.fat32);
    assert!(first.total_bytes > 0);
    assert!(!first.read_only);

    assert_eq!(second.mount, stick);
    assert_eq!(second.key(), stick.to_string_lossy());
    assert_eq!(second.label, "Old stick");
    assert_eq!(second.fs, "vfat");
    assert_eq!(second.fs_display, "FAT32");
    assert!(second.fat32);
    assert_eq!(second.total_bytes, 8_000_000_000);
    assert!(second.free_bytes <= 8_000_000_000 - 1000);

    // Unplugging is a rename out of the root's visible children.
    let before: Vec<String> = found.iter().map(|drive| drive.key()).collect();
    std::fs::rename(&stick, root.path().join(".STICK")).unwrap();
    let after: Vec<String> = drives::scan().iter().map(|drive| drive.key()).collect();
    let change = drives::diff(&before, &after);
    assert_eq!(change.removed, vec![stick.to_string_lossy().into_owned()]);
    assert!(change.appeared.is_empty());

    std::env::remove_var(DRIVE_ROOTS_ENV);
}
