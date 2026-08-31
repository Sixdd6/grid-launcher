# GRID Launcher Module Map

This document is a stable module map for the current codebase.

- Use `SPEC.md` for product behavior and UX intent.
- Use `openapi.json` as a single source of truth for all api calls to the server.

## Runtime Entry
- `grid-launcher.py`
  - Application entry point and `MainWindow` orchestration shell.
  - Wires together UI interactions, background jobs, and domain helpers.
  - Builds the page stack and nav bar (Library, Server, Discover, Downloads, Emulators, Settings, plus the Game Details page) and switches into TV mode.
  - The Game Details cloud panel, shared-save scope handling, the handoff from `Details` to `Manage Saves` / `Emulator Saves` / `Manage States`, and screenshot directory resolution live in the mixins below — check those first.

## Package Map
- `grid_launcher/core/`
  - Shared low-level helpers used across the app.
  - `api.py`: authenticated HTTP and multipart helpers.
  - `config.py`: config normalization, merging, and persistence.
  - `path.py`: path sanitization and containment helpers; also provides XDG base-directory helpers (`xdg_config_home`, `xdg_data_home`).
  - `process.py`: `clean_subprocess_env()` — restores the loader environment PyInstaller overrode (`LD_LIBRARY_PATH` / `LD_LIBRARY_PATH_ORIG`) so host binaries such as 7z, tar, and emulators spawned from the frozen build resolve their own system libraries.
  - `token_store.py`: keyring-backed secret persistence (OS Credential Manager / Keychain / Secret Service) for the API token, RetroAchievements token, and RetroAchievements Web API key, with DPAPI-encrypted file storage as a Windows-only fallback when the keyring backend is unavailable, and automatic migration from the legacy per-platform file format.
  - `types.py`: shared protocol typing.

- `grid_launcher/server/`
  - Server connection, catalog loading, cached details, and server-page helpers.
  - `catalog.py`: platform mapping and ROM pagination transforms.
  - `connection.py`: connection failure classification.
  - `details_cache.py`: ROM detail lookup and ROM-id caching.
  - `discover.py`: Discover tab data layer — `DiscoverCache` (TTL cache with disk persistence), section fetchers (short games, new games, highly rated, recommendations, per-genre, per-platform), genre totals/stats, installed/client-side filtering, watchlist load/save, and discover event recording.
  - `metadata.py`: metadata source priority (LaunchBox, ScreenScraper, IGDB, MobyGames) plus description/genre/rating/release-date extraction and rating normalization to a five-point scale.
  - `orchestrator.py`: coordinated server fetch flow.
  - `pcgamingwiki.py`: PCGamingWiki API client — page lookup, wikitext parsing, and expansion of Windows save-path templates into `%APPDATA%`-style paths.
  - `platform_metadata.py`: static per-slug platform metadata (manufacturer, release year, player count) and platform logo file resolution.
  - `state.py`: credential, base URL, and identity helpers.
  - `status.py`: server status presentation.
  - `view.py`: server-page selection, search, and render helpers.
  - `retroachievements.py`: RetroAchievements Web API client - achievement fetching and RA game ID resolution.

- `grid_launcher/library/`
  - Install, uninstall, archive prep, identity, downloads, and cloud-save behavior.
  - `archive_preparation.py`: extraction and install-prep flow.
  - `cloud_restore.py`: restore record and target selection, including slot-aware restore grouping.
  - `cloud_sync.py`: sync state normalization, shared-save discovery, Redream hash-based savestate matching, and session filtering.
  - `cloud_transfer.py`: upload/restore archive transfer utilities, including session-window screenshot attachment for emulators that save screenshots to a dedicated directory.
  - `cloud_upload.py`: upload planning and result messaging.
  - `downloads.py`: download status and detail formatting.
  - `identity.py`: game key and installed-record lookup helpers.
  - `install_cleanup.py`: uninstall orchestration and file cleanup.
  - `firmware_install.py`: RetroArch and emulator firmware download, routing, and extraction. Supports keyword-filtered routing, MAME-format zip preservation, flat and path-preserving zip extraction, and debug logging via `grid_launcher.library.firmware_install` logger.
  - `install_metadata.py`: install-time metadata hydration.
  - `install_paths.py`: archive, extracted, and native path resolution.
  - `install_registry.py`: installed-game record construction and matching.
  - `install_state.py`: queue, pending, and progress state helpers.
  - `ps3_install.py`: PS3 content classification, VFS routing, and game-ID helpers.
  - `update_detection.py`: decides whether an installed game has a newer server copy — server timestamp extraction, `(vNNNNN)` / semver ROM filename version comparison, and Windows-PC platform special-casing.

