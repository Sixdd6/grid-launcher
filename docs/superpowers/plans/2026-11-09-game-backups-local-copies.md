# Game Backups: copying installed games from this computer (implementation plan)

> **For agentic workers:** Do one task at a time, in dependency order. Run the gate after each task. Commit with `git commit --only <paths>` plus the Co-Authored-By trailer. Never run checkout, restore, reset or stash on tracked files. Leave `attachments/` and `.claude/` out of every commit. Suggested home for this plan: `docs/superpowers/plans/2026-11-09-game-backups-local-copies.md`.

**Goal.** Some games in a backup may already be installed on this computer. For each one, GRID can write the local install to the drive as an uncompressed archive, so it does not have to download the game from the server. The Backup dialog gets **"Copy installed games from this computer"**, ticked by default, and shows where each game will come from. If a server update is waiting for a local game, the dialog asks the user to pick: this computer's copy or the newer server version. A game that is Missing, installed outside the library, or not a plain library install downloads as it does today.

**This replaces Decided 1** in `docs/design/system/components/GameBackups/README.md`, "A backup always downloads the game from the server". SPEC.md lines 772-774 say the same and must change too. Every other earlier decision still holds:
- packs have no id;
- downloads go through `<library>/.grid-staging/backup-<id>/`;
- drive writes are checked by read-back against the MD5 of the bytes GRID produced, never against RomM's hash;
- emulators, firmware and the runtime work as before.

---

## Part 1: Decisions this plan makes, grounded in the code

### 1.1 Archive format: uncompressed **tar**, split into parts of at most 4 GiB, for every drive

The user said zip or tar. Tar is the better fit because of how the code works:

- **Zip needs seeking.** `zip::ZipWriter::new` goes back to fill in each entry's sizes and CRC (see `zip_program_folder` in `backup_job.rs`). That breaks hashing as bytes are written, which is how `copy_checked` in `write.rs` checks a file. It also breaks splitting into parts.
- **Tar only writes forward.** The `tar = "0.4.46"` crate is already a dependency. Each part can be hashed as it is written, synced, read back past the cache (`verify_on_drive`) and compared. That is exactly the check `write_file` does today.
- **Restoring also only reads forward.** The parts can be read as one stream straight off the drive, with no seeking and no local copy of the archive.
- **Tar keeps unix file modes** (the executable bit), and the crate handles long names with the GNU extension.

**Parts.** Files are named `installed.tar.001`, `.002`, and so on. Each part is at most `PART_BYTES = 4095 MiB`, which is under `FAT32_MAX_FILE`, so `check_fat32` in `write.rs` never refuses one.

- I recommend splitting on every file system, not only FAT32. It keeps one code path, a drive stays usable if it is copied to FAT32 later, and each read-back covers a smaller piece.
- **Side effect:** the FAT32 `too_big` rule in `plan::plan` no longer applies to local archives. A game whose single server file is over 4 GB can still go on FAT32 as a local archive.
- **Side effect:** anyone can rebuild an archive without GRID with `cat installed.tar.* | tar x`. Document this in the README.

### 1.2 Archive straight to the drive, not to staging

The archive is written straight into the drive's `.gridtmp` part files. It is not written to `<library>/.grid-staging` first.

- **Why:**
  - An uncompressed archive is as big as the install, so staging would need that much free space on the library disk again. Local installs are the big ones (extracted PS4 trees, Windows games), so this matters.
  - The drive is the slow part either way.
  - The check is just as strong: the MD5 is taken over the bytes GRID produced as it writes, and compared with a read-back of each part.
- **Cost:** the write phase (the finalize slot) runs a little longer than writing a staged file of the same size, because it also reads the local files. A file that changes while it is read (for example a running game writing a save) ends up in the archive as it was when read. See Risks.
- **Staging is still used** for the game's art, which is still downloaded from the server.

### 1.3 Single-file and multi-file installs whose files match the server: copy them as they are

Two kinds of local install are byte-for-byte the server's files:
- a single file installed as downloaded (`BaseRoute::Downloaded`, where `record.archive_path` is set);
- a multi-file game (`multi_file_game_dir`).

For these, when the game is **not out of date** and every file in `download_set(&detail)` exists locally with the same name and size, GRID writes the local files as ordinary `PackFile` entries. `StagedFile.src` points at the library file, and `write_file` reads it with no change.

- The pack entry looks exactly like a downloaded game.
- Install from the drive uses `plan_install` and finalize with no change.
- Anything else gets the tar archive.
- This is Question 3.

### 1.4 Pack format: stay at format 1, with additive fields

