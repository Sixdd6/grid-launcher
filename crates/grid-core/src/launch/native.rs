//! Native (non-emulated) launch command construction: resolves the game's
//! own executable, wraps it in Wine or Proton (via `umu-run`) per the row's
//! or config's compat-tool choice, and builds the spawn-ready
//! argv/cwd/env. Ports `prepare_native_launch_command`
//! (`grid_launcher/emulator/launch.py:223-272`); see
//! `docs/porting/04-emulator-launch.md` §9.

use std::fs;
use std::path::{Path, PathBuf};

use crate::library::paths::{archive_name, candidate_archives, expand_home};
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

/// Why [`build_native_command`] could not build a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCommandError {
    /// A validation failure, worded for the user.
    Message(String),
    /// The row runs through Proton and no `umu-run` was found. Its own
    /// variant so the frontend can offer to install umu-launcher.
    UmuMissing,
}

/// The [`NativeCommandError::UmuMissing`] text the user reads.
pub const UMU_MISSING_MESSAGE: &str = "umu-run is not installed. Proton games run through \
     umu-launcher: install it from Emulators › Compat tools, or install your distribution's \
     umu-launcher package.";

impl std::fmt::Display for NativeCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeCommandError::Message(message) => f.write_str(message),
            NativeCommandError::UmuMissing => f.write_str(UMU_MISSING_MESSAGE),
        }
    }
}

impl From<String> for NativeCommandError {
    fn from(message: String) -> Self {
        NativeCommandError::Message(message)
    }
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
///    unblanked config value too. Both are blanked outright when the row's
///    platform is not a windows one ([`InstalledGame::is_windows`], which reads the slug first) — user ruling
///    2026-09-08: a linux-platform game runs its own executable directly,
///    with no wrapper, no `WINEPREFIX`, and a blank `tool_label`.
/// 4. `"wine"` prepends `which("wine")` (falling back to the literal
///    `"wine"` when not found); any other non-blank value requires
///    `which("umu-run")` (missing -> [`NativeCommandError::UmuMissing`];
///    the caller's `which` answers it with [`super::umu::locate`]) and sets
///    `PROTONPATH` to the tool value verbatim. Both of those branches also
///    set `WINEPREFIX` (creating the directory) when `row.native_wineprefix`
///    is non-blank. A blank tool runs the executable directly, with no
///    wrapper and no env overrides.
///
/// 5. On a Windows host with no compat tool, a `.ps1` script runs as
///    `powershell.exe -NoProfile -ExecutionPolicy Bypass -File <script>
///    <args>`. A `.bat`/`.cmd` is spawned as itself: `std::process::Command`
///    starts it through cmd.exe and escapes its arguments.
///
/// `cwd` is the `game.json` working directory when the sidecar pin is the
/// file that launches ([`sidecar_working_dir`]); otherwise the executable's
/// parent directory (`.` when it has none, mirrors
/// [`super::spawn::prepare_emulator_launch`]'s fallback).
pub fn build_native_command(
    row: &InstalledGame,
    library: &Path,
    default_compat_tool: &str,
    host: &str,
    which: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<NativeLaunch, NativeCommandError> {
    let name = archive_name(&row.rom_file_name, &row.title, &row.platform);
    let archives = candidate_archives(
        library,
        &row.platform,
        &row.archive_path,
        &name,
        &row.native_game_dir,
    );
    let game_dir = install_dir(row, &archives);
    let executable = game_dir
        .as_ref()
        .map(|dir| executable_candidates(dir, &row.title))
        .and_then(|candidates| resolved_executable(row, &candidates));
    let Some(executable) = executable else {
        return Err(NativeCommandError::Message(
            "No launchable native executable is configured for this game. Use Game Settings to select one."
                .to_string(),
        ));
    };

    let args = split_template(row.native_launch_parameters.trim())
        .map_err(|e| format!("Invalid custom launch parameters: {e}"))?;

    let default_compat_tool = if host.trim().to_lowercase().starts_with("win") {
        ""
    } else {
        default_compat_tool.trim()
    };
    let row_tool = row.native_compat_tool.trim();
    let tool = if !row.is_windows() {
        ""
    } else if !row_tool.is_empty() {
        row_tool
    } else {
        default_compat_tool
    };

    let exe_string = executable.to_string_lossy().into_owned();
    let mut command = Vec::with_capacity(args.len() + 6);
    // A PowerShell script is not a program Windows can start: run it
    // through powershell.exe, with no profile and no execution-policy
    // prompt. A .bat/.cmd needs no wrapper — std::process::Command starts
    // it through cmd.exe and escapes its arguments for batch parsing. Under
    // Wine/Proton the tool decides what to do with the file.
    if tool.is_empty() && host.trim().to_lowercase().starts_with("win") && is_ps1(&executable) {
        let powershell = which("powershell.exe")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "powershell.exe".to_string());
        command.extend([
            powershell,
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
        ]);
    }
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
        let Some(umu) = which(super::umu::UMU_RUN) else {
            return Err(NativeCommandError::UmuMissing);
        };
        command.insert(0, umu.to_string_lossy().into_owned());
        env.push(("PROTONPATH".to_string(), tool.to_string()));
        tool_label = tool.to_string();
        set_wineprefix(row, &mut env)?;
    } else {
        tool_label = String::new();
    }

