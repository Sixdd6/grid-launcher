//! Backup jobs: one game, emulator release or firmware set written to a
//! drive's `gridpack/`. A backup row runs through the same queue as an
//! install (Part 3 of the backups plan):
//! - the **download phase** ([`download`], on the download slot) fetches
//!   everything from the server (or a forge, or zips a manual emulator's
//!   folder) into `<library>/.grid-staging/backup-<entry id>/`;
//! - the **write phase** ([`write`], on the finalize slot's blocking pool)
//!   writes the staged files to the drive through `backup::write`, then
//!   records the item in the drive's index under its lock.
//!
//! The staging folder is deleted whatever happens. Nothing here ever logs a
//! URL: image and download URLs can carry a query string.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::backup::pack::{
    self, asset_dir_rel, emulator_dir_rel, firmware_dir_rel, game_dir_rel, game_file_rel,
    pack_component, unique_rel, PackAssets, PackEmulator, PackError, PackFile, PackFirmware,
    PackFirmwareFile, PackGame, PackOs, PackSource, PackState,
};
use crate::backup::plan::{
    os_name, EmulatorItem, EmulatorKind, EmulatorSource, FirmwareItem, GameItem,
};
use crate::backup::write::{self as drive_write, FileWrite, WriteError, WriteTarget};
use crate::config::Config;
use crate::images::urls::{extension_for, filter_to_server_host, server_resolver};
use crate::launch::binary_target::Cpu;
use crate::launch::forge::{ForgeClient, ForgeProvider, ResolvedDownload};
use crate::launch::setup::{newest_server_build, recommendations, ServerPackage};
use crate::romm::{strip_userinfo, RomDetail, RomFile, RommClient};

use super::download::{download_targets, FileTarget, RommProvider};
use super::queue::BackupItemKey;
use super::update_detection::is_emulators_platform;
use super::{content_target, download_set, encode_file_segment, DownloadSet, LibraryError};
use super::{InstallService, COMPAT_TOOL_PLATFORM, EMULATOR_PLATFORM};

/// The library folder backups stage their downloads in.
pub const STAGING_DIR: &str = ".grid-staging";

/// The most screenshots a backed-up game carries.
const MAX_SCREENSHOTS: usize = 8;

/// The drive a backup writes to, resolved from a drive the current scan
/// found. A queued job keeps the mount path from its admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriveTarget {
    /// The drive key (`backup::drives::drive_key`): its mount path, or its
    /// drive letter on Windows.
    pub key: String,
    pub mount: PathBuf,
    /// The volume label, for "<label> was removed".
    pub label: String,
    /// The pack name the user gave the drive.
    pub name: String,
    pub fat32: bool,
}

/// What [`InstallService::abort_drive`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct AbortReport {
    /// Rows that were downloading or writing for the drive: they fail with
    /// the reason.
    pub failed: u32,
    /// Waiting rows folded into the one `backup_cancelled` row.
    pub cancelled: u32,
    /// Rows for the drive that had completed (still in the history).
    pub written: u32,
}

/// One backup row's item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupItem {
    Game(GameItem),
    Emulator(EmulatorItem),
    Firmware(FirmwareItem),
}

impl BackupItem {
    /// The queue key part that names this item.
    pub fn key(&self) -> BackupItemKey {
        match self {
            Self::Game(game) => BackupItemKey::Game(game.rom_id),
            Self::Emulator(item) => BackupItemKey::Emulator {
                name: item.pick.name.clone(),
                os: item.os,
            },
            Self::Firmware(item) => BackupItemKey::Firmware(item.slug.clone()),
        }
    }

    /// The row's title, platform column and kind.
    pub(crate) fn row(&self) -> (String, String, &'static str) {
        match self {
            Self::Game(game) => (game.name.clone(), game.platform_name.clone(), "backup_game"),
            Self::Emulator(item) => {
                let platform = match item.pick.kind {
                    EmulatorKind::Emulator => EMULATOR_PLATFORM,
                    EmulatorKind::CompatTool => COMPAT_TOOL_PLATFORM,
                };
                (
                    format!("{} for {}", item.pick.name, os_name(item.os)),
                    platform.to_string(),
                    "backup_emulator",
                )
            }
            Self::Firmware(item) => (
                format!("{} firmware", item.platform_name),
                item.platform_name.clone(),
                "backup_firmware",
            ),
        }
    }
}

/// One admitted backup row.
pub(crate) struct BackupJob {
    pub drive: DriveTarget,
    pub item: BackupItem,
    pub client: Arc<RommClient>,
    /// The server, as the pack records it: never with userinfo.
    pub server: String,
    /// The library root; staging goes under it.
    pub library: PathBuf,
    /// `<library>/.grid-staging/backup-<entry id>/`, set when the download
    /// starts.
    pub staging: PathBuf,
    /// What the download phase left for the write phase.
    pub staged: Option<Staged>,
    /// Warnings collected on the way (an asset that could not be fetched, a
    /// firmware file that failed its check). Never fail the row.
    pub warnings: Vec<String>,
}

