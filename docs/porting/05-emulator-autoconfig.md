# 05 — Automatic emulator configuration

## Purpose

This document describes how GRID Launcher writes emulator configuration files on
the user's behalf, and how it derives save/state directories from those files.

It covers:

- the two distinct layers that share the name "autoconfig":
  1. **entry autoconfig** — building/updating the launcher's own emulator entries and
     platform defaults from a shipped autoprofile
     (grid_launcher/emulator/autoconfig.py:472),
  2. **settings sync** — writing native emulator config files via `ensure_*` functions,
     dispatched from `_ensure_emulator_sync_settings`
     (grid_launcher/ui/mixins/emulator_ui_mixin.py:365);
- per-emulator config file locations, formats, and the exact keys written;
- portable-mode detection per emulator;
- the `*_directory_settings` / `*_save_path_overrides` / `*_state_path_overrides`
  readers that turn config files into concrete save paths;
- RetroAchievements credential wiring;
- RetroArch core metadata files and installed-core discovery.

Out of scope (cross-references):

- Autoprofile *normalization* rules (field-by-field coercion) — doc 02 and doc 04
  (grid_launcher/emulator/profiles.py:427).
- Where autoprofile/core-list JSON files are located on disk (`GRID_LAUNCHER_SHARE_DIR`,
  Flatpak, PyInstaller) — doc 02.
- Downloading and extracting emulator archives, including RetroArch's
  `supplemental_downloads` core bundle — doc 04.
- How the resolved save/state paths are turned into upload/download operations — doc 06.
- Launch-argument templating (`%rom%`, `%core%`) — doc 04.

## External surfaces

### Config files written by `ensure_*` functions

One row per file that an `ensure_*` function may create or modify.

| Emulator | File written | Format | Keys written | Anchor |
|---|---|---|---|---|
| RetroArch | `<selected>/retroarch.cfg` | flat `key = "value"` | `savefile_directory`, `savestate_directory`, `video_windowed_fullscreen`, `audio_volume`, `discord_enable`, `pause_nonactive`, `video_vsync`, `input_menu_toggle_gamepad_combo`, `savestate_auto_save`, `savestate_auto_load`, `rgui_show_start_screen`, `menu_show_core_updater`, `sort_savefiles_enable`, `sort_savestates_enable`, `sort_savefiles_by_content_enable`, `sort_savestates_by_content_enable`, `savefiles_in_content_dir`, `savestates_in_content_dir`, `cheevos_hardcore_mode_enable`, `cheevos_visibility_lboard_start`, `cheevos_visibility_lboard_submit`, `cheevos_visibility_lboard_trackers`; conditionally `netplay_nickname`, `video_fullscreen`, `cheevos_enable`/`cheevos_username`/`cheevos_token` | grid_launcher/emulator/retroarch.py:258, grid_launcher/emulator/retroarch.py:287, grid_launcher/emulator/retroarch.py:291, grid_launcher/emulator/retroarch.py:294 |
| DuckStation | `<emulator_dir>/settings.ini` | INI, `key = value` (spaces) | `[MemoryCards]` Directory/Card1Type/Card2Type/UsePlaylistTitle; `[Main]` InhibitScreensaver/SetupWizardIncomplete/ConfirmPowerOff/StartFullscreen; `[Display]` FullscreenMode/Scaling/Scaling24Bit; `[AutoUpdater]` CheckAtStartup; `[GPU]` 9 keys; `[Audio]` OutputVolume; `[Hotkeys]` OpenPauseMenu; `[Pad1]` 26 keys; `[Cheevos]` Enabled/ChallengeMode/LeaderboardNotifications/LeaderboardTrackers | grid_launcher/emulator/duckstation.py:222, grid_launcher/emulator/duckstation.py:235, grid_launcher/emulator/duckstation.py:252, grid_launcher/emulator/duckstation.py:263, grid_launcher/emulator/duckstation.py:275, grid_launcher/emulator/duckstation.py:283, grid_launcher/emulator/duckstation.py:363 |
| DuckStation | `<emulator_dir>/portable.txt` | empty file | — | grid_launcher/emulator/duckstation.py:207 |
| PCSX2 | `<emulator_dir>/inis/PCSX2.ini` | INI, `key = value` | `[UI]`, `[AutoUpdater]`, `[EmuCore]`, `[EmuCore/GS]`, `[EmuCore/Speedhacks]`, `[Pad1]`, `[Hotkeys]`, `[SPU2/Output]`, `[Achievements]`, `[Folders]` — see per-emulator section | grid_launcher/emulator/pcsx2.py:194, grid_launcher/emulator/pcsx2.py:200 |
| PCSX2 | `<emulator_dir>/portable.ini` | empty file | — | grid_launcher/emulator/pcsx2.py:187 |
| Dolphin | `<selected>/Dolphin.ini` | INI, `key = value` | `[Analytics]` Enabled/PermissionAsked; `[Display]` Fullscreen/RenderToMain; `[General]` ShowLaunchWarning; `[DSP]` Volume | grid_launcher/emulator/dolphin.py:277 |
| Dolphin | `<selected>/GFX.ini` | INI | `[Settings]` UseVerticalSync | grid_launcher/emulator/dolphin.py:302 |
| Dolphin | `<selected>/Dolphin.ini` (separate call) | INI | `[Core]` SkipIPL = False | grid_launcher/emulator/dolphin.py:328 |
| Dolphin | `<selected>/GCPadNew.ini` | INI block append | whole `[GCPad1]` block | grid_launcher/emulator/dolphin.py:340, grid_launcher/emulator/dolphin.py:397 |
| Dolphin | `<emulator_dir>/portable.txt` | empty file | — | grid_launcher/emulator/dolphin.py:257 |
| Azahar | `<selected>/qt-config.ini` | Qt INI, `key = value` plus `key\default` companions | `[Renderer]` resolution_factor/use_vsync; `[Audio]` volume; `[UI]` discord/confirmClose/fullscreen/pauseWhenInBackground/hideInactiveMouse/two shortcut keys | grid_launcher/emulator/azahar.py:165, grid_launcher/emulator/azahar.py:177, grid_launcher/emulator/azahar.py:187 |
| Azahar | `<emulator_dir>/user/` | directory | — | grid_launcher/emulator/azahar.py:146 |
| Eden | `<selected>/qt-config.ini` | Qt INI with generated `key\default=false` lines | `[UI]` 8 keys; `[WebService]` enable_telemetry; `[Audio]` volume/muteWhenInBackground; `[Renderer]` scaling_filter | grid_launcher/emulator/eden.py:237, grid_launcher/emulator/eden.py:253, grid_launcher/emulator/eden.py:260, grid_launcher/emulator/eden.py:267 |
| Eden | `<emulator_dir>/user/` | directory | — | grid_launcher/emulator/eden.py:220 |
| RPCS3 | `<emulator_dir>/portable/config/config.yml` | YAML (2-space indented sections) | `Miscellaneous: Start games in fullscreen mode`, `Audio: Master Volume` | grid_launcher/emulator/rpcs3.py:558 |
| RPCS3 | `<emulator_dir>/portable/GuiConfigs/GuiSettings.ini` | Qt-annotated INI (`key\default=false` then `key = value`) | `[main_window]` infoBoxEnabledWelcome, confirmationBoxExitGame, confirmationBoxBootGame, infoBoxEnabledInstallPUP | grid_launcher/emulator/rpcs3.py:565 |
| RPCS3 | `<emulator_dir>/portable/GuiConfigs/CurrentSettings.ini` | plain INI, `key=value`, no annotations | `[Meta]` checkUpdateStart, useRichPresence; `[main_window]` same four keys | grid_launcher/emulator/rpcs3.py:576, grid_launcher/emulator/rpcs3.py:580 |
| RPCS3 | `<emulator_dir>/portable/config/vfs.yml` | quoted YAML scalars | `"$(EmulatorDir)"`, `"/dev_hdd0/"`, `"/games/"` | grid_launcher/emulator/rpcs3.py:419, grid_launcher/emulator/rpcs3.py:449 |
| RPCS3 | `<data_root>/config/games.yml` | YAML scalar per game id | `<GAMEID>: "<abs dir>/"` | grid_launcher/emulator/rpcs3.py:329, grid_launcher/emulator/rpcs3.py:337 |
| PPSSPP | `<emulator_dir>/memstick/PSP/SYSTEM/PPSSPP.INI` | INI, `key = value` | `[General]`, `[Graphics]`, `[Sound]`, `[Theme]`, and conditionally `[Achievements]` | grid_launcher/emulator/ppsspp.py:97, grid_launcher/emulator/ppsspp.py:100 |
| PPSSPP | `<emulator_dir>/memstick/PSP/SYSTEM/ppsspp_retroachievements.dat` | plain text (token) | — | grid_launcher/emulator/ppsspp.py:156 |
| PPSSPP | `<emulator_dir>/installed.txt` | **deleted** | — | grid_launcher/emulator/ppsspp.py:87 |
| Cemu | `<emulator_dir>/portable/settings.xml` | XML | `use_discord_presence`, `check_update`, `receive_untested_updates`, `gp_download`, `fullscreen`, `window_maximized` (whole file templated on create) | grid_launcher/emulator/cemu.py:299, grid_launcher/emulator/cemu.py:314 |
| Cemu | `<emulator_dir>/portable/controllerProfiles/controller0.xml` | XML | whole file, XInput or SDL variant | grid_launcher/emulator/cemu.py:341, grid_launcher/emulator/cemu.py:346 |
| Xemu | `<emulator_dir>/xemu.toml` | TOML | `[general] show_welcome`; `[misc] check_for_updates`; `[display] vsync`; `[display.window] fullscreen_on_startup`; `[display.quality] surface_scale`; `[audio] volume_limit`; `[input.bindings] port1_driver`; `[sys.files]` bootrom/flashrom/hdd/eeprom paths | grid_launcher/emulator/xemu.py:250, grid_launcher/emulator/xemu.py:257, grid_launcher/emulator/xemu.py:306 |
| Redream | `<data_root>/redream.cfg` | flat `key=value`, no spaces | `mode=fullscreen`, `volume=40` | grid_launcher/emulator/redream.py:161, grid_launcher/emulator/redream.py:190 |
| Xenia | *(none)* | — | `apply_xenia_content_without_ui` copies STFS packages into the content tree instead | grid_launcher/emulator/xenia.py:70 |
| Vita3K, FBNeo, MAME, Pico-8 | *(none)* | — | read-only modules; no `ensure_*` function exists | grid_launcher/emulator/vita3k.py:9, grid_launcher/emulator/fbneo.py:80, grid_launcher/emulator/mame.py:172, grid_launcher/emulator/pico8.py:174 |

### Shipped data files read

| File | Purpose | Anchor |
|---|---|---|
| `emulator-autoprofiles.json` | 21 profiles: match tokens, arg templates, save/state directory hints, source metadata | grid_launcher/emulator/profiles.py:409, grid_launcher/emulator/profiles.py:413 |
| `retroarch-core-list.json` | 233 entries: core file → platforms, capability flags, firmware/config/saves metadata | grid_launcher/emulator/retroarch.py:14, grid_launcher/emulator/retroarch.py:367 |
| `romm-platform-cores.json` | RomM platform slug → ordered core-id list | grid_launcher/emulator/retroarch.py:18, grid_launcher/emulator/retroarch.py:22 |

All three resolve under `grid_launcher_share_dir(...)` (grid_launcher/core/path.py:47,
grid_launcher/ui/mixins/emulator_ui_mixin.py:446, grid_launcher/ui/mixins/emulator_ui_mixin.py:469).

### Environment variables read by this subsystem

| Variable | Used by | Anchor |
|---|---|---|
| `RPCS3_CONFIG_DIR` | RPCS3 data-root candidate list (inserted at index 1) | grid_launcher/emulator/rpcs3.py:618 |
| `XDG_CONFIG_HOME` | RPCS3, Eden, Cemu, DuckStation, RetroArch, PCSX2 candidates | grid_launcher/core/path.py:33, grid_launcher/emulator/pcsx2.py:476 |
| `XDG_DATA_HOME` | Azahar, Eden, Pico-8, Xemu, Xenia, Redream, DuckStation, RetroArch | grid_launcher/core/path.py:40, grid_launcher/emulator/azahar.py:243, grid_launcher/emulator/xemu.py:130 |
| `APPDATA` | Azahar, Eden, Dolphin, Cemu, Xemu, Pico-8 Windows candidates | grid_launcher/emulator/azahar.py:133, grid_launcher/emulator/eden.py:209, grid_launcher/emulator/xemu.py:122 |
| `LOCALAPPDATA` | DuckStation, Cemu Windows candidates | grid_launcher/emulator/duckstation.py:21, grid_launcher/emulator/cemu.py:264 |
| `OneDrive`, `USERPROFILE` | Dolphin and PCSX2 Documents candidates | grid_launcher/emulator/dolphin.py:126, grid_launcher/emulator/pcsx2.py:459 |
| `HOME` | PCSX2 Documents candidate | grid_launcher/emulator/pcsx2.py:462 |

### Platform APIs

- Windows Shell `SHGetKnownFolderPath(FOLDERID_Documents)` via `ctypes`, used to resolve
  the redirected Documents folder for PCSX2 (grid_launcher/emulator/pcsx2.py:10,
  grid_launcher/emulator/pcsx2.py:37). Returns `None` on non-Windows
  (grid_launcher/emulator/pcsx2.py:17).
- Windows registry `HKCU\Software\Dolphin Emulator`, values `LocalUserConfig` and
  `UserConfigPath`, used to locate the Dolphin user root
  (grid_launcher/emulator/dolphin.py:80, grid_launcher/emulator/dolphin.py:89).

### Processes spawned

`trigger_rpcs3_firmware_install(exe, pup)` spawns `[<rpcs3>, "--installfw", <PS3UPDAT.PUP>]`
with cwd = exe's parent and a cleaned environment
(grid_launcher/emulator/rpcs3.py:379). Both paths must exist and be files
(grid_launcher/emulator/rpcs3.py:374). Python never waits on this child, leaving a zombie
process until the whole app exits. **Rust port (milestone 8):** `spawn_rpcs3_installfw`
(canonicalizes both paths, same argv/cwd/cleaned-environment shape) reaps the child on a
detached thread instead — see doc 04's D10/"Rulings on open questions".

## Data model

### Autoprofile record (`emulator-autoprofiles.json`)

Normalized by `normalize_emulator_autoprofiles` (grid_launcher/emulator/profiles.py:427).
Full field-by-field normalization is documented in doc 02/doc 04; the fields that drive
*this* document's behavior are:

| Field | Type | Role in autoconfig | Anchor |
|---|---|---|---|
| `match_tokens` | list of strings, lowercased | Matches an executable/entry to a profile. An entry with no tokens is dropped unless `is_compat_tool` is true | grid_launcher/emulator/profiles.py:440, grid_launcher/emulator/profiles.py:449 |
| `name` | string | Becomes the emulator entry's `name`; blank profile is skipped | grid_launcher/emulator/profiles.py:452, grid_launcher/emulator/autoconfig.py:489 |
| `args` | string, default `%rom%` | Becomes the entry's `args` | grid_launcher/emulator/autoconfig.py:494 |
| `all_platforms` | bool | If true, the emulator is assigned as default for every assignable server platform | grid_launcher/emulator/autoconfig.py:363 |
| `platform_keywords` | list of strings | If `all_platforms` is false, keyword-matched platforms receive the default | grid_launcher/emulator/autoconfig.py:373 |
| `save_strategy` | string | Copied to the entry when the entry's own value normalizes to `auto` | grid_launcher/emulator/autoconfig.py:497, grid_launcher/emulator/autoconfig.py:252 |
| `save_directories` | list of strings | Joined with `";\n"` into the entry's `save_paths` | grid_launcher/emulator/autoconfig.py:106, grid_launcher/emulator/autoconfig.py:500 |
| `state_directories` | list of strings | Joined into the entry's `state_paths` | grid_launcher/emulator/autoconfig.py:501 |
| `ignore_files`, `ignore_extensions` | lists | Joined into entry fields of the same names | grid_launcher/emulator/autoconfig.py:498 |
| `firmware_directories` | list of strings or dicts | Consumed by firmware install, including the PCSX2 BIOS directory hand-off | grid_launcher/emulator/profiles.py:524, grid_launcher/ui/mixins/emulator_ui_mixin.py:405 |
| `screenshot_directories` | list of strings | Passed through normalization only | grid_launcher/emulator/profiles.py:516 |
| `source`, `is_compat_tool`, `compat_tool_type` | dict / bool / string | Acquisition metadata — doc 04 | grid_launcher/emulator/profiles.py:534, grid_launcher/emulator/profiles.py:539 |

