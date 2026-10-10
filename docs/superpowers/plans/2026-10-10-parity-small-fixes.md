# Plan: four small parity fixes (G1, G9, G11, "x ago")

Source: `.claude/PARITY_ANALYSIS.md` gap IDs. All four claims confirmed in code. Order by data risk: G1 → G9 → G11 → x-ago. Disjoint files; one implementer, gate, commit `--only` per item. Builds throttled (`CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4`).

## 1. G1 — RPCS3/Cemu cloud saves pick up other games' save folders

**Evidence.**
- `app/src-tauri/src/cloud_service.rs:1344-1346`, `:1361-1363` blank `title_id`, `base_title_id`, `ps3_game_id`, though `InstalledGame.ps3_game_id` exists (`registry.rs:300`), is filled at PS3 install (`library/mod.rs:2156`) and the frontend sends it (`app/src/lib/api.ts:238`); `CloudGameInput` (`:1306-1326`) has no field for it.
- RPCS3: `cloud/ops/mod.rs:837` → `rpcs3_save_directories(dirs, &ps3_id_tokens(game))`; `candidates.rs:774` keeps every child when `ids` is empty; `ops/upload.rs:255` → `transfer.rs:546-569` uploads **every** folder as its own job under this game's rom_id. One upload pushes every PS3 game's savedata to this ROM. Not Python parity: Python's `_ps3_game_ids_for_game` was undefined (`cloud_mixin.py:1178`) and raised.
- Cemu: `ops/mod.rs:803` passes title-text tokens; Python used `_cemu_title_id_tokens(game)` (`cloud_mixin.py:1268-1332`: hex ids from `title_id`, `base_title_id`, `rom_id`, `rom_file_name`, `extracted_path`, `archive_path`, `extracted_dir`, `native_executable_path`, plus `app.xml`/`meta.xml` next to extracted paths and under `<extracted_dir>/code`, `/meta`). `candidates.rs:667-671` falls back to **every** Cemu title folder when nothing matches.
- `title_id`/`base_title_id` have no source in either codebase (not in `openapi.json`); for Wii U the real source is the XML scan.
- `latest_local_save_mtime` (`ops/mod.rs:908-926`) maxes over the same targets → another game's recent save makes this one "local newer" and skips its download (`ops/restore.rs:216-218`).

**Rule (settled by SPEC.md:166, :169 — archive "the relevant subfolder" / "files relevant to the game"):** unknown id → no match → existing "nothing to upload" message.

**Steps.**
1. `cloud/candidates.rs`: `rpcs3_save_directories` returns empty for empty `ids`; `cemu_save_directories` returns empty for empty tokens and only matched folders (drop the fallback). Update doc comments.
2. `cloud/install_match.rs`: `extracted_file_candidates` → `pub(crate)` (no other change).
3. `cloud/tokens.rs`: game-level Cemu collector porting `_cemu_title_id_tokens` (hex16 + hex8-pair variants from the listed fields — `CloudGame` lacks `native_executable_path`, note it; plus `app.xml`/`meta.xml` text from the sibling and `extracted_dir/{code,meta}` locations; `~` expanded; case-insensitive dedupe; existence checks; title text contributes nothing). Run the set through the existing `cemu_title_id_tokens`.
4. `cloud/ops/mod.rs:803`: feed the collector to `cemu_save_directories`.
5. `cloud/mod.rs:55-74`: rewrite the "data-availability gap" doc.
6. `app/src-tauri/src/cloud_service.rs`: `#[serde(default)] ps3_game_id` on `CloudGameInput`; copy it in both builders; fix the doc at `:1328-1332`.

**Tests first.** `candidates.rs`: `rpcs3_empty_ids_match_nothing`, `cemu_empty_tokens_match_nothing`, `cemu_unmatched_tokens_do_not_fall_back_to_every_title`; update `rpcs3_directory_index_outranks_recency` (pass `["BLUS30443"]`), `cemu_uses_user_root_itself_when_childless`, `cemu_drops_candidates_with_zero_latest_mtime` (pass a matching token). `tokens.rs`: collector reads `<extracted_dir>/meta/meta.xml` → 16-char id + halves; hex id in `[0005000010145D00].wua`; title-only game → empty. `ops/tests.rs`: two RPCS3 savedata folders — a game with `ps3_game_id` gets only its own; blank id gets none. `cloud_service.rs`: `cloud_game_from_installed` copies `ps3_game_id`; `CloudGameInput` from InstalledGame-shaped JSON carries it.

**Risks.** PS3 rows with no id and Wii U `.wua`/`.wux` with no XML can no longer upload (today they upload wrong data). `latest_local_save_mtime` → 0 for them, so auto-download runs (like the PCSX2-without-serials exemption). Records already uploaded by released builds may hold other games' folders. Adjacent, out of scope: PPSSPP and PCSX2 scanners also match everything on empty ids.

