//! nile installs: the install, update and verify job runners, and
//! uninstall.
//!
//! nile exits 0 for most failures and prints `Download complete` even
//! after `Not enough space available` or `Unable to load manifest`
//! (spike 1), so a transfer counts as done only when it printed no failure
//! ([`step_failure`]), `installed.json` then names the game with a version
//! ([`check_installed`]) and the game's `fuel.json` is readable. A dropped
//! connection can still end an install as "complete" (nile #51): an
//! install that saw a network failure but left an `installed.json` entry
//! runs `verify` once, and that decides.
//!
//! Progress comes from [`crate::stores::progress::parse_nile_line`] in
//! real bytes; before nile's first progress line the total is
//! `install --info`'s download size.
//!
//! After the row is written, the game's fuel.json `PostInstall` steps run
//! through `stores::redist` (`docs/stores-phase3-plan.md` "User decisions"
//! 2): after an install, and after an update or a verify when the list
//! differs from [`AmazonMeta::post_install_ran`], the list that last
//! worked. Their problems are the finished entry's warning, never a
//! failed job; a cancel stops them and leaves the game installed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::list::{self, info_args, parse_info};
use super::meta::AmazonMeta;
use super::row::{amazon_row, read_fuel, read_post_install, FuelMain};
use super::{AmazonInstallRequest, AmazonInstalled, AmazonJobCtx, AmazonService};
use crate::library::queue::{JobPhase, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use crate::stores::gog::install::{game_dir, removal_refusal};
use crate::stores::gog::library_error;
use crate::stores::progress::ProgressMerger;
use crate::stores::redist::{run_steps, steps_due, RedistTarget};
use crate::stores::runner::{ToolExit, ToolSignal, TIMED_OUT};
use crate::stores::{StoreError, StoreKind};

/// The Wine prefix folder inside an Amazon game's folder (as GOG, Epic and
/// RomM native games use).
pub const PREFIX_DIR: &str = crate::backup::local::WINE_PREFIX_DIR;

/// The characters nile strips from a title to make its folder name
/// (`ILLEGAL_FNAME_CHARS`, nile 1.2.0 `constants.py`).
pub const ILLEGAL_FOLDER_CHARS: [char; 11] =
    [':', '/', '*', '?', '<', '>', '\\', '|', '™', '"', '®'];

/// How long `uninstall` may take (it deletes the manifest's files one by
/// one, after nile's FuelSDK attempt, which can hang).
pub const UNINSTALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

const TOOL: &str = "nile";

// --- argv builders ----------------------------------------------------------

/// The folder name nile would give `title` (its banned characters
/// stripped, then outer spaces and dots), `id` when nothing is left.
pub fn folder_name(title: &str, id: &str) -> String {
    let clean = |text: &str| {
        text.chars()
            .filter(|c| !ILLEGAL_FOLDER_CHARS.contains(c) && !c.is_control())
            .collect::<String>()
            .trim_matches(|c: char| c.is_whitespace() || c == '.')
            .to_string()
    };
    let name = clean(title);
    if name.is_empty() {
        clean(id)
    } else {
        name
    }
}

/// `install --path <dir> <id>`: the game lands in exactly `dir` (never
/// `--base-path`; `--max-workers` is ignored by nile).
pub fn install_args(id: &str, dir: &Path) -> Vec<String> {
    vec![
        "install".to_string(),
        "--path".to_string(),
        dir.to_string_lossy().into_owned(),
        id.trim().to_string(),
    ]
}

/// `update <id>`: the diff against the installed manifest.
pub fn update_args(id: &str) -> Vec<String> {
    vec!["update".to_string(), id.trim().to_string()]
}

/// `verify <id>`: re-hash, then download bad or missing files.
pub fn verify_args(id: &str) -> Vec<String> {
    vec!["verify".to_string(), id.trim().to_string()]
}

/// `uninstall <id>`.
pub fn uninstall_args(id: &str) -> Vec<String> {
    vec!["uninstall".to_string(), id.trim().to_string()]
}

// --- outcome rules ----------------------------------------------------------

fn tool_failed(reason: &str) -> StoreError {
    StoreError::ToolFailed {
        tool: TOOL,
        reason: reason.to_string(),
    }
}

/// Why a nile run failed, in GRID's words, `None` when it worked: a
/// failure signal (specific ones first), else a non-zero exit, else any
/// other error line. "Game is up to date" is not a failure.
pub fn step_failure(exit: &ToolExit) -> Option<StoreError> {
    let has = |signal: ToolSignal| exit.has(&signal);
    if has(ToolSignal::NotLoggedIn) || has(ToolSignal::LoginFailed) {
        return Some(StoreError::LoginExpired);
    }
    if has(ToolSignal::NotEnoughSpace) {
        return Some(tool_failed("there isn't enough disk space"));
    }
    if has(ToolSignal::Network) {
        return Some(tool_failed("Amazon's download server didn't answer"));
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

/// The job's last check: `id` must be in `installed.json` with a version;
/// after an update the version must have changed, unless nile said the
/// game was already up to date (`up_to_date`).
pub fn check_installed(
    op: StoreOp,
    id: &str,
    installed: &[AmazonInstalled],
    old_version: &str,
    up_to_date: bool,
) -> Result<AmazonInstalled, StoreError> {
    let unfinished = || tool_failed("it stopped before it finished. Try again.");
    let entry = installed
        .iter()
        .find(|entry| entry.id.trim() == id.trim())
        .ok_or_else(unfinished)?;
    let version = entry.version.trim();
    if version.is_empty() || entry.path.trim().is_empty() {
        return Err(unfinished());
    }
    if op == StoreOp::Update && !up_to_date && version == old_version.trim() {
        return Err(unfinished());
    }
    Ok(entry.clone())
}

/// The Downloads row's `(done, total)` for a nile progress report: nile's
/// bytes when it gave a total, else `fallback` (`install --info`'s size).
pub fn progress_of(done: u64, total: u64, fallback: u64) -> (u64, u64) {
    if total > 0 {
        (done.min(total), total)
    } else {
        (done.min(fallback), fallback)
    }
}

// --- jobs -------------------------------------------------------------------

/// What the download half learned, for `finalize`.
#[derive(Debug, Clone)]
struct Fetched {
    entry: AmazonInstalled,
    fuel: FuelMain,
}

/// Everything one Amazon job needs.
struct AmazonJob {
    svc: AmazonService,
    ctx: AmazonJobCtx,
    op: StoreOp,
    id: String,
    title: String,
    install_dir: PathBuf,
    compat_tool: String,
    old_row: Option<InstalledGame>,
    fetched: Mutex<Option<Fetched>>,
}

/// The install job for `req`.
pub fn install_job(svc: AmazonService, req: AmazonInstallRequest, ctx: AmazonJobCtx) -> StoreJob {
    store_job(AmazonJob {
        svc,
        ctx,
        op: StoreOp::Install,
        id: req.product_id.trim().to_string(),
        title: req.title,
        install_dir: req.install_dir,
        compat_tool: req.compat_tool,
        old_row: None,
        fetched: Mutex::new(None),
    })
}

/// The update or verify (`StoreOp::Repair`) job for an installed row.
pub fn row_job(
    svc: AmazonService,
    row: &InstalledGame,
    op: StoreOp,
    ctx: AmazonJobCtx,
) -> StoreJob {
    store_job(AmazonJob {
        svc,
        ctx,
        op,
        id: row.store_id.trim().to_string(),
        title: row.title.clone(),
        install_dir: game_dir(row),
        compat_tool: row.native_compat_tool.clone(),
        old_row: Some(row.clone()),
        fetched: Mutex::new(None),
    })
}

fn store_job(job: AmazonJob) -> StoreJob {
    StoreJob {
        source: StoreKind::Amazon,
        store_id: job.id.clone(),
        op: job.op,
        title: job.title.clone(),
        drive: None,
        runner: Arc::new(job),
    }
}

/// A row the removal guard can judge `dir` against (an install that
/// never wrote one).
fn probe_row(dir: &Path) -> InstalledGame {
    InstalledGame {
        platform: StoreKind::Amazon.platform_name().to_string(),
        native_game_dir: dir.to_string_lossy().into_owned(),
        extracted_dir: dir.to_string_lossy().into_owned(),
        ..Default::default()
    }
}

/// Deletes `dir` unless the removal guard refuses it (then it stays, with
/// a warning that names the path, never tool output).
fn remove_guarded(row: &InstalledGame, dir: &Path) -> Result<(), StoreError> {
    if dir.as_os_str().is_empty() || !dir.is_dir() {
        return Ok(());
    }
    match removal_refusal(row, dir) {
        None => fs::remove_dir_all(dir).map_err(|e| {
            StoreError::Refused(format!(
                "Couldn't delete {} ({}).",
                dir.to_string_lossy(),
                e.kind()
            ))
        }),
        Some(reason) => {
            tracing::warn!(
                path = %dir.to_string_lossy(),
                reason,
                "Amazon Games kept a folder the removal guard refused"
            );
            Ok(())
        }
    }
}

impl AmazonJob {
    fn old_meta(&self) -> Option<AmazonMeta> {
        self.old_row
            .as_ref()
            .map(|r| AmazonMeta::from_json(&r.store_meta))
    }

    /// `install --info` for the bar's total before nile's first progress
    /// line; 0 when it fails (the size is only for the bar).
    fn info_size(&self, cancel: &AtomicBool) -> Result<u64, LibraryError> {
        let mut cmd = self.svc.tool(info_args(&self.id)).map_err(library_error)?;
        cmd.timeout = Some(super::QUICK_RUN_TIMEOUT);
        match self.svc.run(&cmd, cancel, &mut |_| {}) {
            Ok(exit) if step_failure(&exit).is_none() => {
                Ok(exit.read_stdout(parse_info).unwrap_or(0))
            }
            Ok(_) => Ok(0),
            Err(StoreError::Library(LibraryError::Cancelled)) => Err(LibraryError::Cancelled),
            Err(error @ StoreError::Refused(_)) => Err(library_error(error)),
            Err(_) => Ok(0),
        }
    }

    /// One nile transfer with its progress.
    fn transfer(
        &self,
        args: Vec<String>,
        fallback_total: u64,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<ToolExit, LibraryError> {
        let cmd = self.svc.tool(args).map_err(library_error)?;
        let mut merger = ProgressMerger::default();
        progress(0, fallback_total, 0.0);
        self.svc
            .run(&cmd, cancel, &mut |line| {
                let merged = merger.push(line);
                let (done, total) = progress_of(merged.done, merged.total, fallback_total);
                progress(done, total, merged.speed_bps);
            })
            .map_err(library_error)
    }

    fn fetch(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        let mut size = 0;
        let args = match self.op {
            StoreOp::Install => {
                size = self.info_size(cancel)?;
                if let Some(parent) = self.install_dir.parent() {
                    fs::create_dir_all(parent)?;
                }
                install_args(&self.id, &self.install_dir)
            }
            StoreOp::Update => update_args(&self.id),
            _ => verify_args(&self.id),
        };
        let mut exit = self.transfer(args, size, cancel, progress)?;
        let config = self.svc.config_dir();
        if self.op == StoreOp::Install
            && exit.has(&ToolSignal::Network)
            && list::installed_entry(&config, &self.id)
                .is_some_and(|entry| !entry.version.trim().is_empty())
        {
            // nile #51: a dropped connection that still ended "complete".
            exit = self.transfer(verify_args(&self.id), size, cancel, progress)?;
        }
        if let Some(error) = step_failure(&exit) {
            return Err(library_error(error));
        }
        let up_to_date = exit.has(&ToolSignal::DownloadSize(0));
        let old_version = self.old_meta().map(|m| m.version).unwrap_or_default();
        let entry = check_installed(
            self.op,
            &self.id,
            &list::read_installed(&config),
            &old_version,
            up_to_date,
        )
        .map_err(library_error)?;
        let fuel = read_fuel(Path::new(entry.path.trim())).ok_or_else(|| {
            library_error(tool_failed(
                "the game's start-up file (fuel.json) is missing or unreadable",
            ))
        })?;
        let total = entry.size.max(size);
        progress(total, total, 0.0);
        *self.fetched.lock().unwrap_or_else(|e| e.into_inner()) = Some(Fetched { entry, fuel });
        Ok(())
    }
}

impl StoreJobRunner for AmazonJob {
    fn download(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        if !self.svc.has_login() {
            return Err(library_error(StoreError::NotSignedIn));
        }
        if self.op == StoreOp::Repair && !self.install_dir.is_dir() {
            return Err(LibraryError::Extract(
                "GRID can't verify this game: its folder is gone. Reinstall it.".to_string(),
            ));
        }
        let fresh = self.op == StoreOp::Install && !self.install_dir.exists();
        let result = self.fetch(cancel, progress);
        if fresh && matches!(result, Err(LibraryError::Cancelled)) {
            // A cancelled first install leaves nothing behind.
            let _ = remove_guarded(&probe_row(&self.install_dir), &self.install_dir);
        }
        result
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
        let game = list::read_library(&self.svc.config_dir())
            .and_then(|games| games.into_iter().find(|g| g.product_id == self.id));
        let mut meta = self.old_meta().unwrap_or_default();
        meta.product_id = self.id.clone();
        meta.version = fetched.entry.version.trim().to_string();
        let dir = PathBuf::from(fetched.entry.path.trim());
        let steps = read_post_install(&dir);
        let due = steps_due(self.op == StoreOp::Install, &steps, &meta.post_install_ran);
        if steps.is_empty() {
            meta.post_install_ran.clear();
        }
        let mut row = build_row(
            &fetched.entry,
            game.as_ref(),
            &meta,
            &fetched.fuel,
            self.old_row.as_ref(),
            &self.compat_tool,
        )?;
        // The game is installed before its steps run: a cancel or a
        // failed step leaves it so.
        self.ctx.registry.upsert_store(&row)?;
        sink.progress(1, 1);
        if !due {
            return Ok(String::new());
        }
        let compat = match row.native_compat_tool.trim() {
            "" => self.ctx.default_compat_tool.trim().to_string(),
            chosen => chosen.to_string(),
        };
        let prefix = Some(row.native_wineprefix.trim())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);
        let outcome = run_steps(
            &steps,
            RedistTarget {
                game_dir: &dir,
                compat: &compat,
                prefix: prefix.as_deref(),
            },
            &self.svc.redist,
            sink,
        );
        if outcome.all_worked {
            meta.post_install_ran = steps;
            row.store_meta = meta.to_json();
            self.ctx.registry.upsert_store(&row)?;
        }
        Ok(outcome.warning())
    }
}

/// The row for a finished job: [`amazon_row`], the Wine prefix (off
/// Windows) and compat tool, and from the old row the user's pinned
/// executable, the cached covers and (with no owned-list entry) the
/// details.
fn build_row(
    entry: &AmazonInstalled,
    game: Option<&super::AmazonGame>,
    meta: &AmazonMeta,
    fuel: &FuelMain,
    old: Option<&InstalledGame>,
    compat_tool: &str,
) -> Result<InstalledGame, LibraryError> {
    let dir = PathBuf::from(entry.path.trim());
    let prefix = (!cfg!(windows)).then(|| {
        old.map(|r| r.native_wineprefix.trim().to_string())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.join(PREFIX_DIR))
    });
    if let Some(prefix) = &prefix {
        fs::create_dir_all(prefix)?;
    }
    let mut row = amazon_row(entry, game, meta, Some(fuel), prefix.as_deref());
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
            row.genres = old.genres.clone();
        }
        row.cover_small_path = old.cover_small_path.clone();
        row.cover_large_path = old.cover_large_path.clone();
        row.fanart_urls = old.fanart_urls.clone();
    }
    Ok(row)
}

