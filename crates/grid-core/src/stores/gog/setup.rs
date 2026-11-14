//! A gogdl install's setup steps (user decisions 2026-11-14,
//! `docs/external-stores-plan.md`; research
//! `docs/research/store-redistributables.md` §1): for a Windows build only,
//! after its files are on disk, run silently in the game's prefix:
//!
//! 1. **The setup script**, for the base game and each installed DLC: GOG's
//!    script interpreter (`__redist/ISI/scriptinterpreter.exe`) when the
//!    build manifest says `scriptInterpreter: true`, else the product's
//!    `temp_executable` from its support files
//!    (`<support>/<product id>/`); a v1 manifest's `support_commands`.
//!    These write the registry keys, CD keys and language settings a game
//!    expects.
//! 2. **The redistributables** the manifest lists (`dependencies`; v1:
//!    `product.depots[].redist`), only those whose repository entry is an
//!    installer under `__redist/` that isn't internal. Never an empty-path
//!    entry (DOSBox, ScummVM, nGlide: gogdl already put those in the game
//!    folder), never bare ISI, QuickTime (not silent), Foxit or AdobeAir.
//!
//! The installers come from `gogdl redist` into one shared folder,
//! [`GogService::redist_dir`]. gogdl deletes what a call doesn't list, so
//! every call names the full set: ISI plus every installed GOG game's
//! dependencies ([`redist_ids`]). Each item's program and silent arguments
//! come from the repository manifest gogdl writes there
//! ([`REDIST_MANIFEST`]); a `.msi` runs through `msiexec` (the shared
//! runner does that).
//!
//! When: the last step of an install and of a DLC add (script then
//! redistributables); after an update only the script runs again (its id
//! carries the build, so the prefix marker doesn't skip it). A repair runs
//! none. Retry on a setup-step warning is [`setup_steps_job`]. The running,
//! the skipping of what is already installed, the Windows prompt and the
//! warnings are `stores::redist`'s. Nothing here is a secret: the manifest's
//! client secret is never read.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use super::install::game_dir;
use super::meta::{GogMeta, GogOrigin, GogPlatform};
use super::{tool_exit_error, GogJobCtx, GogService};
use crate::launch::template::split_template;
use crate::library::queue::{JobPhase, StoreJob, StoreProgressSink};
use crate::library::registry::{InstalledGame, Registry};
use crate::library::LibraryError;
use crate::stores::offline::exec::windows_path;
use crate::stores::progress::ProgressMerger;
use crate::stores::redist::{resolve_in, run_steps, RedistStep, RedistTarget, REDIST_CANCELLED};
use crate::stores::runner::run_tool;
use crate::stores::setup_steps::{self, SetupStepsStore, NO_SETUP_STEPS};
use crate::stores::{GameKey, StoreError, StoreKind};

/// The shared redistributables folder, under gogdl's own folder.
pub const REDIST_DIR: &str = "redist";
/// The repository manifest `gogdl redist` writes in that folder.
pub const REDIST_MANIFEST: &str = ".gogdl-redist-manifest";
/// GOG's script interpreter's dependency id.
pub const ISI: &str = "ISI";
/// The script interpreter, relative to the redistributables folder.
pub const ISI_EXE: &str = "__redist/ISI/scriptinterpreter.exe";
/// Dependency ids GRID never runs, whatever a manifest lists: bare ISI
/// (only as the script step), QuickTime (its installer isn't silent), and
/// the Foxit and AdobeAir extras.
pub const NEVER_RUN: [&str; 4] = [ISI, "QuickTime", "Foxit", "AdobeAir"];
/// The start of a setup-script step's id: `gog-script:<product>:<build>`.
pub const SCRIPT_ID: &str = "gog-script:";

// --- the build manifest --------------------------------------------------------

/// What GRID reads from gogdl's saved build manifest
/// (`heroic_gogdl/manifests/<id>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildManifest {
    /// 1 (legacy) or 2.
    pub version: u8,
    /// v1: `product.timestamp` (Heroic's `/buildId` for a v1 script).
    pub timestamp: String,
    pub script_interpreter: bool,
    /// v2 `products`.
    pub products: Vec<ManifestProduct>,
    /// v2 top-level `dependencies`, v1 `product.depots[].redist`, in order,
    /// each once.
    pub dependencies: Vec<String>,
    /// v1 `product.support_commands`.
    pub support_commands: Vec<SupportCommand>,
}

/// One v2 product (the base game or a DLC).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestProduct {
    pub product_id: String,
    /// `""` for none.
    pub temp_executable: String,
}

/// One v1 support command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupportCommand {
    pub languages: Vec<String>,
    pub executable: String,
    pub game_id: String,
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn push_once(list: &mut Vec<String>, id: String) {
    if !id.is_empty() && !list.contains(&id) {
        list.push(id);
    }
}

