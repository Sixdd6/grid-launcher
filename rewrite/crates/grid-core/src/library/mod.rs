//! Library install pipeline.
//!
//! [`InstallService`] is the glue that turns the queue state machine, the
//! downloader, the extraction engine, launch-file selection, the path rules
//! and the SQLite registry into one install / uninstall flow. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! ("InstallService", "Data flow for one install") and
//! `docs/porting/03-library-install.md` for the behavior this mirrors.

pub mod content;
pub mod download;
pub mod extract;
pub mod launch_select;
pub mod paths;
pub mod platforms;
pub mod queue;
pub mod registry;
pub mod specials;
pub mod update_detection;

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde_json::Value;

use crate::autoconfig::readers::{
    ps3_vfs_dev_hdd0_path, ps3_vfs_games_path, rpcs3_data_root, xenia_directory_settings,
};
use crate::autoconfig::rpcs3::update_games_yml;
use crate::autoconfig::{self, RaCredentials};
use crate::config::{CompatToolInstall, Config, EmulatorEntry};
use crate::images::ImageFields;
use crate::launch::compat;
use crate::launch::forge::{ForgeClient, ForgeProvider, ResolvedDownload};
use crate::launch::profiles::{
    load_profiles, profile_available_on_host, profile_for_entry, EmulatorProfile,
};
use crate::launch::selection::{default_emulator_name_for_platform, emulator_entry_by_name};
use crate::launch::source::HOST_PLATFORM;
use crate::launch::template::split_template;
use crate::launch::{catalog, emu_install};
use crate::romm::{RomDetail, RomFile, RommClient};
use content::{content_file_ids, ContentKind};
use download::{download_targets, FileTarget, RommProvider};
use extract::{
    extract_archive, extract_iso_with_system_7z, is_extractable_archive, should_extract,
};
use launch_select::select_launch_file;
use paths::{
    archive_name, candidate_archives, candidate_extracted_dirs, extraction_dir, platform_dir,
    sanitize_component,
};
use platforms::{is_native_platform, is_ps3_platform, is_ps4_platform, is_xbox360_platform};
use queue::{Admission, CancelAction, DownloadStatus, DownloadsSnapshot, JobKey, QueueState};
use registry::{installed_match, InstalledGame, Registry};
use specials::ps3::Ps3Roots;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error(transparent)]
    Romm(#[from] crate::romm::RommError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("{0}")]
    Extract(String),
    #[error("Archive extracted but no ROM file was found")]
    NoLaunchFile,
    #[error("cancelled")]
    Cancelled,
    #[error("Set a library folder in settings before installing games")]
    LibraryPathUnset,
    #[error("registry: {0}")]
    Registry(String),
}

// --- constants --------------------------------------------------------------

/// Minimum gap between change notifications caused by download progress.
/// Status transitions ignore this and always notify.
const DOWNLOAD_NOTIFY_INTERVAL: Duration = Duration::from_millis(100);
/// Minimum gap between change notifications caused by install (extraction)
/// progress.
const INSTALL_NOTIFY_INTERVAL: Duration = Duration::from_millis(150);
/// Attempts and pause for the post-extract archive delete (doc 03: an
/// antivirus or indexer can hold the file open briefly after extraction).
const ARCHIVE_DELETE_ATTEMPTS: u32 = 20;
const ARCHIVE_DELETE_PAUSE: Duration = Duration::from_millis(250);
/// The metadata sidecar the server lists among a game's files. It is never a
/// download target.
const METADATA_FILE_NAME: &str = "game.json";
const NO_DOWNLOADABLE_FILE: &str = "the server lists no downloadable file for this game";
/// The native (Windows) branch's own "nothing to install" message, which the
/// reference words differently from the generic one above.
const NO_NATIVE_DOWNLOADABLE_FILE: &str = "No downloadable file was found for this game";

/// The `platform` column an emulator entry shows in the downloads drawer.
/// Emulators are config entries, never registry rows, so this is a label
/// only — nothing looks a platform directory up from it.
const EMULATOR_PLATFORM: &str = "Emulator";
/// The `platform` column a managed compat-tool entry shows in the downloads
/// drawer.
const COMPAT_TOOL_PLATFORM: &str = "Compatibility Tool";
/// [`finalize_emulator`]'s message when a downloaded compat tool's tree
/// carries no `proton` entry point ([`compat::find_proton_dir`]).
const NO_PROTON_ENTRY_POINT: &str = "Downloaded compatibility tool has no `proton` entry point";
/// [`InstallService::install_compat_tool`]'s message when `source_id` names
/// no compat-tool catalog profile.
const UNKNOWN_COMPAT_TOOL_SOURCE: &str = "unknown compat tool source";
/// Where an emulator archive is extracted before its contents are merged
/// into the install directory. A sibling of the archive inside the install
/// directory, so the merge is a rename and never a cross-device copy; the
/// extraction engine wipes its destination, which is why it cannot be the
/// install directory itself (that would delete the downloaded supplementals
/// sitting next to the archive).
const EXTRACT_TMP_DIR: &str = ".extract-tmp";
const NO_EMULATOR_EXECUTABLE: &str = "No launchable emulator executable was found after install";

/// The verbatim messages the update/DLC flows show. Worded exactly as
/// the reference does (`install_mixin.py:364-383`,
/// `details_view_mixin.py:1559`, `:1849`); the drawer row shows them
/// unchanged, so they must not drift.
const XENIA_CONTENT_ROOT_UNKNOWN: &str =
    "Could not determine Xenia content directory. Is Xenia configured?";
const XBOX360_NEEDS_LINUX_EMULATOR: &str =
    "Xbox 360 content requires a Linux-compatible emulator such as Xenia Edge. \
     Install and configure Xenia Edge, then try again.";
const XBOX360_EMULATOR_WINDOWS_ONLY: &str =
    "The configured Xbox 360 emulator only runs on Windows. Install a \
     Linux-compatible emulator such as Xenia Edge to apply content.";
const CONTENT_UNSUPPORTED_PLATFORM: &str =
    "Update/DLC content is only supported for PS4 and Xbox 360 games";
/// A native update needs the directory it merges into.
const NO_NATIVE_INSTALL_DIR: &str =
    "Game install directory could not be found. Reinstall the game and try again.";
/// What every "this game is not in the registry" failure says.
pub const NOT_INSTALLED: &str = "not installed";
/// `install_update` refuses native rows: those merge through
/// `install_native_update` instead of replacing the install.
pub const NATIVE_UPDATE_REQUIRED: &str = "Native games update through the merge path.";

/// Everything outside the RFC 3986 unreserved set (`ALPHA / DIGIT / - . _ ~`)
/// is percent-encoded. Applied to the file-name segment of a content URL so a
/// name containing a space, `#`, `?` or `/` can never change the shape of the
/// request.
const FILE_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub(crate) fn encode_file_segment(name: &str) -> String {
    utf8_percent_encode(name, FILE_SEGMENT).to_string()
}

// --- install plan -----------------------------------------------------------

/// What one game job installs. Every job carries one; `Base` is the ordinary
/// "install this game" flow and the only mode this task's `plan_install`
/// produces. The rest name the add-on flows that install ON TOP of an
/// already installed game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    Base,
    /// A non-native game re-installed over its existing install ("update"
    /// mode, install_mixin.py:1554): the base pipeline, minus the
    /// already-installed short-circuit.
    Update,
    Ps4Content,
    Xbox360Content,
    NativeUpdate,
}

impl InstallMode {
    /// The `kind` string the drawer row carries for this mode.
    pub fn kind(self) -> &'static str {
        match self {
            InstallMode::Base => "base",
            InstallMode::Update => "update",
            InstallMode::Ps4Content => "ps4_content",
            InstallMode::Xbox360Content => "xbox360_content",
            InstallMode::NativeUpdate => "native_update",
        }
    }
}

/// Everything one admitted install needs, computed once before admission so a
/// queued job can start later without re-fetching anything.
#[derive(Clone)]
struct InstallJob {
    rom_id: i64,
    /// What this job installs. Decides the finalize branch and the drawer
    /// row's `kind`.
    mode: InstallMode,
    /// The content category this job installs, for a `Ps4Content` /
    /// `Xbox360Content` mode; `None` for every other mode.
    content_kind: Option<ContentKind>,
    /// The server file ids this job downloads. For a base or native-update
    /// job that is one id per target, in target order; for a content job it
    /// is every file of the requested category, which the server bundles
    /// into ONE archive named by a single `file_ids` query pair.
    file_ids: Vec<i64>,
    detail: RomDetail,
    targets: Vec<FileTarget>,
    /// The primary downloaded file: the archive for a single-file install,
    /// the launch entry's path for a multi-file one.
    primary_archive: PathBuf,
    /// `Some` only for a multi-file game, which is never extracted.
    multi_file_game_dir: Option<PathBuf>,
    /// Server `file_name` of the primary file — recorded as `rom_file_name`,
    /// and for a multi-file game the launch entry's name.
    launch_entry: String,
    /// `Some` only for a native (Windows) game: the per-game directory the
    /// archive, the extracted `game/` tree and the Wine prefix all live
    /// under.
    native_game_dir: Option<PathBuf>,
    /// `Some` when the server listed a `game.json` sidecar for a native
    /// game: where that sidecar was downloaded to, so finalize can read the
    /// metadata back off disk.
    game_json_target: Option<PathBuf>,
    /// The client the download runs on, carried on the job so a queued
    /// install can start after its caller has gone away.
    client: Arc<RommClient>,
}

/// Everything one admitted emulator acquisition needs. Unlike an
/// [`InstallJob`], the download plan is NOT computed before admission: the
/// forge round trips that resolve a release down to one asset happen inside
/// the download task, so a resolution failure lands on the drawer row
/// exactly like a download failure does (matching the reference, which
/// resolves inside `InstallDownloadWorker.run`).
struct EmulatorJob {
    source_id: String,
    profile_name: String,
    profile_args: String,
    /// The tag the catalog source block CONFIGURES
    /// ([`catalog::configured_tag`]) — `"latest"` when it pins nothing.
    ///
    /// This, not the tag the release resolves to, names the install
    /// directory and is recorded as `source_release_tag`
    /// (install_mixin.py:1444, emulator_ui_mixin.py:1168-1190). A
    /// `latest`-pinned emulator therefore reinstalls over one stable
    /// `<name>-latest` directory instead of leaving one directory per
    /// release behind.
    configured_tag: String,
    /// The profile's RAW `source` block. Supplemental specs are read from
    /// here, not from the normalized/merged map: `workers.py:128` reads
    /// `self.source_metadata`, and each spec carries its own
    /// `platform_overrides` that `_resolve_source_download` merges per-spec.
    raw_source: Value,
    /// The library root, already `~`-expanded by [`InstallService::library_root`].
    /// For a managed compat-tool job this is instead [`compat::managed_root`]
    /// — never expanded from the user's library, and never a game-facing
    /// path.
    library: PathBuf,
    forge: Arc<ForgeClient>,
    /// Filled in by the download task once the forge has resolved the
    /// release; consumed by finalize.
    resolved: Option<ResolvedPaths>,
    /// Whether this job installs a managed compat tool (Task 12) rather than
    /// an ordinary emulator. Decides which directory function names the
    /// install directory ([`emu_install::compat_tool_install_dir`] vs.
    /// [`emu_install::emulator_install_dir`]) and which finalize branch runs.
    compat_tool: bool,
}

/// What the download task resolved an [`EmulatorJob`] down to.
struct ResolvedPaths {
    install_dir: PathBuf,
    /// The primary archive, inside `install_dir`.
    archive: PathBuf,
    /// Downloaded supplemental archives, siblings of `archive`.
    supplementals: Vec<PathBuf>,
    resolved: ResolvedDownload,
}

/// The two job kinds the queue drives. One `pending_jobs` map and one
/// download/finalize pair serve both, so an emulator acquisition and a game
/// install queue behind each other rather than racing for the same slot.
enum JobPayload {
    Game(InstallJob),
    Emulator(EmulatorJob),
}

/// Whether a download from `provider` needs the GitHub API headers. Read
/// per download, not per job: a GitHub primary can carry a Gitea or direct
/// supplemental.
fn needs_github_headers(provider: &str) -> bool {
    provider == "github"
}

/// Whether the server lists `file` as something to download: a top-level
/// entry that is neither the metadata sidecar nor a nested path (the server
/// occasionally flags a nested entry as top-level; such a name would escape
/// the game directory).
fn is_download_candidate(file: &RomFile) -> bool {
    file.is_top_level
        && file.file_name != METADATA_FILE_NAME
        && !file.file_name.contains('/')
        && !file.file_name.contains('\\')
        && content::is_game_category(&file.category)
}

fn content_target(rom_id: i64, file: &RomFile, dest: PathBuf, expected_size: i64) -> FileTarget {
    FileTarget {
        url_path: format!(
            "/api/roms/{rom_id}/content/{}",
            encode_file_segment(&file.file_name)
        ),
        query: vec![("file_ids".to_string(), file.id.to_string())],
        dest,
        expected_size,
    }
}

/// Computes the download plan for `detail` under `library`. Pure apart from
/// cloning the client handle: no I/O, no queue access.
///
/// One candidate ⇒ a single archive at `<platform dir>/<archive name>`.
/// More than one ⇒ a multi-file game: every candidate lands in
/// `<platform dir>/<safe title>/`, and the launch entry is the first `.m3u`
/// if there is one, else the first candidate.
fn plan_install(
    detail: &RomDetail,
    library: &Path,
    client: Arc<RommClient>,
) -> Result<InstallJob, LibraryError> {
    let platform_root = platform_dir(library, &detail.platform_name);

    // A native (Windows) game never becomes a multi-file install: the
    // server lists its archive next to a `game.json` sidecar and any number
    // of extras, and exactly one of those is the game. Everything lands in
    // one per-game directory instead of the platform root.
    if is_native_platform(&detail.platform_name) {
        return plan_native_install(detail, &platform_root, client);
    }

    let candidates: Vec<&RomFile> = detail
        .files
        .iter()
        .filter(|file| is_download_candidate(file))
        .collect();

    match candidates.as_slice() {
        [] => Err(LibraryError::Extract(NO_DOWNLOADABLE_FILE.to_string())),
        [only] => {
            let dest = platform_root.join(archive_name(
                &detail.fs_name,
                &detail.name,
                &detail.platform_name,
            ));
            // A single-file game's own size is the better total when the file
            // entry does not carry one.
            let size = if only.file_size_bytes > 0 {
                only.file_size_bytes
            } else {
                detail.filesize_bytes
            };
            Ok(InstallJob {
                rom_id: detail.id,
                mode: InstallMode::Base,
                content_kind: None,
                file_ids: vec![only.id],
                detail: detail.clone(),
                targets: vec![content_target(detail.id, only, dest.clone(), size)],
                primary_archive: dest,
                multi_file_game_dir: None,
                launch_entry: only.file_name.clone(),
                native_game_dir: None,
                game_json_target: None,
                client,
            })
        }
        many => {
            let game_dir = platform_root.join(sanitize_component(&detail.name, "game"));
            let launch = many
                .iter()
                .copied()
                .find(|file| file.file_name.to_lowercase().ends_with(".m3u"))
                .unwrap_or(many[0]);
            let targets = many
                .iter()
                .map(|file| {
                    content_target(
                        detail.id,
                        file,
                        game_dir.join(&file.file_name),
                        file.file_size_bytes,
                    )
                })
                .collect();
            Ok(InstallJob {
                rom_id: detail.id,
                mode: InstallMode::Base,
                content_kind: None,
                file_ids: many.iter().map(|file| file.id).collect(),
                detail: detail.clone(),
                targets,
                primary_archive: game_dir.join(&launch.file_name),
                multi_file_game_dir: Some(game_dir),
                launch_entry: launch.file_name.clone(),
                native_game_dir: None,
                game_json_target: None,
                client,
            })
        }
    }
}

