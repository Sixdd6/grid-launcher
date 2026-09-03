# Install Specials, Native Games, and Firmware — Rust Rewrite Design (Milestone 8)

Date: 2026-09-02
Status: draft for user review
Behavior contract: `docs/porting/03-library-install.md` (doc 03) sections
2, 3, 7, 8, 9, 11–14, 16, 18; `docs/porting/04-emulator-launch.md` (doc 04)
sections 9, 10, 11, 12 (compat-tool parts). Python reference:
`grid_launcher/library/{ps3_install,archive_preparation,firmware_install,
install_metadata,install_cleanup}.py`, `grid_launcher/emulator/{xenia,
launch,wine,rpcs3}.py`, `grid_launcher/ui/mixins/{install_mixin,
emulator_ui_mixin,details_view_mixin}.py`.

## Goal

Finish doc 03 in the Rust rewrite: PS3, PS4 and Xbox 360 install specials,
native Windows games (install, update, launch through Wine/Proton, compat
tools), and firmware auto-install. Behavioral parity with the Python app
except for the numbered deviations below.

User rulings (2026-09-02):

- Everything in doc 03 in one milestone, with doc 04's native launch path
  and compat tools pulled in so native games work end to end (option 2).
- RAR archives: bundle a decoder (the `unrar` crate) and extract `.rar` on
  every platform, not only PS3.
- PS3 firmware: drop the Sony direct download; the RomM server is the only
  firmware source (option 2).
- Compat tools: discovery plus managed installs from the catalog (option 1).
- Architecture: extend the install service with an install mode and a
  specials layer (approach A).

## Scope

In scope:

- Install modes `base`, `ps4_content`, `xbox360_content`, `native_update`
  (game jobs) and `compat_tool` (emulator jobs); content file ids by
  category; queue ordering unchanged (single FIFO).
- Registry v3: the remaining record fields as columns.
- Extraction: RAR for every platform; PS3 `.rar` included.
- PS3 install routing, ISO short circuit, `games.yml`, PS3 launch target.
- PS4 title-id detection, `eboot.bin` selection, content apply, Details
  buttons.
- Xbox 360 STFS content apply, automatic update/DLC queueing, Linux gating.
- Native games: install target, `game.json`, prefix, update merge, launch
  (Wine/Proton), executable and compat-tool resolution, per-game settings,
  compat-tool discovery, default picker, managed compat-tool installs.
- Firmware: server firmware routing/writing, RetroArch/Cemu/Dolphin shaping,
  the three triggers, the RPCS3 PUP note and `--installfw` button.
- Uninstall branches for PS3 and native games.
- Details Cancel button (milestone 2 deviation 7).
- Tests: unit, wiremock, vitest, four E2E stage groups. Docs: deviations in
  docs 03 and 04.

Out of scope:

- Update detection and "Update Available" (doc 10).
- Sony direct firmware download (ruled out).
- TV mode (redesign later). Discover.
- Emulator source version checks (milestone 4 deferral stands).
- Windows-only `windows_assets` asset override (milestone 4 scope).

## Authority and porting rules

- Docs 03 and 04 are normative; the Python source wins over the docs on
  any disagreement; the spec's deviations win over both.
- User-facing strings port verbatim (listed under "Strings").
- Follow-the-code quirks are ported as-is unless listed as a deviation.
- Token secrecy (hard requirement): no new `expose_secret()` sites; firmware
  bytes and content downloads go through `RommClient`; nothing logs URLs
  with query strings, headers, or tokens.
- grid-core never imports Tauri.

## Architecture

