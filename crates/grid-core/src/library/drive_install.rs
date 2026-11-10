//! Installs from a backup drive's `gridpack/` folder: games, emulators and
//! compat tools (`docs/superpowers/plans/2026-11-09-game-backups.md`,
//! Task 11 and Part 4).
//!
//! A game plans exactly like a server install ([`super::plan_install`] over
//! [`pack::pack_game_detail`]) and finalizes through the same path; only its
//! bytes come from the drive ([`super::GameSource::Drive`]). An emulator
//! runs as an ordinary [`super::EmulatorJob`] whose download step copies the
//! backed-up release instead of resolving a forge ([`LocalBuild`]).
//!
//! Firmware from a pack needs no queue row: the app layer calls
//! `firmware::install_platform_firmware_from` with a `PackFirmwareSource`.
//!
//! A drive's `gridpack.json` is untrusted: every path goes through
//! [`pack::resolve`], and an emulator still passes the ELF/PE header check
//! before it is registered.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde_json::Value;

use super::backup_job::DriveTarget;
use super::queue::{EntryMeta, JobKey};
use super::{
    drive_copy, drive_restore, file_stem_of, is_emulators_platform, plan_install, safe_file_name,
    server_entry_name, DriveSaves, DriveSource, EmulatorJob, GameSource, InstallMode,
    InstallService, JobPayload, JobTarget, LibraryError, ResolvedPaths, COMPAT_TOOL_PLATFORM,
    EMULATOR_PLATFORM, SERVER_SOURCE_PREFIX,
};
use crate::backup::archive::PartSource;
use crate::backup::pack::{self, PackEmulator, PackError, PackGame, PackOs, PackSaves};
use crate::config::Config;
use crate::images::urls::resolve_image_url;
use crate::launch::forge::ResolvedDownload;
use crate::launch::{catalog, compat, emu_install, umu};
use crate::library::extract::should_extract;

/// The `source_id` prefix of an archived manual emulator in a pack.
pub const LOCAL_SOURCE_PREFIX: &str = "local:";

/// The `ResolvedDownload::provider` of an emulator copied from a drive.
pub const GRIDPACK_PROVIDER: &str = "gridpack";

/// A hook run after each chunk an install copies off a drive. Tests use it
/// to remove the drive in the middle of a copy.
pub type DriveCopyHook = Arc<dyn Fn() -> io::Result<()> + Send + Sync>;

/// An emulator release on a backup drive, copied instead of downloaded.
#[derive(Clone, Debug)]
pub struct LocalBuild {
    /// The drive key, so a removed drive fails the row with its reason.
    pub drive_id: String,
    pub label: String,
    pub mount: PathBuf,
    /// The primary file on the drive (absolute, validated).
    pub primary: PathBuf,
    /// Supplemental files on the drive (absolute, validated).
    pub supplementals: Vec<PathBuf>,
    /// The release version the pack records.
    pub version: String,
    /// The primary's release asset name.
    pub asset_name: String,
    /// Set for an archived manual emulator (`local:` source).
    pub manual: Option<ManualEmulator>,
}

/// What an archived manual emulator registers with.
#[derive(Clone, Debug)]
pub struct ManualEmulator {
    /// The program's path inside the archive (`/`-separated).
    pub program: String,
}

/// What `retry` rebuilds an install-from-drive row from.
#[derive(Clone)]
pub(crate) enum DriveRow {
    Game {
        drive: DriveTarget,
        game: Box<PackGame>,
        live_server: Option<String>,
    },
    Emulator {
        drive: DriveTarget,
        entry: Box<PackEmulator>,
    },
}

/// The error for a pack path that cannot be used.
fn damaged(label: &str, error: PackError) -> LibraryError {
    LibraryError::Extract(format!("The backup on {label} can't be used: {error}"))
}

/// The drive file of every file of `game`, by server file id.
fn game_files(
    drive: &DriveTarget,
    game: &PackGame,
) -> Result<BTreeMap<i64, PathBuf>, LibraryError> {
    game.files
        .iter()
        .map(|file| {
            pack::resolve(&drive.mount, &file.path)
                .map(|path| (file.server_file_id, path))
                .map_err(|e| damaged(&drive.label, e))
        })
        .collect()
}

