# 02 — Configuration, Persistence Roots, and Secret Storage

## Purpose

This document describes how GRID Launcher stores and reads its persistent state so the behavior can be reimplemented in another language. It covers three concerns:

1. The single JSON configuration document (`config.json`) — its location, schema, merge rules, and per-key normalization.
2. The path conventions used everywhere else in the codebase (config root, data root, path sanitizing, path comparison, bundled-data resolution).
3. Secret storage for the RomM API token, the RetroAchievements session token, and the RetroAchievements API key — OS keychain first, encrypted-file fallback second, with a one-way migration from an older file format.

Every behavioral claim below carries a source anchor.

---

## External surfaces

### Persistence root

All user state lives in one directory. It is `~/.grid-launcher` on every platform — there is no XDG or `%APPDATA%` variant for this root (grid-launcher.py:2386-2387). The same literal is duplicated in the TV-mode bridges (grid_launcher/tv/bridge/app_backend.py:530, grid_launcher/tv/bridge/game_backend.py:771, grid_launcher/tv/bridge/game_backend.py:800) and in the archive-tool helper (grid_launcher/library/archive_preparation.py:19).

`~` here is the process user's home directory as the platform resolves it.

| File / directory | Path | Format | Written by |
| --- | --- | --- | --- |
| Config document | `~/.grid-launcher/config.json` | JSON object, UTF-8, 2-space indent, keys sorted ascending | grid-launcher.py:2392-2393, grid_launcher/core/config.py:261-266 |
| API token (legacy fallback) | `~/.grid-launcher/token.bin` | Binary; DPAPI blob on Windows, base64 text on other platforms | grid-launcher.py:2395-2396, grid_launcher/core/token_store.py:110-115 |
| RetroAchievements token (legacy fallback) | `~/.grid-launcher/ra_token.bin` | Same as above | grid-launcher.py:2398-2399 |
| RetroAchievements API key (legacy fallback) | `~/.grid-launcher/ra_api_key.bin` | Same as above | grid-launcher.py:2401-2402 |
| Cover image cache | `~/.grid-launcher/imagecache/` | Raw image bytes, one file per cached cover; filename is `<cover cache key><extension>` | grid-launcher.py:2389-2390, grid_launcher/cover/cache.py:48-54 |
| Discover section cache | `~/.grid-launcher/discover_cache.json` | JSON object: `section_id -> {"data": …, "timestamp": <epoch seconds>}` | grid-launcher.py:2404-2405, grid_launcher/server/discover.py:111-122 |
| Watchlist | `~/.grid-launcher/watchlist.json` | JSON object: `rom_id -> game card object`; a bare JSON array of rom-id strings is the accepted legacy form | grid-launcher.py:2407-2408, grid_launcher/server/discover.py:582-618 |
| Discover analytics | `~/.grid-launcher/discover_events.jsonl` | Append-only JSON Lines; one object per line | grid-launcher.py:2446-2447, grid_launcher/server/discover.py:561-579 |
| Discover UI state | `~/.grid-launcher/discover_ui.json` | JSON object; three keys are read: `preferred_platforms` (list of strings), `hidden_sections` (list of section keys, grid-launcher.py:991-992), and `section_order` (list of section keys, grid-launcher.py:1279-1280, :1361) | grid-launcher.py:2452-2453, grid-launcher.py:2455-2475, grid-launcher.py:2503-2507 |
| Downloaded archive tools | `~/.grid-launcher/tools/` (`7zr.exe`, `7zz.exe`) | Executables fetched at runtime | grid_launcher/library/archive_preparation.py:19-23 |
| Installed compatibility tools | `<XDG data home>/grid-launcher/compat-tools/` | Extracted tool trees | grid_launcher/core/path.py:60-61 |

Secrets normally do **not** live in any of the files above; they live in the OS keychain (see "Behavior → Secret storage").

#### config.json example

Written by grid_launcher/core/config.py:261-266. Secret fields are always blanked on write (grid_launcher/core/config.py:253-258).

```json
{
  "api_token": "",
  "auto_cloud_save_download_on_launch": true,
  "auto_cloud_save_skip_download_if_local_newer": true,
  "auto_cloud_save_upload_delay_seconds": 3,
  "auto_cloud_save_upload_on_exit": true,
  "cloud_sync_state": {
    "rom:1042": {
      "last_downloaded_save_id": "884",
      "last_server_timestamp": 1770000000.0,
      "last_uploaded_local_mtime": 1769999000.0
    }
  },
  "compat_tool_installs": {},
  "debug_prints": true,
  "default_compat_tool": "",
  "default_emulators": { "PlayStation 2": "PCSX2" },
  "default_retroarch_cores": { "snes": "snes9x_libretro" },
  "emulator_source_installs": {},
  "emulators": [
    {
      "args": "%rom%",
      "ignore_extensions": "",
      "ignore_files": "",
      "name": "PCSX2",
      "path": "/usr/bin/pcsx2",
      "save_paths": "",
      "save_strategy": "auto",
      "state_paths": ""
    }
  ],
  "first_run_completed": true,
  "installed_games": [],
  "launch_args": "",
  "library_path": "/home/user/Games",
  "retroachievements_api_key": "",
  "retroachievements_token": "",
  "retroachievements_username": "",
  "server_url": "https://romm.example.com",
  "theme": "system",
  "tv_guide_button_default_opt_outs": [],
  "tv_guide_button_exclusion_list": [],
  "tv_mode_home_view": "home",
  "tv_mode_last_active": false,
  "username": "alice",
  "window_geometry": "AdnQywADAAAAAAAA…",
  "window_state": "normal"
}
```

#### Secret file example (legacy fallback only)

`token.bin` / `ra_token.bin` / `ra_api_key.bin` hold a single opaque byte string with no header, length prefix, or trailing newline:

- Windows: the raw output of the platform DPAPI "protect" call over the UTF-8 secret bytes (grid_launcher/core/token_store.py:150-152).
- Other platforms: standard base64 of the UTF-8 secret bytes. Nothing in the current code writes this form; it is only ever decoded (grid_launcher/core/token_store.py:112-116, grid_launcher/core/token_store.py:147-156).

### Bundled read-only data files

These ship with the application and are never written:

| File | Resolved by |
| --- | --- |
| `emulator-autoprofiles.json` | grid_launcher/emulator/profiles.py:409-410, grid_launcher/ui/mixins/emulator_ui_mixin.py:445-446 |
| `retroarch-core-list.json` | grid_launcher/emulator/retroarch.py:14-15, grid_launcher/ui/mixins/emulator_ui_mixin.py:468-469 |
| `romm-platform-cores.json` | grid_launcher/emulator/retroarch.py:18-19, grid_launcher/ui/mixins/emulator_ui_mixin.py:471-478 |
| `assets/` tree (icons, QSS colors, RetroArch assets) | grid_launcher/ui/theme.py:50, grid_launcher/server/catalog.py:11, grid_launcher/tv/widgets/views/details_view.py:30-31 |

### Environment variables read