impl BackupJob {
    pub(crate) fn new(
        drive: DriveTarget,
        item: BackupItem,
        client: Arc<RommClient>,
        library: PathBuf,
    ) -> Self {
        let server = strip_userinfo(client.server_url());
        Self {
            drive,
            item,
            client,
            server,
            library,
            staging: PathBuf::new(),
            staged: None,
            warnings: Vec::new(),
        }
    }
}

/// One staged file and the name it takes on the drive.
#[derive(Clone, Debug)]
pub(crate) struct StagedFile {
    pub src: PathBuf,
    /// The file name inside the item's folder on the drive.
    pub name: String,
    /// The expected MD5, blank when unknown.
    pub md5: String,
}

/// Which art a staged asset is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssetKind {
    Cover,
    Fanart,
    Logo,
    Screenshot,
}

/// What the download phase staged.
pub(crate) enum Staged {
    Game {
        /// The entry, with every `path` still blank.
        game: Box<PackGame>,
        /// One file per `game.files`, in order.
        files: Vec<StagedFile>,
        /// A single-file game owns a file in the pack, not a folder.
        single: bool,
        assets: Vec<(AssetKind, StagedFile)>,
    },
    Emulator {
        entry: Box<PackEmulator>,
        primary: StagedFile,
        supplementals: Vec<StagedFile>,
    },
    Firmware {
        entry: PackFirmware,
        files: Vec<StagedFile>,
    },
}

/// `<library>/.grid-staging/backup-<id>`.
pub fn staging_dir(library: &Path, id: u64) -> PathBuf {
    library.join(STAGING_DIR).join(format!("backup-{id}"))
}

/// Deletes a job's staging folder, and the staging root when it is empty.
pub(crate) fn remove_staging(staging: &Path) {
    if staging.as_os_str().is_empty() {
        return;
    }
    if let Err(error) = fs::remove_dir_all(staging) {
        if error.kind() != io::ErrorKind::NotFound {
            tracing::warn!(path = %staging.display(), %error, "backup staging was not removed");
        }
    }
    if let Some(root) = staging.parent() {
        let _ = fs::remove_dir(root);
    }
}

type Progress<'a> = &'a mut (dyn FnMut(u64, u64, f64) + Send);

// --- download phase -----------------------------------------------------------

/// The download phase of one backup row: fetches the item into
/// `job.staging` and fills `job.staged`.
pub(crate) async fn download(
    service: &InstallService,
    job: &mut BackupJob,
    cancel: &AtomicBool,
    on_progress: Progress<'_>,
) -> Result<(), LibraryError> {
    fs::create_dir_all(&job.staging)?;
    match job.item.clone() {
        BackupItem::Game(item) => download_game(job, item.rom_id, cancel, on_progress).await,
        BackupItem::Emulator(item) => {
            download_emulator(service, job, &item, cancel, on_progress).await
        }
        BackupItem::Firmware(item) => download_firmware(job, &item, cancel, on_progress).await,
    }
}

async fn download_game(
    job: &mut BackupJob,
    rom_id: i64,
    cancel: &AtomicBool,
    on_progress: Progress<'_>,
) -> Result<(), LibraryError> {
    let detail = job.client.rom_detail(rom_id).await?;
    let (picked, single): (Vec<&RomFile>, bool) = match download_set(&detail)? {
        DownloadSet::Single(file) => (vec![file], true),
        DownloadSet::Multi { files, .. } => (files, false),
        DownloadSet::Native { archive, sidecar } => {
            (std::iter::once(archive).chain(sidecar).collect(), false)
        }
    };
    let files_dir = job.staging.join("files");
    fs::create_dir_all(&files_dir)?;
    let mut targets = Vec::new();
    let mut staged = Vec::new();
    for (index, file) in picked.iter().enumerate() {
        // Staged under its position: two server names that sanitize alike
        // must not overwrite each other.
        let dest = files_dir.join(format!("{index}"));
        let size = if file.file_size_bytes > 0 {
            file.file_size_bytes
        } else if single {
            detail.filesize_bytes
        } else {
            0
        };
        targets.push(content_target(detail.id, file, dest.clone(), size));
        staged.push(StagedFile {
            src: dest,
            name: file.file_name.clone(),
            md5: file.md5_hash.clone(),
        });
    }
    download_targets(&RommProvider(&job.client), &targets, cancel, on_progress).await?;

    let assets = download_assets(job, &detail, cancel).await?;
    let mut pack_files = Vec::new();
    let mut size: u64 = 0;
    for (file, staged) in picked.iter().zip(&staged) {
        let len = fs::metadata(&staged.src)?.len();
        size = size.saturating_add(len);
        pack_files.push(PackFile {
            path: String::new(),
            size: len,
            server_file_id: file.id,
            name: file.file_name.clone(),
            category: file.category.clone(),
            top_level: file.is_top_level,
            md5: file.md5_hash.clone(),
            sha1: file.sha1_hash.clone(),
        });
    }
    let game = pack_game(&detail, &job.server, pack_files, size);
    job.staged = Some(Staged::Game {
        game: Box::new(game),
        files: staged,
        single,
        assets,
    });
    Ok(())
}

