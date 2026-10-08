# Dolphin auto-install and KytyPS5 `.zar` games: implementation plan (2026-10-08, revised)

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax. One coder per task, with a reviewer after each. Tasks run in the order listed. A task may assume that every earlier task is committed. Each task lists its own files, interfaces and context, so read only your own task plus "Global Constraints". Commit each verified task with `git commit --only <paths>`. Never run `git checkout`, `git restore`, `git reset` or `git stash` on tracked files.

**Goal:**
- GRID installs Dolphin from the catalog:
  - On Windows it uses the official portable `.7z`.
  - On Linux it uses the unofficial `pkgforge-dev` AppImage.
  - It is not offered on macOS.
- Dolphin games launch fullscreen, on Vulkan, at 3x internal resolution, with user data in `<exe dir>/User`.
- KytyPS5 gets explicit 1080p output args.
- PS5 games:
  - A `.zar` installs as downloaded, without extraction.
  - A `.pkg` or a bare multi-file folder game is rejected before any download.
- After an update of any AppImage emulator, GRID launches the newly downloaded AppImage and removes the one it replaced.

**Architecture:**
- **One catalog row on every OS.** A `platform_overrides` entry is merged into the raw source before normalization. This lets one profile switch provider: `direct` on Windows, `github` on Linux. The row still comes from the top-level source identity.
- **`%emu_dir%` placeholder.** It expands to the executable's directory. `prepare_emulator_launch` fills it in. The Dolphin save reader expands the same placeholder.
- **PS5 checks.** They run in `plan_install`, before admission.
- **AppImage updates.** A primary AppImage IS the executable, so the directory scan never ranks it against a stale one. The entry's previous AppImage in the same install directory is removed after the config write.

**Tech Stack:** Rust (grid-core, Tauri 2 shell), serde_json (`preserve_order`), regex, wiremock (tests), vitest unchanged, WebdriverIO e2e (regression only).

**Spec:** `docs/superpowers/specs/2026-10-08-dolphin-and-kyty-zar-design.md`.

**Gate (repo root, `CLAUDE.md` order):** `scripts/check_secret_hygiene.sh`, `cargo fmt --check`, `cd app && npm ci && npx svelte-check && npm run build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`, `cd app && npm test`.

---

## Decisions taken while checking the code

These settle the points the spec left to the planner. Each one binds the coder.

1. **The override can switch provider (no second profile).**
   - Today `forge.rs::resolve` (`:92-93`) and `commands.rs::update_source_for` (`:1705-1706`) normalize first and merge the override after.
   - A `provider` key in an override therefore lands un-normalized (`"github-release"`) and fails with "Unsupported source provider".
   - The fix is a new `source::resolve_source_for_host(raw, host)`. It merges the override into the RAW object with the existing `merge_platform_override_for`, then calls `normalize_source`.
   - A regression test pins that every catalog override that does not set `provider` gives the same map as before.

2. **The catalog row keeps the top-level identity.**
   - `catalog::catalog_row` keeps reading the top-level `provider`/`owner`/`repo`, as it already does for RPCS3.
   - So `source_id` stays `dolphin-emu/dolphin` on every host, and `find_profile` finds the profile from any host.
   - Side effect: the Linux row's meta line reads `direct • latest` (Open Question 4).

3. **Dolphin save lists stay empty, on purpose.**
   - `cloud/dirs.rs:484` asks the Dolphin readers only when the entry's `save_paths`/`state_paths` are blank.
   - A catalog install copies the profile's lists into the entry (`autoconfig::entry::apply_manual_emulator_profile_defaults`). Empty profile lists therefore keep the readers in charge.
   - The readers handle memory-card permutations, GCI region folders, Wii title groups and `Dolphin.ini` overrides.
   - `readers::dolphin_launch_user_root` (`readers.rs:621`) reads the `-u` value. Today it would read the literal `%emu_dir%/User`, so Task 4 makes it expand the placeholder.

4. **`user_data: ["User"]`, `screenshot_directories: ["User/ScreenShots"]`.**
   - The data root is the executable's directory for every non-PCSX2 binary (`autoconfig::emulator_data_root`).
   - That holds for the Windows `.7z` (which may nest the exe as `Dolphin-x64/`) and for the Linux AppImage.
   - Both match `-u "%emu_dir%/User"` and `dolphin::ini_path_candidates` candidate 0 (`<exe parent>/User/Config`).
   - `ensure_user_data_links` links that `User` into `saves/<profile>/User`.

5. **No `firmware_directories` for Dolphin.**
   - `firmware::routing::install_for_game` (`routing.rs:428`) returns early when there are no targets. The Dolphin hooks (`:472-477`) therefore run only when targets exist.
   - `ensure_gcpad_config` writes an XInput-only block, which would break the default Linux mapping.
   - Leaving firmware routing off keeps today's behaviour, where no Dolphin entry ever reaches those hooks.
   - IPL support needs per-region `User/GC/<REGION>/IPL.bin` paths. That is a follow-up (Open Question 3).

6. **`save_strategy: "single_file"`.** `cloud/ops/mod.rs:804` gives Dolphin its own branch, so the strategy is not read for Dolphin saves.

7. **Executable choice.** `Dolphin.exe` and `DolphinTool.exe` score the same, so the shallower path wins. `select_executable` gains the preferred name `dolphin.exe` for a title containing `dolphin`.

8. **The profile name is `Dolphin (GameCube, Wii)` (user ruling 2026-10-08).** I checked that the comma is safe at every step:
   - **Directory names.** `library::paths::sanitize_component` replaces only `<>:"/\|?*` and control characters, so it keeps the comma. The install dir is `emulators/Dolphin (GameCube, Wii)/` and the saves dir is `saves/Dolphin (GameCube, Wii)/`.
   - **Args.** `split_template` splits the TEMPLATE before substitution. The expanded path is a single argv element handed to `std::process::Command` with no shell.
   - **Links.** `ensure_user_data_links` builds a relative symlink or junction, and neither cares about commas.
   - **Cloud paths.** The cloud lists split on `;`, `\r` and `\n` only (`cloud/dirs.rs:189`, `cloud/candidates.rs:114`, `layout_migration.rs:117`).
   - **Other comma splits.** The only `split(',')` calls are the D-Bus address (`spawn.rs:336`) and the frontend's company names.
   - **Config.** TOML strings take commas.
   - Tasks 3 and 6 pin this.

9. **The feed regex.**
   - Captured live on 2026-10-08: the payload is Python-style JSON with unescaped `/`, and arm64 is listed BEFORE x64.
   - The href walk finds nothing: the only `href` is `href=\"…` inside `changelog_html`.
   - The whole-page fallback therefore returns the whole match.
   - The regex is `https://dl\.dolphin-emu\.org/releases/[0-9A-Za-z._-]+/dolphin-[0-9A-Za-z._-]+-x64\.7z`, compiled case-insensitive.
   - The character class excludes `/` and `"`, so a match cannot cross into a neighbouring artifact.

10. **`.zar` needs no routing code.** `extract::EXTRACTABLE_SUFFIXES` does not list `zar`, so the route is already `Downloaded`. Task 10 pins it.

11. **PS5 rules (user ruling 2026-10-08).**
    - Rejected: a single `.pkg`, or more than one download candidate.
    - Accepted and routed `Downloaded`: a single non-`.pkg`, non-archive file, such as a bare `eboot.bin`.
    - The message surfaces verbatim: `LibraryError::Extract` displays `{0}`, and `Server.svelte:271-274` and `Details.svelte:416-419` show it unchanged.

12. **No Dolphin e2e stage.** `e2e/mock-romm/mock-forge.mjs` serves only PCSX2, Redream and GRID's own release, and the spec allows no new mock routes. The `emulator-catalog` and `launch` groups run as regression.

