//! A game's redistributable steps (Amazon's fuel.json `PostInstall`, and
//! next GOG's and Epic's prerequisites): the OpenAL, DirectX or Visual C++
//! installers a game lists, run silently after its install. Store-neutral:
//! a store gives the steps ([`RedistStep`], each a program in the game
//! folder with its arguments), where they run ([`RedistTarget`]) and keeps
//! the list that last worked in its own row meta ([`steps_due`] says when
//! to run them again). User decisions 2026-11-14
//! (`docs/stores-phase3-plan.md` "User decisions" 2).
//!
//! Each step's command is resolved inside the game folder ([`resolve_step`]:
//! either separator, never out of the folder) and runs with its arguments
//! as given, in the game folder (or, for a step marked
//! [`RedistStep::in_exe_folder`], its program's own folder). A `.msi` runs
//! through `msiexec /i`.
//!
//! - **Linux and macOS:** each step runs under the game's compat tool in the
//!   game's own prefix: the command a launch of the game would use
//!   (`umu-run` with `PROTONPATH`, or `wine`, and `WINEPREFIX`), with no
//!   store environment (no FUEL variables). No prompt, TV mode or not. A
//!   step has [`STEP_TIMEOUT`], and one whose prefix stops growing for
//!   [`STEP_STALL`] is stopped too.
//! - **Windows:** redistributables need an administrator, so every step of
//!   one job runs inside one elevated PowerShell runner (one Windows
//!   permission prompt per install or repair, desktop and TV mode alike).
//!   The runner runs the steps in order and writes each step's exit code to
//!   a result file GRID reads back ([`parse_results`]; no secret in it). A
//!   declined prompt is the [`REDIST_DECLINED`] warning: the Downloads row
//!   then offers Retry.
//!
//! Before anything runs, a step whose redistributable is already there is
//! skipped, so nobody sees a prompt or waits for nothing ([`satisfied`]):
//! [`redist_of`] maps a store id or a file name to a known redistributable
//! (OpenAL, DirectX June 2010, Visual C++ 2010–2022, .NET 4.8, and
//! Unreal Engine's prerequisites bundle by its parts), and
//! [`already_installed`] checks it: on Windows its registry value
//! (`VisualStudio\<ver>\VC\Runtimes`, .NET's `Release`) or its system
//! files; in a prefix its files under `drive_c/windows/system32|syswow64`,
//! Wine's own builtin and placeholder DLLs not counting. GRID also notes in
//! the prefix ([`PREFIX_MARKER`]) each step that worked there, and skips it
//! next time unless its program changed. A step with no rule always runs.
//! When every step is satisfied, nothing runs and no phase shows.
//!
//! The machinery is the offline GOG installer's (`stores::offline::exec`):
//! [`build_installer_command`], the spawner (no stdin, output discarded: an
//! installer's output is never read, logged or forwarded), the supervisor
//! and the kill that also stops the prefix's `wineserver`. A step that
//! fails, times out or can't start is a warning line in GRID's words
//! ("Couldn't run oalinst.exe: …"), never a failed install. A cancel stops
//! the running step and skips the rest; the game stays installed.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::launch::native::build_installer_command;
use crate::launch::template::host_os;
use crate::library::queue::{JobPhase, StoreProgressSink};
use crate::stores::gog::row::relative_path;
use crate::stores::offline::exec::{
    stop_prefix, supervise, system_spawner, to_command, tree_growth, which_with_managed_umu,
    windows_path, InstallerCommand, InstallerProcess, InstallerSpawner, RunEnd, Watch,
};

/// How long one step may run (on Windows, the elevated runner gets this
/// per step).
pub const STEP_TIMEOUT: Duration = Duration::from_secs(600);

/// How long a step under a compat tool may run without its prefix growing.
/// Longer than the offline installer's: umu's first run fetches its
/// runtime outside the prefix.
pub const STEP_STALL: Duration = Duration::from_secs(300);

/// The exit codes that mean a step worked when the store gives none: 0,
/// Windows Installer's "a restart is needed" (3010) and "already
/// installed" (1638).
pub const DEFAULT_VALID_RETURNS: [i64; 3] = [0, 1638, 3010];

/// A step whose path leads out of the game folder.
pub const ESCAPES_FOLDER: &str = "its path leads outside the game folder";

/// A step whose file isn't there.
pub const NOT_IN_FOLDER: &str = "it isn't in the game folder";

/// The warning when the Windows permission prompt was declined (or could
/// not be shown). The Downloads row offers Retry on it; the frontend
/// matches this text (`gamestores/downloads.ts`).
pub const REDIST_DECLINED: &str =
    "Its setup steps didn't run: Windows didn't get permission to run them";

/// The elevated runner's exit code for a declined prompt
/// (`ERROR_CANCELLED`).
pub const DECLINED_EXIT: i32 = 1223;

/// The result file's line for a step that could not start.
const START_FAILED: &str = "failed";

/// One redistributable step: a program in the game folder and its
/// arguments, as the store gives them (`{"Command": "oalinst.exe",
/// "Args": ["/silent"]}`). No secret: a store keeps the list that last
/// worked in its row meta.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RedistStep {
    /// The store's id for what the step installs (GOG's `MSVC2015_x64`,
    /// `DirectX`, `openAL`; Epic's prerequisite id), `""` when it gives
    /// none (Amazon): [`redist_of`] then goes by the file name.
    pub id: String,
    /// Relative to the game folder, either separator.
    pub command: String,
    pub args: Vec<String>,
    /// The exit codes that mean it worked; empty for
    /// [`DEFAULT_VALID_RETURNS`].
    pub valid_returns: Vec<i64>,
    /// Starts in its program's own folder instead of the game folder
    /// (Epic's prerequisite, as legendary runs it).
    #[serde(skip_serializing_if = "is_false")]
    pub in_exe_folder: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The folder `step` (program `exe`) starts in: the game folder, or with
/// [`RedistStep::in_exe_folder`] its program's own folder.
pub fn step_dir(step: &RedistStep, exe: &Path, game_dir: &Path) -> PathBuf {
    match exe.parent() {
        Some(dir) if step.in_exe_folder && !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => game_dir.to_path_buf(),
    }
}

// --- what is already installed ---------------------------------------------------

/// A redistributable GRID knows how to detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redist {
    OpenAl,
    /// The DirectX End-User Runtime (June 2010): D3DX9, XAudio 2.7,
    /// D3DCompiler 43.
    DirectXJun2010,
    /// Visual C++ 2010, 2012, 2013 or 2015–2022 (`year` 2010, 2012, 2013,
    /// 2015), 64-bit or 32-bit.
    VcRedist {
        year: u16,
        x64: bool,
    },
    /// .NET Framework 4.8.
    DotNet48,
    /// Unreal Engine's prerequisites bundle (`UE4PrereqSetup_x64.exe`,
    /// `UEPrereqSetup_x64.exe`): Visual C++ 2015–2022 (both builds for the
    /// 64-bit bundle) and the DirectX June 2010 runtime. There when all
    /// its parts are ([`Redist::parts`]).
    UePrereqs {
        x64: bool,
    },
}

impl Redist {
    /// The id GRID's prefix marker keys it by.
    pub fn key(self) -> String {
        match self {
            Redist::OpenAl => "openal".to_string(),
            Redist::DirectXJun2010 => "directx-jun2010".to_string(),
            Redist::VcRedist { year, x64 } => {
                format!("vcredist-{year}-{}", if x64 { "x64" } else { "x86" })
            }
            Redist::DotNet48 => "dotnet48".to_string(),
            Redist::UePrereqs { x64 } => {
                format!("ue-prereqs-{}", if x64 { "x64" } else { "x86" })
            }
        }
    }

    /// What a bundle installs (each checked on its own), `[]` for a single
    /// redistributable.
    pub fn parts(self) -> Vec<Redist> {
        match self {
            Redist::UePrereqs { x64 } => {
                let mut parts = vec![Redist::VcRedist {
                    year: 2015,
                    x64: false,
                }];
                if x64 {
                    parts.push(Redist::VcRedist {
                        year: 2015,
                        x64: true,
                    });
                }
                parts.push(Redist::DirectXJun2010);
                parts
            }
            _ => Vec::new(),
        }
    }
}

/// What a step installs, from the store's id, else its file name (and a
/// year in its path for an older `vcredist_x64.exe`). `None`: no rule, so
/// the step always runs.
///
/// | id (any case) | file name | redistributable |
/// |---|---|---|
/// | `openal` | `oalinst.exe` | OpenAL |
/// | `directx`, `directx_jun2010` | `dxsetup.exe` | DirectX June 2010 |
/// | `msvc2010[_x64]` … `msvc2013[_x64]` | `vcredist_x86/x64.exe` with that year in its path | VC++ of that year |
/// | `msvc2015` … `msvc2022[_x64]` | `vc_redist.x86/x64.exe`, `vcredist_x86/x64.exe` | VC++ 2015–2022 |
/// | `dotnet48`, `dotnet4` | `ndp48-*.exe` | .NET Framework 4.8 |
/// | | `ue4prereqsetup[_x64].exe`, `ueprereqsetup[_x64].exe` | Unreal Engine prerequisites (VC++ 2015–2022, DirectX June 2010) |
pub fn redist_of(step: &RedistStep) -> Option<Redist> {
    let id = step.id.trim().to_lowercase();
    if !id.is_empty() {
        if let Some(found) = redist_of_id(&id) {
            return Some(found);
        }
    }
    let command = step.command.trim().to_lowercase().replace('\\', "/");
    let name = command.rsplit('/').next().unwrap_or("");
    match name {
        "oalinst.exe" => Some(Redist::OpenAl),
        "dxsetup.exe" => Some(Redist::DirectXJun2010),
        "vc_redist.x64.exe" | "vc_redist.x86.exe" => Some(Redist::VcRedist {
            year: 2015,
            x64: name.contains("x64"),
        }),
        "vcredist_x64.exe" | "vcredist_x86.exe" => {
            let year = [2010u16, 2012, 2013]
                .into_iter()
                .find(|year| command.contains(&year.to_string()))
                .unwrap_or(2015);
            Some(Redist::VcRedist {
                year,
                x64: name.contains("x64"),
            })
        }
        _ if name.starts_with("ndp48") && name.ends_with(".exe") => Some(Redist::DotNet48),
        _ if (name.starts_with("ue4prereqsetup") || name.starts_with("ueprereqsetup"))
            && name.ends_with(".exe") =>
        {
            Some(Redist::UePrereqs {
                x64: !name.contains("x86"),
            })
        }
        _ => None,
    }
}