/// Fetches the game's art into staging: the cover (large, else small), the
/// first fanart, the logo and up to [`MAX_SCREENSHOTS`] screenshots. Every
/// URL is resolved against, and filtered to, the server's host, as
/// `ensure_image` does. A failed fetch is a warning, never an error; only a
/// cancel stops it.
async fn download_assets(
    job: &mut BackupJob,
    detail: &RomDetail,
    cancel: &AtomicBool,
) -> Result<Vec<(AssetKind, StagedFile)>, LibraryError> {
    let base = job.server.trim_end_matches('/').to_string();
    let resolve = server_resolver(&base);
    let filter = |url: &str| filter_to_server_host(url, &base);
    let cover = [&detail.cover_large_path, &detail.cover_small_path]
        .into_iter()
        .map(|path| resolve(path))
        .find(|url| !url.is_empty());
    let mut wanted: Vec<(AssetKind, String, String)> = Vec::new();
    if let Some(url) = cover {
        wanted.push((AssetKind::Cover, url, "cover".into()));
    }
    if let Some(url) = detail.fanart_urls.first() {
        wanted.push((AssetKind::Fanart, filter(url), "fanart".into()));
    }
    if !detail.logo_url.is_empty() {
        wanted.push((AssetKind::Logo, filter(&detail.logo_url), "logo".into()));
    }
    for (index, url) in detail
        .screenshot_urls
        .iter()
        .take(MAX_SCREENSHOTS)
        .enumerate()
    {
        wanted.push((
            AssetKind::Screenshot,
            filter(url),
            format!("screenshot-{:02}", index + 1),
        ));
    }
    let dir = job.staging.join("assets");
    fs::create_dir_all(&dir)?;
    let mut staged = Vec::new();
    for (kind, url, stem) in wanted {
        if cancel.load(Ordering::Relaxed) {
            return Err(LibraryError::Cancelled);
        }
        if url.is_empty() {
            continue;
        }
        match job.client.get_bytes_with_type(&url).await {
            Ok((bytes, content_type)) if !bytes.is_empty() => {
                let ext = extension_for(&url, &bytes, &content_type).ext;
                let name = format!("{stem}.{}", if ext.is_empty() { "img" } else { &ext });
                let src = dir.join(&name);
                fs::write(&src, &bytes)?;
                staged.push((
                    kind,
                    StagedFile {
                        src,
                        name,
                        md5: String::new(),
                    },
                ));
            }
            Ok(_) | Err(_) => job.warnings.push(format!(
                "The {} of {} could not be downloaded",
                asset_word(kind),
                detail.name
            )),
        }
    }
    Ok(staged)
}

fn asset_word(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Cover => "cover",
        AssetKind::Fanart => "fanart",
        AssetKind::Logo => "logo",
        AssetKind::Screenshot => "screenshot",
    }
}

