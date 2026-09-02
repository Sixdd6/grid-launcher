# Cloud Saves Implementation Plan (rewrite milestone 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the cloud save/state sync engine, its auto-sync triggers, and
the desktop cloud UI to grid-core/Tauri, and replace the xemu whole-image
sync with a clean-room raw-disk FATX method (extract/inject `E:/UDATA` +
`E:/TDATA` directly, qcow2 unsupported by decision).

**Architecture:** New grid-core modules `src/cloud/` (pure logic first, IO at
the edges: state, session window, tokens, candidates, archive, scope,
transfer, restore, native, dirs, retention, ops, xemu_sync) and `src/fatx/`
(clean-room FATX read+write over raw images). RomM client gains the
save/state endpoints. App layer wires auto triggers into the existing
2500 ms session poll loop and adds the Details cloud panel.

**Tech Stack:** Rust (reqwest + `multipart` feature, zip, serde, regex,
tempfile, wiremock for tests), Tauri 2 commands, Svelte 5, WebdriverIO.

**Spec:** `docs/superpowers/specs/2026-09-02-cloud-saves-design.md` (binding
authority). Behavior contract: `docs/porting/06-cloud-saves.md` (doc 06) —
where the spec is silent doc 06 wins; where both are silent the Python
source cited by doc 06 wins. Reference code:
`grid_launcher/library/cloud_sync.py`, `cloud_transfer.py`,
`cloud_upload.py`, `cloud_restore.py`, `identity.py`,
`grid_launcher/emulator/selection.py`, `profiles.py`, `retroarch.py`,
`grid_launcher/ui/mixins/cloud_mixin.py`, `details_view_mixin.py`,
`grid_launcher/server/pcgamingwiki.py`, `grid-launcher.py`. Python test
oracles: `tests/test_cloud_transfer.py`, `test_cloud_restore.py`,
`test_cloud_save_block_reason.py`, `test_cloud_state_filter.py`,
`test_flycast_vmu.py`.

## Global Constraints

- **Byte-for-byte strings.** Every user-facing message, endpoint path,
  query/multipart field name, slot literal (`shared-media`, `vmu0`..`vmu3`),
  marker value (`native_multi_dir`, `native_dir:`,
  `_grid_launcher_dirs.json`), config key and ignore-set entry ports
  verbatim from the reference. Do not "improve" wording.
- **Follow-the-code quirks are mandatory.** The spec's "Follow-the-code
  quirks" section lists Python behaviors that look like bugs and port
  anyway (PPSSPP scanner mtime/ignore quirk, RPCS3 directory precedence,
  scope/block flag-fallback asymmetry, shared-scope re-download, cache key
  without ROM id, substring owner match, any-path-in-window jobs, unpruned
  states, the relative-time bucket bug in `relative_timestamp_text`).
  Fixing one is a defect.
- **Deviations D1–D8** (spec "Deviations" section) are the ONLY intended
  behavior changes: D1 xemu raw-disk sync with no whole-image path, D2
  legacy xemu records skipped on restore, D3 autoconfig accepts
  `xbox_hdd.img`, D4 `_authorized_headers` branch dropped (server-relative
  candidates only; absolute `http(s)` candidates are skipped), D5 auto
  uploads serialized per game with a bounded pool, D6 staged atomic
  shared-slotted restore, D7 `cloud_save_retention_limit` config key
  (default 3, min 1), D8 unpollable sessions count as finished (spec-level;
  the Rust store always polls).
- **D9 — credential files are never synced.** Token secrecy outranks
  parity: the always-ignored basename set additionally contains
  `retroarch.cfg`, `pcsx2.ini`, `ppsspp.ini` and
  `ppsspp_retroachievements.dat` (lowercased comparison, both archive-write
  and candidate-scan sides), so a save path pointed at an emulator root can
  never upload a file holding a RetroAchievements token. Tests pin this.
- **Tokens.** No new `expose_secret()` call sites. Nothing cloud-related
  logs, serializes, or uploads a token; `scripts/check_secret_hygiene.sh`
  passes after every task. All server traffic goes through `RommClient`'s
  existing auth header.
- **grid-core never imports Tauri.** Orchestration takes plain data
  (`&Config`, `&[CloudGame]`, paths); errors cross as `Display` strings.
- **Async boundary.** `RommClient` is async (reqwest/tokio). Everything in
  `cloud/` below `ops.rs` is synchronous pure logic; `ops.rs` functions
  that touch the server are `async fn` taking `&RommClient`.
- **Case-insensitive path comparisons** everywhere the reference casefolds
  (dedupe, ignore matching — cloud_sync.py:346): compare
  `to_lowercase()` of the string form.
- **`stat()` failures never abort a scan** — skip the entry and continue
  (doc 06 invariants).
- **Every task ends green**, run from `rewrite/`:
  - `cargo test -p grid-core` (workspace when app touched)
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --check`
  - `bash scripts/check_secret_hygiene.sh`
  - `npm run check` + `npm test` in `rewrite/app` when the frontend is touched
  - The full `rewrite/scripts/e2e.sh` gates the milestone at Task 20.
- **Windows-only behaviors** (Shell Documents redirection, `%DOCUMENTS%`)
  are implemented behind `cfg(windows)` with the documented non-Windows
  fallbacks, unit-tested via the injectable-parameter pattern used by
  `autoconfig` (pass the documents path in, don't read the OS in the pure
  fn).

## File Structure

```
rewrite/crates/grid-core/src/cloud/mod.rs        NEW  CloudGame, SaveType, shared types, pub mods
rewrite/crates/grid-core/src/cloud/state.rs      NEW  identity keys, SyncStateEntry, normalize/update, auto plan, summary
rewrite/crates/grid-core/src/cloud/window.rs     NEW  session window + the five mtime filters
rewrite/crates/grid-core/src/cloud/tokens.rs     NEW  match tokens, serial/id extractors, state-name variants
rewrite/crates/grid-core/src/cloud/candidates.rs NEW  ignore sets, file/folder scanners (generic, Cemu, PCSX2, RPCS3, PPSSPP)
rewrite/crates/grid-core/src/cloud/archive.rs    NEW  3 zip writers, temp naming, filtered extraction + 7z fallback, zip-slip
rewrite/crates/grid-core/src/cloud/scope.rs      NEW  SaveScope, block reasons, shared owner, native-platform predicate
rewrite/crates/grid-core/src/cloud/transfer.rs   NEW  URL normalization, sidecars, job builders, window job filter, short circuits, messages
rewrite/crates/grid-core/src/cloud/restore.rs    NEW  record parsing/sorting/selection, target selection, payload placement
rewrite/crates/grid-core/src/cloud/native.rs     NEW  native path resolution, wine translation, manifest archive + restore helpers
rewrite/crates/grid-core/src/cloud/dirs.rs       NEW  sync-directory resolution: overrides, token expansion, screenshot dirs
rewrite/crates/grid-core/src/cloud/retention.rs  NEW  prune_server_save_records
rewrite/crates/grid-core/src/cloud/ops.rs        NEW  upload/restore orchestration, dispatch, emulator resolution + cache
rewrite/crates/grid-core/src/cloud/xemu_sync.rs  NEW  image sniff/classify, E: extract/inject, legacy-record skip
rewrite/crates/grid-core/src/fatx/mod.rs         NEW  errors, FatxError, pub mods
rewrite/crates/grid-core/src/fatx/layout.rs      NEW  retail offsets, superblock, cluster geometry
rewrite/crates/grid-core/src/fatx/fat.rs         NEW  FAT16X/FAT32X read/write, chains, allocation
rewrite/crates/grid-core/src/fatx/dir.rs         NEW  directory entries, traversal, create/update/delete
rewrite/crates/grid-core/src/fatx/image.rs       NEW  FatxPartition: read_tree / write_tree / remove_tree
rewrite/crates/grid-core/src/fatx/builder.rs     NEW  test-only image builder (cfg(test) + pub for integration tests)
rewrite/crates/grid-core/src/pcgw.rs             NEW  PCGamingWiki wikitext parsing + fetch
rewrite/crates/grid-core/src/lib.rs              MOD  pub mod cloud; pub mod fatx; pub mod pcgw;
rewrite/crates/grid-core/src/config.rs           MOD  auto_cloud_save_* keys, retention limit, cloud_sync_state, native_manual_save_paths
rewrite/crates/grid-core/src/romm/mod.rs         MOD  save/state endpoints, multipart uploads, deletes
rewrite/crates/grid-core/src/launch/profiles.rs  MOD  EmulatorProfile gains screenshot_directories
rewrite/crates/grid-core/src/launch/mod.rs       MOD  finished-session hook for auto upload
rewrite/crates/grid-core/src/autoconfig/xemu.rs  MOD  D3: xbox_hdd.img acceptance
rewrite/crates/grid-core/src/autoconfig/writers.rs MOD Task 1 cleanup only
rewrite/crates/grid-core/src/autoconfig/cemu.rs  MOD  Task 1 cleanup only
rewrite/crates/grid-core/Cargo.toml              MOD  reqwest "multipart" feature, wiremock dev-dep (if absent)
rewrite/app/src-tauri/src/commands.rs            MOD  cloud commands
rewrite/app/src-tauri/src/lib.rs                 MOD  CloudService in AppState, command registration, poll hook
rewrite/app/src/lib/api.ts                       MOD  cloud types + invokes
rewrite/app/src/lib/Details.svelte               MOD  cloud panel
rewrite/app/src/lib/details/cloud.ts             NEW  pure panel helpers + vitest
rewrite/e2e/specs/cloud-saves.spec.ts            NEW  E2E group
rewrite/e2e/                                     MOD  mock RomM gains save/state endpoints
docs/porting/06-cloud-saves.md                   MOD  deviations section (Task 20)
```

**Shared types** (defined in `cloud/mod.rs` by Task 2, used everywhere):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SaveType { Save, State }
impl SaveType { pub fn as_str(&self) -> &'static str /* "save" | "state" */ }

/// Plain-data view of a game for cloud logic. Built from an InstalledGame
/// or a server GameSummary; fields the source lacks stay "".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloudGame {
    pub title: String,
    pub platform: String,
    pub rom_id: String,        // string form, "" when absent (Python parity)
    pub rom_file_name: String,
    pub extracted_path: String,
    pub archive_path: String,
    pub description: String,
    pub title_id: String,      // data-availability gap: the rewrite's
    pub base_title_id: String, // registry does not carry these three yet;
    pub ps3_game_id: String,   // token logic ports fully, wiring passes ""
}
```

The three id fields are a recorded data gap (doc 06 deviations, Task 20):
Python fills them during PS3/Wii-U archive preparation, which the rewrite
has not ported. With them blank, the RPCS3/Cemu scanners run with empty
token sets, which per the reference accept everything — degraded matching,
not a crash.

---

### Task 1: Milestone 5 cleanup carried forward

