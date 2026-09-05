# 06 — Cloud saves and save states

## Purpose

This document describes how GRID Launcher synchronises emulator save files, emulator save
states, and native-game save folders with a RomM server. It covers:

- how local save/state candidates are discovered per emulator family,
- the "session window" mtime filter that narrows candidates to the files a play session
  actually touched,
- how candidates become upload jobs (raw file, grouped archive, directory archive,
  combined native archive) and which shape applies when,
- what is sent to which RomM endpoint and with which query parameters,
- how the newest server record (or newest per slot) is chosen and restored,
- the local-newer-than-server and already-have-this-record short circuits,
- server-side retention pruning after an upload,
- the desktop state machine (`cloud_sync_state`, block reasons, auto upload/download
  triggers) and how the TV bridge differs.

Out of scope (cross-references):

- HTTP request mechanics — URL joining, query encoding, auth headers, multipart body
  construction, timeouts, error mapping. See doc 01. This document only specifies the
  path, method, query parameters, multipart field names, and response semantics.
- Config file location, load/normalise/save, and secret storage. See doc 02.
  `cloud_sync_state` and the `auto_cloud_save_*` keys live in the same config document.
- Archive extraction used by the game installer, and `format_size`. See doc 03. Cloud code
  reuses exactly one thing from `grid_launcher/library/downloads.py`: `format_size`, to
  render `file_size_bytes` on a cloud record row
  (grid_launcher/ui/mixins/details_view_mixin.py:727). Cloud downloads do **not** reuse the
  installer's streaming download path; they use plain `_api_get_bytes`
  (grid_launcher/ui/mixins/cloud_mixin.py:1775).
- Emulator selection, the launch command, and `_ensure_emulator_sync_settings`. See doc 04
  and doc 05. Cloud sync calls `_ensure_emulator_sync_settings` once while resolving sync
  directories (grid_launcher/ui/mixins/cloud_mixin.py:646) but the writers belong to doc 05.

## External surfaces

### Server endpoints

| Purpose | Method + path | Query / body | Response used | Anchor |
|---|---|---|---|---|
| List saves for a ROM | `GET /api/saves` | `rom_id=<string>` | JSON array of save records | grid_launcher/ui/mixins/cloud_mixin.py:1593 |
| List states for a ROM | `GET /api/states` | `rom_id=<string>` | JSON array of state records | grid_launcher/ui/mixins/cloud_mixin.py:1651 |
| Download save content | `GET /api/saves/{id}/content` | `id` percent-encoded with `safe=""` | raw bytes | grid_launcher/ui/mixins/cloud_mixin.py:1774 |
| Fetch a single state record | `GET /api/states/{id}` | `id` percent-encoded with `safe=""` | JSON object; used only to read its download paths | grid_launcher/ui/mixins/cloud_mixin.py:1779 |
| Download state content | `GET <record download path>` | see "State content resolution" below | raw bytes | grid_launcher/ui/mixins/cloud_mixin.py:1783 |
| Upload a save | `POST /api/saves` multipart | query `rom_id`, `emulator`, `overwrite=true`, optional `slot`; parts `saveFile`, optional `screenshotFile` | ignored on success | grid_launcher/ui/mixins/cloud_mixin.py:2478, grid_launcher/ui/mixins/cloud_mixin.py:2607 |
| Upload a state | `POST /api/states` multipart | query `rom_id`, `emulator`; parts `stateFile`, optional `screenshotFile` | ignored on success | grid_launcher/ui/mixins/cloud_mixin.py:2479, grid_launcher/ui/mixins/cloud_mixin.py:2624 |
| Delete saves | `POST /api/saves/delete` | JSON `{"saves": [<int id>]}` | ignored | grid_launcher/ui/mixins/cloud_mixin.py:1745, grid_launcher/ui/mixins/details_view_mixin.py:1303 |
| Delete states | `POST /api/states/delete` | JSON `{"states": [<int id>]}` | ignored | grid_launcher/ui/mixins/details_view_mixin.py:1303 |

Contract notes checked against `openapi.json`:

- `POST /api/saves` accepts `rom_id` (integer, required), `emulator`, `slot`, `device_id`,
  `session_id`, `overwrite` (default `false`), `autocleanup`, `autocleanup_limit` as query
  parameters, and a multipart body with required `saveFile` and optional `screenshotFile`.
  GRID sends only `rom_id`, `emulator`, `overwrite`, and sometimes `slot`
  (grid_launcher/ui/mixins/cloud_mixin.py:2607). It never uses the server-side
  `autocleanup` mechanism; it prunes client-side instead (see "Retention pruning").
- `POST /api/states` accepts only `rom_id` and `emulator` as query parameters, and a
  multipart body with required `stateFile` and optional `screenshotFile`. There is no
  `slot` and no `overwrite` for states, which is why the upload code adds those parameters
  only in the `save_type == "save"` branch (grid_launcher/ui/mixins/cloud_mixin.py:2611).
- `POST /api/saves/delete` body key is `saves`; `POST /api/states/delete` body key is
  `states`; both are arrays of integers. GRID always sends exactly one id per call
  (grid_launcher/ui/mixins/cloud_mixin.py:1746, grid_launcher/ui/mixins/details_view_mixin.py:1307).
- `rom_id` is declared as an integer by the schema but GRID passes the string form it
  carries in game records (grid_launcher/ui/mixins/cloud_mixin.py:2608); query serialisation
  handles the conversion (doc 01).

### State content resolution

`GET /api/states/{id}` returns a record; the download is then attempted against each of
`download_path`, `file_path`, `full_path` in that order, skipping blanks
(grid_launcher/library/cloud_transfer.py:141). For each candidate
(grid_launcher/ui/mixins/cloud_mixin.py:1785):

- If the candidate starts with `http://` or `https://`, it is fetched directly with an
  authorised `GET`.
- Otherwise it is treated as a server-relative path, prefixed with `/` if missing, and
  fetched through the normal API client.

Both forms are normalised first with `normalize_candidate_url`, which percent-encodes the
path with `safe="/%"` and re-encodes the query string
(grid_launcher/library/cloud_transfer.py:133). Failures on one candidate fall through to
the next; if all fail the operation raises
`ValueError("State content path could not be resolved from server record.")`
(grid_launcher/ui/mixins/cloud_mixin.py:1796).

The same three-key candidate list and the same loop are used for the screenshot attached
to a state record (grid_launcher/library/cloud_transfer.py:214,
grid_launcher/ui/mixins/cloud_mixin.py:1806).

### Local paths scanned

Sync directories come from `_resolved_sync_directory_paths(emulator_entry, key)` where
`key` is `"save_paths"` or `"state_paths"`
(grid_launcher/ui/mixins/cloud_mixin.py:618). Resolution order:

1. The emulator entry's own `save_paths` / `state_paths` string, split into a list. If
   non-empty, this wins and **all** per-emulator override probing below is skipped
   (grid_launcher/ui/mixins/cloud_mixin.py:633, and every `if not configured_paths` guard).
2. Otherwise the matched autoprofile's `save_directories` / `state_directories` list
   (grid_launcher/ui/mixins/cloud_mixin.py:638).
3. Emulator-specific override paths are prepended (RetroArch prepends its configured
   `savefile_directory`/`savestate_directory` and appends the literal fallbacks
   `saves`,`savefiles` / `states`,`savestates`
   — grid_launcher/ui/mixins/cloud_mixin.py:647). The same prepend-and-dedupe block is
   repeated for Azahar (663), Dolphin (687), PCSX2 (711), RPCS3 (735, save only),
   Vita3k (750, save only), Cemu (765, save only), PICO-8 (780, save only), FBNeo (795),
   MAME (819), Eden (843, save only), Xenia (858), Redream (882), and xemu (906, save only).

Each raw path is then expanded (grid_launcher/ui/mixins/cloud_mixin.py:939):

| Token | Replacement | Anchor |
|---|---|---|
| OS environment variables | `os.path.expandvars` | grid_launcher/ui/mixins/cloud_mixin.py:940 |
| `%EMULATOR_DIR%` | parent directory of the emulator executable | grid_launcher/ui/mixins/cloud_mixin.py:942 |
| `%LIBRARY_DIR%` | `config["library_path"]`, expanded | grid_launcher/ui/mixins/cloud_mixin.py:943 |
| `%CONFIG_DIR%` | the launcher config directory | grid_launcher/ui/mixins/cloud_mixin.py:944 |
| `%DOCUMENTS%` | Shell-resolved Windows Documents folder, else `%USERPROFILE%\Documents` | grid_launcher/ui/mixins/cloud_mixin.py:936 |