/// Parses a build manifest; `None` when it isn't one.
pub fn parse_build_manifest(raw: &str) -> Option<BuildManifest> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    let mut manifest = BuildManifest::default();
    if let Some(product) = object.get("product").filter(|p| p.is_object()) {
        manifest.version = 1;
        manifest.timestamp = text(product.get("timestamp"));
        for depot in product
            .get("depots")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            push_once(&mut manifest.dependencies, text(depot.get("redist")));
        }
        manifest.support_commands = product
            .get("support_commands")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|c| SupportCommand {
                languages: c
                    .get("languages")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|l| text(Some(l)))
                    .filter(|l| !l.is_empty())
                    .collect(),
                executable: text(c.get("executable")),
                game_id: text(c.get("gameID")),
            })
            .filter(|c| !c.executable.is_empty() && !c.game_id.is_empty())
            .collect();
        return Some(manifest);
    }
    manifest.version = 2;
    manifest.script_interpreter = object
        .get("scriptInterpreter")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    manifest.products = object
        .get("products")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|p| ManifestProduct {
            product_id: text(p.get("productId")),
            temp_executable: text(p.get("temp_executable")),
        })
        .filter(|p| !p.product_id.is_empty())
        .collect();
    for id in object
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        push_once(&mut manifest.dependencies, text(Some(id)));
    }
    Some(manifest)
}

/// The build manifest at `path`, `None` when missing or unreadable.
pub fn read_build_manifest(path: &Path) -> Option<BuildManifest> {
    parse_build_manifest(&fs::read_to_string(path).ok()?)
}

// --- the redistributables repository ------------------------------------------

/// One item of GOG's dependency repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoItem {
    pub id: String,
    /// Relative to the redistributables folder; `""` for a game-folder
    /// item.
    pub path: String,
    /// The silent arguments, as one command line.
    pub arguments: String,
    pub internal: bool,
}

/// The repository manifest's items (`depots`), empty when it isn't one.
pub fn parse_repository(raw: &str) -> Vec<RepoItem> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    value
        .get("depots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|d| RepoItem {
            id: text(d.get("dependencyId")),
            path: text(d.get("executable").and_then(|e| e.get("path"))),
            arguments: text(d.get("executable").and_then(|e| e.get("arguments"))),
            internal: d.get("internal").and_then(Value::as_bool).unwrap_or(false),
        })
        .filter(|d| !d.id.is_empty())
        .collect()
}

/// The repository manifest in `redist_dir`.
pub fn read_repository(redist_dir: &Path) -> Vec<RepoItem> {
    fs::read_to_string(redist_dir.join(REDIST_MANIFEST))
        .map(|raw| parse_repository(&raw))
        .unwrap_or_default()
}

fn never_run(id: &str) -> bool {
    NEVER_RUN.iter().any(|n| n.eq_ignore_ascii_case(id.trim()))
}

/// The items of `deps` GRID runs, in the manifest's order: in the
/// repository, an installer under `__redist/`, not internal, and not one
/// of [`NEVER_RUN`]. An id the repository doesn't have is left out.
pub fn runnable_dependencies<'a>(deps: &[String], repo: &'a [RepoItem]) -> Vec<&'a RepoItem> {
    let mut out: Vec<&RepoItem> = Vec::new();
    for id in deps {
        if never_run(id) {
            continue;
        }
        let Some(item) = repo.iter().find(|item| item.id == id.trim()) else {
            continue;
        };
        let path = item.path.trim().replace('\\', "/");
        if path.is_empty() || !path.starts_with("__redist/") || item.internal {
            continue;
        }
        if !out.iter().any(|seen| seen.id == item.id) {
            out.push(item);
        }
    }
    out
}

/// `--ids` for `gogdl redist`: ISI, then every id of `lists` (every
/// installed GOG game's dependencies), each once. Always the full set:
/// gogdl deletes what it isn't given.
pub fn redist_ids<'a>(lists: impl IntoIterator<Item = &'a [String]>) -> Vec<String> {
    let mut ids = vec![ISI.to_string()];
    for list in lists {
        for id in list {
            push_once(&mut ids, id.trim().to_string());
        }
    }
    ids
}

/// `redist --ids <a,b> --path <dir>` (gogdl's argv adds
/// `--auth-config-path` in front).
pub fn redist_args(ids: &[String], dir: &Path) -> Vec<String> {
    vec![
        "redist".to_string(),
        "--ids".to_string(),
        ids.join(","),
        "--path".to_string(),
        dir.to_string_lossy().into_owned(),
    ]
}

// --- the steps -----------------------------------------------------------------

/// The English name GOG's scripts take for a language code (`de-DE` →
/// `German`); English for one GRID doesn't know.
pub fn language_name(code: &str) -> &'static str {
    let base = code.trim().split(['-', '_']).next().unwrap_or("");
    match base.to_lowercase().as_str() {
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pl" => "Polish",
        "ru" => "Russian",
        "pt" => "Portuguese",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "cs" => "Czech",
        "hu" => "Hungarian",
        "nl" => "Dutch",
        "tr" => "Turkish",
        "sv" => "Swedish",
        "da" => "Danish",
        "fi" => "Finnish",
        "no" | "nb" => "Norwegian",
        "uk" => "Ukrainian",
        "ar" => "Arabic",
        "ro" => "Romanian",
        "el" => "Greek",
        "th" => "Thai",
        "bg" => "Bulgarian",
        _ => "English",
    }
}

