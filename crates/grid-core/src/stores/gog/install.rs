//! gogdl installs: `gogdl info` (install options), the install, update,
//! repair and DLC job runners, DLC removal and uninstall.
//!
//! A job's download half runs `gogdl info` (version, build, sizes) and then
//! the transfer; success is exit 0 **and** gogdl's manifest rewritten (a
//! fatal worker error exits 0 without writing it). The manifest is
//! `<tool dir>/heroic_gogdl/manifests/<id>` for a Windows (content-system)
//! build and `<game dir>/.gogdl-linux-manifest` for a Linux installer.
//!
//! A fresh install runs `gogdl update --path <game dir>` rather than
//! `download --path <parent>`: `download` appends GOG's own folder name,
//! while `update` with no stored manifest downloads everything into the
//! exact folder the user picked. A stale manifest from an earlier install
//! is deleted first, so the update is a full download.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde_json::Value;

use super::auth;
use super::goggame::{find_info_files, parse_info, GogGameInfo, PlayTask};
use super::meta::{GogDlcMeta, GogMeta, GogOrigin, GogPlatform};
use super::row::gog_row;
use super::{
    library_error, tool_exit_error, GogDlcOption, GogInstallOptions, GogInstallRequest, GogJobCtx,
    GogService, GogVersionOption,
};
use crate::library::queue::{JobPhase, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use crate::stores::progress::ProgressMerger;
use crate::stores::runner::run_tool;
use crate::stores::{StoreError, StoreKind};

/// The Linux installer manifest gogdl writes in the game folder.
pub const LINUX_MANIFEST: &str = ".gogdl-linux-manifest";
/// The Wine prefix folder inside a Windows game's folder (as RomM native
/// games and drive restores use).
pub const PREFIX_DIR: &str = crate::backup::local::WINE_PREFIX_DIR;
/// How long an Inno uninstaller may run before GRID stops it.
const UNINSTALLER_LIMIT: Duration = Duration::from_secs(120);

// --- argv builders ----------------------------------------------------------

/// gogdl's full argv for a subcommand: `--auth-config-path <file>` first
/// (after the subcommand gogdl silently ignores it), then `sub`.
pub fn gogdl_argv(auth_file: &Path, sub: Vec<String>) -> Vec<String> {
    let mut argv = Vec::with_capacity(sub.len() + 2);
    argv.push("--auth-config-path".to_string());
    argv.push(auth_file.to_string_lossy().into_owned());
    argv.extend(sub);
    argv
}

/// gogdl's `--platform` value.
pub fn os_name(platform: GogPlatform) -> &'static str {
    match platform {
        GogPlatform::Linux => "linux",
        GogPlatform::Windows => "windows",
    }
}

/// `info <id> --platform <os> --with-dlcs [--lang <lang>]`.
pub fn info_args(id: &str, platform: GogPlatform, lang: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "info".to_string(),
        id.trim().to_string(),
        "--platform".to_string(),
        os_name(platform).to_string(),
        "--with-dlcs".to_string(),
    ];
    if let Some(lang) = lang.map(str::trim).filter(|l| !l.is_empty()) {
        args.extend(["--lang".to_string(), lang.to_string()]);
    }
    args
}

/// What a transfer does: gogdl's subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    /// A fresh install or an update (`update`; see the module notes).
    Update,
    /// `repair`: verify and fix.
    Repair,
}

/// `update|repair <id> --path <dir> --support <dir> --platform <os>
/// [--lang <l>] (--with-dlcs --dlcs a,b | --skip-dlcs)`.
pub fn transfer_args(
    transfer: Transfer,
    id: &str,
    dir: &Path,
    support: &Path,
    platform: GogPlatform,
    lang: &str,
    dlc_ids: &[String],
) -> Vec<String> {
    let mut args = vec![
        match transfer {
            Transfer::Update => "update",
            Transfer::Repair => "repair",
        }
        .to_string(),
        id.trim().to_string(),
        "--path".to_string(),
        dir.to_string_lossy().into_owned(),
        "--support".to_string(),
        support.to_string_lossy().into_owned(),
        "--platform".to_string(),
        os_name(platform).to_string(),
    ];
    if !lang.trim().is_empty() {
        args.extend(["--lang".to_string(), lang.trim().to_string()]);
    }
    let dlcs: Vec<&str> = dlc_ids
        .iter()
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
        .collect();
    if dlcs.is_empty() {
        args.push("--skip-dlcs".to_string());
    } else {
        args.extend([
            "--with-dlcs".to_string(),
            "--dlcs".to_string(),
            dlcs.join(","),
        ]);
    }
    args
}

// --- gogdl info -------------------------------------------------------------

/// Download and disk bytes, per language key (`*` = every language).
pub type Sizes = BTreeMap<String, (u64, u64)>;

/// What `gogdl info` printed, either shape.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoOutput {
    pub sizes: Sizes,
    pub dlcs: Vec<InfoDlc>,
    pub languages: Vec<String>,
    pub build_id: String,
    pub version_name: String,
    pub folder_name: String,
}

/// One DLC `gogdl info` lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoDlc {
    pub id: String,
    pub title: String,
    pub sizes: Sizes,
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn pair(value: &Value) -> Option<(u64, u64)> {
    let download = value.get("download_size").and_then(Value::as_u64);
    let disk = value.get("disk_size").and_then(Value::as_u64);
    (download.is_some() || disk.is_some()).then(|| (download.unwrap_or(0), disk.unwrap_or(0)))
}

