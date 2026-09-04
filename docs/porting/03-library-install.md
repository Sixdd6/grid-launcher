# 03 — Library: Download and Install Pipeline

## Purpose

This document describes how GRID Launcher downloads a game (or emulator, or platform
content pack) from the RomM server, turns the downloaded bytes into an on-disk install,
records that install in a registry, and later removes it. It is written so the same
behavior can be reimplemented in another language without reading the Python source.

Scope covered here:

- The download queue and its entry states (`grid_launcher/library/downloads.py`,
  `grid_launcher/library/install_state.py`).
- Archive extraction: format detection, the extractor fallback chain, flattening,
  merge-into-directory, launch-file selection
  (`grid_launcher/library/archive_preparation.py`).
- On-disk layout, the installed-game record, registry persistence, metadata
  hydration, uninstall (`install_paths.py`, `install_registry.py`, `install_state.py`,
  `install_metadata.py`, `install_cleanup.py`).
- Platform specials: PS3 (`ps3_install.py`), PS4 content apply, Xbox 360 (Xenia)
  content apply, firmware download/install (`firmware_install.py`).

Explicitly out of scope (separate documents): cloud saves
(`cloud_sync.py`, `cloud_transfer.py`, `cloud_restore.py`, `cloud_upload.py`) and
identity/update detection (`identity.py`, `update_detection.py`). They are referenced
here only where the install pipeline calls into them.

---

## External surfaces

### Configuration and registry file

There is exactly one persisted state file. It is the application config, and the
installed-game registry is a key inside it.

| Item | Value | Source |
| --- | --- | --- |
| Config directory | `~/.grid-launcher` | (grid-launcher.py:2386) |
| Config file | `~/.grid-launcher/config.json` | (grid-launcher.py:2392) |
| Image cache | `~/.grid-launcher/imagecache` | (grid-launcher.py:2389) |
| Server token blob | `~/.grid-launcher/token.bin` | (grid-launcher.py:2395) |
| Downloaded 7-Zip tools | `~/.grid-launcher/tools` | (grid_launcher/library/archive_preparation.py:19) |
| Registry key inside config | `installed_games` (JSON array of records) | (grid-launcher.py:3240) |

Writing the config: the whole config dict is serialized with `indent=2` and
`sort_keys=True` as UTF-8 JSON, after the directory is created with parents
(grid_launcher/core/config.py:263). Before writing, three secret fields are blanked in
the serialized copy: `api_token`, `retroachievements_token`,
`retroachievements_api_key` (grid_launcher/core/config.py:255). Persisting the registry
is: normalize the in-memory list, assign it to `config["installed_games"]`, write the
config file (grid-launcher.py:3238).

Loading merges file content over defaults key by key; the `installed_games` key is run
through the normalizer rather than copied verbatim (grid_launcher/core/config.py:243).

### Library directory layout on disk

The library root is `config["library_path"]`, expanded for `~`
(grid_launcher/ui/mixins/install_mixin.py:943). All install locations derive from it.

```
<library_path>/
  <SanitizedPlatform>/                     platform library dir  (install_mixin.py:858)
    <archive file>                         emulator-run ROM, single-file case
    <archive stem>/                        extraction dir for that archive
    <SanitizedTitle>/                      "game home dir"      (install_mixin.py:868)
      game.json                            Windows-native metadata (install_mixin.py:934)
      <archive file>                       native archive download target
      game/                                fixed native extraction dir (archive_preparation.py:846)
      prefix/                              Wine prefix, Linux native games (workers.py:583)
      Disc1.chd, Disc2.chd, game.m3u       multi-file ROM folder (install_mixin.py:1266)
  Emulators/
    <SanitizedEmulatorName>/               emulator install dir (grid_launcher/emulator/autoconfig.py:16)
      <asset file> and its extraction dir
```

Path component sanitization replaces every character in `<>:"/\|?*` and every control
character (`ord < 32`) with `_`, then converts trailing spaces and dots to `_`; if what
remains is only spaces/underscores/dots the caller's fallback string is used
(grid_launcher/core/path.py:7).

The default archive file name is the server's `rom_file_name` reduced to its last path
segment (backslashes normalized to forward slashes first); if that is empty the name is
`<safe_title>-<safe_platform>.zip` (grid_launcher/library/install_metadata.py:7).

The extraction directory for an archive is `<archive parent>/<archive stem>`. If that
path equals the archive itself, or already exists as a file, the name becomes
`<stem>_extracted` instead (grid_launcher/library/archive_preparation.py:838). Native
(Windows) games ignore the archive name entirely and always extract into
`<archive parent>/game` (grid_launcher/library/archive_preparation.py:846), selected by
the caller based on the native-platform test
(grid_launcher/ui/mixins/install_mixin.py:446).

### HTTP endpoints used by the install pipeline

| Purpose | Request | Source |
| --- | --- | --- |
| ROM detail payload (files list, cover, screenshots) | `GET /api/roms/{rom_id}` | (grid_launcher/background/workers.py:771) |
| Download ROM content | `GET {base}/api/roms/{rom_id}/content/{file_name}` | (grid_launcher/ui/mixins/install_mixin.py:1294) |
| Download a specific server file | same URL plus `?file_ids=<id>` or `?file_ids=<csv>` | (grid_launcher/ui/mixins/install_mixin.py:1276) |
| Windows `game.json` sidecar | `GET /api/roms/{rom_id}/content/game.json?file_ids=<id>` | (grid_launcher/ui/mixins/install_mixin.py:931) |
| Firmware list for a platform | `GET /api/firmware?platform_id=<id>` | (grid_launcher/library/firmware_install.py:28) |
| Firmware file bytes | `GET /api/firmware/{firmware_id}/content/{file_name}` | (grid_launcher/library/firmware_install.py:35) |

Non-RomM endpoints:

| Purpose | URL | Source |
| --- | --- | --- |
| PS3 firmware manifest | `https://fus01.ps3.update.playstation.net/update/ps3/list/us/ps3-updatelist.txt` | (grid_launcher/library/firmware_install.py:22) |
| Portable 7zr (Windows) | `https://www.7-zip.org/a/7zr.exe` | (grid_launcher/library/archive_preparation.py:20) |
| Full 7-Zip extras (Windows) | `https://www.7-zip.org/a/7z2600-extra.7z` | (grid_launcher/library/archive_preparation.py:22) |
| Emulator source releases | GitHub `https://api.github.com/repos/{owner}/{repo}/releases[...]`, Gitea `{base_url}/api/v1/repos/{owner}/{repo}/releases[...]`, or a `direct` page scrape | (grid_launcher/background/workers.py:192, workers.py:197, workers.py:244) |

Download transport: `GET` with a 60-second connect/read timeout, streamed in 64 KiB
chunks straight to the target file handle; a progress event is emitted at most every
0.1 s with cumulative bytes, `Content-Length` total (0 when absent) and average speed
since start (grid_launcher/background/workers.py:104-125). Cancellation is checked
before every chunk read and raises `Download cancelled by user`
(grid_launcher/background/workers.py:113).

GitHub-flavored request headers are added only when the caller did not already supply
them: `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`,
`User-Agent: grid-launcher` (grid_launcher/background/workers.py:295).

### Processes spawned

All extractor invocations use the exact argument patterns below. `stdout` is discarded and
`stderr` is captured as text. Most rows pass the "clean" environment described under
Invariants; two exceptions inherit the parent process environment unchanged: the 7zr unpack
of the 7-Zip extras archive (grid_launcher/library/archive_preparation.py:305-311) and the
`tasklist` probe (grid_launcher/library/archive_preparation.py:355-362) — both Windows-only
paths.

| Tool | Arguments | Source |
| --- | --- | --- |
| Bundled 7z (Windows) | `<assets/tools/7z/7z.exe> x <archive> -o<extracted_dir> -y` | (grid_launcher/library/archive_preparation.py:515) |
| System 7-Zip | `<resolved 7z|7za|7zz> x <archive> -o<extracted_dir> -y` | (grid_launcher/library/archive_preparation.py:423) |
| Downloaded portable 7zz (Windows) | `<~/.grid-launcher/tools/7zz.exe> x <archive> -o<extracted_dir> -y` | (grid_launcher/library/archive_preparation.py:547) |
| 7zr unpacking the 7-Zip extras archive (Windows) | `<7zr.exe> x <7z-extra.tmp> -o<~/.grid-launcher/tools> -y` | (grid_launcher/library/archive_preparation.py:305) |
| Archive size listing | `<7z exe> l -slt <archive>` with a 30 s timeout | (grid_launcher/library/archive_preparation.py:486) |
| tar extraction | `tar -xf <archive> -C <extracted_dir>` | (grid_launcher/library/archive_preparation.py:648) |
| tar listing (for total size) | `tar -tvf <archive>` | (grid_launcher/library/archive_preparation.py:1066) |
| Windows process probe | `tasklist /FO CSV /NH` with a 5 s timeout | (grid_launcher/library/archive_preparation.py:362) |

On Windows every spawned process gets creation flags `CREATE_NO_WINDOW`; on other
platforms the flag value is 0 (grid_launcher/library/archive_preparation.py:351).

Search order for a system 7-Zip executable: `PATH` lookups for `7z`, `7za`, `7zz` in
that order, then these absolute paths if they exist as files —
`/usr/bin/7z`, `/usr/bin/7za`, `/usr/bin/7zz`, `/usr/lib/p7zip/7za`,
`/opt/homebrew/bin/7z`, `/usr/local/bin/7z`, `/usr/local/bin/7za`. Duplicates are
removed by resolved string (grid_launcher/library/archive_preparation.py:25,
archive_preparation.py:406).

---

## Data model

### Download entry

A download entry is the UI-facing record of one download/install job. Entries live in
an ordered list; the newest is appended last, and the list is reversed for display
(grid_launcher/library/downloads.py:224).

Entry id format: `"<time.time_ns()>-<current entry count>"`
(grid_launcher/ui/mixins/install_mixin.py:1836).

| Field | Type | Initial value | Meaning | Source |
| --- | --- | --- | --- | --- |
| `id` | string | generated | Unique entry id | (grid_launcher/library/downloads.py:148) |
| `game` | object | shallow copy of the game dict | The job's game payload, including `_install_mode` and other underscore-prefixed control keys | (grid_launcher/library/downloads.py:149) |
| `title` | string | `game.title` stripped, else `"Game"` | Display title | (grid_launcher/library/downloads.py:132) |
| `platform` | string | `game.platform` stripped, else `""` | Display platform | (grid_launcher/library/downloads.py:132) |
| `status` | string | as passed | One of the states below | (grid_launcher/library/downloads.py:152) |
| `downloaded_bytes` | int | 0 | Bytes fetched so far | (grid_launcher/library/downloads.py:153) |
| `total_bytes` | int | 0 | `Content-Length`, 0 if unknown | (grid_launcher/library/downloads.py:154) |
| `speed_bps` | float | 0.0 | Average download speed | (grid_launcher/library/downloads.py:155) |
| `install_processed_bytes` | int | 0 | Bytes written during extraction | (grid_launcher/library/downloads.py:156) |
| `install_total_bytes` | int | 0 | Expected uncompressed size | (grid_launcher/library/downloads.py:157) |
| `error` | string | stripped error text | Failure detail | (grid_launcher/library/downloads.py:158) |

Progress mutators clamp: `downloaded_bytes`/`total_bytes` to `>= 0` and `speed_bps` to
`>= 0.0` (grid_launcher/library/downloads.py:169); install progress likewise
(grid_launcher/library/downloads.py:180). Setting status also stores the stripped error
and zeroes `speed_bps` when the new status is `completed`, `failed` or `cancelled`
(grid_launcher/library/downloads.py:162).

Display text per status (grid_launcher/library/downloads.py:34):

| Status | Detail text |
| --- | --- |
| `queued` | `Queued` |
| `downloading`, total > 0 | `Downloading <pct>% • <done> / <total> • <speed>/s` |
| `downloading`, total = 0 | `Downloading • <done> • <speed>/s` |
| `installing`, total > 0 | `Installing <pct>% • <done> / <total>` |
| `installing`, total = 0 | `Installing...` |
| `cancelling` | `Cancelling...` |
| `completed` | `Completed • <size>` or `Completed • Unknown size` when 0 bytes |
| `failed` | `Failed • <error or "Unknown error">` |
| `cancelled` | `Cancelled` |
| anything else | capitalized status, else `Unknown` |

