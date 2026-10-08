# Dolphin auto-install and KytyPS5 `.zar` games: implementation plan (2026-10-08)

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax. One implementer at a time in the shared working tree. Commit each verified task with `git commit --only <paths>`. Never run `git checkout`, `git restore`, `git reset` or `git stash` on tracked files.

**Goal:** GRID installs Dolphin from the catalog. On Windows it uses the official portable `.7z`. On Linux it uses the unofficial `pkgforge-dev` AppImage. Dolphin is not offered on macOS. Games launch fullscreen, on Vulkan, at 3x internal resolution, with user data in `<exe dir>/User`. KytyPS5 gets explicit 1080p output args. A PS5 `.zar` installs untouched. A PS5 `.pkg` or bare folder game is rejected before any download.

**Architecture:**
- **One catalog row on every OS.** A `platform_overrides` entry is now merged into the raw source before normalization. This lets one profile switch provider: `direct` on Windows, `github` on Linux. The catalog still shows exactly one Dolphin row, built from the top-level source identity.
- **`%emu_dir%` placeholder.** A new placeholder in `launch/template.rs` expands to the executable's directory. `prepare_emulator_launch` fills it in. The Dolphin save reader expands the same placeholder, so cloud saves follow the `-u` flag.
- **PS5 rejection.** The PS5 checks run in `plan_install`, before admission. `.zar` already takes the `Downloaded` route because it is not an extractable suffix. A test pins that.

**Tech Stack:** Rust (grid-core, Tauri 2 shell), serde_json (`preserve_order`), regex, wiremock (tests), vitest unchanged, WebdriverIO e2e (regression only).

**Spec:** `docs/superpowers/specs/2026-10-08-dolphin-and-kyty-zar-design.md`.

**Gate (repo root, `CLAUDE.md` order):** `scripts/check_secret_hygiene.sh`, `cargo fmt --check`, `cd app && npm ci && npx svelte-check && npm run build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`, `cd app && npm test`.

---

## Decisions taken while checking the code

These settle the points the spec left to the planner. Each one binds the coder.

1. **The override can switch provider (no second profile).** Today `forge.rs::resolve` (`:92-93`) and `commands.rs::update_source_for` (`:1705-1706`) normalize first and merge the override after. A `provider` key in an override therefore lands un-normalized (`"github-release"`). It then fails with "Unsupported source provider". The fix is a new `source::resolve_source_for_host(raw, host)`. It shallow-merges the matching override into the RAW object with the existing `merge_platform_override_for`, then calls `normalize_source`. A regression test pins that every override in today's catalog that does not set `provider` gives the same map as before.
2. **The catalog row keeps the top-level identity.** `catalog::catalog_row` keeps reading the top-level `provider`/`owner`/`repo`. RPCS3 already works this way: its row says `rpcs3-binaries-win` on Linux. So `source_id` stays `dolphin-emu/dolphin` on every host, and `find_profile` finds the profile from any host. Side effect: the Linux row's meta line reads `direct • latest` (see Open Questions).
3. **Dolphin save lists stay empty, on purpose.** `cloud/dirs.rs:484` asks the Dolphin readers only when the entry's `save_paths`/`state_paths` are blank. A catalog install copies the profile's lists into the entry (`autoconfig::entry::apply_manual_emulator_profile_defaults`). Empty `save_directories`/`state_directories` keep the readers in charge. The readers know memory-card permutations, GCI region folders, Wii title groups and `Dolphin.ini` overrides. `readers::dolphin_launch_user_root` (`readers.rs:621`) reads the `-u` value, and today it would read the literal `%emu_dir%/User`. Task 4 makes it expand the placeholder.
4. **`user_data: ["User"]`, `screenshot_directories: ["User/ScreenShots"]`.** The data root is the executable's directory for every non-PCSX2 binary (`autoconfig::emulator_data_root`):
   - Windows `.7z`: the directory that holds `Dolphin.exe`, possibly nested as `Dolphin-x64/`.
   - Linux: the AppImage's directory.
   Both match `-u "%emu_dir%/User"` and `dolphin::ini_path_candidates` candidate 0 (`<exe parent>/User/Config`). `ensure_user_data_links` links that `User` into `saves/<profile>/User`.
5. **No `firmware_directories` for Dolphin.** `firmware::routing::install_for_game` (`routing.rs:428`) returns early when no targets exist. The Dolphin hooks (`ensure_skip_ipl`, `ensure_gcpad_config`, `:472-477`) only run when targets exist. `ensure_gcpad_config` appends an `XInput/0/Gamepad` block, which is Windows-only and would break the default Linux mapping. Leaving firmware routing off keeps today's behaviour, where no Dolphin entry ever reaches those hooks. Dolphin's own IPL handling also needs per-region `User/GC/<REGION>/IPL.bin` paths that flat routing cannot produce. This is a follow-up (Open Questions).
6. **`save_strategy: "single_file"`.** `cloud/ops/mod.rs:804` gives Dolphin its own branch, so the strategy is not read for Dolphin saves. `"single_file"` matches DuckStation/PCSX2.
7. **Executable choice.** The title tokens give `Dolphin.exe` and `DolphinTool.exe` the same score. Then the shallower path wins. `select_executable` gains the preferred name `dolphin.exe` for a title containing `dolphin`.
8. **Install directory name.** `sanitize_component` turns `/` into `_`. The profile `Dolphin (GameCube / Wii)` therefore installs to `emulators/Dolphin (GameCube _ Wii)/` and saves to `saves/Dolphin (GameCube _ Wii)/`. The spec's wording `emulators/Dolphin (GameCube / Wii)/` cannot exist on disk. See Open Question 1. That question must be answered before Task 5 lands.
9. **The feed regex.** Captured live on 2026-10-08: the payload is Python-style JSON (`", "` separators, unescaped `/`). The artifact order puts `-arm64.7z` BEFORE `-x64.7z`. The href walk in `scrape_download_url` finds nothing: the only `href` sits inside `changelog_html` as `href=\"…`, and `\` is not whitespace. The whole-page fallback then returns the whole match, because the regex has no capture group. The catalog regex is `https://dl\.dolphin-emu\.org/releases/[0-9A-Za-z._-]+/dolphin-[0-9A-Za-z._-]+-x64\.7z`, compiled case-insensitive. The character class excludes `/` and `"`, so a match cannot cross into a neighbouring artifact. `-arm64.7z` can never end in `-x64.7z`.
10. **`.zar` needs no routing code.** `extract::EXTRACTABLE_SUFFIXES` does not list `zar`, so `base_finalize_route("PlayStation 5", "x.zar")` is already `Downloaded`. Task 9 pins it.
11. **PS5 rejection surfaces verbatim.** `LibraryError::Extract` displays `{0}`. `commands::install_game` maps it with `err`. `Server.svelte:271-274` and `Details.svelte:416-419` show the message as-is.
12. **No Dolphin e2e stage.** `e2e/mock-romm/mock-forge.mjs` serves only PCSX2, Redream and GRID's own release. A Dolphin stage needs new routes and fixture bytes. Per the spec rule, it is out. The `emulator-catalog` and `launch` groups run as regression.

---

## Global Constraints

- **Never destroy work:** no `git checkout`, `git restore`, `git reset`, `git stash` on tracked files.
- **Commits:** `git commit --only <paths>` with the paths listed in each task. Every message ends with the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **grid-core stays Tauri-free.** Only Task 1 touches `app/src-tauri` (one import and one call site).
- **Secrets:** nothing here reads or logs a credential. Fixtures contain no `token`/`password`/`Bearer` strings, so `check_secret_hygiene.sh` stays green.
- **Catalog JSON is compiled in** (`include_str!` in `profiles.rs` and `source.rs` tests). Editing it is a rebuild.
- **Formatting:** run `cargo fmt` before each commit and commit only the task's paths.
- **No RomM request changes** (`openapi.json` not involved). No IPC payload shape changes.
- **Autoconfig writers are untouched.** `autoconfig/dolphin.rs` keeps its overwrite policy and its Vulkan seed.

## Review Focus

These are the five failure modes most likely to ship without a test today. Each one gets a test in its owning task.

1. **Linux Dolphin installs the Windows `.7z`.** The override fails to switch provider, so the Linux host scrapes the Windows feed. Tests: Task 1 `resolve_source_for_host_lets_an_override_switch_the_provider`; Task 5 `embedded_dolphin_source_is_direct_on_windows_and_github_on_linux`.
2. **The feed regex picks `-arm64.7z`, a `.flatpak`, or the `.dmg`.** The real feed lists arm64 first. Tests: Task 6 `dolphin_regex_picks_the_windows_x64_7z_out_of_the_update_feed` (fixture keeps the real order) and `dolphin_regex_finds_nothing_when_the_feed_has_no_x64_build`.
3. **Cloud saves read the literal `%emu_dir%/User`.** They then fall back to a stale `~/.dolphin-emu`, so the wrong saves are uploaded or restored. Test: Task 4 `dolphin_emu_dir_user_flag_wins_over_a_stale_home_install`. It uses no `portable.txt`, so the fallback cannot hide the bug.
4. **A PS5 `.pkg` or folder game starts downloading, or a `.zar` gets extracted.** Tests: Task 9:
   - `plan_rejects_a_ps5_pkg_before_any_download`
   - `plan_rejects_a_ps5_folder_game`
   - `plan_accepts_a_single_ps5_zar_and_routes_it_as_downloaded`