fn redist_of_id(id: &str) -> Option<Redist> {
    match id {
        "openal" => return Some(Redist::OpenAl),
        "directx" | "directx_jun2010" | "directx_june2010" => return Some(Redist::DirectXJun2010),
        "dotnet48" | "dotnet4" | "dotnet_48" => return Some(Redist::DotNet48),
        _ => {}
    }
    let rest = id.strip_prefix("msvc")?;
    let (year, x64) = match rest.strip_suffix("_x64") {
        Some(year) => (year, true),
        None => (rest.strip_suffix("_x86").unwrap_or(rest), false),
    };
    let year = match year.parse::<u16>().ok()? {
        2010 => 2010,
        2012 => 2012,
        2013 => 2013,
        2015 | 2017 | 2019 | 2022 => 2015,
        _ => return None,
    };
    Some(Redist::VcRedist { year, x64 })
}

/// Which Windows system folder a file is checked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bits {
    /// Every system folder there is (32 and 64-bit both installed).
    Both,
    /// `System32` on 64-bit Windows.
    X64,
    /// `SysWOW64` on 64-bit Windows, else `System32`.
    X86,
}

/// The files that show `redist` is installed, and where.
fn files_of(redist: Redist) -> (&'static [&'static str], Bits) {
    match redist {
        Redist::OpenAl => (&["OpenAL32.dll"], Bits::Both),
        Redist::DirectXJun2010 => (
            &["d3dx9_43.dll", "XAudio2_7.dll", "d3dcompiler_43.dll"],
            Bits::Both,
        ),
        Redist::VcRedist { year, x64 } => {
            let files: &'static [&'static str] = match year {
                2010 => &["msvcr100.dll", "msvcp100.dll"],
                2012 => &["msvcr110.dll", "msvcp110.dll"],
                2013 => &["msvcr120.dll", "msvcp120.dll"],
                _ => &["vcruntime140.dll", "msvcp140.dll"],
            };
            (files, if x64 { Bits::X64 } else { Bits::X86 })
        }
        // No file says which .NET release is there; a bundle goes by its
        // parts.
        Redist::DotNet48 | Redist::UePrereqs { .. } => (&[], Bits::Both),
    }
}

/// .NET 4.8's `Release` value.
pub const DOTNET48_RELEASE: u32 = 528_040;

/// The registry value that shows `redist` is installed on a Windows host
/// (a key, a DWORD value, and the least it must be), `None` for one GRID
/// checks by its files.
fn registry_of(redist: Redist) -> Option<(String, &'static str, u32)> {
    match redist {
        Redist::VcRedist { year, x64 } => {
            let arch = if x64 { "x64" } else { "x86" };
            let node = if x64 { "" } else { "WOW6432Node\\" };
            let key = match year {
                2010 => {
                    format!(r"HKLM\SOFTWARE\{node}Microsoft\VisualStudio\10.0\VC\VCRedist\{arch}")
                }
                2012 => {
                    format!(r"HKLM\SOFTWARE\{node}Microsoft\VisualStudio\11.0\VC\Runtimes\{arch}")
                }
                2013 => {
                    format!(r"HKLM\SOFTWARE\{node}Microsoft\VisualStudio\12.0\VC\Runtimes\{arch}")
                }
                _ => format!(r"HKLM\SOFTWARE\{node}Microsoft\VisualStudio\14.0\VC\Runtimes\{arch}"),
            };
            Some((key, "Installed", 1))
        }
        Redist::DotNet48 => Some((
            r"HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full".to_string(),
            "Release",
            DOTNET48_RELEASE,
        )),
        _ => None,
    }
}

/// How GRID looks at a system: files, and on Windows the registry.
pub trait SystemProbe: Send + Sync {
    /// Whether `path` is a real file, not one of Wine's builtin or
    /// placeholder DLLs (which every prefix has for many redistributables).
    fn real_file(&self, path: &Path) -> bool;
    /// A DWORD registry value (Windows host only; `None` elsewhere or when
    /// it isn't there).
    fn reg_dword(&self, key: &str, name: &str) -> Option<u32>;
}

/// The real probe: the file system, and `reg query` on Windows.
#[derive(Debug, Clone, Copy, Default)]
pub struct HostProbe;

impl SystemProbe for HostProbe {
    fn real_file(&self, path: &Path) -> bool {
        use std::io::Read;
        let Ok(mut file) = fs::File::open(path) else {
            return false;
        };
        if !file.metadata().is_ok_and(|m| m.is_file()) {
            return false;
        }
        let mut head = [0u8; 0x80];
        let n = file.read(&mut head).unwrap_or(0);
        !is_wine_stub(&head[..n])
    }

    fn reg_dword(&self, key: &str, name: &str) -> Option<u32> {
        if !host_os().starts_with("win") {
            return None;
        }
        let out = std::process::Command::new("reg")
            .args(["query", key, "/v", name])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        parse_reg_dword(&String::from_utf8_lossy(&out.stdout), name)
    }
}

/// Whether a DLL's first bytes are Wine's builtin or placeholder stub.
pub fn is_wine_stub(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    text.contains("Wine builtin DLL") || text.contains("Wine placeholder DLL")
}

/// `reg query <key> /v <name>`'s value for a `REG_DWORD`.
pub fn parse_reg_dword(output: &str, name: &str) -> Option<u32> {
    output.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if !parts.next()?.eq_ignore_ascii_case(name) || parts.next()? != "REG_DWORD" {
            return None;
        }
        let value = parts.next()?;
        let hex = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))?;
        u32::from_str_radix(hex, 16).ok()
    })
}

/// The system folders a check reads: the prefix's
/// (`drive_c/windows/system32` and `syswow64`), or the Windows host's
/// (`System32`, `SysWOW64` under `system_root`). Only folders that exist.
fn system_dirs(
    prefix: Option<&Path>,
    system_root: Option<&Path>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let (sys32, wow) = match (prefix, system_root) {
        (Some(prefix), _) => {
            let windows = prefix.join("drive_c").join("windows");
            (windows.join("system32"), windows.join("syswow64"))
        }
        (None, Some(root)) => (root.join("System32"), root.join("SysWOW64")),
        (None, None) => return (None, None),
    };
    (sys32.is_dir().then_some(sys32), wow.is_dir().then_some(wow))
}

/// Whether `redist` is already installed: in `prefix` (Linux, macOS) by its
/// real files; on a Windows host (`prefix` `None`) by its registry value,
/// else its files under `system_root`. `false` when nothing says so.
pub fn already_installed(
    redist: Redist,
    prefix: Option<&Path>,
    system_root: Option<&Path>,
    probe: &dyn SystemProbe,
) -> bool {
    let parts = redist.parts();
    if !parts.is_empty() {
        return parts
            .into_iter()
            .all(|part| already_installed(part, prefix, system_root, probe));
    }
    if prefix.is_none() {
        if let Some((key, name, least)) = registry_of(redist) {
            if probe.reg_dword(&key, name).is_some_and(|v| v >= least) {
                return true;
            }
        }
    }
    let (files, bits) = files_of(redist);
    if files.is_empty() {
        return false;
    }
    let (sys32, wow) = system_dirs(prefix, system_root);
    let dirs: Vec<PathBuf> = match bits {
        Bits::Both => sys32.into_iter().chain(wow).collect(),
        // On 32-bit Windows (no SysWOW64) there is no 64-bit folder.
        Bits::X64 => wow
            .is_some()
            .then_some(sys32)
            .flatten()
            .into_iter()
            .collect(),
        Bits::X86 => wow.or(sys32).into_iter().collect(),
    };
    !dirs.is_empty()
        && dirs
            .iter()
            .all(|dir| files.iter().all(|file| probe.real_file(&dir.join(file))))
}

/// The file in a prefix where GRID notes the steps that worked there.
pub const PREFIX_MARKER: &str = ".grid-redist.json";

/// One step GRID ran in a prefix: what it installs (the [`Redist`] key, or
/// the step's command in lower case) and its program's size, which stands
/// in for its version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MarkerEntry {
    pub key: String,
    pub size: u64,
}

/// The marker entry for `step`, whose program is `exe`.
pub fn marker_entry(step: &RedistStep, exe: &Path) -> MarkerEntry {
    MarkerEntry {
        key: redist_of(step)
            .map(Redist::key)
            .unwrap_or_else(|| step.command.trim().to_lowercase().replace('\\', "/")),
        size: fs::metadata(exe).map(|m| m.len()).unwrap_or(0),
    }
}

