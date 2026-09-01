# 09 — TV mode (10-foot UI, controller input, navigation)

## Purpose

This document describes GRID Launcher's TV mode: a fullscreen, controller-first
"10-foot" interface that replaces the desktop window while active. It covers how TV mode
is entered and left, how raw gamepad hardware events become abstract navigation events,
how focus moves inside and between screens, what each screen shows and does, what the
view-to-backend API surface looks like, and how the pause overlay interrupts a running
game.

It is written so the behavior can be reimplemented without Qt. Widget classes are named
only as source anchors; the described contracts are: *an input source produces direction
events*, *a router delivers them to exactly one focus owner*, and *each screen owns an
index-based focus model*.

Language-neutral vocabulary used throughout:

- **navigation event** — one of the nine strings `up`, `down`, `left`, `right`,
  `confirm`, `back`, `tab_prev`, `tab_next`, `guide_button`
  (grid_launcher/tv/bridge/controller.py:308).
- **view** — a screen-level component that implements `handle_nav(direction)`
  (grid_launcher/tv/widgets/views/).
- **overlay** — a component drawn over a view that consumes navigation events before the
  view sees them.
- **shell** — the TV window that owns the tab strip, the view stacks, the controls hint
  bar, and the navigation router (grid_launcher/tv/widgets/window.py:17).
- **backend** — a non-visual object that exposes data (properties), commands (slots), and
  change events (signals) to views (grid_launcher/tv/bridge/).

Cross-references (this document does not re-derive their logic):

- Server queries and payload shapes: `docs/porting/01-romm-api.md`.
- Config keys and persistence: `docs/porting/02-config-and-secrets.md`.
- Download/install/uninstall semantics: `docs/porting/03-library-install.md`.
- Emulator selection and process launch: `docs/porting/04-emulator-launch.md`.
- Cloud save slot/restore/upload semantics: `docs/porting/06-cloud-saves.md`.
- Cover/fanart fetching and caching: `docs/porting/07-covers-images.md`.
- Worker/thread lifetimes: `docs/porting/08-background-threading.md`.

---

## External surfaces

### Input devices

TV mode reads gamepads directly; it does not rely on the windowing system for gamepad
events.

- **Windows**: `XInput1_4.dll`, falling back to `XInput9_1_0.dll`, then `XInput1_3.dll`
  (grid_launcher/tv/bridge/controller.py:90). The entry point used is **ordinal 100**
  (`XInputGetStateEx`), which exposes the Guide button as bit `0x0400`; if the ordinal is
  unavailable the code falls back to the public `XInputGetState`, which does not report
  Guide (grid_launcher/tv/bridge/controller.py:119).
  Four controller slots (`user_index` 0–3) are polled each pass
  (grid_launcher/tv/bridge/controller.py:138).
- **All other platforms**: SDL via `pygame.joystick`
  (grid_launcher/tv/bridge/controller.py:242). If `pygame` is not importable, the thread
  prints a warning to stderr and exits, leaving keyboard as the only input
  (grid_launcher/tv/bridge/controller.py:244).

Environment variables set by the SDL path before initialization
(grid_launcher/tv/bridge/controller.py:248):

| Variable | Value | Note |
| --- | --- | --- |
| `SDL_VIDEODRIVER` | `dummy` | set only if unset (`setdefault`) |
| `SDL_AUDIODRIVER` | `dummy` | set only if unset (`setdefault`) |
| `SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS` | `1` | always overwritten; required so a running game in the foreground does not starve the launcher of Guide-button presses |

### Keyboard

The shell maps a fixed key set onto the same navigation events
(grid_launcher/tv/widgets/window.py:393): arrows → `up`/`down`/`left`/`right`,
Return/Enter → `confirm`, Backspace → `back`, End → `tab_prev`, PageDown → `tab_next`,
Escape → `guide_button`. The pause overlay accepts a subset directly
(grid_launcher/tv/widgets/pause_window.py:166).

### Files

TV mode introduces no file formats of its own. It reads and writes the shared config file
`~/.grid-launcher/config.json` (grid_launcher/tv/bridge/app_backend.py:530), and reads
image assets:

- Controller glyph PNGs from `assets/retroarch-assets/<stem>.png`, used by the controls
  hint bar (grid_launcher/tv/widgets/components/controls_bar.py:17) and by the details
  lightbox hint strip (grid_launcher/tv/widgets/views/details_view.py:30).
- Themed SVG icons through the shared desktop icon helper
  (grid_launcher/tv/widgets/views/details_view.py:10).
- Cover/fanart/platform-logo images through the cover loader — see
  `docs/porting/07-covers-images.md`.

Config keys owned or read by TV mode:

| Key | Read by | Written by |
| --- | --- | --- |
| `tv_guide_button_exclusion_list` | guide suppression (grid_launcher/tv/bridge/controller.py:424), settings list (grid_launcher/tv/widgets/views/settings_view.py:476) | add/remove exclusion (grid_launcher/tv/bridge/app_backend.py:292, :307) |
| `tv_mode_home_view` | `homeViewTab` property (grid_launcher/tv/bridge/app_backend.py:254) | `setHomeViewTab` (grid_launcher/tv/bridge/app_backend.py:326) |
| `auto_cloud_save_download_on_launch` / `auto_cloud_save_upload_on_exit` | `isAutoSync` (grid_launcher/tv/bridge/app_backend.py:268), launch/exit sync (grid_launcher/tv/bridge/game_backend.py:318, :827) | `setAutoSync` (grid_launcher/tv/bridge/app_backend.py:330) |
| `installed_games` | library rows, install state (grid_launcher/tv/bridge/app_backend.py:157) | install finalize (grid_launcher/tv/bridge/game_backend.py:687) |
| `native_executable_path` (per installed game) | details "Change Executable" (grid_launcher/tv/widgets/views/details_view.py:737) | `saveNativeExecutable` (grid_launcher/tv/bridge/game_backend.py:755) |
| `last_played` (per installed game) | Library "Recently Played" filter (grid_launcher/tv/widgets/views/library_view.py:257) | session end (grid_launcher/tv/bridge/game_backend.py:799) |

### Processes

TV mode spawns emulator/game processes through the same command construction as desktop —
see `docs/porting/04-emulator-launch.md`. The TV-specific parts are the process handle
lifetime and suspend/resume, documented under *Pause flow* below.

---

## Data model

### View inventory

Two stacks exist. The **inner stack** holds the three tabbed root screens; the **outer
stack** holds the root container at index 0 plus at most one pushed screen
(grid_launcher/tv/widgets/window.py:48, :59).

| View | Stack / index | Constructed | Entry condition |
| --- | --- | --- | --- |
| Home | inner 0 | at shell construction (grid_launcher/tv/widgets/window.py:62) | tab 0 |
| Library | inner 1 | at shell construction (grid_launcher/tv/widgets/window.py:73) | tab 1 |
| Server | inner 2 | at shell construction (grid_launcher/tv/widgets/window.py:84) | tab 2 |
| Details | outer, pushed | per selection, from Home/Library/Server (grid_launcher/tv/widgets/views/home_view.py:318, grid_launcher/tv/widgets/views/library_view.py:635, grid_launcher/tv/widgets/views/server_view.py:258) | `confirm` on a game card |
| Settings | outer, pushed | on demand (grid_launcher/tv/widgets/window.py:147) | `guide_button` at root with no active session |

Server internally holds a two-page stack: platform grid (page 0) and game wall (page 1)
(grid_launcher/tv/widgets/views/server_view.py:67, :114).

The tab strip has exactly three labels, `Home`, `Library`, `Server`
(grid_launcher/tv/widgets/tab_bar.py:11), and a bottom hint bar renders the active view's
`CONTROL_HINTS` list (grid_launcher/tv/widgets/window.py:372).

### Overlay inventory

| Overlay | Owner | Shown by | Consumes nav before | Marks `uiOverlayActive` |
| --- | --- | --- | --- | --- |
| Pause window | shell (separate top-level window) | `pauseRequested` → `openForActiveSession` (grid_launcher/tv/bridge/pause_backend.py:37) | everything, including shell globals (grid_launcher/tv/widgets/window.py:135) | no |
| Screenshot lightbox | Details view | `confirm` on a focused screenshot (grid_launcher/tv/widgets/views/details_view.py:492) | cloud/native overlays and the view (grid_launcher/tv/widgets/views/details_view.py:426) | no |
| Cloud saves overlay | Details view | "Cloud Saves" button (grid_launcher/tv/widgets/views/details_view.py:742) | the view (grid_launcher/tv/widgets/views/details_view.py:439) | yes (grid_launcher/tv/widgets/components/cloud_saves_overlay.py:100) |
| Native executable picker | Details view | "Change Executable" button, or `nativeExecPickerNeeded` during launch (grid_launcher/tv/widgets/views/details_view.py:738, :1042) | the view (grid_launcher/tv/widgets/views/details_view.py:443) | yes (grid_launcher/tv/widgets/components/native_exec_picker.py:81) |
| Emulator picker | Settings view | "+ Add Exclusion" (grid_launcher/tv/widgets/views/settings_view.py:589) | the view (grid_launcher/tv/widgets/views/settings_view.py:333) | yes (grid_launcher/tv/widgets/components/emulator_picker_overlay.py:75) |