**G1-Q (ask user; does not block steps 1–6):** for an RPCS3/Cemu game with an unknown id, should the before-launch auto-download still run? Options: (a) as today; (b) skip, manual restore only. Until answered: (a), no change.

## 2. G9 — uninstall candidate paths miss `native_game_dir` and `~`

**Evidence.** `library/paths.rs:218-232` (`candidate_archives`) has no `<native_game_dir>/<archive_name>`; `:237-249` (`candidate_extracted_dirs`) has no `expand_home` and no native siblings. Python: `install_paths.py:19-43`, `:68-89`; full Rust set in `cloud/install_match.rs:82-152`. Python used the same helpers for uninstall, launch and native settings → changing the shared helpers is parity for all callers.

**No safety guard today.** `uninstall_steps` (`library/mod.rs:2859-2927`) only checks shape. Latent hazard: `archive_name` (`paths.rs:69-78`) returns the last segment of `rom_file_name` unchecked; an `fs_name` ending in `..` yields `library.join("..")`, `extraction_dir` → `parent.join("")` = platform dir or library root, which passes `is_dir`. A blanket "inside the library" rule is wrong (PS3 dirs live in `dev_hdd0`; native rows anywhere).

**Change.** Archive list adds `<expanded native_game_dir>/<name>` last; extracted list expands `~` and adds `<native_game_dir>/<extraction dir name>` after each archive's dir (Python order). Uninstall guard: skip archive-derived candidates when `archive_name` is not one plain component (`""`, `.`, `..`, or contains a separator); refuse (skip silently) any removal whose normalized path is the FS root, home, the library root or an ancestor, `<library>/games`, a platform dir (current or legacy), or an ancestor of a recorded `native_game_dir`.

**Steps.** `library/paths.rs` (new `native_game_dir: &str` param on both; `expand_home`; pure plain-component predicate); `library/mod.rs` `uninstall_steps` (pass it; pure guard; keep the native/PS3/multi-file early returns); callers `launch/rom.rs:27,35`, `launch/native.rs:66`, `app/src-tauri/src/commands/specials.rs:140`.

**Tests first.** `paths.rs`: native archive entry last, skipped when blank; extracted dir expands `~` (pattern: `candidate_archives_expands_leading_tilde`); native sibling per archive; dedupe holds; predicate table (`Game.zip` ok; `""`, `.`, `..` rejected). `mod.rs`: `rom_file_name = ".."` → no step at library root or platform dir; `extracted_dir` = library root or `~` → no step; existing native and PS3 uninstall tests unchanged.

## 3. G11 — Linux platforms still get a default emulator

**Evidence.** `autoconfig/entry.rs:170-179` drops only `windows*` and `emulators`. Ruling: `library/platforms.rs:10-13` ("User ruling 2026-09-08: linux platforms are native everywhere…"), mirrored in `app/src/lib/details/cloud.ts:105-111`; the frontend already hides the selector (`emulators/defaults.ts:27-28`). Only the backend backfill (`commands.rs:227`, `library/mod.rs:2450`) still writes Linux defaults.

**Change.** `assignable_platforms` also drops `library::platforms::is_native_platform` platforms; `emulators` stays excluded; original spelling kept. Doc cites the ruling.

**Tests first.** Extend `assignable_platforms_drops_windows_prefixed_and_emulators` (`:746`) with `"Linux"`, `"  linux games "`; one assign-defaults case: an `all_platforms` profile writes no Linux default (pattern `:1155`).

**Risk.** Existing configs with a Linux default keep it (cleanup out of scope — note to user).

## 4. "x ago" shows hours for minute-old saves

**Evidence.** `cloud/restore.rs:351` `RANGES = [(86_400, 3_600, "hour"), (3_600, 60, "minute")]` — the hour bucket catches 90 s..1 day, so 120 s → "1 hour ago". Doc `:324-333` says "do not fix" (port-time parity policy, deleted `docs/porting/06`); test `relative_timestamp_text_120_seconds_renders_1_hour_ago_bug` (`:924`) pins it. SPEC.md:141 requires "x hours/minutes ago". User asked for parity or better (2026-10-09) → fix, and report it.

**Change.** Minutes (< 3600 s) before hours (< 86400 s); everything else unchanged.

**Test first.** Table `relative_timestamp_text_buckets`: 0 → Unknown; future → just now; 29 → just now; 30, 89, 90, 119 → 1 minute ago; 120 → 2 minutes ago; 3599 → 59 minutes ago; 3600, 7199 → 1 hour ago; 7200 → 2 hours ago; 86399 → 23 hours ago; 86400 → 1 day ago; 604799 → 6 days ago; 604800 → 1 week ago; 14 d → 2 weeks ago. Fold in the existing range/week tests.
