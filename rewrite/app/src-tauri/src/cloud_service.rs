//! App-layer cloud save wiring: `CloudService` (the emulator-entry/sync-dir
//! caches plus the D5 auto-upload pool), the DTOs the Tauri commands in
//! `commands/cloud.rs` return, and the two auto-sync triggers
//! (`grid_launcher/ui/mixins/cloud_mixin.py:2800-2970`):
//!
//! - **before launch** — [`CloudService::auto_restore_before_launch`],
//!   called from `commands::launch_game` before the process spawns
//!   (`details_view_mixin.py:1497`'s call site; `_auto_sync_before_launch`
//!   at cloud_mixin.py:2799-2814);
//! - **after exit** — [`CloudService::install_session_finished_hook`]
//!   installs a `LaunchService::set_session_finished_hook` that stamps the
//!   session-start/end sync state and, gated on the config flag and a live
//!   connection, schedules an auto-upload after the (clamped) delay
//!   (`_register_game_session_for_auto_upload` at
//!   cloud_mixin.py:2818-2842, `_handle_finished_game_session` /
//!   `_auto_upload_after_session` at :2851-2925, the
//!   `AutoCloudSaveUploadWorker` in `background/workers.py:650`, and
//!   `_on_auto_cloud_upload_finished` at cloud_mixin.py:2941).
//!
//! grid-core never imports Tauri — every `RommClient`/`Config`/registry
//! read here happens in THIS crate, on the blocking pool for anything that
//! touches disk or sqlite (matching `commands.rs`'s existing convention).
//!
//! Token secrecy: nothing here ever formats a `RommClient`/`SessionError`
//! error into a *stored* value — only into `tracing::debug!` text, and
//! every error type crossing this module already carries the crate-wide
//! guarantee that its `Display` is credential-free (see `commands::err`'s
//! doc comment). No header, token, or secret is ever logged.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use grid_core::cloud::native::normalize_manual_save_path;
use grid_core::cloud::ops::native::manual_paths_key;
use grid_core::cloud::ops::{self, CloudCaches, CloudContext, CloudMessage};
use grid_core::cloud::scope::SaveScope;
use grid_core::cloud::state::{
    apply_sync_update, auto_cloud_upload_plan, game_key, games_match_identity,
    summarize_auto_cloud_upload_result, sync_entry_for, PerTypeResult, SyncStateUpdate,
};
use grid_core::cloud::transfer::MessageSeverity;
use grid_core::cloud::{restore as cloud_restore, CloudGame, SaveType};
use grid_core::config::Config;
use grid_core::launch::profiles::{load_profiles, EmulatorProfile};
use grid_core::launch::{GameSession, LaunchService};
use grid_core::library::registry::InstalledGame;
use grid_core::library::InstallService;
use grid_core::romm::RommClient;
use grid_core::session::SessionManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

/// D5: at most this many auto-uploads run at once, across every game.
const MAX_CONCURRENT_AUTO_UPLOADS: usize = 2;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn unix_now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------------
// D5: the auto-upload pool
// ---------------------------------------------------------------------

/// A small tokio task pool for auto-uploads, keyed by `game_key`
/// (spec D5): a trigger for a game with an upload already in flight is
/// coalesced (dropped — the running upload's plan already covers the
/// newest mtimes, or the next session re-triggers), and GLOBAL
/// concurrency across every game is capped by a semaphore.
///
/// The in-flight set and the semaphore are two independent mechanisms:
/// the set is what makes a SECOND trigger for the same game a no-op while
/// the first is still running (or queued); the semaphore is what stops
/// MORE than [`MAX_CONCURRENT_AUTO_UPLOADS`] uploads from actually
/// executing at once, even across different games.
pub struct AutoUploadPool {
    inflight: Arc<StdMutex<HashSet<String>>>,
    semaphore: Arc<Semaphore>,
}

impl AutoUploadPool {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            inflight: Arc::new(StdMutex::new(HashSet::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Attempts to trigger `task` for `key`. Returns `false` (and never
    /// spawns `task`) when a trigger for `key` is already in flight;
    /// otherwise spawns it on tokio, gated by the pool's semaphore, and
    /// returns `true`. `key` leaves the in-flight set the moment `task`
    /// finishes OR panics (fix round 1, FIX 3: a [`RemoveOnDrop`] guard,
    /// not a plain post-await removal, so a panicking `task` can never
    /// leak its key forever), so a later trigger for the same game runs
    /// again either way.
    pub fn trigger<F, Fut>(&self, key: String, task: F) -> bool
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        {
            let mut inflight = lock_tolerant(&self.inflight);
            if !inflight.insert(key.clone()) {
                return false;
            }
        }
        let inflight = self.inflight.clone();
        let semaphore = self.semaphore.clone();
        tokio::spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("the pool's semaphore is never closed");
            // Removed on drop — including an unwind from a panicking
            // `task`, not only the ordinary post-await path — so a
            // panic can never leave `key` permanently un-retriggerable.
            let _guard = RemoveOnDrop { inflight, key };
            task().await;
        });
        true
    }
}

/// Removes `key` from `inflight` when dropped, panic-unwind included.
/// Fix round 1, FIX 3.
struct RemoveOnDrop {
    inflight: Arc<StdMutex<HashSet<String>>>,
    key: String,
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        lock_tolerant(&self.inflight).remove(&self.key);
    }
}

