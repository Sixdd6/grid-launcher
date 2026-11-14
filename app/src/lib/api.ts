import { invoke } from '@tauri-apps/api/core';
import { toLaunchFailure } from './launchError';

// Field names below mirror the Rust structs' actual serde output exactly
// (see crates/grid-core/src/session.rs and romm/mod.rs) — no
// cosmetic renames.
export type SessionState = { connected: boolean; username: string; server_url: string };
/** Which kind of credential the keyring holds — never its value. */
export type AuthKind = 'token' | 'basic';
/** Payload of `SESSION_UNAUTHORIZED_EVENT` (`grid_core::session::SessionUnauthorized`). */
export type SessionUnauthorized = { server_url: string; username: string; auth_kind: AuthKind };
export type RestoreOutcome =
  | { kind: 'no_session'; server_url: string; username: string }
  | { kind: 'connected'; state: SessionState }
  | ({ kind: 'unauthorized' } & SessionUnauthorized)
  | { kind: 'unreachable'; server_url: string; username: string; error: string }
  /** `Config.offline`: the user chose to run without a server; no probe ran. */
  | { kind: 'offline'; server_url: string; username: string };
/** Emitted once when a live session gets a 401 (commands.rs `SESSION_UNAUTHORIZED_EVENT`). */
export const SESSION_UNAUTHORIZED_EVENT = 'session-unauthorized';
export type Platform = {
  id: number;
  name: string;
  slug: string;
  rom_count: number;
  /** User-set name from RomM's platform settings; `null` when unset. */
  custom_name: string | null;
  /** What RomM's web UI shows: the custom name when set, else `name`.
   *  Older servers omit it, so it can come back empty. */
  display_name: string;
};
/** One platform a batched emulator/core lookup is asking about. Field names
 *  match the backend's `PlatformRef` (app/src-tauri/src/commands.rs). */
export type PlatformRef = { name: string; slug: string };

/** The three card sizes design §5 offers, in `ui.card_size_*`. */
export type CardSizeName = 'small' | 'medium' | 'large';

/** Desktop shell appearance, mirroring `grid_core::config::UiSettings`. */
export type UiSettings = {
  theme: 'system' | 'dark' | 'light';
  background_fade: number;
  /** Blur sigma for the background variant, 0-40 (`ui.background_blur`). */
  background_blur: number;
  card_size_library: CardSizeName;
  card_size_server: CardSizeName;
  /** "Game labels": `always` or `hover` (`ui.card_labels`). */
  card_labels: 'always' | 'hover';
  /** "Game card display": `rows` (Equal height) or `columns` (Equal width)
   *  (`ui.card_display`). */
  card_display: 'rows' | 'columns';
  /** TV mode's own "Card size", one for every TV grid (`ui.tv_card_size`).
   *  Apart from `card_size_library` / `card_size_server`. */
  tv_card_size: CardSizeName;
  /** TV mode's own "Game labels": `always` or `focus`
   *  (`ui.tv_card_labels`). Apart from `card_labels`. */
  tv_card_labels: 'always' | 'focus';
  /** "Hardware acceleration": the webview renders on the GPU. Read at
   *  startup, so a change applies after a restart (`ui.hardware_acceleration`). */
  hardware_acceleration: boolean;
  /** "Open GRID on", shared by desktop and TV mode (`ui.open_on`). */
  open_on: OpenOn;
  /** TV mode's "Button prompts" (`ui.tv_prompts`). */
  tv_prompts: TvPrompts;
  /** TV mode's "Button assignments": controller key → style
   *  (`ui.tv_assignments`). A controller with no entry uses its layout's. */
  tv_assignments: Record<string, TvAssignment>;
  /** TV mode's welcome card has been shown (`ui.tv_welcomed`): it shows the
   *  first time TV mode opens, then from Menu › Controls. */
  tv_welcomed: boolean;
  /** The video players' own volume, 0–100, shared by desktop and TV mode
   *  (`ui.video_volume`, default 50). Not the system volume. */
  video_volume: number;
};
/** Where GRID opens: `automatic` (Server › Discover when connected, else the
 *  Library), or forced to `discover` or `library`. */
export type OpenOn = 'automatic' | 'discover' | 'library';
export type TvPrompts = 'match' | 'xbox' | 'ps' | 'nintendo' | '8bitdo';
export type TvAssignment = 'xbox' | 'nintendo';
/** A connected controller (`gamepad::identity::PadInfo`). */
export type PadInfo = {
  /** This connection's id; every `gamepad` event carries it. */
  id: number;
  /** The model's key (gilrs UUID, 32 hex digits): Button assignments are
   *  stored under it (`ui.tv_assignments`). Two pads of one model share it. */
  key: string;
  name: string;
  vendor: number | null;
  product: number | null;
};
/** A button by physical position. `south` is the bottom face button on every
 *  pad; Button assignments (Xbox / Nintendo style) are applied by the UI.
 *  `left_bumper` / `right_bumper` are LB / RB, `left_trigger` /
 *  `right_trigger` LT / RT, `select` View, `start` Menu. `guide` is
 *  reported and bound to nothing. */
export type PadButton =
  | 'south'
  | 'east'
  | 'west'
  | 'north'
  | 'left_bumper'
  | 'right_bumper'
  | 'left_trigger'
  | 'right_trigger'
  | 'select'
  | 'start'
  | 'guide'
  | 'left_stick'
  | 'right_stick'
  | 'dpad_up'
  | 'dpad_down'
  | 'dpad_left'
  | 'dpad_right';
/** The `gamepad` event's payload (`GAMEPAD_EVENT`). Input arrives only while
 *  TV mode is on, GRID's window is focused and no game runs; when that stops,
 *  `released_all` says every held button is up. Buttons are sent on change
 *  only (no repeats). Sticks: −1…1, Y positive up, sent when they move by
 *  0.02 or more; triggers 0…1 (`button` down / up at 0.5 is sent too). */
export type PadEvent =
  | { kind: 'connected'; pad: PadInfo }
  | { kind: 'disconnected'; id: number }
  | { kind: 'button'; id: number; button: PadButton; down: boolean }
  | { kind: 'stick'; id: number; stick: 'left' | 'right'; x: number; y: number }
  | { kind: 'trigger'; id: number; trigger: 'left' | 'right'; value: number }
  | { kind: 'released_all' };
/** TV mode's controller events (`gamepad::GAMEPAD_EVENT`); payload `PadEvent`. */
export const GAMEPAD_EVENT = 'gamepad';
/** One synthetic input for `e2e_gamepad_inject` (e2e builds only): a raw
 *  event goes through the same mapping and gate as a real controller. */
export type GamepadRawEvent =
  | { kind: 'pressed' | 'released'; button: PadButton }
  | { kind: 'changed'; button: PadButton; value: number }
  | {
      kind: 'axis';
      axis: 'left_stick_x' | 'left_stick_y' | 'right_stick_x' | 'right_stick_y' | 'dpad_x' | 'dpad_y';
      value: number;
    };
export type GamepadInput =
  | { kind: 'connect'; pad: PadInfo }
  | { kind: 'disconnect'; id: number }
  | { kind: 'input'; id: number; event: GamepadRawEvent }
  | { kind: 'focus'; focused: boolean };
export type UiMode = 'desktop' | 'tv';
export type UiModeInfo = { mode: UiMode; started_with_tv: boolean; fullscreen: boolean };
export type FolderEntry = { name: string; is_dir: boolean };
/** One folder for TV mode's folder browser (`list_folder`). */
export type FolderListing = { path: string; parent: string | null; entries: FolderEntry[] };
export type GameSummary = {
  id: number;
  name: string;
  platform_id: number;
  path_cover_small: string | null;
  path_cover_large: string | null;
  /** Resolved + host-filtered; the Server grid's background art reads these,
   *  and before the detail loads so does the Overview strip and Media gallery. */
  screenshot_urls: string[];
  /** Resolved + host-filtered; the Server grid's background art reads these. */
  fanart_urls: string[];
  /** The server's platform label; the Server search box matches it. */
  platform_display_name: string;
  /** The server's genres for the game; the Server search box matches them. */
  genres: string[];
  /** When the game reached the server, ISO 8601 verbatim; `''` when unknown. */
  created_at: string;
  /** First release date in seconds since the epoch; `null` when unknown. */
  first_release_date: number | null;
  /** The server's clear logo, resolved + host-filtered; `''` when none. */
  logo_url: string;
  /** The ROM's size on the server's disk (`fs_size_bytes`), all files; 0 when unknown. */
  fs_size_bytes: number;
  /** The largest top-level game file, in bytes; `null` when the list carried no files. */
  largest_file_bytes: number | null;
};

/** `discover_feed`: Server › Discover's two lists, already filtered and
 *  sorted by the backend (`grid_core::romm::discover`). */
export type DiscoverFeed = {
  /** The coverflow: preferred platforms, released in the last six months,
   *  newest first, at most 25. */
  new_releases: GameSummary[];
  /** Newest on the server, without the coverflow's games, at most 8. */
  recently_added: GameSummary[];
  /** Recently added's platform filters: the platforms among the newest
   *  additions (less the coverflow's games), most recent addition first. */
  recent_platforms: RecentPlatform[];
};

/** `grid_core::romm::discover::RecentPlatform`: one Recently added filter. */
export type RecentPlatform = {
  id: number;
  /** The platform's label (`Platform::label`). */
  label: string;
};

export type RomFile = {
  id: number;
  file_name: string;
  file_size_bytes: number;
  is_top_level: boolean;
  /** ISO 8601 as the server states it, or `''`. D-UI-10's fallback. */
  last_modified: string;
  /** RomM's file category (e.g. `"update"`, `"dlc"`); `''` for an ordinary game file. */
  category: string;
  /** The file's MD5 as the server states it; `''` when none. */
  md5_hash: string;
  /** The file's SHA-1 as the server states it; `''` when none. */
  sha1_hash: string;
};

/** One Overview "Related" chip. Mirrors `grid_core::romm::RelatedGame`. */
export type RelatedGame = { name: string; kind: string };

