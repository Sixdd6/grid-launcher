# RetroArch platform support and per-platform core picker — design

Date: 2026-09-04. Scope: the Rust/Tauri rewrite (`rewrite/`). Follows milestone 9
and the emulator-selector fix (main `dd67683`).

## 1. Problem

Two reports from desktop testing:

1. After a catalog install of RetroArch, the Emulators panel shows RetroArch as the
   selected emulator for every platform that has no saved default, including the
   native Windows platforms and PS4/PS5.
2. There is no way to choose which libretro core a platform launches with, so the
   auto-picked core (first compatible installed core in compatibility-map order,
   e.g. `gpsp` for Game Boy Advance, `dice` for Arcade) is the only option.

### Root cause of report 1

`emulator_supports_platform` (`crates/grid-core/src/launch/selection.rs`) checks the
profile's `all_platforms` flag **before** the RetroArch core gate. The RetroArch
autoprofile sets `all_platforms: true`, so the predicate returns true for every
platform and the core gate never runs. The Python original has the same order
(`grid-launcher.py:3556`), and doc 04 §2 records it. The config is not written for
those platforms: `platformDefaultSelect` falls back to the first compatible name for
display only, and RetroArch is first in config order.

### Root cause of report 2

The Python panel had a third combo (platform → emulator → core,
`emulator_ui_mixin.py:577` `_refresh_retroarch_core_options`) that listed installed
compatible cores and saved the pick into `default_retroarch_cores`. The rewrite kept
the config map (`Config.retroarch_cores`) and the launch-time `%core%` handling but
never ported the combo.

## 2. Decisions

- **D-RC-1 — RetroArch support is decided by installed cores.** RetroArch supports a
  platform iff at least one core compatible with that platform is installed for that
  RetroArch entry. The `all_platforms` shortcut no longer applies to RetroArch: the
  RetroArch gate runs first. This supersedes doc 04 milestone 3 deviation 4
  ("a non-blank `retroarch_cores` config entry").
- **D-RC-2 — slug-first core resolution.** Compatible cores for a platform come from
  the bundled slug map (`romm-platform-cores.json`, curated preference order) when
  the platform's server slug has an entry; otherwise from the fuzzy compatibility map
  (`cores_for_platform`). Deviation from Python, which returned an empty list for any
  non-empty slug missing from the map: a server slug the map does not know (RomM
  spellings drift) must not silently drop RetroArch support for that platform.
- **D-RC-3 — core picker inline.** Each platform row in the Emulators panel gets a
  second `<select>` shown only when that row's selected emulator is RetroArch.
- **D-RC-4 — picking RetroArch records a core.** Setting a platform's default to a
  RetroArch entry also records the first compatible installed core when no core is
  saved for that platform (Python `_on_default_platform_changed`,
  `emulator_ui_mixin.py:1684`). A saved core is never overwritten by this path.
- **D-RC-5 — display fallback stays display-only.** As with the emulator select, a
  saved core that is no longer installed displays the first option; nothing is
  rewritten until the user changes it.
- **D-RC-6 — no core downloads, no per-game core override, no launch template change.**

## 3. Behavior

### 3.1 Platform support predicate (grid-core `launch/selection.rs`)

New order:

1. Blank platform → true.
2. Entry name or profile name contains "retroarch" (case-insensitive) → true iff
   `installed_compatible_cores(platform, entry)` is non-empty.
3. Profile `all_platforms` → true.
4. No profile → true.
5. Otherwise `platform_matches_keywords`.

The `retroarch_cores` config map is no longer an input to the predicate. Callers
(`compatible_emulator_names_for_platform`, `default_emulator_name_for_platform`,
`available_emulator_name_for_platform`, the Tauri `compatible_emulators` and
`set_default_emulator` commands, and the launch resolver) supply a core resolver
instead.

### 3.2 Installed compatible cores (grid-core `autoconfig`)

`installed_compatible_cores(platform_name, platform_slug, entry) -> Vec<String>`:

1. `candidates` = slug map entry for `platform_slug` when the slug is non-empty and
   present; else `cores_for_platform(platform_name, compatibility_map())`.
2. Result = `candidates` filtered to `installed_core_ids(entry.path)`, in `candidates`
   order. Empty when the entry has no core files.

The existing `installed_cores_for_platform` in `autoconfig/mod.rs` becomes this
function; `DefaultsContext` and `SyncContext` learn platform slugs so the defaults
backfill and `sync_new_emulator` use the same resolution. Slugs reach grid-core the
way names do today: `list_platforms` records them on the install service alongside
`set_known_platforms`, and in the process-wide registry
(`launch/platform_slugs.rs`) that `installed_core_resolver` reads, so the launch,
cloud, firmware and install paths are slug-aware without a signature change.
A platform with no recorded slug — any platform before the first successful
platform fetch — uses an empty slug and therefore the fuzzy fallback.

