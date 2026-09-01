# 08 — Background work, threading, and cancellation

## Purpose

This document describes every piece of concurrent work GRID Launcher performs: which
tasks run off the UI thread, what starts them, what they produce, how their results get
back to the UI, what owns their lifetime, and what happens on shutdown. It is written so
the same concurrency behavior can be reimplemented in a language with different
primitives.

Language-neutral vocabulary used throughout:

- **worker** — a unit of work executed on a background task/thread. In Python it is a
  `QObject` subclass with a `run()` method.
- **completion event** — what the worker publishes when it finishes or makes progress.
  In Python this is a Qt signal carrying one dict payload.
- **host thread** — the thread the worker's `run()` executes on. There are two hosting
  styles in this codebase (see Behavior): an event-loop thread (`QThread`) and a plain
  OS thread (`threading.Thread`).
- **UI thread** — the thread that owns all widgets. Every completion handler named
  `_on_*` in this document runs there.

Scope covered here:

- `grid_launcher/background/workers.py` — the desktop worker family.
- `grid_launcher/tv/bridge/workers.py` plus TV-local workers in
  `tv/bridge/game_backend.py`, `tv/bridge/cloud_backend.py`, `tv/bridge/cloud_helpers.py`,
  `tv/bridge/app_backend.py`.
- Ad-hoc threads elsewhere (`grid-launcher.py`, `ui/mixins/emulator_ui_mixin.py`,
  `library/archive_preparation.py`, `tv/widgets/cover_loader.py`).
- Polling timers, thread-affinity rules, and shutdown.

Cross-references: download/install payload semantics live in
`docs/porting/03-library-install.md`; cloud save semantics live in
`docs/porting/06-cloud-saves.md`; controller input semantics live in the TV/controller
document (`docs/porting/09-*`). This
document only covers those workers' *concurrency* contracts.

---

## External surfaces

None. Threading introduces no new files, sockets, environment variables, or processes of
its own. Every external effect (HTTP requests, file writes, subprocess spawns) is
described in the document that owns that feature; this document only says *which thread
performs it*.

One adjacent surface worth naming because it is process-wide and startup-ordered: the
app enforces single-instance by probing and then listening on a local socket named
`grid-launcher-singleton` before `MainWindow` is constructed, and closes that socket from
the application's `aboutToQuit` event (grid-launcher.py:3781).

---

## Data model

### Worker inventory — desktop (`grid_launcher/background/workers.py`)

| Worker | Trigger | Emits (payload) | Consumer | Lifetime / ownership |
| --- | --- | --- | --- | --- |
| `InstallDownloadWorker` (grid_launcher/background/workers.py:32) | `_start_async_install` (grid_launcher/ui/mixins/install_mixin.py:1377) for ROM/content; `_start_async_source_emulator_install` (grid_launcher/ui/mixins/install_mixin.py:1493) for emulator sources | `progress` `{downloaded, total, speed}` throttled to one per 0.1 s (grid_launcher/background/workers.py:110); `finished` `{archive_path, error}` (grid_launcher/background/workers.py:67) | `_on_async_install_progress` (grid_launcher/ui/mixins/install_mixin.py:1731), `_on_async_install_finished` (grid_launcher/ui/mixins/install_mixin.py:1515) | Exactly one at a time app-wide; stored in `install_thread` / `install_worker` (grid-launcher.py:444) and cleared by `_on_install_thread_finished` (grid_launcher/ui/mixins/install_mixin.py:1721) |
| `InstallFinalizeWorker` (grid_launcher/background/workers.py:528) | `_start_async_install_finalize` (grid_launcher/ui/mixins/install_mixin.py:1597), always immediately after a successful download | `progress` `{installed, total}` (grid_launcher/background/workers.py:646); `finished` `{game, archive_path, warning, error}` (grid_launcher/background/workers.py:642) | `_on_async_install_finalize_progress` (grid_launcher/ui/mixins/install_mixin.py:1750), `_on_async_install_finalize_finished` (grid_launcher/ui/mixins/install_mixin.py:1612) | One at a time; `install_finalize_thread` / `install_finalize_worker` (grid-launcher.py:449), cleared by `_on_install_finalize_thread_finished` (grid_launcher/ui/mixins/install_mixin.py:1726) |
| `SourceVersionCheckWorker` (grid_launcher/background/workers.py:439) | `_start_source_emulator_update_at_index` (grid_launcher/ui/mixins/emulator_ui_mixin.py:1279) | `finished` `{installed_tag, available_tag, error}` (grid_launcher/background/workers.py:499) | `_on_source_version_check_finished_slot` (grid_launcher/ui/mixins/emulator_ui_mixin.py:1316) | One at a time, guarded by an explicit "is the previous thread still running" check (grid_launcher/ui/mixins/emulator_ui_mixin.py:1273); handles clear `_source_check_thread`/`_source_check_worker` on thread finish (grid_launcher/ui/mixins/emulator_ui_mixin.py:1288) |
| `AutoCloudSaveUploadWorker` (grid_launcher/background/workers.py:650) | `_start_auto_cloud_upload_worker` (grid_launcher/ui/mixins/cloud_mixin.py:2940), after a game session ends | `finished` `{game, result:{per_type, local_latest_mtimes}}` (grid_launcher/background/workers.py:685) | `_on_auto_cloud_upload_finished` (grid_launcher/ui/mixins/cloud_mixin.py:2958) | Unbounded fan-out; appended to `auto_cloud_upload_threads` / `auto_cloud_upload_workers` lists (grid-launcher.py:485) and removed by `_cleanup_auto_cloud_upload_worker` (grid_launcher/ui/mixins/cloud_mixin.py:2954) |
| `DetailsCloudRecordsWorker` (grid_launcher/background/workers.py:713) | `_start_details_cloud_records_worker` (grid_launcher/ui/mixins/details_view_mixin.py:808) when the details cloud panel opens or switches save/state mode | `finished` `{request_id, save_type, records, error}` (grid_launcher/background/workers.py:749) | `_on_details_cloud_records_loaded` (grid_launcher/ui/mixins/details_view_mixin.py:828) | Unbounded fan-out; tracked in `details_cloud_threads` / `details_cloud_workers` (grid-launcher.py:487), removed by `_cleanup_details_cloud_worker` (grid_launcher/ui/mixins/details_view_mixin.py:824). Stale results are dropped by monotonic `request_id` |
| `RomDetailWorker` (grid_launcher/background/workers.py:760) | `_start_rom_detail_lookup` (grid_launcher/ui/mixins/details_view_mixin.py:206) when a details page lacks server metadata | `finished` `{rom_id, payload, error}` (grid_launcher/background/workers.py:774) | `_on_rom_detail_loaded` (grid_launcher/ui/mixins/details_view_mixin.py:228) | At most one tracked; a new request asks the previous thread to quit (grid_launcher/ui/mixins/details_view_mixin.py:202). Result is discarded unless its `rom_id` still matches the open details game (grid_launcher/ui/mixins/details_view_mixin.py:237) |
| `RetroAchievementsWorker` (grid_launcher/background/workers.py:779) | `_load_achievements_for_ra_id` (grid_launcher/ui/mixins/details_view_mixin.py:1931) | `finished` `{request_id, achievements, error}` (grid_launcher/background/workers.py:794) | `_on_achievements_loaded` (grid_launcher/ui/mixins/details_view_mixin.py:1949) | One tracked in `_ra_thread`/`_ra_worker` (grid-launcher.py:410); stale results dropped by `request_id` |
| `RALoginWorker` (grid_launcher/background/workers.py:799) | `_ra_login_clicked` (grid-launcher.py:2705) | `finished` `{username, token, error}` (grid_launcher/background/workers.py:814) | `_on_ra_login_finished` (grid-launcher.py:2730) | One tracked in `_ra_login_thread`/`_ra_login_worker` (grid-launcher.py:412); the login button is disabled for the duration (grid-launcher.py:2713) |
| `PCGamingWikiWorker` (grid_launcher/background/workers.py:819) | `_start_pcgw_lookup_for_game` (grid_launcher/ui/mixins/details_view_mixin.py:164) | `finished` `{request_id, paths, error}` (grid_launcher/background/workers.py:831) | `_on_pcgw_paths_loaded` (grid_launcher/ui/mixins/details_view_mixin.py:264) | One tracked in `_pcgw_thread`/`_pcgw_worker` (grid-launcher.py:416); `_pending_pcgw_request_id` decides staleness |
| `MissingCoverReplenishWorker` (grid_launcher/background/workers.py:836) | `_start_missing_cover_replenish` (grid-launcher.py:2846), called once each time the server connection succeeds (grid-launcher.py:3039) | `game_cover_cached` `{game_key, path}` once per downloaded cover (grid_launcher/background/workers.py:880); `finished` with no payload (grid_launcher/background/workers.py:882) | `_on_cover_replenish_game_cached` (grid-launcher.py:2877), `_on_cover_replenish_finished` (grid-launcher.py:2887) | One at a time, guarded by an is-running check (grid-launcher.py:2847). Iterates a snapshot list captured on the UI thread |
| `DiscoverLoadWorker` (grid_launcher/background/workers.py:885) | `_start_discover_load_thread` (grid-launcher.py:1242), reached from `_refresh_discover_data` (grid-launcher.py:1171) or the hourly staleness timer (grid-launcher.py:1181) | `finished` with one aggregate dict of Discover sections (grid_launcher/background/workers.py:1116); `error` with a message string, only if *nothing* was produced (grid_launcher/background/workers.py:1113) | `_on_discover_data_loaded` (grid-launcher.py:1258), `_on_discover_data_error` (grid-launcher.py:1414) | One at a time, guarded by an is-running check (grid-launcher.py:1239). Internally fans out to a 12-slot task pool (grid_launcher/background/workers.py:1049) |