export type RomDetail = {
  id: number;
  name: string;
  platform_id: number;
  platform_name: string;
  /** The server platform's slug; nativeness reads it first. '' when the payload has none. */
  platform_slug: string;
  fs_name: string;
  description: string;
  regions: string;
  languages: string;
  tags: string;
  revision: string;
  rating: string;
  genres: string;
  companies: string;
  first_release_date: string;
  franchises: string;
  game_modes: string;
  player_count: string;
  filesize_bytes: number;
  server_updated_at: string;
  files: RomFile[];
  cover_small_path: string;
  cover_large_path: string;
  screenshot_urls: string[];
  fanart_urls: string[];
  youtube_video_id: string;
  video_path: string;
  is_identified: boolean;
  related: RelatedGame[];
  /** How many achievements RomM lists for the ROM; the Achievements tab
   *  shows only when this is above zero. */
  achievement_count: number;
  /** The game's clear logo, resolved + host-filtered; `''` when none. */
  logo_url: string;
};

/** One Achievements tab row. Mirrors `grid_core::romm::achievements::AchievementRow`. */
export type AchievementRow = {
  ra_id: number | null;
  title: string;
  description: string;
  points: number;
  /** Resolved, server-hosted badge URL for the row state (lock badge while
   *  locked); `""` when RomM serves none. Load it through `ensureImage`. */
  badge_url: string;
  earned: boolean;
  /** The unlock date as RomM states it; `null` while locked. */
  unlocked_at: string | null;
  hardcore: boolean;
};
export type AchievementsSummary = {
  total: number;
  earned: number;
  hardcore: number;
  points_total: number;
  points_earned: number;
};
/** `get_achievements`. No username or token: `progress_known` says whether
 *  the RomM account has an RA username. */
export type AchievementsView = {
  progress_known: boolean;
  summary: AchievementsSummary;
  rows: AchievementRow[];
};
/** `sync_ra_username_to_romm`: what happened to the RomM side of an RA
 *  username save. */
export type RaUsernameSync =
  | { kind: 'skipped' }
  | { kind: 'unchanged' }
  | { kind: 'updated'; refreshed: boolean }
  | { kind: 'failed'; error: string }
  | { kind: 'not_connected' };

// Settings › Library (Q2). Mirrors `grid_core::library::path_change` and
// `commands/library_path.rs`.
export type PathChangeRefusal =
  | 'blank'
  | 'not_absolute'
  | 'same_as_current'
  | 'inside_current'
  | 'contains_current'
  | 'not_a_directory'
  | 'not_creatable'
  | 'not_writable'
  | 'install_active'
  | 'game_running'
  | 'cloud_transfer_active'
  | 'library_move_active';

export type LibraryPathCheck =
  | { status: 'ok'; path: string }
  | { status: 'refused'; reason: PathChangeRefusal; message: string };

export type StartFreshGame = { title: string; platform: string; paths: string[]; bytes: number };

export type StartFreshPreview = {
  old_root: string;
  games: StartFreshGame[];
  total_bytes: number;
  /** Paths a normal uninstall would also remove but that lie outside the old folder: left on disk. */
  left_outside: string[];
};

/** A folder an uninstall left on disk because the removal guard refused it
 *  (`grid_core::library::SkippedRemoval`). `reason` is user-facing text. */
export type SkippedRemoval = { path: string; reason: string };

/** What `uninstall_game` and `delete_emulator` report on success. */
export type UninstallReport = { skipped: SkippedRemoval[] };

export type StartFreshOutcome =
  | {
      status: 'switched';
      library_path: string;
      rows_removed: number;
      failures: string[];
      /** Delete only: folders the uninstall guard kept on disk. */
      skipped: SkippedRemoval[];
    }
  | { status: 'refused'; reason: PathChangeRefusal; message: string }
  | { status: 'stale'; message: string };

// Settings › Library › Move existing files (9b) and the unreachable-library
// dialog. Mirrors `library_move_service.rs` and `grid_core::library::
// relocate` / `root_access`.

/** Where a move is. Only `copy` can be cancelled. */
export type MovePhase = 'copy' | 'commit' | 'cleanup';
/** Same drive: one rename. Another drive: copied, verified, then the source removed. */
export type MoveStrategy = 'rename' | 'copy';

export type MovePreviewItem = { name: string; strategy: MoveStrategy; bytes: number; files: number };

export type MovePreview = {
  old_root: string;
  new_root: string;
  items: MovePreviewItem[];
  total_bytes: number;
  /** Bytes on another drive, which are copied. */
  copy_bytes: number;
  /** `copy_bytes` plus the safety margin; 0 for a same-drive move. */
  required_bytes: number;
  /** Free bytes at the new folder; `null` when unknown. */
  free_bytes: number | null;
};

export type MoveRefusalReason =
  | 'layout_not_current'
  | 'old_root_missing'
  | 'target_has_library_folders'
  | 'not_enough_space'
  | 'move_active'
  | 'library_unreachable'
  | 'no_move'
  | PathChangeRefusal;

export type MoveRefusal = { reason: MoveRefusalReason; message: string };

export type LibraryMovePreviewOutcome =
  | { status: 'ok'; preview: MovePreview }
  | ({ status: 'refused' } & MoveRefusal);

export type LibraryMoveStartOutcome =
  | { status: 'started'; new_root: string }
  | ({ status: 'refused' } & MoveRefusal);

export type LibraryMoveResumeOutcome =
  | { status: 'resumed'; new_root: string }
  | ({ status: 'refused' } & MoveRefusal);

export type LibraryMoveCancelOutcome =
  | { status: 'cancelling' }
  | { status: 'cancelled' }
  | { status: 'not_cancellable'; message: string }
  | { status: 'no_move'; message: string };

/** `library_move_status`, and the `library-move-changed` event's payload. */
export type LibraryMoveStatus = {
  /** `null` when no move is in progress. */
  phase: MovePhase | null;
  running: boolean;
  old_root: string;
  new_root: string;
  /** The move was resumed at startup. */
  resumed: boolean;
  /** Why the last run stopped, while the move waits. */
  error: string | null;
};

/** The `library-move-finished` event's payload. */
export type LibraryMoveFinished =
  | { status: 'finished'; new_root: string; report: string[]; old_root_removed: boolean }
  | { status: 'cancelled' }
  | { status: 'failed'; error: string };

export type UnreachableReason =
  | 'missing'
  | 'drive_missing'
  | 'not_a_directory'
  | 'permission_denied'
  | 'unreadable';

/** `library_unreachable_status`, and the `library-unreachable` event's payload. */
export type LibraryUnreachableStatus =
  | { status: 'reachable' }
  | { status: 'unreachable'; root: string; reason: UnreachableReason; message: string };

export type RelocateRefusalReason =
  | 'blank'
  | 'not_absolute'
  | 'not_a_directory'
  | 'no_library_folders'
  | 'library_reachable'
  | 'move_active';

export type LibraryRelocateOutcome =
  | { status: 'relocated'; library_path: string; report: string[] }
  | { status: 'refused'; reason: RelocateRefusalReason; message: string };

export type DownloadStatus =
  | 'queued'
  | 'downloading'
  | 'installing'
  | 'cancelling'
  | 'completed'
  | 'failed'
  | 'cancelled';

// Which half of the install pipeline owns an entry. `'firmware'` is an
// EXTERNAL row: the background firmware installer moves its own bytes, so
// the entry takes no queue slot and reports no progress. `'backup'` is one
// item of a backup to a drive: `downloading` from the server into staging,
// then `installing` while it writes to the drive.
// A store install or update (external game stores) is `job: 'store'`; its
// `source_id` is the product's game key (`gog:1207664643`).
export type DownloadJob = 'game' | 'emulator' | 'firmware' | 'library' | 'backup' | 'store';

// What an entry installs — finer-grained than `job`.
export type DownloadKind =
  | 'base'
  | 'update'
  | 'ps4_content'
  | 'xbox360_content'
  | 'native_update'
  | 'emulator'
  | 'compat_tool'
  | 'firmware'
  // Settings › Library › Move existing files (9b): the one library move
  // row. It takes no queue slot; its Cancel and Retry are the move's own
  // commands. `downloading` while it copies (cancellable), `installing`
  // while it commits and cleans up (not cancellable).
  | 'library_move'
  // Game backups (`job: 'backup'`): one game, one emulator release for one
  // OS, or one platform's firmware written to a drive.
  | 'backup_game'
  // A game copied from this computer's install instead of downloaded.
  | 'backup_game_local'
  | 'backup_emulator'
  | 'backup_firmware'
  // The one `cancelled` row that stands for the waiting backup rows of a
  // removed drive ("13 more items"). Dismiss only, never Retry.
  | 'backup_cancelled'
  // External game stores (`job: 'store'`).
  | 'store_install'
  | 'store_update'
  | 'store_repair'
  | 'store_dlc'
  // Installing from an offline installer file.
  | 'store_installer'
  // A store game copied back from a backup drive.
  | 'store_restore'
  // Only an installed store game's setup steps (redistributables), run
  // again from Retry: no download, no login (`StoreOp::SetupSteps`).
  | 'store_setup'
  // The store tool's own download (gogdl).
  | 'store_tool'
  // An install the store's own client runs (Steam): GRID shows its progress
  // from Steam's files. No slot, no Cancel, no Retry (`queue::admit_store_external`).
  | 'store_external'
  // Game backups: a store game (GOG) written to a drive (`job: 'backup'`;
  // its `source_id` is the game key).
  | 'backup_store_game';

/** What a running job is doing right now (`grid_core::library::queue::JobPhase`). */
export type JobPhase =
  | 'none'
  | 'downloading'
  | 'installing'
  | 'verifying'
  | 'unpacking'
  | 'installer_silent'
  | 'installer_wizard'
  | 'copying_installer'
  | 'running_installer'
  | 'copying_from_drive'
  | 'checksum';

/** Why a failed row failed, when it is worded from parts (`queue::JobFailure`). */
export type JobFailure = {
  code: 'installer_unrecognised' | 'installer_dlc_base_missing' | 'installer_exit' | 'installer_silent_failed';
  base_title: string;
  base_gog_id: string;
  /** The download (or installer) was kept for a retry. */
  kept: boolean;
  /**
   * `installer_silent_failed`: why, in GRID's words, as the end of a sentence
   * ("there isn't enough disk space"); `''` for the other codes.
   */
  reason: string;
};