| Variable | Effect |
| --- | --- |
| `XDG_CONFIG_HOME` | Config root for helpers that use the XDG convention; empty or unset falls back to `~/.config` (grid_launcher/core/path.py:33-37) |
| `XDG_DATA_HOME` | Data root; empty or unset falls back to `~/.local/share` (grid_launcher/core/path.py:40-44) |
| `GRID_LAUNCHER_SHARE_DIR` | Absolute override for the bundled-data directory; empty or unset falls back to the caller-supplied path (grid_launcher/core/path.py:47-57) |
| `LD_LIBRARY_PATH_ORIG` | Restores the pre-bundle loader path when spawning host binaries (grid_launcher/core/process.py:19-24) |
| `USERPROFILE` | Windows fallback when resolving the Documents folder for emulator save-path tokens (grid_launcher/ui/mixins/cloud_mixin.py:936) |

---

## Data model

### Config schema (top-level keys)

The defaults object is the schema: only keys present in it survive a load (grid-launcher.py:2329-2360, grid_launcher/core/config.py:229). Types below are the default's type, which also drives merge behavior.

| Key | Type | Default | Semantics |
| --- | --- | --- | --- |
| `server_url` | string | `""` | RomM server base URL. Consumers trim it and strip trailing `/` (grid_launcher/server/state.py:14-18). |
| `api_token` | string | `""` | RomM API token, in memory only. Always written to disk as `""` (grid_launcher/core/config.py:255). |
| `username` | string | `""` | Last known RomM account name, used for the "Logged in as" label (grid_launcher/server/state.py:21-25). |
| `library_path` | string | `""` | Root directory into which games are installed. |
| `first_run_completed` | bool | `false` | Suppresses the first-run setup dialog. Read leniently: strings `1/true/yes/on` and `0/false/no/off` are accepted (grid-launcher.py:2628-2638). |
| `launch_args` | string | `""` | Extra launch arguments passed through to the launch path (grid_launcher/ui/mixins/details_view_mixin.py:1406). Preserved verbatim across settings saves (grid-launcher.py:2595-2597). |
| `debug_prints` | bool | `true` | Enables debug logging; same lenient string parsing (grid-launcher.py:2150-2156). |
| `theme` | string | `"system"` | One of `system`, `dark`, `light`; anything else collapses to `system` (grid_launcher/ui/theme.py:140-146). |
| `window_geometry` | string | `""` | Base64 of the opaque toolkit geometry blob; invalid base64 is ignored (grid-launcher.py:2364-2380). |
| `window_state` | string | `"normal"` | `"maximized"` (case-insensitive) restores a maximized window; any other value means normal (grid-launcher.py:2382-2384). |
| `emulators` | list of objects | `[]` | Configured emulators. Normalized — see "Emulator entry". |
| `default_emulators` | object | `{}` | Platform label -> emulator name. Normalized. |
| `default_retroarch_cores` | object | `{}` | Platform slug -> RetroArch core id. Normalized. |
| `installed_games` | list of objects | `[]` | The local library. Normalized — see "Installed game entry". |
| `emulator_source_installs` | object | `{}` | Emulator id (lowercased) -> install provenance record. Normalized outside the generic merge (grid-launcher.py:2535-2537). |
| `compat_tool_installs` | object | `{}` | Compat-tool id -> `{name, compat_tool_type, install_path}`. A normalizer exists (grid_launcher/core/config.py:193-212) but the merge never applies it — see "Invariants". |
| `default_compat_tool` | string | `""` | Path/identifier of the compat tool used for Windows-native games on non-Windows hosts (grid_launcher/ui/mixins/emulator_ui_mixin.py:178-183). |
| `auto_cloud_save_download_on_launch` | bool | `true` | Download cloud saves before launch (grid-launcher.py:2211-2212). |
| `auto_cloud_save_upload_on_exit` | bool | `true` | Upload cloud saves after the game exits (grid-launcher.py:2214-2215). Bound to the same checkbox as the download flag on save (grid-launcher.py:2600-2602). |
| `auto_cloud_save_skip_download_if_local_newer` | bool | `true` | Skip download when the local save is newer (grid-launcher.py:2217-2218). |
| `auto_cloud_save_upload_delay_seconds` | int | `3` | Delay before the post-exit upload; clamped to `[0, 60]` on read; string values are parsed as integers, booleans are rejected (grid-launcher.py:2198-2209, grid-launcher.py:2220-2221). |
| `cloud_sync_state` | object | `{}` | Sync bookkeeping keyed by game — see "Cloud sync state entry". |
| `retroachievements_username` | string | `""` | RetroAchievements account name. |
| `retroachievements_api_key` | string | `""` | RetroAchievements API key, in memory only; always blanked on write (grid_launcher/core/config.py:257). |
| `retroachievements_token` | string | `""` | RetroAchievements session token, in memory only; always blanked on write (grid_launcher/core/config.py:256). |
| `tv_mode_home_view` | string | `"home"` | Which TV-mode view opens on entry (grid_launcher/tv/bridge/app_backend.py:129, grid_launcher/tv/bridge/app_backend.py:565). |
| `tv_guide_button_exclusion_list` | list of strings | `[]` | Emulators for which the TV guide button is disabled. Entries are stringified and trimmed on read; blanks dropped (grid_launcher/tv/bridge/app_backend.py:537-546). |
| `tv_guide_button_default_opt_outs` | list of strings | `[]` | Emulators the user explicitly opted out of the built-in exclusion defaults (grid_launcher/ui/mixins/emulator_ui_mixin.py:1414-1445). Built-in default exclusions are `rpcs3, cemu, dolphin, xemu, xenia, retroarch` (grid_launcher/ui/mixins/emulator_ui_mixin.py:102-104). |
| `tv_mode_last_active` | bool | `false` | Whether the app was last in TV mode. |

### Emulator entry (`emulators[]`)

Produced by grid_launcher/core/config.py:8-81. Always exactly these eight keys, plus up to five optional provenance keys that are emitted only when non-empty.

| Field | Type | Default when missing/invalid | Notes |
| --- | --- | --- | --- |
| `name` | string | entry dropped | Trimmed. Blank or non-string drops the whole entry (grid_launcher/core/config.py:32-33). |
| `path` | string | `""` | Trimmed (grid_launcher/core/config.py:34-35, :60). |
| `args` | string | `"%rom%"` | Trimmed; an all-whitespace value also becomes `"%rom%"` (grid_launcher/core/config.py:36-37, :61). |
| `save_strategy` | string | `"auto"` | Passed through the save-strategy normalizer (grid_launcher/core/config.py:62). |
| `ignore_files` | string | `""` | Trimmed (grid_launcher/core/config.py:63). |
| `ignore_extensions` | string | `""` | Trimmed (grid_launcher/core/config.py:64). |
| `save_paths` | string | `""` | Trimmed (grid_launcher/core/config.py:65). |
| `state_paths` | string | `""` | Trimmed (grid_launcher/core/config.py:66). |
| `source_id` | string | key omitted | Emitted only if non-empty after trimming (grid_launcher/core/config.py:68-69). |
| `source_provider` | string | key omitted | grid_launcher/core/config.py:70-71 |
| `source_owner` | string | key omitted | grid_launcher/core/config.py:72-73 |
| `source_repo` | string | key omitted | grid_launcher/core/config.py:74-75 |
| `source_release_tag` | string | key omitted | grid_launcher/core/config.py:76-77 |

