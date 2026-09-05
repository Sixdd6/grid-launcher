import { invoke } from '@tauri-apps/api/core';

// Field names below mirror the Rust structs' actual serde output exactly
// (see rewrite/crates/grid-core/src/session.rs and romm/mod.rs) — no
// cosmetic renames.
export type SessionState = { connected: boolean; username: string; server_url: string };
export type RestoreOutcome =
  | { kind: 'no_session' }
  | { kind: 'connected'; state: SessionState }
  | { kind: 'unreachable'; server_url: string; username: string; error: string };
export type Platform = { id: number; name: string; slug: string; rom_count: number };
/** One platform a batched emulator/core lookup is asking about. Field names
 *  match the backend's `PlatformRef` (app/src-tauri/src/commands.rs). */
export type PlatformRef = { name: string; slug: string };

/** The three card sizes design §5 offers, in `ui.card_size_*`. */
export type CardSizeName = 'small' | 'medium' | 'large';

/** Desktop shell appearance, mirroring `grid_core::config::UiSettings`. */
export type UiSettings = {
  theme: 'system' | 'dark' | 'light';
  background_fade: number;
  card_size_library: CardSizeName;
  card_size_server: CardSizeName;
};
export type GameSummary = {
  id: number;
  name: string;
  platform_id: number;
  path_cover_small: string | null;
  path_cover_large: string | null;
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
  youtube_video_id: string;
  video_path: string;
  is_identified: boolean;
  related: RelatedGame[];
};

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
export type DownloadJob = 'game' | 'emulator' | 'firmware';

// What an entry installs — finer-grained than `job`.
export type DownloadKind =
  | 'base'
  | 'update'
  | 'ps4_content'
  | 'xbox360_content'
  | 'native_update'
  | 'emulator'
  | 'compat_tool'
  | 'firmware';

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
export type EmulatorEntry = {
  name: string;
  path: string;
  args: string;
  source_id?: string;
  source_provider?: string;
  source_owner?: string;
  source_repo?: string;
  source_release_tag?: string;
  save_strategy?: string;
  ignore_files?: string;
  ignore_extensions?: string;
  save_paths?: string;
  state_paths?: string;
};

export type ProfileSummary = { name: string; args: string };

export type RaStatus = { username: string; token_present: boolean };
export type RaFanOutRow = { emulator: string; changed: boolean };

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
};

// Cloud save/state sync (rewrite/app/src-tauri/src/cloud_service.rs,
// rewrite/app/src-tauri/src/commands/cloud.rs). `game` for every cloud
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

export type RestoreReport = { ok: boolean; messages: CloudMessage[] };

export type NativeSavePaths = { pcgw: string[]; manual: string[] };

export type CloudSettings = {
  download_on_launch: boolean;
  upload_on_exit: boolean;
  skip_if_local_newer: boolean;
  upload_delay_seconds: number;
  retention_limit: number;
};

// Install specials: extra content, native launch settings, compat tools and
// the RPCS3 PS3 firmware button
// (rewrite/app/src-tauri/src/commands/specials.rs).

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
};

/// `kind` is `'wine'` or `'proton'`; `source` is `'system'`, `'steam'` or
/// `'managed'`.
export type CompatTool = { name: string; kind: string; path: string; source: string };

export type CompatToolsDto = { tools: CompatTool[]; default_tool: string };

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

// Server updates (rewrite/app/src-tauri/src/update_service.rs,
// rewrite/app/src-tauri/src/commands/updates.rs).

/// One installed game with a newer version on the server. `label` is the
/// button text, already resolved ('Update' or 'Update to v1.2.0').
export type UpdateRow = { rom_id: number; label: string };

/// Emitted after every update-set recompute (connect, install, uninstall),
/// and on disconnect with an empty array — but only when the set was
/// non-empty, so a disconnect from an up-to-date library emits nothing.
export const UPDATES_CHANGED_EVENT = 'updates-changed';

// Launcher self-update (rewrite/app/src-tauri/src/app_update.rs).

export type AppUpdateNotice = { tag: string; url: string };
/** `app_update_notice`'s payload: the notice, if any, and when the startup
 *  check completed (RFC 3339 UTC) — `null` when it was skipped or failed. */
export type AppUpdateStatus = { notice: AppUpdateNotice | null; checked_at: string | null };
/// Emitted at most once per process when a newer launcher release exists.
export const APP_UPDATE_EVENT = 'app-update-available';