### Reusable focusable components

| Component | Focus model | Selection event |
| --- | --- | --- |
| `GameRow` | single index into a horizontal list; `left`/`right` clamp at both ends (grid_launcher/tv/widgets/components/game_row.py:95) | `game_selected(game)` on `confirm` (grid_launcher/tv/widgets/components/game_row.py:110) |
| `GameWall` | single index into a row-major grid; `±1` horizontally, `±columns` vertically, clamped to `[0, n-1]` (grid_launcher/tv/widgets/components/game_wall.py:125) | `game_selected(game)` on `confirm` (grid_launcher/tv/widgets/components/game_wall.py:139) |
| `LibraryView` carousel | fixed 11-slot ring of cards, focus is always the center slot; the *data* index moves (grid_launcher/tv/widgets/views/library_view.py:19, :133) | `game_selected(game)` on `confirm` (grid_launcher/tv/widgets/views/library_view.py:430) |
| `HomeView` row stack | one active row index of four; the row owns horizontal focus (grid_launcher/tv/widgets/views/home_view.py:69, :175) | forwarded from the active row (grid_launcher/tv/widgets/views/home_view.py:309) |
| Platform grid (Server) | same index math as `GameWall`, computed inline (grid_launcher/tv/widgets/views/server_view.py:156) | direct call to `_on_platform_selected` on `confirm` (grid_launcher/tv/widgets/views/server_view.py:152) |
| Settings rows | flat list of focus entries with a special "action group" (grid_launcher/tv/widgets/views/settings_view.py:288, :275) | type-dispatched activation (grid_launcher/tv/widgets/views/settings_view.py:539) |

Card visuals: focused cards use the accent color for their border, unfocused use the
inactive border color (grid_launcher/tv/widgets/components/home_card.py:70,
grid_launcher/tv/widgets/components/game_card.py:61, grid_launcher/tv/widgets/components/platform_card.py:49). Grid cards animate a 1.05 scale over 120 ms on
focus (grid_launcher/tv/widgets/components/game_card.py:20, :35). Fixed card sizes:
home row card 780×260 (grid_launcher/tv/widgets/components/home_card.py:23), wall card 296×480 (grid_launcher/tv/widgets/components/game_card.py:24), platform
card 280×190 (grid_launcher/tv/widgets/components/platform_card.py:22), library carousel card 200×300 grown to 1.20×
(grid_launcher/tv/widgets/views/library_view.py:16).

### Theme constants

`grid_launcher/tv/widgets/theme.py` is the single source of color values for the whole TV
stack; it defines ten flat string constants (`BG`, `PANEL`, `TERTIARY`,
`BORDER_INACTIVE`, `TEXT_PRIMARY`, `TEXT_SECONDARY`, `ACCENT`, `PURPLE`, `SUCCESS`,
`ERROR`) and nothing else (grid_launcher/tv/widgets/theme.py:3). Its role is to keep focus
color (`ACCENT`), section headings (`PURPLE`), and status feedback (`SUCCESS`/`ERROR`)
consistent across every TV component. A port only needs an equivalent palette table; the
values themselves are not behavioral.

### Backend API surface — `AppBackend`

Config, library data, server catalog, favorites, and TV settings
(grid_launcher/tv/bridge/app_backend.py:51).

| Member | Kind | Input | Behavior / output |
| --- | --- | --- | --- |
| `libraryGames` | property (:157) | — | Installed games from config, excluding platform `Emulators`; fills missing `local_path` from any known path key; merges nine metadata fields from the server catalog index when absent; annotates `has_cloud_saves` and `is_favorite` |
| `favoritesGames` | property (:201) | — | Server favorites collection, each forced `is_favorite="true"`, enriched with local paths |
| `newAdditionsGames` | property (:205) | — | Recently added server ROMs, annotated with favorite state |
| `highlyRatedGames` | property (:211) | — | High-rating server ROMs, annotated with favorite state |
| `platforms` | property (:217) | — | Sorted platform labels |
| `platformDetails` | property (:221) | — | Platform records including `url_logo` and `local_logo_path` |
| `isConnected` | property (:225) | — | True once a catalog fetch produced platforms |
| `tvGuideExclusionList` | property (:229) | — | Non-empty string entries from config |
| `availableEmulatorNames` | property (:237) | — | Configured emulator names *not* already excluded, sorted |
| `homeViewTab` | property (:254) | — | `home` \| `library` \| `server` |
| `serverUrl` | property (:261) | — | Config server URL |
| `isAutoSync` | property (:268) | — | Auto cloud save on launch/exit flag, default true |
| `uiOverlayActive` | property (:272) | — | Whether a modal overlay is up |
| `syncConfig(config)` | slot (:120) | config object | Rebinds config; if server URL changed, clears the cached catalog; emits library/platform/home-tab change events |
| `connectToServer()` | slot (:386) | — | No-op without credentials (emits status text); no-op if catalog already fetched; else starts catalog fetch |
| `loadPlatformGames(label)` | slot (:412) | platform label | Re-emits `serverGamesChanged` if already cached; else starts a ROM list fetch |
| `serverGamesForPlatform(label)` | slot (:396) | platform label | Returns cached games with local-path, cloud-save, and favorite annotations |
| `fetchRomMetadata(game_json)` | slot (:346) | JSON game | Skips if six key fields are already non-empty, or without credentials, or if a fetch for that ROM is in flight; else emits `romMetadataFetchStarted` and fetches detail metadata |
| `toggleFavorite(rom_id)` | slot (:441) | ROM id string | Fetches the favorites collection, adds/removes the id, creates the collection if none exists, then re-fetches favorites |
| `setHomeViewTab(view)` / `setHomeView(view)` | slots (:326, :322) | one of the three tab names | Validates, persists, emits |
| `setAutoSync(enabled)` | slot (:330) | bool | Writes both the download-on-launch and upload-on-exit flags, drops the legacy `auto_cloud_sync` key, persists |
| `addExclusionEntry(name)` / `removeExclusionEntry(name)` / `setGuideExclusionList(list)` | slots (:292, :307, :288) | emulator name(s) | Case-insensitive dedupe/removal, persists, emits |
| `setUiOverlayActive(active)` | slot (:340) | bool | Emits only on change |
| `getInstalledLocalPath(rom_id)` | slot (:427) | ROM id | First non-empty of `local_path`, `extracted_path`, `extracted_dir`, `archive_path` |
| `requestDesktopMode()` / `requestQuit()` | slots (:280, :284) | — | Emit shell-exit requests handled by the desktop window |
| `logHandleDiag(label)` | slot (:422) | label | Optional diagnostic callback hook |

Events: `libraryGamesChanged`, `platformsChanged`, `serverGamesChanged(label)`,
`connectionStatusChanged(text)`, `switchToDesktopModeRequested`, `quitRequested`,
`exclusionListChanged`, `exclusionDataChanged`, `homeViewTabChanged(view)`,
`autoSyncChanged(bool)`, `overlayStateChanged`, `favoritesGamesChanged`,
`newAdditionsGamesChanged`, `highlyRatedGamesChanged`,
`favoriteToggleComplete({rom_id, is_now_favorite})`,
`romMetadataReady({rom_id, metadata_json})`, `romMetadataFetchStarted(rom_id)`,
`saveConfigRequested` (grid_launcher/tv/bridge/app_backend.py:54).

### Backend API surface — `GameBackend`

Install, launch, session, and native-executable handling
(grid_launcher/tv/bridge/game_backend.py:129). The install and launch *logic* is the same
as desktop — see `docs/porting/03-library-install.md` and
`docs/porting/04-emulator-launch.md`.

| Member | Kind | Input | Behavior / output |
| --- | --- | --- | --- |
| `activeEmulatorName` | property (:170) | — | Emulator name of the running session, empty for native games |
| `activeGameTitle` | property (:174) | — | `title`, else `name`, else empty |
| `isSessionActive` | property (:187) | — | True while the spawned process has not exited |
| `isInstallActive` | property (:192) | — | True while either the download thread or the finalize thread runs |
| `syncConfig(config)` | slot (:202) | config | Rebinds config |
| `launchGame(game)` | slot (:206) | game record | Rejects invalid payloads and concurrent sessions; routes native-PC games to the native path; otherwise builds the emulator command; if auto-sync is on and credentials exist, runs a cloud restore first and launches in its completion handler |
| `stopGame()` | slot (:440) | — | Terminates the process, clears session state, emits `sessionEnded("")` |
| `pauseEmulator()` / `resumeEmulator()` | slots (:455, :468) | — | Suspend/resume the process tree via `psutil`; no-op when `psutil` is absent or the process already exited |
| `requestPause()` | slot (:481) | — | Emits `pauseRequested` only while a session is active |
| `installGame(game)` | slot (:486) | game record | Validates ROM id, server URL, library path; refuses concurrent installs; downloads to `<library>/<platform>/<rom_file_name>`, then finalizes (extraction, PS3 handling) on a plain thread; TV mode deliberately skips supplemental archives and firmware (:647) |
| `cancelInstall()` | slot (:706) | — | Requests cancellation on the download worker |
| `uninstallGame(game)` | slot (:711) | game record | Delegates to the desktop window's uninstall routine and reports success/failure |
| `getNativeExecutableCandidates(rom_id)` | slot (:728) | ROM id | List of `{label, path}` for executables under the install dir |
| `saveNativeExecutable({rom_id, exe_path})` | slot (:755) | bundle | Writes `native_executable_path` into the installed game record and persists config |