/// A game's saves on the drive mounted at `mount`, its parts resolved. A
/// part path the drive can't use never refuses the install: the saves are
/// marked unusable and skipped with a warning when it finalizes.
fn drive_saves(mount: &Path, saves: &PackSaves) -> DriveSaves {
    let parts: Result<Vec<PartSource>, PackError> = saves
        .parts
        .iter()
        .map(|part| {
            pack::resolve(mount, &part.path).map(|path| PartSource {
                path,
                size: part.size,
                md5: part.md5.clone(),
            })
        })
        .collect();
    match parts {
        Ok(parts) => DriveSaves {
            folders: saves.folders.clone(),
            parts,
            unusable: false,
        },
        Err(_) => DriveSaves {
            folders: saves.folders.clone(),
            parts: Vec::new(),
            unusable: true,
        },
    }
}

/// The compat-tool entry a drive's pack holds for the OS tag `os` whose
/// `source_id` passes `is`, if any (umu-launcher, the Steam Linux Runtime).
fn pack_entry(mount: &Path, os: &str, is: fn(&str) -> bool) -> Option<PackEmulator> {
    match pack::read_pack(mount) {
        Ok(pack::PackState::Ready(pack)) => pack.emulators.into_iter().find(|emulator| {
            emulator.kind == "compat_tool" && emulator.os == os && is(&emulator.source_id)
        }),
        _ => None,
    }
}

/// Whether a drive install keeps the pack's rom id: always offline, and
/// online only when the live session is the server the backup came from.
pub fn keeps_rom_id(pack_server: &str, live_server: Option<&str>) -> bool {
    match live_server {
        Some(live) => pack::server_key(live) == pack::server_key(pack_server),
        None => true,
    }
}

impl InstallService {
    /// Starts (or queues) the install of `game` from the backup drive
    /// `drive`.
    ///
    /// The plan is the server install's own, so the files land where a
    /// server install puts them and finalize is unchanged. The row is keyed
    /// `JobKey::Rom(rom_id)`, so it dedupes with a server install of the same
    /// game, and carries the drive, so removing the drive fails it with
    /// "<label> was removed".
    ///
    /// The registry row records `source_server` (the pack's server) and image
    /// fields as absolute URLs on that server, so covers keep working
    /// offline and after a later connect. `live_server` is the URL of the
    /// connected session, `None` offline: when it names another server the
    /// row is stored with no `rom_id` (that server's ids mean other games).
    ///
    /// `Err` only before admission: a closed library gate, no library path,
    /// a drive marked removed, an unsafe pack path, or a backup missing a
    /// planned file. A game already live in the queue is ignored silently.
    pub fn install_from_drive(
        self: &Arc<Self>,
        drive: DriveTarget,
        game: &PackGame,
        live_server: Option<&str>,
    ) -> Result<(), LibraryError> {
        self.check_library_gate()?;
        let library = self.library_root()?;
        if let Some(reason) = self.removed_reason(&drive.key) {
            return Err(LibraryError::DriveRemoved(reason));
        }
        let server = crate::romm::strip_userinfo(game.source.server.trim())
            .trim_end_matches('/')
            .to_string();
        let mut detail = pack::pack_game_detail(game);
        detail.cover_small_path = resolve_image_url(&detail.cover_small_path, &server);
        detail.cover_large_path = resolve_image_url(&detail.cover_large_path, &server);
        let absolute = |urls: &[String]| -> Vec<String> {
            urls.iter()
                .map(|url| resolve_image_url(url, &server))
                .filter(|url| !url.is_empty())
                .collect()
        };
        detail.screenshot_urls = absolute(&detail.screenshot_urls);
        detail.fanart_urls = absolute(&detail.fanart_urls);

        let source = DriveSource {
            drive_id: drive.key.clone(),
            label: drive.label.clone(),
            mount: drive.mount.clone(),
            files: game_files(&drive, game)?,
            server: server.clone(),
            keep_rom_id: keeps_rom_id(&server, live_server),
            saves: game
                .saves
                .as_ref()
                .map(|saves| drive_saves(&drive.mount, saves)),
        };
        // A copy of an install restores its archive; anything else (a
        // download, or an install copied as the server's files) plans like a
        // server install.
        let job = match &game.installed {
            None => plan_install(&detail, &library, GameSource::Drive(source))?,
            Some(installed) => {
                // The free-space check is the ledger's, at admission
                // (`InstallService::attach_space`), so queued installs count.
                drive_restore::plan_restore(&detail, game, installed, &library, source, None)?
            }
        };
        let key = JobKey::Rom(job.rom_id);
        let title = job.detail.name.clone();
        let platform = job.detail.platform_name.clone();
        let kind = if job.mode == InstallMode::Base && is_emulators_platform(&platform) {
            "emulator"
        } else {
            job.mode.kind()
        };
        let admitted = self.admit_meta(
            key,
            &title,
            &platform,
            kind,
            JobPayload::Game(job),
            drive_meta(&drive),
        )?;
        if let Some(id) = admitted {
            self.drive_rows.lock().unwrap().insert(
                id,
                DriveRow::Game {
                    drive,
                    game: Box::new(game.clone()),
                    live_server: live_server.map(str::to_string),
                },
            );
        }
        Ok(())
    }

