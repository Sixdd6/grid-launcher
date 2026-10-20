//! umu-launcher (`umu-run`): the wrapper every Proton launch runs through
//! ([`super::native::build_native_command`]). A system `umu-run` on `PATH`
//! wins; otherwise GRID uses the one copy it downloaded itself — the
//! upstream zipapp release asset, extracted under
//! `<compat::managed_root()>/umu-launcher/` and recorded in
//! `Config.umu_launcher`. The zipapp is a Python program
//! (`#!/usr/bin/env python3`) that needs Python 3.10 or newer, so a launch
//! through the managed copy checks the `python3` the child will find first
//! ([`probe_python`]).
//!
//! Everything here is pure or takes its lookups injected, so the rules are
//! testable without a real `umu-run` or `python3` on the test host.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::config::Config;
use crate::library::extract::which_in;
use crate::library::paths::expand_home;

/// The upstream repository, and the Downloads row's `source_id` for a
/// managed umu-launcher install. No catalog profile carries it.
pub const UMU_SOURCE_ID: &str = "Open-Wine-Components/umu-launcher";
/// The Downloads row title of a managed umu-launcher install.
pub const UMU_TITLE: &str = "umu-launcher";
/// The executable name looked up on `PATH`, and the zipapp's file name.
pub const UMU_RUN: &str = "umu-run";
/// The directory under [`super::compat::managed_root`] the managed copy is
/// extracted into.
const INSTALL_DIR_NAME: &str = "umu-launcher";
/// The oldest Python the umu-launcher zipapp runs on.
pub const MIN_PYTHON: (u32, u32) = (3, 10);

/// The forge source a managed install resolves: the newest GitHub release's
/// zipapp tarball (`umu-launcher-<tag>-zipapp.tar`). The other assets are
/// distribution packages.
pub fn default_source() -> Value {
    json!({
        "provider": "github",
        "owner": "Open-Wine-Components",
        "repo": "umu-launcher",
        "release_tag": "latest",
        "asset_patterns": ["umu-launcher-*-zipapp.tar"],
    })
}

/// Where the managed copy is extracted: `<root>/umu-launcher`.
pub fn install_dir(root: &Path) -> PathBuf {
    root.join(INSTALL_DIR_NAME)
}

/// The `umu-run` zipapp inside an extracted release: `umu/umu-run` (the
/// upstream tarball's layout), else a top-level `umu-run`, else the first
/// (sorted) child directory holding one.
pub fn find_umu_run(install_dir: &Path) -> Option<PathBuf> {
    let nested = install_dir.join("umu").join(UMU_RUN);
    if nested.is_file() {
        return Some(nested);
    }
    let top = install_dir.join(UMU_RUN);
    if top.is_file() {
        return Some(top);
    }
    let mut children: Vec<PathBuf> = std::fs::read_dir(install_dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    children.sort();
    children
        .into_iter()
        .map(|dir| dir.join(UMU_RUN))
        .find(|candidate| candidate.is_file())
}

/// The managed `umu-run` `config` records, when the file is still there.
pub fn managed_umu_run(config: &Config) -> Option<PathBuf> {
    let record = config.umu_launcher.as_ref()?;
    let path = record.path.trim();
    if path.is_empty() {
        return None;
    }
    let path = expand_home(path);
    path.is_file().then_some(path)
}

/// Where a usable `umu-run` came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmuSource {
    /// On `PATH` (a distribution package, or the user's own install).
    System,
    /// GRID's own download ([`managed_umu_run`]).
    Managed,
}

/// The `umu-run` a Proton launch uses: the system one on `PATH` first, then
/// the managed copy. `None` when neither exists.
pub fn locate(
    which: &dyn Fn(&str) -> Option<PathBuf>,
    managed: Option<&Path>,
) -> Option<(PathBuf, UmuSource)> {
    if let Some(system) = which(UMU_RUN) {
        return Some((system, UmuSource::System));
    }
    managed.map(|path| (path.to_path_buf(), UmuSource::Managed))
}

/// The Compat tools pane's umu-run line. Serialized straight to the
/// frontend, so the field names are part of the IPC contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UmuRunStatus {
    /// `"system"`, `"managed"` or `"missing"`.
    pub source: String,
    /// The `umu-run` a launch would use; `""` when missing.
    pub path: String,
    /// The managed copy's release tag; `""` unless `source` is `"managed"`.
    pub release_tag: String,
    /// The Downloads row `source_id` a managed install runs under, so the
    /// pane can tell when its install is live.
    pub source_id: String,
}