Events: `sessionStarted(emulator)`, `sessionEnded(emulator)`, `launchError(text)`,
`sessionPaused`, `sessionResumed`, `pauseRequested`,
`installProgress({downloaded, total, speed})`,
`installComplete({success, message, game})`, `uninstallComplete({success, message, game})`,
`cloudSyncStatus(text)`, `nativeExecPickerNeeded([{label, path}])`
(grid_launcher/tv/bridge/game_backend.py:130).

### Backend API surface — `PauseBackend`

Pure state holder for the pause overlay (grid_launcher/tv/bridge/pause_backend.py:8).

| Member | Kind | Behavior |
| --- | --- | --- |
| `visible` | property (:21) | Whether the overlay should be up |
| `gameTitle` | property (:25) | Snapshot of the running game's title |
| `emulatorName` | property (:29) | Snapshot of the emulator name, or the literal `Native Game` when empty |
| `actions` | property (:33) | Constant two-item list: `Resume Game`, `Quit to TV Mode` |
| `openForActiveSession()` | slot (:37) | No-op without an active session; otherwise suspends the emulator, snapshots title/emulator, sets visible |
| `resumeGame()` | slot (:53) | Only when visible: clears visible, then resumes the emulator |
| `quitGame()` | slot (:60) | Stops the game, clears visible and the snapshots |
| `dismiss()` | slot (:72) | Alias of `resumeGame` |
| `forceClose()` | slot (:76) | Clears visible and snapshots *without* resuming — used during teardown |

Events: `visibleChanged`, `gameTitleChanged`, `emulatorNameChanged`.

### Backend API surface — `CloudBackend`

Thin command surface over the cloud save logic in `docs/porting/06-cloud-saves.md`
(grid_launcher/tv/bridge/cloud_backend.py:106).

| Member | Input bundle | Emits |
| --- | --- | --- |
| `syncConfig(config)` (:122) | config | — |
| `loadSlotsForGame` (:126) | `{game, save_type}` | `slotsLoaded({save_type, slots})` or `slotsError({save_type, error})` |
| `deleteSlot` (:163) | `{save_id, save_type}` | `deleteComplete({success, message})` |
| `restoreSlot` (:196) | `{game, save_id, save_type}` | `restoreComplete({success, message})` |
| `uploadSave` (:248) | `{game, save_type}` | `uploadComplete({success, message})` |

### Signal payload convention

Every payload-bearing event carries exactly one object — a dict bundle — rather than
multiple positional arguments (for example
grid_launcher/tv/bridge/app_backend.py:68, grid_launcher/tv/bridge/game_backend.py:137,
grid_launcher/tv/widgets/components/game_row.py:13). A port should keep the
single-payload rule so that event plumbing stays uniform.

---

## Behavior

### Entry into TV mode

Three triggers exist:

1. The desktop top-bar **TV Mode** button (grid-launcher.py:607) calls
   `_switch_to_tv_mode` (grid-launcher.py:638).
2. The command-line flag `-tv` calls the same function immediately after first-run setup
   (grid-launcher.py:3787).
3. Nothing else. There is no automatic re-entry (see *Open questions*).

`_switch_to_tv_mode` (grid-launcher.py:638):

1. If a desktop install is in progress, ask for confirmation; anything other than "Yes"
   aborts the switch (grid-launcher.py:639).
2. Persist the current config (grid-launcher.py:650).
3. Stop the desktop session poll timer (grid-launcher.py:651).
4. Build or reuse the TV stack (grid-launcher.py:652).

First-time construction (grid-launcher.py:655) creates, in order: `AppBackend` (config +
image cache dir), `CloudBackend`, `GameBackend` (config + a reference to the desktop
window, used for uninstall and PS3 path helpers), `PauseBackend` (wrapping the game
backend), `ControllerBackend` (wrapping app + game backends), a `CoverLoader` seeded with
a `cover_url → cached_cover_path` map built from installed games (grid-launcher.py:714),
and the `TVWindow` shell (grid-launcher.py:726).

Cross-wiring established once (grid-launcher.py:685):

| Source event | Effect |
| --- | --- |
| `game_backend.pauseRequested` | `pause_backend.openForActiveSession()` (grid-launcher.py:685) |
| `game_backend.installComplete` (success) | `app_backend.syncConfig(config)` and refresh of the library/favorites/new/highly-rated row events (grid-launcher.py:686, :690) |
| `game_backend.uninstallComplete` (success) | same row refresh (grid-launcher.py:699) |
| `app_backend.switchToDesktopModeRequested` | `_switch_to_desktop_mode` (grid-launcher.py:710) |
| `app_backend.quitRequested` | application quit (grid-launcher.py:711) |
| `app_backend.saveConfigRequested` | persist config (grid-launcher.py:712) |

The controller is then told which windows count as "focused" for input gating — the shell
window and the pause window (grid-launcher.py:735).

Every entry (first or subsequent) does: `syncConfig` on the app, cloud, and game backends
(grid-launcher.py:739), `connectToServer()` (grid-launcher.py:742), resize the shell to
the primary screen and show it fullscreen (grid-launcher.py:743), hide the desktop window
(grid-launcher.py:747), and start controller polling (grid-launcher.py:748).

### Exit from TV mode

- **Return to Desktop Mode**: Settings row activation calls `requestDesktopMode`
  (grid_launcher/tv/widgets/views/settings_view.py:558), which reaches
  `_switch_to_desktop_mode` (grid-launcher.py:750). That handler stops any active game
  session, force-closes the pause backend (without resuming the emulator), hides the pause
  and shell windows, stops controller polling, **reloads config from disk**, refreshes the
  desktop library state and checkbox, shows and activates the desktop window, and restarts
  the session poll timer (grid-launcher.py:751–771).
- **Exit**: Settings row activation calls `requestQuit`
  (grid_launcher/tv/widgets/views/settings_view.py:562), which quits the application
  (grid-launcher.py:711).
- **Application close while in TV mode**: the desktop close handler hides the shell and
  pause windows, stops an active game session, and stops the controller
  (grid-launcher.py:622).

There is no controller-only exit: `guide_button` opens Settings, and the desktop/exit
actions live inside Settings.

### Raw input → navigation events

Both poll threads publish the same raw event shape: `{"code": <string>, "value": <float>}`
(grid_launcher/tv/bridge/controller.py:70).

**XInput producer** (grid_launcher/tv/bridge/controller.py:63): for each of four slots,
digital buttons are edge-detected against the previous mask and emitted with value `1.0`
on press and `0.0` on release (grid_launcher/tv/bridge/controller.py:152). Analog triggers
are thresholded at raw `> 30` of 0–255 and reported as the pseudo-buttons `BTN_TL2` /
`BTN_TR2` on transition (grid_launcher/tv/bridge/controller.py:163). Sticks are normalized
by 32767 with the Y axes inverted, and are emitted either when the value moved by more
than `0.02`, or when held past the dead zone and at least `0.2 s` has elapsed since the
last emit for that axis (grid_launcher/tv/bridge/controller.py:173).

**SDL producer** (grid_launcher/tv/bridge/controller.py:227): button down/up map through a
fixed index table (`0..10` → `BTN_SOUTH`, `BTN_EAST`, `BTN_WEST`, `BTN_NORTH`, `BTN_TL`,
`BTN_TR`, `BTN_SELECT`, `BTN_START`, `BTN_THUMBL`, `BTN_THUMBR`, `BTN_MODE`)
(grid_launcher/tv/bridge/controller.py:203); axis motion maps `0..5` → `ABS_X`, `ABS_Y`,
`ABS_Z`, `ABS_RX`, `ABS_RY`, `ABS_RZ` (grid_launcher/tv/bridge/controller.py:217); hat
motion is decomposed into `ABS_HAT0X` and `ABS_HAT0Y`, with the Y component negated so
that "up" is negative like the sticks (grid_launcher/tv/bridge/controller.py:288).

**Mapping to navigation events**
(grid_launcher/tv/bridge/controller.py:316, :329):

| Raw code | Source | Navigation event | Notes |
| --- | --- | --- | --- |
| `BTN_DPAD_UP` | XInput D-pad | `up` | press only |
| `BTN_DPAD_DOWN` | XInput D-pad | `down` | press only |
| `BTN_DPAD_LEFT` | XInput D-pad | `left` | press only |
| `BTN_DPAD_RIGHT` | XInput D-pad | `right` | press only |
| `BTN_SOUTH` | A / Cross | `confirm` | press only |
| `BTN_EAST` | B / Circle | `back` | press only |
| `BTN_TL` | LB | `tab_prev` | press only |
| `BTN_TR` | RB | `tab_next` | press only |
| `BTN_MODE` | Guide / Home | `guide_button` | press only; see suppression rules |
| `ABS_X` | left stick X | `left` (< −0.3) / `right` (> +0.3) | repeat-gated |
| `ABS_Y` | left stick Y | `up` (< −0.3) / `down` (> +0.3) | repeat-gated |
| `ABS_HAT0X` | SDL hat X | `left` / `right` | repeat-gated |
| `ABS_HAT0Y` | SDL hat Y | `up` / `down` | repeat-gated |
| `BTN_WEST`, `BTN_NORTH`, `BTN_START`, `BTN_SELECT`, `BTN_THUMBL`, `BTN_THUMBR`, `BTN_TL2`, `BTN_TR2`, `ABS_Z`, `ABS_RX`, `ABS_RY`, `ABS_RZ` | — | none | classified "unknown" and dropped (grid_launcher/tv/bridge/controller.py:480) |

