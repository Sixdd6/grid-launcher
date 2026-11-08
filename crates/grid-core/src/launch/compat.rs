//! Wine/Proton compat-tool discovery: system Wine on `PATH`, Steam-managed
//! Proton installs, and the compat tools GRID itself downloads and manages.
//! Ports `_scan_system_proton_installs` and `_available_compat_tools_for_dialog`
//! (`grid_launcher/ui/mixins/emulator_ui_mixin.py:200-253`); see
//! `docs/porting/04-emulator-launch.md` §10.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{data_dir_override, CompatToolInstall, Config};

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

/// Settings › Systems › Windows, a Download pick: remembers `source_id` as
/// the compat tool to make the default once it is installed, then applies it
/// at once when GRID already has that tool ([`apply_pending_compat_default`]).
/// `Some(path)` means the default is set now and nothing needs installing;
/// `None` means the caller queues the install, and the install's record
/// applies it when it lands. The user picked it, so it replaces whatever
/// default was there.
pub fn request_compat_default(config: &mut Config, source_id: &str) -> Option<String> {
    config.setup.pending_compat = source_id.trim().to_string();
    apply_pending_compat_default(config)
}

/// Makes the pending compat tool ([`request_compat_default`]) the default
/// once a managed install with its `source_id` is recorded, and clears the
/// pending choice. Returns the path now stored in `default_compat_tool`, or
/// `None` when nothing is pending or its install has not landed yet (the
/// choice is kept, so a retried install still gets it).
pub fn apply_pending_compat_default(config: &mut Config) -> Option<String> {
    let wanted = config.setup.pending_compat.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let path = config
        .compat_tool_installs
        .iter()
        .find(|install| {
            install.source_id.trim().to_lowercase() == wanted && !install.path.trim().is_empty()
        })
        .map(|install| install.path.clone())?;
    config.default_compat_tool = path.clone();
    config.setup.pending_compat.clear();
    Some(path)
}

/// The user picked a compat tool that is already on the computer: any
/// download still waiting to become the default is no longer wanted.
pub fn clear_pending_compat_default(config: &mut Config) {
    config.setup.pending_compat.clear();
}

/// First-run setup's Windows row (Linux): the default Proton version the
/// user picked. `Installed` is a tool already on the computer, by the path
/// [`CompatTool::path`] gives (what `default_compat_tool` stores);
/// `Download` is a compat-tool catalog entry, by its `source_id`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetupCompatChoice {
    Installed { path: String },
    Download { source_id: String },
}

/// What [`apply_setup_compat_choice`] leaves to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupCompatPlan {
    /// The default is this path now: nothing to install.
    Set(String),
    /// Queue this catalog install; it becomes the default when it lands
    /// (the pending compat default, [`request_compat_default`]).
    Install(String),
}