/// [`UmuRunStatus`] for `config`, with [`locate`]'s precedence.
pub fn status(which: &dyn Fn(&str) -> Option<PathBuf>, config: &Config) -> UmuRunStatus {
    let managed = managed_umu_run(config);
    let (source, path, release_tag) = match locate(which, managed.as_deref()) {
        Some((path, UmuSource::System)) => ("system", path, String::new()),
        Some((path, UmuSource::Managed)) => (
            "managed",
            path,
            config
                .umu_launcher
                .as_ref()
                .map(|record| record.release_tag.clone())
                .unwrap_or_default(),
        ),
        None => ("missing", PathBuf::new(), String::new()),
    };
    UmuRunStatus {
        source: source.to_string(),
        path: path.to_string_lossy().into_owned(),
        release_tag,
        source_id: UMU_SOURCE_ID.to_string(),
    }
}

/// The `(major, minor)` of a `python3 --version` line (`"Python 3.12.1"`;
/// Python before 3.4 printed it to stderr, so the caller passes both
/// streams). `None` when no version is found.
pub fn parse_python_version(output: &str) -> Option<(u32, u32)> {
    output.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Python")?.trim();
        let mut parts = rest.split('.');
        let major = parts.next()?.trim().parse().ok()?;
        let minor: String = parts
            .next()?
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        Some((major, minor.parse().ok()?))
    })
}

/// The launch error for a managed `umu-run` whose interpreter is `found`
/// (`None` = no `python3` on the child's `PATH`, or no readable version),
/// or `Ok` when it meets [`MIN_PYTHON`].
pub fn python_requirement(found: Option<(u32, u32)>) -> Result<(), String> {
    let (min_major, min_minor) = MIN_PYTHON;
    match found {
        Some(version) if version >= MIN_PYTHON => Ok(()),
        Some((major, minor)) => Err(format!(
            "GRID's umu-launcher needs Python {min_major}.{min_minor} or newer, but python3 is \
             {major}.{minor}. Update Python, or install the umu-launcher package from your \
             distribution."
        )),
        None => Err(format!(
            "GRID's umu-launcher needs Python {min_major}.{min_minor} or newer, and python3 was \
             not found. Install Python 3, or install the umu-launcher package from your \
             distribution."
        )),
    }
}

