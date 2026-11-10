# Game Backups: implementation plan

> **For agentic workers:** Work one task at a time, in dependency order. Each task lists the files it may touch, what to build, its tests and what it depends on. Run the gate after every task. Commit with `git commit --only <paths>` and the Co-Authored-By trailer. Never run checkout, restore, reset or stash on tracked files.

**Goal.** Back up server games to a removable drive's `gridpack/` folder, together with their art, metadata, emulators for every OS the pack is for, and firmware. Install those games again from the drive with no network. Add "Use without a server".

**Spec (binding).** `docs/design/system/components/GameBackups/README.md`. The prototype `docs/design/prototype/backups.js` shows UI behaviour only.

**Branch.** `feat/game-backups`. The design (prototype source, the GameBackups card and its README) is committed separately, before Task 0. Leave `attachments/`, `.claude/` and `docs/rendering-engine-options.md` out of every commit.

**Architecture in one paragraph.** A new grid-core module `backup` owns four things: the gridpack format (`pack.rs`), removable-drive detection (`drives.rs` plus per-OS files), the write engine (`write.rs`: temporary name, then rename) and backup planning (`plan.rs`). Backups run inside the existing `InstallService` queue as a third job kind, `JobPayload::Backup`. The download slot downloads from the server into a local staging folder. The finalize slot writes to the drive. So "Downloading from server" and then "Writing to <label>" map onto the existing `Downloading` and `Installing` statuses. Install-from-drive reuses `plan_install` and the whole finalize path unchanged. The only change is where the game's bytes come from: a new `GameSource::Drive` copies files from the drive instead of calling `download_targets`. On the app side, a new `drive_service.rs` polls for drives every 2 s, keeps the list of remembered drives in config, and on removal calls `InstallService::abort_drive`. New commands go in `commands/backups.rs`. The frontend gets a `lib/backups/` feature folder, `stores/drives.svelte.ts` and a Settings pane.

## Global constraints

- **Dependency direction.** grid-core does not know Tauri. All OS drive probing lives in grid-core (`backup::drives`). The app layer owns only when it runs (the poll loop and events).
- **Secrets.** `gridpack.json` stores `source.server` only after `romm::strip_userinfo`. No header, token or URL with credentials ever goes into the pack, a log line or an IPC payload. A drive path in a log is fine; a server URL with userinfo is not.
- **Trust boundary.** Treat a drive's `gridpack.json` as untrusted input:
  - Every relative path must be validated: relative, only `Normal` components, no `..`, no drive prefix, at most 255 bytes per component.
  - It must resolve under `<mount>/gridpack/`.
  - The JSON may be at most 64 MiB.
  - IPC commands take a drive id or key, never a raw path to write to. The service resolves the id to a drive found by the current scan.
- **GRID never touches anything outside `<mount>/gridpack/`.** The only delete of the whole folder ("Remove GRID files…") checks that the path is exactly `<mount>/gridpack`, that it holds `gridpack.json`, and that it is not a symlink.
- **TypeScript types in `app/src/lib/api.ts`** mirror the serde output exactly.
- **Timestamps.** RFC 3339 UTC strings (`chrono`, already a dependency).

---

## Part 1: Shared contracts (Task 0)

### 1.1 gridpack.json schema (`crates/grid-core/src/backup/pack.rs`)

This extends the spec's draft. All new fields are additive, and "Decided 1–4" is kept. The one real change: `files` holds objects, not bare strings. This is Question 3.

```rust
pub const PACK_DIR: &str = "gridpack";
pub const PACK_FILE: &str = "gridpack.json";
pub const FORMAT: u32 = 1;
pub const TMP_SUFFIX: &str = ".gridtmp";          // every in-flight file: "<name>.gridtmp"
pub const FAT32_MAX_FILE: u64 = 4_294_967_295;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PackOs { Linux, Windows }               // unknown values -> parse error -> drive status "unreadable"
impl PackOs { pub fn host() -> Option<Self>; pub fn forge_host(self) -> &'static str /* "linux" | "win32" */; pub fn arch_tag(self) -> &'static str /* "linux-x86_64" | "windows-x86_64" */ }

pub struct Pack {
    pub format: u32, pub name: String, pub os: Vec<PackOs>,
    pub created: String, pub updated: String,
    #[serde(default)] pub games: Vec<PackGame>,
    #[serde(default)] pub emulators: Vec<PackEmulator>,
    #[serde(default)] pub firmware: Vec<PackFirmware>,
    #[serde(flatten)] pub extra: BTreeMap<String, serde_json::Value>,   // round-trips unknown keys
}
pub struct PackSource {            // everything needed to rebuild a RomDetail and the image cache keys
    pub server: String, pub rom_id: i64, #[serde(default)] pub igdb_id: Option<i64>,
    #[serde(default)] pub fs_name: String, #[serde(default)] pub server_updated_at: String,
    #[serde(default)] pub cover_small_path: String, #[serde(default)] pub cover_large_path: String,
    #[serde(default)] pub screenshot_urls: Vec<String>, #[serde(default)] pub fanart_urls: Vec<String>,
    #[serde(default)] pub logo_url: String,
}
pub struct PackGame {
    pub platform: String /* RomM slug */, pub platform_name: String, pub source: PackSource,
    pub name: String, #[serde(default)] pub summary: String, #[serde(default)] pub region: String,
    #[serde(default)] pub languages: String, #[serde(default)] pub tags: String, #[serde(default)] pub revision: String,
    #[serde(default)] pub rating: String, #[serde(default)] pub companies: Vec<String>,
    #[serde(default)] pub release_date: String, #[serde(default)] pub genres: Vec<String>,
    #[serde(default)] pub game_modes: Vec<String>, #[serde(default)] pub franchises: Vec<String>,
    #[serde(default)] pub players: String, #[serde(default)] pub version: String,
    pub size: u64, #[serde(default)] pub sha1: String,
    pub files: Vec<PackFile>, #[serde(default)] pub assets: PackAssets, pub added: String,
}
pub struct PackFile { pub path: String, pub size: u64, pub server_file_id: i64, pub name: String,
    #[serde(default)] pub category: String, #[serde(default = "yes")] pub top_level: bool,
    #[serde(default)] pub md5: String, #[serde(default)] pub sha1: String }
pub struct PackAssets { pub cover: Option<String>, pub cover_small: Option<String>, pub fanart: Option<String>,
    pub logo: Option<String>, #[serde(default)] pub screenshots: Vec<String> }
pub struct PackEmulator { pub name: String /* profile name */, pub kind: String /* "emulator" | "compat_tool" */,
    pub source_id: String /* catalog "owner/repo", "romm:<folder>/<emu>", or "local:<name>" for an archived manual emulator */, pub version: String,
    #[serde(default)] pub program: String /* local: path of the program inside the archive */, #[serde(default)] pub args: String /* local: launch options */,
    pub os: String /* PackOs::arch_tag */, pub platforms: Vec<String>, pub path: String,
    #[serde(default)] pub supplementals: Vec<String>, pub asset_name: String, pub size: u64 }
pub struct PackFirmware { pub platform: String, pub platform_name: String, pub files: Vec<PackFirmwareFile> }
pub struct PackFirmwareFile { pub name: String, pub path: String, pub size: u64, #[serde(default)] pub md5: String }
```

**Functions (pure or filesystem-local, all unit-tested):**

- `read_pack(mount) -> Result<PackState, PackError>`
  - `PackState::{Absent, Ready(Pack), NewerFormat(u32), Unreadable(String)}`.
  - A `format` greater than `FORMAT` is read-only. It is never rewritten.
- `validate_rel(rel) -> Result<PathBuf, PackError>` and `resolve(mount, rel) -> Result<PathBuf, PackError>`.
- `write_pack_atomic(mount, &Pack)`:
  1. Write `gridpack.json.gridtmp`.
  2. `sync_all`.
  3. `fs::rename` over `gridpack.json`.
  4. On unix, fsync the folder.