Format 1 has never shipped. The first backup commit (`b7e3af9`) is in no tag, the newest release is `v0.9.12`, and `feat/game-backups` is unmerged. So the additive fields below cost nothing.

**A real gap to fix now.** `Pack` keeps unknown top-level keys (`extra`), but `PackGame`, `PackFile`, `PackEmulator` and `PackFirmware` do not. Every `edit_pack` call **drops** unknown per-game keys when it rewrites the file. Add `#[serde(flatten)] extra: BTreeMap<String, Value>` to all four.

**Format version.** If this ships in a later release than format 1, it must bump to format 2. An older GRID would read an `installed` entry as a game with `files: []`, fail with `NO_DOWNLOADABLE_FILE`, and its `edit_pack` would remove the `installed` key, leaving parts on the drive that nothing points to. This is Question 1.

### 1.5 Which games count as installed and eligible

The rules live in a pure function, `backup::local::assess(row, missing, library, home, live_server) -> LocalState`. Its checks:

1. **A matching registry row.** The row passes `installed_match(row, rom_id)` and has `rom_id == Some(rom_id)`. Its server matches: `source_server` is empty (a server install from the live session), or `server_key(source_server) == server_key(live)`.
   - No row means the game is simply not installed, and there is no note.
2. **Files present.** The row must not be Missing in `InstallService::installed_rows()`, which runs `reconcile_library`.
   - If it is Missing: "Its files weren't found on this computer" (downloads).
3. **Platform kinds that are refused** (these download, with a reason):
   - the Emulators platform (`is_emulators_platform`);
   - Xbox 360 (`is_xbox360_platform`): its content lives in Xenia's content root, outside the library;
   - PS3 rows with a non-empty `ps3_game_id` or `ps3_trophy_paths`, which were routed into RPCS3's `dev_hdd0` by `specials::ps3::route`.
   - A PS3 game installed as an ISO only (`ps3_iso_path` in the platform folder, no game id) is allowed, as layout `file`.
4. **Layout.** The anchor is found in the same order as `reconcile::anchor`:

| Row columns | Layout | Tree root | Matching finalize route |
|---|---|---|---|
| `multi_file_game_dir` | `multi` (copied as-is when possible) | that folder | multi-file, m3u |
| `native_game_dir` | `native` | that folder, leaving out `prefix/` (`native_wineprefix`) | `finalize_native_base` |
| `extracted_dir` | `extracted` | that folder | `BaseRoute::Extract`, `Eboot{ps4}`, `Eboot{ps5}` |
| `archive_path` only | `file` (copied as-is when possible) | the file | `BaseRoute::Downloaded` |
| `ps3_iso_path` / `extracted_path` only | `file` | the ISO | PS3 ISO-only shortcut |

5. **Location.** The root must be strictly inside the library (`path_change::is_strictly_inside`). Its parent must be `platform_dir(library, row.platform)` or `legacy_platform_dir`. Reuse `reconcile::at_layout_location`, made `pub(crate)`, but without its "root itself" case.
   - `extracted_path` and `native_executable_path` must lie inside the root.
   - Otherwise: "Installed outside the library folder" (downloads). This covers native games moved with Locate… and PS3 `dev_hdd0`.
6. **The tree scan** (`backup::local::scan_tree`) refuses:
   - any symlink or junction ("Its install contains links");
   - any name that is not portable: not UTF-8, containing `<>:"\|?*`, a reserved device name (use the `RESERVED_NAMES` check in `pack.rs`), or a component over 255 bytes. This one is checked only when the pack is for Windows.

   The scan also returns the file count, the tree size and the **exact archive size**:
   - each entry is a 512-byte header plus its size rounded up to 512;
   - names over 100 bytes add a GNU long-name record;
   - each directory is one header;
   - the archive ends with 1024 bytes.

### 1.6 Out-of-date detection

- **The source of truth** is `UpdateService` (`app/src-tauri/src/update_service.rs`). It holds `available: HashMap<rom_id, UpdateInfo{server_rom_file_name}>`, the same set behind Library's Update badges. Add `pub fn info(&self, rom_id) -> Option<UpdateInfo>` and `pub fn has_completed_pass(&self) -> bool`.
- **If no pass has finished this session,** the command layer fetches `rom_detail` for the eligible local games only (4 at a time) and runs `game_has_server_update(row, &ServerVersion{…})`.
- **Version labels:**
  - this computer: `format_version_tag(rom_file_name_version(row.rom_file_name))`;
  - the server: `UpdateService::button_label_for` logic on `server_rom_file_name`;
  - "" when there is no version tag (the dialog then says "an older copy").