Size formatting: units `B, KB, MB, GB, TB`, divide by 1024 while `>= 1024` and not at
the last unit; 0 decimals for bytes, 1 decimal otherwise; negative input clamps to 0
(grid_launcher/library/downloads.py:23). Percent is integer, clamped to `0..100`, and
0 when total `<= 0` (grid_launcher/library/downloads.py:6).

Per-entry action affordance by status (grid_launcher/library/downloads.py:214):

| Status | Action mode |
| --- | --- |
| `queued`, `downloading`, `cancelling` | `cancel` |
| `installing` | `installing` (no action) |
| `failed`, `cancelled` | `retry-dismiss` |
| everything else (`completed`) | `dismiss` |

### Installed-game record

Built by `build_installed_game_record` at registration time
(grid_launcher/library/install_registry.py:14). Every value is a string; every field is
read with a "strip, and fall back to a default if not a string" helper
(grid_launcher/library/install_registry.py:7).

| Field | Source of value | Notes |
| --- | --- | --- |
| `title` | game | Identity component. (install_registry.py:27) |
| `platform` | game | Identity component. (install_registry.py:28) |
| `rating` | game, default `"N/A"` | Empty becomes `"N/A"`. (install_registry.py:23) |
| `description` | game, default `"No description available."` | (install_registry.py:24) |
| `cover_url` | caller-resolved cover URL | (install_registry.py:31) |
| `cached_cover_path` | caller-cached cover file path | (install_registry.py:32) |
| `screenshot_urls` | game | Newline-separated list. (install_registry.py:33) |
| `genres` | game | (install_registry.py:34) |
| `regions` | game | (install_registry.py:35) |
| `filesize_bytes` | game | (install_registry.py:36) |
| `rom_id` | game | Server ROM id. (install_registry.py:37) |
| `ra_id` | game | RetroAchievements id. (install_registry.py:38) |
| `server_updated_at` | game | Used by update detection (separate doc). (install_registry.py:39) |
| `rom_file_name` | game | Server-side file name. (install_registry.py:40) |
| `extracted_path` | game | The launch file (or, for PS3, an install directory). (install_registry.py:41) |
| `extracted_dir` | game | Root of the extracted install. (install_registry.py:42) |
| `archive_path` | `str(archive_path)` **only if `extracted_path` is empty**, else `""` | Prevents pointing at an archive that was deleted after extraction. (install_registry.py:21) |
| `native_executable_path` | game | Manual executable override for native games. (install_registry.py:44) |
| `native_launch_parameters` | game | (install_registry.py:45) |
| `native_compat_tool` | game | Proton/Wine tool path (Linux). (install_registry.py:46) |
| `native_wineprefix` | game | Prefix directory. (install_registry.py:47) |
| `native_game_dir` | game | The game home dir for native installs. (install_registry.py:48) |
| `multi_file_game_dir` | game | Folder holding a multi-file ROM set. (install_registry.py:49) |
| `included_dlc` | game | JSON array text from `game.json`. (install_registry.py:50) |
| `ps3_trophy_paths` | game | JSON array of installed trophy dirs. (install_registry.py:51) |
| `ps3_game_id` | game | e.g. `BLUS30336`. (install_registry.py:52) |
| `ps3_iso_path` | game | Direct-boot ISO. (install_registry.py:53) |
| `ps4_game_id` | game | 9-char title id. (install_registry.py:54) |
| `ps4_content` | game | JSON array of applied update/DLC entries. (install_registry.py:55) |
| `revision` | game | (install_registry.py:56) |
| `languages` | game | (install_registry.py:57) |
| `tags` | game | (install_registry.py:58) |
| `fanart_url` | game | (install_registry.py:59) |
| `companies` | game | (install_registry.py:60) |
| `first_release_date` | game | (install_registry.py:61) |

The persistence normalizer (`normalize_installed_games`) is the authoritative on-disk
schema. It is stricter and slightly different from the builder
(grid_launcher/core/config.py:106):

- Non-dict entries, and entries with a blank/non-string `title` or `platform`, are
  dropped (grid_launcher/core/config.py:117-124).
- All the fields above are re-stripped; `rating` defaults to `"N/A"` and `description`
  to `"No description available."` when blank (grid_launcher/core/config.py:155).
- `ps3_game_id` and `ps4_game_id` are upper-cased (grid_launcher/core/config.py:179,
  config.py:181).
- An extra field `local_path` is always written (empty when absent)
  (grid_launcher/core/config.py:184).
- **Fields the normalizer does not preserve**: `revision`, `languages`, `tags`,
  `fanart_url`, `companies`, `first_release_date` are written by the record builder but
  are not in the normalizer's output dict, so they are dropped on the next
  normalize/persist cycle (grid_launcher/core/config.py:152-182 vs
  install_registry.py:56-61).
- Deduplication: the first record wins per `(title.lower(), platform.lower())` key;
  later duplicates are discarded (grid_launcher/core/config.py:186).

Identity key used everywhere: `(title.strip().lower(), platform.strip().lower())`
(grid_launcher/library/identity.py:4). A looser match is used for "is this installed":
if both sides have a non-empty `rom_id`, compare case-folded rom ids; otherwise compare
the identity key (grid_launcher/library/identity.py:15, identity.py:23).

### Control keys on the in-flight game dict

These underscore-prefixed keys steer the pipeline and are not part of the persisted
record.

| Key | Values | Effect | Source |
| --- | --- | --- | --- |
| `_install_mode` | `base` (default), `ps4_content`, `xbox360_content`, `native_update`, `update`, `source_emulator`, `source_emulator_update`, `compat_tool` | Selects the finalize path and the completion branch | (install_mixin.py:1163, install_mixin.py:1583) |
| `_download_entry_id` | entry id | Links a queued game back to its download entry | (install_mixin.py:1366) |
| `_ps4_content_kind` | `update` / `dlc` | PS4 content kind label and metadata tag | (install_mixin.py:1585) |
| `_xenia_content_kind` | `update` / `dlc` | Xbox 360 content label | (install_mixin.py:1638) |
| `_ps4_file_ids_csv` | comma-separated ids | Adds `?file_ids=` to the download URL (also used for Xbox 360) | (install_mixin.py:1295) |
| `_archive_name_override` | file name | Overrides the computed archive name | (install_mixin.py:1227) |
| `_source_metadata` | object | Emulator source descriptor incl. `supplemental_downloads` | (install_mixin.py:1410) |
| `_compat_tool_install_dir` | path | Fallback install path for compat tools | (grid_launcher/background/workers.py:590) |

---

## Behavior

### 1. Queue lifecycle

State variables held by the window (grid-launcher.py:469-478):
`active_download_count`, `active_download_bytes`, `active_download_total`,
`active_download_speed_bps`, `active_download_entry_id`, `active_install_bytes`,
`active_install_total`, `download_entries`, plus `install_in_progress`,
`install_pending_game`, `install_finalize_in_progress`, `install_finalize_game`,
`install_finalize_entry_id`, `install_queue`.

Starting an install (grid_launcher/ui/mixins/install_mixin.py:1162):

1. Read `_install_mode`. `source_emulator`, `source_emulator_update` and `compat_tool`
   divert to the emulator-source path (install_mixin.py:1165).
2. Evaluate the blocking reason for the mode: PS4 content
   (install_mixin.py:300), Xbox 360 content (install_mixin.py:312), or the generic
   install block (install_mixin.py:1181). A non-empty reason aborts with a dialog.
3. Resolve the ROM id; abort if missing (install_mixin.py:1186).
4. Copy the game, hydrate server metadata into the copy (see §7), resolve the
   server-side ROM file name; abort if the server gives none (install_mixin.py:1194).
5. Ensure `<library>/<platform>` exists; abort if the library path is unset or the
   directory cannot be created (install_mixin.py:1209-1218).
6. Ensure a server base URL is configured (install_mixin.py:1220).
7. Choose the archive name: `rom_nested_file_name` if set, else
   `_archive_name_override`, else the computed archive name (install_mixin.py:1229).
8. Choose the download target and URL — see §2.
9. **Admission control.** If `install_in_progress` or `install_finalize_in_progress` is
   true: if this game's key equals the pending key or is already queued, do nothing and
   return `false`; otherwise create a `queued` download entry, append the game to
   `install_queue`, refresh UI, return `true` (install_mixin.py:1347-1356).
10. Otherwise: reuse the game's existing entry id (setting it to `downloading`) or
    create a new `downloading` entry; set `install_in_progress = true`,
    `install_pending_game`, `active_download_entry_id`; increment
    `active_download_count`; zero the three active-download counters
    (install_mixin.py:1358-1372).
11. Start the download worker on its own thread (install_mixin.py:1376-1395).

Download completion (install_mixin.py:1515):

1. Clear `install_in_progress`, `install_pending_game`, `active_download_entry_id`;
   decrement `active_download_count` with a floor of 0 (install_state.py:43); if it
   reached 0, zero the byte/speed counters (install_state.py:47).
2. If no pending game is known, set the entry status from the error text and start the
   next queued install (install_mixin.py:1529).
3. Error → status derived from the error string: contains `"cancel"`
   (case-insensitive) ⇒ `cancelled`, any other non-empty error ⇒ `failed`, empty ⇒
   `completed` (grid_launcher/library/install_state.py:35). A dialog is shown only when
   the status is not `cancelled`. Then start the next queued install
   (install_mixin.py:1542-1552).
4. If the mode is not one of `ps4_content`, `xbox360_content`, `update`,
   `source_emulator`, `source_emulator_update` **and** the game is already installed,
   mark the entry `completed` and skip finalize (install_mixin.py:1554).
5. For native platforms, fetch `game.json` into the game home dir now
   (install_mixin.py:1563).
6. Set the entry to `installing` and start the finalize worker
   (install_mixin.py:1567-1569).

Finalize start (install_mixin.py:1572): sets `install_finalize_in_progress`, copies the
game into `install_finalize_game`, stores `install_finalize_entry_id`, zeroes
`active_install_bytes`/`active_install_total`, maps `_install_mode` to a finalize
content kind (`ps4_content` → the PS4 kind, `xbox360_content` → `xenia_content`,
`native_update` → `native_update`, otherwise empty), and starts the worker thread.

Finalize completion (install_mixin.py:1612): clears the finalize flags and counters,
then branches by mode:

- Error or non-dict result → entry `failed` with the error text (or
  `"Failed to extract downloaded archive"`); **the downloaded archive is deliberately
  kept** so a retry does not re-download (install_mixin.py:1643-1648).
- `ps4_content` → sync `ps4_game_id`/`ps4_content` back into the stored record and the
  details view, mark `completed` (install_mixin.py:1657).
- `xbox360_content` → mark `completed` (install_mixin.py:1670).
- `native_update` → register the updated record, mark `completed`, show a success toast
  (install_mixin.py:1679).
- Base/source installs → register the record, write the RPCS3 `games.yml` entry, queue
  Xbox 360 content downloads, auto-configure the emulator, trigger firmware install for
  freshly installed source emulators, record the source install, mark `completed`
  (install_mixin.py:1690-1700).

Every branch ends by calling "start next queued install", which pops index 0 of
`install_queue` and starts it — but only when neither a download nor a finalize is in
progress and the queue is non-empty (grid_launcher/library/install_state.py:68,
grid_launcher/ui/mixins/details_view_mixin.py:336).

Cancel (install_mixin.py:1896): if the entry is the active download and a worker
exists, request cancellation and set status `cancelling` (the worker raises on its next
chunk boundary). Otherwise remove matching games from `install_queue` by
`_download_entry_id` (grid_launcher/library/install_state.py:28) and, if the queue
actually shrank, set the entry to `cancelled` with error `"Cancelled while queued"`.
There is no pause/resume: a cancelled download is discarded and its partial file
deleted (grid_launcher/background/workers.py:72, workers.py:81).

Retry (install_mixin.py:1885): allowed only for `failed` or `cancelled` entries with a
dict `game`; the game is copied, `_download_entry_id` is stripped from the copy, the old
entry is dismissed, and a new install is started
(grid_launcher/library/downloads.py:196).

Dismiss (install_mixin.py:1874): removes the entry from the list by id
(grid_launcher/library/downloads.py:192). Dismiss does not touch files or the queue.

Aggregate status text (grid_launcher/library/downloads.py:72): when a finalize is
running and there are no active downloads, `Installing 1 game` (plus
`(N queued download(s))` when queued); otherwise `N active download(s)` plus the queued
suffix. The aggregate progress bar shows download percent when downloads are active and
a total is known, an indeterminate `Downloading...` when the total is unknown,
`Installing...` (with percent when known) during finalize, `Queued` when only queued
work remains, else `0%` (grid_launcher/library/downloads.py:90).

