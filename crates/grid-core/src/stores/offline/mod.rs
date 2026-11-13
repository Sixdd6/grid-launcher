//! GOG offline installers: a Linux `.sh` (read as a zip; `data/noarch/` is
//! copied and the script never runs), a Windows `setup_*.exe` (silent under
//! Wine/umu, native on Windows, the wizard only when allowed) and a macOS
//! `.pkg` (refused). Shared by the offline-installer flow, RomM archives
//! that hold an installer (`library::specials::gog_installer`), and drive
//! restores.
//!
//! - [`inspect`] reads a file and decides what installing it would do
//!   ([`Verdict`]), with the GameStores card's refusals.
//! - [`run_into`] unpacks or runs an inspected installer into a folder.
//! - [`install_job`] is the Downloads job (`StoreOp::Installer`) that
//!   installs one and writes its GOG row.
//! - [`detect_in_dir`] finds an installer in an extracted RomM archive.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::gog::goggame::{find_info_files, GogGameInfo};
use super::gog::meta::{GogMeta, GogPlatform};
use super::gog::GogJobCtx;
use super::{GameKey, StoreError, StoreKind};
use crate::launch::template::host_os;
use crate::library::queue::{JobPhase, StoreJob, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;

pub mod exe;
pub mod exec;
mod job;
pub mod sh;

pub use exec::{InstallerCommand, InstallerProcess, InstallerSpawner, SystemSpawner, Watch};
pub use job::GOG_FOLDER;
pub(crate) use job::{linux_launch, remove_install};

/// What kind of installer a file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallerKind {
    LinuxSh,
    /// `setup_*.exe` and its `.bin` parts, in order.
    WindowsExe {
        parts: Vec<PathBuf>,
    },
    MacPkg,
}

impl InstallerKind {
    /// The GOG version this installer holds; `None` for a `.pkg`.
    pub fn platform(&self) -> Option<GogPlatform> {
        match self {
            InstallerKind::LinuxSh => Some(GogPlatform::Linux),
            InstallerKind::WindowsExe { .. } => Some(GogPlatform::Windows),
            InstallerKind::MacPkg => None,
        }
    }
}

/// What GRID found in an installer file before running it.
#[derive(Debug, Clone, PartialEq)]
pub struct Inspection {
    pub path: PathBuf,
    pub kind: InstallerKind,
    /// `None` when the identity is only known after the run.
    pub info: Option<GogGameInfo>,
    pub version: String,
    /// All parts together.
    pub bytes: u64,
    pub verdict: Verdict,
}

/// What GRID will do with an inspected installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Install,
    Update { installed_version: String },
    Dlc { base_title: String },
    Refused { title: String, body: String },
}

/// The user's choices for an offline install.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OfflineOpts {
    /// `None`: `<GOG install dir>/<name>`.
    pub install_dir: Option<PathBuf>,
    pub compat_tool: String,
}

impl Inspection {
    /// Every installer file: the `.sh`, or the `.exe` and its parts.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.path.clone()];
        if let InstallerKind::WindowsExe { parts } = &self.kind {
            files.extend(parts.iter().cloned());
        }
        files
    }

    /// The installer's file name.
    pub fn file_name(&self) -> String {
        file_name(&self.path)
    }

    /// The title the job and the dialog show: the game info's name, else
    /// one guessed from the file name.
    pub fn title(&self) -> String {
        self.info
            .as_ref()
            .map(|info| info.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| exe::guess_from_name(&self.file_name()).0)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Why [`read_installer`] refused a file before looking at the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    MacPkg,
    NotGog,
    MissingPart,
    /// A Linux installer on a host that can't run Linux games.
    WrongHost,
}

// --- the design's refusal words (GameStores card, "Refusals") ---------------

/// A refusal as one line ("<title>. <body>"), for a failed Downloads row;
/// `""` for any other verdict.
pub fn refusal_message(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Refused { title, body } => format!("{title}. {body}"),
        _ => String::new(),
    }
}

fn refused(title: impl Into<String>, body: impl Into<String>) -> Verdict {
    Verdict::Refused {
        title: title.into(),
        body: body.into(),
    }
}

const MAC_REFUSAL_TITLE: &str = "This is a macOS installer";