/// [`plan_install`]'s native (Windows) branch.
///
/// The archive is picked by [`specials::native::select_archive`], not by the
/// generic candidate filter: a native game's server payload routinely lists
/// artwork and soundtracks before the archive, and `game.json` is a metadata
/// sidecar rather than something to launch. Everything is downloaded into
/// `<platform dir>/<safe title>/` — the game's own home directory, which
/// later holds the extracted `game/` tree and (on non-Windows hosts) the
/// Wine prefix. Ports `_download_game_archive`'s native branch
/// (`install_mixin.py:977-1020`).
fn plan_native_install(
    detail: &RomDetail,
    platform_root: &Path,
    client: Arc<RommClient>,
) -> Result<InstallJob, LibraryError> {
    let Some(archive) = specials::native::select_archive(&detail.files) else {
        return Err(LibraryError::Extract(
            NO_NATIVE_DOWNLOADABLE_FILE.to_string(),
        ));
    };
    let game_dir = platform_root.join(sanitize_component(&detail.name, "game"));
    let dest = game_dir.join(&archive.file_name);
    let size = if archive.file_size_bytes > 0 {
        archive.file_size_bytes
    } else {
        detail.filesize_bytes
    };

    let mut file_ids = vec![archive.id];
    let mut targets = vec![content_target(detail.id, archive, dest.clone(), size)];
    // The sidecar is fetched as its own request with its own `file_ids`:
    // asking for the content endpoint without one makes the server bundle
    // every listed file into a single zip.
    let game_json_target = specials::native::has_game_json(&detail.files).map(|sidecar| {
        let sidecar_dest = game_dir.join(METADATA_FILE_NAME);
        file_ids.push(sidecar.id);
        targets.push(content_target(
            detail.id,
            sidecar,
            sidecar_dest.clone(),
            sidecar.file_size_bytes,
        ));
        sidecar_dest
    });

    Ok(InstallJob {
        rom_id: detail.id,
        mode: InstallMode::Base,
        content_kind: None,
        file_ids,
        detail: detail.clone(),
        targets,
        primary_archive: dest,
        multi_file_game_dir: None,
        launch_entry: archive.file_name.clone(),
        native_game_dir: Some(game_dir),
        game_json_target,
        client,
    })
}

/// The install mode an update/DLC job for `platform` runs in, or `None`
/// when that platform has no content flow at all.
fn content_mode(platform: &str) -> Option<InstallMode> {
    if is_ps4_platform(platform) {
        Some(InstallMode::Ps4Content)
    } else if is_xbox360_platform(platform) {
        Some(InstallMode::Xbox360Content)
    } else {
        None
    }
}

/// The verbatim "the server lists none of this" message for `mode`. The two
/// consoles share one wording and differ only in the console they name, so
/// the message follows the platform rather than the content kind
/// (`details_view_mixin.py:1559` for PS4, `:1640` for Xbox 360).
fn no_content_message(mode: InstallMode, kind: ContentKind) -> String {
    let console = if mode == InstallMode::Xbox360Content {
        "Xbox 360"
    } else {
        "PS4"
    };
    format!(
        "No {console} {} files were found for this title in server metadata.",
        kind.as_str()
    )
}

/// `ids` as the `file_ids` query value: a comma-separated list, in order.
fn file_ids_csv(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

/// Builds the update/DLC job for `detail`: the admission key, the drawer
/// row's title, and the payload.
///
/// ONE target, whatever `file_ids` holds: the content endpoint bundles every
/// requested file into a single archive, so the whole category arrives as
/// `<platform dir>/<safe title>-<kind>.zip`
/// (`install_mixin.py:319-328`). The URL's file-name segment is the game's
/// own `fs_name`, which is what the reference asks for too — the server
/// names the bundle, not the caller.
fn content_job(
    library: &Path,
    detail: &RomDetail,
    kind: ContentKind,
    mode: InstallMode,
    file_ids: Vec<i64>,
    client: Arc<RommClient>,
) -> (JobKey, String, JobPayload) {
    let fallback = if mode == InstallMode::Xbox360Content {
        "xbox360-content"
    } else {
        "ps4-content"
    };
    let dest = platform_dir(library, &detail.platform_name).join(format!(
        "{}-{}.zip",
        sanitize_component(&detail.name, fallback),
        kind.as_str()
    ));

    let mut job = InstallJob {
        rom_id: detail.id,
        mode,
        content_kind: Some(kind),
        file_ids,
        detail: detail.clone(),
        targets: Vec::new(),
        primary_archive: dest.clone(),
        multi_file_game_dir: None,
        // Nothing reads this for a content job — no registry row is
        // created — but the archive it names is what was downloaded.
        launch_entry: detail.fs_name.clone(),
        native_game_dir: None,
        game_json_target: None,
        client,
    };
    job.targets.push(FileTarget {
        url_path: format!(
            "/api/roms/{}/content/{}",
            detail.id,
            encode_file_segment(&detail.fs_name)
        ),
        query: vec![("file_ids".to_string(), file_ids_csv(&job.file_ids))],
        dest,
        // The server builds the bundle on the fly, so its size is not
        // knowable before the response arrives.
        expected_size: 0,
    });

    let title = format!("{} ({})", detail.name, kind.as_str());
    (
        JobKey::Content(detail.id, kind),
        title,
        JobPayload::Game(job),
    )
}

// --- service ----------------------------------------------------------------

type Listener = Arc<dyn Fn(DownloadsSnapshot) + Send + Sync>;

/// Supplies the RetroAchievements pair to the autoconfig hook. grid-core
/// never reads the keyring itself, so the app installs this.
pub type RaProvider = Arc<dyn Fn() -> Option<RaCredentials> + Send + Sync>;

/// Notified with a finalized game's image fields right after the registry
/// write. grid-core never imports Tauri, so the app installs this to trigger
/// its own post-install cover prefetch (D5).
pub type ImageHook = Arc<dyn Fn(ImageFields) + Send + Sync>;

/// Notified with a base install's finalized registry row, after the row and
/// (for PS3) `games.yml` have been written. The app hangs the follow-on
/// work that grid-core cannot do itself off this — auto-queuing Xbox 360
/// content, kicking off a firmware install — so nothing here imports Tauri.
pub type GameFinalizedHook = Arc<dyn Fn(InstalledGame) + Send + Sync>;

/// Notified (with no arguments) right after a managed compat-tool install
/// writes its `CompatToolInstall` config record. The app hangs its own
/// `compat-tools-changed` event emission off this (Task 15), so nothing here
/// imports Tauri.
pub type CompatToolsHook = Arc<dyn Fn() + Send + Sync>;

/// What [`EmulatorInstalledHook`] is notified with, right after an ordinary
/// emulator's config entry is written and autoconfig has run. Never fired
/// for a managed compat-tool install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmulatorInstalled {
    pub name: String,
    /// `true` when no `[[emulators]]` entry with this name existed before
    /// this write — i.e. this was not a reinstall over an existing entry.
    pub fresh: bool,
    /// Always `false` today: a managed compat-tool install returns from its
    /// own finalize branch before this hook is reached, so the hook is only
    /// ever fired for an ordinary emulator. Kept as a field, and checked by
    /// the app's hook (`app/src-tauri/src/lib.rs`), as defense in depth — a
    /// future compat-tool branch that starts firing the hook must not
    /// silently trigger a firmware pass for a compat tool.
    pub compat_tool: bool,
}

/// Notified after an ordinary emulator install writes its config entry and
/// autoconfig has run. The app hangs its own follow-on work off this — a
/// fresh install can trigger a firmware install (Task 15) — so nothing here
/// imports Tauri.
pub type EmulatorInstalledHook = Arc<dyn Fn(EmulatorInstalled) + Send + Sync>;

/// Owns the install queue and drives one download task and one finalize task
/// at a time. Every method takes `&self`; the async entry points take
/// `&Arc<Self>` because they spawn tasks that outlive the call.
pub struct InstallService {
    queue: Mutex<QueueState>,
    registry: Arc<Registry>,
    config_path: PathBuf,
    notify: RwLock<Option<Listener>>,
    /// One cooperative cancellation flag per active download, keyed by entry
    /// id and removed when that download ends.
    cancel_flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    /// Plans for entries that were admitted as `Queued` and are waiting for a
    /// free slot.
    pending_jobs: Mutex<HashMap<u64, JobPayload>>,
    /// The autoprofile catalog emulator `source_id`s resolve against.
    profiles: Cow<'static, [EmulatorProfile]>,
    /// Built on the first emulator install and shared by every later one, so
    /// all forge traffic goes through one connection pool. Deliberately not
    /// built in [`Self::new`]: constructing it can fail, and a process that
    /// never installs an emulator should never pay for it.
    forge: OnceLock<Arc<ForgeClient>>,
    last_emit: Mutex<Instant>,
    /// The assignable server platform names the last successful
    /// `list_platforms` saw — the only way grid-core learns the platform
    /// list, since it holds no session of its own.
    known_platforms: RwLock<Vec<String>>,
    /// `None` until the app installs one; autoconfig then writes no
    /// RetroAchievements credentials.
    ra_provider: RwLock<Option<RaProvider>>,
    /// `None` until the app installs one; a finalized game then triggers no
    /// cover prefetch.
    image_hook: RwLock<Option<ImageHook>>,
    /// `None` until the app installs one; a finalized base install then
    /// triggers no follow-on work.
    game_finalized_hook: RwLock<Option<GameFinalizedHook>>,
    /// Server platform name -> platform id, as the last successful platform
    /// fetch saw it. Firmware lookups need the id, and grid-core holds no
    /// session of its own to fetch it with.
    platform_ids: RwLock<BTreeMap<String, i64>>,
    /// `None` until the app installs one; a finalized compat-tool install
    /// then triggers no notification.
    compat_tools_hook: RwLock<Option<CompatToolsHook>>,
    /// `None` until the app installs one; a finalized ordinary emulator
    /// install then triggers no follow-on work.
    emulator_installed_hook: RwLock<Option<EmulatorInstalledHook>>,
}

impl InstallService {
    pub fn new(registry: Arc<Registry>, config_path: PathBuf) -> Arc<Self> {
        Self::build(registry, config_path, Cow::Borrowed(load_profiles()))
    }

    /// [`Self::new`], but resolving emulator `source_id`s against `profiles`
    /// instead of the embedded autoprofile catalog. The integration tests use
    /// this to point a profile's `source` block at a local server; production
    /// code calls [`Self::new`].
    pub fn with_profiles(
        registry: Arc<Registry>,
        config_path: PathBuf,
        profiles: Vec<EmulatorProfile>,
    ) -> Arc<Self> {
        Self::build(registry, config_path, Cow::Owned(profiles))
    }