Timing constants: dead zone `0.3`, repeat interval `0.2 s`
(grid_launcher/tv/bridge/controller.py:13).

**Axis debounce/repeat algorithm** (grid_launcher/tv/bridge/controller.py:505):

1. Resolve the direction from the sign of the value; if `|value| ≤ 0.3`, drop the stored
   repeat state for that axis and emit nothing (this is the "axis returned to center"
   reset).
2. Otherwise, emit if the direction differs from the last emitted direction for that axis,
   **or** if at least `0.2 s` passed since the last emit for that axis. Record direction
   and timestamp.

Net effect: a flick fires once immediately; a held stick auto-repeats at 5 Hz; reversing
direction fires immediately without waiting for the interval. Digital buttons have no
auto-repeat at this layer — a held D-pad on XInput produces exactly one event because only
the press edge is emitted.

**Input gating** (grid_launcher/tv/bridge/controller.py:452), in order:

1. If the raw code is `BTN_MODE`, the value is a press, and a game session is active →
   handle it immediately, bypassing focus checks (the game owns the screen, so the shell
   never has OS focus).
2. Else if a game session is active: drop the event unless the pause overlay is visible.
3. Else if no registered TV window has OS focus, drop the event. With no windows
   registered the check passes unconditionally
   (grid_launcher/tv/bridge/controller.py:427).
4. Classify: buttons act on value `1.0` only; axes go through the repeat algorithm.

**Guide-button handling** (grid_launcher/tv/bridge/controller.py:487):

1. If `should_suppress_guide_button()` returns true, drop the event entirely. Suppression
   is true when the active emulator's name (from `activeEmulatorName`, falling back to the
   session record's `emulator_name`) case-insensitively matches any entry in
   `tvGuideExclusionList` (grid_launcher/tv/bridge/controller.py:406). The intent is that
   such emulators consume the Guide button natively.
2. Else, if a game session is active, call `requestPause()` on the game backend and stop —
   the pause overlay opens rather than a navigation event being routed.
3. Else emit `guide_button` normally.
4. Routing target: while the pause backend reports visible, *all* navigation events go out
   on the pause channel; otherwise on the main channel
   (grid_launcher/tv/bridge/controller.py:500, :525). The shell subscribes to both channels
   with the same handler (grid_launcher/tv/widgets/window.py:106).

**Hotplug**: SDL device-added events construct and initialize a joystick on the fly;
device-removed events drop it (grid_launcher/tv/bridge/controller.py:270). XInput needs no
hotplug handling because it re-queries all four slots each pass and simply skips
`ERROR_NOT_CONNECTED` slots, clearing their remembered button mask
(grid_launcher/tv/bridge/controller.py:141).

### Navigation routing algorithm (shell)

`_on_nav_event` is the single authoritative dispatcher
(grid_launcher/tv/widgets/window.py:134):

1. **Pause first.** If the pause window is visible, forward to it and return
   (grid_launcher/tv/widgets/window.py:135).
2. **`tab_prev` / `tab_next`.** Move the tab index by one, clamped to `[0, 2]`; no wrap
   (grid_launcher/tv/widgets/window.py:139).
3. **`guide_button`.** Only when the outer stack is at the root *and* no game session is
   active: push a freshly constructed Settings view. Otherwise ignore
   (grid_launcher/tv/widgets/window.py:145).
4. **`back` with a pushed view.** Ask the pushed view whether it intercepts back
   (`intercepts_back()`); if it does, forward `back` to it instead of popping. Otherwise
   pop (grid_launcher/tv/widgets/window.py:149).
5. **Everything else** goes to `get_current_view().handle_nav(direction)`, where the
   current view is the pushed view when one exists, else the active tab's view
   (grid_launcher/tv/widgets/window.py:160, :367).

`back` while at the root falls through to step 5, so root views decide what it means (Home
and Library ignore it; Server uses it to leave the game wall).

**Tab change** (grid_launcher/tv/widgets/window.py:114): capture the outgoing screen,
switch the inner stack, then re-focus the newly shown view — Home resets to row 0
(`focus_default_row`), Library and Server call their `activate()` methods — capture the
incoming screen, run a 200 ms horizontal slide whose direction depends on whether the
index increased, and refresh the hint bar.

**Push** (grid_launcher/tv/widgets/window.py:165): add the view to the outer stack, make it
current, take keyboard focus, subscribe to the view's `controlHintsChanged` event if it has
one, animate a forward slide, refresh hints.

**Pop** (grid_launcher/tv/widgets/window.py:181): no-op at the root; otherwise capture the
current screen, set the outer stack **directly back to index 0** (not one level up), remove
and detach the popped view, animate a backward slide, refresh hints. Because pushes are
never nested in practice, pop is equivalent to "return to root".

**Scroll-into-view** is the responsibility of whichever component owns focus: rows, walls,
platform grids, settings lists, and the overlay lists each call an "ensure visible" on
their scroll container after moving the focus index (grid_launcher/tv/widgets/components/game_row.py:154, grid_launcher/tv/widgets/components/game_wall.py:166,
grid_launcher/tv/widgets/views/server_view.py:283, grid_launcher/tv/widgets/views/settings_view.py:518, grid_launcher/tv/widgets/views/details_view.py:473,
grid_launcher/tv/widgets/components/emulator_picker_overlay.py:188). Scroll containers deliberately do **not** consume
navigation keys — a scroll area subclass ignores arrows/Enter/Escape/Backspace/End/PageDown
in both its viewport filter and its own key handler so the events propagate to the shell
(grid_launcher/tv/widgets/components/nav_scroll_area.py:8, :33, :44). Any port using a
scrolling container with built-in key handling must do the same.

### Home view

Data (grid_launcher/tv/widgets/views/home_view.py:69): four horizontal rows in fixed order
— **Continue Playing**, **Favorites**, **New Additions**, **Highly Rated**.

| Row | Source | Transform |
| --- | --- | --- |
| Continue Playing | `app_backend.libraryGames` (:270) | reversed installed list, capped at 20 |
| Favorites | `app_backend.favoritesGames` (:280) | as-is |
| New Additions | `app_backend.newAdditionsGames` (:288) | as-is |
| Highly Rated | `app_backend.highlyRatedGames` (:296) | as-is |

Each row subscribes to its backend change event and refreshes through an 80 ms
single-shot debounce, so a burst of backend updates rebuilds the row once
(grid_launcher/tv/widgets/views/home_view.py:86, :106). After a refresh, the active row
re-applies focus (grid_launcher/tv/widgets/views/home_view.py:274).

Input (grid_launcher/tv/widgets/views/home_view.py:175):

- `up` / `down`: change the active row, clamped to `[0, 3]` with no wrap. If a row
  transition animation is running, the direction is stored as a single pending event
  instead (later events overwrite the pending one). Before animating, the horizontal screen
  position of the currently focused card is captured.
- `left` / `right` / `confirm`: forwarded to the active row.
- `back`: ignored.

Row transition (grid_launcher/tv/widgets/views/home_view.py:201): the outgoing row slides
off in the opposite direction of travel while the incoming row slides in, 300 ms, ease-out.
On completion, focus in the new row goes to the card whose screen x-center is nearest the
captured value, or to the first card if there was none
(grid_launcher/tv/widgets/views/home_view.py:245, grid_launcher/tv/widgets/components/game_row.py:131). This keeps the cursor
roughly in a vertical line while moving between rows. If a pending direction was queued,
the next transition runs at 0.9× duration, giving faster repeats when the user holds a
direction (grid_launcher/tv/widgets/views/home_view.py:256).

Layout facts that affect navigation: the active row sits at 70 % of view height, rows above
are parked above the viewport and rows below are parked below it
(grid_launcher/tv/widgets/views/home_view.py:139); up/down chevrons are shown only when a
row exists in that direction (grid_launcher/tv/widgets/views/home_view.py:166); a vertical
dot indicator marks the active row (grid_launcher/tv/widgets/views/home_view.py:355).

Media: the row's focused card drives the fanart background. Screenshot URLs come from
`screenshot_urls` or `url_screenshots`, accepted as either a list or a newline-separated
string (grid_launcher/tv/widgets/views/home_view.py:300). Fanart cycles every 5 s with a
1 s crossfade (grid_launcher/tv/widgets/components/fanart_background.py:53, :106) — see
`docs/porting/07-covers-images.md`.

Selection pushes a Details view for the selected game
(grid_launcher/tv/widgets/views/home_view.py:309).

### Library view

Data: `app_backend.libraryGames`, sorted case-insensitively by `name`/`title`
(grid_launcher/tv/widgets/views/library_view.py:456). With no games, an empty label is
shown and the carousel, letter bar, and toggle bar are all hidden
(grid_launcher/tv/widgets/views/library_view.py:465).

Three focus regions exist, arranged vertically: the **letter filter bar** (above), the
**carousel** (middle, default), and the **toggle bar** (below).