- `grid_launcher/emulator/`
  - Emulator selection, profiles, auto-configuration, RetroArch integration, and launching.
  - `autoconfig.py`: known emulator auto-configuration.
  - `launch.py`: launch argument substitution and command preparation. `retroarch_core_argument_path()` branches on platform, emitting `.so` on Linux, `.dylib` on macOS, and `.dll` on Windows; it strips any existing known core extension before applying the platform-appropriate one.
  - `cemu.py`: Cemu portable-mode settings and controller profile writing. `ensure_cemu_controller_config()` writes the XInput Pro Controller profile (`_DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE`) on Windows and the SDL controller profile (`_DEFAULT_CEMU_SDL_CONTROLLER_PROFILE`, using `<api>SDLController</api>`) on all other platforms.
  - `xenia.py`: Xenia variant detection and directory resolution. Supports master, Canary, and Edge variants; `_is_edge_variant()` plus edge-aware `_config_name_candidates()`/`xenia_directory_settings()` map the Edge build (variant `edge`) to `xenia-edge.config.toml` and the `cache_host` cache directory.
  - `profiles.py`: emulator profile defaults and matching, plus `is_available_on_current_platform()` — the platform gate that hides Windows-only autoprofiles (`_WINDOWS_ONLY_EMULATOR_SLUGS`: Xenia master/Canary, the ShadPS4 Qt launcher) and honours an explicit `source.platforms` allowlist.
  - `retroarch.py`: RetroArch core discovery and compatibility mapping.
  - `selection.py`: default emulator and platform resolution, including cloud save scope classification such as per-game vs shared emulator media.
  - `source.py`: emulator auto-install source resolution — normalizes an autoprofile `source` block, picks a GitHub release, and matches a download asset by filename pattern.
  - `wine.py`: `translate_windows_path_to_wine_prefix()` — maps a Windows env-var save path (`%APPDATA%`, `%USERPROFILE%`, `%LOCALAPPDATA%`, ...) to its location inside a Wine prefix, used by `cloud_transfer.py` for native-game saves on Linux. The Wine / umu-run (Proton) launch dispatch itself lives in `launch.py`.
  - Per-emulator config modules — `azahar.py`, `dolphin.py`, `duckstation.py`, `eden.py`, `fbneo.py`, `mame.py`, `pcsx2.py`, `pico8.py`, `ppsspp.py`, `redream.py`, `rpcs3.py`, `vita3k.py`, `xemu.py`: config/user-root path candidates per platform, `ensure_*_settings()` autoconfig writers where the emulator has a writable config, and the save/state directory overrides cloud sync uses. `cemu.py` and `xenia.py` are listed separately above because their behavior is not purely path resolution.

- `grid_launcher/cover/`
  - Cover parsing, caching, loading, and details-view media helpers.
  - `cache.py`: cover cache persistence and fallback save behavior.
  - `details.py`: details-view cover and screenshot refresh helpers.
  - `loader.py`: async image loading and application.
  - `manager.py`: queueing and cache cleanup wrappers.
  - `utils.py`: URL normalization and cache-key helpers.

- `grid_launcher/ui/`
  - UI-specific dialogs, views, theming, widget helpers, and `MainWindow` mixin behavior.
  - `dialogs.py`: `FirstRunSetupDialog`, `NativeGameSettingsDialog` (which also carries the non-Windows compatibility-tool picker), and `EmulatorConfigDialog`.
  - `discover.py`: Discover page widgets — `DiscoverPageWidget` plus `DiscoverCarouselSection`, `DiscoverGenreSection`, and `DiscoverFilterPanel`.
  - `downloads.py`: downloads page/widget construction.
  - `emulators.py`: emulator settings form helpers.
  - `game_views.py`: library cards and details-view UI helpers, including cloud button visibility/label updates like `Emulator Saves`.
  - `spinner.py`: `LoadingSpinnerWidget`, an animated arc overlay parented to whatever widget it should cover.
  - `theme.py`: theme selection and stylesheet generation.
  - `toast.py`: `ToastWidget` and `show_toast` helper for transient in-window notifications.

