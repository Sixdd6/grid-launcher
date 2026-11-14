//! Where Steam is: its root folders and the executable GRID hands
//! `steam://` links to (`docs/stores-phase4-plan.md` "Where Steam is" and
//! "Which executable").
//!
//! The candidate lists and the executable choice are pure functions of a
//! home folder, the environment and a file check, so they are tested with
//! temp folders on every OS. The Windows registry reads (`HKCU`
//! `SteamPath` / `SteamExe`, then `HKLM` `InstallPath`) are
//! `cfg(windows)`.

use std::path::{Path, PathBuf};

use super::{ClientExe, SteamClient};

/// The Flatpak's application id.
pub const FLATPAK_ID: &str = "com.valvesoftware.Steam";

/// The Snap's package name.
pub const SNAP_NAME: &str = "steam";

/// The Snap's launcher.
pub const SNAP_BIN: &str = "/snap/bin/steam";

/// Linux Steam roots, in steamlocate 2.1.1's order: the Flatpak's three,
/// `$XDG_DATA_HOME/Steam` (default `~/.local/share/Steam`), the three
/// `~/.steam` links, then the Snap's three (`$SNAP_USER_DATA` or
/// `~/snap`). Unchecked: [`existing_roots`] keeps the real ones.
pub fn linux_root_candidates(
    home: &Path,
    xdg_data_home: Option<&Path>,
    snap_user_data: Option<&Path>,
) -> Vec<PathBuf> {
    let flatpak = home.join(".var").join("app").join(FLATPAK_ID);
    let data_home = xdg_data_home
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join(".local").join("share"));
    let snap = snap_user_data
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("snap"))
        .join(SNAP_NAME)
        .join("common");
    vec![
        flatpak.join(".local").join("share").join("Steam"),
        flatpak.join(".steam").join("steam"),
        flatpak.join(".steam").join("root"),
        data_home.join("Steam"),
        home.join(".steam").join("steam"),
        home.join(".steam").join("root"),
        home.join(".steam").join("debian-installation"),
        snap.join(".local").join("share").join("Steam"),
        snap.join(".steam").join("steam"),
        snap.join(".steam").join("root"),
    ]
}

/// The macOS root (kept for a later universal build; not tested on a Mac).
pub fn macos_root_candidates(home: &Path) -> Vec<PathBuf> {
    vec![home
        .join("Library")
        .join("Application Support")
        .join("Steam")]
}

/// The candidates that are Steam roots (they hold a `steamapps/` folder),
/// in order, each folder once: two candidates that reach the same folder
/// through a symlink count once (the first spelling is kept).
pub fn existing_roots(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut roots = Vec::new();
    for candidate in candidates {
        if candidate.as_os_str().is_empty() || !candidate.join("steamapps").is_dir() {
            continue;
        }
        let resolved = std::fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if seen.contains(&resolved) {
            continue;
        }
        seen.push(resolved);
        roots.push(candidate);
    }
    roots
}

/// How a root was installed, read from where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    /// Under `~/.var/app/com.valvesoftware.Steam/`.
    Flatpak,
    /// Under `<snap>/steam/common/`.
    Snap,
    /// Anything else (a native install, Windows, macOS, a test root).
    Native,
}

/// Whether `root` belongs to the Flatpak, the Snap or a native install.
pub fn root_kind(root: &Path) -> RootKind {
    let parts: Vec<&std::ffi::OsStr> = root.iter().collect();
    let has_run = |run: &[&str]| {
        parts
            .windows(run.len())
            .any(|window| window.iter().zip(run).all(|(part, want)| *part == *want))
    };
    if has_run(&[".var", "app", FLATPAK_ID]) {
        RootKind::Flatpak
    } else if has_run(&["snap", SNAP_NAME, "common"]) {
        RootKind::Snap
    } else {
        RootKind::Native
    }
}

/// The executable for a root, with what Settings shows for it.
///
/// - Flatpak root: `flatpak run com.valvesoftware.Steam`, shown as
///   "Flatpak (com.valvesoftware.Steam)" (user decision 3);
/// - Snap root: `/snap/bin/steam` when it exists, shown as "Snap (steam)";
/// - otherwise `steam` on `PATH`, else `<root>/steam.sh`, else
///   `~/.steam/steam.sh`, shown as its path;
/// - nothing found: `xdg-open` (the OS's link opener), shown as the root.
///
/// `on_path` looks a program name up on `PATH`; `is_file` checks a path.
pub fn choose_exe(
    root: &Path,
    home: Option<&Path>,
    on_path: &dyn Fn(&str) -> Option<PathBuf>,
    is_file: &dyn Fn(&Path) -> bool,
) -> (ClientExe, String) {
    match root_kind(root) {
        RootKind::Flatpak => {
            return (ClientExe::Flatpak, format!("Flatpak ({FLATPAK_ID})"));
        }
        RootKind::Snap => {
            let snap = PathBuf::from(SNAP_BIN);
            if is_file(&snap) {
                return (ClientExe::Path(snap), format!("Snap ({SNAP_NAME})"));
            }
        }
        RootKind::Native => {}
    }
    let found = on_path("steam")
        .or_else(|| Some(root.join("steam.sh")).filter(|path| is_file(path)))
        .or_else(|| {
            home.map(|home| home.join(".steam").join("steam.sh"))
                .filter(|path| is_file(path))
        });
    match found {
        Some(path) => {
            let display = path.display().to_string();
            (ClientExe::Path(path), display)
        }
        None => (ClientExe::XdgOpen, root.display().to_string()),
    }
}

