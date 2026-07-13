# Linux Support — Future Plans

> **Status**: ~85% Complete (Updated 2026-07-11). Core functionality implemented; versioning and release polish remain.
> This document defines the scope, design decisions, and sequencing for adding full
> first-class Linux support to grid-launcher.

---

## Implementation Status

**Phase 1 - Build Infrastructure: ✅ ~95% Complete**
- ✅ XDG path helpers implemented (`xdg_config_home()`, `xdg_data_home()`)
- ✅ Platform guards in all major emulator files
- ✅ RetroArch core extension support (`.so`/`.dll`/`.dylib`)
- ✅ GitHub Actions workflows (Linux testing + AppImage builds)
- ✅ Keyring-based token storage with fallbacks
- ✅ AppImage packaging infrastructure
- ❌ Xenia Canary platform gating (blocks Windows-only variant on Linux)

**Phase 2 - Linux-Native Emulators: ✅ 95% Complete**
- ✅ All native emulators (Dolphin, PCSX2, DuckStation, RPCS3, Xemu, Cemu, Eden, Azahar, PPSSPP, Redream, RetroArch) have XDG paths
- ✅ Xenia Edge now has Linux AppImage support
- ✅ Cemu SDL controller profile for Linux
- ✅ Comprehensive Linux path candidate tests

**Phase 3 - Wine/Proton: ✅ 85% Complete**
- ✅ Wine/Proton auto-detection on startup (scans Steam directories)
- ✅ Windows game launch dispatch via Wine/umu-run for Proton
- ✅ Per-game and default compat tool configuration UI
- ✅ Wine path translation utilities
- ⚠️ Extended configuration (DXVK, env vars) could be enhanced

**Phase 4 - Public Release: ❌ 30% Complete**
- ✅ AppImage build script functional
- ❌ AppImage versioning and update mechanism (BLOCKER)
- ❌ AppStream metadata
- ❌ First-run Linux setup experience
- ❌ System dependency documentation

---

## Migration Note: Flatpak Support Dropped

grid-launcher no longer targets Flatpak — neither for distributing the app itself nor for
auto-installing emulators. Linux distribution is via **AppImage**, and emulator auto-install
pulls **native or AppImage** builds only.

Rationale:

- **Fewer sandboxing issues.** An unsandboxed AppImage has direct access to the ROM library,
  user-chosen emulator paths, `/dev/input`, and the system keyring — no portal negotiation,
  D-Bus proxy policy, or `--filesystem=home` caveats.
- **Simpler pipeline.** One PyInstaller-based build (`build.sh --appimage`) replaces the
  Flatpak runtime/SDK/base-app stack and `flatpak-pip-generator` dependency conversion.

What this means in practice:

- **Auto-install** downloads native or AppImage emulator builds only. The Flatpak detection
  and install code paths have been removed.
- **Manual configuration still works.** Users who prefer a Flatpak emulator can install it
  themselves and point grid-launcher at the Flatpak wrapper (or `~/.var/app/<id>/` config);
  the app just won't install or auto-detect it for them.
- **Dolphin and MAME are no longer part of auto-install.** Both remain fully playable through
  their RetroArch cores (`dolphin_libretro`, `mame_libretro` / `mame2003_plus_libretro`),
  which the RetroArch integration already covers.

The remainder of this document has been updated for the AppImage-based approach. Emulator
`~/.var/app/<id>/` paths are retained below only as candidates for locating configs of
*manually installed* Flatpak emulators — they are not used to install or auto-detect them.

---

## 1. Overview & Scope

"First-class Linux support" means parity with the Windows experience, not just "boots on Linux." The bar is:

- **Distribution**: A self-contained AppImage — no user-installed Python, no venv setup.
- **Emulator autoconfig**: Every `ensure_*` setting function in `grid_launcher/emulator/` resolves correct XDG/Linux paths, not Windows registry or `%APPDATA%` paths.
- **Library operations**: Install, extract, cloud-save, and launch all function without Windows-specific syscalls.
- **Secure token storage**: API token and RetroAchievements token are stored via the platform keyring (GNOME Keyring / KWallet), not bare base64.
- **Controller input (TV mode)**: Full gamepad input through the existing pygame path — no code changes needed. The AppImage runs unsandboxed, so `/dev/input` access works without any portal configuration.
- **Native Linux emulators**: All emulators with native Linux builds are detected, configured, and launched correctly.
- **Windows-only emulators**: Xenia Canary (original Xbox 360 emulator) should be hidden on Linux; Xenia Edge has native Linux builds and is fully supported. Emulators with both native Linux builds and Windows builds show the correct default path.
- **CI**: A Linux build job produces the AppImage artifact on every release, alongside the existing Windows `.exe`.

SPEC.md already acknowledges this intent: *"…intended to ship as a self-contained executable on Windows and a wrapper-based launch target on Linux."*

---

## 2. AppImage Packaging

> **Superseded**: Earlier drafts proposed distributing grid-launcher as a Flathub Flatpak.
> That approach was dropped (see the migration note at the top). The app ships as an
> AppImage, which the repository already builds via `build.sh --appimage` and `appimagetool`.

### 2.1 Build Pipeline

The Linux build reuses the existing PyInstaller spec (`grid-launcher.spec`) and wraps the
output in an AppImage:

1. `build.sh` runs PyInstaller to produce a self-contained bundle (Python, PySide6/Qt6, and
   every `requirements.txt` dependency included).
