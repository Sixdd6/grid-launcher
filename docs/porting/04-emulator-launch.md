# 04 — Emulator selection, command construction, and process launch

## Purpose

This document describes what happens between "the user presses Play on an installed
game" and "a child process is running". It covers:

- picking an emulator for a game's platform,
- matching an emulator executable to an autoprofile,
- turning a profile argument template into an argument vector,
- the Windows-native (non-emulated) launch path, including Wine/Proton wrapping on Linux,
- environment cleaning before `spawn`,
- how emulators and compatibility tools are downloaded and installed,
- how running sessions are tracked and how exit is detected.

Out of scope (cross-references):

- Archive extraction internals used by the installer — see doc 03.
- Per-emulator configuration file writing (`ensure_*_settings`, `*_directory_settings`,
  `*_save_path_overrides`) — see doc 05. The launch path calls
  `_ensure_emulator_sync_settings` once per emulator+path pair immediately before
  spawning (grid_launcher/ui/mixins/details_view_mixin.py:1453), but the contents of
  those writers belong to doc 05.

## External surfaces

### Processes spawned

| Situation | Argv shape | Working directory | Anchor |
|---|---|---|---|
| Emulated game | `[<emulator_path>, *resolved_args]` | `emulator_path.parent` | grid_launcher/emulator/launch.py:317 |
| Native game, no compat tool | `[<game_exe>, *custom_args]` | `game_exe.parent` | grid_launcher/emulator/launch.py:242 |
| Native game, `native_compat_tool == "wine"` | `[<wine>, <game_exe>, *custom_args]` | `game_exe.parent` | grid_launcher/emulator/launch.py:252 |
| Native game, any other non-empty compat tool | `[<umu-run>, <game_exe>, *custom_args]` | `game_exe.parent` | grid_launcher/emulator/launch.py:257 |
| RPCS3 firmware install (side action, not a game launch) | see `trigger_rpcs3_firmware_install` | — | grid_launcher/ui/mixins/emulator_ui_mixin.py:799 |

Spawn call sites:

- Desktop UI: `subprocess.Popen(command, cwd=working_directory, env=..., creationflags=...)`
  for native (grid_launcher/ui/mixins/details_view_mixin.py:1435) and for emulated
  (grid_launcher/ui/mixins/details_view_mixin.py:1465).
- TV/QML backend: `_subprocess_popen(command, cwd=cwd, close_fds=True, env=..., creationflags=...)`
  (grid_launcher/tv/bridge/game_backend.py:399). `_subprocess_popen` is a module-level
  alias of `subprocess.Popen` kept for test patching (grid_launcher/tv/bridge/game_backend.py:66).

`creationflags` is `subprocess.CREATE_NEW_PROCESS_GROUP` on `win32` and `0` elsewhere
(grid_launcher/ui/mixins/details_view_mixin.py:1439, grid_launcher/tv/bridge/game_backend.py:398).

### Executables looked up on PATH

- `wine` — `shutil.which("wine")`, falling back to the literal string `"wine"` when not
  found (grid_launcher/emulator/launch.py:253). Also probed to decide whether to offer
  "Wine (system)" in the compat-tool list (grid_launcher/ui/mixins/emulator_ui_mixin.py:233,
  grid_launcher/ui/mixins/emulator_ui_mixin.py:295).
- `umu-run` — `detect_umu_run()` is `shutil.which("umu-run")`
  (grid_launcher/emulator/launch.py:19).

### Environment variables

| Variable | Direction | Rule | Anchor |
|---|---|---|---|
| `LD_LIBRARY_PATH_ORIG` | read | If present, its value is copied into `LD_LIBRARY_PATH` for the child | grid_launcher/core/process.py:20 |
| `LD_LIBRARY_PATH` | write / delete | Overwritten from `..._ORIG`; else removed entirely when the app is a frozen build (`sys.frozen`) | grid_launcher/core/process.py:21, grid_launcher/core/process.py:23 |
| `WINEPREFIX` | write | Set to `game["native_wineprefix"]` when non-empty, for both the wine and the umu/Proton branch | grid_launcher/emulator/launch.py:256, grid_launcher/emulator/launch.py:267 |
| `PROTONPATH` | write | Set to the selected compat tool path in the umu/Proton branch | grid_launcher/emulator/launch.py:264 |
| `XDG_DATA_HOME` | read | Base for the managed compat-tool install directory | grid_launcher/core/path.py:40, grid_launcher/core/path.py:60 |
| `XDG_CONFIG_HOME` | read | Used by config-path resolution | grid_launcher/core/path.py:33 |
| `GRID_LAUNCHER_SHARE_DIR` | read | Overrides the directory holding `emulator-autoprofiles.json` and `retroarch-core-list.json` (Flatpak) | grid_launcher/core/path.py:54 |

The desktop native path builds the child environment as
`clean_subprocess_env({**os.environ, **compat_env})` when there are compat overrides, and
`clean_subprocess_env(None)` otherwise (grid_launcher/ui/mixins/details_view_mixin.py:1438).
The emulated path always uses `clean_subprocess_env()` with no overrides
(grid_launcher/ui/mixins/details_view_mixin.py:1468).

### Files and directories probed

- Emulator executable existence and file-ness (grid_launcher/emulator/launch.py:298).
- ROM candidate existence and file-ness (grid_launcher/emulator/launch.py:306).
- RetroArch core file relative to the emulator directory
  (grid_launcher/emulator/launch.py:217).
- Steam compatibility-tool roots, each scanned for immediate subdirectories that contain
  a `proton` file (grid_launcher/ui/mixins/emulator_ui_mixin.py:201):
  - `~/.steam/steam/compatibilitytools.d`
  - `~/.local/share/Steam/compatibilitytools.d`
  - `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d`
- Managed compat-tool install root: `<XDG_DATA_HOME>/grid-launcher/compat-tools`
  (grid_launcher/core/path.py:61).
- Emulator install root: `<library_path>/Emulators/<sanitized name>`
  (grid_launcher/emulator/autoconfig.py:13).
- `emulator-autoprofiles.json` next to the repository root, or under
  `GRID_LAUNCHER_SHARE_DIR` (grid_launcher/emulator/profiles.py:409,
  grid_launcher/ui/mixins/emulator_ui_mixin.py:446).
- Wine prefix directory is created with `os.makedirs(..., exist_ok=True)` at launch time
  when configured (grid_launcher/emulator/launch.py:255, grid_launcher/emulator/launch.py:266).

### Network

Only the acquisition path talks to the network:

- GitHub: `https://api.github.com/repos/{owner}/{repo}/releases[/latest|/tags/{tag}]`
  (grid_launcher/background/workers.py:192, grid_launcher/background/workers.py:209).
- Gitea: `{base_url}/api/v1/repos/{owner}/{repo}/...` (grid_launcher/background/workers.py:197).
- Direct: an arbitrary `download_url`, or an HTML `page_url` scraped for a matching link
  (grid_launcher/background/workers.py:250).

GitHub requests default `Accept: application/vnd.github+json`,
`X-GitHub-Api-Version: 2022-11-28`, `User-Agent: grid-launcher`
(grid_launcher/background/workers.py:295). Gitea requests send no extra headers
(grid_launcher/background/workers.py:198).

## Data model

### Emulator entry (persisted in config under `emulators`)

Normalized by `normalize_emulators` (grid_launcher/core/config.py:8).

| Field | Type | Default | Notes | Anchor |
|---|---|---|---|---|
| `name` | string | — | Entry is dropped if missing/blank; matched case-insensitively | grid_launcher/core/config.py:32, grid_launcher/emulator/selection.py:235 |
| `path` | string | `""` | Executable path; `~` is expanded at use time | grid_launcher/core/config.py:60, grid_launcher/emulator/launch.py:297 |
| `args` | string | `"%rom%"` | Argument template; blank collapses to `"%rom%"` | grid_launcher/core/config.py:61 |
| `save_strategy` | string | `"auto"` | Normalized to `auto` / `single_file` / `folder` | grid_launcher/emulator/profiles.py:141 |
| `ignore_files` | string | `""` | `;`/newline-separated list | grid_launcher/emulator/profiles.py:133 |
| `ignore_extensions` | string | `""` | `;`/newline-separated list | grid_launcher/core/config.py:64 |
| `save_paths` | string | `""` | Cloud-save concern (doc 05/06) | grid_launcher/core/config.py:65 |
| `state_paths` | string | `""` | Cloud-save concern | grid_launcher/core/config.py:66 |
| `source_id`, `source_provider`, `source_owner`, `source_repo`, `source_release_tag` | string | omitted when blank | Written only for source-installed emulators | grid_launcher/core/config.py:68 |

### Autoprofile (`emulator-autoprofiles.json`)

Normalized by `normalize_emulator_autoprofiles` (grid_launcher/emulator/profiles.py:427).
Entries with neither `match_tokens` nor `is_compat_tool == true` are dropped
(grid_launcher/emulator/profiles.py:449); entries without a non-blank `name` are dropped
(grid_launcher/emulator/profiles.py:452).

