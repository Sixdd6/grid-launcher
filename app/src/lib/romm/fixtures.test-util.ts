// Test fixtures for the RomM storefront modules' tests.
import type { GameSummary, InstalledGame, Platform, RommStatus } from '../api';

export function rommStatus(o: Partial<RommStatus> = {}): RommStatus {
  return {
    connection: 'connected', server_url: 'https://romm.home.lan', username: 'player', auth_kind: 'token', last_error: '',
    installed_count: 0, in_rail: true, busy: false, ...o,
  };
}

export function platform(o: Partial<Platform> = {}): Platform {
  return {
    id: 1, name: 'Super Nintendo Entertainment System', slug: 'snes', rom_count: 4, custom_name: null,
    display_name: 'Super Nintendo Entertainment System', ...o,
  } as Platform;
}

export function installed(o: Partial<InstalledGame> = {}): InstalledGame {
  return {
    title: 'Chrono Trigger', platform: 'Super Nintendo Entertainment System', platform_slug: 'snes', rom_id: 42, source_server: '',
    rom_file_name: '', archive_path: '', extracted_path: '', extracted_dir: '', multi_file_game_dir: '',
    description: '', rating: '', genres: '', regions: '', languages: '', tags: '', revision: '',
    companies: '', first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
    cover_small_path: '', cover_large_path: '', screenshot_urls: '', fanart_urls: '',
    source: 'romm', store_id: '', native_executable_path: '', native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
    native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '', ps3_iso_path: '', ps4_game_id: '',
    ps4_content: '', ra_id: '', last_played_at: 0, ...o,
  } as InstalledGame;
}

export function game(o: Partial<GameSummary> = {}): GameSummary {
  return {
    id: 42, name: 'Chrono Trigger', platform_id: 1, path_cover_small: null, path_cover_large: null, screenshot_urls: [],
    fanart_urls: [], platform_display_name: 'Super Nintendo Entertainment System', genres: [], created_at: '',
    first_release_date: null, logo_url: '', fs_size_bytes: 0, largest_file_bytes: null, ...o,
  };
}
