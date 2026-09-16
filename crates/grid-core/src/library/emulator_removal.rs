//! Removing a managed emulator install's files.
//!
//! Deleting an emulator entry from the Emulators page removes its config
//! entry; this module removes the bytes that entry owns. Only a MANAGED
//! install is touched: a directory one level below `<library>/Emulators`,
//! which is where [`crate::launch::emu_install::emulator_install_dir`] and
//! `install_manual_archive` put everything they extract. A hand-configured
//! path (`/usr/bin/retroarch`, `~/Applications/Foo.AppImage`) is never
//! removed, and neither is a directory two entries share — Dolphin installs
//! one binary but is recorded as "Dolphin (GameCube)" and "Dolphin (Wii)".

use std::path::{Path, PathBuf};

use super::paths::{expand_home, library_root};
use super::{apply_removal, run_removals, LibraryError, Removal, RemovalLabel};
use crate::config::{Config, EmulatorEntry};

/// The removal plan for `entry`: at most one [`RemovalLabel::Folder`] step
/// for the `<library>/Emulators/<X>` directory the entry's executable lives
/// in.
///
/// `others` is every config entry — the one being deleted is skipped by
/// name (case-insensitively), so the caller can pass the whole list. Any
/// other entry resolving into the same `<X>` cancels the plan: its files
/// would go with it.
///
/// An empty plan is the normal answer for anything unmanaged: a missing
/// executable, a path outside `<library>/Emulators/`, or `Emulators` itself.
pub(crate) fn emulator_removal_steps(
    entry: &EmulatorEntry,
    others: &[EmulatorEntry],
    library: &Path,
) -> Vec<Removal> {
    let root = emulators_root(library);
    let Some(target) = managed_install_dir(entry, &root) else {
        return Vec::new();
    };
    let folded = entry.name.trim().to_lowercase();
    for other in others {
        if other.name.trim().to_lowercase() == folded {
            continue;
        }
        if managed_install_dir(other, &root).is_some_and(|dir| dir == target) {
            return Vec::new();
        }
    }
    vec![Removal {
        path: target,
        label: RemovalLabel::Folder,
    }]
}

/// Removes the files of the emulator named `name`, leaving the config entry
/// alone (the caller edits the config afterwards, so a failed removal leaves
/// the entry in place to retry — the same order [`super::InstallService::uninstall`]
/// uses for a game).
///
/// Succeeds with nothing to do when no entry carries the name or when no
/// library folder is configured: nothing managed can exist in either case.
/// Every step is attempted; the failures come back as one error listing them
/// all (D11).
pub fn remove_emulator_files(config: &Config, name: &str) -> Result<(), LibraryError> {
    let folded = name.trim().to_lowercase();
    let Some(entry) = config
        .emulators
        .iter()
        .find(|e| e.name.trim().to_lowercase() == folded)
    else {
        return Ok(());
    };
    let Some(library) = library_root(config) else {
        return Ok(());
    };
    let steps = emulator_removal_steps(entry, &config.emulators, &library);
    let failures = run_removals(&steps, &mut apply_removal);
    if !failures.is_empty() {
        return Err(LibraryError::Registry(failures.join("\n")));
    }
    Ok(())
}

// --- internals --------------------------------------------------------------

/// `<library>/Emulators`, canonicalized when it exists so it compares equal
/// to a canonicalized entry path.
fn emulators_root(library: &Path) -> PathBuf {
    let root = library.join("Emulators");
    root.canonicalize().unwrap_or(root)
}

/// The first directory level below `root` that contains `entry`'s
/// executable, or `None` when the executable is missing, blank, or not
/// strictly inside `root`.
fn managed_install_dir(entry: &EmulatorEntry, root: &Path) -> Option<PathBuf> {
    let dir = resolved_entry_dir(entry)?;
    let relative = dir.strip_prefix(root).ok()?;
    let first = relative.components().next()?;
    Some(root.join(first))
}

