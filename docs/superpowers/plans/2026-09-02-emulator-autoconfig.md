# Emulator Autoconfig Implementation Plan (rewrite milestone 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port both autoconfig layers and the save-path readers to grid-core:
entry autoconfig (emulator entry creation/update, platform + core defaults,
Dolphin variants, defaults backfill), the eleven `ensure_*` native-config
writers, every `*_directory_settings` / `*_save_path_overrides` /
`*_state_path_overrides` reader, and the RetroArch core metadata
(core list, slug map, installed-core discovery, fuzzy platform match,
capability flags). Wire the writers to run on new-emulator creation only
(D1) and add RetroAchievements credential storage + a narrow credential
fan-out (D2).

**Architecture:** New grid-core module `src/autoconfig/`. The three write
policies (INI overwrite, add-only, append-if-absent block) live once in
`writers.rs`; per-emulator modules declare only sections, keys, probes and
file locations. `cores.rs` owns the two embedded JSON data files.
`entry.rs` is layer 1 (pure, no I/O beyond installed-core discovery).
`readers.rs` is ported and unit-tested now, consumed by milestone 6.
Orchestration is `autoconfig::sync_new_emulator`, called from
`InstallService::finalize_emulator` and the manual-add IPC path; failures
are non-fatal and ride the existing finalize `warning` string onto the
downloads-drawer row. grid-core never imports Tauri.

**Tech Stack:** Rust (regex, serde_json, secrecy, keyring, tempfile for
tests), Tauri 2 commands, Svelte 5, WebdriverIO.

**Spec:** `docs/superpowers/specs/2026-09-02-emulator-autoconfig-design.md`
(binding authority). Behavior contract: `docs/porting/05-emulator-autoconfig.md`
(doc 05) — where the spec is silent doc 05 wins; where both are silent the
Python source cited by doc 05 wins. Reference code lives under
`grid_launcher/emulator/` plus
`grid_launcher/ui/mixins/emulator_ui_mixin.py` and
`grid_launcher/ui/mixins/cloud_mixin.py`.

## Global Constraints

- **Byte-for-byte strings.** Every key name, section name, value literal,
  file name and error message that exists in the reference is ported
  verbatim. `"Borderless Windowed"`, `"Slate Forest"`, `-18.000000`,
  `True`/`False` (Dolphin, PPSSPP) vs `true`/`false` (PCSX2, DuckStation,
  Azahar, Eden, RPCS3, Xemu) — do not normalize casing, do not "improve"
  anything.
- **grid-core never imports Tauri.** Errors cross the boundary as `Display`
  strings. Orchestration takes plain data (a `&Config`, an
  `&EmulatorEntry`, a `&[String]` of platform names) — never a Tauri
  `State`.
- **Tokens.** The RetroAchievements token is a credential: OS keyring only,
  held in `secrecy::SecretString`, never in `config.toml`, never in logs,
  errors, IPC responses or console output. Its ONLY permitted disk
  destinations are the emulator config files this feature writes
  (`retroarch.cfg` `cheevos_token`, `PCSX2.ini` `[Achievements] Token`,
  `PPSSPP.INI` `[Achievements] AchievementsToken`, and
  `ppsspp_retroachievements.dat`). `expose_secret()` is called in exactly
  ONE new place, `autoconfig/mod.rs`, which is added to the
  `check_secret_hygiene.sh` allowlist in Task 3 — the task that introduces
  the call site, so its own gate passes — nowhere else.
  `get_retroachievements_status` returns a boolean, never the token.
- **Every task ends green**, run from `rewrite/`:
  - `cargo test -p grid-core`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --check`
  - `bash scripts/check_secret_hygiene.sh`
  - `npm run check` + `npm test` in `rewrite/app` when the frontend is touched
  The full `rewrite/scripts/e2e.sh` gates the milestone at Task 13.
- **D1 — trigger policy.** `ensure_*` writers and entry autoconfig run ONLY
  when a NEW emulator entry is created: after
  `InstallService::finalize_emulator` writes the config entry, or on a new
  manual add. Never on edits, never pre-launch, never on view refresh. No
  session cache exists — there is nothing to deduplicate.
- **D2 — RA credential fan-out.** Saving RA credentials triggers a one-shot
  narrow write to every registered entry matching an RA-capable predicate
  (RetroArch, PCSX2, PPSSPP) that touches ONLY the RA credential keys.
  Clearing credentials writes nothing and scrubs nothing (parity).
- **D3 — defaults backfill** runs at the two D1 trigger points only,
  immediately after entry autoconfig.
- **D4 — PCSX2 raw-path bug fixed.** `ensure_pcsx2` uses the expanded,
  trimmed path throughout.
- **D5 — PPSSPP unprotected reads guarded.** An unreadable INI or `.dat`
  yields `changed=false`, never a propagating error.
- **D6 — PCSX2 `[Folders] Bios` omitted** (firmware subsystem deferred).
- **D7 — RPCS3 background firmware download out** (same deferral).
- **D8 — one return type.** Every `ensure_*` returns `EnsureResult`.
- **Idempotency is mandatory.** Every writer test asserts a second run on
  unchanged input reports `changed=false` and performs no write.
- **Never `unwrap()` on I/O.** Writers swallow I/O failure into
  `EnsureResult::default()`-shaped results exactly where the reference
  swallows it (doc 05 invariants 3 and 4); they never panic and never
  propagate.
- **Ordered desired-value maps.** Missing-key flush order and
  absent-section append order follow Python dict insertion order. Use
  `Vec<(String, String)>` (aliased `Desired`); never `HashMap`/`BTreeMap`
  for a desired-values set.

## File Structure

```
rewrite/crates/grid-core/src/autoconfig/mod.rs          NEW  EnsureResult, RaCredentials, sync_new_emulator, predicates
rewrite/crates/grid-core/src/autoconfig/paths.rs        NEW  xdg/appdata/home helpers, candidate dedupe, expand
rewrite/crates/grid-core/src/autoconfig/writers.rs      NEW  the shared section-writer families
rewrite/crates/grid-core/src/autoconfig/cores.rs        NEW  core list + slug map, installed cores, fuzzy match, flags
rewrite/crates/grid-core/src/autoconfig/entry.rs        NEW  layer 1: entry autoconfig, defaults, variants, backfill
rewrite/crates/grid-core/src/autoconfig/readers.rs      NEW  every *_directory_settings / override reader, Vita3K, VMU
rewrite/crates/grid-core/src/autoconfig/retroarch.rs    NEW  retroarch.cfg writer + config candidates
rewrite/crates/grid-core/src/autoconfig/rpcs3.rs        NEW  config.yml, GuiSettings, CurrentSettings, vfs.yml, games.yml
rewrite/crates/grid-core/src/autoconfig/pcsx2.rs        NEW
rewrite/crates/grid-core/src/autoconfig/duckstation.rs  NEW
rewrite/crates/grid-core/src/autoconfig/dolphin.rs      NEW
rewrite/crates/grid-core/src/autoconfig/azahar.rs       NEW
rewrite/crates/grid-core/src/autoconfig/eden.rs         NEW
rewrite/crates/grid-core/src/autoconfig/ppsspp.rs       NEW
rewrite/crates/grid-core/src/autoconfig/cemu.rs         NEW
rewrite/crates/grid-core/src/autoconfig/xemu.rs         NEW
rewrite/crates/grid-core/src/autoconfig/redream.rs      NEW
rewrite/crates/grid-core/src/lib.rs                     MOD  pub mod autoconfig;
rewrite/crates/grid-core/src/config.rs                  MOD  EmulatorEntry gains 5 profile-derived fields
rewrite/crates/grid-core/src/secrets.rs                 MOD  RaTokenStore (second keyring account)
rewrite/crates/grid-core/src/launch/profiles.rs         MOD  EmulatorProfile gains 5 autoprofile fields
rewrite/crates/grid-core/src/library/mod.rs             MOD  finalize hook, known-platforms slot
rewrite/app/src-tauri/src/commands.rs                   MOD  3 RA commands, manual-add sync, platform capture
rewrite/app/src-tauri/src/lib.rs                        MOD  register commands, RA keyring store in AppState
rewrite/app/src/lib/api.ts                              MOD  RA types + invokes
rewrite/app/src/lib/Emulators.svelte                    MOD  RA settings block
rewrite/app/src/lib/emulators/retroachievements.ts      NEW  pure form helper + vitest
rewrite/e2e/specs/emulator-catalog.spec.ts              MOD  post-install PCSX2 config assertions
rewrite/scripts/check_secret_hygiene.sh                 MOD  autoconfig/mod.rs allowlist + RA-token guards
rewrite/README.md                                       MOD  E2E table row
docs/porting/05-emulator-autoconfig.md                  MOD  deviations section (Task 13)
```

**Embedded data files** — `cores.rs` embeds the two repo-root JSON files the
same way `launch/profiles.rs` embeds the autoprofiles:

```rust
const CORE_LIST_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../retroarch-core-list.json"));
const SLUG_CORES_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../romm-platform-cores.json"));
```

**The three write policies** (doc 05 §"Section-writer helpers and the
overwrite question", the binding contract):

| Policy | Behavior on an existing managed key | Used by |
|---|---|---|
| **Overwrite** | line replaced with the desired value; `changed` only if the text differs; a second occurrence in the same section is DELETED and marks `changed` | retroarch (flat), pcsx2, duckstation, dolphin, azahar, eden, ppsspp, rpcs3 (both INI files) |
| **Add-only** | key recorded as seen, original line emitted verbatim; only missing keys appended | rpcs3 `config.yml` + `vfs.yml`, xemu `xemu.toml` |
| **Append-if-absent block** | the whole block is appended only when its marker header is missing | dolphin `[GCPad1]`, cemu `controller0.xml` |

**Universal writer invariants** (doc 05 invariants 10-12, apply to every
family in `writers.rs`):

- Section headers match `^\[(.+?)\]\s*$` on the STRIPPED line and compare
  case-INSENSITIVELY; keys compare case-SENSITIVELY (exactly). The probe
  helper compares keys case-INSENSITIVELY — deliberately different.
- Missing keys flush at the end of the target section: immediately BEFORE
  the next `[Section]` header, or at EOF.
- An absent section appends one blank separator line (only when the last
  output line is non-blank), then `[<section verbatim>]`, then the keys.
- Output is `lines.join("\n").trim_end() + "\n"` — Python `str.rstrip()`
  removes ALL trailing whitespace including blank lines. CRLF input becomes
  LF (Python `splitlines()`).
- An EMPTY desired set returns `(raw, false)` with NO normalization — the
  early return is observable and must be ported.

---

### Task 1: `autoconfig/writers.rs` + `paths.rs` + module skeleton

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/mod.rs` (skeleton: `EnsureResult`, `pub mod` lines only)
- Create: `rewrite/crates/grid-core/src/autoconfig/writers.rs`
- Create: `rewrite/crates/grid-core/src/autoconfig/paths.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs` (`pub mod autoconfig;`)

**Interfaces (Produces):**
```rust
// --- mod.rs -------------------------------------------------------------
/// Every `ensure_*` writer's return value (spec deviation D8; Python's
/// str-vs-Path-vs-dict mix is a dynamic-typing artifact).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnsureResult {
    /// True when this call wrote at least one file.
    pub changed: bool,
    /// The primary file the writer targeted. `None` when the writer bailed
    /// out (blank path, missing executable, unreadable or unwritable file).
    pub config_path: Option<std::path::PathBuf>,
    /// Secondary files a writer also owns. Documented keys, and only these:
    ///   dolphin -> "gfx_ini_path", "gcpad_ini_path"
    ///   rpcs3   -> "gui_config_path", "current_settings_path", "vfs_path"
    ///   cemu    -> "profile_path"
    ///   ppsspp  -> "ra_token_path"
    pub extras: std::collections::BTreeMap<String, std::path::PathBuf>,
}
impl EnsureResult {
    pub fn unchanged() -> Self;                       // all-default
    pub fn at(path: impl Into<PathBuf>, changed: bool) -> Self;
    pub fn with_extra(self, key: &str, path: impl Into<PathBuf>) -> Self;
    pub fn merge_changed(&mut self, other: bool);     // `self.changed |= other`
}

// --- writers.rs ---------------------------------------------------------
/// Desired section values in Python dict insertion order. Never a hash map:
/// flush order and append order are observable.
pub type Desired = Vec<(String, String)>;
/// `desired![("Key", "value"), ...]` — builds a `Desired` from &str pairs.
#[macro_export] macro_rules! desired { /* ... */ }

/// Overwrite policy, narrow key charset `[A-Za-z0-9_]`.
/// pcsx2.py:56-122, duckstation.py:54-120, dolphin.py:159-225, ppsspp.py:6-72.
pub fn ini_overwrite_section(raw: &str, section: &str, desired: &Desired) -> (String, bool);

/// Overwrite policy, WIDENED key charset `[A-Za-z0-9_%\\]` (azahar.py:94).
pub fn azahar_section(raw: &str, section: &str, desired: &Desired) -> (String, bool);

/// Overwrite policy + generated `key\default=false` annotation lines
/// (eden.py:111-203).
pub fn eden_annotated_section(raw: &str, section: &str, desired: &Desired) -> (String, bool);

/// Overwrite policy + annotation handling driven by `annotate`
/// (rpcs3.py:172-271). `annotate=true` writes `key\default=false` then
/// `key = value`; `annotate=false` DELETES managed annotation lines and
/// writes `key=value` with NO spaces.
pub fn rpcs3_gui_section(raw: &str, section: &str, desired: &Desired, annotate: bool) -> (String, bool);

/// Add-only, 2-space-indented YAML sections (rpcs3.py:113-169).
pub fn yaml_add_only_section(raw: &str, section: &str, desired: &Desired) -> (String, bool);

/// Add-only TOML, key charset allows `-`, dotted section names are literal
/// (xemu.py:184-240).
pub fn toml_add_only_section(raw: &str, section: &str, desired: &Desired) -> (String, bool);

/// The flat `key = "value"` RetroArch writer (retroarch.py:301-350).
/// `preserve_if_present` names keys whose existing line is kept verbatim
/// (only `audio_volume` today).
pub fn flat_cfg(raw: &str, desired: &Desired, preserve_if_present: &[&str]) -> (String, bool);

/// Append-if-absent whole block (dolphin.py:390, cemu.py:343).
/// `marker` is a pre-built case-insensitive multiline regex.
pub fn append_block_if_absent(raw: &str, marker: &regex::Regex, block: &str) -> (String, bool);

/// Case-INSENSITIVE key probe, narrow charset (pcsx2.py:125-143,
/// duckstation.py:123-141). Section compare is case-insensitive too.
pub fn section_has_key(raw: &str, section: &str, key: &str) -> bool;

// --- paths.rs -----------------------------------------------------------
pub fn home_dir() -> Option<PathBuf>;                 // directories::UserDirs / $HOME
pub fn xdg_config_home() -> PathBuf;                  // $XDG_CONFIG_HOME or ~/.config (core/path.py:33)
pub fn xdg_data_home() -> PathBuf;                    // $XDG_DATA_HOME  or ~/.local/share (core/path.py:40)
pub fn env_dir(var: &str) -> Option<PathBuf>;         // set + non-blank after trim
pub fn expand_user(text: &str) -> PathBuf;            // leading `~` only, like Path.expanduser
pub fn dedupe_casefold(paths: Vec<PathBuf>) -> Vec<PathBuf>; // key = to_string_lossy().to_lowercase(), first wins
/// `path` when it is an existing directory, else its parent. The
/// `emulator_dir` rule shared by duckstation/dolphin/azahar/eden/cemu/
/// ppsspp/xemu/redream.
pub fn emulator_dir(path: &Path) -> Option<PathBuf>;
```

**Per-family details the implementer must not deviate from:**

- **`ini_overwrite_section`** (the family used by four modules): section
  regex `^\[(.+?)\]\s*$` on the trimmed line, name trimmed, compared
  lowercased. Key regex `^\s*([A-Za-z0-9_]+)\s*=` on the RAW line. Managed
  key → emit `format!("{key} = {value}")` (one space either side); set
  `changed` only when `raw_line.trim() != replacement`. Duplicate managed
  key in the same section → skip the line, `changed = true`. Unmanaged keys
  inside the target are NOT recorded as seen and pass through verbatim.
  Empty `desired` → `(raw.to_string(), false)` immediately.