Two RetroArch-only notations are handled after expansion: the literal value `default`
becomes `<emulator_dir>/saves` or `<emulator_dir>/states`
(grid_launcher/ui/mixins/cloud_mixin.py:951), and a leading `:\` or `:/` marks a path
relative to the emulator root and is stripped
(grid_launcher/ui/mixins/cloud_mixin.py:954). Otherwise relative paths resolve against the
emulator directory (grid_launcher/ui/mixins/cloud_mixin.py:961).

**Rewrite deviation — RetroArch AppImages resolve against their portable home.** When
`<AppImage>.home/.config/retroarch` exists next to the executable, the AppImage runtime sets
`$HOME` to `<AppImage>.home`, so RetroArch writes to
`<AppImage>.home/.config/retroarch/saves|states` and its `retroarch.cfg` records those as
`~/.config/retroarch/saves|states`. The rewrite therefore resolves a leading `~` in the
config's `savefile_directory`/`savestate_directory`, and the `default` sentinel and the
`:\`/`:/` notation, against that portable home instead of the real user home / the emulator
directory, and appends the portable home's `saves`/`states` after the literal
`saves`,`savefiles` / `states`,`savestates` fallbacks so a cfg missing the key still resolves
(`crates/grid-core/src/cloud/dirs.rs`, `ResolveContext::retroarch_portable_home`). Python
expanded `~` against the real home and only ever fell back to `<emulator_dir>/saves|states`,
neither of which a portable install writes to, so no save or state was ever detected for a
RetroArch AppImage. The portable home is detected by the same
`autoconfig::paths::retroarch_portable_home` helper the config writer and core resolver use
(doc 05, "Config discovery").

A resolved entry is kept only if it exists, and it may be **either a directory or a file**
(grid_launcher/ui/mixins/cloud_mixin.py:966). Results are de-duplicated case-insensitively
(grid_launcher/ui/mixins/cloud_mixin.py:969) and memoised per
`(emulator_name, emulator_path, key)` (grid_launcher/ui/mixins/cloud_mixin.py:626).

Screenshot directories use the same expansion minus `%DOCUMENTS%` and must be directories
(grid_launcher/ui/mixins/cloud_mixin.py:983). They come only from the autoprofile key
`screenshot_directories`; there is no per-entry override
(grid_launcher/ui/mixins/cloud_mixin.py:987).

Native (Windows-platform) games do not use emulator sync directories at all. Their save
locations are the PCGamingWiki-derived list plus a per-game manual list, combined as
`pcgw_paths + [p for p in manual if p not in pcgw_paths]`
(grid_launcher/ui/mixins/cloud_mixin.py:2689,
grid_launcher/ui/mixins/details_view_mixin.py:1065). Each raw entry keeps its
environment-variable form and is expanded at use time by `resolve_native_save_dir`
(grid_launcher/library/cloud_transfer.py:484). Manually browsed folders are converted back
into env-var form before storage by `normalize_manual_save_path`, which rewrites
`%APPDATA%`, `%LOCALAPPDATA%`, `%USERPROFILE%\AppData\LocalLow`,
`%USERPROFILE%\Documents`, and `%USERPROFILE%` prefixes
(grid_launcher/library/cloud_transfer.py:559).

### Always-ignored file names

`DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES = {".ds_store", "desktop.ini", "ehthumbs.db",
"thumbs.db"}` is merged into every blocked-basename set, on both the archive-writing and
the archive-extraction side (grid_launcher/library/cloud_transfer.py:19,
grid_launcher/library/cloud_transfer.py:37). Per-emulator additions come from the entry's
`ignore_files` / `ignore_extensions` or the profile's equivalents
(grid_launcher/emulator/profiles.py:361, grid_launcher/emulator/profiles.py:385), and for
PCSX2 saves the basename `_pcsx2_superblock` is added unconditionally
(grid-launcher.py:3712).

### Archive formats

Only ZIP is produced, always with `ZIP_DEFLATED`. Three layouts exist:

| Layout | Member naming | Produced by | Anchor |
|---|---|---|---|
| Directory archive | `<dirname>/<path relative to dir>` | one archive per folder target | grid_launcher/library/cloud_transfer.py:419 |
| Grouped-file archive | path relative to the common parent of the selected files, or bare filename if that fails | grouping several files into one upload | grid_launcher/library/cloud_transfer.py:328, grid_launcher/library/cloud_transfer.py:343 |
| Native multi-directory archive | `<index>/<path relative to that directory>` plus a top-level `_grid_launcher_dirs.json` manifest mapping `"<index>"` → raw (unexpanded) directory string | native Windows games | grid_launcher/library/cloud_transfer.py:469, grid_launcher/library/cloud_transfer.py:476 |

Temporary archive naming: `<sanitised title>-<local ISO-8601 seconds with ':' replaced by
'-'>.zip` in the system temp directory; on collision a millisecond suffix is appended
(grid_launcher/library/cloud_transfer.py:286).

On the read side, downloaded payloads are sniffed: if the bytes are a zip archive the
payload is extracted, otherwise it is written verbatim
(grid_launcher/library/cloud_restore.py:180, grid_launcher/library/cloud_restore.py:206).
Extraction uses the standard zip reader and falls back to an external 7-Zip binary when the
reader raises `NotImplementedError` (unsupported compression method)
(grid_launcher/library/cloud_transfer.py:271). The fallback tries the bundled
`assets/tools/7z/7z.exe` first, then `7z`, `7za`, `7zz` from `PATH`, and raises
`OSError("No 7-Zip found to extract this archive.")` if none work
(grid_launcher/library/cloud_transfer.py:34, grid_launcher/library/cloud_transfer.py:163,
grid_launcher/library/cloud_transfer.py:187).

## Data model

### Sync state entry

Stored under config key `cloud_sync_state` as a map of key → entry
(grid_launcher/ui/mixins/details_view_mixin.py:362). The key is `rom:<rom_id casefolded>`
when the game has a ROM id, otherwise `name:<title lowercased>::<platform lowercased>`;
an empty key means "not trackable" and all reads/writes become no-ops
(grid_launcher/library/cloud_sync.py:66, grid_launcher/library/identity.py:4).

Normalisation drops any key that is not a non-blank string, any value that is not a
mapping, and any field with the wrong type; an entry that ends up empty is dropped entirely
(grid_launcher/library/cloud_sync.py:8, grid_launcher/library/cloud_sync.py:56).

| Field | Type | Meaning | Written by | Anchor |
|---|---|---|---|---|
| `last_downloaded_save_id` | non-blank string | Server save id last restored for this game | successful save restore | grid_launcher/library/cloud_sync.py:20, grid_launcher/ui/mixins/cloud_mixin.py:2099 |
| `last_server_timestamp` | float | Timestamp of the newest server save record involved in that restore | successful save restore | grid_launcher/library/cloud_sync.py:24, grid_launcher/ui/mixins/cloud_mixin.py:2100 |
| `last_uploaded_local_mtime` | float | Legacy alias of `last_uploaded_save_mtime`; still written for backward compatibility | auto upload summary | grid_launcher/library/cloud_sync.py:28, grid_launcher/library/cloud_sync.py:231 |
| `last_uploaded_at` | non-blank string | ISO-8601 UTC instant of the last successful auto upload, `+00:00` rewritten to `Z` | auto upload summary | grid_launcher/library/cloud_sync.py:32, grid_launcher/ui/mixins/cloud_mixin.py:2966 |
| `last_downloaded_state_id` | non-blank string | Server state id last restored | successful state restore | grid_launcher/library/cloud_sync.py:36, grid_launcher/ui/mixins/cloud_mixin.py:2401 |
| `last_uploaded_save_mtime` | float | Newest local save mtime at the time of the last successful save upload | auto upload summary | grid_launcher/library/cloud_sync.py:40, grid_launcher/library/cloud_sync.py:230 |
| `last_uploaded_state_mtime` | float | Newest local state mtime at the time of the last successful state upload | auto upload summary | grid_launcher/library/cloud_sync.py:44, grid_launcher/library/cloud_sync.py:233 |
| `last_session_started_at` | float | Unix time the last play session for this game started | session registration | grid_launcher/library/cloud_sync.py:48, grid_launcher/ui/mixins/cloud_mixin.py:2842 |
| `last_session_ended_at` | float | Unix time the session ended; reset to `0.0` when a new session starts | session finish / registration | grid_launcher/library/cloud_sync.py:52, grid_launcher/ui/mixins/cloud_mixin.py:2843, grid_launcher/library/cloud_sync.py:150 |

Updates merge shallowly into the existing entry and the whole config document is written to
disk on every update (grid_launcher/library/cloud_sync.py:106,
grid_launcher/ui/mixins/details_view_mixin.py:384).

### Server save / state record

Fields consumed by the client (names match the RomM `SaveSchema` / `StateSchema`):

| Field | Used for | Anchor |
|---|---|---|
| `id` | identity, download URL, delete payload, tie-break in recency sort | grid_launcher/library/cloud_restore.py:65, grid_launcher/ui/mixins/cloud_mixin.py:1989 |
| `updated_at`, `created_at` | recency; `updated_at` preferred, `created_at` is the fallback | grid_launcher/library/cloud_restore.py:14 |
| `file_name` | restore target filename; slot key fallback; row title; image-sidecar filtering for states | grid_launcher/library/cloud_restore.py:199, grid_launcher/library/cloud_restore.py:138, grid_launcher/ui/mixins/cloud_mixin.py:1665 |
| `emulator` | record filtering and the "which emulator wrote this" check on restore; also carries the native-save marker | grid_launcher/library/cloud_restore.py:107, grid_launcher/ui/mixins/cloud_mixin.py:1947, grid_launcher/ui/mixins/cloud_mixin.py:2176 |
| `slot` | per-slot latest selection and retention grouping (saves only) | grid_launcher/library/cloud_restore.py:135, grid_launcher/ui/mixins/cloud_mixin.py:1706 |
| `file_size_bytes` | row subtitle | grid_launcher/ui/mixins/details_view_mixin.py:725 |
| `download_path`, `file_path`, `full_path` | state content download candidates | grid_launcher/library/cloud_transfer.py:143 |
| `screenshot` (nested object) | optional sidecar image; skipped when `missing_from_fs` is `true`; `file_extension` gives the sidecar suffix, default `.png` | grid_launcher/ui/mixins/cloud_mixin.py:1799, grid_launcher/ui/mixins/cloud_mixin.py:1803, grid_launcher/ui/mixins/cloud_mixin.py:1810 |

`server_records_from_payload` rejects a non-list payload, rejects non-dict items, drops
items whose `id` stringifies to blank, and de-duplicates on that string id keeping the first
occurrence (grid_launcher/library/cloud_restore.py:79).

The `emulator` field carries three special values written by this client:

| Value | Meaning | Anchor |
|---|---|---|
| `native_multi_dir` | Combined native-game archive with a `_grid_launcher_dirs.json` manifest | grid_launcher/ui/mixins/cloud_mixin.py:2740 |
| `native_dir:<raw path>` | Legacy single-directory native archive; still read, never written | grid_launcher/ui/mixins/cloud_mixin.py:2231 |
| anything else | Emulator display name as configured on the uploading device | grid_launcher/ui/mixins/cloud_mixin.py:2609 |

### Upload job

An upload job is a pair `(display_name, files_payload)` where `files_payload` maps a
multipart field name to a local path (grid_launcher/library/cloud_upload.py:9).

| Element | Type | Notes | Anchor |
|---|---|---|---|
| `display_name` | string | Used only for failure reporting and slot inference; never sent to the server | grid_launcher/ui/mixins/cloud_mixin.py:2627, grid_launcher/ui/mixins/cloud_mixin.py:1638 |
| `files_payload["saveFile"]` | path | Present for `save_type == "save"` | grid_launcher/ui/mixins/cloud_mixin.py:2479 |
| `files_payload["stateFile"]` | path | Present for `save_type == "state"` | grid_launcher/ui/mixins/cloud_mixin.py:2479 |
| `files_payload["screenshotFile"]` | path | Optional sidecar image | grid_launcher/library/cloud_transfer.py:632, grid_launcher/ui/mixins/cloud_mixin.py:2604 |

Job builders also return a list of temporary archive paths that the caller must delete after
the requests complete (grid_launcher/library/cloud_upload.py:17,
grid_launcher/library/cloud_transfer.py:365, grid_launcher/ui/mixins/cloud_mixin.py:2629).

### Session record

Held in memory only (grid-launcher.py:484):

| Field | Type | Anchor |
|---|---|---|
| `game` | copy of the game record | grid_launcher/ui/mixins/cloud_mixin.py:2833 |
| `process` | the spawned child process handle | grid_launcher/ui/mixins/cloud_mixin.py:2834 |
| `emulator_name` | trimmed emulator display name | grid_launcher/ui/mixins/cloud_mixin.py:2835 |
| `started_at` | float unix time | grid_launcher/ui/mixins/cloud_mixin.py:2836 |
| `ended_at` | float unix time, added when the session finishes | grid_launcher/ui/mixins/cloud_mixin.py:2863 |

## Behavior

### Candidate discovery

`_cloud_sync_targets_for_game(game, emulator_name, emulator, directories, save_type)`
returns `(files, folder_targets)` (grid_launcher/ui/mixins/cloud_mixin.py:506). It first
computes the resolved save strategy (`auto` / `single_file` / `folder`, where `state`
defaults to `single_file` — grid_launcher/emulator/profiles.py:356) and the ignore sets, and
records which of the configured sync paths are *files* rather than directories
(grid_launcher/ui/mixins/cloud_mixin.py:520).

Dispatch, in evaluation order:

| # | Condition | Result | Anchor |
|---|---|---|---|
| 1 | `save_type == "state"` | file candidates only; returns immediately after session filtering | grid_launcher/ui/mixins/cloud_mixin.py:522 |
| 2 | Cemu | folder targets from the Cemu title-id tree | grid_launcher/ui/mixins/cloud_mixin.py:533 |
| 3 | Dolphin | both files and folders | grid_launcher/ui/mixins/cloud_mixin.py:540 |
| 4 | strategy `folder` | generic folder targets | grid_launcher/ui/mixins/cloud_mixin.py:547 |
| 5 | RetroArch **and** save scope `shared-slotted` | Flycast VMU `.bin` files if any exist, else generic save file candidates | grid_launcher/ui/mixins/cloud_mixin.py:554 |
| 6 | strategy `single_file` | generic save file candidates | grid_launcher/ui/mixins/cloud_mixin.py:575 |
| 7 | PPSSPP | folder targets keyed on PSP game ids | grid_launcher/ui/mixins/cloud_mixin.py:583 |
| 8 | RPCS3 | folder targets keyed on PS3 game ids | grid_launcher/ui/mixins/cloud_mixin.py:585 |
| 9 | PCSX2 | folder targets keyed on PS2 serials | grid_launcher/ui/mixins/cloud_mixin.py:587 |
| 10 | fallback | generic save file candidates | grid_launcher/ui/mixins/cloud_mixin.py:589 |

If the chosen branch yields nothing at all and at least one configured sync path was a file,
that file list is re-scanned as an explicit root
(grid_launcher/ui/mixins/cloud_mixin.py:598). Finally, non-empty file and folder lists are
each passed through the session-window filter
(grid_launcher/ui/mixins/cloud_mixin.py:607).

#### Generic file candidates

`cloud_sync_candidates_for_game` (grid_launcher/library/cloud_sync.py:574) returns `[]` for
any `save_type` other than `save`/`state`. For each configured directory that exists, it
iterates the directory itself when the path is a file (`explicit_file_root`), otherwise
every entry under `rglob("*")` (grid_launcher/library/cloud_sync.py:606). Non-files, blocked
basenames, and blocked extensions are skipped
(grid_launcher/library/cloud_sync.py:610).

For `save_type == "save"`: a candidate is kept when there are no match tokens, or when the
path is an explicit file root, or when some token is a substring of the lowercased filename
or of the alphanumeric-compacted stem (grid_launcher/library/cloud_sync.py:626).

For `save_type == "state"`: the file must first pass `is_state_file_candidate`. It is then
sorted into "matched" if it is an explicit file root or its name variants match a token, and
"unmatched" otherwise (grid_launcher/library/cloud_sync.py:617). Matched candidates win; if
there are none, a fallback groups the unmatched ones (below).

`is_state_file_candidate` (grid_launcher/ui/mixins/cloud_mixin.py:1334, duplicated for TV at
grid_launcher/tv/bridge/cloud_helpers.py:126):

- reject anything whose suffix is a supported image extension;
- accept suffix in `.state`, `.savestate`, `.st`, `.ss`, `.ppst`, `.p2s`;
- accept any name containing `.state`;
- accept names matching `[._]\d+\.sav$` (DuckStation-style numbered slots);
- accept names matching `_resume\.sav$`;
- reject everything else.

State name matching tolerates emulator naming conventions. `_state_candidate_base_variants`
produces the raw name and stem plus versions with these suffix patterns stripped
(grid_launcher/library/cloud_sync.py:370):
`(\s*\([0-9a-f]+\))?(\.\d+)?\.p2s$`, `\.(savestate|state|st|ss|ppst)(\.auto|auto|[0-9]+)?$`,
`(\.\d+)?\.sav$`, `[_](\d+|resume)\.sav$`, `\.\d+$`. A candidate matches if a token equals
one of those variants exactly, or if the alphanumeric-compacted token equals a compacted
variant (grid_launcher/library/cloud_sync.py:384). An empty token set matches everything
(grid_launcher/library/cloud_sync.py:385).

The unmatched fallback (grid_launcher/library/cloud_sync.py:416): with exactly one unmatched
candidate, take it; otherwise pick the newest by mtime (ties broken by lowercased name),
derive its group key, and return every unmatched candidate sharing that key, newest first.
The group key is the 8-hex-digit prefix of `<hash>[.<n>].sav`, or the stem before
`_<digits>` / `_resume` in `<name>_<n>.sav`, else the empty string — an empty key means the
fallback returns nothing (grid_launcher/library/cloud_sync.py:405).

Final ordering for file candidates is newest mtime first, then lowercased name, then
case-insensitive de-duplication (grid_launcher/library/cloud_sync.py:638).

#### Match tokens

`_game_save_match_tokens` (grid_launcher/ui/mixins/cloud_mixin.py:1204) builds a lowercase
token set from:

- the title, plus a variant with a trailing possessive `'s` / `’s` removed, plus the
  alphanumeric-compacted forms of both (grid_launcher/ui/mixins/cloud_mixin.py:1207);
- `title_id` and `base_title_id`, both as plain variants and as Nintendo-id variants
  (grid_launcher/ui/mixins/cloud_mixin.py:1247);
- the stems of `rom_file_name`, `extracted_path`, `archive_path`
  (grid_launcher/ui/mixins/cloud_mixin.py:1253);
- `ps3_game_id` verbatim, lowercased (grid_launcher/ui/mixins/cloud_mixin.py:1261).

Nintendo-id variants add, for each `\b[A-Z][A-Z0-9]{3,5}\b` match, the first four characters
lowercased **and** their ASCII hex encoding; for each 16-hex-digit run, the whole value plus
its high and low halves; and for each `<8 hex><separator><8 hex>` pair, the high, the low,
and the concatenation (grid_launcher/ui/mixins/cloud_mixin.py:1221).

#### Folder candidates

Generic (`cloud_sync_directory_candidates_for_game`,
grid_launcher/library/cloud_sync.py:439): for each existing sync directory, consider its
immediate child directories; a child qualifies only if it contains at least one non-blocked
file anywhere beneath it (grid_launcher/library/cloud_sync.py:467). Matching compares the
alphanumeric-compacted child name **and** the compacted relative path against the token set;
an empty token set accepts everything (grid_launcher/library/cloud_sync.py:475). Results are
sorted by the newest non-blocked file mtime beneath each candidate, descending, then
de-duplicated case-insensitively (grid_launcher/library/cloud_sync.py:481).

Cemu (`cemu_save_directories_for_game`, grid_launcher/library/cloud_sync.py:492): title-id
tokens are upper-cased and stripped to `[A-Z0-9]`. Tokens of length ≥ 16 are preferred; if
none, tokens of exactly 8 characters that do **not** start with `0005`; if none, all tokens
(grid_launcher/library/cloud_sync.py:506). The scanner walks
`<dir>/<titleHigh>/<titleLow>/user/`, and takes each child directory of `user`, or `user`
itself when it has no subdirectories (grid_launcher/library/cloud_sync.py:546). Candidates
with no non-blocked files (latest mtime ≤ 0) are dropped
(grid_launcher/library/cloud_sync.py:555). Title-id matches are collected separately from
all candidates, and the matched list is used when non-empty, otherwise the full list
(grid_launcher/library/cloud_sync.py:562).

