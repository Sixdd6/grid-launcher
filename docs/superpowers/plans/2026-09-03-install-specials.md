# Install Specials, Native Games, and Firmware Implementation Plan (rewrite milestone 8)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish doc 03 in the Rust rewrite: PS3/PS4/Xbox 360 install
specials, native Windows games (install, update, Wine/Proton launch, compat
tools with managed installs), firmware auto-install from the RomM server,
registry v3, RAR extraction everywhere, and the matching Details/Emulators
UI, E2E groups, and docs.

**Architecture:** The install service gains an `InstallMode` per job and a
`library/specials/` layer (`ps3`, `ps4`, `xenia`, `native`) that finalize
dispatches to by platform and mode. Content jobs (PS4/Xbox 360 update/DLC,
native update) are ordinary FIFO jobs keyed by rom+kind. `launch/native.rs`
and `launch/compat.rs` add the Wine/Proton branch and compat-tool discovery;
managed compat tools are emulator jobs installed under the app data dir. A
new `firmware/` module ports `firmware_install.py` plus the RetroArch/Cemu
shaping from `install_mixin.py`; the Tauri `FirmwareService` runs firmware
jobs beside the queue, one per emulator directory. The frontend adds
content/cancel/settings buttons to Details, a CompatTools panel and RPCS3
firmware button to Emulators, and typed drawer rows.

**Tech Stack:** Rust (tokio, reqwest, rusqlite, serde, regex, zip,
sevenz-rust2, `unrar` 0.5.8, wiremock), Tauri 2, Svelte 5, vitest,
WebdriverIO.

**Spec:** `docs/superpowers/specs/2026-09-02-install-specials-design.md`
(binding). Behavior contract: `docs/porting/03-library-install.md` §2, 3, 7,
8, 9, 11–14, 16, 18 and `docs/porting/04-emulator-launch.md` §5, 9, 10, 11,
12. Python sources (win over the docs): `grid_launcher/library/{ps3_install,
archive_preparation,firmware_install,install_metadata,install_cleanup,
install_paths}.py`, `grid_launcher/emulator/{xenia,launch,rpcs3}.py`,
`grid_launcher/ui/mixins/{install_mixin,emulator_ui_mixin,cloud_mixin}.py`.
Oracle tests: `tests/test_firmware_install.py`.

## Global Constraints

- **Behavioral parity, free machinery** (user ruling). User-facing strings
  port verbatim. Python source wins over docs; spec deviations D1–D11 win
  over both. New rulings made while planning (record in doc 03 as D12+):
  - **D12** Base-install candidates exclude files whose `category` is not
    `game` (blank counts as `game`), so a PS4/Xbox 360 ROM with update/DLC
    files does not become a multi-file game.
  - **D13** A native payload whose archive suffix is not extractable
    (e.g. a bare `.iso`) installs as a direct file (`archive_path` set, no
    `game/` dir) instead of failing extraction.
  - **D14** Firmware warnings from the finalize and launch triggers are
    logged (`tracing::warn`), not joined into the download entry (they run
    beside the queue per D6).
  - **D15** The managed compat-tool root honors the data-dir override:
    `<GRID_LAUNCHER_DATA_DIR>/compat-tools` when set, else
    `<XDG_DATA_HOME>/grid-launcher/compat-tools`.
  - **D16** An Xbox 360 content archive is deleted after a successful apply
    (Python left it to the generic cleanup).
  - **D17** The RPCS3 "PS3 Firmware" job is skipped silently when the
    server's PS3 platform id is unknown (offline); no drawer row appears.
  - **D18** A native launch registers a session like an emulated launch, so
    Stop works (Python registered none).
  - Native update has no UI trigger this milestone (update detection is
    doc 10); only the command and `api.ts` wrapper exist.
