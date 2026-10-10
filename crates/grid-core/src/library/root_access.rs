//! Can GRID reach the library root? (phase 9b, the unreachable-library
//! dialog.)
//!
//! A library root is one of three things:
//!
//! - **not set**: `library_path` is blank. That is a first run; the Connect
//!   and First Run Setup flows handle it, and nothing here applies.
//! - **reachable**: the folder exists, is a folder, and can be listed.
//! - **unreachable**: anything else — the folder is gone, its drive is not
//!   there (an unplugged USB disk, an unmounted share), the path is a file,
//!   or the folder cannot be read (permission denied, I/O error).
//!
//! An unreachable root only blocks the app when GRID has something stored
//! there ([`library_in_use`]): an installed game, an emulator entry, or a
//! library move record. A configured folder that GRID never used is
//! created again by the next install, so it is no reason to stop.
//!
//! The unreachable-library dialog offers exactly two choices: relocate the
//! library to the folder where it now is (checked by
//! [`validate_relocate_target`]; references are rewritten by
//! `relocate::relocate_references`, no file moves), or close the app
//! without changing anything.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::path_change::{expand, is_strictly_inside};
use super::paths::{EMULATORS_DIR, GAMES_DIR, SAVES_DIR};
use super::registry::InstalledGame;
use crate::config::Config;

/// Why the library root cannot be reached. Serialized as a snake_case tag;
/// [`Self::message`] is the sentence the dialog shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnreachableReason {
    /// The folder is not there, but its drive is.
    Missing,
    /// The drive or volume the folder lives on is not there.
    DriveMissing,
    /// The path names a file, not a folder.
    NotADirectory,
    /// GRID is not allowed to read the folder.
    PermissionDenied,
    /// Any other read error.
    Unreadable,
}

impl UnreachableReason {
    pub fn message(self) -> &'static str {
        match self {
            Self::Missing => "The library folder is not there any more.",
            Self::DriveMissing => "The drive that holds the library folder is not connected.",
            Self::NotADirectory => "The library path is a file, not a folder.",
            Self::PermissionDenied => "GRID is not allowed to open the library folder.",
            Self::Unreadable => "GRID cannot read the library folder.",
        }
    }
}

/// The state of a configured library root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootAccess {
    NotSet,
    Reachable,
    Unreachable(UnreachableReason),
}

/// What a look at the root found, before it is classified. Split out so
/// the classification is a pure table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// The path is a folder; `listed` is the result of listing it.
    Dir { listed: Result<(), io::ErrorKind> },
    /// The path exists and is not a folder.
    NotDir,
    /// Reading the path's metadata failed with this error.
    Failed {
        kind: io::ErrorKind,
        /// Whether the path's volume root (`E:\`, `\\server\share\`, `/`)
        /// exists.
        volume_exists: bool,
    },
}

/// The pure classification of a [`Probe`].
pub fn classify(probe: Probe) -> RootAccess {
    use UnreachableReason::*;
    let denied_or = |kind: io::ErrorKind, other: UnreachableReason| {
        if kind == io::ErrorKind::PermissionDenied {
            PermissionDenied
        } else {
            other
        }
    };
    RootAccess::Unreachable(match probe {
        Probe::Dir { listed: Ok(()) } => return RootAccess::Reachable,
        Probe::Dir { listed: Err(kind) } => denied_or(kind, Unreadable),
        Probe::NotDir => NotADirectory,
        Probe::Failed {
            volume_exists: false,
            ..
        } => DriveMissing,
        Probe::Failed {
            kind: io::ErrorKind::NotFound,
            ..
        } => Missing,
        Probe::Failed { kind, .. } => denied_or(kind, Unreadable),
    })
}

/// The volume root of an absolute path: its prefix and root (`E:\`,
/// `\\server\share\`, `/`). `None` for a relative path.
fn volume_root(path: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => root.push(component.as_os_str()),
            _ => break,
        }
    }
    (path.is_absolute() && !root.as_os_str().is_empty()).then_some(root)
}

/// Looks at the root `raw` (as stored; `~/` expanded against `home`).
pub fn library_root_access(raw: &str, home: Option<&Path>) -> RootAccess {
    if raw.trim().is_empty() {
        return RootAccess::NotSet;
    }
    let path = expand(raw, home);
    let probe = match fs::metadata(&path) {
        Ok(meta) if meta.is_dir() => Probe::Dir {
            listed: fs::read_dir(&path).map(|_| ()).map_err(|e| e.kind()),
        },
        Ok(_) => Probe::NotDir,
        Err(e) => Probe::Failed {
            kind: e.kind(),
            // A relative path has no volume to miss.
            volume_exists: volume_root(&path).is_none_or(|root| root.exists()),
        },
    };
    classify(probe)
}

