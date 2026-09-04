# RetroArch platform support and per-platform core picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make RetroArch's platform support depend on its installed libretro cores, and give every platform row in the Emulators panel a core picker.

**Architecture:** `emulator_supports_platform` stops reading the `retroarch_cores` config map and instead calls a core resolver, so a RetroArch entry supports a platform only when a compatible core file is installed next to it. The resolver is one new grid-core function, `installed_compatible_cores`, that resolves candidates slug-first from the bundled `romm-platform-cores.json` and falls back to the fuzzy compatibility map. The app layer feeds it server platform slugs, exposes two new Tauri commands (`retroarch_core_options`, `set_retroarch_core`), and the Emulators panel renders a second `<select>` per row.

**Tech Stack:** Rust (grid-core, Tauri 2 `app` crate, `tempfile` in tests), Svelte 5 + TypeScript + vitest, WebdriverIO E2E with the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-retroarch-platform-cores-design.md` — binding. Read it before any task; it settles every conflict inside this plan.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## Global Constraints

- **Token secrecy (hard):** tokens only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output.
- **Refusal string, verbatim:** `<core> is not an installed RetroArch core for <platform>` — format string `"{core} is not an installed RetroArch core for {platform}"`.
- **Core select DOM contract:** `data-testid="default-core-<platformId>"`, `id="default-core-<platformId>"`, label text `Core`, empty-state option text `No installed core`.
- **`PlatformRef { name: String, slug: String }`** on both sides — a Rust serde struct in `app/src-tauri/src/commands.rs` and a TS type in `app/src/lib/api.ts`.
- **Predicate order is spec §3.1** (blank → RetroArch core gate → `all_platforms` → no profile → keywords). `retroarch_cores` is no longer an input to `emulator_supports_platform`.
- **Slug-first with fuzzy fallback** on a slug MISS *and* on an EMPTY slug (D-RC-2).
- **No `retroarch_cores` value is ever overwritten by `set_default_emulator`** (D-RC-4): it only fills a platform that has no non-blank core.
- **`all_platforms` stays `true`** in the autoprofile JSON. Do NOT edit `emulator-autoprofiles.json`.
- **Every task ends with** `cargo fmt` (from `rewrite/`), `cargo clippy --workspace --all-targets -- -D warnings` clean (from `rewrite/`), and a commit whose subject starts `rewrite: `.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash` on tracked files. Commit with explicit pathspecs.
- **Test commands:** `cargo test -p grid-core` and `cargo test -p app` from `rewrite/`; `npm run check` and `npx vitest run` from `rewrite/app`; `scripts/e2e.sh <group>` from `rewrite/`.

---

## File map

| File | Responsibility |
|---|---|
| `crates/grid-core/src/autoconfig/mod.rs` | `installed_compatible_cores` (replaces `installed_cores_for_platform`); `SyncContext.platform_slugs`; the two backfill closures |
| `crates/grid-core/src/launch/selection.rs` | `CoreResolver`, `installed_core_resolver`, public `is_retroarch_name`, reordered predicate, updated callers + tests |
| `crates/grid-core/src/launch/mod.rs`, `cloud/ops/mod.rs`, `firmware/routing.rs`, `library/mod.rs` | call-site updates to the new resolver argument |
| `crates/grid-core/src/library/mod.rs` | `InstallService::set_platform_slugs` / `platform_slugs` |
| `app/src-tauri/src/commands.rs` | `PlatformRef`, `compatible_emulators`, `retroarch_core_options`, `set_retroarch_core`, `set_default_emulator` core recording, slug recording in `list_platforms`, `SyncContext` wiring |
| `app/src-tauri/src/lib.rs` | handler registration for the new command |
| `app/src-tauri/src/firmware_service.rs` | call-site update |
| `app/src/lib/api.ts` | `PlatformRef`, three wrappers |
| `app/src/lib/emulators/defaults.ts` (+ `defaults.test.ts`) | `isRetroarchName`, `platformCoreSelect` |
| `app/src/lib/Emulators.svelte` | core-options fetch, core select markup, change handler |
| `e2e/seed/launch-seed.mjs`, `e2e/specs/launch.spec.ts`, `e2e/specs/emulators.spec.ts` | E2E coverage per spec §4 |
| `docs/porting/04-emulator-launch.md`, `docs/porting/05-emulator-autoconfig.md`, `README.md` | doc updates |

---

### Task 1: `installed_compatible_cores` in grid-core autoconfig

**Files:**
- Modify: `crates/grid-core/src/autoconfig/mod.rs:419-445` (replace `installed_cores_for_platform`), `crates/grid-core/src/autoconfig/mod.rs:531-535` and `:660-664` (the two closures that call it)

**Interfaces:**
- Consumes (all already exist, all `pub` in `crates/grid-core/src/autoconfig/cores.rs`):
  - `pub fn slug_core_map() -> &'static BTreeMap<String, Vec<String>>` (cores.rs:130)
  - `pub fn cores_for_slug(slug: &str, map: &BTreeMap<String, Vec<String>>) -> Vec<String>` (cores.rs:487)
  - `pub fn compatibility_map() -> &'static CompatMap` (cores.rs:124)
  - `pub fn cores_for_platform(platform: &str, compat: &CompatMap) -> Vec<String>` (cores.rs:452)
  - `pub fn installed_core_ids(emulator_path: &str, cores_dir: Option<&Path>) -> BTreeSet<String>` (cores.rs:516)
  - `pub fn emulator_entry_by_name<'a>(emulators: &'a [EmulatorEntry], name: &str) -> Option<&'a EmulatorEntry>` (`crates/grid-core/src/launch/selection.rs:47`)
- Produces: `pub fn installed_compatible_cores(platform_name: &str, platform_slug: &str, entry: &EmulatorEntry) -> Vec<String>` in `grid_core::autoconfig`. Tasks 2, 3 and 4 all call it.

- [ ] **Step 1: Write the failing tests**

Append these to the existing `#[cfg(test)] mod tests` block in `crates/grid-core/src/autoconfig/mod.rs` (it already has `use super::*;`, `use crate::config::{Config, EmulatorEntry};` and a local `fn entry(name: &str, path: &str) -> EmulatorEntry`). Add the helper first, then the four tests:

```rust
    // --- installed_compatible_cores ------------------------------------------

    /// A RetroArch-shaped stub at `<dir>/retroarch` with a sibling
    /// `cores/` directory holding one file per id in `core_ids`, written
    /// with ALL THREE host extensions (`so`, `dylib`, `dll`) so the test
    /// passes whichever host `cores::installed_core_ids` is compiled for
    /// (`host_core_extension`, cores.rs:497).
    fn retroarch_with_cores(dir: &Path, core_ids: &[&str]) -> EmulatorEntry {
        let exe = dir.join("retroarch");
        std::fs::write(&exe, b"binary").unwrap();
        let cores_dir = dir.join("cores");
        std::fs::create_dir_all(&cores_dir).unwrap();
        for id in core_ids {
            for extension in ["so", "dylib", "dll"] {
                std::fs::write(cores_dir.join(format!("{id}_libretro.{extension}")), b"").unwrap();
            }
        }
        entry("RetroArch", exe.to_str().unwrap())
    }

    #[test]
    fn installed_compatible_cores_uses_the_slug_map_curated_order() {
        // romm-platform-cores.json maps "snes" to
        // ["snes9x", "snes9x2010", "bsnes"]; only two of those are
        // installed, and the answer keeps the map's order, not the
        // filesystem's.
        let temp = tempfile::tempdir().unwrap();
        let entry = retroarch_with_cores(temp.path(), &["bsnes", "snes9x"]);
        assert_eq!(
            installed_compatible_cores("Super Nintendo Entertainment System", "snes", &entry),
            vec!["snes9x".to_string(), "bsnes".to_string()]
        );
    }

    #[test]
    fn installed_compatible_cores_falls_back_to_fuzzy_on_an_unknown_slug() {
        // D-RC-2: a server slug the bundled map has never heard of must not
        // silently drop RetroArch support — the fuzzy platform matcher
        // answers instead.
        let temp = tempfile::tempdir().unwrap();
        let entry = retroarch_with_cores(temp.path(), &["snes9x"]);
        assert_eq!(
            installed_compatible_cores(
                "Super Nintendo Entertainment System",
                "nintendo-sfc-2026",
                &entry
            ),
            vec!["snes9x".to_string()]
        );
    }

    #[test]
    fn installed_compatible_cores_falls_back_to_fuzzy_on_an_empty_slug() {
        // The launch path and the offline library have no slug at all.
        let temp = tempfile::tempdir().unwrap();
        let entry = retroarch_with_cores(temp.path(), &["snes9x"]);
        assert_eq!(
            installed_compatible_cores("Super Nintendo Entertainment System", "", &entry),
            vec!["snes9x".to_string()]
        );
    }

    #[test]
    fn installed_compatible_cores_is_empty_when_no_core_file_is_installed() {
        let temp = tempfile::tempdir().unwrap();
        let entry = retroarch_with_cores(temp.path(), &["mgba"]);
        // mgba is a Game Boy Advance core, not a SNES one.
        assert!(installed_compatible_cores("Super Nintendo Entertainment System", "snes", &entry)
            .is_empty());
        // And an entry whose path does not exist has no cores at all.
        let missing = entry("RetroArch", "/nonexistent/retroarch");
        assert!(installed_compatible_cores("Super Nintendo Entertainment System", "snes", &missing)
            .is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run from `rewrite/`: `cargo test -p grid-core autoconfig::tests::installed_compatible_cores`
Expected: FAIL to compile — `cannot find function 'installed_compatible_cores' in this scope`.

- [ ] **Step 3: Replace `installed_cores_for_platform`**

In `crates/grid-core/src/autoconfig/mod.rs`, delete lines 419-445 (the doc comment plus `fn installed_cores_for_platform`) and put this in their place:

```rust
/// `_installed_retroarch_cores_for_platform` (emulator_ui_mixin.py:547-568):
/// the cores compatible with a platform, narrowed to the ones actually
/// installed next to `entry`'s executable, in candidate order. `[]` when the
/// entry has no core files or nothing compatible is installed.
///
/// Candidate resolution is SLUG-FIRST (design D-RC-2): the bundled
/// `romm-platform-cores.json` curated list for `platform_slug` when that
/// slug is non-blank and present, else the fuzzy
/// [`cores::cores_for_platform`] match on `platform_name`. The reference
/// returned `[]` for any non-blank slug missing from the map; this port
/// falls back instead, so a RomM slug spelling the bundled map has not
/// caught up with cannot silently drop RetroArch support for the platform.
pub fn installed_compatible_cores(
    platform_name: &str,
    platform_slug: &str,
    entry: &EmulatorEntry,
) -> Vec<String> {
    let installed = cores::installed_core_ids(&entry.path, None);
    if installed.is_empty() {
        return Vec::new();
    }

    let slug = platform_slug.trim();
    let mut candidates = if slug.is_empty() {
        Vec::new()
    } else {
        cores::cores_for_slug(slug, cores::slug_core_map())
    };
    if candidates.is_empty() {
        candidates = cores::cores_for_platform(platform_name, cores::compatibility_map());
    }

    candidates
        .into_iter()
        .filter(|core| installed.contains(core))
        .collect()
}
```

- [ ] **Step 4: Point the two backfill closures at it**

`crates/grid-core/src/autoconfig/mod.rs:531-534` (inside `sync_new_emulator`) currently reads:

```rust
    let compat = cores::compatibility_map();
    let installed_cores = |platform: &str, emulator_name: &str| -> Vec<String> {
        installed_cores_for_platform(platform, emulator_name, &snapshot, compat)
    };