    /// Starts (or queues) the install of the emulator release `entry` from
    /// the backup drive `drive`.
    ///
    /// - A catalog emulator or a server build (`romm:` source) installs
    ///   under `emulators/<name>` like a download would, and its
    ///   `[[emulators]]` entry records the pack's `source_id` and version, so
    ///   update checks work later.
    /// - A compat tool installs under the managed compat-tools root and
    ///   records a compat-tool install.
    /// - An archived manual emulator (`local:` source) is extracted under
    ///   `emulators/<name>` and registered as a manual entry with the pack's
    ///   program and launch options, and no update source.
    ///
    /// The program still passes the ELF/PE header check: a release for
    /// another OS or processor fails the row.
    ///
    /// `Err` only before admission: a closed gate, no library path, a drive
    /// marked removed, a release for another OS, an unsafe pack path, or an
    /// emulator this build does not know.
    pub fn install_emulator_from_drive(
        self: &Arc<Self>,
        drive: DriveTarget,
        entry: &PackEmulator,
    ) -> Result<(), LibraryError> {
        self.check_library_gate()?;
        if let Some(reason) = self.removed_reason(&drive.key) {
            return Err(LibraryError::DriveRemoved(reason));
        }
        let host = PackOs::host();
        if host.map(PackOs::arch_tag) != Some(entry.os.as_str()) {
            return Err(LibraryError::Extract(format!(
                "The copy of {} on {} is not for this computer",
                entry.name, drive.label
            )));
        }
        let resolve =
            |rel: &str| pack::resolve(&drive.mount, rel).map_err(|e| damaged(&drive.label, e));
        let primary = resolve(&entry.path)?;
        let supplementals = entry
            .supplementals
            .iter()
            .map(|rel| resolve(rel))
            .collect::<Result<Vec<_>, _>>()?;
        let manual = entry
            .source_id
            .starts_with(LOCAL_SOURCE_PREFIX)
            .then(|| ManualEmulator {
                program: entry.program.clone(),
            });
        if let Some(manual) = &manual {
            pack::validate_rel(&manual.program).map_err(|e| damaged(&drive.label, e))?;
        }
        let local = LocalBuild {
            drive_id: drive.key.clone(),
            label: drive.label.clone(),
            mount: drive.mount.clone(),
            primary,
            supplementals,
            version: entry.version.clone(),
            asset_name: entry.asset_name.clone(),
            manual,
        };
        let unknown = || LibraryError::Registry(format!("unknown emulator: {}", entry.source_id));

        let compat_tool = entry.kind == "compat_tool";
        let umu_launcher = compat_tool && crate::backup::plan::is_umu_entry(&entry.source_id);
        let steam_runtime =
            compat_tool && crate::backup::steam_runtime::is_runtime_entry(&entry.source_id);
        let mut needs_umu = false;
        let (job, platform, kind) = if steam_runtime {
            let umu_local = crate::backup::steam_runtime::umu_local_here().ok_or_else(|| {
                LibraryError::Extract("No home folder for the Steam Linux Runtime".to_string())
            })?;
            let job = EmulatorJob {
                space: None,
                source_id: crate::backup::steam_runtime::RUNTIME_SOURCE_ID.to_string(),
                profile_name: crate::backup::steam_runtime::RUNTIME_TITLE.to_string(),
                profile_args: String::new(),
                configured_tag: entry.version.clone(),
                raw_source: Value::Null,
                library: umu_local,
                forge: self.forge()?,
                resolved: None,
                target: JobTarget::SteamRuntime,
                entry_name: crate::backup::steam_runtime::RUNTIME_TITLE.to_string(),
                romm: None,
                local: Some(local),
            };
            (job, COMPAT_TOOL_PLATFORM, "compat_tool")
        } else if umu_launcher {
            let job = EmulatorJob {
                space: None,
                source_id: umu::UMU_SOURCE_ID.to_string(),
                profile_name: umu::UMU_TITLE.to_string(),
                profile_args: String::new(),
                configured_tag: entry.version.clone(),
                raw_source: Value::Null,
                library: compat::managed_root(),
                forge: self.forge()?,
                resolved: None,
                target: JobTarget::UmuLauncher,
                entry_name: umu::UMU_TITLE.to_string(),
                romm: None,
                local: Some(local),
            };
            (job, COMPAT_TOOL_PLATFORM, "compat_tool")
        } else if compat_tool {
            let profile = catalog::find_compat_profile(&self.profiles, &entry.source_id)
                .ok_or_else(unknown)?;
            needs_umu = profile.compat_tool_type == "proton";
            let configured_tag = configured_tag(profile.source.as_ref(), &entry.version);
            let job = EmulatorJob {
                space: None,
                source_id: entry.source_id.clone(),
                profile_name: profile.name.clone(),
                profile_args: profile.args.clone(),
                configured_tag,
                raw_source: Value::Null,
                library: compat::managed_root(),
                forge: self.forge()?,
                resolved: None,
                target: JobTarget::CompatTool,
                entry_name: profile.name.clone(),
                romm: None,
                local: Some(local),
            };
            (job, COMPAT_TOOL_PLATFORM, "compat_tool")
        } else {
            let library = self.library_root()?;
            let (profile_name, profile_args, configured_tag, entry_name) = if local.manual.is_some()
            {
                (
                    entry.name.clone(),
                    entry.args.clone(),
                    "local".to_string(),
                    entry.name.clone(),
                )
            } else if entry.source_id.starts_with(SERVER_SOURCE_PREFIX) {
                let profile = self
                    .profiles
                    .iter()
                    .find(|p| p.name == entry.name && !p.is_compat_tool)
                    .ok_or_else(unknown)?;
                let config = Config::load(&self.config_path)?;
                let entry_name = server_entry_name(&config, &profile.name, &entry.source_id);
                (
                    profile.name.clone(),
                    profile.args.clone(),
                    entry.version.clone(),
                    entry_name,
                )
            } else {
                let profile =
                    catalog::find_profile(&self.profiles, &entry.source_id).ok_or_else(unknown)?;
                (
                    profile.name.clone(),
                    profile.args.clone(),
                    configured_tag(profile.source.as_ref(), &entry.version),
                    profile.name.clone(),
                )
            };
            let job = EmulatorJob {
                space: None,
                source_id: entry.source_id.clone(),
                profile_name,
                profile_args,
                configured_tag,
                raw_source: Value::Null,
                library,
                forge: self.forge()?,
                resolved: None,
                target: JobTarget::Emulator,
                entry_name,
                romm: None,
                local: Some(local),
            };
            (job, EMULATOR_PLATFORM, "emulator")
        };
        let title = job.entry_name.clone();
        let admitted = self.admit_meta(
            JobKey::Emulator(entry.source_id.clone()),
            &title,
            platform,
            kind,
            JobPayload::Emulator(job),
            drive_meta(&drive),
        )?;
        if let Some(id) = admitted {
            self.drive_rows.lock().unwrap().insert(
                id,
                DriveRow::Emulator {
                    drive: drive.clone(),
                    entry: Box::new(entry.clone()),
                },
            );
        }
        // A Proton tool runs through umu-run. With none on this computer,
        // the drive's own umu-launcher (backed up beside the tool) goes
        // behind it as its own row; its failure is that row's.
        if needs_umu && cfg!(target_os = "linux") && !self.umu_available() {
            match pack_entry(&drive.mount, &entry.os, crate::backup::plan::is_umu_entry) {
                Some(umu_entry) => {
                    if let Err(error) = self.install_emulator_from_drive(drive.clone(), &umu_entry)
                    {
                        tracing::warn!(%error, "umu-launcher from the drive could not be queued");
                    }
                }
                None => tracing::info!("the drive holds no umu-launcher for this computer"),
            }
        }
        // umu-run downloads the Steam Linux Runtime on first use. With none
        // here, the drive's copy (when the backup carried one) is placed
        // where umu looks, so a first launch with no network works.
        let runtime_missing = crate::backup::steam_runtime::umu_local_here()
            .is_some_and(|local| !crate::backup::steam_runtime::runtime_present(&local));
        if needs_umu && cfg!(target_os = "linux") && runtime_missing {
            if let Some(runtime) = pack_entry(
                &drive.mount,
                &entry.os,
                crate::backup::steam_runtime::is_runtime_entry,
            ) {
                if let Err(error) = self.install_emulator_from_drive(drive, &runtime) {
                    tracing::warn!(%error, "the Steam Linux Runtime from the drive could not be queued");
                }
            }
        }
        Ok(())
    }

