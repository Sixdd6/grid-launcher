# Plan: make the full gate pass natively on Windows

Base: d374aa8. Evidence: Windows `cargo test --workspace --no-fail-fast` (2064 passed, 69 failed, 3 dangerous tests skipped) and `cargo clippy --keep-going` (17 errors). Runbook: `.claude/WINDOWS_MIGRATION.md` Phase 3.

Classes: **a** test-only Unix assumption · **b** real Windows product bug · **c** Unix-only by design (`#[cfg(unix)]` + one-line why).
Rules: no weakened assertions, no `#[ignore]`, no blanket `allow`. Product bugs get a test.

## Decisions

- **D1 — isolated home for tests.** New `platform::home_dir()` in `crates/grid-core/src/platform.rs`: under `#[cfg(test)]` a non-blank `HOME` wins on every OS; otherwise `directories::UserDirs`, then the existing `HOME` fallback. `autoconfig::paths::home_dir` delegates to it; `library::paths::expand_home` uses it instead of `BaseDirs`; under `cfg(test)` with `HOME` set, `platform::windows_documents_dir()` returns `HOME\Documents`. Unix behavior is unchanged (directories already reads `$HOME`); Windows release builds never read `HOME` (Python `expanduser` parity since 3.8). grid-core stays Tauri-free.
- **D2 — one non-verbatim canonicalize.** `pub fn platform::canonicalize` wraps `dunce::canonicalize` (new dep `dunce = "1"`). In-crate tests compare against `resolve_best_effort` where the product uses it.
- **D3 — batch verification.** The full gate cannot pass before the last batch. A batch is verified when: `cargo fmt --check` passes; its targeted tests pass; a full Windows `cargo test --workspace --no-fail-fast` shows the previous failure set minus this batch, nothing new; clippy shows only known remaining items; and the full gate passes on Linux (WSL). Keep `--skip extract_7z_entry_with_absolute_path` until B0b lands.
- Builds: `CARGO_BUILD_JOBS=4`, Idle priority.

## Batches (disjoint files; one implementer, gate, commit `--only`)

### B0a — home seam, canonicalize helper, profile-writing tests (FIRST)
Files: `crates/grid-core/Cargo.toml`, `Cargo.lock`, `src/platform.rs`, `src/autoconfig/paths.rs`, `src/library/paths.rs`, `src/autoconfig/xemu.rs` (tests), `src/autoconfig/duckstation.rs` (tests).
1. `platform.rs`: `home_dir()` (D1), Documents seam in `windows_documents_dir`, `canonicalize()` (D2); unit tests: HOME override honored under test; canonicalize of a temp dir never starts with `\\?\`; off Windows equals `std::fs::canonicalize`.
2. `autoconfig/paths.rs`: `home_dir` delegates; fix `resolve_best_effort_canonicalizes_an_existing_path` (compare with `platform::canonicalize`) and `resolve_best_effort_clamps_a_leading_parent_dir_at_root` (input from the cwd's root ancestor, expect `root/some/nonexistent/dir`).
3. `library/paths.rs`: `expand_home` via `platform::home_dir`; `candidate_*_dedups_by_string` build the recorded path with `Path::join` like the product; `candidate_archives_expands_leading_tilde` holds `test_env::lock()` and sets `HOME` itself.
4. `xemu.rs` `xemu_blank_path_targets_default_base_root`: also set `APPDATA=<temp>/appdata`; expect `APPDATA/xemu/xemu/xemu.toml` on Windows, XDG path elsewhere; assert `default_base_root()` starts with the temp dir BEFORE `ensure_settings`.
5. `duckstation.rs` `duckstation_candidate_scan_counts_an_empty_valued_key_as_parsed`: assert the second candidate starts with the temp dir before writing.
Canary: before/after the first full run, list `%APPDATA%\xemu`, `%USERPROFILE%\Documents\DuckStation`, `%USERPROFILE%\.config`, `.local`, `PCSX2`, `Library`, `Vita3K`, `fw`, `GridLauncherTest`, `C:\etc` — nothing new may appear.

### B0b — archive-entry traversal on Windows (security)
Files: `src/library/extract.rs`, `tests/extract.rs`, `src/firmware/write.rs`.
1. `is_absolute_entry_path` (extract.rs ~185), same rule on every OS, `pub(crate)`: after `\`→`/`, reject a leading `/`, a drive prefix (`X:` incl. drive-relative `C:x`), or UNC. Covers zip, tar, 7z (`is_unsafe_7z_path`), RAR (`rar_entry_relative_path`). Today 7z/RAR `dest.join("/etc/x")` → `C:\etc\x`.
2. `firmware/write.rs` `safe_relative_path` uses the same rule; unit tests for `/abs`, `\abs`, `C:/x`, `C:x`, `//srv/share/x`. Extend `rar_entry_relative_path_rejects_traversal` likewise.
3. `which_on_path`: on Windows also try each `PATHEXT` extension; `#[cfg(windows)]` test ("PATHEXT exists only on Windows") under `test_env::lock()` finds `7z.exe` for `7z`.
4. `tests/extract.rs`: `#[cfg(unix)]` on `write_zip_with_modes`/`write_7z_with_modes`; add a 7z `C:/evil.txt` case.
Fixes: extract_{zip,tar,7z}_entry_with_absolute_path_fails_and_deletes_dest, rar_entry_relative_path_rejects_traversal. Unskip the 7z test from here on.

