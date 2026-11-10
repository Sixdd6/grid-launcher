# TV mode: handover notes between phases

## After P4 (6e69cc6): native interface for the TV UI
- Event `gamepad` (`GAMEPAD_EVENT` in api.ts), payload `PadEvent` tagged by `kind`:
  `connected {pad: PadInfo}`, `disconnected {id}`, `button {id, button: PadButton, down}`,
  `stick {id, stick: 'left'|'right', x, y}` (−1…1, Y positive up),
  `trigger {id, trigger: 'left'|'right', value}` (0…1), `released_all`.
- `PadInfo = {id: number, key: string (gilrs UUID, 32 hex), name, vendor: number|null, product: number|null}`.
- `PadButton` (physical positions): `south east west north left_bumper right_bumper
  left_trigger right_trigger select start guide left_stick right_stick dpad_up dpad_down
  dpad_left dpad_right`. `guide` is reported but must not be bound (ruling 5).
- Buttons arrive on change only: repeat timing (400/90 ms; keyboard 500 ms then 20/s) is the
  UI's job. Xbox/Nintendo assignment is applied in the UI from `uiSettings.tvAssignments[pad.key]`.
  A `disconnected` pad's held buttons get no up events: treat as released. connected/disconnected
  are sent even while the gate is closed. `gamepad_list` is empty until TV mode first turns on.
- Commands (api.ts wrappers): `getUiMode() -> {mode: 'desktop'|'tv', started_with_tv, fullscreen}`,
  `setUiMode(mode)`, `gamepadList() -> PadInfo[]`, `tvYieldToGame()`, `quitApp()`,
  `listFolder(path, files) -> {path, parent|null, entries:[{name,is_dir}]}` (empty path = home),
  `readClipboardText()`, e2e only `e2eGamepadInject(events: GamepadInput[])` where GamepadInput is
  `{kind:'connect', pad}` | `{kind:'disconnect', id}` | `{kind:'input', id, event}` (event
  `{kind:'pressed'|'released', button}` | `{kind:'changed', button, value}` |
  `{kind:'axis', axis: left_stick_x|left_stick_y|right_stick_x|right_stick_y|dpad_x|dpad_y, value}`)
  | `{kind:'focus', focused}`.
- With `--tv`, Rust is already in TV mode (fullscreen, gate open) before the UI loads: read
  `get_ui_mode` at startup to mount TvShell.
- `[ui]` keys: `open_on` (automatic|discover|library), `tv_prompts` (match|xbox|ps|nintendo|8bitdo),
  `tv_assignments` (pad key → xbox|nintendo). uiSettings store: `openOn`, `tvPrompts`,
  `tvAssignments`, `loaded`, `setOpenOn`.
- `initialView(connected, offline, firstDrive, openOn)` in lib/shell.ts is the shared start rule.
- Unverified (do in P5): window geometry after TV → quit → relaunch (window-state plugin has
  FULLSCREEN removed; `exit_app` leaves fullscreen first).

## After P5: the TV shell, focus engine, input and primitives
- **Mounting.** `stores/uiMode.svelte.ts` (`uiMode.mode|startedWithTv|loaded|tvStart|desktopStart`,
  `initUiMode()`, `enterTv(view)`, `leaveTv(tab, pane)`) mirrors Rust's `get_ui_mode` / `set_ui_mode`.
  App.svelte mounts `lib/tv/TvShell.svelte` **or** the desktop screens (nothing until `get_ui_mode`
  answers) and sets `html[data-ui="tv"]`; every rule in `lib/tv/tv.css` is scoped to it. Desktop
  `Shell` takes `initialView` / `initialSettingsPage` (Switch to desktop mode lands on the same
  view) and has the `tv-mode-button` (look not designed yet: TODO(design) in Shell.svelte).
  `uiMode.ts` maps views ↔ tabs and Settings pages ↔ TV panes (`TV_PANES`, `TV_PANE_LABELS`; TV
  mode's own pane leaves to Appearance). TvShell owns the start tab (`uiMode.tvStart`, else the
  shared `initialView` rule), the theme override (`setTvThemeActive`: Follow system → dark), the
  clock, the input router and `registerNavigation` (toast actions switch tabs).