### 2. Choosing the download target

Three shapes, decided in order (grid_launcher/ui/mixins/install_mixin.py:1233-1336):

**a. Native (Windows) game.** Create `<library>/<platform>/<SafeTitle>/`, store it as
`native_game_dir`, and set the archive path to that directory plus the archive name
(install_mixin.py:1243). If no `file_ids` were already set, fetch the ROM payload and
pick a single archive entry: skip `game.json`, skip any entry whose name contains `/`
or `\`, take the first remaining entry, then download it by its own `file_ids`
(install_mixin.py:1306-1336). The dedicated selector used by the synchronous path
prefers the first candidate whose lower-cased name ends with `.7z .zip .rar .tar .gz
.tgz .xz .zst .bz2`, and only falls back to the first top-level candidate when none
match — so extras such as soundtracks listed before the archive are not picked
(grid_launcher/library/install_metadata.py:217).

**b. Multi-file ROM.** For non-native, non-content installs, list the ROM's top-level
content entries (skipping `game.json` and any name containing a slash)
(install_mixin.py:878). If more than one remains, create `<library>/<platform>/
<SafeTitle>/`, record it as `multi_file_game_dir`, choose the launch entry (first entry
whose name ends with `.m3u`, else the first entry) (install_mixin.py:903), set
`rom_file_name` to that name, download it into the folder, and add every other entry to
`extra_downloads` — each with its own `?file_ids=` (install_mixin.py:1255-1291).

**c. Single file.** Archive path is `<library>/<platform>/<archive name>`; the URL is
the content endpoint with the server content file name. `?file_ids=` is appended from
`_ps4_file_ids_csv` when present, otherwise from `rom_base_file_id` for non-content
installs (install_mixin.py:1293-1305).

Emulator-source installs bypass all of this: the install directory is
`<library>/Emulators/<stem of archive name>` created before the download, and the URL is
resolved by the worker from the source metadata
(grid_launcher/ui/mixins/install_mixin.py:1444-1451,
grid_launcher/background/workers.py:88).

The worker may rewrite the target file name after resolving a source asset: an asset
whose name ends in `.appimage` replaces the whole file name; otherwise the archive's
suffix is replaced by the asset's suffix when they differ; a suffix-less asset leaves
the path alone (grid_launcher/background/workers.py:153). Supplemental downloads land
next to the primary archive as `<stem>-supplemental-<n><suffix>`, or
`<stem>-supplemental-<n>-<asset name>` for AppImages
(grid_launcher/background/workers.py:147).

### 3. Should this archive be extracted?

Evaluated before any extraction work (grid_launcher/library/archive_preparation.py:821),
in this exact order:

1. Native (Windows) platform ⇒ **always extract**, whatever the suffix.
2. Arcade platform ⇒ **never extract** (MAME-style zips are the ROM).
3. PS3 platform ⇒ extract if the suffix is one of
   `.zip .7z .rar .tar .gz .bz2 .xz`.
4. Everything else ⇒ extract if the suffix is one of
   `.7z .zip .tar .gz .bz2 .xz`. Note `.rar` is **not** in this set, so a non-PS3
   `.rar` is left as-is. **Rust port (D1, "Rust port deviations (milestone 8)" below):** the
   PS3-only restriction is not carried into the port — RAR archives extract on every platform
   through the bundled `unrar` crate.

Platform predicates: native = platform casefolds to something starting with `windows`
(grid_launcher/emulator/selection.py:145); arcade = platform contains any of `arcade`,
`mame`, `fbneo`, `final burn` (selection.py:11); PS3 = platform is exactly
`playstation 3` or `ps3` (selection.py:18); PS4 = normalized platform equals
`playstation 4`/`ps4`, or contains the token `ps4`, or compacts to contain
`playstation4` (selection.py:24 and the local copy at archive_preparation.py:38);
Xbox 360 = contains an `xbox` token or compacts to contain `xbox360`, and also contains
`xbox360` or the token `360` (selection.py:40).

When extraction is skipped (archive_preparation.py:1129):

- A `.appimage` archive that exists is chmod'ed to `0o755`.
- A PS3 `.iso` records `ps3_iso_path = <archive path>` so RPCS3 boots it directly.
- `extracted_path`, `extracted_dir`, `ps3_game_id`, `ps3_trophy_paths`, `ps3_iso_path`,
  `ps4_game_id` were all reset to empty at the top of preparation
  (archive_preparation.py:1122-1128), so only what is explicitly set survives.

### 4. Extraction decision tree

`extract_archive_into_directory` (grid_launcher/library/archive_preparation.py:573):

1. **Wipe the target.** If `extracted_dir` exists: remove the tree if it is a directory,
   otherwise unlink it (ignoring failures). Then create it with parents
   (archive_preparation.py:579-587).
2. **Dispatch by format**, in this order:
   - Suffix is `.7z` or `.rar` (case-insensitive) ⇒ the 7-Zip fallback chain (§5).
   - Else the file passes a ZIP signature check ⇒ the built-in ZIP reader.
   - Else ⇒ external `tar`.
   Note the ZIP check is content-based, not suffix-based, so a `.tar.gz` that is really
   a zip is read as a zip (archive_preparation.py:590, archive_preparation.py:618).
3. **On any `OSError` or bad-zip error**, delete the whole extraction directory and
   re-raise (archive_preparation.py:670).
4. If the caller asked for flattening, run the flatten step (§6)
   (archive_preparation.py:674).

Progress accounting per branch:

- **7z/rar with a progress callback**: compute the expected uncompressed total, emit
  `(0, total)`, run the extraction on a background thread, and every 150 ms emit
  `(min(bytes currently on disk, total), total)`. After the thread joins, re-raise any
  captured error, then emit a final `(bytes on disk, max(total, bytes on disk))`
  (archive_preparation.py:591-615). Without a callback the extraction runs inline
  (archive_preparation.py:616).
- **ZIP**: total is the sum of `file_size` over non-directory members. Directory
  members are created with backslashes normalized to `/` and trailing `/` stripped.
  File members whose name contains a backslash are written manually at the normalized
  path (parents created first); all others use the library extractor. Progress is
  emitted after every member (archive_preparation.py:619-642).
- **tar**: total comes from parsing `tar -tvf`; the child process is polled while alive
  and progress is emitted from the on-disk byte count every 150 ms; a non-zero exit code
  raises with the captured stderr or `"Unknown extraction error"`
  (archive_preparation.py:644-669).

Expected uncompressed size (archive_preparation.py:448): for non-`.rar` archives, read
only the 7z header via the pure-Python library and sum uncompressed sizes of
non-directory entries (fast even for very large archives). If that fails, or for `.rar`,
run `7z l -slt` and sum every `Size = <n>` line. If no executable is found, return 0.
The executable for listing prefers the bundled Windows binary if it exists, then
`PATH` (`7z`, `7za`, `7zz`), then the known absolute paths
(archive_preparation.py:468-481).

tar listing sizes are parsed heuristically: split the line on whitespace, and take the
first all-digit token that is immediately followed by either an ISO date `YYYY-MM-DD` or
a three-letter month abbreviation; lines with fewer than 4 tokens yield 0
(archive_preparation.py:1048). Lines that are empty or start with `tar:` are skipped
(archive_preparation.py:1078).

Directory byte totals walk the tree and sum `st_size` of regular files, skipping entries
that raise (archive_preparation.py:1032).

### 5. The 7-Zip fallback chain

`_extract_7z_with_fallbacks` (grid_launcher/library/archive_preparation.py:509). Each
stage appends a human-readable failure line; the first success returns immediately.

| Order | Stage | Gate | Source |
| --- | --- | --- | --- |
| 1 | Bundled `assets/tools/7z/7z.exe` | Windows only (`os.name == "nt"`) **and** the file exists | (archive_preparation.py:512, archive_preparation.py:504) |
| 2 | System 7-Zip: every candidate from `PATH` and the known-paths list, tried in order until one succeeds | all platforms | (archive_preparation.py:522, archive_preparation.py:400) |
| 3 | Pure-Python `py7zr` | all platforms; reports `"not installed"` when the import fails | (archive_preparation.py:528, archive_preparation.py:432) |
| 4 | Downloaded portable 7-Zip (`7zz.exe`) | Windows only | (archive_preparation.py:539) |

Details that matter for a port:

- Stage 2 returns "no failures" on the first executable that exits 0. If no candidate
  executable was found at all it returns a single marker failure
  `"no 7-Zip executable found on this system"`, which is later used to pick the error
  wording (archive_preparation.py:416, archive_preparation.py:526).
- Stage 3 distinguishes "library missing" from "library failed": the missing case is
  reported as `python fallback (py7zr): not installed`; a real failure is reported with
  the exception text plus the note that py7zr does not support all 7z compression
  methods, e.g. BCJ2 (archive_preparation.py:531-538).
- Stage 4 is the only stage that **wipes and recreates** the extraction directory before
  running, because at that point a known-good extractor is in hand
  (archive_preparation.py:544). Stages 1–3 leave whatever partial output exists in place;
  the test suite pins this (see Test oracle).
- If everything fails, the raised error text differs by whether a system 7-Zip was found:
  "the installed 7-Zip failed to extract it" versus "no working 7-Zip installation is
  available" followed by per-distro install hints for Debian/Ubuntu, Fedora/RHEL, Arch,
  macOS and Windows (archive_preparation.py:554-570).

Obtaining the portable tools (Windows only, both return nothing on other platforms):

- `7zr.exe`: return the cached copy if present; otherwise create the tools directory,
  download the URL with a 30 s timeout into `7zr.tmp`, and atomically replace. Any
  exception deletes the temp file and yields nothing
  (archive_preparation.py:270).
- `7zz.exe`: return the cached copy if present; otherwise ensure `7zr.exe`, download
  the extras archive with a 60 s timeout into `7z-extra.tmp`, unpack it with `7zr`, and
  if `7zz.exe` did not land at the tools root, move it up from the `x64/` subdirectory.
  Then delete the `x64/` directory and these leftovers: `7za.exe`, `7zS.sfx`,
  `7zSD.sfx`, `readme.txt`, `History.txt`, `License.txt`, `7-ZipFar.dll`, `7zS2.sfx`,
  `7zS2con.sfx`. The temp archive is always deleted. If `7zz.exe` still does not exist,
  yield nothing (archive_preparation.py:290-348).

### 6. Flattening a single nested directory

`_flatten_single_subdirectory` (grid_launcher/library/archive_preparation.py:678):

1. List the extraction directory's immediate children; on error, do nothing.
2. If there is not exactly one child, or the single child is not a directory, do nothing.
3. Move every immediate child of that nested directory up into the extraction directory
   (preserving each item's own name, so deeper structure is untouched), then remove the
   now-empty nested directory.
4. Any `OSError` during the move aborts the operation, leaving whatever was already
   moved in place.

Flattening only runs when the caller passes `flatten_single_subdir = True`
(archive_preparation.py:674).
`OPEN QUESTION:` no caller in the package passes that flag — every call site uses the
default `False` — so flattening appears to be reachable only from tests. Confirm whether
a port needs it wired into a real path.

### 7. Metadata hydration and sync

`hydrate_install_game_metadata` (grid_launcher/library/install_metadata.py:35) runs
before download. With a blank ROM id it does nothing. It scans every cached server
platform list for a game whose case-folded rom id matches, or whose identity key
matches, and copies these fields when the target's value is blank:
`cover_url`, `screenshot_urls`, `rating`, `description`, `genres`, `regions`,
`release_year`, `filesize_bytes`, `rom_file_name`, `rom_nested_file_name`,
`rom_base_file_id`, `ps4_has_update`, `ps4_has_dlc`, `ps4_file_ids_by_category`,
`revision`, `languages`, `tags`, `fanart_url`, `companies`, `first_release_date`
(install_metadata.py:57-77). Two placeholder values count as blank for this purpose:
`rating` equal to `n/a` and `description` equal to `no description available.`
(install_metadata.py:83-90). It stops at the first matching server game.

Then it takes the cached ROM payload (fetching it if absent). If the game still has no
resolved cover URL, it takes the cover from the payload; if the payload yields
screenshots, they overwrite `screenshot_urls` joined by newlines
(install_metadata.py:98-111).

`sync_install_metadata_to_details_game` (install_metadata.py:114) copies the refreshed
values into the currently displayed details game, but only when the identity keys match.
It copies `rom_id`, `rom_file_name`, `rom_base_file_id`, then stripped `cover_url` and
`screenshot_urls`, then the display fields (`rating`, `description`, `genres`, `regions`,
`release_year`, `filesize_bytes`, `revision`, `languages`, `tags`, `fanart_url`,
`companies`, `first_release_date`), then the PS4 flags (`ps4_has_update`, `ps4_has_dlc`,
`ps4_file_ids_by_category`).

Windows `game.json` (install_metadata.py:146): parsed leniently — invalid JSON or a
non-object yields an empty result. `version` becomes `revision` (stringified, empty when
null); `year` or `release_year` is coerced through `int` then stringified, empty when it
cannot be coerced; `tags` becomes a comma-space-joined string only when it is a
non-empty list of strings; `included_dlc` becomes the JSON text of the list, else `"[]"`;
`name` is stringified. Applying it (install_metadata.py:189) fills `revision`,
`first_release_date` and `tags` only when the game's value is blank, but **always**
overwrites `included_dlc`. `name` is parsed but never written to the game.

### 8. Preparing an installed game (the finalize core)

`prepare_installed_game_without_ui`
(grid_launcher/library/archive_preparation.py:1109):

1. Copy the game; blank `extracted_path`, `extracted_dir`, `ps3_game_id`,
   `ps3_trophy_paths`, `ps3_iso_path`, `ps4_game_id`.
2. If extraction is not wanted (§3), apply the AppImage chmod / PS3-ISO special and
   return `(prepared, "")`.
3. PS3 branch: extract into the archive-derived directory, then require at least one
   extracted file — otherwise delete the directory and raise
   `"Archive extracted but no ROM file was found"`. The "extracted file" for PS3 is the
   directory itself (archive_preparation.py:1140-1151).
4. Non-PS3 branch: extract and select a launch file. If no launch file can be chosen,
   the extraction directory is deleted and the same error is raised
   (archive_preparation.py:1101-1104).
5. Extraction errors (`OSError` or bad-zip) are caught and returned as
   `(None, message)` (archive_preparation.py:1158).
6. If `cleanup_archive_on_success` is true, delete the archive; a failure becomes a
   warning `"Extracted <title>, but could not delete archive:\n<detail>"`
   (archive_preparation.py:1162). The finalize worker always passes `false` here and
   performs cleanup itself in a specific order (see §9).
7. Record `extracted_path` and `extracted_dir`. On non-Windows, chmod the launch file
   to `0o755` when it is a regular file (archive_preparation.py:1171).
8. For PS4 platforms, detect and store `ps4_game_id` (§12).
9. For PS3 platforms, run the PS3 routing described in §11.

### 9. Finalize worker ordering

`InstallFinalizeWorker.run` (grid_launcher/background/workers.py:546) picks one of four
preparation calls by content kind — `xenia_content`, `update`/`dlc` (PS4),
`native_update`, or the default preparation with `cleanup_archive_on_success=False` —
then, on success:

1. **Linux native prefix.** For a native platform on Linux with an existing
   `extracted_dir`, create `<native_game_dir>/prefix` (or `<extracted_dir>/prefix` when
   `native_game_dir` is empty) and store it as `native_wineprefix`
   (workers.py:578-587).
2. **Compat tool path.** For `_install_mode == "compat_tool"`, set
   `_compat_tool_install_path` to `extracted_dir` if set, else
   `<_compat_tool_install_dir>/<sanitized title>` (workers.py:588-602).
3. **Main archive cleanup — only if `extracted_path` is non-empty.** A direct-file
   install (nothing extracted) keeps its archive, because the archive *is* the game
   (workers.py:605).
4. **Apply supplemental archives** into the install directory (workers.py:615).
5. **Supplemental archive cleanup** (workers.py:623).
6. **Firmware install** for the game's platform; any exception becomes a warning line
   rather than a failure (workers.py:633-641).
7. Emit `{game, archive_path, warning, error}`. Every exception in the whole method is
   converted into `{game: None, ..., error: str}` (workers.py:643).

Warnings from stages are joined with a blank line between them (workers.py:614).

Supplemental application (grid_launcher/ui/mixins/install_mixin.py:739): the target
directory is `extracted_dir`, or the parent of `extracted_path`, otherwise nothing
happens. Each supplemental file `<stem>-supplemental-<n><suffix>` that exists is merged
into the target via a temp directory named
`<target parent>/.<target name>-supplemental-<n>-merge`.

Merge semantics (`merge_archive_into_directory`,
grid_launcher/library/archive_preparation.py:1236): the temp directory is removed first
if it exists, the archive is extracted into it, the tree is copied over the target, and
the temp directory is removed in a `finally` so it disappears on success and on failure.
The tree copy walks every entry: directories are created, files are copied with metadata
preserved, overwriting same-named files. Files already in the target that the archive
does not contain are left untouched (archive_preparation.py:258).

Archive cleanup (grid_launcher/ui/mixins/install_mixin.py:700) is split into a main part
and a supplemental part so the worker can order them around the merge. Supplemental
paths are recomputed from the source metadata's `supplemental_downloads` list using the
same `<stem>-supplemental-<n><suffix>` naming, where the suffix comes from the spec's
`asset_name`, else the archive's suffix, else `.zip`.

Deleting an archive (`cleanup_install_archive`,
grid_launcher/library/archive_preparation.py:194) is retry-heavy because of Windows
anti-virus locks:

- Missing or non-file ⇒ no-op.
- Up to 20 attempts, 0.25 s apart. Before each retry it waits for extractor processes to
  exit (archive_preparation.py:166).
- If all attempts fail, try to schedule deletion at next reboot (Windows
  `MoveFileExW` with `MOVEFILE_DELAY_UNTIL_REBOOT`; always false elsewhere)
  (archive_preparation.py:140).
- If that also fails, spawn a detached background thread that sleeps 5 s and then retries
  up to 60 times at 1 s intervals, silently (archive_preparation.py:175).
- The function's return value is the *warning text*, and it is `""` in every path —
  including failure — so callers never see a cleanup error string from it
  (archive_preparation.py:194-201).
  `OPEN QUESTION:` the `"could not delete archive"` warning branches in
  `prepare_installed_game_without_ui` (archive_preparation.py:1163) and
  `_cleanup_install_archives_without_ui` (install_mixin.py:714) are therefore
  unreachable today. Confirm whether a port should keep them or surface real errors.
  **RULED (milestone 8): surface real errors.** The port's `delete_with_retry` reports a
  failed delete as the warning `"could not delete archive: <path>"` on the completed entry
  instead of always returning success (restates milestone 2 deviation 3; see "Rust port
  deviations (milestone 8)" below).

The Windows process wait (archive_preparation.py:376) polls `tasklist` for `7z.exe`,
`7za.exe`, `7zz.exe`, `7zr.exe` or `tar.exe` in the output every 150 ms until they are
gone or the timeout expires; it is a no-op on other platforms.

### 10. Launch-file selection

`select_extracted_launch_file` (grid_launcher/library/archive_preparation.py:855):

1. Collect every regular file under the extraction directory, recursively. No files ⇒
   no selection.
2. Build the pool: files whose suffix is **not** in
   `.zip .7z .rar .tar .gz .bz2 .xz`. If that leaves nothing, use all files
   (archive_preparation.py:866).
3. Preferred extension list, in priority order:
   `.m3u .cue .chd .iso .xex .bin .pbp .cso .img .ccd .nrg .mdf .gdi .rvz .gcz .wbfs
   .gcm .dol .elf .nes .fds .sfc .smc .gba .gb .gbc .n64 .z64 .v64 .nds .3ds .cia .xci
   .nsp .gen .smd .md .32x .sms .gg .pce .sgx .a26 .a52 .a78 .lnx .ws .wsc .ngp .ngc
   .jag .rom` (archive_preparation.py:870-923). For PS3, `.pkg` is prepended, making it
   the single highest priority (archive_preparation.py:924).
4. PS4 short-circuit: if the platform is PS4, try the PS4 selector first and return its
   result when non-null (archive_preparation.py:990).
5. Sort key, ascending on each component (archive_preparation.py:995):
   1. `support_dir_penalty + support_ext_penalty` (0, 1 or 2). The directory penalty is 1
      when any parent path segment (relative to the extraction root, excluding the file
      name) is one of `__macosx, glcache, cache, caches, shadercache, shaders, docs, doc,
      manual, manuals, readme, licenses, license, resources`
      (archive_preparation.py:971). The extension penalty is 1 for the "support" suffix
      set: `.txt .nfo .diz .log .json .xml .ini .cfg .conf .url .pdf .html .htm .png .jpg
      .jpeg .gif .bmp .webp .svg .ico .dll .so .dylib .py .lua .js .css .db .sqlite .tmp
      .cache .sav .srm .state .states .cht .slangp .slang .glsl .vert .frag`
      (archive_preparation.py:927).
   2. Rank in the preferred-extension list; unlisted extensions get
      `len(list) + 10`.
   3. 0 when the file stem case-folds equal to the archive stem, else 1.
   4. Number of relative path segments (shallower wins).
   5. Full path, case-folded (deterministic tie-break).
6. Selection order:
   - If any pool file has a preferred extension, sort those and return the first.
   - Otherwise, narrow to files with a zero penalty; if none, keep the whole pool.
   - Within that narrowed set, if any stem matches the archive stem, sort those and
     return the first.
   - Otherwise sort the narrowed set and return the first
     (archive_preparation.py:1015-1029).

### 11. PS3 install

Extraction produces a staging directory that is then *classified* and *routed* into the
RPCS3 virtual filesystem; the staging directory is deleted afterwards.

**ISO-only short circuit** (archive_preparation.py:1186): if the staging directory
contains exactly one classified entry and it is an ISO file
(grid_launcher/library/ps3_install.py:146), the ISO is moved next to the archive
(overwriting an existing file of that name), `extracted_path` and `ps3_iso_path` are set
to it, `extracted_dir` is cleared, the staging directory is deleted, and preparation
returns. RPCS3 boots such an ISO directly.

**Required roots** (archive_preparation.py:1199): `dev_hdd0` must resolve or the install
fails with `"No PS3 VFS dev_hdd0 path configured for <title>"`. `games_root` and
`rpcs3_data_root` are optional. In practice the install mixin passes `dev_hdd0` and
`games_root` but **not** `rpcs3_data_root` (install_mixin.py:505-506), so config routing
falls back to `dev_hdd0.parent` (ps3_install.py:268).
`OPEN QUESTION:` is omitting `ps3_rpcs3_data_root` at that call site intended, given
`_rpcs3_data_root_for_game` exists (grid-launcher.py:3479) and is used for `games.yml`?
**RULED (milestone 8, D4): wire it in.** `ps3_roots_from_config`
(`crates/grid-core/src/library/mod.rs:2560`) always resolves the data root from the configured
PS3 emulator, so `config/` lands in the data root rather than falling back to
`dev_hdd0.parent`.

**Classification** of each top-level entry, sorted directories-first then by case-folded
name (grid_launcher/library/ps3_install.py:71):

| Class | Condition |
| --- | --- |
| `iso_file` | a file with suffix `.iso` |
| `unknown` | any other file |
| `trophy_dir` | directory named `NPWR#####` |
| `disc_game_id_dir` | directory named `AAAA#####` containing both `PS3_GAME/` and `PS3_DISC.SFB` |
| `game_id_dir` | directory named `AAAA#####` containing `PS3_GAME/`; also the fallback for an `AAAA#####` directory with neither marker |
| `bare_disc_dir` | directory named `PS3_GAME` at the top level |
| `nested_hdd0_game` | directory named `dev_hdd0` containing `game/` or `home/` |
| `config_dir` | directory named `config` |
| `unknown` | anything else |

