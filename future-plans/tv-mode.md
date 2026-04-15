# TV Mode

Fullscreen experience that works like EmulationStation, ES-DE, Steam Big Picture, or BigBox. Activated from within the desktop app via a nav button; switches back to desktop mode from within TV Mode or its Settings view.

## Software Stack

**Framework: PySide6 QML / Qt Quick** — visually and architecturally distinct from Qt Widgets (GPU-accelerated, OpenGL/Vulkan rendering, shader effects). No new Qt dependency since PySide6 is already required. QML's `ApplicationWindow`, `StackView`, `PathView`, and `ListView` provide Netflix-style carousels, push/pop navigation, and smooth property animations natively.

**New Python dependencies:**
- `inputs` — cross-platform gamepad and Guide/Home button detection (XInput on Windows, evdev on Linux). Import is guarded with `try/except ImportError`; falls back to keyboard-only mode.
- `psutil` — emulator process suspend/resume (`psutil.Process(pid).suspend()` / `.resume()`) for the Pause Overlay.

## Architecture

### In-Process Mode Switch
TV Mode runs in the **same Python process** as the desktop app. A single `QQmlApplicationEngine` is created lazily on first TV Mode activation and never destroyed between switches. The desktop `MainWindow` and the QML `QQuickWindow` simply `hide()` and `show()` each other. `QApplication` (a superset of `QGuiApplication`) supports both Qt Widgets and Qt Quick simultaneously.

- **`_switch_to_tv_mode()`** in `rom-mate.py`: saves config → lazily builds engine + backends → `self.hide()` → `tv_window.show()`. Guards against mid-install switch with a `QMessageBox` warning.
- **`_switch_to_desktop_mode()`** in `rom-mate.py`: `tv_window.hide()` → reloads config from disk → `self.show()` → `_refresh_library_grid()`.
- QML window close button routes back to desktop mode (`onClosing: { close.accepted = false; appBackend.requestDesktopMode() }`), never quits.

### Single Instance
A `QLocalServer` guard in `main()` enforces one running instance. On startup, the app attempts to connect as a `QLocalSocket` to `"rom-mate-neo-singleton"`. If connection succeeds, a second instance is already running — exit immediately. `QLocalServer.removeServer(name)` is called before listening to clear stale locks from prior crashes.

### Module Structure

```
rom-mate.py                              ← modified: TV Mode button, _switch_to_tv_mode(), _switch_to_desktop_mode(), single-instance guard, 3 new config defaults

rom_mate/tv/
├── __init__.py
├── bridge/
│   ├── __init__.py
│   ├── app_backend.py                   ← AppBackend QObject: config, library, server access, desktop-switch signal
│   ├── game_backend.py                  ← GameBackend QObject: launch → cloud restore → subprocess → cloud upload; pause/resume
│   ├── cloud_backend.py                 ← CloudBackend QObject: interactive slot list, restore, upload, delete
│   ├── controller.py                    ← ControllerBackend QObject: gamepad polling, dead-zone, guide button exclusion guard
│   └── image_provider.py               ← CoverImageProvider (QQuickImageProvider wrapping cover/cache.py; no loader.py dependency)
└── qml/
    ├── Main.qml                         ← root ApplicationWindow, StackView, ViewTabBar at top, FocusManager, navigation event wiring; LB/RB and Del/End switch tabs
    ├── theme/
    │   └── Theme.qml                    ← pragma Singleton: colors, font sizes, spacing constants (mirrors dark theme)
    ├── views/
    │   ├── HomeView.qml                 ← Netflix-style rows + FanartBackground
    │   ├── LibraryView.qml              ← installed game GridView
    │   ├── ServerView.qml               ← platform carousel → per-platform game wall
    │   ├── DetailsView.qml              ← metadata, cover, action buttons, cloud saves panel
    │   └── SettingsView.qml             ← General / Theme / Controller Mapping / Keybinds; editable exclusion list
    └── components/
        ├── FanartBackground.qml         ← two-image crossfade animation (SequentialAnimation, 5s hold + 1s fade)
        ├── GameCard.qml                 ← focusable cover art card with title; activeFocus border
        ├── GameWall.qml                 ← GridView of GameCard with controller focus wrap
        ├── PlatformCarousel.qml         ← horizontal ListView of platform images
        ├── ViewTabBar.qml               ← horizontal icon row (Home / Library / Server tabs); switched with LB/RB or Del/End
        ├── SlotPickerDialog.qml         ← interactive cloud save slot selection overlay (controller-navigable)
        └── PauseOverlay.qml             ← fullscreen guide button pause menu (Resume / View Manual / Quit)
```