2. The bundle is staged into an `AppDir` under `appimage/` (`AppRun`, `.desktop`, icon).
3. `appimagetool` packages the `AppDir` into a single executable `.AppImage`.

No Flatpak runtime, SDK, base app, or `flatpak-pip-generator` step is involved — dependencies
come straight from `requirements.txt` via PyInstaller.

### 2.2 Runtime File Layout

`retroarch-core-list.json`, `emulator-autoprofiles.json`, and `assets/` are bundled alongside
the frozen application. `grid-launcher.py` resolves these relative to `sys._MEIPASS`
(PyInstaller) or the script directory, so no `GRID_LAUNCHER_SHARE_DIR` indirection is required.

### 2.3 No Sandbox, No Portals

The AppImage runs unsandboxed with the same permissions as the invoking user. This is a
deliberate simplification: it removes the portal negotiation, D-Bus proxy, and
`--filesystem=home` concerns a Flatpak build would impose:

- **ROM library / emulator paths**: Full read/write access to any user-chosen path — no
  file-chooser portal round-trip.
- **Controller input**: `/dev/input/event*` and `/dev/hidraw*` are directly accessible for the
  pygame/SDL2 path (subject to the usual `input` group / udev rules on some distros).
- **Secret storage**: The `org.freedesktop.secrets` D-Bus service is reachable directly, so
  `keyring` works without any `--talk-name` allowance (see Section 7).

### 2.4 GitHub Actions CI Workflow

The Linux release job runs `build.sh --appimage` on `ubuntu-latest` and attaches the resulting
`.AppImage` to the release, mirroring the Windows `.exe` job:

```yaml
name: Build Linux (AppImage)

on:
  release:
    types: [created]

permissions:
  contents: write

jobs:
  appimage:
    name: Build AppImage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - name: Install system dependencies
        run: sudo apt-get update && sudo apt-get install -y 7zip librsvg2-bin libfuse2
      - name: Build AppImage
        run: |
          python3 -m venv .venv
          . .venv/bin/activate
          pip install -r requirements.txt
          ./build.sh --appimage
      - name: Attach AppImage to release
        uses: softprops/action-gh-release@v2
        with:
          files: dist/grid-launcher-x86_64.AppImage
```

`7zip` provides the `7z` binary that RetroArch archive extraction relies on (see
`archive_preparation.py`); `librsvg2-bin` supplies `rsvg-convert` for the icon; `libfuse2`
lets the AppImage mount at runtime.

---

## 3. XDG Path Migration

### 3.1 XDG Base Directory Quick Reference

| Variable | Default | Used For |
|---|---|---|
| `$XDG_CONFIG_HOME` | `~/.config` | Emulator `.ini` / `.cfg` config files |
| `$XDG_DATA_HOME` | `~/.local/share` | Emulator data, save files, NAND images |
| `$XDG_CACHE_HOME` | `~/.cache` | Shader caches, thumbnails |
| `$XDG_STATE_HOME` | `~/.local/state` | Logs, recent files (rarely needed here) |

Always read the environment variable first and fall back to the default. Never hardcode `~/.config` directly — `os.environ.get("XDG_CONFIG_HOME", "")` with a fallback.

### 3.2 Platform-Aware Path Resolver

Add `xdg_config_home()` and `xdg_data_home()` helpers to `grid_launcher/core/path.py`:

```python
import os
import sys
from pathlib import Path

def xdg_config_home() -> Path:
    """Return $XDG_CONFIG_HOME or ~/.config on Linux/macOS.
    Returns None on Windows (callers should use APPDATA/LOCALAPPDATA instead)."""
    if sys.platform == "win32":
        return None
    val = os.environ.get("XDG_CONFIG_HOME", "").strip()
    if val:
        return Path(val).expanduser()
    return Path.home() / ".config"

def xdg_data_home() -> Path:
    """Return $XDG_DATA_HOME or ~/.local/share on Linux/macOS."""
    if sys.platform == "win32":
        return None
    val = os.environ.get("XDG_DATA_HOME", "").strip()
    if val:
        return Path(val).expanduser()
    return Path.home() / ".local" / "share"
```

Emulator modules then call these helpers instead of repeating the environment lookup inline. This consolidates the XDG logic that is already scattered across `duckstation.py`, `xemu.py`, `redream.py`, `azahar.py`, and `eden.py`.

### 3.3 Per-Emulator Linux Config Paths

> The `~/.var/app/<id>/` "Flatpak <emulator>" candidates below are only for locating the
> config of a Flatpak emulator a user installed and configured **manually**. Auto-install and
> auto-detection target native/AppImage builds; these Flatpak paths are never used to install
> or discover an emulator.

#### `dolphin.py` — `dolphin_user_root_candidates()`

**Current state**: `_registry_user_root()` is properly guarded behind `sys.platform == "win32"`. Windows-specific `%APPDATA%` and `OneDrive` / `USERPROFILE\Documents` paths are in a `sys.platform == "win32"` block. `~/.dolphin-emu` is already in the candidate list for non-Windows.

**Required change**: Add the XDG data home path. Dolphin on Linux (native) uses `~/.local/share/dolphin-emu/` by default (not `~/.dolphin-emu`, which is the older pre-XDG path). Both should be candidates:

```python
# After the win32 block:
else:
    xdg = xdg_data_home()
    if xdg:
        candidates.append(xdg / "dolphin-emu")
    candidates.append(home_path / ".dolphin-emu")  # legacy
```

