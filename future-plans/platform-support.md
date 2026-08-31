# Platform Support — Status & Plans

This document records what each operating system target looks like today, what shipped to get
there, and what is still open. It covers all OS support topics; it was previously scoped to
Linux only.

| OS | Status | Distribution | CI |
|---|---|---|---|
| Windows | Primary target, fully supported | PyInstaller `--onefile` `.exe` | `.github/workflows/pyinstaller-windows.yml` |
| Linux | Supported | AppImage (with `.zsync` update info); also a plain onefile binary | `.github/workflows/appimage-linux.yml`, `.github/workflows/pyinstaller-linux.yml` |
| macOS | Untargeted | None | None |

---

## Windows

The reference platform. Everything in the app is developed and QA'd here first.

**Distribution.** `build.bat` / `build.ps1` and the Windows CI workflow run PyInstaller in
`--onefile --windowed` mode. Bundled data is staged first by
`python scripts/stage_assets.py --platform windows --output build/bundle-assets`, then passed
as `--add-data "build/bundle-assets;assets"` alongside `retroarch-core-list.json`,
`romm-platform-cores.json`, and `emulator-autoprofiles.json`.

**Windows-only pieces:**

- **Bundled 7-Zip.** `assets/tools/7z/7z.exe` and `7z.dll` ship only in Windows builds —
  `scripts/stage_assets.py::plan_copies()` copies `assets/tools/7z/` for `--platform windows`
  and skips it for Linux. `archive_preparation.py` additionally downloads a portable `7zr.exe`
  / `7zz.exe` when needed; both `_ensure_portable_7z()` and `_ensure_full_7z()` return early on
  non-Windows. Linux and macOS instead probe a fixed list of system paths (`/usr/bin/7z`,
  `/usr/bin/7za`, `/usr/bin/7zz`, `/usr/lib/p7zip/7za`, `/opt/homebrew/bin/7z`,
  `/usr/local/bin/7z`).
- **Keyring backend.** The Windows build pins `--hidden-import keyring.backends.Windows` so
  PyInstaller keeps the Credential Manager backend. `token_store.py` also keeps a DPAPI
  (`CryptProtectData`) encrypted-file fallback that only runs on Windows, used as a last resort
  when the keyring backend is unavailable.
- **Registry-derived paths.** Emulator modules read user roots from the registry / `%APPDATA%`
  / `OneDrive` Documents only inside `sys.platform == "win32"` blocks (for example
  `dolphin.py::_registry_user_root()`, `pcsx2.py::_windows_documents_folder()`).
- **Windows-only emulators.** Xenia (master), Xenia Canary, and the ShadPS4 Qt launcher are
  Windows-only builds; see the platform gate under Linux below.
- **Controller input.** TV mode polls XInput directly on Windows
  (`_XInputPollThread`, guide button via `XInputGetStateEx` ordinal 100).

---

## Linux

Supported and shipping. The bar was parity with Windows, not just "boots on Linux":
self-contained distribution, correct XDG paths in every emulator autoconfig path, working
cloud saves, secure token storage, gamepad input in TV mode, and native Linux emulators
detected/configured/launched correctly.

### What shipped

- **AppImage distribution.** `build.sh --appimage` (and `appimage-linux.yml`) run PyInstaller in
  `--onedir --windowed` mode, stage assets via
  `python scripts/stage_assets.py --platform linux`, assemble an `AppDir` from `appimage/`
  (`AppRun`, `.desktop`, rasterized icon), and package it with `appimagetool`. `build.sh` with
  no target (and `pyinstaller-linux.yml`) instead produces a plain `--onefile` binary. There is
  no PyInstaller spec file — every build script and workflow passes explicit flags.
- **Versioning and in-place updates.** `build.sh` derives the version from
  `git describe --tags --always --dirty` (leading `v` stripped), writes
  `grid_launcher/version.py`, and substitutes `%GRIDLAUNCHER_VERSION%` in the `.desktop` file.
  The AppImage is built with update information
  `gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync`, and
  the `.zsync` file is generated when `zsyncmake` is present (always in CI) and attached to
  the GitHub release. Local builds without `zsyncmake` still embed the update info.
