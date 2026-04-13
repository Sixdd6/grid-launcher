# Rom Mate Neo Module Map

This document is a stable module map for the current codebase.

- Use `SPEC.md` for product behavior and UX intent.
- Use `openapi.json` as a single source of truth for all api calls to the server.

## Runtime Entry
- `rom-mate.py`
  - Application entry point and `MainWindow` orchestration shell.
  - Wires together UI interactions, background jobs, and domain helpers.
  - Owns the Game Details cloud panel orchestration, shared-save scope handling, the responsive handoff from `Details` to `Manage Saves` / `Emulator Saves` / `Manage States`, and screenshot directory resolution for cloud upload jobs.

## Package Map
- `rom_mate/core/`
  - Shared low-level helpers used across the app.
  - `api.py`: authenticated HTTP and multipart helpers.
  - `config.py`: config normalization, merging, and persistence.
  - `path.py`: path sanitization and containment helpers.
  - `token_store.py`: secure API token persistence.
  - `types.py`: shared protocol typing.

- `rom_mate/server/`
  - Server connection, catalog loading, cached details, and server-page helpers.
  - `catalog.py`: platform mapping and ROM pagination transforms.
  - `connection.py`: connection failure classification.
  - `details_cache.py`: ROM detail lookup and ROM-id caching.
  - `orchestrator.py`: coordinated server fetch flow.
  - `state.py`: credential, base URL, and identity helpers.
  - `status.py`: server status presentation.
  - `view.py`: server-page selection, search, and render helpers.
  - `retroachievements.py`: RetroAchievements Web API client - achievement fetching and RA game ID resolution.

- `rom_mate/library/`
  - Install, uninstall, archive prep, identity, downloads, and cloud-save behavior.
  - `archive_preparation.py`: extraction and install-prep flow.
  - `cloud_restore.py`: restore record and target selection, including slot-aware restore grouping.
  - `cloud_sync.py`: sync state normalization, shared-save discovery, Redream hash-based savestate matching, and session filtering.
  - `cloud_transfer.py`: upload/restore archive transfer utilities, including session-window screenshot attachment for emulators that save screenshots to a dedicated directory.
  - `cloud_upload.py`: upload planning and result messaging.
  - `downloads.py`: download status and detail formatting.
  - `identity.py`: game key and installed-record lookup helpers.
  - `install_cleanup.py`: uninstall orchestration and file cleanup.
  - `firmware_install.py`: RetroArch and emulator firmware download, routing, and extraction. Supports keyword-filtered routing, MAME-format zip preservation, flat and path-preserving zip extraction, and debug logging via `rom_mate.library.firmware_install` logger.
  - `install_metadata.py`: install-time metadata hydration.
  - `install_paths.py`: archive, extracted, and native path resolution.
  - `install_registry.py`: installed-game record construction and matching.
  - `install_state.py`: queue, pending, and progress state helpers.
  - `ps3_install.py`: PS3 content classification, VFS routing, and game-ID helpers.

- `rom_mate/emulator/`
  - Emulator selection, profiles, auto-configuration, RetroArch integration, and launching.
  - `autoconfig.py`: known emulator auto-configuration.
  - `launch.py`: launch argument substitution and command preparation.
  - `profiles.py`: emulator profile defaults and matching.
  - `retroarch.py`: RetroArch core discovery and compatibility mapping.
  - `selection.py`: default emulator and platform resolution, including cloud save scope classification such as per-game vs shared emulator media.

- `rom_mate/cover/`
  - Cover parsing, caching, loading, and details-view media helpers.
  - `cache.py`: cover cache persistence and fallback save behavior.
  - `details.py`: details-view cover and screenshot refresh helpers.
  - `loader.py`: async image loading and application.
  - `manager.py`: queueing and cache cleanup wrappers.
  - `utils.py`: URL normalization and cache-key helpers.

- `rom_mate/ui/`
  - UI-specific dialogs, views, theming, widget helpers, and `MainWindow` mixin behavior.
  - `dialogs.py`: first-run and native game settings dialogs.
  - `downloads.py`: downloads page/widget construction.
  - `emulators.py`: emulator settings form helpers.
  - `game_views.py`: library cards and details-view UI helpers, including cloud button visibility/label updates like `Emulator Saves`.
  - `theme.py`: theme selection and stylesheet generation.
  - `toast.py`: `ToastWidget` and `show_toast` helper for transient in-window notifications.

