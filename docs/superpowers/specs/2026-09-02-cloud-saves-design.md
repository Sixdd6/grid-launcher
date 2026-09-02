# Cloud Saves — Rust Rewrite Design (Milestone 6)

Date: 2026-09-02
Status: draft for user review
Behavior contract: `docs/porting/06-cloud-saves.md` (doc 06)

## Goal

Port the cloud save/state sync engine, its auto-sync triggers, and the desktop
cloud UI to the Rust rewrite, at behavioral parity with the Python app except
for the deviations recorded below. Replace the xemu whole-image sync with a
raw-disk FATX method: extract and inject Xbox save data (`E:/UDATA`,
`E:/TDATA`) directly, so xemu cloud records shrink from the full HDD image to
kilobytes of save files.

## Scope

In scope:

- Core sync engine in `grid-core` (candidate discovery, session window, upload
  planning/execution, retention pruning, restore, conflict detection, save
  scope and block reasons).
- Native-game save sync (manual paths + resolution, combined archives).
- Auto-sync triggers: restore-on-launch, upload-after-exit, the session poll.
- Desktop cloud UI: Details-view cloud panel, record rows, manual
  upload/download/delete, restore confirmations, native path list.
- xemu raw-disk subsystem: clean-room FATX reader/writer over raw HDD
  images, save inject/extract around sessions. No qcow2 support of any
  kind — no decoder, no conversion, no qemu-img (user decision,
  2026-09-02); a qcow2 image is a block reason with migration guidance.
- Opening cleanup carried from milestone 5: deduplicate `apply_section` /
  `SECTION_RE` in `autoconfig/writers.rs`, hoist the Cemu regex recompile,
  and remove the substring fallback duplication flagged in the M5 final
  review.

Out of scope (deferred, not dropped):

- TV-mode cloud variants (doc 06 "TV-mode variants" section) — deferred with
  the TV milestone (doc 09). The four TV-specific open questions in doc 06
  defer with them.
- Per-title-per-game xemu records (each Xbox title syncing to its own game's
  record). This milestone keeps the shared-media record shape; per-title is a
  follow-up that needs XBE title-id ↔ ROM mapping.
- Firmware auto-install (separate committed milestone).

## Authority and porting rules

Doc 06 is the binding behavior contract. Default is follow-the-code: quirks
port as-is and are recorded, not fixed. Only the numbered deviations below
change behavior. When plan text and doc 06 disagree, doc 06 wins. Doc 06
gains a "Rust port deviations (milestone 6)" section when this milestone
merges, mirroring docs 04/05.

## Architecture

### grid-core: `cloud/` module

New module `crates/grid-core/src/cloud/`, pure logic first, IO at the edges:

- `mod.rs` — public API, shared types (`UploadJob`, `SyncStateEntry`,
  `SessionRecord`, `SaveScope`, `BlockReason`, `CloudRecord`).
- `state.rs` — sync-state normalization, keying (`game_key`, `rom_id_key`
  ports live in the existing identity code or move here), persistence shape
  for the `cloud_sync_state` config value.
- `candidates.rs` — save/state/folder candidate discovery: generic file
  candidates, match tokens, folder candidates, per-emulator scanners
  (RetroArch, PPSSPP, RPCS3, Cemu, Flycast VMU, …), ignore sets from
  emulator profiles, always-ignored names.
- `session_window.rs` — the mtime-window algorithm, session partitioning,
  `auto_cloud_upload_plan`, job-level window filtering.
- `jobs.rs` — upload planning: raw-file jobs, grouped-archive jobs,
  directory-archive jobs, native combined archives.
- `archive.rs` — the three archive writers, zip extraction with the 7-Zip
  fallback, temp cleanup.
- `transfer.rs` — upload/download execution against the RomM save/state
  endpoints through the existing authenticated client; URL normalization;
  image sidecar lookup; screenshot selection; `should_skip_known_latest`;
  `is_local_newer_than_server`.
- `restore.rs` — record timestamp parsing, recency sorting, latest-record and
  latest-per-slot selection, restore target selection, payload placement,
  native restore.
- `retention.rs` — pruning (saves only; see deviation D7 for the limit).
- `scope.rs` — `cloud_save_scope_for_game`, `cloud_save_block_reason_for_game`,
  shared-owner resolution, cloud emulator resolution and cache.
- `xemu_sync.rs` — bridges the FATX module into the engine: image resolution
  from `xemu.toml`, image sniffing and block classification, archive build
  from `E:` trees,
  inject-on-restore.

### grid-core: `fatx/` module (clean-room)

New module `crates/grid-core/src/fatx/`. Written from public format
documentation (xboxdevwiki / Free60 FATX pages), NOT from GPL code (pyfatx,
libfatx, the `fatx` crate are all GPL-2.0 and must not be read or linked).

