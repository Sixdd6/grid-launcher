# Detached TV Pause Menu

## Summary

The current TV pause menu is implemented as `PauseOverlay.qml`, a child `Item` inside the main TV `ApplicationWindow`. That makes the pause UI visually correct only while the TV shell itself is the active foreground window; once an emulator is launched, toggling `pauseOverlay.visible = true` only reveals the menu inside the background TV window.

This design replaces the in-window overlay with a dedicated fullscreen top-level pause window that is created alongside TV mode, remains hidden during play, and can be explicitly shown, raised, and activated when the Guide button is pressed. The detached window keeps the existing TV-mode visual language, preserves the current Python `requestPause()` entry point, and narrows the refactor to a small, testable ownership boundary.

## Problem Statement

### Current behavior

- `ControllerBackend._handle_button()` routes `BTN_MODE` to `gameBackend.requestPause()` for active sessions.
- `GameBackend.requestPause()` emits `pauseRequested`.
- `Main.qml` handles `onPauseRequested` by setting `pauseOverlay.visible = true`.
- `PauseOverlay.qml` is an `Item` anchored to the main TV `ApplicationWindow`.

### Why it fails

The pause menu is not its own window. It cannot independently request foreground focus or top-most behavior because it only exists inside the already-background TV root window.

## Goals

- Show the pause menu in the foreground while a game session is active.
- Keep the existing Guide-button control path centered on `ControllerBackend` and `GameBackend.requestPause()`.
- Preserve the Dracula TV-mode theming and controller-first navigation patterns already used by TV overlays.
- Allow the pause UI to evolve independently from `Main.qml` and the tab shell.
- Keep business logic in Python and presentation in QML.

## Non-Goals

- Reworking game launching, process watching, or auto cloud sync behavior.
- Moving the rest of TV mode away from the current single-engine architecture.
- Replacing the general `uiOverlayActive` concept across all TV overlays in this change.
- Adding desktop-widget pause UI; this is TV-mode only.

## Proposed Architecture

### High-level approach

Create a dedicated top-level QML `Window` for pause UI and manage it from Python as a sibling to the main TV window.

The current `pauseRequested` signal remains the single activation entry point. Instead of `Main.qml` toggling an embedded overlay, Python will route pause visibility through a small coordinator object that owns:

- the detached pause window instance
- pause/open/close state
- foreground activation attempts
- session metadata exposed to QML

### Ownership

| File | Change | Reason |
|------|--------|--------|
| `rom-mate.py` | Modify TV-mode bootstrap and teardown | Create and retain the detached pause window alongside the main TV window. |
| `rom_mate/tv/bridge/game_backend.py` | Small signal/state additions | Keep process pause/resume/stop ownership in Python and expose pause-session state cleanly. |
| `rom_mate/tv/bridge/app_backend.py` | Add detached-pause state property/signal or attach a new pause backend | Lets QML and navigation guards distinguish shell overlays from the detached pause window. |
| `rom_mate/tv/bridge/pause_backend.py` | New | Central coordinator for detached pause state, labels, actions, and window lifecycle hooks. |
| `rom_mate/tv/qml/Main.qml` | Remove embedded `PauseOverlay` usage | Main TV shell should no longer own or render the pause menu. |
| `rom_mate/tv/qml/components/PauseOverlay.qml` | Replace or retire | Existing `Item`-based overlay should be replaced with detached-window content. |
| `rom_mate/tv/qml/windows/PauseWindow.qml` | New | Fullscreen top-level pause window using the TV design language. |
| `tests/test_tv_controller_backend.py` | Update/extend | Confirm Guide button still routes only to `requestPause()` when session-active. |
| `tests/test_tv_game_backend.py` | Update/extend | Cover pause-state signals and no-op behavior when session inactive. |
| `tests/test_tv_app_backend.py` | Update/extend | Validate overlay-active semantics after detached pause state is introduced. |
| `tests/test_tv_pause_backend.py` | New | Unit tests for detached pause open/close/quit/resume orchestration. |

### New backend: `PauseBackend`

Add `rom_mate/tv/bridge/pause_backend.py` as a dedicated `QObject` exposed to QML as `pauseBackend`.

Responsibilities:

- hold `visible`, `gameTitle`, `emulatorName`, `statusText`, and focused action index
- expose `openForActiveSession()`, `resumeGame()`, and `quitGame()` slots
- coordinate with `GameBackend.pauseEmulator()`, `resumeEmulator()`, and `stopGame()`
- emit `visibilityChanged` and `pauseWindowActivationRequested`
- normalize close reasons so Guide/Back/Escape all route through one path

Non-responsibilities:

- launching games
- controller polling
- direct process management beyond calling `GameBackend`
- cloud sync

This keeps the pause state out of `Main.qml` and avoids turning `AppBackend` into a generic modal controller.

