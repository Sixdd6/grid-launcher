//! legendary installs: the install options (`info --json`), the install,
//! update, repair and DLC job runners, DLC removal and uninstall.
//!
//! A job's download half runs one or more legendary transfers (an install
//! is the base game with `--skip-dlcs`, then each DLC the user picked; an
//! update or a repair is the base game, then each installed DLC). legendary
//! often exits 0 when it failed, so a transfer counts as done only when it
//! exited 0 with no failure signal ([`step_failure`]), and the job only
//! when `list-installed` then shows the game with the expected version
//! ([`check_installed`]). A transfer that runs out of shared memory is
//! retried once with `--max-shared-memory 5000`.
//!
//! Progress comes from [`crate::stores::progress::parse_legendary_line`]
//! as a fraction of [`LEGENDARY_PROGRESS_TOTAL`], turned into bytes with
//! `info`'s download sizes when GRID has them ([`overall_progress`]).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::list::{info_args, installed_args, parse_info, parse_installed, EpicInfo};
use super::meta::{EpicDlcMeta, EpicMeta};
use super::row::epic_row;
use super::{
    EpicGame, EpicGameDlc, EpicInstallOptions, EpicInstallRequest, EpicInstalled, EpicJobCtx,
    EpicService,
};
use crate::library::queue::{JobPhase, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use crate::stores::gog::install::{game_dir, removal_refusal};
use crate::stores::gog::library_error;
use crate::stores::progress::{ProgressMerger, LEGENDARY_PROGRESS_TOTAL};
use crate::stores::runner::{run_tool, ToolExit, ToolSignal};
use crate::stores::{StoreError, StoreKind};

/// The Wine prefix folder inside an Epic game's folder (as GOG and RomM
/// native games use).
pub const PREFIX_DIR: &str = crate::backup::local::WINE_PREFIX_DIR;
/// legendary's platform name for every Epic install (Epic has no Linux
/// builds).
pub const PLATFORM: &str = "Windows";
/// The shared memory (MiB) a retry after a `MemoryError` asks for
/// (Heroic's value).
pub const RETRY_SHARED_MEMORY: &str = "5000";

const TOOL: &str = "legendary";

// --- argv builders ----------------------------------------------------------

/// `-y --skip-sdl --skip-dlcs`: no prompt (the SDL prompt ignores `-y`, so
/// `--skip-sdl`), and never every DLC (`-y` alone installs them all).
fn quiet(args: &mut Vec<String>) {
    args.extend(["-y", "--skip-sdl", "--skip-dlcs"].map(String::from));
}

/// `install <app> -y --skip-sdl --skip-dlcs --platform Windows
/// --base-path <parent> --game-folder <folder>`: the game lands in exactly
/// `dir`.
pub fn install_args(app: &str, dir: &Path) -> Vec<String> {
    let mut args = vec!["install".to_string(), app.trim().to_string()];
    quiet(&mut args);
    let parent = dir.parent().unwrap_or(dir);
    let folder = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    args.extend([
        "--platform".to_string(),
        PLATFORM.to_string(),
        "--base-path".to_string(),
        parent.to_string_lossy().into_owned(),
        "--game-folder".to_string(),
        folder,
    ]);
    args
}

/// `install <dlc> -y --skip-sdl --skip-dlcs`: legendary puts a DLC in its
/// installed base game's folder.
pub fn dlc_args(dlc: &str) -> Vec<String> {
    let mut args = vec!["install".to_string(), dlc.trim().to_string()];
    quiet(&mut args);
    args
}

/// `update <app> -y --skip-sdl --skip-dlcs`.
pub fn update_args(app: &str) -> Vec<String> {
    let mut args = vec!["update".to_string(), app.trim().to_string()];
    quiet(&mut args);
    args
}

/// `repair <app> -y --skip-sdl --skip-dlcs`: verify, then fix.
pub fn repair_args(app: &str) -> Vec<String> {
    let mut args = vec!["repair".to_string(), app.trim().to_string()];
    quiet(&mut args);
    args
}

/// `uninstall <app> -y --skip-uninstaller` (a base game's DLC go first).
pub fn uninstall_args(app: &str) -> Vec<String> {
    vec![
        "uninstall".to_string(),
        app.trim().to_string(),
        "-y".to_string(),
        "--skip-uninstaller".to_string(),
    ]
}

/// `args` with `--max-shared-memory 5000` (the retry after a
/// `MemoryError`).
pub fn with_more_memory(mut args: Vec<String>) -> Vec<String> {
    args.extend([
        "--max-shared-memory".to_string(),
        RETRY_SHARED_MEMORY.to_string(),
    ]);
    args
}

// --- outcome rules ----------------------------------------------------------

fn tool_failed(reason: &str) -> StoreError {
    StoreError::ToolFailed {
        tool: TOOL,
        reason: reason.to_string(),
    }
}

/// Why a transfer failed, in GRID's words, `None` when it worked: a
/// failure signal (specific ones first), else a non-zero exit, else any
/// other error line. A repair's verify result is not a failure.
pub fn step_failure(exit: &ToolExit) -> Option<StoreError> {
    let has = |signal: ToolSignal| exit.has(&signal);
    if has(ToolSignal::NotLoggedIn) || has(ToolSignal::LoginFailed) {
        return Some(StoreError::LoginExpired);
    }
    if has(ToolSignal::ThirdPartyManaged) {
        return Some(StoreError::Refused(
            "GRID can't install games that start through another company's launcher.".to_string(),
        ));
    }
    if has(ToolSignal::NotEnoughSpace) {
        return Some(tool_failed("there isn't enough disk space"));
    }
    if has(ToolSignal::Network) {
        return Some(tool_failed("Epic's download server didn't answer"));
    }
    if has(ToolSignal::MemoryError) {
        return Some(tool_failed("it ran out of memory"));
    }
    if has(ToolSignal::NotInstalled) {
        return Some(tool_failed("the game isn't installed"));
    }
    match exit.code {
        Some(0) => {}
        Some(code) if code < 0 => {
            return Some(tool_failed(&format!("it was stopped (signal {})", -code)))
        }
        Some(code) => return Some(tool_failed(&format!("it exited with code {code}"))),
        None => return Some(tool_failed("it was stopped")),
    }
    if has(ToolSignal::OtherError) {
        return Some(tool_failed("it reported an error"));
    }
    None
}

/// The job's last check: `app` must be in `list-installed`; after an
/// update its version must be the expected one, or at least not the old
/// one, unless legendary said there was nothing to download (`up_to_date`).
pub fn check_installed(
    op: StoreOp,
    app: &str,
    installed: &[EpicInstalled],
    old_version: &str,
    expected: &str,
    up_to_date: bool,
) -> Result<EpicInstalled, StoreError> {
    let unfinished = || tool_failed("it stopped before it finished. Try again.");
    let entry = installed
        .iter()
        .find(|entry| entry.app_name.trim() == app.trim() && !entry.is_dlc)
        .ok_or_else(unfinished)?;
    let version = entry.version.trim();
    if version.is_empty() {
        return Err(unfinished());
    }
    if op == StoreOp::Update {
        let expected = expected.trim();
        let ok = up_to_date
            || (!expected.is_empty() && version == expected)
            || version != old_version.trim();
        if !ok {
            return Err(unfinished());
        }
    }
    Ok(entry.clone())
}

/// The Downloads row's progress for transfer `index` at `fraction` (of
/// [`LEGENDARY_PROGRESS_TOTAL`]): bytes when every transfer's size is
/// known, else a plain fraction of the whole job.
pub fn overall_progress(sizes: &[u64], index: usize, fraction: u64) -> (u64, u64) {
    let fraction = fraction.min(LEGENDARY_PROGRESS_TOTAL);
    if !sizes.is_empty() && sizes.iter().all(|&size| size > 0) {
        let total: u64 = sizes.iter().sum();
        let before: u64 = sizes.iter().take(index).sum();
        let size = sizes.get(index).copied().unwrap_or(0);
        let part =
            (u128::from(size) * u128::from(fraction) / u128::from(LEGENDARY_PROGRESS_TOTAL)) as u64;
        (before + part, total)
    } else {
        let count = sizes.len().max(1) as u64;
        (
            index as u64 * LEGENDARY_PROGRESS_TOTAL + fraction,
            count * LEGENDARY_PROGRESS_TOTAL,
        )
    }
}

/// The repair result for the Downloads row's detail line ("Verified · …"):
/// `""` when nothing needed fixing.
pub fn repair_detail(repaired: u32) -> String {
    match repaired {
        0 => String::new(),
        1 => "1 file repaired".to_string(),
        n => format!("{n} files repaired"),
    }
}

// --- install options --------------------------------------------------------

/// The install options from the base game's `info` and each DLC's. A DLC
/// whose `info` failed is still offered, with no size.
pub fn options_from(
    base: &EpicInfo,
    dlc: Vec<(EpicGameDlc, Option<EpicInfo>)>,
) -> EpicInstallOptions {
    EpicInstallOptions {
        download_bytes: base.download_bytes,
        disk_bytes: base.disk_bytes,
        dlc: dlc
            .into_iter()
            .map(|(dlc, info)| (dlc, info.map_or(0, |i| i.download_bytes)))
            .collect(),
    }
}

/// The DLC to offer: `info`'s installable owned DLC, then the owned list's,
/// each once.
pub fn dlc_to_offer(info: &EpicInfo, game: Option<&EpicGame>) -> Vec<EpicGameDlc> {
    let mut list: Vec<EpicGameDlc> = Vec::new();
    for dlc in info
        .dlc
        .iter()
        .chain(game.into_iter().flat_map(|g| g.dlc.iter()))
    {
        if !list.iter().any(|d| d.app_name == dlc.app_name) {
            list.push(dlc.clone());
        }
    }
    list
}

// --- jobs -------------------------------------------------------------------

/// One legendary transfer of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    app: String,
    args: Vec<String>,
    is_dlc: bool,
}