/// What the script step's arguments are made of.
#[derive(Debug, Clone, Copy)]
pub struct ScriptContext<'a> {
    pub game_dir: &'a Path,
    /// `<gogdl>/support/<id>`: the temp executables and the script files.
    pub support_dir: &'a Path,
    pub redist_dir: &'a Path,
    pub base_id: &'a str,
    pub dlc_ids: &'a [String],
    /// The install language code (`en-US`).
    pub language: &'a str,
    pub version: &'a str,
    /// The installed build (v2).
    pub build_id: &'a str,
    /// Paths as they are (a Windows host, or a ScummVM game); else Wine's
    /// `Z:\…` view.
    pub native: bool,
}

impl ScriptContext<'_> {
    fn installed(&self, product: &str) -> bool {
        product == self.base_id.trim() || self.dlc_ids.iter().any(|d| d.trim() == product)
    }

    fn path(&self, path: &Path) -> String {
        windows_path(path, self.native)
    }

    fn code(&self) -> String {
        match self.language.trim() {
            "" => "en-US".to_string(),
            code => code.to_string(),
        }
    }
}

/// A script step's id: what it installs for, and the build it was for (so
/// an update runs it again).
pub fn script_id(product: &str, build: &str) -> String {
    format!("{SCRIPT_ID}{}:{}", product.trim(), build.trim())
}

/// Whether `step` is a setup-script step.
pub fn is_script(step: &RedistStep) -> bool {
    step.id.starts_with(SCRIPT_ID)
}

const SHORTCUTS: [&str; 2] = ["/nodesktopshorctut", "/nodesktopshortcut"];

/// The setup-script steps, base game first, then each installed DLC in the
/// manifest's order (Heroic's arguments, both spellings of the shortcut
/// flag included: GOG's scripts have the typo).
pub fn script_steps(manifest: &BuildManifest, ctx: &ScriptContext<'_>) -> Vec<RedistStep> {
    let dir = format!("/DIR={}", ctx.path(ctx.game_dir));
    let mut steps = Vec::new();
    if manifest.version == 1 {
        let name = language_name(ctx.language);
        let lower = name.to_lowercase();
        for command in &manifest.support_commands {
            let wanted = command.languages.iter().any(|l| {
                l.eq_ignore_ascii_case("Neutral")
                    || l.eq_ignore_ascii_case(name)
                    || l.eq_ignore_ascii_case(ctx.language.trim())
            });
            if !wanted || !ctx.installed(&command.game_id) {
                continue;
            }
            let mut args = vec![
                "/VERYSILENT".to_string(),
                dir.clone(),
                format!("/Language={lower}"),
                format!("/LANG={lower}"),
                format!("/ProductId={}", command.game_id),
                "/galaxyclient".to_string(),
                format!("/buildId={}", manifest.timestamp),
                format!("/versionName={}", ctx.version.trim()),
            ];
            args.extend(SHORTCUTS.iter().map(|s| s.to_string()));
            steps.push(RedistStep {
                id: script_id(&command.game_id, &manifest.timestamp),
                command: format!(
                    "{}/{}",
                    command.game_id,
                    command.executable.trim_start_matches(['/', '\\'])
                ),
                args,
                root: ctx.support_dir.to_string_lossy().into_owned(),
                ..Default::default()
            });
        }
        return steps;
    }
    let mut products: Vec<&ManifestProduct> = manifest
        .products
        .iter()
        .filter(|p| ctx.installed(&p.product_id))
        .collect();
    // The base game first.
    products.sort_by_key(|p| p.product_id != ctx.base_id.trim());
    let name = language_name(ctx.language);
    for product in products {
        let pid = product.product_id.as_str();
        let common =
            |language: &str| vec![format!("/Language={language}"), format!("/LANG={language}")];
        if manifest.script_interpreter {
            let mut args = vec!["/VERYSILENT".to_string(), dir.clone()];
            args.extend(common(name));
            args.extend([
                format!("/ProductId={pid}"),
                "/galaxyclient".to_string(),
                format!("/buildId={}", ctx.build_id.trim()),
                format!("/versionName={}", ctx.version.trim()),
                format!("/lang-code={}", ctx.code()),
                format!("/supportDir={}", ctx.path(ctx.support_dir)),
            ]);
            args.extend(SHORTCUTS.iter().map(|s| s.to_string()));
            steps.push(RedistStep {
                id: script_id(pid, ctx.build_id),
                command: ISI_EXE.to_string(),
                args,
                root: ctx.redist_dir.to_string_lossy().into_owned(),
                ..Default::default()
            });
        } else if !product.temp_executable.is_empty() {
            let mut args = vec!["/VERYSILENT".to_string(), dir.clone()];
            args.extend(common(&name.to_lowercase()));
            args.extend([
                format!("/lang-code={}", ctx.code()),
                format!("/ProductId={pid}"),
                "/galaxyclient".to_string(),
                format!("/buildId={}", ctx.build_id.trim()),
                format!("/versionName={}", ctx.version.trim()),
            ]);
            args.extend(SHORTCUTS.iter().map(|s| s.to_string()));
            steps.push(RedistStep {
                id: script_id(pid, ctx.build_id),
                command: format!("{pid}/{}", product.temp_executable),
                args,
                root: ctx.support_dir.to_string_lossy().into_owned(),
                ..Default::default()
            });
        }
    }
    steps
}