/// `Mutex::lock` that tolerates poisoning: a panic while `task` (or any
/// other holder) held the lock must not permanently break every later
/// trigger — fix round 1, FIX 3. The set this guards is plain data with
/// no invariant a partial mutation could violate, so recovering the
/// (possibly stale, never corrupt) inner value is safe.
fn lock_tolerant<T>(mutex: &StdMutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------
// CloudService
// ---------------------------------------------------------------------

/// Everything a Tauri cloud command or auto-trigger needs beyond the
/// `RommClient`: the memoized emulator-entry/sync-directory caches
/// [`CloudCaches`] owns (behind a `tokio` mutex so an `ops` call's
/// internal `.await`s can hold it without blocking a whole OS thread —
/// see [`Self::caches`]), the D5 [`AutoUploadPool`], and Task 18's
/// PCGamingWiki client + per-title path cache (see [`Self::native_save_paths`]
/// / [`Self::cached_pcgw_paths`]).
pub struct CloudService {
    caches: AsyncMutex<CloudCaches>,
    pool: AutoUploadPool,
    /// Plain `reqwest::Client` (never `RommClient` — different host, and
    /// the RomM token must never reach PCGamingWiki); built once by
    /// `grid_core::pcgw::build_http_client`.
    pcgw_client: reqwest::Client,
    /// `title.trim() -> Vec<String>` for the process lifetime, matching
    /// Python's `_pcgw_paths_cache` (`details_view_mixin.py`'s
    /// `_pcgw_cache_key`, `:152-153`).
    pcgw_cache: StdMutex<HashMap<String, Vec<String>>>,
}

impl CloudService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            caches: AsyncMutex::new(CloudCaches::default()),
            pool: AutoUploadPool::new(MAX_CONCURRENT_AUTO_UPLOADS),
            pcgw_client: grid_core::pcgw::build_http_client(),
            pcgw_cache: StdMutex::new(HashMap::new()),
        })
    }

    // -- PCGamingWiki save-location cache (Task 18) ----------------------

    /// `_pcgw_cache_key` + `_pcgw_paths_for_game` + `_start_pcgw_lookup_for_game`
    /// (`details_view_mixin.py:152-182`): the ONLY place that ever triggers
    /// a PCGamingWiki network fetch. A cache hit returns immediately; a
    /// miss fetches, caches the result (success OR failure — a failure
    /// caches an empty list, matching `_on_pcgw_paths_loaded` storing
    /// `bundle.get("paths", [])`, which is `[]` on the worker's caught
    /// exception path), and returns it. Called only from
    /// [`Self::native_save_paths`] — the command behind the native-save
    /// panel, matching `_refresh_native_save_panel`'s the only call site of
    /// `_start_pcgw_lookup_for_game` (`:1166`).
    async fn pcgw_paths_for_title(&self, title: &str) -> Vec<String> {
        let key = title.trim().to_string();
        if let Some(cached) = self.pcgw_cache.lock().unwrap().get(&key).cloned() {
            return cached;
        }
        let fetched = grid_core::pcgw::fetch_windows_save_paths(&self.pcgw_client, &key)
            .await
            .unwrap_or_default();
        self.pcgw_cache.lock().unwrap().insert(key, fetched.clone());
        fetched
    }

    /// `_pcgw_paths_for_game(game) or []` (`cloud_mixin.py:2136`, `:2687`):
    /// a CACHE-ONLY read — never triggers a fetch. Every `ops` call site
    /// that builds a `CloudContext` (panel info, records, upload, restore,
    /// the auto-restore/auto-upload triggers) reads through here, exactly
    /// like Python's upload/restore helpers read `self._pcgw_paths_cache`
    /// directly rather than kicking off their own worker.
    fn cached_pcgw_paths(&self, title: &str) -> Vec<String> {
        self.pcgw_cache
            .lock()
            .unwrap()
            .get(title.trim())
            .cloned()
            .unwrap_or_default()
    }

    // -- context building -------------------------------------------------

    async fn load_inputs(
        config_path: &Path,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
    ) -> Result<Inputs, String> {
        let config_path = config_path.to_path_buf();
        let config = blocking_load_config(config_path.clone()).await?;
        let installed = blocking_installed(install).await?;
        let all_games: Vec<CloudGame> = installed.iter().map(cloud_game_from_installed).collect();
        let sessions = launch.snapshot().sessions;
        let active_sessions = active_session_refs(&sessions, &installed);
        let config_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Ok(Inputs {
            config,
            profiles: load_profiles(),
            all_games,
            installed,
            config_dir,
            active_sessions,
            now: unix_now_f64(),
        })
    }

    // -- panel / records / upload / restore / delete -----------------------

    pub async fn panel_info(
        &self,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
        config_path: &Path,
        game: CloudGameInput,
        save_type: SaveType,
    ) -> Result<CloudPanelInfoDto, String> {
        let inputs = Self::load_inputs(config_path, install, launch).await?;
        let cloud_game = cloud_game_from_input(&game);
        let installed = inputs
            .all_games
            .iter()
            .any(|g| games_match_identity(g, &cloud_game));
        let pcgw_paths = self.cached_pcgw_paths(&cloud_game.title);
        let ctx = inputs.context(&pcgw_paths);
        let mut caches = self.caches.lock().await;
        let entry = ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &cloud_game, save_type);
        let supported =
            ops::details_cloud_mode_supported(&ctx, &mut caches, &cloud_game, save_type, installed);
        let block_reason = ops::block_reason_for_game(&ctx, &cloud_game, save_type, entry.as_ref());
        let scope = ops::scope_for_game(&ctx, &cloud_game, save_type, entry.as_ref());
        Ok(CloudPanelInfoDto {
            supported,
            block_reason,
            scope: SaveScopeDto::from(scope),
        })
    }

    pub async fn records(
        &self,
        session: &SessionManager,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
        config_path: &Path,
        game: CloudGameInput,
        save_type: SaveType,
    ) -> Result<Vec<CloudRecordDto>, String> {
        let client = session.client().ok_or("not connected")?;
        let inputs = Self::load_inputs(config_path, install, launch).await?;
        let cloud_game = cloud_game_from_input(&game);
        let pcgw_paths = self.cached_pcgw_paths(&cloud_game.title);
        let ctx = inputs.context(&pcgw_paths);
        let mut caches = self.caches.lock().await;
        let records =
            ops::fetch_cloud_records(&client, &ctx, &mut caches, &cloud_game, save_type).await?;

        // Fix round 1, FIX 4: `restore_enabled_for_record` is pure but not
        // cheap (it can walk `resolved_sync_dirs`), and many records in
        // one list typically share the same emulator — memoize per THIS
        // request, keyed like Python's own per-request cache
        // (`(save_type, game_key, emulator name lowercased)`,
        // `details_view_mixin.py:637-641`), rather than recomputing per
        // row.
        let key_prefix = format!("{}::{}", save_type.as_str(), game_key(&cloud_game));
        let mut restore_enabled_memo: HashMap<String, (bool, String)> = HashMap::new();
        let mut dtos = Vec::with_capacity(records.len());
        for record in &records {
            let emulator_field = record
                .get("emulator")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let memo_key = format!("{key_prefix}::{emulator_field}");
            let (restorable, tooltip) = match restore_enabled_memo.get(&memo_key) {
                Some(cached) => cached.clone(),
                None => {
                    let computed = ops::restore_enabled_for_record(
                        &ctx,
                        &mut caches,
                        &cloud_game,
                        save_type,
                        record,
                    );
                    restore_enabled_memo.insert(memo_key, computed.clone());
                    computed
                }
            };
            dtos.push(record_dto(record, inputs.now, restorable, tooltip));
        }
        Ok(dtos)
    }

    pub async fn upload(
        &self,
        session: &SessionManager,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
        config_path: &Path,
        game: CloudGameInput,
        save_type: SaveType,
    ) -> Result<UploadReportDto, String> {
        let client = session.client().ok_or("not connected")?;
        let inputs = Self::load_inputs(config_path, install, launch).await?;
        let cloud_game = cloud_game_from_input(&game);
        // `_perform_upload_saves_action` / `_perform_upload_states_action`
        // (cloud_mixin.py:2780-2797, doc 06 "Manual actions"): a no-op —
        // no dialog, no upload attempt — when the game isn't installed.
        if !inputs
            .all_games
            .iter()
            .any(|g| games_match_identity(g, &cloud_game))
        {
            return Ok(UploadReportDto {
                uploaded: 0,
                total: 0,
                failed: Vec::new(),
                messages: Vec::new(),
            });
        }
        let pcgw_paths = self.cached_pcgw_paths(&cloud_game.title);
        let ctx = inputs.context(&pcgw_paths);
        let mut caches = self.caches.lock().await;
        let report = ops::upload::upload_cloud_files_for_game(
            &client,
            &ctx,
            &mut caches,
            &cloud_game,
            save_type,
        )
        .await;
        Ok(UploadReportDto::from(report))
    }

    /// `record_id: None` restores the latest server record(s), matching
    /// the auto-sync path; `Some(id)` restores exactly that record — the
    /// manual "Restore" action on one row of the `cloud_records` list
    /// (`_confirm_restore_details_cloud_record`,
    /// `details_view_mixin.py:1237-1263`; `skip_if_local_newer` and
    /// `skip_if_known_latest` are both `false` for a manual restore,
    /// cloud_mixin.py:1906-1908's defaults).
    #[allow(clippy::too_many_arguments)]
    pub async fn restore(
        &self,
        session: &SessionManager,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
        config_path: &Path,
        game: CloudGameInput,
        save_type: SaveType,
        record_id: Option<i64>,
    ) -> Result<RestoreReportDto, String> {
        let client = session.client().ok_or("not connected")?;
        let inputs = Self::load_inputs(config_path, install, launch).await?;
        let cloud_game = cloud_game_from_input(&game);
        let pcgw_paths = self.cached_pcgw_paths(&cloud_game.title);
        let ctx = inputs.context(&pcgw_paths);
        let mut caches = self.caches.lock().await;

        let selected: Option<Value> = match record_id {
            None => None,
            Some(id) => {
                let all =
                    ops::fetch_cloud_records(&client, &ctx, &mut caches, &cloud_game, save_type)
                        .await?;
                let found = all.into_iter().find(|record| record_id_matches(record, id));
                if found.is_none() {
                    return Err(format!("Record {id} not found on the server."));
                }
                found
            }
        };

        let (ok, messages, update) = match save_type {
            SaveType::Save => {
                ops::restore::restore_cloud_save_for_game(
                    &client,
                    &ctx,
                    &mut caches,
                    &cloud_game,
                    selected.as_ref(),
                    false,
                    false,
                )
                .await
            }
            SaveType::State => {
                ops::restore::restore_cloud_state_for_game(
                    &client,
                    &ctx,
                    &mut caches,
                    &cloud_game,
                    selected.as_ref(),
                    false,
                )
                .await
            }
        };

        // Fix round 1, FIX 1: reload immediately before saving — `inputs`
        // was captured before the fetch/restore network calls above, so
        // saving from it would clobber a concurrent write.
        let key = game_key(&cloud_game);
        if !key.is_empty() && update != SyncStateUpdate::default() {
            reload_apply_and_save(config_path, &key, update).await?;
            caches.clear();
        }

        Ok(RestoreReportDto {
            ok,
            messages: messages.into_iter().map(CloudMessageDto::from).collect(),
        })
    }

    pub async fn delete(
        &self,
        session: &SessionManager,
        save_type: SaveType,
        record_id: i64,
    ) -> Result<(), String> {
        let client = session.client().ok_or("not connected")?;
        ops::delete_cloud_record(&client, save_type, record_id).await
    }

    // -- native save paths --------------------------------------------------

    pub async fn native_save_paths(
        &self,
        config_path: &Path,
        game: CloudGameInput,
    ) -> Result<NativeSavePathsDto, String> {
        let config = blocking_load_config(config_path.to_path_buf()).await?;
        let cloud_game = cloud_game_from_input(&game);
        let key = manual_paths_key(&cloud_game);
        let manual = config
            .native_manual_save_paths
            .get(&key)
            .cloned()
            .unwrap_or_default();
        // The only call site that ever fetches: see `pcgw_paths_for_title`'s
        // doc comment. A fetch failure degrades to an empty list here (via
        // that method's own `unwrap_or_default`) — never an error, so the
        // panel still allows manual paths.
        let pcgw = self.pcgw_paths_for_title(&cloud_game.title).await;
        Ok(NativeSavePathsDto { pcgw, manual })
    }

    pub async fn native_add_manual_save_path(
        &self,
        config_path: &Path,
        game: CloudGameInput,
        path: String,
    ) -> Result<(), String> {
        let normalized = normalize_manual_save_path(Path::new(&path));
        self.mutate_manual_paths(config_path, game, move |paths| {
            if !paths.iter().any(|p| p == &normalized) {
                paths.push(normalized.clone());
            }
        })
        .await
    }

    pub async fn native_remove_manual_save_path(
        &self,
        config_path: &Path,
        game: CloudGameInput,
        path: String,
    ) -> Result<(), String> {
        self.mutate_manual_paths(config_path, game, move |paths| {
            paths.retain(|p| p != &path);
        })
        .await
    }

    async fn mutate_manual_paths(
        &self,
        config_path: &Path,
        game: CloudGameInput,
        mutate: impl FnOnce(&mut Vec<String>) + Send + 'static,
    ) -> Result<(), String> {
        let config_path = config_path.to_path_buf();
        let cloud_game = cloud_game_from_input(&game);
        let key = manual_paths_key(&cloud_game);
        let mut config = blocking_load_config(config_path.clone()).await?;
        let mut paths = config
            .native_manual_save_paths
            .get(&key)
            .cloned()
            .unwrap_or_default();
        mutate(&mut paths);
        config.native_manual_save_paths.insert(key, paths);
        blocking_save_config(config_path, config).await?;
        self.caches.lock().await.clear();
        Ok(())
    }

    // -- settings -------------------------------------------------------

    pub async fn settings(&self, config_path: &Path) -> Result<CloudSettingsDto, String> {
        let config = blocking_load_config(config_path.to_path_buf()).await?;
        Ok(CloudSettingsDto {
            download_on_launch: config.auto_cloud_save_download_on_launch,
            upload_on_exit: config.auto_cloud_save_upload_on_exit,
            skip_if_local_newer: config.auto_cloud_save_skip_download_if_local_newer,
            // The clamp lives HERE and nowhere else (task ruling): every
            // other reader of this field goes through
            // `clamped_upload_delay_seconds`, never the raw config value.
            upload_delay_seconds: clamped_upload_delay_seconds(&config),
            retention_limit: config.cloud_save_retention_limit,
        })
    }

    /// Persists `settings` verbatim — no clamping here (task ruling:
    /// clamping happens only where the delay is actually consumed/read,
    /// not on write); `cloud_save_retention_limit`'s minimum-1 floor is
    /// applied inside `ops` at upload time, not duplicated here either.
    pub async fn set_settings(
        &self,
        config_path: &Path,
        settings: CloudSettingsDto,
    ) -> Result<(), String> {
        let config_path = config_path.to_path_buf();
        let mut config = blocking_load_config(config_path.clone()).await?;
        config.auto_cloud_save_download_on_launch = settings.download_on_launch;
        config.auto_cloud_save_upload_on_exit = settings.upload_on_exit;
        config.auto_cloud_save_skip_download_if_local_newer = settings.skip_if_local_newer;
        config.auto_cloud_save_upload_delay_seconds = settings.upload_delay_seconds;
        config.cloud_save_retention_limit = settings.retention_limit;
        blocking_save_config(config_path, config).await?;
        self.caches.lock().await.clear();
        Ok(())
    }

    // -- auto trigger: before launch -----------------------------------

    /// `_auto_sync_before_launch` (cloud_mixin.py:2799-2814), called from
    /// `commands::launch_game` BEFORE the process spawns
    /// (`details_view_mixin.py:1497`). Every failure is logged at debug
    /// only and never blocks the launch — there is no `Result` to
    /// propagate on purpose.
    pub async fn auto_restore_before_launch(
        &self,
        session: &SessionManager,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
        config_path: &Path,
        installed_game: &InstalledGame,
    ) {
        let Some(client) = session.client() else {
            return;
        };
        let inputs = match Self::load_inputs(config_path, install, launch).await {
            Ok(inputs) => inputs,
            Err(e) => {
                tracing::debug!("cloud auto-restore: failed to load context: {e}");
                return;
            }
        };
        if !inputs.config.auto_cloud_save_download_on_launch {
            return;
        }

        let cloud_game = cloud_game_from_installed(installed_game);
        let skip_if_local_newer = inputs.config.auto_cloud_save_skip_download_if_local_newer;
        let pcgw_paths = self.cached_pcgw_paths(&cloud_game.title);
        let ctx = inputs.context(&pcgw_paths);
        let mut caches = self.caches.lock().await;
        let mut combined = SyncStateUpdate::default();

        let save_entry =
            ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &cloud_game, SaveType::Save);
        if ops::block_reason_for_game(&ctx, &cloud_game, SaveType::Save, save_entry.as_ref())
            .is_empty()
        {
            let (_ok, messages, update) = ops::restore::restore_cloud_save_for_game(
                &client,
                &ctx,
                &mut caches,
                &cloud_game,
                None,
                skip_if_local_newer,
                true,
            )
            .await;
            log_cloud_messages("auto-restore save", &messages);
            merge_update(&mut combined, update);
        }

        let state_entry =
            ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &cloud_game, SaveType::State);
        if ops::block_reason_for_game(&ctx, &cloud_game, SaveType::State, state_entry.as_ref())
            .is_empty()
        {
            let (_ok, messages, update) = ops::restore::restore_cloud_state_for_game(
                &client,
                &ctx,
                &mut caches,
                &cloud_game,
                None,
                true,
            )
            .await;
            log_cloud_messages("auto-restore state", &messages);
            merge_update(&mut combined, update);
        }

        // Fix round 1, FIX 1: reload immediately before saving — the two
        // restore calls above are network round trips; `inputs.config`
        // was captured before them.
        let key = game_key(&cloud_game);
        if !key.is_empty() && combined != SyncStateUpdate::default() {
            if let Err(e) = reload_apply_and_save(config_path, &key, combined).await {
                tracing::debug!("cloud auto-restore: config save failed: {e}");
            } else {
                caches.clear();
            }
        }
    }

    // -- auto trigger: session registration at spawn -----------------------

    /// `_register_game_session_for_auto_upload`'s sync-state half
    /// (cloud_mixin.py:2818-2842): stamps `last_session_started_at`/
    /// `last_session_ended_at = 0.0` when at least one of the save/state
    /// block reasons is empty. Called right after `LaunchService::launch`
    /// returns a session (the session list itself is grid-core's job;
    /// this only adds the cloud sync-state parity Python's mixin does at
    /// the same call site).
    pub async fn stamp_session_started(
        &self,
        install: Arc<InstallService>,
        launch: Arc<LaunchService>,
        config_path: &Path,
        installed_game: &InstalledGame,
        started_at: f64,
    ) {
        let inputs = match Self::load_inputs(config_path, install, launch).await {
            Ok(inputs) => inputs,
            Err(e) => {
                tracing::debug!("cloud session registration: failed to load context: {e}");
                return;
            }
        };
        let cloud_game = cloud_game_from_installed(installed_game);
        let pcgw_paths = self.cached_pcgw_paths(&cloud_game.title);
        let ctx = inputs.context(&pcgw_paths);
        let mut caches = self.caches.lock().await;
        let save_entry =
            ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &cloud_game, SaveType::Save);
        let save_reason =
            ops::block_reason_for_game(&ctx, &cloud_game, SaveType::Save, save_entry.as_ref());
        let state_entry =
            ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &cloud_game, SaveType::State);
        let state_reason =
            ops::block_reason_for_game(&ctx, &cloud_game, SaveType::State, state_entry.as_ref());
        if !save_reason.is_empty() && !state_reason.is_empty() {
            return;
        }

        let key = game_key(&cloud_game);
        if key.is_empty() {
            return;
        }
        // Fix round 1, FIX 1: same reload-before-save discipline as every
        // other sync-state persist here, for consistency (this call site
        // has no network gap today, but nothing stops one being added
        // later without anyone noticing the staleness risk it would
        // reintroduce).
        if let Err(e) = reload_apply_and_save(
            config_path,
            &key,
            SyncStateUpdate {
                last_session_started_at: Some(started_at),
                last_session_ended_at: Some(0.0),
                ..Default::default()
            },
        )
        .await
        {
            tracing::debug!("cloud session registration: config save failed: {e}");
        } else {
            caches.clear();
        }
    }

    // -- auto trigger: after exit -----------------------------------------

    /// Installs the cloud auto-upload trigger on `launch`. Called once
    /// from `lib.rs`'s `.setup()`. The hook itself must be a plain (not
    /// `async`) closure — `LaunchService::set_session_finished_hook`'s
    /// contract — so it hands the actual work to
    /// `tauri::async_runtime::spawn`, exactly like `spawn_poll_loop`'s own
    /// call site requires for any code that reaches `tokio::spawn`.
    pub fn install_session_finished_hook(
        self: &Arc<Self>,
        launch: &Arc<LaunchService>,
        session_mgr: Arc<SessionManager>,
        install: Arc<InstallService>,
        config_path: PathBuf,
    ) {
        let cloud = self.clone();
        launch.set_session_finished_hook(Arc::new(move |session: GameSession| {
            let cloud = cloud.clone();
            let session_mgr = session_mgr.clone();
            let install = install.clone();
            let config_path = config_path.clone();
            tauri::async_runtime::spawn(async move {
                cloud
                    .handle_session_finished(session, session_mgr, install, config_path)
                    .await;
            });
        }));
    }

    /// `_handle_finished_game_session` + `_auto_upload_after_session`
    /// (cloud_mixin.py:2851-2925): stamp the session end, then — gated on
    /// the config flag and a live connection — sleep the (clamped) delay
    /// and hand the actual upload to the D5 pool, keyed by `game_key` so a
    /// second exit for the same game while the first upload is still
    /// running is coalesced.
    async fn handle_session_finished(
        self: Arc<Self>,
        session: GameSession,
        session_mgr: Arc<SessionManager>,
        install: Arc<InstallService>,
        config_path: PathBuf,
    ) {
        let ended_at = unix_now_f64();
        let installed = match blocking_installed(install).await {
            Ok(installed) => installed,
            Err(e) => {
                tracing::debug!("cloud session-finished: registry lookup failed: {e}");
                return;
            }
        };
        let Some(installed_game) = installed.iter().find(|g| g.rom_id == Some(session.rom_id))
        else {
            return;
        };
        let cloud_game = cloud_game_from_installed(installed_game);
        let key = game_key(&cloud_game);

        // `session.started_at` is a unix-seconds `i64`; only re-stamped
        // when positive (parity: `session_cloud_sync_updates`,
        // cloud_sync.py:139-151).
        if session.started_at > 0 && !key.is_empty() {
            if let Err(e) = reload_apply_and_save(
                &config_path,
                &key,
                SyncStateUpdate {
                    last_session_started_at: Some(session.started_at as f64),
                    last_session_ended_at: Some(ended_at),
                    ..Default::default()
                },
            )
            .await
            {
                tracing::debug!("cloud session-finished: config save failed: {e}");
            } else {
                self.caches.lock().await.clear();
            }
        }

        if key.is_empty() {
            return;
        }
        let Some(client) = session_mgr.client() else {
            return;
        };
        let Ok(config) = blocking_load_config(config_path.clone()).await else {
            return;
        };
        if !config.auto_cloud_save_upload_on_exit {
            return;
        }

        let delay = clamped_upload_delay_seconds(&config);
        if delay > 0 {
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }

        let all_games: Vec<CloudGame> = installed.iter().map(cloud_game_from_installed).collect();
        let config_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();

        let cloud_for_task = self.clone();
        let key_for_trigger = key.clone();
        self.pool.trigger(key_for_trigger, move || async move {
            cloud_for_task
                .run_auto_upload(
                    client,
                    cloud_game,
                    all_games,
                    config,
                    config_dir,
                    config_path,
                    key,
                )
                .await;
        });
    }

    /// `AutoCloudSaveUploadWorker.run` + `_on_auto_cloud_upload_finished`
    /// (`background/workers.py:650-711`, `cloud_mixin.py:2941-2970`): plan
    /// which save types have moved since the last upload, run each
    /// through `ops::upload_cloud_files_for_game`, then persist the
    /// bookkeeping in ONE config save (see the note in
    /// `auto_restore_before_launch` for why this port merges rather than
    /// writing once per type as Python's two separate restore calls do —
    /// same reasoning, one fewer stale-base race).
    ///
    /// Fix round 1, FIX 2: uses its OWN local [`CloudCaches`] rather than
    /// `self.caches` — an auto-upload's resolution + network POST(s) can
    /// run for a while, and holding the app-wide cache mutex for that
    /// whole span would (a) serialize the D5 pool's cap-2 concurrency
    /// down to effectively 1 (two auto-uploads would queue behind the
    /// SAME lock even though the pool's semaphore allows both to run),
    /// and (b) make a manual "Play"/panel action queue behind an
    /// in-flight auto-upload for an unrelated game. A fresh cache per run
    /// is cheap and correct — parity note: Python's own caches are
    /// cleared on every config save anyway (`CloudCaches::clear`'s doc
    /// comment), so per-run freshness costs nothing this port didn't
    /// already accept elsewhere.
    #[allow(clippy::too_many_arguments)]
    async fn run_auto_upload(
        self: Arc<Self>,
        client: Arc<RommClient>,
        game: CloudGame,
        all_games: Vec<CloudGame>,
        config: Config,
        config_dir: PathBuf,
        config_path: PathBuf,
        key: String,
    ) {
        let now = unix_now_f64();
        let pcgw_paths = self.cached_pcgw_paths(&game.title);
        let ctx = CloudContext {
            config: &config,
            profiles: load_profiles(),
            all_games: &all_games,
            resolve_ctx: grid_core::cloud::dirs::ResolveContext {
                emulator_dir: None,
                library_dir: &config.library_path,
                config_dir: &config_dir,
                windows_documents: None,
            },
            // This game's own session already ended; no live session
            // window applies to its own upload plan.
            active_sessions: &[],
            now,
            pcgw_paths: &pcgw_paths,
            wine_prefix: None,
        };

        let mut caches = CloudCaches::default();

        let save_entry =
            ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &game, SaveType::Save);
        let save_entry_name = save_entry.as_ref().map(|e| e.name.as_str()).unwrap_or("");
        let save_mtime =
            if ops::block_reason_for_game(&ctx, &game, SaveType::Save, save_entry.as_ref())
                .is_empty()
            {
                ops::latest_local_save_mtime(&ctx, &mut caches, &game, save_entry_name)
            } else {
                0.0
            };

        let state_entry =
            ops::resolved_cloud_emulator_entry(&ctx, &mut caches, &game, SaveType::State);
        let state_entry_name = state_entry.as_ref().map(|e| e.name.as_str()).unwrap_or("");
        let state_reason =
            ops::block_reason_for_game(&ctx, &game, SaveType::State, state_entry.as_ref());
        let include_state = state_reason.is_empty();
        // `latest_local_state_mtime` itself returns 0.0 for RPCS3 and for
        // "no state directories configured" — folding those into the
        // plan's own `> 0.0` gate rather than re-deriving them here.
        let state_mtime = if include_state {
            ops::latest_local_state_mtime(&ctx, &mut caches, &game, state_entry_name)
        } else {
            0.0
        };

        let sync_entry = sync_entry_for(&config, &key);
        let plan = auto_cloud_upload_plan(&sync_entry, save_mtime, state_mtime, include_state);
        if plan.types.is_empty() {
            return;
        }

        let mut per_type: BTreeMap<SaveType, PerTypeResult> = BTreeMap::new();
        for save_type in plan.types.clone() {
            let report = ops::upload::upload_cloud_files_for_game(
                &client,
                &ctx,
                &mut caches,
                &game,
                save_type,
            )
            .await;
            per_type.insert(
                save_type,
                PerTypeResult {
                    uploaded: report.uploaded as i64,
                    total: report.total as i64,
                    failed: report.failed,
                },
            );
        }

        let uploaded_at = now_iso_z();
        let (update, debug_segments) =
            summarize_auto_cloud_upload_result(&per_type, &plan.latest_mtimes, &uploaded_at);
        if !debug_segments.is_empty() {
            tracing::debug!("cloud auto-upload ({key}): {}", debug_segments.join(" "));
        }

        // Fix round 1, FIX 1: reload fresh from disk immediately before
        // applying — `config` was captured before the (up to 60 s) delay
        // sleep AND before every network call this function just made;
        // saving from that stale snapshot would silently clobber
        // whatever another session's stamp, a settings change, or a
        // concurrent auto-upload for a different game wrote to disk in
        // the meantime. Mirrors `handle_session_finished`'s own
        // reload-then-apply-then-save pattern for its session-end stamp.
        if let Err(e) = reload_apply_and_save(&config_path, &key, update).await {
            tracing::debug!("cloud auto-upload: config save failed: {e}");
        }
    }
}

