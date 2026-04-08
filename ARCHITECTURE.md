# Rom Mate Neo Architecture Map

This document tracks the current modular boundaries and where to implement new changes.

## Runtime Entry
- `rom-mate.py`
  - App entry point (`main`) and `MainWindow` orchestration.
  - UI composition, user interactions, and high-level workflows.

## Extracted Modules
- `rom_mate/dialogs.py`
  - `FirstRunSetupDialog`: first-run server/token/library setup UI and validation.

- `rom_mate/workers.py`
  - `InstallDownloadWorker`: background archive download worker.
  - `InstallFinalizeWorker`: post-download install preparation worker.
  - `AutoCloudSaveUploadWorker`: background cloud upload worker.

- `rom_mate/path_utils.py`
  - Path sanitization and path relationship helpers.
  - Canonical path key generation and path containment checks.

- `rom_mate/cover/`
  - Consolidated cover-domain subpackage for parsing, caching, loading, detail-view refresh, and management helpers.
  - `utils.py`: URL normalization, screenshot extraction, and cache key/extension helpers.
  - `cache.py`: cover cache persistence workflow and fallback save behavior.
  - `details.py`: resolved game-cover lookup plus details-view screenshot, layout sizing, and media refresh helpers.
  - `loader.py`: cover image application and async network loader handling.
  - `manager.py`: cache retrieval, queueing, and cleanup wrappers.

- `rom_mate/server/`
  - Consolidated server-domain subpackage for connection, catalog, details-cache, state, status, and server page view helpers.
  - `catalog.py`: platform mapping and ROM pagination/payload transforms.
  - `connection.py`: connection failure classification.
  - `details_cache.py`: ROM detail payload lookup, ROM filename extraction, and details-view ROM-id cache helpers.
  - `orchestrator.py`: connection payload fetch orchestration.
  - `state.py`: credentials/base URL/identity label helpers.
  - `status.py`: server status label presentation helper.
  - `view.py`: server page selection/search/render plus connection reset and platform-list population handlers.

- `rom_mate/server_*.py` (legacy compatibility shims)
  - Removed after migration; imports now target `rom_mate/server/` directly.

- `rom_mate/ui/`
  - UI-focused components and dialogs.
  - `dialogs.py`: first-run setup and native game settings dialog behavior.
  - `downloads.py`: downloads page/widget construction and refresh helpers.
  - `emulators.py`: emulator settings form-state, mapping-list, and default-selection helpers.
  - `game_views.py`: library card construction, visible-library filtering, and game-details view/action-button helpers.
  - `theme.py`: theme normalization, palette-aware variant resolution, stylesheet generation, and inline widget restyling helpers.

- `rom_mate/emulator/`
  - Emulator selection, autoprofile, auto-configuration, RetroArch metadata, and launch-resolution helpers.
  - `selection.py`: platform mapping, platform classification, assignable-platform and Dolphin variant helpers, default emulator selection, emulator-entry resolution, RPCS3 entry detection, and install-block decisions.
  - `profiles.py`: emulator autoprofile defaults, save-strategy/ignore normalization, config-path splitting, loading, and executable/profile matching helpers.
  - `autoconfig.py`: emulator executable selection and auto-configuration of emulator/default/core settings for installed emulator packages.
  - `retroarch.py`: RetroArch core-list parsing, compatibility map loading, platform-key normalization, and installed-core discovery helpers.
  - `launch.py`: launch placeholder substitution, launchable-file predicates, ROM/argument resolution, command preparation, RetroArch core token normalization, and launch arg parsing.

- `rom_mate/background/`
  - Background worker components for threaded download/install/cloud-upload tasks.
  - `workers.py`: install download/finalize and auto cloud upload workers.

- `rom_mate/core/`
  - Shared foundational helpers for HTTP API, config normalization, token persistence, path utilities, and protocol typing.
  - `api.py`: authenticated HTTP and multipart helper functions.
  - `config.py`: emulator/default/installed-game normalization plus config merge and persistence serialization helpers.
  - `token_store.py`: secure API token load/save helpers, including Windows DPAPI handling.
  - `path.py`: path sanitization and containment helpers.
  - `types.py`: shared protocol typing used by cross-module workers.

- `rom_mate/library/`
  - Library-domain helpers for game identity, cloud state, restore selection, install preparation, and installed-game lookup semantics.
  - `identity.py`: game key, ROM identity, and installed-record lookup helpers.
  - `cloud_sync.py`: cloud sync state normalization, candidate/session filtering, session/upload planning, lookup, and update helpers.
  - `cloud_restore.py`: server record selection, timestamp ranking, and local restore target resolution helpers.
  - `cloud_transfer.py`: transfer URL normalization, archive extraction, upload archive creation, session-window upload filtering, and upload/restore transfer utility helpers.
  - `cloud_upload.py`: upload job construction, no-match message selection, and final upload result messaging helpers.
  - `downloads.py`: download status/detail text, status-bar display-state, list entry action/title policy, entry data mutation, and transfer size/percent formatting helpers.
  - `archive_preparation.py`: archive extraction decisions, extracted-dir and launch-file resolution, tar size/progress helpers, and non-UI installed-game preparation orchestration.
  - `install_metadata.py`: install-time ROM filename derivation and metadata hydration helpers for queued installs and details sync.
  - `install_paths.py`: archive/extracted/native executable path resolution helpers used by install, launch, and uninstall flows.
  - `install_registry.py`: installed-game record construction plus library match/filter helpers for install and uninstall flows.
  - `install_cleanup.py`: non-UI uninstall file cleanup plus shared library/emulator uninstall orchestration for archives, extracted directories, and PS3 link removal/update flows.
  - `install_state.py`: install queue, pending-install, transfer progress/status, and queue-start guards.
  - `ps3_links.py`: PS3 symlink planning, game-id detection, link path parsing, and RPCS3 `games.yml` lookup/update helpers.