/// The redistributable steps for `deps`: [`runnable_dependencies`], each
/// its repository program in `redist_dir` with its silent arguments.
pub fn dependency_steps(deps: &[String], repo: &[RepoItem], redist_dir: &Path) -> Vec<RedistStep> {
    runnable_dependencies(deps, repo)
        .into_iter()
        .map(|item| RedistStep {
            id: item.id.clone(),
            command: item.path.trim().to_string(),
            args: split_template(&item.arguments).unwrap_or_default(),
            root: redist_dir.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .collect()
}

/// Which steps a job runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The script, then the redistributables (install, DLC add, Retry).
    Full,
    /// The script only (an update).
    ScriptOnly,
}

/// The ids `scope` needs in the shared folder: ISI for a script-interpreter
/// build, plus (full) the manifest's dependencies.
pub fn needed_ids(manifest: &BuildManifest, scope: Scope) -> Vec<String> {
    let mut ids = Vec::new();
    if manifest.version == 2 && manifest.script_interpreter {
        ids.push(ISI.to_string());
    }
    if scope == Scope::Full {
        for id in &manifest.dependencies {
            if !never_run(id) {
                push_once(&mut ids, id.clone());
            }
        }
    }
    ids
}

/// Whether the shared folder lacks any of `needed`: no repository manifest
/// yet, or an installer it lists for one of them isn't on disk. An id the
/// repository doesn't have (or a game-folder item) needs nothing.
pub fn redist_missing(redist_dir: &Path, needed: &[String]) -> bool {
    if needed.is_empty() {
        return false;
    }
    let repo = read_repository(redist_dir);
    if repo.is_empty() {
        return true;
    }
    needed.iter().any(|id| {
        repo.iter()
            .find(|item| item.id == *id)
            .map(|item| item.path.trim().replace('\\', "/"))
            .filter(|path| path.starts_with("__redist/"))
            .is_some_and(|path| !redist_dir.join(path).is_file())
    })
}

/// Whether `row` is a game GOG's setup steps are for: a gogdl install of a
/// Windows build.
pub fn has_setup_steps(meta: &GogMeta) -> bool {
    meta.origin == GogOrigin::Gogdl && meta.platform == GogPlatform::Windows
}

// --- running -------------------------------------------------------------------

impl GogService {
    /// The shared folder `gogdl redist` downloads the installers into.
    pub fn redist_dir(&self) -> PathBuf {
        self.tool_dir().join(REDIST_DIR)
    }

    /// Every installed gogdl (Windows) game's dependencies: one list per
    /// saved build manifest.
    fn installed_dependencies(&self) -> Vec<Vec<String>> {
        let Ok(entries) = fs::read_dir(self.manifests_dir()) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        paths
            .iter()
            .filter_map(|path| read_build_manifest(path))
            .map(|m| m.dependencies)
            .collect()
    }

    /// `gogdl redist` with the full set into [`Self::redist_dir`]; its
    /// progress (gogdl's own lines) goes to `progress`. Blocking.
    pub fn fetch_redist(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<(), StoreError> {
        let lists = self.installed_dependencies();
        let ids = redist_ids(lists.iter().map(Vec::as_slice));
        let dir = self.redist_dir();
        fs::create_dir_all(&dir).map_err(|e| {
            StoreError::Refused(format!(
                "Couldn't create GRID's GOG redistributables folder ({}).",
                e.kind()
            ))
        })?;
        let cmd = self.gogdl(redist_args(&ids, &dir))?;
        let mut merger = ProgressMerger::default();
        let exit = run_tool(&cmd, cancel, &mut |report| {
            let merged = merger.push(report);
            progress(merged.done, merged.total);
        })?;
        if !exit.success() {
            return Err(tool_exit_error(exit.code));
        }
        if !dir.join(REDIST_MANIFEST).is_file() {
            return Err(StoreError::ToolFailed {
                tool: crate::stores::tools::StoreTool::Gogdl.name(),
                reason: "it didn't list the redistributables".to_string(),
            });
        }
        Ok(())
    }

    /// `row`'s steps for `scope` from its saved build `manifest`.
    fn steps_from(
        &self,
        row: &InstalledGame,
        meta: &GogMeta,
        manifest: &BuildManifest,
        scope: Scope,
    ) -> Vec<RedistStep> {
        let dir = game_dir(row);
        let support = self.support_dir(&row.store_id);
        let redist = self.redist_dir();
        let dlc_ids: Vec<String> = meta.dlc.iter().map(|d| d.id.clone()).collect();
        let scummvm = manifest
            .dependencies
            .iter()
            .any(|d| d.eq_ignore_ascii_case("ScummVM"));
        let ctx = ScriptContext {
            game_dir: &dir,
            support_dir: &support,
            redist_dir: &redist,
            base_id: row.store_id.trim(),
            dlc_ids: &dlc_ids,
            language: &meta.language,
            version: &meta.version,
            build_id: &meta.build_id,
            native: self.redist.host.trim().to_lowercase().starts_with("win") || scummvm,
        };
        let mut steps = script_steps(manifest, &ctx);
        if scope == Scope::Full {
            steps.extend(dependency_steps(
                &manifest.dependencies,
                &read_repository(&redist),
                &redist,
            ));
        }
        steps
    }

    fn manifest_of(&self, row: &InstalledGame) -> Option<BuildManifest> {
        read_build_manifest(&self.manifests_dir().join(row.store_id.trim()))
    }
}

/// How a job's setup steps went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupOutcome {
    /// The finished entry's warning lines (`""`: none).
    pub warning: String,
    /// The steps that ran and all worked (or were already satisfied), to
    /// record in [`GogMeta::setup_ran`]; `None` after a warning or when
    /// there were none.
    pub ran: Option<Vec<RedistStep>>,
}

/// The warning when `gogdl redist` failed.
fn fetch_warning(error: &StoreError) -> String {
    format!("Couldn't run its setup steps: GRID couldn't download them ({error})")
}

/// Runs `row`'s setup steps for `scope` after a job wrote the row: fetches
/// what the shared folder lacks (cancellable, "Running the installer
/// silently"), then [`run_steps`] in the game's prefix. Never fails: every
/// problem is a warning line. Blocking.
pub fn run_after_job(
    svc: &GogService,
    row: &InstalledGame,
    meta: &GogMeta,
    scope: Scope,
    default_compat: &str,
    sink: &mut dyn StoreProgressSink,
) -> SetupOutcome {
    if !has_setup_steps(meta) {
        return SetupOutcome::default();
    }
    let Some(manifest) = svc.manifest_of(row) else {
        return SetupOutcome::default();
    };
    let mut warnings: Vec<String> = Vec::new();
    let needed = needed_ids(&manifest, scope);
    let redist = svc.redist_dir();
    let mut fetched = true;
    if redist_missing(&redist, &needed) {
        sink.phase(JobPhase::InstallerSilent);
        sink.progress(0, 1);
        let cancel = AtomicBool::new(false);
        let result = svc.fetch_redist(&cancel, &mut |done, total| {
            if sink.cancelled() {
                cancel.store(true, Ordering::Relaxed);
            }
            if total > 0 {
                sink.progress(done, total);
            }
        });
        if sink.cancelled() || matches!(result, Err(StoreError::Library(LibraryError::Cancelled))) {
            return SetupOutcome {
                warning: REDIST_CANCELLED.to_string(),
                ran: None,
            };
        }
        if let Err(error) = result {
            warnings.push(fetch_warning(&error));
            fetched = false;
        }
    }
    let dir = game_dir(row);
    let mut steps = svc.steps_from(row, meta, &manifest, scope);
    if !fetched {
        // Only what is on disk runs; the warning says the rest didn't.
        steps.retain(|step| resolve_in(step, &dir).is_ok());
    }
    if steps.is_empty() {
        return SetupOutcome {
            warning: warnings.join("\n"),
            ran: None,
        };
    }
    let compat = match row.native_compat_tool.trim() {
        "" => default_compat.trim().to_string(),
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
        &svc.redist,
        sink,
    );
    let worked = outcome.all_worked && warnings.is_empty();
    warnings.extend(outcome.warnings);
    SetupOutcome {
        warning: warnings.join("\n"),
        ran: worked.then_some(steps),
    }
}

/// `meta.setup_ran` after `ran` worked for `scope`: the full list, or (an
/// update) the new script steps in place of the old ones.
pub fn record_ran(meta: &mut GogMeta, ran: Vec<RedistStep>, scope: Scope) {
    match scope {
        Scope::Full => meta.setup_ran = ran,
        Scope::ScriptOnly => {
            let rest: Vec<RedistStep> = meta
                .setup_ran
                .iter()
                .filter(|step| !is_script(step))
                .cloned()
                .collect();
            meta.setup_ran = ran;
            meta.setup_ran.extend(rest);
        }
    }
}

// --- Retry: the setup-steps job ----------------------------------------------

/// Where GOG's setup-steps job reads and records a game's steps.
struct GogSetupSteps {
    svc: GogService,
    registry: Arc<Registry>,
    id: String,
}

impl GogSetupSteps {
    fn row(&self) -> Result<Option<InstalledGame>, LibraryError> {
        self.registry
            .find_key(&GameKey::store(StoreKind::Gog, self.id.clone()))
    }
}

impl SetupStepsStore for GogSetupSteps {
    /// The script and redistributable steps, re-read from the saved build
    /// manifest; the shared folder is fetched again only when it lacks one
    /// of them.
    fn steps(&self) -> Result<Vec<RedistStep>, LibraryError> {
        let Some(row) = self.row()? else {
            return Ok(Vec::new());
        };
        let meta = GogMeta::from_json(&row.store_meta);
        let manifest = self.svc.manifest_of(&row).ok_or_else(no_manifest)?;
        if redist_missing(&self.svc.redist_dir(), &needed_ids(&manifest, Scope::Full)) {
            self.svc
                .fetch_redist(&AtomicBool::new(false), &mut |_, _| {})
                .map_err(|error| LibraryError::Extract(fetch_warning(&error)))?;
        }
        Ok(self.svc.steps_from(&row, &meta, &manifest, Scope::Full))
    }

    fn record(&self, ran: &[RedistStep]) -> Result<(), LibraryError> {
        let Some(mut row) = self.row()? else {
            return Ok(());
        };
        let mut meta = GogMeta::from_json(&row.store_meta);
        record_ran(&mut meta, ran.to_vec(), Scope::Full);
        row.store_meta = meta.to_json();
        self.registry.upsert_store(&row)
    }
}

fn no_manifest() -> LibraryError {
    LibraryError::Extract(
        "GRID can't run its setup steps: gogdl has no record of its files. Verify and repair it."
            .to_string(),
    )
}

/// The job that runs only `row`'s setup steps (no game download, no
/// login; `gogdl redist` only when the shared folder lacks an installer).
/// Refuses a game that isn't a gogdl Windows install
/// ([`NO_SETUP_STEPS`]) or whose build lists no steps.
pub fn setup_steps_job(
    svc: &GogService,
    row: &InstalledGame,
    ctx: GogJobCtx,
) -> Result<StoreJob, StoreError> {
    let meta = GogMeta::from_json(&row.store_meta);
    if !has_setup_steps(&meta) {
        return Err(StoreError::Refused(NO_SETUP_STEPS.to_string()));
    }
    let manifest = svc
        .manifest_of(row)
        .ok_or_else(|| StoreError::Library(no_manifest()))?;
    let none = script_steps_count(svc, row, &meta, &manifest) == 0
        && runnable_count(&manifest, &svc.redist_dir()) == 0;
    if none {
        return Err(StoreError::Refused(format!(
            "{} has no setup steps to run.",
            row.title.trim()
        )));
    }
    let store = GogSetupSteps {
        svc: svc.clone(),
        registry: ctx.registry.clone(),
        id: row.store_id.trim().to_string(),
    };
    Ok(setup_steps::setup_steps_job(
        StoreKind::Gog,
        row,
        &ctx.default_compat_tool,
        Arc::new(store),
        svc.redist.clone(),
    ))
}

fn script_steps_count(
    svc: &GogService,
    row: &InstalledGame,
    meta: &GogMeta,
    manifest: &BuildManifest,
) -> usize {
    svc.steps_from(row, meta, manifest, Scope::ScriptOnly).len()
}

/// How many of the manifest's dependencies could run: with no repository
/// yet, every one GRID doesn't refuse by id.
fn runnable_count(manifest: &BuildManifest, redist_dir: &Path) -> usize {
    let repo = read_repository(redist_dir);
    if repo.is_empty() {
        return needed_ids(manifest, Scope::Full)
            .iter()
            .filter(|id| id.as_str() != ISI)
            .count();
    }
    runnable_dependencies(&manifest.dependencies, &repo).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const V2: &str = r#"{"version": 2, "baseProductId": "100", "buildId": "77", "clientSecret": "not-read",
      "dependencies": ["DirectX", "MSVC2019_x64", "openAL", "DOSBox074", "ISI", "QuickTime", "Foxit", "AdobeAir", "openAL", "Missing"],
      "scriptInterpreter": true,
      "products": [{"productId": "101", "name": "DLC", "temp_executable": "", "temp_arguments": ""},
                   {"productId": "100", "name": "Base", "temp_executable": "", "temp_arguments": ""},
                   {"productId": "102", "name": "Other DLC", "temp_executable": "", "temp_arguments": ""}]}"#;

    const REPO: &str = r#"{"build_id": "601", "HGLInstalled": ["ISI"], "depots": [
      {"dependencyId": "ISI", "executable": {"path": "__redist/ISI/scriptinterpreter.exe", "arguments": ""}, "internal": true},
      {"dependencyId": "DirectX", "executable": {"path": "__redist/DirectX/DXSETUP.exe", "arguments": "/silent"}, "internal": false},
      {"dependencyId": "MSVC2019_x64", "executable": {"path": "__redist/MSVC2019_x64/VC_redist.x64.exe", "arguments": "/install /quiet /norestart"}, "internal": false},
      {"dependencyId": "MSVC2005", "executable": {"path": "__redist/MSVC2005/vcredist_x86.exe", "arguments": "/q:a /c:\"VCREDI~3.EXE /q:a /c:\"\"msiexec /i vcredist.msi /qb\"\" \""}, "internal": false},
      {"dependencyId": "openAL", "executable": {"path": "__redist/openAL/oalinst.exe", "arguments": "/S"}, "internal": false},
      {"dependencyId": "XNA_40", "executable": {"path": "__redist/XNA_40/xnafx40_redist.msi", "arguments": "/qn"}, "internal": false},
      {"dependencyId": "DOSBox074", "executable": {"path": "", "arguments": ""}, "internal": true},
      {"dependencyId": "QuickTime", "executable": {"path": "__redist/QuickTime/QuickTimeInstaller.exe", "arguments": ""}, "internal": true},
      {"dependencyId": "Foxit", "executable": {"path": "__redist/Foxit/FoxitReader.exe", "arguments": "/VERYSILENT"}, "internal": true},
      {"dependencyId": "AdobeAir", "executable": {"path": "__redist/AdobeAir/AIRRuntimeInstaller.exe", "arguments": "-silent"}, "internal": false},
      {"dependencyId": "Hidden", "executable": {"path": "__redist/Hidden/x.exe", "arguments": ""}, "internal": true}
    ]}"#;

    fn ids(items: &[&RepoItem]) -> Vec<String> {
        items.iter().map(|i| i.id.clone()).collect()
    }

    #[test]
    fn manifests_of_both_versions_parse() {
        let v2 = parse_build_manifest(V2).unwrap();
        assert_eq!(v2.version, 2);
        assert!(v2.script_interpreter);
        assert_eq!(v2.products.len(), 3);
        assert_eq!(
            v2.dependencies.len(),
            9,
            "each id once: {:?}",
            v2.dependencies
        );
        let bare = parse_build_manifest(r#"{"version": 2, "products": []}"#).unwrap();
        assert!(bare.dependencies.is_empty(), "no key is no dependencies");
        assert!(!bare.script_interpreter);

        let v1 = parse_build_manifest(
            r#"{"version": 1, "product": {"timestamp": 1500000000, "depots": [
              {"languages": ["English"], "gameIDs": ["100"], "manifest": "m"},
              {"redist": "DirectX", "executable": "x", "argument": "", "size": "1"},
              {"redist": "MSVC2008", "executable": "y", "argument": "", "size": "1"}],
              "support_commands": [{"languages": ["Neutral"], "executable": "/support.exe", "gameID": "100", "argument": "", "systems": ["Windows"]}]}}"#,
        )
        .unwrap();
        assert_eq!(v1.version, 1);
        assert_eq!(v1.timestamp, "1500000000");
        assert_eq!(v1.dependencies, ["DirectX", "MSVC2008"]);
        assert_eq!(v1.support_commands[0].game_id, "100");
        assert!(parse_build_manifest("nope").is_none());
    }

    #[test]
    fn only_silent_redist_installers_run() {
        let v2 = parse_build_manifest(V2).unwrap();
        let repo = parse_repository(REPO);
        assert_eq!(repo.len(), 11);
        // Manifest order; no empty path, no ISI, no internal extras, no
        // AdobeAir even when not internal, no id the repository lacks.
        assert_eq!(
            ids(&runnable_dependencies(&v2.dependencies, &repo)),
            ["DirectX", "MSVC2019_x64", "openAL"]
        );
        let listed = vec!["Hidden".to_string(), "XNA_40".to_string()];
        assert_eq!(ids(&runnable_dependencies(&listed, &repo)), ["XNA_40"]);

        let steps = dependency_steps(&v2.dependencies, &repo, Path::new("/r"));
        assert_eq!(steps[1].id, "MSVC2019_x64");
        assert_eq!(steps[1].command, "__redist/MSVC2019_x64/VC_redist.x64.exe");
        assert_eq!(steps[1].args, ["/install", "/quiet", "/norestart"]);
        assert_eq!(steps[1].root, "/r");
        // VC++ 2005's nested quotes split as Python's shlex does.
        let vc5 = dependency_steps(&["MSVC2005".to_string()], &repo, Path::new("/r"));
        assert_eq!(
            vc5[0].args,
            [
                "/q:a",
                "/c:VCREDI~3.EXE /q:a /c:msiexec /i vcredist.msi /qb "
            ]
        );
    }

    #[test]
    fn the_redist_call_always_names_the_full_set() {
        let a = vec!["DirectX".to_string(), "openAL".to_string()];
        let b = vec![
            "openAL".to_string(),
            "MSVC2019_x64".to_string(),
            "ISI".into(),
        ];
        let none: Vec<String> = Vec::new();
        let all = redist_ids([a.as_slice(), none.as_slice(), b.as_slice()]);
        assert_eq!(all, ["ISI", "DirectX", "openAL", "MSVC2019_x64"]);
        assert_eq!(redist_ids(std::iter::empty::<&[String]>()), ["ISI"]);
        assert_eq!(
            redist_args(&all, Path::new("/t/redist")),
            [
                "redist",
                "--ids",
                "ISI,DirectX,openAL,MSVC2019_x64",
                "--path",
                "/t/redist"
            ]
        );
    }

    fn ctx<'a>(dlc: &'a [String], native: bool) -> ScriptContext<'a> {
        ScriptContext {
            game_dir: Path::new("/g/Game"),
            support_dir: Path::new("/t/support/100"),
            redist_dir: Path::new("/t/redist"),
            base_id: "100",
            dlc_ids: dlc,
            language: "de-DE",
            version: "1.5",
            build_id: "77",
            native,
        }
    }

    #[test]
    fn a_script_interpreter_build_runs_isi_for_the_base_then_each_installed_dlc() {
        let v2 = parse_build_manifest(V2).unwrap();
        let dlc = vec!["101".to_string()];
        let steps = script_steps(&v2, &ctx(&dlc, false));
        assert_eq!(steps.len(), 2, "102 isn't installed");
        assert_eq!(steps[0].id, "gog-script:100:77");
        assert_eq!(steps[1].id, "gog-script:101:77");
        assert!(steps.iter().all(is_script));
        assert_eq!(steps[0].command, ISI_EXE);
        assert_eq!(steps[0].root, "/t/redist");
        assert_eq!(
            steps[0].args,
            [
                "/VERYSILENT",
                "/DIR=Z:\\g\\Game",
                "/Language=German",
                "/LANG=German",
                "/ProductId=100",
                "/galaxyclient",
                "/buildId=77",
                "/versionName=1.5",
                "/lang-code=de-DE",
                "/supportDir=Z:\\t\\support\\100",
                "/nodesktopshorctut",
                "/nodesktopshortcut"
            ]
        );
        // On a Windows host the paths stay as they are.
        let native = script_steps(&v2, &ctx(&dlc, true));
        assert_eq!(native[0].args[1], "/DIR=/g/Game");
    }

    #[test]
    fn without_isi_the_temp_executable_runs_and_v1_its_support_commands() {
        let v2 = parse_build_manifest(
            r#"{"version": 2, "products": [{"productId": "100", "temp_executable": "galaxy_setup.exe"},
              {"productId": "101", "temp_executable": ""}]}"#,
        )
        .unwrap();
        let dlc = vec!["101".to_string()];
        let steps = script_steps(&v2, &ctx(&dlc, false));
        assert_eq!(steps.len(), 1, "an empty temp_executable runs nothing");
        assert_eq!(steps[0].command, "100/galaxy_setup.exe");
        assert_eq!(steps[0].root, "/t/support/100");
        assert_eq!(steps[0].args[2], "/Language=german");
        assert!(steps[0].args.contains(&"/ProductId=100".to_string()));
        assert!(!steps[0].args.iter().any(|a| a.starts_with("/supportDir")));

        let v1 = parse_build_manifest(
            r#"{"version": 1, "product": {"timestamp": 15, "depots": [], "support_commands": [
              {"languages": ["Neutral"], "executable": "/setup.exe", "gameID": "100"},
              {"languages": ["French"], "executable": "/fr.exe", "gameID": "100"},
              {"languages": ["German"], "executable": "/de.exe", "gameID": "100"},
              {"languages": ["Neutral"], "executable": "/dlc.exe", "gameID": "999"}]}}"#,
        )
        .unwrap();
        let steps = script_steps(&v1, &ctx(&[], false));
        let commands: Vec<&str> = steps.iter().map(|s| s.command.as_str()).collect();
        assert_eq!(commands, ["100/setup.exe", "100/de.exe"]);
        assert_eq!(steps[0].id, "gog-script:100:15");
        assert!(steps[0].args.contains(&"/buildId=15".to_string()));
        assert!(steps[0].args.contains(&"/Language=german".to_string()));
    }

    #[test]
    fn an_update_reruns_only_the_script_and_records_it_in_place() {
        let v2 = parse_build_manifest(V2).unwrap();
        assert_eq!(needed_ids(&v2, Scope::ScriptOnly), ["ISI"]);
        assert_eq!(
            needed_ids(&v2, Scope::Full),
            [
                "ISI",
                "DirectX",
                "MSVC2019_x64",
                "openAL",
                "DOSBox074",
                "Missing"
            ]
        );
        let no_isi = parse_build_manifest(r#"{"version": 2, "dependencies": ["openAL"]}"#).unwrap();
        assert!(needed_ids(&no_isi, Scope::ScriptOnly).is_empty());

        let old_script = RedistStep {
            id: script_id("100", "77"),
            ..Default::default()
        };
        let dep = RedistStep {
            id: "openAL".into(),
            ..Default::default()
        };
        let mut meta = GogMeta {
            setup_ran: vec![old_script, dep.clone()],
            ..Default::default()
        };
        let new_script = RedistStep {
            id: script_id("100", "78"),
            ..Default::default()
        };
        record_ran(&mut meta, vec![new_script.clone()], Scope::ScriptOnly);
        assert_eq!(meta.setup_ran, [new_script.clone(), dep]);
        record_ran(&mut meta, vec![new_script.clone()], Scope::Full);
        assert_eq!(meta.setup_ran, [new_script]);
    }

    #[test]
    fn the_shared_folder_is_fetched_only_when_it_lacks_an_installer() {
        let dir = tempfile::tempdir().unwrap();
        let redist = dir.path();
        let need = vec![
            "ISI".to_string(),
            "openAL".to_string(),
            "DOSBox074".to_string(),
        ];
        assert!(!redist_missing(redist, &[]));
        assert!(redist_missing(redist, &need), "no repository yet");
        fs::write(redist.join(REDIST_MANIFEST), REPO).unwrap();
        assert!(redist_missing(redist, &need));
        for path in [
            "__redist/ISI/scriptinterpreter.exe",
            "__redist/openAL/oalinst.exe",
        ] {
            fs::create_dir_all(redist.join(path).parent().unwrap()).unwrap();
            fs::write(redist.join(path), b"MZ").unwrap();
        }
        assert!(
            !redist_missing(redist, &need),
            "DOSBox is a game-folder item"
        );
        assert!(!redist_missing(redist, &["NotInRepo".to_string()]));
    }

    #[test]
    fn only_gogdl_windows_installs_have_setup_steps() {
        let mut meta = GogMeta {
            platform: GogPlatform::Windows,
            ..Default::default()
        };
        assert!(has_setup_steps(&meta));
        meta.origin = GogOrigin::Offline;
        assert!(!has_setup_steps(&meta));
        meta.origin = GogOrigin::Drive;
        assert!(!has_setup_steps(&meta));
        assert!(!has_setup_steps(&GogMeta::default()), "Linux build");
        assert_eq!(language_name("en-US"), "English");
        assert_eq!(language_name("pt-BR"), "Portuguese");
        assert_eq!(language_name(""), "English");
    }
}