- **The backup job** fetches `rom_detail` anyway, recomputes `outdated` and records the true value in the pack. If the user was never asked because the set was stale, the local copy still goes as planned, and the pack records it as out of date.

---

## Part 2: Shared contracts (Task 0)

### 2.1 `crates/grid-core/src/backup/pack.rs`

```rust
pub struct PackGame {
    /* …existing… */
    #[serde(default)] pub files: Vec<PackFile>,            // was required; empty for an archived install
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<PackInstalled>,
    #[serde(flatten)] pub extra: BTreeMap<String, serde_json::Value>,
}
// + `#[serde(flatten)] extra` on PackFile, PackEmulator, PackFirmware.

/// A copy of the game as installed on the computer that backed it up.
pub struct PackInstalled {
    pub archive: ArchiveKind,         // "tar"; #[serde(other)] Unknown
    pub layout: InstalledLayout,      // "extracted" | "native" | "multi" | "file"; #[serde(other)] Unknown
    pub name: String,                 // the folder (or file) name in the platform folder
    pub parts: Vec<PackPart>,         // in order
    pub tree_bytes: u64,              // space the restore needs
    pub tree_files: u64,
    #[serde(default)] pub launch: String,        // the launch file, relative to the tree ("" for `file`)
    #[serde(default)] pub rom_file_name: String, // the row's, for update detection
    #[serde(default)] pub server_updated_at: String, // the row's (the installed copy's)
    #[serde(default)] pub version: String,       // "v1.2" or ""
    #[serde(default)] pub outdated: bool,        // the server had newer files at backup time
    #[serde(default)] pub from_os: Option<PackOs>,
    #[serde(default)] pub row: PackInstalledRow,
}
pub struct PackPart { pub path: String, pub size: u64, pub md5: String /* of the bytes GRID wrote */ }
#[derive(Default)]
pub struct PackInstalledRow {           // relative or plain values only; never an absolute path
    #[serde(default)] pub native_executable: String,     // relative to the tree
    #[serde(default)] pub native_launch_parameters: String,
    #[serde(default)] pub native_launch_source: String,
    #[serde(default)] pub native_working_dir: String,
    #[serde(default)] pub ps4_game_id: String,
    #[serde(default)] pub ps4_content: String,
    #[serde(default)] pub included_dlc: String,
    #[serde(default)] pub ra_id: String,
}
```

- **Paths.** `PackGame::paths()` chains `installed.parts[].path`. That brings `parse_pack` validation, `unique_rel` collisions, `remove_from_drive` and `Pack::paths` along for free.
  - `launch` and `row.native_executable` are paths inside the archive, not pack paths. They are checked at restore time with `validate_rel` plus an "inside the restored tree" check (the same pattern as `drive_install::manual_program`).
- **Size.** `game.size` is the sum of the parts, so `pack_bytes` and the usage bar stay right.
- **Unknown values.** An `Unknown` archive kind or layout never makes the pack unreadable. That game is listed, and Install says "This backup needs a newer GRID".
- **New helpers:**
  - `installed_part_rel(game_dir, index) -> "…/installed.tar.001"`;
  - `PackGame::is_installed_copy()`;
  - `pub const PART_BYTES: u64 = 4095 * 1024 * 1024`.
- **Parts folder.** `game_dir_rel(slug, leaf, None)` through `unique_rel(.., is_file = false)`, the same as a multi-file game.

### 2.2 `crates/grid-core/src/backup/plan.rs`

```rust
pub enum LocalState {
    NotInstalled,
    Eligible { layout: InstalledLayout, archive_bytes: u64, outdated: Option<Outdated> },
    Ineligible { reason: String },   // "Its files weren't found on this computer"
}
pub struct Outdated { pub installed_version: String, pub server_version: String }
pub struct PlanGame { /* …existing… */ pub local: LocalState }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SourceChoice { Local, Download }

pub struct PlanInput<'a> { /* … */ pub use_local: bool, pub choices: &'a HashMap<i64, SourceChoice> }

// New BackupPlan fields (serde default for each):
pub local_available: bool,          // some requested game is installed here (shows the checkbox)
pub use_local: bool,                // echo
pub sources: Vec<PlanSource>,       // one per game being written
pub outdated: Vec<PlanOutdated>,    // eligible local games with a newer server copy
pub needs_choice: u32,              // outdated games with no choice yet, while use_local
pub local_bytes: u64,
pub download_bytes: u64,

