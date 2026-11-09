//! Backup planning: what a backup writes to a drive, what it skips and whether
//! it fits. [`plan`] is pure: the caller gathers the inputs (the drive's pack,
//! the requested games, the per-system emulator choice and the server's
//! firmware) and gets back both the [`BackupPlan`] the dialog shows and the
//! [`BackupItems`] `InstallService::start_backup` queues.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::pack::{Pack, PackOs, FAT32_MAX_FILE};
use super::write::SPACE_MARGIN;
use crate::config::Config;
use crate::launch::catalog;
use crate::launch::profiles::EmulatorProfile;
use crate::launch::selection::NO_EMULATOR;
use crate::launch::setup::recommendations;
use crate::library::platforms::{is_native_platform, is_windows_platform};

/// A game named in a plan without being written (already on the drive, or too
/// big for the drive's file system).
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PlanGameRef {
    pub rom_id: i64,
    pub name: String,
}

/// One emulator a backup carries, across every OS the pack is for.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PlanEmulator {
    pub name: String,
    pub os: Vec<PackOs>,
    /// `None` when the release size is not known before the download.
    pub bytes: Option<u64>,
    /// Already on the drive for every OS: nothing to write.
    pub already: bool,
    /// Why this emulator goes for fewer computers than the pack is for, or
    /// not at all ("PICO-8 is only for Linux", "PICO-8 can't be copied
    /// (installed by the system)"). Empty when there is nothing to say.
    #[serde(default)]
    pub note: String,
}

/// The firmware of one platform a backup carries.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PlanFirmware {
    pub platform_name: String,
    /// The number of firmware files.
    pub files: u32,
    pub bytes: u64,
    pub already: bool,
}

/// The answer to `plan_backup`: what the Backup dialog shows before it starts.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BackupPlan {
    /// The drive key: its mount path, or its drive letter on Windows.
    pub drive: String,
    pub games_to_write: Vec<i64>,
    pub skipped: Vec<PlanGameRef>,
    pub too_big: Vec<PlanGameRef>,
    pub emulators: Vec<PlanEmulator>,
    pub firmware: Vec<PlanFirmware>,
    /// Why the firmware checkbox is disabled, else empty.
    pub firmware_reason: String,
    pub bytes_to_write: u64,
    pub free_bytes: u64,
    pub fits: bool,
}

/// What `InstallService::start_backup` answers: the batch the rows share and
/// how many rows of each kind were queued.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct BackupQueued {
    pub batch: u64,
    pub games: u32,
    pub emulators: u32,
    pub firmware: u32,
}

// --- emulator choice ----------------------------------------------------------

/// Whether a backed-up emulator is an ordinary emulator or a compat tool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmulatorKind {
    Emulator,
    CompatTool,
}

impl EmulatorKind {
    /// The `kind` a [`super::pack::PackEmulator`] records.
    pub fn pack_kind(self) -> &'static str {
        match self {
            Self::Emulator => "emulator",
            Self::CompatTool => "compat_tool",
        }
    }
}

/// Where a backed-up emulator's bytes come from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EmulatorSource {
    /// A forge release of the catalog profile named [`EmulatorPick::name`],
    /// resolved for each OS. `source_id` is the catalog `owner/repo`.
    Catalog { source_id: String },
    /// The newest build of `romm:<folder>/<emulator>` on the server's
    /// Emulators platform, for each OS.
    Server { source_id: String },
    /// A manually added emulator with no download source: its program
    /// folder, zipped. Built for this computer's OS only.
    Local { program: PathBuf, args: String },
}

/// The emulator a backup carries for one system.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmulatorPick {
    /// The profile name (the entry name for a manual emulator).
    pub name: String,
    pub kind: EmulatorKind,
    pub source: EmulatorSource,
    /// The RomM platform slugs it serves in this backup.
    pub platforms: Vec<String>,
}

impl EmulatorPick {
    /// The `source_id` a [`super::pack::PackEmulator`] records.
    pub fn source_id(&self) -> String {
        match &self.source {
            EmulatorSource::Catalog { source_id } | EmulatorSource::Server { source_id } => {
                source_id.clone()
            }
            EmulatorSource::Local { .. } => format!("local:{}", self.name),
        }
    }

    /// The OSes this emulator can be backed up for, out of `pack_os`: a
    /// compat tool goes for Linux only, a manual emulator for this
    /// computer's OS only.
    pub fn target_os(&self, pack_os: &[PackOs], host: Option<PackOs>) -> Vec<PackOs> {
        pack_os
            .iter()
            .copied()
            .filter(|os| match (&self.source, self.kind) {
                (_, EmulatorKind::CompatTool) => *os == PackOs::Linux,
                (EmulatorSource::Local { .. }, _) => Some(*os) == host,
                _ => true,
            })
            .collect()
    }
}

/// What a backup carries for one system.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EmulatorChoice {
    /// No emulator: a native game, or a system with no default and no
    /// recommendation.
    None,
    Pick(EmulatorPick),
    /// An emulator that cannot be backed up, with the dialog's reason.
    Refused {
        name: String,
        reason: String,
    },
}