- `cleanup_leftovers(mount) -> io::Result<u64>`. Removes every `*.gridtmp` under `gridpack/`, and nothing else.
- `new_pack(name, os) -> Pack`. A pack has no id: a drive is identified by its mount path (Linux, macOS) or drive letter (Windows), which the OS keeps unique.
- Layout helpers:
  - `game_dir_rel(platform_slug, fs_name, single_file_name)`:
    - A single file goes to `games/<slug>/<file>`.
    - A multi-file or native game goes to `games/<slug>/<fs_name>/<file>`. This matches the ClasPlatf `games/ngc/Wario World (igdb-1700)/` shape.
  - `asset_dir_rel(slug, game_stem)` gives `assets/<slug>/<stem>/`.
  - `emulator_dir_rel(name, version, os)`.
  - `firmware_dir_rel(slug)`.
  - Every component goes through `library::paths::sanitize_component`.
  - A collision check is case-insensitive (FAT, exFAT and NTFS fold case). On a collision, the folder gets ` [rom <id>]` appended.
- `pack_game_detail(&PackGame) -> RomDetail`. The inverse of the backup mapping. It rebuilds `files: Vec<RomFile>` from `PackFile` (id, name, size, category, top_level), so `plan_install` picks exactly the same targets the server install would.
- Index edits: `upsert_game`, `remove_game`, `upsert_emulator`, `remove_emulators_for_os`, `upsert_firmware`, `pack_bytes()` (the sum of indexed sizes, used for the usage bar), `has_game(server, rom_id)` (host-normalized compare).
- `PackLock`: a process-wide `Mutex<HashMap<PathBuf /*mount*/, Arc<Mutex<()>>>>`. Every edit of an index is one locked read-modify-write.

### 1.2 Drive detection model (`crates/grid-core/src/backup/drives.rs`, `drives/{linux,windows,macos}.rs`)

```rust
pub struct DetectedDrive {
    pub mount: PathBuf, pub label: String, pub fs: String /* raw: vfat, exfat, ntfs3, fuseblk, FAT32, NTFS... */,
    pub fs_display: String /* FAT32 | exFAT | NTFS | ext4 | ... */, pub fat32: bool,
    pub total_bytes: u64, pub free_bytes: u64, pub read_only: bool,
}
pub fn default_roots() -> Vec<PathBuf>;                       // per OS, see below
pub fn scan() -> Vec<DetectedDrive>;                           // honours the e2e override
pub fn fs_display(raw: &str) -> (String, bool /*fat32*/);      // pure, tested
pub fn diff(prev: &[DriveKey], next: &[DriveKey]) -> DriveDiff { appeared, removed } // pure
pub fn disk_space(path) -> Option<(u64 /*total*/, u64 /*free*/)>;   // moved next to relocate::free_space_at
```

**Linux.** Parse `/proc/self/mountinfo` (a pure parser over a string, unit-tested with captured samples).

- Keep entries whose mount point is under `/run/media/` or `/media/`, and whose source is under `/dev/`.
- The fstype comes from mountinfo.
- `read_only` comes from the `ro` mount option.
- The label is the mount folder's name (udisks names the folder after the label). Fall back to a `/dev/disk/by-label` reverse lookup.
- Mountinfo uses octal escapes such as `\040` for a space. Unescape them.

**Windows.**

- Enumerate letters with `GetLogicalDrives`. Skip `%SystemDrive%`.
- `GetDriveTypeW`:
  - `DRIVE_REMOVABLE`: include.
  - `DRIVE_FIXED`: include only if `IOCTL_STORAGE_QUERY_PROPERTY` (StorageDeviceProperty) reports `BusType` USB, SD, MMC or 1394. This catches USB SSDs such as the SAMSUNG T7, which Windows reports as fixed.
- `GetVolumeInformationW` gives the label and the fs name (`FAT32`, `exFAT`, `NTFS`).
- `GetDiskFreeSpaceExW` gives the space.
- All of these are covered by the `windows-sys` features grid-core already enables (`Win32_Storage_FileSystem`, `Win32_System_Ioctl`, `Win32_System_IO`).

**macOS (cheap, best effort, not a release target).** Scan `/Volumes/*`, skip any entry that canonicalizes to `/`, and use `statfs` for `f_fstypename`. Compile-gated. No CI.

**e2e override.** With the `e2e` feature only, `GRID_LAUNCHER_E2E_DRIVE_ROOTS` (a path list) replaces `default_roots()`.

- Every child folder of a root counts as a mounted drive.
- An optional `<child>/.e2e-drive.json` with `{label, fs, total_bytes}` fakes the label, fs and size. This lets a test show the FAT32 warning.
- So a spec plugs in a drive with `mkdir` and unplugs it with `rename` or `rm -r`.
- Same pattern as `relocate::FORCE_COPY_ENV`.

**Recommendation and tradeoffs.**

| Approach | Pros | Cons |
|---|---|---|
| `sysinfo::Disks` + `is_removable()` | Cross-platform; fs type and space in one call | `is_removable` reads `/sys/block/<dev>/removable` on Linux and `GetDriveTypeW == DRIVE_REMOVABLE` on Windows. USB SSDs and HDDs report RM=0 / `DRIVE_FIXED` there, so they are missed. The ClasPlatf stick shows RM=1, but a T7 would not. It is also a new dependency that lists every snap and loop mount. |
| udisks2 over D-Bus (zbus) | Authoritative (`HintAuto`, `Removable`, label), with change signals | Heavy async dependency. Linux only. Absent in the e2e container, so tests would need a fake bus. |
| `lsblk -J` subprocess | Rich data (RM, HOTPLUG, TRAN, LABEL) | A process per poll. Needs util-linux. Parsing a CLI's output. |
| **Mountinfo + Win32 APIs, polled every 2 s (recommended)** | No new dependencies. Works for USB SSDs and SD cards (the Steam Deck mounts them under `/run/media`). Matches the spec's "under /run/media and /media". Trivial to fake in e2e. | You keep a small parser per OS. Polling instead of events: about 2 s of latency, which the spec ("within a few seconds") allows. |

Optional later: on Linux, `poll(2)` `/proc/self/mountinfo` for POLLPRI to get updates at once.

### 1.3 IPC contract (Tauri commands in `app/src-tauri/src/commands/backups.rs`, events, TS types in `api.ts`)

**DTOs (serde, snake_case).**

```
DriveInfo { key: String /* the mount path, or the drive letter on Windows */, mount: String, label: String,
  fs: String /* fs_display */, fat32: bool, total_bytes: u64, free_bytes: u64, read_only: bool,
  status: "ready" | "not_set_up" | "read_only" | "unreadable" | "newer_format",
  pack: Option<PackSummary> }
PackSummary { name, os: Vec<"linux"|"windows">, game_count, gridpack_bytes,
  emulators: Vec<{ name, version, os }>, firmware: Vec<String /*platform_name*/>, updated: String,
  rom_ids: Vec<{ server: String, rom_id: i64 }> }        // for Details "On Couch kit"
RememberedDrive { mount, name, label, fs, total_bytes, game_count, last_seen: String }  // keyed by mount path / drive letter
DrivesSnapshot { connected: Vec<DriveInfo>, remembered: Vec<RememberedDrive> }
DriveGame { rom_id, server, name, platform, platform_name, size, files_max: u64,
  cover: String /* rel */, installed: bool, ... (the PackGame fields Details needs) }
BackupRequest { rom_ids: Vec<i64>, title: String /* dialog title text source */ }
BackupPlan { drive: String, games_to_write: Vec<i64>, skipped: Vec<{rom_id,name}>, too_big: Vec<{rom_id,name}>,
  emulators: Vec<{ name, os: Vec<..>, bytes: Option<u64>, already: bool }>, firmware: Vec<{ platform_name, files, bytes, already: bool }>,
  firmware_reason: String /* why the checkbox is disabled, else "" */, bytes_to_write: u64, free_bytes: u64, fits: bool }
BackupQueued { batch: u64, games: u32, emulators: u32, firmware: u32 }
OsChangeOutcome { batch: u64 /*0 when nothing queued*/, queued: u32, removed: u32 }
```