- **`azahar_section`**: identical, key regex
  `^\s*([A-Za-z0-9_%\\]+)\s*=` — the `%` and `\` are what let it manage
  `Shortcuts\Main%20Window\Fullscreen\KeySeq`. Use a Rust raw string for
  the pattern.
- **`eden_annotated_section`**: two tracked sets, `seen_keys` and
  `seen_annotations`. Annotation regex `^\s*([A-Za-z0-9_]+)\\default\s*=`.
  An annotation for a MANAGED key is rewritten to exactly
  `key\default=false` (NO spaces); a duplicate annotation is dropped and
  marks changed; an annotation for an UNMANAGED key passes through and the
  line is consumed (it must not fall into the key branch). A managed key
  line with no annotation seen yet emits `key\default=false` BEFORE it and
  marks changed, then `key = value` (WITH spaces). Flush and new-section
  paths emit the annotation then the value line.
- **`rpcs3_gui_section`**: same annotation regexes; `annotate=false` DELETES
  every managed `key\default=` line (marks changed) and emits
  `format!("{key}={value}")` with no spaces. `annotate=true` behaves like
  `eden_annotated_section`. Both overwrite existing managed values.
- **`yaml_add_only_section`**: section regex `^([A-Za-z][^:\n]*):[ \t]*$` on
  the RAW line; name compared with `trim()` and CASE-SENSITIVELY (the only
  case-sensitive section compare in the file). Key regex `^  ([^:]+):` —
  two leading spaces to match, and the captured group is recorded TRIMMED
  (rpcs3.py:154's `group(1).strip()`), so a deeper-nested `    Key:` and a
  padded `  Key :` both mark `Key` as seen. Under add-only that is the safe
  direction — appending a duplicate mapping key would corrupt the YAML.
  Emitted line `format!("  {key}: {value}")`, unquoted. Absent section
  appends `format!("{section}:")` using the UNTRIMMED argument.
- **`toml_add_only_section`**: key regex `^\s*([A-Za-z0-9_\-]+)\s*=`;
  records EVERY matched key in `seen_keys` (managed or not — this differs
  from every other family), and records it UNTRIMMED (xemu.py:225 has no
  `.strip()`; only the YAML writer trims); emitted line
  `format!("{key} = {value}")`.
  Dotted names like `display.window` are matched as literal whole strings,
  never resolved as a path.
- **`flat_cfg`**: no sections. Key regex `^\s*([A-Za-z0-9_]+)\s*=` on the
  raw line. Unmatched lines and unmanaged keys pass through verbatim.
  Duplicate managed key → dropped, `changed = true`. A key in
  `preserve_if_present` → the ORIGINAL line is appended byte-for-byte, the
  key is added to `seen_keys` (so the append phase skips it) and `changed`
  is NOT set. Otherwise emit `format!("{key} = \"{value}\"")` — the value is
  double-quoted here, unlike every INI family. Append phase adds every
  unseen desired key in order, marking changed. Output normalization as
  above.
- **`append_block_if_absent`**: if `marker.is_match(raw)` → `(raw, false)`,
  no write. Else, when `raw` is non-empty and does not end with `\n`, push
  one `\n`; then push `block` verbatim. NO blank separator line, NO
  `trim_end()` normalization.
- **`section_has_key`**: `in_target` is REASSIGNED at every header (so it
  correctly turns off); key compare is lowercased on both sides.

- [ ] **Step 1: write the failing test module.** Concrete test names:
  - `ini_overwrite_replaces_existing_key_and_reports_changed`
  - `ini_overwrite_reports_unchanged_when_text_already_matches` — same
    file twice, second call `changed == false` and output identical
  - `ini_overwrite_deletes_duplicate_managed_key_in_section`
  - `ini_overwrite_leaves_unmanaged_keys_and_comments_verbatim`
  - `ini_overwrite_flushes_missing_keys_before_next_section_header`
  - `ini_overwrite_flushes_missing_keys_at_eof`
  - `ini_overwrite_appends_absent_section_with_one_blank_separator`
  - `ini_overwrite_appends_absent_section_without_separator_after_blank_line`
  - `ini_overwrite_matches_section_case_insensitively` — `[ui]` is found
    when the target is `UI`
  - `ini_overwrite_matches_keys_case_sensitively` — a file with
    `startfullscreen = x` and desired `StartFullscreen` produces BOTH lines
  - `ini_overwrite_empty_desired_returns_input_verbatim` — input with
    trailing blank lines comes back byte-identical
  - `ini_overwrite_normalizes_trailing_whitespace_and_crlf`
  - `azahar_key_regex_manages_backslash_and_percent_keys` — asserts
    `Shortcuts\Main%20Window\Fullscreen\KeySeq = F1` is rewritten, not
    duplicated
  - `eden_generates_annotation_line_before_managed_key`
  - `eden_rewrites_existing_annotation_to_canonical_no_space_form`
  - `eden_drops_duplicate_annotation_and_passes_unmanaged_annotation_through`
  - `rpcs3_gui_annotate_true_emits_annotation_pairs`
  - `rpcs3_gui_annotate_false_deletes_managed_annotation_lines`
  - `rpcs3_gui_annotate_false_writes_key_equals_value_without_spaces`
  - `yaml_add_only_keeps_existing_value` and
    `yaml_add_only_appends_missing_key_with_two_space_indent`
  - `yaml_section_compare_is_case_sensitive` — target `Audio`, file has
    `audio:` → a second `Audio:` section is appended
  - `yaml_nested_or_padded_key_still_marks_desired_key_seen` — the captured
    key is trimmed (rpcs3.py:154), so both `    Key: old` and `  Key : old`
    mark `Key` seen and the file comes back unchanged
  - `toml_add_only_keeps_existing_value_and_allows_dashed_keys`
  - `toml_dotted_section_is_literal` — `[display.window]` desired does not
    match a `[display]` header
  - `flat_cfg_quotes_values_and_appends_missing_keys`
  - `flat_cfg_preserves_audio_volume_line_verbatim_without_marking_changed`
  - `flat_cfg_drops_duplicate_managed_keys`
  - `append_block_if_absent_skips_when_marker_matches_case_insensitively`
  - `append_block_if_absent_adds_newline_only_when_missing`
  - `section_has_key_is_case_insensitive_on_key_and_section`
  - `paths::dedupe_casefold_keeps_first_occurrence`
  - `paths::emulator_dir_uses_parent_for_a_file_and_self_for_a_directory`

  The policy table gets one table-driven test, pinned exactly:

  ```rust
  /// doc 05's three write policies, pinned as a table. `raw` already
  /// contains the managed key with a DIFFERENT value; only the overwrite
  /// family may change it.
  #[test]
  fn write_policy_table_matches_doc_05() {
      let ini_raw = "[Sec]\nKey = old\n";
      let (out, changed) = ini_overwrite_section(ini_raw, "Sec", &desired![("Key", "new")]);
      assert_eq!(out, "[Sec]\nKey = new\n");
      assert!(changed, "overwrite policy must rewrite an existing key");

      let yaml_raw = "Sec:\n  Key: old\n";
      let (out, changed) = yaml_add_only_section(yaml_raw, "Sec", &desired![("Key", "new")]);
      assert_eq!(out, "Sec:\n  Key: old\n");
      assert!(!changed, "add-only policy must never touch an existing key");

      let toml_raw = "[sec]\nkey = old\n";
      let (out, changed) = toml_add_only_section(toml_raw, "sec", &desired![("key", "new")]);
      assert_eq!(out, "[sec]\nkey = old\n");
      assert!(!changed, "add-only policy must never touch an existing key");

      let block_raw = "[GCPad1]\nDevice = x\n";
      let marker = regex::RegexBuilder::new(r"^\[GCPad1\]")
          .case_insensitive(true).multi_line(true).build().unwrap();
      let (out, changed) = append_block_if_absent(block_raw, &marker, "[GCPad1]\nDevice = y\n");
      assert_eq!(out, block_raw);
      assert!(!changed, "append-if-absent must not append when the marker exists");
  }
  ```

- [ ] **Step 2: run** `cargo test -p grid-core autoconfig::writers` — red.
- [ ] **Step 3: implement** `paths.rs`, `writers.rs`, and the `mod.rs`
  skeleton. The families may share a private core function, but each public
  entry point must keep its exact documented behavior.
- [ ] **Step 4: green** — `cargo test -p grid-core`, clippy, fmt, hygiene.
- [ ] **Step 5: commit** `rewrite: autoconfig shared section writers and path helpers`

### Task 2: `autoconfig/cores.rs` — RetroArch core metadata

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/cores.rs`
- Modify: `rewrite/crates/grid-core/src/autoconfig/mod.rs` (`pub mod cores;`)

**Interfaces (Produces):**
```rust
/// One `retroarch-core-list.json` element, as parsed.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CoreEntry {
    #[serde(default)] pub core_file: String,
    #[serde(default)] pub platforms: Vec<serde_json::Value>,
    #[serde(default)] pub supports_save_states: Option<serde_json::Value>,
    #[serde(default)] pub supports_saves: Option<serde_json::Value>,
    #[serde(default)] pub cloud_sync_safe: Option<serde_json::Value>,
    #[serde(default)] pub vmu_shared_saves: Option<serde_json::Value>,
    #[serde(default)] pub firmware: Option<serde_json::Value>,
    #[serde(default)] pub config_files: Option<serde_json::Value>,
    #[serde(default)] pub saves_files: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreFlags {
    pub supports_save_states: bool, // default true
    pub supports_saves: bool,       // default true
    pub cloud_sync_safe: bool,      // default true
    pub vmu_shared_saves: bool,     // default FALSE
}
impl Default for CoreFlags { /* the four defaults above */ }

/// Embedded, parsed once. `compatibility_map` is platform key -> core ids.
pub fn core_entries() -> &'static [CoreEntry];
pub fn compatibility_map() -> &'static std::collections::BTreeMap<String, Vec<String>>; // see ordering note
pub fn slug_core_map() -> &'static std::collections::BTreeMap<String, Vec<String>>;

pub fn core_id_from_file_name(name: &str) -> String;   // retroarch.py:104-116
pub fn core_id_from_display_name(name: &str) -> String;// retroarch.py:59-101
pub fn markdown_label(value: &str) -> String;          // retroarch.py:49-56
pub fn normalize_platform_key(value: &str) -> String;  // retroarch.py:119-127
pub fn platform_tokens(value: &str) -> std::collections::BTreeSet<String>; // retroarch.py:130-133

pub fn cores_for_slug(slug: &str, map: &BTreeMap<String, Vec<String>>) -> Vec<String>;
pub fn system_keys_for_platform(platform: &str, compat: &CompatMap) -> Vec<String>;
pub fn cores_for_platform(platform: &str, compat: &CompatMap) -> Vec<String>;
pub fn all_cores(compat: &CompatMap) -> Vec<String>;

/// Installed core IDS (not paths). `cores_dir` overrides discovery.
pub fn installed_core_ids(emulator_path: &str, cores_dir: Option<&Path>) -> std::collections::BTreeSet<String>;
pub fn core_flags(core_id: &str, entries: &[CoreEntry]) -> CoreFlags;
pub fn core_flags_for_platform(platform: &str, entries: &[CoreEntry]) -> Option<CoreFlags>;
pub fn core_firmware_metadata(core_id: &str, entries: &[CoreEntry]) -> Option<&serde_json::Map<String, serde_json::Value>>;
pub fn core_config_files_metadata(core_id: &str, entries: &[CoreEntry]) -> Option<&serde_json::Map<String, serde_json::Value>>;
pub fn core_saves_files_metadata(core_id: &str, entries: &[CoreEntry]) -> Option<&serde_json::Map<String, serde_json::Value>>;
```

**Ordering note (binding):** Python's compatibility map preserves
first-encountered key order and per-key first-seen core order. Key order is
observable ONLY in the fuzzy-match tie-break (`>` is strict, so the FIRST
key at a tied score wins). Use an insertion-ordered map — a
`Vec<(String, Vec<String>)>` behind a `CompatMap` newtype with a
`get(&str)` and an `iter()` — NOT a `BTreeMap`, or the tie-break changes.
Fix the `CompatMap` type in the interface accordingly.

**Pinned rules:**

- **Core list loading** (retroarch.py:367-432): missing/unreadable → empty
  map. JSON parse failure does NOT return — it falls through to the
  Markdown-table parser. Only a JSON **array** takes the JSON branch;
  an object falls through to Markdown (which finds no `|` lines and yields
  an empty map). JSON branch: skip non-objects; require a non-blank string
  `core_file`; `core_id = core_id_from_file_name(core_file)`, skip if
  empty; require `platforms` to be an array; per string platform,
  `key = normalize_platform_key(platform)`, skip empty, append `core_id`
  to that key's list only when not already present.
- **Markdown fallback** (retroarch.py:409-432): keep lines whose trimmed
  form starts with `|`; `columns = line.split('|')` each trimmed; require
  `columns.len() >= 4`; `core_cell = columns[1]`, `system_cell =
  columns[2]`; skip when either is empty, when
  `core_cell.to_lowercase() == "core"`, when `system_cell` starts with `:`,
  or when `system_cell == "-"`. `core_id_from_display_name(core_cell)` —
  the DISPLAY-name derivation, not the file-name one.
- **`core_id_from_file_name`**: trim, `\` → `/`, take the last `/` segment,
  lowercase, strip ONE suffix from `(".dll", ".so", ".dylib")` (first match
  wins), then strip a trailing `_libretro`, then trim.
- **`markdown_label`**: not a regex. Trim; return as-is unless it starts
  with `[`; find `"]("`; return as-is when that index is `<= 1` or the
  string does not end with `)`; else the trimmed slice between index 1 and
  the marker.
- **`core_id_from_display_name`**: `markdown_label` → trim → lowercase,
  then the 22-entry override table (retroarch.py:61-84), quoted verbatim
  below; on a miss, collapse every run of non-alphanumeric characters to a
  single `_` and trim leading/trailing `_`.

  | key | value | | key | value |
  |---|---|---|---|---|
  | `beetle psx` | `mednafen_psx` | | `mupen64plus-next gles3` | `mupen64plus_next` |
  | `beetle psx hw` | `mednafen_psx_hw` | | `parallel n64` | `parallel_n64` |
  | `beetle saturn` | `mednafen_saturn` | | `pcsx rearmed` | `pcsx_rearmed` |
  | `beetle vb` | `mednafen_vb` | | `snes9x 2002` | `snes9x2002` |
  | `fb neo` | `fbneo` | | `snes9x 2005` | `snes9x2005` |
  | `fceumm` | `fceumm` | | `snes9x 2005 plus` | `snes9x2005_plus` |
  | `flycast gles2` | `flycast` | | `snes9x 2010` | `snes9x2010` |
  | `lrps2` | `lrps2` | | `same cdi` | `same_cdi` |
  | `mame 2003-plus` | `mame2003_plus` | | `vba-m` | `vbam` |
  | `mesen-s` | `mesen_s` | | `vba next` | `vba_next` |
  | `mupen64plus-next` | `mupen64plus_next` | | `mupen64plus-next gles2` | `mupen64plus_next` |

- **`normalize_platform_key`**: trim, lowercase, return `""` if empty,
  `\` → `/`, replace every run of `[^a-z0-9]` with one space, collapse
  whitespace runs, trim.
- **`platform_tokens`**: replace `[^a-z0-9]+` with a space on the trimmed
  lowercased input, split on whitespace, drop exactly these five tokens:
  `the`, `and`, `of`, `for`, `system`.
- **`system_keys_for_platform`** (retroarch.py:435-463): normalize; return
  `[]` on an empty key or an empty map; exact hit → `vec![key]`; else
  `input_tokens = platform_tokens(platform)` from the RAW platform string
  (not the normalized key — keep both call sites); return `[]` if empty.
  Walk keys in map order, `score = |a ∩ b| / |a ∪ b|` as f64, keep the best
  with a STRICT `>` (first key wins a tie), and return `vec![best]` only
  when `best_score >= 0.7` (`>=`, not `>`). Otherwise `[]`.
- **`cores_for_platform`** (retroarch.py:466-478): an EMPTY compatibility
  map returns the hardcoded `vec!["fbneo", "mame2003_plus"]`; a populated
  map with no match returns `[]`. Order-preserving dedupe on append.
- **`cores_for_slug`**: `[]` for a blank slug or an empty map; else exact
  lookup on the TRIMMED slug (case-sensitive), cloned. Map loading keeps
  entries whose key is a non-blank string and whose value is an array,
  storing the key trimmed and each non-blank string element UNTRIMMED, with
  no dedupe and no lowercasing.
- **`installed_core_ids`** (retroarch.py:481-526): with an explicit
  `cores_dir`, skip every emulator-path check. Otherwise: blank path →
  empty set; the path must EXIST and be a FILE → else empty set; try the
  AppImage layout `<parent>/<full file name>.home/.config/retroarch/cores`
  and use it when it exists and is a directory, else `<parent>/cores`; the
  chosen directory must exist and be a directory. Non-recursive read of the
  directory, files only, extension by host: `dll` on windows, `dylib` on
  macos, `so` elsewhere (gate with `cfg!` and expose a
  `#[cfg(test)]`-visible extension override so all three can be tested).
  Collect `core_id_from_file_name(file_name)` when non-empty.
- **`core_flags`** (retroarch.py:581-604): defaults
  `supports_save_states=true, supports_saves=true, cloud_sync_safe=true,
  vmu_shared_saves=FALSE` (the docstring claiming all-true is wrong —
  follow the code). First entry whose `core_file` is non-blank and whose
  derived core id equals `core_id` wins; each of the four flags is
  `bool()`-coerced from whatever JSON value is present, so `0`, `""`,
  `null` and `[]` are all false; a missing key falls back to its default.
  No match → the defaults.
- **`core_flags_for_platform`** (retroarch.py:607-622): `target =
  platform.trim().to_lowercase()`; `None` for a blank target. Match is a
  plain trimmed+lowercased EXACT compare against each `platforms` string —
  NOT `normalize_platform_key`. On a match, derive the core id and, when
  non-empty, return `core_flags(core_id, entries)` (a second full scan, so
  a different entry sharing the id may answer); an empty core id CONTINUES
  to later entries. No match at all → `None`, which is distinct from the
  all-defaults value.
- **Metadata accessors**: return the sub-value only when it is a JSON
  object, and return on the FIRST core-id match — a matching entry without
  the field yields `None`, it does not keep searching.

- [ ] **Step 1: failing tests.** Named cases:
  - `embedded_core_list_parses_and_has_233_entries`
  - `embedded_slug_map_parses_and_has_75_slugs`
  - `core_id_from_file_name_strips_extension_then_libretro` — table:
    `flycast_libretro.dll`→`flycast`, `MGBA_LIBRETRO.SO`→`mgba`,
    `a/b/snes9x_libretro.dylib`→`snes9x`, `a\\b\\x.dll`→`x`,
    `no_extension`→`no_extension`, `""`→`""`
  - `core_id_from_display_name_applies_all_22_overrides` — one assert per
    table row, driven by the table above
  - `core_id_from_display_name_slugifies_unknown_names` — `FB Neo (2019)`
    → `fb_neo_2019`, `  --x--  ` → `x`
  - `markdown_label_extracts_link_text_and_passes_plain_text_through` —
    `[FB Neo](u)`→`FB Neo`, `[](u)`→`[](u)`, `[x](u` →`[x](u`
  - `normalize_platform_key_collapses_punctuation`
  - `platform_tokens_drops_the_five_stopwords`
  - `system_keys_exact_match_wins_before_fuzzy`
  - `system_keys_fuzzy_accepts_at_exactly_070` and
    `system_keys_fuzzy_rejects_just_below_070` — construct token sets whose
    Jaccard is exactly 0.7 and 0.6
  - `system_keys_fuzzy_tie_break_keeps_first_map_key`
  - `cores_for_platform_returns_arcade_fallback_only_for_an_empty_map`
  - `cores_for_platform_returns_empty_for_a_populated_map_with_no_match`
  - `cores_for_slug_is_exact_on_the_trimmed_slug`
  - `slug_map_drops_non_string_slugs_blank_slugs_and_non_list_values`
  - `installed_cores_prefers_appimage_home_layout` — tempdir with both
    `<exe>.home/.config/retroarch/cores/a_libretro.so` and
    `<dir>/cores/b_libretro.so`; only `a` is returned
  - `installed_cores_falls_back_to_sibling_cores_dir`
  - `installed_cores_requires_an_existing_file_without_an_override`
  - `installed_cores_explicit_dir_skips_the_executable_checks`
  - `installed_cores_extension_per_platform` — via the test-only extension
    override, `.dll` / `.dylib` / `.so`
  - `core_flags_defaults_vmu_shared_saves_to_false`
  - `core_flags_coerces_json_falsy_values`
  - `core_flags_for_platform_matches_case_insensitively_and_exactly`
  - `core_flags_for_platform_returns_none_when_no_platform_matches`
  - `metadata_accessors_return_none_for_a_matching_entry_without_the_field`
  - `real_catalog_flycast_has_vmu_shared_saves` — asserts against the
    embedded file, mirroring `tests/test_flycast_vmu.py:41`