```

Replace it with (the slug map arrives in Task 3; for now the slug is always empty, which is the fuzzy fallback and preserves today's behavior exactly):

```rust
    let installed_cores = |platform: &str, emulator_name: &str| -> Vec<String> {
        match emulator_entry_by_name(&snapshot, emulator_name) {
            Some(entry) => installed_compatible_cores(platform, "", entry),
            None => Vec::new(),
        }
    };
```

Make the identical replacement at `crates/grid-core/src/autoconfig/mod.rs:660-663` (inside `backfill_all_defaults`).

- [ ] **Step 5: Run the tests to verify they pass**

Run from `rewrite/`: `cargo test -p grid-core`
Expected: PASS, including the four new tests.

- [ ] **Step 6: Format, lint, commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add crates/grid-core/src/autoconfig/mod.rs
git commit -m "rewrite: slug-first installed_compatible_cores in autoconfig"
```

---

### Task 2: Reorder the platform-support predicate around a core resolver

**Files:**
- Modify: `crates/grid-core/src/launch/selection.rs:60-162` (predicate + two callers) and its test module `:266-490`
- Modify: `crates/grid-core/src/launch/mod.rs:461-467`, `crates/grid-core/src/cloud/ops/mod.rs:331-337` and `:505-511`, `crates/grid-core/src/firmware/routing.rs:370-376`, `crates/grid-core/src/library/mod.rs:1776-1782` and `:2628-2634`, `app/src-tauri/src/firmware_service.rs:496-502`, `app/src-tauri/src/commands.rs:528-533` and `:857-858`

**Interfaces:**
- Consumes: `grid_core::autoconfig::installed_compatible_cores(platform_name: &str, platform_slug: &str, entry: &EmulatorEntry) -> Vec<String>` (Task 1).
- Produces, all in `grid_core::launch::selection`:
  - `pub type CoreResolver<'a> = &'a dyn Fn(&EmulatorEntry, &str) -> Vec<String>;`
  - `pub fn installed_core_resolver(entry: &EmulatorEntry, platform: &str) -> Vec<String>` — the slug-less production resolver; pass it as `&installed_core_resolver`.
  - `pub fn is_retroarch_name(name: &str) -> bool` (was private).
  - `pub fn emulator_supports_platform(entry: &EmulatorEntry, platform: &str, profiles: &[EmulatorProfile], cores: CoreResolver<'_>) -> bool`
  - `pub fn compatible_emulator_names_for_platform(emulators: &[EmulatorEntry], platform: &str, profiles: &[EmulatorProfile], cores: CoreResolver<'_>) -> Vec<String>`
  - `pub fn default_emulator_name_for_platform(emulators: &[EmulatorEntry], default_emulators: &BTreeMap<String, String>, platform: &str, profiles: &[EmulatorProfile], cores: CoreResolver<'_>) -> String`
  - Task 4 supplies its own slug-aware closure in place of `&installed_core_resolver`.

**Rationale for the resolver shape:** a `&dyn Fn(&EmulatorEntry, &str)` keeps the six slug-less grid-core call sites to a one-token diff (`&config.retroarch_cores` → `&installed_core_resolver`) while letting the app layer close over a name→slug map. A precomputed `BTreeMap<String, Vec<String>>` was rejected: `default_emulator_name_for_platform` is called per-game on the launch path, where precomputing every platform's cores would stat the cores directory for platforms nobody asked about.

- [ ] **Step 1: Write the failing tests**

In `crates/grid-core/src/launch/selection.rs`, inside `mod tests`, add these helpers just after the existing `fn profile_with_tokens(...)` (which ends at line 203):

```rust
    /// A resolver that answers `cores` for every entry and platform.
    fn cores_always(cores: &[&str]) -> impl Fn(&EmulatorEntry, &str) -> Vec<String> {
        let cores: Vec<String> = cores.iter().map(|c| c.to_string()).collect();
        move |_entry, _platform| cores.clone()
    }

    /// A resolver that answers nothing for anything — "no core installed".
    fn no_cores(_entry: &EmulatorEntry, _platform: &str) -> Vec<String> {
        Vec::new()
    }
```

Then add these three tests to the `emulator_supports_platform` section:

```rust
    #[test]
    fn supports_retroarch_gate_beats_all_platforms_when_no_core_is_installed() {
        // Design D-RC-1 and the root cause of report 1: the shipped
        // RetroArch autoprofile sets all_platforms: true, which used to
        // short-circuit ahead of the core gate and make RetroArch the
        // apparent default for every platform, PS5 and Windows included.
        let profiles = vec![profile_with("RetroArch (Multi-System)", true, &[])];
        let e = entry("RetroArch", "/x/retroarch");
        assert!(!emulator_supports_platform(
            &e,
            "PlayStation 5",
            &profiles,
            &no_cores
        ));
    }

    #[test]
    fn supports_retroarch_all_platforms_profile_is_supported_once_a_core_is_installed() {
        let profiles = vec![profile_with("RetroArch (Multi-System)", true, &[])];
        let e = entry("RetroArch", "/x/retroarch");
        let cores = cores_always(&["snes9x"]);
        assert!(emulator_supports_platform(&e, "SNES", &profiles, &cores));
    }

    #[test]
    fn supports_all_platforms_still_wins_for_a_non_retroarch_profile() {
        // The reorder must not touch the native all_platforms path.
        let profiles = vec![profile_with("MAME", true, &[])];
        let e = entry("MAME", "/x/mame");
        assert!(emulator_supports_platform(
            &e,
            "PlayStation 5",
            &profiles,
            &no_cores
        ));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run from `rewrite/`: `cargo test -p grid-core launch::selection`
Expected: FAIL to compile — the fourth argument of `emulator_supports_platform` is still `&BTreeMap<String, String>`.

- [ ] **Step 3: Rewrite the predicate and its two callers**

In `crates/grid-core/src/launch/selection.rs`, replace lines 60-162 (from the `is_retroarch_name` doc comment through the end of `default_emulator_name_for_platform`) with:

```rust
/// Whether `name` should be treated as a RetroArch build: the entry or
/// profile name contains "retroarch", case-insensitively. A simplified
/// stand-in for `_is_retroarch_emulator_name` (emulator_ui_mixin.py:1916),
/// which additionally consults autoprofile metadata not modeled here.
pub fn is_retroarch_name(name: &str) -> bool {
    name.to_lowercase().contains("retroarch")
}

/// Resolves a RetroArch entry's INSTALLED compatible cores for a platform.
/// `(entry, platform_name) -> core ids`, empty when none are installed.
///
/// The predicate takes this rather than the `retroarch_cores` config map
/// (design D-RC-1): what a RetroArch build can play is decided by the core
/// files on disk, not by what the user happened to save. The app layer
/// passes a closure that also knows the platform's server slug; grid-core's
/// own call sites, which have no slug, pass [`installed_core_resolver`].
pub type CoreResolver<'a> = &'a dyn Fn(&EmulatorEntry, &str) -> Vec<String>;

/// The production [`CoreResolver`] for callers that hold no platform slug —
/// the launch path, cloud ops, firmware routing, and the install service,
/// all of which see only a platform NAME. An empty slug takes
/// `installed_compatible_cores`' fuzzy fallback (D-RC-2).
///
/// Pass it as `&installed_core_resolver`.
pub fn installed_core_resolver(entry: &EmulatorEntry, platform: &str) -> Vec<String> {
    crate::autoconfig::installed_compatible_cores(platform, "", entry)
}

/// Whether `entry` supports `platform` (`_emulator_supports_platform`,
/// grid-launcher.py:3556; doc 04 §2, as amended by design D-RC-1):
///
/// 1. Blank platform → `true`.
/// 2. Entry or profile name contains "retroarch" → supported iff `cores`
///    resolves at least one installed compatible core. This runs BEFORE the
///    `all_platforms` shortcut, which is the whole point of D-RC-1: the
///    shipped RetroArch autoprofile sets `all_platforms: true`, and the old
///    order let that mark RetroArch compatible with Windows and PS5.
/// 3. Profile `all_platforms` → `true`.
/// 4. No profile matched → `true`.
/// 5. Otherwise compare `platform` against the profile's
///    `platform_keywords`.
pub fn emulator_supports_platform(
    entry: &EmulatorEntry,
    platform: &str,
    profiles: &[EmulatorProfile],
    cores: CoreResolver<'_>,
) -> bool {
    let selected = platform.trim();
    if selected.is_empty() {
        return true;
    }

    let profile = profile_for_entry(&entry.name, &entry.path, profiles);

    let is_retroarch =
        is_retroarch_name(&entry.name) || profile.is_some_and(|p| is_retroarch_name(&p.name));
    if is_retroarch {
        return !cores(entry, selected).is_empty();
    }

    if profile.is_some_and(|p| p.all_platforms) {
        return true;
    }

    let Some(profile) = profile else {
        return true;
    };

    platform_matches_keywords(selected, &profile.platform_keywords)
}

