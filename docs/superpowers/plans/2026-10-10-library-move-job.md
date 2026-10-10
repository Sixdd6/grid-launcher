# B9 phase 9b — Move existing files (background job)

Parent: `docs/superpowers/plans/2026-10-10-parity-decisions-milestone.md` (B9). Builds on the 9a seam committed in a5070da (`library/path_change.rs`, `commands/library_path.rs`, `settings/library.ts` `MOVE_AVAILABLE`).

## User decisions (2026-10-10)
- The move is a **background job** (Downloads-strip row). While it runs, installs, launches, cloud transfers and another path change are refused. Resumable after a crash or close.
- **Verification:** every file's size is checked; `saves/` gets a full SHA-256 compare; `games/` and `emulators/` get a sampled SHA-256 (first and last 1 MiB).
- **Old folder:** removed after a successful move only if it is empty.
- **Unreachable library (general rule, also for a failed commit):** when the library root is unreachable — e.g. the new drive was unplugged after a copy, or at any startup — GRID alerts the user and offers exactly two choices: **Relocate the library** (point GRID at the folder where the library now is; references are rewritten, no files are moved) or **Close without changing anything**. GRID cannot run without a valid library, so there is no "continue anyway". This replaces the planner's "Retry / Undo" proposal and also covers phase 9c's "library root absent" case.

## Summary
"Move existing files" moves `games/`, `emulators/`, `saves/` from the current root to a new folder. Same-volume items are renamed; cross-volume items are copied to a staging folder and verified, and sources are deleted only after the move commits. After the files move, every stored path is rewritten (registry, config, emulator config files, Windows junctions), then `switch_library_root` runs. A cancelled move leaves the old library exactly as it was.

## Evidence
- 9a seam: `validate_new_library_path`/`check_new_library_path`, `prepare_library_dir`, `switch_library_root` (path_change.rs), `protected_library_roots`, `disk_usage` (skips links/reparse points), `is_strictly_inside`; `LibraryActivity` + `library_activity()` (commands/library_path.rs); installs = `snapshot().has_live_entry()`; `MOVE_AVAILABLE=false` in settings/library.ts.
- layout_migration.rs: private `RewritePlan.library_forms` (~:45); `rewrite_paths_in_text` (~:1151, case-sensitive prefix match); `rewritten_config` (~:986); `rewrite_emulator_configs`/`emulator_config_files` (~:1016/:1059); rename-only moves (~:1111-1127); crash-ordering lesson in `step_rewrite` (~:963-972).
- user_data_links.rs: `ensure_user_data_links` (~:68) repoints wrong links; `move_tree_preferring_dest` (~:210) copies+deletes per file and merges destination-wins; `copy_symlink` is Unsupported on Windows; Rust reports junctions as symlinks → these helpers are NOT reused. Windows junctions are absolute (`link_dir` ~:302); unix links are relative.
- registry.rs: `Registry::rewrite_paths` (~:758) covers `REWRITE_PATH_COLUMNS` + `ps3_trophy_paths` in one transaction; check `ps4_content` at the start of step 2.
- Startup (app/src-tauri/src/lib.rs): Python import → args migration → layout migration → relinked resync → InstallService / LaunchService → `.setup()`. Notice pattern: `AppState.layout_migration` → `commands::updates::layout_migration_notice` → `stores/layoutMigration.svelte.ts`.
- Queue: `InstallService::admit` is the single admission point; `admit_external`/`complete_external` for slotless rows; `DownloadEntry` has no free-text detail field.
- Cloud gap: `handle_session_finished` (cloud_service.rs) sleeps the upload delay (≤60 s) before `pool.trigger` — `transfers_active()` is false meanwhile — and captures installed rows before the sleep (stale after a move).
- `launch_emulator` (commands.rs) spawns standalone emulators not tracked by LaunchService.
- No new crate: `sha2`, `windows-sys` (`GetDiskFreeSpaceExW`), `libc` (`statvfs`), std `File::set_times` (mtimes must be kept — the cloud "local newer" check compares them).
- `LAYOUT_VERSION_CURRENT` = 2 (`paths.rs:26`). `layout_version_for_library_path` (`paths.rs:170`) returns V1 when `games`/`emulators`/`saves` exist → after a move the new root would be stamped 1 → keep the old root's version (2) in commit step d; test it.