fn mac_refusal() -> Verdict {
    refused(
        MAC_REFUSAL_TITLE,
        "GRID installs GOG's Linux (.sh) and Windows (.exe) installers. Download one of those from your GOG library.",
    )
}

fn not_gog_refusal(file: &str) -> Verdict {
    refused(
        "This isn't a GOG installer",
        format!(
            "{file} has no GOG game info in it. GRID reads GOG's own setup_….exe and ….sh installers."
        ),
    )
}

fn missing_part_refusal(parts: u32, missing: &str) -> Verdict {
    refused(
        "Part of this installer is missing",
        format!(
            "This installer comes in {parts} parts. {missing} isn't in the same folder. Download every part from your GOG library and put them beside the .exe."
        ),
    )
}

fn wrong_host_refusal() -> Verdict {
    refused(
        "This is a Linux installer",
        "GRID on Windows installs GOG's Windows (.exe) installers. Download that one from your GOG library.",
    )
}

fn base_missing_refusal(base: &str) -> Verdict {
    refused(
        format!("This is DLC for {base}, which isn't installed"),
        format!("Install {base} first, from GOG or from its own installer. Then install this DLC."),
    )
}

fn version_name(platform: GogPlatform) -> &'static str {
    match platform {
        GogPlatform::Linux => "Linux",
        GogPlatform::Windows => "Windows",
    }
}

fn other_os_refusal(title: &str, installed: GogPlatform, offered: GogPlatform) -> Verdict {
    refused(
        format!(
            "{title} is installed here as the {} version",
            version_name(installed)
        ),
        format!(
            "This is the {} version's installer. To switch, uninstall {title} first, then open this installer again.",
            version_name(offered)
        ),
    )
}

fn other_os_dlc_refusal(base: &str, installed: GogPlatform, offered: GogPlatform) -> Verdict {
    refused(
        format!(
            "{base} is installed here as the {} version",
            version_name(installed)
        ),
        format!(
            "This is DLC for the {} version. A {} game takes {} DLC installers only.",
            version_name(offered),
            version_name(installed),
            version_name(installed)
        ),
    )
}

/// The base game's title when it isn't installed: the part of the DLC's
/// name before " - " (GOG names DLC "<Game> - <DLC>"), else "its base
/// game".
pub fn base_title_guess(dlc_name: &str) -> String {
    dlc_name
        .split_once(" - ")
        .map(|(base, _)| base.trim().to_string())
        .filter(|base| !base.is_empty())
        .unwrap_or_else(|| "its base game".to_string())
}

// --- reading ------------------------------------------------------------------

/// Reads `path` without the registry: its kind, identity, version and size,
/// and the refusals that need no registry (macOS, not GOG's, a missing
/// part, a Linux installer off Linux). The verdict is `Install` or that
/// refusal.
pub(crate) fn read_installer(path: &Path) -> Result<(Inspection, Option<Refusal>), StoreError> {
    let meta = fs::metadata(path).map_err(LibraryError::Io)?;
    let name = file_name(path);
    let lower = name.to_lowercase();
    let mut insp = Inspection {
        path: path.to_path_buf(),
        kind: InstallerKind::LinuxSh,
        info: None,
        version: String::new(),
        bytes: meta.len(),
        verdict: Verdict::Install,
    };
    let refuse = |mut insp: Inspection, verdict: Verdict, why: Refusal| {
        insp.verdict = verdict;
        Ok((insp, Some(why)))
    };
    if lower.ends_with(".pkg") || lower.ends_with(".dmg") {
        insp.kind = InstallerKind::MacPkg;
        return refuse(insp, mac_refusal(), Refusal::MacPkg);
    }
    if lower.ends_with(".exe") {
        insp.kind = InstallerKind::WindowsExe { parts: Vec::new() };
        let read = exe::read(path);
        let identity = match read {
            exe::ExeRead::NotGog => return refuse(insp, not_gog_refusal(&name), Refusal::NotGog),
            exe::ExeRead::Inno(identity) => *identity,
            exe::ExeRead::Unknown => exe::ExeIdentity::default(),
        };
        let (parts, missing) = exe::bin_parts(path, identity.bin_parts);
        if let Some(missing) = missing {
            let count = parts.len() as u32 + 2;
            let count = count.max(identity.bin_parts + 1);
            return refuse(
                insp,
                missing_part_refusal(count, &missing),
                Refusal::MissingPart,
            );
        }
        insp.bytes = parts
            .iter()
            .filter_map(|part| fs::metadata(part).ok())
            .fold(insp.bytes, |sum, meta| sum.saturating_add(meta.len()));
        insp.kind = InstallerKind::WindowsExe { parts };
        insp.version = if identity.version.is_empty() {
            exe::guess_from_name(&name).1
        } else {
            identity.version
        };
        insp.info = identity.info;
        return Ok((insp, None));
    }
    // Anything else must be a Linux installer.
    let Some(contents) = sh::read(path) else {
        return refuse(insp, not_gog_refusal(&name), Refusal::NotGog);
    };
    let Some(info) = contents.info else {
        return refuse(insp, not_gog_refusal(&name), Refusal::NotGog);
    };
    insp.info = Some(info);
    insp.version = if contents.version.is_empty() {
        exe::guess_from_name(&name).1
    } else {
        contents.version
    };
    if !cfg!(target_os = "linux") {
        return refuse(insp, wrong_host_refusal(), Refusal::WrongHost);
    }
    Ok((insp, None))
}