- **Verbatim strings** (exact, including punctuation):
  `"No PS3 VFS dev_hdd0 path configured for <title>"`,
  `"No PS3 game ID found in archive for <title>"`,
  `"Failed to install PS3 game <title>: <error>"`,
  `"Cannot extract ISO <name>: no 7-Zip binary found"`,
  `"Archive extracted but no ROM file was found"`,
  `"PS4 content apply is only supported for PS4 games"`,
  `"Installed PS4 game is missing a detectable title ID"`,
  `"Installed PS4 game is missing an extracted install directory"`,
  `"Installed PS4 directory does not exist: <path>"`,
  `"Installed PS4 title directory was not found: <path>"`,
  `"PS4 content archive must include a title-ID root folder"`,
  `"PS4 content title ID mismatch: expected <id>, archive contains <ids or 'unknown'>"`,
  `"Failed to merge PS4 content into installed game: <error>"`,
  `"Applied PS4 content, but could not delete archive:\n<path>\n<error>"`,
  `"Content file not found: <path>"`,
  `"File does not appear to be an STFS package (bad magic)"`,
  `"Title ID mismatch: expected <ID>, archive contains <id>"`,
  `"Could not determine Xenia content directory. Is Xenia configured?"`,
  `"Xbox 360 content requires a Linux-compatible emulator such as Xenia Edge. Install and configure Xenia Edge, then try again."`,
  `"The configured Xbox 360 emulator only runs on Windows. Install a Linux-compatible emulator such as Xenia Edge to apply content."`,
  `"Installed game directory not found - reinstall the game and try again."`,
  `"Installed game directory does not exist: <path>"`,
  `"Updated <title>, but could not delete archive:\n<path>\n<error>"`,
  `"No launchable native executable is configured for this game. Use Game Settings to select one."`,
  `"Invalid custom launch parameters: <error>"`,
  `"umu-run is not installed. Install the umu-launcher package to use Proton compatibility tools."`
  (Python verbatim; the spec's paraphrase is superseded by the verbatim rule),
  `"Firmware fetch failed for platform <id>: <error>"`,
  `"Failed to download firmware <name>: <error>"`,
  `"Could not create firmware directory <dir>: <error>"`,
  `"Failed to extract firmware archive <name>: <error>"`,
  `"Failed to write firmware <name> to <dest>: <error>"`,
  `"Firmware install error: <e>"`, `"PS3 Firmware"`, `"PlayStation 3"`,
  `"Wine (system)"`, `"No compatibility tools installed"`,
  `"PS3 firmware downloaded — click Install to activate it."`,
  `"Install PS3 Firmware"`,
  `"PS3 firmware installation started — follow the RPCS3 dialog to complete."`,
  `"Could not launch RPCS3 to install firmware. Check the emulator path."`,
  `"Install App"`, `"Install Update"`, `"Install DLC"`, `"Game Settings"`,
  `"Cancel"`.
- **Tokens.** No new `expose_secret()` call sites. Firmware and content
  downloads go through `RommClient`. No log line carries a URL with a query
  string, a header, or a token. `bash scripts/check_secret_hygiene.sh`
  passes after every task.
- **grid-core never imports Tauri.** The app layer owns `AppHandle`, events,
  the `FirmwareService`, and the platform-id map feed.
- **Registry:** `LATEST_USER_VERSION = 3`; the twelve v3 columns are
  `TEXT NOT NULL DEFAULT ''`: `native_executable_path`,
  `native_launch_parameters`, `native_compat_tool`, `native_wineprefix`,
  `native_game_dir`, `included_dlc`, `ps3_trophy_paths`, `ps3_game_id`,
  `ps3_iso_path`, `ps4_game_id`, `ps4_content`, `ra_id`. Migration 2→3 is
  one transaction, idempotent via `PRAGMA table_info`. `SCHEMA_SQL` (fresh
  DBs) carries the columns too.
- **Platform predicates** (`library/platforms.rs`, ported from
  `selection.py`): native = trimmed lower-cased platform starts with
  `windows`; PS3 = casefold in {`playstation 3`, `ps3`}; PS4 = normalized
  (non-alphanumerics → single space) in {`playstation 4`, `ps4`} or token
  `ps4` or compact contains `playstation4`; Xbox 360 = (token `xbox` or
  compact contains `xbox360`) and (compact contains `xbox360` or token
  `360`).
- **Extraction table** (Python order, `archive_preparation.py:821`, D1):
  native → always; arcade → never; PS3 → suffix in
  `{zip,7z,rar,tar,gz,bz2,xz}`; else suffix in `{7z,zip,rar,tar,gz,bz2,xz}`.
  RAR uses the `unrar` crate. Every extractor path keeps the traversal guard.
- **Queue:** single FIFO unchanged. New `JobKey` variants
  `Content(i64, ContentKind)`, `NativeUpdate(i64)`, `External(u64)`;
  `DownloadEntry.kind` ∈ {`base`, `ps4_content`, `xbox360_content`,
  `native_update`, `emulator`, `compat_tool`, `firmware`}; content titles
  are `"<title> (update)"` / `"<title> (dlc)"`.
- **Content download:** target `<platform dir>/<safe title>-<kind>.zip`,
  URL `/api/roms/<id>/content/<encoded fs_name>` with ONE query pair
  `("file_ids", "<csv>")`.
- **Firmware:** `skip_existing` default true and it never skips the
  download (only the write); `.7z`/`.rar` flat copy; zip keep-as-archive
  only when routed through a keyword entry whose list contains the exact
  lower-cased file name and `extract_zip_with_paths` is false; `__MACOSX`
  and `.DS_Store` skipped; with-paths members that are absolute or contain
  `..` skipped.
- **Compat tools:** `wine` is special-cased; any other non-empty value is a
  Proton path run through `umu-run` with `PROTONPATH=<value>`;
  `WINEPREFIX` only when the row has a prefix (dir created). Windows hosts:
  default compat tool is blank, discovery returns empty.
- **Process spawns** (`wine`, `umu-run`, `rpcs3 --installfw`) use
  `launch::spawn::clean_env()` plus the branch's overrides.
- **Commit after every task** (standing instruction). Never run `git
  checkout/restore/reset` on tracked files.

## Shared interfaces (names every task must use)

```rust
// grid-core
library::platforms::{is_native_platform, is_ps3_platform, is_ps4_platform, is_xbox360_platform}  // fn(&str) -> bool
library::content::{ContentKind, file_ids_by_category, content_file_ids, ContentAvailability, content_availability}
library::InstallMode { Base, Ps4Content, Xbox360Content, NativeUpdate }
library::queue::{JobKey::{Rom(i64), Content(i64, ContentKind), NativeUpdate(i64), Emulator(String), External(u64)}, DownloadEntry { kind: &'static str, .. }}
library::registry::{InstalledGame (12 new fields), Registry::{update_native_settings, update_ps4_content}}
library::extract::{extract_archive, should_extract, extract_iso_with_system_7z}
library::specials::ps3::{Ps3Roots, Ps3Outcome, Ps3Class, classify, route, iso_only_file, IsoExtract}
library::specials::ps4::{normalize_title_id, detect_title_id, select_ps4_launch_file, expected_title_id, apply_content, Ps4Applied}
library::specials::xenia::{read_stfs_header, apply_content_file, apply_content_archive, XeniaApplied}
library::specials::native::{select_archive, GameJson, parse_game_json, apply_game_json, is_launchable_native_file, install_dir, executable_candidates, resolved_executable, apply_update, NativeUpdate}
library::InstallService::{install_content, install_native_update, install_compat_tool, cancel_for_rom, admit_external, complete_external, set_platform_ids, platform_ids, set_game_finalized_hook, set_emulator_installed_hook, set_compat_tools_hook}
launch::native::{NativeLaunch, build_native_command}
launch::compat::{CompatTool, discover, managed_root, find_proton_dir}
launch::profiles::{EmulatorProfile { firmware_directories: Vec<FirmwareDirSpec>, compat_tool_type: String, .. }, FirmwareDirSpec, profile_available_on_host}
launch::catalog::{compat_tool_catalog_entries, find_compat_profile}
config::{Config { default_compat_tool: String, compat_tool_installs: Vec<CompatToolInstall>, .. }, CompatToolInstall}
romm::{RomFile { category: String, .. }, FirmwareRecord, RommClient::{firmware, firmware_bytes}}
firmware::{FirmwareTarget, FirmwareOptions, resolve_targets, should_keep_zip, install_platform_firmware}
firmware::routing::{targets_for_entry, RetroArchPlan, shape_for_retroarch, shape_for_cemu, GameFirmwareContext, install_for_game, platform_ids_for_profile}
firmware::rpcs3::{rpcs3_pup_path, spawn_rpcs3_installfw}
```

Tauri commands (Task 15): `install_content`, `install_native_update`,
`content_availability`, `native_game_settings`, `set_native_game_settings`,
`list_compat_tools`, `set_default_compat_tool`, `list_compat_tool_catalog`,
`install_compat_tool`, `rpcs3_firmware_status`, `install_ps3_firmware`,
`cancel_download_for_rom`. Events: `compat-tools-changed`.

---

### Task 1: Platform predicates, content categories, `RomFile.category`

**Files:**
- Create: `rewrite/crates/grid-core/src/library/platforms.rs`
- Create: `rewrite/crates/grid-core/src/library/content.rs`
- Modify: `rewrite/crates/grid-core/src/library/mod.rs` (add `pub mod platforms; pub mod content;`; `is_download_candidate` category filter D12)
- Modify: `rewrite/crates/grid-core/src/romm/mod.rs:288-295` (`RomFile.category`)
- Modify: `rewrite/crates/grid-core/src/launch/mod.rs:419-424` (delete the private `is_native_platform`; `use crate::library::platforms::is_native_platform;`)
- Test: inline `#[cfg(test)]` in both new files; `rewrite/crates/grid-core/tests/install_service.rs` (one D12 test)

**Interfaces:**
- Produces:
  ```rust
  pub fn is_native_platform(platform: &str) -> bool;
  pub fn is_ps3_platform(platform: &str) -> bool;
  pub fn is_ps4_platform(platform: &str) -> bool;
  pub fn is_xbox360_platform(platform: &str) -> bool;
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum ContentKind { Update, Dlc }
  impl ContentKind { pub fn as_str(self) -> &'static str /* "update" | "dlc" */ ; pub fn parse(s: &str) -> Option<Self> }
  pub fn file_ids_by_category(files: &[RomFile]) -> BTreeMap<String, Vec<i64>>;
  pub fn content_file_ids(files: &[RomFile], kind: ContentKind) -> Vec<i64>;
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
  pub struct ContentAvailability { pub update: bool, pub dlc: bool }
  pub fn content_availability(files: &[RomFile]) -> ContentAvailability;
  pub fn is_game_category(category: &str) -> bool; // trimmed lower-cased == "game" or blank
  ```
  `RomFile` gains `#[serde(default)] pub category: String` (RomM sends `null` for
  no category: use `#[serde(default, deserialize_with = "null_to_empty")]` — copy the
  pattern already used for nullable strings in `romm/mod.rs`, or add a small
  `fn null_to_empty<'de, D>(d: D) -> Result<String, D::Error>` in that file).

- [ ] **Step 1: `platforms.rs`** — port `selection.py:11-52` verbatim:

```rust
//! Platform predicates (`grid_launcher/emulator/selection.py:11-52`).
use regex::Regex;
use std::sync::OnceLock;

pub fn is_native_platform(platform: &str) -> bool {
    platform.trim().to_lowercase().starts_with("windows")
}

pub fn is_ps3_platform(platform: &str) -> bool {
    matches!(platform.trim().to_lowercase().as_str(), "playstation 3" | "ps3")
}

fn normalized_tokens(platform: &str) -> (String, String, Vec<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[^a-z0-9]+").unwrap());
    let lowered = platform.trim().to_lowercase();
    let normalized = re.replace_all(&lowered, " ").trim().to_string();
    let compact = normalized.replace(' ', "");
    let tokens = normalized.split_whitespace().map(str::to_string).collect();
    (normalized, compact, tokens)
}

pub fn is_ps4_platform(platform: &str) -> bool {
    let (normalized, compact, tokens) = normalized_tokens(platform);
    if normalized.is_empty() { return false; }
    if normalized == "playstation 4" || normalized == "ps4" { return true; }
    if tokens.iter().any(|t| t == "ps4") { return true; }
    compact.contains("playstation4")
}

pub fn is_xbox360_platform(platform: &str) -> bool {
    let (normalized, compact, tokens) = normalized_tokens(platform);
    if normalized.is_empty() { return false; }
    let has_xbox = tokens.iter().any(|t| t == "xbox") || compact.contains("xbox360");
    if !has_xbox { return false; }
    compact.contains("xbox360") || tokens.iter().any(|t| t == "360")
}
```

Tests (table): `"Windows"`, `" windows 10"` → native; `"PlayStation 3"`,
`"PS3"` → ps3, `"Sony PlayStation 3"` → NOT ps3; `"PlayStation 4"`,
`"Sony PS4"`, `"PlayStation4"` → ps4, `"PlayStation 3"` → not ps4;
`"Xbox 360"`, `"Microsoft Xbox360"` → xbox360, `"Xbox"`, `"Xbox One"` → not.

- [ ] **Step 2: `content.rs`** — port `server/catalog.py:246-262`:

```rust
pub fn file_ids_by_category(files: &[RomFile]) -> BTreeMap<String, Vec<i64>> {
    let mut map: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for file in files {
        let category = file.category.trim().to_lowercase();
        let key = if category.is_empty() { "game".to_string() } else { category };
        map.entry(key).or_default().push(file.id);
    }
    map
}
pub fn content_file_ids(files: &[RomFile], kind: ContentKind) -> Vec<i64> {
    file_ids_by_category(files).remove(kind.as_str()).unwrap_or_default()
}
pub fn content_availability(files: &[RomFile]) -> ContentAvailability {
    let map = file_ids_by_category(files);
    let has = |k: &str| map.get(k).is_some_and(|v| !v.is_empty());
    ContentAvailability { update: has("update"), dlc: has("dlc") }
}
pub fn is_game_category(category: &str) -> bool {
    let c = category.trim().to_lowercase();
    c.is_empty() || c == "game"
}
```

Tests: blank category → `game`; `" Update "` → `update`; two updates keep
order; availability from a mixed list; `ContentKind::parse("DLC")` → Dlc,
`parse("x")` → None; serde round-trip of `ContentKind` is `"update"`.

- [ ] **Step 3: D12 in `is_download_candidate`** (`library/mod.rs:193-198`): add
  `&& content::is_game_category(&file.category)`. Add an integration test in
  `tests/install_service.rs`: a detail with files `[game.zip (category "game"),
  update.zip (category "update")]` installs as a SINGLE-file game (no
  `multi_file_game_dir`, only `game.zip` requested — assert via wiremock
  `received_requests()` that no `/content/update.zip` request occurred). Extend
  `file_spec`/`detail_json` helpers with an optional `category` (default `null`).
- [ ] **Step 4:** `cargo test -p grid-core` green; `cargo clippy -p grid-core --all-targets -- -D warnings`; `cargo fmt`.
- [ ] **Step 5: Commit** `rewrite: platform predicates, content categories, RomFile.category (D12)`

---

### Task 2: Registry v3

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/registry.rs`
- Test: `rewrite/crates/grid-core/tests/registry.rs`
- Modify: every E2E seed that copies the schema is NOT touched here (Task 18 writes new seeds at v3).

**Interfaces:**
- Produces: `InstalledGame` gains `pub native_executable_path, native_launch_parameters, native_compat_tool, native_wineprefix, native_game_dir, included_dlc, ps3_trophy_paths, ps3_game_id, ps3_iso_path, ps4_game_id, ps4_content, ra_id: String` (all `#[serde(default)]`, in this order, appended after `screenshot_urls`);
  `pub fn update_native_settings(&self, rom_id: i64, executable: &str, parameters: &str, compat_tool: &str) -> Result<bool, LibraryError>`;
  `pub fn update_ps4_content(&self, rom_id: i64, game_id: &str, content_json: &str) -> Result<bool, LibraryError>` (both return `Ok(false)` when no row has that rom_id).
  Native update re-registers through the existing `upsert` (no `update_record`).
  `ps3_game_id`/`ps4_game_id` are stored upper-cased by `upsert` (`to_uppercase()` on write).

- [ ] **Step 1: Failing tests** in `tests/registry.rs`:
  - `fresh_db_is_v3_and_has_the_twelve_columns` (PRAGMA user_version == 3; `PRAGMA table_info` lists all 12).
  - `migrates_v1_to_v3_transactionally`: open a v1 DB (copy the v1 `SCHEMA_SQL` from `e2e/seed/images-seed.mjs` into the test as a const), insert one row, `Registry::open` → version 3, all columns exist, row readable with blank new fields.
  - `migrates_v2_to_v3` and `migration_is_idempotent_when_columns_preexist` (create a v2 DB that already has `ra_id` added by hand, then open: no error, version 3).
  - `upsert_round_trips_new_fields` (all twelve set; `ps3_game_id: "blus30336"` reads back `"BLUS30336"`).
  - `update_native_settings_and_ps4_content_return_false_for_unknown_rom` and the `true` cases.
- [ ] **Step 2: Implement.** Bump `LATEST_USER_VERSION` to 3; `const V3_COLUMNS: [&str; 12]`; `fn migrate_2_to_3` mirroring `migrate_1_to_2` (registry.rs:61-89) over `V3_COLUMNS`; add the `2 => migrate_2_to_3(&mut conn)?` arm; extend `SCHEMA_SQL`, `SELECT_COLUMNS`, `from_row` (indices 23..=34), and the `upsert` INSERT/UPDATE lists. Implement the two update fns as `UPDATE installed_games SET ... WHERE rom_id = ?1` returning `changes() > 0`.
- [ ] **Step 3:** `cargo test -p grid-core --test registry` green; whole crate green (existing `install_service` tests build `InstalledGame` with `..Default::default()`; fix any struct literal that does not).
- [ ] **Step 4: Commit** `rewrite: registry v3 — native/ps3/ps4 columns, update_native_settings, update_ps4_content`

---

### Task 3: Extraction — RAR everywhere, Python should-extract table, ISO helper

**Files:**
- Modify: `rewrite/crates/grid-core/Cargo.toml` (`unrar = "0.5.8"`)
- Modify: `rewrite/crates/grid-core/src/library/extract.rs`
- Create: `rewrite/crates/grid-core/tests/fixtures/rar/version.rar` (copied from the `unrar` crate's `data/version.rar`, MIT OR Apache-2.0) and `rewrite/crates/grid-core/tests/fixtures/rar/README.md` (one line: source + license)
- Test: `rewrite/crates/grid-core/tests/extract.rs`

**Interfaces:**
- Produces:
  ```rust
  pub fn should_extract(platform: &str, archive: &Path) -> bool; // new table (Global Constraints)
  pub(crate) fn extract_rar(archive: &Path, dest: &Path, progress: ExtractProgress) -> Result<(), LibraryError>;
  pub(crate) fn rar_entry_relative_path(raw: &str) -> Result<PathBuf, LibraryError>; // traversal guard, shared
  pub(crate) fn extract_iso_with_system_7z(iso: &Path, dest: &Path) -> Result<(), String>;
  // Err(format!("Cannot extract ISO {name}: no 7-Zip binary found")) when find_system_7z() is None;
  // else runs extract_7z_system_fallback(iso, dest) (existing fn) and maps its error string through.
  ```

- [ ] **Step 1: Failing tests** (`tests/extract.rs`):
  - `should_extract_follows_the_python_table`: `("Windows","game.exe",true)`, `("Windows","game.iso",true)`, `("Arcade","game.zip",false)`, `("PlayStation 3","game.rar",true)`, `("SNES","game.rar",true)` (flip the existing false case), `("SNES","game.bin",false)`.
  - `extracts_the_rar_fixture`: copy `tests/fixtures/rar/version.rar` to a temp dir, `extract_archive` → `dest/VERSION` has bytes `unrar-0.4.0`; progress callback last `(processed, total)` has `processed == total == 11`.
  - `rar_entry_relative_path_rejects_traversal`: `"../x"` and `"/abs"` and `"a/../../b"` → `Err`, `"dir/file.bin"` → `Ok`, backslashes normalized to `/`.
  - `iso_helper_reports_missing_7z`: with `PATH=""` (set via `std::env::set_var` inside the test, restore after) → `Err("Cannot extract ISO game.iso: no 7-Zip binary found")`.
- [ ] **Step 2: Implement.** `dispatch()` gets `if lowercase_suffix(archive).as_deref() == Some("rar") { return extract_rar(...); }` before the zip sniff. `extract_rar`: first pass `Archive::new(archive).open_for_listing()` summing `unpacked_size` for `total`; second pass `open_for_processing()`; loop `read_header()`: `let name = header.entry().filename.to_string_lossy()`; `let rel = rar_entry_relative_path(&name)?`; directories (`entry().is_directory()`) → `fs::create_dir_all(dest.join(rel))`, `archive = header.skip()?`; files → create parent, `archive = header.extract_to(dest.join(rel))?`, `processed += size`, `progress(processed, total)`. Map `unrar::error::UnrarError` to `LibraryError::Extract(e.to_string())`. `should_extract` rewritten per the table with `is_native_platform`/`is_ps3_platform` from Task 1 (arcade check stays `is_arcade_platform`). Keep `EXTRACTABLE_SUFFIXES` as the default list plus `"rar"`; add `PS3_SUFFIXES`.
- [ ] **Step 3:** `cargo test -p grid-core --test extract`, clippy, fmt. Also `cargo build -p grid-core` must succeed on a clean machine (the crate builds the vendored unrar C++ source with `cc`; note in the task report if a system package was needed).
- [ ] **Step 4: Commit** `rewrite: RAR extraction on every platform via unrar (D1); Python should-extract table; ISO 7-Zip helper`

---

### Task 4: Config keys, profile firmware/compat fields, RomM firmware endpoints

**Files:**
- Modify: `rewrite/crates/grid-core/src/config.rs`
- Modify: `rewrite/crates/grid-core/src/launch/profiles.rs`
- Modify: `rewrite/crates/grid-core/src/romm/mod.rs`
- Test: inline tests + `rewrite/crates/grid-core/tests/romm_client.rs`

**Interfaces:**
- Produces:
  ```rust
  // config.rs
  #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
  pub struct CompatToolInstall { #[serde(default)] pub name: String, #[serde(default)] pub path: String,
      #[serde(default)] pub source_id: String, #[serde(default)] pub release_tag: String }
  // Config: #[serde(default)] pub default_compat_tool: String,
  //         #[serde(default)] pub compat_tool_installs: Vec<CompatToolInstall>,
  // profiles.rs
  #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
  pub struct FirmwareDirSpec { pub path: String, pub keywords: Option<Vec<String>> }
  // EmulatorProfile: pub firmware_directories: Vec<FirmwareDirSpec>, pub compat_tool_type: String
  pub fn profile_available_on_host(profile: &EmulatorProfile, host: &str) -> bool;
  // romm/mod.rs
  #[derive(Debug, Clone, serde::Deserialize)] pub struct FirmwareRecord { pub id: i64, #[serde(default)] pub file_name: String }
  impl RommClient {
      pub async fn firmware(&self, platform_id: i64) -> Result<Vec<FirmwareRecord>, RommError>;
      pub async fn firmware_bytes(&self, id: i64, file_name: &str) -> Result<Vec<u8>, RommError>;
  }
  ```

- [ ] **Step 1: Config.** Add the two fields (+ `impl Default`), tests mirroring `config_defaults_for_the_seven_new_fields` (config.rs:619) and `config_without_the_new_fields_writes_no_new_keys` (config.rs:457): defaults are `""`/empty; a config without the keys saves without `\ndefault_compat_tool =` / `\ncompat_tool_installs`; round-trip of one `CompatToolInstall`.
- [ ] **Step 2: Profiles.** `RawProfile` gains `#[serde(default)] firmware_directories: Vec<serde_json::Value>` and `#[serde(default)] compat_tool_type: String`. In `normalize_one`, map each value: a non-blank string → `FirmwareDirSpec { path: trimmed, keywords: None }`; an object with non-blank string `path` and a list `match` whose trimmed lower-cased non-empty strings are non-empty → `Some(keywords)`; anything else dropped (mirrors `cloud_mixin.py:1055-1075`). `profile_available_on_host(profile, host)`: host starting with `win` → true; profile name casefold in `WINDOWS_ONLY_SLUGS` → false; `source["platforms"]` list of non-blank strings not containing host casefold → false; else true (`profiles.py:22-56`). Tests against the real catalog: RetroArch → `[FirmwareDirSpec{path:"system", keywords:None}]`; Eden → two routed specs with keywords `["keys"]` and `["firmware"]`; GE-Proton → `compat_tool_type == "proton"`; `profile_available_on_host(Xenia Canary, "linux") == false`, `(Xenia Edge, "linux") == true`, `(Xenia Canary, "win32") == true`.
- [ ] **Step 3: RomM.** `firmware`: `self.get_json::<serde_json::Value>("/api/firmware", &[("platform_id", id.to_string())])`, then if the value is an array deserialize each element leniently (skip items without an integer `id`), else `Ok(vec![])` (`firmware_install.py:29-33`). `firmware_bytes`: `self.get_bytes(&format!("/api/firmware/{id}/content/{}", encode_file_segment(file_name)))` — reuse `library::mod::encode_file_segment` (make it `pub(crate)` in `library/paths.rs` if it is private). wiremock tests: query matcher `query_param("platform_id","19")`; non-array body → empty; bytes route returns the body; 401 → `RommError::Unauthorized`.
- [ ] **Step 4:** tests, clippy, fmt, hygiene script.
- [ ] **Step 5: Commit** `rewrite: compat-tool config keys, profile firmware_directories/compat_tool_type, RomM firmware endpoints`

---

### Task 5: `specials::ps3` — classify, route, ISO short circuit, ids

**Files:**
- Create: `rewrite/crates/grid-core/src/library/specials/mod.rs` (`pub mod ps3; pub mod ps4; pub mod xenia; pub mod native;` plus shared `pub(crate) fn copy_tree_merge(src, dst) -> io::Result<()>` = mkdir -p dst then recursive copy of every entry, files copied with `fs::copy` overwriting, i.e. Python `copytree(dirs_exist_ok=True)`; and `pub(crate) fn merge_tree(src, dst) -> io::Result<()>` = `_merge_tree` (archive_preparation.py:258-268): walk src recursively, mkdir dirs, copy files overwriting, never delete)
- Create: `rewrite/crates/grid-core/src/library/specials/ps3.rs`
- Modify: `rewrite/crates/grid-core/src/library/mod.rs` (`pub mod specials;`)
- Test: inline tests in `ps3.rs` (temp dirs)

**Interfaces:**
- Consumes: nothing from the install service (pure fs + closures).
- Produces:
  ```rust
  pub type IsoExtract<'a> = &'a dyn Fn(&Path, &Path) -> Result<(), String>;
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Ps3Class { DiscGameIdDir, GameIdDir, TrophyDir, BareDiscDir, IsoFile, NestedHdd0Game, ConfigDir, Unknown }
  pub fn classify(extracted_dir: &Path) -> Vec<(PathBuf, Ps3Class)>;
  pub fn iso_only_file(extracted_dir: &Path) -> Option<PathBuf>;
  #[derive(Debug, Clone)]
  pub struct Ps3Roots { pub dev_hdd0: PathBuf, pub games_root: Option<PathBuf>, pub data_root: Option<PathBuf> }
  #[derive(Debug, Clone, Default, PartialEq, Eq)]
  pub struct Ps3Outcome { pub game_id: String, pub installed_paths: Vec<PathBuf>, pub trophy_paths_json: String, pub extracted_path: String, pub extracted_dir: String }
  /// Routes and deletes `staging`. Errors are the verbatim strings from the Global Constraints.
  pub fn route(staging: &Path, roots: &Ps3Roots, title: &str, iso_extract: IsoExtract) -> Result<Ps3Outcome, String>;
  pub fn game_id_from_text(value: &str) -> String;           // ps3_install.py:17
  pub fn game_id_from_paths(paths: &[PathBuf]) -> String;    // ps3_install.py:26
  pub(crate) fn detect_game_id_from_sfo(parent: &Path) -> String; // ps3_install.py:288
  ```

- [ ] **Step 1: Failing tests** (build layouts in `tempfile::tempdir()`):
  - `classify_every_class`: a staging dir holding `NPWR12345/`, `BLUS30336/PS3_GAME/` + `BLUS30336/PS3_DISC.SFB` (disc), `BLES01234/PS3_GAME/` (game id), `BCUS99999/` (empty → GameIdDir fallback), `PS3_GAME/`, `dev_hdd0/game/`, `config/`, `misc/`, `game.iso`, `readme.txt` → expected classes; order = directories first then case-folded name.
  - `iso_only_file_requires_exactly_one_iso_entry`.
  - `route_disc_and_game_id_dirs_into_roots` (games_root set → disc goes to `<games_root>/BLUS30336`, game id dir to `<dev_hdd0>/game/BLES01234`; `game_id == "BLUS30336"` (first routed); `extracted_dir == <games_root>/BLUS30336`; staging deleted).
  - `route_trophy_and_nested_hdd0` (trophy → `<dev_hdd0>/home/00000001/trophy/NPWR12345`; nested `dev_hdd0/game/BLUS00001/x` and `dev_hdd0/home/00000001/trophy/NPWR00002/y` → merged, trophy_paths_json lists both trophy dirs as a JSON array of strings, `installed_paths` contains the NPWR dest).
  - `route_bare_disc_synthesizes_id_from_sfo` (`PS3_GAME/PARAM.SFO` bytes contain `...BLUS30336...` → dest `<dev_hdd0>/game/BLUS30336/PS3_GAME`), and `_without_sfo_uses_placeholder` (`PS3_GAME_DISC`).
  - `route_config_dir_prefers_data_root` (data_root Some → `<data_root>/config`; None → `<dev_hdd0 parent>/config`).
  - `route_iso_entry_uses_the_extractor` (fake extractor writes `BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN` into the temp dir → routed; extractor error → `Err(that string)` propagated as `"Failed to install PS3 game <title>: <error>"`).
  - `route_without_game_id_fails` (`"No PS3 game ID found in archive for Foo"`), `route_scans_installed_paths_for_id_skipping_npwr`.
  - `game_id_from_text_and_paths` unit cases.
- [ ] **Step 2: Implement** — port `ps3_install.py` function-for-function (regexes `^[A-Z]{4}\d{5}$`, `^NPWR\d{5}$`, `[A-Z]{4}\d{5}` search on upper-cased text, byte regex `[A-Z]{4}\d{5}` on SFO bytes). `route`: classify → per-class routing exactly as `ps3_route_extracted_contents` (ISO entries: `tempfile::tempdir()`, call `iso_extract(iso, tmp)`, on `Err(e)` return `Err(format!("Failed to install PS3 game {title}: {e}"))`, recurse `route_inner`); id fallback scan; then compute `extracted_path`/`extracted_dir` = first installed path whose file name upper-cases to the id, else `<dev_hdd0>/game/<ID>`; `trophy_paths_json` = `serde_json::to_string` of installed paths whose string casefold contains `trophy`; `fs::remove_dir_all(staging)` ignoring errors; io errors → `"Failed to install PS3 game {title}: {error}"`. Empty id → `"No PS3 game ID found in archive for {title}"`.
- [ ] **Step 3:** tests, clippy, fmt.
- [ ] **Step 4: Commit** `rewrite: specials::ps3 — classification, routing, ISO short circuit, id synthesis`

---

### Task 6: `specials::ps4` — title ids, eboot ranking, content apply

**Files:**
- Create: `rewrite/crates/grid-core/src/library/specials/ps4.rs`
- Test: inline

**Interfaces:**
- Consumes: `library::registry::InstalledGame` (fields `ps4_game_id`, `extracted_path`, `extracted_dir`, `ps4_content`, `platform`), `library::platforms::is_ps4_platform`, `specials::merge_tree`, `library::content::ContentKind`.
- Produces:
  ```rust
  pub fn normalize_title_id(value: &str) -> Option<String>;   // strip non-alphanumerics, upper, ^[A-Z]{4}\d{5}$
  pub fn detect_title_id(extracted_dir: &Path, launch_file: &Path, archive: &Path) -> String; // archive_preparation.py:95
  pub fn select_ps4_launch_file(extracted_dir: &Path, pool: &[PathBuf]) -> Option<PathBuf>;   // :61
  pub fn title_id_roots(dir: &Path) -> Vec<PathBuf>;                                          // :128, sorted casefold
  pub fn expected_title_id(row: &InstalledGame) -> String;                                    // :204
  pub type ExtractFn<'a> = &'a dyn Fn(&Path, &Path) -> Result<(), LibraryError>;
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct Ps4Applied { pub game_id: String, pub content_json: String, pub warning: String }
  pub fn apply_content(row: &InstalledGame, archive: &Path, kind: ContentKind, staging: &Path, extract: ExtractFn) -> Result<Ps4Applied, String>;
  pub(crate) fn read_content_entries(text: &str) -> Vec<BTreeMap<String, String>>;  // :227 lenient
  ```
  `apply_content` steps (verbatim from `apply_ps4_content_archive_without_ui`): platform check; expected id; `extracted_dir` checks (three messages); extract into `staging` (error → `Err(e.to_string())`); roots / mismatch / merge; append `{kind, title_id, archive_name, applied_at}` (kind = `kind.as_str()`, `applied_at` = unix seconds as a string) serialized compact (`serde_json::to_string`); delete archive (`fs::remove_file`; failure → warning `"Applied PS4 content, but could not delete archive:\n<path>\n<error>"`); `staging` removed on every exit after extraction (use a guard struct with `Drop`).

- [ ] **Step 1: Failing tests:** `normalize_title_id("cusa-12345") == Some("CUSA12345")`, `"CUSA1234"` → None; `detect_title_id` order (segment in launch path wins over top-level dir; falls back to archive stem); `select_ps4_launch_file` prefers the eboot under a top-level title-id dir, then shallower, then casefold path, and returns None with no `eboot.bin`; `expected_title_id` from explicit field, from `extracted_path` parents, from `extracted_dir` roots; `apply_content` happy path (staging with `CUSA12345/patch.txt` merges into `<extracted_dir>/CUSA12345/`, archive deleted, `content_json` has one entry with `kind == "update"` and `title_id`), each of the error strings (non-PS4 platform, missing id, missing dir, missing title dir, no roots, mismatch listing `"CUSA00001"`), archive-delete failure becomes the warning (make the archive a directory to force `remove_file` failure).
- [ ] **Step 2: Implement** per the Python; the extract closure is what the service will bind to `extract::extract_archive` with a progress sink.
- [ ] **Step 3:** tests, clippy, fmt.
- [ ] **Step 4: Commit** `rewrite: specials::ps4 — title-id detection, eboot ranking, content apply`

---

### Task 7: `specials::xenia` — STFS header and content apply

**Files:**
- Create: `rewrite/crates/grid-core/src/library/specials/xenia.rs`
- Test: inline

**Interfaces:**
- Produces:
  ```rust
  pub const STFS_HEADER_LEN: usize = 0x368;
  pub fn read_stfs_header(path: &Path) -> Option<(String, String)>; // (title_id_hex8, content_type_hex8), `{:08X}`
  #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
  pub struct XeniaApplied { pub title_id: String, pub content_type: String, pub destination: String }
  pub fn apply_content_file(file: &Path, content_root: &Path, expected_title_id: &str) -> Result<XeniaApplied, String>;
  pub fn apply_content_archive(archive: &Path, content_root: &Path, staging: &Path, expected_title_id: &str, extract: ExtractFn) -> Result<(Vec<XeniaApplied>, String), String>;
  pub fn build_stfs_bytes(magic: &[u8; 4], title_id: u32, content_type: u32) -> Vec<u8>; // test helper, pub for E2E-parity tests; 0x368 zero bytes with the fields set
  ```
  Destination `<content_root>/0000000000000000/<TitleID>/<ContentType>/<file name>`; copy with `fs::copy` (metadata preservation is best-effort: copy then set the source mtime via `filetime`-free `File::set_modified` on stable std). `apply_content_archive`: extract into `staging` (error → `Err(e.to_string())`), walk regular files sorted by path, collect `Ok`s and error strings; errors and no successes → `Err(joined by "\n")`; else `Ok((successes, joined))`; staging removed always.

- [ ] **Step 1: Failing tests:** header good (`LIVE`, title 0x415608C3, type 0x000B0000 → `("415608C3","000B0000")`), short file → None, bad magic → None; `apply_content_file` not found / bad magic / mismatch (expected `"41560000"` → `"Title ID mismatch: expected 41560000, archive contains 415608C3"`) / success path layout; `apply_content_archive` mixed success+error returns warning, all-error returns Err, staging removed.
- [ ] **Step 2: Implement** per `xenia.py:1-95` and `archive_preparation.py:781-830`.
- [ ] **Step 3:** tests, clippy, fmt.
- [ ] **Step 4: Commit** `rewrite: specials::xenia — STFS header, content apply`

---

### Task 8: `specials::native` — archive selection, game.json, executables, update merge

**Files:**
- Create: `rewrite/crates/grid-core/src/library/specials/native.rs`
- Test: inline

**Interfaces:**
- Consumes: `InstalledGame`, `RomFile`, `RomDetail`, `library::paths::{sanitize_component, candidate_archives}`, `library::launch_select::select_launch_file`, `specials::merge_tree`.
- Produces:
  ```rust
  pub const NATIVE_ARCHIVE_SUFFIXES: [&str; 9] = [".7z", ".zip", ".rar", ".tar", ".gz", ".tgz", ".xz", ".zst", ".bz2"];
  pub const NATIVE_GAME_SUFFIXES: [&str; 5] = ["exe", "bat", "cmd", "ps1", "sh"];
  pub fn select_archive(files: &[RomFile]) -> Option<&RomFile>;      // install_metadata.py:217
  pub fn has_game_json(files: &[RomFile]) -> Option<&RomFile>;       // top-level `game.json` (casefold)
  #[derive(Debug, Clone, Default, PartialEq, Eq)]
  pub struct GameJson { pub revision: String, pub first_release_date: String, pub tags: String, pub included_dlc: String, pub name: String }
  pub fn parse_game_json(bytes: &[u8]) -> Option<GameJson>;          // install_metadata.py:146 (None == Python's {})
  pub fn apply_game_json(row: &mut InstalledGame, parsed: &GameJson); // :189
  pub fn is_launchable_native_file(path: &Path) -> bool;
  pub fn install_dir(row: &InstalledGame, archive_candidates: &[PathBuf]) -> Option<PathBuf>; // install_paths.py:92
  pub fn executable_candidates(install_dir: &Path) -> Vec<PathBuf>;   // :114 sorted (component count, casefold)
  pub fn resolved_executable(row: &InstalledGame, candidates: &[PathBuf]) -> Option<PathBuf>; // :130
  pub fn update_temp_dir(row: &InstalledGame) -> PathBuf;             // install_mixin.py:806
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct NativeUpdate { pub row: InstalledGame, pub warning: String }
  pub fn apply_update(row: &InstalledGame, detail: &RomDetail, archive: &Path, temp_dir: &Path, extract: ExtractFn) -> Result<NativeUpdate, String>;
  ```
  `parse_game_json`: `version` → `revision` (`null` → ""; numbers stringified as JSON renders them); `year` else `release_year` coerced to an integer (a number → its integer part; a numeric string → parsed) else ""; `tags` non-empty list of strings joined `", "`; `included_dlc` = compact JSON of the list else `"[]"`; `name` stringified. `apply_game_json`: fill `revision`, `first_release_date`, `tags` only when the row's value is blank; always set `included_dlc`. `apply_update`: overwrite `rom_id` (Some(detail.id)), `rom_file_name` (= `detail.fs_name` when non-empty), `server_updated_at`, `description`, `rating`, `genres`, `regions`, `filesize_bytes` (when non-zero), `screenshot_urls` (joined "\n" when non-empty), `ra_id` (stays; RomDetail has no ra_id today — keep the row's); the two directory errors; merge = `merge_tree(temp_dir, extracted_dir)` after `extract(archive, temp_dir)`, temp_dir removed always (pre-existing temp dir removed first); re-detect via `select_launch_file(&extracted_dir, archive_stem)` and set `extracted_path` only when `native_executable_path` is blank; delete archive → warning `"Updated <title>, but could not delete archive:\n<path>\n<error>"`.

- [ ] **Step 1: Failing tests:** `select_archive` skips `game.json` and nested names and prefers the first archive suffix over an earlier `.mp3`; falls back to first top-level; `parse_game_json` matrix (invalid JSON → None; `{"version": 2}` → `"2"`; `year: "1998"` → `"1998"`; `year: "x"` → `""`; tags list, non-string tags → `""`; `included_dlc` list → compact JSON, missing → `"[]"`); `apply_game_json` fill-only-blank vs always-dlc; `executable_candidates` ordering (`a/z.exe` before `b/c/a.exe`; `.EXE` uppercase accepted; `.txt` ignored); `resolved_executable` pinned path wins only if it exists and is launchable, else first candidate; `install_dir` precedence; `apply_update` happy path (new file added, existing unrelated file preserved, exe re-detected when no pin, pinned exe keeps `extracted_path`), both error strings, archive-delete warning.
- [ ] **Step 2: Implement.**
- [ ] **Step 3:** tests, clippy, fmt.
- [ ] **Step 4: Commit** `rewrite: specials::native — archive selection, game.json, executables, update merge`

---

### Task 9: Install service — modes, typed entries, native/PS3/PS4 base finalize, uninstall branches

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/mod.rs`
- Modify: `rewrite/crates/grid-core/src/library/queue.rs`
- Modify: `rewrite/crates/grid-core/src/library/download.rs` (only if `FileTarget` needs a `content_type` hint — it does not; leave alone)
- Modify: `rewrite/crates/grid-core/src/library/extract.rs` (make `extract_iso_with_system_7z` reachable; done in Task 3)
- Test: `rewrite/crates/grid-core/tests/install_service.rs`, `queue.rs` inline tests

**Interfaces:**
- Consumes: Tasks 1–8 (`platforms`, `content`, registry v3, `should_extract`, `specials::*`), `autoconfig::readers::{ps3_vfs_dev_hdd0_path, ps3_vfs_games_path, rpcs3_data_root}`, `autoconfig::rpcs3::update_games_yml`, `autoconfig::ps3_library_path`, `launch::selection::{default_emulator_name_for_platform, emulator_entry_by_name}`, `launch::template::split_template`.
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
  #[serde(rename_all = "snake_case")]
  pub enum InstallMode { Base, Ps4Content, Xbox360Content, NativeUpdate }
  impl InstallMode { pub fn kind(self) -> &'static str /* "base" | "ps4_content" | "xbox360_content" | "native_update" */ }
  // queue.rs
  pub enum JobKey { Rom(i64), Content(i64, ContentKind), NativeUpdate(i64), Emulator(String), External(u64) }
  pub struct DownloadEntry { ..existing.., pub kind: &'static str }   // serialized as `kind`
  // QueueState::admit gains a `kind: &'static str` parameter; `admit_external(title, platform) -> u64` creates an entry
  // (kind "firmware", job "firmware", status Downloading, no bytes) that `next_ready()` never returns; `finish_external(id, error: &str)`.
  // InstallJob gains: mode: InstallMode, content_kind: Option<ContentKind>, file_ids: Vec<i64>,
  //                   native_game_dir: Option<PathBuf>, game_json_target: Option<PathBuf>
  // InstallService
  pub fn cancel_for_rom(&self, rom_id: i64);            // cancels the first live (Queued|Downloading|Installing) entry with that rom_id
  pub fn set_platform_ids(&self, ids: BTreeMap<String, i64>);   pub fn platform_ids(&self) -> BTreeMap<String, i64>;
  pub type GameFinalizedHook = Arc<dyn Fn(InstalledGame) + Send + Sync>;   pub fn set_game_finalized_hook(&self, f: GameFinalizedHook);
  pub fn admit_external(&self, title: &str, platform: &str) -> u64;   pub fn complete_external(&self, id: u64, error: &str);
  ```
  Base-install target rules added to `plan_install` (`mod.rs:219-285`):
  - **Native platform** (`is_native_platform`): archive = `specials::native::select_archive(&detail.files)` (None → `LibraryError::Extract("No downloadable file was found for this game")`); `native_game_dir = platform_dir/<sanitize_component(title,"game")>`; dest = `<native_game_dir>/<file_name>`; if `has_game_json` → a second `FileTarget` for it with dest `<native_game_dir>/game.json` (`game_json_target`). `multi_file_game_dir` stays None.
  - Everything else: unchanged (D12 filter from Task 1).
  Finalize dispatch (`finalize_inner`), in this order:
  1. Multi-file games: unchanged.
  2. Native base: if `should_extract(platform, archive)` → `dest = native_game_dir/game`; `extract_archive`; `select_launch_file(&dest, stem)` (None → remove dest, `NoLaunchFile`); `make_executable`; `record.extracted_path/extracted_dir`; `record.native_game_dir`; on non-Windows create `<native_game_dir>/prefix` and set `record.native_wineprefix`; archive deleted after upsert. Else (D13) `record.archive_path = archive`, `record.native_game_dir` set, no prefix. If `game_json_target` exists on disk: `parse_game_json(bytes)` → `apply_game_json(&mut record, ..)` (parse failure ignored).
  3. PS3 base, extractable: staging = `extraction_dir(archive)`; `extract_archive`; require at least one regular file under staging else remove staging and `NoLaunchFile`; `iso_only_file(staging)` → move ISO next to archive (overwrite), `extracted_path = ps3_iso_path = <iso>`, `extracted_dir = ""`, remove staging; else `Ps3Roots` via `self.ps3_roots(&detail.platform_name)` (below; `None` dev_hdd0 → `LibraryError::Extract(format!("No PS3 VFS dev_hdd0 path configured for {title}"))`) and `specials::ps3::route(&staging, &roots, &title, &|iso, dest| extract::extract_iso_with_system_7z(iso, dest))` → `Err(s)` → `LibraryError::Extract(s)`; fill `ps3_game_id`, `ps3_trophy_paths`, `extracted_path`, `extracted_dir`. PS3 base, NOT extractable: today's direct-file path plus `if suffix == "iso" { record.ps3_iso_path = archive }`.
  4. PS4 base (extractable): today's path but launch selection = `select_ps4_launch_file(&dest, &pool)` where pool = every regular file under dest; None → generic `select_launch_file`; then `record.ps4_game_id = detect_title_id(&dest, &launch, archive)`.
  5. Everything else: unchanged.
  After `self.registry.upsert(&record)`: existing image hook; then `if is_ps3_platform && !ps3_game_id.is_empty()` → `self.write_games_yml(&record)` (data root from `rpcs3_data_root(entry.path)` where entry = default PS3 emulator; skip silently when any of data_root/dev_hdd0 is None; `update_games_yml(data_root, game_id, dev_hdd0, games_root.as_deref())`); then the game-finalized hook (`Base` mode only) with `record.clone()`; then archive deletion.
  `fn ps3_roots(&self, platform: &str) -> Result<Ps3Roots, String>`: load `Config`; `ps3_library = autoconfig::ps3_library_path(&config.library_path)`; `name = default_emulator_name_for_platform(&config.emulators, &config.default_emulators, platform, &self.profiles, &config.retroarch_cores)`; `entry = emulator_entry_by_name(&config.emulators, &name)`; `(path, args)` = entry's `path` and `split_template(&entry.args).unwrap_or_default()` or `("", vec![])`; `dev_hdd0 = ps3_vfs_dev_hdd0_path(path, &args, &ps3_library)` (None → Err message); `games_root = ps3_vfs_games_path(..)`; `data_root = entry.map(|e| rpcs3_data_root(&e.path)).flatten()` (D4).
  Uninstall (`mod.rs:567-604`) becomes branch-by-platform per `install_cleanup.py` with D11 aggregation: collect every step failure as `"Could not remove file: <path>\n<error>"` / `"Could not remove PS3 trophy directory: <path>\n<error>"` / `"Could not remove folder: <path>\n<error>"` lines; continue past failures; if any line → `Err(LibraryError::Registry(lines.join("\n")))` and the row stays; PS3: iso, trophy dirs (lenient JSON), candidate extracted dirs; native: `native_game_dir` if dir → remove and stop, else candidate extracted dirs; others: unchanged logic.
  `DownloadEntry.kind` for game jobs = `mode.kind()`, emulator jobs `"emulator"` (Task 12 adds `"compat_tool"`).

- [ ] **Step 1: Failing tests** (`tests/install_service.rs`, using the existing `Harness`; extend `detail_json` to accept a platform name and files with `category`; helper `write_zip` exists):
  - `entry_carries_kind_base`.
  - `native_install_lays_out_game_dir_prefix_and_game_json`: platform `"Windows"`, files `[{id:1, "mygame.zip", top}, {id:2, "game.json", top}]`; mount both; zip holds `MyGame/mygame.exe` + `readme.txt`; game.json `{"version":"1.2","year":2001,"tags":["a","b"],"included_dlc":["x"]}`. Assert: `<library>/Windows/<Title>/game/MyGame/mygame.exe` exists; `<library>/Windows/<Title>/prefix` is a dir (cfg unix); archive deleted; row has `native_game_dir`, `extracted_dir == .../game`, `extracted_path == .../mygame.exe`, `native_wineprefix`, `revision == "1.2"`, `first_release_date == "2001"`, `tags == "a, b"`, `included_dlc == "[\"x\"]"`.
  - `native_non_archive_payload_installs_as_direct_file` (D13): `game.iso` → `archive_path` set, no `game/`.
  - `ps3_install_routes_into_the_library_vfs_fallback`: platform `"PlayStation 3"`, config with `library_path` only (no emulators) → dev_hdd0 = `<library>/PlayStation 3/.vfs/dev_hdd0`; zip `BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN`; assert routed path exists, staging gone, row `ps3_game_id == "BLUS30336"`, `extracted_dir == <dev_hdd0>/game/BLUS30336`, archive deleted, `ps3_trophy_paths == "[]"`.
  - `ps3_iso_only_archive_short_circuits`: zip with one `game.iso` → ISO beside archive, row `ps3_iso_path`, `extracted_dir == ""`.
  - `ps3_without_dev_hdd0_fails_with_the_verbatim_message` (config without library path is already `LibraryPathUnset`; instead seed a config whose `library_path` is set but pass a platform… — simplest: temporarily point `ps3_library_path` blank by building the harness `without_library_path()` is the wrong error. Use a custom harness config with `library_path` set and a default PS3 emulator entry whose `vfs.yml` sets `/dev_hdd0/` to a blank scalar; if the readers then return the library fallback, drop this test and cover the message via a `ps3_roots` unit test with an empty `ps3_library`).
  - `ps3_game_id_missing_fails`: zip with only `readme.txt` inside `misc/` → `Failed` with `"No PS3 game ID found in archive for <title>"`.
  - `ps4_install_detects_title_id_and_prefers_eboot`: zip `CUSA12345/eboot.bin`, `CUSA12345/sce_sys/param.sfo` → `ps4_game_id == "CUSA12345"`, `extracted_path` ends with `eboot.bin`.
  - `uninstall_native_removes_the_game_dir` and `uninstall_ps3_removes_iso_trophies_and_dir_and_aggregates_failures` (make one trophy dir unremovable by turning it into a file path mismatch — e.g. list a trophy path that is a file → `"Could not remove PS3 trophy directory"` is NOT raised for non-dirs (skipped), so instead assert aggregation by pre-creating a read-only parent on unix… if too fiddly, assert the happy path plus a unit test of the aggregation helper with an injected failing remover).
  - `games_yml_written_for_ps3_with_configured_rpcs3` (config with an RPCS3 entry whose path is a temp `rpcs3` file with a `portable/` dir beside it → `portable/config/games.yml` contains `BLUS30336:`).
  - `cancel_for_rom_cancels_the_live_entry`, `admit_external_and_complete_external_round_trip` (kind `"firmware"`, status → Completed / Failed with error).
- [ ] **Step 2: Implement** per the interface notes. Keep `finalize_inner` readable by extracting `finalize_native_base`, `finalize_ps3_base`, `finalize_ps4_base` helpers.
- [ ] **Step 3:** full crate tests, clippy, fmt, hygiene.
- [ ] **Step 4: Commit** `rewrite: install modes, typed download entries, native/PS3/PS4 base finalize, games.yml, uninstall branches (D11–D13)`

---

### Task 10: Install service — content jobs (PS4/Xbox 360/native update), Xbox auto-queue

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/mod.rs`, `queue.rs`
- Modify: `rewrite/crates/grid-core/src/launch/profiles.rs` (nothing; `profile_available_on_host` from Task 4)
- Test: `rewrite/crates/grid-core/tests/install_service.rs`

**Interfaces:**
- Consumes: Task 9, `specials::{ps4, xenia, native}`, `content`, `autoconfig::readers::xenia_directory_settings`, `launch::profiles::{profile_for_entry, profile_available_on_host}`, `launch::source::HOST_PLATFORM`.
- Produces:
  ```rust
  pub async fn install_content(self: &Arc<Self>, client: Arc<RommClient>, rom_id: i64, kind: ContentKind) -> Result<(), LibraryError>;
  pub async fn install_native_update(self: &Arc<Self>, client: Arc<RommClient>, rom_id: i64) -> Result<(), LibraryError>;
  ```
  `install_content`: row must exist (`registry.find(Some(rom_id),"","")` + `installed_match`) else `LibraryError::Registry("not installed")`; `detail = client.rom_detail(rom_id)`; `file_ids = content_file_ids(&detail.files, kind)` empty → `LibraryError::Extract(format!("No PS4 {kind} files were found for this title in server metadata."))` for PS4 platforms, and `"No Xbox 360 {kind} content is available for this title."` for Xbox 360; any other platform → `LibraryError::Extract("Update/DLC content is only supported for PS4 and Xbox 360 games")`. Job: `mode = Ps4Content | Xbox360Content` by platform, `content_kind = Some(kind)`, `file_ids`, one `FileTarget { url_path: /api/roms/{id}/content/{encode(detail.fs_name)}, query: [("file_ids", csv)], dest: platform_dir/<sanitize_component(title, "ps4-content"|"xbox360-content")>-<kind>.zip }`; `JobKey::Content(rom_id, kind)`; entry title `"{title} ({kind})"`, kind string from mode.
  `install_native_update`: row must be native with a non-blank `extracted_dir` that is a dir (else `LibraryError::Extract("Game install directory could not be found. Reinstall the game and try again.")`); `detail`; archive = `select_archive(&detail.files)`; dest = `<native_game_dir or extracted_dir parent>/<file_name>`; `JobKey::NativeUpdate(rom_id)`; title `"{title} (update)"`.
  Finalize by mode:
  - `Ps4Content`: `staging = extraction_dir(archive)`; `ps4::apply_content(&row, archive, kind, &staging, &extract_fn)` → `Err(s)` → `LibraryError::Extract(s)`; `Ok(applied)` → `registry.update_ps4_content(rom_id, &applied.game_id, &applied.content_json)`; warning appended. No new row, no image hook, no game-finalized hook.
  - `Xbox360Content`: `content_root` via `fn xenia_content_root(&self, platform) -> Result<PathBuf, String>`: config; default emulator name for the platform; on non-Windows hosts: no name → the "requires a Linux-compatible emulator" message; `profile_for_entry(name, path, profiles)` present and `!profile_available_on_host(profile, HOST_PLATFORM)` → the "only runs on Windows" message; `settings = xenia_directory_settings(&entry.path, &split_template(&entry.args).unwrap_or_default())`; blank `content_root` → `"Could not determine Xenia content directory. Is Xenia configured?"`. Then `xenia::apply_content_archive(archive, &root, &staging, "", &extract_fn)`; `Err(s)` → `LibraryError::Extract(s)`; `Ok((_, warning))` → append; delete archive (D16).
  - `NativeUpdate`: `native::apply_update(&row, &detail, archive, &native::update_temp_dir(&row), &extract_fn)` → `Ok(u)` → `registry.upsert(&u.row)`, warning appended; `Err(s)` → `LibraryError::Extract(s)`.
  Completion for `Base` on an Xbox 360 platform (in the finalize-success path, after the hook): for `kind in [Update, Dlc]`, `ids = content_file_ids(&job.detail.files, kind)`; non-empty → build the same content job as `install_content` would (no server round trip; reuse a `fn content_job(&self, client, detail, kind, ids) -> (JobKey, String /*title*/, JobPayload)` helper) and `admit` it silently.
  `finalize` for `Ps4Content`/`Xbox360Content`/`NativeUpdate` must load the current row fresh from the registry (not the detail) — the row may have changed since admission.

- [ ] **Step 1: Failing tests:**
  - `install_content_requires_an_installed_row`, `install_content_rejects_unsupported_platform`, `install_content_with_no_files_of_that_kind_fails_with_the_platform_message`.
  - `ps4_update_applies_and_records_content`: base install (as Task 9 test) then `install_content(.., Update)` with `files` having an `update` category file; mount the content URL with `query_param("file_ids","1002")`; the update zip holds `CUSA12345/patch.txt`; assert the entry `kind == "ps4_content"`, title `"<title> (update)"`, `patch.txt` merged, archive gone, row `ps4_content` JSON has one entry `kind:"update"`.
  - `ps4_update_title_mismatch_fails_with_message` (`CUSA00001/...` → `"PS4 content title ID mismatch: expected CUSA12345, archive contains CUSA00001"`).
  - `xbox360_base_install_queues_update_then_dlc_silently`: config with an emulator named `"Xenia Edge"` (path: temp file; `portable.txt` beside it so the content root resolves to `<dir>/content` — verify against `xenia_directory_settings` and adjust the seed to whatever yields a non-empty `content_root`), default for `"Xbox 360"`; detail files `[game(3001), update(3002), dlc(3003)]`; base zip `default.xex`; update zip one STFS file built with `xenia::build_stfs_bytes(b"LIVE", 0x415608C3, 0x000B0000)` named `tu00000001`; dlc zip another STFS with type `0x00000002`. Assert three entries complete in order (base, update, dlc), kinds `xbox360_content`, files at `<content>/0000000000000000/415608C3/000B0000/tu00000001` and `.../00000002/<name>`, content archives deleted.
  - `xbox360_content_without_emulator_fails_with_the_linux_message` (cfg unix).
  - `native_update_merges_and_keeps_pinned_executable` (base native install; `update_native_settings` pins the exe; update zip adds `data/new.txt` and a different exe → `extracted_path` unchanged, `new.txt` present, entry kind `native_update`).
- [ ] **Step 2: Implement.**
- [ ] **Step 3:** tests, clippy, fmt, hygiene.
- [ ] **Step 4: Commit** `rewrite: content jobs — PS4/Xbox 360 update+DLC, native update, Xbox auto-queue (D16)`

---

### Task 11: Launch — native Wine/Proton branch, compat discovery, PS3 launch target

**Files:**
- Create: `rewrite/crates/grid-core/src/launch/native.rs`
- Create: `rewrite/crates/grid-core/src/launch/compat.rs`
- Modify: `rewrite/crates/grid-core/src/launch/mod.rs` (`pub mod native; pub mod compat;` native branch in `launch`; `ps3_launch_target` in `resolve_launch`)
- Modify: `rewrite/crates/grid-core/src/library/extract.rs` (`which_on_path` stays `pub(crate)`; re-export as `pub(crate) use crate::library::extract::which_on_path` where needed)
- Test: inline + `rewrite/crates/grid-core/tests/launch_service.rs`

**Interfaces:**
- Consumes: registry v3 row fields, `specials::native::{install_dir, executable_candidates, resolved_executable}`, `library::paths::candidate_archives`, `template::split_template`, `spawn::clean_env`, `config::{Config, CompatToolInstall}`, `config::data_dir_override`.
- Produces:
  ```rust
  // native.rs
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct NativeLaunch { pub argv: Vec<String>, pub cwd: PathBuf, pub env: Vec<(String, String)>, pub tool_label: String /* "wine" | "<compat path>" | "" */ }
  pub fn build_native_command(row: &InstalledGame, library: &Path, default_compat_tool: &str, host: &str, which: &dyn Fn(&str) -> Option<PathBuf>) -> Result<NativeLaunch, String>;
  // compat.rs
  #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
  pub struct CompatTool { pub name: String, pub kind: String /* "wine" | "proton" */, pub path: String, pub source: String /* "system" | "steam" | "managed" */ }
  pub fn steam_roots(home: &Path) -> [PathBuf; 3];
  pub fn managed_root() -> PathBuf;   // D15
  pub fn discover(home: &Path, managed: &[CompatToolInstall], host: &str, which: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<CompatTool>;
  pub fn find_proton_dir(install_dir: &Path) -> Option<PathBuf>; // install_dir itself if it has a `proton` file, else its first (sorted) child dir that does
  ```
  `build_native_command` (`launch.py:223-272`): executable = `resolved_executable(row, &executable_candidates(install_dir))` with `install_dir(row, &candidate_archives(library, platform, archive_path, archive_name(..)))`; None → the verbatim "No launchable native executable…" error; args = `split_template(native_launch_parameters.trim())` (Err(e) → `"Invalid custom launch parameters: {e}"`); tool = row's `native_compat_tool.trim()` else `default_compat_tool.trim()` (caller passes `""` on Windows hosts); `"wine"` → argv `[which("wine") or "wine", exe, args..]`; other non-empty → `which("umu-run")` None → the verbatim umu message, argv `[umu, exe, args..]`, env `PROTONPATH=<tool>`; both branches add `WINEPREFIX` when `native_wineprefix` is non-blank (`fs::create_dir_all` it); blank tool → `[exe, args..]`; `cwd = exe.parent()`.
  `discover`: host starts with `win` → empty; `which("wine")` → `CompatTool{ name:"Wine (system)", kind:"wine", path:"wine", source:"system" }`; Steam roots (`~/.steam/steam/compatibilitytools.d`, `~/.local/share/Steam/compatibilitytools.d`, `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d`) sorted subdirs containing a `proton` file, `canonicalize`d, dedup by canonical path string, skipping paths equal to a managed install path; then managed installs with non-blank paths (`kind: "proton"`, `source: "managed"`).
  `LaunchService::launch` native branch (replaces the refusal at `mod.rs:172-177`): load config; `default_compat_tool = if host_os starts with "win" { "" } else { config.default_compat_tool }`; `library = paths::expand_home(&config.library_path)`; `build_native_command(.., &which_on_path)` → `Err(s)` → `LaunchError::Validation(s)`; spawn `Command::new(&argv[0]).args(..).current_dir(cwd).env_clear().envs(clean_env()).envs(env)`; session `emulator_name` = `tool_label` (`"wine"`, the Proton path, or `"native"`); duplicate check, early-exit watch, notify as for emulators.
  `resolve_launch`: `ps3_launch_target = if !row.ps3_iso_path.trim().is_empty() { row.ps3_iso_path.trim() } else if !row.ps3_game_id.trim().is_empty() { format!("%RPCS3_GAMEID%:{}", row.ps3_game_id.trim()) } else { "" }`.

- [ ] **Step 1: Failing tests:** `build_native_command` plain/wine/proton/env/cwd/prefix-creation/missing-umu/invalid-params/no-executable (temp dirs, fake `which` closures); `discover` builds a fake `home` with two Steam roots (one a symlink to the other → dedup), a managed entry, and a `which` returning Some for wine; Windows host → empty; `find_proton_dir` nested case. `tests/launch_service.rs` (unix): installed `"Windows"` row with `game/MyGame/mygame.exe` (a recording stub), config `default_compat_tool = "wine"` and a `wine` stub found via a temp `PATH` (set `PATH` env for the test; restore) → the session starts; the wine stub's argv file lists the exe path and the params; `LaunchService` snapshot `emulator_name == "wine"`; an RPCS3 row with `ps3_game_id = "BLUS30336"` and a template `%ps3_launch_target%` → argv contains `%RPCS3_GAMEID%:BLUS30336`; with `ps3_iso_path` → the ISO path.
- [ ] **Step 2: Implement.** Delete the "not supported yet" refusal and its test; update `launch/mod.rs` doc comment.
- [ ] **Step 3:** tests, clippy, fmt, hygiene.
- [ ] **Step 4: Commit** `rewrite: native launch via Wine/Proton, compat-tool discovery, PS3 launch target (closes doc 04 deviation 3)`

---

### Task 12: Managed compat-tool installs

**Files:**
- Modify: `rewrite/crates/grid-core/src/launch/catalog.rs` (`compat_tool_catalog_entries`, `find_compat_profile`, `mark_compat_installed`)
- Modify: `rewrite/crates/grid-core/src/library/mod.rs` (`EmulatorJob.compat_tool`, `install_compat_tool`, finalize branch, hook)
- Modify: `rewrite/crates/grid-core/src/launch/emu_install.rs` (`compat_tool_install_dir`)
- Test: `rewrite/crates/grid-core/tests/emulator_install.rs`, catalog inline tests

**Interfaces:**
- Consumes: Task 4 (`compat_tool_type`, `CompatToolInstall`), Task 11 (`compat::{managed_root, find_proton_dir}`), existing forge/download machinery.
- Produces:
  ```rust
  pub fn compat_tool_catalog_entries(profiles: &[EmulatorProfile]) -> Vec<CatalogEntry>;  // is_compat_tool only, same shape/sort as catalog_entries
  pub fn mark_compat_installed(entries: &mut [CatalogEntry], config: &Config);              // installed = any compat_tool_installs.source_id matches
  pub fn find_compat_profile<'a>(profiles: &'a [EmulatorProfile], source_id: &str) -> Option<&'a EmulatorProfile>;
  pub fn compat_tool_install_dir(root: &Path, archive_stem: &str) -> PathBuf;              // root.join(sanitize_component(stem, "compat-tool"))
  pub async fn install_compat_tool(self: &Arc<Self>, source_id: String) -> Result<(), LibraryError>;
  pub type CompatToolsHook = Arc<dyn Fn() + Send + Sync>;  pub fn set_compat_tools_hook(&self, f: CompatToolsHook);
  pub struct EmulatorInstalled { pub name: String, pub fresh: bool, pub compat_tool: bool }
  pub type EmulatorInstalledHook = Arc<dyn Fn(EmulatorInstalled) + Send + Sync>;  pub fn set_emulator_installed_hook(&self, f: EmulatorInstalledHook);
  ```
  `install_compat_tool`: profile via `find_compat_profile` (None → `LibraryError::Extract("unknown compat tool source")`); `EmulatorJob { compat_tool: true, library: compat::managed_root(), .. }`; `JobKey::Emulator(source_id)`; entry `job: "emulator"`, `kind: "compat_tool"`, platform label `"Compatibility Tool"`. Download path: `install_dir = compat_tool_install_dir(&job.library, &stem)` when `compat_tool`. `finalize_emulator` with `compat_tool`: extract (as today), skip `select_executable`/`write_emulator_entry`/`sync_autoconfig`; `path = compat::find_proton_dir(install_dir)` (None → `LibraryError::Extract("Downloaded compatibility tool has no `proton` entry point")`); `modify config`: replace-or-append `CompatToolInstall { name: profile.name, path, source_id, release_tag: resolved tag }` by `source_id`; delete archives; call the compat-tools hook. `finalize_emulator` for ordinary emulators: after `write_emulator_entry` determine `fresh` (no entry with that name existed before the write) and call the emulator-installed hook after `sync_autoconfig` with `{ name, fresh, compat_tool: false }`.

- [ ] **Step 1: Failing tests:** catalog: real catalog `compat_tool_catalog_entries` contains `GE-Proton` and `Proton-CachyOS` and nothing else; `find_compat_profile("GloriousEggroll/proton-ge-custom")`; `mark_compat_installed`. `tests/emulator_install.rs`: a compat profile (`is_compat_tool: true`, `compat_tool_type: "proton"`, gitea source at the mock) whose asset is a tar.gz with `GE-Proton9-1/proton` (mode 755) and `GE-Proton9-1/version` → after `install_compat_tool`, entry kind `compat_tool` Completed; `<root>/<stem>/GE-Proton9-1/proton` exists; config `compat_tool_installs[0] == { name, path: <root>/<stem>/GE-Proton9-1, source_id, release_tag }`; no `[[emulators]]` entry added; hook fired once. Set `GRID_LAUNCHER_DATA_DIR` to the temp dir for the test so `managed_root()` lands inside it (D15). Also: `emulator_installed_hook_reports_fresh_true_then_false` on two installs of the same profile.
- [ ] **Step 2: Implement.**
- [ ] **Step 3:** tests, clippy, fmt, hygiene.
- [ ] **Step 4: Commit** `rewrite: managed compat-tool installs, compat catalog, emulator-installed hook (D7, D15)`

---

### Task 13: `firmware` — targets, routing rules, `install_platform_firmware`

**Files:**
- Create: `rewrite/crates/grid-core/src/firmware/mod.rs` (types, `resolve_targets`, `should_keep_zip`, `install_platform_firmware`)
- Create: `rewrite/crates/grid-core/src/firmware/write.rs` (per-file dispatch)
- Modify: `rewrite/crates/grid-core/src/lib.rs` (`pub mod firmware;`)
- Test: inline (routing, keep-zip, write dispatch with temp dirs) + `rewrite/crates/grid-core/tests/firmware_install.rs` (wiremock end-to-end; maps `tests/test_firmware_install.py` classes `FirmwareRoutingTests`, `FirmwareInstallTests`, `FirmwareZipArchiveBehaviorTests`, `FirmwareExtractZipWithPathsTests`)

**Interfaces:**
- Consumes: `romm::RommClient::{firmware, firmware_bytes}`, `library::extract::extract_archive` (for `.7z`/`.rar`).
- Produces:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct FirmwareTarget { pub path: PathBuf, pub keywords: Option<Vec<String>> } // keywords already lower-cased
  #[derive(Debug, Clone, Copy)]
  pub struct FirmwareOptions { pub skip_existing: bool, pub extract_zip_with_paths: bool }
  impl Default for FirmwareOptions { skip_existing: true, extract_zip_with_paths: false }
  pub fn resolve_targets<'a>(file_name: &str, targets: &'a [FirmwareTarget]) -> Vec<&'a FirmwareTarget>;   // firmware_install.py:38
  pub fn should_keep_zip(file_name: &str, targets: &[FirmwareTarget], applicable: &[&FirmwareTarget]) -> bool; // :60
  pub async fn install_platform_firmware(client: &RommClient, platform_id: i64, targets: &[FirmwareTarget], opts: FirmwareOptions) -> Vec<String>;
  // write.rs
  pub(crate) fn write_firmware_file(file_name: &str, data: &[u8], target_dir: &Path, keep_archive: bool, opts: FirmwareOptions) -> Result<(), String>;
  pub(crate) fn is_zip_bytes(data: &[u8]) -> bool;   // "PK\x03\x04" | "PK\x05\x06" | "PK\x07\x08"
  ```
  `install_platform_firmware` (steps 1–9 of doc 03 §18 verbatim): empty targets → `[]`; `client.firmware(platform_id)` error → `[format!("Firmware fetch failed for platform {platform_id}: {e}")]`; empty → `[]`; per record: skip `file_name` blank; `applicable = resolve_targets`; empty → continue; `keep = !opts.extract_zip_with_paths && lower.ends_with(".zip") && should_keep_zip(..)`; `client.firmware_bytes(id, &file_name)` error → warning `"Failed to download firmware {name}: {e}"` continue; per target: `create_dir_all` failure → `"Could not create firmware directory {dir}: {e}"` continue; `write_firmware_file` error → push, continue. Every `Err` from `write_firmware_file` is already the full warning text.
  `write_firmware_file`: `.7z`/`.rar` → temp file + temp staging via `extract_archive`, walk regular files, skip any path component `__MACOSX` and file name `.DS_Store`, flat copy to `<target>/<file name>` honoring `skip_existing`; errors → `"Failed to extract firmware archive {name}: {e}"`. `.zip` or `is_zip_bytes` → keep → write raw bytes (skip if exists & skip_existing) error `"Failed to write firmware {name} to {dest}: {e}"`; else iterate `zip::ZipArchive` entries: skip names ending `/` or starting `__MACOSX`; with-paths: `\`→`/`, reject empty/`..`/absolute, `create_dir_all(parent)`, write; flat: `Path::file_name`, skip empty; each write honors `skip_existing`; errors → `"Failed to extract firmware archive {name}: {e}"`. Else raw write with the write message.

- [ ] **Step 1: Failing tests (inline):** the eight `FirmwareRoutingTests` cases; the three keep-zip cases (`naomi.zip` routed through `["naomi.zip"]` → keep; plain target → extract; keywords `["naomi"]` → extract); write dispatch: with-paths nested, traversal member skipped (`../../outside` never lands), `__MACOSX` skipped both modes, flat mode, skip_existing per member, non-zip raw write, bad zip warning text, 7z flat copy via a real `.7z` fixture built with `sevenz-rust2` in the test (skip `.DS_Store`, preserve unrelated existing files, overwrite when `skip_existing=false`).
- [ ] **Step 2: Failing tests (`tests/firmware_install.rs`, wiremock):** `no_targets_never_fetches`; `empty_list_no_warnings`; `single_file_written`; `skip_existing_still_downloads_but_does_not_write` (assert `received_requests()` shows the content GET); `zip_extracted_flat`; `download_once_for_multiple_dirs`; `download_error_is_a_warning` (500 → `"Failed to download firmware gc.bin: ..."`); `fetch_error_is_a_warning` (500 on list); `record_missing_id_or_blank_name_skipped`; `routed_zip_to_correct_region_dir` (JAP/USA/EUR, non-matching dirs never created); `non_list_body_yields_no_warnings`; `fetch_uses_platform_id_query` (`query_param("platform_id","19")`).
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** tests, clippy, fmt, hygiene (no URL with query logged).
- [ ] **Step 5: Commit** `rewrite: firmware — target routing, keep/flat/with-paths writes, install_platform_firmware`

---

### Task 14: `firmware::routing` — entry targets, RetroArch/Cemu shaping, per-game install, profile platform ids, RPCS3 PUP

**Files:**
- Create: `rewrite/crates/grid-core/src/firmware/routing.rs`
- Create: `rewrite/crates/grid-core/src/firmware/rpcs3.rs`
- Modify: `rewrite/crates/grid-core/src/autoconfig/mod.rs` (delete the D7 test `sync_starts_no_firmware_download` at :1424-1459 and the `// No background firmware download — D7.` comment; nothing else)
- Test: inline (temp dirs; real catalog + real `retroarch-core-list.json`), maps the remaining `test_firmware_install.py` classes (`CemuFirmwareRoutingTests`, `EdenFirmwareRoutingTests`, `XemuFirmwareRoutingTests`, `Rpcs3FirmwareRoutingTests`, `RetroArchFirmwareDirectoryRoutingTests`, `RetroArchConfigFileDirsFilteringTests`, `RetroArchSavesFileDirsAssemblyTests`, `RetroArchFirmwareDirsFilteringTests`, the three metadata presence classes are already covered by `autoconfig::cores` tests — verify and add any missing case there)