// --- uninstall --------------------------------------------------------------

/// Runs `uninstall <id>`. `Ok(true)` when nile removed the game itself,
/// `Ok(false)` when GRID must forget it in nile's files: nile is missing
/// or can't run on this system, it says the game isn't installed, or it
/// crashed (a missing manifest). Any other failure stops the uninstall
/// before a file is touched.
fn nile_uninstall(svc: &AmazonService, id: &str) -> Result<bool, StoreError> {
    let mut cmd = match svc.tool(uninstall_args(id)) {
        Ok(cmd) => cmd,
        Err(StoreError::ToolMissing(_)) => return Ok(false),
        Err(other) => return Err(other),
    };
    cmd.timeout = Some(UNINSTALL_TIMEOUT);
    let exit = match svc.run(&cmd, &AtomicBool::new(false), &mut |_| {}) {
        Ok(exit) => exit,
        Err(StoreError::Refused(text)) if text == super::NILE_SYSTEM_TOO_OLD => return Ok(false),
        Err(StoreError::ToolFailed { reason, .. }) if reason == TIMED_OUT => {
            return Err(tool_failed(TIMED_OUT))
        }
        Err(other) => return Err(other),
    };
    if exit.has(&ToolSignal::NotInstalled) {
        return Ok(false);
    }
    match exit.code {
        // An uncaught exception (the manifest is missing): installed.json
        // and the files are untouched.
        Some(code) if code > 0 => Ok(false),
        _ => match step_failure(&exit) {
            Some(error) => Err(error),
            None => Ok(true),
        },
    }
}