// ---------------------------------------------------------------------
// Owned per-call context inputs
// ---------------------------------------------------------------------

struct Inputs {
    config: Config,
    profiles: &'static [EmulatorProfile],
    all_games: Vec<CloudGame>,
    #[allow(dead_code)] // kept for callers that need the raw registry rows too
    installed: Vec<InstalledGame>,
    config_dir: PathBuf,
    active_sessions: Vec<grid_core::cloud::window::ActiveSessionRef>,
    now: f64,
}

impl Inputs {
    /// `pcgw_paths` is threaded in by the caller (Task 18: [`CloudService::cached_pcgw_paths`],
    /// a cache-only read keyed on the specific game's title — `Inputs`
    /// itself is built once per command with no single game in view, so it
    /// cannot resolve this on its own).
    fn context<'a>(&'a self, pcgw_paths: &'a [String]) -> CloudContext<'a> {
        CloudContext {
            config: &self.config,
            profiles: self.profiles,
            all_games: &self.all_games,
            resolve_ctx: grid_core::cloud::dirs::ResolveContext {
                emulator_dir: None,
                library_dir: &self.config.library_path,
                config_dir: &self.config_dir,
                windows_documents: None,
            },
            active_sessions: &self.active_sessions,
            now: self.now,
            pcgw_paths,
            wine_prefix: None,
        }
    }
}