- `layout.rs` — retail Xbox partition table constants (E: at offset
  0xabe80000), 0x1000-byte "FATX" superblock, cluster size derivation,
  FAT16X/FAT32X selection by cluster count. Original-Xbox variant only:
  little-endian throughout (the big-endian XTAF variant is Xbox 360 and out
  of scope).
- `fat.rs` — FAT read/write, cluster chain walking, free-cluster allocation.
- `dir.rs` — 64-byte directory entries: name (max 42 chars), attributes,
  first cluster, size, timestamps; directory traversal; entry
  create/update/delete.
- `image.rs` — `FatxPartition` over a `File` + partition offset: `read_tree`
  (extract a directory subtree to a local path), `write_tree` (inject a local
  tree, overwriting existing files, allocating as needed), `remove_tree`.

Scope limits: read/write within the `E:` partition of a standard retail
layout image. Non-retail layouts, other partitions, defragmentation, and
free-space reclaim beyond honest FAT bookkeeping are out of scope; an image
whose E: superblock does not validate is a block reason, never a write
target. Every write path re-validates the superblock magic and FAT bounds
before touching the file, and IO errors abort without partial FAT updates
where possible (FAT written before directory entry, so a crash orphans
clusters rather than corrupting entries).

Endianness and layout are verified empirically during implementation:
integration tests round-trip through our own builder, and an optional test
oracle runs `pyfatx` as an external process on our generated images (skipped
when pyfatx is not installed). Running a GPL tool as a black-box oracle does
not taint the clean-room implementation; its source stays unread.

### xemu flow

Resolution: read `sys.files.hdd_path` from `xemu.toml` (milestone 5 reader).

Raw images only — no qcow2 handling (user decision, 2026-09-02). GRID ships
no conversion machinery: no qcow2 decoder, no qemu-img dependency. The image
is sniffed once per resolution: a valid E: FATX superblock → sync ready;
the qcow2 magic (`QFI\xfb`, a 4-byte check) → blocked with reason
`xemu-image-not-raw`, whose panel text tells the user to supply a raw HDD
image (and names the `qemu-img convert -O raw` one-liner for anyone
migrating an existing qcow2 by hand). Users who point `hdd_path` at a raw
image get sync with zero setup; the add-only TOML writer never overwrites a
user-set `hdd_path`, so that choice survives autoconfig.

So that a raw setup is first-class in autoconfig too: the required-BIOS
probe and the `hdd_path` default accept `xbox_hdd.img` alongside
`xbox_hdd.qcow2`, preferring `.img` when both exist (deviation D3 — a small
amendment to the milestone 5 xemu module).

Upload (after an xemu session ends, and on manual upload): open the raw
image read-only, extract `E:/UDATA` and `E:/TDATA` into a temp directory,
build one zip archive of both trees (paths archived as `UDATA/...` and
`TDATA/...`), upload as the shared-media save record on the shared owner
game — the same record shape and slot (`shared-media`) as before; only the
archive content changes.

Restore (before an xemu launch, and on manual restore): download the newest
shared-media record; if the archive's top level is `UDATA`/`TDATA`, extract
to a temp directory and inject both trees into `E:` (overwrite semantics);
if the archive holds a legacy whole-image payload (Python-era record), skip
it with a logged incompatibility notice — legacy records are never applied
(deviation D2). Injection failures leave the image as-is where the FATX
write ordering allows and surface an error; they never fall back to image
replacement.