PCSX2 (grid_launcher/ui/mixins/cloud_mixin.py:1147): immediate child directories that
contain at least one file, matched against PS2 serial tokens extracted by
`[A-Z]{4}[-_ ]?\d{3}\.\d{2}` or `[A-Z]{4}[-_ ]?\d{5}` from `title`, `rom_file_name`,
`extracted_path`, `archive_path` (grid_launcher/ui/mixins/cloud_mixin.py:1401). Sorted by
newest file mtime beneath, descending.

RPCS3 (grid_launcher/ui/mixins/cloud_mixin.py:1177): immediate child directories matched
against the game's PS3 ids. Sorted by **directory index first**, then newest mtime — i.e.
earlier configured sync directories always outrank later ones
(grid_launcher/ui/mixins/cloud_mixin.py:1192).

PPSSPP (grid_launcher/ui/mixins/cloud_mixin.py:1427): immediate child directories matched
against PSP ids (`[A-Z]{4}[-_ ]?\d{5}`), sorted by the directory's own mtime descending. Note
this branch does not require the directory to contain files and does not apply ignore sets.

Dolphin (grid_launcher/ui/mixins/cloud_mixin.py:1124): returns generic file candidates and
generic folder candidates, both.

### The mtime-window (session) algorithm

`session_window_for_state_upload(active_sessions, game, games_match_identity, sync_state,
now)` (grid_launcher/library/cloud_sync.py:243) returns `(start, end)` or `None`:

1. Walk `active_game_sessions` **in reverse** (most recently registered first). For the
   first session whose `game` matches the target by identity and whose `started_at` parses
   to a positive float, return `(max(0.0, started_at - 2.0), now + 30.0)`
   (grid_launcher/library/cloud_sync.py:250).
2. Otherwise fall back to the persisted `last_session_started_at` /
   `last_session_ended_at`. If `started_at <= 0` return `None` — no window, no filtering
   (grid_launcher/library/cloud_sync.py:276). If `ended_at <= 0` or `ended_at < started_at`,
   clamp it to `started_at` (grid_launcher/library/cloud_sync.py:278). Return
   `(max(0.0, started_at - 2.0), ended_at + 30.0)`.

Identity matching prefers ROM ids when both sides have one, else compares
`(lowercased title, lowercased platform)` (grid_launcher/library/identity.py:15).

The 2-second lead-in absorbs writes that an emulator makes as it starts; the 30-second
tail-out absorbs writes flushed at (or shortly after) exit. `now` is wall-clock time at the
moment the window is computed (grid-launcher.py:3725), so for a *running* session the window
end is always slightly in the future.

Applying the window:

| Function | Input | Empty-result behaviour | Anchor |
|---|---|---|---|
| `filter_files_by_mtime_window` | file paths | returns the empty list; stat failures skip the file | grid_launcher/library/cloud_sync.py:285 |
| `session_filtered_file_candidates` | file paths | **falls back to the unfiltered list** | grid_launcher/library/cloud_sync.py:318 |
| `filter_directories_by_mtime_window` | directories, compared on the newest non-blocked file beneath each | returns the empty list | grid_launcher/library/cloud_sync.py:297 |
| `session_filtered_directory_candidates` | directories | **falls back to the unfiltered list** | grid_launcher/library/cloud_sync.py:325 |
| `filter_upload_jobs_by_session_window` | built jobs; a job survives if *any* path in its payload is in-window | returns the empty list — **no fallback** | grid_launcher/library/cloud_transfer.py:668 |

Both bounds are inclusive (`start <= mtime <= end`). A `None` window disables filtering
entirely in all five functions.

The consequence of the two different empty-result policies: candidate-level filtering can
only ever narrow a non-empty set (never empty it), so a manual "Upload" always has something
to send if any candidate exists at all; but the PPSSPP state path, which filters the finished
job list, can legitimately produce zero jobs and report "No matching PPSSPP .ppst state
files were found to upload." (grid_launcher/ui/mixins/cloud_mixin.py:2556,
grid_launcher/library/cloud_upload.py:30).

### Upload planning

`_upload_cloud_files_for_game(game, save_type, show_dialogs)`
(grid_launcher/ui/mixins/cloud_mixin.py:2427) is the single entry point for both manual and
automatic uploads; `show_dialogs=False` turns every message box into a silent no-op.

Preconditions, in order (each returns `(0, 0, [])`):

1. Native Windows platform → delegate to `_upload_native_saves_for_game`
   (grid_launcher/ui/mixins/cloud_mixin.py:2445).
2. A non-empty block reason → informational message, stop
   (grid_launcher/ui/mixins/cloud_mixin.py:2447).
3. No resolvable ROM id → "Missing ROM id for this game."
   (grid_launcher/ui/mixins/cloud_mixin.py:2463).
4. No emulator entry → "No default emulator is configured for this game's platform."
   (grid_launcher/ui/mixins/cloud_mixin.py:2467).
5. No resolved sync directories → "No save/state directories were found for emulator
   '<name>'. Configure them in Emulators."
   (grid_launcher/ui/mixins/cloud_mixin.py:2473).
6. RPCS3 + `state` → "RPCS3 savestate uploads are not supported yet."
   (grid_launcher/ui/mixins/cloud_mixin.py:2483).

Job construction by branch:

**`save_type == "save"`** (grid_launcher/ui/mixins/cloud_mixin.py:2492)

- Every folder target becomes its own directory archive, display name = folder name
  (grid_launcher/library/cloud_upload.py:13).
- If the save scope is `shared-single` and there are file candidates, **all** of them are
  zipped into one archive and uploaded as a single job named
  `"<emulator name or 'Shared Save'> Storage"`
  (grid_launcher/ui/mixins/cloud_mixin.py:2519).
- Otherwise `grouped_file_upload_jobs` groups the files
  (grid_launcher/library/cloud_transfer.py:361). The grouping key is the lowercased **stem**
  for `saveFile` and the lowercased **full name** for `stateFile`
  (grid_launcher/library/cloud_transfer.py:354) — so `game.srm` and `game.sav` group
  together, while `game.state1` and `game.state2` do not. A group of one uploads the raw
  file with its own name; a group of two or more is zipped and named after the first file's
  stem (grid_launcher/library/cloud_transfer.py:385).
- Zero jobs → "No matching save files or save folders were found to upload."
  (grid_launcher/library/cloud_upload.py:32).

**PPSSPP + `state`** (grid_launcher/ui/mixins/cloud_mixin.py:2546)

`ppsspp_state_upload_jobs` globs `*.ppst` (non-recursively) in each directory, keeps files
whose `[^A-Z0-9]`-stripped uppercase name contains a PSP id token (or all files when there
are no tokens), attaches a screenshot found by replacing the suffix
(`game.ppst` → `game.png`), sorts newest first, de-duplicates, and emits one job per file
(grid_launcher/library/cloud_transfer.py:590). The resulting jobs are then passed through
`filter_upload_jobs_by_session_window` (grid_launcher/ui/mixins/cloud_mixin.py:2556).

**RetroArch + `state`** (grid_launcher/ui/mixins/cloud_mixin.py:2576)

`retroarch_state_upload_jobs` emits one job per file with no grouping and no archiving, and
attaches a screenshot found by **appending** an image extension to the complete filename
(`game.state1` → `game.state1.png`) (grid_launcher/library/cloud_transfer.py:638,
grid_launcher/library/cloud_transfer.py:70). This preserves one server record per slot.

**Any other `state`** (grid_launcher/ui/mixins/cloud_mixin.py:2585)

`grouped_file_upload_jobs` with the `stateFile` key rule (group by full filename), which in
practice means one job per file unless two directories hold identically named states.

**Screenshot fallback** (grid_launcher/ui/mixins/cloud_mixin.py:2597)

If the emulator profile declares screenshot directories, the newest supported image whose
mtime falls in the session window is chosen (`session_screenshot_path`,
grid_launcher/library/cloud_transfer.py:89) and attached to **every** job that does not
already carry a `screenshotFile`. Supported extensions are `.jpg`, `.jpeg`, `.png`, `.webp`,
`.gif`, `.bmp` (grid_launcher/library/cloud_transfer.py:25).

**Slot assignment** (`_cloud_save_slot_for_upload_job`,
grid_launcher/ui/mixins/cloud_mixin.py:1615)

- states: always empty (no `slot` parameter is ever sent for states).
- scope `shared-single` → the literal `shared-media`.
- scope `shared-slotted` → the first `vmu([0-3])` match found across the display name, then
  each payload path's stem and full name; result `vmu0`…`vmu3`. No match → empty.
- scope `per-game` → empty.

### Upload execution

For each job (grid_launcher/ui/mixins/cloud_mixin.py:2606):

- Query parameters are `rom_id` and `emulator`; for saves also `overwrite="true"` and, when
  non-empty, `slot`.
- One multipart `POST` per job. Success increments the counter; any of `HTTPError`,
  `URLError`, `OSError`, `ValueError`, JSON decode error appends the job's display name to
  the failure list and continues with the next job
  (grid_launcher/ui/mixins/cloud_mixin.py:2626).

After the loop all temporary archives are deleted, ignoring `OSError`
(grid_launcher/ui/mixins/cloud_mixin.py:2629, grid_launcher/library/cloud_transfer.py:691).

The return value is `(success_count, total_job_count, failed_display_names)`
(grid_launcher/ui/mixins/cloud_mixin.py:2666).

The completion message (grid_launcher/library/cloud_upload.py:37):

| Condition | Message | Severity |
|---|---|---|
| failures and zero successes | `Cloud upload failed for all matching files.` | warning |
| some failures | `Uploaded N save files\|save states. Failed: <first 5 names>` | warning |
| retention delete failures | `Uploaded N …. Could not remove K older cloud saves for retention limit L.` | warning |
| otherwise | `Uploaded N save files\|save states.` | info |

### Retention pruning

Runs only for `save_type == "save"` with at least one successful upload, and only after the
uploads (grid_launcher/ui/mixins/cloud_mixin.py:2634). The limit is the constant `3`
(grid-launcher.py:2224), clamped to a minimum of 1
(grid_launcher/ui/mixins/cloud_mixin.py:1677).

`_prune_server_save_records(rom_id, emulator_name, keep_latest)`
(grid_launcher/ui/mixins/cloud_mixin.py:1676):

1. Re-fetch `GET /api/saves?rom_id=…`.
2. Keep records whose `emulator` matches case-insensitively; if `emulator_name` is blank,
   keep everything (grid_launcher/ui/mixins/cloud_mixin.py:1680). Unlike
   `latest_server_record`, there is **no** "fall back to all records when nothing matches"
   step here — a mismatch simply prunes nothing.
3. Sort by `(timestamp, numeric id)` descending
   (grid_launcher/ui/mixins/cloud_mixin.py:1701).
4. Group by slot key: `slot` lowercased, else `Path(file_name).stem` lowercased, else
   `__default__` (grid_launcher/ui/mixins/cloud_mixin.py:1706).
5. Within each group, everything after the first `keep` entries is stale
   (grid_launcher/ui/mixins/cloud_mixin.py:1721).
6. Delete each stale record with `POST /api/saves/delete {"saves": [id]}`. A non-integer id
   is recorded as a failure without a request, but a BLANK id is silently skipped and counted
   in neither list (grid_launcher/ui/mixins/cloud_mixin.py:1734-1736). HTTP 404 and 410 count as **successful**
   deletions (grid_launcher/ui/mixins/cloud_mixin.py:1752). Any other HTTP or transport
   error records the id as failed and the loop continues
   (grid_launcher/ui/mixins/cloud_mixin.py:1761).

Returns `(deleted_count, failed_ids)`. Exceptions escaping the whole call are converted into
a single failed-id entry containing the error text
(grid_launcher/ui/mixins/cloud_mixin.py:2641).

There is no client-side pruning of **states**.

### Restore — saves

`_restore_cloud_save_for_game(game, save_record=None, show_dialogs, skip_if_local_newer,
skip_if_known_latest)` (grid_launcher/ui/mixins/cloud_mixin.py:1901):

1. Native Windows platform → delegate to `_restore_native_cloud_save_for_game`
   (grid_launcher/ui/mixins/cloud_mixin.py:1920).
2. Block reason → info message, `False` (grid_launcher/ui/mixins/cloud_mixin.py:1930).
3. Missing ROM id → warning, `False` (grid_launcher/ui/mixins/cloud_mixin.py:1940).
4. If a specific record was supplied and it names an emulator: look that emulator up. If it
   is not configured **and** its name differs from the resolved emulator name, refuse with
   "Emulator '<name>' is not configured on this device."
   (grid_launcher/ui/mixins/cloud_mixin.py:1954). If it is configured, it overrides the
   resolved emulator for the rest of the restore
   (grid_launcher/ui/mixins/cloud_mixin.py:1956).
5. No emulator entry, or no resolved save directories → warning, `False`
   (grid_launcher/ui/mixins/cloud_mixin.py:1960, grid_launcher/ui/mixins/cloud_mixin.py:1964).
6. Record selection when none was supplied
   (`_latest_server_save_records_for_game`, grid_launcher/ui/mixins/cloud_mixin.py:1596):
   scope `per-game` → the single latest record; any shared scope → the latest record **per
   slot**. An empty result reports "No cloud save was found on the server for this game."
   (grid_launcher/ui/mixins/cloud_mixin.py:1985).
7. Short circuits — see "Conflict and newer detection".
8. Download and place each selected record, in list order
   (grid_launcher/ui/mixins/cloud_mixin.py:2056):
   - **Folder-save emulators** (PPSSPP, RPCS3, PCSX2, Cemu —
     grid_launcher/ui/mixins/cloud_mixin.py:2041): the payload is extracted into
     `directories[0]`; zero extracted files raises "Save archive downloaded, but no files
     were restored." (grid_launcher/ui/mixins/cloud_mixin.py:2072).
   - **Everything else**: `_restore_single_save_file` picks a target path and either extracts
     (zip payload) or writes the bytes (grid_launcher/ui/mixins/cloud_mixin.py:2076).
   - Any `HTTPError`, `URLError`, `OSError`, `ValueError`, or bad-zip error aborts the whole
     restore with a warning and `False` — records already written stay written
     (grid_launcher/ui/mixins/cloud_mixin.py:2092).
9. On success, persist `last_downloaded_save_id` (the id of the newest record actually
   restored) and `last_server_timestamp`
   (grid_launcher/ui/mixins/cloud_mixin.py:2096), then report success.

Target-path selection (`preferred_restore_target_path`,
grid_launcher/library/cloud_restore.py:150), given the record's `file_name`, the local
candidate list, and a fallback name:

1. No directories at all → `None`.
2. For the record filename, then the fallback filename: return the first local candidate
   whose filename matches case-insensitively. This is what keeps a restored save in the
   nested folder it currently lives in.
3. Record filename present and candidates exist → `candidates[0].parent / record_filename`.
4. Candidates exist → `candidates[0]` (overwrite it).
5. Record filename present → `directories[0] / record_filename`.
6. Fallback filename present → `directories[0] / fallback_filename`.
7. Else `None`.

The save fallback name is `<sanitised title>.srm`
(grid_launcher/ui/mixins/cloud_mixin.py:1858); the state fallback name is
`<sanitised title>.state` (grid_launcher/ui/mixins/cloud_mixin.py:1888).

Placement (`restore_single_save_payload`, grid_launcher/library/cloud_restore.py:186):
empty payload or no directories → `None`; parent directories are created; if the payload
sniffs as a zip it is extracted into the **parent** of the target path and the parent is
returned (or `None` when nothing was extracted); otherwise the bytes overwrite the target
path unconditionally.

### Restore — states

`_restore_cloud_state_for_game` (grid_launcher/ui/mixins/cloud_mixin.py:2254) mirrors the
save flow with these differences:

- The ROM id comes straight from `_resolve_rom_id_for_game`, never from the shared-owner
  lookup (grid_launcher/ui/mixins/cloud_mixin.py:2283).
- RPCS3 is refused outright: "RPCS3 savestate restore is not supported yet."
  (grid_launcher/ui/mixins/cloud_mixin.py:2308).
- Exactly one record is restored — `latest_server_record`, never a per-slot list
  (grid_launcher/ui/mixins/cloud_mixin.py:2319).
- Content is fetched through the two-step record-then-path flow described under "State
  content resolution".
- A screenshot is fetched if the record carries one; failure to fetch is logged (in debug
  builds) and ignored (grid_launcher/ui/mixins/cloud_mixin.py:2362).
- Placement uses `restore_single_state_payload`
  (grid_launcher/library/cloud_restore.py:219): identical to the save version except that
  after a **non-zip** write it also writes the screenshot bytes to
  `str(target_path) + screenshot_extension` (default `.png`). Zip payloads never get a
  sidecar (grid_launcher/library/cloud_restore.py:241).
- On success only `last_downloaded_state_id` is persisted
  (grid_launcher/ui/mixins/cloud_mixin.py:2398).

State record listing filters out records whose `file_name` ends with a supported image
extension, so screenshot assets returned by the server are never treated as states
(grid_launcher/ui/mixins/cloud_mixin.py:1653).

### Restore — native games

`_restore_native_cloud_save_for_game` (grid_launcher/ui/mixins/cloud_mixin.py:2114):

- Builds a fallback directory list from the PCGW + manual raw paths, expanded with the
  Shell Documents folder and the game's `native_wineprefix`
  (grid_launcher/ui/mixins/cloud_mixin.py:2142).
- Fetches records with `_latest_server_save_records_for_game(game, rom_id, "", {})` — an
  empty emulator name, so `cloud_save_scope_for_game` resolves to `per-game` and exactly the
  single LATEST record is restored, not one per slot
  (grid_launcher/ui/mixins/cloud_mixin.py:2154). RULED (milestone 6): the Rust port follows
  the code, not this section's old "per-slot" prose (now fixed) — see "Rust port deviations
  (milestone 6)", follow-the-code rulings, and
  `crates/grid-core/src/cloud/ops/native.rs:184-188`.
- Branches on each record's `emulator` field
  (grid_launcher/ui/mixins/cloud_mixin.py:2176):
  - `native_multi_dir`: read `_grid_launcher_dirs.json` from the archive (a missing or
    malformed manifest degrades to an empty manifest —
    grid_launcher/ui/mixins/cloud_mixin.py:2191); for each member split off the leading
    `<index>/`; resolve the target root from the manifest entry for that index, else from
    `fallback_dirs[0]`, else skip the member
    (grid_launcher/ui/mixins/cloud_mixin.py:2208); reject members that resolve outside the
    target root; create parents and overwrite.
  - `native_dir:<raw path>`: expand the suffix, create the directory, extract the whole
    archive into it (grid_launcher/ui/mixins/cloud_mixin.py:2231).
  - anything else: extract into `fallback_dirs[0]`, or fail with "No restore directories
    configured." (grid_launcher/ui/mixins/cloud_mixin.py:2238).
- The entire body is wrapped in a bare `except Exception` that reports and returns `False`
  (grid_launcher/ui/mixins/cloud_mixin.py:2246) — broader than the emulator restore path.

### Upload — native games

`_upload_native_saves_for_game` (grid_launcher/ui/mixins/cloud_mixin.py:2668):

1. No configured paths → "No save locations are configured for this game. Use 'Manage
   Saves' → 'Browse' to add one." (grid_launcher/ui/mixins/cloud_mixin.py:2692).
2. Missing ROM id → warning (grid_launcher/ui/mixins/cloud_mixin.py:2699).
3. Expand each raw path; keep only those that currently exist. If none exist, the warning
   lists every expanded path that was checked
   (grid_launcher/ui/mixins/cloud_mixin.py:2714).
4. Build one combined archive (`zip_native_save_dirs_for_upload`,
   grid_launcher/library/cloud_transfer.py:431). A directory that raises `OSError` while
   being walked is skipped **and omitted from the manifest**; a file that raises `OSError`
   while being written is skipped; the manifest member is always written even when zero
   files were added (grid_launcher/library/cloud_transfer.py:457,
   grid_launcher/library/cloud_transfer.py:473, grid_launcher/library/cloud_transfer.py:476).
5. Zero files → delete the archive and report "No matching save files or save folders were
   found to upload." (grid_launcher/ui/mixins/cloud_mixin.py:2730).
6. One multipart `POST /api/saves` with `emulator="native_multi_dir"` and
   `overwrite="true"`; the archive is deleted in a `finally`
   (grid_launcher/ui/mixins/cloud_mixin.py:2738).
7. Retention prune keyed on the emulator name `native_multi_dir`
   (grid_launcher/ui/mixins/cloud_mixin.py:2758).
8. Returns `(success_count, 1, failed)` — the total is always 1
   (grid_launcher/ui/mixins/cloud_mixin.py:2778).

`resolve_native_save_dir(raw_path, windows_documents, wine_prefix)`
(grid_launcher/library/cloud_transfer.py:484):

- On non-Windows with a wine prefix, first try translating the Windows env-var path into the
  prefix; return that when it succeeds
  (grid_launcher/library/cloud_transfer.py:506).
- Otherwise expand environment variables. On non-Windows, or when no Shell Documents path
  was supplied, return that directly.
- On Windows, compare the Shell Documents path with `%USERPROFILE%\Documents`. If they
  agree, no redirection is in play and the expansion stands
  (grid_launcher/library/cloud_transfer.py:522). If the expanded path *is*
  `%USERPROFILE%\Documents`, return the Shell path; if it is *under* it, splice the
  remainder onto the Shell path (grid_launcher/library/cloud_transfer.py:532).

### Save-location panel

`_native_save_paths_for_game` (grid_launcher/ui/mixins/details_view_mixin.py:1060) builds the
row list: PCGamingWiki-discovered paths first, then any manually-added path not already among
them (`path not in cached`). `_render_native_save_path_section` (:1072-1141) turns that list
into one row per path — a label showing the raw (unexpanded) string, `os.path.expandvars(raw)`
as the label's tooltip, and a trash-can remove button (accessible name "Remove", tooltip
"Remove this path") that calls `_pcgw_remove_path_for_game` and refreshes the panel
(:1096-1126).

`_refresh_native_save_panel` (:1143-1185) picks the status line, empty label and upload-button
tooltip from the row count and the ROM id: no rows → status "No save locations found on
PCGamingWiki.", upload disabled, tooltip "Add a save location to enable uploads."; rows present
→ status "`<n>` save location(s) configured.", upload enabled only when a ROM id exists, tooltip
"Upload save files from the listed locations." when it does, else "Missing ROM id for this
game." The empty label under the section heading is "Missing ROM id for this game." or "Loading
cloud saves from the server..." depending on the ROM id.

Removing a row (`_pcgw_remove_path_for_game`, :1224-1235) deletes it from both the PCGW cache
and the manual list for that game and persists the manual-list change to
`config["native_manual_save_paths"]`. Adding the same raw path back through the manual field or
Browse (`_pcgw_add_manual_path_for_game`, :1211-1222) re-adds it to the manual cache, so it
reappears on the next refresh.

### Conflict and newer detection

Two independent short circuits, both only active on automatic (pre-launch) restores.

**Already have this record** — `should_skip_known_latest(last_downloaded_id, current_id,
local_latest_mtime)` returns true when the stored id is non-empty, equals the server record's
id, **and** the local latest mtime is greater than zero
(grid_launcher/library/cloud_transfer.py:705). The mtime clause is what makes a fresh
install re-download a save it has already "downloaded" on another machine: the ids match but
nothing exists locally, so the restore proceeds
(grid_launcher/ui/mixins/cloud_mixin.py:2013). For saves this check is applied **only when
the scope is `per-game`** (grid_launcher/ui/mixins/cloud_mixin.py:2001); for states it always
applies (grid_launcher/ui/mixins/cloud_mixin.py:2333).

**Local newer than server** — `is_local_newer_than_server(local_mtime, server_timestamp)`
returns true when the local mtime is positive and exceeds the server timestamp by more than
one second (grid_launcher/library/cloud_transfer.py:709). The server timestamp used is the
maximum over all records selected for restore
(grid_launcher/ui/mixins/cloud_mixin.py:2028). This check is applied only to saves, only
when `skip_if_local_newer` is set, and it is **deliberately skipped for PCSX2 when no PS2
serial tokens could be derived** — without serials the local candidate scan is too broad to
trust (grid_launcher/ui/mixins/cloud_mixin.py:2020). States have no local-newer check at
all.

`_latest_local_save_mtime_for_game` computes the maximum over both file candidates and the
newest non-blocked file under each folder target
(grid_launcher/ui/mixins/cloud_mixin.py:1559). `_latest_local_state_mtime_for_game` returns
`0.0` for RPCS3 and otherwise the maximum over state file candidates
(grid_launcher/ui/mixins/cloud_mixin.py:1534). Both synthesise a stub emulator entry
`{"name": …, "path": "", "args": "%rom%", "save_strategy": "auto"}` when the named emulator
is not configured (grid_launcher/ui/mixins/cloud_mixin.py:1547).

### Save scope

`cloud_save_scope_for_game` (grid_launcher/emulator/selection.py:56) ignores the game entirely
(`del game`) and returns one of three values:

| Scope | Trigger | Effect |
|---|---|---|
| `per-game` | `save_type != "save"`, or none of the rules below | one record per game; latest-only selection; no `slot` parameter |
| `shared-single` | emulator is xemu (grid_launcher/emulator/selection.py:70) | all save files bundled into one archive; `slot="shared-media"` |
| `shared-slotted` | emulator is Redream (grid_launcher/emulator/selection.py:77), or RetroArch with core flag `vmu_shared_saves` (grid_launcher/emulator/selection.py:84) | one record per VMU slot; `slot="vmu0".."vmu3"`; per-slot latest selection on restore |

Scope drives: the button label ("Emulator Saves" vs "Manage Saves" —
grid_launcher/ui/mixins/cloud_mixin.py:246), the warning banner shown before restore/delete
(grid_launcher/ui/mixins/cloud_mixin.py:284), which records get restored
(grid_launcher/ui/mixins/cloud_mixin.py:1610), the archive shape
(grid_launcher/ui/mixins/cloud_mixin.py:2519), the slot value
(grid_launcher/ui/mixins/cloud_mixin.py:1633), and whether the known-latest short circuit
applies (grid_launcher/ui/mixins/cloud_mixin.py:2001).

Shared-scope emulators also change **which ROM id** the records hang off. For saves,
`_cloud_sync_rom_id_for_game` first looks for a "shared sync owner": any library or server
game whose combined `title/platform/description/rom_file_name` text contains `xemu` (for
xemu) or `redream` (for Redream) and which has a resolvable ROM id
(grid_launcher/ui/mixins/cloud_mixin.py:398, grid_launcher/ui/mixins/cloud_mixin.py:392). If
found, that owner's ROM id is used instead of the game's own
(grid_launcher/ui/mixins/cloud_mixin.py:458). When no such game exists, the emulator's
install directory is scanned for matching installed games as a last resort
(grid_launcher/ui/mixins/cloud_mixin.py:424). States never use this indirection.

### Emulator resolution for cloud operations

`_resolved_cloud_emulator_entry_for_game` (grid_launcher/ui/mixins/cloud_mixin.py:175):

1. Cache lookup on `"<title>::<platform>::<save_type>"`
   (grid_launcher/ui/mixins/cloud_mixin.py:191). The cache is cleared on every config save
   (grid-launcher.py:3152).
2. The normal default-emulator resolution for the game's platform. If it yields an entry,
   done (grid_launcher/ui/mixins/cloud_mixin.py:198).
3. If the game's platform is not the literal `Emulators`, give up
   (grid_launcher/ui/mixins/cloud_mixin.py:205).
4. Otherwise scan all configured emulators for one whose shared-sync token appears in the
   game's text, skipping any whose save scope is `per-game` when `save_type == "save"`
   (grid_launcher/ui/mixins/cloud_mixin.py:212). This is how an entry on the synthetic
   `Emulators` platform (e.g. the xemu package itself) gets a cloud panel.

### Block reasons

`cloud_save_block_reason_for_game` (grid_launcher/emulator/selection.py:96) returns an empty
string when the operation is allowed, otherwise a user-facing sentence. Every reason:

| Returned string | Trigger | Anchor |
|---|---|---|
| `Cloud save management is only available for emulator-based games.` | `is_native_executable_platform(game)` is true, i.e. the platform string starts with `windows`. Checked before anything else and for both save types. | grid_launcher/emulator/selection.py:110, grid_launcher/emulator/selection.py:145 |
| `This core does not support save states.` | `save_type == "state"` **and** a non-empty emulator name **and** that emulator is RetroArch **and** a core-flags dict was supplied **and** `supports_save_states` is false | grid_launcher/emulator/selection.py:113 |
| `Save state format for this core may not be stable across devices.` | Same preconditions, `supports_save_states` true but `cloud_sync_safe` false | grid_launcher/emulator/selection.py:122 |
| `This core does not support battery saves.` | `save_type == "save"` **and** a non-empty emulator name **and** RetroArch **and** a core-flags dict was supplied **and** `supports_saves` is false | grid_launcher/emulator/selection.py:125 |
| `""` (allowed) | none of the above | grid_launcher/emulator/selection.py:135 |

