# GRID Launcher Module Map

A stable map of where behavior lives. Update it when module ownership changes.

- Use `SPEC.md` for product behavior and UX intent.
- Use `openapi.json` as the source of truth for RomM server requests.
- Build, test and release commands are in `BUILD.md`.

## Top level

    Cargo.toml            workspace: crates/grid-core, app/src-tauri
    crates/grid-core/     UI-agnostic core library (never depends on Tauri)
    app/src-tauri/        Tauri 2 shell: services, commands, hooks
    app/src/              Svelte 5 frontend
    e2e/                  WebdriverIO harness, mock RomM server, mock forge
    scripts/              e2e.sh, check_secret_hygiene.sh
    *.json                data files and the RomM API contract
    assets/               repository assets (7-Zip for Windows, artwork)

The dependency direction is one-way: frontend → Tauri commands → grid-core. grid-core
knows nothing about Tauri, the webview, or the event system, which is what keeps it
unit-testable in a plain `cargo test`.

## `crates/grid-core`

Modules as declared in `src/lib.rs`:

- `autoconfig` — the `ensure_*` writers that seed an emulator's own settings files so a
  launched game finds its saves, firmware, controller profile and RetroAchievements login
  where GRID expects them. Per-emulator submodules (`azahar`, `cemu`, `dolphin`,
  `duckstation`, `eden`, `pcsx2`, `ppsspp`, `redream`, `retroarch`, `rpcs3`, `xemu`) sit
  on shared `readers`/`writers`/`paths` helpers; `cores` owns RetroArch core lookup.
