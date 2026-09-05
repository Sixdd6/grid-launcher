import { describe, expect, it } from 'vitest';
import type { InstalledGame } from './api';
import { HOVER_DELAY_MS, startupCover } from './background';

function row(overrides: Partial<InstalledGame>): InstalledGame {
  // Only the three fields `startupCover` reads are meaningful; the rest are
  // filled from the registry's own "blank, never null" convention.
  return {
    title: 'Chrono Trigger', platform: 'SNES', rom_id: 42, rom_file_name: '', archive_path: '',
    extracted_path: '', extracted_dir: '', multi_file_game_dir: '', description: '', rating: '',
    genres: '', regions: '', languages: '', tags: '', revision: '', companies: '',
    first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
    cover_small_path: '', cover_large_path: '', screenshot_urls: '', fanart_urls: '', native_executable_path: '',
    native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
    native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '',
    ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '', last_played_at: 0,
    ...overrides,
  };
}

describe('startupCover', () => {
  it('is null when there is nothing installed', () => {
    expect(startupCover([])).toBeNull();
  });

  it('picks the newest row that actually has a large cover', () => {
    expect(
      startupCover([
        row({ installed_at: 100, cover_large_path: 'https://romm/old.png' }),
        row({ installed_at: 300, cover_large_path: 'https://romm/newest.png' }),
        row({ installed_at: 200, cover_large_path: 'https://romm/middle.png' }),
      ]),
    ).toBe('https://romm/newest.png');
  });

  it('skips cover-less rows rather than returning a blank', () => {
    expect(
      startupCover([
        row({ installed_at: 900, cover_large_path: '' }),
        row({ installed_at: 100, cover_large_path: 'https://romm/only.png' }),
      ]),
    ).toBe('https://romm/only.png');
  });

  it('is null when no row has a cover at all', () => {
    expect(startupCover([row({ installed_at: 5 }), row({ installed_at: 6 })])).toBeNull();
  });

  it('holds the 500ms hover dwell from design section 3', () => {
    expect(HOVER_DELAY_MS).toBe(500);
  });
});