export const api = {
  connect: (serverUrl: string, username: string, secret: string, useToken: boolean) =>
    invoke<SessionState>('connect', { serverUrl, username, secret, useToken }),
  restoreSession: () => invoke<RestoreOutcome>('restore_session'),
  retryConnect: () => invoke<SessionState>('retry_connect'),
  disconnect: () => invoke<void>('disconnect'),
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  listGames: (platformId: number) => invoke<GameSummary[]>('list_games', { platformId }),
  getRomDetail: (romId: number) => invoke<RomDetail>('get_rom_detail', { romId }),
  ensureImage: (url: string) => invoke<string>('ensure_image', { url }),
  ensureVideo: (url: string) => invoke<string>('ensure_video', { url }),
  installGame: (romId: number) => invoke<void>('install_game', { romId }),
  cancelInstall: (entryId: number) => invoke<void>('cancel_install', { entryId }),
  retryInstall: (entryId: number) => invoke<void>('retry_install', { entryId }),
  dismissDownload: (entryId: number) => invoke<void>('dismiss_download', { entryId }),
  uninstallGame: (romId: number) => invoke<void>('uninstall_game', { romId }),
  listDownloads: () => invoke<DownloadsSnapshot>('list_downloads'),
  listInstalled: () => invoke<InstalledGame[]>('list_installed'),
  getLibraryPath: () => invoke<string>('get_library_path'),
  setLibraryPath: (path: string) => invoke<void>('set_library_path', { path }),
  getUiSettings: () => invoke<UiSettings>('get_ui_settings'),
  setUiSettings: (settings: UiSettings) => invoke<void>('set_ui_settings', { settings }),
  /** Opens the configured RomM server in the browser. The URL comes from the
   *  backend's own config read — never from the frontend. */
  openServerPage: () => invoke<void>('open_server_page'),
  launchGame: (romId: number) => invoke<GameSession>('launch_game', { romId }),
  stopGame: (sessionId: number) => invoke<void>('stop_game', { sessionId }),
  listSessions: () => invoke<SessionsSnapshot>('list_sessions'),
  listEmulators: () => invoke<EmulatorEntry[]>('list_emulators'),
  saveEmulator: (originalName: string, entry: EmulatorEntry) =>
    invoke<void>('save_emulator', { originalName, entry }),
  deleteEmulator: (name: string) => invoke<void>('delete_emulator', { name }),
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
  setRetroachievementsCredentials: (username: string, token: string) =>
    invoke<RaFanOutRow[]>('set_retroachievements_credentials', { username, token }),
  getRetroachievementsStatus: () => invoke<RaStatus>('get_retroachievements_status'),
  clearRetroachievementsCredentials: () => invoke<void>('clear_retroachievements_credentials'),
  cloudPanelInfo: (game: InstalledGame, saveType: SaveType) =>
    invoke<CloudPanelInfo>('cloud_panel_info', { game, saveType }),
  cloudRecords: (game: InstalledGame, saveType: SaveType) =>
    invoke<CloudRecord[]>('cloud_records', { game, saveType }),
  cloudUpload: (game: InstalledGame, saveType: SaveType) =>
    invoke<UploadReport>('cloud_upload', { game, saveType }),
  cloudRestore: (game: InstalledGame, saveType: SaveType, recordId: string | null) =>
    invoke<RestoreReport>('cloud_restore', { game, saveType, recordId }),
  cloudDelete: (saveType: SaveType, recordId: number) =>
    invoke<void>('cloud_delete', { saveType, recordId }),
  nativeSavePaths: (game: InstalledGame) => invoke<NativeSavePaths>('native_save_paths', { game }),
  nativeAddManualSavePath: (game: InstalledGame, path: string) =>
    invoke<void>('native_add_manual_save_path', { game, path }),
  nativeRemoveManualSavePath: (game: InstalledGame, path: string) =>
    invoke<void>('native_remove_manual_save_path', { game, path }),
  cloudSettings: () => invoke<CloudSettings>('cloud_settings'),
  setCloudSettings: (settings: CloudSettings) => invoke<void>('set_cloud_settings', { settings }),
  installContent: (romId: number, kind: ContentKind) =>
    invoke<void>('install_content', { romId, kind }),
  installNativeUpdate: (romId: number) => invoke<void>('install_native_update', { romId }),
  contentAvailability: (romId: number) =>
    invoke<ContentAvailability>('content_availability', { romId }),
  nativeGameSettings: (romId: number) =>
    invoke<NativeGameSettings>('native_game_settings', { romId }),
  setNativeGameSettings: (
    romId: number,
    executable: string,
    parameters: string,
    compatTool: string,
  ) => invoke<void>('set_native_game_settings', { romId, executable, parameters, compatTool }),
  listCompatTools: () => invoke<CompatToolsDto>('list_compat_tools'),
  setDefaultCompatTool: (value: string) => invoke<void>('set_default_compat_tool', { value }),
  listCompatToolCatalog: () => invoke<CatalogEntry[]>('list_compat_tool_catalog'),
  installCompatTool: (sourceId: string) => invoke<void>('install_compat_tool', { sourceId }),
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
};