/// A `size` object (`{"*": {...}, "en-US": {...}}`) or a flat object.
fn sizes_of(value: Option<&Value>) -> Sizes {
    let mut sizes = Sizes::new();
    let Some(value) = value else {
        return sizes;
    };
    if let Some(flat) = pair(value) {
        sizes.insert("*".to_string(), flat);
        return sizes;
    }
    if let Some(map) = value.as_object() {
        for (key, entry) in map {
            if let Some(found) = pair(entry) {
                sizes.insert(key.clone(), found);
            }
        }
    }
    sizes
}

/// The download and disk bytes for `lang`: `*` plus that language (or,
/// with no language, `*` plus `en-US`, else the first language listed).
pub fn size_for(sizes: &Sizes, lang: Option<&str>) -> (u64, u64) {
    let base = sizes.get("*").copied().unwrap_or((0, 0));
    let extra = match lang {
        Some(lang) => sizes.get(lang).copied(),
        None => sizes
            .get("en-US")
            .or_else(|| {
                sizes
                    .iter()
                    .find(|(k, _)| k.as_str() != "*")
                    .map(|(_, v)| v)
            })
            .copied(),
    }
    .unwrap_or((0, 0));
    (base.0 + extra.0, base.1 + extra.1)
}

/// Parses gogdl's `info` stdout: the v2 shape (`size` per language,
/// `buildId`, `versionName`, DLC with sizes) and the flat Linux shape
/// (`download_size`, `disk_size`, DLC without sizes). The last JSON line
/// wins. `None` when no line is an info object.
pub fn parse_info_output(stdout: &str) -> Option<InfoOutput> {
    let value = stdout
        .lines()
        .rev()
        .map(str::trim)
        .filter(|l| l.starts_with('{'))
        .find_map(|l| serde_json::from_str::<Value>(l).ok())?;
    let object = value.as_object()?;
    let mut sizes = sizes_of(object.get("size"));
    if sizes.is_empty() {
        if let Some(flat) = pair(&value) {
            sizes.insert("*".to_string(), flat);
        }
    }
    if sizes.is_empty() && !object.contains_key("languages") && !object.contains_key("dlcs") {
        return None;
    }
    let dlcs = object
        .get("dlcs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| InfoDlc {
                    id: text(item.get("id")),
                    title: text(item.get("title")),
                    sizes: sizes_of(item.get("size")),
                })
                .filter(|d| !d.id.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let languages = object
        .get("languages")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|l| text(Some(l)))
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some(InfoOutput {
        sizes,
        dlcs,
        languages,
        build_id: text(object.get("buildId")),
        version_name: text(object.get("versionName")),
        folder_name: text(object.get("folder_name")),
    })
}

/// The language GRID picks when the user has not: `en-US`, else `en`,
/// else the first.
fn default_language(languages: &[String]) -> String {
    ["en-US", "en"]
        .iter()
        .find_map(|want| languages.iter().find(|l| l.as_str() == *want))
        .or_else(|| languages.first())
        .cloned()
        .unwrap_or_default()
}

/// The install options from each platform's `info`.
pub fn options_from(infos: &[(GogPlatform, InfoOutput)]) -> GogInstallOptions {
    let mut languages: Vec<String> = Vec::new();
    for (_, info) in infos {
        for lang in &info.languages {
            if !languages.contains(lang) {
                languages.push(lang.clone());
            }
        }
    }
    let default = default_language(&languages);
    let lang = (!default.is_empty()).then_some(default.as_str());
    let versions = infos
        .iter()
        .map(|(platform, info)| {
            let (download_bytes, disk_bytes) = size_for(&info.sizes, lang);
            GogVersionOption {
                platform: *platform,
                download_bytes,
                disk_bytes,
            }
        })
        .collect();
    // DLC sizes come from the content-system (Windows) answer when there is
    // one; the Linux answer has none.
    let source = infos
        .iter()
        .find(|(p, _)| *p == GogPlatform::Windows)
        .or_else(|| infos.first());
    let mut dlc: Vec<GogDlcOption> = source
        .map(|(_, info)| {
            info.dlcs
                .iter()
                .map(|d| GogDlcOption {
                    id: d.id.clone(),
                    title: d.title.clone(),
                    download_bytes: size_for(&d.sizes, lang).0,
                })
                .collect()
        })
        .unwrap_or_default();
    for (_, info) in infos {
        for d in &info.dlcs {
            if !dlc.iter().any(|x| x.id == d.id) {
                dlc.push(GogDlcOption {
                    id: d.id.clone(),
                    title: d.title.clone(),
                    download_bytes: size_for(&d.sizes, lang).0,
                });
            }
        }
    }
    GogInstallOptions {
        versions,
        languages,
        default_language: default,
        dlc,
    }
}

// --- jobs -------------------------------------------------------------------

/// Everything one GOG job needs.
struct GogJob {
    svc: GogService,
    ctx: GogJobCtx,
    op: StoreOp,
    store_id: String,
    title: String,
    platform: GogPlatform,
    language: String,
    /// Every DLC the game should have after the job.
    dlc_ids: Vec<String>,
    install_dir: PathBuf,
    compat_tool: String,
    old_row: Option<InstalledGame>,
    /// What the download half learned (`gogdl info`).
    fetched: Mutex<Option<InfoOutput>>,
}

/// The install job for `req`.
pub fn install_job(svc: GogService, req: GogInstallRequest, ctx: GogJobCtx) -> StoreJob {
    let job = GogJob {
        svc,
        ctx,
        op: StoreOp::Install,
        store_id: req.store_id.trim().to_string(),
        title: req.title.clone(),
        platform: req.platform,
        language: req.language,
        dlc_ids: req.dlc_ids,
        install_dir: req.install_dir,
        compat_tool: req.compat_tool,
        old_row: None,
        fetched: Mutex::new(None),
    };
    store_job(job)
}