/// `name` in one of `path_var`'s folders, as a file.
pub fn find_on_path(name: &str, path_var: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    std::env::split_paths(path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The real client: the first root this OS has, and its executable.
pub fn locate() -> Option<SteamClient> {
    #[cfg(windows)]
    {
        windows::locate()
    }
    #[cfg(not(windows))]
    {
        let home = crate::platform::home_dir()?;
        let candidates = if cfg!(target_os = "macos") {
            macos_root_candidates(&home)
        } else {
            let env_path = |name: &str| {
                std::env::var_os(name)
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            };
            linux_root_candidates(
                &home,
                env_path("XDG_DATA_HOME").as_deref(),
                env_path("SNAP_USER_DATA").as_deref(),
            )
        };
        let root = existing_roots(candidates).into_iter().next()?;
        let path_var = std::env::var_os("PATH");
        let (exe, display) = choose_exe(
            &root,
            Some(&home),
            &|name| find_on_path(name, path_var.as_deref()),
            &|path| path.is_file(),
        );
        Some(SteamClient { root, exe, display })
    }
}

/// The client tests and e2e name: the first given root that is a Steam
/// root, and the given executable, else `xdg-open`. Never reads `PATH` or
/// a real root.
pub fn from_override(roots: &[PathBuf], exe: Option<&Path>) -> Option<SteamClient> {
    let root = existing_roots(roots.iter().cloned()).into_iter().next()?;
    let (exe, display) = match exe {
        Some(path) => (
            ClientExe::Path(path.to_path_buf()),
            path.display().to_string(),
        ),
        None => (ClientExe::XdgOpen, root.display().to_string()),
    };
    Some(SteamClient { root, exe, display })
}

#[cfg(windows)]
mod windows {
    //! `HKCU\Software\Valve\Steam` `SteamPath` and `SteamExe` (Playnite's
    //! choice), then `HKLM` `InstallPath` (steamlocate's).

    use std::path::PathBuf;

    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    use super::super::{ClientExe, SteamClient};
    use super::existing_roots;

    fn hkcu_value(name: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Valve\\Steam", KEY_READ)
            .ok()?
            .get_value::<String, _>(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    fn hklm_install_path() -> Option<String> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        hklm.open_subkey_with_flags("SOFTWARE\\Wow6432Node\\Valve\\Steam", KEY_READ)
            .or_else(|_| hklm.open_subkey_with_flags("SOFTWARE\\Valve\\Steam", KEY_READ))
            .ok()?
            .get_value::<String, _>("InstallPath")
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    pub(super) fn locate() -> Option<SteamClient> {
        let candidates = [hkcu_value("SteamPath"), hklm_install_path()]
            .into_iter()
            .flatten()
            .map(PathBuf::from);
        let root = existing_roots(candidates).into_iter().next()?;
        let exe = hkcu_value("SteamExe")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| Some(root.join("steam.exe")).filter(|path| path.is_file()));
        let (exe, display) = match exe {
            Some(path) => {
                let display = path.display().to_string();
                (ClientExe::Path(path), display)
            }
            None => (ClientExe::XdgOpen, root.display().to_string()),
        };
        Some(SteamClient { root, exe, display })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn root_at(path: &Path) {
        fs::create_dir_all(path.join("steamapps")).unwrap();
    }

    #[test]
    fn linux_candidates_follow_steamlocates_order() {
        let home = Path::new("/home/u");
        let list = linux_root_candidates(home, None, None);
        assert_eq!(
            list,
            [
                "/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam",
                "/home/u/.var/app/com.valvesoftware.Steam/.steam/steam",
                "/home/u/.var/app/com.valvesoftware.Steam/.steam/root",
                "/home/u/.local/share/Steam",
                "/home/u/.steam/steam",
                "/home/u/.steam/root",
                "/home/u/.steam/debian-installation",
                "/home/u/snap/steam/common/.local/share/Steam",
                "/home/u/snap/steam/common/.steam/steam",
                "/home/u/snap/steam/common/.steam/root",
            ]
            .map(PathBuf::from)
        );
        let custom =
            linux_root_candidates(home, Some(Path::new("/data")), Some(Path::new("/snapdata")));
        assert_eq!(custom[3], Path::new("/data").join("Steam"));
        assert_eq!(
            custom[7],
            Path::new("/snapdata/steam/common/.local/share/Steam")
        );
    }

    #[test]
    fn only_folders_with_steamapps_count_once() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        let empty = tmp.path().join("empty");
        root_at(&a);
        root_at(&b);
        fs::create_dir_all(&empty).unwrap();
        let roots = existing_roots([
            empty.clone(),
            tmp.path().join("missing"),
            a.clone(),
            b.clone(),
            a.clone(),
        ]);
        assert_eq!(roots, vec![a, b]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_root_counts_once() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join(".local/share/Steam");
        root_at(&real);
        fs::create_dir_all(tmp.path().join(".steam")).unwrap();
        let link = tmp.path().join(".steam/steam");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let roots = existing_roots([real.clone(), link]);
        assert_eq!(roots, vec![real]);
    }

    #[test]
    fn root_kinds() {
        assert_eq!(
            root_kind(Path::new(
                "/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam"
            )),
            RootKind::Flatpak
        );
        assert_eq!(
            root_kind(Path::new("/home/u/snap/steam/common/.steam/steam")),
            RootKind::Snap
        );
        assert_eq!(
            root_kind(Path::new("/home/u/.local/share/Steam")),
            RootKind::Native
        );
        assert_eq!(
            root_kind(Path::new("/home/u/.var/app/org.other/.local/share/Steam")),
            RootKind::Native
        );
    }

    #[test]
    fn the_executable_per_root_layout() {
        let home = Path::new("/home/u");
        let none = |_: &str| None;
        let no_file = |_: &Path| false;

        let flatpak = home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam");
        assert_eq!(
            choose_exe(&flatpak, Some(home), &none, &no_file),
            (
                ClientExe::Flatpak,
                "Flatpak (com.valvesoftware.Steam)".to_string()
            )
        );

        let snap = home.join("snap/steam/common/.local/share/Steam");
        let snap_bin = |path: &Path| path == Path::new(SNAP_BIN);
        assert_eq!(
            choose_exe(&snap, Some(home), &none, &snap_bin),
            (
                ClientExe::Path(PathBuf::from(SNAP_BIN)),
                "Snap (steam)".to_string()
            )
        );

        let native = home.join(".local/share/Steam");
        let usr_bin = PathBuf::from("/usr/bin/steam");
        let on_path = |name: &str| (name == "steam").then(|| usr_bin.clone());
        assert_eq!(
            choose_exe(&native, Some(home), &on_path, &no_file),
            (
                ClientExe::Path(usr_bin.clone()),
                usr_bin.display().to_string()
            )
        );

        let root_sh = native.join("steam.sh");
        let has_root_sh = |path: &Path| path == root_sh;
        assert_eq!(
            choose_exe(&native, Some(home), &none, &has_root_sh).0,
            ClientExe::Path(root_sh.clone())
        );

        let home_sh = home.join(".steam/steam.sh");
        let has_home_sh = |path: &Path| path == home_sh;
        assert_eq!(
            choose_exe(&native, Some(home), &none, &has_home_sh).0,
            ClientExe::Path(home_sh.clone())
        );

        assert_eq!(
            choose_exe(&native, Some(home), &none, &no_file),
            (ClientExe::XdgOpen, native.display().to_string())
        );
        // A Snap root without the Snap's launcher falls back like a native one.
        assert_eq!(
            choose_exe(&snap, Some(home), &on_path, &no_file).0,
            ClientExe::Path(usr_bin)
        );
    }

    #[test]
    fn path_lookup_finds_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        let dir_only = tmp.path().join("other");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(dir_only.join("steam")).unwrap();
        fs::write(bin.join("steam"), "").unwrap();
        let path_var = std::env::join_paths([dir_only, bin.clone()]).unwrap();
        assert_eq!(
            find_on_path("steam", Some(&path_var)),
            Some(bin.join("steam"))
        );
        assert_eq!(find_on_path("nothing", Some(&path_var)), None);
        assert_eq!(find_on_path("steam", None), None);
    }

    #[test]
    fn the_override_reads_only_what_it_is_given() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        root_at(&root);
        let exe = tmp.path().join("fake-steam");
        let client =
            from_override(&[tmp.path().join("missing"), root.clone()], Some(&exe)).unwrap();
        assert_eq!(client.root, root);
        assert_eq!(client.exe, ClientExe::Path(exe.clone()));
        assert_eq!(client.display, exe.display().to_string());
        let bare = from_override(std::slice::from_ref(&root), None).unwrap();
        assert_eq!(bare.exe, ClientExe::XdgOpen);
        assert_eq!(bare.display, root.display().to_string());
        assert!(from_override(&[], Some(&exe)).is_none());
    }
}