**Flatpak Dolphin**: `org.DolphinEmu.dolphin-emu` stores data at `~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/`. This path should be an additional candidate when running inside or alongside a Flatpak environment.

#### `pcsx2.py` — `pcsx2_windows_documents_folder()` and config candidates

**Current state**: `_windows_documents_folder()` is fully guarded. The config path function needs Linux candidates added.

**Required change**: Add Linux config path. PCSX2 on Linux defaults to `$XDG_CONFIG_HOME/PCSX2/` (i.e., `~/.config/PCSX2/inis/PCSX2.ini`). The Flatpak version (`net.pcsx2.PCSX2`) stores config at `~/.var/app/net.pcsx2.PCSX2/config/PCSX2/`.

```python
# In pcsx2_config_path_candidates():
if sys.platform != "win32":
    xdg_cfg = xdg_config_home()
    if xdg_cfg:
        candidates.append(xdg_cfg / "PCSX2" / "inis" / "PCSX2.ini")
    candidates.append(Path.home() / ".var" / "app" / "net.pcsx2.PCSX2" / "config" / "PCSX2" / "inis" / "PCSX2.ini")
```

#### `duckstation.py` — `duckstation_config_path_candidates()`

**Current state**: Already has `~/.local/share/duckstation`, `~/.config/duckstation`, and `$XDG_DATA_HOME/duckstation`, `$XDG_CONFIG_HOME/duckstation` in candidates. This file is already Linux-ready.

**Remaining gap**: Flatpak DuckStation (`org.duckstation.DuckStation`) stores config at `~/.var/app/org.duckstation.DuckStation/config/duckstation/`. Add this candidate.

#### `azahar.py` (Citra fork for 3DS) — config path candidates

**Current state**: Has a `sys.platform == "win32"` block for `%APPDATA%/Azahar` and an `else` block for `$XDG_DATA_HOME/Azahar`. Partially done.

**Required change**: Azahar on Linux uses `$XDG_CONFIG_HOME/azahar-emu/` for config (the Qt app writes config there). The `XDG_DATA_HOME` path is for data/saves. Both are needed. Additionally, the `os.path.expandvars("%APPDATA%")` call on line 132 runs unconditionally — on Linux, `%APPDATA%` is not set and `os.path.expandvars` returns the literal string `%APPDATA%`. Wrap line 132 in `if sys.platform == "win32":` or move it inside the existing guarded block.

**Fix for line 132**:

```python
# Before (runs on all platforms):
Path(os.path.expandvars("%APPDATA%")) / "Azahar" / "qt-config.ini"

# After:
if sys.platform == "win32":
    appdata = os.environ.get("APPDATA", "")
    if appdata:
        candidates.append(Path(appdata) / "Azahar" / "qt-config.ini")
else:
    xdg_cfg = xdg_config_home()
    if xdg_cfg:
        candidates.append(xdg_cfg / "azahar-emu" / "qt-config.ini")
    xdg_data = xdg_data_home()
    if xdg_data:
        candidates.append(xdg_data / "azahar-emu" / "qt-config.ini")
```

#### `eden.py` (yuzu fork for Switch) — config path candidates

**Current state**: Line 206 has `Path(os.path.expandvars("%APPDATA%")) / "eden" / "config" / "qt-config.ini"` running unconditionally — same bug as `azahar.py` line 132. Guarded block at line 331 handles XDG correctly in the `else` branch.

**Fix**: Wrap the unconditional `%APPDATA%` expansion in a `sys.platform == "win32"` check. The existing `else` block at line 337 already adds `$XDG_DATA_HOME/eden/`. Add `$XDG_CONFIG_HOME/eden/qt-config.ini` as well since Eden stores its config there (not data home).

#### `cemu.py` — controller profile and config candidates

**Current state**: The XInput controller profile XML is hardcoded and references the XInput API. On Linux, Cemu uses SDL2 for input. The `APPDATA`/`LOCALAPPDATA` lookup on line 209 has no platform guard.

**Required changes**:

1. Guard the `APPDATA`/`LOCALAPPDATA` lookup behind `sys.platform == "win32"`.
2. Add Linux config path: Cemu (native Linux) uses `$XDG_CONFIG_HOME/Cemu/` and stores its settings in `settings.xml` there. The Flatpak (`info.cemu.Cemu`) uses `~/.var/app/info.cemu.Cemu/config/Cemu/`.
3. The controller profile XML (`_DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE`) uses `<api>XInput</api>`. On Linux this needs to be `<api>SDLController</api>` with an appropriate SDL UUID. A separate `_DEFAULT_CEMU_SDL_CONTROLLER_PROFILE` constant should be written and selected based on `sys.platform`.

#### `xemu.py` (Xbox OG emulator)

**Current state**: Already has proper `sys.platform == "win32"`, `sys.platform == "darwin"`, and XDG fallback branches. The Flatpak ID is `app.xemu.xemu` — add `~/.var/app/app.xemu.xemu/data/xemu/xemu/` as a candidate in the non-Windows path.

**Status**: Near complete, only Flatpak path needs adding.

#### `ppsspp.py`

**Current state**: Config is resolved relative to the emulator directory or `--home` arg. No OS-specific paths. PPSSPP on Linux stores its data in `~/.config/ppsspp/PSP/` when no `--home` is passed. Add this as a fallback candidate when `emulator_path_text` is empty and no `--home` is in the template.

The Flatpak PPSSPP (`org.ppsspp.PPSSPP`) stores config at `~/.var/app/org.ppsspp.PPSSPP/config/ppsspp/PSP/`.

