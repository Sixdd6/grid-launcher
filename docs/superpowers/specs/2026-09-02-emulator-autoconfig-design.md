# Emulator Autoconfig (rewrite milestone 5) — design

**Date:** 2026-09-02
**Behavior contract:** `docs/porting/05-emulator-autoconfig.md` (doc 05). Where this
spec is silent, doc 05 wins; where both are silent, the Python source cited by doc 05
wins. Error strings that exist in the reference are ported byte-for-byte.

## Goal

Port both autoconfig layers and the save-path readers to grid-core:

1. **Entry autoconfig** — `auto_configure_emulator_settings`,
   `assign_profile_platform_defaults`, `apply_manual_emulator_profile_defaults`,
   the Dolphin variant naming rules, and the defaults backfill.
2. **Settings sync** — the 11 `ensure_*` writer modules (matching the reference's
   dispatch table): RetroArch (flat cfg), RPCS3 (YAML + GuiSettings + CurrentSettings
   + vfs/games.yml helpers), PCSX2, DuckStation, Dolphin (incl. SkipIPL + GCPad
   block), Azahar, Eden, PPSSPP, Cemu (incl. controller XML), Xemu (TOML), Redream.
   FBNeo, MAME, Pico-8, Vita3K, and Xenia have NO writers — they contribute only
   readers (item 3).
3. **Save-path readers** — every `*_directory_settings` / `*_save_path_overrides` /
   `*_state_path_overrides` reader, plus the Vita3K pref-path/save enumeration and
   Flycast VMU helpers. Ported and unit-tested now; consumed by milestone 6
   (cloud saves). Nothing else calls them yet.
4. **RetroArch core metadata** — embed `retroarch-core-list.json` and
   `romm-platform-cores.json`; port installed-core discovery, platform-key fuzzy
   matching (0.7 threshold), core capability flags, and `retroarch_cores_for_platform`.

## Deviations from the Python reference (all recorded in doc 05 at milestone end)

- **D1 — trigger policy (user decision).** `ensure_*` writers and entry autoconfig run
  ONLY when a NEW emulator entry is created: catalog install (after
  `InstallService::finalize_emulator` writes the entry) or a new manual add. Never on
  edits, never pre-launch, never on view refresh. No session cache (nothing to
  deduplicate). Python's six dispatch call sites collapse to these two.
- **D2 — RA credential fan-out (user decision).** Saving RetroAchievements credentials
  triggers a one-shot write to the RA-capable emulators already registered
  (RetroArch, PCSX2, PPSSPP) that touches ONLY the RA credential keys — a dedicated
  narrow writer per module (`ensure_*_ra_credentials`), not the full managed-key set.
  Runs for every registered entry matching an RA-capable predicate. Clearing
  credentials still writes nothing and scrubs nothing (parity with Python).
- **D3 — defaults backfill trigger.** Python re-runs `_backfill_missing_emulator_defaults`
  on every emulator view refresh. The rewrite runs the same backfill logic at the two
  D1 trigger points only, immediately after entry autoconfig.
- **D4 — PCSX2 raw-path bug fixed.** `ensure_pcsx2_settings` uses the expanded,
  trimmed path throughout (Python computes `emulator_dir` from the raw text and can
  create a literal `~` directory — doc 05 open question; ruled a bug).
- **D5 — PPSSPP unprotected reads guarded.** The two unprotected `read_text` calls
  (doc 05 invariant 3) are wrapped like every other writer: an unreadable INI yields
  `changed=false`, never a propagating error.
- **D6 — PCSX2 `bios_directory` omitted.** The firmware subsystem is deferred
  (milestone 4 deviation 5), so `_resolved_firmware_directories` has no equivalent;
  `[Folders] Bios` is not written. Revisit with the firmware milestone.
- **D7 — RPCS3 background firmware download stays out** (same deferral).
- **D8 — return types unified.** Every `ensure_*` returns one Rust type
  (`EnsureResult { changed: bool, config_path: Option<PathBuf>, extras }`);
  Python's `str`-vs-`Path` mix is a dynamic-typing artifact.

## Open-question rulings (doc 05 "Open questions" — the rest follow the code)

- DuckStation is NOT an RA target; suppression keys only. Follow the code.
- Only RPCS3 YAML and Xemu TOML are add-only; the seven INI writers overwrite with
  pre-probe preservation. Follow the code (the three write policies in doc 05's table
  are the contract).
- DuckStation read-candidates/write-target divergence, Dolphin candidate-index
  divergence, RPCS3 always-portable write target, arcade-biased core fallback,
  no-scrub-on-credential-clear: follow the code, byte-for-byte.
