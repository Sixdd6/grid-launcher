# TV mode: implementation plan (2026-11-10)

How to build TV mode on `feat/tv-mode` so the real app satisfies every item in
`docs/tv-mode/checklist.md` (1–528). Inputs: `docs/tv-mode/app-map.md` (the existing code),
`docs/design/system/components/TVMode/README.md` (its **Decided** section is binding), the
prototype `docs/design/prototype/tvmode.js` / `tvmode.css` (behaviour to reproduce), the
glyphs in `docs/design/prototype/glyphs/` with `map.json`, and the Game backups card.

Rules that hold in every phase:

- Dependency direction `app/src` → `app/src-tauri/src` → `crates/grid-core`; grid-core never
  depends on Tauri. Pure logic lives in `.ts` modules with a `.test.ts` beside them, and in
  Rust with `#[cfg(test)]` beside the code.
- Secrets never reach config, logs, IPC payloads or fixtures. The on-screen keyboard's masked
  fields hold the value only in component state and hand it straight to the existing
  `connect` / RetroAchievements commands, exactly as the desktop forms do.
- One implementer at a time in the shared tree; commit each phase with
  `git commit --only <paths>` after `scripts/ctr.sh gate` passes and the phase's e2e groups
  pass (`scripts/ctr.sh e2e <groups>`). Commit on the host.
- A phase that changes desktop behaviour on purpose (only P3 and the desktop half of P5) says
  so; every other phase leaves the desktop e2e groups green with no assertion changes.

---

## 1. Architecture

### 1.1 Two shells, one app

```
App.svelte
 ├─ startLifetime()                      (app-lifetime listeners, once, either mode)
 ├─ uiMode.mode === 'desktop'
 │    ├─ phase none/setup  → Connect / ChooseSystems           (existing)
 │    ├─ phase shell       → <Shell initialView=…>             (existing, slimmed)
 │    └─ <Toast>                                               (existing)
 └─ uiMode.mode === 'tv'
      └─ <TvShell>  (top bar, stage, hint bar, toasts, layers)
           ├─ phase none/setup → tv/startup/TvConnect / TvChooseSystems
           ├─ unreachable      → tv/startup/TvUnreachable
           └─ phase shell      → tabs Library / Server / Downloads / Settings + page stack
```

- `App.svelte` mounts **either** the desktop `Shell` **or** `TvShell`, never both. The desktop's
  window-level key handlers (Shell Ctrl+1..4/Escape, Server, Library, RowMenu, `searchKeys.ts`)
  therefore never run in TV mode, and TV's `[role="dialog"]` overlays never silence desktop
  handlers.
- `html[data-ui="tv"]` is set by `App.svelte` while TV is mounted. All TV CSS is scoped under
  it, including `html { font-size: min(100vh / 54, 100vw / 80) }` (item 2, ruling 1) and `overflow: hidden` on
  `html, body` (item 4), so the desktop's px-based layout is untouched.
- Switching mode unmounts one shell and mounts the other. Shared stores (session, installed,
  downloads, sessions, updates, drives, uiSettings, toasts, compatTools, emulatorUpdates,
  libraryMove, libraryUnreachable, lastViewed) keep their data, so the switch is instant and
  loses nothing. The desktop `Shell` gets an `initialView` prop so "Switch to desktop mode"
  lands on the same view (item 523).

### 1.2 Frontend modules

**Moved out of `Shell.svelte` (P1)**

- `app/src/lib/lifetime.ts` — `startShellLifetime()`: `initNotifications()`,
  `initEmulatorEvents()`, `refreshInstalled()`, `seedLastViewed`, the
  `cloud-upload-finished` / `cloud-restore-notice` toast listeners and the
  `systems/events.ts` notice. `App.svelte` calls it once when the phase first reaches `shell`,
  whatever the mode, and never tears it down on a mode switch.
- `app/src/lib/stores/navigation.svelte.ts` — mode-agnostic intents: `go(view)`,
  `goSettings(page, {system})`, `openDetails(subject)`. Each shell registers a handler on
  mount. Toast actions ("Open Downloads", "Set up <Platform>") call `navigation.go(...)` instead
  of `shell?.show(...)`, so the same notifier works in both modes.
- `app/src/lib/stores/uiMode.svelte.ts` — `mode`, `startedWithTv`, `enterTv(view)`,
  `leaveTv()`; talks to the Rust `ui_mode` commands; pure mapping in `app/src/lib/uiMode.ts`
  (`tvTabFor(desktopView)`, `desktopViewFor(tvTab, settingsPane)`).

**Business logic extracted from views (P1, P2) — both UIs call these**

| New module | Extracted from | Holds |
| --- | --- | --- |
| `lib/games/quick.ts` | Server `primary()`, Library `play()`/`drivePrimary()`, Details button state | pure: the quick action and its label for a game in context (Play / Install / Install from drive / none), the Details button set per state |
| `lib/games/run.ts` | Server, Library, Details handlers | effectful: `playInstalled`, `installServerGame`, `installFromDrive`, `cancelInstall`, `updateGame`, `uninstallGame`, `removeFromLibrary`, `locateGame`, `linkGame`, `stopGame`. Return outcomes (`{kind:'needs-umu'}`, `{kind:'needs-library-folder'}`, `{kind:'refused', reason}`) so each UI shows its own prompt |
| `lib/server/catalog.svelte.ts` | `Server.svelte` (`selectPlatform`, `loadLaunchDefaults`, `refreshFirmwareStatus`, `checkLibraryPath`) | platforms, per-platform games, launch defaults, firmware status |
| `lib/discover/feedStore.svelte.ts` | `discover/Discover.svelte` `lastFeed` | Discover feed load and cache |
| `lib/details/detailsStore.svelte.ts` | `Details.svelte` | ROM detail / media / related for a subject, install-state derivation, `linkTo`, `locate`, `removeFromLibrary`, drive install/remove |
| `lib/details/cloudStore.svelte.ts` | `details/CloudPanel.svelte` | records load, upload latest, restore, delete, request guard |
| `lib/details/nativeSettingsStore.ts` | `details/NativeSettings.svelte` | load / save / reload from server |
| `lib/systems/systemsStore.svelte.ts` | `systems/SystemsPage.svelte` (18 api calls) | refresh emulators/defaults/systems/pending, install, Install recommended, pick, core change, update, companion install, check updates, firmware install (incl. PS3), open, show folder, remove |
| `lib/emulators/formSave.ts` | `emulators/EmulatorForm.svelte` | add / save an emulator entry |
| `lib/settings/libraryChange.svelte.ts` | `settings/LibraryChangeDialog.svelte` | validate target, plan, Move existing files, Start fresh, delete old files |
| `lib/settings/raActions.ts` | `settings/RetroAchievementsPage.svelte` | status, save login, clear |
| `lib/backups/backupPlan.svelte.ts` | `backups/BackupDialog.svelte` | plan fetch, drive choice, source picks, start |
| `lib/backups/driveActions.ts` | `backups/DriveCard.svelte` | set up, rename, OS change, remove GRID files, forget, open folder |
| `lib/setup/chooseSystems.svelte.ts` | `setup/ChooseSystems.svelte` | load rows, Windows row, install names, install, skip |
| `lib/connectFlow.ts` | `Connect.svelte` | save library path, use without server, submit |
| `lib/downloads/actions.ts` | `Downloads.svelte` | run action, cancel / resume a library move |
| `lib/libraryUnreachable.ts` (existing pure half) | `LibraryUnreachableDialog.svelte` | relocate, close without changes |