    let cwd = match sidecar_working_dir(row, game_dir.as_deref(), &executable) {
        Some(dir) => dir,
        None => match executable.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        },
    };

    Ok(NativeLaunch {
        argv: command,
        cwd,
        env,
        tool_label,
    })
}

/// Whether `path` is a PowerShell script (`.ps1`, any case).
fn is_ps1(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
}

/// The working directory the `game.json` `launch` section named
/// (`row.native_working_dir`, relative to `game_dir`; `"."` = `game_dir`),
/// when it applies: the pinned file is the one that launches (a fallback to
/// the ranking drops it) and the folder exists. `None` = the executable's
/// own folder.
fn sidecar_working_dir(
    row: &InstalledGame,
    game_dir: Option<&Path>,
    executable: &Path,
) -> Option<PathBuf> {
    let relative = row.native_working_dir.trim();
    let game_dir = game_dir?;
    let pinned = row.native_executable_path.trim();
    if relative.is_empty() || pinned.is_empty() || expand_home(pinned) != executable {
        return None;
    }
    let dir = relative
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .fold(game_dir.to_path_buf(), |dir, part| dir.join(part));
    dir.is_dir().then_some(dir)
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
            NativeCommandError::Message(
                "No launchable native executable is configured for this game. Use Game Settings to select one."
                    .to_string()
            )
        );
    }

    #[test]
    fn executable_resolution_falls_back_to_the_legacy_platform_directory() {
        let dir = tempfile::tempdir().unwrap();
        // Only the legacy (pre-v1) `<library>/<Platform>/` archive and
        // executable exist — no `games/` directory, no extracted_dir/path
        // recorded, so install_dir must fall back to candidate_archives'
        // legacy entry.
        let legacy_dir = dir.path().join("Windows");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(legacy_dir.join("Legacy Game.zip"), b"archive bytes").unwrap();
        let exe = legacy_dir.join("Legacy Game.exe");
        fs::write(&exe, b"exe bytes").unwrap();

        let g = InstalledGame {
            title: "Legacy Game".to_string(),
            platform: "Windows".to_string(),
            rom_file_name: "Legacy Game.zip".to_string(),
            ..Default::default()
        };

        let result = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap();
        assert_eq!(result.argv[0], exe.to_string_lossy());
        assert_eq!(result.cwd, legacy_dir);
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
    fn missing_umu_run_is_its_own_error() {
        let (dir, install, _exe) = fixture();
        let mut g = row("Game", &install);
        g.native_compat_tool = "GE-Proton9".to_string();

        let error = build_native_command(&g, dir.path(), "", "linux", &no_which).unwrap_err();
        assert_eq!(error, NativeCommandError::UmuMissing);
        assert_eq!(error.to_string(), UMU_MISSING_MESSAGE);
        assert!(error.to_string().contains("Emulators › Compat tools"));
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

    // --- linux platform rows -------------------------------------------------

    #[test]
    fn a_linux_platform_row_ignores_every_compat_tool() {
        let (dir, install, exe) = fixture();
        let mut g = row("Game", &install);
        g.platform = "Linux".to_string();
        g.native_compat_tool = "wine".to_string();
        g.native_wineprefix = dir.path().join("prefix").to_string_lossy().into_owned();

        let which = |name: &str| -> Option<PathBuf> {
            panic!("a linux-platform row must not look up {name}");
        };
        let result = build_native_command(&g, dir.path(), "GE-Proton9", "linux", &which).unwrap();
        assert_eq!(result.argv, vec![exe.to_string_lossy().into_owned()]);
        assert!(result.env.is_empty());
        assert_eq!(result.tool_label, "");
        assert!(
            !dir.path().join("prefix").exists(),
            "a linux-platform row must not create a Wine prefix"
        );
    }

    #[test]
    fn a_windows_platform_row_still_takes_the_default_compat_tool() {
        let (dir, install, exe) = fixture();
        let mut g = row("Game", &install);
        g.platform = "Windows 10".to_string();

        let result = build_native_command(&g, dir.path(), "wine", "linux", &no_which).unwrap();
        assert_eq!(
            result.argv,
            vec!["wine".to_string(), exe.to_string_lossy().into_owned()]
        );
        assert_eq!(result.tool_label, "wine");
    }

    /// User bug 2026-10-10: "Tamriel Rebuilt" was installed on a custom-named
    /// Windows platform (slug `win`, display "PC Fangames & Ports") before
    /// the slug rule, as a PLAIN extract: `extracted_dir` and
    /// `extracted_path` are set, `native_game_dir` and `archive_path` are
    /// blank, and the archive is gone. Once the backfill stores the slug,
    /// the row is native, Game Settings lists its executables from the
    /// existing extract, and it launches — with Wine/Proton, because the
    /// slug makes it a Windows game — without a reinstall.
    #[test]
    fn an_existing_plain_extract_on_a_custom_named_windows_platform_launches_natively() {
        let dir = tempfile::tempdir().unwrap();
        let platform_dir = dir.path().join("games").join("PC Fangames & Ports");
        let extracted = platform_dir.join("TamrielRebuilt");
        fs::create_dir_all(extracted.join("Data Files")).unwrap();
        let exe = extracted.join("Morrowind.exe");
        fs::write(&exe, b"MZ").unwrap();
        fs::write(extracted.join("Data Files").join("TR_Mainland.esm"), b"esm").unwrap();
        let g = InstalledGame {
            title: "Tamriel Rebuilt".to_string(),
            platform: "PC Fangames & Ports".to_string(),
            platform_slug: "win".to_string(),
            rom_id: Some(52001),
            rom_file_name: "TamrielRebuilt.zip".to_string(),
            extracted_dir: extracted.to_string_lossy().into_owned(),
            extracted_path: extracted
                .join("Data Files")
                .join("TR_Mainland.esm")
                .to_string_lossy()
                .into_owned(),
            ..Default::default()
        };
        assert!(g.is_native() && g.is_windows());
        assert!(g.native_game_dir.is_empty());

        // What `native_game_settings` shows: the install dir is the extract,
        // and the executable is a candidate inside it.
        let archives = candidate_archives(
            dir.path(),
            &g.platform,
            &g.archive_path,
            &archive_name(&g.rom_file_name, &g.title, &g.platform),
            &g.native_game_dir,
        );
        let install = install_dir(&g, &archives).expect("the extract resolves");
        assert_eq!(install, extracted);
        let candidates = executable_candidates(&install, &g.title);
        assert_eq!(candidates, vec![exe.clone()]);

        // Launch on a Windows host: the executable runs directly.
        let on_windows = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(on_windows.argv, vec![exe.to_string_lossy().into_owned()]);
        assert_eq!(on_windows.cwd, extracted);
        // On Linux the slug makes it a Windows game, so the default compat
        // tool applies.
        let on_linux = build_native_command(&g, dir.path(), "wine", "linux", &no_which).unwrap();
        assert_eq!(on_linux.tool_label, "wine");
    }

    // --- game.json launch: working directory and script files ----------------

    /// An install with `bin/start.bat`, `bin/run.ps1`, `Game.exe` and a
    /// `data/` folder; the row pins `pinned` (relative to the install).
    fn sidecar_fixture(
        pinned: &str,
        working_dir: &str,
    ) -> (tempfile::TempDir, PathBuf, InstalledGame) {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("My Game").join("game");
        fs::create_dir_all(install.join("bin")).unwrap();
        fs::create_dir_all(install.join("data")).unwrap();
        fs::write(install.join("bin").join("start.bat"), b"@echo off").unwrap();
        fs::write(install.join("bin").join("run.ps1"), b"exit 0").unwrap();
        fs::write(install.join("Game.exe"), b"MZ").unwrap();
        let mut g = row("Game", &install);
        let pinned = pinned.split('/').fold(install.clone(), |p, s| p.join(s));
        g.native_executable_path = pinned.to_string_lossy().into_owned();
        g.native_launch_source = "sidecar".to_string();
        g.native_working_dir = working_dir.to_string();
        (dir, install, g)
    }

    #[test]
    fn a_sidecar_working_dir_is_the_cwd() {
        let (dir, install, g) = sidecar_fixture("bin/start.bat", "data");
        let result = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(result.cwd, install.join("data"));
        let (dir, install, g) = sidecar_fixture("bin/start.bat", ".");
        let result = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(result.cwd, install);
    }

    #[test]
    fn a_blank_or_missing_working_dir_falls_back_to_the_executables_folder() {
        let (dir, install, g) = sidecar_fixture("bin/start.bat", "");
        let result = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(result.cwd, install.join("bin"));
        let (dir, install, g) = sidecar_fixture("bin/start.bat", "gone");
        let result = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(result.cwd, install.join("bin"));
    }

    #[test]
    fn the_working_dir_is_ignored_when_the_pinned_file_is_gone() {
        let (dir, install, mut g) = sidecar_fixture("bin/start.bat", "data");
        g.native_executable_path = install
            .join("bin")
            .join("gone.bat")
            .to_string_lossy()
            .into_owned();
        let result = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        // The ranking picks Game.exe at the root; the sidecar dir belonged
        // to the pinned file, not to it.
        assert_eq!(
            result.argv,
            vec![install.join("Game.exe").to_string_lossy().into_owned()]
        );
        assert_eq!(result.cwd, install);
    }

    #[test]
    fn a_batch_file_on_a_windows_host_is_the_program_itself() {
        // std::process::Command runs a .bat/.cmd through cmd.exe with its
        // own batch-argument escaping, so no wrapper is added here.
        let (dir, install, mut g) = sidecar_fixture("bin/start.bat", "");
        g.native_launch_parameters = "--mode \"two words\"".to_string();
        let result = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(
            result.argv,
            vec![
                install
                    .join("bin")
                    .join("start.bat")
                    .to_string_lossy()
                    .into_owned(),
                "--mode".to_string(),
                "two words".to_string(),
            ]
        );
    }

    #[test]
    fn a_powershell_script_on_a_windows_host_runs_through_powershell() {
        let (dir, install, mut g) = sidecar_fixture("bin/run.ps1", "");
        g.native_launch_parameters = "-a \"b c\"".to_string();
        let which = |name: &str| -> Option<PathBuf> {
            (name == "powershell.exe").then(|| PathBuf::from("C:\\PS\\powershell.exe"))
        };
        let result = build_native_command(&g, dir.path(), "", "windows", &which).unwrap();
        let script = install
            .join("bin")
            .join("run.ps1")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            result.argv,
            vec![
                "C:\\PS\\powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                script,
                "-a".to_string(),
                "b c".to_string(),
            ]
        );
        assert_eq!(result.cwd, install.join("bin"));

        // Not on PATH: the bare name, resolved by the OS at spawn.
        let fallback = build_native_command(&g, dir.path(), "", "windows", &no_which).unwrap();
        assert_eq!(fallback.argv[0], "powershell.exe");
    }

    #[test]
    fn a_powershell_script_under_wine_is_left_to_wine() {
        let (dir, install, g) = sidecar_fixture("bin/run.ps1", "");
        let result = build_native_command(&g, dir.path(), "wine", "linux", &no_which).unwrap();
        assert_eq!(
            result.argv,
            vec![
                "wine".to_string(),
                install
                    .join("bin")
                    .join("run.ps1")
                    .to_string_lossy()
                    .into_owned(),
            ]
        );
    }

    // --- invalid custom launch parameters ------------------------------------
    //
    // `split_template`'s POSIX-mode splitter always has a fallback that
    // succeeds (see `launch/template.rs`), so there is no real input that
    // makes it return `Err` today; the wrapping above
    // ("Invalid custom launch parameters: {e}") is defensive/forward-looking
    // and is not independently exercised by a test here.
}