13. **Versioned AppImage updates (user ruling 2026-10-08).** Three parts of the current code combine into the bug:
    - The download step unlinks only a same-named primary (`library/mod.rs:2585`).
    - `finalize_emulator` then calls `select_executable` over the whole install directory (`:2290`).
    - The old and new AppImage score the same, and the casefolded-path tie-break prefers `…-2609-…` over `…-2612-…`.

    The fix has two parts:
    - **(a)** A non-extracted AppImage primary becomes the executable directly, with no scan.
    - **(b)** After the config write, the entry's PREVIOUS executable is deleted, but only when all of these hold: it is an AppImage, a regular file (not a link or directory), directly inside the same install directory, and not the file just written.

    Nothing else is ever deleted: no user data, no stray AppImages, no other files. This applies to every AppImage emulator: PCSX2, PPSSPP, Cemu, Eden, xemu, Xenia Canary/Edge, Dolphin and any future one.

---

## Global Constraints

- **Never destroy work:** no `git checkout`, `git restore`, `git reset`, `git stash` on tracked files.
- **Commits:** `git commit --only <paths>` with the paths listed in the task. Every message ends with the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **grid-core stays Tauri-free.** Only Task 1 touches `app/src-tauri` (one import and one call site).
- **Secrets:** nothing here reads or logs a credential. Fixtures contain no `token`/`password`/`Bearer` strings.
- **Catalog JSON is compiled in** (`include_str!` in `profiles.rs` and `source.rs` tests). Editing it is a rebuild.
- **Formatting:** run `cargo fmt` before each commit and commit only the task's paths.
- **No RomM request changes** (`openapi.json` not involved). No IPC payload shape changes.
- **Autoconfig writers are untouched.** `autoconfig/dolphin.rs` keeps its overwrite policy and its Vulkan seed.
- **The Dolphin profile name is exactly `Dolphin (GameCube, Wii)`.** Its install and saves folders have the same name.

## Review Focus

These are the top five failure modes that today's tests do not cover. Each one gets a test in its owning task.

1. **Linux Dolphin installs the Windows `.7z`.** The override fails to switch provider, so Linux scrapes the Windows feed.
   - Task 1 `resolve_source_for_host_lets_an_override_switch_the_provider`
   - Task 6 `embedded_dolphin_source_is_direct_on_windows_and_github_on_linux`
2. **After an AppImage update GRID still launches the old build, or the cleanup deletes the wrong file.**
   - Task 5 `an_appimage_update_with_a_new_file_name_launches_the_new_build_and_removes_the_old_one`. It checks that a stray AppImage, a stray file and the user data behind the `User` link all survive.
   - Task 5 unit tests for `superseded_appimage` (outside the dir, a link, a directory, the same file).
3. **The feed regex picks `-arm64.7z`, a `.flatpak` or the `.dmg`.**
   - Task 7 `dolphin_regex_picks_the_windows_x64_7z_out_of_the_update_feed` (real artifact order)
   - Task 7 `dolphin_regex_finds_nothing_when_the_feed_has_no_x64_build`
4. **Cloud saves read the literal `%emu_dir%/User` and fall back to a stale `~/.dolphin-emu`.**
   - Task 4 `dolphin_emu_dir_user_flag_wins_over_a_stale_home_install`. It writes no `portable.txt`, so the fallback cannot hide the bug.
5. **A PS5 `.pkg` or folder game starts downloading, a `.zar` gets extracted, or a bare `eboot.bin` is wrongly rejected.**
   - Task 10 `plan_rejects_a_ps5_pkg_before_any_download`
   - Task 10 `plan_rejects_a_ps5_folder_game`
   - Task 10 `plan_accepts_a_single_ps5_zar_and_routes_it_as_downloaded`
   - Task 10 `plan_accepts_a_single_bare_ps5_executable_and_routes_it_as_downloaded`

Also pinned:
- **Exactly one Dolphin row** on win32 and linux, and none on darwin (hard user requirement): Task 6 `real_catalog_shows_exactly_one_dolphin_row_on_windows_and_linux_and_none_on_macos`.
- **Windows Dolphin launches `Dolphin.exe`, not `DolphinTool.exe`:** Task 6.
- **`%emu_dir%` with spaces and a comma stays one argument, and a blank value is an error:** Task 3.

---

## Task 1: Platform overrides can switch the source provider

**Context:** Today the override is merged after normalization (`forge.rs:92-93`, `commands.rs:1705-1706`), so an override cannot change `provider`. This task makes normalization run after the merge.

**Files:**
- `crates/grid-core/src/launch/source.rs` (new fns + tests)
- `crates/grid-core/src/launch/forge.rs` (`ForgeClient::resolve`, imports at `:24-27`)
- `crates/grid-core/src/launch/profiles.rs` (test helper `win32_source` at `:1443-1455` only)
- `app/src-tauri/src/commands.rs` (`update_source_for` at `:1696-1716`, imports at `:23-25`)

**Interfaces:**
- Consumes:
  - `pub fn normalize_source(raw: &Value) -> Result<SourceMap, SourceError>`
  - `pub(crate) fn merge_platform_override_for(source: &mut SourceMap, host: &str)`
  - `pub const HOST_PLATFORM: &str`
- Produces:
  - `pub fn resolve_source_for_host(raw: &Value, host: &str) -> Result<SourceMap, SourceError>`
  - `pub fn resolve_source(raw: &Value) -> Result<SourceMap, SourceError>`

- [ ] **1.1 Write the failing tests** in `source.rs`'s `#[cfg(test)] mod tests`, after the `merge_platform_override` block:

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

- [ ] **1.2 Run** `cargo test -p grid-core resolve_source`. Expected: a compile failure, because `resolve_source_for_host` does not exist yet.

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

- [ ] **1.4 Switch the callers.**
  - **`forge.rs` imports:** replace `merge_platform_override, normalize_source` with `resolve_source`.
  - **`ForgeClient::resolve` body:** replace `let mut source = normalize_source(raw)?; merge_platform_override(&mut source);` with `let source = resolve_source(raw)?;`.
  - **`ForgeClient::resolve` text:** change the comment and the `expect` text that name `normalize_source` to name `resolve_source`. The doc becomes: "resolve for this host (override merged into the raw source, then normalized), then dispatch on provider."
  - **`commands.rs:23-25`:** replace the import with `use grid_core::launch::source::{allow_prerelease, resolve_source, str_field, SourceMap};`.
  - **`update_source_for`:** use `let source = resolve_source(&raw).map_err(|e| e.0)?;`. The doc says "resolved the same way the install resolves it".
  - **`profiles.rs` `win32_source`:** the body ends with `crate::launch::source::resolve_source_for_host(raw, "win32").unwrap()`.
  - Keep `merge_platform_override` (pub) and its tests.

- [ ] **1.5 Run:**
  - `cargo test -p grid-core resolve_source`
  - `cargo test -p grid-core embedded_sources_pick_the_windows_asset_from_real_release_listings`
  - `cargo test -p grid-core forge`
  - `cargo test -p app`
  - `cargo clippy --workspace --all-targets -- -D warnings`

  Expected: all pass, with no unused-import warning.