| Field | Type | Default | Meaning | Anchor |
|---|---|---|---|---|
| `match_tokens` | string[] | `[]` | Executable basenames, casefolded; may contain `*`/`?` globs | grid_launcher/emulator/profiles.py:443, grid_launcher/emulator/profiles.py:188 |
| `name` | string | — | Profile / default emulator display name | grid_launcher/emulator/profiles.py:452 |
| `args` | string | `"%rom%"` | Argument template | grid_launcher/emulator/profiles.py:456 |
| `all_platforms` | bool | `false` | If true, the emulator is considered to support every platform | grid_launcher/emulator/profiles.py:460, grid-launcher.py:3569 |
| `platform_keywords` | string[] | `[]` | Token sets used to derive supported server platforms | grid_launcher/emulator/profiles.py:462 |
| `use_game_title_as_name` | bool | `false` | Names the created entry after the downloaded title | grid_launcher/emulator/profiles.py:471, grid_launcher/emulator/profiles.py:312 |
| `save_strategy` | string | `"auto"` | Doc 05/06 | grid_launcher/emulator/profiles.py:473 |
| `ignore_files`, `ignore_extensions` | string[] | `[]` | Doc 06 | grid_launcher/emulator/profiles.py:478, grid_launcher/emulator/profiles.py:487 |
| `save_directories`, `state_directories`, `screenshot_directories` | string[] | `[]` | Doc 05/06 | grid_launcher/emulator/profiles.py:498, grid_launcher/emulator/profiles.py:507, grid_launcher/emulator/profiles.py:516 |
| `firmware_directories` | (string \| object)[] | `[]` | Strings kept as-is; dicts shallow-copied | grid_launcher/emulator/profiles.py:525 |
| `is_compat_tool` | bool | `false` | Marks the profile as a Wine/Proton build rather than an emulator | grid_launcher/emulator/profiles.py:448 |
| `compat_tool_type` | string | `""` | Free-form type tag, e.g. `proton` | grid_launcher/emulator/profiles.py:537 |
| `source` | object \| absent | absent | Download metadata; shallow-copied verbatim | grid_launcher/emulator/profiles.py:534 |

Profiles shipped in the repo use exactly these `args` templates
(`emulator-autoprofiles.json`):

`-L "%core%" "%rom%"`, `-fullscreen -batch "%rom%"`, `-portable -fullscreen -batch "%rom%"`,
`--fullscreen --pause-menu-exit "%rom%"`, `--no-gui "%ps3_launch_target%"`, `"%rom%"`,
`-g "%rom%"`, `-f "%rom%"`, `-run "%rom%"`, `-full-screen -dvd_path "%rom%"`,
`-f -g "%rom%"`, and `""` (the two compat-tool profiles GE-Proton and Proton-CachyOS, plus
the ShadPS4 Qt launcher).

### Source metadata (`profile.source`)

Normalized by `normalize_emulator_source_metadata` (grid_launcher/emulator/source.py:59).

| Field | Type | Notes | Anchor |
|---|---|---|---|
| `provider` / `type` | string | Aliased to `github`, `gitea`, or `direct`; missing → error | grid_launcher/emulator/source.py:63, grid_launcher/emulator/source.py:81 |
| `owner` | string | Required for all providers | grid_launcher/emulator/source.py:83 |
| `repo` / `repository` | string | Required for all providers | grid_launcher/emulator/source.py:84 |
| `asset_patterns` / `asset_globs` | string[] | Include globs, default `["*"]` | grid_launcher/emulator/source.py:86 |
| `asset_exclude_patterns` / `exclude_asset_patterns` | string[] | Exclude globs, default `[]` | grid_launcher/emulator/source.py:90 |
| `asset_preferred_patterns` / `preferred_asset_patterns` | string[] | Ranking globs, default `[]` | grid_launcher/emulator/source.py:94 |
| `tag` / `release_tag` / `version` | string | First non-blank wins | grid_launcher/emulator/source.py:100 |
| `allow_prerelease` | bool | Default false | grid_launcher/emulator/source.py:106 |
| `base_url` | string | Required for `gitea`; trailing `/` stripped | grid_launcher/emulator/source.py:124 |
| `download_url` / `url` / `browser_download_url` | string | `direct` only | grid_launcher/emulator/source.py:128 |
| `page_url` / `index_url` / `listing_url` | string | `direct` only; scraped when `download_url` absent | grid_launcher/emulator/source.py:133 |
| `download_url_regex` / `url_regex` / `asset_url_regex` | string | `direct` only | grid_launcher/emulator/source.py:138 |
| `asset_name` | string | `direct` only; defaults to the URL basename | grid_launcher/emulator/source.py:143, grid_launcher/background/workers.py:284 |
| `supplemental_downloads` | object[] | `direct` only; each resolved and downloaded next to the primary archive | grid_launcher/emulator/source.py:156, grid_launcher/background/workers.py:127 |
| `platform_overrides` | object | Keyed by a `sys.platform` prefix; merged over the source at resolve time | grid_launcher/emulator/source.py:119, grid_launcher/background/workers.py:167 |
| `platforms` | string[] | Allowlist of `sys.platform` prefixes; gates availability and install | grid_launcher/emulator/profiles.py:46, grid_launcher/background/workers.py:178 |
| `windows_assets`, `windows_arch` | list / string | Windows-only asset override, consulted before generic asset selection | grid_launcher/background/workers.py:322, grid_launcher/background/workers.py:393 |
| `manual_install_hint` | string | Appended to the "not available on this platform" message | grid_launcher/background/workers.py:183 |

### Compat tool (runtime list entry)

Produced by `_available_compat_tools_for_dialog` (grid_launcher/ui/mixins/emulator_ui_mixin.py:231).

| Field | Meaning | Anchor |
|---|---|---|
| `name` | Label shown to the user | grid_launcher/ui/mixins/emulator_ui_mixin.py:232 |
| `type` | `""` (None), `"wine"`, or `"proton"` | grid_launcher/ui/mixins/emulator_ui_mixin.py:234, grid_launcher/ui/mixins/emulator_ui_mixin.py:228 |
| `path` | `""` (None), the literal `"wine"`, or an absolute Proton directory | grid_launcher/ui/mixins/emulator_ui_mixin.py:234, grid_launcher/ui/mixins/emulator_ui_mixin.py:250 |

The list is always seeded with a `None` entry (`{"name": "None", "type": "", "path": ""}`)
(grid_launcher/ui/mixins/emulator_ui_mixin.py:232), then "Wine (system)" if `wine` is on
PATH, then scanned system Proton installs, then managed installs. Managed install paths
suppress duplicate system entries (grid_launcher/ui/mixins/emulator_ui_mixin.py:235).

### Compat tool (persisted, config `compat_tool_installs`)

`dict[key -> {name, compat_tool_type, install_path}]`; entries without a non-blank `name`
are dropped (grid_launcher/core/config.py:193). The selected default lives in the scalar
config key `default_compat_tool` (grid-launcher.py:2347), default `""`.

### Installed-game fields relevant to launch

Normalized by `normalize_installed_games` (grid_launcher/core/config.py:152).

| Field | Used for | Anchor |
|---|---|---|
| `platform` | Emulator selection; `windows*` prefix routes to the native path | grid_launcher/emulator/selection.py:150 |
| `archive_path`, `extracted_path`, `extracted_dir`, `local_path`, `native_game_dir` | ROM / install-dir resolution | grid_launcher/core/config.py:167, grid_launcher/library/install_paths.py:92 |
| `native_executable_path` | User-pinned native executable | grid_launcher/library/install_paths.py:135 |
| `native_launch_parameters` | Extra argv appended after the executable | grid_launcher/emulator/launch.py:235 |
| `native_compat_tool` | Per-game compat-tool override | grid_launcher/emulator/launch.py:245 |
| `native_wineprefix` | Per-game `WINEPREFIX` | grid_launcher/emulator/launch.py:248 |
| `ps3_game_id`, `ps3_iso_path` | RPCS3 launch target | grid_launcher/emulator/launch.py:88 |

## Behavior

### 1. Play button dispatch

`_perform_game_action` (grid_launcher/ui/mixins/details_view_mixin.py:1491):

1. Return if there is no current details game.
2. If the game is **not** installed → `_start_async_install(game)` and stop
   (grid_launcher/ui/mixins/details_view_mixin.py:1505).
3. Otherwise resolve the installed record; fall back to the details game when there is no
   record (grid_launcher/ui/mixins/details_view_mixin.py:1495).
4. `_auto_sync_before_launch(launch_game)` — pre-launch cloud restore
   (grid_launcher/ui/mixins/details_view_mixin.py:1497).
5. `_install_firmware_for_game_without_ui(launch_game, {})` inside a bare `try/except`
   that swallows every exception (grid_launcher/ui/mixins/details_view_mixin.py:1499).
6. `_launch_installed_game(launch_game)` (grid_launcher/ui/mixins/details_view_mixin.py:1503).