/// Names of emulators in `emulators` that support `platform`, in config
/// order, skipping entries with a blank name
/// (`compatible_emulator_names_for_platform`, selection.py:254).
pub fn compatible_emulator_names_for_platform(
    emulators: &[EmulatorEntry],
    platform: &str,
    profiles: &[EmulatorProfile],
    cores: CoreResolver<'_>,
) -> Vec<String> {
    emulators
        .iter()
        .filter_map(|entry| {
            let name = entry.name.trim();
            if name.is_empty() {
                return None;
            }
            if emulator_supports_platform(entry, platform, profiles, cores) {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// The default emulator name for `platform` (`default_emulator_name_for_platform`,
/// selection.py:270; doc 04 §2):
///
/// 1. Look up `platform` in `default_emulators` via
///    [`mapping_value_for_platform`].
/// 2. If configured, find the entry by case-insensitive name and keep it
///    only when it supports `platform`.
/// 3. Otherwise fall back to the first name in
///    [`compatible_emulator_names_for_platform`].
/// 4. If nothing matches, `""`.
pub fn default_emulator_name_for_platform(
    emulators: &[EmulatorEntry],
    default_emulators: &BTreeMap<String, String>,
    platform: &str,
    profiles: &[EmulatorProfile],
    cores: CoreResolver<'_>,
) -> String {
    if let Some(configured) = mapping_value_for_platform(default_emulators, platform) {
        if let Some(entry) = emulator_entry_by_name(emulators, configured) {
            if emulator_supports_platform(entry, platform, profiles, cores) {
                return configured.to_string();
            }
        }
    }

    compatible_emulator_names_for_platform(emulators, platform, profiles, cores)
        .into_iter()
        .next()
        .unwrap_or_default()
}
```

- [ ] **Step 4: Update the existing tests in `selection.rs`**

Every existing `emulator_supports_platform` / `compatible_emulator_names_for_platform` / `default_emulator_name_for_platform` call in `mod tests` passes `&BTreeMap::new()` or a `map(&[...])` as the last argument. Replace each as follows:

- `&BTreeMap::new()` (as the cores argument) → `&no_cores`
- `&cores` where `let cores = map(&[("SNES", "snes9x_libretro")]);` → drop the `let` and pass `&cores_always(&["snes9x"])`

Then apply these three specific edits, because the tests assert the old semantics:

1. `supports_all_platforms_profile_is_always_true` (line 275): its profile is named `Cemu`, not RetroArch, so it only needs the argument swap to `&no_cores`.
2. `supports_retroarch_entry_true_when_core_mapped` (line 298) — rename and rewrite:

```rust
    #[test]
    fn supports_retroarch_entry_true_when_a_core_is_installed() {
        let e = entry("RetroArch", "/x/retroarch");
        assert!(emulator_supports_platform(
            &e,
            "SNES",
            &[],
            &cores_always(&["snes9x"])
        ));
    }
```

3. `supports_retroarch_entry_false_when_no_core_mapped` (line 305) — rename to `supports_retroarch_entry_false_when_no_core_is_installed` and pass `&no_cores`.
4. `supports_retroarch_gate_applies_when_only_the_profile_name_carries_it` (line 316): keep the body, swap `&BTreeMap::new()` for `&no_cores` and the `cores` map for `&cores_always(&["snes9x"])`.
5. `compatible_names_preserve_config_order_and_skip_blank_names` (line 366): its third profile is `profile_with("RetroArch", true, &[])` and the assertion expects `["Dolphin", "RetroArch"]`. Under D-RC-1 RetroArch with `&no_cores` is no longer compatible, so pass `&cores_always(&["dolphin_core"])` and add this comment above the call: `// A resolver that answers a core for everything keeps RetroArch compatible, so this still tests ORDER rather than the D-RC-1 gate.`
6. `default_retroarch_gate_participates_in_fallback` (line 472): keep it as-is apart from passing `&no_cores`; it already asserts the fallback to `Dolphin`.

- [ ] **Step 5: Update the eight production call sites**

Each is a one-argument swap. In every file, add `installed_core_resolver` to the existing `use ...::selection::{...}` import list, then change the last argument.

- `crates/grid-core/src/launch/mod.rs:35` import list and `:466` — `&config.retroarch_cores` → `&installed_core_resolver`
- `crates/grid-core/src/cloud/ops/mod.rs:37` import list, `:336` and `:510` — same swap
- `crates/grid-core/src/firmware/routing.rs:38` import list and `:375` — same swap
- `crates/grid-core/src/library/mod.rs:44` import list, `:1781` and `:2633` — same swap
- `app/src-tauri/src/firmware_service.rs:501` — `&config.retroarch_cores` → `&grid_core::launch::selection::installed_core_resolver`
- `app/src-tauri/src/commands.rs:532` (inside `compatible_emulators`) and `:857` (inside `check_default_emulator_supported`) — `&config.retroarch_cores` → `&installed_core_resolver`, adding `installed_core_resolver` to the `grid_core::launch::selection` import at `:14-16`. Task 4 replaces both with slug-aware closures; this step only keeps the crate compiling.

- [ ] **Step 6: Run the full Rust suite**

Run from `rewrite/`: `cargo test -p grid-core && cargo test -p app`
Expected: PASS.

- [ ] **Step 7: Format, lint, commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add crates/grid-core/src/launch/selection.rs crates/grid-core/src/launch/mod.rs \
        crates/grid-core/src/cloud/ops/mod.rs crates/grid-core/src/firmware/routing.rs \
        crates/grid-core/src/library/mod.rs app/src-tauri/src/firmware_service.rs \
        app/src-tauri/src/commands.rs
git commit -m "rewrite: RetroArch core gate runs before all_platforms (D-RC-1)"
```

---

### Task 3: Platform slugs reach the defaults backfill

**Files:**
- Modify: `crates/grid-core/src/autoconfig/mod.rs:375-391` (`SyncContext`), `:531-534` and `:660-663` (the closures), the 13 `SyncContext { ... }` literals in its test module
- Modify: `crates/grid-core/src/library/mod.rs:645-712` (field + constructor) and `:765-773` area (accessors), `:2142-2148` (`sync_autoconfig`)
- Modify: `app/src-tauri/src/commands.rs:168-174` and `:464-470` (the two `SyncContext` literals), `:143-150` (`list_platforms` recording), `:433-436` (`save_emulator`'s read-out)

**Interfaces:**
- Consumes: `grid_core::autoconfig::installed_compatible_cores` (Task 1); `grid_core::launch::selection::emulator_entry_by_name`.
- Produces:
  - `grid_core::autoconfig::SyncContext` gains `pub platform_slugs: &'a std::collections::BTreeMap<String, String>` (server platform NAME → server platform slug).
  - `grid_core::library::InstallService::set_platform_slugs(&self, slugs: BTreeMap<String, String>)` and `InstallService::platform_slugs(&self) -> BTreeMap<String, String>`.
- `entry::DefaultsContext` is deliberately UNCHANGED: its `installed_cores: &dyn Fn(&str, &str) -> Vec<String>` closure now captures the slug map, so the seam and every existing `entry.rs` test stay as they are.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/grid-core/src/autoconfig/mod.rs`. It writes a real config, a real RetroArch stub with two SNES cores, and checks the backfill picked the SLUG map's first entry (`snes9x`) rather than the fuzzy map's:

```rust
    #[test]
    fn backfill_uses_the_platform_slug_for_core_resolution() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = lock();
        let _env = isolated(temp.path());

        let entry = retroarch_with_cores(temp.path(), &["bsnes", "snes9x"]);
        let config_path = temp.path().join("config.toml");
        let config = Config {
            emulators: vec![entry],
            ..Config::default()
        };
        config.save(&config_path).unwrap();

        let profiles = vec![EmulatorProfile {
            name: "RetroArch (Multi-System)".to_string(),
            match_tokens: vec!["retroarch".to_string()],
            args: "-L \"%core%\" \"%rom%\"".to_string(),
            all_platforms: true,
            ..Default::default()
        }];
        let platforms = vec!["Super Nintendo Entertainment System".to_string()];
        let slugs: BTreeMap<String, String> = [(
            "Super Nintendo Entertainment System".to_string(),
            "snes".to_string(),
        )]
        .into_iter()
        .collect();

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &platforms,
            platform_slugs: &slugs,
            ps3_library_path: String::new(),
            ra: None,
            profiles: &profiles,
        };
        assert!(backfill_all_defaults(&ctx).unwrap());

        let saved = Config::load(&config_path).unwrap();
        assert_eq!(
            saved
                .default_emulators
                .get("Super Nintendo Entertainment System")
                .map(String::as_str),
            Some("RetroArch")
        );
        // The slug map's curated order puts snes9x first; the fuzzy
        // compatibility map is not consulted at all.
        assert_eq!(
            saved
                .retroarch_cores
                .get("Super Nintendo Entertainment System")
                .map(String::as_str),
            Some("snes9x")
        );
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/`: `cargo test -p grid-core backfill_uses_the_platform_slug`
Expected: FAIL to compile — `struct 'SyncContext' has no field named 'platform_slugs'`.

- [ ] **Step 3: Add the field and use it**

In `crates/grid-core/src/autoconfig/mod.rs`, inside `pub struct SyncContext<'a>` (line 375), add this field directly after `pub platforms: &'a [String],`:

```rust
    /// Server platform NAME → server platform slug, as the last successful
    /// platform fetch saw it. Empty when no session has fetched them; an
    /// absent entry means an empty slug, which takes
    /// [`installed_compatible_cores`]' fuzzy fallback (D-RC-2).
    ///
    /// This lives here rather than on [`entry::DefaultsContext`] on purpose:
    /// the defaults seam is the `installed_cores` CLOSURE, and letting that
    /// closure capture the map keeps `DefaultsContext` — and every test
    /// built against it — unchanged.
    pub platform_slugs: &'a BTreeMap<String, String>,
```

Replace the closure at `crates/grid-core/src/autoconfig/mod.rs:531-534` (inside `sync_new_emulator`) and the identical one at `:660-663` (inside `backfill_all_defaults`) with:

```rust
    let installed_cores = |platform: &str, emulator_name: &str| -> Vec<String> {
        let Some(entry) = emulator_entry_by_name(&snapshot, emulator_name) else {
            return Vec::new();
        };
        let slug = ctx
            .platform_slugs
            .get(platform)
            .map(String::as_str)
            .unwrap_or("");
        installed_compatible_cores(platform, slug, entry)
    };
```

- [ ] **Step 4: Fix the 13 existing `SyncContext` literals in the test module**

Each of `crates/grid-core/src/autoconfig/mod.rs` lines 864, 894, 927, 969, 1007, 1043, 1080, 1122, 1156, 1192, 1235, 1270 constructs a `SyncContext { ... }`. Add a module-level helper next to the other test fixtures:

```rust
    /// The empty slug map every pre-D-RC-2 test wants: no slug, hence the
    /// fuzzy fallback, hence exactly the behavior these tests were written
    /// against.
    fn no_slugs() -> BTreeMap<String, String> {
        BTreeMap::new()
    }
```

and in each of the 13 literals insert `platform_slugs: &no_slugs(),` immediately after the `platforms:` line. (A temporary bound is needed if the borrow checker complains: `let slugs = no_slugs();` above the literal, then `platform_slugs: &slugs,`.)

- [ ] **Step 5: Record slugs on the install service**

In `crates/grid-core/src/library/mod.rs`, add the field after `platform_ids` (line 664):

```rust
    /// Server platform name -> server platform slug, as the last successful
    /// platform fetch saw it. The autoconfig defaults backfill resolves
    /// RetroArch cores slug-first (D-RC-2) and grid-core holds no session of
    /// its own to fetch these with.
    platform_slugs: RwLock<BTreeMap<String, String>>,
```

initialize it in `build` after `platform_ids: RwLock::new(BTreeMap::new()),` (line 709):

```rust
            platform_slugs: RwLock::new(BTreeMap::new()),
```

and add the accessors next to `set_platform_ids` / `platform_ids` (line 765):

```rust
    /// Records the server platform slugs the app just fetched.
    pub fn set_platform_slugs(&self, slugs: BTreeMap<String, String>) {
        *self.platform_slugs.write().unwrap() = slugs;
    }

    /// The platform slugs [`Self::set_platform_slugs`] last recorded; empty
    /// until the app has fetched them once.
    pub fn platform_slugs(&self) -> BTreeMap<String, String> {
        self.platform_slugs.read().unwrap().clone()
    }
```

Then update `sync_autoconfig` (line 2140-2148) to read and pass them:

```rust
        let platforms = self.known_platforms();
        let platform_slugs = self.platform_slugs();
        let ctx = autoconfig::SyncContext {
            config_path: &self.config_path,
            platforms: &platforms,
            platform_slugs: &platform_slugs,
            ps3_library_path: autoconfig::ps3_library_path(&library_path),
            ra: self.ra_credentials(),
            profiles: &self.profiles,
        };
```

- [ ] **Step 6: Feed the slugs from the app layer**

In `app/src-tauri/src/commands.rs`, inside `list_platforms`, directly after the `install.set_platform_ids(...)` call (line 148):

```rust
        // Slug-first RetroArch core resolution (D-RC-2) needs the server's
        // own slug for each platform; like the ids above, this is recorded
        // from the FULL list, not the assignable subset.
        install.set_platform_slugs(
            platforms
                .iter()
                .map(|p| (p.name.clone(), p.slug.clone()))
                .collect(),
        );
```

Then, still in `list_platforms`, read them out before the blocking hop and pass them into the `SyncContext` (line 168):

```rust
            let slugs = install.platform_slugs();
            let outcome = tokio::task::spawn_blocking(move || {
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &assignable,
                    platform_slugs: &slugs,
                    ps3_library_path: String::new(),
                    ra,
                    profiles,
                };
                autoconfig::backfill_all_defaults(&ctx)
            })
            .await;
```

In `save_emulator`, widen the read-out at line 433-436 and pass it through at line 464:

```rust
    let (platforms, platform_slugs, ra) = match state.install.as_ref() {
        Ok(install) => (
            install.known_platforms(),
            install.platform_slugs(),
            install.ra_credentials(),
        ),
        Err(_) => (Vec::new(), std::collections::BTreeMap::new(), None),
    };
```

```rust
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &platforms,
                    platform_slugs: &platform_slugs,
                    ps3_library_path: autoconfig::ps3_library_path(&library_path),
                    ra,
                    profiles,
                };
```

- [ ] **Step 7: Run the Rust suite**

Run from `rewrite/`: `cargo test -p grid-core && cargo test -p app`
Expected: PASS, including `backfill_uses_the_platform_slug_for_core_resolution`.

- [ ] **Step 8: Format, lint, commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add crates/grid-core/src/autoconfig/mod.rs crates/grid-core/src/library/mod.rs \
        app/src-tauri/src/commands.rs
git commit -m "rewrite: platform slugs reach the emulator defaults backfill"
```

---

### Task 4: Tauri commands for the core picker

**Files:**
- Modify: `app/src-tauri/src/commands.rs` (`PlatformRef`, `compatible_emulators` at `:509-539`, new `retroarch_core_options`, new `set_retroarch_core`, `set_default_emulator` at `:541-555`, `check_default_emulator_supported` at `:837-864`, `apply_set_default_emulator` at `:866-877`, `mod merge_tests` at `:903+`)
- Modify: `app/src-tauri/src/lib.rs:265-300` (handler list)

**Interfaces:**
- Consumes: `grid_core::autoconfig::installed_compatible_cores` (Task 1); `grid_core::launch::selection::{is_retroarch_name, installed_core_resolver, mapping_value_for_platform, emulator_entry_by_name, compatible_emulator_names_for_platform, emulator_supports_platform}` (Task 2); `InstallService::platform_slugs` (Task 3).
- Produces (the frontend in Task 5 calls all three):
  - `compatible_emulators(platforms: Vec<PlatformRef>) -> Result<BTreeMap<String, Vec<String>>, String>` — keyed by platform NAME, unchanged answer shape.
  - `retroarch_core_options(platforms: Vec<PlatformRef>) -> Result<BTreeMap<String, Vec<String>>, String>`
  - `set_retroarch_core(platform: String, core: String) -> Result<(), String>`
  - `pub struct PlatformRef { pub name: String, pub slug: String }`

- [ ] **Step 1: Write the failing tests**

Add to `mod merge_tests` in `app/src-tauri/src/commands.rs` (it already has `use super::*;` and a `fn config_with(emulators: &[&str], defaults: &[(&str, &str)]) -> Config`). Add the fixture helper first:

```rust
    /// A config holding one RetroArch entry whose executable really exists,
    /// with `core_ids` installed beside it in every host extension so
    /// `installed_core_ids` (cores.rs:516) finds them on any host.
    fn config_with_retroarch(dir: &std::path::Path, core_ids: &[&str]) -> Config {
        let exe = dir.join("retroarch");
        std::fs::write(&exe, b"binary").unwrap();
        let cores_dir = dir.join("cores");
        std::fs::create_dir_all(&cores_dir).unwrap();
        for id in core_ids {
            for extension in ["so", "dylib", "dll"] {
                std::fs::write(cores_dir.join(format!("{id}_libretro.{extension}")), b"").unwrap();
            }
        }
        Config {
            emulators: vec![EmulatorEntry {
                name: "RetroArch".to_string(),
                path: exe.to_string_lossy().into_owned(),
                args: "-L \"%core%\" \"%rom%\"".to_string(),
                ..Default::default()
            }],
            ..Config::default()
        }
    }

    #[test]
    fn core_options_lists_installed_cores_in_slug_order() {
        let temp = tempfile::tempdir().unwrap();
        let config = config_with_retroarch(temp.path(), &["bsnes", "snes9x"]);
        let profiles = load_profiles();
        assert_eq!(
            core_options_for(
                &config,
                profiles,
                "Super Nintendo Entertainment System",
                "snes"
            ),
            vec!["snes9x".to_string(), "bsnes".to_string()]
        );
    }

    #[test]
    fn core_options_is_empty_without_a_retroarch_entry() {
        let config = config_with(&["PCSX2"], &[]);
        let profiles = load_profiles();
        assert!(core_options_for(
            &config,
            profiles,
            "Super Nintendo Entertainment System",
            "snes"
        )
        .is_empty());
    }

    #[test]
    fn set_retroarch_core_refuses_a_core_that_is_not_installed() {
        let temp = tempfile::tempdir().unwrap();
        let config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let err = check_retroarch_core_installed(
            &config,
            profiles,
            "Super Nintendo Entertainment System",
            "snes",
            "bsnes",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "bsnes is not an installed RetroArch core for Super Nintendo Entertainment System"
        );
    }

    #[test]
    fn set_retroarch_core_accepts_an_installed_core_and_a_blank_clear() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";
        assert!(
            check_retroarch_core_installed(&config, profiles, platform, "snes", "snes9x").is_ok()
        );
        apply_set_retroarch_core(&mut config, platform, "snes9x");
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("snes9x")
        );

        assert!(check_retroarch_core_installed(&config, profiles, platform, "snes", "  ").is_ok());
        apply_set_retroarch_core(&mut config, platform, "  ");
        assert!(config.retroarch_cores.is_empty());
    }

    #[test]
    fn set_default_emulator_records_the_first_core_only_when_unset() {
        // D-RC-4: picking RetroArch records a core, but never overwrites one.
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_with_retroarch(temp.path(), &["bsnes", "snes9x"]);
        let profiles = load_profiles();
        let platform = "Super Nintendo Entertainment System";

        apply_record_retroarch_core(&mut config, profiles, platform, "snes", "RetroArch");
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("snes9x")
        );

        // A saved core survives a second pick.
        config
            .retroarch_cores
            .insert(platform.to_string(), "bsnes".to_string());
        apply_record_retroarch_core(&mut config, profiles, platform, "snes", "RetroArch");
        assert_eq!(
            config.retroarch_cores.get(platform).map(String::as_str),
            Some("bsnes")
        );
    }

    #[test]
    fn set_default_emulator_records_no_core_for_a_native_emulator() {
        let mut config = config_with(&["PCSX2"], &[]);
        let profiles = load_profiles();
        apply_record_retroarch_core(&mut config, profiles, "PlayStation 2", "ps2", "PCSX2");
        assert!(config.retroarch_cores.is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run from `rewrite/`: `cargo test -p app merge_tests`
Expected: FAIL to compile — `cannot find function 'core_options_for'` (and the other three helpers).

- [ ] **Step 3: Add `tempfile` as an app dev-dependency if it is missing**

Check `app/src-tauri/Cargo.toml` for a `[dev-dependencies]` `tempfile` entry. If absent, add `tempfile = "3"` under `[dev-dependencies]`.

- [ ] **Step 4: Add `PlatformRef` and the four helpers**

In `app/src-tauri/src/commands.rs`, extend the selection import at lines 14-16 to:

```rust
use grid_core::launch::selection::{
    compatible_emulator_names_for_platform, emulator_entry_by_name, emulator_supports_platform,
    is_retroarch_name, mapping_value_for_platform,
};
```

Add `PlatformRef` next to the other command payload types (immediately above `compatible_emulators`, line 509):

```rust
/// One platform the Emulators panel is asking about: the NAME every config
/// map is keyed by, plus the server SLUG that drives slug-first core
/// resolution (D-RC-2). Both sides use exactly these two field names — see
/// `PlatformRef` in `app/src/lib/api.ts`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PlatformRef {
    pub name: String,
    #[serde(default)]
    pub slug: String,
}