- [ ] **Step 2: red run.**
- [ ] **Step 3: implement.**
- [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: retroarch core metadata, installed-core discovery, platform matching`

### Task 3: `autoconfig/retroarch.rs` — config candidates, reader, flat writer

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/retroarch.rs`
- Modify: `autoconfig/mod.rs` (`pub mod retroarch;`)

**Interfaces (Produces):**
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetroarchSettings {
    pub config_path: String,          // "" when nothing parsed
    pub savefile_directory: String,   // "" when unset or the `default` sentinel
    pub savestate_directory: String,
    pub savefiles_in_content_dir: bool,
    pub savestates_in_content_dir: bool,
    pub sort_savefiles_enable: bool,
    pub sort_savestates_enable: bool,
    pub sort_savefiles_by_content_enable: bool,
    pub sort_savestates_by_content_enable: bool,
}

pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf>;
pub fn directory_settings(emulator_path: &str) -> RetroarchSettings;

/// The full managed-key write. `romm_username` is the netplay nickname;
/// `ra` is the RetroAchievements pair. Two DISTINCT variables — the Python
/// rebinds one parameter (retroarch.py:287 vs :294); the port must not.
pub fn ensure_settings(
    emulator_path: &str,
    enable_fullscreen: bool,
    romm_username: &str,
    ra: Option<&crate::autoconfig::RaCredentials>,
) -> EnsureResult;

/// D2 narrow writer: ONLY `cheevos_enable`, `cheevos_username`,
/// `cheevos_token`, and only when both RA fields are non-blank.
pub fn ensure_ra_credentials(emulator_path: &str, ra: &crate::autoconfig::RaCredentials) -> EnsureResult;
```

`RaCredentials` reaches its final form in Task 11, but THIS task defines it
in `mod.rs` as `{ username: String, token: secrecy::SecretString }` with
two accessors, `username(&self) -> &str` and `token(&self) -> &str` (the
second is the ONLY place the secret is exposed, inside `autoconfig/mod.rs`),
and constructs it from plain strings in tests. Because `token()` calls
`expose_secret()`, THIS task also adds `autoconfig/mod.rs` to the
`check_secret_hygiene.sh` allowlist — otherwise this task's own hygiene
gate fails. Add `rewrite/scripts/check_secret_hygiene.sh` to this task's
Modify list.

**Pinned rules:**

- **Candidates** (retroarch.py:136-165): blank path → `vec![]`. Expand `~`.
  Root = the parent when the path IS an existing file OR merely has a
  non-empty extension (`.AppImage`, `.exe`) even if it does not exist;
  otherwise the path itself. Then in order: `<root>/retroarch.cfg`,
  `<root>/config/retroarch.cfg`,
  `<xdg_config_home>/retroarch/retroarch.cfg`,
  `<xdg_data_home>/retroarch/retroarch.cfg`,
  `<home>/.config/retroarch/retroarch.cfg`. Deduped case-insensitively,
  first wins — so candidate 5 usually collapses into candidate 3.
  Existence is NOT checked here.
- **`directory_settings`** (retroarch.py:173-229): walk candidates; skip
  unless the file exists and is a file and reads; parse lines — trim, skip
  empty, skip `#`-prefixed, skip lines with no `=`; split on the FIRST `=`,
  trim both halves; strip ONE matched surrounding quote pair (`"` or `'`)
  when `len >= 2` and the first and last characters are the same quote;
  last duplicate key wins. If nothing parsed, CONTINUE to the next
  candidate. On the first parseable candidate: set `config_path`, then for
  `savefile_directory`/`savestate_directory` take the trimmed value only
  when non-blank AND not the case-insensitive sentinel `default`; then the
  six booleans via the truthy set `{"1", "true", "yes", "on"}` (lowercased,
  trimmed) when the key is present with a non-blank value. Then STOP.
- **`ensure_settings`** (retroarch.py:232-355). Read the settings and the
  candidates. No candidates (blank path) → `EnsureResult::unchanged()`
  (the reference logs a warning and returns the unchanged settings).
  Target = the parsed `config_path` when non-blank (expanded), else
  `candidates[0]`. Desired values, IN THIS ORDER:

  | # | key | value |
  |---|---|---|
  | 1 | `savefile_directory` | current value, else the literal `saves` |
  | 2 | `savestate_directory` | current value, else the literal `states` |
  | 3 | `video_windowed_fullscreen` | `true` |
  | 4 | `audio_volume` | `-18.000000` |
  | 5 | `discord_enable` | `false` |
  | 6 | `pause_nonactive` | `true` |
  | 7 | `video_vsync` | `true` |
  | 8 | `input_menu_toggle_gamepad_combo` | `2` |
  | 9 | `savestate_auto_save` | `false` |
  | 10 | `savestate_auto_load` | `false` |
  | 11 | `rgui_show_start_screen` | `false` |
  | 12 | `menu_show_core_updater` | `false` |
  | 13 | `sort_savefiles_enable` | `false` |
  | 14 | `sort_savestates_enable` | `false` |
  | 15 | `sort_savefiles_by_content_enable` | `false` |
  | 16 | `sort_savestates_by_content_enable` | `false` |
  | 17 | `savefiles_in_content_dir` | `false` |
  | 18 | `savestates_in_content_dir` | `false` |
  | 19 | `cheevos_hardcore_mode_enable` | `false` |
  | 20 | `cheevos_visibility_lboard_start` | `false` |
  | 21 | `cheevos_visibility_lboard_submit` | `false` |
  | 22 | `cheevos_visibility_lboard_trackers` | `false` |

  Then, appended in this order: `netplay_nickname` = the trimmed
  `romm_username` when non-blank; `video_fullscreen` = `true` ONLY when
  `enable_fullscreen` (when false the key is OMITTED, so an existing line
  survives); and, only when BOTH RA fields are non-blank after trimming,
  `cheevos_enable` = `true`, `cheevos_username` = the RA username,
  `cheevos_token` = the token. Keys 19-22 are unconditional regardless of
  credentials.

  Write via `writers::flat_cfg(raw, &desired, &["audio_volume"])`, with
  `changed` seeded to "the file did not exist". Read failure → treat the
  raw content as `""` (the reference discards the unreadable file and
  rewrites from the desired set alone). Write only when changed, after
  `create_dir_all` on the parent. On a write error, return the PRE-write
  settings with `changed = false` (doc 05 invariant 4). On success, re-run
  `directory_settings`, force `config_path` to the write target, and report
  `changed`.
- **`ensure_ra_credentials`** (D2): the same candidate/target resolution,
  but `desired` is exactly the three `cheevos_*` credential keys, and the
  call returns `EnsureResult::unchanged()` immediately when either field is
  blank. It must NOT touch `savefile_directory`, the sort booleans, or the
  four suppression keys.

- [ ] **Step 1: failing tests** (tempdirs; set `XDG_CONFIG_HOME`/
  `XDG_DATA_HOME`/`HOME` inside a serialized env guard so the XDG
  candidates are deterministic — one `static ENV_LOCK: Mutex<()>` in the
  module's test mod, taken by every env-sensitive test):
  - `candidates_use_parent_for_a_file_path_and_self_for_a_directory`
  - `candidates_use_parent_for_a_nonexistent_suffixed_path`
  - `candidates_are_deduped_case_insensitively`
  - `candidates_are_empty_for_a_blank_path`
  - `directory_settings_strips_one_quote_pair`
  - `directory_settings_treats_default_as_unset` (both `default` and
    `DEFAULT`)
  - `directory_settings_parses_the_six_booleans`
  - `directory_settings_skips_a_candidate_with_no_parseable_line`
  - `ensure_writes_defaults_and_disables_all_six_sort_flags` (mirrors
    `tests/test_retroarch_config.py:25`)
  - `ensure_preserves_explicit_directories` (`:65`)
  - `ensure_preserves_an_existing_audio_volume_line_verbatim` (`:72`)
  - `ensure_writes_fullscreen_and_ra_credentials_when_enabled` (`:97`)
  - `ensure_omits_video_fullscreen_when_disabled`
  - `ensure_writes_the_four_cheevos_suppression_keys_unconditionally` (`:120`)
  - `ensure_skips_ra_keys_when_only_the_username_is_set`
  - `ensure_writes_netplay_nickname_from_the_romm_username_not_the_ra_one`
    — romm `six`, RA `sixdd6`: asserts `netplay_nickname = "six"` AND
    `cheevos_username = "sixdd6"` in one file (the rebinding trap)
  - `ensure_is_idempotent_on_a_second_run`
  - `ensure_returns_unchanged_for_a_blank_path` (`:186`)
  - `ensure_ra_credentials_touches_only_the_three_cheevos_keys` — a
    pre-existing config with a sentinel `my_unmanaged_key = "keep"` and a
    non-default `savefile_directory`: both survive byte-identically
  - `ensure_ra_credentials_is_a_no_op_when_either_field_is_blank`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: retroarch.cfg writer, config candidates, directory settings reader`

### Task 4: `autoconfig/rpcs3.rs` — config.yml, GuiSettings, CurrentSettings, vfs.yml, games.yml

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/rpcs3.rs`
- Modify: `autoconfig/mod.rs` (`pub mod rpcs3;`)

**Interfaces (Produces):**
```rust
pub fn ensure_settings(emulator_path: &str, ps3_library_path: &str) -> EnsureResult;
pub fn ensure_vfs_settings(emulator_path: &str, ps3_library_path: &str) -> EnsureResult;
/// Returns false for a blank game id or any I/O error, true otherwise.
pub fn update_games_yml(data_root: &Path, game_id: &str, dev_hdd0_root: &Path, games_root: Option<&Path>) -> bool;
```

**Pinned rules (rpcs3.py:533-602, :389-468, :307-362):**

- `ensure_settings`: blank path → `EnsureResult::unchanged()`. The
  executable must EXIST and be a FILE → else unchanged. Paths, all under
  `<exe parent>`: `portable/`, `portable/config/`, `portable/GuiConfigs/`,
  created EAGERLY with `create_dir_all` before any read. Targets:
  `portable/config/config.yml`, `portable/GuiConfigs/GuiSettings.ini`,
  `portable/GuiConfigs/CurrentSettings.ini`. This is the always-portable
  write target even when the installation is not portable — doc 05's open
  question; follow the code.
- `config.yml` — `writers::yaml_add_only_section`, two calls chained:
  section `Miscellaneous` key `Start games in fullscreen mode` = `true`;
  section `Audio` key `Master Volume` = `40`. Written once when either
  reports changed.
- `GuiSettings.ini` — `writers::rpcs3_gui_section(.., annotate = true)`,
  section `main_window`, keys in order: `infoBoxEnabledWelcome` = `false`,
  `confirmationBoxExitGame` = `false`, `confirmationBoxBootGame` = `false`,
  `infoBoxEnabledInstallPUP` = `false`.
- `CurrentSettings.ini` — `writers::rpcs3_gui_section(.., annotate = false)`,
  two calls: section `Meta` with `checkUpdateStart` = `false`,
  `useRichPresence` = `false`; then section `main_window` with the SAME four
  keys as GuiSettings.ini. **Trap:** this file's lines are
  `key=value` with NO spaces, and every managed `key\default=` annotation
  line in it is DELETED. It is the runtime-authoritative file.
- VFS chaining: when `ps3_library_path.trim()` is non-blank, call
  `ensure_vfs_settings` and OR its `changed` in.
- Result: `config_path` = the `config.yml` path;
  `extras["gui_config_path"]` = GuiSettings.ini;
  `extras["current_settings_path"]` = CurrentSettings.ini (the reference
  omits the third path from its dict — D8 unifies it, so include it);
  `extras["vfs_path"]` when the VFS step ran and produced a path. Any I/O
  error → `EnsureResult::unchanged()`.
- `ensure_vfs_settings`: unchanged for a blank emulator path or a blank
  library path, or when the executable is not an existing file. Target
  `<exe parent>/portable/config/vfs.yml`; the config dir is created
  unconditionally. `library = expand_user(ps3_library_path)` then
  canonicalized where possible; `dev_hdd0 = <library>/.vfs/dev_hdd0`,
  `games = <library>/.vfs/games`, both converted to forward-slash form with
  a guaranteed trailing `/`. Desired entries, in order:
  `$(EmulatorDir)` → `""` (empty), `/dev_hdd0/` → the dev_hdd0 string,
  `/games/` → the games string.
  **Add-only comparison rule:** for each existing line, trim, skip empty
  and `#`-prefixed lines, find the FIRST `:`, skip when absent, take the
  text before it, trim, then `trim_matches('"')` then `trim_matches('\'')`
  IN THAT ORDER. Any desired key already present is skipped entirely.
  Emitted lines are APPENDED at the end: `"{key}": "{value}"` — both key
  and value always double-quoted, including the empty value. Write only
  when changed; output is `lines.join("\n")` plus one `\n` when non-empty
  and not already newline-terminated — **no `trim_end()` normalization here**.
- `update_games_yml`: false for a blank trimmed game id. Game directory =
  `<games_root>/<game_id>` when `games_root` is `Some`, else
  `<dev_hdd0_root>/game/<game_id>`; rendered forward-slash with a trailing
  `/`. Target `<data_root>/config/games.yml`, its parent created. Line
  format `{game_id}: "{dir}"` — unquoted key, `: `, double-quoted value.
  Replacement scan: per line, trim; keep verbatim when there is no `:`;
  else take the text before the first `:`, trim, strip `"` then `'`, and
  when it equals the game id substitute the new line (ALL matching lines
  are replaced, so duplicates become duplicate updated lines). Append when
  nothing matched. Write `lines.join("\n")` plus `\n` when non-empty; no
  normalization. Any I/O error → false.

- [ ] **Step 1: failing tests** (mirroring
  `tests/test_emulator_autoconfig_settings.py:1800-1975` and `:2508-2903`):
  - `ensure_requires_an_existing_executable_file`
  - `ensure_creates_the_three_portable_directories`
  - `ensure_first_run_writes_all_three_files_and_reports_changed`
  - `ensure_second_run_reports_unchanged_and_writes_nothing` — capture the
    three files' mtimes/bytes and assert equality
  - `config_yml_preserves_an_existing_master_volume` — add-only proof
  - `config_yml_appends_a_missing_section_with_a_blank_separator`
  - `gui_settings_writes_annotation_and_value_line_pairs` — asserts the
    exact two-line pair `infoBoxEnabledWelcome\default=false` then
    `infoBoxEnabledWelcome = false`
  - `current_settings_writes_bare_key_equals_value_with_no_spaces` —
    asserts the file contains `checkUpdateStart=false` and NOT
    `checkUpdateStart = false`
  - `current_settings_deletes_managed_default_annotation_lines` — seed the
    file with `confirmationBoxExitGame\default=false` and assert it is gone
  - `current_settings_overwrites_an_existing_managed_value`
  - `ensure_folds_vfs_changed_in_when_a_library_path_is_given`
  - `vfs_writes_the_three_quoted_entries` (`:2508`)
  - `vfs_is_idempotent` (`:2540`)
  - `vfs_never_overwrites_an_existing_key` — seed
    `"/dev_hdd0/": "/somewhere/else/"` and assert it survives (`:2560`)
  - `vfs_matches_an_unquoted_existing_key`
  - `vfs_output_is_not_trailing_whitespace_normalized`
  - `games_yml_appends_a_new_entry_with_the_dev_hdd0_layout` (`:2748`)
  - `games_yml_uses_the_games_root_layout_when_given`
  - `games_yml_updates_in_place` (`:2800`)
  - `games_yml_is_idempotent` (`:2860`)
  - `games_yml_returns_false_for_a_blank_game_id`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: rpcs3 config, gui settings, vfs and games.yml writers`

### Task 5: `autoconfig/pcsx2.rs` + `autoconfig/duckstation.rs`

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/pcsx2.rs`, `duckstation.rs`
- Modify: `autoconfig/mod.rs`

**Interfaces (Produces):**
```rust
// pcsx2.rs
pub fn ensure_settings(
    emulator_path: &str,
    enable_fullscreen: bool,
    ra: Option<&crate::autoconfig::RaCredentials>,
) -> EnsureResult;
pub fn ensure_ra_credentials(emulator_path: &str, ra: &crate::autoconfig::RaCredentials) -> EnsureResult;

// duckstation.rs — NO RetroAchievements parameters. DuckStation is NOT an
// RA target (doc 05 open question, ruled: follow the code); it gets only
// the four `[Cheevos]` suppression keys.
pub fn ensure_memory_card_settings(emulator_path: &str, enable_fullscreen: bool) -> EnsureResult;
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf>;
```

**PCSX2 pinned rules** (pcsx2.py:170-380; the authoritative key tables are
the Python line ranges cited per row — port every key, in order):

- **D4 (binding):** compute `emulator_dir` from the EXPANDED, TRIMMED path
  (`expand_user(text.trim()).parent()`), not the raw text. The reference's
  `Path(emulator_path_text).parent` can create a literal `~` directory;
  that is a bug and is fixed here.
- Blank path → `EnsureResult::unchanged()`. The executable must EXIST and be
  a FILE → else unchanged.
- Always create an empty `portable.ini` next to the executable when absent;
  an I/O failure there is swallowed and does not abort.
- The config file is ALWAYS `<emulator_dir>/inis/PCSX2.ini`. The Documents
  and XDG candidates in `pcsx2_config_path_candidates` (pcsx2.py:146-167)
  are for the READERS only (Task 8) — never for this writer.
- **Trap:** every preserve-if-present probe reads the PROGRESSIVELY
  REWRITTEN content, so a probe at step N sees keys written at steps < N.
  (DuckStation does the opposite — see below. Do not unify them.)
- Write order and gating:

  | # | py line | section | keys | gate |
  |---|---|---|---|---|
  | 1 | :200 | `UI` | `SetupWizardIncomplete`=`false`, `SettingsVersion`=`1` | forced |
  | 2 | :205 | `AutoUpdater` | `CheckAtStartup`=`false` | forced |
  | 3 | :210-217 | `UI` | `InhibitScreensaver`=`true` forced; `ConfirmShutdown`=`false`, `PauseOnFocusLoss`=`true`, `HideMouseCursor`=`true` each per-key preserve-if-present | mixed |
  | 4 | :222 | `EmuCore` | `EnableDiscordPresence`=`false` | forced |
  | 5 | :231-242 | `EmuCore` | `EnableWideScreenPatches`=`true`, `EnableNoInterlacingPatches`=`true` | per-key preserve |
  | 6 | :244-252 | `Achievements` | `Enabled`=`true`, `Username`, `Token` | whole block, only when BOTH RA fields non-blank |
  | 7 | :254 | `EmuCore/GS` | `pcrtc_antiblur`=`true`, `pcrtc_offsets`=`false` | forced |
  | 8 | :265-276 | `EmuCore/GS` | the 10 quality keys | per-key preserve |
  | 9 | :285-297 | `EmuCore/Speedhacks` | `fastCDVD`=`false`, `vuThread`=`true`, `vu1Instant`=`true` | per-key preserve |
  | 10 | :303-339 | `Pad1` | the 35-key SDL map | whole block, gated on `Pad1.Type` being ABSENT |
  | 11 | :343 | `Hotkeys` | `OpenPauseMenu`=`SDL-0/Guide` | block gated on the same key's absence |
  | 12 | :349 | `SPU2/Output` | `StandardVolume`=`40` | block gated on absence |
  | 13 | :355 | `EmuCore/GS` | `upscale_multiplier`=`3` | block gated on absence |
  | 14 | :361 | `UI` | `StartFullscreen`=`true` | only when `enable_fullscreen` |
  | 15 | — | `Folders` | `Bios` | **OMITTED — spec deviation D6** |

  The 10 `[EmuCore/GS]` keys (pcsx2.py:265-276, in order):
  `VsyncEnable`=`true`, `Renderer`=`14`, `filter`=`2`,
  `accurate_blending_unit`=`3`, `MaxAnisotropy`=`4`, `dithering_ps2`=`2`,
  `CASMode`=`2`, `CASSharpness`=`50`, `hw_mipmap`=`true`,
  `texture_preloading`=`2`. The 35-key `[Pad1]` block starts
  `Type`=`DualShock2` and is the authoritative table at pcsx2.py:303-339 —
  transcribe it key-for-key in file order.
- Write only when something changed; `create_dir_all` on `inis/` first.
  Any I/O error → `EnsureResult::unchanged()`.
- `ensure_ra_credentials` (D2): the same target file, `desired` = exactly
  the three `[Achievements]` keys, unchanged when either field is blank.

**DuckStation pinned rules** (duckstation.py:198-386):

- **Trap:** every preserve-if-present probe reads the ORIGINAL,
  pre-write `raw_content`, never the progressively updated text.
- `emulator_dir` = the path itself when it is a directory, else its parent
  (on the trimmed, expanded path). Create an empty `portable.txt` when
  absent; swallow failure.
- Candidates (duckstation.py:10), each with `settings.ini` appended and
  deduped: emulator dir → `%LOCALAPPDATA%/DuckStation` →
  `~/Documents/DuckStation` → `~/.local/share/duckstation` →
  `~/.config/duckstation` → `~/Library/Application Support/DuckStation` →
  `<XDG_DATA_HOME>/duckstation` → `<XDG_CONFIG_HOME>/duckstation`.
  Empty candidate list → return the reader's values with `changed = false`.
- **Write target:** ALWAYS `<emulator_dir>/settings.ini` when a path was
  supplied, regardless of which candidate was READ. Only with no path does
  it fall back to the parsed `config_path`, then `candidates[0]`. Values
  are therefore migrated out of a system config into a new portable file —
  doc 05 open question; follow the code.
- `changed` is seeded to "the target did not exist".
- Write order and gating:

  | # | py line | section | keys | gate |
  |---|---|---|---|---|
  | 1 | :231-249 | `MemoryCards` | `Directory` (current value or `memcards`), `Card1Type` (kept only when one of `PerGame`/`PerGameTitle`/`PerGameFileTitle`, else `PerGameTitle`), `Card2Type` (those three or `None`, else `None`), `UsePlaylistTitle`=`true` | forced keys, value-level preservation |
  | 2 | :252-258 | `Main` | `InhibitScreensaver`=`true`, `SetupWizardIncomplete`=`false` forced; `ConfirmPowerOff`=`false` preserve-if-present | mixed |
  | 3 | :263-269 | `Display` | `FullscreenMode`=`Borderless Windowed` forced (a space, unquoted); `Scaling`=`Lanczos`, `Scaling24Bit`=`Lanczos` preserve | mixed |
  | 4 | :275 | `AutoUpdater` | `CheckAtStartup`=`false` | forced |
  | 5 | :284-294 | `GPU` | the 9 keys | per-key preserve |
  | 6 | :304 | `Audio` | `OutputVolume`=`60` | preserve; when present the desired set is EMPTY and the writer early-returns without normalizing |
  | 7 | :312 | `Hotkeys` | `OpenPauseMenu`=`SDL-0/Guide` | same |
  | 8 | :322-348 | `Pad1` | the 26-key map | whole block gated on `Pad1.Type` absence |
  | 9 | :355 | `Main` | `StartFullscreen`=`true` | only when `enable_fullscreen` |
  | 10 | :363 | `Cheevos` | `Enabled`=`true`, `ChallengeMode`=`false`, `LeaderboardNotifications`=`false`, `LeaderboardTrackers`=`false` | forced, unconditional — no credential gate |

  The 9 `[GPU]` keys (duckstation.py:284-294, in order):
  `ResolutionScale`=`4`, `PGXPEnable`=`true`, `PGXPColorCorrection`=`true`,
  `TextureFilter`=`Scale2x`, `SpriteTextureFilter`=`Scale2x`,
  `DitheringMode`=`TrueColorFull`, `LineDetectMode`=`BasicTriangles`,
  `DownsampleMode`=`Box`, `DownsampleScale`=`2`. The 26-key `[Pad1]` block
  starts `Type`=`AnalogController`; duckstation.py:322-348 is the
  authoritative table — transcribe it key-for-key in file order.
- Write only when changed. A write error returns the pre-write values with
  `changed = false`.

- [ ] **Step 1: failing tests** (tempdirs; the env guard from Task 3 for
  the XDG candidates). PCSX2, mirroring
  `tests/test_emulator_autoconfig_settings.py:201-500`:
  - `pcsx2_creates_portable_ini` / `pcsx2_does_not_overwrite_portable_ini`
  - `pcsx2_requires_an_existing_executable_file`
  - `pcsx2_expands_a_tilde_path_and_creates_no_literal_tilde_directory`
    (D4 — the regression this deviation fixes)
  - `pcsx2_writes_the_forced_ui_and_gs_keys`
  - `pcsx2_fullscreen_key_only_when_enabled` (`:278`, `:295`)
  - `pcsx2_ra_block_requires_both_fields` (`:311`)
  - `pcsx2_preserves_an_existing_pad1_block` (`:384`) — seed only
    `[Pad1] Type = X` and assert none of the 35 keys were added
  - `pcsx2_preserves_hotkey_volume_and_upscale_when_present`
    (`:419`, `:453`, `:487`)
  - `pcsx2_writes_all_ten_gs_quality_keys_when_absent`
  - `pcsx2_never_writes_folders_bios` (D6)
  - `pcsx2_is_idempotent`
  - `pcsx2_ensure_ra_credentials_touches_only_the_achievements_keys` — a
    sentinel `[UI] MyKey = keep` and a non-default `[EmuCore/GS] Renderer`
    both survive
  DuckStation, mirroring `tests/test_duckstation_config.py`:
  - `duckstation_creates_portable_txt` / `..._does_not_overwrite_it`
  - `duckstation_forces_per_game_memory_card_defaults` (`:17`)
  - `duckstation_preserves_an_explicit_memcard_directory` (`:52`)
  - `duckstation_forces_fullscreen_mode_and_cheevos_suppression` (`:79`)
  - `duckstation_disables_the_auto_updater` (`:112`)
  - `duckstation_preserves_gpu_display_audio_hotkey_and_pad1_when_present`
    (`:127`-`:355`) — one test per group
  - `duckstation_forces_setup_wizard_incomplete_from_true_to_false` (`:370`)
  - `duckstation_preserves_existing_cheevos_credentials_through_a_portable_write`
    (`:389`) — the migration case: read from an XDG config, write to the
    portable target, `[Cheevos] Username` survives
  - `duckstation_probes_the_original_content_not_the_rewritten_one` — seed
    a file WITHOUT `[Main] ConfirmPowerOff` but WITH `[Audio] OutputVolume`;
    assert `ConfirmPowerOff` is written and `OutputVolume` is untouched
  - `duckstation_writes_to_the_emulator_dir_even_when_read_elsewhere` (`:171`)
  - `duckstation_candidate_order_under_xdg_overrides` (`:421`-`:446`)
  - `duckstation_is_idempotent`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: pcsx2 and duckstation settings writers`

### Task 6: `autoconfig/dolphin.rs` + `azahar.rs` + `eden.rs`

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/dolphin.rs`, `azahar.rs`, `eden.rs`
- Modify: `autoconfig/mod.rs`

**Interfaces (Produces):**
```rust
// dolphin.rs
pub fn ini_path_candidates(emulator_path: &str, ini_name: &str) -> Vec<PathBuf>;
pub fn ensure_settings(emulator_path: &str) -> EnsureResult;   // Dolphin.ini + GFX.ini
pub fn ensure_skip_ipl(emulator_path: &str) -> EnsureResult;   // [Core] SkipIPL = False
pub fn ensure_gcpad_config(emulator_path: &str) -> EnsureResult;

// azahar.rs
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf>;
pub fn ensure_settings(emulator_path: &str) -> EnsureResult;

// eden.rs
pub fn config_path_candidates(emulator_path: &str) -> Vec<PathBuf>;
pub fn ensure_settings(emulator_path: &str) -> EnsureResult;
```

**Dolphin** (dolphin.py:228-406):

- Candidates for an ini name, in order, deduped case-insensitively:
  `<exe parent>/User/Config/<name>` (ONLY when the expanded path is
  absolute), `%APPDATA%/Dolphin Emulator/Config/<name>`,
  `~/.local/share/dolphin-emu/<name>`,
  `~/Library/Application Support/Dolphin/<name>`,
  `~/.var/app/org.DolphinEmu.dolphin-emu/data/dolphin-emu/<name>`. Note
  candidates 3-5 have NO `Config` component.
- `ensure_settings`: create an empty `portable.txt` next to the executable
  when absent (swallow failure). **Selection rule:** when the path text is
  non-blank, use `candidates[0]` UNCONDITIONALLY — the portable
  `User/Config` path; otherwise the first EXISTING candidate, falling back
  to `candidates[0]`. `ensure_skip_ipl` instead picks the first EXISTING
  candidate. The divergence is deliberate (doc 05 open question; follow the
  code) — the two entry points can target different files.
- `Dolphin.ini`, all forced overwrites, in order: `[Analytics] Enabled` =
  `False`, `PermissionAsked` = `True`; `[Display] Fullscreen` = `True`,
  `RenderToMain` = `True`; `[General] ShowLaunchWarning` = `False`;
  `[DSP] Volume` = `70`. `GFX.ini`: `[Settings] UseVerticalSync` = `True`.
  **Capitalized `True`/`False` — Dolphin's convention, not `true`/`false`.**
- Each file is written in its own fallible scope: a failure on one sets
  that path to `None` in the result without aborting the other. Result:
  `config_path` = the `Dolphin.ini` path, `extras["gfx_ini_path"]` = the
  `GFX.ini` path; `changed` = the OR of the two, set only when a write
  actually happened.
- `ensure_skip_ipl`: one section write, `[Core] SkipIPL` = `False`.
- `ensure_gcpad_config`: target selection has three tiers — the first
  EXISTING `GCPadNew.ini` candidate; else the parent directory of the first
  EXISTING `Dolphin.ini` candidate joined with `GCPadNew.ini`; else
  `candidates[0]` for `GCPadNew.ini`. Marker regex
  `^\[GCPad1\]` built with `case_insensitive(true).multi_line(true)`, used
  with `is_match` (a `search`, not a full-line match, so
  `[GCPad1] trailing` also counts as present). Present → no write,
  `changed = false`. Otherwise append via
  `writers::append_block_if_absent`. The block is `_DEFAULT_GCPAD_CONFIG`
  (dolphin.py:340-368): 27 lines starting `[GCPad1]` and
  `Device = XInput/0/Gamepad`, values wrapped in BACKTICKS (`` `Button A` ``),
  keys containing `/`, spaces and `-` (`Main Stick/Up`, `C-Stick/Left`,
  `Triggers/L-Analog`, `D-Pad/Up`), `Main Stick/Calibration = 100.00`,
  ending `Rumble/Motor = `Motor L` | `Motor R``, with exactly one trailing
  newline and NO leading newline. Transcribe it verbatim from
  dolphin.py:340-368 into a `const DEFAULT_GCPAD_CONFIG: &str`. Note those
  keys can never be matched by the narrow INI key regex — that is why this
  is an append-block writer, not a section writer.
  Result: `config_path` = `None`; `extras["gcpad_ini_path"]` = the target.

**Azahar** (azahar.py:55-216):

- Create `<emulator_dir>/user/` when it does not exist (dir-or-parent rule,
  swallow failure) — the portable marker.
- Candidates (using `emulator_path.parent()` UNCONDITIONALLY, with no
  is-dir check, unlike the mkdir above):
  `<parent>/user/config/qt-config.ini`, `<parent>/qt-config.ini`,
  `%APPDATA%/Azahar/qt-config.ini` (when `APPDATA` is set and non-blank; no
  platform check), `~/.config/Azahar/qt-config.ini`,
  `~/.var/app/org.azahar_emu.Azahar/config/Azahar/qt-config.ini`.
  Blank path → `EnsureResult::unchanged()`. Selection: first EXISTING, else
  `candidates[0]`.
- All keys are unconditional overwrites via `writers::azahar_section`, and
  every real key is preceded by its `<key>\default` companion written as an
  ORDINARY key (which is exactly why the widened regex exists).

  `[Renderer]`: `resolution_factor\default`=`false`, `resolution_factor`=`4`,
  `use_vsync\default`=`false`, `use_vsync`=`true`.
  `[Audio]`: `volume\default`=`false`, `volume`=`0.4`.
  `[UI]`, in order: `enable_discord_presence\default`=`false`,
  `enable_discord_presence`=`false`, `confirmClose\default`=`false`,
  `confirmClose`=`false`, `fullscreen\default`=`false`, `fullscreen`=`true`,
  `pauseWhenInBackground\default`=`false`, `pauseWhenInBackground`=`true`,
  `hideInactiveMouse\default`=`false`, `hideInactiveMouse`=`true`,
  `Shortcuts\Main%20Window\Fullscreen\KeySeq\default`=`false`,
  `Shortcuts\Main%20Window\Fullscreen\KeySeq`=`F1`,
  `Shortcuts\Main%20Window\Stop%20Emulation\KeySeq\default`=`false`,
  `Shortcuts\Main%20Window\Stop%20Emulation\KeySeq`=`Escape`.

  Use Rust RAW strings for every key containing a backslash, e.g.
  `r"Shortcuts\Main%20Window\Fullscreen\KeySeq"` — a single backslash, not
  an escape.
- Write when changed after `create_dir_all` on the parent; any I/O error →
  unchanged.

**Eden** (eden.py:111-280):

- Create `<emulator_dir>/user/` when missing, same rule as Azahar.
- Candidates: `<exe parent>/user/config/qt-config.ini`;
  `%APPDATA%/eden/config/qt-config.ini` when `APPDATA` is set and non-blank
  (inserted BETWEEN the portable and the XDG candidate);
  `<XDG_CONFIG_HOME>/eden/qt-config.ini`. Selection: first EXISTING, else
  the first.
- Writes through `writers::eden_annotated_section`, which GENERATES the
  `key\default=false` line rather than taking it as a desired key.
  **Format asymmetry to preserve exactly:** the annotation line has NO
  spaces (`enable_gamemode\default=false`); the value line HAS them
  (`enable_gamemode = false`).
- Keys, in order: `[UI]` `enable_discord_presence`=`false`,
  `confirmStop`=`2`, `fullscreen`=`true`, `firstStart`=`false`,
  `pauseWhenInBackground`=`true`, `enable_gamemode`=`true`,
  `theme`=`colorful_dark`, `check_for_updates`=`false`;
  `[WebService]` `enable_telemetry`=`false`;
  `[Audio]` `volume`=`40`, `muteWhenInBackground`=`true`;
  `[Renderer]` `scaling_filter`=`6`. All unconditional overwrites.
- The whole read/edit/write body is one fallible scope; any I/O error →
  unchanged.

- [ ] **Step 1: failing tests**, mirroring
  `tests/test_emulator_autoconfig_settings.py:544-1062`:
  - `dolphin_writes_both_ini_files` (`:544`)
  - `dolphin_creates_portable_txt` / `..._does_not_overwrite_it` (`:568`, `:580`)
  - `dolphin_uses_candidate_zero_when_a_path_is_given`
  - `dolphin_skip_ipl_uses_the_first_existing_candidate` (`:609`-`:682`)
  - `dolphin_skip_ipl_and_settings_can_target_different_files` — the
    divergence, asserted directly
  - `dolphin_gcpad_appends_the_block_when_absent` (`:690`)
  - `dolphin_gcpad_skips_when_the_header_exists_case_insensitively`
    (`:767`) — seed `[gcpad1]`
  - `dolphin_gcpad_adds_a_newline_before_appending_only_when_missing`
  - `dolphin_gcpad_block_matches_the_reference_byte_for_byte` — the
    written tail equals `DEFAULT_GCPAD_CONFIG`
  - `dolphin_gcpad_falls_back_next_to_an_existing_dolphin_ini`
  - `azahar_creates_the_user_directory` / `..._does_not_recreate_it`
    (`:864`, `:876`)
  - `azahar_writes_companion_keys_without_duplication` (`:848`) — run
    twice, assert each `\default` key appears exactly once
  - `azahar_manages_the_shortcut_keys_with_backslashes_and_percent`
  - `azahar_is_idempotent`
  - `eden_annotation_format_has_no_spaces_and_the_value_line_does` (`:948`)
  - `eden_writes_confirm_stop_two` (`:985`)
  - `eden_overwrites_an_existing_audio_volume` (`:1062`)
  - `eden_rewrites_a_malformed_existing_annotation_line`
  - `eden_is_idempotent`
  - `eden_windows_candidate_sits_between_portable_and_xdg` — with `APPDATA`
    set inside the env guard
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: dolphin, azahar and eden settings writers`

### Task 7: `autoconfig/ppsspp.rs` + `cemu.rs` + `xemu.rs` + `redream.rs`

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/ppsspp.rs`, `cemu.rs`, `xemu.rs`, `redream.rs`
- Modify: `autoconfig/mod.rs`

**Interfaces (Produces):**
```rust
// ppsspp.rs
pub fn ensure_settings(emulator_path: &str, ra: Option<&crate::autoconfig::RaCredentials>) -> EnsureResult;
pub fn ensure_ra_credentials(emulator_path: &str, ra: &crate::autoconfig::RaCredentials) -> EnsureResult;
// cemu.rs
pub fn ensure_settings(emulator_path: &str) -> EnsureResult;
pub fn ensure_controller_config(emulator_path: &str) -> EnsureResult;
pub fn settings_path_candidates(emulator_path: &str) -> Vec<PathBuf>;
// xemu.rs
pub fn ensure_settings(emulator_path: &str) -> EnsureResult;
pub fn missing_bios_files(emulator_path: &str) -> Vec<&'static str>;
pub fn default_base_root() -> PathBuf;
// redream.rs
pub fn ensure_settings(emulator_path: &str) -> EnsureResult;
```

**PPSSPP** (ppsspp.py:6-166):

- Blank path → `EnsureResult::unchanged()`. `emulator_dir` = the path when
  it is a directory, else its parent — **no existence check at all**.
- **Delete `<emulator_dir>/installed.txt`** when it exists; the deletion
  itself counts as `changed`. This is what suppresses the first-run
  installer flow. Swallow an I/O error (and then do NOT set `changed`).
- The one config file is always
  `<emulator_dir>/memstick/PSP/SYSTEM/PPSSPP.INI` — exact casing:
  lowercase `memstick`, uppercase `PSP`, `SYSTEM`, `PPSSPP.INI`. No
  platform candidates.
- **D5 (binding):** the two reads the reference leaves unprotected
  (ppsspp.py:99 and :156) are wrapped like every other writer. An
  unreadable INI or `.dat` yields `changed = false` and never propagates.
- Sections, all unconditional overwrites via
  `writers::ini_overwrite_section`, in order:
  `[General]` `CheckForNewVersion`=`False`, `SaveStateSlotCount`=`3`;
  `[Graphics]` `InternalResolution`=`4`, `MultiSampleLevel`=`2`,
  `Smart2DTexFiltering`=`True`, `TexScalingLevel`=`4`, `TexScalingType`=`0`,
  `TexDeposterize`=`True`, `TexHardwareScaling`=`False`,
  `TextureShader`=`Off`, `HardwareTessellation`=`False`;
  `[Sound]` `GameVolume`=`25`, `AchievementVolume`=`40`;
  `[Theme]` `ThemeName`=`Slate Forest` (a space, written unquoted).
- `[Achievements]`, only when BOTH RA fields are non-blank, in order:
  `AchievementsEnable`=`True`, `AchievementsUserName`=<user>,
  `AchievementsToken`=<token>, `AchievementsChallengeMode`=`False`,
  `AchievementsLeaderboardTrackerPos`=`3`,
  `AchievementsLeaderboardStartedOrFailedPos`=`3`,
  `AchievementsLeaderboardSubmittedPos`=`3`,
  `AchievementsProgressPos`=`3`, `AchievementsChallengePos`=`3`,
  `AchievementsUnlockedPos`=`4`. (Five position keys at `3`, the last at
  `4`.)
- Parent directories are created LAZILY, only when a write is needed.
- The RA token file `<...>/PSP/SYSTEM/ppsspp_retroachievements.dat` is
  written only when both fields are non-blank AND the existing trimmed
  contents differ from the token. Content is the bare trimmed token with
  **no trailing newline**. Result: `extras["ra_token_path"]`.
- Result `config_path` = the INI path.
- `ensure_ra_credentials` (D2): the `[Achievements]` block plus the `.dat`
  file only; it must NOT delete `installed.txt` and must NOT write
  `[General]`/`[Graphics]`/`[Sound]`/`[Theme]`.

**Cemu** (cemu.py:274-358):

- Blank or non-string path → unchanged. `emulator_dir` from the EXPANDED
  path (dir-or-parent). Create `<emulator_dir>/portable/` unconditionally
  before any file check. Target `<emulator_dir>/portable/settings.xml`.
- **Create-from-template branch:** when the target does not exist, write
  `DEFAULT_CEMU_SETTINGS_XML` and return `changed = true` IMMEDIATELY — the
  six forced elements are already baked into the template.
  `DEFAULT_CEMU_SETTINGS_XML` is cemu.py:115-237: first line
  `<?xml version="1.0" encoding="UTF-8"?>` (uppercase `UTF-8`), last line
  `</content>`, one trailing newline. Transcribe verbatim into a `const`.
- Otherwise parse the XML. Empty-after-trim content is an error. The root
  is the document element when its tag is `content`, else the first
  `.//content` descendant; neither → error. Six forced elements, in order,
  each created as a sub-element when missing and otherwise set only when
  the text differs: `use_discord_presence`=`false`, `check_update`=`false`,
  `receive_untested_updates`=`false`, `gp_download`=`true`,
  `fullscreen`=`false`, `window_maximized`=`true`. Everything else in the
  document is preserved.
- On change, write `<?xml version="1.0" encoding="utf-8"?>\n` (LOWERCASE
  `utf-8` here, unlike the template) followed by the serialized root, with
  no added trailing newline.
- **Every** failure — parse error and I/O alike — yields
  `EnsureResult::unchanged()` (the reference's bare `except Exception`).
- Rust has no ElementTree. Use a minimal hand-written XML edit rather than
  a new dependency: locate `<tag>...</tag>` inside the root element by
  regex on the raw text and replace the inner text, appending
  `<tag>value</tag>` before `</content>` when absent, preserving all other
  bytes. This keeps "everything else in the document is preserved" literal
  (byte-preserving), which is STRONGER than ElementTree's reserialization —
  record it as deviation D11 in Task 13.
- `ensure_controller_config`: target
  `<emulator_dir>/portable/controllerProfiles/controller0.xml`, using the
  path WITHOUT `~` expansion (cemu.py:339 — parity). Exists → no write,
  `changed = false`, `extras["profile_path"]` still set. Otherwise write
  `DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE` on windows
  (cemu.py:13-62, `<api>XInput</api>`, type `Wii U Pro Controller`) and
  `DEFAULT_CEMU_SDL_CONTROLLER_PROFILE` everywhere else
  (cemu.py:64-113, `<api>SDLController</api>`); both carry 24
  `<entry><mapping>N</mapping><button>M</button></entry>` rows.
  Transcribe both verbatim into `const`s.

**Xemu** (xemu.py:184-335):

- Target: `<emulator_dir>/xemu.toml` when a non-blank path is given
  (trimmed, expanded, dir-or-parent; **no existence check on the file**),
  else `<default_base_root>/xemu.toml`. `default_base_root()` =
  `%APPDATA%/xemu/xemu` on windows,
  `~/Library/Application Support/xemu/xemu` on macos,
  `$XDG_DATA_HOME/xemu/xemu` else `~/.local/share/xemu/xemu` elsewhere.
- Add-only throughout via `writers::toml_add_only_section`. Sections in
  order: `[general] show_welcome`=`false`; `[misc] check_for_updates`=`false`;
  `[display] vsync`=`true`; `[display.window] fullscreen_on_startup`=`true`;
  `[display.quality] surface_scale`=`2`; `[audio] volume_limit`=`0.4`;
  `[input.bindings] port1_driver`=`"usb-xbox-gamepad"` — **the value string
  INCLUDES the double quotes**.
- `[sys.files]`, with `base_dir` = the emulator dir when there is one, else
  `default_base_root()`; each value is the absolute path wrapped in SINGLE
  quotes with no escaping: `bootrom_path`=`'<base>/mcpx_1.0.bin'`,
  `flashrom_path`=`'<base>/complex_4627.bin'`,
  `hdd_path`=`'<base>/xbox_hdd.qcow2'`,
  `eeprom_path`=`'<base>/eeprom.bin'`.
- `changed` is a single accumulator OR-folded over all eight section calls
  (doc 05 open question; the Python's mid-sequence rebinding is recovered
  and observably identical). Every section call runs unconditionally and
  chains the content.
- Parent directory creation is LAZY, only before a write. Any I/O error →
  unchanged.
- `missing_bios_files` returns, in order, the subset of
  `["mcpx_1.0.bin", "complex_4627.bin", "xbox_hdd.qcow2"]` that does not
  exist under `base_dir`. **`eeprom.bin` is deliberately NOT required**
  even though the writer writes an `eeprom_path`.

**Redream** (redream.py:154-204):

- `config_path` comes from the reader (Task 9) called with an empty launch
  template; blank → unchanged. Until Task 9 lands, implement the small
  portable/default-root resolution here and have Task 9 re-export it rather
  than duplicating it: the emulator directory is the data root when it
  contains any of `redream.cfg`, `flash.bin`, `vmu0.bin`..`vmu3.bin`, or any
  `*.sav`/`*.png`; then the platform default
  (`~/Library/Application Support/redream` on macos, else
  `$XDG_DATA_HOME/redream` or `~/.local/share/redream`) but only when it
  exists or the host is macos; then the emulator directory as a fallback.
  Config path = `<data root>/redream.cfg`.
- Parse: split lines, no comment or section handling; for each line
  containing `=`, split on the FIRST `=`, trim both halves into a map.
- Managed keys: `mode`=`fullscreen`, `volume`=`40`.
- When both already hold those values, return `changed = false` with
  **no write at all** — the file stays byte-identical.
- Otherwise rewrite: each line whose pre-`=` trimmed key is managed becomes
  `{key}={value}` (NO spaces, original spacing dropped, and every duplicate
  is rewritten — there is no dedupe here, unlike the INI families); every
  other line, comments included, is preserved verbatim; unwritten managed
  keys are appended in order. Output is `lines.join("\n") + "\n"` with **no
  `trim_end()`** — pre-existing trailing blank lines survive.

- [ ] **Step 1: failing tests**, mirroring
  `tests/test_emulator_autoconfig_settings.py:1101-1755`:
  - `ppsspp_deletes_installed_txt_and_reports_changed` (`:1259`)
  - `ppsspp_writes_the_ini_at_the_memstick_path_with_exact_casing`
  - `ppsspp_writes_all_four_base_sections`
  - `ppsspp_achievements_block_requires_both_fields` (`:1301`)
  - `ppsspp_writes_the_ra_token_dat_without_a_trailing_newline` (`:1333`)
  - `ppsspp_skips_the_dat_write_when_the_token_is_unchanged`
  - `ppsspp_unreadable_ini_yields_changed_false` (D5) — make the INI
    unreadable (chmod 0o000 on unix, `#[cfg(unix)]`-gated) and assert no
    panic and `changed == false`
  - `ppsspp_is_idempotent`
  - `ppsspp_ensure_ra_credentials_does_not_delete_installed_txt`
  - `cemu_creates_the_default_settings_xml_when_missing` (`:1515`)
  - `cemu_enforces_the_six_forced_values_on_an_existing_file`
  - `cemu_preserves_unmanaged_settings` (`:1650`) — a sentinel element
    survives byte-for-byte
  - `cemu_no_op_when_all_six_already_match` (`:1755`)
  - `cemu_malformed_xml_yields_unchanged`
  - `cemu_controller_profile_is_written_once_and_never_overwritten`
  - `xemu_add_only_per_key` (`:1101`-`:1259`) — seed each managed key with
    a different value in turn and assert it survives
  - `xemu_writes_all_eight_sections_on_a_fresh_file`
  - `xemu_input_driver_value_keeps_its_double_quotes`
  - `xemu_sys_files_paths_are_single_quoted_absolute`
  - `xemu_is_idempotent`
  - `xemu_missing_bios_files_excludes_eeprom` (`:2467`)
  - `redream_writes_mode_and_volume` (`:1384`)
  - `redream_is_idempotent_with_no_write` (`:1368`) — assert the file's
    bytes AND mtime are unchanged
  - `redream_preserves_comments_and_trailing_blank_lines`
  - `redream_rewrites_every_duplicate_managed_key`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: ppsspp, cemu, xemu and redream settings writers`

### Task 8: `autoconfig/readers.rs` part 1 — PCSX2, DuckStation, Dolphin, RPCS3

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/readers.rs`
- Modify: `autoconfig/mod.rs` (`pub mod readers;`)

Nothing consumes these yet: they are ported and unit-tested now, and
milestone 6 (cloud saves) is their only caller. Every reader returns a
FULLY POPULATED struct — unresolvable values are empty strings, never
`Option` (doc 05 invariant 6). Every override function takes the same three
arguments and every path list is deduped CASE-INSENSITIVELY (doc 05
invariant 7).

**Interfaces (Produces):**
```rust
/// The launch template already split into arguments, or an empty vec when
/// the reference's splitter would have raised (rpcs3.py:40).
pub type Args<'a> = &'a [String];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pcsx2Settings { pub config_path: String, pub data_root: String,
    pub memory_cards: String, pub savestates: String,
    pub slot1_filename: String, pub slot2_filename: String }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuckstationSettings { pub config_path: String, pub directory: String,
    pub card1_type: String, pub card2_type: String, pub use_playlist_title: bool }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DolphinSettings { pub config_path: String, pub user_root: String,
    pub gc_root: String, pub wii_root: String, pub state_saves: String,
    pub memcard_a_path: String, pub memcard_b_path: String,
    pub gci_folder_a_path: String, pub gci_folder_b_path: String,
    pub gci_folder_a_override: String, pub gci_folder_b_override: String }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rpcs3Settings { pub config_path: String, pub persistent_settings_path: String,
    pub data_root: String, pub dev_hdd0: String, pub current_user: String }

pub fn pcsx2_directory_settings(path: &str, args: Args) -> Pcsx2Settings;
pub fn pcsx2_data_root_candidates(path: &str, args: Args) -> Vec<PathBuf>;
pub fn pcsx2_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn pcsx2_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn duckstation_memory_card_settings(path: &str) -> DuckstationSettings;
pub fn dolphin_directory_settings(path: &str, args: Args) -> DolphinSettings;
pub fn dolphin_user_root_candidates(path: &str, args: Args) -> Vec<PathBuf>;
pub fn dolphin_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn dolphin_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn rpcs3_directory_settings(path: &str, args: Args) -> Rpcs3Settings;
pub fn rpcs3_data_root_candidates(path: &str, args: Args) -> Vec<PathBuf>;
pub fn rpcs3_data_root(path: &str) -> Option<PathBuf>;
pub fn rpcs3_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn ps3_vfs_dev_hdd0_path(path: &str, args: Args, ps3_library: &str) -> Option<PathBuf>;
pub fn ps3_vfs_games_path(path: &str, args: Args, ps3_library: &str) -> Option<PathBuf>;

// shared helpers, used again in Task 9
pub(crate) fn consume_arg_value(args: Args, index: usize) -> Option<String>; // xemu.py:36
pub(crate) fn unique_paths(paths: Vec<PathBuf>) -> Vec<PathBuf>;             // case-insensitive
pub(crate) fn resolve_against(base: &Path, value: &str) -> PathBuf;          // expandvars + ~ + join
```

**Authoritative tables.** Port each reader against doc 05's per-emulator
section and the Python line range it cites — those are the contract, and
they are too long to restate here:

| Reader | doc 05 section | Python |
|---|---|---|
| PCSX2 data roots / overrides | "PCSX2 — Portable/data-root resolution", "Save/state overrides" | pcsx2.py:420-612 |
| DuckStation memory cards | "DuckStation" steps 2-3 | duckstation.py:10, :145-196 |
| Dolphin user root, saves, states | "Dolphin — User-root resolution", "Save overrides" | dolphin.py:53-157, :435-560 |
| RPCS3 data roots, VFS, user, saves | "RPCS3 — Data-root candidate order", "VFS path resolution", "Active user", "Save overrides" | rpcs3.py:13-110, :285-305, :469-531, :605-745 |

**Subtle traps that must be stated in code comments and covered by tests:**

- **PCSX2 portable detection** (pcsx2.py:420): portable when `-portable`
  appears in the launch template, OR `portable.ini`/`portable.txt` exists.
  When `portable.txt` contains TEXT, that text is a subdirectory suffix
  under the emulator directory. Candidate order is portable roots, then
  user roots (Windows Documents via the Shell API — on non-Windows this
  resolver returns `None`; `$OneDrive/Documents/PCSX2`,
  `$USERPROFILE/Documents/PCSX2`, `$HOME/Documents/PCSX2`,
  `~/Documents/PCSX2`, `~/.config/PCSX2`,
  `~/Library/Application Support/PCSX2`, `$XDG_CONFIG_HOME/PCSX2`), then
  the plain emulator directory.
- **PCSX2 save overrides return FILES FIRST**:
  `[<memcards>/<slot1>, <memcards>/<slot2>, <memcards>]`, slots defaulting
  to `Mcd001.ps2` / `Mcd002.ps2` and overridable from `[MemoryCards]`.
  State overrides are the single `savestates` directory. Relative INI
  values resolve against the data root; defaults are `memcards` / `sstates`.
- **Dolphin memcard permutations**: for each configured
  `MemcardAPath`/`MemcardBPath` (when set), emit it and then EVERY
  `MemoryCard{A,B}.{USA,JPN,JAP,EUR,DEV}{,.59,.123,.251,.507,.1019,.2043}.raw`
  permutation under `<user_root>/GC` — 5 regions × 7 size suffixes each.
  Then the GCI overrides, then the GCI paths expanded to sibling region
  directories (when the configured directory's own name is already a region
  name, its PARENT is the base) plus the `<GC>/<region>/Card {A,B}`
  defaults, then `<wii_root>/title` and the six Wii title groups
  `00010000`, `00010001`, `00010002`, `00010004`, `00010005`, `00010008`.
  `wii_root` comes from `[General] NANDRootPath`, defaulting to
  `<user_root>/Wii`; states are `<user_root>/StateSaves`.
- **RPCS3 `$RPCS3_CONFIG_DIR` insertion**: it goes in at index 1 of the
  platform list, so the effective order is `<exe_dir>/portable` (when it
  exists) → `$RPCS3_CONFIG_DIR` → `<exe_dir>` → `$XDG_CONFIG_HOME/rpcs3` →
  `~/Library/Application Support/rpcs3`; with no `portable/` the order
  starts `<exe_dir>` → `$RPCS3_CONFIG_DIR`.
- **RPCS3 VFS scalars**: `vfs.yml` is looked for at `<root>/config/vfs.yml`
  then `<root>/vfs.yml`. Keys may be quoted; the values `""`, `{}`, `[]`,
  `|`, `>` all mean empty; an unquoted trailing `# comment` is stripped.
  `$(EmulatorDir)` expands to the resolved base root in forward-slash form
  WITH a trailing slash, then env-var and `~` expansion run, and a relative
  result is joined onto the base root. `/dev_hdd0/` defaults to
  `$(EmulatorDir)dev_hdd0/`. `ps3_vfs_*_path` fall back to
  `<ps3_library>/.vfs/dev_hdd0` and `<ps3_library>/.vfs/games`, and return
  `None` when the library path is also blank.
- **RPCS3 active user**: `--user-id <id>` or `--user-id=<id>` from the
  launch template, else `[Users] active_user` in
  `<root>/GuiConfigs/persistent_settings.dat` (standard INI parse), else
  `00000001`. A user id is valid only when it is exactly 8 DIGITS and not
  `00000000`.
- **RPCS3 save overrides**: the current user's
  `<dev_hdd0>/home/<user>/savedata` first, then every EXISTING valid 8-digit
  user directory in NAME order, then `<dev_hdd0>/home/00000001/savedata` as
  a guaranteed tail entry; all resolved and deduped case-insensitively.
  RPCS3 has no state-path override function.
- **`consume_arg_value`** rejoins tokens until a closing quote is found —
  it must tolerate a splitter that produced fragments (doc 05 invariant 9).

- [ ] **Step 1: failing tests**, ported from the
  `tests/test_emulator_profiles.py:13-63` oracles plus the specific cases
  below (all in tempdirs, env-guarded):
  - `pcsx2_portable_detected_from_portable_ini` /
    `..._from_a_portable_launch_flag` / `..._suffix_from_portable_txt_text`
  - `pcsx2_candidate_order_is_portable_then_user_then_emulator_dir`
  - `pcsx2_save_overrides_list_slot_files_before_the_directory`
  - `pcsx2_slot_filenames_come_from_the_ini_when_set`
  - `pcsx2_relative_ini_values_resolve_against_the_data_root`
  - `pcsx2_state_overrides_are_the_single_savestates_directory`
  - `duckstation_settings_stop_at_the_first_candidate_with_a_memorycards_key`
  - `duckstation_settings_defaults_when_nothing_parses` — asserts
    `memcards`, `PerGameTitle`, `None`, `use_playlist_title == true`
  - `dolphin_user_root_prefers_a_launch_user_flag` — `-u`, `--user`,
    `--user=` forms
  - `dolphin_user_root_uses_exe_dir_user_when_portable_txt_exists`
  - `dolphin_save_overrides_emit_all_thirty_five_memcard_permutations` —
    asserts the count and two sample names
  - `dolphin_gci_region_directory_uses_the_parent_when_already_a_region`
  - `dolphin_save_overrides_end_with_the_six_wii_title_groups`
  - `dolphin_wii_root_defaults_to_user_root_wii`
  - `rpcs3_config_dir_env_is_inserted_at_index_one`
  - `rpcs3_candidates_start_with_exe_dir_when_portable_is_absent`
  - `rpcs3_vfs_expands_emulator_dir_token_with_a_trailing_slash`
  - `rpcs3_vfs_treats_empty_yaml_scalars_as_unset` — table over
    `""`, `{}`, `[]`, `|`, `>`
  - `rpcs3_vfs_strips_an_unquoted_trailing_comment`
  - `rpcs3_current_user_from_launch_args_then_persistent_settings_then_default`
  - `rpcs3_user_id_must_be_eight_digits_and_not_all_zero`
  - `rpcs3_save_overrides_put_the_current_user_first_and_00000001_last`
  - `ps3_vfs_paths_fall_back_to_the_library_dot_vfs_directories`
  - `ps3_vfs_paths_are_none_when_the_library_is_blank`
  - `consume_arg_value_rejoins_split_quoted_fragments`
  - `unique_paths_dedupes_case_insensitively_keeping_the_first`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: pcsx2, duckstation, dolphin and rpcs3 save-path readers`

### Task 9: `autoconfig/readers.rs` part 2 — the remaining readers, Vita3K, Flycast VMU

**Files:**
- Modify: `rewrite/crates/grid-core/src/autoconfig/readers.rs`

Same contract as Task 8: fully populated results, case-insensitive path
dedupe, launch arguments beat config values, relative config values resolve
against the data root after env-var and `~` expansion.

**Interfaces (Produces):**
```rust
pub struct AzaharSettings { pub config_path: String, pub user_root: String, pub nand_root: String,
    pub sdmc_root: String, pub states_root: String, pub use_custom_storage: bool, pub use_virtual_sd: bool }
pub struct EdenSettings   { /* same field set as AzaharSettings */ }
pub struct CemuSettings   { pub config_path: String, pub mlc_path: String }
pub struct XemuSettings   { pub config_path: String, pub base_path: String, pub hdd_path: String, pub eeprom_path: String }
pub struct XeniaSettings  { pub variant: String, pub config_path: String, pub storage_root: String,
    pub content_root: String, pub cache_root: String, pub portable: bool }
pub struct RedreamSettings{ pub config_path: String, pub data_root: String, pub portable: bool }
pub struct FbneoSettings  { pub config_path: String, pub base_path: String, pub eeprom_path: String,
    pub memcard_path: String, pub hiscore_path: String, pub hdd_path: String, pub state_path: String }
pub struct MameSettings   { pub ini_path: String, pub base_path: String, pub cfg_directory: String,
    pub nvram_directory: String, pub memcard_directory: String, pub diff_directory: String, pub state_directory: String }
pub struct Pico8Settings  { pub config_path: String, pub user_root: String, pub carts_root: String,
    pub cdata_root: String, pub cstore_root: String, pub backup_root: String, pub desktop_path: String }

pub fn azahar_directory_settings(path: &str, args: Args) -> AzaharSettings;
pub fn azahar_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn azahar_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn eden_directory_settings(path: &str, args: Args) -> EdenSettings;
pub fn eden_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn eden_keys_path(path: &str) -> Option<PathBuf>;
pub fn eden_has_firmware(path: &str) -> bool;
pub fn cemu_directory_settings(path: &str, args: Args) -> CemuSettings;
pub fn cemu_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn xemu_directory_settings(path: &str, args: Args) -> XemuSettings;
pub fn xemu_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn xenia_directory_settings(path: &str, args: Args) -> XeniaSettings;
pub fn xenia_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn xenia_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>; // always empty
pub fn redream_directory_settings(path: &str, args: Args) -> RedreamSettings;
pub fn redream_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn redream_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn fbneo_directory_settings(path: &str, args: Args) -> FbneoSettings;
pub fn fbneo_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn fbneo_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn mame_directory_settings(path: &str, args: Args) -> MameSettings;
pub fn mame_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn mame_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn pico8_directory_settings(path: &str, args: Args) -> Pico8Settings;
pub fn pico8_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
pub fn vita3k_pref_path(path: &str) -> Option<PathBuf>;
pub fn vita3k_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf>;
/// Newest `vmu[0-3]*.bin` per slot, ordered slot 0→3 (retroarch.py:625-650).
pub fn flycast_vmu_file_candidates(directories: &[PathBuf]) -> Vec<PathBuf>;
```

Port each against its doc 05 section and Python line range — those tables
are the contract:

| Reader | doc 05 section | Python |
|---|---|---|
| Azahar | "Azahar — User-root resolution / Storage semantics / Save overrides" | azahar.py:10, :219-415 |
| Eden | "Eden — User-root resolution / probes / Save overrides" | eden.py:294-472 |
| Cemu | "Cemu — Settings candidates / Save overrides" | cemu.py:252-429 |
| Xemu | "Xemu — Base-path resolution / Save overrides" | xemu.py:36-137, :340-440 |
| Xenia | "Xenia — Variant and portable detection / Save overrides" | xenia.py:91-489 |
| Redream | "Redream — Portable detection / Save overrides" | redream.py:30-152 |
| FBNeo | "FBNeo" | fbneo.py:35-154 |
| MAME | "MAME" | mame.py:57-239 |
| Pico-8 | "Pico-8" | pico8.py:61-228 |
| Vita3K | "Vita3K" | vita3k.py:9-95 |

**Subtle traps that must be stated in code comments and covered by tests:**

- **Azahar** `use_virtual_sd` defaults to **true**, `use_custom_storage` to
  false. SDMC enumeration is SKIPPED when `use_virtual_sd == "false"`.
  Title groups are `00040000`, `00040002`, `0004000e`, `0004008c`,
  `00048004`; only EXISTING directories are kept; when nothing exists, fall
  back to the all-zero 32-character id path
  `<sdmc>/Nintendo 3DS/<0*32>/<0*32>/title/<group>`. NAND containers are
  `<nand>/title` plus `<nand>/<child>/title` with groups `00040010`,
  `00040030`, falling back to `<nand>/<0*32>/title/<group>` and
  `<nand>/title/<group>`. States are the single `<user_root>/states`.
- **Eden** user-root resolution additionally probes alternate application
  names: the executable stem in three casings, then `Eden`, `eden`, `yuzu`,
  `Yuzu`, `suyu`, `Suyu`, deduped case-insensitively. Save overrides
  enumerate `<nand>/user/save/0000000000000000/<user-dir>` keeping only
  user directories that contain at least one subdirectory; when none
  qualify, return the parent `<nand>/user/save/0000000000000000` itself.
  Eden has no state-path override function.
- **Cemu** MLC paths come from `-m`/`--mlc`/`--mlc=`/`-m=` in the launch
  template FIRST, then the `<mlc_path>` element in the settings XML. Each
  becomes a save root: already ending in `usr/save` (either slash style,
  case-insensitively) → used as-is with trailing separators trimmed; else
  `usr/save` is appended. No state-path override.
- **Xemu** TOML reading flattens dotted keys into synthetic sections
  (`a.b = x` inside `[sec]` becomes `sections["sec.a"]["b"]`) and expands an
  inline `files = { … }` table into a `<section>.files` pseudo-section.
  Base path: a `-config_path`/`--config-path` (and `=` forms) launch
  argument wins, and a bare DIRECTORY value gets `xemu.toml` appended; then
  the emulator directory when it contains `xemu.toml`, `xbox_hdd.qcow2` or
  `eeprom.bin`; then the platform default. Save overrides are
  `[hdd_path, eeprom_path]`, deduped; no state override.
- **Xenia** variant: `canary` when the path contains `xenia_canary`,
  `xenia-canary` or bare `canary`; `edge` for `xenia_edge`/`xenia-edge`;
  else `master`. Portable when `<emulator_dir>/portable.txt` exists or a
  `-portable`/`--portable` flag is present (optionally followed by a
  boolean token), and defaults to "canary on Windows". Cache directory is
  `cache_host` for canary/edge and `cache` for master. Config file names are
  probed in variant order, e.g. `xenia-canary.config.toml`,
  `xenia-canary-config.toml`, `xenia_canary.config.toml`,
  `xenia_canary-config.toml`, then the generic `xenia.config.toml`,
  `xenia-config.toml`. Launch overrides beat `[storage] content_root`
  /`cache_root`. Save enumeration: a first-level entry matching 16 hex
  characters is a XUID whose 8-hex children are titles; a first-level entry
  matching 8 hex characters is a title directly; within each title, the
  existing subset of `00000001`, `Headers/00000001`, `profile` is
  collected. `xenia_state_path_overrides` ALWAYS returns `[]`.
  **`apply_xenia_content_without_ui` (the STFS content installer,
  xenia.py:36-70) is OUT OF SCOPE** — the spec's scope bullets cover
  readers only. Record it as deviation D9 in Task 13.
- **Redream** save overrides are FILE paths: the existing subset of
  `vmu0.bin`..`vmu3.bin` in the data root. State overrides are
  `<data_root>/states` when it exists, then the data root itself.
- **FBNeo** config probing order is `<emulator_dir>/config/<exe stem>.ini`,
  then `config/fbneo.ini`, then `config/FinalBurn Neo.ini`. Format is
  whitespace-separated `key value` with `//`, `#`, `;` comments.
  `szAppEEPROMPath` (default `config/games`), `szAppHiscorePath` (default
  `support/hiscores`), `szAppHDDPath` (default `support/hdd`).
  `memcard_path` is ALWAYS `<emulator_dir>/config/memcards` and
  `state_path` ALWAYS `<emulator_dir>/savestates` — neither is
  configurable. With no path supplied the base is the process working
  directory. Save overrides emit eeprom, memcard, hiscore, hdd in that
  order; state overrides emit the state path.
- **MAME** searches `mame.ini` in the `-inipath` directories
  (semicolon-separated) when that option is present, else `<base>`,
  `<base>/ini`, `<base>/ini/presets`. Keys are lowercased; comments are
  `#`/`;`. Options: `cfg_directory` (`cfg`), `nvram_directory` (`nvram`),
  `memcard_directory` (`memcard`), `diff_directory` (`diff`),
  `state_directory` (`sta`). Argument parsing strips leading dashes,
  accepts `-opt value` and `-opt=value`, normalizes `-` to `_` in the
  option name, and REFUSES a following token that itself starts with `-`.
  Launch arguments win over the ini.
- **Pico-8** user-root candidates: a `-home`/`--home` launch argument; then
  `<emulator_dir>`, `<emulator_dir>/pico-8`, `<emulator_dir>/userdata` —
  each only when it contains `config.txt`, `cdata` or `cstore`; then
  `%APPDATA%/pico-8` on windows, `~/Library/Application Support/pico-8` on
  macos, or BOTH `~/.lexaloffle/pico-8` and `$XDG_DATA_HOME/pico-8` on
  linux. `config.txt` is whitespace-separated `key value` with `#`, `;`,
  `--` comments. `root_path` overrides the carts root (default `carts`),
  `desktop` overrides the desktop path (default `desktop`); `cdata`,
  `cstore` and `backup` are fixed subdirectories. Save overrides emit
  `cdata_root` then `cstore_root`; no state override.
- **Vita3K** pref path in strict priority: `<emulator_dir>/portable/` when
  it is a directory; then the `pref-path:` scalar in
  `<emulator_dir>/config.yml` with ONE matching quote pair stripped and `~`
  expanded; then the platform default
  (`~/.local/share/Vita3K/Vita3K` linux, `~/AppData/Roaming/Vita3K/Vita3K`
  windows, `~/Library/Application Support/Vita3K/Vita3K` macos). The file
  is read lossily (Python `errors="replace"`). Save overrides enumerate
  `<pref_path>/ux0/user/<NN>/savedata` for every EXISTING two-digit user
  directory in name order, and ALWAYS PREPEND user `00` when it is not
  already present — even when that directory does not exist. The launch
  arguments are accepted for signature uniformity and unused.
- **Flycast VMU**: non-recursive `*.bin` glob (case-SENSITIVE on Linux, so
  `VMU0.BIN` is invisible — parity, do not fix), name matched against
  `^vmu([0-3]).*\.bin$` case-INSENSITIVELY, newest per slot by mtime with a
  STRICT `>` (an exact tie keeps the first seen), returned ordered slot
  0→3 with absent slots omitted.

- [ ] **Step 1: failing tests**, ported from
  `tests/test_emulator_profiles.py:13-63`, `tests/test_vita3k.py` and
  `tests/test_flycast_vmu.py:49-92`. One `*_directory_settings_defaults`
  test per reader asserting a fully populated struct for a nonexistent
  path, plus:
  - `azahar_use_virtual_sd_defaults_true` /
    `azahar_sdmc_skipped_when_virtual_sd_is_false`
  - `azahar_falls_back_to_the_all_zero_id_path_when_nothing_exists`
  - `eden_probes_alternate_app_names_including_yuzu_and_suyu`
  - `eden_save_overrides_keep_only_user_dirs_with_children` and
    `eden_save_overrides_fall_back_to_the_parent`
  - `cemu_mlc_launch_flag_beats_the_settings_xml`
  - `cemu_save_root_appends_usr_save_only_when_absent`
  - `xemu_config_path_launch_flag_appends_xemu_toml_to_a_directory`
  - `xemu_toml_reader_flattens_dotted_keys_and_inline_files_table`
  - `xenia_variant_detection_table` — canary/edge/master cases
  - `xenia_save_overrides_walk_xuid_and_bare_title_directories`
  - `xenia_state_overrides_are_always_empty`
  - `redream_portable_detected_from_each_marker` — table over
    `redream.cfg`, `flash.bin`, `vmu0.bin`, `*.sav`, `*.png`
  - `redream_save_overrides_are_existing_vmu_files`
  - `fbneo_memcard_and_state_paths_are_not_configurable`
  - `fbneo_probes_the_three_config_names_in_order`
  - `mame_launch_args_beat_the_ini` and
    `mame_arg_parser_refuses_a_dash_prefixed_value`
  - `mame_inipath_is_semicolon_separated`
  - `pico8_user_root_requires_a_marker_file`
  - `pico8_root_path_and_desktop_overrides`
  - `vita3k_pref_path_priority` (`test_vita3k.py:21`-`:99`) — portable over
    config.yml, quoted values, `~` expansion, missing key, the three
    platform defaults
  - `vita3k_save_overrides_always_prepend_user_00` (`:111`, `:138`) and
    `vita3k_excludes_non_two_digit_directories` (`:147`)
  - `flycast_vmu_keeps_the_newest_per_slot_and_orders_zero_to_three`
    (`:49`-`:92`)
  - `flycast_vmu_rejects_non_vmu_names_and_a_missing_directory`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4: green + clippy/fmt/hygiene.**
- [ ] **Step 5: commit** `rewrite: remaining emulator save-path readers, vita3k and flycast vmu`

### Task 10: `autoconfig/entry.rs` — layer 1, plus the config and profile fields it needs

**Files:**
- Create: `rewrite/crates/grid-core/src/autoconfig/entry.rs`
- Modify: `rewrite/crates/grid-core/src/config.rs`
- Modify: `rewrite/crates/grid-core/src/launch/profiles.rs`
- Modify: `autoconfig/mod.rs`

**Config and profile fields (a spec gap, resolved here).** Layer 1 writes
eight entry fields and reads nine profile fields; the rewrite's structs
currently carry only three and six of them. Add the missing ones, following
the existing `source_*` serde pattern so an untouched config round-trips
byte-identically:

```rust
// config.rs — EmulatorEntry gains, after the source_* fields:
#[serde(default, skip_serializing_if = "String::is_empty")] pub save_strategy: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub ignore_files: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub ignore_extensions: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub save_paths: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub state_paths: String,

// launch/profiles.rs — RawProfile and EmulatorProfile gain:
pub save_strategy: String,            // raw; normalized by entry.rs
pub save_directories: Vec<String>,
pub state_directories: Vec<String>,
pub ignore_files: Vec<String>,
pub ignore_extensions: Vec<String>,
```
`EmulatorProfile`'s five new fields carry `#[serde(skip_serializing)]`,
like `source`, so the `ProfileSummary` IPC payload is unchanged.
`normalize_one` copies the four lists through with blanks stripped and
blank entries dropped; `save_strategy` is copied raw.

**`Config` already has the core-default map** as `retroarch_cores`
(Python's `default_retroarch_cores`). The spec's proposed `default_cores`
would duplicate it — do NOT add a second field; use `retroarch_cores`.
`Config` gains only:
```rust
#[serde(default)] pub retroachievements_username: String,   // plain, non-secret
```

**Interfaces (Produces):**
```rust
/// Profile list values flattened the way the reference does
/// (autoconfig.py:106): non-string and blank items dropped, each item
/// trimmed, joined with the literal separator ";\n".
pub fn multiline_profile_value(items: &[String]) -> String;

/// profiles.py:141-156. Aliases -> one of "auto" | "single_file" | "folder";
/// anything unrecognized becomes "auto".
pub fn normalize_save_strategy(value: &str) -> String;

/// selection.py:168-190. Reads the game's `title`, `platform`,
/// `rom_file_name` in that order. Returns "GameCube", "Wii" or "".
pub fn dolphin_variant_label(title: &str, platform: &str, rom_file_name: &str) -> String;

/// selection.py:192-212. `""` for anything but gamecube/wii.
pub fn dolphin_target_platforms(variant: &str, platforms: &[String]) -> Vec<String>;

/// autoconfig.py:90-103. `base_name` unchanged unless it case-folds to
/// "dolphin" and a variant is available, in which case
/// `format!("{base} ({variant})")`.
pub fn auto_configured_emulator_name(base_name: &str, variant: &str) -> String;

/// selection.py:157-165. Drops non-strings (n/a in Rust), anything whose
/// trimmed casefolded name starts with "windows", and the exact name
/// "emulators". Order preserved.
pub fn assignable_platforms(platforms: &[String]) -> Vec<String>;

/// The inputs layer 1 needs that grid-core cannot derive itself.
pub struct DefaultsContext<'a> {
    /// Assignable server platform names, already filtered.
    pub platforms: &'a [String],
    /// `(platform, emulator_name) -> installed compatible core ids`.
    /// Production passes a closure over `cores::installed_core_ids` +
    /// `cores::cores_for_platform`; tests pass a table.
    pub installed_cores: &'a dyn Fn(&str, &str) -> Vec<String>,
    /// `emulator_name -> is this RetroArch?` — the ported
    /// `_is_retroarch_emulator_name` predicate.
    pub is_retroarch: &'a dyn Fn(&str) -> bool,
}

/// autoconfig.py:346-402. Returns the updated (defaults, core_defaults).
pub fn assign_profile_platform_defaults(
    game: Option<&GameFacts>,
    emulator_name: &str,
    profile: &EmulatorProfile,
    defaults: &BTreeMap<String, String>,
    core_defaults: &BTreeMap<String, String>,
    ctx: &DefaultsContext,
) -> (BTreeMap<String, String>, BTreeMap<String, String>);

/// autoconfig.py:472-582. Returns the updated (emulators, defaults, core_defaults).
pub fn auto_configure_emulator_settings(
    game: Option<&GameFacts>,
    executable_path: &str,
    profile: &EmulatorProfile,
    emulators: &[EmulatorEntry],
    defaults: &BTreeMap<String, String>,
    core_defaults: &BTreeMap<String, String>,
    ctx: &DefaultsContext,
) -> (Vec<EmulatorEntry>, BTreeMap<String, String>, BTreeMap<String, String>);

/// autoconfig.py:228-270. Fills only blank fields; never touches `path`.
pub fn apply_manual_emulator_profile_defaults(entry: &EmulatorEntry, profile: &EmulatorProfile) -> EmulatorEntry;

/// emulator_ui_mixin.py:1790-1839. Re-runs the assignment for every
/// registered emulator with a matching profile; returns `true` when either
/// map changed (the caller saves only then).
pub fn backfill_missing_defaults(config: &mut Config, profiles: &[EmulatorProfile], ctx: &DefaultsContext) -> bool;

/// The game fields the Dolphin variant rule reads. `None` at the D1
/// trigger points — no game is in hand there, exactly like the reference's
/// backfill call (`game=None`), so the Dolphin variant branch is inert.
pub struct GameFacts { pub title: String, pub platform: String, pub rom_file_name: String }
```

**Pinned rules:**

- `auto_configure_emulator_settings`, step by step (autoconfig.py:489-582):
  1. `emulator_name = auto_configured_emulator_name(profile.name, variant)`
     with the profile-name fallback literal `"Emulator"`.
  2. `args_template` = the profile's trimmed args, falling back to `%rom%`
     when blank.
  3. `profile_save_strategy = normalize_save_strategy(profile.save_strategy)`
     with the default `"auto"`; the four flattened strings come from
     `ignore_files`, `ignore_extensions`, `save_directories` (→ the entry's
     `save_paths`), `state_directories` (→ `state_paths`).
  4. Find the first entry whose `name.trim().to_lowercase()` equals
     `emulator_name.to_lowercase()` — the LEFT side is trimmed, the right
     side is NOT. Port that asymmetry.
  5. **Existing entry:** the entry is REBUILT with exactly the eight fields;
     any other field on the old entry is DROPPED. `name` and `path` are
     always overwritten. `args` is replaced when
     `is_retroarch(emulator_name) || existing_args.trim().is_empty() ||
     existing_args.trim() == "%rom%"`, else kept as
     `existing_args.trim()` (trimmed even when preserved).
     `save_strategy` keeps a non-blank existing value but RE-NORMALIZES it;
     the four path/ignore fields keep a non-blank trimmed existing value
     and otherwise take the profile value.
     **Note:** the eight-field rebuild drops the `source_*` fields the
     rewrite added in milestone 4. Deviate here: preserve `source_id`,
     `source_provider`, `source_owner`, `source_repo` and
     `source_release_tag` from the existing entry, because they are the
     rewrite's own install provenance and the reference has no equivalent.
     Record it as deviation D12 in Task 13, and cover it with a test.
  6. **New entry:** appended with all profile values.
  7. Then `assign_profile_platform_defaults`.
- `assign_profile_platform_defaults` (autoconfig.py:346-402): target
  platforms = all assignable platforms when `all_platforms` is true, and
  for RetroArch further filtered to platforms with at least one INSTALLED
  compatible core; otherwise the keyword-matched platforms
  (`launch::profiles::platform_matches_keywords`, already ported). When a
  game is in hand AND the profile name case-folds to `dolphin`, a non-empty
  variant platform list REPLACES the keyword result (an empty one leaves it
  alone). Per platform: a blank current default is filled; a non-blank one
  is replaced ONLY when the incoming emulator is not RetroArch and the
  current default IS RetroArch. Core defaults: only when the incoming
  emulator is RetroArch, only for platforms whose default now case-folds
  equal to it, only when no core default is recorded yet, and the value is
  the FIRST installed compatible core.
- `apply_manual_emulator_profile_defaults` (autoconfig.py:228-270): copies
  the entry (unlisted fields survive, unlike layer 1's rebuild). `name` is
  filled only when blank. `args` is replaced when blank or exactly `%rom%`,
  taking the profile's trimmed args. `save_strategy` is replaced whenever
  the current value normalizes to `"auto"` — so `"auto"` counts as unset.
  The four blank-only fields map `ignore_files→ignore_files`,
  `ignore_extensions→ignore_extensions`, `save_paths→save_directories`,
  `state_paths→state_directories`, and the key is always written even when
  the profile value is `""`. `path` is NEVER touched.
- `backfill_missing_defaults` (D3): iterate `config.emulators`; skip blank
  names; resolve the profile via
  `launch::profiles::profile_for_entry(&e.name, &e.path, profiles)`; skip a
  miss; call `assign_profile_platform_defaults` with `game = None` and the
  maps as they stand AFTER the previous iteration (each iteration builds on
  the last); write both results back. Return whether either final map
  differs from its snapshot.
- `assignable_platforms`, `normalize_save_strategy` and
  `multiline_profile_value` are pure and belong in `entry.rs`, not in
  `launch/`.

- [ ] **Step 1: failing tests** (all pure, no tempdirs except the config
  round-trip):
  - `multiline_profile_value_joins_with_semicolon_newline_and_drops_blanks`
  - `normalize_save_strategy_alias_table` — every alias from
    profiles.py:141-156 plus an unknown value → `auto`
  - `dolphin_variant_gamecube_wii_and_wii_u_exclusion` — `Wii U` yields `""`
  - `dolphin_target_platforms_excludes_wii_u`
  - `auto_configured_name_appends_the_variant_only_for_dolphin`
  - `assignable_platforms_drops_windows_prefixed_and_emulators`
  - `auto_configure_creates_a_new_entry_with_all_profile_values`
  - `auto_configure_always_overwrites_name_and_path`
  - `auto_configure_replaces_args_for_retroarch` /
    `..._replaces_blank_args` / `..._replaces_the_bare_rom_placeholder` /
    `..._preserves_custom_args_trimmed`
  - `auto_configure_preserves_nonblank_fields_and_fills_blank_ones`
  - `auto_configure_preserves_the_source_fields_on_an_existing_entry` (D12)
  - `auto_configure_matches_an_existing_entry_case_insensitively`
  - `assign_defaults_fills_an_empty_platform_default`
  - `assign_defaults_lets_a_native_emulator_displace_retroarch`
  - `assign_defaults_never_lets_retroarch_displace_a_native_emulator`
  - `assign_defaults_filters_all_platforms_by_installed_cores_for_retroarch`
  - `assign_defaults_records_the_first_installed_core_only_when_unset`
  - `assign_defaults_dolphin_variant_platforms_replace_the_keyword_match`
  - `assign_defaults_keyword_match_survives_an_empty_variant_list`
  - `manual_defaults_fill_blank_fields_only_and_never_touch_path`
  - `manual_defaults_replace_auto_save_strategy`
  - `manual_defaults_replace_the_bare_rom_placeholder_args`
  - `backfill_is_a_no_op_when_nothing_is_missing` (mirrors
    `tests/test_emulator_autoconfig_settings.py:3009`)
  - `backfill_fills_a_platform_whose_cores_appeared_after_install` (`:3022`)
  - `backfill_accumulates_across_entries`
  - `config_round_trips_the_five_new_emulator_fields`
  - `config_without_the_new_fields_writes_no_new_keys`
  - `config_round_trips_retroachievements_username`
  - `profile_normalization_keeps_the_five_new_autoprofile_fields`
  - `profile_summary_serialization_is_unchanged` — asserts the serialized
    `EmulatorProfile` JSON has no new keys
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4:**
  `cargo test -p grid-core`, clippy, fmt, hygiene, plus `npm run check` in
  `rewrite/app` (the `EmulatorEntry` IPC type gains optional fields — update
  `api.ts`'s `EmulatorEntry` to carry them as optional so `save_emulator`
  round-trips an edited entry without dropping them).
- [ ] **Step 5: commit** `rewrite: entry autoconfig, platform and core defaults, defaults backfill`

### Task 11: orchestration — `sync_new_emulator`, predicates, and the two D1 call sites

**Files:**
- Modify: `rewrite/crates/grid-core/src/autoconfig/mod.rs`
- Modify: `rewrite/crates/grid-core/src/library/mod.rs`
- Modify: `rewrite/app/src-tauri/src/commands.rs`
- Modify: `rewrite/scripts/check_secret_hygiene.sh` — `RaCredentials::token()`
  is the crate's third `expose_secret()` call site, so `allowed_files` gains
  `crates/grid-core/src/autoconfig/mod.rs`. Without this the hygiene gate
  fails at the end of THIS task. Nothing else in the task's diff may call
  `expose_secret`.

**Interfaces (Produces):**
```rust
// autoconfig/mod.rs
/// The RetroAchievements pair. `token` never leaves this type except
/// through `token()`, which is the single `expose_secret()` call site in
/// the crate outside `secrets.rs`/`romm`.
#[derive(Clone)]
pub struct RaCredentials { username: String, token: secrecy::SecretString }
impl RaCredentials {
    pub fn new(username: String, token: secrecy::SecretString) -> Self;
    pub fn username(&self) -> &str;
    pub fn token(&self) -> &str;                   // expose_secret lives here
    /// `None` unless BOTH fields are non-blank after trimming — the gate
    /// every RA-aware writer applies (doc 05 invariant 5).
    pub fn usable(&self) -> Option<&Self>;
}
impl std::fmt::Debug for RaCredentials { /* username only, token redacted */ }

/// Everything `sync_new_emulator` needs, assembled by the caller.
pub struct SyncContext<'a> {
    pub config_path: &'a Path,
    /// Assignable server platform names (already filtered by
    /// `entry::assignable_platforms`). Empty when no session is connected,
    /// which makes the platform-defaults step a no-op — matching the
    /// reference's behavior with an empty platform list.
    pub platforms: &'a [String],
    /// `<library>/PlayStation 3`, or "" when no library path is set.
    pub ps3_library_path: String,
    pub ra: Option<RaCredentials>,
    pub profiles: &'a [EmulatorProfile],
}

/// What one sync produced. Every field is diagnostic only; a caller turns
/// a non-empty `warnings` into a user-visible warning line.
#[derive(Debug, Default)]
pub struct SyncReport { pub wrote: Vec<String>, pub warnings: Vec<String> }

/// The D1 entry point: entry autoconfig + defaults backfill + the native
/// `ensure_*` writers for ONE newly created emulator entry. Loads, mutates
/// and saves the config itself. Never returns `Err` for a writer failure —
/// those land in `report.warnings`.
pub fn sync_new_emulator(entry_name: &str, ctx: &SyncContext) -> Result<SyncReport, ConfigError>;

/// `_emulator_matches_tokens` (cloud_mixin.py:1349-1363): autoprofile token
/// matching on the entry first, then a plain case-folded SUBSTRING test of
/// each token against the entry name. So "My DuckStation build" matches
/// `duckstation` with no profile at all.
pub fn emulator_matches_tokens(entry: &EmulatorEntry, tokens: &[&str], profiles: &[EmulatorProfile]) -> bool;

pub fn is_retroarch(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool;   // token "retroarch"
pub fn is_duckstation(..) -> bool;  // "duckstation"
pub fn is_xemu(..) -> bool;         // "xemu"
pub fn is_pcsx2(..) -> bool;        // "pcsx2"
pub fn is_dolphin(..) -> bool;      // "dolphin"
pub fn is_azahar(..) -> bool;       // "azahar"
pub fn is_eden(..) -> bool;         // "eden"
pub fn is_rpcs3(..) -> bool;        // "rpcs3", OR-ed with the standalone name check (install_mixin.py:410)
pub fn is_ppsspp(..) -> bool;       // "ppsspp"
pub fn is_cemu(..) -> bool;         // "cemu"
pub fn is_redream(..) -> bool;      // "redream"

// library/mod.rs
impl InstallService {
    /// The assignable server platform names the last successful
    /// `list_platforms` saw. grid-core has no session of its own, so the
    /// app feeds this in; empty until it does.
    pub fn set_known_platforms(&self, platforms: Vec<String>);
    pub fn known_platforms(&self) -> Vec<String>;
    /// Supplies the RA credentials to the autoconfig hook. `None` until the
    /// app installs one (Task 12).
    pub fn set_ra_provider(&self, f: Arc<dyn Fn() -> Option<RaCredentials> + Send + Sync>);
}
```

**Orchestration order inside `sync_new_emulator`** (ports
`_ensure_emulator_sync_settings`, emulator_ui_mixin.py:365-440, minus the
session cache which D1 moots):

1. Load the config. Find the entry by name (exact); a miss returns an empty
   report.
2. Run entry autoconfig for that entry when a profile matches it
   (`profile_for_entry`), then `entry::backfill_missing_defaults` (D3), then
   save the config — once, before the writers run, so a writer failure
   cannot lose the entry work.
3. Trim the entry's `path`; **return immediately when it is blank**
   (doc 05 invariant 1).
4. Dispatch as a FLAT SEQUENCE OF INDEPENDENT `if`s, not a chain, so a name
   matching two predicates runs both writers, in exactly this order:

   | predicate | call | extra arguments |
   |---|---|---|
   | retroarch | `retroarch::ensure_settings` | `enable_fullscreen = true`, RA creds, `romm_username = config.username` |
   | duckstation | `duckstation::ensure_memory_card_settings` | `enable_fullscreen = true` |
   | xemu | `xemu::ensure_settings` | — |
   | pcsx2 | `pcsx2::ensure_settings` | `enable_fullscreen = true`, RA creds (NO `bios_directory` — D6) |
   | dolphin | `dolphin::ensure_settings` | — |
   | azahar | `azahar::ensure_settings` | — |
   | eden | `eden::ensure_settings` | — |
   | rpcs3 | `rpcs3::ensure_settings` | `ps3_library_path` (NO background firmware — D7) |
   | ppsspp | `ppsspp::ensure_settings` | RA creds |
   | cemu | `cemu::ensure_settings` then `cemu::ensure_controller_config` | — |
   | redream | `redream::ensure_settings` | — |

   The RomM username feeds ONLY RetroArch's netplay nickname. The PS3
   library path is `<library_path>/PlayStation 3`, or empty when no library
   path is configured.
5. Every writer runs inside a `catch_unwind`-free guard: writers already
   swallow I/O, so a writer that returns `config_path == None` while
   `changed == false` contributes a line to `report.warnings` naming the
   emulator and the writer; nothing aborts the remaining writers.

**Call site A — catalog install** (`library/mod.rs`,
`finalize_emulator`): after `write_emulator_entry` succeeds and BEFORE the
archive cleanup, build a `SyncContext` from `Config::load(&self.config_path)`,
`self.known_platforms()`, the config's library path, and the RA provider,
then call `sync_new_emulator(&job.profile_name, &ctx)`. A `Err(ConfigError)`
or a non-empty `report.warnings` appends one line to the existing finalize
`warning` string via `append_warning` — the install still reports
`Completed`, exactly like a failed archive delete. Autoconfig NEVER fails an
install.

**Call site B — manual add** (`commands.rs`, `save_emulator`): the command
already distinguishes an add from an edit (`original_name` naming no current
entry, or blank). For an ADD only: apply
`entry::apply_manual_emulator_profile_defaults` to the incoming entry before
`apply_save_emulator`, then, after the config is saved, call
`sync_new_emulator`. An edit does neither (D1). The command's `Result` is
unchanged: a sync warning is logged with `tracing::warn!` (no secret can
appear in it — the warnings name emulators and file paths only) and the
command still returns `Ok`.

**Platform capture:** `list_platforms` calls
`install.set_known_platforms(entry::assignable_platforms(&names))` after a
successful fetch, where `names` are the platform display names. This is the
only way grid-core learns the platform list; it is a spec gap resolved here.

- [ ] **Step 1: failing tests.**
  In `autoconfig/mod.rs` (tempdir config + a stub profile list):
  - `matches_tokens_by_profile_then_by_substring` — "My DuckStation build"
    with no profile matches `duckstation`
  - `is_rpcs3_matches_the_standalone_name_check`
  - `predicates_are_independent_so_two_can_fire` — an entry named
    "RetroArch + PPSSPP" runs both writers
  - `sync_returns_empty_for_an_unknown_entry_name`
  - `sync_returns_before_dispatch_for_a_blank_path`
  - `sync_runs_entry_autoconfig_then_backfill_then_writers` — a PCSX2 entry
    in a tempdir library: `portable.ini` and `inis/PCSX2.ini` exist, the
    config gained the profile's `save_paths`, and the PS2 platform default
    points at it
  - `sync_writer_failure_is_reported_as_a_warning_not_an_error` — point the
    entry at a path under a read-only directory (`#[cfg(unix)]`) and assert
    `Ok` with a non-empty `warnings`
  - `sync_passes_the_romm_username_only_to_retroarch`
  - `sync_passes_the_ps3_library_path_to_rpcs3`
  - `sync_omits_pcsx2_bios_directory` (D6) and
    `sync_starts_no_firmware_download` (D7)
  - `ra_credentials_debug_redacts_the_token`
  - `ra_credentials_usable_requires_both_fields`
  In `library/mod.rs` (extending the Task 6 milestone-4 integration tests):
  - `emulator_install_runs_autoconfig_after_writing_the_entry` — the
    wiremock github AppImage install, asserting `portable.ini` next to the
    installed executable
  - `autoconfig_failure_leaves_the_install_completed_with_a_warning`
  - `known_platforms_defaults_to_empty_and_round_trips`
  In `commands.rs`:
  - `manual_add_applies_profile_defaults_but_an_edit_does_not`
  - `manual_add_never_overwrites_the_typed_path`
- [ ] **Step 2: red.** — [ ] **Step 3: implement.** — [ ] **Step 4:**
  `cargo test -p grid-core`, `cargo build -p app` and
  `cargo build -p app --features e2e`, clippy, fmt, hygiene.
- [ ] **Step 5: commit** `rewrite: autoconfig orchestration wired to catalog install and manual add`

### Task 12: RetroAchievements credentials — keyring, IPC, the D2 fan-out, UI

**Files:**
- Modify: `rewrite/crates/grid-core/src/secrets.rs`
- Modify: `rewrite/crates/grid-core/src/autoconfig/mod.rs`
- Modify: `rewrite/app/src-tauri/src/commands.rs`, `src/lib.rs`
- Modify: `rewrite/app/src/lib/api.ts`, `Emulators.svelte`
- Create: `rewrite/app/src/lib/emulators/retroachievements.ts` (+ vitest)
- Modify: `rewrite/scripts/check_secret_hygiene.sh`

**Interfaces (Produces):**
```rust
// secrets.rs — a SECOND keyring item, alongside the RomM credential.
// The existing SERVICE constant is reused; only the account differs.
const RA_ACCOUNT: &str = "retroachievements-token";
pub trait RaTokenStore: Send + Sync {
    fn save(&self, token: &secrecy::SecretString) -> Result<(), SecretError>;
    fn load(&self) -> Result<Option<secrecy::SecretString>, SecretError>;
    fn clear(&self) -> Result<(), SecretError>;
}
impl RaTokenStore for KeyringStore { /* SERVICE + RA_ACCOUNT */ }
/// Second slot on the existing in-memory test store.
impl RaTokenStore for MemoryStore { /* independent of the Credential slot */ }

// autoconfig/mod.rs — D2
/// The RA-capable predicates, in dispatch order. DuckStation is NOT here:
/// it takes no credential parameters and writes only suppression keys
/// (doc 05 open question, ruled: follow the code).
pub fn ra_capable(entry: &EmulatorEntry, profiles: &[EmulatorProfile]) -> bool; // retroarch || pcsx2 || ppsspp

/// One-shot narrow write across every registered RA-capable entry.
/// Touches ONLY the RA credential keys — never the full managed set.
/// Returns `(emulator_name, changed)` per entry it ran, in config order.
pub fn fan_out_ra_credentials(
    config: &Config,
    profiles: &[EmulatorProfile],
    ra: &RaCredentials,
) -> Vec<(String, bool)>;
```

```rust
// commands.rs — three commands. The token NEVER appears in a return value.
#[tauri::command] pub async fn set_retroachievements_credentials(
    state: State<'_, AppState>, username: String, token: String,
) -> Result<Vec<RaFanOutRow>, String>;
#[tauri::command] pub async fn get_retroachievements_status(
    state: State<'_, AppState>,
) -> Result<RaStatus, String>;
#[tauri::command] pub async fn clear_retroachievements_credentials(
    state: State<'_, AppState>,
) -> Result<(), String>;

#[derive(Serialize)] pub struct RaStatus { pub username: String, pub token_present: bool }
#[derive(Serialize)] pub struct RaFanOutRow { pub emulator: String, pub changed: bool }
```

**Pinned rules:**

- `set_retroachievements_credentials`: wrap `token` in `SecretString`
  IMMEDIATELY (the plain `String` dies at the end of the scope, matching
  `connect`). A blank trimmed token CLEARS the keyring entry rather than
  storing an empty secret. The username is written to
  `Config.retroachievements_username` (plain, non-secret). Then run the D2
  fan-out and return one row per RA-capable registered entry. Errors are
  `Display`-mapped and are credential-free by construction.
- `get_retroachievements_status` returns the username from config and
  `token_present = store.load()?.is_some()`. **It must never return the
  token, its length, or a prefix.**
- `clear_retroachievements_credentials` clears the keyring entry and blanks
  `Config.retroachievements_username`. It writes NOTHING to any emulator
  config and scrubs NOTHING already written (parity — doc 05's
  "credentials are written but never removed" open question; the spec rules
  follow the code).
- `fan_out_ra_credentials` calls, per matching entry, exactly one of
  `retroarch::ensure_ra_credentials`, `pcsx2::ensure_ra_credentials`,
  `ppsspp::ensure_ra_credentials` — never the full `ensure_settings`. When
  the credentials are not `usable()` (either field blank) it returns an
  empty vec and writes nothing.
- The RA provider installed on `InstallService` (Task 11) reads the keyring
  and the config, so the D1 path picks credentials up automatically for a
  newly installed emulator.
- **Hygiene additions** in `check_secret_hygiene.sh`, next to the existing
  checks:
  - `RA_ACCOUNT`/`retroachievements-token` may appear only in
    `crates/grid-core/src/secrets.rs`.
  - No source file may serialize a field literally named
    `retroachievements_token` — grep `crates app/src-tauri/src app/src`
    for it and fail on any hit outside `secrets.rs`. (`Config` carries
    `retroachievements_username` only; the token has no config key.)
  - The existing long-bearer-string scan already covers fixtures; add
    `FAKE-RA-TOKEN-not-real` to the allowed-fakes list so tests can use a
    readable placeholder.

**Frontend.** `Emulators.svelte` gains a RetroAchievements block in the
panel's settings area, below the per-platform defaults, following the
existing label/input/error-line pattern:

- `data-testid="ra-username"` (text), `data-testid="ra-token"`
  (`type="password"`, never bound to a value read back from the backend —
  it starts empty on every mount and is write-only),
  `data-testid="ra-save"`, `data-testid="ra-clear"`,
  `data-testid="ra-status"` (renders `Not set` / `Set for <username>`),
  `data-testid="ra-error"`.
- On mount, `getRetroachievementsStatus()` fills the username field and the
  status line. Saving shows the fan-out result as a single line
  (`Updated: RetroArch, PPSSPP` — names only, from `RaFanOutRow`).
- `retroachievements.ts` exports the pure helpers the vitest covers:
  `canSubmit(username, token)` (both non-blank after trim),
  `statusLabel(status)` and `fanOutSummary(rows)` (the names of rows with
  `changed === true`, comma-joined; `No changes` when none).
- `api.ts` gains `RaStatus`, `RaFanOutRow` and the three invokes.

- [ ] **Step 1: failing tests.**
  Rust:
  - `ra_token_store_round_trips_independently_of_the_romm_credential` —
    saving a RomM credential and an RA token to `MemoryStore` leaves both
    readable and clearing one does not clear the other
  - `ra_capable_excludes_duckstation_and_dolphin`
  - `fan_out_writes_only_the_ra_keys` — a config with a RetroArch, a PCSX2
    and a Dolphin entry, each with a pre-existing config file carrying a
    sentinel unmanaged key: after the fan-out the sentinels are untouched,
    the two RA files carry the credentials, and the Dolphin file is
    byte-identical
  - `fan_out_is_a_no_op_when_either_field_is_blank`
  - `fan_out_reports_changed_false_on_a_second_run`
  - `ra_status_never_contains_the_token` — build the `RaStatus` for a
    stored token and assert `serde_json::to_string(&status)` does not
    contain the token text
  - `clear_blanks_the_username_and_writes_no_emulator_file` — mtimes of the
    emulator configs are unchanged
  Vitest (`retroachievements.test.ts`):
  - `canSubmit requires both fields`
  - `statusLabel renders set and unset states`
  - `fanOutSummary lists only changed emulators` / `reports no changes`
- [ ] **Step 2: red.** — [ ] **Step 3: implement** Rust, then the UI.
- [ ] **Step 4: green** — `cargo test -p grid-core`, both app builds,
  `npm run check` + `npm test`, clippy, fmt, and
  `bash scripts/check_secret_hygiene.sh` with the new guards.
- [ ] **Step 5: commit** `rewrite: retroachievements credential storage, ipc and narrow writer fan-out`

### Task 13: E2E extension, porting-doc deviations, README, hygiene

**Files:**
- Modify: `rewrite/e2e/specs/emulator-catalog.spec.ts`
- Modify: `rewrite/README.md`
- Modify: `docs/porting/05-emulator-autoconfig.md`

**E2E.** The `emulator-catalog` group already installs
`PCSX2 (Playstation 2)` as an AppImage into
`<library>/Emulators/PCSX2 (Playstation 2)-latest/`. D1 means the sync runs
right after that install, so the emulator directory is the AppImage's
parent. Extend the existing
`'installs PCSX2 from the catalog and marks it installed'` test (or add one
immediately after it, before the Play test, so the drawer state is
unambiguous) with condition-based waits only:

- `pcsx2Dir()` = `path.dirname(pcsx2Path())`.
- `browser.waitUntil` until `existsSync(path.join(pcsx2Dir(), 'portable.ini'))`
  — timeout `TRANSITION_TIMEOUT`, message
  `'autoconfig never created PCSX2 portable.ini after install'`.
- `browser.waitUntil` until
  `existsSync(path.join(pcsx2Dir(), 'inis', 'PCSX2.ini'))`.
- Read `inis/PCSX2.ini` and assert the managed keys landed:
  `expect(ini).toContain('[UI]')`,
  `toContain('SetupWizardIncomplete = false')`,
  `toContain('SettingsVersion = 1')`,
  `toContain('InhibitScreensaver = true')`,
  `toContain('[AutoUpdater]')`, `toContain('CheckAtStartup = false')`,
  `toContain('[EmuCore]')`, `toContain('EnableDiscordPresence = false')`,
  `toContain('[EmuCore/GS]')`, `toContain('pcrtc_antiblur = true')`,
  `toContain('StartFullscreen = true')`.
- Assert the RA and BIOS keys are ABSENT (no credentials are configured in
  the E2E run, and D6 removed the BIOS write):
  `expect(ini).not.toContain('[Achievements]')`,
  `expect(ini).not.toContain('Bios')`.
- Assert `expect(ini).not.toContain('portable')` is NOT used — `portable.ini`
  is a separate empty file, not an INI key.
- Update the spec's file-header comment to say the group now also covers
  autoconfig, so the doc block matches the assertions.

Nothing about the Redream install changes: Redream's writer needs a data
root and the stub tarball provides one, so its `redream.cfg` write is
covered by the Rust tests rather than a second E2E assertion.

**doc 05 deviations.** Append a section
`## Rust port deviations (milestone 5)` to
`docs/porting/05-emulator-autoconfig.md`, in the style of doc 04's
milestone-3/4 sections:

1. Trigger policy: `ensure_*` writers and entry autoconfig run only when a
   NEW emulator entry is created (catalog install, manual add). Never on
   edits, launches or view refreshes. The `name::path` session cache is
   gone — there is nothing left to deduplicate, which also moots the
   stronger-invalidation open question.
2. RA credential fan-out: saving credentials runs a dedicated narrow
   writer per RA-capable module (`ensure_*_ra_credentials`) that touches
   only the RA keys. Clearing still writes nothing and scrubs nothing.
3. Defaults backfill runs at the two trigger points above, not on every
   emulator view refresh.
4. `ensure_pcsx2_settings` uses the expanded, trimmed path throughout; the
   reference's raw-text `emulator_dir` could create a literal `~`
   directory. Ruled a bug and fixed.
5. PPSSPP's two unprotected reads are guarded; an unreadable INI yields
   `changed=false` instead of propagating.
6. PCSX2 `[Folders] Bios` is not written — the firmware subsystem is
   deferred to its own milestone, which also owns closing this.
7. The RPCS3 background firmware download (`PS3UPDAT.PUP` fetch and the
   `--installfw` spawn) is out, same deferral.
8. Every `ensure_*` returns one `EnsureResult { changed, config_path,
   extras }`; the reference's `str`-vs-`Path`-vs-dict mix was a
   dynamic-typing artifact.
9. `apply_xenia_content_without_ui` (the STFS content installer) is out of
   scope: this milestone ports Xenia's readers only.
   `copy_ps3_custom_config_to_emulator` and
   `trigger_rpcs3_firmware_install` are likewise unported.
10. The readers are ported and unit-tested but have no caller yet —
    milestone 6 (cloud saves) is their consumer.
11. Cemu's `settings.xml` edit is byte-preserving (targeted text
    replacement) rather than an XML reserialization. This is a strictly
    stronger form of the reference's "everything else is preserved"
    guarantee and avoids adding an XML dependency.
12. Entry autoconfig preserves an existing entry's `source_id`,
    `source_provider`, `source_owner`, `source_repo` and
    `source_release_tag`; the reference rebuilds the entry with eight keys
    and has no equivalent of these install-provenance fields.
13. `Config` reuses the existing `retroarch_cores` map for core defaults
    (the reference's `default_retroarch_cores`); no second map was added.
    `EmulatorEntry` gains `save_strategy`, `ignore_files`,
    `ignore_extensions`, `save_paths`, `state_paths`, and
    `EmulatorProfile` gains the five autoprofile fields that feed them.
14. The assignable server-platform list reaches grid-core through
    `InstallService::set_known_platforms`, fed by the `list_platforms`
    command; with no connected session the list is empty and the
    platform-defaults step is a no-op.
15. `sync_new_emulator`'s entry-autoconfig step uses
    `apply_manual_emulator_profile_defaults` at BOTH D1 sites; layer 1's
    `auto_configure_emulator_settings` rebuild path is unreachable in the
    rewrite because `finalize_emulator` already writes the profile-named
    entry before the sync runs. `auto_configure_emulator_settings` is
    retained reference-only, exercised by its own tests and called from no
    production path. Site B (manual add) is exact Python parity, and it is
    what decides the design; at site A the two functions' outputs are
    equivalent, because the entry was just written from the same profile.
    Field nuance: a blank-`args` profile leaves the entry's `args` blank
    instead of writing `"%rom%"`. Three catalog profiles have blank `args`
    (`ShadPS4 Qt Launcher`, `GE-Proton`, `Proton-CachyOS`), of which only
    the first is reachable — `profile_for_entry` skips compat-tool profiles
    outright. The difference is launch-identical: `template::build_args`
    substitutes `"%rom%"` for a blank `entry_args` at launch time
    (launch.py:150).

Also update doc 05's "Open questions" section: mark each question this
milestone rules on with its ruling (follow-the-code for DuckStation RA,
the three write policies, the DuckStation and Dolphin target divergences,
the RPCS3 always-portable target, the arcade-biased core fallback, and
no-scrub-on-clear; fixed for the PCSX2 raw path and the PPSSPP unprotected
reads; single accumulator for Xemu; two distinct variables for the
RetroArch username; mooted for the session cache; `EnsureResult` for the
return-type question).

**README.** Update the `emulator-catalog` row of the E2E table to mention
the post-install autoconfig assertions, and add a milestone 5 line to the
milestone list in the same style as the existing ones.

- [ ] **Step 1:** write the doc 05 deviations section and the open-question
  rulings; update the README.
- [ ] **Step 2:** extend `emulator-catalog.spec.ts`.
- [ ] **Step 3:** `rewrite/scripts/e2e.sh emulator-catalog` green locally.
- [ ] **Step 4:** full `rewrite/scripts/e2e.sh` (all groups) green.
- [ ] **Step 5:** `cargo test -p grid-core`, `cargo test -p grid-core --features e2e`,
  clippy, fmt and `bash scripts/check_secret_hygiene.sh` all green in both
  feature states.
- [ ] **Step 6: commit** `rewrite: milestone 5 e2e autoconfig assertions, deviations and docs`

---

## Self-review notes (already applied)

- **Decomposition rationale.** The spec's suggested 12-task split put every
  reader in one task; the readers are roughly 2000 lines of Python across
  13 modules, so they are split into Tasks 8 and 9 along a dependency line
  (Task 8's shared `consume_arg_value` / `unique_paths` / `resolve_against`
  helpers are reused by Task 9). The writer modules are batched by writer
  FAMILY, not alphabetically, so each task exercises one or two families
  from `writers.rs` and its tests pin that family's policy: Task 5 is the
  two probe-gated INI writers (and the PCSX2-vs-DuckStation probe-source
  divergence sits in one task, where it can be contrasted), Task 6 is the
  three Qt/annotation writers plus the append-block writer, Task 7 is the
  four odd ones out (lazy-path INI, XML, add-only TOML, flat cfg). RetroArch
  and RPCS3 get their own tasks: RetroArch owns a unique flat writer plus a
  reader its writer depends on, and RPCS3 owns five files across three
  formats.
- **`writers.rs` before everything.** Every emulator module is a thin
  declaration of sections and keys over it, so it lands first and its tests
  are the doc 05 policy table itself. That table test is written out in
  full in Task 1 because it is the single most load-bearing assertion in the
  milestone.
- **Spec gap 1 — `Config.default_cores`.** The spec asks for a new
  `default_cores: BTreeMap<String, String>`. The rewrite already carries
  exactly that map as `retroarch_cores` (the reference's
  `default_retroarch_cores`), populated by the milestone-3 defaults UI.
  **Resolution:** reuse `retroarch_cores`; adding a second map would split
  the core-default state in two. Recorded as deviation 13.
- **Spec gap 2 — entry and profile fields.** The spec lists only
  `default_cores` and `retroachievements_username` as config additions, but
  layer 1 writes eight entry fields and reads nine profile fields; the
  rewrite's `EmulatorEntry` carries three plus the milestone-4 `source_*`
  set, and `EmulatorProfile` carries six. **Resolution:** Task 10 adds the
  five missing entry fields and the five missing profile fields, using the
  established `skip_serializing_if = "String::is_empty"` pattern so an
  untouched config round-trips byte-identically. Recorded as deviation 13.
- **Spec gap 3 — where the platform list comes from.**
  `assign_profile_platform_defaults` needs the assignable server-platform
  names, and both D1 trigger points run without a RomM session in hand
  (`finalize_emulator` lives inside `InstallService`, which has no client).
  **Resolution:** `InstallService::set_known_platforms`, fed by the
  `list_platforms` command after a successful fetch, with an empty list
  meaning "assign nothing" — which is exactly what the reference does when
  its platform map is empty. grid-core stays Tauri-free and the seam is
  directly testable. Recorded as deviation 14.
- **Spec gap 4 — the `expose_secret` allowlist.**
  `check_secret_hygiene.sh` hard-codes two permitted call sites, and the RA
  token must reach three writers. **Resolution:** exactly one new call
  site, `RaCredentials::token()` in `autoconfig/mod.rs`, added to the
  allowlist in Task 11 (the task that introduces it — otherwise that task's
  own gate fails). The per-emulator modules receive `&RaCredentials` and
  call `token()`; they never touch `SecretString`.
- **Spec gap 5 — one keyring slot.** `secrets.rs` stores a single
  credential under one account. **Resolution:** Task 12 adds an independent
  `RaTokenStore` trait on a second account under the same service, so the
  RomM credential and the RA token can be cleared independently.
- **Spec gap 6 — Xenia's content installer.** The spec's scope bullets
  cover writers, readers, entry autoconfig and core metadata;
  `apply_xenia_content_without_ui` is none of those, and neither is
  `copy_ps3_custom_config_to_emulator`. **Resolution:** declared out of
  scope (deviation 9) rather than smuggled into the readers task.
- **Cemu without an XML crate.** The reference uses ElementTree, whose
  reserialization normalizes some whitespace. Adding an XML dependency for
  six element writes is disproportionate, and a hand-rolled parse would be
  a fidelity risk. **Resolution:** targeted text replacement inside the
  root element, which preserves every other byte — strictly stronger than
  the reference's guarantee, recorded as deviation 11 and covered by the
  "unmanaged settings survive byte-for-byte" test.
- **The two probe-source behaviors are deliberately NOT unified.** PCSX2
  probes its progressively rewritten content; DuckStation probes the frozen
  pre-write content. Both are pinned by a named test in Task 5 so a future
  refactor cannot quietly merge them.
- **`ensure_*` argument shape.** The reference passes RA credentials as two
  `str` parameters with a both-non-blank gate repeated in each writer. The
  port passes `Option<&RaCredentials>` and centralizes the gate in
  `RaCredentials::usable()`; the observable behavior is identical and the
  token stops being a bare `String` at every boundary.
- **Testing budget.** Doc 05's oracle lists 177 tests in
  `test_emulator_autoconfig_settings.py` alone. The plan names concrete
  cases per task rather than a count, and anchors each group to the Python
  test line numbers so parity is checkable; the writer-policy table, the
  RetroArch username-rebinding trap, the PCSX2 tilde-path fix, the
  DuckStation probe-source divergence, the Azahar widened key regex, the
  RPCS3 `CurrentSettings` no-spaces format, the PPSSPP `installed.txt`
  deletion and `.dat` token file, and the Dolphin GCPad append-if-absent
  rule each get a dedicated named test.
- **A `paths.rs` helper module** is not in the spec's module map. It exists
  because eleven modules need the same XDG/APPDATA/home resolution and
  case-insensitive candidate dedupe, and duplicating it eleven times is
  exactly the near-duplication the spec asks the port to collapse.
- **Exit gate.** Task 13 step 4 (full `e2e.sh`) plus CI on the pushed
  branch. Per the standing milestone rule, finish with
  `cargo clean --profile dev` from `rewrite/`.