pub struct PlanSource { pub rom_id: i64, pub name: String, pub source: SourceChoice, pub note: String /* why it downloads */ }
pub struct PlanOutdated { pub rom_id: i64, pub name: String, pub installed_version: String,
                          pub server_version: String, pub choice: Option<SourceChoice> }

pub struct GameItem { /* … */ pub local: bool }   // BackupItemKey::Game(rom_id) unchanged
```

**What `plan()` does with these:**
- A game is **local** when `use_local` is on, it is `Eligible`, and it is either not out of date or the user chose `Local`.
- `too_big` applies to download sources only.
- `bytes` adds `archive_bytes` for local games and `size` for downloads.
- `needs_choice` counts out-of-date eligible games with no entry in `choices` (only when `use_local` is on).

### 2.3 IPC (`app/src-tauri/src/commands/backups.rs`, `app/src/lib/api.ts`)

- **`plan_backup` and `start_backup`** take `use_local: Option<bool>` (default true) and `choices: Option<Vec<{rom_id, source}>>`.
- **`start_backup` refuses** when `plan.needs_choice > 0`, with "Choose which copy to backup for N games".
- **`DriveGame` gains:**
  - `installed_copy: bool`;
  - `installed_version: String`;
  - `outdated: bool`, so Details from a drive can say "A copy of an installed game · v1.2 (the server had newer files)".
- **The TypeScript types** in `api.ts` mirror all of this exactly.
- **Downloads rows.** A local game row has kind `backup_game_local` (from `BackupItem::row`), so `downloads/format.ts` can say "Getting details from server" and then "Copying from this computer to SAMSUNG T7 45%".

---

## Part 3: The backup side

### 3.1 `crates/grid-core/src/backup/archive.rs` (new)

- **`TarPartWriter`** implements `io::Write`. For each part it writes `<rel>.gridtmp` through `pack::resolve`, hashing as it goes.
  - At `PART_BYTES` it:
    1. syncs the part;
    2. checks the size;
    3. reads the part back with `verify_on_drive` (made `pub(super)`, together with `tmp_path`, `classify_io` and `sync_folder`);
    4. compares the MD5;
    5. moves on to the next part.
  - It checks `cancel` and the space margin (`check_space`) at each part.
  - Drive I/O errors go through `classify_io`.
- **`write_installed_tree(target, tree: &TreePlan, rel_base, part_bytes, cancel, progress) -> Result<Vec<WrittenPart>, WriteError>`**
  - It drives a `tar::Builder` over the writer, with entries in sorted order.
  - Local reads are wrapped so their errors become a new `WriteError::SourceChanged(name)`, never a drive error.
  - It refuses a file whose length differs from what the scan saw.
  - Progress covers write plus read-back over `2 × archive_bytes`.
- **At the end** it renames every part from `.gridtmp` to its final name. On any failure it removes the parts it wrote, unless the drive is gone. This follows the existing pattern in `backup_job::write`.
- **`TarPartReader`** chains the part files off the drive. It checks each part's size and MD5 at the end of that part, and raises `PartMismatch` or `Incomplete`.
- **`restore_tree(parts, dest, cancel, progress, after_chunk) -> Result<(), RestoreError>`**
  - It accepts only regular files and directories. Symlink, hardlink, device and FIFO entries are refused.
  - Each path must have only `Normal` components (the same rules as `validate_rel`); absolute paths and `..` are refused.
  - It masks file modes with `& 0o777` (unix only).
  - It copies in 1 MiB chunks, with the cancel check and `after_chunk` hook.
  - The error-text helpers `extract.rs::is_absolute_entry_path` and `unsafe_path_error` are made `pub(crate)`.
- **Test hook:** `#[doc(hidden)] set_part_bytes_override`, so tests can use small parts.

### 3.2 `crates/grid-core/src/backup/local.rs` (new)

- `assess` and `scan_tree` as described in §1.5.
- `TreePlan { root, layout, excludes: Vec<PathBuf>, entries: Vec<TreeEntry>, archive_bytes, tree_bytes }`.
- `snapshot_row(row, root) -> PackInstalledRow`, which turns the row's paths into tree-relative ones.
- A plan-time cache in `InstallService` (`local_scans: Mutex<HashMap<i64, (Instant, TreeScan)>>`, 60 s), because the dialog plans every ready drive.

### 3.3 `crates/grid-core/src/library/backup_job.rs`