fn active_session_refs(
    sessions: &[GameSession],
    installed: &[InstalledGame],
) -> Vec<grid_core::cloud::window::ActiveSessionRef> {
    sessions
        .iter()
        .filter_map(|s| {
            installed
                .iter()
                .find(|g| g.rom_id == Some(s.rom_id))
                .map(|g| grid_core::cloud::window::ActiveSessionRef {
                    game: cloud_game_from_installed(g),
                    started_at: s.started_at as f64,
                })
        })
        .collect()
}

fn merge_update(base: &mut SyncStateUpdate, other: SyncStateUpdate) {
    macro_rules! take {
        ($field:ident) => {
            if other.$field.is_some() {
                base.$field = other.$field;
            }
        };
    }
    take!(last_downloaded_save_id);
    take!(last_server_timestamp);
    take!(last_uploaded_local_mtime);
    take!(last_uploaded_at);
    take!(last_downloaded_state_id);
    take!(last_uploaded_save_mtime);
    take!(last_uploaded_state_mtime);
    take!(last_session_started_at);
    take!(last_session_ended_at);
}

fn log_cloud_messages(context: &str, messages: &[CloudMessage]) {
    for message in messages {
        tracing::debug!("cloud {context}: {}", message.text);
    }
}

/// The single clamp site (task ruling): `auto_cloud_save_upload_delay_seconds`
/// bounded 0..=60 wherever it is actually consumed. Nothing else clamps.
fn clamped_upload_delay_seconds(config: &Config) -> u64 {
    config.auto_cloud_save_upload_delay_seconds.min(60)
}