    fn build(
        registry: Arc<Registry>,
        config_path: PathBuf,
        profiles: Cow<'static, [EmulatorProfile]>,
    ) -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(QueueState::default()),
            registry,
            config_path,
            notify: RwLock::new(None),
            cancel_flags: Mutex::new(HashMap::new()),
            pending_jobs: Mutex::new(HashMap::new()),
            profiles,
            forge: OnceLock::new(),
            last_emit: Mutex::new(Instant::now()),
            known_platforms: RwLock::new(Vec::new()),
            ra_provider: RwLock::new(None),
            image_hook: RwLock::new(None),
            game_finalized_hook: RwLock::new(None),
            platform_ids: RwLock::new(BTreeMap::new()),
            compat_tools_hook: RwLock::new(None),
            emulator_installed_hook: RwLock::new(None),
        })
    }

    /// Installs the change-notification callback. Called once by the UI
    /// layer; a second call replaces the first.
    pub fn set_notify(&self, f: Listener) {
        *self.notify.write().unwrap() = Some(f);
    }

    /// Records the assignable server platform names the app just fetched.
    /// Callers pass the list already run through
    /// [`autoconfig::entry::assignable_platforms`].
    pub fn set_known_platforms(&self, platforms: Vec<String>) {
        *self.known_platforms.write().unwrap() = platforms;
    }

    /// The platform names [`Self::set_known_platforms`] last recorded; empty
    /// until the app has fetched them once.
    pub fn known_platforms(&self) -> Vec<String> {
        self.known_platforms.read().unwrap().clone()
    }

    /// Installs the RetroAchievements credential source the autoconfig hook
    /// reads. A second call replaces the first.
    pub fn set_ra_provider(&self, f: RaProvider) {
        *self.ra_provider.write().unwrap() = Some(f);
    }

    /// Installs the post-install image prefetch hook (D5). A second call
    /// replaces the first.
    pub fn set_image_hook(&self, f: ImageHook) {
        *self.image_hook.write().unwrap() = Some(f);
    }

    /// Installs the post-install follow-on hook. A second call replaces the
    /// first.
    pub fn set_game_finalized_hook(&self, f: GameFinalizedHook) {
        *self.game_finalized_hook.write().unwrap() = Some(f);
    }

    /// Installs the post-compat-tool-install notification hook. A second
    /// call replaces the first.
    pub fn set_compat_tools_hook(&self, f: CompatToolsHook) {
        *self.compat_tools_hook.write().unwrap() = Some(f);
    }

    /// Installs the post-emulator-install follow-on hook. A second call
    /// replaces the first.
    pub fn set_emulator_installed_hook(&self, f: EmulatorInstalledHook) {
        *self.emulator_installed_hook.write().unwrap() = Some(f);
    }

    /// Records the server platform name -> id map the app just fetched.
    pub fn set_platform_ids(&self, ids: BTreeMap<String, i64>) {
        *self.platform_ids.write().unwrap() = ids;
    }

    /// The platform ids [`Self::set_platform_ids`] last recorded; empty
    /// until the app has fetched them once.
    pub fn platform_ids(&self) -> BTreeMap<String, i64> {
        self.platform_ids.read().unwrap().clone()
    }

    /// The current RetroAchievements pair, or `None` when no provider is
    /// installed or the user has no login.
    pub fn ra_credentials(&self) -> Option<RaCredentials> {
        let provider = self.ra_provider.read().unwrap();
        provider.as_ref().and_then(|f| f())
    }

    /// The current entry list, newest first.
    pub fn snapshot(&self) -> DownloadsSnapshot {
        self.queue.lock().unwrap().snapshot()
    }

    /// Every installed game in the registry.
    pub fn installed(&self) -> Result<Vec<InstalledGame>, LibraryError> {
        self.registry.all()
    }

    /// The registry backing this service, for callers that need to write to
    /// it directly (e.g. the image replenish job's `update_images`).
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    /// Starts (or queues) an install for `rom_id`.
    ///
    /// `Err` is returned only for failures before admission — no library
    /// path, an unreachable server, or a game the server lists no
    /// downloadable file for. Once an entry exists, every later failure shows
    /// up on that entry instead. A `rom_id` that is already downloading,
    /// finalizing or queued is ignored silently.
    pub async fn install(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        rom_id: i64,
    ) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        let detail = client.rom_detail(rom_id).await?;
        let job = plan_install(&detail, &library, client)?;
        let key = JobKey::Rom(job.rom_id);
        let title = job.detail.name.clone();
        let platform = job.detail.platform_name.clone();
        let kind = job.mode.kind();
        self.admit(key, &title, &platform, kind, JobPayload::Game(job));
        Ok(())
    }

    /// Starts (or queues) an update or DLC install for an already installed
    /// PS4 or Xbox 360 game.
    ///
    /// The row is checked BEFORE anything is fetched: content is applied on
    /// top of an install and is meaningless without one. `Err` covers every
    /// pre-admission failure — no library path, no row, an unreachable
    /// server, a platform with no content flow, or a title the server lists
    /// no files of that kind for. Once the entry exists, every later failure
    /// shows up on it. A `(rom_id, kind)` pair that is already downloading,
    /// finalizing or queued is ignored silently; the same game's update and
    /// DLC queue side by side.
    pub async fn install_content(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        rom_id: i64,
        kind: ContentKind,
    ) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        self.current_row(rom_id)?;
        let detail = client.rom_detail(rom_id).await?;
        let Some(mode) = content_mode(&detail.platform_name) else {
            return Err(LibraryError::Extract(
                CONTENT_UNSUPPORTED_PLATFORM.to_string(),
            ));
        };
        let file_ids = content_file_ids(&detail.files, kind);
        if file_ids.is_empty() {
            return Err(LibraryError::Extract(no_content_message(mode, kind)));
        }
        let (key, title, payload) = content_job(&library, &detail, kind, mode, file_ids, client);
        self.admit(key, &title, &detail.platform_name, mode.kind(), payload);
        Ok(())
    }

    /// Starts (or queues) an update install for an already installed native
    /// (Windows) game. The new archive is MERGED into the existing install
    /// rather than replacing it, so saves and configs survive.
    ///
    /// `Err` covers every pre-admission failure: no row, a row that is not a
    /// native game or has lost its install directory, an unreachable server,
    /// or a payload with no archive in it.
    pub async fn install_native_update(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        rom_id: i64,
    ) -> Result<(), LibraryError> {
        let row = self.current_row(rom_id)?;
        let extracted_dir = row.extracted_dir.trim();
        if !is_native_platform(&row.platform)
            || extracted_dir.is_empty()
            || !Path::new(extracted_dir).is_dir()
        {
            return Err(LibraryError::Extract(NO_NATIVE_INSTALL_DIR.to_string()));
        }

        let detail = client.rom_detail(rom_id).await?;
        let Some(archive) = specials::native::select_archive(&detail.files) else {
            return Err(LibraryError::Extract(
                NO_NATIVE_DOWNLOADABLE_FILE.to_string(),
            ));
        };
        // The game's own home directory, which already holds the extracted
        // tree: the update archive lands beside it so the merge never
        // crosses a filesystem.
        let native_game_dir = match row.native_game_dir.trim() {
            "" => Path::new(extracted_dir)
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf(),
            recorded => PathBuf::from(recorded),
        };
        let dest = native_game_dir.join(&archive.file_name);
        let size = if archive.file_size_bytes > 0 {
            archive.file_size_bytes
        } else {
            detail.filesize_bytes
        };

        let title = format!("{} (update)", detail.name);
        let platform = detail.platform_name.clone();
        let job = InstallJob {
            rom_id,
            mode: InstallMode::NativeUpdate,
            content_kind: None,
            file_ids: vec![archive.id],
            targets: vec![content_target(rom_id, archive, dest.clone(), size)],
            primary_archive: dest,
            multi_file_game_dir: None,
            launch_entry: archive.file_name.clone(),
            native_game_dir: Some(native_game_dir),
            game_json_target: None,
            detail: detail.clone(),
            client,
        };
        self.admit(
            JobKey::NativeUpdate(rom_id),
            &title,
            &platform,
            InstallMode::NativeUpdate.kind(),
            JobPayload::Game(job),
        );
        Ok(())
    }

    /// Starts (or queues) a plain re-install of an already installed
    /// non-native game (Python "update" mode, doc 10 "Performing the update").
    /// Same plan as a base install, but the job is marked `Update` so
    /// `finish_download` never short-circuits on the existing row and
    /// `finalize_base` replaces it. Admitted under `JobKey::Rom`, so an update
    /// and a base install of the same rom can never run side by side.
    pub async fn install_update(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        rom_id: i64,
    ) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        let row = self.current_row(rom_id)?;
        if is_native_platform(&row.platform) {
            return Err(LibraryError::Extract(NATIVE_UPDATE_REQUIRED.to_string()));
        }
        let detail = client.rom_detail(rom_id).await?;
        let mut job = plan_install(&detail, &library, client)?;
        job.mode = InstallMode::Update;
        let key = JobKey::Rom(job.rom_id);
        let title = job.detail.name.clone();
        let platform = job.detail.platform_name.clone();
        self.admit(
            key,
            &title,
            &platform,
            InstallMode::Update.kind(),
            JobPayload::Game(job),
        );
        Ok(())
    }

    /// Starts (or queues) an emulator acquisition for the catalog
    /// `source_id` (`"{owner}/{repo}"`).
    ///
    /// `Err` is returned only for failures before admission: no library
    /// path, or a `source_id` no catalog profile carries. Everything the
    /// forge is involved in — resolving the release, picking the asset,
    /// downloading, extracting — happens on the admitted entry, so those
    /// failures show up on the drawer row. A `source_id` that is already
    /// downloading, finalizing or queued is ignored silently.
    ///
    /// Nothing here touches the SQLite registry: an installed emulator is a
    /// config entry only.
    pub async fn install_emulator(self: &Arc<Self>, source_id: String) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        fn unknown(source_id: &str) -> LibraryError {
            LibraryError::Registry(format!("unknown emulator: {source_id}"))
        }

        let profile =
            catalog::find_profile(&self.profiles, &source_id).ok_or_else(|| unknown(&source_id))?;
        // `find_profile` only matches a profile whose `source` is an object
        // carrying an owner and a repo, so this clone always succeeds; the
        // fallback keeps that assumption from turning into a panic.
        let raw_source = profile.source.clone().ok_or_else(|| unknown(&source_id))?;
        let configured_tag = raw_source
            .as_object()
            .map(catalog::configured_tag)
            .unwrap_or_else(|| "latest".to_string());

        let job = EmulatorJob {
            source_id: source_id.clone(),
            profile_name: profile.name.clone(),
            profile_args: profile.args.clone(),
            configured_tag,
            raw_source,
            library,
            forge: self.forge()?,
            resolved: None,
            compat_tool: false,
        };
        let title = job.profile_name.clone();
        self.admit(
            JobKey::Emulator(source_id),
            &title,
            EMULATOR_PLATFORM,
            "emulator",
            JobPayload::Emulator(job),
        );
        Ok(())
    }

    /// Starts (or queues) a managed compat-tool acquisition for the
    /// compat-tool catalog `source_id` (`"{owner}/{repo}"`).
    ///
    /// Unlike [`Self::install_emulator`], this never reads the configured
    /// library path: a managed compat tool installs under
    /// [`compat::managed_root`], independent of the user's game library
    /// (D15). `Err` is returned only for a `source_id` no compat-tool
    /// catalog profile carries; everything the forge is involved in shows up
    /// on the drawer row instead, exactly like an emulator acquisition. A
    /// `source_id` that is already downloading, finalizing or queued is
    /// ignored silently.
    pub async fn install_compat_tool(
        self: &Arc<Self>,
        source_id: String,
    ) -> Result<(), LibraryError> {
        fn unknown() -> LibraryError {
            LibraryError::Extract(UNKNOWN_COMPAT_TOOL_SOURCE.to_string())
        }

        let profile =
            catalog::find_compat_profile(&self.profiles, &source_id).ok_or_else(unknown)?;
        // `find_compat_profile` only matches a profile whose `source` is an
        // object carrying an owner and a repo, so this clone always
        // succeeds; the fallback keeps that assumption from turning into a
        // panic.
        let raw_source = profile.source.clone().ok_or_else(unknown)?;
        let configured_tag = raw_source
            .as_object()
            .map(catalog::configured_tag)
            .unwrap_or_else(|| "latest".to_string());

        let job = EmulatorJob {
            source_id: source_id.clone(),
            profile_name: profile.name.clone(),
            profile_args: profile.args.clone(),
            configured_tag,
            raw_source,
            library: compat::managed_root(),
            forge: self.forge()?,
            resolved: None,
            compat_tool: true,
        };
        let title = job.profile_name.clone();
        self.admit(
            JobKey::Emulator(source_id),
            &title,
            COMPAT_TOOL_PLATFORM,
            "compat_tool",
            JobPayload::Emulator(job),
        );
        Ok(())
    }

    /// Cancels `entry_id`: an active download gets its flag flipped and ends
    /// as `Cancelled` once the task observes it; a queued entry is dropped
    /// immediately. Anything else is ignored.
    pub fn cancel(&self, entry_id: u64) {
        let action = self.queue.lock().unwrap().request_cancel(entry_id);
        match action {
            CancelAction::ActiveDownload => {
                if let Some(flag) = self.cancel_flags.lock().unwrap().get(&entry_id) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
            CancelAction::RemovedFromQueue => {
                self.pending_jobs.lock().unwrap().remove(&entry_id);
            }
            CancelAction::Ignored => return,
        }
        self.notify_now();
    }

    /// Cancels the oldest live (queued, downloading or finalizing) entry for
    /// `rom_id`. A game with no live entry is ignored, and a finalizing
    /// entry is left alone by [`Self::cancel`] itself — extraction is not
    /// cancellable.
    pub fn cancel_for_rom(&self, rom_id: i64) {
        let id = self.queue.lock().unwrap().first_live_for_rom(rom_id);
        if let Some(id) = id {
            self.cancel(id);
        }
    }

    /// Creates a drawer row for a transfer this service does NOT drive (the
    /// background firmware installer moves its own bytes). The returned id
    /// is handed back to [`Self::complete_external`] when that transfer
    /// ends. The row takes no queue slot, so it neither blocks nor is
    /// blocked by a real install.
    pub fn admit_external(&self, title: &str, platform: &str) -> u64 {
        let id = self.queue.lock().unwrap().admit_external(title, platform);
        self.notify_now();
        id
    }

    /// Reports the outcome of an [`Self::admit_external`] row: a blank
    /// `error` completes it, anything else fails it with that text.
    pub fn complete_external(&self, id: u64, error: &str) {
        self.queue.lock().unwrap().finish_external(id, error);
        self.notify_now();
    }

    /// Retries a failed or cancelled entry: the old entry is dismissed and a
    /// fresh install starts for the same rom or emulator. Any other entry is
    /// ignored.
    ///
    /// `client` is `None` when no RomM session is connected. An emulator
    /// retry never needs one — the forge client is separate and
    /// unauthenticated — so only a game retry fails with `"not connected"`,
    /// and it fails BEFORE the entry is dismissed so the row survives for a
    /// later attempt.
    pub async fn retry(
        self: &Arc<Self>,
        client: Option<Arc<RommClient>>,
        entry_id: u64,
    ) -> Result<(), LibraryError> {
        let retryable = self.queue.lock().unwrap().retryable(entry_id);
        match retryable {
            Some(JobKey::Rom(rom_id)) => {
                let client =
                    client.ok_or_else(|| LibraryError::Registry("not connected".to_string()))?;
                self.dismiss(entry_id);
                self.install(client, rom_id).await
            }
            // A source_id cannot name both an ordinary emulator and a
            // compat tool (`find_profile`/`find_compat_profile` partition
            // the catalog by `is_compat_tool`), so this dispatch is
            // unambiguous.
            Some(JobKey::Emulator(source_id)) => {
                self.dismiss(entry_id);
                if catalog::find_compat_profile(&self.profiles, &source_id).is_some() {
                    self.install_compat_tool(source_id).await
                } else {
                    self.install_emulator(source_id).await
                }
            }
            // A content / native-update retry re-plans through the same
            // entry point the first attempt used, so the row and the server
            // metadata are read fresh: the game may have been updated, or
            // uninstalled, since the failed attempt.
            Some(JobKey::Content(rom_id, kind)) => {
                let client =
                    client.ok_or_else(|| LibraryError::Registry("not connected".to_string()))?;
                self.dismiss(entry_id);
                self.install_content(client, rom_id, kind).await
            }
            Some(JobKey::NativeUpdate(rom_id)) => {
                let client =
                    client.ok_or_else(|| LibraryError::Registry("not connected".to_string()))?;
                self.dismiss(entry_id);
                self.install_native_update(client, rom_id).await
            }
            // An external entry is somebody else's transfer: this service
            // never moved its bytes and has no plan to restart. The owner
            // re-requests it instead, which creates a new row.
            Some(JobKey::External(_)) => Ok(()),
            None => Ok(()),
        }
    }

    /// Removes `entry_id` from the list. An entry that still owns the
    /// download or finalize slot is left alone (the queue refuses it), so
    /// this is a no-op for anything still running.
    pub fn dismiss(&self, entry_id: u64) {
        // Bound to a `let` so the queue lock is definitely released before
        // `notify_now` below takes it again to build the snapshot.
        let removed = self.queue.lock().unwrap().dismiss(entry_id);
        if !removed {
            return;
        }
        self.pending_jobs.lock().unwrap().remove(&entry_id);
        self.cancel_flags.lock().unwrap().remove(&entry_id);
        self.notify_now();
    }

    /// Deletes an installed game's files and its registry row.
    ///
    /// What gets removed depends on the platform ([`uninstall_steps`]): a
    /// PS3 game gives up its ISO, its trophy directories and its routed
    /// directories; a native game its whole home directory; a multi-file
    /// game its directory; anything else every existing candidate archive
    /// and extraction directory.
    ///
    /// D11: every step runs even after an earlier one failed, and the
    /// failures come back as one error listing them all. The row is deleted
    /// only when nothing failed, so a partial removal leaves the game
    /// installed rather than orphaning it.
    pub fn uninstall(&self, rom_id: i64) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        // `find` falls back to the (title, platform) identity when no row
        // carries the rom id, but never for this call: title and platform
        // are passed blank, so `find` itself refuses the fallback and this
        // only succeeds on a genuine rom_id match. `installed_match` then
        // has the final word on what comes back — a null-rom_id row is
        // still accepted (matching the frontend's `matchesInstalled`), but
        // a row that carries a *different* rom_id is rejected.
        let record = self
            .registry
            .find(Some(rom_id), "", "")?
            .filter(|found| installed_match(found, rom_id))
            .ok_or_else(|| LibraryError::Registry(NOT_INSTALLED.to_string()))?;

        let steps = uninstall_steps(&record, &library);
        let failures = run_removals(&steps, &mut apply_removal);
        if !failures.is_empty() {
            return Err(LibraryError::Registry(failures.join("\n")));
        }

        self.registry.remove(&record.title, &record.platform)?;
        Ok(())
    }

    // --- internals ----------------------------------------------------------

    /// The configured library root, with a leading `~/` expanded.
    fn library_root(&self) -> Result<PathBuf, LibraryError> {
        let config = Config::load(&self.config_path)?;
        if config.library_path.trim().is_empty() {
            return Err(LibraryError::LibraryPathUnset);
        }
        Ok(paths::expand_home(&config.library_path))
    }

    /// The shared forge client, built on first use. Separate from every RomM
    /// client in the process and never given a credential.
    fn forge(&self) -> Result<Arc<ForgeClient>, LibraryError> {
        if let Some(client) = self.forge.get() {
            return Ok(client.clone());
        }
        let built = Arc::new(ForgeClient::new().map_err(|e| LibraryError::Extract(e.0))?);
        // A concurrent caller may have won the race; whichever client is
        // stored is the one everybody uses.
        Ok(self.forge.get_or_init(|| built).clone())
    }

    /// Admits a planned job: starts it, stashes it for a free slot, or drops
    /// it as a duplicate.
    ///
    /// The stash happens while the queue lock is still held. Releasing it
    /// first would let a finishing download's [`Self::pump`] pop the brand
    /// new id out of `waiting` before its job exists, fail the entry as lost
    /// and leave the job stranded in `pending_jobs`.
    fn admit(
        self: &Arc<Self>,
        key: JobKey,
        title: &str,
        platform: &str,
        kind: &'static str,
        payload: JobPayload,
    ) {
        let (admitted, start) = {
            let mut queue = self.queue.lock().unwrap();
            match queue.admit(key, title, platform, kind) {
                Admission::Start(id) => (true, Some((id, payload))),
                Admission::Queued(id) => {
                    self.pending_jobs.lock().unwrap().insert(id, payload);
                    (true, None)
                }
                Admission::Duplicate => (false, None),
            }
        };
        // Nothing below runs under the queue lock: `spawn_download` takes it
        // again, and the listener must never see it held.
        if !admitted {
            return;
        }
        if let Some((id, payload)) = start {
            self.spawn_download(id, payload);
        }
        self.notify_now();
    }

    /// Starts the download task for `id`, registering its cancellation flag
    /// before the task exists so an immediate `cancel` is never lost.
    fn spawn_download(self: &Arc<Self>, id: u64, payload: JobPayload) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flags.lock().unwrap().insert(id, cancel.clone());
        // A cancel that arrived after the entry took the download slot but
        // before the flag above existed found nothing to flip. The
        // `Cancelling` status it left behind is the record of it, so honour
        // that here instead of downloading a file nobody wants.
        let cancelled = matches!(
            self.queue.lock().unwrap().entry(id).map(|e| e.status),
            Some(DownloadStatus::Cancelling)
        );
        if cancelled {
            cancel.store(true, Ordering::Relaxed);
        }
        let service = self.clone();
        tokio::spawn(async move {
            // The fallible half runs in its own task so a panic anywhere in
            // it — including inside the caller's notify listener, reached
            // through `on_download_progress` — comes back as a `JoinError`
            // instead of leaving the download slot taken forever.
            let reporter = service.clone();
            let worker = tokio::spawn(async move {
                let mut payload = payload;
                let mut on_progress = move |downloaded, total, speed| {
                    reporter.on_download_progress(id, downloaded, total, speed);
                };
                let result = match &mut payload {
                    JobPayload::Game(job) => {
                        download_targets(
                            &RommProvider(&job.client),
                            &job.targets,
                            &cancel,
                            &mut on_progress,
                        )
                        .await
                    }
                    JobPayload::Emulator(job) => {
                        download_emulator(job, &cancel, &mut on_progress).await
                    }
                };
                (payload, result)
            });
            let outcome = worker.await;
            service.cancel_flags.lock().unwrap().remove(&id);
            match outcome {
                Ok((payload, result)) => service.finish_download(id, payload, result).await,
                Err(e) => service.fail_download(
                    id,
                    LibraryError::Extract(format!("the download did not finish: {e}")),
                ),
            }
        });
    }

    /// Fails `id` with no job in hand — the download task itself died, so
    /// there is nothing left to finalize. Frees the download slot so the
    /// queue keeps moving instead of wedging on a task that will never
    /// report back.
    fn fail_download(self: &Arc<Self>, id: u64, error: LibraryError) {
        self.queue
            .lock()
            .unwrap()
            .download_finished(id, Err(error), false);
        self.pump();
        self.notify_now();
    }

    /// Records the download outcome, starts finalizing when there is
    /// something to finalize, and lets the queue move on. Called exactly once
    /// by the task that owns the download slot.
    async fn finish_download(
        self: &Arc<Self>,
        id: u64,
        payload: JobPayload,
        result: Result<(), LibraryError>,
    ) {
        // doc 03 §1 step 4: a game that is already in the registry completes
        // straight away — the bytes are on disk, nothing needs installing.
        // An emulator never short-circuits: the registry is games-only, so
        // there is no "already installed" row to find, and the extract /
        // config-write half still has to run.
        // A content / native-update job applies ON TOP of an installed
        // game, so "already installed" is its precondition, not a reason to
        // skip: only a base install short-circuits here. An `Update` job
        // exists precisely to bypass this.
        let skip_finalize = match &payload {
            JobPayload::Game(job) => {
                job.mode == InstallMode::Base
                    && result.is_ok()
                    && self.already_installed(&job.detail).await
            }
            JobPayload::Emulator(_) => false,
        };
        // A cancel that lands after the last chunk arrived is too late: the
        // download succeeded, so `Cancelling` gives way to `Installing` here
        // and the entry finishes. Extraction is not cancellable (doc 03 §1).
        let finalize = {
            let mut queue = self.queue.lock().unwrap();
            queue.download_finished(id, result, skip_finalize);
            matches!(
                queue.entry(id).map(|entry| entry.status),
                Some(DownloadStatus::Installing)
            )
        };
        if finalize {
            self.spawn_finalize(id, payload);
        }
        self.pump();
        self.notify_now();
    }

    /// Registry lookup on the blocking pool. A lookup error is treated as
    /// "not installed": finalizing again is harmless, and the same error will
    /// surface from the upsert with a real message.
    ///
    /// `find`'s title/platform fallback can return a row for a different game
    /// that merely shares a title and platform; `installed_match` is what
    /// keeps that row from being reported as this rom_id's install (doc 03
    /// identity rules; mirrored in the frontend's `matchesInstalled`).
    async fn already_installed(&self, detail: &RomDetail) -> bool {
        let registry = self.registry.clone();
        let rom_id = detail.id;
        let title = detail.name.clone();
        let platform = detail.platform_name.clone();
        tokio::task::spawn_blocking(move || registry.find(Some(rom_id), &title, &platform))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten()
            .is_some_and(|row| installed_match(&row, rom_id))
    }

    /// Starts the finalize task for `id`. Called exactly once by the task
    /// that owns the finalize slot.
    fn spawn_finalize(self: &Arc<Self>, id: u64, payload: JobPayload) {
        let service = self.clone();
        tokio::spawn(async move {
            let worker = service.clone();
            let (result, warning) =
                match tokio::task::spawn_blocking(move || worker.finalize(id, &payload)).await {
                    Ok(outcome) => outcome,
                    Err(e) => (
                        Err(LibraryError::Extract(format!(
                            "the install step did not finish: {e}"
                        ))),
                        String::new(),
                    ),
                };
            service
                .queue
                .lock()
                .unwrap()
                .finalize_finished(id, result, &warning);
            service.pump();
            service.notify_now();
        });
    }

    /// Starts the next ready job when both slots are free.
    ///
    /// Taking the job out of `pending_jobs` under the queue lock is what
    /// makes the "no stashed plan" branch below unreachable: every id in
    /// `waiting` had its job stashed inside the same critical section that
    /// put it there. The lock ordering — queue first, `pending_jobs` second,
    /// never the reverse — is shared with [`Self::admit`].
    fn pump(self: &Arc<Self>) {
        loop {
            let start = {
                let mut queue = self.queue.lock().unwrap();
                let Some(id) = queue.next_ready() else {
                    return;
                };
                match self.pending_jobs.lock().unwrap().remove(&id) {
                    Some(payload) => Some((id, payload)),
                    None => {
                        // Defensive: a queued entry with no stashed plan can
                        // never run. Fail it so the slot frees and the queue
                        // keeps moving instead of wedging.
                        queue.download_finished(
                            id,
                            Err(LibraryError::Extract(
                                "the queued install was lost".to_string(),
                            )),
                            false,
                        );
                        None
                    }
                }
            };
            // `spawn_download` takes the queue lock itself, so it can only be
            // called once the guard above has been released.
            if let Some((id, payload)) = start {
                self.spawn_download(id, payload);
                return;
            }
        }
    }

    /// The blocking half of finalizing: extraction, launch selection,
    /// registry write and archive cleanup. Returns the outcome plus a
    /// warning string that is empty unless the install succeeded with a
    /// caveat.
    fn finalize(
        self: &Arc<Self>,
        id: u64,
        payload: &JobPayload,
    ) -> (Result<(), LibraryError>, String) {
        let mut warning = String::new();
        let result = match payload {
            JobPayload::Game(job) => self.finalize_inner(id, job, &mut warning),
            JobPayload::Emulator(job) => self.finalize_emulator(id, job, &mut warning),
        };
        (result, warning)
    }

    /// Routes a game job to its mode's finalize. `Base` and `Update` lay a
    /// registry row down; the other two modify the row an earlier install
    /// left.
    fn finalize_inner(
        self: &Arc<Self>,
        id: u64,
        job: &InstallJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        match job.mode {
            InstallMode::Base | InstallMode::Update => self.finalize_base(id, job, warning),
            InstallMode::Ps4Content => self.finalize_ps4_content(id, job, warning),
            InstallMode::Xbox360Content => self.finalize_xbox360_content(id, job, warning),
            InstallMode::NativeUpdate => self.finalize_native_update(id, job, warning),
        }
    }

    fn finalize_base(
        self: &Arc<Self>,
        id: u64,
        job: &InstallJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let detail = &job.detail;
        let platform = detail.platform_name.as_str();
        let mut record = new_record(detail, &job.launch_entry);
        let archive = job.primary_archive.as_path();
        let archive_to_delete: Option<&Path>;

        if let Some(game_dir) = &job.multi_file_game_dir {
            // A multi-file game is already laid out on disk: the files are
            // the install, and the launch entry is what gets started.
            record.multi_file_game_dir = path_string(game_dir);
            record.extracted_path = path_string(&job.primary_archive);
            archive_to_delete = None;
        } else if is_native_platform(platform) {
            archive_to_delete = self.finalize_native_base(id, job, &mut record)?;
        } else if is_ps3_platform(platform) {
            archive_to_delete = self.finalize_ps3_base(id, job, &mut record)?;
        } else if is_ps4_platform(platform) && should_extract(platform, archive) {
            archive_to_delete = self.finalize_ps4_base(id, job, &mut record)?;
        } else if should_extract(platform, archive) {
            let dest = extraction_dir(archive);
            let mut progress = |processed, total| self.on_install_progress(id, processed, total);
            extract_archive(archive, &dest, &mut progress)?;
            let Some(launch) = select_launch_file(&dest, &archive_stem(archive)) else {
                let _ = fs::remove_dir_all(&dest);
                return Err(LibraryError::NoLaunchFile);
            };
            make_executable(&launch);
            record.extracted_path = path_string(&launch);
            record.extracted_dir = path_string(&dest);
            archive_to_delete = Some(archive);
        } else {
            // Nothing to extract: the downloaded file is the install.
            if is_appimage(archive) {
                make_executable(archive);
            }
            record.archive_path = path_string(archive);
            archive_to_delete = None;
        }

        self.registry.upsert(&record)?;

        // Bound to a `let` first so the read lock is released before the
        // hook runs — an `if let` scrutinee would keep the temporary guard
        // alive for the whole body, holding the lock across arbitrary hook
        // code.
        let hook = self.image_hook.read().unwrap().clone();
        if let Some(hook) = hook {
            hook(ImageFields {
                cover_small_path: record.cover_small_path.clone(),
                cover_large_path: record.cover_large_path.clone(),
                screenshot_urls: record.screenshot_urls.clone(),
            });
        }

        // RPCS3 only sees a routed game once `games.yml` names it, so this
        // runs on every PS3 install that produced an id — never on a failed
        // one, and never before the row exists (install_mixin.py:120).
        if is_ps3_platform(platform) && !record.ps3_game_id.trim().is_empty() {
            self.write_games_yml(&record);
        }

        // Same lock discipline as the image hook above. `finalize_native_update`
        // fires the same hook for the merge path, so every finalize that
        // writes a full registry row reports one.
        let finalized = self.game_finalized_hook.read().unwrap().clone();
        if let Some(finalized) = finalized {
            finalized(record.clone());
        }

        if is_xbox360_platform(platform) {
            self.queue_xbox360_content(job);
        }

        // Only after a successful write, and only when the archive has been
        // superseded by an extraction — a finalize failure keeps the archive
        // so a retry skips the re-download (doc 03 invariant 5).
        if let Some(archive) = archive_to_delete {
            if !delete_with_retry(archive) {
                *warning = format!("could not delete archive: {}", archive.display());
            }
        }
        Ok(())
    }

    /// D16 / doc 03 §13: after a base Xbox 360 install finishes, its update
    /// and then its DLC are queued automatically and silently.
    ///
    /// The file ids come from the base job's OWN `detail`, so no server
    /// round trip happens here, and admission failures are not surfaced: a
    /// duplicate (the user asked for the same content by hand a moment ago)
    /// is simply dropped by the queue. This runs while the base entry still
    /// owns the finalize slot, so both content jobs are queued rather than
    /// started, and the FIFO gives base -> update -> DLC.
    fn queue_xbox360_content(self: &Arc<Self>, job: &InstallJob) {
        let Ok(library) = self.library_root() else {
            return;
        };
        for kind in [ContentKind::Update, ContentKind::Dlc] {
            let file_ids = content_file_ids(&job.detail.files, kind);
            if file_ids.is_empty() {
                continue;
            }
            let (key, title, payload) = content_job(
                &library,
                &job.detail,
                kind,
                InstallMode::Xbox360Content,
                file_ids,
                job.client.clone(),
            );
            self.admit(
                key,
                &title,
                &job.detail.platform_name,
                InstallMode::Xbox360Content.kind(),
                payload,
            );
        }
    }

    /// Applies a PS4 update/DLC archive to the game's installed title-id
    /// tree and records what was applied.
    ///
    /// No new registry row, no image hook and no game-finalized hook: this
    /// modifies an existing install rather than creating one. The archive is
    /// deleted by [`specials::ps4::apply_content`] itself, which is also
    /// where a delete failure becomes a warning.
    fn finalize_ps4_content(
        &self,
        id: u64,
        job: &InstallJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let row = self.current_row(job.rom_id)?;
        let archive = job.primary_archive.as_path();
        let staging = extraction_dir(archive);
        // Always `Some` for a content job (`content_job` sets it); the
        // fallback keeps a malformed job from panicking.
        let kind = job.content_kind.unwrap_or(ContentKind::Update);
        let extract = self.extract_fn(id);

        let applied = specials::ps4::apply_content(&row, archive, kind, &staging, &extract)
            .map_err(LibraryError::Extract)?;
        self.registry
            .update_ps4_content(job.rom_id, &applied.game_id, &applied.content_json)?;
        if !applied.warning.is_empty() {
            append_warning(warning, &applied.warning);
        }
        Ok(())
    }

    /// Copies every STFS package in an Xbox 360 update/DLC archive into
    /// Xenia's content directory.
    ///
    /// Nothing is written to the registry: Xenia finds its content by
    /// scanning that directory, so the install IS the copy. D16: the archive
    /// is deleted here, after a successful apply, exactly like a base
    /// install's is.
    fn finalize_xbox360_content(
        &self,
        id: u64,
        job: &InstallJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let row = self.current_row(job.rom_id)?;
        let content_root = self
            .xenia_content_root(&row.platform)
            .map_err(LibraryError::Extract)?;
        let archive = job.primary_archive.as_path();
        let staging = extraction_dir(archive);
        let extract = self.extract_fn(id);

        // The expected title id is left blank: the reference passes none
        // either, so a package for a different title is copied under its own
        // id rather than rejected.
        let (_applied, apply_warning) =
            specials::xenia::apply_content_archive(archive, &content_root, &staging, "", &extract)
                .map_err(LibraryError::Extract)?;
        if !apply_warning.is_empty() {
            append_warning(warning, &apply_warning);
        }
        if !delete_with_retry(archive) {
            append_warning(
                warning,
                &format!("could not delete archive: {}", archive.display()),
            );
        }
        Ok(())
    }

    /// Merges a native (Windows) game's update archive into its existing
    /// install and re-registers the resulting row.
    fn finalize_native_update(
        &self,
        id: u64,
        job: &InstallJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        let row = self.current_row(job.rom_id)?;
        let archive = job.primary_archive.as_path();
        let temp_dir = specials::native::update_temp_dir(&row);
        let extract = self.extract_fn(id);

        let updated =
            specials::native::apply_update(&row, &job.detail, archive, &temp_dir, &extract)
                .map_err(LibraryError::Extract)?;
        self.registry.upsert(&updated.row)?;
        if !updated.warning.is_empty() {
            append_warning(warning, &updated.warning);
        }

        // The merged row replaces the installed one, so the app's follow-on
        // work (firmware, the update-set recompute) has to run here too —
        // `finalize_base` never sees a native update. Same lock discipline:
        // the guard is dropped before the hook runs.
        let finalized = self.game_finalized_hook.read().unwrap().clone();
        if let Some(finalized) = finalized {
            finalized(updated.row.clone());
        }
        Ok(())
    }

    /// The registry row for `rom_id` as it stands RIGHT NOW.
    ///
    /// Every content job reads this at finalize time rather than trusting
    /// the plan: an install can finish, or be uninstalled, between admission
    /// and the moment the archive is applied. `find`'s title/platform
    /// fallback is skipped (both are passed blank) and `installed_match` has
    /// the final word, so this only ever returns a row that genuinely
    /// belongs to `rom_id`.
    fn current_row(&self, rom_id: i64) -> Result<InstalledGame, LibraryError> {
        self.registry
            .find(Some(rom_id), "", "")?
            .filter(|row| installed_match(row, rom_id))
            .ok_or_else(|| LibraryError::Registry(NOT_INSTALLED.to_string()))
    }

    /// The extraction callback the specials take, bound to entry `id`'s
    /// install-progress sink.
    fn extract_fn(&self, id: u64) -> impl Fn(&Path, &Path) -> Result<(), LibraryError> + '_ {
        move |archive: &Path, dest: &Path| {
            let mut progress = |processed, total| self.on_install_progress(id, processed, total);
            extract_archive(archive, dest, &mut progress)
        }
    }

    /// Xenia's content directory for `platform`, or the verbatim message the
    /// drawer row shows when it cannot be resolved.
    ///
    /// On a non-Windows host the emulator has to exist and has to be able to
    /// run here first: a Windows-only Xenia build cannot apply anything, and
    /// saying so is more use than "content directory not found". Ports
    /// `_apply_xenia_content_archive_without_ui`'s preamble
    /// (`install_mixin.py:348-383`).
    fn xenia_content_root(&self, platform: &str) -> Result<PathBuf, String> {
        let config =
            Config::load(&self.config_path).map_err(|_| XENIA_CONTENT_ROOT_UNKNOWN.to_string())?;
        let name = default_emulator_name_for_platform(
            &config.emulators,
            &config.default_emulators,
            platform,
            &self.profiles,
            &config.retroarch_cores,
        );
        let entry = emulator_entry_by_name(&config.emulators, &name);
        let path = entry.map(|e| e.path.as_str()).unwrap_or("");

        if !cfg!(windows) {
            if name.trim().is_empty() {
                return Err(XBOX360_NEEDS_LINUX_EMULATOR.to_string());
            }
            let profile = profile_for_entry(&name, path, &self.profiles);
            if profile.is_some_and(|profile| !profile_available_on_host(profile, HOST_PLATFORM)) {
                return Err(XBOX360_EMULATOR_WINDOWS_ONLY.to_string());
            }
        }

        let args = entry
            .map(|e| split_template(&e.args).unwrap_or_default())
            .unwrap_or_default();
        // Defensive, and deliberately kept: today no branch of
        // `xenia_directory_settings` can yield a blank content root (every
        // storage-root branch is non-empty and the config override falls back
        // to the literal "content"), but the reference checks it and a future
        // reader change must not silently install into "".
        let content_root = xenia_directory_settings(path, &args).content_root;
        if content_root.trim().is_empty() {
            return Err(XENIA_CONTENT_ROOT_UNKNOWN.to_string());
        }
        Ok(PathBuf::from(content_root))
    }

    /// Finalizes a native (Windows) base install into the game's own home
    /// directory: `<home>/game/` holds the extracted tree, `<home>/prefix`
    /// the Wine prefix on a non-Windows host, and the `game.json` sidecar
    /// downloaded next to the archive supplies the metadata the server did
    /// not. Ports `InstallFinalizeWorker.run`'s native branch
    /// (`workers.py:576-587`) plus `native_extracted_dir_for_archive_path`
    /// (`archive_preparation.py:846`).
    fn finalize_native_base<'a>(
        &self,
        id: u64,
        job: &'a InstallJob,
        record: &mut InstalledGame,
    ) -> Result<Option<&'a Path>, LibraryError> {
        let archive = job.primary_archive.as_path();
        // Always `Some` for a native job (`plan_native_install` sets it);
        // the archive's own parent IS that directory, so the fallback keeps
        // the layout right rather than guarding with an error.
        let game_dir = job
            .native_game_dir
            .as_deref()
            .or_else(|| archive.parent())
            .unwrap_or(archive);
        record.native_game_dir = path_string(game_dir);

        let mut archive_to_delete = None;
        // D13: a bare disc image or loose executable served under a native
        // platform is not an archive, whatever `should_extract`'s table says.
        if should_extract(&job.detail.platform_name, archive) && is_extractable_archive(archive) {
            // Fixed at `<home>/game` whatever the archive is called, so
            // every native install has the same shape.
            let dest = game_dir.join("game");
            let mut progress = |processed, total| self.on_install_progress(id, processed, total);
            extract_archive(archive, &dest, &mut progress)?;
            let Some(launch) = select_launch_file(&dest, &archive_stem(archive)) else {
                let _ = fs::remove_dir_all(&dest);
                return Err(LibraryError::NoLaunchFile);
            };
            make_executable(&launch);
            record.extracted_path = path_string(&launch);
            record.extracted_dir = path_string(&dest);
            #[cfg(not(windows))]
            {
                // A Windows game on a non-Windows host launches through
                // Wine/Proton, which needs a prefix of its own. Created
                // here so the first launch does not have to.
                let prefix = game_dir.join("prefix");
                if fs::create_dir_all(&prefix).is_ok() {
                    record.native_wineprefix = path_string(&prefix);
                }
            }
            archive_to_delete = Some(archive);
        } else {
            // D13: a native payload this engine cannot extract (a disc
            // image, a bare executable) IS the install, exactly as for any
            // other platform's non-extractable download.
            if is_appimage(archive) {
                make_executable(archive);
            }
            record.archive_path = path_string(archive);
        }

        if let Some(sidecar) = &job.game_json_target {
            if let Ok(bytes) = fs::read(sidecar) {
                if let Some(parsed) = specials::native::parse_game_json(&bytes) {
                    specials::native::apply_game_json(record, &parsed);
                }
            }
        }
        Ok(archive_to_delete)
    }

    /// Finalizes a PlayStation 3 base install.
    ///
    /// An extractable archive is unpacked into a staging directory and then
    /// either short-circuited (a lone ISO, which RPCS3 boots directly, is
    /// moved next to the archive) or routed into RPCS3's `dev_hdd0` VFS by
    /// [`specials::ps3::route`]. A non-extractable one is the install, and a
    /// bare `.iso` is additionally recorded as `ps3_iso_path`. Ports
    /// `prepare_installed_game_without_ui`'s PS3 branches
    /// (`archive_preparation.py:1135-1229`).
    fn finalize_ps3_base<'a>(
        &self,
        id: u64,
        job: &'a InstallJob,
        record: &mut InstalledGame,
    ) -> Result<Option<&'a Path>, LibraryError> {
        let archive = job.primary_archive.as_path();
        let title = job.detail.name.as_str();

        if !should_extract(&job.detail.platform_name, archive) {
            if is_appimage(archive) {
                make_executable(archive);
            }
            record.archive_path = path_string(archive);
            if lowercase_suffix(archive).as_deref() == Some("iso") {
                record.ps3_iso_path = path_string(archive);
            }
            return Ok(None);
        }

        let staging = extraction_dir(archive);
        let mut progress = |processed, total| self.on_install_progress(id, processed, total);
        extract_archive(archive, &staging, &mut progress)?;
        // The PS3 branch does not pick a launch file, so this is the only
        // check that an archive of nothing but empty directories cannot
        // pass for an install.
        if !contains_regular_file(&staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(LibraryError::NoLaunchFile);
        }

        if let Some(iso) = specials::ps3::iso_only_file(&staging) {
            let destination = archive
                .parent()
                .unwrap_or(Path::new(""))
                .join(iso.file_name().unwrap_or_default());
            if destination != iso {
                if destination.exists() {
                    fs::remove_file(&destination)?;
                }
                move_file(&iso, &destination)?;
            }
            record.extracted_path = path_string(&destination);
            record.extracted_dir = String::new();
            record.ps3_iso_path = path_string(&destination);
            let _ = fs::remove_dir_all(&staging);
            return Ok(Some(archive));
        }

        let roots = self
            .ps3_roots(&job.detail.platform_name, title)
            .map_err(LibraryError::Extract)?;
        let outcome = specials::ps3::route(&staging, &roots, title, &|iso, dest| {
            extract_iso_with_system_7z(iso, dest)
        })
        .map_err(LibraryError::Extract)?;

        record.ps3_game_id = outcome.game_id;
        record.ps3_trophy_paths = outcome.trophy_paths_json;
        record.extracted_path = outcome.extracted_path;
        record.extracted_dir = outcome.extracted_dir;
        Ok(Some(archive))
    }

    /// Finalizes a PlayStation 4 base install: the generic extract-and-pick
    /// path, but `eboot.bin` under a title-id root wins over whatever the
    /// generic ranking would have chosen, and the title id detected from the
    /// resulting layout is recorded. Ports the PS4 hooks in
    /// `select_extracted_launch_file` (`archive_preparation.py:990`) and
    /// `prepare_installed_game_without_ui` (`archive_preparation.py:1177`).
    fn finalize_ps4_base<'a>(
        &self,
        id: u64,
        job: &'a InstallJob,
        record: &mut InstalledGame,
    ) -> Result<Option<&'a Path>, LibraryError> {
        let archive = job.primary_archive.as_path();
        let dest = extraction_dir(archive);
        let mut progress = |processed, total| self.on_install_progress(id, processed, total);
        extract_archive(archive, &dest, &mut progress)?;

        let pool = regular_files_under(&dest);
        let launch = specials::ps4::select_ps4_launch_file(&dest, &pool)
            .or_else(|| select_launch_file(&dest, &archive_stem(archive)));
        let Some(launch) = launch else {
            let _ = fs::remove_dir_all(&dest);
            return Err(LibraryError::NoLaunchFile);
        };

        make_executable(&launch);
        record.extracted_path = path_string(&launch);
        record.extracted_dir = path_string(&dest);
        record.ps4_game_id = specials::ps4::detect_title_id(&dest, &launch, archive);
        Ok(Some(archive))
    }

    /// The RPCS3 VFS roots a PS3 install for `platform` routes into.
    ///
    /// `Err` carries the verbatim message the drawer row shows when no
    /// `dev_hdd0` can be resolved at all — neither from a configured
    /// emulator's `vfs.yml` nor from the library's own `.vfs` fallback.
    fn ps3_roots(&self, platform: &str, title: &str) -> Result<Ps3Roots, String> {
        let config = Config::load(&self.config_path).map_err(|_| no_dev_hdd0_message(title))?;
        ps3_roots_from_config(&config, &self.profiles, platform, title)
    }

    /// Names the just-installed PS3 game in RPCS3's `games.yml`, so the
    /// emulator lists it without a rescan. Silent when RPCS3 is not
    /// configured, or when its data root / `dev_hdd0` cannot be resolved:
    /// this never fails an install (`install_mixin.py:512-525`).
    fn write_games_yml(&self, record: &InstalledGame) {
        let Ok(roots) = self.ps3_roots(&record.platform, &record.title) else {
            return;
        };
        let Some(data_root) = roots.data_root.as_deref() else {
            return;
        };
        update_games_yml(
            data_root,
            &record.ps3_game_id,
            &roots.dev_hdd0,
            roots.games_root.as_deref(),
        );
    }

    /// The blocking half of finalizing an emulator: extraction into the
    /// install directory, supplemental merges, executable selection, the
    /// config entry, and archive cleanup. Ports the emulator half of the
    /// reference's post-download install path (`install_mixin.py:690-780`,
    /// `autoconfig.py:19-87`); nothing here writes to the SQLite registry.
    fn finalize_emulator(
        &self,
        id: u64,
        job: &EmulatorJob,
        warning: &mut String,
    ) -> Result<(), LibraryError> {
        // Unreachable: the download task fills `resolved` in before it can
        // succeed, and only a successful download reaches finalize.
        let Some(paths) = job.resolved.as_ref() else {
            return Err(LibraryError::Extract(
                "the emulator download plan was lost".to_string(),
            ));
        };
        let install_dir = paths.install_dir.as_path();
        let archive = paths.archive.as_path();
        let mut progress = |processed, total| self.on_install_progress(id, processed, total);
        // Archives that have been superseded by their extracted contents.
        // Anything left out of this list stays on disk: an AppImage IS the
        // install, and a failure before this point must keep every archive so
        // a retry skips the finished downloads (doc 03 invariant 5).
        let mut extracted_archives: Vec<&Path> = Vec::new();

        if should_extract(EMULATOR_PLATFORM, archive) {
            let staging = install_dir.join(EXTRACT_TMP_DIR);
            extract_archive(archive, &staging, &mut progress)?;
            let merged = merge_tree_into(&staging, install_dir);
            let _ = fs::remove_dir_all(&staging);
            merged?;
            extracted_archives.push(archive);
        }

        for (index, supplemental) in paths.supplementals.iter().enumerate() {
            if !supplemental.is_file() {
                continue;
            }
            // A supplemental that is not an archive (an AppImage, a firmware
            // blob) is already where it belongs: a sibling of the primary.
            if !should_extract(EMULATOR_PLATFORM, supplemental) {
                continue;
            }
            // Numbered by position in the downloaded list, which is a
            // scratch name only — the file names themselves carry the
            // reference's spec index, assigned when the plan was built.
            let staging = install_dir.join(format!(".supp-tmp-{}", index + 1));
            extract_archive(supplemental, &staging, &mut progress)?;
            let merged = merge_tree_into(&staging, install_dir);
            let _ = fs::remove_dir_all(&staging);
            merged?;
            extracted_archives.push(supplemental);
        }

        if job.compat_tool {
            let Some(proton_dir) = compat::find_proton_dir(install_dir) else {
                return Err(LibraryError::Extract(NO_PROTON_ENTRY_POINT.to_string()));
            };
            self.write_compat_tool_entry(job, &proton_dir, &paths.resolved.release_tag)?;

            // Only after a successful config write, matching the game path.
            for path in extracted_archives {
                if !delete_with_retry(path) {
                    append_warning(
                        warning,
                        &format!("could not delete archive: {}", path.display()),
                    );
                }
            }

            // Bound to a `let` first so the read lock is released before the
            // hook runs — never invoked under a lock.
            let hook = self.compat_tools_hook.read().unwrap().clone();
            if let Some(hook) = hook {
                hook();
            }
            return Ok(());
        }

        let Some(exe) = emu_install::select_executable(&job.profile_name, install_dir, archive)
        else {
            return Err(LibraryError::Extract(NO_EMULATOR_EXECUTABLE.to_string()));
        };
        make_executable(&exe);

        let fresh = self.write_emulator_entry(job, &paths.resolved, &exe)?;
        self.sync_autoconfig(&job.profile_name, warning);

        // Only after a successful config write, matching the game path.
        for path in extracted_archives {
            if !delete_with_retry(path) {
                append_warning(
                    warning,
                    &format!("could not delete archive: {}", path.display()),
                );
            }
        }

        // Bound to a `let` first so the read lock is released before the
        // hook runs — never invoked under a lock.
        let hook = self.emulator_installed_hook.read().unwrap().clone();
        if let Some(hook) = hook {
            hook(EmulatorInstalled {
                name: job.profile_name.clone(),
                fresh,
                compat_tool: false,
            });
        }
        Ok(())
    }

    /// D1 call site A: runs [`autoconfig::sync_new_emulator`] for the entry
    /// the install just wrote, before the archive cleanup.
    ///
    /// Autoconfig NEVER fails an install. A config error, or any writer that
    /// reached nothing, appends ONE line to the finalize warning — exactly
    /// like a failed archive delete — and the install still reports
    /// `Completed`. No credential can appear in that line: the report names
    /// emulators and writers only.
    fn sync_autoconfig(&self, entry_name: &str, warning: &mut String) {
        let library_path = Config::load(&self.config_path)
            .map(|config| config.library_path)
            .unwrap_or_default();
        let platforms = self.known_platforms();
        let ctx = autoconfig::SyncContext {
            config_path: &self.config_path,
            platforms: &platforms,
            ps3_library_path: autoconfig::ps3_library_path(&library_path),
            ra: self.ra_credentials(),
            profiles: &self.profiles,
        };
        match autoconfig::sync_new_emulator(entry_name, &ctx) {
            Ok(report) if !report.warnings.is_empty() => append_warning(
                warning,
                &format!("emulator autoconfig: {}", report.warnings.join("; ")),
            ),
            Ok(_) => {}
            Err(e) => append_warning(warning, &format!("emulator autoconfig: {e}")),
        }
    }

    /// Writes (or replaces) `job`'s emulator entry in the config file.
    /// Returns whether this was a FRESH install: `true` when no
    /// `[[emulators]]` entry with this name existed before the write.
    ///
    /// An existing entry with the same name is replaced AT ITS INDEX, so the
    /// user's ordering survives a reinstall; the match is exact, mirroring
    /// the `save_emulator` command's replace rule.
    fn write_emulator_entry(
        &self,
        job: &EmulatorJob,
        resolved: &ResolvedDownload,
        exe: &Path,
    ) -> Result<bool, LibraryError> {
        let mut config = Config::load(&self.config_path)?;
        let entry = EmulatorEntry {
            name: job.profile_name.clone(),
            path: path_string(exe),
            args: job.profile_args.clone(),
            source_id: job.source_id.clone(),
            source_provider: resolved.provider.clone(),
            source_owner: resolved.owner.clone(),
            source_repo: resolved.repo.clone(),
            // The CONFIGURED tag, not the resolved one — a `latest` pin is
            // recorded as `latest` so the entry keeps meaning "track the
            // newest release" (emulator_ui_mixin.py:1168-1175).
            source_release_tag: job.configured_tag.clone(),
            // The layer-1 autoconfig fields are not this writer's concern:
            // `autoconfig::entry` fills them on the next configure pass.
            ..Default::default()
        };
        let fresh = match config
            .emulators
            .iter()
            .position(|existing| existing.name == entry.name)
        {
            Some(index) => {
                config.emulators[index] = entry;
                false
            }
            None => {
                config.emulators.push(entry);
                true
            }
        };
        config.save(&self.config_path)?;
        Ok(fresh)
    }

    /// Writes (or replaces) `job`'s [`CompatToolInstall`] config record.
    ///
    /// An existing record for the same `source_id` is replaced AT ITS INDEX
    /// — the compat-tool counterpart of [`Self::write_emulator_entry`]'s
    /// by-name replace rule, matched by `source_id` instead because a
    /// compat tool's `name` is not guaranteed unique the way an emulator
    /// entry's is.
    ///
    /// `release_tag` is the RESOLVED tag the forge actually downloaded
    /// (`ResolvedDownload::release_tag`), NOT `job.configured_tag`: a
    /// `latest`-pinned compat tool must record which concrete release is on
    /// disk, unlike an ordinary emulator entry's `source_release_tag` (which
    /// deliberately keeps recording the pin itself, so it keeps tracking the
    /// newest release on a later reinstall).
    fn write_compat_tool_entry(
        &self,
        job: &EmulatorJob,
        proton_dir: &Path,
        release_tag: &str,
    ) -> Result<(), LibraryError> {
        let mut config = Config::load(&self.config_path)?;
        let entry = CompatToolInstall {
            name: job.profile_name.clone(),
            path: path_string(proton_dir),
            source_id: job.source_id.clone(),
            release_tag: release_tag.to_string(),
        };
        match config
            .compat_tool_installs
            .iter()
            .position(|existing| existing.source_id == entry.source_id)
        {
            Some(index) => config.compat_tool_installs[index] = entry,
            None => config.compat_tool_installs.push(entry),
        }
        config.save(&self.config_path)?;
        Ok(())
    }

    fn on_download_progress(&self, id: u64, downloaded: u64, total: u64, speed: f64) {
        self.queue
            .lock()
            .unwrap()
            .set_progress(id, downloaded, total, speed);
        self.notify_throttled(DOWNLOAD_NOTIFY_INTERVAL);
    }

    fn on_install_progress(&self, id: u64, processed: u64, total: u64) {
        self.queue
            .lock()
            .unwrap()
            .set_install_progress(id, processed, total);
        self.notify_throttled(INSTALL_NOTIFY_INTERVAL);
    }

    /// Notifies only when at least `min_gap` has passed since the last
    /// notification, so a fast download cannot flood the UI.
    fn notify_throttled(&self, min_gap: Duration) {
        let due = {
            let mut last = self.last_emit.lock().unwrap();
            let due = last.elapsed() >= min_gap;
            if due {
                *last = Instant::now();
            }
            due
        };
        if due {
            self.emit();
        }
    }

    /// Notifies unconditionally — used for every status transition.
    fn notify_now(&self) {
        *self.last_emit.lock().unwrap() = Instant::now();
        self.emit();
    }

    /// Hands the snapshot to the listener with NO lock held: the callback is
    /// arbitrary UI code and must never be able to block a state change.
    fn emit(&self) {
        let listener = self.notify.read().unwrap().clone();
        let Some(listener) = listener else {
            return;
        };
        let snapshot = self.snapshot();
        listener(snapshot);
    }
}