/// Runs `python3 --version` exactly as the launched child would find it:
/// looked up on `env`'s `PATH` (the cleaned child environment,
/// [`super::spawn::clean_env`]) and run with that environment. `None` when
/// there is no `python3` there or it prints no version. Blocking.
pub fn probe_python(env: &HashMap<String, String>) -> Option<(u32, u32)> {
    let path_var = env.get("PATH")?;
    let python = which_in(OsStr::new(path_var), "python3")?;
    let output = std::process::Command::new(python)
        .arg("--version")
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_python_version(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UmuLauncherInstall;
    use std::fs;

    fn no_which(_name: &str) -> Option<PathBuf> {
        None
    }

    fn system_which(name: &str) -> Option<PathBuf> {
        (name == UMU_RUN).then(|| PathBuf::from("/usr/bin/umu-run"))
    }

    fn config_with(path: &Path, tag: &str) -> Config {
        Config {
            umu_launcher: Some(UmuLauncherInstall {
                path: path.to_string_lossy().into_owned(),
                release_tag: tag.to_string(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn the_default_source_picks_the_zipapp_tarball_of_the_latest_release() {
        let source = default_source();
        assert_eq!(source["provider"], "github");
        assert_eq!(source["release_tag"], "latest");
        assert_eq!(
            source["asset_patterns"],
            json!(["umu-launcher-*-zipapp.tar"])
        );
        assert_eq!(
            format!(
                "{}/{}",
                source["owner"].as_str().unwrap(),
                source["repo"].as_str().unwrap()
            ),
            UMU_SOURCE_ID
        );
    }

    #[test]
    fn install_dir_is_a_fixed_child_of_the_root() {
        assert_eq!(
            install_dir(Path::new("/data/compat-tools")),
            Path::new("/data/compat-tools/umu-launcher")
        );
    }

    #[test]
    fn find_umu_run_prefers_the_upstream_layout() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_umu_run(dir.path()), None);

        let nested = dir.path().join("other");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join(UMU_RUN), b"zipapp").unwrap();
        assert_eq!(find_umu_run(dir.path()), Some(nested.join(UMU_RUN)));

        fs::write(dir.path().join(UMU_RUN), b"zipapp").unwrap();
        assert_eq!(find_umu_run(dir.path()), Some(dir.path().join(UMU_RUN)));

        let upstream = dir.path().join("umu");
        fs::create_dir_all(&upstream).unwrap();
        fs::write(upstream.join(UMU_RUN), b"zipapp").unwrap();
        assert_eq!(find_umu_run(dir.path()), Some(upstream.join(UMU_RUN)));
    }

    #[test]
    fn managed_umu_run_needs_a_record_and_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let zipapp = dir.path().join(UMU_RUN);
        assert_eq!(managed_umu_run(&Config::default()), None);
        assert_eq!(managed_umu_run(&config_with(&zipapp, "1.4.4")), None);
        fs::write(&zipapp, b"zipapp").unwrap();
        assert_eq!(
            managed_umu_run(&config_with(&zipapp, "1.4.4")),
            Some(zipapp)
        );
        assert_eq!(
            managed_umu_run(&config_with(Path::new("  "), "1.4.4")),
            None
        );
    }

    #[test]
    fn locate_prefers_the_system_umu_run() {
        let managed = Path::new("/data/umu/umu-run");
        assert_eq!(
            locate(&system_which, Some(managed)),
            Some((PathBuf::from("/usr/bin/umu-run"), UmuSource::System))
        );
        assert_eq!(
            locate(&no_which, Some(managed)),
            Some((managed.to_path_buf(), UmuSource::Managed))
        );
        assert_eq!(locate(&no_which, None), None);
    }

    #[test]
    fn status_reports_system_managed_or_missing() {
        let dir = tempfile::tempdir().unwrap();
        let zipapp = dir.path().join(UMU_RUN);
        fs::write(&zipapp, b"zipapp").unwrap();
        let config = config_with(&zipapp, "1.4.4");

        let system = status(&system_which, &config);
        assert_eq!(system.source, "system");
        assert_eq!(system.path, "/usr/bin/umu-run");
        assert_eq!(system.release_tag, "");
        assert_eq!(system.source_id, UMU_SOURCE_ID);

        let managed = status(&no_which, &config);
        assert_eq!(managed.source, "managed");
        assert_eq!(managed.path, zipapp.to_string_lossy());
        assert_eq!(managed.release_tag, "1.4.4");

        let missing = status(&no_which, &Config::default());
        assert_eq!(missing.source, "missing");
        assert_eq!(missing.path, "");
        assert_eq!(missing.release_tag, "");
    }

    #[test]
    fn parse_python_version_reads_major_and_minor() {
        assert_eq!(parse_python_version("Python 3.12.1\n"), Some((3, 12)));
        assert_eq!(parse_python_version("\nPython 3.10.0rc2"), Some((3, 10)));
        assert_eq!(parse_python_version("Python 3.9"), Some((3, 9)));
        assert_eq!(parse_python_version("Python 3.13t"), Some((3, 13)));
        assert_eq!(parse_python_version(""), None);
        assert_eq!(parse_python_version("python: command not found"), None);
        assert_eq!(parse_python_version("Python x.y"), None);
    }

    #[test]
    fn python_requirement_accepts_3_10_and_newer() {
        assert_eq!(python_requirement(Some((3, 10))), Ok(()));
        assert_eq!(python_requirement(Some((3, 13))), Ok(()));
        assert_eq!(python_requirement(Some((4, 0))), Ok(()));

        let old = python_requirement(Some((3, 9))).unwrap_err();
        assert!(old.contains("3.10 or newer"), "{old}");
        assert!(old.contains("python3 is 3.9"), "{old}");

        let missing = python_requirement(None).unwrap_err();
        assert!(missing.contains("python3 was not found"), "{missing}");
    }

    #[test]
    fn probe_python_without_a_path_finds_nothing() {
        assert_eq!(probe_python(&HashMap::new()), None);
        let empty = tempfile::tempdir().unwrap();
        let env = HashMap::from([(
            "PATH".to_string(),
            empty.path().to_string_lossy().into_owned(),
        )]);
        assert_eq!(probe_python(&env), None);
    }

    /// A stub `python3` on the given `PATH` is run with that environment.
    #[cfg(unix)]
    #[test]
    fn probe_python_runs_the_python3_on_the_given_path() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let stub = dir.path().join("python3");
        fs::write(&stub, "#!/bin/sh\necho \"Python 3.11.4\"\n").unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        let env = HashMap::from([(
            "PATH".to_string(),
            format!("{}:/usr/bin:/bin", dir.path().display()),
        )]);
        assert_eq!(probe_python(&env), Some((3, 11)));
    }
}