Save-strategy normalization maps a case-folded, trimmed value through this alias table and falls back to `auto` for anything unlisted (grid_launcher/emulator/profiles.py:141-156):

| Input (case-folded) | Output |
| --- | --- |
| `""`, `auto` | `auto` |
| `singlefile`, `single_file`, `single-file`, `single file`, `file` | `single_file` |
| `folder`, `directory`, `folder_per_game`, `folder-per-game` | `folder` |
| anything else | `auto` |

### Installed game entry (`installed_games[]`)

Produced by grid_launcher/core/config.py:106-190. The output object has a fixed key set; **any key not listed here is discarded**.

| Field | Type | Default when missing/invalid | Notes |
| --- | --- | --- | --- |
| `title` | string | entry dropped | Trimmed; blank drops the entry (grid_launcher/core/config.py:120-121). |
| `platform` | string | entry dropped | Trimmed; blank drops the entry (grid_launcher/core/config.py:122-123). |
| `rating` | string | `"N/A"` | Blank or non-string becomes `"N/A"` (grid_launcher/core/config.py:155). |
| `description` | string | `"No description available."` | grid_launcher/core/config.py:156 |
| `cover_url` | string | `""` | grid_launcher/core/config.py:157 |
| `cached_cover_path` | string | `""` | Absolute path into the image cache (grid_launcher/core/config.py:158). |
| `screenshot_urls` | string | `""` | Serialized as a single string, not a list (grid_launcher/core/config.py:159). |
| `genres` | string | `""` | grid_launcher/core/config.py:160 |
| `regions` | string | `""` | grid_launcher/core/config.py:161 |
| `filesize_bytes` | string | `""` | Stored as a string (grid_launcher/core/config.py:162). |
| `rom_id` | string | `""` | Server ROM id; primary identity when present (grid_launcher/core/config.py:163). |
| `ra_id` | string | `""` | RetroAchievements game id (grid_launcher/core/config.py:164). |
| `server_updated_at` | string | `""` | Server-side timestamp used for update detection (grid_launcher/core/config.py:165). |
| `rom_file_name` | string | `""` | grid_launcher/core/config.py:166 |
| `extracted_path` | string | `""` | grid_launcher/core/config.py:167 |
| `extracted_dir` | string | `""` | grid_launcher/core/config.py:168 |
| `archive_path` | string | `""` | grid_launcher/core/config.py:169 |
| `native_executable_path` | string | `""` | grid_launcher/core/config.py:170 |
| `native_launch_parameters` | string | `""` | grid_launcher/core/config.py:171 |
| `native_compat_tool` | string | `""` | grid_launcher/core/config.py:172 |
| `native_wineprefix` | string | `""` | grid_launcher/core/config.py:173 |
| `native_game_dir` | string | `""` | grid_launcher/core/config.py:174 |
| `multi_file_game_dir` | string | `""` | grid_launcher/core/config.py:175 |
| `included_dlc` | string | `""` | grid_launcher/core/config.py:176 |
| `ps3_trophy_paths` | string | `""` | grid_launcher/core/config.py:177 |
| `ps3_game_id` | string | `""` | Trimmed **and upper-cased** (grid_launcher/core/config.py:178). |
| `ps3_iso_path` | string | `""` | grid_launcher/core/config.py:179 |
| `ps4_game_id` | string | `""` | Trimmed **and upper-cased** (grid_launcher/core/config.py:180). |
| `ps4_content` | string | `""` | grid_launcher/core/config.py:181 |
| `local_path` | string | `""` | Added after the main object literal (grid_launcher/core/config.py:183-184). |

### Cloud sync state entry (`cloud_sync_state`)

Map key format (grid_launcher/library/cloud_sync.py:66-77):

- `rom:<lowercased rom_id>` when the game has a rom id;
- otherwise `name:<lowercased title>::<lowercased platform>`;
- empty string (entry unusable) when both title and platform are empty.

Value fields, each written only when the input has an acceptable type and a non-empty/parsable value (grid_launcher/library/cloud_sync.py:20-57):

| Field | Type | Condition to be kept |
| --- | --- | --- |
| `last_downloaded_save_id` | string | non-blank string, trimmed (grid_launcher/library/cloud_sync.py:20-22) |
| `last_server_timestamp` | number | numeric, coerced to floating point (grid_launcher/library/cloud_sync.py:24-26) |
| `last_uploaded_local_mtime` | number | numeric (grid_launcher/library/cloud_sync.py:28-30) |
| `last_uploaded_at` | string | non-blank string, trimmed (grid_launcher/library/cloud_sync.py:32-34) |
| `last_downloaded_state_id` | string | non-blank string, trimmed (grid_launcher/library/cloud_sync.py:36-38) |
| `last_uploaded_save_mtime` | number | numeric (grid_launcher/library/cloud_sync.py:40-42) |
| `last_uploaded_state_mtime` | number | numeric (grid_launcher/library/cloud_sync.py:44-46) |
| `last_session_started_at` | number | numeric (grid_launcher/library/cloud_sync.py:48-50) |
| `last_session_ended_at` | number | numeric (grid_launcher/library/cloud_sync.py:52-54) |

An entry whose value object ends up empty is dropped entirely (grid_launcher/library/cloud_sync.py:56-57). Keys that are non-strings, blank, or map to non-objects are skipped (grid_launcher/library/cloud_sync.py:14-15).

### Emulator source install entry (`emulator_source_installs`)

Key is the raw key trimmed **and case-folded**; non-string or blank keys and non-object values are skipped (grid_launcher/ui/mixins/emulator_ui_mixin.py:123-127). The value always has exactly these six string fields, each trimmed, defaulting to `""`: `name`, `provider`, `owner`, `repo`, `release_tag`, `installed_at` (grid_launcher/ui/mixins/emulator_ui_mixin.py:128-132).

### Compat tool install entry (`compat_tool_installs`)

Key is the raw key trimmed if it is a string, otherwise its string rendering (grid_launcher/core/config.py:207). Value has exactly `name`, `compat_tool_type`, `install_path`, all trimmed strings. An entry whose `name` is blank or non-string is dropped (grid_launcher/core/config.py:201-206, :208-211).

---

## Behavior

### Path conventions (cross-cutting)

**`sanitize_path_component(value, fallback)`** — turns arbitrary text into one safe filename component (grid_launcher/core/path.py:7-12):

1. Replace each character that is in the set `< > : " / \ | ? *`, or whose code point is below 32, with `_` (grid_launcher/core/path.py:8-9).
2. While the result ends with a space or a period, replace that trailing character with `_` (grid_launcher/core/path.py:10-11). The replacement appends `_`, so the loop condition is false after one pass and can never run twice: `"name.. "` becomes `"name.._"`, and `"name..."` also becomes `"name.._"`.
3. If, after removing all leading/trailing spaces, underscores, and periods, nothing is left, return `fallback` instead; otherwise return the sanitized string (grid_launcher/core/path.py:12).

