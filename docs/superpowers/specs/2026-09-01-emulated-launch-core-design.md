# Rust rewrite milestone 3 — emulated launch core (design)

**Status:** approved design, pre-implementation
**Behavior contract:** `docs/porting/04-emulator-launch.md` (cited as "doc 04");
the keyword matcher additionally cites `grid_launcher/emulator/profiles.py:67-130`.
**Builds on:** milestones 1–2
(`docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`)

## Goal

The Rust app can launch an installed game through a user-configured emulator:
manage emulator entries, auto-fill them from the shipped autoprofiles, pick a
default per platform, build the argument vector exactly as the reference does,
spawn with a cleaned environment, track the running session with exit
detection, and stop it — with Play/Stop wired into the details overlay.

## Scope

In scope:

- Config: `emulators` array (name/path/args), `default_emulators` map,
  `retroarch_cores` map, global `launch_args` string.
- Autoprofile loading (the repo-root `emulator-autoprofiles.json`, embedded at
  compile time), profile matching (doc 04 §3), platform-support test (§2 + the
  keyword matcher), platform gating of profiles (§4, subset).
- ROM resolution (§6), placeholder table + template construction (§5, §7)
  with the reference's exact validation error messages.
- Spawn (§8 emulated branch): cwd = emulator dir, cleaned env
  (`LD_LIBRARY_PATH_ORIG` rule), `expanduser` but never `resolve` on paths.
- Session tracking: session list, 2.5 s exit polling, 500 ms early-exit
  warning, Stop (SIGTERM on unix, kill on Windows, no escalation).
- IPC: launch/stop/list-sessions + emulator CRUD + defaults; `sessions-changed`
  event.
- UI: Emulators settings panel (list, add/edit with profile auto-fill,
  per-platform default picker) reachable from the footer; Play/Stop and a
  "Playing" badge in the details overlay; launch errors inline.

Out of scope (later milestones): native Windows games and compat tools (§9,
§10), emulator/compat-tool acquisition (§12), firmware (§11), doc 05 config
writers (`_ensure_emulator_sync_settings` is NOT called before spawn this
milestone), cloud pre-launch/post-exit hooks, RPCS3 custom-config copy,
pause/resume, RetroArch core management UI, `GRID_LAUNCHER_SHARE_DIR`
profile override.

## Global constraints

- Milestone 1 secret rules unchanged and normative (keyring + SecretString,
  two `expose_secret` sites, hygiene script green). Nothing in this milestone
  touches credentials; spawned processes never receive them.
- grid-core never imports Tauri types. Errors cross IPC as `Display` strings.
- The reference's launch error messages are ported verbatim (doc 04 §8 lists
  them; invariants section: validation order emulator name → entry → path
  text → path exists → ROM text → ROM exists → argument parse).
- Existing suites stay green; Python suite untouched.

## Architecture

New `launch` module in grid-core:

```
rewrite/crates/grid-core/src/launch/
  mod.rs        LaunchService + LaunchError + re-exports
  profiles.rs   autoprofile deserialization/normalization + matching +
                the keyword matcher + platform gating
  selection.rs  default lookup, compatible fallback, supports-platform test
  template.rs   placeholders, shell splitting, validation, quote stripping,
                RetroArch -L path normalization (pure)
  rom.rs        ROM path resolution over InstalledGame (pure)
  spawn.rs      command assembly + clean env + std::process spawn (blocking)
  sessions.rs   session list + poll loop + stop
```

### Config additions (`Config`, typed fields, all `#[serde(default)]`)

```rust
pub struct EmulatorEntry {
    pub name: String,           // entry dropped at load if blank after trim
    pub path: String,           // "~" expanded at use time only
    pub args: String,           // blank collapses to "%rom%" at use time
}
pub emulators: Vec<EmulatorEntry>,
pub default_emulators: BTreeMap<String, String>,   // platform name -> emulator name
pub retroarch_cores: BTreeMap<String, String>,     // platform name -> core value
pub launch_args: String,                            // appended to every template
```

Normalization (on load): entries with blank `name` are dropped; names are
matched case-insensitively everywhere; `default_emulators` lookup is exact key
first, then case-insensitive scan, blank values ignored (doc 04 §2 step 1).

### Autoprofiles

