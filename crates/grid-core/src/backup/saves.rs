//! A Windows game's saves on a backup drive ([`PackSaves`]).
//!
//! The Wine prefix never goes to a drive as a whole. What does:
//! - **the folder list** ([`recorded_folders`]): every save folder GRID knows
//!   for the game, the same list cloud saves use (PCGamingWiki's and the
//!   user's own, less the removed ones), as Windows-token paths. A folder
//!   the user added as a plain path inside the game's prefix is turned into
//!   its token form ([`token_for_prefix_path`]); any other plain path is
//!   left out.
//! - **the saves themselves** ([`scan_saves`]), when the game is installed
//!   with a Wine prefix on this computer: the folders that exist in the
//!   prefix, written by [`super::archive::write_saves_tree`] as one tar
//!   whose entries sit under each folder's position (`0/…`, `1/…`).
//!
//! A restore ([`restore_saves`]) reads the tar into a temporary folder and
//! copies each folder to where it belongs on this computer
//! ([`restore_targets`]): into the new prefix off Windows, into the real
//! folder on Windows. A folder that already holds files is never written
//! into. [`seed_folders`] gives the list to cloud saves.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::archive::{self, PartSource, RestoreError};
use super::local::{self, ScanRefusal, TreeEntryKind, TreePlan};
use super::pack::{InstalledLayout, PackSaves};
use crate::cloud::native::{
    native_save_paths, resolve_native_save_dir, translate_windows_path_to_wine_prefix,
    visible_native_paths,
};
use crate::cloud::ops::native::manual_paths_key_for_title;
use crate::config::Config;

/// The save folders to record for a game, in the order cloud saves use:
/// the visible PCGamingWiki folders, then the visible manual ones
/// ([`visible_native_paths`], [`native_save_paths`]).
///
/// - A Windows-token path (`%APPDATA%\Foo`) is kept as it is.
/// - A plain path inside `prefix`'s `drive_c` (`~/` expanded against
///   `home`) becomes its token form ([`token_for_prefix_path`]).
/// - Any other path (outside the prefix, or with no prefix to compare
///   with) is left out.
///
/// Duplicates (compared without case) are dropped, the first one kept.
pub fn recorded_folders(
    pcgw: &[String],
    manual: &[String],
    removed: &[String],
    prefix: Option<&Path>,
    home: Option<&Path>,
) -> Vec<String> {
    let (pcgw, manual) = visible_native_paths(pcgw, manual, removed);
    let prefix =
        prefix.map(|prefix| crate::library::path_change::expand(&prefix.to_string_lossy(), home));
    let mut seen = HashSet::new();
    native_save_paths(&pcgw, &manual)
        .into_iter()
        .filter_map(|raw| {
            let raw = raw.trim();
            if is_token_path(raw) {
                return Some(raw.to_string());
            }
            let prefix = prefix.as_deref()?;
            let path = crate::library::path_change::expand(raw, home);
            token_for_prefix_path(&path, prefix)
        })
        .filter(|folder| seen.insert(folder.to_lowercase()))
        .collect()
}

/// Whether `raw` starts with a `%NAME%` token.
fn is_token_path(raw: &str) -> bool {
    let Some(rest) = raw.strip_prefix('%') else {
        return false;
    };
    match rest.find('%') {
        Some(end) if end > 0 => rest[..end]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_'),
        _ => false,
    }
}