    /// Retries a failed install-from-drive row from the same drive.
    pub(crate) fn retry_drive_row(
        self: &Arc<Self>,
        entry_id: u64,
        row: DriveRow,
    ) -> Result<(), LibraryError> {
        let drive = match &row {
            DriveRow::Game { drive, .. } | DriveRow::Emulator { drive, .. } => drive,
        };
        if self.removed_reason(&drive.key).is_some() || !pack::pack_dir(&drive.mount).is_dir() {
            return Err(LibraryError::Extract(format!(
                "Connect {} to retry",
                drive.label
            )));
        }
        self.dismiss(entry_id);
        match row {
            DriveRow::Game {
                drive,
                game,
                live_server,
            } => self.install_from_drive(drive, &game, live_server.as_deref()),
            DriveRow::Emulator { drive, entry } => self.install_emulator_from_drive(drive, &entry),
        }
    }

    /// Installs a hook run after each chunk an install copies off a drive.
    /// Tests use it to remove the drive mid-copy; production never sets one.
    #[doc(hidden)]
    pub fn set_drive_copy_hook(&self, hook: Option<DriveCopyHook>) {
        *self.drive_copy_hook.write().unwrap() = hook;
    }

    pub(super) fn drive_copy_hook(&self) -> Option<DriveCopyHook> {
        self.drive_copy_hook.read().unwrap().clone()
    }
}