/// UTC now, ISO-8601 seconds, `Z` offset — `datetime.now(UTC).isoformat(
/// timespec="seconds").replace("+00:00", "Z")` (cloud_mixin.py:2947-2948).
fn now_iso_z() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ---------------------------------------------------------------------
// blocking helpers (matches commands.rs's existing spawn_blocking convention)
// ---------------------------------------------------------------------

async fn blocking_installed(install: Arc<InstallService>) -> Result<Vec<InstalledGame>, String> {
    tokio::task::spawn_blocking(move || install.installed().map_err(err))
        .await
        .map_err(|e| format!("registry lookup did not finish: {e}"))?
}

async fn blocking_load_config(config_path: PathBuf) -> Result<Config, String> {
    tokio::task::spawn_blocking(move || Config::load(&config_path).map_err(err))
        .await
        .map_err(|e| format!("config load did not finish: {e}"))?
}

async fn blocking_save_config(config_path: PathBuf, config: Config) -> Result<(), String> {
    tokio::task::spawn_blocking(move || config.save(&config_path).map_err(err))
        .await
        .map_err(|e| format!("config save did not finish: {e}"))?
}

/// Reloads `config_path` fresh from disk, applies `update` under `key`,
/// and saves — fix round 1, FIX 1: every sync-state persist that follows
/// a nontrivial `.await` gap (a network call, a delay sleep) MUST reload
/// immediately before applying, never reuse a `Config` snapshot taken
/// before that gap. A stale snapshot's save would silently overwrite
/// whatever anything else (another session's stamp, a settings change, a
/// concurrent auto-upload for a different game) wrote to disk in the
/// meantime — the exact bug this helper exists to make structurally hard
/// to reintroduce. A blank `key` or a no-op `update` is a no-op (no
/// load, no save), matching [`apply_sync_update`]'s own guard.
async fn reload_apply_and_save(
    config_path: &Path,
    key: &str,
    update: SyncStateUpdate,
) -> Result<(), String> {
    if key.is_empty() || update == SyncStateUpdate::default() {
        return Ok(());
    }
    let mut config = blocking_load_config(config_path.to_path_buf()).await?;
    apply_sync_update(&mut config, key, update);
    blocking_save_config(config_path.to_path_buf(), config).await
}