All three core flags default to `true` when the core is missing from the core list or the
field is absent, so unknown cores are never blocked
(grid_launcher/emulator/retroarch.py:587). The `is_xemu_emulator_name` and
`is_redream_emulator_name` callbacks are accepted but explicitly discarded
(grid_launcher/emulator/selection.py:107) — they exist only so callers can share one
signature with `cloud_save_scope_for_game`.

The wrapper `_cloud_save_block_reason_for_game`
(grid_launcher/ui/mixins/cloud_mixin.py:96) supplies those inputs:

- emulator name = explicit argument, else the `name` field of the supplied entry, else the
  platform's default emulator (grid_launcher/ui/mixins/cloud_mixin.py:104).
- core flags are computed **only** when the emulator is RetroArch *and* a default core is
  configured for the platform (grid_launcher/ui/mixins/cloud_mixin.py:116). With no
  configured core, flags stay `None` and no core-based reason can fire. Note the asymmetry
  with `_cloud_save_scope_for_game`, which additionally falls back to
  `retroarch_core_flags_for_platform` in that case
  (grid_launcher/ui/mixins/cloud_mixin.py:162).

Beyond that function, these additional refusals gate cloud actions. A port needs all of
them, but they are not part of the block-reason API:

| Message | Where | Anchor |
|---|---|---|
| `Missing ROM id for this game.` | upload, restore, native upload/restore | grid_launcher/ui/mixins/cloud_mixin.py:1941 |
| `No default emulator is configured for this game's platform.` | upload, restore | grid_launcher/ui/mixins/cloud_mixin.py:1961 |
| `No save\|state directories were found for emulator '<n>'. Configure them in Emulators.` | upload, restore | grid_launcher/ui/mixins/cloud_mixin.py:1966 |
| `Emulator '<n>' is not configured on this device.` | restoring a record whose `emulator` is unknown locally and differs from the resolved emulator | grid_launcher/ui/mixins/cloud_mixin.py:1954 |
| `RPCS3 savestate uploads are not supported yet.` | state upload | grid_launcher/ui/mixins/cloud_mixin.py:2484 |
| `RPCS3 savestate restore is not supported yet.` | state restore, and the per-row Restore button | grid_launcher/ui/mixins/cloud_mixin.py:2309, grid_launcher/ui/mixins/details_view_mixin.py:676 |
| `Configure emulator '<n>' in Emulators to restore this entry.` | per-row Restore button | grid_launcher/ui/mixins/details_view_mixin.py:683 |
| `No cloud save\|save state was found on the server for this game.` | restore with no records | grid_launcher/ui/mixins/cloud_mixin.py:1985, grid_launcher/ui/mixins/cloud_mixin.py:2325 |
| `Server save\|state record is missing an id.` | malformed record | grid_launcher/ui/mixins/cloud_mixin.py:1991 |
| `Downloaded cloud save\|state content was empty.` | zero-byte download | grid_launcher/ui/mixins/cloud_mixin.py:2063 |
| `Save archive downloaded, but no files were restored.` | zip extracted zero members | grid_launcher/ui/mixins/cloud_mixin.py:2073 |
| `Save\|State content downloaded, but no file was restored.` | target path could not be chosen | grid_launcher/ui/mixins/cloud_mixin.py:2085 |
| `Install this game to manage cloud saves\|states.` | panel gate for an uninstalled, non-shared game | grid_launcher/ui/mixins/details_view_mixin.py:986 |
| `Save states are not supported for native games.` | native panel with `save_type == "state"` | grid_launcher/ui/mixins/details_view_mixin.py:1148 |

`_details_cloud_mode_supported` decides whether the Manage Saves / Manage States buttons
appear at all (grid_launcher/ui/mixins/cloud_mixin.py:296). It returns false for: an invalid
save type; native platform + `state`; native platform that is not installed; a non-installed
game that is not on the `Emulators` platform; no resolvable emulator entry; `save` on the
`Emulators` platform with `per-game` scope; `state` on the `Emulators` platform or on RPCS3;
a non-empty block reason; and finally no resolved sync directories.

### Auto-sync triggers

**Before launch** — `_auto_sync_before_launch(game)`
(grid_launcher/ui/mixins/cloud_mixin.py:2800), called from `_perform_game_action` just before
launching an installed game (grid_launcher/ui/mixins/details_view_mixin.py:1497). It runs
only when `auto_cloud_save_download_on_launch` is true (default true, grid-launcher.py:2212),
credentials exist, and the server is connected. It then restores the save with
`skip_if_local_newer = auto_cloud_save_skip_download_if_local_newer` (default true,
grid-launcher.py:2218) and `skip_if_known_latest = True`, and the state with
`skip_if_known_latest = True`, both with dialogs suppressed. Each is gated on its own block
reason (grid_launcher/ui/mixins/cloud_mixin.py:2805).

**Session registration** — `_register_game_session_for_auto_upload(game, process,
emulator_name)` (grid_launcher/ui/mixins/cloud_mixin.py:2819), called right after the
emulator process is spawned (grid_launcher/ui/mixins/details_view_mixin.py:1472). It returns
early only when **both** the save and the state block reasons are non-empty
(grid_launcher/ui/mixins/cloud_mixin.py:2825). It appends the session record and writes
`last_session_started_at = now`, `last_session_ended_at = 0.0`
(grid_launcher/ui/mixins/cloud_mixin.py:2839).

**Polling** — a repeating 2500 ms timer calls `_poll_active_game_sessions`
(grid-launcher.py:514). `partition_active_game_sessions` splits sessions into still-running
and finished by calling `process.poll()`: a session whose `process` has no callable `poll`
is **dropped from both lists** (i.e. silently discarded); a poll that raises is treated as
still running; `None` means running; anything else means finished
(grid_launcher/library/cloud_sync.py:114).