- `rom_mate/ui/mixins/`
  - `MainWindow` behavior extracted into composable mixins. `MainWindow` inherits all four in MRO order: `CloudSaveMixin`, `EmulatorUIMixin`, `InstallMixin`, `DetailsViewMixin`.
  - `cloud_mixin.py` (`CloudSaveMixin`): cloud save orchestration — save-scope classification, block-reason resolution, sync candidate discovery (per-game and shared-emulator paths for all emulators), emulator-specific save-directory overrides (Cemu, Dolphin, PCSX2, RPCS3, etc.), session-window filtering, screenshot and firmware directory resolution, and upload/restore coordinator helpers.
  - `emulator_ui_mixin.py` (`EmulatorUIMixin`): emulator settings page behavior — emulator config normalization, autoprofile loading, RetroArch core list and compatibility map access, emulator view refresh, emulator add/edit/remove/save form actions, source-download emulator install flow, RPCS3 firmware background download trigger, and emulator path/library browsing.
  - `install_mixin.py` (`InstallMixin`): game install/uninstall lifecycle — async download and finalize workers, archive extraction, PS4/Xbox 360 content install flows, firmware routing post-install, native game update application, installed-game registration, update-state refresh, and library path resolution.
  - `details_view_mixin.py` (`DetailsViewMixin`): game details panel — opening/closing the details view, cloud panel rendering (records, upload, restore, delete), details-view responsive layout (responsive resize), PCGamingWiki save-path lookup, native save-path section rendering, cloud sync state accessors, ROM ID caching, install-queue integration, and the async `DetailsCloudRecordsWorker` lifecycle.

- `rom_mate/background/`
  - Threaded background workers for downloads, installs, cloud uploads, and async details-panel cloud record loading.
  - `workers.py`: worker implementations such as install/download workers, auto cloud upload workers, and the async details cloud-record fetch worker used to keep the Details view responsive.

## Practical Change Guide
- Server API and auth requests: `rom_mate/core/api.py`
- Server browsing, status, and details flow: `rom_mate/server/`
- Install, uninstall, archive, and cloud-save behavior: `rom_mate/library/`
- Emulator detection, defaults, save-scope rules, and launching: `rom_mate/emulator/`
- Cover caching and details loading: `rom_mate/cover/`
- Dialog, widget, theme behavior, and details-button visibility/labels: `rom_mate/ui/`
- Cloud save orchestration (scopes, candidates, emulator path overrides, upload/restore): `rom_mate/ui/mixins/cloud_mixin.py`
- Emulator settings page, autoprofiles, RetroArch core UI, source-download emulator installs: `rom_mate/ui/mixins/emulator_ui_mixin.py`
- Game install/uninstall lifecycle, async workers, archive extraction, PS4/Xbox 360 content: `rom_mate/ui/mixins/install_mixin.py`
- Details panel rendering, cloud record display, PCGamingWiki paths, native save paths: `rom_mate/ui/mixins/details_view_mixin.py`
- Background worker behavior and async details cloud loading: `rom_mate/background/workers.py`
- Top-level orchestration, shared-save warnings, and signal wiring: `rom-mate.py`

## Maintenance Notes
- Keep `MainWindow` focused on orchestration.
- Prefer reusable logic in the `rom_mate/*` packages.
- Shared emulator save media (for example Xemu HDD images and Redream VMUs) should be represented in the UI as emulator-wide backup scopes rather than per-game saves.
- Any future details-panel cloud queries should stay async so the view can switch immediately before remote/local lookup work begins.
- Update this file when module ownership changes.
- Emulator autoprofiles (`emulator-autoprofiles.json`) define `screenshot_directories` alongside `save_directories` and `state_directories`. The `_resolved_screenshot_directories()` method in `rom-mate.py` resolves these paths; `session_screenshot_path()` in `cloud_transfer.py` finds the most recent session-window screenshot to attach to cloud uploads. These are intentionally absent for PPSSPP and RetroArch which use file sidecars instead.
- The `InstallFinalizeWorker` in `workers.py` only deletes the downloaded archive after installation when extraction actually occurred (`extracted_path` is non-empty). Direct game file formats (.chd, .iso, .bin, etc.) must never be deleted post-install.
- RetroArch firmware routing details are in `retroarch-core-list.json` and `firmware_install.py`. Debug output available via the `rom_mate.library.firmware_install` logger.
- `MainWindow` is composed via four mixins (`CloudSaveMixin`, `EmulatorUIMixin`, `InstallMixin`, `DetailsViewMixin`) in `rom_mate/ui/mixins/`. When behavior spans mixins (e.g., install triggering a cloud refresh), the call crosses from `InstallMixin` into a method resolved on `self` that lives in another mixin — check the mixin that owns the behavior you're tracing before searching `rom-mate.py`.
