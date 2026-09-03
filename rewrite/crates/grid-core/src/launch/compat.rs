//! Wine/Proton compat-tool discovery: system Wine on `PATH`, Steam-managed
//! Proton installs, and the compat tools GRID itself downloads and manages.
//! Ports `_scan_system_proton_installs` and `_available_compat_tools_for_dialog`
//! (`grid_launcher/ui/mixins/emulator_ui_mixin.py:200-253`); see
//! `docs/porting/04-emulator-launch.md` §10.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{data_dir_override, CompatToolInstall};

/// One compat tool offered to the user (§10 "Compat tool (runtime list
/// entry)"). Serialized straight to the frontend by a later task's
/// `list_compat_tools` command, so field names are part of the IPC
/// contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompatTool {
    pub name: String,
    /// `"wine"` or `"proton"`.
    pub kind: String,
    pub path: String,
    /// `"system"` (Wine on `PATH`), `"steam"` (a Steam `compatibilitytools.d`
    /// install), or `"managed"` (GRID installed and manages it).
    pub source: String,
}

/// The three Steam `compatibilitytools.d` roots scanned for Proton installs,
/// in the reference's exact order (`_scan_system_proton_installs`,
/// emulator_ui_mixin.py:200-211).
pub fn steam_roots(home: &Path) -> [PathBuf; 3] {
    [
        home.join(".steam")
            .join("steam")
            .join("compatibilitytools.d"),
        home.join(".local")
            .join("share")
            .join("Steam")
            .join("compatibilitytools.d"),
        home.join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join("data")
            .join("Steam")
            .join("compatibilitytools.d"),
    ]
}

/// Where GRID installs and manages compat tools it downloads (D15):
/// `<data dir override>/compat-tools`, else
/// `<XDG_DATA_HOME or ~/.local/share>/grid-launcher/compat-tools`.
pub fn managed_root() -> PathBuf {
    if let Some(dir) = data_dir_override() {
        return dir.join("compat-tools");
    }
    directories::BaseDirs::new()
        .expect("home directory must exist")
        .data_dir()
        .join("grid-launcher")
        .join("compat-tools")
}

