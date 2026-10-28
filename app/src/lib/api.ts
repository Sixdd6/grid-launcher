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
  | { kind: 'unreachable'; server_url: string; username: string; error: string };
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
};
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
// the entry takes no queue slot and reports no progress.
export type DownloadJob = 'game' | 'emulator' | 'firmware' | 'library';

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
  | 'library_move';

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
};

export type DownloadsSnapshot = { entries: DownloadEntry[] };

export type GameSession = {
  id: number;
  rom_id: number;
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

export type LaunchDefaults = {
  default_emulators: Record<string, string>;
  retroarch_cores: Record<string, string>;
  launch_args: string;
};

export type InstalledGame = {
  title: string;
  platform: string;
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

/// The umu-run a Proton launch would use (grid-core `launch::umu::UmuRunStatus`).
/// `source` is `'system'` (on PATH), `'managed'` (GRID's own download, with
/// its `release_tag`) or `'missing'`. `source_id` is the Downloads row
/// `source_id` a managed install runs under.
export type UmuRunStatus = { source: 'system' | 'managed' | 'missing'; path: string; release_tag: string; source_id: string };

export type CompatToolsDto = { tools: CompatTool[]; default_tool: string; umu_run: UmuRunStatus };

/// `pup_path` is the `PS3UPDAT.PUP` beside the RPCS3 executable, or `null`
/// when that install has none. The Emulators panel shows the firmware note
/// and its Install Firmware button only when `pup_path` is set: the button
/// hands the already-downloaded PUP to RPCS3 (Python parity — the download
/// itself is the background firmware job's work, not this button's).
export type Rpcs3FirmwareStatus = { pup_path: string | null };

/// The Server platform header's firmware chip input (design §6). See
/// `app/src-tauri/src/commands.rs`'s `PlatformFirmwareStatus`.
export type PlatformFirmwareStatus = { file_count: number; has_default_emulator: boolean };

/// The `firmware-pass-finished` payload (`app/src-tauri/src/firmware_service.rs`'s
/// `FirmwarePassFinished`). `ok` is false when the pass could not run (no
/// session, unreadable config) or grid-core reported warnings. It carries a
/// platform id and a flag only — never a path, an emulator name, or a URL.
export type FirmwarePassFinished = { platform_id: number; ok: boolean };

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

export const api = {
  connect: (serverUrl: string, username: string, secret: string, useToken: boolean) =>
    invoke<SessionState>('connect', { serverUrl, username, secret, useToken }),
  restoreSession: () => invoke<RestoreOutcome>('restore_session'),
  retryConnect: () => invoke<RestoreOutcome>('retry_connect'),
  disconnect: () => invoke<void>('disconnect'),
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  listGames: (platformId: number) => invoke<GameSummary[]>('list_games', { platformId }),
  getRomDetail: (romId: number) => invoke<RomDetail>('get_rom_detail', { romId }),
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
  ensureBackgroundVariant: (url: string, blur: number) =>
    invoke<string>('ensure_background_variant', { url, blur }),
  /** A game card's letterbox backdrop for a cover URL: a small pre-blurred,
   *  pre-darkened JPEG (`images::backdrop`), built once and cached. Same host
   *  filter as `ensureImage`. */
  ensureCardBackdrop: (url: string) => invoke<string>('ensure_card_backdrop', { url }),
  /** Opens a trailer's YouTube watch page in the system browser (the media
   *  viewer's "Watch on YouTube" button). `videoId` is validated on the
   *  Rust side before it reaches a URL; rejects with "not a YouTube video
   *  id" for anything but an 11-character id. */
  openYoutubeVideo: (videoId: string) => invoke<void>('open_youtube_video', { videoId }),
  installGame: (romId: number) => invoke<void>('install_game', { romId }),
  cancelInstall: (entryId: number) => invoke<void>('cancel_install', { entryId }),
  retryInstall: (entryId: number) => invoke<void>('retry_install', { entryId }),
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
  stopGame: (sessionId: number) => invoke<void>('stop_game', { sessionId }),
  listSessions: () => invoke<SessionsSnapshot>('list_sessions'),
  listEmulators: () => invoke<EmulatorEntry[]>('list_emulators'),
  emulatorFacts: (name: string) => invoke<EmulatorFacts>('emulator_facts', { name }),
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
  /** Queues GRID's own umu-launcher (Linux only); a reinstall resolves the newest release. */
  installUmuLauncher: () => invoke<void>('install_umu_launcher'),
  rpcs3FirmwareStatus: (emulatorName: string) =>
    invoke<Rpcs3FirmwareStatus>('rpcs3_firmware_status', { emulatorName }),
  installPs3Firmware: (emulatorName: string) =>
    invoke<boolean>('install_ps3_firmware', { emulatorName }),
  platformFirmwareStatus: (platformId: number, platform: string) =>
    invoke<PlatformFirmwareStatus>('platform_firmware_status', { platformId, platform }),
  installFirmwareForPlatform: (platformId: number, platform: string) =>
    invoke<void>('install_firmware_for_platform', { platformId, platform }),
  cancelDownloadForRom: (romId: number) => invoke<void>('cancel_download_for_rom', { romId }),
  listUpdates: () => invoke<UpdateRow[]>('list_updates'),
  updateGame: (romId: number) => invoke<void>('update_game', { romId }),
  appVersion: () => invoke<string>('app_version'),
  appUpdateNotice: () => invoke<AppUpdateStatus>('app_update_notice'),
  openReleasePage: (url: string) => invoke<void>('open_release_page', { url }),
  pythonImportNotice: () => invoke<PythonImportReport | null>('python_import_notice'),
  layoutMigrationNotice: () => invoke<LayoutMigrationNotice | null>('layout_migration_notice'),
};