/// The Windows-token form of `path`, a folder inside `prefix`'s `drive_c`,
/// or `None` when it is not strictly inside it:
///
/// | inside `drive_c`                    | token                              |
/// |-------------------------------------|------------------------------------|
/// | `users/Public/…`                    | `%PUBLIC%\…`                       |
/// | `users/<user>/AppData/Roaming/…`    | `%APPDATA%\…`                      |
/// | `users/<user>/AppData/Local/…`      | `%LOCALAPPDATA%\…`                 |
/// | `users/<user>/AppData/LocalLow/…`   | `%USERPROFILE%\AppData\LocalLow\…` |
/// | `users/<user>/Documents/…`          | `%USERPROFILE%\Documents\…`        |
/// | `users/<user>/…`                    | `%USERPROFILE%\…`                  |
/// | `ProgramData/…`                     | `%PROGRAMDATA%\…`                  |
/// | `windows/…`                         | `%WINDIR%\…`                       |
/// | anything else                       | `%SYSTEMDRIVE%\…`                  |
///
/// `<user>` is any user, not only this computer's. Folder names match
/// without case. Lexical: nothing is read from the disk.
pub fn token_for_prefix_path(path: &Path, prefix: &Path) -> Option<String> {
    if path.components().any(|c| c == Component::ParentDir) {
        return None;
    }
    let rel = path.strip_prefix(prefix.join("drive_c")).ok()?;
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    let is = |index: usize, name: &str| {
        parts
            .get(index)
            .is_some_and(|p| p.eq_ignore_ascii_case(name))
    };
    let (token, skip) = if is(0, "users") && is(1, "Public") {
        ("%PUBLIC%", 2)
    } else if is(0, "users") && parts.len() >= 2 {
        if is(2, "AppData") && is(3, "Roaming") {
            ("%APPDATA%", 4)
        } else if is(2, "AppData") && is(3, "LocalLow") {
            ("%USERPROFILE%\\AppData\\LocalLow", 4)
        } else if is(2, "AppData") && is(3, "Local") {
            ("%LOCALAPPDATA%", 4)
        } else if is(2, "Documents") {
            ("%USERPROFILE%\\Documents", 3)
        } else {
            ("%USERPROFILE%", 2)
        }
    } else if is(0, "ProgramData") {
        ("%PROGRAMDATA%", 1)
    } else if is(0, "windows") {
        ("%WINDIR%", 1)
    } else {
        ("%SYSTEMDRIVE%", 0)
    };
    let rest = &parts[skip.min(parts.len())..];
    Some(if rest.is_empty() {
        token.to_string()
    } else {
        format!("{token}\\{}", rest.join("\\"))
    })
}

/// What [`scan_saves`] found.
#[derive(Debug, Default)]
pub struct SavesScan {
    /// The folders that exist, ready to archive; `None` when none does.
    pub tree: Option<TreePlan>,
    /// Folders left out, with the reason ("it contains links").
    pub refused: Vec<(String, String)>,
}

/// The reason a save folder was left out, as a warning reads it.
fn refusal_text(refusal: &ScanRefusal) -> &'static str {
    match refusal {
        ScanRefusal::Link(_) => "it contains links",
        ScanRefusal::NotPortable(_) => "it has a file name Windows can't use",
        ScanRefusal::NotText(_) => "it has a file name that isn't valid text",
        ScanRefusal::Special(_) => "it contains a file GRID can't copy",
        ScanRefusal::Unreadable(_) => "it couldn't be read",
    }
}

/// Walks the save `folders` (Windows-token paths) that exist in the Wine
/// `prefix` ([`translate_windows_path_to_wine_prefix`], this computer's
/// user name). Folder `i`'s contents are named `i/…`. A folder whose
/// tree [`local::scan_tree`] would refuse (a link inside it, a name a
/// Windows drive can't hold when `for_windows`…) is left out and listed
/// in [`SavesScan::refused`]; the others are still copied. A folder that
/// is itself a link to a folder is followed.
pub fn scan_saves(folders: &[String], prefix: &Path, for_windows: bool) -> SavesScan {
    let mut entries = Vec::new();
    let mut refused = Vec::new();
    for (index, raw) in folders.iter().enumerate() {
        let Some(dir) = translate_windows_path_to_wine_prefix(raw, prefix) else {
            continue;
        };
        let meta = match fs::metadata(&dir) {
            Ok(meta) if meta.is_dir() => meta,
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                refused.push((
                    raw.clone(),
                    refusal_text(&local::unreadable(error)).to_string(),
                ));
                continue;
            }
        };
        let before = entries.len();
        let name = format!("{index}/");
        entries.push(local::entry(
            dir.clone(),
            name.clone(),
            TreeEntryKind::Dir,
            &meta,
        ));
        if let Err(refusal) = local::walk(&dir, &name, &HashSet::new(), for_windows, &mut entries) {
            entries.truncate(before);
            refused.push((raw.clone(), refusal_text(&refusal).to_string()));
        }
    }
    let tree = (!entries.is_empty()).then(|| {
        local::plan_of(
            prefix.to_path_buf(),
            InstalledLayout::Multi,
            Vec::new(),
            entries,
        )
    });
    SavesScan { tree, refused }
}

