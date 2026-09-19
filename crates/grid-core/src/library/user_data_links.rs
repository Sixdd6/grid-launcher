//! Moves the directories an emulator writes beside its own binary under
//! `saves/<Emulator>/` and leaves a link behind, so an emulator reinstall
//! (which replaces the install directory's contents) never takes a user's
//! memory cards, states or configs with it.
//!
//! The link is a relative symlink on unix and an NTFS junction on Windows —
//! both are transparent to the emulator, which keeps writing to the path it
//! always wrote to.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use super::LibraryError;
use crate::config::EmulatorEntry;
use crate::launch::profiles::EmulatorProfile;

/// The directory a profile's `user_data` links belong in for the executable
/// `exe`: the DATA root `autoconfig::emulator_data_root` resolves for a
/// synthetic entry named after the profile — `<exe dir>/PCSX2` for the PCSX2
/// AppImage (the only place that build reads), and the executable's own
/// directory for everything else. The exe parent is the fallback for a path
/// with no resolvable root at all.
///
/// The link call sites (`library::InstallService`,
/// `launch::emu_install::install_manual_archive`,
/// `library::layout_migration`) all go through this one function, so a link
/// can never land somewhere the writers and readers do not look.
pub fn user_data_root(profile: &EmulatorProfile, exe: &Path) -> PathBuf {
    let entry = EmulatorEntry {
        name: profile.name.clone(),
        path: exe.to_string_lossy().into_owned(),
        ..Default::default()
    };
    crate::autoconfig::emulator_data_root(&entry, std::slice::from_ref(profile))
        .unwrap_or_else(|| exe.parent().unwrap_or(exe).to_path_buf())
}

/// Points every `user_data` directory of one emulator install at
/// `saves_dir`, returning whether anything on disk changed.
///
/// `install_dir` is the DATA root the emulator reads from — build it with
/// [`user_data_root`], which is the directory holding the chosen EXECUTABLE
/// for every emulator but the PCSX2 AppImage. It is never the extraction
/// root, which is a different directory whenever the binary is nested. The
/// caller creates the root first: a link cannot be made inside a directory
/// that does not exist.
///
/// Per entry: the destination `saves_dir/<dir>` is created; a real directory
/// at `install_dir/<dir>` has its contents moved there (the destination wins
/// every collision — those bytes are the ones that survived earlier runs)
/// and is replaced by a link; an absent entry just gets the link; a link
/// already pointing at the destination is left alone, and one pointing
/// anywhere else is replaced.
///
/// Two things are skipped with a warning rather than failing the call: an
/// entry that is not a single path component (a malformed catalog value),
/// and a regular FILE sitting at `install_dir/<dir>` (the emulator names a
/// file, not a directory — moving it would break the install). The first
/// I/O error returns immediately; the caller decides whether that fails the
/// whole install or is only worth a warning line.
///
/// Both sides are CANONICALIZED before the link is created, so the unix
/// link text is computed between two real paths. A `..` component or a
/// symlinked parent in either argument would otherwise produce a `..` count
/// the kernel reads differently than this code does, leaving a dangling
/// link that every later run replaces again.
pub fn ensure_user_data_links(
    install_dir: &Path,
    saves_dir: &Path,
    user_data: &[String],
) -> Result<bool, LibraryError> {
    // The install directory exists by the time this runs; the fallback keeps
    // a caller that got that wrong on the old lexical behavior instead of
    // failing outright.
    let install_root = fs::canonicalize(install_dir).unwrap_or_else(|_| install_dir.to_path_buf());
    let mut changed = false;
    for entry in user_data {
        let name = entry.trim();
        if !is_single_component(name) {
            tracing::warn!(
                entry = %name,
                "user data entry is not a single directory name; skipped"
            );
            continue;
        }
        let target = saves_dir.join(name);
        let link = install_root.join(name);
        fs::create_dir_all(&target)?;
        // Canonical only after `create_dir_all`, which is what makes the
        // path resolvable.
        let target = fs::canonicalize(&target).unwrap_or(target);

        if is_link(&link) {
            if points_at(&link, &target) {
                continue;
            }
            let current = read_link_target(&link).unwrap_or_default();
            tracing::debug!(
                link = %link.display(),
                from = %current.display(),
                to = %target.display(),
                "user data link repointed"
            );
            remove_link(&link)?;
            link_dir(&target, &link)?;
            changed = true;
        } else if link.is_dir() {
            move_into(&link, &target)?;
            // `move_into` leaves nothing behind on the rename path and an
            // empty husk on the merge path; either way the link needs the
            // name free.
            let _ = fs::remove_dir(&link);
            link_dir(&target, &link)?;
            changed = true;
        } else if link.exists() {
            tracing::warn!(
                path = %link.display(),
                "user data path is a file, not a directory; left in place"
            );
        } else {
            link_dir(&target, &link)?;
            changed = true;
        }
    }
    Ok(changed)
}

