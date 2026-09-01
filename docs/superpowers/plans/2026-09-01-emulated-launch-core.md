# Emulated Launch Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Launch installed games through user-configured emulators — emulator management, profile auto-fill, command construction, spawn, session tracking, Play/Stop UI. Milestone 3 of the Rust rewrite.

**Architecture:** New `launch` module in grid-core (profiles, selection, template, rom, spawn, sessions + a `LaunchService`), ten new Tauri commands + a `sessions-changed` event, an Emulators settings panel, and Play/Stop in the details overlay.

**Tech Stack:** Existing workspace. New deps: `libc` (unix SIGTERM only). No other crates.

**Spec:** `docs/superpowers/specs/2026-09-01-emulated-launch-core-design.md` — binding. Doc 04 (`docs/porting/04-emulator-launch.md`) is the behavior contract; the keyword matcher's reference is `grid_launcher/emulator/profiles.py:67-130`.

**Branch:** `rust-launch`, from `main`.

## Global Constraints

- The seven launch validation messages verbatim, validated in the reference order: emulator name → entry → path text → path exists → ROM text → ROM exists → argument parse (spec §Spawn).
- Paths `~`-expanded at use time, never canonicalized (except the RetroArch `-L` post-pass, which resolves).
- grid-core never imports Tauri; errors cross IPC as Display strings; no secrets anywhere near spawn env, logs, or errors; `rewrite/scripts/check_secret_hygiene.sh` green after every task.
- TDD per task; suites (`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `npm test`, `npx svelte-check`, hygiene) green at every commit; Python suite untouched.
- Commit each task with trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Config — emulator entries and launch defaults

**Files:** Modify `rewrite/crates/grid-core/src/config.rs`

**Interfaces — Produces:**
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmulatorEntry {
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub args: String,
}
// On Config:
#[serde(default)] pub emulators: Vec<EmulatorEntry>,
#[serde(default)] pub default_emulators: BTreeMap<String, String>,
#[serde(default)] pub retroarch_cores: BTreeMap<String, String>,
#[serde(default)] pub launch_args: String,
```
`Config::load` filters `emulators` after parse: entries whose `name.trim()` is empty are dropped. `Default` gains the four empty fields. Values stored as typed; no expansion at save time.

**Steps:**
- [ ] Failing tests: emulator array round-trips (name/path/args); blank-name entry dropped on load; defaults maps round-trip; missing keys default; `preserves_unknown_keys` still green (flatten must not swallow the new named fields).
- [ ] RED → implement → GREEN (`cargo test -p grid-core config`). fmt + hygiene.
- [ ] Commit `rewrite: config gains emulators and launch defaults`.

---

### Task 2: Autoprofiles (`launch/profiles.rs`)

**Files:** Create `rewrite/crates/grid-core/src/launch/mod.rs` (module decls + `LaunchError` skeleton), `rewrite/crates/grid-core/src/launch/profiles.rs`; modify `src/lib.rs` (`pub mod launch;`)

**Interfaces — Produces:**
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EmulatorProfile {
    pub name: String,
    pub match_tokens: Vec<String>,   // casefolded at load
    pub args: String,                // default "%rom%"
    pub all_platforms: bool,
    pub platform_keywords: Vec<String>,
    pub is_compat_tool: bool,
}
pub fn load_profiles() -> &'static [EmulatorProfile];   // OnceLock over include_str!
pub fn profile_for_entry<'a>(entry_name: &str, exe_path: &str, profiles: &'a [EmulatorProfile]) -> Option<&'a EmulatorProfile>;
pub fn platform_matches_keywords(platform: &str, keywords: &[String]) -> bool;
pub fn visible_profiles(profiles: &[EmulatorProfile]) -> Vec<&EmulatorProfile>; // drops compat tools always; drops WINDOWS_ONLY_SLUGS on non-windows
pub const WINDOWS_ONLY_SLUGS: [&str; 3] = ["xenia canary (xbox 360)", "xenia (xbox 360)", "shadps4 qt launcher"];
```
- Embed: `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../emulator-autoprofiles.json"))` (crate at rewrite/crates/grid-core; repo root three levels up). Normalization: drop entries with blank name; drop entries with neither match_tokens nor is_compat_tool; blank args → `"%rom%"`; tokens casefolded (`to_lowercase`).
- `profile_for_entry` order (doc 04 §3): (1) entry name casefold == profile name casefold; (2) exe **basename** (casefolded; parse with a Windows-tolerant split on both `/` and `\`) matches any token — glob semantics when the token contains `*`/`?` (implement a small glob for `*`/`?` only, case-sensitive over already-casefolded strings; no new dep), else exact; (3) exe **stem** == token stem. Compat-tool profiles are skipped in all three stages.
- `platform_matches_keywords` — exact port of profiles.py:67-130 restricted to one platform. Token set of a string: for each `[A-Za-z0-9]+` run: add casefolded run; split at letter↔digit and lower→upper boundaries, add each casefolded part; add concatenation of the alphabetic parts. A keyword matches when: its token set is non-empty and ⊆ platform's; NOT (extra tokens contain a digit-only token while keyword has none); NOT (some extra alphabetic token occurs in the casefolded platform at index ≥ the max end-index of any keyword alpha-token occurrence). Any keyword matching ⇒ true.

**Steps:**
- [ ] Failing tests: embedded JSON parses and is non-empty with ≥1 compat tool filtered out; normalization drops; matching order (name beats token; glob token `retroarch*`; stem match `pcsx2-qt` vs token `pcsx2-qt.exe`… use real values from the JSON where possible); keyword table: keyword "snes" matches "Super Nintendo Entertainment System"-style tokens? (build table from the algorithm: e.g. "playstation 2" vs platform "PlayStation 2" true, vs "PlayStation" false [keyword has extra token], vs "PlayStation 3" false [numeric guard]; "gamecube" vs "Nintendo GameCube" true [positional guard: extra "nintendo" occurs before]; "nintendo 64" vs "Nintendo 64DD"-like false via numeric/positional as derived); camelCase splitting ("GameCube" → gamecube/game/cube).
- [ ] RED → implement → GREEN. fmt + clippy + hygiene.
- [ ] Commit `rewrite: emulator autoprofiles + keyword matcher`.

---

### Task 3: Selection (`launch/selection.rs`)

**Files:** Create `rewrite/crates/grid-core/src/launch/selection.rs`

**Interfaces — Consumes:** Task 1 config fields, Task 2 `profile_for_entry`/`platform_matches_keywords`. **Produces:**
```rust
pub fn emulator_supports_platform(entry: &EmulatorEntry, platform: &str, profiles: &[EmulatorProfile], retroarch_cores: &BTreeMap<String, String>) -> bool;
pub fn mapping_value_for_platform<'a>(map: &'a BTreeMap<String, String>, platform: &str) -> Option<&'a str>; // exact key, then case-insensitive scan; blank ignored
pub fn emulator_entry_by_name<'a>(emulators: &'a [EmulatorEntry], name: &str) -> Option<&'a EmulatorEntry>; // None for blank query; casefold match
pub fn compatible_emulator_names_for_platform(...) -> Vec<String>; // config order, blank names skipped, supports filter
pub fn default_emulator_name_for_platform(...) -> String; // doc 04 §2 order, "" when none
```
`emulator_supports_platform`: blank platform → true; profile all_platforms → true; entry/profile name contains "retroarch" (casefold) → non-blank `mapping_value_for_platform(retroarch_cores, platform)`; no profile → true; else keyword matcher.

**Steps:**
- [ ] Failing tests: mapping precedence + case-insensitive + blank-ignored; by-name blank query; default→configured-but-unsupported falls through to first compatible; retroarch gate on/off via cores map; no-profile → true; "" when nothing.
- [ ] RED → implement → GREEN. Commit `rewrite: emulator selection`.

---

### Task 4: ROM resolution (`launch/rom.rs`)

**Files:** Create `rewrite/crates/grid-core/src/launch/rom.rs`

**Interfaces — Consumes:** `library::registry::InstalledGame`, `library::paths::{candidate_archives, candidate_extracted_dirs, archive_name}`, `library::launch_select::select_launch_file`, `library::extract::is_arcade_platform`. **Produces:**
```rust
pub fn resolve_rom_path(game: &InstalledGame, library: &Path) -> String;
```
Doc 04 §6: unless arcade — extracted candidates first: for each candidate extracted dir that exists, `select_launch_file(dir, archive_stem)`; then `extracted_path` when it exists as a file. Then (or for arcade) the first archive candidate existing as a file. Else raw trimmed `archive_path`. (Archive name for candidates: `archive_name(&game.rom_file_name, &game.title, &game.platform)`.) Return the path as a String (`~` expansion left to spawn).

**Steps:**
- [ ] Failing tests (tempdir fixtures): extracted-first for normal platform; arcade returns archive even when extracted exists; multi-file row resolves extracted_path (the .m3u); raw fallback when nothing exists.
- [ ] RED → implement → GREEN. Commit `rewrite: launch ROM resolution`.

---

### Task 5: Template construction (`launch/template.rs`)

**Files:** Create `rewrite/crates/grid-core/src/launch/template.rs`

**Interfaces — Produces:**
```rust
pub struct Placeholders { pub rom: String, pub core: String, pub ps3_launch_target: String }
pub fn retroarch_core_argument_path(value: &str, os: &str) -> String; // os: "windows"|"macos"|"linux" (host default fn provided)
pub fn split_template(template: &str) -> Result<Vec<String>, String>; // "" -> []; POSIX-style split, fallback splitter on unbalanced quotes
pub fn validate_placeholders(template: &str, ph: &Placeholders) -> Result<(), String>; // the two verbatim messages
pub fn apply_placeholders(tokens: Vec<String>, ph: &Placeholders) -> Vec<String>;
pub fn normalized_retroarch_core_args(emulator_dir: &Path, args: Vec<String>) -> Vec<String>;
pub fn build_args(entry_args: &str, global_launch_args: &str, ph: &Placeholders) -> Result<Vec<String>, String>;
```
Exact rules (spec §Template):
- Core path: blank → ""; `\`→`/`; contains `/` → as-is; else ext by os (`.dll`/`.dylib`/`.so`), strip one trailing `.dll|.dylib|.so` case-insensitive (first match wins in that order), append `_libretro` unless suffix already, return `cores/<base>_libretro<ext>`.
- Split: implement POSIX-ish splitting (whitespace-separated, `"`/`'` quoting, backslash escapes outside single quotes); on unbalanced quote, FALLBACK: scan for quoted `"..."`/`'...'` chunks and bare words without escape processing (mirrors shlex posix=False closely enough; pin behavior in tests: `-fullscreen "unclosed` → `["-fullscreen", "\"unclosed"]`-style tokens are acceptable ONLY if tests document the exact chosen output — the plan pins: fallback splits on whitespace but keeps quoted spans together and preserves quote chars, which `strip_wrapping_quotes` later removes).
- Validation messages verbatim: `No RetroArch core is configured for this platform. Set one in Emulators > Defaults.` and `No PS3 ISO or game ID was found for this game.`
- Apply: remember per-token `%core%` presence; replace `%rom%`, `%core%`, `%ps3_launch_target%` (plain replace, dictionary order rom→core→target); strip one wrapping `"` or `'` pair when len ≥ 2 after trim; if token had `%core%` and core blank → pop preceding `-L|--libretro|--core` and skip; drop empties.
- `build_args` = template join (entry_args blank → `%rom%`; append global, blank parts dropped, single space) → split → validate → apply.
- Post-pass: for each element equal to `-L|--libretro|--core` except last index, next token non-blank AND relative AND `emulator_dir/token` is a file → replace with resolved absolute.