5. **Windows Dolphin launches `DolphinTool.exe`.** Test: Task 5 `dolphin_title_prefers_dolphin_exe_over_its_bundled_tools`.

Also pinned:
- The hard user requirement, exactly one Dolphin row on win32 and linux and none on darwin: Task 5 `real_catalog_shows_exactly_one_dolphin_row_on_windows_and_linux_and_none_on_macos`.
- `%emu_dir%` with spaces stays one argument, and a blank value is an error: Task 3.

---

## Task 1: Platform overrides can switch the source provider

**Files:**
- `crates/grid-core/src/launch/source.rs` (new fns + tests)
- `crates/grid-core/src/launch/forge.rs` (`ForgeClient::resolve`, imports)
- `crates/grid-core/src/launch/profiles.rs` (test helper `win32_source` only)
- `app/src-tauri/src/commands.rs` (`update_source_for`, imports at `:23-25`)

**Interfaces:**
- Consumes:
  - `pub fn normalize_source(raw: &Value) -> Result<SourceMap, SourceError>`
  - `pub(crate) fn merge_platform_override_for(source: &mut SourceMap, host: &str)`
  - `pub const HOST_PLATFORM: &str`
- Produces:
  - `pub fn resolve_source_for_host(raw: &Value, host: &str) -> Result<SourceMap, SourceError>`
  - `pub fn resolve_source(raw: &Value) -> Result<SourceMap, SourceError>`

- [ ] **1.1 Write the failing tests** in `source.rs`'s `#[cfg(test)] mod tests` (after the `merge_platform_override` block):

```rust
    // --- resolve_source_for_host ------------------------------------------------

    #[test]
    fn resolve_source_for_host_lets_an_override_switch_the_provider() {
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r", "release_tag": "latest",
            "page_url": "https://example.invalid/latest/",
            "download_url_regex": "https://example\\.invalid/x64\\.7z",
            "platform_overrides": {"linux": {
                "provider": "github-release", "owner": "p", "repo": "q",
                "asset_patterns": ["App-*-x86_64.AppImage"],
                "asset_exclude_patterns": ["*.zsync"]
            }}
        });

        let linux = resolve_source_for_host(&raw, "linux").unwrap();
        assert_eq!(str_field(&linux, "provider"), "github");
        assert_eq!(str_field(&linux, "owner"), "p");
        assert_eq!(str_field(&linux, "repo"), "q");
        assert_eq!(str_field(&linux, "release_tag"), "latest");
        assert_eq!(linux["asset_patterns"], json!(["App-*-x86_64.AppImage"]));
        assert_eq!(linux["asset_exclude_patterns"], json!(["*.zsync"]));
        assert!(
            !linux.contains_key("download_url"),
            "direct-only keys are not normalized for a github source"
        );

        let windows = resolve_source_for_host(&raw, "win32").unwrap();
        assert_eq!(str_field(&windows, "provider"), "direct");
        assert_eq!(
            str_field(&windows, "page_url"),
            "https://example.invalid/latest/"
        );
    }

    #[test]
    fn resolve_source_for_host_rejects_a_non_object_source_verbatim() {
        let err = resolve_source_for_host(&json!(["x"]), "linux").unwrap_err();
        assert_eq!(err.0, "Source metadata must be a dictionary.");
    }

    /// Merging the override before normalizing must not change any catalog
    /// entry whose override leaves `provider` alone: for those, the new
    /// order and the reference order (normalize, then merge) give the same
    /// map on every host. Every source must also resolve on every host.
    #[test]
    fn resolve_source_matches_normalize_then_merge_for_the_catalog() {
        let entries: Vec<Value> = serde_json::from_str(AUTOPROFILES_JSON).unwrap();
        for entry in &entries {
            let Some(source) = entry.get("source").filter(|s| s.is_object()) else {
                continue;
            };
            let switches_provider = source
                .get("platform_overrides")
                .and_then(Value::as_object)
                .is_some_and(|overrides| {
                    overrides
                        .values()
                        .any(|o| o.get("provider").is_some() || o.get("type").is_some())
                });
            for host in ["win32", "linux", "darwin"] {
                let resolved = resolve_source_for_host(source, host).unwrap_or_else(|e| {
                    panic!("{:?} on {host}: {}", entry.get("name"), e.0)
                });
                if switches_provider {
                    continue;
                }
                let mut reference = normalize_source(source).unwrap();
                merge_platform_override_for(&mut reference, host);
                assert_eq!(resolved, reference, "{:?} on {host}", entry.get("name"));
            }
        }
    }
```

- [ ] **1.2 Run:** `cargo test -p grid-core resolve_source` (expect a compile failure: `resolve_source_for_host` not found).

- [ ] **1.3 Implement** in `source.rs`, directly after `merge_platform_override_for`:

```rust
/// `raw` as `host` sees it: the first `platform_overrides` entry whose key
/// is a prefix of `host` (the [`merge_platform_override_for`] rule) is
/// shallow-merged over the RAW object, and only then is the result
/// normalized. Merging first lets an override change anything
/// normalization reads, `provider` included, so one catalog profile can
/// scrape a `direct` page on Windows and pick a GitHub release asset on
/// Linux (Dolphin).
///
/// DEVIATION from workers.py:165-175, which normalizes first and merges
/// the raw override afterwards. For an override that does not set
/// `provider`, both orders give the same map
/// (`resolve_source_matches_normalize_then_merge_for_the_catalog`).
pub fn resolve_source_for_host(raw: &Value, host: &str) -> Result<SourceMap, SourceError> {
    let Some(obj) = raw.as_object() else {
        return normalize_source(raw);
    };
    let mut merged = obj.clone();
    merge_platform_override_for(&mut merged, host);
    normalize_source(&Value::Object(merged))
}

/// [`resolve_source_for_host`] for this build's [`HOST_PLATFORM`]: the
/// source an install or an update check actually uses.
pub fn resolve_source(raw: &Value) -> Result<SourceMap, SourceError> {
    resolve_source_for_host(raw, HOST_PLATFORM)
}
```

- [ ] **1.4 Switch the callers:**
  - `forge.rs` imports (`:24-27`): replace `merge_platform_override, normalize_source` with `resolve_source`.
  - In `ForgeClient::resolve`, replace the two lines `let mut source = normalize_source(raw)?; merge_platform_override(&mut source);` with `let source = resolve_source(raw)?;`. Change the comment and the `expect` text that mention `normalize_source` to `resolve_source`.
  - Update the `resolve` doc comment to "resolve for this host (override merged into the raw source, then normalized), then dispatch on provider".
  - `commands.rs:23-25`: the import becomes `use grid_core::launch::source::{allow_prerelease, resolve_source, str_field, SourceMap};`.
  - In `update_source_for` (`:1705-1706`), use `let source = resolve_source(&raw).map_err(|e| e.0)?;`. Update the doc comment's "normalized the same way the install itself normalizes it" to "resolved the same way the install resolves it".
  - `profiles.rs` test helper `win32_source` (`:1443-1455`): the body ends with `crate::launch::source::resolve_source_for_host(raw, "win32").unwrap()`. Doc: "resolved as a win32 host would see it".
  - Keep `merge_platform_override` (pub) and its tests unchanged.

- [ ] **1.5 Run:** `cargo test -p grid-core resolve_source`, `cargo test -p grid-core embedded_sources_pick_the_windows_asset_from_real_release_listings`, `cargo test -p grid-core forge`, `cargo test -p app update_check`. All pass. Run `cargo clippy --workspace --all-targets -- -D warnings`. Expected: no unused-import warning.

- [ ] **1.6 Commit:**
```
git commit --only crates/grid-core/src/launch/source.rs crates/grid-core/src/launch/forge.rs crates/grid-core/src/launch/profiles.rs app/src-tauri/src/commands.rs \
  -m "feat(launch): platform overrides can switch the source provider" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 2: Catalog rows can be built for any host

**Files:** `crates/grid-core/src/launch/catalog.rs`

**Interfaces:**
- Consumes: `HOST_PLATFORM`.
- Produces (private to the module, used by its tests):
  - `fn catalog_entries_for_host(profiles: &[EmulatorProfile], host: &str) -> Vec<CatalogEntry>`
  - `fn catalog_row(profile: &EmulatorProfile, host: &str) -> Option<CatalogEntry>`
  - `fn platforms_allow_host(source: &Map<String, Value>, host: &str) -> bool`
- Public signatures unchanged: `catalog_entries`, `compat_tool_catalog_entries`, `find_profile`, `find_compat_profile`.

- [ ] **2.1 Write the failing test** in `catalog.rs` tests (after `platforms_gate_is_a_noop_when_the_list_is_empty`):

```rust
    #[test]
    fn one_profile_with_a_provider_switching_override_is_one_row_per_allowed_host() {
        let profiles = vec![profile(
            "Switcher",
            false,
            Some(json!({
                "provider": "direct", "owner": "o", "repo": "r",
                "page_url": "https://x.invalid/",
                "platforms": ["win32", "linux"],
                "platform_overrides": {"linux": {
                    "provider": "github-release", "owner": "p", "repo": "q"
                }}
            })),
        )];
        for host in ["win32", "linux"] {
            let rows = catalog_entries_for_host(&profiles, host);
            assert_eq!(rows.len(), 1, "host {host}: {rows:?}");
            // The row keeps the top-level identity on every host, so an
            // installed entry's source_id finds its profile again wherever
            // the config is read.
            assert_eq!(rows[0].source_id, "o/r", "host {host}");
            assert_eq!(rows[0].provider, "direct", "host {host}");
        }
        assert!(catalog_entries_for_host(&profiles, "darwin").is_empty());
    }
