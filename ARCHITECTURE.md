# GRID Launcher Module Map

A stable map of where behavior lives. Update it when module ownership changes.

- Use `SPEC.md` for product behavior and UX intent.
- Use `openapi.json` as the source of truth for RomM server requests.
- Build, test and release commands are in `BUILD.md`.

## Top level

    Cargo.toml            workspace: crates/grid-core, app/src-tauri
    crates/grid-core/     UI-agnostic core library (never depends on Tauri)
    app/src-tauri/        Tauri 2 shell: services, commands, hooks
    app/src/              Svelte 5 frontend
    e2e/                  WebdriverIO harness, mock RomM server, mock forge
    scripts/              e2e.sh, check_secret_hygiene.sh
    *.json                data files and the RomM API contract
    assets/               repository assets (7-Zip for Windows, artwork)

The dependency direction is one-way: frontend → Tauri commands → grid-core. grid-core
knows nothing about Tauri, the webview, or the event system, which is what keeps it
unit-testable in a plain `cargo test`.

## `crates/grid-core`

Modules as declared in `src/lib.rs`:

- `autoconfig` — the `ensure_*` writers that seed an emulator's own settings files so a
  launched game finds its saves, firmware, controller profile and RetroAchievements login
  where GRID expects them. Per-emulator submodules (`azahar`, `cemu`, `dolphin`,
  `duckstation`, `eden`, `pcsx2`, `ppsspp`, `redream`, `retroarch`, `rpcs3`, `xemu`) sit
  on shared `readers`/`writers`/`paths` helpers; `cores` owns RetroArch core lookup.
- `cloud` — the cloud save/state sync engine: scope classification, sync candidates,
  per-emulator directories, archive transfer, restore, retention, session windows, and
  xemu's raw-disk sync.
- `config` — `Config` and its TOML load/atomic save, the emulator entries and UI
  settings it holds, and `data_dir_override()`.
- `fatx` — clean-room FATX reader/writer for the `E:` partition of a raw xemu Xbox HDD
  image, so cloud sync can pull `E:/UDATA` and `E:/TDATA` out and put them back.
- `firmware` — server firmware download and install: keyword-based target routing, the
  zip keep-vs-extract decision, and the RPCS3 PS3 firmware path.
- `images` — the image pipeline: URL resolution rules, the on-disk cache, the startup
  sweep, the replenish job, background art, and cached video.
- `import_python` — one-shot import of a Python-era `~/.grid-launcher/config.json` into
  `config.toml` and the installed-games registry. Holds no token, by construction: the
  parsed struct has no field for one.
- `launch` — the launch core: emulator profiles and the install catalog, forge
  (GitHub/direct) downloads, argument templates, emulator selection per platform, native
  (Windows) launch and compat tools, process spawn and session tracking.
- `library` — the install pipeline: download queue, extraction, path rules, launch-file
  selection, the SQLite registry, platform rules, install specials (PS3, extra content),
  and update detection.
- `pcgw` — PCGamingWiki client that resolves a Windows game's save paths.
- `platform` — host-platform lookups that need OS APIs (for example the Shell-resolved
  Windows Documents folder).
- `retroachievements` — the RetroAchievements login client. Login only.
- `romm` — the RomM HTTP client: `RommClient`, platforms, games, ROM detail, firmware,
  byte fetches, and the cloud save/state endpoints. All request construction lives here;
  check `openapi.json` before changing any of it.
- `secrets` — `Credential`, the `SecretStore`/`RaTokenStore` traits, and `KeyringStore`,
  the OS keyring implementation. Secrets are `SecretString` values that cannot be
  formatted or logged.
- `session` — `SessionManager`: connect, restore, disconnect, and the shared
  `RommClient` and image cache handles the rest of the app reads.

Data files are compiled in with `include_str!` from the repository root:
`emulator-autoprofiles.json` (`launch/profiles.rs`), `retroarch-core-list.json` and
`romm-platform-cores.json` (`autoconfig/cores.rs`). Editing one of those JSON files is a
rebuild, not a restart.

## `app/src-tauri/src`

The shell owns *when* things run; grid-core owns *what* they do.

- `lib.rs` — startup: logging, the data directory, `SessionManager`, `InstallService`,
  `LaunchService`, the registry, the keyring stores, the Tauri builder, and the hook
  wiring in `.setup()`.
- `commands.rs` (+ `commands/cloud.rs`, `commands/specials.rs`, `commands/updates.rs`,
  `commands/logging.rs`) — the `#[tauri::command]` surface. Thin wrappers: they build a
  context, call a service or grid-core, and map the result to a serializable DTO.
- Services — `cloud_service.rs` (sync caches, auto-restore before launch, auto-upload
  after exit), `firmware_service.rs` (the background firmware triggers and their
  one-job-per-pass guard), `update_service.rs` (when the library is re-checked for server
  updates; never persisted), `images.rs` (startup sweep, replenish job, post-install
  cover prefetch), `media_server.rs` (a loopback HTTP server with Range support that
  feeds cached videos to the webview), `app_update.rs` (one `releases/latest` check per
  process, banner only, nothing downloaded), `python_import.rs` (the startup presence
  check for a Python-era config), `config_write.rs` (one config writer at a time, so two
  load-modify-save cycles cannot lose an update), `logging.rs` (the tracing filter and
  the `debug_prints` toggle), `gamepad/` (polling and the navigation events it emits).