// ---------------------------------------------------------------------
// CloudGame construction (rulings: id fields blank — data gap, recorded)
// ---------------------------------------------------------------------

/// The `game` parameter every cloud command takes. Deliberately a
/// separate DTO from `grid_core::library::registry::InstalledGame`
/// (which has no `Deserialize`, only `Serialize` — it never needs to
/// cross the IPC boundary in the other direction): the frontend already
/// holds an `InstalledGame`-shaped object from `list_installed`/
/// `list_games` and passes exactly these fields back, with `serde`
/// silently ignoring the ones this DTO doesn't need.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudGameInput {
    pub title: String,
    pub platform: String,
    pub rom_id: Option<i64>,
    #[serde(default)]
    pub rom_file_name: String,
    #[serde(default)]
    pub archive_path: String,
    #[serde(default)]
    pub extracted_path: String,
    #[serde(default)]
    pub description: String,
}

/// `CloudGame` from an `InstalledGame` registry row (task ruling):
/// title/platform/rom_id (string form, `""` when `None`)/rom_file_name/
/// archive_path/extracted_path/description; `title_id`/`base_title_id`/
/// `ps3_game_id` stay blank — the registry does not carry them yet (same
/// documented gap `CloudGame`'s own doc comment records).
pub fn cloud_game_from_installed(game: &InstalledGame) -> CloudGame {
    CloudGame {
        title: game.title.clone(),
        platform: game.platform.clone(),
        rom_id: game.rom_id.map(|id| id.to_string()).unwrap_or_default(),
        rom_file_name: game.rom_file_name.clone(),
        extracted_path: game.extracted_path.clone(),
        archive_path: game.archive_path.clone(),
        description: game.description.clone(),
        title_id: String::new(),
        base_title_id: String::new(),
        ps3_game_id: String::new(),
    }
}

fn cloud_game_from_input(game: &CloudGameInput) -> CloudGame {
    CloudGame {
        title: game.title.clone(),
        platform: game.platform.clone(),
        rom_id: game.rom_id.map(|id| id.to_string()).unwrap_or_default(),
        rom_file_name: game.rom_file_name.clone(),
        extracted_path: game.extracted_path.clone(),
        archive_path: game.archive_path.clone(),
        description: game.description.clone(),
        title_id: String::new(),
        base_title_id: String::new(),
        ps3_game_id: String::new(),
    }
}

// ---------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveScopeDto {
    PerGame,
    SharedSingle,
    SharedSlotted,
}