**Steps:**
- [ ] Failing tests — the full table: core-path derivation (3 os exts, `/` passthrough, `_libretro` idempotence, `.DLL` strip case-insensitive); split (quotes, blank → [], fallback on unbalanced with pinned output); both validation messages verbatim; substitution + quote stripping + empty-drop + `-L` pop; post-pass (relative rewritten absolute, absolute untouched, last-position ignored, missing file untouched); `build_args` end-to-end with `-L "%core%" "%rom%"` and a real-looking core.
- [ ] RED → implement → GREEN. Commit `rewrite: launch argument templates`.

---

### Task 6: Spawn + sessions + LaunchService (`launch/spawn.rs`, `launch/sessions.rs`, `launch/mod.rs`)

**Files:** Create `spawn.rs`, `sessions.rs`; fill `mod.rs`. Test: `rewrite/crates/grid-core/tests/launch_service.rs`

**Interfaces — Produces:**
```rust
// mod.rs
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("{0}")] Validation(String),
    #[error("This game is already running.")] AlreadyRunning,
    #[error("Game is not installed.")] NotInstalled,
    #[error("registry: {0}")] Registry(String),
    #[error(transparent)] Config(#[from] crate::config::ConfigError),
    #[error("failed to launch game: {0}")] Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GameSession { pub id: u64, pub rom_id: i64, pub title: String, pub emulator_name: String, pub started_at: i64, pub pid: u32 }
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionsSnapshot { pub sessions: Vec<GameSession>, pub warning: Option<String> }

pub struct LaunchService { /* Mutex<Vec<(GameSession, Child)>>, next_id, notify RwLock<Option<Arc<dyn Fn(SessionsSnapshot)+Send+Sync>>> */ }
impl LaunchService {
    pub fn new(registry: Arc<Registry>, config_path: PathBuf) -> Arc<Self>;
    pub fn set_notify(&self, f: ...);
    pub fn snapshot(&self) -> SessionsSnapshot;               // warning: None
    pub async fn launch(self: &Arc<Self>, rom_id: i64) -> Result<GameSession, LaunchError>;
    pub fn stop(&self, session_id: u64);
    pub fn spawn_poll_loop(self: &Arc<Self>);                 // 2500ms tokio interval; idempotent (once)
}
// spawn.rs
/// Returns (argv incl. exe, cwd). Applies the seven verbatim validation
/// errors in the reference order. `is_retroarch` decides the -L post-pass.
pub fn prepare_emulator_launch(
    emulator_name: &str,               // resolved default; "" triggers the first error
    entry: Option<&EmulatorEntry>,
    rom_path: &str,
    placeholders: &Placeholders,
    global_launch_args: &str,
    is_retroarch: bool,
) -> Result<(Vec<String>, PathBuf), String>;
pub fn clean_env() -> HashMap<String, String>; // parent env; LD_LIBRARY_PATH_ORIG -> LD_LIBRARY_PATH copy
```
- `launch` flow: registry find by rom_id (miss → NotInstalled); platform casefold starts_with("windows") → `Validation("Native Windows games are not supported yet in the Rust preview.")`; live session with same rom_id → AlreadyRunning; load Config; resolve emulator name via selection (blank → the verbatim "No emulator is configured…" Validation); build placeholders (rom via Task 4 with `~`-expanded library root; core via retroarch gate + `retroarch_core_argument_path`; ps3 target ""); `prepare_emulator_launch` (validation chain; RetroArch entries get the post-pass with emulator_dir = exe parent); spawn under `spawn_blocking`: `Command::new(exe).args(rest).current_dir(cwd).env_clear().envs(clean_env())`; unix nothing extra, windows `CREATE_NEW_PROCESS_GROUP` via `creation_flags`. Register session (unix seconds, child pid), notify, schedule the 500 ms early-exit check (tokio sleep → try_wait; exited ⇒ remove session + notify with `warning: Some(format!("Game exited immediately (code {code}): {joined_command}"))` — pin this exact format in a test).
- Poll loop: every 2500 ms `try_wait` each child; remove exited; notify only when something changed. `stop`: unix `libc::kill(pid, SIGTERM)`, windows `child.kill()`; swallow errors; poll observes exit.
- `~` expansion for emulator path and rom path via the existing paths helper (make it `pub(crate)` if needed).

