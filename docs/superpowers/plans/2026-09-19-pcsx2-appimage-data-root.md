# PCSX2 AppImage data root — implementation plan (2026-09-19)

## Problem

PCSX2's Linux AppImage roots every data directory at `<AppImage dir>/PCSX2/` whenever the
`APPIMAGE` environment variable is set (`pcsx2/Pcsx2Config.cpp`: `DataRoot =
Path::RealPath(Path::Combine(Path::GetDirectory(getenv("APPIMAGE")), "PCSX2"))`). Neither
`portable.ini` beside the AppImage nor `-portable` changes that: `ShouldUsePortableMode` looks
in AppRoot, which for an AppImage is inside the mount. On Windows (`pcsx2-qt.exe` from the 7z)
`portable.ini` beside the exe makes the exe directory the data root.

GRID assumed `<exe dir>`, so on Linux the user-data links, the managed `inis/PCSX2.ini`, the
RetroAchievements writer, the config readers, cloud save/state/screenshot resolution, firmware
(`bios`) routing and removal salvage all point at a directory PCSX2 never reads.

## User rulings (2026-09-19)

- Repair collisions: **saves/ wins** (the existing destination-wins policy of
  `ensure_user_data_links`). No new merge mode.
- **Stop writing `portable.ini`** for an AppImage executable. Keep it for every other PCSX2
  binary.
- Layout version bump to 2 with a startup repair step: approved.

## Design

### Root rule (one place, pure)

`crates/grid-core/src/autoconfig/paths.rs`:
- `pub fn is_appimage(path: &Path) -> bool` — extension equals `appimage`, case-insensitive.
  Move the private copy out of `library/emulator_removal.rs` and reuse it.
- `pub fn pcsx2_data_root(exe: &Path) -> Option<PathBuf>` — `emulator_dir(exe)` joined with
  `PCSX2` when `is_appimage(exe)`, else `emulator_dir(exe)`. No filesystem access.

`crates/grid-core/src/autoconfig/mod.rs`, beside `is_pcsx2`:
- `pub fn emulator_data_root(entry, profiles) -> Option<PathBuf>` — PCSX2 entries go through
  `pcsx2_data_root`; every other entry keeps `paths::emulator_dir`. Blank path → `None`.

`crates/grid-core/src/library/user_data_links.rs`:
- `pub fn user_data_root(profile: &EmulatorProfile, exe: &Path) -> PathBuf` — synthetic entry
  from the profile name and exe, then `emulator_data_root`, falling back to the exe parent. The
  three link call sites use it.

### `%EMULATOR_DIR%` for PCSX2

`cloud::ops::emulator_dir_for`, `emulator_removal::emulator_dir_for` and
`firmware::routing::targets_for_entry` derive their base from `emulator_data_root`. Relative
profile paths (`memcards/...`, `sstates`, `snaps`, `bios`) then resolve under `PCSX2/` for the
AppImage and stay unchanged for every other emulator.

### Writer and readers

- `pcsx2::resolve_target` → `<root>/inis/PCSX2.ini`; `portable.ini` is written beside the exe
  only when the exe is not an AppImage.
- `readers::pcsx2_data_root_candidates`: for an AppImage the portable root is `<parent>/PCSX2`
  unconditionally (no marker or `-portable` needed); `<parent>` stays as the last fallback.

### Migration: layout v2

- `library/paths.rs`: `LAYOUT_VERSION_V2 = 2`, `LAYOUT_VERSION_CURRENT = LAYOUT_VERSION_V2`.
  `layout_version_for_library_path`: absent/empty → CURRENT; v1-shaped (`games`/`emulators`/
  `saves` present) → V1; other non-empty → 0.
- `layout_migration::run` skips when `version >= CURRENT`. `migrate` runs the v1 steps only when
  `version < V1`, then always `step_data_root_links`, then stamps CURRENT.
- `step_data_root_links`: for each config entry under `<library>/emulators/` with a matched
  profile whose `user_data` is non-empty and whose `user_data_root` differs from the exe dir:
  1. remove each `user_data` name beside the exe that is a link (a real directory or file is left
     alone with a warning);
  2. `create_dir_all(root)` and `ensure_user_data_links(root, saves/<Profile>, user_data)`
     (destination-wins);
  3. reuse the v1 duplicate-profile guard: a second entry of the same profile is skipped with the
     same warning;
  4. record the entry name in `MigrationOutcome::Completed { relinked, .. }`.
  A `portable.ini` beside the AppImage is left alone.
- `step_user_data` (v1) and `rewrite_emulator_configs` locate links and `inis/PCSX2.ini` under
  the data root.
- `app/src-tauri/src/lib.rs`: after `InstallService` is built, resync each `relinked` entry
  through `run_emulator_sync(..., false, autoconfig::sync_new_emulator)` so `[Folders] Bios`
  and the managed keys point at `<root>/bios` in the ini that PCSX2 now reads. Credentials stay
  inside `RaCredentials`; logs carry names and paths only.

## Steps (each with tests beside the changed function)

1. `autoconfig/paths.rs`: `is_appimage`, `pcsx2_data_root`.
2. `autoconfig/mod.rs`: `emulator_data_root`.
3. `autoconfig/pcsx2.rs`: `resolve_target` (root + no `portable.ini` for AppImage); docs.
4. `autoconfig/readers.rs`: AppImage portable root.
5. `firmware/routing.rs`: base via `emulator_data_root`.
6. `cloud/ops/{mod,upload}.rs` + `cloud/dirs.rs` test.
7. `library/user_data_links.rs`: `user_data_root`.
8. `library/mod.rs` and `launch/emu_install.rs`: link at `user_data_root`.
9. `library/emulator_removal.rs`: root-aware `emulator_dir_for` and salvage; shared `is_appimage`.
10. `library/paths.rs`: version constants and detection rule.
11. `library/layout_migration.rs`: gate, root-aware v1 steps, `step_data_root_links`, `relinked`.
12. `autoconfig/mod.rs`: `sync_new_emulator` AppImage end-to-end test (`Bios = <root>/bios`).
13. `app/src-tauri/src/lib.rs`: destructure `relinked`, log, resync.
14. E2E: `emulator-catalog.spec.ts` (ini/bios under `PCSX2/`, no `portable.ini`, no `inis` link
    beside the AppImage), `layout-migration.spec.ts` (version 2).
15. Docs: `ARCHITECTURE.md` library layout; `.claude/skills/emulator-autoconfig/SKILL.md` PCSX2 +
    `user_data_links`; append a "Layout v2" section to
    `docs/superpowers/specs/2026-09-15-library-layout-v1-design.md`.
16. Gate, scoped e2e (`emulator-catalog`, `layout-migration`), `git commit --only` per group.

## Risks

- `%EMULATOR_DIR%` changes meaning for PCSX2 AppImage entries only (now the correct directory).
- `is_pcsx2`'s name fallback: an entry named "pcsx2 …" pointing at another AppImage gets a
  `PCSX2/` root; it already gets the PCSX2 writers. Accepted.
- Windows unchanged (rule keyed on `.appimage`).
- A hand-configured AppImage outside `<library>/emulators/` gets the right writer/reader paths
  but no links (existing policy).
- Destination-wins drops the ini PCSX2 wrote itself on an already-run install; the managed
  copy in `saves/` survives and the resync fixes its `Bios` path.