export type DownloadEntry = {
  id: number;
  job: DownloadJob;
  kind: DownloadKind;
  rom_id: number;
  source_id: string;
  title: string;
  platform: string;
  status: DownloadStatus;
  downloaded_bytes: number;
  total_bytes: number;
  speed_bps: number;
  install_processed_bytes: number;
  install_total_bytes: number;
  error: string;
  /** One line of free text: what a library move is doing ("games/ · 1.2 of 5.6 GB"). Empty for every other row. */
  detail: string;
  /** The drive a backup row writes to, or an install-from-drive row copies from: its mount path, or its drive letter on Windows. `''` for every other row. */
  drive_id: string;
  /** That drive's pack name. `''` when `drive_id` is. */
  drive_name: string;
  /** That drive's volume label. Non-empty on a `game` row: it installs from the drive ("Copying from <label>"). */
  drive_label: string;
  /** The backup batch, for one notification per batch. `0` for none. */
  batch: number;
  /** What the job is doing right now; `'none'` for a job that does not say. */
  phase: JobPhase;
  /** Set on a failed installer row; `null` otherwise. */
  failure: JobFailure | null;
};

export type DownloadsSnapshot = { entries: DownloadEntry[] };

// ── Game backups (plan §1.3; commands in app/src-tauri/src/commands/backups.rs). ──

/** An OS a backup drive carries emulators for. Mirrors `grid_core::backup::pack::PackOs`. */
export type PackOs = 'linux' | 'windows';

/** What GRID found on a connected drive. */
export type DriveStatus = 'ready' | 'not_set_up' | 'read_only' | 'unreadable' | 'newer_format';

/** `os` is `null` for an OS tag this build does not know. */
export type PackSummaryEmulator = { name: string; version: string; os: PackOs | null };

/** A game on a drive, by the server it came from (Details' "On <pack>" line). */
export type PackRomRef = { server: string; rom_id: number };

export type PackSummary = {
  name: string;
  os: PackOs[];
  game_count: number;
  gridpack_bytes: number;
  emulators: PackSummaryEmulator[];
  /** Platform names. */
  firmware: string[];
  /** RFC 3339 UTC. */
  updated: string;
  rom_ids: PackRomRef[];
};

/** A connected drive. `key` is the mount path, or the drive letter on Windows; a pack has no id. */
export type DriveInfo = {
  key: string;
  mount: string;
  label: string;
  /** The display file system name: FAT32, exFAT, NTFS, ext4… */
  fs: string;
  fat32: boolean;
  total_bytes: number;
  free_bytes: number;
  read_only: boolean;
  status: DriveStatus;
  /** Why the drive's index can't be read (`unreadable`), else `''`. */
  problem: string;
  pack: PackSummary | null;
};

/** A set-up drive GRID has seen, keyed by its mount path or drive letter. */
export type RememberedDrive = {
  mount: string;
  name: string;
  label: string;
  fs: string;
  total_bytes: number;
  game_count: number;
  /** RFC 3339 UTC. */
  last_seen: string;
};

/** `list_drives` and the `drives-changed` event payload. `remembered` lists only the drives that are not connected. */
export type DrivesSnapshot = { connected: DriveInfo[]; remembered: RememberedDrive[] };

/** One game on a drive (Library › Backup drives and Details). Paths are relative to `gridpack/`. */
export type DriveGame = {
  rom_id: number;
  server: string;
  name: string;
  platform: string;
  platform_name: string;
  size: number;
  /** The largest single file, for the FAT32 limit. */
  files_max: number;
  cover: string;
  installed: boolean;
  summary: string;
  region: string;
  languages: string;
  tags: string;
  revision: string;
  rating: string;
  companies: string[];
  release_date: string;
  genres: string[];
  game_modes: string[];
  franchises: string[];
  players: string;
  version: string;
  fanart: string;
  logo: string;
  screenshots: string[];
  /** RFC 3339 UTC: when it was backed up. */
  added: string;
  /** A copy of the install on the computer that backed it up, not the server's files. */
  installed_copy: boolean;
  /** The installed copy's version (`"v1.2"`), else `''`. */
  installed_version: string;
  /** The server had newer files when the installed copy was backed up. */
  outdated: boolean;
  /** A Windows game whose saves were copied out of its Wine prefix. */
  saves_copied: boolean;
};

/**
 * What to backup. `platform_ids` (optional) are the platforms the games are
 * on, when the caller knows them: the plan then reads one game list per
 * platform instead of one detail per game.
 */
export type BackupRequest = { rom_ids: number[]; platform_ids?: number[]; title: string };

/** A game a backup plan names without writing it. Mirrors `grid_core::backup::plan::PlanGameRef`. */
export type PlanGameRef = { rom_id: number; name: string };

export type PlanEmulator = {
  name: string;
  os: PackOs[];
  /** `null` when the size is not known before the download. */
  bytes: number | null;
  already: boolean;
  /** Why it goes for fewer computers than the pack, or not at all; else `''`. */
  note: string;
};

export type PlanFirmware = { platform_name: string; files: number; bytes: number; already: boolean };

/** The Steam Linux Runtime a backup can carry. Mirrors `grid_core::backup::plan::PlanRuntime`. */
export type PlanRuntime = {
  /** "Steam Linux Runtime". */
  name: string;
  /** `null` when the size is not known. */
  bytes: number | null;
  already: boolean;
  /** `include_runtime` was set: it is in this backup. */
  included: boolean;
};

/** `plan_backup`'s answer. Mirrors `grid_core::backup::plan::BackupPlan`. */
export type BackupPlan = {
  /** The drive key. */
  drive: string;
  games_to_write: number[];
  skipped: PlanGameRef[];
  too_big: PlanGameRef[];
  emulators: PlanEmulator[];
  firmware: PlanFirmware[];
  /** Why the firmware checkbox is disabled, else `''`. */
  firmware_reason: string;
  /** The "Make Windows games work offline" line: set when the pack is for Linux and Windows games are being written, ticked or not. */
  runtime: PlanRuntime | null;
  bytes_to_write: number;
  /** The drive's free space less what other queued or running backups still hold on it. */
  free_bytes: number;
  /** The drive's space other queued or running backups hold (0 when none). */
  reserved_bytes: number;
  fits: boolean;
  /** Some requested game is installed on this computer (shows "Copy installed games from this computer"). */
  local_available: boolean;
  /** The `use_local` the plan was made with. */
  use_local: boolean;
  /** Where each game being written comes from. */
  sources: PlanSource[];
  /** Eligible local games the server has newer files for. */
  outdated: PlanOutdated[];
  /** Out-of-date local games with no choice yet, while `use_local` is on. */
  needs_choice: number;
  /** The part of `bytes_to_write` copied from this computer. */
  local_bytes: number;
  /** The part of `bytes_to_write` downloaded from the server. */
  download_bytes: number;
};

/** Where a game in a backup comes from. Mirrors `grid_core::backup::plan::SourceChoice`. */
export type SourceChoice = 'local' | 'download';

/** The user's pick for one out-of-date local game. Mirrors `commands::backups::SourcePick`. */
export type SourcePick = { rom_id: number; source: SourceChoice };

/** One game being written and where it comes from. Mirrors `grid_core::backup::plan::PlanSource`. */
export type PlanSource = {
  rom_id: number;
  name: string;
  source: SourceChoice;
  /** Why it downloads although it is installed here, else `''`. */
  note: string;
};

/** An eligible local game the server has newer files for. Mirrors `grid_core::backup::plan::PlanOutdated`. */
export type PlanOutdated = {
  rom_id: number;
  name: string;
  /** `"v1.2"`, or `''` when there is no version tag. */
  installed_version: string;
  server_version: string;
  /** The user's pick, if any. */
  choice: SourceChoice | null;
};

export type BackupQueued = { batch: number; games: number; emulators: number; firmware: number };

/** `set_drive_os`'s answer. `batch` is 0 when nothing was queued. */
export type OsChangeOutcome = { batch: number; queued: number; removed: number };

/** `install_from_drive`'s answer: one toast line per extra item it queued ("DuckStation from Couch kit added to download queue"). */
export type DriveInstallOutcome = { notices: string[] };

/** The `backup-notice` event payload: a toast the backend raised ("PlayStation firmware installed from Couch kit"). */
export type BackupNotice = { message: string; error: boolean };

/** `get_backup_settings`'s answer. */
export type BackupSettings = { include_firmware: boolean };

/** Emitted with a `DrivesSnapshot`, only when the snapshot changes (drive_service.rs `DRIVES_CHANGED_EVENT`). */
export const DRIVES_CHANGED_EVENT = 'drives-changed';

/** Emitted with a `BackupNotice` for a toast the backend decides on (drive_service.rs `BACKUP_NOTICE_EVENT`). */
export const BACKUP_NOTICE_EVENT = 'backup-notice';

export type GameSession = {
  id: number;
  /** The game's identity (`romm:123`, `gog:1207664643`); see gameKey.ts. */
  key: string;
  /** The RomM rom id; null for a store game. */
  rom_id: number | null;
  title: string;
  emulator_name: string;
  started_at: number;
  pid: number;
};

export type SessionsSnapshot = { sessions: GameSession[]; warning: string | null };

/// The backend carries more per-entry state than the form edits: the
/// `source_*` install provenance and the five layer-1 autoconfig fields.
/// They are optional here and passed straight back through `saveEmulator`,
/// so editing an entry never drops them.
/** `check_emulator_update`'s answer (grid-core `launch::forge::VersionCheck`). */
export type VersionCheck = {
  installed_display: string;
  available_display: string;
  up_to_date: boolean;
};

export type EmulatorEntry = {
  name: string;
  path: string;
  args: string;
  source_id?: string;
  source_provider?: string;
  source_owner?: string;
  source_repo?: string;
  source_release_tag?: string;
  source_installed_tag?: string;
  save_strategy?: string;
  ignore_files?: string;
  ignore_extensions?: string;
  save_paths?: string;
  state_paths?: string;
};