### Backend Bridges
All business logic stays in Python `QObject` subclasses. QML calls `@Slot` methods and reads `@Property` values; no business logic inside QML files.

- **`AppBackend`**: Config reads/writes, library game list, server platform/ROM catalog, `requestDesktopMode()` slot that emits `switchToDesktopModeRequested` → wired to `_switch_to_desktop_mode()` in `MainWindow`.
- **`GameBackend`**: `launchGame()` (pre-launch cloud restore → emulator resolution → subprocess → post-launch cloud upload), `quitGame()`, `pauseEmulator()` / `resumeEmulator()` via `psutil`, `isGuideButtonAllowed()`.
- **`CloudBackend`**: `loadSlotsForGame()` calls `latest_server_records_by_slot` from `rom_mate/library/cloud_restore.py` in a `QThread`. Emits `slotsLoaded(QVariantList)` with items containing `id`, `file_name`, `slot`, `timestamp_text`, `emulator`. Also provides `restoreSlot()`, `uploadSave()`, `deleteSlot()`.
- **`ControllerBackend`**: Polls `inputs.get_gamepad()` in a background `QThread`. Maps BTN_DPAD_*, BTN_SOUTH (confirm), BTN_EAST (back), BTN_MODE (Guide), BTN_TL (LB → previous tab), BTN_TR (RB → next tab) to `navigationEvent(direction: str)` signal. Analog stick events filtered by dead-zone with 200ms repeat cap. Tab navigation events also fired by Del (previous tab) and End (next tab) keyboard keys. Guide button emission suppressed when active emulator is on the exclusion list.
- **`CoverImageProvider`**: `QQuickImageProvider` registered as `"covers"` so QML uses `image://covers/<key>`. Checks `rom_mate/cover/cache.py` disk cache first; falls back to HTTP fetch with auth token. Does **not** import `rom_mate/cover/loader.py` (Widgets-specific).

### Config Keys Added
| Key | Default | Purpose |
|-----|---------|---------|
| `tv_mode_home_view` | `"home"` | Which view opens first in TV mode (`"home"`, `"library"`, `"server"`) |
| `tv_guide_button_exclusion_list` | `["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia"]` | Emulators that handle the Guide button internally |
| `tv_mode_last_active` | `false` | Whether the app was last closed while in TV mode (reserved for future auto-start) |

---

## Views

### Tab Bar Navigation
Home, Library, and Server are displayed as a **horizontal icon row (`ViewTabBar`) at the top of the screen**. The active tab's content fills the area below the tab bar. Tab switching uses:
- **Controller**: Left Shoulder (LB) → previous tab; Right Shoulder (RB) → next tab
- **Keyboard**: Del → previous tab; End → next tab

D-pad/analog navigation within a view is independent of tab switching. Details view and Settings are not tabs — they are pushed onto the view stack over the tab bar.

### Home
- [ ] Netflix-style layout with rows of game cards
  - [ ] "Continue Playing" — games with a cloud sync session entry, sorted by recency
  - [ ] "Favorites" — favorited games
  - [ ] "New Additions" — server games sorted by `created_at` descending
  - [ ] "Highly Rated" - server games filtered to only games with a rating of 4 out of 5 stars or better
- [ ] `FanartBackground` behind all rows — two stacked images crossfade every ~5 seconds through the focused game's screenshot URLs
- [ ] Up/Down moves between rows; Left/Right scrolls within a row
- [ ] Selecting a game card navigates to the Details view

### Library
- [ ] Wall grid (`GridView`) of all installed games, displayed by cover art
- [ ] Empty state message when no games are installed
- [ ] Selecting a game navigates to the Details view (source: `"library"`)
- [ ] Active by default when the Library tab is selected via the tab bar (LB/RB or Del/End)

### Server
- [ ] Horizontal platform image carousel (`PlatformCarousel`) — platforms shown by cover art or name fallback
- [ ] Selecting a platform loads its game list (`appBackend.loadPlatformGames()`) and shows a `GameWall`
- [ ] Selecting a game in the wall navigates to the Details view (source: `"server"`)
- [ ] Active by default when the Server tab is selected via the tab bar (LB/RB or Del/End)