/// The update, repair or DLC (`dlc_id`) job for an installed row.
pub fn row_job(
    svc: GogService,
    row: &InstalledGame,
    op: StoreOp,
    dlc_id: Option<&str>,
    ctx: GogJobCtx,
) -> StoreJob {
    let meta = GogMeta::from_json(&row.store_meta);
    let mut dlc_ids: Vec<String> = meta.dlc.iter().map(|d| d.id.clone()).collect();
    if let Some(id) = dlc_id.map(str::trim).filter(|d| !d.is_empty()) {
        if !dlc_ids.iter().any(|d| d == id) {
            dlc_ids.push(id.to_string());
        }
    }
    let job = GogJob {
        svc,
        ctx,
        op,
        store_id: row.store_id.trim().to_string(),
        title: row.title.clone(),
        platform: meta.platform,
        language: meta.language.clone(),
        dlc_ids,
        install_dir: game_dir(row),
        compat_tool: row.native_compat_tool.clone(),
        old_row: Some(row.clone()),
        fetched: Mutex::new(None),
    };
    store_job(job)
}

fn store_job(job: GogJob) -> StoreJob {
    StoreJob {
        source: StoreKind::Gog,
        store_id: job.store_id.clone(),
        op: job.op,
        title: job.title.clone(),
        drive: None,
        runner: Arc::new(job),
    }
}

/// A row's game folder.
pub fn game_dir(row: &InstalledGame) -> PathBuf {
    let dir = if row.native_game_dir.trim().is_empty() {
        row.extracted_dir.trim()
    } else {
        row.native_game_dir.trim()
    };
    PathBuf::from(dir)
}

/// The manifest gogdl rewrites when a transfer of `id` succeeds.
pub fn manifest_path(svc: &GogService, platform: GogPlatform, id: &str, dir: &Path) -> PathBuf {
    match platform {
        GogPlatform::Windows => svc.manifests_dir().join(id.trim()),
        GogPlatform::Linux => dir.join(LINUX_MANIFEST),
    }
}

/// Whether `path` was written at or after `since`.
pub fn written_since(path: &Path, since: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|modified| modified >= since)
}

impl GogJob {
    fn transfer(&self) -> Transfer {
        if self.op == StoreOp::Repair {
            Transfer::Repair
        } else {
            Transfer::Update
        }
    }

    fn run_info(&self, cancel: &AtomicBool) -> Result<Option<InfoOutput>, LibraryError> {
        let lang = (!self.language.trim().is_empty()).then_some(self.language.as_str());
        let cmd = self
            .svc
            .gogdl(info_args(&self.store_id, self.platform, lang))
            .map_err(library_error)?;
        let exit = run_tool(&cmd, cancel, &mut |_| {}).map_err(library_error)?;
        if !exit.success() {
            return if self.op == StoreOp::Repair {
                Ok(None)
            } else {
                Err(library_error(tool_exit_error(exit.code)))
            };
        }
        Ok(auth::read_stdout(&exit.stdout, parse_info_output))
    }
}

impl StoreJobRunner for GogJob {
    fn download(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        let info = self.run_info(cancel)?;
        let lang = (!self.language.trim().is_empty()).then_some(self.language.as_str());
        let expected = info
            .as_ref()
            .map(|i| size_for(&i.sizes, lang).0)
            .unwrap_or(0);

        let manifest = manifest_path(&self.svc, self.platform, &self.store_id, &self.install_dir);
        if self.op == StoreOp::Install {
            // A manifest left by an earlier install would turn the
            // install into a patch against files that are not there.
            let _ = fs::remove_file(&manifest);
            let _ = fs::remove_file(self.install_dir.join(LINUX_MANIFEST));
        }
        if self.op == StoreOp::Repair && !manifest.is_file() {
            return Err(LibraryError::Extract(
                "GRID can't verify this game: gogdl has no record of its files. Reinstall it."
                    .to_string(),
            ));
        }
        fs::create_dir_all(&self.install_dir)?;
        let support = self.svc.support_dir(&self.store_id);
        fs::create_dir_all(&support)?;

        let cmd = self
            .svc
            .gogdl(transfer_args(
                self.transfer(),
                &self.store_id,
                &self.install_dir,
                &support,
                self.platform,
                &self.language,
                &self.dlc_ids,
            ))
            .map_err(library_error)?;
        let since = SystemTime::now() - Duration::from_secs(2);
        let mut merger = ProgressMerger::default();
        progress(0, expected, 0.0);
        let exit = run_tool(&cmd, cancel, &mut |report| {
            let merged = merger.push(report);
            let total = if merged.total > 0 {
                merged.total
            } else {
                expected
            };
            progress(merged.done, total, merged.speed_bps);
        })
        .map_err(library_error)?;
        if !exit.success() {
            return Err(library_error(tool_exit_error(exit.code)));
        }
        let ok = if self.op == StoreOp::Repair {
            // A repair that finds nothing broken leaves the manifest alone.
            manifest.is_file()
        } else {
            written_since(&manifest, since)
        };
        if !ok {
            return Err(LibraryError::Extract(
                "gogdl stopped before it finished. Try again.".to_string(),
            ));
        }
        *self.fetched.lock().unwrap() = info;
        Ok(())
    }