export type ProfileSummary = { name: string; args: string };

export type RaStatus = { username: string; token_present: boolean };
export type RaFanOutRow = { emulator: string; changed: boolean };
/** `retroachievements_login`'s answer. Carries no token, by construction. */
export type RaLoginResult = { username: string; fan_out: RaFanOutRow[] };

export type CatalogEntry = {
  name: string;
  source_id: string;
  provider: string;
  owner: string;
  repo: string;
  tag: string;
  installed: boolean;
};

/** One emulator first-run setup can install for a platform
 *  (grid-core `launch::setup::SetupOption`). */
export type SetupOption = {
  source_id: string;
  name: string;
  /** `standalone`: a catalog download. `server`: a build on the RomM
   *  server's Emulators platform. `retroarch` only where a core is known. */
  kind: 'standalone' | 'server' | 'retroarch';
  /** Already configured. */
  installed: boolean;
  /** A server build (`docs/server-emulator-packages.md`); `null` for a
   *  catalog download. */
  server: ServerBuildInfo | null;
  /** An installed server build's entry name (may end " (server)"). */
  entry?: string;
  /** RetroArch, from the catalog or the server: it becomes a default only
   *  when a core for the platform is on disk. */
  needs_core: boolean;
};

/** Where a server build comes from (grid-core `launch::setup::ServerBuildInfo`). */
export type ServerBuildInfo = {
  rom_id: number;
  file_id: number;
  file_name: string;
  /** From the file's `(v…)` tag. */
  version: string;
  /** The platform's slug, or `multi`. */
  folder: string;
};

/** One row of the first-run platform picker (grid-core
 *  `launch::setup::PlatformSetup`). */
export type PlatformSetup = {
  /** The display label: shown, and the key platform defaults use. */
  label: string;
  name: string;
  slug: string;
  rom_count: number;
  /** Standalone first, in recommendation order; RetroArch last. */
  options: SetupOption[];
  /** The pre-selected option's `source_id`; `null` when nothing installable
   *  serves the platform. */
  recommended: string | null;
  /** The core RetroArch would use here; `null` when RetroArch is not offered. */
  core: string | null;
};

/** One ticked platform and the emulator kept or picked for it. */
export type SetupChoice = { platform: string; source_id: string };

/** What happened to one platform's setup default (grid-core
 *  `launch::setup::PendingOutcome`), from `startPlatformSetup` or the
 *  `setup-defaults` event. */
export type SetupOutcome =
  | { outcome: 'assigned'; platform: string; emulator: string; core: string | null }
  | { outcome: 'core_missing'; platform: string; emulator: string }
  | { outcome: 'kept_none'; platform: string };

export type SetupStatus = {
  /** Show the platform picker: a fresh install that never finished setup. */
  needed: boolean;
  /** Platform labels still waiting on their emulator's install. */
  pending: string[];
};

export type SetupStarted = {
  /** The `source_id`s queued for install: catalog `owner/repo`s and server
   *  builds' `romm:<folder>/<emulator>`, each its Downloads row's source. */
  queued: string[];
  /** Defaults applied at once (emulators already configured). */
  applied: SetupOutcome[];
  /** Installs that could not be queued. */
  errors: string[];
  /** The Proton version queued for Windows games (also in `queued`). */
  compat_queued: string | null;
  /** The Proton pick is the default already. */
  compat_set: boolean;
};

/** First-run setup's Windows row (grid-core `launch::compat::SetupCompatChoice`):
 *  a tool on the computer by its path, or a catalog download by source id. */
export type SetupCompatChoice =
  | { kind: 'installed'; path: string }
  | { kind: 'download'; source_id: string };

/** Event: setup defaults applied after an emulator installed. */
export const SETUP_DEFAULTS_EVENT = 'setup-defaults';

export type LaunchDefaults = {
  default_emulators: Record<string, string>;
  retroarch_cores: Record<string, string>;
  launch_args: string;
};

/** Where an installed row came from (registry v11 `source`). */
export type GameSource = 'romm' | 'gog' | 'epic' | 'amazon' | 'steam';

export type InstalledGame = {
  title: string;
  platform: string;
  /** 'romm' for a RomM game, else the store it came from. */
  source: GameSource;
  /** The store's product id; '' for a RomM game. */
  store_id: string;
  /** A RomM game whose archive held a GOG installer that GRID ran (registry v12). */
  installed_by_installer?: boolean;
  /**
   * The server platform's slug (registry v8); nativeness reads it first
   * (`isNativePlatform`). '' for a row the connect-time backfill has not
   * reached yet — then the platform name decides.
   */
  platform_slug: string;
  rom_id: number | null;
  rom_file_name: string;
  archive_path: string;
  extracted_path: string;
  extracted_dir: string;
  multi_file_game_dir: string;
  description: string;
  rating: string;
  genres: string;
  regions: string;
  languages: string;
  tags: string;
  revision: string;
  companies: string;
  first_release_date: string;
  filesize_bytes: number;
  server_updated_at: string;
  installed_at: number;
  cover_small_path: string;
  cover_large_path: string;
  screenshot_urls: string;
  /** Newline-joined, like screenshot_urls; '' for a row installed before schema v5. */
  fanart_urls: string;
  // Native (Windows) launch settings, the PS3/PS4 identifiers, the bundled
  // DLC manifest and the RetroAchievements id. All serialize as plain
  // strings (never null): grid-core stores a blank rather than a NULL.
  native_executable_path: string;
  native_launch_parameters: string;
  native_compat_tool: string;
  native_wineprefix: string;
  native_game_dir: string;
  included_dlc: string;
  ps3_trophy_paths: string;
  ps3_game_id: string;
  ps3_iso_path: string;
  ps4_game_id: string;
  ps4_content: string;
  ra_id: string;
  /** Epoch seconds of the last launch; 0 when never launched through GRID. */
  last_played_at: number;
  /**
   * 9c: the files are not on disk. Set by `list_installed` only (the
   * reconcile pass computes it on every listing; it is never stored), so a
   * row from any other command leaves it out.
   */
  missing?: boolean;
};

// Cloud save/state sync (app/src-tauri/src/cloud_service.rs,
// app/src-tauri/src/commands/cloud.rs). `game` for every cloud
// call below is any `InstalledGame`-shaped object — the Rust command
// only reads title/platform/rom_id/rom_file_name/archive_path/
// extracted_path/description and ignores the rest.
export type SaveType = 'save' | 'state';
export type SaveScope = 'per_game' | 'shared_single' | 'shared_slotted';

export type CloudPanelInfo = { supported: boolean; block_reason: string; scope: SaveScope };

export type CloudMessage = { text: string; severity: 'info' | 'warning' };

export type CloudRecord = {
  id: number;
  file_name: string;
  emulator: string;
  slot: string | null;
  size_text: string;
  absolute_time: string;
  relative_time: string;
  restorable: boolean;
  // Paired with `restorable` either way (fix round 1, FIX 4): a refusal
  // reason when `false`, or the shared-scope notice (possibly absent)
  // when `true` — set it as the Restore button's tooltip in both cases.
  restore_tooltip: string | null;
};

export type UploadReport = {
  uploaded: number;
  total: number;
  failed: string[];
  messages: CloudMessage[];
};

/** `skipped_folders` (G1-Q): top-level folders of the record that were not
 * restored because they could not be matched to the game. */
export type RestoreReport = { ok: boolean; messages: CloudMessage[]; skipped_folders: string[] };

/** One save-location row: `raw` is the stored path (and the value
 * `native_remove_save_path` takes back), `expanded` is that path resolved
 * for this host and the game's wine prefix — the row's tooltip. */
export type NativeSavePathEntry = { raw: string; expanded: string };

/** A native game's save locations: PCGamingWiki rows first, then manual rows. */
export type NativeSavePaths = { pcgw: NativeSavePathEntry[]; manual: NativeSavePathEntry[] };

export type CloudSettings = {
  download_on_launch: boolean;
  upload_on_exit: boolean;
  skip_if_local_newer: boolean;
  upload_delay_seconds: number;
  /** Saves kept per rom and slot; 0 keeps all. */
  retention_limit: number;
};

// Install specials: extra content, native launch settings, compat tools and
// the RPCS3 PS3 firmware button
// (app/src-tauri/src/commands/specials.rs).

/// Which extra content kinds the server lists files for, from
/// `client.rom_detail` — not the registry.
export type ContentAvailability = { update: boolean; dlc: boolean };

export type ContentKind = 'update' | 'dlc';

/// `executable` is the RESOLVED executable (the pinned one when it still
/// exists, else the first candidate), so the form shows what would actually
/// launch. `candidates` are full paths, shallowest first.
export type NativeGameSettings = {
  executable: string;
  parameters: string;
  compat_tool: string;
  wineprefix: string;
  /** The install directory the executable candidates are labelled relative to; '' when none resolved. */
  install_dir: string;
  candidates: string[];
  /** Who set the launch file: 'sidecar' (game.json), 'user' (this dialog), or '' (automatic). */
  launch_source: string;
};

/// `reload_native_launch`'s answer. `outcome` is 'pinned',
/// 'kept_user_choice', 'no_sidecar', 'no_launch' or 'invalid'; `message`
/// is the line the dialog shows.
export type NativeLaunchReload = { outcome: string; message: string };

/// `kind` is `'wine'` or `'proton'`; `source` is `'system'`, `'steam'` or
/// `'managed'`.
export type CompatTool = { name: string; kind: string; path: string; source: string };

/// The per-entry file probes behind the Eden advisory notes
/// (`emulator_facts`). Both `true` for any non-Eden entry, which is the
/// "nothing to advise" answer — see `app/src-tauri/src/commands.rs`.
export type EmulatorFacts = { eden_keys_present: boolean; eden_firmware_present: boolean };

/// One `companions` pairing (grid-core `launch::catalog::CompanionLink`):
/// a profile that names a companion (ShadPS4) and the companion (its Qt
/// launcher), each with the installed entry that resolves to it, if any.
/// `companion_source_id` is what `installEmulator` takes; `null` when the
/// companion has no catalog row on this computer.
export type CompanionLink = {
  parent_profile: string;
  parent_entry: string | null;
  companion_profile: string;
  companion_entry: string | null;
  companion_source_id: string | null;
};