### Worker inventory — TV mode (`grid_launcher/tv/bridge/`)

| Worker | Trigger | Emits (payload) | Consumer | Lifetime / ownership |
| --- | --- | --- | --- | --- |
| `CatalogFetchWorker` (grid_launcher/tv/bridge/workers.py:9) | `_start_catalog_fetch` (grid_launcher/tv/bridge/app_backend.py:626) from `connectToServer` (grid_launcher/tv/bridge/app_backend.py:386) | `finished` `{me, platforms}`; `error` message string (grid_launcher/tv/bridge/workers.py:23) | `_on_catalog_finished` / `_on_catalog_error` (grid_launcher/tv/bridge/app_backend.py:661, :686) | One at a time via `_is_thread_running(self._catalog_thread)` (grid_launcher/tv/bridge/app_backend.py:628); plain daemon thread |
| `RomListFetchWorker` (grid_launcher/tv/bridge/workers.py:28) | `_start_rom_fetch` (grid_launcher/tv/bridge/app_backend.py:638) | `finished` `{platform_label, games}`; `error` `{platform_label, message}` (grid_launcher/tv/bridge/workers.py:63) | `_on_roms_finished` / `_on_roms_error` (grid_launcher/tv/bridge/app_backend.py:693, :703) | One per platform label, keyed in `_rom_threads` (grid_launcher/tv/bridge/app_backend.py:90); dead entries reaped by `_on_rom_fetch_thread_done` (grid_launcher/tv/bridge/app_backend.py:655) |
| `FavoritesRomFetchWorker` (grid_launcher/tv/bridge/workers.py:68) | `_start_favorites_fetch` (grid_launcher/tv/bridge/app_backend.py:570) | `finished` list of ≤20 games (grid_launcher/tv/bridge/workers.py:97); `error` string | `_on_favorites_finished` (grid_launcher/tv/bridge/app_backend.py:710) | One at a time; `_favorites_thread` (grid_launcher/tv/bridge/app_backend.py:96) |
| `NewAdditionsRomFetchWorker` (grid_launcher/tv/bridge/workers.py:102) | `_start_curated_rows_fetch` (grid_launcher/tv/bridge/app_backend.py:585) | `finished` list of games; `error` string | `_on_new_additions_finished` (grid_launcher/tv/bridge/app_backend.py:778) | One at a time; `_new_additions_thread` |
| `HighlyRatedRomFetchWorker` (grid_launcher/tv/bridge/workers.py:135) | `_start_curated_rows_fetch` (grid_launcher/tv/bridge/app_backend.py:585) | `finished` top-20 sorted list; `error` string (grid_launcher/tv/bridge/workers.py:170) | `_on_highly_rated_finished` (grid_launcher/tv/bridge/app_backend.py:789) | One at a time; `_highly_rated_thread`. Fetches up to 500 ROMs and filters/sorts in the worker |
| `SavesBatchFetchWorker` (grid_launcher/tv/bridge/workers.py:175) | `_start_saves_fetch` (grid_launcher/tv/bridge/app_backend.py:614) | `finished` list of distinct `rom_id` strings (grid_launcher/tv/bridge/workers.py:191) | `_on_saves_finished` (grid_launcher/tv/bridge/app_backend.py:800) | One at a time; `_saves_thread` |
| `CollectionsFetchWorker` (grid_launcher/tv/bridge/workers.py:196) | `toggleFavorite` (grid_launcher/tv/bridge/app_backend.py:441) | `finished` `{id, rom_ids}` for the favourites collection, or `None` when none exists (grid_launcher/tv/bridge/workers.py:218/:220) | `_on_toggle_collections_fetched` (grid_launcher/tv/bridge/app_backend.py:722) | Step 1 of a 2-step chain; single-flight via `_toggle_thread` (grid_launcher/tv/bridge/app_backend.py:448) |
| `CollectionCreateWorker` (grid_launcher/tv/bridge/workers.py:254) | Step 2a, when no favourites collection exists (grid_launcher/tv/bridge/app_backend.py:727) | `finished` raw create response; `error` string | `_on_toggle_collection_created` (grid_launcher/tv/bridge/app_backend.py:763) | Replaces `_toggle_thread` in place; cleared by the handler |
| `CollectionUpdateWorker` (grid_launcher/tv/bridge/workers.py:225) | Step 2b, when a favourites collection exists (grid_launcher/tv/bridge/app_backend.py:743) | `finished` raw update response; `error` string | `_on_toggle_collection_updated` (grid_launcher/tv/bridge/app_backend.py:753) | Same as above; on success it re-triggers `_start_favorites_fetch` |
| `_RomMetaFetchWorker` (grid_launcher/tv/bridge/app_backend.py:24) | `fetchRomMetadata` (grid_launcher/tv/bridge/app_backend.py:346), only when a game record is missing fields | `finished` `{rom_id, metadata}` (grid_launcher/tv/bridge/app_backend.py:42) | `_on_rom_meta_finished` (grid_launcher/tv/bridge/app_backend.py:810) | One per `rom_id`, keyed in `_rom_meta_threads` (grid_launcher/tv/bridge/app_backend.py:109); re-entry for the same id is skipped while alive (grid_launcher/tv/bridge/app_backend.py:374) |
| `InstallDownloadWorker` (aliased `_InstallDownloadWorker`, grid_launcher/tv/bridge/game_backend.py:69) | `installGame` (grid_launcher/tv/bridge/game_backend.py:544) | Same payloads as desktop (see doc 03) | `_on_install_progress` (grid_launcher/tv/bridge/game_backend.py:559), `_on_install_download_done` (grid_launcher/tv/bridge/game_backend.py:566) | One at a time, enforced by the `isInstallActive` property (grid_launcher/tv/bridge/game_backend.py:503) |
| TV install finalize closure `_run_finalize` (grid_launcher/tv/bridge/game_backend.py:665) | `_on_install_download_done` after a successful download | Publishes through the backend's own `_finalizeResult` event with `{game_json, archive_path, warning, error}` (grid_launcher/tv/bridge/game_backend.py:651) | `_on_install_finalize_done` (grid_launcher/tv/bridge/game_backend.py:671) | Plain daemon thread stored in `_finalize_thread`; counts toward `isInstallActive` (grid_launcher/tv/bridge/game_backend.py:198). Result payload is JSON-serialized to cross the boundary as a string |
| `_ProcessWatchThread` (grid_launcher/tv/bridge/game_backend.py:77) | `_do_launch` after the emulator process starts (grid_launcher/tv/bridge/game_backend.py:416) | `_exited` with the emulator name, after a blocking process wait (grid_launcher/tv/bridge/game_backend.py:95) | `_on_process_exited` (grid_launcher/tv/bridge/game_backend.py:808) | One per session; the handler joins it with an unbounded wait then deletes it (grid_launcher/tv/bridge/game_backend.py:812) |
| `_TvAutoRestoreWorker` (grid_launcher/tv/bridge/cloud_helpers.py:307) | `launchGame` when auto-download-on-launch is enabled (grid_launcher/tv/bridge/game_backend.py:322) | `finished` `{ok, message}` | `_on_restore_worker_done` (grid_launcher/tv/bridge/game_backend.py:422) | Blocks the launch: the actual process spawn is deferred until this completes. Previous restore thread is quit + waited 500 ms first (grid_launcher/tv/bridge/game_backend.py:313) |
| `_TvAutoUploadWorker` (grid_launcher/tv/bridge/game_backend.py:98) | `_on_process_exited` when auto-upload-on-exit is enabled (grid_launcher/tv/bridge/game_backend.py:830) | `finished` `{success, message}` | `_on_auto_upload_done` (grid_launcher/tv/bridge/game_backend.py:843) | One tracked in `_auto_upload_thread`; fire-and-forget |
| `_SlotFetchWorker` (grid_launcher/tv/bridge/cloud_backend.py:31) | `loadSlotsForGame` (grid_launcher/tv/bridge/cloud_backend.py:148) | `finished` `{save_type, slots}`; `error` object | `_on_slots_loaded` / `_on_slots_error` (grid_launcher/tv/bridge/cloud_backend.py:292, :300) | One at a time; a new request cancels the previous via `_cancel_fetch_thread` (grid_launcher/tv/bridge/cloud_backend.py:277) |
| `_CloudUploadWorker` (grid_launcher/tv/bridge/cloud_backend.py:70) | `uploadSave` (grid_launcher/tv/bridge/cloud_backend.py:264) | `finished` `{success, message}` | `_on_upload_done` (grid_launcher/tv/bridge/cloud_backend.py:312) | One at a time; `_cancel_upload_thread` (grid_launcher/tv/bridge/cloud_backend.py:284) |
| `_XInputPollThread` (grid_launcher/tv/bridge/controller.py:63) / `_GamepadPollThread` (grid_launcher/tv/bridge/controller.py:227) | `ControllerBackend.start` (grid_launcher/tv/bridge/controller.py:369, :376) | `event_received` `{code, value}` per input edge | `_on_raw_event`, connected explicitly as a queued delivery (grid_launcher/tv/bridge/controller.py:370) | Long-lived poll loop for the whole TV session. See doc 09 for input semantics |