- [ ] **1.6 Commit:**
```
git commit --only crates/grid-core/src/launch/source.rs crates/grid-core/src/launch/forge.rs crates/grid-core/src/launch/profiles.rs app/src-tauri/src/commands.rs \
  -m "feat(launch): platform overrides can switch the source provider" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 2: Catalog rows can be built for any host

**Context:** `catalog_row`'s `platforms` gate reads the compile-time `HOST_PLATFORM`. A later task must pin "one Dolphin row on win32 and linux, none on darwin" from one test run, so the host becomes a parameter.

**Files:**
- `crates/grid-core/src/launch/catalog.rs`

**Interfaces:**
- Produces (module-private, used by this module's tests):
  - `fn catalog_entries_for_host(profiles: &[EmulatorProfile], host: &str) -> Vec<CatalogEntry>`
  - `fn catalog_row(profile: &EmulatorProfile, host: &str) -> Option<CatalogEntry>`
  - `fn platforms_allow_host(source: &Map<String, Value>, host: &str) -> bool`
  - `fn catalog_entries_filtered(profiles: &[EmulatorProfile], compat: bool, host: &str) -> Vec<CatalogEntry>`
- These public signatures do not change: `catalog_entries`, `compat_tool_catalog_entries`, `find_profile`, `find_compat_profile`.

- [ ] **2.1 Write the failing test** in `catalog.rs` tests, after `platforms_gate_is_a_noop_when_the_list_is_empty`:

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

- [ ] **2.2 Run** `cargo test -p grid-core one_profile_with_a_provider_switching_override`. Expected: a compile failure.

- [ ] **2.3 Implement.**
  - **`platforms_allow_host(source, host: &str)`:** replace `HOST_PLATFORM.starts_with(text.as_str())` with `host.starts_with(text.as_str())`.
  - **`catalog_row(profile, host: &str)`:** pass `host` to `platforms_allow_host`.
  - **`catalog_entries_filtered(profiles, compat, host: &str)`:** pass `host` to `catalog_row`.
  - **Add:**
```rust
/// [`catalog_entries`] as `host` (a `sys.platform`-shaped slug) would list
/// it: the same rows, gated by that host's `platforms` allowlist instead of
/// this build's.
fn catalog_entries_for_host(profiles: &[EmulatorProfile], host: &str) -> Vec<CatalogEntry> {
    catalog_entries_filtered(profiles, false, host)
}
```
  - **`catalog_entries`:** return `catalog_entries_for_host(profiles, HOST_PLATFORM)`.
  - **`compat_tool_catalog_entries`:** pass `HOST_PLATFORM`.
  - **`find_profile_filtered`:** call `catalog_row(profile, HOST_PLATFORM)`.

- [ ] **2.4 Run** `cargo test -p grid-core catalog`. Expected: all pass.

- [ ] **2.5 Commit:**
```
git commit --only crates/grid-core/src/launch/catalog.rs \
  -m "refactor(catalog): build catalog rows for an explicit host" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 3: `%emu_dir%` launch placeholder

**Context:** Dolphin will launch with `-u "%emu_dir%/User"`. The placeholder expands to the directory that holds the executable. A blank value must fail the launch rather than produce `-u /User`. A path with spaces or a comma must stay one argument.

**Files:**
- `crates/grid-core/src/launch/template.rs`
- `crates/grid-core/src/launch/spawn.rs`
- `crates/grid-core/src/launch/mod.rs` (the `Placeholders` literal in `resolve_launch`, `:551-555`)

**Interfaces:**
- Produces:
  - `pub struct Placeholders { pub rom: String, pub core: String, pub ps3_launch_target: String, pub emu_dir: String }` (derives `Default`, as today)
  - `pub const EMU_DIR_PLACEHOLDER: &str = "%emu_dir%";`
  - `pub const EMU_DIR_MISSING: &str`
  - `pub fn emulator_dir_placeholder(executable: &Path) -> String`
- These signatures do not change: `build_args`, `validate_placeholders`, `apply_placeholders`, `spawn::prepare_emulator_launch`.

- [ ] **3.1 Write the failing tests.**

  In `template.rs` tests, add `emu_dir: String::new(),` to the helper `placeholders(rom, core, ps3)`, then add:

```rust
    // --- %emu_dir% ------------------------------------------------------------

    #[test]
    fn emu_dir_is_substituted_and_a_path_with_spaces_and_a_comma_stays_one_argument() {
        let ph = Placeholders {
            rom: "/roms/Some Game.rvz".to_string(),
            emu_dir: "/lib/emulators/Dolphin (GameCube, Wii)".to_string(),
            ..Default::default()
        };
        let result = build_args("-u \"%emu_dir%/User\" -b -e \"%rom%\"", "", &ph).unwrap();
        assert_eq!(
            result,
            vec![
                "-u",
                "/lib/emulators/Dolphin (GameCube, Wii)/User",
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

  In `spawn.rs` tests, add `emu_dir: String::new(),` to the helper `placeholders(rom, core)`, then add:

```rust
    #[test]
    fn emu_dir_expands_to_the_executable_folder_as_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let emu_dir = dir.path().join("Dolphin (GameCube, Wii)");
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

- [ ] **3.2 Run** `cargo test -p grid-core emu_dir`. Expected: a compile failure.

- [ ] **3.3 Implement in `template.rs`.**
  - Add `pub emu_dir: String,` to `Placeholders` with the doc: "The directory holding the emulator's executable (`%emu_dir%`). Filled by `spawn::prepare_emulator_launch`; callers leave it blank."
  - Above `validate_placeholders`, add:

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
  - At the end of `validate_placeholders`, add:
```rust
    if template.contains(EMU_DIR_PLACEHOLDER) && ph.emu_dir.trim().is_empty() {
        return Err(EMU_DIR_MISSING.to_string());
    }
```
  - In `apply_placeholders`, append `.replace(EMU_DIR_PLACEHOLDER, &ph.emu_dir)` after the `%ps3_launch_target%` replace, and update the doc's list.
  - In the module doc, name `%emu_dir%` as a GRID addition.

- [ ] **3.4 Implement in `spawn.rs`.**
  - The import becomes `use super::template::{build_args, emulator_dir_placeholder, normalized_retroarch_core_args, Placeholders};`.
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
  - Add one line to its doc: "`%emu_dir%` is filled from the resolved executable's parent."

- [ ] **3.5 Implement in `launch/mod.rs`.** In `resolve_launch`, add `emu_dir: String::new(), // filled by prepare_emulator_launch from the resolved executable` to the `Placeholders` literal.

- [ ] **3.6 Run:**
  - `cargo test -p grid-core template`
  - `cargo test -p grid-core spawn`
  - `cargo test -p grid-core --test launch_service`
  - `cargo test -p grid-core --test launch_tilde`

  Expected: all pass.

- [ ] **3.7 Commit:**
```
git commit --only crates/grid-core/src/launch/template.rs crates/grid-core/src/launch/spawn.rs crates/grid-core/src/launch/mod.rs \
  -m "feat(launch): %emu_dir% expands to the emulator executable's folder" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 4: The Dolphin save reader expands `%emu_dir%` in `-u`

**Context:**
- Cloud saves for a Dolphin entry with blank `save_paths` come from `readers::dolphin_save_path_overrides`. That reader reads the `-u` flag from the entry's args through `dolphin_launch_user_root`.
- Dolphin's args will contain `-u "%emu_dir%/User"`. The reader must expand the placeholder the same way a launch does (Task 3 added `crate::launch::template::EMU_DIR_PLACEHOLDER`).

**Files:**
- `crates/grid-core/src/autoconfig/readers.rs` (`dolphin_launch_user_root` at `:621`, `dolphin_user_root_candidates` at `:678`, tests)
- `crates/grid-core/src/cloud/dirs.rs` (one test only)

**Interfaces:**
- Consumes: `crate::launch::template::EMU_DIR_PLACEHOLDER`, `crate::launch::template::split_template`.
- Produces (private):
  - `fn dolphin_launch_user_root(args: Args, emulator_dir: Option<&Path>) -> Option<PathBuf>`
  - `fn expand_emu_dir(value: &str, emulator_dir: Option<&Path>) -> Option<String>`
- This public signature does not change: `dolphin_user_root_candidates(path: &str, args: Args) -> Vec<PathBuf>`.

- [ ] **4.1 Write the failing tests.**

  In the `readers.rs` tests Dolphin block, after `dolphin_user_root_prefers_a_launch_user_flag`:

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

  In `cloud/dirs.rs` tests, after `dolphin_override_wiring_lands_ahead_of_profile_paths`:

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

        let emulator_dir = temp.path().join("Dolphin (GameCube, Wii)");
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
        let mut e = entry("Dolphin (GameCube, Wii)", &exe.to_string_lossy());
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

- [ ] **4.2 Run** `cargo test -p grid-core dolphin_user_root_expands` and `cargo test -p grid-core dolphin_emu_dir_user_flag_wins`. Expected: both FAIL.

- [ ] **4.3 Implement in `readers.rs`.**
  - Above `dolphin_launch_user_root`, add:

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
  - In both the `-u`/`--user` branch and the `--user=` branch, replace `if !cleaned.is_empty() { return Some(resolve_best_effort(&paths::expand_user(&cleaned))); }` with:
```rust
                if let Some(value) =
                    expand_emu_dir(&cleaned, emulator_dir).filter(|v| !v.is_empty())
                {
                    return Some(resolve_best_effort(&paths::expand_user(&value)));
                }