/// The prefix's marker (empty when missing or unreadable).
pub fn read_marker(prefix: &Path) -> Vec<MarkerEntry> {
    fs::read_to_string(prefix.join(PREFIX_MARKER))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn add_to_marker(prefix: &Path, entries: Vec<MarkerEntry>) {
    if entries.is_empty() || !prefix.is_dir() {
        return;
    }
    let mut marker = read_marker(prefix);
    for entry in entries {
        marker.retain(|old| old.key != entry.key);
        marker.push(entry);
    }
    if let Ok(text) = serde_json::to_string(&marker) {
        let _ = fs::write(prefix.join(PREFIX_MARKER), text);
    }
}

/// Whether `step` (program `exe`) needs no run: what it installs is
/// already there ([`already_installed`]), or GRID already ran this same
/// step (same key, same program size) in `prefix`.
pub fn satisfied(
    step: &RedistStep,
    exe: &Path,
    prefix: Option<&Path>,
    system_root: Option<&Path>,
    probe: &dyn SystemProbe,
) -> bool {
    if let Some(prefix) = prefix {
        if read_marker(prefix).contains(&marker_entry(step, exe)) {
            return true;
        }
    }
    redist_of(step).is_some_and(|redist| already_installed(redist, prefix, system_root, probe))
}

/// `which` for the step command (`umu-run`, `wine`).
pub type Which = Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;

/// What runs the steps: the process spawner, the program lookup, the host
/// and the timing. [`RedistRunner::system`] in the app; tests swap in a
/// scripted spawner.
#[derive(Clone)]
pub struct RedistRunner {
    pub spawner: Arc<dyn InstallerSpawner>,
    pub which: Which,
    /// [`host_os`]'s value: `"windows"` runs the steps elevated.
    pub host: String,
    pub watch: Watch,
    pub timeout: Duration,
    /// How "already installed" is checked ([`satisfied`]).
    pub probe: Arc<dyn SystemProbe>,
    /// The Windows host's `%SystemRoot%` (`None` elsewhere).
    pub system_root: Option<PathBuf>,
}

impl RedistRunner {
    /// The real runner: [`system_spawner`], `which` with GRID's managed
    /// umu-run, this host, [`STEP_STALL`], [`STEP_TIMEOUT`], [`HostProbe`]
    /// and `%SystemRoot%`.
    pub fn system() -> RedistRunner {
        let host = host_os();
        RedistRunner {
            probe: Arc::new(HostProbe),
            system_root: host
                .starts_with("win")
                .then(|| std::env::var_os("SystemRoot").map(PathBuf::from))
                .flatten(),
            spawner: system_spawner(),
            which: Arc::new(which_with_managed_umu),
            host: host.to_string(),
            watch: Watch {
                stall: STEP_STALL,
                poll: Duration::from_secs(2),
            },
            timeout: STEP_TIMEOUT,
        }
    }

    fn on_windows(&self) -> bool {
        self.host.trim().to_lowercase().starts_with("win")
    }
}

/// Whether a finished job runs `steps`: a first install (`first`) always,
/// when there are any; an update or a repair only when they differ from
/// `ran`, the list that last worked.
pub fn steps_due(first: bool, steps: &[RedistStep], ran: &[RedistStep]) -> bool {
    !steps.is_empty() && (first || steps != ran)
}

/// The name a warning gives a step: its file name (`DXSETUP.exe`).
pub fn step_name(step: &RedistStep) -> String {
    step.command
        .trim()
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or(step.command.trim())
        .to_string()
}

/// The step's program in `game_dir`: `command` with either separator, as
/// a path that must stay inside the folder (no `..`, no drive letter, no
/// absolute path, no link out of it). `Err` is the warning's reason:
/// [`ESCAPES_FOLDER`] or [`NOT_IN_FOLDER`].
pub fn resolve_step(game_dir: &Path, command: &str) -> Result<PathBuf, &'static str> {
    let rel = relative_path(command).ok_or(ESCAPES_FOLDER)?;
    let path = game_dir.join(rel);
    if !path.is_file() {
        return Err(NOT_IN_FOLDER);
    }
    let (Ok(real), Ok(root)) = (path.canonicalize(), game_dir.canonicalize()) else {
        return Err(NOT_IN_FOLDER);
    };
    if !real.starts_with(&root) {
        return Err(ESCAPES_FOLDER);
    }
    Ok(path)
}

/// Whether `path` is a Windows Installer package.
pub fn is_msi(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
}

/// The command for one step off Windows: [`build_installer_command`] (the
/// compat tool and prefix as a launch wraps the game), started in
/// `game_dir` (the step's folder: [`step_dir`]). A `.msi` runs as
/// `msiexec /i <package> <args>`. `Err` is the warning's reason (no compat
/// tool, umu-run missing).
pub fn step_command(
    exe: &Path,
    args: &[String],
    game_dir: &Path,
    compat: &str,
    prefix: Option<&Path>,
    host: &str,
    which: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<InstallerCommand, String> {
    let native = host.trim().to_lowercase().starts_with("win");
    let (program, args) = if is_msi(exe) {
        let mut all = vec!["/i".to_string(), windows_path(exe, native)];
        all.extend(args.iter().cloned());
        (PathBuf::from("msiexec"), all)
    } else {
        (exe.to_path_buf(), args.to_vec())
    };
    let launch = build_installer_command(&program, args, compat, prefix, host, which)
        .map_err(|error| error.to_string())?;
    let mut cmd = to_command(launch, prefix, compat);
    cmd.cwd = game_dir.to_path_buf();
    Ok(cmd)
}

/// Whether a step's exit means it worked: one of `valid`, else of
/// [`DEFAULT_VALID_RETURNS`]. A kill (`None`) never did.
pub fn step_worked(code: Option<i32>, valid: &[i64]) -> bool {
    let Some(code) = code else {
        return false;
    };
    let code = i64::from(code);
    if valid.is_empty() {
        DEFAULT_VALID_RETURNS.contains(&code)
    } else {
        valid.contains(&code)
    }
}

// --- the Windows runner --------------------------------------------------------

/// `text` as a PowerShell single-quoted string.
fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// `args` as one Windows command line (each argument quoted when it holds
/// a space, a tab or a quote, or is empty).
pub fn windows_command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
                return arg.clone();
            }
            let mut out = String::from("\"");
            let mut slashes = 0;
            for c in arg.chars() {
                match c {
                    '\\' => slashes += 1,
                    '"' => {
                        out.push_str(&"\\".repeat(slashes * 2 + 1));
                        out.push('"');
                        slashes = 0;
                    }
                    _ => {
                        out.push_str(&"\\".repeat(slashes));
                        out.push(c);
                        slashes = 0;
                    }
                }
            }
            out.push_str(&"\\".repeat(slashes * 2));
            out.push('"');
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One step of the elevated runner: its program, its arguments and the
/// folder it starts in ([`step_dir`]).
pub type ScriptStep = (PathBuf, Vec<String>, PathBuf);

/// The elevated runner's script: each step in order (`Start-Process
/// -Wait`, in its own folder; a `.msi` through `msiexec.exe /i`), its index
/// and exit code (or `failed`) appended to `result` as one line.
pub fn runner_script(steps: &[ScriptStep], game_dir: &Path, result: &Path) -> String {
    let game = ps_quote(&game_dir.to_string_lossy());
    let mut script = format!(
        "$ErrorActionPreference = 'Stop'\n$result = {}\nSet-Location -LiteralPath {game}\n",
        ps_quote(&result.to_string_lossy())
    );
    for (index, (exe, args, work)) in steps.iter().enumerate() {
        let dir = ps_quote(&work.to_string_lossy());
        let (file, args) = if is_msi(exe) {
            let mut all = vec!["/i".to_string(), exe.to_string_lossy().into_owned()];
            all.extend(args.iter().cloned());
            ("msiexec.exe".to_string(), all)
        } else {
            (exe.to_string_lossy().into_owned(), args.clone())
        };
        let list = if args.is_empty() {
            String::new()
        } else {
            format!(" -ArgumentList {}", ps_quote(&windows_command_line(&args)))
        };
        script.push_str(&format!(
            "try {{ $p = Start-Process -FilePath {}{list} -WorkingDirectory {dir} -Wait -PassThru; \
             Add-Content -LiteralPath $result -Value \"{index} $($p.ExitCode)\" }} \
             catch {{ Add-Content -LiteralPath $result -Value '{index} {START_FAILED}' }}\n",
            ps_quote(&file)
        ));
    }
    script
}

/// The command that starts the runner `script` elevated (one Windows
/// permission prompt) and waits for it; it exits [`DECLINED_EXIT`] when
/// the prompt is declined.
pub fn elevated_command(script: &Path, game_dir: &Path) -> InstallerCommand {
    let inner = format!(
        "-NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"",
        script.to_string_lossy()
    );
    let outer = format!(
        "try {{ Start-Process -FilePath 'powershell.exe' -ArgumentList {} -Verb RunAs -Wait \
         -WindowStyle Hidden -ErrorAction Stop; exit 0 }} catch {{ exit {DECLINED_EXIT} }}",
        ps_quote(&inner)
    );
    InstallerCommand {
        argv: [
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
        ]
        .iter()
        .map(|s| s.to_string())
        .chain([outer])
        .collect(),
        env: Vec::new(),
        cwd: game_dir.to_path_buf(),
        prefix: None,
        compat: String::new(),
    }
}