```

- [ ] **2.2 Run:** `cargo test -p grid-core one_profile_with_a_provider_switching_override` (expect a compile failure).

- [ ] **2.3 Implement:**
  - `platforms_allow_host(source, host: &str)`: replace `HOST_PLATFORM.starts_with(text.as_str())` with `host.starts_with(text.as_str())`. The doc now says "allows `host`".
  - `catalog_row(profile, host: &str)` passes `host` to `platforms_allow_host`.
  - `catalog_entries_filtered(profiles, compat: bool, host: &str)` passes `host` to `catalog_row`.
  - Add:
```rust
/// [`catalog_entries`] as `host` (a `sys.platform`-shaped slug) would list
/// it: the same rows, gated by that host's `platforms` allowlist instead of
/// this build's.
fn catalog_entries_for_host(profiles: &[EmulatorProfile], host: &str) -> Vec<CatalogEntry> {
    catalog_entries_filtered(profiles, false, host)
}
```
  - `catalog_entries` returns `catalog_entries_for_host(profiles, HOST_PLATFORM)`.
  - `compat_tool_catalog_entries` passes `HOST_PLATFORM`.
  - `find_profile_filtered` calls `catalog_row(profile, HOST_PLATFORM)`.

- [ ] **2.4 Run:** `cargo test -p grid-core catalog`. All pass.

- [ ] **2.5 Commit:**
```
git commit --only crates/grid-core/src/launch/catalog.rs \
  -m "refactor(catalog): build catalog rows for an explicit host" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 3: `%emu_dir%` launch placeholder

**Files:**
- `crates/grid-core/src/launch/template.rs`
- `crates/grid-core/src/launch/spawn.rs`
- `crates/grid-core/src/launch/mod.rs` (`resolve_launch` literal at `:551-555`)

**Interfaces:**
- Produces:
  - `pub struct Placeholders { pub rom: String, pub core: String, pub ps3_launch_target: String, pub emu_dir: String }`
  - `pub const EMU_DIR_PLACEHOLDER: &str = "%emu_dir%";`
  - `pub const EMU_DIR_MISSING: &str`
  - `pub fn emulator_dir_placeholder(executable: &Path) -> String`
- Unchanged signatures: `build_args`, `validate_placeholders`, `apply_placeholders`, `prepare_emulator_launch`.

- [ ] **3.1 Write the failing tests.** In `template.rs` tests:
  - Change the helper `placeholders(rom, core, ps3)` to add `emu_dir: String::new(),`.
  - Add:

```rust
    // --- %emu_dir% ------------------------------------------------------------

    #[test]
    fn emu_dir_is_substituted_and_a_path_with_spaces_stays_one_argument() {
        let ph = Placeholders {
            rom: "/roms/Some Game.rvz".to_string(),
            emu_dir: "/lib/emulators/Dolphin (GameCube _ Wii)".to_string(),
            ..Default::default()
        };
        let result = build_args("-u \"%emu_dir%/User\" -b -e \"%rom%\"", "", &ph).unwrap();
        assert_eq!(
            result,
            vec![
                "-u",
                "/lib/emulators/Dolphin (GameCube _ Wii)/User",
                "-b",
                "-e",
                "/roms/Some Game.rvz"
            ]
        );
    }

    #[test]
    fn a_blank_emu_dir_fails_instead_of_producing_a_bare_user_flag() {
        let ph = Placeholders {
            rom: "/roms/game.rvz".to_string(),
            ..Default::default()
        };
        let err = build_args("-u \"%emu_dir%/User\" -e \"%rom%\"", "", &ph).unwrap_err();
        assert_eq!(err, EMU_DIR_MISSING);

        let whitespace = Placeholders {
            emu_dir: "   ".to_string(),
            ..ph
        };
        assert_eq!(
            build_args("-u %emu_dir%", "", &whitespace).unwrap_err(),
            EMU_DIR_MISSING
        );
    }

    #[test]
    fn a_template_without_emu_dir_ignores_a_blank_value() {
        let ph = Placeholders {
            rom: "/roms/game.rvz".to_string(),
            ..Default::default()
        };
        assert_eq!(build_args("%rom%", "", &ph).unwrap(), vec!["/roms/game.rvz"]);
    }

    #[test]
    fn emulator_dir_placeholder_is_the_parent_or_blank() {
        assert_eq!(
            emulator_dir_placeholder(Path::new("/opt/Dolphin/Dolphin.exe")),
            "/opt/Dolphin"
        );
        assert_eq!(emulator_dir_placeholder(Path::new("dolphin-emu")), "");
        assert_eq!(emulator_dir_placeholder(Path::new("/")), "");
    }
```

  In `spawn.rs` tests:
  - Change the helper `placeholders(rom, core)` to add `emu_dir: String::new(),`.
  - Add:

```rust
    #[test]
    fn emu_dir_expands_to_the_executable_folder_as_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let emu_dir = dir.path().join("Dolphin (GameCube _ Wii)");
        std::fs::create_dir_all(&emu_dir).unwrap();
        let exe = emu_dir.join("Dolphin_Emulator-2609-anylinux-x86_64.AppImage");
        std::fs::write(&exe, b"stub").unwrap();
        let rom = dir.path().join("Some Game.rvz");
        std::fs::write(&rom, b"rom").unwrap();
        let rom_text = rom.to_string_lossy().into_owned();

        let e = entry(exe.to_str().unwrap(), "-u \"%emu_dir%/User\" -b -e \"%rom%\"");
        let (argv, working_dir) = prepare_emulator_launch(
            "Dolphin",
            Some(&e),
            &rom_text,
            &placeholders(&rom_text, ""),
            "",
            false,
        )
        .unwrap();

        let expected: Vec<String> = vec![
            "-u".to_string(),
            format!("{}/User", emu_dir.display()),
            "-b".to_string(),
            "-e".to_string(),
            rom_text.clone(),
        ];
        assert_eq!(argv[1..].to_vec(), expected);
        assert_eq!(working_dir, emu_dir);
    }
```

- [ ] **3.2 Run:** `cargo test -p grid-core emu_dir` (expect a compile failure: no field `emu_dir`, no `EMU_DIR_MISSING`).

- [ ] **3.3 Implement** in `template.rs`:
  - Add `pub emu_dir: String,` to `Placeholders` with the doc "The directory holding the emulator's executable (`%emu_dir%`). Filled by `spawn::prepare_emulator_launch` from the resolved executable; callers leave it blank."
  - Add, above `validate_placeholders`:

```rust
/// The launch placeholder for the directory that holds the emulator's
/// executable — the AppImage's own directory for an AppImage.
pub const EMU_DIR_PLACEHOLDER: &str = "%emu_dir%";

/// [`validate_placeholders`]'s message when a template names
/// [`EMU_DIR_PLACEHOLDER`] but the executable path has no directory.
pub const EMU_DIR_MISSING: &str =
    "The emulator's folder could not be found from its executable path.";

/// `%emu_dir%`'s value for `executable`: its parent directory, or `""`
/// when there is none (a bare file name, a filesystem root). A blank value
/// makes [`validate_placeholders`] fail, so a launch never receives a bare
/// `-u` or a `-u /User`.
pub fn emulator_dir_placeholder(executable: &Path) -> String {
    match executable.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().into_owned(),
        _ => String::new(),
    }
}
```
  - In `validate_placeholders`, add as the last check:
```rust
    if template.contains(EMU_DIR_PLACEHOLDER) && ph.emu_dir.trim().is_empty() {
        return Err(EMU_DIR_MISSING.to_string());
    }
```
  - In `apply_placeholders`, append `.replace(EMU_DIR_PLACEHOLDER, &ph.emu_dir)` after the `%ps3_launch_target%` replace. Update the doc's replace list.
  - Module doc: mention `%emu_dir%` next to the reference placeholders as a GRID addition.

  In `spawn.rs`:
  - Import becomes `use super::template::{build_args, emulator_dir_placeholder, normalized_retroarch_core_args, Placeholders};`.
  - In `prepare_emulator_launch`, replace the `build_args` call with:
```rust
    // `%emu_dir%` comes from the executable this function just resolved, so
    // every caller gets it without computing it itself.
    let placeholders = Placeholders {
        emu_dir: emulator_dir_placeholder(&executable),
        ..placeholders.clone()
    };
    let args = build_args(&entry.args, global_launch_args, &placeholders)
        .map_err(|e| format!("Invalid launch arguments: {e}"))?;
```
  - Add one line to the function doc: "`%emu_dir%` is filled from the resolved executable's parent."

  In `launch/mod.rs::resolve_launch`, add `emu_dir: String::new(),` to the `Placeholders` literal with the comment `// filled by prepare_emulator_launch from the resolved executable`.