/// What the download half learned, for `finalize`.
#[derive(Debug, Clone, Default)]
struct Fetched {
    entry: EpicInstalled,
    installed: Vec<EpicInstalled>,
    info: Option<EpicInfo>,
    warnings: Vec<String>,
    repaired: u32,
}

/// Everything one Epic job needs.
struct EpicJob {
    svc: EpicService,
    ctx: EpicJobCtx,
    op: StoreOp,
    app: String,
    title: String,
    /// The DLC this job installs (an install's picks, a DLC job's one).
    new_dlc: Vec<String>,
    install_dir: PathBuf,
    compat_tool: String,
    old_row: Option<InstalledGame>,
    fetched: Mutex<Option<Fetched>>,
}

/// The install job for `req`.
pub fn install_job(svc: EpicService, req: EpicInstallRequest, ctx: EpicJobCtx) -> StoreJob {
    store_job(EpicJob {
        svc,
        ctx,
        op: StoreOp::Install,
        app: req.app_name.trim().to_string(),
        title: req.title,
        new_dlc: clean_ids(&req.dlc),
        install_dir: req.install_dir,
        compat_tool: req.compat_tool,
        old_row: None,
        fetched: Mutex::new(None),
    })
}

/// The update, repair or DLC (`dlc`) job for an installed row.
pub fn row_job(
    svc: EpicService,
    row: &InstalledGame,
    op: StoreOp,
    dlc: Option<&str>,
    ctx: EpicJobCtx,
) -> StoreJob {
    store_job(EpicJob {
        svc,
        ctx,
        op,
        app: row.store_id.trim().to_string(),
        title: row.title.clone(),
        new_dlc: dlc.map(|d| clean_ids(&[d.to_string()])).unwrap_or_default(),
        install_dir: game_dir(row),
        compat_tool: row.native_compat_tool.clone(),
        old_row: Some(row.clone()),
        fetched: Mutex::new(None),
    })
}