/// How one step went in the runner's result file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    Exited(i32),
    StartFailed,
}

/// The runner's result file: one entry per step (`None` for a step with
/// no line: it never ran). Unreadable lines are ignored.
pub fn parse_results(text: &str, steps: usize) -> Vec<Option<StepResult>> {
    let mut out = vec![None; steps];
    for line in text.lines() {
        let mut parts = line
            .trim()
            .trim_start_matches('\u{feff}')
            .split_whitespace();
        let (Some(index), Some(value)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let result = if value == START_FAILED {
            StepResult::StartFailed
        } else if let Ok(code) = value.parse::<i32>() {
            StepResult::Exited(code)
        } else {
            continue;
        };
        if let Some(slot) = out.get_mut(index) {
            *slot = Some(result);
        }
    }
    out
}

// --- running -------------------------------------------------------------------

/// Where the steps run.
#[derive(Debug, Clone, Copy)]
pub struct RedistTarget<'a> {
    /// The game folder: where each command is resolved and started.
    pub game_dir: &'a Path,
    /// The compat tool (the row's, else the default); ignored on Windows.
    pub compat: &'a str,
    /// The game's prefix (`None` on Windows).
    pub prefix: Option<&'a Path>,
}

/// How a run of the steps went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedistOutcome {
    /// One line per step that didn't work, one for a cancel, and
    /// [`REDIST_DECLINED`] for a declined prompt.
    pub warnings: Vec<String>,
    /// Every step ran and worked: the store records the list as run.
    pub all_worked: bool,
    /// The user cancelled during the steps.
    pub cancelled: bool,
    /// The Windows permission prompt was declined.
    pub declined: bool,
}

impl RedistOutcome {
    /// The warnings as a finished Downloads entry's warning (one per
    /// line), `""` for none.
    pub fn warning(&self) -> String {
        self.warnings.join("\n")
    }
}

/// The cancel line.
pub const REDIST_CANCELLED: &str =
    "Cancelled its setup steps: the game is installed, but they didn't all run";

/// A step's warning line.
fn couldnt_run(name: &str, reason: &str) -> String {
    format!("Couldn't run {name}: {}", reason.trim())
}

/// A launch error's sentence ("Choose a compatibility tool…") read on
/// after a colon: its first letter in lower case.
fn read_on(reason: &str) -> String {
    let reason = reason.trim();
    let mut chars = reason.chars();
    match chars.next() {
        Some(first) if chars.next().is_some_and(char::is_lowercase) => {
            first.to_lowercase().chain(reason.chars().skip(1)).collect()
        }
        _ => reason.to_string(),
    }
}

fn minutes(duration: Duration) -> String {
    let n = (duration.as_secs() / 60).max(1);
    if n == 1 {
        "1 minute".to_string()
    } else {
        format!("{n} minutes")
    }
}

fn exit_reason(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("it exited with code {code}"),
        None => "it was stopped".to_string(),
    }
}

/// Runs `steps` in order. The Downloads row shows "Running the installer
/// silently" ([`JobPhase::InstallerSilent`], the cancellable phase) with
/// one progress step per step. Never fails: every problem is a warning
/// line. Blocking.
pub fn run_steps(
    steps: &[RedistStep],
    target: RedistTarget<'_>,
    runner: &RedistRunner,
    sink: &mut dyn StoreProgressSink,
) -> RedistOutcome {
    let mut outcome = RedistOutcome::default();
    let prefix = target.prefix.filter(|_| !runner.on_windows());
    // What is already installed (or already ran here) needs no run, so no
    // prompt and no wait. A step GRID can't resolve still goes on, for
    // its warning.
    let todo: Vec<RedistStep> = steps
        .iter()
        .filter(|step| match resolve_step(target.game_dir, &step.command) {
            Ok(exe) => !satisfied(
                step,
                &exe,
                prefix,
                runner.system_root.as_deref(),
                runner.probe.as_ref(),
            ),
            Err(_) => true,
        })
        .cloned()
        .collect();
    if !todo.is_empty() {
        sink.phase(JobPhase::InstallerSilent);
        let worked = if runner.on_windows() {
            run_elevated(&todo, target, runner, sink, &mut outcome)
        } else {
            run_wrapped(&todo, target, runner, sink, &mut outcome)
        };
        if let Some(prefix) = prefix {
            add_to_marker(
                prefix,
                worked
                    .iter()
                    .map(|(step, exe)| marker_entry(step, exe))
                    .collect(),
            );
        }
        let total = todo.len() as u64;
        sink.progress(total, total);
    }
    if outcome.cancelled {
        outcome.warnings.push(REDIST_CANCELLED.to_string());
    }
    outcome.all_worked = outcome.warnings.is_empty();
    outcome
}

/// The steps that worked, with their programs.
type Worked = Vec<(RedistStep, PathBuf)>;

/// Linux and macOS: each step under the compat tool, one at a time.
fn run_wrapped(
    steps: &[RedistStep],
    target: RedistTarget<'_>,
    runner: &RedistRunner,
    sink: &mut dyn StoreProgressSink,
    outcome: &mut RedistOutcome,
) -> Worked {
    let mut worked = Worked::new();
    let total = steps.len() as u64;
    for (index, step) in steps.iter().enumerate() {
        let name = step_name(step);
        if sink.cancelled() {
            outcome.cancelled = true;
            return worked;
        }
        sink.progress(index as u64, total);
        let exe = match resolve_step(target.game_dir, &step.command) {
            Ok(exe) => exe,
            Err(reason) => {
                outcome.warnings.push(couldnt_run(&name, reason));
                continue;
            }
        };
        let cmd = match step_command(
            &exe,
            &step.args,
            &step_dir(step, &exe, target.game_dir),
            target.compat,
            target.prefix,
            &runner.host,
            runner.which.as_ref(),
        ) {
            Ok(cmd) => cmd,
            Err(reason) => {
                outcome.warnings.push(couldnt_run(&name, &read_on(&reason)));
                continue;
            }
        };
        let stall = cmd.prefix.is_some();
        let reason = match run_one(&cmd, runner, runner.timeout, stall, sink) {
            Ok(StepEnd::Ended(RunEnd::Exited(code))) if step_worked(code, &step.valid_returns) => {
                worked.push((step.clone(), exe));
                continue;
            }
            Ok(StepEnd::Ended(RunEnd::Exited(code))) => exit_reason(code),
            Ok(StepEnd::Ended(RunEnd::Stalled)) => format!(
                "it showed no progress for {}, so GRID stopped it",
                minutes(runner.watch.stall)
            ),
            Ok(StepEnd::TimedOut) => format!(
                "it didn't finish in {}, so GRID stopped it",
                minutes(runner.timeout)
            ),
            Ok(StepEnd::Ended(RunEnd::Cancelled)) => {
                outcome.cancelled = true;
                return worked;
            }
            Err(kind) => format!("it couldn't be started ({kind})"),
        };
        outcome.warnings.push(couldnt_run(&name, &reason));
    }
    worked
}

/// Windows: every runnable step in one elevated runner (one prompt), then
/// its result file.
fn run_elevated(
    steps: &[RedistStep],
    target: RedistTarget<'_>,
    runner: &RedistRunner,
    sink: &mut dyn StoreProgressSink,
    outcome: &mut RedistOutcome,
) -> Worked {
    sink.progress(0, steps.len() as u64);
    // Resolve first: refused steps are warnings; the rest run in order.
    let mut runnable: Vec<(usize, PathBuf)> = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        match resolve_step(target.game_dir, &step.command) {
            Ok(exe) => runnable.push((index, exe)),
            Err(reason) => outcome.warnings.push(couldnt_run(&step_name(step), reason)),
        }
    }
    if runnable.is_empty() {
        return Worked::new();
    }
    if sink.cancelled() {
        outcome.cancelled = true;
        return Worked::new();
    }
    let work = match work_dir(target.game_dir) {
        Ok(dir) => dir,
        Err(e) => {
            for (index, _) in &runnable {
                let reason = format!("GRID couldn't prepare it ({})", e.kind());
                outcome
                    .warnings
                    .push(couldnt_run(&step_name(&steps[*index]), &reason));
            }
            return Worked::new();
        }
    };
    let script_path = work.path().join("grid-setup-steps.ps1");
    let result_path = work.path().join("grid-setup-results.txt");
    let items: Vec<ScriptStep> = runnable
        .iter()
        .map(|(index, exe)| {
            let step = &steps[*index];
            (
                exe.clone(),
                step.args.clone(),
                step_dir(step, exe, target.game_dir),
            )
        })
        .collect();
    let script = runner_script(&items, target.game_dir, &result_path);
    if let Err(e) = fs::write(&script_path, script) {
        for (index, _) in &runnable {
            let reason = format!("GRID couldn't prepare it ({})", e.kind());
            outcome
                .warnings
                .push(couldnt_run(&step_name(&steps[*index]), &reason));
        }
        return Worked::new();
    }
    let cmd = elevated_command(&script_path, target.game_dir);
    let budget = runner
        .timeout
        .saturating_mul(u32::try_from(runnable.len()).unwrap_or(u32::MAX));
    let end = run_one(&cmd, runner, budget, false, sink);
    let results = parse_results(
        &fs::read_to_string(&result_path).unwrap_or_default(),
        runnable.len(),
    );
    match end {
        Ok(StepEnd::Ended(RunEnd::Cancelled)) => {
            outcome.cancelled = true;
            return Worked::new();
        }
        Ok(StepEnd::Ended(RunEnd::Exited(Some(DECLINED_EXIT)))) => {
            outcome.declined = true;
            outcome.warnings.push(REDIST_DECLINED.to_string());
            return Worked::new();
        }
        Err(kind) => {
            outcome.warnings.push(format!(
                "Its setup steps didn't run: Windows couldn't start them ({kind})"
            ));
            return Worked::new();
        }
        _ => {}
    }
    let timed_out = matches!(end, Ok(StepEnd::TimedOut));
    let mut worked = Worked::new();
    for ((index, exe), result) in runnable.iter().zip(results) {
        let step = &steps[*index];
        let reason = match result {
            Some(StepResult::Exited(code)) if step_worked(Some(code), &step.valid_returns) => {
                worked.push((step.clone(), exe.clone()));
                continue;
            }
            Some(StepResult::Exited(code)) => exit_reason(Some(code)),
            Some(StepResult::StartFailed) => "Windows couldn't start it".to_string(),
            None if timed_out => format!(
                "it didn't finish in {}, so GRID stopped waiting",
                minutes(runner.timeout)
            ),
            None => "it didn't run".to_string(),
        };
        outcome
            .warnings
            .push(couldnt_run(&step_name(step), &reason));
    }
    worked
}