    fn finalize(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        sink.phase(JobPhase::Installing);
        sink.progress(0, 1);
        let info = self.fetched.lock().unwrap().clone();
        let game = game_info(
            &self.install_dir,
            &self.store_id,
            self.platform,
            &self.title,
        )
        .ok_or_else(|| {
            LibraryError::Extract(
                "The download has no GOG game info, so GRID can't launch it.".to_string(),
            )
        })?;
        let old_meta = self
            .old_row
            .as_ref()
            .map(|r| GogMeta::from_json(&r.store_meta));
        let product = self.svc.product(&self.store_id);
        let dlc_title = |id: &str| {
            info.as_ref()
                .and_then(|i| i.dlcs.iter().find(|d| d.id == id))
                .map(|d| d.title.clone())
                .filter(|t| !t.is_empty())
                .or_else(|| {
                    product
                        .as_ref()
                        .and_then(|p| p.dlc.iter().find(|d| d.id == id))
                        .map(|d| d.title.clone())
                })
                .or_else(|| {
                    old_meta
                        .as_ref()
                        .and_then(|m| m.dlc.iter().find(|d| d.id == id))
                        .map(|d| d.title.clone())
                })
                .unwrap_or_default()
        };
        let mut meta = old_meta.clone().unwrap_or(GogMeta {
            cloud_saves: true,
            ..Default::default()
        });
        if self.op == StoreOp::Install {
            meta.origin = GogOrigin::Gogdl;
            meta.installer.clear();
            meta.installer_bytes = 0;
        }
        meta.platform = self.platform;
        meta.language = self.language.clone();
        if let Some(info) = &info {
            if !info.version_name.is_empty() {
                meta.version = info.version_name.clone();
            }
            if !info.build_id.is_empty() {
                meta.build_id = info.build_id.clone();
            }
        }
        if meta.build_id.is_empty() && !game.build_id.is_empty() {
            meta.build_id = game.build_id.clone();
        }
        meta.dlc = self
            .dlc_ids
            .iter()
            .map(|id| GogDlcMeta {
                id: id.clone(),
                title: dlc_title(id),
                installer: Vec::new(),
            })
            .collect();

        let row = build_row(
            &game,
            &self.install_dir,
            &meta,
            product.as_ref(),
            self.old_row.as_ref(),
            &self.compat_tool,
        )?;
        self.ctx.registry.upsert_store(&row)?;
        sink.progress(1, 1);
        Ok(String::new())
    }
}

/// The row for a finished job: [`gog_row`], the Wine prefix and compat
/// tool, the user's pinned executable carried over, and the size.
fn build_row(
    game: &GogGameInfo,
    dir: &Path,
    meta: &GogMeta,
    product: Option<&super::GogProduct>,
    old: Option<&InstalledGame>,
    compat_tool: &str,
) -> Result<InstalledGame, LibraryError> {
    let prefix = if meta.platform == GogPlatform::Windows && !cfg!(windows) {
        Some(
            old.map(|r| r.native_wineprefix.trim().to_string())
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| dir.join(PREFIX_DIR)),
        )
    } else {
        None
    };
    if let Some(prefix) = &prefix {
        fs::create_dir_all(prefix)?;
    }
    let mut row = gog_row(game, dir, meta, product, prefix.as_deref());
    row.native_compat_tool = compat_tool.trim().to_string();
    if let Some(old) = old {
        if meta.exe_user_pinned && !old.native_executable_path.trim().is_empty() {
            row.native_executable_path = old.native_executable_path.clone();
            row.native_launch_parameters = old.native_launch_parameters.clone();
            row.native_working_dir = old.native_working_dir.clone();
        }
        if product.is_none() {
            row.title = old.title.clone();
            row.description = old.description.clone();
            row.genres = old.genres.clone();
            row.companies = old.companies.clone();
            row.first_release_date = old.first_release_date.clone();
            row.screenshot_urls = old.screenshot_urls.clone();
        }
        row.cover_small_path = old.cover_small_path.clone();
        row.cover_large_path = old.cover_large_path.clone();
        row.fanart_urls = old.fanart_urls.clone();
    }
    row.filesize_bytes = i64::try_from(dir_size(dir, prefix.as_deref())).unwrap_or(i64::MAX);
    Ok(row)
}

/// The game info for `id` in `dir`: its `goggame-<id>.info` (top level,
/// then `game/`), else the first base-game file. A Linux build launches
/// through the installer's `start.sh` when it has one, and gets a
/// synthesized info (named after `gameinfo`'s first line, else `title`)
/// when it has no info file at all.
pub fn game_info(dir: &Path, id: &str, platform: GogPlatform, title: &str) -> Option<GogGameInfo> {
    let read = |folder: &Path| -> Vec<GogGameInfo> {
        find_info_files(folder)
            .iter()
            .filter_map(|path| fs::read(path).ok())
            .filter_map(|bytes| parse_info(&bytes))
            .collect()
    };
    let mut infos = read(dir);
    if infos.is_empty() {
        infos = read(&dir.join("game"));
    }
    let found = infos
        .iter()
        .find(|i| i.game_id.trim() == id.trim())
        .or_else(|| infos.iter().find(|i| !i.is_dlc()))
        .cloned();
    if platform == GogPlatform::Linux {
        let start = dir.join("start.sh");
        if start.is_file() {
            let mut info = found.unwrap_or_else(|| GogGameInfo {
                game_id: id.trim().to_string(),
                root_game_id: id.trim().to_string(),
                name: linux_game_name(dir).unwrap_or_else(|| title.trim().to_string()),
                ..Default::default()
            });
            info.game_id = id.trim().to_string();
            info.play_tasks = vec![PlayTask {
                is_primary: true,
                path: "start.sh".to_string(),
                category: "game".to_string(),
                ..Default::default()
            }];
            return Some(info);
        }
    }
    found.map(|mut info| {
        info.game_id = id.trim().to_string();
        info
    })
}