/// Whether GRID stored anything in the library at `root`: an installed
/// game's files, or an emulator entry's executable, save or state folder.
pub fn library_in_use(
    config: &Config,
    rows: &[InstalledGame],
    root: &Path,
    home: Option<&Path>,
) -> bool {
    let inside = |raw: &str| {
        let raw = raw.trim();
        !raw.is_empty() && is_strictly_inside(&expand(raw, home), root, home)
    };
    let row_inside = |row: &InstalledGame| {
        [
            &row.archive_path,
            &row.extracted_path,
            &row.extracted_dir,
            &row.multi_file_game_dir,
            &row.native_game_dir,
            &row.ps3_iso_path,
        ]
        .into_iter()
        .any(|raw| inside(raw))
    };
    let list_inside = |value: &str| value.split([';', '\n']).any(&inside);
    rows.iter().any(row_inside)
        || config.emulators.iter().any(|entry| {
            inside(&entry.path) || list_inside(&entry.save_paths) || list_inside(&entry.state_paths)
        })
}

/// Why a folder cannot be the relocated library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocateRefusal {
    Blank,
    NotAbsolute,
    /// Not there, or not a folder.
    NotADirectory,
    /// It holds none of `games`, `emulators`, `saves`.
    NoLibraryFolders,
}

impl RelocateRefusal {
    pub fn message(self) -> &'static str {
        match self {
            Self::Blank => "Choose the folder where the library is now.",
            Self::NotAbsolute => {
                "Enter a full folder path, such as D:\\Games or /media/you/Games."
            }
            Self::NotADirectory => "This folder cannot be found.",
            Self::NoLibraryFolders => {
                "This folder has no games, emulators or saves folder, so it is not a GRID library. Choose the folder that holds them."
            }
        }
    }
}