/// The canonical directory `entry`'s path points at: the path itself when it
/// is a directory, its parent when it is a file. `None` when the path is
/// blank or does not exist.
fn resolved_entry_dir(entry: &EmulatorEntry) -> Option<PathBuf> {
    let raw = entry.path.trim();
    if raw.is_empty() {
        return None;
    }
    let resolved = expand_home(raw).canonicalize().ok()?;
    if resolved.is_dir() {
        return Some(resolved);
    }
    resolved.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn entry(name: &str, path: &Path) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string_lossy().into_owned(),
            ..Default::default()
        }
    }

    fn raw_entry(name: &str, path: &str) -> EmulatorEntry {
        EmulatorEntry {
            name: name.to_string(),
            path: path.to_string(),
            ..Default::default()
        }
    }

    /// Creates `<library>/Emulators/<install>/<relative>` as an empty file
    /// and returns it.
    fn touch_install(library: &Path, install: &str, relative: &str) -> PathBuf {
        let file = library.join("Emulators").join(install).join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"stub").unwrap();
        file
    }

    fn folder_steps(steps: &[Removal]) -> Vec<&Path> {
        steps
            .iter()
            .map(|s| {
                assert_eq!(s.label, RemovalLabel::Folder);
                s.path.as_path()
            })
            .collect()
    }

    /// The install directory as the planner reports it: canonicalized, so a
    /// symlinked temp root (`/tmp` on some systems) still compares equal.
    fn expected_dir(library: &Path, install: &str) -> PathBuf {
        library
            .join("Emulators")
            .join(install)
            .canonicalize()
            .unwrap()
    }

    // (a)
    #[test]
    fn a_catalog_install_yields_its_install_directory() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "PCSX2 (Playstation 2)-latest", "pcsx2-qt");

        let steps = emulator_removal_steps(&entry("PCSX2 (Playstation 2)", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "PCSX2 (Playstation 2)-latest").as_path()]
        );
    }

    // (b)
    #[test]
    fn an_appimage_kept_in_place_yields_its_install_directory() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "Cemu (Wii U)-latest", "Cemu-2.6-x86_64.AppImage");

        let steps = emulator_removal_steps(&entry("Cemu (Wii U)", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "Cemu (Wii U)-latest").as_path()]
        );
    }

    // (c)
    #[test]
    fn a_nested_executable_still_yields_the_top_install_directory() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "X", "bin/foo");

        let steps = emulator_removal_steps(&entry("X", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "X").as_path()]
        );
    }

    // (d)
    #[test]
    fn an_unmanaged_path_yields_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        // Something managed has to exist, or `<library>/Emulators` would be
        // missing and every case below would pass for the wrong reason.
        touch_install(library, "Keep", "keep");

        let system = raw_entry("RetroArch", "/usr/bin/retroarch");
        assert!(emulator_removal_steps(&system, &[], library).is_empty());

        let elsewhere = library.join("SomethingElse").join("foo");
        fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
        fs::write(&elsewhere, b"stub").unwrap();
        assert!(emulator_removal_steps(&entry("Elsewhere", &elsewhere), &[], library).is_empty());

        let root = library.join("Emulators");
        assert!(
            emulator_removal_steps(&entry("Root", &root), &[], library).is_empty(),
            "the Emulators root itself is never a removal target"
        );
    }

    // (e)
    #[test]
    fn a_tilde_path_is_expanded_before_it_is_matched() {
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let _env = crate::test_env::EnvGuard::set(&[("HOME", Some(&home.to_string_lossy()))]);

        let library = home.join("GRID");
        touch_install(&library, "Redream-nightly", "redream");

        let tilde = raw_entry("Redream", "~/GRID/Emulators/Redream-nightly/redream");
        let steps = emulator_removal_steps(&tilde, &[], &library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(&library, "Redream-nightly").as_path()]
        );
    }

    // (f)
    #[test]
    fn two_entries_sharing_one_install_directory_remove_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "Dolphin-latest", "dolphin-emu");
        let entries = vec![
            entry("Dolphin (GameCube)", &exe),
            entry("Dolphin (Wii)", &exe),
        ];

        assert!(emulator_removal_steps(&entries[0], &entries, library).is_empty());
        assert!(emulator_removal_steps(&entries[1], &entries, library).is_empty());
    }

    // (g)
    #[test]
    fn a_missing_executable_yields_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        touch_install(library, "Keep", "keep");
        let gone = library.join("Emulators").join("Gone-latest").join("gone");

        assert!(emulator_removal_steps(&entry("Gone", &gone), &[], library).is_empty());
        assert!(emulator_removal_steps(&raw_entry("Blank", "  "), &[], library).is_empty());
    }

    // (h)
    #[test]
    fn remove_emulator_files_deletes_the_tree_and_leaves_siblings_alone() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let pcsx2 = touch_install(library, "PCSX2-latest", "pcsx2-qt");
        let redream = touch_install(library, "Redream-nightly", "redream");
        let config = Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators: vec![entry("PCSX2", &pcsx2), entry("Redream", &redream)],
            ..Default::default()
        };

        remove_emulator_files(&config, "pcsx2").unwrap();

        assert!(!library.join("Emulators").join("PCSX2-latest").exists());
        assert!(redream.is_file(), "the other install must survive");
        assert_eq!(
            config.emulators.len(),
            2,
            "the config edit belongs to the caller"
        );
    }

    // (i)
    #[test]
    fn a_blank_library_or_an_unknown_name_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "PCSX2-latest", "pcsx2-qt");

        let no_library = Config {
            library_path: String::new(),
            emulators: vec![entry("PCSX2", &exe)],
            ..Default::default()
        };
        remove_emulator_files(&no_library, "PCSX2").unwrap();
        assert!(exe.is_file());

        let config = Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators: vec![entry("PCSX2", &exe)],
            ..Default::default()
        };
        remove_emulator_files(&config, "Nothing Like This").unwrap();
        assert!(exe.is_file());
    }
}