- **Screens.** `nav.svelte.ts` holds the tab, `pane`, the page stack, `layers` (empty until P6),
  `startup`, `selMode`, per-screen focus memory (`data-k`) and scroll (`data-scroll` keys).
  `showTab(tab)`, `pushPage({title, component, props})` (breadcrumb uses `title`), `popPage()`,
  `setPane(pane)`, `setStartup(on)`. TvStage mounts one screen at a time inside `ScreenFrame`; a
  screen registers hooks with `useScreen({...})` (`screen.svelte.ts`):
  `hints(focusedEl) → Hint[]` (after A and X), `x`, `y`, `lt`, `rt`, `lb`, `rb` (a page's LB / RB, e.g.
  Details tabs, win over tab switching), `main() → {disabled, why?, press} | null` (Start presses it
  or toasts `why`; the hint bar then drops "Menu"), `arrive()` (after the screen is shown and focus
  restored: P7's Discover focus, item 104). `registerMenu(fn)` / `registerView(fn)` are the hooks P6
  fills for Start and View. B rules are generic (`input/press.ts`): overlay (P6 `closeLayer` TODO),
  page pop, select mode (P13 TODO), grid/pane → rail.
- **Items.** `use:tvf={{a, x, adjust, focus}}` (`focus/actions.ts`) makes an element focusable;
  never give a TV item its own onclick (a click focuses and presses A). Markup attributes:
  `data-k` (stable key), `data-al` (A hint, default "Select"), `data-xl` (X hint), `data-why` +
  `aria-disabled="true"` (A / click toasts why; use aria-disabled, not disabled), `data-autofocus`,
  `data-adj` (Left/Right call `adjust`), `data-read` (Up/Down scroll the `data-read-area` first,
  P9), `data-cover`, `data-rail-open` (Right = A on rail rows). Areas: `data-nav="h|v|grid"`,
  `data-enter="sel|first"`, `data-edge` (Discover's rows area), `data-up|down|left|right="<data-k>"`
  links, `data-rail` (B target; its `.sel[data-f]` row), `data-rail-target` (what A/Right on a rail
  row enters), `data-track` (sideways row: 1.2rem edge), `data-shelves` + `data-shelf` (Discover:
  shelf title to top), `data-scroll="<key>"` (scroll kept per screen), `data-read-area` (Details
  tab body: right stick / PgUp/PgDn target). Focus is the `.f` class; the engine is pure
  (`focus/engine.ts`, `reveal.ts`, `grid.ts`), `focus/dom.ts` measures. A redraw keeps focus by key,
  else the enabled item in its place inside the nearest `[id]` box (give panels ids).
  One deliberate extension of the prototype: a list's end carry-on treats a rail row as spanning
  its whole rail, so Left from any Discover shelf reaches the rail.
- **Primitives (`lib/tv/ui/`).** `TvButton {k, primary, danger, disabled, why, al, xl, autofocus,
  glyph, testId, onA, onX}`, `TvCover {k, title, cover, installed, update, missing, platform, queue,
  picked, label, width, art (BackgroundSubject → background follows focus), quick (X label), onA,
  onX}`, `TvRail {label, scroll}` + `TvRailRow {k, selected, count, sub, fold (folder header open
  state), xl, onSelect (on focus), onToggle, onX}`, `TvGrid {id, scroll}` (not windowed yet:
  TODO(P7/P8) use `focus/grid.ts`), `TvTrack {scroll, up, down}`, `TvRows {rows: Row[]}` over
  `rows.ts` (switch | choice | slider | text | action | info), `TvPage {title, sub, scroll}`,
  `TvChip {glyph, warn, k+onA for focusable}`, `TvSwitch`, `TvSlider`, `TvIcon {name, px}` (rem-sized
  icon), `glyphs/Glyph {button}` (prompt set from `controllers.promptSet`; `data-set` attribute).
- **Input.** `input/router.svelte.ts` (`controllers.pads|current|name|assignment|promptSet`,
  `setTypingRepeat(fn)` for P6's keyboard) feeds `handlePress` from the `gamepad` event (per-pad
  `Repeater` + `Causes`, assignment per pad key), the keyboard (Q/E/Z/R/M/V…, held keys use the same
  400/90 ms repeater, PgUp/PgDn scroll 70%) and the mouse (pointing focuses only after a real move;
  click = focus + A; `data-btn` hints and `data-tab` tabs are clickable). TODO(P6): physical typing
  into the OSK (item 74). Notices: P5 still renders the desktop `Toast` (px, bottom right) in TV mode;
  P6 replaces it with TV cards.
- **Interim (P12):** TvShell's startup screens are a placeholder box that offers Switch to desktop
  mode. Settings › TV mode has only Controller and Switch to desktop mode (P10 adds the rest).
- **e2e.** Group `tv-shell` (seed `e2e/seed/tv-shell-seed.mjs`: offline profile). `tv-*` groups get
  `E2E_APP_ARGS=--tv`; a spec opts out with a first line `// e2e-app-args:` (wdio.conf.ts passes
  `appArgs`). Helpers in `e2e/helpers/tv.ts` (`XBOX_PAD`, `NINTENDO_PAD`, `connectPad`, `press`,
  `hold`/`release`, `axis`, `key`, `focused`, `waitFocused`, `hints`, `promptSet`, `waitTab`, `invoke`).
- **Settings store.** uiSettings gained `setTvPrompts(choice)`, `setTvAssignment(key, style)` (for
  P10's pane) and `setTvThemeActive(on)`. `BackgroundArt` takes `fadeMs` (TV: 360).
- **Still unverified:** window geometry after TV → quit → relaunch. The container can't check it:
  e2e builds don't register the window-state plugin (`#[cfg(not(feature = "e2e"))]` in lib.rs)
  and Xvfb has no window manager. Check by hand on the P14 .rpm.

## After P6: overlays and notifications
- **Overlays** (`lib/tv/overlays/overlays.svelte.ts`; each is a layer on nav's stack, drawn by
  TvShell's `.layers` box through `TvLayer`, scrim to the hint bar):
  - `openList({title, sub?, items})` — items `{label, sub?, checked?, danger?, onPick?, testId?}` or
    `{group}` (heading, not focusable). Give every option `checked: true|false` in a choice list (the
    check column shows only then); the checked one has focus on open. A closes, then runs `onPick`.
  - `confirmBox({title, body, ok, danger?, onOk})` — Cancel focused; `onOk` runs after it closes.
    `quitBox()` is Quit GRID's (calls `api.quitApp()`).
  - `openKeyboard({title, value?, mask?, onInput?, onDone?, onCancel?})` — masked opens empty and
    shows dots; `onInput` on every change (live search); `onDone(value)` after it closes; B →
    `onCancel`. Pad shortcuts, typing repeat (layer `typingRepeat`) and physical typing (layer
    `keydown`) are built in. Model: `overlays/osk.ts` (pure).
  - `openMenu()` — Start's Menu (TvShell registers it with `registerMenu`). Discover only shows the
    Server tab: TODO(P7) select the Discover rail row so `arrive` focuses its content (item 365).
    Reconnect is `lib/tv/reconnect.ts` (no server → Connect; a failed retry says
    "Can't reach <URL>", TODO(design)).
  - Anything custom (P9's media viewer, P12's blocking unreachable dialog):
    `openLayer({component, props, blocking?, hints?, binds?, cls?, keydown?, typingRepeat?, onClose?})`
    from nav.svelte.ts → id. `binds` = buttons the layer answers itself (a bound direction too, e.g.
    the viewer's LEFT/RIGHT/LB/RB); every other button but A/B/directions does nothing under a layer
    (press.ts `layerButton`). `blocking` = B and a click on the scrim do nothing. `closeLayer()`
    restores focus to the opener (by element, else by `data-k`), then runs `onClose`, then
    `afterOverlays(fn)` callbacks (item 117: wrap a redraw that would move things under an overlay).
    `topLayer()` reads the top one. Focus memory is per layer (`layer:<id>`), so the screen's is kept.
- **Notices** (`lib/tv/notices.svelte.ts`, pure rules in `notices.ts`): `pushToast(text, level,
  durationMs?, {sub?, action?: {label, run}})` reaches TV through `onToastPushed` (desktop ignores
  the extras and is unchanged; TvShell hides the desktop `Toast` in TV mode). The notifier gives
  queued / failed download cards and backup-queued cards `action: 'downloads'` → "Open Downloads"
  (`Notice.action` in notifications/notices.ts; summaries keep a shared action). TV-only cards:
  `showNotice({title, sub?, error?, action?, ms?, hold?, pinned?, segs?, step?})` → id;
  `updateNotice(id, patch)`, `releaseNotice(id, ms?)` (unpins and starts the countdown),
  `closeNotice(id)`. P12's setup card = `showNotice({hold: true, pinned: true, segs, step})`, then
  `releaseNotice(id, 10_000)` when done. `sub` may mix text and `{glyph}` parts. History: whole
  session from app start (ruling 16), `tvNotices.history`, `clearHistory()`; page
  `pages/TvNotifications.svelte`. Welcome card: TvShell, after the first `showScreen` (ruling 15).
- **TODO for later phases:** "Couldn't launch X: no emulator for P" with "Set up P" (tvmode.js) needs
  the platform in the launch failure (P9). `pushPage(page, {fresh: true})` closes pages first.
- **e2e.** `tv-shell-d.spec.ts` (overlays, keyboard, Menu, cards, Notifications). VITE_E2E builds
  expose `window.__tvE2E = {notify, openList, openKeyboard, confirmBox}` (TvShell) until real
  screens open them; later specs should drive the real screens instead.

## After P7: the Server tab
- **Done from the P5 pending list:** the desktop TV button is a 28×28 `icon-btn` with the new
  outline `tv` icon (icons.ts + prototype icons.json); Space stays A with the OSK's typing branch
  first (comment in input/buttons.ts; already tested). The P6 pending items (reconnect.ts, menu.ts,
  the scrim) are untouched: they were held back from this phase.
- **Server state** (`tabs/server/server.svelte.ts`): `srv.rail` ('discover' | platform id),
  `srv.query`, `srv.discoverKey` (Discover cover last focused), `srv.pendingArrive`; outlive the
  screen (TvStage remounts a tab on every switch). `serverCatalog()` = one `Catalog` per server URL
  (a new server resets the rail). `selectRail(r)` clears the search. `goDiscover()` = Menu ›
  Discover (selects Discover, `rememberFocus('tab:server','r-discover')`, `showTab('server',
  {again: true})`, then `arrive` carries focus into Discover). `loadAllGames()` = every platform's
  `listGames`, cached per server, for Search everything (no server-side search command exists; a
  failed platform is asked again next time).
- **Screens:** `TvServer` (rail, `arrive`, Y/RT hooks as getters, offline / no-server panels; back
  online it runs `arrive` again), `TvDiscover` (hero + `#disc` rows: `data-edge data-shelves
  data-rail-target data-scroll="disc"`, shelves `data-shelf`, tracks `disc-<id>`; cover keys
  `<shelf><romId>` e.g. `n501`; hero summary = installed row's description, else `getRomDetail`
  after a 250 ms rest, cached), `TvPlatform` (header + `TvGrid #srv-grid`, keys `sg<romId>`; its
  platform row comes from `srv.rail`, not the catalog's active row, which lags a selection),
  `pages/TvSearchAll` (keys `sr<romId>`; Y reopens the keyboard with live results; an emptied field
  keeps the last results; the breadcrumb follows via `setPageTitle`).
- **Shared TV game model** (`tv/games.ts`, for P8/P9 too): `TvGame` from a `GameSummary`
  (`gameFromSummary`) or an installed row (`gameFromRow`), `liveInstall`/`queueTag`, `quickFor`
  (Play / Install / nothing), `platformTag`. `ui/TvGameCover` = a cover wired to the stores (badges,
  live tag, art, A → Details, X → `runQuick`). `gameActions.ts`: `openDetails(subject)`
  (registered as `navigation.openDetails` in TvShell), `openGameDetails`, `runQuick`, `openSystem`.
  **Stubs:** `pages/TvDetailsStub` (TODO(P9): TvDetails) and `pages/TvSystemStub` (TODO(P11)), each
  a TvPage with Back; swap the component in `gameActions.ts`.
- **Primitives:** `TvCover` gained `onFocus` and `testId`; `TvChip` gained `testId`.
- **nav.svelte.ts:** `rememberFocus(screen, k)`, `showTab(tab, {again})`, `setPageTitle(id, t)`.
  Two fixes that matter to every later phase: `useScreen` registers untracked (registering bumped
  `hooksVersion`, which the effect also read: an `effect_update_depth_exceeded` loop that aborted
  whole flushes and left stale DOM); `closeLayer` blocks `stageChanged` until it has put focus back
  (else the observer restored the screen's focus first, and focusing a rail row selects it), and
  when the opener is gone it takes the item in its place in the opener's panel, or the rail's
  selected row when the panel is empty (a live search that matches nothing).
- **Not done (TODO in code):** RT select mode on a platform and Backup all N… (P13: the hook and the
  button exist, both do nothing); X's umu-launcher / library-folder prompts (only their refusal
  cards show); Play doesn't push Playing yet (P9); `TvGrid` is not windowed, so a platform renders
  every cover (TODO(P7/P8) in TvGrid; large platforms in P14's `tv-layout`).
- **e2e:** group `tv-server` (`specs/tv-server.spec.ts`, fixtures-discover, seed
  `seed/tv-server-seed.mjs`: library folder + SNES default emulator). The spec connects on the
  desktop (`// e2e-app-args:`) and enters with the TV button. `helpers/tv.ts` gained
  `assertNoClipping()` (focused item + ring / cover growth inside every clipping ancestor, waits for
  glides), `toastTitles()`, `waitToast(text|re)`. The spec captures page errors from the start and
  its last test asserts there were none: worth copying into later TV specs.

## After P8: the Library tab
- **Done from the pending lists:** `reconnect.ts` does nothing while `session.busy`, its TODO(design) is gone
  (card unchanged; `reconnect.test.ts`); `overlays/menu.ts` takes the session: used without a server the
  Reconnect row reads "Connect to a server…" (same key and handler) and `menuHead` reads "Offline · no
  server" (TvMenu passes `session`). The `.layer` scrim was a no-op. The P7 Search everything list is
  still pending (P9, untouched here, as is the Server platform search).
- **Library state** (`tabs/library/library.svelte.ts`): `lib.query`, `lib.sort` (LibrarySort, default
  `played`), `lib.folds` ({plat, drives}, both open, kept for the session), `selectLibRail(key)` (clears
  the search), `toggleFold(id)`, `driveGamesCache` (each drive's games, so a remount draws at once). The
  selected row is the desktop's `librarySelection` (shared by both modes); a vanished row falls back to
  All games, written back.
- **Pure modules:** `libraryRail.ts` (`libraryRail()` over `library/rail.ts` → filters / platforms /
  drives, keys `r-<RailKey>` e.g. `r-platform:sega-genesis`, `r-drive:<mount>`, folder headers
  `fold-plat` / `fold-drives`, `NO_DRIVES`, `backupAllLabel`), `libraryHeader.ts` (`TV_SORTS`, `countLine`,
  `libraryGrid`, `driveGrid` (the four TV sorts over drive games; played / installed by this computer's
  copy), `emptyState`, `driveEmptyState`, `driveCountLine` (games shown and their size), `driveChips`
  ("N systems")). Search ruling: a blank query is no filter; the Search chip and the no-match text show
  only for a non-empty one; B on the keyboard restores the value it had.
- **Screen** `TvLibrary`: covers `lg<id>` (installed rows) and `dg<romId>-<server>` (drive games),
  test ids `tv-cover-<key>`, `tv-lib-name|count|search|sort|emus|fw|empty|no-drives|fold-*|rail-*`.
  X on a platform row: offline → "Backing up needs a server."; online it is TODO(P13) (the Backup page).
  RT is TODO(P13). Drive covers: `games.ts` `gameFromDrive` (cover via `driveAssetUrl`, installed from the
  registry row by rom id or the drive's own flag), `quickFor` gives Install from drive, `runQuick` copies
  it (`games/run.ts` `installFromDrive`), `subjectOf` gives `fromDriveGame` (Details stub until P9).
- **Closed folder holding the selection:** its header gets `data-holds-sel`; the engine (`dom.ts`
  snapshot) and `toRail` / `closeLayer` treat it as the rail's selected row, so B and Left land on it
  instead of selecting another row (the prototype would have selected All games). Deliberate extension.
- **Windowed grids (R8):** `ui/TvGrid` takes `items`, `key` (the cover's `data-k`), a `cell` snippet and
  `listKey` (a new list scrolls to the top); without `items` it is the old plain grid. It renders the rows
  meeting the view ±2 and the focused cover's row ±1 (`focus/grid.ts` `gridWindow`), spacers stand in for
  the rest (exact pitch from two drawn rows; `overflow-anchor: none`). Each cover sits in a
  `.gcell[data-i]` (`display: contents`) inside `[data-window][data-count]`. `focus/windows.ts` holds a
  handle per grid; `nav.svelte.ts` moves by index inside (`gridMove`; only an edge goes spatial), renders
  an unrendered cover before focusing it (restore by key, closeLayer, stageChanged), syncs windows around
  `loadScroll` in `showScreen`; `dom.ts` `reveal` takes first / last by index. Used by Server platforms
  and the Library; Search everything still uses a plain `.grid` (P9 may switch it).
- **Notifier fix (shared with the desktop):** `initNotifications` started after the downloads store's
  first snapshot (TV mode's offline start) took the next snapshot as its baseline, so the first install
  of the session had no "added to download queue" card. It now starts from the store's current entries
  (`downloads.hasSnapshot()`); notifier.test.ts covers it.
- **e2e:** group `tv-library` (seed `tv-library-seed.mjs`: offline, three installed games, fake drive
  TESTDRIVE with the "Travel kit" pack; `E2E_DRIVE_ROOTS` like `backups`) and `tv-large-platform`
  (large-platform fixtures + 600 seeded installed games; connects on the desktop). Both capture page
  errors.
- **Not done / for later:** P13 owns the Backup page hooks, select mode, the drive's Install-from-drive
  notices beyond the queue card (425–429) and Details from a drive. Where GRID opens on the first drive in
  TV (item 415) is P12: TvShell doesn't select the drive row yet.

## After P9: Details and its pages
- **Done from the pending lists:** Search everything's states (consultant rulings): `loadAllGames()`
  resolves `{games, partial}` and drops a failed or partial load from the cache; `searchResults.ts` has
  `searchingHeading`, `LOAD_FAILED_TITLE`, `loadFailedLine`, `PARTIAL_NOTE`, `RETRY`; `TvSearchAll` shows
  the loading heading over an empty grid, the failure block (`tv-search-failed`, Try again `sr-retry`
  focused) and the partial warning (`tv-search-partial`) with Try again; an emptied field on the results
  page is still "nothing typed". The Server platform search now follows the Library's rule:
  `serverHeader.ts` `searching(q)` (blank = no filter, no chip, plain count, no no-match text) and B on
  the keyboard restores the search it had. The P7 TODOs for the umu-launcher and library-folder prompts
  are built (`gameActions.ts` `promptFor`), and Play pushes Playing from Details and from X on a cover.
- **Pages** (`lib/tv/pages/`): `TvDetails` (+ `details/TvOverviewTab|TvMediaTab|TvSavesTab|TvSavesPanel|
  TvAchievementsTab`), `TvPlaying`, `TvMediaViewer` (a layer: `mediaViewer.svelte.ts` `openViewer`, binds
  LB / RB / Left / Right), `TvGameSettings`, `TvLinkPage`, `TvFolderBrowser`. Pure rules and words:
  `details.ts` (button row per state, left lines, header line, tab step, facts, media caption and
  stepping, Saves words, every confirmation, Playing line and note, link and launch-file lines) and
  `folderBrowser.ts`. `detailsState.svelte.ts` keeps the tab and the Saves | States choice for the
  session (a page pushed from Details remounts it).
- **Entry points** (`gameActions.ts`): `openDetails(subject, sizeBytes?)`, `playGame({romId, title,
  platform})` (Playing on success, else `promptFor(outcome)`), `installGame`, `promptFor`, `offerUmu`
  (confirm, then `api.installUmuLauncher`; the desktop offer's note as a card with Open Downloads),
  `promptLibraryFolder` (confirm, folder browser, `api.setLibraryPath`; the install is pressed again, as
  on the desktop), `openFolderBrowser({title, start?, files?, onPick})` (P10–P12 reuse it for Change
  library folder, Relocate, Program, Connect), `openSystem` (still the stub: P11 swaps in TvSystem).
- **Shared changes:** `DetailsStore.serverGames` (Related's covers; `serverTitles` derives from it).
  `NoticeAction` gained `{system}`: `launchFailedNotice(title, reason, setUp?)`, `notifier.launchGame(romId,
  title, platform?)` and `run.playInstalled(..., platform?)` put "Set up <Platform>" on the card when
  `launchError.ts` `needsEmulatorSetup(message)` (none configured, default missing, no program); the
  desktop's card shows no action. `navigation.openSystem` (TvShell registers it). The card keeps the
  desktop's text ("Couldn't launch X: No emulator is configured. Add one in Settings › Systems.").
- **nav.svelte.ts:** `setPageProps(id, props)` (Link updates the Details page under it to the linked
  row); `setFocus` scrolls the previous item's `data-read-area` to its top when focus enters a
  `data-read-reset` row (item 299). Details markup: buttons `#det-btns[data-read-reset]` keys
  `db-<kind>`, tab body `#det-body[data-read-area][data-scroll=tbody]`, summary `prose[data-read]`,
  Related `rel<romId>` (track `up="prose"`), media `m<i>`, Saves `sk-save|sk-state`, `up`, rows
  `sv<recordId>` in `#saves-list`; viewer item `viewer`; Playing `pl-resume|pl-stop`; folder browser
  `up`, `fs-<name>` in `#fs-list`; link rows `lk<romId>`; Game settings rows `s-exe|s-params|s-compat|
  s-reload|s-save`.
- **Not done / for later:** Backup to drive… (Details) does nothing yet: TODO(P13) the Backup page. Item
  438 (refusals while a game is being backed up) is P13. Details › Saves doesn't manage a native game's
  save folders (the desktop's PC save-folder list); upload still follows its rules.
- **e2e:** group `tv-details` (fixtures `fixtures-tv-details`: the cloud-saves games, Link Target,
  Lost Cart, My Game on Windows, No Emu Game on Nintendo 64; seed `tv-details-seed.mjs`; mock flag
  `--stateful-saves`, so uploads join and deletes leave a rom's records). `helpers/tvDetails.ts` has
  `openLibraryDetails(title)`, `detButtons`/`waitButtons`, `bodyScroll`, `captureErrors`/`pageErrors`.
  The tv-server, tv-library and tv-large-platform specs now wait for `tv-details`.


## Pending for the next phase (consultant, after P9): prompts and native save locations
- Checklist 529–531 (README § Consultant decisions). Files: app/src/lib/tv/gameActions.ts (`offerUmu(title)`,
  `promptLibraryFolder(title)`, `promptFor(outcome, title)`, the "Library folder set" card), app/src/lib/tv/pages/details.ts
  + test (`umuConfirm(title)`, `libraryFolderConfirm(title)`, `LIBRARY_FOLDER_SET`, `LIBRARY_FOLDER_BROWSER_TITLE`,
  save-location words `removeLocationConfirm(raw)`, `SAVE_FOLDER_ADDED`, `saveFolderBrowserTitle(title)`),
  app/src/lib/tv/pages/TvDetails.svelte (pass the title), app/src/lib/tv/pages/details/TvSavesPanel.svelte (the
  "Save locations" section on CloudStore.nativePaths/nativePhase/nativePathCount/addManualPath/removeSavePath,
  list `#saves-locs` data-nav="v", keys `sl<raw>`), e2e in tv-details (library-folder confirm; add/remove a
  location through the browser with the "My Game on Windows" fixture). Remove the handover line saying TV
  Saves doesn't manage native save folders.

## After P10: Downloads and the Settings panes
- **Downloads** (`tabs/downloads/TvDownloads.svelte`, pure `downloadRows.ts`): panels `tv-dl-seg-<active|queued|completed>`
  (counts `tv-dl-count-*`, empty lines "Nothing active." etc.), rows `dl-<entry id>` (`tv-dl-row-<id>`, line
  `tv-dl-line-<id>`) inside `#dl-list`, so a row that leaves gives focus to the one in its place. `statusText` is the
  prototype's dotted wording for installs ("Downloading · 45% · 5.0 MB/s · 11s left", "Queued · from <label>"), the
  desktop's `entryDetail` for backup rows (GB card words) and the library move. `rowActions` follows the desktop's
  `actionFor` (firmware and extracting rows can't be cancelled; a library move's Cancel / Retry are the move's own);
  `rowA` gives "Cancel" / "Retry or dismiss" / "Dismiss" (cancelled reads "Dismiss", as on the prototype), "Retry or
  cancel" for a failed move still in its copy, "Select" for a row that only opens its game, and disables a row with
  nothing to offer (why: `CANCEL_EXTRACTING`). Cancel always asks (`cancelWords`: installs reuse Details'
  `cancelConfirm`; backup and library-move wordings are mine, TODO(design)). Open <title> opens Details from the
  installed row, else a minimal server subject. Y = `downloads/actions.ts` `dismissAll(completedIds)`. The sparkline
  is drawn inline from `sparklinePaths` (stretched 12 × 2.6rem box) and shows only while active, as on the prototype.
- **Settings** (`tabs/settings/TvSettings.svelte`): rows come from `panes/{appearance,cloud,connection,library,ra,tv,
  updates}.ts` (pure, tested in `panes/panes.test.ts`); `panes/choice.ts` builds a choice row whose list is drawn by
  TvSettings (`openChoice`, option test ids `tv-opt-<value>`). Row keys / test ids: `s-<key>` / `tv-row-<key>`
  (`open-on theme art fade blur hw cards`, `restore upload skip-newer delay retain`, `server user credential reconnect
  disconnect connect`, `lib-path lib-change`, `ra-user ra-token ra-save ra-clear`, `prompts assign tv-controller
  tv-desktop tv-quit`, `version checked release debug`); the heading is `tv-pane-title`. `rows.ts`: `disabled` / `why`
  now on every row kind (TvRows sets `aria-disabled` / `data-why`), `sliderPercent` clamps. Blur shows 0–100% in
  steps of 10 over the stored 0–40 (`blurPercent` / `blurFromPercent`). Debug logging lives in Updates (prototype),
  backed by `get/set_debug_prints`. Cloud saves save the whole `CloudSettings` on every change. RetroAchievements keeps
  the typed username and token in the screen only (the token in a plain local, never in a row: the row shows a "•"
  stand-in), forgets them when the pane changes; Save is disabled until both are set (why: TODO(design)); Clear acts
  at once with the desktop's card. Button assignments is disabled with no controller (TODO(design)); TV mode pane's
  first row is now `s-prompts` (tv-shell-a / -b updated).
- **Library change**: the folder browser opens beside the current library (`browserStart`); the flow runs on
  `LibraryChange` (check → `refusalNotice` → `askMove` → list "Move to <path>?" with `moveListSub` → `startMove` or
  `startFresh` → `askDelete` for the size → list Leave / Delete → `confirmBox(deleteWords)`). Cards: "Moving the library
  to <path>…", "Library is now <path>" (sub: the desktop's outcome words), "Can't use that folder". `settingsState.
  svelte.ts` keeps the library path across remounts: popping a page remounts the tab, and a disabled row there sent
  focus to the rail's first row (which then selected Appearance).
- **e2e:** group `tv-settings` (`specs/tv-settings.spec.ts`, seed `tv-settings-seed.mjs`: library + empty
  `library-new`; default fixtures with `--throttle-ms 100`). Connects on the desktop, installs 301 / 201 / 101 by
  `install_game`, and covers Cancel (asked), Dismiss, Retry, Y with focus kept, the rail, a slider, Theme, prompts,
  assignments per pad (config), the refused and the Start fresh › Leave change, Disconnect → Connect placeholder.
- **Not done / for later:** Backup drives pane (P13) and Systems (P11) stay empty. RA's username + password login (the
  desktop's second form) isn't in the TV design. Connection has no "edit connection" (not in the design).

## Pending for the next phase: Downloads Cancel wording (consultant ruling 8, exact text)
P10 used its own wording for backup and library-move rows; replace it in
app/src/lib/tv/tabs/downloads/downloadRows.ts (+ test) with the ruled text:
- Game / update / emulator / firmware row: "Cancel <title>?" / "The download stops and its files are removed. You can
  install it again later." (queued: "It leaves the queue. You can install it again later.") / "Cancel install".
- Backup row: "Cancel <title>?" / "The backup stops and what it wrote to <label> is removed. You can back it up again
  later." / "Cancel backup".
- Library move: "Cancel the move?" / "The files copied so far are removed and the library stays where it is." /
  "Cancel move".
- Then the card "Cancelled <title>". Danger action button, Cancel focused. Remove the TODO(design) markers.

## Pending for the next phase (consultant, after P10): checklist 532–534
- app/src/lib/tv/tabs/settings/panes/tv.ts: `ASSIGN_NEEDS_PAD` = "Connect a controller to choose its buttons. Each
  controller keeps its own choice."; line "For the controller in use", value "Xbox style"; drop TODO(design); tests.
- app/src/lib/tv/tabs/settings/panes/ra.ts + TvSettings.svelte: dynamic SAVE/LOGIN reasons; `RaState.passwordTyped`,
  `canLogin`; `RaActions.editPassword`, `login`; rows Password + Log In after Save (order: Username, Token, Save,
  Password, Log In, Clear); password is a non-reactive local forgotten by `forgetRaTyping`, never logged/echoed;
  Log In uses raActions.raLogin then blanks the password, resets tokenTyped, refreshes status. Tests; e2e if the mock
  supports RA login.
- app/src/lib/tv/tabs/downloads/downloadRows.ts: `rowWhy(e)` by kind (firmware → "Installing firmware — this step
  cannot be cancelled"; library_move → "Updating paths — this step cannot be cancelled"; else CANCEL_EXTRACTING);
  `rowA` returns 'Details' when only `open` remains; tests.