/// The queue meta of a row that copies from `drive`.
fn drive_meta(drive: &DriveTarget) -> EntryMeta {
    EntryMeta {
        drive_id: drive.key.clone(),
        drive_name: drive.name.clone(),
        drive_label: drive.label.clone(),
        batch: 0,
    }
}

/// The tag a catalog profile's source block configures (`latest` when it
/// pins nothing); `fallback` when the profile has no source block.
fn configured_tag(source: Option<&Value>, fallback: &str) -> String {
    match source.and_then(Value::as_object) {
        Some(map) => catalog::configured_tag(map),
        None => fallback.to_string(),
    }
}

/// Copies `files` off the drive mounted at `mount` on the blocking pool
/// ([`drive_copy::copy_targets`]), running `hook` after each chunk.
pub(super) async fn copy_off_drive(
    mount: &Path,
    label: &str,
    files: Vec<drive_copy::CopyFile>,
    cancel: Arc<AtomicBool>,
    hook: Option<DriveCopyHook>,
    mut on_progress: impl FnMut(u64, u64, f64) + Send + 'static,
) -> Result<(), LibraryError> {
    let root = pack::pack_dir(mount);
    let label = label.to_string();
    tokio::task::spawn_blocking(move || {
        let mut after_chunk = || hook.as_ref().map_or(Ok(()), |hook| hook());
        drive_copy::copy_targets_with(
            &root,
            &label,
            &files,
            &cancel,
            &mut on_progress,
            &mut after_chunk,
        )
    })
    .await
    .map_err(|e| LibraryError::Extract(format!("the copy did not finish: {e}")))?
}