/// The umu-run a Proton launch would use (grid-core `launch::umu::UmuRunStatus`).
/// `source` is `'system'` (on PATH), `'managed'` (GRID's own download, with
/// its `release_tag`) or `'missing'`. `source_id` is the Downloads row
/// `source_id` a managed install runs under.
export type UmuRunStatus = { source: 'system' | 'managed' | 'missing'; path: string; release_tag: string; source_id: string };

export type CompatToolsDto = { tools: CompatTool[]; default_tool: string; umu_run: UmuRunStatus };

/// An RPCS3 install's PS3 firmware (`commands/specials.rs`).
/// `installed` is RPCS3's own state: its `dev_flash` holds `liblv2.sprx`;
/// `version` (`"4.93"`) comes from `dev_flash/vsh/etc/version.txt` and is
/// `null` when unknown. `pup_path` is the downloaded `PS3UPDAT.PUP` beside
/// the executable, or `null`. Settings › Systems offers Install Firmware
/// (handing the PUP to RPCS3's `--installfw`) only when the PUP is present
/// and the firmware is not installed (`systems/ps3Firmware.ts`).
export type Rpcs3FirmwareStatus = { pup_path: string | null; installed: boolean; version: string | null };

/// The Server platform header's firmware chip input (design §6). See
/// `app/src-tauri/src/commands.rs`'s `PlatformFirmwareStatus`.
export type PlatformFirmwareStatus = {
  file_count: number;
  has_default_emulator: boolean;
  /** Every server file is already installed for the default emulator. */
  installed: boolean;
  /** Some server file can land in the default emulator's folders; when
   *  not, Install would place nothing ("Not needed"). */
  available: boolean;
};

/// The `firmware-pass-finished` payload (`app/src-tauri/src/firmware_service.rs`'s
/// `FirmwarePassFinished`). `ok` is false when the pass could not run (no
/// session, unreadable config) or grid-core reported warnings; `failed`
/// counts those warnings (one per file), `0` when the pass never ran. It
/// carries a platform id, a flag and a count only — never a path, an
/// emulator name, or a URL.
export type FirmwarePassFinished = { platform_id: number; ok: boolean; failed: number };

/// Emitted once for every `installFirmwareForPlatform` call, when that call's
/// background pass has ended — including when it ended with nothing to do.
/// The firmware chip's Install button waits on it to re-enable.
export const FIRMWARE_PASS_FINISHED_EVENT = 'firmware-pass-finished';

/// Emitted after `setDefaultCompatTool` and after a managed compat-tool
/// install finalizes in the background. Re-run `listCompatTools` on it.
export const COMPAT_TOOLS_CHANGED_EVENT = 'compat-tools-changed';

/** Emitted once an emulator install or update has written its config entry. */
export const EMULATOR_INSTALLED_EVENT = 'emulator-installed';

export type EmulatorInstalledEvent = {
  name: string;
  /** `false` when the write replaced an existing entry — i.e. an update. */
  fresh: boolean;
  /** The platform labels whose default this install just applied ("Used for <system>"). */
  used_for: string[];
};

/// The `cloud-upload-finished` payload (`app/src-tauri/src/cloud_service.rs`'s
/// `CloudUploadFinished`). `message` is the completion text
/// `upload_completion_message` produced for this run; `failed` is true for a
/// partial upload as well as a total one. Carries no path, URL or token.
export type CloudUploadFinished = { title: string; message: string; failed: boolean };

/// Emitted after every cloud upload that ran — the manual panel button and
/// the auto upload after a game exits. The Shell turns it into one toast.
export const CLOUD_UPLOAD_FINISHED_EVENT = 'cloud-upload-finished';

/// The `cloud-restore-notice` payload (`cloud_service.rs`'s `CloudRestoreNotice`,
/// G1-Q): the pre-launch restore skipped save folders it could not match to
/// the game. `failed` is true when nothing was restored. Folder names only.
export type CloudRestoreNotice = { title: string; message: string; failed: boolean };

/// Emitted at most once per game and record per run. The Shell turns it into
/// one toast.
export const CLOUD_RESTORE_NOTICE_EVENT = 'cloud-restore-notice';

// Server updates (app/src-tauri/src/update_service.rs,
// app/src-tauri/src/commands/updates.rs).

/// One installed game with a newer version on the server. `label` is the
/// button text, already resolved ('Update' or 'Update to v1.2.0').
export type UpdateRow = { rom_id: number; label: string };

/// Emitted after every update-set recompute (connect, install, uninstall),
/// and on disconnect with an empty array — but only when the set was
/// non-empty, so a disconnect from an up-to-date library emits nothing.
export const UPDATES_CHANGED_EVENT = 'updates-changed';

// Launcher self-update (app/src-tauri/src/app_update.rs).

export type AppUpdateNotice = { tag: string; url: string };
/** `app_update_notice`'s payload: the notice, if any, and when the startup
 *  check completed (RFC 3339 UTC) — `null` when it was skipped or failed. */
export type AppUpdateStatus = { notice: AppUpdateNotice | null; checked_at: string | null };
/// Emitted at most once per process when a newer launcher release exists.
export const APP_UPDATE_EVENT = 'app-update-available';

/** `python_import_notice`'s payload: what the one-shot Python-config import
 *  did at startup, or `null` when nothing was imported. Counts only. */
export type PythonImportReport = {
  emulators: number;
  games: number;
  skipped_games: number;
  retroachievements: boolean;
};

/** `layout_migration_notice`'s payload: why the startup library-layout
 *  migration did not finish, or `null` when it succeeded or was skipped.
 *  Paths only — never a credential. */
export type LayoutMigrationNotice = { message: string };

// ── External game stores, Phase 1: GOG (docs/stores-phase1-plan.md; commands in
// app/src-tauri/src/commands/stores.rs). ──

export type StoreSource = Exclude<GameSource, 'romm'>;
export type StorePlatform = 'linux' | 'windows';
export type ToolStatus = {
  /** `''` for Steam, which has no tool. */
  name: 'gogdl' | 'legendary' | 'nile' | '';
  version: string;
  state: 'not_installed' | 'installing' | 'installed' | 'failed';
  size_bytes: number;
  error: string;
  license: string;
  source_url: string;
  done: number;
  total: number;
};
export type StoreStatus = {
  source: StoreSource;
  connection: 'connected' | 'expired' | 'not_connected';
  username: string;
  tool: ToolStatus;
  owned: 'idle' | 'loading' | 'ready' | 'error';
  owned_error: string;
  owned_count: number;
  installed_count: number;
  installed_bytes: number;
  refreshed_at: number;
  cloud_saves: boolean;
  install_dir: string;
  in_rail: boolean;
  last_error: string;
  /** Steam's part of the card; `null` for every other store. */
  steam: SteamCard | null;
};
/** Steam's part of `StoreStatus` (Phase 4). Never the Web API key, nor any part of it. */
export type SteamCard = {
  /** The Steam client line (`/usr/bin/steam`, `Flatpak (com.valvesoftware.Steam)`); `''` = Steam not found. */
  client_path: string;
  steam_id: string;
  key: 'ok' | 'none' | 'bad' | 'private';
  /** Always `''`: a saved key shows as dots only (user decision 2). */
  key_tail: string;
  accounts: { persona: string; steam_id: string; most_recent: boolean }[];
};
/** `steamTest` / `steamSave`'s answer. */
export type SteamTestResult =
  | { kind: 'ok'; owned_count: number }
  | { kind: 'bad_key'; status: number }
  | { kind: 'private' }
  | { kind: 'bad_steam_id' }
  | { kind: 'network' };
export type StoreDlc = { id: string; title: string; download_bytes: number; installed: boolean };
export type StoreGame = {
  key: string;
  source: StoreSource;
  store_id: string;
  title: string;
  installed: boolean;
  cover: string;
  screenshots: string[];
  description: string;
  developer: string;
  genres: string[];
  year: string;
  platforms: StorePlatform[];
  installed_platform: StorePlatform | null;
  download_bytes: number;
  disk_bytes: number;
  version: string;
  update_version: string;
  /** `steam`: installed through GRID's Install in Steam; `steam_outside`: found in Steam's files. */
  origin: 'gogdl' | 'offline' | 'drive' | 'legendary' | 'nile' | 'steam' | 'steam_outside' | null;
  not_supported: string;
  dlc: StoreDlc[];
  cloud: 'off' | 'none' | 'synced' | 'conflict' | 'unknown';
  last_sync: number;
  install_dir: string;
  compat_tool: string;
};
export type StoreLibrary = { status: StoreStatus; installed: StoreGame[]; available: StoreGame[] };
export type StoreInstallOptions = {
  versions: { platform: StorePlatform; recommended: boolean; download_bytes: number; disk_bytes: number }[];
  languages: string[];
  default_language: string;
  dlc: StoreDlc[];
  install_dir: string;
  free_bytes: number;
  volume: string;
  compat_tool: string;
};
export type StoreInstallRequest = {
  key: string;
  platform: StorePlatform;
  language: string;
  dlc_ids: string[];
  install_dir: string;
  compat_tool: string;
};
export type StoreGameSettings = {
  install_dir: string;
  executable: string;
  exe_user_pinned: boolean;
  launch_parameters: string;
  compat_tool: string;
  wineprefix: string;
  cloud_saves: boolean;
};
export type StoreCloudState = {
  state: 'off' | 'none' | 'synced' | 'syncing' | 'conflict';
  synced_at: number;
  locations: string[];
  conflict: {
    local_at: number;
    local_files: number;
    local_bytes: number;
    cloud_at: number;
    /** The cloud files in the game's save locations (GOG's cloud listing);
     *  `null` when the store gives no file list (Epic's `list-saves`). */
    cloud_files: number | null;
    /** Their bytes; `null` when the listing gives no sizes. */
    cloud_bytes: number | null;
  } | null;
};
export type InstallerInspection = {
  path: string;
  kind: 'linux_sh' | 'windows_exe' | 'mac_pkg';
  key: string | null;
  title: string;
  cover: string;
  version: string;
  bytes: number;
  verdict:
    | { kind: 'install' }
    | { kind: 'update'; installed_version: string }
    | { kind: 'dlc'; base_title: string }
    | { kind: 'refused'; title: string; body: string };
  /** Free bytes on the volume it installs to (the new game's folder, the installed game's for an update, the base game's for DLC); `null` when unknown or refused. */
  free_bytes: number | null;
};
export type StoreNoticeAction = {
  kind:
    | 'play'
    | 'open_downloads'
    | 'try_again'
    | 'sign_in'
    | 'install_tool'
    | 'show_updates'
    | 'install_base'
    | 'open_settings';
  key?: string;
  title?: string;
};
export type StoreNotice = {
  source: StoreSource | null;
  message: string;
  detail: string;
  error: boolean;
  action: StoreNoticeAction | null;
};
/** The `store-login` event's payload. Never the login code. */
export type StoreLoginEvent = {
  source: StoreSource;
  outcome: 'connected' | 'cancelled' | 'failed';
  username: string;
  error: string;
};
export type DriveStoreGame = {
  key: string;
  name: string;
  platform: StorePlatform;
  layout: 'installer' | 'installed';
  version: string;
  size: number;
  cover: string;
  installed: boolean;
  added: string;
  summary: string;
};
export type StoreBackupPlan = {
  drive: string;
  rows: {
    key: string;
    name: string;
    layout: 'installer' | 'installed';
    detail: string;
    bytes: number;
    windows_prefix_note: boolean;
    already: boolean;
    refused: string;
  }[];
  bytes_to_write: number;
  free_bytes: number;
  fits: boolean;
};