- [ ] **3.4 Run:** `cargo test -p grid-core template`, `cargo test -p grid-core spawn`, `cargo test -p grid-core --test launch_service`, `cargo test -p grid-core --test launch_tilde`. All pass.

- [ ] **3.5 Commit:**
```
git commit --only crates/grid-core/src/launch/template.rs crates/grid-core/src/launch/spawn.rs crates/grid-core/src/launch/mod.rs \
  -m "feat(launch): %emu_dir% expands to the emulator executable's folder" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 4: Dolphin save reader expands `%emu_dir%` in `-u`

**Files:**
- `crates/grid-core/src/autoconfig/readers.rs` (`dolphin_launch_user_root`, `dolphin_user_root_candidates`, tests)
- `crates/grid-core/src/cloud/dirs.rs` (test only)

**Interfaces:**
- Consumes: `crate::launch::template::EMU_DIR_PLACEHOLDER` (Task 3).
- Produces (private):
  - `fn dolphin_launch_user_root(args: Args, emulator_dir: Option<&Path>) -> Option<PathBuf>`
  - `fn expand_emu_dir(value: &str, emulator_dir: Option<&Path>) -> Option<String>`
- Public `dolphin_user_root_candidates(path: &str, args: Args) -> Vec<PathBuf>` is unchanged.

- [ ] **4.1 Write the failing tests.** In `readers.rs` tests (Dolphin block, after `dolphin_user_root_prefers_a_launch_user_flag`):

```rust
    #[test]
    fn dolphin_user_root_expands_the_emu_dir_placeholder_in_the_user_flag() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = dolphin_emulator(temp.path());
        // No portable.txt: only the expanded -u flag can name <exe dir>/User first.
        let launch = crate::launch::template::split_template(
            "-u \"%emu_dir%/User\" -b -v Vulkan -e \"%rom%\"",
        )
        .unwrap();
        assert_eq!(
            dolphin_user_root_candidates(&exe, &launch).first(),
            Some(&resolve_best_effort(&dir.join("User")))
        );

        let equals = args(&["--user=%emu_dir%/User"]);
        assert_eq!(
            dolphin_user_root_candidates(&exe, &equals).first(),
            Some(&resolve_best_effort(&dir.join("User")))
        );
    }

    #[test]
    fn dolphin_user_root_skips_an_emu_dir_flag_with_no_executable() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let launch = args(&["-u", "%emu_dir%/User"]);
        let roots = dolphin_user_root_candidates("", &launch);
        assert!(
            roots
                .iter()
                .all(|root| !root.to_string_lossy().contains("%emu_dir%")),
            "{roots:?}"
        );
    }
```

  In `cloud/dirs.rs` tests (after `dolphin_override_wiring_lands_ahead_of_profile_paths`):

```rust
    /// The catalog Dolphin profile launches with `-u "%emu_dir%/User"` and
    /// leaves its save lists empty, so the readers resolve saves from that
    /// flag. With no `portable.txt` and a stale `~/.dolphin-emu` on disk,
    /// only the EXPANDED flag can put `<exe dir>/User` first.
    #[test]
    fn dolphin_emu_dir_user_flag_wins_over_a_stale_home_install() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let emulator_dir = temp.path().join("Dolphin (GameCube _ Wii)");
        std::fs::create_dir_all(&emulator_dir).unwrap();
        let exe = emulator_dir.join("Dolphin_Emulator-2609-anylinux-x86_64.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let card_a = emulator_dir
            .join("User")
            .join("GC")
            .join("USA")
            .join("Card A");
        std::fs::create_dir_all(&card_a).unwrap();
        let stale_home = temp.path().join(".dolphin-emu");
        std::fs::create_dir_all(stale_home.join("GC").join("USA").join("Card A")).unwrap();

        let profile = EmulatorProfile {
            match_tokens: vec!["dolphin*.appimage".to_string()],
            ..Default::default()
        };
        let mut e = entry("Dolphin (GameCube / Wii)", &exe.to_string_lossy());
        e.args = "-u \"%emu_dir%/User\" -b -v Vulkan -e \"%rom%\"".to_string();
        let c = ctx(Some(&emulator_dir), temp.path());

        let (resolved, _files) =
            resolved_sync_directory_paths(&e, Some(&profile), PathKey::SavePaths, &c);

        assert!(
            resolved.contains(&paths::resolve_best_effort(&card_a)),
            "{resolved:?}"
        );
        let stale = paths::resolve_best_effort(&stale_home);
        assert!(
            resolved.iter().all(|p| !p.starts_with(&stale)),
            "{resolved:?}"
        );
    }
```

- [ ] **4.2 Run:** `cargo test -p grid-core dolphin_user_root_expands` and `cargo test -p grid-core dolphin_emu_dir_user_flag_wins`. Expected: both FAIL. The first candidate is the relative `%emu_dir%/User`, and the cloud test resolves under `.dolphin-emu`.

- [ ] **4.3 Implement** in `readers.rs`, above `dolphin_launch_user_root`:

```rust
/// `value` with the `%emu_dir%` launch placeholder
/// ([`crate::launch::template::EMU_DIR_PLACEHOLDER`]) replaced by the
/// directory that holds the executable — the expansion a launch performs,
/// so a reader sees the same user root Dolphin will. `None` when `value`
/// names the placeholder but no directory is known; the caller skips that
/// candidate rather than resolving a literal `%emu_dir%` path.
fn expand_emu_dir(value: &str, emulator_dir: Option<&Path>) -> Option<String> {
    use crate::launch::template::EMU_DIR_PLACEHOLDER;
    if !value.contains(EMU_DIR_PLACEHOLDER) {
        return Some(value.to_string());
    }
    let dir = emulator_dir.filter(|dir| !dir.as_os_str().is_empty())?;
    Some(value.replace(EMU_DIR_PLACEHOLDER, &dir.to_string_lossy()))
}
```
  - Change `dolphin_launch_user_root(args: Args)` to `dolphin_launch_user_root(args: Args, emulator_dir: Option<&Path>)`.
  - In the `-u`/`--user` branch, replace `if !cleaned.is_empty() { return Some(resolve_best_effort(&paths::expand_user(&cleaned))); }` with:
```rust
                if let Some(value) =
                    expand_emu_dir(&cleaned, emulator_dir).filter(|v| !v.is_empty())
                {
                    return Some(resolve_best_effort(&paths::expand_user(&value)));
                }
```
  - Make the same replacement in the `--user=` branch.
  - Doc addition: "A value naming `%emu_dir%` is expanded against `emulator_dir`, or skipped when that is unknown."
  - In `dolphin_user_root_candidates`, call `dolphin_launch_user_root(args, emulator_dir.as_deref())`. `emulator_dir` is already computed above it. Add one line to its doc: "`-u` values expand `%emu_dir%` exactly as a launch does."

- [ ] **4.4 Run:** `cargo test -p grid-core dolphin` and `cargo test -p grid-core cloud::dirs`. All pass.

- [ ] **4.5 Commit:**
```
git commit --only crates/grid-core/src/autoconfig/readers.rs crates/grid-core/src/cloud/dirs.rs \
  -m "fix(cloud): Dolphin save reader expands %emu_dir% in the -u flag" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 5: Dolphin catalog profile

Open Question 1 (the folder name) must be answered before this task lands.

**Files:**
- `emulator-autoprofiles.json`
- `crates/grid-core/src/launch/emu_install.rs` (`select_executable` + test)
- `crates/grid-core/src/launch/profiles.rs` (tests)
- `crates/grid-core/src/launch/catalog.rs` (tests)

**Interfaces:**
- Consumes:
  - `resolve_source_for_host` (Task 1)
  - `catalog_entries_for_host` (Task 2)
  - `profile_available_on_host`, `platform_matches_keywords`, `profile_for_entry`
- Produces: the catalog profile `Dolphin (GameCube / Wii)` with `source_id` `dolphin-emu/dolphin`.

- [ ] **5.1 Write the failing tests.** In `profiles.rs` tests (after `embedded_kyty_profile_launches_fullscreen_and_names_the_ps5_platform`):