The TV backend has a parallel dispatcher, `launchGame`
(grid_launcher/tv/bridge/game_backend.py:206): it rejects a second launch while a session
is active (grid_launcher/tv/bridge/game_backend.py:212) and routes native platforms to
`_handle_native_launch` (grid_launcher/tv/bridge/game_backend.py:216). It does **not**
perform a firmware step (grid_launcher/tv/bridge/game_backend.py:647).

### 2. Emulator selection algorithm

Resolution order for a platform string (`default_emulator_name_for_platform`,
grid_launcher/emulator/selection.py:270):

1. Look up the platform in the `default_emulators` mapping via
   `mapping_value_for_platform`: exact key first, then a case-insensitive key scan; blank
   values are ignored (grid_launcher/emulator/selection.py:214).
2. If a name is configured, find its entry by case-insensitive name match
   (grid_launcher/emulator/selection.py:234) and keep it **only if**
   `emulator_supports_platform(entry, platform)` (grid_launcher/emulator/selection.py:280).
3. Otherwise fall back to the first entry in `compatible_emulator_names_for_platform`,
   which preserves the order of the `emulators` list and skips entries with a blank name
   (grid_launcher/emulator/selection.py:254).
4. If nothing matches, return `""` (grid_launcher/emulator/selection.py:290).

There is no separate "per-game override" key for emulator choice: the override surface is
the platform→emulator mapping. Per-game overrides exist only for the native path
(`native_executable_path`, `native_launch_parameters`, `native_compat_tool`,
`native_wineprefix`).

`available_emulator_name_for_platform` (grid_launcher/emulator/selection.py:293) is a
stricter variant used by install gating: it walks `[default, *compatible]` (de-duplicated,
grid_launcher/emulator/selection.py:306) and returns the first whose `path` exists and is a
file (`emulator_entry_has_usable_path`, grid_launcher/emulator/selection.py:245). A blank
platform returns `""` (grid_launcher/emulator/selection.py:301).

`emulator_supports_platform` is supplied by the window
(grid-launcher.py:3556):

1. Blank platform → `True` (grid-launcher.py:3558).
2. Resolve the profile for the entry; if the profile has `all_platforms` → `True`
   (grid-launcher.py:3569).
3. If the entry name or the profile name matches "retroarch", support is decided purely by
   whether any installed RetroArch core is mapped to the platform
   (grid-launcher.py:3572).
4. If no profile matched at all → `True` (grid-launcher.py:3579).
5. Otherwise the profile's `platform_keywords` are expanded into concrete server platform
   names and compared case-insensitively (grid-launcher.py:3586).

### 3. Profile matching

`emulator_profile_for_entry(emulator, profiles)` (grid_launcher/emulator/profiles.py:192)
iterates profiles in file order and returns the first match by:

1. Entry `name` equal (casefolded) to profile `name` (grid_launcher/emulator/profiles.py:205).
2. Executable **basename** matching any `match_tokens` entry; tokens containing `*` or `?`
   use glob matching (`fnmatch.fnmatchcase`), otherwise exact equality
   (grid_launcher/emulator/profiles.py:219, grid_launcher/emulator/profiles.py:188).
3. Executable **stem** equal to the stem of any token
   (grid_launcher/emulator/profiles.py:221).

`emulator_profile_for_game(game, executable_path, profiles)`
(grid_launcher/emulator/profiles.py:279) is the install-time variant. It parses the
executable with `PureWindowsPath` so Windows-style paths resolve on any host
(grid_launcher/emulator/profiles.py:286), collects **all** token matches, and:

- one match → use it (grid_launcher/emulator/profiles.py:299);
- several matches → prefer the one whose `name` equals the downloaded title exactly,
  otherwise the first (grid_launcher/emulator/profiles.py:302);
- no match → synthesize `{name: title or "Emulator", args: "%rom%", ...}`
  (grid_launcher/emulator/profiles.py:326).

When the chosen profile sets `use_game_title_as_name`, the resulting entry name becomes the
game title (grid_launcher/emulator/profiles.py:312).

### 4. Platform gating

`is_available_on_current_platform(profile, platform=None)`
(grid_launcher/emulator/profiles.py:22):

1. `platform` defaults to `sys.platform` when not given or blank
   (grid_launcher/emulator/profiles.py:33).
2. Any platform starting with `win` (casefolded) → `True` unconditionally
   (grid_launcher/emulator/profiles.py:34).
3. Non-dict profile → `True` (grid_launcher/emulator/profiles.py:37).
4. Profile `name`, casefolded and stripped, present in `_WINDOWS_ONLY_EMULATOR_SLUGS`
   → `False`. The set is exactly `{"xenia canary (xbox 360)", "xenia (xbox 360)",
   "shadps4 qt launcher"}` (grid_launcher/emulator/profiles.py:13).
5. `profile["source"]["platforms"]`, when a non-empty list, acts as an allowlist compared
   against the casefolded platform string with **exact** equality (not prefix matching)
   → `False` on no match (grid_launcher/emulator/profiles.py:44).
6. Else `True`.

Applied at three points:

- Autoprofile lookup for the running app filters the returned list, but keeps the unfiltered
  list in the cache (grid_launcher/ui/mixins/emulator_ui_mixin.py:466).
- The emulator config dialog's supported-profile list filters on load
  (grid_launcher/ui/dialogs.py:53).
- Xbox 360 content application refuses a Windows-only emulator on other platforms
  (grid_launcher/ui/mixins/install_mixin.py:370).

Note that the download listing uses a *different*, prefix-based rule:
`source_download_emulator_entries` compares `current_platform.startswith(p)` for each entry
in `source["platforms"]` (grid_launcher/ui/emulators.py:191), as does the install-time check
(grid_launcher/ui/mixins/install_mixin.py:1422) and the download worker
(grid_launcher/background/workers.py:180).

### 5. Placeholder table

Placeholders are produced by `launch_placeholders_for_game`
(grid_launcher/emulator/launch.py:79) and applied by
`apply_launch_placeholders_to_args` (grid_launcher/emulator/launch.py:111).

| Token | Expansion | Anchor |
|---|---|---|
| `%rom%` | Resolved ROM path (see §6) | grid_launcher/emulator/launch.py:98 |
| `%core%` | RetroArch core argument path, or `""` for non-RetroArch emulators | grid_launcher/emulator/launch.py:99, grid_launcher/emulator/launch.py:60 |
| `%ps3_launch_target%` | For RPCS3 only: the PS3 ISO path if set, else the literal string `%RPCS3_GAMEID%:<GAMEID>`, else `""` | grid_launcher/emulator/launch.py:87 |
| `%RPCS3_GAMEID%` | **Not expanded by the launcher.** It is emitted verbatim as a prefix inside `%ps3_launch_target%` and consumed by RPCS3 itself | grid_launcher/emulator/launch.py:95 |
| `%ps3_gameid%` | **Not in the placeholder map.** Appears only in legacy argument templates used by config parsers and tests; it passes through unchanged | tests/test_emulator_profiles.py:267 |

Substitution is a plain, unanchored `str.replace` for every token over every argv element,
applied in dictionary order (grid_launcher/emulator/launch.py:118). There is no escaping.

`%core%` derivation (`retroarch_core_value`, grid_launcher/emulator/launch.py:60):

1. Non-RetroArch emulator name → `""` (grid_launcher/emulator/launch.py:68).
2. Blank platform → `""` (grid_launcher/emulator/launch.py:70).
3. Look up the platform in `default_retroarch_cores`; blank → `""`
   (grid_launcher/emulator/launch.py:73).
4. Convert with `retroarch_core_argument_path` (grid_launcher/emulator/launch.py:31):
   - blank → `""`;
   - backslashes are converted to forward slashes; if the value still contains `/`, it is
     returned as-is (already a path) (grid_launcher/emulator/launch.py:36);
   - otherwise pick the platform extension: `.dll` on `win32`, `.dylib` on `darwin`,
     `.so` elsewhere (grid_launcher/emulator/launch.py:40);
   - strip a trailing `.dll`/`.dylib`/`.so` (case-insensitive, first match wins)
     (grid_launcher/emulator/launch.py:48);
   - append `_libretro` unless the base already ends with it
     (grid_launcher/emulator/launch.py:53);
   - return `cores/<base>_libretro<ext>` (grid_launcher/emulator/launch.py:57).

### 6. ROM path resolution

`resolve_rom_path_for_game` (grid_launcher/emulator/launch.py:181):

1. Unless the platform is arcade, return the first *extracted* candidate that exists and is
   a file (grid_launcher/emulator/launch.py:188).
2. Otherwise (or if none matched), return the first *archive* candidate that exists and is a
   file (grid_launcher/emulator/launch.py:192).
3. Otherwise return the raw `archive_path` string, stripped
   (grid_launcher/emulator/launch.py:196).

Arcade detection is a substring test on the lowercased platform against
`("arcade", "mame", "fbneo", "final burn")` (grid_launcher/emulator/selection.py:14). The
effect is that arcade titles are handed the archive (zipped ROM set), not the extracted
directory contents.

### 7. Argument template construction

`resolve_launch_arguments_for_game` (grid_launcher/emulator/launch.py:150):