/// Whether `name` is exactly one ordinary path component — no separator, no
/// `.`, no `..`, not empty, no drive prefix.
fn is_single_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// Moves everything in `src` into `dest`. A rename when `dest` is still
/// empty (the common case: the first run of a fresh install), otherwise a
/// destination-wins merge.
fn move_into(src: &Path, dest: &Path) -> Result<(), LibraryError> {
    if fs::read_dir(dest)?.next().is_none() {
        // `rename` onto an existing directory is not portable, so the empty
        // destination is removed first — and recreated when the rename
        // fails, which for a `saves/` on another filesystem is
        // `CrossesDevices`. The merge below then moves the tree file by
        // file, copying each one across the device boundary.
        let _ = fs::remove_dir(dest);
        if fs::rename(src, dest).is_ok() {
            return Ok(());
        }
        fs::create_dir_all(dest)?;
    }
    move_tree_preferring_dest(src, dest)
}

/// Moves one file, falling back to copy-then-delete when `src` and `dest`
/// are on different filesystems. Split from [`finish_move`] so the fallback
/// is testable without two filesystems.
pub(crate) fn move_file(src: &Path, dest: &Path) -> io::Result<()> {
    finish_move(src, dest, fs::rename(src, dest))
}

/// [`move_file`]'s tail, given the result of the rename attempt. The source
/// is deleted only once the copy is known to be complete: a short copy
/// (a full disk) must leave the original where it is.
fn finish_move(src: &Path, dest: &Path, renamed: io::Result<()>) -> io::Result<()> {
    match renamed {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            // A symlink would be followed by `fs::copy`, so it is recreated
            // rather than copied.
            if fs::symlink_metadata(src)?.file_type().is_symlink() {
                return copy_symlink(src, dest);
            }
            let copied = fs::copy(src, dest)?;
            let source_len = fs::metadata(src)?.len();
            if copied != source_len || fs::metadata(dest)?.len() != source_len {
                return Err(io::Error::other(format!(
                    "short copy: {} of {source_len} bytes reached {}",
                    copied,
                    dest.display()
                )));
            }
            fs::remove_file(src)
        }
        Err(error) => Err(error),
    }
}

/// Recreates the symlink `src` at `dest` and removes `src`. Only reachable
/// from the cross-device fallback, where a rename is not available.
#[cfg(unix)]
fn copy_symlink(src: &Path, dest: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, dest)?;
    fs::remove_file(src)
}

#[cfg(not(unix))]
fn copy_symlink(src: &Path, dest: &Path) -> io::Result<()> {
    let _ = (src, dest);
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// Recursively moves `src`'s entries into `dest`, keeping `dest`'s version
/// of every collision (the opposite of `library::merge_tree_into`, which is
/// for freshly extracted payloads: here `dest` holds the user's own data).
/// A colliding source entry is deleted. `src` is removed once empty.
///
/// `DirEntry::file_type` does not follow symlinks, so a symlink inside the
/// tree is moved as itself rather than descended into.
pub(crate) fn move_tree_preferring_dest(src: &Path, dest: &Path) -> Result<(), LibraryError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let from_is_dir = entry.file_type()?.is_dir();

        if from_is_dir && to.is_dir() && !is_link(&to) {
            move_tree_preferring_dest(&from, &to)?;
            continue;
        }
        // `symlink_metadata` rather than `exists`, so a dangling symlink at
        // the destination still counts as occupied.
        if fs::symlink_metadata(&to).is_ok() {
            if from_is_dir {
                fs::remove_dir_all(&from)?;
            } else {
                fs::remove_file(&from)?;
            }
            continue;
        }
        if from_is_dir {
            match fs::rename(&from, &to) {
                Ok(()) => continue,
                // `dest` is on another filesystem: move the subtree entry by
                // entry instead, which copies each file across.
                Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
                    move_tree_preferring_dest(&from, &to)?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            }
        }
        move_file(&from, &to)?;
    }
    // A husk left by a failed removal is not worth failing the move over:
    // the caller only needs the name free, and reports that itself.
    let _ = fs::remove_dir(src);
    Ok(())
}