**Commands.** Each is a thin wrapper.

| Command | Notes |
|---|---|
| `list_drives() -> DrivesSnapshot` | |
| `setup_drive(mount, name, os) -> DriveInfo` | Refuses a mount the current scan did not find. |
| `rename_drive(key, name)` | |
| `set_drive_os(key, os) -> OsChangeOutcome` | Removing an OS works offline. Adding one needs a server. |
| `remove_grid_files_size(key) -> u64` | |
| `remove_grid_files(key)` | |
| `forget_drive(mount)` | |
| `open_drive_folder(key)` | Uses `tauri-plugin-opener`. |
| `drive_games(key) -> Vec<DriveGame>` | |
| `drive_asset(key, rel) -> String` | Seeds the image cache; returns the cache path. |
| `plan_backup(key, request, include_firmware) -> BackupPlan` | |
| `start_backup(key, rom_ids, include_firmware) -> BackupQueued` | Needs a server. |
| `install_from_drive(key, rom_id)` | |
| `remove_from_drive(key, rom_ids)` | |
| `get_backup_settings() / set_backup_include_firmware(bool)` | |
| `use_without_server()` | |

**Event.** `drives-changed` carries `DrivesSnapshot`. It is emitted only when the snapshot changes.

**`DownloadEntry` gains `drive_id`, `drive_name`, `drive_label` (Strings) and `batch: u64` (0 for none).**

- New `job` value: `"backup"`.
- New kinds:
  - `backup_game`, `backup_emulator`, `backup_firmware`
  - `backup_cancelled`: the "13 more items" summary row.
- An install-from-drive row stays `job:"game", kind:"base"`. A non-empty `drive_label` makes the UI say "Copying from <label>".

**`RestoreOutcome` gains `Offline { server_url, username }`.**

---

## Part 2: Ordered tasks

Legend: **[P-A]**, **[P-B]** and so on mark parallel waves. Tasks in the same wave touch disjoint files and can run in parallel worktrees. Waves run in order.

### Task 0: Contract skeleton [sequential, first]

**Files.**
- Create: `crates/grid-core/src/backup/mod.rs` (module decls), `backup/pack.rs` (types and consts only), `backup/drives.rs` (types and signatures with `todo!()`-free stub bodies that return empty), `backup/plan.rs` (types), `backup/write.rs` (empty).
- Modify: `crates/grid-core/src/lib.rs` (`pub mod backup;`), `app/src/lib/api.ts` (all TS types above, no wrappers yet).

**Tests.** One serde round-trip test of the spec's own draft JSON (adapted to `files` objects) and of the ClasPlatf shape. The ClasPlatf shape must fail cleanly as `Unreadable`: it is an old prototype format and is not migrated.

**Depends on.** Nothing.

### Task 1: gridpack read, write, validate, cleanup [P-A]

**Files.** `crates/grid-core/src/backup/pack.rs`.

**Build.** Everything in §1.1.

**Tests (`#[cfg(test)]`, tempdir):**
- Path validation: `..`, absolute paths, `C:\`, a backslash, an over-long component.
- Atomic write leaves no `.gridtmp` behind.
- A crash simulation (an existing `gridpack.json.gridtmp` plus a valid index): read returns the valid index, and cleanup removes the temporary file.
- Cleanup never touches a file outside `gridpack/`, nor a non-`.gridtmp` file.
- Unknown keys round-trip.
- `format: 2` returns `NewerFormat`.
- Case-insensitive collision naming.
- `pack_game_detail` then `plan_install` gives the same targets as the server path, for a single-file game, a multi-file game and a native game. This test lives in `library/mod.rs` tests after Task 5; here, assert the `RomDetail` fields.

**Depends on.** Task 0.

### Task 2: Drive detection [P-A]

**Files.** `crates/grid-core/src/backup/drives.rs`, `backup/drives/linux.rs`, `drives/windows.rs`, `drives/macos.rs`, `crates/grid-core/src/platform.rs` (add `disk_space`), `crates/grid-core/src/library/relocate.rs` (only so `free_space_at` calls `platform::disk_space`; no behaviour change).

**Build.** §1.2.

**Tests.**
- The mountinfo parser on samples: udisks `/run/media/samm/ClasPlatf exfat`, a `\040` label, an `ro` vfat mount, a tmpfs under `/media` (excluded), a fuse sshfs (excluded).
- The `fs_display` table.
- `diff`.
- An integration test `crates/grid-core/tests/backup_drives.rs` (e2e-feature-gated, run with `cargo test -p grid-core --features e2e`): the override root with two child folders and a `.e2e-drive.json` gives two drives with the faked fields.
- Windows: one `#[cfg(windows)]` smoke test that `scan()` does not panic. The windows-native gate runs it.

**Depends on.** Task 0.

### Task 3: Config: remembered drives, firmware default, offline flag [P-A]

**Files.** `crates/grid-core/src/config.rs`.

**Build.**
- Scalar `offline: bool` (default false), placed with the other scalars.
- A new `[backup]` table:
  ```
  BackupSettings { include_firmware: bool = true, #[serde(default)] drives: Vec<RememberedDrive{ mount, name, label, fs, total_bytes, game_count, last_seen }> }
  ```
- Mind the existing ordering note: tables go after every scalar, before `extra`.

**Tests.**
- Defaults when the key is missing.
- Round-trip.
- The existing `preserves_unknown_keys` still passes.

**Depends on.** Nothing.

### Task 4: Queue: backup job key, drive fields, drive abort, cancellable write [P-A]

**Files.** `crates/grid-core/src/library/queue.rs`.

**Build.**
- `JobKey::Backup { drive: String, item: BackupItemKey }` with `BackupItemKey::{Game(i64), Emulator { name: String, os: PackOs }, Firmware(String /*slug*/)}`.
- `DownloadEntry` gains `drive_id, drive_name, drive_label, batch`.
- `admit` takes an `EntryMeta { drive_id, drive_name, drive_label, batch }`. Add `admit_with(meta)` and keep `admit` as a wrapper so existing call sites stay unchanged. For a `Backup` key, `job` is `"backup"`.
- `set_drive_meta(id, meta)`, used by install-from-drive rows.
- `cancel_drive_queue(drive_id, reason) -> usize`:
  1. Removes every waiting entry whose `drive_id` matches, both backup rows and install-from-drive rows, from `waiting` and from `entries`.
  2. If at least one was removed, pushes one `Cancelled` row: `job "backup"`, `kind "backup_cancelled"`, title `"<n> more items"`, error = reason, the same `batch` as the first removed entry.
  3. Never touches slot owners.
- Write-phase cancel:
  - `request_cancel` for an id in `finalize_active` whose key is `Backup` sets `Cancelling` and returns `CancelAction::ActiveWrite`.
  - `finalize_finished` maps `Err(LibraryError::Cancelled)` to `Cancelled` (today it maps to `Failed`).
  - Non-backup finalize stays uncancellable.
- `LibraryError::DriveRemoved(String)` lives in `library/mod.rs`. Coordinate: Task 4 adds the variant there in one line, the only edit to `mod.rs` in this wave. Both `download_finished` and `finalize_finished` map it to `Failed` with its text, for example "SAMSUNG T7 was removed".
- `retryable` for `backup_cancelled` returns `None`, so that row offers Dismiss only.

**Tests.** Pure, in the existing `mod tests`:
- Dedupe per (drive, item).
- `cancel_drive_queue` leaves the slot owner and other drives alone, and makes exactly one summary row with the right count and batch.
- A write-phase cancel goes to `Cancelled`.
- `DriveRemoved` goes to `Failed`.
- Pruning still respects `TERMINAL_HISTORY`.

**Depends on.** Task 0 (`PackOs`).

### Task 5: Install source abstraction and shared download candidates [P-B]

**Files.** `crates/grid-core/src/library/mod.rs`, new `crates/grid-core/src/library/drive_copy.rs`.

