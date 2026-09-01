# Rust rewrite milestone 2 — core install pipeline (design)

**Status:** approved design, pre-implementation
**Behavior contract:** `docs/porting/03-library-install.md` (cited below as "doc 03")
**Builds on:** milestone 1 walking skeleton
(`docs/superpowers/specs/2026-08-31-rust-tauri-walking-skeleton-design.md`)

## Goal

The Rust app can download a game from the RomM server, extract it, record the
install in a SQLite registry, show live progress in a downloads UI, and
uninstall it — with the same queue semantics and on-disk library layout as the
Python reference, minus the deferred subsystems listed below.

## Scope

In scope:

- Download queue: queued/downloading/installing/completed/failed/cancelled
  entries, one active download + one active finalize, FIFO waiting list,
  cancel/retry/dismiss.
- Streamed download of single-file and multi-file ROMs with progress.
- Should-extract rules, archive extraction (zip, tar with gz/bz2/xz, 7z),
  launch-file selection.
- Install registry in SQLite; installed badge in the library UI.
- Uninstall (files + registry row).
- Downloads UI: drawer with per-entry rows and actions, aggregate status line.
- New `RommClient` surface: ROM detail (`GET /api/roms/{id}`) and content
  download (`GET /api/roms/{id}/content/{file_name}`, `?file_ids=`).

Out of scope (later milestones): PS3/PS4/Xbox 360 specials, firmware,
emulator-source installs, native Windows games (`game.json`, Wine prefixes,
compat tools), `.rar` archives (the reference only extracts `.rar` for PS3),
update detection, launching games.

## Global constraints

- Secret handling rules from the milestone 1 spec are normative here
  unchanged: credentials only in the OS keyring and `SecretString`; exactly two
  `expose_secret()` call sites (`secrets.rs`, `romm/mod.rs`); no secrets in
  config, database, logs, IPC payloads, events, or test fixtures.
  `rewrite/scripts/check_secret_hygiene.sh` must stay green.
- All new persisted paths derive from milestone 1's directories: config dir
  holds `config.toml` and the new `grid-launcher.db`; the cover cache is
  unchanged.
- Rust edition/toolchain, workspace layout, CI workflow, and test frameworks
  (cargo test, wiremock, vitest) are unchanged from milestone 1.
- Errors cross the IPC boundary as `Display` strings, never containing
  credentials (milestone 1 rule).

## Architecture

New `library` module in `grid-core`, one responsibility per file:

```
rewrite/crates/grid-core/src/library/
  mod.rs            public surface: InstallService, types re-exports
  queue.rs          entry list + state machine + admission/dequeue rules
  download.rs       streamed HTTP download with progress + cancellation
  extract.rs        format dispatch, extraction, progress
  launch_select.rs  launch-file ranking (pure functions)
  registry.rs       SQLite installed-games registry
  uninstall.rs      file removal + registry delete
  paths.rs          sanitization, archive naming, candidate resolution
```

Tauri layer: new commands in `commands.rs` and a `downloads-changed` event.
Frontend: a downloads store, a downloads drawer component, a game details
overlay with Install/Uninstall, an installed badge on cards.

### InstallService

`InstallService` lives in `grid-core`, is owned by `AppState` next to
`SessionManager`, and holds:

- `Mutex<QueueState>` — the entry list and the two in-progress slots.
- An `Arc<Registry>` (SQLite handle behind a mutex; rusqlite connections are
  not Sync).
- A change-notification callback `Arc<dyn Fn(DownloadsSnapshot) + Send + Sync>`
  set once by the Tauri layer, which emits the event to the webview.

Starting an install spawns tokio tasks; extraction and all SQLite and file I/O
run under `tokio::task::spawn_blocking`. The service never touches Tauri types
— grid-core stays UI-agnostic, same as milestone 1.

### Data flow for one install

1. Command `install_game(rom_id)` → service resolves the game from the client's
   detail payload, computes the target paths, and admits the job (doc 03 §1:
   if a download or finalize is running, append to the FIFO queue as `queued`).
2. Download task streams to the archive path in 64 KiB chunks, emitting
   progress at most every 100 ms (cumulative bytes, `Content-Length` total or
   0, average speed since start). Cancellation is checked before every chunk.
3. On download success the entry becomes `installing` and the finalize task
   runs: should-extract decision, extraction, launch-file selection, registry
   upsert, archive cleanup.
