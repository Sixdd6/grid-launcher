# Linux Support — Phase 1 Implementation Plan

> **Source document**: `future-plans/linux-support.md`
> **Status**: Ready for implementation
> **Phase goal**: The app compiles, runs, and passes all tests on Linux. No feature regressions on Windows. Flatpak CI produces a `.flatpak` bundle.

---

## 1. Summary

Phase 1 adds the foundational plumbing for Linux support: XDG path helpers, per-emulator Linux path candidates, fixes for two unconditional `%APPDATA%` expansion bugs (`azahar.py`, `eden.py`), a cross-platform RetroArch core extension, Xenia platform-gating, a `ROM_MATE_SHARE_DIR` env-var escape hatch for Flatpak data files, `keyring` dependency declarations, a two-OS test CI matrix, and a Flatpak manifest with its own CI workflow. No emulator behavior changes on Windows. Every change is backward-compatible.

---

## 2. Files to Change

| File | Change | Reason |
|------|--------|--------|
| `rom_mate/core/path.py` | Add `xdg_config_home()` and `xdg_data_home()` helpers | Central XDG path resolution used by all emulator modules |
| `rom_mate/core/config.py` | Add `share_dir()` helper that checks `ROM_MATE_SHARE_DIR` env var, falls back to repo root | Flatpak: data files live at `/app/share/rom-mate-neo/`, not next to `__file__` |
| `rom_mate/emulator/azahar.py` | Guard `%APPDATA%` expansion on line 132 behind `sys.platform == "win32"`; add Linux `XDG_CONFIG_HOME/azahar-emu/` and `XDG_DATA_HOME/azahar-emu/` candidates | Crashes on Linux today (`%APPDATA%` unexpanded); add Linux paths |
| `rom_mate/emulator/eden.py` | Guard `%APPDATA%` expansion on line 206 behind `sys.platform == "win32"`; add Linux `XDG_CONFIG_HOME/eden/` candidates | Crashes on Linux today; add Linux config path |
| `rom_mate/emulator/dolphin.py` | Add `XDG_DATA_HOME/dolphin-emu/` and `~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/` candidates; verify line 233 `%APPDATA%` is guarded | Native + Flatpak Dolphin support |
| `rom_mate/emulator/pcsx2.py` | Add `XDG_CONFIG_HOME/PCSX2/inis/PCSX2.ini` and `~/.var/app/net.pcsx2.PCSX2/config/PCSX2/inis/PCSX2.ini` candidates | Native + Flatpak PCSX2 support |
| `rom_mate/emulator/duckstation.py` | Add `~/.var/app/org.duckstation.DuckStation/config/duckstation/` candidate | Flatpak DuckStation support |
| `rom_mate/emulator/xemu.py` | Add `~/.var/app/app.xemu.xemu/data/xemu/xemu/` candidate | Flatpak Xemu support |
| `rom_mate/emulator/cemu.py` | Guard `APPDATA`/`LOCALAPPDATA` lookup behind `sys.platform == "win32"`; add `XDG_CONFIG_HOME/Cemu/` and `~/.var/app/info.cemu.Cemu/config/Cemu/` candidates | Fix Linux crash; add Linux paths. (SDL controller profile deferred to Phase 2) |
| `rom_mate/emulator/ppsspp.py` | Add `~/.config/ppsspp/PSP/` and `~/.var/app/org.ppsspp.PPSSPP/config/ppsspp/PSP/` fallback candidates when no emulator path is configured | Linux PPSSPP support |
| `rom_mate/emulator/mame.py` | Add `~/.mame/mame.ini` and `XDG_CONFIG_HOME/mame/mame.ini` as fallback candidates | Linux MAME support |
| `rom_mate/emulator/pico8.py` | Add `~/.lexaloffle/pico-8/` and `XDG_DATA_HOME/pico-8/` fallback candidates | Linux Pico-8 support |
| `rom_mate/emulator/launch.py` | Rewrite `retroarch_core_argument_path()` with `_core_extension()` helper; strip any existing platform extension before normalising | `.so` on Linux, `.dylib` on macOS, `.dll` on Windows |
| `rom_mate/emulator/profiles.py` | Add `_WINDOWS_ONLY_EMULATOR_SLUGS = frozenset({"xenia", "xenia-canary"})` and `is_available_on_current_platform(emulator_slug: str) -> bool` | Centralised platform-gate for Xenia |
| `rom_mate/ui/mixins/emulator_ui_mixin.py` | Import `is_available_on_current_platform` and filter emulator list/picker at the build step | Hides Xenia on Linux |
| `rom_mate/ui/mixins/install_mixin.py` | At the point where platform is identified as Xbox 360, add `sys.platform != "win32"` early-exit with a user-facing message | Block Xbox 360 installs on Linux |
| `requirements.txt` | Add `keyring>=24.0.0 ; sys_platform != "win32"` and `secretstorage>=3.3.3 ; sys_platform == "linux"` | Linux keyring dependencies |
| `.github/workflows/tests.yml` | New file: `matrix.os: [windows-latest, ubuntu-latest]` test workflow | Linux CI test coverage |
| `flatpak/io.github.yourorg.rommateneoz.yml` | New file: Flatpak manifest (skeleton from Section 2.3 of research doc) | Flatpak packaging |
| `flatpak/rom-mate-neo.sh` | New file: launcher shell script that sets `ROM_MATE_SHARE_DIR` and execs Python | Flatpak entry point |
| `.github/workflows/flatpak-linux.yml` | New file: Flatpak build CI workflow (from Section 2.4 of research doc), triggered on release | Produce `.flatpak` artifact on release |
| `tests/test_emulator_path_candidates_linux.py` | New file: Linux path candidate tests with `sys.platform` patched to `"linux"` and XDG env mocked | New coverage — Linux paths |
| `tests/test_retroarch_config.py` | Add Linux and macOS extension assertions to `retroarch_core_argument_path` tests | Cover new `.so` / `.dylib` logic |
| `tests/test_emulator_autoconfig_settings.py` | Add Linux-platform variant tests for `cemu`, `ppsspp`, `mame`, `pico8`, `dolphin`, `pcsx2` path candidates | Cover new Linux candidates |

