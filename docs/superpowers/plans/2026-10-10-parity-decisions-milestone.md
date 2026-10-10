# Milestone plan — parity decisions Q2–Q11

Base: 88a238d. Decisions: `.claude/PARITY_ANALYSIS.md` → "User decisions (2026-10-10)" (final; Q4 approved).
File:line references are provisional: **every batch starts with an evidence step** (Explore pass that pins file:line and openapi names) before code.

Rules for every batch: one implementer at a time; tests first; full gate (CLAUDE.md) on Windows and Linux (WSL); scoped e2e group when a user-visible flow changes; `git commit --only <paths>`; grid-core stays Tauri-free; secrets only via `secrets.rs` types; server writes in API checks only as the `tester` account, cleaned up after. Builds throttled (`CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4`). Each batch lands its own SPEC.md edit (B11).

## Order
B1 Q6 re-auth (S) → B2 Q11 search dropdown (S) → B3 Q9 retention (M) → B4 Q10 play sessions/last played (M) → B5 Q4 id-less rows (M) → B6 Q5 achievements (M/L) → B7 Q3 server emulator packages (M) → B8 Q7 xemu raw-HDD guidance (S/M) → B9 Q2 library path (L, 3 phases) → B10 cheap superiority (later; confirm each with the user).

## B1 — Q6: a rejected token re-opens Connect, pre-filled (S)
Goal: a 401 at restore or mid-session takes the user to the Connect screen with server URL and username pre-filled and the token field empty; library and config untouched.
1. Evidence: `session.rs` RestoreOutcome; `romm/error.rs` status classification; session-phase commands/events (`shell.ts`, `stores/session.svelte.ts`); Connect component path; e2e mock 401 ability; openapi security (401 vs 403).
2. grid-core: Unauthorized classification for 401 (and 403 if the server uses it for a bad token); `RestoreOutcome::Unauthorized` (no secret); a mid-session 401 sets the session state once, not per request.
3. src-tauri: map to the session DTO + existing session-changed event; carry URL + username from config, never the token. Keyring credential kept until a new one is saved (Q-1 proposal).
4. Frontend: phase "unauthorized" → Connect; prefill; message "Your token was rejected — enter a new one"; focus the token field.
5. designer: banner copy and state.
Tests first: error.rs 401 → Unauthorized, 500 → not; session restore with 401 stub; shell.test.ts mapping; vitest prefill builder; e2e mock "reject token" switch + `connect-reauth` group.
API check (read): `/api/users/me` with a bogus token → 401; tester token → 200.
Risks: offline must stay "Not connected + Retry"; errors never include the Authorization header.

## B2 — Q11: server search by Title / Platform / Genre (S)
Client-side (SPEC.md:39-41 "filters the loaded platform list"; Python filtered in memory). Global server search stays in B10.
1. Evidence: Server filter module (`app/src/lib/server/`); whether the list DTO carries genres and platform name; Python matching rules, labels, default.
2. coder: add genres to SimpleRom deserialization + api.ts type if missing; pure `filterServerGames(list, query, field)`.
3. designer: dropdown (Title default, Platform, Genre) beside the search box; gamepad focus; keep Ctrl+F.
Tests: vitest per field, case-insensitive, empty query, null metadata; Rust deserialization if a field is added.
API check (read): `/api/roms?platform_ids=<id>` carries genres — if not, stop and re-plan (detail fetch or server `genres` param).
Check: Platform mode only matters across platforms — confirm against the Python UI; ask the user only if Python differs.

## B3 — Q9: retention (M)
SPEC.md:70-71 already lists a retention limit under Settings › Cloud saves. Saves: `autocleanup=true&autocleanup_limit=<n>` on POST /api/saves. States: after a successful state upload keep the newest N per scope/slot (SPEC.md:172), delete the rest via the state-delete endpoint; best-effort.
1. Evidence: what `cloud/retention.rs` does today; config key; Settings control; Python default; shared-media scopes (xemu, Redream VMU — SPEC.md:175-176) rule.
2. grid-core: upload builder params when limit > 0; pure `states_to_prune(records, limit)`.
3. src-tauri: prune after auto/manual state upload.
4. designer/coder: "Keep saves" / "Keep states" (0 = unlimited).
Tests: builder params; prune table (0, per-slot, equal timestamps); config round-trip; e2e cloud-saves (mock records autocleanup; third state upload with limit 2 deletes the oldest).
API check (WRITE as tester): saves ×3 with limit 2 → 2 remain; states ×3 + prune → 2 remain; clean up.