```rust
    fn embedded_dolphin() -> &'static EmulatorProfile {
        load_profiles()
            .iter()
            .find(|p| p.name == "Dolphin (GameCube / Wii)")
            .expect("the catalog ships a Dolphin profile")
    }

    #[test]
    fn embedded_dolphin_profile_launches_portable_fullscreen_vulkan_at_1080p() {
        let profile = embedded_dolphin();
        assert_eq!(
            profile.args,
            "-u \"%emu_dir%/User\" -b -v Vulkan -C Dolphin.Display.Fullscreen=True \
             -C GFX.Settings.InternalResolution=3 -e \"%rom%\""
        );
        assert_eq!(profile.user_data, vec!["User".to_string()]);
        assert_eq!(
            profile.screenshot_directories,
            vec!["User/ScreenShots".to_string()]
        );
        assert!(
            profile.save_directories.is_empty() && profile.state_directories.is_empty(),
            "the Dolphin readers own save resolution; a profile list would bypass them"
        );
        assert!(profile.firmware_directories.is_empty());
        assert!(profile.legacy_args.is_empty());
    }

    #[test]
    fn embedded_dolphin_matches_gamecube_and_wii_but_never_wii_u() {
        let keywords = &embedded_dolphin().platform_keywords;
        for platform in ["Nintendo GameCube", "GameCube", "ngc", "Nintendo Wii", "Wii"] {
            assert!(platform_matches_keywords(platform, keywords), "{platform}");
        }
        for platform in ["Nintendo Wii U", "Wii U", "wiiu", "Nintendo Switch"] {
            assert!(!platform_matches_keywords(platform, keywords), "{platform}");
        }
    }

    #[test]
    fn embedded_dolphin_is_offered_on_windows_and_linux_only() {
        let profile = embedded_dolphin();
        assert!(profile_available_on_host(profile, "win32"));
        assert!(profile_available_on_host(profile, "linux"));
        assert!(!profile_available_on_host(profile, "darwin"));
    }

    #[test]
    fn dolphin_binaries_resolve_to_the_dolphin_profile() {
        for exe in [
            r"C:\Emulators\Dolphin-x64\Dolphin.exe",
            "/lib/emulators/Dolphin (GameCube _ Wii)/Dolphin_Emulator-2609-anylinux-x86_64.AppImage",
            "/usr/bin/dolphin-emu",
        ] {
            assert_eq!(
                profile_for_entry("", exe, load_profiles()).map(|p| p.name.as_str()),
                Some("Dolphin (GameCube / Wii)"),
                "{exe}"
            );
        }
    }

    #[test]
    fn embedded_dolphin_source_is_direct_on_windows_and_github_on_linux() {
        use crate::launch::source::{allow_prerelease, resolve_source_for_host, str_field};
        let raw = embedded_dolphin().source.as_ref().unwrap();

        let windows = resolve_source_for_host(raw, "win32").unwrap();
        assert_eq!(str_field(&windows, "provider"), "direct");
        assert_eq!(
            str_field(&windows, "page_url"),
            "https://dolphin-emu.org/update/latest/beta/"
        );

        let linux = resolve_source_for_host(raw, "linux").unwrap();
        assert_eq!(str_field(&linux, "provider"), "github");
        assert_eq!(str_field(&linux, "owner"), "pkgforge-dev");
        assert_eq!(str_field(&linux, "repo"), "Dolphin-emu-AppImage");
        assert_eq!(str_field(&linux, "release_tag"), "latest");
        assert!(!allow_prerelease(&linux), "the nightly prerelease is never picked");
    }
```

  In `catalog.rs` tests:
  - In `real_catalog_includes_expected_rows_with_expected_fields`, before its end, add:
```rust
        let dolphin = by_name("Dolphin (GameCube / Wii)");
        assert_eq!(dolphin.provider, "direct");
        assert_eq!(dolphin.source_id, "dolphin-emu/dolphin");
```
  - Add:
```rust
    /// User ruling (2026-10-08): one Dolphin row on every OS that offers it.
    #[test]
    fn real_catalog_shows_exactly_one_dolphin_row_on_windows_and_linux_and_none_on_macos() {
        for (host, expected) in [("win32", 1usize), ("linux", 1), ("darwin", 0)] {
            let rows = catalog_entries_for_host(load_profiles(), host);
            let dolphin: Vec<&CatalogEntry> = rows
                .iter()
                .filter(|row| row.name.to_lowercase().contains("dolphin"))
                .collect();
            assert_eq!(dolphin.len(), expected, "host {host}: {dolphin:?}");
            if expected == 1 {
                assert_eq!(dolphin[0].name, "Dolphin (GameCube / Wii)");
                assert_eq!(dolphin[0].source_id, "dolphin-emu/dolphin");
            }
        }
    }
```

  In `emu_install.rs` tests:
```rust
    /// The Windows build ships helper executables beside `Dolphin.exe`; the
    /// title tokens score them all the same, so without the preferred name
    /// the shallower `DolphinTool.exe` would win the path tie-break.
    #[test]
    fn dolphin_title_prefers_dolphin_exe_over_its_bundled_tools() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path();
        touch(&install.join("DolphinTool.exe"));
        touch(&install.join("Dolphin-x64").join("Dolphin.exe"));
        touch(&install.join("Dolphin-x64").join("Updater.exe"));
        let picked = select_executable(
            "Dolphin (GameCube / Wii)",
            install,
            &install.join("absent.7z"),
        );
        assert_eq!(picked, Some(install.join("Dolphin-x64").join("Dolphin.exe")));
    }
```

- [ ] **5.2 Run:** `cargo test -p grid-core dolphin` (expect FAIL: no profile, wrong executable).

- [ ] **5.3 Implement the catalog entry.** In `emulator-autoprofiles.json`, insert directly after the `"Cemu (Wii U)"` object (after its closing `},`, before the Azahar object):

```json
  {
    "match_tokens": ["Dolphin.exe", "dolphin-emu", "dolphin-emu-nogui", "dolphin*.appimage"],
    "source": {
      "provider": "direct",
      "owner": "dolphin-emu",
      "repo": "dolphin",
      "release_tag": "latest",
      "platforms": ["win32", "linux"],
      "page_url": "https://dolphin-emu.org/update/latest/beta/",
      "download_url_regex": "https://dl\\.dolphin-emu\\.org/releases/[0-9A-Za-z._-]+/dolphin-[0-9A-Za-z._-]+-x64\\.7z",
      "platform_overrides": {
        "linux": {
          "provider": "github-release",
          "owner": "pkgforge-dev",
          "repo": "Dolphin-emu-AppImage",
          "release_tag": "latest",
          "asset_patterns": ["Dolphin_Emulator-*-anylinux-x86_64.AppImage"],
          "asset_exclude_patterns": ["*.zsync"]
        }
      }
    },
    "name": "Dolphin (GameCube / Wii)",
    "args": "-u \"%emu_dir%/User\" -b -v Vulkan -C Dolphin.Display.Fullscreen=True -C GFX.Settings.InternalResolution=3 -e \"%rom%\"",
    "all_platforms": false,
    "platform_keywords": ["gamecube", "ngc", "wii"],
    "use_game_title_as_name": false,
    "save_strategy": "single_file",
    "save_directories": [],
    "state_directories": [],
    "user_data": ["User"],
    "screenshot_directories": ["User/ScreenShots"]
  },
```

  The profile's `args` and the Rust test string must be byte-identical. The test uses a `\` line continuation. Before running, check that the joined string has exactly one space between `InternalResolution=3`'s preceding flag and `-C`. Write it as one line if in doubt.

- [ ] **5.4 Implement the executable preference.** In `emu_install.rs::select_executable`, after the ShadPS4 block:
```rust
    if title_casefold.contains("dolphin") {
        // The Windows build ships DolphinTool.exe and Updater.exe beside
        // Dolphin.exe, and every one of them carries the title token.
        preferred_names.insert("dolphin.exe");
    }
```
  Extend the module doc's parity note to "for KytyPS5, ShadPS4 and Dolphin".

- [ ] **5.5 Run:**
  - `cargo test -p grid-core dolphin`
  - `cargo test -p grid-core real_catalog`
  - `cargo test -p grid-core embedded_`
  - `cargo test -p grid-core every_catalog_source_block_with_a_recognized_provider_normalizes`
  - `cargo test -p grid-core resolve_source_matches_normalize_then_merge_for_the_catalog`
  - `cargo test -p grid-core emu_install`
  All pass.