**Steps:**
- [ ] Failing integration tests (stub emulators are shell scripts written by the test into tempdir, `chmod +x`; e.g. `#!/bin/sh\nsleep 30` and `#!/bin/sh\nexit 3`): launch long-runner → session listed with pid, argv received (stub writes its args to a file; assert `%rom%` path arrived); stop → session gone within poll budget; instant-exit → removed + warning contains "code 3" and the joined command; duplicate rom → AlreadyRunning; missing exe file → verbatim "Emulator executable not found:\n<path>"; missing ROM → verbatim message; not-installed rom → NotInstalled; windows-platform row → the native-unsupported message; RetroArch `-L` relative core rewritten (create `cores/x_libretro.so` under stub dir, entry named "RetroArch", args `-L "%core%" "%rom%"`, cores map set).
- [ ] Unit tests in spawn.rs: clean_env copies ORIG into LD_LIBRARY_PATH; without ORIG, env passthrough.
- [ ] RED → implement → GREEN. `cargo add libc --target 'cfg(unix)'`-style (unix-only dep). fmt + clippy + hygiene.
- [ ] Commit `rewrite: LaunchService — spawn, sessions, stop` (multiple commits fine).

---

### Task 7: Tauri commands + event

**Files:** Modify `rewrite/app/src-tauri/src/commands.rs`, `src/lib.rs`