- **`download_game`** branches on `item.local`:
  1. `rom_detail` (still needed for metadata and art).
  2. Read the registry row again and run `assess` again, using `reconcile::fs_probe` on the anchor.
     - If the game is no longer eligible, fail with "<Game> is no longer installed on this computer; backup again". It never silently switches to downloading.
  3. `download_assets` into staging, unchanged.
  4. Decide how to copy it:
     - copy as-is (§1.3): `Staged::Game` with `StagedFile.src` set to the library files;
     - otherwise: a new `Staged::Installed { game, tree: TreePlan, assets }`.
  5. Build the entry with `pack_game(...)`, setting `source.server_updated_at = row.server_updated_at` and `version = installed label` in both cases.
- **`write()`** gets a `Staged::Installed` arm:
  - same "already on the drive" rule;
  - parts folder from `unique_rel(game_dir_rel(.., None))`;
  - art first or last as today;
  - `archive::write_installed_tree`;
  - fills `installed.parts`;
  - `edit_pack` upsert;
  - on error, removes what it wrote.
  - **The write holds a library-gate ticket** (`library_gate.read().enter()`) while archiving, so a library move cannot start mid-read.
- **`BackupItem::row()`** returns kind `backup_game_local` for a local game.

### 3.4 `crates/grid-core/src/library/mod.rs`

- **New `local_backup_busy(rom_id)`:** true when a live `JobKey::Backup` row with `BackupItemKey::Game(rom_id)` and `local == true` exists. Track this in `backup_rows`.
- **Callers that refuse** with "This game is being backed up. Wait for it to finish.":
  - `uninstall`;
  - `install_update`;
  - `install_native_update`;
  - `locate_row` (through `row_is_busy`).
- `admit_backup_item` keeps `GameItem.local`, so `retry_backup` reuses the user's choice.

---

## Part 4: The install-from-drive side (restore)

### 4.1 Routing

`InstallService::install_from_drive` (in `drive_install.rs`) branches:
- **`game.installed.is_none()`:** `plan_install` as today. This includes games copied as-is in §1.3.
- **`Some(installed)`:** the new `plan_restore(&detail, game, installed, &library, DriveSource)`.
  - It refuses an `Unknown` archive kind or layout before admission ("This backup needs a newer GRID").
  - It checks library free space against `tree_bytes + SPACE_MARGIN` (`platform::disk_space`).
  - It returns an `InstallJob` with `mode: Base` (so `already_installed` still short-circuits in `finish_download`), with empty `targets` and `file_ids`, and with a new field `restore: Option<RestorePlan>`.

### 4.2 `RestorePlan { parts: Vec<(PathBuf /*resolved*/, u64, String /*md5*/)>, dest, tmp, layout, installed }`

Where it goes on this computer: always the modern platform folder, `platform_dir(library, game.platform_name)`.
- `extracted`: `<platform_dir>/<sanitize_component(installed.name)>`
- `native`: `<platform_dir>/<sanitize_component(game.name)>`, the same as `plan_native_install`
- `multi`: `<platform_dir>/<sanitize_component(game.name)>`, the same as `plan_install`'s multi branch
- `file`: `<platform_dir>/<installed.name>`

The temporary folder is `tmp = <platform_dir>/.grid-restore-<rom_id>`.

### 4.3 Download phase (`spawn_download`, the `GameSource::Drive` arm)

When `job.restore` is set, it calls the new `drive_restore::restore_from_drive(...)`:
- It runs `archive::restore_tree` into `tmp` on the blocking pool. Progress reads "Copying from <label>", the same wording as today.
- The hook is `drive_copy_hook`.
- **Errors:**
  - an I/O error while `pack_dir(mount)` is gone becomes `LibraryError::DriveRemoved("<label> was removed")` (the same rule as `drive_copy::classify`);
  - a part mismatch becomes "The backup of <Game> on <label> is damaged";
  - on any error or cancel, `tmp` is deleted.

### 4.4 Finalize (`finalize_base`)

`if let Some(r) = &job.restore { return self.finalize_restore(id, job, r, warning) }`. `finalize_restore` does this:

1. Remove an existing `dest` that has no registry row, then rename `tmp` to `dest`.
2. Build the record with a pure function, `restored_record(detail, installed, dest, keep_rom_id, server)`:
   - start from `new_record(detail, &installed.rom_file_name)`;
   - `server_updated_at = installed.server_updated_at`, so an out-of-date copy shows Update once connected;
   - `source_server` and `rom_id` follow `DriveSource.keep_rom_id`;
   - then by layout:

| Layout | Row fields |
|---|---|
| `extracted` | `extracted_dir = dest`, `extracted_path = dest/launch`, plus `ps4_game_id` and `ps4_content` |
| `native` | `native_game_dir = dest`, `extracted_dir = dest/game` when present, `extracted_path`, `native_executable_path = dest/rel` (when inside), parameters, launch source, working dir; on non-Windows hosts create `dest/prefix` and set `native_wineprefix`; `native_compat_tool` left blank so the default applies |
| `multi` | `multi_file_game_dir`, `extracted_path` |
| `file` | `archive_path`; for PS3 also `ps3_iso_path` and `extracted_path` |

3. `make_executable(launch)`. This matters for a copy made on Windows and restored on Linux.
4. `registry.upsert`, the image hook and the `game_finalized_hook`, the same as `finalize_base`.
   - No `queue_xbox360_content`: Xbox 360 is excluded.
   - No `register_server_emulator`: the Emulators platform is excluded.

### 4.5 What is not supported

These are never archived, so they never need restoring:
- PS3 games routed into `dev_hdd0` (`games.yml` and trophies);
- Xbox 360 (Xenia content root);
- Emulators-platform packages;
- anything outside the library folder;
- trees with links or names that are not portable.

**Wine prefixes are left out** (Question 2).

---

## Part 5: App layer and frontend

### 5.1 `commands/backups.rs`

**`plan_games` and `make_plan`:**
- load `install.installed_rows()` on the blocking pool;
- for each requested game, run `local::assess` and (cached) `scan_tree`;
- work out `Outdated` from `UpdateService`, with the detail fetch as fallback (§1.6);
- pass `use_local` and `choices` into `PlanInput`.

**`start_backup`:** refuse when `needs_choice > 0`.

**`drive_games`:** fill the new `DriveGame` fields.

### 5.2 `app/src/lib/backups/dialog.ts` (pure, vitest)

- **`dialogLede(host, useLocal, anyLocal)`:** "GRID copies games installed on this computer and downloads the rest from <host>, then writes each one, with its art and details, to the drive's gridpack folder."
- **`localLine(plan)`:** the checkbox label "Copy installed games from this computer" and its note, for example "3 games from this computer (11.2 GB) · 2 downloaded".
- **`sourceRows(plan)`:** each game with "From this computer" or "Download", plus the note for a game that downloads, for example "Moss & Marrow downloads: its files weren't found on this computer". With more than 6 games this sits in a collapsed "Show each game".
- **`outdatedRows(plan)`:** "Velvet Static: the server has newer files (v1.3; this computer has v1.2)". Two radios each, "Use this computer's copy" and "Download v1.3 from the server". With more than one game, bulk buttons "Use this computer's copies" and "Download all".
- **`footerModel`:**
  - when `needs_choice > 0`: button "Choose for 2 games", disabled;
  - note: "11.1 GB to write · 3.2 GB to download".
- **`planNotes`:** the FAT32 note names only games that download.

### 5.3 `BackupDialog.svelte`

- **New state:** `useLocal = $state(true)` and `choices = $state<Record<number, SourceChoice>>({})`. Both join the re-plan effect's dependencies and are passed to `planBackup` and `startBackup`.
- **New section "Source"** between Drive and Also copy. It is shown only when `plan.local_available`.
- **Hand-off:** designer for layout and styling; coder for the wiring.

### 5.4 Other frontend changes

- **`downloads/format.ts`:** text for `backup_game_local`.
- **Details from a drive:** the "A copy of an installed game" line.

---

## Part 6: Tests

### 6.1 Unit tests (`#[cfg(test)]` beside the code)

**`pack.rs`**
- An `installed` block round-trips.
- An unknown `layout` or `archive` still reads as `Ready`.
- An unsafe part path makes the pack `Unreadable`.
- `paths()` includes parts.
- Unknown keys on game, file, emulator and firmware entries survive `edit_pack`.
- `files` may be missing.

**`local.rs`**
- A table test of `assess`: one row per layout plus each exclusion:
  - Missing;
  - outside the library;
  - legacy platform folder accepted;
  - PS3 routed (refused) vs PS3 ISO-only (accepted);
  - Xbox 360;
  - Emulators platform;
  - another server, and a null rom id;
  - an `extracted_path` outside the root;
  - native leaves out `prefix`.
- `scan_tree` refuses a symlink and names that are not portable.
- **The size estimate equals the bytes `tar::Builder` writes**, for a tree with long names, empty folders and unicode.