The desktop components become thin views over these modules. Existing pure modules are reused
as-is: `details/{actions,subject,media,tabs,overview,related,achievements,cloud}.ts`,
`discover/{feed,filters}.ts`, `cards/*`, `library/{rail,drive,sort}.ts`,
`backups/{dialog,drives,selection,notices,install,driveArt}.ts`, `systems/rows.ts`,
`emulators/*.ts`, `notifications/notices.ts`, `downloads/format.ts`.

**The TV directory `app/src/lib/tv/`**

```
tv/
  TvShell.svelte  TvTopBar.svelte  TvHintBar.svelte  TvStage.svelte  TvBackground.svelte
  tv.css                     tokens + primitives, all in rem, scoped to html[data-ui="tv"]
  size.ts                    rootFontPx(vw, vh) = min(vh/54, vw/80) (ruling 1)
  theme.ts                   tvTheme(setting): Follow system → dark (ruling 22)
  clock.svelte.ts            minute clock
  nav.svelte.ts              tab, page stack, layer stack, per-screen focus memory
  hints.ts                   hint list builder: A, X, screen hints, B Back, Menu (items 128–133)
  topbar.ts                  top-bar download / connection / breadcrumb text
  notices.svelte.ts          TV card stack (≤3, pinned setup card, history for the page)
  focus/
    engine.ts                PURE spatial engine over rects (items 77–91)
    reveal.ts                PURE scroll targets (items 107–114)
    grid.ts                  PURE index moves for windowed grids
    dom.ts                   DOM adapter: collects [data-f], reads rects, applies .f, scrolls
    actions.ts               Svelte actions: use:focusable, use:navArea, use:scroller
  input/
    buttons.ts               logical buttons, KEYMAP (item 71)
    controllers.ts           PURE brand, default assignment, display name, positional→logical
    prompts.ts               PURE prompt set resolution (items 67–69)
    repeat.ts                PURE repeat scheduler per controller with injected clock (40–45)
    sticks.ts                PURE left-stick thresholds, right-stick scroll speed (35–37)
    press.ts                 PURE dispatch: (button, ScreenContext) → Command (92–103)
    router.svelte.ts         listens to the `gamepad` event, keydown, mouse; runs press()
  glyphs/                    Kenney SVGs listed in map.json + License.txt, glyphs.ts, Glyph.svelte
  ui/                        TvCover, TvButton, TvSwitch, TvSlider, TvChip, TvRail, TvGrid,
                             TvTrack (sideways row), TvRows (settings-style row list), TvPage
  rows.ts                    PURE row model: switch | choice | slider | text | action | info
  overlays/                  TvLayer, TvList, TvConfirm, TvKeyboard (+ osk.ts), TvMenu
  tabs/server/               TvServer, TvDiscover, TvPlatform, discoverRows.ts, serverHeader.ts
  tabs/library/              TvLibrary, libraryRail.ts, libraryHeader.ts
  tabs/downloads/            TvDownloads, downloadRows.ts
  tabs/settings/             TvSettings, panes/{appearance,cloud,connection,library,ra,systems,
                             tv,updates,drives}.ts (row models), TvSystemsPane
  pages/                     TvDetails (+ details.ts), TvPlaying, TvMediaViewer, TvSystemPage
                             (+ systemPage.ts), TvEmulatorForm, TvGameSettings, TvLinkPage,
                             TvFolderBrowser (+ folderBrowser.ts), TvSearchAll, TvNotifications,
                             TvBackupPage (+ backupPage.ts), TvDrivePage, TvDriveOsPage,
                             TvDriveSetupPage
  startup/                   TvConnect, TvChooseSystems, TvUnreachable, TvSetupCard (+ setupCard.ts)
  select.svelte.ts           TV select mode over lib/backups/selection.ts
```

**Focus engine design.** The prototype's model carries over as data attributes, and the pure
engine is the prototype's `move` / `restoreFocus` / `reveal` rewritten over plain rects:

- `data-f` focusable, `data-k` stable identity (game id, row key; never an index), `data-nav`
  on areas (`h`, `v`, `grid`), `data-enter="sel|first"` (rails land on the selected row),
  `data-edge` (Discover's rows area leaves only straight past its edges), `data-up` /
  `data-down` explicit links (Related → summary), `data-autofocus` (Cancel, the checked option).
- `engine.ts` exports `move(snapshot, dir)` and `restore(snapshot, memory)` where a snapshot is
  `{ items: {key, rect, disabled, area}[], areas: {key, axis, enter, rect, last, links}[],
  current }`. Score = gap along + 2.2 × offset across (item 81). The DOM adapter only builds
  the snapshot and applies the result, so every rule is unit-tested with synthetic rects.
- `nav.svelte.ts` keeps focus memory per tab, per page and per layer (`_k`), and each area's
  last item (`_last`), so B, tab switches and overlays restore focus by identity (items 79–80,
  87, 115).
- Windowed grids: the TV grid reuses `lib/cards/window.ts`. Inside a `grid` area, moves are
  index arithmetic (`focus/grid.ts`: columns from the measured width, `scrollToIndex` before
  focusing an unrendered card); only leaving the grid's edge falls back to the spatial engine.
  This keeps a 2500-game platform fast and correct.
- Redraws keep every scroll position: each scroller registers with `use:scroller` under a
  stable key; `nav.svelte.ts` saves positions per screen and restores them after a redraw
  (item 113). Svelte keyed `{#each}` keeps DOM identity, so most redraws never lose position.
- An install's progress updates only text nodes (`$derived` per cover tag / hero line / top
  bar), so nothing else redraws (item 118). A data change that removes the focused key moves
  focus to the enabled item now at the same position in the same area (item 115).

**Input design.** Rust sends positional, raw events; the frontend owns everything that depends
on what is on screen:

- `controllers.ts` maps positional face buttons (South/East/West/North) to logical A/B/X/Y by
  the controller's assignment (Xbox style: S=A E=B W=X N=Y; Nintendo style: E=A S=B N=X W=Y);
  shoulders, triggers, View, Menu, D-pad and sticks never move (items 49–55). Default assignment
  and brand come from vendor id and name (items 51–52, 67–68).
- `repeat.ts` keeps held state per controller id; directions repeat at 400 ms then 90 ms; on the
  on-screen keyboard the typing buttons repeat at 500 ms then 50 ms; releasing (or the Rust
  `released_all` event) clears at once (items 40–45).
