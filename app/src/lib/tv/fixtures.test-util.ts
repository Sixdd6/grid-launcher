// Test fixtures for the TV mode modules' tests.
import type { DownloadEntry, GameSummary, InstalledGame } from '../api';

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
