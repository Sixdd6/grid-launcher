//! Removing a managed emulator install's files.
//!
//! Deleting an emulator entry from the Emulators page removes its config
//! entry; this module removes the bytes that entry owns. Only a MANAGED
//! install is touched: a directory one level below `<library>/emulators` or
//! the legacy `<library>/Emulators`, which is where
//! [`crate::launch::emu_install::emulator_install_dir`] and
//! `install_manual_archive` put everything they extract — the new root is
//! checked first, the legacy one second, so an unmigrated library's installs
//! are still found. A hand-configured path (`/usr/bin/retroarch`,
//! `~/Applications/Foo.AppImage`) is never removed, and neither is a
//! directory two entries share — Dolphin installs one binary but is recorded
//! as "Dolphin (GameCube)" and "Dolphin (Wii)".

use std::fs;
use std::path::{Path, PathBuf};

use super::extract::is_extractable_archive;
use super::paths::{emulators_dir, expand_home, legacy_emulators_dir, library_root, saves_dir};
use super::user_data_links::{move_file, move_tree_preferring_dest};
use super::{apply_removal, run_removals, LibraryError, Removal, RemovalLabel};
use crate::autoconfig::paths::is_appimage;
use crate::cloud::dirs::{
    resolved_screenshot_directories, resolved_sync_directory_paths, PathKey, ResolveContext,
};
use crate::config::{Config, EmulatorEntry};
use crate::launch::profiles::{profile_for_entry, EmulatorProfile};