fn clean_ids(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids.iter().map(|d| d.trim()).filter(|d| !d.is_empty()) {
        if !out.iter().any(|d| d == id) {
            out.push(id.to_string());
        }
    }
    out
}

fn store_job(job: EpicJob) -> StoreJob {
    StoreJob {
        source: StoreKind::Epic,
        store_id: job.app.clone(),
        op: job.op,
        title: job.title.clone(),
        drive: None,
        runner: Arc::new(job),
    }
}

impl EpicJob {
    fn old_meta(&self) -> Option<EpicMeta> {
        self.old_row
            .as_ref()
            .map(|r| EpicMeta::from_json(&r.store_meta))
    }

    /// The transfers this job runs, in order.
    fn steps(&self) -> Vec<Step> {
        let installed_dlc: Vec<String> = self
            .old_meta()
            .map(|m| m.dlc.iter().map(|d| d.app_name.clone()).collect())
            .unwrap_or_default();
        let base = |args: Vec<String>| Step {
            app: self.app.clone(),
            args,
            is_dlc: false,
        };
        let dlc = |app: &String, args: Vec<String>| Step {
            app: app.clone(),
            args,
            is_dlc: true,
        };
        match self.op {
            StoreOp::Install => std::iter::once(base(install_args(&self.app, &self.install_dir)))
                .chain(self.new_dlc.iter().map(|d| dlc(d, dlc_args(d))))
                .collect(),
            StoreOp::Update => std::iter::once(base(update_args(&self.app)))
                .chain(installed_dlc.iter().map(|d| dlc(d, update_args(d))))
                .collect(),
            StoreOp::Repair => std::iter::once(base(repair_args(&self.app)))
                .chain(installed_dlc.iter().map(|d| dlc(d, repair_args(d))))
                .collect(),
            _ => self.new_dlc.iter().map(|d| dlc(d, dlc_args(d))).collect(),
        }
    }