/// The download step of an emulator installed from a drive: copies the
/// backed-up release into the install directory under the names a download
/// would give it, and records it for finalize as a resolved download from
/// provider [`GRIDPACK_PROVIDER`] (`romm` for a server build, as its
/// download records).
pub(super) async fn copy_local_emulator(
    job: &mut EmulatorJob,
    cancel: Arc<AtomicBool>,
    hook: Option<DriveCopyHook>,
    on_progress: impl FnMut(u64, u64, f64) + Send + 'static,
) -> Result<(), LibraryError> {
    let Some(local) = job.local.clone() else {
        return Err(LibraryError::Extract(
            "the emulator copy plan was lost".to_string(),
        ));
    };
    let install_dir = match job.target {
        JobTarget::CompatTool => {
            let dir_name =
                emu_install::archive_file_name(&job.profile_name, &job.configured_tag, "");
            emu_install::compat_tool_install_dir(&job.library, &file_stem_of(&dir_name))
        }
        JobTarget::UmuLauncher => umu::install_dir(&job.library),
        JobTarget::SteamRuntime => crate::backup::steam_runtime::download_dir(&job.library),
        JobTarget::Emulator => emu_install::emulator_install_dir(&job.library, &job.entry_name),
    };
    let server_build = job.source_id.starts_with(SERVER_SOURCE_PREFIX);
    let archive_name = if local.manual.is_some() || server_build {
        safe_file_name(&local.asset_name, &local.asset_name)?.to_string()
    } else {
        emu_install::archive_file_name(&job.profile_name, &job.configured_tag, &local.asset_name)
    };
    let archive = install_dir.join(safe_file_name(&archive_name, &local.asset_name)?);
    // As for a download: a kept non-archive (an AppImage) must not be
    // reused by size.
    if !should_extract("", EMULATOR_PLATFORM, &archive) {
        let _ = fs::remove_file(&archive);
    }
    let mut supplemental_paths = Vec::new();
    for (index, src) in local.supplementals.iter().enumerate() {
        let asset = src
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = emu_install::supplemental_file_name(&archive, index + 1, &asset);
        supplemental_paths.push(install_dir.join(safe_file_name(&name, &asset)?));
    }

    let size_of = |path: &Path| fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let mut files = vec![(
        local.primary.clone(),
        archive.clone(),
        size_of(&local.primary),
    )];
    for (src, dest) in local.supplementals.iter().zip(&supplemental_paths) {
        files.push((src.clone(), dest.clone(), size_of(src)));
    }
    let total: u64 = files.iter().map(|(_, _, size)| size).sum();

    let (provider, owner, repo) = if server_build {
        let folder = job
            .source_id
            .trim_start_matches(SERVER_SOURCE_PREFIX)
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string();
        ("romm".to_string(), String::new(), folder)
    } else if local.manual.is_some() {
        (GRIDPACK_PROVIDER.to_string(), String::new(), String::new())
    } else {
        let (owner, repo) = job.source_id.split_once('/').unwrap_or(("", ""));
        (
            GRIDPACK_PROVIDER.to_string(),
            owner.to_string(),
            repo.to_string(),
        )
    };
    job.resolved = Some(ResolvedPaths {
        install_dir,
        archive,
        supplementals: supplemental_paths,
        resolved: ResolvedDownload {
            provider,
            owner,
            repo,
            release_tag: local.version.clone(),
            asset_name: local.asset_name.clone(),
            download_url: String::new(),
            size: i64::try_from(total).unwrap_or(i64::MAX),
            digest: String::new(),
        },
    });
    copy_off_drive(&local.mount, &local.label, files, cancel, hook, on_progress).await
}

/// The executable of an archived manual emulator: `program` (pack-relative,
/// `/`-separated) inside the extracted `install_dir`. Refused when it is not
/// a plain path, does not exist, or resolves outside `install_dir`.
pub(super) fn manual_program(
    install_dir: &Path,
    program: &str,
    name: &str,
) -> Result<PathBuf, LibraryError> {
    let missing =
        || LibraryError::Extract(format!("The backup of {name} has no program at {program}"));
    let rel = pack::validate_rel(program).map_err(|_| missing())?;
    let exe = install_dir.join(rel);
    if !exe.is_file() {
        return Err(missing());
    }
    let inside = match (
        crate::platform::canonicalize(&exe),
        crate::platform::canonicalize(install_dir),
    ) {
        (Ok(exe), Ok(dir)) => exe.starts_with(dir),
        _ => false,
    };
    if !inside {
        return Err(missing());
    }
    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_install_keeps_the_rom_id_offline_and_on_the_same_server() {
        assert!(keeps_rom_id("https://romm.example", None));
        assert!(keeps_rom_id(
            "https://romm.example",
            Some("https://user:pw@ROMM.example:443/")
        ));
        assert!(!keeps_rom_id(
            "https://romm.example",
            Some("https://other.example")
        ));
    }

    #[test]
    fn a_manual_program_must_be_a_file_inside_the_install() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("emu");
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::write(dir.join("bin/run"), b"x").unwrap();
        assert_eq!(
            manual_program(&dir, "bin/run", "Emu").unwrap(),
            dir.join("bin").join("run")
        );
        for bad in ["../run", "bin/missing", "/etc/passwd", "bin"] {
            assert_eq!(
                manual_program(&dir, bad, "Emu").unwrap_err().to_string(),
                format!("The backup of Emu has no program at {bad}")
            );
        }
    }
}