### B1 — clippy + emulator_install test file
Files: `app/src-tauri/src/lib.rs`, `src/launch/sessions.rs`, `src/launch/emu_install.rs`, `src/cloud/archive.rs`, `tests/emulator_install.rs`.
`#[cfg(any(target_os = "linux", test))]` on `dmabuf_override_needed`; `#[cfg(unix)]` on sessions.rs test `Command` import + `fn session`, `emu_install.rs` `write_tar_gz`, `archive.rs` `use std::io::Read as _` (same cfg as its users), emulator_install.rs unix-only imports/helpers. Fix `zip_install_extracts_writes_config_entry_and_deletes_the_archive` and `an_existing_config_entry_with_the_same_name_is_replaced_at_its_index` with `.join("bin").join("testemu.sh")`. Re-run both clippy commands with `--keep-going` (previously hidden targets may show new lints). Run the rest of the Windows gate once (hygiene, fmt, npm ci, svelte-check, build, npm test).

### B2 — layout migration (product bug)
File: `src/library/layout_migration.rs`.
1. `preflight` (~503) intersects `entry_names(library/Emulators)` with `entry_names(library/emulators)`; on case-insensitive NTFS both read one directory, so every v0 library with `Emulators/` fails "both already exist". Read a root's entries only when its exact name appears in `entry_names(library)`; same rule in `kept_emulator_pairs` (~233).
2. Fixtures build `ps3_trophy_paths` JSON with `format!` (`\U` → invalid JSON on Windows): use `serde_json` (incl. `assert_migrated_tree` expectation).
3. `tree()`/`contents()` normalize relative paths to `/`; collision message expectation with `.join("games").join("Sony PlayStation 2")`.
4. `#[cfg(unix)]` on `pcsx2_entry`, `PCSX2_USER_DATA`, `stamp_v1`.
Risk: may expose follow-on failures once preflight passes — report before widening scope.

### B3 — autoconfig tests
Files: `src/autoconfig/mod.rs`, `rpcs3.rs`, `readers.rs` (tests only). `ps3_library_path_lives_under_games` via `Path::join`; two shadPS4 Qt tests expect `shadps4_path` with `/` (product writes `/` by design); rpcs3 `games_yml_*` expected dir via `resolve_best_effort`; `pico8_directory_settings_defaults` sets `APPDATA` to temp; `flycast_vmu_rejects_non_vmu_names_and_a_missing_directory` expectation split by platform (product lowercases on Windows by design, readers.rs ~4030).