### Ad-hoc background tasks (no worker class)

| Task | Site | Publishes via | Lifetime |
| --- | --- | --- | --- |
| Server platform ROM fetch | `_load_server_games` → inner `_fetch` on a daemon thread (grid-launcher.py:3122) | Writes the tuple `(games, payloads, error)` into `self._platform_games_results[platform_label]`, then emits `_platform_games_ready(platform_label)` (grid-launcher.py:291); the UI thread pops the entry in `_on_platform_games_ready` (grid-launcher.py:3124) | One per platform label; in-flight labels tracked in `_server_platforms_loading` (grid-launcher.py:3099) |
| Emulator platform cache warm-up | `_warm_emulator_platform_caches` (grid_launcher/ui/mixins/emulator_ui_mixin.py:901), itself deferred to the next UI event-loop turn (grid_launcher/ui/mixins/emulator_ui_mixin.py:899) | Nothing. It only populates `_platform_default_emulator_cache` / `_platform_available_emulator_cache` (grid-launcher.py:424) | Fire-and-forget daemon thread; no tracking handle |
| PS3 firmware download | `_trigger_rpcs3_firmware_download_background` → `_worker` (grid_launcher/ui/mixins/emulator_ui_mixin.py:1787) | `_firmware_download_progress` and `_firmware_download_done` events (grid-launcher.py:289) plus `_emulator_refresh_requested` | Fire-and-forget daemon thread; no handle. Occupies a download-status entry (`_firmware_download_entry_id`) |
| Firmware install for a source-installed emulator | `_trigger_firmware_install_for_source_emulator` → `_worker` (grid_launcher/ui/mixins/emulator_ui_mixin.py:1914) | `_emulator_refresh_requested` only | Fire-and-forget daemon thread |
| Archive-delete retry after AV lock | `_delete_with_background_retry` (grid_launcher/library/archive_preparation.py:190) | Nothing; silent | Daemon thread that sleeps 5 s then retries up to 60 times at 1 s intervals |
| 7-Zip extraction with progress | `_run_7z_extraction` (grid_launcher/library/archive_preparation.py:603) | Nothing across threads; the *calling* thread polls directory size every 0.15 s and then joins (grid_launcher/library/archive_preparation.py:605) | Bounded by the join; the caller is itself already a worker thread |
| TV cover image load | `CoverLoader.load_async` → `_worker` (grid_launcher/tv/widgets/cover_loader.py:120) | Hops back by scheduling `_deliver` as a zero-delay UI-thread callback (grid_launcher/tv/widgets/cover_loader.py:116) | One thread per requested image; cancellation is per-batch (see Behavior) |