/// The emulator a backup carries for the system `label` (its display label,
/// which `Config.default_emulators` keys by) with RomM slug `slug`:
/// 1. the system's default emulator, as its installed `[[emulators]]` entry
///    records it: a catalog `owner/repo`, a server build `romm:…`, or no
///    source at all (a manual emulator, archived from its program folder);
/// 2. with no default, the first recommendation for the slug.
///
/// Native games (`platform_name`) need no emulator here; their compat tool is
/// [`choose_compat_tool`]'s.
pub fn choose_emulator(
    config: &Config,
    profiles: &[EmulatorProfile],
    label: &str,
    slug: &str,
    platform_name: &str,
) -> EmulatorChoice {
    if is_native_platform(slug, platform_name) {
        return EmulatorChoice::None;
    }
    let platforms = vec![slug.to_string()];
    let default = config
        .default_emulators
        .get(label)
        .map(|name| name.trim())
        .filter(|name| !name.is_empty());
    let Some(default) = default else {
        let recommended = recommendations()
            .recommended
            .get(slug)
            .and_then(|names| names.first());
        return match recommended.and_then(|name| profile_named(profiles, name, false)) {
            Some(profile) => catalog_pick(profile, EmulatorKind::Emulator, platforms),
            None => EmulatorChoice::None,
        };
    };
    if default == NO_EMULATOR {
        return EmulatorChoice::None;
    }
    let entry = config
        .emulators
        .iter()
        .find(|entry| entry.name.trim().eq_ignore_ascii_case(default));
    let Some(entry) = entry else {
        // A default whose install has not finished: the catalog profile of
        // that name, if there is one.
        return match profile_named(profiles, default, false) {
            Some(profile) => catalog_pick(profile, EmulatorKind::Emulator, platforms),
            None => EmulatorChoice::None,
        };
    };
    let source_id = entry.source_id.trim();
    if let Some(rest) = source_id.strip_prefix("romm:") {
        let emulator = rest.rsplit('/').next().unwrap_or(rest);
        let name = profile_named(profiles, emulator, false)
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| entry.name.clone());
        return EmulatorChoice::Pick(EmulatorPick {
            name,
            kind: EmulatorKind::Emulator,
            source: EmulatorSource::Server {
                source_id: source_id.to_lowercase(),
            },
            platforms,
        });
    }
    if !source_id.is_empty() {
        if let Some(profile) = catalog::find_profile(profiles, source_id) {
            return EmulatorChoice::Pick(EmulatorPick {
                name: profile.name.clone(),
                kind: EmulatorKind::Emulator,
                source: EmulatorSource::Catalog {
                    source_id: source_id.to_string(),
                },
                platforms,
            });
        }
    }
    // No source GRID can download again: archive the program folder.
    let program = crate::library::paths::expand_home(entry.path.trim());
    if entry.path.trim().is_empty() {
        return EmulatorChoice::None;
    }
    if let Some(reason) = archive_refusal(&entry.name, &program) {
        return EmulatorChoice::Refused {
            name: entry.name.clone(),
            reason,
        };
    }
    EmulatorChoice::Pick(EmulatorPick {
        name: entry.name.clone(),
        kind: EmulatorKind::Emulator,
        source: EmulatorSource::Local {
            program,
            args: entry.args.clone(),
        },
        platforms,
    })
}

/// The compat tool Windows games carry when the pack is for Linux:
/// `Config.default_compat_tool` (by its recorded install, else its catalog
/// profile), else GE-Proton. `None` when the catalog has neither.
pub fn choose_compat_tool(config: &Config, profiles: &[EmulatorProfile]) -> Option<EmulatorPick> {
    let wanted = config.default_compat_tool.trim();
    let wanted = if wanted.is_empty() {
        "GE-Proton"
    } else {
        wanted
    };
    let installed = config
        .compat_tool_installs
        .iter()
        .find(|install| install.name.trim().eq_ignore_ascii_case(wanted))
        .and_then(|install| catalog::find_compat_profile(profiles, install.source_id.trim()));
    let profile = installed
        .or_else(|| profile_named(profiles, wanted, true))
        .or_else(|| profile_named(profiles, "GE-Proton", true))?;
    match catalog_pick(profile, EmulatorKind::CompatTool, Vec::new()) {
        EmulatorChoice::Pick(pick) => Some(pick),
        _ => None,
    }
}

/// The umu-launcher a pack carries beside its compat tool: the release
/// `InstallService::install_umu_launcher` would fetch
/// ([`crate::launch::umu::default_source`]), for Linux only. Recorded with
/// kind `compat_tool` and `source_id` [`crate::launch::umu::UMU_SOURCE_ID`].
pub fn umu_pick() -> EmulatorPick {
    EmulatorPick {
        name: crate::launch::umu::UMU_TITLE.to_string(),
        kind: EmulatorKind::CompatTool,
        source: EmulatorSource::Catalog {
            source_id: crate::launch::umu::UMU_SOURCE_ID.to_string(),
        },
        platforms: Vec::new(),
    }
}