/// Whether a folder a save may be restored into is free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetState {
    Missing,
    Empty,
    /// It holds something, is a file, or can't be read.
    NotEmpty,
}

/// [`TargetState`] of `path` on the disk.
pub fn probe(path: &Path) -> TargetState {
    match fs::read_dir(path) {
        Ok(mut items) => match items.next() {
            None => TargetState::Empty,
            Some(_) => TargetState::NotEmpty,
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => TargetState::Missing,
        Err(_) => TargetState::NotEmpty,
    }
}

/// What a restore does with one save folder from the drive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveTarget {
    /// Copy folder `index` into `path`.
    Restore { index: usize, path: PathBuf },
    /// `path` already holds files: leave it alone.
    Occupied { index: usize, path: PathBuf },
    /// The folder has no place on this computer (a token this host can't
    /// resolve).
    Unresolved { index: usize },
}

/// Where each save folder on the drive (`present`, positions in `folders`)
/// goes on this computer. Pure: `probe` reports each target's state.
///
/// - With a Wine `prefix` (Linux, macOS): inside it, through
///   [`translate_windows_path_to_wine_prefix`] (this computer's user name).
/// - Without one (Windows): the real folder, through
///   [`resolve_native_save_dir`] with the Shell's `windows_documents`.
///
/// A target that is missing or empty is restored into; one that holds
/// anything is [`SaveTarget::Occupied`], so a restore never mixes old and
/// new saves.
pub fn restore_targets(
    folders: &[String],
    present: &[usize],
    prefix: Option<&Path>,
    windows_documents: Option<&Path>,
    probe: impl Fn(&Path) -> TargetState,
) -> Vec<SaveTarget> {
    present
        .iter()
        .filter_map(|&index| {
            let raw = folders.get(index)?;
            let path = match prefix {
                Some(prefix) => translate_windows_path_to_wine_prefix(raw, prefix),
                None => Some(resolve_native_save_dir(raw, windows_documents, None)),
            }
            .filter(|path| path.is_absolute());
            Some(match path {
                None => SaveTarget::Unresolved { index },
                Some(path) => match probe(&path) {
                    TargetState::Missing | TargetState::Empty => {
                        SaveTarget::Restore { index, path }
                    }
                    TargetState::NotEmpty => SaveTarget::Occupied { index, path },
                },
            })
        })
        .collect()
}

/// What [`restore_saves`] did.
#[derive(Debug, Default)]
pub struct SavesRestored {
    pub targets: Vec<SaveTarget>,
}

impl SavesRestored {
    /// One warning line per folder that was not restored.
    pub fn warnings(&self, folders: &[String]) -> Vec<String> {
        let folder = |index: usize| folders.get(index).cloned().unwrap_or_default();
        self.targets
            .iter()
            .filter_map(|target| match target {
                SaveTarget::Restore { .. } => None,
                SaveTarget::Occupied { index, path } => Some(format!(
                    "Saves for {} weren't restored: {} already has files",
                    folder(*index),
                    path.display()
                )),
                SaveTarget::Unresolved { index } => Some(format!(
                    "Saves for {} weren't restored: GRID can't tell where that folder is on this computer",
                    folder(*index)
                )),
            })
            .collect()
    }
}