    /// `info --json` for `app`, `None` when it fails (sizes are only for
    /// the progress bar).
    fn info(&self, app: &str, cancel: &AtomicBool) -> Result<Option<EpicInfo>, LibraryError> {
        let cmd = self.svc.tool(info_args(app)).map_err(library_error)?;
        let exit = run_tool(&cmd, cancel, &mut |_| {}).map_err(library_error)?;
        if step_failure(&exit).is_some() {
            return Ok(None);
        }
        Ok(exit.read_stdout(parse_info))
    }

    /// One transfer, retried once with more shared memory after a
    /// `MemoryError`.
    fn run_step(
        &self,
        step: &Step,
        cancel: &AtomicBool,
        report: &mut dyn FnMut(u64, f64),
    ) -> Result<ToolExit, LibraryError> {
        let mut args = step.args.clone();
        let mut retried = false;
        loop {
            let mut cmd = self.svc.tool(args.clone()).map_err(library_error)?;
            cmd.progress_stdout = self.op == StoreOp::Repair;
            let mut merger = ProgressMerger::default();
            let exit = run_tool(&cmd, cancel, &mut |line| {
                let merged = merger.push(line);
                report(merged.done, merged.speed_bps);
            })
            .map_err(library_error)?;
            if exit.has(&ToolSignal::MemoryError) && !retried {
                retried = true;
                args = with_more_memory(step.args.clone());
                continue;
            }
            return Ok(exit);
        }
    }
}

impl StoreJobRunner for EpicJob {
    fn download(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        let game = self.svc.game(&self.app);
        if let Some(launcher) = game.as_ref().and_then(|g| g.third_party.clone()) {
            return Err(LibraryError::Extract(format!(
                "Needs {launcher}. GRID can't install games that start through another company's launcher."
            )));
        }
        if self.op == StoreOp::Repair && !self.install_dir.is_dir() {
            return Err(LibraryError::Extract(
                "GRID can't verify this game: its folder is gone. Reinstall it.".to_string(),
            ));
        }
        let steps = self.steps();
        if steps.is_empty() {
            return Err(LibraryError::Extract(
                "There's nothing to install.".to_string(),
            ));
        }

        // Sizes for the bar: an install's or a DLC's `info` download sizes.
        // An update's delta and a repair's fixes are unknown up front.
        let mut base_info = None;
        let mut sizes = vec![0u64; steps.len()];
        if matches!(self.op, StoreOp::Install | StoreOp::Dlc) {
            for (i, step) in steps.iter().enumerate() {
                let info = self.info(&step.app, cancel)?;
                sizes[i] = info.as_ref().map_or(0, |i| i.download_bytes);
                if !step.is_dlc {
                    base_info = info;
                }
            }
        }
        if self.op == StoreOp::Install {
            if let Some(parent) = self.install_dir.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        let (_, total) = overall_progress(&sizes, 0, 0);
        progress(0, total, 0.0);
        let mut warnings = Vec::new();
        let mut repaired = 0u32;
        let mut up_to_date = false;
        for (index, step) in steps.iter().enumerate() {
            let exit = self.run_step(step, cancel, &mut |fraction, speed| {
                let (done, total) = overall_progress(&sizes, index, fraction);
                progress(done, total, speed);
            })?;
            for signal in &exit.signals {
                if let ToolSignal::VerifyFailed { corrupted, missing } = signal {
                    repaired = repaired.saturating_add(corrupted.saturating_add(*missing));
                }
            }
            if let Some(error) = step_failure(&exit) {
                if step.is_dlc && self.op != StoreOp::Dlc {
                    let name = game
                        .as_ref()
                        .and_then(|g| g.dlc.iter().find(|d| d.app_name == step.app))
                        .map(|d| d.title.clone())
                        .filter(|t| !t.is_empty())
                        .unwrap_or_else(|| step.app.clone());
                    let verb = match self.op {
                        StoreOp::Update => "update",
                        StoreOp::Repair => "repair",
                        _ => "install",
                    };
                    warnings.push(format!("Couldn't {verb} {name}: {error}"));
                    continue;
                }
                return Err(library_error(error));
            }
            if !step.is_dlc && exit.has(&ToolSignal::DownloadSize(0)) {
                up_to_date = true;
            }
            let (done, total) = overall_progress(&sizes, index, LEGENDARY_PROGRESS_TOTAL);
            progress(done, total, 0.0);
        }

        let cmd = self.svc.tool(installed_args()).map_err(library_error)?;
        let exit = run_tool(&cmd, cancel, &mut |_| {}).map_err(library_error)?;
        let installed = exit.read_stdout(parse_installed).ok_or_else(|| {
            library_error(tool_failed("it gave an answer GRID doesn't understand"))
        })?;
        let old_version = self.old_meta().map(|m| m.version).unwrap_or_default();
        let expected = game
            .as_ref()
            .map(|g| g.build_version.clone())
            .unwrap_or_default();
        let entry = check_installed(
            self.op,
            &self.app,
            &installed,
            &old_version,
            &expected,
            up_to_date,
        )
        .map_err(library_error)?;
        if self.op == StoreOp::Dlc {
            for dlc in &self.new_dlc {
                if !installed.iter().any(|e| e.app_name.trim() == dlc) {
                    return Err(library_error(tool_failed(
                        "it stopped before it finished. Try again.",
                    )));
                }
            }
        }
        *self.fetched.lock().unwrap_or_else(|e| e.into_inner()) = Some(Fetched {
            entry,
            installed,
            info: base_info,
            warnings,
            repaired,
        });
        Ok(())
    }

    fn download_phase(&self) -> JobPhase {
        if self.op == StoreOp::Repair {
            JobPhase::Verifying
        } else {
            JobPhase::None
        }
    }

    fn finalize(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        sink.phase(JobPhase::Installing);
        sink.progress(0, 1);
        let fetched = self
            .fetched
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| LibraryError::Extract("The download didn't finish.".to_string()))?;
        let game = self.svc.game(&self.app);
        let old_meta = self.old_meta();
        let meta = build_meta(
            &self.app,
            &fetched.entry,
            &fetched.installed,
            game.as_ref(),
            fetched.info.as_ref(),
            old_meta.as_ref(),
            &self.new_dlc,
        );
        let row = build_row(
            &fetched.entry,
            game.as_ref(),
            &meta,
            self.old_row.as_ref(),
            &self.compat_tool,
        )?;
        self.ctx.registry.upsert_store(&row)?;
        sink.progress(1, 1);
        // A repair's result is the row's detail line ([`Self::detail`]);
        // the message holds only warnings.
        Ok(fetched.warnings.join("\n"))
    }