/// Whether a pack emulator entry is the umu-launcher [`umu_pick`] adds.
pub fn is_umu_entry(source_id: &str) -> bool {
    source_id
        .trim()
        .eq_ignore_ascii_case(crate::launch::umu::UMU_SOURCE_ID)
}

fn profile_named<'a>(
    profiles: &'a [EmulatorProfile],
    name: &str,
    compat: bool,
) -> Option<&'a EmulatorProfile> {
    let wanted = name.trim();
    profiles
        .iter()
        .filter(|profile| profile.is_compat_tool == compat)
        .find(|profile| {
            let full = profile.name.trim();
            let short = full.split(" (").next().unwrap_or(full).trim();
            full.eq_ignore_ascii_case(wanted) || short.eq_ignore_ascii_case(wanted)
        })
}

fn catalog_pick(
    profile: &EmulatorProfile,
    kind: EmulatorKind,
    platforms: Vec<String>,
) -> EmulatorChoice {
    let source = profile
        .source
        .as_ref()
        .and_then(|source| source.as_object());
    let field = |key: &str| {
        source
            .and_then(|source| source.get(key))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let (owner, repo) = (field("owner"), field("repo"));
    if owner.is_empty() || repo.is_empty() {
        return EmulatorChoice::None;
    }
    EmulatorChoice::Pick(EmulatorPick {
        name: profile.name.clone(),
        kind,
        source: EmulatorSource::Catalog {
            source_id: format!("{owner}/{repo}"),
        },
        platforms,
    })
}

// --- manual emulator archives ---------------------------------------------------

/// The largest program folder GRID archives for a manual emulator.
pub const LOCAL_ARCHIVE_MAX: u64 = 4 * 1024 * 1024 * 1024;

/// Whether `path` (a program's folder, as text) lies in a folder the system
/// owns: `/usr`, `/bin`, `/sbin`, `/lib*`, `/etc`, `/snap`, a root-owned
/// `/opt` (`opt_root_owned`), `C:\Windows` or `C:\Program Files*` (any
/// drive letter, any case). Pure, so both styles are tested on any OS.
pub fn is_system_path(path: &str, opt_root_owned: bool) -> bool {
    let unified = path.replace('\\', "/");
    let lower = unified.to_ascii_lowercase();
    let windows = lower.as_bytes();
    if windows.len() >= 2 && windows[0].is_ascii_alphabetic() && windows[1] == b':' {
        let rest = lower[2..].trim_start_matches('/');
        let first = rest.split('/').next().unwrap_or("");
        return first == "windows" || first.starts_with("program files");
    }
    let first = unified
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    if !unified.starts_with('/') {
        return false;
    }
    match first {
        "usr" | "bin" | "sbin" | "lib" | "lib32" | "lib64" | "etc" | "snap" => true,
        "opt" => opt_root_owned,
        _ => false,
    }
}

/// Why the manual emulator `name`, whose program is `program`, cannot be
/// archived for a backup, or `None` when it can: the program must exist; its
/// folder must not be a system folder ([`is_system_path`]), the home folder
/// or a drive root; and the folder must hold at most [`LOCAL_ARCHIVE_MAX`].
pub fn archive_refusal(name: &str, program: &Path) -> Option<String> {
    let system = || format!("{name} can't be copied (installed by the system)");
    let folder = program.parent().filter(|p| !p.as_os_str().is_empty());
    let text = program.to_string_lossy();
    if is_system_path(&text, opt_root_owned(program)) {
        return Some(system());
    }
    if !program.is_file() {
        return Some(format!(
            "{name} can't be copied (its program was not found)"
        ));
    }
    let folder = folder?;
    let home = crate::platform::home_dir();
    if folder.parent().is_none() || home.as_deref() == Some(folder) {
        return Some(format!(
            "{name} can't be copied (its program is not in a folder of its own)"
        ));
    }
    if folder_bytes_over(folder, LOCAL_ARCHIVE_MAX) {
        return Some(format!("{name} can't be copied (its folder is over 4 GB)"));
    }
    None
}

#[cfg(unix)]
fn opt_root_owned(program: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let opt = Path::new("/opt");
    match program.strip_prefix(opt) {
        Ok(rest) => {
            let top = rest.components().next();
            let dir = top.map(|top| opt.join(top)).unwrap_or(opt.to_path_buf());
            std::fs::metadata(dir).map_or(true, |meta| meta.uid() == 0)
        }
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn opt_root_owned(_program: &Path) -> bool {
    false
}

/// Whether the regular files under `folder` add up to more than `limit`.
/// Links are not followed. Stops as soon as the limit is passed.
pub fn folder_bytes_over(folder: &Path, limit: u64) -> bool {
    let mut total: u64 = 0;
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
                if total > limit {
                    return true;
                }
            }
        }
    }
    false
}

// --- the plan -----------------------------------------------------------------