// --- emulator download ------------------------------------------------------

/// Resolves `job` against its forge and downloads the primary asset plus
/// every supplemental, recording the resulting paths on `job` for finalize.
///
/// Ports `InstallDownloadWorker.run`'s resolve-then-download order
/// (`workers.py:57-138`): the release lookups happen here, inside the
/// download task, so a resolution failure ends up on the drawer row.
async fn download_emulator(
    job: &mut EmulatorJob,
    cancel: &AtomicBool,
    on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
) -> Result<(), LibraryError> {
    let forge = job.forge.clone();
    let primary = forge
        .resolve(&job.raw_source, &job.profile_name)
        .await
        .map_err(|e| LibraryError::Extract(e.0))?;
    // A cancel that arrived while the forge was being queried must not turn
    // into a download.
    if cancel.load(Ordering::Relaxed) {
        return Err(LibraryError::Cancelled);
    }

    let supplementals = resolve_supplementals(&forge, job).await?;

    // The install directory is named from the profile name and the
    // CONFIGURED tag, fixed before the asset is known — `_archive_name_override`
    // with no asset suffix applied, then its stem (emulator_ui_mixin.py:1186-1190
    // + install_mixin.py:1444). The asset-suffix rewrite below renames only
    // the file INSIDE that already-fixed directory.
    let dir_name = emu_install::archive_file_name(&job.profile_name, &job.configured_tag, "");
    let stem = file_stem_of(&dir_name);
    // `job.library` already IS the managed compat-tools root for a compat
    // job ([`InstallService::install_compat_tool`]), so only the directory
    // FUNCTION differs — never an extra path segment on top.
    let install_dir = if job.compat_tool {
        emu_install::compat_tool_install_dir(&job.library, &stem)
    } else {
        emu_install::emulator_install_dir(&job.library, &stem)
    };

    let archive_name =
        emu_install::archive_file_name(&job.profile_name, &job.configured_tag, &primary.asset_name);
    let archive = install_dir.join(safe_file_name(&archive_name, &primary.asset_name)?);

    // Every destination name is validated before ANY request goes out, so a
    // hostile asset name cannot write a byte anywhere.
    let mut supplemental_paths = Vec::new();
    for (index, supplemental) in &supplementals {
        let name = emu_install::supplemental_file_name(&archive, *index, &supplemental.asset_name);
        supplemental_paths.push(install_dir.join(safe_file_name(&name, &supplemental.asset_name)?));
    }

    let mut targets = vec![FileTarget {
        url_path: primary.download_url.clone(),
        query: Vec::new(),
        dest: archive.clone(),
        expected_size: primary.size,
    }];
    let mut github_headers = vec![(
        primary.download_url.clone(),
        needs_github_headers(&primary.provider),
    )];

    for ((_, supplemental), dest) in supplementals.iter().zip(&supplemental_paths) {
        targets.push(FileTarget {
            url_path: supplemental.download_url.clone(),
            query: Vec::new(),
            dest: dest.clone(),
            expected_size: supplemental.size,
        });
        github_headers.push((
            supplemental.download_url.clone(),
            needs_github_headers(&supplemental.provider),
        ));
    }

    job.resolved = Some(ResolvedPaths {
        install_dir,
        archive,
        supplementals: supplemental_paths,
        resolved: primary,
    });

    let provider = ForgeProvider::new(&forge, github_headers);
    download_targets(&provider, &targets, cancel, on_progress).await
}