### Window creation strategy

The detached pause window should be created during `_switch_to_tv_mode()` alongside the current `Main.qml` root load.

Recommended implementation:

1. Keep the existing `QQmlApplicationEngine`.
2. Continue loading `Main.qml` as the main TV root window.
3. Create a `QQmlComponent` from `rom_mate/tv/qml/windows/PauseWindow.qml` using the same engine.
4. Instantiate the pause window against the engine root context so it receives `appBackend`, `gameBackend`, `controllerBackend`, and `pauseBackend` without duplicate wiring.
5. Retain the created object on `MainWindow` as `_tv_pause_window`.

Why this shape:

- It preserves one engine and one shared object graph.
- It avoids nesting a `Window` inside another `Window` in QML.
- It gives Python a direct handle for `showFullScreen()`, `raise()`, and `requestActivate()`.

## Foreground Window Contract

When the pause menu opens, the coordinator must perform these steps in order:

1. Confirm that `gameBackend.isSessionActive` is true.
2. Call `gameBackend.pauseEmulator()` before painting the menu.
3. Update `pauseBackend` presentation state from the active session.
4. Show the detached window fullscreen.
5. Call `raise()` and `requestActivate()` on the pause window.
6. Keep the window on top with `Qt.WindowStaysOnTopHint` while visible.

When the pause menu closes via Resume:

1. Hide the pause window.
2. Call `gameBackend.resumeEmulator()`.
3. Clear `pauseBackend.visible` and focus state.

When the pause menu closes via Quit:

1. Call `gameBackend.stopGame()`.
2. Hide the pause window.
3. Clear pause state.

### Windows focus caveat

`requestActivate()` is usually sufficient when the app is responding to direct user input, but Windows can refuse focus steals in some circumstances. The initial implementation should stay Qt-only. If real-world verification shows intermittent background activation failures, add a Windows-specific escalation helper behind a narrow abstraction in Python rather than leaking native logic into QML.

## UI And Theming Design

### Visual direction

The detached pause window should feel like a first-class TV-mode surface, not a system dialog.

Use the established TV palette:

- background scrim: `#CC000000`
- panel: `#1e1f29`
- panel border: `#44475a`
- primary text: `#f8f8f2`
- secondary text: `#6272a4`
- primary focus accent: `#ff79c6`
- secondary identity accent: `#bd93f9`
- destructive emphasis: `#ff5555`

### Window structure

`PauseWindow.qml` should use `Window` as the root type and render a centered panel over a fullscreen blocker.

Layout:

- fullscreen transparent-to-black blocker layer
- centered pause card, width `min(screenWidth * 0.42, 560)`
- card radius `14`
- top status glyph or cover fallback block
- game title
- emulator/session subtitle
- action list
- secondary footer hint row for button mappings

### Content model

Recommended actions for v1:

1. `Resume Game`
2. `Quit to TV Mode`

Deferred-safe slots for future extension, without implementing them now:

- `Cloud Saves`
- `Controller Settings`
- `Manual / Game Info`

The initial design should visually reserve room for more actions by using a vertical action list with a reusable delegate instead of two one-off rectangles.

### Action styling

Focused primary action:

- fill `#ff79c6`
- text `#282a36`
- subtle scale `1.02`
- border `#f8f8f2`, width `2`

Focused secondary/destructive action:

- fill `#383a59`
- border `#ff79c6` or `#ff5555` depending on semantic type
- text `#f8f8f2`

Unfocused action:

- fill `#383a59`
- border `#44475a`
- text `#f8f8f2`

### Motion

Use the same restrained motion language as the rest of TV mode:

- window opacity fade in: `140-180ms`
- panel scale from `0.985` to `1.0` on open
- focus changes: `100-120ms` scale/border animation

No large fly-ins or cinematic transitions; the pause surface must appear immediate and stable.

### Footer hints

Add a bottom hint row using secondary text color:

- `A Confirm`
- `B Back`
- `Guide Resume`

This matches the controller-first language already used throughout TV mode and makes the detached window self-explanatory when it becomes the foreground surface.

## Navigation Model

The detached pause window should consume controller navigation only while visible.

Rules:

- `up` / `down`: move selection through the action list
- `confirm`: execute selected action
- `back`: resume game
- `guide_button`: resume game
- keyboard `Escape`: resume game
- keyboard `Return` / `Enter`: confirm selected action

`Main.qml` should not handle pause-navigation events once the detached pause window is shown.

## Overlay State Semantics

The current `appBackend.uiOverlayActive` state blocks top-level navigation in TV views. Detached pause needs slightly different semantics because it is no longer a child overlay inside the main shell.

Recommended split:

- keep `uiOverlayActive` for in-shell overlays such as slot pickers
- add `pauseMenuVisible` on `PauseBackend`
- update `_navBlocked()` helpers in TV views to return true when either `appBackend.uiOverlayActive` or `pauseBackend.visible`

