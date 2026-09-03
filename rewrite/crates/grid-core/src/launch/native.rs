//! Native (non-emulated) launch command construction: resolves the game's
//! own executable, wraps it in Wine or Proton (via `umu-run`) per the row's
//! or config's compat-tool choice, and builds the spawn-ready
//! argv/cwd/env. Ports `prepare_native_launch_command`
//! (`grid_launcher/emulator/launch.py:223-272`); see
//! `docs/porting/04-emulator-launch.md` §9.

use std::fs;
use std::path::{Path, PathBuf};

use crate::library::paths::{archive_name, candidate_archives};
use crate::library::registry::InstalledGame;
use crate::library::specials::native::{executable_candidates, install_dir, resolved_executable};

use super::template::split_template;

/// A resolved native launch: everything [`std::process::Command`] needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLaunch {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// `"wine"`, the compat-tool value used as `PROTONPATH` for the
    /// `umu-run` branch, or `""` when the game runs with no compat tool at
    /// all. The caller maps `""` to `"native"` for the session record.
    pub tool_label: String,
}

/// Resolves and builds a native launch for `row` (`prepare_native_launch_command`,
/// launch.py:223-272):
///
/// 1. Resolves the executable via [`install_dir`]/[`executable_candidates`]/
///    [`resolved_executable`], from the archive candidates built by
///    [`candidate_archives`] over `library`; `None` -> the verbatim "No
///    launchable native executable…" error.
/// 2. Splits `row.native_launch_parameters.trim()` with [`split_template`];
///    an `Err` is wrapped as `"Invalid custom launch parameters: {e}"`.
/// 3. Picks the compat tool: `row.native_compat_tool.trim()` if non-blank,
///    else `default_compat_tool.trim()`. The latter is also blanked here
///    when `host` starts with `"win"`, matching the caller's own gate
///    defensively (`LaunchService::launch` computes the same thing before
///    calling this) so this function is safe to call directly with an
///    unblanked config value too.
/// 4. `"wine"` prepends `which("wine")` (falling back to the literal
///    `"wine"` when not found); any other non-blank value requires
///    `which("umu-run")` (missing -> the verbatim umu-run message) and sets
///    `PROTONPATH` to the tool value verbatim. Both of those branches also
///    set `WINEPREFIX` (creating the directory) when `row.native_wineprefix`
///    is non-blank. A blank tool runs the executable directly, with no
///    wrapper and no env overrides.
///
/// `cwd` is the executable's parent directory (`.` when it has none, mirrors
/// [`super::spawn::prepare_emulator_launch`]'s fallback).
pub fn build_native_command(
    row: &InstalledGame,
    library: &Path,
    default_compat_tool: &str,
    host: &str,
    which: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<NativeLaunch, String> {
    let name = archive_name(&row.rom_file_name, &row.title, &row.platform);
    let archives = candidate_archives(library, &row.platform, &row.archive_path, &name);
    let executable = install_dir(row, &archives)
        .map(|dir| executable_candidates(&dir))
        .and_then(|candidates| resolved_executable(row, &candidates));
    let Some(executable) = executable else {
        return Err(
            "No launchable native executable is configured for this game. Use Game Settings to select one."
                .to_string(),
        );
    };

    let args = split_template(row.native_launch_parameters.trim())
        .map_err(|e| format!("Invalid custom launch parameters: {e}"))?;

    let default_compat_tool = if host.trim().to_lowercase().starts_with("win") {
        ""
    } else {
        default_compat_tool.trim()
    };
    let row_tool = row.native_compat_tool.trim();
    let tool = if !row_tool.is_empty() {
        row_tool
    } else {
        default_compat_tool
    };

    let exe_string = executable.to_string_lossy().into_owned();
    let mut command = Vec::with_capacity(args.len() + 1);
    command.push(exe_string);
    command.extend(args);

    let mut env: Vec<(String, String)> = Vec::new();
    let tool_label;

    if tool == "wine" {
        let wine = which("wine")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "wine".to_string());
        command.insert(0, wine);
        tool_label = "wine".to_string();
        set_wineprefix(row, &mut env)?;
    } else if !tool.is_empty() {
        let Some(umu) = which("umu-run") else {
            return Err(
                "umu-run is not installed. Install the umu-launcher package to use Proton compatibility tools."
                    .to_string(),
            );
        };
        command.insert(0, umu.to_string_lossy().into_owned());
        env.push(("PROTONPATH".to_string(), tool.to_string()));
        tool_label = tool.to_string();
        set_wineprefix(row, &mut env)?;
    } else {
        tool_label = String::new();
    }

    let cwd = match executable.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    Ok(NativeLaunch {
        argv: command,
        cwd,
        env,
        tool_label,
    })
}