**`archive.rs`**
- Parts break at the boundary (`part_bytes = 4096`), with an MD5 per part.
- Cancel removes the temporary parts.
- A read-back mismatch is caught, using a hook like `write.rs`'s `AFTER_SYNC`.
- A file that changes while it is read gives `SourceChanged`.
- Restore round-trips the tree, including modes on unix.
- Restore refuses `..`, absolute, symlink and hardlink entries in a hand-made tar.
- A wrong part MD5 or a missing part fails the restore and cleans up.
- Cancel works mid-restore.
- A drive removed mid-restore (unix) gives `DriveRemoved`.

**`plan.rs`**
- Local vs download for each game.
- `use_local = false` sends everything to download.
- Out-of-date games without a choice raise `needs_choice`.
- An explicit choice is honoured.
- `too_big` counts only games that download.
- `bytes_to_write`, `local_bytes` and `download_bytes` add up.

**Restore helpers**
- A table test of `restored_record` per layout: the paths, `server_updated_at` kept, `keep_rom_id`.
- `plan_restore` destinations per layout.
- The free-space refusal.

**`update_service.rs`**
- `info` and `has_completed_pass`.

### 6.2 Integration tests (`crates/grid-core/tests/`)

**`backup_service.rs`**
- **An extracted local game is backed up.** The wiremock content route is mounted with `.expect(0)`. With the small-part override, the pack has `installed` with several parts, and joining the parts and untarring them gives back the tree.
- **A single-file game copied as-is.** The content route is never hit, and the pack has an ordinary `files` entry.
- **An out-of-date game with choice Download.** The content route is hit once.
- **The drive is pulled mid-archive.** The row fails with "SAMSUNG T7 was removed", the queue is aborted as one row, and only `.gridtmp` files are left, which cleanup removes.
- **Uninstall and update are refused** while the local backup runs.
- **A library move cannot start** while archiving.

**`drive_install.rs`**
- Restore the `extracted`, `native` (`game.json` kept, prefix created on unix) and PS4 `extracted` (eboot) layouts.
- The registry row matches, and the launch file exists and is executable.
- `game_has_server_update` is true for a restored out-of-date copy.
- A damaged part fails the row and leaves no tree and no row.

### 6.3 Vitest

- `dialog.test.ts`: the new functions.
- `format` test: the `backup_game_local` status texts.

### 6.4 e2e: a new `e2e/specs/backups-d.spec.ts` in the `backups` group

**Wiring:**
- add the spec to `STAGE_GROUPS` in `scripts/e2e.sh`;
- add `seedD` in `e2e/seed/backups-seed.mjs`.

**The seed** (using `writeRegistry`) installs three games:
1. an extracted zip game from `fixtures-backups`;
2. a native game with `game.json`;
3. a game whose `server_updated_at` is older than the fixture's, so it is out of date.

**The steps:**
1. Open the dialog for all three. The checkbox is ticked and the sources are listed.
2. Check the out-of-date prompt. The button reads "Choose for 1 game". Pick "Use this computer's copy".
3. Backup.
4. Check `e2e/last-run-requests.log`: no `/content/` request for the local games.
5. Check `gridpack.json` (with the `readPack` helper): `installed` entries with relative part paths, and nothing outside `gridpack/`.
6. Uninstall one game, then install it from the drive. Play becomes available, and the sqlite registry row has paths under the library and the original `server_updated_at`.

**Also re-run** these groups, whose files change: `backups` (a, b, c), `install`, `downloads`, `updates`, `library`.

### 6.5 api-tester

**Not needed.** The local path makes only server calls the original plan already checked live (Part 7): `GET /api/roms/{id}` and the art fetches. It adds no new endpoint or field.

---

## Part 7: Ordered tasks

Legend: [P-x] tasks in the same wave touch different files and can run in parallel.