**Hooks pattern.** grid-core services expose setters — `InstallService`'s
`set_game_finalized_hook`, `set_emulator_installed_hook`, `set_image_hook`,
`set_compat_tools_hook`, and `LaunchService::set_session_finished_hook`. `lib.rs`
installs closures on them at startup, and each closure body stays trivial: it hands the
work to an app-layer service. That is how a finished install can trigger a firmware pass,
a cover prefetch and an update recompute without grid-core knowing any of them exist.

**The `e2e` cargo feature.** `app` builds with an embedded WebDriver server behind the
`e2e` feature, and redirects forge requests to `GRID_LAUNCHER_E2E_FORGE_BASE` at request
time. Every other URL stays real. CI clippy-checks that feature separately.

## `app/src`

Svelte 5 with runes. `main.ts` mounts `App.svelte`, which shows `Connect.svelte` until a
session exists and `Shell.svelte` afterwards.

- `lib/api.ts` — the single `invoke` boundary. Every command wrapper and every payload
  type lives here, and the type field names mirror the Rust structs' serde output
  exactly, with no cosmetic renames.
- `lib/stores/*.svelte.ts` — rune-based state shared across views: `session`,
  `installed`, `downloads`, `sessions`, `updates`, `appUpdate`, `uiSettings`, `toasts`,
  `inputMode`, `lastViewed`, `compatTools`, `pythonImport`.
- Views — `Library.svelte`, `Server.svelte`, `Downloads.svelte`, `Emulators.svelte`,
  `Settings.svelte`, plus `Details.svelte` as a sub-view. `shell.ts` holds the view list
  and the session-phase mapping.
- Feature folders — `lib/details/`, `lib/downloads/`, `lib/emulators/`, `lib/settings/`,
  `lib/library/`, `lib/server/`, `lib/cards/`, `lib/focus/`. Each pairs a `.svelte`
  component with a pure `.ts` module holding the logic, and a `.test.ts` beside it. Put
  new logic in the pure module: that is what vitest can reach.
- `lib/icons.ts` — hand-authored 24×24 icon paths. The old app's `assets/svg/` files are
  deliberately not reused.

## `e2e`

`scripts/e2e.sh` builds an e2e-feature binary and runs WebdriverIO specs against it and a
mock RomM server. Structure:

- `scripts/e2e.sh` `STAGE_GROUPS` — the ordered list of `name:spec [spec...]` groups. One
  group = one fresh data directory and one mock server; each spec inside it is a separate
  `wdio run`, which is how a "relaunch the app" pair (`connect-restore-a/-b`) works.
- `e2e/specs/` — the specs. `e2e/fixtures*/` — per-group server fixtures.
  `e2e/seed/` — pre-seeded config, registry rows and stub binaries.
  `e2e/helpers/`, `e2e/wdio.conf.ts` — per-stage settings read from the environment
  (`E2E_SPEC`, `E2E_DATA_DIR`, `E2E_MOCK_URL`).
- `e2e/mock-romm/server.mjs` — the mock RomM server (fixtures, archive generation,
  throttling, an offline switch, a request-introspection endpoint).
  `e2e/mock-romm/mock-forge.mjs` — stands in for GitHub and a direct-download provider,
  so catalog scraping runs against genuine markup.

Usage and exit codes are in `BUILD.md`.

## User state

All of it lives in one directory: `config.toml`, `grid-launcher.db` (the SQLite registry
of installed games) and `covers/`. The location is the platform `ProjectDirs`
(`io.github` / `Sixdd6` / `grid-launcher`) directory, unless `GRID_LAUNCHER_DATA_DIR` is
set and non-empty, in which case everything moves under that path. The e2e harness relies
on that override; nothing else should.

Game files themselves go to the user-configured library path, not here.

## Security rule (normative)

Tokens and passwords live **only** in the OS keyring, and in memory only as redacting
types (`SecretString` / `Credential`). They must never reach a config file, a log line at
any level, an error message, an IPC payload, a test fixture, or console output.

`scripts/check_secret_hygiene.sh` enforces the mechanical half: `expose_secret()` is
allowed only at a fixed list of call sites, and committed fixtures must not contain
anything that looks like a real bearer token. The judgement half is on you — a new log
line that formats a URL, a header, or an error body is the usual way this rule breaks.

## Bundled tools

`app/src-tauri/tauri.windows.conf.json` ships `assets/tools/7z/` (7z.exe, 7z.dll, License.txt) as Tauri resources on Windows only. The NSIS installer places them beside the executable, where `library::extract::bundled_7z_windows_path` finds them before falling back to a 7-Zip on `PATH`. The AppImage carries no bundled tools.