1. Read `game["platform"]` and resolve the emulator name and entry
   (grid_launcher/emulator/launch.py:162).
2. Emulator args default to `"%rom%"`; the entry's `args` is used when it is a non-blank
   string, stripped (grid_launcher/emulator/launch.py:165).
3. Append the global config `launch_args`, stripped, joined by a single space; blank parts
   are dropped (grid_launcher/emulator/launch.py:172).
4. Tokenize with `split_launch_template_args` (grid_launcher/emulator/launch.py:130):
   a blank template gives `[]`; otherwise POSIX-mode shell splitting, retrying in non-POSIX
   mode when POSIX splitting raises (unbalanced quotes)
   (grid_launcher/emulator/launch.py:134).
5. Build placeholders, then `validate_launch_placeholders`
   (grid_launcher/emulator/launch.py:140):
   - template mentions `%core%` but the resolved core is blank → raise
     `ValueError("No RetroArch core is configured for this platform. Set one in
     Emulators > Defaults.")`;
   - template mentions `%ps3_launch_target%` but it is blank → raise a "No PS3 ISO or game
     ID was found…" `ValueError` (grid_launcher/emulator/launch.py:143).
6. Substitute and normalize each token (`apply_launch_placeholders_to_args`,
   grid_launcher/emulator/launch.py:111):
   - remember whether the raw token contained `%core%`;
   - replace all tokens;
   - `strip_wrapping_quotes`: trim whitespace, then drop one matching leading/trailing pair
     of `"` or `'` when the string is at least 2 characters
     (grid_launcher/emulator/launch.py:104);
   - if the token carried `%core%` and the core is blank, also **pop the preceding argv
     element** when it is one of `-L`, `--libretro`, `--core`, then skip the token
     (grid_launcher/emulator/launch.py:121);
   - drop any token that became empty (grid_launcher/emulator/launch.py:125).

Note the asymmetry: step 5 already raises when the *combined template* mentions `%core%`
with no core configured, so the orphan-flag cleanup in step 6 is only reachable via callers
that skip validation.

RetroArch-only post-pass, `normalized_retroarch_core_args(emulator_dir, args)`
(grid_launcher/emulator/launch.py:202): for every element (except the last) equal to `-L`,
`--libretro`, or `--core`, if the following token is a non-blank **relative** path and
`emulator_dir / token` exists as a file, rewrite it to the resolved absolute path
(grid_launcher/emulator/launch.py:217). Absolute paths and non-existent candidates are left
untouched.

### 8. Launch decision tree

```
_launch_installed_game(game)                       details_view_mixin.py:1426
├── is_native_executable_platform(game)?           selection.py:145  (platform starts with "windows")
│   ├── yes → prepare_native_launch_command(...)   launch.py:223
│   │         default_compat_tool = _default_compat_tool() on non-win32, "" on win32
│   │                                              details_view_mixin.py:1433
│   │         ├── resolve executable → None ⇒ ValueError
│   │         ├── split game["native_launch_parameters"]; ValueError ⇒
│   │         │   ValueError("Invalid custom launch parameters: …")   launch.py:240
│   │         ├── command = [exe, *args]; cwd = exe.parent            launch.py:242
│   │         ├── tool = game["native_compat_tool"] or default_compat_tool  launch.py:245
│   │         ├── tool == "wine"  → prepend which("wine") or "wine"   launch.py:252
│   │         │                     WINEPREFIX set + mkdir if configured
│   │         ├── tool non-empty  → umu = which("umu-run")
│   │         │                     umu missing ⇒ ValueError("umu-run is not installed…")
│   │         │                     prepend umu; PROTONPATH = tool
│   │         │                     WINEPREFIX set + mkdir if configured  launch.py:257
│   │         └── tool empty      → run the .exe directly, no env overrides
│   │       Popen(env=clean_subprocess_env({**os.environ, **overrides}))  details_view_mixin.py:1438
│   │       QTimer.singleShot(500, warn-if-exited)                    details_view_mixin.py:1441
│   │       return True   (no cloud session is registered for native games)
│   └── no  → prepare_emulator_launch_command(...)                    launch.py:273
│             ├── emulator name blank ⇒ "No emulator is configured. Add one in Emulators settings."
│             ├── entry missing       ⇒ "Default emulator '<name>' was not found."
│             ├── entry path blank    ⇒ "Emulator '<name>' has no executable path configured."
│             ├── path missing/not a file ⇒ "Emulator executable not found:\n<path>"
│             ├── rom path blank      ⇒ "No ROM file is available for this game."
│             ├── rom missing/not a file ⇒ "ROM file not found:\n<path>"
│             ├── argument resolution ValueError ⇒ "Invalid launch arguments: <e>"
│             └── RetroArch ⇒ normalized_retroarch_core_args(exe.parent, args)
│           _ensure_emulator_sync_settings(name, path)   (doc 05)     details_view_mixin.py:1457
│           RPCS3 + non-blank ps3_game_id + both dirs resolvable ⇒
│               copy_ps3_custom_config_to_emulator(dev_hdd0.parent/"config", rpcs3_root)
│                                                                     details_view_mixin.py:1464
│           Popen(env=clean_subprocess_env())                         details_view_mixin.py:1465
│           QTimer.singleShot(500, warn-if-exited)                    details_view_mixin.py:1471
│           _register_game_session_for_auto_upload(game, process, name)  details_view_mixin.py:1472
└── ValueError ⇒ "Launch Error" dialog, return False                  details_view_mixin.py:1474
    OSError    ⇒ "Failed to launch game:\n<e>", return False          details_view_mixin.py:1477
```

The 500 ms post-launch check calls `process.poll()`; a non-`None` exit code produces a
warning containing the code and the space-joined command line
(`process_exited_early_message`, grid_launcher/emulator/launch.py:321,
grid_launcher/ui/mixins/details_view_mixin.py:1481).

### 9. Native (non-emulated) launch details

A game is "native" when its platform, casefolded and stripped, starts with `windows`
(grid_launcher/emulator/selection.py:150). Note this is the *server platform* name, not the
host OS — a Windows-platform title on Linux still takes this branch, which is why the compat
tool exists.

Executable resolution, `resolved_native_executable_path_for_game`
(grid_launcher/library/install_paths.py:130):

1. If `game["native_executable_path"]` is set, exists, is a file, and has a launchable
   suffix, use it (grid_launcher/library/install_paths.py:139).
2. Otherwise use the first entry of the candidate list
   (grid_launcher/library/install_paths.py:142).
3. Otherwise `None` → `prepare_native_launch_command` raises "No launchable native
   executable is configured for this game. Use Game Settings to select one."
   (grid_launcher/emulator/launch.py:231).

Candidates come from `native_executable_candidates_for_game`
(grid_launcher/library/install_paths.py:114): a recursive `rglob("*")` over the install
directory keeping files whose suffix (casefolded) is in
`{.exe, .bat, .cmd, .ps1, .sh}` (grid_launcher/emulator/launch.py:11), sorted by
`(number of path parts, casefolded full path)` — i.e. shallowest first, then
alphabetically (grid_launcher/library/install_paths.py:126).

Install directory, `native_install_dir_for_game`
(grid_launcher/library/install_paths.py:92): `extracted_dir` if it is an existing directory;
else the parent of an existing `extracted_path` file; else the parent of the first existing
archive candidate; else `None`.

The emulator variant of the suffix set additionally allows `.appimage`
(`_EMULATOR_SUFFIXES`, grid_launcher/emulator/launch.py:12); it is used when auto-detecting
a downloaded emulator's executable, not when picking a game executable.

Compat-tool resolution order at launch (grid_launcher/emulator/launch.py:245):

1. `game["native_compat_tool"]`, stripped.
2. If blank, the `default_compat_tool` argument. Desktop passes
   `self._default_compat_tool()` on non-`win32` and `""` on `win32`
   (grid_launcher/ui/mixins/details_view_mixin.py:1433); the TV backend passes
   `config["default_compat_tool"]` under the same condition
   (grid_launcher/tv/bridge/game_backend.py:376). This auto-selection was added in commit
   0e3dc3e; before it, a Windows `.exe` on Linux with no per-game tool was executed directly.
3. If still blank, no wrapper is used.

The value `"wine"` is special-cased (grid_launcher/emulator/launch.py:252); **any other**
non-empty value is treated as a Proton path and requires `umu-run`
(grid_launcher/emulator/launch.py:257). `PROTONPATH` receives the value verbatim
(grid_launcher/emulator/launch.py:264). The wine branch never sets `PROTONPATH`.

`WINEPREFIX` is only set when `game["native_wineprefix"]` is non-blank; the directory is
created eagerly with `exist_ok=True` (grid_launcher/emulator/launch.py:255,
grid_launcher/emulator/launch.py:266). The prefix itself is created at *install* time on
Linux for native games: `<native_game_dir>/prefix`, or `<extracted_dir>/prefix` when
`native_game_dir` is blank (grid_launcher/background/workers.py:578).