/// The removal plan for `entry`: at most one [`RemovalLabel::Folder`] step
/// for the `<root>/<X>` directory the entry's executable lives in, under
/// whichever of [`emulator_roots`] holds it.
///
/// `others` is every config entry — the one being deleted is skipped by
/// name (case-insensitively), so the caller can pass the whole list. Any
/// other entry resolving into the same `<X>` cancels the plan: its files
/// would go with it.
///
/// An empty plan is the normal answer for anything unmanaged: a missing
/// executable, a path outside both emulator roots, or a root directory
/// itself.
pub(crate) fn emulator_removal_steps(
    entry: &EmulatorEntry,
    others: &[EmulatorEntry],
    library: &Path,
) -> Vec<Removal> {
    let roots = emulator_roots(library);
    let Some(target) = managed_install_dir(entry, &roots) else {
        return Vec::new();
    };
    let folded = entry.name.trim().to_lowercase();
    for other in others {
        if other.name.trim().to_lowercase() == folded {
            continue;
        }
        if managed_install_dir(other, &roots).is_some_and(|dir| dir == target) {
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
/// User data still living inside the install directory is salvaged into
/// `<library>/saves/<Emulator>` FIRST ([`salvage_user_data`]); a salvage
/// failure aborts before anything is deleted.
///
/// Succeeds with nothing to do when no entry carries the name or when no
/// library folder is configured: nothing managed can exist in either case.
/// Every removal step is attempted; the failures come back as one error
/// listing them all (D11).
pub fn remove_emulator_files(
    config: &Config,
    config_path: &Path,
    name: &str,
    profiles: &[EmulatorProfile],
) -> Result<(), LibraryError> {
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
    if steps.is_empty() {
        // Nothing managed is being removed, so nothing inside it is about
        // to be lost: salvaging here would move a live install's saves out
        // from under it.
        return Ok(());
    }

    if let Some(install_dir) = managed_install_dir_for(entry, &library) {
        let profile = profile_for_entry(&entry.name, &entry.path, profiles);
        let emulator = profile.map_or(entry.name.as_str(), |p| p.name.as_str());
        let destination = saves_dir(&library, emulator);
        let emulator_dir = emulator_dir_for(entry, profiles);
        let config_dir = config_path.parent().unwrap_or(Path::new("."));
        let ctx = ResolveContext {
            emulator_dir: emulator_dir.as_deref(),
            library_dir: &config.library_path,
            config_dir,
            windows_documents: None,
            retroarch_portable_home: None,
        };
        let salvaged = salvage_user_data(entry, &install_dir, &destination, profile, &ctx)?;
        if !salvaged.is_empty() {
            tracing::info!(
                install_dir = %install_dir.display(),
                destinations = %display_paths(&salvaged),
                "salvaged emulator user data before removal"
            );
        }
    }

    let failures = run_removals(&steps, &mut apply_removal);
    if !failures.is_empty() {
        return Err(LibraryError::Registry(failures.join("\n")));
    }
    Ok(())
}

/// Moves whatever user data still sits inside `install_dir` under
/// `saves_dir`, so deleting the install directory afterwards cannot take it
/// along. Returns the destinations, for the caller's log line.
///
/// The save, state and screenshot directories are resolved with the same
/// cloud-save resolution the sync uses, so every per-emulator quirk (a
/// RetroArch config override, a Dolphin ini) is accounted for. Each result
/// is fully resolved, which decides what happens to it:
///
/// - strictly inside `install_dir`: the whole directory moves to
///   `saves_dir/<relative>` (a rename when the destination is absent, a
///   destination-wins merge otherwise). `<relative>` is measured from the
///   emulator's DATA root when that root is itself inside the install
///   directory — PCSX2's AppImage, whose `PCSX2/memcards` lands at
///   `saves/<E>/memcards`, the same name a reinstall links;
/// - EQUAL to `install_dir` (a profile with a `"."` directory, Redream):
///   only the top-level regular files move, minus the entry's own
///   executable, anything [`is_extractable_archive`] recognizes, and
///   `.AppImage` files (Decision 17);
/// - anywhere else: skipped. A directory reached through a link resolves to
///   its target, and a user data link always points under `saves/`, so a
///   linked install's directory lands here and is left alone. Nothing here
///   tests for a link: what decides is where the candidate resolves, so a
///   link whose target DID sit inside the install directory would be moved
///   whole, exactly like a real directory.
pub(crate) fn salvage_user_data(
    entry: &EmulatorEntry,
    install_dir: &Path,
    saves_dir: &Path,
    profile: Option<&EmulatorProfile>,
    ctx: &ResolveContext,
) -> Result<Vec<PathBuf>, LibraryError> {
    let root = fs::canonicalize(install_dir).unwrap_or_else(|_| install_dir.to_path_buf());
    // PCSX2's AppImage keeps everything one level deeper, in
    // `<exe dir>/PCSX2`. When that data root sits inside the install
    // directory, a salvaged directory keeps its path BELOW the root
    // (`PCSX2/memcards` -> `saves/<E>/memcards`), so the links a reinstall
    // creates at the data root find the same bytes again.
    let data_root = profile
        .and_then(|p| crate::autoconfig::emulator_data_root(entry, std::slice::from_ref(p)))
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .filter(|candidate| *candidate != root && candidate.starts_with(&root));
    let mut candidates: Vec<PathBuf> = Vec::new();
    for key in [PathKey::SavePaths, PathKey::StatePaths] {
        let (directories, _files) = resolved_sync_directory_paths(entry, profile, key, ctx);
        candidates.extend(directories);
    }
    candidates.extend(resolved_screenshot_directories(entry, profile, ctx));

    let mut seen: Vec<PathBuf> = Vec::new();
    let mut salvaged: Vec<PathBuf> = Vec::new();
    for candidate in candidates {
        let Ok(resolved) = fs::canonicalize(&candidate) else {
            continue;
        };
        if !resolved.is_dir() || seen.contains(&resolved) {
            continue;
        }
        seen.push(resolved.clone());

        if resolved == root {
            salvaged.extend(salvage_loose_files(entry, &root, saves_dir)?);
            continue;
        }
        let relative = data_root
            .as_deref()
            .and_then(|base| resolved.strip_prefix(base).ok())
            .or_else(|| resolved.strip_prefix(&root).ok());
        if let Some(relative) = relative {
            let destination = saves_dir.join(relative);
            move_directory(&resolved, &destination)?;
            salvaged.push(destination);
        }
    }
    Ok(salvaged)
}

// --- internals --------------------------------------------------------------

/// The two roots a managed emulator install may live under: `emulators/`
/// (v1 layout) first, then the legacy `Emulators/`. Each is canonicalized
/// when it exists, so it compares equal to a canonicalized entry path.
fn emulator_roots(library: &Path) -> [PathBuf; 2] {
    let canon = |root: PathBuf| root.canonicalize().unwrap_or(root);
    [
        canon(emulators_dir(library)),
        canon(legacy_emulators_dir(library)),
    ]
}

/// The first directory level below whichever of `roots` contains `entry`'s
/// executable, or `None` when the executable is missing, blank, or not
/// strictly inside either root.
fn managed_install_dir(entry: &EmulatorEntry, roots: &[PathBuf; 2]) -> Option<PathBuf> {
    let dir = resolved_entry_dir(entry)?;
    for root in roots {
        if let Ok(relative) = dir.strip_prefix(root) {
            if let Some(first) = relative.components().next() {
                return Some(root.join(first));
            }
        }
    }
    None
}

/// The canonical `<root>/<X>` directory `entry`'s executable lives under —
/// the same lookup [`emulator_removal_steps`] plans a removal for, exposed
/// separately for the salvage step.
pub(crate) fn managed_install_dir_for(entry: &EmulatorEntry, library: &Path) -> Option<PathBuf> {
    managed_install_dir(entry, &emulator_roots(library))
}

/// The `%EMULATOR_DIR%` the cloud resolution expands against
/// (`cloud::ops::emulator_dir_for`): the entry's DATA root, which is its
/// executable's parent for every emulator but the PCSX2 AppImage
/// (`<exe dir>/PCSX2`). `None` for a blank path.
fn emulator_dir_for(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> Option<PathBuf> {
    crate::autoconfig::emulator_data_root(entry, profiles)
}

/// The top-level regular files of `root` worth keeping: everything except
/// the entry's own executable, an archive [`is_extractable_archive`]
/// recognizes (the downloaded release, left beside the install) and an
/// `.AppImage` (the emulator itself). Each one moves to `saves_dir/<name>`;
/// a name already present there wins, matching
/// [`move_tree_preferring_dest`].
fn salvage_loose_files(
    entry: &EmulatorEntry,
    root: &Path,
    saves_dir: &Path,
) -> Result<Vec<PathBuf>, LibraryError> {
    let executable = expand_home(entry.path.trim()).canonicalize().ok();
    let mut salvaged = Vec::new();
    for item in fs::read_dir(root)? {
        let item = item?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let path = item.path();
        if executable.as_deref() == Some(path.as_path())
            || is_extractable_archive(&path)
            || is_appimage(&path)
        {
            continue;
        }
        let destination = saves_dir.join(item.file_name());
        if destination.exists() {
            continue;
        }
        fs::create_dir_all(saves_dir)?;
        move_file(&path, &destination)?;
        salvaged.push(destination);
    }
    Ok(salvaged)
}

/// Moves `src` to `dest`: a rename while `dest` is still absent, otherwise
/// (and whenever the rename fails, e.g. across filesystems) the
/// destination-wins merge the install path already uses.
fn move_directory(src: &Path, dest: &Path) -> Result<(), LibraryError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if !dest.exists() && fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    move_tree_preferring_dest(src, dest)
}

/// The salvaged destinations as one log-safe string — paths only.
fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
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

    /// Creates `<library>/<root>/<install>/<relative>` as an empty file and
    /// returns it. `root` is `"emulators"` or `"Emulators"`.
    fn touch_install(library: &Path, root: &str, install: &str, relative: &str) -> PathBuf {
        let file = library.join(root).join(install).join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"stub").unwrap();
        file
    }

    /// The launcher config file path the salvage resolution uses for
    /// `%CONFIG_DIR%`. Never read from disk here.
    fn config_path(library: &Path) -> PathBuf {
        library.join("config.json")
    }

    /// A catalog profile named `name` with one save, state or screenshot
    /// directory list filled in.
    fn profile(name: &str, save: &[&str], state: &[&str]) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            save_directories: save.iter().map(|s| s.to_string()).collect(),
            state_directories: state.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    /// The resolve context `remove_emulator_files` builds, for a direct
    /// [`salvage_user_data`] call.
    fn ctx<'a>(
        emulator_dir: &'a Path,
        library: &'a str,
        config_dir: &'a Path,
    ) -> ResolveContext<'a> {
        ResolveContext {
            emulator_dir: Some(emulator_dir),
            library_dir: library,
            config_dir,
            windows_documents: None,
            retroarch_portable_home: None,
        }
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
    fn expected_dir(library: &Path, root: &str, install: &str) -> PathBuf {
        library.join(root).join(install).canonicalize().unwrap()
    }

    // (a)
    #[test]
    fn a_catalog_install_yields_its_install_directory() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "PCSX2 (Playstation 2)", "pcsx2-qt");

        let steps = emulator_removal_steps(&entry("PCSX2 (Playstation 2)", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "emulators", "PCSX2 (Playstation 2)").as_path()]
        );
    }

    // (b)
    #[test]
    fn an_appimage_kept_in_place_yields_its_install_directory() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(
            library,
            "emulators",
            "Cemu (Wii U)",
            "Cemu-2.6-x86_64.AppImage",
        );

        let steps = emulator_removal_steps(&entry("Cemu (Wii U)", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "emulators", "Cemu (Wii U)").as_path()]
        );
    }

    // (c)
    #[test]
    fn a_nested_executable_still_yields_the_top_install_directory() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "X", "bin/foo");

        let steps = emulator_removal_steps(&entry("X", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "emulators", "X").as_path()]
        );
    }

    // (d)
    #[test]
    fn an_unmanaged_path_yields_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        // Something managed has to exist, or `<library>/emulators` would be
        // missing and every case below would pass for the wrong reason.
        touch_install(library, "emulators", "Keep", "keep");

        let system = raw_entry("RetroArch", "/usr/bin/retroarch");
        assert!(emulator_removal_steps(&system, &[], library).is_empty());

        let elsewhere = library.join("SomethingElse").join("foo");
        fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
        fs::write(&elsewhere, b"stub").unwrap();
        assert!(emulator_removal_steps(&entry("Elsewhere", &elsewhere), &[], library).is_empty());

        let root = library.join("emulators");
        assert!(
            emulator_removal_steps(&entry("Root", &root), &[], library).is_empty(),
            "the emulators root itself is never a removal target"
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
        touch_install(&library, "emulators", "Redream", "redream");

        let tilde = raw_entry("Redream", "~/GRID/emulators/Redream/redream");
        let steps = emulator_removal_steps(&tilde, &[], &library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(&library, "emulators", "Redream").as_path()]
        );
    }

    // (f)
    #[test]
    fn two_entries_sharing_one_install_directory_remove_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "Dolphin", "dolphin-emu");
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
        touch_install(library, "emulators", "Keep", "keep");
        let gone = library.join("emulators").join("Gone").join("gone");

        assert!(emulator_removal_steps(&entry("Gone", &gone), &[], library).is_empty());
        assert!(emulator_removal_steps(&raw_entry("Blank", "  "), &[], library).is_empty());
    }

    // (h)
    #[test]
    fn remove_emulator_files_deletes_the_tree_and_leaves_siblings_alone() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let pcsx2 = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");
        let redream = touch_install(library, "emulators", "Redream", "redream");
        let config = Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators: vec![entry("PCSX2", &pcsx2), entry("Redream", &redream)],
            ..Default::default()
        };

        remove_emulator_files(&config, &config_path(library), "pcsx2", &[]).unwrap();

        assert!(!library.join("emulators").join("PCSX2").exists());
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
        let exe = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");

        let no_library = Config {
            library_path: String::new(),
            emulators: vec![entry("PCSX2", &exe)],
            ..Default::default()
        };
        remove_emulator_files(&no_library, &config_path(library), "PCSX2", &[]).unwrap();
        assert!(exe.is_file());

        let config = Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators: vec![entry("PCSX2", &exe)],
            ..Default::default()
        };
        remove_emulator_files(&config, &config_path(library), "Nothing Like This", &[]).unwrap();
        assert!(exe.is_file());
    }

    // (j)
    #[test]
    fn a_legacy_emulators_root_install_is_still_removed() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "Emulators", "PCSX2-latest", "pcsx2-qt");

        let steps = emulator_removal_steps(&entry("PCSX2", &exe), &[], library);
        assert_eq!(
            folder_steps(&steps),
            vec![expected_dir(library, "Emulators", "PCSX2-latest").as_path()]
        );
    }

    #[test]
    fn managed_install_dir_for_matches_emulator_removal_steps() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");

        assert_eq!(
            managed_install_dir_for(&entry("PCSX2", &exe), library),
            Some(expected_dir(library, "emulators", "PCSX2"))
        );
    }

    // (k)
    #[test]
    fn two_roots_never_both_match_one_executable() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let new_exe = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");
        let legacy_exe = touch_install(library, "Emulators", "PCSX2-latest", "pcsx2-qt");

        let new_steps = emulator_removal_steps(&entry("PCSX2 New", &new_exe), &[], library);
        assert_eq!(
            folder_steps(&new_steps),
            vec![expected_dir(library, "emulators", "PCSX2").as_path()]
        );

        let legacy_steps =
            emulator_removal_steps(&entry("PCSX2 Legacy", &legacy_exe), &[], library);
        assert_eq!(
            folder_steps(&legacy_steps),
            vec![expected_dir(library, "Emulators", "PCSX2-latest").as_path()]
        );
    }

    // --- salvage ------------------------------------------------------------

    #[test]
    fn salvage_moves_a_save_dir_inside_the_install_dir() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");
        let install_dir = expected_dir(library, "emulators", "PCSX2");
        fs::create_dir_all(install_dir.join("memcards")).unwrap();
        fs::write(install_dir.join("memcards").join("slot1.mcd"), b"SAVE").unwrap();

        let entry = entry("PCSX2", &exe);
        let profile = profile("PCSX2", &["memcards"], &[]);
        let destination = super::saves_dir(library, "PCSX2");
        let library_raw = library.to_string_lossy().into_owned();
        let salvaged = salvage_user_data(
            &entry,
            &install_dir,
            &destination,
            Some(&profile),
            &ctx(&install_dir, &library_raw, library),
        )
        .unwrap();

        assert_eq!(salvaged, vec![destination.join("memcards")]);
        assert_eq!(
            fs::read(destination.join("memcards").join("slot1.mcd")).unwrap(),
            b"SAVE"
        );
        assert!(!install_dir.join("memcards").exists());
    }

    #[cfg(unix)]
    #[test]
    fn salvage_moves_real_dirs_but_skips_a_linked_dir() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");
        let install_dir = expected_dir(library, "emulators", "PCSX2");
        let destination = super::saves_dir(library, "PCSX2");

        // `memcards` is already linked into `saves/`; `savestates` is still a
        // real directory inside the install.
        let linked_target = destination.join("memcards");
        fs::create_dir_all(&linked_target).unwrap();
        fs::write(linked_target.join("slot1.mcd"), b"CARD").unwrap();
        std::os::unix::fs::symlink(&linked_target, install_dir.join("memcards")).unwrap();
        fs::create_dir_all(install_dir.join("savestates")).unwrap();
        fs::write(install_dir.join("savestates").join("slot1.p2s"), b"STATE").unwrap();

        let entry = entry("PCSX2", &exe);
        let profile = profile("PCSX2", &["memcards"], &["savestates"]);
        let library_raw = library.to_string_lossy().into_owned();
        let salvaged = salvage_user_data(
            &entry,
            &install_dir,
            &destination,
            Some(&profile),
            &ctx(&install_dir, &library_raw, library),
        )
        .unwrap();

        // Exactly one destination: the real directory ran through the move,
        // the linked one was considered and rejected.
        assert_eq!(salvaged, vec![destination.join("savestates")]);
        assert_eq!(
            fs::read(destination.join("savestates").join("slot1.p2s")).unwrap(),
            b"STATE"
        );
        assert!(!install_dir.join("savestates").exists());

        assert!(
            super::super::user_data_links::is_link(&install_dir.join("memcards")),
            "the link itself must survive"
        );
        assert_eq!(fs::read(linked_target.join("slot1.mcd")).unwrap(), b"CARD");
    }

    /// PCSX2's AppImage writes under `<install>/PCSX2`, so the salvaged
    /// directory keeps its name BELOW that data root — `saves/<E>/memcards`,
    /// the name a reinstall links again, not `saves/<E>/PCSX2/memcards`.
    #[test]
    fn salvage_strips_the_pcsx2_appimage_data_root_from_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "PCSX2", "pcsx2-2.5.0.AppImage");
        let install_dir = expected_dir(library, "emulators", "PCSX2");
        let data_root = install_dir.join("PCSX2");
        fs::create_dir_all(data_root.join("memcards")).unwrap();
        fs::write(data_root.join("memcards").join("slot1.mcd"), b"SAVE").unwrap();

        let entry = entry("PCSX2", &exe);
        let profile = profile("PCSX2", &["memcards"], &[]);
        let emulator_dir = emulator_dir_for(&entry, std::slice::from_ref(&profile)).unwrap();
        assert_eq!(emulator_dir, data_root);
        let destination = super::saves_dir(library, "PCSX2");
        let library_raw = library.to_string_lossy().into_owned();
        let salvaged = salvage_user_data(
            &entry,
            &install_dir,
            &destination,
            Some(&profile),
            &ctx(&emulator_dir, &library_raw, library),
        )
        .unwrap();

        assert_eq!(salvaged, vec![destination.join("memcards")]);
        assert_eq!(
            fs::read(destination.join("memcards").join("slot1.mcd")).unwrap(),
            b"SAVE"
        );
        assert!(!data_root.join("memcards").exists());
    }

    /// The same install once the links are in place: the candidate resolves
    /// into `saves/`, so nothing is salvaged and the link survives.
    #[cfg(unix)]
    #[test]
    fn salvage_skips_a_linked_pcsx2_appimage_install() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "emulators", "PCSX2", "pcsx2-2.5.0.AppImage");
        let install_dir = expected_dir(library, "emulators", "PCSX2");
        let data_root = install_dir.join("PCSX2");
        fs::create_dir_all(&data_root).unwrap();
        let destination = super::saves_dir(library, "PCSX2");
        let linked_target = destination.join("memcards");
        fs::create_dir_all(&linked_target).unwrap();
        fs::write(linked_target.join("slot1.mcd"), b"CARD").unwrap();
        std::os::unix::fs::symlink(&linked_target, data_root.join("memcards")).unwrap();

        let entry = entry("PCSX2", &exe);
        let profile = profile("PCSX2", &["memcards"], &[]);
        let emulator_dir = emulator_dir_for(&entry, std::slice::from_ref(&profile)).unwrap();
        let library_raw = library.to_string_lossy().into_owned();
        let salvaged = salvage_user_data(
            &entry,
            &install_dir,
            &destination,
            Some(&profile),
            &ctx(&emulator_dir, &library_raw, library),
        )
        .unwrap();

        assert!(salvaged.is_empty(), "{salvaged:?}");
        assert!(
            super::super::user_data_links::is_link(&data_root.join("memcards")),
            "the link itself must survive"
        );
        assert_eq!(fs::read(linked_target.join("slot1.mcd")).unwrap(), b"CARD");
    }

    #[test]
    fn salvage_moves_loose_files_when_the_directory_is_the_install_dir_itself() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let name = "Redream (Sega Dreamcast)";
        let exe = touch_install(library, "emulators", name, "redream");
        let install_dir = expected_dir(library, "emulators", name);
        for file in [
            "vmu0.bin",
            "redream.cfg",
            "Redream (Sega Dreamcast)-nightly.gz",
        ] {
            fs::write(install_dir.join(file), b"stub").unwrap();
        }

        let entry = entry(name, &exe);
        let profile = profile(name, &[], &["."]);
        let destination = super::saves_dir(library, name);
        let library_raw = library.to_string_lossy().into_owned();
        let mut salvaged = salvage_user_data(
            &entry,
            &install_dir,
            &destination,
            Some(&profile),
            &ctx(&install_dir, &library_raw, library),
        )
        .unwrap();
        salvaged.sort();

        assert_eq!(
            salvaged,
            vec![
                destination.join("redream.cfg"),
                destination.join("vmu0.bin")
            ]
        );
        assert!(destination.join("vmu0.bin").is_file());
        assert!(destination.join("redream.cfg").is_file());
        assert!(exe.is_file(), "the executable stays in the install");
        assert!(
            install_dir
                .join("Redream (Sega Dreamcast)-nightly.gz")
                .is_file(),
            "the downloaded archive stays in the install"
        );
    }

    #[test]
    fn remove_emulator_files_salvages_then_removes() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let pcsx2 = touch_install(library, "emulators", "PCSX2", "pcsx2-qt");
        let redream = touch_install(library, "emulators", "Redream", "redream");
        let install_dir = expected_dir(library, "emulators", "PCSX2");
        fs::create_dir_all(install_dir.join("memcards")).unwrap();
        fs::write(install_dir.join("memcards").join("slot1.mcd"), b"SAVE").unwrap();

        let config = Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators: vec![entry("PCSX2", &pcsx2), entry("Redream", &redream)],
            ..Default::default()
        };
        let profiles = vec![profile("PCSX2", &["memcards"], &[])];

        remove_emulator_files(&config, &config_path(library), "pcsx2", &profiles).unwrap();

        assert_eq!(
            fs::read(
                super::saves_dir(library, "PCSX2")
                    .join("memcards")
                    .join("slot1.mcd")
            )
            .unwrap(),
            b"SAVE"
        );
        assert!(!library.join("emulators").join("PCSX2").exists());
        assert!(redream.is_file(), "the other install must survive");
    }

    #[test]
    fn a_delete_under_the_legacy_root_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path();
        let exe = touch_install(library, "Emulators", "PCSX2-latest", "pcsx2-qt");
        let install_dir = expected_dir(library, "Emulators", "PCSX2-latest");
        fs::create_dir_all(install_dir.join("memcards")).unwrap();
        fs::write(install_dir.join("memcards").join("slot1.mcd"), b"SAVE").unwrap();

        let config = Config {
            library_path: library.to_string_lossy().into_owned(),
            emulators: vec![entry("PCSX2", &exe)],
            ..Default::default()
        };
        let profiles = vec![profile("PCSX2", &["memcards"], &[])];

        remove_emulator_files(&config, &config_path(library), "PCSX2", &profiles).unwrap();

        assert_eq!(
            fs::read(
                saves_dir(library, "PCSX2")
                    .join("memcards")
                    .join("slot1.mcd")
            )
            .unwrap(),
            b"SAVE"
        );
        assert!(!install_dir.exists());
    }
}
