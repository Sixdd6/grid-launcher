import { invoke } from '@tauri-apps/api/core';

// Field names below mirror the Rust structs' actual serde output exactly
// (see rewrite/crates/grid-core/src/session.rs and romm/mod.rs) — no
// cosmetic renames.
export type SessionState = { connected: boolean; username: string; server_url: string };
export type Platform = { id: number; name: string; slug: string; rom_count: number };
export type GameSummary = { id: number; name: string; platform_id: number; path_cover_small: string | null };

export type DownloadStatus =
  | 'queued'
  | 'downloading'
  | 'installing'
  | 'cancelling'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type DownloadEntry = {
  id: number;
  job: 'game' | 'emulator';
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
  disabled_reason: string | null;
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

export const api = {
  connect: (serverUrl: string, username: string, secret: string, useToken: boolean) =>
    invoke<SessionState>('connect', { serverUrl, username, secret, useToken }),
  restoreSession: () => invoke<SessionState | null>('restore_session'),
  disconnect: () => invoke<void>('disconnect'),
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  listGames: (platformId: number) => invoke<GameSummary[]>('list_games', { platformId }),
  ensureCover: (gameId: number, coverPath: string) =>
    invoke<string>('ensure_cover', { gameId, coverPath }),
  installGame: (romId: number) => invoke<void>('install_game', { romId }),
  cancelInstall: (entryId: number) => invoke<void>('cancel_install', { entryId }),
  retryInstall: (entryId: number) => invoke<void>('retry_install', { entryId }),
  dismissDownload: (entryId: number) => invoke<void>('dismiss_download', { entryId }),
  uninstallGame: (romId: number) => invoke<void>('uninstall_game', { romId }),
  listDownloads: () => invoke<DownloadsSnapshot>('list_downloads'),
  listInstalled: () => invoke<InstalledGame[]>('list_installed'),
  getLibraryPath: () => invoke<string>('get_library_path'),
  setLibraryPath: (path: string) => invoke<void>('set_library_path', { path }),
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
};