- `grid_launcher/ui/mixins/`
  - `MainWindow` behavior extracted into composable mixins. `MainWindow` inherits all four in MRO order: `CloudSaveMixin`, `EmulatorUIMixin`, `InstallMixin`, `DetailsViewMixin`.
  - `cloud_mixin.py` (`CloudSaveMixin`): cloud save orchestration — save-scope classification, block-reason resolution, sync candidate discovery (per-game and shared-emulator paths for all emulators), emulator-specific save-directory overrides (Cemu, Dolphin, PCSX2, RPCS3, etc.), session-window filtering, screenshot and firmware directory resolution, and upload/restore coordinator helpers.
  - `emulator_ui_mixin.py` (`EmulatorUIMixin`): emulator settings page behavior — emulator config normalization, autoprofile loading, RetroArch core list and compatibility map access, emulator view refresh, emulator add/edit/remove/save form actions, source-download emulator install flow, RPCS3 firmware background download trigger, and emulator path/library browsing.
  - `install_mixin.py` (`InstallMixin`): game install/uninstall lifecycle — async download and finalize workers, archive extraction, PS4/Xbox 360 content install flows, firmware routing post-install, native game update application, installed-game registration, update-state refresh, and library path resolution.
  - `details_view_mixin.py` (`DetailsViewMixin`): game details panel — opening/closing the details view, cloud panel rendering (records, upload, restore, delete), details-view responsive layout (responsive resize), PCGamingWiki save-path lookup, native save-path section rendering, cloud sync state accessors, ROM ID caching, install-queue integration, and the async `DetailsCloudRecordsWorker` lifecycle.

- `grid_launcher/background/`
  - Threaded background workers for downloads, installs, cloud uploads, async details-panel cloud record loading, Discover section loading, and cover backfill.
  - `workers.py`: worker implementations such as install/download workers (`InstallDownloadWorker`, `InstallFinalizeWorker`), `SourceVersionCheckWorker`, auto cloud upload workers, the async details cloud-record fetch worker used to keep the Details view responsive, `RomDetailWorker`, `RetroAchievementsWorker` / `RALoginWorker`, `PCGamingWikiWorker`, `MissingCoverReplenishWorker` (fetches and caches covers for library entries missing artwork), and `DiscoverLoadWorker` (loads Discover tab sections from the server off the UI thread).

- `grid_launcher/tv/`
  - TV mode: a separate fullscreen, controller-driven QWidget UI. It shares the domain packages above (`core/`, `library/`, `emulator/`, `server/`) but has its own state backends and widgets — it does not reuse `MainWindow` or `grid_launcher/ui/`. `grid-launcher.py` constructs the backends and `TVWindow` when switching into TV mode.

- `grid_launcher/tv/bridge/`
  - Backend objects that own TV-mode state and threaded work; views read from them and connect to their signals.
  - `app_backend.py`: `AppBackend` — config, library games, platforms, server games, connection status, favorites / new additions / highly rated rows, exclusion lists, per-ROM metadata fetch, and the desktop-mode-switch and quit requests. Owns `_RomMetaFetchWorker`.
  - `game_backend.py`: `GameBackend` — launch, session lifecycle (`sessionStarted`/`sessionEnded`/pause/resume), install and uninstall progress, native-executable picking, and cloud sync status. Owns `_ProcessWatchThread` and `_TvAutoUploadWorker`.
  - `cloud_backend.py`: `CloudBackend` — cloud save/state slot listing, restore, delete, and upload for the TV cloud saves overlay. Owns `_SlotFetchWorker` and `_CloudUploadWorker`.
  - `cloud_helpers.py`: shared cloud-save resolution for TV mode — game save-match tokens, state-file detection, emulator entry lookup, per-emulator save/state directory resolution, `perform_tv_save_upload()`, and `_TvAutoRestoreWorker` (pre-launch restore).
  - `pause_backend.py`: `PauseBackend` — visibility, game title, and emulator name for the fullscreen pause overlay.
  - `controller.py`: `ControllerBackend` and its poll threads — `_XInputPollThread` (Windows XInput) and `_GamepadPollThread` (pygame/SDL elsewhere), translating gamepad input into navigation and button signals.
  - `workers.py`: threaded server fetch workers — `CatalogFetchWorker`, `RomListFetchWorker`, `FavoritesRomFetchWorker`, `NewAdditionsRomFetchWorker`, `HighlyRatedRomFetchWorker`, `SavesBatchFetchWorker`, and the collections workers (`CollectionsFetchWorker`, `CollectionUpdateWorker`, `CollectionCreateWorker`).