4. Every terminal transition (completed/failed/cancelled) starts the next
   queued job if both slots are free (doc 03 invariant 12).

## Data model

### Download entry (in memory only, not persisted)

```rust
pub struct DownloadEntry {
    pub id: u64,                       // monotonically increasing per session
    pub rom_id: i64,
    pub title: String,
    pub platform: String,
    pub status: DownloadStatus,        // Queued | Downloading | Installing
                                       // | Cancelling | Completed | Failed | Cancelled
    pub downloaded_bytes: u64,
    pub total_bytes: u64,              // 0 when Content-Length absent
    pub speed_bps: f64,
    pub install_processed_bytes: u64,
    pub install_total_bytes: u64,
    pub error: String,                 // stripped; empty unless Failed
}
```

Progress values clamp to ≥ 0; entering a terminal status zeroes `speed_bps`
(doc 03, downloads.py rules). Display formatting (percent, size units B..TB
with the 1024 rule, per-status detail text, aggregate status line) is ported
from doc 03 §1's tables and implemented **in the frontend** from the raw
snapshot — grid-core ships numbers, not display strings.

Per-entry action affordance by status is the doc 03 table: cancel for
Queued/Downloading/Cancelling, none for Installing, retry+dismiss for
Failed/Cancelled, dismiss for Completed.

### SQLite registry

File: `<config dir>/grid-launcher.db`. `rusqlite` with the `bundled` feature.
Schema version via `PRAGMA user_version` (starts at 1); opening a database
with a newer version than the app knows fails with a clear error.

```sql
CREATE TABLE installed_games (
    id                  INTEGER PRIMARY KEY,
    title               TEXT NOT NULL,
    platform            TEXT NOT NULL,
    title_key           TEXT NOT NULL,   -- title.trim().to_lowercase()
    platform_key        TEXT NOT NULL,   -- platform.trim().to_lowercase()
    rom_id              INTEGER,         -- server ROM id, NULL if unknown
    rom_file_name       TEXT NOT NULL DEFAULT '',
    archive_path        TEXT NOT NULL DEFAULT '',
    extracted_path      TEXT NOT NULL DEFAULT '',
    extracted_dir       TEXT NOT NULL DEFAULT '',
    multi_file_game_dir TEXT NOT NULL DEFAULT '',
    description         TEXT NOT NULL DEFAULT '',
    rating              TEXT NOT NULL DEFAULT '',
    genres              TEXT NOT NULL DEFAULT '',
    regions             TEXT NOT NULL DEFAULT '',
    languages           TEXT NOT NULL DEFAULT '',
    tags                TEXT NOT NULL DEFAULT '',
    revision            TEXT NOT NULL DEFAULT '',
    companies           TEXT NOT NULL DEFAULT '',
    first_release_date  TEXT NOT NULL DEFAULT '',
    filesize_bytes      INTEGER NOT NULL DEFAULT 0,
    server_updated_at   TEXT NOT NULL DEFAULT '',
    installed_at        INTEGER NOT NULL,          -- unix seconds
    UNIQUE (title_key, platform_key)
);
```

- Registration is an upsert on `(title_key, platform_key)` — replaces any
  existing row, which is the reference's "remove same identity then append"
  (doc 03 §15) without the duplicate-record window.
- `archive_path` is stored **only when `extracted_path` is empty** (doc 03
  invariant 4 — mutually exclusive).
- Installed lookup: if both sides have a rom id, compare rom ids; otherwise
  compare `(title_key, platform_key)` (doc 03 identity rules).
- Later milestones add columns (native/PS3/PS4 fields) by bumping
  `user_version` with an `ALTER TABLE` migration.
- Unlike the Python normalizer, no field is dropped on persist — this fixes
  the reference defect where `revision`, `languages`, `tags`, `companies`,
  `first_release_date` (and `fanart_url`) were lost on the next save.
  `fanart_url` and `cached_cover_path` are intentionally absent: covers live
  in milestone 1's content-addressed `CoverCache` keyed by rom id, so the
  registry does not track image files.

### Config addition

`Config` gains a typed field `library_path: String` (default `""`, `~`
expanded on use). An empty library path blocks installs with the error
`"Set a library folder in settings before installing games"`. A minimal
settings input for it is part of this milestone's UI (a text field is enough;
a native folder picker is a later polish item).