The per-game compat tool and executable are edited through `NativeGameSettingsDialog`
(grid_launcher/ui/dialogs.py:187). The compat-tool combo and the read-only wine-prefix label
are only built on non-`win32` (grid_launcher/ui/dialogs.py:231); the combo preselects the
game's tool, falling back to the global default
(grid_launcher/ui/mixins/details_view_mixin.py:1742). On accept, the executable and
parameters are written unconditionally and the compat tool only on non-`win32`
(grid_launcher/ui/mixins/details_view_mixin.py:1771), then
`_persist_installed_games()` (grid_launcher/ui/mixins/details_view_mixin.py:1791).

### 10. Wine/Proton discovery and the compat-tool list

`grid_launcher/emulator/wine.py` contains **only** Windows→Wine-prefix *path* translation
(`translate_windows_path_to_wine_prefix`, grid_launcher/emulator/wine.py:7). It maps, in
order, `%USERPROFILE%\AppData\LocalLow`, `%USERPROFILE%\Documents`, `%APPDATA%`,
`%LOCALAPPDATA%`, `%USERPROFILE%`, `%PROGRAMDATA%`, `%PUBLIC%`, `%WINDIR%` onto
`<prefix>/drive_c/...` locations, using `getpass.getuser()` as the in-prefix username
(grid_launcher/emulator/wine.py:17). Matching is case-insensitive on the leading variable;
the remainder has `\` converted to `/` and leading slashes stripped
(grid_launcher/emulator/wine.py:37); unknown leading variables return `None`
(grid_launcher/emulator/wine.py:42). This is a save-path concern, not part of process spawn.

Discovery of compat tools lives in the emulator UI mixin:

- `_scan_system_proton_installs` (grid_launcher/ui/mixins/emulator_ui_mixin.py:200) walks
  the three Steam roots, skips non-directories, requires a `proton` **file** inside each
  candidate subdirectory, resolves symlinks, and de-duplicates by resolved path. Each hit is
  `{"name": <dirname>, "type": "proton", "path": <resolved>}`.
- `_available_compat_tools_for_dialog` (grid_launcher/ui/mixins/emulator_ui_mixin.py:231)
  composes None + system Wine + system Proton + managed installs, as described in the data
  model.
- `_refresh_compat_tool_list` (grid_launcher/ui/mixins/emulator_ui_mixin.py:255) renders the
  same three groups as exclusive radio buttons; selecting one writes
  `config["default_compat_tool"]` and saves immediately
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:182,
  grid_launcher/ui/mixins/emulator_ui_mixin.py:186). Note the "None" pseudo-entry is *not*
  rendered here — only the dialog list contains it. When nothing was added, a disabled
  "No compatibility tools installed" row is shown
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:318).
- `_open_compat_tool_download_dialog` (grid_launcher/ui/mixins/emulator_ui_mixin.py:323)
  returns immediately on `win32` (grid_launcher/ui/mixins/emulator_ui_mixin.py:324), filters
  autoprofiles to `is_compat_tool is True`
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:330), and starts an install with
  `_install_mode = "compat_tool"` plus `_compat_tool_install_dir` set to
  `<XDG_DATA_HOME>/grid-launcher/compat-tools`
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:361).

### 11. Firmware gating

Firmware never blocks a launch. `_perform_game_action` calls
`_install_firmware_for_game_without_ui` before launching and discards both its return value
and any exception it raises (grid_launcher/ui/mixins/details_view_mixin.py:1499); the launch
proceeds either way. The tests pin this behavior explicitly: firmware install runs before
launch (tests/test_firmware_launch.py:95), a non-empty warning string still launches
(tests/test_firmware_launch.py:109), an exception still launches
(tests/test_firmware_launch.py:136), and an uninstalled game skips firmware entirely and
starts an install instead (tests/test_firmware_launch.py:121).

`_install_firmware_for_game_without_ui` (grid_launcher/ui/mixins/install_mixin.py:528)
returns `""` early when: the platform is blank, the platform has no integer server id, no
default emulator resolves, or the emulator entry is missing
(grid_launcher/ui/mixins/install_mixin.py:532 … grid_launcher/ui/mixins/install_mixin.py:545).
For RetroArch, it additionally returns `""` when no core is configured for the platform
(grid_launcher/ui/mixins/install_mixin.py:557) and rewrites the firmware directories from the
core's metadata subdirectory and file list (grid_launcher/ui/mixins/install_mixin.py:564).

Missing-firmware conditions surface as advisory UI text, not gates: Eden shows notes when
`eden_keys_path` or `eden_has_firmware` is falsy
(grid_launcher/ui/mixins/emulator_ui_mixin.py:732,
grid_launcher/ui/mixins/emulator_ui_mixin.py:741), and RPCS3 offers an "Install PS3 Firmware"
button when a `.PUP` is present (grid_launcher/ui/mixins/emulator_ui_mixin.py:779).

Install-side gating is separate: `install_block_reason_for_game`
(grid_launcher/emulator/selection.py:370) returns `""` for native and "Emulators" platforms,
an error when the platform is blank, and an error naming the platform when no *available*
emulator (with an existing executable) is configured
(grid_launcher/emulator/selection.py:387).

### 12. Emulator / compat-tool acquisition

Listing → selection → download → extract → register.

**Listing.** `source_download_emulator_entries(autoprofiles, current_platform)`
(grid_launcher/ui/emulators.py:168) keeps profiles that have a dict `source`, pass the
`platforms` prefix test, have a recognized provider alias (only the GitHub aliases are
mapped here; anything else passes through unchanged,
grid_launcher/ui/emulators.py:159), and have both `owner` and `repo`
(grid_launcher/ui/emulators.py:202). Release tag falls back through
`release_tag` → `tag` → `version` → `"latest"` (grid_launcher/ui/emulators.py:205). Rows are
de-duplicated on `(name, provider, owner, repo)` casefolded
(grid_launcher/ui/emulators.py:214) and sorted by `(name, source_id)` casefolded
(grid_launcher/ui/emulators.py:231). `source_id` is `"{owner}/{repo}"`
(grid_launcher/ui/emulators.py:226). `filter_source_download_emulator_entries`
(grid_launcher/ui/emulators.py:235) then removes already-installed names and source ids and
applies an AND-of-tokens substring search.

**Release/asset resolution** happens twice, in two places with different code paths:

- Pure resolver `resolve_emulator_source_release_asset` (grid_launcher/emulator/source.py:11):
  for `direct`, it requires `download_url` and derives `asset_name` from the URL basename
  (grid_launcher/emulator/source.py:25); for `github`/`gitea`, it selects a release then an
  asset; any other provider raises (grid_launcher/emulator/source.py:38).
- `_select_github_release` (grid_launcher/emulator/source.py:243): a `release_tag` of
  `"latest"` is treated as "unset" (grid_launcher/emulator/source.py:254); drafts are always
  skipped; prereleases are skipped unless `allow_prerelease`; when a tag is set, the first
  release whose `tag_name` matches case-insensitively wins; otherwise the first surviving
  release in list order wins (grid_launcher/emulator/source.py:259). Failure messages list
  the available tags (grid_launcher/emulator/source.py:283).
- `_select_github_asset` (grid_launcher/emulator/source.py:303): each asset needs a `name`
  and a `browser_download_url`; it must match some include pattern and no exclude pattern
  (fnmatch, both sides casefolded, grid_launcher/emulator/source.py:295). Candidates sort by
  `(include_index, preferred_index, state_penalty, casefolded name)`, where
  `preferred_index` defaults to `len(preferred_patterns)` when unmatched and
  `state_penalty` is 0 for `state` in `{"", "uploaded"}` and 1 otherwise
  (grid_launcher/emulator/source.py:338). The lowest tuple wins
  (grid_launcher/emulator/source.py:364).

**Download worker** `_resolve_source_download` (grid_launcher/background/workers.py:165)
adds runtime behavior on top of the pure resolver:

1. Merge the first `platform_overrides` entry whose key is a prefix of `sys.platform`
   (grid_launcher/background/workers.py:172).
2. For `direct`, enforce the `platforms` allowlist (prefix match) with a message that
   appends `manual_install_hint` (grid_launcher/background/workers.py:180), then resolve
   the URL — scraping `page_url` for `href="…"` matches against `download_url_regex`,
   preferring an `href` hit, then a whole-page regex hit (first non-empty capture group,
   else the whole match), all joined with `urljoin`
   (grid_launcher/background/workers.py:250).
3. For `github`/`gitea`, pick the endpoint: `/releases/tags/{quoted tag}` for an explicit
   tag, `/releases/latest` for the literal `latest`, `/releases` when unset
   (grid_launcher/background/workers.py:206).
4. On `win32` only, `windows_assets` overrides generic selection: specs are filtered by the
   target architecture (`windows_arch`, else `platform.machine()` mapped to `arm64` or
   `x64`, grid_launcher/background/workers.py:393), then matched by exact `asset_name`
   first and `asset_name_regex` second (grid_launcher/background/workers.py:414). A present
   but unmatched `windows_assets` raises rather than falling through
   (grid_launcher/background/workers.py:380).

Downloads stream in 64 KiB chunks, emit a progress signal at most every 100 ms, and abort
with `OSError("Download cancelled by user")` when cancellation is requested
(grid_launcher/background/workers.py:110). `supplemental_downloads` are fetched afterwards
into sibling files named `<stem>-supplemental-<n><suffix>`, or
`<stem>-supplemental-<n>-<asset>` for AppImages
(grid_launcher/background/workers.py:147).

**Install directory.** `_start_async_source_emulator_install`
(grid_launcher/ui/mixins/install_mixin.py:1399) is entered for `_install_mode` in
`{source_emulator, source_emulator_update, compat_tool}`
(grid_launcher/ui/mixins/install_mixin.py:1165). It skips the install-block check for
updates (grid_launcher/ui/mixins/install_mixin.py:1404), requires dict source metadata
(grid_launcher/ui/mixins/install_mixin.py:1411), re-checks the `direct` platform allowlist
(grid_launcher/ui/mixins/install_mixin.py:1419), requires a library path
(grid_launcher/ui/mixins/install_mixin.py:1433), and computes
`install_path = <library>/Emulators/<sanitized stem of archive name>`
(grid_launcher/ui/mixins/install_mixin.py:1444, grid_launcher/emulator/autoconfig.py:13).
The archive is downloaded to `install_path / archive_name`
(grid_launcher/ui/mixins/install_mixin.py:1451). Archive name defaults to
`"<sanitized profile name>-<sanitized release tag>.zip"`
(grid_launcher/ui/mixins/emulator_ui_mixin.py:1187). Concurrent installs are queued by game
key (grid_launcher/ui/mixins/install_mixin.py:1462).

**Extraction and post-install.** Extraction itself is doc 03. Afterwards
`InstallFinalizeWorker` (grid_launcher/background/workers.py:546):

- creates the Wine prefix for native Linux installs (grid_launcher/background/workers.py:578);
- for `_install_mode == "compat_tool"`, sets `_compat_tool_install_path` to the extracted
  directory, or `<_compat_tool_install_dir>/<sanitized title>` as a fallback
  (grid_launcher/background/workers.py:589);
- cleans up archives, applies supplemental archives, then runs the firmware installer,
  folding any error into the warning text rather than failing
  (grid_launcher/background/workers.py:633).

Then `_on_install_finalize_finished` (grid_launcher/ui/mixins/install_mixin.py:1626)
registers the installed game, auto-configures the emulator entry
(grid_launcher/ui/mixins/install_mixin.py:1694 → grid-launcher.py:3622), triggers firmware
install for first-time source emulator installs
(grid_launcher/ui/mixins/install_mixin.py:1696), and records the source install
(grid_launcher/ui/mixins/install_mixin.py:1698). Manual archive adds mark the detected
executable `0o755` on non-`win32` (grid_launcher/ui/mixins/emulator_ui_mixin.py:1399).

**Version check.** `SourceVersionCheckWorker.run` (grid_launcher/background/workers.py:447)
emits a single `{installed_tag, available_tag, error}` dict:

- `direct` → `available_tag == "direct"` with no network call
  (grid_launcher/background/workers.py:461);
- `github` → `/releases/tags/{tag}` for an explicit non-`latest` tag, else `/releases/latest`
  (grid_launcher/background/workers.py:471);
- `gitea` → `{base_url}/api/v1/repos/{owner}/{repo}/releases/latest`
  (grid_launcher/background/workers.py:478);
- unknown provider → `{"installed_tag": "", "available_tag": "", "error": "Unsupported
  provider: <p>"}` (grid_launcher/background/workers.py:487);
- non-dict payload or missing `tag_name` → raises internally and is reported as an error
  string with both tags blank (grid_launcher/background/workers.py:490,
  grid_launcher/background/workers.py:500).

## Invariants and error handling

- All launch failures are raised as `ValueError` from the pure `prepare_*` functions and
  converted to a "Launch Error" message box by the caller; `OSError` from `Popen` gets a
  separate message (grid_launcher/ui/mixins/details_view_mixin.py:1474). The TV backend
  emits `launchError` instead (grid_launcher/tv/bridge/game_backend.py:307,
  grid_launcher/tv/bridge/game_backend.py:406).
- `prepare_emulator_launch_command` validates in a fixed order: emulator name → entry →
  path text → path exists → ROM text → ROM exists → argument parse
  (grid_launcher/emulator/launch.py:284 … grid_launcher/emulator/launch.py:312). Port this
  order so error messages match.
- Emulator and ROM paths are `expanduser()`-ed but never `resolve()`-d before spawn
  (grid_launcher/emulator/launch.py:297, grid_launcher/emulator/launch.py:305). Only
  RetroArch core paths are resolved (grid_launcher/emulator/launch.py:217).
- `clean_subprocess_env` never mutates its input mapping — it copies first
  (grid_launcher/core/process.py:19), a property pinned by
  tests/test_subprocess_env.py:32.
- When not frozen and without `LD_LIBRARY_PATH_ORIG`, the environment passes through
  untouched (grid_launcher/core/process.py:23, tests/test_subprocess_env.py:28).
- Blank argv elements are always dropped after substitution
  (grid_launcher/emulator/launch.py:125), so a template like `-L "%core%" "%rom%"` with no
  core cannot emit a stray empty argument.
- `emulator_entry_by_name` returns `None` for a blank query
  (grid_launcher/emulator/selection.py:236).
- `default_emulator_autoprofiles` swallows both `json.JSONDecodeError` and `OSError`,
  returning `[]`; a non-list payload also yields `[]`
  (grid_launcher/emulator/profiles.py:419).
- `load_emulator_autoprofiles` returns the cached list unchanged when it is already a list,
  otherwise loads + normalizes, and returns `[]` when normalization is empty
  (grid_launcher/emulator/profiles.py:566).
- `emulator_install_directory` sanitizes the emulator name with a fallback of `"emulator"`,
  replacing `<>:"/\|?*` and control characters with `_` and converting trailing spaces/dots
  to `_` (grid_launcher/emulator/autoconfig.py:15, grid_launcher/core/path.py:7).
- Source resolution errors are a dedicated `EmulatorSourceResolutionError(ValueError)`
  (grid_launcher/emulator/source.py:7), so callers catching `ValueError` catch them too.

## Platform differences

| Concern | Windows (`win32`) | Linux | macOS (`darwin`) |
|---|---|---|---|
| `creationflags` | `CREATE_NEW_PROCESS_GROUP` | `0` | `0` (grid_launcher/ui/mixins/details_view_mixin.py:1439) |
| RetroArch core extension | `.dll` | `.so` | `.dylib` (grid_launcher/emulator/launch.py:40) |
| Compat tool at launch | `default_compat_tool` forced to `""` | default applies | default applies (grid_launcher/ui/mixins/details_view_mixin.py:1433) |
| Compat-tool UI | combo/labels not built; download dialog returns early | shown | shown (grid_launcher/ui/dialogs.py:231, grid_launcher/ui/mixins/emulator_ui_mixin.py:324) |
| Windows-only profiles | always available | gated out by slug/allowlist | gated out (grid_launcher/emulator/profiles.py:34) |
| `windows_assets` asset override | applied | ignored | ignored (grid_launcher/background/workers.py:327) |
| Wine prefix creation at install | not created | `<dir>/prefix` created | not created (grid_launcher/background/workers.py:578) |
| `chmod 0o755` on detected emulator executable | skipped | applied | applied (grid_launcher/ui/mixins/emulator_ui_mixin.py:1397) |
| `LD_LIBRARY_PATH` handling | irrelevant (variable absent) | active | active (grid_launcher/core/process.py:20) |
| Managed compat-tool dir | `XDG_DATA_HOME` fallback `~/.local/share` | same | same (grid_launcher/core/path.py:40) |
| Read-only-file removal during uninstall | `stat.S_IWRITE` chmod | n/a | n/a (grid_launcher/ui/mixins/install_mixin.py:1090) |

`sys.platform` is also compared with `startswith` in several places
(`win`, `win32`, `linux`) — `profiles.is_available_on_current_platform` uses
`startswith("win")` (grid_launcher/emulator/profiles.py:34), the download worker uses
`startswith("win32")` (grid_launcher/background/workers.py:327), and finalize uses
`startswith("linux")` (grid_launcher/background/workers.py:578).

## Concurrency

### Desktop session tracking

- Sessions live in `self.active_game_sessions`, a list of
  `{game, process, emulator_name, started_at}` (grid_launcher/ui/mixins/cloud_mixin.py:2832).
- A session is registered **only for emulated launches**
  (grid_launcher/ui/mixins/details_view_mixin.py:1472) and only when at least one of the
  save/state cloud-block reasons is empty
  (grid_launcher/ui/mixins/cloud_mixin.py:2825).
- Registration also stamps `last_session_started_at` and clears `last_session_ended_at`
  (grid_launcher/ui/mixins/cloud_mixin.py:2839).
- A repeating `QTimer` with a 2500 ms interval drives `_poll_active_game_sessions`
  (grid-launcher.py:513).
- `partition_active_game_sessions` (grid_launcher/library/cloud_sync.py:114) splits the list:
  a session whose `process` has no callable `poll` is **dropped from both lists** (leaked
  silently); a `poll()` that raises keeps the session as still-running; `poll() is None`
  means running; anything else means finished.
- Finished sessions get `last_session_ended_at` written, then, if auto-upload is enabled and
  the server is reachable, an upload is scheduled after
  `_auto_cloud_upload_delay_seconds()` (or immediately when the delay is ≤ 0)
  (grid_launcher/ui/mixins/cloud_mixin.py:2874).
- The 500 ms `QTimer.singleShot` early-exit warning is independent of session tracking and
  fires for both native and emulated launches
  (grid_launcher/ui/mixins/details_view_mixin.py:1441,
  grid_launcher/ui/mixins/details_view_mixin.py:1471).

### TV backend session tracking

- Exactly one process at a time: `self._process`
  (grid_launcher/tv/bridge/game_backend.py:150); a second `launchGame` while active is
  rejected (grid_launcher/tv/bridge/game_backend.py:212).
- Exit detection is a dedicated `QThread` that blocks on `process.wait()` and emits an
  `_exited` signal with a queued connection (grid_launcher/tv/bridge/game_backend.py:77,
  grid_launcher/tv/bridge/game_backend.py:416). Exceptions from `wait()` silently end the
  watch thread (grid_launcher/tv/bridge/game_backend.py:92).
- `stopGame` calls `process.terminate()` when `poll()` is `None`, swallowing `OSError`, then
  clears the session state and emits `sessionEnded`
  (grid_launcher/tv/bridge/game_backend.py:440). There is no `kill()` escalation and no
  wait-with-timeout.
- `pauseEmulator` / `resumeEmulator` use `psutil.Process(pid).suspend()/.resume()` and are
  no-ops when psutil is unavailable (grid_launcher/tv/bridge/game_backend.py:459).
- Pre-launch cloud restore runs on a worker thread; the resolved command is parked in
  `_pending_restore_launch` and spawned from the worker's `finished` slot
  (grid_launcher/tv/bridge/game_backend.py:326,
  grid_launcher/tv/bridge/game_backend.py:437).

### Install concurrency

One download and one finalize at a time; further requests are appended to `install_queue`
and started by `_start_next_queued_install` (grid_launcher/ui/mixins/install_mixin.py:1462,
grid_launcher/ui/mixins/install_mixin.py:1718). Download and finalize each run on their own
`QThread` with `deleteLater` cleanup (grid_launcher/ui/mixins/install_mixin.py:1492).

## Test oracle

| File | What it pins |
|---|---|
| tests/test_compat_tool_launch.py | The whole `prepare_native_launch_command` matrix: no tool (tests/test_compat_tool_launch.py:27), wine with and without a prefix (tests/test_compat_tool_launch.py:35, tests/test_compat_tool_launch.py:47), Proton via umu present/absent (tests/test_compat_tool_launch.py:57, tests/test_compat_tool_launch.py:72), default-tool fallback for both Proton and wine (tests/test_compat_tool_launch.py:81, tests/test_compat_tool_launch.py:92), per-game override beating the default (tests/test_compat_tool_launch.py:103), and `detect_umu_run` (tests/test_compat_tool_launch.py:124) |
| tests/test_emulator_profiles.py | Profile matching, autoprofile defaults, and the RPCS3 argument-template parsing used by directory/save resolution (tests/test_emulator_profiles.py:267, tests/test_emulator_profiles.py:296) |
| tests/test_emulator_source.py | Release/asset selection success and failure paths, provider aliases and Gitea `base_url` handling, and autoprofile normalization preserving `source`, `screenshot_directories`, and `firmware_directories` (tests/test_emulator_source.py:39, tests/test_emulator_source.py:192, tests/test_emulator_source.py:299) |
| tests/test_source_version_check.py | Version-check endpoint choice per provider and error shaping (tests/test_source_version_check.py:19, tests/test_source_version_check.py:87, tests/test_source_version_check.py:152) |
| tests/test_subprocess_env.py | `clean_subprocess_env` restore/drop/passthrough/no-mutation/os.environ-default (tests/test_subprocess_env.py:12) |
| tests/test_platform_gating.py | `is_available_on_current_platform` slug and allowlist rules (tests/test_platform_gating.py:25), autoprofile list filtering with full-cache preservation (tests/test_platform_gating.py:143), dialog profile filtering (tests/test_platform_gating.py:163), Xbox 360 content platform gate (tests/test_platform_gating.py:200) |
| tests/test_firmware_launch.py | Firmware-before-launch ordering and non-blocking semantics (tests/test_firmware_launch.py:82), plus the RPCS3 custom-config copy conditions at launch (tests/test_firmware_launch.py:194) |
| tests/test_native_game_settings.py | `NativeGameSettingsDialog` compat-tool combo: empty list, wine entry, preselection, and "None" returning `""` (tests/test_native_game_settings.py:30) |
| tests/test_emulator_install_subfolder.py | Source downloads land in `<library>/Emulators/<name>/`, the folder exists before the download starts, nothing is written to the `Emulators` root, and supplementals stay in the same subfolder (tests/test_emulator_install_subfolder.py:73) |
| tests/test_install_paths_native_resolver.py | `native_game_dir` participation in archive/extracted candidate lists and uninstall removal (tests/test_install_paths_native_resolver.py:28) |
| tests/test_tv_game_backend.py | TV launch/stop/pause backend behavior |
| tests/test_retroarch_config.py, tests/test_duckstation_config.py, tests/test_vita3k.py, tests/test_flycast_vmu.py | Per-emulator settings writers (doc 05) |

Run everything with `python -m unittest discover tests/`.

## Open questions

- `OPEN QUESTION:` `_record_compat_tool_install`
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:190) has no callers anywhere in the
  repository, and `_on_install_finalize_finished` treats `is_source_install` as only
  `{source_emulator, source_emulator_update}` (grid_launcher/ui/mixins/install_mixin.py:1630),
  excluding `compat_tool`. `InstallFinalizeWorker` computes `_compat_tool_install_path`
  (grid_launcher/background/workers.py:602) but nothing consumes it. Is a downloaded
  compatibility tool ever written into `config["compat_tool_installs"]`, or is it only
  discovered afterwards by `_scan_system_proton_installs`? A port needs to know whether
  managed installs are expected to persist.