Multi-value profile fields are flattened with `";\n".join(...)` after stripping blanks
(grid_launcher/emulator/autoconfig.py:106).

### RetroArch core-list entry (`retroarch-core-list.json`)

A JSON array. Each element:

| Field | Type | Meaning | Anchor |
|---|---|---|---|
| `core_file` | string | e.g. `flycast_libretro.dll`; the core id is derived by stripping `.dll`/`.so`/`.dylib` then the `_libretro` suffix, lowercased | grid_launcher/emulator/retroarch.py:104 |
| `platforms` | list of strings | Display names normalized into compatibility keys | grid_launcher/emulator/retroarch.py:394 |
| `supports_save_states` | bool, default `true` | Capability flag | grid_launcher/emulator/retroarch.py:587 |
| `supports_saves` | bool, default `true` | Capability flag | grid_launcher/emulator/retroarch.py:588 |
| `cloud_sync_safe` | bool, default `true` | Capability flag | grid_launcher/emulator/retroarch.py:589 |
| `vmu_shared_saves` | bool, default `false` | Marks Flycast-style shared VMU saves | grid_launcher/emulator/retroarch.py:590 |
| `firmware` | dict: `needs_bios`, `subdirectory` (may be `null`), `files[]`, `extract_with_paths` | Where BIOS files go under the system dir | grid_launcher/emulator/retroarch.py:529, grid_launcher/ui/mixins/install_mixin.py:564 |
| `config_files` | dict: `base_dir`, `files[]` | Core option files relative to the emulator directory | grid_launcher/emulator/retroarch.py:547, grid_launcher/ui/mixins/install_mixin.py:582 |
| `saves_files` | dict: `file` | Single archive dropped into the resolved savefile directory | grid_launcher/emulator/retroarch.py:564, grid_launcher/ui/mixins/install_mixin.py:601 |

The loader accepts **two formats** for this file: a JSON array as above, or — when JSON
parsing fails — a Markdown table where column 1 is the core name and column 2 is the
system name (grid_launcher/emulator/retroarch.py:378, grid_launcher/emulator/retroarch.py:409).
Rows not starting with `|`, header rows (`core`), separator rows (`:`), and `-` cells are
skipped (grid_launcher/emulator/retroarch.py:420).

### RomM slug → core map (`romm-platform-cores.json`)

Flat object: `{"<romm-slug>": ["<core_id>", ...]}`. Non-string slugs, blank slugs, and
non-list values are dropped; non-string core entries inside a list are dropped
(grid_launcher/emulator/retroarch.py:33). Lookup is exact on the trimmed slug
(grid_launcher/emulator/retroarch.py:46).

### `*_directory_settings` result shapes

Each reader returns a flat `str → str` (or `str → object`) dict with a fixed key set and
always-present defaults, so callers never need existence checks.

| Function | Keys | Anchor |
|---|---|---|
| `retroarch_directory_settings` | config_path, savefile_directory, savestate_directory, plus 6 booleans | grid_launcher/emulator/retroarch.py:174 |
| `duckstation_memory_card_settings` | config_path, directory, card1_type, card2_type, use_playlist_title | grid_launcher/emulator/duckstation.py:145 |
| `pcsx2_directory_settings` | config_path, data_root, memory_cards, savestates, slot1_filename, slot2_filename | grid_launcher/emulator/pcsx2.py:537 |
| `dolphin_directory_settings` | config_path, user_root, gc_root, wii_root, state_saves, memcard_a/b_path, gci_folder_a/b_path, gci_folder_a/b_override | grid_launcher/emulator/dolphin.py:435 |
| `azahar_directory_settings` / `eden_directory_settings` | config_path, user_root, nand_root, sdmc_root, states_root, use_custom_storage, use_virtual_sd | grid_launcher/emulator/azahar.py:280, grid_launcher/emulator/eden.py:402 |
| `rpcs3_directory_settings` | config_path, persistent_settings_path, data_root, dev_hdd0, current_user | grid_launcher/emulator/rpcs3.py:666 |
| `cemu_directory_settings` | config_path, mlc_path | grid_launcher/emulator/cemu.py:362 |
| `xemu_directory_settings` | config_path, base_path, hdd_path, eeprom_path | grid_launcher/emulator/xemu.py:400 |
| `xenia_directory_settings` | variant, config_path, storage_root, content_root, cache_root, portable | grid_launcher/emulator/xenia.py:371 |
| `redream_directory_settings` | config_path, data_root, portable | grid_launcher/emulator/redream.py:89 |
| `fbneo_directory_settings` | config_path, base_path, eeprom_path, memcard_path, hiscore_path, hdd_path, state_path | grid_launcher/emulator/fbneo.py:92 |
| `mame_directory_settings` | ini_path, base_path, cfg/nvram/memcard/diff/state_directory | grid_launcher/emulator/mame.py:182 |
| `pico8_directory_settings` | config_path, user_root, carts_root, cdata_root, cstore_root, backup_root, desktop_path | grid_launcher/emulator/pico8.py:182 |

## Behavior

### Layer 1 — entry autoconfig (`auto_configure_emulator_settings`)

Runs when a downloaded/selected executable is matched to an autoprofile
(grid-launcher.py:3663). Inputs: the game record, the executable path, the matched
profile, the current emulator list, and the current default/core-default maps
(grid_launcher/emulator/autoconfig.py:472).

1. Resolve the target entry name from `profile["name"]`, defaulting to `"Emulator"`
   (grid_launcher/emulator/autoconfig.py:489). If the name case-folds to `dolphin`,
   a variant suffix is appended: `Dolphin (<variant>)`
   (grid_launcher/emulator/autoconfig.py:90, grid_launcher/emulator/autoconfig.py:103).
2. Precompute profile-derived values: `args_template` (blank → `%rom%`), normalized
   save strategy, and the four multi-line path/ignore strings
   (grid_launcher/emulator/autoconfig.py:494).
3. Find an existing entry whose name case-folds equal
   (grid_launcher/emulator/autoconfig.py:503).
   - **Existing entry** — `name` and `path` are always overwritten. `args` is replaced
     only when the emulator is RetroArch, or the current args are blank or exactly
     `%rom%` (grid_launcher/emulator/autoconfig.py:518). Every other field
     (`save_strategy`, `ignore_files`, `ignore_extensions`, `save_paths`, `state_paths`)
     keeps its non-blank current value and otherwise takes the profile value
     (grid_launcher/emulator/autoconfig.py:524).
   - **No existing entry** — a fresh entry is appended with all profile values
     (grid_launcher/emulator/autoconfig.py:555).
4. Assign platform defaults via `assign_profile_platform_defaults`
   (grid_launcher/emulator/autoconfig.py:568).

`assign_profile_platform_defaults` (grid_launcher/emulator/autoconfig.py:346):

- Target platforms: all assignable server platforms when `all_platforms` is true;
  for RetroArch this is further filtered to platforms that have at least one
  **installed** core (grid_launcher/emulator/autoconfig.py:365). Otherwise the
  platforms matched by `platform_keywords`
  (grid_launcher/emulator/autoconfig.py:373). For a profile literally named `dolphin`
  with a game in hand, variant-specific platforms replace the keyword match
  (grid_launcher/emulator/autoconfig.py:376).
- Per platform: an empty current default is filled; a non-empty default is replaced
  **only** when the incoming emulator is not RetroArch and the current default is
  RetroArch — i.e. a native emulator outranks RetroArch, never the reverse
  (grid_launcher/emulator/autoconfig.py:382).
- For RetroArch platforms whose default now points at this emulator and that have no
  core default yet, the first installed compatible core is recorded
  (grid_launcher/emulator/autoconfig.py:391). **Rust port (D-RC-2):** the
  installed-core filter resolves candidates SLUG-FIRST —
  `autoconfig::installed_compatible_cores` uses the bundled
  `romm-platform-cores.json` list for a known server slug and falls back to the fuzzy
  `cores_for_platform` match on a slug miss or an empty slug. Slugs reach grid-core
  through `SyncContext::platform_slugs`, recorded by `list_platforms` on the install
  service alongside `set_known_platforms`.

`_backfill_missing_emulator_defaults` re-runs the same assignment for every registered
emulator whenever the emulator views refresh, and saves only if something changed
(grid_launcher/ui/mixins/emulator_ui_mixin.py:1790,
grid_launcher/ui/mixins/emulator_ui_mixin.py:1838). It is connected to
`_emulator_refresh_requested` (grid-launcher.py:463).

Manual (hand-typed) entries take a related but separate path:
`apply_manual_emulator_profile_defaults` fills only blank fields and never touches
`path` (grid_launcher/emulator/autoconfig.py:228), with the profile field mapping
`ignore_files→ignore_files`, `ignore_extensions→ignore_extensions`,
`save_paths→save_directories`, `state_paths→state_directories`
(grid_launcher/emulator/autoconfig.py:258).

### Layer 2 — settings-sync orchestration (`_ensure_emulator_sync_settings`)

Signature: `(emulator_name, emulator_path_text) -> None`
(grid_launcher/ui/mixins/emulator_ui_mixin.py:365).

Order of operations:

1. Read the RomM `username` from config (used only for RetroArch's netplay nickname)
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:373).
2. Trim the path; **return immediately if blank**
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:375).
3. Idempotency gate: build the cache key `f"{emulator_name}::{path_text}"` and return if
   it is already in the session set `_emulator_sync_settings_done`
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:379). The set is created empty at
   startup (grid-launcher.py:431) and cleared on every config save
   (grid-launcher.py:3150), because a save may have changed emulator or library paths.
4. Read RetroAchievements `username` and `token` from config
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:386).
5. Dispatch — a **flat sequence of independent `if` checks**, not a chain, so an emulator
   name matching two predicates runs both writers
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:388 through
   grid_launcher/ui/mixins/emulator_ui_mixin.py:439):

   | Predicate | Call | Extra arguments |
   |---|---|---|
   | retroarch | `ensure_retroarch_save_location_settings` | `enable_fullscreen=True`, RA creds, `username=<romm user>` |
   | duckstation | `ensure_duckstation_memory_card_settings` | `enable_fullscreen=True` |
   | xemu | `ensure_xemu_settings` | — |
   | pcsx2 | `ensure_pcsx2_settings` | `enable_fullscreen=True`, RA creds, `bios_directory=<first resolved firmware dir>` |
   | dolphin | `ensure_dolphin_settings` | — |
   | azahar | `ensure_azahar_settings` | — |
   | eden | `ensure_eden_settings` | — |
   | rpcs3 | `ensure_rpcs3_settings` + `_trigger_rpcs3_firmware_download_background` | `ps3_library_path=<library>/PlayStation 3` |
   | ppsspp | `ensure_ppsspp_settings` | RA creds |
   | cemu | `ensure_cemu_settings` + `ensure_cemu_controller_config` | — |
   | redream | `ensure_redream_settings` | — |

6. Record the cache key (grid_launcher/ui/mixins/emulator_ui_mixin.py:440).

The PCSX2 BIOS directory is the **first** entry of `_resolved_firmware_directories`,
unwrapping `(path, files)` tuples to just the path
(grid_launcher/ui/mixins/emulator_ui_mixin.py:404).

The RPCS3 PS3 library path is `<library_path>/PlayStation 3`, or empty when no library
path is configured (grid_launcher/ui/mixins/emulator_ui_mixin.py:424).

**Emulator identification.** Every `_is_*_emulator_name(name, entry)` predicate delegates
to `_emulator_matches_tokens` (grid_launcher/ui/mixins/cloud_mixin.py:1349), which first
tries autoprofile token matching on the entry and then falls back to a plain case-folded
substring test of the token against the entry name
(grid_launcher/ui/mixins/cloud_mixin.py:1362). So an entry literally named
"My DuckStation build" matches `duckstation` even without a profile.
RPCS3 additionally ORs in a standalone name check
(grid_launcher/ui/mixins/install_mixin.py:410).

**Call sites** (all synchronous, on the UI thread):

| Trigger | Anchor |
|---|---|
| Before launching a game | grid_launcher/ui/mixins/details_view_mixin.py:1457 |
| Before launching an emulator standalone | grid_launcher/ui/mixins/emulator_ui_mixin.py:1655 |
| On saving an emulator entry in the dialog | grid_launcher/ui/mixins/emulator_ui_mixin.py:1537 |
| While resolving cloud sync directories | grid_launcher/ui/mixins/cloud_mixin.py:646 |
| After a successful RetroAchievements login, for every registered emulator | grid-launcher.py:2755 |
| After entry autoconfig from a downloaded executable | grid-launcher.py:3663 |

### Section-writer helpers and the overwrite question

Each emulator module carries its own near-duplicate `_ensure_*_section_values(raw, section, desired)`
helper returning `(new_text, changed)`. There are **three distinct write policies**, and the
difference matters for porting:

| Policy | Behavior on a key that already exists in the section | Modules |
|---|---|---|
| **Overwrite** | The existing line is replaced with `key = value`; `changed` is set only if the text actually differs. A second occurrence of the same key is deleted and marks `changed` | ppsspp.py:45, pcsx2.py:95, duckstation.py:93, dolphin.py:198, azahar.py:94, eden.py:171, rpcs3.py:238 |
| **Add-only** | The key is recorded as seen and the original line is emitted verbatim; only missing keys are appended | rpcs3.py:152 (`_ensure_yaml_section_values`), xemu.py:224 (`_ensure_toml_section_values`) |
| **Append-if-absent (whole block)** | The block is appended only when a marker section is missing | dolphin.py:390 (`[GCPad1]`), cemu.py:343 (controller profile) |

Common structure across all of them:

- Sections are matched case-insensitively on `^\[(.+?)\]$` after stripping
  (grid_launcher/emulator/pcsx2.py:83).
- Missing keys are flushed at the end of the target section, or at the section boundary
  when the next `[Section]` header is reached
  (grid_launcher/emulator/pcsx2.py:72, grid_launcher/emulator/pcsx2.py:85).
- If the section is absent, a blank separator line and then the whole section are appended
  (grid_launcher/emulator/pcsx2.py:114).
- Output is always `"\n".join(lines).rstrip() + "\n"` — trailing whitespace is normalized
  away on every write (grid_launcher/emulator/pcsx2.py:122).
- The key regex is `^\s*([A-Za-z0-9_]+)\s*=` in most modules; Azahar widens it to
  `[A-Za-z0-9_%\\]+` so it can manage `Shortcuts\Main%20Window\...` keys
  (grid_launcher/emulator/azahar.py:94).

Because the dominant policy is **overwrite**, "do not clobber the user" is implemented
one level up, by *omitting the key from `desired_values`* when a probe function reports it
already exists — `_section_has_key` in PCSX2 (grid_launcher/emulator/pcsx2.py:125) and
`_duckstation_section_has_key` in DuckStation (grid_launcher/emulator/duckstation.py:123).

### RetroArch — `ensure_retroarch_save_location_settings`

Signature: `(path, *, enable_fullscreen=False, username="", retroachievements_username="", retroachievements_token="")`
(grid_launcher/emulator/retroarch.py:232).

**Config discovery** (`retroarch_config_path_candidates`, grid_launcher/emulator/retroarch.py:136):

