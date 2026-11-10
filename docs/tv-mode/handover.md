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
  438 (refusals while a game is being backed up) is P13.
- **e2e:** group `tv-details` (fixtures `fixtures-tv-details`: the cloud-saves games, Link Target,
  Lost Cart, My Game on Windows, No Emu Game on Nintendo 64; seed `tv-details-seed.mjs`; mock flag
  `--stateful-saves`, so uploads join and deletes leave a rom's records). `helpers/tvDetails.ts` has
  `openLibraryDetails(title)`, `detButtons`/`waitButtons`, `bodyScroll`, `captureErrors`/`pageErrors`.
  The tv-server, tv-library and tv-large-platform specs now wait for `tv-details`.


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

## After P11: Settings › Systems, the System page and the emulator form
- **Done from the pending lists:** the Downloads Cancel wording (ruling 8, exact text: `downloadRows.ts`
  `cancelWords`, a queued install "It leaves the queue…", the backup body names the drive's label; no
  TODO(design) left there; tv-settings checks the body, the button and its danger style). Checklist 529–531:
  `details.ts` `umuConfirm(title)`, `libraryFolderConfirm()`, `LIBRARY_FOLDER_BROWSER_TITLE`, `LIBRARY_FOLDER_SET`,
  `SAVE_LOCATIONS_HEADING`, `ADD_SAVE_FOLDER`, `SAVE_FOLDER_ADDED`, `saveFolderBrowserTitle`, `removeLocationConfirm`
  (body = the raw path); `gameActions.ts` `promptFor(outcome, title)`, `offerUmu(title)`, `promptLibraryFolder()`
  (card "Library folder set", sub the path; error card on refusal). `TvSavesPanel` lists a native game's save
  locations under the records (`#saves-locs`, rows `sl<raw>` with `data-raw`, test id `tv-saves-loc`, then
  `sl-add` "Add a save folder…", which opens the folder browser in the game's own folder). The browser page
  remounts Details, so `det.locations` (detailsState) is bumped after an add and the new panel reloads.
  The pending section for 532–534 is untouched (held back from this phase).
- **One shared store** (`tv/systemsTv.svelte.ts`): `tvSystems` (a `SystemsStore`) outlives the remounts, so an
  install, "Installing N emulators…" and the lists survive a System page and back. `useTvSystems()` (call from a
  screen's script) runs what the desktop SystemsPage runs while shown; the setup-defaults and firmware-pass
  listeners stay on for the session. Also `tvProton` (catalog, pending), `windowsNow()` (picker, status, tool
  label), `openProtonList()`, `openEmulatorList(row)`.
- **Pane** `tabs/settings/TvSystemsPane.svelte` (TvSettings draws it for `systems`), pure `panes/systems.ts`
  (`recButton`, `systemItem`, `windowsItem`, `orderSystems`, `otherItems`, `checkingText`, `updatesSub`,
  `installingCount`). Header buttons in `#sy-head`: `sy-rec` / `sy-chk` / `sy-add` (test ids `tv-sys-rec|check|add`);
  rows `sy-<slug>` (`tv-sys-row-<slug>`, `tv-sys-sub-<slug>`, pill `tv-sys-status-<slug>` with `data-status`),
  Windows `sy-windows`; Other emulators rows `sy-other-<name key>` or `tv-sys-other-empty`. Install recommended
  shows "Installing N emulators…" (aria-disabled, why `REC_RUNNING`) while its press is queued and while
  no-emulator systems still install; when it goes, focus takes the control in its place in `#sy-head`. Check for
  updates: "Checking N emulators for updates…", then the desktop's result card with each update as its sub line.
- **Pages** `pages/TvSystemPage.svelte` (props `system` = label or server name | `windows` | `emulator` for an
  Other emulator), pure `pages/systemPage.ts` (`systemPageRows` per kind: emulator, none, windows, other; plus
  `emulatorListItems`, `protonListItems`, `removeConfirm`, `pageSub`, `umuValue`). Row keys `s-emu|ver|update|core|
  fw|ps3fw|open|settings|remove|tips|install|choose|proton|umu|umu-install|first`, companions `s-comp-<key>-*`.
  `gameActions.openSystem(system)` pushes it (Server chips, "Set up <Platform>"); the stub is gone.
  `pages/TvEmulatorForm.svelte` + pure `pages/emulatorForm.ts`, opened by `emulatorFormOpen.svelte.ts`
  `openEmulatorForm(mode, entry)`: the fields are a `$state` object in the page's props, so the Program browser
  (a page over the form) doesn't lose them. Rows `s-which|name|prog|args|strategy|save-paths|state-paths|
  ignore-files|ignore-ext|save`.
- **Install recommended's count:** `tvRec.keys` (systemsTv) holds the systems it was pressed for; `installingCount`
  counts those (and any no-emulator system) still installing, since a system can show a fallback emulator (a
  hand-added one with no profile plays every system) while its own install runs.
- **Focus:** the pane is wrapped in a `data-edge` area, so Up from the header buttons stays in the pane (it used to
  reach the rail and switch the pane); the system rows (`#sy-list`) and Other emulators (`#sy-others`) are their own
  `data-nav="v"` lists, so Down from the header reaches the first row.
- **Mock forge:** `POST /__e2e__/slow?ms=<n>` delays every download (not the api.github.com JSON) by n ms.
- **e2e:** group `tv-systems` (`specs/tv-systems.spec.ts`, fixtures-systems + mock forge, no seed; connects on
  the desktop). tv-details-c now opens the real System page from "Set up Nintendo 64" and covers 530 and 531;
  tv-server's chip test waits for `tv-system-page`. Row text in specs: TvRows puts label, line and value in
  separate divs with no spaces, so tv-systems reads rows with its own `rowText` (`.t`, `.s`, `.val` joined).
- **Not done / open (TODO in code):** "Used for <system>" under "<emulator> installed" (item 260; the
  `emulator-installed` event names no system); the why texts for the disabled Installing button, Check for updates
  while checking, a busy row, and Save with no name are mine (TODO(design)); "Show program folder" (the desktop's ⋯
  menu) isn't offered in TV.

## Pending for the next phase (consultant, after P11)
- Checklist 260 "Used for <system>": app/src-tauri/src/commands.rs `EmulatorInstalledEvent` gains `used_for: Vec<String>`
  (platform labels) from the `PendingOutcome::Assigned` outcomes `apply_after_install` returns for this emulator;
  app/src-tauri/src/lib.rs (set_emulator_installed_hook closure, ~l.578–607): move the `emulator-installed` emit into the
  existing spawn_blocking after apply_after_install so the payload carries them, then emit `setup-defaults` as today.
  api.ts type; notifications/notices.ts: `Notice.sub?`, `emulatorInstalledNotice(name, fresh, usedFor)` sets sub only when
  fresh && usedFor non-empty ("Used for A", "Used for A and B", "A, B and C"); notifier.ts coalescer passes
  `{ sub: card.sub, action }` to pushToast (summary cards have none). Desktop Toast ignores sub (card unchanged). Tests;
  tv-systems e2e if it asserts the card.
- Reasons: panes/systems.ts `REC_RUNNING` → "Still installing. Downloads shows the progress."; `CHECK_RUNNING` stays
  "Already checking for updates."; pages/systemPage.ts `BUSY_WHY` → "Still in progress. Try again when it finishes.";
  pages/emulatorForm.ts `NAME_NEEDED` stays "Give the emulator a name first."; remove the four TODO(design); tests.
- No "Show program folder" on the TV System page (close the open item).

## After P12: the startup screens and where TV opens
- **Done from the pending lists:** checklist 532–534 (the "after P10" section, now removed). `panes/tv.ts`
  `ASSIGN_NEEDS_PAD` is the ruled text and the row reads "Xbox style" with no controller; `panes/ra.ts` has
  Password and Log In after Save (`saveWhy` / `loginWhy` name the missing field; `RaState.passwordTyped`,
  `canLogin`; `RaActions.editPassword`, `login`); TvSettings keeps the password in a plain local forgotten with the
  token, Log In runs `raLogin`, blanks it and refreshes the status. `downloadRows.ts` `rowWhy` by kind
  (`FIRMWARE_WHY`, `MOVE_WHY`) and `rowA` "Details" when only Open remains. The "after P11" pending section
  (Used for, Systems reasons, Show program folder) is untouched: held back from this phase.
- **Phases** (`TvShell`): `phase` = loading | unreachable | connect | choose | shell, from `uiSettings.loaded`,
  `libraryUnreachable.blocking` and `session.phase`. Each startup phase calls `nav.resetStartup(page)`: the
  startup screen is the only page on nav's stack (so `press.ts`'s `pages > 1` rule lets B leave the folder
  browser but never the screen, item 97) and TvStage draws it like any page. Entering `shell` runs `startShell()`:
  the desktop view TV was entered from (first start only), else `initialView` (Discover selected on Server; the
  first Backup drive selected, its folder open and its row focused without a server, also once the drives load
  late: `pendingDrive`). The welcome card shows once per TvShell mount. `ScreenHooks.view` is the screen's View
  when no card offers an action (TvShell's registered View).
- **Screens** (`lib/tv/startup/`): `TvConnect` + pure `connectPage.ts` (rows, keyboard titles, `connectReady`,
  `connectWhy`, `offlineReady`, `parentOf`) + `connectForm.svelte.ts` (the fields outlive the folder browser's
  remount; the token or password is a plain module local, `setSecret` / `takeSecret`, only `secretTyped` is
  reactive; the stored library is not stored again). Keys `cf-url|how|token|user|password|lib`, buttons
  `cf-go|cf-none`; test ids `tv-connect`, `tv-cf-<key>` / `tv-cf-<key>-value`, `tv-connect-go|offline|reason`.
  `TvChooseSystems` + pure `choose.ts` over the desktop's `ChooseSystems` class: rows `ch-<slug>` and `ch-windows`
  (`tv-ch-row-*`, `tv-ch-val-*`), buttons `ch-go|ch-skip` (`tv-choose-install|skip`), intro `tv-choose-intro`.
  None in the X list unticks the system. `TvUnreachable` (an empty base screen whose `arrive` opens the blocking
  `TvUnreachableDialog` layer; `ur-relocate|ur-close`, `tv-unreachable*`) + `unreachable.ts`; Relocate closes the
  dialog and pushes the folder browser (B there reopens it); `relocateTo(path, notice)` gained an optional notice
  so TV says "Library relocated to <path>" / "No files were moved". `TvSetupCard` (renderless, mounted in the
  shell phase) + pure `setupCard.ts` drive the P6 held / pinned notice from `setupRun` and the Downloads rows;
  `setup/progress.ts` now exports `latestRows`. The card closing ends the run (`dismissSetupRun`).
- **Rust:** e2e-only `e2e_clipboard_write(text)` (commands/tv.rs) seeds the clipboard for View-to-paste.
- **e2e:** group `tv-startup` (first-run-setup fixtures, mock forge, `E2E_FIRST_RUN_SETUP`, drive roots, seeded per
  spec by `seed/tv-startup-seed.mjs`, which empties the data dir first): a Connect / Choose / Start skips, b
  unreachable + Relocate, c Use without a server → first drive, d Install 2 emulators + setup card, e Close sends the quit with no
  question (the e2e build's `window.__tvE2EHoldQuit` counts it in `__tvE2EQuits` instead of exiting, which would
  end the WebDriver session). The keyring credential is cleared before a, c and d. Choose your systems draws its
  buttons only once the systems (or their failure) arrive, so focus starts on the first system.
- **Deviations / open:** Connect reads "Connecting…" while the connect runs (the prototype connects at once);
  a failed relocate shows the refusal as an error card and reopens the dialog; an unavailable system's A says
  "No emulator available" (TODO(design) wording); a connect failure other than a rejected credential shows the
  backend's message on the reason line. The unreachable dialog's two buttons wrap onto two lines in its 40rem box
  (as on the prototype, same CSS), so Down, not Right, reaches Close. The setup card unpins when its countdown
  starts (as on the prototype), so later cards can push it out before the 10 s end.

## Pending for P14 (consultant, after P12)

Code changes from the consultant's Phase 12 rulings (README § Consultant decisions, last four bullets):
- `startup/choose.ts`: replace the TODO(design) with `noEmulatorWhy(label)` → "No emulator GRID installs plays
  <System>. If you have one, add it in Settings › Systems after setup." (`NO_EMULATOR` stays the row's value);
  `TvChooseSystems.svelte` puts it on the disabled row's `data-why`; no X on that row; test in `choose.test.ts`.
- `startup/connectPage.ts` + `TvConnect.svelte`: while a connect runs, Connect reads "Connecting…", Connect and
  Use without a server are disabled, `why` "Still connecting. Wait for it to finish." (A and Start).
- `TvConnect.svelte` `paste()`: empty clipboard on a field → notice "Nothing to paste: the clipboard has no
  text."; not on a field → "Focus a field to paste into it." (plain notices). Cover in `tv-startup-a` if cheap.
- `notices.svelte.ts` `releaseNotice` keeps `pinned` (update its comment, `notices.svelte.test.ts`, `notices.ts`
  header, `TvSetupCard.svelte` comment); the setup card stays pinned through its 10 s countdown. The "after P12"
  note above about unpinning is superseded.

## After P13: Game backups in TV mode
- **Done from the pending lists:** the "after P11" consultant section: `emulator-installed` carries `used_for`
  (app/src-tauri/src/commands.rs `used_for()` over the `Assigned` outcomes; lib.rs now emits it inside the
  spawn_blocking after `apply_after_install`), `notifications/notices.ts` `Notice.sub`,
  `emulatorInstalledNotice(name, fresh, usedFor)` ("Used for A and B", fresh installs only), the coalescer passes
  `sub`; the Systems / System page / emulator form reasons take the ruled words and their TODO(design) markers are
  gone; the TV System page never had Show program folder (closed). Also: a game queued from a drive gets the sub
  "Copying from <label>" (item 426), and a finished drive copy's Downloads line reads "Installed from <label>"
  (`downloadRows.ts`, item 425).
- **Select mode** (`lib/tv/select.svelte.ts`, over the desktop's now generic `backups/selection.ts`): `select.on`,
  `select.ids` (the covers' `TvGame.id`s), `selToggle / selPick / selAll / selKeep / selEnd / selAct`, pure
  `selActOutcome`, `selBar`, `selHints`. The grid on show registers `registerSelectGrid({games, drive, connected,
  act})` (TvLibrary, TvPlatform); nav's `selMode` reads `select.on`, `run()` handles `selToggle|selPick|selAll|
  selAct` (selPick reads the cover's `data-sel`). Covers get `selectable` (TvGameCover → `sel` / `picked` on
  TvCover). `ui/TvSelBar` (test ids `tv-selbar`, `-summary|-x|-y|-rt`) pins under the grid. Select mode ends on
  another tab (`showTab`), another rail row (`selectLibRail`, Server `selectRail`, as the desktop's), the drive
  going away (TvLibrary's fallback effect), a started backup and either drive action. The hint bar shows
  `currentSelHints` while no page is pushed.
- **Backup page** (`pages/TvBackupPage.svelte` over the shared `BackupPlanStore`; pure `pages/backupPage.ts`):
  rows `bk-drive-<mount>` (radio), `bk-setup`, `bk-local`, `bk-wine`, `bk-ood-note`, `bk-ood-<rom>`, `bk-all-local`,
  `bk-all-download`, `bk-each`, `bk-src-<rom>`, `bk-emus`, `bk-fw`, `bk-runtime`, `bk-note-<i>`, `bk-error`; each
  as `tv-pr-<key>`. Opened by `backupActions.ts` `openBackupPage(subject, romIds, platformIds?)` from Details,
  select mode's Y (Library, Server), Server's Backup all N… and X on a Library platform row (offline: the
  "Backing up needs a server." card). Words come from `backups/dialog.ts` (lede, local line, source rows, emulator
  line, notes, set-aside line) so desktop and TV can't drift.
- **Rows pages:** `pages/pageRows.ts` (types), `ui/TvPageRows.svelte`, `pages/TvRowsPage.svelte` (title `tv-rows-title`,
  sub `tv-rows-sub`, footer `tv-rows-summary`, main button `tv-rows-main` / key `main`, Start presses it, a
  disabled one says why). The Backup page and the three drive pages use it.
- **Settings › Backup drives:** `tabs/settings/TvDrivesPane.svelte` + pure `panes/drives.ts`; rows `dr-<mount>`
  (test id `tv-drive-row`, `data-drive-key` = mount, `tv-drive-sub`, `tv-drive-status`), `tv-drives-none`,
  `tv-drives-firmware` (`dr-fw`). Pages: `TvDrivePage` (rows `dp-os|space|games|emus|fw|last|folder|show|open|rename|
  remove`), `TvDriveOsPage` (`os-linux|windows`; turning one off asks, ruling 9), `TvDriveSetupPage` (`su-name|
  su-linux|su-windows|su-fat32|su-note`; this computer's OS ticked, ruling 14), pure `pages/drivePages.ts`. Show in
  Library is `library.svelte.ts` `showDriveInLibrary` (`selectDriveRow` moved there from TvShell). Open folder
  leaves TV mode, then opens the folder. `driveActions.ts` `setUpDrive` / `removeGridFiles` put the TV second
  line on their cards ("<label> · <fs>", "<size> freed"; the desktop card is unchanged).
- **Two nav fixes every screen gets:** `showScreen` blocks `stageChanged` until the new screen is drawn (the ring's
  class mutation ran the observer before Svelte drew the new screen, so it restored focus among the old screen's
  items: pushing a page from Settings or a Server grid selected Appearance / Discover). `restoreFocus`, when the
  remembered item has gone and nothing is autofocus, lands on the rail's selected row instead of the first item
  (a game removed from the drive while its Details was open used to select All games).
- **e2e:** group `tv-backups` (fixtures-backups, fake drives, `backups-seed.mjs` variants `tv-backups-a…d`): a set up
  a drive with Start, RT/A/X/Y, the Backup page, Start, Downloads; b unplug mid-backup (failed row, folded row, one
  card, rail falls back, plug back in); c used without a server: select mode on a drive, Details from a drive,
  X Install from drive with the drive's emulator and firmware notices, Backup all offline, Remove from drive…; d
  firmware default, Change… refusal, a FAT32 setup and Rename…, Remove GRID files asks twice, Forget. VITE_E2E
  builds expose `__tvE2E.notices()` (the session's notices, "title | sub") and `__tvE2E.memory()` (focus memory);
  `helpers/tvBackups.ts` has `tap`, `tapK`, `waitNotice`, `pageRow`, `confirmDialog` (waits for its own layer, so
  a second question can follow), `screenState` for failure messages.
- **Checklist:** 94, 418, 419, 421–514, 520 built; 420 verified (tv-backups-b/c). 438 is the backend's refusal
  (grid-core `BACKING_UP_LOCAL`) shown through the shared actions; not driven in a TV spec. The running backup in
  the top bar (ruling 21) is `topbar.ts` from P5 (unit-tested); not asserted end to end (the e2e backups finish
  too fast to catch it reliably).
- **Decisions taken here (report them to the user):**
  - Phase 8's question, merge the queued and the installed drive-copy cards? No: they stay two events, as the Game
    backups card and README § The shell ("the same events and wording as the desktop") have them: "<title> added to
    download queue · Copying from <label>", then "<title> installed"; the emulator and firmware get their own cards.
  - Where the checklist quotes tvmode.js but the shared notifier has the desktop's words, TV keeps the desktop's
    (README: notifications are the desktop's events and wording): the removal card "Removed <title> from <pack>"
    (434, 451), the mid-backup removal card "<label> was removed. <n> of <m> items were written to <pack>." (492),
    the OS-removal card (504), the lede "GRID downloads each game from <host> and writes it, …" (458), the runtime
    line (475). The Backup page's own words (464 no-drive note, 467–469 version rows, 474 "<n> systems · <n> files",
    481 reasons) follow the checklist.
  - Select mode ends when the rail row changes (the desktop's rule; the prototype kept it).
  - The firmware switch reads "None needed" when the plan has no firmware sets and "Already on the drive" when all
    are there (no words in the sources); a still-planning drive's main button reads "Checking the drive…" with the
    why "GRID is still checking the drive."
- **For P14:** the "Pending for P14" section above is untouched. `tv-layout` should walk the Backup page (long:
  per-game rows, notes), the drive pages and select mode's bar. `selKeep` drops picks a search hides (the desktop's
  `keepOnly`); the prototype kept them.
- **Results:** gate PASS; e2e `tv-backups` (a–d), `tv-shell`, `backups`, `tv-library`, `tv-details`, `tv-settings`,
  `tv-server`, `tv-startup`, `tv-systems` pass (there is no `tv-downloads` group: Downloads is in `tv-settings`).
  `tv-library` changed: its first test now walks up from the first drive to All games, because GRID used without
  a server opens on the first drive (item 415, built in P12); it had passed only through the observer race fixed
  above. The nav fix commit (d816ecb) imports `select.svelte.ts`, which lands in the next commit (7cbac73).

## Pending for P14 (consultant, after P13)

From the consultant's Phase 13 rulings (README § Consultant decisions, last six bullets):
1. `tabs/library/TvLibrary.svelte` (~141) and `tabs/server/TvPlatform.svelte` (~68): call `selKeep` with the rail
   row's whole list (Library: the rows / drive list before `libraryGrid`/`driveGrid` apply `lib.query`; Server:
   `all`, not `shown`) and register that whole list for sizing; fix the comments that say a search drops picks.
2. `select.svelte.ts`: `selBar` takes the shown games (X's count) and the whole list (the summary's size);
   `currentSelHints`'s `all` uses the shown ids; `selAct` passes every pick. `select.svelte.test.ts`: replace "drops
   hidden picks" with "keeps picks a search hides; the summary counts them; Select all N counts the shown covers".
3. `pages/backupPage.ts` `firmwareText` (~117–121): remove the "None needed" / "Already on the drive" branches; with
   no reason and no set to write answer `{ text: '', disabled: true }` like `dialog.ts firmwareLine`;
   `backupPage.test.ts` asserts the two backend reasons verbatim and disabled.
4. `e2e/specs/tv-backups-b.spec.ts`, second `it`, after the wait that sees "Big Arcade Game" downloading and before
   `unplug(DRIVE)`: `expect(await text('tv-topbar-download')).toMatch(/^Big Arcade Game\b.*\d+%$/)`.
5. New `e2e/specs/tv-backups-e.spec.ts` (checklist 438): seed case `tv-backups-e` → `seedD()` in
   `e2e/seed/backups-seed.mjs`; add the spec to the `tv-backups` group in `scripts/e2e.sh` (~197) and the variant to
   the keyring-clear pattern (~811). Flow: Server › PS2 → X on Big Arcade Game (903, throttled ~10 s) → Library ›
   All games → Details on Cinder Peaks (901) → Backup to drive… → Start (queued behind the install) → Details on
   Cinder Peaks → Uninstall → A on the confirm → notice "This game is being backed up. Wait for it to finish." →
   let the queue drain, no page errors.
6. `docs/tv-mode/checklist.md`: tick 434, 451, 492, 504, 458, 475 "(desktop wording, intro rule)"; mark 452 "also
   ends on a rail row change (consultant)"; correct "After P13" here: 425 is asserted in tv-backups-c, and the
   select-mode search note is superseded.