/// The FIRST RetroArch entry in config order — the one the emulator select
/// offers, and therefore the one whose installed cores the core picker
/// lists. `None` when no configured entry is a RetroArch build.
fn first_retroarch_entry<'a>(
    config: &'a Config,
    profiles: &[EmulatorProfile],
) -> Option<&'a EmulatorEntry> {
    config.emulators.iter().find(|entry| {
        is_retroarch_name(&entry.name)
            || profile_for_entry(&entry.name, &entry.path, profiles)
                .is_some_and(|p| is_retroarch_name(&p.name))
    })
}

/// The installed compatible cores the picker offers for one platform, or
/// `[]` when there is no RetroArch entry or nothing compatible is installed.
fn core_options_for(
    config: &Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
    platform_slug: &str,
) -> Vec<String> {
    first_retroarch_entry(config, profiles)
        .map(|entry| autoconfig::installed_compatible_cores(platform_name, platform_slug, entry))
        .unwrap_or_default()
}

/// [`set_retroarch_core`]'s guard. A blank `core` (which CLEARS the mapping)
/// always passes; any other value must be one this platform's picker would
/// have offered.
fn check_retroarch_core_installed(
    config: &Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
    platform_slug: &str,
    core: &str,
) -> Result<(), String> {
    let trimmed = core.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if core_options_for(config, profiles, platform_name, platform_slug)
        .iter()
        .any(|installed| installed == trimmed)
    {
        Ok(())
    } else {
        Err(format!(
            "{trimmed} is not an installed RetroArch core for {platform_name}"
        ))
    }
}