/// Uninstalls an Amazon game: nile's `uninstall` (the manifest's files),
/// then GRID drops the game from nile's `installed.json` and its manifest
/// when nile didn't, then what is left of the folder (the Wine prefix
/// inside it with it) through the library's removal guard, and the row. A
/// folder the guard refuses stays on disk (the row still goes). Blocking.
pub fn uninstall(svc: &AmazonService, registry: &Registry, id: &str) -> Result<(), StoreError> {
    let row = registry
        .find_store(StoreKind::Amazon, id.trim())?
        .ok_or_else(|| StoreError::Refused("That game isn't installed.".to_string()))?;
    let removed = nile_uninstall(svc, &row.store_id)?;
    if !removed {
        list::forget_installed(&svc.config_dir(), &row.store_id).map_err(|e| {
            StoreError::Refused(format!(
                "Couldn't update nile's list of installed games ({}).",
                e.kind()
            ))
        })?;
    }
    remove_guarded(&row, &game_dir(&row))?;
    registry.remove_store(StoreKind::Amazon, &row.store_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    fn exit(code: i32, signals: Vec<ToolSignal>) -> ToolExit {
        ToolExit {
            code: Some(code),
            stdout: SecretString::from("FAKE-STORE-TOKEN-not-real"),
            signals,
        }
    }

    #[test]
    fn every_transfer_names_its_exact_folder_and_never_debugs() {
        let dir = Path::new("/games/Amazon Games/Fogline");
        let id = "amzn1.adg.product.aaaa";
        assert_eq!(
            install_args(id, dir),
            ["install", "--path", "/games/Amazon Games/Fogline", id]
        );
        assert_eq!(update_args(id), ["update", id]);
        assert_eq!(verify_args(id), ["verify", id]);
        assert_eq!(uninstall_args(id), ["uninstall", id]);
        for args in [
            install_args(id, dir),
            update_args(id),
            verify_args(id),
            uninstall_args(id),
        ] {
            assert!(
                !args.iter().any(|a| a == "-d" || a == "--base-path"),
                "{args:?}"
            );
        }
    }

    #[test]
    fn folder_names_drop_niles_banned_characters() {
        assert_eq!(folder_name("Fogline", "x"), "Fogline");
        assert_eq!(
            folder_name("Kettle Drum™: The \"Return\" / Part 2?", "x"),
            "Kettle Drum The Return  Part 2"
        );
        assert_eq!(folder_name(" Lowsun® ", "x"), "Lowsun");
        assert_eq!(
            folder_name("???", "amzn1.adg.product.aaaa"),
            "amzn1.adg.product.aaaa"
        );
        assert_eq!(folder_name("...", ":amzn1:"), "amzn1");
    }

    #[test]
    fn a_transfer_fails_on_signals_even_with_exit_0() {
        assert!(step_failure(&exit(0, vec![])).is_none());
        assert!(step_failure(&exit(0, vec![ToolSignal::DownloadSize(0)])).is_none());
        let cases: [(i32, Vec<ToolSignal>, &str); 7] = [
            (
                0,
                vec![ToolSignal::NotLoggedIn, ToolSignal::OtherError],
                "login expired",
            ),
            (
                0,
                vec![ToolSignal::OtherError, ToolSignal::NotEnoughSpace],
                "nile stopped: there isn't enough disk space",
            ),
            (
                0,
                vec![ToolSignal::Network],
                "nile stopped: Amazon's download server didn't answer",
            ),
            (
                0,
                vec![ToolSignal::NotInstalled],
                "nile stopped: the game isn't installed",
            ),
            (
                1,
                vec![ToolSignal::OtherError],
                "nile stopped: it exited with code 1",
            ),
            (-15, vec![], "nile stopped: it was stopped (signal 15)"),
            (
                0,
                vec![ToolSignal::OtherError],
                "nile stopped: it reported an error",
            ),
        ];
        for (code, signals, words) in cases {
            let error = step_failure(&exit(code, signals.clone())).unwrap();
            assert_eq!(error.to_string(), words, "{signals:?}");
        }
    }

    fn entry(version: &str) -> AmazonInstalled {
        AmazonInstalled {
            id: "A".into(),
            version: version.into(),
            path: "/g/A".into(),
            size: 10,
        }
    }

    #[test]
    fn the_job_is_done_only_when_installed_json_agrees() {
        let list = [entry("v2")];
        assert!(check_installed(StoreOp::Install, "A", &list, "", false).is_ok());
        assert!(check_installed(StoreOp::Install, "B", &list, "", false).is_err());
        assert!(check_installed(StoreOp::Install, "A", &[entry("")], "", false).is_err());
        assert!(check_installed(StoreOp::Update, "A", &list, "v1", false).is_ok());
        let error = check_installed(StoreOp::Update, "A", &list, "v2", false).unwrap_err();
        assert!(error.to_string().contains("before it finished"), "{error}");
        assert!(
            check_installed(StoreOp::Update, "A", &list, "v2", true).is_ok(),
            "already up to date"
        );
        assert!(check_installed(StoreOp::Repair, "A", &list, "v2", false).is_ok());
    }

    #[test]
    fn progress_is_niles_bytes_else_the_info_size() {
        assert_eq!(progress_of(50, 100, 999), (50, 100));
        assert_eq!(progress_of(0, 0, 999), (0, 999));
        assert_eq!(progress_of(150, 100, 0), (100, 100));
        assert_eq!(progress_of(5, 0, 0), (0, 0));
    }
}