**Interfaces — Produces** (names verbatim; frontend depends on them): `launch_game(rom_id) -> GameSession`, `stop_game(session_id)`, `list_sessions() -> SessionsSnapshot`, `list_emulators() -> Vec<EmulatorEntry>`, `save_emulator(original_name, entry)`, `delete_emulator(name)`, `list_profiles() -> Vec<ProfileSummary{name,args}>`, `match_profile(executable_path) -> Option<ProfileSummary>`, `get_launch_defaults() -> LaunchDefaults{default_emulators, retroarch_cores, launch_args}`, `set_default_emulator(platform, name)`.
- AppState gains `pub launch: Result<Arc<LaunchService>, String>` (built beside install; shares the same `Arc<Registry>` — refactor lib.rs so the registry Arc is created once and cloned into both services; on registry error both are Err).
- `.setup`: launch notify emits `sessions-changed` with the snapshot; call `spawn_poll_loop` once.
- `save_emulator`: load config; validate entry name non-blank (`"Emulator name is required."`); if original_name non-blank remove the case-insensitive match for it, else reject a duplicate name (`"An emulator named '<name>' already exists."`); push; save. `delete_emulator`: remove by case-insensitive name; also drop `default_emulators` values equal to it (case-insensitive). `set_default_emulator`: blank name removes the platform key (exact key, else case-insensitive match). All config mutations in spawn_blocking, same JoinError-string pattern used by set_library_path.
- `list_profiles` = `visible_profiles(load_profiles())` mapped to summaries; `match_profile` = `profile_for_entry("", path, …)`.