### Timers

| Timer | Interval / mode | Purpose |
| --- | --- | --- |
| `session_poll_timer` (grid-launcher.py:513) | 2500 ms, repeating, started at construction | Polls tracked game processes and reacts to exits (`_poll_active_game_sessions`, grid_launcher/ui/mixins/cloud_mixin.py:2847) |
| `discover_auto_refresh_timer` (grid-launcher.py:520) | 3 600 000 ms, repeating | Force-refreshes Discover once its cache is 7+ days old (grid-launcher.py:1180) |
| `downloads_refresh_timer` (grid-launcher.py:458) | 120 ms, single-shot, coalescing | Debounces rebuilds of the Downloads page |
| Auto-upload delay (grid_launcher/ui/mixins/cloud_mixin.py:2879) | single-shot, configured seconds | Delays the post-session cloud upload |
| Details cloud worker start (grid_launcher/ui/mixins/details_view_mixin.py:820) | zero-delay single-shot | Defers `thread.start()` by one event-loop turn so the panel paints first |
| Post-launch process check (grid_launcher/ui/mixins/details_view_mixin.py:1441, :1471; grid_launcher/ui/mixins/emulator_ui_mixin.py:1662) | 500 ms single-shot | Warns if a just-spawned process already exited |
| Discover "updated N ago" label (grid_launcher/ui/discover.py:523) | 60 000 ms repeating | Cosmetic label refresh |
| TV home row debounces (grid_launcher/tv/widgets/views/home_view.py:86, :91, :96, :101) | 80 ms single-shot each | Coalesce row rebuilds after data events |
| TV library fanart debounce (grid_launcher/tv/widgets/views/library_view.py:84) | 500 ms single-shot | Delays fanart swap while navigating |
| TV fanart cycle (grid_launcher/tv/widgets/components/fanart_background.py:52) | 5000 ms repeating | Rotates background art |
| TV library-changed debounce (grid_launcher/tv/bridge/app_backend.py:843) | 300 ms single-shot | Coalesces `libraryGamesChanged` notifications |
| Spinner animation (grid_launcher/ui/spinner.py:22) | 16 ms repeating while visible | Cosmetic |
| Toast dismiss (grid_launcher/ui/toast.py:55), TV status banners (grid_launcher/tv/widgets/views/details_view.py:307, grid_launcher/tv/widgets/components/cloud_saves_overlay.py:73), TV scrollbar fade (grid_launcher/tv/widgets/components/scrollbar.py:22) | single-shot | Cosmetic auto-hide |

---

## Behavior

### 1. The two hosting styles

**Style A — event-loop thread (`QThread` + `moveToThread`).** Used for every desktop
worker and for the TV cloud/game backends. The wiring is identical at every site; the
canonical example is the install download start (grid_launcher/ui/mixins/install_mixin.py:1376):

1. Create a thread object owned by the window/backend.
2. Create the worker on the UI thread, then reassign its affinity to the new thread.
3. Connect "thread started" → worker's `run`.
4. Connect worker's `finished` → the UI-thread handler, and → "thread quit".
5. Connect "thread finished" → delete worker, delete thread, and (usually) a cleanup
   callback that nulls the tracking handles.
6. Start the thread.

Ordering guarantee that matters for a port: the UI-thread completion handler is invoked
*before* the thread teardown callbacks, because both are queued on the same completion
event and Qt invokes connections in connection order. The cleanup callback that nulls
`install_thread`/`install_worker` is attached to the *thread's* finished event, which
happens strictly after the worker's finished event has been delivered
(grid_launcher/ui/mixins/install_mixin.py:1386).

**Style B — plain daemon thread.** Used by all of `tv/bridge/app_backend.py` and by the
ad-hoc tasks. The worker object is constructed on the UI thread and then its `run` method
is called directly as the thread body (e.g. grid_launcher/tv/bridge/app_backend.py:622).
The worker never changes affinity, so it still "belongs" to the UI thread even though its
code executes elsewhere. Delivery is still asynchronous and marshalled, because delivery
mode is decided by comparing the *emitting* thread with the *receiving object's* thread.

A port that does not have this automatic rule must marshal explicitly: capture the result,
post it to the UI queue, and run the handler there.

### 2. Result marshalling

Three marshalling mechanisms exist, and a port needs all three:

1. **Payload-carrying completion event.** The dominant pattern. One event, one dictionary,
   always including an `error` key (empty string means success). The worker never throws
   across the boundary — see Invariants.
2. **Signal-only notification plus a shared staging slot.** Used by
   `_load_server_games`: the background thread writes into
   `self._platform_games_results[platform_label]` and then emits a signal carrying only
   the label; the UI handler pops the value (grid-launcher.py:3122, grid-launcher.py:3125).
   The in-code justification is that a single dict assignment is atomic under the GIL
   (grid-launcher.py:3117). A port without that guarantee must add a lock or carry the
   payload in the event.
3. **Deferred callback onto the UI queue.** `CoverLoader.load_async` schedules `_deliver`
   as a zero-delay callback bound to the application object
   (grid_launcher/tv/widgets/cover_loader.py:116), falling back to a direct call when no
   application exists (grid_launcher/tv/widgets/cover_loader.py:118).

Some connections request queued delivery explicitly rather than relying on the automatic
rule: the favourites-toggle chain (grid_launcher/tv/bridge/app_backend.py:453), the TV
finalize result (grid_launcher/tv/bridge/game_backend.py:164), the process-exit event
(grid_launcher/tv/bridge/game_backend.py:418), the restore completion
(grid_launcher/tv/bridge/game_backend.py:327), and controller input events
(grid_launcher/tv/bridge/controller.py:370).