### Details View
- [ ] Left column: cover art image, action buttons
- [ ] Center column: title, platform, description, metadata grid (rating, regions, genres)
- [ ] Right column (if space): horizontal screenshot strip
- [ ] Action buttons:
  - [ ] **Play / Install** — launches game (pre-launch cloud restore → emulator subprocess → post-launch cloud upload) if installed; queues install if not
  - [ ] **Uninstall** — removes installed game files
  - [ ] **Cloud Saves** — opens `SlotPickerDialog` for interactive save slot selection and restore
  - [ ] **Cloud States** — opens `SlotPickerDialog` for save state slots
  - [ ] **Achievements** — *(deferred)*
- [ ] Back navigates to the previous view

### Cloud Save Slot Picker (`SlotPickerDialog`)
- [ ] Fullscreen semi-transparent modal overlay
- [ ] Controller-navigable list of save slots: slot name, emulator, relative timestamp
- [ ] Loading state (`BusyIndicator`) while slot list is fetching
- [ ] Confirm on a slot calls `cloudBackend.restoreSlot()`
- [ ] "Cancel" item at the bottom dismisses the picker
- [ ] Connects to `cloudBackend.restoreComplete` to close and show status

### Settings
Reachable by pressing Back (Esc / B/Circle) from any top-level view, or from a "Return to Desktop Mode" button.

- [ ] **General**
  - [ ] Default startup tab selector (Home / Library / Server) — which tab is active when TV Mode first opens
  - [ ] Server URL and connection status display (read-only; edit in Desktop Mode)
  - [ ] Auto cloud sync toggle
- [ ] **Theme**
  - [ ] TV mode theme selector (dark / light)
- [ ] **Controller Mapping**
  - [ ] Guide button exclusion list — user-editable `ListView`; pre-filled with `["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia"]`
  - [ ] Add / remove emulator names from the exclusion list
  - [ ] Changes persist to config immediately
- [ ] **Keybinds**
  - [ ] Display-only table of controller button → action mappings *(editing deferred)*
  - [ ] Includes: LB/RB = previous/next tab; Del/End = previous/next tab; D-pad = navigate; A/Cross = confirm; B/Circle = back; Guide = pause overlay
- [ ] **Sound** *(deferred — placeholder)*
- [ ] **Video** *(deferred — placeholder)*
- [ ] "Return to Desktop Mode" button → calls `appBackend.requestDesktopMode()`

---

## Misc Features

- [ ] Ability to change which view opens on startup (Home / Library / Server) from Settings → General
- [ ] **Fullscreen Pause Overlay** — when the Guide button is pressed during an active emulator session:
  - [ ] Display a fullscreen semi-transparent pause menu over the TV Mode UI
  - [ ] Pause the running emulator process via `psutil.Process(pid).suspend()`
  - [ ] Three options: **Resume Game** (resumes process, dismisses overlay), **View Manual** *(deferred — button disabled for MVP)*, **Quit Game** (terminates process, pops to previous view)
  - [ ] Suppress the overlay for emulators on the Guide button exclusion list (they handle it internally)
- [ ] Mode switch from desktop app (nav button) and from TV Mode (Settings → "Return to Desktop Mode")

---

## Implementation Phases

### Phase 1 — Entry Point and Shell
**Goal:** Blank fullscreen QML window activates from a nav button in the desktop app; Esc closes it and returns to desktop mode.
- Single-instance `QLocalServer` guard added to `main()`
- Three new config defaults added to `_config_defaults()`
- "TV Mode" nav button added to `MainWindow.__init__()`
- `_switch_to_tv_mode()` and `_switch_to_desktop_mode()` methods added to `rom-mate.py`
- Lazy `QQmlApplicationEngine` creation with blank `Main.qml`
- `requirements.txt` updated with `inputs` and `psutil`
- `rom_mate/tv/` package scaffold created

### Phase 2 — Config, Library, and Server Bridges
**Goal:** TV Mode reads the same config and game data as the desktop app; cover art displays via QML image provider.
- `AppBackend` QObject registered into QML context
- `CoverImageProvider` registered as `image://covers/`
- `CatalogFetchWorker` and `RomListFetchWorker` QThread workers
- Tests: `test_tv_app_backend.py`, `test_tv_image_provider.py`

### Phase 3 — Controller Input
**Goal:** D-pad/analog stick navigation, A/B confirm/back, Guide button detection with exclusion guard.
- `ControllerBackend` with `inputs` polling thread and dead-zone filtering
- `Main.qml` connects `controllerBackend.navigationEvent` to a `FocusManager`
- Tests: `test_tv_controller_backend.py`