/// A comma-joined `RomDetail` field as a list.
fn split_list(joined: &str) -> Vec<String> {
    joined
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// A release date in seconds as ISO `YYYY-MM-DD`; blank when none.
fn iso_date(seconds: &str) -> String {
    seconds
        .trim()
        .parse::<i64>()
        .ok()
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .map(|time| time.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// The pack entry of a backed-up game: everything needed to rebuild its
/// `RomDetail` ([`pack::pack_game_detail`]) and its image cache keys. Paths
/// stay blank until the write phase places the files.
pub(crate) fn pack_game(
    detail: &RomDetail,
    server: &str,
    files: Vec<PackFile>,
    size: u64,
) -> PackGame {
    PackGame {
        platform: detail.platform_slug.clone(),
        platform_name: detail.platform_name.clone(),
        source: PackSource {
            server: strip_userinfo(server),
            rom_id: detail.id,
            igdb_id: None,
            fs_name: detail.fs_name.clone(),
            server_updated_at: detail.server_updated_at.clone(),
            cover_small_path: detail.cover_small_path.clone(),
            cover_large_path: detail.cover_large_path.clone(),
            screenshot_urls: detail.screenshot_urls.clone(),
            fanart_urls: detail.fanart_urls.clone(),
            logo_url: detail.logo_url.clone(),
        },
        name: detail.name.clone(),
        summary: detail.description.clone(),
        region: detail.regions.clone(),
        languages: detail.languages.clone(),
        tags: detail.tags.clone(),
        revision: detail.revision.clone(),
        rating: detail.rating.clone(),
        companies: split_list(&detail.companies),
        release_date: iso_date(&detail.first_release_date),
        genres: split_list(&detail.genres),
        game_modes: split_list(&detail.game_modes),
        franchises: split_list(&detail.franchises),
        players: detail.player_count.clone(),
        version: String::new(),
        size,
        sha1: String::new(),
        files,
        assets: PackAssets::default(),
        added: pack::now_rfc3339(),
    }
}

async fn download_emulator(
    service: &InstallService,
    job: &mut BackupJob,
    item: &EmulatorItem,
    cancel: &AtomicBool,
    on_progress: Progress<'_>,
) -> Result<(), LibraryError> {
    let pick = &item.pick;
    let entry = |version: &str, asset_name: &str| PackEmulator {
        name: pick.name.clone(),
        kind: pick.kind.pack_kind().to_string(),
        source_id: pick.source_id(),
        version: version.to_string(),
        program: String::new(),
        args: String::new(),
        os: item.os.arch_tag().to_string(),
        platforms: pick.platforms.clone(),
        path: String::new(),
        supplementals: Vec::new(),
        asset_name: asset_name.to_string(),
        size: 0,
    };
    let staged = match &pick.source {
        EmulatorSource::Catalog { .. } => {
            let forge = service.forge()?;
            let (primary, supplementals) =
                resolve_catalog(service, &forge, &pick.name, item.os).await?;
            if cancel.load(Ordering::Relaxed) {
                return Err(LibraryError::Cancelled);
            }
            let dir = job.staging.join("emulator");
            fs::create_dir_all(&dir)?;
            let mut targets = Vec::new();
            let mut headers = Vec::new();
            let mut files = Vec::new();
            for (index, download) in std::iter::once(&primary)
                .chain(supplementals.iter())
                .enumerate()
            {
                let dest = dir.join(format!("{index}"));
                targets.push(FileTarget {
                    url_path: download.download_url.clone(),
                    query: Vec::new(),
                    dest: dest.clone(),
                    expected_size: download.size,
                });
                headers.push((download.download_url.clone(), download.provider == "github"));
                files.push(StagedFile {
                    src: dest,
                    name: download.asset_name.clone(),
                    md5: String::new(),
                });
            }
            let provider = ForgeProvider::new(&forge, headers);
            download_targets(&provider, &targets, cancel, on_progress).await?;
            let primary_file = files.remove(0);
            Staged::Emulator {
                entry: Box::new(entry(&primary.release_tag, &primary.asset_name)),
                primary: primary_file,
                supplementals: files,
            }
        }
        EmulatorSource::Server { source_id } => {
            let (request, size) = server_build(service, &job.client, source_id, item.os).await?;
            let dest = job.staging.join("emulator.bin");
            let target = FileTarget {
                url_path: format!(
                    "/api/roms/{}/content/{}",
                    request.rom_id,
                    encode_file_segment(&request.file_name)
                ),
                query: vec![("file_ids".to_string(), request.file_id.to_string())],
                dest: dest.clone(),
                expected_size: size,
            };
            download_targets(&RommProvider(&job.client), &[target], cancel, on_progress).await?;
            Staged::Emulator {
                entry: Box::new(entry(&request.version, &request.file_name)),
                primary: StagedFile {
                    src: dest,
                    name: request.file_name.clone(),
                    md5: String::new(),
                },
                supplementals: Vec::new(),
            }
        }
        EmulatorSource::Local { program, args } => {
            if PackOs::host() != Some(item.os) {
                return Err(LibraryError::Extract(format!(
                    "{} is only for {}",
                    pick.name,
                    PackOs::host().map(os_name).unwrap_or("this computer")
                )));
            }
            let name = format!("{}.zip", pack_component(&pick.name, "emulator"));
            let dest = job.staging.join(&name);
            let program = program.clone();
            let archive = dest.clone();
            let label = pick.name.clone();
            let cancel_flag = Arc::new(AtomicBool::new(cancel.load(Ordering::Relaxed)));
            let worker_flag = cancel_flag.clone();
            let zipping = tokio::task::spawn_blocking(move || {
                zip_program_folder(&label, &program, &archive, &worker_flag)
            });
            // The zip runs on the blocking pool; its flag follows the row's.
            tokio::pin!(zipping);
            let inside = loop {
                tokio::select! {
                    joined = &mut zipping => {
                        break joined.map_err(|e| LibraryError::Extract(format!("the archive did not finish: {e}")))??;
                    }
                    () = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        if cancel.load(Ordering::Relaxed) {
                            cancel_flag.store(true, Ordering::Relaxed);
                        }
                    }
                }
            };
            let size = fs::metadata(&dest)?.len();
            on_progress(size, size, 0.0);
            let mut entry = entry("local", &name);
            entry.program = inside;
            entry.args = args.clone();
            Staged::Emulator {
                entry: Box::new(entry),
                primary: StagedFile {
                    src: dest,
                    name,
                    md5: String::new(),
                },
                supplementals: Vec::new(),
            }
        }
    };
    if let Staged::Emulator {
        primary,
        supplementals,
        ..
    } = &staged
    {
        let size = std::iter::once(primary)
            .chain(supplementals)
            .filter_map(|file| fs::metadata(&file.src).ok())
            .map(|meta| meta.len())
            .sum::<u64>();
        service.record_emulator_size(&pick.name, item.os, size);
    }
    job.staged = Some(staged);
    Ok(())
}

/// The forge release (and supplementals) of the catalog profile `name` for
/// `os`.
pub(crate) async fn resolve_catalog(
    service: &InstallService,
    forge: &ForgeClient,
    name: &str,
    os: PackOs,
) -> Result<(ResolvedDownload, Vec<ResolvedDownload>), LibraryError> {
    // umu-launcher has no catalog profile: it resolves the source
    // `install_umu_launcher` uses, so the pack holds the same release.
    let raw = if name == crate::launch::umu::UMU_TITLE {
        service.umu_source.read().unwrap().clone()
    } else {
        let profile = service
            .profiles
            .iter()
            .find(|profile| profile.name == name)
            .ok_or_else(|| LibraryError::Registry(format!("unknown emulator: {name}")))?;
        profile
            .source
            .clone()
            .ok_or_else(|| LibraryError::Registry(format!("unknown emulator: {name}")))?
    };
    let host = os.forge_host();
    let primary = forge
        .resolve_for_host(&raw, name, host)
        .await
        .map_err(|e| LibraryError::Extract(e.0))?;
    let supplementals = forge
        .resolve_supplementals_for_host(&raw, name, host)
        .await
        .map_err(|e| LibraryError::Extract(e.0))?
        .into_iter()
        .map(|(_, download)| download)
        .collect();
    Ok((primary, supplementals))
}

/// The newest build of `source_id` (`romm:<folder>/<emulator>`) on the
/// server's Emulators platform for `os` on 64-bit x86, and its size (`0`
/// when the server does not say).
pub(crate) async fn server_build(
    service: &InstallService,
    client: &RommClient,
    source_id: &str,
    os: PackOs,
) -> Result<(super::ServerEmulatorRequest, i64), LibraryError> {
    let folder = source_id
        .strip_prefix("romm:")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
        .to_string();
    let missing = || {
        LibraryError::Extract(format!(
            "Your server no longer has a {} build of this emulator",
            os_name(os)
        ))
    };
    let platforms = client.platforms().await?;
    let shelf = platforms
        .iter()
        .find(|p| is_emulators_platform(p.label()) || is_emulators_platform(&p.name))
        .ok_or_else(missing)?;
    let mut package = None;
    for game in client.games(shelf.id).await? {
        let detail = client.rom_detail(game.id).await?;
        if detail.fs_name.trim().eq_ignore_ascii_case(&folder) {
            package = Some(detail);
            break;
        }
    }
    let detail = package.ok_or_else(missing)?;
    let packaged = ServerPackage {
        rom_id: detail.id,
        folder: detail.fs_name.clone(),
        files: detail
            .files
            .iter()
            .map(|file| (file.id, file.file_name.clone()))
            .collect(),
    };
    let config = Config::load(&service.config_path).unwrap_or_default();
    let request = newest_server_build(
        std::slice::from_ref(&packaged),
        &service.profiles,
        &config,
        os.forge_host(),
        Some(Cpu::X64),
        recommendations(),
        source_id,
    )
    .ok_or_else(missing)?;
    let size = detail
        .files
        .iter()
        .find(|file| file.id == request.file_id)
        .map_or(0, |file| file.file_size_bytes.max(0));
    Ok((request, size))
}

/// Zips the folder holding `program` into `archive` and returns the
/// program's path inside the archive (`/`-separated). Links are skipped,
/// never followed. Unix permissions are kept, so the program stays
/// executable. Checks `cancel` between files.
pub fn zip_program_folder(
    name: &str,
    program: &Path,
    archive: &Path,
    cancel: &AtomicBool,
) -> Result<String, LibraryError> {
    if let Some(reason) = crate::backup::plan::archive_refusal(name, program) {
        return Err(LibraryError::Extract(reason));
    }
    let folder = program
        .parent()
        .ok_or_else(|| LibraryError::Extract(format!("{name} can't be copied")))?;
    let inside = program
        .strip_prefix(folder)
        .map(rel_text)
        .map_err(|_| LibraryError::Extract(format!("{name} can't be copied")))?;
    let out = fs::File::create(archive)?;
    let mut zip = zip::ZipWriter::new(out);
    let mut stack = vec![folder.to_path_buf()];
    let mut buf = vec![0u8; 1024 * 1024];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            if cancel.load(Ordering::Relaxed) {
                drop(zip);
                let _ = fs::remove_file(archive);
                return Err(LibraryError::Cancelled);
            }
            let meta = path.symlink_metadata()?;
            let rel = rel_text(path.strip_prefix(folder).unwrap_or(&path));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .large_file(meta.len() > u64::from(u32::MAX) / 2)
                .unix_permissions(unix_mode(&meta));
            if meta.is_dir() {
                zip.add_directory(format!("{rel}/"), options)
                    .map_err(zip_error)?;
                stack.push(path);
            } else if meta.is_file() {
                zip.start_file(rel, options).map_err(zip_error)?;
                let mut src = fs::File::open(&path)?;
                loop {
                    let n = src.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    zip.write_all(&buf[..n])?;
                }
            }
        }
    }
    zip.finish().map_err(zip_error)?;
    Ok(inside)
}