#### `fbneo.py`

**Current state**: Purely emulator-directory-relative. FinalBurn Neo does not have a separate system-level config directory. No changes needed for Linux path resolution. The `savestates` directory referenced in `fbneo_directory_settings` is relative to `emulator_dir`, which works cross-platform.

#### `mame.py`

**Current state**: Parses `--inipath`, `--cfg_directory`, etc. from the launch template. MAME on Linux defaults to `~/.mame/` for config. Add `~/.mame/mame.ini` and `$XDG_CONFIG_HOME/mame/mame.ini` as fallback candidates when launch template overrides are absent.

#### `pico8.py`

**Current state**: Uses `--home` argument parsing. Pico-8 on Linux stores data at `~/.lexaloffle/pico-8/` or `$XDG_DATA_HOME/pico-8/` (Pico-8 0.2.x+). Add these as default candidates when no `--home` is in the launch template.

#### `redream.py`

**Current state**: `_default_user_root()` already correctly returns `$XDG_DATA_HOME/redream` or `~/.local/share/redream` on Linux. Fully ready.

#### `dolphin.py` — Flatpak candidate

As noted above, add `~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/` as a candidate.

### 3.4 RetroArch Core Path in `launch.py`

`retroarch_core_argument_path()` currently always generates `.dll` extensions:

```python
# Current — Windows-only
if normalized.casefold().endswith(".dll"):
    core_file = normalized
elif normalized.casefold().endswith("_libretro"):
    core_file = f"{normalized}.dll"
else:
    core_file = f"{normalized}_libretro.dll"
return f"cores/{core_file}"
```

On Linux, RetroArch cores use `.so`. On macOS they use `.dylib`. This function needs a platform-aware extension:

```python
import sys

def _core_extension() -> str:
    if sys.platform == "win32":
        return ".dll"
    if sys.platform == "darwin":
        return ".dylib"
    return ".so"

def retroarch_core_argument_path(configured_core: str) -> str:
    core = configured_core.strip()
    if not core:
        return ""
    ext = _core_extension()
    normalized = core.replace("\\", "/")
    if "/" in normalized:
        return normalized
    # Strip any existing platform-specific extension to normalise
    for old_ext in (".dll", ".so", ".dylib"):
        if normalized.casefold().endswith(old_ext):
            normalized = normalized[:-len(old_ext)]
            break
    if normalized.casefold().endswith("_libretro"):
        core_file = f"{normalized}{ext}"
    else:
        core_file = f"{normalized}_libretro{ext}"
    return f"cores/{core_file}"
```

This is a **pure logic change with no side effects** — existing tests pass `configured_core` values without extensions and check for `_libretro.dll`, so those tests will need Linux variants that assert `.so`.

### 3.5 `archive_preparation.py` — `MoveFileExW`

The `MoveFileExW` call is already guarded (`sys.platform != "win32"` returns early). On Linux the delayed-delete fallback never runs, which is fine — Linux does not need it because file handles hold the inode open after unlink.

No changes needed beyond the existing guard.

---

## 4. Windows Game Launching on Linux

This is the most complex area and is deferred to Phase 3. The core question is: which Windows-format games can realistically run on Linux, and what is the dispatch strategy.

### 4.1 Platform Viability Matrix

| Platform | Windows Emulator | Linux Native Emulator | Linux via Wine/Proton | Recommendation |
|---|---|---|---|---|
| Xbox 360 | Xenia | None viable | Xenia via Wine (experimental) | Disable/hide on Linux in Phase 1-2 |
| Xbox OG | Xemu | Xemu (native Linux build) | N/A | Supported natively |
| PS4 (install pipeline) | FPKG extraction only | Same | N/A | Install pipeline is file-based, works |
| PS3 | RPCS3 | RPCS3 (native Linux) | N/A | Supported natively |
| Wii/GameCube | Dolphin | Dolphin (native) | N/A | Supported natively |
| Switch | Eden/Yuzu forks | Eden (native Linux) | N/A | Supported natively |
| 3DS | Azahar/Citra | Azahar (native Linux) | N/A | Supported natively |
| PS1 | DuckStation | DuckStation (native) | N/A | Supported natively |
| PS2 | PCSX2 | PCSX2 (native) | N/A | Supported natively |
| PSP | PPSSPP | PPSSPP (native) | N/A | Supported natively |
| Dreamcast | Redream | Redream (native Linux) | N/A | Supported natively |
| Wii U | Cemu | Cemu (native Linux) | N/A | Supported natively |
| Arcade (MAME/FBNeo) | MAME/FBNeo | Both native | N/A | Supported natively |
| Pico-8 | Pico-8 executable | Pico-8 Linux binary | N/A | Supported if user installs Linux binary |
| RetroArch | RetroArch | RetroArch (native) | N/A | Supported natively |
| Native PC games (.exe) | Direct launch | Wine/Proton | Optional | Phase 3 (opt-in) |

### 4.2 Proton/Wine Dispatch Strategy

This section applies to Phase 3 only, for native PC game platform games that ship as `.exe` installers. The dispatch should be **opt-in** and explicitly configured by the user, not automatic. Unexpected Wine invocations are a bad user experience.

#### Detection

Auto-detect available Wine/Proton installations in this priority order:

1. **User-configured**: A "Wine/Proton executable" setting in the Emulators settings page, stored in the app config. This is the only path that is actually invoked; auto-detection is read-only for presenting choices in the UI.
2. **Steam Proton**: Walk `~/.steam/steam/steamapps/common/` for directories matching `Proton *`. Within each, look for `proton` script or `files/bin/wine`.
3. **Proton-GE**: Common in `~/.steam/root/compatibilitytools.d/` or `~/.local/share/Steam/compatibilitytools.d/`.
4. **System Wine**: `shutil.which("wine")` and `shutil.which("wine64")`.
5. **Flatpak Bottles**: `flatpak list --app` output containing `com.usebottles.bottles`.

Detection logic should live in a new `grid_launcher/emulator/wine.py` module, keeping it separate from launch dispatch.

#### `launch.py` Integration

The launch pipeline in `grid-launcher.py` / `InstallMixin` builds a command list from the emulator path and template. For Windows executables on Linux with Wine configured, prepend the Wine/Proton command:

```python
# Pseudo-code in launch dispatch:
if sys.platform != "win32" and is_windows_executable(game_path) and wine_path:
    command = [wine_path, game_path, *extra_args]
else:
    command = [emulator_path, game_path, *extra_args]
```

`is_windows_executable()` checks `game_path.suffix.casefold() in {".exe", ".bat"}`.

The `launch.py` module's `launchable_native_game_file()` already includes `.exe` — on Linux this should either be gated behind a Wine-configured check, or `.exe` should be excluded from `_NATIVE_GAME_SUFFIXES` on Linux and only re-added when Wine is configured.

#### Bottles DBus Integration (Advanced / Optional)

Bottles exposes a DBus API at `com.usebottles.bottles` for launching executables in a named bottle. This is an advanced integration that allows the user to pick a pre-configured bottle (with specific Wine version, DXVK config, etc.) for each game. This is only worth implementing if there is user demand — defer to post-Phase-3.

### 4.3 Xenia (Xbox 360) on Linux

Xenia has no native Linux build. Running it under Wine is theoretically possible but the emulator relies heavily on DirectX 12 / Vulkan translation and has only been lightly tested under Wine. The recommendation:

- **Phase 1-2**: Gate the Xbox 360 platform and all Xenia-related UI behind `sys.platform != "win32"` — hide the emulator from the emulator list, hide the Xbox 360 platform from install targets.
- **Phase 3**: Revisit if a stable Wine-wrapped Xenia workflow emerges. If so, add it as an explicit opt-in setting ("Run Xenia via Wine") with a clear warning.

The STFS content installation code in `xenia.py` is purely file I/O and will work on Linux as-is. The content can be installed to the correct directory structure even on Linux, ready for if/when Xenia becomes viable.

---

## 5. Platform-Gated UI

### 5.1 Where to Put the Guard

Per ARCHITECTURE.md, emulator management lives in `grid_launcher/ui/mixins/emulator_ui_mixin.py` (`EmulatorUIMixin`). The emulator list shown to the user is built there. The guard should live in `EmulatorUIMixin._filter_emulators_for_platform()` (or equivalent — check the current method name). A simple helper suffices:

```python
# In grid_launcher/emulator/profiles.py or selection.py
_WINDOWS_ONLY_EMULATOR_SLUGS = frozenset({"xenia", "xenia-canary"})

def is_available_on_current_platform(emulator_slug: str) -> bool:
    if sys.platform != "win32" and emulator_slug.casefold() in _WINDOWS_ONLY_EMULATOR_SLUGS:
        return False
    return True
```

`EmulatorUIMixin` calls this when populating the emulator picker and when loading the emulator settings list. Windows-only emulators simply do not appear on Linux.

### 5.2 Install Flow Gating