Input (grid_launcher/tv/widgets/views/library_view.py:340), evaluated in this order:

1. If the toggle bar has focus: `up` returns focus to the carousel; `left`/`right` move
   between the two toggles, clamped; `confirm` toggles the filter — pressing the active one
   clears it, pressing the other one activates it and resets the letter filter to "All".
2. If the letter bar has focus: `down` returns focus to the carousel; `left`/`right` move
   the letter cursor, clamped over 27 entries (`All` + `A`–`Z`); `confirm` applies the
   letter, or resets to `All` if that letter was already active, and always clears the
   toggle filter.
3. Otherwise (carousel focus): `up` moves focus to the letter bar; `down` moves focus to
   the toggle bar (ignored while an animation is running); `left`/`right` move the data
   index by one with clamping, schedule a fanart update, and start the slide animation;
   `confirm` emits the focused game.

Filter semantics (grid_launcher/tv/widgets/views/library_view.py:252):

| Active filter | Result set | Order |
| --- | --- | --- |
| toggle `favorites` | games with `is_favorite == "true"` | title, case-insensitive |
| toggle `recently_played` | games with a non-empty `last_played` | `last_played` descending |
| letter `All` | all games | source order (already alphabetical) |
| letter `X` | games whose `name`/`title` uppercases to a `X` prefix | source order |

The two filter kinds are mutually exclusive — activating either clears the other.

Carousel mechanics (grid_launcher/tv/widgets/views/library_view.py:283, :479): a fixed pool
of 11 cards is arranged in a ring; slot 5 is the visual center and always holds the current
data index; slots are bound from `current_index - 5 … current_index + 5`, with
out-of-range slots bound to an empty record. Moving right runs: shrink the center card back
to base size (120 ms), then slide every card one stride while the *new* center grows to
1.20× (260 ms slide, 190 ms grow), then recycle the card that fell off the leading edge to
the trailing edge and bind it to the newly needed data index. Left is the mirror image.
As in Home, nav during an animation stores one pending direction and the following
animation runs at 0.9× speed (grid_launcher/tv/widgets/views/library_view.py:582).

Fanart updates are debounced 500 ms so fast scrolling does not thrash image loads
(grid_launcher/tv/widgets/views/library_view.py:84, :321).

Cards show a favorite badge and a cloud-save badge derived from `is_favorite` and
`has_saves` string flags (grid_launcher/tv/widgets/components/library_card.py:30).

Selection pushes a Details view
(grid_launcher/tv/widgets/views/library_view.py:626).

### Server view

Two pages inside one view (grid_launcher/tv/widgets/views/server_view.py:114).

**Page 0 — platform grid.** Data comes from `app_backend.platformDetails`; each entry's
`url_logo` is resolved against the server URL and stored as `logo_url`
(grid_launcher/tv/widgets/views/server_view.py:172). Entries without a name are dropped.
Cards prefer a bundled `local_logo_path` over the remote `logo_url`
(grid_launcher/tv/widgets/components/platform_card.py:30). If there are no platform cards,
an explanatory label replaces the grid
(grid_launcher/tv/widgets/views/server_view.py:211). Input: `±1` horizontal, `±columns`
vertical, clamped; `confirm` opens the platform
(grid_launcher/tv/widgets/views/server_view.py:156). Column count is recomputed on resize
from the available width (grid_launcher/tv/widgets/views/server_view.py:305).

**Platform open** (grid_launcher/tv/widgets/views/server_view.py:217): switch to page 1,
immediately show any cached games for that platform, mark loading when the cache was empty,
then request a load. A spinner replaces the wall while loading
(grid_launcher/tv/widgets/views/server_view.py:285). When `serverGamesChanged` arrives for
the *currently selected* platform (label match required), the wall is refilled, loading
clears, and focus resets to the first card
(grid_launcher/tv/widgets/views/server_view.py:237).

**Page 1 — game wall.** `back` clears the selection, returns to page 0, and restores focus
to the previously focused platform card
(grid_launcher/tv/widgets/views/server_view.py:139). All other directions go to the wall,
which does clamped grid math and emits selection on `confirm`
(grid_launcher/tv/widgets/components/game_wall.py:125). Covers are loaded lazily: only
cards in the visible row band (computed from the scroll offset and a 480+18 px row height)
are populated, re-evaluated on every scroll, and the scroll subscription is dropped once all
cards have been populated (grid_launcher/tv/widgets/components/game_wall.py:89). Each
refill cancels the previous cover batch so stale callbacks cannot paint over new cards
(grid_launcher/tv/widgets/components/game_wall.py:66).

Selection pushes a Details view
(grid_launcher/tv/widgets/views/server_view.py:249).

### Details view

The most complex screen: three columns — actions (left), title/description/metadata
(center), screenshots (right) (grid_launcher/tv/widgets/views/details_view.py:296).

On construction it subscribes to install progress/completion, uninstall completion, launch
errors, session start/end, native-picker requests, ROM metadata start/ready, and favorite
toggle completion (grid_launcher/tv/widgets/views/details_view.py:404), then loads the
cover, logs a diagnostic label, and requests ROM metadata for the game
(grid_launcher/tv/widgets/views/details_view.py:421).

**Button model** (grid_launcher/tv/widgets/views/details_view.py:591), rebuilt on every UI
refresh from four inputs — installed, connected, native-PC platform, installing:

| Order | Button | Condition | Action |
| --- | --- | --- | --- |
| 1 | `Cancel` | install in progress | `cancelInstall()` |
| 1 | `Play` | installed, not installing | `launchGame(game)` |
| 1 | `Install` | not installed, not installing | `installGame(game)`; refuses with a banner if not connected (:714) |
| 2 | `Uninstall` | installed | `uninstallGame(game)` |
| 3 | `Change Executable` | installed **and** platform is `Windows` or `Windows 9x` (:928) | fetch candidates, show the native picker; banner if the ROM id is missing or there are no candidates (:728) |
| 4 | `Cloud Saves` | connected | open the cloud overlay for save type `save` |
| 5 | `Add to Favorites` / `Remove from Favorites` | connected; label from the game's `is_favorite` flag | `toggleFavorite(rom_id)` |

"Installed" means an entry with the same ROM id exists in `libraryGames` *and* carries a
non-empty `local_path` (grid_launcher/tv/widgets/views/details_view.py:913). The button
index is clamped whenever the list shrinks (:563).

**Metadata panel** (grid_launcher/tv/widgets/views/details_view.py:880): nine cells in a
3×3 grid — platform, released, by, version, size, rating, region, languages, genres.
Released prefers `first_release_date` then `release_year`; size is a binary
unit-scaled byte count; rating renders as up to five asterisks, accepting either a 0–5 or a
0–100 scale (`>5` is divided by 20) (:946, :950, :965). Missing values render as `-`.
Description falls back to `Loading metadata...` while a fetch is in flight, else
`No description available.` (:938).

**Input** (grid_launcher/tv/widgets/views/details_view.py:425), in priority order:

1. Lightbox visible: `back`/`confirm` close it; `left`/`up` go to the previous screenshot;
   `right`/`down` to the next; both clamp.
2. Cloud overlay visible: forward everything to it.
3. Native picker visible: forward everything to it.
4. `back`: pop the view.
5. `left`: if focus is in the screenshots column, move to the actions column and reset the
   screenshot index. `right`: the mirror. There is no focusable middle column — the center
   column is display-only, and the column index literally jumps 0 ↔ 2 (:451).
6. `up`/`down`: in the actions column, move the button index with clamping; in the
   screenshots column, move the screenshot index with clamping and scroll it into view.
7. `confirm`: in the actions column, run the focused action; in the screenshots column,
   open the lightbox.

Mouse wheel over the screenshot list is also accepted: it forces focus into the screenshot
column and synthesizes an `up`/`down` (grid_launcher/tv/widgets/views/details_view.py:494).

**Back interception**: `intercepts_back()` returns true exactly while the lightbox is
visible, so the shell forwards `back` instead of popping the whole screen
(grid_launcher/tv/widgets/views/details_view.py:526).

**Hints**: the view exposes a dynamic hint list — lightbox hints while the lightbox is up,
details hints otherwise — and emits `controlHintsChanged` when the lightbox opens or closes
so the shell refreshes the bar (grid_launcher/tv/widgets/views/details_view.py:50, :806).

**Live updates**:

| Event | Effect |
| --- | --- |
| `installProgress` | recompute fraction (`downloaded/total`, 0 when total is 0) and speed, refresh UI, which shows `Installing... N%  K KB/s` plus a progress bar (:996, :566) |
| `installComplete` | merge the returned game record on success, show a success/error banner, refresh (:1006) |
| `uninstallComplete` | on success drop `local_path`, `extracted_path`, `archive_path` from the local copy, banner, refresh (:1015) |
| `launchError` | error banner (:1025) |
| `sessionStarted` | success banner `Launched with <emulator>`, or `Game launched` when the name is empty (native) (:1028) |
| `sessionEnded` | success banner `Session ended` (:1033) |
| `nativeExecPickerNeeded` | open the picker with the current selection preselected; error banner if the candidate list is empty (:1037) |
| `romMetadataFetchStarted` | only for this ROM id: mark loading and refresh (:1044) |
| `romMetadataReady` | only for this ROM id: merge non-null metadata fields, clear loading, rebuild fanart and screenshots, refresh (:1050) |
| `favoriteToggleComplete` | only for this ROM id: set `is_favorite` from the payload and refresh (:1071) |