Effect on auto-picks for a fresh install: Game Boy Advance → `mgba`, Arcade →
`fbneo`, Super Nintendo → `snes9x`. Existing saved cores are untouched.

### 3.3 Tauri commands (`app/src-tauri/src/commands.rs`)

- `compatible_emulators(platforms: Vec<PlatformRef>) -> BTreeMap<String, Vec<String>>`
  where `PlatformRef { name: String, slug: String }`. Same answer shape as today,
  keyed by name.
- New `retroarch_core_options(platforms: Vec<PlatformRef>) -> BTreeMap<String, Vec<String>>`:
  for each platform, the installed compatible cores of that platform's RetroArch
  entry — its saved default when that entry is a RetroArch build, else the **first**
  RetroArch entry in config order — or `[]`. `set_retroarch_core`'s guard resolves
  the entry the same way, so the picker and the guard never disagree when two
  RetroArch builds are configured.
- New `set_retroarch_core(platform: String, core: String) -> Result<(), String>`:
  inside the `modify_config` closure, refuse with
  `"<core> is not an installed RetroArch core for <platform>"` unless `core` is in
  `retroarch_core_options` for that platform; a blank `core` removes the key
  (exact key first, then case-insensitive, same as `apply_set_default_emulator`).
- `set_default_emulator`: after the existing support check, when the chosen entry is
  RetroArch and `retroarch_cores` has no non-blank value for the platform, insert the
  first installed compatible core (D-RC-4). Both writes happen in the one closure.
- `get_launch_defaults` is unchanged (`retroarch_cores` is already returned).

### 3.4 Frontend (`app/src/lib`)

- `api.ts`: `PlatformRef` type; `compatibleEmulators(platforms: PlatformRef[])`,
  `retroarchCoreOptions(platforms: PlatformRef[])`, `setRetroarchCore(platform, core)`.
- `emulators/defaults.ts`: `isRetroarchName(name)` (contains "retroarch",
  case-insensitive, mirrors the backend) and
  `platformCoreSelect(defaults, platformName, selectedEmulator, coreOptions) ->
  { visible, options, selected, disabled }`: `visible` iff `selectedEmulator` is a
  RetroArch name; `selected` = saved core when in `options`, else first option, else
  `''`; `disabled` iff `options` is empty (option text "No installed core").
- `Emulators.svelte`: fetch core options together with compatibility (same trigger
  set: platform list, emulator list, catalog install completion, terminal signature
  effect); render the core select after the emulator select in the same row with
  `data-testid="default-core-<platformId>"`, `id="default-core-<platformId>"`, and a
  visually-hidden label "Core"; `onchange` → `setRetroarchCore` then refresh
  defaults. A core-options fetch failure shows in the existing `compatibleError`
  slot text. Changing the emulator select re-derives the core row from the refreshed
  defaults (the backend may have recorded a core).

### 3.5 Docs

- `docs/porting/04-emulator-launch.md`: §2 predicate order rewritten to §3.1; a new
  "Rust port deviations (RetroArch cores)" section listing D-RC-1..D-RC-5 and marking
  milestone 3 deviation 4 closed.
- `docs/porting/05-emulator-autoconfig.md`: Layer 1 note that the installed-core
  filter resolves slug-first (D-RC-2).
- `rewrite/README.md`: manual checklist rows for the core picker.

## 4. Tests

- grid-core unit: predicate order (RetroArch entry with no cores is unsupported even
  though the profile has `all_platforms`; with a mapped installed core it is
  supported; a non-RetroArch `all_platforms` profile is unaffected); slug-first
  resolution (slug hit uses curated order; unknown slug falls back to fuzzy; empty
  slug falls back to fuzzy); `set_default_emulator` records a core only when unset.
- vitest: `platformCoreSelect` (hidden for non-RetroArch, saved-in-options, fallback
  to first, disabled when empty), `isRetroarchName`.
- E2E (`emulators` group): the seed creates `cores/snes9x_libretro.so` and
  `cores/bsnes_libretro.so` next to the RetroArch stub; selecting RetroArch for
  platform 1 writes both `default_emulators` and `retroarch_cores` (`snes9x`) to
  config.toml; `default-core-1` lists `snes9x, bsnes`; choosing `bsnes` rewrites the
  core line; the Arcade row (no arcade cores installed) does not list RetroArch.
- E2E (`launch` group): the RetroArch stub gains `cores/snes9x_libretro.so` so the
  entry remains selectable; the "No RetroArch core is configured" case is seeded by
  config (RetroArch default, no `retroarch_cores` entry) rather than through the UI,
  since the UI now records a core.
- E2E (`emulator-catalog` group): unchanged assertions; RetroArch is not part of it.

## 5. Out of scope

Core downloads or the buildbot updater, per-game core overrides, changes to `%core%`
template handling, TV mode.