**Build.**
- Pull the file choice out of `plan_install` and `plan_native_install` into `pub(crate) fn download_set(detail) -> Result<DownloadSet, LibraryError>`, with `DownloadSet::{Single(&RomFile), Multi { files, launch }, Native { archive, sidecar }}`. `plan_install` keeps its exact behaviour; the existing plan tests must stay green unchanged.
- Replace `InstallJob.client: Arc<RommClient>` with `source: GameSource`:
  ```
  enum GameSource {
      Server(Arc<RommClient>),
      Drive { drive_id, label, files: Vec<(PathBuf /*src on drive*/, PathBuf /*dest*/, u64)> },
  }
  ```
- In `spawn_download`, the `Drive` arm runs `drive_copy::copy_targets(&files, &cancel, &mut on_progress)` on `spawn_blocking`.
  - It copies in 1 MiB chunks, checks cancel per chunk, and gives cumulative progress like `download_targets`.
  - It skips a dest whose size already matches (same rule as `existing_matching_size`).
  - On error it deletes the current partial dest.
  - An I/O error while the source drive's root no longer exists becomes `LibraryError::DriveRemoved("<label> was removed")`.
- `queue_xbox360_content` (mod.rs:2786, which reads `job.client`) runs only for `GameSource::Server`.
- `plan_install` takes `source: GameSource`. Every test call site passes `GameSource::Server(client())`.

**Tests.**
- `copy_targets`: progress totals, cancel, a missing source becomes `DriveRemoved` (simulate by deleting the source root mid-copy, with a tempdir and a slow reader through a test hook), skipping an existing complete file.
- The round-trip test from Task 1: `pack_game_detail` then `plan_install` then targets, for single-file, multi-file and native games.

**Depends on.** Tasks 1 and 4 (the `DriveRemoved` variant).

### Task 6: Forge and server-build resolution for another OS [P-A]

**Files.** `crates/grid-core/src/launch/forge.rs`, `crates/grid-core/src/launch/source.rs`, `crates/grid-core/src/launch/setup.rs`.

**Build.**
- `ForgeClient::resolve_for_host(raw, profile_name, host)`. `resolve` becomes a wrapper with `HOST_PLATFORM`. It uses the existing `resolve_source_for_host`.
- A supplementals helper that takes a host as well.
- `setup::server_builds` and `newest_server_build` already take `host: &str`, but the CPU comes from `Cpu::host()`. Add a `cpu` parameter, with `Cpu::host()` as the default at existing call sites. A backup asks for `(PackOs::Windows, x64)` and `(PackOs::Linux, x64)`.

**Tests.**
- A unit test that `resolve_for_host(.., "win32")` picks the win32 `platform_overrides` asset. Use the wiremock pattern already in `forge.rs` tests, for example with the RetroArch profile, whose override picks the linux or windows asset.
- A server-build test picking the `arch-windows-x64` file on a Linux host.

**Depends on.** Nothing.

### Task 7: Firmware source trait [P-A]

**Files.** `crates/grid-core/src/firmware/mod.rs`, `crates/grid-core/src/firmware/routing.rs`.

**Build.**
```
trait FirmwareSource: Sync {
    async fn records(&self) -> Result<Vec<FirmwareRecord>, String>;
    async fn bytes(&self, record: &FirmwareRecord) -> Result<Vec<u8>, String>;
}
```
- Implement it for `ServerFirmware { client, platform_id }`, which is today's `client.firmware` / `client.firmware_bytes`, and for `PackFirmwareSource { mount, entry: PackFirmware }`. The latter builds `FirmwareRecord`s from the pack and reads bytes from validated paths.
- `install_platform_firmware_recorded` becomes generic over the source. The current signatures stay as thin wrappers.
- Add `routing::install_for_game_from(source, ctx)`.
- `verify_download` is unchanged and applies to drive bytes too.

**Tests.** The existing `crates/grid-core/tests/firmware_install.rs` stays green. Add a pack-source test: the files route to the same targets as the server source.

**Depends on.** Task 0.

### Task 8: RomM model additions for sizes, logo and hashes [P-A] (api-tester)

**Files.** `crates/grid-core/src/romm/mod.rs`.

**Build.**
- `GameSummary` gains `fs_size_bytes: i64` and `largest_file_bytes: i64`. The largest file is the maximum top-level game-category `files[].file_size_bytes` from `SimpleRomSchema.files`. The Server select bar ("3 selected · 4.2 GB") and the FAT32 dialog check need both, with no per-game request.
- `RomDetail` gains `logo_url`, with the same `logo_url_from_payload` helper.
- `RomFile` gains `md5_hash` and `sha1_hash` (optional strings; the openapi `RomFileSchema` has both).

**Tests.** Payload decode tests beside the existing ones (lenient when fields are missing).

**Depends on.** Nothing.

### Task 9: Write engine [P-B]

**Files.** `crates/grid-core/src/backup/write.rs`.

**Build.**
- `write_file(src_local, mount, rel, fat32, cancel, progress) -> Result<(), WriteError>`:
  1. Validate `rel`.
  2. Refuse a size over `FAT32_MAX_FILE` on FAT32, with "<name> is over 4 GB, which <label> (FAT32) can't hold".
  3. Check space: `disk_space` free must be at least the size plus a 64 MiB margin.
  4. Copy in 1 MiB chunks to `<rel>.gridtmp`.
  5. `sync_all`.
  6. Check that the size equals the source size, and the md5/sha1 when known.
  7. Rename onto `<rel>`, replacing an existing file.
- `classify_io(err, mount) -> WriteError::{DriveRemoved, NoSpace, Io}`. It is "drive removed" when the mount is no longer in the latest scan or `<mount>/gridpack/gridpack.json` cannot be read.
- `remove_rel(mount, rel)` and `remove_dir_rel` (guarded under `gridpack/`).
- `remove_all_grid_files(mount)` (the guarded delete, §Global).

**Tests.**
- A FAT32 refusal.
- An interrupted copy leaves only `.gridtmp`, and cleanup removes it.
- A mismatched size fails and leaves no final file.
- Rename replaces.
- `classify_io` when the mount folder was deleted.
- The guard refuses a symlinked `gridpack`, a missing `gridpack.json`, and any path other than `<mount>/gridpack`.

**Depends on.** Task 1.

### Task 10: Backup jobs in InstallService [sequential, after 5]

**Files.** `crates/grid-core/src/library/mod.rs`, new `crates/grid-core/src/library/backup_job.rs` (keeps the bulk out of `mod.rs`), `crates/grid-core/src/backup/plan.rs`.

**Build.**
- `JobPayload::Backup(BackupJob)`:
  ```
  BackupJob { drive: DriveTarget { mount, label, name, fat32 }, item: BackupItem, batch, client: Arc<RommClient>,
      staging: PathBuf /* <library>/.grid-staging/backup-<entry id>/ */, staged: Option<Staged> }
  ```
- The **download phase** is `backup_job::download`, run in the download task:
  - **Game.** `client.rom_detail`, then `download_set`, then download into staging with `download_targets(RommProvider)`, plus the assets.
    - Assets: the cover (large, else small), fanart, logo and screenshots, through `client.get_bytes` on the resolved and host-filtered URLs. The same filter as `ensure_image` applies, so no request leaves the server host.
    - Asset failures are warnings. They never fail the row.
    - Builds the `PackGame` from `RomDetail` with `source.server = strip_userinfo(session url)`.
  - **Emulator.** Either a server build, or a forge release:
    - A server build (`romm:` source) uses `newest_server_build(.., host, cpu)` and downloads that file.
    - A forge release uses `resolve_for_host` plus the supplementals for `PackOs::forge_host()`.
    - Compat tool: GE-Proton for Linux only.
  - **Firmware.** `client.firmware(platform_id)`, each file through `firmware_bytes` plus `verify_download`.
- The **write phase** is `backup_job::write`, run on the finalize blocking pool:
  - It re-reads the pack (id check).
  - It skips the item if it is already indexed, and completes with warning "Already on <name>".
  - It calls `write_file` for every staged file, reporting through `on_install_progress`.
  - It updates the index under `PackLock` and calls `write_pack_atomic` after each item.
  - It deletes staging.
  - It sets `updated`.
