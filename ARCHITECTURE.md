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

- `rom_mate/cover_utils.py`
  - Cover URL normalization and payload parsing helpers.
  - Screenshot URL extraction and cover cache naming/extension helpers.

- `rom_mate/cover_cache.py`
  - Cover cache persistence workflow for game records.
  - Download-to-cache and fallback pixmap-to-file save behavior.

- `rom_mate/cover_loader.py`
  - Cover image application, async queueing, and network reply handling.
  - Shared loader orchestration used by `MainWindow` wrappers.

- `rom_mate/cover_manager.py`
  - Remaining cover management wrappers for cache retrieval, queueing, and cleanup.
  - Shared path-key based cache bookkeeping helpers.

- `rom_mate/server_catalog.py`
  - Server catalog helpers for connected user extraction, platform mapping, and ROM pagination.
  - Transformation of ROM payloads into UI game-card dictionaries.

- `rom_mate/server_connection.py`
  - Connection error classification for HTTP and network failures.
  - Shared status/dialog decision model used by `MainWindow`.

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
  - `rom_mate/cover_utils.py`

- Change cover cache write/fallback persistence flow:
  - `rom_mate/cover_cache.py`

- Change cover load/render orchestration:
  - `rom_mate/cover_loader.py`

- Change cover management wrappers and cleanup flow:
  - `rom_mate/cover_manager.py`

- Change server platform/game mapping and pagination behavior:
  - `rom_mate/server_catalog.py`

- Change server connection failure handling decisions:
  - `rom_mate/server_connection.py`

## Refactor Pattern Used
- Keep `MainWindow` public/internal method names stable.
- Move pure/reusable logic to `rom_mate/*` modules.
- Retain thin wrappers in `MainWindow` to minimize behavioral risk.
- Validate every extraction with `python -m py_compile`.

## Suggested Next Extractions
- Cloud sync state normalization helpers.
- Cover image caching and URL resolution helpers.
- Emulator profile matching and compatibility logic.
- Install/uninstall file candidate resolution helpers.