## B4 — Q10: push play sessions and last played (M)
1. Evidence: `launch/sessions.rs` tracking; session-finished hook; openapi `/api/play-sessions` body; `PUT /api/roms/{id}/props` body/encoding; device id.
2. grid-core: `create_play_session`, `update_rom_props(last_played)` per openapi; SQLite outbox in the registry DB (offline queue, idempotency marker); pure session → payload.
3. src-tauri: enqueue on session end; flush on hook, connect, restore. Rows without rom_id skipped.
Tests: payload (durations, UTC); outbox enqueue/flush/dedupe; sessions under a threshold (propose ≥ 30 s) not pushed; e2e launch group (mock records both calls).
API check (WRITE as tester): create + read back a play session (delete if allowed, else note residue); set and reset last_played.
Update ARCHITECTURE.md (new service).

## B5 — Q4: id-less rows (approved) (M)
1. Evidence: registry fields; creators (`import_python.rs` / python import); Details "no server id"; platform ROM listing.
2. grid-core: pure `match_unlinked(rows, server_roms)` on normalized title + platform; link only on exactly one match; `set_rom_id` refreshing derived fields.
3. src-tauri: run after connect/restore once platform lists load; emit installed-changed; commands `link_installed_to_rom`, `remove_from_library` (registry row only — never uninstall_steps).
4. designer: "Not linked to the server" chip; "Link to server game" picker (reuse B2 filter); "Remove from library (keeps files)" confirm naming the folder that stays.
Tests: matcher table; set_rom_id; remove keeps files (temp dir); e2e python-import: imported row auto-links on connect.
API check (read): platform ROM names.

## B6 — Q5: achievements in Game Details (M/L)
Source (to confirm in step 1): RomM data — `merged_ra_metadata` on the ROM detail, `ra_progression` on the user. No RA Web API key; the Settings key/token stays for emulator configs. Fallback if progression is empty: list without unlock state + hint to set the RA username in RomM.
1. Evidence + api-tester (read): exact schema names/endpoints; refresh endpoint; live data for a ROM with an ra_id.
2. grid-core: typed RA structs (all Option); pure join → rows {title, description, points, badge url, unlocked_at, hardcore}.
3. src-tauri: `achievements_for_rom(rom_id)`; badges via the image cache (`images/urls.rs`).
4. designer: Achievements tab (progress header, unlocked first, locked dimmed); shown only when RA data exists.
Tests: openapi-shaped fixture (no tokens); join/sort; vitest tab visibility; e2e library group with a fixture ROM carrying RA data.

## B7 — Q3: server "Emulators" packages auto-register (M)
1. Evidence: Python `_install_game` → `_auto_configure_installed_emulator` (match rule, exe detection); where Rust hides the platform; catalog registration + autoconfig + default-assignment path.
2. grid-core: pure `profile_for_package(...)` against `emulator-autoprofiles.json`, platform-appropriate exe; on finalize for the `emulators` platform extract under `emulators/<Profile>/` (layout v2) and run the catalog register + autoconfig + user_data_links path; unknown package → today's behavior + toast "not a known emulator, add it under Emulators › Manual".
3. src-tauri: finalize branch; Downloads badge "Emulator".
4. Uninstall: keep the emulator entry (as catalog installs) — confirm against Python in step 1.
Tests: matcher table (names, Windows vs Linux exe); finalize → config entry + links (temp dir); e2e: install an Emulators package from the mock → appears in Emulators › Installed. Autoconfig must not overwrite user settings (emulator-autoconfig skill).
API check (read): emulators platform slug; its package list.