/// A fresh folder beside the game folder for the runner's script and
/// result file (gone when dropped), else the system's temp folder.
fn work_dir(game_dir: &Path) -> std::io::Result<tempfile::TempDir> {
    let parent = game_dir.parent().unwrap_or(game_dir);
    tempfile::Builder::new()
        .prefix(".grid-setup-")
        .tempdir_in(parent)
        .or_else(|_| tempfile::tempdir())
}

/// How one process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepEnd {
    Ended(RunEnd),
    TimedOut,
}

/// Starts `cmd` and waits for it: the user's cancel and `timeout` kill it
/// (and its prefix's processes); with `stall`, so does a prefix that stops
/// growing. `Err` is the spawn error's kind.
fn run_one(
    cmd: &InstallerCommand,
    runner: &RedistRunner,
    timeout: Duration,
    stall: bool,
    sink: &mut dyn StoreProgressSink,
) -> Result<StepEnd, std::io::ErrorKind> {
    let mut process: Box<dyn InstallerProcess> = runner.spawner.spawn(cmd).map_err(|e| e.kind())?;
    let prefix = cmd.prefix.clone();
    let mut growth = || prefix.as_deref().map(tree_growth).unwrap_or(0);
    let deadline = Instant::now() + timeout;
    let timed_out = Cell::new(false);
    let sink = RefCell::new(sink);
    let stop = || {
        if sink.borrow().cancelled() {
            return true;
        }
        if Instant::now() >= deadline {
            timed_out.set(true);
            return true;
        }
        false
    };
    let end = match supervise(
        process.as_mut(),
        &mut growth,
        runner.watch,
        stall,
        &stop,
        &mut |_| {},
    ) {
        Ok(end) => end,
        Err(e) => {
            process.kill();
            stop_prefix(cmd);
            return Err(e.kind());
        }
    };
    if matches!(end, RunEnd::Stalled | RunEnd::Cancelled) {
        stop_prefix(cmd);
    }
    Ok(match end {
        RunEnd::Cancelled if timed_out.get() => StepEnd::TimedOut,
        other => StepEnd::Ended(other),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::native::INSTALLER_NEEDS_COMPAT_MESSAGE;
    use std::io;
    use std::sync::Mutex;

    fn step(command: &str, args: &[&str]) -> RedistStep {
        RedistStep {
            id: String::new(),
            command: command.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            valid_returns: vec![],
            in_exe_folder: false,
        }
    }

    #[test]
    fn a_first_install_runs_any_steps_and_a_changed_list_reruns() {
        let oal = vec![step("oalinst.exe", &["/silent"])];
        let both = vec![
            step("oalinst.exe", &["/silent"]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
        ];
        assert!(!steps_due(true, &[], &[]));
        assert!(
            steps_due(true, &oal, &oal),
            "a first install always runs them"
        );
        assert!(!steps_due(false, &oal, &oal), "unchanged");
        assert!(steps_due(false, &both, &oal), "a step was added");
        assert!(steps_due(false, &oal, &[]), "never ran (a row from before)");
        assert!(
            steps_due(false, &[step("oalinst.exe", &["/s"])], &oal),
            "new arguments"
        );
        assert!(!steps_due(false, &[], &oal), "nothing to run");
    }

    #[test]
    fn a_step_resolves_inside_the_game_folder_with_either_separator() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Hotline Miami");
        fs::create_dir_all(game.join("DirectX")).unwrap();
        fs::write(game.join("DirectX/DXSETUP.exe"), b"MZ").unwrap();
        fs::write(game.join("oalinst.exe"), b"MZ").unwrap();
        fs::write(dir.path().join("evil.exe"), b"MZ").unwrap();

        assert_eq!(
            resolve_step(&game, "DirectX\\DXSETUP.exe").unwrap(),
            game.join("DirectX").join("DXSETUP.exe")
        );
        assert_eq!(
            resolve_step(&game, "./DirectX/DXSETUP.exe").unwrap(),
            game.join("DirectX").join("DXSETUP.exe")
        );
        assert_eq!(
            resolve_step(&game, "oalinst.exe").unwrap(),
            game.join("oalinst.exe")
        );
        for escape in [
            "..\\evil.exe",
            "../evil.exe",
            "/bin/sh",
            "C:\\x.exe",
            "\\evil.exe",
            "",
        ] {
            assert_eq!(
                resolve_step(&game, escape),
                Err(ESCAPES_FOLDER),
                "{escape:?}"
            );
        }
        assert_eq!(resolve_step(&game, "redist\\vc.exe"), Err(NOT_IN_FOLDER));
        assert_eq!(
            resolve_step(&game, "DirectX"),
            Err(NOT_IN_FOLDER),
            "a folder"
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("evil.exe"), game.join("link.exe")).unwrap();
            assert_eq!(resolve_step(&game, "link.exe"), Err(ESCAPES_FOLDER));
        }
    }

    #[test]
    fn step_names_and_exit_codes() {
        assert_eq!(step_name(&step("DirectX\\DXSETUP.exe", &[])), "DXSETUP.exe");
        assert_eq!(step_name(&step("a/b/oalinst.exe", &[])), "oalinst.exe");
        assert_eq!(step_name(&step("x.exe\\", &[])), "x.exe");
        assert!(step_worked(Some(0), &[]));
        assert!(step_worked(Some(3010), &[]));
        assert!(step_worked(Some(1638), &[]));
        assert!(!step_worked(Some(1641), &[]));
        assert!(!step_worked(Some(1), &[]));
        assert!(!step_worked(None, &[]));
        assert!(step_worked(Some(5), &[0, 5]));
        assert!(!step_worked(Some(3010), &[0]), "the store's list wins");
    }

    #[test]
    fn under_umu_a_step_runs_like_the_launch_in_the_games_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Game");
        let prefix = game.join("prefix");
        let exe = game.join("oalinst.exe");
        let which = |name: &str| match name {
            "umu-run" => Some(PathBuf::from("/usr/bin/umu-run")),
            "wine" => Some(PathBuf::from("/usr/bin/wine")),
            _ => None,
        };
        let args = vec!["/silent".to_string()];
        let cmd = step_command(
            &exe,
            &args,
            &game,
            "GE-Proton9-20",
            Some(&prefix),
            "linux",
            &which,
        )
        .unwrap();
        assert_eq!(
            cmd.argv,
            vec![
                "/usr/bin/umu-run".to_string(),
                exe.to_string_lossy().into_owned(),
                "/silent".to_string()
            ]
        );
        assert_eq!(cmd.cwd, game, "the game folder, not the step's own");
        assert!(cmd
            .env
            .contains(&("PROTONPATH".into(), "GE-Proton9-20".into())));
        assert!(cmd
            .env
            .contains(&("WINEPREFIX".into(), prefix.to_string_lossy().into_owned())));
        assert!(!cmd
            .env
            .iter()
            .any(|(k, _)| k.starts_with("AMAZON_GAMES") || k == "FUEL_DIR"));
        assert_eq!(cmd.prefix.as_deref(), Some(prefix.as_path()));
        assert_eq!(cmd.compat, "GE-Proton9-20");
        assert!(prefix.is_dir(), "the prefix is made first");

        let wine =
            step_command(&exe, &args, &game, "wine", Some(&prefix), "linux", &which).unwrap();
        assert_eq!(wine.argv[0], "/usr/bin/wine");

        // A .msi goes through msiexec /i, with the package as Wine sees it.
        let msi = game.join("redist").join("oal.msi");
        let cmd = step_command(&msi, &args, &game, "wine", Some(&prefix), "linux", &which).unwrap();
        assert_eq!(cmd.argv[1], "msiexec");
        assert_eq!(cmd.argv[2], "/i");
        assert_eq!(cmd.argv[3], windows_path(&msi, false));
        assert!(cmd.argv[3].starts_with("Z:"));
        assert_eq!(cmd.argv[4], "/silent");

        // No compat tool, or no umu-run: the warning's reason.
        let error =
            step_command(&exe, &args, &game, "", Some(&prefix), "linux", &which).unwrap_err();
        assert_eq!(error, INSTALLER_NEEDS_COMPAT_MESSAGE);
        let error = step_command(
            &exe,
            &args,
            &game,
            "GE-Proton9-20",
            Some(&prefix),
            "linux",
            &|_: &str| None,
        )
        .unwrap_err();
        assert!(error.contains("umu-run"), "{error}");
    }

    #[test]
    fn on_windows_every_step_runs_in_one_elevated_runner() {
        let game = Path::new("C:\\Games\\Hotline Miami");
        let here = game.to_path_buf();
        let steps = vec![
            (
                game.join("oalinst.exe"),
                vec!["/silent".to_string()],
                here.clone(),
            ),
            (
                game.join("DirectX\\DXSETUP.exe"),
                vec!["/silent".to_string(), "two words".to_string()],
                PathBuf::from("C:\\Games\\Hotline Miami\\DirectX"),
            ),
            (game.join("vc.msi"), vec!["/qn".to_string()], here.clone()),
            (game.join("it's.exe"), vec![], here),
        ];
        let result = Path::new("C:\\Games\\.grid-setup-x\\grid-setup-results.txt");
        let script = runner_script(&steps, game, result);
        let lines: Vec<&str> = script.lines().collect();
        assert_eq!(lines.len(), 3 + 4);
        assert!(lines[1].contains("grid-setup-results.txt"));
        assert!(
            lines[3].contains("-FilePath 'C:\\Games\\Hotline Miami"),
            "{}",
            lines[3]
        );
        assert!(lines[3].contains("-ArgumentList '/silent'"));
        assert!(lines[3].contains("-WorkingDirectory 'C:\\Games\\Hotline Miami' -Wait"));
        assert!(
            lines[4].contains("-WorkingDirectory 'C:\\Games\\Hotline Miami\\DirectX' -Wait"),
            "a step in its own folder: {}",
            lines[4]
        );
        assert!(lines[3].contains("-Wait -PassThru"));
        assert!(lines[3].contains("\"0 $($p.ExitCode)\""));
        assert!(lines[3].contains("'0 failed'"));
        assert!(
            lines[4].contains("-ArgumentList '/silent \"two words\"'"),
            "{}",
            lines[4]
        );
        assert!(lines[5].contains("-FilePath 'msiexec.exe'"));
        assert!(
            lines[5].contains("-ArgumentList '/i \"C:\\Games\\Hotline Miami"),
            "{}",
            lines[5]
        );
        assert!(lines[6].contains("it''s.exe"), "a quote is doubled");
        assert!(!lines[6].contains("-ArgumentList"), "no arguments");
        assert!(
            !script.contains("RunAs"),
            "the runner itself is already elevated"
        );

        let cmd = elevated_command(Path::new("C:\\t\\grid-setup-steps.ps1"), game);
        assert_eq!(cmd.argv[0], "powershell.exe");
        let outer = cmd.argv.last().unwrap();
        assert_eq!(outer.matches("RunAs").count(), 1, "one prompt");
        assert!(outer.contains("-File \"C:\\t\\grid-setup-steps.ps1\""));
        assert!(outer.contains("-Wait"));
        assert!(outer.contains(&format!("exit {DECLINED_EXIT}")));
        assert!(cmd.env.is_empty());
        assert_eq!(cmd.prefix, None);
        assert_eq!(cmd.cwd, game);
    }

    #[test]
    fn windows_command_lines_and_result_files() {
        let line = |args: &[&str]| {
            windows_command_line(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
        };
        assert_eq!(line(&["/silent", "/q"]), "/silent /q");
        assert_eq!(line(&["a b", ""]), "\"a b\" \"\"");
        assert_eq!(line(&["say \"hi\""]), "\"say \\\"hi\\\"\"");
        assert_eq!(line(&["C:\\x y\\"]), "\"C:\\x y\\\\\"");

        let results = parse_results("\u{feff}0 0\r\n1 failed\njunk\n2 x\n9 0\n3 3010\n", 4);
        assert_eq!(
            results,
            vec![
                Some(StepResult::Exited(0)),
                Some(StepResult::StartFailed),
                None,
                Some(StepResult::Exited(3010)),
            ]
        );
        assert_eq!(parse_results("", 2), vec![None, None]);
    }

    // --- run_steps with a scripted spawner ------------------------------------

    /// What a scripted process does when it ends: write result lines (the
    /// elevated runner) and exit.
    struct Proc {
        polls: usize,
        exit: Option<Option<i32>>,
        write: Option<(PathBuf, String)>,
        killed: Arc<Mutex<usize>>,
    }

    impl InstallerProcess for Proc {
        fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
            if self.polls == 0 {
                if let Some((path, text)) = self.write.take() {
                    fs::write(path, text).unwrap();
                }
                return Ok(self.exit);
            }
            self.polls -= 1;
            Ok(None)
        }
        fn kill(&mut self) {
            *self.killed.lock().unwrap() += 1;
        }
    }

    /// One scripted answer: `exit` (`None`: runs for ever, `Err`: can't
    /// start), and for the elevated runner the result lines it writes.
    type Answer = (Result<Option<Option<i32>>, io::ErrorKind>, &'static str);

    struct Script {
        answers: Mutex<Vec<Answer>>,
        spawned: Mutex<Vec<InstallerCommand>>,
        killed: Arc<Mutex<usize>>,
    }

    impl Script {
        fn new(answers: Vec<Answer>) -> Arc<Script> {
            Arc::new(Script {
                answers: Mutex::new(answers),
                spawned: Mutex::new(Vec::new()),
                killed: Arc::new(Mutex::new(0)),
            })
        }
    }

    impl InstallerSpawner for Script {
        fn spawn(&self, cmd: &InstallerCommand) -> io::Result<Box<dyn InstallerProcess>> {
            self.spawned.lock().unwrap().push(cmd.clone());
            let (exit, lines) = self.answers.lock().unwrap().remove(0);
            let exit = exit.map_err(io::Error::from)?;
            // The elevated runner writes its result file beside its script.
            let write = cmd.argv.last().and_then(|outer| {
                let start = outer.find("-File \"")? + 7;
                let end = start + outer[start..].find('"')?;
                let script = PathBuf::from(&outer[start..end]);
                Some((
                    script.with_file_name("grid-setup-results.txt"),
                    lines.to_string(),
                ))
            });
            Ok(Box::new(Proc {
                polls: if exit.is_some() { 2 } else { usize::MAX },
                exit,
                write,
                killed: self.killed.clone(),
            }))
        }
    }

    #[derive(Default)]
    struct Sink {
        phases: Vec<JobPhase>,
        progress: Vec<(u64, u64)>,
        cancel_after_polls: Option<Cell<usize>>,
    }

    impl StoreProgressSink for Sink {
        fn progress(&mut self, done: u64, total: u64) {
            self.progress.push((done, total));
        }
        fn phase(&mut self, phase: JobPhase) {
            self.phases.push(phase);
        }
        fn cancelled(&self) -> bool {
            match &self.cancel_after_polls {
                Some(left) if left.get() == 0 => true,
                Some(left) => {
                    left.set(left.get() - 1);
                    false
                }
                None => false,
            }
        }
    }

    fn runner(script: &Arc<Script>, host: &str) -> RedistRunner {
        RedistRunner {
            spawner: script.clone(),
            which: Arc::new(|name: &str| {
                (name == "umu-run").then(|| PathBuf::from("/usr/bin/umu-run"))
            }),
            host: host.to_string(),
            watch: Watch {
                stall: Duration::from_secs(60),
                poll: Duration::from_millis(2),
            },
            timeout: Duration::from_secs(60),
            probe: Arc::new(HostProbe),
            system_root: None,
        }
    }

    /// Answers registry reads from a table; files are real.
    struct Reg(Vec<(&'static str, &'static str, u32)>);

    impl SystemProbe for Reg {
        fn real_file(&self, path: &Path) -> bool {
            HostProbe.real_file(path)
        }
        fn reg_dword(&self, key: &str, name: &str) -> Option<u32> {
            self.0
                .iter()
                .find(|(k, n, _)| *k == key && *n == name)
                .map(|(_, _, v)| *v)
        }
    }

    fn id_step(id: &str, command: &str) -> RedistStep {
        RedistStep {
            id: id.into(),
            ..step(command, &[])
        }
    }

    #[test]
    fn the_detection_table_knows_ids_and_file_names() {
        let vc = |year, x64| Some(Redist::VcRedist { year, x64 });
        for (id, want) in [
            ("openAL", Some(Redist::OpenAl)),
            ("DirectX", Some(Redist::DirectXJun2010)),
            ("MSVC2015_x64", vc(2015, true)),
            ("MSVC2015", vc(2015, false)),
            ("MSVC2019_x86", vc(2015, false)),
            ("MSVC2022_x64", vc(2015, true)),
            ("MSVC2013_x64", vc(2013, true)),
            ("MSVC2010", vc(2010, false)),
            ("dotNet48", Some(Redist::DotNet48)),
            ("MSVC2005", None),
            ("physx", None),
        ] {
            assert_eq!(redist_of(&id_step(id, "setup.exe")), want, "{id}");
        }
        for (command, want) in [
            ("oalinst.exe", Some(Redist::OpenAl)),
            ("redist\\OALINST.EXE", Some(Redist::OpenAl)),
            ("DirectX\\DXSETUP.exe", Some(Redist::DirectXJun2010)),
            ("vcredist_x64.exe", vc(2015, true)),
            ("VC_redist.x86.exe", vc(2015, false)),
            (
                "_CommonRedist\\vcredist\\2013\\vcredist_x64.exe",
                vc(2013, true),
            ),
            ("redist/2010/vcredist_x86.exe", vc(2010, false)),
            ("ndp48-x86-x64-allos-enu.exe", Some(Redist::DotNet48)),
            (
                "Engine\\Extras\\Redist\\en-us\\UE4PrereqSetup_x64.exe",
                Some(Redist::UePrereqs { x64: true }),
            ),
            (
                "Engine/Extras/Redist/en-us/UEPrereqSetup_x64.exe",
                Some(Redist::UePrereqs { x64: true }),
            ),
            (
                "UE4PrereqSetup_x86.exe",
                Some(Redist::UePrereqs { x64: false }),
            ),
            ("EasyAntiCheat_EOS_Setup.exe", None),
        ] {
            assert_eq!(redist_of(&step(command, &[])), want, "{command}");
        }
        // An unknown id falls back to the file name.
        assert_eq!(
            redist_of(&id_step("thing", "oalinst.exe")),
            Some(Redist::OpenAl)
        );
        assert_eq!(
            Redist::VcRedist {
                year: 2015,
                x64: true
            }
            .key(),
            "vcredist-2015-x64"
        );
    }

    fn sys(prefix: &Path, sub: &str, files: &[&str], stub: bool) {
        let dir = prefix.join("drive_c/windows").join(sub);
        fs::create_dir_all(&dir).unwrap();
        for file in files {
            let body: &[u8] = if stub {
                b"MZ\x90\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0Wine builtin DLL\0"
            } else {
                b"MZ real redistributable"
            };
            fs::write(dir.join(file), body).unwrap();
        }
    }

    #[test]
    fn a_prefix_has_a_redistributable_only_with_its_real_files_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        let probe = HostProbe;
        let have = |r| already_installed(r, Some(&prefix), None, &probe);
        assert!(!have(Redist::OpenAl), "no prefix yet");
        // Wine's builtin stubs don't count.
        sys(
            &prefix,
            "system32",
            &[
                "d3dx9_43.dll",
                "XAudio2_7.dll",
                "d3dcompiler_43.dll",
                "vcruntime140.dll",
                "msvcp140.dll",
            ],
            true,
        );
        sys(
            &prefix,
            "syswow64",
            &["d3dx9_43.dll", "XAudio2_7.dll", "d3dcompiler_43.dll"],
            true,
        );
        assert!(!have(Redist::DirectXJun2010));
        assert!(!have(Redist::VcRedist {
            year: 2015,
            x64: true
        }));
        // OpenAL in one folder only: not yet.
        sys(&prefix, "system32", &["OpenAL32.dll"], false);
        assert!(!have(Redist::OpenAl));
        sys(&prefix, "syswow64", &["OpenAL32.dll"], false);
        assert!(have(Redist::OpenAl));
        // 32-bit VC++ is the syswow64 folder's.
        sys(
            &prefix,
            "syswow64",
            &["vcruntime140.dll", "msvcp140.dll"],
            false,
        );
        assert!(have(Redist::VcRedist {
            year: 2015,
            x64: false
        }));
        assert!(!have(Redist::VcRedist {
            year: 2015,
            x64: true
        }));
        assert!(!have(Redist::DotNet48), "no file says so");
        // Unreal's bundle is there only with every part: 32-bit VC++ is,
        // 64-bit VC++ and DirectX aren't (stubs).
        assert!(!have(Redist::UePrereqs { x64: false }));
        sys(
            &prefix,
            "system32",
            &["d3dx9_43.dll", "XAudio2_7.dll", "d3dcompiler_43.dll"],
            false,
        );
        sys(
            &prefix,
            "syswow64",
            &["d3dx9_43.dll", "XAudio2_7.dll", "d3dcompiler_43.dll"],
            false,
        );
        assert!(have(Redist::UePrereqs { x64: false }));
        assert!(!have(Redist::UePrereqs { x64: true }), "no 64-bit VC++");
        sys(
            &prefix,
            "system32",
            &["vcruntime140.dll", "msvcp140.dll"],
            false,
        );
        assert!(have(Redist::UePrereqs { x64: true }));
        assert_eq!(Redist::UePrereqs { x64: true }.key(), "ue-prereqs-x64");
        assert!(is_wine_stub(b"MZ....Wine placeholder DLL"));
        assert!(!is_wine_stub(b"MZ real"));
    }

    #[test]
    fn a_windows_host_reads_the_registry_then_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Windows");
        fs::create_dir_all(root.join("System32")).unwrap();
        fs::create_dir_all(root.join("SysWOW64")).unwrap();
        let probe = Reg(vec![
            (
                r"HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64",
                "Installed",
                1,
            ),
            (
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\VisualStudio\14.0\VC\Runtimes\x86",
                "Installed",
                0,
            ),
            (
                r"HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full",
                "Release",
                461_808,
            ),
        ]);
        let have = |r| already_installed(r, None, Some(&root), &probe);
        assert!(have(Redist::VcRedist {
            year: 2015,
            x64: true
        }));
        assert!(
            !have(Redist::VcRedist {
                year: 2015,
                x64: false
            }),
            "Installed = 0"
        );
        assert!(!have(Redist::DotNet48), "4.7.2 is older");
        assert!(!have(Redist::OpenAl));
        for sub in ["System32", "SysWOW64"] {
            fs::write(root.join(sub).join("OpenAL32.dll"), b"MZ").unwrap();
        }
        assert!(have(Redist::OpenAl));
        assert_eq!(
            parse_reg_dword(
                "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\x\r\n    Installed    REG_DWORD    0x1\r\n\r\n",
                "Installed"
            ),
            Some(1)
        );
        assert_eq!(
            parse_reg_dword("    Release    REG_DWORD    0x80ea8", "release"),
            Some(528_040)
        );
        assert_eq!(
            parse_reg_dword("    Version    REG_SZ    v14", "Version"),
            None
        );
    }

    #[test]
    fn installed_or_already_run_steps_are_skipped_without_a_prompt_or_a_run() {
        let (_dir, game) = game();
        let prefix = game.join("prefix");
        // OpenAL is in the prefix already; DXSETUP isn't.
        sys(&prefix, "system32", &["OpenAL32.dll"], false);
        sys(&prefix, "syswow64", &["OpenAL32.dll"], false);
        fs::write(game.join("redist.exe"), b"MZ unknown").unwrap();
        let steps = vec![
            step("oalinst.exe", &["/silent"]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
            step("redist.exe", &[]),
        ];
        let script = Script::new(vec![(Ok(Some(Some(0))), ""), (Ok(Some(Some(0))), "")]);
        let outcome = run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut Sink::default(),
        );
        assert!(outcome.all_worked, "{outcome:?}");
        let spawned: Vec<String> = script
            .spawned
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.argv[1].clone())
            .collect();
        assert_eq!(spawned.len(), 2, "{spawned:?}");
        assert!(spawned[0].ends_with("DXSETUP.exe"));
        assert!(
            spawned[1].ends_with("redist.exe"),
            "a step with no rule runs"
        );
        let marker = read_marker(&prefix);
        assert_eq!(marker.len(), 2);
        assert_eq!(marker[0].key, "directx-jun2010");
        assert_eq!(marker[1].key, "redist.exe");

        // Again: everything is satisfied, nothing runs, no phase shows.
        let script = Script::new(vec![]);
        let mut sink = Sink::default();
        let outcome = run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut sink,
        );
        assert!(outcome.all_worked);
        assert!(script.spawned.lock().unwrap().is_empty());
        assert!(sink.phases.is_empty());

        // A new program (another size) for the same step runs again.
        fs::write(game.join("redist.exe"), b"MZ unknown, version 2").unwrap();
        let script = Script::new(vec![(Ok(Some(Some(0))), "")]);
        run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut Sink::default(),
        );
        assert_eq!(script.spawned.lock().unwrap().len(), 1);
        assert_eq!(read_marker(&prefix).len(), 2, "replaced, not added");

        // On Windows: everything installed means no elevated runner.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Windows");
        for sub in ["System32", "SysWOW64"] {
            fs::create_dir_all(root.join(sub)).unwrap();
            for file in [
                "OpenAL32.dll",
                "d3dx9_43.dll",
                "XAudio2_7.dll",
                "d3dcompiler_43.dll",
            ] {
                fs::write(root.join(sub).join(file), b"MZ").unwrap();
            }
        }
        let script = Script::new(vec![]);
        let mut win = runner(&script, "windows");
        win.system_root = Some(root);
        let target = RedistTarget {
            game_dir: &game,
            compat: "",
            prefix: None,
        };
        let outcome = run_steps(&steps[..2], target, &win, &mut Sink::default());
        assert!(outcome.all_worked);
        assert!(script.spawned.lock().unwrap().is_empty(), "no prompt");
    }

    /// A game folder inside a parent the runner's work folder can go in.
    fn game() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Game");
        fs::create_dir_all(game.join("DirectX")).unwrap();
        fs::write(game.join("oalinst.exe"), b"MZ").unwrap();
        fs::write(game.join("DirectX/DXSETUP.exe"), b"MZ").unwrap();
        (dir, game)
    }

    fn wrapped<'a>(game: &'a Path, prefix: &'a Path) -> RedistTarget<'a> {
        RedistTarget {
            game_dir: game,
            compat: "GE-Proton9-20",
            prefix: Some(prefix),
        }
    }

    #[test]
    fn every_step_runs_in_order_and_failures_are_warnings() {
        let (_dir, game) = game();
        let prefix = game.join("prefix");
        let steps = vec![
            step("oalinst.exe", &["/silent"]),
            step("redist\\vc.exe", &["/quiet"]),
            step("..\\up.exe", &[]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
        ];
        let script = Script::new(vec![(Ok(Some(Some(0))), ""), (Ok(Some(Some(7))), "")]);
        let mut sink = Sink::default();
        let outcome = run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut sink,
        );
        assert_eq!(
            outcome.warnings,
            vec![
                "Couldn't run vc.exe: it isn't in the game folder",
                "Couldn't run up.exe: its path leads outside the game folder",
                "Couldn't run DXSETUP.exe: it exited with code 7",
            ]
        );
        assert!(!outcome.all_worked);
        assert!(!outcome.cancelled);
        assert_eq!(outcome.warning().lines().count(), 3);
        let spawned = script.spawned.lock().unwrap();
        assert_eq!(spawned.len(), 2);
        assert!(spawned[0].argv[1].ends_with("oalinst.exe"));
        assert_eq!(spawned[0].argv[2], "/silent");
        assert!(spawned[1].argv[1].ends_with("DXSETUP.exe"));
        assert!(spawned.iter().all(|cmd| cmd.cwd == game));
        assert_eq!(sink.phases, vec![JobPhase::InstallerSilent]);
        assert_eq!(sink.progress.first(), Some(&(0, 4)));
        assert_eq!(sink.progress.last(), Some(&(4, 4)));
    }

    #[test]
    fn a_step_marked_in_exe_folder_starts_in_its_programs_folder() {
        let (_dir, game) = game();
        let prefix = game.join("prefix");
        let mut own = step("DirectX\\DXSETUP.exe", &["/silent"]);
        own.in_exe_folder = true;
        let steps = vec![step("oalinst.exe", &["/silent"]), own.clone()];
        let script = Script::new(vec![(Ok(Some(Some(0))), ""), (Ok(Some(Some(0))), "")]);
        let outcome = run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut Sink::default(),
        );
        assert!(outcome.all_worked, "{outcome:?}");
        let spawned = script.spawned.lock().unwrap();
        assert_eq!(spawned[0].cwd, game);
        assert_eq!(spawned[1].cwd, game.join("DirectX"));
        assert_eq!(
            step_dir(&own, Path::new("x.exe"), &game),
            game,
            "no folder of its own"
        );
        // Only a marked step keeps the flag in a saved list.
        let json = serde_json::to_string(&steps).unwrap();
        assert_eq!(json.matches("in_exe_folder").count(), 1, "{json}");
        let back: Vec<RedistStep> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, steps);
    }

    #[test]
    fn working_steps_count_as_run_and_no_compat_tool_is_a_warning() {
        let (_dir, game) = game();
        let prefix = game.join("prefix");
        let steps = vec![step("oalinst.exe", &["/silent"])];
        let script = Script::new(vec![(Ok(Some(Some(3010))), "")]);
        let outcome = run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut Sink::default(),
        );
        assert!(outcome.all_worked, "{outcome:?}");
        assert_eq!(outcome.warning(), "");

        // (A fresh prefix: the first one notes that oalinst.exe ran.)
        let script = Script::new(vec![]);
        let other = game.join("prefix2");
        let target = RedistTarget {
            compat: "",
            ..wrapped(&game, &other)
        };
        let outcome = run_steps(
            &steps,
            target,
            &runner(&script, "linux"),
            &mut Sink::default(),
        );
        assert_eq!(
            outcome.warnings,
            vec!["Couldn't run oalinst.exe: choose a compatibility tool (Wine or Proton) to run a Windows installer."]
        );
        assert!(script.spawned.lock().unwrap().is_empty());
        assert!(run_steps(&[], target, &runner(&script, "linux"), &mut Sink::default()).all_worked);
    }

    #[test]
    fn a_step_that_runs_too_long_or_stalls_is_stopped_and_the_next_still_runs() {
        let (_dir, game) = game();
        let prefix = game.join("prefix");
        let steps = vec![
            step("oalinst.exe", &["/silent"]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
        ];
        let script = Script::new(vec![(Ok(None), ""), (Ok(Some(Some(0))), "")]);
        let mut fast = runner(&script, "linux");
        fast.timeout = Duration::from_millis(40);
        let outcome = run_steps(&steps, wrapped(&game, &prefix), &fast, &mut Sink::default());
        assert_eq!(
            outcome.warnings,
            vec!["Couldn't run oalinst.exe: it didn't finish in 1 minute, so GRID stopped it"]
        );
        assert_eq!(*script.killed.lock().unwrap(), 1);
        assert_eq!(script.spawned.lock().unwrap().len(), 2);

        let script = Script::new(vec![(Ok(None), "")]);
        let mut stall = runner(&script, "linux");
        stall.watch.stall = Duration::from_millis(30);
        let outcome = run_steps(
            &steps[..1],
            wrapped(&game, &prefix),
            &stall,
            &mut Sink::default(),
        );
        assert_eq!(
            outcome.warnings,
            vec![
                "Couldn't run oalinst.exe: it showed no progress for 1 minute, so GRID stopped it"
            ]
        );
    }

    #[test]
    fn a_cancel_stops_the_running_step_and_skips_the_rest() {
        let (_dir, game) = game();
        let prefix = game.join("prefix");
        let steps = vec![
            step("oalinst.exe", &["/silent"]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
        ];
        let script = Script::new(vec![(Ok(None), ""), (Ok(Some(Some(0))), "")]);
        let mut sink = Sink {
            cancel_after_polls: Some(Cell::new(5)),
            ..Default::default()
        };
        let outcome = run_steps(
            &steps,
            wrapped(&game, &prefix),
            &runner(&script, "linux"),
            &mut sink,
        );
        assert!(outcome.cancelled);
        assert!(!outcome.all_worked);
        assert_eq!(outcome.warnings, vec![REDIST_CANCELLED]);
        assert_eq!(*script.killed.lock().unwrap(), 1);
        assert_eq!(
            script.spawned.lock().unwrap().len(),
            1,
            "DXSETUP never started"
        );
    }

    #[test]
    fn on_windows_one_prompt_runs_them_all_and_its_result_file_decides() {
        let (_dir, game) = game();
        let target = RedistTarget {
            game_dir: &game,
            compat: "",
            prefix: None,
        };
        let steps = vec![
            step("oalinst.exe", &["/silent"]),
            step("missing.exe", &[]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
        ];
        // Both runnable steps worked (indexes are the runner's: 0 and 1).
        let script = Script::new(vec![(Ok(Some(Some(0))), "0 0\n1 3010\n")]);
        let outcome = run_steps(
            &steps,
            target,
            &runner(&script, "windows"),
            &mut Sink::default(),
        );
        assert_eq!(
            outcome.warnings,
            vec!["Couldn't run missing.exe: it isn't in the game folder"]
        );
        let spawned = script.spawned.lock().unwrap().clone();
        assert_eq!(spawned.len(), 1, "one elevated runner for every step");
        assert_eq!(spawned[0].argv[0], "powershell.exe");
        assert!(
            !game.parent().unwrap().read_dir().unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".grid-setup-")),
            "the runner's folder is removed"
        );

        let steps = vec![
            step("oalinst.exe", &["/silent"]),
            step("DirectX\\DXSETUP.exe", &["/silent"]),
        ];
        let script = Script::new(vec![(Ok(Some(Some(0))), "0 0\n1 failed\n")]);
        let outcome = run_steps(
            &steps,
            target,
            &runner(&script, "windows"),
            &mut Sink::default(),
        );
        assert_eq!(
            outcome.warnings,
            vec!["Couldn't run DXSETUP.exe: Windows couldn't start it"]
        );

        let script = Script::new(vec![(Ok(Some(Some(0))), "0 0\n1 1\n")]);
        let outcome = run_steps(
            &steps,
            target,
            &runner(&script, "windows"),
            &mut Sink::default(),
        );
        assert_eq!(
            outcome.warnings,
            vec!["Couldn't run DXSETUP.exe: it exited with code 1"]
        );

        // A declined prompt: one warning, nothing counts as run.
        let script = Script::new(vec![(Ok(Some(Some(DECLINED_EXIT))), "")]);
        let outcome = run_steps(
            &steps,
            target,
            &runner(&script, "windows"),
            &mut Sink::default(),
        );
        assert!(outcome.declined);
        assert!(!outcome.all_worked);
        assert_eq!(outcome.warnings, vec![REDIST_DECLINED]);

        // The runner ran out of time after the first step.
        let script = Script::new(vec![(Ok(None), "")]);
        let mut fast = runner(&script, "windows");
        fast.timeout = Duration::from_millis(20);
        let outcome = run_steps(&steps, target, &fast, &mut Sink::default());
        assert_eq!(outcome.warnings.len(), 2);
        assert!(
            outcome.warnings[1].ends_with("so GRID stopped waiting"),
            "{outcome:?}"
        );

        // A cancel while the runner waits.
        let script = Script::new(vec![(Ok(None), "")]);
        let mut sink = Sink {
            cancel_after_polls: Some(Cell::new(3)),
            ..Default::default()
        };
        let outcome = run_steps(&steps, target, &runner(&script, "windows"), &mut sink);
        assert!(outcome.cancelled);
        assert_eq!(outcome.warnings, vec![REDIST_CANCELLED]);
    }
}