/// The first line of a Linux installer's `gameinfo` (the game's name).
fn linux_game_name(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join("gameinfo")).ok()?;
    text.lines()
        .next()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
}

/// The bytes under `dir`, links not followed, `skip` (the prefix) left out.
pub fn dir_size(dir: &Path, skip: Option<&Path>) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if skip.is_some_and(|s| s == path) {
                return 0;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => dir_size(&path, skip),
                Ok(kind) if kind.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
                _ => 0,
            }
        })
        .sum()
}

// --- DLC removal and uninstall ---------------------------------------------

/// Removes one DLC: gogdl updates the game without it (its files go), then
/// the row's meta drops it. Blocking.
pub fn remove_dlc(
    svc: &GogService,
    registry: &Registry,
    row: &InstalledGame,
    dlc_id: &str,
) -> Result<(), StoreError> {
    let mut meta = GogMeta::from_json(&row.store_meta);
    let dlc_id = dlc_id.trim();
    if !meta.dlc.iter().any(|d| d.id == dlc_id) {
        return Err(StoreError::Refused("That DLC isn't installed.".to_string()));
    }
    meta.dlc.retain(|d| d.id != dlc_id);
    let dir = game_dir(row);
    if meta.origin == GogOrigin::Gogdl {
        let keep: Vec<String> = meta.dlc.iter().map(|d| d.id.clone()).collect();
        let support = svc.support_dir(&row.store_id);
        let cmd = svc.gogdl(transfer_args(
            Transfer::Update,
            &row.store_id,
            &dir,
            &support,
            meta.platform,
            &meta.language,
            &keep,
        ))?;
        let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {})?;
        if !exit.success() {
            return Err(tool_exit_error(exit.code));
        }
    }
    let mut updated = row.clone();
    updated.store_meta = meta.to_json();
    registry.upsert_store(&updated)?;
    Ok(())
}

/// Uninstalls a GOG game: a Windows build's Inno uninstaller (silently,
/// through the row's compat tool) when the folder has one, then the folder
/// through the library's removal guard, gogdl's manifest and support files,
/// and the row. A folder the guard refuses stays on disk (the row still
/// goes), as a RomM uninstall does.
pub fn uninstall(svc: &GogService, registry: &Registry, store_id: &str) -> Result<(), StoreError> {
    let row = registry
        .find_store(StoreKind::Gog, store_id)?
        .ok_or_else(|| StoreError::Refused("That game isn't installed.".to_string()))?;
    let meta = GogMeta::from_json(&row.store_meta);
    let dir = game_dir(&row);
    if meta.platform == GogPlatform::Windows {
        run_uninstaller(&row, &dir);
    }
    if !dir.as_os_str().is_empty() && dir.is_dir() {
        match removal_refusal(&row, &dir) {
            None => fs::remove_dir_all(&dir).map_err(|e| {
                StoreError::Refused(format!(
                    "Couldn't delete {} ({}).",
                    dir.to_string_lossy(),
                    e.kind()
                ))
            })?,
            Some(reason) => tracing::warn!(
                path = %dir.to_string_lossy(),
                reason,
                "GOG uninstall kept a folder the removal guard refused"
            ),
        }
    }
    let _ = fs::remove_file(svc.manifests_dir().join(row.store_id.trim()));
    let _ = fs::remove_dir_all(svc.support_dir(&row.store_id));
    registry.remove_store(StoreKind::Gog, &row.store_id)?;
    Ok(())
}

/// Why the removal guard keeps `dir`, `None` when it may go. The game's
/// parent folder stands in as a library root (so it and everything above
/// it are protected), beside every configured library root.
pub fn removal_refusal(row: &InstalledGame, dir: &Path) -> Option<&'static str> {
    let parent = dir.parent().unwrap_or(dir);
    let others = crate::config::Config::load(&crate::config::Config::default_path())
        .map(|config| crate::library::path_change::protected_library_roots(&config))
        .unwrap_or_default();
    let home = crate::platform::home_dir();
    let guard = crate::library::RemovalGuard::new(row, parent, &others, home.as_deref());
    guard.refusal(dir).map(|reason| reason.text())
}