### 3. Cancellation semantics, per worker family

There is no generic cancellation. Four distinct mechanisms exist.

**(a) Cooperative flag — downloads only.** `InstallDownloadWorker` exposes
`request_cancel()`, which sets a boolean checked at the top of each 64 KiB read loop
iteration (grid_launcher/background/workers.py:55, grid_launcher/background/workers.py:113).
When set, the worker raises an OS error with the text `Download cancelled by user`
(grid_launcher/background/workers.py:114), which flows down the normal error path: the
partial file is deleted (grid_launcher/background/workers.py:81) and `finished` carries
that message as `error`. The UI translates the message into the `cancelled` entry status
rather than showing a failure dialog (grid_launcher/ui/mixins/install_mixin.py:1548).
Desktop entry point: `_cancel_download_entry` (grid_launcher/ui/mixins/install_mixin.py:1898),
which sets the entry to `cancelling` and waits for the worker to notice. TV entry point:
`cancelInstall` (grid_launcher/tv/bridge/game_backend.py:706).

If the download being cancelled is only *queued*, not active, it is removed from
`install_queue` synchronously on the UI thread and marked `cancelled` immediately
(grid_launcher/ui/mixins/install_mixin.py:1903).

**(b) Supersede-and-drop — request-id workers.** `DetailsCloudRecordsWorker`,
`RetroAchievementsWorker`, `PCGamingWikiWorker`, and `RomDetailWorker` are never actually
stopped. A monotonically increasing request id is stamped at start time
(grid_launcher/ui/mixins/details_view_mixin.py:798) and the completion handler discards any
result whose id no longer matches the current one
(grid_launcher/ui/mixins/details_view_mixin.py:844). `RomDetailWorker` compares the ROM id
against the currently open details game instead (grid_launcher/ui/mixins/details_view_mixin.py:237).
`_on_details_cloud_records_loaded` additionally drops results when the panel has switched
between save and state mode (grid_launcher/ui/mixins/details_view_mixin.py:848).

**(c) Quit-and-wait — TV backends.** `CloudBackend` stops the previous fetch or upload
before starting a new one by asking the thread to quit and waiting up to 2000 ms
(grid_launcher/tv/bridge/cloud_backend.py:277, :284). Because the workers do no
interruptible looping, "quit" only takes effect once `run()` returns; the bounded wait
means the UI never blocks longer than 2 s but the old worker may still be running after
the wait expires. `GameBackend` does the same for restore with a 500 ms bound
(grid_launcher/tv/bridge/game_backend.py:313).

**(d) Single-flight guards — no cancellation at all.** Most fetch workers simply refuse to
start a second instance: the Discover loader (grid-launcher.py:1239), the missing-cover
replenisher (grid-launcher.py:2847), the source version check
(grid_launcher/ui/mixins/emulator_ui_mixin.py:1273), and every TV app-backend fetch via
`_is_thread_running` (grid_launcher/tv/bridge/app_backend.py:12). The TV ROM-metadata
fetch is single-flight *per ROM id* (grid_launcher/tv/bridge/app_backend.py:374), and the TV
ROM-list fetch is single-flight *per platform label*
(grid_launcher/tv/bridge/app_backend.py:640).

**(e) Batch cancellation — TV cover loads.** Image loads are tagged with a batch id
obtained from `create_batch` (grid_launcher/tv/widgets/cover_loader.py:64). When a view
replaces its contents it calls `cancel_batch` for the old id
(grid_launcher/tv/widgets/components/game_wall.py:67) and creates a new one
(grid_launcher/tv/widgets/components/game_wall.py:68). The network fetch still completes;
only the UI-thread delivery is skipped (grid_launcher/tv/widgets/cover_loader.py:96).
Cancelled batch ids are never removed from the cancelled set.

### 4. The install pipeline as a state machine

The desktop install path is the only place where two workers are chained and where a queue
exists. Payload details are in doc 03; the concurrency shape is:

1. At most one download and at most one finalize may be in flight, tracked by the two
   booleans `install_in_progress` and `install_finalize_in_progress` (grid-launcher.py:441,
   grid-launcher.py:446). If either is set when a new install is requested, the game is
   appended to `install_queue` with a `queued` entry status and the call returns
   (grid_launcher/ui/mixins/install_mixin.py:1349).
2. Duplicate suppression: a request whose key equals the pending key or is already queued
   is dropped silently (grid_launcher/ui/mixins/install_mixin.py:1347).
3. `_on_async_install_finished` clears the download flags first, then either fails the
   entry or starts the finalize worker (grid_launcher/ui/mixins/install_mixin.py:1515).
4. `_on_async_install_finalize_finished` clears the finalize flags and, on every exit path,
   calls `_start_next_queued_install` (grid_launcher/ui/mixins/install_mixin.py:1718), which
   pops the head of the queue only when both flags are clear
   (grid_launcher/ui/mixins/details_view_mixin.py:336).

TV mode has no queue. It rejects a concurrent install outright with the message
`An install is already in progress.` (grid_launcher/tv/bridge/game_backend.py:503), where
"active" means either the download thread is running or the finalize thread is alive
(grid_launcher/tv/bridge/game_backend.py:192).

### 5. Nested concurrency inside `DiscoverLoadWorker`

`DiscoverLoadWorker` is the only worker that spawns its own pool. It runs on an event-loop
thread but internally opens a 12-slot task pool
(grid_launcher/background/workers.py:1049) and submits: new games, highly rated, optionally
recommendations (only when the local library has 20+ games,
grid_launcher/background/workers.py:1054), platform sections, up to 6 per-genre fetches
(grid_launcher/background/workers.py:1061), and genre totals for up to 15 genres
(grid_launcher/background/workers.py:1065).

Ordering and gating:

- One serial fetch runs first and gates everything: `short_games` also returns the genre
  list, so per-genre fetches cannot be submitted until it completes
  (grid_launcher/background/workers.py:1034).
- Results are harvested by awaiting each task individually; any task that raises is
  skipped and contributes nothing to the result
  (grid_launcher/background/workers.py:1073, :1080, :1092, :1107).
- The pool is fully drained before the worker publishes anything; there is exactly one
  `finished` event carrying the whole aggregate.
- The `error` event fires only if the aggregate is empty *and* the gating fetch failed
  (grid_launcher/background/workers.py:1112). A partial failure is therefore invisible to
  the UI.

Every pool task reads and writes the shared `DiscoverCache`, which is why that cache is the
one place in the codebase with an explicit mutex (grid_launcher/server/discover.py:24).

### 6. Session tracking and post-session upload