/// Checks the folder the user picked as the library's new place: an
/// existing folder that holds at least one of `games`, `emulators`,
/// `saves`. Returns the folder as it will be stored.
pub fn validate_relocate_target(
    raw: &str,
    home: Option<&Path>,
) -> Result<PathBuf, RelocateRefusal> {
    if raw.trim().is_empty() {
        return Err(RelocateRefusal::Blank);
    }
    let path = expand(raw, home);
    if !path.is_absolute() {
        return Err(RelocateRefusal::NotAbsolute);
    }
    if !path.is_dir() {
        return Err(RelocateRefusal::NotADirectory);
    }
    // A link to a folder counts: `metadata` follows it.
    let has_library_folder = [GAMES_DIR, EMULATORS_DIR, SAVES_DIR]
        .iter()
        .any(|name| fs::metadata(path.join(name)).is_ok_and(|meta| meta.is_dir()));
    if !has_library_folder {
        return Err(RelocateRefusal::NoLibraryFolders);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EmulatorEntry;
    use io::ErrorKind;

    fn s(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn classify_table() {
        use RootAccess::*;
        use UnreachableReason::*;
        let cases: &[(Probe, RootAccess)] = &[
            (Probe::Dir { listed: Ok(()) }, Reachable),
            (
                Probe::Dir {
                    listed: Err(ErrorKind::PermissionDenied),
                },
                Unreachable(PermissionDenied),
            ),
            (
                Probe::Dir {
                    listed: Err(ErrorKind::Other),
                },
                Unreachable(Unreadable),
            ),
            (Probe::NotDir, Unreachable(NotADirectory)),
            (
                Probe::Failed {
                    kind: ErrorKind::NotFound,
                    volume_exists: true,
                },
                Unreachable(Missing),
            ),
            (
                Probe::Failed {
                    kind: ErrorKind::NotFound,
                    volume_exists: false,
                },
                Unreachable(DriveMissing),
            ),
            (
                Probe::Failed {
                    kind: ErrorKind::PermissionDenied,
                    volume_exists: true,
                },
                Unreachable(PermissionDenied),
            ),
            (
                Probe::Failed {
                    kind: ErrorKind::Other,
                    volume_exists: true,
                },
                Unreachable(Unreadable),
            ),
            (
                Probe::Failed {
                    kind: ErrorKind::Other,
                    volume_exists: false,
                },
                Unreachable(DriveMissing),
            ),
        ];
        for (probe, expected) in cases {
            assert_eq!(classify(*probe), *expected, "{probe:?}");
        }
    }

    #[test]
    fn access_on_the_real_file_system() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, b"x").unwrap();
        assert_eq!(library_root_access("", None), RootAccess::NotSet);
        assert_eq!(library_root_access("   ", None), RootAccess::NotSet);
        assert_eq!(
            library_root_access(&s(dir.path()), None),
            RootAccess::Reachable
        );
        assert_eq!(
            library_root_access(&s(&dir.path().join("gone")), None),
            RootAccess::Unreachable(UnreachableReason::Missing)
        );
        assert_eq!(
            library_root_access(&s(&file), None),
            RootAccess::Unreachable(UnreachableReason::NotADirectory)
        );
        // `~/` expands against the home handed in.
        assert_eq!(
            library_root_access("~/", Some(dir.path())),
            RootAccess::Reachable
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_missing_drive_letter_is_a_missing_drive() {
        // Pick a drive letter that is not mounted.
        let letter = ('D'..='Z')
            .rev()
            .find(|c| !Path::new(&format!("{c}:\\")).exists())
            .expect("a free drive letter");
        assert_eq!(
            library_root_access(&format!("{letter}:\\Games"), None),
            RootAccess::Unreachable(UnreachableReason::DriveMissing)
        );
    }

    #[test]
    fn volume_root_of_absolute_paths() {
        assert_eq!(volume_root(Path::new("relative/x")), None);
        #[cfg(windows)]
        assert_eq!(
            volume_root(Path::new(r"E:\Games\x")),
            Some(PathBuf::from(r"E:\"))
        );
        #[cfg(unix)]
        assert_eq!(
            volume_root(Path::new("/media/u/x")),
            Some(PathBuf::from("/"))
        );
    }

    #[test]
    fn in_use_when_a_row_or_an_emulator_lives_under_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Lib");
        let other = dir.path().join("Other");
        let row_in = InstalledGame {
            title: "A".into(),
            platform: "SNES".into(),
            extracted_dir: s(&root.join("games").join("SNES").join("A")),
            ..Default::default()
        };
        let row_out = InstalledGame {
            title: "B".into(),
            platform: "SNES".into(),
            extracted_dir: s(&other.join("B")),
            ..Default::default()
        };
        // A row with no recorded location says nothing about a missing root.
        let row_blank = InstalledGame {
            title: "C".into(),
            platform: "SNES".into(),
            ..Default::default()
        };
        let empty = Config::default();
        assert!(!library_in_use(&empty, &[], &root, None));
        assert!(!library_in_use(
            &empty,
            &[row_out.clone(), row_blank],
            &root,
            None
        ));
        assert!(library_in_use(&empty, &[row_out, row_in], &root, None));

        let with_emulator = |entry: EmulatorEntry| Config {
            emulators: vec![entry],
            ..Default::default()
        };
        let exe = with_emulator(EmulatorEntry {
            name: "Emu".into(),
            path: s(&root.join("emulators").join("Emu").join("emu.exe")),
            ..Default::default()
        });
        assert!(library_in_use(&exe, &[], &root, None));
        let saves = with_emulator(EmulatorEntry {
            name: "Emu".into(),
            path: s(&other.join("emu.exe")),
            save_paths: format!("{};{}", s(&other), s(&root.join("saves").join("Emu"))),
            ..Default::default()
        });
        assert!(library_in_use(&saves, &[], &root, None));
        let states = with_emulator(EmulatorEntry {
            name: "Emu".into(),
            path: s(&other.join("emu.exe")),
            state_paths: s(&root.join("saves").join("Emu").join("states")),
            ..Default::default()
        });
        assert!(library_in_use(&states, &[], &root, None));
        let outside = with_emulator(EmulatorEntry {
            name: "Emu".into(),
            path: s(&other.join("emu.exe")),
            ..Default::default()
        });
        assert!(!library_in_use(&outside, &[], &root, None));
    }

    #[test]
    fn relocate_target_needs_a_folder_with_library_folders() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, b"x").unwrap();
        assert_eq!(
            validate_relocate_target("  ", None),
            Err(RelocateRefusal::Blank)
        );
        assert_eq!(
            validate_relocate_target("relative/lib", None),
            Err(RelocateRefusal::NotAbsolute)
        );
        assert_eq!(
            validate_relocate_target(&s(&dir.path().join("gone")), None),
            Err(RelocateRefusal::NotADirectory)
        );
        assert_eq!(
            validate_relocate_target(&s(&file), None),
            Err(RelocateRefusal::NotADirectory)
        );
        assert_eq!(
            validate_relocate_target(&s(&empty), None),
            Err(RelocateRefusal::NoLibraryFolders)
        );
        // Any one of the three is enough; a file named `games` is not.
        let only_file = dir.path().join("only_file");
        fs::create_dir_all(&only_file).unwrap();
        fs::write(only_file.join("games"), b"x").unwrap();
        assert_eq!(
            validate_relocate_target(&s(&only_file), None),
            Err(RelocateRefusal::NoLibraryFolders)
        );
        for name in ["games", "emulators", "saves"] {
            let lib = dir.path().join(format!("lib-{name}"));
            fs::create_dir_all(lib.join(name)).unwrap();
            assert_eq!(
                validate_relocate_target(&format!("  {}  ", s(&lib)), None),
                Ok(lib.clone()),
                "{name}"
            );
        }
        // `~/` expands against the home handed in.
        let home_lib = dir.path().join("lib-games");
        assert_eq!(
            validate_relocate_target("~/lib-games", Some(dir.path())),
            Ok(home_lib)
        );
    }
}