`InstallMixin` orchestrates Xbox 360 XBEX installs. On Linux, the Xbox 360 install flow should be blocked early with a clear message ("Xbox 360 game installation requires Windows"). The block lives in `install_mixin.py` at the point where the platform is identified as Xbox 360. The same guard already exists implicitly for the PS4 pipeline (Linux PS4 installs work since they're file-based), so only Xbox 360 needs explicit blocking.

### 5.3 Emulator Dialog

`grid_launcher/ui/emulators.py` builds the form helpers. The emulator type dropdown that lets users add a new emulator should filter `_WINDOWS_ONLY_EMULATOR_SLUGS` via `is_available_on_current_platform()`. No other changes are needed in this file.

### 5.4 Details View

`DetailsViewMixin` renders the install/launch panel. If a ROM is Xbox 360 and the platform is Linux, the install button should show a disabled state with a tooltip like "Xbox 360 requires Windows". This mirrors the existing "block reason" system in `cloud_mixin.py` — define a new block reason constant `BLOCK_XBOX360_LINUX_UNSUPPORTED`.

---

## 6. Controller Input (TV Mode)

### 6.1 Current Architecture

`grid_launcher/tv/bridge/controller.py` already has three poll thread classes:

- `_XInputPollThread`: Windows-only, polls XInput DLL. Imports `ctypes.wintypes` inside `run()` (not at module load), so no import error on Linux.
- `_PygameGuidePollThread`: Windows helper for guide button only via pygame/SDL HID path.
- `_GamepadPollThread`: Non-Windows (Linux/macOS) full pygame joystick poll.

`ControllerBackend.start()` dispatches based on `sys.platform == "win32"`. On Linux, `_GamepadPollThread` is used. This is already correct.

### 6.2 What Actually Needs Changing

**Nothing in the controller backend itself needs changing for Linux.** The pygame SDL2 backend handles all major gamepads (Xbox controllers, PS controllers, 8BitDo, etc.) on Linux correctly. The guide button (BTN_MODE) is exposed via SDL2 on Linux without any workarounds.

The main concerns are operational:

1. **Device access**: The AppImage runs unsandboxed, so `/dev/input/event*` and `/dev/hidraw*` are directly accessible to pygame — no Flatpak `--device=all` allowance or portal is needed.
2. **SDL joystick permission in some distros**: Some distributions require the user to be in the `input` group or have a udev rule. This is a deployment/documentation concern, not a code concern.
3. **`joystickCount()` on Linux**: The non-Windows branch returns `-1` when pygame joystick is not initialized. Consider returning `0` instead to avoid confusing any UI that displays this count.

### 6.3 `_PygameGuidePollThread` on Linux

This thread is only started on Windows. On Linux, the guide button is already handled by `_GamepadPollThread` (button index 10 in `_PYGAME_BUTTON_MAP` maps to `BTN_MODE`). No changes needed.

---

## 7. Token Store

### 7.1 Current State

`token_store.py` uses DPAPI (`CryptProtectData`) on Windows and bare base64 on all other platforms. Base64 is encoding, not encryption — the token is effectively stored in plaintext on Linux. This is a meaningful security regression on a system where the config directory may be readable by other processes or users.

### 7.2 Linux Secret Storage Target

The standard cross-desktop Linux secret storage interface is the **Secret Service API** (D-Bus), implemented by:
- **GNOME Keyring** (`gnome-keyring-daemon`)
- **KWallet** (via the `kwallet-pam` / `org.kde.kwalletd6` bridge)

Both implement the `org.freedesktop.secrets` D-Bus interface.

### 7.3 Implementation Plan

Add `secretstorage` (PyPI: `secretstorage`) to `requirements.txt` as an **optional** dependency. It is a pure-Python D-Bus client for the Secret Service API. Mark it optional because headless/server environments may not have a running secret service daemon.

Add `keyring` (PyPI: `keyring`) as the higher-level abstraction — it wraps `secretstorage`, KWallet, and macOS Keychain under one API. This is the recommended approach since `keyring` handles backend selection automatically.

**New `token_store.py` logic for Linux**:

```python
def _linux_load_token(service: str, username: str) -> str:
    """Load a token from the system keyring on Linux."""
    try:
        import keyring
        value = keyring.get_password(service, username)
        return value or ""
    except Exception:
        return ""

def _linux_save_token(service: str, username: str, token: str) -> bool:
    """Save a token to the system keyring on Linux."""
    try:
        import keyring
        if token:
            keyring.set_password(service, username, token)
        else:
            try:
                keyring.delete_password(service, username)
            except keyring.errors.PasswordDeleteError:
                pass
        return True
    except Exception:
        return False
```

The service name should be `"grid-launcher"` with `username` values `"api-token"` and `"ra-token"`.

**Fallback**: If `import keyring` fails (not installed, or no backend available — e.g. headless CI without a running secret service), fall back to the current base64 file approach with a logged warning. This ensures the app still works in all environments.

**Migration**: On first run after upgrade, if a base64 token file exists and `keyring` is available, migrate the token to the keyring and delete the file. Add a one-shot migration step in `set_api_token()` / `set_ra_token()`.

### 7.4 `requirements.txt` Change

```
keyring>=24.0.0  ; sys_platform != "win32"
secretstorage>=3.3.3  ; sys_platform == "linux"
```

Both packages are pulled in by PyInstaller from `requirements.txt` and bundled into the AppImage. Because the AppImage is unsandboxed, `org.freedesktop.secrets` is reachable over D-Bus directly — no `--talk-name` allowance is required.

---

## 8. Testing

### 8.1 New Test Fixtures and Mocks

**XDG environment mocking**: Tests for emulator path candidates that involve XDG variables need an `xdg_env` fixture:

```python
@pytest.fixture
def xdg_env(monkeypatch, tmp_path):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "config"))
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "data"))
    monkeypatch.setenv("XDG_CACHE_HOME", str(tmp_path / "cache"))
    return tmp_path
```

**Platform mocking**: Many emulator path functions branch on `sys.platform`. Tests need to assert Linux paths without running on Linux. Use `unittest.mock.patch` on `sys.platform`:

```python
from unittest.mock import patch

with patch("sys.platform", "linux"):
    candidates = dolphin_user_root_candidates("", "", split_fn)
    # assert XDG path is in candidates
```

Because `sys.platform` is read at call time (not import time) in the emulator modules, patching it at the function level is safe.

**Token store keyring mock**: Tests for `token_store.py` Linux paths should mock `keyring`:

```python
import unittest.mock as mock

keyring_mock = mock.MagicMock()
keyring_mock.get_password.return_value = "test-token"
with mock.patch.dict("sys.modules", {"keyring": keyring_mock}):
    result = load_api_token(token_file)
    assert result == "test-token"
```

**`retroarch_core_argument_path` Linux extension test**:

```python
with patch("sys.platform", "linux"):
    result = retroarch_core_argument_path("snes9x")
    assert result == "cores/snes9x_libretro.so"
```

### 8.2 CI Test Matrix

Add a Linux job to the test workflow (separate from the Flatpak build job):

```yaml
# .github/workflows/tests.yml
jobs:
  test:
    strategy:
      matrix:
        os: [windows-latest, ubuntu-latest]
        python-version: ["3.12"]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: ${{ matrix.python-version }}
      - run: pip install -r requirements.txt pytest
      - run: python -m pytest tests/ -v
```

The Linux runner will catch any remaining `win32`-unguarded code that raises `ImportError` on Linux (e.g., `winreg`, `ctypes.windll`).

### 8.3 Existing Test Coverage Notes

- `test_emulator_autoconfig_settings.py` and `test_duckstation_config.py` — add Linux-platform variants for each `*_path_candidates` function.
- `test_retroarch_config.py` — add a test asserting `.so` extension on Linux.
- `test_cloud_save_block_reason.py` — add a test for the `BLOCK_XBOX360_LINUX_UNSUPPORTED` block reason.
- New `test_token_store_linux.py` — test keyring integration path and base64 fallback.
- New `test_wine_detection.py` — test the Proton/Wine auto-detection logic in `wine.py` (Phase 3).

All new tests must follow the project convention of using `unittest` (per AGENTS.md), not pytest (though `monkeypatch` is a pytest fixture — use `unittest.mock.patch.dict` for environment variables in unittest-style tests instead).

---

## 9. Emulator Platform Matrix

| Emulator | Platform | Native Linux Build | Flatpak ID | Config Path (Linux) | Save Path (Linux) | Windows-Only |
|---|---|---|---|---|---|---|
| RetroArch | Multi | Yes | `org.libretro.RetroArch` | `~/.config/retroarch/` | `~/.config/retroarch/saves/` | No |
| Dolphin | Wii/GC | Yes | `org.DolphinEmu.dolphin-emu` | `~/.local/share/dolphin-emu/Config/` | `~/.local/share/dolphin-emu/GC/` | No |
| PCSX2 | PS2 | Yes | `net.pcsx2.PCSX2` | `~/.config/PCSX2/inis/` | `~/.config/PCSX2/memcards/` | No |
| DuckStation | PS1 | Yes | `org.duckstation.DuckStation` | `~/.local/share/duckstation/` | `~/.local/share/duckstation/memcards/` | No |
| RPCS3 | PS3 | Yes | `net.rpcs3.RPCS3` | `~/.config/rpcs3/` | `~/.config/rpcs3/dev_hdd0/home/` | No |
| Xemu | Xbox OG | Yes | `app.xemu.xemu` | `~/.local/share/xemu/xemu/` | `~/.local/share/xemu/xemu/` | No |
| Cemu | Wii U | Yes | `info.cemu.Cemu` | `~/.config/Cemu/` | `~/.config/Cemu/mlc01/` | No |
| Eden/Yuzu fork | Switch | Yes | N/A (no Flathub) | `~/.local/share/eden/` | `~/.local/share/eden/nand/` | No |
| Azahar/Citra fork | 3DS | Yes | N/A | `~/.config/azahar-emu/` | `~/.local/share/azahar-emu/` | No |
| PPSSPP | PSP | Yes | `org.ppsspp.PPSSPP` | `~/.config/ppsspp/PSP/SYSTEM/` | `~/.config/ppsspp/PSP/SAVEDATA/` | No |
| Redream | Dreamcast | Yes | N/A | `~/.local/share/redream/` | `~/.local/share/redream/` | No |
| MAME | Arcade | Yes | `org.mamedev.MAME` | `~/.mame/` | `~/.mame/nvram/` | No |
| FinalBurn Neo | Arcade | Yes (Linux build) | N/A | `<emulator_dir>/config/` | `<emulator_dir>/savestates/` | No |
| Pico-8 | Pico-8 | Yes (commercial) | N/A | `~/.lexaloffle/pico-8/` | `~/.lexaloffle/pico-8/carts/` | No |
| **Xenia** | **Xbox 360** | **No** | **N/A** | N/A | N/A | **Yes** |
| **Xenia Canary** | **Xbox 360** | **No** | **N/A** | N/A | N/A | **Yes** |

**Notes**:
- "Flatpak ID" is listed where an official Flathub Flatpak exists. These are **not** auto-installed or auto-detected — the column is a reference for users who install a Flatpak emulator manually. Their `~/.var/app/<id>/` config paths can be added as candidates so autoconfig still finds a manually configured Flatpak emulator.
- **Dolphin** and **MAME** are not part of the app's auto-install list. They are covered by their RetroArch cores (`dolphin_libretro`, `mame_libretro` / `mame2003_plus_libretro`); a user wanting the standalone emulator installs it manually.
- Pico-8 is a commercial product. The Linux binary is distributed separately by the user; the app only launches it, not installs it.
- Eden / Azahar forks are not on Flathub due to legal concerns around the original projects; users download binaries directly.
- FinalBurn Neo's Linux availability refers to standalone builds. The libretro core (via RetroArch) is fully supported.

---

## 10. Phased Rollout Recommendation

### Phase 1: Build Infrastructure — ✅ ~95% Complete

**Goal**: The app compiles, runs, and passes all tests on Linux. No feature regressions on Windows.

**Tasks** (in order):

1. Add `xdg_config_home()` and `xdg_data_home()` to `grid_launcher/core/path.py`.
2. Fix the unconditional `%APPDATA%` expansions in `azahar.py` (line 132) and `eden.py` (line 206) — wrap in `sys.platform == "win32"` guards.
3. Add XDG path candidates to all emulator files that currently lack them: `cemu.py`, `ppsspp.py`, `mame.py`, `pico8.py`. Confirm Flatpak variant paths for `dolphin.py`, `pcsx2.py`, `duckstation.py`, `xemu.py`.
4. Fix `retroarch_core_argument_path()` in `launch.py` to use `.so` on Linux.
5. Add `is_available_on_current_platform()` to `grid_launcher/emulator/profiles.py` and gate Xenia in `EmulatorUIMixin` and `InstallMixin`.
6. Add a GitHub Actions test workflow (`.github/workflows/tests.yml`) with both `windows-latest` and `ubuntu-latest` matrix entries.
7. Add an AppImage build workflow (`.github/workflows/appimage-linux.yml`) that runs `build.sh --appimage` on `ubuntu-latest` (see Section 2.4).
8. Confirm `retroarch-core-list.json` and `emulator-autoprofiles.json` resolve correctly from the PyInstaller bundle (`sys._MEIPASS`) inside the AppImage — no `GRID_LAUNCHER_SHARE_DIR` indirection needed.
9. Update `requirements.txt` with platform-conditional `keyring` and `secretstorage` entries.

**Exit criteria**: `python -m pytest tests/` passes on `ubuntu-latest`. The AppImage CI job produces a `.AppImage` bundle.

### Phase 2: Linux-Native Emulators Working — ✅ 95% Complete

**Goal**: All emulators with native Linux builds can be configured, auto-configured, and launched.

**Tasks** (in order):

1. Manual QA pass: install each native Linux emulator (Dolphin, PCSX2, DuckStation, RPCS3, Xemu, Cemu, Eden, Azahar, PPSSPP, Redream, RetroArch), point the app at them, verify autoconfig writes correct settings.
2. Fix any `ensure_*` functions that write incorrect paths on Linux (discovered during QA). These will be in `grid_launcher/emulator/autoconfig.py` and the per-emulator files.
3. Implement `keyring`-based token storage and add `test_token_store_linux.py`.
4. Add new test coverage for Linux path candidates in `test_emulator_autoconfig_settings.py` and a new `test_emulator_path_candidates_linux.py`.
5. Test cloud save upload/restore cycle with a Linux-native emulator.
6. Cemu Linux: write `_DEFAULT_CEMU_SDL_CONTROLLER_PROFILE` and wire it into the autoconfig path.

**Exit criteria**: All Linux-native emulators listed in the platform matrix function end-to-end (launch, save, cloud sync). No Windows-specific crashes.

### Phase 3: Windows-Game-on-Linux Launching (Proton/Wine) — ✅ 85% Complete

**Goal**: Users can optionally configure Wine/Proton to launch `.exe`-based PC games.

**Tasks** (in order):

1. Create `grid_launcher/emulator/wine.py` with Proton/Wine auto-detection logic.
2. Add a "Wine/Proton executable" setting to the Emulators settings page (in `EmulatorUIMixin`), visible only on Linux.
3. Modify the launch dispatch in `InstallMixin` / `grid-launcher.py` to prepend the Wine command for `.exe` games on Linux when Wine is configured.
4. Update `launchable_native_game_file()` in `launch.py` to gate `.exe` launches on Linux behind a "Wine configured" check.
5. Write `test_wine_detection.py`.
6. Document setup steps in README.

**Out of scope for Phase 3**: Bottles DBus integration, automatic DXVK configuration, Xenia-via-Wine.

### Phase 4: Public User-Facing Release — ❌ 30% Complete

**Goal**: A polished, self-updating AppImage release for end users.

**Tasks** (in order):

1. Finalize the AppImage `.desktop` file and app icon; embed AppStream `metainfo.xml` (screenshots, release notes) inside the `AppDir` for software-center discovery.
2. Integrate the first-run dialog (`grid_launcher/ui/dialogs.py`) to prompt for the ROM library root and emulator paths on initial launch.
3. Add AppImage update support (e.g. embed update information / ship a `.zsync` file) so users can update in place.
4. Verify the AppImage runs across common distros (glibc floor, `libfuse2` availability) and document any required system packages (`7zip` for RetroArch extraction).
5. Attach the `.AppImage` to GitHub Releases via the CI workflow from Section 2.4.

**Note**: No emulator binaries are bundled. Pico-8 is user-supplied, so there are no proprietary-software distribution concerns.

---

## Open Questions

1. **App ID**: The AppImage `.desktop` file and AppStream metainfo still need a stable reverse-DNS ID (`io.github.*`). Confirm the intended public name/repo URL before finalizing.
2. **Eden legal status**: Eden is a yuzu fork. Its legal standing is uncertain following the yuzu settlement. The app should not bundle it; users supply their own binary. Confirm whether the app should show a warning in the emulator dialog about its status.
3. **Azahar config path verification**: The Linux config path `~/.config/azahar-emu/` is based on the Qt app naming convention. Verify against an actual Azahar Linux installation before writing tests.
4. **Cemu SDL controller profile**: The `<api>SDLController</api>` format for Cemu on Linux needs verification against Cemu's actual XML schema for SDL controller mappings.
5. **Keyring backend availability**: The AppImage reaches `org.freedesktop.secrets` over the session D-Bus directly. Verify a secret service (GNOME Keyring / KWallet) is running on target desktops; fall back to the base64 file store when it is absent (e.g. headless setups).
6. **RetroArch core path**: `retroarch_core_argument_path()` generates a relative `cores/` path. Verify this resolves correctly against the RetroArch working directory for both a native install and a manually configured Flatpak RetroArch (`~/.var/app/org.libretro.RetroArch/config/retroarch/cores/`).