/// Adds `WINEPREFIX` to `env` and creates the directory when
/// `row.native_wineprefix` is non-blank (`os.makedirs(..., exist_ok=True)`,
/// launch.py:255/266). Unlike the Python original — where a `makedirs`
/// failure is an uncaught `OSError` that surfaces generically — this
/// reports the failure through the same `Result` every other validation in
/// this function uses, since there is nowhere in grid-core to swallow an
/// I/O error silently.
fn set_wineprefix(row: &InstalledGame, env: &mut Vec<(String, String)>) -> Result<(), String> {
    let prefix = row.native_wineprefix.trim();
    if prefix.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(prefix)
        .map_err(|e| format!("Could not create Wine prefix directory:\n{prefix}\n{e}"))?;
    env.push(("WINEPREFIX".to_string(), prefix.to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn row(title: &str, extracted_dir: &Path) -> InstalledGame {
        InstalledGame {
            title: title.to_string(),
            platform: "Windows".to_string(),
            rom_file_name: format!("{title}.zip"),
            extracted_dir: extracted_dir.to_string_lossy().into_owned(),
            ..Default::default()
        }
    }

    /// A tempdir with an installed native game's directory and a launchable
    /// executable inside it.
    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let exe = install.join("Game.exe");
        fs::write(&exe, b"exe bytes").unwrap();
        (dir, install, exe)
    }

    fn no_which(_name: &str) -> Option<PathBuf> {
        None
    }

    // --- executable resolution -------------------------------------------

    #[test]
    fn no_executable_is_the_verbatim_error() {
        let dir = tempfile::tempdir().unwrap();
        let empty_install = dir.path().join("install");
        fs::create_dir_all(&empty_install).unwrap();
        let g = row("Empty", &empty_install);

        let error = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap_err();
        assert_eq!(
            error,
            "No launchable native executable is configured for this game. Use Game Settings to select one."
        );
    }

    // --- plain (no compat tool) -------------------------------------------

    #[test]
    fn plain_launch_runs_the_executable_directly() {
        let (dir, install, exe) = fixture();
        let mut g = row("Game", &install);
        g.native_launch_parameters = "--fullscreen \"extra arg\"".to_string();

        let result = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap();
        assert_eq!(
            result.argv,
            vec![
                exe.to_string_lossy().into_owned(),
                "--fullscreen".to_string(),
                "extra arg".to_string(),
            ]
        );
        assert_eq!(result.cwd, install);
        assert!(result.env.is_empty());
        assert_eq!(result.tool_label, "");
    }

    #[test]
    fn default_compat_tool_is_used_when_the_row_has_none() {
        let (dir, install, exe) = fixture();
        let g = row("Game", &install);

        let result = build_native_command(&g, dir.path(), "wine", "linux", &no_which).unwrap();
        assert_eq!(result.argv[0], "wine");
        assert_eq!(result.argv[1], exe.to_string_lossy());
        assert_eq!(result.tool_label, "wine");
    }

    #[test]
    fn the_rows_own_compat_tool_overrides_the_default() {
        let (dir, install, exe) = fixture();
        let mut g = row("Game", &install);
        g.native_compat_tool = "wine".to_string();

        let result =
            build_native_command(&g, dir.path(), "/opt/proton", "linux", &no_which).unwrap();
        assert_eq!(
            result.argv,
            vec!["wine".to_string(), exe.to_string_lossy().into_owned()]
        );
        assert_eq!(result.tool_label, "wine");
    }

    // --- wine branch --------------------------------------------------------

    #[test]
    fn wine_prepends_the_resolved_path_when_which_finds_it() {
        let (dir, install, exe) = fixture();
        let mut g = row("Game", &install);
        g.native_compat_tool = "wine".to_string();

        let which = |name: &str| -> Option<PathBuf> {
            (name == "wine").then(|| PathBuf::from("/usr/bin/wine"))
        };
        let result = build_native_command(&g, dir.path(), "", "linux", &which).unwrap();
        assert_eq!(
            result.argv,
            vec![
                "/usr/bin/wine".to_string(),
                exe.to_string_lossy().into_owned()
            ]
        );
        assert_eq!(result.tool_label, "wine");
        assert!(result.env.is_empty());
    }

    #[test]
    fn wine_falls_back_to_the_literal_name_when_not_on_path() {
        let (dir, install, _exe) = fixture();
        let mut g = row("Game", &install);
        g.native_compat_tool = "wine".to_string();

        let result = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap();
        assert_eq!(result.argv[0], "wine");
    }

    // --- proton (umu-run) branch --------------------------------------------

    #[test]
    fn proton_wraps_with_umu_run_and_sets_protonpath() {
        let (dir, install, exe) = fixture();
        let mut g = row("Game", &install);
        g.native_compat_tool = "/home/user/.steam/compat/GE-Proton9".to_string();

        let which = |name: &str| -> Option<PathBuf> {
            (name == "umu-run").then(|| PathBuf::from("/usr/bin/umu-run"))
        };
        let result = build_native_command(&g, dir.path(), "", "linux", &which).unwrap();
        assert_eq!(
            result.argv,
            vec![
                "/usr/bin/umu-run".to_string(),
                exe.to_string_lossy().into_owned()
            ]
        );
        assert_eq!(
            result.env,
            vec![(
                "PROTONPATH".to_string(),
                "/home/user/.steam/compat/GE-Proton9".to_string()
            )]
        );
        assert_eq!(result.tool_label, "/home/user/.steam/compat/GE-Proton9");
    }

    #[test]
    fn missing_umu_run_is_the_verbatim_error() {
        let (dir, install, _exe) = fixture();
        let mut g = row("Game", &install);
        g.native_compat_tool = "GE-Proton9".to_string();

        let error = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap_err();
        assert_eq!(
            error,
            "umu-run is not installed. Install the umu-launcher package to use Proton compatibility tools."
        );
    }

    // --- WINEPREFIX ----------------------------------------------------------

    #[test]
    fn wineprefix_is_created_and_set_for_wine() {
        let (dir, install, _exe) = fixture();
        let prefix = dir.path().join("prefix");
        let mut g = row("Game", &install);
        g.native_compat_tool = "wine".to_string();
        g.native_wineprefix = prefix.to_string_lossy().into_owned();

        let result = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap();
        assert!(prefix.is_dir(), "WINEPREFIX directory was not created");
        assert_eq!(
            result.env,
            vec![(
                "WINEPREFIX".to_string(),
                prefix.to_string_lossy().into_owned()
            )]
        );
    }

    #[test]
    fn wineprefix_is_created_and_set_for_proton_alongside_protonpath() {
        let (dir, install, _exe) = fixture();
        let prefix = dir.path().join("prefix");
        let mut g = row("Game", &install);
        g.native_compat_tool = "GE-Proton9".to_string();
        g.native_wineprefix = prefix.to_string_lossy().into_owned();

        let which = |name: &str| -> Option<PathBuf> {
            (name == "umu-run").then(|| PathBuf::from("/usr/bin/umu-run"))
        };
        let result = build_native_command(&g, dir.path(), "", "linux", &which).unwrap();
        assert!(prefix.is_dir(), "WINEPREFIX directory was not created");
        assert_eq!(
            result.env,
            vec![
                ("PROTONPATH".to_string(), "GE-Proton9".to_string()),
                (
                    "WINEPREFIX".to_string(),
                    prefix.to_string_lossy().into_owned()
                ),
            ]
        );
    }

    #[test]
    fn blank_tool_never_sets_wineprefix() {
        let (dir, install, _exe) = fixture();
        let mut g = row("Game", &install);
        g.native_wineprefix = dir.path().join("prefix").to_string_lossy().into_owned();

        let result = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap();
        assert!(result.env.is_empty());
    }

    // --- host gate -----------------------------------------------------------

    #[test]
    fn a_windows_host_blanks_the_default_compat_tool() {
        let (dir, install, exe) = fixture();
        let g = row("Game", &install);

        let result = build_native_command(&g, dir.path(), "wine", "windows", &no_which).unwrap();
        assert_eq!(result.argv, vec![exe.to_string_lossy().into_owned()]);
        assert_eq!(result.tool_label, "");
    }

    // --- invalid custom launch parameters ------------------------------------
    //
    // `split_template`'s POSIX-mode splitter always has a fallback that
    // succeeds (see `launch/template.rs`), so there is no real input that
    // makes it return `Err` today; the wrapping above
    // ("Invalid custom launch parameters: {e}") is defensive/forward-looking
    // and is not independently exercised by a test here.
}