- `OPEN QUESTION:` Platform gating is applied to the *autoprofile list*
  (grid_launcher/ui/mixins/emulator_ui_mixin.py:466), but `_emulator_supports_platform`
  returns `True` when no profile matches (grid-launcher.py:3579). A configured
  `Xenia (Xbox 360)` entry on Linux therefore loses its profile and becomes "supports every
  platform" rather than being excluded. Is this intended, or should a filtered-out profile
  make the entry unusable?
- `OPEN QUESTION:` Two different platform-allowlist semantics coexist for
  `source["platforms"]`: exact equality in `is_available_on_current_platform`
  (grid_launcher/emulator/profiles.py:53) and prefix matching everywhere else
  (grid_launcher/ui/emulators.py:191, grid_launcher/background/workers.py:180,
  grid_launcher/ui/mixins/install_mixin.py:1422). Which is authoritative for a port?
- `OPEN QUESTION:` `validate_launch_placeholders` already raises when the template mentions
  `%core%` with no configured core (grid_launcher/emulator/launch.py:141), which makes the
  orphaned-`-L` cleanup inside `apply_launch_placeholders_to_args`
  (grid_launcher/emulator/launch.py:121) unreachable from the normal launch path. Is the
  cleanup dead code, or is there a caller that intentionally skips validation?