    fn detail(&self) -> String {
        if self.op != StoreOp::Repair {
            return String::new();
        }
        self.fetched
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|fetched| repair_detail(fetched.repaired))
            .unwrap_or_default()
    }
}

/// The meta for a finished job: the old row's (cloud and save settings,
/// the pinned executable) with legendary's version, and the DLC that
/// `list-installed` now shows.
fn build_meta(
    app: &str,
    entry: &EpicInstalled,
    installed: &[EpicInstalled],
    game: Option<&EpicGame>,
    info: Option<&EpicInfo>,
    old: Option<&EpicMeta>,
    new_dlc: &[String],
) -> EpicMeta {
    let mut meta = old.cloned().unwrap_or(EpicMeta {
        cloud_saves: true,
        ..Default::default()
    });
    meta.app_name = app.trim().to_string();
    meta.version = entry.version.trim().to_string();
    meta.platform = PLATFORM.to_string();
    meta.can_run_offline = entry.can_run_offline || game.is_some_and(|g| g.can_run_offline);
    let folder = game
        .map(|g| g.cloud_save_folder.trim().to_string())
        .filter(|f| !f.is_empty())
        .or_else(|| {
            info.map(|i| i.cloud_save_folder.trim().to_string())
                .filter(|f| !f.is_empty())
        });
    if let Some(folder) = folder {
        meta.cloud_save_folder = folder;
    }
    if meta.save_path.trim().is_empty() && !entry.save_path.trim().is_empty() {
        meta.save_path = entry.save_path.trim().to_string();
    }
    let title_of = |id: &str| {
        game.and_then(|g| g.dlc.iter().find(|d| d.app_name == id))
            .map(|d| d.title.clone())
            .or_else(|| {
                info.and_then(|i| i.dlc.iter().find(|d| d.app_name == id))
                    .map(|d| d.title.clone())
            })
            .filter(|t| !t.trim().is_empty())
            .or_else(|| {
                old.and_then(|m| m.dlc.iter().find(|d| d.app_name == id))
                    .map(|d| d.title.clone())
                    .filter(|t| !t.trim().is_empty())
            })
            .or_else(|| {
                installed
                    .iter()
                    .find(|e| e.app_name.trim() == id)
                    .map(|e| e.title.trim().to_string())
            })
            .unwrap_or_default()
    };
    let mut wanted: Vec<String> = old
        .map(|m| m.dlc.iter().map(|d| d.app_name.clone()).collect())
        .unwrap_or_default();
    for id in new_dlc {
        if !wanted.contains(id) {
            wanted.push(id.clone());
        }
    }
    meta.dlc = wanted
        .iter()
        .filter(|id| installed.iter().any(|e| e.app_name.trim() == id.as_str()))
        .map(|id| EpicDlcMeta {
            app_name: id.clone(),
            title: title_of(id),
        })
        .collect();
    meta
}