/// [`set_retroarch_core`]'s merge logic, mirroring
/// [`apply_set_default_emulator`]: a blank `core` removes the `platform` key
/// (exact match first, then case-insensitive); otherwise the value is
/// upserted under the exact key, else a case-insensitive match's key, else a
/// new key.
fn apply_set_retroarch_core(config: &mut Config, platform: &str, core: &str) {
    let trimmed = core.trim();
    if trimmed.is_empty() {
        remove_platform_key(&mut config.retroarch_cores, platform);
        return;
    }
    upsert_platform_key(&mut config.retroarch_cores, platform, trimmed);
}

/// D-RC-4: making a RetroArch entry a platform's default also records the
/// first installed compatible core — but ONLY when no non-blank core is
/// saved for that platform. A saved core is never overwritten here; the core
/// picker ([`set_retroarch_core`]) is the only way to change one.
///
/// A no-op for a blank name, a name matching no entry, and any entry that is
/// not a RetroArch build.
fn apply_record_retroarch_core(
    config: &mut Config,
    profiles: &[EmulatorProfile],
    platform_name: &str,
    platform_slug: &str,
    name: &str,
) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    if mapping_value_for_platform(&config.retroarch_cores, platform_name).is_some() {
        return;
    }

    // Scoped so the immutable borrow of `config.emulators` ends before the
    // mutable borrow of `config.retroarch_cores` below.
    let first = {
        let Some(entry) = emulator_entry_by_name(&config.emulators, trimmed) else {
            return;
        };
        let is_retroarch = is_retroarch_name(&entry.name)
            || profile_for_entry(&entry.name, &entry.path, profiles)
                .is_some_and(|p| is_retroarch_name(&p.name));
        if !is_retroarch {
            return;
        }
        autoconfig::installed_compatible_cores(platform_name, platform_slug, entry)
            .into_iter()
            .next()
    };

    if let Some(core) = first {
        upsert_platform_key(&mut config.retroarch_cores, platform_name, &core);
    }
}
```

- [ ] **Step 5: Rewrite the three commands**

Replace `compatible_emulators` (lines 509-539) with:

```rust
/// The emulator names that support each requested platform, keyed by the
/// platform NAME that was asked about. One config + profile load answers the
/// whole batch; each platform runs the ported
/// `compatible_emulator_names_for_platform` (doc 04 §2), so names come back
/// in config order with blank-named entries skipped.
///
/// Each request carries the platform's server SLUG as well as its name, so
/// the RetroArch support gate resolves cores slug-first (D-RC-2).
///
/// The Emulators panel calls this to build its per-platform default
/// selector, which offers only compatible emulators — matching Python's
/// `_on_default_platform_changed` (emulator_ui_mixin.py:598).
#[tauri::command]
pub async fn compatible_emulators(
    platforms: Vec<PlatformRef>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let profiles = load_profiles();
        Ok(platforms
            .into_iter()
            .map(|platform| {
                let slug = platform.slug.clone();
                let resolver = move |entry: &EmulatorEntry, name: &str| -> Vec<String> {
                    autoconfig::installed_compatible_cores(name, &slug, entry)
                };
                let names = compatible_emulator_names_for_platform(
                    &config.emulators,
                    &platform.name,
                    profiles,
                    &resolver,
                );
                (platform.name, names)
            })
            .collect())
    })
    .await
    .map_err(|e| format!("compatible_emulators did not finish: {e}"))?
}

/// The installed libretro cores the core picker offers for each requested
/// platform, keyed by platform NAME. Every platform is answered against the
/// FIRST RetroArch entry in config order — the entry the emulator select
/// would offer — or `[]` when there is none (design §3.3).
#[tauri::command]
pub async fn retroarch_core_options(
    platforms: Vec<PlatformRef>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let profiles = load_profiles();
        Ok(platforms
            .into_iter()
            .map(|platform| {
                let options = core_options_for(&config, profiles, &platform.name, &platform.slug);
                (platform.name, options)
            })
            .collect())
    })
    .await
    .map_err(|e| format!("retroarch_core_options did not finish: {e}"))?
}