### grid-core: install modes and dispatch (`library/mod.rs`, `library/queue.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode { Base, Ps4Content, Xbox360Content, NativeUpdate }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind { Update, Dlc }
```

`InstallJob` gains `mode: InstallMode`, `content_kind: Option<ContentKind>`,
and `file_ids: Vec<i64>` (the category's ids; empty means the base
selection). `EmulatorJob` gains `compat_tool: bool` (install into the
managed compat-tool directory instead of `<library>/Emulators`).

`DownloadEntry` gains `mode: InstallMode | "compat_tool" | "firmware"` as a
string field `kind` and a `title` that already carries ` (update)` /
` (dlc)` for content jobs, so the drawer needs no new rendering logic.

**File ids by category** (`library/content.rs`): from `RomDetail.files`,
each file's `category` (trimmed, lower-cased; blank → `game`) maps to its
id list. `RomFile` gains `category: String` (serde default `""`). PS4 and
Xbox 360 share this parser (D5).

**Content job download target:** `<platform dir>/<safe title>-<kind>.zip`
with `?file_ids=<csv>`; queued behind the base install (same FIFO).

**Finalize dispatch** (after extraction, in `finalize_inner`):

| Platform / mode | Path |
|---|---|
| PS3 (`Base`) | `specials::ps3::route(staging, roots, title) -> Ps3Outcome` |
| PS4 (`Base`) | `specials::ps4::detect_title_id(...)`, `select_launch_file` |
| PS4 (`Ps4Content`) | `specials::ps4::apply_content(...)` (no new row) |
| Xbox 360 (`Xbox360Content`) | `specials::xenia::apply_content_archive(...)` (no new row) |
| native (`Base`) | `specials::native::finalize(...)` (game dir, prefix on Linux) |
| native (`NativeUpdate`) | `specials::native::apply_update(...)` (updates the row) |
| everything else | today's path |

Every game finalize ends with `firmware::install_for_game(...)`; its
warnings join the entry's warning text; it never fails the install.

**Completion by mode:** `Base` registers the row, writes `games.yml` for
PS3 (existing writer), queues Xbox 360 update/DLC when categories exist,
runs the emulator autoconfig hook (existing), triggers firmware for a
fresh source emulator; `Ps4Content` updates `ps4_game_id`/`ps4_content` on
the row; `Xbox360Content` marks completed; `NativeUpdate` re-registers the
row.

### grid-core: registry v3 (`library/registry.rs`)

Migration 2→3 (transactional, idempotent via `PRAGMA table_info`) adds
`TEXT NOT NULL DEFAULT ''` columns: `native_executable_path`,
`native_launch_parameters`, `native_compat_tool`, `native_wineprefix`,
`native_game_dir`, `included_dlc`, `ps3_trophy_paths`, `ps3_game_id`,
`ps3_iso_path`, `ps4_game_id`, `ps4_content`, `ra_id`. `InstalledGame`
gains the fields; `ps3_game_id`/`ps4_game_id` are stored upper-cased.
`Registry::update_native_settings(rom_id, exe, params, compat)`,
`update_ps4_content(rom_id, game_id, content_json)`,
`update_record(rom_id, &InstalledGame)` (native update) are added.
`LATEST_USER_VERSION = 3`.

### grid-core: extraction (`library/extract.rs`)

`EXTRACTABLE_SUFFIXES` gains `rar`; a `rar` branch uses the `unrar` crate
(vendored build) streaming each entry through the existing traversal
guard. The should-extract table keeps Python's order (native always;
arcade never; PS3 list; default list) with `rar` in both lists (D1).
Firmware's `.7z`/`.rar` flat copy reuses `extract_archive`.

### grid-core: specials (`library/specials/`)

```
library/specials/mod.rs   re-exports; shared helpers (game-id regexes, merge_tree)
library/specials/ps3.rs   classify / route / synthesize id / Ps3Outcome
library/specials/ps4.rs   title-id normalize+detect, eboot ranking, apply_content, Ps4ContentEntry
library/specials/xenia.rs read_stfs_header, apply_content_file, apply_content_archive
library/specials/native.rs finalize (game dir, prefix), game_json apply, apply_update, executable candidates
```

**PS3** (`ps3.rs`), ported from `ps3_install.py` and
`archive_preparation.py:1186-1229`:

- `classify(entry) -> Ps3Entry` per doc 03 §11 table; entries sorted
  directories-first then case-folded name. Regexes: game id
  `^[A-Z]{4}\d{5}$`, trophy `^NPWR\d{5}$`.
- `Ps3Roots { dev_hdd0: PathBuf, games_root: Option<PathBuf>, data_root: Option<PathBuf> }`.
  `dev_hdd0` from `autoconfig::readers::ps3_vfs_dev_hdd0_path`; `games_root`
  from `ps3_vfs_games_path`; `data_root` from the RPCS3 data-root resolver
  (D4). Missing `dev_hdd0` → `"No PS3 VFS dev_hdd0 path configured for <title>"`.
- `route(staging, roots, title) -> Result<Ps3Outcome, String>`: the routing
  table verbatim (merge copies; `config` → `data_root or dev_hdd0.parent`;
  ISO entries expanded via the extractor's external 7-Zip fallback into a
  temp dir and routed recursively — no 7-Zip binary → that entry fails
  with `"Cannot extract ISO <name>: no 7-Zip binary found"` (D3);
  `nested_hdd0_game` per the table). Game id: first routed game id / bare
  disc synthetic id / ISO result; else scan installed paths skipping NPWR;
  empty → `"No PS3 game ID found in archive for <title>"`. `PARAM.SFO`
  synthesis scans `PS3_GAME/PARAM.SFO` or `PARAM.SFO` bytes for the first
  `[A-Z]{4}\d{5}` pattern; fallback id `PS3_GAME_DISC`.
- ISO-only short circuit: exactly one classified entry and it is an ISO →
  move next to the archive (overwrite), `extracted_path = ps3_iso_path =
  <iso>`, `extracted_dir = ""`, staging deleted, return.
- `Ps3Outcome { game_id, installed_paths, trophy_paths (JSON array text),
  extracted_path, extracted_dir }`; `extracted_path`/`extracted_dir` = the
  installed path whose dir name upper-cases to the id, else
  `<dev_hdd0>/game/<ID>`. Routing `io::Error` → `"Failed to install PS3 game
  <title>: <error>"`. Staging deleted after routing.