0. **Rewrite deviation:** `<exe>.home/.config/retroarch/retroarch.cfg`, only when that
   directory exists. The Python version has no equivalent candidate. This is deliberate,
   for the same reason as the `-L` core-resolution deviation in doc 04: the AppImage
   runtime sets `$HOME` to `<AppImage>.home` whenever that directory exists next to the
   file, so RetroArch reads its config from there instead of the emulator directory —
   writing anywhere else would never be applied. This is the one candidate that is
   existence-gated; writing a cfg into a `.home` that RetroArch will not use would be
   wrong for every non-AppImage install, so every other candidate below is unconditional.
1. `<root>/retroarch.cfg` and `<root>/config/retroarch.cfg`, where `<root>` is the
   emulator directory if the path looks like a file (is a file, or has a suffix),
   else the path itself (grid_launcher/emulator/retroarch.py:142).
2. `<XDG_CONFIG_HOME>/retroarch/retroarch.cfg`
3. `<XDG_DATA_HOME>/retroarch/retroarch.cfg`
4. `~/.config/retroarch/retroarch.cfg`

An empty path yields an empty candidate list and the function returns unchanged with a
logged warning (grid_launcher/emulator/retroarch.py:245).

**Target file:** whichever candidate the reader already parsed successfully
(`config_path` from `retroarch_directory_settings`), otherwise the first candidate
(grid_launcher/emulator/retroarch.py:252). On an AppImage install with a portable home,
that first candidate is the portable-home cfg, so the written file is the one RetroArch
actually reads.

**Save-location semantics.** `savefile_directory`/`savestate_directory` are seeded from
the *current* values and only fall back to the literals `saves` / `states` when unset
(grid_launcher/emulator/retroarch.py:259). The reader treats the literal value
`default` (case-insensitive) as "unset" (grid_launcher/emulator/retroarch.py:213), and
strips one layer of matching surrounding quotes from every value
(grid_launcher/emulator/retroarch.py:203). All six sort/in-content-dir booleans are forced
to `false`, which is what makes save paths stable enough to sync
(grid_launcher/emulator/retroarch.py:275).

**Write algorithm** (not a section writer — RetroArch's config is flat):
each existing line matching `^\s*([A-Za-z0-9_]+)\s*=` whose key is in `desired_values` is
replaced with `key = "value"`; duplicates of an already-seen key are dropped; unknown keys
pass through untouched; remaining desired keys are appended
(grid_launcher/emulator/retroarch.py:311, grid_launcher/emulator/retroarch.py:337).
`audio_volume` is the single explicit exception: if the key already exists, the user's line
is preserved verbatim (grid_launcher/emulator/retroarch.py:326).

`changed` starts as "the file did not exist"
(grid_launcher/emulator/retroarch.py:301). After writing, the reader runs again and the
fresh settings are returned with `config_path` and `changed` merged in
(grid_launcher/emulator/retroarch.py:352). A write failure returns the *pre-write*
settings with `changed=False` (grid_launcher/emulator/retroarch.py:348).

**RetroAchievements:** `cheevos_enable`, `cheevos_username`, `cheevos_token` are added only
when **both** username and token are non-blank
(grid_launcher/emulator/retroarch.py:296). The four `cheevos_*` hardcore/leaderboard
suppression keys are written unconditionally
(grid_launcher/emulator/retroarch.py:281). Note the local variable `username` is rebound
from the RomM nickname to the RA username partway through
(grid_launcher/emulator/retroarch.py:294) — the nickname was already consumed at
grid_launcher/emulator/retroarch.py:287.

**Core list handling.**

- Core id from a file name: strip `.dll`/`.so`/`.dylib`, then a trailing `_libretro`,
  lowercase (grid_launcher/emulator/retroarch.py:104).
- Core id from a display name: strip Markdown link syntax
  (grid_launcher/emulator/retroarch.py:49), apply a 22-entry override table for names
  whose slug is not mechanically derivable (`beetle psx` → `mednafen_psx`,
  `mupen64plus-next gles3` → `mupen64plus_next`, …)
  (grid_launcher/emulator/retroarch.py:61), otherwise collapse every run of
  non-alphanumerics to a single `_` and trim leading/trailing `_`
  (grid_launcher/emulator/retroarch.py:89).
- Platform key normalization: lowercase, backslashes → `/`, non-alphanumerics → spaces,
  runs collapsed (grid_launcher/emulator/retroarch.py:119).
- Platform → cores: prefer the RomM slug map when the game carries a slug
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:568), else the compatibility map.
  Compatibility lookup is exact first; on a miss it scores every key by Jaccard overlap
  of significant tokens (`the`, `and`, `of`, `for`, `system` are dropped) and accepts the
  best key only at score ≥ 0.7 (grid_launcher/emulator/retroarch.py:130,
  grid_launcher/emulator/retroarch.py:447, grid_launcher/emulator/retroarch.py:461).
  With an *empty* compatibility map the function returns the hardcoded
  `["fbneo", "mame2003_plus"]` (grid_launcher/emulator/retroarch.py:468).
- Installed cores: glob the cores directory for `*.dll` on win32, `*.dylib` on darwin,
  `*.so` elsewhere (grid_launcher/emulator/retroarch.py:511). The cores directory is
  `<exe>.home/.config/retroarch/cores` next to the executable when that exists (AppImage
  layout), else `<exe_dir>/cores`
  (grid_launcher/emulator/retroarch.py:496, grid_launcher/emulator/retroarch.py:506).
  With no explicit `cores_dir` the executable must exist and be a file
  (grid_launcher/emulator/retroarch.py:493). Results are cached per
  emulator+path in the UI layer (grid_launcher/ui/mixins/emulator_ui_mixin.py:541).

**Core download/install** is not performed by this module. RetroArch's autoprofile carries
a `supplemental_downloads` entry pointing at the libretro buildbot's `RetroArch_cores.7z`
(emulator-autoprofiles.json, first profile), which is fetched and extracted by the generic
acquisition path — doc 04 (grid_launcher/background/workers.py:128,
grid_launcher/ui/mixins/install_mixin.py:751). What *is* RetroArch-specific is the
post-install firmware/config/saves placement driven by the core-list metadata
(grid_launcher/ui/mixins/install_mixin.py:552):

- `firmware.subdirectory` appends a subdirectory to each system dir
  (grid_launcher/ui/mixins/install_mixin.py:564); `firmware.files` restricts the fetch to
  named files (grid_launcher/ui/mixins/install_mixin.py:570);
  `firmware.extract_with_paths` controls zip extraction mode
  (grid_launcher/ui/mixins/install_mixin.py:576).
- `config_files.base_dir` is resolved relative to the emulator directory
  (grid_launcher/ui/mixins/install_mixin.py:593).