/// The row for a finished job: [`epic_row`], the Wine prefix (off
/// Windows) and compat tool, and from the old row the user's pinned
/// executable, the cached covers and (with no owned-list entry) the
/// details.
fn build_row(
    entry: &EpicInstalled,
    game: Option<&EpicGame>,
    meta: &EpicMeta,
    old: Option<&InstalledGame>,
    compat_tool: &str,
) -> Result<InstalledGame, LibraryError> {
    let dir = PathBuf::from(entry.install_path.trim());
    let prefix = (!cfg!(windows)).then(|| {
        old.map(|r| r.native_wineprefix.trim().to_string())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.join(PREFIX_DIR))
    });
    if let Some(prefix) = &prefix {
        fs::create_dir_all(prefix)?;
    }
    let mut row = epic_row(entry, game, meta, prefix.as_deref());
    row.native_compat_tool = compat_tool.trim().to_string();
    if let Some(old) = old {
        if meta.exe_user_pinned && !old.native_executable_path.trim().is_empty() {
            row.native_executable_path = old.native_executable_path.clone();
            row.native_launch_parameters = old.native_launch_parameters.clone();
            row.native_working_dir = old.native_working_dir.clone();
        }
        if game.is_none() {
            row.title = old.title.clone();
            row.description = old.description.clone();
            row.companies = old.companies.clone();
            row.first_release_date = old.first_release_date.clone();
            row.screenshot_urls = old.screenshot_urls.clone();
        }
        row.cover_small_path = old.cover_small_path.clone();
        row.cover_large_path = old.cover_large_path.clone();
        row.fanart_urls = old.fanart_urls.clone();
        row.genres = old.genres.clone();
    }
    Ok(row)
}

// --- DLC removal and uninstall ---------------------------------------------