Game-id pattern is four upper-case letters plus five digits; trophy ids are `NPWR` plus
five digits (ps3_install.py:9-10).

**Routing** (ps3_install.py:165), per entry, all copies merging into existing content
(`copytree` with `dirs_exist_ok`, ps3_install.py:282):

| Class | Destination |
| --- | --- |
| `disc_game_id_dir` | `<games_root or dev_hdd0/game>/<ID>` |
| `game_id_dir` | `<dev_hdd0>/game/<ID>` |
| `trophy_dir` | `<dev_hdd0>/home/00000001/trophy/<NPWR id>` |
| `bare_disc_dir` | `<dev_hdd0>/game/<synthetic id>/PS3_GAME`, where the synthetic id comes from `PARAM.SFO` or is `PS3_GAME_DISC` |
| `iso_file` | extracted into a temporary directory via 7-Zip and routed recursively; the temp directory is removed automatically |
| `nested_hdd0_game` | each child directory of `dev_hdd0/game/` → `<dev_hdd0>/game/<ID>`; `dev_hdd0/home/` merged into `<dev_hdd0>/home`, with each `NPWR#####` directory under `home/00000001/trophy/` also recorded as an installed path |
| `config_dir` | `<rpcs3_data_root or dev_hdd0.parent>/config` |
| `unknown` | skipped silently |