- Xemu `changed` accumulator: single accumulator in Rust (same observable behavior).
- RetroArch username rebinding: two distinct variables (`romm_username`,
  `ra_username`), identical output.
- The `name::path` session cache question is mooted by D1.

## Architecture

New grid-core module `crates/grid-core/src/autoconfig/`:

```
autoconfig/mod.rs        pub api + orchestration entry points
autoconfig/writers.rs    shared section writers: INI overwrite family, Qt-annotated
                         INI, RPCS3 CurrentSettings strict format, YAML add-only,
                         TOML add-only, flat-cfg (RetroArch), append-if-absent block
autoconfig/entry.rs      layer 1: entry autoconfig, platform/core defaults, manual
                         defaults, Dolphin variants, backfill
autoconfig/cores.rs      embedded core-list + slug map, installed-core discovery,
                         fuzzy platform match, capability flags
autoconfig/readers.rs    *_directory_settings / override readers, Vita3K, Flycast VMU
autoconfig/<emulator>.rs one file per writer emulator (retroarch, rpcs3, pcsx2,
                         duckstation, dolphin, azahar, eden, ppsspp, cemu, xemu,
                         redream); reader-only emulators (fbneo, mame, pico8,
                         vita3k, xenia) live in readers.rs
```

- The three write policies live once in `writers.rs`; per-emulator modules declare
  sections/keys/probes. Python's near-duplicate per-module helpers collapse into the
  shared writers — behavior pinned by the doc 05 policy table, not by duplication.
- Emulator identification ports `_emulator_matches_tokens`: profile token match, then
  case-folded substring fallback; RPCS3's extra name check ORed in.
- Orchestration lives in grid-core (`autoconfig::sync_new_emulator(config, entry)`),
  called from the two D1 sites; failures are non-fatal and surface as the existing
  warning event kind. grid-core never imports Tauri.

## Config, credentials, IPC

- `Config` gains `default_cores: BTreeMap<String, String>` (platform → core id) and
  `retroachievements_username: String` (plain, non-secret).
- The RA **token** is a credential: OS keyring via the existing secret store, held in
  the redacting secret type, never in grid config, logs, errors, or IPC responses.
  It IS written into emulator config files/token files — that is the feature — and
  those writes are the only allowed disk destinations.
- IPC: `set_retroachievements_credentials(username, token)` (stores, then runs the D2
  fan-out; returns per-emulator changed flags), `get_retroachievements_status()`
  (username + token-present boolean only — never the token),
  `clear_retroachievements_credentials()`. Frontend: two fields + status line in the
  Emulators panel settings area, following existing form patterns.
- Manual-add flow calls `apply_manual_emulator_profile_defaults` parity rules (fills
  blank fields only, never `path`), then D1 sync for the new entry.

## Testing

- TDD throughout. Unit tests per writer family and per emulator module in tempdirs:
  first-run write, second-run `changed=false` idempotency, overwrite-vs-add-only
  policy, format strictness (RPCS3 CurrentSettings `key=value` no-spaces; Qt
  annotation line pairs; Azahar widened key regex), preserve-if-present probes,
  RA gating on both-fields-present, D2 narrow writer touching only RA keys.
- Layer 1 table tests mirroring doc 05 rules (args replacement conditions, native
  outranks RetroArch, core-default assignment, Dolphin variants, manual fill rules).
- Reader tests ported from `tests/test_emulator_profiles.py`, `test_vita3k.py`,
  `test_flycast_vmu.py`, `test_retroarch_config.py` oracles.
- Integration: catalog install through `InstallService` produces the native config
  files for a wired test profile; manual add ditto; RA save fan-out writes RA keys to
  a pre-existing config without touching a sentinel unmanaged key.
- E2E: extend the `emulator-catalog` group — after the PCSX2 install, assert
  `portable.ini` and the managed INI keys exist under the install dir.
- Full doc 05 test-oracle parity is the goal; Python test counts are the checklist.

## Out of scope

- Server-backed settings backup/restore (user raising per-user blob storage upstream
  with RomM; revisit when an endpoint exists).
- Firmware download/install (D6/D7) — **deferred to a dedicated milestone, not
  dropped (user commitment, 2026-09-02).** That milestone owns: RomM
  `/api/firmware` download, `install_platform_firmware` parity, firmware directory
  resolution, the RPCS3 PUP background flow, and closing D6 (PCSX2
  `[Folders] Bios`) and D7. It must be scheduled before the rewrite is declared
  feature-complete.
- Cloud-save consumption of the readers (milestone 6).
- Any launch-time or refresh-time sync (D1).
