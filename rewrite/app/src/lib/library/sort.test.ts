import { describe, expect, it } from 'vitest';
import type { InstalledGame } from '../api';
import { LIBRARY_SORTS, normalizeSort, sortGames, sortLabel, titleContains } from './sort';

const row = (overrides: Partial<InstalledGame>): InstalledGame => ({
  title: 'Game', platform: 'SNES', rom_id: 1, rom_file_name: '', archive_path: '',
  extracted_path: '', extracted_dir: '', multi_file_game_dir: '', description: '', rating: '',
  genres: '', regions: '', languages: '', tags: '', revision: '', companies: '',
  first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
  cover_small_path: '', cover_large_path: '', screenshot_urls: '', native_executable_path: '',
  native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
  native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '',
  ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '', last_played_at: 0,
  ...overrides,
});

describe('normalizeSort / sortLabel', () => {
  it('accepts the four stored names and falls back to title', () => {
    expect(LIBRARY_SORTS).toEqual(['played', 'installed', 'title', 'platform']);
    expect(normalizeSort('played')).toBe('played');
    expect(normalizeSort('nonsense')).toBe('title');
  });
  it('labels each sort the way design section 5 names it', () => {
    expect(LIBRARY_SORTS.map(sortLabel)).toEqual([
      'Recently played',
      'Recently installed',
      'Title',
      'Platform',
    ]);
  });
});

describe('sortGames', () => {
  const a = row({ title: 'Alpha', platform: 'SNES', installed_at: 10, last_played_at: 0 });
  const b = row({ title: 'beta', platform: 'Arcade', installed_at: 30, last_played_at: 500 });
  const c = row({ title: ' Ceta', platform: 'GBA', installed_at: 20, last_played_at: 900 });

  it('does not mutate the input', () => {
    const input = [a, b, c];
    sortGames(input, 'title');
    expect(input).toEqual([a, b, c]);
  });

  it('sorts by title case- and space-insensitively', () => {
    expect(sortGames([c, b, a], 'title').map((r) => r.title)).toEqual(['Alpha', 'beta', ' Ceta']);
  });

  it('sorts by platform then title', () => {
    expect(sortGames([a, b, c], 'platform').map((r) => r.platform)).toEqual([
      'Arcade',
      'GBA',
      'SNES',
    ]);
  });

  it('sorts most recently installed first', () => {
    expect(sortGames([a, b, c], 'installed').map((r) => r.installed_at)).toEqual([30, 20, 10]);
  });

  it('sorts most recently played first, with never-played rows last by title', () => {
    const d = row({ title: 'Delta', installed_at: 99, last_played_at: 0 });
    expect(sortGames([a, b, c, d], 'played').map((r) => r.title)).toEqual([
      ' Ceta',
      'beta',
      'Alpha',
      'Delta',
    ]);
  });
});

describe('titleContains', () => {
  it('is a case-insensitive substring test', () => {
    expect(titleContains('Chrono Trigger', 'chrono')).toBe(true);
    expect(titleContains('Chrono Trigger', 'TRIG')).toBe(true);
    expect(titleContains('Chrono Trigger', 'zelda')).toBe(false);
  });
  it('accepts everything for a blank or whitespace query', () => {
    expect(titleContains('Chrono Trigger', '')).toBe(true);
    expect(titleContains('Chrono Trigger', '   ')).toBe(true);
  });
  it('trims the query, so a trailing space still matches', () => {
    expect(titleContains('Chrono Trigger', ' chrono ')).toBe(true);
  });
});