/// The installed GOG row for `id`, if any.
fn gog_row_for(registry: &Registry, id: &str) -> Result<Option<InstalledGame>, LibraryError> {
    if id.trim().is_empty() {
        return Ok(None);
    }
    registry.find_key(&GameKey::store(StoreKind::Gog, id))
}

/// The GOG version a row has installed.
pub fn row_platform(row: &InstalledGame) -> GogPlatform {
    if row.is_store() {
        return GogMeta::from_json(&row.store_meta).platform;
    }
    if row.is_windows() {
        GogPlatform::Windows
    } else {
        GogPlatform::Linux
    }
}

/// What installing `insp` would do now, against the installed GOG games.
pub(crate) fn decide(insp: &Inspection, registry: &Registry) -> Result<Verdict, LibraryError> {
    if let Verdict::Refused { .. } = insp.verdict {
        return Ok(insp.verdict.clone());
    }
    let Some(offered) = insp.kind.platform() else {
        return Ok(mac_refusal());
    };
    let Some(info) = &insp.info else {
        // Identity only after the run: a fresh install.
        return Ok(Verdict::Install);
    };
    if info.is_dlc() {
        let Some(base) = gog_row_for(registry, &info.root_game_id)? else {
            return Ok(base_missing_refusal(&base_title_guess(&info.name)));
        };
        let installed = row_platform(&base);
        if installed != offered {
            return Ok(other_os_dlc_refusal(&base.title, installed, offered));
        }
        return Ok(Verdict::Dlc {
            base_title: base.title,
        });
    }
    if let Some(row) = gog_row_for(registry, &info.game_id)? {
        let installed = row_platform(&row);
        if installed != offered {
            return Ok(other_os_refusal(&row.title, installed, offered));
        }
        return Ok(Verdict::Update {
            installed_version: GogMeta::from_json(&row.store_meta).version,
        });
    }
    Ok(Verdict::Install)
}

/// Reads `path` and decides what installing it would do.
///
/// A file GRID can't read at all is an error; everything the design
/// refuses (a macOS `.pkg`, a file with no GOG game info, a missing `.bin`
/// part, the other OS's installer of an installed game, a DLC whose base
/// game isn't installed or is the other OS's version) is
/// `Ok` with [`Verdict::Refused`] in the card's words. `tools_dir` is not
/// used (no external tool reads installers; spike 5).
pub fn inspect(
    path: &Path,
    registry: &Registry,
    tools_dir: &Path,
) -> Result<Inspection, StoreError> {
    let _ = tools_dir;
    let (mut insp, refusal) = read_installer(path)?;
    if refusal.is_none() {
        insp.verdict = decide(&insp, registry)?;
    }
    Ok(insp)
}

// --- RomM archives ------------------------------------------------------------

/// Files that may sit beside an installer in an archive without making it
/// "loose game files".
const EXTRA_SUFFIXES: [&str; 12] = [
    "txt", "pdf", "nfo", "md", "url", "html", "htm", "rtf", "jpg", "jpeg", "png", "json",
];