**Files:**
- Modify: `rewrite/crates/grid-core/src/autoconfig/writers.rs`
- Modify: `rewrite/crates/grid-core/src/autoconfig/cemu.rs`

The M5 final review deferred three duplication nits into this milestone's
opening (spec "In scope" list): (1) `writers.rs` carries two near-identical
`apply_section`-style walk loops and two copies of the section-header
regex — hoist ONE `SECTION_RE` (`once_cell`/`std::sync::OnceLock` +
`regex`) and ONE shared walk helper, keeping every family's behavior
byte-identical (the existing writer tests are the oracle; do not touch a
single test expectation); (2) `cemu.rs` recompiles its controller-XML
regex per call — hoist to a `OnceLock`; (3) the duplicated
lowercased-substring fallback helper flagged in the M5 final review —
single helper, both call sites.

- [ ] **Step 1:** `cargo test -p grid-core` green before touching anything
  (baseline).
- [ ] **Step 2:** Refactor. NO behavior change: no test file is modified.
- [ ] **Step 3:** `cargo test -p grid-core` green, clippy/fmt/hygiene green.
- [ ] **Step 4: Commit** `rewrite: autoconfig cleanup — shared section walk, hoisted regexes`

### Task 2: `cloud/state.rs` — identity, sync state, auto plan + config keys

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/mod.rs`, `cloud/state.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs` (`pub mod cloud;`),
  `rewrite/crates/grid-core/src/config.rs`

**Interfaces (Produces):**
```rust
// mod.rs: SaveType, CloudGame (above)

// state.rs
pub fn game_key(game: &CloudGame) -> String;      // identity.py:4 — "rom:<id casefolded>" else "name:<title lower>::<platform lower>"; "" when untrackable
pub fn rom_id_key(rom_id: &str) -> String;        // "rom:<trimmed casefolded>", "" for blank
pub fn games_match_identity(a: &CloudGame, b: &CloudGame) -> bool; // identity.py:15 — rom ids when both present, else (title,platform) lowercased

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SyncStateEntry {          // all fields #[serde(default)]
    pub last_downloaded_save_id: String,
    pub last_server_timestamp: f64,
    pub last_uploaded_local_mtime: f64,   // legacy alias, still written
    pub last_uploaded_at: String,
    pub last_downloaded_state_id: String,
    pub last_uploaded_save_mtime: f64,
    pub last_uploaded_state_mtime: f64,
    pub last_session_started_at: f64,
    pub last_session_ended_at: f64,
}

pub fn normalize_sync_state(raw: &toml::value::Table) -> BTreeMap<String, SyncStateEntry>;
pub fn sync_entry_for(config: &Config, key: &str) -> SyncStateEntry;
/// Shallow-merges only the `Some` fields into the stored entry and returns
/// the updated map for the caller to store + save (parity: every update
/// saves the whole config — details_view_mixin.py:384).
pub struct SyncStateUpdate { /* Option<> per field, Default */ }
pub fn apply_sync_update(config: &mut Config, key: &str, update: SyncStateUpdate);

pub struct UploadPlan { pub types: Vec<SaveType>, pub latest_mtimes: BTreeMap<SaveType, f64> }
pub fn auto_cloud_upload_plan(entry: &SyncStateEntry, save_mtime: f64, state_mtime: f64, include_state: bool) -> UploadPlan; // cloud_sync.py:154
pub struct PerTypeResult { pub uploaded: i64, pub total: i64, pub failed: Vec<String> }
/// Returns (updates-to-apply, debug segments). cloud_sync.py:186.
pub fn summarize_auto_cloud_upload_result(
    per_type: &BTreeMap<SaveType, PerTypeResult>,
    latest_mtimes: &BTreeMap<SaveType, f64>,
    uploaded_at: &str,
) -> (SyncStateUpdate, Vec<String>);
```

**Config additions** (all `#[serde(default = ...)]`, TOML round-trip safe
like the M5 fields):

| Field | Type | Default | Reference |
|---|---|---|---|
| `auto_cloud_save_download_on_launch` | bool | true | grid-launcher.py:2212 |
| `auto_cloud_save_upload_on_exit` | bool | true | grid-launcher.py:2215 |
| `auto_cloud_save_skip_download_if_local_newer` | bool | true | grid-launcher.py:2218 |
| `auto_cloud_save_upload_delay_seconds` | u64 | 3, clamped 0–60 on read | grid-launcher.py:2221 |
| `cloud_save_retention_limit` | u32 | 3, min 1 on read (D7) | grid-launcher.py:2224 |
| `cloud_sync_state` | `toml::value::Table` | empty | doc 06 "Sync state entry" |
| `native_manual_save_paths` | `BTreeMap<String, Vec<String>>` | empty | grid-launcher.py:434 |

Store `cloud_sync_state` raw (a `toml::value::Table`) so foreign junk
round-trips; `normalize_sync_state` ports the tolerant normalization
(cloud_sync.py:8-56): drop non-string/blank keys, non-table values,
wrong-typed fields (TOML integer coerces to f64 for the float fields;
strings do NOT); drop entries that end up empty.

**Pinned rules:**
- `auto_cloud_upload_plan` (cloud_sync.py:154-184): plan `save` when
  `save_mtime > 0.0 && save_mtime > previous + 1.0`, previous =
  `last_uploaded_save_mtime` else `last_uploaded_local_mtime` else 0;
  `state` under the same rule vs `last_uploaded_state_mtime`, only when
  `include_state`. Unparseable stored values are 0.0 (normalization already
  guarantees this).
- `summarize` (cloud_sync.py:186-241): walk `save` then `state`; skip a
  type entirely when `total <= 0 && uploaded <= 0 && failed.is_empty()`;
  when `uploaded > 0` write back that type's planned latest mtime (save →
  BOTH `last_uploaded_save_mtime` and `last_uploaded_local_mtime`); write
  `last_uploaded_at` once when anything uploaded and the string is
  non-blank; per-type debug segment
  `"<type>=<uploaded>/<max(total,uploaded)> failed=<first 3 joined ','>"`.

- [ ] **Step 1: failing tests**
  - `game_key_prefers_rom_id_and_casefolds` / `..._falls_back_to_title_platform` / `..._is_empty_when_untrackable`
  - `games_match_identity_prefers_rom_ids` / `..._compares_title_platform_when_either_lacks_a_rom_id`
  - `normalize_drops_wrong_typed_fields_and_empty_entries` (mixed table: good entry, string value, int mtime coerced, bool in a string field dropped, entry left empty dropped)
  - `apply_sync_update_merges_shallowly_and_preserves_other_fields`
  - `upload_plan_requires_more_than_one_second_of_drift` (exact boundary: previous+1.0 is NOT planned, previous+1.000001 is)
  - `upload_plan_falls_back_to_the_legacy_mtime_field`
  - `upload_plan_skips_state_when_include_state_is_false`
  - `summarize_writes_both_save_mtime_fields_and_uploaded_at`
  - `summarize_skips_an_all_zero_type_and_builds_debug_segments`
  - `config_defaults_for_the_seven_new_fields` + round-trip test (save/load preserves unknown `cloud_sync_state` junk)
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud sync state, identity keys, auto upload plan, config keys`

### Task 3: `cloud/window.rs` — session window + mtime filters

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/window.rs`
- Modify: `cloud/mod.rs` (`pub mod window;`)

**Interfaces (Produces):**
```rust
pub type Window = (f64, f64); // inclusive bounds

pub struct ActiveSessionRef { pub game: CloudGame, pub started_at: f64 }
/// cloud_sync.py:243 — walk sessions IN REVERSE; first identity match with
/// started_at > 0 → (max(0, start-2), now+30); else persisted fallback:
/// started<=0 → None; ended clamped to started when <=0 or < started;
/// (max(0, start-2), ended+30).
pub fn session_window_for_state_upload(sessions: &[ActiveSessionRef], game: &CloudGame, entry: &SyncStateEntry, now: f64) -> Option<Window>;

pub fn filter_files_by_mtime_window(files: &[PathBuf], window: Option<Window>) -> Vec<PathBuf>;            // :285 — None window = passthrough; stat failure skips
pub fn session_filtered_file_candidates(files: Vec<PathBuf>, window: Option<Window>) -> Vec<PathBuf>;       // :318 — empty result FALLS BACK to input
pub fn filter_directories_by_mtime_window(dirs: &[PathBuf], window: Option<Window>, ignore: &IgnoreSets) -> Vec<PathBuf>; // :297 — newest non-blocked file beneath
pub fn session_filtered_directory_candidates(dirs: Vec<PathBuf>, window: Option<Window>, ignore: &IgnoreSets) -> Vec<PathBuf>; // :325 — fallback like files
```
`IgnoreSets` arrives in Task 5; THIS task defines it in `cloud/mod.rs` as
`{ pub basenames: BTreeSet<String>, pub extensions: BTreeSet<String> }`
(lowercased members) with `fn blocks(&self, path: &Path) -> bool`, plus the
free fn `latest_mtime_under(dir: &Path, ignore: &IgnoreSets) -> f64`
(0.0 when nothing qualifies; walks recursively; stat failures skip).

D8 note for the ledger: `partition_active_game_sessions`' unpollable-drop
branch is NOT ported — the rewrite's `SessionStore::reap` always polls real
children; doc 06 records this as D8 in Task 20.

- [ ] **Step 1: failing tests** (tempdir + `filetime` or explicit
  `File::set_modified` to pin mtimes)
  - `window_uses_the_most_recent_matching_active_session` (two sessions, same identity, reverse order wins)
  - `window_applies_the_2s_leadin_and_30s_tailout` (exact arithmetic asserted)
  - `window_falls_back_to_persisted_state_and_clamps_ended` (ended < started; ended == 0)
  - `window_is_none_when_no_session_and_no_persisted_start`
  - `filter_files_is_inclusive_on_both_bounds`
  - `filter_files_passthrough_on_none_window`
  - `session_filtered_files_fall_back_when_everything_is_out_of_window`
  - `filter_directories_compares_the_newest_non_blocked_file` (blocked-basename file newest → older good file decides)
  - `session_filtered_directories_fall_back_when_empty`
  - `latest_mtime_under_skips_blocked_and_unstatable_entries`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud session window and mtime filters`

### Task 4: `cloud/tokens.rs` — match tokens, ids, state-name machinery

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/tokens.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
pub fn compact_alnum(s: &str) -> String; // lowercased [a-z0-9] only
pub fn game_save_match_tokens(game: &CloudGame) -> BTreeSet<String>;     // cloud_mixin.py:1204
pub fn ps2_serial_tokens(game: &CloudGame) -> BTreeSet<String>;          // cloud_mixin.py:1401
pub fn psp_id_tokens(game: &CloudGame) -> BTreeSet<String>;              // cloud_mixin.py:1427 vicinity
pub fn ps3_id_tokens(game: &CloudGame) -> Vec<String>;                   // cloud_mixin.py:1177 vicinity — read the Python for the exact source fields
pub fn cemu_title_id_tokens(tokens: &BTreeSet<String>) -> Vec<String>;   // cloud_sync.py:506 — the ≥16 / exactly-8-not-0005 / all preference ladder
pub fn is_state_file_candidate(path: &Path) -> bool;                     // cloud_mixin.py:1334
pub fn state_candidate_base_variants(name: &str) -> Vec<String>;         // cloud_sync.py:370
pub fn state_candidate_matches_tokens(name: &str, tokens: &BTreeSet<String>) -> bool; // cloud_sync.py:384 — empty set matches all
pub fn state_candidate_hash_group_key(name: &str) -> String;             // cloud_sync.py:405
```