/// Saves `platform`'s libretro core. A blank `core` clears it. Refuses
/// anything the picker would not have offered, with the verbatim message
/// `<core> is not an installed RetroArch core for <platform>`.
///
/// The slug comes from the install service's recorded platform list rather
/// than the caller, so a stale frontend cannot steer core resolution.
#[tauri::command]
pub async fn set_retroarch_core(
    state: State<'_, AppState>,
    platform: String,
    core: String,
) -> Result<(), String> {
    // Read out before the blocking hop: `State` is not `Send`.
    let slugs = match state.install.as_ref() {
        Ok(install) => install.platform_slugs(),
        Err(_) => BTreeMap::new(),
    };
    tokio::task::spawn_blocking(move || {
        let profiles = load_profiles();
        let slug = slugs.get(&platform).cloned().unwrap_or_default();
        modify_config(&Config::default_path(), |config| {
            // Inside the closure so the check and the write see the same
            // config; an Err here aborts the write (config_write.rs).
            check_retroarch_core_installed(config, profiles, &platform, &slug, &core)?;
            apply_set_retroarch_core(config, &platform, &core);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_retroarch_core did not finish: {e}"))?
}
```

Replace `set_default_emulator` (lines 541-555) with:

```rust
#[tauri::command]
pub async fn set_default_emulator(
    state: State<'_, AppState>,
    platform: String,
    name: String,
) -> Result<(), String> {
    let slugs = match state.install.as_ref() {
        Ok(install) => install.platform_slugs(),
        Err(_) => BTreeMap::new(),
    };
    tokio::task::spawn_blocking(move || {
        let profiles = load_profiles();
        let slug = slugs.get(&platform).cloned().unwrap_or_default();
        modify_config(&Config::default_path(), |config| {
            // Both writes happen in the ONE closure, so the support check,
            // the default, and the recorded core all see the same config.
            check_default_emulator_supported(config, &platform, &name, &slug, profiles)?;
            apply_set_default_emulator(config, &platform, &name);
            apply_record_retroarch_core(config, profiles, &platform, &slug, &name);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_default_emulator did not finish: {e}"))?
}
```

And update `check_default_emulator_supported` (lines 848-864) to take the slug and build the resolver:

```rust
fn check_default_emulator_supported(
    config: &Config,
    platform: &str,
    name: &str,
    platform_slug: &str,
    profiles: &[EmulatorProfile],
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let resolver = |entry: &EmulatorEntry, platform_name: &str| -> Vec<String> {
        autoconfig::installed_compatible_cores(platform_name, platform_slug, entry)
    };
    let supported = emulator_entry_by_name(&config.emulators, trimmed)
        .is_some_and(|entry| emulator_supports_platform(entry, platform, profiles, &resolver));
    if supported {
        Ok(())
    } else {
        Err(format!("{trimmed} does not support {platform}"))
    }
}
```

Fix any existing `check_default_emulator_supported` call in `mod merge_tests` by inserting `""` as the new fourth argument.

- [ ] **Step 6: Register the new command**

In `app/src-tauri/src/lib.rs`, in the `tauri::generate_handler![...]` list (line 265), add after `commands::compatible_emulators,` (line 293):

```rust
            commands::retroarch_core_options,
            commands::set_retroarch_core,
```

- [ ] **Step 7: Run the Rust suite**

Run from `rewrite/`: `cargo test -p grid-core && cargo test -p app`
Expected: PASS.

- [ ] **Step 8: Format, lint, commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src-tauri/Cargo.toml
git commit -m "rewrite: retroarch_core_options and set_retroarch_core commands"
```

---

### Task 5: Frontend core picker

**Files:**
- Modify: `app/src/lib/api.ts:11` area (`PlatformRef`), `:322-323` (`compatibleEmulators`) and the surrounding `api` object
- Modify: `app/src/lib/emulators/defaults.ts` (append), `app/src/lib/emulators/defaults.test.ts` (append)
- Modify: `app/src/lib/Emulators.svelte` — imports (`:19`), state (`:38-41`), `refreshCompatible` (`:282-294`), the compatibility effect (`:205-216`), `selectFor` (`:446-448`), `handleDefaultChange` (`:450-457`), the defaults markup (`:733-758`), the style block (`:1310+`)

**Interfaces:**
- Consumes (Task 4): `compatible_emulators(platforms: PlatformRef[])`, `retroarch_core_options(platforms: PlatformRef[])`, `set_retroarch_core(platform, core)`; existing `LaunchDefaults.retroarch_cores: Record<string, string>` (`app/src/lib/api.ts:140-144`).
- Produces:
  - `export type PlatformRef = { name: string; slug: string }` in `api.ts`
  - `api.retroarchCoreOptions(platforms: PlatformRef[]): Promise<Record<string, string[]>>`
  - `api.setRetroarchCore(platform: string, core: string): Promise<void>`
  - `export function isRetroarchName(name: string): boolean` in `emulators/defaults.ts`
  - `export const NO_CORE_VALUE = ''`
  - `export type PlatformCoreSelect = { visible: boolean; options: string[]; selected: string; disabled: boolean }`
  - `export function platformCoreSelect(defaults: LaunchDefaults | null, platformName: string, selectedEmulator: string, coreOptions: string[]): PlatformCoreSelect`

- [ ] **Step 1: Write the failing vitest cases**

Append to `app/src/lib/emulators/defaults.test.ts`, and change its import line 3 to `import { isRetroarchName, NO_CORE_VALUE, NO_DEFAULT_VALUE, platformCoreSelect, platformDefaultSelect } from './defaults';`. Also change the local `launchDefaults` helper (lines 5-7) to accept cores:

```ts
function launchDefaults(
  entries: Record<string, string>,
  cores: Record<string, string> = {}
): LaunchDefaults {
  return { default_emulators: entries, retroarch_cores: cores, launch_args: '' };
}

describe('isRetroarchName', () => {
  it('matches any casing and any surrounding text', () => {
    expect(isRetroarchName('RetroArch')).toBe(true);
    expect(isRetroarchName('retroarch (multi-system)')).toBe(true);
    expect(isRetroarchName('My RETROARCH Build')).toBe(true);
  });

  it('does not match a different emulator', () => {
    expect(isRetroarchName('PCSX2')).toBe(false);
    expect(isRetroarchName('')).toBe(false);
  });
});

describe('platformCoreSelect', () => {
  it('is hidden when the row’s selected emulator is not RetroArch', () => {
    const result = platformCoreSelect(launchDefaults({}), 'SNES', 'Snes9x', ['snes9x']);
    expect(result.visible).toBe(false);
  });

  it('is visible for a RetroArch selection', () => {
    const result = platformCoreSelect(launchDefaults({}), 'SNES', 'RetroArch', ['snes9x']);
    expect(result.visible).toBe(true);
    expect(result.options).toEqual(['snes9x']);
    expect(result.disabled).toBe(false);
  });

  it('a saved core that is still installed stays selected', () => {
    const defaults = launchDefaults({}, { SNES: 'bsnes' });
    expect(platformCoreSelect(defaults, 'SNES', 'RetroArch', ['snes9x', 'bsnes']).selected).toBe(
      'bsnes'
    );
  });

  it('a saved core that is no longer installed falls back to the first option', () => {
    // D-RC-5: display-only fallback; nothing is rewritten.
    const defaults = launchDefaults({}, { SNES: 'bsnes' });
    expect(platformCoreSelect(defaults, 'SNES', 'RetroArch', ['snes9x']).selected).toBe('snes9x');
  });

  it('the platform key lookup is case-insensitive', () => {
    const defaults = launchDefaults({}, { snes: 'bsnes' });
    expect(platformCoreSelect(defaults, 'SNES', 'RetroArch', ['snes9x', 'bsnes']).selected).toBe(
      'bsnes'
    );
  });

  it('no installed core yields an empty, disabled select', () => {
    const result = platformCoreSelect(launchDefaults({}, { SNES: 'bsnes' }), 'SNES', 'RetroArch', []);
    expect(result.visible).toBe(true);
    expect(result.options).toEqual([]);
    expect(result.selected).toBe(NO_CORE_VALUE);
    expect(result.disabled).toBe(true);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run from `rewrite/app`: `npx vitest run src/lib/emulators/defaults.test.ts`
Expected: FAIL — `platformCoreSelect is not a function` / no export `isRetroarchName`.

- [ ] **Step 3: Implement the helpers**

Append to `app/src/lib/emulators/defaults.ts`:

```ts
/** The `<select>` value that means "no core for this platform". */
export const NO_CORE_VALUE = '';

/**
 * Whether `name` is a RetroArch build. Mirrors the backend's
 * `is_retroarch_name` (crates/grid-core/src/launch/selection.rs): the name
 * contains "retroarch", case-insensitively.
 */
export function isRetroarchName(name: string): boolean {
  return name.toLowerCase().includes('retroarch');
}

/** What one platform row's core `<select>` renders. */
export type PlatformCoreSelect = {
  /** True only when the row's selected emulator is a RetroArch build. */
  visible: boolean;
  /** The installed compatible core ids, in backend order. */
  options: string[];
  /** The value shown selected — always one of `options`, or [`NO_CORE_VALUE`]. */
  selected: string;
  /** True when no compatible core is installed. */
  disabled: boolean;
};

/** The core saved for `platformName`, or `''`. Case-insensitive lookup. */
function savedCoreFor(defaults: LaunchDefaults | null, platformName: string): string {
  if (!defaults) return '';
  const folded = platformName.toLowerCase();
  const key = Object.keys(defaults.retroarch_cores).find((k) => k.toLowerCase() === folded);
  return key ? defaults.retroarch_cores[key] : '';
}

/**
 * The core select for `platformName`. `coreOptions` is the backend's
 * `retroarch_core_options` answer for that platform, so only cores actually
 * installed beside the RetroArch executable are ever offered.
 *
 * A saved core that is no longer installed shows the first option instead
 * (D-RC-5). This is DISPLAY only — falling back never writes the fallback.
 */
export function platformCoreSelect(
  defaults: LaunchDefaults | null,
  platformName: string,
  selectedEmulator: string,
  coreOptions: string[]
): PlatformCoreSelect {
  const saved = savedCoreFor(defaults, platformName);
  const selected = coreOptions.includes(saved) ? saved : (coreOptions[0] ?? NO_CORE_VALUE);
  return {
    visible: isRetroarchName(selectedEmulator),
    options: coreOptions,
    selected,
    disabled: coreOptions.length === 0,
  };
}
```

- [ ] **Step 4: Run the vitest suite**

Run from `rewrite/app`: `npx vitest run`
Expected: PASS.

- [ ] **Step 5: Wire the API**

In `app/src/lib/api.ts`, add the type next to `Platform` (line 11):

```ts
/** One platform a batched emulator/core lookup is asking about. Field names
 *  match the backend's `PlatformRef` (app/src-tauri/src/commands.rs). */
export type PlatformRef = { name: string; slug: string };
```

and replace the `compatibleEmulators` entry (lines 321-323) with:

```ts
  /** Emulator names supporting each platform, keyed by the platform name asked about. */
  compatibleEmulators: (platforms: PlatformRef[]) =>
    invoke<Record<string, string[]>>('compatible_emulators', { platforms }),
  /** Installed libretro cores offered for each platform, keyed by platform name. */
  retroarchCoreOptions: (platforms: PlatformRef[]) =>
    invoke<Record<string, string[]>>('retroarch_core_options', { platforms }),
  setRetroarchCore: (platform: string, core: string) =>
    invoke<void>('set_retroarch_core', { platform, core }),
```

- [ ] **Step 6: Wire the panel**

In `app/src/lib/Emulators.svelte`:

Extend the import at line 19 to:

```ts
  import {
    NO_CORE_VALUE,
    NO_DEFAULT_VALUE,
    platformCoreSelect,
    platformDefaultSelect,
  } from './emulators/defaults';
```

and add `type PlatformRef,` to the `./api` import list at the top.

Add state after `compatibleError` (line 41):

```ts
  // The backend's `retroarch_core_options` answer, keyed by platform NAME.
  // Fetched on the same trigger set as `compatible`, because both depend on
  // the emulator list and on which core files are on disk.
  let coreOptions = $state<Record<string, string[]>>({});
```

Replace `refreshCompatible` (lines 282-294) with:

```ts
  async function refreshCompatible(refs: PlatformRef[]) {
    if (refs.length === 0) {
      compatible = {};
      return;
    }
    try {
      compatible = await api.compatibleEmulators(refs);
      compatibleError = null;
    } catch (err) {
      compatibleError = errorMessage(err);
    }
  }

  async function refreshCoreOptions(refs: PlatformRef[]) {
    if (refs.length === 0) {
      coreOptions = {};
      return;
    }
    try {
      coreOptions = await api.retroarchCoreOptions(refs);
      compatibleError = null;
    } catch (err) {
      // Shares the compatibility error slot (design §3.4) so a core-options
      // failure cannot clear a real defaults error.
      compatibleError = errorMessage(err);
    }
  }
```

Replace the derived + effect at lines 205-216 with:

```ts
  // Both inputs of the compatibility and core answers: the platforms they
  // are asked about, and the emulator list the backend draws them from.
  // Reading both here is what makes a freshly added (or installed) emulator
  // show up in the per-platform selects without a reload.
  let compatibilityInputs = $derived({
    platformRefs: platforms.map((p) => ({ name: p.name, slug: p.slug })),
    emulatorNames: emulators.map((e) => e.name).join(','),
  });

  $effect(() => {
    const { platformRefs, emulatorNames } = compatibilityInputs;
    void emulatorNames;
    refreshCompatible(platformRefs);
    refreshCoreOptions(platformRefs);
  });
```

Add a core selector and change handler next to `selectFor` (lines 446-457):

```ts
  function coreSelectFor(platformName: string, selectedEmulator: string) {
    return platformCoreSelect(
      defaults,
      platformName,
      selectedEmulator,
      coreOptions[platformName] ?? []
    );
  }

  async function handleCoreChange(platformName: string, value: string) {
    try {
      await api.setRetroarchCore(platformName, value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }
```

`handleDefaultChange` (line 450) already calls `refreshDefaults()` after the write, which is what re-derives the core row from the core the backend may have just recorded (D-RC-4). Leave it as it is.

In the markup, replace the `<li class="defaults-row">` block (lines 733-758) with:

```svelte
            <li class="defaults-row">
              <label class="platform-name" for={selectId}>{p.name}</label>
              <!-- `default-select-<platformId>` is the per-platform select's
                   test id; its `id` (used by the label) is
                   `default-emulator-<platformId>`. -->
              <select
                data-testid={`default-select-${p.id}`}
                id={selectId}
                disabled={choice.disabled}
                value={choice.selected}
                onchange={(e) => handleDefaultChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
              >
                {#if choice.disabled}
                  <option value={NO_DEFAULT_VALUE}>No compatible emulator</option>
                {:else}
                  <option value={NO_DEFAULT_VALUE}>(none)</option>
                  {#each choice.options as name (name)}
                    <option value={name}>{name}</option>
                  {/each}
                {/if}
              </select>
              {#if core.visible}
                <label class="visually-hidden" for={coreId}>Core</label>
                <select
                  data-testid={`default-core-${p.id}`}
                  id={coreId}
                  disabled={core.disabled}
                  value={core.selected}
                  onchange={(e) => handleCoreChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
                >
                  {#if core.disabled}
                    <option value={NO_CORE_VALUE}>No installed core</option>
                  {:else}
                    {#each core.options as id (id)}
                      <option value={id}>{id}</option>
                    {/each}
                  {/if}
                </select>
              {/if}
            </li>
```

and extend the two `{@const}` lines directly above it (line 734-735) to four:

```svelte
            {@const selectId = `default-emulator-${p.id}`}
            {@const choice = selectFor(p.name)}
            {@const coreId = `default-core-${p.id}`}
            {@const core = coreSelectFor(p.name, choice.selected)}
```

Add the label class to the style block, next to `.defaults-row select` (line 1325):

```css
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
  }
```

- [ ] **Step 7: Typecheck and test**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS with no type errors.

- [ ] **Step 8: Commit**

```bash
cd rewrite
git add app/src/lib/api.ts app/src/lib/emulators/defaults.ts \
        app/src/lib/emulators/defaults.test.ts app/src/lib/Emulators.svelte
git commit -m "rewrite: per-platform RetroArch core picker in the Emulators panel"
```

---

### Task 6: E2E coverage

**Files:**
- Modify: `e2e/specs/emulators.spec.ts` (the `before` hook at lines 27-56, and new `it` cases after line 192)
- Modify: `e2e/seed/launch-seed.mjs` (lines 22-33 doc comment, and the stub section at lines 67-69)
- Modify: `e2e/specs/launch.spec.ts:190-215` (the "No RetroArch core is configured" case)

**Interfaces:**
- Consumes: the `data-testid="default-core-<platformId>"` select from Task 5; `config.toml`'s `[retroarch_cores]` table.
- Produces: no new group and no new seed script — the `emulators` group builds its stub in the spec's own `before` hook (it has no `seed_script_for_group` entry in `rewrite/scripts/e2e.sh`, unlike `launch`), so `scripts/e2e.sh` needs no change.

- [ ] **Step 1: Install core files for the `emulators` group**

In `e2e/specs/emulators.spec.ts`, inside `before` (after `chmodSync(stubPath, 0o755);`, line 37):

```ts
    // Design D-RC-1: RetroArch's platform support is now decided by the
    // core files installed beside its executable, so the stub needs a
    // `cores/` sibling. Two SNES cores in the bundled slug map's curated
    // order (romm-platform-cores.json maps "snes" to
    // ["snes9x", "snes9x2010", "bsnes"]) and no Arcade core at all, so
    // platform 1 offers RetroArch and platform 2 (Arcade) does not.
    const coresDir = path.join(stubsDir, 'cores');
    mkdirSync(coresDir, { recursive: true });
    for (const core of ['snes9x', 'bsnes']) {
      writeFileSync(path.join(coresDir, `${core}_libretro.so`), '');
    }
```

- [ ] **Step 2: Add the `emulators` assertions**

Replace the final `it(...)` of `e2e/specs/emulators.spec.ts` (lines 176-192) with these four, keeping the existing `selectValue` helper and `configPath()`/`readFileSync` imports:

```ts
  /** Waits until config.toml contains `line`, or fails with a useful message. */
  async function waitForConfigLine(line: string) {
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(configPath(), 'utf-8').includes(line);
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `config.toml never contained ${line}`,
      },
    );
  }

  /** The `<option>` values of a select, in DOM order. */
  async function optionValues(testIdName: string): Promise<string[]> {
    return browser.execute((selector) => {
      const el = document.querySelector(selector) as HTMLSelectElement | null;
      if (!el) throw new Error(`no element matched ${selector}`);
      return Array.from(el.options).map((o) => o.value);
    }, testId(testIdName));
  }

  it('assigns a per-platform default and records a core in config.toml', async () => {
    await selectValue('default-select-1', 'RetroArch Renamed');
    await waitForConfigLine('"Super Nintendo Entertainment System" = "RetroArch Renamed"');
    // D-RC-4: picking RetroArch also records the first installed compatible
    // core, which the slug map orders snes9x before bsnes.
    await waitForConfigLine('"Super Nintendo Entertainment System" = "snes9x"');
  });

  it('lists the installed cores for the RetroArch row, in slug-map order', async () => {
    await $(testId('default-core-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the core select never appeared for the RetroArch default',
    });
    await expect(await optionValues('default-core-1')).toEqual(['snes9x', 'bsnes']);
  });

  it('changing the core rewrites the retroarch_cores line', async () => {
    await selectValue('default-core-1', 'bsnes');
    await waitForConfigLine('"Super Nintendo Entertainment System" = "bsnes"');
  });

  it('does not offer RetroArch for a platform with no installed core', async () => {
    // Arcade (platform 2) needs fbneo/mame2003_plus/mame; only SNES cores
    // are installed, so D-RC-1's gate keeps RetroArch out of the list even
    // though its autoprofile sets all_platforms: true.
    const names = await optionValues('default-select-2');
    expect(names).not.toContain('RetroArch Renamed');
  });
```

- [ ] **Step 3: Give the `launch` group's RetroArch stub a core**

In `e2e/seed/launch-seed.mjs`, replace the paragraph at lines 23-33 of the header comment with:

```js
/*
 * The RetroArch stub's basename ("retroarch") is a literal entry in the
 * repo-root emulator-autoprofiles.json's match_tokens for the "RetroArch
 * (Multi-System)" profile. Under design D-RC-1 that profile's
 * `all_platforms: true` no longer implies support: `emulator_supports_platform`
 * (launch/selection.rs) runs the RetroArch core gate FIRST, so the stub
 * needs a real core file beside it to be selectable at all. This seed writes
 * `stubs/cores/snes9x_libretro.so` for exactly that reason, and deliberately
 * writes NO [retroarch_cores] table — which is the setup launch.spec.ts's
 * "no RetroArch core configured" test needs.
 */
```

and after the `chmodSync(retroarch, 0o755);` line (line 69):

```js
const coresDir = path.join(stubsDir, 'cores');
mkdirSync(coresDir, { recursive: true });
writeFileSync(path.join(coresDir, 'snes9x_libretro.so'), '');
```

- [ ] **Step 4: Seed the "no core" launch case from config, not the UI**

Replace the final `it(...)` of `e2e/specs/launch.spec.ts` (lines 190-215) with:

```ts
  it('shows the verbatim "No RetroArch core is configured" error for a coreless RetroArch default', async () => {
    // Seeded through config.toml rather than the UI (design §4): the
    // Emulators panel now records a core whenever RetroArch is picked
    // (D-RC-4), so the UI can no longer produce this state. Rewriting only
    // the default_emulators line preserves the InstantExit path edit the
    // previous test made.
    const text = readFileSync(configPath(), 'utf-8');
    writeFileSync(
      configPath(),
      text.replace(`"${PLATFORM}" = "LongRunner"`, `"${PLATFORM}" = "RetroArch"`),
    );

    await openDetails();
    await $(testId('details-play')).click();
    await $(testId('details-error')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'details-error never appeared for the unmapped RetroArch default',
    });
    // The template layer's own validation error ("No RetroArch core is
    // configured...") is wrapped by prepare_emulator_launch (spawn.rs) as
    // "Invalid launch arguments: <e>" — pinned by spawn.rs's
    // an_argument_failure_is_wrapped unit test. Assert the full wrapped
    // string verbatim, not just the inner message.
    await expect($(testId('details-error'))).toHaveText(
      'Invalid launch arguments: No RetroArch core is configured for this platform. ' +
        'Set one in Emulators > Defaults.',
    );
  });
```

Make sure `writeFileSync` is in the `node:fs` import at the top of `e2e/specs/launch.spec.ts` (it already imports `readFileSync`); add it if it is not.

- [ ] **Step 5: Run both groups**

Run from `rewrite/`: `scripts/e2e.sh emulators` then `scripts/e2e.sh launch`
Expected: both groups PASS. Do not set `E2E_SKIP_BUILD` — the stamp ignores sources.

- [ ] **Step 6: Commit**

```bash
cd rewrite
git add e2e/specs/emulators.spec.ts e2e/specs/launch.spec.ts e2e/seed/launch-seed.mjs
git commit -m "rewrite: E2E coverage for the RetroArch core gate and core picker"
```

---

### Task 7: Docs

**Files:**
- Modify: `docs/porting/04-emulator-launch.md:274-284` (the `emulator_supports_platform` list), `:982-993` (milestone 3 deviations), and a new section appended after the milestone 8 deviations section
- Modify: `docs/porting/05-emulator-autoconfig.md:220-227` (Layer 1's `assign_profile_platform_defaults` bullets)
- Modify: `rewrite/README.md:114-131` (Residual manual checklist)

- [ ] **Step 1: Doc 04 §2 predicate order**

Replace the numbered list at `docs/porting/04-emulator-launch.md:274-284` (`emulator_supports_platform` is supplied by the window …) with:

```markdown
`emulator_supports_platform` is supplied by the window
(grid-launcher.py:3556). The Rust port REORDERS this list — see "Rust port
deviations (RetroArch cores)" below (D-RC-1):

1. Blank platform → `True` (grid-launcher.py:3558).
2. **(Rust port only, D-RC-1)** If the entry name or the profile name matches
   "retroarch", support is decided purely by whether at least one compatible
   core is INSTALLED beside the executable — before the `all_platforms`
   check, not after it.
3. Resolve the profile for the entry; if the profile has `all_platforms` → `True`
   (grid-launcher.py:3569).
4. *(Reference position of the RetroArch check.)* If the entry name or the profile
   name matches "retroarch", support is decided purely by whether any installed
   RetroArch core is mapped to the platform (grid-launcher.py:3572).
5. If no profile matched at all → `True` (grid-launcher.py:3579).
6. Otherwise the profile's `platform_keywords` are expanded into concrete server
   platform names and compared case-insensitively (grid-launcher.py:3586).
```

- [ ] **Step 2: Close milestone 3 deviation 4**

At `docs/porting/04-emulator-launch.md:992`, replace deviation 4 with:

```markdown
4. ~~RetroArch platform support = a non-blank `retroarch_cores` config entry, not a scan of installed core files.~~ **Closed (RetroArch cores, D-RC-1):** support is now a scan of the core files installed beside the executable, resolved slug-first; the `retroarch_cores` config map is no longer an input to the predicate.
```

- [ ] **Step 3: New deviations section**

Append to the end of `docs/porting/04-emulator-launch.md`:

```markdown
## Rust port deviations (RetroArch cores)

Implements `docs/superpowers/specs/2026-09-04-retroarch-platform-cores-design.md`.
Rust paths are relative to `rewrite/`.

1. **D-RC-1 — RetroArch support is decided by installed cores.** The RetroArch gate
   runs BEFORE the `all_platforms` shortcut in `emulator_supports_platform`
   (`crates/grid-core/src/launch/selection.rs`), and asks a `CoreResolver`
   (`(entry, platform) -> Vec<String>`) rather than the `retroarch_cores` config map.
   The shipped autoprofile still sets `all_platforms: true`; it simply no longer wins
   for RetroArch. Closes milestone 3 deviation 4.
2. **D-RC-2 — slug-first core resolution.** `installed_compatible_cores`
   (`crates/grid-core/src/autoconfig/mod.rs`) takes the bundled
   `romm-platform-cores.json` curated list for a non-blank, known slug, and otherwise
   falls back to the fuzzy `cores_for_platform` match. The reference returned `[]` for
   any non-blank slug missing from the map; falling back keeps a drifted RomM slug
   spelling from silently dropping RetroArch support.
3. **D-RC-3 — core picker inline.** Each platform row in the Emulators panel renders a
   second `<select>` (`data-testid="default-core-<platformId>"`) shown only when that
   row's selected emulator is a RetroArch build
   (`app/src/lib/Emulators.svelte`, `app/src/lib/emulators/defaults.ts`).
4. **D-RC-4 — picking RetroArch records a core.** `set_default_emulator`
   (`app/src-tauri/src/commands.rs`) inserts the first installed compatible core when
   the platform has no non-blank one, in the same `modify_config` closure as the
   default write. A saved core is never overwritten by that path.
5. **D-RC-5 — display fallback stays display-only.** A saved core that is no longer
   installed shows the first option; nothing is rewritten until the user changes it.
6. Out of scope, deliberately: core downloads, per-game core overrides, and any change
   to `%core%` template handling.
```

- [ ] **Step 4: Doc 05 Layer 1 note**

At `docs/porting/05-emulator-autoconfig.md:225-227`, replace the last bullet of the `assign_profile_platform_defaults` list with:

```markdown
- For RetroArch platforms whose default now points at this emulator and that have no
  core default yet, the first installed compatible core is recorded
  (grid_launcher/emulator/autoconfig.py:391). **Rust port (D-RC-2):** the
  installed-core filter resolves candidates SLUG-FIRST —
  `autoconfig::installed_compatible_cores` uses the bundled
  `romm-platform-cores.json` list for a known server slug and falls back to the fuzzy
  `cores_for_platform` match on a slug miss or an empty slug. Slugs reach grid-core
  through `SyncContext::platform_slugs`, recorded by `list_platforms` on the install
  service alongside `set_known_platforms`.
```

- [ ] **Step 5: README checklist rows**

In `rewrite/README.md`, append to the "Residual manual checklist" bullet list (after the "Basic-auth mode" bullet, line 131):

```markdown
- **RetroArch core picker**: with a real RetroArch install, confirm the Emulators
  panel shows a Core select only on rows whose emulator is RetroArch, that it lists
  only cores present in the RetroArch `cores/` directory, and that changing it writes
  `[retroarch_cores]` in `config.toml`.
- **RetroArch platform gating**: with a real RetroArch install missing a platform's
  core, confirm that platform's emulator select does NOT offer RetroArch, and that
  installing the core makes it appear after the panel refreshes.
```

- [ ] **Step 6: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add docs/porting/04-emulator-launch.md docs/porting/05-emulator-autoconfig.md \
        rewrite/README.md
git commit -m "rewrite: record the RetroArch core deviations in docs 04/05 and the README"
```

---

## Self-review notes

**Spec coverage:** §1 report 1 → T2; §1 report 2 → T4+T5. D-RC-1 → T2; D-RC-2 → T1+T3; D-RC-3 → T5; D-RC-4 → T4; D-RC-5 → T5; D-RC-6 → nothing changes `%core%` handling or adds a download path. §3.1 → T2; §3.2 → T1+T3; §3.3 → T4; §3.4 → T5; §3.5 → T7; §4 → T1/T2/T3 (grid-core unit), T4 (`set_default_emulator` recording), T5 (vitest), T6 (`emulators` + `launch` groups; `emulator-catalog` is untouched, as §4 requires). §5 out-of-scope items are not implemented anywhere.

**Type consistency:** `installed_compatible_cores(platform_name, platform_slug, entry)` has the same argument order in T1, T3 and T4. `CoreResolver<'a> = &'a dyn Fn(&EmulatorEntry, &str) -> Vec<String>` is the same in T2's definition, T2's call sites, and T4's closures. `PlatformRef { name, slug }` matches between the Rust struct (T4) and the TS type (T5), and the invoke key is `platforms` on both sides. `platformCoreSelect(defaults, platformName, selectedEmulator, coreOptions)` has the same signature in T5's test, its implementation, and its Svelte caller. `default-core-<platformId>` is the same string in T5's markup and T6's assertions.

**Judgment points the executor should expect:**
1. `DefaultsContext` (`crates/grid-core/src/autoconfig/entry.rs:53-68`) is deliberately NOT given a slug field — the closure captures `SyncContext::platform_slugs` instead. This keeps `entry.rs` and its ~20 tests unchanged. If a later requirement needs the slug inside `assign_profile_platform_defaults` itself, widen the closure to three arguments then.
2. T2 step 4 rewrites existing tests. Read each one before editing; two of them (`compatible_names_preserve_config_order_and_skip_blank_names`, `supports_all_platforms_profile_is_always_true`) assert behavior the reorder changes for RetroArch-named profiles only.
3. T4's `set_default_emulator` and `set_retroarch_core` gain a `State<'_, AppState>` first parameter. Tauri fills it in; the JS `invoke` payload is unchanged, so `api.ts` needs no edit for it.