```
  - Add to its doc: "A value naming `%emu_dir%` is expanded against `emulator_dir`, or skipped when that is unknown."
  - In `dolphin_user_root_candidates`, call `dolphin_launch_user_root(args, emulator_dir.as_deref())`. `emulator_dir` is computed just above. Add to its doc: "`-u` values expand `%emu_dir%` exactly as a launch does."

- [ ] **4.4 Run** `cargo test -p grid-core dolphin` and `cargo test -p grid-core cloud::dirs`. Expected: all pass.

- [ ] **4.5 Commit:**
```
git commit --only crates/grid-core/src/autoconfig/readers.rs crates/grid-core/src/cloud/dirs.rs \
  -m "fix(cloud): Dolphin save reader expands %emu_dir% in the -u flag" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 5: An AppImage update launches the new build and removes the one it replaced

**Context:**
- **Why the old build survives.** Most AppImage assets carry a version in their file name: PCSX2 `pcsx2-v2.1.0-…`, PPSSPP, Cemu, Eden, xemu, and Dolphin `Dolphin_Emulator-2609-…`. The download step (`library/mod.rs:2585`) unlinks only a same-named primary. An update therefore leaves the old AppImage beside the new one.
- **Why the old build is picked.** `finalize_emulator` (`:2290`) calls `emu_install::select_executable`, which scans the whole install directory. The two AppImages tie on every rank field except the casefolded path. So the OLDER one (`…2609…` < `…2612…`) becomes the entry's path.
- **Rule (user ruling 2026-10-08):**
  - GRID launches the newly downloaded AppImage.
  - Only the AppImage the entry pointed at before this install is deleted.
  - User data, links, stray files and AppImages the entry never pointed at are never deleted.

**Files:**
- `crates/grid-core/src/library/mod.rs` (`InstallService::finalize_emulator` at `:2210-2344`, new helper, unit tests)
- `crates/grid-core/tests/emulator_install.rs` (integration test)

**Interfaces:**
- Consumes (all exist in `library/mod.rs`):
  - `fn is_appimage(path: &Path) -> bool` (`:3095`)
  - `fn delete_with_retry(path: &Path) -> bool`
  - `fn append_warning(warning: &mut String, text: &str)`
  - `should_extract`, `EMULATOR_PLATFORM`
  - `emu_install::select_executable(title: &str, install_dir: &Path, archive: &Path) -> Option<PathBuf>`
  - `crate::library::paths::expand_home`
  - `Config::load`
- Produces:
  - `fn superseded_appimage(previous_path: &str, install_dir: &Path, new_exe: &Path) -> Option<PathBuf>`
- Behaviour: in `finalize_emulator`, a non-extracted AppImage primary is the executable. After the config write and archive cleanup, the superseded AppImage (if any) is deleted. A failed delete is a warning on a Completed row.
- No public signature changes.

- [ ] **5.1 Write the failing unit tests** in `library/mod.rs` tests:

```rust
    // --- superseded_appimage ---------------------------------------------------

    fn appimage_install() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("emulators").join("Dolphin (GameCube, Wii)");
        fs::create_dir_all(&install).unwrap();
        (dir, install)
    }

    #[test]
    fn superseded_appimage_is_the_previous_appimage_in_the_install_dir() {
        let (_dir, install) = appimage_install();
        let old = install.join("Dolphin_Emulator-2609-anylinux-x86_64.AppImage");
        let new = install.join("Dolphin_Emulator-2612-anylinux-x86_64.AppImage");
        fs::write(&old, b"old").unwrap();
        fs::write(&new, b"new").unwrap();

        assert_eq!(
            superseded_appimage(&old.to_string_lossy(), &install, &new),
            Some(old.clone())
        );
        // The file the update just wrote is never "superseded".
        assert_eq!(superseded_appimage(&new.to_string_lossy(), &install, &new), None);
        assert_eq!(superseded_appimage("   ", &install, &new), None);
        // Already gone: nothing to delete.
        fs::remove_file(&old).unwrap();
        assert_eq!(superseded_appimage(&old.to_string_lossy(), &install, &new), None);
    }

    #[test]
    fn superseded_appimage_never_reaches_past_the_install_dir_or_a_non_appimage_file() {
        let (dir, install) = appimage_install();
        let new = install.join("Dolphin_Emulator-2612-anylinux-x86_64.AppImage");
        fs::write(&new, b"new").unwrap();

        // An AppImage the user keeps elsewhere.
        let outside = dir.path().join("Dolphin_Emulator-2609-anylinux-x86_64.AppImage");
        fs::write(&outside, b"mine").unwrap();
        assert_eq!(superseded_appimage(&outside.to_string_lossy(), &install, &new), None);

        // Not an AppImage.
        let exe = install.join("Dolphin.exe");
        fs::write(&exe, b"exe").unwrap();
        assert_eq!(superseded_appimage(&exe.to_string_lossy(), &install, &new), None);

        // A directory with an AppImage name.
        let folder = install.join("folder.AppImage");
        fs::create_dir_all(&folder).unwrap();
        assert_eq!(superseded_appimage(&folder.to_string_lossy(), &install, &new), None);

        // Nested one level down: not the install's own primary.
        let nested = install.join("sub").join("Old.AppImage");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(&nested, b"x").unwrap();
        assert_eq!(superseded_appimage(&nested.to_string_lossy(), &install, &new), None);
    }

    #[cfg(unix)]
    #[test]
    fn superseded_appimage_never_follows_a_link() {
        let (dir, install) = appimage_install();
        let new = install.join("New.AppImage");
        fs::write(&new, b"new").unwrap();
        let target = dir.path().join("Target.AppImage");
        fs::write(&target, b"user").unwrap();
        let link = install.join("Linked.AppImage");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(superseded_appimage(&link.to_string_lossy(), &install, &new), None);
    }
```

- [ ] **5.2 Write the failing integration test.** Add it to `crates/grid-core/tests/emulator_install.rs`, after `a_reinstalled_appimage_of_the_same_length_is_replaced_on_disk`:

```rust
/// An update whose AppImage carries a new version in its FILE NAME must
/// launch the new AppImage and remove the one it replaced — and nothing
/// else: the user data behind the `User` link, a stray file, and an
/// AppImage the entry never pointed at all survive. Applies to every
/// AppImage emulator; Dolphin's names are the example.
#[cfg(unix)]
#[tokio::test]
async fn an_appimage_update_with_a_new_file_name_launches_the_new_build_and_removes_the_old_one() {
    let harness = Harness::new(|uri| {
        let mut source = gitea_source(uri);
        source["release_tag"] = json!("latest");
        vec![profile_with_user_data("Dolphin (GameCube, Wii)", source, &["User"])]
    })
    .await;
    let latest = "/api/v1/repos/acme/widget/releases/latest";
    let old_name = "Dolphin_Emulator-2609-anylinux-x86_64.AppImage";
    let new_name = "Dolphin_Emulator-2612-anylinux-x86_64.AppImage";

    harness
        .mount_widget_at(latest, "2609@2026-10-01_1", old_name, b"OLD-APPIMAGE".to_vec(), 0)
        .await;
    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let entry = harness.wait_terminal(harness.newest_entry_id()).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let install_dir = harness.install_dir("Dolphin (GameCube, Wii)");
    let old = install_dir.join(old_name);
    assert_eq!(harness.config().emulators[0].path, old.to_string_lossy());

    // State the update must not touch.
    let memcard = harness
        .library
        .join("saves")
        .join("Dolphin (GameCube, Wii)")
        .join("User")
        .join("GC")
        .join("MemoryCardA.USA.raw");
    fs::create_dir_all(memcard.parent().unwrap()).unwrap();
    fs::write(&memcard, b"MEMCARD").unwrap();
    // Sorts BEFORE both builds: a directory scan would pick it.
    let stray = install_dir.join("Dolphin_Emulator-2603-anylinux-x86_64.AppImage");
    fs::write(&stray, b"STRAY").unwrap();
    let notes = install_dir.join("notes.txt");
    fs::write(&notes, b"keep").unwrap();

    harness.server.reset().await;
    harness
        .mount_widget_at(latest, "2612@2026-12-01_1", new_name, b"NEW-APPIMAGE".to_vec(), 0)
        .await;
    harness
        .service
        .install_emulator("acme/widget".to_string())
        .await
        .unwrap();
    let entry = harness.wait_terminal(harness.newest_entry_id()).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);

    let new = install_dir.join(new_name);
    let config = harness.config();
    assert_eq!(config.emulators.len(), 1);
    assert_eq!(config.emulators[0].path, new.to_string_lossy());
    assert_eq!(config.emulators[0].source_installed_tag, "2612@2026-12-01_1");
    assert_eq!(fs::read(&new).unwrap(), b"NEW-APPIMAGE");
    assert_eq!(mode_of(&new), 0o755);
    assert!(!old.exists(), "the replaced AppImage must be removed");
    assert!(stray.is_file(), "an AppImage the entry never pointed at stays");
    assert!(notes.is_file(), "unrelated files stay");

    let user = install_dir.join("User");
    assert!(
        fs::symlink_metadata(&user).unwrap().file_type().is_symlink(),
        "the User link survives the update"
    );
    assert_eq!(
        fs::read(user.join("GC").join("MemoryCardA.USA.raw")).unwrap(),
        b"MEMCARD"
    );
}
```

- [ ] **5.3 Run:**
  - `cargo test -p grid-core superseded_appimage`. Expected: a compile failure.
  - `cargo test -p grid-core --test emulator_install an_appimage_update_with_a_new_file_name`. Expected: FAIL, because the path still points at the 2603 or 2609 file and the old file still exists.

- [ ] **5.4 Implement the helper** in `library/mod.rs`, next to `is_appimage`:

```rust
/// The AppImage an emulator install replaced: the entry's PREVIOUS
/// executable, when it is a regular AppImage file (not a link, not a
/// directory) sitting directly inside `install_dir` and is not `new_exe`.
/// `None` for anything else — a blank or missing path, a file outside the
/// install directory or nested below it, a non-AppImage, or the file this
/// install just wrote. Only this one file is ever deleted after an update;
/// user data, links and every other file stay.
fn superseded_appimage(previous_path: &str, install_dir: &Path, new_exe: &Path) -> Option<PathBuf> {
    let trimmed = previous_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let previous = crate::library::paths::expand_home(trimmed);
    if !is_appimage(&previous) {
        return None;
    }
    if !fs::symlink_metadata(&previous).ok()?.file_type().is_file() {
        return None;
    }
    let previous_dir = fs::canonicalize(previous.parent()?).ok()?;
    if previous_dir != fs::canonicalize(install_dir).ok()? {
        return None;
    }
    let same_file = match (fs::canonicalize(&previous), fs::canonicalize(new_exe)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    (!same_file).then_some(previous)
}
```

- [ ] **5.5 Implement the changes in `finalize_emulator`.**

  **(a) Read the previous executable** before anything is written. Place this after the compat-tool early return and before executable selection:
```rust
        // The entry's executable BEFORE this install, matched by exact name
        // like `write_emulator_entry`. An update deletes it below once the new
        // entry is saved, if it is the AppImage this install replaced.
        let previous_path = Config::load(&self.config_path).ok().and_then(|config| {
            config
                .emulators
                .iter()
                .find(|existing| existing.name == job.profile_name)
                .map(|existing| existing.path.clone())
        });
```

  **(b) Replace the `select_executable` call** (`:2290-2293`) with:
```rust
        // A primary AppImage IS the new install. Scanning the directory would
        // rank it against the AppImage it replaces — both carry the title
        // token, so the casefolded-path tie-break can pick the OLD one
        // (`…-2609-…` sorts before `…-2612-…`).
        let selected = if !should_extract(EMULATOR_PLATFORM, archive)
            && is_appimage(archive)
            && archive.is_file()
        {
            Some(archive.to_path_buf())
        } else {
            emu_install::select_executable(&job.profile_name, install_dir, archive)
        };
        let Some(exe) = selected else {
            return Err(LibraryError::Extract(NO_EMULATOR_EXECUTABLE.to_string()));
        };
```

  **(c) Delete the superseded AppImage** after the extracted-archive cleanup loop and before the `emulator_installed_hook` block:
```rust
        // Only after the new entry is saved: a failure before this point
        // keeps the old AppImage launchable.
        if let Some(old) = previous_path
            .as_deref()
            .and_then(|path| superseded_appimage(path, install_dir, &exe))
        {
            if !delete_with_retry(&old) {
                append_warning(
                    warning,
                    &format!("could not delete the replaced AppImage: {}", old.display()),
                );
            }
        }
```

  **(d) Update `finalize_emulator`'s doc** with: "A primary AppImage is the executable; an update removes the AppImage the entry pointed at before (`superseded_appimage`)."

- [ ] **5.6 Run:**
  - `cargo test -p grid-core superseded_appimage`
  - `cargo test -p grid-core --test emulator_install`. Expected: all pass, including the existing `an_appimage_primary_is_kept_in_place_made_executable_and_recorded`, `a_reinstalled_appimage_of_the_same_length_is_replaced_on_disk`, `emulator_install_runs_autoconfig_after_writing_the_entry` and `updating_a_source_installed_emulator_preserves_user_fields_and_records_the_new_tag`.

- [ ] **5.7 Commit:**
```
git commit --only crates/grid-core/src/library/mod.rs crates/grid-core/tests/emulator_install.rs \
  -m "fix(library): AppImage updates launch the new build and remove the replaced one" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 6: Dolphin catalog profile

**Context:**
- Tasks 1-4 are in place: `resolve_source_for_host`, `catalog_entries_for_host`, `%emu_dir%`, and the reader expansion.
- **Profile name:** `Dolphin (GameCube, Wii)` (user ruling). The comma survives `sanitize_component`.
- **Windows source:** the `direct` feed at `https://dolphin-emu.org/update/latest/beta/`.
- **Linux source:** a `github-release` override to `pkgforge-dev/Dolphin-emu-AppImage`. It skips the nightly prerelease and excludes `.zsync`.
- **macOS:** gated out by `source.platforms`.