/// Resolves every `supplemental_downloads` entry of `job`'s RAW source,
/// paired with its 1-based index (`_download_supplemental_archives`,
/// workers.py:124-138).
///
/// The reference numbers the entries BEFORE skipping the unusable ones, so
/// a skipped spec still consumes its index and the file names of the
/// supplementals after it do not shift. A spec that resolves to no download
/// URL is skipped; a spec that fails to resolve fails the whole install.
async fn resolve_supplementals(
    forge: &ForgeClient,
    job: &EmulatorJob,
) -> Result<Vec<(usize, ResolvedDownload)>, LibraryError> {
    let Some(Value::Array(specs)) = job.raw_source.get("supplemental_downloads") else {
        return Ok(Vec::new());
    };
    let mut resolved = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if !spec.is_object() {
            continue;
        }
        let supplemental = forge
            .resolve(spec, &job.profile_name)
            .await
            .map_err(|e| LibraryError::Extract(e.0))?;
        if supplemental.download_url.trim().is_empty() {
            continue;
        }
        resolved.push((index + 1, supplemental));
    }
    Ok(resolved)
}

/// `name`'s file stem, or the whole name when it has none.
fn file_stem_of(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// Returns `name` when it is a single, ordinary file name, and an error
/// naming the offending asset otherwise.
///
/// The naming helpers copy a release asset's own name into the result (whole
/// for an AppImage, its suffix otherwise), and that name is remote data: it
/// comes from a release payload served by whatever host a `gitea` or
/// `direct` source's `base_url`/`page_url` points at. Joining `../../x` onto
/// the install directory would write outside the library, so it is rejected
/// here, in the wiring, before the path is built.
///
/// The reference gets this for free — `Path.with_name`/`with_suffix` raise
/// `ValueError` on a name containing a separator (workers.py:147-163) — so
/// failing the job is also the reference's behavior. Both separators are
/// rejected on every platform: a config written on one OS is read on the
/// other.
fn safe_file_name<'a>(name: &'a str, asset_name: &str) -> Result<&'a str, LibraryError> {
    let is_plain_component = !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && Path::new(name).file_name().is_some_and(|only| only == name);
    if is_plain_component {
        return Ok(name);
    }
    Err(LibraryError::Extract(format!(
        "Refusing to install release asset '{asset_name}': it does not name a plain file."
    )))
}