**Pinned rules** (regex table — port each verbatim):
- Match tokens (cloud_mixin.py:1204-1263): title + possessive-stripped
  variant (`'s` and `’s`) + compacted forms of both; `title_id` /
  `base_title_id` plain and Nintendo-id variants; stems of
  `rom_file_name` / `extracted_path` / `archive_path`; `ps3_game_id`
  lowercased verbatim. All lowercased; blanks dropped.
- Nintendo variants (cloud_mixin.py:1221): for each `\b[A-Z][A-Z0-9]{3,5}\b`
  match, first four chars lowercased AND their ASCII-hex encoding; for each
  16-hex-digit run, whole + high half + low half; for each
  `<8hex><sep><8hex>` pair, high, low, concatenation.
- PS2 serials: `[A-Z]{4}[-_ ]?\d{3}\.\d{2}` and `[A-Z]{4}[-_ ]?\d{5}` over
  title / rom_file_name / extracted_path / archive_path; PSP ids:
  `[A-Z]{4}[-_ ]?\d{5}`.
- State suffix acceptance: `.state .savestate .st .ss .ppst .p2s`; any name
  containing `.state`; `[._]\d+\.sav$`; `_resume\.sav$`; reject image
  suffixes FIRST (`.jpg .jpeg .png .webp .gif .bmp`).
- Variant-stripping patterns (cloud_sync.py:370):
  `(\s*\([0-9a-f]+\))?(\.\d+)?\.p2s$`,
  `\.(savestate|state|st|ss|ppst)(\.auto|auto|[0-9]+)?$`, `(\.\d+)?\.sav$`,
  `[_](\d+|resume)\.sav$`, `\.\d+$` — applied to raw name and stem.
- Hash group key (cloud_sync.py:405): 8-hex prefix of `<hash>[.<n>].sav`,
  else stem before `_<digits>`/`_resume` in `<name>_<n>.sav`, else "".

- [ ] **Step 1: failing tests** — port the oracle set:
  - `test_cloud_state_filter.py:50-71` → `state_candidate_rejects_image_sidecars`, `..._accepts_state_and_slot_files`, `..._accepts_duckstation_slots`, `..._accepts_numbered_sav`
  - `:78,110` → `base_variants_strip_duckstation_naming`, `..._strip_pcsx2_p2s_naming` (incl. the `(<hex>)` parenthesized-hash form)
  - `:87` → `hash_group_key_handles_duckstation_names`
  - `:120` → `p2s_candidate_matches_serial_tokens`
  - `tokens_include_possessive_stripped_and_compacted_title` (e.g. `Luigi's Mansion` → `luigi's mansion`, `luigis mansion`, `luigismansion`…)
  - `nintendo_variants_add_hex_forms` (hand-computed vector: `GALE01` → `gale`, `47414c45`; a 16-hex id → whole/high/low)
  - `ps2_serials_extracted_from_all_four_fields` (`SLUS-203.12`, `SCES12345`)
  - `cemu_ladder_prefers_16_then_8_not_0005`
  - `empty_token_set_matches_everything`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud match tokens and state-name machinery`

### Task 5: `cloud/candidates.rs` — ignore sets + all scanners

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/candidates.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
/// DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES ∪ D9 credential files ∪ entry/profile
/// additions ∪ (`_pcsx2_superblock` when is_pcsx2 && save_type==Save).
/// Entry strings are comma/newline-split like the M5 entry fields;
/// profile lists come from EmulatorProfile. profiles.py:361,385.
pub fn resolved_ignore_sets(entry: Option<&EmulatorEntry>, profile: Option<&EmulatorProfile>, save_type: SaveType, is_pcsx2: bool) -> IgnoreSets;
pub fn resolved_save_strategy(entry: Option<&EmulatorEntry>, profile: Option<&EmulatorProfile>, save_type: SaveType) -> String; // profiles.py:356 — state defaults "single_file"; save: entry, else profile, else "auto"

pub fn file_candidates(dirs: &[PathBuf], tokens: &BTreeSet<String>, save_type: SaveType, ignore: &IgnoreSets, explicit_file_roots: &[PathBuf]) -> Vec<PathBuf>; // cloud_sync.py:574
pub fn directory_candidates(dirs: &[PathBuf], tokens: &BTreeSet<String>, ignore: &IgnoreSets) -> Vec<PathBuf>;   // :439
pub fn cemu_save_directories(dirs: &[PathBuf], tokens: &BTreeSet<String>, ignore: &IgnoreSets) -> Vec<PathBuf>;  // :492
pub fn pcsx2_save_directories(dirs: &[PathBuf], serials: &BTreeSet<String>, ignore: &IgnoreSets) -> Vec<PathBuf>; // cloud_mixin.py:1147
pub fn rpcs3_save_directories(dirs: &[PathBuf], ids: &[String]) -> Vec<PathBuf>;   // cloud_mixin.py:1177 — index-first sort quirk
pub fn ppsspp_save_directories(dirs: &[PathBuf], ids: &BTreeSet<String>) -> Vec<PathBuf>; // cloud_mixin.py:1427 — own-mtime sort, NO ignore sets (quirk)
```

**Pinned rules:**
- `DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES = {".ds_store", "desktop.ini",
  "ehthumbs.db", "thumbs.db"}` (cloud_transfer.py:19) — plus D9's four.
- `file_candidates`: `[]` unless save/state; per existing dir: iterate the
  file itself for explicit file roots, else `rglob("*")` equivalent
  (`walkdir` or manual recursion); skip non-files, blocked basenames,
  blocked extensions. Save keep-rule: no tokens ∨ explicit root ∨ token
  substring of lowercased filename or compacted stem (cloud_sync.py:626).
  State: must pass `is_state_file_candidate`; split matched (explicit root
  ∨ variants match) / unmatched; matched wins, else the hash-group
  fallback (cloud_sync.py:416): one unmatched → take it; else newest by
  (mtime, lowercased-name tiebreak) picks the group key; empty key → none;
  return the group newest-first. Final order: mtime desc, then lowercased
  name; case-insensitive dedupe (cloud_sync.py:638).
- `directory_candidates`: immediate children containing ≥1 non-blocked file
  anywhere beneath; match = compacted child name OR compacted relative path
  vs tokens; empty set accepts all; sort newest-non-blocked-file desc;
  dedupe (cloud_sync.py:439-481).
- Cemu: walk `<dir>/<high>/<low>/user/`, children of `user` or `user`
  itself when childless; drop latest-mtime ≤ 0; matched list wins when
  non-empty (cloud_sync.py:492-566).
- PCSX2: immediate children with ≥1 file, serial-matched, newest-beneath
  desc. RPCS3: index-first, then mtime (QUIRK). PPSSPP: dir's own mtime,
  no file requirement, no ignore (QUIRK).

- [ ] **Step 1: failing tests** — including ports of
  `test_cloud_transfer.py:512,532,565,595` (explicit file roots accepted;
  cemu nested `user/` selection; state candidates filtered to matching rom
  name; only common variants allowed) plus:
  - `file_candidates_skip_blocked_basenames_extensions_and_d9_credential_files` (a `retroarch.cfg` inside a save dir never surfaces)
  - `save_candidates_match_on_compacted_stem_substring`
  - `state_fallback_returns_the_newest_hash_group`
  - `ordering_is_mtime_desc_then_name_with_ci_dedupe`
  - `directory_candidates_require_a_non_blocked_file_beneath`
  - `rpcs3_directory_index_outranks_recency` (stale dir in dirs[0] beats fresh dir in dirs[1])
  - `ppsspp_uses_directory_own_mtime_and_ignores_nothing`
  - `pcsx2_ignore_set_gains_the_superblock_basename`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud candidate scanners and ignore sets`

### Task 6: `cloud/archive.rs` — writers, extraction, zip-slip

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/archive.rs`
- Modify: `cloud/mod.rs`; `grid-core/Cargo.toml` only if a helper crate is
  genuinely missing (zip is already a dependency via the extractor).

**Interfaces (Produces):**
```rust
pub fn temp_archive_path(title: &str) -> PathBuf; // "<sanitized>-<local ISO seconds, ':'→'-'>.zip" in std::env::temp_dir(); ms suffix on collision (cloud_transfer.py:286)
pub fn zip_directory_for_upload(dir: &Path, ignore: &IgnoreSets) -> io::Result<PathBuf>;                    // members "<dirname>/<rel>" (:419)
pub fn zip_grouped_files_for_upload(files: &[PathBuf], archive_name_stem: &str) -> io::Result<PathBuf>;    // members rel to common parent, else bare name (:328,:343)
/// Members "<index>/<rel>" + top-level _grid_launcher_dirs.json manifest
/// {"<index>": "<raw unexpanded dir>"}. Unreadable dir → skipped AND
/// omitted from manifest; unreadable file → skipped; manifest member always
/// written. Returns (archive, files_added). (:431-476)
pub fn zip_native_save_dirs_for_upload(dirs: &[(String /*raw*/, PathBuf /*resolved*/)], ignore: &IgnoreSets) -> io::Result<(PathBuf, usize)>;
pub fn payload_is_zip(bytes: &[u8]) -> bool; // "PK" magic sniff (restore.py:180)
/// Extract with ignore filtering + zip-slip guard; NotImplemented-style
/// unsupported-method errors fall back to the system 7z path reused from
/// library/extract.rs. Returns extracted file count. (:253-278, :34,:163,:187)
pub fn extract_payload_zip(bytes: &[u8], dest: &Path, ignore: &IgnoreSets) -> Result<usize, String>;
pub fn cleanup_temp_archives(paths: &[PathBuf]); // best-effort unlink (:691)
```

**Pinned rules:**
- All three writers use deflate. Partial archive is unlinked and the error
  re-raised on any write failure (cloud_transfer.py:345,422,477).
- Zip-slip (doc 06 invariants): skip members with absolute paths or any
  `.`/`..`/empty component; resolved destination must remain under the
  resolved root; the 7z fallback extracts to a tempdir and re-applies the
  same member checks before moving files in.