- `grid_launcher/tv/widgets/`
  - The TV-mode widget tree.
  - `window.py`: `TVWindow` — outer/inner `QStackedWidget` shell, tab switching, view push/pop with slide transitions, controls-bar updates, and keyboard/controller event routing.
  - `pause_window.py`: `PauseWindow` — the always-on-top fullscreen pause overlay with its own control hints.
  - `cover_loader.py`: `CoverLoader` — threaded cover download with an on-disk cache keyed by URL hash, delivering `QPixmap`s back on the Qt thread.
  - `theme.py`: TV-mode colour constants (background, panel, accent, text, status colours).
  - `tab_bar.py`: `ViewTabBar` — the Home / Library / Server tab strip and its `tabChanged` signal.

- `grid_launcher/tv/widgets/views/`
  - `home_view.py`: `HomeView` — the Continue Playing / Favorites / New Additions / Highly Rated rows, with per-row debounced refresh.
  - `library_view.py`: `LibraryView` — installed-game carousel with a recycled card pool.
  - `server_view.py`: `ServerView` — platform list on the left, game wall for the selected platform on the right.
  - `details_view.py`: `DetailsView` — game details, action buttons, fanart, screenshots with lightbox, and install progress. Pushed onto the window stack by Home, Library, and Server views.
  - `settings_view.py`: `SettingsView` plus its row widgets (`_SettingRow`, `_TabSelectorRow`, `_ToggleSwitch`, `_ToggleRow`, `_ActionButton`); pushed by `TVWindow` on the guide button rather than being a tab.

- `grid_launcher/tv/widgets/components/`
  - Reusable widgets shared by the TV views.
  - `game_row.py` / `home_card.py`: horizontal Home-tab row and the wide (780x260) card it lays out.
  - `game_wall.py` / `game_card.py`: the Server-tab grid and its portrait (296x480) cover card.
  - `library_card.py`: cover card used by the Library carousel.
  - `platform_card.py`: platform entry card in the Server view's platform list.
  - `controls_bar.py`: `ControlHint` and `ControlsBar` — the bottom hint strip, driven by each view's `CONTROL_HINTS`.
  - `fanart_background.py`: blurred, cross-faded cover art background.
  - `cloud_saves_overlay.py`: cloud save/state slot list overlay with restore/delete/upload actions.
  - `emulator_picker_overlay.py`: emulator choice overlay used by the settings view.
  - `native_exec_picker.py`: `NativeExecPickerDialog` for choosing a native game's launch executable, shown from the details view.
  - `install_progress_bar.py`: install/download progress bar used in the details view.
  - `nav_scroll_area.py`: `QScrollArea` subclass that does not swallow TV navigation key events.
  - `scrollbar.py`: `TvScrollBar`, a custom-painted scrollbar sized for TV viewing distance.