`emulator-autoprofiles.json` at the repository root stays the single source of
truth: grid-core embeds it with `include_str!` (relative path from the crate)
and parses at first use. Parsed subset per profile: `name` (drop if blank),
`match_tokens` (casefolded; entries may contain `*`/`?` globs),
`args` (default `"%rom%"`), `all_platforms`, `platform_keywords`,
`is_compat_tool` (profiles with `is_compat_tool == true` are excluded from
emulator matching this milestone; profiles with neither match_tokens nor
is_compat_tool are dropped). Unknown fields ignored. A parse failure of the
embedded JSON is a compile-visible test failure, not a runtime branch.

## Behavior

### Profile matching (doc 04 §3, ported)

`profile_for_entry(entry, profiles)` — first match in file order by:
1. entry name == profile name (casefold);
2. executable basename matches any token (glob when the token contains
   `*`/`?`, else exact, both casefolded);
3. executable stem == token stem.
Used by the UI to auto-fill `args` when the user picks an executable, and by
the supports-platform test.

### Platform support (doc 04 §2 + profiles.py:67-130, ported)

`emulator_supports_platform(entry, platform, profiles, retroarch_cores)`:
1. blank platform → true;
2. profile `all_platforms` → true;
3. entry or profile name contains "retroarch" (casefold) → true iff
   `retroarch_cores[platform]` (case-insensitive key lookup) is non-blank —
   milestone simplification of the reference's installed-core scan, recorded
   as deviation 4;
4. no profile matched → true;
5. else the keyword matcher below against the launch platform.

Keyword matcher (`platform_matches_keywords(platform, keywords) -> bool`,
verbatim port of `matching_platforms_for_emulator_keywords` restricted to one
platform): build a token set for a string by extracting `[A-Za-z0-9]+` runs,
casefolding each; additionally splitting each run at letter↔digit and
lower→upper boundaries and adding those parts; additionally adding the
concatenation of the alphabetic parts. A keyword entry matches when its token
set is a non-empty subset of the platform's token set, AND NOT (the extra
platform tokens contain a numeric token while the keyword has none), AND NOT
(some extra alphabetic token occurs in the casefolded platform string at or
after the end of the last keyword-alpha-token occurrence). Any matching
keyword entry ⇒ supported.

### Selection (doc 04 §2, ported)

`default_emulator_name_for_platform`: mapping lookup → entry by
case-insensitive name → keep only if it supports the platform → else first
name from `compatible_emulator_names_for_platform` (config order, blank names
skipped, supports-platform filter) → else `""`.

### Platform gating of profiles (doc 04 §4, subset)

For the settings UI's profile list only: drop profiles whose casefolded name
is in `{"xenia canary (xbox 360)", "xenia (xbox 360)", "shadps4 qt launcher"}`
on non-Windows. (Source-allowlist gating belongs to the acquisition milestone.)

### ROM resolution (doc 04 §6, ported)

Over the registry row: unless the platform is arcade (substring test:
arcade/mame/fbneo/"final burn"), first existing extracted-candidate file
(launch file inside `extracted_dir`, then `extracted_path`); else first
existing archive-candidate file; else the raw trimmed `archive_path`.
Multi-file rows resolve through `extracted_path` (the launch entry, e.g. the
`.m3u`). Reuses milestone 2's `paths.rs` candidate helpers.

### Template construction (doc 04 §5 + §7, ported verbatim)