Game sessions are not watched by a thread on desktop. A 2.5 s repeating timer partitions
`active_game_sessions` into still-running and finished
(grid_launcher/ui/mixins/cloud_mixin.py:2847). For each finished session the handler updates
sync state, then — if auto-upload is enabled and credentials exist — schedules
`_auto_upload_after_session` after the configured delay
(grid_launcher/ui/mixins/cloud_mixin.py:2879), which computes the upload plan on the UI
thread and only then starts `AutoCloudSaveUploadWorker`.

TV mode instead uses a dedicated blocking watcher thread per session
(grid_launcher/tv/bridge/game_backend.py:77) that waits on the process handle and publishes
one exit event.

### 7. Shutdown sequence

There is no global "drain all workers" step. What exists:

- `MainWindow.closeEvent` (grid-launcher.py:622) persists window geometry, hides the TV
  window and pause window, stops the TV game session if one is active, and stops the
  controller backend (grid-launcher.py:634), which stops the poll loop and waits up to
  500 ms for it (grid_launcher/tv/bridge/controller.py:386).
- The application's `aboutToQuit` event closes only the single-instance socket
  (grid-launcher.py:3781).
- No `closeEvent` code touches `install_thread`, `install_finalize_thread`,
  `auto_cloud_upload_threads`, `details_cloud_threads`, `_discover_load_thread`,
  `_ra_thread`, `_pcgw_thread`, `_rom_detail_thread`, or `_cover_replenish_thread`.
- Every plain thread in the codebase is created as a daemon thread, so the process does not
  wait for them (for example grid-launcher.py:3122,
  grid_launcher/ui/mixins/emulator_ui_mixin.py:1787,
  grid_launcher/tv/bridge/app_backend.py:622,
  grid_launcher/tv/widgets/cover_loader.py:120,
  grid_launcher/library/archive_preparation.py:190).

The only blocking waits performed anywhere are: Discover thread quit + unbounded wait in
its two completion handlers (grid-launcher.py:1267 and grid-launcher.py:1431), the TV
download thread quit + 2000 ms wait (grid_launcher/tv/bridge/game_backend.py:575), the TV
process-watch join (grid_launcher/tv/bridge/game_backend.py:812), the TV cloud
fetch/upload 2000 ms waits (grid_launcher/tv/bridge/cloud_backend.py:280, :287), the TV
restore 500 ms wait (grid_launcher/tv/bridge/game_backend.py:315), and the controller
500 ms waits (grid_launcher/tv/bridge/controller.py:388, :394).

Practical consequence for a port: closing the window during an install kills the in-flight
download abruptly. Partial archive files are removed only by the worker's own error path,
which does not run in this case. The launcher does warn before *switching to TV mode*
during a download (grid-launcher.py:639) but not before closing.

---

## Invariants and error handling

1. **Workers never propagate exceptions across the thread boundary.** Every `run()` wraps
   its body in a catch and converts the failure into a normal completion event with a
   populated `error` field. Some workers catch a specific set — for example
   `AutoCloudSaveUploadWorker` catches HTTP/URL/OS/value/JSON errors
   (grid_launcher/background/workers.py:694) and `DetailsCloudRecordsWorker` the same set
   (grid_launcher/background/workers.py:750) — while others catch everything
   (`SourceVersionCheckWorker` at grid_launcher/background/workers.py:500, `RomDetailWorker`
   at grid_launcher/background/workers.py:775, `PCGamingWikiWorker` at
   grid_launcher/background/workers.py:832, all TV workers). A port must decide per worker;
   the narrow-catch workers will crash their host thread on an unexpected exception type.
2. **`error == ""` means success.** Every dict payload that has an `error` key uses the
   empty string for success. Handlers check truthiness of `error` first.
3. **A completion event fires exactly once per worker**, on every path including error
   paths. The one exception is `MissingCoverReplenishWorker`, which fires `game_cover_cached`
   zero-or-more times and then exactly one `finished`
   (grid_launcher/background/workers.py:880, :882).
4. **Partial download files are deleted by the worker itself** before it reports the error,
   on both the HTTP-error and the general-error path
   (grid_launcher/background/workers.py:72, :81).
5. **Stale results must be dropped, not applied.** Any worker family that can have two
   generations in flight carries a request id or an identity key and its handler returns
   early on mismatch (grid_launcher/ui/mixins/details_view_mixin.py:844,
   grid_launcher/ui/mixins/details_view_mixin.py:237,
   grid_launcher/ui/mixins/details_view_mixin.py:1953).
6. **Handlers must tolerate a destroyed UI.** `_on_details_cloud_records_loaded` returns
   early when its target labels/layouts are gone
   (grid_launcher/ui/mixins/details_view_mixin.py:832). `CoverLoader`'s delivery swallows
   "object already deleted" runtime errors and re-raises anything else
   (grid_launcher/tv/widgets/cover_loader.py:112).
7. **The "without_ui" naming convention marks thread-safe entry points.** Workers that call
   back into the main window only call methods whose names end in `_without_ui`
   (grid_launcher/background/workers.py:549, :555, :561, :568, :603, :615, :633). Those
   methods do no widget access. `AutoCloudSaveUploadWorker` achieves the same by passing
   `show_dialogs=False`, which is the flag that suppresses the modal message boxes inside
   `_upload_cloud_files_for_game` (grid_launcher/ui/mixins/cloud_mixin.py:2436).
8. **Everything a worker needs is captured on the UI thread before start.** Base URL, token,
   headers, cache directory, and game snapshots are read and copied at construction time
   (grid-launcher.py:3094, grid_launcher/tv/bridge/game_backend.py:594, and the `dict(...)`
   copies at grid_launcher/background/workers.py:48, :661, :718).
9. **A worker's progress rate is capped, not its count.** Download progress is emitted at
   most every 0.1 s (grid_launcher/background/workers.py:110). Install-finalize progress is
   emitted at whatever rate the extraction callback fires
   (grid_launcher/background/workers.py:646).
10. **Server connection itself is synchronous.** `_connect_to_server` performs its HTTP
    calls inline on the UI thread (grid-launcher.py:3033); only the follow-on cover
    replenishment is backgrounded. TV mode does the equivalent work on a worker
    (`CatalogFetchWorker`).

---

## Platform differences

| Area | Windows | Linux / macOS |
| --- | --- | --- |
| Controller polling | `_XInputPollThread`, which reads XInput including the guide button (grid_launcher/tv/bridge/controller.py:63, selected at grid_launcher/tv/bridge/controller.py:367) | `_GamepadPollThread`, backed by pygame joysticks (grid_launcher/tv/bridge/controller.py:227) |
| Archive deletion | Antivirus scanners hold locks, so a failed delete spawns the retry thread (grid_launcher/library/archive_preparation.py:190) | Same code path, but the retry rarely triggers |
| Emulator asset resolution inside the download worker | A Windows-specific asset matcher runs first and can raise if the configured Windows asset specs match nothing (grid_launcher/background/workers.py:327, :380) | Returns immediately without matching (grid_launcher/background/workers.py:327) |
| Wine prefix creation in the finalize worker | Not performed | For native-executable platforms, the finalize worker creates a `prefix` directory and records it on the prepared game (grid_launcher/background/workers.py:578) |
| Windows target architecture | Chosen from explicit metadata or the machine architecture, defaulting to x64 (grid_launcher/background/workers.py:393) | Not consulted |