**Interfaces:**
- Consumes: Task 4 (`FirmwareDirSpec`, `profile_for_entry`), Task 13, `autoconfig::{is_retroarch, is_cemu, is_dolphin, is_rpcs3}`, `autoconfig::cores::{core_entries, core_firmware_metadata, core_config_files_metadata, core_saves_files_metadata, cores_for_platform, compatibility_map}`, `autoconfig::retroarch::directory_settings`, `autoconfig::dolphin::{ensure_skip_ipl, ensure_gcpad_config}`, `autoconfig::paths::expand_user`, `launch::selection::{default_emulator_name_for_platform, emulator_entry_by_name, mapping_value_for_platform}`, `launch::profiles::platform_matches_keywords`, `launch::spawn::clean_env`.
- Produces:
  ```rust
  pub fn targets_for_entry(entry: &EmulatorEntry, profile: Option<&EmulatorProfile>, library_dir: &str, config_dir: &Path) -> Vec<FirmwareTarget>; // cloud_mixin.py:1032
  #[derive(Debug, Default, Clone, PartialEq, Eq)]
  pub struct RetroArchPlan { pub firmware: Vec<FirmwareTarget>, pub extract_with_paths: bool, pub configs: Vec<FirmwareTarget>, pub saves: Vec<FirmwareTarget> }
  pub fn shape_for_retroarch(core_id: &str, entries: &[CoreEntry], emulator_dir: &Path, savefile_directory: &str, firmware: Vec<FirmwareTarget>) -> Option<RetroArchPlan>; // None == Python's early `return ""` (no metadata → firmware=[] but still a plan; None only when core_id is blank)
  pub fn shape_for_cemu(firmware: Vec<FirmwareTarget>) -> Vec<FirmwareTarget>;  // plain → keywords ["keys.txt"]
  pub struct GameFirmwareContext<'a> { pub platform: &'a str, pub platform_id: i64, pub config: &'a Config, pub profiles: &'a [EmulatorProfile], pub config_dir: &'a Path }
  pub async fn install_for_game(client: &RommClient, ctx: &GameFirmwareContext<'_>) -> String;  // install_mixin.py:528; "" when nothing to do
  pub fn platform_ids_for_profile(profile: &EmulatorProfile, entry: &EmulatorEntry, platforms: &BTreeMap<String, i64>) -> Vec<i64>; // emulator_ui_mixin.py:1865-1890
  pub fn emulator_dir_of(entry: &EmulatorEntry) -> PathBuf; // parent of the expanded path, or the path when it is a dir
  // rpcs3.rs
  pub fn rpcs3_pup_path(emulator_path: &str) -> Option<PathBuf>;            // rpcs3.py:274
  pub fn spawn_rpcs3_installfw(exe: &Path, pup: &Path) -> bool;             // rpcs3.py:365
  pub fn ps3_platform_id(platforms: &BTreeMap<String, i64>) -> Option<i64>; // key casefold contains "playstation 3" or == "ps3"
  ```
  `targets_for_entry`: profile `firmware_directories` (empty → `[]`); `emulator_dir = emulator_dir_of(entry)` (blank path → empty); for each spec: `expanded = env-var expansion` (implement `%VAR%` on Windows and `$VAR`/`${VAR}` on unix via a small helper; use `std::env::var`), then replace `%EMULATOR_DIR%`, `%LIBRARY_DIR%` (= `expand_user(library_dir)` or "" when blank), `%CONFIG_DIR%`; `expand_user`; relative and emulator_dir non-empty → `emulator_dir.join(candidate)` then best-effort canonicalize (`fs::canonicalize` falling back to the joined path); absolute → same canonicalize; dedup by `to_string_lossy().to_lowercase()` keeping the first. `shape_for_retroarch` (install_mixin.py:552-631): metadata None → `firmware = []`; `subdirectory` non-blank → every target path joined; `files` non-empty → plain targets become keyworded with those names lower-cased; `extract_with_paths`; configs: `base_dir` → `[(emulator_dir/base_dir, files?)]`; saves: `file` → `saves_dir` from `savefile_directory`: blank or `default` → `emulator_dir/saves`; `:\`/`:/` prefix → `emulator_dir/<rest>`; else expand_user, relative → under emulator_dir; `[(saves_dir, [file])]`. `install_for_game`: blank platform / no id / no default emulator / no entry → `""`; targets; RetroArch (`is_retroarch(entry, profiles)`) → `configured_core = mapping_value_for_platform(&config.retroarch_cores, platform)` blank → `""`; plan; Cemu → `shape_for_cemu`; nothing to do → `""`; three `install_platform_firmware` calls (firmware with plan opts; configs default opts; saves with `extract_zip_with_paths: true`), each wrapped: the function never panics, but wrap the calls in `catch_unwind`-free style — errors already come back as warnings; Dolphin → `ensure_skip_ipl` + `ensure_gcpad_config` ignoring results; return `warnings.join("\n")`. `platform_ids_for_profile`: `all_platforms && is_retroarch` → ids of platforms with `!cores_for_platform(name, compatibility_map()).is_empty()`; `all_platforms` → all ids; else `platform_keywords` non-empty → ids of platforms where `platform_matches_keywords(name, &keywords)`; empty keywords → `[]`. `spawn_rpcs3_installfw`: exe and pup must be files (canonicalize both); `Command::new(exe).args(["--installfw", pup]).current_dir(exe.parent()).env_clear().envs(clean_env()).spawn().is_ok()`.

- [ ] **Step 1: Failing tests:** `targets_for_entry` with the real Eden profile → two keyworded targets under `<emulator dir>/user/keys` and `.../registered`; RetroArch → `[<dir>/system]`; RPCS3 `["."]` → `[<dir>]`; `%LIBRARY_DIR%` and `~` expansion; dedup case-insensitive; the `Rpcs3FirmwareRoutingTests` (`PS3UPDAT.PUP` routes to `.`; `rpcs3_pup_path` present/absent); `shape_for_retroarch` cases from the four RetroArch test classes (subdirectory appended to plain and keyworded; null leaves unchanged; files list wraps plain only; config dirs tuple vs plain; saves dir `default`, `:\rel`, absolute, relative; Dolphin real metadata → `extract_with_paths == true`, subdirectory `dolphin-emu/Sys`); `shape_for_cemu`; `platform_ids_for_profile` for RetroArch/all_platforms/keywords; `ps3_platform_id`; `install_for_game` wiremock happy path (DuckStation entry, `bios` dir receives `scph5501.bin`) and early-return cases return `""` without any request (assert `received_requests().is_empty()`).
- [ ] **Step 2: Implement.**
- [ ] **Step 3:** tests, clippy, fmt, hygiene.
- [ ] **Step 4: Commit** `rewrite: firmware routing — entry targets, RetroArch/Cemu shaping, install_for_game, profile platform ids, RPCS3 PUP helpers (closes doc 05 D7)`

---

### Task 15: App layer — commands, `FirmwareService`, triggers, events, `api.ts`

**Files:**
- Create: `rewrite/app/src-tauri/src/firmware_service.rs`
- Create: `rewrite/app/src-tauri/src/commands/specials.rs` (content, native settings, compat tools, RPCS3 firmware, cancel-for-rom commands)
- Modify: `rewrite/app/src-tauri/src/commands.rs` (`pub mod specials;`, `AppState.firmware`, `launch_game` native + firmware-before-launch, `list_platforms` feeds `set_platform_ids`, `save_emulator` RPCS3 trigger)
- Modify: `rewrite/app/src-tauri/src/lib.rs` (hooks, handler list)
- Modify: `rewrite/app/src/lib/api.ts`
- Test: `cargo test -p app` inline where pure; `npm run check`

**Interfaces:**
- Consumes: Tasks 9–14.
- Produces (Tauri commands; all `Result<_, String>`):
  ```rust
  install_content(rom_id: i64, kind: String /* "update" | "dlc" */) -> ()
  install_native_update(rom_id: i64) -> ()
  content_availability(rom_id: i64) -> ContentAvailability                // via client.rom_detail; not connected → Err("not connected")
  native_game_settings(rom_id: i64) -> NativeGameSettings { executable: String, parameters: String, compat_tool: String, wineprefix: String, candidates: Vec<String> }
  set_native_game_settings(rom_id: i64, executable: String, parameters: String, compat_tool: String) -> ()   // compat_tool ignored (stored as "") on Windows hosts
  list_compat_tools() -> CompatToolsDto { tools: Vec<CompatTool>, default_tool: String }
  set_default_compat_tool(value: String) -> ()                            // modify_config; then emit compat-tools-changed
  list_compat_tool_catalog() -> Vec<CatalogEntry>                         // compat_tool_catalog_entries + mark_compat_installed
  install_compat_tool(source_id: String) -> ()
  rpcs3_firmware_status(emulator_name: String) -> Rpcs3FirmwareStatus { pup_path: Option<String> }
  install_ps3_firmware(emulator_name: String) -> bool                     // spawn_rpcs3_installfw
  cancel_download_for_rom(rom_id: i64) -> ()
  ```
  `FirmwareService` (pattern: `images.rs`): `in_flight: StdMutex<HashSet<PathBuf>>` keyed by `emulator_dir_of(entry)`; `pub fn spawn_for_game(self: &Arc<Self>, session: Arc<SessionManager>, install: Arc<InstallService>, record: InstalledGame)` → resolves platform id from `install.platform_ids()` (missing → return), computes the emulator dir for the default emulator (missing → return), `try_begin(dir)` false → return, spawns `install_for_game` on `tauri::async_runtime::spawn`, logs warnings at `warn` (D14), drop guard releases the dir; `pub fn spawn_for_emulator(self, session, install, name: String)` → entry + profile → RPCS3 → `spawn_ps3_firmware(..)`; else `platform_ids_for_profile` → for each id `install_platform_firmware(client, id, &targets, default)`; `pub fn spawn_ps3_firmware(self, session, install, entry: EmulatorEntry)` → `rpcs3_pup_path(&entry.path)` Some → return; targets empty → return; `ps3_platform_id` None → return (D17); `id = install.admit_external("PS3 Firmware", "PlayStation 3")`; spawn `install_platform_firmware(client, id, &targets, default)`; `install.complete_external(id, &warnings.first().cloned().unwrap_or_default())`.
  Wiring in `lib.rs` setup: `install.set_game_finalized_hook(|record| firmware.spawn_for_game(..))`; `install.set_emulator_installed_hook(|e| if e.fresh && !e.compat_tool { firmware.spawn_for_emulator(e.name) })`; `install.set_compat_tools_hook(|| handle.emit("compat-tools-changed", ()))`. `save_emulator`: after the existing `sync_new_emulator` when `is_add` and the entry `is_rpcs3` → `firmware.spawn_ps3_firmware(entry)`. `launch_game`: before `launch.launch`, if the installed row exists and the session has a client → `firmware.spawn_for_game(..)` (fire-and-forget; never blocks). `list_platforms`: `install.set_platform_ids(platforms.iter().map(|p| (p.name.clone(), p.id)).collect())`.
  `api.ts`: `DownloadEntry.job: 'game' | 'emulator' | 'firmware'`, `kind: DownloadKind = 'base' | 'ps4_content' | 'xbox360_content' | 'native_update' | 'emulator' | 'compat_tool' | 'firmware'`; types `ContentAvailability`, `NativeGameSettings`, `CompatTool`, `CompatToolsDto`, `Rpcs3FirmwareStatus`; `InstalledGame` gains the twelve fields; wrappers `installContent(romId, kind)`, `installNativeUpdate(romId)`, `contentAvailability(romId)`, `nativeGameSettings(romId)`, `setNativeGameSettings(romId, executable, parameters, compatTool)`, `listCompatTools()`, `setDefaultCompatTool(value)`, `listCompatToolCatalog()`, `installCompatTool(sourceId)`, `rpcs3FirmwareStatus(emulatorName)`, `installPs3Firmware(emulatorName)`, `cancelDownloadForRom(romId)`.

- [ ] **Step 1:** Write `firmware_service.rs` and `commands/specials.rs`; register commands; wire hooks; update `api.ts`.
- [ ] **Step 2:** `cargo build -p app` (from `rewrite/app/src-tauri` or `cargo build --workspace`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `npm run check` in `rewrite/app`, hygiene script. Unit-test the pure pieces: `ps3 firmware in-flight guard` (two `try_begin` on the same dir → second false; released after drop) and `set_native_game_settings` Windows-host blanking (behind a `fn normalize_compat_for_host(host, value)`).
- [ ] **Step 3: Commit** `rewrite: app commands for content/native/compat/firmware, FirmwareService triggers, compat-tools-changed event, api.ts types`

---

### Task 16: Details — Install App label, Install Update/DLC, Cancel, Game Settings dialog

**Files:**
- Create: `rewrite/app/src/lib/details/actions.ts` + `actions.test.ts` (pure)
- Create: `rewrite/app/src/lib/details/NativeSettings.svelte`
- Modify: `rewrite/app/src/lib/Details.svelte`
- Modify: `rewrite/app/src/lib/details/subject.ts` (no new fields needed; `platformName` suffices)

**Interfaces:**
- Consumes: `api.ts` from Task 15, `stores/downloads.svelte.ts`, `stores/installed.svelte.ts`.
- Produces (`details/actions.ts`, no store/API imports):
  ```ts
  export function isNativePlatform(platform: string): boolean;        // trimmed lower-case startsWith('windows')
  export function isContentPlatform(platform: string): boolean;       // PS4 or Xbox 360 predicates ported from platforms.rs
  export function installLabel(platform: string): 'Install App' | 'Install';
  export type ContentButtons = { update: boolean; dlc: boolean };
  export function contentButtons(avail: { update: boolean; dlc: boolean } | null, installed: boolean, busy: boolean): ContentButtons;
  export function candidateLabel(candidate: string, installDir: string): string; // path relative to the install dir when it is inside it
  ```
  Details behavior: primary button text `installLabel(subject.platformName)` when not installed (`data-testid="details-install"` unchanged). When a `liveEntry` exists: keep the disabled "Installing…" button AND add `<button data-testid="details-cancel" class="secondary">Cancel</button>` → `api.cancelDownloadForRom(romId)`. When installed and `isContentPlatform`: on mount/`$effect` fetch `api.contentAvailability(romId)` (errors → null) and render `details-install-update` ("Install Update") / `details-install-dlc` ("Install DLC") per `contentButtons` → `api.installContent(romId, 'update'|'dlc')`. When installed and native: `<button data-testid="details-game-settings" class="secondary">Game Settings</button>` opening `NativeSettings` (pattern 1 modal: backdrop + `role="dialog"`, `data-testid="native-settings"`, close on Escape/backdrop/`native-settings-close`). `NativeSettings.svelte` props `{ romId, title, onClose, onSaved }`: loads `api.nativeGameSettings(romId)`; `<select data-testid="native-settings-exe">` of `candidates` (labels via `candidateLabel`, value = full path; preselect `executable` when present else first candidate); `<input data-testid="native-settings-params">`; on non-Windows hosts (`navigator.platform` does not start with `Win`) a `<select data-testid="native-settings-compat">` fed by `api.listCompatTools()` with a leading `None` option (`""`) and the game's `compat_tool` preselected, falling back to `default_tool`; read-only line `data-testid="native-settings-prefix"` showing `wineprefix`; `native-settings-save` → `api.setNativeGameSettings(...)` then `onSaved()`/`onClose()`; errors inline `native-settings-error`. When `candidates` is empty show `"No launchable executables were found in this game's install directory."` and disable Save.

- [ ] **Step 1: vitest** for `actions.ts`: platform predicates (`'Windows'`, `'PlayStation 4'`, `'Xbox 360'`, `'SNES'`), `installLabel`, `contentButtons` (null → none; busy → none; both true), `candidateLabel`.
- [ ] **Step 2: Implement** the Svelte changes; keep the existing testids; follow the CSS conventions (secondary buttons, `.error`).
- [ ] **Step 3:** `npm test`, `npm run check`.
- [ ] **Step 4: Commit** `rewrite: Details — Install App label, Update/DLC buttons, Cancel (D9), native Game Settings dialog`

---

### Task 17: Emulators — CompatTools panel, RPCS3 firmware note/button, compat store, drawer kinds

**Files:**
- Create: `rewrite/app/src/lib/emulators/compatTools.ts` + `compatTools.test.ts`
- Create: `rewrite/app/src/lib/emulators/CompatTools.svelte`
- Create: `rewrite/app/src/lib/stores/compatTools.svelte.ts`
- Modify: `rewrite/app/src/lib/Emulators.svelte`
- Modify: `rewrite/app/src/lib/App.svelte` (init the compat store listener next to `initDownloads()`)
- Modify: `rewrite/app/src/lib/downloads/format.ts` + `format.test.ts` (`kindLabel`)
- Modify: `rewrite/app/src/lib/Downloads.svelte` (show the kind label for non-base kinds)

**Interfaces:**
- Produces (`emulators/compatTools.ts`):
  ```ts
  export type CompatGroup = { title: 'Wine' | 'Proton (system)' | 'Managed'; tools: CompatTool[] };
  export function groupCompatTools(tools: CompatTool[]): CompatGroup[];   // wine → 'Wine'; kind proton & source steam → 'Proton (system)'; source managed → 'Managed'; empty groups omitted
  export function compatToolLabel(tool: CompatTool): string;             // "Wine (system) — <path>" | "<name> (system) — <path>" | "<name> — <path>"
  export function isWindowsHost(platform: string): boolean;
  // downloads/format.ts
  export function kindLabel(kind: DownloadKind): string; // base → '', ps4_content/xbox360_content → 'Content', native_update → 'Update', emulator → 'Emulator', compat_tool → 'Compat tool', firmware → 'Firmware'
  ```
  Store `stores/compatTools.svelte.ts`: `compatTools` (`get tools()`, `get defaultTool()`), `refresh()` (`api.listCompatTools()`), `init()` → refresh + `listen('compat-tools-changed', refresh)`.
  `CompatTools.svelte` (rendered as a `<section class="compat-section" data-testid="compat-tools-section">` in `Emulators.svelte` on non-Windows hosts): radio groups per `groupCompatTools` (`<input type="radio" name="compat-default" data-testid="compat-default-<index>">`, checked when `tool.path === defaultTool`, change → `api.setDefaultCompatTool(path)`); empty → `<p data-testid="compat-empty">No compatibility tools installed</p>`; catalog list from `api.listCompatToolCatalog()` with `data-testid="compat-catalog-install-<testKeyFor(source_id)>"` / `compat-catalog-installed-…` (disabled) → `api.installCompatTool(source_id)`. Refresh the store after a compat-tool drawer entry completes (watch `downloads.entries` for kind `compat_tool` → completed, like `installed.svelte.ts` does) in addition to the event.
  RPCS3 card: for each entry whose name lower-cased contains `rpcs3`, call `api.rpcs3FirmwareStatus(name)`; when `pup_path` is set render `<p class="hint" data-testid="emulator-ps3-firmware-note-<sanitized>">PS3 firmware downloaded — click Install to activate it.</p>` and `<button data-testid="emulator-ps3-firmware-<sanitized>">Install PS3 Firmware</button>` → `api.installPs3Firmware(name)` → true: `<p data-testid="emulator-ps3-firmware-toast" class="hint">PS3 firmware installation started — follow the RPCS3 dialog to complete.</p>`; false: same testid with class `error` and text `Could not launch RPCS3 to install firmware. Check the emulator path.`. Re-query the status whenever a drawer entry with kind `firmware` reaches completed.

- [ ] **Step 1: vitest** for `groupCompatTools`, `compatToolLabel`, `kindLabel`.
- [ ] **Step 2: Implement.**
- [ ] **Step 3:** `npm test`, `npm run check`.
- [ ] **Step 4: Commit** `rewrite: Emulators CompatTools panel, RPCS3 firmware button, compat store, drawer kind labels (D8, D10)`

---

### Task 18: E2E — mock routes/fixtures, seeds, four stage groups

**Files:**
- Modify: `rewrite/scripts/e2e.sh` (groups `ps3-install`, `content`, `native`, `firmware`; `mock_args_for_group` fixture dirs; `seed_script_for_group`; export `E2E_STUB_BIN="$data_dir/stubs/bin"` when that dir exists)
- Modify: `rewrite/e2e/wdio.conf.ts` (`PATH` prepends `E2E_STUB_BIN` when set)
- Modify: `rewrite/e2e/mock-romm/server.mjs` (`file_ids` by-id lookup; `/api/firmware?platform_id=` and `/api/firmware/:id/content/:name` routes from a new optional `firmware.json` fixture `{ "<platform_id>": [{ id, file_name, content_key }] }` with bytes generated by a `firmwareBytesFor(content_key)` builder; `files[].category` passthrough; new content builders: PS3 zip, PS4 base/update zips, STFS zip via a `buildStfs(magic, titleId, contentType)` helper in `archives.mjs`, Windows game zip with a shell-script `mygame.exe`, `PS3UPDAT.PUP` bytes)
- Modify: `rewrite/e2e/mock-romm/server.test.mjs` (file_ids by-id, firmware routes)
- Create: `rewrite/e2e/fixtures-ps3-install/{platforms,roms,rom-details}.json`, `rewrite/e2e/fixtures-content/…`, `rewrite/e2e/fixtures-native/…`, `rewrite/e2e/fixtures-firmware/{platforms,roms,rom-details,firmware}.json`
- Create: `rewrite/e2e/seed/{ps3-install,content,native,firmware}-seed.mjs` (v3 `SCHEMA_SQL` copied from `registry.rs`; config.toml with `library_path`, emulator stubs under `<data>/stubs/`, `[default_emulators]`, and for native `default_compat_tool = "wine"` plus `<data>/stubs/bin/wine` — a `#!/bin/sh` script appending `"$@"` to `<data>/wine-argv.log` then `sleep 30`)
- Create: `rewrite/e2e/specs/{ps3-install,content,native,firmware}.spec.ts`
- Modify: `rewrite/README.md` coverage table (four rows)