### Absolute library paths GRID writes (inventory, planner addendum)
| # | File | Writer | Keys | Repaired by a later sync? | In `emulator_config_files`? |
|---|---|---|---|---|---|
| 1 | RPCS3 `portable/config/vfs.yml` | `autoconfig/rpcs3.rs:191` | `/dev_hdd0/`, `/games/` (canonicalized :213) | No (add-only) | Yes |
| 2 | RPCS3 `config/games.yml` | `autoconfig/rpcs3.rs:300` (from `library/mod.rs:2459`) | `<game_id>: <path>` (canonicalized) | Only on reinstall | Yes |
| 3 | PCSX2 `inis/PCSX2.ini` | `autoconfig/pcsx2.rs:316-326` | `[Folders] Bios` (canonicalized, `firmware/routing.rs:173-177`) | No (add-only) | Yes |
| 4 | shadPS4 `launcher/qt_ui.ini` | `autoconfig/shadps4_qt.rs:64` | `[version_manager] versionSelected` | Yes (overwritten on sync) | Yes |
| 5 | xemu `xemu.toml` (beside the exe) | `autoconfig/xemu.rs:179`, keys :146-163 | `[sys.files] bootrom_path, flashrom_path, hdd_path, eeprom_path` (single-quoted) | No (add-only) | **No — add it** |
| 6 | Windows junctions | `user_data_links.rs:302` | absolute target | Re-created by `ensure_user_data_links` | n/a |

Not affected (checked): RetroArch, DuckStation, Cemu, Dolphin, PPSSPP, Azahar, Eden, Redream writers; firmware (except PCSX2 Bios); `cloud_sync_state`; image cache; logs. `compat_tool_installs[].path` lives under the DATA dir (`launch/compat.rs:52`) → NOT rewritten. `native_wineprefix` = `<game dir>/prefix` → moves with `games/`, covered by `Registry::rewrite_paths`. The xemu cloud sync reads `hdd_path` (`xemu.rs:233`), so a stale value would break xemu saves after a move.

## Design
1. **What moves:** only exact `games`, `emulators`, `saves` at the old root; layout must be `LAYOUT_VERSION_CURRENT`; unrelated files and dot-dirs stay; a top-level link moves as a link; rows and native dirs outside the root are untouched.
2. **Strategy per item:** probe a rename of a temp dir old→new root: success = Rename (done in Commit); `CrossesDevices` = Copy. Injectable (trait/closure) plus an e2e-feature env switch to force Copy.
3. **Phases:**
   - **Copy** (cancellable): Copy items → `<new>/.grid-library-move/<name>`, chunked with cancel checks and progress; mtime and read-only kept; symlinks copied as links; junctions/reparse points never followed (recorded, re-created at commit); resume skips files with equal length+mtime. Verify per item (user decision above). Sources untouched.
   - **Commit** (not cancellable; each step idempotent; re-run whole on resume): a) place items (staging → `<new>/<name>`; Rename items old → new); b) rewrite the emulator text files (rows 1–5) in place — this is the ONLY repair for the add-only rows 1, 3, 5; c) `Registry::rewrite_paths` with the relocate closure; d) one `modify_config`: relocate emulator path/save fields + `switch_library_root` keeping `library_layout_version` = 2 (atomic switch point); e) `ensure_user_data_links` per managed entry (re-creates Windows junctions; checks unix relative links) + re-create recorded absolute links; f) `run_emulator_sync`, `fresh_install=false` — only for keys a sync overwrites (shadPS4); g) journal → Cleanup.
   - **Cleanup** (background): per Copy item, re-check source vs copy (count, length, mtime): equal → delete source; differs → keep source and report "changed during the move; left at <path>"; remove the old root only if empty; delete the journal.
4. Rename items move in Commit, so the old root stays complete until then (trivial cancel, crash-safe Copy).
5. **Relocate matcher** (pure): value = old-root spelling + separator + `games|emulators|saves` + separator-or-end → new root + rest. Spellings: as typed, `~`-expanded, **`fs::canonicalize(old_root)` (dunce on Windows)**, both separator forms, and a `\\`-escaped form if needed; case-insensitive on Windows. Files that stored the canonical form get the new root in canonical form. Generalize `rewrite_paths_in_text` to take `(forms, &dyn Fn)`, keep the old signature as a wrapper; it must accept single-quoted values (xemu). Parameterize on `(old_root, new_root)` only (multi-library friendly).
6. **Journal:** `Config::default_path().parent()/library-move.json` (beside `grid-launcher.db`; honors `GRID_LAUNCHER_DATA_DIR`; `firmware::routing::config_dir_of` derives that directory). Atomic temp + fsync + rename on each state change, never per byte. Fields: `version, old_root, new_root, created_new_root, phase, items[{name, strategy, state, bytes, files}], links[]`. No secrets. Separate from config.toml.
7. **Cancel** (Copy phase only): delete staging, remove `<new>` if GRID created it and it is empty, delete the journal, open the gate → config, rows and old root unchanged.
8. **Refusals:** `LayoutNotCurrent`, `OldRootMissing`, `TargetHasLibraryFolders`, `NotEnoughSpace` (Copy items + max(1 GiB, 2%); rechecked on resume), `MoveActive`, plus the 9a checks.
9. **Gate:** Tauri-free `LibraryGate` in grid-core, owned by AppState, cloned into InstallService/LaunchService; closed at construction when a journal exists; the move start closes the gate FIRST, then reads `library_activity()` under the queue lock, and reopens + refuses if busy.
10. **Startup:** after the layout migration, before services: `relocate::recover` finishes a Commit synchronously; Copy/Cleanup resume in the background from `.setup()` with a toast "Resuming the library move to <new>". If the library root (old during Copy, new after Commit) is unreachable → the **unreachable-library dialog** (user decision): Relocate the library / Close without changing anything.