The first `disc_game_id_dir`, `game_id_dir`, `bare_disc_dir`, nested game id or ISO
result sets the game id. If nothing set it, the id is scanned out of the installed paths,
skipping `NPWR` ids (ps3_install.py:276, ps3_install.py:26). Synthesizing an id from
`PARAM.SFO` scans the raw bytes of `PS3_GAME/PARAM.SFO` or `PARAM.SFO` for the first
`AAAA#####` byte pattern (ps3_install.py:288).

After routing (archive_preparation.py:1205-1229):

- An empty game id fails the install with
  `"No PS3 game ID found in archive for <title>"`.
- `extracted_path` and `extracted_dir` are both set to the installed path whose directory
  name upper-cases to the game id, falling back to `<dev_hdd0>/game/<ID>`.
- `ps3_trophy_paths` is the JSON array of installed paths whose string contains
  `trophy` (case-insensitive).
- The staging directory is deleted.
- Any `OSError` during routing returns
  `"Failed to install PS3 game <title>: <error>"`.

Post-install, the caller writes an RPCS3 `games.yml` entry when the game is PS3, has a
game id, and both the data root and `dev_hdd0` resolve
(grid_launcher/ui/mixins/install_mixin.py:512).

The ISO helper extracts with the shared 7-Zip fallback chain and returns the temp
directory (ps3_install.py:313).

### 12. PS4 install and content apply

**Title id detection at install time** (`_detected_ps4_game_id_for_layout`,
grid_launcher/library/archive_preparation.py:95), in order: the first `AAAA#####`
segment in the launch file's path relative to the extraction root (excluding the file
name); then the first top-level directory whose name is a valid id; then walking the
launch file's parents up to the extraction root; finally the archive's stem.
Normalization strips all non-alphanumerics, upper-cases, and requires exactly four
letters plus five digits (archive_preparation.py:54).

**Launch-file selection for PS4** (`_select_ps4_launch_file`,
archive_preparation.py:61): only files named `eboot.bin` (case-insensitive) are
candidates; if none exist the generic selector takes over. Candidates sort by (a) whether
any directory segment of their relative path is one of the top-level title ids — matches
first, (b) path depth, (c) case-folded full path.

**Content apply** (`apply_ps4_content_archive_without_ui`,
archive_preparation.py:696) — the update/DLC flow:

1. Reject non-PS4 platforms:
   `"PS4 content apply is only supported for PS4 games"`.
2. Determine the expected title id from the installed record: explicit `ps4_game_id`,
   else the first valid id among the parents of `extracted_path`, else the first
   title-id root directory inside `extracted_dir` (archive_preparation.py:204). Missing
   ⇒ `"Installed PS4 game is missing a detectable title ID"`.
3. Require a non-empty `extracted_dir` that exists as a directory, and require
   `<extracted_dir>/<title id>` to exist as a directory; each has its own error message.
4. Extract the content archive into the archive-derived directory. Extraction errors are
   returned as the error string.
5. Require at least one title-id root directory inside the extracted content, else
   `"PS4 content archive must include a title-ID root folder"`.
6. Require one of those roots to upper-case-equal the expected id, else
   `"PS4 content title ID mismatch: expected <id>, archive contains <ids or 'unknown'>"`.
7. Merge that root into the installed title directory. Merge failures return
   `"Failed to merge PS4 content into installed game: <error>"`.