**Interfaces:**
- Consumes: every testid from Tasks 16–17; `unrar` not needed (no RAR in E2E).
- Group contracts:
  - `ps3-install`: platform `{id:1,name:"PlayStation 3"}`; rom 401 file `game.zip` → `BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN` + `PARAM.SFO`; no emulator seeded (VFS fallback). Spec: connect, set library path, install → Completed → assert `<library>/PlayStation 3/.vfs/dev_hdd0/game/BLUS30336/PS3_GAME/USRDIR/EBOOT.BIN` exists, the staging dir `<library>/PlayStation 3/game/` does not, `grid-launcher.db` row `ps3_game_id = 'BLUS30336'` (sqlite3 CLI as the seeds use).
  - `content`: platforms `{1,"PlayStation 4"}`, `{2,"Xbox 360"}`; rom 501 files `[{id:2001,"ps4-base.zip",category:"game"},{id:2002,"ps4-update.zip",category:"update"}]`; rom 601 files `[{id:3001,"x360.zip",category:"game"},{id:3002,"x360-update.zip",category:"update"}]` (STFS `tu00000001`, title `415608C3`, type `000B0000`). Seed: `Xenia Edge` stub (`<data>/stubs/xenia-edge/xenia_canary` + `portable.txt` in that dir — the implementer must confirm with `autoconfig::readers::xenia_directory_settings` which layout yields `content_root = <dir>/content` and seed exactly that) default for `Xbox 360`. Spec: install 501 → Completed, `details-install-update` visible → click → drawer row titled `… (update)` → Completed → `<library>/PlayStation 4/ps4-base/CUSA12345/patch.txt` exists (the extraction dir name follows `extraction_dir(archive)`; assert with a glob on `CUSA12345/patch.txt` under the platform dir); install 601 → base Completed → a second row `… (update)` appears without any click → Completed → `<xenia dir>/content/0000000000000000/415608C3/000B0000/tu00000001` exists.
  - `native`: platform `{1,"Windows"}`; rom 701 files `[{id:4001,"mygame.zip"},{id:4002,"game.json"}]`; zip `MyGame/mygame.exe` (shell script content) + `readme.txt`; game.json `{"version":"1.0","year":2004,"tags":["indie"]}`. Spec: `details-install` text is `Install App`; install → Completed → `<library>/Windows/<Title>/game/MyGame/mygame.exe` and `<library>/Windows/<Title>/prefix` exist; `details-game-settings` → dialog lists `MyGame/mygame.exe`; set params `--fullscreen`; save; `details-play` → `<data>/wine-argv.log` contains the exe path and `--fullscreen`; `details-stop`; `details-cancel` smoke: start a second install of rom 702 (a throttled 300 KB zip via `e2e_throttle` — reuse the `downloads` group's big-archive pattern) and click `details-cancel` → row `Cancelled`.
  - `firmware`: platforms `{1,"PlayStation"}`, `{2,"PlayStation 3"}`; `firmware.json` `{"1":[{"id":9001,"file_name":"scph5501.bin","content_key":"bios"}],"2":[{"id":9002,"file_name":"PS3UPDAT.PUP","content_key":"pup"}]}`; rom 801 (PS1 zip with `game.bin`). Seed: `DuckStation` stub at `<data>/stubs/duckstation/duckstation-qt` default for `PlayStation`. Spec: install 801 → Completed → poll until `<stubs>/duckstation/bios/scph5501.bin` exists; then Emulators → add manual emulator `RPCS3` with path `<stubs>/rpcs3/rpcs3` (seed writes the stub file but NOT the config entry) → drawer shows a row `PS3 Firmware` → Completed → `<stubs>/rpcs3/PS3UPDAT.PUP` exists → the RPCS3 card shows `emulator-ps3-firmware-note-rpcs3` and `emulator-ps3-firmware-rpcs3` → click → toast text equals the verbatim success string and `<data>/rpcs3-argv.log` (the stub records argv) contains `--installfw`.
- [ ] **Step 1:** mock server + `server.test.mjs` (`node --test`), builders, fixtures, seeds, `e2e.sh`/`wdio.conf.ts` plumbing.
- [ ] **Step 2:** the four specs; run each group: `bash scripts/e2e.sh ps3-install`, `… content`, `… native`, `… firmware` (with `E2E_SKIP_BUILD=1` after the first build). All green.
- [ ] **Step 3:** README rows.
- [ ] **Step 4: Commit** `rewrite: E2E groups ps3-install, content, native, firmware; mock firmware routes and file_ids`

---

### Task 19: Docs — deviations and rulings; milestone gate

**Files:**
- Modify: `docs/porting/03-library-install.md` (new section "Rust port deviations (milestone 8)" with D1–D9, D11–D18 verbatim from the spec/plan; rule every open question in §9 (unreachable cleanup warnings → ported as real warnings via `delete_with_retry`), §11 (`ps3_rpcs3_data_root` → D4), §12/13 (category parser → D5), §18 (Sony path → D2; background jobs → D6); mark the "RAR only for PS3" table note as D1)
- Modify: `docs/porting/04-emulator-launch.md` ("Rust port deviations (milestone 8)": D10 — PS3 launch target from registry fields closes milestone 3 deviation 3; firmware after fresh source install closes milestone 4 deviation 5; compat tools in their own panel amends milestone 4 deviation 2; D18 native sessions; umu message verbatim note; `compat_tool_installs` persistence D7)
- Modify: `docs/porting/05-emulator-autoconfig.md` (the D7 "no background firmware download" ruling is superseded: RPCS3 firmware now downloads from the server as a drawer job)
- Modify: `rewrite/README.md` ("Manual test checklist — Milestone 8": residual manual items: a real RomM server with PS4/Xbox 360 content categories and firmware; a real Proton install through `umu-run`; an RPCS3 `--installfw` dialog; RAR archives from a real server)

- [ ] **Step 1:** Write the doc sections, following doc 07's "Rust port deviations (milestone 7)" formatting.
- [ ] **Step 2: Milestone gate**, from `rewrite/`: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check && bash scripts/check_secret_hygiene.sh && (cd app && npm run check && npm test) && bash scripts/e2e.sh` — all green.
- [ ] **Step 3: Commit** `rewrite: milestone 8 deviations in docs 03/04/05; README checklist`

---

## Self-review notes

- Spec coverage: install modes/dispatch/content ids (T1, T9, T10); registry v3 (T2); RAR + should-extract table + ISO helper (T3); config keys, profile fields, RomM firmware endpoints (T4); PS3 (T5, T9); PS4 (T6, T9, T10); Xbox 360 (T7, T10); native install/update/game.json/executables (T8, T9, T10); native launch + compat discovery + PS3 launch target (T11); managed compat installs + catalog (T12); firmware write/routing/per-game/profile ids/RPCS3 PUP (T13, T14); app commands, FirmwareService, three triggers, external entries, event (T9 external API, T15); Details UI (T16); Emulators UI + drawer kinds (T17); E2E four groups (T18); docs (T19). Uninstall branches D11 (T9). Cancel button D9 (T16 + T9 `cancel_for_rom`).
- Type consistency: `ContentKind` (T1) is used by `JobKey::Content` (T9), `install_content` (T10), `ps4::apply_content` (T6), and the `install_content` command's `kind` string (T15 parses with `ContentKind::parse`). `FirmwareTarget` (T13) is what `targets_for_entry`/`shape_for_*` (T14) produce and `install_platform_firmware` consumes. `ExtractFn` is defined once in `specials::ps4` and reused by `xenia` and `native` (import it). `EmulatorInstalled`/`GameFinalizedHook`/`CompatToolsHook` (T9/T12) are what `lib.rs` (T15) binds. `DownloadEntry.kind` strings (T9) match the `DownloadKind` union (T15) and `kindLabel` (T17).
- Known dependency ordering: T11 needs T2/T8 for `resolved_executable`; T12 needs T11's `managed_root`/`find_proton_dir`; T14 needs T4 profile fields and T13; T15 needs T9–T14; T16/T17 need T15's `api.ts`; T18 needs T16/T17 testids.
- Spec strings superseded by verbatim Python: only the umu-run message (Global Constraints). Spec's `update_record` replaced by `upsert` (Global Constraints, T2).