- `press.ts` is the prototype's `press()` as a pure function returning a command
  (`move`, `activate`, `back`, `tab±`, `menu`, `mainButton`, `view`, `scroll`, …) given a
  `ScreenContext` (top layer, page stack depth, startup, select mode, focused item's flags).
- Keyboard and mouse feed the same router (items 71–76). The last controller pressed drives
  prompts, the hint-bar name and the Button assignments row (item 47).

### 1.3 Rust side

**Gamepad reader — `app/src-tauri/src/gamepad/`** (restored from `git show 75dda40^:…` as a
starting point, then rewritten)

- `mod.rs` — `GamepadService`: owns the thread `gamepad-poll`, started lazily the first time TV
  mode turns on (README: "for TV mode only") and kept for the process. Loop: drain
  `gilrs.next_event()`, map, pass through the gate, emit; sleep 8 ms. gilrs init failure (no
  `/dev/input` access, container, CI) is logged once at `warn` and leaves the service with no
  pads; the keyboard still works.
- `mapping.rs` (pure) — gilrs `Button`/`Axis`/`EventType` → `PadEvent`. Buttons: `South`,
  `East`, `West`, `North`, `LeftBumper`, `RightBumper`, `LeftTrigger`, `RightTrigger`
  (`LeftTrigger2`/`RightTrigger2`, with analog `ButtonChanged` thresholded at 0.5),
  `Select` (View), `Start` (Menu), `Guide`, `LeftStick`, `RightStick`, `DpadUp/Down/Left/Right`.
  Sticks: `stick` events for both sticks, sent when a value moves by ≥ 0.02, at most once per
  loop tick per stick.
- `identity.rs` (pure) — `PadInfo { id: u32, key: String, name: String, vendor: Option<u16>,
  product: Option<u16> }`; `key` is the gilrs UUID as hex (two identical models share a key,
  and therefore an assignment; noted in SPEC).
- `gate.rs` (pure) — `Gate { tv_active, window_focused, games_running }` with `open()`. A
  transition from open to closed emits `released_all`, so no repeat survives (items 31–33,
  44). Fed by: `set_ui_mode` (tv_active), `.on_window_event(WindowEvent::Focused)` in
  `lib.rs` (window_focused), and the `launch.set_notify` closure in `lib.rs` that already emits
  `sessions-changed` (games_running = snapshot not empty).
- Event `gamepad` (constant `GAMEPAD_EVENT` in `api.ts`), payload (serde tag `kind`):
  `connected {pad}`, `disconnected {id}`, `button {id, button, down}`,
  `stick {id, stick: "left"|"right", x, y}`, `released_all`.
- Commands (`app/src-tauri/src/commands/tv.rs`): `gamepad_list() -> Vec<PadInfo>`; under the
  `e2e` feature only, `e2e_gamepad_inject(events)` feeds synthetic gilrs-shaped input through
  the same mapping and gate (that is how e2e drives controllers, assignments, multiple pads and
  gating with no hardware).

**UI mode, window and helpers — `app/src-tauri/src/commands/tv.rs`, `app/src-tauri/src/cli.rs`**

- `cli.rs` (pure) — `parse_cli(args) -> CliOptions { tv: bool }`; `--tv` (item 522). Stored in
  `AppState`.
- `get_ui_mode() -> UiModeInfo { mode, started_with_tv, fullscreen }`;
  `set_ui_mode(mode)` — TV: remember the window's normal size/maximized state, then
  `set_fullscreen(true)`, open the gate's `tv_active`, start the gamepad thread; desktop:
  close `tv_active`, `set_fullscreen(false)`, restore size/maximized.
- `tv_yield_to_game()` — "Back to the game" (item 324): minimizes GRID's window so the game's
  window is in front; when the last session ends and TV is active, `lib.rs` un-minimizes and
  focuses GRID (item 33).
- `quit_app()` — Quit GRID (items 267, 376); `app.exit(0)` after the same shutdown the window
  close does.
- `list_folder(path, files) -> FolderListing { path, parent, entries: [{name, is_dir}] }` —
  the TV folder browser (items 349–352). Pure listing rules (dot-folders hidden, folders first,
  case-insensitive sort, files only in file mode) in a new `crates/grid-core/src/fsbrowse.rs`
  with tests; the command only resolves the start folder (home when empty).
- `read_clipboard_text()` — View pastes on Connect (item 396), through
  `tauri-plugin-clipboard-manager` with only `clipboard-manager:allow-read-text`. The text goes
  straight into the field; it is never logged.
- Window state: `tauri_plugin_window_state::Builder` gets
  `.with_state_flags(StateFlags::all() - StateFlags::FULLSCREEN)`, and `set_ui_mode(tv)` calls
  `save_window_state` before going fullscreen; on close while in TV, the saved pre-TV state is
  kept (skip the plugin's save, or restore normal size first). See risk R2.
- Capabilities: no new window permission is needed (fullscreen goes through commands); add the
  clipboard read permission to `capabilities/default.json`.

**Config — `[ui]` in `crates/grid-core/src/config.rs` `UiSettings`** (each key in the four
places: Rust struct + default, `normalize_ui_settings` in `commands.rs`, `UiSettings` in
`api.ts`, `payload()` in `stores/uiSettings.svelte.ts`)

| Key | Type / values | Default | Used by |
| --- | --- | --- | --- |
| `open_on` | `"automatic"`, `"discover"`, `"library"` | `"automatic"` | both modes (items 234, 413–416, 525–526) |
| `tv_prompts` | `"match"`, `"xbox"`, `"ps"`, `"nintendo"`, `"8bitdo"` | `"match"` | TV (items 67–69, 261) |
| `tv_assignments` | map pad key → `"xbox"` / `"nintendo"` | empty | TV (items 53, 262–264) |

Unknown strings round-trip like `theme`; `normalize_ui_settings` drops invalid map values. The
current mode is **not** persisted: GRID starts in desktop mode unless `--tv` is given
(item 524).

**Container and CI.** gilrs needs `libudev-dev` again, removed in 75dda40 from:
`scripts/container/Containerfile` (the apt line), `.github/workflows/build.yml` (the `check`
job line 33, the `e2e` job line 99, the `build-linux-packages` job line 290, and the AppImage
job's in-container apt line near 172), and `BUILD.md` prerequisites. Rebuild the images with
`scripts/ctr.sh build`. Windows (`check-windows`) needs nothing (gilrs uses XInput/WGI).

---

## 2. Phases

Each phase ends with `scripts/ctr.sh gate` green, then the listed e2e groups, then a commit with
`git commit --only`. "Desktop e2e" means every default group in `STAGE_GROUPS`
(`scripts/ctr.sh e2e`). New TV groups are added to `STAGE_GROUPS` in `scripts/e2e.sh` as they
appear; `e2e/wdio.conf.ts` reads `E2E_APP_ARGS` (space-separated) into
`'tauri:options'.args`, and `e2e.sh` sets `E2E_APP_ARGS=--tv` for groups whose name starts
with `tv-`. Shared TV helpers go in `e2e/helpers/tv.ts` (`press(button, pad?)` through
`e2e_gamepad_inject`, `key(k)`, `focused()` = the `data-k` of `.f`, `waitFocused(k)`,
`assertNoClipping()`, `hints()`).

### P1 — Shared logic extraction I: lifetime, navigation, game actions (no behaviour change)

- **Goal:** the shell-lifetime listeners and every game action leave the view components, so a
  second UI can call them.
- **Files:** new `lib/lifetime.ts`, `lib/stores/navigation.svelte.ts`, `lib/games/quick.ts`,
  `lib/games/run.ts`, `lib/server/catalog.svelte.ts`, `lib/discover/feedStore.svelte.ts`,
  `lib/downloads/actions.ts` (+ tests); changed `App.svelte`, `lib/Shell.svelte`,
  `lib/Server.svelte`, `lib/Library.svelte`, `lib/Details.svelte` (handlers only),
  `lib/discover/Discover.svelte`, `lib/Downloads.svelte`, `lib/Toast.svelte`,
  `lib/notifications/notifier.ts` (actions through `navigation`), `ARCHITECTURE.md`.
- **Checklist:** none directly (enables 1, 139, 172, 192, 277–290, 316–318).
- **Tests:** vitest for `quick.ts` (every state: installed, queued, installing, missing,
  unlinked, offline, drive), `run.ts` outcomes with `invoke` mocked, `navigation` handler
  registration, `catalog` and `feedStore` caching.
- **Exit:** gate; desktop e2e (all default groups), unchanged assertions.

### P2 — Shared logic extraction II: Details, Settings, Systems, backups, startup (no behaviour change)

- **Goal:** the rest of the business logic TV needs leaves components.
- **Files:** new `lib/details/detailsStore.svelte.ts`, `lib/details/cloudStore.svelte.ts`,
  `lib/details/nativeSettingsStore.ts`, `lib/systems/systemsStore.svelte.ts`,
  `lib/emulators/formSave.ts`, `lib/settings/libraryChange.svelte.ts`,
  `lib/settings/raActions.ts`, `lib/backups/backupPlan.svelte.ts`,
  `lib/backups/driveActions.ts`, `lib/setup/chooseSystems.svelte.ts`, `lib/connectFlow.ts`
  (+ tests); changed `Details.svelte`, `details/CloudPanel.svelte`,
  `details/NativeSettings.svelte`, `details/LinkPicker.svelte`, `systems/SystemsPage.svelte`,
  `emulators/EmulatorForm.svelte`, `settings/LibraryChangeDialog.svelte`,
  `settings/RetroAchievementsPage.svelte`, `backups/BackupDialog.svelte`,
  `backups/DriveCard.svelte`, `setup/ChooseSystems.svelte`, `Connect.svelte`,
  `LibraryUnreachableDialog.svelte`, `ARCHITECTURE.md`.
- **Checklist:** none directly (enables 241–260, 270–348, 389–412, 418–520).
- **Tests:** vitest per new module (store transitions with mocked `invoke`; the Systems store's
  Install recommended state "Installing N emulators…", item 253, is modelled here).
- **Exit:** gate; desktop e2e (all default groups), unchanged assertions.

### P3 — Config keys and "Where GRID opens" (desktop behaviour change)

- **Goal:** the new `[ui]` keys exist; both modes open by one rule; the desktop gets
  Settings › Appearance › Open GRID on.
- **Files:** `crates/grid-core/src/config.rs` (`open_on`, `tv_prompts`, `tv_assignments`),
  `app/src-tauri/src/commands.rs` (`normalize_ui_settings` + tests), `lib/api.ts`,
  `lib/stores/uiSettings.svelte.ts`, `lib/shell.ts` (`initialView(connected, offline,
  reachable, firstDrive, openOn)` → view + selection), `lib/Shell.svelte` (no more
  `discover = true` regardless), `lib/Server.svelte` (forced Discover with no server shows the
  not-connected panel), `lib/settings/AppearancePage.svelte` ("Open GRID on" first in the pane, ruling 23), `SPEC.md`.
- **Checklist:** 413–416 (rule, shared), 525, 526, 527 (TV reads no card settings; asserted
  again in P10).
- **Tests:** Rust round-trip and normalisation for the three keys; vitest `initialView` truth
  table (automatic × connected / unreachable / no server with and without a drive; forced Discover;
  forced Library). E2E (desktop Appearance shows Open GRID on first): new group `open-on` (`open-on-a.spec.ts` connected → Discover;
  `open-on-b.spec.ts` server offline via `/__e2e__/offline` → Library; `open-on-c.spec.ts`
  forced Library and forced Discover offline). Update `discover`, `connect-restore` and
  `backups` (spec c, no server → first drive) where they assumed "always Discover".
- **Exit:** gate; e2e `open-on discover connect-restore backups library`, then all default
  groups.

### P4 — Rust: controller reader, gate, UI mode, fullscreen, `--tv`, helpers; libudev-dev

- **Goal:** everything native TV needs, testable without a UI.
- **Files:** `scripts/container/Containerfile`, `.github/workflows/build.yml` (4 apt lines),
  `BUILD.md`; `app/src-tauri/Cargo.toml` (`gilrs = "0.11"`, `tauri-plugin-clipboard-manager`),
  new `app/src-tauri/src/gamepad/{mod,mapping,identity,gate}.rs`, `app/src-tauri/src/cli.rs`,
  `app/src-tauri/src/commands/tv.rs`; changed `app/src-tauri/src/lib.rs` (AppState, CLI,
  window-event focus, sessions notify → gate, un-minimize on last session end, window-state
  flags, handler list), `app/src-tauri/capabilities/default.json`,
  `crates/grid-core/src/fsbrowse.rs` + `lib.rs`; `lib/api.ts` wrappers and types
  (`PadInfo`, `PadEvent`, `UiModeInfo`, `FolderListing`), `ARCHITECTURE.md`.
- **Checklist:** 30, 31, 32, 33 (native half), 34 (raw inputs), 522 (flag parsed), 524 (no
  persisted mode).
- **Tests:** Rust unit: `mapping` (every button, analog triggers at 0.5, stick throttle),
  `gate` (each input closes it; open→closed yields `released_all`; reopening after a game
  ends), `identity` (key, name), `cli` (`--tv`, unknown args ignored), `fsbrowse` (sorting,
  dot-folders, file mode, root has no parent) in a temp dir. E2E: none yet (no TV UI); the
  `e2e` clippy pass covers `e2e_gamepad_inject`.
- **Exit:** `scripts/ctr.sh build` (new image), gate, desktop e2e smoke `connect library launch`
  (startup path changed).

### P5 — TV shell, focus engine, input, glyphs, primitives; entering and leaving

- **Goal:** a working, empty-tabbed TV mode you can enter with the desktop TV button or `--tv`,
  drive with keyboard, mouse or injected controllers, and leave.
- **Files:** `lib/stores/uiMode.svelte.ts`, `lib/uiMode.ts`, `App.svelte` (mode branch,
  `data-ui`), `lib/Shell.svelte` (TV button in the top bar, `initialView` prop); `lib/tv/`:
  `TvShell`, `TvTopBar`, `TvHintBar`, `TvStage`, `TvBackground`, `tv.css`, `size.ts`,
  `clock.svelte.ts`, `theme.ts`, `nav.svelte.ts`, `hints.ts`, `topbar.ts`, `focus/*`, `input/*`,
  `glyphs/*` (copy the 39 SVGs listed in `map.json` and `License.txt`; no Xbox guide glyph),
  `ui/{TvCover,TvButton,TvSwitch,TvSlider,TvChip,TvRail,TvGrid,TvTrack,TvRows,TvPage}.svelte`,
  `rows.ts`; tab placeholders `tabs/*/Tv*.svelte` rendering a rail + empty pane.
- **Checklist:** 2–29 (shell, sizing, cover, ring, primitives, background art, theme,
  reduced motion), 30–41, 44–47, 49–52, 54–73, 75, 76 (except overlay click-outside),
  77–91, 93, 95–99, 100–101 (the page main-button hook), 105–107, 110–116, 119–138,
  319–320 (page frame and breadcrumb), 517 (top/hint bar without a server), 521, 522, 523,
  524.
- **Tests:** vitest — `engine.ts` (score rule; along-axis containment; list end carries on in
  its column; rail `enter=sel` after redraw; `_last`; grid edges only straight past; links win;
  disabled skipped on restore), `reveal.ts` (1.2rem inset, item taller than area, first/last
  to the ends, sideways row edge margin, shelf title to top), `grid.ts`, `controllers.ts`
  (brand/assignment table from items 51–52, 67–68; name stripping, item 38), `prompts.ts`,
  `repeat.ts` (fake clock: 400/90, release, per-pad isolation, `released_all`), `sticks.ts`
  (0.5 left, 0.25 right, speed ∝ tilt), `press.ts` (B rules 93–98, LB/RB wrap 98–99,
  Start main button vs Menu 100–102), `hints.ts`, `topbar.ts`, `size.ts`, `glyphs.ts`
  (every set has all 13 buttons, files exist, labels). E2E: new group
  `tv-shell` — `tv-shell-a.spec.ts` (`--tv`: fullscreen reported by `get_ui_mode`, three
  bands with their heights at 1080p, root font = vh/54, window never scrolls, tab switching
  with injected LB/RB and Q/E, mouse hover/click, hint clicks, prompts follow the last pad,
  Nintendo pad gets swapped face buttons, assignment saved per pad), `tv-shell-b.spec.ts`
  (desktop TV button → TV on the matching tab; Switch to desktop → same view; restart without
  `--tv` opens desktop).
- **Exit:** gate; e2e `tv-shell` plus desktop `library discover` (Shell changed).

### P6 — Overlays and notifications

- **Goal:** every overlay and the TV notification stack.
- **Files:** `lib/tv/overlays/{TvLayer,TvList,TvConfirm,TvKeyboard,TvMenu}.svelte`,
  `overlays/osk.ts`, `lib/tv/notices.svelte.ts`, `lib/tv/TvToasts.svelte`,
  `lib/tv/pages/TvNotifications.svelte`; changed `TvShell`, `input/press.ts` (overlay rules),
  `input/repeat.ts` (keyboard typing repeats), `lib/stores/toasts.svelte.ts` (a subscriber hook
  so TV renders the same pushed cards with its own limit; desktop unchanged).
- **Checklist:** 42, 43, 74, 76 (click outside a non-blocking overlay), 92, 102, 103, 117,
  139–150, 356–359, 360–388. Rulings: the "Welcome to TV mode" card shows each time TV mode
  opens, once the main screen is up (15); Menu › Notifications keeps the whole session, no cap
  (16); the keyboard stops at the hint bar, which shows the keyboard's hints (20).
- **Tests:** vitest — `osk.ts` (caret moves, Shift one letter, symbols, masked shows dots and
  opens empty, Done/cancel values, physical Enter only after physical typing), `notices`
  (max 3, oldest unpinned dropped, newest under pinned, 10 s / 15 s, history), `press.ts`
  overlay suppression of X/Y/LT/RT/LB/RB/Menu/View; Guide maps to nothing (ruling 5). E2E: `tv-shell-c.spec.ts` (Menu opens
  with items and counts, Quit confirmation with Cancel focused, a list opens on its checked
  option, OSK typing and held-X repeat via inject down/up, View acts on the newest
  actionable card, Notifications page lists and clears).
- **Exit:** gate; e2e `tv-shell`.

### P7 — Server tab: Discover, platforms, offline panels, Search everything

- **Goal:** the Server tab end to end.
- **Files:** `lib/tv/tabs/server/{TvServer,TvDiscover,TvPlatform}.svelte`,
  `discoverRows.ts`, `serverHeader.ts`, `lib/tv/pages/TvSearchAll.svelte`; uses
  `catalog.svelte.ts`, `feedStore.svelte.ts`, `games/quick.ts`, `games/run.ts`.
- **Checklist:** 104, 108, 109, 118, 157–175, 196–208, 353–355, 417 (TV landing on Discover's
  first cover), 518, 519. Rulings: sideways rows keep 1.2rem (3); Search everything shows results
  on Done, and Y on the results page reopens the keyboard with live results from then on (17).
- **Tests:** vitest — `discoverRows.ts` (row order and contents from the feed: Continue
  playing, New releases six months, Recently added minus New releases, Updates only when any,
  Your library; "Nothing here yet."), hero line and `ago` texts (items 161–162),
  `serverHeader.ts` counts and chips (197–202, natives have no chips). E2E: group
  `tv-server` on the `discover` fixtures — arriving on Server focuses Discover content; shelf
  title scrolls to top; Up/Down at Discover's edges stay put; Left to rail; platform header
  counts while searching; X installs (notice + Open Downloads); offline panel with Try again
  (`/__e2e__/offline`); `assertNoClipping()` after each move.
- **Exit:** gate; e2e `tv-shell tv-server`.

### P8 — Library tab

- **Goal:** the Library rail with folders and the grid.
- **Files:** `lib/tv/tabs/library/{TvLibrary.svelte,libraryRail.ts,libraryHeader.ts}`; uses
  `library/{rail,drive,sort}.ts`, `library.ts`, `games/*`.
- **Checklist:** 176–195, 420 (drive rows and "No Backup drives connected" in the rail).
- **Tests:** vitest — `libraryRail.ts` (counts for All/Recent 30 days/Updates/platforms;
  folders open by default, remembered per session; chevron and A label), `libraryHeader.ts`
  (empty-state texts 189–191, sort names). E2E: group `tv-library` on the `library` fixtures —
  moving through the rail filters at once and clears search; A on a folder header closes it
  and the header still names the filter; Up from the top row stays put; Left to the selected
  rail row; LT sort list; Y search narrows live; an install finishing flips every cover's tag
  to the installed dot.
- **Exit:** gate; e2e `tv-shell tv-library`.

### P9 — Details and its pages: Playing, media viewer, Saves, Link, Game settings, folder browser

- **Goal:** Details and every page pushed from it.
- **Files:** `lib/tv/pages/{TvDetails,TvPlaying,TvMediaViewer,TvGameSettings,TvLinkPage,
  TvFolderBrowser}.svelte`, `pages/details.ts`, `pages/folderBrowser.ts`; uses
  `detailsStore`, `cloudStore`, `nativeSettingsStore`, `games/run.ts`, `list_folder`,
  `tv_yield_to_game`, `stop_game`.
- **Checklist:** 11 (art on Details/Playing), 270–318, 321–325, 342–352. Rulings: Playing's
  buttons answer mouse and keyboard while the game runs (controller gated), and the page closes
  by itself when the game exits (7); an empty Details tab focuses the first button (10).
- **Tests:** vitest — `details.ts` (left line per state 272–273, 281–282; header line; button
  row per state incl. Cancel install · n% and Offline-disabled Install; tab list with
  Achievements only when listed; summary read-through stepping), `folderBrowser.ts` (Up one
  folder absent at root, file mode has no Y). E2E: group `tv-details` on `install` +
  `cloud-saves` + `native` fixtures (one spec each, `tv-details-a/b/c.spec.ts`) — LB/RB tab
  wrap and first-item focus; Up from Related returns to the summary; right stick (inject
  `stick right y`) scrolls without moving focus; Play pushes Playing, gate ignores injected
  input while the stub game runs, Stop (keyboard) asks and returns, and Playing pops by
  itself when the stub exits; Uninstall confirm with Cancel
  focused; Saves upload / restore / delete with focus moving to the entry in its place;
  Locate… through the folder browser; Link page search; Game settings save.
- **Exit:** gate; e2e `tv-shell tv-details`, desktop `launch cloud-saves native`
  (stores shared).

### P10 — Downloads and Settings panes (all but Systems)

- **Goal:** the Downloads tab and Settings › Appearance, Backup drives (rail row; pane in
  P13), Cloud saves, Connection, Library, RetroAchievements, TV mode, Updates.
- **Files:** `lib/tv/tabs/downloads/{TvDownloads.svelte,downloadRows.ts}`,
  `lib/tv/tabs/settings/TvSettings.svelte`,
  `tabs/settings/panes/{appearance,cloud,connection,library,ra,tv,updates}.ts`; uses
  `downloads/actions.ts`, `libraryChange.svelte.ts`, `raActions.ts`, `appUpdate`, logging
  commands, `uiSettings` (prompts / assignments), `TvFolderBrowser`.
- **Checklist:** 10, 12, 13 (live from the pane), 48, 53 (UI), 70, 209–224, 225–251,
  261–269, 527 (the info row; TV ignores card settings). Rulings: Cancel in Downloads asks
  first for every row kind (install, backup, library move) (8); RetroAchievements › Clear acts
  at once (9); Theme is shared, TV shows Follow system as dark (22).
- **Tests:** vitest — `downloadRows.ts` (segments, status lines, A labels and action lists per
  state 213–217), each pane's row model (labels, lines, slider ranges/steps/formatting
  237–238, 242; Connection credential line; Library change errors 246; TV mode lines naming
  the controller 262–265). E2E: group `tv-settings` — Downloads cancel/retry/dismiss and Y
  clear with focus kept; Settings rail shows each pane at once; a slider adjusts with
  Left/Right; Theme applies at once; Button prompts change redraws hints; Button assignments
  stored per injected pad and the row renames on pad change; Disconnect returns to Connect;
  Change library folder → Start fresh › Leave (reusing `install-c`'s flow).
- **Exit:** gate; e2e `tv-shell tv-settings`, desktop `install downloads`.

### P11 — Settings › Systems, System page, emulator form

- **Goal:** emulators and systems from a controller.
- **Files:** `lib/tv/tabs/settings/{TvSystemsPane.svelte,panes/systems.ts}`,
  `lib/tv/pages/{TvSystemPage.svelte,systemPage.ts,TvEmulatorForm.svelte}`; uses
  `systemsStore.svelte.ts`, `emulators/*`, `formSave.ts`, `systems/rows.ts`,
  `systems/proton.ts`, `TvFolderBrowser` (file mode).
- **Checklist:** 200 (chip opens System page, verified here), 252–260, 326–341. Ruling 19:
  Open <emulator> opens its window over TV mode, no mode switch.
- **Tests:** vitest — `systems.ts` (order: none first, then with, then Windows; status pill;
  X label; Other emulators note), `systemPage.ts` (rows per system kind: RetroArch core,
  firmware, Windows, none). E2E: group `tv-systems` on the `systems` fixtures + mock forge —
  Install recommended (N) runs, reads "Installing N emulators…", and focus moves to the
  control in its place when it goes; pick from the Download group installs; Remove asks and
  goes back; Add manually through the file-mode browser; update check notice.
- **Exit:** gate; e2e `tv-shell tv-systems`, desktop `systems emulators`.

### P12 — Startup screens and where TV opens

- **Goal:** first run, Choose your systems with the setup card, the unreachable library, and
  the start rule in TV.
- **Files:** `lib/tv/startup/{TvConnect,TvChooseSystems,TvUnreachable,TvSetupCard}.svelte`,
  `startup/setupCard.ts`; changed `TvShell` (phase branch, startup top bar), uses
  `connectFlow.ts`, `chooseSystems.svelte.ts`, `libraryUnreachable.ts`, `read_clipboard_text`.
- **Checklist:** 97, 123, 132, 133, 151–156, 367 (Menu unavailable at startup), 389–412,
  413–417 (in TV), 515, 516. Rulings: on Connect, Start presses Connect and, while it is
  disabled, says what is missing; on Choose your systems Start skips (6); Close without
  changing anything quits GRID at once (18).
- **Tests:** vitest — `setupCard.ts` (segments per unique emulator, half fills, failed red,
  titles 154, step line 155, countdown only after the last install 156), Connect enablement
  texts (397). E2E: group `tv-startup` (`tv-startup-a.spec.ts` fresh profile with `--tv`:
  Connect via OSK, Start presses Connect (and names what is missing when disabled), View pastes
  (clipboard seeded with a non-secret URL), rejected token
  returns with the reason, Choose systems with Start skipping, Install N with the setup card;
  `tv-startup-b.spec.ts` unreachable library: B does nothing, Relocate through the browser
  (a second seeded run checks Close quits with no question);
  `tv-startup-c.spec.ts` Use without a server opens the Library on the first drive).
  Fixtures: reuse `first-run-setup` and `library-move-c`.
- **Exit:** gate; e2e `tv-shell tv-startup`.

### P13 — Game backups in TV mode

- **Goal:** the whole Game backups card on a controller.
- **Files:** `lib/tv/select.svelte.ts`, `lib/tv/pages/{TvBackupPage,TvDrivePage,TvDriveOsPage,
  TvDriveSetupPage}.svelte`, `pages/backupPage.ts`, `tabs/settings/panes/drives.ts`; changed
  `TvLibrary` (drive grid/header, X on platform rows), `TvPlatform` (Backup all N…, select
  mode), `TvDetails` (drive buttons, Backup to drive…), `TvDownloads` (backup rows, removed
  drive), `TvSettings` (Backup drives pane); uses `backupPlan.svelte.ts`, `driveActions.ts`,
  `backups/*`, `games/run.ts` (`installFromDrive`).
- **Checklist:** 94, 418, 419, 421–514, 520 (and verifies 420). Rulings: per-game rows inline
  up to 6 games, behind "Show each game (n)" above that (11); the space-set-aside line, the
  disabled Firmware reason, the Wine prefix note and the FAT32 over-4 GB note in the desktop's
  wording (12); Install from drive uses the Game backups card's notices (game card "Copying from
  <label>", emulator card "<emulator> from <pack> added to download queue" then "<emulator>
  installed · Used for <system>", firmware "<System> firmware installed from <pack>") (13);
  Set up for GRID starts with this computer's OS ticked (14); turning a computer off on a pack
  asks first (9); a running backup shows in the top bar as title, bar, percent (21).
- **Tests:** vitest — `backupPage.ts` (sections and their visibility rules 465–478; main
  button label and why 480–481; footer sizes 479; GB-card wording 469, 472), `drives.ts` pane
  rows (495–498), `select.svelte.ts` (pick, select all/clear, Y with nothing picked, offline,
  leave on act or unplug). E2E: group `tv-backups` (fake drives, like `backups`):
  `tv-backups-a` set up a drive (Start presses Set up for GRID), RT select two games, Y →
  Backup page, Start backs up, Downloads rows read the backup states;
  `tv-backups-b` unplug mid-backup (failed row, folded Completed row, one error card, rail
  falls back to All games); `tv-backups-c` no server: Install from drive with emulator from the
  drive, Details from a drive, Remove from drive…; `tv-backups-d` Remove GRID files asks twice,
  Change… refuses the last computer, Forget a remembered drive. Seeds from
  `e2e/seed/backups-seed.mjs`.
- **Exit:** gate; e2e `tv-shell tv-backups`, desktop `backups`.

### P14 — TV suite hardening, layout sweep, docs, .rpm

- **Goal:** prove the whole checklist, at every resolution, and hand over a native test build.
- **Files:** `e2e/specs/tv-layout.spec.ts`, `e2e/helpers/tv.ts` (`assertNoClipping`,
  `assertMinText`), `scripts/e2e.sh` (group `tv-layout` run at `E2E_WINDOW=1280x720`,
  `1920x1080`, `3840x2160`), `SPEC.md` (TV mode section), `ARCHITECTURE.md` (`lib/tv/`,
  `gamepad/`, `cli.rs`, `commands/tv.rs`, `fsbrowse`), `BUILD.md` (`--tv`, controller
  permissions), `README.md` (TV mode, glyph credit), `docs/tv-mode/checklist.md` ticked.
- **Checklist:** 1, 3 (minimum text sweep), 4, 7, 26, 113, 114, 528, and a final re-verify of
  every item (each spec cites the item numbers it asserts in a comment).
- **Tests:** `tv-layout` walks every tab, page and overlay with injected input at three sizes
  and asserts: no focused element is clipped by any scroll ancestor (the prototype's clipping
  checker: for the focused element's ring box, inflated by 0.42rem, every ancestor with
  `overflow != visible` must contain it), no text below 0.85rem (computed `font-size`), the
  document never scrolls, bands keep their rem heights. Screenshots per screen under
  `ctr-out/tv-screens/` (opt-in, like `ui-screens`). Then the full default e2e run.
- **Exit:** gate; full `scripts/ctr.sh e2e` (desktop and all `tv-*` groups);
  `scripts/ctr.sh packages <ver>`, unpack the .rpm in `ctr-out/` (`rpm2cpio … | cpio -idm`)
  and hand over `usr/bin/grid-launcher` for manual testing with a real controller
  (`grid-launcher --tv`): Xbox, PlayStation and an 8BitDo/Nintendo pad, two at once, Wayland
  and X11.

---

## 3. Coverage table

Every checklist item is owned by exactly one phase (later phases may re-verify).

| Items | Section | Phase |
| --- | --- | --- |
| 1 | Reaches every feature | P14 |
| 2–29 | Shell & sizing | P5 (3, 4, 7, 26 re-verified in P14) |
| 30–32 | Controller reading, gating | P4 (consumed in P5) |
| 33 | Listens again after a game | P4 (Rust) + P9 (save notices on Playing) → owner P9 |
| 34–39 | Logical inputs, sticks, names, disconnects | P5 |
| 40–41 | Repeat | P5 |
| 42–43 | Keyboard typing repeat | P6 |
| 44–47 | Release, multiple controllers | P5 |
| 48 | TV pane redraws on pad change | P10 |
| 49–52, 54–56 | Button assignments | P5 |
| 53 | Assignment stored per controller | P10 (key in P3) |
| 57–69 | Prompts & glyphs | P5 |
| 70 | Redraw on prompt/assignment change | P10 |
| 71–73 | Keyboard fallback | P5 |
| 74 | Physical keys on the OSK | P6 |
| 75–76 | Mouse | P5 (click-outside in P6) |
| 77–91 | Focus & movement | P5 |
| 92 | B closes overlay | P6 |
| 93 | B pops a page | P5 |
| 94 | B leaves select mode | P13 |
| 95–99 | B / LB / RB rules | P5 (97 in P12) |
| 100–101 | Start presses main button | P5 (verified P12, P13) |
| 102–103 | Start opens Menu; overlay suppression | P6 |
| 104 | Arriving on Server focuses Discover | P7 |
| 105–107 | Rails, selected row, scroll rules | P5 |
| 108–109 | Discover scroll and hero height | P7 |
| 110–116 | Scroll and redraw rules | P5 (113–114 re-verified P14) |
| 117 | Redraw waits for overlay | P6 |
| 118 | Only percent text updates | P7 |
| 119–122, 124–126 | Top bar | P5 |
| 123 | Startup top bar | P12 |
| 127–131, 134–138 | Hint bar | P5 |
| 132–133 | Startup hints | P12 |
| 139–150 | Notifications | P6 |
| 151–156 | Setup card | P12 |
| 157–175 | Server › Discover | P7 |
| 176–195 | Library | P8 |
| 196–208 | Server platforms | P7 (200 verified in P11) |
| 209–224 | Downloads | P10 |
| 225–251 | Settings (general panes) | P10 |
| 252–260 | Settings › Systems | P11 |
| 261–269 | TV mode, Updates panes | P10 |
| 270–318 | Details | P9 |
| 319–320 | Page frame | P5 |
| 321–325 | Playing | P9 |
| 326–341 | System page, emulator form | P11 |
| 342–352 | Game settings, Link, folder browser | P9 |
| 353–355 | Search everything | P7 |
| 356–359 | Notifications page | P6 |
| 360–366, 368–388 | Overlays | P6 |
| 367 | No Menu on startup | P12 |
| 389–412 | Startup screens | P12 |
| 413–416 | Where GRID opens | P3 (rule) — TV landing verified P12 |
| 417 | Discover first cover | P7 |
| 418–419 | Game backups general | P13 |
| 420 | Drive rows in the rail | P8 (verified P13) |
| 421–514 | Game backups screens | P13 |
| 515–516 | Use without a server on Connect | P12 |
| 517 | Offline · no server bars | P5 |
| 518–519 | Server tab without a server | P7 |
| 520 | Everything works without a server | P13 |
| 521–524 | Entering and leaving | P5 (522 parsed in P4) |
| 525–526 | Where GRID opens (desktop) | P3 |
| 527 | Card settings desktop-only | P3 (info row in P10) |
| 528 | Ships together on feat/tv-mode | P14 |

### Conflict rulings (README § "Consultant decisions (2026-11-10)")

The 24 conflicts are decided; the plan builds these rulings. Each lives in one place, so an
overturn by the user is a local change.

| # | Ruling | Lives in | Phase |
| --- | --- | --- | --- |
| 1 | Root font min(vh/54, vw/80) | `tv/size.ts`, `tv.css` | P5 |
| 2 | No text under 0.85rem (TV tag, cover tags, badges, Latest, facts labels, hero platform, headings raised) | `tv.css` tokens; `assertMinText` | P5, P14 |
| 3 | Sideways rows keep 1.2rem (every row sits in a panel) | `TvTrack` padding, `reveal.ts` | P5, P7 |
| 4 | 8BitDo View/Menu are − / + (labels "Minus", "Plus") | `glyphs.ts` | P5 |
| 5 | Guide does nothing | `controllers.ts` (Rust still reports it) | P5 |
| 6 | No Menu at startup; Start presses Connect (says what's missing when disabled); skips on Choose systems | `press.ts` startup branch, `TvConnect` main button | P12 |
| 7 | Controller gated while a game runs; Playing answers mouse/keyboard; Playing closes when the game exits | `gate.rs`, `TvPlaying` | P4, P9 |
| 8 | Downloads Cancel asks for every row kind | `downloadRows.ts` action list | P10 |
| 9 | RA Clear acts at once; turning a pack computer off asks | `panes/ra.ts`, `TvDriveOsPage` | P10, P13 |
| 10 | Empty Details tab focuses the first button | `details.ts` | P9 |
| 11 | Backup page per-game rows inline up to 6, else "Show each game (n)" | `backupPage.ts` | P13 |
| 12 | Set-aside line, disabled Firmware reason, Wine note, FAT32 note (desktop wording) | `backupPage.ts` over `backups/dialog.ts` | P13 |
| 13 | Install-from-drive notices from the Game backups card | `lib/backups/install.ts` (shared with desktop) | P13 |
| 14 | Set up for GRID ticks this computer's OS | `TvDriveSetupPage` | P13 |
| 15 | "Welcome to TV mode" card each time TV opens, after the main screen is up | `TvShell` mount | P6 |
| 16 | Whole-session notice history, no cap | `notices.svelte.ts` | P6 |
| 17 | Search everything: results on Done; Y on results reopens the keyboard, then live | `TvSearchAll` | P7 |
| 18 | Unreachable › Close quits at once | `TvUnreachable` → `closeWithoutChanges` | P12 |
| 19 | Open <emulator> opens over TV mode, no mode switch | `systemPage.ts` | P11 |
| 20 | OSK stops at the hint bar; the bar shows the keyboard's hints | `tv.css` `.osk`, `hints.ts` | P6 |
| 21 | A running backup shows in the top bar as title, bar, percent | `topbar.ts` | P5 (verified P13) |
| 22 | Theme shared; TV shows Follow system as dark | `tv/theme.ts` `tvTheme()` | P5 |
| 23 | Desktop Appearance shows Open GRID on first; key `[ui] open_on` = automatic, discover, library | `config.rs`, `AppearancePage.svelte` | P3 |
| 24 | No controller: "No controller · the keyboard works too" | `hints.ts` `hintLeft()` | P5 |

Open questions the README leaves for the user (not blocking; no phase depends on them): a
controller-only way out of TV mode from Connect; whether a controller press should reach Playing
while GRID's own window is focused (would be a `gate.rs` flag); the welcome card every time or
only the first time (one condition in `TvShell`).

---

## 4. Risks and how each is verified

| # | Risk | Mitigation | Verification |
| --- | --- | --- | --- |
| R1 | WebKitGTK / GTK fullscreen: `set_fullscreen` behaves differently on GNOME Wayland, KDE, X11; under Xvfb (no window manager) it may not resize at all | Fullscreen through the Tauri window, never the HTML Fullscreen API; layout depends only on the viewport, so a non-fullscreen window still renders correctly | e2e asserts `get_ui_mode().fullscreen` (the flag), and layout at `E2E_WINDOW` sizes; manual check on the .rpm under GNOME Wayland, KDE Wayland and an X11 session (P14) |
| R2 | `tauri_plugin_window_state` saving the fullscreen geometry, or restoring fullscreen on a desktop start | FULLSCREEN removed from its flags; state saved before entering TV; close while in TV keeps the pre-TV state | Manual + Rust test of the save decision: desktop at 1280×800 → TV → quit → relaunch without `--tv` opens at 1280×800, not fullscreen (P5, re-checked P14) |
| R3 | gilrs on Linux needs read access to `/dev/input/event*` (logind `uaccess` usually grants it for joysticks); Steam Input may hide or duplicate pads; no udev in the container | Init failure is non-fatal and logged once; keyboard keeps working; the hint bar shows "No controller · the keyboard works too" | e2e runs with no devices (proves graceful start) and drives input through `e2e_gamepad_inject`; manual test with Steam running and not running; BUILD.md notes the udev rule if a distro lacks `uaccess` |
| R4 | gilrs on Wayland focus: gilrs reads devices globally, so input arrives while GRID is not focused | The gate's `window_focused` from `WindowEvent::Focused` | Rust `gate` unit tests; manual: alt-tab away, press A, nothing happens |
| R5 | Multiple controllers: shared held state, identical models sharing a key, hot-plug of the pad in use | Per-id held state in `repeat.ts`; assignment by UUID key (documented); next connected pad takes over on disconnect | vitest (`repeat.ts`, `controllers.ts`); e2e injects two pads (held RB on one, presses on the other) and a disconnect |
| R6 | A game launched from TV: input must stop and resume, and the save notices must show after | Gate fed by `launch.set_notify`; `released_all` on close; un-minimize on last session end | e2e `tv-details` with the stub emulator: injected presses during the session change nothing; after it ends, presses work and the upload notice shows |
| R7 | Scroll and clip rules: focus ring and 7% growth cut by an overflow ancestor; browser "nearest" scrolling sneaking in (`focus()` scrolls by default) | Focus is a class, never `element.focus()` without `preventScroll`; `reveal.ts` computes every scroll; padding/negative-margin rules from `tvmode.css` | The prototype's clipping checker as `assertNoClipping()` after every move in every TV spec, and the P14 three-resolution sweep |
| R8 | Windowed grids vs spatial focus (unrendered cards have no rects) | Index-based moves inside grids (`focus/grid.ts`) with `scrollToIndex` | vitest; e2e on the `large-platform` fixtures in `tv-layout` (End-of-grid moves, focus restore after Details) |
| R9 | Desktop regressions from P1/P2 extraction | Pure moves with no behaviour change; components keep their markup | Full desktop e2e after P1 and P2 with unchanged assertions |
| R10 | Rem sizing at 4K and 720p (text too small, layout off) | One root-font rule; no px in `tv.css` except hairlines | `tv-layout` at 1280×720, 1920×1080, 3840×2160 |
| R11 | "Back to the game" can't raise another app's window on Wayland | Minimize GRID instead of raising the game | Manual on the .rpm; if a compositor refuses minimize, the fallback is leaving fullscreen (noted in SPEC) |
| R12 | Secrets through the OSK or clipboard | Masked fields never echo; values go only to the existing commands; nothing logs field values | `scripts/check_secret_hygiene.sh` in the gate; code review of `TvKeyboard`, `TvConnect`, `read_clipboard_text` |
| R13 | CI/container drift (libudev-dev) | All four apt lines and the Containerfile in P4; `scripts/ctr.sh build` | gate in the new image; `check` job on the next CI run |
