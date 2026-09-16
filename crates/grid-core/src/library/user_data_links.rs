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

/// Points every `user_data` directory of one emulator install at
/// `saves_dir`, returning whether anything on disk changed.
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
pub fn ensure_user_data_links(
    install_dir: &Path,
    saves_dir: &Path,
    user_data: &[String],
) -> Result<bool, LibraryError> {
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
        let link = install_dir.join(name);
        fs::create_dir_all(&target)?;

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
        // destination is removed first and recreated if the rename fails
        // (a cross-device move, which the merge below handles by copying
        // nothing and renaming per entry).
        let _ = fs::remove_dir(dest);
        if fs::rename(src, dest).is_ok() {
            return Ok(());
        }
        fs::create_dir_all(dest)?;
    }
    move_tree_preferring_dest(src, dest)
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
        fs::rename(&from, &to)?;
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
        fs::create_dir_all(install.join("inis")).unwrap();
        fs::write(install.join("inis").join("PCSX2.ini"), b"install").unwrap();
        fs::write(install.join("inis").join("extra.ini"), b"extra").unwrap();
        fs::create_dir_all(saves.join("inis")).unwrap();
        fs::write(saves.join("inis").join("PCSX2.ini"), b"saves").unwrap();

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
    fn an_entry_with_a_separator_is_skipped() {
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