/// Whether `dir` itself, or its first (sorted) child directory, contains a
/// file named `proton` — the on-disk marker of a Proton install directory.
pub fn find_proton_dir(install_dir: &Path) -> Option<PathBuf> {
    if install_dir.join("proton").is_file() {
        return Some(install_dir.to_path_buf());
    }
    let mut children: Vec<PathBuf> = fs::read_dir(install_dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    children.sort();
    children
        .into_iter()
        .find(|dir| dir.join("proton").is_file())
}

/// `PATH` lookup for [`discover`]'s `which` argument, re-exported here so
/// the app layer can pass it in: the real implementation lives in
/// [`crate::library::extract`] and is `pub(crate)`.
pub fn which_on_path(name: &str) -> Option<PathBuf> {
    crate::library::extract::which_on_path(name)
}

/// Discovers the compat tools available to offer, in the reference's order
/// (`_available_compat_tools_for_dialog`, emulator_ui_mixin.py:231-253),
/// minus its leading "None" sentinel (a UI-only entry, not this layer's
/// concern):
///
/// 1. `host` starting with `"win"` -> `[]` (no compat tools make sense on a
///    Windows host).
/// 2. `which("wine")` found -> `{name: "Wine (system)", kind: "wine", path:
///    "wine", source: "system"}`.
/// 3. Every subdirectory of the three [`steam_roots`], sorted, that contains
///    a `proton` file: canonicalized, deduplicated by canonical path, and
///    skipped when that canonical path equals a managed install's *raw*
///    recorded path (matching the reference, which never canonicalizes the
///    managed side of that comparison).
/// 4. Every managed install with a non-blank path.
pub fn discover(
    home: &Path,
    managed: &[CompatToolInstall],
    host: &str,
    which: &dyn Fn(&str) -> Option<PathBuf>,
) -> Vec<CompatTool> {
    if host.trim().to_lowercase().starts_with("win") {
        return Vec::new();
    }

    let mut tools = Vec::new();

    if which("wine").is_some() {
        tools.push(CompatTool {
            name: "Wine (system)".to_string(),
            kind: "wine".to_string(),
            path: "wine".to_string(),
            source: "system".to_string(),
        });
    }

    let managed_paths: HashSet<&str> = managed
        .iter()
        .map(|install| install.path.as_str())
        .filter(|path| !path.is_empty())
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    for root in steam_roots(home) {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut subdirs: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        subdirs.sort();

        for subdir in subdirs {
            if !subdir.join("proton").is_file() {
                continue;
            }
            let Ok(real) = fs::canonicalize(&subdir) else {
                continue;
            };
            let real_str = real.to_string_lossy().into_owned();
            if managed_paths.contains(real_str.as_str()) {
                continue;
            }
            if !seen.insert(real_str.clone()) {
                continue;
            }
            let name = subdir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            tools.push(CompatTool {
                name,
                kind: "proton".to_string(),
                path: real_str,
                source: "steam".to_string(),
            });
        }
    }

    for install in managed {
        if install.path.trim().is_empty() {
            continue;
        }
        tools.push(CompatTool {
            name: install.name.clone(),
            kind: "proton".to_string(),
            path: install.path.clone(),
            source: "managed".to_string(),
        });
    }

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    fn managed(name: &str, path: &str) -> CompatToolInstall {
        CompatToolInstall {
            name: name.to_string(),
            path: path.to_string(),
            source_id: String::new(),
            release_tag: String::new(),
        }
    }

    fn no_which(_name: &str) -> Option<PathBuf> {
        None
    }

    fn found_wine(name: &str) -> Option<PathBuf> {
        (name == "wine").then(|| PathBuf::from("/usr/bin/wine"))
    }

    // --- host gate -------------------------------------------------------

    #[test]
    fn a_windows_host_returns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let tools = discover(dir.path(), &[], "windows", &found_wine);
        assert!(tools.is_empty());
    }

    // --- system wine -------------------------------------------------------

    #[test]
    fn system_wine_is_offered_when_found() {
        let dir = tempfile::tempdir().unwrap();
        let tools = discover(dir.path(), &[], "linux", &found_wine);
        assert_eq!(
            tools,
            vec![CompatTool {
                name: "Wine (system)".to_string(),
                kind: "wine".to_string(),
                path: "wine".to_string(),
                source: "system".to_string(),
            }]
        );
    }

    #[test]
    fn system_wine_is_omitted_when_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let tools = discover(dir.path(), &[], "linux", &no_which);
        assert!(tools.is_empty());
    }

    // --- steam roots ---------------------------------------------------------

    /// Creates `<root>/<name>/proton` as a file, returning the subdir path.
    fn proton_install(root: &Path, name: &str) -> PathBuf {
        let subdir = root.join(name);
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("proton"), b"proton launcher script").unwrap();
        subdir
    }

    #[test]
    fn steam_installs_are_found_canonicalized_and_sorted() {
        let home = tempfile::tempdir().unwrap();
        let roots = steam_roots(home.path());
        fs::create_dir_all(&roots[0]).unwrap();
        proton_install(&roots[0], "GE-Proton9");
        proton_install(&roots[0], "Proton-CachyOS");
        // A subdir with no `proton` file must not qualify.
        fs::create_dir_all(roots[0].join("not-a-proton-dir")).unwrap();

        let tools = discover(home.path(), &[], "linux", &no_which);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        // Sorted subdir order within the root.
        assert_eq!(names, vec!["GE-Proton9", "Proton-CachyOS"]);
        for tool in &tools {
            assert_eq!(tool.kind, "proton");
            assert_eq!(tool.source, "steam");
            let expected = fs::canonicalize(roots[0].join(&tool.name)).unwrap();
            assert_eq!(tool.path, expected.to_string_lossy());
        }
    }

    #[test]
    fn duplicate_steam_roots_via_symlink_are_deduplicated() {
        let home = tempfile::tempdir().unwrap();
        let roots = steam_roots(home.path());
        fs::create_dir_all(&roots[0]).unwrap();
        proton_install(&roots[0], "GE-Proton9");

        // The second root is a symlink to the first, so its scan resolves to
        // the exact same canonical subdir path.
        if let Some(parent) = roots[1].parent() {
            fs::create_dir_all(parent).unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&roots[0], &roots[1]).unwrap();

        let tools = discover(home.path(), &[], "linux", &no_which);
        assert_eq!(
            tools.len(),
            1,
            "expected the symlinked root to dedup: {tools:?}"
        );
        assert_eq!(tools[0].name, "GE-Proton9");
    }

    #[test]
    fn a_steam_install_matching_a_managed_path_is_skipped() {
        let home = tempfile::tempdir().unwrap();
        let roots = steam_roots(home.path());
        fs::create_dir_all(&roots[0]).unwrap();
        let subdir = proton_install(&roots[0], "GE-Proton9");
        let canonical = fs::canonicalize(&subdir).unwrap();

        let installs = vec![managed("GE-Proton9", &canonical.to_string_lossy())];
        let tools = discover(home.path(), &installs, "linux", &no_which);
        // Only the managed entry is offered, not a duplicate steam entry.
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].source, "managed");
    }

    // --- managed installs ----------------------------------------------------

    #[test]
    fn managed_installs_with_blank_paths_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let installs = vec![managed("Blank", ""), managed("Real", "/opt/real-proton")];
        let tools = discover(dir.path(), &installs, "linux", &no_which);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "Real");
        assert_eq!(tools[0].kind, "proton");
        assert_eq!(tools[0].source, "managed");
        assert_eq!(tools[0].path, "/opt/real-proton");
    }

    #[test]
    fn full_order_is_wine_then_steam_then_managed() {
        let home = tempfile::tempdir().unwrap();
        let roots = steam_roots(home.path());
        fs::create_dir_all(&roots[0]).unwrap();
        proton_install(&roots[0], "GE-Proton9");
        let installs = vec![managed("Managed-Proton", "/opt/managed-proton")];

        let tools = discover(home.path(), &installs, "linux", &found_wine);
        let sources: Vec<&str> = tools.iter().map(|t| t.source.as_str()).collect();
        assert_eq!(sources, vec!["system", "steam", "managed"]);
    }

    // --- managed_root ----------------------------------------------------

    #[test]
    fn managed_root_honors_the_data_dir_override() {
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let path_str = dir.path().to_string_lossy().into_owned();
        let _guard =
            crate::test_env::EnvGuard::set(&[("GRID_LAUNCHER_DATA_DIR", Some(path_str.as_str()))]);
        assert_eq!(managed_root(), dir.path().join("compat-tools"));
    }

    // --- find_proton_dir -------------------------------------------------

    #[test]
    fn find_proton_dir_matches_the_directory_itself() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("proton"), b"x").unwrap();
        assert_eq!(find_proton_dir(dir.path()), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn find_proton_dir_matches_the_first_sorted_child() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("b-child");
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("proton"), b"x").unwrap();
        let a = dir.path().join("a-child");
        fs::create_dir_all(&a).unwrap();
        fs::write(a.join("proton"), b"x").unwrap();
        // A third child with no proton file must not match.
        fs::create_dir_all(dir.path().join("c-child")).unwrap();

        assert_eq!(find_proton_dir(dir.path()), Some(a));
    }

    #[test]
    fn find_proton_dir_none_when_nothing_matches() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("child")).unwrap();
        assert_eq!(find_proton_dir(dir.path()), None);
    }
}