/// Runs `uninstall <app> -y --skip-uninstaller`. A missing legendary, or
/// legendary saying the app is not installed, is fine (the files go
/// anyway); any other failure stops the removal before a file is touched.
fn legendary_uninstall(svc: &EpicService, app: &str) -> Result<(), StoreError> {
    let cmd = match svc.tool(uninstall_args(app)) {
        Ok(cmd) => cmd,
        Err(StoreError::ToolMissing(_)) => return Ok(()),
        Err(other) => return Err(other),
    };
    let exit = run_tool(&cmd, &AtomicBool::new(false), &mut |_| {})?;
    if exit.has(&ToolSignal::NotInstalled) {
        return Ok(());
    }
    match step_failure(&exit) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Removes one DLC (`uninstall <dlc>`; the base game stays), then the
/// row's meta drops it. Blocking.
pub fn remove_dlc(
    svc: &EpicService,
    registry: &Registry,
    row: &InstalledGame,
    dlc: &str,
) -> Result<(), StoreError> {
    let mut meta = EpicMeta::from_json(&row.store_meta);
    let dlc = dlc.trim();
    if !meta.dlc.iter().any(|d| d.app_name == dlc) {
        return Err(StoreError::Refused("That DLC isn't installed.".to_string()));
    }
    legendary_uninstall(svc, dlc)?;
    meta.dlc.retain(|d| d.app_name != dlc);
    let mut updated = row.clone();
    updated.store_meta = meta.to_json();
    registry.upsert_store(&updated)?;
    Ok(())
}

/// Uninstalls an Epic game: legendary's `uninstall` (the manifest's files
/// and its DLC), then what is left of the folder (the Wine prefix inside
/// it with it) through the library's removal guard, and the row. A folder
/// the guard refuses stays on disk (the row still goes), as a GOG
/// uninstall does. Blocking.
pub fn uninstall(svc: &EpicService, registry: &Registry, app: &str) -> Result<(), StoreError> {
    let row = registry
        .find_store(StoreKind::Epic, app.trim())?
        .ok_or_else(|| StoreError::Refused("That game isn't installed.".to_string()))?;
    legendary_uninstall(svc, &row.store_id)?;
    let dir = game_dir(&row);
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
                "Epic uninstall kept a folder the removal guard refused"
            ),
        }
    }
    registry.remove_store(StoreKind::Epic, &row.store_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    fn exit(code: i32, signals: Vec<ToolSignal>) -> ToolExit {
        ToolExit {
            code: Some(code),
            stdout: SecretString::from(String::new()),
            signals,
        }
    }

    #[test]
    fn every_transfer_skips_dlc_and_the_sdl_prompt() {
        let dir = Path::new("/games/Epic Games/Starlight Courier");
        let all = [
            install_args("Starlight", dir),
            dlc_args("StarlightDLC1"),
            update_args("Starlight"),
            repair_args("Starlight"),
        ];
        for args in &all {
            assert!(args.contains(&"--skip-dlcs".to_string()), "{args:?}");
            assert!(args.contains(&"--skip-sdl".to_string()), "{args:?}");
            assert!(args.contains(&"-y".to_string()), "{args:?}");
            assert!(!args.iter().any(|a| a == "--code" || a == "--with-dlcs"));
        }
        assert_eq!(
            all[0],
            [
                "install",
                "Starlight",
                "-y",
                "--skip-sdl",
                "--skip-dlcs",
                "--platform",
                "Windows",
                "--base-path",
                "/games/Epic Games",
                "--game-folder",
                "Starlight Courier"
            ]
        );
        assert_eq!(all[1][..2], ["install", "StarlightDLC1"]);
        assert_eq!(all[2][0], "update");
        assert_eq!(all[3][0], "repair");
        assert_eq!(
            uninstall_args("Starlight"),
            ["uninstall", "Starlight", "-y", "--skip-uninstaller"]
        );
        let retry = with_more_memory(update_args("A"));
        assert_eq!(retry[retry.len() - 2..], ["--max-shared-memory", "5000"]);
    }

    #[test]
    fn a_transfer_fails_on_signals_even_with_exit_0() {
        assert!(step_failure(&exit(0, vec![])).is_none());
        assert!(step_failure(&exit(0, vec![ToolSignal::DownloadSize(0)])).is_none());
        assert!(step_failure(&exit(
            0,
            vec![ToolSignal::VerifyFailed {
                corrupted: 1,
                missing: 1
            }]
        ))
        .is_none());
        let cases: [(i32, Vec<ToolSignal>, &str); 9] = [
            (
                1,
                vec![ToolSignal::NotLoggedIn, ToolSignal::OtherError],
                "login expired",
            ),
            (0, vec![ToolSignal::LoginFailed], "login expired"),
            (
                1,
                vec![ToolSignal::OtherError, ToolSignal::NotEnoughSpace],
                "legendary stopped: there isn't enough disk space",
            ),
            (
                0,
                vec![ToolSignal::Network, ToolSignal::OtherError],
                "legendary stopped: Epic's download server didn't answer",
            ),
            (
                0,
                vec![ToolSignal::ThirdPartyManaged, ToolSignal::OtherError],
                "another company's launcher",
            ),
            (
                1,
                vec![ToolSignal::MemoryError],
                "legendary stopped: it ran out of memory",
            ),
            (
                1,
                vec![ToolSignal::NotInstalled],
                "legendary stopped: the game isn't installed",
            ),
            (2, vec![], "legendary stopped: it exited with code 2"),
            (
                0,
                vec![ToolSignal::OtherError],
                "legendary stopped: it reported an error",
            ),
        ];
        for (code, signals, words) in cases {
            let error = step_failure(&exit(code, signals.clone())).unwrap();
            assert!(error.to_string().contains(words), "{signals:?}: {error}");
        }
        assert!(matches!(
            step_failure(&exit(0, vec![ToolSignal::NotLoggedIn])),
            Some(StoreError::LoginExpired)
        ));
    }

    fn entry(app: &str, version: &str, is_dlc: bool) -> EpicInstalled {
        EpicInstalled {
            app_name: app.into(),
            version: version.into(),
            install_path: "/g/S".into(),
            is_dlc,
            ..Default::default()
        }
    }

    #[test]
    fn the_job_is_done_only_when_list_installed_agrees() {
        let list = [entry("S", "2", false), entry("D", "1", true)];
        assert!(check_installed(StoreOp::Install, "S", &list, "", "", false).is_ok());
        assert!(check_installed(StoreOp::Install, "X", &list, "", "", false).is_err());
        assert!(
            check_installed(StoreOp::Install, "D", &list, "", "", false).is_err(),
            "a DLC entry is not the game"
        );
        assert!(check_installed(
            StoreOp::Install,
            "S",
            &[entry("S", "", false)],
            "",
            "",
            false
        )
        .is_err());
        // Updates: the expected build, or at least a different one.
        assert!(check_installed(StoreOp::Update, "S", &list, "1", "2", false).is_ok());
        assert!(check_installed(StoreOp::Update, "S", &list, "1", "3", false).is_ok());
        let error = check_installed(StoreOp::Update, "S", &list, "2", "3", false).unwrap_err();
        assert!(error.to_string().contains("before it finished"), "{error}");
        assert!(
            check_installed(StoreOp::Update, "S", &list, "2", "3", true).is_ok(),
            "nothing to download"
        );
        assert!(check_installed(StoreOp::Repair, "S", &list, "2", "2", false).is_ok());
    }

    #[test]
    fn progress_is_bytes_when_sizes_are_known_else_a_fraction() {
        assert_eq!(overall_progress(&[1000, 500], 0, 5000), (500, 1500));
        assert_eq!(overall_progress(&[1000, 500], 1, 5000), (1250, 1500));
        assert_eq!(overall_progress(&[1000, 500], 1, 99_999), (1500, 1500));
        assert_eq!(overall_progress(&[0], 0, 4500), (4500, 10_000));
        assert_eq!(overall_progress(&[1000, 0], 1, 2500), (12_500, 20_000));
        assert_eq!(overall_progress(&[], 0, 0), (0, 10_000));
        assert_eq!(
            overall_progress(&[50_000_000_000], 0, 10_000),
            (50_000_000_000, 50_000_000_000)
        );
    }

    #[test]
    fn repair_detail_words() {
        assert_eq!(repair_detail(0), "");
        assert_eq!(repair_detail(1), "1 file repaired");
        assert_eq!(repair_detail(2), "2 files repaired");
    }

    #[test]
    fn options_offer_info_dlc_then_owned_dlc_once() {
        let info = EpicInfo {
            download_bytes: 10,
            disk_bytes: 20,
            dlc: vec![EpicGameDlc {
                app_name: "D1".into(),
                title: "One".into(),
            }],
            ..Default::default()
        };
        let game = EpicGame {
            dlc: vec![
                EpicGameDlc {
                    app_name: "D1".into(),
                    title: "One (owned)".into(),
                },
                EpicGameDlc {
                    app_name: "D2".into(),
                    title: "Two".into(),
                },
            ],
            ..Default::default()
        };
        let offer = dlc_to_offer(&info, Some(&game));
        assert_eq!(offer.len(), 2);
        assert_eq!(offer[0].title, "One");
        let options = options_from(
            &info,
            vec![
                (
                    offer[0].clone(),
                    Some(EpicInfo {
                        download_bytes: 5,
                        ..Default::default()
                    }),
                ),
                (offer[1].clone(), None),
            ],
        );
        assert_eq!((options.download_bytes, options.disk_bytes), (10, 20));
        assert_eq!(options.dlc[0].1, 5);
        assert_eq!(options.dlc[1].1, 0);
    }

    #[test]
    fn meta_keeps_the_old_settings_and_only_installed_dlc() {
        let old = EpicMeta {
            app_name: "S".into(),
            version: "1".into(),
            cloud_saves: false,
            save_path: "/saves".into(),
            last_sync: 9,
            exe_user_pinned: true,
            dlc: vec![EpicDlcMeta {
                app_name: "D1".into(),
                title: "One".into(),
            }],
            ..Default::default()
        };
        let installed = [
            entry("S", "2", false),
            entry("D1", "1", true),
            entry("D2", "1", true),
        ];
        let game = EpicGame {
            cloud_save_folder: "{AppData}/S".into(),
            dlc: vec![EpicGameDlc {
                app_name: "D2".into(),
                title: "Two".into(),
            }],
            ..Default::default()
        };
        let meta = build_meta(
            "S",
            &installed[0],
            &installed,
            Some(&game),
            None,
            Some(&old),
            &["D2".into(), "D3".into()],
        );
        assert_eq!(meta.version, "2");
        assert_eq!(meta.platform, "Windows");
        assert!(!meta.cloud_saves);
        assert_eq!(meta.save_path, "/saves");
        assert_eq!(meta.last_sync, 9);
        assert!(meta.exe_user_pinned);
        assert_eq!(meta.cloud_save_folder, "{AppData}/S");
        let names: Vec<(&str, &str)> = meta
            .dlc
            .iter()
            .map(|d| (d.app_name.as_str(), d.title.as_str()))
            .collect();
        assert_eq!(names, [("D1", "One"), ("D2", "Two")], "D3 never installed");

        let fresh = build_meta("S", &installed[0], &installed, None, None, None, &[]);
        assert!(fresh.cloud_saves, "a new install has cloud saves on");
        assert!(fresh.dlc.is_empty());
    }
}