/// One requested game, as the plan needs it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanGame {
    pub rom_id: i64,
    pub name: String,
    /// The platform's display label (what defaults key by).
    pub platform_label: String,
    /// The server platform name.
    pub platform_name: String,
    pub platform_slug: String,
    /// The game's size on the server (`fs_size_bytes`).
    pub size: u64,
    /// The largest single file (`largest_file_bytes`).
    pub largest_file: u64,
}

/// One platform's firmware on the server.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanFirmwareSet {
    pub platform_id: i64,
    pub slug: String,
    pub platform_name: String,
    pub files: u32,
    pub bytes: u64,
}

/// One game row [`BackupItems`] queues.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GameItem {
    pub rom_id: i64,
    pub name: String,
    pub platform_name: String,
}

/// One emulator row: one emulator for one OS.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmulatorItem {
    pub pick: EmulatorPick,
    pub os: PackOs,
}

/// One firmware row: one platform's firmware.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FirmwareItem {
    pub platform_id: i64,
    pub slug: String,
    pub platform_name: String,
}

/// The rows a backup queues, in the order they run: games, then emulators,
/// then firmware.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BackupItems {
    pub games: Vec<GameItem>,
    pub emulators: Vec<EmulatorItem>,
    pub firmware: Vec<FirmwareItem>,
}

impl BackupItems {
    pub fn is_empty(&self) -> bool {
        self.games.is_empty() && self.emulators.is_empty() && self.firmware.is_empty()
    }
}

/// Everything [`plan`] reads.
pub struct PlanInput<'a> {
    /// The drive key.
    pub drive: &'a str,
    pub pack: &'a Pack,
    /// The server the games come from (any form: compared by
    /// [`super::pack::server_key`]).
    pub server: &'a str,
    pub fat32: bool,
    pub free_bytes: u64,
    pub games: &'a [PlanGame],
    /// The emulator choice per platform slug ([`choose_emulator`]).
    pub emulators: &'a [(String, EmulatorChoice)],
    /// The compat tool for Windows games ([`choose_compat_tool`]).
    pub compat_tool: Option<&'a EmulatorPick>,
    /// The server's firmware for the requested systems.
    pub firmware: &'a [PlanFirmwareSet],
    pub include_firmware: bool,
    /// This computer's OS ([`PackOs::host`]).
    pub host: Option<PackOs>,
    /// Known emulator sizes, per name and OS (the service's cache).
    pub sizes: &'a HashMap<(String, PackOs), u64>,
}

/// The plan and the rows it would queue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Planned {
    pub plan: BackupPlan,
    pub items: BackupItems,
}