fn rel_text(rel: &Path) -> String {
    rel.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(unix)]
fn unix_mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn unix_mode(meta: &fs::Metadata) -> u32 {
    if meta.is_dir() {
        0o755
    } else {
        0o644
    }
}

fn zip_error(error: zip::result::ZipError) -> LibraryError {
    LibraryError::Extract(format!(
        "the emulator archive could not be written: {error}"
    ))
}

async fn download_firmware(
    job: &mut BackupJob,
    item: &FirmwareItem,
    cancel: &AtomicBool,
    on_progress: Progress<'_>,
) -> Result<(), LibraryError> {
    let records = job.client.firmware(item.platform_id).await?;
    if records.is_empty() {
        return Err(LibraryError::Extract(format!(
            "Your server has no firmware for {}",
            item.platform_name
        )));
    }
    let total: u64 = records.iter().map(|record| record.file_size_bytes).sum();
    let dir = job.staging.join("firmware");
    fs::create_dir_all(&dir)?;
    let mut done: u64 = 0;
    let mut files = Vec::new();
    let mut entry_files = Vec::new();
    let started = std::time::Instant::now();
    for (index, record) in records.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(LibraryError::Cancelled);
        }
        let bytes = match job
            .client
            .firmware_bytes(record.id, &record.file_name)
            .await
        {
            Ok(bytes) => bytes,
            Err(error) => {
                job.warnings.push(format!(
                    "Firmware {} was not backed up: {error}",
                    record.file_name
                ));
                continue;
            }
        };
        if let Err(warning) = crate::firmware::verify_download(record, &bytes) {
            job.warnings.push(warning);
            continue;
        }
        let src = dir.join(format!("{index}"));
        fs::write(&src, &bytes)?;
        done = done.saturating_add(bytes.len() as u64);
        let speed = done as f64 / started.elapsed().as_secs_f64().max(0.001);
        on_progress(done, total.max(done), speed);
        entry_files.push(PackFirmwareFile {
            name: record.file_name.clone(),
            path: String::new(),
            size: bytes.len() as u64,
            md5: record.md5_hash.clone(),
        });
        files.push(StagedFile {
            src,
            name: record.file_name.clone(),
            md5: record.md5_hash.clone(),
        });
    }
    if files.is_empty() {
        return Err(LibraryError::Extract(format!(
            "No {} firmware could be downloaded",
            item.platform_name
        )));
    }
    job.staged = Some(Staged::Firmware {
        entry: PackFirmware {
            platform: item.slug.clone(),
            platform_name: item.platform_name.clone(),
            files: entry_files,
        },
        files,
    });
    Ok(())
}