## Behavior

### Library layout and naming (doc 03, ported verbatim)

```
<library_path>/
  <SanitizedPlatform>/
    <archive file>              single-file download target
    <archive stem>/             its extraction directory
    <SanitizedTitle>/           multi-file ROM folder (Disc1.chd, game.m3u, …)
```

- Sanitization: every character in `<>:"/\|?*` and every `ord < 32` becomes
  `_`; trailing spaces/dots become `_`; if the result is only
  spaces/underscores/dots, use the caller's fallback string.
- Archive name: server `fs_name` (last path segment, backslashes normalized),
  else `<safe_title>-<safe_platform>.zip`.
- Extraction dir: `<archive parent>/<archive stem>`; if that equals the
  archive or exists as a file, `<stem>_extracted`.

### Choosing the download target (doc 03 §2, minus deferred shapes)

From the detail payload's `files` (RomFileSchema), the top-level candidates
are entries where `is_top_level` is true, the name is not `game.json`, and the
name contains no `/` or `\`:

- **Multi-file** (more than one candidate): create
  `<platform dir>/<SafeTitle>/`, record it as `multi_file_game_dir`, pick the
  launch entry (first name ending `.m3u`, else first entry), download every
  candidate into the folder — each with its own `?file_ids=<id>`. Multi-file
  entries are never extracted; `extracted_path` is the launch entry's path,
  `extracted_dir` is empty.
- **Single file**: target `<platform dir>/<archive name>`, URL
  `{base}/api/roms/{rom_id}/content/{file_name}` with `?file_ids=` from the
  base file id when the payload provides one.

Auth: the same singleton header the client already holds; download requests
never build a second credential path.

### Should this archive be extracted? (doc 03 §3, reduced to core scope)

1. Arcade platform (platform name contains `arcade`, `mame`, `fbneo`, or
   `final burn`, case-insensitive) ⇒ never extract.
2. Otherwise extract iff the file suffix is one of
   `.7z .zip .tar .gz .bz2 .xz` (case-insensitive).

When extraction is skipped: a `.appimage` file is chmod'ed `0o755` on
non-Windows; the archive is the install (`extracted_path`/`extracted_dir`
empty, `archive_path` kept).

### Extraction (doc 03 §4, pure-Rust engine)

Crates: `zip` for zip, `tar` + `flate2`/`liblzma`/`bzip2` for tarballs,
`sevenz-rust2` for 7z. Dispatch order:

1. Suffix `.7z` ⇒ 7z chain: `sevenz-rust2` first; if it errors, fall back to a
   system `7z`/`7za`/`7zz` binary found on `PATH` or at the doc 03 known
   absolute paths (`x <archive> -o<dir> -y`, stdout discarded, stderr
   captured). No portable-tool downloads — that machinery existed because
   Python lacked a 7z library. If both fail, the error names both failures.
2. Else, if the file starts with a ZIP signature (content check, not suffix)
   ⇒ the `zip` crate.
3. Else ⇒ `tar` with the decompressor chosen by content sniffing
   (gzip/bzip2/xz magic bytes, else plain tar).

Invariants (doc 03, kept):

- The target directory is removed and recreated before extraction.
- Any extraction failure deletes the target directory before returning
  (exception: partial output from the pure-Rust 7z stage is left for the
  system-7z fallback, which wipes before re-extracting — the doc 03 stage-4
  rule).
- Zip member paths are normalized (backslash → `/`); members that are
  absolute or contain `..` are rejected (path-traversal guard applied to all
  formats, stricter than the reference — see Deviations).
- Progress: totals are the sum of member sizes read from the archive
  metadata; processed bytes are emitted per member, throttled to 150 ms.

After extraction, launch-file selection runs; no selectable file ⇒ delete the
extraction directory and fail with
`"Archive extracted but no ROM file was found"`. On non-Windows, chmod the
selected launch file `0o755`. Record `extracted_path` + `extracted_dir`,
leave `archive_path` empty, and delete the archive (see cleanup below).

### Launch-file selection (doc 03 §10, ported verbatim)

Pure function over the recursive file list of the extraction directory:

- Pool: files whose suffix is not in `.zip .7z .rar .tar .gz .bz2 .xz`; if
  empty, all files.
- Preferred extensions, priority order:
  `.m3u .cue .chd .iso .xex .bin .pbp .cso .img .ccd .nrg .mdf .gdi .rvz .gcz
  .wbfs .gcm .dol .elf .nes .fds .sfc .smc .gba .gb .gbc .n64 .z64 .v64 .nds
  .3ds .cia .xci .nsp .gen .smd .md .32x .sms .gg .pce .sgx .a26 .a52 .a78
  .lnx .ws .wsc .ngp .ngc .jag .rom`
- Penalties: +1 when any parent segment (relative to the root, excluding the
  file name) is one of `__macosx glcache cache caches shadercache shaders docs
  doc manual manuals readme licenses license resources`; +1 when the suffix is
  in the doc 03 "support" set
  (`.txt .nfo .diz .log .json .xml .ini .cfg .conf .url .pdf .html .htm .png
  .jpg .jpeg .gif .bmp .webp .svg .ico .dll .so .dylib .py .lua .js .css .db
  .sqlite .tmp .cache .sav .srm .state .states .cht .slangp .slang .glsl
  .vert .frag`).
- Sort key (ascending): penalty sum; preferred-list rank (unlisted =
  `len + 10`); 0 when the stem case-folds equal to the archive stem else 1;
  path depth; case-folded full path.
- Selection: preferred-extension files first if any; else the zero-penalty
  subset (or the whole pool); within that, stem-matches first; return the
  sorted first.

### Archive cleanup

After a successful finalize where something was extracted, delete the
downloaded archive: up to 20 attempts 0.25 s apart (no Windows
reboot-scheduling and no `tasklist` wait — deferred with Windows support).
If deletion still fails, the entry completes with a visible warning string
(see Deviations). A finalize failure keeps the archive so retry skips the
re-download (doc 03 invariant 5). A failed or cancelled download deletes its
partial file (invariant 6).

### Queue rules (doc 03 §1, ported)

- Admission: if a download or finalize is in progress — same identity already
  pending/queued ⇒ no-op; otherwise append as `queued`.
- Dequeue: pop the queue head only when neither slot is busy.
- Already-installed check on download completion: if the game is already
  installed (identity/rom-id match), mark `completed` and skip finalize.
- Cancel: active download ⇒ cooperative flag, status `cancelling` until the
  task observes it; queued ⇒ remove from queue, status `cancelled` with error
  `"Cancelled while queued"`. In-progress extraction is not cancellable.
- Retry: only `failed`/`cancelled`; dismiss the old entry, start a new
  install for the same rom.
- Dismiss: remove the entry from the list; never touches files or the queue.

### Uninstall (doc 03 §16, core branches)

Per game: if `multi_file_game_dir` exists as a directory, remove it wholesale;
otherwise delete every existing candidate archive file and remove every
existing candidate extracted directory (candidate resolution per doc 03 §17,
minus native fields). Directory removal chmods a read-only offender writable
and retries once. On success, delete the registry row. Covers are not touched
(they live in the shared cache).

### Tauri IPC surface

Commands (all errors as `Display` strings):

| Command | Args | Returns |
| --- | --- | --- |
| `install_game` | `rom_id: i64` | `()` (entry appears via event) |
| `cancel_install` | `entry_id: u64` | `()` |
| `retry_install` | `entry_id: u64` | `()` |
| `dismiss_download` | `entry_id: u64` | `()` |
| `uninstall_game` | `rom_id: i64` | `()` |
| `list_downloads` | — | `DownloadsSnapshot` |
| `list_installed` | — | `Vec<InstalledGame>` (registry rows, serde snake_case) |
| `get_library_path` / `set_library_path` | — / `path: String` | current value / `()` |

Event: `downloads-changed`, payload `DownloadsSnapshot { entries: [...] }` —
the full entry list, newest first. Emitted on every status transition
immediately, and on progress updates coalesced to 100 ms. No payload ever
contains a URL with credentials or any secret.

### Frontend

- `stores/downloads.svelte.ts`: holds the snapshot from the event + initial
  `list_downloads`; derives the aggregate status text and per-entry detail
  text using the doc 03 formatting tables (percent clamped 0–100, sizes B..TB
  with 1 decimal above bytes).
- `Downloads.svelte`: drawer listing entries with progress bars and the
  per-status action buttons; opened from a persistent footer status line that
  shows the aggregate text and overall progress.
- `Details.svelte`: overlay opened by activating a card (click / gamepad
  accept): cover, title, metadata, and Install / Cancel / Uninstall according
  to state. Gamepad `back` closes it.
- `Library.svelte`: installed badge on cards, driven by `list_installed`
  refreshed on `downloads-changed` terminal transitions.
- Settings: minimal library-path field (shown when unset, and reachable from
  the footer).

## Deliberate deviations from the reference

Each resolves a doc 03 open question; the porting doc gets a note when this
milestone merges:

1. **Typed cancellation.** Cancellation is a dedicated error variant mapped to
   `Cancelled` status — never a substring match on "cancel" in error text.
2. **Uninstall continues past failures.** Batch uninstall processes each game
   independently: files removed ⇒ row deleted; a failure leaves that game's
   row and files intact, and all failures are reported together. (The
   reference aborted the whole batch, already-deleted games included.)
3. **Archive-deletion failures are visible.** The retrying delete reports
   failure as a warning on the completed entry instead of always returning
   success silently.
4. **No registry field loss.** The SQLite schema persists every field the
   record builder produces.
5. **Traversal guard everywhere.** Absolute/`..` member paths are rejected in
   all archive formats, not only firmware zips.
6. **Flattening is not ported.** `flatten_single_subdir` is dead code in the
   reference (no caller passes it); the port omits it.

## Concurrency

- tokio throughout; extraction, SQLite, and bulk file I/O in
  `spawn_blocking`.
- One download task and one finalize task at most (two `Option<JoinHandle>`
  slots guarded by the queue mutex); the FIFO queue feeds them.
- Cancellation: `Arc<AtomicBool>` per download task, checked before each
  chunk read.
- Progress flows service → callback → Tauri event; the callback is invoked
  outside the queue lock to keep emission from blocking state changes.
- No fire-and-forget threads: the background retry-delete loop is a tokio
  task tracked by the service (abandoned at shutdown like the reference, but
  observable in tests).

## Error handling

- `LibraryError` enum in grid-core (`thiserror`): `Http`, `Io`, `Extract`,
  `NoLaunchFile`, `Cancelled`, `LibraryPathUnset`, `Registry`, each with a
  human-readable `Display` and no secrets.
- Pipeline boundary: tasks resolve to `Result<(), LibraryError>`; the queue
  maps `Cancelled` ⇒ `cancelled` status, everything else ⇒ `failed` with the
  Display text.
- Registry open/migration errors surface at startup as a visible error state,
  not a crash.

## Testing

- `queue.rs`: state machine unit tests — admission, dequeue ordering, cancel
  in every status, retry/dismiss rules, already-installed skip.
- `paths.rs`: sanitization table tests, archive naming, extraction-dir
  collision, candidate resolution.
- `launch_select.rs`: ranking table tests ported from the doc 03 rules
  (preferred extensions, penalties, stem match, depth, determinism).
- `extract.rs`: integration tests with archives generated in the test (zip,
  tar.gz, tar.xz, 7z), including wipe-before-extract, failure-deletes-dir,
  traversal rejection, no-launch-file failure.
- `registry.rs`: temp-file SQLite — upsert replaces identity, rom-id lookup,
  archive/extracted exclusivity, schema-version guard.
- Download flow: wiremock — single-file, multi-file with `file_ids`,
  Content-Length absent, mid-stream cancellation deletes the partial file,
  HTTP error ⇒ failed entry.
- Frontend: vitest for the downloads store's formatting and derived
  aggregate text.
- Secret hygiene script unchanged and green.

## Manual test checklist (milestone exit gate)

1. Set the library path in the UI; confirm it persists in `config.toml`.
2. Install a small single-file game: entry appears, progress and speed move,
   archive extracts, entry completes, card shows the installed badge.
3. Install a multi-file game: all files land in one `<SafeTitle>/` folder.
4. Cancel a download mid-stream: entry shows `cancelled`, partial file gone.
5. Retry a cancelled entry: fresh download starts.
6. Queue two installs: second waits as `queued`, starts when the first
   finishes.
7. Quit and relaunch: installed badges persist (from `grid-launcher.db`).
8. Uninstall from the details overlay: files and badge are gone.
9. `config.toml` and `grid-launcher.db` contain no token or password.