8. Append a metadata entry to `ps4_content` (a compact JSON array) with keys `kind`
   (lower-cased content kind, defaulting to `content`), `title_id`, `archive_name`
   (the archive's file name) and `applied_at` (Unix seconds as a string). Set
   `ps4_game_id` to the expected id (archive_preparation.py:755-765).
9. Delete the content archive with the retrying unlink; failure yields the warning
   `"Applied PS4 content, but could not delete archive:\n<path>\n<error>"`.
10. The extracted content directory is removed in a `finally`, so it is cleaned up on
    every exit path after extraction (archive_preparation.py:777).

Reading `ps4_content` tolerates junk: non-string or blank input, invalid JSON, or a
non-array all yield an empty list; non-object items are skipped; string values are
stripped and non-string values are stringified (archive_preparation.py:227).

Initiating a PS4 content install (grid_launcher/ui/mixins/details_view_mixin.py:1507):
block reasons are checked first (base game must be installed, ROM id must exist, content
must be available); when both `update` and `dlc` are available the user picks one; the
selected file ids become `_ps4_file_ids_csv`; the archive name is
`<safe title>-<kind>.zip` (install_mixin.py:321); mode is `ps4_content`.

### 13. Xbox 360 (Xenia) content apply

`apply_xenia_content_archive_without_ui` (archive_preparation.py:781):

1. Extract the archive into the archive-derived directory; any exception returns
   `([], str(exc))`.
2. Walk every regular file under the extraction directory in sorted order and hand each
   one to the STFS installer.
3. Collect successes; collect error strings. If there were errors and **no** successes,
   return `([], joined errors)`. Otherwise return the successes with the joined errors as
   a warning.
4. The extraction directory is removed in a `finally`.

The STFS installer (grid_launcher/emulator/xenia.py:36) reads the file's STFS header to
get an 8-hex-digit title id and content type, rejects non-STFS files
(`"File does not appear to be an STFS package (bad magic)"`), optionally enforces an
expected title id, and copies the file (metadata preserved) to
`<content_root>/0000000000000000/<TitleID>/<ContentType>/<file name>`, creating parents
(xenia.py:70, xenia.py:15).

The caller resolves `content_root` from the configured Xenia emulator's directory
settings and fails with `"Could not determine Xenia content directory. Is Xenia
configured?"` when it is empty (grid_launcher/ui/mixins/install_mixin.py:376-383). On
non-Windows hosts it first requires a configured Xbox 360 emulator that is available on
the current platform, with dedicated messages pointing at Xenia Edge
(install_mixin.py:364-375).

After a base Xbox 360 game finishes installing, update and DLC downloads are queued
automatically and silently: for each of `update` then `dlc` that has file ids, a copy of
the installed record is started with mode `xbox360_content`, `_xenia_content_kind`,
`_ps4_file_ids_csv` (the id list is reused for both consoles), and the archive name
`<safe title>-<kind>.zip` (grid_launcher/ui/mixins/details_view_mixin.py:1669,
grid_launcher/ui/mixins/install_mixin.py:327).

### 14. Native (Windows) game update

`prepare_native_game_update_without_ui` (archive_preparation.py:1264) merges a new
archive into an existing install rather than replacing it:

1. Copy the installed record, then overwrite from the update payload — but only with
   non-empty values — these fields: `rom_id`, `rom_file_name`, `server_updated_at`,
   `description`, `rating`, `genres`, `regions`, `filesize_bytes`, `screenshot_urls`,
   `ra_id` (archive_preparation.py:1294-1308).
   **Rust port (milestone 8):** `rom_id` is overwritten unconditionally, because
   `RomDetail.id` is a non-optional `i64` there — Python's "only non-empty" guard is
   vacuous for that field, since the update payload always carries a rom id.
2. Require a non-blank `extracted_dir`
   (`"Installed game directory not found - reinstall the game and try again."`) that
   exists as a directory (`"Installed game directory does not exist: <path>"`).
3. Merge the archive into it through a temp directory chosen by the caller. The caller
   uses `<extracted_dir parent>/<safe title>-temp` so the temp lives on the same
   filesystem, falling back to the system temp directory as
   `grid-launcher-<safe title>-temp` (grid_launcher/ui/mixins/install_mixin.py:806).
4. Re-detect the launch file. Update `extracted_path` **only when the record has no
   manual `native_executable_path`** (archive_preparation.py:1331-1336).
5. Delete the update archive with the retrying unlink; failure yields
   `"Updated <title>, but could not delete archive:\n<path>\n<error>"`.

### 15. Registering an install

`_register_installed_game` (grid_launcher/ui/mixins/install_mixin.py:124):

1. Remove any existing record with the same identity key.
2. Append the newly built record, with the cover URL resolved and the cover image cached
   to disk at this moment.
3. Normalize the whole list (dedupe, coerce, drop invalid).
4. Recompute per-game update availability.
5. Refresh the library grid.
6. Persist the config.

Update-state recomputation (install_mixin.py:141) skips emulator entries, compares each
installed record against its server counterpart, writes `update_available` as the string
`"true"`/`"false"`, and collects the keys with updates. (Update detection itself is a
separate document.)

### 16. Uninstall

`remove_game_files` (grid_launcher/library/install_cleanup.py:8) branches by platform and
returns after the first successful branch:

**PS3** (install_cleanup.py:17):
1. Delete `ps3_iso_path` if it exists as a file; failure raises
   `"Could not remove file: <path>\n<error>"`.
2. Parse `ps3_trophy_paths` as JSON (invalid ⇒ empty list) and remove each existing
   trophy directory; failure raises
   `"Could not remove PS3 trophy directory: <path>\n<error>"`.
3. Remove every existing candidate extracted directory; failure raises
   `"Could not remove folder: <path>\n<error>"`.

**Native** (install_cleanup.py:50): if `native_game_dir` exists as a directory, remove it
and stop — this takes the archive, the `game/` directory, the prefix and `game.json` with
it. Otherwise remove every existing candidate extracted directory.

**Everything else** (install_cleanup.py:69):
1. If `multi_file_game_dir` exists as a directory, remove it and stop.
2. Otherwise delete every existing candidate archive file, then remove every existing
   candidate extracted directory.

Directory removal uses an error handler that chmods the offending path writable and
retries the operation once, re-raising on a second failure
(grid_launcher/ui/mixins/install_mixin.py:1086).

`uninstall_library_games` (install_cleanup.py:96) is the transactional wrapper:

1. Empty key set, or no matching games ⇒ return the list unchanged with `changed = false`.
2. Compute the set of cached cover paths still referenced by the games that are **not**
   being removed, so shared covers survive.
3. For each matching game: remove its files; if that fails, **abort immediately** and
   return the original list with `changed = false`. Then clean its cached cover with the
   protected set; the same abort applies.
4. Filter the removed keys out of the list and report whether the length changed.

Note the abort is not a rollback: games processed before the failure have already lost
their files (install_cleanup.py:116-120).

Uninstalling a single game refreshes update state, refreshes the grid, and persists
(install_mixin.py:1145). Uninstalling an emulator finds every installed "Emulators"
platform game whose archive path, extracted path, or containing extracted directory
matches the emulator's executable path, and removes them all
(install_mixin.py:1119, grid_launcher/library/install_registry.py:65). Path comparison
uses a case-folded, non-strict resolved path (grid_launcher/core/path.py:15).

### 17. Path candidate resolution

Candidate archive paths, deduplicated by string in this order
(grid_launcher/library/install_paths.py:19):
1. `archive_path` from the record, `~`-expanded.
2. `<platform library dir>/<archive name>`.
3. `<library path>/<archive name>`.
4. `<native_game_dir>/<archive name>`.

Candidate extracted files (install_paths.py:46): the launch file selected inside
`extracted_dir` when that directory exists, then `extracted_path` when it exists as a
file.

Candidate extracted directories (install_paths.py:68): `extracted_dir`, then for each
candidate archive path both the archive-derived extraction directory and
`<native_game_dir>/<that directory's name>`.

Native install directory (install_paths.py:92): `extracted_dir` if it is a directory,
else the parent of `extracted_path` if that is a file, else the parent of the first
existing candidate archive file, else nothing.

Native executable candidates (install_paths.py:114): every regular file under the install
directory whose suffix is `.exe .bat .cmd .ps1 .sh`
(grid_launcher/emulator/launch.py:11), sorted by path-segment count then case-folded
path. The resolved executable is the record's `native_executable_path` when it exists,
is a file and is launchable; otherwise the first candidate
(install_paths.py:130).

### 18. Firmware download and install

Server firmware (`install_platform_firmware`,
grid_launcher/library/firmware_install.py:83):

1. No target directories ⇒ nothing.
2. Fetch the firmware list; a fetch exception becomes a single warning
   `"Firmware fetch failed for platform <id>: <error>"` and stops. An empty list ⇒
   nothing.
3. For each record: skip when `id` is not an integer or `file_name` is empty.
4. Route the file to target directories. A plain path accepts every file; a
   `(path, keywords)` pair accepts the file only when one of the keywords appears as a
   substring of the lower-cased file name (firmware_install.py:38). No matching target ⇒
   skip the file.
5. Decide whether a `.zip` should be stored as-is rather than extracted: only when it
   was routed through a keyword entry whose keyword list contains the file name exactly
   (case-insensitive) and `extract_zip_with_paths` is false
   (firmware_install.py:60).
6. Download the bytes; failure becomes the warning
   `"Failed to download firmware <name>: <error>"` and moves on.
7. Per target directory: create it (failure ⇒ warning), then dispatch by extension:
   - `.7z` / `.rar`: write the bytes to a temp file, extract into a temp staging
     directory with the shared extractor, and copy every regular file **flat** into the
     target, skipping `__MACOSX` members and `.DS_Store`, and skipping existing files
     when `skip_existing`. The temp file is always deleted
     (firmware_install.py:138-168).
   - `.zip`, or content that sniffs as a zip: honour the keep-as-archive decision first;
     otherwise walk the members, skipping directory entries and `__MACOSX`. With
     `extract_zip_with_paths`, backslashes are normalized to `/` and members that are
     absolute or contain `..` are skipped (path-traversal guard); parents are created.
     Without it, members are flattened to their base name
     (firmware_install.py:170-210).
   - Anything else: write the bytes to `<target>/<file name>`
     (firmware_install.py:211).
8. `skip_existing` (default true) makes every write a no-op when the destination already
   exists.
9. Return the accumulated warning list; empty means success.

PS3 firmware direct from Sony (`download_ps3_firmware_direct`,
firmware_install.py:265): file name is always `PS3UPDAT.PUP`; if every applicable target
already has it and `skip_existing`, do nothing. Resolve the URL by fetching the manifest
and taking the `CDN=` value from the first line that contains `PS3UPDAT.PUP` and does not
contain `PS3PATCH.PUP`; not found raises
`"PS3UPDAT.PUP URL not found in Sony firmware manifest"` (firmware_install.py:238).
Download with a 300 s timeout in 64 KiB chunks, reporting `(downloaded, total, speed)`
where speed is cumulative bytes over elapsed seconds. Manifest and download failures
become single warnings rather than exceptions. Write the bytes to each applicable
directory, skipping existing files.

Sony's CDN presents a certificate whose hostname does not match, so both requests use a
TLS context with hostname checking and verification disabled
(firmware_install.py:224).

Background firmware jobs:

- **RPCS3 on emulator configuration** (grid_launcher/ui/mixins/emulator_ui_mixin.py:1737):
  skipped when a `.PUP` is already present or no firmware directories resolve. It creates
  a synthetic download entry titled `"PS3 Firmware"` on platform `"PlayStation 3"`,
  increments `active_download_count`, zeroes the counters, and runs a daemon thread that
  calls the Sony direct download; if that returns warnings and a PS3 platform id was
  found, it falls back to the server firmware install, ignoring exceptions. The first
  warning becomes the entry's error. Completion decrements the active count with a floor
  of 0, resets the counters when it hits 0, and marks the entry `failed` or `completed`
  (grid_launcher/ui/mixins/install_mixin.py:1780).
- **Fresh source-emulator install** (emulator_ui_mixin.py:1841): RPCS3 is skipped (it is
  already covered above). Platform ids come from the emulator profile: all platforms with
  RetroArch cores for a RetroArch-style emulator, all platform ids for an
  `all_platforms` profile, else the platforms matching the profile's
  `platform_keywords`. A daemon thread installs firmware for each platform id, swallowing
  exceptions, then requests an emulator view refresh. This runs only on a fresh install,
  never on a source update (install_mixin.py:1695).
- **Per-game firmware during finalize** (install_mixin.py:528): resolves the platform id,
  the default emulator for the platform and its firmware directories. RetroArch installs
  additionally consult the configured core's metadata to append a firmware
  subdirectory, restrict targets to the core's firmware file list, decide
  `extract_zip_with_paths`, and derive config-file and save-file target directories
  (including RetroArch's `default` and `:\`-relative save-directory notations)
  (install_mixin.py:552-631). Cemu firmware directories are restricted to `keys.txt`
  (install_mixin.py:632). With no targets at all it returns immediately. Firmware, config
  and saves are installed as three separate calls (saves always with
  `extract_zip_with_paths=True`), each wrapped so an exception becomes the warning
  `"Firmware install error: <e>"`. Dolphin installs finish by ensuring the skip-IPL and
  GC-pad configs, ignoring failures.

Firmware target directories come from the emulator profile's `firmware_directories`,
which may be plain strings or `{path, match}` objects. Each is expanded for environment
variables and the tokens `%EMULATOR_DIR%`, `%LIBRARY_DIR%`, `%CONFIG_DIR%`, expanded for
`~`, resolved relative to the emulator directory when not absolute, and deduplicated by
case-folded path (grid_launcher/ui/mixins/cloud_mixin.py:1032).

**Rust port (milestone 8):** the three background firmware jobs above (RPCS3-on-configuration,
fresh-source-emulator-install, per-game during finalize) move outside the install queue and
run beside it, one per emulator directory at a time, instead of inline in finalize or on daemon
threads (D6; `FirmwareService`, `app/src-tauri/src/firmware_service.rs:84`). The RPCS3 job is
skipped silently, with no drawer row, when the server's PS3 platform id is unknown offline
(D17). A fourth, launch-time call to the per-game trigger (doc 04 §11) is throttled to at most
one pass per emulator directory per app process, where Python re-ran the full download before
every launch (D19; `app/src-tauri/src/firmware_service.rs:29-83`); the install-time and
fresh-emulator-install triggers are unchanged. `%LIBRARY_DIR%`/`%EMULATOR_DIR%` expand to `.`
when blank, matching Python's `str(Path())`; config- and saves-target metadata file names are
still matched verbatim (preserving the latent Python bug where firmware/config/saves matching
is inconsistent), while firmware file names are lower-cased before the keyword match, matching
Python's `file_name.lower()`.

---

## Invariants and error handling

1. **Extraction is destructive to its target.** `extract_archive_into_directory` always
   removes and recreates the destination first, so a partially extracted directory from a
   previous attempt never leaks into a new one
   (grid_launcher/library/archive_preparation.py:579).
2. **A failed extraction leaves no directory.** Any `OSError`/bad-zip inside the
   dispatch deletes the whole extraction directory before re-raising
   (archive_preparation.py:670).
3. **Dead-end fallbacks preserve partial output.** Stages 1–3 of the 7-Zip chain never
   wipe the directory; only the last-resort portable extractor does, and only because it
   is about to re-extract everything (archive_preparation.py:544).
4. **`archive_path` and `extracted_path` are mutually exclusive in the record.** The
   builder stores the archive path only when nothing was extracted
   (grid_launcher/library/install_registry.py:21).
5. **A failed finalize keeps the downloaded archive** so a retry after fixing the
   environment does not re-download (grid_launcher/ui/mixins/install_mixin.py:1647).
6. **A failed or cancelled download deletes its partial file**
   (grid_launcher/background/workers.py:72, workers.py:81).
7. **Archive deletion never fails loudly.** It retries, then schedules a reboot-time
   delete, then retries in the background; the caller always receives an empty string
   (archive_preparation.py:194).
8. **Temporary directories are removed in `finally` blocks** for merge
   (archive_preparation.py:1260), PS4 content (archive_preparation.py:777), Xenia content
   (archive_preparation.py:818) and the PS3 ISO staging area
   (grid_launcher/library/ps3_install.py:221).
9. **Preparation blanks derived fields before deriving them**, so a re-install cannot
   inherit stale `extracted_*`, `ps3_*` or `ps4_game_id` values
   (archive_preparation.py:1122).
10. **Registry writes are normalize-then-write**, so the on-disk shape is always the
    normalizer's shape and duplicate identities cannot persist
    (grid-launcher.py:3238, grid_launcher/core/config.py:186).
11. **Uninstall aborts on the first file-removal failure** and returns the list
    unchanged, so the registry never loses a record whose files are still present
    (grid_launcher/library/install_cleanup.py:117).
12. **Only one download and one finalize run at a time**; everything else waits in
    `install_queue` (grid_launcher/library/install_state.py:68).
13. **Errors are strings, not exceptions, at the pipeline boundary.** Preparation and
    content-apply functions return `(None, message)`; the finalize worker converts any
    escaping exception into an `error` field (grid_launcher/background/workers.py:643).
14. **Cancellation is detected by substring.** Any error text containing `cancel`
    (case-insensitive) is classified as a cancellation rather than a failure, and
    suppresses the error dialog (grid_launcher/library/install_state.py:35,
    grid_launcher/ui/mixins/install_mixin.py:1548).
15. **Firmware never fails an install.** All firmware problems are accumulated as
    warnings (grid_launcher/library/firmware_install.py:103,
    grid_launcher/background/workers.py:639).
16. **Warnings mentioning "could not delete archive" are suppressed in the UI** — they
    are cosmetic (grid_launcher/ui/mixins/install_mixin.py:475).
17. **Spawned extractors get a cleaned environment.** When `LD_LIBRARY_PATH_ORIG` is
    present it is restored into `LD_LIBRARY_PATH`; otherwise, in a frozen build,
    `LD_LIBRARY_PATH` is removed entirely. This stops host binaries from resolving their
    C++ runtime against the bundle's older libraries
    (grid_launcher/core/process.py:8).
18. **Zip member paths are normalized for cross-platform safety** (backslash to slash);
    firmware zip extraction additionally rejects absolute paths and `..` segments
    (archive_preparation.py:632, grid_launcher/library/firmware_install.py:189).

---

## Platform differences

| Concern | Windows | Linux | macOS |
| --- | --- | --- | --- |
| Bundled 7-Zip | `assets/tools/7z/7z.exe` is used first when present | not used (Windows binary) | not used |
| Portable 7-Zip download | `7zr.exe` then `7zz.exe` into `~/.grid-launcher/tools` | never attempted | never attempted |
| Both above gated by | `os.name == "nt"` (archive_preparation.py:504) and `sys.platform != "win32"` early-returns (archive_preparation.py:271, archive_preparation.py:291) | — | — |
| System 7-Zip search | `PATH` only in practice | `PATH` plus `/usr/bin/*`, `/usr/lib/p7zip/7za` | `PATH` plus `/opt/homebrew/bin/7z`, `/usr/local/bin/7z`, `/usr/local/bin/7za` |
| Subprocess creation flags | `CREATE_NO_WINDOW` | 0 | 0 |
| Post-extraction chmod | skipped | launch file chmod `0o755` (archive_preparation.py:1171) | same as Linux |
| AppImage chmod | n/a in practice | `0o755` when extraction is skipped (archive_preparation.py:1130) | same code path |
| Wine prefix creation | not created | `<native_game_dir or extracted_dir>/prefix` for native games (workers.py:578) | not created (`sys.platform.startswith("linux")` gate) |
| Compat tool recorded on save | not recorded | `native_compat_tool` written (details_view_mixin.py:1772) | written (any non-win32) |
| Archive delete retries | reboot-scheduled delete via `MoveFileExW` as a last resort | never scheduled (`os.name != "nt"` ⇒ false) | never scheduled |
| Extractor-process wait before retry | polls `tasklist` | no-op | no-op |
| Xbox 360 content apply | proceeds without an emulator-availability check | requires a configured, Linux-capable emulator (e.g. Xenia Edge) | same gate as Linux (non-win32 branch) |
| Emulator source asset choice | `windows_assets` specs with `x64`/`arm64` arch matching (workers.py:322, workers.py:393) | asset resolution from the generic source rules | same |

---

## Concurrency

- **One download at a time.** `install_in_progress` guards the download slot; a second
  request is queued instead of started (grid_launcher/ui/mixins/install_mixin.py:1347).
- **One finalize at a time.** `install_finalize_in_progress` guards extraction; the next
  queued install starts only when both flags are clear and the queue is non-empty
  (grid_launcher/library/install_state.py:68).
- **`active_download_count` is a counter, not a boolean**, because the background
  firmware download increments it too (grid_launcher/ui/mixins/emulator_ui_mixin.py:1762).
  It is decremented with a floor of 0 and the byte/speed counters reset only when it
  reaches 0 (grid_launcher/library/install_state.py:43, install_state.py:47).
- **Threads used.** Download runs on a dedicated thread with signal-based progress and
  completion; finalize likewise (install_mixin.py:1376, install_mixin.py:1596). Both
  threads are torn down after completion and the window's references cleared
  (install_mixin.py:1721, install_mixin.py:1726).
- **Inside extraction**, the 7z/rar branch runs the extractor on a daemon thread while
  the calling thread polls on-disk bytes every 150 ms; the error captured by the thread is
  re-raised after `join` (archive_preparation.py:596-612). The tar branch polls the child
  process the same way (archive_preparation.py:655).
- **Fire-and-forget daemon threads**: background archive deletion
  (archive_preparation.py:190), RPCS3 firmware download
  (emulator_ui_mixin.py:1785) and source-emulator firmware install
  (emulator_ui_mixin.py:1914). None of them are joined; the process may exit while they
  run.
- **UI refresh is coalesced.** Download-entry mutations schedule a page refresh through a
  120 ms timer that ignores re-entrant scheduling while active
  (grid_launcher/ui/mixins/install_mixin.py:1879, grid-launcher.py:460).
- **Progress emission is throttled to 0.1 s** on the download side
  (grid_launcher/background/workers.py:110) and 150 ms on the extraction side.
- **Cancellation is cooperative**: a flag is set on the worker and observed before the
  next chunk read (grid_launcher/background/workers.py:55, workers.py:113). There is no
  way to cancel an in-progress extraction.
- **The only shared mutable state across threads** is the extraction directory
  (writer thread vs. progress poller) and the window's counters, which are mutated only
  from signal handlers on the UI thread.

---

## Test oracle

| Test file | What it pins |
| --- | --- |
| `tests/test_archive_extraction_fallbacks.py` | The extractor subprocess receives the cleaned environment with `LD_LIBRARY_PATH` restored from `LD_LIBRARY_PATH_ORIG` (test:18); the final error text quotes the real py7zr failure (test:57); a dead-end chain does **not** wipe already-extracted files (test:66); a system 7-Zip that fails is reported as a failure, not as "not found" (test:77). Skipped entirely on Windows (test:42). |
| `tests/test_archive_flattening.py` | Flatten moves a single nested directory's contents up (test:13); does nothing for multiple top-level items (test:29), a single file (test:42), an empty directory (test:52); preserves deeper structure (test:60). |
| `tests/test_install_metadata.py` | Hydration refreshes stale screenshot URLs and copies `release_year` from the server game; sync propagates the new metadata fields to the details view; `game.json` parsing handles missing/invalid/non-object payloads and prefers `year` over `release_year`; applying fills only blank fields but always overwrites `included_dlc` and never writes `name`; native archive selection prefers a real archive over earlier extras, skips `game.json` and subfolder entries, and falls back to the first top-level file. |
| `tests/test_install_paths_native_resolver.py` | Candidate archive paths and extracted dirs include `native_game_dir` when set and omit it when not; uninstall removes `native_game_dir` wholesale, falls back to `extracted_dir`, and does not error when the directory is already gone. |
| `tests/test_ps3_install.py` | Preparation routes a `disc_game_id_dir` to the VFS games root and a `game_id_dir` to `dev_hdd0`; errors when no `dev_hdd0` is configured and when no game id is found; the id helpers skip `NPWR` ids; all nine classifications; routing for trophies, nested `dev_hdd0` game/home/exdata, combined game+trophy, `config/` with and without a data root, and unknown-only archives; `games.yml` is written for PS3 games with an id and skipped otherwise. |
| `tests/test_ps4_install.py` | PS4 platform label detection; `eboot.bin` preference under a title-id root; `ps4_game_id` set during preparation; archive cleanup retries after a transient Windows lock and falls back to scheduled deletion when the lock persists; config normalization upper-cases `ps4_game_id`. |
| `tests/test_ps4_content_apply.py` | Content merges into the existing title directory and appends `ps4_content` metadata; title-id mismatch fails; record building and config normalization preserve the content metadata. Also the extractor chain: `.7z` support, `.rar` routed through the 7-Zip fallbacks, system 7-Zip tried before the portable download, portable 7-Zip downloaded only as a last resort and reused when cached, bundled 7-Zip preferred when present, and the full `_ensure_full_7z` behavior (already-present, non-Windows, missing 7zr, `x64/` move plus leftover cleanup, extraction failure, subprocess exception, temp-file cleanup). |
| `tests/test_xbox360_install.py` | Xbox 360 platform detection accepts `Xbox 360`/`xbox360`/`Microsoft Xbox 360` and rejects original Xbox, Xbox One, PlayStation and empty; STFS header parsing (title id, content type, `LIVE`/`PIRS` magics, non-STFS and too-short rejects); title updates and DLC land under the right content path; mismatch and missing-file rejects; the content root is created on demand; archive apply installs every STFS file from a zip and reports errors for non-STFS members. |
| `tests/test_firmware_install.py` | Routing (plain vs. keyword tuple, case-insensitivity, no-match skip, mixed lists) and the Cemu/Eden/xemu/RPCS3 routing shapes; install behavior for plain files, zips (extract vs. keep-as-archive, skip-existing, `__MACOSX`), `extract_zip_with_paths` including traversal rejection and nested skip-existing, `.7z`/`.rar` staging (preserve existing, skip-existing, overwrite, skip `__MACOSX`/`.DS_Store`), multiple target dirs sharing one download, download/fetch/mkdir errors becoming warnings, invalid records skipped, and the exact API paths; Sony PS3 direct download (manifest constant, URL parsing, not-found raise, write, skip-existing, manifest/download error warnings, empty targets); Windows-style backslash members normalized on Linux; RetroArch core firmware/config/saves metadata and directory routing; and a py7zr-only fallback plus `.tar.gz` extraction through the tar path. |
| `tests/test_emulator_install_subfolder.py` | AppImages and archives download into `<library>/Emulators/<name>/`, the subfolder exists before the download starts, nothing is placed at the `Emulators` root, and supplemental archives land in the same subfolder. |
| `tests/test_native_game_update.py` | Merge overwrites archive-provided files, preserves files the archive does not contain, creates new subdirectories, and removes the temp directory on both success and failure; the update returns an error when `extracted_dir` is missing or absent from disk; a manual `native_executable_path` is preserved; server metadata fields are updated; the record builder preserves `native_executable_path`. |
| `tests/test_background_workers.py` | `InstallDownloadWorker` HTTP error detail, debug logging, and a large matrix of emulator-source asset resolution (GitHub/Gitea/direct, Windows `x64`/`arm64`, regex and exact asset names, platform overrides, platform restrictions, supplemental downloads, archive-suffix rewriting, partial-file cleanup on error). `InstallFinalizeWorker` ordering is pinned exactly: `prepare(cleanup=False)` → `cleanup(main=True, supplementals=False)` → supplementals → `cleanup(main=False, supplementals=True)`, and the main cleanup is **skipped** when nothing was extracted. Also the Linux prefix placement under `native_game_dir`, and AppImage-aware archive/supplemental path naming. |
| `tests/test_platform_gating.py` | Windows-only emulator profiles are hidden on Linux and shown on Windows; source `platforms` allowlists gate resolution; Xbox 360 content apply on Linux returns a clear error without an emulator or with a Windows-only emulator, passes with a compatible one, and skips the gate on Windows. |
| `tests/test_stage_assets.py` | The bundled 7-Zip is packaged for Windows builds and excluded from Linux builds (test:60). |

Run everything with `python -m unittest discover tests/`.

---

## Open questions

- `OPEN QUESTION:` `extract_archive_into_directory`'s `flatten_single_subdir` parameter
  is never set to `True` by any caller in `grid_launcher/`
  (grid_launcher/library/archive_preparation.py:577 vs. the call sites at
  archive_preparation.py:1095, archive_preparation.py:1142, archive_preparation.py:1258).
  Is flattening dead code, or is a caller missing?
- `OPEN QUESTION:` `cleanup_install_archive` always returns `""`
  (grid_launcher/library/archive_preparation.py:194), so the "could not delete archive"
  warning branches at archive_preparation.py:1163 and
  grid_launcher/ui/mixins/install_mixin.py:715 can never fire. Should a port surface real
  deletion failures, or keep the silent-retry behavior?
- `OPEN QUESTION:` `build_installed_game_record` writes `revision`, `languages`, `tags`,
  `fanart_url`, `companies` and `first_release_date`
  (grid_launcher/library/install_registry.py:56-61), but `normalize_installed_games` does
  not carry them (grid_launcher/core/config.py:152-182), so they are lost on the next
  persist. Intended, or a normalizer gap?
- `OPEN QUESTION:` `_prepare_installed_game_without_ui` does not pass
  `ps3_rpcs3_data_root` (grid_launcher/ui/mixins/install_mixin.py:499-509), so a PS3
  archive's `config/` directory is routed to `dev_hdd0.parent/config` instead of the
  RPCS3 data root (grid_launcher/library/ps3_install.py:268). Should the resolver
  (grid-launcher.py:3479) be wired in?
- `OPEN QUESTION:` `_xbox360_file_ids_by_category_for_game` falls back to
  `_ps4_file_ids_by_category_from_payload` when parsing a server payload
  (grid_launcher/ui/mixins/install_mixin.py:257). Is the PS4 parser really correct for
  Xbox 360 payloads, or is this a copy-paste that a port should not replicate?
  **RULED (milestone 8, D5): one parser for both.** RomM's `files[].category` is parsed by a
  single content-category parser shared by PS4 and Xbox 360 from the start, rather than a
  PS4-specific parser one console falls back to.
- `OPEN QUESTION:` `uninstall_library_games` aborts on the first failure without rolling
  back the games already deleted (grid_launcher/library/install_cleanup.py:116). Should a
  port continue past failures and report them, instead?
  **RULED (milestone 8, D11): continue and report.** `uninstall`
  (`crates/grid-core/src/library/mod.rs:1147`) and `uninstall_steps`
  (`crates/grid-core/src/library/mod.rs:2465`) run every removal step for a game even after an
  earlier one failed, and join every failure into one message per failure; the registry row
  stays when any step fails. This extends milestone 2 deviation 2, which covers only the
  batch level across multiple games, not per-step failures within one game's removal.
- `OPEN QUESTION:` cancellation is classified by searching the error string for `cancel`
  (grid_launcher/library/install_state.py:35). A server error message containing that
  word would be misclassified. Should a port use a distinct error type instead?
- `OPEN QUESTION:` `download_progress_display` converts a percent string back to an
  integer by stripping `%` (grid_launcher/library/downloads.py:107) instead of calling
  `percent_value` directly. Behaviorally equivalent, but confirm no rounding difference is
  intended.
- `OPEN QUESTION:` the PS3 firmware endpoints disable TLS hostname and certificate
  verification (grid_launcher/library/firmware_install.py:232). Is this acceptable for a
  port, or should it pin Sony's certificate?
  **RULED (milestone 8, D2): moot — the direct-from-Sony path is dropped.** The port only
  ever fetches firmware through the RomM server (`install_platform_firmware`,
  `crates/grid-core/src/firmware/mod.rs:112`); there is no direct-from-Sony request left to
  relax TLS verification for, and no TLS relaxation exists anywhere in the port.

---

## Source map

| Path | Role |
| --- | --- |
| `grid_launcher/library/downloads.py` | Download-entry record shape, status transitions, progress clamping, and all display-text formatting. |
| `grid_launcher/library/install_state.py` | Queue predicates: admission, dequeue eligibility, active-count arithmetic, error→status classification, progress normalization. |
| `grid_launcher/library/archive_preparation.py` | Extraction engine: format dispatch, 7-Zip fallback chain, portable-tool bootstrap, flattening, merge, launch-file selection, should-extract rules, PS4/Xenia content apply, native update merge, archive cleanup with retries. |
| `grid_launcher/library/install_paths.py` | Candidate archive/extracted path resolution and native executable discovery. |
| `grid_launcher/library/install_registry.py` | Installed-game record construction; emulator↔game matching; list filtering by identity key. |
| `grid_launcher/library/install_metadata.py` | Archive naming, server-metadata hydration, details-view sync, Windows `game.json` parsing, native archive-entry selection. |
| `grid_launcher/library/install_cleanup.py` | Per-platform file removal and the transactional uninstall wrapper. |
| `grid_launcher/library/ps3_install.py` | PS3 content classification, VFS routing, ISO handling, game-id detection incl. `PARAM.SFO` scan. |
| `grid_launcher/library/firmware_install.py` | Server firmware fetch/route/install and Sony direct PS3 firmware download. |
| `grid_launcher/library/identity.py` | `game_key`, rom-id key, identity matching, installed lookup. |
| `grid_launcher/library/__init__.py` | Public re-export surface for the package. |
| `grid_launcher/background/workers.py` | `InstallDownloadWorker` (streaming download, source-asset resolution, supplementals) and `InstallFinalizeWorker` (finalize ordering, prefix and compat-tool handling). |
| `grid_launcher/ui/mixins/install_mixin.py` | Orchestration: target selection, URL construction, queue admission, worker wiring, registration, uninstall entry points, per-game firmware. |
| `grid_launcher/ui/mixins/details_view_mixin.py` | Queue dequeue, PS4/Xbox 360 content install initiation, post-install content queueing. |
| `grid_launcher/ui/mixins/emulator_ui_mixin.py` | Background firmware jobs for RPCS3 and freshly installed source emulators. |
| `grid_launcher/ui/mixins/cloud_mixin.py` | `_resolved_firmware_directories` (token expansion for firmware targets). |
| `grid_launcher/core/config.py` | Registry normalization, config merge, serialization, file write. |
| `grid_launcher/core/path.py` | Path-component sanitization and case-folded path keys. |
| `grid_launcher/core/process.py` | Cleaned subprocess environment for spawned host binaries. |
| `grid_launcher/emulator/selection.py` | Platform predicates (native, arcade, PS3, PS4, Xbox 360). |
| `grid_launcher/emulator/launch.py` | Launchable-suffix sets for native games and emulators. |
| `grid_launcher/emulator/autoconfig.py` | `emulator_install_directory` (`<library>/Emulators/<name>`). |
| `grid_launcher/emulator/xenia.py` | STFS header parsing and content placement under the Xenia content root. |
| `grid-launcher.py` | Config/cache directory constants, window state fields, PS3 VFS path resolvers, registry persistence. |

---

## Rust port deviations (milestone 2)

Rust implementation diverges from the reference in six ways; each resolves an open
question from the design spec, cited in
`docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`:

1. **Typed cancellation** (Deviations §1): Cancellation is a dedicated error variant mapped to `Cancelled` status, not a substring match on "cancel".
2. **Uninstall continues past failures** (Deviations §2): Batch uninstall processes each game independently; per-game failures leave that row and files intact; all failures reported together.
3. **Archive-deletion failures are visible** (Deviations §3): Retrying delete reports failure as a warning on the completed entry, not always success silently.
4. **No registry field loss** (Deviations §4): SQLite schema persists every field (revision, languages, tags, companies, first_release_date) without normalizer drops.
5. **Traversal guard everywhere** (Deviations §5): Absolute and `..` member paths rejected in all archive formats, not only firmware zips.
6. **Flattening is not ported** (Deviations §6): `flatten_single_subdir` dead code in reference (no real caller); port omits it.
7. **Details overlay is thinner than the spec** (milestone 2): the overlay has no metadata block and no Cancel button (cancel lives in the downloads drawer); deferred to a later milestone.

## Rust port deviations (milestone 8)

Deliberate deviations, and rulings on open questions, made while porting install specials — PS3,
PS4 and Xbox 360 install and content apply, native (Windows/Proton) install and update, managed
compat-tool installs, and firmware download/routing — to Rust (grid-core's `library/mod.rs`,
`library/extract.rs`, `library/specials/` (`ps3`, `ps4`, `xenia`, `native`), `firmware/` modules,
the Tauri `app/src-tauri/src/firmware_service.rs` glue, and the `app/src/lib/` Details and
Emulators components). Rust paths are relative to `rewrite/`. D1-D9 and D11 restate the
deviations already declared by the install-specials design task
(`docs/superpowers/specs/2026-09-02-install-specials-design.md`, "Deviations" D1-D11) for
completeness; D10 is a launch-side deviation and is recorded in doc 04 instead. D12-D18 restate
the plan's Global Constraints rulings
(`docs/superpowers/plans/2026-09-03-install-specials.md`, "Global Constraints"); D19 and the
Rulings below are new to this milestone's review.

1. **D1 — RAR archives extract on every platform through the bundled `unrar` crate.** Python
   restricted RAR extraction to PS3, going through an external 7-Zip binary. `extract_rar`
   (`crates/grid-core/src/library/extract.rs:323`) has no platform gate; whichever platform's
   should-extract table (`crates/grid-core/src/library/extract.rs:65`) admits a `.rar` suffix
   routes to it.
2. **D2 — The Sony direct PS3 firmware path is dropped; server firmware only.** Python tried
   Sony's CDN first, with TLS hostname/certificate verification disabled, and fell back to the
   RomM server. `install_platform_firmware` (`crates/grid-core/src/firmware/mod.rs:112`) is the
   only firmware path; no TLS relaxation exists anywhere in the port.
3. **D3 — An ISO inside a PS3 archive requires an external 7-Zip binary; with none, that entry
   fails visibly** (Python had the same dependency but silently skipped the entry).
   `crates/grid-core/src/library/extract.rs:684` returns `"Cannot extract ISO <name>: no
   7-Zip binary found"` instead of skipping.
4. **D4 — PS3 routing receives the RPCS3 data root, so `config/` lands in the data root**
   (doc 03 §11 open question; Python omitted it at its one call site). `ps3_roots_from_config`
   (`crates/grid-core/src/library/mod.rs:2560`) always resolves the data root from the
   configured PS3 emulator.
5. **D5 — One content-category parser (RomM's `files[].category`) serves PS4 and Xbox 360**
   (doc 03 §12/13 open question about the Python Xbox 360 path falling back to the PS4 parser).
6. **D6 — Firmware jobs run beside the install queue, one per emulator directory at a time,
   never inside it** (Python ran firmware inline in `InstallFinalizeWorker` and spawned daemon
   threads from the UI mixins). `FirmwareService` (`app/src-tauri/src/firmware_service.rs:84`)
   owns all firmware triggers outside `InstallService`'s queue, serialized per emulator
   directory by its `in_flight` set.
7. **D7 — Managed compat-tool installs persist in config across restarts** (Python reset
   `compat_tool_installs` on load — doc 02/04 defect). `Config.compat_tool_installs` round-trips
   through `config.toml` with no load-time reset.
8. **D8 — Content, compat-tool and firmware jobs are typed rows in the downloads drawer with
   the kind in the title.** Python's firmware jobs were synthetic entries bolted onto the same
   download-entry shape; the port gives every job kind (`content`, `compat_tool`, `firmware`)
   its own label in the drawer.
9. **D9 — Details gains a Cancel button** (closes milestone 2 deviation 7).
   `app/src/lib/Details.svelte:188` (`handleCancel`) and the `details-cancel` button
   (`app/src/lib/Details.svelte:307`) cancel a live install for the current rom.
11. **D11 — Uninstall of a PS3 or native game continues past per-step failures and reports them
    together** (extends milestone 2 deviation 2, which covers only the batch level across
    multiple games). `uninstall` (`crates/grid-core/src/library/mod.rs:1147`) and
    `uninstall_steps` (`crates/grid-core/src/library/mod.rs:2465`) run every removal step even
    after an earlier one failed and join every failure into one message line per failure; the
    registry row stays when any step fails.
12. **D12 — Base-install candidates exclude files whose `category` is not `game`** (blank
    counts as `game`), so a PS4/Xbox 360 ROM with update/DLC files does not become a multi-file
    game. `is_download_candidate` (`crates/grid-core/src/library/mod.rs:295`) calls
    `content::is_game_category`.
13. **D13 — A native payload whose archive suffix is not extractable (e.g. a bare `.iso`)
    installs as a direct file** (`archive_path` set, no `game/` dir) instead of failing
    extraction (`crates/grid-core/src/library/extract.rs:65`,
    `crates/grid-core/src/library/mod.rs:1777-1804`).
14. **D14 — Firmware warnings from the finalize and launch triggers are logged
    (`tracing::warn`), not joined into the download entry** (they run beside the queue per D6).
    `app/src-tauri/src/firmware_service.rs:205`.
15. **D15 — The managed compat-tool root honors the data-dir override:**
    `<GRID_LAUNCHER_DATA_DIR>/compat-tools` when set, else
    `<XDG_DATA_HOME>/grid-launcher/compat-tools`. `managed_root`
    (`crates/grid-core/src/launch/compat.rs:52`); doc 04 §12 covers the compat-tool acquisition
    path that calls it.
16. **D16 — An Xbox 360 content archive is deleted after a successful apply** (Python left it
    to the generic cleanup, which a Xenia content job never reaches).
    `crates/grid-core/src/library/mod.rs:1553-1624`.
17. **D17 — The RPCS3 "PS3 Firmware" job is skipped silently when the server's PS3 platform id
    is unknown (offline); no drawer row appears.** `FirmwareService::spawn_ps3_firmware`
    (`app/src-tauri/src/firmware_service.rs:295-320`).
18. **D18 — A native launch registers a session like an emulated launch, so Stop works**
    (Python registered none). See doc 04's D18 for detail. Native update has no UI trigger this
    milestone (update detection is doc 10) — only the command and `api.ts` wrapper exist.
19. **D19 — The launch-time firmware trigger runs at most once per emulator directory per app
    process.** Python re-ran the full per-game firmware install before every launch.
    `FirmwareService` (`app/src-tauri/src/firmware_service.rs:29-83`) remembers completed
    directories and skips a repeat pass; the install-time (finalize) and
    fresh-emulator-install triggers are unchanged — each still runs once per install, as in
    Python.
20. **D20 — `Update` (`kind` `update`, added for the identity/updates milestone): plain
    re-install of an installed non-native rom; bypasses the already-installed short-circuit;
    finalizes as Base (D-10-j: no pre-clean).** `InstallMode::Update`
    (`crates/grid-core/src/library/mod.rs:176`) requires an existing installed row for the rom,
    then routes through `finalize_base` unchanged, extracting into the directory derived from the
    server's CURRENT file name — the same directory when the name is unchanged, a sibling when the
    server renamed the file — with no clean of the old extraction directory first, matching
    Python's plain replacement. The superseded directory is never removed. A native-platform row
    is rejected here; the app layer sends native updates through the merge path instead (see doc
    10 "Rust port deviations (milestone 9)" D-10-j for the full rationale).

### Rulings on open questions

Additional decisions made during execution, not individually numbered as deviations because
they resolve implementation questions the design left open rather than diverging from a stated
Python behavior:

- §9 (finalize worker ordering): the "could not delete archive" warning branches that were
  unreachable in Python (`cleanup_install_archive` always returned `""`) are reachable in the
  port — `delete_with_retry`'s failure surfaces as the warning `"could not delete archive:
  <path>"` on the completed entry (`crates/grid-core/src/library/mod.rs:1547,1653,2024,2052`;
  restates milestone 2 deviation 3).
- §11 (PS3 install): `ps3_rpcs3_data_root` is wired in — see D4 above.
- §12/13 (PS4/Xbox 360 content apply): one category parser serves both — see D5 above.
- §16 (uninstall): `uninstall_library_games` no longer aborts without rollback on the first
  failure — see D11 above.
- §18 (firmware): the PS3-direct-from-Sony TLS-relaxation question is moot — see D2 above
  (there is no direct-from-Sony request left to relax TLS for). The three background firmware
  triggers move outside the install queue — see D6 above — and the per-game trigger gains a
  fourth, launch-time call site that gates on D19 (doc 04 §11 covers that call site).
- A failed PS4/Xbox 360 content row or native-update row can be retried from the drawer
  (re-planned through the same path as the original request); an external "PS3 Firmware" row
  (D17) cannot — there is no installed-game record to re-plan against.
- The Xbox 360 "no `<kind>` files" message is Python's verbatim `"No Xbox 360 <kind> files were
  found for this title in server metadata."` (`grid_launcher/ui/mixins/details_view_mixin.py:1640`,
  matching the PS4 sibling at `details_view_mixin.py:1559`); the plan's paraphrase ("No Xbox 360
  {kind} content is available for this title") is superseded by the verbatim-strings rule (same
  rule that supersedes the umu-run message — doc 04).