---

## 3. Ordered Steps

```text
1.  In rom_mate/core/path.py, add xdg_config_home() and xdg_data_home() helpers.
    - Read XDG_CONFIG_HOME / XDG_DATA_HOME env vars, fall back to ~/.config and
      ~/.local/share on non-Windows. Return None on sys.platform == "win32".
    - These are imported by emulator modules in subsequent steps.

2.  In rom_mate/core/config.py, add share_dir() -> Path helper.
    - Check os.environ.get("ROM_MATE_SHARE_DIR") first; if set, return Path(value).
    - Fall back to Path(__file__).resolve().parents[2] (repo root) so dev runs work unchanged.
    - Callers of retroarch_core_list_path() and dialogs.py's autoprofile path should use
      share_dir() as the base_path argument.

3.  In rom_mate/emulator/azahar.py, fix line 132:
    - Wrap Path(os.path.expandvars("%APPDATA%")) / "Azahar" / "qt-config.ini" inside
      if sys.platform == "win32": using os.environ.get("APPDATA", "") instead of expandvars.
    - In the else branch, add XDG_CONFIG_HOME/azahar-emu/qt-config.ini and
      XDG_DATA_HOME/azahar-emu/qt-config.ini using the helpers from step 1.

4.  In rom_mate/emulator/eden.py, fix line 206:
    - Wrap the windows_candidate assignment inside if sys.platform == "win32":
    - In the else branch (or after the existing else block at line 337), add
      XDG_CONFIG_HOME/eden/qt-config.ini using the xdg_config_home() helper.

5.  In rom_mate/emulator/dolphin.py:
    - Verify whether line 233 has an unguarded %APPDATA% expansion; if so, guard it.
    - In dolphin_user_root_candidates(), add to the non-Windows branch:
        xdg / "dolphin-emu"  (from xdg_data_home())
        ~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/
    - The ~/.dolphin-emu legacy path already exists; keep it after the XDG path.

6.  In rom_mate/emulator/pcsx2.py:
    - In the config path candidates function, add non-Windows candidates:
        XDG_CONFIG_HOME/PCSX2/inis/PCSX2.ini
        ~/.var/app/net.pcsx2.PCSX2/config/PCSX2/inis/PCSX2.ini

7.  In rom_mate/emulator/duckstation.py:
    - Append ~/.var/app/org.duckstation.DuckStation/config/duckstation/ to the existing
      Linux candidates.

8.  In rom_mate/emulator/xemu.py:
    - In the non-Windows branch, add ~/.var/app/app.xemu.xemu/data/xemu/xemu/.

9.  In rom_mate/emulator/cemu.py:
    - Guard the APPDATA / LOCALAPPDATA lookup behind sys.platform == "win32".
    - Add non-Windows candidates: XDG_CONFIG_HOME/Cemu/ and
      ~/.var/app/info.cemu.Cemu/config/Cemu/.
    - SDL controller profile (step deferred to Phase 2 — XInput profile is not written
      on non-Windows, so no crash, just incomplete support).

10. In rom_mate/emulator/ppsspp.py:
    - When emulator_path_text is empty and no --home is in the launch template,
      add ~/.config/ppsspp/PSP/ and
      ~/.var/app/org.ppsspp.PPSSPP/config/ppsspp/PSP/ as fallback candidates.

11. In rom_mate/emulator/mame.py:
    - Add fallback candidates when launch template overrides are absent:
        ~/.mame/mame.ini
        XDG_CONFIG_HOME/mame/mame.ini

12. In rom_mate/emulator/pico8.py:
    - Add fallback default home candidates:
        ~/.lexaloffle/pico-8/
        XDG_DATA_HOME/pico-8/

13. In rom_mate/emulator/launch.py, rewrite retroarch_core_argument_path():
    - Extract _core_extension() -> str that returns ".dll" / ".dylib" / ".so" based on sys.platform.
    - Strip any existing .dll / .so / .dylib suffix before normalising.
    - Compose core_file with the platform extension.
    - The function signature and the cores/ prefix are unchanged.

14. In rom_mate/emulator/profiles.py:
    - Add _WINDOWS_ONLY_EMULATOR_SLUGS = frozenset({"xenia", "xenia-canary"}).
    - Add is_available_on_current_platform(emulator_slug: str) -> bool that returns
      False when sys.platform != "win32" and the slug is in the frozenset.

15. In rom_mate/ui/mixins/emulator_ui_mixin.py:
    - Import is_available_on_current_platform from rom_mate.emulator.profiles.
    - At the point where the emulator list is built (currently line 89 area and wherever
      the emulator picker is populated), filter out slugs where
      is_available_on_current_platform(slug) is False.

16. In rom_mate/ui/mixins/install_mixin.py:
    - Identify the point where the platform is confirmed as Xbox 360.
    - Add: if sys.platform != "win32": show error message
      "Xbox 360 game installation requires Windows" and return early.

17. In requirements.txt:
    - Append: keyring>=24.0.0 ; sys_platform != "win32"
    - Append: secretstorage>=3.3.3 ; sys_platform == "linux"

18. Create flatpak/ directory with:
    - flatpak/rom-mate-neo.sh  (launcher script: sets ROM_MATE_SHARE_DIR, execs python3)
    - flatpak/io.github.yourorg.rommateneoz.yml  (manifest skeleton from research doc §2.3)
      NOTE: Replace "yourorg" placeholder with actual org name before submission.

19. Create .github/workflows/tests.yml:
    - Matrix: os: [windows-latest, ubuntu-latest], python-version: ["3.12"]
    - Steps: checkout, setup-python, pip install -r requirements.txt pytest, pytest tests/ -v

20. Create .github/workflows/flatpak-linux.yml:
    - Trigger: on release created
    - Uses flatpak/flatpak-github-actions/flatpak-builder@v6
    - Uploads rom-mate-neo.flatpak as release asset

21. In tests/test_retroarch_config.py:
    - Add test_core_argument_path_linux_so: patch sys.platform "linux", assert "snes9x_libretro.so"
    - Add test_core_argument_path_macos_dylib: patch sys.platform "darwin", assert ".dylib"
    - Existing Windows tests must still pass (no change to Windows behaviour).

22. In tests/test_emulator_autoconfig_settings.py:
    - Add Linux-platform variant test methods for cemu, ppsspp, mame, pico8, dolphin, and pcsx2
      path candidate functions. Use patch("sys.platform", "linux") and
      patch.dict(os.environ, {"XDG_CONFIG_HOME": ..., "XDG_DATA_HOME": ...}).

23. Create tests/test_emulator_path_candidates_linux.py:
    - Tests for azahar, eden, dolphin, pcsx2, duckstation, xemu, cemu Linux candidates.
    - Use unittest.TestCase. Mock sys.platform via unittest.mock.patch.
    - Mock XDG env vars via unittest.mock.patch.dict(os.environ, {...}).
    - Assert XDG path IS in candidates on Linux and IS NOT the literal string "%APPDATA%".
    - Assert %APPDATA%-derived paths are NOT in candidates on Linux.
```