Nothing else in the concurrency layer branches on platform. Thread counts, timer
intervals, and cancellation semantics are identical everywhere.

---

## Concurrency

### Thread-affinity map

**UI thread only — never touched from a worker:**

- All widgets, layouts, dialogs, and the details/downloads/discover page objects.
- `download_entries` and all download-entry status transitions
  (grid_launcher/ui/mixins/install_mixin.py:1363 and the `_on_async_*` handlers).
- `install_queue`, `install_in_progress`, `install_finalize_in_progress`,
  `install_pending_game`, `install_finalize_game` (grid-launcher.py:441).
- `active_download_*` counters (grid-launcher.py:470).
- `library_games` mutation and `config` writes. `_save_config` is only ever called from
  UI-thread handlers, for example after cover replenishment finishes (grid-launcher.py:2888).
- `cover_cache`, `cover_waiters`, `cover_loading` — the desktop cover pipeline is
  asynchronous but single-threaded, driven by the network manager's completion callback on
  the UI thread (grid_launcher/cover/loader.py:57, :64).
- Worker/thread tracking handles: `install_thread`, `_ra_thread`, `_pcgw_thread`,
  `_rom_detail_thread`, `_cover_replenish_thread`, `_discover_load_thread`,
  `auto_cloud_upload_threads`, `details_cloud_threads` (grid-launcher.py:410–509).

**Crosses threads by value (copied at construction, read-only in the worker):**

- Base URL, API token, auth headers, archive path, image cache directory.
- Game snapshots — `dict(game)` copies at grid_launcher/background/workers.py:661 and :718.
- Source metadata dicts — `dict(source_metadata)` at grid_launcher/background/workers.py:50.
- The captured base URL closed over by the TV ROM workers
  (grid_launcher/tv/bridge/workers.py:53).

**Crosses threads by reference, guarded:**

- `DiscoverCache.cache` — read/written from up to 12 pool tasks concurrently; guarded by a
  mutex in `get_section`, `set_section`, `invalidate_section`, and `is_stale`
  (grid_launcher/server/discover.py:24, :60, :77, :89, :101).
- `CoverLoader._cache`, `_next_batch_id`, `_cancelled_batches` — read/written from one
  thread per image load; guarded by a mutex
  (grid_launcher/tv/widgets/cover_loader.py:55, :66, :73, :94, :127).

**Crosses threads by reference, unguarded (see Open questions):**

- `MainWindow` itself is passed into `InstallFinalizeWorker`,
  `AutoCloudSaveUploadWorker`, and `DetailsCloudRecordsWorker`
  (grid_launcher/background/workers.py:534, :655, :716). The workers call back into it from
  the background thread.
- `_platform_games_results` — written by each platform-fetch thread, popped by the UI
  thread (grid-launcher.py:3117, grid-launcher.py:3125).
- `_platform_default_emulator_cache` and `_platform_available_emulator_cache` — populated by
  the warm-up thread (grid_launcher/ui/mixins/emulator_ui_mixin.py:917) and read by UI code.
- `DiscoverCache.clear`, `save_to_disk`, and `load_from_disk` operate on `self.cache`
  without taking the mutex (grid_launcher/server/discover.py:107, :111, :124).
- `config` dict shared into the TV backends by reference
  (grid_launcher/tv/bridge/game_backend.py:151, grid_launcher/tv/bridge/cloud_backend.py:115)
  and read inside worker threads (grid_launcher/tv/bridge/cloud_backend.py:45).
- `_rom_meta_threads` / `_rom_meta_workers` dicts, mutated from UI-thread slots only, but
  the worker's completion handler deletes entries while other starts may be in flight
  (grid_launcher/tv/bridge/app_backend.py:109).

### Degree of parallelism

| Scope | Max concurrent background tasks |
| --- | --- |
| Downloads (desktop) | 1, plus 1 finalize; extra requests are queued |
| Downloads (TV) | 1; extra requests are rejected |
| Discover load | 1 worker + 12 pool tasks |
| Details panel lookups | Unbounded in principle (cloud records, ROM detail, achievements, wiki paths can all be in flight), but each family is at most one useful generation |
| Auto cloud uploads | Unbounded — one per finished session, no cap (grid_launcher/ui/mixins/cloud_mixin.py:2950) |
| TV catalog/curated fetches | 1 each per kind; ROM list 1 per platform; ROM metadata 1 per ROM id |
| TV cover images | Unbounded — one thread per requested image (grid_launcher/tv/widgets/cover_loader.py:120) |
| Server platform ROM fetch | 1 per platform label, unbounded across labels |

---

## Test oracle

| Test file | What it pins down |
| --- | --- |
| `tests/test_background_workers.py` | The desktop worker contracts, exercised by calling `run()` directly on the test thread and collecting emitted payloads. Covers `DetailsCloudRecordsWorker` success and error payload shape (tests/test_background_workers.py:42, :56); `InstallDownloadWorker` HTTP-error detail text, debug logging, partial-file cleanup on failure, and roughly 30 emulator-source asset-resolution cases (tests/test_background_workers.py:89, :116, :195); `InstallFinalizeWorker` cleanup ordering and Linux prefix placement (tests/test_background_workers.py:1374, :1444); `RetroAchievementsWorker`, `PCGamingWikiWorker`, and `MissingCoverReplenishWorker` emission behavior including the skip-and-continue paths (tests/test_background_workers.py:1547, :1587, :1616) |
| `tests/test_rom_detail_worker.py` | `RomDetailWorker` payload on success and on exception, plus the trigger condition and the field-merge applied by the handler (tests/test_rom_detail_worker.py:37, :57, :78, :96) |
| `tests/test_source_version_check.py` | `SourceVersionCheckWorker` per provider (github latest, github pinned tag, gitea, direct-with-no-network) and its error payloads (tests/test_source_version_check.py:19, :41, :65, :87, :107, :126, :152) |
| `tests/test_discover.py` | `DiscoverCache` TTL, invalidation, disk round-trip, and explicitly the mutex behavior: 50 writes across 8 concurrent tasks must all survive (tests/test_discover.py:87) |
| `tests/test_game_wall_batching.py` | Cover-batch cancellation: replacing the wall's contents must cancel the previous batch id and create a new one (tests/test_game_wall_batching.py:53) |
| `tests/test_tv_game_backend.py` | TV install concurrency rules — start is rejected while a thread is running or while a target game is still set (tests/test_tv_game_backend.py:1075, :1100); the finalize step must be a plain thread stored on the backend (tests/test_tv_game_backend.py:752); process-watch thread construction (tests/test_tv_game_backend.py:307) |
| `tests/test_tv_app_backend.py` | That TV fetches use plain threads, are single-flight, and store their handles (tests/test_tv_app_backend.py:863, :897, :987, :1019) |
| `tests/test_tv_cloud_backend.py` | TV cloud fetch/upload start behavior, verified by patching thread start (tests/test_tv_cloud_backend.py:48, :278, :292, :448) |
| `tests/test_tv_controller_backend.py` | Poll-thread construction and the install-active throttle callable defaults (tests/test_tv_controller_backend.py:294, :308, :314) |
| `tests/test_emulator_install_subfolder.py` | Emulator install start path, verified by patching the thread class in the install mixin (tests/test_emulator_install_subfolder.py:65) |
| `tests/test_emulator_autoconfig_settings.py` | The firmware background threads: that one is spawned, that it is skipped when there are no firmware directories, and what the thread body does when run inline (tests/test_emulator_autoconfig_settings.py:2224, :2266, :2324) |