Banners auto-hide after 4 s (grid_launcher/tv/widgets/views/details_view.py:911).

**Font scaling**: all registered labels are re-styled with a scale factor of
`view_height / 1080`, floored at `720/1080` and capped at 2.5, recomputed on resize
(grid_launcher/tv/widgets/views/details_view.py:541). This exists so the screen stays
readable at 4 K TV distances; a port needs an equivalent resolution-relative type scale.

### Settings view

Pushed by `guide_button` at the root (grid_launcher/tv/widgets/window.py:147) and
constructed with only the app backend and a pop callback
(grid_launcher/tv/widgets/views/settings_view.py:283).

Row inventory, in build order (grid_launcher/tv/widgets/views/settings_view.py:372):

| Index group | Rows | Type |
| --- | --- | --- |
| Action group (one horizontal band) | `< Back`, `Return to Desktop Mode`, `Exit` | `back`, `desktop`, `exit` |
| General card | `Default Tab` (three pills: Home/Library/Server), `Auto Cloud Sync` (toggle) | `home_tab`, `auto_sync` |
| Exclusions card | `+ Add Exclusion`, then one row per excluded emulator with an `x Remove` affordance | `add_exclusion`, `remove_exclusion` |

Navigation (grid_launcher/tv/widgets/views/settings_view.py:331):

- Overlay first: while the emulator picker is visible it consumes everything.
- `up`: inside the action group, do nothing (it is the top band). Otherwise, if focus is on
  the first non-action row, jump to index 0 (the first action button); else move up one.
- `down`: from the action group, jump to the first non-action row; else move down one,
  clamped.
- `left`/`right`: inside the action group, cycle **with wraparound** across the three action
  buttons; on the `Default Tab` row, move the pill cursor with wraparound and preview it as
  active; elsewhere ignored (grid_launcher/tv/widgets/views/settings_view.py:520,
  :162).
- `confirm`: dispatch by row type — `back` pops; `home_tab` commits the pill under the
  cursor via `setHomeViewTab`; `auto_sync` inverts `isAutoSync` via `setAutoSync`;
  `desktop` calls `requestDesktopMode`; `exit` calls `requestQuit`; `add_exclusion` opens
  the emulator picker; `remove_exclusion` calls `removeExclusionEntry(name)`
  (grid_launcher/tv/widgets/views/settings_view.py:539).
- `back`: pop.

The emulator picker is opened only when `availableEmulatorNames` is non-empty
(grid_launcher/tv/widgets/views/settings_view.py:584); selecting a name calls
`addExclusionEntry` and closes.

The view re-renders reactively: `exclusionDataChanged` rebuilds the whole row list (which
preserves the focus index by clamping), `homeViewTabChanged` re-syncs the pill cursor and
active pill, `autoSyncChanged` re-syncs the toggle
(grid_launcher/tv/widgets/views/settings_view.py:327, :596).

### Overlay stack semantics

There is no general-purpose overlay stack object. Precedence is expressed by ordered
`if` checks, and it is fixed:

1. Pause window (shell level) — grid_launcher/tv/widgets/window.py:135.
2. The current view's own overlays, in the order that view checks them:
   Details → lightbox, then cloud saves, then native picker
   (grid_launcher/tv/widgets/views/details_view.py:426); Settings → emulator picker
   (grid_launcher/tv/widgets/views/settings_view.py:333).
3. The view itself.

Rules a port must preserve:

- Show = make visible, raise above siblings, and (for the three modal overlays) call
  `setUiOverlayActive(true)`; hide = the inverse
  (grid_launcher/tv/widgets/components/cloud_saves_overlay.py:98,
  grid_launcher/tv/widgets/components/native_exec_picker.py:79, grid_launcher/tv/widgets/components/emulator_picker_overlay.py:75).
- `back` inside an overlay closes only that overlay; it must never reach the view's own
  `back` handling (each overlay handles `back` itself:
  grid_launcher/tv/widgets/components/cloud_saves_overlay.py:166, grid_launcher/tv/widgets/components/native_exec_picker.py:115,
  grid_launcher/tv/widgets/components/emulator_picker_overlay.py:112).
- The lightbox is the one overlay that does not set `uiOverlayActive`; instead the view
  advertises `intercepts_back()` so the shell will not pop it
  (grid_launcher/tv/widgets/views/details_view.py:526).
- Every overlay dims the screen behind it by filling with black at ~80 % opacity
  (grid_launcher/tv/widgets/components/cloud_saves_overlay.py:178, grid_launcher/tv/widgets/components/native_exec_picker.py:127,
  grid_launcher/tv/widgets/components/emulator_picker_overlay.py:127); the pause window uses ~78 %
  (grid_launcher/tv/widgets/pause_window.py:89).

**Cloud saves overlay input model** (grid_launcher/tv/widgets/components/cloud_saves_overlay.py:107):
row 0 is always "Upload New Save"; rows `1..n` are cloud slots. `up`/`down` move the row
index over `[0, n]` and reset the action mode. On a slot row, `right` advances an action
mode `0 → 1 (Restore) → 2 (Delete)` and `left` walks it back. `confirm` on row 0 starts an
upload (guarded against re-entry); `confirm` on a slot with action mode 0 arms mode 1;
with mode 1 restores; with mode 2 deletes. Restore/delete/upload completion shows a
4 s status chip and, on success, reloads the slot list
(grid_launcher/tv/widgets/components/cloud_saves_overlay.py:342). Slot payloads are
filtered by matching `save_type` so a stale response for another type is ignored (:323).

**Native picker input model** (grid_launcher/tv/widgets/components/native_exec_picker.py:87):
`up`/`down` over `[0, n]` where index `n` is the trailing "Close" row; `confirm` on `n`
closes, `confirm` on a candidate persists the executable and marks it selected (the dialog
stays open); `back` closes. When opened with a current path, that candidate is preselected
and the cursor starts on it (:70).

**Emulator picker input model** (grid_launcher/tv/widgets/components/emulator_picker_overlay.py:83):
identical shape — `[0, n]` with a trailing "Close"; `confirm` on a name invokes the
callback and closes; `back` closes; cursor moves scroll the row into view.

### Pause flow

Trigger path while a game is running:

1. Guide press reaches the controller even though the shell is unfocused
   (grid_launcher/tv/bridge/controller.py:460).
2. Unless the active emulator is on the exclusion list, the controller calls
   `requestPause()` (grid_launcher/tv/bridge/controller.py:491).
3. `requestPause()` emits `pauseRequested` only if a session is active
   (grid_launcher/tv/bridge/game_backend.py:481).
4. The desktop wiring routes that to `openForActiveSession()` (grid-launcher.py:685).
5. `openForActiveSession()` re-checks the session, **suspends the emulator process**,
   snapshots the game title and emulator name (substituting `Native Game` for an empty
   emulator name), and sets visible
   (grid_launcher/tv/bridge/pause_backend.py:37).
6. The pause window shows only if the backend says visible *and* a session is genuinely
   active — a stale visible flag cannot surface the overlay during normal browsing
   (grid_launcher/tv/widgets/pause_window.py:55). On show it resets its index to 0,
   activates itself, and raises.

The overlay is a frameless, always-on-top, translucent top-level window sized to the
screen, so it can appear over the running game rather than inside the shell
(grid_launcher/tv/widgets/pause_window.py:33, grid_launcher/tv/widgets/window.py:388). It renders a centered
400×280 panel with the game title, the emulator name, and the two actions from
`PauseBackend.actions` (grid_launcher/tv/widgets/pause_window.py:91, :121).

Input (grid_launcher/tv/widgets/pause_window.py:144): `up`/`down` clamp the index to
`[0, 1]`; `confirm` runs the focused action; `back` resumes. Action 0 (**Resume Game**) is
deferred by 150 ms before calling `resumeGame()` so the overlay can disappear before the
game unfreezes (grid_launcher/tv/widgets/pause_window.py:161). Action 1
(**Quit to TV Mode**) calls `quitGame()`, which terminates the process, clears visibility,
and clears the snapshots (grid_launcher/tv/bridge/pause_backend.py:60).

While the pause overlay is up, the controller sends navigation on the pause channel and the
shell routes everything to the overlay, so `tab_prev`/`tab_next`/`guide_button` are inert
(grid_launcher/tv/bridge/controller.py:500, grid_launcher/tv/widgets/window.py:135). The hint bar switches to the
pause hint set, driven by the backend's `visibleChanged` event
(grid_launcher/tv/widgets/window.py:110, :374).

Session end while paused is not special-cased in the pause backend: when the process exits,
the game backend clears session state and emits `sessionEnded`
(grid_launcher/tv/bridge/game_backend.py:808), and the next visibility change re-evaluates
the "session actually active" guard.

---

## Invariants and error handling

- **Exactly one focus owner.** Every navigation event is delivered to a single handler; the
  router returns immediately after dispatching (grid_launcher/tv/widgets/window.py:134).
- **All index movement clamps; nothing wraps** — except the Settings action group and the
  Default Tab pills, which wrap deliberately
  (grid_launcher/tv/widgets/views/settings_view.py:530, :164).