**Steps:**
- [ ] Implement; `generate_handler!` grows by ten. Unit test any pure helper (the save/delete merge logic — extract to a testable function in grid-core config or a commands helper with tests).
- [ ] `cargo test --workspace` + clippy + fmt + hygiene green.
- [ ] Commit `rewrite: launch/emulator Tauri commands + sessions-changed event`.

---

### Task 8: Frontend API + sessions store

**Files:** Modify `rewrite/app/src/lib/api.ts`; create `rewrite/app/src/lib/stores/sessions.svelte.ts`, `rewrite/app/src/lib/stores/sessions.test.ts`

**Interfaces:**
```ts
export type GameSession = { id: number; rom_id: number; title: string; emulator_name: string; started_at: number; pid: number };
export type SessionsSnapshot = { sessions: GameSession[]; warning: string | null };
export type EmulatorEntry = { name: string; path: string; args: string };
export type ProfileSummary = { name: string; args: string };
export type LaunchDefaults = { default_emulators: Record<string, string>; retroarch_cores: Record<string, string>; launch_args: string };
// wrappers: launchGame, stopGame, listSessions, listEmulators, saveEmulator(originalName, entry),
// deleteEmulator, listProfiles, matchProfile, getLaunchDefaults, setDefaultEmulator
```
Store: `sessions` `$state` snapshot; `init()` = `listSessions` + subscribe `sessions-changed`; `sessionFor(romId)`; `lastWarning` captured from event payloads (cleared on read/dismiss — pin the chosen semantic in tests); pure helpers extracted for vitest (e.g. merge/warning handling).