/// Runs `unins000.exe /VERYSILENT` in the game folder, best effort:
/// natively on Windows, through the row's compat tool elsewhere (no compat
/// tool: skipped; the folder is deleted anyway). Stopped after
/// [`UNINSTALLER_LIMIT`].
fn run_uninstaller(row: &InstalledGame, dir: &Path) {
    let unins = dir.join("unins000.exe");
    if !unins.is_file() {
        return;
    }
    if !cfg!(windows) && row.native_compat_tool.trim().is_empty() {
        return;
    }
    let mut probe = row.clone();
    probe.native_executable_path = unins.to_string_lossy().into_owned();
    probe.native_launch_parameters = "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART".to_string();
    probe.native_working_dir = String::new();
    probe.native_launch_source = crate::library::specials::native::LAUNCH_SOURCE_USER.to_string();
    let managed = crate::config::Config::load(&crate::config::Config::default_path())
        .ok()
        .and_then(|config| crate::launch::umu::managed_umu_run(&config));
    let which = |name: &str| {
        crate::launch::compat::which_on_path(name).or_else(|| {
            (name == crate::launch::umu::UMU_RUN)
                .then(|| managed.clone())
                .flatten()
        })
    };
    let Ok(launch) = crate::launch::native::build_native_command(
        &probe,
        Path::new(""),
        "",
        std::env::consts::OS,
        &which,
        None,
    ) else {
        return;
    };
    let mut argv = launch.argv.into_iter();
    let Some(program) = argv.next() else {
        return;
    };
    let cmd = crate::stores::runner::ToolCommand {
        program: PathBuf::from(program),
        args: argv.collect(),
        env: launch.env,
        cwd: Some(launch.cwd),
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let cancel = &cancel;
        scope.spawn(move || {
            if done_rx.recv_timeout(UNINSTALLER_LIMIT).is_err() {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
        let _ = run_tool(&cmd, cancel, &mut |_| {});
        let _ = done_tx.send(());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_puts_the_auth_path_first() {
        let argv = gogdl_argv(
            Path::new("/d/auth.json"),
            info_args("123", GogPlatform::Windows, Some("de-DE")),
        );
        assert_eq!(
            argv,
            [
                "--auth-config-path",
                "/d/auth.json",
                "info",
                "123",
                "--platform",
                "windows",
                "--with-dlcs",
                "--lang",
                "de-DE"
            ]
        );
    }

    #[test]
    fn transfer_argv_for_dlc_and_none() {
        let args = transfer_args(
            Transfer::Update,
            "123",
            Path::new("/g/Game"),
            Path::new("/s/123"),
            GogPlatform::Linux,
            "en-US",
            &["9".into(), " 10 ".into()],
        );
        assert_eq!(
            args,
            [
                "update",
                "123",
                "--path",
                "/g/Game",
                "--support",
                "/s/123",
                "--platform",
                "linux",
                "--lang",
                "en-US",
                "--with-dlcs",
                "--dlcs",
                "9,10"
            ]
        );
        let none = transfer_args(
            Transfer::Repair,
            "1",
            Path::new("/g"),
            Path::new("/s"),
            GogPlatform::Windows,
            "",
            &[],
        );
        assert_eq!(none[0], "repair");
        assert_eq!(none.last().unwrap(), "--skip-dlcs");
        assert!(!none.contains(&"--lang".to_string()));
    }

    const V2: &str = r#"{"size": {"*": {"download_size": 1000, "disk_size": 2000}, "en-US": {"download_size": 10, "disk_size": 20}, "de-DE": {"download_size": 30, "disk_size": 40}}, "dlcs": [{"title": "Bonus", "id": "9", "size": {"*": {"download_size": 5, "disk_size": 6}, "en-US": {"download_size": 1, "disk_size": 1}}}], "buildId": "5639", "languages": ["en-US", "de-DE"], "folder_name": "The Witcher", "dependencies": [], "versionEtag": "x", "versionName": "1.5", "builds": {"items": []}}"#;
    const FLAT: &str = r#"{"download_size": 700, "disk_size": 900, "dlcs": [{"title": "Linux DLC", "id": 9, "languages": [["en"]]}], "languages": ["en-US"], "folder_name": "Witcher", "dependencies": [], "versionName": "2.1"}"#;

    #[test]
    fn info_parses_both_shapes() {
        let v2 = parse_info_output(&format!("[INFO] noise\n{V2}\n")).unwrap();
        assert_eq!(v2.build_id, "5639");
        assert_eq!(v2.version_name, "1.5");
        assert_eq!(v2.folder_name, "The Witcher");
        assert_eq!(size_for(&v2.sizes, Some("de-DE")), (1030, 2040));
        assert_eq!(size_for(&v2.sizes, None), (1010, 2020));
        assert_eq!(v2.dlcs[0].id, "9");
        assert_eq!(size_for(&v2.dlcs[0].sizes, Some("en-US")).0, 6);

        let flat = parse_info_output(FLAT).unwrap();
        assert_eq!(size_for(&flat.sizes, Some("en-US")), (700, 900));
        assert_eq!(flat.version_name, "2.1");
        assert_eq!(flat.build_id, "");
        assert_eq!(flat.dlcs[0].id, "9");
        assert!(flat.dlcs[0].sizes.is_empty());

        assert!(parse_info_output("null").is_none());
        assert!(parse_info_output("").is_none());
        assert!(parse_info_output(r#"{"error": true}"#).is_none());
    }

    #[test]
    fn options_merge_both_platforms() {
        let infos = vec![
            (GogPlatform::Windows, parse_info_output(V2).unwrap()),
            (GogPlatform::Linux, parse_info_output(FLAT).unwrap()),
        ];
        let options = options_from(&infos);
        assert_eq!(options.default_language, "en-US");
        assert_eq!(options.languages, ["en-US", "de-DE"]);
        assert_eq!(options.versions.len(), 2);
        assert_eq!(options.versions[0].download_bytes, 1010);
        assert_eq!(options.versions[1].platform, GogPlatform::Linux);
        assert_eq!(options.versions[1].disk_bytes, 900);
        assert_eq!(
            options.dlc.len(),
            1,
            "one DLC, sized from the Windows answer"
        );
        assert_eq!(options.dlc[0].download_bytes, 6);
    }

    #[test]
    fn a_linux_build_launches_start_sh() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("start.sh"), "#!/bin/sh\n").unwrap();
        fs::write(dir.path().join("gameinfo"), "Witcher Linux\n2.1\n").unwrap();
        let info = game_info(dir.path(), "123", GogPlatform::Linux, "Fallback").unwrap();
        assert_eq!(info.game_id, "123");
        assert_eq!(info.name, "Witcher Linux");
        assert_eq!(info.primary_task().unwrap().path, "start.sh");

        let win = tempfile::tempdir().unwrap();
        assert!(game_info(win.path(), "123", GogPlatform::Windows, "x").is_none());
        fs::write(
            win.path().join("goggame-123.info"),
            r#"{"gameId": "123", "name": "W", "playTasks": [{"isPrimary": true, "path": "w.exe", "type": "FileTask"}]}"#,
        )
        .unwrap();
        let info = game_info(win.path(), "123", GogPlatform::Windows, "x").unwrap();
        assert_eq!(info.primary_task().unwrap().path, "w.exe");
    }

    #[test]
    fn the_guard_protects_the_parent_and_allows_the_game() {
        let _lock = crate::test_env::lock();
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("GOG").join("Game");
        let row = InstalledGame {
            native_game_dir: dir.to_string_lossy().into_owned(),
            platform: "GOG".into(),
            ..Default::default()
        };
        assert_eq!(removal_refusal(&row, &dir), None);
        assert!(removal_refusal(&row, &root.path().join("GOG")).is_some());
        assert!(removal_refusal(&row, Path::new("/")).is_some());
    }

    #[test]
    fn dir_size_skips_the_prefix() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), [0u8; 10]).unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("b"), [0u8; 5]).unwrap();
        fs::create_dir_all(dir.path().join(PREFIX_DIR)).unwrap();
        fs::write(dir.path().join(PREFIX_DIR).join("c"), [0u8; 100]).unwrap();
        assert_eq!(dir_size(dir.path(), Some(&dir.path().join(PREFIX_DIR))), 15);
    }

    #[cfg(unix)]
    mod fake_gogdl {
        use super::*;
        use crate::library::queue::StoreProgressSink;
        use crate::stores::gog::{GogEndpoints, GogProduct, GogProductDlc};
        use std::os::unix::fs::PermissionsExt;

        /// A fake gogdl: answers `info` with the v2 JSON, and on
        /// `update`/`repair` writes progress, the game files and (unless
        /// `$FAKE_NO_MANIFEST` exists) the manifest, then exits
        /// `$(cat exit_code)` or 0. It records its argv.
        fn fake(dir: &Path) -> PathBuf {
            let script = dir.join("gogdl.sh");
            let body = format!(
                r#"#!/bin/sh
echo "$@" >> '{log}'
# skip --auth-config-path <file>
shift 2
cmd="$1"; id="$2"; shift 2
path=""
while [ $# -gt 0 ]; do
  case "$1" in --path) path="$2"; shift 2;; *) shift;; esac
done
case "$cmd" in
  info) printf '%s\n' '{v2}'; exit 0;;
  update|repair)
    echo '[PROGRESS] INFO: = Progress: 50.00 500/1000, Running for: 00:00:01, ETA: 00:00:01' >&2
    echo '[PROGRESS] INFO:  + Download	- 1.00 MiB/s (raw) / 1.00 MiB/s (decompressed)' >&2
    echo '[AUTH] INFO: FAKE-STORE-TOKEN-not-real' >&2
    mkdir -p "$path"
    printf '%s' '{{"gameId": "123", "name": "Fake Game", "buildId": "5639", "playTasks": [{{"isPrimary": true, "path": "bin\\game.exe", "type": "FileTask"}}]}}' > "$path/goggame-123.info"
    printf 'x' > "$path/data.bin"
    if [ ! -e '{no_manifest}' ]; then
      mkdir -p "$GOGDL_CONFIG_PATH/heroic_gogdl/manifests"
      printf '{{}}' > "$GOGDL_CONFIG_PATH/heroic_gogdl/manifests/$id"
    fi
    if [ -e '{code}' ]; then exit "$(cat '{code}')"; fi
    exit 0;;
esac
exit 9
"#,
                log = dir.join("argv.log").to_string_lossy(),
                v2 = V2,
                no_manifest = dir.join("no-manifest").to_string_lossy(),
                code = dir.join("exit_code").to_string_lossy(),
            );
            fs::write(&script, body).unwrap();
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
            script
        }

        struct Sink(Vec<JobPhase>);
        impl StoreProgressSink for Sink {
            fn progress(&mut self, _done: u64, _total: u64) {}
            fn phase(&mut self, phase: JobPhase) {
                self.0.push(phase);
            }
        }

        struct Env {
            _root: tempfile::TempDir,
            root: PathBuf,
            svc: GogService,
            ctx: GogJobCtx,
        }

        fn env() -> Env {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().to_path_buf();
            let svc = GogService::new(
                path.join("data"),
                reqwest::Client::new(),
                GogEndpoints::real(),
            )
            .with_tool_for_test(fake(&path));
            svc.set_owned_for_test(vec![GogProduct {
                id: "123".into(),
                title: "Fake Game: Store Title".into(),
                dlc: vec![GogProductDlc {
                    id: "9".into(),
                    title: "Bonus".into(),
                }],
                ..Default::default()
            }]);
            let registry = Arc::new(Registry::open(&path.join("reg.db")).unwrap());
            let ctx = GogJobCtx {
                registry,
                library: path.join("lib"),
                default_compat_tool: String::new(),
            };
            Env {
                _root: root,
                root: path,
                svc,
                ctx,
            }
        }

        fn request(env: &Env) -> GogInstallRequest {
            GogInstallRequest {
                store_id: "123".into(),
                title: "Fake Game".into(),
                platform: GogPlatform::Windows,
                language: "en-US".into(),
                dlc_ids: vec!["9".into()],
                install_dir: env.root.join("lib").join("GOG").join("Fake Game"),
                compat_tool: "GE-Proton".into(),
            }
        }

        #[test]
        fn an_install_downloads_reports_progress_and_writes_the_row() {
            let env = env();
            let job = env.svc.install_job(request(&env), env.ctx.clone());
            assert_eq!(job.op, StoreOp::Install);
            let mut reports = Vec::new();
            job.runner
                .download(&AtomicBool::new(false), &mut |d, t, s| {
                    reports.push((d, t, s))
                })
                .unwrap();
            assert!(reports.iter().any(|(d, t, _)| (*d, *t) == (500, 1000)));
            assert!(reports.iter().any(|(_, _, s)| *s > 0.0), "speed merged in");
            let mut sink = Sink(Vec::new());
            assert_eq!(job.runner.finalize(&mut sink).unwrap(), "");
            assert_eq!(sink.0, [JobPhase::Installing]);

            let row = env
                .ctx
                .registry
                .find_store(StoreKind::Gog, "123")
                .unwrap()
                .unwrap();
            assert_eq!(row.title, "Fake Game: Store Title");
            assert_eq!(row.platform_slug, "win");
            assert_eq!(row.native_compat_tool, "GE-Proton");
            assert!(row.native_executable_path.ends_with("game.exe"));
            if !cfg!(windows) {
                assert!(row.native_wineprefix.ends_with(PREFIX_DIR));
            }
            assert!(row.filesize_bytes > 0);
            let meta = GogMeta::from_json(&row.store_meta);
            assert_eq!(meta.origin, GogOrigin::Gogdl);
            assert_eq!(meta.version, "1.5");
            assert_eq!(meta.build_id, "5639");
            assert_eq!(meta.dlc[0].title, "Bonus");
            assert!(meta.cloud_saves);

            let argv = fs::read_to_string(env.root.join("argv.log")).unwrap();
            for line in argv.lines() {
                assert!(line.starts_with("--auth-config-path "), "{line}");
            }
            assert!(argv.contains("update 123 --path"));
            assert!(argv.contains("--with-dlcs --dlcs 9"));
        }

        #[test]
        fn exit_zero_without_a_manifest_is_a_failure() {
            let env = env();
            fs::write(env.root.join("no-manifest"), "").unwrap();
            let job = env.svc.install_job(request(&env), env.ctx.clone());
            let error = job
                .runner
                .download(&AtomicBool::new(false), &mut |_, _, _| {})
                .unwrap_err();
            assert!(error.to_string().contains("before it finished"), "{error}");
            assert!(!error.to_string().contains("FAKE-STORE"));
        }

        #[test]
        fn a_disk_full_exit_is_worded() {
            let env = env();
            fs::write(env.root.join("exit_code"), "2").unwrap();
            let job = env.svc.install_job(request(&env), env.ctx.clone());
            let error = job
                .runner
                .download(&AtomicBool::new(false), &mut |_, _, _| {})
                .unwrap_err();
            assert!(error.to_string().contains("disk space"), "{error}");
        }

        #[test]
        fn update_keeps_the_pinned_exe_and_uninstall_removes_everything() {
            let env = env();
            let job = env.svc.install_job(request(&env), env.ctx.clone());
            job.runner
                .download(&AtomicBool::new(false), &mut |_, _, _| {})
                .unwrap();
            job.runner.finalize(&mut Sink(Vec::new())).unwrap();
            let mut row = env
                .ctx
                .registry
                .find_store(StoreKind::Gog, "123")
                .unwrap()
                .unwrap();
            let mut meta = GogMeta::from_json(&row.store_meta);
            meta.exe_user_pinned = true;
            meta.last_sync = 77;
            row.store_meta = meta.to_json();
            row.native_executable_path = "/custom/run.exe".into();
            env.ctx.registry.upsert_store(&row).unwrap();

            let update = env.svc.update_job(&row, env.ctx.clone());
            assert_eq!(update.op, StoreOp::Update);
            update
                .runner
                .download(&AtomicBool::new(false), &mut |_, _, _| {})
                .unwrap();
            update.runner.finalize(&mut Sink(Vec::new())).unwrap();
            let after = env
                .ctx
                .registry
                .find_store(StoreKind::Gog, "123")
                .unwrap()
                .unwrap();
            assert_eq!(after.native_executable_path, "/custom/run.exe");
            assert_eq!(GogMeta::from_json(&after.store_meta).last_sync, 77);

            let repair = env.svc.repair_job(&after, env.ctx.clone());
            repair
                .runner
                .download(&AtomicBool::new(false), &mut |_, _, _| {})
                .unwrap();

            env.svc.remove_dlc(&env.ctx.registry, &after, "9").unwrap();
            let no_dlc = env
                .ctx
                .registry
                .find_store(StoreKind::Gog, "123")
                .unwrap()
                .unwrap();
            assert!(GogMeta::from_json(&no_dlc.store_meta).dlc.is_empty());
            let argv = fs::read_to_string(env.root.join("argv.log")).unwrap();
            assert!(argv.lines().last().unwrap().ends_with("--skip-dlcs"));

            let dir = game_dir(&no_dlc);
            assert!(dir.is_dir());
            {
                let _lock = crate::test_env::lock();
                env.svc.uninstall(&env.ctx.registry, "123").unwrap();
            }
            assert!(!dir.exists());
            assert!(!env.svc.manifests_dir().join("123").exists());
            assert!(env
                .ctx
                .registry
                .find_store(StoreKind::Gog, "123")
                .unwrap()
                .is_none());
        }
    }
}