// --- write phase --------------------------------------------------------------

/// The write phase of one backup row: re-reads the drive's index, skips an
/// item that is already there ("Already on <name>"), writes every staged
/// file through [`drive_write::write_file`], then records the item in the
/// index under the drive's lock. A file written before a failure is removed
/// again, so an item is either fully indexed or absent.
///
/// `before_file` runs before each file (a test hook).
pub(crate) fn write(
    job: &BackupJob,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
    before_file: &dyn Fn(),
    warnings: &mut Vec<String>,
) -> Result<(), LibraryError> {
    let drive = &job.drive;
    let mount = drive.mount.as_path();
    let pack = match pack::read_pack(mount) {
        Ok(PackState::Ready(pack)) => pack,
        Ok(PackState::NewerFormat(_)) => {
            return Err(LibraryError::Extract(format!(
                "{} was set up by a newer GRID; it can't be written to",
                drive.label
            )))
        }
        Ok(PackState::Absent) | Err(PackError::Io(_)) if drive_gone(mount) => {
            return Err(removed(drive))
        }
        Ok(PackState::Absent) => {
            return Err(LibraryError::Extract(format!(
                "{} is not set up for backups",
                drive.label
            )))
        }
        Ok(PackState::Unreadable(why)) => return Err(LibraryError::Extract(why)),
        Err(error) => return Err(pack_error(error, drive)),
    };
    let Some(staged) = &job.staged else {
        return Err(LibraryError::Extract("nothing was downloaded".to_string()));
    };
    let already = || format!("Already on {}", pack.name);

    // Place every file, then write them.
    let mut writes: Vec<(&StagedFile, String)> = Vec::new();
    let entry: Entry = match staged {
        Staged::Game {
            game,
            files,
            single,
            assets,
        } => {
            if pack.has_game(&game.source.server, game.source.rom_id) {
                warnings.push(already());
                return Ok(());
            }
            let mut game = (**game).clone();
            let server = game.source.server.clone();
            let rom_id = game.source.rom_id;
            let leaf = if game.source.fs_name.trim().is_empty() {
                game.name.clone()
            } else {
                game.source.fs_name.clone()
            };
            let stem;
            if *single {
                let candidate = game_dir_rel(&game.platform, &leaf, Some(&files[0].name));
                let rel = unique_rel(&pack, &candidate, &server, rom_id, true);
                stem = Path::new(&rel)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| leaf.clone());
                game.files[0].path = rel.clone();
                writes.push((&files[0], rel));
            } else {
                let candidate = game_dir_rel(&game.platform, &leaf, None);
                let dir = unique_rel(&pack, &candidate, &server, rom_id, false);
                stem = dir.rsplit('/').next().unwrap_or(&leaf).to_string();
                for (file, pack_file) in files.iter().zip(game.files.iter_mut()) {
                    let rel = game_file_rel(&dir, &file.name);
                    pack_file.path = rel.clone();
                    writes.push((file, rel));
                }
            }
            let asset_dir = unique_rel(
                &pack,
                &asset_dir_rel(&game.platform, &stem),
                &server,
                rom_id,
                false,
            );
            for (kind, file) in assets {
                let rel = format!("{asset_dir}/{}", pack_component(&file.name, "image"));
                match kind {
                    AssetKind::Cover => game.assets.cover = Some(rel.clone()),
                    AssetKind::Fanart => game.assets.fanart = Some(rel.clone()),
                    AssetKind::Logo => game.assets.logo = Some(rel.clone()),
                    AssetKind::Screenshot => game.assets.screenshots.push(rel.clone()),
                }
                writes.push((file, rel));
            }
            Entry::Game(Box::new(game))
        }
        Staged::Emulator {
            entry,
            primary,
            supplementals,
        } => {
            if pack
                .emulators
                .iter()
                .any(|have| have.name == entry.name && have.os == entry.os)
            {
                warnings.push(already());
                return Ok(());
            }
            let os = match entry.os.as_str() {
                tag if tag == PackOs::Windows.arch_tag() => PackOs::Windows,
                _ => PackOs::Linux,
            };
            let dir = emulator_dir_rel(&entry.name, &entry.version, os);
            let mut entry = (**entry).clone();
            let rel = format!("{dir}/{}", pack_component(&primary.name, "emulator"));
            entry.path = rel.clone();
            writes.push((primary, rel));
            for file in supplementals {
                let rel = format!("{dir}/{}", pack_component(&file.name, "supplemental"));
                entry.supplementals.push(rel.clone());
                writes.push((file, rel));
            }
            entry.size = std::iter::once(primary)
                .chain(supplementals)
                .filter_map(|file| fs::metadata(&file.src).ok())
                .map(|meta| meta.len())
                .sum();
            Entry::Emulator(entry)
        }
        Staged::Firmware { entry, files } => {
            if pack
                .firmware
                .iter()
                .any(|have| have.platform == entry.platform && !have.files.is_empty())
            {
                warnings.push(already());
                return Ok(());
            }
            let dir = firmware_dir_rel(&entry.platform);
            let mut entry = entry.clone();
            for (file, pack_file) in files.iter().zip(entry.files.iter_mut()) {
                let rel = format!("{dir}/{}", pack_component(&file.name, "firmware"));
                pack_file.path = rel.clone();
                writes.push((file, rel));
            }
            Entry::Firmware(entry)
        }
    };

    let total: u64 = writes
        .iter()
        .filter_map(|(file, _)| fs::metadata(&file.src).ok())
        .map(|meta| meta.len())
        .sum();
    let target = WriteTarget {
        mount,
        label: &drive.label,
        fat32: drive.fat32,
    };
    let mut done: u64 = 0;
    let mut written: Vec<String> = Vec::new();
    let mut outcome = Ok(());
    for (file, rel) in &writes {
        before_file();
        let md5 = (!file.md5.trim().is_empty()).then_some(file.md5.as_str());
        let result = drive_write::write_file(
            &target,
            &FileWrite {
                src: &file.src,
                rel,
                md5,
            },
            cancel,
            &mut |bytes, _| progress(done + bytes, total),
        );
        match result {
            Ok(bytes) => {
                done += bytes;
                written.push(rel.clone());
            }
            Err(error) => {
                outcome = Err(write_error(error, drive));
                break;
            }
        }
    }
    if outcome.is_ok() {
        let edited = pack::edit_pack(mount, |pack| match &entry {
            Entry::Game(game) => {
                pack.upsert_game((**game).clone());
            }
            Entry::Emulator(emulator) => {
                pack.upsert_emulator(emulator.clone());
            }
            Entry::Firmware(firmware) => {
                pack.upsert_firmware(firmware.clone());
            }
        });
        if let Err(error) = edited {
            outcome = Err(pack_error(error, drive));
        }
    }
    if outcome.is_err() && !drive_gone(mount) {
        for rel in &written {
            let _ = drive_write::remove_rel(mount, rel);
        }
    }
    outcome
}