/// The installer inside an extracted RomM archive, `None` when it holds
/// loose game files.
///
/// Through a single wrapper folder, the archive must hold one installer —
/// a `setup*.exe` (with only its own `-N.bin` parts), or a `.sh` that is a
/// GOG Linux installer — and nothing else but notes and pictures (no
/// folders).
pub fn detect_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut dir = dir.to_path_buf();
    loop {
        let entries: Vec<_> = fs::read_dir(&dir).ok()?.flatten().collect();
        if entries.len() == 1 && entries[0].file_type().ok()?.is_dir() {
            dir = entries[0].path();
            continue;
        }
        if entries
            .iter()
            .any(|entry| entry.file_type().map_or(true, |t| t.is_dir()))
        {
            return None;
        }
        let names: Vec<String> = entries
            .iter()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        let lower = |name: &str| name.to_lowercase();
        let setups: Vec<&String> = names
            .iter()
            .filter(|name| {
                let name = lower(name);
                name.starts_with("setup") && name.ends_with(".exe")
            })
            .collect();
        let shells: Vec<&String> = names
            .iter()
            .filter(|name| lower(name).ends_with(".sh"))
            .collect();
        let installer = match (setups.as_slice(), shells.as_slice()) {
            ([setup], []) => {
                let lowered = lower(setup);
                let stem = lowered.strip_suffix(".exe").unwrap_or(&lowered);
                let others_ok = names.iter().filter(|name| name != setup).all(|name| {
                    let name = lower(name);
                    (name.starts_with(&format!("{stem}-")) && name.ends_with(".bin"))
                        || is_extra(&name)
                });
                others_ok.then(|| dir.join(setup))
            }
            ([], [shell]) => {
                let path = dir.join(shell);
                let others_ok = names
                    .iter()
                    .filter(|name| name != shell)
                    .all(|name| is_extra(&lower(name)));
                (others_ok && sh::read(&path).is_some()).then_some(path)
            }
            _ => None,
        };
        return installer;
    }
}

fn is_extra(lower_name: &str) -> bool {
    lower_name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| EXTRA_SUFFIXES.contains(&ext))
}

// --- running ------------------------------------------------------------------

/// Runs (or unpacks) the installer into `dest` and returns the installed
/// game's info. Shared by the RomM path and the drive restore.
///
/// A `.sh` is unpacked (phase `Unpacking`); a `.exe` runs silently
/// (`InstallerSilent`) under `compat` with `prefix` (natively on a Windows
/// host), and when that fails or stalls, with its setup window
/// (`InstallerWizard`) if `wizard_allowed`, else
/// [`LibraryError::InstallerNeedsWizard`] `{ kept: true }`. The returned
/// info is read from `dest` after the run (the inspected product's file
/// when `dest` holds several); none there is
/// [`LibraryError::InstallerUnrecognised`]. A cancel (`sink.cancelled()`)
/// kills the installer and its prefix's processes.
pub fn run_into(
    insp: &Inspection,
    dest: &Path,
    prefix: Option<&Path>,
    compat: &str,
    wizard_allowed: bool,
    sink: &mut dyn StoreProgressSink,
) -> Result<GogGameInfo, LibraryError> {
    run_into_with(
        insp,
        dest,
        prefix,
        compat,
        wizard_allowed,
        sink,
        &SystemSpawner,
        Watch::default(),
    )
}

/// [`run_into`] with the process spawner and the supervisor's timing given
/// (tests script the installer).
#[allow(clippy::too_many_arguments)]
pub fn run_into_with(
    insp: &Inspection,
    dest: &Path,
    prefix: Option<&Path>,
    compat: &str,
    wizard_allowed: bool,
    sink: &mut dyn StoreProgressSink,
    spawner: &dyn InstallerSpawner,
    watch: Watch,
) -> Result<GogGameInfo, LibraryError> {
    match &insp.kind {
        InstallerKind::MacPkg => Err(LibraryError::InstallerRefused(
            MAC_REFUSAL_TITLE.to_string(),
        )),
        InstallerKind::LinuxSh => {
            sink.phase(JobPhase::Unpacking);
            sh::unpack(&insp.path, dest, sink)?;
            installed_info(dest, insp).ok_or_else(|| unrecognised(insp))
        }
        InstallerKind::WindowsExe { .. } => {
            let lang = match exe::read(&insp.path) {
                exe::ExeRead::Inno(identity) => identity.lang,
                _ => String::new(),
            };
            exec::run_exe(
                &exec::ExeRun {
                    setup: &insp.path,
                    dest,
                    prefix,
                    compat,
                    lang: &lang,
                    wizard_allowed,
                    expected_bytes: insp.bytes,
                    spawner,
                    watch,
                },
                sink,
            )?;
            installed_info(dest, insp).ok_or_else(|| unrecognised(insp))
        }
    }
}