**Steps:**
- [ ] Failing vitest for the pure store helpers (snapshot replace, sessionFor hit/miss, warning captured then cleared).
- [ ] RED → implement → GREEN; svelte-check clean; hygiene.
- [ ] Commit `rewrite: sessions store + launch API wrappers`.

---

### Task 9: Emulators settings panel

**Files:** Create `rewrite/app/src/lib/Emulators.svelte`; modify `rewrite/app/src/lib/Downloads.svelte` (footer gains an "Emulators" button next to the aggregate text), `rewrite/app/src/App.svelte` (mount the panel overlay)

**Design:** modal panel (same backdrop/panel pattern as Details.svelte): list of emulator entries (name, path, args preview) with Edit/Delete (inline confirm) per row; an Add/Edit form (name, path — plain text input, args) where blurring/entering a path with empty name+args calls `matchProfile` and fills both when it returns a profile; Save calls `saveEmulator(originalName, entry)` and refreshes; errors inline. Below: a Defaults section — for each platform from the already-loaded platforms list (pass them in from Library or refetch `listPlatforms`), a `<select>` of all emulator names plus "(none)" wired to `setDefaultEmulator`. Uses CSS vars, scoped styles, Escape/backdrop close. No new tests (presentational; matcher/commands tested elsewhere) — svelte-check gates.

**Steps:**
- [ ] Implement + wire footer button and App mount.
- [ ] npm test + svelte-check green; hygiene.
- [ ] Commit `rewrite: emulators settings panel`.

---

### Task 10: Details Play/Stop

**Files:** Modify `rewrite/app/src/lib/Details.svelte`

**Design:** for an installed game the primary action becomes Play (`launchGame(game.id)`; pending state "Launching…"); when `sessionFor(game.id)` is live show a "Playing" chip and a Stop button (`stopGame(session.id)`) instead of Play; Uninstall becomes a secondary (smaller, bordered) button below, keeping its two-click confirm; launch errors (incl. the verbatim backend messages) render in the existing error slot; a session warning from the store (early-exit) shows in the same slot. Keep existing install/uninstall flows intact.

**Steps:**
- [ ] Implement; run existing vitest (46) + svelte-check; hygiene.
- [ ] Commit `rewrite: Play/Stop in details overlay`.

---

### Task 11: Docs + full verification

**Files:** Modify `rewrite/README.md`, `docs/porting/04-emulator-launch.md`

**Steps:**
- [ ] README: update test counts (measure), add a "Manual test checklist — milestone 3" section with the spec's 9 steps verbatim, note the `libc` dep and the embedded autoprofiles file.
- [ ] doc 04: append `## Rust port deviations (milestone 3)` with the spec's seven deviations, one line each, citing the spec path.
- [ ] Run everything: cargo test/clippy/fmt-check, npm test, svelte-check, hygiene, `python -m unittest discover tests/`. Record results.
- [ ] Commit `rewrite: milestone 3 docs + porting-doc deviation notes`.