impl From<SaveScope> for SaveScopeDto {
    fn from(scope: SaveScope) -> Self {
        match scope {
            SaveScope::PerGame => SaveScopeDto::PerGame,
            SaveScope::SharedSingle => SaveScopeDto::SharedSingle,
            SaveScope::SharedSlotted => SaveScopeDto::SharedSlotted,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudPanelInfoDto {
    pub supported: bool,
    pub block_reason: String,
    pub scope: SaveScopeDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudRecordDto {
    pub id: i64,
    pub file_name: String,
    pub emulator: String,
    pub slot: Option<String>,
    pub size_text: String,
    pub absolute_time: String,
    pub relative_time: String,
    pub restorable: bool,
    /// The `restore_tooltip` Python's `_details_cloud_restore_enabled`
    /// pairs with the Restore button's enabled state either way — a
    /// refusal reason when `restorable` is `false`, or the shared-scope
    /// notice (possibly absent) when it's `true`. Fix round 1, FIX 4.
    pub restore_tooltip: Option<String>,
}

fn record_id_i64(record: &Value) -> Option<i64> {
    match record.get("id") {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

fn record_id_matches(record: &Value, id: i64) -> bool {
    record_id_i64(record) == Some(id)
}

/// `format_size` (`grid_launcher/library/downloads.py:23-30`): binary
/// (1024-based) units, 0 decimals for bytes, 1 decimal otherwise.
fn format_size(size_bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = size_bytes.max(0.0);
    let mut unit_index = 0usize;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{size:.0} {}", UNITS[unit_index])
    } else {
        format!("{size:.1} {}", UNITS[unit_index])
    }
}

/// Any JSON scalar, stringified — `str(value)` for the one Python field
/// (`slot`) that can arrive as either a string or a number.
fn stringify_json_scalar(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// `size_value = record.get("file_size_bytes", 0)` / `format_size(int(
/// size_value))` except `TypeError`/`ValueError` -> `"Unknown size"`
/// (`details_view_mixin.py:725-729`, fold-in: reads ONLY
/// `file_size_bytes` — no `"size"` fallback). An ABSENT key defaults to
/// Python's literal `0` (`"0 B"`); a PRESENT but non-numeric value is
/// what actually produces `"Unknown size"`.
fn size_text_for_record(record: &Value) -> String {
    match record.get("file_size_bytes") {
        None => format_size(0.0),
        Some(Value::Number(n)) => match n.as_f64() {
            Some(v) => format_size(v),
            None => "Unknown size".to_string(),
        },
        Some(Value::String(s)) => match s.trim().parse::<f64>() {
            Ok(v) => format_size(v),
            Err(_) => "Unknown size".to_string(),
        },
        Some(_) => "Unknown size".to_string(),
    }
}

fn record_dto(record: &Value, now: f64, restorable: bool, tooltip: String) -> CloudRecordDto {
    let id = record_id_i64(record).unwrap_or(0);
    let file_name = record
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let emulator = record
        .get("emulator")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let slot = record.get("slot").and_then(stringify_json_scalar);
    let timestamp = cloud_restore::record_timestamp(record);
    CloudRecordDto {
        id,
        file_name,
        emulator,
        slot,
        size_text: size_text_for_record(record),
        absolute_time: absolute_time_text(timestamp),
        relative_time: cloud_restore::relative_timestamp_text(timestamp, now),
        restorable,
        restore_tooltip: (!tooltip.trim().is_empty()).then_some(tooltip),
    }
}

/// `_details_cloud_uploaded_text` (`details_view_mixin.py:619-625`): LOCAL
/// time, `"%Y-%m-%d %H:%M"` (no seconds, no zone suffix — `strftime`'s
/// own text carries no "UTC"/offset marker either), and the fixed
/// `"Unknown upload time"` string for a non-positive timestamp. Fix
/// round 1, FIX 5: this port previously rendered UTC with a trailing
/// "UTC" and an empty string for the unknown case — both wrong.
fn absolute_time_text(timestamp: f64) -> String {
    if timestamp <= 0.0 {
        return "Unknown upload time".to_string();
    }
    match chrono::DateTime::from_timestamp(timestamp as i64, 0) {
        Some(utc) => {
            let local: chrono::DateTime<chrono::Local> = utc.into();
            local.format("%Y-%m-%d %H:%M").to_string()
        }
        None => "Unknown upload time".to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudMessageDto {
    pub text: String,
    pub severity: &'static str,
}

impl From<CloudMessage> for CloudMessageDto {
    fn from(message: CloudMessage) -> Self {
        Self {
            severity: match message.severity {
                MessageSeverity::Info => "info",
                MessageSeverity::Warning => "warning",
            },
            text: message.text,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadReportDto {
    pub uploaded: usize,
    pub total: usize,
    pub failed: Vec<String>,
    pub messages: Vec<CloudMessageDto>,
}

impl From<ops::upload::UploadReport> for UploadReportDto {
    fn from(report: ops::upload::UploadReport) -> Self {
        Self {
            uploaded: report.uploaded,
            total: report.total,
            failed: report.failed,
            messages: report
                .messages
                .into_iter()
                .map(CloudMessageDto::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreReportDto {
    pub ok: bool,
    pub messages: Vec<CloudMessageDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NativeSavePathsDto {
    pub pcgw: Vec<String>,
    pub manual: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSettingsDto {
    pub download_on_launch: bool,
    pub upload_on_exit: bool,
    pub skip_if_local_newer: bool,
    pub upload_delay_seconds: u64,
    pub retention_limit: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn format_size_matches_python_thresholds_and_precision() {
        assert_eq!(format_size(0.0), "0 B");
        assert_eq!(format_size(512.0), "512 B");
        assert_eq!(format_size(1024.0), "1.0 KB");
        assert_eq!(format_size(1536.0), "1.5 KB");
        assert_eq!(format_size(1024.0 * 1024.0), "1.0 MB");
        assert_eq!(format_size(1024.0 * 1024.0 * 1024.0 * 1024.0), "1.0 TB");
        // Beyond TB stays in TB (last unit), matching the Python loop's
        // `unit_index < len(units) - 1` bound.
        assert_eq!(
            format_size(1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0),
            "1024.0 TB"
        );
    }

    /// Fix round 1, FIX 5: local time, no seconds, no zone suffix, and a
    /// fixed string for "no timestamp" — not the previous UTC-with-
    /// trailing-"UTC" rendering (which also returned `""` for `<= 0`).
    #[test]
    fn absolute_time_text_matches_python_local_time_format() {
        assert_eq!(absolute_time_text(0.0), "Unknown upload time");
        assert_eq!(absolute_time_text(-5.0), "Unknown upload time");
        let text = absolute_time_text(1_700_000_000.0);
        assert!(
            !text.contains("UTC"),
            "must not carry a zone suffix: {text}"
        );
        assert!(!text.is_empty());
        // "%Y-%m-%d %H:%M" is exactly 16 characters (no seconds).
        assert_eq!(text.len(), 16, "expected no seconds in: {text}");
    }

    /// Fix round 1, fold-in: reads ONLY `file_size_bytes` (no `"size"`
    /// fallback), defaults an ABSENT key to Python's literal `0`
    /// (`"0 B"`), and reports `"Unknown size"` only for a value that is
    /// actually present but non-numeric.
    #[test]
    fn size_text_for_record_reads_only_file_size_bytes() {
        assert_eq!(
            size_text_for_record(&serde_json::json!({"size": 999999})),
            "0 B",
            "the legacy `size` field must be ignored entirely"
        );
        assert_eq!(size_text_for_record(&serde_json::json!({})), "0 B");
        assert_eq!(
            size_text_for_record(&serde_json::json!({"file_size_bytes": 2048})),
            "2.0 KB"
        );
        assert_eq!(
            size_text_for_record(&serde_json::json!({"file_size_bytes": "not-a-number"})),
            "Unknown size"
        );
        assert_eq!(
            size_text_for_record(&serde_json::json!({"file_size_bytes": null})),
            "Unknown size"
        );
    }

    #[test]
    fn stringify_json_scalar_handles_numeric_and_string_slots() {
        assert_eq!(
            stringify_json_scalar(&serde_json::json!(2)),
            Some("2".to_string())
        );
        assert_eq!(
            stringify_json_scalar(&serde_json::json!("vmu1")),
            Some("vmu1".to_string())
        );
        assert_eq!(stringify_json_scalar(&serde_json::json!("")), None);
        assert_eq!(stringify_json_scalar(&serde_json::Value::Null), None);
    }

    /// Fix round 1, FIX 1: the exact regression this fix closes. A
    /// literal reproduction of `run_auto_upload`'s old bug — capture a
    /// `Config` snapshot, let something else mutate a DIFFERENT field on
    /// disk (standing in for "another session's stamp" / "a settings
    /// change" during the up-to-60s sleep or the upload's network
    /// calls), then persist through `reload_apply_and_save` rather than
    /// the stale snapshot. The concurrent mutation must survive.
    #[tokio::test]
    async fn reload_apply_and_save_reloads_immediately_before_saving() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        let initial = Config {
            library_path: "initial".to_string(),
            ..Default::default()
        };
        blocking_save_config(config_path.clone(), initial.clone())
            .await
            .unwrap();

        // This is the snapshot a caller might have captured BEFORE a
        // sleep/network gap — deliberately never reloaded before use
        // below, to prove the helper doesn't need (or accept) it.
        let _stale_snapshot = initial;

        // Something else mutates the on-disk config while our caller was
        // "asleep" — a field `reload_apply_and_save`'s own update never
        // touches.
        let mut concurrent = blocking_load_config(config_path.clone()).await.unwrap();
        concurrent.library_path = "changed-during-the-gap".to_string();
        blocking_save_config(config_path.clone(), concurrent)
            .await
            .unwrap();

        reload_apply_and_save(
            &config_path,
            "rom:1",
            SyncStateUpdate {
                last_uploaded_at: Some("2026-09-02T00:00:00Z".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let final_config = blocking_load_config(config_path.clone()).await.unwrap();
        assert_eq!(
            final_config.library_path, "changed-during-the-gap",
            "the concurrent mutation must survive the later save"
        );
        assert_eq!(
            sync_entry_for(&final_config, "rom:1").last_uploaded_at,
            "2026-09-02T00:00:00Z",
            "the caller's own update must still land"
        );
    }

    #[tokio::test]
    async fn reload_apply_and_save_is_a_noop_for_a_blank_key_or_empty_update() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        // No file exists yet — a no-op must never try to load/save it.
        reload_apply_and_save(&config_path, "", SyncStateUpdate::default())
            .await
            .unwrap();
        reload_apply_and_save(&config_path, "rom:1", SyncStateUpdate::default())
            .await
            .unwrap();
        assert!(!config_path.exists());
    }

    /// Fix round 1, FIX 3: a panicking task must not leak its in-flight
    /// key forever, and must not poison `inflight` for every later
    /// trigger either.
    #[tokio::test]
    async fn a_panicking_task_still_leaves_the_key_re_triggerable() {
        let pool = AutoUploadPool::new(2);
        let accepted = pool.trigger("game-x".to_string(), || async move {
            panic!("boom — simulated task failure");
        });
        assert!(accepted);

        // Give the spawned task time to panic and unwind (dropping its
        // `RemoveOnDrop` guard).
        tokio::time::sleep(Duration::from_millis(100)).await;

        let ran = Arc::new(AtomicUsize::new(0));
        let ran2 = ran.clone();
        let accepted_again = pool.trigger("game-x".to_string(), move || async move {
            ran2.fetch_add(1, Ordering::SeqCst);
        });
        assert!(
            accepted_again,
            "a panicked task must not leak its in-flight key forever, \
             nor poison the lock for every later trigger"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }

    /// Fix round 1, FIX 2: `run_auto_upload` must not serialize on the
    /// app-wide `self.caches` lock — two uploads for different games,
    /// each blocked on a slow (mocked) upload POST, must overlap rather
    /// than queue behind each other. Proven two ways: total wall time
    /// stays close to ONE mock delay (not two, which serialization would
    /// produce), and the mock server actually received both POSTs.
    #[tokio::test]
    async fn two_auto_uploads_for_different_games_run_concurrently() {
        use wiremock::matchers::{method, path as wm_path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        const DELAY: Duration = Duration::from_millis(200);
        Mock::given(method("POST"))
            .and(wm_path("/api/saves"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(DELAY)
                    .set_body_json(serde_json::json!({})),
            )
            .mount(&server)
            .await;
        // Retention refetch after a successful upload: empty, so pruning
        // finds nothing to delete and issues no further request.
        Mock::given(method("GET"))
            .and(wm_path("/api/saves"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let client = Arc::new(
            RommClient::new(
                &server.uri(),
                grid_core::secrets::Credential::Token(secrecy::SecretString::from(
                    "FAKE-TEST-TOKEN-not-real",
                )),
            )
            .unwrap(),
        );

        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        let saves_a = dir.path().join("saves_a");
        let saves_b = dir.path().join("saves_b");
        std::fs::create_dir_all(&saves_a).unwrap();
        std::fs::create_dir_all(&saves_b).unwrap();
        std::fs::write(saves_a.join("alpha.srm"), b"a").unwrap();
        std::fs::write(saves_b.join("beta.srm"), b"b").unwrap();

        let entry_a = grid_core::config::EmulatorEntry {
            name: "DolphinA".to_string(),
            path: String::new(),
            args: "%rom%".to_string(),
            save_paths: saves_a.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let entry_b = grid_core::config::EmulatorEntry {
            name: "DolphinB".to_string(),
            path: String::new(),
            args: "%rom%".to_string(),
            save_paths: saves_b.to_string_lossy().into_owned(),
            ..Default::default()
        };

        let mut config = Config::default();
        config
            .default_emulators
            .insert("GameCube".to_string(), "DolphinA".to_string());
        config
            .default_emulators
            .insert("Wii".to_string(), "DolphinB".to_string());
        config.emulators = vec![entry_a, entry_b];
        config.save(&config_path).unwrap();

        let game_a = CloudGame {
            title: "Alpha".to_string(),
            platform: "GameCube".to_string(),
            rom_id: "1".to_string(),
            ..Default::default()
        };
        let game_b = CloudGame {
            title: "Beta".to_string(),
            platform: "Wii".to_string(),
            rom_id: "2".to_string(),
            ..Default::default()
        };
        let all_games = vec![game_a.clone(), game_b.clone()];
        let config_dir = dir.path().to_path_buf();

        let cloud = CloudService::new();

        let start = std::time::Instant::now();
        let a = cloud.clone().run_auto_upload(
            client.clone(),
            game_a.clone(),
            all_games.clone(),
            config.clone(),
            config_dir.clone(),
            config_path.clone(),
            game_key(&game_a),
        );
        let b = cloud.clone().run_auto_upload(
            client.clone(),
            game_b.clone(),
            all_games,
            config.clone(),
            config_dir,
            config_path,
            game_key(&game_b),
        );
        tokio::join!(a, b);
        let elapsed = start.elapsed();

        let saves_posts = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.method == wiremock::http::Method::POST && r.url.path() == "/api/saves")
            .count();
        assert_eq!(
            saves_posts, 2,
            "both games' uploads must have actually reached the network"
        );

        // Serialized (the old shared-lock behavior), this would take
        // roughly 2x DELAY; overlapping, it stays close to one.
        assert!(
            elapsed < DELAY * 2 - Duration::from_millis(50),
            "expected the two auto-uploads to overlap, took {elapsed:?} for a {DELAY:?} mock delay"
        );
    }

    #[test]
    fn now_iso_z_never_emits_a_plus_offset() {
        let text = now_iso_z();
        assert!(text.ends_with('Z'), "expected a Z offset, got: {text}");
        assert!(!text.contains("+00:00"), "expected no +00:00, got: {text}");
    }

    /// D5 (spec): a coalesced trigger for a key already in flight is
    /// dropped, and global concurrency across every key is capped —
    /// asserted here with the exact cap this pool is constructed with in
    /// `CloudService::new` (2).
    #[tokio::test]
    async fn auto_upload_pool_coalesces_per_game_and_caps_at_two() {
        let pool = AutoUploadPool::new(2);
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));

        for i in 0..3 {
            let concurrent = concurrent.clone();
            let max_seen = max_seen.clone();
            let started = started.clone();
            let release = release.clone();
            pool.trigger(format!("game-{i}"), move || async move {
                started.fetch_add(1, Ordering::SeqCst);
                let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(now, Ordering::SeqCst);
                while !release.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                concurrent.fetch_sub(1, Ordering::SeqCst);
            });
        }

        // Give the pool a moment to admit as many as it will (2 of 3);
        // the third stays queued on the semaphore.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            2,
            "exactly 2 of the 3 triggers must have started"
        );
        assert_eq!(max_seen.load(Ordering::SeqCst), 2);

        // A duplicate trigger for a key already in flight (game-0, still
        // blocked) must be coalesced (dropped), not queued behind it.
        let coalesced_ran = Arc::new(AtomicUsize::new(0));
        let coalesced_ran2 = coalesced_ran.clone();
        let accepted = pool.trigger("game-0".to_string(), move || async move {
            coalesced_ran2.fetch_add(1, Ordering::SeqCst);
        });
        assert!(
            !accepted,
            "a trigger for an in-flight key must be coalesced"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(coalesced_ran.load(Ordering::SeqCst), 0);

        // Release the running two; the queued third must then get its slot.
        release.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            3,
            "the third trigger must eventually run once a slot frees"
        );
        assert!(max_seen.load(Ordering::SeqCst) <= 2, "cap never exceeded");

        // The key leaves the in-flight set once its task finishes, so a
        // fresh trigger for the same key is accepted again.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let ran_again = Arc::new(AtomicUsize::new(0));
        let ran_again2 = ran_again.clone();
        let accepted_again = pool.trigger("game-0".to_string(), move || async move {
            ran_again2.fetch_add(1, Ordering::SeqCst);
        });
        assert!(accepted_again, "a finished key must accept a new trigger");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(ran_again.load(Ordering::SeqCst), 1);
    }
}