Why not overload `uiOverlayActive` alone:

- the detached window is not visually hosted by the main shell
- separating states keeps intent clear in tests and future overlay work
- it avoids coupling `AppBackend` to window management details

## Session Data Contract

`PauseBackend` should derive its display data from `GameBackend` without duplicating launch ownership.

Expose:

- `gameTitle`: best-effort game title; fallback to ROM/display name
- `emulatorName`: current emulator name or `Native Game`
- `isVisible`: whether detached pause window is open
- `actions`: static action list for QML delegate rendering

If there is no active session, `openForActiveSession()` is a no-op.

## Ordered Implementation Steps

1. In `rom_mate/tv/bridge/pause_backend.py`, create `PauseBackend` with `visible`, `gameTitle`, `emulatorName`, `actions`, and slots for `openForActiveSession()`, `resumeGame()`, `quitGame()`, and `dismiss()`.
2. In `rom_mate/tv/bridge/game_backend.py`, add any missing helper needed to expose the current session title cleanly without forcing QML to inspect private session dictionaries.
3. In `rom-mate.py`, instantiate `PauseBackend` during `_switch_to_tv_mode()`, register it on the root context as `pauseBackend`, create `PauseWindow.qml` via `QQmlComponent`, and retain the resulting object as `_tv_pause_window`.
4. In `rom-mate.py`, connect `gameBackend.pauseRequested` to a Python handler that asks `pauseBackend` to open and then shows/raises/activates `_tv_pause_window`.
5. In `rom-mate.py`, connect `gameBackend.sessionEnded` and desktop-mode teardown paths to force-hide the pause window and clear pause state.
6. In `rom_mate/tv/qml/windows/PauseWindow.qml`, implement the fullscreen detached window using the existing TV modal styling, shared palette, controller `Connections`, and an action delegate driven by `pauseBackend.actions`.
7. In `rom_mate/tv/qml/Main.qml`, remove the embedded `PauseOverlay` item and its `onPauseRequested` visibility toggles; leave Guide-button behavior routed through Python-only pause orchestration.
8. In TV views/components that use `_navBlocked()`, include `pauseBackend.visible` in the guard so the background shell never processes navigation while the detached window is open.
9. Add unit tests for `PauseBackend`, then update backend tests to cover session-end cleanup and the Guide-button contract.

## Testing Impact

### Test files to modify

- `tests/test_tv_controller_backend.py`
- `tests/test_tv_game_backend.py`
- `tests/test_tv_app_backend.py`
- `tests/test_tv_pause_backend.py` (new)

### Test cases to add

- Guide button still calls `gameBackend.requestPause()` and does not emit `navigationEvent` during an active session.
- `PauseBackend.openForActiveSession()` pauses the emulator, populates title/emulator labels, and sets `visible = True`.
- `PauseBackend.openForActiveSession()` is a no-op when no session is active.
- `PauseBackend.resumeGame()` hides the menu and resumes the emulator.
- `PauseBackend.quitGame()` hides the menu and stops the session.
- Session-ended signals force-hide the detached pause state even if the emulator exits externally.
- TV shell nav guards treat `pauseBackend.visible` as blocking.

### Patch namespaces

- Patch `GameBackend` interactions in the pause-backend module namespace where used.
- Keep the controller tests patching `rom_mate.tv.bridge.controller.ControllerBackend` collaborators, not the original symbol definitions.

## Risks And Edge Cases

- Native focus behavior on Windows may still require a platform-specific fallback if `requestActivate()` is insufficient.
- The existing `uiOverlayActive` flag is used broadly in TV views; mixing detached pause state into the same boolean without clear separation can create regressions in shell navigation.
- Session end can happen asynchronously while the pause menu is visible. Cleanup must be idempotent.
- Native launches use an empty emulator name in some paths, so the pause subtitle needs a clean fallback label.
- If `pauseEmulator()` fails because `psutil` is unavailable or the process already exited, the window should not remain stuck in a fake paused state.

## Acceptance Criteria

- Pressing Guide during an active supported game session opens a fullscreen pause menu as a foreground top-level window.
- The pause menu uses the same TV-mode palette, typography scale, spacing, and controller conventions as the rest of the app.
- Background TV views stop reacting to navigation while the detached pause window is visible.
- Resume and Quit actions work for both controller and keyboard input.
- Session exit always closes the pause window and clears its state.

## Recommended Sequence

Implement this in two passes:

1. Extract ownership and window lifecycle first: `PauseBackend`, detached `PauseWindow.qml`, and Python wiring.
2. Refine visual polish second: action delegate reuse, footer hints, and future-action placeholders.

That sequence minimizes regressions by proving the foreground window contract before spending time on richer UI surface details.