- Ignore filtering applies on BOTH write and extract sides.
- 7-Zip fallback order: bundled `assets/tools/7z/7z.exe` (Windows), then
  `7z`, `7za`, `7zz` from PATH; all missing →
  `"No 7-Zip found to extract this archive."`. Reuse/extend the private
  system-7z helper in `library/extract.rs` (make it `pub(crate)`) rather
  than duplicating it.
- Sanitized title: port the same filename sanitizer the Python uses
  (follow cloud_transfer.py:286's helper).

- [ ] **Step 1: failing tests** — ports of `test_cloud_transfer.py:203`
  (OS metadata skipped), `:224,:256,:288` (native archive: unreadable dir
  omitted from manifest; locked file skipped — simulate with a directory
  where a file should be; all-fail → zero files, empty manifest), plus:
  - `grouped_archive_members_are_relative_to_the_common_parent`
  - `directory_archive_prefixes_the_dirname`
  - `temp_archive_name_shape_and_collision_suffix`
  - `extract_skips_zip_slip_members_and_blocked_names`
  - `extract_writes_nested_members_and_counts_them`
  - `payload_is_zip_sniffs_magic_only`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud archive writers and filtered extraction`

### Task 7: `cloud/scope.rs` — scope, block reasons, shared owner

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/scope.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveScope { PerGame, SharedSingle, SharedSlotted }
impl SaveScope { pub fn as_str(&self) -> &'static str /* "per-game" | "shared-single" | "shared-slotted" */ }

pub fn is_native_executable_platform(platform: &str) -> bool; // trimmed lowercased starts_with "windows" (selection.py — read is_native_executable_platform for the exact rule)
pub fn is_xemu_emulator_name(name: &str) -> bool;    // port grid_launcher/emulator predicates verbatim
pub fn is_redream_emulator_name(name: &str) -> bool;
pub fn is_retroarch_emulator_name(name: &str) -> bool; // reuse the M5 predicate if one exists in autoconfig; do not duplicate

pub fn cloud_save_scope(save_type: SaveType, emulator_name: &str, core_flags: Option<&CoreFlags>) -> SaveScope; // selection.py:56 — game arg is ignored in Python; don't take one
pub fn cloud_save_block_reason(platform: &str, save_type: SaveType, emulator_name: &str, core_flags: Option<&CoreFlags>) -> String; // selection.py:96
pub fn shared_sync_owner<'a>(token: &str, games: &'a [CloudGame]) -> Option<&'a CloudGame>; // cloud_mixin.py:392 — substring over title/platform/description/rom_file_name, must have resolvable rom id
```

**Pinned rules:**
- Scope: state → PerGame always; xemu → SharedSingle; Redream OR
  (RetroArch && flags.vmu_shared_saves) → SharedSlotted; else PerGame.
- Block reasons — the exact four strings and trigger chains from doc 06's
  block-reason table (native platform first, checked for both types; the
  three RetroArch core-flag reasons need name non-empty AND RetroArch AND
  flags `Some`). All flags default true for unknown cores — that is
  `CoreFlags::default()`, already ported.
- The flag-fallback ASYMMETRY between the block-reason wrapper and the
  scope wrapper (doc 06 quirk 7) lives in `ops.rs` (Task 16), not here —
  these pure fns just take `Option<&CoreFlags>`.

- [ ] **Step 1: failing tests** — port `test_cloud_save_block_reason.py:38-117`
  one test per branch (8 tests), plus `scope_state_is_always_per_game`,
  `scope_xemu_shared_single`, `scope_redream_and_vmu_flag_shared_slotted`,
  `shared_owner_requires_a_rom_id_and_matches_substrings_case_insensitively`.
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud save scope and block reasons`

### Task 8: `cloud/transfer.rs` — sidecars, job builders, short circuits, messages

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/transfer.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
pub const SUPPORTED_IMAGE_EXTENSIONS: [&str; 6]; // .jpg .jpeg .png .webp .gif .bmp (cloud_transfer.py:25)

#[derive(Debug, Clone, PartialEq)]
pub struct UploadJob { pub display_name: String, pub payload: Vec<(String, PathBuf)> } // field names "saveFile"/"stateFile"/"screenshotFile", ORDER preserved
#[derive(Debug, Default)]
pub struct BuiltJobs { pub jobs: Vec<UploadJob>, pub temp_archives: Vec<PathBuf> }

pub fn replaced_suffix_sidecar_path(path: &Path) -> Option<PathBuf>;   // game.ppst → game.png etc, first existing supported image (cloud_transfer.py — sidecar suffix-replace helper)
pub fn appended_image_sidecar_path(path: &Path) -> Option<PathBuf>;    // game.state1 → game.state1.png (:70)
pub fn session_screenshot_path(dirs: &[PathBuf], window: Option<Window>, ignore: &IgnoreSets) -> Option<PathBuf>; // :89 — newest in-window supported image, recursive, None on None-window
pub fn normalize_candidate_url(raw: &str) -> String;                    // :133 — percent-encode path safe="/%", re-encode query
pub fn state_content_candidate_paths(record: &serde_json::Value) -> Vec<String>;      // :141 — download_path, file_path, full_path, skip blanks
pub fn screenshot_download_candidate_paths(record: &serde_json::Value) -> Vec<String>; // :214 — same keys off record["screenshot"], skip when missing_from_fs

pub fn grouped_file_upload_jobs(files: &[PathBuf], field: &str, title: &str) -> io::Result<BuiltJobs>; // :361 — group by lowercased STEM for "saveFile", lowercased FULL NAME for "stateFile" (:354); 1 file → raw upload named after the file; ≥2 → archive named after first file's stem (:385)
pub fn directory_archive_upload_jobs(dirs: &[PathBuf], ignore: &IgnoreSets) -> io::Result<BuiltJobs>;  // cloud_upload.py:13 — display name = folder name
pub fn shared_single_upload_job(files: &[PathBuf], display_name: &str, title: &str) -> io::Result<BuiltJobs>; // all files, one archive (cloud_mixin.py:2519)
pub fn ppsspp_state_upload_jobs(dirs: &[PathBuf], tokens: &BTreeSet<String>) -> BuiltJobs; // :590 — *.ppst non-recursive, [^A-Z0-9]-stripped uppercase contains a token (or all when none), replaced-suffix sidecar, newest first, dedupe
pub fn retroarch_state_upload_jobs(files: &[PathBuf]) -> BuiltJobs;    // :638 — one job per file, appended sidecar
pub fn filter_upload_jobs_by_session_window(jobs: BuiltJobs, window: Option<Window>) -> BuiltJobs; // :668 — ANY in-window payload path keeps the job; no fallback (quirk 11); dropped jobs' temp archives move to the cleanup list, not leak

pub fn should_skip_known_latest(last_downloaded_id: &str, current_id: &str, local_latest_mtime: f64) -> bool; // :705
pub fn is_local_newer_than_server(local_mtime: f64, server_timestamp: f64) -> bool;   // :709 — local > 0 && local > server + 1.0

pub struct UploadOutcome { pub uploaded: usize, pub total: usize, pub failed: Vec<String> }
pub enum MessageSeverity { Info, Warning }
pub fn upload_completion_message(outcome: &UploadOutcome, save_type: SaveType, retention_failed: usize, retention_limit: u32) -> Option<(String, MessageSeverity)>; // cloud_upload.py:37 message table, verbatim
pub fn no_jobs_message(save_type: SaveType) -> String; // "No matching save files or save folders were found to upload." / PPSSPP variant lives at its call site (cloud_upload.py:30-35)
```

**Pinned rules:** every anchor above, plus: `session_screenshot_path`
returns `None` for no dirs or a `None` window; recurses; skips blocked
basenames; picks most recent. The message table (doc 06 "Upload execution"):
`Cloud upload failed for all matching files.` (warning, failures + zero
success); `Uploaded N save files|save states. Failed: <first 5 names>`
(warning); `Uploaded N …. Could not remove K older cloud saves for
retention limit L.` (warning); `Uploaded N save files|save states.` (info).
Singular/plural follows the Python (read cloud_upload.py:37-60 for the
exact pluralization).

- [ ] **Step 1: failing tests** — ports of `test_cloud_transfer.py:315`
  (ppsspp sidecars: only supported images), `:333` (appended sidecar),
  `:354,363,371` (screenshot candidate ordering/blanks/empty),
  `:376-452` (retroarch jobs: sidecar attached/omitted, one per slot,
  non-image ignored, sidecar in payload, tuple shape), `:463,490`
  (grouping: same stem → one archive; distinct state slots separate),
  `TestSessionScreenshotPath` `:631` (all nine cases), plus:
  - `known_latest_requires_matching_id_and_positive_local_mtime` (all
    three clauses toggled)
  - `local_newer_needs_more_than_one_second`
  - `window_job_filter_keeps_any_inwindow_path_and_reaps_dropped_temp_archives`
  - `normalize_candidate_url_percent_encodes_path_and_query`
  - `completion_message_table` (all four rows byte-exact)
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud transfer helpers, job builders, short circuits`

### Task 9: `cloud/restore.rs` — records, selection, placement

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/restore.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
pub fn record_timestamp(record: &serde_json::Value) -> f64;             // cloud_restore.py:14 — updated_at preferred, created_at fallback; ISO-8601 parse, 0.0 on failure (match the Python's accepted formats incl. trailing Z)
pub fn relative_timestamp_text(ts: f64, now: f64) -> String;            // :30 — port the BUGGY bucket table verbatim (minutes bucket unreachable above 90s; doc 06 "Manual actions"); zero → "Unknown"
pub fn server_records_from_payload(payload: &serde_json::Value) -> Vec<serde_json::Value>; // :79 — reject non-list/non-dict, drop blank ids, dedupe on string id first-wins
pub fn sort_server_records_by_recency(records: &mut Vec<serde_json::Value>); // (timestamp, numeric id) desc
pub fn latest_server_record<'a>(records: &'a [serde_json::Value], emulator_name: &str) -> Option<&'a serde_json::Value>; // ci emulator filter, FALLS BACK to all records when nothing matches (contrast retention!) — read cloud_restore.py for the exact fn
pub fn latest_server_records_by_slot(records: &[serde_json::Value], emulator_name: &str) -> Vec<serde_json::Value>; // :135 — slot key: slot lower, else file_name stem lower, else "__default__"; newest per slot
pub fn preferred_restore_target_path(record_file_name: &str, fallback_name: &str, candidates: &[PathBuf], directories: &[PathBuf]) -> Option<PathBuf>; // :150 — the 7-step ladder from doc 06 verbatim
pub fn restore_single_save_payload(payload: &[u8], target: &Path, ignore: &IgnoreSets) -> Result<Option<PathBuf>, String>; // :186 — zip → extract into PARENT, return parent (None when 0 members); else overwrite target
pub fn restore_single_state_payload(payload: &[u8], target: &Path, screenshot: Option<(&[u8], &str)>, ignore: &IgnoreSets) -> Result<Option<PathBuf>, String>; // :219 — same + sidecar "<target><ext>" ONLY for non-zip payloads
```

**Pinned rules:** doc 06 "Restore — saves" target ladder (7 steps), the
fallback names (`<sanitized title>.srm` / `.state` — built by the CALLER,
ops.rs), and the sidecar-extension default `.png`. Timestamp parsing must
accept the formats the Python `datetime.fromisoformat` path accepts plus
the `Z` suffix rewrite — read cloud_restore.py:14-28 and port exactly.

- [ ] **Step 1: failing tests** — ports of `test_cloud_restore.py:21`
  (bucket table: assert `just now`, `1 minute ago`, the 120s→`1 hour ago`
  BUG, days, weeks, Unknown), `:28` (recency sort), `:39` (per-slot
  latest), `:51` (exact candidate filename preferred), `:123-244` (state
  placement: nested candidate kept, sidecar written/omitted/re-extended,
  no sidecar for zip, zip unpacked into matching directory), plus
  `records_from_payload_dedupes_and_drops_blank_ids`,
  `latest_record_falls_back_to_all_when_emulator_never_matches`,
  `target_ladder_steps_3_through_7` (one test per remaining step).
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud record selection and payload placement`

### Task 10: `cloud/native.rs` — native paths, wine translation, manifest restore

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/native.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
pub fn resolve_native_save_dir(raw: &str, windows_documents: Option<&Path>, wine_prefix: Option<&Path>) -> PathBuf; // cloud_transfer.py:484
pub fn normalize_manual_save_path(path: &Path) -> String;  // :559 — %APPDATA%, %LOCALAPPDATA%, %USERPROFILE%\AppData\LocalLow, %USERPROFILE%\Documents, %USERPROFILE% prefix rewrites; forward slashes normalized
pub fn translate_windows_path_to_wine_prefix(raw: &str, prefix: &Path) -> Option<PathBuf>; // the helper the two pinned Python test tables cover
/// native_multi_dir restore: parse the manifest (missing/malformed → empty
/// map, cloud_mixin.py:2191), split each member's leading "<index>/",
/// resolve root from manifest[index] else fallback_dirs[0] else skip,
/// zip-slip-check per member, create parents, overwrite. Returns files written.
pub fn restore_native_multi_dir_archive(payload: &[u8], fallback_dirs: &[PathBuf], windows_documents: Option<&Path>, wine_prefix: Option<&Path>) -> Result<usize, String>;
pub fn native_save_paths(pcgw: &[String], manual: &[String]) -> Vec<String>; // pcgw + [m for m in manual if m not in pcgw] (cloud_mixin.py:2689)
```

**Pinned rules:** env expansion uses process env (tests set vars under the
existing `test_env` lock). Windows Documents redirection logic per doc 06
"Platform differences" — pure-parameter form so non-Windows tests drive
both branches. The wine translation walks the documented prefix mapping
(`%USERPROFILE%` → `drive_c/users/<user>`… read the Python helper and its
two test tables). `native_dir:<raw path>` legacy restore branch stays in
ops.rs; this module only supplies path resolution.

- [ ] **Step 1: failing tests** — ports of `test_cloud_transfer.py:30,45,62,82`
  (resolve: plain, no-redirect, redirected Documents, non-Documents
  untouched), `:98-188` (normalize: all six prefix cases + slashes),
  `WinePrefixPathTranslationTests` (`:734`) and
  `TranslateWindowsPathToWinePrefixTests` (`:824`) — the full pinned
  tables — plus `manifest_restore_resolves_indices_and_blocks_zip_slip`,
  `manifest_restore_degrades_to_empty_manifest`,
  `native_save_paths_dedupes_manual_against_pcgw`.
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: native save path resolution and manifest restore`

### Task 11: RomM cloud endpoints + `cloud/retention.rs`

**Files:**
- Modify: `rewrite/crates/grid-core/src/romm/mod.rs`,
  `rewrite/crates/grid-core/Cargo.toml` (reqwest `multipart` feature;
  wiremock dev-dependency if not already present)
- Create: `rewrite/crates/grid-core/src/cloud/retention.rs`
- Modify: `cloud/mod.rs`

**Interfaces (Produces):**
```rust
impl RommClient {
    pub async fn saves_for_rom(&self, rom_id: &str) -> Result<serde_json::Value, RommError>;   // GET /api/saves?rom_id=
    pub async fn states_for_rom(&self, rom_id: &str) -> Result<serde_json::Value, RommError>;  // GET /api/states?rom_id=
    pub async fn save_content(&self, id: &str) -> Result<Vec<u8>, RommError>;                  // GET /api/saves/{id}/content, id percent-encoded safe=""
    pub async fn state_record(&self, id: &str) -> Result<serde_json::Value, RommError>;        // GET /api/states/{id}
    pub async fn get_relative_bytes(&self, candidate: &str) -> Result<Vec<u8>, RommError>;     // normalize_candidate_url + leading "/" + get_bytes; D4: an absolute http(s) candidate returns Err — callers skip to the next candidate
    pub async fn upload_save(&self, rom_id: &str, emulator: &str, slot: Option<&str>, payload: &[(String, PathBuf)]) -> Result<(), RommError>; // POST /api/saves?rom_id&emulator&overwrite=true[&slot] multipart
    pub async fn upload_state(&self, rom_id: &str, emulator: &str, payload: &[(String, PathBuf)]) -> Result<(), RommError>;                     // POST /api/states?rom_id&emulator multipart — NO slot, NO overwrite
    pub async fn delete_save(&self, id: i64) -> Result<u16, RommError>;   // POST /api/saves/delete {"saves":[id]} — returns status so retention can treat 404/410 as success
    pub async fn delete_state(&self, id: i64) -> Result<u16, RommError>;  // POST /api/states/delete {"states":[id]}
}

// retention.rs
/// doc 06 "Retention pruning": refetch, ci-emulator filter with NO
/// fall-back-to-all (contrast latest_server_record), sort (ts, numeric id)
/// desc, group by slot key, keep `keep` per group, delete stale one call
/// each. Non-integer id → failed without a request; BLANK id silently
/// skipped (counted in neither list); 404/410 = success; other errors →
/// failed, continue. Returns (deleted, failed_ids).
pub async fn prune_server_save_records(client: &RommClient, rom_id: &str, emulator_name: &str, keep: u32) -> (usize, Vec<String>);
```

Multipart part file names: the payload path's file name; field names come
straight from the job payload. `rom_id` is sent as the string GRID carries
(doc 06 contract note). Keep every new fn's error mapping consistent with
the existing `get_response` (401/403 → the existing auth error variant).

- [ ] **Step 1: failing tests** (wiremock, `#[tokio::test]`):
  - `saves_and_states_list_send_rom_id_query`
  - `save_content_percent_encodes_the_id` (id with a `/`)
  - `upload_save_sends_query_and_multipart_shape` (assert `overwrite=true`, optional slot present/absent, `saveFile` part name + file name)
  - `upload_state_sends_no_slot_and_no_overwrite`
  - `delete_bodies_use_the_right_keys` (`saves` vs `states`)
  - `get_relative_bytes_prefixes_a_slash_and_rejects_absolute_urls` (D4)
  - `prune_keeps_n_per_slot_and_treats_404_as_success`
  - `prune_mismatched_emulator_prunes_nothing`
  - `prune_blank_id_skipped_non_integer_failed`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: romm save/state endpoints and retention pruning`

### Task 12: `cloud/dirs.rs` — sync-directory resolution

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/dirs.rs`
- Modify: `cloud/mod.rs`,
  `rewrite/crates/grid-core/src/launch/profiles.rs` (EmulatorProfile +
  RawProfile gain `screenshot_directories: Vec<String>`, `skip_serializing`
  like the other five cloud fields)

**Interfaces (Produces):**
```rust
pub enum PathKey { SavePaths, StatePaths }
pub struct ResolveContext<'a> {
    pub emulator_dir: Option<&'a Path>,   // parent of the exe
    pub library_dir: &'a str,             // config.library_path (may be "")
    pub config_dir: &'a Path,             // launcher config directory
    pub windows_documents: Option<&'a Path>,
}
/// The full doc 06 "Local paths scanned" pipeline: entry paths win outright
/// (skipping ALL per-emulator probing); else profile save/state_directories;
/// per-emulator override prepend/append for the 13 emulators in doc 06's
/// list (RetroArch via autoconfig::retroarch::directory_settings + the
/// literal fallbacks, then Azahar, Dolphin, PCSX2, RPCS3(save), Vita3k(save),
/// Cemu(save), PICO-8(save), FBNeo, MAME, Eden(save), Xenia, Redream,
/// xemu(save) — each using the matching autoconfig::readers override fn);
/// token expansion; retroarch `default`/`:\` notations; relative→emulator
/// dir; keep only existing (dir OR file); ci-dedupe. Returns (paths,
/// explicit_file_roots).
pub fn resolved_sync_directory_paths(entry: &EmulatorEntry, profile: Option<&EmulatorProfile>, key: PathKey, ctx: &ResolveContext) -> (Vec<PathBuf>, Vec<PathBuf>);
pub fn resolved_screenshot_directories(entry: &EmulatorEntry, profile: Option<&EmulatorProfile>, ctx: &ResolveContext) -> Vec<PathBuf>; // profile-only, must be dirs, no %DOCUMENTS%
pub fn expand_sync_path(raw: &str, key_is_save: bool, ctx: &ResolveContext) -> Option<PathBuf>; // the token table (env vars, %EMULATOR_DIR%, %LIBRARY_DIR%, %CONFIG_DIR%, %DOCUMENTS%) + retroarch notations
```
No memoization in grid-core — Python's per-(name,path,key) cache is a UI
concern; ops.rs (Task 16) holds the cache alongside the emulator-entry
cache so both clear together on config save.

**Pinned rules:** doc 06 lines 95-152 are the contract. Read
cloud_mixin.py:618-990 for each per-emulator block; each override list
comes from the matching `autoconfig::readers` fn already ported in M5
(pcsx2/dolphin/azahar/eden/…), so this task is plumbing + expansion, not
re-parsing. `%DOCUMENTS%` resolves via `ctx.windows_documents`, else
`%USERPROFILE%\Documents` — non-Windows: plain env expansion only.

- [ ] **Step 1: failing tests** (tempdirs, env via `test_env` lock):
  - `entry_paths_win_and_skip_all_probing` (a RetroArch entry with save_paths set: the retroarch reader is NOT consulted — point it at a poisoned config that would add paths)
  - `retroarch_prepends_config_dir_and_appends_literal_fallbacks`
  - `retroarch_default_sentinel_and_colon_slash_notation`
  - `tokens_expand_emulator_library_config_dirs`
  - `relative_paths_resolve_against_the_emulator_dir`
  - `existing_files_are_kept_as_explicit_file_roots`
  - `results_dedupe_case_insensitively`
  - `screenshot_dirs_are_profile_only_and_must_be_directories`
  - one per-emulator override wiring test for a representative trio
    (PCSX2, Dolphin, xemu-save) asserting the reader output lands ahead of
    profile paths
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud sync-directory resolution`

### Task 13: `fatx/` — layout, FAT, directories, read path, builder

**Files:**
- Create: `rewrite/crates/grid-core/src/fatx/mod.rs`, `layout.rs`,
  `fat.rs`, `dir.rs`, `image.rs`, `builder.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs` (`pub mod fatx;`)

**CLEAN-ROOM RULE (binding, from the spec):** implement from public format
documentation (xboxdevwiki.net "FATX", Free60 wiki) and first principles
ONLY. Do NOT read pyfatx, libfatx, fatx-tools, the `fatx` crate, or any
GPL FATX source. Original-Xbox variant: LITTLE-ENDIAN. If any observed
detail conflicts with these pinned constants, stop and surface it — do not
guess.

**Interfaces (Produces):**
```rust
// layout.rs
pub const RETAIL_PARTITION_E_OFFSET: u64 = 0xABE8_0000;
pub const FATX_SUPERBLOCK_SIZE: u64 = 0x1000;
pub const FATX_SIGNATURE: [u8; 4] = *b"FATX";
pub struct Superblock { pub volume_id: u32, pub sectors_per_cluster: u32, pub root_dir_first_cluster: u32 }
impl Superblock { pub fn parse(bytes: &[u8]) -> Result<Self, FatxError> } // signature at offset 0, volume id u32le at 4, sectors/cluster u32le at 8, root cluster u32le at 12; sectors are 512 bytes; sectors_per_cluster must be a power of two in 1..=128
pub struct Geometry { pub cluster_size: u64, pub cluster_count: u64, pub fat_offset: u64, pub fat_size: u64, pub data_offset: u64, pub fat32: bool }
pub fn geometry(partition_size: u64, sb: &Superblock) -> Result<Geometry, FatxError>;
// FAT16X when cluster_count < 0xFFF0, else FAT32X (32-bit entries).
// fat_offset = 0x1000 (after superblock); fat_size = cluster_count+1 entries
// of 2 or 4 bytes, rounded UP to a 0x1000 boundary; data_offset follows.
// Cluster numbering starts at 1 (entry 0 reserved); data cluster N begins at
// data_offset + (N-1)*cluster_size.

// fat.rs
pub struct Fat { /* entries in memory */ }
impl Fat {
    pub fn read(io: &mut (impl Read + Seek), geo: &Geometry, base: u64) -> Result<Self, FatxError>;
    pub fn chain(&self, first: u32) -> Result<Vec<u32>, FatxError>; // follow until >= 0xFFF8/0xFFFFFFF8 end marker; detect loops (visited set) → FatxError::CorruptChain
    pub fn free_clusters(&self) -> impl Iterator<Item = u32>;       // entry == 0
    pub fn allocate(&mut self, count: usize) -> Result<Vec<u32>, FatxError>; // links them, marks end
    pub fn free_chain(&mut self, first: u32);
    pub fn write(&self, io: &mut (impl Write + Seek), geo: &Geometry, base: u64) -> Result<(), FatxError>;
}

// dir.rs — 64-byte entries:
// [0] name_len (0xFF = end-of-directory marker, 0xE5 = deleted),
// [1] attributes (0x10 = directory), [2..44] name (space/0xFF padded),
// [44..48] first_cluster u32le, [48..52] file_size u32le,
// [52..60] create/modify FATX timestamps (u16 date, u16 time pairs).
pub struct DirEntry { pub name: String, pub is_dir: bool, pub first_cluster: u32, pub size: u32 }
pub fn parse_dir_cluster(bytes: &[u8]) -> Vec<(usize /*slot offset*/, DirEntry)>;
pub fn encode_dir_entry(entry: &DirEntry, timestamp: u32) -> [u8; 64];

// image.rs
pub struct FatxPartition { /* file handle, base offset, geometry, fat */ }
impl FatxPartition {
    pub fn open(path: &Path, base_offset: u64, partition_size: u64) -> Result<Self, FatxError>;
    pub fn validate(path: &Path, base_offset: u64) -> Result<(), FatxError>; // superblock + FAT bounds only, read-only — the sniffer's backend
    pub fn read_tree(&mut self, dir_path: &str, dest: &Path) -> Result<usize, FatxError>; // "UDATA" etc; returns files extracted; missing dir → Ok(0)
    pub fn list_dir(&mut self, dir_path: &str) -> Result<Vec<DirEntry>, FatxError>;
}

// builder.rs (pub, used by tests and by fatx write tests in Task 14)
pub struct FatxImageBuilder { /* cluster size, files to place */ }
// builds a raw file containing ONLY the E: partition region layout at a
// configurable base offset (0 for unit tests; the retail offset for
// integration tests with a sparse file), formats superblock + FAT + root
// dir, places files/dirs.
```
Name rules: max 42 bytes, case preserved on write, compared
case-insensitively on lookup. Timestamps: FATX packed date/time (year
offset 2000 — if the public docs disagree with each other on the epoch,
pick DOS-style year-1980 packing, note it in the module docs, and let the
Task 14 pyfatx oracle test settle it empirically).

- [ ] **Step 1: failing tests**
  - `superblock_rejects_bad_signature_and_bad_cluster_size`
  - `geometry_selects_fat16x_below_the_threshold_and_fat32x_above`
  - `geometry_rounds_fat_size_to_a_page_boundary`
  - `builder_roundtrip_read_tree_extracts_placed_files` (nested dirs, 3 files, content byte-compared)
  - `read_tree_of_a_missing_dir_returns_zero`
  - `chain_loop_detection_errors_instead_of_hanging`
  - `deleted_and_end_markers_terminate_directory_parsing`
  - `names_compare_case_insensitively_on_lookup`
  - `validate_rejects_truncated_images` (file shorter than fat_offset+fat_size)
  - `retail_offset_integration` (sparse temp file, builder at 0xABE80000, validate + read succeed)
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: clean-room FATX layout, FAT, directory read path`

### Task 14: `fatx/` write path + pyfatx oracle

**Files:**
- Modify: `rewrite/crates/grid-core/src/fatx/image.rs`, `fat.rs`, `dir.rs`,
  `builder.rs`
- Create: `rewrite/crates/grid-core/tests/fatx_oracle.rs` (integration test)

**Interfaces (Produces):**
```rust
impl FatxPartition {
    /// Inject a local tree under `dir_path` ("UDATA"/"TDATA"), creating
    /// directories as needed and OVERWRITING existing files (free old
    /// chain, allocate new). Returns files written.
    pub fn write_tree(&mut self, dir_path: &str, src: &Path) -> Result<usize, FatxError>;
    pub fn remove_tree(&mut self, dir_path: &str) -> Result<(), FatxError>;
}
```

**Pinned rules:**
- **Crash ordering (spec):** for each file: write data clusters, write the
  updated FAT, THEN write the directory entry. A crash mid-file orphans
  clusters (fsck-able) but never leaves a directory entry pointing at
  garbage. Superblock is never rewritten.
- Every write path calls `validate`-grade checks first (signature, FAT
  bounds) and re-checks allocation bounds before seeking; out-of-space →
  `FatxError::NoSpace`, nothing partially applied beyond the documented
  orphan window.
- Overwrite = free the old chain in the in-memory FAT, allocate fresh,
  update the existing directory slot in place (size, first cluster,
  timestamps).
- **pyfatx oracle** (`tests/fatx_oracle.rs`): `#[ignore]`-free but
  self-skipping — probe `python3 -c "import fatx"`; when importable, build
  an image with our writer, list/extract it via a small inline python
  script using pyfatx as a BLACK BOX subprocess, and byte-compare; when
  not importable, print a skip note and pass. This is the empirical
  endianness/epoch check the spec requires: if the oracle disagrees, the
  constants are wrong — fix the constants, never fudge the test.

- [ ] **Step 1: failing tests**
  - `write_tree_roundtrips_through_read_tree` (write with writer A, read with reader — nested, multi-cluster file > 2 clusters, exact bytes)
  - `write_overwrites_an_existing_file_and_frees_its_old_chain` (free-cluster count restored)
  - `write_grows_and_shrinks_files_correctly`
  - `write_creates_intermediate_directories`
  - `remove_tree_frees_every_chain`
  - `no_space_errors_cleanly` (tiny image)
  - `crash_ordering_fat_before_direntry` (instrument with a failing writer wrapper after the FAT write: directory must NOT name the new file)
  - oracle test as above
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: FATX write path with crash-safe ordering + pyfatx oracle`

### Task 15: `cloud/xemu_sync.rs` + autoconfig D3

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/xemu_sync.rs`
- Modify: `cloud/mod.rs`, `rewrite/crates/grid-core/src/autoconfig/xemu.rs`

**Interfaces (Produces):**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XemuImageStatus {
    Ready,                 // raw + E: superblock validates
    NotRaw,                // qcow2 magic "QFI\xfb" (or any non-FATX leading bytes with the qcow2 magic)
    UnsupportedLayout,     // raw-looking but E: superblock invalid
    Missing,               // hdd_path blank or file absent
}
pub fn classify_hdd_image(hdd_path: &str) -> XemuImageStatus; // sniff 4 bytes; qcow2 magic → NotRaw; else fatx::FatxPartition::validate at RETAIL_PARTITION_E_OFFSET
pub fn block_reason_for_status(status: &XemuImageStatus) -> Option<String>;
// Ready → None. The three strings (spec "xemu flow", user-facing):
// NotRaw → "xemu cloud sync needs a raw HDD image (xbox_hdd.img). Convert your qcow2 once with: qemu-img convert -O raw xbox_hdd.qcow2 xbox_hdd.img"
// UnsupportedLayout → "The xemu HDD image is not a standard retail-layout FATX image, so cloud sync is unavailable."
// Missing → "No xemu HDD image is configured, so cloud sync is unavailable."
pub fn xemu_hdd_path_from_config(emulator_path: &str) -> Option<String>; // read sys.files.hdd_path via the M5 xemu.toml reader/parse (single-quoted value form)

/// Extract E:/UDATA + E:/TDATA into a temp dir laid out as UDATA/... and
/// TDATA/..., zip it (deflate, cloud temp naming), return (archive, files).
/// Zero files → Ok(None), no archive left behind.
pub fn build_xemu_save_archive(hdd_path: &str, title: &str) -> Result<Option<(PathBuf, usize)>, String>;
/// True when the zip's top level contains a UDATA/ or TDATA/ member — the
/// new-format check for D2. A legacy whole-image record fails this.
pub fn archive_is_udata_tdata(payload: &[u8]) -> bool;
/// Extract payload to a temp dir, then write_tree UDATA and TDATA into E:.
/// Legacy payload (archive_is_udata_tdata false) → Err with the D2 notice:
/// "This cloud save is a legacy whole-image xemu backup and cannot be restored by this version. Upload a new save to replace it."
pub fn inject_xemu_save_archive(hdd_path: &str, payload: &[u8]) -> Result<usize, String>;
```

**autoconfig D3** (`autoconfig/xemu.rs`): the required-BIOS probe accepts
`xbox_hdd.img` OR `xbox_hdd.qcow2` as satisfying the HDD slot (probe list
becomes: mcpx, complex, and hdd-either); the `sys.files` `hdd_path` default
points at `xbox_hdd.img` when that file exists in the base dir, else
`xbox_hdd.qcow2` (add-only semantics unchanged — an existing key is never
touched). Update the module's doc comment to cite deviation D3.

- [ ] **Step 1: failing tests**
  - `classify_detects_qcow2_magic_ready_missing_and_bad_layout` (builder image at retail offset → Ready)
  - `block_reason_strings_are_exact`
  - `hdd_path_read_from_xemu_toml_single_quotes`
  - `build_archive_lays_out_udata_and_tdata_roots` (builder image with files in both; zip members checked)
  - `build_archive_returns_none_when_both_trees_are_empty`
  - `archive_format_check_accepts_new_and_rejects_legacy` (legacy = zip with a `something.qcow2` member; also non-zip bytes)
  - `inject_writes_both_trees_and_overwrites` (roundtrip: build → wipe → inject → read_tree compare)
  - `inject_rejects_a_legacy_archive_with_the_d2_notice`
  - autoconfig: `probe_accepts_img_or_qcow2_for_the_hdd_slot`, `hdd_path_default_prefers_an_existing_img`, `existing_hdd_path_key_is_never_touched`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: xemu raw-disk sync bridge + xbox_hdd.img autoconfig (D1-D3)`

### Task 16: `cloud/ops.rs` — orchestration

**Files:**
- Create: `rewrite/crates/grid-core/src/cloud/ops.rs`
- Modify: `cloud/mod.rs`

This is the integration task: it ports the cloud_mixin control flow onto
the pure modules. Everything takes plain data + `&RommClient`; nothing
touches Tauri. Dialog strings are RETURNED (a `Vec<CloudMessage>` of
`{ text, severity }`), never printed.

**Interfaces (Produces):**
```rust
pub struct CloudContext<'a> {
    pub config: &'a Config,
    pub profiles: &'a [EmulatorProfile],
    pub all_games: &'a [CloudGame],          // registry rows (+ server cache when the caller has one)
    pub resolve_ctx: ResolveContext<'a>,
    pub active_sessions: &'a [ActiveSessionRef],
    pub now: f64,
}
pub struct CloudCaches { /* sync-dir memo + emulator-entry memo; pub fn clear(&mut self) */ } // cleared on config save (parity)

pub fn resolved_cloud_emulator_entry(ctx: &CloudContext, caches: &mut CloudCaches, game: &CloudGame, save_type: SaveType) -> Option<EmulatorEntry>; // cloud_mixin.py:175 — cache key "<title>::<platform>::<save_type>" (QUIRK: no rom id); default resolution, then the Emulators-platform shared-token scan skipping per-game scopes for saves
pub fn block_reason_for_game(ctx…, game, save_type, entry: Option<&EmulatorEntry>) -> String; // the wrapper asymmetry: flags ONLY when RetroArch AND a default core is configured (cloud_mixin.py:116); PLUS the xemu image block reasons (Task 15) when the resolved emulator is xemu and save_type == Save
pub fn scope_for_game(ctx…, game, save_type, entry) -> SaveScope;      // wrapper WITH the retroarch_core_flags_for_platform fallback (cloud_mixin.py:162)
pub fn cloud_sync_rom_id(ctx…, game, save_type) -> Option<String>;     // shared-owner indirection for saves (cloud_mixin.py:398-458); states never
pub fn cloud_sync_targets(ctx…, caches, game, entry, save_type) -> (Vec<PathBuf>, Vec<PathBuf>); // the ten-branch dispatch (doc 06 "Candidate discovery" table) + file-root rescan + session filtering; xemu (SharedSingle via raw image) contributes NO generic candidates — its saves live inside the image
pub fn latest_local_save_mtime(ctx…, caches, game, entry_name: &str) -> f64;  // cloud_mixin.py:1559 — stub entry {"name", "", "%rom%", "auto"} when unconfigured; xemu: the raw image file's own mtime stands in (D1 note)
pub fn latest_local_state_mtime(ctx…, caches, game, entry_name: &str) -> f64; // :1534 — 0.0 for RPCS3

pub struct UploadReport { pub uploaded: usize, pub total: usize, pub failed: Vec<String>, pub messages: Vec<CloudMessage> }
pub async fn upload_cloud_files_for_game(client: &RommClient, ctx…, caches, game: &CloudGame, save_type: SaveType) -> UploadReport;
pub async fn restore_cloud_save_for_game(client, ctx…, caches, game, record: Option<&serde_json::Value>, skip_if_local_newer: bool, skip_if_known_latest: bool) -> (bool, Vec<CloudMessage>, SyncStateUpdate);
pub async fn restore_cloud_state_for_game(client, ctx…, caches, game, record: Option<&serde_json::Value>, skip_if_known_latest: bool) -> (bool, Vec<CloudMessage>, SyncStateUpdate);
pub async fn upload_native_saves_for_game(client, ctx…, game, pcgw_paths: &[String]) -> UploadReport;
pub async fn restore_native_cloud_save_for_game(client, ctx…, game, pcgw_paths: &[String]) -> (bool, Vec<CloudMessage>);
pub async fn fetch_cloud_records(client, ctx…, caches, game, save_type) -> Result<Vec<serde_json::Value>, String>; // list + state image-name filtering (cloud_mixin.py:1653)
pub async fn delete_cloud_record(client, save_type, id: i64) -> Result<(), String>; // 404/410 success
```

**Pinned rules (the control flow, all from doc 06):**
- Upload preconditions in order (native→delegate; block reason; rom id;
  entry; directories; RPCS3+state refusal) with the exact messages.
- Save-branch job construction: folder targets → directory archives;
  SharedSingle + files → one archive named
  `"<emulator name or 'Shared Save'> Storage"`; else grouped. **xemu
  (SharedSingle, raw image Ready): the single job comes from
  `build_xemu_save_archive`** — display name per the same rule; zero
  files → the no-jobs message. PPSSPP-state and RetroArch-state branches +
  generic-state fallback per doc 06. Screenshot fallback fills every job
  lacking one. Slot assignment table (states never; shared-media; vmu
  regex over display name then payload stems/names; per-game empty).
- Execution: one POST per job, per-job error isolation, temp cleanup
  after the loop, retention prune for saves with ≥1 success (limit from
  `cloud_save_retention_limit`, min 1 — D7), completion message table.
- Save restore: the 9-step flow incl. record-emulator override rule,
  per-slot selection for shared scopes, the TWO short circuits (known
  latest only for per-game saves; local-newer with the PCSX2-no-serials
  exemption), folder-save emulators extract into `directories[0]`,
  **D6:** for multi-record (shared-slotted) restores, download and unpack
  EVERY record into a staging temp dir first; only when all succeed, move
  results into place (per-record placement semantics preserved); any
  failure before commit leaves local files untouched (deviation from
  Python's abort-mid-way). Single-record restores keep Python's direct
  placement. **xemu restore: `inject_xemu_save_archive`; legacy records →
  the D2 notice, treated as "nothing restored"** (not an error dialog when
  auto-triggered). Persist `last_downloaded_save_id` + `last_server_timestamp`
  via the returned `SyncStateUpdate`.
- State restore: rom id resolved directly (never the shared-owner
  indirection), RPCS3 refusal, single record, two-step
  content resolution walking `state_content_candidate_paths` through
  `get_relative_bytes` (absolute-URL candidates skipped — D4), all-fail →
  the `ValueError` message string, screenshot best-effort, sidecar rules,
  `last_downloaded_state_id`.
- Native upload/restore: the numbered flows from doc 06 including
  `native_dir:<raw path>` legacy read support and the always-1 total.
- The `Emulators`-platform gates for the panel
  (`_details_cloud_mode_supported` — port as
  `pub fn details_cloud_mode_supported(...) -> bool`).

- [ ] **Step 1: failing tests** — wiremock-backed `#[tokio::test]` plus
  pure tests; minimum set:
  - `upload_precondition_messages_fire_in_order` (each of the six, exact strings)
  - `shared_single_bundles_everything_into_one_named_job`
  - `slot_assignment_table` (vmu match in display name vs payload path; shared-media; states never)
  - `upload_isolates_per_job_failures_and_prunes_after` (mock: first POST 500, second 200; prune called with limit)
  - `save_restore_prefers_supplied_record_and_enforces_the_emulator_rule`
  - `known_latest_skip_only_for_per_game_scope`
  - `local_newer_skip_exempts_pcsx2_without_serials`
  - `shared_slotted_restore_is_atomic` (second record's download 500s → NO local file changed; then both 200 → both placed) — D6
  - `folder_save_emulators_extract_into_the_first_directory`
  - `state_restore_walks_candidates_and_skips_absolute_urls` (D4)
  - `xemu_upload_builds_the_udata_archive_and_restore_injects` (builder image end-to-end against wiremock)
  - `xemu_legacy_record_is_skipped_with_the_notice` (D2)
  - `xemu_block_reasons_surface_through_block_reason_for_game`
  - `native_upload_manifest_flow_and_retention_key`
  - `emulator_cache_key_omits_rom_id_and_clears_with_caches`
  - `details_cloud_mode_supported_gate_table` (each doc-06 bullet)
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: cloud upload/restore orchestration (D2, D4, D6, D7)`

### Task 17: app wiring — CloudService, commands, auto triggers

**Files:**
- Modify: `rewrite/app/src-tauri/src/lib.rs`, `commands.rs`,
  `rewrite/crates/grid-core/src/launch/mod.rs`
- Modify: `rewrite/app/src/lib/api.ts` (types + invoke wrappers only; UI is Task 19)

**Interfaces (Produces):**
```rust
// src-tauri: CloudService in AppState
pub struct CloudService { /* Arc<SessionState>, CloudCaches (Mutex), auto-upload pool */ }
// Tauri commands (all Result<_, String>):
cloud_panel_info(game, save_type) -> CloudPanelInfo   // { supported, block_reason, scope, records_loading handled frontend-side }
cloud_records(game, save_type) -> Vec<CloudRecordDto> // id, file_name, emulator, slot, size_text (library download format_size reuse), absolute + relative time (restore.rs), restorable flag + disabled reason
cloud_upload(game, save_type) -> UploadReportDto
cloud_restore(game, save_type, record_id: Option<String>) -> RestoreReportDto
cloud_delete(game, save_type, record_id: i64) -> ()
native_save_paths(game) -> { pcgw: Vec<String>, manual: Vec<String> }   // pcgw from Task 18's cache
native_add_manual_save_path(game, path) / native_remove_manual_save_path(game, path)
cloud_settings() / set_cloud_settings({ download_on_launch, upload_on_exit, skip_if_local_newer, upload_delay_seconds, retention_limit })
```

**Auto triggers:**
- **Before launch:** in the existing launch command path, when
  `auto_cloud_save_download_on_launch` && connected: run save restore
  (`skip_if_local_newer` from config, `skip_if_known_latest=true`) then
  state restore (`skip_if_known_latest=true`), dialogs suppressed (messages
  logged at debug, not surfaced), each gated on its own block reason —
  BEFORE the process spawns (parity: details_view_mixin.py:1497). Failures
  never block the launch.
- **After exit:** `LaunchService` gains an optional
  `set_session_finished_hook(Box<dyn Fn(GameSession) + Send + Sync>)`
  invoked from the poll loop for each reaped session (after the snapshot
  emit, no locks held). The hook records
  `last_session_started_at`/`ended_at` (only when started_at > 0), then —
  when `auto_cloud_save_upload_on_exit` && connected — schedules the auto
  upload after `auto_cloud_save_upload_delay_seconds` (tokio sleep; 0 =
  immediate).
- **D5 — serialization:** the auto-upload worker is a small tokio task
  pool (max 2 concurrent) keyed by `game_key`: a trigger for a game with
  an in-flight upload is coalesced (dropped — the running upload's plan
  already covers the newest mtimes or the next session re-triggers).
  Result bookkeeping via `summarize_auto_cloud_upload_result` →
  `apply_sync_update` → config save. `uploaded_at` string:
  UTC now, ISO-8601 seconds, `+00:00`→`Z`.
- Session registration parity: launch already records sessions; add the
  sync-state stamp (`last_session_started_at = now`, `ended_at = 0.0`)
  at spawn when at least one of the two block reasons is empty
  (cloud_mixin.py:2825).

- [ ] **Step 1: failing tests** — grid-core: hook fires per reaped session
  (extend the existing sessions tests with the hook installed);
  `auto_upload_pool_coalesces_per_game_and_caps_at_two` (pure test on the
  pool with sleeps); commands compile-level coverage rides E2E (Task 20).
  api.ts additions covered by `npm run check`.
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green (workspace) + clippy/fmt/hygiene + npm run check.**
- [ ] **Step 5: Commit** `rewrite: cloud service, IPC commands, auto sync triggers (D5)`

### Task 18: `pcgw.rs` — PCGamingWiki save locations

**Files:**
- Create: `rewrite/crates/grid-core/src/pcgw.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs`,
  `rewrite/app/src-tauri/src/commands.rs` (wire the fetch behind the
  Task 17 `native_save_paths` command with an in-memory per-title cache)

**Interfaces (Produces):**
```rust
pub fn parse_windows_save_paths(wikitext: &str) -> Vec<String>;   // pcgamingwiki.py:96 — the Game data/saves template extraction; port _extract_template_block (:76), _split_template_args (:153), _expand_pcgw_path/_expand_pcgw_path_var (:52,:56) exactly
pub async fn fetch_windows_save_paths(http: &reqwest::Client, title: &str) -> Result<Vec<String>, String>; // :264 — page-id lookup (:218, including the URL-title extraction fallback :208), wikitext fetch (:250), parse. Plain reqwest, NOT RommClient (different host, no auth header — never send the RomM token to PCGW).
```

**Pinned rules:** read pcgamingwiki.py top to bottom; port the template
scanner (brace balancing), the `|`-split that respects nested braces and
`[[...]]` links, the PCGW path-variable table
(`{{p|appdata}}` → `%APPDATA%` etc — the `_expand_pcgw_path_var` mapping,
every key), rows that expand to `None` dropped, results deduped in order.
Cache: the command layer holds `title → Vec<String>` for the process
lifetime plus the persisted manual list from config
(`native_manual_save_paths`, merged via `native_save_paths()` from
Task 10). Fetch failures degrade to an empty list (panel still allows
manual paths) — never an error dialog.

- [ ] **Step 1: failing tests** — pure parsing only (no network):
  - `parse_extracts_paths_from_a_realistic_game_data_saves_block` (fixture wikitext with nested templates and a `[[link|label]]`)
  - `parse_expands_the_path_variable_table` (one assert per mapping entry)
  - `parse_drops_unexpandable_rows_and_dedupes`
  - `template_block_extraction_balances_braces`
  - fetch fns covered by a wiremock test hitting the two-step page-id →
    wikitext flow with canned JSON.
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: Commit** `rewrite: PCGamingWiki save-location fetch and parsing`

### Task 19: Details cloud panel UI

**Files:**
- Modify: `rewrite/app/src/lib/Details.svelte`, `api.ts`
- Create: `rewrite/app/src/lib/details/cloud.ts` (+ vitest file per app convention)

**Behavior (doc 06 "Manual actions" + spec "App layer"):**
- Two buttons in the Details action row when
  `details_cloud_mode_supported`: label `Manage Saves` normally,
  `Emulator Saves` when the save scope is shared (cloud_mixin.py:246);
  `Manage States` for states. Selecting the active mode again returns to
  overview.
- Panel: header, Upload button (disabled with a tooltip carrying the block
  reason when blocked), loading state, record rows — title (`file_name`
  else `Cloud Save|State #<id>`), summary `emulator • size [• Slot <slot>]`,
  absolute + relative time — each with Restore / Delete.
- Restore/Delete confirmations; shared scope appends the `Warning:`
  paragraph (port the exact Python copy from
  details_view_mixin.py:1247/1276). Per-row restore disabled reasons
  (`RPCS3 savestate restore…`, `Configure emulator '<n>'…`).
- Native (windows-platform) games: the native panel variant — save-path
  list (PCGW + manual), Browse-to-add / remove for manual paths, records
  below the path section, states unsupported message.
- Block-reason display includes the xemu migration guidance verbatim
  (it arrives from the backend — no frontend copy).
- Stale-response guard: a monotonically increasing request id per panel
  fetch; discard stale results (parity with details_view_mixin.py:797).
- `cloud.ts` holds the pure bits (row model building, mode toggling,
  request-id guard) for vitest; Svelte stays thin.
- Settings: the cloud settings block (four toggles/fields + retention
  limit input) — put it where the existing settings UI lives
  (`Connect.svelte`'s settings area — follow the existing pattern).

- [ ] **Step 1: failing vitest** for `cloud.ts`: row model (title
  fallback, summary composition, slot suffix), mode toggle
  (same-mode→overview), stale-request guard.
- [ ] **Step 2: red.** — [ ] **Step 3: implement panel + settings.**
- [ ] **Step 4: green** — `npm test`, `npm run check`, `npm run build`, workspace cargo suite untouched-green.
- [ ] **Step 5: Commit** `rewrite: details cloud panel and cloud settings UI`

### Task 20: E2E, porting-doc deviations, milestone exit

**Files:**
- Create: `rewrite/e2e/specs/cloud-saves.spec.ts`
- Modify: the E2E mock RomM server (save/state endpoints), `rewrite/README.md`
  (E2E table row), `docs/porting/06-cloud-saves.md`

**E2E group `cloud-saves`** (follow the existing spec structure/harness):
- Mock RomM serves: `GET /api/saves?rom_id` (canned records),
  `POST /api/saves` (captures multipart), `POST /api/saves/delete`,
  content downloads. Fake emulator = the existing E2E stub script writing
  a save file into its save dir on "run".
- Scenarios: (1) manual upload from the panel → mock received one POST
  with `overwrite=true` and the file; (2) launch with
  auto-download-on-launch → save restored to disk before the stub runs;
  (3) exit → auto upload fires after the delay (override the delay to 0
  via settings); (4) retention: seed 4 records, upload, assert one delete
  POST; (5) block reason renders for a native-platform game's state mode.
  xemu E2E is NOT run here (no xemu binary in CI) — the Task 16 wiremock
  integration test is its coverage; note that in the spec file header.

**doc 06 update** — append a `## Rust port deviations (milestone 6)`
section listing D1–D9 with one paragraph each (matching the spec), the
follow-the-code quirk rulings (including the `relative_timestamp_text`
bucket bug ported as-is), the D8 partition note, the CloudGame
`title_id`/`base_title_id`/`ps3_game_id` data-availability gap, and the
TV-variant deferral. Update the two `OPEN QUESTION` markers that are now
ruled (retention → D7, `_authorized_headers` → D4) in place with a
pointer to the section.

**Milestone exit:**
- [ ] **Step 1:** E2E mock endpoints + spec; run `scripts/e2e.sh` — full
  suite green (all groups, not just the new one).
- [ ] **Step 2:** doc 06 deviations section; README E2E row.
- [ ] **Step 3:** full local gate from `rewrite/`: `cargo test --workspace`,
  clippy, fmt, hygiene, `npm test` + `npm run check` + `npm run build`,
  `scripts/e2e.sh`.
- [ ] **Step 4:** Commit `rewrite: cloud-saves E2E group + doc 06 deviations`
- [ ] **Step 5:** `cargo clean --profile dev` from `rewrite/` (keep
  release) — the standing milestone-exit rule.

---

## Task order and model notes for the executor

Dependency order is the task order; Tasks 13-14 (fatx) are independent of
2-12 and may slot anywhere before 15. Suggested models: haiku/sonnet for
Tasks 1-12 and 18 (rich anchors, mechanical porting; use sonnet where a
task spans many rules — 5, 8, 12, 16 are sonnet-or-better), opus for
Tasks 13, 14, 16 (design judgment / integration), sonnet for 15, 17, 19,
20. Final whole-branch review on the most capable model per SDD.

## Self-review notes (already applied)

- Type consistency: `IgnoreSets` + `latest_mtime_under` defined in Task 3
  (cloud/mod.rs) because window.rs needs them before candidates.rs exists;
  Task 5 only builds them. `Window` defined in Task 3, consumed by 8/12/16.
  `BuiltJobs`/`UploadJob` defined in Task 8, consumed by 16. `CloudGame`/
  `SaveType` in Task 2. `CoreFlags` comes from `autoconfig::cores` (M5).
- Spec coverage: engine (T2-12, 16), auto triggers (T17), desktop UI
  (T19), native saves (T10, 18, plus ops branches in T16), xemu raw-disk
  (T13-15), D1-D9 all placed, M5 cleanup (T1), doc/exit (T20).
- The xemu candidate-discovery interaction is pinned in Task 16: xemu
  contributes no generic file candidates; its upload/restore branch goes
  through xemu_sync. The Python xemu save-path override block
  (cloud_mixin.py:906) still ports in Task 12 for directory RESOLUTION
  parity (the panel's "directories found" gate), but targets from it are
  unused by the xemu branch — doc 06 deviation text covers this under D1.
- Placeholder scan: no TBDs; every "read the Python" pointer names an
  exact file:line anchor and is accompanied by pinned behavior, not a
  blank delegation.
