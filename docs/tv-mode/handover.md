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

## Pending for the next phase (consultant, after P5)
- Desktop TV button (app/src/lib/Shell.svelte, TODO(design)): make it a 28×28 `icon-btn` like the
  theme toggle, new outline `tv` icon at size 18, colour `var(--text-h)`, shared `.icon-btn`
  hover/focus states, no animation, kept just left of the light/dark toggle (after the update
  badge); `aria-label` and `title` "Switch to TV mode"; keep `data-testid="tv-mode-button"`; drop
  the `.tv-btn` text styles, add only `.tv-btn:disabled { color: var(--text-muted); cursor: default; }`.
  Add to app/src/lib/icons.ts (outline, not in FILLED_ICONS) and mirror into
  docs/design/prototype/icons.json:
  `tv: 'M5 8h14a1.5 1.5 0 0 1 1.5 1.5v8.5a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5V9.5A1.5 1.5 0 0 1 5 8zM8 3.5l4 4.5 4-4.5'`
- Space stays A (`' ': 'A'` in tv/input/buttons.ts); while the OSK is open the printable-key branch
  runs first, so Space types a space and marks physical typing (Enter = Done).

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
