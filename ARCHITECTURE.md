# Rom Mate Neo Module Map

This document is a stable module map for the current codebase.

- Use `SPEC.md` for product behavior and UX intent.
- Use `openapi.json` when changing server connectivity or API contracts.

## Runtime Entry
- `rom-mate.py`
  - Application entry point and `MainWindow` orchestration shell.
  - Wires together UI interactions, background jobs, and domain helpers.
  - Owns the Game Details cloud panel orchestration, shared-save scope handling, and the responsive handoff from `Details` to `Manage Saves` / `Emulator Saves` / `Manage States`.

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

- `rom_mate/library/`
  - Install, uninstall, archive prep, identity, downloads, and cloud-save behavior.
  - `archive_preparation.py`: extraction and install-prep flow.
  - `cloud_restore.py`: restore record and target selection, including slot-aware restore grouping.
  - `cloud_sync.py`: sync state normalization, shared-save discovery, Redream hash-based savestate matching, and session filtering.
  - `cloud_transfer.py`: upload/restore archive transfer utilities.
  - `cloud_upload.py`: upload planning and result messaging.
  - `downloads.py`: download status and detail formatting.
  - `identity.py`: game key and installed-record lookup helpers.
  - `install_cleanup.py`: uninstall orchestration and file cleanup.
  - `install_metadata.py`: install-time metadata hydration.
  - `install_paths.py`: archive, extracted, and native path resolution.
  - `install_registry.py`: installed-game record construction and matching.
  - `install_state.py`: queue, pending, and progress state helpers.
  - `ps3_links.py`: RPCS3 / PS3 link planning and metadata helpers.

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
  - UI-specific dialogs, views, theming, and widget helpers.
  - `dialogs.py`: first-run and native game settings dialogs.
  - `downloads.py`: downloads page/widget construction.
  - `emulators.py`: emulator settings form helpers.
  - `game_views.py`: library cards and details-view UI helpers, including cloud button visibility/label updates like `Emulator Saves`.
  - `theme.py`: theme selection and stylesheet generation.

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
- Background worker behavior and async details cloud loading: `rom_mate/background/workers.py`
- Top-level orchestration, shared-save warnings, and signal wiring: `rom-mate.py`

## Maintenance Notes
- Keep `MainWindow` focused on orchestration.
- Prefer reusable logic in the `rom_mate/*` packages.
- Shared emulator save media (for example Xemu HDD images and Redream VMUs) should be represented in the UI as emulator-wide backup scopes rather than per-game saves.
- Any future details-panel cloud queries should stay async so the view can switch immediately before remote/local lookup work begins.
- Update this file when module ownership changes.