/// Restores the saves archive in `parts` (`root` is the drive's `gridpack`
/// folder): reads it into `tmp` ([`archive::restore_tree`], every part's
/// size and MD5 checked), then copies each folder to its
/// [`restore_targets`] target. `tmp` is removed whatever happens.
///
/// A copy that fails part way leaves the files it copied: they are saves
/// the folder did not have.
pub fn restore_saves(
    root: &Path,
    parts: &[PartSource],
    folders: &[String],
    tmp: &Path,
    prefix: Option<&Path>,
    windows_documents: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<SavesRestored, RestoreError> {
    let _ = fs::remove_dir_all(tmp);
    let result = restore_saves_via(root, parts, folders, tmp, prefix, windows_documents, cancel);
    let _ = fs::remove_dir_all(tmp);
    result
}

fn restore_saves_via(
    root: &Path,
    parts: &[PartSource],
    folders: &[String],
    tmp: &Path,
    prefix: Option<&Path>,
    windows_documents: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<SavesRestored, RestoreError> {
    archive::restore_tree(root, parts, tmp, cancel, &mut |_, _| {}, &mut || Ok(()))?;
    let present: Vec<usize> = (0..folders.len())
        .filter(|index| tmp.join(index.to_string()).is_dir())
        .collect();
    let targets = restore_targets(folders, &present, prefix, windows_documents, probe);
    for target in &targets {
        if let SaveTarget::Restore { index, path } = target {
            copy_tree(&tmp.join(index.to_string()), path).map_err(RestoreError::Io)?;
        }
    }
    Ok(SavesRestored { targets })
}

/// Copies the folder `src` (regular files and folders only, as
/// [`archive::restore_tree`] leaves them) into `dest`, creating it.
fn copy_tree(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for item in fs::read_dir(src)? {
        let item = item?;
        let kind = item.file_type()?;
        let to = dest.join(item.file_name());
        if kind.is_dir() {
            copy_tree(&item.path(), &to)?;
        } else if kind.is_file() {
            fs::copy(item.path(), &to)?;
        }
    }
    Ok(())
}

/// Gives a restored game's save `folders` to cloud saves, keyed by its
/// `title` as cloud saves key it, without touching what is there:
/// - no PCGamingWiki entry yet: the folders become it, so no lookup is
///   needed;
/// - otherwise each folder in neither list (nor removed by the user) is
///   added to the manual list.
///
/// Returns whether `config` changed.
pub fn seed_folders(config: &mut Config, title: &str, folders: &[String]) -> bool {
    if folders.is_empty() || title.trim().is_empty() {
        return false;
    }
    let key = manual_paths_key_for_title(title);
    let Some(pcgw) = config.native_pcgw_save_paths.get(&key) else {
        config.native_pcgw_save_paths.insert(key, folders.to_vec());
        return true;
    };
    let removed = config
        .native_removed_save_paths
        .get(&key)
        .cloned()
        .unwrap_or_default();
    let fresh: Vec<String> = {
        let manual = config.native_manual_save_paths.get(&key);
        folders
            .iter()
            .filter(|folder| {
                !pcgw.contains(folder)
                    && !removed.contains(folder)
                    && !manual.is_some_and(|manual| manual.contains(folder))
            })
            .cloned()
            .collect()
    };
    if fresh.is_empty() {
        return false;
    }
    config
        .native_manual_save_paths
        .entry(key)
        .or_default()
        .extend(fresh);
    true
}

/// The saves record of a pack entry whose saves were not copied.
pub fn folders_only(folders: Vec<String>) -> PackSaves {
    PackSaves {
        folders,
        ..PackSaves::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::pack::PackPart;
    use crate::backup::write::WriteTarget;
    use crate::test_env::{lock, EnvGuard};

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // --- recording -----------------------------------------------------------

    #[test]
    fn prefix_paths_become_tokens() {
        let prefix = Path::new("/games/Foo/prefix");
        let user = prefix.join("drive_c/users/sam");
        let cases = [
            (user.join("AppData/Roaming/Foo"), "%APPDATA%\\Foo"),
            (
                user.join("AppData/Local/Foo/Saves"),
                "%LOCALAPPDATA%\\Foo\\Saves",
            ),
            (
                user.join("AppData/LocalLow/Dev/Foo"),
                "%USERPROFILE%\\AppData\\LocalLow\\Dev\\Foo",
            ),
            (
                user.join("Documents/My Games/Foo"),
                "%USERPROFILE%\\Documents\\My Games\\Foo",
            ),
            (
                user.join("Saved Games/Foo"),
                "%USERPROFILE%\\Saved Games\\Foo",
            ),
            (user.clone(), "%USERPROFILE%"),
            (prefix.join("drive_c/users/Public/Foo"), "%PUBLIC%\\Foo"),
            (prefix.join("drive_c/ProgramData/Foo"), "%PROGRAMDATA%\\Foo"),
            (prefix.join("drive_c/windows/Foo"), "%WINDIR%\\Foo"),
            (
                prefix.join("drive_c/Games/Foo/save"),
                "%SYSTEMDRIVE%\\Games\\Foo\\save",
            ),
            (
                prefix.join("drive_c/users/steamuser/appdata/roaming/Foo"),
                "%APPDATA%\\Foo",
            ),
        ];
        for (path, want) in cases {
            assert_eq!(
                token_for_prefix_path(&path, prefix).as_deref(),
                Some(want),
                "{}",
                path.display()
            );
        }
    }

    #[test]
    fn paths_outside_drive_c_have_no_token() {
        let prefix = Path::new("/games/Foo/prefix");
        assert_eq!(
            token_for_prefix_path(Path::new("/home/sam/saves"), prefix),
            None
        );
        assert_eq!(token_for_prefix_path(&prefix.join("drive_c"), prefix), None);
        assert_eq!(
            token_for_prefix_path(&prefix.join("dosdevices"), prefix),
            None
        );
        assert_eq!(
            token_for_prefix_path(&prefix.join("drive_c/../../etc"), prefix),
            None
        );
    }

    #[test]
    fn the_recorded_list_keeps_tokens_and_converts_or_drops_plain_paths() {
        let prefix = Path::new("/games/Foo/prefix");
        let home = Path::new("/home/sam");
        let pcgw = strings(&[
            "%APPDATA%\\Foo",
            "%USERPROFILE%\\Documents\\Foo",
            "%LOCALAPPDATA%\\Gone",
        ]);
        let manual = strings(&[
            "/games/Foo/prefix/drive_c/users/sam/AppData/Roaming/foo",
            "/home/sam/elsewhere",
            "%USERPROFILE%\\Saved Games\\Foo",
            "/games/Foo/prefix/drive_c/Games/Foo/save",
        ]);
        let removed = strings(&["%LOCALAPPDATA%\\Gone"]);
        let folders = recorded_folders(&pcgw, &manual, &removed, Some(prefix), Some(home));
        assert_eq!(
            folders,
            strings(&[
                "%APPDATA%\\Foo",
                "%USERPROFILE%\\Documents\\Foo",
                "%USERPROFILE%\\Saved Games\\Foo",
                "%SYSTEMDRIVE%\\Games\\Foo\\save",
            ]),
            "the converted manual path duplicates a PCGW one, without case"
        );
        let no_prefix = recorded_folders(&pcgw, &manual, &removed, None, Some(home));
        assert_eq!(
            no_prefix,
            strings(&[
                "%APPDATA%\\Foo",
                "%USERPROFILE%\\Documents\\Foo",
                "%USERPROFILE%\\Saved Games\\Foo",
            ])
        );
    }

    #[test]
    fn a_home_relative_prefix_and_manual_path_compare_expanded() {
        let home = Path::new("/home/sam");
        let manual = strings(&["~/Games/Foo/prefix/drive_c/users/sam/Documents/Foo"]);
        let folders = recorded_folders(
            &[],
            &manual,
            &[],
            Some(Path::new("~/Games/Foo/prefix")),
            Some(home),
        );
        assert_eq!(folders, strings(&["%USERPROFILE%\\Documents\\Foo"]));
    }

    #[test]
    fn token_detection() {
        assert!(is_token_path("%APPDATA%\\Foo"));
        assert!(is_token_path("%SystemDrive%"));
        assert!(!is_token_path("%%\\Foo"));
        assert!(!is_token_path("%APPDATA"));
        assert!(!is_token_path("/home/sam/%APPDATA%"));
        assert!(!is_token_path("%A B%\\Foo"));
    }

    // --- copy and restore ----------------------------------------------------

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn user_dir(prefix: &Path) -> PathBuf {
        prefix.join("drive_c").join("users").join("tester")
    }

    #[test]
    fn only_existing_save_folders_are_scanned_under_their_position() {
        let _lock = lock();
        let _env = EnvGuard::set(&[("LOGNAME", Some("tester"))]);
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        let user = user_dir(&prefix);
        write(&user.join("AppData/Roaming/Foo/slot1.sav"), b"one");
        write(&user.join("Documents/Foo/cfg/settings.ini"), b"[a]");
        write(&user.join("AppData/Roaming/Other/not-a-save"), b"x");
        write(&prefix.join("system.reg"), b"reg");
        let folders = strings(&[
            "%LOCALAPPDATA%\\Missing",
            "%APPDATA%\\Foo",
            "%USERPROFILE%\\Documents\\Foo",
            "%UNKNOWN%\\Foo",
        ]);
        let scan = scan_saves(&folders, &prefix, false);
        assert!(scan.refused.is_empty());
        let tree = scan.tree.expect("two folders exist");
        let names: Vec<&str> = tree.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            ["1/", "1/slot1.sav", "2/", "2/cfg/", "2/cfg/settings.ini"]
        );
        assert_eq!(tree.tree_files, 2);
        assert_eq!(tree.tree_bytes, 6);
    }

    #[test]
    fn nothing_to_scan_is_no_tree() {
        let _lock = lock();
        let _env = EnvGuard::set(&[("LOGNAME", Some("tester"))]);
        let dir = tempfile::tempdir().unwrap();
        let scan = scan_saves(&strings(&["%APPDATA%\\Foo"]), dir.path(), false);
        assert!(scan.tree.is_none());
        assert!(scan.refused.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_with_a_link_inside_is_left_out_alone() {
        let _lock = lock();
        let _env = EnvGuard::set(&[("LOGNAME", Some("tester"))]);
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        let user = user_dir(&prefix);
        write(&user.join("AppData/Roaming/Foo/slot1.sav"), b"one");
        write(&user.join("AppData/Roaming/Bar/real"), b"x");
        std::os::unix::fs::symlink("/etc", user.join("AppData/Roaming/Bar/link")).unwrap();
        let folders = strings(&["%APPDATA%\\Bar", "%APPDATA%\\Foo"]);
        let scan = scan_saves(&folders, &prefix, false);
        assert_eq!(
            scan.refused,
            vec![(
                "%APPDATA%\\Bar".to_string(),
                "it contains links".to_string()
            )]
        );
        let names: Vec<String> = scan
            .tree
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, ["1/", "1/slot1.sav"]);
    }

    #[test]
    fn the_windows_rule_skips_folders_that_hold_files() {
        let folders = strings(&["%APPDATA%\\A", "%APPDATA%\\B", "%APPDATA%\\C", "%NOPE%\\D"]);
        let docs = Path::new("C:\\Users\\sam\\Documents");
        let _lock = lock();
        let _env = EnvGuard::set(&[("APPDATA", Some("/win/AppData/Roaming")), ("NOPE", None)]);
        // Off Windows the expanded path keeps its backslash, so match the
        // text's last character, not a file name.
        let targets = restore_targets(&folders, &[0, 1, 2, 3], None, Some(docs), |path| {
            let text = path.to_string_lossy();
            if text.ends_with('A') {
                TargetState::Missing
            } else if text.ends_with('B') {
                TargetState::Empty
            } else {
                TargetState::NotEmpty
            }
        });
        assert_eq!(
            targets,
            vec![
                SaveTarget::Restore {
                    index: 0,
                    path: PathBuf::from("/win/AppData/Roaming\\A")
                },
                SaveTarget::Restore {
                    index: 1,
                    path: PathBuf::from("/win/AppData/Roaming\\B")
                },
                SaveTarget::Occupied {
                    index: 2,
                    path: PathBuf::from("/win/AppData/Roaming\\C")
                },
                SaveTarget::Unresolved { index: 3 },
            ]
        );
        let restored = SavesRestored { targets };
        assert_eq!(restored.warnings(&folders).len(), 2);
    }

    #[test]
    fn a_prefix_target_is_inside_the_new_prefix() {
        let _lock = lock();
        let _env = EnvGuard::set(&[("LOGNAME", Some("newuser"))]);
        let prefix = Path::new("/lib/games/windows/Foo/prefix");
        let targets = restore_targets(
            &strings(&["%APPDATA%\\Foo", "%NOPE%\\Bar"]),
            &[0, 1],
            Some(prefix),
            None,
            |_| TargetState::Missing,
        );
        assert_eq!(
            targets,
            vec![
                SaveTarget::Restore {
                    index: 0,
                    path: prefix.join("drive_c/users/newuser/AppData/Roaming/Foo")
                },
                SaveTarget::Unresolved { index: 1 },
            ]
        );
    }

    #[test]
    fn saves_round_trip_through_a_drive_into_a_new_prefix() {
        let _lock = lock();
        let _env = EnvGuard::set(&[("LOGNAME", Some("tester"))]);
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old/prefix");
        write(
            &user_dir(&old).join("AppData/Roaming/Foo/slot1.sav"),
            b"slot one",
        );
        write(&user_dir(&old).join("Documents/Foo/deep/cfg.ini"), b"cfg");
        let folders = strings(&[
            "%APPDATA%\\Foo",
            "%USERPROFILE%\\Documents\\Foo",
            "%APPDATA%\\Gone",
        ]);
        let tree = scan_saves(&folders, &old, false).tree.unwrap();

        let mount = dir.path().join("drive");
        fs::create_dir_all(mount.join("gridpack")).unwrap();
        let target = WriteTarget {
            mount: &mount,
            label: "DRIVE",
            fat32: true,
        };
        let cancel = AtomicBool::new(false);
        let parts: Vec<PackPart> = archive::write_saves_tree(
            &target,
            &tree,
            "saves/win/Foo",
            1024,
            &cancel,
            &mut |_, _| {},
        )
        .unwrap();
        assert!(parts.len() > 1, "small parts make several");
        assert_eq!(parts[0].path, "saves/win/Foo/saves.tar.001");

        let new = dir.path().join("new/prefix");
        // The new prefix already has a folder for the second save, with files.
        write(&user_dir(&new).join("Documents/Foo/keep.ini"), b"mine");
        let sources: Vec<PartSource> = parts
            .iter()
            .map(|part| PartSource {
                path: mount.join("gridpack").join(&part.path),
                size: part.size,
                md5: part.md5.clone(),
            })
            .collect();
        let tmp = dir.path().join("new/.grid-saves-1");
        let restored = restore_saves(
            &mount.join("gridpack"),
            &sources,
            &folders,
            &tmp,
            Some(&new),
            None,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            fs::read(user_dir(&new).join("AppData/Roaming/Foo/slot1.sav")).unwrap(),
            b"slot one"
        );
        assert!(!user_dir(&new).join("Documents/Foo/deep").exists());
        assert_eq!(
            fs::read(user_dir(&new).join("Documents/Foo/keep.ini")).unwrap(),
            b"mine"
        );
        assert!(!tmp.exists());
        let warnings = restored.warnings(&folders);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("Saves for %USERPROFILE%\\Documents\\Foo weren't restored"));
    }

    #[test]
    fn a_damaged_saves_archive_restores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("gridpack");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("saves.tar.001"), b"not a tar").unwrap();
        let sources = vec![PartSource {
            path: root.join("saves.tar.001"),
            size: 9,
            md5: "00".repeat(16),
        }];
        let tmp = dir.path().join("tmp");
        let prefix = dir.path().join("prefix");
        let result = restore_saves(
            &root,
            &sources,
            &strings(&["%APPDATA%\\Foo"]),
            &tmp,
            Some(&prefix),
            None,
            &AtomicBool::new(false),
        );
        assert!(result.is_err());
        assert!(!tmp.exists());
        assert!(!prefix.exists());
    }

    // --- cloud saves ---------------------------------------------------------

    #[test]
    fn seeding_fills_an_empty_pcgw_entry() {
        let mut config = Config::default();
        assert!(seed_folders(
            &mut config,
            " Foo ",
            &strings(&["%APPDATA%\\Foo"])
        ));
        assert_eq!(
            config.native_pcgw_save_paths.get("Foo__manual"),
            Some(&strings(&["%APPDATA%\\Foo"]))
        );
        assert!(config.native_manual_save_paths.is_empty());
    }

    #[test]
    fn seeding_never_clobbers_what_is_there() {
        let mut config = Config::default();
        config
            .native_pcgw_save_paths
            .insert("Foo__manual".into(), strings(&["%APPDATA%\\Foo"]));
        config
            .native_manual_save_paths
            .insert("Foo__manual".into(), strings(&["/home/sam/mine"]));
        config
            .native_removed_save_paths
            .insert("Foo__manual".into(), strings(&["%LOCALAPPDATA%\\Removed"]));
        let folders = strings(&[
            "%APPDATA%\\Foo",
            "%LOCALAPPDATA%\\Removed",
            "%USERPROFILE%\\New",
        ]);
        assert!(seed_folders(&mut config, "Foo", &folders));
        assert_eq!(
            config.native_pcgw_save_paths.get("Foo__manual"),
            Some(&strings(&["%APPDATA%\\Foo"]))
        );
        assert_eq!(
            config.native_manual_save_paths.get("Foo__manual"),
            Some(&strings(&["/home/sam/mine", "%USERPROFILE%\\New"]))
        );
        assert!(
            !seed_folders(&mut config, "Foo", &folders),
            "a second seed adds nothing"
        );
        assert!(!seed_folders(&mut config, "Foo", &[]));
    }
}
