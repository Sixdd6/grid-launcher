use grid_core::images::cache::image_key;
use grid_core::images::sweep::{pinned_keys, sweep, SweepReport};
use std::collections::HashSet;
use std::fs;
use std::time::{Duration, SystemTime};

fn write(dir: &std::path::Path, name: &str, size: usize, age_secs: u64) {
    let p = dir.join(name);
    fs::write(&p, vec![0u8; size]).unwrap();
    let t = SystemTime::now() - Duration::from_secs(age_secs);
    fs::File::options()
        .write(true)
        .open(&p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

#[test]
fn under_cap_deletes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.png", 100, 10);
    write(dir.path(), "b.jpg", 100, 20);
    let r = sweep(dir.path(), 1000, &HashSet::new());
    assert_eq!(
        r,
        SweepReport {
            total_before: 200,
            total_after: 200,
            deleted: 0,
            stale_parts: 0
        }
    );
}

#[test]
fn over_cap_deletes_oldest_unpinned_until_under_cap() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "old.png", 100, 300);
    write(dir.path(), "mid.png", 100, 200);
    write(dir.path(), "new.png", 100, 100);
    let r = sweep(dir.path(), 150, &HashSet::new());
    assert_eq!(r.deleted, 2);
    assert_eq!(r.total_after, 100);
    assert!(!dir.path().join("old.png").exists());
    assert!(!dir.path().join("mid.png").exists());
    assert!(dir.path().join("new.png").exists());
}

#[test]
fn pinned_files_survive_even_above_cap() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://h/assets/pinned.png");
    write(dir.path(), &format!("{key}.png"), 100, 300);
    write(dir.path(), "loose.png", 100, 100);
    let pinned = pinned_keys(["/assets/pinned.png"], "https://h");
    let r = sweep(dir.path(), 50, &pinned);
    assert_eq!(r.deleted, 1);
    assert!(dir.path().join(format!("{key}.png")).exists());
    assert!(!dir.path().join("loose.png").exists());
}

#[test]
fn stale_part_files_are_removed_and_fresh_ones_kept() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "stale.part", 10, 7200);
    write(dir.path(), "fresh.part", 10, 10);
    let r = sweep(dir.path(), 1000, &HashSet::new());
    assert_eq!(r.stale_parts, 1);
    assert!(!dir.path().join("stale.part").exists());
    assert!(dir.path().join("fresh.part").exists());
    assert_eq!(r.total_before, 0); // .part files never count toward the total
}

#[test]
fn pinned_keys_skips_empty_and_foreign_hosts() {
    let pinned = pinned_keys(["", "/a.png", "https://other/b.png"], "https://h");
    assert_eq!(pinned.len(), 1);
    assert!(pinned.contains(&image_key("https://h/a.png")));
}

#[test]
fn missing_dir_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let r = sweep(&dir.path().join("nope"), 10, &HashSet::new());
    assert_eq!(r, SweepReport::default());
}

/// `<key>.bg12.jpg` has the file stem `<key>.bg12`, so pinning by whole stem
/// would evict every background variant while keeping its source.
#[test]
fn a_background_variant_is_pinned_with_its_source() {
    let dir = tempfile::tempdir().unwrap();
    let key = image_key("https://h/assets/cover.png");
    write(dir.path(), &format!("{key}.png"), 4096, 300);
    write(dir.path(), &format!("{key}.bg12.jpg"), 4096, 300);
    let victim = image_key("https://h/assets/other.png");
    write(dir.path(), &format!("{victim}.png"), 8192, 100);

    let pinned = pinned_keys(["/assets/cover.png"], "https://h");
    // A cap below the total forces the sweep to delete something.
    let report = sweep(dir.path(), 8192, &pinned);

    assert!(dir.path().join(format!("{key}.png")).exists());
    assert!(dir.path().join(format!("{key}.bg12.jpg")).exists());
    assert!(!dir.path().join(format!("{victim}.png")).exists());
    assert_eq!(report.deleted, 1);
}

/// A fanart-sourced variant lives under the FANART's key, not the cover's, so
/// the startup pin set has to include `background_source_url`'s answer or the
/// sweep evicts the variant and its source together every start.
#[test]
fn a_fanart_sourced_background_is_pinned_when_its_own_key_is_in_the_set() {
    use grid_core::images::replenish::background_source_url;
    let dir = tempfile::tempdir().unwrap();
    let row = grid_core::library::registry::InstalledGame {
        title: "G".to_string(),
        platform: "SNES".to_string(),
        rom_id: Some(1),
        cover_small_path: "/assets/1.png".to_string(),
        cover_large_path: "/assets/1l.png".to_string(),
        fanart_urls: "https://h/assets/1f.png".to_string(),
        ..Default::default()
    };
    let fanart = background_source_url(&row, "https://h");
    assert_eq!(fanart, "https://h/assets/1f.png");
    let fanart_key = image_key(&fanart);
    write(dir.path(), &format!("{fanart_key}.png"), 4096, 300);
    write(dir.path(), &format!("{fanart_key}.bg12.jpg"), 4096, 300);
    write(dir.path(), "loose.png", 8192, 100);

    // Covers alone: the fanart source and its variant are the oldest, so both
    // are evicted.
    let covers_only = pinned_keys(["/assets/1.png", "/assets/1l.png"], "https://h");
    let report = sweep(dir.path(), 8192, &covers_only);
    assert_eq!(report.deleted, 2);
    assert!(!dir.path().join(format!("{fanart_key}.bg12.jpg")).exists());

    // With the background source in the set, both survive.
    write(dir.path(), &format!("{fanart_key}.png"), 4096, 300);
    write(dir.path(), &format!("{fanart_key}.bg12.jpg"), 4096, 300);
    let with_background = pinned_keys(
        ["/assets/1.png", "/assets/1l.png", fanart.as_str()],
        "https://h",
    );
    let report = sweep(dir.path(), 8192, &with_background);
    assert!(dir.path().join(format!("{fanart_key}.png")).exists());
    assert!(dir.path().join(format!("{fanart_key}.bg12.jpg")).exists());
    assert!(!dir.path().join("loose.png").exists());
    assert_eq!(report.deleted, 1);
}