- **Pointer input is not a navigation path.** The card components expose a `selected` event
  and views subscribe to it, but no card emits it: the only mouse handler in the TV stack is
  the tab strip's click-to-select-tab (grid_launcher/tv/widgets/tab_bar.py:53). A port may
  leave pointer support out entirely.
- **Async image callbacks must tolerate a destroyed target.** Every card checks that it is
  still attached to a parent before repainting, and the wall/carousel additionally cancel
  cover batches on refill
  (grid_launcher/tv/widgets/components/game_card.py:48, grid_launcher/tv/widgets/components/home_card.py:45,
  grid_launcher/tv/widgets/components/library_card.py:36, grid_launcher/tv/widgets/components/platform_card.py:36,
  grid_launcher/tv/widgets/components/fanart_background.py:87, grid_launcher/tv/widgets/components/game_wall.py:66).
- **Backend responses are filtered by identity before use.** Details view ignores metadata
  and favorite results for other ROM ids (grid_launcher/tv/widgets/views/details_view.py:1045, :1074); Server view ignores
  game lists for a platform other than the selected one (grid_launcher/tv/widgets/views/server_view.py:239); the cloud
  overlay ignores slot payloads for a different save type
  (grid_launcher/tv/widgets/components/cloud_saves_overlay.py:323).
- **Animation-gated views drop or defer input rather than corrupting state.** Home and
  Library keep at most one pending direction and discard the rest
  (grid_launcher/tv/widgets/views/home_view.py:177, grid_launcher/tv/widgets/views/library_view.py:404).
- **Launch and install are single-flight.** A second `launchGame` while a session is active
  raises `launchError` (grid_launcher/tv/bridge/game_backend.py:212); a second `installGame` while one is active
  raises `launchError` or emits a failed `installComplete`
  (grid_launcher/tv/bridge/game_backend.py:503, :507).
- **Missing prerequisites produce user-visible messages, not silent failures**: missing ROM
  id, server URL, or library path all emit `launchError`
  (grid_launcher/tv/bridge/game_backend.py:492, :500, :514); "Install" without a connection shows a banner
  (grid_launcher/tv/widgets/views/details_view.py:714).
- **Config writes are best-effort.** Both `AppBackend` and `GameBackend` swallow
  `OSError` when writing the config file (grid_launcher/tv/bridge/app_backend.py:534, grid_launcher/tv/bridge/game_backend.py:775, :804).
- **Favorite toggle failures are silent by design** — the completion event is simply not
  emitted, so the button label never flips
  (grid_launcher/tv/bridge/app_backend.py:773).
- **Missing controller stack degrades to keyboard.** No `ctypes` (Windows) or no `pygame`
  (elsewhere) logs to stderr and returns; the UI still works with the key mappings
  (grid_launcher/tv/bridge/controller.py:84, :244).
- **The SDL poll loop swallows all exceptions per iteration** so a transient device error
  cannot kill input (grid_launcher/tv/bridge/controller.py:292).
- **Teardown ordering matters**: leaving TV mode force-closes the pause backend *without*
  resuming, because the process is being terminated anyway
  (grid-launcher.py:753, grid_launcher/tv/bridge/pause_backend.py:76).
- **Config is re-read from disk when returning to desktop**, so settings changed in TV mode
  are not lost or double-applied (grid-launcher.py:764).

---

## Platform differences (XInput vs SDL)

| Aspect | Windows / XInput | Other platforms / SDL |
| --- | --- | --- |
| Source | `XInput1_4.dll` → `XInput9_1_0.dll` → `XInput1_3.dll` (grid_launcher/tv/bridge/controller.py:90) | `pygame.joystick` (grid_launcher/tv/bridge/controller.py:242) |
| Model | polling: full state read for slots 0–3 each pass (grid_launcher/tv/bridge/controller.py:138) | event queue drained each pass (grid_launcher/tv/bridge/controller.py:269) |
| Poll interval | 16 ms normally, 33 ms while an install is active (grid_launcher/tv/bridge/controller.py:195) | 8 ms normally, 33 ms while an install is active (grid_launcher/tv/bridge/controller.py:294) |
| Guide button | available only through ordinal 100 (`XInputGetStateEx`); absent if the fallback entry point is used (grid_launcher/tv/bridge/controller.py:119) | reported as button index 10 (grid_launcher/tv/bridge/controller.py:214) |
| Digital edges | computed by diffing the previous button mask per slot (grid_launcher/tv/bridge/controller.py:152) | delivered as discrete down/up events (grid_launcher/tv/bridge/controller.py:276) |
| Triggers | thresholded at raw 30 and surfaced as `BTN_TL2`/`BTN_TR2` (grid_launcher/tv/bridge/controller.py:163) — currently unmapped to navigation | delivered as axes `ABS_Z`/`ABS_RZ` — currently unmapped |
| D-pad | dedicated button bits (grid_launcher/tv/bridge/controller.py:40) | hat events split into `ABS_HAT0X`/`ABS_HAT0Y` (grid_launcher/tv/bridge/controller.py:288) |
| Stick normalization | divide by 32767; Y axes negated (grid_launcher/tv/bridge/controller.py:178) | SDL's native −1…1; Y already negative-up |
| Stick repeat | producer-side: re-emits a held axis every 200 ms (grid_launcher/tv/bridge/controller.py:188) *and* consumer-side repeat gate | consumer-side repeat gate only |
| Hotplug | implicit: disconnected slots return `ERROR_NOT_CONNECTED` and are skipped (grid_launcher/tv/bridge/controller.py:141) | explicit device-added/removed events (grid_launcher/tv/bridge/controller.py:270) |
| Background input | inherent to polling | requires `SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1` (grid_launcher/tv/bridge/controller.py:250) |
| Process group | launches use `CREATE_NEW_PROCESS_GROUP` (grid_launcher/tv/bridge/game_backend.py:398) | no creation flags |
| Native launch | no compat tool | default compat tool applied when configured (grid_launcher/tv/bridge/game_backend.py:376) |

Both threads accept an "install active" predicate and back off to a 33 ms interval while a
download runs, to leave CPU for the transfer (grid_launcher/tv/bridge/controller.py:365).

---

## Concurrency

See `docs/porting/08-background-threading.md` for the full worker inventory and lifetime
rules. TV-mode-specific points:

- **Two long-lived poll threads at most.** `ControllerBackend.start()` creates exactly one
  poll thread and returns early if one is already running
  (grid_launcher/tv/bridge/controller.py:360). `stop()` sets the running flag false, asks
  the thread to quit, waits up to 500 ms, and drops the reference
  (grid_launcher/tv/bridge/controller.py:383).
- **Raw events cross threads as queued deliveries**, so the mapping/repeat logic and all UI
  work run on the UI thread (grid_launcher/tv/bridge/controller.py:370). A port must
  marshal raw device events onto the UI thread before the repeat state machine touches
  them, because the repeat state is unsynchronized
  (grid_launcher/tv/bridge/controller.py:348).
- **Process watching** uses a dedicated thread that blocks on process exit and then emits
  one event, delivered queued (grid_launcher/tv/bridge/game_backend.py:77, :418).
- **Install** is two-phase: an event-loop thread for the download and a plain thread for
  finalization, whose result is delivered back through a queued event
  (grid_launcher/tv/bridge/game_backend.py:665, :164).
- **Cloud restore-before-launch** runs on its own thread; the actual process spawn happens
  in its completion handler, so launch is asynchronous when auto-sync is enabled
  (grid_launcher/tv/bridge/game_backend.py:322, :432).
- **Catalog, favorites, new additions, highly rated, saves index, ROM metadata, and
  favorite toggling** each run on plain daemon threads with a per-kind "already running"
  guard; per-platform ROM fetches are keyed by platform label
  (grid_launcher/tv/bridge/app_backend.py:570, :585, :614, :626, :638, :373, :448).
- **Cover loading** is asynchronous with results marshalled back to the UI thread and with
  batch cancellation — see `docs/porting/07-covers-images.md`.
- **UI-side debounces** exist to coalesce bursts: 80 ms per Home row
  (grid_launcher/tv/widgets/views/home_view.py:88), 500 ms for Library fanart
  (grid_launcher/tv/widgets/views/library_view.py:86), 300 ms for library-changed
  re-emission after metadata writeback
  (grid_launcher/tv/bridge/app_backend.py:846).

---

## Test oracle

Twelve unit-test files exercise TV mode. They are the behavioral contract a port should
mirror.