## Steps (one implementer, sequential)
1. grid-core `library/relocate.rs` pure part + tests first: matcher (incl. canonical form of a symlinked root, single-quoted values), plan/preflight refusals, journal round trip (unknown version refused), `LibraryGate`; generalize `rewrite_paths_in_text`; add xemu `xemu.toml` to `emulator_config_files` behind an `is_xemu` check (also fixes a latent layout-migration gap). Check `ps4_content` for paths first.
2. grid-core executor + `tests/library_relocate.rs`: copy/verify/phases/recover/cancel; failpoint after each journal write → resume reaches the same end state; forced-Copy; verify catches truncated/missing files; conflicting target refused; source changed before cleanup kept; unrelated root file stays; layout version stays 2; Windows-only junction re-creation; unix relative links resolve.
3. grid-core hooks: queue `JobKey::LibraryMove`, kind `library_move`, a `detail` field, live-check split (a move is not a live install); gate checks in `admit`/`admit_external`/`uninstall`/`start_fresh_*`; launch gate + `LaunchError::LibraryBusy`; path_change refusals and `LibraryActivity.library_move`.
4. src-tauri: `library_move_service.rs`; commands `library_move_preview/start/cancel/status` + the unreachable-library commands (relocate references to a chosen folder, close); lib.rs recover/resume/gate; cloud_service gap fix (keep the transfer count raised across the delay, wait for the gate, reload rows + config after the wait); `ensure_library_idle` on: uninstall_game, set_library_path, start fresh ×3, save/delete/launch/install/update_emulator, install_compat_tool, install_firmware_for_platform, install_ps3_firmware, set_retroachievements_credentials, cloud_upload, cloud_restore, native_add/remove_save_path, link_installed_row, remove_from_library, update_game, retry_install. Fix the wrong comment at `lib.rs:163-166` (resync does not rewrite PCSX2 Bios).
5. Frontend logic: api.ts, downloads/format.ts (+test: "Library move" label, actions per status), settings/library.ts (+test: remove `MOVE_AVAILABLE`, `move-confirm` step, preview summary, pane state), the unreachable-library dialog logic.
6. Designer: move-confirm step (size; "Same drive — instant" vs "Copies N GB; X GB free"; what is blocked; "close emulators you opened yourself"), pane "Moving to <new>…" state, Downloads row label/detail, the unreachable-library dialog.
7. e2e group `library-move` (seed a current-layout library + installed ROM + stub emulator + saves link + a PCSX2-style ini and an xemu.toml holding absolute old-root paths): a) move completes; tree, config, ini/toml rewritten, launch works, unrelated file stays; b) resume from a seeded Copy-phase journal (forced-copy env, e2e feature); c) unreachable root at startup → dialog → Relocate to the folder where the files are → library works.
8. Docs: SPEC.md (move, blocking, resume, cancel, unreachable library), ARCHITECTURE.md (relocate, library_move_service, journal).
9. Gate on Windows + WSL, `scripts/e2e.sh library-move install`, commit `--only`. No server calls change, so no API check.

Size: L, about 7–8 working days.

## Risks
- Data loss: sources are deleted only in Cleanup, after commit and a recheck.
- Files in use: Windows rename fails → job fails naming the folder; Linux → cleanup keeps changed sources. Standalone emulator launches are untracked (follow-up).
- Stale paths: after commit, scan registry + config for values still under `<old>/{games,emulators,saves}`; if found, log and keep that item's source.
- Long paths (>260), read-only files, OneDrive placeholders are not followed.
- Proton/Wine `user.reg` absolute paths are not rewritten (low impact).
- grid-core stays Tauri-free; journal, events and errors carry paths only.