- Placeholders: `%rom%` = resolved ROM path; `%core%` = RetroArch core
  argument path (`retroarch_core_argument_path` rules: blank → ""; `\`→`/`;
  contains `/` → as-is; else strip a trailing `.dll/.dylib/.so`, append
  `_libretro` unless present, return `cores/<base>_libretro<ext>` with ext
  `.dll` win / `.dylib` mac / `.so` linux); `%ps3_launch_target%` = "" this
  milestone (no PS3 fields in the registry yet — its validation error still
  fires, deviation 3).
- Template = entry args (blank → `%rom%`, trimmed) + " " + global
  `launch_args` (blank parts dropped).
- Tokenize: POSIX shell splitting, retry non-POSIX on error (Rust: a
  `shlex`-style split; on failure fall back to whitespace splitting that
  keeps quoted chunks — the plan pins the exact fallback; blank template →
  `[]`).
- Validate before substitution: template mentions `%core%` with blank core →
  `"No RetroArch core is configured for this platform. Set one in Emulators > Defaults."`;
  mentions `%ps3_launch_target%` with blank target → the reference's "No PS3
  ISO or game ID was found for this game." message.
- Substitute every token in every element (plain replace, no escaping), strip
  one wrapping quote pair (len ≥ 2), drop elements that became empty, and for
  an element that carried `%core%` resolving blank also pop a preceding
  `-L`/`--libretro`/`--core`.
- RetroArch post-pass: for each `-L`/`--libretro`/`--core` (except last
  position), a following non-blank relative token that exists under the
  emulator dir is rewritten absolute (resolved); absolute/missing left alone.

### Spawn (doc 04 §8 emulated branch + invariants)

Validation order and messages, verbatim: blank emulator name →
`"No emulator is configured. Add one in Emulators settings."`; entry missing →
`"Default emulator '<name>' was not found."`; blank path →
`"Emulator '<name>' has no executable path configured."`; path not a file →
`"Emulator executable not found:\n<path>"`; blank ROM →
`"No ROM file is available for this game."`; ROM not a file →
`"ROM file not found:\n<path>"`; template error →
`"Invalid launch arguments: <e>"`.

Command `[emulator_path, *args]`, cwd = emulator path's parent. Environment:
copy the parent env; if `LD_LIBRARY_PATH_ORIG` is present copy its value into
`LD_LIBRARY_PATH`; (the frozen-build deletion branch does not apply — the app
is not a bundled Python). `expanduser` on emulator and ROM paths, never
canonicalize. Spawn via `std::process::Command` inside `spawn_blocking`.
Windows: `CREATE_NEW_PROCESS_GROUP` creation flag; elsewhere nothing.

### Sessions (doc 04 concurrency, adapted)

`LaunchService` (grid-core, owned by AppState):

```rust
pub struct GameSession {           // serde Serialize, snake_case
    pub id: u64,                   // monotonically increasing
    pub rom_id: i64,
    pub title: String,
    pub emulator_name: String,
    pub started_at: i64,           // unix seconds
    pub pid: u32,
}
```

- `launch(registry_row, config, profiles) -> Result<GameSession, LaunchError>`;
  a second launch for a rom_id with a live session is rejected with
  `"This game is already running."` (deviation 1: the reference desktop
  allowed unbounded duplicates).
- Poll loop: a tokio interval of 2500 ms calls `try_wait` on every child;
  exited sessions are removed and `sessions-changed` fires. 500 ms after each
  spawn a one-shot check fires: if the child already exited, the session is
  removed and the event carries a warning string with the exit code and the
  space-joined command (`process_exited_early_message` semantics).
- `stop(session_id)`: SIGTERM on unix (via `libc::kill`), `Child::kill` on
  Windows; errors swallowed; the poll loop observes the exit (no
  wait-with-timeout, no escalation — same as the reference TV path).
- Sessions are tracked for every emulated launch (deviation 2: the reference
  only registered sessions for cloud purposes).
- Event payload: `{ sessions: [GameSession], warning: string | null }` —
  full list, newest first; `warning` set only by the early-exit check.

### Tauri IPC surface

| Command | Args | Returns |
| --- | --- | --- |
| `launch_game` | `rom_id: i64` | `GameSession` |
| `stop_game` | `session_id: u64` | `()` |
| `list_sessions` | — | `Vec<GameSession>` |
| `list_emulators` | — | `Vec<EmulatorEntry>` |
| `save_emulator` | `original_name: String, entry: EmulatorEntry` | `()` (empty original = add; rename = replace by original) |
| `delete_emulator` | `name: String` | `()` |
| `list_profiles` | — | profile summaries `{name, args}` (gated for this OS) |
| `match_profile` | `executable_path: String` | `Option<{name, args}>` |
| `get_launch_defaults` | — | `{default_emulators, retroarch_cores, launch_args}` |
| `set_default_emulator` | `platform: String, name: String` | `()` (blank name removes the key) |

Config-mutating commands load-modify-save the TOML (spawn_blocking), same as
`set_library_path`. Event: `sessions-changed` as above.

### Frontend

- `stores/sessions.svelte.ts`: snapshot + event subscription (pattern of the
  downloads store); derived `sessionFor(romId)`; surfaces the early-exit
  warning.
- `Emulators.svelte`: settings panel opened from a footer "Emulators" button:
  emulator list (name, path, args), add/edit inline form — picking/typing an
  executable path calls `match_profile` and fills name/args when the fields
  are empty; delete with inline confirm; a defaults section listing the
  loaded platforms, each with a select over all emulator names (the select is
  not filtered by the supports-platform test — an unsupported choice surfaces
  at launch; deviation 5).
- `Details.svelte`: installed game gains Play as the primary action
  (uninstall moves to a secondary button); while a session for the rom is
  live show a "Playing" badge and a Stop button; launch errors render inline
  with the exact backend message.
- Gamepad: unchanged mappings; Play is the focused default action in the
  overlay.

## Deliberate deviations from the reference (recorded in doc 04 at merge)

1. Duplicate launches of the same rom are rejected (reference desktop allowed
   them; the TV backend allowed one global session — we allow one per rom).
2. Sessions are tracked for every emulated launch and drive UI state; the
   reference tracked them only for cloud auto-upload.
3. PS3 titles cannot resolve `%ps3_launch_target%` yet (registry lacks PS3
   fields until the PS3 install milestone); the reference's validation error
   is shown.
4. RetroArch platform support = a non-blank `retroarch_cores` config entry,
   not a scan of installed core files.
5. The per-platform default picker lists all emulators rather than filtering
   by the supports-platform test; the test still gates automatic selection.
6. Desktop UI gains a Stop button (reference desktop had none).
7. No `_ensure_emulator_sync_settings` call before spawn (doc 05 deferred).

## Error handling

`LaunchError` (thiserror): `Validation(String)` (all doc-04 message strings),
`Io(#[from] std::io::Error)`, `AlreadyRunning`, `Registry(String)`,
`Config(#[from] ConfigError)`, `NotInstalled`. Display strings credential-free.
`launch_game` on a not-installed rom returns `NotInstalled` ("Game is not
installed.") — the frontend never offers Play for those, but the command
guards anyway. Windows-native platforms (platform casefold starts with
"windows") return `Validation("Native Windows games are not supported yet in
the Rust preview.")` (deviation-3 family; scope note).

## Testing

- `template.rs`: the doc's full rule set as unit tables — placeholder
  substitution, quote stripping, empty-drop, `-L` pop, validation messages
  verbatim, splitter fallback, core-path derivation (all three extensions,
  `/`-containing passthrough, `_libretro` idempotence), RetroArch post-pass
  (relative rewritten, absolute kept, last-position ignored).
- `profiles.rs`: matching order (name > token glob > stem), compat-tool
  exclusion, normalization drops; keyword matcher table incl. the reference's
  tricky cases (numeric-guard: "playstation 2" keyword not matching
  "PlayStation 3"; positional guard; camelCase/digit splitting).
- `selection.rs`: mapping precedence, case-insensitive lookup, support
  filter, fallback order, blank results.
- `rom.rs`: arcade archive-first, extracted-first otherwise, raw fallback.
- `spawn.rs`/`sessions.rs`: integration tests with a shell-script stub
  emulator under tempdir — long-running stub: session appears with pid,
  stop() ends it, poll removes it; instant-exit stub: early-exit warning
  carries code + joined command; duplicate launch rejected; validation errors
  for missing executable/ROM verbatim.
- Config: emulator array round-trip + blank-name drop + defaults maps.
- Frontend: vitest for the sessions store (event merge, sessionFor, warning
  surfacing); svelte-check clean.

## Manual test checklist (milestone exit gate)

1. Open Emulators from the footer; add a real emulator by path; name and args
   auto-fill from its profile; save; relaunch app — entry persists in
   config.toml.
2. Set it as default for a platform with an installed game.
3. Play the game from the details overlay; the emulator starts with the right
   ROM; the overlay shows Playing and the session appears.
4. Quit the emulator normally; within ~2.5 s the badge clears.
5. Play again, press Stop; the emulator terminates and the badge clears.
6. Point the entry at a nonexistent path and Play: the exact
   "Emulator executable not found:" message shows inline.
7. A RetroArch platform with no core mapping shows the exact "No RetroArch
   core is configured…" message; adding `retroarch_cores` in config.toml
   fixes it.
8. Break an entry's args with an unclosed quote and confirm launch still
   proceeds via the fallback splitter (or shows "Invalid launch arguments"
   when truly unparseable).
9. config.toml and grid-launcher.db still contain no secrets.