- `finish_download` never short-circuits a `Backup`.
- `InstallService::start_backup(client, drive, plan: BackupItems, batch) -> Result<BackupQueued>` admits in order: games, then emulators, then firmware. It returns the counts. The batch id is a `next_batch` AtomicU64 on the service.
- `InstallService::abort_drive(drive_id, reason) -> AbortReport { failed, cancelled, written }`:
  1. Marks the drive in a `removed_drives: Mutex<HashMap<String,String>>`.
  2. Flips the cancel flags of every live entry with that `drive_id`. A cancelled download for a removed drive is converted to `DriveRemoved` in `finish_download`.
  3. Calls `queue.cancel_drive_queue`.
  4. The write path also calls `abort_drive` itself when `classify_io` says removed, so a removal before the watcher notices behaves the same way.
  5. `removed_drives` is cleared when the drive appears again.
- `backup::plan::plan(...)` is pure. It takes the pack, the requested `GameSummary`/installed rows, the drive's fat32 flag and free space, the per-system emulator choice (below) and the firmware records. It returns `BackupPlan`.
- **Emulator choice per system.** This is the prototype's `bkEmuFor`:
  - Take `Config.default_emulators[platform label]`, then the installed entry's profile and `source_id`.
  - With no default: fall back to `setup::recommendations()` for the platform.
  - **A manual entry with no source** is archived as it is: GRID zips the emulator's program folder (the folder holding its program) into `emulators/<name>/local/<host os>/<name>.zip` and records `source_id: "local:<name>"`, the program path inside the archive and the launch options. It is built for this computer's OS only, so on a pack for another OS too the dialog notes "<name> is only for Linux". GRID refuses to archive a program in a system folder (`/usr`, `/bin`, `/opt` owned by root, `C:\Windows`, `C:\Program Files`) or a folder over 4 GiB; the dialog then says "<name> can't be copied (installed by the system)". Install from a drive extracts the archive under `emulators/<name>` and adds a manual `[[emulators]]` entry with that program and those options.
  - Windows games: `default_compat_tool`, else GE-Proton. Included only when the pack has Linux.
  - Native games for this OS: no emulator.
- Emulator sizes come from the forge or server resolution. Cache them for the process in the service (`resolved_sizes`) so the dialog does not hit GitHub's 60 requests per hour limit again. When unresolved, `bytes: None`, and the dialog shows "size unknown".

**Tests.**
- An archive test: a manual emulator folder in a tempdir zips with its program path and options recorded; a program under `/usr` (or `C:\Windows`) is refused with the reason.
- `plan.rs` unit tests: skip already-present games (same server and rom), the FAT32 `too_big` list from `largest_file_bytes`, "Nothing to back up", `fits`, firmware already present, the emulator already on the drive for every pack OS.
- Integration `crates/grid-core/tests/backup_service.rs` (wiremock RomM plus a tempdir "drive"):
  - Back up a single-file game and a native game. Assert the drive layout and the `gridpack.json` content, and that nothing exists outside `gridpack/`.
  - Delete the drive folder mid-download (a throttled wiremock response): the current row is `Failed` "X was removed", the rest is one `backup_cancelled` row, and no `.gridtmp` is left after `cleanup_leftovers`.
  - Cancel in the write phase.
  - Retry after a failure.

**Depends on.** Tasks 1, 4, 5, 6, 7, 8 and 9.

### Task 11: Install from a drive (games, emulators, firmware) [sequential, after 10]

**Files.** `crates/grid-core/src/library/mod.rs`, `crates/grid-core/src/library/backup_job.rs` (or a new `library/drive_install.rs`), `crates/grid-core/src/library/registry.rs`.

**Build.**
- `InstallService::install_from_drive(drive: DriveTarget, game: &PackGame) -> Result<()>`:
  1. `pack_game_detail`.
  2. `plan_install(&detail, &library, GameSource::Drive{..})`.
  3. Map each `FileTarget.dest` to its drive file by `server_file_id` (fallback: by name).
  4. Admit under `JobKey::Rom(rom_id)`, so it dedupes with a server install of the same game. The kind stays the same; the meta has `drive_*`.
  5. Finalize is unchanged. `new_record` keeps `rom_id = Some(source.rom_id)`.
- **Covers that work offline.** For a drive install, `new_record`'s image fields are absolute URLs, `resolve_image_url(source.cover_*_path, source.server)`; screenshots and fanart are already absolute. A later connect to the same server resolves them identically.
- **Registry v10: `source_server TEXT NOT NULL DEFAULT ''`.** The registry is at `user_version` 9 today. Use the same migration pattern as `migrate_3_to_4` (one transaction for the `ALTER` and the `user_version` bump).
  - Set it for drive installs.
  - On install with a live session to a different host: store `rom_id = None` (the relink pass can match it).
  - Offline: keep the rom id and record the server.
- `InstallService::install_emulator_from_drive(entry: &PackEmulator, files: abs paths, drive)`:
  - `EmulatorJob` gains `local: Option<LocalBuild { primary, supplementals, version, asset_name, size }>`.
  - `download_emulator` copies into `install_dir` (via `drive_copy`) and fills `job.resolved` with `ResolvedDownload { provider: "gridpack", release_tag: version, asset_name, .. }`.
  - `finalize_emulator` records `source_id` from the pack and `source_release_tag`/version, so update checks work later. For a `romm:` source it records the server source id and version the way `register_server_emulator` does.
  - Compat tool: `JobTarget::CompatTool`.
  - A `local:` entry extracts its zip under `emulators/<name>` and registers a manual `[[emulators]]` entry with the pack's program path and options. It has no update source.
  - The existing ELF/PE header check (`binary_target`) still guards against the wrong OS or architecture.
- `firmware::install_platform_firmware_recorded` with a `PackFirmwareSource` is called from the app layer (Task 13). No queue row, matching today's background firmware passes.

**Tests.**
- Integration `crates/grid-core/tests/drive_install.rs`, with no RomM server at all:
  - A pack with a single-file zip game installs and gets a registry row with the `rom_id` and absolute cover URLs.
  - A native game installs through `finalize_native_base`.
  - Removing the drive folder mid-copy gives `Failed` "was removed", no registry row, and no partial file in the library.
  - An emulator from the pack (a stub AppImage) installs and writes an `[[emulators]]` entry with the pack's `source_id`.
- Registry migration test: v9 to v10 keeps rows.

**Depends on.** Tasks 5, 7 and 10.

### Task 12: Session offline mode [P-B]

**Files.** `crates/grid-core/src/session.rs`, `crates/grid-core/tests/session.rs`.

**Build.**
- `RestoreOutcome::Offline { server_url, username }`.
- `restore()`: when `cfg.offline`, return `Offline` without a probe, and set `self.server_url` to the config's URL if it has one.
- `use_offline()` sets `cfg.offline = true` and saves. It leaves the keyring untouched.
- `connect()` success sets `cfg.offline = false` in the same save.
- `retry()` with `offline` set returns `Err(NoStoredSession)` as before when no credential is stored.

**Tests.** Restore with `offline` makes no HTTP request (wiremock expects 0 calls). A connect clears the flag.

**Depends on.** Task 3.

### Task 13: Image cache: cache hits before the host filter, plus seeding from a drive [P-B]

**Files.** `app/src-tauri/src/commands.rs` (`ensure_image`, around line 384), `crates/grid-core/src/images/cache.rs`.

**Build.**
- In `ensure_image`: resolve first. If `cache.cached_path(resolved)` is present, return it before `filter_to_server_host`. A cache hit makes no request, so the token-leak reason for the filter does not apply. A miss keeps today's filter and fetch exactly.
- `ImageCache::seed(url, src_file) -> Result<PathBuf>`: an atomic copy to `<image_key(url)>.<ext>`, with the extension from `extension_for` sniffing.
- `drive_asset` (Task 14) uses this: the key is the absolute server URL when the pack has one, else `gridpack://<mount>/<rel>`. One file then serves the drive grid and the later installed row.
- The asset protocol scope (`tauri.conf.json`, covers dir only) stays unchanged.