// --- uninstall ---------------------------------------------------------------

/// What kind of thing one uninstall step removes. The variant decides both
/// how the removal is done and the verbatim message a failure produces
/// (`install_cleanup.py:19-91`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemovalLabel {
    /// A plain file — an archive, or a PS3 ISO.
    File,
    /// A PS3 trophy directory, which the reference names separately.
    TrophyDir,
    /// Any other directory: an extraction directory, a native game's home
    /// directory, a multi-file game's directory.
    Folder,
}

impl RemovalLabel {
    fn message(self, path: &Path, error: &str) -> String {
        let what = match self {
            RemovalLabel::File => "Could not remove file",
            RemovalLabel::TrophyDir => "Could not remove PS3 trophy directory",
            RemovalLabel::Folder => "Could not remove folder",
        };
        format!("{what}: {}\n{error}", path.display())
    }
}

/// One thing an uninstall removes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Removal {
    path: PathBuf,
    label: RemovalLabel,
}

/// Runs every removal in order, CONTINUING past a failure, and returns one
/// message line per failure (D11).
///
/// The remover is injected so the aggregation itself is testable without a
/// filesystem that can be made to fail on demand.
fn run_removals(
    steps: &[Removal],
    remove: &mut dyn FnMut(&Removal) -> Result<(), String>,
) -> Vec<String> {
    let mut failures = Vec::new();
    for step in steps {
        if let Err(error) = remove(step) {
            failures.push(step.label.message(&step.path, &error));
        }
    }
    failures
}

/// The production remover: a file is unlinked, a directory is removed whole.
fn apply_removal(step: &Removal) -> Result<(), String> {
    match step.label {
        RemovalLabel::File => fs::remove_file(&step.path).map_err(|e| e.to_string()),
        RemovalLabel::TrophyDir | RemovalLabel::Folder => {
            remove_dir_tree(&step.path).map_err(|e| e.to_string())
        }
    }
}

/// The ordered removal plan for `record`, branching by platform exactly as
/// `remove_game_files` does (`install_cleanup.py:7-91`).
///
/// Every step's path is checked to exist in the expected shape here rather
/// than at removal time: the reference skips a missing or wrong-shaped path
/// silently, and nothing else is touching these paths between the two
/// points. The reference's early `return`s become "stop adding steps": a
/// native game whose home directory exists never falls through to the
/// candidate extraction directories, and neither does a multi-file game.
fn uninstall_steps(record: &InstalledGame, library: &Path) -> Vec<Removal> {
    let mut steps = Vec::new();
    let push_dir = |path: &Path, label: RemovalLabel, steps: &mut Vec<Removal>| {
        if path.is_dir() {
            steps.push(Removal {
                path: path.to_path_buf(),
                label,
            });
        }
    };

    let name = archive_name(&record.rom_file_name, &record.title, &record.platform);
    let archives = candidate_archives(library, &record.platform, &record.archive_path, &name);
    let extracted = candidate_extracted_dirs(&archives, &record.extracted_dir);

    if is_ps3_platform(&record.platform) {
        let iso = record.ps3_iso_path.trim();
        if !iso.is_empty() && Path::new(iso).is_file() {
            steps.push(Removal {
                path: PathBuf::from(iso),
                label: RemovalLabel::File,
            });
        }
        for trophy in parse_trophy_paths(&record.ps3_trophy_paths) {
            push_dir(&trophy, RemovalLabel::TrophyDir, &mut steps);
        }
        for dir in &extracted {
            push_dir(dir, RemovalLabel::Folder, &mut steps);
        }
        return steps;
    }

    if is_native_platform(&record.platform) {
        let game_dir = record.native_game_dir.trim();
        if !game_dir.is_empty() && Path::new(game_dir).is_dir() {
            steps.push(Removal {
                path: PathBuf::from(game_dir),
                label: RemovalLabel::Folder,
            });
            return steps;
        }
        for dir in &extracted {
            push_dir(dir, RemovalLabel::Folder, &mut steps);
        }
        return steps;
    }

    let multi_file = record.multi_file_game_dir.trim();
    if !multi_file.is_empty() && Path::new(multi_file).is_dir() {
        steps.push(Removal {
            path: PathBuf::from(multi_file),
            label: RemovalLabel::Folder,
        });
        return steps;
    }

    for archive in &archives {
        if archive.is_file() {
            steps.push(Removal {
                path: archive.clone(),
                label: RemovalLabel::File,
            });
        }
    }
    for dir in &extracted {
        push_dir(dir, RemovalLabel::Folder, &mut steps);
    }
    steps
}

/// The trophy directories a record's `ps3_trophy_paths` JSON names. Lenient:
/// anything that is not a JSON array of strings yields no paths, matching
/// the reference's `except (ValueError, TypeError): trophy_paths = []`.
fn parse_trophy_paths(raw: &str) -> Vec<PathBuf> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Vec<String>>(raw)
        .map(|paths| paths.into_iter().map(PathBuf::from).collect())
        .unwrap_or_default()
}

// --- PS3 VFS roots -----------------------------------------------------------

/// The message a PS3 install fails with when no `dev_hdd0` can be resolved.
fn no_dev_hdd0_message(title: &str) -> String {
    format!("No PS3 VFS dev_hdd0 path configured for {title}")
}