**Files:**
- `emulator-autoprofiles.json`
- `crates/grid-core/src/launch/emu_install.rs` (`select_executable` + tests)
- `crates/grid-core/src/launch/profiles.rs` (tests)
- `crates/grid-core/src/launch/catalog.rs` (tests)

**Interfaces:**
- Consumes:
  - `crate::launch::source::{resolve_source_for_host, allow_prerelease, str_field}`
  - `catalog_entries_for_host` (catalog.rs, private)
  - `profile_available_on_host`, `platform_matches_keywords`, `profile_for_entry`, `load_profiles`
  - `emulator_install_dir(library: &Path, name: &str) -> PathBuf`
- Produces: the catalog profile `Dolphin (GameCube, Wii)` with `source_id` `dolphin-emu/dolphin`, and the preferred executable name `dolphin.exe`.

- [ ] **6.1 Write the failing tests in `profiles.rs`**, after `embedded_kyty_profile_launches_fullscreen_and_names_the_ps5_platform`:

```rust
    fn embedded_dolphin() -> &'static EmulatorProfile {
        load_profiles()
            .iter()
            .find(|p| p.name == "Dolphin (GameCube, Wii)")
            .expect("the catalog ships a Dolphin profile")
    }

    #[test]
    fn embedded_dolphin_profile_launches_portable_fullscreen_vulkan_at_1080p() {
        let profile = embedded_dolphin();
        assert_eq!(
            profile.args,
            "-u \"%emu_dir%/User\" -b -v Vulkan -C Dolphin.Display.Fullscreen=True -C GFX.Settings.InternalResolution=3 -e \"%rom%\""
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
            "/lib/emulators/Dolphin (GameCube, Wii)/Dolphin_Emulator-2609-anylinux-x86_64.AppImage",
            "/usr/bin/dolphin-emu",
        ] {
            assert_eq!(
                profile_for_entry("", exe, load_profiles()).map(|p| p.name.as_str()),
                Some("Dolphin (GameCube, Wii)"),
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

- [ ] **6.2 Write the failing tests in `catalog.rs`.** In `real_catalog_includes_expected_rows_with_expected_fields`, add before its end:
```rust
        let dolphin = by_name("Dolphin (GameCube, Wii)");
        assert_eq!(dolphin.provider, "direct");
        assert_eq!(dolphin.source_id, "dolphin-emu/dolphin");
```
  Then add:
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
                assert_eq!(dolphin[0].name, "Dolphin (GameCube, Wii)");
                assert_eq!(dolphin[0].source_id, "dolphin-emu/dolphin");
            }
        }
    }
```

- [ ] **6.3 Write the failing tests in `emu_install.rs`:**
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
            "Dolphin (GameCube, Wii)",
            install,
            &install.join("absent.7z"),
        );
        assert_eq!(picked, Some(install.join("Dolphin-x64").join("Dolphin.exe")));
    }

    /// The profile name's comma is a legal path character: it survives
    /// `sanitize_component` unchanged in the install directory name.
    #[test]
    fn a_comma_in_the_profile_name_survives_the_install_dir() {
        assert_eq!(
            emulator_install_dir(Path::new("/lib"), "Dolphin (GameCube, Wii)"),
            Path::new("/lib/emulators/Dolphin (GameCube, Wii)")
        );
    }
```

- [ ] **6.4 Run** `cargo test -p grid-core dolphin`. Expected: FAIL (no profile, wrong executable). The comma test may already pass. Keep it as a pin.

- [ ] **6.5 Add the catalog entry.** In `emulator-autoprofiles.json`, insert directly after the `"Cemu (Wii U)"` object (after its closing `},`, before the Azahar object):

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
    "name": "Dolphin (GameCube, Wii)",
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

- [ ] **6.6 Add the preferred executable name.** In `emu_install.rs::select_executable`, after the ShadPS4 block:
```rust
    if title_casefold.contains("dolphin") {
        // The Windows build ships DolphinTool.exe and Updater.exe beside
        // Dolphin.exe, and every one of them carries the title token.
        preferred_names.insert("dolphin.exe");
    }
```
  Extend the module doc's parity note to read "for KytyPS5, ShadPS4 and Dolphin".

- [ ] **6.7 Run:**
  - `cargo test -p grid-core dolphin`
  - `cargo test -p grid-core real_catalog`
  - `cargo test -p grid-core embedded_`
  - `cargo test -p grid-core every_catalog_source_block_with_a_recognized_provider_normalizes`
  - `cargo test -p grid-core resolve_source_matches_normalize_then_merge_for_the_catalog`
  - `cargo test -p grid-core emu_install`

  Expected: all pass.

- [ ] **6.8 Commit:**
```
git commit --only emulator-autoprofiles.json crates/grid-core/src/launch/emu_install.rs crates/grid-core/src/launch/profiles.rs crates/grid-core/src/launch/catalog.rs \
  -m "feat(catalog): Dolphin installs from the official 7z on Windows and the pkgforge AppImage on Linux" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 7: Pin Dolphin asset selection against captured payloads

**Context:**
- The `Dolphin (GameCube, Wii)` catalog profile exists (Task 6).
- `scrape_download_url` walks hrefs first, then falls back to a whole-page regex search. Without capture groups, it returns the whole match.
- The live feed (captured 2026-10-08) lists arm64 before x64. The Linux release list has a `nightly` prerelease plus `.zsync` and aarch64 assets.

**Files:**
- `crates/grid-core/src/launch/forge.rs` (tests only)

**Interfaces:**
- Consumes:
  - `fn scrape_download_url(page_text: &str, download_url_regex: &str, page_url: &str) -> Result<String, SourceError>`
  - `fn basename_of_url(url: &str) -> String`
  - `select_release`, `select_asset`, `str_field`, `SourceMap`
  - `crate::launch::source::resolve_source_for_host`
  - `crate::launch::profiles::load_profiles`
  - `ForgeClient::resolve(&self, raw: &Value, profile_name: &str) -> Result<ResolvedDownload, SourceError>`
- Produces: tests only.

- [ ] **7.1 Add fixtures and tests** at the end of `forge.rs`'s `mod tests`:

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
            .find(|p| p.name == "Dolphin (GameCube, Wii)")
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
            .resolve(&raw, "Dolphin (GameCube, Wii)")
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

- [ ] **7.2 Run** `cargo test -p grid-core forge::tests::dolphin` and `cargo test -p grid-core a_direct_source_resolves_the_dolphin`. Expected: all pass. If a regex test fails, the catalog regex is wrong: fix the JSON from Task 6, not the test.

- [ ] **7.3 Commit:**
```
git commit --only crates/grid-core/src/launch/forge.rs \
  -m "test(forge): pin Dolphin asset selection against the live feed and release shapes" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 8: Integration test: a Dolphin AppImage install runs portable from its own directory

**Context:**
- The real catalog profile `Dolphin (GameCube, Wii)` exists (Task 6). Its args contain `-u "%emu_dir%/User"`, which Task 3 expands.
- The test harness cannot reach `api.github.com`, so the profile's source is swapped for a `gitea` one served by wiremock.

**Files:**
- `crates/grid-core/tests/emulator_install.rs`

**Interfaces:**
- Consumes:
  - `grid_core::launch::profiles::load_profiles`
  - `grid_core::launch::spawn::prepare_emulator_launch(emulator_name: &str, entry: Option<&EmulatorEntry>, rom_path: &str, placeholders: &Placeholders, global_launch_args: &str, is_retroarch: bool) -> Result<(Vec<String>, PathBuf), String>`
  - `grid_core::launch::template::Placeholders`
  - Harness helpers in this file: `Harness::new`, `gitea_source`, `mount_widget`, `newest_entry_id`, `wait_terminal`, `install_dir`, `config`, `mode_of`.
- Produces: tests only.

- [ ] **8.1 Add the imports** `use grid_core::launch::spawn::prepare_emulator_launch;` and `use grid_core::launch::template::Placeholders;`, then add the test after `an_appimage_primary_is_kept_in_place_made_executable_and_recorded`:

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
            .find(|p| p.name == "Dolphin (GameCube, Wii)")
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

    let install_dir = harness.install_dir("Dolphin (GameCube, Wii)");
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
                .join("Dolphin (GameCube, Wii)")
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
    assert_eq!(emu.name, "Dolphin (GameCube, Wii)");
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

- [ ] **8.2 Run** `cargo test -p grid-core --test emulator_install a_dolphin_appimage_install`. Expected: pass. If `Dolphin.ini` is missing, read the warning in `entry.error` before changing code.

- [ ] **8.3 Commit:**
```
git commit --only crates/grid-core/tests/emulator_install.rs \
  -m "test(library): Dolphin AppImage install links User and launches with -u to its own folder" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 9: KytyPS5 output resolution args and their migration