**Tests.**
- Unit test `seed` plus `cached_path`.
- A test in commands (or a pure helper `resolve_for_cache(url, base) -> (resolved, allowed_fetch)`) that an empty base with an absolute URL is a hit when cached and refused when not.

**Depends on.** Task 0.

### Task 14: App wiring: DriveService, commands, events, hooks [sequential, after 10–13]

**Files.**
- Create: `app/src-tauri/src/drive_service.rs`, `app/src-tauri/src/commands/backups.rs`.
- Modify: `app/src-tauri/src/lib.rs` (AppState field, spawn the poll loop in `.setup()`, register commands), `app/src-tauri/src/commands.rs` (AppState struct, `restore_session` handles `Offline`, add `use_without_server`), `app/src-tauri/src/firmware_service.rs` (a `spawn_from_pack` variant through `FirmwareSource`).

**Build.**

The DriveService:
- A tokio interval of 2 s calls `spawn_blocking(drives::scan)` and `read_pack` per drive.
- It builds `DriveInfo` with the status: read-only, newer format.
- It diffs with the previous scan. Then:
  - Appeared and set up: `pack::cleanup_leftovers`, but skipped while the install snapshot has a live entry with that `drive_id`. Clear `removed_drives`.
  - Removed: `install.abort_drive(id, "<label> was removed")`.
- It updates `Config.backup.drives` through `config_write.rs`, only on change: last seen, counts.
- It emits `drives-changed`.

The commands are thin:
- `start_backup` requires `session.client()`, else "Backing up needs a server".
- `setup_drive`:
  1. Re-checks the mount against the latest scan.
  2. Runs a write probe (create, then remove, `gridpack/.probe.gridtmp`). On failure: "GRID can't write to <label>".
  3. Calls `new_pack` and `write_pack_atomic`.
- `set_drive_os`:
  - Removal deletes `emulators/*/*/<os>/` entries (index-driven, never a blind glob) and updates the index.
  - Addition queues `BackupItem::Emulator` for every indexed emulator for the new OS. Needs a client.
- `remove_from_drive` refuses while the drive has a live backup row.
- `install_from_drive`:
  1. Seeds the cache from the pack assets.
  2. Calls `install.install_from_drive`.
  3. If the game's platform has no usable emulator (`launch::selection::default_emulator_name_for_platform` / `emulator_entry_by_name`) and the pack has one for `PackOs::host()`:
     - Writes a `config.setup.pending` `PendingDefault` for the platform label (the waiting-defaults path in `launch::setup::apply_pending_defaults`).
     - Admits `install_emulator_from_drive`.
     - Records a pending drive firmware pass for that platform.
  4. Firmware runs when the emulator-installed hook fires, or at once when the system already has its emulator and needs firmware (`firmware::manifest::firmware_installed`). It goes through `FirmwareService::spawn_from_pack`, and the toast reads "PlayStation firmware installed from Couch kit".
  5. A Windows game on Linux with no compat tool installs the pack's Proton the same way (`request_compat_default`).

**Tests.**
- Rust unit tests for the pure halves inside `drive_service.rs`: the status classifier and the remembered-drive merge, both pure functions.
- Clippy with `--features e2e` passes.

**Depends on.** Tasks 2, 3, 10, 11, 12 and 13.

### Task 15: Frontend contract: api wrappers and drives store [sequential, after 14]

**Files.** `app/src/lib/api.ts` (wrappers), new `app/src/lib/stores/drives.svelte.ts`, `app/src/App.svelte` (init when the phase is `shell`, like the others), `app/src/lib/shell.ts` plus `shell.test.ts` (the `offline` outcome, `chipLabel` reads "Offline · no server", `initialView(connected, offline, firstDrive)`).

**Tests.** vitest for `shell.ts` and a pure `drives.ts` helper: `readyDrives`, `drivesHoldingRom(snapshot, serverUrl, romId)`, which compares hosts like `hostOf`.

**Depends on.** Task 14.

### Task 16: Settings › Backup drives [P-C]

**Files.** `app/src/lib/settings/pages.ts` plus test (insert `'backup-drives'` second, so there are eight panes), `app/src/lib/Settings.svelte` (mount the page), new `app/src/lib/settings/BackupDrivesPage.svelte`, `app/src/lib/backups/DriveCard.svelte`, `app/src/lib/backups/drives.ts` plus `drives.test.ts`.

**Build.** The pure `drives.ts` holds:
- The card model: title, status text, "For Linux and Windows".
- Usage segments: other, GRID, free.
- The FAT32 warning text.
- OS-change validation: at least one stays ticked.
- The OS-change toast text.
- The "Remove GRID files" confirmation text with the bytes it frees.

The cards follow the spec: connected drives first, then remembered ones (last seen, Forget).

**Tests.** vitest for `drives.ts`.

**Depends on.** Task 15.

### Task 17: Downloads rows, strip and batched notifications [P-C]