- `backup` — game backups to removable drives (SPEC "Game backups"). A drive is keyed by
  its mount path, or its drive letter on Windows; a pack has no id. `pack` owns the
  `gridpack/gridpack.json` schema (`Pack` and its serde types), reading it as untrusted
  input (size cap, unknown OS or old prototype shape refused, `validate_rel` and
  `resolve` for every relative path, links refused), the atomic index write, leftover
  `*.gridtmp` cleanup, the folder layout helpers, `pack_game_detail` (a pack game back to
  a `RomDetail`, so a drive install plans like a server install), `server_key`, and
  `edit_pack`, the per-drive locked read-modify-write. `drives` answers what is mounted
  now (`scan`) and diffs scans; the per-OS probes are `drives/linux.rs`
  (`/proc/self/mountinfo` under `/run/media` and `/media`), `drives/windows.rs` (the
  Win32 volume APIs: removable letters plus fixed drives on an external bus) and
  `drives/macos.rs` (`/Volumes`, best effort). `write` is the write engine: every file
  goes to `<name>.gridtmp`, is synced, is read back (past the page cache where the OS
  allows) and checked against the MD5 of the staged bytes it copied, and is renamed. It
  never checks against a server hash: RomM hashes an archive's contents, so a pack
  file's `md5`/`sha1` are informational; `classify_io` tells a removed drive
  from another I/O error; `guard_grid_root` and `remove_all_grid_files` are the only
  whole-folder delete. `plan` is pure: what a backup writes, skips and whether it fits
  (`BackupPlan` for the dialog, `BackupItems` for the queue), the per-system emulator
  choice, the compat tool and umu-launcher picks, the manual-emulator archive
  refusals, and where each game comes from (`LocalState` in, `sources`, `outdated` and
  `needs_choice` out). `local` decides whether an installed game can be copied instead of
  downloaded: `locate` is the pure rule set over the registry row (same rom id and server,
  present, not Xbox 360, the Emulators platform or RPCS3's `dev_hdd0`, anchored directly
  in the library's platform folder) and returns the tree, its layout and its excludes
  (a native game's Wine prefix); `scan_tree` walks it without following links, refuses
  links, special files and names that are not UTF-8 or (on a pack for Windows) not
  portable, and returns the `TreePlan` with the exact tar size; `snapshot_row` keeps the
  row values a restore needs, relative to the tree. Each reason is a `REASON_*`
  constant, the text the dialog shows. `archive` writes and reads the installed copy:
  `write_installed_tree` streams the `TreePlan` through `tar::Builder` into a
  `TarPartWriter` that cuts `installed.tar.NNN` parts of at most `pack::PART_BYTES`
  (4095 MiB, under FAT32's limit, checked at compile time), each written to `.gridtmp`,
  hashed, synced, read back and renamed only when all are checked; `restore_tree` chains
  the parts through `TarPartReader`, checks each part's size and MD5, and accepts only
  regular files and folders with plain relative names; `write_saves_tree` writes a
  game's saves the same way as `saves.tar.NNN`. `saves` owns a Windows game's saves on
  a drive (`PackSaves`): `recorded_folders` turns the cloud-save folder list from the
  config into Windows-token paths (`token_for_prefix_path` for a manual folder inside the
  prefix), `scan_saves` walks the folders that exist in the prefix into a `TreePlan`
  (folder `i` under `i/`), `restore_targets` is the pure rule for where each folder goes
  (inside the new prefix, or the real folder on Windows, never into one that holds
  files), `restore_saves` reads the tar back and copies it there, and `seed_folders`
  gives the folders to cloud saves without replacing anything (`scan_saves_with`,
  `restore_targets_with` and `restore_saves_with` take the caller's own resolver).
  `store` is GOG games on a drive (`Pack.store_games`, under `stores/gog/<name>-<id>/`):
  `decide_source` is the pure layout rule (installer files when every recorded
  installer file, DLC included, is still there with the recorded size, else the
  installed copy), `installed_source` the install folder less its Wine prefix,
  `save_folders` / `resolve_save_folder` GOG's save locations as folder strings
  (`<?INSTALL?>\…` for the install folder), and `plan_detail` the dialog's line;
  Epic, Amazon and Steam are refused with `local::REASON_STORE`. A store backup is a
  `backup_store_game` row (`BackupItem::Store`, no server needed:
  `InstallService::start_store_backup`, planned by `plan_store_backup` over
  `plan::plan_store`); a restore is a `StoreOp::Restore` job built by
  `drive_restore::store_restore_job` (installer files run again through
  `stores::offline::run_into`, origin `offline`; an installed copy moved into place,
  origin `drive`). `steam_runtime` fetches
  the Steam Linux Runtime the way umu does and places it where umu looks.
- `cloud` — the cloud save/state sync engine: scope classification, sync candidates,
  per-emulator directories, archive transfer, restore, retention, session windows, and
  xemu's raw-disk sync.
- `config` — `Config` and its TOML load/atomic save, the emulator entries and UI
  settings it holds, and `data_dir_override()`. `offline` is "Use without a server";
  `[backup]` holds the firmware default and the remembered drives (`RememberedDrive`,
  keyed by mount).
- `fsbrowse` — one folder's listing for TV mode's folder browser (`list_folder`): dot-entries
  hidden, folders first, then a case-insensitive sort by name; files only when the browser
  picks a file. An unreadable entry is left out; only an unreadable folder fails.
- `fatx` — clean-room FATX reader/writer for the `E:` partition of a raw xemu Xbox HDD
  image, so cloud sync can pull `E:/UDATA` and `E:/TDATA` out and put them back.
- `firmware` — server firmware download and install: keyword-based target routing, the
  zip keep-vs-extract decision, and the RPCS3 PS3 firmware path. The install pass reads a
  `FirmwareSource`: `ServerFirmware`, or `PackFirmwareSource` for a drive's `firmware/`
  (`install_platform_firmware_from`); both verify size and MD5 the same way.
- `images` — the image pipeline: URL resolution rules, the on-disk cache, the startup
  sweep, the replenish job, background art, cached video, and `shade` (a cached image's
  dominant shade, remembered per process, for Discover's generated logos).
- `import_python` — one-shot import of a Python-era `~/.grid-launcher/config.json` into
  `config.toml` and the installed-games registry. Holds no token, by construction: the
  parsed struct has no field for one.
- `launch` — the launch core: emulator profiles and the install catalog, forge
  (GitHub/direct) downloads, argument templates, emulator selection per platform, native
  (Windows) launch and compat tools, process spawn and session tracking. Launch and
  sessions are keyed by `stores::GameKey`: `LaunchService::launch_key` resolves a RomM row
  by rom id or a store row by its key (`launch(rom_id)` is a thin wrapper), and a GOG,
  Epic or Amazon row always takes the native branch (`build_native_command`; the row sets
  `extracted_dir`, `native_game_dir`, `native_executable_path` and a `win`/`linux`
  `platform_slug`). Before it builds, a store row asks the app's `StoreLaunchHook` (one
  slot; the app installs a router that asks the row's own store) for a
  `StoreLaunchPrep` (Epic: legendary's executable, folder, arguments, and the exchange
  code as `secret_args`, every one a redacted position; Amazon: nile's FUEL environment). `GameSession` carries `key` and an optional `rom_id`; one live session
  per key. `LaunchService::register_watched` adds a session GRID did not spawn (Steam,
  later): no child, liveness from a polled `AliveCheck`, never signalled by `stop`.
  `NativeLaunch::redact` lists argv positions (an Epic exchange code) that print as
  `<redacted>` (`redact_argv`) in logs, errors and the early-exit warning. `launch::umu`
  owns umu-run: the system-then-managed lookup, the managed copy's install directory and
  forge source, its config record (`Config.umu_launcher`) and the python3 >= 3.10 check;
  `InstallService::install_umu_launcher` installs it.
- `library` — the install pipeline: download queue, extraction, path rules, launch-file
  selection, the SQLite registry, platform rules, install specials (PS3, extra content),
  update detection, and `relink` (Q4: links installed rows that have no server ROM id
  to their unique server match) and `platform_slug_backfill` (fills the registry v8
  `platform_slug` from ROM details; nativeness reads the slug first). `server_emulator` (Q3) registers a package from the
  server's "Emulators" platform as an emulator entry; `emulator_removal` removes an
  emulator's files and finds the package rows a deleted entry takes with it.
  The queue (`queue.rs`) also runs store jobs: `JobKey::Store { source, store_id, op }`
  and `JobPayload::Store(StoreJob)`, admitted by `InstallService::enqueue_store_job`. A
  `StoreJob` carries a `StoreJobRunner` whose `download` and `finalize` halves run on the
  blocking pool through the same slots, progress, cancel and retry as a RomM install
  (`job: "store"`, `source_id` = the game key). A store install the store's own client
  drives (Steam) is a `JobKey::StoreExternal` row (`kind: "store_external"`,
  `admit_store_external` / `finish_store_external` with an `ExternalOutcome`): no slot, no
  waiting, never cancelled or retried by GRID, one live row per product, and a library
  path change does not wait for it.
  `user_data_links` links an emulator's own-written directories into
  `saves/`; `layout_migration` is the one-shot startup migration to the layout below.
  `path_change` (Q2) validates a new library folder, switches `library_path` (the old
  root goes to `Config::former_library_paths`, which the uninstall `RemovalGuard`
  protects like the current root) and picks the rows Start fresh drops; the app side is
  `commands/library_path.rs`. `relocate` (9b) is the "Move existing files" engine: plan
  and refusals, Copy (staging + verify) → Commit (place, relink, rewrite the registry,
  the config and the emulator config files that hold absolute paths, switch the root) →
  Cleanup, journaled in `library-move.json` beside `config.toml` so a crash resumes;
  `relocate_references` rewrites references only (the unreachable-library Relocate);
  `LibraryGate` is the open/closed flag every library writer checks. `root_access` (9b)
  says whether the library root can be reached (not set / reachable / unreachable and
  why), whether GRID stored anything there, and checks a Relocate target. `reconcile`
  (9c) is moved-file detection: a pure plan over the installed rows (present / moved
  inside a library root → re-point the row / missing, computed and never stored) and
  the Locate… folder check; `InstallService::reconcile_library` applies it at startup
  and on every `list_installed`.
  Game backups add three modules. `backup_job` runs a backup row through the same queue
  and slots as an install (`JobPayload::Backup`): the download phase fetches the game,
  its art, an emulator release (or zips a manual emulator's folder) or a firmware set
  into `<library>/.grid-staging/backup-<id>/`; the write phase writes it to the drive
  through `backup::write` and records it in the index under the drive's lock. Staging is
  always deleted. `drive_copy` is the byte source of an install from a drive
  (`GameSource::Drive`): same progress, cancel and skip rules as a download, and a lost
  drive is `LibraryError::DriveRemoved`. `drive_install` plans a drive game with
  `plan_install` over `pack::pack_game_detail` and finalizes it like a server install,
  and installs an emulator, Proton, umu-launcher or the Steam Linux Runtime from the
  drive as an ordinary `EmulatorJob` whose download step copies the backed-up archive.
  `InstallService::abort_drive` (with `QueueState::cancel_drive_queue` in `queue.rs`)
  fails the current row and folds a removed drive's waiting rows into one
  `backup_cancelled` row.
  A game copied from this computer is a `backup_game_local` row: its download phase
  judges the row again with `backup::local::locate`, fetches the details and art, and
  either stages the library files themselves (`exact_copy`: an install that is exactly
  the server's files, not out of date) or scans the tree; the write phase then writes
  the files, or the archive straight from the library to the drive. It holds a
  `LibraryGate` ticket while it writes, so a library move can't start under it.
  Any Windows game row (downloaded or local) also runs `stage_saves` in its download
  phase: the save folders from the config and, when the game is installed here with a
  Wine prefix, the `backup::saves` scan of that prefix; the write phase writes the
  saves after the game's files (holding the gate ticket too) and records `saves` in the
  pack entry. A drive install carries the pack's saves in `DriveSource::saves`, and
  `InstallService::restore_drive_saves` (in `drive_restore`, called by both
  `finalize_restore` and `finalize_base`) puts them back and seeds the config before the
  finalized hook runs; its problems are warnings, never a failed install.
  `drive_restore` restores a pack game with an `installed` block: `plan_restore` refuses
  an unsupported copy (`NEEDS_NEWER_GRID`) or a library without room before admission and
  builds a target-less `InstallJob` carrying a `RestorePlan`; `restore_from_drive` is its
  download step (the parts into `<platform folder>/.grid-restore-<rom id>`); and
  `InstallService::finalize_restore` moves the tree into place and writes the row
  (`restored_record`, which keeps the copy's ROM file name and server timestamp so
  update detection works). `drive_install` routes a pack game to it when the game has an
  `installed` block. `InstallService::local_states` answers the dialog's plan: one
  registry pass, `locate` per rom id, and a scan cache keyed by rom id (fresh for 60 s,
  for the same tree and the same Windows flag; an uninstall drops its entry). The
  app installs a `RunningCheck` (`set_running_check`, wired in `lib.rs` to the launch
  service's sessions), so a game GRID launched is not copied (`RUNNING_NOW`).
  `local_backup_busy` makes uninstall, update and Locate… refuse a game a live local row
  is copying (`BACKING_UP_LOCAL`).
  `space` is the space ledger, owned by `InstallService` (in memory only): each queued
  or running job holds a `Reservation` per filesystem it adds bytes to (keyed by
  `FsKey`: `st_dev` on unix, the volume root on Windows), and a check sees free − every
  other job's outstanding (reserved − written) − `SPACE_MARGIN`. A job's
  `space::JobSpace` rides on its payload (`InstallJob`, `EmulatorJob`, `BackupJob`), so
  it is released whenever the payload drops (finished, failed, cancelled, panicked).
  `InstallService::admit_meta` reserves a game or emulator job's estimated peak
  (`install_estimate`, `emulator_estimate`) and refuses one that does not fit;
  `start_backup_on` checks and reserves a whole batch (drive bytes and library staging)
  in one `try_reserve`. Progress reaches the ledger through `on_download_progress` and
  `on_install_progress` (by entry id, through a weak map); `check_extract_space` resizes
  to a zip's real unpacked size before extracting. `drive_reserved` is the dialog
  plan's `reserved_bytes`; the app hands the ledger to `LibraryMoveService` so a move's
  free-space check counts it. `SpaceProbe` (free space and filesystem key) is
  injectable for tests (`set_space_probe`).
  The registry is at v12. v10's `source_server` records the server a row came from when
  that was not the live session (an install from a drive), `""` otherwise. v11 adds
  `source` (`romm` or a store id), `store_id` and `store_meta` (the store's JSON
  metadata, never a token), and a partial unique index on `(source, store_id) WHERE
  source <> 'romm'`. A store row never has a `rom_id`; its stored `title_key` carries its
  game key after a U+001F separator (`InstalledGame::identity_keys`), so two products
  with one title never collide on `UNIQUE (title_key, platform_key)` and no
  title/platform API (`find`, `remove`, `set_rom_id_by_key`, `rewrite_row_paths`)
  reaches a store row. Store rows use the key APIs: `find_key`, `find_store`,
  `upsert_store`, `remove_key`/`remove_store`, `store_rows`, `touch_last_played_key`,
  `update_images_key`. Store rows are excluded from relink (`relink::needs_link`),
  server update detection, image replenish, play activity, RomM cloud saves
  (`cloud::romm_cloud_applies`) and drive backups (`backup::local::REASON_STORE`); the
  missing-files reconcile checks them but never searches for them.
  v12 adds `installed_by_installer`: a RomM row whose archive held a GOG installer that
  GRID ran (`specials::gog_installer`), which Details words ("Installed by running its GOG
  installer").
  `Registry::clear_foreign_rom_ids` is the relink guard: it clears `rom_id` on rows whose
  `source_server` names another server (compared by `pack::server_key`).
- `pcgw` — PCGamingWiki client that resolves a Windows game's save paths.
- `platform` — host-platform lookups that need OS APIs (for example the Shell-resolved
  Windows Documents folder).
- `play_activity` — play activity (Q10): which finished game session is sent to RomM's
  play-session ingest (≥ 30 s, with a rom id, not the Emulators platform), and the flush
  of the registry's `pending_play_sessions` outbox — what is deleted, dropped or kept
  after each batch.
- `retroachievements` — the RetroAchievements login client. Login only.
- `stores` — external game stores (SPEC "Game stores", `docs/external-stores-plan.md`,
  `docs/stores-phase1-plan.md`, `docs/stores-phase2-plan.md`, `docs/stores-phase3-plan.md`, `docs/stores-phase4-plan.md`). The shared identity types: `StoreKind` (gog, epic,
  amazon, steam, with their platform names "GOG", "Epic Games", "Amazon Games", "Steam"),
  `GameSource` (the registry `source`) and `GameKey` (`Rom(i64)` or `Store { source,
  store_id }`, string and serde form `romm:123` / `gog:1207664643`). Beside them:
  `tools` (the pinned heroic-gogdl, legendary and nile releases, their SHA-256 check and
  the command that starts one), `runner` (the only place a store tool is spawned; its
  output is never logged or forwarded; it writes a stdin secret, reads a `LineParser`'s
  progress and legendary's and nile's known lines as `ToolSignal`s, stops a run past its
  `timeout`, and makes the tool's folder owner-only after each run), `progress` (tool
  progress lines, `legendary_signal` and `nile_signal`), `paths`
  (`<data_dir>/stores/<tool>/`, 0700/0600, `make_tree_private`) and `art` (store image
  URLs, the token-free art cache under `<data_dir>/stores/art/`, and `is_store_art_url`,
  the GOG, Epic, Amazon and Steam host rule `ensure_image` uses: domains with their
  subdomains, plus exact hosts such as `steamcdn-a.akamaihd.net`, never all of
  `akamaihd.net`). The cloud save types every store
  shares (`CloudState`, `SyncMode`, `SaveConflict`) live in `stores` itself.
  `gog` is GOG's HTTP APIs (`api`), sign-in (`auth`), gogdl install, update, repair, DLC
  and uninstall jobs (`install`), cloud saves (`cloud`), the update check (`update`), and
  the parts every install path shares: `goggame` (`goggame-*.info`), `meta` (a row's
  `store_meta` JSON) and `row` (`gog_row`). `offline` is GOG offline installers (a Linux
  `.sh` read as a zip, a Windows Inno Setup `.exe` run silently or, outside TV mode, with
  its wizard; a macOS `.pkg` refused) and their Downloads job. Every GOG Wine prefix is
  `<install dir>/prefix` (gogdl, offline installers, drive restores), so uninstall
  removes it and a backup leaves it out. `epic` is the Epic Games Store through legendary
  (Phase 2). `EpicService` (`mod.rs`) builds every legendary run (`tool()`: the pinned
  zipapp or the test/e2e override, `tool_env()`, `LineParser::Legendary`, and
  `private_tree` so the runner makes `stores/legendary/` owner-only after each run), and
  keeps the last owned list for the jobs. `auth` (sign-in with the code on stdin,
  `status --json`), `list` (`list --json -T`: Windows titles plus third-party ones shown
  as Not supported, DLC folded into the base, portrait covers first; `info`,
  `list-installed`; the owned-cache format and its read/write/remove helpers), `install`
  (install, update, repair and DLC jobs; an update or repair also runs each installed
  DLC; a repair's "N files repaired" is the job's `StoreJobRunner::detail`, the
  Downloads row's "Verified · …"), `update` (the pure build comparison), `launch`
  (`launch --json`: the ticket's `egl_parameters` become redacted `secret_args`;
  `launch_ticket_error` gives GRID's words; it never retries by itself), `cloud`
  (`list-saves` / `sync-saves`, the save path GRID resolves from `CloudSaveFolder`, a copy
  of the local saves under `stores/save-backups/epic/` before a download), `meta`
  (`EpicMeta`) and `row` (`epic_row`; every Epic row is a Windows version with its
  prefix at `<install dir>/prefix`). `amazon` is Amazon Games through nile (Phase 3,
  built 2026-11-14): `AmazonService` (`mod.rs`) builds every nile run (`tool()`: the pinned
  PyInstaller binary or the test/e2e override, `NILE_CONFIG_PATH=<data_dir>/stores/nile`
  (nile keeps its files in `stores/nile/nile/`), `LineParser::Nile`, the folder made
  owner-only before and after each run; short commands set `QUICK_RUN_TIMEOUT`), the
  endpoints and the login-URL host check, `AmazonLoginStart` (nile's per-attempt
  `client_id`/`code_verifier`/`serial` in a `SecretString`, read only by `auth.rs`),
  `meta` (`AmazonMeta`) and `row` (`amazon_row` and the JSON5 `fuel.json` reader
  `read_fuel`; every Amazon row is a Windows version with its prefix at
  `<install dir>/prefix`). `AmazonService::run` is every nile run's one path: a nile
  that can't start (its glibc 2.35 floor) fails with `NILE_SYSTEM_TOO_OLD`. `auth`
  (`auth --login --non-interactive` for the URL and PKCE values after deleting nile's
  saved login, `register` with the code on argv, `auth --status`, `auth --logout`),
  `list` (nile's own `library.json` is the only owned list, deduplicated by product id
  and ASIN; `installed.json`; `install --info`; `list-updates`), `install` (install with
  `--path <the game's folder>` exactly, `install::folder_name` as nile names it; update,
  verify, uninstall; after the row is written, fuel.json's `PostInstall` steps, read by
  `row::read_post_install`, run through `stores::redist` when `redist::steps_due` says
  so, against `AmazonMeta.post_install_ran`) and `launch` (`launch --json
  --no-wine` gives the five FUEL variables; `launch_error` gives GRID's words). Every parser ignores unknown JSON keys and non-JSON stdout lines. Nothing here may log or forward a store tool's stdout or
  stderr. `redist` is store-neutral: a game's redistributable steps (`RedistStep`), for
  Amazon now and GOG and Epic next. It skips what is already installed (`redist_of`
  maps an id or a file name to a known redistributable; `already_installed` checks the
  Windows registry, or real, non-Wine-stub system files in the host or the prefix;
  `.grid-redist.json` in the prefix notes the steps that worked there). Then it runs the
  rest: under the compat tool in the game's prefix (`step_command`, through
  `launch::native::build_installer_command`), or on a Windows host in one elevated
  PowerShell runner whose result file it reads back (`runner_script`,
  `elevated_command`, `parse_results`). It reuses `offline::exec`'s spawner, supervisor
  and prefix kill. Every problem is a warning line (`REDIST_DECLINED` for a declined
  prompt), never an error. `setup_steps` is the store-neutral "run only the setup steps"
  job (`StoreOp::SetupSteps`, kind `store_setup`) behind Retry on a setup-step warning: a
  store gives it the game row and a `SetupStepsStore` (re-read the step list, record it
  once every step worked), and it runs them through `redist::run_steps` with no download.
  The app's `StoreBackend::setup_steps_job` builds it (Amazon now; the default refuses)
  and the `store_setup_steps` command guards it (installed, not running, no other live
  job, no backup). `steam` is Steam (Phase 4, built 2026-11-14): no tool, no login window, no
  GRID-run job. `mod.rs` holds `SteamEndpoints`, `SteamId`, `SteamApiError` (whose words
  never carry a URL or a status code), the owned-list, `GetItems`, client, account and
  manifest types (`ManifestRead`: found, unreadable while Steam writes it, missing),
  `SteamLink` and `SteamService`: the Web API in `api.rs` (`GetOwnedGames`,
  `GetPlayerSummaries`, the keyless `GetItems`; the only file that reads the key, and only
  into a request's query; no redirect followed); roots (native, Flatpak, Snap; Windows
  from `HKCU`/`HKLM`), the executable and `loginusers.vdf` in `paths.rs`; libraries and
  manifests in `local.rs` (`scan_report` keeps unreadable manifests and unplugged
  libraries apart from uninstalls); the `steam://` opener in `client.rs` (clean env, own
  process group, null stdio); and in `watch.rs` the alive check (`registry.vdf`'s
  `RunningAppID`, the `/proc` fallback, the Windows registry) and `InstallWatch`, the one
  pure install-watch step the app drives (unreadable keeps the last progress; two polls
  in a row without an installing manifest end it Cancelled; no manifest within the start
  timeout ends it Cancelled; `resumed()` for an install Steam started itself). The e2e
  override (`with_roots_override`) reads only the roots it is given. `filter` (games vs tools: the denylist, the name and
  folder patterns, the store type; the `StateFlags` phase and progress), `meta`
  (`SteamMeta`) and `row` (`steam_row`: no path column is set, so no GRID path that
  removes, moves or backs up files can reach a folder Steam owns; the folder is in
  `store_meta`).
- `romm` — the RomM HTTP client: `RommClient`, platforms, games, ROM detail, firmware,
  byte fetches, the cloud save/state endpoints, and the play-session ingest. All request construction lives here;
  check `openapi.json` before changing any of it. `RommClient::recent_games` is one
  `/api/roms` page by `order_by`; `romm::discover` holds Server › Discover's pure rules
  (the preferred platforms, the New releases window and limit, Recently added without
  the coverflow's games, and its platform filters), which always re-filter and re-sort because the server ignores
  an unknown `order_by`.
- `secrets` — `Credential`, the `SecretStore`/`RaTokenStore`/`SteamKeyStore` traits (the
  RomM credential, the RetroAchievements token and the Steam Web API key: three
  independent keyring items under one service), and `KeyringStore`, the OS keyring
  implementation (`MemoryStore` for tests). Secrets are `SecretString` values that cannot be
  formatted or logged.
- `session` — `SessionManager`: connect, restore, disconnect, and the shared
  `RommClient` and image cache handles the rest of the app reads. `use_offline` turns on
  offline mode; with `Config.offline` set, restore answers `RestoreOutcome::Offline` with
  no keyring read and no probe. A connect, or a retry that connects, clears it.

Data files are compiled in with `include_str!` from the repository root:
`emulator-autoprofiles.json` (`launch/profiles.rs`), `retroarch-core-list.json` and
`romm-platform-cores.json` (`autoconfig/cores.rs`). Editing one of those JSON files is a
rebuild, not a restart.

## `app/src-tauri/src`

The shell owns *when* things run; grid-core owns *what* they do.

- `lib.rs` — startup: logging, the data directory, `SessionManager`, `InstallService`,
  `LaunchService`, the registry, the keyring stores, the Tauri builder, and the hook
  wiring in `.setup()`. Startup order: registry → Python import → args migration →
  library move service (gate closed if a journal exists) and the library root check →
  layout migration and its relinked resync (skipped while the root is unreachable) →
  `relocate::recover` (finishes a Commit) → gates handed to the services → `.setup()`,
  which resumes a Copy or Cleanup in the background.
- `stores_service/` — the stores in the app, one backend per store behind the
  `StoreBackend` trait (`mod.rs`), which has every method a store command calls (login,
  disconnect, tool install, refresh, library, updates, install options and jobs, DLC,
  uninstall, game settings, cloud state and sync, the syncs around a session, art, the
  launch hook, and `launch_watched` for a store whose own client starts the game). A new login window gets its spec from `prepare_login` (by default
  `begin_login` then `login_spec`; Amazon installs nile and asks it for the per-attempt
  login URL first); an open one is only focused. `StoresService` is the facade the app
  holds: `backend(StoreKind)`, `statuses()` (every store, in the frontend's
  `STORE_ORDER`: amazon, epic, gog, steam), and the fan-outs (startup, Downloads snapshots, the
  session-finished upload, art). The launch service has one store launch hook slot;
  `install_launch_hook` fills it with `StoreLaunchRouter`, which hands each store row to
  its own backend's `launch_hook()` (Epic and Amazon have one; GOG's rows launch as they
  are). `StoreShared` carries the emitter, the registry, the art client and the
  backends (weakly; `StoreShared::backend(kind)`). `gog.rs` is `GogStore` around grid-core's
  `stores::gog::GogService`: the store status and library the UI reads (owned list cached
  at `<data_dir>/stores/cache/gog-owned.json`), the gogdl install, the cloud sync before a
  launch and after a session, the update check, and the `store-notice` cards for what
  happens in the background (`observe_downloads`; a frontend command's own outcome is
  worded by the frontend, never sent twice); its GOG-only extras (offline installers,
  drive backups) are reached as `StoresService::gog`. `epic.rs` is `EpicStore` around
  `stores::epic::EpicService` (Phase 2, behind the `EpicApi` seam its tests fake): the
  connection state (legendary is installed before the login code is redeemed), the owned
  list and its cache at `<data_dir>/stores/cache/epic-owned.json` (this backend is the
  cache's only writer, in grid-core's format, and deletes it on Disconnect), the
  background `store-notice` cards, the install guards, the cloud syncs around a session
  (gated by `[stores.epic] cloud_saves` and the game's own switch), and the
  `StoreLaunchHook` that adds Epic's launch ticket. The hook owns the one offline retry:
  signed out, a game that may run offline asks for an `--offline` ticket; signed in, a
  network failure retries once with `--offline` for such a game. `amazon.rs` is
  `AmazonStore` around `stores::amazon::AmazonService` (Phase 3, behind the `AmazonApi`
  seam its tests fake): `prepare_login` installs nile when it is missing, then asks it
  for the per-attempt login URL, and keeps nile's `AmazonLoginStart` until
  `run_finish_login`. The connection state (connected in the config but nile has no
  saved login, as after a cancelled "Sign in again", is the expired state), the owned
  list in memory only (read from nile's `library.json` at startup, replaced by each
  `library sync`; GRID writes no copy), the background `store-notice` cards, the install
  guards, nile's glibc floor shown as the tool's error, and the `StoreLaunchHook` that
  adds nile's FUEL environment (not connected or no nile, as after a Disconnect: the row
  launches as it is). No cloud saves (`cloud_state` answers `none`) and no DLC.
  `steam.rs` is `SteamStore` (Phase 4): no tool (an empty `ToolStatus`), no login window
  (`prepare_login` answers "Steam connects in Settings › Stores."), every install,
  update, repair, DLC and uninstall path refused ("Steam installs, updates and uninstalls
  its games itself."), and `launch_watched`, which opens `steam://rungameid/<id>` through
  `SteamService::open` and registers a watched session. It reaches Steam only through
  the `SteamApi` seam (`RealSteam` forwards to grid-core's `SteamService`; the unit tests
  fake it, so no test reaches the network). It owns the Steam registry rows (reconciled
  from the manifests by the 15 s background scan, tools and runtimes never written,
  `outside` unless GRID's own install watch saw it finish), the only writer of
  `stores/cache/steam-owned.json` and `steam-apps.json` (Disconnect deletes both; Remove
  key deletes the owned list), the install watches (grid-core's `InstallWatch` stepped
  every 2 s, each with its store-external Downloads row) and the background
  `store-notice` cards ("Steam connected · N games owned" / the private-connect error after
  `steam_save`, a Refresh's "Steam: N games found · M to install" and its still-private
  error, "<title> installed" · Play, "<title> uninstalled"); the frontend raises the cards
  for the other commands it calls. Its status carries `StoreStatus.steam` (`SteamCard`:
  the client line, the SteamID64, the key's state, the local accounts; never the key). It
  holds the keyring's `SteamKeyStore` slot (`StoresService::real` takes it from
  `lib.rs`). The Steam commands (`steam_test`, `steam_save`, `steam_remove_key`,
  `steam_recheck_client`, `steam_open`) are in `commands/stores.rs`; `steam_open`'s
  install admits the queue's store-external Downloads row
  (`InstallService::admit_store_external`: no slot, no cancel or retry, Steam owns them).
  The UI words every Steam sentence in `app/src/lib/gamestores/steam.ts` (desktop and
  TV), reads `managedByClient` rather than the store's name, and keeps the key out of
  every Svelte store (the desktop field is read once on Test or Save; TV's masked
  keyboard hands it to a plain variable); Settings is `settings/SteamCard.svelte` and TV's
  `tv/stores/TvSteamPage.svelte`. Store updates are keyed
  by `GameKey` in `update_service.rs` (`StoreUpdateSource`, one per store). `store_auth.rs` is the login window: a separate, unprivileged, incognito webview
  whose finish URL (a store's `LoginSpec`: scheme, host, port and path, plus a required
  query pair when `finish_query` is set, as Amazon's `openid.assoc_handle`, so Amazon's
  home page is not the finish) is caught in Rust and refused (the code goes to the
  backend as a `SecretString`). The commands are `commands/stores.rs` (`backend_for`,
  `backend_for_key`) and, for GOG backups, `commands/store_backups.rs`.
- `commands.rs` — also `launch_game_key`: a RomM key takes `launch_game`'s path, a store
  key skips the RomM-only steps (cloud restore, firmware, session stamps) and stamps
  `last_played_at` by key. A store whose backend answers `launch_watched` (Steam) gets
  that watched session; every other store row goes through `LaunchService::launch_key`.
- `commands.rs` (+ `commands/cloud.rs`, `commands/specials.rs`, `commands/updates.rs`,
  `commands/logging.rs`, `commands/setup.rs`, `commands/discover.rs`,
  `commands/backups.rs`) — the `#[tauri::command]` surface. Thin
  wrappers: they build a context, call a service or grid-core, and map the result to a
  serializable DTO. `commands/setup.rs` owns first-run setup and Settings › Systems'
  installs: `start_platform_setup` and `install_for_systems` share one body
  (`queue_setup`: plan, save the waiting defaults, queue), so a Systems pick and a
  first-run pick become the system's default the same way, through the
  emulator-installed hook's `apply_after_install`. `show_emulator_folder` (commands.rs)
  opens an entry's program folder by name; `emulator_companions` (commands.rs) answers
  grid-core's `launch::catalog::companion_links` (each profile's `companions` paired
  with the installed entries that resolve to either side) for Settings › Systems; `install_compat_tool_as_default`
  (specials.rs) records a waiting compat-tool default
  (`grid_core::launch::compat::request_compat_default`) that the compat-tools hook
  applies when the install lands. `commands/discover.rs` owns `discover_feed` (the
  preferred platforms from the saved defaults, two `recent_games` pages, filtered by
  `romm::discover`, plus Recently added's platform filters), `discover_recently_added`
  (Recently added on one platform, asked for when a filter is clicked) and
  `image_dominant_shade` (`ensure_image`'s host filter, then
  `images::shade`). `commands/backups.rs` owns the drive commands (`list_drives`,
  `setup_drive`, `rename_drive`, `set_drive_os`, `remove_grid_files`, `forget_drive`,
  `drive_games`, `drive_asset`, `plan_backup`, `start_backup`, `install_from_drive`,
  `remove_from_drive`, the firmware default). Each names a drive by its key and resolves
  it against the latest scan; none takes a raw path to write to. `plan_backup` and
  `start_backup` take "Copy installed games from this computer" (`use_local`) and the
  per-game picks; they fill each eligible game's out-of-date state from the update set
  when a pass has finished this session, else from each game's details (4 at a time),
  and `start_backup` refuses a plan that still waits on a pick. `install_from_drive`
  also decides what the drive brings beside the game (emulator, Proton, firmware) and
  answers with the notices it words. `use_without_server` (commands.rs) turns on offline
  mode. `ensure_image` checks the image cache before the host filter, so a row installed
  from a drive shows its art whatever server it came from.
- TV mode's native half — `cli.rs` (`--tv`, the only flag read; the mode is never
  persisted), `gamepad/` (the controller reader: `mapping.rs` turns raw input into
  `button` / `stick` / `trigger` events by physical position, `gate.rs` lets input
  through only while TV mode is on and the window is focused (a running game doesn't
  close it, so the Playing page hears the controller while GRID is in front), and sends
  `released_all` when it closes, `identity.rs` names a controller; `mod.rs` owns the
  `gamepad-poll` thread, started the first time TV mode turns on, and the `gamepad`
  event) and `commands/tv.rs` (`get_ui_mode` / `set_ui_mode` with fullscreen,
  `gamepad_list`, `tv_yield_to_game`, `quit_app` and `exit_app`, which leaves
  fullscreen before exiting so the window-state plugin saves the desktop geometry,
  `list_folder` over `grid_core::fsbrowse`, `read_clipboard_text`, and under the `e2e`
  feature `e2e_gamepad_inject`). `lib.rs` feeds the gate from the window's focus events
  and the launch service's session snapshot, and brings GRID back to the front when the
  last game ends in TV mode.
- Services — `cloud_service.rs` (sync caches, auto-restore before launch, auto-upload
  after exit), `firmware_service.rs` (the background firmware triggers and their
  one-job-per-pass guard), `update_service.rs` (when the library is re-checked for server
  updates; never persisted), `images.rs` (startup sweep, replenish job, post-install
  cover prefetch), `media_server.rs` (a loopback HTTP server with Range support that
  feeds cached videos to the webview), `app_update.rs` (one `releases/latest` check per
  process, banner only, nothing downloaded), `python_import.rs` (the startup presence
  check for a Python-era config), `config_write.rs` (one config writer at a time, so two
  load-modify-save cycles cannot lose an update), `logging.rs` (the tracing filter and
  the `debug_prints` toggle),
  `drive_service.rs` (backup drives: the 2 s scan on the blocking pool, the
  `drives-changed` event when the snapshot changes, the remembered list in config,
  leftover cleanup when a drive appears, `abort_drive` when one disappears, and the
  drive firmware a drive install waits to place once the emulator-installed hook has
  applied the defaults),
  `play_session_service.rs` (play activity: queues a finished session in the registry
  outbox and flushes it on session end and after connect, restore or retry; offline, the
  queue waits), `relink_service.rs` (after connect, restore or retry: the relink guard
  `guard_foreign_rows`, then the relink pass, the platform-slug backfill, then the image replenish and the update recompute; `installed-changed` tells the frontend),
  `library_move_service.rs` (9b: when the library move runs — plan, Downloads row, close
  the `LibraryGate`, wait for launch tickets, re-check what runs, start, run on the
  blocking pool with a throttled row detail; cancel, resume, finish events; the startup
  root check and Commit recovery; the unreachable-library state and Relocate; its
  commands are `commands/library_move.rs`). The gate is created once in `lib.rs` and
  shared with `InstallService`, `LaunchService`, `CloudService` and `FirmwareService`;
  commands that write into the library call `commands::ensure_library_idle` first.

**Hooks pattern.** grid-core services expose setters — `InstallService`'s
`set_game_finalized_hook`, `set_emulator_installed_hook`, `set_image_hook`,
`set_compat_tools_hook` (it also applies a waiting compat-tool default), and
`LaunchService::add_session_finished_hook` (a list: cloud
auto-upload and play activity each add a listener). `lib.rs`
installs closures on them at startup, and each closure body stays trivial: it hands the
work to an app-layer service. That is how a finished install can trigger a firmware pass,
a cover prefetch and an update recompute without grid-core knowing any of them exist.

**The `e2e` cargo feature.** `app` builds with an embedded WebDriver server behind the
`e2e` feature, and redirects forge requests to `GRID_LAUNCHER_E2E_FORGE_BASE` at request
time. Every other URL stays real. The feature turns on `grid-core/e2e`, where
`GRID_LAUNCHER_E2E_DRIVE_ROOTS` replaces the removable-drive probe
(`backup::drives::scan`): every child folder of its roots, dot-folders excepted, is a
mounted drive, and an optional `.e2e-drive.json` inside it fakes the label, file system,
size or read-only flag. A release build has no such override. CI clippy-checks that
feature separately.

## `app/src`

Svelte 5 with runes. `main.ts` mounts `App.svelte`, which shows `Connect.svelte` until a
session exists and `Shell.svelte` afterwards.

- `lib/lifetime.svelte.ts` — `startShellLifetime()`: the listeners and loads that live as
  long as a shell is on screen, whichever shell it is — the notification cards
  (`notifications/notifier.ts`), the emulator notices (`systems/events.ts`), the
  `cloud-upload-finished` / `cloud-restore-notice` cards, the installed-registry load and
  the background-art seed. `App.svelte` starts it while the phase is `shell` and the
  unreachable-library dialog is not blocking, so each event shows one card.
- `lib/stores/navigation.svelte.ts` — mode-agnostic navigation intents: `navigation.go(view)`,
  `goSettings(page, { system })`, `openDetails(subject)`. The mounted shell registers the
  handler (`registerNavigation`); code that is not a view (the setup card's Open
  Downloads, a card's action) navigates through it instead of holding the shell.
- Shared game logic, called by the views and by any second UI:
  `lib/games/quick.ts` (pure: the quick action and its label for a game in its context —
  Play, Install, Install from drive, Locate…, Queued, Installing… or none — and which main
  button group Details shows) and `lib/games/run.ts` (effectful: play, install from the
  server or a drive, cancel, update, install content, uninstall, remove from library,
  locate, link, stop; each sends its cards and refreshes the registry, and answers an
  outcome — `ok`, `needs-umu`, `needs-library-folder` or `refused` — for the screen to
  show its own prompt). `lib/server/catalog.svelte.ts` (`Catalog`, one per Server screen:
  platforms, the selected platform's games, launch defaults, firmware status, the library
  folder check, and the events that refresh them). `lib/discover/feedStore.svelte.ts` (the
  last Discover feed per server, kept for the app's life, and the per-platform Recently
  added lists). `lib/downloads/actions.ts` (a Downloads row's Cancel, Retry and Dismiss,
  including a library move's own commands).
- Shared screen logic, the rest of what a second UI needs: each module holds a screen's
  data, its derived state and its commands, and the component is a thin view that adds
  layout, focus and two-click confirms. The classes take getters for their inputs, and
  their loaders read those inputs before their first await, so a view calls each loader
  from its own `$effect`.
  `lib/details/detailsStore.svelte.ts` (`DetailsStore`: the ROM detail overlay, launch
  defaults, the Related filter, the install-block reason, content availability, the
  cloud panel facts, the media failure map, the install state and the main button group,
  and every Details action — install, uninstall, update, cancel, content, play, stop,
  link, locate, remove from library, drive install and remove, Backup to drive),
  `lib/details/linkList.svelte.ts` (the Link to server game list and pick),
  `lib/details/cloudStore.svelte.ts` (`CloudStore`: one saves or states panel — records,
  upload, restore and delete with the restore-all offer, native save folders),
  `lib/details/nativeSettingsStore.ts` (Game Settings load, save, reload from server),
  `lib/systems/systemsStore.svelte.ts` (`SystemsStore`: Settings › Systems' data and
  commands, including firmware and PS3 firmware, and Install recommended's running count),
  `lib/emulators/formSave.ts` (the settings sheet's profile autofill and save),
  `lib/settings/libraryChange.svelte.ts` (`LibraryChange`: the change-library steps,
  folder check, previews, Move and Start fresh), `lib/settings/raActions.ts` (the
  RetroAchievements commands; the secrets stay in the caller's fields),
  `lib/backups/backupPlan.svelte.ts` (`BackupPlanStore`: the per-drive plans, picks and
  Start), `lib/backups/driveActions.ts` (a drive's set up, rename, computers, GRID files,
  folder, forget), `lib/setup/chooseSystems.svelte.ts` (`ChooseSystems`: first-run rows,
  ticks, the Proton row, Install, Skip), `lib/connectFlow.ts` (Connect's library folder,
  Use without a server, the connect) and `relocateTo` in
  `stores/libraryUnreachable.svelte.ts` (the unreachable-library dialog's Relocate).

- `lib/api.ts` — the single `invoke` boundary. Every command wrapper and every payload
  type lives here, and the type field names mirror the Rust structs' serde output
  exactly, with no cosmetic renames.
- `lib/gameKey.ts` — `gameKey(row)` and `parseGameKey`, the TypeScript mirror of
  grid-core's `GameKey` (`launchGameKey`, `GameSession.key`).
- `lib/stores/*.svelte.ts` — rune-based state shared across views: `session`,
  `installed`, `downloads`, `sessions`, `updates`, `appUpdate`, `uiSettings`, `toasts`,
  `drives` (the latest drives snapshot from `list_drives` and `drives-changed`, and the
  `backup-notice` cards; its reads are pure functions in `lib/backups/drives.ts`),
  `inputMode`, `lastViewed`, `compatTools`, `pythonImport`, `libraryMove` (the 9b move status and its toasts),
  `libraryUnreachable` (while it says unreachable, `App.svelte` shows only
  `LibraryUnreachableDialog.svelte`, whose pure half is `lib/libraryUnreachable.ts`).
- Views — `Library.svelte`, `Server.svelte`, `Downloads.svelte`, `Settings.svelte`, plus
  `Details.svelte` as a sub-view. Server opens on Discover (`lib/discover/`:
  `Discover.svelte` mounted by `Server.svelte` in place of the folding header and grid,
  with `Coverflow`, `RecentlyAdded`, `Shot` and `GameLogo`; the pure halves are
  `feed.ts` — action label, dates, coverflow geometry, the picture and logo colours —
  `rotation.ts`, Recently added's pausable 10 s timer, and `filters.ts`, its platform
  filter pills; its data is `feedStore.svelte.ts`). Server still owns Details and the
  install/launch prompts (through `games/run.ts`); Discover calls back into it. `shell.ts` holds the view list (its order is the
  `Ctrl+1`..`Ctrl+4` accelerators) and the session-phase mapping. Emulators live in
  Settings › Systems; `Shell.svelte` routes the Server header's chips there with
  `Settings.show('systems', { system })`.
- Game backups — `lib/backups/`: `BackupDialog.svelte` (one instance, mounted by
  `Shell.svelte`, opened through `dialogState.svelte.ts`) with `dialog.ts`;
  `DriveCard.svelte` (Settings › Backup drives, page `lib/settings/BackupDrivesPage.svelte`)
  with `drives.ts`; `SelectionBar.svelte` with `selection.ts` (select mode on the Server
  and Library grids); `notices.ts` (one card per backup batch, keyed by
  `DownloadEntry.batch`, fed by `notifications/notifier.ts`); `install.ts` (Install from a
  drive and its notices); `driveArt.ts` (a drive asset as a `gridpack-asset:` URL that
  `drive_asset` copies into the image cache). Library's drive rows are `lib/library/rail.ts`
  and `lib/library/drive.ts`; Details' drive actions are in `lib/details/actions.ts` and
  `subject.ts`. Offline mode is `session.offline` (`stores/session.svelte.ts`) and the
  `initialView`/`chipLabel` rules in `shell.ts`. `initialView` also applies Open GRID on
  (`[ui] open_on`, labels and normalisation in `lib/settings/openOn.ts`), the start rule
  shared by the desktop and TV mode; `App.svelte` mounts the shell only once
  `uiSettings.loaded`.
- Feature folders — `lib/backups/`, `lib/discover/`, `lib/details/`, `lib/downloads/`, `lib/emulators/`, `lib/settings/`,
  `lib/systems/`, `lib/library/`, `lib/server/`, `lib/cards/`, `lib/focus/`. Each pairs a `.svelte`
  component with a pure `.ts` module holding the logic, and a `.test.ts` beside it. Put
  new logic in the pure module: that is what vitest can reach.
- Card grids — both card grids (Server, and Library for every rail section) are
  `CardGrid.svelte` around `GameCard.svelte`. The grid is windowed: it renders only the
  rows near its scroll container's visible part, and its padding carries the height of
  the rest. `lib/cards/window.ts` owns that arithmetic (row metrics, the rendered range,
  the scroll target for an unrendered card). `lib/imageLoad.ts` bounds the cover loads
  and remembers each url's file, so a card that mounts again shows its cover at once;
  `lib/visibleWarm.ts` warms background art by each card's `data-index`.
- Settings › Systems — `lib/systems/SystemsPage.svelte` (the page, mounted by
  `Settings.svelte`) with `SystemRow`, `EmulatorBlock`, `RowMenu` and `WindowsRow`; the
  pure halves are `rows.ts` (the row model from `setup_platform_options`, the picker
  groups, status, Install recommended, Other emulators, companions under their
  emulator's line), `menuPlacement.ts` (the ⋯
  menu's `placeMenu`) and `proton.ts` (the Windows row's picker). `reveal(system)` opens
  one system's row and scrolls to it. It reuses `lib/emulators/*.ts` (defaults,
  firmware, notes, form, compat tools, the install signal) and `EmulatorForm.svelte` as
  its add/edit settings sheet. `stores/emulatorUpdates.svelte.ts` caches the
  once-per-session emulator update check. `systems/events.ts` holds the app-lifetime
  notice for a RetroArch pick with no core; `lifetime.svelte.ts` registers it once.
- Notifications — `lib/notifications/`: `notices.ts` (pure) diffs successive downloads
  snapshots into queued/installed/failed/cancelled events, words every download,
  install and launch card, and coalesces a burst into one summary card;
  `notifier.ts` wires it to the `downloads` and `sessions` stores (`onSnapshot`,
  `onSessionsSnapshot`), the `emulator-installed` event, and the launch and install
  calls (`launchGame`, `installGame`), and holds the per-entry wording hints
  (`expectQueued`). `lifetime.svelte.ts` registers it once, so each event shows one card
  whichever views are mounted.
- TV mode — `lib/tv/` (SPEC "TV mode"; design `docs/design/system/components/TVMode/`).
  `App.svelte` mounts `lib/tv/TvShell.svelte` or the desktop screens, from
  `stores/uiMode.svelte.ts` (Rust's `get_ui_mode` / `set_ui_mode`; `lib/uiMode.ts` maps
  views to tabs and Settings pages to TV panes), and sets `html[data-ui="tv"]`, which scopes
  every rule in `lib/tv/tv.css`. TV mode reuses the shared stores and screen logic above
  (`games/run.ts`, `DetailsStore`, `SystemsStore`, `BackupPlanStore`, `ChooseSystems`, the
  notifier) and owns only layout, focus and input:
  - `TvShell.svelte` (the phases loading / unreachable / connect / choose / shell, the start
    tab, the welcome card, the theme override), `TvTopBar`, `TvHintBar`, `TvStage` +
    `ScreenFrame` (one screen or page at a time), `TvToasts` + `notices.svelte.ts` (the
    cards and the session's history; pure rules in `notices.ts`), `TvBackground`.
  - `nav.svelte.ts` — the tab, the Settings pane, the page stack, overlay layers, select
    mode, focus memory per screen and the kept scroll; `screen.svelte.ts` (`useScreen`: a
    screen's hints and its X / Y / LB / RB / LT / RT / Start / View hooks).
  - `focus/` — the focus engine: `engine.ts` (pure spatial moves over a snapshot),
    `reveal.ts` (every scroll; nothing uses the browser's own scrolling), `grid.ts` and
    `windows.ts` (windowed grids), `dom.ts` (measures), `actions.ts` (`use:tvf`).
  - `input/` — `router.svelte.ts` feeds `press.ts` from the `gamepad` event, the keyboard
    and the mouse; `buttons.ts` (keyboard map), `controllers.ts` (Xbox / Nintendo style per
    pad), `prompts.ts` + `glyphs/` (the prompt sets, Kenney's CC0 art), `repeat.ts`,
    `sticks.ts`.
  - `overlays/` (list, confirm, on-screen keyboard with `osk.ts`, Menu with `menu.ts`),
    `tabs/` (`server/`, `library/`, `downloads/`, `settings/` with the pure `panes/*.ts`),
    `pages/` (Details and its tabs, Playing, the media viewer, Game settings, Link, the
    folder browser, Search everything, Notifications, the System page and emulator form,
    the Backup page and the drive pages over `TvRowsPage`; each with a pure `.ts`),
    `startup/` (Connect, Choose your systems, the unreachable library, the setup card),
    `ui/` (primitives: `TvButton`, `TvCover`, `TvGrid`, `TvRail`, `TvRows`, `TvTrack`,
    `TvSelBar`…), `select.svelte.ts` (select mode), `size.ts` (the root font rule),
    `games.ts`, `gameActions.ts`, `backupActions.ts`, `systemsTv.svelte.ts`.
  Rust reads the controller (`gamepad/`, above); the webview's Gamepad API is not used.
- `lib/icons.ts` — hand-authored 24×24 icon paths. The old app's `assets/svg/` files are
  deliberately not reused.

- `lib/gamestores/` — Game stores (GameStores README). Pure `.ts` modules shared by both
  modes: the rail group (`rail.ts`), cover tags (`cover.ts`), Details lines
  (`details.ts`), Downloads lines (`downloads.ts`), the cards' words (`notices.ts`),
  `storeArt.ts` (a store image's `src` through `ensure_image`'s store branch: the CSP
  refuses web images) and the rest; `desktop/` holds the desktop's store dialogs, mounted
  once by `StoreLayer.svelte`, which also shows `store-notice` and sign-in cards while
  the desktop shell is up. `lib/stores/gameStores.svelte.ts` owns the store status and
  libraries for either shell. `lib/settings/StoresPage.svelte` is Settings › Stores.
  `lib/tv/stores/` is TV mode's side (TVStores README): its rail folders, pages, store
  cards (`storeEvents.ts`, started by the TV shell) and TV-only words (`storeWords.ts`).
  Store rows never appear as a Library platform (`library/rail.ts`).

## `e2e`

`scripts/e2e.sh` builds an e2e-feature binary and runs WebdriverIO specs against it and a
mock RomM server. Structure:

- `scripts/e2e.sh` `STAGE_GROUPS` — the ordered list of `name:spec [spec...]` groups. One
  group = one fresh data directory and one mock server; each spec inside it is a separate
  `wdio run`, which is how a "relaunch the app" pair (`connect-restore-a/-b`) works.
- `e2e/specs/` — the specs. `e2e/fixtures*/` — per-group server fixtures.
  `e2e/seed/` — pre-seeded config, registry rows and stub binaries.
  `e2e/helpers/`, `e2e/wdio.conf.ts` — per-stage settings read from the environment
  (`E2E_SPEC`, `E2E_DATA_DIR`, `E2E_MOCK_URL`).
- `e2e/mock-romm/server.mjs` — the mock RomM server (fixtures, archive generation,
  throttling, an offline switch, a request-introspection endpoint).
  `e2e/mock-romm/mock-forge.mjs` — stands in for GitHub and a direct-download provider,
  so catalog scraping runs against genuine markup.
- TV mode — a group whose name starts with `tv-` starts the app with `--tv`
  (`E2E_APP_ARGS`; a spec's `// e2e-app-args:` line overrides it). `e2e/helpers/tv.ts`
  drives a controller through the e2e-only `e2e_gamepad_inject` and holds the layout
  checks (`assertNoClipping`, `assertMinText`, `assertNoDocumentScroll`, `assertBands`) that
  the `tv-layout` group runs on every screen at 720p, 1080p and 4K.
- Fake drives — the `backups` group sets `E2E_DRIVE_ROOTS` to `<data dir>/media`, which
  `wdio.conf.ts` passes to the app as `GRID_LAUNCHER_E2E_DRIVE_ROOTS`.
  `e2e/helpers/drives.ts` plugs a drive in with `mkdir` and unplugs it by renaming it to
  a dot-folder; `e2e/seed/backups-seed.mjs` seeds each spec.

Usage and exit codes are in `BUILD.md`.

## Library layout

The library root has three folders (plus `.grid-staging/`, where a backup waits between
download and write, and `.grid-restore-<rom id>` in a platform folder while a copy of an
installed game is restored from a drive): `games/<Platform>/` (game installs), `emulators/<Emulator>/`
(an emulator's binaries), and `saves/<Emulator>/<dir>/` (every directory the emulator writes
beside its binary). `<Emulator>` is the sanitized catalog PROFILE name, with no release tag, so
several config entries can share one emulator and one saves folder. The `user_data` entries of a
profile are links into `saves/<Emulator>/<dir>` — created under the emulator's DATA ROOT, the
directory that emulator actually reads: the chosen executable's own directory for every binary
except the PCSX2 Linux AppImage, which roots its data at `<exe dir>/PCSX2` and reads nothing else
(`autoconfig::emulator_data_root`). The links are a relative symlink on unix, an NTFS junction on
Windows, so deleting an emulator's install directory never deletes its saves, settings or
firmware.

`Config::library_layout_version` (0 legacy, 1 the flat-to-folders move, 2 current) gates a
one-shot migration that runs at startup in `lib.rs`, before any service is built. Version 2 adds
`step_data_root_links`: it removes stale links beside a PCSX2 AppImage and relinks under
`PCSX2/`, destination-wins, then `lib.rs` re-runs autoconfig for each relinked entry so the ini
PCSX2 reads carries the new `[Folders] Bios`. The migration itself lives in grid-core
(`library::layout_migration`), is Tauri-free, reference-driven (it moves only the platform
directories the registry and config actually point at), idempotent, and resumable. The path
resolution fallbacks (`library::paths`, registry re-resolution) still read the legacy shape, so a
library stuck at version 0 keeps working. See
`docs/superpowers/specs/2026-09-15-library-layout-v1-design.md` for the full design.

## User state

All of it lives in one directory: `config.toml`, `library-move.json` (only while a library
move runs or waits), `grid-launcher.db` (the SQLite registry
of installed games) and `covers/`. The location is the platform `ProjectDirs`
(`io.github` / `Sixdd6` / `grid-launcher`) directory, unless `GRID_LAUNCHER_DATA_DIR` is
set and non-empty, in which case everything moves under that path. The e2e harness relies
on that override; nothing else should.

Game files themselves go to the user-configured library path, not here.

## Security rule (normative)

Tokens and passwords live **only** in the OS keyring, and in memory only as redacting
types (`SecretString` / `Credential`). They must never reach a config file, a log line at
any level, an error message, an IPC payload, a test fixture, or console output.

One narrow exception (AGENTS.md "Secrets"): the external store tools legendary (Epic),
heroic-gogdl (GOG) and nile (Amazon) keep their own token files, and only under
`<data_dir>/stores/{legendary,gogdl,nile}/` (folder 0700, files 0600, deleted on
Disconnect). GRID never reads those tokens into its own types, never logs or forwards the
tools' stdout or stderr, and redacts an Epic exchange code in a launch argv
(`NativeLaunch::redact`). GRID's own secrets, the Steam Web API key included, stay in the
keyring. The hygiene script refuses a log or print macro in the store code that mentions
stdout or stderr, and allows only the fake markers `FAKE-STORE-TOKEN-not-real` and
`FAKE-STEAM-KEY-not-real` in store fixtures. For Steam it also refuses the keyring account
name outside `secrets.rs`, a key-named field (`steam_api_key`, `web_api_key`,
`steam_key`) anywhere else, and a format or log macro in the Steam code that names a URL
and the key together. reqwest, hyper, hyper-util and h2 log no request URL at any level
(`app/src-tauri/tests/steam_key_logging.rs` checks it at `trace`), but a
`reqwest::Error`'s text carries the full URL: Steam's errors go through `without_url()`.

`scripts/check_secret_hygiene.sh` enforces the mechanical half: `expose_secret()` is
allowed only at a fixed list of call sites, and committed fixtures must not contain
anything that looks like a real bearer token. The judgement half is on you — a new log
line that formats a URL, a header, or an error body is the usual way this rule breaks.

## Bundled tools

`app/src-tauri/tauri.windows.conf.json` ships `assets/tools/7z/` (7z.exe, 7z.dll, License.txt) as Tauri resources on Windows only. The NSIS installer places them beside the executable, where `library::extract::bundled_7z_windows_path` finds them before falling back to a 7-Zip on `PATH`. The AppImage carries no bundled tools.