**Context:**
- Kyty's args become `--fullscreen --screen-width 1920 --screen-height 1080 --game "%rom%"`.
- Existing installs carry `--fullscreen --game "%rom%"`. `autoconfig::entry::migrate_legacy_args` rewrites any entry whose trimmed args equal one of the profile's `legacy_args`.
- The app runs that at startup (`app/src-tauri/src/lib.rs:111` via `migrate_legacy_args_in_config`).

**Files:**
- `emulator-autoprofiles.json` (the `"KytyPS5 (Playstation 5)"` object only)
- `crates/grid-core/src/launch/profiles.rs` (the test at `:714-729`)
- `crates/grid-core/src/autoconfig/entry.rs` (a test)

**Interfaces:**
- Consumes: `pub fn migrate_legacy_args(emulators: &mut [EmulatorEntry], profiles: &[EmulatorProfile]) -> usize`, `load_profiles`.
- Produces: a data change only.

- [ ] **9.1 Write the failing tests.**

  In `profiles.rs`, `embedded_kyty_profile_launches_fullscreen_and_names_the_ps5_platform`: replace the args assertion with the block below and keep the two keyword assertions.
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

  In `entry.rs` tests, in the `migrate_legacy_args` block:
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

- [ ] **9.2 Run** `cargo test -p grid-core kyty`. Expected: FAIL.

- [ ] **9.3 Implement.** In the `"KytyPS5 (Playstation 5)"` object:
  - Set `"args": "--fullscreen --screen-width 1920 --screen-height 1080 --game \"%rom%\""`.
  - Add `"legacy_args": ["--fullscreen --game \"%rom%\""]` directly after `args`.
  - Change nothing else.

- [ ] **9.4 Run** `cargo test -p grid-core kyty` and `cargo test -p grid-core migrate_`. Expected: all pass.

- [ ] **9.5 Commit:**
```
git commit --only emulator-autoprofiles.json crates/grid-core/src/launch/profiles.rs crates/grid-core/src/autoconfig/entry.rs \
  -m "feat(catalog): KytyPS5 renders at 1920x1080; old default args migrate at startup" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 10: PS5 install planning rejects `.pkg` and folder games, keeps `.zar` and bare files untouched

**Context:**
- `plan_install` (`library/mod.rs:349-438`) builds the download plan before admission. Its `Err` reaches the UI verbatim, because `LibraryError::Extract` displays `{0}`.
- Rules (user rulings 2026-10-08), for a PlayStation 5 platform (`is_ps5_platform`):
  - **Rejected:** a single `.pkg` candidate, or more than one candidate.
  - **Accepted and routed `Downloaded`:** a single `.zar`, or a single bare non-archive file such as `eboot.bin`. `zar` and `bin` are not in `extract::EXTRACTABLE_SUFFIXES`.
  - **Unchanged:** a single `.zip`/`.7z`, which keeps the `Eboot { ps4: false }` route.

**Files:**
- `crates/grid-core/src/library/mod.rs`

**Interfaces:**
- Consumes:
  - `is_ps5_platform(&str) -> bool` (already imported)
  - `fn plan_install(detail: &RomDetail, library: &Path, client: Arc<RommClient>) -> Result<InstallJob, LibraryError>`
  - `fn base_finalize_route(platform: &str, archive: &Path) -> BaseRoute`
  - Test helpers in this module: `detail`, `rom_file`, `client`.
- Produces:
  - `const KYTY_UNSUPPORTED_GAME: &str`
  - `fn ps5_unsupported_download(platform: &str, candidates: &[&RomFile]) -> bool`

- [ ] **10.1 Write the failing tests** in `library/mod.rs` tests, after `plan_multi_file_without_an_m3u_launches_the_first_candidate`:

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
                "KytyPS5 needs a .zar archive, or a .zip/.7z that contains eboot.bin. .pkg files and folder games are not supported."
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

    /// User ruling (2026-10-08): a PS5 game whose only candidate is a bare
    /// executable (an `eboot.bin` / ELF) is allowed and installed as-is.
    #[test]
    fn plan_accepts_a_single_bare_ps5_executable_and_routes_it_as_downloaded() {
        let job = plan_install(
            &ps5_detail(vec![rom_file(1, "eboot.bin", true)]),
            Path::new("/library"),
            client(),
        )
        .unwrap();
        assert_eq!(
            job.primary_archive,
            PathBuf::from("/library/games/PlayStation 5/eboot.bin")
        );
        assert_eq!(
            base_finalize_route("PlayStation 5", &job.primary_archive),
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

- [ ] **10.2 Run** `cargo test -p grid-core plan_`. Expected: FAIL (`KYTY_UNSUPPORTED_GAME` is missing, and the `.pkg` plan succeeds).

- [ ] **10.3 Implement.** Next to `NO_DOWNLOADABLE_FILE` (`:108`), add:

```rust
/// Why a PlayStation 5 game cannot be installed for KytyPS5: Kyty opens a
/// `.zar` in place, or GRID extracts a `.zip`/`.7z` and launches its
/// `eboot.bin`. Raised by [`plan_install`], before any byte is requested.
const KYTY_UNSUPPORTED_GAME: &str = "KytyPS5 needs a .zar archive, or a .zip/.7z that contains eboot.bin. .pkg files and folder games are not supported.";

/// Whether a PS5 game's download set is one KytyPS5 cannot run: a single
/// `.pkg` (no `.pkg` support anywhere) or more than one file (a bare folder
/// game, whose nested files are never downloaded). A single file of any
/// other kind — a `.zar`, an archive, a bare `eboot.bin` — is allowed. Any
/// other platform, and an empty set (reported as
/// [`NO_DOWNLOADABLE_FILE`]), is `false`.
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
  In `plan_install`, directly after `let candidates: Vec<&RomFile> = …collect();` and before the `match`, add:
```rust
    if ps5_unsupported_download(&detail.platform_name, &candidates) {
        return Err(LibraryError::Extract(KYTY_UNSUPPORTED_GAME.to_string()));
    }
```
  Add to `plan_install`'s doc: "A PlayStation 5 `.pkg` or multi-file game is rejected here ([`KYTY_UNSUPPORTED_GAME`])."

- [ ] **10.4 Run** `cargo test -p grid-core plan_` and `cargo test -p grid-core base_route`. Expected: all pass.

- [ ] **10.5 Commit:**
```
git commit --only crates/grid-core/src/library/mod.rs \
  -m "feat(library): reject PS5 .pkg and folder games at install planning; .zar installs as-is" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 11: Docs

**Context:** These are behaviour docs that must travel with the change. Dolphin is now in the catalog as `Dolphin (GameCube, Wii)`. PS5 accepts `.zar`. AppImage updates replace the old file.

**Files:**
- `future-plans/platform-support.md`
- `docs/superpowers/plans/2026-09-15-library-layout-v1.md`
- `.claude/skills/emulator-autoconfig/SKILL.md`

- [ ] **11.1 Edit `future-plans/platform-support.md`.**
  - Replace lines 166-167 with:
```
- **MAME is not part of auto-install.** It remains playable through its RetroArch cores
  (`mame_libretro` / `mame2003_plus_libretro`).