Blocked states for xemu (extending doc 06's block reasons):
`xemu-image-not-raw` (qcow2 or otherwise not raw FATX),
`xemu-image-unsupported-layout` (raw but E: superblock invalid),
`xemu-image-missing` (hdd_path unset or file absent).

### App layer

- Tauri commands in `app/src-tauri/src/commands.rs`: cloud record listing
  (async worker parity with `DetailsCloudRecordsWorker`), manual
  upload/restore/delete, block-reason/scope queries for the panel,
  auto-sync settings passthrough.
- Session poll: the existing session tracking gains the 2500 ms poll timer
  parity behavior; finished sessions trigger the auto-upload path after the
  3 s upload-on-exit delay; launches trigger auto-restore first (doc 06
  "Auto-sync triggers").
- Svelte: cloud panel in `Details.svelte` (records list with size/relative
  time, restore/delete with confirmation, manual upload, native path list),
  block-reason display with the xemu migration guidance, auto-sync toggles
  where the Python settings expose them.

## Deviations (numbered; recorded in doc 06 on merge)

- **D1 — xemu raw-disk sync replaces whole-image sync.** The whole-image
  shared archive path is removed entirely, not kept as a fallback (user
  decision, 2026-09-02). Where the raw-disk path is unavailable, xemu sync
  is blocked with a reason, never falling back to uploading the image.
- **D2 — legacy xemu records are not restored.** Whole-image records from
  the Python app are recognized and skipped with a notice. They still count
  toward retention pruning, so new-format uploads age them out.
- **D3 — autoconfig accepts a raw HDD image.** The milestone 5 xemu
  required-BIOS probe and `hdd_path` default gain `xbox_hdd.img` alongside
  `xbox_hdd.qcow2`, preferring `.img` when both exist. GRID itself handles
  raw images only: no qcow2 decoder, no conversion, no qemu-img dependency
  (user decision, 2026-09-02) — a qcow2 `hdd_path` is the
  `xemu-image-not-raw` block, whose text carries manual migration guidance.
- **D4 — the undefined `_authorized_headers` branch is dropped.** Python
  calls a method that does not exist when a state/screenshot candidate is an
  absolute URL (doc 06 open question 1). The port always treats candidates
  as server-relative; absolute-URL candidates are ignored.
- **D5 — auto-uploads are serialized per game.** Python spawns unbounded
  auto-upload threads with no per-game de-duplication (open question 15).
  The port keys in-flight auto-uploads by game identity: a new trigger for a
  game with an upload in flight is coalesced, and global concurrency is
  bounded by a small worker pool.
- **D6 — shared-slotted restore is staged and atomic.** Python aborts the
  multi-record restore loop mid-way, leaving a partially restored slot set
  (open question 16). The port stages all records to a temp directory and
  commits only when every record downloaded and unpacked cleanly.
- **D7 — the retention limit becomes a config key.** `cloud_save_retention_limit`,
  default 3, minimum 1, no UI yet (open question 2). Saves only; states stay
  unpruned (follow-the-code, see recorded quirks).
- **D8 — unpollable sessions count as finished.** Python silently drops
  sessions whose process lacks a callable `poll` from both lists, so their
  auto-upload never fires (open question 4). The port treats them as
  finished. (The Rust process handle always supports polling, so this is
  mostly a spec-level clarification.)

## Follow-the-code quirks (ported as-is, recorded)

From doc 06's open questions, resolved as "port the code":

- PPSSPP folder scanner sorts by the directory's own mtime and skips the
  ignore sets (question 5).
- RPCS3 directory precedence dominates recency (question 6).
- Block-reason and scope computations use different RetroArch core-flag
  fallbacks (question 7).
- The known-latest short circuit is skipped for shared scopes; shared saves
  re-download on every launch (question 8 — now cheap for xemu given D1).
- The cloud emulator cache key omits the ROM id (question 9).
- Shared-owner detection is a substring match on free text (question 10).
- A job is in-window when any payload path is in-window (question 11).
- States are never pruned (question 3).

TV-specific questions 12–14 (state restore/upload, the read probe) defer
with TV scope.

## Configuration

Existing keys port unchanged (`auto_cloud_save_download`,
`auto_cloud_save_upload`, upload-on-exit delay, `cloud_sync_state`,
manual native save paths). New: `cloud_save_retention_limit` (D7). All live
in the existing config store; no schema migration needed beyond tolerant
defaults.

## Security

Unchanged hard requirement: tokens only in the OS keyring and the redacting
in-memory type; never in files, logs, errors, IPC, or console output. All
server transfer goes through the existing authenticated client;
`expose_secret()` stays confined to its current three allowlisted files;
`scripts/check_secret_hygiene.sh` must pass. Cloud archives contain only
emulator save data — never config files that might embed credentials (the
candidate ignore sets already exclude them; tests assert it for the RA-
credential config files written by milestone 5).

## Testing

- Unit tests per `cloud/` submodule against doc 06's test oracle section
  (byte-for-byte expectations where the doc pins them).
- `fatx/`: a test-only image builder constructs valid FATX partitions in
  temp files (configurable cluster size, FAT16X and FAT32X); round-trip
  read/write tests, overwrite/grow/shrink, deep trees, 42-char names,
  invalid-superblock and truncated-image rejection, orphaned-cluster crash
  ordering. Optional oracle test shells out to `pyfatx` on our generated
  images, skipped when not installed.
- Image sniffing: unit tests for the raw-vs-qcow2-vs-invalid classification
  (magic bytes + superblock validation) driving the block reasons.
- Transfer: wiremock integration tests for the save/state endpoints, upload
  multipart shapes, retention delete calls, and download restore paths.
- E2E: a `cloud-saves` group in the existing harness — mock RomM serves
  save records; auto restore-on-launch and upload-on-exit verified against
  a fake emulator; xemu path exercised with a generated raw image.
- Full suite gate: `cargo fmt --check`, `clippy -D warnings`,
  `cargo test --workspace`, frontend tests, `scripts/e2e.sh`, secret
  hygiene — all local (CI stays manual-only).

## Milestone exit

- Doc 06 updated with the deviations section and rulings.
- `cargo clean --profile dev` from `rewrite/` (keep release).
- Merge menu per finishing-a-development-branch.