- Launch: `%ps3_launch_target%` (doc 04 §5) resolves from the row: the
  `ps3_iso_path` when non-empty, else the literal string
  `%RPCS3_GAMEID%:<ps3_game_id>` (the prefix is NOT expanded by the
  launcher; RPCS3 consumes it), else `""` (which keeps the existing
  validation error). Closes doc 04 deviation 3.

**PS4** (`ps4.rs`), ported from `archive_preparation.py:54-265,696-777`:

- `normalize_title_id(&str) -> Option<String>`: strip non-alphanumerics,
  upper-case, require `^[A-Z]{4}\d{5}$`.
- `detect_title_id(launch_rel_path, extraction_root, archive_stem)` in the
  order: launch path segments (excluding file name), first top-level dir,
  launch-file parents up to root, archive stem.
- `select_launch_file(candidates)` — `eboot.bin` (case-insensitive) only;
  sort `(not in a top-level title-id dir, depth, casefold path)`; none →
  generic selector.
- `apply_content(record, archive, kind, extractor) -> Result<Ps4Applied, String>`:
  the ten steps with these exact strings: `"PS4 content apply is only
  supported for PS4 games"`, `"Installed PS4 game is missing a detectable
  title ID"`, `"PS4 content archive must include a title-ID root folder"`,
  `"PS4 content title ID mismatch: expected <id>, archive contains <ids or
  'unknown'>"`, `"Failed to merge PS4 content into installed game: <error>"`,
  warning `"Applied PS4 content, but could not delete archive:\n<path>\n<error>"`;
  the directory checks with their exact messages: `"Installed PS4 game is
  missing an extracted install directory"`, `"Installed PS4 directory does
  not exist: <path>"`, `"Installed PS4 title directory was not found:
  <path>"`.
  Appends `{kind, title_id, archive_name, applied_at}` to the `ps4_content`
  JSON array (lenient parse of the existing text). Staging removed on every
  exit.

**Xbox 360** (`xenia.rs`), ported from `emulator/xenia.py:1-95` and
`archive_preparation.py:781-830`:

- `read_stfs_header(path) -> Option<(title_id_hex8, content_type_hex8)>`:
  first 0x368 bytes; magic in {`CON `, `LIVE`, `PIRS`}; big-endian u32 at
  0x344 (content type) and 0x360 (title id), formatted `{:08X}`.
- `apply_content_file(file, content_root, expected_title_id) -> Result<XeniaApplied, String>`
  with `"Content file not found: <path>"`, `"File does not appear to be an
  STFS package (bad magic)"`, `"Title ID mismatch: expected <ID>, archive
  contains <id>"`; destination `<root>/0000000000000000/<TitleID>/<ContentType>/<name>`,
  copy preserving metadata.
- `apply_content_archive(archive, content_root, expected) -> (Vec<XeniaApplied>, String)`:
  extract, walk regular files sorted, collect successes and errors; errors
  with no successes → `Err(joined)`; else successes + joined warning;
  staging removed always.
- Content root from `autoconfig::readers::xenia_directory_settings`; empty
  → `"Could not determine Xenia content directory. Is Xenia configured?"`.
  Non-Windows gate first, in this order: no configured Xbox 360 emulator →
  `"Xbox 360 content requires a Linux-compatible emulator such as Xenia
  Edge. Install and configure Xenia Edge, then try again."`; the configured
  one is not available on this platform → `"The configured Xbox 360
  emulator only runs on Windows. Install a Linux-compatible emulator such
  as Xenia Edge to apply content."`
- After a base Xbox 360 install: for `update` then `dlc` with file ids,
  admit a `Xbox360Content` job silently.

**Native** (`native.rs`), ported from doc 03 §2a, §7, §9.1, §14, doc 04 §9:

- `is_native_platform` already exists in `launch/mod.rs`; move to
  `specials::native` and re-export.
- `select_archive(files) -> Option<&RomFile>`: skip `game.json` and names
  containing `/` or `\`; first whose lower-cased name ends with one of
  `.7z .zip .rar .tar .gz .tgz .xz .zst .bz2`, else first top-level.
- Install target: `<library>/<platform>/<SafeTitle>/` = `native_game_dir`;
  archive at `<native_game_dir>/<archive name>`; always extract into
  `<native_game_dir>/game/`.
- `game.json`: fetched after download via the content endpoint when the ROM's
  file list has it; `parse_game_json(text) -> GameJson` lenient; `apply`
  fills `revision`, `first_release_date`, `tags` only when blank, always
  writes `included_dlc` (`"[]"` default).
- Finalize on Linux: create `<native_game_dir>/prefix` (or
  `<extracted_dir>/prefix`), store as `native_wineprefix`.
- `executable_candidates(install_dir) -> Vec<PathBuf>`: recursive, suffix in
  `{exe, bat, cmd, ps1, sh}` (casefold), sorted by (path component count,
  casefold path). `install_dir`: `extracted_dir` if dir, else parent of an
  existing `extracted_path`, else parent of the first existing archive
  candidate.
- `resolved_executable(row) -> Option<PathBuf>`: pinned path if exists, is a
  file and has a launchable suffix; else first candidate.
- `apply_update(row, archive, temp_dir, extractor) -> Result<InstalledGame, String>`:
  overwrite only non-empty fields (`rom_id`, `rom_file_name`,
  `server_updated_at`, `description`, `rating`, `genres`, `regions`,
  `filesize_bytes`, `screenshot_urls`, `ra_id`); require `extracted_dir`
  (`"Installed game directory not found - reinstall the game and try again."`,
  `"Installed game directory does not exist: <path>"`); merge through
  `<extracted_dir parent>/<safe title>-temp` (fallback system temp
  `grid-launcher-<safe title>-temp`); re-detect the launch file unless
  `native_executable_path` is set; delete the archive, warning
  `"Updated <title>, but could not delete archive:\n<path>\n<error>"`.

### grid-core: launch — native branch and compat tools (`launch/native.rs`, `launch/compat.rs`)

- `build_native_command(row, default_compat_tool, host) -> Result<Command, String>`:
  executable via `resolved_executable` else `"No launchable native executable
  is configured for this game. Use Game Settings to select one."`; compat =
  row's `native_compat_tool` trimmed, else `default_compat_tool` (blank on
  Windows hosts), else none. `wine` → `[which("wine") or "wine", exe, args…]`;
  other non-empty → `[which("umu-run"), exe, args…]` with `PROTONPATH=<value>`
  and `"umu-run was not found on PATH; install umu-launcher to use Proton."`
  when missing; `WINEPREFIX` set and dir created when the row has a prefix;
  cwd = exe parent; args parsed with the existing shell-words splitter;
  env via the existing clean-env helper.
