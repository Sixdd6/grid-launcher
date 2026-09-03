import { describe, expect, it } from 'vitest';
import { isHiddenLibraryPlatform, visibleLibraryGames } from './library';
import type { InstalledGame } from './api';

const row = (title: string, platform: string): InstalledGame => ({
  title, platform, rom_id: 1, rom_file_name: '', archive_path: '', extracted_path: '', extracted_dir: '',
  multi_file_game_dir: '', description: '', rating: '', genres: '', regions: '', languages: '', tags: '',
  revision: '', companies: '', first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
  cover_small_path: '', cover_large_path: '', screenshot_urls: '',
  native_executable_path: '', native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '', native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '', ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '',
});

describe('library visibility (game_views.py:297-311)', () => {
  it('hides the synthetic emulator platform, case- and space-insensitively', () => {
    expect(isHiddenLibraryPlatform(' Emulators ')).toBe(true);
    expect(isHiddenLibraryPlatform('emulator')).toBe(true);
    expect(isHiddenLibraryPlatform('SNES')).toBe(false);
  });
  it('sorts by title then platform, case-folded and trimmed', () => {
    const out = visibleLibraryGames([row('zelda', 'SNES'), row(' Alpha', 'PS2'), row('alpha', 'GBA'), row('Redream', 'Emulators')]);
    expect(out.map((r) => `${r.title}|${r.platform}`)).toEqual(['alpha|GBA', ' Alpha|PS2', 'zelda|SNES']);
  });
});