fn unrecognised(insp: &Inspection) -> LibraryError {
    LibraryError::InstallerUnrecognised {
        file: insp.file_name(),
    }
}

/// The goggame files of an install folder: at its top, and (Linux) in its
/// `game/` folder.
pub fn installed_infos(dir: &Path) -> Vec<(PathBuf, GogGameInfo)> {
    let mut found = Vec::new();
    for folder in [dir.to_path_buf(), dir.join("game")] {
        for file in find_info_files(&folder) {
            if let Some(info) = fs::read(&file)
                .ok()
                .and_then(|bytes| super::gog::goggame::parse_info(&bytes))
            {
                found.push((file, info));
            }
        }
    }
    found
}

/// The info of what `insp` installed into `dest`: the file with the
/// inspected product id, else the base game's, else the first; the
/// inspection's own info when `dest` has none and it has one (a `.sh`
/// whose identity came from `config.lua`).
fn installed_info(dest: &Path, insp: &Inspection) -> Option<GogGameInfo> {
    let infos: Vec<GogGameInfo> = installed_infos(dest).into_iter().map(|(_, i)| i).collect();
    if let Some(wanted) = insp.info.as_ref().map(|info| info.game_id.clone()) {
        if let Some(info) = infos.iter().find(|info| info.game_id == wanted) {
            return Some(info.clone());
        }
    }
    exe::pick_base(infos).or_else(|| {
        insp.info
            .clone()
            .filter(|_| matches!(insp.kind, InstallerKind::LinuxSh))
    })
}

/// Where a fresh Windows install's Wine prefix goes: `<install dir>/prefix`,
/// like every GOG game's (gogdl installs, drive restores), so uninstalling
/// the game removes it and a backup leaves it out
/// ([`crate::backup::store::STORE_PREFIX_DIR`]).
pub fn default_prefix(dest: &Path) -> PathBuf {
    dest.join(crate::backup::store::STORE_PREFIX_DIR)
}

/// Whether a Windows installer needs a Wine prefix on this host.
pub fn needs_prefix(kind: &InstallerKind) -> bool {
    matches!(kind, InstallerKind::WindowsExe { .. }) && !host_os().starts_with("win")
}

// --- the Downloads job ------------------------------------------------------------

/// The Downloads job (`StoreOp::Installer`) that installs `insp`.
///
/// Its download half only checks that the installer files are still
/// there; its finalize decides again against the registry, runs the
/// installer ([`run_into`]) into the game's folder — a fresh one, the
/// installed game's for an update, the base game's (and its Wine prefix)
/// for a DLC — and writes the GOG row ([`super::gog::row::gog_row`],
/// `origin: offline`, `meta.installer` the installer's absolute paths) or,
/// for a DLC, adds it to the base row's `meta.dlc`.
pub fn install_job(
    insp: Inspection,
    opts: OfflineOpts,
    ctx: GogJobCtx,
    wizard_allowed: Arc<dyn Fn() -> bool + Send + Sync>,
) -> StoreJob {
    install_job_with(
        insp,
        opts,
        ctx,
        wizard_allowed,
        exec::system_spawner(),
        Watch::default(),
    )
}

/// [`install_job`] with the process spawner and timing given (tests).
pub fn install_job_with(
    insp: Inspection,
    opts: OfflineOpts,
    ctx: GogJobCtx,
    wizard_allowed: Arc<dyn Fn() -> bool + Send + Sync>,
    spawner: Arc<dyn InstallerSpawner>,
    watch: Watch,
) -> StoreJob {
    let store_id = insp
        .info
        .as_ref()
        .map(|info| info.game_id.clone())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| insp.file_name());
    let title = insp.title();
    StoreJob {
        source: StoreKind::Gog,
        store_id,
        op: StoreOp::Installer,
        title,
        drive: None,
        runner: Arc::new(job::OfflineRunner {
            insp,
            opts,
            ctx,
            wizard_allowed,
            spawner,
            watch,
        }),
    }
}