### Phase 4 — Home View and Fanart Background
**Goal:** Home view renders five rows with a crossfading fanart background; game selection navigates to Details.
- `FanartBackground.qml` — two-image `SequentialAnimation` crossfade
- `ViewTabBar.qml` — horizontal icon row; LB/RB and Del/End shoulder/key events switch active tab
- `GameCard.qml`, `GameRow.qml`, `HomeView.qml`

### Phase 5 — Library and Server Views
**Goal:** Library wall and Server platform carousel are navigable and linked to Details.
- `GameWall.qml`, `LibraryView.qml`, `PlatformCarousel.qml`, `ServerView.qml`

### Phase 6 — Details View and Game Launch
**Goal:** Full details panel with Play/Install triggering cloud restore → subprocess → cloud upload.
- `GameBackend` QObject with full launch lifecycle
- `DetailsView.qml` with action buttons wired to backend slots
- Tests: `test_tv_cloud_backend.py` (slot loading), emulator launch mocked in backend tests

### Phase 7 — Interactive Cloud Save Slot Picker
**Goal:** Cloud Saves and Cloud States buttons open a controller-navigable slot picker.
- `CloudBackend` QObject — `loadSlotsForGame()`, `restoreSlot()`, `uploadSave()`, `deleteSlot()`
- `SlotPickerDialog.qml` — fullscreen modal with slot list and busy indicator
- Tests: `test_tv_cloud_backend.py` completed

### Phase 8 — Settings View
**Goal:** Settings panel is reachable from any view; includes editable exclusion list and startup view selector.
- `SettingsView.qml` with all submenus
- Exclusion list editing wired to `appBackend.setGuideExclusionList()`

### Phase 9 — Pause Overlay
**Goal:** Guide button mid-game shows a fullscreen pause menu; emulator process is suspended while overlay is visible.
- `PauseOverlay.qml` — Resume / View Manual (disabled MVP) / Quit
- `GameBackend.pauseEmulator()` and `resumeEmulator()` via `psutil`
- `ControllerBackend` suppression check for exclusion list

### Phase 10 — Tests
**Goal:** All backend bridges have unit test coverage.
- `test_tv_app_backend.py` — config defaults, exclusion list, homeView property
- `test_tv_cloud_backend.py` — slot loading, restore, upload, error paths
- `test_tv_image_provider.py` — cache hit/miss, no `loader.py` import
- `test_tv_controller_backend.py` — exclusion check, case-insensitivity, dead-zone
- `test_tv_single_instance.py` — `QLocalServer` acquire and duplicate guard

---

## Files Modified in Existing Codebase

| File | Change |
|------|--------|
| `rom-mate.py` | Single-instance guard; TV Mode nav button; `_switch_to_tv_mode()` / `_switch_to_desktop_mode()`; 3 new config defaults; `closeEvent` cleanup |
| `requirements.txt` | Add `inputs`, `psutil` |

All other changes are additive (new files under `rom_mate/tv/` and `tests/`).

---

## Edge Cases and Constraints

- **Mid-install switch**: `_switch_to_tv_mode()` checks `self.install_in_progress` and shows a `QMessageBox` warning before proceeding.
- **Engine load failure**: If `rootObjects()` is empty after engine load, stay on desktop, show error toast, log `engine.warnings()`.
- **QML window close button**: Routes back to desktop mode, never quits the application.
- **Stale `QLocalServer` socket** from crash: `QLocalServer.removeServer(name)` called before listening.
- **`psutil.suspend()` on Windows**: Catches `AccessDenied` and `NoSuchProcess` silently.
- **`loader.py` is off-limits**: `CoverImageProvider` uses only `rom_mate/cover/cache.py` + direct `QImage` construction.
- **Config reload on desktop switch**: `_switch_to_desktop_mode()` reloads config from disk to pick up any TV Mode settings changes immediately.
- **`inputs` not installed**: `ControllerBackend` operates in keyboard-only mode; QML `Keys` attached property handles all navigation natively.
- **Emulator exclusion list check is case-insensitive** (e.g. `"cemu"` matches `"Cemu"`).
- **Fractional DPI**: No extra work needed — `AA_EnableHighDpiScaling` is default in PySide6 6.x.
- **"Favorites" row**: Renders empty with a placeholder. Full favorites system is a future enhancement.
- **"View Manual" in Pause Overlay**: Button renders as disabled ("Manual Not Available") for MVP; no manual URL field is currently in the RomM API payload.