- **XDG paths.** `core/path.py` provides `xdg_config_home()` and `xdg_data_home()`; the
  emulator modules that need them use them (`azahar`, `cemu`, `duckstation`, `eden`, `pcsx2`,
  `pico8`, `redream`, `retroarch`, `rpcs3`, `xemu`, `xenia`). `dolphin.py` resolves
  `~/.local/share/dolphin-emu` and `~/.dolphin-emu` directly. No unguarded `%APPDATA%`
  expansions remain.
- **RetroArch core extension.** `retroarch_core_argument_path()` in `launch.py` emits `.so` on
  Linux, `.dylib` on macOS, and `.dll` on Windows, stripping any pre-existing core extension
  first.
- **Cemu SDL controller profile.** `cemu.py` writes `_DEFAULT_CEMU_SDL_CONTROLLER_PROFILE`
  (`<api>SDLController</api>`) on non-Windows platforms and the XInput profile on Windows.
- **Wine/Proton dispatch for Windows-native games.** `prepare_native_launch_command()` in
  `launch.py` prepends `wine` when the compat tool is `wine`, otherwise runs the game through
  `umu-run` with `PROTONPATH` set to the selected Proton directory, creating and exporting
  `WINEPREFIX` when one is configured. Compat tools are offered by
  `EmulatorUIMixin._available_compat_tools_for_dialog()`: system `wine` from `shutil.which`,
  Proton builds discovered by `_scan_system_proton_installs()` under
  `~/.steam/steam/compatibilitytools.d`, `~/.local/share/Steam/compatibilitytools.d`, and
  `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d`, plus app-managed
  installs recorded in `compat_tool_installs`. A **default compat tool** is stored in config and
  auto-selected for a native game that has none, so Windows-native games launch through the
  default tool instead of being executed directly.
- **Wine prefix save paths.** `emulator/wine.py::translate_windows_path_to_wine_prefix()` maps
  `%APPDATA%`, `%LOCALAPPDATA%`, `%USERPROFILE%`, `%PROGRAMDATA%`, `%PUBLIC%`, and `%WINDIR%`
  into `<prefix>/drive_c/...`, so cloud save discovery for native games works under Wine
  (`cloud_transfer.py`).
- **Platform gating for Windows-only emulators.**
  `emulator/profiles.py::is_available_on_current_platform()` filters autoprofiles by
  `_WINDOWS_ONLY_EMULATOR_SLUGS` (`xenia canary (xbox 360)`, `xenia (xbox 360)`,
  `shadps4 qt launcher`) and by an explicit `source.platforms` allowlist. It is applied in
  `emulator_ui_mixin.py` (emulator list), `ui/dialogs.py` (add-emulator dropdown), and
  `install_mixin.py` (Xbox 360 content install, which refuses a Windows-only emulator with a
  message pointing at Xenia Edge). Covered by `tests/test_platform_gating.py`.
- **Xenia Edge.** Ships as a native Linux AppImage from the `has207/xenia-edge` release, so
  Xbox 360 remains playable on Linux; variant handling is in `emulator/xenia.py`.
- **Token storage.** `token_store.py` uses `keyring` on every platform (Secret Service /
  KWallet on Linux), migrating any legacy file-format secret into the keyring on first read.
  The Linux AppImage pins `keyring.backends.SecretService` and `keyring.backends.kwallet` as
  PyInstaller hidden imports. `requirements.txt` lists `keyring==25.6.0` unconditionally; no
  `secretstorage` pin was needed.
- **Frozen-build subprocess environment.** `core/process.py::clean_subprocess_env()` restores
  `LD_LIBRARY_PATH` from `LD_LIBRARY_PATH_ORIG` (or drops it) before spawning host binaries, so
  7z, tar, and emulators launched from the AppImage do not resolve against the bundle's private
  libraries and fail with loader errors such as `version 'CXXABI_1.3.15' not found`.
- **Controller input.** No Linux-specific work was required. `ControllerBackend.start()`
  dispatches to `_GamepadPollThread` (pygame/SDL) on non-Windows platforms; the guide button
  arrives as `BTN_MODE`. The AppImage is unsandboxed, so `/dev/input/event*` and
  `/dev/hidraw*` are reachable without portals.

