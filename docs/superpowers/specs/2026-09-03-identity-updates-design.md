# Identity, update detection, and app version (Rust port milestone 9) — design

**Status:** approved in chat 2026-09-03; binding authority for the implementation plan.
**Reference:** `docs/porting/10-identity-updates.md` (behavior), Python sources
`grid_launcher/library/update_detection.py`, `grid-launcher.py:3270-3325`,
`tests/test_update_detection.py` (oracle).
**Rust paths** below are relative to `rewrite/`.

## 1. Scope

Port doc 10 into the Rust/Tauri rewrite:

1. Version-tag parsing and server-update detection (pure core module).
2. A non-native `Update` install mode that re-installs an already installed ROM.
3. An app-layer update service that recomputes "which installed games have an update"
   on connect, after installs/updates/uninstalls, and clears on disconnect.
4. Surfacing: Library card badge, Details Update button (native merge or plain
   replacement), Details version label.
5. App version in the window title.
6. A **check-only** self-update notice (user decision 2026-09-03): one GitHub
   `releases/latest` request per process, a banner with an "Open release" button,
   no download and no install.

Out of scope: identity-key changes (the registry already implements doc 10's keys —
lowercased `(title, platform)` uniqueness, rom-id-first lookup, `installed_match`
rescue rule); the TV frontend; emulator source-version checks (doc 04, already
ported); PS4/Xbox 360 content buttons (milestone 8, unrelated to `update_available`).

## 2. Core: `library/update_detection.rs` (pure, no I/O)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionTag { Numeric(u32), Semver(Vec<u32>) }