/// Plans a backup. Pure.
/// - Games already on the drive (same server and rom id) are skipped; on a
///   FAT32 drive, games with a file over 4 GB are `too_big`.
/// - Each system of the games to write brings its emulator for every OS of
///   the pack that does not have it yet; Windows games bring the compat tool
///   when the pack is for Linux.
/// - Firmware for those systems goes when `include_firmware` is set and the
///   drive does not have it yet.
/// - `fits` when the bytes to write, plus the write margin, fit the free
///   space. An emulator of unknown size counts as 0.
pub fn plan(input: &PlanInput<'_>) -> Planned {
    let mut items = BackupItems::default();
    let mut skipped = Vec::new();
    let mut too_big = Vec::new();
    let mut games_to_write = Vec::new();
    let mut bytes: u64 = 0;
    let mut systems: Vec<&PlanGame> = Vec::new();
    for game in input.games {
        let entry = PlanGameRef {
            rom_id: game.rom_id,
            name: game.name.clone(),
        };
        if input.pack.has_game(input.server, game.rom_id) {
            skipped.push(entry);
        } else if input.fat32 && game.largest_file > FAT32_MAX_FILE {
            too_big.push(entry);
        } else if !games_to_write.contains(&game.rom_id) {
            games_to_write.push(game.rom_id);
            bytes = bytes.saturating_add(game.size);
            items.games.push(GameItem {
                rom_id: game.rom_id,
                name: game.name.clone(),
                platform_name: game.platform_name.clone(),
            });
            systems.push(game);
        }
    }

    // One pick per emulator name, serving every system that chose it.
    let mut picks: Vec<EmulatorPick> = Vec::new();
    let mut emulators: Vec<PlanEmulator> = Vec::new();
    let mut seen_slugs = BTreeSet::new();
    let mut needs_compat = false;
    for game in &systems {
        if is_windows_platform(&game.platform_slug, &game.platform_name) {
            needs_compat = true;
        }
        if !seen_slugs.insert(game.platform_slug.clone()) {
            continue;
        }
        let choice = input
            .emulators
            .iter()
            .find(|(slug, _)| *slug == game.platform_slug)
            .map(|(_, choice)| choice);
        match choice {
            Some(EmulatorChoice::Pick(pick)) => {
                match picks.iter_mut().find(|known| known.name == pick.name) {
                    Some(known) => {
                        for slug in &pick.platforms {
                            if !known.platforms.contains(slug) {
                                known.platforms.push(slug.clone());
                            }
                        }
                    }
                    None => picks.push(pick.clone()),
                }
            }
            Some(EmulatorChoice::Refused { name, reason }) => {
                if !emulators.iter().any(|known| known.name == *name) {
                    emulators.push(PlanEmulator {
                        name: name.clone(),
                        os: Vec::new(),
                        bytes: None,
                        already: false,
                        note: reason.clone(),
                    });
                }
            }
            Some(EmulatorChoice::None) | None => {}
        }
    }
    if needs_compat && input.pack.os.contains(&PackOs::Linux) {
        if let Some(tool) = input.compat_tool {
            if !picks.iter().any(|known| known.name == tool.name) {
                picks.push(tool.clone());
            }
            // A Proton tool runs through umu-run, which a computer with no
            // network may not have: the pack carries GRID's own copy too.
            let umu = umu_pick();
            if !picks.iter().any(|known| known.name == umu.name) {
                picks.push(umu);
            }
        }
    }
    for pick in picks {
        let targets = pick.target_os(&input.pack.os, input.host);
        let missing: Vec<PackOs> = targets
            .iter()
            .copied()
            .filter(|os| {
                !input
                    .pack
                    .emulators
                    .iter()
                    .any(|have| have.name == pick.name && have.os == os.arch_tag())
            })
            .collect();
        let mut size: Option<u64> = Some(0);
        for os in &missing {
            size = match (size, input.sizes.get(&(pick.name.clone(), *os))) {
                (Some(sum), Some(one)) => Some(sum.saturating_add(*one)),
                _ => None,
            };
        }
        bytes = bytes.saturating_add(size.unwrap_or(0));
        let note = match (&pick.source, input.host) {
            (EmulatorSource::Local { .. }, Some(host)) if input.pack.os.len() > 1 => {
                format!("{} is only for {}", pick.name, os_name(host))
            }
            _ => String::new(),
        };
        emulators.push(PlanEmulator {
            name: pick.name.clone(),
            os: targets.clone(),
            bytes: if missing.is_empty() { Some(0) } else { size },
            already: missing.is_empty() && !targets.is_empty(),
            note,
        });
        for os in missing {
            items.emulators.push(EmulatorItem {
                pick: pick.clone(),
                os,
            });
        }
    }

    let mut firmware = Vec::new();
    let mut slugs_done = BTreeSet::new();
    for game in &systems {
        if !slugs_done.insert(game.platform_slug.clone()) {
            continue;
        }
        let Some(set) = input
            .firmware
            .iter()
            .find(|set| set.slug == game.platform_slug && set.files > 0)
        else {
            continue;
        };
        let already = input
            .pack
            .firmware
            .iter()
            .any(|have| have.platform == set.slug && !have.files.is_empty());
        firmware.push(PlanFirmware {
            platform_name: set.platform_name.clone(),
            files: set.files,
            bytes: set.bytes,
            already,
        });
        if input.include_firmware && !already {
            bytes = bytes.saturating_add(set.bytes);
            items.firmware.push(FirmwareItem {
                platform_id: set.platform_id,
                slug: set.slug.clone(),
                platform_name: set.platform_name.clone(),
            });
        }
    }
    let firmware_reason = if firmware.is_empty() {
        "Your server has no firmware for these systems".to_string()
    } else if firmware.iter().all(|set| set.already) {
        format!(
            "{} already has the firmware for these systems",
            input.pack.name
        )
    } else {
        String::new()
    };

    let fits = bytes == 0 || bytes.saturating_add(SPACE_MARGIN) <= input.free_bytes;
    Planned {
        plan: BackupPlan {
            drive: input.drive.to_string(),
            games_to_write,
            skipped,
            too_big,
            emulators,
            firmware,
            firmware_reason,
            bytes_to_write: bytes,
            free_bytes: input.free_bytes,
            fits,
        },
        items,
    }
}

