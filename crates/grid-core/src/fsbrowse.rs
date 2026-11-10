//! One folder's listing for TV mode's folder browser (Locate…, Change
//! library folder, Relocate the library, an emulator's Program, Connect's
//! library folder). Pure listing rules over `std::fs`: dot-entries hidden,
//! folders first, then a case-insensitive sort by name; files only when the
//! browser picks a file.

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderListing {
    /// The folder listed.
    pub path: String,
    /// The folder above it ("Up one folder"), or `None` at a root.
    pub parent: Option<String>,
    pub entries: Vec<FolderEntry>,
}

/// Lists `path`. A symlink counts as what it points to; an entry that can't
/// be read (a dangling link, no permission) is left out rather than failing
/// the whole folder. Fails only when `path` itself can't be read.
pub fn list_folder(path: &Path, files: bool) -> std::io::Result<FolderListing> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::metadata(entry.path()) else {
            continue;
        };
        let is_dir = meta.is_dir();
        if is_dir || files {
            entries.push(FolderEntry { name, is_dir });
        }
    }
    sort_entries(&mut entries);
    Ok(FolderListing {
        path: path.to_string_lossy().into_owned(),
        parent: path.parent().map(|p| p.to_string_lossy().into_owned()),
        entries,
    })
}

/// Folders first, then by name ignoring case (ties by exact name, so the
/// order never depends on the directory's own order).
pub fn sort_entries(entries: &mut [FolderEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn names(listing: &FolderListing) -> Vec<(&str, bool)> {
        listing
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.is_dir))
            .collect()
    }

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("roms")).unwrap();
        fs::create_dir(dir.path().join("Games")).unwrap();
        fs::create_dir(dir.path().join(".cache")).unwrap();
        fs::write(dir.path().join("b.iso"), b"").unwrap();
        fs::write(dir.path().join("A.bin"), b"").unwrap();
        fs::write(dir.path().join(".hidden"), b"").unwrap();
        dir
    }

    #[test]
    fn folder_mode_lists_visible_folders_only_sorted_ignoring_case() {
        let dir = tree();
        let listing = list_folder(dir.path(), false).unwrap();
        assert_eq!(names(&listing), vec![("Games", true), ("roms", true)]);
        assert_eq!(listing.path, dir.path().to_string_lossy());
        assert_eq!(
            listing.parent.as_deref(),
            dir.path().parent().map(|p| p.to_string_lossy()).as_deref()
        );
    }

    #[test]
    fn file_mode_lists_folders_first_then_files() {
        let dir = tree();
        let listing = list_folder(dir.path(), true).unwrap();
        assert_eq!(
            names(&listing),
            vec![
                ("Games", true),
                ("roms", true),
                ("A.bin", false),
                ("b.iso", false)
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_counts_as_its_target_and_a_dangling_one_is_left_out() {
        let dir = tree();
        std::os::unix::fs::symlink(dir.path().join("roms"), dir.path().join("linked")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("gone"), dir.path().join("dangling")).unwrap();
        let listing = list_folder(dir.path(), true).unwrap();
        assert!(names(&listing).contains(&("linked", true)));
        assert!(!names(&listing).iter().any(|(n, _)| *n == "dangling"));
    }

    #[test]
    fn a_root_has_no_parent() {
        let root = if cfg!(windows) {
            Path::new("C:\\")
        } else {
            Path::new("/")
        };
        if let Ok(listing) = list_folder(root, false) {
            assert_eq!(listing.parent, None);
        }
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_folder(&dir.path().join("nope"), false).is_err());
    }

    #[test]
    fn ties_in_case_fall_back_to_the_exact_name() {
        let mut entries = vec![
            FolderEntry {
                name: "b".into(),
                is_dir: false,
            },
            FolderEntry {
                name: "B".into(),
                is_dir: false,
            },
            FolderEntry {
                name: "a".into(),
                is_dir: true,
            },
        ];
        sort_entries(&mut entries);
        let order: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(order, vec!["a", "B", "b"]);
    }
}