/// The item a write records in the index.
enum Entry {
    Game(Box<PackGame>),
    Emulator(PackEmulator),
    Firmware(PackFirmware),
}

/// "<label> was removed".
pub(crate) fn removed(drive: &DriveTarget) -> LibraryError {
    LibraryError::DriveRemoved(format!("{} was removed", drive.label))
}

/// The drive is gone: its mount folder or its index no longer exists.
fn drive_gone(mount: &Path) -> bool {
    !mount.is_dir() || !pack::index_path(mount).is_file()
}

/// A drive write error as the row shows it, naming the drive.
fn write_error(error: WriteError, drive: &DriveTarget) -> LibraryError {
    match error {
        WriteError::DriveRemoved => removed(drive),
        WriteError::Pack(_) if drive_gone(&drive.mount) => removed(drive),
        WriteError::NoSpace => {
            LibraryError::Extract(format!("There isn't enough free space on {}", drive.label))
        }
        WriteError::Cancelled => LibraryError::Cancelled,
        other => LibraryError::Extract(other.to_string()),
    }
}

fn pack_error(error: PackError, drive: &DriveTarget) -> LibraryError {
    if drive_gone(&drive.mount) {
        removed(drive)
    } else {
        LibraryError::Extract(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_dates_are_iso_days() {
        assert_eq!(iso_date("1234567890"), "2009-02-13");
        assert_eq!(iso_date(""), "");
        assert_eq!(iso_date("soon"), "");
    }

    #[test]
    fn lists_split_on_commas() {
        assert_eq!(split_list("RPG, Action,,  "), vec!["RPG", "Action"]);
    }

    #[test]
    fn a_manual_emulator_folder_zips_with_its_program_path() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("pico");
        fs::create_dir_all(folder.join("data")).unwrap();
        let program = folder.join("pico8");
        fs::write(&program, b"#!/bin/sh\necho hi\n").unwrap();
        fs::write(folder.join("data/carts.txt"), b"carts").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let archive = tmp.path().join("pico.zip");
        let inside =
            zip_program_folder("PICO-8", &program, &archive, &AtomicBool::new(false)).unwrap();
        assert_eq!(inside, "pico8");
        let mut zip = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
        let mut names: Vec<String> = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().unwrap().to_string())
            .collect();
        names.sort();
        assert_eq!(names, vec!["data/", "data/carts.txt", "pico8"]);
        let entry = zip.by_name("pico8").unwrap();
        #[cfg(unix)]
        assert_eq!(entry.unix_mode().unwrap() & 0o777, 0o755);
        drop(entry);
    }

    #[test]
    fn a_program_under_usr_is_not_zipped() {
        let tmp = tempfile::tempdir().unwrap();
        let error = zip_program_folder(
            "Sys",
            Path::new("/usr/bin/sh"),
            &tmp.path().join("x.zip"),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Sys can't be copied (installed by the system)"
        );
        assert!(!tmp.path().join("x.zip").exists());
    }
}