**Files.** `app/src/lib/Downloads.svelte`, `app/src/lib/DownloadsFooter.svelte`, `app/src/lib/downloads/format.ts` plus test (status lines), new `app/src/lib/backups/notices.ts` plus test, `app/src/lib/notifications/notifier.ts` (route backup rows to it; `notices.ts` skips `job === 'backup'` and drive-sourced rows' queued wording), `app/src/lib/notifications/notices.ts` (exclusion only).

**Build.**

Status lines:
- "Downloading from server 45%"
- "Writing to SAMSUNG T7 70%"
- "Waiting · to Couch kit"
- Completed: "Written to Couch kit"
- "Copying from SAMSUNG T7" for drive installs
- The summary row: "13 more items · Cancelled — SAMSUNG T7 was removed · back up again to finish"

The kind badge reads "Backup".

The strip reads "Backing up Velvet Static to Couch kit · 45% · downloading".

The sparkline uses the network colour while `downloading` and the disk colour while `installing`. Check `downloads/sampler.ts`: it probably keys on status already.

Batch notices are grouped by `batch`, not by the 0.4 s window:
- A batch seen for the first time: "3 games added to the backup queue for Couch kit".
- The OS-change hint, registered through `expectQueued`-style batch hints: "Windows builds of 2 emulators added to the backup queue for Couch kit".
- When every row is terminal: "3 games and 1 emulator backed up to Couch kit".
- A batch with `backup_cancelled` or a failed row: one error card: "SAMSUNG T7 was removed. 4 of 17 items were written to Couch kit."

**Tests.** vitest for the batch diffing (the queued card once, the done card once, the removal card once) and for the status lines.

**Depends on.** Task 15.

### Task 18: Offline mode UI [P-C]

**Files.** `app/src/lib/Connect.svelte` plus `connectForm.ts`/test (the "or" divider and **Use without a server**, enabled once the library field is valid; it calls `setLibraryPath` and then `useWithoutServer`), `app/src/lib/stores/session.svelte.ts` (the `useOffline()` action; the chip menu's "Connect to a server…" sets the phase to `none`), `app/src/lib/Shell.svelte` (the grey dot, chip and menu), `app/src/lib/Server.svelte` (the offline panel replaces the "Not connected to <host>" block at line 544 when `session.offline`), `app/src/lib/settings/ConnectionPage.svelte` plus `connection.ts`.

**Tests.** vitest for `connectForm` and `connection.ts`.

**Depends on.** Task 15.

### Task 19: Backup dialog and entry points [sequential, after 18]

**Files.**
- Create: `app/src/lib/backups/BackupDialog.svelte`, `app/src/lib/backups/dialog.ts` plus test, `app/src/lib/backups/selection.ts` plus test, `app/src/lib/backups/SelectionBar.svelte`.
- Modify: `app/src/lib/Details.svelte` plus `lib/details/actions.ts` (Back up to drive…, "On Couch kit", disabled offline with its reason), `app/src/lib/Server.svelte` (Select in the toolbar beside Size, select mode, "Back up all 11…" in the platform header; not on Discover), `app/src/lib/CardGrid.svelte` / `GameCard.svelte` (a `selecting` prop: the check, ring, shrink, no hover bar; click picks, Shift-click picks a range), `app/src/lib/FoldingHeader.svelte` (if the toolbar slot lives there).

**Build.**

The pure `dialog.ts` turns `BackupPlan` into a view model:
- Titles: "Back up 3 games" / "Back up Cinder Peaks" / "Back up PlayStation · 4 games".
- Drive rows: "Needs 76 GB · 61.4 GB free".
- The emulator line, the firmware checkbox and its reason.
- Notes: skipped games, too big for FAT32.
- The footer: "11.1 GB to write", "Nothing to back up", "Not enough space".
- With no drives: name the connected drives that are not set up, and offer "Set up a drive…" (`Settings.show('backup-drives')`).

The pure `selection.ts` handles toggle, shift-range over the visible order, select all, clear, the total size (uses `fs_size_bytes`) and Esc.

**Tests.** vitest for `dialog.ts` and `selection.ts`. `GameCard.svelte.test.ts` gets a selected-state case.

**Depends on.** Tasks 15 and 18 (both touch `Server.svelte`).

### Task 20: Library › Backup drives [sequential, after 19]

**Files.** `app/src/lib/library/rail.ts` plus test (`RailKey` gains `` `drive:${string}` ``; the "BACKUP DRIVES" heading under the platforms; one row per ready drive with icon, pack name, count and a label tooltip), `app/src/lib/Library.svelte` (the drive header, chips, grid from `driveGames`, Install / Play, select mode with Install N games and Remove from drive…, Library platform-row "Back up all N…" for installed games only), `app/src/lib/library/selection.svelte.ts`, `app/src/lib/Details.svelte` plus `lib/details/subject.ts`/test (a drive subject: "From Couch kit", Install from drive, two-click Remove from drive…), `app/src/lib/icons.ts` (a drive icon).

**Build.**
- Drive covers come from `drive_asset`.
- Offline, Library opens on the first ready drive (`initialView`).

**Tests.** vitest for `rail.ts` (the drive rows and their empty text) and `subject.ts`.

**Depends on.** Task 19.

### Task 21: End-to-end coverage [sequential, last before docs]

**Files.** `scripts/e2e.sh` (`STAGE_GROUPS` gains `"backups:specs/backups-a.spec.ts specs/backups-b.spec.ts specs/backups-c.spec.ts"`; `export E2E_DRIVE_ROOTS="$data_dir/media"` for that group; per-spec seeds), `e2e/wdio.conf.ts` (pass `GRID_LAUNCHER_E2E_DRIVE_ROOTS`), new `e2e/fixtures-backups/` (a small platform with zip ROMs, one multi-file ROM and firmware; reuse `fixtures-firmware` shapes), `e2e/seed/backups*` (a ready-made gridpack for spec c, with a tiny zip and a stub emulator archive), new `e2e/helpers/drives.ts` (`plugIn(label, {fs})`, `unplug(label)`, `readPack(label)`), new `e2e/specs/backups-{a,b,c}.spec.ts`.

**Stages.**
- **backups-a.**
  1. Plug in "TESTDRIVE" (exFAT). Settings › Backup drives shows "Not set up".
  2. Set up with Linux and Windows.
  3. Server select mode: pick 2 games, then Back up to drive…, then the dialog shows the size and firmware ticked, then Back up.
  4. Downloads shows Backup rows that reach "Written to …". Exactly one queued card and one done card.
  5. Assert from Node fs: `gridpack.json` (two games, the server rom ids, relative paths), the files present, nothing outside `gridpack/`, no `.gridtmp`.
  6. Details shows "On <pack>".
  7. A FAT32-faked second drive shows the 4 GB warning in setup.
- **backups-b.**
  1. Mock throttling on (existing mock-romm throttle). Back up 4 games.
  2. During the first download, `unplug`. Expect one Failed "TESTDRIVE was removed", one "3 more items · Cancelled…" row and one error card.
  3. Seed a `games/x/foo.zip.gridtmp`, then `plugIn` again. The temporary file is gone, and `gridpack.json` is still valid.
- **backups-c (offline).**
  1. Fresh profile (no keyring item), seeded pack on the fake drive, mock server offline switch on.
  2. Connect, then **Use without a server**. The chip reads "Offline · no server", Library opens on the drive, Server shows the offline panel.
  3. Install from drive: "Copying from TESTDRIVE", then installed. The registry row (read with sqlite3, as other specs do) has the seeded `rom_id`. The drive's emulator becomes the system's default. Play is enabled.
  4. Back up to drive… is disabled with "Backing up needs a server".
- Existing stages to re-run because their files change: `downloads`, `install`, `connect`, `connect-restore`, `library`, `firmware`, `server-emulator`.
- Check that the mock forge serves win32 assets for the emulator the backup picks. If not, give the fixture's platform a server-build emulator in `fixtures-backups`'s Emulators platform with `arch-linux-x64` and `arch-windows-x64` files.

**Depends on.** Tasks 16–20.

### Task 22: Documentation [last]

**Files.** `SPEC.md`:
- Top Bar notifications: the backup cards and the offline chip.
- Main Sections: Library › Backup drives; Server select and "Back up all"; Downloads Backup rows; Settings gets eight panes with Backup drives.
- First Run Setup: Use without a server.
- Game Details View: Back up, On <pack>, From <pack>.
- A new "Game backups" section: the gridpack format summary.

`ARCHITECTURE.md`:
- The `backup` module.
- `drive_service.rs`, `commands/backups.rs`.
- `lib/backups/`, `stores/drives`.
- The `GRID_LAUNCHER_E2E_DRIVE_ROOTS` note.
- Registry v10.

`BUILD.md`: the e2e group list.

**Depends on.** Everything.

### Parallelism summary

| Wave | Tasks | Notes |
|---|---|---|
| Task 0 | 0 | Alone |
| P-A | 1, 2, 3, 4, 6, 7, 8 | Disjoint files. Task 4 adds one `LibraryError` variant in `library/mod.rs`; nothing else in this wave touches that file. |
| P-B | 5, 9, 12, 13 | Disjoint: `library/mod.rs`+`drive_copy.rs` / `backup/write.rs` / `session.rs` / `commands.rs ensure_image`+`images/cache.rs`. Task 13 and Task 14 both touch `commands.rs`, so Task 13 finishes first. |
| Sequential | 10, then 11, then 14, then 15 | All touch `library/mod.rs`, or wire the app. |
| P-C | 16, 17, 18 | Disjoint frontend files |
| Sequential | 19, then 20 | They share `Server.svelte`, `Library.svelte` and `Details.svelte` |
| End | 21, then 22 | |

---

## Part 3: How backups use the download queue

- **Job kind.** `JobPayload::Backup` shares the one download slot and the one finalize slot with installs. Backups and installs queue behind each other first in, first out. A backup's rows are admitted in the order games, then emulators, then firmware.
- **Phases.**
  - `Downloading` (download slot): resolve and download to `<library>/.grid-staging/backup-<id>/`. Progress uses `downloaded_bytes`/`total_bytes`.
  - `Installing` (finalize slot): the write to the drive. Progress uses `install_processed_bytes`/`install_total_bytes`.
  - The UI words the two statuses by `job === "backup"`.
- **Cancel.**
  - Queued: removed from the queue, as today.
  - Downloading: the cancel flag, as today.
  - Writing: the new `ActiveWrite` path; the writer checks a flag per 1 MiB chunk and removes its `.gridtmp`.
  - Staging is always deleted.
- **Removed drive.** Two triggers end in the same `abort_drive`: the poll noticing the removal, and the writer's `classify_io`.
  - The current item becomes `Failed` with "<label> was removed".
  - Every waiting row for that drive, backup rows and install-from-drive rows alike, collapses into one `backup_cancelled` row.
  - The frontend raises one error card per batch.
- **Completed rows.** Each item is its own Completed row ("Written to <pack>"), subject to `TERMINAL_HISTORY` (50).
- **Retry.** Failed and Cancelled backup rows can be retried through `retry`, which rebuilds the payload from `JobKey::Backup` plus the drive, if it is connected. The summary row only offers Dismiss.
- **Notifications.** One card per batch, through `DownloadEntry.batch` (§Task 17). Per-row cards are suppressed.

## Part 4: Install from a drive

- **The same plan.** `pack_game_detail` rebuilds a `RomDetail` whose `files` carry the server file ids, names, sizes, categories and top-level flags. `plan_install` (through the shared `download_set`) therefore picks the same targets and the same finalize route (`base_finalize_route`) as a server install: extract, native, PS3 or eboot. Only the byte source differs (`GameSource::Drive` and `drive_copy::copy_targets`).
- **The server id is kept.** `new_record` writes `rom_id = source.rom_id`. Registry v10's `source_server` guards against connecting later to another server. On connect, `relink_service::spawn_connected_jobs` first clears `rom_id` on rows whose `source_server` host differs from the current server, then the existing relink pass runs. That prevents cloud saves and play activity going to the wrong ROM. Updates (`update_detection`), cloud saves and play activity then work once connected to the same server.
- **Dedupe.** The job is keyed `JobKey::Rom(rom_id)`. "Already installed" short-circuits through `already_installed` as usual.
- **Emulator, Proton and firmware.** They come from the pack only when the system lacks them, through the waiting-defaults mechanism, the emulator-installed hook and `PackFirmwareSource` (Task 14).
- **Drive removed mid-copy.** `DriveRemoved` gives a Failed row, no registry row, and the partial dest is deleted. The drive's other queued installs are cancelled by the same `cancel_drive_queue`.

## Part 5: Offline mode

**What assumes a connected session today (found in code).**
- `commands.rs` returns `"not connected"` at lines 254, 341, 347, 360 and 533 (install, content, updates, …).
- `ensure_image` filters against `session.server_url()`.
- `withSetupCheck` in `stores/session.svelte.ts` (already skips when not connected).
- The Server grid and Discover loads (already gated on `session.connected` in `Server.svelte`).
- `relink_service::spawn_connected_jobs` (only on connect).
- `update_service`, `firmware_service` triggers (need a client), cloud auto-restore before launch (check that `cloud_service` skips when the client is `None`; add a guard if not).
- The play-session outbox already waits while offline.

**The existing "unreachable" restore outcome already gives a shell with `connected: false`.** Offline mode builds on it:
- `Config.offline` and `RestoreOutcome::Offline` skip the probe entirely.
- A distinct chip and menu.
- The Connect entry point.
- The Server offline panel.
- Backup entry points disabled with "Backing up needs a server".
- Adding an OS to a pack needs a server as well, because server builds need RomM.
- Library, drives, Downloads (drive installs), Settings and play keep working.

**Images offline.** Rows installed from a drive store absolute URLs. `ensure_image` checks the cache before the host filter (Task 13), so they show offline on any computer.

## Part 6: Risks and edge cases

- **FAT32.** Mountinfo cannot tell FAT16 from FAT32 (`vfat`), so the 4 GiB limit applies to all `vfat`. On Windows, `FAT32` and `FAT` are reported directly.
  - The dialog lists games over the limit using `largest_file_bytes`. The writer checks the real size again before writing.
  - Setup warns and suggests exFAT. GRID never formats a drive.
- **exFAT and NTFS.**
  - Both fold case, so pack paths are checked for collisions case-insensitively.
  - Characters invalid on Windows (`<>:"/\|?*`) are sanitized even when writing from Linux. Reserved device names (`CON`, `AUX`, …) are not caught by `sanitize_component` today; extend it, or add a pack-specific check that appends `_`.
  - ntfs-3g shows `fuseblk`, mapped to "NTFS".
  - Linux renames over an existing file on vfat and exFAT, and Windows `MoveFileExW(REPLACE_EXISTING)` does too. Both need tests.
- **Mount point changes.** All pack paths are relative. A drive is identified by its mount path or drive letter. A queued job holds the mount path from its admission. A re-mount at another path mid-job fails as "removed", and the user backs up again; the work is idempotent through the index.
- **Partial writes.** Every file is `.gridtmp`, then `sync_all`, then rename, and the index is rewritten atomically after each item. Leftovers are removed on the next appearance, never while a live job targets that drive. Data still in the drive's cache after a yank is covered by `sync_all`. Optionally show "Safe to remove" once the queue for that drive is empty.
- **Cloned drives.** Not a problem: a drive is identified by its mount path or drive letter, which the OS keeps unique. Two drives with identical packs are two separate drives.
- **Space.**
  - The dialog uses `plan.bytes_to_write` against the drive's free space.
  - Each write re-checks the free space and keeps a 64 MiB margin.
  - Local staging also needs space for the largest single item. The download fails with the existing I/O message if the library volume is full (Question 1).
- **Windows path length.** Rust std adds the `\\?\` prefix for long paths automatically. The real limit is 255 UTF-16 units per component on FAT, exFAT and NTFS: validate it and truncate the stem when sanitizing.
- **Permissions.**
  - A drive formatted ext4 or btrfs under `/run/media` is owned by root, so setup's write probe fails with "GRID can't write to <label>".
  - A drive mounted `ro` gets the `read_only` status.
  - A gridpack written by a newer GRID (`format > 1`) is read-only: install yes, write no.
- **Untrusted pack content.**
  - Path traversal: validated, §Global.
  - Huge JSON: capped.
  - Emulators from a drive are executables. They are installed only on the user's Install, and pass the existing header check. Their trust is the trust in the drive. Document this in SPEC.
- **Drive removed during install from a drive.** Part 4.
- **GitHub rate limits** when sizing emulators for two OSes: resolutions are cached per process; the size shows as unknown when it cannot be resolved.
- **RetroArch.** Its cores are a forge supplemental (`RetroArch_cores.7z` in `emulator-autoprofiles.json`). The pack stores supplementals per OS, so a RetroArch default from the drive applies only once a core is on disk. That is the existing `apply_pending_defaults` rule.
- **Big platform backups.** Admitting 500 items is cheap: details are fetched inside each item's download. Completed history keeps only 50.
- **Secret hygiene.** Strip userinfo from `source.server`. Never log asset URLs with query strings. Run `check_secret_hygiene.sh`. No new `expose_secret()` call sites.

## Part 7: Verification split

**api-tester (live RomM, read-only GETs as `tester`).**
- `GET /api/roms?platform_id=…`: `files[].file_size_bytes` and `fs_size_bytes` decode into `GameSummary` (Task 8).
- `GET /api/roms/{id}`: `ss_metadata.logo_path`, `files[].md5_hash` and `files[].sha1_hash` (Task 8).
- `GET /api/roms/{id}/content/{file_name}?file_ids=<id>` for a single-file, a multi-file and a native ROM: the backup download (Task 10).
- Cover, fanart, screenshot and logo image fetches through the session client (Task 10).
- `GET /api/firmware?platform_id=` and `GET /api/firmware/{id}/content/{file_name}` (Task 10).
- Emulators platform listing, and choosing `arch-windows-x64` builds from a Linux host (Task 6).

**e2e (fake drive through `GRID_LAUNCHER_E2E_DRIVE_ROOTS` plus the mock RomM).** Covered by Task 21: setup, backup, the removal mid-backup, leftover cleanup, offline install from a drive, the emulator default from the drive, and the disabled reasons. Add the existing groups whose files change, listed under Task 21.

**Unit and integration only.** Pack parsing and validation, the mountinfo parser, the queue transitions, the write engine, planning, `copy_targets`, firmware sources, registry v10.

## Part 8: Decisions (from the user, 2026-11-09)

1. **Staging.** A backed-up item downloads into `<library>/.grid-staging/backup-<entry id>/`, then is written to the drive and the staging folder deleted.
2. **Manual emulators.** A manual emulator with no source is archived from its program folder and copied as that archive (Task 10).
3. **File entries.** `games[].files` holds objects (`path`, `size`, `server_file_id`, `name`, `category`, `top_level`, hashes).
4. **No pack id.** A drive is identified by its mount path or drive letter. Cloned drives need no handling.