/// [`InstallService::ps3_roots`]'s pure core: resolves the RPCS3 VFS roots
/// from an already-loaded config.
///
/// The `dev_hdd0` and `games` roots come from the default PS3 emulator's
/// `vfs.yml` when there is one, and from `<library>/PlayStation 3/.vfs/...`
/// when there is not — so a PS3 game installs into a usable layout before
/// RPCS3 is ever configured. The data root (D4) is only ever the configured
/// emulator's: with no entry there is nowhere to write `games.yml`.
fn ps3_roots_from_config(
    config: &Config,
    profiles: &[EmulatorProfile],
    platform: &str,
    title: &str,
) -> Result<Ps3Roots, String> {
    let ps3_library = autoconfig::ps3_library_path(&config.library_path);
    let name = default_emulator_name_for_platform(
        &config.emulators,
        &config.default_emulators,
        platform,
        profiles,
        &config.retroarch_cores,
    );
    let entry = emulator_entry_by_name(&config.emulators, &name);
    let path = entry.map(|e| e.path.as_str()).unwrap_or("");
    let args = entry
        .map(|e| split_template(&e.args).unwrap_or_default())
        .unwrap_or_default();

    let dev_hdd0 = ps3_vfs_dev_hdd0_path(path, &args, &ps3_library)
        .ok_or_else(|| no_dev_hdd0_message(title))?;
    Ok(Ps3Roots {
        dev_hdd0,
        games_root: ps3_vfs_games_path(path, &args, &ps3_library),
        data_root: entry.and_then(|e| rpcs3_data_root(&e.path)),
    })
}

// --- record + filesystem helpers --------------------------------------------

/// `archive`'s file stem, or an empty string when it has none. The stem is
/// what launch-file ranking scores a candidate's name against.
fn archive_stem(archive: &Path) -> String {
    archive
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `path`'s extension, lowercased, without the leading dot.
fn lowercase_suffix(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
}

/// Whether `root` holds at least one regular file, at any depth. Symlinked
/// directories are not descended into, so the walk can never leave the tree.
fn contains_regular_file(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            return true;
        }
        if file_type.is_dir() && contains_regular_file(&entry.path()) {
            return true;
        }
    }
    false
}

/// Every regular file under `root`, at any depth, in filesystem order.
/// Symlinked directories are not descended into.
fn regular_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_regular_files(root, &mut files);
    files
}

fn collect_regular_files(root: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            into.push(entry.path());
        } else if file_type.is_dir() {
            collect_regular_files(&entry.path(), into);
        }
    }
}

/// Moves `from` to `to`, falling back to copy-then-delete when the two are
/// on different filesystems (a rename cannot cross a mount point).
fn move_file(from: &Path, to: &Path) -> Result<(), LibraryError> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to)?;
            let _ = fs::remove_file(from);
            Ok(())
        }
    }
}

/// Appends `line` to `warning`, keeping any warning already there.
fn append_warning(warning: &mut String, line: &str) {
    if !warning.is_empty() {
        warning.push('\n');
    }
    warning.push_str(line);
}

/// Merges every entry of `src` into `dest`: directories are created (and
/// recursed into when they already exist), files overwrite their
/// counterparts, and anything in `dest` the tree does not carry is left
/// alone — the merge semantics of doc 03 / `_merge_tree`
/// (`archive_preparation.py:258-267`).
///
/// Entries are MOVED rather than copied. `src` is always a staging directory
/// inside `dest` that is deleted immediately afterwards, so a rename cannot
/// cross a filesystem boundary and the result is identical to a copy with
/// half the writes.
fn merge_tree_into(src: &Path, dest: &Path) -> Result<(), LibraryError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        // `file_type` does not follow symlinks, so a symlink is moved as
        // itself rather than being descended into.
        let from_is_dir = entry.file_type()?.is_dir();

        if from_is_dir && to.is_dir() {
            merge_tree_into(&from, &to)?;
            // Now empty; a failure to remove the husk is not worth failing
            // the install for, and the whole staging tree is deleted next.
            let _ = fs::remove_dir(&from);
            continue;
        }
        if to.is_dir() {
            remove_dir_tree(&to)?;
        } else if to.exists() {
            fs::remove_file(&to)?;
        }
        fs::rename(&from, &to)?;
    }
    Ok(())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A registry record carrying every metadata field from `detail`. The path
/// fields are left empty for the finalize branch to fill in.
fn new_record(detail: &RomDetail, rom_file_name: &str) -> InstalledGame {
    InstalledGame {
        title: detail.name.clone(),
        platform: detail.platform_name.clone(),
        rom_id: Some(detail.id),
        rom_file_name: rom_file_name.to_string(),
        description: detail.description.clone(),
        rating: detail.rating.clone(),
        genres: detail.genres.clone(),
        regions: detail.regions.clone(),
        languages: detail.languages.clone(),
        tags: detail.tags.clone(),
        revision: detail.revision.clone(),
        companies: detail.companies.clone(),
        first_release_date: detail.first_release_date.clone(),
        filesize_bytes: detail.filesize_bytes,
        server_updated_at: detail.server_updated_at.clone(),
        installed_at: unix_now(),
        cover_small_path: detail.cover_small_path.clone(),
        cover_large_path: detail.cover_large_path.clone(),
        screenshot_urls: detail.screenshot_urls.join("\n"),
        ..Default::default()
    }
}

fn is_appimage(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("appimage"))
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Deletes `path`, retrying every [`ARCHIVE_DELETE_PAUSE`] for up to
/// [`ARCHIVE_DELETE_ATTEMPTS`] attempts. Returns whether the file is gone.
/// Blocking — only ever called from the finalize task's blocking half.
fn delete_with_retry(path: &Path) -> bool {
    for attempt in 0..ARCHIVE_DELETE_ATTEMPTS {
        match fs::remove_file(path) {
            Ok(()) => return true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => {}
        }
        if attempt + 1 < ARCHIVE_DELETE_ATTEMPTS {
            std::thread::sleep(ARCHIVE_DELETE_PAUSE);
        }
    }
    false
}

/// Removes a directory tree. A read-only directory inside the tree makes the
/// first attempt fail with `PermissionDenied`; that case chmods everything in
/// the tree writable once and retries.
fn remove_dir_tree(dir: &Path) -> Result<(), LibraryError> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            make_tree_writable(dir);
            fs::remove_dir_all(dir).map_err(LibraryError::Io)
        }
        Err(e) => Err(LibraryError::Io(e)),
    }
}

/// Adds owner write (and, for directories, traverse) permission to every
/// entry under `dir`, `dir` itself included. Symlinks are skipped so the
/// walk can never change permissions outside the tree. Best effort: an entry
/// that cannot be changed is left as it is and the retry reports it.
#[cfg(unix)]
fn make_tree_writable(dir: &Path) {
    add_mode(dir, 0o700);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            make_tree_writable(&path);
        } else {
            add_mode(&path, 0o600);
        }
    }
}

#[cfg(unix)]
fn add_mode(path: &Path, extra: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mode = meta.permissions().mode();
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode | extra));
    }
}