/// "Linux" or "Windows".
pub fn os_name(os: PackOs) -> &'static str {
    match os {
        PackOs::Linux => "Linux",
        PackOs::Windows => "Windows",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::pack::{new_pack, PackEmulator, PackFirmware, PackFirmwareFile};

    const SERVER: &str = "https://romm.home.lan";

    fn game(rom_id: i64, slug: &str, size: u64, largest: u64) -> PlanGame {
        PlanGame {
            rom_id,
            name: format!("Game {rom_id}"),
            platform_label: slug.to_uppercase(),
            platform_name: slug.to_uppercase(),
            platform_slug: slug.to_string(),
            size,
            largest_file: largest,
        }
    }

    fn pick(name: &str, slug: &str) -> EmulatorPick {
        EmulatorPick {
            name: name.to_string(),
            kind: EmulatorKind::Emulator,
            source: EmulatorSource::Catalog {
                source_id: format!("owner/{}", name.to_lowercase()),
            },
            platforms: vec![slug.to_string()],
        }
    }

    fn backed_up(rom_id: i64) -> crate::backup::pack::PackGame {
        serde_json::from_value(serde_json::json!({
            "platform": "psx", "platform_name": "PSX",
            "source": { "server": "https://ROMM.home.lan:443/", "rom_id": rom_id },
            "name": format!("Game {rom_id}"), "size": 1, "files": [], "added": "2026-01-01T00:00:00Z"
        }))
        .unwrap()
    }

    struct Fixture {
        pack: Pack,
        games: Vec<PlanGame>,
        emulators: Vec<(String, EmulatorChoice)>,
        firmware: Vec<PlanFirmwareSet>,
        sizes: HashMap<(String, PackOs), u64>,
        fat32: bool,
        free: u64,
        include_firmware: bool,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                pack: new_pack("Couch kit", &[PackOs::Linux, PackOs::Windows]),
                games: vec![game(1, "psx", 100, 60), game(2, "psx", 200, 150)],
                emulators: vec![(
                    "psx".to_string(),
                    EmulatorChoice::Pick(pick("DuckStation", "psx")),
                )],
                firmware: vec![PlanFirmwareSet {
                    platform_id: 9,
                    slug: "psx".to_string(),
                    platform_name: "PSX".to_string(),
                    files: 2,
                    bytes: 1000,
                }],
                sizes: HashMap::new(),
                fat32: false,
                free: 1 << 40,
                include_firmware: true,
            }
        }

        fn run(&self) -> Planned {
            plan(&PlanInput {
                drive: "/run/media/u/T7",
                pack: &self.pack,
                server: SERVER,
                fat32: self.fat32,
                free_bytes: self.free,
                games: &self.games,
                emulators: &self.emulators,
                compat_tool: None,
                firmware: &self.firmware,
                include_firmware: self.include_firmware,
                host: Some(PackOs::Linux),
                sizes: &self.sizes,
            })
        }
    }

    #[test]
    fn skips_games_already_on_the_drive_by_server_and_rom() {
        let mut fx = Fixture::new();
        fx.pack.games.push(backed_up(1));
        let planned = fx.run();
        assert_eq!(planned.plan.games_to_write, vec![2]);
        assert_eq!(
            planned.plan.skipped,
            vec![PlanGameRef {
                rom_id: 1,
                name: "Game 1".into()
            }]
        );
        assert_eq!(planned.items.games.len(), 1);
    }

    #[test]
    fn fat32_lists_games_with_a_file_over_4_gb_as_too_big() {
        let mut fx = Fixture::new();
        fx.fat32 = true;
        fx.games.push(game(3, "psx", 5 << 30, FAT32_MAX_FILE + 1));
        let planned = fx.run();
        assert_eq!(planned.plan.too_big.len(), 1);
        assert_eq!(planned.plan.too_big[0].rom_id, 3);
        assert_eq!(planned.plan.games_to_write, vec![1, 2]);
        // Not FAT32: it goes.
        fx.fat32 = false;
        assert_eq!(fx.run().plan.games_to_write, vec![1, 2, 3]);
    }

    #[test]
    fn nothing_to_back_up_when_everything_is_on_the_drive() {
        let mut fx = Fixture::new();
        fx.pack.games.push(backed_up(1));
        fx.pack.games.push(backed_up(2));
        let planned = fx.run();
        assert!(planned.plan.games_to_write.is_empty());
        assert!(planned.items.is_empty());
        assert_eq!(planned.plan.bytes_to_write, 0);
        assert!(planned.plan.emulators.is_empty());
        assert!(planned.plan.fits);
    }

    #[test]
    fn fits_compares_bytes_plus_margin_with_free_space() {
        let mut fx = Fixture::new();
        fx.sizes.insert(("DuckStation".into(), PackOs::Linux), 10);
        fx.sizes.insert(("DuckStation".into(), PackOs::Windows), 20);
        let total = 100 + 200 + 10 + 20 + 1000;
        fx.free = total + SPACE_MARGIN;
        let planned = fx.run();
        assert_eq!(planned.plan.bytes_to_write, total);
        assert!(planned.plan.fits);
        fx.free = total + SPACE_MARGIN - 1;
        assert!(!fx.run().plan.fits);
    }

    #[test]
    fn firmware_already_on_the_drive_is_not_written_and_disables_the_box() {
        let mut fx = Fixture::new();
        fx.pack.firmware.push(PackFirmware {
            platform: "psx".into(),
            platform_name: "PSX".into(),
            files: vec![PackFirmwareFile {
                name: "scph1001.bin".into(),
                path: "firmware/psx/scph1001.bin".into(),
                size: 512,
                md5: String::new(),
            }],
        });
        let planned = fx.run();
        assert!(planned.plan.firmware[0].already);
        assert!(planned.items.firmware.is_empty());
        assert_eq!(
            planned.plan.firmware_reason,
            "Couch kit already has the firmware for these systems"
        );
        // Off by request: listed, not queued, no reason.
        let mut fx = Fixture::new();
        fx.include_firmware = false;
        let planned = fx.run();
        assert!(planned.items.firmware.is_empty());
        assert_eq!(planned.plan.firmware_reason, "");
        // The server has none.
        let mut fx = Fixture::new();
        fx.firmware.clear();
        assert_eq!(
            fx.run().plan.firmware_reason,
            "Your server has no firmware for these systems"
        );
    }

    #[test]
    fn emulator_already_on_the_drive_for_every_pack_os_is_not_queued() {
        let mut fx = Fixture::new();
        for os in [PackOs::Linux, PackOs::Windows] {
            fx.pack.emulators.push(PackEmulator {
                name: "DuckStation".into(),
                kind: "emulator".into(),
                source_id: "owner/duckstation".into(),
                version: "v1".into(),
                program: String::new(),
                args: String::new(),
                os: os.arch_tag().into(),
                platforms: vec!["psx".into()],
                path: "emulators/duckstation/v1/x/a.zip".into(),
                supplementals: Vec::new(),
                asset_name: "a.zip".into(),
                size: 5,
            });
        }
        let planned = fx.run();
        assert!(planned.items.emulators.is_empty());
        assert!(planned.plan.emulators[0].already);
        // Only the Windows build is there: Linux is queued.
        fx.pack.emulators.remove(0);
        let planned = fx.run();
        assert!(!planned.plan.emulators[0].already);
        assert_eq!(planned.items.emulators.len(), 1);
        assert_eq!(planned.items.emulators[0].os, PackOs::Linux);
        // Unknown size.
        assert_eq!(planned.plan.emulators[0].bytes, None);
    }

    #[test]
    fn one_emulator_row_per_pack_os_and_compat_tool_only_for_linux() {
        let mut fx = Fixture::new();
        fx.games.push(game(5, "win", 10, 10));
        let mut tool = pick("GE-Proton", "");
        tool.kind = EmulatorKind::CompatTool;
        let planned = plan(&PlanInput {
            drive: "E:",
            pack: &fx.pack,
            server: SERVER,
            fat32: false,
            free_bytes: 1 << 40,
            games: &fx.games,
            emulators: &fx.emulators,
            compat_tool: Some(&tool),
            firmware: &[],
            include_firmware: true,
            host: Some(PackOs::Linux),
            sizes: &fx.sizes,
        });
        let rows: Vec<(String, PackOs)> = planned
            .items
            .emulators
            .iter()
            .map(|item| (item.pick.name.clone(), item.os))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("DuckStation".into(), PackOs::Linux),
                ("DuckStation".into(), PackOs::Windows),
                ("GE-Proton".into(), PackOs::Linux),
                ("umu-launcher".into(), PackOs::Linux),
            ]
        );
        let umu = &planned.items.emulators[3].pick;
        assert_eq!(umu.kind, EmulatorKind::CompatTool);
        assert_eq!(umu.source_id(), crate::launch::umu::UMU_SOURCE_ID);
        // A Windows-only pack carries no compat tool.
        fx.pack.os = vec![PackOs::Windows];
        let planned = plan(&PlanInput {
            drive: "E:",
            pack: &fx.pack,
            server: SERVER,
            fat32: false,
            free_bytes: 1 << 40,
            games: &fx.games,
            emulators: &fx.emulators,
            compat_tool: Some(&tool),
            firmware: &[],
            include_firmware: true,
            host: Some(PackOs::Linux),
            sizes: &fx.sizes,
        });
        assert!(planned
            .items
            .emulators
            .iter()
            .all(|item| item.pick.name != "GE-Proton" && item.pick.name != "umu-launcher"));
    }

    #[test]
    fn umu_launcher_already_on_the_drive_is_not_queued_again() {
        let mut fx = Fixture::new();
        fx.pack.os = vec![PackOs::Linux];
        fx.games = vec![game(5, "win", 10, 10)];
        fx.pack.emulators.push(PackEmulator {
            name: "umu-launcher".into(),
            kind: "compat_tool".into(),
            source_id: crate::launch::umu::UMU_SOURCE_ID.into(),
            version: "1.2.6".into(),
            program: String::new(),
            args: String::new(),
            os: PackOs::Linux.arch_tag().into(),
            platforms: Vec::new(),
            path: "emulators/umu-launcher/1.2.6/linux-x86_64/umu.tar".into(),
            supplementals: Vec::new(),
            asset_name: "umu.tar".into(),
            size: 10,
        });
        let mut tool = pick("GE-Proton", "");
        tool.kind = EmulatorKind::CompatTool;
        let planned = plan(&PlanInput {
            drive: "E:",
            pack: &fx.pack,
            server: SERVER,
            fat32: false,
            free_bytes: 1 << 40,
            games: &fx.games,
            emulators: &[],
            compat_tool: Some(&tool),
            firmware: &[],
            include_firmware: true,
            host: Some(PackOs::Linux),
            sizes: &fx.sizes,
        });
        let names: Vec<&str> = planned
            .items
            .emulators
            .iter()
            .map(|item| item.pick.name.as_str())
            .collect();
        assert_eq!(names, vec!["GE-Proton"]);
        assert!(planned
            .plan
            .emulators
            .iter()
            .any(|row| row.name == "umu-launcher" && row.already));
        assert!(is_umu_entry(&fx.pack.emulators[0].source_id));
    }

    #[test]
    fn a_manual_emulator_goes_for_this_os_only_with_a_note() {
        let mut fx = Fixture::new();
        fx.emulators = vec![(
            "psx".into(),
            EmulatorChoice::Pick(EmulatorPick {
                name: "MyEmu".into(),
                kind: EmulatorKind::Emulator,
                source: EmulatorSource::Local {
                    program: PathBuf::from("/home/u/emus/myemu/myemu"),
                    args: "-f".into(),
                },
                platforms: vec!["psx".into()],
            }),
        )];
        let planned = fx.run();
        assert_eq!(planned.items.emulators.len(), 1);
        assert_eq!(planned.items.emulators[0].os, PackOs::Linux);
        assert_eq!(planned.plan.emulators[0].note, "MyEmu is only for Linux");
        assert_eq!(planned.items.emulators[0].pick.source_id(), "local:MyEmu");
    }

    #[test]
    fn a_refused_emulator_is_listed_with_its_reason_and_not_queued() {
        let mut fx = Fixture::new();
        fx.emulators = vec![(
            "psx".into(),
            EmulatorChoice::Refused {
                name: "SysEmu".into(),
                reason: "SysEmu can't be copied (installed by the system)".into(),
            },
        )];
        let planned = fx.run();
        assert!(planned.items.emulators.is_empty());
        assert_eq!(
            planned.plan.emulators[0].note,
            "SysEmu can't be copied (installed by the system)"
        );
    }

    #[test]
    fn system_folders_are_recognised_in_both_path_styles() {
        assert!(is_system_path("/usr/bin/mednafen", false));
        assert!(is_system_path("/bin/x", false));
        assert!(is_system_path("/opt/emu/emu", true));
        assert!(!is_system_path("/opt/emu/emu", false));
        assert!(!is_system_path("/home/u/emus/x", false));
        assert!(is_system_path("C:\\Windows\\System32\\x.exe", false));
        assert!(is_system_path("c:/program files (x86)/Emu/emu.exe", false));
        assert!(is_system_path("D:\\Program Files\\Emu\\emu.exe", false));
        assert!(!is_system_path("D:\\Emus\\emu.exe", false));
    }

    #[test]
    fn a_manual_emulator_under_usr_is_refused_with_the_reason() {
        assert_eq!(
            archive_refusal("Mednafen", Path::new("/usr/bin/mednafen")).as_deref(),
            Some("Mednafen can't be copied (installed by the system)")
        );
        assert_eq!(
            archive_refusal("Emu", Path::new("C:\\Windows\\emu.exe")).as_deref(),
            Some("Emu can't be copied (installed by the system)")
        );
    }

    #[test]
    fn a_manual_emulator_in_its_own_folder_is_accepted_and_a_missing_one_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("myemu");
        std::fs::create_dir_all(&folder).unwrap();
        let program = folder.join("myemu");
        std::fs::write(&program, b"#!/bin/sh\n").unwrap();
        assert_eq!(archive_refusal("MyEmu", &program), None);
        assert_eq!(
            archive_refusal("MyEmu", &folder.join("gone")).as_deref(),
            Some("MyEmu can't be copied (its program was not found)")
        );
    }

    #[test]
    fn folder_size_check_stops_past_the_limit() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a"), vec![0u8; 10]).unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/b"), vec![0u8; 10]).unwrap();
        assert!(!folder_bytes_over(tmp.path(), 20));
        assert!(folder_bytes_over(tmp.path(), 19));
    }

    #[test]
    fn choose_emulator_reads_the_default_entry_source() {
        let mut config = Config::default();
        config
            .default_emulators
            .insert("PlayStation".into(), "MyEmu".into());
        config.emulators.push(crate::config::EmulatorEntry {
            name: "MyEmu".into(),
            path: "/usr/bin/myemu".into(),
            ..Default::default()
        });
        assert_eq!(
            choose_emulator(&config, &[], "PlayStation", "psx", "PlayStation"),
            EmulatorChoice::Refused {
                name: "MyEmu".into(),
                reason: "MyEmu can't be copied (installed by the system)".into()
            }
        );
        config.emulators[0].source_id = "romm:psx/duckstation".into();
        match choose_emulator(&config, &[], "PlayStation", "psx", "PlayStation") {
            EmulatorChoice::Pick(pick) => assert_eq!(
                pick.source,
                EmulatorSource::Server {
                    source_id: "romm:psx/duckstation".into()
                }
            ),
            other => panic!("{other:?}"),
        }
        // Explicit none, and native games.
        config
            .default_emulators
            .insert("PlayStation".into(), NO_EMULATOR.into());
        assert_eq!(
            choose_emulator(&config, &[], "PlayStation", "psx", "PlayStation"),
            EmulatorChoice::None
        );
        assert_eq!(
            choose_emulator(&config, &[], "Windows", "win", "Windows"),
            EmulatorChoice::None
        );
    }
}