- [ ] **5.6 Commit:**
```
git commit --only emulator-autoprofiles.json crates/grid-core/src/launch/emu_install.rs crates/grid-core/src/launch/profiles.rs crates/grid-core/src/launch/catalog.rs \
  -m "feat(catalog): Dolphin installs from the official 7z on Windows and the pkgforge AppImage on Linux" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 6: Pin Dolphin asset selection against captured payloads

**Files:** `crates/grid-core/src/launch/forge.rs` (tests only)

**Interfaces:**
- Consumes:
  - `fn scrape_download_url(page_text: &str, download_url_regex: &str, page_url: &str) -> Result<String, SourceError>`
  - `fn basename_of_url(url: &str) -> String`
  - `select_release`, `select_asset`, `str_field`
  - `crate::launch::source::resolve_source_for_host`
  - `ForgeClient::resolve(&self, raw: &Value, profile_name: &str) -> Result<ResolvedDownload, SourceError>`
- Produces: tests only.

- [ ] **6.1 Add the fixtures and tests** to `forge.rs`'s `mod tests` (append at the end of that module):

```rust
    // --- Dolphin: captured payloads ----------------------------------------------

    /// `https://dolphin-emu.org/update/latest/beta/` as served on 2026-10-08
    /// (`changelog_html` shortened). Artifact order is the real one: arm64
    /// comes BEFORE x64, and the only `href` sits inside an escaped string,
    /// so the scrape must fall through to the whole-page regex search.
    const DOLPHIN_LATEST_BETA_JSON: &str = r#"{"shortrev": "2609a", "date": "2026-10-08T00:57:28.587Z", "hash": "409881c34357d0f105fd473167d15ab0bd9c1628", "changelog_html": "<p>See the <a href=\"https://dolphin-emu.org/blog/2026/09/24/dolphin-progress-report-release-2609/\">Dolphin Progress Report: Release 2609!</a></p>", "artifacts": [{"system": "Linux x86_64 (Flatpak)", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-x86_64.flatpak"}, {"system": "Linux aarch64 (Flatpak)", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-aarch64.flatpak"}, {"system": "Android", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a.apk"}, {"system": "macOS (ARM/Intel Universal)", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-universal.dmg"}, {"system": "Windows arm64", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-arm64.7z"}, {"system": "Windows x64", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-x64.7z"}]}"#;

    const DOLPHIN_X64_URL: &str =
        "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-x64.7z";

    fn dolphin_source_for(host: &str) -> SourceMap {
        let profile = crate::launch::profiles::load_profiles()
            .iter()
            .find(|p| p.name == "Dolphin (GameCube / Wii)")
            .expect("the catalog ships a Dolphin profile");
        crate::launch::source::resolve_source_for_host(profile.source.as_ref().unwrap(), host)
            .unwrap()
    }

    #[test]
    fn dolphin_regex_picks_the_windows_x64_7z_out_of_the_update_feed() {
        let source = dolphin_source_for("win32");
        let url = scrape_download_url(
            DOLPHIN_LATEST_BETA_JSON,
            &str_field(&source, "download_url_regex"),
            &str_field(&source, "page_url"),
        )
        .unwrap();
        assert_eq!(url, DOLPHIN_X64_URL);
        assert_eq!(basename_of_url(&url), "dolphin-2609a-x64.7z");
    }

    #[test]
    fn dolphin_regex_finds_nothing_when_the_feed_has_no_x64_build() {
        let source = dolphin_source_for("win32");
        let without_x64 = DOLPHIN_LATEST_BETA_JSON.replace(
            r#", {"system": "Windows x64", "url": "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-x64.7z"}"#,
            "",
        );
        assert!(!without_x64.contains("-x64.7z"), "the fixture edit must apply");
        let url = scrape_download_url(
            &without_x64,
            &str_field(&source, "download_url_regex"),
            &str_field(&source, "page_url"),
        )
        .unwrap();
        assert_eq!(url, "", "arm64, flatpak, apk and dmg must never match");
    }

    /// End to end through `resolve`, with the catalog regex and a mock page
    /// that serves the JSON feed. A synthetic source without overrides keeps
    /// the `direct` provider on every test host.
    #[tokio::test]
    async fn a_direct_source_resolves_the_dolphin_x64_7z_from_the_json_feed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/update/latest/beta/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(DOLPHIN_LATEST_BETA_JSON, "application/json"),
            )
            .mount(&server)
            .await;
        let regex = str_field(&dolphin_source_for("win32"), "download_url_regex");
        let raw = json!({
            "provider": "direct", "owner": "dolphin-emu", "repo": "dolphin",
            "release_tag": "latest",
            "page_url": format!("{}/update/latest/beta/", server.uri()),
            "download_url_regex": regex,
        });

        let resolved = ForgeClient::new()
            .unwrap()
            .resolve(&raw, "Dolphin (GameCube / Wii)")
            .await
            .unwrap();
        assert_eq!(resolved.provider, "direct");
        assert_eq!(resolved.download_url, DOLPHIN_X64_URL);
        assert_eq!(resolved.asset_name, "dolphin-2609a-x64.7z");
        assert_eq!(resolved.release_tag, "latest");
    }

    /// `pkgforge-dev/Dolphin-emu-AppImage` releases, newest first: the
    /// rolling `nightly` prerelease, then the stable release. Every release
    /// carries aarch64 builds and `.zsync` files.
    fn dolphin_appimage_releases() -> Value {
        let asset = |tag: &str, name: &str| {
            json!({
                "name": name,
                "browser_download_url": format!(
                    "https://github.com/pkgforge-dev/Dolphin-emu-AppImage/releases/download/{tag}/{name}"
                ),
                "size": 1,
                "state": "uploaded",
            })
        };
        let stable = "2609%402026-10-01_1790876487";
        json!([
            {
                "tag_name": "nightly", "draft": false, "prerelease": true,
                "assets": [
                    asset("nightly", "Dolphin_Emulator_Nightly-a1b2c3d-anylinux-x86_64.AppImage"),
                    asset("nightly", "Dolphin_Emulator_Nightly-a1b2c3d-anylinux-x86_64.AppImage.zsync"),
                ]
            },
            {
                "tag_name": "2609@2026-10-01_1790876487", "draft": false, "prerelease": false,
                "assets": [
                    asset(stable, "Dolphin_Emulator-2609-anylinux-aarch64.AppImage"),
                    asset(stable, "Dolphin_Emulator-2609-anylinux-aarch64.AppImage.zsync"),
                    asset(stable, "Dolphin_Emulator-2609-anylinux-x86_64.AppImage.zsync"),
                    asset(stable, "Dolphin_Emulator-2609-anylinux-x86_64.AppImage"),
                ]
            }
        ])
    }

    #[test]
    fn dolphin_linux_source_picks_the_stable_x86_64_appimage() {
        let source = dolphin_source_for("linux");
        let releases = dolphin_appimage_releases();

        let release = select_release(&source, &releases).unwrap();
        assert_eq!(
            str_field(release, "tag_name"),
            "2609@2026-10-01_1790876487",
            "the nightly prerelease must be skipped"
        );
        let asset = select_asset(&source, release).unwrap();
        assert_eq!(
            str_field(asset, "name"),
            "Dolphin_Emulator-2609-anylinux-x86_64.AppImage"
        );

        // `/releases/latest` returns the stable release as a bare object.
        let latest = releases[1].clone();
        let release = select_release(&source, &latest).unwrap();
        let asset = select_asset(&source, release).unwrap();
        assert_eq!(
            str_field(asset, "name"),
            "Dolphin_Emulator-2609-anylinux-x86_64.AppImage"
        );
    }
```

- [ ] **6.2 Run:** `cargo test -p grid-core forge::tests::dolphin` and `cargo test -p grid-core a_direct_source_resolves_the_dolphin`. All pass. If `dolphin_regex_picks…` fails, the catalog regex in Task 5 is wrong: fix the JSON, not the test.

- [ ] **6.3 Commit:**
```
git commit --only crates/grid-core/src/launch/forge.rs \
  -m "test(forge): pin Dolphin asset selection against the live feed and release shapes" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 7: Integration test: Dolphin AppImage install runs portable from its own directory

**Files:** `crates/grid-core/tests/emulator_install.rs`

**Interfaces:**
- Consumes:
  - `grid_core::launch::profiles::load_profiles`
  - `grid_core::launch::spawn::prepare_emulator_launch`
  - `grid_core::launch::template::Placeholders`
  - harness helpers: `Harness::new`, `gitea_source`, `mount_widget`, `wait_terminal`, `install_dir`, `config`
- Produces: tests only.

- [ ] **7.1 Add the imports** `use grid_core::launch::spawn::prepare_emulator_launch;` and `use grid_core::launch::template::Placeholders;`. Add the test after `an_appimage_primary_is_kept_in_place_made_executable_and_recorded`:

```rust
/// The catalog's Dolphin profile, pointed at the mock forge (the github
/// host is hard-coded, so the source is swapped for a gitea one). A Linux
/// AppImage install lands in the profile-named directory, links `User`
/// into `saves/`, gets its settings written THROUGH that link, and its
/// stored args expand `-u` to that same directory.
#[cfg(unix)]
#[tokio::test]
async fn a_dolphin_appimage_install_runs_portable_from_its_own_directory() {
    let harness = Harness::new(|uri| {
        let mut dolphin = grid_core::launch::profiles::load_profiles()
            .iter()
            .find(|p| p.name == "Dolphin (GameCube / Wii)")
            .expect("the catalog ships Dolphin")
            .clone();
        dolphin.source = Some(gitea_source(uri));
        vec![dolphin]
    })
    .await;
    let asset = "Dolphin_Emulator-2609-anylinux-x86_64.AppImage";
    harness
        .mount_widget(asset, b"APPIMAGE-BYTES".to_vec(), 0)
        .await;

    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    // `/` is illegal in a directory name: sanitize_component makes it `_`.
    let install_dir = harness.install_dir("Dolphin (GameCube _ Wii)");
    let appimage = install_dir.join(asset);
    assert!(appimage.is_file(), "the AppImage is the install");
    assert_eq!(mode_of(&appimage), 0o755);

    let user = install_dir.join("User");
    assert!(
        fs::symlink_metadata(&user).unwrap().file_type().is_symlink(),
        "User must be a link into saves/"
    );
    assert_eq!(
        fs::canonicalize(&user).unwrap(),
        fs::canonicalize(
            harness
                .library
                .join("saves")
                .join("Dolphin (GameCube _ Wii)")
                .join("User")
        )
        .unwrap()
    );
    assert!(
        user.join("Config").join("Dolphin.ini").is_file(),
        "autoconfig writes Dolphin.ini through the link"
    );
    assert!(install_dir.join("portable.txt").is_file());

    let config = harness.config();
    let emu = &config.emulators[0];
    assert_eq!(emu.path, appimage.to_string_lossy());
    let rom = harness.library.join("Some Game.rvz");
    fs::write(&rom, b"rom").unwrap();
    let rom_text = rom.to_string_lossy().into_owned();
    let (argv, working_dir) = prepare_emulator_launch(
        &emu.name,
        Some(emu),
        &rom_text,
        &Placeholders {
            rom: rom_text.clone(),
            ..Default::default()
        },
        "",
        false,
    )
    .unwrap();
    assert_eq!(argv[0], appimage.to_string_lossy());
    assert_eq!(argv[1], "-u");
    assert_eq!(argv[2], format!("{}/User", install_dir.display()));
    assert_eq!(argv.last().unwrap(), &rom_text);
    assert_eq!(working_dir, install_dir);
}
```