/** Emitted with `StoreStatus[]` when a store's status changes (commands/stores.rs `STORES_CHANGED_EVENT`). */
export const STORES_CHANGED_EVENT = 'stores-changed';
/** Emitted with a `StoreLoginEvent` when the login window finishes (`STORE_LOGIN_EVENT`). */
export const STORE_LOGIN_EVENT = 'store-login';
/** Emitted with a `StoreNotice` for a toast the backend decides on (`STORE_NOTICE_EVENT`). */
export const STORE_NOTICE_EVENT = 'store-notice';

export const api = {
  connect: (serverUrl: string, username: string, secret: string, useToken: boolean) =>
    invoke<SessionState>('connect', { serverUrl, username, secret, useToken }),
  restoreSession: () => invoke<RestoreOutcome>('restore_session'),
  retryConnect: () => invoke<RestoreOutcome>('retry_connect'),
  disconnect: () => invoke<void>('disconnect'),
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  listGames: (platformId: number) => invoke<GameSummary[]>('list_games', { platformId }),
  getRomDetail: (romId: number) => invoke<RomDetail>('get_rom_detail', { romId }),
  /** Server › Discover's New releases and Recently added. */
  discoverFeed: () => invoke<DiscoverFeed>('discover_feed'),
  /** Recently added narrowed to one platform, leaving out `exclude` (the
   *  coverflow's games); at most 8, newest first. */
  discoverRecentlyAdded: (platformId: number, exclude: number[]) =>
    invoke<GameSummary[]>('discover_recently_added', { platformId, exclude }),
  /** The dominant shade `[r, g, b]` of the screenshot at `url` (computed in
   *  Rust: an `asset://` image taints a canvas in WebKitGTK). */
  imageDominantShade: (url: string) => invoke<[number, number, number]>('image_dominant_shade', { url }),
  getAchievements: (romId: number) => invoke<AchievementsView>('get_achievements', { romId }),
  ensureImage: (url: string) => invoke<string>('ensure_image', { url }),
  /** The `http://127.0.0.1:<port>/…` URL of a server-hosted game video,
   *  fetched through the session client and cached by the backend, then
   *  served by the app's own loopback range server. A URL rather than bytes:
   *  WebKitGTK will not decode media from a custom URI scheme, and renders
   *  every frame of a `blob:` object URL corrupted on the NVIDIA/Wayland
   *  stack (see `media_server.rs`). Rejects with the backend's message. */
  videoUrl: (url: string) => invoke<string>('video_url', { url }),
  /** The 960px-wide background variant of `url`, blurred at `blur` sigma and
   *  built on demand. Same host filter as `ensureImage`; the sigma is part of
   *  the cached file's name, so each level is built once. A failure means
   *  "this URL has no art" — `BackgroundArt` falls through to the next image;
   *  the prefetch just drops it. */
  /** The background art variant for `url` at `blur`: at `fill`'s size (the
   *  window, drawn 1:1) when given, else the small 960px one. */
  ensureBackgroundVariant: (url: string, blur: number, fill?: { width: number; height: number }) =>
    invoke<string>('ensure_background_variant', { url, blur, width: fill?.width ?? null, height: fill?.height ?? null }),
  /** Opens a trailer's YouTube watch page in the system browser (the media
   *  viewer's "Watch on YouTube" button). `videoId` is validated on the
   *  Rust side before it reaches a URL; rejects with "not a YouTube video
   *  id" for anything but an 11-character id. */
  openYoutubeVideo: (videoId: string) => invoke<void>('open_youtube_video', { videoId }),
  /** The loopback relay page that plays a trailer in an iframe (media
   *  server, `youtube_embed_url`): the id is validated in Rust and the
   *  player opens at `volume` (0–100). */
  youtubeEmbedUrl: (videoId: string, volume: number) => invoke<string>('youtube_embed_url', { videoId, volume }),
  installGame: (romId: number) => invoke<void>('install_game', { romId }),
  cancelInstall: (entryId: number) => invoke<void>('cancel_install', { entryId }),
  retryInstall: (entryId: number) => invoke<void>('retry_install', { entryId }),
  /** TV mode's Delete installer on a row whose installer couldn't run silently: deletes the installer files GRID kept and dismisses the row. */
  deleteFailedInstaller: (entryId: number) => invoke<void>('delete_failed_installer', { entryId }),
  dismissDownload: (entryId: number) => invoke<void>('dismiss_download', { entryId }),
  uninstallGame: (romId: number) => invoke<UninstallReport>('uninstall_game', { romId }),
  listDownloads: () => invoke<DownloadsSnapshot>('list_downloads'),
  listInstalled: () => invoke<InstalledGame[]>('list_installed'),
  // Q4: installed rows with no server rom id. `platform` is the row's own
  // platform text; the backend maps it to the server platform(s) it names.
  listServerRomsForPlatform: (platform: string) =>
    invoke<GameSummary[]>('list_server_roms_for_platform', { platform }),
  linkInstalledRow: (title: string, platform: string, romId: number) =>
    invoke<InstalledGame>('link_installed_row', { title, platform, romId }),
  /** Registry row only: never deletes a file. */
  removeFromLibrary: (title: string, platform: string) =>
    invoke<void>('remove_from_library', { title, platform }),
  /** 9c Locate…: point a missing row at the install inside `folder` (anywhere on disk). */
  locateInstalledRow: (title: string, platform: string, folder: string) =>
    invoke<InstalledGame>('locate_installed_row', { title, platform, folder }),
  getLibraryPath: () => invoke<string>('get_library_path'),
  setLibraryPath: (path: string) => invoke<void>('set_library_path', { path }),
  /** The folder the first-run form suggests (`~/Games/GRID`); `''` when the
   *  home folder is unknown. */
  suggestedLibraryPath: () => invoke<string>('suggested_library_path'),
  /** Settings › Library: can `path` become the library folder? Creates nothing. */
  checkLibraryPath: (path: string) => invoke<LibraryPathCheck>('check_library_path', { path }),
  /** What Start fresh › Delete would remove from the current library. Reads only. */
  libraryStartFreshPreview: () => invoke<StartFreshPreview>('library_start_fresh_preview'),
  /** Start fresh › Leave: switch to `path`; the old games leave the library, their files stay. */
  libraryStartFreshKeep: (path: string, expectedOldRoot: string) =>
    invoke<StartFreshOutcome>('library_start_fresh_keep', { path, expectedOldRoot }),
  /** Start fresh › Delete: switch to `path`, then uninstall the old library's games. */
  libraryStartFreshDelete: (path: string, expectedOldRoot: string) =>
    invoke<StartFreshOutcome>('library_start_fresh_delete', { path, expectedOldRoot }),
  /** Move existing files: what a move to `path` would do. Reads only. */
  libraryMovePreview: (path: string) =>
    invoke<LibraryMovePreviewOutcome>('library_move_preview', { path }),
  /** Starts the background move to `path`; its Downloads row shows the progress. */
  libraryMoveStart: (path: string) => invoke<LibraryMoveStartOutcome>('library_move_start', { path }),
  /** Cancels the move while it copies. */
  libraryMoveCancel: () => invoke<LibraryMoveCancelOutcome>('library_move_cancel'),
  /** Retry on a failed move row: runs the waiting move again. */
  libraryMoveResume: () => invoke<LibraryMoveResumeOutcome>('library_move_resume'),
  libraryMoveStatus: () => invoke<LibraryMoveStatus>('library_move_status'),
  libraryUnreachableStatus: () => invoke<LibraryUnreachableStatus>('library_unreachable_status'),
  /** The library now lives at `path`: stored paths are rewritten, no file moves. */
  libraryRelocate: (path: string) => invoke<LibraryRelocateOutcome>('library_relocate', { path }),
  /** Exits the app without changing anything. */
  libraryUnreachableClose: () => invoke<void>('library_unreachable_close'),
  /** Desktop or TV mode, whether GRID started with `--tv`, and fullscreen. */
  getUiMode: () => invoke<UiModeInfo>('get_ui_mode'),
  /** TV: fullscreen, controller reader on. Desktop: reader gated off
   *  (`released_all`), fullscreen off. */
  setUiMode: (mode: UiMode) => invoke<void>('set_ui_mode', { mode }),
  /** The controllers connected now (empty until TV mode first turns on). */
  gamepadList: () => invoke<PadInfo[]>('gamepad_list'),
  /** Playing › Back to the game: minimizes GRID; it returns when the last game ends. */
  tvYieldToGame: () => invoke<void>('tv_yield_to_game'),
  /** Menu › Quit GRID (leaves fullscreen first, then exits). */
  quitApp: () => invoke<void>('quit_app'),
  /** One folder for TV mode's folder browser; `''` starts at home. */
  listFolder: (path: string, files: boolean) => invoke<FolderListing>('list_folder', { path, files }),
  /** The clipboard's text (Connect's View-to-paste). Never log it. */
  readClipboardText: () => invoke<string>('read_clipboard_text'),
  /** Whether the clipboard holds pasteable text (the on-screen keyboard's
   *  R3 Paste hint); only the yes or no crosses IPC. */
  clipboardHasText: () => invoke<boolean>('clipboard_has_text'),
  /** e2e builds only: synthetic controller input. */
  e2eGamepadInject: (events: GamepadInput[]) => invoke<void>('e2e_gamepad_inject', { events }),
  getUiSettings: () => invoke<UiSettings>('get_ui_settings'),
  setUiSettings: (settings: UiSettings) => invoke<void>('set_ui_settings', { settings }),
  /** The `debug_prints` setting: `debug` rather than `info` logging.
   *  `RUST_LOG` overrides it when set. */
  getDebugPrints: () => invoke<boolean>('get_debug_prints'),
  setDebugPrints: (enabled: boolean) => invoke<void>('set_debug_prints', { enabled }),
  /** Opens the configured RomM server in the browser. The URL comes from the
   *  backend's own config read — never from the frontend. */
  openServerPage: () => invoke<void>('open_server_page'),
  /** Reveals the config directory in the desktop file manager. Takes no path. */
  openConfigFolder: () => invoke<void>('open_config_folder'),
  /** Rejects with a `LaunchFailure` (launchError.ts): `code` is `'umu_missing'`
   *  when a Proton launch found no umu-run. */
  launchGame: (romId: number) =>
    invoke<GameSession>('launch_game', { romId }).catch((err: unknown) => {
      throw toLaunchFailure(err);
    }),
  /** Launches any game by its key (`gameKey(row)`): a RomM key takes
   *  `launchGame`'s path; a store key skips the RomM-only steps. Rejects with
   *  a `LaunchFailure` like `launchGame`. */
  launchGameKey: (key: string) =>
    invoke<GameSession>('launch_game_key', { key }).catch((err: unknown) => {
      throw toLaunchFailure(err);
    }),
  stopGame: (sessionId: number) => invoke<void>('stop_game', { sessionId }),
  listSessions: () => invoke<SessionsSnapshot>('list_sessions'),
  listEmulators: () => invoke<EmulatorEntry[]>('list_emulators'),
  emulatorFacts: (name: string) => invoke<EmulatorFacts>('emulator_facts', { name }),
  emulatorCompanions: () => invoke<CompanionLink[]>('emulator_companions'),
  saveEmulator: (originalName: string, entry: EmulatorEntry) =>
    invoke<void>('save_emulator', { originalName, entry }),
  deleteEmulator: (name: string) => invoke<UninstallReport>('delete_emulator', { name }),
  /**
   * Opens the emulator with no ROM, so its own settings UI can be reached.
   * Resolves 500 ms later with the warning text when the process is already
   * gone by then, or `null` when it is still running.
   */
  launchEmulator: (name: string) => invoke<string | null>('launch_emulator', { name }),
  listProfiles: () => invoke<ProfileSummary[]>('list_profiles'),
  matchProfile: (executablePath: string) =>
    invoke<ProfileSummary | null>('match_profile', { executablePath }),
  getLaunchDefaults: () => invoke<LaunchDefaults>('get_launch_defaults'),
  setDefaultEmulator: (platform: string, name: string) =>
    invoke<void>('set_default_emulator', { platform, name }),
  /** Emulator names supporting each platform, keyed by the platform name asked about. */
  compatibleEmulators: (platforms: PlatformRef[]) =>
    invoke<Record<string, string[]>>('compatible_emulators', { platforms }),
  /** Installed libretro cores offered for each platform, keyed by platform name. */
  retroarchCoreOptions: (platforms: PlatformRef[]) =>
    invoke<Record<string, string[]>>('retroarch_core_options', { platforms }),
  setRetroarchCore: (platform: string, core: string) =>
    invoke<void>('set_retroarch_core', { platform, core }),
  listEmulatorCatalog: () => invoke<CatalogEntry[]>('list_emulator_catalog'),
  /** First-run setup: the server's platforms that need an emulator, with
   *  what GRID can install for each and its recommendation. */
  setupPlatformOptions: () => invoke<PlatformSetup[]>('setup_platform_options'),
  setupStatus: () => invoke<SetupStatus>('setup_status'),
  skipPlatformSetup: () => invoke<void>('skip_platform_setup'),
  startPlatformSetup: (choices: SetupChoice[], compat: SetupCompatChoice | null = null) =>
    invoke<SetupStarted>('start_platform_setup', { choices, compat }),
  /** Settings › Systems: installs the picked emulator per system and makes
   *  it that system's emulator when the install finishes (setup's pending
   *  defaults). Replaces an explicit "(none)" on the chosen systems. */
  installForSystems: (choices: SetupChoice[]) =>
    invoke<SetupStarted>('install_for_systems', { choices }),
  /** Opens the folder holding the named emulator's program. */
  showEmulatorFolder: (name: string) => invoke<void>('show_emulator_folder', { name }),
  installEmulator: (sourceId: string) => invoke<void>('install_emulator', { sourceId }),
  /**
   * The row's "Update from Source" check. Runs one release request against
   * the entry's own source, on click and uncached, like the reference.
   */
  checkEmulatorUpdate: (name: string) => invoke<VersionCheck>('check_emulator_update', { name }),
  /** Re-installs the entry's source over its existing install directory. */
  updateEmulator: (name: string) => invoke<void>('update_emulator', { name }),
  setRetroachievementsCredentials: (username: string, token: string) =>
    invoke<RaFanOutRow[]>('set_retroachievements_credentials', { username, token }),
  /**
   * Username + password login. The password crosses IPC once and is stored
   * nowhere; the token it yields goes straight to the OS keyring and is
   * never part of this answer.
   */
  retroachievementsLogin: (username: string, password: string) =>
    invoke<RaLoginResult>('retroachievements_login', { username, password }),
  getRetroachievementsStatus: () => invoke<RaStatus>('get_retroachievements_status'),
  clearRetroachievementsCredentials: () => invoke<void>('clear_retroachievements_credentials'),
  syncRaUsernameToRomm: () => invoke<RaUsernameSync>('sync_ra_username_to_romm'),
  cloudPanelInfo: (game: InstalledGame, saveType: SaveType) =>
    invoke<CloudPanelInfo>('cloud_panel_info', { game, saveType }),
  cloudRecords: (game: InstalledGame, saveType: SaveType) =>
    invoke<CloudRecord[]>('cloud_records', { game, saveType }),
  cloudUpload: (game: InstalledGame, saveType: SaveType) =>
    invoke<UploadReport>('cloud_upload', { game, saveType }),
  /** `restoreAll` (G1-Q): the user confirmed restoring every folder of the
   *  record, including ones not matched to the game. */
  cloudRestore: (game: InstalledGame, saveType: SaveType, recordId: string | null, restoreAll = false) =>
    invoke<RestoreReport>('cloud_restore', { game, saveType, recordId, restoreAll }),
  cloudDelete: (saveType: SaveType, recordId: number) =>
    invoke<void>('cloud_delete', { saveType, recordId }),
  nativeSavePaths: (game: InstalledGame) => invoke<NativeSavePaths>('native_save_paths', { game }),
  nativeAddManualSavePath: (game: InstalledGame, path: string) =>
    invoke<void>('native_add_manual_save_path', { game, path }),
  /** Removes one row from a native game's save-location list, PCGW or manual. */
  nativeRemoveSavePath: (game: InstalledGame, path: string) =>
    invoke<void>('native_remove_save_path', { game, path }),
  cloudSettings: () => invoke<CloudSettings>('cloud_settings'),
  setCloudSettings: (settings: CloudSettings) => invoke<void>('set_cloud_settings', { settings }),
  installContent: (romId: number, kind: ContentKind) =>
    invoke<void>('install_content', { romId, kind }),
  installNativeUpdate: (romId: number) => invoke<void>('install_native_update', { romId }),
  contentAvailability: (romId: number) =>
    invoke<ContentAvailability>('content_availability', { romId }),
  /** Why the Install button cannot install this platform's games, or ''. */
  installBlockReason: (platform: string, platformSlug: string) =>
    invoke<string>('install_block_reason', { platform, platformSlug }),
  nativeGameSettings: (romId: number) =>
    invoke<NativeGameSettings>('native_game_settings', { romId }),
  setNativeGameSettings: (
    romId: number,
    executable: string,
    parameters: string,
    compatTool: string,
  ) => invoke<void>('set_native_game_settings', { romId, executable, parameters, compatTool }),
  /** Re-reads the server's game.json and applies its launch section (no reinstall). */
  reloadNativeLaunch: (romId: number) =>
    invoke<NativeLaunchReload>('reload_native_launch', { romId }),
  listCompatTools: () => invoke<CompatToolsDto>('list_compat_tools'),
  setDefaultCompatTool: (value: string) => invoke<void>('set_default_compat_tool', { value }),
  listCompatToolCatalog: () => invoke<CatalogEntry[]>('list_compat_tool_catalog'),
  installCompatTool: (sourceId: string) => invoke<void>('install_compat_tool', { sourceId }),
  /** Settings › Systems › Windows: installs the compat tool and makes it the
   *  default once its install finishes (at once when already installed). */
  installCompatToolAsDefault: (sourceId: string) =>
    invoke<void>('install_compat_tool_as_default', { sourceId }),
  /** Queues GRID's own umu-launcher (Linux only); a reinstall resolves the newest release. */
  installUmuLauncher: () => invoke<void>('install_umu_launcher'),
  rpcs3FirmwareStatus: (emulatorName: string) =>
    invoke<Rpcs3FirmwareStatus>('rpcs3_firmware_status', { emulatorName }),
  installPs3Firmware: (emulatorName: string) =>
    invoke<boolean>('install_ps3_firmware', { emulatorName }),
  /** `emulator` names the entry to check; omitted, the platform's default emulator. */
  platformFirmwareStatus: (platformId: number, platform: string, emulator?: string) =>
    invoke<PlatformFirmwareStatus>('platform_firmware_status', {
      platformId,
      platform,
      emulator: emulator ?? null,
    }),
  /** `emulator` names the entry to install into; omitted, the platform's default emulator. */
  installFirmwareForPlatform: (platformId: number, platform: string, emulator?: string) =>
    invoke<void>('install_firmware_for_platform', {
      platformId,
      platform,
      emulator: emulator ?? null,
    }),
  cancelDownloadForRom: (romId: number) => invoke<void>('cancel_download_for_rom', { romId }),
  listUpdates: () => invoke<UpdateRow[]>('list_updates'),
  updateGame: (romId: number) => invoke<void>('update_game', { romId }),
  appVersion: () => invoke<string>('app_version'),
  appUpdateNotice: () => invoke<AppUpdateStatus>('app_update_notice'),
  openReleasePage: (url: string) => invoke<void>('open_release_page', { url }),
  pythonImportNotice: () => invoke<PythonImportReport | null>('python_import_notice'),
  layoutMigrationNotice: () => invoke<LayoutMigrationNotice | null>('layout_migration_notice'),

  // Game backups. Every drive command names a drive by its `key` (the mount
  // path, or the drive letter on Windows) from the latest scan.
  listDrives: () => invoke<DrivesSnapshot>('list_drives'),
  /** Writes an empty pack. Refuses a mount the current scan did not find. */
  setupDrive: (mount: string, name: string, os: PackOs[]) =>
    invoke<DriveInfo>('setup_drive', { mount, name, os }),
  renameDrive: (key: string, name: string) => invoke<void>('rename_drive', { key, name }),
  /** Removing an OS works offline; adding one queues emulator builds and needs a server. */
  setDriveOs: (key: string, os: PackOs[]) => invoke<OsChangeOutcome>('set_drive_os', { key, os }),
  /** The bytes "Remove GRID files" would free. */
  removeGridFilesSize: (key: string) => invoke<number>('remove_grid_files_size', { key }),
  removeGridFiles: (key: string) => invoke<void>('remove_grid_files', { key }),
  /** Drops a remembered (not connected) drive; takes its mount, as `RememberedDrive` has no key. */
  forgetDrive: (mount: string) => invoke<void>('forget_drive', { mount }),
  openDriveFolder: (key: string) => invoke<void>('open_drive_folder', { key }),
  driveGames: (key: string) => invoke<DriveGame[]>('drive_games', { key }),
  /** Seeds the image cache from a pack-relative art path; answers the cache path. */
  driveAsset: (key: string, rel: string) => invoke<string>('drive_asset', { key, rel }),
  /**
   * `includeRuntime`: carry the Steam Linux Runtime ("Make Windows games work offline"); omitted, false.
   * `useLocal`: "Copy installed games from this computer"; omitted, true.
   * `choices`: the picks for out-of-date local games (a rom id picked twice keeps the last pick).
   */
  planBackup: (
    key: string,
    request: BackupRequest,
    includeFirmware: boolean,
    includeRuntime?: boolean,
    useLocal?: boolean,
    choices?: SourcePick[],
  ) =>
    invoke<BackupPlan>('plan_backup', {
      key,
      request,
      includeFirmware,
      includeRuntime: includeRuntime ?? null,
      useLocal: useLocal ?? null,
      choices: choices ?? null,
    }),
  /**
   * Same options as `planBackup`. Needs a server ("Backing up needs a server"); refused while an
   * out-of-date local game has no pick ("Choose which copy to backup for 2 games").
   */
  startBackup: (
    key: string,
    request: BackupRequest,
    includeFirmware: boolean,
    includeRuntime?: boolean,
    useLocal?: boolean,
    choices?: SourcePick[],
  ) =>
    invoke<BackupQueued>('start_backup', {
      key,
      request,
      includeFirmware,
      includeRuntime: includeRuntime ?? null,
      useLocal: useLocal ?? null,
      choices: choices ?? null,
    }),
  /** `server` is the game's `DriveGame.server`; omitted, the first game on the drive with that rom id. */
  installFromDrive: (key: string, romId: number, server?: string) =>
    invoke<DriveInstallOutcome>('install_from_drive', { key, romId, server: server ?? null }),
  /** Answers how many games were removed. Refused while the drive has a live backup row. */
  removeFromDrive: (key: string, romIds: number[], server?: string) =>
    invoke<number>('remove_from_drive', { key, romIds, server: server ?? null }),
  getBackupSettings: () => invoke<BackupSettings>('get_backup_settings'),
  setBackupIncludeFirmware: (value: boolean) => invoke<void>('set_backup_include_firmware', { value }),
  /** Offline mode: sets `Config.offline` and drops any live client. */
  useWithoutServer: () => invoke<void>('use_without_server'),
  // ── External game stores (Phase 1: GOG). `key` is a game key unless noted. ──
  storesStatus: () => invoke<StoreStatus[]>('stores_status'),
  storeLibrary: (source: StoreSource) => invoke<StoreLibrary>('store_library', { source }),
  /** Results come as `stores-changed` events. */
  storeRefresh: (source: StoreSource) => invoke<void>('store_refresh', { source }),
  /** Results come as `updates-changed` events. */
  storeCheckUpdates: (source: StoreSource) => invoke<void>('store_check_updates', { source }),
  /** Opens the login window; the outcome is a `store-login` event. */
  storeConnect: (source: StoreSource) => invoke<void>('store_connect', { source }),
  storeCancelLogin: (source: StoreSource) => invoke<void>('store_cancel_login', { source }),
  storeDisconnect: (source: StoreSource) => invoke<void>('store_disconnect', { source }),
  storeInstallTool: (source: StoreSource) => invoke<void>('store_install_tool', { source }),
  /** TV mode's on-screen keyboard: types `text` into the login page. */
  storeLoginType: (text: string) => invoke<void>('store_login_type', { text }),
  storeSetInstallDir: (source: StoreSource, path: string) =>
    invoke<void>('store_set_install_dir', { source, path }),
  storeSetCloudSaves: (source: StoreSource, on: boolean) =>
    invoke<void>('store_set_cloud_saves', { source, on }),
  storeInstallOptions: (key: string) => invoke<StoreInstallOptions>('store_install_options', { key }),
  storeInstall: (request: StoreInstallRequest) => invoke<void>('store_install', { request }),
  storeUpdate: (key: string) => invoke<void>('store_update', { key }),
  storeRepair: (key: string) => invoke<void>('store_repair', { key }),
  storeSetupSteps: (key: string) => invoke<void>('store_setup_steps', { key }),
  storeUninstall: (key: string) => invoke<void>('store_uninstall', { key }),
  storeInstallDlc: (key: string, dlcId: string) => invoke<void>('store_install_dlc', { key, dlcId }),
  storeRemoveDlc: (key: string, dlcId: string) => invoke<void>('store_remove_dlc', { key, dlcId }),
  storeGameSettings: (key: string) => invoke<StoreGameSettings>('store_game_settings', { key }),
  /** The executables in an installed store game's folder, best first, as full paths. */
  storeGameExecutables: (key: string) => invoke<string[]>('store_game_executables', { key }),
  storeSetGameSettings: (key: string, settings: StoreGameSettings) =>
    invoke<void>('store_set_game_settings', { key, settings }),
  storeCloudState: (key: string) => invoke<StoreCloudState>('store_cloud_state', { key }),
  storeCloudSync: (key: string) => invoke<StoreCloudState>('store_cloud_sync', { key }),
  storeCloudResolve: (key: string, choice: 'upload' | 'download') =>
    invoke<StoreCloudState>('store_cloud_resolve', { key, choice }),
  offlineInspect: (path: string) => invoke<InstallerInspection>('offline_inspect', { path }),
  /** Free bytes on the volume holding `path` (its nearest existing folder); `null` when unknown. */
  storeFreeSpace: (path: string) => invoke<number | null>('store_free_space', { path }),
  offlineInstall: (path: string, compatTool: string, installDir?: string) =>
    invoke<void>('offline_install', { path, compatTool, installDir: installDir ?? null }),
  /** `key` is the drive key; `gameKeys` the store games' keys. */
  planStoreBackup: (key: string, gameKeys: string[]) =>
    invoke<StoreBackupPlan>('plan_store_backup', { key, gameKeys }),
  /** `key` is the drive key. */
  startStoreBackup: (key: string, gameKeys: string[]) =>
    invoke<BackupQueued>('start_store_backup', { key, gameKeys }),
  /** `key` is the drive key. */
  driveStoreGames: (key: string) => invoke<DriveStoreGame[]>('drive_store_games', { key }),
  /** `key` is the drive key; `gameKey` the store game's key. */
  installStoreFromDrive: (key: string, gameKey: string) =>
    invoke<void>('install_store_from_drive', { key, gameKey }),
  /** `key` is the drive key. Answers how many games were removed. */
  removeStoreFromDrive: (key: string, gameKeys: string[]) =>
    invoke<number>('remove_store_from_drive', { key, gameKeys }),
  // ── Steam (Phase 4). The key goes to the backend once and is never kept here. ──
  /** One `GetOwnedGames` call; `key` omitted tests the saved key. */
  steamTest: (steamId: string, key?: string) => invoke<SteamTestResult>('steam_test', { steamId, key: key ?? null }),
  /** The test, then (on ok or private) the keyring, `[stores.steam]`, a scan. */
  steamSave: (steamId: string, key?: string) => invoke<SteamTestResult>('steam_save', { steamId, key: key ?? null }),
  steamRemoveKey: () => invoke<void>('steam_remove_key'),
  steamRecheckClient: () => invoke<void>('steam_recheck_client'),
  /** `key` is a Steam game key (`steam:620`). */
  steamOpen: (key: string, action: 'install' | 'uninstall' | 'validate') => invoke<void>('steam_open', { key, action }),
};