---

## 4. Testing Impact

| Test File | Status | New Cases |
|-----------|--------|-----------|
| `tests/test_retroarch_config.py` | Modify | `test_core_argument_path_linux_so`, `test_core_argument_path_macos_dylib` |
| `tests/test_emulator_autoconfig_settings.py` | Modify | Linux-platform variants for cemu, ppsspp, mame, pico8, dolphin, pcsx2 candidates |
| `tests/test_emulator_path_candidates_linux.py` | **New** | Full Linux path candidate coverage for azahar, eden, dolphin, pcsx2, duckstation, xemu, cemu |
| `tests/test_emulator_source.py` | Modify if applicable | `is_available_on_current_platform` with xenia slug on Linux returns False |

**Patch namespaces**:
- `is_available_on_current_platform`: patch at `rom_mate.emulator.profiles.is_available_on_current_platform` (it is called from the mixin via an import, not re-exported through `__init__`).
- `sys.platform` patches: use `unittest.mock.patch("sys.platform", "linux")` as a context manager at the function level (safe because none of the emulator modules cache `sys.platform` at import time).
- XDG env vars: use `unittest.mock.patch.dict(os.environ, {...})` — do **not** use `monkeypatch` (pytest fixture, incompatible with `unittest.TestCase`).

---

## 5. Risks & Edge Cases

