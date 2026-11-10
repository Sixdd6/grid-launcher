# TV mode: map of the existing app (2026-11-10)

What TV mode builds on, from a read of `feat/tv-mode` after the Game backups merge.

## Shell and startup
- `app/src/App.svelte` switches on `session.phase` (`lib/shell.ts` `SessionPhase`:
  loading / none / setup / shell): Connect, `setup/ChooseSystems.svelte`, or
  `<Shell>`; `LibraryUnreachableDialog` replaces everything when blocking. App-lifetime
  listeners are registered here (`initReplenishListener`, `initAppUpdate`,
  `initPythonImport`, `initLayoutMigration`, `initLibraryUnreachable`, `initLibraryMove`,
  `listenUnauthorized`, `initSetupRun`, `initUiSettings`); `initDownloads`,
  `initSessions`, `initCompatTools`, `initUpdates`, `initDrives` start in phase shell.
  `<Toast lead={setupCard}>` is outside the phase branch; its Open Downloads calls
  `shell?.show('downloads')`.
- Session state: `lib/stores/session.svelte.ts` (`restore`, `connect`, `retry`,
  `disconnect`, `useOffline`, `connectToServer`, `finishSetup`); unreachable and offline
  both end in phase shell with `connected:false`.
- Start view: `initialView(connected, offline, firstDrive)` in `lib/shell.ts` (server
  when connected, else library; offline picks the first ready drive). `Server.svelte`
  has `let discover = $state(true)`. No "Open GRID on" setting yet.
- `lib/Shell.svelte`: top bar (brand, `VIEWS`, update badge, `ThemeToggle`, status dot,
  session menu), four views kept mounted with `hidden`, `DownloadsFooter`, the single
  `BackupDialog`. `show(view)`; Settings has `show(page, {system})`. Mounts
  `initNotifications()`, `initEmulatorEvents()`, `refreshInstalled()`, `seedLastViewed`
  and the `cloud-upload-finished` / `cloud-restore-notice` toast listeners — a TV shell
  must get these too (move them to App.svelte, or mount once per mode).
- Theme: `lib/theme.ts` + `stores/uiSettings.svelte.ts` (`applyTheme` is the only writer
  of `data-theme`). Background art: `lib/BackgroundArt.svelte` + `stores/lastViewed`.
- Notifications: `stores/toasts.svelte.ts` (`pushToast`, limit 4, 10 s), `lib/Toast.svelte`
  (bottom-right, 340px), wording in `notifications/notices.ts`, `notifier.ts` (also
  `launchGame`, `installGame` wrappers), backup batch cards in `backups/notices.ts`.

## State and API
- `lib/api.ts` is the only `invoke` boundary (~150 wrappers + event-name constants).
- Reusable stores (`lib/stores/`): session, installed, downloads, sessions (running
  games), updates, appUpdate, uiSettings, toasts, drives, compatTools, emulatorUpdates,
  setupRun, libraryMove, libraryUnreachable, lastViewed, inputMode (pointer|keyboard;
  the gamepad kind was removed in 75dda40), coverShapes, pixelRatio, viewportFill.
- Logic still inside view components (extract before reuse): Server platforms/games/
  `primary()`/Details opening; Discover feed cache (`discover/Discover.svelte`
  `lastFeed`); Library `play()`, `drivePrimary()`; emulators/systems have no store (pure
  models in `systems/rows.ts`, `lib/emulators/*.ts`); cloud saves `details/cloud.ts`;
  achievements `details/achievements.ts`.
- Pure modules to reuse: `details/{actions,subject,media,tabs,overview,related}.ts`,
  `discover/{feed,rotation,filters}.ts`, `cards/*`, `library/{rail,drive,sort}.ts`,
  `library/selection.svelte.ts`, `backups/{dialog,drives,selection,notices,install,driveArt}.ts`.