- **Dolphin auto-installs** as `Dolphin (GameCube, Wii)`: the official portable `.7z` on
  Windows, the unofficial `pkgforge-dev/Dolphin-emu-AppImage` on Linux (the Flatpak is never
  used), not offered on macOS. It launches with `-u "%emu_dir%/User"`, so both builds keep
  their data in `<exe dir>/User`, linked into `saves/`.
- **AppImage updates** launch the newly downloaded AppImage and delete only the AppImage the
  entry pointed at before; user data and every other file stay.
```
  - Replace the Wii/GameCube row (`:238`) with:
```
| Wii/GameCube | Dolphin | Native (unofficial `pkgforge-dev` AppImage) | Auto-install: official `.7z` on Windows, AppImage on Linux; not offered on macOS |
```
  - After the PS4 row, add:
```
| PS5 | KytyPS5 | Native | Games must be `.zar`, a `.zip`/`.7z` containing `eboot.bin`, or a single bare executable; `.pkg` and multi-file folder games are rejected at install |
```

- [ ] **11.2 Edit `docs/superpowers/plans/2026-09-15-library-layout-v1.md`.** Append this to Decision 9 (line 37) and to the "Dolphin shared directory" bullet (line 313):
```
**Superseded 2026-10-08:** Dolphin is now in the catalog as `Dolphin (GameCube, Wii)` with `user_data: ["User"]` (spec `docs/superpowers/specs/2026-10-08-dolphin-and-kyty-zar-design.md`).
```

- [ ] **11.3 Edit `.claude/skills/emulator-autoconfig/SKILL.md`.** Under "Other Emulators", extend the Dolphin bullet with:
```
  - The catalog profile `Dolphin (GameCube, Wii)` launches with `-u "%emu_dir%/User"` (`%emu_dir%` = the executable's directory, `launch::template`). `readers::dolphin_user_root_candidates` expands the same placeholder, so the Linux AppImage (whose own directory is a read-only mount) and the Windows `.7z` share one `<exe dir>/User` root. The profile has no `firmware_directories`: `ensure_gcpad_config`'s block is XInput-only.
```

- [ ] **11.4 Commit:**
```
git commit --only future-plans/platform-support.md docs/superpowers/plans/2026-09-15-library-layout-v1.md .claude/skills/emulator-autoconfig/SKILL.md \
  -m "docs: Dolphin is in the catalog; PS5 accepts .zar; AppImage updates replace the old file" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

## Task 12: Gate, e2e regression, live check

**Context:** Tasks 1-11 are committed. This task proves the whole change set.

- [ ] **12.1 Run the full gate** from the repo root, in order. Run it detached and read the log.
```
scripts/check_secret_hygiene.sh
cargo fmt --check
cd app && npm ci && npx svelte-check && npm run build
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo test --workspace
cd app && npm test
```
  If a command fails, fix it only in the files this plan touched. Then re-run the failed command and the rest of the gate.

- [ ] **12.2 Run the e2e regression.**
  - First, `scripts/e2e.sh emulator-catalog`. It does a full build, because Rust changed. It covers the PCSX2 AppImage install and "Update from Source", which go through the Task 5 code.
  - Then `E2E_SKIP_BUILD=1 scripts/e2e.sh launch`. Skipping the build is safe because nothing changed since that build.
  - Expected: no spec or fixture changes. The PCSX2 update keeps the same asset name, so nothing is deleted.

- [ ] **12.3 Commit any gate fixes** with `git commit --only <touched paths>` and the same trailer.

- [ ] **12.4 Manual live check (user's machine).**
  - Install Dolphin from the catalog. Expect exactly one row.
  - Launch one GameCube game and one Wii game. Expect fullscreen, Vulkan, 3x internal resolution, and data in `emulators/Dolphin (GameCube, Wii)/User` → `saves/Dolphin (GameCube, Wii)/User`.
  - Run "Update from Source" on one AppImage emulator whose release changed. Expect the new AppImage to launch and the old file to be gone.
  - Restart GRID and confirm the KytyPS5 entry's args were migrated.
  - Launch one `.zar` game.
  - Try a `.pkg` PS5 game. Expect the exact rejection text and no download row.

- [ ] **12.5 Milestone cleanup.** If the orchestrator closes the milestone here, run `cargo clean --profile dev` from the repo root.

---

## Edge cases to handle (covered above unless noted)

- **Override sets `provider` but a blank `owner`.** `normalize_source` errors after the merge. The catalog test resolves every source on every host.
- **`tag` versus `release_tag`.** `normalize_source` reads `tag` first, so a top-level `tag` now beats an override's `release_tag`. No catalog entry uses `tag`, and the regression test would catch a future one.
- **`%emu_dir%` with spaces or a comma.** It stays one argv element. A bare file name or `/` gives `EMU_DIR_MISSING`, never `-u /User` (Task 3).
- **Hand-configured Dolphin entries.** They now match the new profile by token. Saves still come from the readers. Their own args are kept, because `apply_manual_emulator_profile_defaults` replaces args only when they are blank or `%rom%`.
- **Windows `.7z` nested in `Dolphin-x64/`.** `user_data_root` and `%emu_dir%` both use the executable's parent.
- **The previous executable is not an AppImage** (an extracted build, or a user-chosen path outside the install dir). Nothing is deleted (Task 5 unit tests).
- **The previous AppImage is still running.** On Linux, deleting the file leaves the running inode intact.
- **A failed delete** becomes a warning on a Completed row. The new entry is already saved.
- **An update that keeps the same asset name** (PCSX2 in e2e). It is already unlinked before download, so `superseded_appimage` returns `None` because it is the same file.
- **Stray older AppImages from earlier updates** stay on disk, because the entry no longer points at them. Removing them would break the "only the replaced file" rule. A user can delete them by hand.
- **`.zar` with an uppercase suffix.** No `zar` suffix ever reaches the extractor.
- **A PS5 game with a `game.json` sidecar.** The sidecar is not a candidate, so the game is not counted as a folder game.
- **Future Dolphin feed revisions with hyphens.** The character class allows `-`, `_` and `.`. An unmatched revision fails with "did not resolve a download URL"; it never picks a wrong file.

## Open questions

Numbering is kept from the first draft. Questions 1, 2 and 6 were ruled on 2026-10-08.

3. **Dolphin firmware and the GCPad block.** Firmware routing stays off (Decision 5). A follow-up could route `dsp_rom.bin`, `dsp_coef.bin` and `font_*.bin` to `User/GC`, and `IPL.bin` per region. It also needs a non-XInput GCPad default on Linux.
4. **The Linux catalog meta line.** The row shows `direct • latest` while Linux installs from GitHub (Decision 2). Is that acceptable, or should the row show the host-resolved provider while keeping the top-level `source_id`?
5. **Copy changes not made.**
   - `ARGS_LABEL` does not list `%emu_dir%`. It is in `app/src/lib/emulators/form.ts:13`, and is pinned by `form.test.ts` and `e2e/specs/emulators.spec.ts:222`.
   - The KytyPS5 note (`app/src/lib/emulators/notes.ts:47-49`) does not mention `.zar`.

   Both are UI copy, which belongs to the designer.
7. **The Windows Dolphin update check** says "unknown" (direct provider), the same as RetroArch and Redream. Reading `shortrev` from the feed would make it real. This is out of scope.

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