**After exit** — `_handle_finished_game_session`
(grid_launcher/ui/mixins/cloud_mixin.py:2857) stamps `ended_at`, writes
`last_session_started_at` / `last_session_ended_at` into the sync state (only when the
session's `started_at` was positive — grid_launcher/library/cloud_sync.py:146), then returns
early unless `auto_cloud_save_upload_on_exit` is true (default true, grid-launcher.py:2215),
credentials exist, and the server is connected. It then schedules
`_auto_upload_after_session` after `auto_cloud_save_upload_delay_seconds` (default 3, clamped
to 0–60 — grid-launcher.py:2221), or runs it immediately when the delay is zero
(grid_launcher/ui/mixins/cloud_mixin.py:2874).

**Auto upload plan** — `_auto_upload_after_session`
(grid_launcher/ui/mixins/cloud_mixin.py:2881) resolves the emulator (falling back to the
platform default when the session did not record one), gives up if it is not configured, and
computes:

- `local_latest_save_mtime`, but only when the save block reason is empty and save
  directories resolve (grid_launcher/ui/mixins/cloud_mixin.py:2897);
- `local_latest_state_mtime`, but only when state directories resolve, the emulator is not
  RPCS3, and the state block reason is empty
  (grid_launcher/ui/mixins/cloud_mixin.py:2909).

`auto_cloud_upload_plan(sync_state, save_mtime, state_mtime, include_state)`
(grid_launcher/library/cloud_sync.py:154) then decides:

- `"save"` is planned when `save_mtime > 0` and `save_mtime > previous + 1.0`, where
  `previous` is `last_uploaded_save_mtime`, falling back to `last_uploaded_local_mtime`,
  falling back to `0` (grid_launcher/library/cloud_sync.py:163). Unparseable stored values
  are treated as `0.0`.
- `"state"` is planned under the same rule against `last_uploaded_state_mtime`, and only when
  `include_state` is true (grid_launcher/library/cloud_sync.py:173).
- Returns `(upload_types, {type: latest_mtime})`. An empty type list ends the flow
  (grid_launcher/ui/mixins/cloud_mixin.py:2928).

The 1-second slack means a save whose mtime moved by less than a second since the last upload
is not re-uploaded.

**Result bookkeeping** — `summarize_auto_cloud_upload_result(result, uploaded_at)`
(grid_launcher/library/cloud_sync.py:186) walks `save` then `state`:

- entries with `total <= 0`, `uploaded <= 0`, and no failures are skipped entirely;
- when `uploaded > 0`, the planned latest mtime for that type is written back
  (`last_uploaded_save_mtime` **and** the legacy `last_uploaded_local_mtime` for saves;
  `last_uploaded_state_mtime` for states) (grid_launcher/library/cloud_sync.py:229);
- `last_uploaded_at` is written once if anything at all uploaded and the timestamp string is
  non-blank (grid_launcher/library/cloud_sync.py:237);
- a debug segment `"<type>=<uploaded>/<max(total,uploaded)> failed=<first 3>"` is produced per
  type (grid_launcher/library/cloud_sync.py:235).

The caller passes `datetime.now(UTC).isoformat(timespec="seconds")` with `+00:00` rewritten
to `Z` (grid_launcher/ui/mixins/cloud_mixin.py:2966).

### Manual actions

The details panel toggles between `overview`, `save`, and `state` modes
(`current_details_cloud_mode`, grid_launcher/ui/mixins/details_view_mixin.py:934). Selecting
the same mode twice returns to overview
(grid_launcher/ui/mixins/details_view_mixin.py:566). Entering a mode renders the header, the
upload button (enabled/disabled with a tooltip explaining why), and then starts a background
worker to fetch records (grid_launcher/ui/mixins/details_view_mixin.py:1050).

Rows render title (`file_name`, else `Cloud Save|State #<id>`), a summary line
(`emulator • size [• Slot <slot>]`), and an absolute + relative upload time
(grid_launcher/ui/mixins/details_view_mixin.py:614). Relative text buckets as implemented:
`just now` (<30 s), `1 minute ago` (<90 s), then `N hours ago` for EVERYTHING under 24 h
(minimum "1 hour ago" — 120 s renders as `1 hour ago`), `N days ago` (<7 days), else
`N weeks ago`; a zero timestamp renders `Unknown` (grid_launcher/library/cloud_restore.py:30).
The range tuple at grid_launcher/library/cloud_restore.py:39-48 is ordered largest threshold
first, so the `minute` entry is unreachable and `N minutes ago` is never emitted above 90 s.
OPEN QUESTION: this is almost certainly a bug (the intended buckets were minutes then hours);
a port must decide whether to reproduce it or fix it and diverge. RULED (milestone 6): ported
as-is, bug included — `crates/grid-core/src/cloud/restore.rs:321-345`'s
`relative_timestamp_text` reproduces the exact bucket ranges and the 120-second-renders-"1
hour ago" behavior; see "Rust port deviations (milestone 6)", follow-the-code rulings.

Restore and Delete each show a confirmation dialog; the shared-scope notice is appended as a
`Warning:` paragraph when the scope is shared
(grid_launcher/ui/mixins/details_view_mixin.py:1247,
grid_launcher/ui/mixins/details_view_mixin.py:1276). Delete treats HTTP 404 and 410 as
success (grid_launcher/ui/mixins/details_view_mixin.py:1309). A successful restore or delete
refreshes the panel.

Manual upload buttons resolve the installed game record first and do nothing if the game is
not installed (grid_launcher/ui/mixins/cloud_mixin.py:2783).

## Invariants and error handling

- **Zip-slip is blocked on every extraction path.** Members with absolute paths or any
  `.`/`..`/empty component are skipped, and the resolved destination must still be under the
  resolved destination root (grid_launcher/library/cloud_transfer.py:253,
  grid_launcher/library/cloud_transfer.py:260). The 7-Zip fallback re-applies the same check
  after extracting to a temp directory (grid_launcher/library/cloud_transfer.py:198). The
  native manifest restore applies it per member
  (grid_launcher/ui/mixins/cloud_mixin.py:2217).
- **Ignore sets apply on both sides.** Blocked basenames and extensions are filtered when
  writing archives (grid_launcher/library/cloud_transfer.py:414), when extracting
  (grid_launcher/library/cloud_transfer.py:255), when scanning candidates
  (grid_launcher/library/cloud_sync.py:612), and when computing latest mtimes
  (grid_launcher/ui/mixins/cloud_mixin.py:1524).
- **Temporary archives are always cleaned up.** Archive builders unlink the partial file and
  re-raise on `OSError` (grid_launcher/library/cloud_transfer.py:345,
  grid_launcher/library/cloud_transfer.py:422, grid_launcher/library/cloud_transfer.py:477).
  The upload path deletes all collected temporaries after the request loop, ignoring
  per-file `OSError` (grid_launcher/library/cloud_transfer.py:691). The native path uses a
  `finally` (grid_launcher/ui/mixins/cloud_mixin.py:2750). The temp zip used for extraction
  is unlinked in a `finally` (grid_launcher/library/cloud_transfer.py:278).
- **Uploads are per-job independent.** One failed job never aborts the rest
  (grid_launcher/ui/mixins/cloud_mixin.py:2626). Restores are **not** independent: the first
  failure aborts the loop (grid_launcher/ui/mixins/cloud_mixin.py:2092).
- **Sync state is only advanced on observed success.** `last_downloaded_*` is written after
  a completed restore; `last_uploaded_*` only for types with `uploaded_count > 0`.
- **`stat()` failures never abort a scan.** They skip the entry and continue
  (grid_launcher/library/cloud_sync.py:290, grid_launcher/ui/mixins/cloud_mixin.py:1530).
- **Retention deletes are best-effort.** Failures are reported in the completion message but
  do not turn a successful upload into a failure
  (grid_launcher/library/cloud_upload.py:51).
- **Exception scopes differ by path.** Emulator restore/upload catch
  `(HTTPError, URLError, OSError, ValueError, json.JSONDecodeError)` and
  `zipfile.BadZipFile`; the native restore catches bare `Exception`
  (grid_launcher/ui/mixins/cloud_mixin.py:2246); the TV backend catches bare `Exception`
  everywhere (grid_launcher/tv/bridge/cloud_backend.py:66).
- **Latent defect:** the two absolute-URL download branches call
  `self._authorized_headers()` (grid_launcher/ui/mixins/cloud_mixin.py:1786,
  grid_launcher/ui/mixins/cloud_mixin.py:1814), and no such method is defined anywhere in the
  repository. Because `AttributeError` is not in the caught tuple, a state or screenshot
  record whose `download_path` is an absolute `http(s)` URL would raise out of the restore.
  In practice RomM returns server-relative paths, so the branch is not normally reached.

## Platform differences

- **Windows Documents redirection.** Save paths containing `%USERPROFILE%\Documents` are
  rewritten to the Shell-resolved Documents folder when the two differ, both for the
  `%DOCUMENTS%` token in emulator sync paths
  (grid_launcher/ui/mixins/cloud_mixin.py:935) and for native game paths
  (grid_launcher/library/cloud_transfer.py:517). No adjustment happens off Windows.
- **Wine prefix translation.** On non-Windows, a native game with a `native_wineprefix`
  resolves its Windows-style save paths inside that prefix before anything else
  (grid_launcher/library/cloud_transfer.py:506,
  grid_launcher/ui/mixins/cloud_mixin.py:2706).
- **7-Zip fallback binary.** The bundled `assets/tools/7z/7z.exe` is a Windows binary; on
  other platforms only the `PATH` lookups for `7z`/`7za`/`7zz` can succeed
  (grid_launcher/library/cloud_transfer.py:34, grid_launcher/library/cloud_transfer.py:165).
- **Subprocess window suppression.** `CREATE_NO_WINDOW` is passed on `win32` and `0`
  elsewhere when invoking 7-Zip (grid_launcher/library/cloud_transfer.py:160).
- **Path comparisons are case-insensitive everywhere** (`casefold()` on the string form of
  paths for de-duplication and ignore matching —
  grid_launcher/library/cloud_sync.py:346), which is correct on Windows and merely
  conservative elsewhere.
- **Native (Windows-platform) games are excluded from all emulator cloud logic** by the very
  first block-reason check (grid_launcher/emulator/selection.py:110); they get a separate
  panel and separate upload/restore functions.

## Concurrency

- **Session polling** runs on a repeating 2500 ms timer on the UI thread
  (grid-launcher.py:514). It mutates `active_game_sessions` by replacing it with the
  still-running list (grid_launcher/ui/mixins/cloud_mixin.py:2855).
- **Auto upload** runs on a dedicated worker object moved to a fresh `QThread` per upload
  (grid_launcher/ui/mixins/cloud_mixin.py:2939). The worker calls
  `_upload_cloud_files_for_game` with `show_dialogs=False` for each planned type in sequence
  and emits `{"game", "result": {"per_type", "local_latest_mtimes"}}`
  (grid_launcher/background/workers.py:670). Transport errors inside `run` are converted into
  a per-type failure result rather than an exception
  (grid_launcher/background/workers.py:694). Threads and workers are tracked in two parallel
  lists and removed on `thread.finished` (grid_launcher/ui/mixins/cloud_mixin.py:2954).
  There is no cap on concurrent auto-upload threads and no de-duplication per game.
- **Details record loading** uses a monotonically increasing `details_cloud_request_id`
  (grid_launcher/ui/mixins/details_view_mixin.py:797). A result is discarded when its
  request id is stale, when the panel mode changed, or when the stored request context no
  longer matches (grid_launcher/ui/mixins/details_view_mixin.py:844). The thread is started
  via a zero-delay single shot so the loading state paints first
  (grid_launcher/ui/mixins/details_view_mixin.py:820).
- **Restore-enabled memoisation** is per request context, keyed on
  `(save_type, game key, lowercased emulator name)`
  (grid_launcher/ui/mixins/details_view_mixin.py:643) so that rendering N rows does not run
  N directory scans.
- **Caches invalidated on config save.** `_sync_directory_paths_cache` and
  `_cloud_emulator_entry_cache` are both cleared at the top of `_save_config`
  (grid-launcher.py:3151). Since every sync-state update saves the config
  (grid_launcher/ui/mixins/details_view_mixin.py:384), a restore or auto upload also flushes
  these caches.
- **TV bridge** keeps at most one fetch thread and one upload thread; starting a new one
  quits the previous and waits up to 2000 ms
  (grid_launcher/tv/bridge/cloud_backend.py:277,
  grid_launcher/tv/bridge/cloud_backend.py:284). The TV launch path holds the launch command
  in `_pending_restore_launch` and only spawns the emulator after the restore worker
  finishes (grid_launcher/tv/bridge/game_backend.py:325).
- **No file locking anywhere.** Nothing prevents an upload scan from running while the
  emulator is still writing; the session window's 30-second tail is the only mitigation.

## TV-mode variants

The TV/QML bridge is a reduced re-implementation, not a wrapper around the desktop mixin. It
shares only the pure helpers in `grid_launcher/library/cloud_restore.py`,
`cloud_sync.py`, and `cloud_transfer.py`.

| Aspect | Desktop | TV | Anchor |
|---|---|---|---|
| Sync directory resolution | Full pipeline: entry → profile → per-emulator overrides → token expansion → existence check → cache | `resolve_emulator_save_directories`: entry paths if set, else a single per-emulator override list; **no** token expansion, no existence check, no profile `save_directories` | grid_launcher/tv/bridge/cloud_helpers.py:170 |
| RetroArch directories | Config override prepended, then literal `saves`/`savefiles` (or `states`/`savestates`) appended | First non-blank of `savefile_directory`,`saves`,`savefiles` (or the state trio); returns exactly one path | grid_launcher/tv/bridge/cloud_helpers.py:188 |
| Emulator resolution | Default-emulator resolution plus the `Emulators`-platform shared-sync scan and caching | Exact-string lookup of `config["default_emulators"][platform]` in `config["emulators"]` | grid_launcher/tv/bridge/cloud_helpers.py:150 |
| Candidate discovery | Ten-branch dispatch with folder targets, Cemu/PCSX2/RPCS3/PPSSPP/Dolphin specials, session filtering | `cloud_sync_candidates_for_game` only — files, never folders, never session-filtered | grid_launcher/tv/bridge/cloud_helpers.py:269 |
| Upload jobs | Grouping, archiving, screenshots, slots, `overwrite` | One raw file per request, always in the `saveFile` field even for states, query `rom_id` + `emulator` only | grid_launcher/tv/bridge/cloud_helpers.py:293 |
| Retention pruning | Yes, keep 3 per slot | None | — |
| Block reasons / scope | Enforced | Not consulted at all | — |
| Sync state | Read and written | Never read or written | — |
| Record listing | Per-slot for shared scopes, latest-only for per-game; state image records filtered out | Always `latest_server_records_by_slot` with an **empty** emulator name, so slots are computed across every emulator's records | grid_launcher/tv/bridge/cloud_backend.py:49 |
| Restore target | `preferred_restore_target_path` over resolved sync directories and local candidates | `game["install_dir"]`, else the parent of `game["local_path"]`; refuses with "Cannot determine save location. Use Desktop Mode to restore." | grid_launcher/tv/bridge/cloud_backend.py:223 |
| Restore record | The chosen server record | A synthetic `{"file_name": <game name>, "slot": ""}` with an empty candidate list | grid_launcher/tv/bridge/cloud_backend.py:241 |
| State restore | Supported | `restoreSlot` always hits `/api/saves/{id}/content` regardless of `save_type` | grid_launcher/tv/bridge/cloud_backend.py:221 |
| Delete | `/api/saves/delete` or `/api/states/delete` by mode | Same two endpoints; an unrecognised `save_type` reports "Unknown save type." | grid_launcher/tv/bridge/cloud_backend.py:180 |
| Auto restore on launch | `_auto_sync_before_launch`, gated on block reasons and `skip_*` flags | `_TvAutoRestoreWorker`, gated only on `auto_cloud_save_download_on_launch` and credentials; falls back to install dir when no sync directories resolve; no local-newer or known-latest checks | grid_launcher/tv/bridge/cloud_helpers.py:307, grid_launcher/tv/bridge/game_backend.py:318 |
| Auto upload on exit | Yes | Not implemented | — |
| Base URL | `server_base_url(config)` | `server_base_url(config)` in `cloud_backend`, but a raw `config["server_url"].rstrip("/")` in `cloud_helpers` | grid_launcher/tv/bridge/cloud_backend.py:44, grid_launcher/tv/bridge/cloud_helpers.py:284 |

`game_save_match_tokens` and `is_state_file_candidate` are duplicated verbatim in
`cloud_helpers` rather than imported (grid_launcher/tv/bridge/cloud_helpers.py:61,
grid_launcher/tv/bridge/cloud_helpers.py:126); a port should implement them once. The TV
signal surface is `slotsLoaded`, `slotsError`, `restoreComplete`, `deleteComplete`,
`uploadComplete`, each carrying a small dict
(grid_launcher/tv/bridge/cloud_backend.py:107).

## Test oracle

`tests/test_cloud_transfer.py` (889 lines) — the largest oracle.

- `resolve_native_save_dir`: plain expansion with no Shell Documents; no-redirection case;
  redirected Documents; non-Documents paths unaffected (lines 30, 45, 62, 82).
- `normalize_manual_save_path`: `%APPDATA%`, `%LOCALAPPDATA%`, LocalLow, Documents, other
  `%USERPROFILE%` subpaths, unrecognised paths unchanged, forward slashes normalised
  (lines 98–188).
- `zip_directory_for_upload` skips OS metadata files (line 203).
- `zip_native_save_dirs_for_upload`: unreadable directory skipped and omitted from the
  manifest; locked file skipped; all-directories-fail yields zero files and an empty manifest
  (lines 224, 256, 288).
- `ppsspp_state_upload_jobs` attaches only supported image sidecars (line 315).
- `appended_image_sidecar_path` finds `<full name>.png` (line 333).
- `screenshot_download_candidate_paths` ordering, blank/missing keys, empty record
  (lines 354, 363, 371).
- `retroarch_state_upload_jobs`: appended PNG sidecar attached, omitted when absent, one job
  per slot, non-image sidecars ignored, sidecar lives in the files payload, two-tuple shape
  (lines 376–452).
- `grouped_file_upload_jobs`: multiple files with the same stem archive into one upload;
  distinct state slots stay separate (lines 463, 490).
- `cloud_sync_candidates_for_game`: explicit file paths from profiles accepted; state
  candidates filtered to the matching ROM name; only common name variants allowed
  (lines 512, 565, 595).
- `cemu_save_directories_for_game` selects nested `user/` folders (line 532).
- `TestSessionScreenshotPath` (line 631): `None` for no directories and for a `None` window;
  `None` when nothing is in-window; picks the in-window image; picks the most recent of
  several; ignores non-images; recurses into subdirectories; skips blocked basenames;
  tolerates missing directories; supports jpg/webp/bmp.
- `WinePrefixPathTranslationTests` (line 734) and `TranslateWindowsPathToWinePrefixTests`
  (line 824) pin the prefix translation table.

`tests/test_cloud_restore.py` — `relative_timestamp_text` buckets (line 21);
`sort_server_records_by_recency` newest timestamp then id (line 28);
`latest_server_records_by_slot` keeps the newest per slot (line 39);
`restore_single_save_payload` prefers an exact candidate filename (line 51); Redream numeric
savestate name matching and the latest-hash-group fallback (lines 74, 91);
`restore_single_state_payload` keeps a nested candidate folder, writes / omits / re-extends
the screenshot sidecar, never writes a sidecar for a zip payload, and unpacks a zip into the
matching directory (lines 123–244).

`tests/test_cloud_save_block_reason.py` — one test per branch of
`cloud_save_block_reason_for_game`: state blocked when `supports_save_states` is false;
state blocked when `cloud_sync_safe` is false; safe core not blocked; save blocked when
`supports_saves` is false; safe save core not blocked; flags ignored for non-RetroArch;
flags ignored when `None`; native platform blocked regardless of flags (lines 38–117).

`tests/test_cloud_state_filter.py` — `_server_state_records_for_rom` drops image-named
records (line 35); `is_state_file_candidate` rejects image sidecars and accepts
`.state`/slot files, DuckStation slot files, and `.<n>.sav` files (lines 50–71);
`_state_candidate_base_variants` strips DuckStation and PCSX2 `.p2s` naming (lines 78, 110);
`_state_candidate_hash_group_key` handles DuckStation naming (line 87);
`_state_candidate_matches_game_tokens` for PCSX2 `.p2s` (line 120).

`tests/test_details_cloud_native_panel.py` — the per-row Restore button is enabled for
`native_multi_dir` records without any emulator lookup (line 100);
`_refresh_native_save_panel` starts the cloud records worker and renders the "Cloud Saves"
section (line 113); `_on_details_cloud_records_loaded` re-adds the native path section
**before** the record rows (line 135).

`tests/test_tv_cloud_backend.py` — config storage (line 31); `loadSlotsForGame` error paths
for missing credentials and missing ROM id (lines 35, 55); `_SlotFetchWorker` success, empty
result, and API error (lines 72, 119, 138); `deleteSlot` success and failure (lines 157,
176), and the state endpoint variant (line 461); `restoreSlot` without an install dir, the
success path, and the `local_path`-parent fallback (lines 197, 220, 476); `uploadSave`
credential gate, thread start, previous-thread cancellation (lines 257, 274, 285);
`_CloudUploadWorker` success, no-emulator failure, no-files failure, partial upload
(lines 321–406).

Related coverage outside the cloud files: `tests/test_flycast_vmu.py` pins
`flycast_vmu_file_candidates` and the `vmu_shared_saves` flag;
`tests/test_openapi_contract.py` pins the endpoint shapes used here.

## Open questions

- `OPEN QUESTION:` `self._authorized_headers()` is called in two places
  (grid_launcher/ui/mixins/cloud_mixin.py:1786, grid_launcher/ui/mixins/cloud_mixin.py:1814)
  but is defined nowhere in the repository, and `AttributeError` is not caught by the
  surrounding handler. Should a port implement absolute-URL state/screenshot downloads with
  bearer auth, or drop that branch and always treat candidates as server-relative?
  **RULED (milestone 6): dropped — see "Rust port deviations (milestone 6)" D4.**
- `OPEN QUESTION:` The retention limit is the hard-coded constant `3`
  (grid-launcher.py:2224) with no config key and no UI. Is it intended to become
  user-configurable, and should a port expose it?
  **RULED (milestone 6): made configurable — see "Rust port deviations (milestone 6)" D7.**
- `OPEN QUESTION:` Retention pruning never runs for states, so state records accumulate
  without bound. Intended, or an omission?
  **RULED (milestone 6): ported as-is, unbounded — see "Rust port deviations (milestone 6)"
  D7 and the follow-the-code rulings.**
- `OPEN QUESTION:` `partition_active_game_sessions` silently drops any session whose
  `process` lacks a callable `poll` (grid_launcher/library/cloud_sync.py:121) — it appears in
  neither the remaining nor the finished list, so no auto upload ever fires for it. Should a
  port treat such a session as finished instead?
  **RULED (milestone 6): moot by construction, and if it ever mattered, finished — see "Rust
  port deviations (milestone 6)" D8.**
- `OPEN QUESTION:` `_ppsspp_save_directories_for_game`
  (grid_launcher/ui/mixins/cloud_mixin.py:1427) sorts by the directory's own mtime and never
  applies the ignore sets, unlike every other folder scanner which sorts by the newest file
  beneath and honours the ignore sets. Deliberate, or should it be unified?
  **RULED (milestone 6): ported as-is — see "Rust port deviations (milestone 6)",
  follow-the-code rulings.**
- `OPEN QUESTION:` `_rpcs3_save_directories_for_game` sorts by configured-directory index
  before mtime (grid_launcher/ui/mixins/cloud_mixin.py:1192), so a stale save in an earlier
  directory outranks a newer one in a later directory. Is directory precedence intended to
  dominate recency here?
  **RULED (milestone 6): ported as-is — see "Rust port deviations (milestone 6)",
  follow-the-code rulings. The same function's `self._ps3_game_ids_for_game(game)` call
  (cloud_mixin.py:1178) is ALSO undefined anywhere in the repository — a second,
  independent latent defect in this one method. See D10.**
- `OPEN QUESTION:` `_cloud_save_block_reason_for_game` only computes RetroArch core flags
  when a default core is configured for the platform, while `_cloud_save_scope_for_game`
  additionally falls back to `retroarch_core_flags_for_platform`
  (grid_launcher/ui/mixins/cloud_mixin.py:116 vs grid_launcher/ui/mixins/cloud_mixin.py:162).
  A game can therefore be treated as `shared-slotted` for scope purposes while the
  `supports_saves` block check never runs. Should the two use the same fallback?
  **RULED (milestone 6): ported as-is, asymmetry included — see "Rust port deviations
  (milestone 6)", follow-the-code rulings.**
- `OPEN QUESTION:` The known-latest short circuit is skipped for shared save scopes
  (grid_launcher/ui/mixins/cloud_mixin.py:2001), so shared xemu/Redream/VMU saves are
  re-downloaded on every launch even when nothing changed. Is that the intent?
  **RULED (milestone 6): ported as-is — see "Rust port deviations (milestone 6)",
  follow-the-code rulings.**
- `OPEN QUESTION:` `_cloud_emulator_entry_cache` is keyed on
  `"<title>::<platform>::<save_type>"` (grid_launcher/ui/mixins/cloud_mixin.py:191), which
  omits the ROM id. Two games with the same title and platform but different ROM ids share a
  cache entry. Acceptable, or should the key use the identity function?
  **RULED (milestone 6): ported as-is — see "Rust port deviations (milestone 6)",
  follow-the-code rulings.**
- `OPEN QUESTION:` `_shared_cloud_sync_owner_game` identifies the "owner" by searching for
  the literal substrings `xemu` / `redream` in a game's title, platform, description, and ROM
  filename (grid_launcher/ui/mixins/cloud_mixin.py:392). Is a substring match on free text
  the intended contract, or should this key off an explicit field?
  **RULED (milestone 6): ported as-is, gated on the `Emulators` platform — see "Rust port
  deviations (milestone 6)", follow-the-code rulings. The install-path last resort
  (`_matching_installed_emulator_games`) this function also falls back to is deferred — see
  "Other recorded deviations and gaps".**
- `OPEN QUESTION:` `filter_upload_jobs_by_session_window` accepts a job when **any** payload
  path is in-window (grid_launcher/library/cloud_transfer.py:677). A stale state file with a
  freshly written screenshot sidecar therefore uploads. Should the state file itself be
  required to be in-window?
  **RULED (milestone 6): ported as-is — see "Rust port deviations (milestone 6)",
  follow-the-code rulings.**
- `OPEN QUESTION:` The TV backend's `restoreSlot` always downloads from
  `/api/saves/{id}/content` even when `save_type == "state"`
  (grid_launcher/tv/bridge/cloud_backend.py:221), and passes a synthetic record whose
  `file_name` is the game's display name. Is state restore expected to work in TV mode at
  all, or should the UI hide it?
  **DEFERRED (milestone 6): TV-mode cloud saves are out of scope for this milestone — see
  "Rust port deviations (milestone 6)", "Other recorded deviations and gaps", and doc 09.**
- `OPEN QUESTION:` The TV upload always sends the `saveFile` multipart field, even when
  posting to `/api/states` (grid_launcher/tv/bridge/cloud_helpers.py:297), which the OpenAPI
  schema declares requires `stateFile`. Is TV state upload expected to work?
  **DEFERRED (milestone 6): see the note on the previous question.**
- `OPEN QUESTION:` `perform_tv_save_upload` reads the whole candidate into memory and
  discards the result before uploading (grid_launcher/tv/bridge/cloud_helpers.py:292),
  apparently as an existence/readability probe. Should a port keep that probe or drop it?
  **DEFERRED (milestone 6): see the note two questions up.**
- `OPEN QUESTION:` There is no cap on concurrent auto-upload threads and no per-game
  de-duplication (grid_launcher/ui/mixins/cloud_mixin.py:2950). Rapid launch/exit cycles can
  overlap uploads for the same game. Should a port serialise them?
  **RULED (milestone 6): yes — see "Rust port deviations (milestone 6)" D5.**
- `OPEN QUESTION:` A failed restore in the multi-record (shared-slotted) loop aborts after
  earlier records were already written to disk
  (grid_launcher/ui/mixins/cloud_mixin.py:2092), leaving a partially restored slot set. Is
  partial application acceptable, or should a port stage to a temp directory and commit
  atomically?
  **RULED (milestone 6): stage and commit-on-all-success — see "Rust port deviations
  (milestone 6)" D6.**

## Source map

| Path | Role |
|---|---|
| grid_launcher/library/cloud_sync.py | Pure sync-state normalisation/keying, session partitioning, `auto_cloud_upload_plan`, upload-result summarisation, session window computation and mtime filtering, save/state/folder candidate discovery including Cemu and the state-name variant machinery |
| grid_launcher/library/cloud_transfer.py | Ignore-name defaults, image sidecar lookup (suffix-replace and suffix-append), session screenshot selection, URL normalisation, state/screenshot download-candidate extraction, zip extraction with 7-Zip fallback, all three archive writers, native path resolution and manual-path normalisation, PPSSPP/RetroArch job builders, job-level session filtering, temp cleanup, `should_skip_known_latest`, `is_local_newer_than_server` |
| grid_launcher/library/cloud_upload.py | `file_upload_jobs`, `directory_archive_upload_jobs`, and the two user-facing message builders |
| grid_launcher/library/cloud_restore.py | Record timestamp parsing, relative time text, recency sorting, payload→record normalisation, latest-record and latest-per-slot selection, restore target selection, single save/state payload placement |
| grid_launcher/library/downloads.py | `format_size` only, used for the cloud record row subtitle (see doc 03) |
| grid_launcher/library/identity.py | `game_key`, `rom_id_key`, `games_match_identity`, `installed_game_record` — used for sync-state keys and session matching |
| grid_launcher/emulator/selection.py | `cloud_save_scope_for_game`, `cloud_save_block_reason_for_game`, `is_native_executable_platform`, `is_emulators_platform` |
| grid_launcher/emulator/profiles.py | `resolved_save_strategy_for_emulator`, `resolved_ignore_basenames_for_emulator`, `resolved_ignore_extensions_for_emulator` |
| grid_launcher/emulator/retroarch.py | `retroarch_core_flags` (`supports_saves`, `supports_save_states`, `cloud_sync_safe`, `vmu_shared_saves`), `retroarch_directory_settings`, `flycast_vmu_file_candidates` |
| grid_launcher/ui/mixins/cloud_mixin.py | Desktop orchestration: block-reason/scope wrappers, cloud emulator resolution and caching, sync-directory resolution with token expansion, target dispatch, per-emulator folder scanners, token builders, server record fetch/filter, retention pruning, downloads, restore (emulator + native), upload (emulator + native), manual actions, auto-sync triggers, auto-upload worker lifecycle |
| grid_launcher/ui/mixins/details_view_mixin.py | `cloud_sync_state` accessors and persistence, cloud panel mode toggling and rendering, record rows, restore-enabled evaluation with per-request memoisation, restore/delete confirmations, `_delete_cloud_record`, native save path list UI, `_resolve_rom_id_for_game` |
| grid_launcher/background/workers.py | `AutoCloudSaveUploadWorker` (per-type sequential upload, error→result conversion), `DetailsCloudRecordsWorker` (record fetch with request id) |
| grid_launcher/tv/bridge/cloud_backend.py | TV `CloudBackend` QObject: slot listing, delete, restore, upload, thread lifecycle, signal payloads |
| grid_launcher/tv/bridge/cloud_helpers.py | TV-side duplicated token/state predicates, simplified emulator and directory resolution, `perform_tv_save_upload`, `_TvAutoRestoreWorker` |
| grid_launcher/tv/bridge/game_backend.py | Calls `_TvAutoRestoreWorker` before launch and defers the spawn until it finishes |
| grid-launcher.py | Config accessors (`auto_cloud_save_*`, retention constant), ignore-set and save-strategy wrappers, `_session_window_for_state_upload`, session poll timer, cache invalidation on config save, API client wrappers |
| openapi.json | Contract for `/api/saves`, `/api/states`, their `/content`, `/delete`, and the `SaveSchema` / `StateSchema` / `ScreenshotSchema` record shapes |

## Rust port deviations (milestone 6)

Deliberate deviations, and rulings on ambiguous or defective reference behavior, made while
porting cloud save/state sync (candidate discovery, upload/restore/retention, native saves,
xemu raw-disk sync, auto-sync triggers, and the desktop panel) to Rust (grid-core, the Tauri
`cloud_service`/`commands/cloud.rs` layer, and the `app/src/lib/details/` panel). Rust paths
are relative to `rewrite/`. D1-D3 (xemu raw-disk sync) were already declared by the xemu
design task and are restated here for completeness; D4-D13 are new to this milestone's
review.

1. **D1 — xemu raw-disk sync replaces whole-image sync, with no fallback.** The reference
   synced the whole `xbox_hdd.qcow2`/`.img` file as one archive; the rewrite ships no qcow2
   decoder and instead reads/writes the `E:` (data) FATX partition directly inside a raw HDD
   image (`crates/grid-core/src/cloud/xemu_sync.rs:1-13`). A configured image that isn't
   usable for this blocks the save panel's actions with one of three reasons —
   `xemu-image-not-raw`, `xemu-image-unsupported-layout`, `xemu-image-missing` — computed by
   `classify_hdd_image`/`block_reason_for_status`
   (`crates/grid-core/src/cloud/xemu_sync.rs:34-101`).
2. **D2 — legacy whole-image records are skipped with a notice, but still count toward
   retention.** A server record from the old whole-image sync cannot be restored by the
   raw-disk path; `inject_xemu_save_archive` reports the fixed notice `LEGACY_RECORD_NOTICE`
   (`crates/grid-core/src/cloud/xemu_sync.rs:30-32`) rather than attempting it, but the record
   itself is not specially excluded from `prune_server_save_records`'s count — it can still be
   the one a retention prune deletes.
3. **D3 — autoconfig accepts `xbox_hdd.img`, preferred over `.qcow2`.** The BIOS/firmware
   probe and the `sys.files.hdd_path` default both treat `xbox_hdd.img` as satisfying the HDD
   slot when it exists, falling back to `xbox_hdd.qcow2` only when neither is present
   (`crates/grid-core/src/autoconfig/xemu.rs:19-44`, `:102-118`, `:276-292`) — add-only
   semantics never downgrade an existing qcow2 setup.
4. **D4 — the `_authorized_headers` branch is dropped; absolute candidates are skipped.**
   `self._authorized_headers()` is called twice in the reference (a state/screenshot download
   helper) but defined nowhere in the repository — a live `AttributeError` the moment an
   absolute-URL download candidate is actually reached. The port has no equivalent method to
   port: `RommClient::get_relative_bytes` rejects an `http(s)://` candidate outright with
   `Err(RommError::InvalidUrl)` rather than fetching it
   (`crates/grid-core/src/romm/cloud.rs:136-164`), so only server-relative candidates are ever
   downloaded.
5. **D5 — auto uploads are serialized per game, with a pool cap of 2, coalesced when already
   in flight.** `AutoUploadPool` (`app/src-tauri/src/cloud_service.rs:58-124`,
   `MAX_CONCURRENT_AUTO_UPLOADS = 2`) tracks one in-flight key per game; a second exit for the
   SAME game while its upload is still running is coalesced into the first rather than
   starting a second, while two DIFFERENT games' auto uploads run concurrently up to the
   semaphore's cap of 2. The reference has no cap and no de-duplication at all
   (`cloud_mixin.py:2950`).
6. **D6 — shared-slotted restores are staged in a temp directory and committed only when
   every record downloaded and unpacked successfully.** `place_staged`
   (`crates/grid-core/src/cloud/ops/restore.rs:353-419`) downloads and unpacks EVERY record
   into a staging temp directory first; only once all succeed are the staged trees copied into
   place. The commit-phase copy itself is a plain per-file `copy_tree`, not an atomic rename —
   so a crash mid-commit can still leave a partially-applied slot set, though the plain
   download/unpack failure the reference left exposed no longer can.
7. **D7 — `cloud_save_retention_limit` is a config key (default 3, minimum 1); states are
   still never pruned.** The reference hardcodes the retention limit to the literal `3`
   (`grid-launcher.py:2224`); the port exposes it as `Config::cloud_save_retention_limit`
   (`crates/grid-core/src/config.rs:101-109`, default 3), editable from Settings › Cloud saves
   (`app/src/lib/settings/CloudSavesPage.svelte`,
   `cloud-settings-retention-limit`). Every read site clamps to a minimum of 1 (`.max(1)`,
   `crates/grid-core/src/cloud/retention.rs:76`,
   `crates/grid-core/src/cloud/ops/upload.rs:200-201`). Retention pruning still runs for saves
   only — the reference never prunes state records either, and the port does not add that.
8. **D8 — an unpollable session would count as finished; the Rust session store always
   polls.** `partition_active_game_sessions` silently drops a session whose `process` has no
   callable `poll` from BOTH the running and finished lists (`cloud_sync.py:121`), so no auto
   upload ever fires for it. The Rust `LaunchService`/`SessionManager` session model has no
   such degenerate state — every tracked session always owns a real child-process handle it
   can poll — so this branch has no equivalent to port; see the design note at
   `crates/grid-core/src/cloud/window.rs:1-10`. Were a session ever unrepresentable, the
   port's shape naturally treats "cannot be tracked further" as "finished", not "silently
   forgotten".
9. **D9 — four credential-bearing basenames are always-ignored, on both the archive-write and
   scan sides.** `retroarch.cfg`, `pcsx2.ini`, `ppsspp.ini`, `ppsspp_retroachievements.dat`
   are added to `DEFAULT_IGNORE_BASENAMES` (`crates/grid-core/src/cloud/candidates.rs:47-60`)
   — absent from the reference's `DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES`
   (`cloud_transfer.py:19-24`). Token secrecy outranks parity here (standing project rule): a
   save path pointed at an emulator's own config root must never upload a file that can hold a
   RetroAchievements or session token, whether it is picked up by a directory-archive scan or
   excluded explicitly when building an upload archive.
10. **D10 — `_ps3_game_ids_for_game` is undefined; PS3 ids are reconstructed from
    `ps3_game_id`.** `_rpcs3_save_directories_for_game` calls
    `self._ps3_game_ids_for_game(game)` (`cloud_mixin.py:1178`) — a method defined nowhere in
    the repository, and not caught by any surrounding handler, so RPCS3 save-directory
    scanning would crash the moment it actually ran; no test exercises it either. The only
    PS3-id data `CloudGame` carries is `ps3_game_id`, already produced in the exact normalized
    form (`^[A-Z]{4}\d{5}$`, no separators) the call site's substring match needs, so
    `ps3_id_tokens` (`crates/grid-core/src/cloud/tokens.rs:281-307`) reconstructs the missing
    method as "that one field, defensively re-normalized, as a single-element list, or empty
    when blank." Flagged as a discrepancy for human confirmation rather than silently assumed.

11. **D11 — a running xemu blocks its own save sync.** Under D1 the port writes save data
    into the xemu HDD image IN PLACE (`FatxPartition::write_tree`, reached from
    `crates/grid-core/src/cloud/ops/restore.rs` and `.../cloud/xemu_sync.rs`), where the
    reference replaced the whole image file. An in-place FATX write to an image xemu still
    holds open can cross-link clusters and corrupt the filesystem — a hazard the reference's
    whole-file replace did not have. `block_reason_for_game`
    (`crates/grid-core/src/cloud/ops/mod.rs`) therefore returns a new, user-facing block
    reason when any active session resolves to the same xemu emulator entry:

    > `xemu is running — close it before syncing its saves.`

    It gates BOTH upload and restore (it is an action block reason, not a panel-visibility
    one — see the Ruling B split), applies to saves only (xemu has no state flow), and is
    checked BEFORE the image-status reasons, since it is the transient, immediately
    actionable one and must fire before anything opens the image. It has no Python original.
    Auto-upload after exit is unaffected: that path passes no active sessions, because the
    game's own session has already ended.

12. **D12 — native save-path removals are persisted, not session-only.** Python's
    `_pcgw_remove_path_for_game` (`details_view_mixin.py:1224-1235`) edits only the in-memory
    `_pcgw_paths_cache` for a PCGW row, so the row reappears after the next PCGamingWiki lookup.
    The rewrite records the removal in `Config::native_removed_save_paths`
    (`crates/grid-core/src/config.rs`) and filters it out of every read through the one shared
    `visible_native_paths` helper (`crates/grid-core/src/cloud/native.rs`), used by both the
    panel's list command and the upload/restore paths (`crates/grid-core/src/cloud/ops/native.rs`).
    Reason: a removal the user made deliberately must survive a restart. The suppression is keyed
    on the row's raw string (its literal `%APPDATA%\…`-style value, not a resolved path), so it
    clears only when that same raw string is re-added: retyping it into the manual field works,
    but Browse writes back an absolute resolved directory and adds a new row instead of clearing
    the old one. Nothing is unrecoverable either way — the suppressed row is still listed nowhere
    but is not deleted from the underlying save-path source, so re-adding it by hand always works.
13. **D13 — native save-path row tooltips use `resolve_native_save_dir`, not `expandvars`.**
    Python's tooltip is `os.path.expandvars(raw)` (`details_view_mixin.py:1097`), which on Linux
    leaves a `%APPDATA%` path unchanged. The rewrite resolves it through the same
    `resolve_native_save_dir(raw, None, wine_prefix)` the upload and restore paths use
    (`app/src-tauri/src/cloud_service.rs`), so the tooltip names the directory that would really
    be read. Related fix, not a deviation: `CloudContext.wine_prefix` was hardcoded `None` at
    both construction sites, so native upload/restore on Linux never translated
    `%APPDATA%`/`%LOCALAPPDATA%`/`%USERPROFILE%` into the wine prefix despite
    `crates/grid-core/src/cloud/ops/native.rs:118,212,272,276` consuming it correctly. The prefix
    is now threaded from the matching registry row's `native_wineprefix`
    (`crates/grid-core/src/library/registry.rs`, resolved by
    `app/src-tauri/src/cloud_service.rs`'s `wine_prefix_from`/`wine_prefix_for`).

### Follow-the-code rulings (ported as-is)

Where the reference's own behavior conflicts with this doc's prose, or is internally
inconsistent, the port follows the CODE. None of the following is a bug the port introduces,
and none of them is fixed:

- **PPSSPP scanner: own-mtime sort, no ignore-set filtering.** `ppsspp_save_directories`
  (`crates/grid-core/src/cloud/candidates.rs:794-830`) sorts candidate directories by their
  OWN mtime and applies no ignore set at all — unlike every other folder scanner, which sorts
  by the newest file beneath and honours the ignore sets.
- **RPCS3 scanner: directory index before recency.** `rpcs3_save_directories`
  (`crates/grid-core/src/cloud/candidates.rs:756-793`) sorts by configured-directory index
  FIRST, then recency — a stale save in an earlier configured directory outranks a newer one
  in a later directory.
- **Block-vs-scope RetroArch core-flags fallback asymmetry.** `block_reason_flags`
  (`crates/grid-core/src/cloud/ops/mod.rs:454-469`) computes core flags ONLY when a default
  core is configured for the platform, with no `core_flags_for_platform` fallback — unlike
  `cloud_save_scope`'s own resolution, which does fall back. A game can be treated as
  `shared-slotted` for scope purposes while the `supports_saves` block check never runs.
- **Shared-scope re-download.** The known-latest short circuit
  (`crates/grid-core/src/cloud/ops/restore.rs`, pinned by the
  `known_latest_skip_only_for_per_game_scope` test at
  `crates/grid-core/src/cloud/ops/tests.rs:514`) applies to `per-game` scope ONLY — shared
  xemu/Redream/VMU saves are re-downloaded on every auto-restore even when nothing changed
  server-side.
- **Cache key without ROM id.** `CloudCaches`'s sync-dir and emulator-entry memoization keys
  on `(title, platform, save_type)` (`crates/grid-core/src/cloud/ops/mod.rs:129-137`,
  `resolved_cloud_emulator_pair` at `:316`) — two games with the same title and platform but
  different ROM ids share a cache entry.
- **Substring owner match, gated on the `Emulators` platform.** `shared_sync_owner`
  (`crates/grid-core/src/cloud/scope.rs:221`) and its caller `shared_cloud_sync_owner`
  (`crates/grid-core/src/cloud/ops/mod.rs:645-661`) identify a shared-sync "owner" game by a
  plain substring search for `xemu`/`redream` across title, platform, description and ROM
  filename — free-text matching, not an explicit field — restricted to games on the literal
  `Emulators` platform, matching the reference's own first gate.
- **Any-payload-path-in-window jobs.** `filter_upload_jobs_by_session_window`
  (`crates/grid-core/src/cloud/transfer.rs:752-800`) accepts a job when ANY of its payload
  paths is inside the session mtime window — a stale state file with a freshly-written
  screenshot sidecar still uploads.
- **States are unpruned.** (See D7.) `upload_cloud_files_for_game` only calls
  `prune_server_save_records` for `SaveType::Save`
  (`crates/grid-core/src/cloud/ops/upload.rs:200-205`) — never for states, matching the
  reference exactly.
- **`relative_timestamp_text` bucket bug, ported as-is.**
  `crates/grid-core/src/cloud/restore.rs:321-345` reproduces the reference's
  threshold-ordering bug verbatim: the range table is ordered largest-threshold-first, so the
  "minute" bucket is unreachable above 90 seconds and 120 seconds elapsed renders `"1 hour
  ago"`.
- **PCSX2 scanner's sort key is UNFILTERED by ignore sets.** `pcsx2_save_directories`
  (`crates/grid-core/src/cloud/candidates.rs:699-704`) accepts an `IgnoreSets` parameter but
  builds its sort key against a fresh `IgnoreSets::default()` instead — a directory whose only
  contents match the ignore set can still outrank one with real, older save data, because the
  sort key never sees the caller's ignore set.
- **The empty-token guard asymmetry in save matching.** `state_candidate_matches_tokens`
  (`crates/grid-core/src/cloud/tokens.rs:426-450`) treats an EMPTY token set as "matches
  everything" — deliberate (a game with no derivable id tokens must not be excluded from every
  candidate), ported exactly as the reference computes it.
- **Zero-job state uploads still emit "Uploaded 0 save states."** `upload_completion_message`
  (`crates/grid-core/src/cloud/transfer.rs:850-908`) reports the literal `"Uploaded 0 save
  states."` (or `"...save files."`) for a save type whose job list came back empty, rather
  than a distinct "nothing to upload" message — matching the reference's own general-branch
  text.
- **Native restore selects the single latest record.** A blank emulator name resolves
  `cloud_save_scope` to `per-game`, so `restore_native_cloud_save_for_game`
  (`crates/grid-core/src/cloud/ops/native.rs:184-205`) restores exactly ONE record — the
  latest — never one per slot. This doc's "Restore — native games" section previously said
  "per-slot"; that sentence is now fixed in place to match the code.

### Other recorded deviations and gaps

- **`cloud_sync_state`'s debug `failed=` segment is a plain comma-joined list, not Python's
  list-`repr`.** `summarize_auto_cloud_upload_result`'s debug segment
  (`crates/grid-core/src/cloud/state.rs:358-406`) renders `"failed=<first 3 joined by ','>"`
  — no brackets, no quotes — where the reference's f-string embeds the actual Python `list`
  (`"failed=['a.sav', 'b.sav']"`). Debug-only text; nothing user-facing depends on the exact
  punctuation.
- **`_ensure_emulator_sync_settings` is NOT invoked during cloud directory resolution.**
  `resolved_sync_dirs`/`resolved_sync_directory_paths`
  (`crates/grid-core/src/cloud/dirs.rs:51`, `:353-358`) deliberately omit the reference's
  `_ensure_emulator_sync_settings` call (`cloud_mixin.py:646`) — milestone 5's D1 restricts
  every `ensure_*` writer to running only for a NEWLY created emulator entry, and directory
  resolution for an existing entry is not that trigger.
- **`resolve_native_save_dir` expands `%VAR%` unconditionally.**
  `crates/grid-core/src/cloud/native.rs:19-58` substitutes `%VAR%`-syntax environment
  references on every host, not just Windows — the reference's real `os.path.expandvars` only
  understands `%VAR%` on Windows and leaves that text untouched on POSIX (where it expands
  only `$VAR`/`${VAR}`). The port's own pinned oracle tests require `%VAR%` to resolve
  everywhere; on Linux this is inert in practice, since the values it substitutes are Windows
  environment variable names.
- **`CloudGame.title_id` / `base_title_id` / `ps3_game_id` are blank until the registry
  carries them.** `cloud_game_from_installed`
  (`app/src-tauri/src/cloud_service.rs:1155-1167`) leaves all three fields empty — the
  `installed_games` registry has no columns for them yet. RPCS3 and Cemu scanners, whose
  token sets are built from these fields, therefore match EVERY candidate directory (an empty
  token set matches everything — see the follow-the-code ruling above) rather than narrowing
  by id, until a future milestone adds the columns and populates them.
- **The shared-owner install-path last resort is deferred.** The reference's
  `_shared_cloud_sync_owner_game` falls back to `_matching_installed_emulator_games`
  (`install_mixin.py:1106` -> `install_registry.py:65`) when no title/description/filename
  substring match is found; the port's `shared_cloud_sync_owner`
  (`crates/grid-core/src/cloud/ops/mod.rs:645-661`) has no equivalent fallback yet — it needs
  `candidate_archive_paths_for_game`/`candidate_extracted_paths_for_game`/
  `candidate_extracted_dirs_for_game`, none built by this milestone.
- **The PPSSPP and RetroArch state job builders take the already-resolved ignore sets.**
  `ppsspp_state_upload_jobs` (`crates/grid-core/src/cloud/transfer.rs:622`) and
  `retroarch_state_upload_jobs` (`crates/grid-core/src/cloud/transfer.rs:713`) both receive an
  `&IgnoreSets` parameter and apply it, matching the reference, which resolves and applies
  ignore sets identically for both.
- **The xemu image status gates actions and panel text, but not panel visibility.**
  `details_cloud_mode_supported`'s compatibility gate deliberately checks only the BASE block
  reason, not the xemu HDD-image reason (`crates/grid-core/src/cloud/ops/mod.rs:1009-1013`) —
  an unusable xemu image must not hide the Manage Saves panel outright, or the user could
  never see the guidance text explaining why.
- **`_split_template_args` tracks only `{{}}` nesting.** `split_template_args`
  (`crates/grid-core/src/pcgw.rs:187-230`) respects nested `{{...}}` templates when splitting
  a PCGamingWiki wikitext template's `|`-separated arguments, but does not additionally track
  `[[...]]` link nesting — ported exactly as the reference's own splitter behaves.
- **PCSX2's `_pcsx2_superblock` stays in the ignore set for archive filtering.**
  `resolved_ignore_sets` adds the literal basename `_pcsx2_superblock` to the SAVE ignore set
  only for a resolved PCSX2 emulator (`crates/grid-core/src/cloud/candidates.rs:249-291`) — it
  is a directory marker file PCSX2 itself writes, not real save data, and must never be
  treated as the "newest" file when picking a save directory or included in an upload
  archive.
- **The `Fat` empirical oracle (`pyfatx`) was never run.**
  `crates/grid-core/src/fatx/dir.rs:27-31` and `crates/grid-core/src/fatx/layout.rs:314`
  record that the intended cross-check against a real `pyfatx`-produced image never happened
  (the tool fails to import in this environment) — the epoch-1980 timestamp base, the
  `cluster_count + 1` reserved-FAT-entry convention, and date-then-time field order all remain
  documented defaults rather than empirically confirmed values.
- **TV-mode cloud save variants are deferred, together with doc 09.** The `## TV-mode
  variants` section above, and the three TV-bridge open questions immediately preceding this
  section in "Open questions" (`restoreSlot`'s always-content-endpoint download, the TV
  upload's always-`saveFile` field, `perform_tv_save_upload`'s read-and-discard probe), have
  no Rust TV-mode counterpart in this milestone — TV mode itself has not been ported yet (doc
  09).