| # | Task | Files | Depends on |
|---|---|---|---|
| 0 | Contract types: §2.1 types, §2.2 types, TS types (no logic) | `backup/pack.rs`, `backup/plan.rs`, `backup/mod.rs` (`pub mod local; pub mod archive;` stubs), `app/src/lib/api.ts` | — |
| 1 [P-A] | Pack logic and tests: `paths()`, `extra` maps, part helpers, Unknown handling | `backup/pack.rs` | 0 |
| 2 [P-A] | Tar part writer and reader, restore, `write.rs` helpers made `pub(super)`, extract helpers made `pub(crate)` | `backup/archive.rs`, `backup/write.rs`, `library/extract.rs` (visibility only) | 0 |
| 3 [P-A] | `assess`, `scan_tree`, `snapshot_row`; `reconcile::at_layout_location` and `anchor` made `pub(crate)` | `backup/local.rs`, `library/reconcile.rs` (visibility only) | 0 |
| 4 [P-A] | `UpdateService::info` and `has_completed_pass` | `app/src-tauri/src/update_service.rs` | — |
| 5 [P-B] | `plan()` source logic and tests | `backup/plan.rs` | 0, 3 |
| 6 [seq] | Backup job: local download, copying as-is, `Staged::Installed` write, gate ticket, busy guards, `GameItem.local` kept, scan cache | `library/backup_job.rs`, `library/mod.rs`, `library/queue.rs` (if kind or `local` tracking needs it) | 2, 3, 5 |
| 7 [seq] | Restore: `RestorePlan`, `plan_restore`, `drive_restore.rs`, `spawn_download` branch, `finalize_restore`, `restored_record` | `library/drive_install.rs`, new `library/drive_restore.rs`, `library/mod.rs` | 2, 6 (shares `mod.rs`) |
| 8 [seq] | Commands: `plan_games` assessment, `use_local` and `choices` parameters, `start_backup` refusal, `DriveGame` fields, `lib.rs` command list if signatures change | `app/src-tauri/src/commands/backups.rs` | 4, 5, 6, 7 |
| 9a [P-C] | Dialog view model, format texts, api wrappers, vitest | `app/src/lib/backups/dialog.ts`, `dialog.test.ts`, `app/src/lib/downloads/format.ts` (+ test), `app/src/lib/api.ts` | 8 |
| 9b [P-C] | Dialog UI (designer), then wiring (coder); Details-from-drive line | `BackupDialog.svelte`, the Details drive header component | 8 (9b's wiring after 9a) |
| 10 | e2e backups-d plus seed | `e2e/specs/backups-d.spec.ts`, `e2e/seed/backups-seed.mjs`, `scripts/e2e.sh`, possibly `e2e/fixtures-backups/` | 9 |
| 11 | Docs: SPEC "The Backup dialog", "Install from a drive", "The gridpack folder", "Safety rules"; README Decided 1 rewritten plus the schema; ARCHITECTURE (`backup/local.rs`, `backup/archive.rs`, `library/drive_restore.rs`) | `SPEC.md`, `docs/design/system/components/GameBackups/README.md`, `ARCHITECTURE.md` | all |

---

## Part 8: Risks

- **Files changing mid-archive.** A running game or emulator can write saves into the tree, for example a native game saving inside its folder. GRID reads each file in one pass and refuses one whose length changed (`SourceChanged`, which fails the row with "<file> changed while it was being copied; close the game and backup again").
  - Optional: if `LaunchService` exposes the running rom ids, refuse a local copy of a running game up front. Check this in Task 6.
- **The update set may be stale or still computing.** Handled by the detail-fetch fallback (§1.6), and the job records the true `outdated` value.
- **Orphaned parts.** A drive pulled between part renames and the index write can leave renamed parts that no index entry names. This is the same class of problem multi-file games already have today. Possible follow-up: have `cleanup_leftovers` also remove `installed.tar.*` files that no entry names.
- **Same-size patched ROMs.** A single file copied as-is that the user patched without changing its size goes to the drive labelled as the server's file. That fits the intent ("this computer's copy"), but SPEC should say so.
- **Saves in the Wine prefix** stay on this computer (if Question 2 goes the recommended way). SPEC should say so.
- **Restoring on another OS.** A Linux-native game restored on Windows will not run. That is the same as a server install of that game today, so there is no new rule. Names that are not portable are refused at backup time only when the pack is for Windows.
- **Big trees** (PS4 games with 10,000+ files). The scan touches metadata only and is cached for 60 s. Tar headers add at most about 1 KiB per file, and the estimate is exact.
- **The finalize slot is held longer** while a big local game archives, so installs queued behind it wait. This is the same queue rule as today. Cancel works per chunk.
- **Secret hygiene.** Nothing new is logged or stored that comes from the server. `PackInstalledRow` holds only relative paths and plain values. Keep running `check_secret_hygiene.sh`.

---

## Part 9: Decisions (from the user, 2026-11-09)

1. **Release:** ships in the same release as game backups. Keep `"format": 1` with the additive fields.
2. **Wine prefix:** left out of the archive. Saves inside the prefix stay on this computer; SPEC says so.
3. **Exact copies:** a local install that is exactly the server's files (single file as downloaded, multi-file/m3u matching by name and size) goes on the drive as ordinary `PackFile` entries (§1.3).
4. Earlier (this extension): the dialog checkbox "Copy installed games from this computer" is ticked by default; an out-of-date local game asks the user to choose per game; Missing games and installs outside the library download from the server.