| File | Covers |
| --- | --- |
| `tests/test_tv_controller_backend.py` | Guide suppression (including case-insensitivity, no-game, no-backend, empty-list cases); button press emits / release does not; each button→event mapping; unknown codes dropped; pause-channel routing for buttons and axes; dead zone; per-axis direction mapping; repeat suppression within the interval; immediate fire on direction change; center resets repeat state; event classification; graceful exit when `pygame` is missing; install-active predicate defaults |
| `tests/test_tv_app_backend.py` | Exclusion list normalization and persistence; home-view tab validation; `libraryGames` filtering (including exclusion of the `Emulators` platform); `syncConfig` catalog invalidation on URL change; `connectToServer` guards; auto-sync setter; overlay-active state and change-only emission |
| `tests/test_tv_game_backend.py` | Session properties and title fallbacks; launch success/error paths; stop semantics; pause/resume through `psutil`; `requestPause` gating; process-watch thread state clearing (including native games with an empty emulator name); auto restore-before-launch and upload-on-exit paths; `last_played` stamping; install-active flag |
| `tests/test_tv_pause_backend.py` | Initial state; no-op without a session; suspend call; visible transitions; title/emulator snapshots and the `Native Game` fallback; resume/quit/dismiss behavior |
| `tests/test_tv_pause_window.py` | Index clamping at both ends; confirm dispatch per index; `back` resumes; unknown directions are inert |
| `tests/test_tv_home_view.py` | Initial active row; row parking geometry above/below; re-place on resize; animation gating and pending-nav queueing/dispatch/clearing; boundary no-ops; blocked-flag clearing; chevron visibility and placement |
| `tests/test_tv_library_view.py` | Carousel index movement and boundaries; animation gating and pending nav; empty-state label; filter-bar and toggle-bar entry/exit; alphabetical sort; letter filtering and reset; toggle activation, deactivation, and mutual exclusion; recently-played descending sort; favorites alphabetical sort; strip anchor position; grow animation; fanart geometry |
| `tests/test_tv_details_view.py` | Screenshot card height derived from pixmap aspect ratio; font-scale ramp, floor, and cap; button rebuild honoring the current font scale |
| `tests/test_tv_cloud_backend.py` | Slot fetch success/empty/error; credential and ROM-id guards; delete and restore paths; upload worker outcomes including partial upload; thread cancellation on repeat upload |
| `tests/test_tv_image_provider.py` | Cover loader cache hits, empty/None URLs, HTTP fallback, stale cache entries, failure returning nothing |
| `tests/test_tv_server_platform_details.py` | Platform record normalization: empty/non-list payloads, zero-ROM exclusion, slug→metadata and slug→logo mapping (including PSX, GameCube, TurboGrafx, Genesis, 32X, Pico variants), name fallbacks, `url_logo` preservation, display-name preference, sort order |
| `tests/test_game_wall_batching.py` | Wall cards all placed immediately; first visible row populated immediately; state reset on refill; cover batch cancelled on refill |

---

## Open questions

1. **`homeViewTab` is persisted but never applied.** The Settings "Default Tab" row writes
   `tv_mode_home_view` (grid_launcher/tv/bridge/app_backend.py:565), but the tab strip
   always initializes to index 0 (grid_launcher/tv/widgets/tab_bar.py:15) and the shell
   never sets it from config (grid_launcher/tv/widgets/window.py:56), with no read of
   `homeViewTab` anywhere outside Settings. A port should decide whether the setting is meant to select
   the initial tab.
2. **`tv_mode_last_active` is dead config.** It is defaulted and surfaced in the desktop
   settings values (grid-launcher.py:2359, :2620) but never read to auto-enter TV mode.
3. **SDL hotplug keys are asymmetric.** Add uses `event.device_index` while remove uses
   `event.instance_id` (grid_launcher/tv/bridge/controller.py:270). These are different
   identifier spaces in SDL2, so a removed joystick may not be removed from the map. It has
   no visible effect today because events are read from the global queue rather than
   per-joystick, but a port should key both by instance id.
4. **`pop_view` always returns to the root** rather than popping one level
   (grid_launcher/tv/widgets/window.py:188). This is correct for the current one-deep usage;
   whether deeper stacks are intended is unresolved.
5. **Guide-button suppression only checks the emulator name**, not whether the emulator is
   actually in the foreground (grid_launcher/tv/bridge/controller.py:406). If the excluded
   emulator is running but the shell has focus, Guide does nothing at all.
6. **Trigger and face-button codes are produced but unmapped**: `BTN_TL2`, `BTN_TR2`,
   `BTN_WEST`, `BTN_NORTH`, `BTN_START`, `BTN_SELECT`, `BTN_THUMBL`, `BTN_THUMBR`,
   `ABS_Z`, `ABS_RX`, `ABS_RY`, `ABS_RZ`. Whether these are reserved for future bindings or
   should be dropped is unclear.
7. **Right stick does not navigate.** `ABS_RX`/`ABS_RY` are emitted by both producers but
   absent from the axis map (grid_launcher/tv/bridge/controller.py:329).
8. **Card `selected` events are wired but never emitted**, so pointer selection is dead
   code across `HomeCard`, `GameCard`, `LibraryCard`, and `PlatformCard`. A port should
   either implement pointer activation or drop the event.
9. **The skill file lists a `MODAL_OVERLAY` theme constant** that does not exist in
   `grid_launcher/tv/widgets/theme.py`; overlays hard-code their scrim color instead.
10. **Details view has no focusable center column.** `left`/`right` jump between column 0
    and column 2 (grid_launcher/tv/widgets/views/details_view.py:451), so long descriptions
    can only be scrolled by the mouse wheel path, not by controller.
11. **The pause window carries unconditional diagnostic output**: four stdout prints during
    construction (grid_launcher/tv/widgets/pause_window.py:32, :39, :50, :52) and
    info-level logging plus a captured stack frame on every show
    (grid_launcher/tv/widgets/pause_window.py:75). Presumably debug residue; a port need
    not reproduce it.
12. **TV installs intentionally skip supplemental archives and firmware**
    (grid_launcher/tv/bridge/game_backend.py:647), so a TV-installed game may be less
    complete than the same install performed on the desktop. Whether that gap is permanent
    is unresolved.

---

## Source map

| Path | Role |
| --- | --- |
| `grid-launcher.py:607` | Desktop **TV Mode** button |
| `grid-launcher.py:638` | `_switch_to_tv_mode` — guard, persist, stop timers |
| `grid-launcher.py:654` | `_switch_to_tv_mode_widget` — backend construction, wiring, show |
| `grid-launcher.py:750` | `_switch_to_desktop_mode` — teardown and desktop restore |
| `grid-launcher.py:622` | Close handler teardown while TV mode is active |
| `grid-launcher.py:3787` | `-tv` startup flag |
| `grid_launcher/tv/bridge/controller.py:63` | XInput poll thread |
| `grid_launcher/tv/bridge/controller.py:227` | SDL/pygame poll thread |
| `grid_launcher/tv/bridge/controller.py:302` | `ControllerBackend` — mapping, gating, repeat, guide handling |
| `grid_launcher/tv/bridge/app_backend.py:51` | Config/library/catalog/favorites backend |
| `grid_launcher/tv/bridge/game_backend.py:129` | Install/launch/session backend |
| `grid_launcher/tv/bridge/pause_backend.py:8` | Pause state backend |
| `grid_launcher/tv/bridge/cloud_backend.py:106` | Cloud save command surface |
| `grid_launcher/tv/bridge/workers.py` | Catalog/ROM/favorites/saves fetch workers (see doc 08) |
| `grid_launcher/tv/widgets/window.py:17` | Shell: stacks, tab routing, nav dispatch, transitions, key mapping |
| `grid_launcher/tv/widgets/tab_bar.py:8` | Three-tab strip with clamped index |
| `grid_launcher/tv/widgets/theme.py:3` | Color constants |
| `grid_launcher/tv/widgets/pause_window.py:14` | Pause overlay window and its nav |
| `grid_launcher/tv/widgets/cover_loader.py:45` | Image loading (see doc 07) |
| `grid_launcher/tv/widgets/views/home_view.py:31` | Four-row home screen |
| `grid_launcher/tv/widgets/views/library_view.py:43` | Installed-games carousel with letter and toggle filters |
| `grid_launcher/tv/widgets/views/server_view.py:30` | Platform grid + server game wall |
| `grid_launcher/tv/widgets/views/details_view.py:47` | Game details, action buttons, screenshots, lightbox |
| `grid_launcher/tv/widgets/views/settings_view.py:273` | TV settings rows and actions |
| `grid_launcher/tv/widgets/components/nav_scroll_area.py:24` | Scroll container that refuses to consume nav keys |
| `grid_launcher/tv/widgets/components/game_row.py:12` | Horizontal row focus model |
| `grid_launcher/tv/widgets/components/game_wall.py:14` | Grid focus model with lazy cover population |
| `grid_launcher/tv/widgets/components/home_card.py:10` | Wide home card |
| `grid_launcher/tv/widgets/components/game_card.py:10` | Wall card with focus scale animation |
| `grid_launcher/tv/widgets/components/library_card.py:11` | Carousel card with favorite/save badges |
| `grid_launcher/tv/widgets/components/platform_card.py:12` | Platform logo card |
| `grid_launcher/tv/widgets/components/fanart_background.py:37` | Cycling blurred backdrop |
| `grid_launcher/tv/widgets/components/cloud_saves_overlay.py:14` | Cloud save slot overlay |
| `grid_launcher/tv/widgets/components/native_exec_picker.py:14` | Native executable picker |
| `grid_launcher/tv/widgets/components/emulator_picker_overlay.py:15` | Guide-exclusion emulator picker |
| `grid_launcher/tv/widgets/components/controls_bar.py:36` | Bottom hint strip |
| `grid_launcher/tv/widgets/components/install_progress_bar.py:11` | Install progress bar |
| `grid_launcher/tv/widgets/components/scrollbar.py:11` | Slim TV scrollbar indicator |