pub fn rom_file_name_version(rom_file_name: &str) -> Option<VersionTag>;
pub fn format_version_tag(tag: &VersionTag) -> String;      // "v01234" | "v3.6.0"
pub fn has_newer_server_rom_version(installed: &str, server: &str) -> bool;
pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>>;
pub fn is_windows_pc_platform(platform: &str) -> bool;
pub fn is_emulators_platform(platform: &str) -> bool;
pub struct ServerVersion<'a> { pub platform: &'a str, pub rom_file_name: &'a str, pub updated_at: &'a str }
pub fn game_has_server_update(installed: &InstalledGame, server: &ServerVersion<'_>) -> bool;
```

Rules (doc 10 "Version tag formats", "Version-tag comparison", "The decision function"):

- **Tag patterns**, case-insensitive, searched anywhere in the name: numeric
  `\(v(\d{5})\)` tried first, then semver `\(v(\d+(?:\.\d+)+)\)`. `(v1234)` matches
  neither (four digits, no dot). Numeric parses to the integer (`(v00042)` → 42);
  semver parses to the dot-separated integers.
- **`format_version_tag`**: numeric renders zero-padded to five digits (`v00042`);
  semver renders the components joined by `.` with a `v` prefix (`v3.6.0`).
- **`has_newer_server_rom_version`**: false when either side has no tag or the kinds
  differ; numeric compares the integers (`server > installed`); semver compares left
  to right, zero-padding the shorter side, first differing component decides, equal
  throughout is false (`1.2` vs `1.2.0` false, `1.2` vs `1.2.1` true,
  `3.6.0` vs `3.6.0.1` true).
- **`parse_timestamp`**: trim; empty → None; replace every `Z` with `+00:00`; parse
  RFC 3339 / ISO-8601 with offset, else parse as a naive datetime and assume UTC;
  failure → None, never an error. Uses `chrono` (already a dependency).
- **`is_windows_pc_platform`**: trimmed, lowercased platform contains `windows` or
  equals `pc`. Empty → false.
- **`is_emulators_platform`**: trimmed, lowercased platform equals `emulators`.
- **`game_has_server_update`**, in order: (1) either platform is the emulators
  platform → false; (2) if either platform is Windows/PC and
  `has_newer_server_rom_version(installed.rom_file_name, server.rom_file_name)` →
  true; a false here falls through; (3) installed `server_updated_at` missing or
  unparseable → false (legacy install); server timestamp missing or unparseable →
  false; else `server > installed` strictly.
- Only `server_updated_at` is read on the installed side (the Python fallbacks
  `rom_updated_at`/`updated_at` never exist on a Rust row).

## 3. Core: `InstallMode::Update`

- New variant `InstallMode::Update`, `kind()` = `"update"`.
- `InstallService::install_update(self: &Arc<Self>, client, rom_id) -> Result<(), LibraryError>`:
  requires an installed row for `rom_id` (`current_row`; error text is the existing
  `NOT_INSTALLED`), fetches `rom_detail`, runs `plan_install` and overrides the job's
  mode to `Update`, then admits under `JobKey::Rom(rom_id)` (an update and a base
  install for the same ROM never run side by side). A native-platform row is
  rejected with the new verbatim error `NATIVE_UPDATE_REQUIRED` =
  `"Native games update through the merge path."` — the app layer never calls this
  for native rows, the guard only closes the door.
- `finish_download`'s "already installed → skip finalize" short-circuit applies to
  `InstallMode::Base` only (it already does); `Update` always finalizes.
- `finalize_inner` routes `Update` to `finalize_base` unchanged: extraction into the
  same destination, `registry.upsert` replaces the row by `(title, platform)`, the
  finalized hook fires. Doc 03's "plain replacement of the ROM" — no pre-clean of
  the old extraction directory (Python parity).
- `DownloadEntry.kind` gains `"update"`; frontend `DownloadKind` union and
  `kindLabel` (label `Update`) and `actionFor` (same as base) follow.

## 4. App layer: `update_service.rs`

Shape mirrors `firmware_service.rs`: a service struct in `AppState`, spawned tasks via
`tauri::async_runtime::spawn`, no Tauri types inside grid-core.

```rust
pub struct UpdateInfo { pub server_rom_file_name: String }
pub struct UpdateService { available: Mutex<HashMap<i64, UpdateInfo>>, generation: AtomicU64 }
pub const UPDATES_CHANGED_EVENT: &str = "updates-changed";
pub const UPDATE_GONE: &str = "A newer server version is no longer available for this game.";

impl UpdateService {
  pub fn new() -> Arc<Self>;
  pub fn spawn_refresh(self: &Arc<Self>, app: AppHandle, session: Arc<SessionManager>, install: Arc<InstallService>);
  pub fn clear(&self, app: &AppHandle);            // on disconnect; emits the event when the map was non-empty
  pub fn rows(&self, install: &InstallService) -> Vec<UpdateRow>;   // for list_updates
  pub fn button_label(&self, row: &InstalledGame) -> String;        // "Update" | "Update to v3.6.0"
}
#[derive(Serialize)] pub struct UpdateRow { pub rom_id: i64, pub label: String }
```

- **Refresh algorithm**: bump `generation`; read all registry rows; for each row with
  `Some(rom_id)` and a non-emulators platform, fetch `client.rom_detail(rom_id)` with
  bounded concurrency (4 in flight, `tokio::sync::Semaphore`); a fetch error means
  "no update" for that row (logged at debug, never the URL); evaluate
  `game_has_server_update`; when the pass finishes and its generation is still
  current, replace the map wholesale and emit `updates-changed` (payload: the
  `Vec<UpdateRow>`). A stale pass (a newer refresh started meanwhile) discards its
  result. No session → the map is cleared.
- **Triggers**: `connect`, `restore_session` (connected outcome), `retry_connect`
  (success); the game-finalized hook (`set_game_finalized_hook` — the existing
  firmware closure calls the update refresh too); `uninstall_game` after success;
  `disconnect` clears. No timer, no polling, no startup check without a session.
- **Button label** (`_details_update_button_text_for_game`): `Update to <formatted>`
  only when `has_newer_server_rom_version(row.rom_file_name, server_rom_file_name)`
  and the server tag formats non-empty; otherwise `Update`.
- **Commands** (`commands/updates.rs`):
  - `list_updates() -> Vec<UpdateRow>`.
  - `update_game(rom_id) -> Result<(), String>`: requires an installed row (else
    `NOT_INSTALLED` text); fetches `rom_detail` — on any error, spawn a refresh and
    return `UPDATE_GONE`; re-evaluates `game_has_server_update` against the fresh
    detail — false also returns `UPDATE_GONE` (after a refresh); native platform
    (`is_native_platform`) → `install_native_update`; else `install_update`.
  - `app_version() -> String` (package version).
  - `open_release_page(url) -> Result<(), String>`: accepts only URLs starting with
    `https://github.com/Sixdd6/grid-launcher/releases/`; opens via
    `tauri_plugin_opener::OpenerExt::open_url` (Rust side; no JS plugin package, no
    capability scope needed).

## 5. Self-update check (check-only)

`app_update.rs` (app layer) + pure helpers in grid-core `launch/forge.rs` are NOT
touched; the check uses `ForgeClient::get(url, true)` so the E2E redirect
(`GRID_LAUNCHER_E2E_FORGE_BASE`) and the no-credential rule apply.

- URL: `https://api.github.com/repos/Sixdd6/grid-launcher/releases/latest`.
- Runs once per process from `setup`, after the window is created, inside
  `tauri::async_runtime::spawn`. Never blocks startup; never repeats.
- Response fields used: `tag_name`, `html_url`. Missing/invalid → silent.
- Comparison (pure app-layer function `app_update::is_newer(current, tag) -> bool`): strip one leading `v`/`V` from the tag; parse both with the
  `semver` crate (`semver::Version::parse`); unparseable → false; result
  `latest > current` using semver precedence (pre-release ordering included).
- **Dev suppression**: when the running version's pre-release contains the
  identifier `dev` (e.g. `0.9.0-dev`), the check does not run at all.
- Failure of any kind is logged at `debug` with the host only; the notice never
  shows on failure.
- Newer → emit `app-update-available` with `{ tag: String, url: String }`.
- Version bump: `tauri.conf.json` `version` and `app/src-tauri/Cargo.toml` `version`
  become `0.9.0-dev` (above the Python line `v0.8.0-beta4`; the release pipeline
  stamps a real version at parity — see memory `ci-disabled-until-parity`).
- New dependencies: `tauri-plugin-opener = "2"` (app), `semver = "1"` (app). The
  opener plugin is a default-tree dependency; `check_secret_hygiene.sh` only
  guards the wdio plugins and is unaffected.

## 6. Frontend

- **Window title**: `GRID Launcher <version>` — set in `setup` via
  `app.get_webview_window("main").set_title(...)` from `app.package_info().version`.
  `tauri.conf.json` keeps the static `GRID Launcher` as the pre-setup default.
- **Store `stores/updates.svelte.ts`**: `updates.rows: UpdateRow[]`,
  `updates.labelFor(romId): string | null` (null when no update), `refresh()` via
  `api.listUpdates()`, `init()` = refresh then `listen(UPDATES_CHANGED_EVENT)` (payload
  applied directly, no re-fetch). Initialized from `App.svelte` alongside
  `initCompatTools` when the shell phase is active.
- **Library card**: `<span data-testid="library-update-badge-<romId>">Update Available</span>`
  rendered when `updates.labelFor(row.rom_id) !== null`; text verbatim.
- **Details**:
  - Button `data-testid="details-update"` shown when the subject is installed, has a
    rom id, and `updates.labelFor(romId)` is non-null; label = that label; disabled
    while a live drawer entry exists for the rom or another action is pending.
  - Native platform (`isNativePlatform`): first click flips to the confirm state,
    label `Saves and configuration will be preserved — confirm update`; second click
    calls `api.updateGame(romId)`. Non-native: one click calls `api.updateGame`.
  - Error text from `update_game` renders in the existing `error` line.
  - Success toast: when a drawer entry with `kind` `update` or `native_update` and
    this rom's id transitions to `completed` while Details is open, show
    `<p data-testid="details-update-toast">Updated '<title>' successfully.</p>`
    (verbatim Python string, title = subject name).
  - Version row: `<p data-testid="details-version">` with the text from
    `versionLabel(platform, romFileNames, revision)` (pure, `details/version.ts`):
    for Windows/PC platforms, the first tag found in `[detail.fs_name, installedRow.rom_file_name]`
    formats as `Version: v01234` / `Version: v3.6.0`; otherwise the trimmed
    `revision` verbatim (no prefix — Python parity); empty → row hidden. The TS
    tag parser and formatter mirror §2 exactly (unit-tested against the same cases).
- **Shell banner**: on `app-update-available`, render
  `<div data-testid="app-update-banner">GRID Launcher <tag> is available <button data-testid="app-update-open">Open release</button> <button data-testid="app-update-dismiss">Dismiss</button></div>`
  at the top of the shell; "Open release" calls `api.openReleasePage(url)`; dismiss
  hides it for the session (module state, survives Shell remounts).
- **Downloads drawer**: kind `update` shows the `Update` badge (shared with
  `native_update`).

## 7. Rulings on doc 10 open questions (recorded as deviations D-10-b …)

- **D-10-b** `game_key` normalization is `to_lowercase` (Rust `str::to_lowercase`),
  already the registry's rule; OQ1 closed.
- **D-10-c** Server lookup is rom-id only and deterministic (one `rom_detail` per
  installed row); the per-entry title/platform disjunction is not ported; OQ3 closed.
- **D-10-d** No positive-only cache: the update set is recomputed wholesale on every
  trigger and `update_game` re-verifies against a fresh detail; OQ5 closed.
- **D-10-e** PC-platform gap reproduced verbatim: detection accepts `pc`, the merge
  path requires `windows`; OQ6 kept.
- **D-10-f** Naive timestamps are UTC (OQ8 kept); `revision` displayed, never
  compared (OQ9 kept); the version label handles both tag kinds (OQ11 fixed).
- **D-10-g** Rows without a rom id (D-10-a) are never checked and never offer
  Update.
- **D-10-h** Self-update: check-only notice via GitHub `releases/latest`, dev builds
  suppressed; OQ14 decided. `0.0.0-dev` ambiguity (OQ15) is moot: the version is
  the package version, `-dev` marks a source build.
- **D-10-i** `update_available` is never persisted (invariant 5): it lives in
  `UpdateService` memory only.
- **D-10-j** Non-native `Update` re-extracts over the existing directory without a
  pre-clean (Python parity, doc 03 "plain replacement"). Stale files from the old
  build may remain; a follow-up may add a clean.

## 8. Testing and gate

- **Rust unit** (`update_detection.rs`): every case in `tests/test_update_detection.py`
  lines 150–260 (tag extraction incl. `(v1234)` → None and the real semver filename;
  numeric/semver/mixed comparisons; `3.6.0` vs `3.6.0.1`; timestamp newer/equal/legacy;
  emulators veto; Windows tag short-circuit; non-Windows tags ignored; naive UTC).
- **Rust integration** (`tests/install_service.rs`): `install_update` on an installed
  row re-extracts and replaces the row (the mirror of
  `already_installed_rom_completes_without_finalizing`, asserting the extraction DID
  happen and `server_updated_at`/`rom_file_name` are the new values); `install_update`
  on an unknown rom → `NOT_INSTALLED`; native row → `NATIVE_UPDATE_REQUIRED`; entry
  kind `update`.
- **App unit**: `app_update::is_newer` (newer, equal, older, pre-release ordering,
  `v` prefix, garbage tag) and `is_dev_build`; `UpdateService::button_label`.
- **Vitest**: `details/version.ts` (tag parse/format/label), `updates` store
  `labelFor`, `kindLabel('update')`.
- **E2E group `updates`** (`fixtures-updates`, `seed/updates-seed.mjs`, forge on):
  seeded rows — rom 801 "Old Rom" (SNES, `server_updated_at` 2025-01-01, extracted dir
  with a stale `old.sfc`), rom 802 "My Game" (Windows, `rom_file_name`
  `mygame (v1.0.0).zip`, `extracted_dir` with `saves/slot1.sav`), rom 803 "Current
  Rom" (SNES, `server_updated_at` equal to the fixture's); fixture details: 801
  `updated_at` 2026-06-01 (`newrom.zip` archive), 802 `fs_name`
  `mygame (v1.1.0).zip`, 803 unchanged. Asserts: badges on 801 and 802 only; Details
  801 button `Update`, click → drawer row kind `Update` → completed → badge gone and
  the new file present; Details 802 label `Update to v1.1.0`, version row
  `Version: v1.0.0`, confirm flow → merge completes → `saves/slot1.sav` survives and
  the version row reads `Version: v1.1.0`; rom 804 seeded as installed but absent
  from the server → no badge; self-update banner appears with the forge's tag
  `v9.9.9-e2e` and dismisses. The mock forge serves
  `/api.github.com/repos/Sixdd6/grid-launcher/releases/latest`.
- Gate: cargo test, clippy, fmt, hygiene, npm check, vitest, full `e2e.sh` with a
  rebuild (memory `sdd-harness-notes`).
- Docs: doc 10 "Rust port deviations (milestone 9)" D-10-b … D-10-j; doc 03 gains
  `Update` in its mode table and deviation list; `rewrite/README.md` E2E table row
  and milestone 9 manual checklist.