### B4 — launch
Files: `src/launch/source.rs`, `catalog.rs`, `forge.rs` (tests), `spawn.rs`, `template.rs`.
Host-explicit seams (`merge_platform_override_for(.., "linux")`, `catalog_entries_for_host(.., "linux")`); forge `direct_platforms_gate_*` use a platform no host matches (today they hit example.com on Windows); `#[cfg(unix)]` on the D-Bus test group + `exists_only` ("flatpak D-Bus bus repair is Linux-only; unix-socket addresses are POSIX paths"). **Product:** `template.rs` ~367 `std::fs::canonicalize` → `platform::canonicalize` so RetroArch `-L` never gets `\\?\`; update expectations (template.rs ~580/640/663, spawn.rs ~944).

### B5 — cloud
Files: `src/cloud/candidates.rs` (tests), `src/cloud/ops/tests.rs`, `src/cloud/native.rs`.
`touch_dir_at`: on Windows open the dir with write access + `FILE_FLAG_BACKUP_SEMANTICS`; `set_mtime`: open `write(true)`. **Product:** `restore_native_multi_dir_archive` (~399) passes a non-canonical root to `resolve_under_root` (contract: canonical root) → every existing file below the root canonicalizes to `\\?\`, fails `starts_with`, and is silently skipped: on Windows a native save restore over existing saves writes nothing. Use the `std::fs::canonicalize` form when `target_root` exists, else `resolve_best_effort`; add a test for an existing nested dir. Expect follow-on failures (these tests panicked at line 1 before).

### B6 — specials, emulator removal, firmware
Files: `src/library/specials/native.rs`, `specials/ps3.rs`, `library/emulator_removal.rs`, `firmware/routing.rs` (tests), `firmware/rpcs3.rs`.
`ranked()` normalizes to `/` (5 tests); `route_trophy_and_nested_hdd0` chained joins; `salvage_strips_...` compares canonical forms; `saves_dir_honors_absolute_and_relative_paths` uses a platform-absolute temp path. **Product:** `spawn_rpcs3_installfw` passes `\\?\` to `--installfw` and `current_dir` — extract a pure builder using `platform::canonicalize`, test it, update the Linux test (~187).

### B7 — install_service integration tests
File: `tests/install_service.rs`. Chained joins in the 10 failing assertions; `Ps3Vfs::new` (~1740) uses `grid_core::platform::canonicalize`. Final verification: full gate, all 7 commands, no skips; e2e in WSL (B2, B4, B5 touch user-visible flows).

## Test → class → batch
a/0a: xemu_blank_path_targets_default_base_root*, duckstation_candidate_scan_counts_an_empty_valued_key_as_parsed*, and via the seam: xemu default_base_root_for_each_host, azahar_config_path_candidates_include_appdata_and_home_fallbacks, duckstation_candidate_order_under_xdg_overrides, duckstation_writes_to_the_emulator_dir_even_when_read_elsewhere, pcsx2_expands_a_tilde_path_and_creates_no_literal_tilde_directory, vita3k_pref_path_priority, candidates_are_deduped_case_insensitively, non_appimage_retroarch_still_expands_tilde_against_the_user_home, compares_case_insensitively_and_expands_a_leading_tilde, tilde_expands_to_home, a_tilde_path_is_expanded_before_it_is_matched; plus resolve_best_effort_canonicalizes_an_existing_path, resolve_best_effort_clamps_a_leading_parent_dir_at_root, candidate_archives_dedups_by_string, candidate_extracted_dirs_dedups_by_string. (*previously skipped, dangerous)
b/0b: extract_7z_entry_with_absolute_path_fails_and_deletes_dest*, extract_zip_…, extract_tar_…, rar_entry_relative_path_rejects_traversal.
a/1: zip_install_extracts_writes_config_entry_and_deletes_the_archive, an_existing_config_entry_with_the_same_name_is_replaced_at_its_index.
2 (b and a): derive_game_dirs_table, the_temp_name_fallback_finishes_a_half_done_root_rename, a_games_collision_aborts_without_changes, run_migrates_the_fixture, run_is_a_no_op_the_second_time, an_interrupted_run_resumes_to_the_same_end_state, a_failure_after_the_text_rewrites_still_resumes_to_the_same_end_state, an_emulator_target_collision_skips_that_entry_only.
a/3: ps3_library_path_lives_under_games, installing_shadps4_after_the_qt_launcher_…, installing_the_qt_launcher_after_shadps4_…, games_yml_appends_a_new_entry_with_the_dev_hdd0_layout, games_yml_uses_the_games_root_layout_when_given, pico8_directory_settings_defaults, flycast_vmu_rejects_non_vmu_names_and_a_missing_directory.
4: a — 3× merge_platform_override_*, 2× platforms_gate_*, 2× direct_platforms_gate_*; c — clean_env_repoints_a_dead_bus_address_at_the_runtime_dir_bus, clean_env_repairs_the_bus_address_even_with_the_saved_original_present.
5: a — ppsspp_uses_directory_own_mtime_and_ignores_nothing, latest_local_state_mtime_is_zero_for_rpcs3, local_newer_skip_exempts_pcsx2_without_serials, xemu_contributes_no_generic_candidates_and_uses_the_image_mtime; b — manifest_restore_overwrites_existing_file.
a/6: 5× specials::native ranking tests, route_trophy_and_nested_hdd0, salvage_strips_the_pcsx2_appimage_data_root_from_the_destination, saves_dir_honors_absolute_and_relative_paths.
a/7: single_file_zip_install_…, arcade_zip_is_not_extracted_…, multi_file_install_…, a_same_title_platform_row_…, install_update_re_extracts_…, native_install_lays_out_…, native_non_archive_payload_…, ps3_install_routes_into_the_configured_vfs, ps3_iso_only_archive_short_circuits, games_yml_written_for_ps3_with_configured_rpcs3, ps4_install_detects_title_id_and_prefers_eboot, the_game_finalized_hook_sees_the_written_row.
New tests for product bugs without a failing test: `which_on_path` PATHEXT, `safe_relative_path`, RetroArch `-L`, RPCS3 `--installfw`.

## Open questions (not blocking the gate)
1. Expand `~\Games\x` on Windows like `~/Games/x`? (Recommended yes; `autoconfig/paths.rs:70`, `library/paths.rs:137`.)
2. `resolve_best_effort`: canonicalize the existing prefix on Windows (8.3 names, case) like Python `resolve()`? Deferred.
3. Windows coverage follow-up: junction variants of `cfg(unix)` link tests; junction to UNC; `remove_link` on a real symlink.
4. CI: add `cargo test` + clippy to `check-windows` once green (needs approval; `.github/workflows/`).