- `OPEN QUESTION:` `%ps3_gameid%` appears in argument templates used by tests and by the
  RPCS3 directory/save parsers (tests/test_emulator_profiles.py:267) but is not a member of
  the placeholder map (grid_launcher/emulator/launch.py:97), so it would reach the emulator
  verbatim. Is it a retired placeholder that a port should ignore, or should it expand?
- `OPEN QUESTION:` `partition_active_game_sessions` silently discards a session whose
  `process` object has no callable `poll` (grid_launcher/library/cloud_sync.py:120), so its
  end-of-session cloud upload never happens. Intended, or a bug to preserve?
- `OPEN QUESTION:` Native launches never register a cloud session
  (grid_launcher/ui/mixins/details_view_mixin.py:1441 returns before
  `_register_game_session_for_auto_upload`), consistent with
  `cloud_save_block_reason_for_game` blocking native games
  (grid_launcher/emulator/selection.py:110). Should a port keep native games entirely out of
  session tracking, including playtime accounting?
- `OPEN QUESTION:` The desktop launch of a game whose platform is literally `"Emulators"`
  falls through to `prepare_emulator_launch_command` and would fail with "No emulator is
  configured." unless the Play button is suppressed elsewhere. What is the intended Play
  behavior for entries on the `Emulators` platform?
- `OPEN QUESTION:` `stopGame` only calls `terminate()` and never escalates to `kill()` or
  waits for exit (grid_launcher/tv/bridge/game_backend.py:444). Should a port add a timeout
  and force-kill, or reproduce the current best-effort behavior?