### Still open

- **AppStream metadata.** No `metainfo.xml` is embedded in the `AppDir`, so the AppImage is not
  discoverable by software centres. Needs a stable reverse-DNS app ID; the icon already uses
  `io.github.Sixdd6.GRIDLauncher`.
- **First-run Linux setup experience.** `FirstRunSetupDialog` is generic; there is no
  Linux-specific guidance for the ROM library root, emulator locations, or compat tools.
- **System dependency documentation.** Runtime requirements are undocumented: a system `7z`
  (`p7zip` / `7zip` package) for archive extraction, `umu-launcher` for Proton dispatch,
  `libfuse2` for AppImage mounting, and membership of the `input` group or a udev rule on
  distributions that restrict `/dev/input`.
- **No CI test job.** All three workflows are release builds. There is no `tests.yml` running
  `python -m unittest discover tests/` on `ubuntu-latest` (or `windows-latest`), so
  platform-guard regressions are only caught locally.
- **MAME Linux config fallback.** `mame.py` resolves paths only from the launch template; it
  has no `~/.mame/mame.ini` or `$XDG_CONFIG_HOME/mame/mame.ini` fallback. Tracked alongside the
  MAME/FBNeo autoinstall work in `todo.md`.
- **Extended Wine/Proton configuration.** DXVK settings, arbitrary environment variables, and
  Bottles DBus integration are not implemented and are not planned unless there is demand.
- **Details-view messaging for gated platforms.** `install_mixin.py` already refuses Xbox 360
  content installs on non-Windows with a message pointing at Xenia Edge, but the details view
  has no equivalent block reason or disabled-with-tooltip state before the user clicks.

### Flatpak: dropped, not deferred

grid-launcher does not target Flatpak — neither for distributing the app nor for
auto-installing emulators. Linux distribution is via **AppImage**, and emulator auto-install
pulls **native or AppImage** builds only.

Rationale:

- **Fewer sandboxing issues.** An unsandboxed AppImage has direct access to the ROM library,
  user-chosen emulator paths, `/dev/input`, and the system keyring — no portal negotiation,
  D-Bus proxy policy, or `--filesystem=home` caveats.
- **Simpler pipeline.** One PyInstaller-based build (`build.sh --appimage`) replaces the
  Flatpak runtime/SDK/base-app stack and `flatpak-pip-generator` dependency conversion.

Consequences:

- **Auto-install** downloads native or AppImage emulator builds only. The Flatpak detection and
  install code paths were removed.
- **Manual configuration still works.** A user who prefers a Flatpak emulator can install it
  themselves and point grid-launcher at the Flatpak wrapper (or `~/.var/app/<id>/` config); the
  app just will not install or auto-detect it. The `~/.var/app/<id>/` entries that remain in the
  emulator path candidates exist for exactly this case.
- **Dolphin and MAME are not part of auto-install.** Both remain playable through their
  RetroArch cores (`dolphin_libretro`, `mame_libretro` / `mame2003_plus_libretro`).

---

## macOS

Untargeted. There is no macOS build script, no CI job, and no QA coverage. Nothing blocks it in
principle — the code base is already Qt/PySide6 and POSIX-aware, `keyring` supports Keychain,
and several emulator modules already carry `sys.platform == "darwin"` branches
(`launch.py`, `pico8.py`, `redream.py`, `retroarch.py`, `vita3k.py`, `xemu.py`, `xenia.py`,
plus Homebrew 7z paths in `archive_preparation.py`). What is missing is packaging (`.app`
bundle / notarization), a build script, CI, and the per-emulator path verification pass that
Linux received. Treat the existing `darwin` branches as untested.

---

## Reference

### XDG Base Directory quick reference

| Variable | Default | Used For |
|---|---|---|
| `$XDG_CONFIG_HOME` | `~/.config` | Emulator `.ini` / `.cfg` config files |
| `$XDG_DATA_HOME` | `~/.local/share` | Emulator data, save files, NAND images |
| `$XDG_CACHE_HOME` | `~/.cache` | Shader caches, thumbnails |
| `$XDG_STATE_HOME` | `~/.local/state` | Logs, recent files (rarely needed here) |

