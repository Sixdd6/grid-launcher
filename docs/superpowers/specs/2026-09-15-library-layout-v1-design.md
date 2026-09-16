# Library layout v1 — design (2026-09-15)

## Goal

Deleting an emulator must never delete its saves, settings or firmware. Achieve this by
reorganizing the GRID library root into three folders and moving every emulator's user data
out of its install folder. Existing libraries migrate automatically, once, at startup.

## Layout

```
<library>/
  games/<Platform>/            game installs; unchanged internal shape (archive, extraction
                               dir, multi-file dir, native game/ and prefix/, PS3 .vfs)
  emulators/<Emulator>/        the emulator's binaries; folder name is the sanitized profile
                               name with NO release tag (updates install in place)
  saves/<Emulator>/<dir>/      every directory the emulator writes beside its binary
```

- `<Platform>` and `<Emulator>` use the existing `sanitize_component` rules. `<Emulator>` is
  the catalog PROFILE name, so several config entries can share one emulator and one saves
  folder (Dolphin's GameCube and Wii variants).
- Nothing else lives at the library root. Compat tools, cover cache, `config.toml` and the
  registry database stay in the app data directories as today.
- Hand-configured emulators (a config entry whose executable is not under
  `<library>/emulators/`) are never moved, linked or deleted.

## Config

`Config` gains `library_layout_version: u32` (serde default 0).

| value | meaning |
|---|---|
| 0 | legacy layout (`<library>/<Platform>/`, `<library>/Emulators/<Name>-<tag>/`), or unknown |
| 1 | this layout |

A blank `library_path` is version 1 by definition: there is nothing to migrate, and the
first install creates the new shape. Detection is a numeric compare; no directory sniffing.

## User data links

### Catalog

Each profile gains an optional `user_data: [<dir>, ...]` list: directory names, relative to
the emulator install directory, that the emulator writes and that must survive a delete.

| Profile | `user_data` |
|---|---|
| Dolphin | `User` |
| Azahar, Eden, ShadPS4 (Playstation 4) | `user` |
| PPSSPP | `memstick` |
| Cemu, RPCS3 | `portable` |
| KytyPS5 | `_SaveData` |
| PCSX2 | `inis`, `bios`, `memcards`, `sstates`, `snaps`, `cheats`, `textures` |
| DuckStation | `bios`, `memcards`, `savestates`, `screenshots`, `inputprofiles` |
| RetroArch | `saves`, `states`, `system`, `screenshots`, `config` |
| Supermodel | `Saves`, `NVRAM`, `Config` |
| Xenia Canary, Xenia (Xbox 360) | `content`, `cache` (only meaningful where the build is portable, i.e. Windows) |
| ShadPS4 Qt Launcher | `launcher`, `user` |

Profiles without `user_data`: Vita3K and Xenia on Linux (data in the OS data directory,
outside the library), Redream and xemu (loose files beside the binary; see Salvage), Pico-8,
FBNeo, MAME (no catalog source; hand-configured).

### Mechanism

`ensure_user_data_links(install_dir, saves_dir, user_data)` in grid-core:

1. `create_dir_all(saves/<Emulator>/<dir>)`.
2. If `install_dir/<dir>` is a real directory (not a link): move its contents into the
   saves directory (rename when empty target, otherwise merge with `merge_tree_into`), then
   remove it.
3. If `install_dir/<dir>` is absent: create the link.
4. If it is already a link to the right target: nothing. A link to a different target is
   replaced.
5. Linux/macOS: `std::os::unix::fs::symlink`. Windows: an NTFS directory junction (the
   `junction` crate), which needs no privilege. Never a Windows symlink.

Runs at three points: emulator install finalize (after the executable is chosen, BEFORE
`sync_autoconfig`, so writers create files through the links), emulator update (same
place), and the startup migration. Idempotent; returns whether anything changed.

Cloud sync, firmware routing and the config readers resolve emulator-relative paths through
`resolve_best_effort`, which follows links progressively, so they operate on the real
directory under `saves/`. This is pinned by a test that syncs through a linked directory.

### Delete

`delete_emulator` order becomes: salvage → remove install directory → remove config entry.

- **Salvage:** resolve the entry's save, state and screenshot directories (the same
  resolution cloud sync uses). Any that resolves to a real path INSIDE the install directory
  (not through a link) is moved to `saves/<Emulator>/<relative path>`. This protects Redream,
  xemu and any profile whose `user_data` list is incomplete.
- Removing the install directory removes links as links; `saves/<Emulator>` is never
  touched by delete.
- The managed-directory guard accepts `<library>/emulators/` and the legacy
  `<library>/Emulators/` (a delete during a failed migration must still work).

### Reinstall / update

Install into `emulators/<Emulator>` re-creates the links against the existing
`saves/<Emulator>`, so settings, saves and firmware return without a download.

## Games

- `platform_dir(library, platform)` becomes `<library>/games/<Platform>`.
- The resolution fallbacks (`candidate_archives`, `candidate_extracted_dirs`, ROM and
  native launch re-resolution) try the `games/` shape first and the legacy shape second.