The dominant testing technique — and the strongest hint for a port's own testability — is
that every worker's `run()` is callable synchronously with no thread at all. Keep the work
function free of any dependency on its host thread.

---

## Open questions

1. **`MainWindow` is passed into three workers and called from their threads.**
   `InstallFinalizeWorker`, `AutoCloudSaveUploadWorker`, and `DetailsCloudRecordsWorker`
   hold a live reference (grid_launcher/background/workers.py:534, :655, :716). The
   convention is that only `*_without_ui` methods and `show_dialogs=False` paths are called,
   but nothing enforces it, and those methods still read mutable window state such as
   `self.config` and `self.library_games`. It is unclear which of those reads are intended
   to be safe. A port should decide whether to pass an immutable snapshot instead.
2. **`_platform_games_results` relies on interpreter-level atomicity.** The staging dict is
   written from a background thread with the in-code note that "the GIL makes the dict write
   safe" (grid-launcher.py:3117). This does not carry over to a port with real parallelism,
   and it is not clear whether concurrent fetches for different platforms were ever
   considered.
3. **`DiscoverCache.clear`, `save_to_disk`, and `load_from_disk` skip the mutex** that the
   other four methods take (grid_launcher/server/discover.py:107, :111, :124). Whether this
   is deliberate (they are documented as UI-thread-only operations) is not stated anywhere.
4. **Cancelled cover batch ids accumulate without bound.** `_cancelled_batches` is only ever
   added to (grid_launcher/tv/widgets/cover_loader.py:74); nothing prunes it. For a
   long-running TV session this set grows with every view rebuild.
5. **No worker is drained on shutdown.** `closeEvent` (grid-launcher.py:622) stops only the
   controller and the TV session. An in-flight download, finalize, or cloud upload is
   abandoned mid-operation, and a finalize abandoned mid-extraction leaves a partially
   extracted directory that nothing cleans up. Whether this is accepted risk or an
   oversight is not recorded.
6. **`_source_check_thread` is read via `getattr` with a default**
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:1265) rather than being initialized in the
   constructor like every other thread handle, so the first call takes a different code
   path from subsequent ones.
7. **`AutoCloudSaveUploadWorker` has no concurrency cap.** Ending several sessions in quick
   succession starts one upload per session with no serialization
   (grid_launcher/ui/mixins/cloud_mixin.py:2950). It is unclear whether two uploads for the
   same game and save type can interleave.
8. **The TV `_finalizeResult` payload is JSON-encoded to cross threads**
   (grid_launcher/tv/bridge/game_backend.py:651) while every other worker passes a plain
   dictionary. The reason for the difference is not documented.
9. **`DiscoverLoadWorker` reports success on partial failure.** If the gating fetch succeeds
   but every pool task fails, the aggregate is non-empty and the `error` event never fires
   (grid_launcher/background/workers.py:1112). Whether the UI should surface partial failure
   is undecided.
10. **Debug output is printed from worker threads.** `SourceVersionCheckWorker` prints
    unconditionally (grid_launcher/background/workers.py:451) and the download worker prints
    when debug is enabled (grid_launcher/background/workers.py:63). Interleaving from
    multiple threads is not managed.

---

## Source map

| File | Role in the concurrency architecture |
| --- | --- |
| `grid_launcher/background/workers.py` | All eleven desktop worker classes. Pure work functions plus completion events; no widget access |
| `grid_launcher/tv/bridge/workers.py` | Nine TV fetch/mutation workers, all plain request-response |
| `grid_launcher/tv/bridge/app_backend.py` | Owner of every TV catalog/curated/metadata fetch; the plain-daemon-thread hosting style and the `_is_thread_running` single-flight helper (grid_launcher/tv/bridge/app_backend.py:12) |
| `grid_launcher/tv/bridge/game_backend.py` | TV install download + finalize chain, process watch thread, auto-restore and auto-upload workers, the `isInstallActive` gate |
| `grid_launcher/tv/bridge/cloud_backend.py` | TV slot fetch and upload workers with quit-and-wait cancellation |
| `grid_launcher/tv/bridge/cloud_helpers.py` | `_TvAutoRestoreWorker` and the shared TV upload routine used by two workers |
| `grid_launcher/tv/bridge/controller.py` | The two platform-specific poll threads and their start/stop lifecycle (doc 09) |
| `grid_launcher/ui/mixins/install_mixin.py` | Desktop install worker wiring, the install queue, download cancellation entry point |
| `grid_launcher/ui/mixins/details_view_mixin.py` | Details-page worker wiring; the request-id staleness pattern; the queued-install pump |
| `grid_launcher/ui/mixins/cloud_mixin.py` | Session polling, delayed auto-upload scheduling, `AutoCloudSaveUploadWorker` wiring |
| `grid_launcher/ui/mixins/emulator_ui_mixin.py` | Source version check worker; the three fire-and-forget daemon threads (cache warm-up, firmware download, firmware install) |
| `grid-launcher.py` | Thread handle declarations, the polling timers, Discover worker wiring, cover replenishment, the ad-hoc platform ROM fetch, `closeEvent` |
| `grid_launcher/server/discover.py` | `DiscoverCache` — the only mutex-guarded shared state reached from a task pool |
| `grid_launcher/tv/widgets/cover_loader.py` | Per-image threads, mutex-guarded cache, batch cancellation, UI-queue delivery hop |
| `grid_launcher/tv/widgets/components/game_wall.py` | Consumer of cover batch cancellation |
| `grid_launcher/library/archive_preparation.py` | Two internal threads: the delete-retry thread and the 7-Zip extraction thread polled for progress |
| `grid_launcher/cover/loader.py`, `grid_launcher/cover/manager.py` | Desktop cover loading — asynchronous but entirely UI-thread; useful as the contrast case |