- `saves_files.file` is dropped into the resolved savefile directory, where the value
  `default` means `<emulator_dir>/saves` and a leading `:\` or `:/` means "RetroArch
  root-relative", stripped and joined onto the emulator directory
  (grid_launcher/ui/mixins/install_mixin.py:617).
- If the platform has no configured default core, firmware install is skipped entirely
  (grid_launcher/ui/mixins/install_mixin.py:557).

**Flycast/VMU.** `retroarch_core_flags` returns the four capability flags with the
defaults `supports_save_states=True`, `supports_saves=True`, `cloud_sync_safe=True`,
`vmu_shared_saves=False` for unknown cores
(grid_launcher/emulator/retroarch.py:587). `flycast_vmu_file_candidates` scans directories
with a `*.bin` glob and matches names against a case-insensitive `vmu[0-3]*` regex
(grid_launcher/emulator/retroarch.py:638, :645) — only the regex is case-insensitive, so
`VMU0.BIN` is invisible on a case-sensitive filesystem. It keeps the newest file by mtime per slot, and
returns them ordered by slot 0→3 (grid_launcher/emulator/retroarch.py:635,
grid_launcher/emulator/retroarch.py:649). `vmu_shared_saves` promotes the cloud scope to
shared-slotted — doc 06 (grid_launcher/emulator/selection.py:89).

**Save paths for cloud sync.** RetroArch does not have a `*_save_path_overrides` function.
Instead the cloud resolver prepends `savefile_directory`/`savestate_directory` from
`retroarch_directory_settings` and appends the fallbacks `saves`/`savefiles` or
`states`/`savestates` (grid_launcher/ui/mixins/cloud_mixin.py:647).

### DuckStation — `ensure_duckstation_memory_card_settings`

Signature: `(path, *, enable_fullscreen=False)` — **no RetroAchievements parameters**
(grid_launcher/emulator/duckstation.py:198).

1. Resolve `<emulator_dir>` (path itself if a directory, else its parent) and create an
   empty `portable.txt` if absent (grid_launcher/emulator/duckstation.py:206).
2. Read current memory-card settings via `duckstation_memory_card_settings`, which walks
   the candidate list and stops at the first file from which at least one `[MemoryCards]`
   key parsed (grid_launcher/emulator/duckstation.py:191).
3. Candidate order (grid_launcher/emulator/duckstation.py:10):
   emulator dir → `%LOCALAPPDATA%/DuckStation` → `~/Documents/DuckStation` →
   `~/.local/share/duckstation` → `~/.config/duckstation` →
   `~/Library/Application Support/DuckStation` → `<XDG_DATA_HOME>/duckstation` →
   `<XDG_CONFIG_HOME>/duckstation`, each with `settings.ini` appended and deduplicated.
4. **Write target:** always `<emulator_dir>/settings.ini` when a path was supplied,
   regardless of which candidate was read
   (grid_launcher/emulator/duckstation.py:222). Only when no path was supplied does it
   fall back to the parsed `config_path` and then the first candidate
   (grid_launcher/emulator/duckstation.py:225).
5. Desired `[MemoryCards]`: `Directory` keeps the current value or defaults to `memcards`;
   `Card1Type` is kept only if it is one of `PerGame`/`PerGameTitle`/`PerGameFileTitle`,
   else forced to `PerGameTitle`; `Card2Type` is kept only if it is one of those or
   `None`, else forced to `None`; `UsePlaylistTitle` is forced to `true`
   (grid_launcher/emulator/duckstation.py:235).
6. Forced keys: `[Main] InhibitScreensaver=true, SetupWizardIncomplete=false`,
   `[Display] FullscreenMode="Borderless Windowed"`, `[AutoUpdater] CheckAtStartup=false`,
   and the whole `[Cheevos]` block (`Enabled=true`, `ChallengeMode=false`,
   `LeaderboardNotifications=false`, `LeaderboardTrackers=false`)
   (grid_launcher/emulator/duckstation.py:252, grid_launcher/emulator/duckstation.py:363).
7. Preserved-if-present keys, gated on `_duckstation_section_has_key(raw_content, ...)`
   against the **pre-write** content: `Main.ConfirmPowerOff`, `Display.Scaling`,
   `Display.Scaling24Bit`, all nine `[GPU]` keys, `Audio.OutputVolume`,
   `Hotkeys.OpenPauseMenu`, and the entire `[Pad1]` block (gated on `Pad1.Type`)
   (grid_launcher/emulator/duckstation.py:258, grid_launcher/emulator/duckstation.py:283,
   grid_launcher/emulator/duckstation.py:320).
8. `enable_fullscreen` adds `[Main] StartFullscreen=true`
   (grid_launcher/emulator/duckstation.py:355).
9. `changed` starts as "the target did not exist"
   (grid_launcher/emulator/duckstation.py:242). On success the reader runs again and the
   result carries the actual `config_path` and `changed`
   (grid_launcher/emulator/duckstation.py:383).

DuckStation has no `*_save_path_overrides`; cloud sync uses the `memcards`
directory reported by `duckstation_memory_card_settings`.

### PCSX2 — `ensure_pcsx2_settings`

Signature: `(path, *, enable_fullscreen=False, retroachievements_username="", retroachievements_token="", bios_directory="")`
(grid_launcher/emulator/pcsx2.py:170).

- **Requires the executable to exist and be a file**; otherwise returns
  `{"config_path": None, "changed": False}` (grid_launcher/emulator/pcsx2.py:183).
- Always creates an empty `portable.ini` next to the executable if absent — the
  emulator is forced into portable mode (grid_launcher/emulator/pcsx2.py:187).
- Always writes `<emulator_dir>/inis/PCSX2.ini`
  (grid_launcher/emulator/pcsx2.py:194). The Documents / XDG candidates in
  `pcsx2_config_path_candidates` (grid_launcher/emulator/pcsx2.py:146) are used only by
  the *readers*, never by `ensure_pcsx2_settings`.
- Forced keys: `[UI] SetupWizardIncomplete=false, SettingsVersion=1, InhibitScreensaver=true`;
  `[AutoUpdater] CheckAtStartup=false`; `[EmuCore] EnableDiscordPresence=false`;
  `[EmuCore/GS] pcrtc_antiblur=true, pcrtc_offsets=false`
  (grid_launcher/emulator/pcsx2.py:200, grid_launcher/emulator/pcsx2.py:222,
  grid_launcher/emulator/pcsx2.py:254).
- Preserved-if-present: `UI.ConfirmShutdown`, `UI.PauseOnFocusLoss`, `UI.HideMouseCursor`
  (grid_launcher/emulator/pcsx2.py:215); `EmuCore.EnableWideScreenPatches`,
  `EmuCore.EnableNoInterlacingPatches` (grid_launcher/emulator/pcsx2.py:231); ten
  `[EmuCore/GS]` quality keys (grid_launcher/emulator/pcsx2.py:264); three
  `[EmuCore/Speedhacks]` keys (grid_launcher/emulator/pcsx2.py:285); the 35-key `[Pad1]`
  SDL mapping gated on `Pad1.Type` (grid_launcher/emulator/pcsx2.py:299);
  `Hotkeys.OpenPauseMenu` (grid_launcher/emulator/pcsx2.py:343);
  `SPU2/Output.StandardVolume` (grid_launcher/emulator/pcsx2.py:349);
  `EmuCore/GS.upscale_multiplier` (grid_launcher/emulator/pcsx2.py:355).
- `enable_fullscreen` adds `[UI] StartFullscreen=true`
  (grid_launcher/emulator/pcsx2.py:361).
- RetroAchievements writes `[Achievements] Enabled/Username/Token` only when **both**
  credentials are non-blank (grid_launcher/emulator/pcsx2.py:246).
- `bios_directory` writes `[Folders] Bios` only when non-blank **and** the key is not
  already present (grid_launcher/emulator/pcsx2.py:367).
- The file is written only if something changed (grid_launcher/emulator/pcsx2.py:374).

**Portable/data-root resolution for readers** (`pcsx2_data_root_candidates`,
grid_launcher/emulator/pcsx2.py:436): a portable root is used when `-portable` appears in
the launch template, or `portable.ini`/`portable.txt` exists
(grid_launcher/emulator/pcsx2.py:420). If `portable.txt` contains text, that text is
treated as a subdirectory suffix under the emulator directory
(grid_launcher/emulator/pcsx2.py:426). Ordering is portable roots, then user roots
(Windows Documents via Shell API, `$OneDrive/Documents/PCSX2`,
`$USERPROFILE/Documents/PCSX2`, `$HOME/Documents/PCSX2`, `~/Documents/PCSX2`,
`~/.config/PCSX2`, `~/Library/Application Support/PCSX2`, `$XDG_CONFIG_HOME/PCSX2`), then
the plain emulator directory (grid_launcher/emulator/pcsx2.py:480).

**Save/state overrides.** `pcsx2_save_path_overrides` returns
`[<memcards>/<slot1_filename>, <memcards>/<slot2_filename>, <memcards>]` — file paths
first, then the containing directory (grid_launcher/emulator/pcsx2.py:595).
Slot filenames default to `Mcd001.ps2`/`Mcd002.ps2` and are overridden from
`[MemoryCards]` in the INI (grid_launcher/emulator/pcsx2.py:567).
`pcsx2_state_path_overrides` returns the single `savestates` directory
(grid_launcher/emulator/pcsx2.py:612). Both resolve relative INI values against the data
root and default to `memcards`/`sstates` (grid_launcher/emulator/pcsx2.py:564).

### Dolphin

Three separate entry points.

`ensure_dolphin_settings` (grid_launcher/emulator/dolphin.py:253):

- Creates an empty `portable.txt` next to the executable if absent
  (grid_launcher/emulator/dolphin.py:257).
- Candidate INI locations (`dolphin_ini_path_candidates`,
  grid_launcher/emulator/dolphin.py:228): `<exe_dir>/User/Config/<name>` (only when the
  path is absolute), `%APPDATA%/Dolphin Emulator/Config/<name>`,
  `~/.local/share/dolphin-emu/<name>`, `~/Library/Application Support/Dolphin/<name>`,
  `~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/<name>`.
- **Selection rule:** when a non-blank emulator path was given, the *first* candidate is
  used unconditionally — i.e. the portable `User/Config` path — otherwise the first
  existing candidate (grid_launcher/emulator/dolphin.py:270).
- Writes `Dolphin.ini` (`[Analytics] Enabled=False, PermissionAsked=True`;
  `[Display] Fullscreen=True, RenderToMain=True`; `[General] ShowLaunchWarning=False`;
  `[DSP] Volume=70`) and `GFX.ini` (`[Settings] UseVerticalSync=True`)
  (grid_launcher/emulator/dolphin.py:277, grid_launcher/emulator/dolphin.py:302).
  These are unconditional overwrites.
- Returns `{"dolphin_ini_path", "gfx_ini_path", "changed"}`; a per-file `OSError` sets
  that file's path to `None` without aborting the other
  (grid_launcher/emulator/dolphin.py:291).

`ensure_dolphin_skip_ipl` sets `[Core] SkipIPL=False` to re-enable the GameCube boot
animation, selecting the first *existing* candidate
(grid_launcher/emulator/dolphin.py:318, grid_launcher/emulator/dolphin.py:325).

`ensure_dolphin_gcpad_config` appends a fixed `[GCPad1]` XInput block to `GCPadNew.ini`
only if no `[GCPad1]` header exists (case-insensitive, multiline regex)
(grid_launcher/emulator/dolphin.py:371, grid_launcher/emulator/dolphin.py:390). When no
`GCPadNew.ini` exists anywhere, the file is created next to whichever `Dolphin.ini`
already exists, falling back to the first candidate
(grid_launcher/emulator/dolphin.py:382). A trailing newline is added before appending
(grid_launcher/emulator/dolphin.py:395).

Both extra entry points are called after firmware install, wrapped in bare
`except Exception: pass` (grid_launcher/ui/mixins/install_mixin.py:687).

**User-root resolution** (`dolphin_user_root_candidates`,
grid_launcher/emulator/dolphin.py:100), in order:
`-u`/`--user`/`--user=` from the launch template
(grid_launcher/emulator/dolphin.py:53) → `<exe_dir>/User` when `portable.txt` exists
(grid_launcher/emulator/dolphin.py:118) → registry-derived root on Windows
(`LocalUserConfig` truthy ⇒ `<exe_dir>/User`, else `UserConfigPath`)
(grid_launcher/emulator/dolphin.py:80) → `$OneDrive`/`$USERPROFILE` Documents and
`%APPDATA%` on Windows (grid_launcher/emulator/dolphin.py:125) → `~/.dolphin-emu`,
`~/Library/Application Support/Dolphin`, the Flatpak data dir → `<exe_dir>/User` as a
final fallback (grid_launcher/emulator/dolphin.py:134).

**Save overrides** (grid_launcher/emulator/dolphin.py:511) emit, in order:

1. the configured `MemcardAPath`/`MemcardBPath` if set, each followed by all
   `MemoryCard{A,B}.{USA,JPN,JAP,EUR,DEV}{,.59,.123,.251,.507,.1019,.2043}.raw`
   permutations under `<user_root>/GC` (grid_launcher/emulator/dolphin.py:490,
   grid_launcher/emulator/dolphin.py:526);
2. `GCIFolderAPathOverride`/`GCIFolderBPathOverride` when set
   (grid_launcher/emulator/dolphin.py:532);
3. `GCIFolderAPath`/`GCIFolderBPath` expanded to sibling region directories — if the
   configured directory's name is already a region name, its parent is used as base
   (grid_launcher/emulator/dolphin.py:503) — plus `<GC>/<region>/Card {A,B}` defaults
   (grid_launcher/emulator/dolphin.py:499);
4. `<wii_root>/title` and the six Wii title groups `00010000`, `00010001`, `00010002`,
   `00010004`, `00010005`, `00010008` (grid_launcher/emulator/dolphin.py:12,
   grid_launcher/emulator/dolphin.py:543).

`wii_root` comes from `[General] NANDRootPath`, defaulting to `<user_root>/Wii`
(grid_launcher/emulator/dolphin.py:470). States are `<user_root>/StateSaves`
(grid_launcher/emulator/dolphin.py:471, grid_launcher/emulator/dolphin.py:557).

### Azahar

`ensure_azahar_settings` (grid_launcher/emulator/azahar.py:141):

1. Create `<emulator_dir>/user/` if missing (portable marker)
   (grid_launcher/emulator/azahar.py:146).
2. Candidates: `<exe_dir>/user/config/qt-config.ini`, `<exe_dir>/qt-config.ini`,
   `%APPDATA%/Azahar/qt-config.ini`, `~/.config/Azahar/qt-config.ini`, and the Flatpak
   config path (grid_launcher/emulator/azahar.py:129). Selection: **first existing**,
   else the first candidate (grid_launcher/emulator/azahar.py:156).
3. Writes `[Renderer]`, `[Audio]`, `[UI]` with each real key paired with an explicit
   `<key>\default = false` companion key that the module writes as an ordinary key —
   Azahar's widened key regex allows `\` and `%` in key names
   (grid_launcher/emulator/azahar.py:165, grid_launcher/emulator/azahar.py:94).
   Values: `resolution_factor=4`, `use_vsync=true`, `volume=0.4`,
   `enable_discord_presence=false`, `confirmClose=false`, `fullscreen=true`,
   `pauseWhenInBackground=true`, `hideInactiveMouse=true`, Fullscreen shortcut `F1`,
   Stop Emulation shortcut `Escape`.
4. All keys are unconditional overwrites.

**User-root resolution** (grid_launcher/emulator/azahar.py:219): `<exe_dir>/user` when it
exists and is a directory → `%APPDATA%/Azahar` on Windows, else `$XDG_DATA_HOME/Azahar`,
`~/.local/share/Azahar`, `~/Library/Application Support/Azahar`, the Flatpak data dir →
`<exe_dir>/user` again as fallback.

**Storage semantics** (grid_launcher/emulator/azahar.py:272): reads `[Data Storage]`;
`use_custom_storage` (default false) selects between the configured
`nand_directory`/`sdmc_directory` and `<user_root>/nand`, `<user_root>/sdmc`;
`use_virtual_sd` defaults to **true**.

**Save overrides** (grid_launcher/emulator/azahar.py:387):

- SDMC (skipped when `use_virtual_sd == "false"`): walk
  `<sdmc>/Nintendo 3DS/<sysid>/<storeid>/title/<group>` for the groups
  `00040000`, `00040002`, `0004000e`, `0004008c`, `00048004`, keeping only existing
  directories (grid_launcher/emulator/azahar.py:10,
  grid_launcher/emulator/azahar.py:333). If nothing exists, fall back to the all-zero
  32-character id path `<sdmc>/Nintendo 3DS/<0*32>/<0*32>/title/<group>`
  (grid_launcher/emulator/azahar.py:353).
- NAND: title containers are `<nand>/title` plus `<nand>/<child>/title`, groups
  `00040010`, `00040030`; fall back to `<nand>/<0*32>/title/<group>` and
  `<nand>/title/<group>` (grid_launcher/emulator/azahar.py:357).

`azahar_state_path_overrides` returns the single `<user_root>/states`
(grid_launcher/emulator/azahar.py:415).

### Eden

`ensure_eden_settings` (grid_launcher/emulator/eden.py:216):

1. Create `<emulator_dir>/user/` if missing (grid_launcher/emulator/eden.py:220).
2. Candidates: `<exe_parent>/user/config/qt-config.ini`,
   `%APPDATA%/eden/config/qt-config.ini` (Windows only, when `APPDATA` is set),
   `<XDG_CONFIG_HOME>/eden/qt-config.ini` (grid_launcher/emulator/eden.py:206).
   Selection: first existing, else first (grid_launcher/emulator/eden.py:231).
3. Uses `_ensure_eden_section_values`, which *generates* the `key\default=false`
   annotation line before each managed key rather than treating the annotation as a
   separate desired key (grid_launcher/emulator/eden.py:111,
   grid_launcher/emulator/eden.py:134). Existing annotation lines for managed keys are
   rewritten to exactly `key\default=false`; duplicates are dropped
   (grid_launcher/emulator/eden.py:155).
4. Keys: `[UI]` `enable_discord_presence=false`, `confirmStop=2`, `fullscreen=true`,
   `firstStart=false`, `pauseWhenInBackground=true`, `enable_gamemode=true`,
   `theme=colorful_dark`, `check_for_updates=false`; `[WebService] enable_telemetry=false`;
   `[Audio] volume=40, muteWhenInBackground=true`; `[Renderer] scaling_filter=6`
   (grid_launcher/emulator/eden.py:237). All unconditional overwrites.
5. Returns `config_path` as a `Path` object, not a string
   (grid_launcher/emulator/eden.py:280).

**User-root resolution** (grid_launcher/emulator/eden.py:316) additionally probes
alternate application names: the executable stem in three casings, then
`Eden`, `eden`, `yuzu`, `Yuzu`, `suyu`, `Suyu`, deduplicated case-insensitively
(grid_launcher/emulator/eden.py:294).

**Firmware/keys probes:** `eden_keys_path` returns `<emulator_dir>/user/keys/prod.keys`
when it exists (grid_launcher/emulator/eden.py:372); `eden_has_firmware` is true when
`<emulator_dir>/user/nand/system/Contents/registered` is a non-empty directory
(grid_launcher/emulator/eden.py:383).

**Save overrides** (grid_launcher/emulator/eden.py:472): enumerate
`<nand>/user/save/0000000000000000/<user-dir>` and keep only user directories that contain
at least one subdirectory; if none qualify, return the parent
`<nand>/user/save/0000000000000000` itself (grid_launcher/emulator/eden.py:455).
Eden has no state-path override function.

### RPCS3

`ensure_rpcs3_settings(path, ps3_library_path="")` (grid_launcher/emulator/rpcs3.py:533):

- Requires the executable to exist and be a **file**
  (grid_launcher/emulator/rpcs3.py:539).
- Unconditionally targets `<exe_dir>/portable/…`, creating `portable/`, `portable/config/`
  and `portable/GuiConfigs/` (grid_launcher/emulator/rpcs3.py:542,
  grid_launcher/emulator/rpcs3.py:551).
- `config.yml` uses the **add-only** YAML writer: `Miscellaneous:` gains
  `Start games in fullscreen mode: true` and `Audio:` gains `Master Volume: 40`, and
  existing values are never touched (grid_launcher/emulator/rpcs3.py:558,
  grid_launcher/emulator/rpcs3.py:152). Missing sections are appended with a blank
  separator line (grid_launcher/emulator/rpcs3.py:161). Section headers are matched by
  `^([A-Za-z][^:\n]*):[ \t]*$` and keys by exactly two spaces of indentation
  (grid_launcher/emulator/rpcs3.py:140, grid_launcher/emulator/rpcs3.py:152).
- `GuiSettings.ini` is written with `annotate=True`, producing `key\default=false`
  followed by `key = value` for each of the four `[main_window]` suppression keys
  (grid_launcher/emulator/rpcs3.py:565, grid_launcher/emulator/rpcs3.py:199).
- `CurrentSettings.ini` is written with `annotate=False`, producing bare `key=value` with
  no spaces and **deleting** any `key\default=` annotation lines for managed keys
  (grid_launcher/emulator/rpcs3.py:576, grid_launcher/emulator/rpcs3.py:191,
  grid_launcher/emulator/rpcs3.py:223). This file is the runtime-authoritative one for
  `[Meta] checkUpdateStart=false, useRichPresence=false` and the four `[main_window]` keys.
- Both INI writers **overwrite** existing managed values.
- If `ps3_library_path` is non-blank, `ensure_rpcs3_vfs_settings` runs and its `changed`
  is folded in (grid_launcher/emulator/rpcs3.py:590).

`ensure_rpcs3_vfs_settings` (grid_launcher/emulator/rpcs3.py:389) writes
`<exe_dir>/portable/config/vfs.yml`. Desired entries are `"$(EmulatorDir)": ""`,
`"/dev_hdd0/": "<library>/.vfs/dev_hdd0/"`, `"/games/": "<library>/.vfs/games/"`, each
POSIX-form with a guaranteed trailing slash (grid_launcher/emulator/rpcs3.py:412). It is
strictly **add-only**: any key already present in the file — comparing on the text before
the first `:` with quotes stripped — is left alone
(grid_launcher/emulator/rpcs3.py:433, grid_launcher/emulator/rpcs3.py:445).

`update_rpcs3_games_yml` (grid_launcher/emulator/rpcs3.py:307) writes
`<data_root>/config/games.yml`, replacing the line whose key equals the game id or
appending it. The value is the resolved POSIX directory with a trailing slash: either
`<games_root>/<GAMEID>` when a games root is supplied, else
`<dev_hdd0>/game/<GAMEID>` (grid_launcher/emulator/rpcs3.py:320). Returns `False` for a
blank game id, non-`Path` arguments, or any `OSError`
(grid_launcher/emulator/rpcs3.py:313, grid_launcher/emulator/rpcs3.py:359).

`copy_ps3_custom_config_to_emulator` merges `<vfs_config>/custom_configs/*` into
`<data_root>/config/custom_configs/`, silently skipping when the source is missing and
swallowing `OSError` so it can never block a launch
(grid_launcher/emulator/rpcs3.py:746).

**Data-root candidate order** (`rpcs3_data_root_candidates`,
grid_launcher/emulator/rpcs3.py:605): `$RPCS3_CONFIG_DIR` is inserted at index 1 of the
platform list (`candidates.insert(1 if candidates else 0, ...)`,
grid_launcher/emulator/rpcs3.py:619-621), so the effective precedence is
`<exe_dir>/portable` (when it exists) → `$RPCS3_CONFIG_DIR` → `<exe_dir>` →
`<XDG_CONFIG_HOME>/rpcs3` → `~/Library/Application Support/rpcs3`; when `portable/` is
absent the order starts `<exe_dir>` → `$RPCS3_CONFIG_DIR`. Deduplicated
case-insensitively (grid_launcher/emulator/rpcs3.py:618,
grid_launcher/emulator/rpcs3.py:622). `rpcs3_data_root` is the simpler two-way variant
used elsewhere: portable when present, else the emulator directory, or `None` when the
parent directory does not exist (grid_launcher/emulator/rpcs3.py:285).

**VFS path resolution.** `vfs.yml` is looked for at `<root>/config/vfs.yml` then
`<root>/vfs.yml` (grid_launcher/emulator/rpcs3.py:633). Scalars are matched by a
case-insensitive regex allowing optional quoting around the key, and the values
`""`, `{}`, `[]`, `|`, `>` are treated as empty
(grid_launcher/emulator/rpcs3.py:83, grid_launcher/emulator/rpcs3.py:89). Values are
cleaned by stripping an unquoted trailing `# comment`
(grid_launcher/emulator/rpcs3.py:75). `$(EmulatorDir)` is substituted with the
resolved base root in POSIX form with a trailing slash, then `os.path.expandvars` and
`~` expansion run, and relative results are joined onto the base root
(grid_launcher/emulator/rpcs3.py:102). `/dev_hdd0/` defaults to
`$(EmulatorDir)dev_hdd0/` (grid_launcher/emulator/rpcs3.py:700).
`ps3_vfs_dev_hdd0_path`/`ps3_vfs_games_path` fall back to `<ps3_library>/.vfs/dev_hdd0`
and `<ps3_library>/.vfs/games` when no `vfs.yml` is usable, and return `None` when the
library path is also blank (grid_launcher/emulator/rpcs3.py:497,
grid_launcher/emulator/rpcs3.py:527).

**Active user.** `current_user` comes from `--user-id <id>` or `--user-id=<id>` in the
launch template, else `[Users] active_user` in
`<root>/GuiConfigs/persistent_settings.dat` (parsed with a standard INI parser), else
`00000001` (grid_launcher/emulator/rpcs3.py:46, grid_launcher/emulator/rpcs3.py:641,
grid_launcher/emulator/rpcs3.py:671). A user id is valid only if it is exactly 8 digits
and not `00000000` (grid_launcher/emulator/rpcs3.py:13,
grid_launcher/emulator/rpcs3.py:28).

**Save overrides** (grid_launcher/emulator/rpcs3.py:709): the current user's
`<dev_hdd0>/home/<user>/savedata` first, then every existing valid 8-digit user directory
in name order, then `<dev_hdd0>/home/00000001/savedata` as a guaranteed tail entry; all
resolved and deduplicated case-insensitively (grid_launcher/emulator/rpcs3.py:722).
RPCS3 has no state-path override function.

**Background firmware.** `_trigger_rpcs3_firmware_download_background`
(grid_launcher/ui/mixins/emulator_ui_mixin.py:1737):

1. Return immediately if `<emulator_dir>/PS3UPDAT.PUP` already exists
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:1740,
   grid_launcher/emulator/rpcs3.py:274).
2. Return if there are no resolved firmware directories
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:1743).
3. Find the PS3 server platform id by scanning `server_platform_ids` for a key containing
   `playstation 3` or equal to `ps3`
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:1749).
4. Create a UI download entry and bump the active-download counters *before* starting the
   thread (grid_launcher/ui/mixins/emulator_ui_mixin.py:1759).
5. Start a `daemon=True` thread that calls `download_ps3_firmware_direct(...)` with a
   progress callback that emits `_firmware_download_progress`, and on any warning falls
   back to `install_platform_firmware(...)` from the RomM server when a platform id was
   found (grid_launcher/ui/mixins/emulator_ui_mixin.py:1767).
6. Emits `_firmware_download_done` and `_emulator_refresh_requested` on completion
   (grid_launcher/ui/mixins/emulator_ui_mixin.py:1784). Those signals are connected in
   `MainWindow.__init__` (grid-launcher.py:462).

A sibling path, `_trigger_firmware_install_for_source_emulator`, does the same for
non-RPCS3 emulators with `firmware_directories`, skipping RPCS3 explicitly
(grid_launcher/ui/mixins/emulator_ui_mixin.py:1851) and swallowing every worker exception
(grid_launcher/ui/mixins/emulator_ui_mixin.py:1910).

### PPSSPP — `ensure_ppsspp_settings`

Signature: `(path, *, retroachievements_username="", retroachievements_token="")`
(grid_launcher/emulator/ppsspp.py:75). Blank path returns `{"changed": False}`
(grid_launcher/emulator/ppsspp.py:82).

1. **Deletes `<emulator_dir>/installed.txt`** if present — this is what suppresses the
   first-run/installer flow; deletion counts as `changed`
   (grid_launcher/emulator/ppsspp.py:87).
2. Single config file: `<emulator_dir>/memstick/PSP/SYSTEM/PPSSPP.INI`, always this path,
   no platform candidates (grid_launcher/emulator/ppsspp.py:97).
3. Sections written (all unconditional overwrites, `key = value` with spaces):
   `[General] CheckForNewVersion=False, SaveStateSlotCount=3`;
   `[Graphics]` nine keys including `InternalResolution=4`, `MultiSampleLevel=2`,
   `TexScalingLevel=4`; `[Sound] GameVolume=25, AchievementVolume=40`;
   `[Theme] ThemeName=Slate Forest` (grid_launcher/emulator/ppsspp.py:100).
4. RetroAchievements: only when **both** credentials are non-blank, an `[Achievements]`
   section is appended with `AchievementsEnable=True`, username, token,
   `AchievementsChallengeMode=False`, and six notification-position keys
   (grid_launcher/emulator/ppsspp.py:127).
5. Additionally writes the raw token to
   `<…>/SYSTEM/ppsspp_retroachievements.dat`, but only when its current trimmed contents
   differ from the token (grid_launcher/emulator/ppsspp.py:155).
6. Parent directories are created lazily, only when a write is needed
   (grid_launcher/emulator/ppsspp.py:149). Write failures are swallowed and do not set
   `changed` (grid_launcher/emulator/ppsspp.py:152).

PPSSPP has no directory-settings reader or path-override function in this module.

### Cemu

`ensure_cemu_settings` (grid_launcher/emulator/cemu.py:286):

- Always forces portable mode: creates `<emulator_dir>/portable/` and targets
  `portable/settings.xml` (grid_launcher/emulator/cemu.py:297).
- If the file does not exist, a **complete 120-line default `settings.xml`** is written
  and the function returns `changed=True` immediately
  (grid_launcher/emulator/cemu.py:115, grid_launcher/emulator/cemu.py:300).
- Otherwise the XML is parsed; the root is `content`, or the first `.//content` descendant
  (grid_launcher/emulator/cemu.py:308). Six elements are forced:
  `use_discord_presence=false`, `check_update=false`, `receive_untested_updates=false`,
  `gp_download=true`, `fullscreen=false`, `window_maximized=true`
  (grid_launcher/emulator/cemu.py:314). Missing elements are created as sub-elements
  (grid_launcher/emulator/cemu.py:276). Everything else in the document is preserved.
- On change, the file is rewritten as `<?xml version="1.0" encoding="utf-8"?>\n` plus the
  serialized root (grid_launcher/emulator/cemu.py:322).
- The whole body is wrapped in a bare `except Exception`, returning
  `{"config_path": None, "changed": False}` — this covers XML parse errors as well as I/O
  (grid_launcher/emulator/cemu.py:326).

`ensure_cemu_controller_config` (grid_launcher/emulator/cemu.py:330) writes
`portable/controllerProfiles/controller0.xml` only when it does not already exist
(grid_launcher/emulator/cemu.py:343). Windows gets the XInput "Wii U Pro Controller"
profile; every other platform gets the SDLController profile with a different button
mapping table (grid_launcher/emulator/cemu.py:346,
grid_launcher/emulator/cemu.py:13, grid_launcher/emulator/cemu.py:64).

**Settings candidates for reading** (grid_launcher/emulator/cemu.py:252):
`<exe_dir>/portable/settings.xml`, `<exe_dir>/settings.xml`, then
`%APPDATA%/Cemu/settings.xml` and `%LOCALAPPDATA%/Cemu/settings.xml` on Windows, or
`<XDG_CONFIG_HOME>/Cemu/settings.xml` elsewhere.

**Save overrides** (grid_launcher/emulator/cemu.py:398): MLC paths are collected from
`-m`/`--mlc`/`--mlc=`/`-m=` in the launch template first, then from the `<mlc_path>`
element in the settings XML (grid_launcher/emulator/cemu.py:418,
grid_launcher/emulator/cemu.py:429). Each is converted to a save root: if the path already
ends with `usr/save` (in either slash style, case-insensitively) it is used as-is with
trailing separators trimmed; otherwise `usr/save` is appended
(grid_launcher/emulator/cemu.py:387). Cemu has no state-path override function.
`cemu_directory_settings` reports the first candidate that yields a non-blank `mlc_path`
(grid_launcher/emulator/cemu.py:378).

### Xemu

`ensure_xemu_settings(path)` (grid_launcher/emulator/xemu.py:243):

- Target: `<emulator_dir>/xemu.toml` when a path is given, else
  `<default_base_root>/xemu.toml` (grid_launcher/emulator/xemu.py:250). The default base
  root is `%APPDATA%/xemu/xemu` on Windows,
  `~/Library/Application Support/xemu/xemu` on macOS, `$XDG_DATA_HOME/xemu/xemu` or
  `~/.local/share/xemu/xemu` elsewhere (grid_launcher/emulator/xemu.py:120).
- The TOML writer is **add-only** — existing keys are recorded and left untouched
  (grid_launcher/emulator/xemu.py:224). Its key regex allows `-`
  (grid_launcher/emulator/xemu.py:223).
- Sections written: `[general] show_welcome=false`, `[misc] check_for_updates=false`,
  `[display] vsync=true`, `[display.window] fullscreen_on_startup=true`,
  `[display.quality] surface_scale=2`, `[audio] volume_limit=0.4`,
  `[input.bindings] port1_driver="usb-xbox-gamepad"`
  (grid_launcher/emulator/xemu.py:257 through grid_launcher/emulator/xemu.py:302).
- `[sys.files]` receives absolute single-quoted paths to `mcpx_1.0.bin`,
  `complex_4627.bin`, `xbox_hdd.qcow2`, `eeprom.bin` under the base directory
  (grid_launcher/emulator/xemu.py:306, grid_launcher/emulator/xemu.py:77).
- `xemu_missing_bios_files` reports which of `mcpx_1.0.bin`, `complex_4627.bin`,
  `xbox_hdd.qcow2` are absent — `eeprom.bin` is deliberately not required
  (grid_launcher/emulator/xemu.py:329).

**Base-path resolution** (grid_launcher/emulator/xemu.py:137): a
`-config_path`/`--config-path` (and `=` forms) launch argument wins; a bare directory
value gets `xemu.toml` appended (grid_launcher/emulator/xemu.py:104). Then the emulator
directory is used if it contains `xemu.toml`, `xbox_hdd.qcow2` or `eeprom.bin`
(grid_launcher/emulator/xemu.py:154). Then the platform default.

The TOML reader flattens dotted keys into synthetic sections (`a.b = x` inside
`[sec]` becomes `sections["sec.a"]["b"]`) and expands an inline `files = { … }` table into
a `<section>.files` pseudo-section (grid_launcher/emulator/xemu.py:375,
grid_launcher/emulator/xemu.py:382).

**Save overrides** (grid_launcher/emulator/xemu.py:440): `[hdd_path, eeprom_path]` —
the HDD image and EEPROM file, deduplicated. There is no state-path override.

### Xenia

Xenia has no `ensure_*` writer. It has a content installer and readers.

`apply_xenia_content_without_ui` (grid_launcher/emulator/xenia.py:36) parses an STFS
header: magic must be one of `CON `, `LIVE`, `PIRS` at offset 0; ContentType is the
big-endian uint32 at `0x344`; TitleID at `0x360`; both are formatted as uppercase 8-char
hex (grid_launcher/emulator/xenia.py:12, grid_launcher/emulator/xenia.py:31). Requires at
least `0x368` header bytes (grid_launcher/emulator/xenia.py:26). When
`expected_title_id` is supplied and mismatches, the copy is refused
(grid_launcher/emulator/xenia.py:62). Destination is
`<content_root>/0000000000000000/<TitleID>/<ContentType>/<original filename>`
(grid_launcher/emulator/xenia.py:15, grid_launcher/emulator/xenia.py:70). Errors are
returned in the `error` field rather than raised.

**Variant and portable detection** (grid_launcher/emulator/xenia.py:346):
the variant is `canary` when the path contains `xenia_canary`, `xenia-canary`, or bare
`canary`; `edge` when it contains `xenia_edge` or `xenia-edge`; otherwise `master`
(grid_launcher/emulator/xenia.py:180, grid_launcher/emulator/xenia.py:185). Portable mode
is on when `<emulator_dir>/portable.txt` exists, or a `-portable`/`--portable` launch flag
is present (optionally followed by a boolean token), and defaults to
"canary on Windows" (grid_launcher/emulator/xenia.py:356). The storage root is the
launch `-storage_root` override, else the emulator directory in portable mode, else the
platform default (`~/Documents/Xenia` on Windows,
`~/Library/Application Support/Xenia` on macOS, `$XDG_DATA_HOME/Xenia` or
`~/.local/share/Xenia` elsewhere) (grid_launcher/emulator/xenia.py:279,
grid_launcher/emulator/xenia.py:363).

Cache directory naming differs by variant: `cache_host` for canary/edge, `cache` for
master (grid_launcher/emulator/xenia.py:376). Config file names are probed in
variant-specific order, e.g. `xenia-canary.config.toml`, `xenia-canary-config.toml`,
`xenia_canary.config.toml`, `xenia_canary-config.toml`, then the generic
`xenia.config.toml`, `xenia-config.toml` (grid_launcher/emulator/xenia.py:292).
Launch-argument overrides take precedence over `[storage] content_root`/`cache_root` read
from the TOML (grid_launcher/emulator/xenia.py:413).

**Save overrides** (grid_launcher/emulator/xenia.py:467): walk `content_root`; a
first-level entry matching 16 hex characters is a XUID and its 8-hex children are titles;
a first-level entry matching 8 hex characters is a title directly
(grid_launcher/emulator/xenia.py:455, grid_launcher/emulator/xenia.py:461). Within each
title directory, the existing subset of `00000001`, `Headers/00000001`, `profile` is
collected (grid_launcher/emulator/xenia.py:431).
`xenia_state_path_overrides` always returns `[]` (grid_launcher/emulator/xenia.py:489).

### Redream

`ensure_redream_settings(path)` (grid_launcher/emulator/redream.py:154) calls the reader
with an empty launch template and a no-op splitter, takes `config_path`, and forces
`mode=fullscreen` and `volume=40` (grid_launcher/emulator/redream.py:161). The format is
flat `key=value` with no spaces; lines are parsed on the first `=`
(grid_launcher/emulator/redream.py:168). Managed keys already at the desired value cause
an early `changed=False` return with no write
(grid_launcher/emulator/redream.py:180). On write, non-managed lines are preserved in
order and missing managed keys are appended
(grid_launcher/emulator/redream.py:185).

**Portable detection** (grid_launcher/emulator/redream.py:30): the emulator directory
counts as the data root when it contains any of `redream.cfg`, `flash.bin`,
`vmu0.bin`–`vmu3.bin`, or any `*.sav` / `*.png`. Candidate order is portable root →
platform default (`~/Library/Application Support/redream` on macOS,
`$XDG_DATA_HOME/redream`, `~/.local/share/redream`) but only when it exists or the
platform is macOS → the emulator directory as a fallback
(grid_launcher/emulator/redream.py:45, grid_launcher/emulator/redream.py:68).

**Save overrides** (grid_launcher/emulator/redream.py:110): the existing subset of
`vmu0.bin`…`vmu3.bin` in the data root — file paths, not a directory. State overrides
return `<data_root>/states` when it exists, followed by the data root itself
(grid_launcher/emulator/redream.py:132).

### FBNeo

Read-only (grid_launcher/emulator/fbneo.py:80). The config file is probed at
`<emulator_dir>/config/<exe stem>.ini`, then `config/fbneo.ini`, then
`config/FinalBurn Neo.ini` (grid_launcher/emulator/fbneo.py:35). The format is
whitespace-separated `key value` with `//`, `#`, `;` comments
(grid_launcher/emulator/fbneo.py:66). Recognized keys: `szAppEEPROMPath`
(default `config/games`), `szAppHiscorePath` (default `support/hiscores`),
`szAppHDDPath` (default `support/hdd`) (grid_launcher/emulator/fbneo.py:113).
`memcard_path` is always `<emulator_dir>/config/memcards` and `state_path` always
`<emulator_dir>/savestates` — neither is configurable
(grid_launcher/emulator/fbneo.py:96). When no path is supplied the base is the process
working directory (grid_launcher/emulator/fbneo.py:89).

Save overrides emit, in order, `eeprom_path`, `memcard_path`, `hiscore_path`, `hdd_path`
(grid_launcher/emulator/fbneo.py:141); state overrides emit `state_path`
(grid_launcher/emulator/fbneo.py:154).

### MAME

Read-only (grid_launcher/emulator/mame.py:172). `mame.ini` is searched in the
`-inipath` directories (semicolon-separated) when that option is present, else in
`<base>`, `<base>/ini`, `<base>/ini/presets` (grid_launcher/emulator/mame.py:115).
The INI format is whitespace-separated `key value`, keys lowercased, `#`/`;` comments
(grid_launcher/emulator/mame.py:161).

Recognized options, resolvable both from the launch template and from `mame.ini`, with
launch arguments winning (grid_launcher/emulator/mame.py:211):
`cfg_directory` (default `cfg`), `nvram_directory` (`nvram`),
`memcard_directory` (`memcard`), `diff_directory` (`diff`), `state_directory` (`sta`)
(grid_launcher/emulator/mame.py:204). Argument parsing strips leading dashes, accepts
`-opt value` and `-opt=value`, normalizes `-` to `_` in the option name, and refuses a
following token that itself starts with `-`
(grid_launcher/emulator/mame.py:97, grid_launcher/emulator/mame.py:102).

Save overrides emit `nvram_directory`, `memcard_directory`, `diff_directory`
(grid_launcher/emulator/mame.py:226); state overrides emit `state_directory`
(grid_launcher/emulator/mame.py:239).

### Pico-8

Read-only (grid_launcher/emulator/pico8.py:174). User-root candidates:
a `-home`/`--home` launch argument (grid_launcher/emulator/pico8.py:61); then
`<emulator_dir>`, `<emulator_dir>/pico-8`, `<emulator_dir>/userdata` — each only when it
contains `config.txt`, `cdata`, or `cstore` (grid_launcher/emulator/pico8.py:145); then
`%APPDATA%/pico-8` on Windows, `~/Library/Application Support/pico-8` on macOS, or both
`~/.lexaloffle/pico-8` and `$XDG_DATA_HOME/pico-8` on Linux
(grid_launcher/emulator/pico8.py:149).

`config.txt` is parsed as whitespace-separated `key value` with `#`, `;`, `--` comments
(grid_launcher/emulator/pico8.py:114). `root_path` overrides the carts root (default
`carts`) and `desktop` overrides the desktop path (default `desktop`)
(grid_launcher/emulator/pico8.py:209). `cdata`, `cstore` and `backup` roots are always
fixed sub-directories of the user root (grid_launcher/emulator/pico8.py:197).

Save overrides emit `cdata_root` then `cstore_root`
(grid_launcher/emulator/pico8.py:228). There is no state-path override.

### Vita3K

Read-only (grid_launcher/emulator/vita3k.py:9). `vita3k_pref_path` resolves in strict
priority: `<emulator_dir>/portable/` if it is a directory → the `pref-path:` scalar in
`<emulator_dir>/config.yml`, with one layer of matching quotes stripped and `~` expanded →
the platform default `~/.local/share/Vita3K/Vita3K` (Linux),
`~/AppData/Roaming/Vita3K/Vita3K` (Windows),
`~/Library/Application Support/Vita3K/Vita3K` (macOS)
(grid_launcher/emulator/vita3k.py:26, grid_launcher/emulator/vita3k.py:38,
grid_launcher/emulator/vita3k.py:48). An unrecognized platform yields `None`
(grid_launcher/emulator/vita3k.py:55). The file is read with
`errors="replace"` (grid_launcher/emulator/vita3k.py:34).

`vita3k_save_path_overrides` enumerates `<pref_path>/ux0/user/<NN>/savedata` for every
existing two-digit user directory in name order, and always prepends user `00` when it is
not already present — even if that directory does not exist
(grid_launcher/emulator/vita3k.py:58, grid_launcher/emulator/vita3k.py:88). The
`launch_template` and splitter parameters exist only for signature uniformity and are
unused (grid_launcher/emulator/vita3k.py:63).

### How overrides reach cloud sync

`_resolved_sync_directory_paths(emulator, key)` where `key` is `save_paths` or
`state_paths` (grid_launcher/ui/mixins/cloud_mixin.py:618):

1. Prefer the user's explicitly configured paths on the emulator entry; otherwise the
   profile's `save_directories`/`state_directories`
   (grid_launcher/ui/mixins/cloud_mixin.py:640).
2. Call `_ensure_emulator_sync_settings` first, so the config file exists before it is
   read (grid_launcher/ui/mixins/cloud_mixin.py:646).
3. Only when the user configured **no** paths, apply the emulator-specific override
   function, prepending its results to the list and deduplicating on the raw string
   (grid_launcher/ui/mixins/cloud_mixin.py:663). RetroArch instead prepends its
   directory-settings value and appends literal fallbacks
   (grid_launcher/ui/mixins/cloud_mixin.py:647).

Each override function takes the same three arguments: `(emulator_path, emulator_args, split_launch_template_args)`
(grid_launcher/ui/mixins/cloud_mixin.py:666). The TV/QML backend mirrors this dispatch
(grid_launcher/tv/bridge/cloud_helpers.py:11). Consumption of the resulting paths is
doc 06.

## Invariants and error handling

1. **Blank path is a no-op.** `_ensure_emulator_sync_settings` returns before any dispatch
   when the trimmed path is empty (grid_launcher/ui/mixins/emulator_ui_mixin.py:375). Most
   `ensure_*` functions repeat this check independently
   (grid_launcher/emulator/ppsspp.py:82, grid_launcher/emulator/pcsx2.py:178).
2. **Idempotency is enforced at two levels.** The session cache prevents a second dispatch
   for the same `name::path` (grid_launcher/ui/mixins/emulator_ui_mixin.py:379); and each
   writer only writes when `changed` is true, where `changed` means the produced text
   differs from what was there (grid_launcher/emulator/pcsx2.py:374,
   grid_launcher/emulator/duckstation.py:375, grid_launcher/emulator/xemu.py:319).
   Running any `ensure_*` twice on unchanged input reports `changed=False` on the second
   run and performs no write.
3. **Most `ensure_*` functions do not let exceptions escape — but PPSSPP can.** The other
   writers wrap their I/O in `except OSError` and return a null-ish result
   (grid_launcher/emulator/pcsx2.py:377, grid_launcher/emulator/azahar.py:213,
   grid_launcher/emulator/rpcs3.py:595, grid_launcher/emulator/eden.py:277,
   grid_launcher/emulator/redream.py:201). Cemu widens this to bare `except Exception`
   because XML parsing can raise non-`OSError`
   (grid_launcher/emulator/cemu.py:326). PPSSPP swallows errors around its `unlink` and its
   two writes (grid_launcher/emulator/ppsspp.py:94, :152) but has two UNPROTECTED
   `read_text` calls (grid_launcher/emulator/ppsspp.py:99, :156): an `OSError` or
   `UnicodeDecodeError` there propagates out of `ensure_ppsspp_settings`, and the dispatch in
   `_ensure_emulator_sync_settings` (grid_launcher/ui/mixins/emulator_ui_mixin.py:388-439)
   does not catch it either. RULED (milestone 5): a bug, fixed — see "Rust port deviations
   (milestone 5)" deviation 5. The port guards both reads instead of reproducing the crash.
4. **A failed write reports `changed=False`, not an error.** RetroArch and DuckStation
   return the pre-write settings dict with `changed=False`
   (grid_launcher/emulator/retroarch.py:348, grid_launcher/emulator/duckstation.py:380).
5. **RetroAchievements requires both fields.** Every RA-aware writer gates on
   `username and token` after stripping, so clearing either field removes future writes
   (grid_launcher/emulator/retroarch.py:296, grid_launcher/emulator/pcsx2.py:246,
   grid_launcher/emulator/ppsspp.py:127). Note that already-written credentials are not
   removed from the config file when the fields are cleared.
6. **Readers always return a fully populated dict.** No `*_directory_settings` function can
   return a partial map; unresolvable values are empty strings
   (grid_launcher/emulator/pcsx2.py:537, grid_launcher/emulator/dolphin.py:435).
7. **Path deduplication is case-insensitive** everywhere, via a shared `_unique_paths`
   helper duplicated in each module (grid_launcher/emulator/rpcs3.py:16,
   grid_launcher/emulator/dolphin.py:15, grid_launcher/emulator/xenia.py:91).
8. **Relative config values resolve against the emulator's data root**, after
   `os.path.expandvars` and `~` expansion (grid_launcher/emulator/pcsx2.py:497,
   grid_launcher/emulator/azahar.py:44, grid_launcher/emulator/mame.py:57).
9. **Argument-value parsing tolerates split quoting.** `_consume_arg_value` rejoins tokens
   until the closing quote is found, which matters when a splitter produced fragments
   (grid_launcher/emulator/xemu.py:36, grid_launcher/emulator/xenia.py:117).
   `_split_launch_args` returns `[]` on a `ValueError` from the splitter
   (grid_launcher/emulator/rpcs3.py:40).
10. **Section headers are matched case-insensitively; keys are matched case-sensitively.**
    `current_section.casefold() == target_key` versus `key in desired_values`
    (grid_launcher/emulator/pcsx2.py:88, grid_launcher/emulator/pcsx2.py:98). A user file
    with `[ui]` is found, but `startfullscreen` would be written a second time as
    `StartFullscreen`.
11. **Duplicate keys are collapsed by the overwrite-policy writers only.** In the INI
    section writers and the RetroArch flat writer, the second and later occurrences of a
    managed key within a section are deleted and mark the file as changed
    (grid_launcher/emulator/pcsx2.py:99, grid_launcher/emulator/retroarch.py:321). The
    add-only writers (RPCS3 YAML, grid_launcher/emulator/rpcs3.py:152; Xemu TOML,
    grid_launcher/emulator/xemu.py:224) leave existing duplicates untouched.
12. **The INI section writers and the RetroArch flat writer normalize trailing
    whitespace**: `rstrip()` then exactly one `"\n"`
    (grid_launcher/emulator/pcsx2.py:122, grid_launcher/emulator/retroarch.py:347), so a
    file that ended without a newline gains one on the first write. Redream joins lines
    with a plain trailing `"\n"` and no `rstrip()` (grid_launcher/emulator/redream.py:198);
    the Cemu XML write (grid_launcher/emulator/cemu.py:322-323) and the PPSSPP
    `ppsspp_retroachievements.dat` token write (grid_launcher/emulator/ppsspp.py:159) do not
    normalize at all.

## Platform differences

| Concern | Windows | macOS | Linux |
|---|---|---|---|
| RetroArch core extension | `*.dll` | `*.dylib` | `*.so` | 
| — anchor | grid_launcher/emulator/retroarch.py:512 | grid_launcher/emulator/retroarch.py:514 | grid_launcher/emulator/retroarch.py:516 |
| PCSX2 Documents folder | `SHGetKnownFolderPath(FOLDERID_Documents)` | `~/Documents` | `~/Documents` |
| — anchor | grid_launcher/emulator/pcsx2.py:37 | grid_launcher/emulator/pcsx2.py:161 | grid_launcher/emulator/pcsx2.py:161 |
| Dolphin user root | registry `HKCU\Software\Dolphin Emulator`, `%APPDATA%`, OneDrive/USERPROFILE Documents | `~/Library/Application Support/Dolphin` | `~/.dolphin-emu`, Flatpak data dir |
| — anchor | grid_launcher/emulator/dolphin.py:80, grid_launcher/emulator/dolphin.py:125 | grid_launcher/emulator/dolphin.py:138 | grid_launcher/emulator/dolphin.py:137, grid_launcher/emulator/dolphin.py:141 |
| Azahar user root | `%APPDATA%/Azahar` | `~/Library/Application Support/Azahar` | `$XDG_DATA_HOME/Azahar`, `~/.local/share/Azahar`, Flatpak data dir |
| — anchor | grid_launcher/emulator/azahar.py:239 | grid_launcher/emulator/azahar.py:251 | grid_launcher/emulator/azahar.py:243, grid_launcher/emulator/azahar.py:254 |
| Eden config | `%APPDATA%/eden/config/qt-config.ini` | `$XDG_CONFIG_HOME/eden/…` | `$XDG_CONFIG_HOME/eden/…` |
| — anchor | grid_launcher/emulator/eden.py:211 | grid_launcher/emulator/eden.py:208 | grid_launcher/emulator/eden.py:208 |
| Cemu settings | `%APPDATA%/Cemu`, `%LOCALAPPDATA%/Cemu` | `$XDG_CONFIG_HOME/Cemu` | `$XDG_CONFIG_HOME/Cemu` |
| — anchor | grid_launcher/emulator/cemu.py:264 | grid_launcher/emulator/cemu.py:269 | grid_launcher/emulator/cemu.py:269 |
| Cemu controller profile | XInput mapping | SDLController mapping | SDLController mapping |
| — anchor | grid_launcher/emulator/cemu.py:347 | grid_launcher/emulator/cemu.py:349 | grid_launcher/emulator/cemu.py:349 |
| Xemu base root | `%APPDATA%/xemu/xemu` | `~/Library/Application Support/xemu/xemu` | `$XDG_DATA_HOME/xemu/xemu`, `~/.local/share/xemu/xemu` |
| — anchor | grid_launcher/emulator/xemu.py:122 | grid_launcher/emulator/xemu.py:128 | grid_launcher/emulator/xemu.py:130 |
| Xenia storage root | `~/Documents/Xenia`; canary defaults to portable | `~/Library/Application Support/Xenia` | `$XDG_DATA_HOME/Xenia`, `~/.local/share/Xenia` |
| — anchor | grid_launcher/emulator/xenia.py:281, grid_launcher/emulator/xenia.py:357 | grid_launcher/emulator/xenia.py:283 | grid_launcher/emulator/xenia.py:286 |
| Redream default root | *(no Windows branch; falls through to the Linux branch)* | `~/Library/Application Support/redream` | `$XDG_DATA_HOME/redream`, `~/.local/share/redream` |
| — anchor | grid_launcher/emulator/redream.py:45 | grid_launcher/emulator/redream.py:47 | grid_launcher/emulator/redream.py:50 |
| Pico-8 user root | `%APPDATA%/pico-8` | `~/Library/Application Support/pico-8` | `~/.lexaloffle/pico-8`, `$XDG_DATA_HOME/pico-8` |
| — anchor | grid_launcher/emulator/pico8.py:151 | grid_launcher/emulator/pico8.py:156 | grid_launcher/emulator/pico8.py:158 |
| Vita3K pref path | `~/AppData/Roaming/Vita3K/Vita3K` | `~/Library/Application Support/Vita3K/Vita3K` | `~/.local/share/Vita3K/Vita3K` |
| — anchor | grid_launcher/emulator/vita3k.py:51 | grid_launcher/emulator/vita3k.py:53 | grid_launcher/emulator/vita3k.py:49 |

Several modules append macOS paths on *every* non-Windows platform rather than gating on
`darwin` — Azahar, Eden and PCSX2 all list `~/Library/Application Support/…` in the Linux
branch (grid_launcher/emulator/azahar.py:251, grid_launcher/emulator/eden.py:352,
grid_launcher/emulator/pcsx2.py:472). This is harmless because the paths simply do not
exist, but a port must not "fix" it into a strict platform switch without checking the
candidate ordering tests.

Flatpak-specific candidate paths are hardcoded for Azahar
(`~/.var/app/org.azahar_emu.Azahar/...`, grid_launcher/emulator/azahar.py:137) and Dolphin
(`~/.var/app/org.DolphinEmu.dolphin-emu/...`, grid_launcher/emulator/dolphin.py:141).

## Concurrency

- `_ensure_emulator_sync_settings` and every `ensure_*` function run **synchronously on the
  UI thread** at every call site listed above. There is no locking, no queue, and no
  worker offload (grid_launcher/ui/mixins/emulator_ui_mixin.py:365). The function is
  wrapped in optional timing instrumentation, which is the only concession to its cost
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:370).
- The `_emulator_sync_settings_done` set is a plain `set[str]` with no lock
  (grid-launcher.py:431). Because all mutation happens on the UI thread this is safe as
  written, but a port with a different threading model must add synchronization.
- Two background paths exist, both `threading.Thread(..., daemon=True)`:
  - PS3 firmware download (grid_launcher/ui/mixins/emulator_ui_mixin.py:1787),
  - source-emulator firmware install (grid_launcher/ui/mixins/emulator_ui_mixin.py:1914).
  Neither writes config files. Both communicate with the UI **only** through Qt signals:
  `_firmware_download_progress`, `_firmware_download_done`, `_emulator_refresh_requested`
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:1769,
  grid_launcher/ui/mixins/emulator_ui_mixin.py:1784). The refresh signal is connected once
  in `MainWindow.__init__` (grid-launcher.py:462).
- The worker loop in `_trigger_firmware_install_for_source_emulator` swallows every
  exception per platform id so one failure cannot abort the remaining installs
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:1910).
- Caches invalidated on config save: `_emulator_sync_settings_done`,
  `_sync_directory_paths_cache`, `_cloud_emulator_entry_cache` (grid-launcher.py:3150).
  The RetroArch installed-core cache is cleared separately
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:642).

## Test oracle

| Test file | What it pins down |
|---|---|
| tests/test_emulator_autoconfig_settings.py | 177 tests across every `ensure_*` writer. RetroArch defaults and audio-volume preservation (tests/test_emulator_autoconfig_settings.py:54, :72); netplay nickname presence/absence (:87, :101); DuckStation `portable.txt` creation and non-overwrite (:145, :157) and portable write target (:171); PCSX2 `portable.ini` creation/non-overwrite (:201, :215), fullscreen gating (:278, :295), RA credential gating (:311), BIOS directory (:327, :345), and per-key preservation of Pad1/hotkey/volume/upscale (:384, :419, :453, :487); Dolphin two-file write (:544), `portable.txt` (:568, :580), SkipIPL (:609–:682), GCPad append/skip semantics including case-insensitive `[GCPad1]` (:690–:767); Azahar companion-key non-duplication (:848) and `user/` creation (:864, :876); Eden annotation format (:948), `confirmStop=2` (:985), forced audio volume overwrite (:1062); Xemu key-by-key add-only behavior (:1101–:1259); PPSSPP `installed.txt` deletion (:1259) and RA credential file (:1301, :1333); Redream idempotency and value update (:1368, :1384); Cemu forced-value enforcement, unmanaged-setting preservation, and no-op case (:1515–:1755); RPCS3 portable resolution, YAML preservation, GuiSettings/CurrentSettings overwrite, `changed` on first vs second run, and data-root candidate ordering (:1800–:1975) |
| tests/test_emulator_autoconfig_settings.py (dispatch) | `_ensure_emulator_sync_settings` reaching `ensure_ppsspp_settings` (:2005) and the RA-login fan-out including the no-sync-on-error case (:2062, :2103); RPCS3 background firmware skip/start/RomM-fallback matrix (:2144–:2466); `xemu_missing_bios_files` (:2467); `ensure_rpcs3_vfs_settings` write/idempotency/non-overwrite (:2508–:2586) and `ps3_vfs_*_path` resolution and fallbacks (:2587–:2654); `update_rpcs3_games_yml` layouts, update-in-place, and idempotency (:2748–:2903); custom-config copy (:2904); default backfill non-overwrite (:3009, :3022) |
| tests/test_retroarch_config.py | Sorting disabled and defaults written (:25); explicit directories preserved (:65); fullscreen + RA credentials (:97); cheevos leaderboard defaults (:120); XDG discovery and native-Linux credential write (:149, :156); missing config returns unchanged (:176); empty path yields no candidates (:186); core capability flag defaults and merging (:210–:253); core-id extension stripping including uppercase (:254–:270); installed-core discovery per platform, explicit `cores_dir`, AppImage-home priority, sibling fallback (:272–:392); platform-key fuzzy matching with the 0.7 threshold (:393–:428); slug→core map loading and invalid-entry skipping (:429–:474) |
| tests/test_duckstation_config.py | Forced per-game memory-card defaults (:17); explicit memcard directory preserved (:52); fullscreen and cheevos defaults (:79); AutoUpdater disabled (:112); GPU/display/audio/hotkey/Pad1 preserve-if-present pairs (:127–:355); `SetupWizardIncomplete` forced from `true` to `false` (:370); existing `[Cheevos]` credentials preserved through a portable write (:389); candidate ordering under XDG overrides and dotfile fallbacks (:421–:446) |
| tests/test_flycast_vmu.py | `vmu_shared_saves` true for flycast, false otherwise (:41, :45); VMU candidate discovery, newest-per-slot dedup, non-VMU rejection, missing directory (:49–:92); cloud scope becomes shared-slotted for save type but stays per-game for state type and for non-RetroArch emulators (:93–:122); `retroarch_core_flags_for_platform` lookup by platform name, case-insensitively (:123–:139) |
| tests/test_vita3k.py | Pref-path priority: portable over `config.yml` (:21), quoted values (:39, :47), `~` expansion (:55), missing key falls through (:63), the three platform defaults and the unknown-platform `None` (:72–:99); save enumeration with user `00` always present and prepended (:111, :138), multi-user listing (:127), non-two-digit directory exclusion (:147), unused launch template (:164) |
| tests/test_emulator_profiles.py | Every `*_directory_settings` / `*_save_path_overrides` / `*_state_path_overrides` reader for Azahar, Cemu, Eden, FBNeo, Dolphin, MAME, PCSX2, Pico-8, Redream, RPCS3, Xemu, Xenia (tests/test_emulator_profiles.py:13–:63) |

## Open questions

- `OPEN QUESTION:` `.claude/skills/emulator-autoconfig/SKILL.md:150` lists DuckStation as a
  RetroAchievements target with a `[Cheevos]` section. The current
  `ensure_duckstation_memory_card_settings` takes no credential parameters and writes only
  the four suppression keys, never `Username`/`Token`
  (grid_launcher/emulator/duckstation.py:198, grid_launcher/emulator/duckstation.py:363).
  Intended, or a regression? A port should follow the code.
  **RULED (milestone 5): follow-the-code.** `ensure_duckstation_memory_card_settings`
  (`crates/grid-core/src/autoconfig/duckstation.rs`) takes no credential parameters either,
  and DuckStation is not RA-capable in this milestone's `ra_capable`
  (`crates/grid-core/src/autoconfig/mod.rs:320-322`).
- `OPEN QUESTION:` The same skill file states a "non-overwrite invariant" for all
  `_ensure_*_section_values` helpers
  (.claude/skills/emulator-autoconfig/SKILL.md:51). Only the RPCS3 YAML writer and the Xemu
  TOML writer are actually add-only (grid_launcher/emulator/rpcs3.py:152,
  grid_launcher/emulator/xemu.py:224); the seven INI writers overwrite. Which is the
  intended contract? **RULED (milestone 5): the three write policies, as documented in the
  Behavior section's policy table above, are the ported contract** —
  `writers::ini_overwrite_section`, `writers::yaml_add_only_section`/
  `writers::toml_add_only_section`, and the append-if-absent block writer
  (`crates/grid-core/src/autoconfig/writers.rs`) implement exactly those three shapes; the
  skill doc's blanket "non-overwrite invariant" claim was wrong.
- `OPEN QUESTION:` `ensure_pcsx2_settings` computes `emulator_dir` from the *raw*
  `emulator_path_text` without `.expanduser()` or `.strip()`
  (grid_launcher/emulator/pcsx2.py:186), while the existence check two lines above uses the
  expanded path (grid_launcher/emulator/pcsx2.py:182). A `~`-prefixed path would create
  `portable.ini` and the INI under a literal `~` directory. Should the expanded path be
  used? **RULED (milestone 5): fixed** — see "Rust port deviations (milestone 5)" deviation 4.
- `OPEN QUESTION:` `ensure_duckstation_memory_card_settings` parses existing settings from
  whichever candidate matched, but always writes `<emulator_dir>/settings.ini`
  (grid_launcher/emulator/duckstation.py:214, grid_launcher/emulator/duckstation.py:222).
  When a user's real config lives in `~/.config/duckstation`, its values are copied into a
  new portable file that then shadows the original. Intended migration, or a bug?
  **RULED (milestone 5): follow-the-code**, reproduced as-is
  (`crates/grid-core/src/autoconfig/duckstation.rs`) — not fixed this milestone.
- `OPEN QUESTION:` `ensure_dolphin_settings` picks candidate index 0 unconditionally when a
  path is supplied (grid_launcher/emulator/dolphin.py:270), whereas
  `ensure_dolphin_skip_ipl` picks the first *existing* candidate
  (grid_launcher/emulator/dolphin.py:325). The two can therefore target different files for
  the same emulator. Is the divergence deliberate? **RULED (milestone 5): follow-the-code**,
  reproduced as-is — both functions keep the same target divergence in
  `crates/grid-core/src/autoconfig/dolphin.rs`, not unified.
- `OPEN QUESTION:` `ensure_rpcs3_settings` always writes into `<exe_dir>/portable/`, even
  when the installation is non-portable and `rpcs3_data_root_candidates` would prefer
  `$RPCS3_CONFIG_DIR` or `~/.config/rpcs3`
  (grid_launcher/emulator/rpcs3.py:542 vs grid_launcher/emulator/rpcs3.py:605). Writers and
  readers can disagree about which data root is live. **RULED (milestone 5):
  follow-the-code**, reproduced as-is — the always-portable write target is kept in
  `crates/grid-core/src/autoconfig/rpcs3.rs`.
- `OPEN QUESTION:` In `ensure_xemu_settings`, the `misc` section result is assigned to
  `changed` (shadowing the general-section result) before being re-ORed with `gen_changed`
  (grid_launcher/emulator/xemu.py:264). The outcome is correct today but the pattern is
  fragile; is a single accumulator intended? **RULED (milestone 5): yes, a single
  accumulator** — `xemu::ensure_settings` (`crates/grid-core/src/autoconfig/xemu.rs`) folds
  every section's `changed` bit into one running `bool` via the same `apply_section`-style
  pattern used throughout `writers.rs` callers, rather than reproducing the shadow-then-re-OR
  pattern.
- `OPEN QUESTION:` `ensure_retroarch_save_location_settings` rebinds the `username`
  parameter from the RomM nickname to the RetroAchievements username mid-function
  (grid_launcher/emulator/retroarch.py:287 vs grid_launcher/emulator/retroarch.py:294).
  A port should keep two distinct variables; is the rebinding intentional? **RULED
  (milestone 5): yes, two distinct variables** — `retroarch::ensure_settings`
  (`crates/grid-core/src/autoconfig/retroarch.rs`) takes the RomM username and RA credentials
  as separate parameters and never rebinds one to the other.
- `OPEN QUESTION:` `_ensure_emulator_sync_settings` caches on `name::path` only. If a
  file is externally modified without a config save, the launcher will not rewrite it for
  the rest of the session (grid_launcher/ui/mixins/emulator_ui_mixin.py:379,
  grid-launcher.py:3150). Is a stronger invalidation key (e.g. file mtime) wanted? **RULED
  (milestone 5): mooted.** Deviation 1 removes the session cache entirely — writers now run
  once per new entry only, so there is nothing left to invalidate
  (`crates/grid-core/src/autoconfig/mod.rs:479`).
- `OPEN QUESTION:` RetroAchievements credentials are written but never removed. Clearing
  both config fields stops future writes but leaves `cheevos_username`/`cheevos_token`,
  `[Achievements] Username/Token`, and `ppsspp_retroachievements.dat` in place
  (grid_launcher/emulator/retroarch.py:296, grid_launcher/emulator/ppsspp.py:155). Should
  clearing credentials actively scrub them? **RULED (milestone 5): no-scrub-on-clear.** The
  port's narrow `ensure_*_ra_credentials` writers only ever ADD the RA keys when both fields
  are non-blank; `fan_out_ra_credentials` (`crates/grid-core/src/autoconfig/mod.rs:334-341`)
  short-circuits to an empty result on a blank pair and writes (and scrubs) nothing —
  matching the reference's behavior deliberately, not left open.
- `OPEN QUESTION:` `retroarch_cores_for_platform` returns the hardcoded
  `["fbneo", "mame2003_plus"]` when the compatibility map is empty
  (grid_launcher/emulator/retroarch.py:468), but `[]` when the map is populated and nothing
  matches (grid_launcher/emulator/retroarch.py:478). Is the arcade-biased fallback still
  desired for a missing core list? **RULED (milestone 5): follow-the-code**, reproduced
  as-is in `crates/grid-core/src/autoconfig/retroarch.rs`.
- `OPEN QUESTION:` `ensure_eden_settings` returns `config_path` as a `Path`
  (grid_launcher/emulator/eden.py:280) and `ensure_pcsx2_settings` returns it as a `Path`
  too (grid_launcher/emulator/pcsx2.py:380), while all the others return `str`
  (grid_launcher/emulator/azahar.py:216, grid_launcher/emulator/xemu.py:325). What is the
  intended return type for a port with a static type system? **RULED (milestone 5):
  `EnsureResult`** — see "Rust port deviations (milestone 5)" deviation 8.
- `OPEN QUESTION:` No autoprofile ships for Dolphin, MAME, Flycast or Vita3K's siblings
  even though dedicated modules exist for them (`emulator-autoprofiles.json` contains 21
  profiles, none named Dolphin or MAME). Those emulators can therefore only be added
  manually, and are matched by the substring fallback in `_emulator_matches_tokens`
  (grid_launcher/ui/mixins/cloud_mixin.py:1362). Is this intentional? Still open — this
  milestone does not add or remove autoprofiles.

## Source map

| File | Responsibility |
|---|---|
| grid_launcher/emulator/autoconfig.py | Entry autoconfig: emulator entry creation/update from a profile, platform and RetroArch-core default assignment, manual-entry defaults, executable selection, install directory naming |
| grid_launcher/ui/mixins/emulator_ui_mixin.py | `_ensure_emulator_sync_settings` dispatch, session idempotency cache, RA credential plumbing, RetroArch core-list/slug-map/installed-core caches, background firmware threads, default backfill |
| grid_launcher/ui/mixins/cloud_mixin.py | `_resolved_sync_directory_paths` — the only consumer of `*_save_path_overrides` / `*_state_path_overrides`; `_emulator_matches_tokens` and every `_is_*_emulator_name` predicate; `_resolved_firmware_directories` |
| grid_launcher/ui/mixins/install_mixin.py | Post-install firmware/config/saves placement driven by RetroArch core metadata; Dolphin SkipIPL and GCPad hooks |
| grid_launcher/tv/bridge/cloud_helpers.py | TV/QML mirror of the override dispatch |
| grid_launcher/emulator/retroarch.py | `retroarch.cfg` writer, config/core-id/platform-key normalization, compatibility and slug maps, installed-core discovery, core capability flags, Flycast VMU discovery |
| grid_launcher/emulator/duckstation.py | `settings.ini` writer and memory-card reader |
| grid_launcher/emulator/pcsx2.py | `PCSX2.ini` writer, Windows Documents resolution, portable data roots, memcard/savestate overrides |
| grid_launcher/emulator/dolphin.py | `Dolphin.ini`/`GFX.ini`/`GCPadNew.ini` writers, registry and portable user-root resolution, GC/Wii save enumeration |
| grid_launcher/emulator/azahar.py | `qt-config.ini` writer, NAND/SDMC title-group save enumeration |
| grid_launcher/emulator/eden.py | `qt-config.ini` writer with generated `\default` annotations, alternate app-name roots, NAND user-save enumeration, keys/firmware probes |
| grid_launcher/emulator/rpcs3.py | `config.yml`, `GuiSettings.ini`, `CurrentSettings.ini`, `vfs.yml`, `games.yml` writers; data-root candidates; active-user resolution; savedata enumeration; firmware install spawn; custom-config copy |
| grid_launcher/emulator/ppsspp.py | `PPSSPP.INI` writer, `installed.txt` removal, RA token file |
| grid_launcher/emulator/cemu.py | `settings.xml` template and forced elements, controller profile templates, MLC-derived save roots |
| grid_launcher/emulator/xemu.py | `xemu.toml` add-only writer, base-path candidates, TOML reader with dotted/inline-table flattening, BIOS presence check |
| grid_launcher/emulator/xenia.py | STFS content installer, variant/portable detection, storage/content/cache root resolution, save enumeration |
| grid_launcher/emulator/redream.py | `redream.cfg` writer, portable marker detection, VMU and state paths |
| grid_launcher/emulator/fbneo.py | `fbneo.ini` reader, EEPROM/memcard/hiscore/HDD/state paths |
| grid_launcher/emulator/mame.py | `mame.ini` and launch-argument option reader, cfg/nvram/memcard/diff/state paths |
| grid_launcher/emulator/pico8.py | `config.txt` reader, user-root candidates, cdata/cstore paths |
| grid_launcher/emulator/vita3k.py | Pref-path resolution and per-user savedata enumeration |
| grid_launcher/emulator/profiles.py | Autoprofile file location, load, and normalization (details in doc 02) |
| grid_launcher/core/path.py | `xdg_config_home`, `xdg_data_home`, `grid_launcher_share_dir` |
| emulator-autoprofiles.json | 21 shipped profiles driving entry autoconfig and platform defaults |
| retroarch-core-list.json | 233 core entries: platforms, capability flags, firmware/config/saves metadata |
| romm-platform-cores.json | 75 RomM platform slugs → preferred core id lists |

## Rust port deviations (milestone 5)

Deliberate deviations from the reference when porting emulator autoconfig (settings writers,
entry/defaults autoconfig, RetroAchievements credential fan-out) to Rust (grid-core). Rust paths
are relative to `rewrite/`.

1. **Trigger policy.** `ensure_*` writers and entry autoconfig run only when a NEW emulator
   entry is created — catalog install
   (`crates/grid-core/src/library/mod.rs:961` calls `sync_autoconfig`, itself calling
   `autoconfig::sync_new_emulator` at `crates/grid-core/src/library/mod.rs:995`) or manual add
   (`app/src-tauri/src/commands.rs:225`, `save_emulator`, on an ADD). Never on edits, launches
   or view refreshes — the reference's six call sites (launch, standalone launch, entry-dialog
   save, cloud-sync directory resolution, post-RA-login for every registered emulator, and
   post-entry-autoconfig) collapse to these two. The `name::path` session cache
   (`grid_launcher/ui/mixins/emulator_ui_mixin.py:379`) is gone —
   `crates/grid-core/src/autoconfig/mod.rs:479` notes there is nothing left to deduplicate once
   the writers only ever run once per entry, which also moots the "stronger invalidation"
   open question.
2. **RA credential fan-out.** Saving credentials runs a dedicated narrow writer per RA-capable
   module — `ensure_*_ra_credentials`, dispatched by `fan_out_ra_credentials`
   (`crates/grid-core/src/autoconfig/mod.rs:334`) — that touches only the RA keys
   (`retroarch::ensure_ra_credentials`, `pcsx2::ensure_ra_credentials`,
   `ppsspp::ensure_ra_credentials`). Clearing still writes nothing and scrubs nothing (see
   ruling on the no-scrub-on-clear open question below).
3. **Defaults backfill** runs at the same two trigger points as (1), immediately after entry
   autoconfig — `entry::backfill_missing_defaults` is called once, from inside
   `sync_new_emulator` (`crates/grid-core/src/autoconfig/mod.rs:486`, `:534`) — not on every
   emulator view refresh. It also re-runs, across every registered entry, when the platform list
   first arrives: `autoconfig::backfill_all_defaults` (`crates/grid-core/src/autoconfig/mod.rs:648`)
   is called from the `list_platforms` command
   (`app/src-tauri/src/commands.rs:71-115`) once the fetched list is non-empty, closing the gap
   where an emulator added before the first successful platform fetch would otherwise get no
   platform/core defaults until the next add or install.
4. **`ensure_pcsx2_settings` uses the expanded, trimmed path throughout**
   (`expand_user(path.trim()).parent()`), fixing a reference bug: `emulator_dir` there is
   computed from the RAW, unexpanded `emulator_path_text`
   (`grid_launcher/emulator/pcsx2.py:186`), so a `~`-prefixed
   path creates a literal `~` directory instead of resolving through `$HOME`. The port's fix is
   `crates/grid-core/src/autoconfig/pcsx2.rs:8-14` (doc comment) and `:34-45`
   (`resolve_target`), pinned by
   `pcsx2_expands_a_tilde_path_and_creates_no_literal_tilde_directory`
   (`crates/grid-core/src/autoconfig/pcsx2.rs:440`).
5. **PPSSPP's two unprotected reads are guarded.** `ppsspp.py:99` and `ppsspp.py:156` read with
   no `try`/`except` at all, so an unreadable existing `PPSSPP.INI` or `.dat` file crashes
   `ensure_ppsspp_settings` and propagates out of the sync dispatch
   (`grid_launcher/ui/mixins/emulator_ui_mixin.py:388-439` does not catch it either — this is
   the inline `OPEN QUESTION` at line 1142 above, now ruled: fixed). The port wraps both reads
   in `read_guarded` (`crates/grid-core/src/autoconfig/ppsspp.rs:60-65`, doc comment at
   `:8-13`): an unreadable INI yields `changed=false` instead of propagating, exactly like
   every other writer's I/O failure.
6. **PCSX2 `[Folders] Bios` is not written** (`crates/grid-core/src/autoconfig/pcsx2.rs:16`
   declares D6; the call site at `crates/grid-core/src/autoconfig/mod.rs:579` is a bare
   comment, no code) — the firmware subsystem is deferred to its own milestone, which also
   owns closing this.
7. **The RPCS3 background firmware download** (`PS3UPDAT.PUP` fetch and the `--installfw`
   spawn, `grid_launcher/emulator/rpcs3.py:365` `trigger_rpcs3_firmware_install`) **is out**,
   same deferral (`crates/grid-core/src/autoconfig/mod.rs:597`).
   **Superseded (milestone 8):** the firmware subsystem landed. RPCS3 firmware now downloads
   from the RomM server as a drawer job — `FirmwareService::spawn_ps3_firmware`
   (`app/src-tauri/src/firmware_service.rs:295`), triggered on adding/configuring an RPCS3
   entry — rather than a direct-from-Sony background thread (see doc 03's D2 and D6, and
   doc 04's D10). The deferral test `sync_starts_no_firmware_download` and its pinning
   `// No background firmware download — D7.` comment were deleted along with it.
8. **Every `ensure_*` returns one `EnsureResult { changed, config_path, extras }`**
   (`crates/grid-core/src/autoconfig/mod.rs:93-110`); the reference's `str`-vs-`Path`-vs-`dict`
   mix (`grid_launcher/emulator/eden.py:280`, `grid_launcher/emulator/pcsx2.py:380` return
   `Path`; `grid_launcher/emulator/azahar.py:216`, `grid_launcher/emulator/xemu.py:325` return
   `str`) was a dynamic-typing artifact, not a behavior difference.
9. **`apply_xenia_content_without_ui`** (the STFS content installer, `xenia.py:36`) **is out of
   scope**: this milestone ports Xenia's readers only. `copy_ps3_custom_config_to_emulator`
   (`rpcs3.py:746`) and `trigger_rpcs3_firmware_install` (`rpcs3.py:365`) are likewise unported
   — none of the three has any Rust counterpart anywhere under `crates/grid-core/src`.
10. **The readers are ported and unit-tested but have no caller yet.** `readers.rs` (5890
    lines) is declared at `crates/grid-core/src/autoconfig/mod.rs:21` and used only by its own
    `#[cfg(test)]` module; no other file in the crate imports `autoconfig::readers`. Milestone
    6 (cloud saves) is their consumer.
11. **Cemu's `settings.xml` edit is byte-preserving** (targeted text replacement, doc comment
    at `crates/grid-core/src/autoconfig/cemu.rs:9-18`) rather than an XML reserialization —
    the reference uses `xml.etree.ElementTree`, whose reserialization normalizes whitespace.
    This is a strictly stronger form of the reference's "everything else is preserved"
    guarantee (pinned by the byte-for-byte-untouched test at
    `crates/grid-core/src/autoconfig/cemu.rs:650`) and avoids adding an XML dependency for six
    element writes.
12. **Entry autoconfig preserves an existing entry's `source_id`, `source_provider`,
    `source_owner`, `source_repo` and `source_release_tag`**
    (`crates/grid-core/src/autoconfig/entry.rs:400-404`, inside
    `auto_configure_emulator_settings`; doc comment at `:302-305` names this D12); the
    reference rebuilds the entry with eight keys (`autoconfig.py:524-554`) and has no
    equivalent of these install-provenance fields.
13. **`Config` reuses the existing `retroarch_cores` map for core defaults**
    (`crates/grid-core/src/config.rs:77`; the reference's `default_retroarch_cores`); no
    second map was added. `EmulatorEntry` gains `save_strategy`, `ignore_files`,
    `ignore_extensions`, `save_paths`, and `state_paths`
    (`crates/grid-core/src/config.rs:34-48`), and `EmulatorProfile` gains the five autoprofile
    fields that feed them — `save_strategy`, `save_directories`, `state_directories`,
    `ignore_files`, `ignore_extensions` (`crates/grid-core/src/launch/profiles.rs:28-44`).
14. **The assignable server-platform list reaches grid-core through
    `InstallService::set_known_platforms`** (`crates/grid-core/src/library/mod.rs:371`), fed
    by the `list_platforms` command (`app/src-tauri/src/commands.rs:71-115`); with no connected
    session the list is empty and the platform-defaults step is a no-op. See deviation 3 above
    for the backfill re-run this same command now triggers once that list is non-empty.
15. **`sync_new_emulator`'s entry-autoconfig step uses `apply_manual_emulator_profile_defaults`
    at BOTH D1 sites** (`crates/grid-core/src/autoconfig/mod.rs:509-516`): layer 1's
    `auto_configure_emulator_settings` rebuild path (`crates/grid-core/src/autoconfig/entry.rs:317`)
    is unreachable in the rewrite because `finalize_emulator` already writes the profile-named
    entry before the sync runs (`crates/grid-core/src/library/mod.rs:960-961`).
    `auto_configure_emulator_settings` is retained reference-only — its own doc comment at
    `crates/grid-core/src/autoconfig/entry.rs:307-316` records this — exercised by its own
    tests (`crates/grid-core/src/autoconfig/entry.rs:682`) and called from no production
    path.
    Site B (manual add, `app/src-tauri/src/commands.rs:225`) is exact Python parity, and it is
    what decides the design; at site A (catalog install) the two functions' outputs are
    equivalent, because the entry was just written from the same profile.
    Field nuance: a blank-`args` profile leaves the entry's `args` blank instead of writing
    `"%rom%"`. Three catalog profiles have blank `args` in `emulator-autoprofiles.json`
    (`ShadPS4 Qt Launcher`, `GE-Proton`, `Proton-CachyOS`), of which only the first is
    reachable — `profile_for_entry` (`crates/grid-core/src/launch/profiles.rs:243-259`) skips
    compat-tool profiles outright (`:257-259`). The difference is launch-identical:
    `template::build_args` (`crates/grid-core/src/launch/template.rs:334-344`) substitutes
    `"%rom%"` for a blank `entry_args` at launch time (`launch.py:150`), and this is pinned by
    `build_args_blank_entry_defaults_to_rom_and_appends_global`
    (`crates/grid-core/src/launch/template.rs:578`).

### Additional deviations noted during review

16. **The Cemu `mlc_path` reader is a comment-aware but non-structural regex scan**, not the
    Python reader's structural `ET.fromstring(...)` parse. `cemu_directory_settings`
    (`grid_launcher/emulator/cemu.py:361-384`) genuinely parses XML with
    `node.findtext("mlc_path")`; the port has no XML dependency, so
    `cemu_mlc_path_from_xml` (`crates/grid-core/src/autoconfig/readers.rs:2117-2128`) instead
    regex-scans for `<mlc_path>...</mlc_path>`, skipping any match that falls inside an XML
    comment via the same `cemu::comment_ranges`/`position_is_commented_out` helpers the D11
    writer uses. Documented as its own deviation, separate from D11, in the doc comment at
    `crates/grid-core/src/autoconfig/readers.rs:2097-2104`.
17. **Azahar, Eden and Xenia's blank-path gates are real emptiness checks**, deliberately
    diverging from the reference's accidental CWD probing. In each Python reader, an
    `if emulator_dir:` guard (Azahar `azahar.py:256-257`, Eden `eden.py:356-357`) is always
    truthy
    because a bare `Path()` has no `__bool__` override, and Xenia's `if str(candidate)`
    (`xenia.py:397`) is always truthy because `str(Path())` is the non-empty string `"."` — so
    all three unconditionally append an accidental CWD-relative candidate
    (`<cwd>/user` or `<cwd>/xenia.config.toml`) even for a blank emulator path. The port's
    `emulator_dir` is a genuinely empty `PathBuf` for a blank path, so the equivalent gates
    (`!emulator_dir.as_os_str().is_empty()` at
    `crates/grid-core/src/autoconfig/readers.rs:1794` (Azahar), `:1965` (Eden), `:2964`
    (Xenia)) never fire for a blank path and no CWD-relative candidate is ever produced.
    Deliberate, tested
    (`azahar_blank_path_does_not_probe_cwd`, `eden_blank_path_does_not_probe_cwd`,
    `xenia_blank_path_does_not_probe_cwd`), and doc-commented at each gate.
18. **A frontend `saveForm` bug from milestone 4 is fixed in passing.** The edit-save path in
    `app/src/lib/Emulators.svelte` used to build the saved entry from only `name`, `path` and
    `args`, dropping every other field — including the milestone-4 `source_*` provenance —
    on every edit-save. `saveForm` (`app/src/lib/Emulators.svelte:266-271`) now spreads
    `editing.entry` (the original row) first, so an edit-save preserves `source_*` and the
    milestone-5 `save_strategy`/`ignore_files`/`ignore_extensions`/`save_paths`/`state_paths`
    fields. Fixed in `0a74022` ("rewrite: entry autoconfig, platform and core defaults,
    defaults backfill").
19. **`Config.retroachievements_username` always serializes**, with no
    `skip_serializing_if` (`crates/grid-core/src/config.rs:80-84`), unlike the `source_*` and
    autoprofile fields on `EmulatorEntry`, which all use
    `skip_serializing_if = "String::is_empty"` so a config written before they existed
    round-trips byte-identically. The byte-identical round-trip claims in this doc and its
    tests are scoped to those ENTRY fields, not to the whole config file: a config with no
    `retroachievements_username` key gains one (`retroachievements_username = ""`) the first
    time grid-core saves it.
20. **The RetroAchievements token is never written to grid config.** The Python reference
    keeps a live in-memory copy on `self.config["retroachievements_token"]`
    (`grid-launcher.py:2743`), scrubbed to `""` only at serialization time
    (`grid_launcher/core/config.py:256`, `serialized_config`) — so the on-disk file never
    holds it, but the live config dict does. The rewrite's `Config` struct
    (`crates/grid-core/src/config.rs`) has no such field at all: the token lives only in the
    OS keyring, behind the `RaTokenStore` trait's own account
    (`crates/grid-core/src/secrets.rs:35-38`), and reaches the writers as
    `RaCredentials::token()` (`crates/grid-core/src/autoconfig/mod.rs:67`) — never through the
    config struct or file, even transiently. Deliberate security improvement.
21. **`yaml_add_only_section` trims its captured key; `toml_add_only_section` does not.**
    `yaml_add_only_section` records a matched key `.trim()`med
    (`crates/grid-core/src/autoconfig/writers.rs:328`, `:376-377`), matching
    `rpcs3.py:154`'s `group(1).strip()` — ruled in Task 1. `toml_add_only_section` records
    every matched key UNTRIMMED (`crates/grid-core/src/autoconfig/writers.rs:335-337`),
    matching `xemu.py:225`, which has no `.strip()`. The asymmetry between the two add-only
    writers is deliberate parity with two different reference bugs, not a port inconsistency.