Note the rules are applied uniformly on all platforms, not only Windows. Reserved Windows device names (`CON`, `NUL`, `LPT1`, …) are **not** handled. Callers pass domain-specific fallbacks: `"game"` and `"platform"` for install directories (grid_launcher/library/install_metadata.py:21-22), `"emulator"` for emulator directories (grid_launcher/emulator/autoconfig.py:15), `"save"` / `"state"` for cloud-save filenames (grid_launcher/ui/mixins/cloud_mixin.py:1858, :1888).

**`path_key(path)`** — canonical comparison form of a path (grid_launcher/core/path.py:15-20):

1. Expand a leading `~` to the home directory.
2. Fully resolve the path (following links, normalizing `.`/`..`) without requiring the path to exist; if resolution raises an OS error, use the merely expanded path (grid_launcher/core/path.py:18-20).
3. Case-fold the resulting string.

Because it case-folds unconditionally, path comparison is case-insensitive on every platform, including case-sensitive filesystems. It is used as a dictionary key for cover-cache identity (grid_launcher/cover/utils.py:202) and for emulator-path matching (grid_launcher/library/install_registry.py:77-83).

**`path_within_path(path, root)`** — containment test (grid_launcher/core/path.py:23-30):

1. Compute `path_key(path)` and `path_key(root)`.
2. Strip any trailing `\` and `/` characters from the root key (grid_launcher/core/path.py:25).
3. If the root key is now empty, return false (grid_launcher/core/path.py:26-27).
4. Return true when the path key equals the root key, or when the path key starts with the root key followed by `\` or by `/` (grid_launcher/core/path.py:28-30).

Both separators are checked regardless of host platform. A path equal to the root counts as "within" it.

**XDG roots** — `XDG_CONFIG_HOME` if set and non-empty (tilde-expanded), else `~/.config` (grid_launcher/core/path.py:33-37); `XDG_DATA_HOME` if set and non-empty (tilde-expanded), else `~/.local/share` (grid_launcher/core/path.py:40-44). Compat tools install under `<data home>/grid-launcher/compat-tools` (grid_launcher/core/path.py:60-61).

### Bundled-data (frozen vs. source) resolution

There is **no** `sys.frozen` / bundle-temp-dir branch in the data-file lookup. Resolution is purely lexical plus one environment override:

1. Start from the module file's location and walk up a fixed number of directories to reach the project/bundle root. For modules in `grid_launcher/ui/mixins/`, that is four levels up (grid_launcher/ui/mixins/emulator_ui_mixin.py:446, :461, :469, :475). For `grid_launcher/emulator/profiles.py`, the default is three levels up (grid_launcher/emulator/profiles.py:414). For `grid_launcher/ui/theme.py` and `grid_launcher/ui/dialogs.py`, three levels up (grid_launcher/ui/theme.py:50, grid_launcher/ui/dialogs.py:32). For `grid_launcher/tv/widgets/...`, five levels up (grid_launcher/tv/widgets/views/details_view.py:30-31).
2. Pass that path as the *fallback* to `grid_launcher_share_dir`. If `GRID_LAUNCHER_SHARE_DIR` is set and non-blank after trimming, its tilde-expanded value wins; otherwise the fallback is used (grid_launcher/core/path.py:47-57).
3. Append the file name: `emulator-autoprofiles.json` (grid_launcher/emulator/profiles.py:409-410), `retroarch-core-list.json` (grid_launcher/emulator/retroarch.py:14-15), `romm-platform-cores.json` (grid_launcher/emulator/retroarch.py:18-19).

This works unchanged in a single-file bundle because the packaging step copies those JSON files to the bundle root (`--add-data "retroarch-core-list.json:."` and siblings, .github/workflows/pyinstaller-linux.yml:54, .github/workflows/pyinstaller-windows.yml:57), which is exactly the directory the fixed upward walk lands on. Flatpak instead sets `GRID_LAUNCHER_SHARE_DIR=/app/share/grid-launcher` (grid_launcher/core/path.py:50-52).

`assets/` and the bundled 7-Zip binary use the same fixed upward walk but **do not** consult `GRID_LAUNCHER_SHARE_DIR` (grid_launcher/ui/theme.py:50, grid_launcher/library/archive_preparation.py:24, grid_launcher/library/cloud_transfer.py:34).

A missing or unparsable `emulator-autoprofiles.json` yields an empty profile list rather than an error (grid_launcher/emulator/profiles.py:416-424).

### Config load

Entry point runs once at window construction (grid-launcher.py:299) and again when returning from TV mode (grid-launcher.py:764). Steps (grid-launcher.py:2514-2575):

1. Build the defaults object (grid-launcher.py:2515, grid-launcher.py:2329-2360).
2. If `config.json` does not exist, return the defaults unchanged (grid-launcher.py:2518-2519).
3. Read and parse the file as UTF-8 JSON. On a parse error or any OS error, return the defaults unchanged — the corrupt file is left on disk, not deleted or backed up (grid-launcher.py:2521-2524).
4. Merge against defaults (see next section).
5. Normalize and reinstate `emulator_source_installs` from the raw parsed content, because the generic merge does not handle it (grid-launcher.py:2535-2537).
6. Load the API token from secret storage. If found, it overwrites the merged value. If not found and the merged config still carries a non-blank plaintext `api_token` (an old config file), attempt to save it to secret storage; on success keep the trimmed value in memory and flag it for migration (grid-launcher.py:2539-2547).
7. Load the RetroAchievements token; if found, it overwrites the merged value. There is no plaintext migration path for this one (grid-launcher.py:2549-2551).
8. Load the RetroAchievements API key, with the same find-or-migrate logic as the API token (grid-launcher.py:2553-2565).
9. If either migration happened, write a copy of the merged config back to disk with the migrated fields blanked, so the plaintext leaves the file (grid-launcher.py:2567-2573).
10. Return the merged config.

### Merge with defaults

`merge_config_with_defaults` (grid_launcher/core/config.py:215-250):

- If the parsed content is not an object, return a shallow copy of the defaults (grid_launcher/core/config.py:225-226).
- Start from a shallow copy of the defaults, then iterate over **the defaults' keys only**. Any key present in the file but absent from the defaults is silently dropped (grid_launcher/core/config.py:228-229). Verified: an input key `unknown_key` does not appear in the output.
- Per key, in this order (first match wins):
  1. default is a string and the value is a string -> take the value verbatim, no trimming (grid_launcher/core/config.py:231-232).
  2. default is a boolean and the value is a boolean -> take the value (grid_launcher/core/config.py:233-234).
  3. default is an integer (and not a boolean) and the value is an integer -> take the value. Only the DEFAULT is guarded against booleans; the reference implementation ACCEPTS a boolean value here (its boolean type is an integer subtype), so `true` in the file is stored as-is at this stage. A faithful port must reproduce that acceptance; the later runtime reader `_config_int` (grid-launcher.py:2197-2199) is where booleans are rejected (grid_launcher/core/config.py:235-236).
  4. key is `emulators` -> run the emulator normalizer on the value, whatever its type (grid_launcher/core/config.py:237-238).
  5. key is `default_emulators` -> run that normalizer (grid_launcher/core/config.py:239-240).
  6. key is `default_retroarch_cores` -> run that normalizer (grid_launcher/core/config.py:241-242).
  7. key is `installed_games` -> run that normalizer (grid_launcher/core/config.py:243-244).
  8. key is `cloud_sync_state` -> run that normalizer (grid_launcher/core/config.py:245-246).
  9. default is a list and the value is a list -> take the value verbatim, with no element validation (grid_launcher/core/config.py:247-248).
  10. no match -> keep the default. This is what happens to every object-typed key without a dedicated branch (`compat_tool_installs`, `emulator_source_installs`).
- Type mismatches (for example a number where a string is expected) fall through to the default, so a malformed value silently reverts to the factory value.

### Normalizers

**`normalize_emulators(value, save_strategy_normalizer)`** (grid_launcher/core/config.py:8-81): returns `[]` for a non-list input; skips non-object items; applies the field rules in "Emulator entry"; finally sorts the result by the lower-cased `name` (grid_launcher/core/config.py:80). No de-duplication by name occurs.

**`normalize_default_emulators(value)`** (grid_launcher/core/config.py:84-92): returns `{}` for a non-object input; keeps a pair only when the key is a non-blank string and the value is a string. The key is trimmed; **the value is not trimmed** (grid_launcher/core/config.py:90-91). An empty-string value is kept.

**`normalize_default_retroarch_cores(value)`** (grid_launcher/core/config.py:95-103): same shape, but stricter — the value must also be non-blank, and both key and value are trimmed (grid_launcher/core/config.py:101-102).

**`normalize_installed_games(value, game_key_fn)`** (grid_launcher/core/config.py:106-190): returns `[]` for a non-list input; skips non-object items; requires non-blank `title` and `platform`; builds the fixed-field object described above; then de-duplicates: compute `game_key_fn(entry)` and skip any entry whose key was already seen (grid_launcher/core/config.py:185-189). The first occurrence wins; order is otherwise preserved (no sort). The identity function is `(lower-cased trimmed title, lower-cased trimmed platform)` (grid_launcher/library/identity.py:4-5, grid-launcher.py:3223-3224, grid_launcher/ui/mixins/emulator_ui_mixin.py:632-633). Note `rom_id` is not part of the de-duplication key here even though it is the primary key elsewhere (grid_launcher/library/identity.py:15-20).

**`normalize_compat_tool_installs(value)`** (grid_launcher/core/config.py:193-212): defined and used by the emulator UI (grid_launcher/ui/mixins/emulator_ui_mixin.py:165-172), but never wired into the load-time merge.

**`normalize_cloud_sync_state(value)`** (grid_launcher/library/cloud_sync.py:8-59): as tabulated above.

### Config write

`write_config_file(config_dir, config_file, config)` (grid_launcher/core/config.py:261-266):

1. Create the config directory and all missing parents; existing directory is fine.
2. Serialize: shallow-copy the config, then force `api_token`, `retroachievements_token`, and `retroachievements_api_key` to `""` (grid_launcher/core/config.py:253-258). The in-memory object is not mutated.
3. Encode as JSON with 2-space indentation and keys sorted ascending, then write the whole file as UTF-8.

The write is a plain truncate-and-write. There is **no** temporary-file-plus-rename, no fsync, and no backup copy. A crash mid-write leaves a truncated `config.json`, which the next load treats as corrupt and replaces with defaults (grid-launcher.py:2521-2524). This differs from the sibling state files, which do write to `<name>.tmp` and then rename over the target (grid_launcher/server/discover.py:118-120, grid_launcher/server/discover.py:614-616, grid-launcher.py:2470-2473).

Callers:

- Desktop settings/library save wraps it, clears three in-memory caches first (emulator-sync-done set, sync-directory-paths cache, cloud-emulator-entry cache), and reports OS errors to the user (grid-launcher.py:3146-3161).
- TV-mode bridges call it directly with their own `~/.grid-launcher` path and swallow OS errors (grid_launcher/tv/bridge/app_backend.py:529-535, grid_launcher/tv/bridge/game_backend.py:771-776, grid_launcher/tv/bridge/game_backend.py:800-805).

### Settings collection (what a save actually persists)

`_collect_settings` builds a **fresh defaults object** and then copies in a fixed list of values, rather than mutating the loaded config (grid-launcher.py:2577-2626). Consequently, keys not explicitly carried over are reset to their defaults on every settings save. Carried over: `server_url`, `api_token`, `retroachievements_username`, `retroachievements_api_key`, `retroachievements_token`, `username`, `library_path`, `launch_args`, `debug_prints`, the three auto-cloud flags, `theme`, `emulators`, `default_emulators`, `default_retroarch_cores`, `window_geometry`, `window_state`, `first_run_completed`, `installed_games`, `tv_mode_home_view`, the two TV guide-button lists, `tv_mode_last_active`, `emulator_source_installs`, `auto_cloud_save_upload_delay_seconds`, `cloud_sync_state` (grid-launcher.py:2579-2625). Not carried over: `compat_tool_installs`, `default_compat_tool`.

### Secret storage

Service name is the constant `GRIDLauncher`; account names are `api_token`, `retroachievements_token`, `retroachievements_api_key` (grid_launcher/core/token_store.py:11-14). Storage is the OS credential store: Secret Service or KWallet on Linux, the Windows Credential Manager on Windows (the bundling step lists exactly those backends, .github/workflows/appimage-linux.yml:54-55, .github/workflows/pyinstaller-windows.yml:57).

**Write** (`_save_secret`, grid_launcher/core/token_store.py:129-156):

1. Trim the incoming value (grid_launcher/core/token_store.py:136).
2. If the trimmed value is empty: delete the keychain entry (errors ignored) and delete the legacy file if it exists (errors ignored), then report success (grid_launcher/core/token_store.py:138-141, :90-95, :121-126).
3. Otherwise try to store the trimmed value in the keychain. On success, delete any legacy file and report success (grid_launcher/core/token_store.py:143-145).
4. On keychain failure and only on Windows: create the config directory, DPAPI-protect the UTF-8 bytes, write the blob to the legacy file, and report success. If that raises an OS error, report failure (grid_launcher/core/token_store.py:147-154).
5. On keychain failure on any other platform: report failure without writing anything. The launcher deliberately refuses to store the secret unencrypted (grid_launcher/core/token_store.py:130-134, :156).

Any keychain exception is treated as "backend unavailable" rather than propagated (grid_launcher/core/token_store.py:81-87).

**Read** (`_load_secret`, grid_launcher/core/token_store.py:159-172):

1. Query the keychain. Any exception, or a missing entry, yields "not found" (grid_launcher/core/token_store.py:73-78).
2. If a non-empty value came back, return it. The legacy file is not touched (grid_launcher/core/token_store.py:161-163).
3. Otherwise decode the legacy file (grid_launcher/core/token_store.py:98-118):
   - file missing -> `""`;
   - read error -> `""`;
   - empty file -> `""`;
   - on Windows, DPAPI-unprotect the bytes; on other platforms, strict base64-decode them (rejecting non-alphabet characters);
   - decode the result as UTF-8;
   - any OS, value, or decoding error -> `""`.
4. If the legacy value is empty, return `""` (grid_launcher/core/token_store.py:165-167).
5. Otherwise attempt to move it into the keychain. If that succeeds, delete the legacy file. If it fails, keep the file for the next attempt (grid_launcher/core/token_store.py:169-170).
6. Return the legacy value either way (grid_launcher/core/token_store.py:172).

The three secrets use the same code with different account names and different files: `load_api_token`/`save_api_token` (grid_launcher/core/token_store.py:175-176, :187-188), `load_ra_token`/`save_ra_token` (grid_launcher/core/token_store.py:179-180, :191-192), `load_ra_api_key`/`save_ra_api_key` (grid_launcher/core/token_store.py:183-184, :195-196).

**`set_api_token(config, token, save_token)`** (grid_launcher/core/token_store.py:199-209): trim the token, call the save callback, and update the in-memory `api_token` key **only if the save reported success**. Returns the save result. Callers surface a warning and abort the surrounding operation on failure (grid-launcher.py:2681-2686, grid-launcher.py:2648-2650).

**Windows DPAPI wrappers** (grid_launcher/core/token_store.py:17-70): both return an empty byte string for empty input (grid_launcher/core/token_store.py:22-23, :50-51). They pass a data blob of the input, request no entropy, no description, no prompt, and no flags, then copy the output blob's bytes out and free the OS-allocated buffer (grid_launcher/core/token_store.py:28-42, :56-70). A failing call raises an error with the message `Could not securely protect token` / `Could not securely unprotect token` (grid_launcher/core/token_store.py:37, :65). Protection scope is therefore the current user with default flags — a port must use the same scope or previously written files will not decrypt.

---

## Invariants and error handling

- **Secrets never reach `config.json`.** The serializer blanks all three secret keys on every write (grid_launcher/core/config.py:253-258), so even a config object holding live secrets writes a clean file.
- **Load never fails.** A missing, unreadable, or malformed config file produces the defaults (grid-launcher.py:2518-2524). Malformed sub-values produce the normalizer's fallback rather than an error.
- **The defaults object is the whitelist.** Keys absent from the defaults are dropped on every load (grid_launcher/core/config.py:229). `details_rom_id_cache` is read at runtime (grid_launcher/ui/mixins/details_view_mixin.py:353-354) but is not a default, so it can never survive a round trip.
- **`compat_tool_installs` does not survive a load.** Its default is an object with no dedicated merge branch, so the merge always restores `{}` (grid_launcher/core/config.py:229-249). Confirmed by executing the merge with a populated input: the output value is `{}`. `_collect_settings` also omits the key (grid-launcher.py:2577-2626), so a settings save writes `{}` too. `emulator_source_installs` has the same structural problem but is explicitly re-normalized by the caller (grid-launcher.py:2535-2537).
- **Unknown fields on installed games are dropped.** The TV bridge writes `last_played` onto an installed-game record before saving (grid_launcher/tv/bridge/game_backend.py:799-803), but the normalizer rebuilds each record from a fixed key list (grid_launcher/core/config.py:152-184), so `last_played` (and `id`) disappear at the next load.
- **Installed-game identity in storage is title+platform, not rom id.** Two records with the same title and platform but different rom ids collapse to the first one (grid_launcher/core/config.py:185-189, grid_launcher/library/identity.py:4-5), whereas runtime identity checks prefer rom id when both sides have one (grid_launcher/library/identity.py:15-20).
- **Emulator list ordering is deterministic**: sorted by lower-cased name after normalization (grid_launcher/core/config.py:80). Config key ordering is deterministic too: sorted ascending on write (grid_launcher/core/config.py:264).
- **A failed secret save blocks the surrounding save.** First-run setup aborts and warns (grid-launcher.py:2648-2650); settings save aborts before writing the config at all (grid-launcher.py:2681-2692).
- **Config write errors are surfaced; auxiliary write errors are not.** The desktop save path catches OS errors and shows a dialog (grid-launcher.py:3154-3161), while the TV bridges (grid_launcher/tv/bridge/app_backend.py:532-535), watchlist (grid_launcher/server/discover.py:611-618), discover cache (grid_launcher/server/discover.py:115-122), analytics (grid_launcher/server/discover.py:570-579), and discover UI state (grid-launcher.py:2468-2475) swallow every error.
- **Analytics log is size-capped, not rotated.** Once `discover_events.jsonl` exceeds 1,048,576 bytes, new events are dropped silently; the file is never trimmed (grid_launcher/server/discover.py:573-574).
- **Discover cache entries are validated on load.** Only entries that are objects containing both `data` and a numeric `timestamp` are accepted, entries older than the caller's max age are skipped, and existing in-memory entries are not overwritten (grid_launcher/server/discover.py:135-144). The desktop app loads with a 7-day max age (grid-launcher.py:501) and a 1-hour freshness TTL for serving (grid-launcher.py:500, grid_launcher/server/discover.py:65-66).
- **`path_key` never raises.** Resolution errors fall back to the unresolved expanded path (grid_launcher/core/path.py:19-20).
- **`sanitize_path_component` never returns a blank component**; it returns the caller's fallback instead (grid_launcher/core/path.py:12).

---

## Platform differences

| Concern | Windows | Linux / other |
| --- | --- | --- |
| Persistence root | `~/.grid-launcher` — no `%APPDATA%` variant (grid-launcher.py:2386-2387) | `~/.grid-launcher` — not XDG (grid-launcher.py:2386-2387) |
| Secret backend | OS credential store (.github/workflows/pyinstaller-windows.yml:57) | Secret Service or KWallet (.github/workflows/appimage-linux.yml:54-55) |
| Secret fallback when the backend fails | DPAPI-encrypted file in the config directory (grid_launcher/core/token_store.py:147-154) | None; the save is refused and reported as a failure (grid_launcher/core/token_store.py:156) |
| Legacy secret file decoding | DPAPI unprotect (grid_launcher/core/token_store.py:112-113) | Strict base64 decode (grid_launcher/core/token_store.py:114-115) |
| Bundled-data directory | Bundle root via the fixed upward walk (.github/workflows/pyinstaller-windows.yml:57) | Bundle root, or `/app/share/grid-launcher` under Flatpak via `GRID_LAUNCHER_SHARE_DIR` (grid_launcher/core/path.py:47-57) |
| Compat tools | Not used for launching on Windows (grid_launcher/ui/mixins/details_view_mixin.py:1433) | `<XDG data home>/grid-launcher/compat-tools` (grid_launcher/core/path.py:60-61) |
| Loader environment for spawned binaries | No change unless a saved original exists (grid_launcher/core/process.py:19-24) | Restores or drops the bundle's library path (grid_launcher/core/process.py:19-24) |
| Path sanitizing / comparison | Same rules as elsewhere (grid_launcher/core/path.py:7-30) | Same rules, including case-insensitive comparison and Windows-illegal-character stripping (grid_launcher/core/path.py:7-30) |

---

## Concurrency

- **Config file access is unsynchronized.** There is no lock, no advisory file lock, and no atomic rename around `write_config_file` (grid_launcher/core/config.py:261-266). Three independent code paths can write `~/.grid-launcher/config.json` — the desktop save path (grid-launcher.py:3146-3161) and the two TV-mode bridges (grid_launcher/tv/bridge/app_backend.py:529-535, grid_launcher/tv/bridge/game_backend.py:771-776) — each from its own in-memory copy. A last-writer-wins race is possible; a port that adds atomic replace here would strictly improve durability without changing observable read behavior.
- **Two in-memory config objects exist across a mode switch.** Entering TV mode hands the bridges a config object; leaving TV mode re-reads the file from disk and rebuilds the library list from it (grid-launcher.py:764-765). Any desktop-side change made while TV mode was active but not yet written is lost.
- **Secret operations are unsynchronized** but effectively serialized by the credential-store backend; the code holds no lock (grid_launcher/core/token_store.py:73-95).
- **The discover cache is the only structure with an explicit lock**, guarding its section map because worker threads read and write it (grid_launcher/server/discover.py:23-24, grid_launcher/server/discover.py:60-68). Its disk writes use a temporary file plus rename, so readers never observe a partial file (grid_launcher/server/discover.py:118-120).
- **The watchlist and discover UI state also use temporary-file-plus-rename** (grid_launcher/server/discover.py:614-616, grid-launcher.py:2470-2473).
- **The analytics log is append-only**, opened per event, which keeps concurrent appends of short lines mostly intact but offers no formal guarantee (grid_launcher/server/discover.py:576-577).

---

## Test oracle

### tests/test_core_path.py

| Test | Asserted behavior |
| --- | --- |
| `test_xdg_config_home_uses_env_var` | `XDG_CONFIG_HOME=/tmp/somecfg` yields exactly that path (tests/test_core_path.py:11-13) |
| `test_xdg_config_home_falls_back_when_unset` | unset yields `~/.config` (tests/test_core_path.py:15-18) |
| `test_xdg_config_home_falls_back_when_empty` | empty string yields `~/.config` (tests/test_core_path.py:20-22) |
| `test_xdg_data_home_uses_env_var` | `XDG_DATA_HOME=/tmp/somedata` yields exactly that path (tests/test_core_path.py:24-26) |
| `test_xdg_data_home_falls_back_when_unset` | unset yields `~/.local/share` (tests/test_core_path.py:28-31) |
| `test_xdg_data_home_falls_back_when_empty` | empty string yields `~/.local/share` (tests/test_core_path.py:33-35) |
| `test_grid_launcher_share_dir_uses_env_var` | `GRID_LAUNCHER_SHARE_DIR=/app/share/grid-launcher` overrides the fallback (tests/test_core_path.py:39-43) |
| `test_grid_launcher_share_dir_falls_back_when_unset` | unset yields the supplied fallback (tests/test_core_path.py:45-48) |
| `test_grid_launcher_share_dir_falls_back_when_empty` | empty string yields the supplied fallback (tests/test_core_path.py:50-52) |
| `test_normalizes_basic_entry` | an emulator entry with `name`/`path`/`args` round-trips those three fields unchanged (tests/test_core_path.py:61-71) |

### tests/test_token_store.py

| Test | Asserted behavior |
| --- | --- |
| `test_save_api_token_success_deletes_legacy_file` | keychain success stores under account `api_token` and removes `token.bin` (tests/test_token_store.py:10-21) |
| `test_save_ra_token_success_deletes_legacy_file` | same for account `retroachievements_token` and `ra_token.bin` (tests/test_token_store.py:23-34) |
| `test_save_ra_api_key_success_deletes_legacy_file` | same for account `retroachievements_api_key` and `ra_api_key.bin` (tests/test_token_store.py:36-47) |
| `test_save_falls_back_to_dpapi_on_windows_when_keyring_fails` | on Windows, keychain failure writes the DPAPI blob to the file and reports success; the protect call receives the UTF-8 secret bytes (tests/test_token_store.py:49-64) |
| `test_save_refuses_on_non_windows_when_keyring_fails` | on Linux, keychain failure reports failure and creates no file (tests/test_token_store.py:66-77) |
| `test_save_empty_string_clears_keyring_and_legacy_file` | empty input deletes the keychain entry and the file, and reports success (tests/test_token_store.py:79-90) |
| `test_load_returns_keyring_value_directly` | a keychain hit short-circuits; the legacy decoder is never called (tests/test_token_store.py:92-102) |
| `test_load_migrates_legacy_file_on_success` | keychain miss + legacy value + successful store deletes the file and returns the legacy value (tests/test_token_store.py:104-117) |
| `test_load_keeps_legacy_file_when_migration_fails` | failed store leaves the file in place and still returns the value (tests/test_token_store.py:119-132) |
| `test_load_windows_legacy_file_uses_dpapi_unprotect` | on Windows the raw file bytes are passed to the unprotect call (tests/test_token_store.py:134-150) |
| `test_load_returns_empty_string_when_nothing_found` | no keychain entry and no file yields `""` (tests/test_token_store.py:152-158) |
| `test_load_ra_token_and_ra_api_key_use_distinct_accounts` | the two RetroAchievements secrets use separate account names (tests/test_token_store.py:160-168) |
| `test_set_api_token_calls_save_callback_and_updates_config_on_success` | on success the in-memory `api_token` is set to the trimmed value (tests/test_token_store.py:170-178) |
| `test_set_api_token_does_not_update_config_on_failure` | on failure the config key is not created at all (tests/test_token_store.py:180-188) |

### Config assertions elsewhere

| Test | Asserted behavior |
| --- | --- |
| `tests/test_ps4_install.py:121-157` | `merge_config_with_defaults` routes `installed_games` through the normalizer: title and platform are trimmed, `ps4_game_id` is trimmed and upper-cased |
| `tests/test_update_detection.py:73-85` | `server_updated_at` survives normalization verbatim |
| `tests/test_update_detection.py:87-99` | `ra_id` survives normalization verbatim |
| `tests/test_update_detection.py:101-113` | `local_path` survives normalization verbatim |
| `tests/test_ps4_content_apply.py:125` | `normalize_installed_games` is used as the fixture for PS4 content records |
| `tests/test_emulator_autoconfig_settings.py:2733-2745` | `GRID_LAUNCHER_SHARE_DIR` redirects both `retroarch-core-list.json` and `emulator-autoprofiles.json` lookups |
| `tests/test_emulator_profiles.py:936-944` | an unparsable `emulator-autoprofiles.json` yields an empty profile list instead of raising |

Run the suite with the repository's unittest discovery over `tests/`.

---

## Open questions

- `OPEN QUESTION:` Is the loss of `compat_tool_installs` on every config load and every settings save intended? The value has a dedicated normalizer (grid_launcher/core/config.py:193-212) and is read at runtime (grid_launcher/ui/mixins/emulator_ui_mixin.py:168-172), but the merge has no branch for it and `_collect_settings` never copies it (grid-launcher.py:2577-2626). A port must decide whether to reproduce the data loss or to persist the key.
- `OPEN QUESTION:` Should `details_rom_id_cache` be a persisted key? It is read from the config (grid_launcher/ui/mixins/details_view_mixin.py:353-354) but is absent from the defaults, so it is always empty after a load.
- `OPEN QUESTION:` Is `last_played`, written onto installed-game records by the TV bridge (grid_launcher/tv/bridge/game_backend.py:799), meant to persist? The installed-game normalizer discards it (grid_launcher/core/config.py:152-184).
- `OPEN QUESTION:` Should the config write be atomic? Every sibling state file uses temporary-file-plus-rename (grid_launcher/server/discover.py:118-120) while the config does not (grid_launcher/core/config.py:263-266). A port that adds atomicity would diverge from the reference implementation's crash behavior.
- `OPEN QUESTION:` Should `config.json` and the legacy `*.bin` files be created with restricted permissions? No mode is specified anywhere (grid_launcher/core/config.py:262-266, grid_launcher/core/token_store.py:149-152), so they inherit the process umask.
- `OPEN QUESTION:` Is the case-insensitive behavior of `path_key` on case-sensitive filesystems intended? Two distinct files differing only in case collapse to one key (grid_launcher/core/path.py:18).
- `OPEN QUESTION:` `normalize_default_emulators` keeps empty-string values and does not trim them, while `normalize_default_retroarch_cores` rejects and trims (grid_launcher/core/config.py:90-91 vs :101-102). Is the asymmetry deliberate?
- `OPEN QUESTION:` `auto_cloud_save_download_on_launch` and `auto_cloud_save_upload_on_exit` are separate schema keys but are driven by a single checkbox on save (grid-launcher.py:2600-2602). Is the download/upload split still meant to be independently settable?
- `OPEN QUESTION:` What exact DPAPI flags and entropy should a port use? The current calls pass no optional entropy, no description, and zero flags (grid_launcher/core/token_store.py:28-36), which implies current-user scope, but this is not documented anywhere in the repository.
- `OPEN QUESTION:` No code path writes the non-Windows base64 legacy secret file; only the decoder exists (grid_launcher/core/token_store.py:114-115). Which historical version produced it, and can that migration path be dropped?
- `OPEN QUESTION:` `installed_games` de-duplicates on title+platform (grid_launcher/core/config.py:185-189) while runtime identity prefers `rom_id` (grid_launcher/library/identity.py:15-20). Should storage-level de-duplication also prefer `rom_id`?

---

## Source map

| Path | Role |
| --- | --- |
| grid_launcher/core/config.py | Normalizers, defaults merge, secret-blanking serializer, config file write |
| grid_launcher/core/path.py | Path component sanitizing, path key/containment, XDG roots, bundled-data override, compat-tool directory |
| grid_launcher/core/token_store.py | Keychain accessors, DPAPI protect/unprotect, legacy file decode/delete, save/load/migrate flows, `set_api_token` |
| grid_launcher/core/process.py | Loader-environment cleanup for spawned host binaries (bundle-related) |
| grid-launcher.py:2329-2360 | Config defaults — the authoritative schema |
| grid-launcher.py:2386-2453 | Config/data file path accessors under `~/.grid-launcher` |
| grid-launcher.py:2487-2512 | Secret wiring between the window and the token store |
| grid-launcher.py:2514-2575 | Config load, merge, and plaintext-secret migration |
| grid-launcher.py:2577-2626 | Settings collection (which keys a save persists) |
| grid-launcher.py:3146-3171 | Config save wrapper, cache invalidation, config-folder opener |
| grid-launcher.py:2144-2224 | Lenient boolean/integer readers, theme choice, cloud-save flag readers |
| grid_launcher/ui/mixins/emulator_ui_mixin.py:109-198 | Normalizer wrappers, `emulator_source_installs` and `compat_tool_installs` normalization, compat-tool directory |
| grid_launcher/ui/mixins/emulator_ui_mixin.py:445-478 | Bundled-data path resolution for autoprofiles and RetroArch core lists |
| grid_launcher/ui/mixins/details_view_mixin.py:349-368 | Cloud-sync-state wrappers and cache key derivation |
| grid_launcher/library/cloud_sync.py:8-89 | Cloud sync state normalization and key format |
| grid_launcher/library/identity.py | Game identity keys used for installed-game de-duplication |
| grid_launcher/emulator/profiles.py:141-156, :409-424 | Save-strategy normalization; autoprofiles file resolution and parse failure handling |
| grid_launcher/emulator/retroarch.py:14-19 | RetroArch bundled-data file names |
| grid_launcher/server/discover.py:111-146, :561-618 | Discover cache, analytics log, and watchlist file formats |
| grid_launcher/tv/bridge/app_backend.py:529-546 | TV-mode config write and guide exclusion list normalization |
| grid_launcher/tv/bridge/game_backend.py:765-805 | TV-mode config writes for native executable path and last-played |
| grid_launcher/cover/cache.py:40-88 | Image cache file naming and write |
| tests/test_core_path.py | Path and share-directory oracle |
| tests/test_token_store.py | Secret storage oracle |
| tests/test_ps4_install.py, tests/test_update_detection.py, tests/test_ps4_content_apply.py | Config merge and installed-game normalization oracle |
| .github/workflows/pyinstaller-linux.yml, .github/workflows/pyinstaller-windows.yml, .github/workflows/appimage-linux.yml | Bundled data layout and required keychain backends |

## Rust port deviations (milestone 7)

Deliberate deviation, recorded while porting the shell's session-restore behavior for the
covers/images milestone (`docs/superpowers/specs/2026-09-02-covers-images-design.md`,
"Deviations" §D-02-a). Rust paths are relative to `rewrite/`.

- **D-02-a — Offline-first shell: with a stored server URL and credentials the main window
  renders before and regardless of the probe result; "Not connected" state with Retry replaces
  Python's status label + auto-reconnect.** Python's `server_auto_reconnect` flag maps to: probe
  on startup, never automatically again; Retry is manual. `App.svelte` calls `restore()` once on
  mount (`app/src/App.svelte:10-15`); `applyRestore` (`app/src/lib/shell.ts:12-21`) maps BOTH the
  `connected` and `unreachable` outcomes of that one probe to `phase: 'shell'` — the shell renders
  either way, with `session.connected` driving the "Not connected" chip and Retry button
  (`app/src/lib/Shell.svelte:53-56`). `retry()` (`app/src/lib/stores/session.svelte.ts:27-34`) is
  the only other place a connection is attempted; nothing calls it automatically.