## B8 — Q7: xemu raw-HDD guidance (S/M)
1. doc-research: xemu.toml location per OS; HDD key (expected `[sys.files] hdd_path`); accepted formats; `qemu-img convert -f qcow2 -O raw`; Windows availability; UI path (Machine › System › Hard Disk; restart).
2. Evidence: how `cloud/xemu_sync.rs` detects qcow2 today.
3. coder: structured `XemuHddNotRaw { hdd_path }` to the frontend.
4. designer: guided sheet — close xemu; convert (with Windows download link); point xemu at the .img; "Check again".
Tests: qcow2-magic fixture → NotRaw; raw → OK; vitest guidance builder per OS.

## B9 — Q2: library path editable + move/fresh + moved-file detection (L)
"Detect" = on startup and each library refresh, `reconcile_library(root)`: recorded paths exist → OK; missing but present at the same relative path under the current root or the v2 location → re-point; else mark `missing` (new registry state) with "Locate…" and "Remove from library". No disk-wide scan.
- 9a reconcile + missing state (M): `library/reconcile.rs` (pure plan → apply), registry column + migration, run after layout migration and on refresh, UI badge + Locate.
- 9b change path, start fresh (S): Settings › Library pane; `config_write.rs`; existing rows keep absolute paths; new installs go to the new root.
- 9c change path, move existing files (L): `library/relocate.rs` — move games/, emulators/, saves/ (rename, else copy+verify+delete; reuse the layout_migration helper); resumable via a config marker; rewrite registry paths and emulator exe paths; re-create user_data_links (Windows junctions are absolute — must be re-created); re-run autoconfig for configs holding absolute library paths. Refuse when nested old/new roots, conflicting non-empty target, unwritable, or while an install, a running game or a sync is active. Asserts layout v2 first. RemovalGuard must read the root at call time. Native rows outside the root are not moved.
Tests: plan tables; cross-device fallback; junction re-creation; resumable interrupt; e2e `library-relocate` group.
Risks: data loss (never delete the source on a partial copy); long moves (cancel + resume); stale absolute paths in emulator configs.

## B10 — cheap superiority (later; confirm each with the user)
Close guard (S); emulator update badges at startup (S/M); "Update all" in the Updates rail (S); Steam's own Proton (S, Linux); Details "Open game folder / saves folder / Open in RomM" (S); global server search + server-side sort/filters + `updated_after` (M).

## B11 — SPEC.md edits (land with each batch)
Q6 First Run Setup (re-auth); Q11 Server search selector; Q9 retention wording + Cloud list bullet; Q10 play activity; Q4 Details unlinked actions + auto re-link; Q5 Achievements tab + RA pane purpose; Q3 Emulator Configuration; Q7 xemu raw .img; Q2 Settings Library pane + missing-files detection. Q1 done (88a238d).

## Open questions (proposals used until the user rules)
1. Q6: delete the rejected credential from the keyring, or keep until a new one is saved? Proposal: keep.
2. Q9: default limits? Proposal: saves 10, states 5.
3. Q10: toggle or always on? Proposal: always on.
4. Q7: offer a "set xemu to this .img" button, or text only?
5. Q2: "move existing files" as a blocking modal or a background job? Proposal: modal.

---

## Evidence update (planner second pass, 2026-10-10) — supersedes the provisional anchors above

User answers: keep the rejected credential until replaced; retention defaults saves 10 / states 5; play activity always on (no toggle); xemu "Use converted image" button wanted. xemu raw `.img` support: user already uses it (treat as confirmed; no 8a manual test).