Always read the environment variable first and fall back to the default — use
`xdg_config_home()` / `xdg_data_home()` from `core/path.py` rather than hardcoding `~/.config`.

### Emulator platform matrix

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
| Xenia Edge | Xbox 360 | Yes (AppImage) | N/A | Emulator-directory relative | Emulator-directory relative | No |
| **Xenia** | **Xbox 360** | **No** | **N/A** | N/A | N/A | **Yes** |
| **Xenia Canary** | **Xbox 360** | **No** | **N/A** | N/A | N/A | **Yes** |
| **ShadPS4 (Qt launcher)** | **PS4** | **No** | **N/A** | N/A | N/A | **Yes** |

Notes:

- "Flatpak ID" is a reference for users who install a Flatpak emulator manually. These are
  **not** auto-installed or auto-detected; their `~/.var/app/<id>/` paths appear only as config
  candidates so autoconfig can still find a manually configured Flatpak emulator.
- Pico-8 is a commercial product. The Linux binary is user-supplied; the app only launches it.
- Eden / Azahar are forks that are not on Flathub; users download binaries directly.
- FinalBurn Neo's Linux availability refers to standalone builds. The libretro core via
  RetroArch is fully supported.

### Platform viability by console

| Platform | Windows Emulator | Linux Support | Notes |
|---|---|---|---|
| Xbox 360 | Xenia / Xenia Canary | Xenia Edge (native AppImage) | Xenia master/Canary hidden on Linux by the platform gate |
| Xbox OG | Xemu | Native | — |
| PS4 | FPKG extraction | Install pipeline is file-based | ShadPS4 Qt launcher is Windows-only |
| PS3 | RPCS3 | Native | — |
| Wii/GameCube | Dolphin | Native | Auto-install covers the RetroArch core, not standalone Dolphin |
| Switch | Eden | Native | — |
| 3DS | Azahar | Native | — |
| PS1 / PS2 / PSP | DuckStation / PCSX2 / PPSSPP | Native | — |
| Dreamcast | Redream | Native | — |
| Wii U | Cemu | Native (SDL controller profile) | — |
| Arcade | MAME / FBNeo | Native | Autoinstall + default config still open |
| Pico-8 | Pico-8 binary | User-supplied Linux binary | — |
| Native PC games (`.exe`) | Direct launch | Wine or Proton via `umu-run` | Default compat tool auto-selected |

### Testing conventions for platform-specific code

Tests use `unittest` (per AGENTS.md). Two patterns cover most platform work:

```python
from unittest.mock import patch

with patch("sys.platform", "linux"):
    result = retroarch_core_argument_path("snes9x")
    assert result == "cores/snes9x_libretro.so"
```

`sys.platform` is read at call time in the emulator modules, so patching it per-test is safe.
For XDG-dependent path candidates, set `XDG_CONFIG_HOME` / `XDG_DATA_HOME` /
`XDG_CACHE_HOME` with `unittest.mock.patch.dict(os.environ, ...)` pointed at a temporary
directory.

---

## Open questions

1. **App ID.** The AppStream metainfo still needs a confirmed reverse-DNS ID. The icon uses
   `io.github.Sixdd6.GRIDLauncher`; confirm before writing `metainfo.xml`.
2. **Eden legal status.** Eden is a yuzu fork with uncertain legal standing. The app does not
   bundle it. Decide whether the emulator dialog should carry a warning.
3. **Azahar config path.** `~/.config/azahar-emu/` follows the Qt app naming convention; verify
   against a real Azahar Linux install.
4. **Cemu SDL controller profile.** Verify `<api>SDLController</api>` and the SDL UUID against
   Cemu's actual XML schema on Linux hardware.
5. **Keyring availability.** Confirm a secret service (GNOME Keyring / KWallet) is running on
   target desktops. `token_store.py` refuses insecure storage on non-Windows, so a headless
   setup with no secret service cannot persist the token.
6. **RetroArch core path.** `retroarch_core_argument_path()` returns a relative `cores/` path;
   verify it resolves against the RetroArch working directory for a native install and for a
   manually configured Flatpak RetroArch.