#[cfg(not(unix))]
fn make_tree_writable(_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Credential;
    use secrecy::SecretString;

    fn client() -> Arc<RommClient> {
        Arc::new(
            RommClient::new(
                "http://localhost:1",
                Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
            )
            .unwrap(),
        )
    }

    fn rom_file(id: i64, file_name: &str, top_level: bool) -> RomFile {
        RomFile {
            id,
            file_name: file_name.to_string(),
            file_size_bytes: 10,
            is_top_level: top_level,
            category: String::new(),
        }
    }

    fn detail(files: Vec<RomFile>) -> RomDetail {
        RomDetail {
            id: 42,
            name: "Chrono Trigger".to_string(),
            platform_id: 7,
            platform_name: "SNES".to_string(),
            fs_name: "chrono.zip".to_string(),
            description: String::new(),
            regions: String::new(),
            languages: String::new(),
            tags: String::new(),
            revision: String::new(),
            rating: String::new(),
            genres: String::new(),
            companies: String::new(),
            first_release_date: String::new(),
            filesize_bytes: 999,
            server_updated_at: String::new(),
            files,
            cover_small_path: String::new(),
            cover_large_path: String::new(),
            screenshot_urls: Vec::new(),
        }
    }

    // --- encode_file_segment ------------------------------------------------

    #[test]
    fn encodes_everything_but_the_unreserved_set() {
        assert_eq!(encode_file_segment("game.zip"), "game.zip");
        assert_eq!(encode_file_segment("A-b_c.~d"), "A-b_c.~d");
        assert_eq!(encode_file_segment("my game.zip"), "my%20game.zip");
        assert_eq!(
            encode_file_segment("a/b?c#d&e.zip"),
            "a%2Fb%3Fc%23d%26e.zip"
        );
        assert_eq!(encode_file_segment("naïve.zip"), "na%C3%AFve.zip");
    }

    // --- plan_install --------------------------------------------------------

    #[test]
    fn plan_rejects_a_game_with_no_downloadable_file() {
        let detail = detail(vec![
            rom_file(1, "game.json", true),
            rom_file(2, "nested/rom.bin", true),
            rom_file(3, "inner.bin", false),
        ]);
        let Err(err) = plan_install(&detail, Path::new("/library"), client()) else {
            panic!("expected an error");
        };
        assert!(
            matches!(&err, LibraryError::Extract(msg) if msg == NO_DOWNLOADABLE_FILE),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn plan_single_file_targets_the_platform_dir_archive() {
        let detail = detail(vec![rom_file(11, "chrono.zip", true)]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();

        assert_eq!(job.rom_id, 42);
        assert!(job.multi_file_game_dir.is_none());
        assert_eq!(job.launch_entry, "chrono.zip");
        assert_eq!(
            job.primary_archive,
            PathBuf::from("/library/SNES/chrono.zip")
        );
        assert_eq!(job.targets.len(), 1);
        assert_eq!(job.targets[0].url_path, "/api/roms/42/content/chrono.zip");
        assert_eq!(
            job.targets[0].query,
            vec![("file_ids".to_string(), "11".to_string())]
        );
        assert_eq!(job.targets[0].dest, job.primary_archive);
        assert_eq!(job.targets[0].expected_size, 10);
    }

    #[test]
    fn plan_single_file_falls_back_to_the_game_size() {
        let mut detail = detail(vec![rom_file(11, "chrono.zip", true)]);
        detail.files[0].file_size_bytes = 0;
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();
        assert_eq!(job.targets[0].expected_size, 999);
    }

    #[test]
    fn plan_multi_file_uses_a_game_dir_and_prefers_the_m3u() {
        let detail = detail(vec![
            rom_file(21, "disc1.bin", true),
            rom_file(22, "Game.M3U", true),
            rom_file(23, "game.json", true),
        ]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();

        let game_dir = PathBuf::from("/library/SNES/Chrono Trigger");
        assert_eq!(job.multi_file_game_dir, Some(game_dir.clone()));
        assert_eq!(job.launch_entry, "Game.M3U");
        assert_eq!(job.primary_archive, game_dir.join("Game.M3U"));
        assert_eq!(job.targets.len(), 2);
        assert_eq!(job.targets[0].dest, game_dir.join("disc1.bin"));
        assert_eq!(job.targets[1].dest, game_dir.join("Game.M3U"));
        assert_eq!(job.targets[1].url_path, "/api/roms/42/content/Game.M3U");
    }

    #[test]
    fn plan_multi_file_without_an_m3u_launches_the_first_candidate() {
        let detail = detail(vec![
            rom_file(21, "disc1.bin", true),
            rom_file(22, "disc2.bin", true),
        ]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();
        assert_eq!(job.launch_entry, "disc1.bin");
    }

    // --- install modes -------------------------------------------------------

    #[test]
    fn install_mode_kinds_are_the_drawer_strings() {
        assert_eq!(InstallMode::Base.kind(), "base");
        assert_eq!(InstallMode::Update.kind(), "update");
        assert_eq!(InstallMode::Ps4Content.kind(), "ps4_content");
        assert_eq!(InstallMode::Xbox360Content.kind(), "xbox360_content");
        assert_eq!(InstallMode::NativeUpdate.kind(), "native_update");
    }

    // --- plan_install: native ------------------------------------------------

    fn native_detail(files: Vec<RomFile>) -> RomDetail {
        let mut detail = detail(files);
        detail.name = "My Game".to_string();
        detail.platform_name = "Windows".to_string();
        detail
    }

    #[test]
    fn plan_native_downloads_the_archive_into_the_game_home_dir() {
        let detail = native_detail(vec![
            rom_file(1, "artwork.png", true),
            rom_file(2, "mygame.zip", true),
        ]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();

        let game_dir = PathBuf::from("/library/Windows/My Game");
        assert_eq!(job.mode, InstallMode::Base);
        assert_eq!(job.native_game_dir, Some(game_dir.clone()));
        assert!(job.multi_file_game_dir.is_none());
        assert_eq!(job.game_json_target, None);
        assert_eq!(job.launch_entry, "mygame.zip");
        assert_eq!(job.primary_archive, game_dir.join("mygame.zip"));
        assert_eq!(job.file_ids, vec![2]);
        assert_eq!(job.targets.len(), 1);
        assert_eq!(
            job.targets[0].query,
            vec![("file_ids".to_string(), "2".to_string())]
        );
    }

    #[test]
    fn plan_native_adds_a_second_target_for_the_game_json_sidecar() {
        let detail = native_detail(vec![
            rom_file(1, "mygame.zip", true),
            rom_file(2, "game.json", true),
        ]);
        let job = plan_install(&detail, Path::new("/library"), client()).unwrap();

        let game_dir = PathBuf::from("/library/Windows/My Game");
        assert_eq!(job.game_json_target, Some(game_dir.join("game.json")));
        assert_eq!(job.file_ids, vec![1, 2]);
        assert_eq!(job.targets.len(), 2);
        assert_eq!(job.targets[1].dest, game_dir.join("game.json"));
        assert_eq!(
            job.targets[1].query,
            vec![("file_ids".to_string(), "2".to_string())]
        );
    }

    #[test]
    fn plan_native_rejects_a_payload_of_nothing_but_a_sidecar() {
        let detail = native_detail(vec![rom_file(1, "game.json", true)]);
        let Err(err) = plan_install(&detail, Path::new("/library"), client()) else {
            panic!("expected an error");
        };
        assert_eq!(
            err.to_string(),
            "No downloadable file was found for this game"
        );
    }

    // --- ps3_roots_from_config -----------------------------------------------

    fn config_with_library(library_path: &str) -> Config {
        Config {
            library_path: library_path.to_string(),
            ..Default::default()
        }
    }

    /// Points every environment-driven RPCS3 data-root candidate
    /// (`readers::rpcs3_data_root_candidates`) at an empty temp directory.
    ///
    /// With no emulator entry configured, the reader probes
    /// `$RPCS3_CONFIG_DIR` and `$XDG_CONFIG_HOME/rpcs3` for a `vfs.yml`
    /// BEFORE it reaches the `<library>/PlayStation 3/.vfs` fallback these
    /// tests are about. On a machine with a real RPCS3 the fallback would
    /// never be reached and the test would read the developer's own config.
    /// Callers must hold `test_env::lock()` for the guard's lifetime.
    fn empty_rpcs3_env(dir: &Path) -> crate::test_env::EnvGuard {
        let config_dir = dir.join("no-rpcs3-config");
        let xdg = dir.join("no-xdg-config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&xdg).unwrap();
        crate::test_env::EnvGuard::set(&[
            ("RPCS3_CONFIG_DIR", Some(&config_dir.to_string_lossy())),
            ("XDG_CONFIG_HOME", Some(&xdg.to_string_lossy())),
        ])
    }

    #[test]
    fn ps3_roots_falls_back_to_the_library_vfs_when_no_emulator_is_configured() {
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let _env = empty_rpcs3_env(dir.path());
        let library = dir.path().join("library");
        fs::create_dir_all(library.join("PlayStation 3")).unwrap();
        let config = config_with_library(&library.to_string_lossy());

        let roots = ps3_roots_from_config(&config, &[], "PlayStation 3", "Demon's Souls").unwrap();
        assert!(
            roots.dev_hdd0.ends_with("PlayStation 3/.vfs/dev_hdd0"),
            "unexpected dev_hdd0: {}",
            roots.dev_hdd0.display()
        );
        assert!(roots
            .games_root
            .as_ref()
            .is_some_and(|p| p.ends_with("PlayStation 3/.vfs/games")));
        assert_eq!(
            roots.data_root, None,
            "with no emulator entry there is nowhere to write games.yml"
        );
    }

    #[test]
    fn ps3_roots_without_a_library_or_an_emulator_reports_the_verbatim_message() {
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        let _env = empty_rpcs3_env(dir.path());
        let config = config_with_library("");
        let Err(message) = ps3_roots_from_config(&config, &[], "PlayStation 3", "Demon's Souls")
        else {
            panic!("expected an error");
        };
        assert_eq!(
            message,
            "No PS3 VFS dev_hdd0 path configured for Demon's Souls"
        );
    }

    // --- uninstall aggregation (D11) -----------------------------------------

    #[test]
    fn run_removals_continues_past_failures_and_reports_every_one() {
        let steps = vec![
            Removal {
                path: PathBuf::from("/x/game.iso"),
                label: RemovalLabel::File,
            },
            Removal {
                path: PathBuf::from("/x/trophy"),
                label: RemovalLabel::TrophyDir,
            },
            Removal {
                path: PathBuf::from("/x/dir"),
                label: RemovalLabel::Folder,
            },
        ];
        let mut attempted = Vec::new();
        let failures = run_removals(&steps, &mut |step| {
            attempted.push(step.path.clone());
            if step.label == RemovalLabel::Folder {
                Ok(())
            } else {
                Err("Permission denied".to_string())
            }
        });

        assert_eq!(
            attempted.len(),
            3,
            "a failed step must not stop the ones after it"
        );
        assert_eq!(
            failures,
            vec![
                "Could not remove file: /x/game.iso\nPermission denied".to_string(),
                "Could not remove PS3 trophy directory: /x/trophy\nPermission denied".to_string(),
            ]
        );
    }

    #[test]
    fn run_removals_reports_nothing_when_every_step_succeeds() {
        let steps = vec![Removal {
            path: PathBuf::from("/x/dir"),
            label: RemovalLabel::Folder,
        }];
        assert!(run_removals(&steps, &mut |_| Ok(())).is_empty());
    }

    #[test]
    fn uninstall_steps_for_native_stops_at_the_game_dir() {
        let dir = tempfile::tempdir().unwrap();
        let game_dir = dir.path().join("Windows/My Game");
        fs::create_dir_all(game_dir.join("game")).unwrap();
        let record = InstalledGame {
            title: "My Game".to_string(),
            platform: "Windows".to_string(),
            native_game_dir: game_dir.to_string_lossy().into_owned(),
            extracted_dir: game_dir.join("game").to_string_lossy().into_owned(),
            ..Default::default()
        };

        let steps = uninstall_steps(&record, dir.path());
        assert_eq!(
            steps,
            vec![Removal {
                path: game_dir,
                label: RemovalLabel::Folder,
            }]
        );
    }

    #[test]
    fn uninstall_steps_for_ps3_covers_iso_trophies_and_routed_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let iso = dir.path().join("game.iso");
        fs::write(&iso, b"iso").unwrap();
        let trophy = dir.path().join("trophy/NPWR12345");
        fs::create_dir_all(&trophy).unwrap();
        let routed = dir.path().join("dev_hdd0/game/BLUS30336");
        fs::create_dir_all(&routed).unwrap();

        let record = InstalledGame {
            title: "Demon's Souls".to_string(),
            platform: "PlayStation 3".to_string(),
            ps3_iso_path: iso.to_string_lossy().into_owned(),
            ps3_trophy_paths: serde_json::to_string(&vec![
                trophy.to_string_lossy().into_owned(),
                dir.path().join("missing").to_string_lossy().into_owned(),
            ])
            .unwrap(),
            extracted_dir: routed.to_string_lossy().into_owned(),
            ..Default::default()
        };

        let steps = uninstall_steps(&record, dir.path());
        assert_eq!(
            steps,
            vec![
                Removal {
                    path: iso,
                    label: RemovalLabel::File,
                },
                Removal {
                    path: trophy,
                    label: RemovalLabel::TrophyDir,
                },
                Removal {
                    path: routed,
                    label: RemovalLabel::Folder,
                },
            ],
            "a trophy path that is not a directory is skipped, not reported"
        );
    }

    #[test]
    fn parse_trophy_paths_is_lenient_about_junk() {
        assert!(parse_trophy_paths("").is_empty());
        assert!(parse_trophy_paths("   ").is_empty());
        assert!(parse_trophy_paths("not json").is_empty());
        assert!(parse_trophy_paths("{\"a\": 1}").is_empty());
        assert_eq!(
            parse_trophy_paths("[\"/a\", \"/b\"]"),
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );
    }

    // --- filesystem helpers --------------------------------------------------

    #[test]
    fn contains_regular_file_looks_all_the_way_down() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        assert!(!contains_regular_file(dir.path()));
        fs::write(dir.path().join("a/b/c/rom.bin"), b"x").unwrap();
        assert!(contains_regular_file(dir.path()));
    }

    #[test]
    fn regular_files_under_lists_every_file_once() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("CUSA12345/sce_sys")).unwrap();
        fs::write(dir.path().join("CUSA12345/eboot.bin"), b"x").unwrap();
        fs::write(dir.path().join("CUSA12345/sce_sys/param.sfo"), b"x").unwrap();
        let mut found = regular_files_under(dir.path());
        found.sort();
        assert_eq!(
            found,
            vec![
                dir.path().join("CUSA12345/eboot.bin"),
                dir.path().join("CUSA12345/sce_sys/param.sfo"),
            ]
        );
    }

    // --- helpers -------------------------------------------------------------

    #[test]
    fn appimage_detection_ignores_case() {
        assert!(is_appimage(Path::new("/x/Game.AppImage")));
        assert!(is_appimage(Path::new("/x/game.appimage")));
        assert!(!is_appimage(Path::new("/x/game.zip")));
    }

    #[test]
    fn safe_file_name_accepts_a_plain_component() {
        for name in [
            "Test Emu-v1.0.zip",
            "eden-linux.AppImage",
            ".hidden",
            "no-extension",
            "spaced out name.zip",
        ] {
            assert_eq!(safe_file_name(name, "asset").unwrap(), name);
        }
    }

    #[test]
    fn safe_file_name_rejects_anything_that_is_not_one_file_name() {
        for name in [
            "../../evil.AppImage",
            "../evil.zip",
            "a/b.zip",
            "/abs.zip",
            "dir/",
            "a\\b.zip",
            "..",
            ".",
            "",
        ] {
            let Err(err) = safe_file_name(name, "hostile-asset") else {
                panic!("expected {name:?} to be rejected");
            };
            assert_eq!(
                err.to_string(),
                "Refusing to install release asset 'hostile-asset': it does not name a plain file."
            );
        }
    }

    #[test]
    fn file_stem_of_drops_only_the_last_extension() {
        assert_eq!(file_stem_of("Test Emu-v1.0.zip"), "Test Emu-v1.0");
        assert_eq!(file_stem_of("no-extension"), "no-extension");
    }

    #[test]
    fn delete_with_retry_reports_success_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(delete_with_retry(&dir.path().join("nope.zip")));
    }

    #[test]
    fn delete_with_retry_removes_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archive.zip");
        fs::write(&path, b"bytes").unwrap();
        assert!(delete_with_retry(&path));
        assert!(!path.exists());
    }

    #[test]
    fn known_platforms_defaults_to_empty_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let registry = Arc::new(Registry::open(&dir.path().join("registry.db")).unwrap());
        let service =
            InstallService::with_profiles(registry, dir.path().join("config.toml"), Vec::new());

        assert!(
            service.known_platforms().is_empty(),
            "grid-core has no session of its own, so the list starts empty"
        );

        service.set_known_platforms(vec!["SNES".to_string(), "Sony PlayStation 2".to_string()]);
        assert_eq!(
            service.known_platforms(),
            vec!["SNES".to_string(), "Sony PlayStation 2".to_string()]
        );

        service.set_known_platforms(Vec::new());
        assert!(service.known_platforms().is_empty());
    }

    // --- content job planning -----------------------------------------------

    fn content_detail(platform: &str, files: Vec<RomFile>) -> RomDetail {
        RomDetail {
            platform_name: platform.to_string(),
            ..detail(files)
        }
    }

    fn content_file(id: i64, category: &str) -> RomFile {
        RomFile {
            category: category.to_string(),
            ..rom_file(id, &format!("file{id}.zip"), true)
        }
    }

    #[test]
    fn content_mode_follows_the_platform() {
        assert_eq!(content_mode("PlayStation 4"), Some(InstallMode::Ps4Content));
        assert_eq!(content_mode("ps4"), Some(InstallMode::Ps4Content));
        assert_eq!(
            content_mode("Microsoft Xbox 360"),
            Some(InstallMode::Xbox360Content)
        );
        assert_eq!(content_mode("SNES"), None);
        assert_eq!(content_mode(""), None);
    }

    #[test]
    fn no_content_message_words_each_console_differently() {
        assert_eq!(
            no_content_message(InstallMode::Ps4Content, ContentKind::Dlc),
            "No PS4 dlc files were found for this title in server metadata."
        );
        assert_eq!(
            no_content_message(InstallMode::Xbox360Content, ContentKind::Update),
            "No Xbox 360 update files were found for this title in server metadata."
        );
    }

    #[test]
    fn file_ids_csv_joins_in_order() {
        assert_eq!(file_ids_csv(&[3, 1, 2]), "3,1,2");
        assert_eq!(file_ids_csv(&[7]), "7");
        assert_eq!(file_ids_csv(&[]), "");
    }

    #[test]
    fn content_job_builds_one_target_with_one_file_ids_pair() {
        let detail = content_detail(
            "PlayStation 4",
            vec![content_file(1, "game"), content_file(2, "update")],
        );
        let library = Path::new("/lib");
        let (key, title, payload) = content_job(
            library,
            &detail,
            ContentKind::Update,
            InstallMode::Ps4Content,
            vec![2, 3],
            client(),
        );

        assert_eq!(key, JobKey::Content(42, ContentKind::Update));
        assert_eq!(title, "Chrono Trigger (update)");
        let JobPayload::Game(job) = payload else {
            panic!("a content job is a game job");
        };
        assert_eq!(job.mode, InstallMode::Ps4Content);
        assert_eq!(job.content_kind, Some(ContentKind::Update));
        assert_eq!(job.file_ids, vec![2, 3]);
        assert_eq!(
            job.targets.len(),
            1,
            "the server bundles the whole category"
        );
        assert_eq!(job.targets[0].url_path, "/api/roms/42/content/chrono.zip");
        assert_eq!(
            job.targets[0].query,
            vec![("file_ids".to_string(), "2,3".to_string())]
        );
        assert_eq!(
            job.targets[0].dest,
            Path::new("/lib/PlayStation 4/Chrono Trigger-update.zip")
        );
        assert_eq!(job.primary_archive, job.targets[0].dest);
        assert!(job.multi_file_game_dir.is_none());
        assert!(job.native_game_dir.is_none());
    }

    #[test]
    fn content_job_names_the_xbox_archive_by_kind() {
        let detail = content_detail("Xbox 360", vec![content_file(9, "dlc")]);
        let (key, title, payload) = content_job(
            Path::new("/lib"),
            &detail,
            ContentKind::Dlc,
            InstallMode::Xbox360Content,
            vec![9],
            client(),
        );

        assert_eq!(key, JobKey::Content(42, ContentKind::Dlc));
        assert_eq!(title, "Chrono Trigger (dlc)");
        let JobPayload::Game(job) = payload else {
            panic!("a content job is a game job");
        };
        assert_eq!(
            job.targets[0].dest,
            Path::new("/lib/Xbox 360/Chrono Trigger-dlc.zip")
        );
        assert_eq!(
            job.targets[0].expected_size, 0,
            "the bundle is built on the fly, so its size is unknown upfront"
        );
    }

    /// The file-name segment of a content URL is percent-encoded, so a
    /// hostile `fs_name` cannot add a path segment or a second query pair.
    #[test]
    fn content_job_encodes_the_file_name_segment() {
        let mut detail = content_detail("PlayStation 4", vec![content_file(1, "update")]);
        detail.fs_name = "a b/../c?x=1.zip".to_string();
        let (_, _, payload) = content_job(
            Path::new("/lib"),
            &detail,
            ContentKind::Update,
            InstallMode::Ps4Content,
            vec![1],
            client(),
        );
        let JobPayload::Game(job) = payload else {
            panic!("a content job is a game job");
        };
        assert_eq!(
            job.targets[0].url_path,
            "/api/roms/42/content/a%20b%2F..%2Fc%3Fx%3D1.zip"
        );
        assert_eq!(job.targets[0].query.len(), 1);
    }

    // --- xenia_content_root -------------------------------------------------

    /// Builds a service whose config file holds exactly `config_toml`.
    /// Resolves emulator profiles against the embedded catalog, because the
    /// Xenia profiles' Windows-only gating is what these tests exercise.
    fn service_with_config(dir: &Path, config_toml: &str) -> Arc<InstallService> {
        let config_path = dir.join("config.toml");
        fs::write(&config_path, config_toml).unwrap();
        let registry = Arc::new(Registry::open(&dir.join("registry.db")).unwrap());
        InstallService::new(registry, config_path)
    }

    #[cfg(unix)]
    #[test]
    fn xenia_content_root_without_an_emulator_reports_the_linux_message() {
        let dir = tempfile::tempdir().unwrap();
        let service = service_with_config(dir.path(), "schema_version = 1\n");

        let Err(message) = service.xenia_content_root("Xbox 360") else {
            panic!("no configured Xbox 360 emulator must not resolve a content root");
        };
        assert_eq!(
            message,
            "Xbox 360 content requires a Linux-compatible emulator such as Xenia Edge. \
             Install and configure Xenia Edge, then try again."
        );
    }

    /// A configured emulator whose catalog profile is Windows-only gets its
    /// own message: the content root could be resolved, but nothing on this
    /// host could ever read it.
    #[cfg(unix)]
    #[test]
    fn xenia_content_root_rejects_a_windows_only_emulator() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("xenia_canary.exe");
        fs::write(&executable, b"MZ").unwrap();
        // `portable.txt` would make the reader resolve a content root, which
        // this test must NOT reach: the host gate comes first.
        let service = service_with_config(
            dir.path(),
            &format!(
                "schema_version = 1\n\n\
                 [default_emulators]\n\"Xbox 360\" = \"Xenia Canary (Xbox 360)\"\n\n\
                 [[emulators]]\nname = \"Xenia Canary (Xbox 360)\"\npath = {:?}\nargs = \"\"\n",
                executable.to_string_lossy()
            ),
        );

        let Err(message) = service.xenia_content_root("Xbox 360") else {
            panic!("a Windows-only emulator must not resolve a content root");
        };
        assert_eq!(
            message,
            "The configured Xbox 360 emulator only runs on Windows. Install a \
             Linux-compatible emulator such as Xenia Edge to apply content."
        );
    }

    /// The only reachable route to the "could not determine" message: the
    /// config file itself will not parse, so there is no emulator list to
    /// resolve anything from. `xenia_directory_settings` cannot produce a
    /// blank `content_root` — every branch of its storage root is non-empty
    /// and the override path defaults to the literal `"content"` — so a
    /// blank-root test would assert on an unreachable state.
    #[test]
    fn xenia_content_root_reports_the_unknown_message_when_the_config_will_not_load() {
        let dir = tempfile::tempdir().unwrap();
        let service = service_with_config(dir.path(), "this is not = = valid toml [[[");

        let Err(message) = service.xenia_content_root("Xbox 360") else {
            panic!("an unreadable config must not resolve a content root");
        };
        assert_eq!(
            message,
            "Could not determine Xenia content directory. Is Xenia configured?"
        );
    }
}