- **Q6:** 401 and 403 both map to `RommError::Unauthorized` (`romm/mod.rs:143-145, 194-196`, `romm/cloud.rs:225, 274`); `session.rs:172-176` folds it into `RestoreOutcome::Unreachable`, so Retry resends the dead token forever. No command emits a connection event; auth mode is not stored; `Connect.svelte:13` hard-codes token mode. Plan: split `Unauthorized` (401) / `Forbidden` (403, no re-auth); `RestoreOutcome::Unauthorized { server_url, username, auth_kind }` (never the secret); a Tauri-free unauthorized hook on `SessionManager` → `session-unauthorized` event; `retry_connect` returns a typed outcome; fix `settings/connection.ts:13-17`. e2e: mock `/__e2e__/revoke` + spec in `connect-restore`. API check: bogus bearer → 401; tester → 200; `/api/users` as tester → 403.
- **Q11:** `SimpleRomSchema` carries `metadatum.genres` and `platform_display_name`; `RawGameSummary` drops them into `extra` → add `genres`, `platform_display_name` (copy the `RomDetail` mapping, `mod.rs:672-701`). Pure `app/src/lib/server/search.ts`. **Python had no dropdown** (`server/view.py:107-128` matched all three at once) → see question U2.
- **Q9:** `POST /api/saves` has `autocleanup` (bool) + `autocleanup_limit` (int); `POST /api/states` has neither. Existing `cloud_save_retention_limit` (default 3) is enforced client-side (`cloud/retention.rs:70`, UI `CloudSavesPage.svelte:90-98`). New defaults per user: saves 10, states 5 (`cloud_state_retention_limit`); existing stored values are kept. State prune via `POST /api/states/delete`. Live check first to learn how autocleanup groups (rom+emulator? slot?); keep the client save prune as a post-pass unless grouping matches `slot_dedupe_key`.
- **Q10:** `PUT /api/roms/{id}/props` has no `last_played` field: `?update_last_played=true` + `{}`, server stamps now (push at spawn when online, not queued). `POST /api/play-sessions` is a batch call answering `duplicate` for repeats (safe retries). `GameSession` (`launch/sessions.rs:17-25`) has only `started_at`; the session-finished hook is a single slot (`launch/mod.rs:167`) already used by cloud_service → make it multi-listener. Outbox table `pending_play_sessions` (registry user_version 6 → 7). Skip rows with no rom_id and the Emulators platform. Check live whether a play-session POST alone updates last_played.
- **Q4:** `registry.rs` `set_rom_id_by_key` (refuse an id another row holds), `remove_row_by_key` (registry only); pure `library/relink.rs` (unique casefolded title + platform alias match); relink pass after Connected for platforms with id-less rows; Details `:566-567` replaces the "no server id" note.
- **Q5:** no RA Web API key in Rust (`import_python.rs:1051` drops Python's on purpose); the existing RA username + token fill RetroArch/PCSX2/PPSSPP. RomM: `DetailedRomSchema.ra_id` + `merged_ra_metadata.achievements[]`; `UserSchema.ra_progression.results[]` (earned ids as strings, `date`, `date_hardcore`). Progress needs an RA username on the RomM account → question U6. Achievements tab shown only when `ra_id` is set.
- **Q3:** reuse `emu_install.rs:336 select_executable`, `profiles.rs:446 profile_for_entry`, `library/mod.rs:2479 write_emulator_entry`, `autoconfig/mod.rs:571 sync_new_emulator`, `emulator_installed_hook`. Port Python's tie-break (title-equal profile wins) and no-match fallback (name = ROM title, args `%rom%`). Deleting such an emulator also uninstalls its hidden server row (Python `install_registry.py:65-90`). Package stays under `games/Emulators/<stem>`.
- **Q7:** structured `NotRaw` (current path, suggested `.img`, qemu-img command); "Open xemu folder"; "Check again"; "Use converted image" sets `sys.files.hdd_path` only on explicit click, only when the .img exists and xemu is not running — the one deliberate exception to the add-only rule. Fresh GRID-installed xemu writes `hdd_path = xbox_hdd.qcow2` (`autoconfig/xemu.rs:106-113`) — revisit.
- **Q2:** registry paths are absolute; `Registry::rewrite_paths` (`registry.rs:714`), `rewrite_paths_in_text`, `rewritten_config` exist. Windows junctions are absolute (break on move); unix links relative. Add-only writers never repair old paths: RPCS3 `vfs.yml`/`games.yml`, PCSX2 `[Folders] Bios` (the `lib.rs:161-164` comment is wrong about PCSX2) → rewrite those texts directly. Migration moves are rename-only (`layout_migration.rs:1111-1127`); `move_tree_preferring_dest` (`user_data_links.rs:210`) has the copy fallback. RemovalGuard knows only the current root → add `former_library_paths` and protect them. Phases: 9 Settings pane + start fresh; 10 move (journaled, resumable, preconditions); 11 heal/detect (runtime `missing`, unmounted-root banner).

## User answers, round 2 (2026-10-10)
- **Q11 / U2:** no dropdown. One search box matches title, genre OR platform at once, exactly like Python `server/view.py:107-128`. (B2 drops the dropdown; still add `genres` + `platform_display_name` to GameSummary.)
- **Q2 / U1:** "Start fresh" drops the old installs from the library for now (interpreted: registry rows removed, files left on disk untouched — confirm before B9 if deletion was meant). Future milestone: multi-library support (install to any of several library folders).
- **Q3 / U4:** a server package whose name matches an existing emulator entry is added as "<Name> (server)"; the catalog entry keeps its update source.
- **Q5 / U6:** saving the RA username in GRID Settings also sets `ra_username` on the RomM account (username only, never the token).
- **U7 (device registration):** not asked; default = not now.
- **B2 detail (late research report):** Server filter today is `Server.svelte:89` → `library/sort.ts:61-65 titleContains`. Python genres came from `details_metadata_from_item` (`server/metadata.py:8-13, 23-26, 239-259`): `genres`/`genre` keys of `launchbox_metadata`, `ss_metadata`, `igdb_metadata`, `moby_metadata` in that order, merged case-insensitively without duplicates — not `metadatum`. `SimpleRomSchema` has both `metadatum.genres` and the provider blocks; prefer `metadatum.genres` (RomM's merged view) and fall back to the provider order if empty — verify live with api-tester. Platform text per card: `platform_display_name`. Match = casefolded substring OR across title, platform, genres; empty query → all. SPEC.md:40 needs a line.

## B3 live probe (tester account, 2026-10-10; no leftovers)
- `POST /api/saves?...&autocleanup=true&autocleanup_limit=N` (multipart `saveFile`) groups by **rom + slot**; emulator is NOT in the key; **null-slot saves are never cleaned up** (openapi `/api/sync/negotiate` calls them archival manual uploads). Oldest by created_at (= lowest id) is removed. A slotted upload is renamed `<name> [YYYY-MM-DD_HH-MM-SS].<ext>`.
- `POST /api/states` ignores autocleanup params (200). No `DELETE /api/states/{id}` or `/api/saves/{id}`; use `POST /api/states/delete {"states":[ids]}` / `POST /api/saves/delete {"saves":[ids]}`.
- Bulk delete is NOT atomic: it stops at the first missing id with 404, earlier ids stay deleted, later ids are kept. → delete one id per call (or treat 404 per id as success and continue).
- Consequence: server autocleanup only helps for slotted uploads. Keep the client-side prune for whatever GRID uploads without a slot; send autocleanup when a slot is set. Confirm in code which uploads carry a slot (`slot_dedupe_key`).

## B4 live probe (tester account, 2026-10-10)
- `POST /api/play-sessions` JSON `{"sessions":[{"rom_id":N,"start_time":"<RFC3339 Z>","end_time":"...","duration_ms":int}]}` (device_id optional, top-level and per entry) → 201 `{results:[{index,status:"created"|"duplicate"|"error",id}],created_count,skipped_count}`. An exact repeat → `duplicate` (safe retry).
- The ingest ALSO sets the user's `rom_user.last_played` to the session end_time.
- `PUT /api/roms/{id}/props?update_last_played=true` sets last_played to now BUT also sets `now_playing=true` and `status="incomplete"` (would clobber a user's status) → **do not call props**; last played reaches RomM through the session ingest.
- `DELETE /api/play-sessions/{id}` → 204. GET filters: rom_id, device_id, start_after, end_before, limit, offset. Returned timestamps lack an offset (treat as UTC).