/// Whether `link` resolves to the same directory as `target`. Both sides are
/// canonicalized, so a link through another link still matches, and a
/// dangling link never does.
pub(crate) fn points_at(link: &Path, target: &Path) -> bool {
    match (fs::canonicalize(link), fs::canonicalize(target)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// `target` expressed relative to `base`, or `target` unchanged when the two
/// share no common root. Keeps a unix link valid when the whole library is
/// moved or mounted somewhere else.
#[cfg(unix)]
fn relative_target(base: &Path, target: &Path) -> PathBuf {
    if !base.is_absolute() || !target.is_absolute() {
        return target.to_path_buf();
    }
    let mut from = base.components().peekable();
    let mut to = target.components().peekable();
    while from.peek().is_some() && from.peek() == to.peek() {
        from.next();
        to.next();
    }
    let mut relative = PathBuf::new();
    for _ in from {
        relative.push("..");
    }
    for component in to {
        relative.push(component);
    }
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

// --- platform primitives ----------------------------------------------------

/// Creates a link at `link` pointing at the directory `target`: a RELATIVE
/// symlink on unix, an NTFS junction (which has no relative form) on
/// Windows.
#[cfg(unix)]
pub(crate) fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    let base = link.parent().unwrap_or(Path::new("."));
    std::os::unix::fs::symlink(relative_target(base, target), link)
}

#[cfg(windows)]
pub(crate) fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    junction::create(target, link)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    let _ = (target, link);
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// Whether `path` is a link rather than a real directory.
#[cfg(unix)]
pub(crate) fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

#[cfg(windows)]
pub(crate) fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
        || junction::exists(path).unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_link(path: &Path) -> bool {
    let _ = path;
    false
}

/// Where `link` points, as an absolute path. A relative unix symlink is
/// joined onto the link's own parent.
#[cfg(unix)]
pub(crate) fn read_link_target(link: &Path) -> io::Result<PathBuf> {
    let raw = fs::read_link(link)?;
    if raw.is_absolute() {
        return Ok(raw);
    }
    Ok(link.parent().unwrap_or(Path::new(".")).join(raw))
}

#[cfg(windows)]
pub(crate) fn read_link_target(link: &Path) -> io::Result<PathBuf> {
    junction::get_target(link)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn read_link_target(link: &Path) -> io::Result<PathBuf> {
    let _ = link;
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// Removes the link itself, never what it points at. A junction is a real
/// directory carrying a reparse point, so Windows needs both steps.
#[cfg(unix)]
pub(crate) fn remove_link(link: &Path) -> io::Result<()> {
    fs::remove_file(link)
}

#[cfg(windows)]
pub(crate) fn remove_link(link: &Path) -> io::Result<()> {
    junction::delete(link)?;
    fs::remove_dir(link)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn remove_link(link: &Path) -> io::Result<()> {
    let _ = link;
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// A catalog profile named `name` with `user_data` left empty —
    /// [`user_data_root`] reads only the name and the executable.
    fn profile(name: &str) -> EmulatorProfile {
        EmulatorProfile {
            name: name.to_string(),
            match_tokens: vec![name.to_lowercase()],
            ..Default::default()
        }
    }

    #[test]
    fn user_data_root_is_the_pcsx2_subdirectory_for_an_appimage() {
        let install = Path::new("/library/emulators/PCSX2 (Playstation 2)");
        assert_eq!(
            user_data_root(&profile("PCSX2"), &install.join("pcsx2-2.5.0.AppImage")),
            install.join("PCSX2")
        );
    }

    #[test]
    fn user_data_root_is_the_exe_parent_for_a_pcsx2_binary_that_is_not_an_appimage() {
        let install = Path::new("/library/emulators/PCSX2 (Playstation 2)");
        assert_eq!(
            user_data_root(&profile("PCSX2"), &install.join("pcsx2-qt.exe")),
            install.to_path_buf()
        );
    }

    #[test]
    fn user_data_root_is_the_exe_parent_for_another_profile() {
        let install = Path::new("/library/emulators/Dolphin (GameCube)");
        assert_eq!(
            user_data_root(&profile("Dolphin"), &install.join("Dolphin.AppImage")),
            install.to_path_buf()
        );
    }

    /// `<temp>/emulators/E` and `<temp>/saves/E`, the install/saves pair
    /// every test links between.
    fn layout(temp: &Path) -> (PathBuf, PathBuf) {
        let install = temp.join("emulators").join("E");
        let saves = temp.join("saves").join("E");
        fs::create_dir_all(&install).unwrap();
        (install, saves)
    }

    #[cfg(unix)]
    #[test]
    fn absent_dir_becomes_a_relative_link() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());

        let changed = ensure_user_data_links(&install, &saves, &["memcards".to_string()]).unwrap();

        assert!(changed);
        let link = install.join("memcards");
        assert!(is_link(&link));
        let raw = fs::read_link(&link).unwrap();
        assert!(
            raw.starts_with(".."),
            "link target must be relative: {raw:?}"
        );
        assert!(points_at(&link, &saves.join("memcards")));
        assert_eq!(
            read_link_target(&link).unwrap().canonicalize().unwrap(),
            saves.join("memcards").canonicalize().unwrap()
        );
    }

    #[test]
    fn real_dir_is_moved_then_linked() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());
        fs::create_dir_all(install.join("memcards")).unwrap();
        fs::write(install.join("memcards").join("slot.mcd"), b"card").unwrap();

        let changed = ensure_user_data_links(&install, &saves, &["memcards".to_string()]).unwrap();

        assert!(changed);
        assert_eq!(
            fs::read(saves.join("memcards").join("slot.mcd")).unwrap(),
            b"card"
        );
        assert!(is_link(&install.join("memcards")));
        assert!(points_at(
            &install.join("memcards"),
            &saves.join("memcards")
        ));
    }

    #[test]
    fn existing_saves_content_wins_on_merge() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());
        fs::create_dir_all(install.join("inis").join("sub")).unwrap();
        fs::write(install.join("inis").join("PCSX2.ini"), b"install").unwrap();
        fs::write(install.join("inis").join("extra.ini"), b"extra").unwrap();
        fs::write(install.join("inis").join("sub").join("a.ini"), b"install").unwrap();
        fs::write(
            install.join("inis").join("sub").join("b.ini"),
            b"only-install",
        )
        .unwrap();
        fs::create_dir_all(saves.join("inis").join("sub")).unwrap();
        fs::write(saves.join("inis").join("PCSX2.ini"), b"saves").unwrap();
        fs::write(saves.join("inis").join("sub").join("a.ini"), b"saves").unwrap();

        let changed = ensure_user_data_links(&install, &saves, &["inis".to_string()]).unwrap();

        assert!(changed);
        assert_eq!(
            fs::read(saves.join("inis").join("PCSX2.ini")).unwrap(),
            b"saves"
        );
        assert_eq!(
            fs::read(saves.join("inis").join("extra.ini")).unwrap(),
            b"extra"
        );
        // The nested directory merges by the same rule, one level down.
        assert_eq!(
            fs::read(saves.join("inis").join("sub").join("a.ini")).unwrap(),
            b"saves"
        );
        assert_eq!(
            fs::read(saves.join("inis").join("sub").join("b.ini")).unwrap(),
            b"only-install"
        );
        assert!(is_link(&install.join("inis")));
    }

    #[test]
    fn wrong_target_link_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());
        let elsewhere = temp.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let link = install.join("memcards");
        link_dir(&elsewhere, &link).unwrap();

        let changed = ensure_user_data_links(&install, &saves, &["memcards".to_string()]).unwrap();

        assert!(changed);
        assert!(points_at(&link, &saves.join("memcards")));
        assert!(!points_at(&link, &elsewhere));
    }

    #[cfg(unix)]
    #[test]
    fn second_call_changes_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());
        let dirs = vec!["memcards".to_string()];
        assert!(ensure_user_data_links(&install, &saves, &dirs).unwrap());
        let first = fs::read_link(install.join("memcards")).unwrap();

        let changed = ensure_user_data_links(&install, &saves, &dirs).unwrap();

        assert!(!changed);
        assert_eq!(fs::read_link(install.join("memcards")).unwrap(), first);
    }

    #[test]
    fn a_regular_file_at_the_user_data_path_is_left_alone() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());
        fs::write(install.join("memcards"), b"not a directory").unwrap();

        let changed = ensure_user_data_links(&install, &saves, &["memcards".to_string()]).unwrap();

        assert!(!changed);
        assert!(!is_link(&install.join("memcards")));
        assert_eq!(
            fs::read(install.join("memcards")).unwrap(),
            b"not a directory"
        );
    }

    #[test]
    fn invalid_entries_are_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());

        let changed = ensure_user_data_links(
            &install,
            &saves,
            &[
                "nested/memcards".to_string(),
                "..".to_string(),
                String::new(),
            ],
        )
        .unwrap();

        assert!(!changed);
        assert!(!install.join("nested").exists());
        assert!(!saves.join("nested").exists());
    }

    /// The link text is computed from CANONICAL paths, so a symlinked
    /// parent that makes the install directory lexically shallower than it
    /// physically is still yields a link that resolves.
    #[cfg(unix)]
    #[test]
    fn relative_link_is_correct_through_a_symlinked_install_parent() {
        let temp = tempfile::tempdir().unwrap();
        let install = temp
            .path()
            .join("real")
            .join("deep")
            .join("emulators")
            .join("E");
        fs::create_dir_all(&install).unwrap();
        // `alias` stands in for `real/deep`: one component where the real
        // path has two, so a lexical `..` count comes out one short.
        std::os::unix::fs::symlink(
            temp.path().join("real").join("deep"),
            temp.path().join("alias"),
        )
        .unwrap();
        let aliased_install = temp.path().join("alias").join("emulators").join("E");
        let saves = temp.path().join("saves").join("E");

        let changed =
            ensure_user_data_links(&aliased_install, &saves, &["memcards".to_string()]).unwrap();

        assert!(changed);
        let link = install.join("memcards");
        assert!(fs::read_link(&link).unwrap().starts_with(".."));
        assert!(points_at(&link, &saves.join("memcards")));
        assert_eq!(
            read_link_target(&link).unwrap().canonicalize().unwrap(),
            saves.join("memcards").canonicalize().unwrap()
        );
    }

    /// The copy-then-delete path a `saves/` on another filesystem takes.
    /// Driven through `finish_move` with a synthetic `CrossesDevices`,
    /// because a test cannot conjure a second filesystem.
    #[test]
    fn a_cross_device_file_move_copies_then_deletes() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("slot.mcd");
        let dest = temp.path().join("moved.mcd");
        fs::write(&src, b"card").unwrap();

        finish_move(
            &src,
            &dest,
            Err(io::Error::from(io::ErrorKind::CrossesDevices)),
        )
        .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"card");
        assert!(!src.exists(), "the source must be gone after a full copy");
    }

    /// Any other rename failure is returned as itself: the fallback is for
    /// the device boundary only.
    #[test]
    fn a_non_cross_device_rename_error_is_returned() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("slot.mcd");
        fs::write(&src, b"card").unwrap();

        let error = finish_move(
            &src,
            &temp.path().join("moved.mcd"),
            Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(src.exists(), "nothing may be moved on an unrelated error");
    }

    #[cfg(windows)]
    #[test]
    fn junction_is_created_recognized_and_removed() {
        let temp = tempfile::tempdir().unwrap();
        let (install, saves) = layout(temp.path());
        let target = saves.join("memcards");
        fs::create_dir_all(&target).unwrap();
        let link = install.join("memcards");

        link_dir(&target, &link).unwrap();

        assert!(is_link(&link));
        assert_eq!(
            read_link_target(&link).unwrap().canonicalize().unwrap(),
            target.canonicalize().unwrap()
        );
        assert!(points_at(&link, &target));

        remove_link(&link).unwrap();
        assert!(!link.exists());
        assert!(target.is_dir());
    }
}