## Practical Change Guide
- Server API and auth requests: `grid_launcher/core/api.py`
- Server browsing, status, and details flow: `grid_launcher/server/`
- Install, uninstall, archive, and cloud-save behavior: `grid_launcher/library/`
- Emulator detection, defaults, save-scope rules, and launching: `grid_launcher/emulator/`
- Cover caching and details loading: `grid_launcher/cover/`
- Dialog, widget, theme behavior, and details-button visibility/labels: `grid_launcher/ui/`
- Cloud save orchestration (scopes, candidates, emulator path overrides, upload/restore): `grid_launcher/ui/mixins/cloud_mixin.py`
- Emulator settings page, autoprofiles, RetroArch core UI, source-download emulator installs: `grid_launcher/ui/mixins/emulator_ui_mixin.py`
- Game install/uninstall lifecycle, async workers, archive extraction, PS4/Xbox 360 content: `grid_launcher/ui/mixins/install_mixin.py`
- Details panel rendering, cloud record display, PCGamingWiki paths, native save paths: `grid_launcher/ui/mixins/details_view_mixin.py`
- Discover tab: page widgets and sections in `grid_launcher/ui/discover.py`, cache/fetch/watchlist logic in `grid_launcher/server/discover.py`, off-thread loading in `DiscoverLoadWorker` (`grid_launcher/background/workers.py`), and page construction plus section labels in `grid-launcher.py` (`_build_discover_page`, `_DISCOVER_SECTION_LABELS`)
- TV mode: state and threaded work in `grid_launcher/tv/bridge/`, widgets and views in `grid_launcher/tv/widgets/`. TV mode does not share `MainWindow` or `grid_launcher/ui/` — a desktop-mode fix usually needs a matching TV-mode fix
- Controller input and the fullscreen pause overlay: `grid_launcher/tv/bridge/controller.py`, `grid_launcher/tv/bridge/pause_backend.py`, `grid_launcher/tv/widgets/pause_window.py`
- Background worker behavior and async details cloud loading: `grid_launcher/background/workers.py`
- Top-level orchestration, shared-save warnings, and signal wiring: `grid-launcher.py`

## Maintenance Notes
- Keep `MainWindow` focused on orchestration.
- Prefer reusable logic in the `grid_launcher/*` packages.
- Shared emulator save media (for example Xemu HDD images and Redream VMUs) should be represented in the UI as emulator-wide backup scopes rather than per-game saves.
- Any future details-panel cloud queries should stay async so the view can switch immediately before remote/local lookup work begins.
- Update this file when module ownership changes.
- Emulator autoprofiles (`emulator-autoprofiles.json`) define `screenshot_directories` alongside `save_directories` and `state_directories`. The `_resolved_screenshot_directories()` method in `grid_launcher/ui/mixins/cloud_mixin.py` resolves these paths; `session_screenshot_path()` in `cloud_transfer.py` finds the most recent session-window screenshot to attach to cloud uploads. These are intentionally absent for PPSSPP and RetroArch which use file sidecars instead.
- The `InstallFinalizeWorker` in `workers.py` only deletes the downloaded archive after installation when extraction actually occurred (`extracted_path` is non-empty). Direct game file formats (.chd, .iso, .bin, etc.) must never be deleted post-install.
- RetroArch firmware routing details are in `retroarch-core-list.json` and `firmware_install.py`. Debug output available via the `grid_launcher.library.firmware_install` logger.
- Cross-platform emulator support is handled inline within the emulator modules rather than a dedicated Linux layer: RetroArch core paths branch by platform in `launch.py` (`.so`/`.dylib`/`.dll`), Cemu picks the SDL controller profile on non-Windows platforms in `cemu.py`, and Xenia Edge ships as a native Linux AppImage. Keep new platform behavior alongside the emulator it affects.
- Xenia Edge (Xbox 360) is defined in `emulator-autoprofiles.json` as the `"Xenia Edge (Xbox 360)"` profile, sourced as a native Linux AppImage from the `has207/xenia-edge` GitHub release. Its variant handling lives in `xenia.py` (see `_is_edge_variant()`).
- `MainWindow` is composed via four mixins (`CloudSaveMixin`, `EmulatorUIMixin`, `InstallMixin`, `DetailsViewMixin`) in `grid_launcher/ui/mixins/`. When behavior spans mixins (e.g., install triggering a cloud refresh), the call crosses from `InstallMixin` into a method resolved on `self` that lives in another mixin — check the mixin that owns the behavior you're tracing before searching `grid-launcher.py`.