- `OPEN QUESTION:` The compat-tool radio list omits the "None" option that the per-game
  dialog offers (grid_launcher/ui/mixins/emulator_ui_mixin.py:294 vs
  grid_launcher/ui/mixins/emulator_ui_mixin.py:232), so once a global default is chosen it
  cannot be cleared from that panel. Intended?

## Source map

| Path | Role |
|---|---|
| grid_launcher/emulator/launch.py | Pure command construction: suffix sets, `detect_umu_run`, RetroArch core path/value, placeholder build/validate/apply, template splitting, ROM resolution, `prepare_native_launch_command`, `prepare_emulator_launch_command`, early-exit message |
| grid_launcher/emulator/selection.py | Platform predicates (`is_arcade_platform`, `is_native_executable_platform`, `is_ps3/ps4/xbox360_platform`, `is_emulators_platform`), mapping lookup, entry lookup, compatible/default/available emulator resolution, install block reason, cloud-save scope/block helpers |
| grid_launcher/emulator/profiles.py | `_WINDOWS_ONLY_EMULATOR_SLUGS`, `is_available_on_current_platform`, token/keyword matching, `emulator_profile_for_entry`, `emulator_profile_for_game`, autoprofile load + normalize |
| grid_launcher/emulator/source.py | Pure source-metadata normalization and GitHub/Gitea release + asset selection; `EmulatorSourceResolutionError` |
| grid_launcher/emulator/wine.py | Windows env-var path → Wine prefix path translation only |
| grid_launcher/emulator/autoconfig.py | `emulator_install_directory`, `select_emulator_executable_path`, `auto_configure_emulator_settings` and manual-entry defaults |
| grid_launcher/core/process.py | `clean_subprocess_env` (LD_LIBRARY_PATH restore/drop) |
| grid_launcher/core/path.py | `sanitize_path_component`, XDG helpers, `grid_launcher_share_dir`, `compat_tool_install_directory` |
| grid_launcher/core/config.py | `normalize_emulators`, `normalize_installed_games` (native fields), `normalize_compat_tool_installs` |
| grid_launcher/library/install_paths.py | Native install dir and executable candidate/selection resolvers |
| grid_launcher/library/cloud_sync.py | `partition_active_game_sessions`, `session_cloud_sync_updates`, auto-upload planning |
| grid_launcher/background/workers.py | `InstallDownloadWorker` source resolution + streaming download, `SourceVersionCheckWorker`, `InstallFinalizeWorker` (wine prefix, compat-tool path, firmware hook) |
| grid_launcher/ui/mixins/details_view_mixin.py | `_launch_installed_game`, `_perform_game_action`, `_warn_if_process_exited_early`, per-game native settings dialog wiring, thin wrappers over the pure launch helpers |
| grid_launcher/ui/mixins/emulator_ui_mixin.py | Compat-tool discovery/list/default persistence, autoprofile access with platform filtering, source install bookkeeping, emulator archive extraction and chmod, `_ensure_emulator_sync_settings` |
| grid_launcher/ui/mixins/install_mixin.py | Install dispatch by `_install_mode`, source install start, finalize handling, firmware install without UI |
| grid_launcher/ui/emulators.py | Source download row building, filtering, and labels |
| grid_launcher/ui/dialogs.py | `NativeGameSettingsDialog` (executable / compat tool / parameters), supported-profile loading with platform gating |
| grid_launcher/tv/bridge/game_backend.py | TV/QML launch path: `launchGame`, `_handle_native_launch`, `_do_launch`, `stopGame`, pause/resume, `_ProcessWatchThread` |
| grid-launcher.py | `MainWindow` glue: `_emulator_supports_platform`, `_resolved_native_executable_path_for_game`, `_ps3_game_id_for_game`, `_auto_configure_installed_emulator`, session poll timer, config defaults |
| emulator-autoprofiles.json | Shipped profiles: match tokens, argument templates, platform keywords, source metadata, compat-tool entries |

## Rust port deviations (milestone 3)

Deliberate deviations from the reference when porting the launch module to Rust (grid-core):

1. Duplicate launches of the same rom are rejected (reference desktop allowed them; the TV backend allowed one global session — we allow one per rom).
2. Sessions are tracked for every emulated launch and drive UI state; the reference tracked them only for cloud auto-upload.
3. PS3 titles cannot resolve `%ps3_launch_target%` yet (registry lacks PS3 fields until the PS3 install milestone); the reference's validation error is shown.
4. RetroArch platform support = a non-blank `retroarch_cores` config entry, not a scan of installed core files.
5. The per-platform default picker lists all emulators rather than filtering by the supports-platform test; the test still gates automatic selection.
6. Desktop UI gains a Stop button (reference desktop had none).
7. No `_ensure_emulator_sync_settings` call before spawn (doc 05 deferred).

## Rust port deviations (milestone 4)

Deliberate deviations from the reference when porting emulator acquisition (catalog listing,
download, install) to Rust (grid-core):

1. Installed emulators are config entries only — never pseudo-rows in the installed-games
   registry (the reference listed them as library items).
2. Compat-tool profiles are excluded from the catalog entirely this milestone (reference
   listed them in a separate dialog).
3. Version checks deferred; `source_*` fields recorded now.
4. Supplemental failures fail the install (visible) rather than partially succeeding.
5. No firmware step after emulator install (firmware subsystem deferred).
6. `launch_executable`, present in several catalog `source` blocks, is intentionally unread:
   the reference never reads it either — executable choice is scoring-only — so the port
   never reads it (parity with the reference, not a gap).
7. Emulator archive extraction stages into a temporary `.extract-tmp` directory (supplemental
   archives into `.supp-tmp-<n>`) that is merged into the install directory and removed,
   rather than extracting in place.
8. A missing launchable emulator executable after install is a visible failure ("No launchable
   emulator executable was found after install") rather than a silently pathless entry.
9. `merge_tree_into` resolves a file-vs-directory conflict at the destination by removing the
   destination entry first, where the reference's `_merge_tree` raises on both conflict
   shapes.
10. Supplemental delete-failure warnings reuse one short form, "could not delete archive:
    <path>", for both the game and the emulator-supplemental case; the reference uses two
    distinct longer strings ("Extracted <title>, but could not delete archive:\n<error>" vs
    "Applied supplemental emulator files, but could not delete archive:\n<error>").
11. Remote asset file names are validated — rejected if empty, if they contain a path
    separator, or if they are not their own file name — before being joined to the install
    directory; the reference instead fails incidentally when `Path.with_name` raises
    `ValueError` on a separator-bearing name.
12. Executable selection accepts, on unix, an extracted file whose name carries no `.` at
    all and whose filesystem executable bit is set, alongside the reference's
    `.exe/.bat/.cmd/.ps1/.sh/.AppImage` suffix set. Deliberate improvement: the reference's
    name-only `launchable_emulator_file` can never install an emulator that ships a bare ELF
    binary (the catalog's Redream tarball ships `redream`, with no extension). Dot files
    (`.hidden`) and suffixed files (`libfoo.so`) never qualify, and windows keeps the
    suffix-only rule.
13. On unix, zip and 7z extraction now preserve every extracted member's stored Unix
    permission bits (masked to `mode & 0o777`; setuid/setgid/sticky are never propagated),
    not only the single file `launchable_emulator_file` selects. This is a strictly larger
    improvement than "preserves what the reference sometimes preserved": the reference's
    zip extraction drops permissions on *every* code path, full stop. `archive_preparation.py`
    extracts most zip members via stdlib `ZipFile.extract()` — but CPython's
    `ZipFile._extract_member` (verified directly, `/usr/lib64/python3.12/zipfile/__init__.py`
    lines 1788-1839) never calls `chmod`, `os.chmod`, or touches `external_attr` anywhere in
    that method; `external_attr` is a write-side-only field (set when *building* an archive,
    e.g. `zinfo.external_attr = (st.st_mode & 0xFFFF) << 16` at line 590) that stdlib
    `extract()`/`extractall()` never reads back on POSIX. Any entry whose name needs backslash
    normalization is instead written with
    `normalized_path.write_bytes(archive.read(member))`
    (`grid_launcher/library/archive_preparation.py:633-637`), which is no different in this
    respect — both branches produce umask-only permissions. 7z extraction has no explicit
    permission handling either: it shells out to a system `7z`/`7za` binary or falls back to
    `py7zr.SevenZipFile(...).extractall(...)`
    (`grid_launcher/library/archive_preparation.py:441-442`), so whatever Unix attributes
    survive there depend entirely on that external tool/library, not on code in this repo.
    The reference's *only* source of zip-member executability, without exception, is a
    separate, unconditional `os.chmod(extracted_file, 0o755)` on the one resolved launch file
    after extraction (`grid_launcher/library/archive_preparation.py:1171-1175`) — it does not
    depend on, and is not preceded by, any permission bits from extraction itself. The port's
    extraction-time preservation is therefore a strict superset for zip: it is the only code
    path (reference or port) that leaves *companion* files — helper scripts, other binaries in
    the extracted tree, not just the one selected launch file — with a meaningful exec bit.