1. **`dolphin.py` line 233 unverified**: The grep found `Path(os.path.expandvars("%APPDATA%")) / "Dolphin Emulator" / "Config" / ini_name` at line 233. This may already be inside a guarded block (the `dolphin_user_root_candidates` function is guarded). The Coder must read that function's full context before editing. If line 233 is NOT guarded, it is a bug to fix in step 5.

2. **`azahar.py` config path verification**: The Linux config path `~/.config/azahar-emu/` is based on the Qt app naming convention. This is an open question in the research doc — verify against an actual Azahar Linux install before writing the final test assertions. For Phase 1, add the candidate unconditionally and document the uncertainty.

3. **Cemu SDL controller profile (deferred)**: The XInput controller profile XML is written for Windows. On Linux, the correct format is `<api>SDLController</api>` — the exact XML schema needs verification. In Phase 1 the autoconfig for Cemu simply omits the controller XML write on non-Windows (no crash, incomplete feature). Phase 2 will implement the SDL profile.

4. **`retroarch_core_argument_path` existing tests**: Existing tests assert `_libretro.dll` extensions. After step 13, those tests will still pass on Windows (or wherever `sys.platform` is not patched). The Coder must not change the function's behavior when `sys.platform == "win32"`.

5. **Flatpak `yourorg` placeholder**: The manifest uses `io.github.yourorg.rommateneoz` as a placeholder app ID. Open Question #1 from the research doc (app ID / domain ownership) must be resolved before this manifest can be submitted to Flathub. For Phase 1, use the placeholder — it still produces a valid local `.flatpak` bundle.

6. **`share_dir()` callers**: The `ROM_MATE_SHARE_DIR` helper affects `retroarch_core_list_path()` (called from `retroarch.py`) and the autoprofile path in `rom_mate/ui/dialogs.py` line 29. Both must be updated to use `share_dir()` as the base. Do not change existing test fixtures that already resolve the path relative to the repo root — they use `Path(__file__).resolve().parent.parent` directly and bypass the env var path entirely, which is correct for tests.

7. **`secretstorage` Flatpak availability**: `secretstorage` must be added to the `flatpak-pip-generator` input list alongside `keyring`. The `--talk-name=org.freedesktop.secrets` finish-arg is already in the manifest skeleton. Do not skip this or the keyring will fail silently inside the Flatpak sandbox.

8. **Non-overwrite invariant**: Steps 3–12 (emulator path candidate functions) must follow the emulator-autoconfig skill: `ensure_*` functions are non-destructive (they only add missing keys). Path candidate changes do not affect this invariant directly, but any new `ensure_*` logic added in Phase 2 must follow it.