- `LaunchService::launch` replaces the native refusal with this branch;
  session tracking unchanged.
- `compat.rs`: `CompatTool { name, kind: "wine" | "proton", path }`;
  `discover(xdg_data_home) -> Vec<CompatTool>`: system Wine (`which("wine")`,
  name "Wine (system)", path "wine"), Steam roots
  `~/.steam/steam/compatibilitytools.d`,
  `~/.local/share/Steam/compatibilitytools.d`,
  `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d`
  (subdirs containing a `proton` file, symlinks resolved, dedup by resolved
  path), managed `<XDG_DATA_HOME>/grid-launcher/compat-tools/*` (same
  `proton` test). Windows host → empty.
- Config: `default_compat_tool: String` and `compat_tool_installs:
  Vec<CompatToolInstall { name, path, source_id, release_tag }>` are both
  new to the Rust `Config` (neither exists today) and persist (D7).
- Managed install: `EmulatorJob { compat_tool: true }` installs into
  `<XDG_DATA_HOME>/grid-launcher/compat-tools/<sanitized stem>` and records
  a `CompatToolInstall`; catalog listing exposes `is_compat_tool` profiles
  in a separate list.

### grid-core: firmware (`firmware/`)

```
firmware/mod.rs      FirmwareTarget { path, keywords: Option<Vec<String>> }, install_platform_firmware, warnings
firmware/routing.rs  target resolution from profile dirs + tokens; RetroArch/Cemu shaping
firmware/write.rs    per-file dispatch (.7z/.rar flat, zip keep/flat/with-paths, plain write), skip_existing
```

- `RommClient::firmware(platform_id) -> Vec<FirmwareRecord { id, file_name }>`
  (`GET /api/firmware?platform_id=`; non-array → empty) and
  `firmware_bytes(id, file_name)` (`GET /api/firmware/{id}/content/{file_name}`).
- `install_platform_firmware(client, platform_id, targets, opts) -> Vec<String>`
  (warnings) per doc 03 §18 steps 1–9 verbatim, including
  `"Firmware fetch failed for platform <id>: <error>"`,
  `"Failed to download firmware <name>: <error>"`, `skip_existing` default
  true, `extract_zip_with_paths`, keep-as-archive rule, `__MACOSX` /
  `.DS_Store` skips, traversal guard.
- `firmware_targets_for_entry(entry, profile, library_dir, config_dir) ->
  Vec<FirmwareTarget>`: profile `firmware_directories` (string or
  `{path, match}`), env vars, `%EMULATOR_DIR%`, `%LIBRARY_DIR%`,
  `%CONFIG_DIR%`, `~`, relative → emulator dir, dedup case-folded.