- [ ] **7.2 Run:** `cargo test -p grid-core --test emulator_install a_dolphin_appimage_install`. It passes with Tasks 3-5 in place. If `Dolphin.ini` is missing, check `sync_autoconfig`'s warning in `entry.error` before changing anything.

- [ ] **7.3 Commit:**
```
git commit --only crates/grid-core/tests/emulator_install.rs \
  -m "test(library): Dolphin AppImage install links User and launches with -u to its own folder" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 8: KytyPS5 output resolution args and their migration

**Files:**
- `emulator-autoprofiles.json` (KytyPS5 object only)
- `crates/grid-core/src/launch/profiles.rs` (test at `:714-729`)
- `crates/grid-core/src/autoconfig/entry.rs` (test)

**Interfaces:**
- Consumes: `pub fn migrate_legacy_args(emulators: &mut [EmulatorEntry], profiles: &[EmulatorProfile]) -> usize` (run at startup by `app/src-tauri/src/lib.rs:111` via `migrate_legacy_args_in_config`).
- Produces: data change only.

- [ ] **8.1 Write the failing tests.**

  In `profiles.rs`, rewrite `embedded_kyty_profile_launches_fullscreen_and_names_the_ps5_platform` so its args assertions read:
```rust
        assert_eq!(
            profile.args,
            "--fullscreen --screen-width 1920 --screen-height 1080 --game \"%rom%\""
        );
        assert_eq!(
            profile.legacy_args,
            vec!["--fullscreen --game \"%rom%\"".to_string()]
        );
```
  Keep its two keyword assertions.

  In `entry.rs` tests (the `migrate_legacy_args` block):
```rust
    #[test]
    fn migrate_moves_an_installed_kyty_entry_to_the_1080p_args() {
        let profiles = crate::launch::profiles::load_profiles();
        let mut emulators = vec![EmulatorEntry {
            name: "KytyPS5 (Playstation 5)".into(),
            path: "/x/KytyPS5/kyty_emulator".into(),
            args: "--fullscreen --game \"%rom%\"".into(),
            ..Default::default()
        }];
        assert_eq!(migrate_legacy_args(&mut emulators, profiles), 1);
        assert_eq!(
            emulators[0].args,
            "--fullscreen --screen-width 1920 --screen-height 1080 --game \"%rom%\""
        );
        // A second pass finds nothing left to migrate.
        assert_eq!(migrate_legacy_args(&mut emulators, profiles), 0);
    }
```

- [ ] **8.2 Run:** `cargo test -p grid-core kyty` (expect FAIL).

- [ ] **8.3 Implement.** In the `"KytyPS5 (Playstation 5)"` object:
  - Set `"args": "--fullscreen --screen-width 1920 --screen-height 1080 --game \"%rom%\""`.
  - Add `"legacy_args": ["--fullscreen --game \"%rom%\""]` directly after `args`.
  - Change nothing else.

- [ ] **8.4 Run:** `cargo test -p grid-core kyty` and `cargo test -p grid-core migrate_`. All pass.

- [ ] **8.5 Commit:**
```
git commit --only emulator-autoprofiles.json crates/grid-core/src/launch/profiles.rs crates/grid-core/src/autoconfig/entry.rs \
  -m "feat(catalog): KytyPS5 renders at 1920x1080; old default args migrate at startup" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 9: PS5 install planning rejects `.pkg` and folder games, keeps `.zar` untouched

**Files:** `crates/grid-core/src/library/mod.rs`

**Interfaces:**
- Consumes:
  - `is_ps5_platform(&str) -> bool` (already imported)
  - `fn plan_install(detail: &RomDetail, library: &Path, client: Arc<RommClient>) -> Result<InstallJob, LibraryError>`
  - `fn base_finalize_route(platform: &str, archive: &Path) -> BaseRoute`
- Produces:
  - `const KYTY_UNSUPPORTED_GAME: &str`
  - `fn ps5_unsupported_download(platform: &str, candidates: &[&RomFile]) -> bool`

- [ ] **9.1 Write the failing tests** in `library/mod.rs` tests (after `plan_multi_file_without_an_m3u_launches_the_first_candidate`):

```rust
    // --- plan_install: PS5 ------------------------------------------------------

    fn ps5_detail(files: Vec<RomFile>) -> RomDetail {
        let mut detail = detail(files);
        detail.name = "Astro Bot".to_string();
        detail.platform_name = "PlayStation 5".to_string();
        detail
    }

    #[test]
    fn plan_rejects_a_ps5_pkg_before_any_download() {
        for name in ["Astro Bot.pkg", "ASTRO BOT.PKG"] {
            let Err(err) = plan_install(
                &ps5_detail(vec![rom_file(1, name, true)]),
                Path::new("/library"),
                client(),
            ) else {
                panic!("a PS5 {name} must be rejected");
            };
            assert_eq!(
                err.to_string(),
                "KytyPS5 needs a .zar archive, or a .zip/.7z that contains eboot.bin. \
                 .pkg files and folder games are not supported."
            );
        }
    }

    #[test]
    fn plan_rejects_a_ps5_folder_game() {
        let Err(err) = plan_install(
            &ps5_detail(vec![
                rom_file(1, "eboot.bin", true),
                rom_file(2, "libSceFios2.prx", true),
            ]),
            Path::new("/library"),
            client(),
        ) else {
            panic!("a bare PS5 folder must be rejected");
        };
        assert_eq!(err.to_string(), KYTY_UNSUPPORTED_GAME);
    }

    #[test]
    fn plan_accepts_a_single_ps5_zar_and_routes_it_as_downloaded() {
        // The metadata sidecar is not a download candidate, so it does not
        // turn a .zar game into a "folder".
        let job = plan_install(
            &ps5_detail(vec![
                rom_file(1, "Astro Bot.zar", true),
                rom_file(2, "game.json", true),
            ]),
            Path::new("/library"),
            client(),
        )
        .unwrap();
        assert_eq!(
            job.primary_archive,
            PathBuf::from("/library/games/PlayStation 5/Astro Bot.zar")
        );
        assert_eq!(job.file_ids, vec![1]);
        assert_eq!(
            base_finalize_route("PlayStation 5", &job.primary_archive),
            BaseRoute::Downloaded,
            "a .zar is opened in place by Kyty; it must never be extracted"
        );
        assert_eq!(
            base_finalize_route("PlayStation 5", Path::new("ASTRO.ZAR")),
            BaseRoute::Downloaded
        );
    }

    #[test]
    fn plan_keeps_the_ps5_zip_eboot_route() {
        let job = plan_install(
            &ps5_detail(vec![rom_file(1, "Astro Bot.zip", true)]),
            Path::new("/library"),
            client(),
        )
        .unwrap();
        assert_eq!(
            base_finalize_route("PlayStation 5", &job.primary_archive),
            BaseRoute::Eboot { ps4: false }
        );
    }

    #[test]
    fn plan_rejection_applies_to_ps5_only() {
        let mut ps4 = detail(vec![rom_file(1, "game.pkg", true)]);
        ps4.platform_name = "PlayStation 4".to_string();
        assert!(plan_install(&ps4, Path::new("/library"), client()).is_ok());

        let snes = detail(vec![rom_file(1, "disc1.bin", true), rom_file(2, "disc2.bin", true)]);
        assert!(plan_install(&snes, Path::new("/library"), client()).is_ok());
    }
```

  The first test's message string uses a `\` continuation. The continued line's leading spaces are stripped, so the text is `…eboot.bin. .pkg files…` with exactly one space. Check it against the constant.

- [ ] **9.2 Run:** `cargo test -p grid-core plan_` (expect FAIL: `KYTY_UNSUPPORTED_GAME` missing, the pkg plan succeeds).

- [ ] **9.3 Implement.** Next to `NO_DOWNLOADABLE_FILE` (`:108`):

```rust
/// Why a PlayStation 5 game cannot be installed for KytyPS5: Kyty opens a
/// `.zar` in place, or GRID extracts a `.zip`/`.7z` and launches its
/// `eboot.bin`. Raised by [`plan_install`], before any byte is requested.
const KYTY_UNSUPPORTED_GAME: &str = "KytyPS5 needs a .zar archive, or a .zip/.7z that contains eboot.bin. .pkg files and folder games are not supported.";

/// Whether a PS5 game's download set is one KytyPS5 cannot run: a single
/// `.pkg` (no `.pkg` support anywhere) or more than one file (a bare folder
/// game, whose nested files are never downloaded). Any other platform, and
/// an empty set (reported as [`NO_DOWNLOADABLE_FILE`]), is `false`.
fn ps5_unsupported_download(platform: &str, candidates: &[&RomFile]) -> bool {
    if !is_ps5_platform(platform) {
        return false;
    }
    match candidates {
        [] => false,
        [only] => Path::new(&only.file_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pkg")),
        _ => true,
    }
}
```
  In `plan_install`, directly after `let candidates: Vec<&RomFile> = …collect();` and before the `match`:
```rust
    if ps5_unsupported_download(&detail.platform_name, &candidates) {
        return Err(LibraryError::Extract(KYTY_UNSUPPORTED_GAME.to_string()));
    }
```
  Extend `plan_install`'s doc with "A PlayStation 5 `.pkg` or multi-file game is rejected here ([`KYTY_UNSUPPORTED_GAME`])."

- [ ] **9.4 Run:** `cargo test -p grid-core plan_` and `cargo test -p grid-core base_route`. All pass.

- [ ] **9.5 Commit:**
```
git commit --only crates/grid-core/src/library/mod.rs \
  -m "feat(library): reject PS5 .pkg and folder games at install planning; .zar installs as-is" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 10: Docs

**Files:**
- `future-plans/platform-support.md`
- `docs/superpowers/plans/2026-09-15-library-layout-v1.md`
- `.claude/skills/emulator-autoconfig/SKILL.md`

- [ ] **10.1 `future-plans/platform-support.md`.**
  - Replace lines 166-167 with:
```
- **MAME is not part of auto-install.** It remains playable through its RetroArch cores
  (`mame_libretro` / `mame2003_plus_libretro`).
- **Dolphin auto-installs** as `Dolphin (GameCube / Wii)`: the official portable `.7z` on
  Windows, the unofficial `pkgforge-dev/Dolphin-emu-AppImage` on Linux (the Flatpak is never
  used), not offered on macOS. It launches with `-u "%emu_dir%/User"`, so both builds keep
  their data in `<exe dir>/User`, linked into `saves/`.
```
  - Replace the Wii/GameCube row (`:238`) with:
```
| Wii/GameCube | Dolphin | Native (unofficial `pkgforge-dev` AppImage) | Auto-install: official `.7z` on Windows, AppImage on Linux; not offered on macOS |
```
  - Add after the PS4 row:
```
| PS5 | KytyPS5 | Native | Games must be `.zar`, or a `.zip`/`.7z` containing `eboot.bin`; `.pkg` and bare folder games are rejected at install |
```

- [ ] **10.2 `docs/superpowers/plans/2026-09-15-library-layout-v1.md`.** Append to Decision 9 (line 37) and to the "Dolphin shared directory" bullet (line 313):
```
**Superseded 2026-10-08:** Dolphin is now in the catalog as `Dolphin (GameCube / Wii)` with `user_data: ["User"]` (spec `docs/superpowers/specs/2026-10-08-dolphin-and-kyty-zar-design.md`).
```

- [ ] **10.3 `.claude/skills/emulator-autoconfig/SKILL.md`.** Under "Other Emulators", extend the Dolphin bullet with:
```
  - The catalog profile launches with `-u "%emu_dir%/User"` (`%emu_dir%` = the executable's directory, `launch::template`). `readers::dolphin_user_root_candidates` expands the same placeholder, so the Linux AppImage (whose own directory is a read-only mount) and the Windows `.7z` share one `<exe dir>/User` root. The profile has no `firmware_directories`: `ensure_gcpad_config`'s block is XInput-only.
```

- [ ] **10.4 Commit:**
```
git commit --only future-plans/platform-support.md docs/superpowers/plans/2026-09-15-library-layout-v1.md .claude/skills/emulator-autoconfig/SKILL.md \
  -m "docs: Dolphin is in the catalog; PS5 accepts .zar" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 11: Gate, e2e regression, live check

- [ ] **11.1 Run the full gate** from the repo root, in order. Run it detached and read the log.
```
scripts/check_secret_hygiene.sh
cargo fmt --check
cd app && npm ci && npx svelte-check && npm run build
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo test --workspace
cd app && npm test
```
  Fix any failure only in the files this plan touched. Re-run the failed command, then the rest of the gate.
- [ ] **11.2 E2E regression.** Run `scripts/e2e.sh emulator-catalog`. It needs a full build because Rust changed. Then run `E2E_SKIP_BUILD=1 scripts/e2e.sh launch`. That is safe because nothing changed since that build. No spec or fixture changes are expected: no spec lists catalog rows, and the mock forge 404s Dolphin.
- [ ] **11.3 Commit any gate fixes** with `git commit --only <touched paths>` and the same trailer.
- [ ] **11.4 Manual live check (the user's machine).**
  - Install Dolphin from the catalog. Expect one row.
  - Launch one GameCube and one Wii game. Expect fullscreen, Vulkan, 3x IR, and data in `emulators/Dolphin (GameCube _ Wii)/User` → `saves/…`.
  - Restart GRID and confirm the KytyPS5 entry's args migrated.
  - Launch one `.zar` game.
  - Click Install on a `.pkg` PS5 game. Expect the exact rejection text and no download row.
- [ ] **11.5** If the orchestrator closes the milestone here, run `cargo clean --profile dev` from the repo root (standing milestone cleanup).

---

## Edge cases to handle (covered above unless noted)

- **An override with `provider` but a blank `owner`.** `normalize_source` errors after the merge. The catalog test `resolve_source_matches_normalize_then_merge_for_the_catalog` resolves every source on every host.
- **`tag` vs `release_tag`.** `normalize_source` reads `tag` first, so a top-level `tag` now beats an override's `release_tag`. No catalog entry uses `tag`. The regression test would catch a future one.
- **`%emu_dir%` validation.** A path with spaces stays one argv element (Task 3). A bare file name or `/` gives `EMU_DIR_MISSING`, never `-u /User`.
- **Hand-configured Dolphin entries.** A manual Dolphin entry now matches the new profile by token. Saves still come from the readers. Its own args are kept: `apply_manual_emulator_profile_defaults` replaces args only when blank or `%rom%`.
- **Windows `.7z` nested in `Dolphin-x64/`.** `user_data_root` and `%emu_dir%` both use the executable's parent, so the links and `-u` agree.
- **`.zar` with uppercase suffix.** Covered: no `zar` suffix ever reaches the extractor.
- **PS5 game with a `game.json` sidecar.** Not a candidate (`is_download_candidate`), so it is not counted as a folder game (Task 9).
- **PS5 folder game that lists only one top-level file.** It is downloaded alone and will fail at launch. The spec rule is "more than one file". Not handled (Open Question 6).
- **Future feed revs with hyphens.** The character class allows `-`, `_` and `.`. A rev the class cannot match fails the install with "did not resolve a download URL"; it never picks a wrong file.

## Open questions

1. **Folder name (answer before Task 5).** `/` cannot appear in a directory name, so the install and saves folders become `Dolphin (GameCube _ Wii)`. Keep the spec name, or rename the profile now (for example `Dolphin (GameCube, Wii)`)? Renaming later needs a saves-folder migration. Default if unanswered: keep the spec name.
2. **Versioned AppImage names on update (pre-existing).** The download step unlinks only an AppImage with the SAME file name (`library/mod.rs:2585`). After a Dolphin update, `Dolphin_Emulator-2609-…` and `Dolphin_Emulator-2612-…` sit side by side. `select_executable` then breaks the tie by path text and keeps the OLDER one. PCSX2, PPSSPP, Cemu, Eden and xemu already have this gap. Recommend a separate fix: prefer the just-downloaded primary when it is launchable.
3. **Dolphin firmware and the GCPad block.** Firmware routing stays off (Decision 5). A follow-up could route `dsp_rom.bin`, `dsp_coef.bin` and `font_*.bin` to `User/GC`. It could route `IPL.bin` per region. It needs a non-XInput GCPad default on Linux.
4. **Linux catalog meta line.** The row shows `direct • latest` while Linux installs from GitHub (Decision 2). Is that acceptable, or should the row show the host-resolved provider while keeping the top-level `source_id`?
5. **Copy changes not made.**
   - `ARGS_LABEL` (`app/src/lib/emulators/form.ts:13`, pinned by `form.test.ts` and `e2e/specs/emulators.spec.ts:222`) does not list `%emu_dir%`.
   - The KytyPS5 note (`app/src/lib/emulators/notes.ts:47-49`) does not mention `.zar`. Both are UI copy, which belongs to the designer.
6. **Single-file PS5 folder games.** Should a PS5 game whose only candidate is a bare `eboot.bin` also be rejected?
7. **Windows update check.** It says "unknown" for Dolphin (direct provider), the same as RetroArch and Redream. Reading `shortrev` from the feed would make it real. Out of scope.

## Files in this plan

- `/home/six/Documents/Programming/grid-launcher/emulator-autoprofiles.json`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/source.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/forge.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/catalog.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/profiles.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/template.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/spawn.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/mod.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/launch/emu_install.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/autoconfig/readers.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/autoconfig/entry.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/cloud/dirs.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/src/library/mod.rs`
- `/home/six/Documents/Programming/grid-launcher/crates/grid-core/tests/emulator_install.rs`
- `/home/six/Documents/Programming/grid-launcher/app/src-tauri/src/commands.rs`
- `/home/six/Documents/Programming/grid-launcher/future-plans/platform-support.md`
- `/home/six/Documents/Programming/grid-launcher/docs/superpowers/plans/2026-09-15-library-layout-v1.md`
- `/home/six/Documents/Programming/grid-launcher/.claude/skills/emulator-autoconfig/SKILL.md`