- `autoconfig::ps3_library_path` becomes `<library>/games/PlayStation 3`.
- Registry rows keep absolute paths; the migration rewrites them (below).

## Migration

Runs in `app/src-tauri/src/lib.rs` startup, before `InstallService` / `LaunchService` are
built, when `library_layout_version < 1` and `library_path` is non-blank. Implemented in
grid-core (`library::layout_migration`), Tauri-free, with every step idempotent so an
interrupted run resumes on the next startup.

### Preflight (abort without changes on failure)

1. `library_path` exists and is a directory.
2. Nothing conflicting exists: `games/`, `emulators/`, `saves/` may exist only if produced by
   a previous partial run (i.e. they contain nothing that would collide with a pending
   rename). A collision aborts with a message naming the two paths.
3. All moves are renames within the library directory, so no free-space or cross-device
   check is needed; a rename that fails with `CrossesDevices` aborts.

### Steps

1. **Games.** Every top-level directory except `Emulators`, `emulators`, `saves`, `games`
   and dot-directories is renamed to `games/<same name>`. Files at the library root are left
   in place (the legacy bare-root archive fallback remains readable).
2. **Emulators.** Rename `Emulators/` → `emulators/` (two-step through a temporary name on
   case-insensitive filesystems). For each config entry whose path is under it, rename
   `<Name>-<tag>/` → `<Name>/` where `<Name>` is the sanitized PROFILE name, resolved with
   `profile_for_entry` (Dolphin's "Dolphin (GameCube)" and "Dolphin (Wii)" entries share one
   `emulators/Dolphin/` and one `saves/Dolphin/`); an entry with no profile keeps its
   directory name; a target that already exists aborts the step for that entry only, logged.
3. **User data.** For each managed emulator entry with a matched profile, run
   `ensure_user_data_links`.
4. **Path rewrite.** Replace the prefix `<library>/<Platform>/` → `<library>/games/<Platform>/`
   and `<library>/Emulators/<Name>-<tag>/` → `<library>/emulators/<Name>/` in:
   - config: every `emulators[].path`, `save_paths`, `state_paths`;
   - registry: `archive_path`, `extracted_path`, `extracted_dir`, `multi_file_game_dir`,
     `native_executable_path`, `native_wineprefix`, `native_game_dir`, `ps3_iso_path`,
     `ps3_trophy_paths`, in one transaction;
   - RPCS3 `portable/config/vfs.yml`: `/dev_hdd0/` and `/games/` values (explicit rewrite;
     the normal writer is add-only);
   - PCSX2 `inis/PCSX2.ini` `[Folders] Bios`;
   - ShadPS4 Qt launcher `launcher/qt_ui.ini` `versionSelected`.
5. **Version.** Write `library_layout_version = 1`.

### Failure

Any step error: log at warn level (paths only, never secrets), leave the version at 0,
continue startup with the library readable through the legacy fallbacks, and surface one
dismissible startup notice in the UI ("Library reorganization did not finish: <message>. It
will retry on next launch."). The next startup resumes from the first incomplete step.

### Fresh libraries

A `set_library_path` to a new, empty directory sets `library_layout_version = 1`. Pointing
the config at an existing legacy library sets it to 0 so the migration runs on next start.

## UI

- Emulators view rows show the new install path from config; no component changes.
- One startup notice component for a failed migration (reuse the existing self-update
  notice pattern).
- The delete confirm keeps its two-click flow; the button label becomes
  "Confirm delete (keeps saves)".

## Testing

- `library/layout_migration.rs` unit tests: a temp library in the legacy shape with two
  platforms, one native game, RPCS3 with `.vfs`, two managed emulators (one with a `-latest`
  tag, one Dolphin with two config entries), one hand-configured emulator outside the
  library; assert the resulting tree, the rewritten config and registry, and that a second
  run changes nothing. Interrupted-run test: pre-move half the platforms, run, assert the
  same end state. Collision test aborts with no changes.
- `ensure_user_data_links` tests: absent dir → link; real dir → moved then linked; link to
  wrong target → replaced; idempotent second call. Windows-only test for junction creation.
- Salvage test: Redream-shaped install with `vmu0.bin` beside the binary → moved to
  `saves/Redream/`.
- Delete/reinstall integration test in `tests/emulator_install.rs`: install, write a file
  through a link, delete, assert `saves/` intact and install dir gone, reinstall, assert the
  link resolves to the same file.
- Cloud sync test: a profile save directory reached through a link yields the files.
- Path fallbacks: `paths.rs` tests for both shapes.
- E2E: new `layout-migration` stage seeding a legacy library and asserting the migrated
  tree after startup; update the specs and seeds that hardcode `library/<Platform>` and
  `Emulators/`.

## Out of scope

- A `bios/<platform>` folder (firmware stays in each emulator's user data).
- Moving Vita3K or Linux Xenia data out of the OS data directory.
- Rewriting absolute paths inside emulator configs other than the three listed.
- Deduplicating firmware across emulators.