- `rom_mate/api_client.py`
  - Shared HTTP request helpers (GET JSON, GET bytes, POST JSON, multipart POST).
  - Authorization header and multipart payload builders.

- `rom_mate/types.py`
  - Protocol typing used by worker modules to avoid circular imports.

## Practical Change Guide
- Change server HTTP behavior:
  - Primary: `rom_mate/api_client.py`
  - Integration wrappers: `MainWindow._api_*` methods in `rom-mate.py`

- Change first-run setup UX:
  - `rom_mate/dialogs.py`

- Change install/download background task behavior:
  - `rom_mate/workers.py`

- Change filesystem path normalization/safety logic:
  - `rom_mate/path_utils.py`

- Change cover URL parsing and cache key/extension behavior:
  - `rom_mate/cover/utils.py`

- Change cover cache write/fallback persistence flow:
  - `rom_mate/cover/cache.py`

- Change details-view cover/screenshot refresh behavior:
  - `rom_mate/cover/details.py`

- Change cover load/render orchestration:
  - `rom_mate/cover/loader.py`

- Change cover management wrappers and cleanup flow:
  - `rom_mate/cover/manager.py`

- Change server platform/game mapping and pagination behavior:
  - `rom_mate/server/catalog.py`

- Change server details-cache and ROM-id lookup behavior:
  - `rom_mate/server/details_cache.py`

- Change server connection failure handling decisions:
  - `rom_mate/server/connection.py`

- Change server page selection/search/render or platform-list reset/population behavior:
  - `rom_mate/server/view.py`

- Change connection state/identity helper behavior:
  - `rom_mate/server/state.py`

- Change server status label presentation behavior:
  - `rom_mate/server/status.py`

- Change server connection fetch orchestration:
  - `rom_mate/server/orchestrator.py`

- Change setup dialog behavior:
  - `rom_mate/ui/dialogs.py`

- Change library-card and details-view UI behavior:
  - `rom_mate/ui/game_views.py`

- Change theme/appearance behavior:
  - `rom_mate/ui/theme.py`

- Change background task worker behavior:
  - `rom_mate/background/workers.py`

- Change shared HTTP helper behavior:
  - `rom_mate/core/api.py`

- Change shared config normalization or disk-serialization behavior:
  - `rom_mate/core/config.py`

- Change secure API token persistence behavior:
  - `rom_mate/core/token_store.py`

- Change shared path utility behavior:
  - `rom_mate/core/path.py`

- Change shared protocol typing behavior:
  - `rom_mate/core/types.py`

- Change library game identity behavior:
  - `rom_mate/library/identity.py`

- Change install/archive preparation and extraction behavior:
  - `rom_mate/library/archive_preparation.py`

- Change installed-game registration and library match/filter behavior:
  - `rom_mate/library/install_registry.py`

- Change install metadata hydration or archive-name resolution behavior:
  - `rom_mate/library/install_metadata.py`

- Change uninstall file cleanup behavior:
  - `rom_mate/library/install_cleanup.py`

- Change cloud restore target/record selection behavior:
  - `rom_mate/library/cloud_restore.py`

- Change cloud upload job planning/result messaging behavior:
  - `rom_mate/library/cloud_upload.py`

- Change emulator selection/default resolution behavior:
  - `rom_mate/emulator/selection.py`

- Change emulator auto-configuration behavior:
  - `rom_mate/emulator/autoconfig.py`

- Change install queue/pending state behavior:
  - `rom_mate/library/install_state.py`

- Change download detail/status text formatting behavior:
  - `rom_mate/library/downloads.py`

## Refactor Pattern Used
- Keep `MainWindow` public/internal method names stable.
- Move pure/reusable logic to `rom_mate/*` modules.
- Retain thin wrappers in `MainWindow` to minimize behavioral risk.
- Validate every extraction with `python -m py_compile`.

## Suggested Next Extractions
1. Small launch/config polish passes only if `MainWindow` still feels too dense.
2. Any remaining UI-only wrappers only if they still feel noisy.
3. Only extract more if it materially improves clarity.

## Next Agent Handoff
- `rom-mate.py` is now mostly an orchestration shell; install prep, PS3 linking, uninstall cleanup/orchestration, cover/details refresh, emulator selection/launch helpers, server cache helpers, and status/config persistence/token storage have already been extracted.
- The latest token persistence helpers now live in `rom_mate/core/token_store.py`, while install metadata and uninstall orchestration remain in `rom_mate/library/*`, keeping those `MainWindow` methods thin.

### Recommended Next Slice
1. Stop here if the remaining wrappers are already clear enough.
2. Apply only very small polish passes if a concrete noisy cluster still stands out.

### Validation Checklist
- Use the project `.venv`.
- Update relevant `__init__.py` exports when moving helpers.
- Keep this document current after each extraction.
- Re-run `python -m py_compile` on all touched files after each slice.