/// Applies setup's Proton pick to `config`. An installed tool is the
/// default at once (and drops any download still waiting); a download is
/// remembered as the pending compat default, or is the default at once when
/// GRID already has it. A download must be one of `catalog`'s `source_id`s
/// and an installed pick must name a path: anything else is refused before
/// `config` changes.
pub fn apply_setup_compat_choice(
    config: &mut Config,
    choice: &SetupCompatChoice,
    catalog: &[String],
) -> Result<SetupCompatPlan, String> {
    match choice {
        SetupCompatChoice::Installed { path } => {
            let path = path.trim();
            if path.is_empty() {
                return Err("Choose a Proton version for Windows games.".to_string());
            }
            config.default_compat_tool = path.to_string();
            clear_pending_compat_default(config);
            Ok(SetupCompatPlan::Set(path.to_string()))
        }
        SetupCompatChoice::Download { source_id } => {
            let wanted = source_id.trim();
            let known = catalog
                .iter()
                .find(|id| id.trim().eq_ignore_ascii_case(wanted))
                .ok_or_else(|| "That Proton version is not one GRID can download.".to_string())?;
            let known = known.trim().to_string();
            Ok(match request_compat_default(config, &known) {
                Some(path) => SetupCompatPlan::Set(path),
                None => SetupCompatPlan::Install(known),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- setup's Proton pick --------------------------------------------------

    fn catalog() -> Vec<String> {
        vec![
            "GloriousEggroll/proton-ge-custom".to_string(),
            "CachyOS/proton-cachyos".to_string(),
        ]
    }

    #[test]
    fn setup_compat_choice_reads_both_kinds() {
        let installed: SetupCompatChoice =
            serde_json::from_str(r#"{"kind":"installed","path":"wine"}"#).unwrap();
        assert_eq!(
            installed,
            SetupCompatChoice::Installed {
                path: "wine".to_string()
            }
        );
        let download: SetupCompatChoice = serde_json::from_str(
            r#"{"kind":"download","source_id":"GloriousEggroll/proton-ge-custom"}"#,
        )
        .unwrap();
        assert_eq!(
            download,
            SetupCompatChoice::Download {
                source_id: "GloriousEggroll/proton-ge-custom".to_string()
            }
        );
        assert!(serde_json::from_str::<SetupCompatChoice>(r#"{"kind":"other"}"#).is_err());
    }

    #[test]
    fn setup_installed_pick_is_the_default_now() {
        let mut config = Config::default();
        request_compat_default(&mut config, "GloriousEggroll/proton-ge-custom");
        let plan = apply_setup_compat_choice(
            &mut config,
            &SetupCompatChoice::Installed {
                path: "/home/me/.steam/root/compatibilitytools.d/Proton 9.0".to_string(),
            },
            &catalog(),
        );
        assert_eq!(
            plan,
            Ok(SetupCompatPlan::Set(
                "/home/me/.steam/root/compatibilitytools.d/Proton 9.0".to_string()
            ))
        );
        assert_eq!(
            config.default_compat_tool,
            "/home/me/.steam/root/compatibilitytools.d/Proton 9.0"
        );
        assert!(config.setup.pending_compat.is_empty());
    }

    #[test]
    fn setup_download_pick_queues_and_waits() {
        let mut config = Config::default();
        let plan = apply_setup_compat_choice(
            &mut config,
            &SetupCompatChoice::Download {
                source_id: "gloriouseggroll/proton-ge-custom".to_string(),
            },
            &catalog(),
        );
        assert_eq!(
            plan,
            Ok(SetupCompatPlan::Install(
                "GloriousEggroll/proton-ge-custom".to_string()
            ))
        );
        assert!(config.default_compat_tool.is_empty());
        assert_eq!(
            config.setup.pending_compat,
            "GloriousEggroll/proton-ge-custom"
        );
    }

    #[test]
    fn setup_download_pick_already_installed_is_the_default_now() {
        let mut config = Config::default();
        config
            .compat_tool_installs
            .push(ge_install("/lib/compat/GE-Proton10-17"));
        let plan = apply_setup_compat_choice(
            &mut config,
            &SetupCompatChoice::Download {
                source_id: "GloriousEggroll/proton-ge-custom".to_string(),
            },
            &catalog(),
        );
        assert_eq!(
            plan,
            Ok(SetupCompatPlan::Set(
                "/lib/compat/GE-Proton10-17".to_string()
            ))
        );
        assert_eq!(config.default_compat_tool, "/lib/compat/GE-Proton10-17");
    }

    #[test]
    fn setup_refuses_an_unknown_download_or_a_blank_path_untouched() {
        let mut config = Config {
            default_compat_tool: "wine".to_string(),
            ..Config::default()
        };
        assert!(apply_setup_compat_choice(
            &mut config,
            &SetupCompatChoice::Download {
                source_id: "someone/else".to_string()
            },
            &catalog(),
        )
        .is_err());
        assert!(apply_setup_compat_choice(
            &mut config,
            &SetupCompatChoice::Installed {
                path: "  ".to_string()
            },
            &catalog(),
        )
        .is_err());
        assert_eq!(config.default_compat_tool, "wine");
        assert!(config.setup.pending_compat.is_empty());
    }

    // --- pending compat default ---------------------------------------------

    fn ge_install(path: &str) -> CompatToolInstall {
        CompatToolInstall {
            name: "GE-Proton10-17".to_string(),
            path: path.to_string(),
            source_id: "GloriousEggroll/proton-ge-custom".to_string(),
            release_tag: "GE-Proton10-17".to_string(),
        }
    }

    #[test]
    fn a_download_pick_waits_for_its_install() {
        let mut config = Config {
            default_compat_tool: "wine".to_string(),
            ..Config::default()
        };
        assert_eq!(
            request_compat_default(&mut config, "GloriousEggroll/proton-ge-custom"),
            None
        );
        assert_eq!(config.default_compat_tool, "wine");
        assert_eq!(
            config.setup.pending_compat,
            "GloriousEggroll/proton-ge-custom"
        );
    }

    #[test]
    fn the_install_landing_makes_it_the_default_and_clears_the_choice() {
        let mut config = Config::default();
        request_compat_default(&mut config, "GloriousEggroll/proton-ge-custom");
        config
            .compat_tool_installs
            .push(ge_install("/lib/compat/GE-Proton10-17"));
        assert_eq!(
            apply_pending_compat_default(&mut config).as_deref(),
            Some("/lib/compat/GE-Proton10-17")
        );
        assert_eq!(config.default_compat_tool, "/lib/compat/GE-Proton10-17");
        assert!(config.setup.pending_compat.is_empty());
        // Applied once: a later install of something else changes nothing.
        config.default_compat_tool = "wine".to_string();
        assert_eq!(apply_pending_compat_default(&mut config), None);
        assert_eq!(config.default_compat_tool, "wine");
    }

    #[test]
    fn another_tools_install_leaves_the_choice_waiting() {
        let mut config = Config::default();
        request_compat_default(&mut config, "GloriousEggroll/proton-ge-custom");
        config.compat_tool_installs.push(CompatToolInstall {
            name: "proton-cachyos".to_string(),
            path: "/lib/compat/cachyos".to_string(),
            source_id: "CachyOS/proton-cachyos".to_string(),
            release_tag: String::new(),
        });
        assert_eq!(apply_pending_compat_default(&mut config), None);
        assert!(config.default_compat_tool.is_empty());
        assert_eq!(
            config.setup.pending_compat,
            "GloriousEggroll/proton-ge-custom"
        );
    }

    #[test]
    fn a_tool_already_installed_is_the_default_at_once() {
        let mut config = Config::default();
        config
            .compat_tool_installs
            .push(ge_install("/lib/compat/GE-Proton10-17"));
        assert_eq!(
            request_compat_default(&mut config, "gloriouseggroll/proton-ge-custom").as_deref(),
            Some("/lib/compat/GE-Proton10-17")
        );
        assert!(config.setup.pending_compat.is_empty());
    }

    #[test]
    fn picking_an_installed_tool_drops_the_waiting_download() {
        let mut config = Config::default();
        request_compat_default(&mut config, "GloriousEggroll/proton-ge-custom");
        clear_pending_compat_default(&mut config);
        config
            .compat_tool_installs
            .push(ge_install("/lib/compat/GE-Proton10-17"));
        assert_eq!(apply_pending_compat_default(&mut config), None);
        assert!(config.default_compat_tool.is_empty());
    }

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
