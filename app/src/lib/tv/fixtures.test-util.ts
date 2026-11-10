// Test fixtures for the TV mode modules' tests.
import type { DownloadEntry, DriveGame, DriveInfo, GameSummary, InstalledGame, PackSummary } from '../api';

export function row(o: Partial<InstalledGame> = {}): InstalledGame {
  return {
    title: 'Chrono Trigger', platform: 'Super Nintendo Entertainment System', platform_slug: 'snes', rom_id: 42,
    rom_file_name: '', archive_path: '', extracted_path: '', extracted_dir: '', multi_file_game_dir: '',
    description: 'A time-travel RPG.\nSecond paragraph.', rating: '', genres: '', regions: '', languages: '', tags: '', revision: '',
    companies: '', first_release_date: '795571200', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
    cover_small_path: 's.png', cover_large_path: 'l.png', screenshot_urls: '', fanart_urls: '',
    native_executable_path: '', native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
    native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '', ps3_iso_path: '', ps4_game_id: '',
    ps4_content: '', ra_id: '', last_played_at: 0, ...o,
  } as InstalledGame;
}

export function summary(o: Partial<GameSummary> = {}): GameSummary {
  return {
    id: 501, name: 'Starlit Run', platform_id: 1, path_cover_small: 's.png', path_cover_large: null, screenshot_urls: [],
    fanart_urls: [], platform_display_name: 'SNES', genres: [], created_at: '', first_release_date: 1_794_000_000,
    logo_url: '', fs_size_bytes: 0, largest_file_bytes: null, ...o,
  };
}

export const entry = (o: Partial<DownloadEntry>): DownloadEntry =>
  ({
    id: 1, job: 'game', kind: 'base', rom_id: 501, source_id: '', title: 'Game', platform: 'SNES', status: 'queued',
    downloaded_bytes: 0, total_bytes: 0, speed_bps: 0, install_processed_bytes: 0, install_total_bytes: 0,
    error: '', detail: '', drive_id: '', drive_name: '', drive_label: '', batch: 0, ...o,
  }) as DownloadEntry;

export function packSummary(o: Partial<PackSummary> = {}): PackSummary {
  return { name: 'Couch kit', os: ['linux'], game_count: 2, gridpack_bytes: 0, emulators: [], firmware: [], updated: '2026-11-09T00:00:00Z', rom_ids: [], ...o };
}

export function driveInfo(key: string, o: Partial<DriveInfo> = {}): DriveInfo {
  return {
    key, mount: key, label: 'SAMSUNG T7', fs: 'exFAT', fat32: false, total_bytes: 1, free_bytes: 1,
    read_only: false, status: 'ready', problem: '', pack: packSummary(), ...o,
  } as DriveInfo;
}

export function driveGame(o: Partial<DriveGame> = {}): DriveGame {
  return {
    rom_id: 901, server: 'https://romm.home.lan', name: 'Cinder Peaks', platform: 'ps2', platform_name: 'PlayStation 2',
    size: 1_000_000_000, files_max: 0, cover: 'covers/901.png', installed: false, summary: 'On the drive.', region: '',
    languages: '', tags: '', revision: '', rating: '', companies: [], release_date: '', genres: [], game_modes: [],
    franchises: [], players: '', version: '', fanart: '', logo: '', screenshots: [], added: '2026-11-01T00:00:00Z',
    installed_copy: false, installed_version: '', outdated: false, saves_copied: false, ...o,
  } as DriveGame;
}