- `shape_for_retroarch(core_id, entries, targets) -> RetroArchFirmwarePlan
  { firmware_targets, extract_with_paths, config_targets, saves_targets }`
  from `autoconfig::cores::{core_firmware_metadata, core_config_files_metadata,
  core_saves_files_metadata}` per install_mixin.py:552-631 (subdirectory,
  file list restriction, `config/<core>/`, saves with `default` and `:\`
  notations); Cemu → targets restricted to `keys.txt`; Dolphin post-step =
  existing autoconfig skip-IPL + GC-pad writers.
- `install_for_game(client, ctx) -> String` (warning text): platform id →
  default emulator → entry → targets → three calls (firmware, configs,
  saves with paths); each wrapped as `"Firmware install error: <e>"`.
- `platform_ids_for_profile(profile, platforms, retroarch_cores) -> Vec<i64>`
  for the fresh-install trigger.
- RPCS3: `rpcs3_pup_path(entry_path) -> Option<PathBuf>` (new; the Rust
  autoconfig has no such helper today — port `rpcs3_pup_path` from
  `grid_launcher/emulator/rpcs3.py`), and
  `spawn_rpcs3_installfw(exe, pup) -> bool` (`[exe, "--installfw", pup]`,
  cwd exe parent, clean env).

### App layer (Tauri)

- New commands: `install_content(rom_id, kind)`, `install_native_update(rom_id)`,
  `content_availability(rom_id) -> { update: bool, dlc: bool }`,
  `native_game_settings(rom_id) -> { executable, parameters, compat_tool,
  wineprefix, candidates: Vec<String> }`, `set_native_game_settings(rom_id,
  executable, parameters, compat_tool)`, `list_compat_tools -> { tools,
  default }`, `set_default_compat_tool(value)`, `list_compat_tool_catalog`,
  `install_compat_tool(source_id)`, `rpcs3_firmware_status(emulator_name)
  -> { pup_path: Option<String> }`, `install_ps3_firmware(emulator_name)
  -> bool` (spawns `--installfw`), `cancel_download_for_rom(rom_id)`.
- `launch_game`: native branch through grid-core; firmware-before-launch
  trigger (result discarded; warnings logged at debug without paths).
- Firmware triggers: `FirmwareService` (app) with one job per emulator
  directory at a time (`HashSet<PathBuf>` guard), spawned on the async
  runtime: (a) finalize hook from the install service (existing notify
  path extended with a "game finalized" callback carrying the record); (b)
  fresh source-emulator install (existing autoconfig hook site); (c)
  `save_emulator`/emulator install for RPCS3 without a PUP → a synthetic
  downloads entry `"PS3 Firmware"` on platform `"PlayStation 3"` via a new
  `InstallService::admit_external(title, platform, kind="firmware") ->
  entry id` + `complete_external(id, error)`.
- Platform ids: `list_platforms` already feeds names; it now also stores a
  `name → id` map on the install service (`set_platform_ids`).
- Event `compat-tools-changed` after a managed compat-tool install completes.

### Frontend

- `api.ts` types and invokes for every command above; `DownloadEntry.kind`.
- Details: "Install App" label for native platforms; `Install Update` /
  `Install DLC` buttons (installed PS4/Xbox 360 with availability); the
  Cancel button (`details-cancel`) when a live entry exists; "Game
  Settings" (`details-game-settings`) for installed native games opening
  `details/NativeSettings.svelte` (executable select from candidates, params
  input, compat select on non-Windows, read-only prefix line).
- Emulators: `emulators/CompatTools.svelte` panel (non-Windows): radio
  groups Wine / Proton (system) / Managed with `"No compatibility tools
  installed"` when empty; catalog list with install buttons; default
  persists immediately. RPCS3 card: note `"PS3 firmware downloaded — click
  Install to activate it."` and button `"Install PS3 Firmware"`; toast
  `"PS3 firmware installation started — follow the RPCS3 dialog to
  complete."` on success, failure toast per Python.
- Downloads drawer: rows for content, compat-tool and firmware kinds reuse
  the existing row component; titles carry the kind suffix.

## Strings (verbatim, in addition to those inline above)

- `"No PS3 VFS dev_hdd0 path configured for <title>"`,
  `"No PS3 game ID found in archive for <title>"`,
  `"Failed to install PS3 game <title>: <error>"`.
- `"Archive extracted but no ROM file was found"` (existing).
- `"Could not determine Xenia content directory. Is Xenia configured?"`.
- `"Cancelled while queued"` (existing).
- `"Firmware install error: <e>"`, `"PS3 Firmware"`, `"PlayStation 3"`.
- Native launch: `"No launchable native executable is configured for this
  game. Use Game Settings to select one."`
- Compat panel: `"No compatibility tools installed"`, `"Wine (system)"`.

## Deviations (numbered; recorded in doc 03 unless noted)

- **D1** RAR archives extract on every platform through the bundled `unrar`
  crate (Python: PS3 only, external 7-Zip).
- **D2** The Sony direct PS3 firmware path is dropped; server firmware only
  (Python: Sony first, server fallback, TLS verification disabled).
- **D3** An ISO inside a PS3 archive requires an external 7-Zip binary; with
  none, that entry fails visibly (Python: same dependency, silent skip).
- **D4** PS3 routing receives the RPCS3 data root, so `config/` lands in the
  data root (doc 03 open question; Python omitted it).
- **D5** One content-category parser (RomM's `files[].category`) serves PS4
  and Xbox 360 (doc 03 open question).
- **D6** Firmware jobs run beside the install queue, one per emulator
  directory at a time, never inside it (Python: inline in finalize and
  daemon threads).
- **D7** Managed compat-tool installs persist in config across restarts
  (Python reset `compat_tool_installs` on load — doc 02/04 defect).
- **D8** Content, compat-tool and firmware jobs are typed rows in the
  downloads drawer with the kind in the title.
- **D9** Details gains a Cancel button (closes milestone 2 deviation 7).
- **D10** (doc 04) PS3 launch target resolves from registry fields (closes
  milestone 3 deviation 3); firmware runs after a fresh source-emulator
  install (closes milestone 4 deviation 5); compat-tool profiles are listed
  in their own panel (amends milestone 4 deviation 2).
- **D11** Uninstall of a PS3 or native game continues past per-step
  failures and reports them together (extends milestone 2 deviation 2).

## Follow-the-code quirks (ported as-is)

- Native archive selection order; `game.json` `included_dlc` always
  overwritten; `name` parsed but unused.
- PS4 title-id detection order and the `eboot.bin`-only candidate rule.
- STFS offsets and the anonymous XUID directory.
- Firmware keep-as-archive rule (exact-name keyword match + no
  path-preserving flag); `skip_existing` default; flat copy for 7z/rar.
- Compat-tool resolution: `wine` special-cased, anything else is Proton
  through `umu-run`; `PROTONPATH` verbatim.
- Firmware never blocks a launch.

## Configuration

- `default_compat_tool: String` (default `""`).
- `compat_tool_installs: Vec<CompatToolInstall>` (default empty).
- No new firmware keys.

## Security

- Token secrecy unchanged. Firmware and content downloads use the
  authenticated client; no URL with a query string is logged.
- Extraction traversal guard covers RAR, firmware zips, PS4 content, Xbox
  content, native updates.
- Spawned processes (`wine`, `umu-run`, `rpcs3 --installfw`) inherit the
  cleaned environment; no secrets in env.
- No TLS relaxation anywhere (D2).

## Testing

- grid-core unit: PS3 classification (every class), routing into temp roots,
  ISO short circuit, id synthesis/scan, error strings; PS4 normalize/detect
  order, eboot ranking, apply_content ten steps; STFS header good/bad/
  mismatch; native archive selection, candidates ordering, executable
  resolution, command building (plain/wine/proton, env, cwd), game.json
  parsing/apply, update merge; firmware routing/writing from
  `tests/test_firmware_install.py` (all 100 cases mapped), wiremock for
  the two endpoints; RAR fixture extraction + traversal rejection;
  registry v3 migration from v1 and v2 (idempotent).
- vitest: content availability → buttons, compat list grouping, native
  settings form helpers, drawer kind labels.
- E2E stage groups (seeded data dirs + stub scripts): `ps3-install`,
  `content`, `native`, `firmware` as described in section 6 of the design
  conversation; the mock RomM gains `/api/firmware` and firmware content,
  a PS3 zip fixture, a PS4 base+update fixture pair, an STFS fixture, a
  Windows-platform zip with a stub executable, and `files[].category`.

## Milestone exit

- Every plan task reviewed clean; final whole-branch review; one fix wave.
- `cargo test --workspace`, clippy `-D warnings`, fmt, secret hygiene,
  `npm run check`, `npm test`, full `scripts/e2e.sh` green.
- Docs 03 and 04 deviations recorded; doc 03 open questions ruled.
- `cargo clean --profile dev` from `rewrite/`.