## Tauri side
- `main.rs` → `lib.rs` (`AppState`, setup, `generate_handler!` ~line 653); commands in
  `commands.rs` + `commands/*.rs`; events beside their services (images-replenished,
  installed-changed, updates-changed, app-update-available, firmware-pass-finished,
  drives-changed, backup-notice, cloud-*, library-*, emulator-installed,
  downloads-changed, sessions-changed, setup-defaults, compat-tools-changed,
  session-unauthorized).
- One `main` window; fullscreen never set; capabilities grant only `core:default` and
  `dialog:allow-open` (fullscreen needs a command or `core:window:allow-set-fullscreen`);
  `tauri_plugin_window_state` restores size (may fight fullscreen; off under e2e).
- No CLI parsing (only env vars `GRID_LAUNCHER_*`); `--tv` is new code.
- Config `[ui]` = `grid-core/src/config.rs` `UiSettings`; read/write via
  `get_ui_settings` / `set_ui_settings` (`normalize_ui_settings`, `modify_config`). A new
  key goes in four places: the Rust struct + default, `normalize_ui_settings`, the
  `UiSettings` type in `api.ts`, and `payload()` in `uiSettings.svelte.ts`.

## Focus, Details, media
- `lib/focus/grid.ts` (`moveFocus`), `CardGrid.svelte` `neighbor(i, dir)` /
  `scrollToIndex(i)`; Server and Library use `<svelte:window onkeydown>` gated on
  `active`; `views/searchKeys.ts` treats any `[role="dialog"]` as an open dialog — a TV
  overlay using that role silences desktop handlers and vice versa.
- `Details.svelte` (fixed modal 1040×680) with Overview/Media/Saves/Achievements;
  `details/MediaViewer.svelte` (`fitMedia`).

## Removed gamepad code (75dda40, 2026-10-20)
- Rust: `app/src-tauri/src/gamepad/{mod.rs,mapping.rs}`, gilrs 0.11.0, thread
  "gamepad-poll", `Mapper::new(0.3 dead zone, 200 ms repeat)`, drained events then slept
  16 ms, re-polled the left stick; emitted `nav` with up/down/left/right/accept/back.
  Dropping gilrs also dropped libudev-dev from CI and the container (needed again).
- Frontend: `listen('nav')` in App.svelte, `Shell.handleNav`, view `handleNav`s,
  `focus/dialogNav.ts`. Added in b92824c, 0ac4ec2, bdb6089, b6f1414.
- Restore with `git show 75dda40^:app/src-tauri/src/gamepad/mod.rs` (and mapping.rs).
- Gaps vs the TV design: no focus/game-running gating, no shoulders/triggers/menu/view,
  no right stick, single-controller assumptions.

## Tests
- Vitest beside modules (`*.test.ts`, `*.svelte.test.ts`), Tauri mocked via
  `vi.mock('@tauri-apps/api/core')`.
- E2E: `scripts/e2e.sh` builds with `--features e2e`, runs `STAGE_GROUPS` (fresh data dir
  + mock server per group; each spec a separate `wdio run`), seeds `e2e/seed/*`,
  opt-in groups; `e2e/wdio.conf.ts` `'tauri:options': { application, args: [] }` is where
  a `--tv` spec adds the flag. Mock RomM `e2e/mock-romm/server.mjs` (+ `/__e2e__/offline`,
  `/revoke`, `/requests`), `mock-forge.mjs`.

## Hazards for a second UI
- Desktop window-level key handlers (Shell Ctrl+1..4/Escape, Server, Library, RowMenu)
  fire while their view is visible: don't mount the desktop Shell in TV mode.
- Business logic in view components must be extracted to shared modules first.
- Desktop layout is hard-coded (fixed modals, 340px toasts, `--topbar-h`, `#app` column).
- `inputMode` has no gamepad kind; hover logic assumes pointer/keyboard.
- Gamepad gating needs the `sessions` store (running games) and window focus events.
